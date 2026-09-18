use std::path::PathBuf;

use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::package::{PACKAGE_OSARA, PACKAGE_REAPACK};
use rabbit_core::plan::{InstallPlan, PlanAction, PlanActionKind};
use rabbit_core::version::Version;

use super::support::*;
use crate::wizard::*;

#[test]
fn builds_install_request_from_selected_rows() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let installation = fake_installation();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![installation],
        Some(0),
        InstallPlan {
            target: None,
            actions: vec![
                PlanAction {
                    package_id: PACKAGE_OSARA.to_string(),
                    action: PlanActionKind::Install,
                    installed_version: None,
                    available_version: None,
                    reason: "Missing".to_string(),
                },
                PlanAction {
                    package_id: PACKAGE_REAPACK.to_string(),
                    action: PlanActionKind::Keep,
                    installed_version: None,
                    available_version: None,
                    reason: "Current".to_string(),
                },
            ],
            notes: Vec::new(),
        },
    );

    let request = install_request_from_model(
        &model,
        Some(0),
        &[0],
        WizardInstallOptions {
            dry_run: true,
            allow_reaper_running: true,
            stage_unsupported: false,
            osara_keymap_choice: OsaraKeymapChoice::ReplaceCurrent,
            package_variants: Default::default(),
            reaper_language_package: None,
            cache_dir: Some(PathBuf::from("C:/cache")),
        },
    )
    .unwrap();

    assert_eq!(request.resource_path, PathBuf::from("C:/REAPER"));
    assert_eq!(request.package_ids, vec![PACKAGE_OSARA.to_string()]);
    assert!(request.portable);
    assert_eq!(
        request.target_app_path,
        Some(PathBuf::from("C:/REAPER/reaper.exe"))
    );
    assert!(request.dry_run);
    assert_eq!(
        request.osara_keymap_choice,
        OsaraKeymapChoice::ReplaceCurrent
    );
    assert_eq!(request.cache_dir, PathBuf::from("C:/cache"));
}

#[test]
fn install_request_requires_selected_package() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let installation = fake_installation();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![installation],
        Some(0),
        InstallPlan {
            target: None,
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );

    let error = install_request_from_model(&model, Some(0), &[], WizardInstallOptions::default())
        .unwrap_err();

    assert!(error.to_string().contains("No package"));
}

#[test]
fn review_preview_includes_osara_keymap_choice() {
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
                package_id: PACKAGE_OSARA.to_string(),
                action: PlanActionKind::Install,
                installed_version: None,
                available_version: Some(Version::parse("2026.1").unwrap()),
                reason: "Missing".to_string(),
            }],
            notes: Vec::new(),
        },
    );

    let preview = build_review_preview_for_package_rows(
        &model,
        model.target_rows.first(),
        &[0],
        &model.package_rows,
        &model.notes,
        OsaraKeymapChoice::ReplaceCurrent,
    );

    assert!(preview.lines.iter().any(|line| line == "OSARA key map"));
    assert!(
        preview
            .lines
            .iter()
            .any(|line| { line.contains("Backup your current key map") && line.contains("OSARA") })
    );
    assert!(
        !preview
            .lines
            .iter()
            .any(|line| line == "Manual attention expected")
    );
}

#[test]
fn default_install_options_replace_osara_keymap() {
    assert_eq!(
        WizardInstallOptions::default().osara_keymap_choice,
        OsaraKeymapChoice::ReplaceCurrent
    );
}
