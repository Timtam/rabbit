use std::path::PathBuf;

use rabbit_core::artifact::{ArtifactDescriptor, ArtifactKind};
use rabbit_core::install::{InstallFileAction, InstallFileReport, InstallReport};
use rabbit_core::localization::{DEFAULT_LOCALE, Localizer, embedded_locales};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::operation::{
    InstallerExitMeaning, ManualInstallInstruction, PackageOperationItem, PackageOperationMessage,
    PackageOperationReport, PackageOperationStatus, PlannedExecutionKind, PlannedExecutionPlan,
    localized_install_problem,
};
use rabbit_core::package::{
    PACKAGE_JAWS_SCRIPTS, PACKAGE_OSARA, PACKAGE_REAPACK, PACKAGE_SURGE_XT,
};
use rabbit_core::plan::{InstallPlan, PlanActionKind};
use rabbit_core::preflight::PreflightReport;
use rabbit_core::resource::ResourceInitReport;
use rabbit_core::setup::SetupReport;
use rabbit_core::version::Version;

use super::support::*;
use crate::wizard::*;

#[test]
fn the_summary_says_what_runs_for_an_installer_still_in_its_zip() {
    let plan = |kind, program: Option<&str>| PlannedExecutionPlan {
        kind,
        artifact_location: "https://nightly.link/jcsteh/osara/actions/artifacts/1.zip".to_string(),
        program: program.map(str::to_string),
        arguments: Vec::new(),
        working_directory: None,
        verification_paths: Vec::new(),
        requires_elevation: false,
        freshness_paths: Vec::new(),
        installer_log: None,
    };
    let english = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let german = Localizer::embedded("de-DE").unwrap();

    let zipped = plan(PlannedExecutionKind::LaunchInstallerExecutable, None);
    let line = planned_execution_program_line(Some(&english), &zipped).unwrap();
    assert!(line.contains("the installer inside the .zip"), "{line}");
    let line = planned_execution_program_line(Some(&german), &zipped).unwrap();
    assert!(line.contains("ZIP-Datei"), "{line}");

    let downloaded = plan(
        PlannedExecutionKind::LaunchInstallerExecutable,
        Some(r"C:\cache\osara.unzipped\osara.exe"),
    );
    let line = planned_execution_program_line(Some(&english), &downloaded).unwrap();
    assert!(
        line.contains(r"C:\cache\osara.unzipped\osara.exe"),
        "{line}"
    );

    // A plan that never names a program still gets no line.
    let archive = plan(PlannedExecutionKind::ExtractArchiveAndRunInstaller, None);
    assert_eq!(
        planned_execution_program_line(Some(&english), &archive),
        None
    );
}

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
                    channel: None,
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
                    installer_log: None,
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
                    channel: None,
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
                    channel: None,
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
            channel: None,
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

/// A failed Surge XT install with `message_code`, as the run reports it.
fn failed_surge_item(message_code: PackageOperationMessage) -> PackageOperationItem {
    failed_item(PACKAGE_SURGE_XT, message_code)
}

/// A failed Windows install of `package_id` with `message_code`, as the run
/// reports it.
fn failed_item(package_id: &str, message_code: PackageOperationMessage) -> PackageOperationItem {
    PackageOperationItem {
        package_id: package_id.to_string(),
        plan_action: PlanActionKind::Install,
        status: PackageOperationStatus::Failed,
        artifact: ArtifactDescriptor {
            package_id: package_id.to_string(),
            version: Version::parse("2026.11.9").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: "https://example.test/surge-xt-setup.exe".to_string(),
            file_name: "surge-xt-setup.exe".to_string(),
            channel: None,
        },
        cached_artifact: None,
        install_action: None,
        backup_paths: Vec::new(),
        backup_manifest_path: None,
        planned_execution: None,
        manual_instruction: None,
        message: "The English text of the report".to_string(),
        message_code,
    }
}

/// A vendor installer's exit code, and the programs to close before an
/// installer can run, show on Done in the user's language after the
/// package's name: what the code means and where the installer's log is,
/// not "Installation failed:" and the English error with Rust's `Some(5)`.
/// Every locale, with the package named in that language.
#[test]
fn setup_summary_explains_installer_problems_in_every_locale() {
    let english = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let mut model = model_from_plan(
        &english,
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
    let log = PathBuf::from(
        r"C:\Users\Toni Barth\AppData\Roaming\REAPER\RABBIT\logs\surge-xt-installer-unix-1.log",
    );
    let exit_code = PackageOperationMessage::VendorInstallerFailed {
        program: r"C:\cache\surge-xt-setup.exe".to_string(),
        exit_code: 5,
        meaning: Some(InstallerExitMeaning::StoppedAtQuestion),
        log_path: Some(log.clone()),
    };
    let open = PackageOperationMessage::BlockingProgramsRunning {
        programs: vec!["Surge XT.exe".to_string()],
    };

    for &locale in embedded_locales() {
        model.bootstrap_options.locale = locale.to_string();
        let localizer = Localizer::embedded(locale).unwrap();
        let surge = localizer.text("package-surge-xt").value;
        for code in [&exit_code, &open] {
            let mut report = empty_setup_report(PathBuf::from("C:/REAPER"));
            report
                .package_operation
                .items
                .push(failed_surge_item(code.clone()));

            let summary = summarize_setup_report(&model, &report);

            let explained = localized_install_problem(&localizer, code, &surge)
                .unwrap_or_else(|| panic!("{locale} lacks a key for {code:?}"));
            let line = localizer
                .format(
                    "wizard-summary-package-message",
                    &[("package", surge.as_str()), ("message", explained.as_str())],
                )
                .value;
            assert!(
                summary.detail_lines.contains(&line),
                "{locale}: {line:?} not in {:#?}",
                summary.detail_lines
            );
            assert!(
                !summary
                    .detail_lines
                    .iter()
                    .any(|line| line.contains("Some(") || line.contains("English text")),
                "{locale}: {:#?}",
                summary.detail_lines
            );
        }
        let summary = summarize_setup_report(&model, &{
            let mut report = empty_setup_report(PathBuf::from("C:/REAPER"));
            report
                .package_operation
                .items
                .push(failed_surge_item(exit_code.clone()));
            report
        });
        assert!(
            summary
                .detail_lines
                .iter()
                .any(|line| line.contains(&log.display().to_string())),
            "{locale}: the log is named: {:#?}",
            summary.detail_lines
        );
        if locale == "de-DE" {
            assert!(
                summary.detail_lines.contains(&format!(
                    "Surge XT: Das Installationsprogramm wurde mit dem Fehlercode 5 beendet. Das \
                     heißt, es hat bei einer Rückfrage angehalten, die es nicht stellen kann, \
                     solange RABBIT es unbeaufsichtigt ausführt – meist, weil ein anderes \
                     Programm eine seiner Dateien geöffnet hat. Schließen Sie andere Programme, \
                     vor allem andere Musikprogramme und Surge XT, und starten Sie RABBIT dann \
                     erneut. Das Protokoll des Installationsprogramms liegt unter {}.",
                    log.display()
                )),
                "{:#?}",
                summary.detail_lines
            );

            // A package whose name is translated is named in German, not by
            // the manifest's English name.
            let summary = summarize_setup_report(&model, &{
                let mut report = empty_setup_report(PathBuf::from("C:/REAPER"));
                report.package_operation.items.push(failed_item(
                    PACKAGE_JAWS_SCRIPTS,
                    PackageOperationMessage::VendorInstallerFailed {
                        program: r"C:\cache\jaws-scripts-setup.exe".to_string(),
                        exit_code: 1,
                        meaning: None,
                        log_path: None,
                    },
                ));
                report
            });
            assert!(
                summary.detail_lines.iter().any(|line| line.starts_with(
                    "JAWS-Skripte für REAPER: Das Installationsprogramm wurde mit dem Fehlercode 1 \
                     beendet."
                ) && line
                    .contains("installieren Sie JAWS-Skripte für REAPER")),
                "{:#?}",
                summary.detail_lines
            );
            assert!(
                !summary
                    .detail_lines
                    .iter()
                    .any(|line| line.contains("JAWS-for")),
                "{:#?}",
                summary.detail_lines
            );
        }
    }
}
