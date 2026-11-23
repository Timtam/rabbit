use rabbit_core::localization::{DEFAULT_LOCALE, Localizer, embedded_locales};
use rabbit_core::model::{Architecture, LeftToReapack, Platform};
use rabbit_core::package::{
    HostCapabilities, PACKAGE_APP2CLAP, PACKAGE_FFMPEG, PACKAGE_LB_303, PACKAGE_LJUNO_116,
    PACKAGE_LR_608, PACKAGE_LREAPARAMS, PACKAGE_LSAMPLER_24, PACKAGE_OSARA, PACKAGE_REAKONTROL,
    PACKAGE_REAPACK, PACKAGE_REAPER, PACKAGE_SURGE_XT, PACKAGE_SWS, PackageCategory,
    builtin_package_specs,
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
                left_to_reapack: None,
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
                left_to_reapack: None,
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
                left_to_reapack: None,
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

/// The wizard model for a plan whose only action is LReaParams 0.0.70,
/// with 0.0.75 out, kept because of `left`.
fn lreaparams_left_to_reapack(localizer: &Localizer, left: LeftToReapack) -> WizardModel {
    lreaparams_left_to_reapack_on(localizer, left, Architecture::X64)
}

/// [`lreaparams_left_to_reapack`] for a REAPER of `architecture`.
fn lreaparams_left_to_reapack_on(
    localizer: &Localizer,
    left: LeftToReapack,
    architecture: Architecture,
) -> WizardModel {
    let installation = rabbit_core::model::Installation {
        architecture: Some(architecture),
        ..fake_installation()
    };
    model_from_plan(
        localizer,
        Platform::Windows,
        architecture,
        vec![installation.clone()],
        Some(0),
        InstallPlan {
            target: Some(installation),
            actions: vec![PlanAction {
                package_id: PACKAGE_LREAPARAMS.to_string(),
                action: PlanActionKind::Keep,
                installed_version: Some(Version::parse("0.0.70").unwrap()),
                available_version: Some(Version::parse("0.0.75").unwrap()),
                reason: "ReaPack looks after it".to_string(),
                left_to_reapack: Some(left),
            }],
            notes: Vec::new(),
        },
    )
}

/// A package ReaPack installed and keeps up to date can't be ticked, even
/// with a newer release out: its row says ReaPack manages it, and its
/// details say to update or remove it in ReaPack. So does a package whose
/// ReaPack records RABBIT couldn't read, in its own words.
#[test]
fn a_package_reapack_looks_after_cant_be_ticked_and_says_why() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = lreaparams_left_to_reapack(
        &localizer,
        LeftToReapack::Managed {
            reapack_package: "LReaParams.ext".to_string(),
        },
    );
    let row = &model.package_rows[0];
    assert!(!row.available_for_target);
    assert!(!row.selected && !row.default_selected);
    assert_eq!(row.action, PlanActionKind::Keep);
    assert_eq!(row.category, PackageCategory::Tools);
    assert!(!model.controls.can_install);
    assert!(
        row.summary.starts_with("LReaParams")
            && row.summary.ends_with("(not available: managed by ReaPack)"),
        "{}",
        row.summary
    );
    assert_eq!(
        row.unavailability_reason.as_deref(),
        Some("managed by ReaPack")
    );
    // The ReaPack sentence comes once, before the description, which
    // doesn't repeat it.
    assert_eq!(
        row.details,
        format!(
            "{}\n\nReaPack's records say it installed LReaParams as LReaParams.ext, so RABBIT \
             leaves it to ReaPack. Install, update or remove it in ReaPack.\n\nLReaParams is an \
             open source extension by Lo-lo that gives you accessible keyboard control of FX \
             and track parameters in REAPER, and it works alongside OSARA. Windows only.",
            row.summary
        )
    );

    let model = lreaparams_left_to_reapack(
        &localizer,
        LeftToReapack::RecordsUnreadable {
            error: "file is not a database".to_string(),
        },
    );
    let row = &model.package_rows[0];
    assert!(!row.available_for_target && !row.selected);
    assert!(
        row.summary
            .ends_with("(not available: ReaPack's records couldn't be checked)"),
        "{}",
        row.summary
    );
    assert!(
        row.details.contains(
            "RABBIT couldn't read ReaPack's records (file is not a database), so it can't \
             tell whether ReaPack installed LReaParams, and doesn't install, update or remove \
             it."
        ),
        "{}",
        row.details
    );

    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for left in [
            LeftToReapack::Managed {
                reapack_package: "LReaParams.ext".to_string(),
            },
            LeftToReapack::RecordsUnreadable {
                error: "file is not a database".to_string(),
            },
        ] {
            let model = lreaparams_left_to_reapack(&localizer, left);
            let row = &model.package_rows[0];
            assert!(!row.available_for_target, "{locale}");
            assert_reads(locale, &row.summary);
            assert_reads(locale, &row.details);
            assert!(
                !row.details.contains("package-"),
                "{locale}: {}",
                row.details
            );
        }
    }
}

/// Without ReaPack's say, an LReaParams with a newer release out is
/// offered as an update like any package.
#[test]
fn lreaparams_not_left_to_reapack_is_offered_as_usual() {
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
                package_id: PACKAGE_LREAPARAMS.to_string(),
                action: PlanActionKind::Update,
                installed_version: Some(Version::parse("0.0.70").unwrap()),
                available_version: Some(Version::parse("0.0.75").unwrap()),
                reason: "Newer".to_string(),
                left_to_reapack: None,
            }],
            notes: Vec::new(),
        },
    );
    let row = &model.package_rows[0];
    assert!(row.available_for_target && row.selected);
    assert!(!row.left_to_reapack);
    assert_eq!(row.action, PlanActionKind::Update);
    assert!(!row.summary.contains("ReaPack"), "{}", row.summary);
}

/// A failed version check leaves a row ReaPack looks after as it was: its
/// label keeps saying ReaPack manages it, as its details do, and Review
/// gets no note that installing it is disabled for this run, since RABBIT
/// wouldn't install or update it in any run. Another package's failure
/// still counts as usual.
#[test]
fn a_failed_version_check_leaves_a_row_reapack_looks_after_as_it_was() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let mut model = lreaparams_left_to_reapack(
        &localizer,
        LeftToReapack::Managed {
            reapack_package: "LReaParams.ext".to_string(),
        },
    );
    assert!(model.package_rows[0].left_to_reapack);
    let before = model.package_rows.clone();
    let mut notes = Vec::new();

    let can_install = apply_version_check_failures_to_rows(
        &localizer,
        &mut model.package_rows,
        &mut notes,
        &[
            (
                PACKAGE_LREAPARAMS.to_string(),
                "GitHub's rate limit was reached".to_string(),
            ),
            (PACKAGE_SWS.to_string(), "the SWS site is down".to_string()),
        ],
    );

    assert!(!can_install);
    assert_eq!(model.package_rows, before);
    assert!(
        model.package_rows[0]
            .summary
            .ends_with("(not available: managed by ReaPack)")
    );
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("the SWS site is down"), "{notes:?}");
}

#[test]
fn a_declined_language_pack_does_not_auto_tick_its_updates_either() {
    // Someone who stopped using a language pack and turned it down has
    // said no to the package, not merely to one version of it. Ticking
    // its next update for them would re-offer it every time the
    // translators publish a fix, which is the exact loop the refusal is
    // meant to end. The row is still there to tick by hand.
    let localizer = Localizer::embedded("de-DE").unwrap();
    let text = wizard_text(&localizer);
    let specs = builtin_package_specs(Platform::Windows);
    let rows = |declined: &std::collections::BTreeSet<String>| {
        package_rows(
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
                left_to_reapack: None,
            }],
            &[],
            &HostCapabilities::default(),
            declined,
            &Default::default(),
        )
    };

    // Without a refusal, a language pack's update is ticked like any other.
    let rows_without_refusal = rows(&Default::default());
    assert!(rows_without_refusal[0].selected, "{rows_without_refusal:?}");
    assert_eq!(rows_without_refusal[0].action, PlanActionKind::Update);

    let rows = rows(&["langpack-de".to_string()].into_iter().collect());
    assert!(
        !rows[0].selected,
        "a refused language pack must not have its update ticked for the user"
    );
    assert!(
        rows[0].available_for_target,
        "the update must still be listed so it can be taken deliberately"
    );
}

/// One SWS row, planned as `kind`, with `declined` as the refusals the
/// target remembers.
fn sws_row(kind: PlanActionKind, declined: &[&str]) -> PackageRow {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let text = wizard_text(&localizer);
    let specs = builtin_package_specs(Platform::Windows);
    assert!(
        specs
            .iter()
            .any(|spec| spec.id == PACKAGE_SWS && spec.recommended && !spec.remember_opt_out),
        "these tests only mean anything while SWS is recommended and remembers no opt-out"
    );
    let declined: std::collections::BTreeSet<String> =
        declined.iter().map(ToString::to_string).collect();
    package_rows(
        &localizer,
        &text,
        Platform::Windows,
        Architecture::X64,
        &specs,
        &[PlanAction {
            package_id: PACKAGE_SWS.to_string(),
            action: kind,
            installed_version: None,
            available_version: None,
            reason: "test".to_string(),
            left_to_reapack: None,
        }],
        &[],
        &HostCapabilities::default(),
        &declined,
        &Default::default(),
    )
    .remove(0)
}

#[test]
fn a_package_the_user_uninstalled_starts_unticked_while_it_is_gone() {
    // Uninstalling records a "no" for any package, not only the ones that
    // remember an unticked row. A recommended SWS would otherwise come back
    // ticked on the very next run after the user removed it.
    assert!(sws_row(PlanActionKind::Install, &[]).selected);
    let row = sws_row(PlanActionKind::Install, &[PACKAGE_SWS]);
    assert!(
        !row.selected,
        "a removed SWS must not be ticked for the user"
    );
    assert_eq!(row.original_action, PlanActionKind::Install);
    assert!(
        row.available_for_target,
        "it must stay listed so it can be ticked again"
    );
}

#[test]
fn a_package_the_user_uninstalled_and_put_back_gets_its_update_ticked() {
    // An Update row means the package is on disk again: dragged back from
    // the Trash, say, or installed by hand. The removal record doesn't
    // count any more, so its update is ticked as for any package you have.
    // The language packs keep their refusal (see
    // `a_declined_language_pack_does_not_auto_tick_its_updates_either`).
    let row = sws_row(PlanActionKind::Update, &[PACKAGE_SWS]);
    assert!(row.selected, "{row:?}");
    assert_eq!(row.action, PlanActionKind::Update);
    assert!(row.available_for_target, "{row:?}");
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
            left_to_reapack: None,
        }],
        &[],
        &host,
        &Default::default(),
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
            left_to_reapack: None,
        }],
        &[],
        &HostCapabilities::default(),
        &Default::default(),
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
                left_to_reapack: None,
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

/// A portable target in `path` for a 64-bit REAPER. An empty folder would
/// otherwise take the architecture of the computer the tests run on, and
/// the rows would depend on it.
fn x64_portable_target(model: &WizardModel, path: std::path::PathBuf) -> TargetRow {
    TargetRow {
        architecture: Architecture::X64,
        ..custom_portable_target_row(model, path, true)
    }
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
    let target = x64_portable_target(&model, dir.path().join("PortableREAPER"));

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
    assert_eq!(
        surge.unavailability_reason.as_deref(),
        Some("portable REAPER target"),
        "the gate should attach a localized reason so screen readers announce why"
    );
}

/// No run installs Surge XT in a portable REAPER, so a failed version check
/// for it changes nothing there: the row keeps saying it needs a standard
/// installation, and Review gets no note about the check.
#[test]
fn a_failed_version_check_keeps_the_portable_reason() {
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
    let target = x64_portable_target(&model, dir.path().join("PortableREAPER"));
    let mut plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();
    let before = plan.package_rows.clone();
    let mut notes = Vec::new();

    apply_version_check_failures_to_rows(
        &localizer,
        &mut plan.package_rows,
        &mut notes,
        &[(
            PACKAGE_SURGE_XT.to_string(),
            "rate limit exceeded".to_string(),
        )],
    );

    let surge = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_SURGE_XT)
        .expect("Surge XT row should appear in the package list");
    assert!(surge.has_build_for_target);
    assert_eq!(
        surge.unavailability_reason.as_deref(),
        Some("portable REAPER target")
    );
    assert!(
        surge
            .summary
            .ends_with("(not available: portable REAPER target)")
    );
    assert_eq!(plan.package_rows, before);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn portable_target_offers_app2clap() {
    // app2clap installs into REAPER's own UserPlugins\FX folder, which a
    // portable REAPER has too, so unlike Surge XT it stays available
    // there. It is opt-in, so it starts unticked, and can be ticked.
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
    let target = x64_portable_target(&model, dir.path().join("PortableREAPER"));

    let plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();
    let app2clap = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == rabbit_core::package::PACKAGE_APP2CLAP)
        .expect("app2clap row should appear in the package list");

    assert!(
        app2clap.available_for_target,
        "app2clap must stay available on a portable REAPER target"
    );
    assert!(!app2clap.requires_standard_install);
    assert_eq!(app2clap.unavailability_reason, None);
    assert!(!app2clap.summary.contains("portable REAPER target"));
    assert!(!app2clap.selected);
    assert_eq!(app2clap.original_action, PlanActionKind::Install);
}

/// A copy of app2clap in the CLAP folder, which every REAPER on the computer
/// loads, adds a sentence to app2clap's details, right after its summary.
/// On a portable REAPER without app2clap, it says that installing app2clap
/// there too would load two copies, and to update or reinstall it in the
/// standard REAPER first; on one that has its own app2clap already, that it
/// loads both, and how the shared one goes. On the standard REAPER it says
/// that its update or reinstall moves the copy into a backup, and that
/// portable REAPERs and other programs then no longer find it. The row also
/// carries what Review says should it be ticked: on the standard REAPER
/// that the copy moves, on a portable one without app2clap that it will
/// load two copies, and nothing on a portable one with its own. No other row
/// changes, and without a copy nothing does. The plan finds the copy
/// through the wizard's test seam, so the real CLAP folder is never looked
/// at. In every language.
#[test]
fn a_copy_in_the_clap_folder_is_told_in_app2claps_details() {
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
    let target = x64_portable_target(&model, dir.path().join("PortableREAPER"));
    let rows = wizard_package_plan_for_target(&model, Some(&target))
        .unwrap()
        .package_rows;
    assert!(
        rows.iter().all(|row| row.former_copy_review_note.is_none()),
        "without a copy, no row has a note"
    );
    let index = rows
        .iter()
        .position(|row| row.package_id == PACKAGE_APP2CLAP)
        .unwrap();
    let specs = builtin_package_specs(Platform::Windows);
    let copy = std::path::PathBuf::from("C:/CLAP/app2clap.clap");
    let in_clap = move |spec: &rabbit_core::package::PackageSpec| {
        if spec.id == PACKAGE_APP2CLAP {
            vec![copy.clone()]
        } else {
            Vec::new()
        }
    };
    let with_sentence = |row: &PackageRow, sentence: &str| {
        let (summary, rest) = row.details.split_once("\n\n").unwrap();
        format!("{summary}\n\n{sentence}\n\n{rest}")
    };

    // The plan itself finds the copy, and app2clap isn't in this portable
    // REAPER yet.
    {
        let _seam = crate::wizard::packages::former_copies_seam::install(in_clap.clone());
        let planned = wizard_package_plan_for_target(&model, Some(&target))
            .unwrap()
            .package_rows;
        let package = [("package", planned[index].display_name.as_str())];
        let sentence = localizer
            .format("wizard-package-details-clap-copy-portable", &package)
            .value;
        assert_eq!(
            planned[index].details,
            with_sentence(&rows[index], &sentence)
        );
        assert_eq!(
            planned[index].former_copy_review_note,
            Some(
                localizer
                    .format("wizard-review-clap-copy-twice", &package)
                    .value
            )
        );
        for (row, before) in planned.iter().zip(&rows) {
            if row.package_id != PACKAGE_APP2CLAP {
                assert_eq!(row, before);
            }
        }
    }

    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for (portable, action, key, review_key) in [
            (
                true,
                PlanActionKind::Install,
                "wizard-package-details-clap-copy-portable",
                Some("wizard-review-clap-copy-twice"),
            ),
            (
                true,
                PlanActionKind::Update,
                "wizard-package-details-clap-copy-portable-installed",
                None,
            ),
            (
                true,
                PlanActionKind::Keep,
                "wizard-package-details-clap-copy-portable-installed",
                None,
            ),
            (
                false,
                PlanActionKind::Update,
                "wizard-package-details-clap-copy-standard",
                Some("wizard-review-clap-copy-moves"),
            ),
            (
                false,
                PlanActionKind::Keep,
                "wizard-package-details-clap-copy-standard",
                Some("wizard-review-clap-copy-moves"),
            ),
        ] {
            let mut before = rows.clone();
            before[index].original_action = action;
            let mut noted = before.clone();
            crate::wizard::packages::note_copies_in_former_outside_folders(
                &localizer, &mut noted, &specs, portable, &in_clap,
            );
            let app2clap = &noted[index];
            let package = [("package", app2clap.display_name.as_str())];
            let sentence = localizer.format(key, &package);
            assert!(!sentence.missing, "{locale}: {key}");
            assert_reads(locale, &sentence.value);
            assert_eq!(
                app2clap.details,
                with_sentence(&before[index], &sentence.value),
                "{locale}: {key}"
            );
            let review = review_key.map(|review_key| {
                let review = localizer.format(review_key, &package);
                assert!(!review.missing, "{locale}: {review_key}");
                assert_reads(locale, &review.value);
                review.value
            });
            assert_eq!(app2clap.former_copy_review_note, review, "{locale}");
            for (row, before) in noted.iter().zip(&before) {
                if row.package_id != PACKAGE_APP2CLAP {
                    assert_eq!(row, before, "{locale}");
                }
            }
        }
    }
    let english = |key: &str| localizer.format(key, &[("package", "app2clap")]).value;
    assert!(
        english("wizard-package-details-clap-copy-portable")
            .contains("first update or reinstall app2clap in the standard REAPER")
    );
    assert!(
        english("wizard-package-details-clap-copy-portable-installed")
            .starts_with("This REAPER loads two copies of app2clap")
    );

    let mut untouched = rows.clone();
    crate::wizard::packages::note_copies_in_former_outside_folders(
        &localizer,
        &mut untouched,
        &specs,
        true,
        &|_| Vec::new(),
    );
    assert_eq!(untouched, rows);
}

/// Review says what a ticked row does to a copy in the CLAP folder, under
/// its notes, before the plan's own: here, on the standard REAPER, that
/// app2clap's copy moves into the backup. The details pane isn't the one
/// place a screen reader user is sure to hear. Unticked, the row adds
/// nothing.
#[test]
fn review_says_what_a_ticked_row_does_to_a_copy_in_the_clap_folder() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let installation = fake_standard_installation();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![installation.clone()],
        Some(0),
        InstallPlan {
            target: Some(installation),
            actions: vec![PlanAction {
                package_id: PACKAGE_APP2CLAP.to_string(),
                action: PlanActionKind::Update,
                installed_version: None,
                available_version: Some(Version::parse("2026.5.17.34").unwrap()),
                reason: "test".to_string(),
                left_to_reapack: None,
            }],
            notes: Vec::new(),
        },
    );
    let mut rows = model.package_rows.clone();
    let copy = std::path::PathBuf::from("C:/CLAP/app2clap.clap");
    crate::wizard::packages::note_copies_in_former_outside_folders(
        &localizer,
        &mut rows,
        &builtin_package_specs(Platform::Windows),
        false,
        &|_| vec![copy.clone()],
    );
    let note = localizer
        .format("wizard-review-clap-copy-moves", &[("package", "app2clap")])
        .value;
    let plan_note = "A note from the plan.".to_string();
    let review = |ticked: &[usize]| {
        build_review_preview_for_package_rows(
            &model,
            model.target_rows.first(),
            ticked,
            &rows,
            std::slice::from_ref(&plan_note),
            OsaraKeymapChoice::ReplaceCurrent,
            rabbit_core::osara_keymap::OsaraKeymapState::OwnChanges,
        )
        .lines
    };

    let lines = review(&[0]);
    let heading = lines
        .iter()
        .position(|line| *line == model.text.review_notes_heading)
        .expect("a notes heading");
    assert_eq!(lines[heading + 1..], [note.clone(), plan_note.clone()]);
    assert!(note.starts_with("app2clap: the copy in your CLAP folder moves into the backup"));

    let lines = review(&[]);
    assert!(!lines.contains(&note), "{lines:?}");
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
    let target = x64_portable_target(&model, dir.path().join("PortableREAPER"));
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
    assert_eq!(
        sws.unavailability_reason.as_deref(),
        Some("online version check failed"),
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
        channel: None,
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

    let note = osara_keymap_note(
        &model,
        false,
        OsaraKeymapChoice::PreserveCurrent,
        rabbit_core::osara_keymap::OsaraKeymapState::OwnChanges,
    );

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

/// One REAPER row planned against `available` on `channel`, for a target
/// whose REAPER was last installed from `installed_channel`.
fn reaper_row(
    action: PlanActionKind,
    channel: Option<&str>,
    installed_channel: Option<&str>,
) -> PackageRow {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let text = wizard_text(&localizer);
    let specs = builtin_package_specs(Platform::Windows);
    let installed_channels: rabbit_core::package::PackageChannels = installed_channel
        .map(|channel| {
            [(PACKAGE_REAPER.to_string(), channel.to_string())]
                .into_iter()
                .collect()
        })
        .unwrap_or_default();
    package_rows(
        &localizer,
        &text,
        Platform::Windows,
        Architecture::X64,
        &specs,
        &[PlanAction {
            package_id: PACKAGE_REAPER.to_string(),
            action,
            installed_version: Some(Version::parse("7.80").unwrap()),
            available_version: Some(Version::parse("7.80+dev0918").unwrap()),
            reason: "test".to_string(),
            left_to_reapack: None,
        }],
        &[AvailablePackage {
            package_id: PACKAGE_REAPER.to_string(),
            version: Some(Version::parse("7.80+dev0918").unwrap()),
            whats_new: None,
            channel: channel.map(str::to_string),
        }],
        &HostCapabilities::default(),
        &Default::default(),
        &installed_channels,
    )
    .remove(0)
}

#[test]
fn a_development_build_says_so_in_the_row_itself() {
    let row = reaper_row(PlanActionKind::Update, Some("dev"), None);
    assert!(
        row.summary.contains("(development build)"),
        "{}",
        row.summary
    );
    assert!(row.details.contains("unsupported"), "{}", row.details);
}

#[test]
fn ticking_a_row_keeps_its_channel_tag() {
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
            actions: Vec::new(),
            notes: Vec::new(),
        },
    );
    let mut row = reaper_row(PlanActionKind::Update, Some("dev"), None);
    apply_checkbox_state_to_package_row(&model, &mut row, false).unwrap();
    apply_checkbox_state_to_package_row(&model, &mut row, true).unwrap();
    assert!(
        row.summary.contains("(development build)"),
        "{}",
        row.summary
    );

    // The way back is only announced while the row actually takes it.
    let mut back = reaper_row(PlanActionKind::Update, None, Some("dev"));
    apply_checkbox_state_to_package_row(&model, &mut back, false).unwrap();
    assert!(!back.summary.contains("back to"), "{}", back.summary);
    apply_checkbox_state_to_package_row(&model, &mut back, true).unwrap();
    assert!(back.summary.contains("back to"), "{}", back.summary);
}

#[test]
fn a_row_that_installs_nothing_does_not_promise_an_install() {
    // After installing a development build, the re-plan keeps REAPER.
    // The tag still says which build it is, but "will be installed"
    // would be false.
    let row = reaper_row(PlanActionKind::Keep, Some("dev"), Some("dev"));
    assert!(
        row.summary.contains("(development build)"),
        "{}",
        row.summary
    );
    assert!(
        !row.details.contains("will be installed"),
        "{}",
        row.details
    );
}

#[test]
fn a_pull_request_build_names_its_pull_request() {
    let row = reaper_row(PlanActionKind::Update, Some("pr:1454"), None);
    assert!(row.summary.contains("pull request 1454"), "{}", row.summary);
    assert!(row.details.contains("90 days"), "{}", row.details);
}

#[test]
fn going_back_to_the_regular_release_is_announced() {
    // Leaving expert mode quietly takes a pre-release back to the regular
    // release. The unlock dialog is long gone by then, so the row is the
    // only place that can say why REAPER is being "updated" to an older
    // version.
    let row = reaper_row(PlanActionKind::Update, None, Some("dev"));
    assert!(
        row.summary.contains("back to the regular release"),
        "{}",
        row.summary
    );
    // ...but only when this run actually does something with it.
    let kept = reaper_row(PlanActionKind::Keep, None, Some("dev"));
    assert!(!kept.summary.contains("back to"), "{}", kept.summary);
    // and an ordinary stable row carries no note at all.
    let plain = reaper_row(PlanActionKind::Update, None, None);
    assert!(!plain.summary.contains('('), "{}", plain.summary);
}

#[test]
fn expert_mode_env_values_and_channels_outside_expert_mode() {
    for on in ["1", "true", "YES", " on "] {
        assert!(expert_mode_value_enabled(on), "{on:?}");
    }
    for off in ["", "0", "false", "no", "maybe"] {
        assert!(!expert_mode_value_enabled(off), "{off:?}");
    }
    // Outside expert mode no channel is passed, whatever the choices
    // say - which is what sends pre-release packages back to stable.
    assert!(wizard_channels(false, Some("dev".into()), Some("pr:1".into())).is_empty());
    let on = wizard_channels(true, Some("dev".into()), None);
    assert_eq!(on.get(PACKAGE_REAPER).map(String::as_str), Some("dev"));
    assert!(!on.contains_key(PACKAGE_OSARA));
}

/// The Install tree's package groups, built from the real manifest: Packages,
/// Instruments, Tools, Language packs, in that order, each with its own rows
/// in row order. Surge XT and Lo-lo's four instruments are the instruments,
/// and app2clap and LReaParams the tools.
#[test]
fn the_install_tree_groups_packages_instruments_tools_and_language_packs() {
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
    let rows = wizard_package_plan_for_target(&model, Some(&target))
        .unwrap()
        .package_rows;

    let groups = package_tree_groups(rows.iter().map(|row| row.category));
    let categories: Vec<PackageCategory> = groups.iter().map(|group| group.category).collect();
    assert_eq!(
        categories,
        [
            PackageCategory::Core,
            PackageCategory::Instruments,
            PackageCategory::Tools,
            PackageCategory::Language,
        ]
    );
    let labels: Vec<&str> = categories
        .iter()
        .map(|category| model.text.package_group_label(*category))
        .collect();
    assert_eq!(
        labels,
        ["Packages", "Instruments", "Tools", "Language packs"]
    );

    let ids = |group: &PackageTreeGroup| -> Vec<&str> {
        group
            .rows
            .iter()
            .map(|index| rows[*index].package_id.as_str())
            .collect()
    };
    assert_eq!(
        ids(&groups[1]),
        [
            PACKAGE_SURGE_XT,
            PACKAGE_LJUNO_116,
            PACKAGE_LB_303,
            PACKAGE_LR_608,
            PACKAGE_LSAMPLER_24,
        ]
    );
    assert_eq!(ids(&groups[2]), [PACKAGE_APP2CLAP, PACKAGE_LREAPARAMS]);
    assert!(
        ids(&groups[3]).iter().all(|id| id.starts_with("langpack-")),
        "{:?}",
        ids(&groups[3])
    );
    let core = ids(&groups[0]);
    assert_eq!(core.first(), Some(&PACKAGE_REAPER));
    for id in [PACKAGE_OSARA, PACKAGE_SWS, PACKAGE_REAPACK] {
        assert!(core.contains(&id), "{id} in {core:?}");
    }
    // Every row is in exactly one group, and each group keeps row order.
    let mut all: Vec<usize> = groups.iter().flat_map(|group| group.rows.clone()).collect();
    for group in &groups {
        assert!(group.rows.windows(2).all(|pair| pair[0] < pair[1]));
    }
    all.sort_unstable();
    assert_eq!(all, (0..rows.len()).collect::<Vec<_>>());
}

/// A category no row has gets no group at all, so no heading shows empty;
/// the others keep the display order whatever order the rows come in.
#[test]
fn a_category_without_rows_gets_no_group() {
    let groups = package_tree_groups([
        PackageCategory::Language,
        PackageCategory::Core,
        PackageCategory::Instruments,
        PackageCategory::Core,
        PackageCategory::Language,
    ]);
    assert_eq!(
        groups,
        [
            PackageTreeGroup {
                category: PackageCategory::Core,
                rows: vec![1, 3],
            },
            PackageTreeGroup {
                category: PackageCategory::Instruments,
                rows: vec![2],
            },
            PackageTreeGroup {
                category: PackageCategory::Language,
                rows: vec![0, 4],
            },
        ]
    );
    assert_eq!(
        package_tree_groups([PackageCategory::Tools]),
        [PackageTreeGroup {
            category: PackageCategory::Tools,
            rows: vec![0],
        }]
    );
    assert!(package_tree_groups(Vec::new()).is_empty());
}

/// Each category's heading has a key of its own, and the wizard's text holds
/// a label for every category in display order.
#[test]
fn every_package_category_has_its_own_group_label() {
    assert_eq!(
        PackageCategory::ALL.map(package_group_label_key),
        [
            "wizard-packages-tree-group-label",
            "wizard-instruments-tree-group-label",
            "wizard-tools-tree-group-label",
            "wizard-language-tree-group-label",
        ]
    );
    let text = wizard_text(&Localizer::embedded(DEFAULT_LOCALE).unwrap());
    let categories: Vec<PackageCategory> = text
        .package_group_labels
        .iter()
        .map(|(category, _)| *category)
        .collect();
    assert_eq!(categories, PackageCategory::ALL);
    assert_eq!(
        PackageCategory::ALL.map(|category| text.package_group_label(category)),
        ["Packages", "Instruments", "Tools", "Language packs"]
    );
}

/// A model for `platform` and a REAPER of `architecture`, with no target,
/// whose plan updates every package offered there. Updates are ticked by
/// default, so every row would start ticked if nothing stopped it.
fn every_package_updated(platform: Platform, architecture: Architecture) -> WizardModel {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let actions = builtin_package_specs(platform)
        .into_iter()
        .map(|spec| PlanAction {
            package_id: spec.id,
            action: PlanActionKind::Update,
            installed_version: Some(Version::parse("1.0").unwrap()),
            available_version: Some(Version::parse("2.0").unwrap()),
            reason: "test".to_string(),
            left_to_reapack: None,
        })
        .collect();
    model_from_plan(
        &localizer,
        platform,
        architecture,
        Vec::new(),
        None,
        InstallPlan {
            target: None,
            actions,
            notes: Vec::new(),
        },
    )
}

/// The ids of the rows that can't be ticked, in row order.
fn unavailable_rows(rows: &[PackageRow]) -> Vec<&str> {
    rows.iter()
        .filter(|row| !row.available_for_target)
        .map(|row| row.package_id.as_str())
        .collect()
}

/// Asserts `row` reads as a package with no build for this REAPER: not
/// ticked, not tickable, `reason` at the end of its label, and `sentence`,
/// with the package's name for `{package}`, right below it in its details.
fn assert_no_build_row(row: &PackageRow, reason: &str, sentence: &str) {
    assert!(!row.available_for_target, "{row:?}");
    assert!(!row.selected && !row.default_selected, "{row:?}");
    assert_eq!(row.action, PlanActionKind::Keep, "{row:?}");
    assert_eq!(row.unavailability_reason.as_deref(), Some(reason));
    assert!(
        row.summary.ends_with(&format!("(not available: {reason})")),
        "{}",
        row.summary
    );
    let sentence = sentence.replace("{package}", &row.display_name);
    assert!(
        row.details
            .starts_with(&format!("{}\n\n{sentence}", row.summary)),
        "{}",
        row.details
    );
}

/// The packages with no build for a 32-bit REAPER, in row order: FFmpeg,
/// Surge XT, Lo-lo's four instruments, app2clap and LReaParams.
const NO_32_BIT_BUILD: [&str; 8] = [
    PACKAGE_FFMPEG,
    PACKAGE_SURGE_XT,
    PACKAGE_LJUNO_116,
    PACKAGE_LB_303,
    PACKAGE_LR_608,
    PACKAGE_LSAMPLER_24,
    PACKAGE_APP2CLAP,
    PACKAGE_LREAPARAMS,
];

#[test]
fn a_32_bit_reaper_offers_no_ffmpeg_surge_xt_app2clap_or_lolos_packages() {
    let model = every_package_updated(Platform::Windows, Architecture::X86);
    let rows = &model.package_rows;

    assert_eq!(unavailable_rows(rows), NO_32_BIT_BUILD);
    for package_id in NO_32_BIT_BUILD {
        let row = rows
            .iter()
            .find(|row| row.package_id == package_id)
            .unwrap();
        assert_no_build_row(
            row,
            "no version for 32-bit REAPER",
            "For 32-bit REAPER there is no version of {package}, so RABBIT can't install it here.",
        );
    }
    // The default ticks still tick everything that has a build.
    for package_id in [PACKAGE_REAPER, PACKAGE_OSARA, PACKAGE_SWS, PACKAGE_REAPACK] {
        let row = rows
            .iter()
            .find(|row| row.package_id == package_id)
            .unwrap();
        assert!(row.selected && row.default_selected, "{row:?}");
        assert_eq!(row.action, PlanActionKind::Update);
    }
}

/// REAPER on Windows on ARM is ARM64EC, which loads x64 libraries, so every
/// package is offered there: SWS, FFmpeg and app2clap with their x64 builds,
/// as LReaParams and Lo-lo's instruments. SWS is ticked by default, as
/// everywhere, and a package installed there is ticked for its update.
#[test]
fn windows_on_arm_offers_every_package_sws_and_app2clap_included() {
    for architecture in [Architecture::Arm64, Architecture::Arm64Ec] {
        let model = every_package_updated(Platform::Windows, architecture);
        let rows = &model.package_rows;

        assert!(unavailable_rows(rows).is_empty(), "{architecture:?}");
        for package_id in [
            PACKAGE_SWS,
            PACKAGE_APP2CLAP,
            PACKAGE_FFMPEG,
            PACKAGE_SURGE_XT,
            PACKAGE_LJUNO_116,
            PACKAGE_LB_303,
            PACKAGE_LR_608,
            PACKAGE_LSAMPLER_24,
            PACKAGE_LREAPARAMS,
        ] {
            let row = rows
                .iter()
                .find(|row| row.package_id == package_id)
                .unwrap();
            assert!(
                row.available_for_target && row.has_build_for_target && row.selected,
                "{row:?}"
            );
            assert_eq!(row.action, PlanActionKind::Update, "{row:?}");
            assert_eq!(row.unavailability_reason, None, "{row:?}");
            assert!(!row.summary.contains("not available"), "{}", row.summary);
        }

        // A new install there ticks SWS, which is recommended, and leaves
        // app2clap for the user to tick.
        let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
        let new_install = model_from_plan(
            &localizer,
            Platform::Windows,
            architecture,
            Vec::new(),
            None,
            InstallPlan {
                target: None,
                actions: [PACKAGE_SWS, PACKAGE_APP2CLAP]
                    .map(|package_id| PlanAction {
                        package_id: package_id.to_string(),
                        action: PlanActionKind::Install,
                        installed_version: None,
                        available_version: Some(Version::parse("2.0").unwrap()),
                        reason: "test".to_string(),
                        left_to_reapack: None,
                    })
                    .to_vec(),
                notes: Vec::new(),
            },
        );
        let row = |package_id: &str| {
            new_install
                .package_rows
                .iter()
                .find(|row| row.package_id == package_id)
                .unwrap()
                .clone()
        };
        let sws = row(PACKAGE_SWS);
        assert!(sws.available_for_target && sws.selected, "{sws:?}");
        let app2clap = row(PACKAGE_APP2CLAP);
        assert!(
            app2clap.available_for_target && !app2clap.selected,
            "{app2clap:?}"
        );
    }
}

#[test]
fn a_mac_reaper_keeps_every_package_it_has_a_build_for() {
    // A universal REAPER, and one RABBIT is about to install, are judged by
    // the slice they run as on this computer, which every macOS package has.
    for architecture in [
        Architecture::X64,
        Architecture::Arm64,
        Architecture::Universal,
        Architecture::Unknown,
    ] {
        let model = every_package_updated(Platform::MacOs, architecture);
        assert!(
            unavailable_rows(&model.package_rows).is_empty(),
            "{architecture:?}"
        );
    }
    let intel_32_bit = every_package_updated(Platform::MacOs, Architecture::X86);
    assert_eq!(
        unavailable_rows(&intel_32_bit.package_rows),
        [PACKAGE_SURGE_XT]
    );
}

#[test]
fn the_rows_follow_the_chosen_targets_architecture() {
    // The wizard opened on a 64-bit REAPER, and the user picked a 32-bit
    // one: the rows follow the target the install request goes to.
    let dir = tempdir().unwrap();
    let model = every_package_updated(Platform::Windows, Architecture::X64);
    assert!(unavailable_rows(&model.package_rows).is_empty());
    let portable = TargetRow {
        architecture: Architecture::X86,
        ..custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true)
    };
    let standard = TargetRow {
        portable: false,
        ..portable.clone()
    };

    let plan = wizard_package_plan_for_target(&model, Some(&standard)).unwrap();
    let ffmpeg = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_FFMPEG)
        .unwrap();
    assert_no_build_row(
        ffmpeg,
        "no version for 32-bit REAPER",
        "For 32-bit REAPER there is no version of {package}, so RABBIT can't install it here.",
    );
    assert!(plan.can_install, "REAPER and the rest still install");

    // On a portable target, a package without a build keeps that reason
    // rather than the portable one, and Lo-lo's packages, offered for a
    // portable REAPER, have none for a 32-bit one either.
    let plan = wizard_package_plan_for_target(&model, Some(&portable)).unwrap();
    for package_id in NO_32_BIT_BUILD {
        let row = plan
            .package_rows
            .iter()
            .find(|row| row.package_id == package_id)
            .unwrap();
        assert_eq!(
            row.unavailability_reason.as_deref(),
            Some("no version for 32-bit REAPER"),
            "{row:?}"
        );
    }
}

/// An installed REAPER on Windows on ARM is an ARM64EC program, whose header
/// names x64. Its target row reads it as arm64ec from its program file, and
/// the rows for it offer every package, SWS and app2clap with their x64
/// builds, as for a new REAPER on Windows on ARM.
#[test]
fn an_installed_arm64ec_reaper_offers_sws_and_app2clap() {
    use rabbit_core::arch_probe::PeKind;
    use rabbit_core::arch_probe::test_support::pe_image;

    let dir = tempdir().unwrap();
    let folder = dir.path().join("PortableREAPER");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("reaper.exe"), pe_image(PeKind::Arm64Ec)).unwrap();
    let model = every_package_updated(Platform::Windows, Architecture::X64);
    let portable = custom_portable_target_row(&model, folder, true);
    assert_eq!(portable.architecture, Architecture::Arm64Ec);
    // A standard target, so app2clap and Surge XT aren't held back for
    // being on a portable one.
    let target = TargetRow {
        portable: false,
        ..portable
    };

    let plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();

    assert!(unavailable_rows(&plan.package_rows).is_empty());
    // REAPER and ReaPack have arm64ec builds; SWS, FFmpeg and app2clap give
    // it their x64 ones.
    for package_id in [
        PACKAGE_REAPER,
        PACKAGE_REAPACK,
        PACKAGE_SWS,
        PACKAGE_FFMPEG,
        PACKAGE_APP2CLAP,
    ] {
        let row = plan
            .package_rows
            .iter()
            .find(|row| row.package_id == package_id)
            .unwrap();
        assert!(
            row.available_for_target && row.has_build_for_target,
            "{row:?}"
        );
    }
}

/// Every locale says why a package has no build, for a 32-bit REAPER and for
/// one on another processor, in the row and in its details, with the
/// package's name in the sentence. No package lacks a build for REAPER on
/// Windows on ARM or on a Mac today, so the texts for another processor are
/// checked by the keys a row would use.
#[test]
fn the_no_build_texts_are_in_every_locale() {
    for &locale in rabbit_core::localization::embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let (reason_key, sentence_key) = no_build_keys(Architecture::Arm64Ec);
        assert_eq!(
            (reason_key, sentence_key),
            (
                "wizard-package-row-unavailable-processor",
                "wizard-package-details-unavailable-processor"
            )
        );
        let reason = localizer.text(reason_key);
        assert!(
            !reason.missing && !reason.fallback_used && !reason.value.is_empty(),
            "{locale}: {reason:?}"
        );
        let sentence = localizer.format(sentence_key, &[("package", "SWS Extension")]);
        assert!(
            !sentence.missing
                && !sentence.fallback_used
                && sentence.value.contains("SWS Extension")
                && !sentence.value.contains('{'),
            "{locale}: {sentence:?}"
        );
        // A 32-bit REAPER has no FFmpeg, so its row says so.
        let model = model_from_plan(
            &localizer,
            Platform::Windows,
            Architecture::X86,
            Vec::new(),
            None,
            InstallPlan {
                target: None,
                actions: vec![PlanAction {
                    package_id: PACKAGE_FFMPEG.to_string(),
                    action: PlanActionKind::Install,
                    installed_version: None,
                    available_version: None,
                    reason: "test".to_string(),
                    left_to_reapack: None,
                }],
                notes: Vec::new(),
            },
        );
        let row = &model.package_rows[0];
        let reason = row.unavailability_reason.as_deref().unwrap_or_default();
        assert!(
            !reason.is_empty() && !reason.starts_with("wizard-"),
            "{locale}: {reason:?}"
        );
        assert!(row.summary.ends_with(')'), "{locale}: {}", row.summary);
        let sentence = row.details.split("\n\n").nth(1).unwrap_or_default();
        assert!(
            sentence.contains(&row.display_name)
                && !sentence.contains("wizard-")
                && !sentence.contains('{'),
            "{locale}: {sentence:?}"
        );
    }
}

/// A failed version check doesn't replace the reason a row without a build
/// gives, which its details explain, and Review gets no note about a package
/// no run could install here.
#[test]
fn a_failed_version_check_keeps_the_no_build_reason() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let mut model = every_package_updated(Platform::Windows, Architecture::X86);
    let mut notes = Vec::new();

    apply_version_check_failures_to_rows(
        &localizer,
        &mut model.package_rows,
        &mut notes,
        &[(
            PACKAGE_SURGE_XT.to_string(),
            "rate limit exceeded".to_string(),
        )],
    );

    let surge = model
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_SURGE_XT)
        .unwrap();
    assert_no_build_row(
        surge,
        "no version for 32-bit REAPER",
        "For 32-bit REAPER there is no version of {package}, so RABBIT can't install it here.",
    );
    assert!(notes.is_empty(), "{notes:?}");
}

/// The Uninstall tab doesn't ask about builds: FFmpeg installed in a 32-bit
/// REAPER can still be uninstalled, while the Install tab can't offer it.
#[test]
fn the_uninstall_tab_still_offers_a_package_without_a_build() {
    use rabbit_core::receipt::{
        InstallState, InstalledFileReceipt, PackageReceipt, save_install_state,
    };
    use rabbit_core::uninstall::{NoMacosObjects, NothingListed};

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let plugins = resource.join("UserPlugins");
    std::fs::create_dir_all(&plugins).unwrap();
    std::fs::write(plugins.join("avformat-62.dll"), b"ffmpeg").unwrap();
    let mut state = InstallState::default();
    state.packages.insert(
        PACKAGE_FFMPEG.to_string(),
        PackageReceipt {
            id: PACKAGE_FFMPEG.to_string(),
            version: Some(Version::parse("8.0").unwrap()),
            variant: None,
            channel: None,
            source_url: None,
            source_sha256: None,
            installed_files: vec![InstalledFileReceipt {
                path: "UserPlugins/avformat-62.dll".into(),
                sha256: None,
                size: None,
            }],
            installed_at: None,
            rabbit_version: None,
            architecture: Some(Architecture::X64),
            vendor_added_files: Vec::new(),
        },
    );
    save_install_state(&resource, &state).unwrap();
    let model = every_package_updated(Platform::Windows, Architecture::X86);
    let target = TargetRow {
        architecture: Architecture::X86,
        ..custom_portable_target_row(&model, resource.clone(), true)
    };

    let plan = wizard_package_plan_for_target(&model, Some(&target)).unwrap();
    let install_row = plan
        .package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_FFMPEG)
        .unwrap();
    assert!(!install_row.available_for_target);
    let list = uninstall_list_with_roots(
        &model,
        &target,
        &fixture_roots(Platform::Windows, &resource, dir.path()),
        &plan.detections,
        &NothingListed,
        &NoMacosObjects,
    );
    let uninstall_row = list
        .rows
        .iter()
        .find(|row| row.package_id == PACKAGE_FFMPEG)
        .expect("the Uninstall tab lists the installed FFmpeg");
    assert!(uninstall_row.can_tick, "{uninstall_row:?}");
}

/// No build for the REAPER comes before every other reason a row gives,
/// leaving LReaParams to ReaPack included: on a 32-bit REAPER its row says
/// there is no version for it, whether ReaPack's records name it or can't be
/// read, and its details say so first, then still say what ReaPack's
/// records mean for it. A failed version check changes neither, and Review
/// gets no note.
#[test]
fn no_build_comes_before_leaving_a_package_to_reapack() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    for (left, reapack_sentence) in [
        (
            LeftToReapack::Managed {
                reapack_package: "LReaParams.ext".to_string(),
            },
            "ReaPack's records say it installed LReaParams as LReaParams.ext, so RABBIT leaves \
             it to ReaPack. Install, update or remove it in ReaPack.",
        ),
        (
            LeftToReapack::RecordsUnreadable {
                error: "file is not a database".to_string(),
            },
            "RABBIT couldn't read ReaPack's records (file is not a database), so it can't tell \
             whether ReaPack installed LReaParams, and doesn't install, update or remove it.",
        ),
    ] {
        let mut model = lreaparams_left_to_reapack_on(&localizer, left, Architecture::X86);
        let row = &model.package_rows[0];
        assert!(row.left_to_reapack && !row.has_build_for_target, "{row:?}");
        assert_no_build_row(
            row,
            "no version for 32-bit REAPER",
            "For 32-bit REAPER there is no version of {package}, so RABBIT can't install it here.",
        );
        assert!(!row.summary.contains("ReaPack"), "{}", row.summary);
        let (no_build, rest) = row
            .details
            .strip_prefix(&format!("{}\n\n", row.summary))
            .and_then(|details| details.split_once("\n\n"))
            .unwrap();
        assert!(no_build.starts_with("For 32-bit REAPER"), "{}", row.details);
        assert!(rest.starts_with(reapack_sentence), "{}", row.details);
        assert!(!model.controls.can_install);

        let before = model.package_rows.clone();
        let mut notes = Vec::new();
        apply_version_check_failures_to_rows(
            &localizer,
            &mut model.package_rows,
            &mut notes,
            &[(
                PACKAGE_LREAPARAMS.to_string(),
                "GitHub's rate limit was reached".to_string(),
            )],
        );
        assert_eq!(model.package_rows, before);
        assert!(notes.is_empty(), "{notes:?}");
    }

    // On a 64-bit REAPER, ReaPack's reason is the row's own again.
    let model = lreaparams_left_to_reapack_on(
        &localizer,
        LeftToReapack::Managed {
            reapack_package: "LReaParams.ext".to_string(),
        },
        Architecture::X64,
    );
    assert!(model.package_rows[0].has_build_for_target);
    assert_eq!(
        model.package_rows[0].unavailability_reason.as_deref(),
        Some("managed by ReaPack")
    );
}

/// The sentences about a copy of app2clap in the CLAP folder say what
/// installing, updating or reinstalling app2clap here does. app2clap has no
/// build for a 32-bit REAPER, so none of that can happen there: its row and
/// Review say nothing about the copy, on a portable REAPER and on the
/// standard one, and its details start with why it isn't available. A row
/// RABBIT leaves to ReaPack gets no such sentence either.
#[test]
fn a_copy_in_the_clap_folder_is_not_told_without_a_build() {
    let dir = tempdir().unwrap();
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let model = every_package_updated(Platform::Windows, Architecture::X64);
    let portable = TargetRow {
        architecture: Architecture::X86,
        ..custom_portable_target_row(&model, dir.path().join("PortableREAPER"), true)
    };
    let standard = TargetRow {
        portable: false,
        ..portable.clone()
    };
    let in_clap = |spec: &rabbit_core::package::PackageSpec| {
        if spec.id == PACKAGE_APP2CLAP {
            vec![std::path::PathBuf::from("C:/CLAP/app2clap.clap")]
        } else {
            Vec::new()
        }
    };
    {
        let _seam = crate::wizard::packages::former_copies_seam::install(in_clap);
        for target in [&portable, &standard] {
            let rows = wizard_package_plan_for_target(&model, Some(target))
                .unwrap()
                .package_rows;
            let app2clap = rows
                .iter()
                .find(|row| row.package_id == PACKAGE_APP2CLAP)
                .unwrap();
            assert_no_build_row(
                app2clap,
                "no version for 32-bit REAPER",
                "For 32-bit REAPER there is no version of {package}, so RABBIT can't install it here.",
            );
            assert_eq!(app2clap.former_copy_review_note, None);
            assert!(
                !app2clap.details.contains("CLAP folder"),
                "{}",
                app2clap.details
            );
            assert!(rows.iter().all(|row| row.former_copy_review_note.is_none()));
        }
    }

    // Asked directly, the rule leaves a row without a build, and one left to
    // ReaPack, as they are, while it notes the same row with a build.
    let specs = builtin_package_specs(Platform::Windows);
    let x64_standard = TargetRow {
        architecture: Architecture::X64,
        ..standard
    };
    let rows = wizard_package_plan_for_target(&model, Some(&x64_standard))
        .unwrap()
        .package_rows;
    let index = rows
        .iter()
        .position(|row| row.package_id == PACKAGE_APP2CLAP)
        .unwrap();
    let mut noted = rows.clone();
    crate::wizard::packages::note_copies_in_former_outside_folders(
        &localizer, &mut noted, &specs, false, &in_clap,
    );
    assert!(noted[index].former_copy_review_note.is_some());
    for change in [
        (|row: &mut PackageRow| row.has_build_for_target = false) as fn(&mut PackageRow),
        |row: &mut PackageRow| row.left_to_reapack = true,
    ] {
        let mut changed = rows.clone();
        change(&mut changed[index]);
        let before = changed.clone();
        crate::wizard::packages::note_copies_in_former_outside_folders(
            &localizer,
            &mut changed,
            &specs,
            false,
            &in_clap,
        );
        assert_eq!(changed, before);
    }
}
