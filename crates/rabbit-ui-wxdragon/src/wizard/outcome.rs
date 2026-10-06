//! Outcome reports: what the wizard writes to disk after a run and the
//! error summaries it shows when one fails.

use std::path::PathBuf;

use rabbit_core::package::PACKAGE_OSARA;
use rabbit_core::report::{default_report_path, save_json_and_text_reports};
use rabbit_core::setup::SetupReport;
use rabbit_core::{RabbitError, Result};

use super::bootstrap::localizer_from_options;
use super::labels::format_localized_message;
use super::model::{
    OsaraKeymapChoice, WizardInstallRequest, WizardInstallSummary, WizardModel,
    WizardOutcomeReport, WizardOutcomeStatus,
};
use super::packages::package_display_name;
use super::summary::summarize_setup_report;

pub fn wizard_outcome_report_from_success(
    model: &WizardModel,
    request: &WizardInstallRequest,
    report: &SetupReport,
) -> WizardOutcomeReport {
    let summary = summarize_setup_report(model, report);
    WizardOutcomeReport {
        // Cancellation outranks a package failure in the heading: "you
        // stopped it" explains the half-finished run, where "finished with
        // errors" would send the user looking for a fault.
        status: if report.was_cancelled() {
            WizardOutcomeStatus::Cancelled
        } else if report.package_operation.has_failures() {
            WizardOutcomeStatus::CompletedWithErrors
        } else {
            WizardOutcomeStatus::Success
        },
        resource_path: report.resource_path.clone(),
        target_app_path: request.target_app_path.clone(),
        package_ids: request.package_ids.clone(),
        platform: request.platform,
        architecture: request.architecture,
        portable: request.portable,
        dry_run: request.dry_run,
        allow_reaper_running: request.allow_reaper_running,
        stage_unsupported: request.stage_unsupported,
        cache_dir: request.cache_dir.clone(),
        osara_keymap_choice: request.osara_keymap_choice,
        status_line: summary.status_line,
        detail_lines: summary.detail_lines,
        error_message: None,
        setup_report: Some(report.clone()),
    }
}

pub fn summarize_wizard_error(
    model: &WizardModel,
    request: &WizardInstallRequest,
    error: &RabbitError,
) -> WizardInstallSummary {
    let localizer = localizer_from_options(&model.bootstrap_options).ok();
    let selected_packages = if request.package_ids.is_empty() {
        model.text.review_no_package.clone()
    } else {
        request
            .package_ids
            .iter()
            .map(|package_id| package_display_name(model, package_id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut detail_lines = vec![
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-target",
            &[("path", request.resource_path.display().to_string())],
            format!("Target: {}", request.resource_path.display()),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-portable",
            &[(
                "value",
                if request.portable {
                    model.text.common_yes.clone()
                } else {
                    model.text.common_no.clone()
                },
            )],
            format!(
                "Portable target: {}",
                if request.portable {
                    &model.text.common_yes
                } else {
                    &model.text.common_no
                }
            ),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-dry-run",
            &[(
                "value",
                if request.dry_run {
                    model.text.common_yes.clone()
                } else {
                    model.text.common_no.clone()
                },
            )],
            format!(
                "Dry run: {}",
                if request.dry_run {
                    &model.text.common_yes
                } else {
                    &model.text.common_no
                }
            ),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-packages-selected",
            &[("packages", selected_packages.clone())],
            format!("Packages selected: {selected_packages}"),
        ),
        format_localized_message(
            localizer.as_ref(),
            "wizard-summary-cache",
            &[("path", request.cache_dir.display().to_string())],
            format!("Cache: {}", request.cache_dir.display()),
        ),
    ];

    if let Some(target_app_path) = &request.target_app_path {
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-planned-app",
            &[("path", target_app_path.display().to_string())],
            format!("Planned app path: {}", target_app_path.display()),
        ));
    }

    if request
        .package_ids
        .iter()
        .any(|package_id| package_id == PACKAGE_OSARA)
    {
        detail_lines.push(model.text.review_osara_keymap_heading.clone());
        detail_lines.push(match request.osara_keymap_choice {
            OsaraKeymapChoice::PreserveCurrent => model.text.review_osara_keymap_preserve.clone(),
            OsaraKeymapChoice::ReplaceCurrent => model.text.review_osara_keymap_replace.clone(),
        });
    }

    detail_lines.push(format_localized_message(
        localizer.as_ref(),
        "wizard-summary-error",
        &[("message", error.to_string())],
        format!("Error: {error}"),
    ));

    // The error text itself is English (it goes into bug reports); an
    // antivirus block is the one failure a user can usually clear
    // themselves, so follow it with the remediation steps in their own
    // language.
    if matches!(error, RabbitError::WindowsFileBlockedByAntivirus { .. }) {
        detail_lines.push(format_localized_message(
            localizer.as_ref(),
            "wizard-summary-error-antivirus",
            &[],
            "Windows security software blocked this download. Open Windows Security > \
             Virus & threat protection > Protection history, allow the blocked item, and run \
             RABBIT again."
                .to_string(),
        ));
    }

    WizardInstallSummary {
        status_line: model.text.done_status_error.clone(),
        detail_lines,
    }
}

pub fn wizard_outcome_report_from_error(
    model: &WizardModel,
    request: &WizardInstallRequest,
    error: &RabbitError,
) -> WizardOutcomeReport {
    let summary = summarize_wizard_error(model, request, error);
    WizardOutcomeReport {
        status: WizardOutcomeStatus::Error,
        resource_path: request.resource_path.clone(),
        target_app_path: request.target_app_path.clone(),
        package_ids: request.package_ids.clone(),
        platform: request.platform,
        architecture: request.architecture,
        portable: request.portable,
        dry_run: request.dry_run,
        allow_reaper_running: request.allow_reaper_running,
        stage_unsupported: request.stage_unsupported,
        cache_dir: request.cache_dir.clone(),
        osara_keymap_choice: request.osara_keymap_choice,
        status_line: summary.status_line,
        detail_lines: summary.detail_lines,
        error_message: Some(error.to_string()),
        setup_report: None,
    }
}

pub fn save_wizard_outcome_report(report: &WizardOutcomeReport) -> Result<PathBuf> {
    let json_path = default_report_path(&report.resource_path, "setup");
    let saved = save_json_and_text_reports(&json_path, report)?;
    Ok(saved.text_path)
}

pub fn save_wizard_setup_report(report: &SetupReport) -> Result<PathBuf> {
    let json_path = default_report_path(&report.resource_path, "setup");
    let saved = save_json_and_text_reports(&json_path, report)?;
    Ok(saved.text_path)
}
