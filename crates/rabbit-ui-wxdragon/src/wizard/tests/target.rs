use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Confidence, Installation, InstallationKind, Platform};
use rabbit_core::plan::InstallPlan;
use tempfile::tempdir;

use crate::wizard::*;

#[test]
fn builds_custom_portable_target_row() {
    let dir = tempdir().unwrap();
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

    let row = custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true);

    assert!(row.selected);
    assert!(row.portable);
    assert!(row.writable);
    assert!(row.app_path.is_none());
    assert_eq!(
        row.planned_app_path,
        dir.path().join("PortableREAPER").join("reaper.exe")
    );
    assert!(row.label.contains("Portable REAPER folder"));
    assert!(row.details.contains("REAPER application path"));
    assert!(row.details.contains("REAPER version: Unknown version"));
    assert!(!row.details.contains("Architecture"));
    assert!(row.details.contains("Portable resource path"));
}

#[test]
fn custom_portable_target_uses_reaper_exe_when_present() {
    let dir = tempdir().unwrap();
    let resource_path = dir.path().join("PortableREAPER");
    std::fs::create_dir_all(&resource_path).unwrap();
    std::fs::write(resource_path.join("reaper.exe"), b"").unwrap();
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

    let row = custom_portable_target_row(&model, resource_path.clone(), true);

    assert_eq!(row.app_path, Some(resource_path.join("reaper.exe")));
    assert_eq!(row.planned_app_path, resource_path.join("reaper.exe"));
}

#[test]
fn refreshed_standard_target_row_detects_app_that_appeared_after_startup() {
    let dir = tempdir().unwrap();
    let resource_path = dir.path().join("REAPER");
    let app_path = dir
        .path()
        .join("Program Files")
        .join("REAPER")
        .join("reaper.exe");
    std::fs::create_dir_all(&resource_path).unwrap();
    std::fs::create_dir_all(app_path.parent().unwrap()).unwrap();

    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let installation = Installation {
        kind: InstallationKind::Standard,
        platform: Platform::Windows,
        app_path: app_path.clone(),
        resource_path: resource_path.clone(),
        version: None,
        architecture: Some(Architecture::X64),
        writable: true,
        confidence: Confidence::Low,
        evidence: Vec::new(),
    };
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

    assert!(model.target_rows[0].app_path.is_none());

    std::fs::write(&app_path, b"").unwrap();

    let refreshed = refreshed_target_row(&model, &model.target_rows[0]);

    assert_eq!(refreshed.app_path, Some(app_path.clone()));
    assert_eq!(refreshed.planned_app_path, app_path);
    assert!(
        refreshed
            .details
            .contains(&resource_path.display().to_string())
    );
}
