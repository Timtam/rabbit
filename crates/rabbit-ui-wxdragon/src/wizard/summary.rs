//! Renders a finished [`SetupReport`] into the done page's summary text.

use std::path::Path;

use rabbit_core::install::{
    InstallFileAction, InstallFileReport, LeftInPlaceReason, ReaperLanguageReport,
};
use rabbit_core::localization::{FluentArgument, Localizer};
use rabbit_core::operation::{PackageOperationStatus, PlannedExecutionKind, PlannedExecutionPlan};
use rabbit_core::resource::ResourceInitActionKind;
use rabbit_core::setup::SetupReport;

use super::bootstrap::localizer_from_options;
use super::labels::{
    action_label_for_summary, architecture_label_for_summary,
    configuration_status_label_for_summary, format_localized_message,
    localized_configuration_message, localized_configuration_step_name,
    localized_package_operation_message, planned_execution_runner_label, status_label_for_summary,
};
use super::model::{WizardInstallSummary, WizardModel};
use super::packages::package_display_name;

pub fn summarize_setup_report(model: &WizardModel, report: &SetupReport) -> WizardInstallSummary {
    let localizer = localizer_from_options(&model.bootstrap_options).ok();
    let created_resources = report
        .resource_init
        .actions
        .iter()
        .filter(|action| action.action == ResourceInitActionKind::Created)
        .count();
    let installed_or_checked = report
        .package_operation
        .items
        .iter()
        .filter(|item| item.status == PackageOperationStatus::InstalledOrChecked)
        .count();
    let skipped_current = report
        .package_operation
        .items
        .iter()
        .filter(|item| item.status == PackageOperationStatus::SkippedCurrent)
        .count();
    let manual_items = report
        .package_operation
        .items
        .iter()
        .filter(|item| matches!(item.status, PackageOperationStatus::DeferredUnattended))
        .count();
    let failed_items = report
        .package_operation
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.status,
                PackageOperationStatus::Failed | PackageOperationStatus::SkippedDependencyFailed
            )
        })
        .count();
    let any_antivirus_block = report.package_operation.items.iter().any(|item| {
        matches!(
            item.message_code,
            rabbit_core::operation::PackageOperationMessage::InstallFailed {
                antivirus_block: true,
                ..
            }
        )
    });

    let architecture_label = architecture_label_for_summary(model.architecture);
    let mut detail_lines = vec![
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-target",
            &[("path", report.resource_path.display().to_string())],
            format!("Target: {}", report.resource_path.display()),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-architecture",
            &[("architecture", architecture_label.clone())],
            format!("Architecture: {architecture_label}"),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-dry-run",
            &[(
                "value",
                if report.dry_run {
                    model.text.common_yes.clone()
                } else {
                    model.text.common_no.clone()
                },
            )],
            format!(
                "Dry run: {}",
                if report.dry_run {
                    &model.text.common_yes
                } else {
                    &model.text.common_no
                }
            ),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-resource-items-created",
            &[("count", created_resources.to_string())],
            format!("Resource items created: {created_resources}"),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-packages-installed-or-checked",
            &[("count", installed_or_checked.to_string())],
            format!("Packages installed or checked: {installed_or_checked}"),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-packages-current",
            &[("count", skipped_current.to_string())],
            format!("Packages already current: {skipped_current}"),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-packages-manual",
            &[("count", manual_items.to_string())],
            format!("Packages requiring manual attention: {manual_items}"),
        ),
    ];

    if let Some(install_report) = &report.package_operation.install_report {
        let backup_paths = install_report
            .actions
            .iter()
            .filter_map(|action| action.backup_path.as_ref())
            .collect::<Vec<_>>();
        if !backup_paths.is_empty() || install_report.backup_manifest_path.is_some() {
            detail_lines.push(format_localized_message(
                localizer.as_ref(),
                "wizard-summary-backup-files-created",
                &[("count", backup_paths.len().to_string())],
                format!("Backup files created: {}", backup_paths.len()),
            ));
            for path in backup_paths {
                detail_lines.push(format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-backup-file",
                    &[("path", path.display().to_string())],
                    format!("Backup file: {}", path.display()),
                ));
            }
            if let Some(path) = &install_report.backup_manifest_path {
                detail_lines.push(format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-backup-manifest",
                    &[("path", path.display().to_string())],
                    format!("Backup manifest: {}", path.display()),
                ));
            }
        }
        // A file of a package the run has no item for, which it always has
        // for a package it installed, still gets its line, here.
        detail_lines.extend(
            install_report
                .actions
                .iter()
                .filter(|action| {
                    !report
                        .package_operation
                        .items
                        .iter()
                        .any(|item| item.package_id == action.package_id)
                })
                .flat_map(|action| {
                    superseded_file_lines(localizer.as_ref(), &install_report.resource_path, action)
                }),
        );
    }

    let item_backup_paths = report
        .package_operation
        .items
        .iter()
        .flat_map(|item| item.backup_paths.iter())
        .collect::<Vec<_>>();
    let item_backup_manifest_paths = report
        .package_operation
        .items
        .iter()
        .filter_map(|item| item.backup_manifest_path.as_ref())
        .collect::<Vec<_>>();
    if report.package_operation.install_report.is_none()
        && (!item_backup_paths.is_empty() || !item_backup_manifest_paths.is_empty())
    {
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-backup-files-created",
            &[("count", item_backup_paths.len().to_string())],
            format!("Backup files created: {}", item_backup_paths.len()),
        ));
    }
    for path in item_backup_paths {
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-backup-file",
            &[("path", path.display().to_string())],
            format!("Backup file: {}", path.display()),
        ));
    }
    for path in item_backup_manifest_paths {
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-backup-manifest",
            &[("path", path.display().to_string())],
            format!("Backup manifest: {}", path.display()),
        ));
    }

    for item in &report.package_operation.items {
        let package_name = package_display_name(model, &item.package_id);
        let localized_message = localizer
            .as_ref()
            .map(|localizer| {
                localized_package_operation_message(
                    localizer,
                    &item.message_code,
                    &package_name,
                    &item.message,
                )
            })
            .unwrap_or_else(|| item.message.clone());
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-package-message",
            &[
                ("package", package_name.clone()),
                ("message", localized_message.clone()),
            ],
            format!("{package_name}: {localized_message}"),
        ));
        let plan_action_label = action_label_for_summary(localizer.as_ref(), item.plan_action);
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-package-plan-action",
            &[("action", plan_action_label.clone())],
            format!("  Plan action: {plan_action_label}"),
        ));
        let status_label = status_label_for_summary(localizer.as_ref(), item.status);
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-package-status",
            &[("status", status_label.clone())],
            format!("  Status: {status_label}"),
        ));
        // Surface the installed version so users can confirm the install
        // landed without having to scroll through the install report. The
        // artifact descriptor's version is what RABBIT chose to install (and,
        // for an InstalledOrChecked status, what now lives on disk per the
        // receipt the operation pipeline just wrote).
        if matches!(
            item.status,
            PackageOperationStatus::InstalledOrChecked | PackageOperationStatus::SkippedCurrent
        ) {
            let installed_version = item.artifact.version.to_string();
            detail_lines.push(format_localized_message(
                localizer.as_ref(),
                "wizard-summary-package-installed-version",
                &[("version", installed_version.clone())],
                format!("  Installed version: {installed_version}"),
            ));
        }
        // Right after the package's own lines, so each says whose it is:
        // each file of its earlier version that the new one no longer
        // installs, and, after a real run, REAPER's language when one of
        // them was the language pack file it named.
        if let Some(install_report) = &report.package_operation.install_report {
            let languages = install_report
                .reaper_language
                .iter()
                .filter(|_| !report.dry_run)
                .filter(|language| language.package_id == item.package_id)
                .map(|language| reaper_language_line(localizer.as_ref(), language));
            detail_lines.extend(
                install_report
                    .actions
                    .iter()
                    .filter(|action| action.package_id == item.package_id)
                    .flat_map(|action| {
                        superseded_file_lines(
                            localizer.as_ref(),
                            &install_report.resource_path,
                            action,
                        )
                    })
                    .chain(languages),
            );
        }
        if let Some(plan) = &item.planned_execution {
            detail_lines.push(format_localized_message(
                localizer.as_ref(),
                "wizard-summary-planned-execution-title",
                &[],
                "Planned unattended execution:".to_string(),
            ));
            let runner = planned_execution_runner_label(localizer.as_ref(), plan.kind);
            detail_lines.push(format_localized_message(
                localizer.as_ref(),
                "wizard-summary-planned-execution-runner",
                &[("runner", runner.clone())],
                format!("  Runner: {runner}"),
            ));
            detail_lines.push(format_localized_message(
                localizer.as_ref(),
                "wizard-summary-planned-execution-artifact",
                &[("artifact", plan.artifact_location.clone())],
                format!("  Artifact: {}", plan.artifact_location),
            ));
            detail_lines.extend(planned_execution_program_line(localizer.as_ref(), plan));
            if !plan.arguments.is_empty() {
                let arguments = plan.arguments.join(" ");
                detail_lines.push(format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-planned-execution-arguments",
                    &[("arguments", arguments.clone())],
                    format!("  Arguments: {arguments}"),
                ));
            }
            if let Some(path) = &plan.working_directory {
                detail_lines.push(format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-planned-execution-working-directory",
                    &[("path", path.display().to_string())],
                    format!("  Working directory: {}", path.display()),
                ));
            }
            detail_lines.extend(plan.verification_paths.iter().map(|path| {
                format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-planned-execution-verify",
                    &[("path", path.display().to_string())],
                    format!("  Verify: {}", path.display()),
                )
            }));
        }
        if let Some(manual) = &item.manual_instruction {
            detail_lines.push(format_localized_message(
                localizer.as_ref(),
                "wizard-summary-manual-title",
                &[("title", manual.title.clone())],
                format!("{}:", manual.title),
            ));
            detail_lines.extend(manual.steps.iter().map(|step| {
                format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-manual-step",
                    &[("step", step.clone())],
                    format!("  {step}"),
                )
            }));
            detail_lines.extend(manual.notes.iter().map(|note| {
                format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-manual-note",
                    &[("note", note.clone())],
                    format!("  Note: {note}"),
                )
            }));
        }
    }

    for step_report in &report.configuration_steps {
        let step_name = localized_configuration_step_name(localizer.as_ref(), &step_report.step_id);
        let localized_message = localizer
            .as_ref()
            .map(|localizer| {
                localized_configuration_message(
                    localizer,
                    &step_report.message_code,
                    &step_report.message,
                )
            })
            .unwrap_or_else(|| step_report.message.clone());
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-configuration-message",
            &[
                ("step", step_name.clone()),
                ("message", localized_message.clone()),
            ],
            format!("{step_name}: {localized_message}"),
        ));
        let status_label =
            configuration_status_label_for_summary(localizer.as_ref(), step_report.status);
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-configuration-status",
            &[("status", status_label.clone())],
            format!("  Status: {status_label}"),
        ));
    }

    // When some packages failed, lead with a "completed with errors" line
    // (and, for the antivirus false-positive case, the localized how-to-
    // allow-it guidance) instead of the plain finished line, so the result
    // page names the problem up front. A skipped-because-REAPER-failed
    // package counts toward the failure total.
    if failed_items > 0 {
        if any_antivirus_block {
            detail_lines.push(format_localized_message(
                localizer.as_ref(),
                "wizard-summary-error-antivirus",
                &[],
                "Windows security software blocked at least one download. Open Windows \
                 Security > Virus & threat protection > Protection history, allow the blocked \
                 item, and run RABBIT again."
                    .to_string(),
            ));
        }
        return WizardInstallSummary {
            status_line: format_localized_message(
                localizer.as_ref(),
                "wizard-summary-status-finished-with-errors",
                &[
                    ("installed", installed_or_checked.to_string()),
                    ("failed", failed_items.to_string()),
                ],
                format!(
                    "Finished with errors. {installed_or_checked} package item(s) installed or checked; {failed_items} failed."
                ),
            ),
            detail_lines,
        };
    }

    WizardInstallSummary {
        status_line: format_localized_message(
            localizer.as_ref(),
            "wizard-summary-status-finished",
            &[
                ("installed", installed_or_checked.to_string()),
                ("manual", manual_items.to_string()),
            ],
            format!(
                "Finished. {installed_or_checked} package item(s) installed or checked; {manual_items} require manual attention."
            ),
        ),
        detail_lines,
    }
}

/// Show details' lines for a file of a package's earlier version that the
/// new version no longer installs, as [`superseded_file_line`] gives them.
/// A file that moved into the backup from outside the resource folder
/// `resource_path`, from a folder other programs read too, such as an
/// app2clap from the CLAP folder, gets a line of its own instead, which
/// names its path once: the new version installs it in REAPER's own folder
/// instead, and portable REAPERs and other programs that loaded it from
/// there no longer find it.
fn superseded_file_lines(
    localizer: Option<&Localizer>,
    resource_path: &Path,
    action: &InstallFileReport,
) -> Vec<String> {
    if action.action == InstallFileAction::MovedToBackup
        && !action.target_path.starts_with(resource_path)
    {
        let path = action.target_path.display().to_string();
        return vec![format_localized_message(
            localizer,
            "wizard-summary-moved-from-shared-folder",
            &[("path", path.clone())],
            format!(
                "Moved into the backup, because the new version installs it in REAPER's own \
                 folder instead. Portable REAPERs and other programs that loaded it from there \
                 no longer find it: {path}"
            ),
        )];
    }
    superseded_file_line(localizer, action)
        .into_iter()
        .collect()
}

/// Show details' line for a file of a package's earlier version that the
/// new version no longer installs: where it was, when it moved into the
/// update's backup, or, when it stayed where it was, why, in the user's
/// language. The core's own `warning` is English, so it isn't shown. `None`
/// for every other file.
fn superseded_file_line(
    localizer: Option<&Localizer>,
    action: &InstallFileReport,
) -> Option<String> {
    let path = action.target_path.display().to_string();
    let (key, fallback) = match action.action {
        InstallFileAction::MovedToBackup => (
            "wizard-summary-moved-to-backup",
            "Moved into the backup, because the new version no longer installs it",
        ),
        InstallFileAction::LeftInPlace => match action.left_reason {
            Some(LeftInPlaceReason::InUse) => (
                "wizard-summary-left-in-place-in-use",
                "Stays where it is, because another program has it open",
            ),
            Some(LeftInPlaceReason::Link) => (
                "wizard-summary-left-in-place-link",
                "Stays where it is, because it is a link RABBIT never follows",
            ),
            Some(LeftInPlaceReason::Folder) => (
                "wizard-summary-left-in-place-folder",
                "Stays where it is, because it is a folder",
            ),
            Some(LeftInPlaceReason::Keymap) => (
                "wizard-summary-left-in-place-keymap",
                "Stays where it is, because it is REAPER's key map",
            ),
            Some(LeftInPlaceReason::Refused) => (
                "wizard-summary-left-in-place-refused",
                "Stays where it is, because no restore could put it back there",
            ),
            Some(LeftInPlaceReason::Unreadable) => (
                "wizard-summary-left-in-place-unreadable",
                "Stays where it is, because RABBIT couldn't read it",
            ),
            Some(LeftInPlaceReason::Other) | None => (
                "wizard-summary-left-in-place-other",
                "Stays where it is, because RABBIT couldn't move it into the backup",
            ),
        },
        _ => return None,
    };
    Some(format_localized_message(
        localizer,
        key,
        &[("path", path.clone())],
        format!("{fallback}: {path}"),
    ))
}

/// Show details' line for what became of REAPER's language after an update
/// moved the language pack file it named into the backup, in the user's
/// language: set to the new version's file, or why it wasn't. The core's
/// own error is English, so it isn't shown.
fn reaper_language_line(localizer: Option<&Localizer>, language: &ReaperLanguageReport) -> String {
    let file = language.langpack.clone();
    let before = language
        .langpack_before
        .clone()
        .unwrap_or_else(|| file.clone());
    if language.error.is_none() && language.switched {
        format_localized_message(
            localizer,
            "wizard-summary-language-switched",
            &[("file", file.clone()), ("before", before.clone())],
            format!("REAPER's language now uses {file}, because {before} moved into the backup."),
        )
    } else if language.error.is_none() && language.reaper_may_run {
        format_localized_message(
            localizer,
            "wizard-summary-language-reaper-may-run",
            &[("file", file.clone()), ("before", before.clone())],
            format!(
                "REAPER's language still names {before}, which moved into the backup, because \
                 REAPER may be open. Once REAPER is closed, choose {file} in REAPER's preferences."
            ),
        )
    } else {
        format_localized_message(
            localizer,
            "wizard-summary-language-failed",
            &[("file", file.clone())],
            format!(
                "RABBIT couldn't set REAPER's language to {file}. Choose it in REAPER's preferences."
            ),
        )
    }
}

/// The sentences Done adds after an install that left files of a package's
/// earlier version where they were, instead of moving them into the
/// update's backup: how many, that RABBIT no longer keeps track of them and
/// can't restore that earlier version, and that Show details says which
/// and why. `None` when it left none, and
/// after a dry run, which moved nothing.
pub(crate) fn left_in_place_hint(
    localizer: Option<&Localizer>,
    report: &SetupReport,
) -> Option<String> {
    if report.dry_run {
        return None;
    }
    let count = report
        .package_operation
        .install_report
        .as_ref()?
        .actions
        .iter()
        .filter(|action| action.action == InstallFileAction::LeftInPlace)
        .count();
    if count == 0 {
        return None;
    }
    Some(match localizer {
        Some(localizer) => {
            localizer
                .format_with(
                    "wizard-summary-left-in-place-hint",
                    &[("count", FluentArgument::from(count))],
                )
                .value
        }
        None => format!(
            "{count} file(s) of earlier versions stayed where they were. RABBIT no longer keeps \
             track of them and can't restore those versions. Show details says which and why."
        ),
    })
}

/// The summary's "Program" line for a planned execution. An installer that
/// arrives zipped (an OSARA pull-request build) has no program to name until
/// it is downloaded and unpacked, so the line says what will run instead of
/// leaving the reader to wonder why it is missing. Other plans without a
/// program (archives, disk images) get no line, as before.
pub(crate) fn planned_execution_program_line(
    localizer: Option<&Localizer>,
    plan: &PlannedExecutionPlan,
) -> Option<String> {
    match &plan.program {
        Some(program) => Some(format_localized_message(
            localizer,
            "wizard-summary-planned-execution-program",
            &[("program", program.clone())],
            format!("  Program: {program}"),
        )),
        None if plan.kind == PlannedExecutionKind::LaunchInstallerExecutable => {
            Some(format_localized_message(
                localizer,
                "wizard-summary-planned-execution-program-zipped",
                &[],
                "  Program: the installer inside the .zip, once it is downloaded".to_string(),
            ))
        }
        None => None,
    }
}
