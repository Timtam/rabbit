use std::path::PathBuf;

use rabbit_core::artifact::{ArtifactDescriptor, ArtifactKind};
use rabbit_core::install::{InstallFileAction, InstallFileReport, InstallReport};
use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::operation::{
    ManualInstallInstruction, PackageOperationItem, PackageOperationReport, PackageOperationStatus,
    PlannedExecutionKind, PlannedExecutionPlan,
};
use rabbit_core::package::{PACKAGE_OSARA, PACKAGE_REAPACK};
use rabbit_core::plan::{InstallPlan, PlanActionKind};
use rabbit_core::preflight::PreflightReport;
use rabbit_core::resource::ResourceInitReport;
use rabbit_core::setup::SetupReport;
use rabbit_core::version::Version;

use super::support::*;
use crate::wizard::*;

#[test]
fn setup_summary_includes_manual_instruction_notes() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        Vec::new(),
        None,
        InstallPlan {
            target: None,
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );
    let report = SetupReport {
        resource_path: PathBuf::from("C:/PortableREAPER"),
        dry_run: true,
        resource_init: ResourceInitReport {
            resource_path: PathBuf::from("C:/PortableREAPER"),
            dry_run: true,
            portable: true,
            preflight: PreflightReport {
                passed: true,
                checks: Vec::new(),
            },
            actions: Vec::new(),
        },
        package_operation: PackageOperationReport {
            resource_path: PathBuf::from("C:/PortableREAPER"),
            dry_run: true,
            install_report: None,
            receipt_backup_path: None,
            receipt_backup_manifest_path: None,
            items: vec![PackageOperationItem {
                package_id: PACKAGE_OSARA.to_string(),
                plan_action: PlanActionKind::Install,
                status: PackageOperationStatus::DeferredUnattended,
                artifact: ArtifactDescriptor {
                    package_id: PACKAGE_OSARA.to_string(),
                    version: Version::parse("2026.1").unwrap(),
                    platform: Platform::Windows,
                    architecture: Architecture::X64,
                    kind: ArtifactKind::Installer,
                    url: "https://example.test/osara.exe".to_string(),
                    file_name: "osara.exe".to_string(),
                },
                cached_artifact: None,
                install_action: None,
                backup_paths: Vec::new(),
                backup_manifest_path: None,
                planned_execution: Some(PlannedExecutionPlan {
                    kind: PlannedExecutionKind::LaunchInstallerExecutable,
                    artifact_location: "https://example.test/osara.exe".to_string(),
                    program: Some("https://example.test/osara.exe".to_string()),
                    arguments: Vec::new(),
                    working_directory: None,
                    verification_paths: vec![
                        PathBuf::from("C:/PortableREAPER/UserPlugins"),
                        PathBuf::from("C:/PortableREAPER/osara"),
                    ],
                    requires_elevation: false,
                    freshness_paths: Vec::new(),
                }),
                manual_instruction: Some(ManualInstallInstruction {
                    title: "Manual install required for osara".to_string(),
                    steps: vec!["Use this artifact: https://example.test/osara.exe".to_string()],
                    notes: vec![
                        "The selected workflow preserves the current key map. Leave reaper-kb.ini unchanged.".to_string(),
                    ],
                }),
                message: "This build has not implemented the planned unattended vendor installer execution path yet. RABBIT did not download or run the artifact.".to_string(),
                message_code: rabbit_core::operation::PackageOperationMessage::DeferredUnattendedNotStaged {
                    artifact_kind: ArtifactKind::Installer,
                },
            }],
        },
        configuration_steps: Vec::new(),
    };

    let summary = summarize_setup_report(&model, &report);

    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Planned unattended execution"))
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Runner:") && line.contains("Launch installer executable"))
    );
    assert!(
        summary.detail_lines.iter().any(|line| {
            line.contains("Note:") && line.contains("Leave reaper-kb.ini unchanged")
        })
    );
    // Architecture line + per-package plan action / status are now part
    // of the saved report so power users have everything the wizard hides.
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Architecture:") && line.contains("x64"))
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Plan action:") && line.contains("Will install"))
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Status:") && line.contains("Deferred unattended"))
    );
}

#[test]
fn setup_summary_includes_backup_paths_when_present() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        Vec::new(),
        None,
        InstallPlan {
            target: None,
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );
    let report = SetupReport {
        resource_path: PathBuf::from("C:/PortableREAPER"),
        dry_run: false,
        resource_init: ResourceInitReport {
            resource_path: PathBuf::from("C:/PortableREAPER"),
            dry_run: false,
            portable: true,
            preflight: PreflightReport {
                passed: true,
                checks: Vec::new(),
            },
            actions: Vec::new(),
        },
        package_operation: PackageOperationReport {
            resource_path: PathBuf::from("C:/PortableREAPER"),
            dry_run: false,
            install_report: Some(InstallReport {
                resource_path: PathBuf::from("C:/PortableREAPER"),
                dry_run: false,
                preflight: PreflightReport {
                    passed: true,
                    checks: Vec::new(),
                },
                receipt_written: true,
                receipt_backup_path: Some(PathBuf::from(
                    "C:/PortableREAPER/RABBIT/backups/unix-1/RABBIT/install-state.json",
                )),
                backup_manifest_path: Some(PathBuf::from(
                    "C:/PortableREAPER/RABBIT/backups/unix-1/backup-manifest.json",
                )),
                actions: vec![InstallFileReport {
                    package_id: PACKAGE_REAPACK.to_string(),
                    source_path: PathBuf::from("C:/cache/reaper_reapack-x64.dll"),
                    target_path: PathBuf::from(
                        "C:/PortableREAPER/UserPlugins/reaper_reapack-x64.dll",
                    ),
                    backup_path: Some(PathBuf::from(
                        "C:/PortableREAPER/RABBIT/backups/unix-1/UserPlugins/reaper_reapack-x64.dll",
                    )),
                    action: InstallFileAction::Replaced,
                    size: 7,
                    sha256: "hash".to_string(),
                }],
            }),
            receipt_backup_path: None,
            receipt_backup_manifest_path: None,
            items: vec![PackageOperationItem {
                package_id: PACKAGE_REAPACK.to_string(),
                plan_action: PlanActionKind::Update,
                status: PackageOperationStatus::InstalledOrChecked,
                artifact: ArtifactDescriptor {
                    package_id: PACKAGE_REAPACK.to_string(),
                    version: Version::parse("1.2.6").unwrap(),
                    platform: Platform::Windows,
                    architecture: Architecture::X64,
                    kind: ArtifactKind::ExtensionBinary,
                    url: "https://example.test/reaper_reapack-x64.dll".to_string(),
                    file_name: "reaper_reapack-x64.dll".to_string(),
                },
                cached_artifact: None,
                install_action: None,
                backup_paths: Vec::new(),
                backup_manifest_path: None,
                planned_execution: None,
                manual_instruction: None,
                message: "Single extension binary handled by RABBIT installer.".to_string(),
                message_code:
                    rabbit_core::operation::PackageOperationMessage::ExtensionBinaryInstalled,
            }],
        },
        configuration_steps: Vec::new(),
    };

    let summary = summarize_setup_report(&model, &report);

    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Backup file:"))
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Receipt backup:"))
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Backup manifest:"))
    );
}

#[test]
fn setup_summary_includes_unattended_receipt_backup_paths() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        Vec::new(),
        None,
        InstallPlan {
            target: None,
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );
    let report = SetupReport {
        resource_path: PathBuf::from("C:/PortableREAPER"),
        dry_run: false,
        resource_init: ResourceInitReport {
            resource_path: PathBuf::from("C:/PortableREAPER"),
            dry_run: false,
            portable: true,
            preflight: PreflightReport {
                passed: true,
                checks: Vec::new(),
            },
            actions: Vec::new(),
        },
        package_operation: PackageOperationReport {
            resource_path: PathBuf::from("C:/PortableREAPER"),
            dry_run: false,
            install_report: None,
            receipt_backup_path: Some(PathBuf::from(
                "C:/PortableREAPER/RABBIT/backups/unattended-1/RABBIT/install-state.json",
            )),
            receipt_backup_manifest_path: Some(PathBuf::from(
                "C:/PortableREAPER/RABBIT/backups/unattended-1/backup-manifest.json",
            )),
            items: vec![PackageOperationItem {
                package_id: PACKAGE_OSARA.to_string(),
                plan_action: PlanActionKind::Install,
                status: PackageOperationStatus::InstalledOrChecked,
                artifact: ArtifactDescriptor {
                    package_id: PACKAGE_OSARA.to_string(),
                    version: Version::parse("2026.1").unwrap(),
                    platform: Platform::Windows,
                    architecture: Architecture::X64,
                    kind: ArtifactKind::Installer,
                    url: "https://example.test/osara.exe".to_string(),
                    file_name: "osara.exe".to_string(),
                },
                cached_artifact: None,
                install_action: None,
                backup_paths: Vec::new(),
                backup_manifest_path: None,
                planned_execution: None,
                manual_instruction: None,
                message: "RABBIT ran the upstream installer unattended, verified the expected target paths, and updated the RABBIT receipt.".to_string(),
                message_code: rabbit_core::operation::PackageOperationMessage::UnattendedInstalled,
            }],
        },
        configuration_steps: Vec::new(),
    };

    let summary = summarize_setup_report(&model, &report);

    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Receipt backup:"))
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Backup manifest:"))
    );
}

/// A run with a failed package renders the "finished with errors"
/// status line, and an antivirus block adds the localized how-to-allow
/// guidance once for the whole run.
#[test]
fn setup_summary_reports_failures_and_antivirus_hint() {
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
    let mut report = empty_setup_report(PathBuf::from("C:/PortableREAPER"));
    report.package_operation.items.push(PackageOperationItem {
        package_id: PACKAGE_OSARA.to_string(),
        plan_action: PlanActionKind::Install,
        status: PackageOperationStatus::Failed,
        artifact: ArtifactDescriptor {
            package_id: PACKAGE_OSARA.to_string(),
            version: Version::parse("2026.1").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: "https://example.test/osara.exe".to_string(),
            file_name: "osara.exe".to_string(),
        },
        cached_artifact: None,
        install_action: None,
        backup_paths: Vec::new(),
        backup_manifest_path: None,
        planned_execution: None,
        manual_instruction: None,
        message: "Installation failed: Windows security software blocked …".to_string(),
        message_code: rabbit_core::operation::PackageOperationMessage::InstallFailed {
            error: "Windows security software blocked …".to_string(),
            antivirus_block: true,
        },
    });

    let summary = summarize_setup_report(&model, &report);

    assert!(
        summary.status_line.contains("Finished with errors"),
        "status line: {}",
        summary.status_line
    );
    assert!(
        summary
            .detail_lines
            .iter()
            .any(|line| line.contains("Protection history")),
        "antivirus hint missing: {:?}",
        summary.detail_lines
    );
}
