//! Running an uninstall from the wizard, apart from wxWidgets: the request
//! the Uninstall button hands the engine, what has to be closed before it
//! may start, what the progress page says first, and what the Done page
//! says afterwards.
//!
//! The engine, `rabbit_core::uninstall::execute_uninstall`, saves its own
//! report under `RABBIT/logs` for every run that got as far as the lock.
//! The Done page points to that file rather than writing a second one, which
//! within the same second would even overwrite it. Only a run that failed
//! before that point leaves nothing behind, so the wizard saves a report of
//! its own for it, except after the refusal to run as root, which must
//! leave nothing of root's in the user's resource folder.

use std::path::{Path, PathBuf};

use rabbit_core::localization::Localizer;
use rabbit_core::model::{InstallationKind, Platform};
use rabbit_core::package::PACKAGE_REAPER;
use rabbit_core::preflight::{
    PreflightCheck, PreflightReport, PreflightStatus, REAPER_PROCESS_CHECK,
};
use rabbit_core::report::{default_report_path, save_json_and_text_reports};
use rabbit_core::uninstall::{
    ANY_REAPER_PROCESS_CHECK, AdministratorStepKind, AdministratorStepVerdict, BACKUP_SPACE_CHECK,
    KeptFileReason, LeftoverResult, PROGRAMS_RUNNING_CHECK, RegistryEntryAction, Removability,
    RemovedFile, UninstallMessage, UninstallPlan, UninstallProblem, UninstallReport,
    UninstallReportItem, UninstallRequest, UninstallStatus,
};
use rabbit_core::{RabbitError, Result};
use serde::Serialize;

use super::model::{TargetRow, WizardModel};
use super::outcome::wizard_error_reason;
use super::packages::localized_package_display_name;
use super::uninstall::{WizardUninstallList, by_hand_lines, refusal_text};

/// What the engine is asked to do when the Uninstall button is pressed: the
/// ticked packages, from the target Review named, with "Also remove
/// settings" only while it applies. The target's kind and its REAPER come
/// from the target row, as they did for the rows, so the run and the list
/// agree about what can go.
///
/// Fails, like the install's request, for a target RABBIT can't write to and
/// for nothing ticked. Review keeps the button disabled in both cases, so
/// this is the last line of defence, not the way the user finds out.
pub fn uninstall_request_for_target(
    model: &WizardModel,
    target: &TargetRow,
    list: &WizardUninstallList,
    remove_settings: bool,
) -> Result<UninstallRequest> {
    if !target.writable {
        return Err(RabbitError::PreflightFailed {
            message: format!(
                "Target resource path is not writable: {}",
                target.path.display()
            ),
        });
    }
    let package_ids = list.ticked_package_ids();
    if package_ids.is_empty() {
        return Err(RabbitError::PreflightFailed {
            message: "No package was ticked to uninstall.".to_string(),
        });
    }
    Ok(UninstallRequest {
        resource_path: target.path.clone(),
        platform: model.platform,
        package_ids,
        // The checkbox is unticked whenever it stops applying, but a stale
        // tick must never take settings of a package that has none listed.
        remove_settings: remove_settings && list.settings_option_available(),
        dry_run: false,
        target_app_path: target.app_path.clone(),
        target: Some(if target.portable {
            InstallationKind::Portable
        } else {
            InstallationKind::Standard
        }),
    })
}

/// The message shown instead of starting an uninstall while something it
/// needs closed is running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallBlocker {
    pub title: String,
    pub body: String,
}

/// What the user has to close before uninstalling `package_ids`, read from
/// the process checks of the uninstall preflight, so the wizard and the
/// engine always agree. `running_programs` are the running programs the
/// plans name, such as Surge XT's; `None` when nothing is in the way.
///
/// Asked while the user is still on Review and can do something about it:
/// the engine refuses too, but only once the wizard has moved on to its
/// progress page, and there is no way back from there.
pub fn uninstall_blocker(
    localizer: &Localizer,
    plan: &UninstallPlan,
    package_ids: &[String],
    preflight: &PreflightReport,
    running_programs: &[String],
) -> Option<UninstallBlocker> {
    let failed = |name: &str| {
        preflight
            .checks
            .iter()
            .any(|check| check.name == name && check.status == PreflightStatus::Fail)
    };
    let ticked = || {
        plan.items
            .iter()
            .filter(|item| package_ids.contains(&item.package_id))
    };
    let mut lines = Vec::new();
    // A package every REAPER on the computer may load needs all of them
    // closed, which covers the target's REAPER too: one sentence, not two.
    let reaper = if failed(ANY_REAPER_PROCESS_CHECK) {
        // REAPER is never among the packages every REAPER can use. On
        // Windows its own uninstaller needs them all closed because it
        // doesn't run while any REAPER is open; on macOS the administrator
        // command that moves its app can't tell one REAPER from another.
        // Alone, each says so.
        let packages = ticked()
            .filter(|item| item.every_reaper_closed && item.package_id != PACKAGE_REAPER)
            .map(|item| localized_package_display_name(localizer, &item.package_id))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(if packages.is_empty() {
            localizer
                .text(if plan.platform == Platform::MacOs {
                    "wizard-reaper-running-uninstall-reaper-body-macos"
                } else {
                    "wizard-reaper-running-uninstall-reaper-body"
                })
                .value
        } else {
            localizer
                .format(
                    "wizard-reaper-running-uninstall-every-body",
                    &[("packages", packages.as_str())],
                )
                .value
        });
        true
    } else if failed(REAPER_PROCESS_CHECK) {
        lines.push(localizer.text("wizard-reaper-running-uninstall-body").value);
        true
    } else {
        false
    };
    if failed(PROGRAMS_RUNNING_CHECK) {
        // The programs found running; should they have closed in between,
        // every program the ticked packages name.
        let mut programs: Vec<String> = Vec::new();
        let named: Vec<String> = ticked()
            .flat_map(|item| item.blocking_programs.iter().cloned())
            .collect();
        let source = if running_programs.is_empty() {
            &named
        } else {
            running_programs
        };
        for program in source {
            if !programs.contains(program) {
                programs.push(program.clone());
            }
        }
        lines.push(
            localizer
                .format(
                    "wizard-programs-running-uninstall-body",
                    &[("programs", programs.join(", ").as_str())],
                )
                .value,
        );
    }
    if lines.is_empty() {
        return None;
    }
    let title = if reaper {
        "wizard-reaper-running-title"
    } else {
        "wizard-programs-running-uninstall-title"
    };
    Some(UninstallBlocker {
        title: localizer.text(title).value,
        body: lines.join("\n\n"),
    })
}

/// The progress log's first lines for an uninstall: that it is starting,
/// the target, and each ticked package with "Will uninstall", with the
/// settings that go too when "Also remove settings" is ticked.
pub fn uninstall_progress_details_for_start(
    localizer: &Localizer,
    target: &TargetRow,
    list: &WizardUninstallList,
    remove_settings: bool,
) -> String {
    let action = localizer.text("action-uninstall").value;
    let mut lines = vec![
        localizer
            .text("wizard-progress-details-uninstall-starting")
            .value,
        localizer
            .format(
                "wizard-review-target",
                &[("path", target.path.display().to_string().as_str())],
            )
            .value,
    ];
    let remove_settings = remove_settings && list.settings_option_available();
    for row in list.ticked_rows() {
        lines.push(
            localizer
                .format(
                    "wizard-summary-package-message",
                    &[
                        ("package", row.display_name.as_str()),
                        ("message", action.as_str()),
                    ],
                )
                .value,
        );
        if remove_settings && !row.settings.is_empty() {
            lines.push(
                localizer
                    .format(
                        "wizard-review-uninstall-settings-removed",
                        &[("settings", row.settings.join(", ").as_str())],
                    )
                    .value,
            );
        }
    }
    lines.join("\n")
}

/// How an uninstall went, as the Done page sums it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UninstallOutcomeStatus {
    /// Everything ticked is gone, with nothing left to look at.
    Success,
    /// Something was uninstalled, but a package failed or was left alone,
    /// something of a removed one is still there, or a step after a removal
    /// didn't work.
    CompletedWithProblems,
    /// Nothing was uninstalled, because a removal failed or an error
    /// stopped the run before it began.
    Failed,
    /// The user stopped the run. Packages removed before that stay removed.
    Cancelled,
    /// Nothing was uninstalled and nothing failed: a check stopped the run
    /// before anything changed, or nothing ticked was installed any more.
    NothingDone,
    /// Everything ticked is gone, but some of it only once the computer
    /// restarts.
    PendingRestart,
    /// The administrator permission prompt was declined, Windows' or
    /// macOS's password window, so nothing was uninstalled at all.
    PermissionDeclined,
    /// No package was uninstalled, but the macOS administrator command
    /// removed part of one before it stopped. What it removed can't be put
    /// back, so "nothing was uninstalled" would hide it.
    PartlyRemoved,
}

/// Part of the package is gone and part is still there: the macOS
/// administrator command stopped in the middle of it. Its status is Failed,
/// but unlike any other failure, nothing was put back.
pub fn partly_removed(item: &UninstallReportItem) -> bool {
    matches!(
        &item.message_code,
        UninstallMessage::AdministratorStepFailed { gone, .. } if !gone.is_empty()
    )
}

/// The status of a run that returned a report. A declined permission prompt
/// comes first: it is why nothing happened at all. Then a stop outranks a
/// problem, as for an install: "you stopped it" explains a half-finished
/// run, where "finished with problems" would send the user looking for a
/// fault. A package removed only in part is never "nothing uninstalled".
pub fn uninstall_outcome_status(report: &UninstallReport) -> UninstallOutcomeStatus {
    if report.elevation_declined() {
        UninstallOutcomeStatus::PermissionDeclined
    } else if report.was_cancelled() {
        UninstallOutcomeStatus::Cancelled
    } else if report.changed_anything() {
        if report.has_failures() || report.finished_with_problems() {
            UninstallOutcomeStatus::CompletedWithProblems
        } else if report.needs_restart() {
            UninstallOutcomeStatus::PendingRestart
        } else {
            UninstallOutcomeStatus::Success
        }
    } else if report.items.iter().any(partly_removed) {
        UninstallOutcomeStatus::PartlyRemoved
    } else if report
        .items
        .iter()
        .any(|item| item.status == UninstallStatus::Failed)
    {
        UninstallOutcomeStatus::Failed
    } else {
        UninstallOutcomeStatus::NothingDone
    }
}

/// What the Done page shows after an uninstall, and what a run that failed
/// before the engine could save its report saves instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WizardUninstallOutcome {
    pub status: UninstallOutcomeStatus,
    pub resource_path: PathBuf,
    pub package_ids: Vec<String>,
    pub remove_settings: bool,
    /// One sentence: the progress page's last status line, and the first
    /// line of the Done page.
    pub heading: String,
    /// The Done page's status text, which gets focus and is read: the
    /// heading, why anything stayed, and how to get a package back.
    pub status_text: String,
    /// What "Show details" shows.
    pub detail_lines: Vec<String>,
    /// The engine's error, in English, when it stopped before the run.
    pub error_message: Option<String>,
    /// The saved report the details point to: the engine's, or for a run
    /// that stopped before the engine could save one, the wizard's own.
    pub report_path: Option<PathBuf>,
}

/// The Done page's text for a run of `package_ids` that returned `report`.
pub fn wizard_uninstall_outcome(
    localizer: &Localizer,
    package_ids: &[String],
    report: &UninstallReport,
) -> WizardUninstallOutcome {
    let status = uninstall_outcome_status(report);
    let removed = report
        .items
        .iter()
        .filter(|item| item.status.is_removed())
        .map(|item| package_name(localizer, item))
        .collect::<Vec<_>>()
        .join(", ");
    let format = |key: &str| {
        localizer
            .format(key, &[("packages", removed.as_str())])
            .value
    };
    let heading = match status {
        UninstallOutcomeStatus::Success => format("wizard-done-uninstall-status-success"),
        UninstallOutcomeStatus::CompletedWithProblems => {
            format("wizard-done-uninstall-status-completed-with-errors")
        }
        UninstallOutcomeStatus::Failed => format("wizard-done-uninstall-status-error"),
        UninstallOutcomeStatus::Cancelled => format("wizard-done-uninstall-status-cancelled"),
        UninstallOutcomeStatus::NothingDone => format("wizard-done-uninstall-status-no-packages"),
        UninstallOutcomeStatus::PendingRestart => {
            format("wizard-done-uninstall-status-pending-restart")
        }
        // macOS's window asks for a password, and is cancelled rather than
        // declined; Windows' asks for permission.
        UninstallOutcomeStatus::PermissionDeclined if report.platform == Platform::MacOs => {
            format("wizard-done-uninstall-status-declined-macos")
        }
        UninstallOutcomeStatus::PermissionDeclined => {
            format("wizard-done-uninstall-status-declined")
        }
        UninstallOutcomeStatus::PartlyRemoved => {
            let partly = report
                .items
                .iter()
                .filter(|item| partly_removed(item))
                .map(|item| package_name(localizer, item))
                .collect::<Vec<_>>()
                .join(", ");
            localizer
                .format(
                    "wizard-done-uninstall-status-partly-removed",
                    &[("packages", partly.as_str())],
                )
                .value
        }
    };
    let mut blocks = vec![heading.clone()];
    let mut reasons = uninstall_reasons(localizer, report);
    // The heading already says to restart; read right after it, the same
    // instruction per package only repeats it. Show details keeps it.
    if status == UninstallOutcomeStatus::PendingRestart {
        let restart_lines: Vec<String> = report
            .items
            .iter()
            .filter(|item| item.status == UninstallStatus::RemovedPendingRestart)
            .map(|item| {
                localizer
                    .format(
                        "wizard-done-uninstall-pending-restart",
                        &[("package", package_name(localizer, item).as_str())],
                    )
                    .value
            })
            .collect();
        reasons.retain(|reason| !restart_lines.contains(reason));
    }
    if !reasons.is_empty() {
        blocks.push(reasons.join("\n"));
    }
    if report.changed_anything() || status == UninstallOutcomeStatus::PartlyRemoved {
        if status == UninstallOutcomeStatus::Cancelled {
            blocks.push(format("wizard-done-uninstall-removed-before-stop"));
        }
        // There is no way back to the package list from Done, so the way to
        // undo a removal, whole or in part, is a new run.
        blocks.push(localizer.text("wizard-done-uninstall-reinstall-hint").value);
    }
    WizardUninstallOutcome {
        status,
        resource_path: report.resource_path.clone(),
        package_ids: package_ids.to_vec(),
        remove_settings: report.remove_settings,
        heading,
        status_text: blocks.join("\n\n"),
        detail_lines: summarize_uninstall_report(localizer, report),
        error_message: None,
        report_path: report
            .saved_report
            .as_ref()
            .map(|saved| saved.text_path.clone()),
    }
}

/// The Done page's text for a run that failed before it started: the
/// engine refused it, such as while another RABBIT holds the lock, or the
/// wizard couldn't even put the request together. Nothing changed.
pub fn wizard_uninstall_outcome_from_error(
    model: &WizardModel,
    localizer: &Localizer,
    resource_path: &Path,
    package_ids: &[String],
    remove_settings: bool,
    error: &RabbitError,
) -> WizardUninstallOutcome {
    let heading = localizer.text("wizard-done-uninstall-status-error").value;
    let packages = package_ids
        .iter()
        .map(|id| localized_package_display_name(localizer, id))
        .collect::<Vec<_>>()
        .join(", ");
    let detail_lines = vec![
        target_line(localizer, resource_path),
        localizer
            .format(
                "wizard-summary-packages-selected",
                &[("packages", packages.as_str())],
            )
            .value,
        localizer
            .format(
                "wizard-summary-error",
                &[("message", error.to_string().as_str())],
            )
            .value,
    ];
    WizardUninstallOutcome {
        status: UninstallOutcomeStatus::Failed,
        resource_path: resource_path.to_path_buf(),
        package_ids: package_ids.to_vec(),
        remove_settings,
        status_text: format!("{heading}\n\n{}", wizard_error_reason(model, error)),
        heading,
        detail_lines,
        error_message: Some(error.to_string()),
        report_path: None,
    }
}

/// Save `outcome` as `RABBIT/logs/uninstall-<time>.json` and `.txt` when
/// the run left no report of the engine's, such as one that stopped before
/// the lock, and add where it went to the details. Writes nothing into a
/// resource folder that doesn't exist, as the engine doesn't either: there
/// was nothing to remove from it.
pub fn save_wizard_uninstall_outcome(
    localizer: &Localizer,
    outcome: &mut WizardUninstallOutcome,
) -> Result<()> {
    if outcome.report_path.is_some() || !outcome.resource_path.is_dir() {
        return Ok(());
    }
    let saved = save_json_and_text_reports(
        &default_report_path(&outcome.resource_path, "uninstall"),
        outcome,
    )?;
    outcome.detail_lines.push(
        localizer
            .format(
                "wizard-summary-uninstall-report",
                &[("path", saved.text_path.display().to_string().as_str())],
            )
            .value,
    );
    outcome.report_path = Some(saved.text_path);
    Ok(())
}

/// Whether Done may save a report of its own for a run that ended in
/// `error`. Not after the refusal to run as root, as under sudo: the files,
/// and any RABBIT or logs folder made for them, would be root's, in the
/// user's resource folder. The engine refuses first so that it leaves none.
pub fn saves_report_after_error(error: &RabbitError) -> bool {
    !matches!(error, RabbitError::UninstallAsRoot)
}

/// Why packages stayed, or what to look at after a removal, one sentence
/// each and each said once, in the user's language where RABBIT knows the
/// reason. Packages that only stayed because an earlier one stopped the run
/// add nothing: the sentence for that earlier one says what to do.
pub fn uninstall_reasons(localizer: &Localizer, report: &UninstallReport) -> Vec<String> {
    let mut reasons: Vec<String> = Vec::new();
    for item in &report.items {
        for reason in item_reasons(localizer, report, item) {
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
    }
    reasons
}

/// The programs the macOS administrator command looked for first, when that
/// first step didn't come back with "nothing found": one of them was
/// running, or pgrep couldn't check. Either way the command changed
/// nothing. `None` for any other run.
fn programs_check_failed(report: &UninstallReport) -> Option<&str> {
    report
        .administrator_steps
        .iter()
        .find(|step| {
            step.kind == AdministratorStepKind::ProgramsClosed
                && step.verdict == AdministratorStepVerdict::Failed
        })
        .map(|step| step.target.as_str())
}

/// The administrator command moved the package's app into the Trash, rather
/// than RABBIT as the user: the app keeps the owner it had there.
fn moved_by_administrator(report: &UninstallReport, item: &UninstallReportItem) -> bool {
    report.administrator_steps.iter().any(|step| {
        step.kind == AdministratorStepKind::MoveToTrash
            && step.package_id.as_deref() == Some(item.package_id.as_str())
    })
}

fn item_reasons(
    localizer: &Localizer,
    report: &UninstallReport,
    item: &UninstallReportItem,
) -> Vec<String> {
    let package = package_name(localizer, item);
    let with_package = |key: &str| {
        localizer
            .format(key, &[("package", package.as_str())])
            .value
    };
    let with = |key: &str, name: &str, value: &str| {
        localizer
            .format(key, &[("package", package.as_str()), (name, value)])
            .value
    };
    let app_management = || localizer.text("wizard-done-uninstall-app-management").value;
    let mut reasons = Vec::new();
    match &item.message_code {
        UninstallMessage::Removed { .. }
        | UninstallMessage::WouldRemove
        | UninstallMessage::UninstalledByVendor
        | UninstallMessage::RemovedAsAdministrator => {}
        // Where it is in the Trash, when RABBIT saw it there, and that it
        // can come back from there. Never a place RABBIT didn't see.
        UninstallMessage::MovedToTrash { trashed_to } => {
            reasons.push(match trashed_to {
                Some(path) => with(
                    "wizard-done-uninstall-moved-to-trash",
                    "path",
                    &path.display().to_string(),
                ),
                None => with_package("wizard-done-uninstall-moved-to-trash-unknown"),
            });
            if moved_by_administrator(report, item) {
                reasons.push(localizer.text("wizard-done-uninstall-trash-owner").value);
            }
        }
        // Done's first line says the prompt was declined, for every package.
        UninstallMessage::ElevationDeclined => {}
        UninstallMessage::VendorRefused { refusal } => {
            reasons.push(refusal_text(localizer, refusal, &package));
            reasons.push(with_package("uninstall-vendor-refused-nothing-changed"));
        }
        UninstallMessage::VendorCancelled => {
            reasons.push(with_package("wizard-done-uninstall-vendor-cancelled"));
        }
        // Show details is only worth naming when it lists something.
        UninstallMessage::VendorFailed { exit_code } => reasons.push(with(
            if vendor_still_present(item).is_empty() {
                "wizard-done-uninstall-vendor-failed-unsure"
            } else {
                "wizard-done-uninstall-vendor-failed"
            },
            "code",
            &exit_code.to_string(),
        )),
        // Windows Settings only helps while Windows still lists the
        // package; once its entry is gone, sending the user there leads
        // nowhere.
        UninstallMessage::VendorStillThere { still_listed, .. } => {
            let left = if vendor_still_present(item).is_empty() {
                "wizard-done-uninstall-vendor-not-confirmed"
            } else {
                "wizard-done-uninstall-vendor-files-left"
            };
            match still_listed {
                Some(true) => {
                    reasons.push(with_package("wizard-done-uninstall-vendor-still-there"));
                }
                Some(false) => {
                    reasons.push(with_package(left));
                    reasons.push(with_package("wizard-done-uninstall-vendor-not-listed"));
                }
                // A problem below says the list couldn't be read.
                None => reasons.push(with_package(left)),
            }
        }
        UninstallMessage::VendorStillOpen { .. } => {
            reasons.push(with_package("wizard-done-uninstall-vendor-still-open"));
        }
        UninstallMessage::VendorRunFailed { error } => reasons.push(with(
            "wizard-done-uninstall-vendor-run-failed",
            "error",
            error,
        )),
        UninstallMessage::VendorLost { .. } => {
            reasons.push(with_package("wizard-done-uninstall-vendor-lost"));
            // Only REAPER's uninstaller works next to REAPER's settings.
            if item.package_id == PACKAGE_REAPER {
                reasons.push(
                    localizer
                        .text("wizard-done-uninstall-vendor-lost-reaper")
                        .value,
                );
            }
        }
        // Refused before any password window: the administrator command
        // couldn't be put together, so macOS was never asked.
        UninstallMessage::ElevationFailed { error }
            if report.platform == Platform::MacOs && item.status == UninstallStatus::Blocked =>
        {
            reasons.push(with(
                "wizard-done-uninstall-admin-steps-refused",
                "error",
                error,
            ));
        }
        UninstallMessage::ElevationFailed { error } => reasons.push(with(
            "wizard-done-uninstall-elevation-failed",
            "error",
            error,
        )),
        UninstallMessage::ResourceFolderDamaged { missing } => {
            let paths = missing
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            reasons.push(with(
                "wizard-done-uninstall-resource-damaged",
                "paths",
                &paths,
            ));
        }
        UninstallMessage::NotInstalled => {
            reasons.push(with_package("wizard-done-uninstall-not-installed"));
        }
        // The heading says the run was stopped, and the package that
        // stopped it says why.
        UninstallMessage::Cancelled
        | UninstallMessage::RunStopped { .. }
        | UninstallMessage::OtherPackageBlocked { .. } => {}
        UninstallMessage::ChecksFailed { checks } => {
            match programs_check_failed(report) {
                // The administrator command's first step didn't come back
                // with "nothing found": one of the programs it looks for
                // was running, or pgrep couldn't check. Said once, with
                // every name root looked for, rather than as a check that
                // ran before anything started.
                Some(programs) => reasons.push(
                    localizer
                        .format(
                            "wizard-done-uninstall-program-started",
                            &[("programs", programs)],
                        )
                        .value,
                ),
                None => {
                    for check in checks
                        .iter()
                        .filter(|check| check.status == PreflightStatus::Fail)
                    {
                        reasons.push(check_reason(localizer, check));
                    }
                }
            }
        }
        UninstallMessage::FileInUse { path, .. } => reasons.push(
            localizer
                .format(
                    "wizard-error-uninstall-file-in-use",
                    &[
                        ("package", package.as_str()),
                        ("path", path.display().to_string().as_str()),
                    ],
                )
                .value,
        ),
        // A macOS app that changed between Review and the run, so that
        // only the user can move it now: why, and the Finder steps, as the
        // row says them.
        UninstallMessage::NotRemovable {
            removability: Removability::ByHand { path, reason },
        } => reasons.extend(by_hand_lines(localizer, path, reason, &package)),
        // Rare enough that the engine's English sentence, which names the
        // file and the error, says it best.
        UninstallMessage::UnknownPackage
        | UninstallMessage::NotRemovable { .. }
        | UninstallMessage::NotYetSupported
        | UninstallMessage::PathRefused { .. }
        | UninstallMessage::Unreadable { .. }
        | UninstallMessage::MoveFailed { .. }
        | UninstallMessage::BackupFailed { .. }
        | UninstallMessage::ReceiptSaveFailed { .. } => reasons.push(item.message.clone()),
        // App Management lets macOS refuse a move, and later a deletion,
        // even to root; turning RABBIT on there is the way past it.
        UninstallMessage::TrashFailed {
            error,
            refused_by_macos,
        } => {
            reasons.push(with("wizard-done-uninstall-trash-failed", "error", error));
            if *refused_by_macos {
                reasons.push(app_management());
            }
        }
        UninstallMessage::TrashUnclear { path } => reasons.push(with(
            "wizard-done-uninstall-trash-unclear",
            "path",
            &path.display().to_string(),
        )),
        // Never "put back": what root removed stays removed. Show details
        // lists what is gone and what is left. Where the command stopped
        // only when its step was this package's, and "without saying" only
        // when the command didn't say: otherwise it stopped at another
        // package's step, or ran to its end, and this is still there.
        UninstallMessage::AdministratorStepFailed {
            path,
            ended_unclearly,
            gone,
            left,
            refused_by_macos,
            ..
        } => {
            let (key, shown) = match (path, *ended_unclearly, gone.is_empty()) {
                (Some(path), _, true) => ("wizard-done-uninstall-admin-step-failed", Some(path)),
                (Some(path), _, false) => ("wizard-done-uninstall-partly-removed", Some(path)),
                (None, true, true) => ("wizard-done-uninstall-admin-stopped", None),
                (None, true, false) => ("wizard-done-uninstall-partly-removed-unclear", None),
                (None, false, _) => ("wizard-done-uninstall-admin-left", left.first()),
            };
            let path = shown
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            reasons.push(with(key, "path", &path));
            if *refused_by_macos {
                reasons.push(app_management());
            }
        }
    }
    if item.status == UninstallStatus::RemovedPendingRestart {
        reasons.push(with_package("wizard-done-uninstall-pending-restart"));
    }
    if !item.left_in_backup.is_empty() {
        reasons.push(with_package("wizard-done-uninstall-left-in-backup"));
    }
    if !item.leftovers.is_empty() {
        reasons.push(with_package("wizard-done-uninstall-leftovers"));
    }
    for problem in &item.problems {
        match problem {
            UninstallProblem::LanguageResetFailed { .. } => {
                reasons.push(with_package("wizard-done-uninstall-language-reset-failed"));
            }
            UninstallProblem::RegistryEntryUnreadable { .. }
            | UninstallProblem::RegistryEntryNotRemoved { .. } => {
                reasons.push(with_package("wizard-done-uninstall-registry-failed"));
            }
            UninstallProblem::ReceiptNotSaved { .. } => {
                reasons.push(with_package("wizard-done-uninstall-receipt-not-saved"));
            }
            UninstallProblem::VendorWorkingFolderLeft { path, .. } => reasons.push(with(
                "wizard-done-uninstall-working-folder-left",
                "path",
                &path.display().to_string(),
            )),
            UninstallProblem::RestartListUnreadable { .. } => {
                reasons.push(with_package(
                    "wizard-done-uninstall-restart-list-unreadable",
                ));
            }
            UninstallProblem::InstalledListUnreadable { .. } => {
                reasons.push(with_package(
                    "wizard-done-uninstall-installed-list-unreadable",
                ));
            }
            // Show details names the receipts, as it names a registry entry.
            UninstallProblem::InstallerRecordsRemain { .. } => {
                reasons.push(with_package(
                    "wizard-done-uninstall-installer-records-remain",
                ));
            }
            UninstallProblem::InstallerRecordsUnchecked { error } => reasons.push(with(
                "wizard-done-uninstall-installer-records-unchecked",
                "error",
                error,
            )),
            UninstallProblem::AdministratorEndedUnclearly { .. } => {
                reasons.push(with_package("wizard-done-uninstall-admin-unclear"));
            }
        }
    }
    reasons
}

/// Why a check stopped a package. A running REAPER is the common case and
/// gets the same sentence an install gets; the other checks the engine words
/// in English, naming the programs or the drive.
fn check_reason(localizer: &Localizer, check: &PreflightCheck) -> String {
    let key = match check.name.as_str() {
        REAPER_PROCESS_CHECK => "wizard-error-reaper-running",
        ANY_REAPER_PROCESS_CHECK => "wizard-error-uninstall-any-reaper-running",
        BACKUP_SPACE_CHECK => "wizard-error-uninstall-not-enough-space",
        _ => return check.message.clone(),
    };
    localizer.text(key).value
}

/// What RABBIT still found of a package after its own uninstaller ran and
/// didn't remove it. Only the engine's message and the vendor run know
/// this: the package's leftovers list only what a removal left behind.
fn vendor_still_present(item: &UninstallReportItem) -> &[PathBuf] {
    match &item.message_code {
        UninstallMessage::VendorStillThere { still_present, .. } => still_present,
        UninstallMessage::VendorFailed { .. } => item
            .vendor
            .as_ref()
            .map_or(&[], |vendor| vendor.still_present.as_slice()),
        _ => &[],
    }
}

/// "Show details" after an uninstall: the target, then for each package
/// what became of it, the files it removed (into the backup set, to the
/// Trash and where it is there, or with administrator rights), kept and
/// left behind, its entries in Windows' list of installed programs, the
/// installer receipts macOS still lists, and why it stayed; then the backup
/// set with a command for each package it holds, and where the report is.
pub fn summarize_uninstall_report(localizer: &Localizer, report: &UninstallReport) -> Vec<String> {
    let path_line = |key: &str, path: &Path| {
        localizer
            .format(key, &[("path", path.display().to_string().as_str())])
            .value
    };
    // A package's files sit indented below its line. The indent isn't in
    // the messages: Fluent drops blanks at the start of a value.
    let file_line = |key: &str, path: &Path| format!("  {}", path_line(key, path));
    let yes_no = |value: bool| {
        localizer
            .text(if value { "common-yes" } else { "common-no" })
            .value
    };
    let mut lines = vec![
        target_line(localizer, &report.resource_path),
        localizer
            .format(
                "wizard-summary-uninstall-remove-settings",
                &[("value", yes_no(report.remove_settings).as_str())],
            )
            .value,
    ];
    for item in &report.items {
        let package = package_name(localizer, item);
        let status = uninstall_status_label(localizer, item.status);
        lines.push(
            localizer
                .format(
                    "wizard-summary-package-message",
                    &[("package", package.as_str()), ("message", status.as_str())],
                )
                .value,
        );
        for file in &item.removed {
            lines.push(file_line(removed_file_key(item, file), &file.path));
            // Only a place RABBIT saw: macOS named it, or RABBIT found it.
            if let Some(trashed_to) = &file.trashed_to {
                lines.push(file_line("wizard-summary-in-trash-as", trashed_to));
            }
        }
        lines.extend(
            item.removed_dirs
                .iter()
                .map(|dir| file_line("wizard-summary-removed-file", dir)),
        );
        // The administrator command stopped part of the way through: what
        // it removed, what is still there, and macOS's own words on why.
        if let UninstallMessage::AdministratorStepFailed {
            gone, left, detail, ..
        } = &item.message_code
        {
            lines.extend(
                gone.iter()
                    .map(|path| file_line("wizard-summary-removed-as-admin", path)),
            );
            lines.extend(
                left.iter()
                    .map(|path| file_line("wizard-summary-left-over", path)),
            );
            if let Some(detail) = detail {
                lines.push(format!(
                    "  {}",
                    localizer
                        .format("wizard-summary-macos-said", &[("error", detail.as_str())])
                        .value
                ));
            }
        }
        // A vendor's uninstaller: which one ran, the links and empty folders
        // RABBIT removed after it, and what goes at the next restart.
        if let Some(vendor) = &item.vendor {
            lines.extend(
                vendor
                    .uninstaller
                    .iter()
                    .map(|path| file_line("wizard-summary-vendor-uninstaller", path)),
            );
            lines.extend(
                vendor
                    .removed_leftovers
                    .iter()
                    .filter(|leftover| leftover.result == LeftoverResult::Removed)
                    .map(|leftover| file_line("wizard-summary-removed-file", &leftover.path)),
            );
        }
        lines.extend(
            item.pending_restart
                .iter()
                .map(|path| file_line("wizard-summary-pending-restart", path)),
        );
        lines.extend(item.kept.iter().map(|kept| {
            let key = match kept.reason {
                KeptFileReason::ChangedSinceInstall => "wizard-summary-kept-file",
                KeptFileReason::HoldsLink => "wizard-summary-kept-link",
                KeptFileReason::InstalledByReaPack => "wizard-summary-kept-reapack",
            };
            file_line(key, &kept.path)
        }));
        // What a vendor's uninstaller didn't remove, which Done points to,
        // then what a removal left; a path in both is said once.
        let still_present = vendor_still_present(item);
        lines.extend(
            still_present
                .iter()
                .chain(
                    item.leftovers
                        .iter()
                        .filter(|path| !still_present.contains(path)),
                )
                .map(|path| file_line("wizard-summary-left-over", path)),
        );
        lines.extend(
            item.left_in_backup
                .iter()
                .map(|path| file_line("wizard-summary-left-in-backup", path)),
        );
        lines.extend(registry_lines(localizer, item));
        lines.extend(item.problems.iter().filter_map(|problem| match problem {
            UninstallProblem::InstallerRecordsRemain { ids } => Some(format!(
                "  {}",
                localizer
                    .format(
                        "wizard-summary-installer-records-remain",
                        &[("ids", ids.join(", ").as_str())],
                    )
                    .value
            )),
            _ => None,
        }));
        lines.extend(
            item_reasons(localizer, report, item)
                .into_iter()
                .map(|reason| format!("  {reason}")),
        );
    }
    if let Some(set_id) = &report.backup_set_id {
        lines.push(
            localizer
                .format(
                    "wizard-summary-uninstall-backup",
                    &[("id", set_id.as_str())],
                )
                .value,
        );
        // Only a package whose files went into the backup set can come back
        // from it. What a vendor's uninstaller or the macOS administrator
        // command removed, or what went to the Trash, isn't in the set,
        // even when another package of the same run is.
        for item in report.items.iter().filter(|item| {
            item.status.is_removed() && item.removed.iter().any(|file| file.backup_path.is_some())
        }) {
            let command = format!(
                "rabbit restore-backup --resource-path \"{}\" --backup-id {set_id} --package {} \
                 --apply",
                report.resource_path.display(),
                item.package_id
            );
            lines.push(
                localizer
                    .format(
                        "wizard-summary-uninstall-restore",
                        &[
                            ("package", package_name(localizer, item).as_str()),
                            ("command", command.as_str()),
                        ],
                    )
                    .value,
            );
        }
    }
    if let Some(saved) = &report.saved_report {
        lines.push(path_line(
            "wizard-summary-uninstall-report",
            &saved.text_path,
        ));
    }
    lines
}

/// How Show details names one path the run removed: moved into the backup
/// set, to the Trash, or deleted by the macOS administrator command. A
/// setting that went into the backup set beside an app in the Trash is
/// still a plain removal.
fn removed_file_key(item: &UninstallReportItem, file: &RemovedFile) -> &'static str {
    if file.backup_path.is_some() {
        return "wizard-summary-removed-file";
    }
    match item.message_code {
        UninstallMessage::MovedToTrash { .. } => "wizard-summary-moved-to-trash",
        UninstallMessage::RemovedAsAdministrator => "wizard-summary-removed-as-admin",
        _ => "wizard-summary-removed-file",
    }
}

/// What became of a package's entries in Windows' list of installed
/// programs, one indented line each, naming the entry's key: the Done
/// page's sentence about one that couldn't go doesn't, and the user needs
/// it to find the entry. An entry that wasn't there says nothing.
fn registry_lines(localizer: &Localizer, item: &UninstallReportItem) -> Vec<String> {
    let line =
        |key: &str, args: &[(&str, &str)]| format!("  {}", localizer.format(key, args).value);
    let mut lines: Vec<String> = item
        .registry_entries
        .iter()
        .filter_map(|entry| {
            let message = match entry.action {
                RegistryEntryAction::Removed => "wizard-summary-registry-removed",
                RegistryEntryAction::PointsElsewhere { .. } => "wizard-summary-registry-kept",
                // The wizard never runs a dry run.
                RegistryEntryAction::WouldRemove | RegistryEntryAction::NotFound => return None,
            };
            Some(line(message, &[("key", entry.key.as_str())]))
        })
        .collect();
    lines.extend(item.problems.iter().filter_map(|problem| match problem {
        UninstallProblem::RegistryEntryUnreadable { key, error }
        | UninstallProblem::RegistryEntryNotRemoved { key, error } => Some(line(
            "wizard-summary-registry-failed",
            &[("key", key.as_str()), ("error", error.as_str())],
        )),
        UninstallProblem::LanguageResetFailed { .. }
        | UninstallProblem::ReceiptNotSaved { .. }
        | UninstallProblem::VendorWorkingFolderLeft { .. }
        | UninstallProblem::RestartListUnreadable { .. }
        | UninstallProblem::InstalledListUnreadable { .. }
        | UninstallProblem::InstallerRecordsRemain { .. }
        | UninstallProblem::InstallerRecordsUnchecked { .. }
        | UninstallProblem::AdministratorEndedUnclearly { .. } => None,
    }));
    lines
}

/// What became of a package, in a few words.
pub fn uninstall_status_label(localizer: &Localizer, status: UninstallStatus) -> String {
    let key = match status {
        UninstallStatus::Removed => "status-removed",
        UninstallStatus::RemovedWithLeftovers => "status-removed-with-leftovers",
        UninstallStatus::RemovedPendingRestart => "status-removed-pending-restart",
        UninstallStatus::WouldRemove => "status-would-remove",
        UninstallStatus::NotInstalled => "status-remove-not-installed",
        UninstallStatus::Blocked => "status-remove-blocked",
        UninstallStatus::Failed => "status-remove-failed",
        UninstallStatus::Cancelled => "status-remove-cancelled",
        UninstallStatus::NotReached => "status-remove-not-reached",
    };
    localizer.text(key).value
}

fn target_line(localizer: &Localizer, path: &Path) -> String {
    localizer
        .format(
            "wizard-summary-target",
            &[("path", path.display().to_string().as_str())],
        )
        .value
}

/// The package's name in the user's language, or the engine's when the
/// locale has none for it.
fn package_name(localizer: &Localizer, item: &UninstallReportItem) -> String {
    let name = localized_package_display_name(localizer, &item.package_id);
    if name == item.package_id {
        item.display_name.clone()
    } else {
        name
    }
}
