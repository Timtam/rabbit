//! Turns `ProgressEvent`s into the progress page's gauge, status line, and
//! log.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rabbit_core::localization::Localizer;
use rabbit_core::progress::ProgressEvent;

use crate::wx_app::globals::with_ui_localizer;
use crate::wx_app::widgets::WizardWidgets;

/// State carried across [`ProgressEvent`] notifications during a wizard
/// install. Holds the totals the install handler pre-computed up front
/// (so the gauge percentage is a fraction of completed work, not a
/// guess) plus per-package byte counters for every download currently
/// streaming — the core pipelines downloads on a small concurrent pool,
/// so several packages can be mid-download at once (FFmpeg streaming
/// while REAPER installs). Mutated only on the UI thread inside each
/// `call_after` closure; the `Arc<Mutex<…>>` wrapper is purely so the
/// closures satisfy `Send`.
#[derive(Debug, Clone)]
pub(crate) struct ProgressUiState {
    /// Total packages selected for install. Each contributes two phases
    /// (download + install) to the overall progress denominator.
    pub(crate) total_packages: usize,
    /// Total opted-in configuration steps. Each contributes one phase.
    pub(crate) total_configuration_steps: usize,
    /// Phases finished so far across all packages and configuration
    /// steps. Bounded above by `total_packages * 2 +
    /// total_configuration_steps`.
    pub(crate) completed_phases: usize,
    /// Byte counters of every in-flight download, keyed by package id:
    /// `(bytes_downloaded, content_length_if_known)`. Inserted on
    /// `DownloadStarted`, updated on `DownloadProgress`, removed on
    /// `DownloadCompleted` (which also bumps `completed_phases`).
    pub(crate) active_downloads: std::collections::HashMap<String, (u64, Option<u64>)>,
    /// Package currently running its install step, if any. While set, the
    /// status label belongs to the install ("Installing REAPER…") and
    /// background download ticks must not overwrite it — the install is
    /// the foreground activity the user is waiting on.
    pub(crate) current_install: Option<String>,
}

impl ProgressUiState {
    pub(crate) fn new(total_packages: usize, total_configuration_steps: usize) -> Self {
        Self {
            total_packages,
            total_configuration_steps,
            completed_phases: 0,
            active_downloads: std::collections::HashMap::new(),
            current_install: None,
        }
    }

    /// Total phases the install will go through: every package emits a
    /// download phase *and* an install phase, every opted-in
    /// configuration step emits one phase. Always at least 1 so the
    /// percentage math doesn't divide by zero on a no-op run.
    pub(crate) fn total_phases(&self) -> usize {
        (self.total_packages * 2 + self.total_configuration_steps).max(1)
    }

    /// Gauge value in 0..=100. Combines completed phases with the byte
    /// fraction of every in-flight download (when its `Content-Length`
    /// is known) so the bar moves smoothly during a long download pull
    /// rather than jumping in step-shaped chunks. Each download's
    /// fraction is capped at its own single phase, so the sum stays
    /// monotonic no matter how many downloads overlap.
    pub(crate) fn percentage(&self) -> i32 {
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
    pub(crate) fn download_totals(&self) -> (u64, Option<u64>) {
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
}

/// Aggregate status line for several concurrent downloads:
/// "Downloading N packages… X / Y". Used whenever more than one download
/// is in flight so the single status label doesn't flip-flop between
/// packages several times a second.
pub(crate) fn aggregate_download_status_line(
    state: &ProgressUiState,
    localizer: &Localizer,
) -> String {
    let (downloaded, total) = state.download_totals();
    let count = state.active_downloads.len().to_string();
    let downloaded = format_bytes_human(downloaded);
    let total = total.map_or_else(|| "?".to_string(), format_bytes_human);
    localizer
        .format(
            "wizard-progress-status-downloading-many",
            &[
                ("count", count.as_str()),
                ("downloaded", downloaded.as_str()),
                ("total", total.as_str()),
            ],
        )
        .value
}

/// Render a byte count in the locale-neutral form `12.4 MB`. The wizard
/// has space for at most a single inline byte counter in the status
/// label, so we always pick whichever IEC unit gives a value below 1024
/// and format it with one decimal place. Bytes (`< 1 KiB`) skip the
/// decimal entirely to avoid "0.4 B"-style nonsense.
pub(crate) fn format_bytes_human(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx + 1 < UNITS.len() {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit_idx])
    }
}

/// Apply a single [`ProgressEvent`] to the wizard's progress page.
/// Mutates the gauge value, replaces the status label, and appends a
/// new log line to the details TextCtrl (so a screen reader can read
/// each line as it lands). The package / configuration display-name
/// lookups go through the pre-built maps so this function never has to
/// touch the package spec list — the install handler builds the maps
/// once, before spawning the worker thread, and they're shared via
/// `Arc<HashMap<…>>`.
///
/// `status_frozen` holds the status label at whatever the close handler
/// wrote there. Without it the next download or install event would paint
/// "Downloading OSARA…" over "Stopping…" and the wizard would look like it
/// had ignored the user. The log lines below keep flowing either way, so the
/// step that is still finishing stays visible.
pub(crate) fn apply_progress_event_to_ui(
    state: &Arc<Mutex<ProgressUiState>>,
    widgets: &WizardWidgets,
    package_display_names: &Arc<HashMap<String, String>>,
    configuration_display_names: &Arc<HashMap<String, String>>,
    event: ProgressEvent,
    status_frozen: bool,
) {
    let mut state = state.lock().unwrap();
    let mut status_line: Option<String> = None;
    let mut log_line: Option<String> = None;

    with_ui_localizer(|localizer| match &event {
        ProgressEvent::DownloadStarted {
            package_id,
            bytes_total,
        } => {
            state
                .active_downloads
                .insert(package_id.clone(), (0, *bytes_total));
            let package = package_display_name(package_display_names, package_id);
            // Downloads only own the status label while no install is
            // running — an in-flight "Installing REAPER…" must not be
            // stomped by a background download starting. And once several
            // downloads are active, the label stays in aggregate form
            // instead of flipping to whichever package started last.
            if state.current_install.is_none() {
                status_line = Some(if state.active_downloads.len() > 1 {
                    aggregate_download_status_line(&state, localizer)
                } else {
                    localizer
                        .format(
                            "wizard-progress-status-downloading",
                            &[("package", package.as_str())],
                        )
                        .value
                });
            }
            log_line = Some(
                localizer
                    .format(
                        "wizard-progress-log-download-started",
                        &[("package", package.as_str())],
                    )
                    .value,
            );
        }
        ProgressEvent::DownloadProgress {
            package_id,
            bytes_downloaded,
            bytes_total,
        } => {
            let entry = state
                .active_downloads
                .entry(package_id.clone())
                .or_insert((0, None));
            entry.0 = *bytes_downloaded;
            if bytes_total.is_some() {
                entry.1 = *bytes_total;
            }
            // Downloads own the status label only while no install runs.
            // One active download keeps today's per-package byte line;
            // several concurrent downloads aggregate into one summary so
            // the label (and a screen reader following it) doesn't
            // flip-flop between unrelated packages several times a second.
            if state.current_install.is_none() {
                status_line = Some(if state.active_downloads.len() > 1 {
                    aggregate_download_status_line(&state, localizer)
                } else {
                    let package = package_display_name(package_display_names, package_id);
                    let downloaded = format_bytes_human(*bytes_downloaded);
                    let total = bytes_total.map_or_else(|| "?".to_string(), format_bytes_human);
                    localizer
                        .format(
                            "wizard-progress-status-downloading-with-bytes",
                            &[
                                ("package", package.as_str()),
                                ("downloaded", downloaded.as_str()),
                                ("total", total.as_str()),
                            ],
                        )
                        .value
                });
            }
            // No log line: the running log shows discrete transitions,
            // not intra-download tick-by-tick noise.
        }
        ProgressEvent::DownloadCompleted { package_id } => {
            state.active_downloads.remove(package_id);
            state.completed_phases += 1;
            let package = package_display_name(package_display_names, package_id);
            log_line = Some(
                localizer
                    .format(
                        "wizard-progress-log-download-completed",
                        &[("package", package.as_str())],
                    )
                    .value,
            );
        }
        ProgressEvent::InstallStarted { package_id } => {
            state.current_install = Some(package_id.clone());
            let package = package_display_name(package_display_names, package_id);
            status_line = Some(
                localizer
                    .format(
                        "wizard-progress-status-installing",
                        &[("package", package.as_str())],
                    )
                    .value,
            );
            log_line = Some(
                localizer
                    .format(
                        "wizard-progress-log-install-started",
                        &[("package", package.as_str())],
                    )
                    .value,
            );
        }
        ProgressEvent::InstallCompleted { package_id } => {
            state.current_install = None;
            state.completed_phases += 1;
            let package = package_display_name(package_display_names, package_id);
            log_line = Some(
                localizer
                    .format(
                        "wizard-progress-log-install-completed",
                        &[("package", package.as_str())],
                    )
                    .value,
            );
        }
        ProgressEvent::ConfigurationStarted { step_id } => {
            let step = configuration_display_name(configuration_display_names, step_id);
            status_line = Some(
                localizer
                    .format(
                        "wizard-progress-status-configuring",
                        &[("step", step.as_str())],
                    )
                    .value,
            );
            log_line = Some(
                localizer
                    .format(
                        "wizard-progress-log-configuration-started",
                        &[("step", step.as_str())],
                    )
                    .value,
            );
        }
        ProgressEvent::ConfigurationCompleted { step_id } => {
            state.completed_phases += 1;
            let step = configuration_display_name(configuration_display_names, step_id);
            log_line = Some(
                localizer
                    .format(
                        "wizard-progress-log-configuration-completed",
                        &[("step", step.as_str())],
                    )
                    .value,
            );
        }
    });

    widgets.progress_gauge.set_value(state.percentage());
    if let Some(line) = status_line
        && !status_frozen
    {
        widgets.progress_status.set_label(&line);
    }
    // Hold the lock no longer than necessary — the TextCtrl call below
    // re-enters the wxWidgets event pump, which can run other queued
    // call_after closures.
    drop(state);
    if let Some(line) = log_line {
        widgets.progress_details.append_text(&format!("\n{line}"));
    }
}

/// Resolve a `package_id` to its localized display name from the wizard
/// plan's pre-built map. Falls back to the raw id when the map doesn't
/// know the package — this only happens for synthetic test packages
/// that aren't in the wizard's PackageRow list, but the wizard should
/// still render *something* readable rather than panicking.
pub(crate) fn package_display_name(map: &Arc<HashMap<String, String>>, package_id: &str) -> String {
    map.get(package_id)
        .cloned()
        .unwrap_or_else(|| package_id.to_string())
}

/// As [`package_display_name`] but for configuration-step ids.
pub(crate) fn configuration_display_name(
    map: &Arc<HashMap<String, String>>,
    step_id: &str,
) -> String {
    map.get(step_id)
        .cloned()
        .unwrap_or_else(|| step_id.to_string())
}
