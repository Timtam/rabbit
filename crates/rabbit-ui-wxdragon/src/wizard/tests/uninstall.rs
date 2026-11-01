use std::path::PathBuf;

use rabbit_core::localization::{DEFAULT_LOCALE, Localizer, embedded_locales};
use rabbit_core::model::{Architecture, ComponentDetection, InstallationKind, Platform};
use rabbit_core::package::uninstall::{UninstallStrategy, UninstallSummary};
use rabbit_core::package::{PACKAGE_REAPACK, PackageCategory, builtin_package_specs};
use rabbit_core::plan::{InstallPlan, PlanAction, PlanActionKind};
use rabbit_core::receipt::{InstallState, PackageReceipt};
use rabbit_core::uninstall::{
    KeptPath, KeptReason, PlannedRemoval, Removability, RemovalReason, UNINSTALL_NOT_YET_KEY,
    UninstallNote, UninstallPlan, UninstallPlanInput, UninstallPlanItem, UninstallRoots,
    UninstallWarning, build_uninstall_plan,
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
/// a backup can't undo, and OSARA, whose manifest asks before REAPER stops
/// speaking.
fn plan_with_questions() -> UninstallPlan {
    plan(vec![
        UninstallPlanItem {
            warnings: vec![UninstallWarning::Always {
                message_key: "uninstall-warning-sws-osara-keymap".to_string(),
                confirm: true,
            }],
            confirm: true,
            ..item("osara", Removability::Removable)
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
        uninstall_confirm_reasons(&plan, &ids(&["osara"])),
        [UninstallConfirmReason::Warning {
            package_id: "osara".to_string(),
            message_key: "uninstall-warning-sws-osara-keymap".to_string(),
        }]
    );
    let body = question(&plan, &["osara", "surge-xt"]).unwrap();
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
            uninstall_confirmation_body(&localizer, &plan, &ids(&["osara", "surge-xt"])).unwrap();
        assert!(
            !body.contains("wizard-") && !body.contains("uninstall-") && !body.contains('{'),
            "{locale}: {body}"
        );
        assert!(body.contains("Surge XT"), "{locale}: {body}");
    }
}

/// Every package RABBIT can remove so far is a file removal that goes into
/// a backup with no warning that asks, so none of them asks a question.
/// The question is for the packages later phases add.
#[test]
fn no_package_the_wizard_can_remove_yet_asks_a_question() {
    let dir = tempdir().unwrap();
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
        let roots = UninstallRoots::for_target(platform, dir.path(), None);
        let plan = build_uninstall_plan(&UninstallPlanInput {
            roots: &roots,
            platform,
            target: InstallationKind::Standard,
            specs: &specs,
            detections: &detections,
            state: Some(&state),
            remove_settings: false,
        });
        let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
        let tickable: Vec<String> = uninstall_list_from_plan(&localizer, &specs, &plan)
            .rows
            .into_iter()
            .filter(|row| row.can_tick)
            .map(|row| row.package_id)
            .collect();
        // Every Phase 1 package, so the coverage can't quietly shrink.
        let mut phase_one = vec![
            "sws",
            "reapack",
            "reakontrol",
            "langpack-es",
            "langpack-de",
            "langpack-fr",
        ];
        if platform == Platform::Windows {
            phase_one.push("ffmpeg");
            // app2clap lives in Windows' own CLAP folder, which only a
            // Windows machine can name; planning for Windows elsewhere
            // rightly blocks it.
            if cfg!(windows) {
                phase_one.push("app2clap");
            }
        }
        for id in phase_one {
            assert!(
                tickable.iter().any(|ticked| ticked == id),
                "{platform:?}: {id} isn't in {tickable:?}"
            );
        }
        assert_eq!(
            uninstall_confirmation_body(&localizer, &plan, &tickable),
            None,
            "{platform:?}: {tickable:?}"
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
            ]),
        );
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
