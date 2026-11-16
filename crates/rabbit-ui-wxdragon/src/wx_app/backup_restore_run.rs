//! What the Restore button on Review does when the Backups tab's choice is
//! Restore: refuse, with an OK-only warning, while REAPER or a program the
//! restore needs closed is running, then restore the ticked backups on a
//! worker thread the way a delete of backups runs, and show how it went on
//! the Done page. There is no Yes/No question: what a restore replaces goes
//! into a backup first.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use wxdragon::prelude::*;

use crate::{
    BackupRestoreRun, ProgressUiState, WizardBackupList, WizardMode, backup_restore_progress,
    backup_restore_progress_details_for_start, backup_restore_run, backup_restore_running_refusal,
    restore_running_warning, run_backup_restore, wizard_backup_restore_outcome,
};

use crate::wx_app::backup_run::{BackupRunDone, BackupRunJob, run_backup_job};
use crate::wx_app::close_guard::InstallRunState;
use crate::wx_app::globals::{show_warning, with_ui_localizer};
use crate::wx_app::uninstall_run::RunNavigation;
use crate::wx_app::widgets::selected_target_row;

/// Everything the Restore button needs, on the UI thread.
pub(crate) struct BackupRestoreRunUi {
    pub(crate) navigation: RunNavigation,
    pub(crate) backup_list: Rc<RefCell<WizardBackupList>>,
    /// The close handshake, shared with the other runs: closing the window
    /// during the restore asks, then stops once the backup it is on is done.
    pub(crate) install_run: Arc<InstallRunState>,
    /// Rebuilds the three Packages tabs' lists, as after any run: a restore
    /// writes a backup of what it replaced and changes RABBIT's records, so
    /// the Install and Uninstall tabs change too. Fired once the run returns
    /// a report.
    pub(crate) rescan: Rc<dyn Fn()>,
    /// What the run does, as Next on the Packages page fixed it.
    pub(crate) wizard_mode: Rc<Cell<WizardMode>>,
}

/// The Restore button on Review, when the choice below the Backups list is
/// Restore. Enter never gets here: Review has no default button in this
/// mode, and Enter in its text starts nothing. In any other mode it does
/// nothing at all, so no other run can end up as a restore.
pub(crate) fn start_backup_restore(ui: &BackupRestoreRunUi) {
    match ui.wizard_mode.get() {
        WizardMode::Restore => {}
        WizardMode::Install | WizardMode::Uninstall | WizardMode::DeleteBackups => return,
    }
    let navigation = &ui.navigation;
    let widgets = navigation.widgets;
    let Some(target) = selected_target_row(&navigation.model, &widgets) else {
        return;
    };
    // Take what the check and the run need, then let the list go: the
    // warning below is a modal dialog, which runs the event loop, and its
    // handlers may read the list or rebuild it.
    let mut copied = None;
    {
        let list = ui.backup_list.borrow();
        with_ui_localizer(|localizer| {
            let run = backup_restore_run(localizer, &target.path, &list);
            let details = backup_restore_progress_details_for_start(localizer, &run);
            copied = Some((run, details));
        });
    }
    let Some((run, details)) = copied else {
        return;
    };
    // The button is disabled with nothing ticked; a stale press changes
    // nothing either.
    if run.items.is_empty() {
        return;
    }
    // The engine checks the same again, by the same rules, but only after
    // the wizard has moved to Progress, and there is no way back from
    // there.
    if let Some(refusal) = backup_restore_running_refusal(&run) {
        let mut warning = None;
        with_ui_localizer(|localizer| warning = restore_running_warning(localizer, &refusal));
        if let Some((title, body)) = warning {
            show_warning(&title, &body);
            // Back where the user was, on Review's text, which still says
            // what would come back.
            widgets.review_text.set_focus();
            return;
        }
    }
    run_restore(ui, run, details);
}

/// Move to Progress and restore on a worker thread, as every backup run
/// does (see [`run_backup_job`]). The engine saves the run's report in
/// `RABBIT/logs` itself.
fn run_restore(ui: &BackupRestoreRunUi, run: BackupRestoreRun, details: String) {
    let text = &ui.navigation.model.text;
    let mut backups = None;
    with_ui_localizer(|localizer| backups = Some(backup_restore_progress(localizer, &run)));
    let job = BackupRunJob {
        mode: WizardMode::Restore,
        resource_path: run.resource_path.clone(),
        heading: text.progress_restore_heading.clone(),
        status: text.progress_status_restore_running.clone(),
        details,
        state: ProgressUiState::new(WizardMode::Restore, run.items.len(), 0)
            .with_backup_restore(backups.unwrap_or_default()),
        run,
        work: run_backup_restore,
        done: |localizer, run, result| {
            let outcome = wizard_backup_restore_outcome(localizer, run, result);
            BackupRunDone {
                heading: outcome.heading,
                status_text: outcome.status_text,
                detail_lines: outcome.detail_lines,
            }
        },
    };
    run_backup_job(&ui.navigation, &ui.install_run, &ui.rescan, job);
}
