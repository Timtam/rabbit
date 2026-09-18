use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, InstallationKind, Platform};
use rabbit_core::package::{PACKAGE_OSARA, PACKAGE_REAPACK};
use rabbit_core::plan::{InstallPlan, PlanAction, PlanActionKind};
use rabbit_core::version::Version;

use super::support::*;
use crate::wizard::*;

#[test]
fn builds_initial_wizard_model_from_plan() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let installation = fake_installation();
    let plan = InstallPlan {
        target: Some(installation.clone()),
        actions: vec![
            PlanAction {
                package_id: PACKAGE_OSARA.to_string(),
                action: PlanActionKind::Install,
                installed_version: None,
                available_version: Some(Version::parse("2026.1").unwrap()),
                reason: "Missing".to_string(),
            },
            PlanAction {
                package_id: PACKAGE_REAPACK.to_string(),
                action: PlanActionKind::Keep,
                installed_version: Some(Version::parse("1.2.6").unwrap()),
                available_version: Some(Version::parse("1.2.6").unwrap()),
                reason: "Current".to_string(),
            },
        ],
        notes: vec!["Review note".to_string()],
    };

    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![installation],
        Some(0),
        plan,
    );

    assert_eq!(
        model.window_title,
        format!(
            "REAPER Accessibility Bootstrap & Bundle Installation Tool v{}",
            env!("CARGO_PKG_VERSION")
        )
    );
    assert_eq!(model.steps.len(), 7);
    assert_eq!(model.target_rows.len(), 1);
    assert!(model.target_rows[0].selected);
    assert!(model.target_rows[0].portable);
    assert!(
        model.target_rows[0]
            .details
            .contains("REAPER installation path")
    );
    assert!(model.target_rows[0].details.contains("Version:"));
    assert!(!model.target_rows[0].details.contains("Architecture"));
    assert!(
        !model.target_rows[0]
            .details
            .contains("Detection confidence")
    );
    assert!(model.target_rows[0].details.contains("Writable"));
    assert_eq!(model.package_rows.len(), 2);
    assert_eq!(model.package_rows[0].display_name, "OSARA");
    assert!(model.package_rows[0].summary.contains("OSARA"));
    // The wizard package details pane no longer surfaces the internal
    // handling-kind line; it stays in the saved report instead.
    assert!(!model.package_rows[0].details.contains("Handling:"));
    // The localized package description from the embedded en-US locale
    // should land in `description` and inside `details` so the wizard's
    // package details pane explains what the package is for.
    assert!(
        model.package_rows[0].description.contains("screen readers"),
        "expected OSARA description in row, got {:?}",
        model.package_rows[0].description
    );
    assert!(
        model.package_rows[0]
            .details
            .contains(&model.package_rows[0].description),
        "expected OSARA description embedded in details"
    );
    assert_eq!(model.package_rows[0].action_label, "Will install");
    assert!(!model.package_rows[0].manual_attention_expected);
    assert_eq!(
        model.package_rows[0].handling_summary,
        model.text.package_handling_unattended
    );
    assert!(model.package_rows[0].selected);
    assert_eq!(model.package_rows[1].action_label, "Won't touch");
    assert!(!model.package_rows[1].manual_attention_expected);
    assert!(!model.package_rows[1].selected);
    assert!(model.controls.can_go_next);
    assert!(model.controls.can_install);
    assert!(model.review_lines.iter().any(|line| line.contains("OSARA")));
}

#[test]
fn disables_next_when_no_target_is_selected() {
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

    assert!(!model.controls.can_go_next);
    assert!(!model.controls.can_install);
    assert_eq!(model.review_lines[0], "No target selected.");
}

#[cfg(target_os = "windows")]
#[test]
fn selectable_installations_appends_standard_target_when_missing() {
    let installations = selectable_installations(Platform::Windows, vec![fake_installation()]);

    assert_eq!(installations[0].kind, InstallationKind::Portable);
    assert_eq!(
        installations
            .iter()
            .filter(|installation| installation.kind == InstallationKind::Standard)
            .count(),
        1
    );
}

#[test]
fn selectable_installations_does_not_duplicate_detected_standard_target() {
    let installations = selectable_installations(
        Platform::Windows,
        vec![fake_standard_installation(), fake_installation()],
    );

    assert_eq!(
        installations
            .iter()
            .filter(|installation| installation.kind == InstallationKind::Standard)
            .count(),
        1
    );
}
