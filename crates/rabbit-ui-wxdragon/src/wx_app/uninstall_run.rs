//! What the Uninstall button on Review does: refuse while something the
//! removal needs closed is running, ask once more when the run can't be
//! undone from a backup or a package's manifest asks for it, then run the
//! uninstall on a worker thread the way an install runs, and show how it
//! went on the Done page.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rabbit_core::model::Platform;
use rabbit_core::preflight::{running_processes_named, running_reaper_processes};
use rabbit_core::progress::ProgressReporter;
use rabbit_core::uninstall::{
    UninstallPlan, UninstallPreflightOptions, UninstallReport, UninstallRequest, execute_uninstall,
    run_uninstall_preflight_with_processes,
};
use wxdragon::prelude::*;
use wxdragon::widgets::SimpleBook;

use crate::{
    ProgressUiState, UninstallBlocker, WizardMode, WizardModel, WizardUninstallList,
    save_wizard_uninstall_outcome, uninstall_blocker, uninstall_confirmation_body,
    uninstall_progress_details_for_start, uninstall_request_for_target, wizard_uninstall_outcome,
    wizard_uninstall_outcome_from_error,
};

use crate::wx_app::close_guard::InstallRunState;
use crate::wx_app::globals::{
    arm_post_install_hook, fire_post_install_hook, show_warning, with_ui_frame, with_ui_localizer,
};
use crate::wx_app::progress_ui::apply_progress_event_to_ui;
use crate::wx_app::widgets::{
    WizardWidgets, can_launch_last_reaper_path, clone_last_resource_path,
    planned_reaper_launch_path_for_target, reapack_ack_confirmed, selected_target_row, set_heading,
    set_last_path, set_last_resource_path, target_is_valid, update_navigation,
};
use crate::wx_app::{DONE_STEP, PROGRESS_STEP};

/// The handles the run needs to move the wizard on, Progress at the start
/// and Done at the end. Everything here is `Send`, so the worker's
/// completion can carry it back to the UI thread in `call_after`.
#[derive(Clone)]
pub(crate) struct UninstallNavigation {
    pub(crate) model: Arc<WizardModel>,
    pub(crate) widgets: WizardWidgets,
    pub(crate) book: SimpleBook,
    pub(crate) step_label: StaticText,
    pub(crate) labels: Arc<Vec<String>>,
    pub(crate) back: Button,
    pub(crate) next: Button,
    pub(crate) install: Button,
    pub(crate) close: Button,
    pub(crate) current_step: Arc<AtomicUsize>,
    /// The REAPER "Open REAPER" on Done starts, and the folder "Open
    /// resource folder" opens.
    pub(crate) last_reaper_app_path: Arc<Mutex<Option<PathBuf>>>,
    pub(crate) last_resource_path: Arc<Mutex<Option<PathBuf>>>,
}

impl UninstallNavigation {
    fn go_to(&self, step: usize) {
        self.current_step.store(step, Ordering::SeqCst);
        update_navigation(
            step,
            &self.book,
            &self.step_label,
            self.labels.as_slice(),
            &self.back,
            &self.next,
            &self.install,
            &self.close,
            &self.widgets.language_footer,
            WizardMode::Uninstall,
            // Progress and Done never enable the run button, whatever the
            // flag says.
            false,
            target_is_valid(&self.model, &self.widgets),
            reapack_ack_confirmed(&self.widgets),
        );
    }
}

/// Everything the Uninstall button needs, on the UI thread.
pub(crate) struct UninstallRunUi {
    pub(crate) navigation: UninstallNavigation,
    pub(crate) uninstall_list: Rc<RefCell<WizardUninstallList>>,
    /// The close handshake, shared with the install: closing the window
    /// during the run asks, then stops after the package it is on.
    pub(crate) install_run: Arc<InstallRunState>,
    /// Rebuilds both Packages tabs from fresh detection, as after an
    /// install. Fired once the run returns a report.
    pub(crate) rescan: Rc<dyn Fn()>,
}

/// The Uninstall button on Review. Enter never gets here: Review has no
/// default button in uninstall mode, and Enter in its text starts nothing.
pub(crate) fn start_uninstall(ui: &UninstallRunUi) {
    let navigation = &ui.navigation;
    let model = &navigation.model;
    let widgets = navigation.widgets;
    let Some(target) = selected_target_row(model, &widgets) else {
        return;
    };
    // Take what the checks, the dialogs and the run need, then let the list
    // go: a modal dialog runs the event loop, and its handlers may read it.
    let list = ui.uninstall_list.borrow();
    let package_ids = list.ticked_package_ids();
    // The button is disabled with nothing ticked; a stale press changes
    // nothing either.
    if package_ids.is_empty() {
        return;
    }
    let remove_settings = widgets.uninstall_remove_settings.get_value();
    let request = uninstall_request_for_target(model, &target, &list, remove_settings);
    let plan = list.plan.clone();
    let display_names: HashMap<String, String> = list
        .ticked_rows()
        .map(|row| (row.package_id.clone(), row.display_name.clone()))
        .collect();
    let mut question = None;
    let mut details = String::new();
    with_ui_localizer(|localizer| {
        if let Some(plan) = &plan {
            question = uninstall_confirmation_body(localizer, plan, &package_ids);
        }
        details = uninstall_progress_details_for_start(localizer, &target, &list, remove_settings);
    });
    drop(list);

    let request = match request {
        Ok(request) => request,
        Err(error) => {
            // Review keeps the button disabled for both reasons this fails,
            // so this only says plainly what a stale press ran into.
            set_last_resource_path(&navigation.last_resource_path, Some(target.path.clone()));
            let result = Err(error);
            show_uninstall_outcome(
                navigation,
                &target.path,
                &package_ids,
                remove_settings,
                &result,
            );
            return;
        }
    };

    // The engine checks the same again, but only after the wizard has moved
    // to Progress, and there is no way back from there.
    if let Some(blocker) = plan
        .as_ref()
        .and_then(|plan| programs_to_close(plan, &request))
    {
        show_warning(&blocker.title, &blocker.body);
        return;
    }
    if let Some(body) = question
        && !confirm_uninstall(model, &body)
    {
        // Back where the user decided, rather than on the button that
        // asked: the summary says what would have happened.
        widgets.review_text.set_focus();
        return;
    }
    run_uninstall(ui, request, details, display_names);
}

/// What has to be closed before uninstalling, read from the uninstall
/// preflight's process checks, the very checks the engine makes: REAPER
/// (every REAPER, for a package any of them can load) and the programs the
/// ticked packages name.
fn programs_to_close(plan: &UninstallPlan, request: &UninstallRequest) -> Option<UninstallBlocker> {
    let named: Vec<String> = plan
        .items
        .iter()
        .filter(|item| request.package_ids.contains(&item.package_id))
        .flat_map(|item| item.blocking_programs.iter().cloned())
        .collect();
    let reapers = running_reaper_processes(Platform::current());
    let programs = running_processes_named(&named);
    let preflight = run_uninstall_preflight_with_processes(
        plan,
        &request.package_ids,
        // Only the process checks are read here. A dry run's preflight
        // leaves out the macOS write rehearsal, which renames files.
        &UninstallPreflightOptions {
            dry_run: true,
            target_app_path: request.target_app_path.clone(),
        },
        &reapers,
        &programs,
    );
    let running: Vec<String> = programs.into_iter().map(|process| process.name).collect();
    let mut blocker = None;
    with_ui_localizer(|localizer| {
        blocker = uninstall_blocker(localizer, plan, &request.package_ids, &preflight, &running);
    });
    blocker
}

/// Ask before an uninstall a backup can't undo, or one a package's manifest
/// asks about, such as OSARA's. `true` when the user said Yes.
///
/// `wxNO_DEFAULT` puts focus on **No**, as when stopping an install: Enter
/// or Space pressed out of habit, or a screen reader user tabbing through
/// the dialog, must not be what removes REAPER for good.
fn confirm_uninstall(model: &WizardModel, body: &str) -> bool {
    let mut confirmed = false;
    with_ui_frame(|frame| {
        let dialog = MessageDialog::builder(frame, body, &model.text.confirm_uninstall_title)
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

/// Move to Progress and run the uninstall on a worker thread. Progress
/// events come back through `call_after`, as an install's do, and the
/// completion shows Done.
fn run_uninstall(
    ui: &UninstallRunUi,
    request: UninstallRequest,
    details: String,
    display_names: HashMap<String, String>,
) {
    let navigation = ui.navigation.clone();
    let model = Arc::clone(&navigation.model);
    let widgets = navigation.widgets;

    navigation.go_to(PROGRESS_STEP);
    navigation.back.enable(false);
    navigation.next.enable(false);
    navigation.install.enable(false);
    widgets.done_launch_reaper.enable(false);
    widgets.done_open_resource.enable(false);
    set_heading(
        &widgets.progress_heading,
        &model.text.progress_uninstall_heading,
    );
    widgets
        .progress_status
        .set_label(&model.text.progress_status_uninstall_running);
    widgets.progress_gauge.set_value(0);
    widgets.progress_details.set_value(&details);
    // Onto the gauge, as the version check does: focus would otherwise stay
    // on the Uninstall button, which the page change has just hidden, and
    // the screen reader would say nothing about the run.
    widgets.progress_gauge.set_focus();
    if let Some(target) = selected_target_row(&model, &widgets) {
        set_last_path(
            &navigation.last_reaper_app_path,
            Some(planned_reaper_launch_path_for_target(&target)),
        );
    }
    set_last_resource_path(
        &navigation.last_resource_path,
        Some(request.resource_path.clone()),
    );

    // Rebuilt from fresh detection once the run is over: the Install tab
    // offers what was removed again, unticked, because the run recorded it
    // as declined.
    let rescan = Rc::clone(&ui.rescan);
    arm_post_install_hook(move || rescan());

    let state = Arc::new(Mutex::new(ProgressUiState::new(
        WizardMode::Uninstall,
        request.package_ids.len(),
        0,
    )));
    let package_display_names = Arc::new(display_names);
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

    let cancel = ui.install_run.begin(WizardMode::Uninstall);
    let install_run = Arc::clone(&ui.install_run);
    std::thread::spawn(move || {
        let result = execute_uninstall(&request, &progress, &cancel);
        operation_finished.store(true, Ordering::Relaxed);
        wxdragon::call_after(Box::new(move || {
            show_uninstall_outcome(
                &navigation,
                &request.resource_path,
                &request.package_ids,
                request.remove_settings,
                &result,
            );
            if result.is_ok() {
                fire_post_install_hook();
            }
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

/// Show how the run went on Done, with focus on its status text, which the
/// screen reader then reads. A run that left no report of the engine's gets
/// the wizard's own.
fn show_uninstall_outcome(
    navigation: &UninstallNavigation,
    resource_path: &std::path::Path,
    package_ids: &[String],
    remove_settings: bool,
    result: &rabbit_core::Result<UninstallReport>,
) {
    let model = &navigation.model;
    let widgets = navigation.widgets;
    widgets.progress_gauge.set_value(100);
    let mut outcome = None;
    with_ui_localizer(|localizer| {
        let mut built = match result {
            Ok(report) => wizard_uninstall_outcome(localizer, package_ids, report),
            Err(error) => wizard_uninstall_outcome_from_error(
                model,
                localizer,
                resource_path,
                package_ids,
                remove_settings,
                error,
            ),
        };
        // Best effort, as for an install: the run is over either way.
        if let Err(error) = save_wizard_uninstall_outcome(localizer, &mut built) {
            eprintln!("could not save the uninstall outcome report: {error}");
        }
        outcome = Some(built);
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
    // A REAPER the run removed is gone from its path, and so is the button.
    // No package RABBIT removes so far takes REAPER with it.
    widgets
        .done_launch_reaper
        .enable(can_launch_last_reaper_path(
            &navigation.last_reaper_app_path,
        ));
    widgets
        .done_open_resource
        .enable(clone_last_resource_path(&navigation.last_resource_path).is_some());
    navigation.go_to(DONE_STEP);
    widgets.done_status.set_focus();
}
