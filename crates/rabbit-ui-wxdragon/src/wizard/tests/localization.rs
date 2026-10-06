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
        assert_eq!(model.controls.close_label, "Close");
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
        assert_eq!(model.controls.close_label, "&Close");
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
