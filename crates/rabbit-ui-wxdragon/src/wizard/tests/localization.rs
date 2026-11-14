use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::plan::InstallPlan;
use tempfile::tempdir;

use crate::wizard::labels::*;
use crate::wizard::text::*;
use crate::wizard::*;

#[test]
fn default_options_use_embedded_localization() {
    let options = UiBootstrapOptions::default();
    let localizer = localizer_from_options(&options).unwrap();

    assert_eq!(localizer.active_locale(), DEFAULT_LOCALE);
    assert!(localizer.source_path().is_none());
    assert_eq!(
        localizer.text("app-title").value,
        "REAPER Accessibility Bootstrap & Bundle Installation Tool"
    );
}

#[test]
fn package_operation_messages_localize_into_german() {
    use rabbit_core::operation::PackageOperationMessage as Msg;
    let de = Localizer::embedded("de-DE").unwrap();
    let extension = localized_package_operation_message(
        &de,
        &Msg::ExtensionBinaryInstalled,
        "OSARA",
        "Single extension binary handled by RABBIT installer.",
    );
    assert!(
        extension.starts_with("Einzelne"),
        "expected German extension-binary status, got: {extension:?}"
    );
    let skipped = localized_package_operation_message(
        &de,
        &Msg::SkippedCurrent {
            installed_version: "8.1.1".to_string(),
            available_version: "8.0".to_string(),
        },
        "OSARA",
        "Installed version 8.1.1 is current or newer than available version 8.0.",
    );
    assert!(
        skipped.contains("Installierte Version") && skipped.contains("8.1.1"),
        "expected German skipped-current with version interpolation, got: {skipped:?}"
    );
    let dry_run = localized_package_operation_message(
        &de,
        &Msg::DryRunWouldRunUnattended {
            artifact_kind: rabbit_core::artifact::ArtifactKind::Installer,
        },
        "OSARA",
        "Dry run: RABBIT would download and run this vendor installer unattended.",
    );
    assert!(
        dry_run.starts_with("Probelauf") && dry_run.contains("Vendor-Installationsprogramm"),
        "expected German dry-run with translated automation kind, got: {dry_run:?}"
    );
}

#[test]
fn configuration_step_messages_localize_into_german() {
    use rabbit_core::configuration::{ConfigurationMessage as Msg, ConfigurationStatus};
    let de = Localizer::embedded("de-DE").unwrap();
    let added = localized_configuration_message(
        &de,
        &Msg::ReapackRemoteAdded {
            name: "REAPER Accessibility".to_string(),
            url: "https://example.test/index.xml".to_string(),
        },
        "Added ReaPack remote ...",
    );
    assert!(
        added.contains("REAPER Accessibility")
            && added.contains("hinzugefügt")
            && added.contains("https://example.test/index.xml"),
        "expected German added-remote message with name + URL interpolation, got: {added:?}"
    );
    let dep_missing = localized_configuration_message(
        &de,
        &Msg::SkippedDependencyMissing {
            step_id: "reapack-add-reaper-accessibility-remote".to_string(),
            dep_id: "reapack".to_string(),
        },
        "Configuration step skipped because dependency missing.",
    );
    assert!(
        dep_missing.contains("übersprungen") && dep_missing.contains("reapack"),
        "expected German dependency-missing message, got: {dep_missing:?}"
    );
    let status = configuration_status_label_for_summary(
        Some(&de),
        ConfigurationStatus::SkippedDependencyMissing,
    );
    assert!(
        status.starts_with("Übersprungen") && status.contains("Abhängigkeit"),
        "expected German skipped-dep-missing status, got: {status:?}"
    );
    // The step name lookup should resolve `reapack-add-reaper-accessibility-remote`
    // to its localized display name.
    let step_name =
        localized_configuration_step_name(Some(&de), "reapack-add-reaper-accessibility-remote");
    assert!(
        step_name.contains("ReaPack")
            && (step_name.contains("Repository") || step_name.contains("Repositorys")),
        "expected German display name for the REAPER Accessibility ReaPack repo step, got: {step_name:?}"
    );
}

#[test]
fn wizard_command_labels_include_native_mnemonics() {
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

    // `localized_wx_mnemonic_label` strips `&` mnemonics on macOS to
    // avoid colliding with Cmd+letter system shortcuts (Cmd+C copying
    // would close the wizard via the `&Close` mnemonic). Other
    // platforms keep the underlined Alt-key access. Test both shapes
    // explicitly so a regression in either direction is caught.
    if cfg!(target_os = "macos") {
        assert_eq!(model.controls.back_label, "Back");
        assert_eq!(model.controls.next_label, "Next");
        assert_eq!(model.controls.install_label, "Install");
        assert_eq!(model.controls.uninstall_label, "Uninstall");
        assert_eq!(model.controls.close_label, "Close");
        assert_eq!(model.controls.cancel_label, "Cancel");
        assert_eq!(
            model.text.done_launch_reaper_label,
            "Open REAPER and close RABBIT"
        );
        assert_eq!(
            model.text.done_open_resource_label,
            "Open resource folder (only for advanced manual maintenance)"
        );
    } else {
        assert_eq!(model.controls.back_label, "&Back");
        assert_eq!(model.controls.next_label, "&Next");
        assert_eq!(model.controls.install_label, "&Install");
        assert_eq!(model.controls.uninstall_label, "&Uninstall");
        assert_eq!(model.controls.close_label, "&Close");
        assert_eq!(model.controls.cancel_label, "&Cancel");
        assert_eq!(
            model.text.done_launch_reaper_label,
            "&Open REAPER and close RABBIT"
        );
        assert_eq!(
            model.text.done_open_resource_label,
            "Open &resource folder (only for advanced manual maintenance)"
        );
    }
    assert_eq!(
        model.text.package_handling_unattended,
        "RABBIT can install this package unattended, including launching its installer when required."
    );
    assert_eq!(
        model.text.package_handling_planned,
        "RABBIT is designed to run this package's installer or setup routine itself and finish the installation unattended, but this build still reports the steps instead of executing them."
    );
    assert_eq!(
        model.text.packages_osara_keymap_replace_label,
        "Replace your current key map with latest OSARA key map"
    );
}

/// The run button sits beside Back, Next and Close, which reads Cancel
/// while a run is going. Whatever it says, Uninstall, Restore or Delete, its
/// access key must be none of theirs in any language, and a letter of its
/// own label rather than one appended in brackets.
#[test]
fn the_run_button_mnemonics_clash_with_no_other_button_in_any_locale() {
    let key = |localizer: &Localizer, id: &str| -> char {
        let text = localizer.text(id);
        assert!(!text.missing, "{}: {id}", localizer.active_locale());
        let mut chars = text.value.trim().chars();
        let key = chars.next().unwrap();
        assert_eq!(chars.next(), None, "{}: {id}", localizer.active_locale());
        key.to_lowercase().next().unwrap()
    };
    for &locale in rabbit_core::localization::embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for run in ["uninstall", "restore", "delete"] {
            let mnemonic_id = format!("wizard-button-{run}-mnemonic");
            let run_key = key(&localizer, &mnemonic_id);
            for other in ["back", "next", "close", "cancel"] {
                assert_ne!(
                    run_key,
                    key(&localizer, &format!("wizard-button-{other}-mnemonic")),
                    "{locale}: {run} and {other} share an access key"
                );
            }
            let label = localizer.text(&format!("wizard-button-{run}"));
            assert!(!label.missing, "{locale}: {run}");
            assert!(
                label.value.to_lowercase().contains(run_key),
                "{locale}: {run_key} isn't in {}",
                label.value
            );
            assert!(
                !wx_mnemonic_label(&label.value, &localizer.text(&mnemonic_id).value)
                    .contains("(&"),
                "{locale}: {}",
                label.value
            );
        }
    }
}

/// The run button's four labels come from the model, each with its own
/// access key outside macOS, in every language.
#[test]
fn the_run_button_has_a_label_for_each_mode_in_every_locale() {
    for &locale in rabbit_core::localization::embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
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
        let labels = model.controls.run_button_labels();
        let modes = [
            (WizardMode::Install, "install"),
            (WizardMode::Uninstall, "uninstall"),
            (WizardMode::Restore, "restore"),
            (WizardMode::DeleteBackups, "delete"),
        ];
        let mut seen = Vec::new();
        for (mode, run) in modes {
            let label = labels.for_mode(mode);
            assert_eq!(
                label,
                localized_wx_mnemonic_label(
                    &localizer,
                    &format!("wizard-button-{run}"),
                    &format!("wizard-button-{run}-mnemonic"),
                ),
                "{locale}: {mode:?}"
            );
            assert_eq!(
                label.contains('&'),
                !cfg!(target_os = "macos"),
                "{locale}: {label}"
            );
            assert!(!seen.contains(&label), "{locale}: {label} twice");
            seen.push(label);
        }
    }
}

/// The Backups tab and the choice below its list read in every language,
/// with no access keys: only buttons carry them.
#[test]
fn the_backups_choice_reads_in_every_locale_without_access_keys() {
    for &locale in rabbit_core::localization::embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let text = wizard_text(&localizer);
        for label in [
            &text.packages_tab_backups,
            &text.backups_choice_label,
            &text.backups_choice_restore,
            &text.backups_choice_delete,
        ] {
            assert!(
                !label.is_empty() && !label.starts_with("wizard-") && !label.contains('&'),
                "{locale}: {label}"
            );
        }
        assert_ne!(
            text.backups_choice_restore, text.backups_choice_delete,
            "{locale}"
        );
    }
}

/// Review's headings before a restore and a delete, and the questions
/// asked before stopping either run, read in every language. The headings
/// are the ones the Review builders give, so the two can't drift apart.
#[test]
fn the_backup_review_headings_and_stop_questions_read_in_every_locale() {
    let list = WizardBackupList::default();
    for &locale in rabbit_core::localization::embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let text = wizard_text(&localizer);
        for label in [
            &text.review_restore_heading,
            &text.review_delete_heading,
            &text.close_during_restore_title,
            &text.close_during_restore_body,
            &text.close_during_delete_title,
            &text.close_during_delete_body,
        ] {
            assert!(
                !label.is_empty() && !label.starts_with("wizard-") && !label.contains('&'),
                "{locale}: {label}"
            );
        }
        assert_eq!(
            text.review_restore_heading,
            build_backup_restore_review(&localizer, None, &list, None, &[]).heading,
            "{locale}"
        );
        assert_eq!(
            text.review_delete_heading,
            build_backup_delete_review(&localizer, None, &list, &[]).heading,
            "{locale}"
        );
        assert_ne!(
            text.close_during_restore_title, text.close_during_delete_title,
            "{locale}"
        );
        assert_ne!(
            text.close_during_restore_body, text.close_during_delete_body,
            "{locale}"
        );
    }
}

/// Each new text of the Backups tab reads its own key, so a restore is
/// never offered, reviewed or stopped with a delete's words, or the other
/// way round. The RadioBox's first choice is always Restore.
#[test]
fn each_backup_text_reads_its_own_key() {
    let text = wizard_text(&Localizer::embedded(DEFAULT_LOCALE).unwrap());
    assert_eq!(BackupChoice::ALL[0], BackupChoice::Restore);
    assert_eq!(text.packages_tab_backups, "Backups");
    assert_eq!(
        text.backups_choice_label,
        "What to do with the ticked backups"
    );
    assert_eq!(text.backups_choice_restore, "Restore");
    assert_eq!(text.backups_choice_delete, "Delete");
    assert_eq!(text.review_restore_heading, "Ready to restore");
    assert_eq!(text.review_delete_heading, "Ready to delete backups");
    assert_eq!(text.close_during_restore_title, "Stop restoring?");
    assert!(
        text.close_during_restore_body
            .starts_with("RABBIT is restoring."),
        "{}",
        text.close_during_restore_body
    );
    assert_eq!(text.close_during_delete_title, "Stop deleting backups?");
    assert!(
        text.close_during_delete_body
            .starts_with("RABBIT is deleting backups."),
        "{}",
        text.close_during_delete_body
    );
}

/// VoiceOver says the macOS hint once, with nothing on screen to read it
/// again, so it must name the keys in every language: Control written out,
/// as each locale spells it, and the Tab key.
#[test]
fn the_tab_switch_hint_names_control_and_tab_in_every_locale() {
    for &locale in rabbit_core::localization::embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let text = localizer.text("wizard-packages-tabs-switch-hint-macos");
        assert!(!text.missing, "{locale}");
        assert_eq!(
            wizard_text(&localizer).packages_tabs_switch_hint_macos,
            text.value
        );
        assert!(
            (text.value.contains("Control") || text.value.contains("Contrôle"))
                && text.value.contains("Tab"),
            "{locale}: {}",
            text.value
        );
    }
}

#[test]
fn wx_mnemonic_labels_support_translated_access_keys() {
    assert_eq!(wx_mnemonic_label("Weiter", "W"), "&Weiter");
    assert_eq!(wx_mnemonic_label("Schliessen", "S"), "&Schliessen");
    assert_eq!(
        wx_mnemonic_label("Bericht speichern", "S"),
        "Bericht &speichern"
    );
    assert_eq!(wx_mnemonic_label("Weiter", "X"), "Weiter (&X)");
    assert_eq!(wx_mnemonic_label("Save & report", "S"), "&Save && report");
}

#[test]
fn locale_directory_override_remains_available_for_development() {
    let dir = tempdir().unwrap();
    let locale_dir = dir.path().join("de-DE");
    std::fs::create_dir_all(&locale_dir).unwrap();
    std::fs::write(locale_dir.join("rabbit.ftl"), "app-title = RABBIT Test\n").unwrap();
    let options = UiBootstrapOptions {
        locale: "de-DE".to_string(),
        locales_dir: Some(dir.path().to_path_buf()),
        portable_roots: Vec::new(),
        online_versions: false,
    };

    let localizer = localizer_from_options(&options).unwrap();

    assert_eq!(localizer.active_locale(), "de-DE");
    assert!(localizer.source_path().is_some());
    assert_eq!(localizer.text("app-title").value, "RABBIT Test");
}
