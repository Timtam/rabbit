use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::package::{
    HostCapabilities, PACKAGE_FFMPEG, PACKAGE_OSARA, PACKAGE_REAKONTROL, PACKAGE_REAPACK,
    PACKAGE_REAPER, PACKAGE_SWS, builtin_package_specs,
};
use rabbit_core::plan::{AvailablePackage, InstallPlan, PlanAction, PlanActionKind};
use rabbit_core::version::Version;
use tempfile::tempdir;

use super::support::*;
use crate::wizard::text::*;
use crate::wizard::*;

#[test]
fn jaws_scripts_only_appear_when_jaws_is_detected() {
    let with_jaws = wizard_desired_package_ids_for_host(
        Platform::Windows,
        &HostCapabilities {
            jaws_installed: true,
            ..HostCapabilities::default()
        },
    );
    assert!(with_jaws.iter().any(|id| id == "jaws-scripts"));

    let without_jaws = wizard_desired_package_ids_for_host(
        Platform::Windows,
        &HostCapabilities {
            jaws_installed: false,
            ..HostCapabilities::default()
        },
    );
    assert!(!without_jaws.iter().any(|id| id == "jaws-scripts"));

    // macOS never sees the JAWS row regardless of the host flag — the
    // package itself is platform-gated to Windows.
    let macos_with_jaws = wizard_desired_package_ids_for_host(
        Platform::MacOs,
        &HostCapabilities {
            jaws_installed: true,
            ..HostCapabilities::default()
        },
    );
    assert!(!macos_with_jaws.iter().any(|id| id == "jaws-scripts"));
}

#[test]
fn toggling_a_package_row_updates_its_action_label_and_summary() {
    // The package list row label must follow the user's checkbox state:
    // unchecking should switch the visible action to "Keep", and
    // re-checking it should restore the install/update action that the
    // plan originally chose for this package.
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
    let mut row = model.package_rows[0].clone();
    assert_eq!(row.action, PlanActionKind::Install);
    assert_eq!(row.action_label, "Will install");
    assert!(row.selected);

    let summary = apply_checkbox_state_to_package_row(&model, &mut row, false).unwrap();
    assert_eq!(row.action, PlanActionKind::Keep);
    assert_eq!(row.action_label, "Won't touch");
    assert!(!row.selected);
    assert!(summary.contains("Won't touch"));
    assert!(row.summary.contains("Won't touch"));

    // Re-checking restores the original install action because the
    // package was originally not installed.
    let summary = apply_checkbox_state_to_package_row(&model, &mut row, true).unwrap();
    assert_eq!(row.action, PlanActionKind::Install);
    assert_eq!(row.action_label, "Will install");
    assert!(row.selected);
    assert!(summary.contains("Will install"));
}

#[test]
fn toggling_a_keep_row_to_checked_promotes_it_to_update() {
    // For a package that was already installed and current, the plan's
    // original action is Keep. If the user explicitly checks the row,
    // they're asking RABBIT to re-stage the package — that translates to
    // Update so the install pipeline runs.
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
                package_id: PACKAGE_REAPACK.to_string(),
                action: PlanActionKind::Keep,
                installed_version: Some(Version::parse("1.2.6").unwrap()),
                available_version: Some(Version::parse("1.2.6").unwrap()),
                reason: "Current".to_string(),
            }],
            notes: Vec::new(),
        },
    );
    let mut row = model.package_rows[0].clone();
    assert_eq!(row.action, PlanActionKind::Keep);
    assert!(!row.selected);

    let _ = apply_checkbox_state_to_package_row(&model, &mut row, true).unwrap();
    assert_eq!(row.action, PlanActionKind::Update);
    assert_eq!(row.action_label, "Will update");
    assert!(row.selected);
    assert!(row.summary.contains("Will update"));
}

#[test]
fn non_recommended_package_install_row_starts_unticked() {
    // FFmpeg is `recommended: false` in builtin-packages.json. Even when
    // the plan's action for it is Install, the wizard must NOT auto-tick
    // the row — non-recommended packages should be opt-in.
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
                package_id: PACKAGE_FFMPEG.to_string(),
                action: PlanActionKind::Install,
                installed_version: None,
                available_version: Some(Version::parse("8.1.1").unwrap()),
                reason: "Missing".to_string(),
            }],
            notes: Vec::new(),
        },
    );
    let row = &model.package_rows[0];
    // The plan's action stays available on `original_action`; the row's
    // current `action` mirrors the auto-untick (Keep) so the row label
    // reads "Won't touch" instead of "Will install" while unselected.
    assert_eq!(row.original_action, PlanActionKind::Install);
    assert_eq!(row.action, PlanActionKind::Keep);
    assert!(!row.selected);
}

#[test]
fn a_declined_package_does_not_auto_tick_its_updates_either() {
    // Someone who stopped using a language pack and turned it down has
    // said no to the package, not merely to one version of it. Ticking
    // its next update for them would re-offer it every time the
    // translators publish a fix, which is the exact loop the refusal is
    // meant to end. The row is still there to tick by hand.
    let localizer = Localizer::embedded("de-DE").unwrap();
    let text = wizard_text(&localizer);
    let specs = builtin_package_specs(Platform::Windows);
    let declined: std::collections::BTreeSet<String> =
        ["langpack-de".to_string()].into_iter().collect();

    let rows = package_rows(
        &localizer,
        &text,
        Platform::Windows,
        Architecture::X64,
        &specs,
        &[PlanAction {
            package_id: "langpack-de".to_string(),
            action: PlanActionKind::Update,
            installed_version: None,
            available_version: None,
            reason: "upstream file changed".to_string(),
        }],
        &[],
        &HostCapabilities::default(),
        &declined,
    );
    assert!(
        !rows[0].selected,
        "a refused package must not have its update ticked for the user"
    );
    assert!(
        rows[0].available_for_target,
        "the update must still be listed so it can be taken deliberately"
    );
}

#[test]
fn reakontrol_install_row_starts_ticked_when_komplete_kontrol_is_detected() {
    // ReaKontrol's manifest baseline is `recommended: false`, but it
    // declares `recommended_when: komplete_kontrol_installed` so the
    // wizard escalates it to recommended-by-default for users who have
    // Komplete Kontrol on their host. This test pins the host capability
    // explicitly so the result doesn't depend on whether dev/CI has KK.
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let text = wizard_text(&localizer);
    let specs = builtin_package_specs(Platform::Windows);
    let host = HostCapabilities {
        komplete_kontrol_installed: true,
        ..HostCapabilities::default()
    };

    let rows = package_rows(
        &localizer,
        &text,
        Platform::Windows,
        Architecture::X64,
        &specs,
        &[PlanAction {
            package_id: PACKAGE_REAKONTROL.to_string(),
            action: PlanActionKind::Install,
            installed_version: None,
            available_version: Some(Version::parse("2026.2").unwrap()),
            reason: "Missing".to_string(),
        }],
        &[],
        &host,
        &Default::default(),
    );
    assert!(
        rows[0].selected,
        "ReaKontrol Install row must auto-tick on a host where Komplete \
         Kontrol is detected"
    );

    // Sanity: same plan, KK absent → row stays unticked (the manifest
    // baseline of recommended:false wins).
    let rows = package_rows(
        &localizer,
        &text,
        Platform::Windows,
        Architecture::X64,
        &specs,
        &[PlanAction {
            package_id: PACKAGE_REAKONTROL.to_string(),
            action: PlanActionKind::Install,
            installed_version: None,
            available_version: Some(Version::parse("2026.2").unwrap()),
            reason: "Missing".to_string(),
        }],
        &[],
        &HostCapabilities::default(),
        &Default::default(),
    );
    assert!(
        !rows[0].selected,
        "ReaKontrol Install row must stay unticked on a host without \
         Komplete Kontrol"
    );
}

#[test]
fn non_recommended_package_update_row_starts_ticked() {
    // Update means the package is already on disk — the user opted into
    // having it. RABBIT should keep it current by default, so the Update
    // row stays auto-ticked even for a non-recommended package.
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
                package_id: PACKAGE_FFMPEG.to_string(),
                action: PlanActionKind::Update,
                installed_version: Some(Version::parse("8.0.0").unwrap()),
                available_version: Some(Version::parse("8.1.1").unwrap()),
                reason: "Older version on disk".to_string(),
            }],
            notes: Vec::new(),
        },
    );
    let row = &model.package_rows[0];
    assert_eq!(row.action, PlanActionKind::Update);
    assert!(row.selected);
}

#[test]
fn builds_package_plan_for_custom_target_path() {
    let dir = tempdir().unwrap();
    let plugins = dir.path().join("PortableREAPER").join("UserPlugins");
    std::fs::create_dir_all(&plugins).unwrap();
    std::fs::write(plugins.join("reaper_reapack-x64.dll"), b"installed").unwrap();
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
    let target = custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true);

    let plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();
    let reapack = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_REAPACK)
        .unwrap();

    assert_eq!(reapack.action, PlanActionKind::Keep);
    assert!(!reapack.selected);
    assert!(plan.package_rows.iter().any(|row| row.selected));
}

#[test]
fn package_plan_includes_reaper_for_empty_custom_target() {
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
    let target = custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true);

    let plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();
    let reaper = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_REAPER)
        .unwrap();

    assert_eq!(reaper.display_name, "REAPER");
    assert_eq!(reaper.action, PlanActionKind::Install);
    assert!(!reaper.manual_attention_expected);
    // Handling-kind no longer surfaces in the wizard details pane; it
    // remains as a structured field on PackageRow for the saved report.
    assert_eq!(
        reaper.handling_summary,
        model.text.package_handling_unattended
    );
    assert!(reaper.selected);
}

#[test]
fn portable_target_marks_surge_xt_as_unavailable() {
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
    let target = custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true);

    let plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();
    let surge = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == rabbit_core::package::PACKAGE_SURGE_XT)
        .expect("Surge XT row should appear in the package list");

    assert!(
        !surge.available_for_target,
        "Surge XT must be marked unavailable on a portable REAPER target"
    );
    assert!(!surge.selected);
    assert_eq!(surge.action, PlanActionKind::Keep);
    assert!(
        surge.unavailability_reason.is_some(),
        "the gate should attach a localized reason so screen readers announce why"
    );
}

#[test]
fn portable_target_marks_app2clap_as_unavailable() {
    // app2clap installs into the per-user CLAP folder, outside any
    // portable REAPER folder, so it must be gated on portable targets
    // exactly like Surge XT.
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
    let target = custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true);

    let plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();
    let app2clap = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == rabbit_core::package::PACKAGE_APP2CLAP)
        .expect("app2clap row should appear in the package list");

    assert!(
        !app2clap.available_for_target,
        "app2clap must be marked unavailable on a portable REAPER target"
    );
    assert!(!app2clap.selected);
    assert_eq!(app2clap.action, PlanActionKind::Keep);
    assert!(
        app2clap.unavailability_reason.is_some(),
        "the gate should attach a localized reason so screen readers announce why"
    );
}

#[test]
fn version_check_failure_disables_row_and_records_note() {
    // SWS-website-down scenario: the deferred latest-version check fails
    // for one package. Its row must be disabled (unchecked, Keep, with a
    // localized reason) while every other row keeps its update flow, and
    // the full error message must land in the notes for the Review page.
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
    let target = custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true);
    let mut plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();

    let failures = vec![(
        PACKAGE_SWS.to_string(),
        "HTTP status server error (503) for https://sws-extension.org/".to_string(),
    )];
    let can_install = apply_version_check_failures_to_rows(
        &localizer,
        &mut plan.package_rows,
        &mut plan.notes,
        &failures,
    );

    let sws = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_SWS)
        .expect("SWS row should be in the package list");
    assert!(!sws.available_for_target);
    assert!(!sws.selected);
    assert_eq!(sws.action, PlanActionKind::Keep);
    assert!(
        sws.unavailability_reason.is_some(),
        "a localized reason should be attached so screen readers announce why"
    );

    // Other packages keep their flow — the plan as a whole stays
    // actionable (REAPER etc. are still installable on the empty target).
    assert!(can_install);
    assert!(
        plan.package_rows
            .iter()
            .any(|row| row.package_id != PACKAGE_SWS && row.available_for_target)
    );

    // The note carries the package name and the full error text.
    assert!(
        plan.notes
            .iter()
            .any(|note| note.contains("SWS") && note.contains("503")),
        "expected a note with the failure detail, got {:?}",
        plan.notes
    );
}

#[test]
fn whats_new_notes_render_in_the_package_details_pane() {
    // A package whose deferred version check resolved What's-New notes
    // gets them appended to its details pane under a localized heading;
    // packages without notes keep the plain summary + description.
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
    let target = custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true);
    let available = vec![AvailablePackage {
        package_id: PACKAGE_OSARA.to_string(),
        version: Some(Version::parse("2026.8.1.2278,857265da").unwrap()),
        whats_new: Some("• Fix the slider.\n• Logging improvements.".to_string()),
    }];
    let plan =
        wizard_package_plan_for_target_with_available(&model, Some(&target), &available).unwrap();

    let osara = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_OSARA)
        .expect("OSARA row should be in the package list");
    assert!(
        osara.details.contains("What's new in OSARA:"),
        "expected a localized What's-New heading, got {:?}",
        osara.details
    );
    assert!(osara.details.contains("• Fix the slider."));
    // The notes follow the description, they don't replace it.
    assert!(osara.details.contains(&osara.description));

    let reaper = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_REAPER)
        .expect("REAPER row should be in the package list");
    assert!(
        !reaper.details.contains("What's new"),
        "a package without resolved notes must keep its plain details"
    );
}

#[test]
fn reaper_windows_row_uses_unattended_handling() {
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
    let target = custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true);
    let plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();
    let reaper_row = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_REAPER)
        .unwrap();

    assert!(!reaper_row.manual_attention_expected);
    assert_eq!(
        reaper_row.handling_summary,
        model.text.package_handling_unattended
    );
}

#[test]
fn sws_windows_uses_unattended_handling_summary() {
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

    let (handling_summary, manual_attention_expected) = package_handling_summary(
        &model.text,
        PACKAGE_SWS,
        Platform::Windows,
        Architecture::X64,
    );

    assert_eq!(handling_summary, model.text.package_handling_unattended);
    assert!(!manual_attention_expected);
}

#[test]
fn osara_keymap_note_defaults_to_unavailable_when_osara_is_not_selected() {
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

    let note = osara_keymap_note(&model, false, OsaraKeymapChoice::PreserveCurrent);

    assert!(note.contains("Select OSARA"));
}

/// An antivirus block is the one install failure a user can usually
/// clear themselves, so the summary follows the (English, report-bound)
/// error text with localized remediation steps. Everything else keeps
/// exactly one error line.
/// The wizard's Spanish OSARA-translation dropdown maps each position to
/// a manifest variant id. Every position must record an EXPLICIT choice:
/// the install path falls back to the previously-installed variant when
/// none is given, so picking the first entry has to actively mean
/// "REAPER Accesible español" or a Team PMA user could never switch back.
#[test]
fn spanish_variant_dropdown_maps_each_position_to_a_manifest_variant() {
    let specs = rabbit_core::package::package_specs_by_id(Platform::Windows);
    let spanish = specs
        .get(VARIANT_CHOICE_PACKAGE_ID)
        .expect("Spanish language pack");

    // The dropdown offers exactly the variants the manifest declares.
    assert_eq!(VARIANT_CHOICE_IDS.len(), spanish.variants.len());
    for id in VARIANT_CHOICE_IDS {
        assert!(
            spanish.variants.iter().any(|v| v.id == *id),
            "dropdown id {id:?} must exist in the manifest"
        );
    }
    // Position 0 is the manifest's default, so an untouched dropdown
    // agrees with what a CLI run with no flag would install.
    assert!(
        spanish
            .variants
            .iter()
            .any(|v| v.default && v.id == VARIANT_CHOICE_IDS[0]),
        "the first entry must be the manifest default"
    );

    for (index, expected) in VARIANT_CHOICE_IDS.iter().enumerate() {
        let chosen = package_variants_from_choice(Some(index as u32));
        assert_eq!(
            chosen.get(VARIANT_CHOICE_PACKAGE_ID).map(String::as_str),
            Some(*expected),
            "position {index} should select {expected:?}"
        );
    }
    // An out-of-range selection falls back to the default rather than
    // silently recording no choice.
    let fallback = package_variants_from_choice(Some(99));
    assert_eq!(
        fallback.get(VARIANT_CHOICE_PACKAGE_ID).map(String::as_str),
        Some(VARIANT_CHOICE_IDS[0])
    );
}
