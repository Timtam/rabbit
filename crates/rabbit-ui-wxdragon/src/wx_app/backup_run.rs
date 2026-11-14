//! What a restore and a delete of backups do alike once they start: move
//! to Progress, run on a worker thread whose events come back through
//! `call_after`, keep the close handshake, rescan the three Packages tabs
//! and show Done. Each run only says what it runs and how its pages read.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rabbit_core::cancel::CancelToken;
use rabbit_core::localization::Localizer;
use rabbit_core::progress::ProgressReporter;
use wxdragon::prelude::*;

use crate::{ProgressUiState, WizardMode};

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

/// What Done shows after a backup run.
pub(crate) struct BackupRunDone {
    /// One sentence: the progress page's last status line, and the first
    /// line of the Done page.
    pub(crate) heading: String,
    /// The Done page's status text, which gets focus and is read.
    pub(crate) status_text: String,
    /// What "Show details" shows.
    pub(crate) detail_lines: Vec<String>,
}

/// A backup run, ready to start: `J` is what the worker is given, copied
/// out of the Backups list, and `R` what the engine reports.
pub(crate) struct BackupRunJob<J, R> {
    /// Restore or DeleteBackups: the run button's label on the way, and
    /// what the close question asks.
    pub(crate) mode: WizardMode,
    pub(crate) run: J,
    /// The resource folder the run works in, which "Open resource folder"
    /// on Done opens.
    pub(crate) resource_path: PathBuf,
    /// The progress page's heading, its first status line and the first
    /// lines of its log.
    pub(crate) heading: String,
    pub(crate) status: String,
    pub(crate) details: String,
    /// The progress page's state at the start, which names the run's
    /// backups for its status and log lines.
    pub(crate) state: ProgressUiState,
    /// The run itself, on the worker thread. The engine saves its report.
    pub(crate) work: fn(&J, &ProgressReporter, &CancelToken) -> rabbit_core::Result<R>,
    /// What Done says of how it went, on the UI thread.
    pub(crate) done: fn(&Localizer, &J, &rabbit_core::Result<R>) -> BackupRunDone,
}

/// Move to Progress and run `job` on a worker thread. Progress events come
/// back through `call_after`, as an uninstall's do. Once the run returns, a
/// run that returned a report rescans the three Packages tabs, Done shows
/// how it went, and the window closes if the user asked it to meanwhile.
pub(crate) fn run_backup_job<J, R>(
    navigation: &RunNavigation,
    install_run: &Arc<InstallRunState>,
    rescan: &Rc<dyn Fn()>,
    job: BackupRunJob<J, R>,
) where
    J: Send + 'static,
    R: Send + 'static,
{
    let BackupRunJob {
        mode,
        run,
        resource_path,
        heading,
        status,
        details,
        state,
        work,
        done,
    } = job;
    let navigation = navigation.clone();
    let model = Arc::clone(&navigation.model);
    let widgets = navigation.widgets;

    navigation.go_to(PROGRESS_STEP, mode);
    navigation.back.enable(false);
    navigation.next.enable(false);
    navigation.install.enable(false);
    widgets.done_launch_reaper.enable(false);
    widgets.done_open_resource.enable(false);
    set_heading(&widgets.progress_heading, &heading);
    widgets.progress_status.set_label(&status);
    widgets.progress_gauge.set_value(0);
    widgets.progress_details.set_value(&details);
    // Onto the gauge, as for an uninstall: focus would otherwise stay on
    // the run button, which the page change has just hidden.
    widgets.progress_gauge.set_focus();
    if let Some(target) = selected_target_row(&model, &widgets) {
        set_last_path(
            &navigation.last_reaper_app_path,
            Some(planned_reaper_launch_path_for_target(&target)),
        );
    }
    set_last_resource_path(&navigation.last_resource_path, Some(resource_path));

    // The three lists are read again once the run is over: a restore
    // writes a backup of what it replaced and changes RABBIT's records,
    // and a delete removes sets.
    let rescan = Rc::clone(rescan);
    arm_post_install_hook(move || rescan());

    let state = Arc::new(Mutex::new(state));
    // A backup run names its backups through the state; it has no package
    // or configuration step names to look up.
    let package_display_names: Arc<HashMap<String, String>> = Arc::new(HashMap::new());
    let configuration_display_names: Arc<HashMap<String, String>> = Arc::new(HashMap::new());
    // Nothing repaints the page once the completion has written the
    // outcome, even an event still queued behind it.
    let operation_finished = Arc::new(AtomicBool::new(false));
    let progress = {
        let operation_finished = Arc::clone(&operation_finished);
        let install_run = Arc::clone(install_run);
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

    let cancel = install_run.begin(mode);
    let install_run = Arc::clone(install_run);
    std::thread::spawn(move || {
        let result = work(&run, &progress, &cancel);
        operation_finished.store(true, Ordering::Relaxed);
        wxdragon::call_after(Box::new(move || {
            // The rescan first: it rebuilds the hidden Packages page, and
            // focus on Done's status must be the last focus change, or the
            // screen reader reads something else. A run that returned an
            // error changed nothing.
            if result.is_ok() {
                fire_post_install_hook();
            }
            let mut outcome = None;
            with_ui_localizer(|localizer| outcome = Some(done(localizer, &run, &result)));
            if let Some(outcome) = outcome {
                show_backup_run_done(&navigation, mode, &outcome);
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

/// Show how a backup run went on Done, with focus on its status text, which
/// the screen reader then reads. Open REAPER is enabled when that REAPER is
/// there, as after an uninstall. Back stays disabled: after a run, RABBIT
/// is started again.
fn show_backup_run_done(navigation: &RunNavigation, mode: WizardMode, outcome: &BackupRunDone) {
    let widgets = navigation.widgets;
    widgets.progress_gauge.set_value(100);
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
    navigation.go_to(DONE_STEP, mode);
    widgets.done_status.set_focus();
}
