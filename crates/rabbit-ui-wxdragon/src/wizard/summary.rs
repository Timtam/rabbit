//! Renders a finished [`SetupReport`] into the done page's summary text.

use rabbit_core::operation::PackageOperationStatus;
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
        if !backup_paths.is_empty()
            || install_report.receipt_backup_path.is_some()
            || install_report.backup_manifest_path.is_some()
        {
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
            if let Some(path) = &install_report.receipt_backup_path {
                detail_lines.push(format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-receipt-backup",
                    &[("path", path.display().to_string())],
                    format!("Receipt backup: {}", path.display()),
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
    let package_receipt_backup_path = report.package_operation.receipt_backup_path.as_ref();
    let package_receipt_backup_manifest_path = report
        .package_operation
        .receipt_backup_manifest_path
        .as_ref();
    if report.package_operation.install_report.is_none()
        && (!item_backup_paths.is_empty()
            || !item_backup_manifest_paths.is_empty()
            || package_receipt_backup_path.is_some()
            || package_receipt_backup_manifest_path.is_some())
    {
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-backup-files-created",
            &[(
                "count",
                (item_backup_paths.len() + usize::from(package_receipt_backup_path.is_some()))
                    .to_string(),
            )],
            format!(
                "Backup files created: {}",
                item_backup_paths.len() + usize::from(package_receipt_backup_path.is_some())
            ),
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
    if let Some(path) = package_receipt_backup_path {
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-receipt-backup",
            &[("path", path.display().to_string())],
            format!("Receipt backup: {}", path.display()),
        ));
    }
    if let Some(path) = package_receipt_backup_manifest_path {
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
                localized_package_operation_message(localizer, &item.message_code, &item.message)
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
            if let Some(program) = &plan.program {
                detail_lines.push(format_localized_message(
                    localizer.as_ref(),
                    "wizard-summary-planned-execution-program",
                    &[("program", program.clone())],
                    format!("  Program: {program}"),
                ));
            }
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
