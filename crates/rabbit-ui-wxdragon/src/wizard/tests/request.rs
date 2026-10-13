use std::path::PathBuf;

use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::osara_keymap::OsaraKeymapState;
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
            package_channels: Default::default(),
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
        OsaraKeymapState::OwnChanges,
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

/// A new OSARA arrives with its own key map, and so does an installed one
/// whose key map is still OSARA's own. Any other key map stays: OSARA's
/// "Merge OSARA key map" action adds the new bindings without throwing the
/// user's own away.
#[test]
fn the_key_map_is_replaced_only_when_nothing_of_the_users_is_lost() {
    let rows_for = |action: Option<PlanActionKind>| {
        let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
        let installation = fake_installation();
        model_from_plan(
            &localizer,
            Platform::Windows,
            Architecture::X64,
            vec![installation.clone()],
            Some(0),
            InstallPlan {
                target: Some(installation),
                actions: action
                    .map(|action| PlanAction {
                        package_id: PACKAGE_OSARA.to_string(),
                        action,
                        installed_version: None,
                        available_version: Some(Version::parse("2026.10.5").unwrap()),
                        reason: "test".to_string(),
                    })
                    .into_iter()
                    .collect(),
                notes: Vec::new(),
            },
        )
        .package_rows
    };
    use OsaraKeymapChoice::{PreserveCurrent, ReplaceCurrent};
    use OsaraKeymapState::*;
    let all = [DefaultKeys, UnchangedOsara, OwnChanges, Unknown];
    for keymap in all {
        // A new OSARA, or none offered at all: the usual default.
        assert_eq!(
            osara_keymap_default(&rows_for(Some(PlanActionKind::Install)), keymap),
            ReplaceCurrent,
            "{keymap:?}"
        );
        assert_eq!(
            osara_keymap_default(&rows_for(None), keymap),
            ReplaceCurrent,
            "{keymap:?}"
        );
    }
    for action in [PlanActionKind::Update, PlanActionKind::Keep] {
        let rows = rows_for(Some(action));
        assert_eq!(osara_keymap_default(&rows, UnchangedOsara), ReplaceCurrent);
        assert_eq!(osara_keymap_default(&rows, OwnChanges), PreserveCurrent);
        assert_eq!(osara_keymap_default(&rows, Unknown), PreserveCurrent);
        assert_eq!(osara_keymap_default(&rows, DefaultKeys), PreserveCurrent);
    }
}

/// The note and the review line say why the checkbox starts where it does.
#[test]
fn the_key_map_texts_follow_the_choice_and_the_key_map() {
    use OsaraKeymapChoice::{PreserveCurrent, ReplaceCurrent};
    use OsaraKeymapState::*;
    let (_, model) = langpack_model(PlanActionKind::Install);
    let text = &model.text;
    let note = |choice, keymap| osara_keymap_note(&model, true, choice, keymap);
    let line = |choice, keymap| osara_keymap_review_line(&model, choice, keymap);

    assert_eq!(
        note(ReplaceCurrent, UnchangedOsara),
        text.packages_osara_keymap_unchanged_note
    );
    assert_eq!(
        line(ReplaceCurrent, UnchangedOsara),
        text.review_osara_keymap_replace_unchanged
    );
    assert_eq!(
        note(PreserveCurrent, DefaultKeys),
        text.packages_osara_keymap_default_keys_note
    );
    assert_eq!(
        line(PreserveCurrent, DefaultKeys),
        text.review_osara_keymap_keep_default_keys
    );
    for keymap in [DefaultKeys, OwnChanges, Unknown] {
        assert_eq!(
            note(ReplaceCurrent, keymap),
            text.packages_osara_keymap_replace_note
        );
        assert_eq!(
            line(ReplaceCurrent, keymap),
            text.review_osara_keymap_replace
        );
    }
    for keymap in [UnchangedOsara, OwnChanges, Unknown] {
        assert_eq!(
            note(PreserveCurrent, keymap),
            text.packages_osara_keymap_preserve_note
        );
        assert_eq!(
            line(PreserveCurrent, keymap),
            text.review_osara_keymap_preserve
        );
    }
    assert!(note(PreserveCurrent, OwnChanges).contains("Merge OSARA key map"));
    // Not one of these is empty or a missing-key placeholder.
    for text in [
        &text.packages_osara_keymap_unchanged_note,
        &text.packages_osara_keymap_default_keys_note,
        &text.review_osara_keymap_replace_unchanged,
        &text.review_osara_keymap_keep_default_keys,
    ] {
        assert!(text.contains("REAPER") || text.contains("OSARA"), "{text}");
    }
}

#[test]
fn default_install_options_replace_osara_keymap() {
    assert_eq!(
        WizardInstallOptions::default().osara_keymap_choice,
        OsaraKeymapChoice::ReplaceCurrent
    );
}
