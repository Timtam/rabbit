use std::path::PathBuf;

use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::plan::InstallPlan;
use tempfile::tempdir;

use super::support::*;
use crate::wizard::*;

/// Issue #32: the Done page said "Review the error below" with nothing
/// below it. The reason now goes into the status field itself.
#[test]
fn a_failed_install_says_why_in_words() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![fake_installation()],
        Some(0),
        InstallPlan {
            target: None,
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );
    // The exact error the reporter's run produced.
    let reaper_open = rabbit_core::RabbitError::PreflightFailed {
        message: "reaper-process: Close REAPER before installing extensions: reaper.exe (5096)."
            .to_string(),
    };
    assert_eq!(
        wizard_error_reason(&model, &reaper_open),
        "RABBIT stopped because REAPER was open. Close REAPER, then run RABBIT again."
    );
    // A second failed check stays in view: closing REAPER won't fix it.
    let both = rabbit_core::RabbitError::PreflightFailed {
        message: "resource-path: the folder is not writable.; reaper-process: Close REAPER before installing extensions: reaper.exe (5096)."
            .to_string(),
    };
    assert_eq!(
        wizard_error_reason(&model, &both),
        "RABBIT stopped because REAPER was open. Close REAPER, then run RABBIT again.\nresource-path: the folder is not writable."
    );
    // Any other failure shows its own message rather than nothing.
    let other = rabbit_core::RabbitError::PreflightFailed {
        message: r"resource-path: C:\REAPER is not writable.".to_string(),
    };
    assert_eq!(wizard_error_reason(&model, &other), other.to_string());
}

#[test]
fn wizard_error_summary_includes_selected_request_context() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![fake_installation()],
        Some(0),
        InstallPlan {
            target: None,
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );
    let request = sample_install_request(PathBuf::from("C:/PortableREAPER"));
    let error = rabbit_core::RabbitError::PreflightFailed {
        message: "REAPER is running.".to_string(),
    };

    let summary = summarize_wizard_error(&model, &request, &error);

    assert_eq!(
        summary.status_line,
        "Installation failed. Review the error below."
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Packages selected: OSARA, ReaPack"))
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line == "OSARA key map")
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Backup your current key map"))
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Error: preflight failed: REAPER is running."))
    );
}

#[test]
fn wizard_error_summary_adds_localized_hint_for_antivirus_blocks() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![fake_installation()],
        Some(0),
        InstallPlan {
            target: None,
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );
    let request = sample_install_request(PathBuf::from("C:/PortableREAPER"));

    let blocked = rabbit_core::RabbitError::WindowsFileBlockedByAntivirus {
        path: PathBuf::from("C:/Temp/rabbit-cache/osara/osara.exe"),
        source: std::io::Error::from_raw_os_error(225),
    };
    let summary = summarize_wizard_error(&model, &request, &blocked);
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Protection history")),
        "antivirus blocks must carry the remediation steps: {:?}",
        summary.detail_lines
    );

    // Unrelated failures must not gain the antivirus advice.
    let other = rabbit_core::RabbitError::PreflightFailed {
        message: "REAPER is running.".to_string(),
    };
    let summary = summarize_wizard_error(&model, &request, &other);
    assert!(
        !summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Protection history")),
        "only antivirus blocks get the antivirus advice: {:?}",
        summary.detail_lines
    );
}

#[test]
fn saves_wizard_outcome_error_report_under_resource_logs() {
    let dir = tempdir().unwrap();
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![fake_installation()],
        Some(0),
        InstallPlan {
            target: None,
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );
    let request = sample_install_request(dir.path().join("PortableREAPER"));
    let error = rabbit_core::RabbitError::PreflightFailed {
        message: "Target path blocked".to_string(),
    };
    let report = wizard_outcome_report_from_error(&model, &request, &error);

    let path = save_wizard_outcome_report(&report).unwrap();
    let json_path = path.with_extension("json");

    assert!(path.starts_with(dir.path().join("PortableREAPER/RABBIT/logs")));
    assert!(path.is_file());
    assert!(json_path.is_file());
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("status: error"));
    assert!(content.contains("error_message: preflight failed: Target path blocked"));
}

#[test]
fn saves_wizard_setup_report_under_resource_logs() {
    let dir = tempdir().unwrap();
    let report = empty_setup_report(dir.path().join("PortableREAPER"));

    let path = save_wizard_setup_report(&report).unwrap();
    let json_path = path.with_extension("json");

    assert!(path.starts_with(dir.path().join("PortableREAPER/RABBIT/logs")));
    assert!(path.is_file());
    assert!(json_path.is_file());
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("RABBIT Report"));
    assert!(content.contains("resource_path:"));
}
