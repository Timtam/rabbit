//! What the Delete button on Review does when the Backups tab's choice is
//! Delete: ask once, with No focused, then delete the ticked backups on a
//! worker thread the way an uninstall runs, and show how it went on the
//! Done page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use wxdragon::prelude::*;

use crate::{
    BackupDeleteRun, ProgressUiState, WizardBackupList, WizardMode, backup_delete_progress,
    backup_delete_progress_details_for_start, backup_delete_run, can_delete_backups,
    delete_backups_question, run_backup_delete, wizard_backup_delete_outcome,
};

use crate::wx_app::backup_run::{BackupRunDone, BackupRunJob, run_backup_job};
use crate::wx_app::close_guard::InstallRunState;
use crate::wx_app::globals::{with_ui_frame, with_ui_localizer};
use crate::wx_app::uninstall_run::RunNavigation;
use crate::wx_app::widgets::selected_target_row;

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
                can_delete_backups(&list),
            ));
        });
    }
    let Some((run, (title, body), details, can_delete)) = copied else {
        return;
    };
    // The button is disabled with nothing ticked, or while Review blocks
    // the delete, as when a ticked backup would take unticked ones with it;
    // a stale press changes nothing either.
    if run.items.is_empty() || !can_delete {
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

/// Move to Progress and delete on a worker thread, as every backup run
/// does (see [`run_backup_job`]). The engine saves the run's report in
/// `RABBIT/logs` itself.
fn run_delete(ui: &BackupDeleteRunUi, run: BackupDeleteRun, details: String) {
    let text = &ui.navigation.model.text;
    let mut backups = None;
    with_ui_localizer(|localizer| backups = Some(backup_delete_progress(localizer, &run)));
    let job = BackupRunJob {
        mode: WizardMode::DeleteBackups,
        resource_path: run.resource_path.clone(),
        heading: text.progress_delete_heading.clone(),
        status: text.progress_status_delete_running.clone(),
        details,
        // One phase per set: the engine starts and ends each set once,
        // however many of its rows are ticked.
        state: ProgressUiState::new(WizardMode::DeleteBackups, run.set_count(), 0)
            .with_backup_delete(backups.unwrap_or_default()),
        run,
        work: run_backup_delete,
        done: |localizer, run, result| {
            let outcome = wizard_backup_delete_outcome(localizer, run, result);
            BackupRunDone {
                heading: outcome.heading,
                status_text: outcome.status_text,
                detail_lines: outcome.detail_lines,
            }
        },
    };
    run_backup_job(&ui.navigation, &ui.install_run, &ui.rescan, job);
}
