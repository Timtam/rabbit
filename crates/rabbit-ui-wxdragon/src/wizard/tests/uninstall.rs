use std::path::PathBuf;

use rabbit_core::localization::{DEFAULT_LOCALE, Localizer, embedded_locales};
use rabbit_core::model::{Architecture, ComponentDetection, InstallationKind, Platform};
use rabbit_core::package::uninstall::{UninstallBackup, UninstallStrategy, UninstallSummary};
use rabbit_core::package::{PACKAGE_REAPACK, PackageCategory, builtin_package_specs};
use rabbit_core::plan::{InstallPlan, PlanAction, PlanActionKind};
use rabbit_core::receipt::{InstallState, PackageReceipt};
use rabbit_core::uninstall::{
    ByHandReason, CleanupJob, FakeMacosObjects, KeptPath, KeptReason, NoMacosObjects,
    NothingListed, ObjectFacts, PathRefusal, PlannedMachineCleanup, PlannedRemoval,
    PlannedVendorRun, Removability, RemovalReason, TrashFolderFacts, UNINSTALL_NOT_YET_KEY,
    UninstallNote, UninstallPlan, UninstallPlanInput, UninstallPlanItem, UninstallRoots,
    UninstallWarning, VendorJob, VendorLookup, VendorRefusal, build_uninstall_plan,
};
use rabbit_core::version::Version;
use tempfile::tempdir;

use super::support::*;
use crate::wizard::text::*;
use crate::wizard::*;

/// A plan item for `package_id` with nothing but its removability set, as
/// the plan builder leaves a package it finds installed.
pub(super) fn item(package_id: &str, removability: Removability) -> UninstallPlanItem {
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
        registry_cleanup: Vec::new(),
        vendor_run: None,
        machine_cleanup: None,
        blocking_programs: Vec::new(),
        macos_pkg_forget: Vec::new(),
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
pub(super) fn removable_sws() -> UninstallPlanItem {
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

pub(super) fn plan(items: Vec<UninstallPlanItem>) -> UninstallPlan {
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
        Removability::NoTrustedUninstaller {
            refusal: VendorRefusal::NotListed,
        },
        Removability::UnsafePath {
            path,
            refusal: rabbit_core::uninstall::PathRefusal::RabbitFolder,
        },
        Removability::NotSupported {
            note_key: "uninstall-manual-reaper-portable".to_string(),
        },
        Removability::CleanupNotTrusted {
            refusal: VendorRefusal::RegistryKeyHasSubkeys,
            by_hand_key: "uninstall-manual-reaper-portable".to_string(),
        },
        Removability::NoFilesForThisAccount {
            by_hand_key: "uninstall-manual-reaper-portable".to_string(),
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

/// The macOS list shows no row for an empty list, since every row there has
/// a checkbox or a disclosure triangle, so switching to the tab says the
/// placeholder's sentence with the tab's name.
#[test]
fn switching_to_an_empty_uninstall_tab_says_why_it_is_empty() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let text = wizard_text(&localizer);

    assert_eq!(
        WizardUninstallList::default().tab_announcement(&text),
        "Uninstall. RABBIT found nothing it can uninstall in this REAPER."
    );
    let unreadable = WizardUninstallList {
        unreadable: Some("RABBIT couldn't read its records.".to_string()),
        ..WizardUninstallList::default()
    };
    assert_eq!(
        unreadable.tab_announcement(&text),
        "Uninstall. RABBIT couldn't read its records."
    );
    assert_eq!(
        list_for(vec![removable_sws()]).tab_announcement(&text),
        "Uninstall"
    );
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

    let list = uninstall_list_for_target(
        &model,
        &target,
        &detections,
        &NothingListed,
        &NoMacosObjects,
    );

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

    let list = uninstall_list_for_target(&model, &target, &[], &NothingListed, &NoMacosObjects);

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
        let plan = uninstall_plan_for_target(&model, &target, &[], &NothingListed, &NoMacosObjects)
            .unwrap();
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
            vendor_surge(),
            item(
                "reaper",
                Removability::NoTrustedUninstaller {
                    refusal: VendorRefusal::ForeignFile {
                        path: PathBuf::from("C:/Program Files/REAPER (x64)/planted.dll"),
                    },
                },
            ),
            // The JAWS scripts: removable with the notes on script files
            // that stay, refused for their uninstaller folder or registry
            // entry, left to another account, and from a portable REAPER.
            UninstallPlanItem {
                notes: vec![
                    UninstallNote::NoVendorRecord,
                    UninstallNote::VendorFilesByHand {
                        note_key: SCRIPTS_NOTE_KEY.to_string(),
                    },
                ],
                ..item("jaws-scripts", Removability::Removable)
            },
            item(
                "jaws-scripts",
                Removability::CleanupNotTrusted {
                    refusal: VendorRefusal::UntrustedFolder {
                        path: PathBuf::from("C:/Program Files (x86)/Reaper_JawsScripts"),
                    },
                    by_hand_key: BY_HAND_KEY.to_string(),
                },
            ),
            item(
                "jaws-scripts",
                Removability::NoFilesForThisAccount {
                    by_hand_key: BY_HAND_KEY.to_string(),
                },
            ),
            UninstallPlanItem {
                notes: vec![UninstallNote::Manifest {
                    note_key: BY_HAND_KEY.to_string(),
                }],
                ..item("jaws-scripts", Removability::NeedsStandardTarget)
            },
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
            &text.packages_tabs_switch_hint_macos,
            &text.uninstall_list_label,
            &text.uninstall_empty,
            &text.uninstall_empty_details,
            &text.progress_uninstall_heading,
            &text.progress_status_uninstall_running,
            &text.progress_status_uninstall_cancelling,
            &text.close_during_uninstall_title,
            &text.close_during_uninstall_body,
            &text.error_lock_held,
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

// ---------------------------------------------------------------------------
// The mode, the run button and Review in uninstall mode.
// ---------------------------------------------------------------------------

#[test]
fn the_tab_showing_decides_the_mode() {
    assert_eq!(
        WizardMode::for_packages_tab(PACKAGES_INSTALL_TAB as i32),
        WizardMode::Install
    );
    assert_eq!(
        WizardMode::for_packages_tab(PACKAGES_UNINSTALL_TAB as i32),
        WizardMode::Uninstall
    );
    // No selection, or a tab that doesn't exist: the page opens on Install.
    assert_eq!(WizardMode::for_packages_tab(-1), WizardMode::Install);
    assert_eq!(WizardMode::for_packages_tab(7), WizardMode::Install);
    assert_eq!(WizardMode::default(), WizardMode::Install);
}

#[test]
fn an_uninstall_never_goes_through_the_reapack_page() {
    assert!(WizardMode::Install.shows_reapack_acknowledgement(true));
    assert!(!WizardMode::Install.shows_reapack_acknowledgement(false));
    // ReaPack ticked for install on the other tab changes nothing about an
    // uninstall, on the way to Review or back from it.
    assert!(!WizardMode::Uninstall.shows_reapack_acknowledgement(true));
    assert!(!WizardMode::Uninstall.shows_reapack_acknowledgement(false));
}

#[test]
fn enter_never_starts_an_uninstall() {
    assert!(WizardMode::Install.enter_starts_run());
    assert!(!WizardMode::Uninstall.enter_starts_run());
}

#[test]
fn the_uninstall_button_has_its_own_run_flag() {
    // Install: both the plan and Review must allow it.
    assert!(WizardMode::Install.run_button_enabled(true, true, false));
    assert!(!WizardMode::Install.run_button_enabled(false, true, true));
    assert!(!WizardMode::Install.run_button_enabled(true, false, true));
    // Uninstall: only its own Review counts.
    assert!(WizardMode::Uninstall.run_button_enabled(false, false, true));
    assert!(!WizardMode::Uninstall.run_button_enabled(true, true, false));
}

/// A target RABBIT can write to, at a folder that needn't exist.
pub(super) fn target(writable: bool) -> TargetRow {
    let (_, model) = langpack_model(rabbit_core::plan::PlanActionKind::Install);
    TargetRow {
        path: PathBuf::from("C:/REAPER"),
        writable,
        ..model.target_rows[0].clone()
    }
}

pub(super) fn list_for(items: Vec<UninstallPlanItem>) -> WizardUninstallList {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    uninstall_list_from_plan(
        &localizer,
        &builtin_package_specs(Platform::Windows),
        &plan(items),
    )
}

pub(super) fn tick(list: &mut WizardUninstallList, package_id: &str) {
    let row = list
        .rows
        .iter_mut()
        .find(|row| row.package_id == package_id)
        .unwrap();
    assert!(apply_uninstall_checkbox(row, true), "{package_id}");
}

fn review(
    list: &WizardUninstallList,
    target: Option<&TargetRow>,
    remove_settings: bool,
    install_tab_ticked: &[String],
) -> WizardUninstallReviewPreview {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    build_uninstall_review_preview(
        &localizer,
        target,
        list,
        remove_settings,
        install_tab_ticked,
    )
}

/// Review's flag in uninstall mode, as the wizard sets it: the run button
/// then follows it alone.
fn uninstall_button_enabled(plan_can_install: bool) -> bool {
    let mut list = list_for(vec![removable_sws()]);
    tick(&mut list, "sws");
    let preview = review(&list, Some(&target(true)), false, &[]);
    WizardMode::Uninstall.run_button_enabled(plan_can_install, false, preview.can_uninstall)
}

#[test]
fn uninstall_works_when_nothing_needs_installing() {
    let (_, model) = langpack_model(rabbit_core::plan::PlanActionKind::Keep);
    assert!(!model.controls.can_install);
    assert!(uninstall_button_enabled(model.controls.can_install));
}

#[test]
fn uninstall_works_when_every_version_check_failed() {
    let (localizer, model) = langpack_model(rabbit_core::plan::PlanActionKind::Install);
    let mut rows = model.package_rows.clone();
    let mut notes = model.notes.clone();
    let plan_can_install = apply_version_check_failures_to_rows(
        &localizer,
        &mut rows,
        &mut notes,
        &[("langpack-de".to_string(), "network down".to_string())],
    );
    assert!(!plan_can_install);
    assert!(uninstall_button_enabled(plan_can_install));
}

#[test]
fn nothing_ticked_disables_uninstall_and_review_says_why() {
    let list = list_for(vec![removable_sws()]);
    let preview = review(&list, Some(&target(true)), false, &[]);

    assert!(!preview.can_uninstall);
    assert!(
        preview
            .lines
            .contains(&"Nothing is ticked to uninstall.".to_string()),
        "{:?}",
        preview.lines
    );
    // Nothing to install again either.
    assert!(
        !preview
            .lines
            .iter()
            .any(|line| line.contains("again later"))
    );
    assert!(!WizardMode::Uninstall.run_button_enabled(true, true, preview.can_uninstall));
}

#[test]
fn review_says_what_goes_and_what_stays_without_listing_files() {
    let mut list = list_for(vec![
        UninstallPlanItem {
            files: vec![PlannedRemoval {
                path: PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-x64.dll"),
                reason: RemovalReason::Receipt,
                size: Some(4_000_000),
                is_dir: false,
                receipt_sha256: None,
                bundle_id: None,
                guard: Default::default(),
            }],
            ..removable_sws()
        },
        UninstallPlanItem {
            summary: Some(UninstallSummary {
                removes_key: "uninstall-summary-langpack-removes".to_string(),
                keeps_key: "uninstall-summary-langpack-keeps".to_string(),
            }),
            ..item("langpack-de", Removability::Removable)
        },
        item("reapack", Removability::Removable),
    ]);
    tick(&mut list, "sws");
    tick(&mut list, "langpack-de");

    let preview = review(&list, Some(&target(true)), false, &[]);

    assert!(preview.can_uninstall);
    assert_eq!(preview.lines[0], "Target: C:/REAPER");
    let packages = preview
        .lines
        .iter()
        .position(|line| line == "Packages to uninstall")
        .unwrap();
    assert_eq!(
        preview.lines[packages + 1],
        "SWS Extension: Removes the SWS extension. Your grooves and your SWS settings, such as \
         your cycle actions, stay."
    );
    assert!(
        preview.lines[packages + 2].starts_with("REAPER in German: Removes the language pack"),
        "{:?}",
        preview.lines
    );
    // ReaPack isn't ticked, so Review doesn't mention it.
    assert!(!preview.lines.iter().any(|line| line.contains("ReaPack")));
    let text = preview.lines.join("\n");
    assert!(text.contains("some of its keys run SWS actions"), "{text}");
    assert!(
        text.contains("You can install these again later from the Install or update tab."),
        "{text}"
    );
    assert!(!text.contains("reaper_sws-x64.dll"), "{text}");
    assert!(!text.contains("backup set"), "{text}");
}

#[test]
fn review_starts_with_the_warnings_that_always_show() {
    let mut list = list_for(vec![
        removable_sws(),
        UninstallPlanItem {
            warnings: vec![UninstallWarning::Always {
                message_key: "uninstall-warning-sws-reapack-scripts".to_string(),
                confirm: false,
            }],
            ..item("reakontrol", Removability::Removable)
        },
    ]);
    tick(&mut list, "sws");
    tick(&mut list, "reakontrol");

    let text = review(&list, Some(&target(true)), false, &[])
        .lines
        .join("\n");

    // ReaKontrol comes after SWS in the list, but its always-shown warning
    // comes before SWS's other one.
    let always = text.find("Many REAPER scripts").unwrap();
    let other = text.find("some of its keys run SWS actions").unwrap();
    assert!(always < other, "{text}");
}

#[test]
fn review_says_the_install_ticks_are_not_part_of_the_run() {
    let mut list = list_for(vec![removable_sws()]);
    tick(&mut list, "sws");

    let text = review(
        &list,
        Some(&target(true)),
        false,
        &["OSARA".to_string(), "REAPER".to_string()],
    )
    .lines
    .join("\n");

    assert!(
        text.contains(
            "What is ticked on the Install or update tab isn't part of this run: OSARA, REAPER."
        ),
        "{text}"
    );
}

/// A model for a portable REAPER whose Install or update tab offers
/// `actions`, each with 9.9 available and `installed` on disk.
fn install_tab_model_with(actions: &[(&str, PlanActionKind, Option<&str>)]) -> WizardModel {
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
            actions: actions
                .iter()
                .map(|(package_id, action, installed)| PlanAction {
                    package_id: package_id.to_string(),
                    action: *action,
                    installed_version: installed.map(|version| Version::parse(version).unwrap()),
                    available_version: Some(Version::parse("9.9").unwrap()),
                    reason: "test".to_string(),
                })
                .collect(),
            notes: Vec::new(),
        },
    )
}

/// OSARA and SWS with updates, which RABBIT ticks by itself, FFmpeg not
/// installed and not recommended, and the German language pack installed
/// and current.
fn install_tab_model() -> (Localizer, WizardModel) {
    let model = install_tab_model_with(&[
        ("osara", PlanActionKind::Update, Some("1.0")),
        ("sws", PlanActionKind::Update, Some("1.0")),
        ("ffmpeg", PlanActionKind::Install, None),
        ("langpack-de", PlanActionKind::Keep, Some("9.9")),
    ]);
    (Localizer::embedded(DEFAULT_LOCALE).unwrap(), model)
}

/// Someone who opens RABBIT only to remove SWS never ticked the OSARA
/// update, and SWS's own update tick would read as a contradiction of its
/// removal, so Review leaves out what RABBIT ticked by itself.
#[test]
fn an_uninstall_review_leaves_out_the_install_ticks_rabbit_made() {
    let (_, model) = install_tab_model();
    let mut package_rows = model.package_rows.clone();
    let mut configuration_rows = model.configuration_rows.clone();
    remember_default_ticks(&mut package_rows, &mut configuration_rows);
    let ticked: Vec<&str> = package_rows
        .iter()
        .filter(|row| row.selected)
        .map(|row| row.package_id.as_str())
        .collect();
    assert_eq!(ticked, ["osara", "sws"]);

    let install_ticked = install_tab_ticks_left_out(&package_rows, &configuration_rows);
    assert!(install_ticked.is_empty(), "{install_ticked:?}");
    let mut list = list_for(vec![removable_sws()]);
    tick(&mut list, "sws");
    let text = review(&list, Some(&target(true)), false, &install_ticked)
        .lines
        .join("\n");
    assert!(!text.contains("Install or update tab isn't"), "{text}");
}

/// What the user did tick on the Install or update tab is named, with the
/// setting RABBIT ticked along with it, since the uninstall leaves both
/// out. Unticking one of RABBIT's ticks leaves nothing out.
#[test]
fn an_uninstall_review_names_the_install_ticks_the_user_made() {
    let (localizer, model) = install_tab_model();
    let mut package_rows = model.package_rows.clone();
    let mut configuration_rows = model.configuration_rows.clone();
    remember_default_ticks(&mut package_rows, &mut configuration_rows);

    for (package_id, checked) in [("ffmpeg", true), ("langpack-de", true), ("osara", false)] {
        let row = package_rows
            .iter_mut()
            .find(|row| row.package_id == package_id)
            .unwrap();
        apply_checkbox_state_to_package_row(&model, row, checked).unwrap();
    }
    recompute_configuration_row_availability(
        &localizer,
        &package_rows,
        None,
        &mut configuration_rows,
    );

    assert_eq!(
        install_tab_ticks_left_out(&package_rows, &configuration_rows),
        [
            "FFmpeg for Improved Video Support",
            "REAPER in German",
            "Set REAPER's language"
        ]
    );
}

/// The configuration rows are built at start-up, and the version check
/// ticks the recommended settings it makes available on them. Those are
/// RABBIT's ticks too, so the defaults are taken once it has settled them.
#[test]
fn defaults_are_the_ticks_the_install_tab_opens_with() {
    // At start-up RABBIT knew nothing of ReaPack, so its repository
    // settings couldn't run.
    let (localizer, model) = install_tab_model();
    let mut configuration_rows = model.configuration_rows.clone();
    // The version check finds ReaPack installed, which ticks them.
    let mut package_rows =
        install_tab_model_with(&[(PACKAGE_REAPACK, PlanActionKind::Keep, Some("9.9"))])
            .package_rows;
    recompute_configuration_row_availability(
        &localizer,
        &package_rows,
        None,
        &mut configuration_rows,
    );
    assert!(
        !install_tab_ticks_left_out(&package_rows, &configuration_rows).is_empty(),
        "the version check should have ticked a ReaPack setting"
    );

    remember_default_ticks(&mut package_rows, &mut configuration_rows);

    assert_eq!(
        install_tab_ticks_left_out(&package_rows, &configuration_rows),
        Vec::<String>::new()
    );
}

#[test]
fn an_install_review_names_the_uninstall_ticks_it_leaves_out() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    assert_eq!(
        inactive_tab_note(
            &localizer,
            WizardMode::Install,
            &["SWS Extension".to_string()]
        )
        .as_deref(),
        Some("What is ticked on the Uninstall tab isn't part of this run: SWS Extension.")
    );
    assert_eq!(
        inactive_tab_note(&localizer, WizardMode::Install, &[]),
        None
    );
    assert_eq!(
        inactive_tab_note(&localizer, WizardMode::Uninstall, &[]),
        None
    );
}

#[test]
fn a_package_for_every_program_asks_to_close_them_first() {
    let mut list = list_for(vec![UninstallPlanItem {
        warnings: vec![UninstallWarning::MachineWide],
        ..item("app2clap", Removability::Removable)
    }]);
    tick(&mut list, "app2clap");

    let text = review(&list, Some(&target(true)), false, &[])
        .lines
        .join("\n");

    assert!(text.contains("Close other music programs first."), "{text}");
}

#[test]
fn a_target_rabbit_cant_write_to_blocks_the_uninstall_and_says_so() {
    let mut list = list_for(vec![removable_sws()]);
    tick(&mut list, "sws");

    let preview = review(&list, Some(&target(false)), false, &[]);

    assert!(!preview.can_uninstall);
    assert_eq!(
        preview.lines.last().unwrap(),
        "Can't uninstall yet: RABBIT can't write to this REAPER's folder."
    );
}

#[test]
fn without_a_target_review_says_so_and_nothing_starts() {
    let mut list = list_for(vec![removable_sws()]);
    tick(&mut list, "sws");

    let preview = review(&list, None, false, &[]);

    assert!(!preview.can_uninstall);
    assert_eq!(preview.lines, ["No target selected."]);
}

// ---------------------------------------------------------------------------
// "Also remove settings".
// ---------------------------------------------------------------------------

pub(super) fn kept(path: &str, note_key: &str, removable_with_settings: bool) -> KeptPath {
    KeptPath {
        path: PathBuf::from(path),
        reason: KeptReason::UserData,
        note_key: Some(note_key.to_string()),
        exists: true,
        removable_with_settings,
    }
}

/// SWS with settings the option could remove, two of them described alike,
/// and one entry the option never takes.
pub(super) fn sws_with_settings() -> UninstallPlanItem {
    UninstallPlanItem {
        kept: vec![
            kept("C:/REAPER/Data/Grooves", "uninstall-kept-sws-grooves", true),
            kept("C:/REAPER/S&M.ini", "uninstall-kept-sws-settings", true),
            kept("C:/REAPER/BR.ini", "uninstall-kept-sws-settings", true),
            kept(
                "C:/REAPER/S&M_Cyclactions.ini",
                "uninstall-kept-sws-cycle-actions",
                false,
            ),
        ],
        ..removable_sws()
    }
}

fn settings_note(list: &WizardUninstallList) -> String {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    uninstall_settings_note(&localizer, list)
}

#[test]
fn a_row_knows_which_settings_the_option_would_remove() {
    let rows = rows_for(vec![
        sws_with_settings(),
        item("ffmpeg", Removability::Removable),
        UninstallPlanItem {
            kept: vec![kept(
                "C:/REAPER/reapack.ini",
                "uninstall-kept-reapack-settings",
                true,
            )],
            ..item(
                "reapack",
                Removability::ManagedByReaPack {
                    path: PathBuf::from("C:/REAPER/UserPlugins/reaper_reapack-x64.dll"),
                    reapack_package: "ReaPack".to_string(),
                },
            )
        },
    ]);

    // Each description once, in the plan's order, and only what the option
    // may take.
    assert_eq!(rows[0].settings, ["your grooves", "SWS settings"]);
    assert!(rows[1].settings.is_empty());
    // A row that can't be ticked has nothing the option could remove.
    assert!(rows[2].settings.is_empty());
}

#[test]
fn the_settings_option_needs_a_ticked_package_with_settings() {
    let mut list = list_for(vec![
        sws_with_settings(),
        item("ffmpeg", Removability::Removable),
    ]);
    assert!(!list.settings_option_available());
    assert_eq!(
        settings_note(&list),
        "Tick a package to see which of its settings RABBIT can remove. Unless you tick this \
         option, settings stay."
    );

    tick(&mut list, "ffmpeg");
    assert!(!list.settings_option_available());
    assert_eq!(
        settings_note(&list),
        "None of the ticked packages has settings RABBIT can remove."
    );

    tick(&mut list, "sws");
    assert!(list.settings_option_available());
    let note = settings_note(&list);
    assert!(
        note.starts_with("With this ticked, RABBIT also removes"),
        "{note}"
    );
    assert!(
        note.ends_with("\nSWS Extension: your grooves, SWS settings"),
        "{note}"
    );
    assert!(!note.contains("FFmpeg"), "{note}");

    let sws = list
        .rows
        .iter_mut()
        .find(|row| row.package_id == "sws")
        .unwrap();
    apply_uninstall_checkbox(sws, false);
    assert!(!list.settings_option_available());
}

#[test]
fn voiceover_hears_only_when_the_settings_option_comes_or_goes() {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let note = |was_available: bool, list: &WizardUninstallList| {
        uninstall_settings_availability_note(&localizer, was_available, list)
    };
    let mut list = list_for(vec![
        sws_with_settings(),
        item("ffmpeg", Removability::Removable),
        UninstallPlanItem {
            kept: vec![kept(
                "C:/REAPER/ReaKontrol/fxMaps",
                "uninstall-kept-reakontrol-fx-maps",
                true,
            )],
            ..item("reakontrol", Removability::Removable)
        },
    ]);

    // A package without settings changes the note, not the option.
    tick(&mut list, "ffmpeg");
    assert_eq!(note(false, &list), None);

    tick(&mut list, "sws");
    assert_eq!(
        note(false, &list).as_deref(),
        Some("Now available below the list: Also remove settings.")
    );

    // A second package with settings only adds to the note.
    tick(&mut list, "reakontrol");
    assert_eq!(note(true, &list), None);

    for package_id in ["sws", "reakontrol"] {
        let row = list
            .rows
            .iter_mut()
            .find(|row| row.package_id == package_id)
            .unwrap();
        apply_uninstall_checkbox(row, false);
    }
    assert_eq!(
        note(true, &list).as_deref(),
        Some("No longer available: Also remove settings.")
    );
}

#[test]
fn with_settings_ticked_review_says_they_go_instead_of_that_they_stay() {
    let mut list = list_for(vec![
        sws_with_settings(),
        UninstallPlanItem {
            summary: Some(UninstallSummary {
                removes_key: "uninstall-summary-ffmpeg-removes".to_string(),
                keeps_key: "uninstall-summary-ffmpeg-keeps".to_string(),
            }),
            ..item("ffmpeg", Removability::Removable)
        },
    ]);
    tick(&mut list, "sws");
    tick(&mut list, "ffmpeg");

    let text = review(&list, Some(&target(true)), true, &[])
        .lines
        .join("\n");

    assert!(
        text.contains(
            "SWS Extension: Removes the SWS extension. Its settings go too, into the backup: \
             your grooves, SWS settings."
        ),
        "{text}"
    );
    assert!(
        !text.contains("Your grooves and your SWS settings"),
        "{text}"
    );
    // FFmpeg has no settings to remove, so what stays is still said.
    assert!(
        text.contains("FFmpeg files that RABBIT has no record of"),
        "{text}"
    );

    let unticked = review(&list, Some(&target(true)), false, &[])
        .lines
        .join("\n");
    assert!(
        unticked.contains("Your grooves and your SWS settings"),
        "{unticked}"
    );
}

/// The plan `rabbit-core` builds from SWS's real manifest, for a copy
/// without a receipt in a folder with two of its settings files: the row
/// offers exactly what the option would take.
#[test]
fn the_real_sws_plan_offers_its_existing_settings() {
    let dir = tempdir().unwrap();
    let resource = dir.path();
    std::fs::create_dir_all(resource.join("UserPlugins")).unwrap();
    std::fs::write(resource.join("UserPlugins/reaper_sws-x64.dll"), b"sws").unwrap();
    std::fs::write(resource.join("S&M.ini"), b"[sws]").unwrap();
    std::fs::write(resource.join("BR.ini"), b"[br]").unwrap();
    let specs = builtin_package_specs(Platform::Windows);
    let detections: Vec<ComponentDetection> = specs
        .iter()
        .map(|spec| {
            let mut detection = ComponentDetection::not_installed(&spec.id, &spec.display_name);
            if spec.id == "sws" {
                detection.installed = true;
                detection.version = Some(Version::parse("2.14.0.7").unwrap());
            }
            detection
        })
        .collect();
    let roots = UninstallRoots::for_target(Platform::Windows, resource, None);
    let plan = build_uninstall_plan(&UninstallPlanInput {
        roots: &roots,
        platform: Platform::Windows,
        target: InstallationKind::Portable,
        specs: &specs,
        detections: &detections,
        state: None,
        remove_settings: false,
        vendor_lookup: &NothingListed,
        objects: &NoMacosObjects,
    });
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let list = uninstall_list_from_plan(&localizer, &specs, &plan);
    let sws = list
        .rows
        .iter()
        .find(|row| row.package_id == "sws")
        .unwrap();

    assert!(sws.can_tick, "{}", sws.details);
    assert_eq!(sws.settings, ["SWS settings"]);
}

// ---------------------------------------------------------------------------
// The question before starting.
// ---------------------------------------------------------------------------

fn ids(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| id.to_string()).collect()
}

fn question(plan: &UninstallPlan, package_ids: &[&str]) -> Option<String> {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    uninstall_confirmation_body(&localizer, plan, &ids(package_ids))
}

#[test]
fn removals_that_go_into_a_backup_need_no_question() {
    let plan = plan(vec![
        removable_sws(),
        item("reapack", Removability::Removable),
        UninstallPlanItem {
            warnings: vec![UninstallWarning::Always {
                message_key: "uninstall-warning-sws-reapack-scripts".to_string(),
                confirm: false,
            }],
            ..item("reakontrol", Removability::Removable)
        },
    ]);

    assert!(uninstall_confirm_reasons(&plan, &ids(&["sws", "reapack", "reakontrol"])).is_empty());
    assert_eq!(question(&plan, &["sws", "reapack", "reakontrol"]), None);
}

/// A plan as later phases bring them: Surge XT's vendor uninstaller, which
/// a backup can't undo, and the JAWS scripts, whose manifest is to ask
/// before they go. The warning's text is borrowed: any manifest's will do.
fn plan_with_questions() -> UninstallPlan {
    plan(vec![
        UninstallPlanItem {
            warnings: vec![UninstallWarning::Always {
                message_key: "uninstall-warning-sws-osara-keymap".to_string(),
                confirm: true,
            }],
            confirm: true,
            ..item("jaws-scripts", Removability::Removable)
        },
        removable_sws(),
        UninstallPlanItem {
            strategy: Some(UninstallStrategy::VendorUninstaller),
            unbacked: true,
            confirm: true,
            ..item("surge-xt", Removability::Removable)
        },
    ])
}

#[test]
fn a_removal_a_backup_cant_undo_asks_first() {
    let plan = plan_with_questions();

    assert_eq!(
        uninstall_confirm_reasons(&plan, &ids(&["sws", "surge-xt"])),
        [UninstallConfirmReason::Unbacked {
            package_id: "surge-xt".to_string()
        }]
    );
    assert_eq!(
        question(&plan, &["sws", "surge-xt"]).as_deref(),
        Some(
            "A backup can't bring Surge XT back. To have it again, you install it again.\n\n\
             Uninstall now?"
        )
    );
}

#[test]
fn a_warning_whose_manifest_asks_to_confirm_asks_first() {
    let plan = plan_with_questions();

    assert_eq!(
        uninstall_confirm_reasons(&plan, &ids(&["jaws-scripts"])),
        [UninstallConfirmReason::Warning {
            package_id: "jaws-scripts".to_string(),
            message_key: "uninstall-warning-sws-osara-keymap".to_string(),
        }]
    );
    let body = question(&plan, &["jaws-scripts", "surge-xt"]).unwrap();
    let warning = body.find("some of its keys run SWS actions").unwrap();
    let unbacked = body.find("A backup can't bring Surge XT back.").unwrap();
    assert!(warning < unbacked, "{body}");
    assert!(body.ends_with("\n\nUninstall now?"), "{body}");
}

#[test]
fn only_the_ticked_packages_decide_the_question() {
    let plan = plan_with_questions();
    assert_eq!(question(&plan, &["sws"]), None);
    assert_eq!(question(&plan, &[]), None);
}

#[test]
fn the_plans_own_flag_asks_even_without_a_sentence_for_it() {
    let plan = plan(vec![UninstallPlanItem {
        confirm: true,
        ..item("sws", Removability::Removable)
    }]);
    assert_eq!(question(&plan, &["sws"]).as_deref(), Some("Uninstall now?"));
}

#[test]
fn the_question_reads_in_every_locale() {
    let plan = plan_with_questions();
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let body =
            uninstall_confirmation_body(&localizer, &plan, &ids(&["jaws-scripts", "surge-xt"]))
                .unwrap();
        assert!(
            !body.contains("wizard-") && !body.contains("uninstall-") && !body.contains('{'),
            "{locale}: {body}"
        );
        assert!(body.contains("Surge XT"), "{locale}: {body}");
    }
}

/// Every package RABBIT can remove whose files go into a backup, with no
/// warning that asks, asks no question before it goes. That includes
/// OSARA: the people who use RABBIT know that REAPER stops speaking
/// without it. The question is for what a backup can't bring back. On
/// Windows, REAPER and Surge XT can't be ticked here, as no uninstaller is
/// found for them. On macOS these roots don't know where REAPER's app is,
/// so `{app_path}` is refused; Surge XT, which the administrator command
/// deletes for good, can be ticked, and is the one package that asks.
/// REAPER's question is tested with its own plans.
#[test]
fn only_a_package_a_backup_cant_bring_back_asks_a_question() {
    let dir = tempdir().unwrap();
    // Surge XT's trees are absolute macOS paths. They are planned below an
    // empty temporary system root, never the host's real /, so what this
    // computer has installed can't change the answer.
    let system = tempdir().unwrap();
    for platform in [Platform::Windows, Platform::MacOs] {
        let specs = builtin_package_specs(platform);
        let detections: Vec<ComponentDetection> = specs
            .iter()
            .map(|spec| ComponentDetection {
                installed: true,
                version: Some(Version::parse("1.0").unwrap()),
                ..ComponentDetection::not_installed(&spec.id, &spec.display_name)
            })
            .collect();
        // A receipt for every package, as RABBIT's own installs leave
        // behind. FFmpeg and the language packs can't be ticked without
        // one, and would drop out of what this test checks.
        let state = InstallState {
            packages: specs
                .iter()
                .map(|spec| (spec.id.clone(), empty_receipt(&spec.id)))
                .collect(),
            ..InstallState::default()
        };
        let mut roots = UninstallRoots::for_target(platform, dir.path(), None);
        roots.system_root = system.path().to_path_buf();
        let plan = build_uninstall_plan(&UninstallPlanInput {
            roots: &roots,
            platform,
            target: InstallationKind::Standard,
            specs: &specs,
            detections: &detections,
            state: Some(&state),
            remove_settings: false,
            vendor_lookup: &NothingListed,
            objects: &NoMacosObjects,
        });
        if platform == Platform::MacOs {
            assert!(
                matches!(
                    plan.item("reaper").map(|item| &item.removability),
                    Some(Removability::UnsafePath {
                        refusal: PathRefusal::UnknownRoot { .. },
                        ..
                    })
                ),
                "{:?}",
                plan.item("reaper")
            );
        }
        let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
        let tickable: Vec<String> = uninstall_list_from_plan(&localizer, &specs, &plan)
            .rows
            .into_iter()
            .filter(|row| row.can_tick)
            .map(|row| row.package_id)
            .collect();
        // Every package of Phases 1 and 3, so the coverage can't quietly
        // shrink.
        let mut removable = vec![
            "sws",
            "reapack",
            "reakontrol",
            "langpack-es",
            "langpack-de",
            "langpack-fr",
            "osara",
        ];
        if platform == Platform::Windows {
            removable.push("ffmpeg");
            // app2clap lives in Windows' own CLAP folder, which only a
            // Windows machine can name; planning for Windows elsewhere
            // rightly blocks it.
            if cfg!(windows) {
                removable.push("app2clap");
            }
        }
        for id in removable {
            assert!(
                tickable.iter().any(|ticked| ticked == id),
                "{platform:?}: {id} isn't in {tickable:?}"
            );
        }
        let (asks, backed_up): (Vec<String>, Vec<String>) =
            tickable.into_iter().partition(|id| id == "surge-xt");
        assert_eq!(
            asks.len(),
            usize::from(platform == Platform::MacOs),
            "{platform:?}: {asks:?}"
        );
        assert_eq!(
            uninstall_confirmation_body(&localizer, &plan, &backed_up),
            None,
            "{platform:?}: {backed_up:?}"
        );
        if !asks.is_empty() {
            let body = uninstall_confirmation_body(&localizer, &plan, &asks)
                .expect("Surge XT asks before it goes");
            assert!(body.contains("Surge XT"), "{body}");
        }
    }
}

/// OSARA's row and its Review line, from the shipped manifest: what goes,
/// on Windows that RABBIT takes it off Windows' list of installed programs,
/// and that the key map and OSARA's settings stay. "Also remove settings"
/// has nothing of OSARA's to take: its settings share a file with every
/// other extension's. Nothing asks before it goes.
#[test]
fn osara_says_what_goes_and_what_stays() {
    let dir = tempdir().unwrap();
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    for (platform, names_the_list) in [(Platform::Windows, true), (Platform::MacOs, false)] {
        let specs = builtin_package_specs(platform);
        let detections = vec![ComponentDetection {
            installed: true,
            version: Some(Version::parse("2025.3.6.1234").unwrap()),
            ..ComponentDetection::not_installed("osara", "OSARA")
        }];
        let roots = UninstallRoots::for_target(platform, dir.path(), None);
        let plan = build_uninstall_plan(&UninstallPlanInput {
            roots: &roots,
            platform,
            target: InstallationKind::Standard,
            specs: &specs,
            detections: &detections,
            state: None,
            remove_settings: false,
            vendor_lookup: &NothingListed,
            objects: &NoMacosObjects,
        });
        let mut list = uninstall_list_from_plan(&localizer, &specs, &plan);
        let row = list
            .rows
            .iter()
            .find(|row| row.package_id == "osara")
            .unwrap();
        assert!(row.can_tick, "{platform:?}: {}", row.details);
        assert!(
            row.details
                .contains("Removes the OSARA extension, its translations"),
            "{}",
            row.details
        );
        assert_eq!(
            row.details.contains("RABBIT takes it off that list."),
            names_the_list,
            "{platform:?}: {}",
            row.details
        );
        assert!(
            row.details.contains("Your key map stays as it is"),
            "{}",
            row.details
        );
        assert!(row.settings.is_empty(), "{:?}", row.settings);

        tick(&mut list, "osara");
        assert!(!list.settings_option_available());
        let preview =
            build_uninstall_review_preview(&localizer, Some(&target(true)), &list, true, &[]);
        let text = preview.lines.join("\n");
        assert!(
            text.contains("OSARA: Removes the OSARA extension"),
            "{text}"
        );
        assert!(text.contains("Your key map stays as it is"), "{text}");
        assert!(preview.can_uninstall);
        assert_eq!(
            uninstall_confirmation_body(&localizer, &plan, &ids(&["osara"])),
            None
        );
    }
}

/// A receipt for `package_id` that lists no files, which is all the plan
/// asks of a package that can only be removed with one.
fn empty_receipt(package_id: &str) -> PackageReceipt {
    PackageReceipt {
        id: package_id.to_string(),
        version: None,
        source_url: None,
        source_sha256: None,
        variant: None,
        channel: None,
        installed_files: Vec::new(),
        installed_at: None,
        rabbit_version: None,
        architecture: None,
        vendor_added_files: Vec::new(),
    }
}

/// `action-keep` is shared with the Install or update tab, and an unticked
/// Uninstall row starts with it. In Spanish it used to say there was no
/// need to touch the package, which read as advice against removing it.
#[test]
fn an_unticked_uninstall_row_only_says_it_isnt_marked_in_spanish() {
    let localizer = Localizer::embedded("es-ES").unwrap();
    let list = uninstall_list_from_plan(
        &localizer,
        &builtin_package_specs(Platform::Windows),
        &plan(vec![removable_sws()]),
    );

    let row = &list.rows[0].summary;
    assert!(row.contains(": No modificar. Tienes la versión"), "{row}");
}

#[test]
fn review_and_the_settings_note_read_in_every_locale() {
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let mut list = uninstall_list_from_plan(
            &localizer,
            &builtin_package_specs(Platform::Windows),
            &plan(vec![
                UninstallPlanItem {
                    warnings: vec![
                        UninstallWarning::MachineWide,
                        UninstallWarning::OtherPackageInstalled {
                            package_id: "osara".to_string(),
                            message_key: "uninstall-warning-sws-osara-keymap".to_string(),
                        },
                    ],
                    ..sws_with_settings()
                },
                item("ffmpeg", Removability::Removable),
                vendor_surge(),
            ]),
        );
        tick(&mut list, "surge-xt");
        let mut texts = vec![
            uninstall_settings_note(&localizer, &list),
            uninstall_settings_availability_note(&localizer, true, &list).unwrap(),
        ];
        tick(&mut list, "ffmpeg");
        texts.push(uninstall_settings_note(&localizer, &list));
        tick(&mut list, "sws");
        texts.push(uninstall_settings_note(&localizer, &list));
        texts.push(uninstall_settings_availability_note(&localizer, false, &list).unwrap());
        for (writable, remove_settings) in [(true, false), (false, true)] {
            let preview = build_uninstall_review_preview(
                &localizer,
                Some(&target(writable)),
                &list,
                remove_settings,
                &["OSARA".to_string()],
            );
            texts.push(preview.lines.join("\n"));
        }
        texts.push(
            build_uninstall_review_preview(
                &localizer,
                Some(&target(true)),
                &list_for(vec![removable_sws()]),
                false,
                &[],
            )
            .lines
            .join("\n"),
        );
        texts.push(
            inactive_tab_note(&localizer, WizardMode::Install, &["SWS".to_string()]).unwrap(),
        );
        for text in texts {
            assert!(
                !text.contains("wizard-")
                    && !text.contains("uninstall-")
                    && !text.contains("action-")
                    && !text.contains('{'),
                "{locale}: {text}"
            );
        }
        let text = wizard_text(&localizer);
        for label in [
            &text.uninstall_settings_heading,
            &text.uninstall_remove_settings_label,
            &text.review_uninstall_heading,
            &text.confirm_uninstall_title,
        ] {
            assert!(
                !label.is_empty() && !label.starts_with("wizard-"),
                "{locale}: {label}"
            );
        }
    }
}

/// Surge XT as the plan describes it once RABBIT found its own uninstaller
/// and trusts it: removed with administrator rights, no backup, a question
/// first, and Surge and every REAPER closed.
pub(super) fn vendor_surge() -> UninstallPlanItem {
    UninstallPlanItem {
        strategy: Some(UninstallStrategy::VendorUninstaller),
        summary: Some(UninstallSummary {
            removes_key: "uninstall-summary-surge-removes".to_string(),
            keeps_key: "uninstall-summary-surge-keeps".to_string(),
        }),
        vendor_run: Some(PlannedVendorRun {
            kind: rabbit_core::package::uninstall::VendorUninstallerKind::Inno,
            uninstaller: PathBuf::from("C:/ProgramData/Surge XT/uninstall/unins000.exe"),
            arguments: vec!["/VERYSILENT".to_string()],
            timeout_seconds: 600,
            still_waiting_seconds: 30,
            success_exit_codes: Vec::new(),
            verify_gone: Vec::new(),
            remove_leftovers: Vec::new(),
            report_leftovers: Vec::new(),
        }),
        blocking_programs: vec!["Surge XT.exe".to_string()],
        every_reaper_closed: true,
        requires_elevation: true,
        unbacked: true,
        confirm: true,
        // Its summary says it goes for every music program, so the plan
        // doesn't add the machine-wide warning.
        warnings: Vec::new(),
        notes: vec![UninstallNote::Manifest {
            note_key: "uninstall-note-surge-fonts-stay".to_string(),
        }],
        ..item("surge-xt", Removability::Removable)
    }
}

/// REAPER as the plan describes a standard REAPER whose own uninstaller
/// RABBIT trusts.
fn vendor_reaper() -> UninstallPlanItem {
    UninstallPlanItem {
        package_id: "reaper".to_string(),
        display_name: "REAPER".to_string(),
        summary: Some(UninstallSummary {
            removes_key: "uninstall-summary-reaper-windows-removes".to_string(),
            keeps_key: "uninstall-summary-reaper-keeps".to_string(),
        }),
        warnings: vec![UninstallWarning::DependentsStay {
            package_ids: vec!["osara".to_string(), "sws".to_string()],
        }],
        blocking_programs: Vec::new(),
        notes: Vec::new(),
        ..vendor_surge()
    }
}

/// A package its own uninstaller removes can be ticked once RABBIT trusts
/// the uninstaller. Its details say what goes, that Windows asks first, and
/// what stays, never that its files go into a backup.
#[test]
fn a_package_its_own_uninstaller_removes_can_be_ticked() {
    let row = rows_for(vec![vendor_surge()]).remove(0);
    assert!(row.can_tick, "{}", row.details);
    assert_eq!(row.unavailability_reason, None);
    for sentence in [
        "Surge XT's own uninstaller removes it for every music program on this computer. A \
         backup can't bring it back.",
        "Windows asks for your permission before anything is removed.",
        "Your own patches, in the Surge XT folder in your Documents, stay.",
        "The fonts Surge XT installed stay",
    ] {
        assert!(
            row.details.contains(sentence),
            "{sentence}: {}",
            row.details
        );
    }
    assert!(!row.details.contains("into a backup"), "{}", row.details);
    // "For every music program" is said once.
    assert_eq!(
        row.details.matches("every music program").count()
            + row.details.matches("every other program").count(),
        1,
        "{}",
        row.details
    );

    let reaper = rows_for(vec![vendor_reaper()]).remove(0);
    assert!(reaper.can_tick);
    for sentence in [
        "REAPER's own uninstaller removes the program.",
        "Your settings, extensions and projects stay.",
        "These packages install on top of REAPER and stay: OSARA, SWS Extension. They work \
         again once you install REAPER again.",
    ] {
        assert!(
            reaper.details.contains(sentence),
            "{sentence}: {}",
            reaper.details
        );
    }
    assert!(
        !reaper.details.contains("may not work"),
        "{}",
        reaper.details
    );
}

/// An uninstaller RABBIT doesn't trust leaves the row unticked, and its
/// details say what was wrong and how to remove the package without RABBIT.
#[test]
fn an_uninstaller_rabbit_cant_trust_says_why_and_how_to_remove_it_by_hand() {
    let row = rows_for(vec![item(
        "surge-xt",
        Removability::NoTrustedUninstaller {
            refusal: VendorRefusal::ForeignFile {
                path: PathBuf::from("C:/ProgramData/Surge XT/uninstall/planted.dll"),
            },
        },
    )])
    .remove(0);
    assert!(!row.can_tick);
    assert_eq!(
        row.unavailability_reason.as_deref(),
        Some("no uninstaller RABBIT can trust")
    );
    for sentence in [
        "RABBIT found no uninstaller for Surge XT it can trust.",
        "Next to Surge XT's uninstaller is a file that doesn't belong there: \
         C:/ProgramData/Surge XT/uninstall/planted.dll",
        "You can uninstall Surge XT yourself in Windows Settings, under Apps, Installed apps.",
    ] {
        assert!(
            row.details.contains(sentence),
            "{sentence}: {}",
            row.details
        );
    }
}

// ---------------------------------------------------------------------------
// The JAWS scripts: rows RABBIT leaves alone because of the uninstaller
// folder and registry entry their installer left, and the notes on the
// script files that stay.
// ---------------------------------------------------------------------------

/// Steps over several lines, standing in for a package's steps to remove it
/// by hand.
pub(super) const BY_HAND_KEY: &str = "uninstall-manual-reaper-portable";

/// One sentence, standing in for the note on removing by hand the script
/// files that stay.
const SCRIPTS_NOTE_KEY: &str = "uninstall-note-reapack-packages-stay";

/// Each line of the stand-in steps to remove a package by hand, in English.
pub(super) fn by_hand_lines_en() -> Vec<String> {
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let text = localizer.text(BY_HAND_KEY).value;
    assert!(text.lines().count() > 2, "{text}");
    text.lines().map(str::to_string).collect()
}

/// A JAWS row whose uninstaller folder or registry entry RABBIT doesn't
/// trust can't be ticked. Its details say so, then why in the user's
/// language, then how to remove the scripts by hand.
#[test]
fn a_refused_jaws_cleanup_row_says_how_to_remove_it_by_hand() {
    let row = rows_for(vec![item(
        "jaws-scripts",
        Removability::CleanupNotTrusted {
            refusal: VendorRefusal::RegistryValueMismatch {
                found: "C:/Program Files/Reaper_JawsScripts".to_string(),
            },
            by_hand_key: BY_HAND_KEY.to_string(),
        },
    )])
    .remove(0);
    assert!(!row.can_tick);
    assert_eq!(
        row.unavailability_reason.as_deref(),
        Some("remove it by hand")
    );
    let refused = "RABBIT doesn't trust what it found in Snowman's JAWS Scripts for REAPER's \
                   uninstaller folder in Program Files or in its registry entry, so it leaves \
                   Snowman's JAWS Scripts for REAPER alone.";
    let reason = "Snowman's JAWS Scripts for REAPER's registry entry points somewhere RABBIT \
                  doesn't expect: C:/Program Files/Reaper_JawsScripts";
    let expected = [refused.to_string(), reason.to_string()]
        .into_iter()
        .chain(by_hand_lines_en())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(row.details.ends_with(&expected), "{}", row.details);
}

/// A JAWS row with no receipt and none of its files in this account can't
/// be ticked: its details say to uninstall from the account that installed
/// it, then how to remove the scripts by hand.
#[test]
fn a_jaws_row_with_no_files_for_this_account_says_where_to_uninstall_it() {
    let row = rows_for(vec![item(
        "jaws-scripts",
        Removability::NoFilesForThisAccount {
            by_hand_key: BY_HAND_KEY.to_string(),
        },
    )])
    .remove(0);
    assert!(!row.can_tick);
    assert_eq!(
        row.unavailability_reason.as_deref(),
        Some("remove it by hand")
    );
    let account = "RABBIT doesn't find any of Snowman's JAWS Scripts for REAPER's files for your \
                   account. If another account installed Snowman's JAWS Scripts for REAPER, \
                   uninstall it from that account; until then RABBIT leaves its uninstaller \
                   folder in Program Files and its registry entry alone.";
    let expected = std::iter::once(account.to_string())
        .chain(by_hand_lines_en())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(row.details.ends_with(&expected), "{}", row.details);
}

/// From a portable REAPER the JAWS row reads like any package a portable
/// REAPER can't remove, then gives the steps to remove the scripts by hand
/// that the standard plan lends it.
#[test]
fn a_portable_jaws_row_shows_the_by_hand_steps() {
    let row = rows_for(vec![UninstallPlanItem {
        strategy: None,
        notes: vec![UninstallNote::Manifest {
            note_key: BY_HAND_KEY.to_string(),
        }],
        ..item("jaws-scripts", Removability::NeedsStandardTarget)
    }])
    .remove(0);
    assert!(!row.can_tick);
    assert!(
        row.summary
            .ends_with("(not available: portable REAPER target)"),
        "{}",
        row.summary
    );
    let portable = Localizer::embedded(DEFAULT_LOCALE)
        .unwrap()
        .format(
            "wizard-uninstall-details-portable",
            &[("package", "Snowman's JAWS Scripts for REAPER")],
        )
        .value;
    let expected = std::iter::once(portable)
        .chain(by_hand_lines_en())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(row.details.ends_with(&expected), "{}", row.details);
}

/// With a receipt that records none of the script files the installer
/// added, the row says they all stay, then how to remove them by hand.
#[test]
fn a_jaws_row_without_a_record_says_all_script_files_stay_and_how_to_remove_them() {
    let row = rows_for(vec![UninstallPlanItem {
        notes: vec![
            UninstallNote::NoVendorRecord,
            UninstallNote::VendorFilesByHand {
                note_key: SCRIPTS_NOTE_KEY.to_string(),
            },
        ],
        ..item("jaws-scripts", Removability::Removable)
    }])
    .remove(0);
    assert!(row.can_tick);
    let note = Localizer::embedded(DEFAULT_LOCALE)
        .unwrap()
        .text(SCRIPTS_NOTE_KEY)
        .value;
    let expected = format!(
        "RABBIT has no record of which files Snowman's JAWS Scripts for REAPER's installer \
         added, so all of them stay.\n{note}"
    );
    assert!(row.details.ends_with(&expected), "{}", row.details);
}

/// Review names the run's one permission prompt, and asks to answer a
/// window from each vendor's uninstaller, once per package. It asks to
/// close other music programs, for REAPER's own uninstaller too. And the
/// question before starting covers every removal a backup can't undo.
#[test]
fn review_names_the_one_permission_prompt_and_the_question_covers_each_package() {
    let mut list = list_for(vec![vendor_reaper(), removable_sws(), vendor_surge()]);
    for id in ["reaper", "sws", "surge-xt"] {
        tick(&mut list, id);
    }
    let preview = review(&list, Some(&target(true)), false, &[]);
    let text = preview.lines.join("\n");
    assert!(preview.can_uninstall);
    let note = "Windows asks for your permission once, before anything is removed. If you don't \
                allow it, nothing is uninstalled.";
    assert_eq!(text.matches(note).count(), 1, "{text}");
    for package in ["REAPER", "Surge XT"] {
        assert!(
            text.contains(&format!(
                "If a window from {package}'s own uninstaller appears, answer it."
            )),
            "{text}"
        );
    }
    assert_eq!(
        text.matches("Close other music programs first.").count(),
        1,
        "{text}"
    );

    // REAPER alone asks too: its own uninstaller removes ReaRoute and the
    // file associations other programs use.
    let mut reaper_only = list_for(vec![vendor_reaper()]);
    tick(&mut reaper_only, "reaper");
    let text = review(&reaper_only, Some(&target(true)), false, &[])
        .lines
        .join("\n");
    assert!(text.contains("Close other music programs first."), "{text}");
    assert!(text.contains(note), "{text}");

    // SWS alone goes into a backup, with no prompt and no question.
    let mut sws_only = list_for(vec![removable_sws()]);
    tick(&mut sws_only, "sws");
    let text = review(&sws_only, Some(&target(true)), false, &[])
        .lines
        .join("\n");
    assert!(!text.contains("permission"), "{text}");

    let plan = plan(vec![vendor_reaper(), removable_sws(), vendor_surge()]);
    let body = question(&plan, &["reaper", "sws", "surge-xt"]).unwrap();
    assert!(
        body.contains("A backup can't bring REAPER back.")
            && body.contains("A backup can't bring Surge XT back.")
            && body.ends_with("Uninstall now?"),
        "{body}"
    );
    assert!(question(&plan, &["sws"]).is_none());
}

/// A package whose uninstaller folder and registry entry the elevated
/// helper removes raises the run's one permission prompt too. But no window
/// of a vendor's own uninstaller appears, and nothing it removes serves
/// other music programs, so Review asks for neither.
#[test]
fn review_names_the_permission_prompt_for_a_cleanup_but_no_vendor_window_or_music_programs() {
    let jaws = UninstallPlanItem {
        machine_cleanup: Some(PlannedMachineCleanup {
            folder: Some(PathBuf::from("C:/Program Files (x86)/Reaper_JawsScripts")),
            files: Vec::new(),
            registry_key: None,
            by_hand_key: BY_HAND_KEY.to_string(),
        }),
        confirm: true,
        ..item("jaws-scripts", Removability::Removable)
    };
    assert!(jaws.needs_elevated_helper() && !jaws.runs_vendor_uninstaller());
    let mut list = list_for(vec![jaws]);
    tick(&mut list, "jaws-scripts");
    let text = review(&list, Some(&target(true)), false, &[])
        .lines
        .join("\n");
    assert!(
        text.contains(
            "Windows asks for your permission once, before anything is removed. If you don't \
             allow it, nothing is uninstalled."
        ),
        "{text}"
    );
    assert!(!text.contains("own uninstaller appears"), "{text}");
    assert!(
        !text.contains("Close other music programs first."),
        "{text}"
    );
}

/// Finds every vendor's uninstaller at one made-up path, as if HKLM listed
/// it and every check passed, and answers every machine cleanup with
/// `cleanup`. Nothing reads the registry.
struct FoundIt {
    uninstaller: PathBuf,
    cleanup: Result<Option<PlannedMachineCleanup>, VendorRefusal>,
}

impl FoundIt {
    /// No machine cleanup's folder or registry entry there.
    fn new(uninstaller: PathBuf) -> Self {
        Self {
            uninstaller,
            cleanup: Ok(None),
        }
    }
}

impl VendorLookup for FoundIt {
    fn locate(&self, _job: &VendorJob<'_>) -> Result<PathBuf, VendorRefusal> {
        Ok(self.uninstaller.clone())
    }

    fn machine_cleanup(
        &self,
        _job: &CleanupJob<'_>,
    ) -> Result<Option<PlannedMachineCleanup>, VendorRefusal> {
        self.cleanup.clone()
    }
}

/// The shipped plans for Surge XT and a standard REAPER on Windows, through
/// the wizard: both rows can be ticked once the lookup trusts their
/// uninstallers, and both read in every language.
#[test]
fn the_shipped_vendor_plans_read_in_every_locale() {
    let dir = tempdir().unwrap();
    let specs = builtin_package_specs(Platform::Windows);
    let detections: Vec<ComponentDetection> = specs
        .iter()
        .map(|spec| ComponentDetection {
            installed: ["reaper", "surge-xt"].contains(&spec.id.as_str()),
            version: Some(Version::parse("7.82").unwrap()),
            ..ComponentDetection::not_installed(&spec.id, &spec.display_name)
        })
        .collect();
    let mut roots = UninstallRoots::new(dir.path());
    roots.common_program_files = Some(dir.path().join("Common Files"));
    roots.program_data = Some(dir.path().join("ProgramData"));
    roots.program_files = vec![dir.path().join("Program Files")];
    roots.documents = Some(dir.path().join("Documents"));
    let app = dir
        .path()
        .join("Program Files")
        .join("REAPER (x64)")
        .join("reaper.exe");
    roots.app_dir = app.parent().map(std::path::Path::to_path_buf);
    roots.app_path = Some(app);
    let plan = build_uninstall_plan(&UninstallPlanInput {
        roots: &roots,
        platform: Platform::Windows,
        target: InstallationKind::Standard,
        specs: &specs,
        detections: &detections,
        state: None,
        remove_settings: false,
        vendor_lookup: &FoundIt::new(dir.path().join("uninstall.exe")),
        objects: &NoMacosObjects,
    });
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let mut list = uninstall_list_from_plan(&localizer, &specs, &plan);
        let listed: Vec<&str> = list
            .rows
            .iter()
            .map(|row| row.package_id.as_str())
            .collect();
        assert_eq!(listed, ["reaper", "surge-xt"], "{locale}");
        assert!(list.rows.iter().all(|row| row.can_tick), "{locale}");
        tick(&mut list, "reaper");
        tick(&mut list, "surge-xt");
        let mut texts: Vec<String> = list.rows.iter().map(|row| row.details.clone()).collect();
        texts.push(
            build_uninstall_review_preview(&localizer, Some(&target(true)), &list, false, &[])
                .lines
                .join("\n"),
        );
        texts.push(
            uninstall_confirmation_body(&localizer, &plan, &ids(&["reaper", "surge-xt"])).unwrap(),
        );
        for text in texts {
            assert!(
                !text.contains("wizard-") && !text.contains("uninstall-") && !text.contains('{'),
                "{locale}: {text}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// macOS: the Trash, the administrator command, and apps moved by hand.
// ---------------------------------------------------------------------------

pub(super) const MAC_RESOURCE: &str = "/Users/me/Library/Application Support/REAPER";
pub(super) const MAC_REAPER_APP: &str = "/Applications/REAPER.app";

pub(super) fn mac_plan(items: Vec<UninstallPlanItem>) -> UninstallPlan {
    UninstallPlan {
        resource_path: PathBuf::from(MAC_RESOURCE),
        platform: Platform::MacOs,
        target: InstallationKind::Standard,
        remove_settings: false,
        items,
    }
}

pub(super) fn mac_list_in(
    localizer: &Localizer,
    items: Vec<UninstallPlanItem>,
) -> WizardUninstallList {
    uninstall_list_from_plan(
        localizer,
        &builtin_package_specs(Platform::MacOs),
        &mac_plan(items),
    )
}

fn mac_list(items: Vec<UninstallPlanItem>) -> WizardUninstallList {
    mac_list_in(&Localizer::embedded(DEFAULT_LOCALE).unwrap(), items)
}

/// Surge XT as the macOS plan describes it: deleted for good by the run's
/// one administrator command, with its settings only listed as kept, a
/// question first, and the machine-wide warning the plan adds.
pub(super) fn admin_surge() -> UninstallPlanItem {
    UninstallPlanItem {
        summary: Some(UninstallSummary {
            removes_key: "uninstall-summary-surge-macos-removes".to_string(),
            keeps_key: "uninstall-summary-surge-keeps".to_string(),
        }),
        backup: UninstallBackup::None,
        files: vec![tree("/Applications/Surge XT.app")],
        kept: vec![kept(
            "/Users/me/Library/Application Support/Surge Synth Team/Surge XT",
            "uninstall-kept-surge-settings-macos",
            false,
        )],
        blocking_programs: vec!["Surge XT".to_string(), "Surge XT Effects".to_string()],
        macos_pkg_forget: vec!["org.surge-synth-team.surge-xt.app.pkg".to_string()],
        every_reaper_closed: true,
        requires_elevation: true,
        unbacked: true,
        confirm: true,
        warnings: vec![UninstallWarning::MachineWide],
        ..item("surge-xt", Removability::Removable)
    }
}

/// REAPER.app as the macOS plan describes one the user may move: to the
/// Trash, as the user, with no password.
pub(super) fn trash_reaper() -> UninstallPlanItem {
    UninstallPlanItem {
        summary: Some(UninstallSummary {
            removes_key: "uninstall-summary-reaper-macos-removes".to_string(),
            keeps_key: "uninstall-summary-reaper-keeps".to_string(),
        }),
        backup: UninstallBackup::ToTrash,
        files: vec![tree(MAC_REAPER_APP)],
        unbacked: true,
        confirm: true,
        ..item("reaper", Removability::Removable)
    }
}

/// REAPER.app the user's account may not move: the administrator command
/// moves it into the user's Trash, with every REAPER closed.
pub(super) fn root_moved_reaper() -> UninstallPlanItem {
    UninstallPlanItem {
        requires_elevation: true,
        every_reaper_closed: true,
        ..trash_reaper()
    }
}

/// REAPER.app RABBIT can't move at all, for `reason`.
fn by_hand_reaper(reason: ByHandReason) -> UninstallPlanItem {
    UninstallPlanItem {
        removability: Removability::ByHand {
            path: PathBuf::from(MAC_REAPER_APP),
            reason,
        },
        ..trash_reaper()
    }
}

fn tree(path: &str) -> PlannedRemoval {
    PlannedRemoval {
        path: PathBuf::from(path),
        reason: RemovalReason::Tree,
        size: None,
        is_dir: true,
        receipt_sha256: None,
        bundle_id: None,
        guard: Default::default(),
    }
}

/// Each macOS route says how the package leaves, never that it goes into
/// a backup: the administrator command deletes it after one password
/// window; the Trash keeps it until it is emptied; an app only root can
/// move keeps its owner there. The Windows sentences stay out.
#[test]
fn macos_rows_say_how_each_package_leaves() {
    let list = mac_list(vec![admin_surge()]);
    let surge = &list.rows[0];
    assert!(surge.can_tick, "{}", surge.details);
    for sentence in [
        "Removes Surge XT's apps and plug-ins, and its factory data in the Library folder. A \
         backup can't bring them back.",
        "macOS asks once for an administrator's name and password, before anything is removed. \
         To have it again, you install it again.",
        "Your own patches, in the Surge XT folder in your Documents, stay.",
        "Surge XT serves every REAPER and every other program on this computer",
    ] {
        assert!(
            surge.details.contains(sentence),
            "{sentence}: {}",
            surge.details
        );
    }
    // The summary says a backup can't bring it back; the route's line
    // doesn't say it again.
    assert_eq!(
        surge.details.matches("backup").count(),
        1,
        "{}",
        surge.details
    );
    // Root's removals never take the user's settings.
    assert!(surge.settings.is_empty(), "{:?}", surge.settings);

    let reaper_list = mac_list(vec![trash_reaper()]);
    let reaper = &reaper_list.rows[0];
    assert!(reaper.can_tick, "{}", reaper.details);
    for sentence in [
        "Moves the REAPER app to the Trash.",
        "Until you empty the Trash, you can drag it back from there.",
        "Your settings, extensions and projects stay.",
    ] {
        assert!(
            reaper.details.contains(sentence),
            "{sentence}: {}",
            reaper.details
        );
    }
    // The summary says where it goes; the route's line doesn't say it again.
    assert_eq!(
        reaper.details.matches("to the Trash").count(),
        1,
        "{}",
        reaper.details
    );
    assert!(!reaper.details.contains("password"), "{}", reaper.details);

    let root_list = mac_list(vec![root_moved_reaper()]);
    let root = &root_list.rows[0];
    assert!(root.can_tick, "{}", root.details);
    assert!(
        root.details.contains(
            "Your account may not move it, so macOS asks once for an administrator's name and \
             password, before anything is removed, and RABBIT then moves it into your Trash. It \
             keeps the owner it had, so Finder offers no Put Back for it, and emptying the Trash \
             may ask for an administrator's password."
        ),
        "{}",
        root.details
    );

    for row in [surge, reaper, root] {
        for windows in ["into a backup rather than", "Windows"] {
            assert!(!row.details.contains(windows), "{windows}: {}", row.details);
        }
    }
}

/// Review names only the extensions that really stay on top of REAPER: one
/// ticked along with it goes in the same run, and with every one ticked the
/// line goes. REAPER's own details, about REAPER alone, still name them all.
#[test]
fn review_says_only_the_extensions_left_behind_stay_on_top_of_reaper() {
    let reaper = UninstallPlanItem {
        warnings: vec![UninstallWarning::DependentsStay {
            package_ids: vec!["osara".to_string(), "sws".to_string()],
        }],
        ..trash_reaper()
    };
    let mut list = mac_list(vec![
        reaper,
        item("osara", Removability::Removable),
        removable_sws(),
    ]);
    let row = list
        .rows
        .iter()
        .find(|row| row.package_id == "reaper")
        .unwrap();
    assert!(
        row.details
            .contains("These packages install on top of REAPER and stay: OSARA, SWS Extension."),
        "{}",
        row.details
    );

    tick(&mut list, "reaper");
    tick(&mut list, "osara");
    let text = review(&list, Some(&target(true)), false, &[])
        .lines
        .join("\n");
    assert!(
        text.contains(
            "These packages install on top of REAPER and stay: SWS Extension. They work again \
             once you install REAPER again."
        ),
        "{text}"
    );

    tick(&mut list, "sws");
    let text = review(&list, Some(&target(true)), false, &[])
        .lines
        .join("\n");
    assert!(!text.contains("install on top of REAPER"), "{text}");
}

/// An app RABBIT can't move to the Trash, by itself or with a password,
/// can't be ticked. Its row says to move it in Finder, and its details say
/// why, then the Finder steps for that reason: a Locked app is unlocked
/// first, and one that isn't REAPER is only the user's to remove if it is.
#[test]
fn an_app_only_finder_can_move_says_why_and_how() {
    let finder = "To remove REAPER, select it in Finder and press Command-Delete. Finder asks for \
                  an administrator's name and password if it needs one.";
    for (reason, why, steps) in [
        (
            ByHandReason::NotMovableByYou,
            "Your account may not move /Applications/REAPER.app: it belongs to another account \
             or to macOS, or its folder is one you can't change.",
            finder,
        ),
        (
            ByHandReason::Locked,
            "/Applications/REAPER.app is locked.",
            "To remove REAPER, select it in Finder and choose File > Get Info. Untick Locked and \
             close that window, then press Command-Delete.",
        ),
        (
            ByHandReason::NotTheExpectedApp,
            "/Applications/REAPER.app isn't the REAPER app RABBIT expects there, so RABBIT \
             leaves it alone.",
            "If it is the REAPER you want to remove, select it in Finder and press \
             Command-Delete.",
        ),
        (
            ByHandReason::CouldNotCheck {
                error: "Permission denied".to_string(),
            },
            "RABBIT couldn't check who may move /Applications/REAPER.app (Permission denied).",
            finder,
        ),
        (
            ByHandReason::NoTrashFolder,
            "Only an administrator can move /Applications/REAPER.app, and your Trash isn't a \
             folder of your own that RABBIT can check.",
            finder,
        ),
        (
            ByHandReason::NotOnTrashVolume,
            "Only an administrator can move /Applications/REAPER.app, and it is on another disk \
             than your Trash.",
            finder,
        ),
    ] {
        let list = mac_list(vec![by_hand_reaper(reason.clone())]);
        let row = &list.rows[0];
        assert!(!row.can_tick, "{reason:?}");
        assert_eq!(
            row.unavailability_reason.as_deref(),
            Some("move it to the Trash in Finder"),
            "{reason:?}"
        );
        assert!(
            row.summary
                .ends_with("(not available: move it to the Trash in Finder)"),
            "{}",
            row.summary
        );
        let (Some(at_why), Some(at_steps)) = (row.details.find(why), row.details.find(steps))
        else {
            panic!("{reason:?}: {}", row.details);
        };
        assert!(at_why < at_steps, "the reason comes first: {}", row.details);
    }
    // Get Info's Locked clears only the lock a user sets, so the steps
    // don't promise to work every time.
    let list = mac_list(vec![by_hand_reaper(ByHandReason::Locked)]);
    assert!(
        list.rows[0].details.contains(
            "If Locked isn't ticked, or Finder still can't move it, it has a lock that Finder \
             can't remove."
        ),
        "{}",
        list.rows[0].details
    );
}

/// Review names macOS's one password window once, for every package the
/// administrator command removes: that it may name osascript, that Touch
/// ID may stand in for the password, and that cancelling it uninstalls
/// nothing. What goes to the Trash says it can come back from there. A
/// REAPER the user may move asks for no password at all.
#[test]
fn macos_review_names_the_one_password_window_and_the_trash() {
    let mut list = mac_list(vec![trash_reaper(), admin_surge()]);
    tick(&mut list, "reaper");
    tick(&mut list, "surge-xt");
    let preview = review(&list, Some(&target(true)), false, &[]);
    let text = preview.lines.join("\n");
    assert!(preview.can_uninstall);
    let note = "Before anything is removed, macOS asks once for an administrator's name and \
                password. It may offer Touch ID instead. The window may name osascript, the tool \
                RABBIT asks through. With that password, RABBIT runs one administrator command \
                that uninstalls Surge XT. If you cancel the window, nothing is uninstalled.";
    assert_eq!(text.matches(note).count(), 1, "{text}");
    assert_eq!(
        text.matches("Close other music programs first.").count(),
        1,
        "{text}"
    );
    assert!(
        text.contains(
            "REAPER goes to the Trash. Until you empty the Trash, you can drag it back from \
             there."
        ),
        "{text}"
    );
    assert!(!text.contains("Windows"), "{text}");

    // The administrator command moves REAPER too: one window for both.
    let mut list = mac_list(vec![root_moved_reaper(), admin_surge()]);
    tick(&mut list, "reaper");
    tick(&mut list, "surge-xt");
    let text = review(&list, Some(&target(true)), false, &[])
        .lines
        .join("\n");
    assert_eq!(
        text.matches("macOS asks once for an administrator's name and password")
            .count(),
        1,
        "{text}"
    );
    assert!(
        text.contains("one administrator command that uninstalls REAPER, Surge XT."),
        "{text}"
    );
    assert!(
        text.contains(
            "REAPER goes into your Trash with the administrator's password. It keeps the owner \
             it had, so Finder offers no Put Back for it"
        ),
        "{text}"
    );

    // REAPER alone, moved as the user: no password, nothing else to close.
    let mut list = mac_list(vec![trash_reaper()]);
    tick(&mut list, "reaper");
    let text = review(&list, Some(&target(true)), false, &[])
        .lines
        .join("\n");
    assert!(!text.contains("password"), "{text}");
    assert!(!text.contains("Close other music programs"), "{text}");
}

/// Going to the Trash asks first, in words that fit: the app can come back
/// from the Trash, unlike what the administrator command deletes, which no
/// backup brings back. An app only root can move may need an
/// administrator's password to come back out.
#[test]
fn an_app_going_to_the_trash_asks_with_its_own_sentence() {
    let plan = mac_plan(vec![trash_reaper(), admin_surge()]);
    assert_eq!(
        uninstall_confirm_reasons(&plan, &ids(&["reaper", "surge-xt"])),
        [
            UninstallConfirmReason::ToTrash {
                package_id: "reaper".to_string(),
                by_administrator: false,
            },
            UninstallConfirmReason::Unbacked {
                package_id: "surge-xt".to_string(),
            },
        ]
    );
    assert_eq!(
        question(&plan, &["reaper", "surge-xt"]).as_deref(),
        Some(
            "REAPER goes to the Trash. You can drag it back from there until you empty the \
             Trash.\n\nA backup can't bring Surge XT back. To have it again, you install it \
             again.\n\nUninstall now?"
        )
    );

    let plan = mac_plan(vec![root_moved_reaper()]);
    assert_eq!(
        uninstall_confirm_reasons(&plan, &ids(&["reaper"])),
        [UninstallConfirmReason::ToTrash {
            package_id: "reaper".to_string(),
            by_administrator: true,
        }]
    );
    let body = question(&plan, &["reaper"]).unwrap();
    assert!(
        body.contains("but macOS may ask for an administrator's password for that.")
            && !body.contains("A backup can't bring REAPER back"),
        "{body}"
    );
}

/// The shipped macOS REAPER plan on a stand-in Mac in a temporary folder,
/// with REAPER.app in its `/Applications` described by `app` and the
/// user's Trash a folder of theirs on the same disk. Nothing here reads or
/// moves a real app.
fn shipped_mac_reaper_plan(app: ObjectFacts) -> (tempfile::TempDir, UninstallPlan) {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let system = dir.path().join("system");
    let applications = system.join("Applications");
    let reaper_app = applications.join("REAPER.app");
    std::fs::create_dir_all(reaper_app.join("Contents")).unwrap();
    std::fs::create_dir_all(&resource).unwrap();
    let mut roots = UninstallRoots::new(&resource);
    roots.system_root = system;
    roots.app_path = Some(reaper_app.clone());
    roots.app_dir = Some(applications);
    let objects = FakeMacosObjects::new()
        .with_object(&reaper_app, app)
        .with_trash(TrashFolderFacts {
            path: PathBuf::from("/Users/me/.Trash"),
            is_dir: true,
            is_symlink: false,
            owner_uid: 501,
            owned_by_me: true,
            dev: 1,
        });
    let specs = builtin_package_specs(Platform::MacOs);
    let detections = vec![ComponentDetection {
        installed: true,
        version: Some(Version::parse("7.82").unwrap()),
        ..ComponentDetection::not_installed("reaper", "REAPER")
    }];
    let plan = build_uninstall_plan(&UninstallPlanInput {
        roots: &roots,
        platform: Platform::MacOs,
        target: InstallationKind::Standard,
        specs: &specs,
        detections: &detections,
        state: None,
        remove_settings: false,
        vendor_lookup: &NothingListed,
        objects: &objects,
    });
    (dir, plan)
}

/// REAPER.app as macOS describes it to the user who may move it, or, with
/// `mine` false, to a user only root lets move it.
fn mac_reaper_app(mine: bool) -> ObjectFacts {
    ObjectFacts {
        is_dir: true,
        is_symlink: false,
        owner_uid: if mine { 501 } else { 0 },
        owned_by_me: mine,
        writable: mine,
        parent_writable_searchable: true,
        removable_by_me: mine,
        parent_sticky: false,
        parent_owned_by_me: false,
        locked: false,
        parent_locked: false,
        dev: 1,
        bundle_identifier: Some("com.cockos.reaper".to_string()),
    }
}

/// REAPER from the shipped macOS plan, as the wizard shows it: its row can
/// be ticked on the Trash route and on root's, its details say where it
/// goes and what stays, and its row, Review and question read in every
/// language.
#[test]
fn the_shipped_macos_reaper_can_be_ticked_and_reads_in_every_locale() {
    for mine in [true, false] {
        let (_dir, plan) = shipped_mac_reaper_plan(mac_reaper_app(mine));
        for &locale in embedded_locales() {
            let localizer = Localizer::embedded(locale).unwrap();
            let mut list = uninstall_list_from_plan(
                &localizer,
                &builtin_package_specs(Platform::MacOs),
                &plan,
            );
            let row = list
                .rows
                .iter()
                .find(|row| row.package_id == "reaper")
                .unwrap();
            assert!(row.can_tick, "{locale}, mine {mine}: {}", row.details);
            assert!(
                row.details.contains(
                    &localizer
                        .text("uninstall-summary-reaper-macos-removes")
                        .value
                ),
                "{locale}: {}",
                row.details
            );
            assert!(
                row.details
                    .contains(&localizer.text("uninstall-summary-reaper-keeps").value),
                "{locale}: {}",
                row.details
            );
            let mut texts = vec![row.summary.clone(), row.details.clone()];
            tick(&mut list, "reaper");
            let preview =
                build_uninstall_review_preview(&localizer, Some(&target(true)), &list, false, &[]);
            assert!(preview.can_uninstall, "{locale}: {:?}", preview.lines);
            texts.push(preview.lines.join("\n"));
            texts.push(uninstall_confirmation_body(&localizer, &plan, &ids(&["reaper"])).unwrap());
            for text in texts {
                assert!(
                    !text.contains("wizard-")
                        && !text.contains("uninstall-")
                        && !text.contains('{'),
                    "{locale}: {text}"
                );
            }
            if locale == DEFAULT_LOCALE {
                // Only root's route asks for a password.
                let details = &list.rows[0].details;
                assert_eq!(details.contains("password"), !mine, "{details}");
                // There is no receipt, but the app is found by its name
                // and bundle ID alone: nothing says it "removes the files".
                assert!(!details.contains("didn't install"), "{details}");
            }
        }
    }
}

/// Surge XT from the shipped macOS plan, on a stand-in Mac in a temporary
/// folder holding its app and its VST3: its row can be ticked, its details
/// say what goes and what stays, that it goes for every program and that
/// macOS asks for an administrator's password, and its row, Review and
/// question read in every language. Nothing here deletes anything.
#[test]
fn the_shipped_macos_surge_xt_can_be_ticked_and_reads_in_every_locale() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let system = dir.path().join("system");
    for tree in [
        system.join("Applications").join("Surge XT.app"),
        system
            .join("Library")
            .join("Audio")
            .join("Plug-Ins")
            .join("VST3")
            .join("Surge XT.vst3"),
    ] {
        std::fs::create_dir_all(tree.join("Contents")).unwrap();
    }
    std::fs::create_dir_all(&resource).unwrap();
    let mut roots = UninstallRoots::new(&resource);
    roots.system_root = system;
    let specs = builtin_package_specs(Platform::MacOs);
    let detections = vec![ComponentDetection {
        installed: true,
        version: Some(Version::parse("1.4.0").unwrap()),
        ..ComponentDetection::not_installed("surge-xt", "Surge XT")
    }];
    let plan = build_uninstall_plan(&UninstallPlanInput {
        roots: &roots,
        platform: Platform::MacOs,
        target: InstallationKind::Standard,
        specs: &specs,
        detections: &detections,
        state: None,
        remove_settings: false,
        vendor_lookup: &NothingListed,
        objects: &NoMacosObjects,
    });
    assert_eq!(
        plan.item("surge-xt").map(|item| item.files.len()),
        Some(2),
        "{:?}",
        plan.item("surge-xt")
    );

    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let mut list = uninstall_list_from_plan(&localizer, &specs, &plan);
        let row = list
            .rows
            .iter()
            .find(|row| row.package_id == "surge-xt")
            .unwrap();
        assert!(row.can_tick, "{locale}: {}", row.details);
        for key in [
            "uninstall-summary-surge-macos-removes",
            "uninstall-summary-surge-keeps",
            "wizard-uninstall-details-elevated-macos",
        ] {
            assert!(
                row.details.contains(&localizer.text(key).value),
                "{locale}: {key}: {}",
                row.details
            );
        }
        // Its root's data is never "Also remove settings" material.
        assert!(row.settings.is_empty(), "{locale}: {:?}", row.settings);
        let mut texts = vec![row.summary.clone(), row.details.clone()];
        tick(&mut list, "surge-xt");
        let preview =
            build_uninstall_review_preview(&localizer, Some(&target(true)), &list, false, &[]);
        assert!(preview.can_uninstall, "{locale}: {:?}", preview.lines);
        texts.push(preview.lines.join("\n"));
        let question = uninstall_confirmation_body(&localizer, &plan, &ids(&["surge-xt"]))
            .expect("Surge XT asks before it goes");
        texts.push(question);
        for text in texts {
            assert!(
                !text.contains("wizard-") && !text.contains("uninstall-") && !text.contains('{'),
                "{locale}: {text}"
            );
        }
        if locale == DEFAULT_LOCALE {
            let details = &list.rows[0].details;
            assert!(
                details.contains(
                    "Surge XT serves every REAPER and every other program on this computer"
                ),
                "{details}"
            );
            let review = preview.lines.join("\n");
            assert!(
                review.contains("administrator's name and password"),
                "{review}"
            );
            // Found by its places and names alone: nothing says RABBIT
            // didn't install it.
            assert!(!details.contains("didn't install"), "{details}");
        }
    }
}

/// Every macOS row, Review and question reads in every language, with no
/// key or placeholder left over, the manifest keys only the shipped macOS
/// plans name included.
#[test]
fn macos_rows_review_and_question_read_in_every_locale() {
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let mut texts = Vec::new();
        for reaper in [trash_reaper(), root_moved_reaper()] {
            let mut list = mac_list_in(&localizer, vec![reaper, admin_surge()]);
            texts.extend(list.rows.iter().map(|row| row.details.clone()));
            tick(&mut list, "reaper");
            tick(&mut list, "surge-xt");
            texts.push(
                build_uninstall_review_preview(&localizer, Some(&target(true)), &list, false, &[])
                    .lines
                    .join("\n"),
            );
            texts.push(
                uninstall_confirmation_body(
                    &localizer,
                    list.plan.as_ref().unwrap(),
                    &ids(&["reaper", "surge-xt"]),
                )
                .unwrap(),
            );
        }
        for reason in [
            ByHandReason::NotMovableByYou,
            ByHandReason::Locked,
            ByHandReason::NotTheExpectedApp,
            ByHandReason::CouldNotCheck {
                error: "denied".to_string(),
            },
            ByHandReason::NoTrashFolder,
            ByHandReason::NotOnTrashVolume,
        ] {
            let list = mac_list_in(&localizer, vec![by_hand_reaper(reason)]);
            texts.push(list.rows[0].summary.clone());
            texts.push(list.rows[0].details.clone());
        }
        for key in [
            "uninstall-kept-surge-settings-macos",
            "uninstall-kept-surge-factory-copy-macos",
        ] {
            let kept = localizer.text(key);
            assert!(!kept.missing && !kept.fallback_used, "{locale}: {key}");
            texts.push(kept.value);
        }
        for text in texts {
            assert!(
                !text.contains("wizard-") && !text.contains("uninstall-") && !text.contains('{'),
                "{locale}: {text}"
            );
        }
    }
}
