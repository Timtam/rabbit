//! The progress page's bookkeeping, apart from wxWidgets: how far a run has
//! got, for the gauge, and which of its events a screen reader hears.

use std::collections::HashMap;

use rabbit_core::localization::Localizer;
use rabbit_core::model::Platform;
use rabbit_core::progress::{ProgressEvent, RemovalMethod};

use super::uninstall::WizardMode;

/// State carried across [`ProgressEvent`] notifications during a wizard
/// run. Holds the totals the run's handler pre-computed up front (so the
/// gauge percentage is a fraction of completed work, not a guess) plus
/// per-package byte counters for every download currently streaming — the
/// core pipelines downloads on a small concurrent pool, so several packages
/// can be mid-download at once (FFmpeg streaming while REAPER installs).
/// Mutated only on the UI thread inside each `call_after` closure; the
/// `Arc<Mutex<…>>` around it there is purely so the closures satisfy `Send`.
#[derive(Debug, Clone)]
pub struct ProgressUiState {
    /// Packages the run installs or uninstalls.
    pub total_packages: usize,
    /// The phases each package goes through; see [`phases_per_package`].
    pub phases_per_package: usize,
    /// Total opted-in configuration steps. Each contributes one phase.
    pub total_configuration_steps: usize,
    /// Phases finished so far across all packages and configuration steps.
    /// [`ProgressUiState::percentage`] caps the gauge at 100 should it ever
    /// pass [`ProgressUiState::total_phases`].
    pub completed_phases: usize,
    /// Byte counters of every in-flight download, keyed by package id:
    /// `(bytes_downloaded, content_length_if_known)`. Inserted on
    /// `DownloadStarted`, updated on `DownloadProgress`, removed on
    /// `DownloadCompleted` (which also bumps `completed_phases`).
    pub active_downloads: HashMap<String, (u64, Option<u64>)>,
    /// The package being installed or removed right now, if any. While set,
    /// the status label belongs to it ("Installing REAPER…") and background
    /// download ticks must not overwrite it: it is the foreground activity
    /// the user is waiting on.
    pub current_package: Option<String>,
    /// How many times a vendor's uninstaller was reported still running.
    pub still_waiting_count: usize,
    /// How many times RABBIT was reported still waiting for an ended
    /// uninstaller's files to go.
    pub waiting_for_files_count: usize,
}

/// The phases one package goes through in a run of `mode`. An install
/// downloads, then installs. An uninstall only removes: counting two
/// phases there would park the gauge near 50 percent and then jump, and
/// NVDA beeps every step of the gauge. A restore or a delete has one phase
/// per backup, which the run counts in place of packages.
pub fn phases_per_package(mode: WizardMode) -> usize {
    match mode {
        WizardMode::Install => 2,
        WizardMode::Uninstall | WizardMode::Restore | WizardMode::DeleteBackups => 1,
    }
}

impl ProgressUiState {
    pub fn new(mode: WizardMode, total_packages: usize, total_configuration_steps: usize) -> Self {
        Self {
            total_packages,
            phases_per_package: phases_per_package(mode),
            total_configuration_steps,
            completed_phases: 0,
            active_downloads: HashMap::new(),
            current_package: None,
            still_waiting_count: 0,
            waiting_for_files_count: 0,
        }
    }

    /// Total phases the run will go through: each package's phases plus
    /// one per opted-in configuration step. Always at least 1 so the
    /// percentage math doesn't divide by zero on a no-op run.
    pub fn total_phases(&self) -> usize {
        (self.total_packages * self.phases_per_package + self.total_configuration_steps).max(1)
    }

    /// Gauge value in 0..=100. Combines completed phases with the byte
    /// fraction of every in-flight download (when its `Content-Length`
    /// is known) so the bar moves smoothly during a long download pull
    /// rather than jumping in step-shaped chunks. Each download's
    /// fraction is capped at its own single phase, so the sum stays
    /// monotonic no matter how many downloads overlap.
    pub fn percentage(&self) -> i32 {
        let total = self.total_phases() as f64;
        let mut fraction = self.completed_phases as f64;
        for (bytes_downloaded, bytes_total) in self.active_downloads.values() {
            if let Some(total_bytes) = bytes_total
                && *total_bytes > 0
            {
                fraction += (*bytes_downloaded as f64 / *total_bytes as f64).clamp(0.0, 1.0);
            }
        }
        ((fraction / total) * 100.0).round().clamp(0.0, 100.0) as i32
    }

    /// Summed `(bytes_downloaded, bytes_total)` across all in-flight
    /// downloads, for the aggregate status line. `bytes_total` is `None`
    /// when any active download has an unknown length.
    pub fn download_totals(&self) -> (u64, Option<u64>) {
        let mut downloaded: u64 = 0;
        let mut total: Option<u64> = Some(0);
        for (bytes_downloaded, bytes_total) in self.active_downloads.values() {
            downloaded += bytes_downloaded;
            total = match (total, bytes_total) {
                (Some(sum), Some(bytes)) => Some(sum + bytes),
                _ => None,
            };
        }
        (downloaded, total)
    }

    /// Count `event` in. The progress page reads the state afterwards for
    /// its status line, so it sees the download that just started among
    /// the active ones.
    pub fn record(&mut self, event: &ProgressEvent) {
        match event {
            ProgressEvent::DownloadStarted {
                package_id,
                bytes_total,
            } => {
                self.active_downloads
                    .insert(package_id.clone(), (0, *bytes_total));
            }
            ProgressEvent::DownloadProgress {
                package_id,
                bytes_downloaded,
                bytes_total,
            } => {
                let entry = self
                    .active_downloads
                    .entry(package_id.clone())
                    .or_insert((0, None));
                entry.0 = *bytes_downloaded;
                if bytes_total.is_some() {
                    entry.1 = *bytes_total;
                }
            }
            ProgressEvent::DownloadCompleted { package_id } => {
                self.active_downloads.remove(package_id);
                self.completed_phases += 1;
            }
            ProgressEvent::InstallStarted { package_id }
            | ProgressEvent::RemoveStarted { package_id, .. } => {
                self.current_package = Some(package_id.clone());
            }
            ProgressEvent::VendorStillWaiting { .. } => {
                self.still_waiting_count += 1;
            }
            ProgressEvent::VendorWaitingForFiles { .. } => {
                self.waiting_for_files_count += 1;
            }
            ProgressEvent::InstallCompleted { .. } | ProgressEvent::RemoveCompleted { .. } => {
                self.current_package = None;
                self.completed_phases += 1;
            }
            // The prompt itself counts no phase; the removals after it do.
            ProgressEvent::ConfigurationStarted { .. }
            | ProgressEvent::AdministratorPrompt { .. }
            | ProgressEvent::AdministratorStillWaiting { .. } => {}
            ProgressEvent::ConfigurationCompleted { .. } => {
                self.completed_phases += 1;
            }
            // The wizard doesn't restore or delete backups until Phase 2 of
            // the backup plan, so their events count nothing yet.
            ProgressEvent::RestoreStarted { .. }
            | ProgressEvent::RestoreCompleted { .. }
            | ProgressEvent::DeleteStarted { .. }
            | ProgressEvent::DeleteCompleted { .. } => {}
        }
    }
}

/// The status line said right before an administrator prompt appears, for
/// the packages named `packages` (localized names, comma-separated), on
/// `platform`, or `None` where the page says nothing new then. macOS's
/// password window keeps macOS's own text, which names osascript rather
/// than RABBIT, and VoiceOver may cut off what RABBIT says once the window
/// takes focus, so the page puts the line on its status label and in its
/// log as well as saying it. Windows' permission prompt names RABBIT
/// itself, so nothing more is said there.
pub fn administrator_prompt_line(
    localizer: &Localizer,
    platform: Option<Platform>,
    packages: &str,
) -> Option<String> {
    (platform == Some(Platform::MacOs)).then(|| {
        localizer
            .format(
                "wizard-progress-status-admin-prompt-macos",
                &[("packages", packages)],
            )
            .value
    })
}

/// The status line when macOS's password window has been open for a while
/// without an answer: it may be behind RABBIT, where the user can't hear
/// it. `None` on Windows, whose permission prompt RABBIT never waits on
/// like that.
pub fn administrator_still_waiting_line(
    localizer: &Localizer,
    platform: Option<Platform>,
) -> Option<String> {
    (platform == Some(Platform::MacOs)).then(|| {
        localizer
            .text("wizard-progress-status-admin-still-waiting-macos")
            .value
    })
}

/// The status line and the log line for the start of a removal by
/// `method`, for the package named `package`. Moving files into the backup,
/// a vendor's uninstaller and a move to the Trash each hold the status line
/// while they run. The macOS administrator command has already ended when
/// its packages' starts are sent, and each one's end follows at once,
/// saying whether it went: so only the log notes, once the command has
/// worked on a package, that it is done with it, and nothing is said twice.
/// A package whose files were all gone before root's turn comes as `Files`,
/// since no command worked on it.
pub fn removal_started_lines(
    localizer: &Localizer,
    method: RemovalMethod,
    package: &str,
) -> (Option<String>, String) {
    let line = |key: &str| localizer.format(key, &[("package", package)]).value;
    match method {
        RemovalMethod::Files => (
            Some(line("wizard-progress-status-removing")),
            line("wizard-progress-log-remove-started"),
        ),
        RemovalMethod::VendorUninstaller => {
            let running = line("wizard-progress-status-vendor-running");
            (Some(running.clone()), running)
        }
        RemovalMethod::Trash => {
            let trashing = line("wizard-progress-status-trashing");
            (Some(trashing.clone()), trashing)
        }
        RemovalMethod::Administrator => (None, line("wizard-progress-log-admin-done")),
    }
}

impl ProgressUiState {
    /// Whether the progress page moves focus to its log for `event`, once
    /// [`ProgressUiState::record`] has counted it in, so that a screen
    /// reader on Windows, which has no `announce()`, reads the line just
    /// added: only for the first time a vendor's uninstaller is reported
    /// still running in a run, and the first time RABBIT is still waiting
    /// for an ended uninstaller's files to go. The first may be waiting for
    /// an answer in a window of its own; the second for a program that has
    /// the package open to be closed. The user has to hear each once, not
    /// with every uninstaller.
    pub fn moves_focus_to_log(&self, event: &ProgressEvent) -> bool {
        match event {
            ProgressEvent::VendorStillWaiting { .. } => self.still_waiting_count == 1,
            ProgressEvent::VendorWaitingForFiles { .. } => self.waiting_for_files_count == 1,
            _ => false,
        }
    }
}

/// Whether the status line `event` sets is spoken (politely, through
/// `announce()` on macOS). Download lines change several times a second
/// and would bury everything else, so only the start of each install,
/// configuration step or removal is spoken, and the end of a removal: it
/// says whether the package really went, which an uninstall's user waits
/// to hear, and nothing else follows it until the next package. An
/// install's end needs no line of its own: the next download or install,
/// or the Done page, says what happens next. A vendor's uninstaller that
/// keeps RABBIT waiting is spoken too: it may be asking a question in a
/// window of its own. So are its files lingering after it ended: another
/// program may have the package open, and nothing else is heard until the
/// uninstaller's time limit. So is the line right before an administrator
/// prompt, which says what the window is for, and the line when its
/// password window has waited a while, which may be behind RABBIT.
///
/// No wildcard arm, on purpose: a new event must not compile until someone
/// decides whether it is heard. A `matches!` here once let the uninstall
/// events through unheard.
pub fn is_spoken(event: &ProgressEvent) -> bool {
    match event {
        ProgressEvent::InstallStarted { .. }
        | ProgressEvent::ConfigurationStarted { .. }
        | ProgressEvent::RemoveStarted { .. }
        | ProgressEvent::RemoveCompleted { .. }
        | ProgressEvent::VendorStillWaiting { .. }
        | ProgressEvent::VendorWaitingForFiles { .. }
        | ProgressEvent::AdministratorPrompt { .. }
        | ProgressEvent::AdministratorStillWaiting { .. } => true,
        ProgressEvent::DownloadStarted { .. }
        | ProgressEvent::DownloadProgress { .. }
        | ProgressEvent::DownloadCompleted { .. }
        | ProgressEvent::InstallCompleted { .. }
        | ProgressEvent::ConfigurationCompleted { .. } => false,
        // The wizard doesn't restore or delete backups until Phase 2 of the
        // backup plan, which decides what they say. Until then they say
        // nothing.
        ProgressEvent::RestoreStarted { .. }
        | ProgressEvent::RestoreCompleted { .. }
        | ProgressEvent::DeleteStarted { .. }
        | ProgressEvent::DeleteCompleted { .. } => false,
    }
}
