use std::path::PathBuf;

use rabbit_core::artifact::{ArtifactDescriptor, ArtifactKind};
use rabbit_core::install::{
    InstallFileAction, InstallFileReport, InstallReport, LeftInPlaceReason, ReaperLanguageReport,
};
use rabbit_core::localization::{DEFAULT_LOCALE, FluentArgument, Localizer, embedded_locales};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::operation::{
    InstallerExitMeaning, ManualInstallInstruction, PackageOperationItem, PackageOperationMessage,
    PackageOperationReport, PackageOperationStatus, PlannedExecutionKind, PlannedExecutionPlan,
    localized_install_problem,
};
use rabbit_core::package::{
    PACKAGE_APP2CLAP, PACKAGE_FFMPEG, PACKAGE_JAWS_SCRIPTS, PACKAGE_OSARA, PACKAGE_REAPACK,
    PACKAGE_SURGE_XT,
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

/// An update's backup set holds the replaced files and no copy of RABBIT's
/// records, so the summary names the backup files and the set's manifest,
/// and no receipt backup.
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
                receipt_backup_path: None,
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
                    warning: None,
                    left_reason: None,
                }],
                reaper_language: Vec::new(),
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
            .any(|line| line.contains("Backup manifest:"))
    );
    assert_no_receipt_backup_line(&summary);
}

/// No line of `summary` names a receipt backup, in English or German.
fn assert_no_receipt_backup_line(summary: &WizardInstallSummary) {
    for locale in [DEFAULT_LOCALE, "de-DE"] {
        let localizer = Localizer::embedded(locale).unwrap();
        let line = localizer
            .format("wizard-summary-receipt-backup", &[("path", "")])
            .value;
        let label = line.trim_end();
        assert!(
            !summary.detail_lines.iter().any(|line| line.contains(label)),
            "{locale}: {:#?}",
            summary.detail_lines
        );
    }
}

/// Vendor installers no longer leave a backup of RABBIT's records, so after
/// them the summary has no receipt backup line. A key map OSARA's replaced
/// is still named, with its set's manifest; a run that backed up nothing
/// names no backup at all.
#[test]
fn setup_summary_names_no_receipt_backup_after_vendor_installers() {
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
    let set = "C:/PortableREAPER/RABBIT/backups/unattended-unix-1-0";
    for keymap_backed_up in [true, false] {
        let mut report = empty_setup_report(PathBuf::from("C:/PortableREAPER"));
        report.dry_run = false;
        report.package_operation.dry_run = false;
        report.package_operation.items.push(PackageOperationItem {
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
            backup_paths: if keymap_backed_up {
                vec![PathBuf::from(format!("{set}/reaper-kb.ini"))]
            } else {
                Vec::new()
            },
            backup_manifest_path: keymap_backed_up
                .then(|| PathBuf::from(format!("{set}/backup-manifest.json"))),
            planned_execution: None,
            manual_instruction: None,
            message: "RABBIT ran the upstream installer unattended, verified the expected target paths, and updated the RABBIT receipt.".to_string(),
            message_code: rabbit_core::operation::PackageOperationMessage::UnattendedInstalled,
        });

        let summary = summarize_setup_report(&model, &report);

        for label in ["Backup file:", "Backup manifest:", "Backup files created:"] {
            assert_eq!(
                summary.detail_lines.iter().any(|line| line.contains(label)),
                keymap_backed_up,
                "{label}: {:#?}",
                summary.detail_lines
            );
        }
        assert_no_receipt_backup_line(&summary);
    }
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

/// Each reason a file of an earlier version can stay where it is, and the
/// Show details line that says it.
const LEFT_IN_PLACE: [(LeftInPlaceReason, &str); 7] = [
    (
        LeftInPlaceReason::InUse,
        "wizard-summary-left-in-place-in-use",
    ),
    (LeftInPlaceReason::Link, "wizard-summary-left-in-place-link"),
    (
        LeftInPlaceReason::Folder,
        "wizard-summary-left-in-place-folder",
    ),
    (
        LeftInPlaceReason::Keymap,
        "wizard-summary-left-in-place-keymap",
    ),
    (
        LeftInPlaceReason::Refused,
        "wizard-summary-left-in-place-refused",
    ),
    (
        LeftInPlaceReason::Unreadable,
        "wizard-summary-left-in-place-unreadable",
    ),
    (
        LeftInPlaceReason::Other,
        "wizard-summary-left-in-place-other",
    ),
];

/// A file of ReaPack's earlier version at `name` in the plug-ins folder,
/// which the update moved into its backup, or, with a `reason`, left where
/// it was.
fn superseded(name: &str, reason: Option<LeftInPlaceReason>) -> InstallFileReport {
    let place = PathBuf::from(format!("C:/PortableREAPER/UserPlugins/{name}"));
    InstallFileReport {
        package_id: PACKAGE_REAPACK.to_string(),
        source_path: place.clone(),
        target_path: place,
        backup_path: reason.is_none().then(|| {
            PathBuf::from(format!(
                "C:/PortableREAPER/RABBIT/backups/unix-1/UserPlugins/{name}"
            ))
        }),
        action: if reason.is_some() {
            InstallFileAction::LeftInPlace
        } else {
            InstallFileAction::MovedToBackup
        },
        size: 3,
        sha256: String::new(),
        warning: reason.map(|_| "The core's English warning.".to_string()),
        left_reason: reason,
    }
}

/// A file of ReaPack's earlier version at `name` that the update left
/// where it was, as a report from before the reason was recorded says it:
/// with the core's English warning and no reason.
fn superseded_without_reason(name: &str) -> InstallFileReport {
    InstallFileReport {
        left_reason: None,
        ..superseded(name, Some(LeftInPlaceReason::Other))
    }
}

/// A finished update of ReaPack whose report holds `actions`, with ReaPack's
/// item as the run reports it.
fn update_report(actions: Vec<InstallFileReport>) -> SetupReport {
    let mut report = empty_setup_report(PathBuf::from("C:/PortableREAPER"));
    report.dry_run = false;
    report.package_operation.dry_run = false;
    report.package_operation.install_report = Some(InstallReport {
        resource_path: PathBuf::from("C:/PortableREAPER"),
        dry_run: false,
        preflight: PreflightReport {
            passed: true,
            checks: Vec::new(),
        },
        receipt_written: true,
        receipt_backup_path: None,
        backup_manifest_path: Some(PathBuf::from(
            "C:/PortableREAPER/RABBIT/backups/unix-1/backup-manifest.json",
        )),
        actions,
        reaper_language: Vec::new(),
    });
    report
        .package_operation
        .items
        .push(updated_item(PACKAGE_REAPACK, "1.2.6"));
    report
}

/// The item of a package the run updated to `version` with RABBIT's own
/// installer.
fn updated_item(package_id: &str, version: &str) -> PackageOperationItem {
    PackageOperationItem {
        plan_action: PlanActionKind::Update,
        status: PackageOperationStatus::InstalledOrChecked,
        artifact: ArtifactDescriptor {
            version: Version::parse(version).unwrap(),
            kind: ArtifactKind::ExtensionBinary,
            ..failed_item(
                package_id,
                PackageOperationMessage::ExtensionBinaryInstalled,
            )
            .artifact
        },
        message: "Single extension binary handled by RABBIT installer.".to_string(),
        ..failed_item(
            package_id,
            PackageOperationMessage::ExtensionBinaryInstalled,
        )
    }
}

/// The Spanish language pack.
const LANGPACK_ES: &str = "langpack-es";

/// Show details' lines of a package: from the line that starts with
/// `first`, its name and a colon, up to the next that starts with one of
/// `others`, another package's, or the end.
fn lines_of_package<'a>(lines: &'a [String], first: &str, others: &[&str]) -> &'a [String] {
    let start = lines
        .iter()
        .position(|line| line.starts_with(first))
        .unwrap_or_else(|| panic!("{first:?} not in {lines:#?}"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| others.iter().any(|other| line.starts_with(other)))
        .map_or(lines.len(), |end| start + 1 + end);
    &lines[start..end]
}

/// After an update, Show details names each file of an earlier version that
/// moved into the backup, and each that stayed where it was, with why in
/// the user's words, never the core's English warning; a report from
/// before the reason was recorded reads as another reason. Done adds what
/// that means: how many stayed, that RABBIT no longer keeps track of them
/// and can't restore those versions. In every language, for every reason,
/// and only after a real run.
#[test]
fn files_an_update_left_in_place_are_told_on_done_and_in_show_details_in_every_locale() {
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
    let request = sample_install_request(PathBuf::from("C:/PortableREAPER"));
    let mut actions = vec![superseded("reaper_reapack64.dll", None)];
    for (index, (reason, _)) in LEFT_IN_PLACE.iter().enumerate() {
        actions.push(superseded(&format!("left{index}.dll"), Some(*reason)));
    }
    actions.push(superseded_without_reason("older.dll"));
    let all = update_report(actions);

    let summary = summarize_setup_report(&model, &all);
    // After ReaPack's own lines, so each says whose version it was.
    let reapack = lines_of_package(&summary.detail_lines, "ReaPack: ", &[]);
    for expected in [
        "Moved into the backup, because the new version no longer installs it: \
         C:/PortableREAPER/UserPlugins/reaper_reapack64.dll",
        "Stays where it is, because another program has it open: \
         C:/PortableREAPER/UserPlugins/left0.dll",
        "Stays where it is, because no restore could put it back there: \
         C:/PortableREAPER/UserPlugins/left4.dll",
        "Stays where it is, because RABBIT couldn't move it into the backup: \
         C:/PortableREAPER/UserPlugins/left6.dll",
        "Stays where it is, because RABBIT couldn't move it into the backup: \
         C:/PortableREAPER/UserPlugins/older.dll",
    ] {
        assert!(
            reapack.iter().any(|line| line == expected),
            "{expected:?} not in {:#?}",
            summary.detail_lines
        );
    }
    let outcome = wizard_outcome_report_from_success(&model, &request, &all);
    assert!(
        outcome.status_line.ends_with(
            "\n\n8 files of earlier versions stayed where they were. RABBIT no longer keeps \
             track of them and can't restore those versions. Show details says which and why."
        ),
        "{}",
        outcome.status_line
    );
    let one = update_report(vec![superseded("left.dll", Some(LeftInPlaceReason::InUse))]);
    assert!(
        wizard_outcome_report_from_success(&model, &request, &one)
            .status_line
            .ends_with(
                "\n\nOne file of an earlier version stayed where it was. RABBIT no longer \
                 keeps track of it and can't restore that version. Show details says which and \
                 why."
            )
    );
    // Nothing stayed, or a dry run, which moved nothing: no sentence.
    let moved = update_report(vec![superseded("reaper_reapack64.dll", None)]);
    let mut dry = one.clone();
    dry.dry_run = true;
    for report in [moved, dry] {
        assert!(
            !wizard_outcome_report_from_success(&model, &request, &report)
                .status_line
                .contains("Show details"),
        );
    }

    for &locale in embedded_locales() {
        model.bootstrap_options.locale = locale.to_string();
        let localizer = Localizer::embedded(locale).unwrap();
        let summary = summarize_setup_report(&model, &all);
        for (index, (_, key)) in LEFT_IN_PLACE.iter().enumerate() {
            let path = format!("C:/PortableREAPER/UserPlugins/left{index}.dll");
            let line = localizer.format(key, &[("path", path.as_str())]).value;
            assert!(
                summary.detail_lines.contains(&line),
                "{locale}: {line:?} not in {:#?}",
                summary.detail_lines
            );
            assert_reads(locale, &line);
        }
        let older = localizer
            .format(
                "wizard-summary-left-in-place-other",
                &[("path", "C:/PortableREAPER/UserPlugins/older.dll")],
            )
            .value;
        assert!(summary.detail_lines.contains(&older), "{locale}");
        let moved = localizer
            .format(
                "wizard-summary-moved-to-backup",
                &[("path", "C:/PortableREAPER/UserPlugins/reaper_reapack64.dll")],
            )
            .value;
        assert!(summary.detail_lines.contains(&moved), "{locale}");
        assert_reads(locale, &moved);
        assert!(
            !summary
                .detail_lines
                .iter()
                .any(|line| line.contains("The core's English warning")),
            "{locale}"
        );
        for (report, count) in [(&all, 8_usize), (&one, 1)] {
            let status = wizard_outcome_report_from_success(&model, &request, report).status_line;
            let hint = localizer
                .format_with(
                    "wizard-summary-left-in-place-hint",
                    &[("count", FluentArgument::from(count))],
                )
                .value;
            assert!(
                status.ends_with(&format!("\n\n{hint}")),
                "{locale}: {status}"
            );
            assert_reads(locale, &hint);
        }
    }
}

/// Show details puts each file an update moved or left under the package
/// whose earlier version it was, after that package's status, never under
/// another package's or above them all, so a screen reader names the
/// package first.
#[test]
fn files_an_update_moved_are_told_under_their_own_package() {
    let english = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = model_from_plan(
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
    let ffmpeg_dll = |name: &str| InstallFileReport {
        package_id: PACKAGE_FFMPEG.to_string(),
        ..superseded(name, Some(LeftInPlaceReason::InUse))
    };
    let mut report = update_report(vec![
        ffmpeg_dll("avcodec-61.dll"),
        superseded("reaper_reapack64.dll", None),
    ]);
    report
        .package_operation
        .items
        .insert(0, updated_item(PACKAGE_FFMPEG, "8.0"));

    let lines = summarize_setup_report(&model, &report).detail_lines;

    let ffmpeg = lines_of_package(
        &lines,
        "FFmpeg for Improved Video Support: ",
        &["ReaPack: "],
    );
    assert!(
        ffmpeg.contains(
            &"Stays where it is, because another program has it open: \
              C:/PortableREAPER/UserPlugins/avcodec-61.dll"
                .to_string()
        ),
        "{lines:#?}"
    );
    assert!(
        !ffmpeg
            .iter()
            .any(|line| line.contains("reaper_reapack64.dll")),
        "{lines:#?}"
    );
    let reapack = lines_of_package(
        &lines,
        "ReaPack: ",
        &["FFmpeg for Improved Video Support: "],
    );
    assert!(
        reapack.contains(
            &"Moved into the backup, because the new version no longer installs it: \
              C:/PortableREAPER/UserPlugins/reaper_reapack64.dll"
                .to_string()
        ),
        "{lines:#?}"
    );
    assert!(
        !reapack.iter().any(|line| line.contains("avcodec-61.dll")),
        "{lines:#?}"
    );
    // Each once.
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("avcodec-61.dll"))
            .count(),
        1,
        "{lines:#?}"
    );
}

/// An update that moved a file into the backup from outside REAPER's own
/// folder, such as an old app2clap from the CLAP folder, says right after
/// that file's line that portable REAPERs and other programs that loaded it
/// from there no longer find it. A file moved from inside REAPER's folder
/// gets no such line. In every language.
#[test]
fn a_file_moved_out_of_the_clap_folder_says_other_programs_lose_it() {
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
    let clap = "C:/Users/Someone/AppData/Local/Programs/Common/CLAP/app2clap.clap";
    let mut report = update_report(vec![
        InstallFileReport {
            package_id: PACKAGE_APP2CLAP.to_string(),
            source_path: PathBuf::from(clap),
            target_path: PathBuf::from(clap),
            backup_path: Some(PathBuf::from(
                "C:/PortableREAPER/RABBIT/backups/unix-1/external/1/app2clap.clap",
            )),
            ..superseded("app2clap.clap", None)
        },
        superseded("reaper_reapack64.dll", None),
    ]);
    report
        .package_operation
        .items
        .insert(0, updated_item(PACKAGE_APP2CLAP, "2026.5.17.34"));

    let lines = summarize_setup_report(&model, &report).detail_lines;
    let moved =
        format!("Moved into the backup, because the new version no longer installs it: {clap}");
    let at = lines
        .iter()
        .position(|line| *line == moved)
        .unwrap_or_else(|| panic!("{moved:?} not in {lines:#?}"));
    assert_eq!(
        lines[at + 1],
        format!(
            "Portable REAPERs and other programs that loaded it from there no longer find it: \
             {clap}"
        )
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("Portable REAPERs"))
            .count(),
        1,
        "{lines:#?}"
    );

    for &locale in embedded_locales() {
        model.bootstrap_options.locale = locale.to_string();
        let localizer = Localizer::embedded(locale).unwrap();
        let lines = summarize_setup_report(&model, &report).detail_lines;
        let moved = localizer
            .format("wizard-summary-moved-to-backup", &[("path", clap)])
            .value;
        let shared = localizer
            .format("wizard-summary-moved-from-shared-folder", &[("path", clap)])
            .value;
        assert_reads(locale, &shared);
        let at = lines.iter().position(|line| *line == moved).unwrap();
        assert_eq!(lines[at + 1], shared, "{locale}");
        assert_eq!(
            lines.iter().filter(|line| **line == shared).count(),
            1,
            "{locale}"
        );
    }
}

/// After an update moved the language pack file REAPER's language named
/// into the backup, Show details says under the pack what became of the
/// language: set to the new file, still the old one while REAPER may be
/// open, or not set. In every language, and not after a dry run, which
/// set nothing.
#[test]
fn what_became_of_reapers_language_is_told_under_the_language_pack_in_every_locale() {
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
    let set = ReaperLanguageReport {
        package_id: LANGPACK_ES.to_string(),
        langpack: "es_MX.ReaperLangPack".to_string(),
        langpack_before: Some("es_ES.ReaperLangPack".to_string()),
        switched: true,
        error: None,
        reaper_may_run: false,
    };
    let reaper_open = ReaperLanguageReport {
        switched: false,
        reaper_may_run: true,
        ..set.clone()
    };
    let failed = ReaperLanguageReport {
        switched: false,
        error: Some("access denied".to_string()),
        ..set.clone()
    };
    let switched = |language: &ReaperLanguageReport| {
        let mut report = update_report(Vec::new());
        report.package_operation.items = vec![updated_item(LANGPACK_ES, "1.0")];
        let install = report.package_operation.install_report.as_mut().unwrap();
        install.actions = vec![InstallFileReport {
            package_id: LANGPACK_ES.to_string(),
            ..superseded("es_ES.ReaperLangPack", None)
        }];
        install.reaper_language = vec![language.clone()];
        report
    };
    let cases = [
        (
            &set,
            "wizard-summary-language-switched",
            "REAPER's language now uses es_MX.ReaperLangPack, because es_ES.ReaperLangPack \
             moved into the backup.",
        ),
        (
            &reaper_open,
            "wizard-summary-language-reaper-may-run",
            "REAPER's language still names es_ES.ReaperLangPack, which moved into the backup, \
             because REAPER may be open. Once REAPER is closed, choose es_MX.ReaperLangPack in \
             REAPER's preferences.",
        ),
        (
            &failed,
            "wizard-summary-language-failed",
            "RABBIT couldn't set REAPER's language to es_MX.ReaperLangPack. Choose it in \
             REAPER's preferences.",
        ),
    ];
    for (language, _, expected) in cases {
        let lines = summarize_setup_report(&model, &switched(language)).detail_lines;
        let pack = lines_of_package(&lines, "REAPER in Spanish: ", &[]);
        assert_eq!(
            pack.last().map(String::as_str),
            Some(expected),
            "{lines:#?}"
        );
        assert!(
            !lines.iter().any(|line| line.contains("access denied")),
            "the core's English error isn't shown: {lines:#?}"
        );
        let mut dry = switched(language);
        dry.dry_run = true;
        assert!(
            !summarize_setup_report(&model, &dry)
                .detail_lines
                .iter()
                .any(|line| line.contains("REAPER's")),
        );
    }

    for &locale in embedded_locales() {
        model.bootstrap_options.locale = locale.to_string();
        let localizer = Localizer::embedded(locale).unwrap();
        for (language, key, _) in cases {
            let line = localizer
                .format(
                    key,
                    &[
                        ("file", "es_MX.ReaperLangPack"),
                        ("before", "es_ES.ReaperLangPack"),
                    ],
                )
                .value;
            let lines = summarize_setup_report(&model, &switched(language)).detail_lines;
            assert!(
                lines.contains(&line),
                "{locale}: {line:?} not in {lines:#?}"
            );
            assert_reads(locale, &line);
        }
    }
}
