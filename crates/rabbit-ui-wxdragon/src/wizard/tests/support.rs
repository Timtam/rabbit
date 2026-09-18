//! Fixtures shared by the wizard tests.

use std::path::PathBuf;

use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Confidence, Installation, InstallationKind, Platform};
use rabbit_core::operation::PackageOperationReport;
use rabbit_core::package::{PACKAGE_OSARA, PACKAGE_REAPACK};
use rabbit_core::plan::{InstallPlan, PlanAction, PlanActionKind};
use rabbit_core::preflight::PreflightReport;
use rabbit_core::resource::ResourceInitReport;
use rabbit_core::setup::SetupReport;
use rabbit_core::version::Version;

use crate::wizard::*;

/// Build a one-package model whose single row is the German language
/// pack in the given plan state.
pub(super) fn langpack_model(action: PlanActionKind) -> (Localizer, WizardModel) {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let installation = fake_installation();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![installation.clone()],
        Some(0),
        InstallPlan {
            target: Some(installation),
            actions: vec![PlanAction {
                package_id: "langpack-de".to_string(),
                action,
                installed_version: None,
                available_version: None,
                reason: "test".to_string(),
            }],
            notes: Vec::new(),
        },
    );
    (localizer, model)
}

pub(super) fn language_step(rows: &[ConfigurationRow]) -> &ConfigurationRow {
    rows.iter()
        .find(|row| row.step_id == rabbit_core::configuration::CONFIG_SET_REAPER_LANGUAGE)
        .expect("set-reaper-language row should exist")
}

pub(super) fn sample_apply_report(
    replaced_files: Vec<rabbit_core::self_update::ReplacedFile>,
    signature_verdicts: Vec<rabbit_core::self_update::SignatureVerdictRecord>,
) -> rabbit_core::self_update::SelfUpdateApplyReport {
    use rabbit_core::model::Platform;
    use rabbit_core::self_update::{
        SelfUpdateApplyReport, SelfUpdateAssetSelection, SelfUpdateCheckReport,
        SelfUpdateStageReport,
    };

    let check = SelfUpdateCheckReport {
        manifest_url: "https://example.test/rabbit-update-stable.json".to_string(),
        current_version: Version::parse("0.1.0").unwrap(),
        latest_version: Version::parse("0.2.0").unwrap(),
        channel: "stable".to_string(),
        published_at: "2026-04-25T00:00:00Z".to_string(),
        release_notes_url: None,
        minimum_supported_previous_version: None,
        update_available: true,
        requires_manual_transition: false,
        asset: SelfUpdateAssetSelection {
            platform: Platform::Windows,
            url: "https://example.test/RABBIT-windows.zip".to_string(),
            sha256: "0".repeat(64),
            kind: rabbit_core::self_update::SelfUpdateAssetKind::Binary,
        },
    };
    let stage = SelfUpdateStageReport {
        check,
        staging_dir: PathBuf::from("/staging"),
        staged_asset_path: Some(PathBuf::from("/staging/0.2.0/RABBIT-windows.zip")),
        downloaded: true,
        reused_existing_file: false,
        verified_sha256: Some("0".repeat(64)),
        ready_to_apply: true,
        status_message: "ready".to_string(),
    };
    SelfUpdateApplyReport {
        stage,
        install_root: PathBuf::from("/install"),
        replaced_files,
        skipped_files: Vec::new(),
        signature_verdicts,
        status_message: "applied".to_string(),
    }
}

pub(super) fn fake_installation() -> Installation {
    Installation {
        kind: InstallationKind::Portable,
        platform: Platform::Windows,
        app_path: PathBuf::from("C:/REAPER/reaper.exe"),
        resource_path: PathBuf::from("C:/REAPER"),
        version: Some(Version::parse("7.69").unwrap()),
        architecture: Some(Architecture::X64),
        writable: true,
        confidence: Confidence::High,
        evidence: Vec::new(),
    }
}

pub(super) fn fake_standard_installation() -> Installation {
    Installation {
        kind: InstallationKind::Standard,
        platform: Platform::Windows,
        app_path: PathBuf::from("C:/Program Files/REAPER/reaper.exe"),
        resource_path: PathBuf::from("C:/Users/Test/AppData/Roaming/REAPER"),
        version: Some(Version::parse("7.69").unwrap()),
        architecture: Some(Architecture::X64),
        writable: true,
        confidence: Confidence::High,
        evidence: Vec::new(),
    }
}

pub(super) fn empty_setup_report(resource_path: PathBuf) -> SetupReport {
    SetupReport {
        resource_path: resource_path.clone(),
        dry_run: true,
        resource_init: ResourceInitReport {
            resource_path: resource_path.clone(),
            dry_run: true,
            portable: true,
            preflight: PreflightReport {
                passed: true,
                checks: Vec::new(),
            },
            actions: Vec::new(),
        },
        package_operation: PackageOperationReport {
            resource_path,
            dry_run: true,
            install_report: None,
            receipt_backup_path: None,
            receipt_backup_manifest_path: None,
            items: Vec::new(),
        },
        configuration_steps: Vec::new(),
    }
}

pub(super) fn sample_install_request(resource_path: PathBuf) -> WizardInstallRequest {
    WizardInstallRequest {
        resource_path: resource_path.clone(),
        package_ids: vec![PACKAGE_OSARA.to_string(), PACKAGE_REAPACK.to_string()],
        platform: Platform::Windows,
        architecture: Architecture::X64,
        portable: true,
        target_app_path: Some(resource_path.join("reaper.exe")),
        dry_run: false,
        allow_reaper_running: false,
        stage_unsupported: true,
        osara_keymap_choice: OsaraKeymapChoice::ReplaceCurrent,
        cache_dir: PathBuf::from("C:/cache"),
        force_reinstall_packages: Vec::new(),
        package_variants: Default::default(),
        reaper_language_package: None,
        configuration_step_ids: Vec::new(),
        declined_packages: Vec::new(),
        accepted_packages: Vec::new(),
    }
}
