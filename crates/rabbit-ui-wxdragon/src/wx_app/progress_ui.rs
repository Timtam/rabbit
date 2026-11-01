//! Turns `ProgressEvent`s into the progress page's gauge, status line, and
//! log.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rabbit_core::localization::Localizer;
use rabbit_core::progress::ProgressEvent;

use crate::{ProgressUiState, is_spoken};

use crate::wx_app::globals::{AnnouncePriority, announce, with_ui_localizer};
use crate::wx_app::widgets::WizardWidgets;

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
/// touch the package spec list — the run's handler builds the maps
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
    state.record(&event);
    let mut status_line: Option<String> = None;
    let mut log_line: Option<String> = None;
    let package_line = |localizer: &Localizer, key: &str, package_id: &str| {
        let package = package_display_name(package_display_names, package_id);
        localizer
            .format(key, &[("package", package.as_str())])
            .value
    };

    with_ui_localizer(|localizer| match &event {
        ProgressEvent::DownloadStarted { package_id, .. } => {
            // Downloads only own the status label while no install is
            // running — an in-flight "Installing REAPER…" must not be
            // stomped by a background download starting. And once several
            // downloads are active, the label stays in aggregate form
            // instead of flipping to whichever package started last.
            if state.current_package.is_none() {
                status_line = Some(if state.active_downloads.len() > 1 {
                    aggregate_download_status_line(&state, localizer)
                } else {
                    package_line(localizer, "wizard-progress-status-downloading", package_id)
                });
            }
            log_line = Some(package_line(
                localizer,
                "wizard-progress-log-download-started",
                package_id,
            ));
        }
        ProgressEvent::DownloadProgress {
            package_id,
            bytes_downloaded,
            bytes_total,
        } => {
            // Downloads own the status label only while no install runs.
            // One active download keeps today's per-package byte line;
            // several concurrent downloads aggregate into one summary so
            // the label (and a screen reader following it) doesn't
            // flip-flop between unrelated packages several times a second.
            if state.current_package.is_none() {
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
            log_line = Some(package_line(
                localizer,
                "wizard-progress-log-download-completed",
                package_id,
            ));
        }
        ProgressEvent::InstallStarted { package_id } => {
            status_line = Some(package_line(
                localizer,
                "wizard-progress-status-installing",
                package_id,
            ));
            log_line = Some(package_line(
                localizer,
                "wizard-progress-log-install-started",
                package_id,
            ));
        }
        ProgressEvent::InstallCompleted { package_id } => {
            log_line = Some(package_line(
                localizer,
                "wizard-progress-log-install-completed",
                package_id,
            ));
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
        // Each removal holds the status line the way an install does.
        ProgressEvent::RemoveStarted { package_id } => {
            status_line = Some(package_line(
                localizer,
                "wizard-progress-status-removing",
                package_id,
            ));
            log_line = Some(package_line(
                localizer,
                "wizard-progress-log-remove-started",
                package_id,
            ));
        }
        // The end of a removal stays on the status line until the next one
        // starts, and is spoken: whether the package really went is what
        // the user is waiting to hear.
        ProgressEvent::RemoveCompleted {
            package_id,
            removed,
        } => {
            let key = if *removed {
                "wizard-progress-log-remove-completed"
            } else {
                "wizard-progress-log-remove-failed"
            };
            let line = package_line(localizer, key, package_id);
            status_line = Some(line.clone());
            log_line = Some(line);
        }
    });

    widgets.progress_gauge.set_value(state.percentage());
    if let Some(line) = status_line
        && !status_frozen
    {
        widgets.progress_status.set_label(&line);
        if is_spoken(&event) {
            announce(&line, AnnouncePriority::Polite);
        }
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
