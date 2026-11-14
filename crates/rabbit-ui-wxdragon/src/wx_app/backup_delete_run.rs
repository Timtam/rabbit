//! What the Delete button on Review does when the Backups tab's choice is
//! Delete: ask once, with No focused, then delete the ticked backups on a
//! worker thread the way an uninstall runs, and show how it went on the
//! Done page.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rabbit_core::backup_delete::DeleteReport;
use rabbit_core::progress::ProgressReporter;
use wxdragon::prelude::*;

use crate::{
    BackupDeleteRun, ProgressUiState, WizardBackupList, WizardMode, backup_delete_progress,
    backup_delete_progress_details_for_start, backup_delete_run, delete_backups_question,
    run_backup_delete, wizard_backup_delete_outcome,
};

use crate::wx_app::close_guard::InstallRunState;
use crate::wx_app::globals::{
    arm_post_install_hook, fire_post_install_hook, with_ui_frame, with_ui_localizer,
};
use crate::wx_app::progress_ui::apply_progress_event_to_ui;
use crate::wx_app::uninstall_run::RunNavigation;
use crate::wx_app::widgets::{
    can_launch_last_reaper_path, clone_last_resource_path, planned_reaper_launch_path_for_target,
    selected_target_row, set_heading, set_last_path, set_last_resource_path,
};
use crate::wx_app::{DONE_STEP, PROGRESS_STEP};

/// Everything the Delete button needs, on the UI thread.
pub(crate) struct BackupDeleteRunUi {
    pub(crate) navigation: RunNavigation,
    pub(crate) backup_list: Rc<RefCell<WizardBackupList>>,
    /// The close handshake, shared with the other runs: closing the window
    /// during the delete asks, then stops once the backup it is on is done.
    pub(crate) install_run: Arc<InstallRunState>,
    /// Rebuilds the three Packages tabs' lists, the Backups rows among them,
    /// as after any run. Fired once the run returns a report.
    pub(crate) rescan: Rc<dyn Fn()>,
    /// What the run does, as Next on the Packages page fixed it.
    pub(crate) wizard_mode: Rc<Cell<WizardMode>>,
}

/// The Delete button on Review, when the choice below the Backups list is
/// Delete. Enter never gets here: Review has no default button in this
/// mode, and Enter in its text starts nothing. In any other mode it does
/// nothing at all, so no other run can end up as a delete.
///
/// There is no REAPER check: deleting a backup never touches REAPER's
/// files, and nothing but a restore reads the backups.
pub(crate) fn start_backup_delete(ui: &BackupDeleteRunUi) {
    match ui.wizard_mode.get() {
        WizardMode::DeleteBackups => {}
        WizardMode::Install | WizardMode::Uninstall | WizardMode::Restore => return,
    }
    let navigation = &ui.navigation;
    let widgets = navigation.widgets;
    let Some(target) = selected_target_row(&navigation.model, &widgets) else {
        return;
    };
    // Take what the question and the run need, then let the list go: a
    // modal dialog runs the event loop, and its handlers may read the list
    // or rebuild it.
    let mut copied = None;
    {
        let list = ui.backup_list.borrow();
        with_ui_localizer(|localizer| {
            copied = Some((
                backup_delete_run(localizer, &target.path, &list),
                delete_backups_question(localizer, &list),
                backup_delete_progress_details_for_start(localizer, &target.path, &list),
            ));
        });
    }
    let Some((run, (title, body), details)) = copied else {
        return;
    };
    // The button is disabled with nothing ticked; a stale press changes
    // nothing either.
    if run.items.is_empty() {
        return;
    }
    if !confirm_delete_backups(&title, &body) {
        // Back where the user decided, rather than on the button that
        // asked: Review says what would have gone.
        widgets.review_text.set_focus();
        return;
    }
    run_delete(ui, run, details);
}

/// Ask before every delete of backups: how many go, what that frees, how
/// many are the only copy of what they hold, and that a deleted backup
/// can't be brought back. `true` when the user said Yes.
///
/// `wxNO_DEFAULT` puts focus on **No**, as before an uninstall: Enter or
/// Space pressed out of habit must not be what deletes a backup for good.
fn confirm_delete_backups(title: &str, body: &str) -> bool {
    let mut confirmed = false;
    with_ui_frame(|frame| {
        let dialog = MessageDialog::builder(frame, body, title)
            .with_style(
                MessageDialogStyle::YesNo
                    | MessageDialogStyle::IconWarning
                    | MessageDialogStyle::Centre
                    | MessageDialogStyle::NoDefault,
            )
            .build();
        confirmed = dialog.show_modal() == ID_YES;
    });
    confirmed
}

/// Move to Progress and delete on a worker thread. Progress events come
/// back through `call_after`, as an uninstall's do, and the completion
/// shows Done.
fn run_delete(ui: &BackupDeleteRunUi, run: BackupDeleteRun, details: String) {
    let navigation = ui.navigation.clone();
    let model = Arc::clone(&navigation.model);
    let widgets = navigation.widgets;

    navigation.go_to(PROGRESS_STEP, WizardMode::DeleteBackups);
    navigation.back.enable(false);
    navigation.next.enable(false);
    navigation.install.enable(false);
    widgets.done_launch_reaper.enable(false);
    widgets.done_open_resource.enable(false);
    set_heading(
        &widgets.progress_heading,
        &model.text.progress_delete_heading,
    );
    widgets
        .progress_status
        .set_label(&model.text.progress_status_delete_running);
    widgets.progress_gauge.set_value(0);
    widgets.progress_details.set_value(&details);
    // Onto the gauge, as for an uninstall: focus would otherwise stay on
    // the Delete button, which the page change has just hidden.
    widgets.progress_gauge.set_focus();
    if let Some(target) = selected_target_row(&model, &widgets) {
        set_last_path(
            &navigation.last_reaper_app_path,
            Some(planned_reaper_launch_path_for_target(&target)),
        );
    }
    set_last_resource_path(
        &navigation.last_resource_path,
        Some(run.resource_path.clone()),
    );

    // The Backups rows are read again once the run is over, so they never
    // list a set this run deleted.
    let rescan = Rc::clone(&ui.rescan);
    arm_post_install_hook(move || rescan());

    let mut backups = None;
    with_ui_localizer(|localizer| backups = Some(backup_delete_progress(localizer, &run)));
    let state = Arc::new(Mutex::new(
        ProgressUiState::new(WizardMode::DeleteBackups, run.items.len(), 0)
            .with_backup_delete(backups.unwrap_or_default()),
    ));
    // A delete names its backups through the state; the run has no package
    // or configuration step names to look up.
    let package_display_names: Arc<HashMap<String, String>> = Arc::new(HashMap::new());
    let configuration_display_names: Arc<HashMap<String, String>> = Arc::new(HashMap::new());
    // Nothing repaints the page once the completion has written the
    // outcome, even an event still queued behind it.
    let operation_finished = Arc::new(AtomicBool::new(false));
    let progress = {
        let operation_finished = Arc::clone(&operation_finished);
        let install_run = Arc::clone(&ui.install_run);
        ProgressReporter::new(move |event| {
            if operation_finished.load(Ordering::Relaxed) {
                return;
            }
            let state = Arc::clone(&state);
            let package_display_names = Arc::clone(&package_display_names);
            let configuration_display_names = Arc::clone(&configuration_display_names);
            let status_frozen = install_run.stop_requested();
            wxdragon::call_after(Box::new(move || {
                apply_progress_event_to_ui(
                    &state,
                    &widgets,
                    &package_display_names,
                    &configuration_display_names,
                    event,
                    status_frozen,
                );
            }));
        })
    };

    let cancel = ui.install_run.begin(WizardMode::DeleteBackups);
    let install_run = Arc::clone(&ui.install_run);
    std::thread::spawn(move || {
        // The engine saves the run's report in RABBIT/logs itself.
        let result = run_backup_delete(&run, &progress, &cancel);
        operation_finished.store(true, Ordering::Relaxed);
        wxdragon::call_after(Box::new(move || {
            // The rescan first, as after an uninstall: it rebuilds the hidden
            // Packages page, and focus on Done's status must be the last
            // focus change, or the screen reader reads something else. A run
            // that returned an error changed nothing.
            if result.is_ok() {
                fire_post_install_hook();
            }
            show_backup_delete_outcome(&navigation, &run, &result);
            // The worker is done and everything it held has been dropped,
            // so the window can go now, if the user asked it to.
            let close_now = install_run.stop_requested();
            install_run.finish();
            if close_now {
                with_ui_frame(|frame| frame.close(true));
            }
        }));
    });
}

/// Show how the delete went on Done, with focus on its status text, which
/// the screen reader then reads. Back stays disabled there: after a run,
/// RABBIT is started again.
fn show_backup_delete_outcome(
    navigation: &RunNavigation,
    run: &BackupDeleteRun,
    result: &rabbit_core::Result<DeleteReport>,
) {
    let widgets = navigation.widgets;
    widgets.progress_gauge.set_value(100);
    let mut outcome = None;
    with_ui_localizer(|localizer| {
        outcome = Some(wizard_backup_delete_outcome(localizer, run, result));
    });
    let Some(outcome) = outcome else {
        return;
    };
    widgets.progress_status.set_label(&outcome.heading);
    widgets.progress_details.set_value(&format!(
        "{}\n\n{}",
        outcome.heading,
        outcome.detail_lines.join("\n")
    ));
    widgets.done_status.set_value(&outcome.status_text);
    widgets
        .done_details
        .set_value(&outcome.detail_lines.join("\n"));
    widgets
        .done_launch_reaper
        .enable(can_launch_last_reaper_path(
            &navigation.last_reaper_app_path,
        ));
    widgets
        .done_open_resource
        .enable(clone_last_resource_path(&navigation.last_resource_path).is_some());
    navigation.go_to(DONE_STEP, WizardMode::DeleteBackups);
    widgets.done_status.set_focus();
}
