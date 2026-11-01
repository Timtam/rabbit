use std::path::PathBuf;

use rabbit_core::localization::{DEFAULT_LOCALE, Localizer, embedded_locales};
use rabbit_core::model::{Architecture, ComponentDetection, InstallationKind, Platform};
use rabbit_core::package::uninstall::{UninstallStrategy, UninstallSummary};
use rabbit_core::package::{PackageCategory, builtin_package_specs};
use rabbit_core::uninstall::{
    Removability, UNINSTALL_NOT_YET_KEY, UninstallNote, UninstallPlan, UninstallPlanItem,
    UninstallWarning,
};
use rabbit_core::version::Version;
use tempfile::tempdir;

use super::support::*;
use crate::wizard::text::*;
use crate::wizard::*;

/// A plan item for `package_id` with nothing but its removability set, as
/// the plan builder leaves a package it finds installed.
fn item(package_id: &str, removability: Removability) -> UninstallPlanItem {
    UninstallPlanItem {
        package_id: package_id.to_string(),
        display_name: package_id.to_string(),
        installed_version: Some(Version::parse("2.14.0.7").unwrap()),
        has_receipt: true,
        depends_on: Vec::new(),
        removability,
        strategy: Some(UninstallStrategy::RemoveFiles),
        backup: Default::default(),
        summary: None,
        files: Vec::new(),
        dirs_if_empty: Vec::new(),
        kept: Vec::new(),
        post: Vec::new(),
        blocking_programs: Vec::new(),
        every_reaper_closed: false,
        requires_elevation: false,
        unbacked: false,
        confirm: false,
        warnings: Vec::new(),
        notes: Vec::new(),
    }
}

/// SWS as the plan describes a copy RABBIT can remove, with its manifest's
/// sentences and a warning because OSARA is installed.
fn removable_sws() -> UninstallPlanItem {
    UninstallPlanItem {
        summary: Some(UninstallSummary {
            removes_key: "uninstall-summary-sws-removes".to_string(),
            keeps_key: "uninstall-summary-sws-keeps".to_string(),
        }),
        warnings: vec![UninstallWarning::OtherPackageInstalled {
            package_id: "osara".to_string(),
            message_key: "uninstall-warning-sws-osara-keymap".to_string(),
        }],
        ..item("sws", Removability::Removable)
    }
}

fn plan(items: Vec<UninstallPlanItem>) -> UninstallPlan {
    UninstallPlan {
        resource_path: PathBuf::from("C:/REAPER"),
        platform: Platform::Windows,
        target: InstallationKind::Portable,
        remove_settings: false,
        items,
    }
}

fn rows_for(items: Vec<UninstallPlanItem>) -> Vec<UninstallRow> {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    uninstall_list_from_plan(
        &localizer,
        &builtin_package_specs(Platform::Windows),
        &plan(items),
    )
    .rows
}

#[test]
fn only_installed_packages_get_a_row_in_manifest_order() {
    let rows = rows_for(vec![
        item(
            "osara",
            Removability::NotSupported {
                note_key: UNINSTALL_NOT_YET_KEY.to_string(),
            },
        ),
        removable_sws(),
        item("reapack", Removability::NotInstalled),
        item("langpack-de", Removability::Removable),
    ]);

    let ids: Vec<&str> = rows.iter().map(|row| row.package_id.as_str()).collect();
    assert_eq!(ids, ["osara", "sws", "langpack-de"]);
}

#[test]
fn nothing_is_ticked_by_default() {
    let rows = rows_for(vec![
        removable_sws(),
        item("reapack", Removability::Removable),
        item("langpack-de", Removability::Removable),
    ]);

    assert!(rows.iter().all(|row| row.can_tick));
    assert!(rows.iter().all(|row| !row.selected));
    assert!(rows.iter().all(|row| row.summary.contains("Won't touch")));
}

#[test]
fn a_row_says_the_action_first_and_ticking_only_changes_its_text() {
    let mut row = rows_for(vec![removable_sws()]).remove(0);
    assert_eq!(row.summary, "SWS Extension: Won't touch. You have 2.14.0.7");
    assert!(row.details.starts_with(&row.summary));
    assert!(row.details.contains("Removes the SWS extension."));
    assert!(row.details.contains("Your grooves and your SWS settings"));
    assert!(row.details.contains("some of its keys run SWS actions"));
    let explanation = row.details[row.summary.len()..].to_string();

    assert!(apply_uninstall_checkbox(&mut row, true));
    assert!(row.selected);
    assert_eq!(
        row.summary,
        "SWS Extension: Will uninstall. You have 2.14.0.7"
    );
    assert!(row.details.starts_with(&row.summary));
    assert_eq!(row.details[row.summary.len()..], explanation);

    assert!(apply_uninstall_checkbox(&mut row, false));
    assert!(!row.selected);
    assert_eq!(row.summary, "SWS Extension: Won't touch. You have 2.14.0.7");
    // Ticking the state a row already has changes nothing.
    assert!(!apply_uninstall_checkbox(&mut row, false));
}

#[test]
fn the_always_shown_warning_comes_before_what_the_removal_does() {
    let row = rows_for(vec![UninstallPlanItem {
        warnings: vec![
            UninstallWarning::OtherPackageInstalled {
                package_id: "reapack".to_string(),
                message_key: "uninstall-warning-sws-reapack-scripts".to_string(),
            },
            UninstallWarning::Always {
                message_key: "uninstall-warning-sws-osara-keymap".to_string(),
                confirm: true,
            },
        ],
        ..removable_sws()
    }])
    .remove(0);

    let always = row
        .details
        .find("some of its keys run SWS actions")
        .unwrap();
    let removes = row.details.find("Removes the SWS extension.").unwrap();
    let other = row.details.find("Many REAPER scripts").unwrap();
    assert!(always < removes && removes < other, "{}", row.details);
}

#[test]
fn a_row_that_cant_be_removed_cant_be_ticked_and_says_why() {
    let mut row = rows_for(vec![item(
        "sws",
        Removability::ManagedByReaPack {
            path: PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-x64.dll"),
            reapack_package: "SWS/S&M extension".to_string(),
        },
    )])
    .remove(0);

    assert!(!row.can_tick);
    assert_eq!(
        row.summary,
        "SWS Extension: Won't touch. You have 2.14.0.7 (not available: managed by ReaPack)"
    );
    assert_eq!(
        row.unavailability_reason.as_deref(),
        Some("managed by ReaPack")
    );
    assert!(
        row.details
            .contains("as SWS/S&M extension. Uninstall it in ReaPack instead.")
    );

    let before = row.clone();
    assert!(!apply_uninstall_checkbox(&mut row, true));
    assert_eq!(row, before);
}

#[test]
fn a_manual_row_shows_the_instructions_to_remove_it_by_hand() {
    let row = rows_for(vec![item(
        "jaws-scripts",
        Removability::Manual {
            note_key: "uninstall-manual-jaws-scripts".to_string(),
        },
    )])
    .remove(0);

    assert!(!row.can_tick);
    assert!(
        row.summary.ends_with("(not available: remove it by hand)"),
        "{}",
        row.summary
    );
    assert!(row.details.contains("Delete Reaper_ComAccess32.dll"));
}

#[test]
fn packages_from_later_phases_say_they_cant_be_uninstalled_yet() {
    let rows = rows_for(vec![
        item(
            "osara",
            Removability::NotSupported {
                note_key: UNINSTALL_NOT_YET_KEY.to_string(),
            },
        ),
        // Removable on paper, but only through a step a backup can't undo,
        // which this version refuses.
        UninstallPlanItem {
            unbacked: true,
            ..item("surge-xt", Removability::Removable)
        },
    ]);

    for row in &rows {
        assert!(!row.can_tick, "{}", row.package_id);
        assert!(
            row.summary
                .ends_with("(not available: RABBIT can't uninstall it yet)"),
            "{}",
            row.summary
        );
        assert!(row.details.contains("RABBIT can't uninstall this yet."));
    }
}

#[test]
fn every_reason_a_package_stays_reads_as_a_sentence() {
    let path = PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-x64.dll");
    let reasons = [
        Removability::ReaPackCheckFailed {
            path: path.clone(),
            reason: "disk error".to_string(),
        },
        Removability::NeedsStandardTarget,
        Removability::NoReceipt,
        Removability::NoTrustedUninstaller,
        Removability::UnsafePath {
            path,
            refusal: rabbit_core::uninstall::PathRefusal::RabbitFolder,
        },
        Removability::NotSupported {
            note_key: "uninstall-manual-reaper-portable".to_string(),
        },
    ];
    for removability in reasons {
        let row = rows_for(vec![item("sws", removability.clone())]).remove(0);
        assert!(!row.can_tick);
        let reason = row.unavailability_reason.clone().unwrap();
        assert!(row.summary.ends_with(&format!("(not available: {reason})")));
        let explanation = &row.details[row.summary.len()..];
        assert!(
            explanation.trim().len() > 20 && !explanation.contains("wizard-"),
            "{removability:?}: {explanation}"
        );
    }
}

#[test]
fn a_portable_target_reads_like_the_install_row_of_the_same_package() {
    let row = rows_for(vec![item("app2clap", Removability::NeedsStandardTarget)]).remove(0);
    assert!(
        row.summary
            .ends_with("(not available: portable REAPER target)")
    );
    assert!(row.details.contains("only from the standard REAPER"));
}

#[test]
fn notes_and_the_other_warnings_follow_what_stays() {
    let row = rows_for(vec![UninstallPlanItem {
        warnings: vec![
            UninstallWarning::DependentsStay {
                package_ids: vec!["osara".to_string(), "sws".to_string()],
            },
            UninstallWarning::MachineWide,
            UninstallWarning::ReaPackStillManages {
                reapack_packages: vec!["ReaLearn".to_string()],
            },
        ],
        notes: vec![
            UninstallNote::NoReceipt,
            UninstallNote::OrphanedFromReaPack {
                reapack_packages: Vec::new(),
            },
            UninstallNote::Manifest {
                note_key: "uninstall-note-reapack-packages-stay".to_string(),
            },
        ],
        ..item("reapack", Removability::Removable)
    }])
    .remove(0);

    for expected in [
        "install on top of ReaPack and stay: OSARA, SWS Extension.",
        "every other program on this computer",
        "once it is gone: ReaLearn.",
        "It removes the files ReaPack is known to install.",
        "ReaPack once installed ReaPack.",
        "nothing keeps them up to date",
    ] {
        assert!(
            row.details.contains(expected),
            "{expected}: {}",
            row.details
        );
    }
}

#[test]
fn rows_go_into_the_same_groups_as_the_install_tree() {
    let rows = rows_for(vec![
        item("sws", Removability::Removable),
        item("app2clap", Removability::Removable),
        item("langpack-de", Removability::Removable),
    ]);

    let categories: Vec<PackageCategory> = rows.iter().map(|row| row.category).collect();
    assert_eq!(
        categories,
        [
            PackageCategory::Core,
            PackageCategory::Additional,
            PackageCategory::Language
        ]
    );
}

#[test]
fn an_empty_list_has_a_placeholder_row_and_a_details_sentence() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let text = wizard_text(&localizer);
    let list = uninstall_list_from_plan(
        &localizer,
        &builtin_package_specs(Platform::Windows),
        &plan(vec![
            item("sws", Removability::NotInstalled),
            item("reapack", Removability::NotInstalled),
        ]),
    );

    assert!(list.rows.is_empty());
    let (label, details) = list.placeholder(&text).unwrap();
    assert_eq!(
        label,
        "RABBIT found nothing it can uninstall in this REAPER."
    );
    assert!(!details.is_empty() && details != label);

    let with_rows = uninstall_list_from_plan(
        &localizer,
        &builtin_package_specs(Platform::Windows),
        &plan(vec![removable_sws()]),
    );
    assert_eq!(with_rows.placeholder(&text), None);
}

#[test]
fn a_target_with_nothing_installed_lists_nothing() {
    let dir = tempdir().unwrap();
    let (_, model) = langpack_model(rabbit_core::plan::PlanActionKind::Install);
    let target = TargetRow {
        path: dir.path().to_path_buf(),
        ..model.target_rows[0].clone()
    };
    let detections: Vec<ComponentDetection> = builtin_package_specs(model.platform)
        .iter()
        .map(|spec| ComponentDetection::not_installed(&spec.id, &spec.display_name))
        .collect();

    let list = uninstall_list_for_target(&model, &target, &detections);

    assert!(list.rows.is_empty());
    assert_eq!(list.unreadable, None);
    assert_eq!(
        list.placeholder(&model.text).unwrap().0,
        model.text.uninstall_empty
    );
}

#[test]
fn a_receipt_that_cant_be_read_says_so_instead_of_listing_nothing() {
    let dir = tempdir().unwrap();
    let receipt = rabbit_core::receipt::receipt_path(dir.path());
    std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
    std::fs::write(&receipt, "not json").unwrap();
    let (_, model) = langpack_model(rabbit_core::plan::PlanActionKind::Install);
    let target = TargetRow {
        path: dir.path().to_path_buf(),
        ..model.target_rows[0].clone()
    };

    let list = uninstall_list_for_target(&model, &target, &[]);

    assert!(list.rows.is_empty());
    let (label, _) = list.placeholder(&model.text).unwrap();
    assert!(
        label.starts_with("RABBIT couldn't read its records for this REAPER ("),
        "{label}"
    );
}

#[test]
fn the_target_row_decides_whether_the_plan_is_for_a_portable_reaper() {
    let dir = tempdir().unwrap();
    let (_, model) = langpack_model(rabbit_core::plan::PlanActionKind::Install);
    for portable in [true, false] {
        let target = TargetRow {
            path: dir.path().to_path_buf(),
            portable,
            ..model.target_rows[0].clone()
        };
        let plan = uninstall_plan_for_target(&model, &target, &[]).unwrap();
        let expected = if portable {
            InstallationKind::Portable
        } else {
            InstallationKind::Standard
        };
        assert_eq!(plan.target, expected);
    }
}

#[test]
fn rows_build_in_every_locale_without_missing_text() {
    let items = || {
        vec![
            removable_sws(),
            item(
                "jaws-scripts",
                Removability::Manual {
                    note_key: "uninstall-manual-jaws-scripts".to_string(),
                },
            ),
            item("app2clap", Removability::NeedsStandardTarget),
            item(
                "reapack",
                Removability::ManagedByReaPack {
                    path: PathBuf::from("C:/REAPER/UserPlugins/reaper_reapack-x64.dll"),
                    reapack_package: "ReaPack".to_string(),
                },
            ),
        ]
    };
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let list = uninstall_list_from_plan(
            &localizer,
            &builtin_package_specs(Platform::Windows),
            &plan(items()),
        );
        for mut row in list.rows {
            apply_uninstall_checkbox(&mut row, true);
            for text in [&row.summary, &row.details] {
                assert!(
                    !text.contains("wizard-") && !text.contains("action-") && !text.contains('{'),
                    "{locale}: {text}"
                );
            }
        }
        let text = wizard_text(&localizer);
        for label in [
            &text.packages_tab_install,
            &text.packages_tab_uninstall,
            &text.uninstall_list_label,
            &text.uninstall_empty,
            &text.uninstall_empty_details,
            &text.uninstall_not_ready_title,
        ] {
            assert!(
                !label.is_empty() && !label.starts_with("wizard-"),
                "{locale}: {label}"
            );
        }
    }
}

#[test]
fn the_wizard_plan_hands_over_its_detections() {
    let dir = tempdir().unwrap();
    let (_, model) = langpack_model(rabbit_core::plan::PlanActionKind::Install);
    let target = TargetRow {
        path: dir.path().to_path_buf(),
        architecture: Architecture::X64,
        ..model.target_rows[0].clone()
    };

    let plan = wizard_package_plan_for_target_with_available(&model, Some(&target), &[]).unwrap();

    // Packages that serve the whole computer are detected outside the
    // target, so only the shape is checked: one detection per package. A
    // missing one would drop that package from the Uninstall tab.
    let specs = builtin_package_specs(model.platform);
    let mut detected: Vec<_> = plan
        .detections
        .iter()
        .map(|detection| detection.package_id.as_str())
        .collect();
    detected.sort_unstable();
    let mut expected: Vec<_> = specs.iter().map(|spec| spec.id.as_str()).collect();
    expected.sort_unstable();
    assert_eq!(detected, expected);
    assert!(
        plan.detections
            .iter()
            .all(|detection| { specs.iter().any(|spec| spec.id == detection.package_id) })
    );
    assert!(
        plan.detections
            .iter()
            .filter(|detection| detection.package_id == "langpack-de")
            .all(|detection| !detection.installed)
    );
}

/// The dialog's example must run as typed: `--resource-path` is required.
#[test]
fn the_not_ready_dialog_names_a_command_that_runs() {
    let resource = std::path::Path::new(r"C:\Users\Test\AppData\Roaming\REAPER");
    for locale in ["en-US", "de-DE", "es-ES", "fr-FR", "it-IT"] {
        let localizer = Localizer::embedded(locale).unwrap();
        let body = uninstall_not_ready_body(&localizer, resource);
        assert!(
            body.contains(r#"rabbit uninstall --resource-path "C:\Users\Test\AppData\Roaming\REAPER" --package sws"#),
            "{locale}: {body}"
        );
        assert!(body.contains("--apply"), "{locale}: {body}");
        assert!(!body.contains('{'), "{locale}: {body}");
    }
}
