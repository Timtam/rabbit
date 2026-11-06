//! Running an uninstall from the wizard: the progress page's bookkeeping, the
//! request, the checks before it starts, and what Done says afterwards.

use std::path::{Path, PathBuf};

use rabbit_core::RabbitError;
use rabbit_core::localization::{DEFAULT_LOCALE, Localizer, embedded_locales};
use rabbit_core::model::{ComponentDetection, InstallationKind, Platform};
use rabbit_core::plan::{AvailablePackage, PlanActionKind};
use rabbit_core::preflight::{
    PreflightCheck, PreflightReport, PreflightStatus, REAPER_PROCESS_CHECK,
};
use rabbit_core::progress::{ProgressEvent, ProgressReporter, RemovalMethod};
use rabbit_core::report::SavedReportPaths;
use rabbit_core::uninstall::{
    ANY_REAPER_PROCESS_CHECK, BACKUP_SPACE_CHECK, KeptFile, KeptFileReason, LeftoverKind,
    LeftoverOutcome, LeftoverResult, NothingListed, PROGRAMS_RUNNING_CHECK, RegistryEntryAction,
    RegistryEntryOutcome, Removability, RemovalReason, RemovedFile, UninstallMessage,
    UninstallPlanItem, UninstallProblem, UninstallReport, UninstallReportItem, UninstallRequest,
    UninstallServices, UninstallStatus, VendorOutcome, VendorRefusal, VendorStatus,
    execute_uninstall,
};
use rabbit_core::version::Version;
use tempfile::tempdir;

use super::support::*;
use super::uninstall::{item, list_for, plan, removable_sws, sws_with_settings, target, tick};
use crate::wizard::*;

fn en() -> Localizer {
    Localizer::embedded(DEFAULT_LOCALE).unwrap()
}

// ---------------------------------------------------------------------------
// The progress page's bookkeeping.
// ---------------------------------------------------------------------------

fn started(id: &str) -> ProgressEvent {
    ProgressEvent::RemoveStarted {
        package_id: id.to_string(),
        method: RemovalMethod::Files,
    }
}

fn still_waiting(id: &str) -> ProgressEvent {
    ProgressEvent::VendorStillWaiting {
        package_id: id.to_string(),
        seconds: 30,
    }
}

fn completed(id: &str, removed: bool) -> ProgressEvent {
    ProgressEvent::RemoveCompleted {
        package_id: id.to_string(),
        removed,
    }
}

/// One phase per package: a two-package uninstall reaches 100, halfway
/// after the first, instead of parking near 50 the way two phases per
/// package would.
#[test]
fn a_two_package_uninstall_fills_the_gauge() {
    let mut state = ProgressUiState::new(WizardMode::Uninstall, 2, 0);
    assert_eq!(state.total_phases(), 2);

    let mut seen = Vec::new();
    for event in [
        started("sws"),
        completed("sws", true),
        started("reapack"),
        completed("reapack", true),
    ] {
        state.record(&event);
        seen.push(state.percentage());
    }

    assert_eq!(seen, [0, 50, 50, 100]);
}

#[test]
fn a_removal_that_failed_still_counts_its_phase() {
    let mut state = ProgressUiState::new(WizardMode::Uninstall, 1, 0);
    state.record(&started("sws"));
    assert_eq!(state.current_package.as_deref(), Some("sws"));

    state.record(&completed("sws", false));

    assert_eq!(state.current_package, None);
    assert_eq!(state.percentage(), 100);
}

#[test]
fn an_install_still_counts_a_download_and_an_install_per_package() {
    let mut state = ProgressUiState::new(WizardMode::Install, 1, 1);
    assert_eq!(state.total_phases(), 3);

    state.record(&ProgressEvent::DownloadStarted {
        package_id: "osara".to_string(),
        bytes_total: Some(100),
    });
    state.record(&ProgressEvent::DownloadProgress {
        package_id: "osara".to_string(),
        bytes_downloaded: 50,
        bytes_total: Some(100),
    });
    assert_eq!(state.percentage(), 17);
    state.record(&ProgressEvent::DownloadCompleted {
        package_id: "osara".to_string(),
    });
    assert!(state.active_downloads.is_empty());
    state.record(&ProgressEvent::InstallStarted {
        package_id: "osara".to_string(),
    });
    assert_eq!(state.current_package.as_deref(), Some("osara"));
    state.record(&ProgressEvent::InstallCompleted {
        package_id: "osara".to_string(),
    });
    state.record(&ProgressEvent::ConfigurationCompleted {
        step_id: "step".to_string(),
    });

    assert_eq!(state.percentage(), 100);
}

/// Every event, and whether its status line is spoken. Starting a step and
/// both ends of a removal are; the download ticks and an install's end
/// aren't.
#[test]
fn the_spoken_events_are_the_starts_and_both_ends_of_a_removal() {
    let id = || "sws".to_string();
    let events = [
        (
            ProgressEvent::DownloadStarted {
                package_id: id(),
                bytes_total: None,
            },
            false,
        ),
        (
            ProgressEvent::DownloadProgress {
                package_id: id(),
                bytes_downloaded: 1,
                bytes_total: None,
            },
            false,
        ),
        (ProgressEvent::DownloadCompleted { package_id: id() }, false),
        (ProgressEvent::InstallStarted { package_id: id() }, true),
        (ProgressEvent::InstallCompleted { package_id: id() }, false),
        (ProgressEvent::ConfigurationStarted { step_id: id() }, true),
        (
            ProgressEvent::ConfigurationCompleted { step_id: id() },
            false,
        ),
        (started("sws"), true),
        (
            ProgressEvent::RemoveStarted {
                package_id: id(),
                method: RemovalMethod::VendorUninstaller,
            },
            true,
        ),
        (still_waiting("surge-xt"), true),
        (completed("sws", true), true),
        (completed("sws", false), true),
    ];
    for (event, spoken) in events {
        assert_eq!(is_spoken(&event), spoken, "{event:?}");
    }
}

/// A vendor's uninstaller still running is heard once per run: the first
/// time moves focus to the log on Windows, where the screen reader reads
/// the line just added; later ones only add their line. It counts no phase
/// of its own, so the gauge still reaches 100.
#[test]
fn still_waiting_moves_focus_to_the_log_only_the_first_time() {
    let mut state = ProgressUiState::new(WizardMode::Uninstall, 2, 0);
    let mut moves = Vec::new();
    for event in [
        started("surge-xt"),
        still_waiting("surge-xt"),
        completed("surge-xt", true),
        started("reaper"),
        still_waiting("reaper"),
        completed("reaper", true),
    ] {
        state.record(&event);
        moves.push(state.moves_focus_to_log(&event));
    }
    assert_eq!(moves, [false, true, false, false, false, false]);
    assert_eq!(state.percentage(), 100);
}

// ---------------------------------------------------------------------------
// The request.
// ---------------------------------------------------------------------------

#[test]
fn the_request_takes_the_ticked_packages_and_the_target_row() {
    let (_, model) = langpack_model(PlanActionKind::Install);
    let target = target(true);
    let mut list = list_for(vec![
        removable_sws(),
        item("reapack", Removability::Removable),
        item("langpack-de", Removability::Removable),
    ]);
    tick(&mut list, "langpack-de");
    tick(&mut list, "sws");

    let request = uninstall_request_for_target(&model, &target, &list, false).unwrap();

    assert_eq!(request.package_ids, ["sws", "langpack-de"]);
    assert_eq!(request.resource_path, target.path);
    assert_eq!(request.platform, model.platform);
    assert_eq!(request.target_app_path, target.app_path);
    assert_eq!(request.target, Some(InstallationKind::Portable));
    assert!(!request.dry_run);
    assert!(!request.remove_settings);

    let standard = TargetRow {
        portable: false,
        ..target
    };
    let request = uninstall_request_for_target(&model, &standard, &list, false).unwrap();
    assert_eq!(request.target, Some(InstallationKind::Standard));
}

/// "Also remove settings" counts only while a ticked package has settings
/// RABBIT may remove, whatever the checkbox says.
#[test]
fn the_request_removes_settings_only_where_the_option_applies() {
    let (_, model) = langpack_model(PlanActionKind::Install);
    let mut list = list_for(vec![
        sws_with_settings(),
        item("langpack-de", Removability::Removable),
    ]);
    tick(&mut list, "langpack-de");
    let request = uninstall_request_for_target(&model, &target(true), &list, true).unwrap();
    assert!(!request.remove_settings);

    tick(&mut list, "sws");
    let request = uninstall_request_for_target(&model, &target(true), &list, true).unwrap();
    assert!(request.remove_settings);
}

#[test]
fn no_request_without_a_writable_target_or_a_tick() {
    let (_, model) = langpack_model(PlanActionKind::Install);
    let mut list = list_for(vec![removable_sws()]);
    assert!(matches!(
        uninstall_request_for_target(&model, &target(true), &list, false),
        Err(RabbitError::PreflightFailed { .. })
    ));
    tick(&mut list, "sws");
    assert!(matches!(
        uninstall_request_for_target(&model, &target(false), &list, false),
        Err(RabbitError::PreflightFailed { .. })
    ));
}

/// From the Uninstall tab to the engine and back, in a temporary REAPER
/// folder: the wizard's request is one the engine carries out, the Done
/// page's details name the backup and the command that brings SWS back,
/// and the rebuilt Install tab offers SWS again, unticked.
#[test]
fn the_wizard_request_runs_in_the_engine_and_the_lists_follow() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    std::fs::create_dir_all(resource.join("UserPlugins")).unwrap();
    let plugin = resource.join("UserPlugins").join("reaper_sws-x64.dll");
    std::fs::write(&plugin, b"sws").unwrap();
    std::fs::write(resource.join("S&M.ini"), b"[sws]").unwrap();
    let (_, model) = langpack_model(PlanActionKind::Install);
    let target = TargetRow {
        path: resource.clone(),
        app_path: Some(resource.join("reaper.exe")),
        planned_app_path: resource.join("reaper.exe"),
        ..model.target_rows[0].clone()
    };
    let detections: Vec<ComponentDetection> =
        rabbit_core::package::builtin_package_specs(model.platform)
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
    let mut list = uninstall_list_for_target(&model, &target, &detections, &NothingListed);
    tick(&mut list, "sws");

    let request = uninstall_request_for_target(&model, &target, &list, true).unwrap();
    assert_eq!(request.package_ids, ["sws"]);
    assert!(request.remove_settings);

    let dry_run = UninstallRequest {
        dry_run: true,
        ..request.clone()
    };
    let preview = execute_uninstall(
        &dry_run,
        &UninstallServices::without_elevation(),
        &ProgressReporter::noop(),
        &rabbit_core::cancel::CancelToken::new(),
    )
    .unwrap();
    assert_eq!(
        preview.item("sws").unwrap().status,
        UninstallStatus::WouldRemove
    );
    assert!(plugin.exists(), "a dry run changes nothing");

    let report = execute_uninstall(
        &request,
        &UninstallServices::without_elevation(),
        &ProgressReporter::noop(),
        &rabbit_core::cancel::CancelToken::new(),
    )
    .unwrap();
    let sws = report.item("sws").unwrap();
    assert_eq!(sws.status, UninstallStatus::Removed, "{}", sws.message);
    assert!(!plugin.exists());
    assert!(!resource.join("S&M.ini").exists(), "settings go too");

    let localizer = en();
    let outcome = wizard_uninstall_outcome(&localizer, &request.package_ids, &report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::Success);
    assert_eq!(outcome.heading, "RABBIT uninstalled SWS Extension.");
    let set_id = report.backup_set_id.clone().unwrap();
    let details = outcome.detail_lines.join("\n");
    assert!(
        details.contains(&format!("Backup set: {set_id}")),
        "{details}"
    );
    assert!(
        details.contains(&format!(
            "rabbit restore-backup --resource-path \"{}\" --backup-id {set_id} --package sws \
             --apply",
            resource.display()
        )),
        "{details}"
    );
    assert!(details.contains(&plugin.display().to_string()), "{details}");
    // The engine saved its report, and Done points to it rather than
    // writing a second one.
    let saved = outcome.report_path.clone().unwrap();
    assert!(saved.exists());
    let mut outcome = outcome;
    save_wizard_uninstall_outcome(&localizer, &mut outcome).unwrap();
    assert_eq!(outcome.report_path, Some(saved));

    // The post-run rebuild: SWS is gone from the Uninstall tab, and the
    // Install tab offers it again without ticking it, because the run
    // recorded it as declined. A REAPER that never had it ticks it.
    let available = [AvailablePackage {
        package_id: "sws".to_string(),
        version: Some(Version::parse("2.14.0.7").unwrap()),
        whats_new: None,
        channel: None,
    }];
    let sws_row = |target: &TargetRow| {
        let plan = wizard_package_plan_for_target_with_available(&model, Some(target), &available)
            .unwrap();
        let list = uninstall_list_for_target(&model, target, &plan.detections, &NothingListed);
        assert!(list.rows.iter().all(|row| row.package_id != "sws"));
        plan.package_rows
            .into_iter()
            .find(|row| row.package_id == "sws")
            .unwrap()
    };
    let row = sws_row(&target);
    assert_eq!(row.original_action, PlanActionKind::Install, "{row:?}");
    assert!(!row.selected, "{row:?}");
    let state = rabbit_core::receipt::load_install_state(&resource)
        .unwrap()
        .unwrap();
    assert!(state.declined_packages.contains("sws"));
    let fresh = TargetRow {
        path: dir.path().join("Fresh"),
        ..target.clone()
    };
    assert!(sws_row(&fresh).selected);
}

// ---------------------------------------------------------------------------
// What has to be closed before the run starts.
// ---------------------------------------------------------------------------

fn check(name: &str, status: PreflightStatus) -> PreflightCheck {
    PreflightCheck {
        name: name.to_string(),
        status,
        message: format!("{name} message"),
    }
}

fn preflight(checks: Vec<PreflightCheck>) -> PreflightReport {
    PreflightReport {
        passed: checks
            .iter()
            .all(|check| check.status != PreflightStatus::Fail),
        checks,
    }
}

fn app2clap() -> UninstallPlanItem {
    UninstallPlanItem {
        every_reaper_closed: true,
        ..item("app2clap", Removability::Removable)
    }
}

#[test]
fn nothing_running_lets_the_run_start() {
    let plan = plan(vec![removable_sws()]);
    let report = preflight(vec![check(REAPER_PROCESS_CHECK, PreflightStatus::Pass)]);
    assert_eq!(
        uninstall_blocker(&en(), &plan, &ids(&["sws"]), &report, &[]),
        None
    );
}

#[test]
fn a_running_reaper_stops_the_run_with_uninstall_wording() {
    let plan = plan(vec![removable_sws()]);
    let report = preflight(vec![check(REAPER_PROCESS_CHECK, PreflightStatus::Fail)]);

    let blocker = uninstall_blocker(&en(), &plan, &ids(&["sws"]), &report, &[]).unwrap();

    assert_eq!(blocker.title, "REAPER is still open");
    assert_eq!(
        blocker.body,
        "Close REAPER, then press Uninstall again. RABBIT can't uninstall anything from REAPER \
         while it is running."
    );
}

/// A package every REAPER can load needs all of them closed, which says
/// everything the target's own check would: one sentence, naming it.
#[test]
fn a_package_every_reaper_loads_asks_to_close_them_all() {
    let plan = plan(vec![removable_sws(), app2clap()]);
    let report = preflight(vec![
        check(REAPER_PROCESS_CHECK, PreflightStatus::Fail),
        check(ANY_REAPER_PROCESS_CHECK, PreflightStatus::Fail),
    ]);

    let blocker =
        uninstall_blocker(&en(), &plan, &ids(&["sws", "app2clap"]), &report, &[]).unwrap();

    assert_eq!(blocker.title, "REAPER is still open");
    assert!(
        blocker
            .body
            .starts_with("Close every copy of REAPER on this computer, including portable ones"),
        "{}",
        blocker.body
    );
    assert!(
        blocker.body.ends_with("can use app2clap."),
        "{}",
        blocker.body
    );
    assert!(!blocker.body.contains("Close REAPER,"), "{}", blocker.body);
}

#[test]
fn a_program_the_plan_names_stops_the_run_by_name() {
    let surge = UninstallPlanItem {
        blocking_programs: vec![
            "Surge XT.exe".to_string(),
            "Surge XT Effects.exe".to_string(),
        ],
        ..item("surge-xt", Removability::Removable)
    };
    let plan = plan(vec![surge]);
    let report = preflight(vec![
        check(REAPER_PROCESS_CHECK, PreflightStatus::Pass),
        check(PROGRAMS_RUNNING_CHECK, PreflightStatus::Fail),
    ]);
    let ticked = ids(&["surge-xt"]);

    let blocker = uninstall_blocker(
        &en(),
        &plan,
        &ticked,
        &report,
        &["Surge XT.exe".to_string(), "Surge XT.exe".to_string()],
    )
    .unwrap();
    assert_eq!(blocker.title, "Close these programs first");
    assert_eq!(
        blocker.body,
        "Close Surge XT.exe, then press Uninstall again."
    );

    // Closed again in between: every program the plan names.
    let blocker = uninstall_blocker(&en(), &plan, &ticked, &report, &[]).unwrap();
    assert!(
        blocker.body.contains("Surge XT.exe, Surge XT Effects.exe"),
        "{}",
        blocker.body
    );
}

#[test]
fn the_checks_before_the_run_read_in_every_locale() {
    let plan = plan(vec![removable_sws(), app2clap()]);
    let report = preflight(vec![
        check(ANY_REAPER_PROCESS_CHECK, PreflightStatus::Fail),
        check(PROGRAMS_RUNNING_CHECK, PreflightStatus::Fail),
    ]);
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let blocker = uninstall_blocker(
            &localizer,
            &plan,
            &ids(&["sws", "app2clap"]),
            &report,
            &["Surge XT.exe".to_string()],
        )
        .unwrap();
        for text in [&blocker.title, &blocker.body] {
            assert_reads(locale, text);
        }
        assert!(
            blocker.body.contains("app2clap"),
            "{locale}: {}",
            blocker.body
        );
        assert!(
            blocker.body.contains("Surge XT.exe"),
            "{locale}: {}",
            blocker.body
        );
    }
}

fn ids(ids: &[&str]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

/// No message key or placeholder shows through.
fn assert_reads(locale: &str, text: &str) {
    assert!(!text.is_empty(), "{locale}: empty");
    for marker in ["wizard-", "status-", "{", "}"] {
        assert!(!text.contains(marker), "{locale}: {text}");
    }
}

// ---------------------------------------------------------------------------
// What the progress log starts with.
// ---------------------------------------------------------------------------

#[test]
fn the_progress_log_starts_with_the_target_and_each_ticked_package() {
    let mut list = list_for(vec![
        sws_with_settings(),
        item("langpack-de", Removability::Removable),
    ]);
    tick(&mut list, "sws");
    tick(&mut list, "langpack-de");
    let target = target(true);

    let details = uninstall_progress_details_for_start(&en(), &target, &list, false);
    assert_eq!(
        details,
        "Starting to uninstall.\n\
         Target: C:/REAPER\n\
         SWS Extension: Will uninstall\n\
         REAPER in German: Will uninstall"
    );

    let details = uninstall_progress_details_for_start(&en(), &target, &list, true);
    assert!(
        details.contains(
            "SWS Extension: Will uninstall\nIts settings go too, into the backup: your grooves, \
             SWS settings."
        ),
        "{details}"
    );
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        assert_reads(
            locale,
            &uninstall_progress_details_for_start(&localizer, &target, &list, true),
        );
    }
}

// ---------------------------------------------------------------------------
// What Done says.
// ---------------------------------------------------------------------------

fn report_item(
    package_id: &str,
    status: UninstallStatus,
    message_code: UninstallMessage,
) -> UninstallReportItem {
    UninstallReportItem {
        package_id: package_id.to_string(),
        display_name: package_id.to_string(),
        status,
        removed: Vec::new(),
        kept: Vec::new(),
        removed_dirs: Vec::new(),
        put_back: Vec::new(),
        left_in_backup: Vec::new(),
        leftovers: Vec::new(),
        registry_entries: Vec::new(),
        problems: Vec::new(),
        pending_restart: Vec::new(),
        vendor: None,
        message: format!("English message for {package_id}."),
        message_code,
    }
}

fn removed(package_id: &str, files: &[&str]) -> UninstallReportItem {
    UninstallReportItem {
        removed: files
            .iter()
            .map(|path| RemovedFile {
                path: PathBuf::from(path),
                reason: RemovalReason::Receipt,
                size: 3,
                changed_since_install: false,
                backup_path: None,
            })
            .collect(),
        ..report_item(
            package_id,
            UninstallStatus::Removed,
            UninstallMessage::Removed {
                backup_set_id: Some("uninstall-unix-1".to_string()),
            },
        )
    }
}

fn checks_failed(package_id: &str, name: &str) -> UninstallReportItem {
    report_item(
        package_id,
        UninstallStatus::Blocked,
        UninstallMessage::ChecksFailed {
            checks: vec![check(name, PreflightStatus::Fail)],
        },
    )
}

fn not_reached(package_id: &str, blocked_by: &str) -> UninstallReportItem {
    report_item(
        package_id,
        UninstallStatus::NotReached,
        UninstallMessage::OtherPackageBlocked {
            package_ids: vec![blocked_by.to_string()],
        },
    )
}

fn report(items: Vec<UninstallReportItem>) -> UninstallReport {
    let changed = items.iter().any(|item| item.status.is_removed());
    UninstallReport {
        resource_path: PathBuf::from("C:/REAPER"),
        platform: Platform::Windows,
        target: InstallationKind::Portable,
        dry_run: false,
        remove_settings: false,
        preflight: preflight(Vec::new()),
        backup_set_id: changed.then(|| "uninstall-unix-1".to_string()),
        backup_set_path: None,
        items,
        plan: plan(Vec::new()),
        saved_report: None,
    }
}

fn outcome(report: &UninstallReport) -> WizardUninstallOutcome {
    let package_ids: Vec<String> = report
        .items
        .iter()
        .map(|item| item.package_id.clone())
        .collect();
    wizard_uninstall_outcome(&en(), &package_ids, report)
}

const HINT: &str = "To get one back, run RABBIT again and tick it on the Install or update tab.";

#[test]
fn success_names_what_went_and_how_to_get_it_back() {
    let report = UninstallReport {
        saved_report: Some(SavedReportPaths {
            json_path: PathBuf::from("C:/REAPER/RABBIT/logs/uninstall-unix-1.json"),
            text_path: PathBuf::from("C:/REAPER/RABBIT/logs/uninstall-unix-1.txt"),
        }),
        ..report(vec![
            removed(
                "sws",
                &[
                    "C:/REAPER/UserPlugins/reaper_sws-x64.dll",
                    "C:/REAPER/Scripts/sws_python64.py",
                ],
            ),
            removed("reapack", &["C:/REAPER/UserPlugins/reaper_reapack-x64.dll"]),
        ])
    };

    let outcome = outcome(&report);

    assert_eq!(outcome.status, UninstallOutcomeStatus::Success);
    assert_eq!(
        outcome.heading,
        "RABBIT uninstalled SWS Extension, ReaPack."
    );
    assert_eq!(
        outcome.status_text,
        format!("{}\n\n{HINT}", outcome.heading)
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER",
            "Also remove settings: no",
            "SWS Extension: Uninstalled",
            "  Removed: C:/REAPER/UserPlugins/reaper_sws-x64.dll",
            "  Removed: C:/REAPER/Scripts/sws_python64.py",
            "ReaPack: Uninstalled",
            "  Removed: C:/REAPER/UserPlugins/reaper_reapack-x64.dll",
            "Backup set: uninstall-unix-1",
            "To put SWS Extension back, close REAPER and run: rabbit restore-backup \
             --resource-path \"C:/REAPER\" --backup-id uninstall-unix-1 --package sws --apply",
            "To put ReaPack back, close REAPER and run: rabbit restore-backup --resource-path \
             \"C:/REAPER\" --backup-id uninstall-unix-1 --package reapack --apply",
            "Report: C:/REAPER/RABBIT/logs/uninstall-unix-1.txt",
        ]
    );
    assert_eq!(
        outcome.report_path,
        Some(PathBuf::from("C:/REAPER/RABBIT/logs/uninstall-unix-1.txt"))
    );
}

#[test]
fn the_details_name_what_stayed_and_why() {
    let sws = UninstallReportItem {
        kept: vec![
            KeptFile {
                path: PathBuf::from("C:/REAPER/Scripts/sws_python.py"),
                reason: KeptFileReason::ChangedSinceInstall,
            },
            KeptFile {
                path: PathBuf::from("C:/REAPER/Data/Grooves"),
                reason: KeptFileReason::HoldsLink,
            },
            KeptFile {
                path: PathBuf::from("C:/REAPER/S&M.ini"),
                reason: KeptFileReason::InstalledByReaPack,
            },
        ],
        removed_dirs: vec![PathBuf::from("C:/REAPER/Data/SWS")],
        ..removed("sws", &["C:/REAPER/UserPlugins/reaper_sws-x64.dll"])
    };

    let details = outcome(&report(vec![sws])).detail_lines.join("\n");

    for line in [
        "  Removed: C:/REAPER/Data/SWS",
        "  Kept, because it changed since RABBIT installed it: C:/REAPER/Scripts/sws_python.py",
        "  Kept, because it holds a link RABBIT never follows: C:/REAPER/Data/Grooves",
        "  Kept, because ReaPack installed it and keeps track of it: C:/REAPER/S&M.ini",
    ] {
        assert!(details.contains(line), "{line}\n{details}");
    }
}

/// A file another program holds open: the package stays, with the
/// sentence in the user's language, and the run still removed the other.
#[test]
fn a_file_in_use_finishes_with_problems_and_says_which_file() {
    let reapack = report_item(
        "reapack",
        UninstallStatus::Failed,
        UninstallMessage::FileInUse {
            path: PathBuf::from("C:/REAPER/UserPlugins/reaper_reapack-x64.dll"),
            error: "sharing violation".to_string(),
        },
    );
    let outcome = outcome(&report(vec![
        removed("sws", &["C:/REAPER/UserPlugins/reaper_sws-x64.dll"]),
        reapack,
    ]));

    assert_eq!(
        outcome.status,
        UninstallOutcomeStatus::CompletedWithProblems
    );
    assert_eq!(
        outcome.heading,
        "RABBIT uninstalled SWS Extension, but not everything went as planned."
    );
    let reason = "ReaPack wasn't uninstalled, because another program is using \
                  C:/REAPER/UserPlugins/reaper_reapack-x64.dll. Close that program, then run \
                  RABBIT again.";
    assert_eq!(
        outcome.status_text,
        format!("{}\n\n{reason}\n\n{HINT}", outcome.heading)
    );
    let details = outcome.detail_lines.join("\n");
    assert!(
        details.contains(&format!("ReaPack: Couldn't be uninstalled\n  {reason}")),
        "{details}"
    );
    assert!(
        !details.contains("--package reapack"),
        "only what went can come back: {details}"
    );
}

#[test]
fn leftovers_and_a_language_left_set_finish_with_problems() {
    let langpack = UninstallReportItem {
        problems: vec![UninstallProblem::LanguageResetFailed {
            error: "access denied".to_string(),
        }],
        ..removed("langpack-de", &["C:/REAPER/LangPack/German.ReaperLangPack"])
    };
    let sws = UninstallReportItem {
        status: UninstallStatus::RemovedWithLeftovers,
        leftovers: vec![PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-arm64.dll")],
        ..removed("sws", &["C:/REAPER/UserPlugins/reaper_sws-x64.dll"])
    };

    let outcome = outcome(&report(vec![sws, langpack]));

    assert_eq!(
        outcome.status,
        UninstallOutcomeStatus::CompletedWithProblems
    );
    assert!(
        outcome.status_text.contains(
            "RABBIT still finds some of SWS Extension and leaves it alone. Show details lists \
             what.\nRABBIT couldn't set REAPER back to English after removing REAPER in German."
        ),
        "{}",
        outcome.status_text
    );
    let details = outcome.detail_lines.join("\n");
    assert!(
        details.contains("SWS Extension: Uninstalled, but some of it is still there"),
        "{details}"
    );
    assert!(
        details.contains("  Still there: C:/REAPER/UserPlugins/reaper_sws-arm64.dll"),
        "{details}"
    );
}

const OSARA_ENTRY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OSARA";

/// The entry OSARA's installer wrote in Windows' list of installed programs
/// is named in the details, whatever became of it. One that went, or that
/// belongs to another REAPER and stayed, is no problem; one RABBIT couldn't
/// remove is, and Done says where to look.
#[test]
fn done_names_the_entry_in_the_list_of_installed_programs() {
    let entry = |action| RegistryEntryOutcome {
        key: OSARA_ENTRY.to_string(),
        action,
    };
    let removed_entry = UninstallReportItem {
        registry_entries: vec![entry(RegistryEntryAction::Removed)],
        ..removed("osara", &["C:/REAPER/UserPlugins/reaper_osara64.dll"])
    };
    let done = outcome(&report(vec![removed_entry]));
    assert_eq!(done.status, UninstallOutcomeStatus::Success);
    let details = done.detail_lines.join("\n");
    assert!(
        details.contains(&format!(
            "  Taken off Windows' list of installed programs: {OSARA_ENTRY}"
        )),
        "{details}"
    );

    let elsewhere = UninstallReportItem {
        registry_entries: vec![entry(RegistryEntryAction::PointsElsewhere {
            found: Some(
                r#""C:\Users\Someone\AppData\Roaming\REAPER\osara\uninstall.exe""#.to_string(),
            ),
            expected: PathBuf::from("C:/REAPER/osara"),
        })],
        ..removed("osara", &["C:/REAPER/UserPlugins/reaper_osara64.dll"])
    };
    let done = outcome(&report(vec![elsewhere]));
    assert_eq!(done.status, UninstallOutcomeStatus::Success);
    let details = done.detail_lines.join("\n");
    assert!(
        details.contains(&format!(
            "  Still in Windows' list of installed programs, because it doesn't point into this \
             REAPER: {OSARA_ENTRY}"
        )),
        "{details}"
    );

    let stuck = UninstallReportItem {
        problems: vec![UninstallProblem::RegistryEntryNotRemoved {
            key: OSARA_ENTRY.to_string(),
            error: "Access is denied. (os error 5)".to_string(),
        }],
        ..removed("osara", &["C:/REAPER/UserPlugins/reaper_osara64.dll"])
    };
    let done = outcome(&report(vec![stuck]));
    assert_eq!(done.status, UninstallOutcomeStatus::CompletedWithProblems);
    assert!(
        done.status_text.contains(
            "RABBIT uninstalled OSARA, but Windows' list of installed programs may still show \
             it. Show details names the entry."
        ),
        "{}",
        done.status_text
    );
    let details = done.detail_lines.join("\n");
    assert!(
        details.contains(&format!(
            "  Still in Windows' list of installed programs, because RABBIT couldn't read or \
             remove it (Access is denied. (os error 5)): {OSARA_ENTRY}"
        )),
        "{details}"
    );
}

/// The user stopped the run: what went is named, and so is the way back.
#[test]
fn a_stopped_run_says_what_went_before_the_stop() {
    let reapack = report_item(
        "reapack",
        UninstallStatus::Cancelled,
        UninstallMessage::Cancelled,
    );
    let outcome = outcome(&report(vec![
        removed("sws", &["C:/REAPER/UserPlugins/reaper_sws-x64.dll"]),
        reapack,
    ]));

    assert_eq!(outcome.status, UninstallOutcomeStatus::Cancelled);
    assert_eq!(
        outcome.status_text,
        format!(
            "You stopped the uninstall. Packages RABBIT hadn't reached yet are still \
             installed.\n\nBefore you stopped it, RABBIT uninstalled SWS Extension.\n\n{HINT}"
        )
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"ReaPack: Still installed (you stopped)".to_string())
    );
}

#[test]
fn a_run_stopped_before_anything_went_offers_no_way_back() {
    let reapack = report_item(
        "reapack",
        UninstallStatus::Cancelled,
        UninstallMessage::Cancelled,
    );
    let outcome = outcome(&report(vec![reapack]));

    assert_eq!(outcome.status, UninstallOutcomeStatus::Cancelled);
    assert!(
        !outcome.status_text.contains(HINT),
        "{}",
        outcome.status_text
    );
    assert!(
        !outcome.status_text.contains("Before you stopped it"),
        "{}",
        outcome.status_text
    );
}

/// REAPER started after the check on Review: nothing changed, and the
/// package that stopped the run says why, once; the ones it kept from
/// starting add nothing.
#[test]
fn a_reaper_opened_after_review_uninstalls_nothing_and_says_why() {
    let outcome = outcome(&report(vec![
        checks_failed("sws", REAPER_PROCESS_CHECK),
        not_reached("reapack", "sws"),
    ]));

    assert_eq!(outcome.status, UninstallOutcomeStatus::NothingDone);
    assert_eq!(
        outcome.status_text,
        "Nothing was uninstalled.\n\nRABBIT stopped because REAPER was open. Close REAPER, then \
         run RABBIT again."
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"ReaPack: Still installed (the run stopped earlier)".to_string())
    );
    assert!(
        !outcome
            .detail_lines
            .iter()
            .any(|line| line.starts_with("Backup set"))
    );
}

#[test]
fn every_check_that_stops_a_package_says_what_to_do() {
    let outcome = outcome(&report(vec![
        checks_failed("app2clap", ANY_REAPER_PROCESS_CHECK),
        checks_failed("ffmpeg", BACKUP_SPACE_CHECK),
        checks_failed("surge-xt", PROGRAMS_RUNNING_CHECK),
    ]));

    assert_eq!(
        uninstall_reasons(
            &en(),
            &report(vec![checks_failed("sws", "programs-running")])
        ),
        ["programs-running message"],
        "a check RABBIT has no sentence for keeps the engine's"
    );
    for expected in [
        "RABBIT stopped because a REAPER was open. Close every copy of REAPER on this computer, \
         including portable ones, then run RABBIT again.",
        "RABBIT stopped because there isn't enough free space for the backup.",
        "programs-running message",
    ] {
        assert!(
            outcome.status_text.contains(expected),
            "{expected}\n{}",
            outcome.status_text
        );
    }
}

#[test]
fn a_failed_removal_uninstalls_nothing_and_keeps_the_engines_words() {
    let sws = report_item(
        "sws",
        UninstallStatus::Failed,
        UninstallMessage::MoveFailed {
            path: PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-x64.dll"),
            error: "denied".to_string(),
        },
    );
    let outcome = outcome(&report(vec![sws]));

    assert_eq!(outcome.status, UninstallOutcomeStatus::Failed);
    assert_eq!(
        outcome.status_text,
        "Uninstalling failed.\n\nEnglish message for sws."
    );
}

#[test]
fn files_left_in_the_backup_after_a_failure_are_pointed_out() {
    let sws = UninstallReportItem {
        left_in_backup: vec![PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-x64.dll")],
        ..report_item(
            "sws",
            UninstallStatus::Failed,
            UninstallMessage::ReceiptSaveFailed {
                error: "disk full".to_string(),
            },
        )
    };
    let outcome = outcome(&report(vec![sws]));

    assert!(
        outcome.status_text.contains(
            "Some files of SWS Extension couldn't be put back and are still in the backup."
        ),
        "{}",
        outcome.status_text
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"  Couldn't be put back, still in the backup: C:/REAPER/UserPlugins/reaper_sws-x64.dll".to_string()),
        "{:?}",
        outcome.detail_lines
    );
}

#[test]
fn a_package_gone_in_the_meantime_is_nothing_done() {
    let outcome = outcome(&report(vec![report_item(
        "sws",
        UninstallStatus::NotInstalled,
        UninstallMessage::NotInstalled,
    )]));

    assert_eq!(outcome.status, UninstallOutcomeStatus::NothingDone);
    assert_eq!(
        outcome.status_text,
        "Nothing was uninstalled.\n\nSWS Extension wasn't installed any more, so there was \
         nothing to remove."
    );
}

#[test]
fn done_reads_in_every_locale() {
    let entry = |action| RegistryEntryOutcome {
        key: OSARA_ENTRY.to_string(),
        action,
    };
    let reports = [
        report(vec![
            removed("sws", &["C:/REAPER/UserPlugins/reaper_sws-x64.dll"]),
            UninstallReportItem {
                registry_entries: vec![
                    entry(RegistryEntryAction::Removed),
                    entry(RegistryEntryAction::PointsElsewhere {
                        found: None,
                        expected: PathBuf::from("C:/REAPER/osara"),
                    }),
                ],
                problems: vec![
                    UninstallProblem::RegistryEntryUnreadable {
                        key: OSARA_ENTRY.to_string(),
                        error: String::new(),
                    },
                    UninstallProblem::RegistryEntryNotRemoved {
                        key: OSARA_ENTRY.to_string(),
                        error: String::new(),
                    },
                ],
                ..removed("osara", &["C:/REAPER/UserPlugins/reaper_osara64.dll"])
            },
            UninstallReportItem {
                leftovers: vec![PathBuf::from("C:/x")],
                left_in_backup: vec![PathBuf::from("C:/y")],
                problems: vec![UninstallProblem::LanguageResetFailed {
                    error: String::new(),
                }],
                ..report_item(
                    "langpack-de",
                    UninstallStatus::Failed,
                    UninstallMessage::FileInUse {
                        path: PathBuf::from("C:/z"),
                        error: String::new(),
                    },
                )
            },
            report_item(
                "reapack",
                UninstallStatus::NotInstalled,
                UninstallMessage::NotInstalled,
            ),
        ]),
        report(vec![
            removed("sws", &["C:/a"]),
            report_item(
                "reapack",
                UninstallStatus::Cancelled,
                UninstallMessage::Cancelled,
            ),
        ]),
        report(vec![
            checks_failed("sws", REAPER_PROCESS_CHECK),
            checks_failed("app2clap", ANY_REAPER_PROCESS_CHECK),
            checks_failed("ffmpeg", BACKUP_SPACE_CHECK),
            not_reached("reapack", "sws"),
        ]),
        report(vendor_outcomes()),
        report(vec![pending_restart_surge()]),
        report(vec![
            declined("sws"),
            declined("surge-xt"),
            declined("reaper"),
        ]),
    ];
    let statuses = [
        UninstallStatus::Removed,
        UninstallStatus::RemovedWithLeftovers,
        UninstallStatus::RemovedPendingRestart,
        UninstallStatus::WouldRemove,
        UninstallStatus::NotInstalled,
        UninstallStatus::Blocked,
        UninstallStatus::Failed,
        UninstallStatus::Cancelled,
        UninstallStatus::NotReached,
    ];
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for report in &reports {
            let package_ids = vec!["sws".to_string()];
            let outcome = wizard_uninstall_outcome(&localizer, &package_ids, report);
            assert_reads(locale, &outcome.heading);
            assert_reads(locale, &outcome.status_text);
            for line in outcome
                .detail_lines
                .iter()
                .filter(|line| !line.contains("rabbit restore-backup"))
            {
                assert_reads(locale, line);
            }
        }
        for status in statuses {
            assert_reads(locale, &uninstall_status_label(&localizer, status));
        }
        let (_, model) = langpack_model(PlanActionKind::Install);
        let outcome = wizard_uninstall_outcome_from_error(
            &model,
            &localizer,
            Path::new("C:/REAPER"),
            &ids(&["sws"]),
            false,
            &RabbitError::PackageInstallInProgress {
                lock_path: PathBuf::from("C:/REAPER/RABBIT/locks/package-install.lock"),
                pid: 42,
            },
        );
        assert_reads(locale, &outcome.heading);
    }
}

// ---------------------------------------------------------------------------
// A run the engine refused before it started.
// ---------------------------------------------------------------------------

/// Another RABBIT holds the lock: the reason is a plain sentence, for an
/// install as much as for an uninstall, and the details keep the engine's
/// own text with the lock file.
#[test]
fn a_held_lock_reads_as_another_rabbit_at_work() {
    let (_, model) = langpack_model(PlanActionKind::Install);
    let error = RabbitError::PackageInstallInProgress {
        lock_path: PathBuf::from("C:/REAPER/RABBIT/locks/package-install.lock"),
        pid: 42,
    };
    assert_eq!(
        wizard_error_reason(&model, &error),
        "Another copy of RABBIT is installing or uninstalling in this REAPER. Wait until it has \
         finished, then run RABBIT again."
    );

    let outcome = wizard_uninstall_outcome_from_error(
        &model,
        &en(),
        Path::new("C:/REAPER"),
        &ids(&["sws", "reapack"]),
        true,
        &error,
    );

    assert_eq!(outcome.status, UninstallOutcomeStatus::Failed);
    assert_eq!(
        outcome.status_text,
        format!(
            "Uninstalling failed.\n\n{}",
            wizard_error_reason(&model, &error)
        )
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER".to_string(),
            "Packages selected: SWS Extension, ReaPack".to_string(),
            format!("Error: {error}"),
        ]
    );
    assert_eq!(outcome.error_message, Some(error.to_string()));
    assert!(outcome.remove_settings);
}

#[test]
fn a_reaper_the_engine_found_reads_as_the_install_one_does() {
    let (_, model) = langpack_model(PlanActionKind::Install);
    let error = RabbitError::PreflightFailed {
        message: format!(
            "{REAPER_PROCESS_CHECK}: Close REAPER before uninstalling: reaper.exe (1)"
        ),
    };
    let outcome = wizard_uninstall_outcome_from_error(
        &model,
        &en(),
        Path::new("C:/REAPER"),
        &ids(&["sws"]),
        false,
        &error,
    );
    assert!(
        outcome
            .status_text
            .ends_with(&model.text.error_reaper_running),
        "{}",
        outcome.status_text
    );
}

/// The engine saves no report for a run it refused before the lock, so the
/// wizard saves its own, beside where the engine's would be, and points to
/// it. Never into a folder that doesn't exist.
#[test]
fn a_refused_run_leaves_a_report_of_its_own() {
    let dir = tempdir().unwrap();
    let (_, model) = langpack_model(PlanActionKind::Install);
    let error = RabbitError::PackageInstallInProgress {
        lock_path: dir.path().join("RABBIT/locks/package-install.lock"),
        pid: 42,
    };
    let localizer = en();
    let mut outcome = wizard_uninstall_outcome_from_error(
        &model,
        &localizer,
        dir.path(),
        &ids(&["sws"]),
        false,
        &error,
    );

    save_wizard_uninstall_outcome(&localizer, &mut outcome).unwrap();

    let saved = outcome.report_path.clone().unwrap();
    assert!(saved.starts_with(dir.path().join("RABBIT").join("logs")));
    let name = saved.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("uninstall-") && name.ends_with(".txt"),
        "{name}"
    );
    assert!(saved.exists() && saved.with_extension("json").exists());
    assert_eq!(
        outcome.detail_lines.last().unwrap(),
        &format!("Report: {}", saved.display())
    );

    let missing = dir.path().join("missing");
    let mut outcome = wizard_uninstall_outcome_from_error(
        &model,
        &localizer,
        &missing,
        &ids(&["sws"]),
        false,
        &error,
    );
    save_wizard_uninstall_outcome(&localizer, &mut outcome).unwrap();
    assert_eq!(outcome.report_path, None);
    assert!(!missing.exists());
}

// ---------------------------------------------------------------------------
// After a vendor's own uninstaller.
// ---------------------------------------------------------------------------

/// Surge XT removed by its own uninstaller, with a file Windows removes at
/// the next restart.
fn pending_restart_surge() -> UninstallReportItem {
    UninstallReportItem {
        pending_restart: vec![PathBuf::from(
            "C:/Program Files/Common Files/VST3/Surge Synth Team/Surge XT.vst3/Contents/x86_64-win/Surge XT.vst3",
        )],
        vendor: Some(VendorOutcome {
            status: VendorStatus::RemovedPendingRestart,
            uninstaller: Some(PathBuf::from(
                "C:/ProgramData/Surge XT/uninstall/unins000.exe",
            )),
            launched: true,
            exit_code: Some(0),
            removed_leftovers: vec![LeftoverOutcome {
                path: PathBuf::from(
                    "C:/Program Files/Common Files/CLAP/Surge Synth Team/SurgeXTData",
                ),
                kind: LeftoverKind::Junction,
                result: LeftoverResult::Removed,
            }],
            ..VendorOutcome::new("surge-xt")
        }),
        ..report_item(
            "surge-xt",
            UninstallStatus::RemovedPendingRestart,
            UninstallMessage::UninstalledByVendor,
        )
    }
}

fn declined(package_id: &str) -> UninstallReportItem {
    report_item(
        package_id,
        UninstallStatus::Cancelled,
        UninstallMessage::ElevationDeclined,
    )
}

/// One of each way a vendor's uninstaller can end badly, one package each.
fn vendor_outcomes() -> Vec<UninstallReportItem> {
    vec![
        report_item(
            "surge-xt",
            UninstallStatus::Blocked,
            UninstallMessage::VendorRefused {
                refusal: VendorRefusal::UntrustedOwner {
                    path: PathBuf::from("C:/ProgramData/Surge XT/uninstall/unins001.exe"),
                },
            },
        ),
        report_item(
            "reaper",
            UninstallStatus::Cancelled,
            UninstallMessage::VendorCancelled,
        ),
        report_item(
            "sws",
            UninstallStatus::Failed,
            UninstallMessage::VendorFailed { exit_code: 2 },
        ),
        report_item(
            "osara",
            UninstallStatus::Failed,
            UninstallMessage::VendorStillThere {
                still_present: vec![PathBuf::from("C:/x")],
                still_listed: true,
            },
        ),
        report_item(
            "reapack",
            UninstallStatus::Failed,
            UninstallMessage::VendorStillOpen { seconds: 600 },
        ),
        report_item(
            "ffmpeg",
            UninstallStatus::Failed,
            UninstallMessage::VendorRunFailed {
                error: "the pipe broke".to_string(),
            },
        ),
        report_item(
            "app2clap",
            UninstallStatus::Failed,
            UninstallMessage::ElevationFailed {
                error: "no pipe".to_string(),
            },
        ),
        UninstallReportItem {
            problems: vec![UninstallProblem::ReceiptNotSaved {
                error: "denied".to_string(),
            }],
            ..report_item(
                "reakontrol",
                UninstallStatus::Failed,
                UninstallMessage::ResourceFolderDamaged {
                    missing: vec![PathBuf::from("C:/REAPER/reaper.ini")],
                },
            )
        },
    ]
}

/// A removal that finishes after a restart says so: on Done's first line,
/// in a sentence of its own, and in the details, next to the uninstaller
/// that ran and the link RABBIT removed after it.
#[test]
fn a_removal_finishing_after_a_restart_says_to_restart() {
    let report = report(vec![
        removed("sws", &["C:/REAPER/UserPlugins/reaper_sws-x64.dll"]),
        pending_restart_surge(),
    ]);
    let outcome = outcome(&report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::PendingRestart);
    assert_eq!(
        outcome.heading,
        "RABBIT uninstalled SWS Extension, Surge XT. Restart your computer to finish."
    );
    assert!(
        outcome
            .status_text
            .contains("Restart your computer to finish removing Surge XT."),
        "{}",
        outcome.status_text
    );
    let details = outcome.detail_lines.join("\n");
    for line in [
        "Surge XT: Uninstalled; the rest goes when you restart the computer",
        "  Its own uninstaller: C:/ProgramData/Surge XT/uninstall/unins000.exe",
        "  Removed: C:/Program Files/Common Files/CLAP/Surge Synth Team/SurgeXTData",
        "  Goes when you restart the computer: C:/Program Files/Common Files/VST3/",
    ] {
        assert!(details.contains(line), "{line}: {details}");
    }
}

/// A declined permission prompt is not a stop the user asked for: Done says
/// plainly that nothing was uninstalled and why, with no way back offered
/// for packages that never went.
#[test]
fn a_declined_prompt_says_nothing_was_uninstalled() {
    let report = report(vec![declined("sws"), declined("surge-xt")]);
    let outcome = outcome(&report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::PermissionDeclined);
    assert_eq!(
        outcome.heading,
        "Nothing was uninstalled, because the Windows permission prompt was declined."
    );
    assert_eq!(outcome.status_text, outcome.heading);
}

/// Each way a vendor's uninstaller ends badly gets its own plain sentence:
/// what happened, and what the user can do.
#[test]
fn each_vendor_outcome_reads_as_a_sentence() {
    let reasons = uninstall_reasons(&en(), &report(vendor_outcomes()));
    for sentence in [
        "Next to Surge XT's uninstaller is a file an administrator didn't put there: \
         C:/ProgramData/Surge XT/uninstall/unins001.exe",
        "Nothing was changed. You can uninstall Surge XT in Windows Settings",
        "REAPER's own uninstaller was cancelled before it finished.",
        "SWS Extension's own uninstaller ended with error code 2.",
        "OSARA's own uninstaller finished, but OSARA is still installed.",
        "ReaPack's own uninstaller is still open, so RABBIT stopped and left the packages after \
         it alone. Press Alt+Tab to find its window, and answer it.",
        "(the pipe broke), so it stopped there.",
        "RABBIT couldn't get administrator rights (no pipe), so nothing was uninstalled.",
        "these files were missing from REAPER's resource folder: C:/REAPER/reaper.ini",
        "couldn't update its records",
    ] {
        assert!(
            reasons.iter().any(|reason| reason.contains(sentence)),
            "{sentence}: {reasons:#?}"
        );
    }
    assert_eq!(
        outcome(&report(vendor_outcomes())).status,
        UninstallOutcomeStatus::Cancelled,
        "REAPER's uninstaller was cancelled by the user"
    );
}
