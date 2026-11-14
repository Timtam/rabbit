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
    ANY_REAPER_PROCESS_CHECK, AdministratorStepKind, AdministratorStepRecord,
    AdministratorStepVerdict, BACKUP_SPACE_CHECK, ByHandReason, CleanupOutcome, CleanupResult,
    CleanupStatus, CleanupStep, CleanupTarget, KeptFile, KeptFileReason, KeptPath, KeptReason,
    LeftoverKind, LeftoverOutcome, LeftoverResult, NoMacosObjects, NothingListed,
    PROGRAMS_RUNNING_CHECK, RegistryEntryAction, RegistryEntryOutcome, Removability, RemovalReason,
    RemovedFile, UninstallMessage, UninstallPlanItem, UninstallProblem, UninstallReport,
    UninstallReportItem, UninstallRequest, UninstallServices, UninstallStatus, VendorOutcome,
    VendorRefusal, VendorStatus, execute_uninstall,
};
use rabbit_core::version::Version;
use tempfile::tempdir;

use super::support::*;
use super::uninstall::{
    JAWS, JAWS_DLL, JAWS_FOLDER, JAWS_KEY, JAWS_SCRIPT, MAC_REAPER_APP, MAC_RESOURCE,
    SCRIPTS_NOTE_KEY, admin_surge, by_hand_lines_en, item, jaws_folder_file, jaws_with_cleanup,
    list_for, mac_plan, plan, removable_sws, root_moved_reaper, sws_with_settings, target, tick,
    trash_reaper,
};
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

fn waiting_for_files(id: &str) -> ProgressEvent {
    ProgressEvent::VendorWaitingForFiles {
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
        (waiting_for_files("surge-xt"), true),
        (completed("sws", true), true),
        (completed("sws", false), true),
        (
            ProgressEvent::RemoveStarted {
                package_id: id(),
                method: RemovalMethod::Trash,
            },
            true,
        ),
        (
            ProgressEvent::RemoveStarted {
                package_id: id(),
                method: RemovalMethod::Administrator,
            },
            true,
        ),
        // What the password window is for, before it takes focus, and that
        // it may be behind RABBIT.
        (
            ProgressEvent::AdministratorPrompt {
                package_ids: vec!["surge-xt".to_string()],
            },
            true,
        ),
        (
            ProgressEvent::AdministratorStillWaiting { seconds: 30 },
            true,
        ),
    ];
    for (event, spoken) in events {
        assert_eq!(is_spoken(&event), spoken, "{event:?}");
    }
}

/// The administrator prompt counts no phase of its own: a one-package
/// run still reaches 100 when that package ends.
#[test]
fn the_administrator_prompt_counts_no_phase() {
    let mut state = ProgressUiState::new(WizardMode::Uninstall, 1, 0);
    for event in [
        ProgressEvent::AdministratorPrompt {
            package_ids: vec!["surge-xt".to_string()],
        },
        ProgressEvent::AdministratorStillWaiting { seconds: 30 },
    ] {
        state.record(&event);
        assert_eq!(state.percentage(), 0);
        assert!(!state.moves_focus_to_log(&event));
    }
    state.record(&ProgressEvent::RemoveStarted {
        package_id: "surge-xt".to_string(),
        method: RemovalMethod::Administrator,
    });
    state.record(&completed("surge-xt", true));
    assert_eq!(state.percentage(), 100);
}

/// The wizard doesn't restore or delete backups until Phase 2 of the
/// backup plan: until then a restore's and a delete's events are neither
/// spoken nor counted, and move no focus.
#[test]
fn restore_and_delete_events_say_and_count_nothing_yet() {
    let mut state = ProgressUiState::new(WizardMode::Uninstall, 1, 0);
    for event in [
        ProgressEvent::RestoreStarted {
            set_id: "uninstall-unix-1".to_string(),
            package_id: Some("sws".to_string()),
        },
        ProgressEvent::RestoreCompleted {
            set_id: "uninstall-unix-1".to_string(),
            package_id: None,
            status: rabbit_core::rollback::RestoreStatus::Restored,
        },
        ProgressEvent::DeleteStarted {
            set_id: "uninstall-unix-1".to_string(),
        },
        ProgressEvent::DeleteCompleted {
            set_id: "uninstall-unix-1".to_string(),
            status: rabbit_core::backup_delete::DeleteStatus::Deleted,
            bytes_freed: 1024,
        },
    ] {
        assert!(!is_spoken(&event), "{event:?}");
        state.record(&event);
        assert_eq!(state.percentage(), 0);
        assert!(!state.moves_focus_to_log(&event));
    }
}

/// macOS's password window keeps its own text, which names osascript, so
/// RABBIT says what it is for, in the user's language; Windows' permission
/// prompt names RABBIT, and nothing new is said there.
#[test]
fn the_page_says_what_macoss_password_window_is_for() {
    let line = administrator_prompt_line(&en(), Some(Platform::MacOs), "Surge XT, REAPER").unwrap();
    assert_eq!(
        line,
        "macOS now asks for an administrator's name and password, once. With it, RABBIT runs \
         one administrator command that uninstalls Surge XT, REAPER. If you cancel, nothing is \
         uninstalled."
    );
    assert_eq!(
        administrator_prompt_line(&en(), Some(Platform::Windows), "Surge XT"),
        None
    );
    assert_eq!(
        administrator_still_waiting_line(&en(), Some(Platform::MacOs)).as_deref(),
        Some(
            "Still waiting for the administrator password window. If you don't hear it, check \
             whether it is behind RABBIT."
        )
    );
    assert_eq!(
        administrator_still_waiting_line(&en(), Some(Platform::Windows)),
        None
    );
    let german = Localizer::embedded("de-DE").unwrap();
    let line = administrator_prompt_line(&german, Some(Platform::MacOs), "Surge XT").unwrap();
    assert!(
        line.contains("Administrators") && line.contains("Surge XT"),
        "{line}"
    );
}

/// Each way a removal starts has its own status and log line. A move to the
/// Trash says so; the administrator command's packages only get a log line,
/// since the command has ended by then and each one's end follows at once.
/// The Windows lines stay as they were.
#[test]
fn each_removal_says_how_it_goes_on_the_progress_page() {
    let lines = |method| removal_started_lines(&en(), method, "REAPER");
    assert_eq!(
        lines(RemovalMethod::Files),
        (
            Some("Uninstalling REAPER…".to_string()),
            "Uninstalling REAPER…".to_string()
        )
    );
    assert_eq!(
        lines(RemovalMethod::VendorUninstaller),
        (
            Some("Running REAPER's own uninstaller…".to_string()),
            "Running REAPER's own uninstaller…".to_string()
        )
    );
    assert_eq!(
        lines(RemovalMethod::Trash),
        (
            Some("Moving REAPER to the Trash…".to_string()),
            "Moving REAPER to the Trash…".to_string()
        )
    );
    assert_eq!(
        lines(RemovalMethod::Administrator),
        (
            None,
            "The administrator command has finished with REAPER.".to_string()
        )
    );
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for method in [
            RemovalMethod::Files,
            RemovalMethod::VendorUninstaller,
            RemovalMethod::Trash,
            RemovalMethod::Administrator,
        ] {
            let (status, log) = removal_started_lines(&localizer, method, "REAPER");
            assert_reads(locale, &log);
            assert!(log.contains("REAPER"), "{locale}: {log}");
            if let Some(status) = status {
                assert_reads(locale, &status);
            }
        }
        assert_reads(
            locale,
            &administrator_prompt_line(&localizer, Some(Platform::MacOs), "Surge XT").unwrap(),
        );
        assert_reads(
            locale,
            &administrator_still_waiting_line(&localizer, Some(Platform::MacOs)).unwrap(),
        );
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

/// Waiting for an ended uninstaller's files to go is heard once per run
/// too, apart from still waiting for one that runs: each tells the user
/// something different to do.
#[test]
fn waiting_for_files_moves_focus_to_the_log_once_of_its_own() {
    let mut state = ProgressUiState::new(WizardMode::Uninstall, 2, 0);
    let mut moves = Vec::new();
    for event in [
        started("surge-xt"),
        still_waiting("surge-xt"),
        waiting_for_files("surge-xt"),
        completed("surge-xt", true),
        started("reaper"),
        waiting_for_files("reaper"),
        completed("reaper", true),
    ] {
        state.record(&event);
        moves.push(state.moves_focus_to_log(&event));
    }
    assert_eq!(moves, [false, true, true, false, false, false, false]);
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
/// and the rebuilt Install tab offers SWS again, unticked. Once SWS is put
/// back by hand, its update is ticked.
///
/// The rows are planned on temporary folders alone. The engine and the
/// Install tab read this computer's own detection and folders, which find
/// the JAWS scripts wherever they are really installed, so the target is
/// a portable REAPER, where nothing of theirs can be planned.
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
    let roots = |target: &TargetRow| fixture_roots(model.platform, &target.path, dir.path());
    let mut list = uninstall_list_with_roots(
        &model,
        &target,
        &roots(&target),
        &detections,
        &NothingListed,
        &NoMacosObjects,
    );
    tick(&mut list, "sws");

    let request = uninstall_request_for_target(&model, &target, &list, true).unwrap();
    assert_eq!(request.package_ids, ["sws"]);
    assert!(request.remove_settings);
    assert_eq!(request.target, Some(InstallationKind::Portable));

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
    let jaws = preview.plan.item("jaws-scripts");
    assert!(
        jaws.is_none_or(|jaws| jaws.files.is_empty()
            && jaws.machine_cleanup.is_none()
            && !jaws.can_remove()),
        "{jaws:?}"
    );

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
    // SWS's row on the Install tab, and whether the Uninstall tab lists it.
    let sws_rows = |target: &TargetRow| {
        let plan = wizard_package_plan_for_target_with_available(&model, Some(target), &available)
            .unwrap();
        let list = uninstall_list_with_roots(
            &model,
            target,
            &roots(target),
            &plan.detections,
            &NothingListed,
            &NoMacosObjects,
        );
        let listed = list.rows.iter().any(|row| row.package_id == "sws");
        let row = plan
            .package_rows
            .into_iter()
            .find(|row| row.package_id == "sws")
            .unwrap();
        (row, listed)
    };
    let (row, listed) = sws_rows(&target);
    assert!(!listed);
    assert_eq!(row.original_action, PlanActionKind::Install, "{row:?}");
    assert!(!row.selected, "{row:?}");
    let declined = || rabbit_core::receipt::declined_packages(&resource);
    assert!(declined().contains("sws"));
    let fresh = TargetRow {
        path: dir.path().join("Fresh"),
        ..target.clone()
    };
    let (fresh_row, fresh_listed) = sws_rows(&fresh);
    assert!(!fresh_listed);
    assert!(fresh_row.selected, "{fresh_row:?}");

    // Put back by hand, SWS is an Update again, and its update is ticked:
    // the removal record only keeps an Install row unticked. The record
    // itself stays until a run with SWS ticked clears it.
    std::fs::write(&plugin, b"sws").unwrap();
    let (row, listed) = sws_rows(&target);
    assert!(listed, "the Uninstall tab lists SWS again");
    assert_eq!(row.original_action, PlanActionKind::Update, "{row:?}");
    assert!(row.selected, "{row:?}");
    assert!(declined().contains("sws"));
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

/// REAPER needs every REAPER closed for a reason of its own: its own
/// uninstaller doesn't run while any REAPER is open. Saying that every
/// REAPER can use REAPER makes no sense, so REAPER is never among the
/// packages the sentence names, and alone it gets a sentence of its own.
#[test]
fn reaper_itself_is_never_a_package_every_reaper_can_use() {
    let reaper = || UninstallPlanItem {
        every_reaper_closed: true,
        ..item("reaper", Removability::Removable)
    };
    let surge = || UninstallPlanItem {
        every_reaper_closed: true,
        ..item("surge-xt", Removability::Removable)
    };
    let report = preflight(vec![
        check(REAPER_PROCESS_CHECK, PreflightStatus::Fail),
        check(ANY_REAPER_PROCESS_CHECK, PreflightStatus::Fail),
    ]);

    let alone = uninstall_blocker(
        &en(),
        &plan(vec![reaper()]),
        &ids(&["reaper"]),
        &report,
        &[],
    )
    .unwrap();
    assert_eq!(
        alone.body,
        "Close every copy of REAPER on this computer, including portable ones, then press \
         Uninstall again. REAPER's own uninstaller doesn't run while any REAPER is open."
    );

    let both = uninstall_blocker(
        &en(),
        &plan(vec![surge(), reaper()]),
        &ids(&["surge-xt", "reaper"]),
        &report,
        &[],
    )
    .unwrap();
    assert!(
        both.body
            .ends_with("Every REAPER on this computer can use Surge XT."),
        "{}",
        both.body
    );
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let alone = uninstall_blocker(
            &localizer,
            &plan(vec![reaper()]),
            &ids(&["reaper"]),
            &report,
            &[],
        )
        .unwrap();
        assert_reads(locale, &alone.body);
    }
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
        cleanup: None,
        message: format!("English message for {package_id}."),
        message_code,
    }
}

/// `package_id` removed into the run's backup set, as a real run leaves a
/// package whose files moved: each file has its place in the set.
fn removed(package_id: &str, files: &[&str]) -> UninstallReportItem {
    UninstallReportItem {
        removed: files
            .iter()
            .map(|path| RemovedFile {
                path: PathBuf::from(path),
                reason: RemovalReason::Receipt,
                size: 3,
                changed_since_install: false,
                backup_path: Some(
                    Path::new("C:/REAPER/RABBIT/backups/uninstall-unix-1/files")
                        .join(Path::new(path).file_name().unwrap()),
                ),
                trashed_to: None,
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
        administrator_steps: Vec::new(),
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
        report(vendor_outcomes_to_check()),
        report(vec![
            surge_files_left(&[SURGE_VST3], Some(false)),
            reaper_failed(&[REAPER_EXE]),
        ]),
        report(vec![surge_files_left(&[], Some(false)), reaper_failed(&[])]),
        report(vec![pending_restart_surge()]),
        report(vec![
            declined("sws"),
            declined("surge-xt"),
            declined("reaper"),
        ]),
        jaws_report(vec![jaws_failed(UninstallMessage::CleanupRefused {
            refusal: VendorRefusal::UntrustedFolder {
                path: PathBuf::from(JAWS_FOLDER),
            },
        })]),
        jaws_report(vec![UninstallReportItem {
            cleanup: Some(jaws_cleanup_failed_at_once()),
            ..jaws_failed(UninstallMessage::CleanupFailed {
                error: "Access is denied.".to_string(),
            })
        }]),
        jaws_report(vec![jaws_failed(UninstallMessage::CleanupLost {
            error: "the pipe broke".to_string(),
        })]),
        jaws_report(vec![jaws_failed(UninstallMessage::FileInUse {
            path: PathBuf::from(JAWS_SCRIPT),
            error: "sharing violation".to_string(),
        })]),
        // What the helper removed with administrator rights, the script
        // files kept and the note on removing them by hand; and what a
        // failed cleanup left.
        jaws_report_planned(
            UninstallPlanItem {
                kept: vec![
                    KeptPath {
                        path: PathBuf::from(JAWS_SCRIPT).with_file_name("reaper.jkm"),
                        reason: KeptReason::ThereBeforeInstall,
                        note_key: None,
                        exists: true,
                        removable_with_settings: false,
                    },
                    KeptPath {
                        path: PathBuf::from(JAWS_SCRIPT).with_file_name("reaper.jcf"),
                        reason: KeptReason::RecordUnclear,
                        note_key: None,
                        exists: true,
                        removable_with_settings: false,
                    },
                ],
                ..jaws_with_cleanup()
            },
            vec![jaws_removed()],
        ),
        jaws_report(vec![UninstallReportItem {
            cleanup: Some(jaws_cleanup_failed()),
            ..jaws_failed(UninstallMessage::CleanupFailed {
                error: "Access is denied.".to_string(),
            })
        }]),
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
                still_listed: Some(true),
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

/// The vendor outcomes that say what the user can still check: a package
/// Windows no longer lists, one whose list couldn't be read, an uninstaller
/// RABBIT lost track of, and a working folder left behind.
fn vendor_outcomes_to_check() -> Vec<UninstallReportItem> {
    let still_there = |package_id: &str, still_listed| {
        report_item(
            package_id,
            UninstallStatus::Failed,
            UninstallMessage::VendorStillThere {
                still_present: vec![PathBuf::from(
                    "C:/Program Files/Common Files/VST3/Surge Synth Team/Surge XT.vst3",
                )],
                still_listed,
            },
        )
    };
    vec![
        still_there("surge-xt", Some(false)),
        UninstallReportItem {
            problems: vec![
                UninstallProblem::InstalledListUnreadable {
                    error: "denied".to_string(),
                },
                UninstallProblem::RestartListUnreadable {
                    error: "denied".to_string(),
                },
            ],
            ..still_there("osara", None)
        },
        report_item(
            "reaper",
            UninstallStatus::Failed,
            UninstallMessage::VendorLost {
                error: "the pipe broke".to_string(),
            },
        ),
        report_item(
            "reapack",
            UninstallStatus::Failed,
            UninstallMessage::VendorLost {
                error: "the pipe broke".to_string(),
            },
        ),
        UninstallReportItem {
            problems: vec![UninstallProblem::VendorWorkingFolderLeft {
                path: PathBuf::from("C:/ProgramData/RABBIT-uninstall-0123"),
                error: "in use".to_string(),
            }],
            ..report_item(
                "sws",
                UninstallStatus::Removed,
                UninstallMessage::UninstalledByVendor,
            )
        },
    ]
}

/// Windows Settings is only worth naming while Windows still lists the
/// package. Once its own uninstaller took the entry, Done says some files
/// are left and that Windows no longer lists it; when the list couldn't be
/// read, it claims neither.
#[test]
fn a_package_windows_no_longer_lists_isnt_sent_to_windows_settings() {
    let settings = "You can uninstall it in Windows Settings";
    let reasons = |items| uninstall_reasons(&en(), &report(items));
    let listed = reasons(vendor_outcomes());
    assert!(
        listed.iter().any(|reason| reason.contains(settings)),
        "{listed:#?}"
    );

    let reasons = reasons(vendor_outcomes_to_check());
    assert!(
        !reasons.iter().any(|reason| reason.contains(settings)),
        "{reasons:#?}"
    );
    for sentence in [
        "Surge XT's own uninstaller finished, but some of its files are still there. Show \
         details lists them.",
        "Windows no longer lists Surge XT among the installed programs, so it isn't in Windows \
         Settings either.",
        "OSARA's own uninstaller finished, but some of its files are still there.",
        "couldn't check whether Windows still lists OSARA.",
        "so it can't tell whether the files of OSARA that are left go then.",
    ] {
        assert!(
            reasons.iter().any(|reason| reason.contains(sentence)),
            "{sentence}: {reasons:#?}"
        );
    }
    assert!(
        !reasons
            .iter()
            .any(|reason| reason.contains("Windows no longer lists OSARA")),
        "{reasons:#?}"
    );
}

const SURGE_VST3: &str = "C:/Program Files/Common Files/VST3/Surge Synth Team/Surge XT.vst3";
const REAPER_EXE: &str = "C:/Program Files/REAPER (x64)/reaper.exe";

/// A vendor run that didn't remove `package_id`, as the engine reports it:
/// the uninstaller it ran, how it ended, and what it found still there.
fn vendor_run_left(
    package_id: &str,
    exit_code: i32,
    still_present: &[&str],
    message_code: UninstallMessage,
) -> UninstallReportItem {
    let uninstaller = if package_id == "reaper" {
        "C:/Program Files/REAPER (x64)/Uninstall.exe"
    } else {
        "C:/ProgramData/Surge XT/uninstall/unins000.exe"
    };
    UninstallReportItem {
        vendor: Some(VendorOutcome {
            status: VendorStatus::Failed,
            uninstaller: Some(PathBuf::from(uninstaller)),
            launched: true,
            exit_code: Some(exit_code),
            still_present: still_present.iter().map(PathBuf::from).collect(),
            ..VendorOutcome::new(package_id)
        }),
        ..report_item(package_id, UninstallStatus::Failed, message_code)
    }
}

/// Surge XT's uninstaller said it succeeded, but files are still there.
fn surge_files_left(still_present: &[&str], still_listed: Option<bool>) -> UninstallReportItem {
    vendor_run_left(
        "surge-xt",
        0,
        still_present,
        UninstallMessage::VendorStillThere {
            still_present: still_present.iter().map(PathBuf::from).collect(),
            still_listed,
        },
    )
}

/// REAPER's uninstaller ended with an error code.
fn reaper_failed(still_present: &[&str]) -> UninstallReportItem {
    vendor_run_left(
        "reaper",
        2,
        still_present,
        UninstallMessage::VendorFailed { exit_code: 2 },
    )
}

/// Done sends the user to Show details for what a vendor's uninstaller left
/// only when Show details lists it: there, each path is "Still there", once.
/// With nothing to list, Done doesn't point there at all.
#[test]
fn show_details_lists_what_a_vendor_uninstaller_left() {
    let leftover = |item: UninstallReportItem| {
        let leftovers = vec![PathBuf::from(SURGE_VST3)];
        UninstallReportItem { leftovers, ..item }
    };
    for (item, paths) in [
        (
            surge_files_left(&[SURGE_VST3], Some(false)),
            vec![SURGE_VST3],
        ),
        (surge_files_left(&[SURGE_VST3], None), vec![SURGE_VST3]),
        (reaper_failed(&[REAPER_EXE]), vec![REAPER_EXE]),
        // Said once, also when the same path is a leftover too.
        (
            leftover(surge_files_left(&[SURGE_VST3], Some(false))),
            vec![SURGE_VST3],
        ),
    ] {
        let outcome = outcome(&report(vec![item]));
        assert!(
            outcome.status_text.contains("Show details lists"),
            "{}",
            outcome.status_text
        );
        for path in paths {
            let line = format!("  Still there: {path}");
            assert_eq!(
                outcome
                    .detail_lines
                    .iter()
                    .filter(|detail| **detail == line)
                    .count(),
                1,
                "{line}: {:#?}",
                outcome.detail_lines
            );
        }
    }

    for item in [
        reaper_failed(&[]),
        report_item(
            "reaper",
            UninstallStatus::Failed,
            UninstallMessage::VendorFailed { exit_code: 2 },
        ),
        surge_files_left(&[], Some(false)),
        surge_files_left(&[], None),
    ] {
        let outcome = outcome(&report(vec![item]));
        assert!(
            !outcome.status_text.contains("Show details"),
            "{}",
            outcome.status_text
        );
        assert!(
            !outcome
                .detail_lines
                .iter()
                .any(|line| line.contains("Still there")),
            "{:#?}",
            outcome.detail_lines
        );
    }
    let reasons = uninstall_reasons(&en(), &report(vec![reaper_failed(&[])]));
    assert_eq!(
        reasons,
        ["REAPER's own uninstaller ended with error code 2, so REAPER may not be fully removed."]
    );
    let reasons = uninstall_reasons(&en(), &report(vec![surge_files_left(&[], Some(false))]));
    assert_eq!(
        reasons[0],
        "Surge XT's own uninstaller finished, but RABBIT can't confirm that Surge XT is gone."
    );
}

/// An uninstaller RABBIT lost track of may still be running: Done says so,
/// and for REAPER's, to check that REAPER's settings survived. A working
/// folder RABBIT couldn't remove is named, so the user can find it.
#[test]
fn a_lost_uninstaller_and_a_folder_left_behind_are_said() {
    let report = report(vendor_outcomes_to_check());
    let reasons = uninstall_reasons(&en(), &report);
    for sentence in [
        "RABBIT lost track of REAPER's own uninstaller after it started. It may still be \
         running, so RABBIT can't tell what it removed, and stopped there.",
        "RABBIT lost track of ReaPack's own uninstaller after it started.",
        "check that reaper.ini is still in REAPER's resource folder",
        "RABBIT couldn't remove C:/ProgramData/RABBIT-uninstall-0123, the folder it ran SWS \
         Extension's uninstaller from. Only an administrator can delete it.",
    ] {
        assert!(
            reasons.iter().any(|reason| reason.contains(sentence)),
            "{sentence}: {reasons:#?}"
        );
    }
    // REAPER's settings only come up for REAPER's uninstaller.
    assert_eq!(
        reasons
            .iter()
            .filter(|reason| reason.contains("reaper.ini"))
            .count(),
        1,
        "{reasons:#?}"
    );
    assert_eq!(
        outcome(&report).status,
        UninstallOutcomeStatus::CompletedWithProblems
    );
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
    // Said once, right at the start: the same instruction for Surge XT
    // straight after it would only repeat it.
    assert_eq!(
        outcome.status_text.matches("Restart your computer").count(),
        1,
        "{}",
        outcome.status_text
    );
    let details = outcome.detail_lines.join("\n");
    for line in [
        "Restart your computer to finish removing Surge XT.",
        "Surge XT: Uninstalled; the rest goes when you restart the computer",
        "  Its own uninstaller: C:/ProgramData/Surge XT/uninstall/unins000.exe",
        "  Removed: C:/Program Files/Common Files/CLAP/Surge Synth Team/SurgeXTData",
        "  Goes when you restart the computer: C:/Program Files/Common Files/VST3/",
    ] {
        assert!(details.contains(line), "{line}: {details}");
    }

    // When the heading is about something else, the package's own
    // sentence is the only place the restart is mentioned, so it stays.
    let report = self::report(vec![
        pending_restart_surge(),
        checks_failed("sws", REAPER_PROCESS_CHECK),
    ]);
    let outcome = self::outcome(&report);
    assert_eq!(
        outcome.status,
        UninstallOutcomeStatus::CompletedWithProblems
    );
    assert!(
        outcome
            .status_text
            .contains("Restart your computer to finish removing Surge XT."),
        "{}",
        outcome.status_text
    );
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
        "SWS Extension's own uninstaller ended with error code 2, so SWS Extension may not be \
         fully removed.",
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

// ---------------------------------------------------------------------------
// The JAWS scripts: what Done and Show details say about the elevated
// helper's cleanup of the uninstaller folder and registry entry, the script
// files that stay, and JAWS holding a script file open.
// ---------------------------------------------------------------------------

/// `items`, with the JAWS scripts' plan item behind them: a plug-in from
/// the receipt and a script file their installer added go into the backup,
/// then the elevated helper removes the uninstaller folder and entry.
fn jaws_report(items: Vec<UninstallReportItem>) -> UninstallReport {
    jaws_report_planned(jaws_with_cleanup(), items)
}

/// `items`, with `planned` as the JAWS scripts' plan item.
fn jaws_report_planned(
    planned: UninstallPlanItem,
    items: Vec<UninstallReportItem>,
) -> UninstallReport {
    UninstallReport {
        plan: plan(vec![planned]),
        ..report(items)
    }
}

fn jaws_failed(message_code: UninstallMessage) -> UninstallReportItem {
    report_item("jaws-scripts", UninstallStatus::Failed, message_code)
}

/// The JAWS scripts removed: both files went into the backup set, then the
/// helper removed the uninstaller folder and registry entry.
fn jaws_removed() -> UninstallReportItem {
    UninstallReportItem {
        cleanup: Some(jaws_cleanup(
            CleanupStatus::Removed,
            jaws_cleanup_steps(),
            &[],
        )),
        ..removed("jaws-scripts", &[JAWS_DLL, JAWS_SCRIPT])
    }
}

fn jaws_cleanup(
    status: CleanupStatus,
    steps: Vec<CleanupStep>,
    still_present: &[&str],
) -> CleanupOutcome {
    CleanupOutcome {
        package_id: "jaws-scripts".to_string(),
        status,
        refusal: None,
        steps,
        still_present: still_present.iter().map(ToString::to_string).collect(),
        error: None,
    }
}

fn file_step(path: &str, result: CleanupResult) -> CleanupStep {
    CleanupStep {
        target: CleanupTarget::File {
            path: PathBuf::from(path),
        },
        result,
    }
}

fn folder_step(result: CleanupResult) -> CleanupStep {
    CleanupStep {
        target: CleanupTarget::Folder {
            path: PathBuf::from(JAWS_FOLDER),
        },
        result,
    }
}

fn key_step(result: CleanupResult) -> CleanupStep {
    CleanupStep {
        target: CleanupTarget::RegistryKey {
            key: JAWS_KEY.to_string(),
        },
        result,
    }
}

/// Every step of a cleanup that went through. `Uninstall.dat` was gone
/// already, which counts as gone but isn't listed as removed.
fn jaws_cleanup_steps() -> Vec<CleanupStep> {
    vec![
        file_step(&jaws_folder_file("Uninstall.dat"), CleanupResult::NotFound),
        file_step(&jaws_folder_file("Uninstall.exe"), CleanupResult::Removed),
        folder_step(CleanupResult::Removed),
        key_step(CleanupResult::Removed),
    ]
}

/// A cleanup that stopped when Windows wouldn't delete `Uninstall.exe`:
/// `Uninstall.dat` is gone, the folder and the registry entry are still
/// there.
fn jaws_cleanup_failed() -> CleanupOutcome {
    CleanupOutcome {
        error: Some("Access is denied.".to_string()),
        ..jaws_cleanup(
            CleanupStatus::Failed,
            vec![
                file_step(&jaws_folder_file("Uninstall.dat"), CleanupResult::Removed),
                file_step(
                    &jaws_folder_file("Uninstall.exe"),
                    CleanupResult::Failed {
                        error: "Access is denied.".to_string(),
                    },
                ),
            ],
            &[JAWS_FOLDER, JAWS_KEY],
        )
    }
}

/// A cleanup whose first delete failed: Windows wouldn't delete
/// `Uninstall.dat`, so nothing is gone.
fn jaws_cleanup_failed_at_once() -> CleanupOutcome {
    CleanupOutcome {
        error: Some("Access is denied.".to_string()),
        ..jaws_cleanup(
            CleanupStatus::Failed,
            vec![file_step(
                &jaws_folder_file("Uninstall.dat"),
                CleanupResult::Failed {
                    error: "Access is denied.".to_string(),
                },
            )],
            &[JAWS_FOLDER, JAWS_KEY],
        )
    }
}

/// `cleanup` failed, so the scripts' files went back.
fn jaws_cleanup_failed_item(cleanup: CleanupOutcome) -> UninstallReportItem {
    UninstallReportItem {
        put_back: vec![PathBuf::from(JAWS_DLL), PathBuf::from(JAWS_SCRIPT)],
        cleanup: Some(cleanup),
        ..jaws_failed(UninstallMessage::CleanupFailed {
            error: "Access is denied.".to_string(),
        })
    }
}

/// After the helper removed the JAWS scripts' uninstaller folder and
/// registry entry, Show details lists what it removed with administrator
/// rights, file by file, then the folder and the entry as regedit names
/// it, after the files that went into the backup. A file that was gone
/// already isn't listed as removed. The restore command follows, for the
/// files in the backup set.
#[test]
fn done_lists_the_cleanup_with_administrator_rights_and_the_registry_key() {
    let report = jaws_report_planned(
        UninstallPlanItem {
            notes: Vec::new(),
            ..jaws_with_cleanup()
        },
        vec![jaws_removed()],
    );
    let outcome = outcome(&report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::Success);
    assert_eq!(outcome.heading, format!("RABBIT uninstalled {JAWS}."));
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER".to_string(),
            "Also remove settings: no".to_string(),
            format!("{JAWS}: Uninstalled"),
            format!("  Removed: {JAWS_DLL}"),
            format!("  Removed: {JAWS_SCRIPT}"),
            format!(
                "  Removed with administrator rights: {}",
                jaws_folder_file("Uninstall.exe")
            ),
            format!("  Removed with administrator rights: {JAWS_FOLDER}"),
            format!("  Removed from the registry with administrator rights: {JAWS_KEY}"),
            "Backup set: uninstall-unix-1".to_string(),
            format!(
                "To put {JAWS} back, close REAPER and run: rabbit restore-backup --resource-path \
                 \"C:/REAPER\" --backup-id uninstall-unix-1 --package jaws-scripts --apply"
            ),
        ]
    );
}

/// A cleanup that stopped part way put the scripts' files back, and Done
/// says Show details lists what is already gone. It does: what the helper
/// removed with administrator rights, then the folder and the registry
/// entry that are still there, with Windows' reason and the steps to
/// remove the rest by hand below. A lost helper reports no steps, so
/// nothing is listed as gone.
#[test]
fn show_details_after_a_failed_cleanup_lists_what_is_gone_and_what_is_still_there() {
    let failed = UninstallReportItem {
        put_back: vec![PathBuf::from(JAWS_DLL), PathBuf::from(JAWS_SCRIPT)],
        cleanup: Some(jaws_cleanup_failed()),
        ..jaws_failed(UninstallMessage::CleanupFailed {
            error: format!("{}: Access is denied.", jaws_folder_file("Uninstall.exe")),
        })
    };
    let outcome = outcome(&jaws_report(vec![failed]));
    assert_eq!(outcome.status, UninstallOutcomeStatus::Failed);
    assert!(
        outcome
            .status_text
            .contains("Show details lists what is already gone."),
        "{}",
        outcome.status_text
    );
    let package = outcome
        .detail_lines
        .iter()
        .position(|line| line == &format!("{JAWS}: Couldn't be uninstalled"))
        .unwrap_or_else(|| panic!("{:#?}", outcome.detail_lines));
    let expected: Vec<String> = [
        format!(
            "  Removed with administrator rights: {}",
            jaws_folder_file("Uninstall.dat")
        ),
        format!("  Still there: {JAWS_FOLDER}"),
        format!("  Still there: {JAWS_KEY}"),
        format!(
            "  RABBIT couldn't remove all of {JAWS}'s uninstaller folder and registry entry ({}: \
             Access is denied.), so it put back what it had moved. Show details lists what is \
             already gone.",
            jaws_folder_file("Uninstall.exe")
        ),
    ]
    .into_iter()
    .chain(by_hand_lines_en().iter().map(|line| format!("  {line}")))
    .collect();
    assert_eq!(
        outcome.detail_lines[package + 1..package + 1 + expected.len()],
        expected[..]
    );
    let details = outcome.detail_lines.join("\n");
    assert!(
        !details.contains(&format!(
            "Removed with administrator rights: {}",
            jaws_folder_file("Uninstall.exe")
        )),
        "{details}"
    );
    // The script files went back, so the note on removing those that stay
    // by hand is for a removed package only.
    let note = en().text(SCRIPTS_NOTE_KEY).value;
    assert!(!details.contains(&note), "{details}");
    assert!(!details.contains("restore-backup"), "{details}");

    let lost = UninstallReportItem {
        cleanup: Some(CleanupOutcome::lost("jaws-scripts", "the pipe broke")),
        ..jaws_failed(UninstallMessage::CleanupLost {
            error: "the pipe broke".to_string(),
        })
    };
    let details = outcome_details(&jaws_report(vec![lost]));
    assert!(!details.contains("administrator rights:"), "{details}");
    assert!(!details.contains("Still there:"), "{details}");
}

fn outcome_details(report: &UninstallReport) -> String {
    outcome(report).detail_lines.join("\n")
}

/// Once the scripts are gone, the script files their installer added that
/// stay, because they were there before RABBIT ran the installer or
/// RABBIT's records of them disagree, are listed by name, as long as they
/// are there; the keeps sentence only said such files stay. The note on
/// removing them by hand follows.
#[test]
fn show_details_lists_files_kept_because_they_were_there_before_and_the_by_hand_note() {
    let settings = "C:/Users/me/AppData/Roaming/Freedom Scientific/JAWS/2025/Settings/enu";
    let kept = |name: &str, reason, exists| KeptPath {
        path: PathBuf::from(format!("{settings}/{name}")),
        reason,
        note_key: None,
        exists,
        removable_with_settings: false,
    };
    let planned = UninstallPlanItem {
        kept: vec![
            kept("reaper.jkm", KeptReason::ThereBeforeInstall, true),
            kept("reaper.jcf", KeptReason::RecordUnclear, true),
            kept("reaper.jsb", KeptReason::ThereBeforeInstall, false),
            kept("reaper.ini", KeptReason::UserData, true),
        ],
        ..jaws_with_cleanup()
    };
    let report = jaws_report_planned(planned.clone(), vec![jaws_removed()]);
    let lines = outcome(&report).detail_lines;
    let first = lines
        .iter()
        .position(|line| line.contains("reaper.jkm"))
        .unwrap_or_else(|| panic!("{lines:#?}"));
    assert_eq!(
        lines[first..first + 3],
        [
            format!(
                "  Kept, because it was already there when RABBIT ran the installer: \
                 {settings}/reaper.jkm"
            ),
            format!("  Kept, because RABBIT's records of it disagree: {settings}/reaper.jcf"),
            format!("  {}", en().text(SCRIPTS_NOTE_KEY).value),
        ]
    );
    // The note names reaper.ini by itself, so look for the paths.
    let details = lines.join("\n");
    for name in ["reaper.jsb", "reaper.ini"] {
        let path = format!("{settings}/{name}");
        assert!(!details.contains(&path), "{path}\n{details}");
    }

    // A package that stayed keeps all of its files, so listing some as
    // kept would suggest the others went, and the note on removing them by
    // hand waits until the package is gone.
    let report = jaws_report_planned(
        planned,
        vec![jaws_failed(UninstallMessage::FileInUse {
            path: PathBuf::from(JAWS_SCRIPT),
            error: "sharing violation".to_string(),
        })],
    );
    let details = outcome_details(&report);
    assert!(!details.contains("Kept, because"), "{details}");
    assert!(
        !details.contains(&en().text(SCRIPTS_NOTE_KEY).value),
        "{details}"
    );
}

/// The helper didn't trust the folder or entry when it checked them again:
/// Done says what was put back, why in the user's language, and then the
/// steps to remove the scripts by hand, each on a line of its own, as the
/// row said them.
#[test]
fn done_after_a_refused_cleanup_shows_the_reason_and_the_by_hand_steps() {
    let report = jaws_report(vec![jaws_failed(UninstallMessage::CleanupRefused {
        refusal: VendorRefusal::RegistryKeyHasSubkeys,
    })]);
    let expected: Vec<String> = [
        format!(
            "RABBIT checked {JAWS}'s uninstaller folder in Program Files and its registry entry \
             again and didn't trust what it found, so it put back what it had moved."
        ),
        format!("{JAWS}'s registry entry holds more than RABBIT expects."),
    ]
    .into_iter()
    .chain(by_hand_lines_en())
    .collect();
    assert_eq!(uninstall_reasons(&en(), &report), expected);

    let outcome = outcome(&report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::Failed);
    let details = outcome.detail_lines;
    let first = details
        .iter()
        .position(|line| line.trim() == expected[2])
        .unwrap_or_else(|| panic!("{details:#?}"));
    let indented: Vec<String> = expected[2..]
        .iter()
        .map(|line| format!("  {line}"))
        .collect();
    assert_eq!(details[first..first + indented.len()], indented[..]);
}

/// A cleanup that failed part way, or lost its helper, puts back what had
/// moved, and Done says so with Windows' or RABBIT's own words. Part of the
/// folder or entry may be gone, and without a receipt nothing may be left
/// to find the scripts by, so the steps to remove them by hand follow. When
/// the first delete failed, nothing is gone, and Done says that rather than
/// pointing to Show details.
#[test]
fn a_failed_or_lost_cleanup_says_what_was_put_back() {
    let reasons = |item| uninstall_reasons(&en(), &jaws_report(vec![item]));
    let then_by_hand = |first: String| {
        std::iter::once(first)
            .chain(by_hand_lines_en())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        reasons(jaws_cleanup_failed_item(jaws_cleanup_failed())),
        then_by_hand(format!(
            "RABBIT couldn't remove all of {JAWS}'s uninstaller folder and registry entry (Access \
             is denied.), so it put back what it had moved. Show details lists what is already \
             gone."
        ))
    );
    assert_eq!(
        reasons(jaws_cleanup_failed_item(jaws_cleanup_failed_at_once())),
        then_by_hand(format!(
            "RABBIT couldn't remove {JAWS}'s uninstaller folder and registry entry (Access is \
             denied.), so it put back what it had moved. Nothing of that folder or its registry \
             entry was removed."
        ))
    );
    assert_eq!(
        reasons(UninstallReportItem {
            cleanup: Some(CleanupOutcome::lost("jaws-scripts", "the pipe broke")),
            ..jaws_failed(UninstallMessage::CleanupLost {
                error: "the pipe broke".to_string(),
            })
        }),
        then_by_hand(format!(
            "RABBIT lost contact with its helper while it removed {JAWS}'s uninstaller folder \
             and registry entry (the pipe broke), so it put back what it had moved. Part of that \
             folder or its registry entry may be gone."
        ))
    );
}

/// After a cleanup whose first delete failed, Show details lists nothing as
/// removed with administrator rights, only the folder and registry entry
/// that are still there.
#[test]
fn show_details_after_a_cleanup_that_removed_nothing_lists_only_what_is_still_there() {
    let report = jaws_report(vec![
        jaws_cleanup_failed_item(jaws_cleanup_failed_at_once()),
    ]);
    let lines = outcome(&report).detail_lines;
    let package = lines
        .iter()
        .position(|line| line == &format!("{JAWS}: Couldn't be uninstalled"))
        .unwrap_or_else(|| panic!("{lines:#?}"));
    assert_eq!(
        lines[package + 1..package + 3],
        [
            format!("  Still there: {JAWS_FOLDER}"),
            format!("  Still there: {JAWS_KEY}"),
        ]
    );
    let details = lines.join("\n");
    assert!(!details.contains("administrator rights:"), "{details}");
    assert!(!details.contains("already gone"), "{details}");
}

/// JAWS may hold a script file its installer added open. Closing the
/// user's screen reader is no answer, so Done says to wait and try again,
/// then to restart JAWS, however Windows spells the path. A plug-in in
/// REAPER's folder gets the usual sentence.
#[test]
fn a_jaws_file_in_use_says_to_wait_and_then_restart_jaws() {
    let reasons = |path: &str| {
        uninstall_reasons(
            &en(),
            &jaws_report(vec![jaws_failed(UninstallMessage::FileInUse {
                path: PathBuf::from(path),
                error: "sharing violation".to_string(),
            })]),
        )
    };
    for path in [JAWS_SCRIPT.to_string(), JAWS_SCRIPT.to_uppercase()] {
        assert_eq!(
            reasons(&path),
            [format!(
                "{JAWS} wasn't uninstalled, because JAWS or another program is using {path}. \
                 Wait a moment, then run RABBIT again. If that doesn't help, restart JAWS first."
            )]
        );
    }
    assert_eq!(
        reasons(JAWS_DLL),
        [format!(
            "{JAWS} wasn't uninstalled, because another program is using {JAWS_DLL}. Close that \
             program, then run RABBIT again."
        )]
    );
}

// ---------------------------------------------------------------------------
// After a macOS removal: the Trash and the administrator command.
// ---------------------------------------------------------------------------

const SURGE_APP: &str = "/Applications/Surge XT.app";
const SURGE_VST3_MAC: &str = "/Library/Audio/Plug-Ins/VST3/Surge XT.vst3";
const IN_TRASH: &str = "/Users/me/.Trash/REAPER 2026-11-07 15.30.12.app";

/// A macOS report of `items`, with the steps the administrator command was
/// given and the plan items they came from.
fn mac_report(
    items: Vec<UninstallReportItem>,
    plan_items: Vec<UninstallPlanItem>,
    steps: Vec<AdministratorStepRecord>,
) -> UninstallReport {
    UninstallReport {
        resource_path: PathBuf::from(MAC_RESOURCE),
        platform: Platform::MacOs,
        target: InstallationKind::Standard,
        backup_set_id: None,
        administrator_steps: steps,
        plan: mac_plan(plan_items),
        ..report(items)
    }
}

fn step(
    package_id: Option<&str>,
    kind: AdministratorStepKind,
    target: &str,
    verdict: AdministratorStepVerdict,
) -> AdministratorStepRecord {
    AdministratorStepRecord {
        package_id: package_id.map(str::to_string),
        kind,
        target: target.to_string(),
        destination: None,
        verdict,
    }
}

fn whole(path: &str, trashed_to: Option<&str>) -> RemovedFile {
    RemovedFile {
        path: PathBuf::from(path),
        reason: RemovalReason::Tree,
        size: 0,
        changed_since_install: false,
        backup_path: None,
        trashed_to: trashed_to.map(PathBuf::from),
    }
}

/// REAPER moved to the Trash, `trashed_to` where RABBIT saw it there.
fn trashed_reaper(trashed_to: Option<&str>) -> UninstallReportItem {
    UninstallReportItem {
        removed: vec![whole(MAC_REAPER_APP, trashed_to)],
        ..report_item(
            "reaper",
            UninstallStatus::Removed,
            UninstallMessage::MovedToTrash {
                trashed_to: trashed_to.map(PathBuf::from),
            },
        )
    }
}

fn surge_removed_as_administrator() -> UninstallReportItem {
    UninstallReportItem {
        removed: vec![whole(SURGE_APP, None), whole(SURGE_VST3_MAC, None)],
        ..report_item(
            "surge-xt",
            UninstallStatus::Removed,
            UninstallMessage::RemovedAsAdministrator,
        )
    }
}

/// Surge XT, which the administrator command stopped at `path`, or with no
/// `path`, ended without saying where it stopped.
fn surge_step_failed(
    path: Option<&str>,
    gone: &[&str],
    left: &[&str],
    refused_by_macos: bool,
) -> UninstallReportItem {
    report_item(
        "surge-xt",
        UninstallStatus::Failed,
        UninstallMessage::AdministratorStepFailed {
            path: path.map(PathBuf::from),
            ended_unclearly: path.is_none(),
            gone: gone.iter().map(PathBuf::from).collect(),
            left: left.iter().map(PathBuf::from).collect(),
            refused_by_macos,
            detail: Some("rm: Surge XT.vst3: Operation not permitted".to_string()),
        },
    )
}

const APP_MANAGEMENT: &str = "If macOS refused it: in System Settings > Privacy & Security > App \
                              Management, turn on RABBIT. Then quit RABBIT, start it again and \
                              uninstall again.";

/// Cancelling macOS's password window says so in macOS's terms; Windows
/// keeps its own sentence.
#[test]
fn a_cancelled_password_window_says_nothing_was_uninstalled_on_macos() {
    let report = mac_report(
        vec![declined("surge-xt"), declined("reaper")],
        vec![admin_surge(), trash_reaper()],
        Vec::new(),
    );
    let outcome = outcome(&report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::PermissionDeclined);
    assert_eq!(
        outcome.heading,
        "Nothing was uninstalled, because the administrator password window was cancelled."
    );
    assert_eq!(outcome.status_text, outcome.heading);
}

/// An app moved to the Trash says where it is there only when RABBIT saw
/// it there, and that it can come back until the Trash is emptied. Show
/// details names where it was and where it is now. Nothing of it is in the
/// backup set, so no restore command is offered for it, even when another
/// package's files went into one.
#[test]
fn an_app_in_the_trash_says_where_it_is_only_when_rabbit_saw_it() {
    let report = UninstallReport {
        backup_set_id: Some("uninstall-unix-1".to_string()),
        ..mac_report(
            vec![
                removed(
                    "sws",
                    &["/Users/me/REAPER/UserPlugins/reaper_sws-arm64.dylib"],
                ),
                trashed_reaper(Some(IN_TRASH)),
            ],
            vec![removable_sws(), trash_reaper()],
            Vec::new(),
        )
    };
    let outcome = outcome(&report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::Success);
    assert!(
        outcome.status_text.contains(&format!(
            "RABBIT moved REAPER to the Trash. It is there as {IN_TRASH}. Until you empty the \
             Trash, you can drag it back from there."
        )),
        "{}",
        outcome.status_text
    );
    assert!(
        !outcome.status_text.contains("Put Back"),
        "{}",
        outcome.status_text
    );
    let details = outcome.detail_lines.join("\n");
    for line in [
        format!("REAPER: Uninstalled\n  Moved to the Trash: {MAC_REAPER_APP}"),
        format!("  Now in the Trash: {IN_TRASH}"),
        "--package sws --apply".to_string(),
    ] {
        assert!(details.contains(&line), "{line}\n{details}");
    }
    assert!(!details.contains("--package reaper"), "{details}");

    // macOS didn't say where, and RABBIT couldn't see it: no place named.
    let outcome = self::outcome(&mac_report(
        vec![trashed_reaper(None)],
        vec![trash_reaper()],
        Vec::new(),
    ));
    assert_eq!(
        uninstall_reasons(
            &en(),
            &mac_report(vec![trashed_reaper(None)], vec![], vec![])
        ),
        [
            "RABBIT moved REAPER to the Trash. Until you empty the Trash, you can drag it back from \
          there."
        ]
    );
    let details = outcome.detail_lines.join("\n");
    assert!(!details.contains("Now in the Trash"), "{details}");
    assert!(!details.contains("restore-backup"), "{details}");
}

/// An app the administrator command moved into the Trash keeps the owner it
/// had: Finder offers no Put Back for it, and taking it out or emptying the
/// Trash may ask for a password. Only that app is said to.
#[test]
fn an_app_the_administrator_command_moved_keeps_its_owner() {
    let owner = "It keeps the owner it had, so Finder offers no Put Back for it. Emptying the \
                 Trash may ask for an administrator's password, and so may dragging it back.";
    let moved_by_root = mac_report(
        vec![trashed_reaper(None)],
        vec![root_moved_reaper()],
        vec![step(
            Some("reaper"),
            AdministratorStepKind::MoveToTrash,
            MAC_REAPER_APP,
            AdministratorStepVerdict::Ran,
        )],
    );
    let reasons = uninstall_reasons(&en(), &moved_by_root);
    assert!(reasons.contains(&owner.to_string()), "{reasons:#?}");

    let moved_as_user = mac_report(vec![trashed_reaper(None)], vec![trash_reaper()], Vec::new());
    let reasons = uninstall_reasons(&en(), &moved_as_user);
    assert!(
        !reasons.iter().any(|reason| reason.contains("owner")),
        "{reasons:#?}"
    );
}

/// A move to the Trash that didn't happen changed nothing; one macOS
/// refused points to App Management. A move RABBIT can't confirm says where
/// to look, and that the run stopped.
#[test]
fn a_trash_move_that_failed_or_is_unclear_says_what_to_do() {
    let failed = |refused_by_macos| {
        report_item(
            "reaper",
            UninstallStatus::Failed,
            UninstallMessage::TrashFailed {
                error: "You don't have permission.".to_string(),
                refused_by_macos,
            },
        )
    };
    let reasons = uninstall_reasons(&en(), &mac_report(vec![failed(true)], vec![], vec![]));
    assert_eq!(
        reasons,
        [
            "RABBIT couldn't move REAPER to the Trash, so nothing of it changed (You don't have \
             permission.)."
                .to_string(),
            APP_MANAGEMENT.to_string(),
        ]
    );
    let reasons = uninstall_reasons(&en(), &mac_report(vec![failed(false)], vec![], vec![]));
    assert_eq!(reasons.len(), 1, "{reasons:#?}");

    let unclear = report_item(
        "reaper",
        UninstallStatus::Failed,
        UninstallMessage::TrashUnclear {
            path: PathBuf::from(MAC_REAPER_APP),
        },
    );
    let outcome = outcome(&mac_report(vec![unclear], vec![], vec![]));
    assert_eq!(outcome.status, UninstallOutcomeStatus::Failed);
    assert!(
        outcome.status_text.contains(
            "RABBIT asked macOS to move /Applications/REAPER.app to the Trash, but can't confirm \
             that it is there now. Look for REAPER in the Trash and in its folder in Finder. \
             RABBIT stopped there."
        ),
        "{}",
        outcome.status_text
    );
}

/// A package the administrator command removed only in part is never
/// "nothing was uninstalled": Done says what is gone and what is left
/// sits in Show details, with macOS's own words, how to get past App
/// Management, and how to get the package back. Nothing of it is said to
/// be put back.
#[test]
fn a_partly_removed_package_is_never_nothing_uninstalled() {
    let report = mac_report(
        vec![
            surge_step_failed(Some(SURGE_VST3_MAC), &[SURGE_APP], &[SURGE_VST3_MAC], true),
            report_item(
                "reaper",
                UninstallStatus::NotReached,
                UninstallMessage::RunStopped {
                    package_id: "surge-xt".to_string(),
                },
            ),
        ],
        vec![admin_surge(), trash_reaper()],
        Vec::new(),
    );
    let outcome = outcome(&report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::PartlyRemoved);
    assert_eq!(outcome.heading, "RABBIT removed only part of Surge XT.");
    assert_eq!(
        outcome.status_text,
        format!(
            "RABBIT removed only part of Surge XT.\n\nThe administrator command stopped at \
             {SURGE_VST3_MAC}, so Surge XT is only partly removed. Show details lists what is \
             gone and what is still there. RABBIT stopped there.\n{APP_MANAGEMENT}\n\nTo get \
             one back, run RABBIT again and tick it on the Install or update tab."
        )
    );
    assert!(
        !outcome.status_text.contains("put back"),
        "{}",
        outcome.status_text
    );
    let details = outcome.detail_lines.join("\n");
    for line in [
        format!(
            "Surge XT: Couldn't be uninstalled\n  Removed with administrator rights: {SURGE_APP}"
        ),
        format!("  Still there: {SURGE_VST3_MAC}"),
        "  macOS said: rm: Surge XT.vst3: Operation not permitted".to_string(),
    ] {
        assert!(details.contains(&line), "{line}\n{details}");
    }

    // Stopped before any tree of it went: a plain failure. `rm -R -f`
    // deletes all it can inside a tree before it fails, so the tree it
    // stopped at is never said to be whole, only that nothing else went.
    for (path, sentence) in [
        (
            Some(SURGE_APP),
            format!(
                "The administrator command stopped at {SURGE_APP}, which may now be only partly \
                 there. Nothing else of Surge XT was removed. RABBIT stopped there."
            ),
        ),
        (
            None,
            "The administrator command ended without saying where it stopped, and nothing of \
             Surge XT was removed. RABBIT stopped there."
                .to_string(),
        ),
    ] {
        let outcome = self::outcome(&mac_report(
            vec![surge_step_failed(path, &[], &[SURGE_APP], false)],
            vec![admin_surge()],
            Vec::new(),
        ));
        assert_eq!(outcome.status, UninstallOutcomeStatus::Failed);
        assert_eq!(
            outcome.status_text,
            format!("Uninstalling failed.\n\n{sentence}")
        );
    }
    let reasons = uninstall_reasons(
        &en(),
        &mac_report(
            vec![surge_step_failed(
                None,
                &[SURGE_APP],
                &[SURGE_VST3_MAC],
                false,
            )],
            vec![],
            vec![],
        ),
    );
    assert_eq!(
        reasons,
        [
            "The administrator command ended without saying where it stopped, and Surge XT is only \
          partly removed. Show details lists what is gone and what is still there. RABBIT \
          stopped there."
        ]
    );
}

/// What the administrator command removed is listed path by path, and only
/// a package in the backup set gets a restore command. Receipts macOS still
/// lists, ones RABBIT couldn't check, and a command that ended unclearly
/// are each said plainly, with no harm claimed where there is none.
#[test]
fn removed_as_administrator_lists_each_path_and_offers_no_restore() {
    let surge = UninstallReportItem {
        problems: vec![
            UninstallProblem::InstallerRecordsRemain {
                ids: vec!["org.surge-synth-team.surge-xt.app.pkg".to_string()],
            },
            UninstallProblem::AdministratorEndedUnclearly { status: Some(255) },
        ],
        ..surge_removed_as_administrator()
    };
    let report = UninstallReport {
        backup_set_id: Some("uninstall-unix-1".to_string()),
        ..mac_report(
            vec![
                surge,
                removed(
                    "sws",
                    &["/Users/me/REAPER/UserPlugins/reaper_sws-arm64.dylib"],
                ),
            ],
            vec![admin_surge(), removable_sws()],
            Vec::new(),
        )
    };
    let outcome = outcome(&report);
    assert_eq!(
        outcome.status,
        UninstallOutcomeStatus::CompletedWithProblems
    );
    for sentence in [
        "macOS's installer still lists receipts of Surge XT, though its files are gone. That does \
         no harm; Show details names them.",
        "The administrator command ended without saying how, but everything of Surge XT is gone.",
    ] {
        assert!(
            outcome.status_text.contains(sentence),
            "{sentence}\n{}",
            outcome.status_text
        );
    }
    let details = outcome.detail_lines.join("\n");
    for line in [
        format!("  Removed with administrator rights: {SURGE_APP}"),
        format!("  Removed with administrator rights: {SURGE_VST3_MAC}"),
        "  Still listed by macOS's installer: org.surge-synth-team.surge-xt.app.pkg".to_string(),
        "--package sws --apply".to_string(),
    ] {
        assert!(details.contains(&line), "{line}\n{details}");
    }
    assert!(!details.contains("--package surge-xt"), "{details}");

    let unchecked = UninstallReportItem {
        problems: vec![UninstallProblem::InstallerRecordsUnchecked {
            error: "pkgutil failed".to_string(),
        }],
        ..surge_removed_as_administrator()
    };
    assert_eq!(
        uninstall_reasons(&en(), &mac_report(vec![unchecked], vec![], vec![])),
        [
            "RABBIT couldn't check that macOS's installer forgot Surge XT (pkgutil failed). That \
          does no harm: its files are gone."
        ]
    );
}

/// The restore list names only packages whose files went into the backup
/// set. On Windows too: a vendor's uninstaller in the same run as SWS
/// leaves nothing in the set to restore.
#[test]
fn the_restore_list_names_only_packages_in_the_backup_set() {
    let surge = report_item(
        "surge-xt",
        UninstallStatus::Removed,
        UninstallMessage::UninstalledByVendor,
    );
    let outcome = outcome(&report(vec![
        removed("sws", &["C:/REAPER/UserPlugins/reaper_sws-x64.dll"]),
        surge,
    ]));
    let restores: Vec<&String> = outcome
        .detail_lines
        .iter()
        .filter(|line| line.contains("restore-backup"))
        .collect();
    assert_eq!(restores.len(), 1, "{restores:#?}");
    assert!(restores[0].contains("--package sws"), "{restores:#?}");
}

/// The administrator command's first step found a program it looks for
/// running, or pgrep couldn't check, so the command changed nothing: Done
/// says so once, naming every program root looked for, rather than as a
/// check made before the run, and claims no cause it can't know.
#[test]
fn a_failed_program_check_by_the_administrator_command_names_the_programs() {
    let blocked = |package_id: &str| {
        report_item(
            package_id,
            UninstallStatus::Blocked,
            UninstallMessage::ChecksFailed {
                checks: vec![check(PROGRAMS_RUNNING_CHECK, PreflightStatus::Fail)],
            },
        )
    };
    let report = mac_report(
        vec![blocked("surge-xt"), blocked("reaper")],
        vec![admin_surge(), root_moved_reaper()],
        vec![step(
            None,
            AdministratorStepKind::ProgramsClosed,
            "Surge XT, Surge XT Effects, REAPER, reaper64",
            AdministratorStepVerdict::Failed,
        )],
    );
    let outcome = outcome(&report);
    assert_eq!(outcome.status, UninstallOutcomeStatus::NothingDone);
    assert_eq!(
        outcome.status_text,
        "Nothing was uninstalled.\n\nThe administrator command found one of these programs \
         running, or couldn't check for them, so it changed nothing: Surge XT, Surge XT \
         Effects, REAPER, reaper64. Close any that is open, then run RABBIT again."
    );
}

/// A package still there although the administrator command didn't stop at
/// its step is never said to be where the command stopped, nor to have
/// ended without saying where: the command stopped at another package's
/// step, or ran to its end, and this one is still there all the same.
#[test]
fn a_package_left_by_a_command_that_stopped_elsewhere_says_what_is_still_there() {
    let left_by_command = |package_id: &str, gone: &[&str], left: &str| {
        report_item(
            package_id,
            UninstallStatus::Failed,
            UninstallMessage::AdministratorStepFailed {
                path: None,
                ended_unclearly: false,
                gone: gone.iter().map(PathBuf::from).collect(),
                left: vec![PathBuf::from(left)],
                refused_by_macos: false,
                detail: None,
            },
        )
    };
    // Surge XT's steps ran, and RABBIT can't see that its app is gone; then
    // the step that moves REAPER stopped the command.
    let reaper_stopped_it = report_item(
        "reaper",
        UninstallStatus::Failed,
        UninstallMessage::AdministratorStepFailed {
            path: Some(PathBuf::from(MAC_REAPER_APP)),
            ended_unclearly: false,
            gone: Vec::new(),
            left: vec![PathBuf::from(MAC_REAPER_APP)],
            refused_by_macos: false,
            detail: None,
        },
    );
    let report = mac_report(
        vec![
            left_by_command("surge-xt", &[SURGE_VST3_MAC], SURGE_APP),
            reaper_stopped_it,
        ],
        vec![admin_surge(), root_moved_reaper()],
        vec![
            step(
                Some("surge-xt"),
                AdministratorStepKind::Delete,
                SURGE_APP,
                AdministratorStepVerdict::Ran,
            ),
            step(
                Some("reaper"),
                AdministratorStepKind::MoveToTrash,
                MAC_REAPER_APP,
                AdministratorStepVerdict::Failed,
            ),
        ],
    );
    let reasons = uninstall_reasons(&en(), &report);
    assert_eq!(
        reasons,
        [
            format!(
                "The administrator command ran, but {SURGE_APP} is still there, or RABBIT can't \
                 see that it is gone, so Surge XT isn't fully removed. Show details lists what is \
                 gone and what is still there. RABBIT stopped there."
            ),
            format!(
                "The administrator command stopped at {MAC_REAPER_APP}, so nothing of REAPER was \
                 removed. RABBIT stopped there."
            ),
        ]
    );

    // `mv -n` found the name taken in the Trash and left the app where it
    // was, with success: the command ran to its end and never stopped.
    let report = mac_report(
        vec![left_by_command("reaper", &[], MAC_REAPER_APP)],
        vec![root_moved_reaper()],
        vec![step(
            Some("reaper"),
            AdministratorStepKind::MoveToTrash,
            MAC_REAPER_APP,
            AdministratorStepVerdict::Ran,
        )],
    );
    let reasons = uninstall_reasons(&en(), &report);
    assert_eq!(
        reasons,
        [format!(
            "The administrator command ran, but {MAC_REAPER_APP} is still there, or RABBIT can't \
             see that it is gone, so REAPER isn't fully removed. Show details lists what is gone \
             and what is still there. RABBIT stopped there."
        )]
    );
    for reason in &reasons {
        assert!(!reason.contains("stopped at"), "{reason}");
        assert!(!reason.contains("without saying"), "{reason}");
    }
}

/// The administrator command that couldn't be put together asked for no
/// password at all; one whose password window failed got no rights. Each
/// says which, and that nothing was uninstalled.
#[test]
fn an_administrator_command_that_never_ran_says_why() {
    let elevation_failed = |status| {
        report_item(
            "surge-xt",
            status,
            UninstallMessage::ElevationFailed {
                error: "too many steps".to_string(),
            },
        )
    };
    assert_eq!(
        uninstall_reasons(
            &en(),
            &mac_report(
                vec![elevation_failed(UninstallStatus::Blocked)],
                vec![],
                vec![]
            )
        ),
        [
            "RABBIT couldn't put the administrator command for Surge XT together (too many steps), \
          so it asked for no password and uninstalled nothing."
        ]
    );
    assert_eq!(
        uninstall_reasons(
            &en(),
            &mac_report(
                vec![elevation_failed(UninstallStatus::Failed)],
                vec![],
                vec![]
            )
        ),
        ["RABBIT couldn't get administrator rights (too many steps), so nothing was uninstalled."]
    );
}

/// An app that changed between Review and the run, so that only Finder can
/// move it now, gets the same reason and Finder steps as its row would.
#[test]
fn an_app_only_finder_can_move_now_gets_the_finder_steps_on_done() {
    let item = report_item(
        "reaper",
        UninstallStatus::Blocked,
        UninstallMessage::NotRemovable {
            removability: Removability::ByHand {
                path: PathBuf::from(MAC_REAPER_APP),
                reason: ByHandReason::Locked,
            },
        },
    );
    let reasons = uninstall_reasons(&en(), &mac_report(vec![item], vec![], vec![]));
    assert_eq!(reasons[0], "/Applications/REAPER.app is locked.");
    assert!(reasons[1].contains("Untick Locked"), "{reasons:#?}");
}

/// Running as root under sudo uninstalls nothing, and Done says what to do
/// instead, in the user's language.
#[test]
fn running_as_root_says_to_start_rabbit_as_yourself() {
    let (_, model) = langpack_model(PlanActionKind::Install);
    let outcome = wizard_uninstall_outcome_from_error(
        &model,
        &en(),
        Path::new(MAC_RESOURCE),
        &ids(&["reaper"]),
        false,
        &RabbitError::UninstallAsRoot,
    );
    assert_eq!(
        outcome.status_text,
        "Uninstalling failed.\n\nRABBIT is running as root, as it does under sudo, so it \
         uninstalls nothing. Quit RABBIT and start it as yourself. It asks for an \
         administrator's password by itself when it needs one."
    );
}

/// The refusal to run as root leaves nothing in the user's resource
/// folder: Done saves no report of its own for it, which would be root's,
/// as would any RABBIT or logs folder made for it. Any other refusal
/// before the lock still gets one.
#[test]
fn running_as_root_saves_no_report_into_the_resource_folder() {
    let dir = tempdir().unwrap();
    let (_, model) = langpack_model(PlanActionKind::Install);
    let localizer = en();
    let error = RabbitError::UninstallAsRoot;
    let mut outcome = wizard_uninstall_outcome_from_error(
        &model,
        &localizer,
        dir.path(),
        &ids(&["reaper"]),
        false,
        &error,
    );
    // What Done does with the outcome of a run that ended in `error`.
    if saves_report_after_error(&error) {
        save_wizard_uninstall_outcome(&localizer, &mut outcome).unwrap();
    }
    assert!(!saves_report_after_error(&error));
    assert!(saves_report_after_error(
        &RabbitError::PackageInstallInProgress {
            lock_path: dir.path().join("RABBIT/locks/package-install.lock"),
            pid: 42,
        }
    ));
    assert_eq!(outcome.report_path, None);
    assert!(!dir.path().join("RABBIT").exists());
}

/// REAPER alone needs every REAPER closed on macOS too, when the
/// administrator command moves its app, for a reason of its own; the
/// Windows reason, its uninstaller, never comes up there.
#[test]
fn reaper_alone_on_macos_says_why_every_reaper_must_close() {
    let report = preflight(vec![check(ANY_REAPER_PROCESS_CHECK, PreflightStatus::Fail)]);
    let blocker = uninstall_blocker(
        &en(),
        &mac_plan(vec![root_moved_reaper()]),
        &ids(&["reaper"]),
        &report,
        &[],
    )
    .unwrap();
    assert_eq!(
        blocker.body,
        "Close every copy of REAPER on this computer, including portable ones, then press \
         Uninstall again. The administrator command that moves REAPER can't tell one REAPER from \
         another, so all of them have to be closed."
    );
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let blocker = uninstall_blocker(
            &localizer,
            &mac_plan(vec![root_moved_reaper()]),
            &ids(&["reaper"]),
            &report,
            &[],
        )
        .unwrap();
        assert_reads(locale, &blocker.body);
    }
}

/// Every macOS Done text reads in every language, with no key or
/// placeholder left over.
#[test]
fn macos_done_reads_in_every_locale() {
    let reports = [
        mac_report(
            vec![declined("surge-xt"), declined("reaper")],
            vec![],
            vec![],
        ),
        mac_report(
            vec![
                trashed_reaper(Some(IN_TRASH)),
                surge_removed_as_administrator(),
            ],
            vec![],
            vec![step(
                Some("reaper"),
                AdministratorStepKind::MoveToTrash,
                MAC_REAPER_APP,
                AdministratorStepVerdict::Ran,
            )],
        ),
        mac_report(vec![trashed_reaper(None)], vec![], vec![]),
        mac_report(
            vec![surge_step_failed(
                Some(SURGE_VST3_MAC),
                &[SURGE_APP],
                &[SURGE_VST3_MAC],
                true,
            )],
            vec![],
            vec![],
        ),
        mac_report(
            vec![surge_step_failed(
                None,
                &[SURGE_APP],
                &[SURGE_VST3_MAC],
                false,
            )],
            vec![],
            vec![],
        ),
        mac_report(
            vec![surge_step_failed(Some(SURGE_APP), &[], &[SURGE_APP], false)],
            vec![],
            vec![],
        ),
        // A tree deleted for good, which may be partly gone.
        mac_report(
            vec![surge_step_failed(Some(SURGE_APP), &[], &[SURGE_APP], false)],
            vec![admin_surge()],
            vec![],
        ),
        mac_report(
            vec![surge_step_failed(None, &[], &[SURGE_APP], false)],
            vec![],
            vec![],
        ),
        mac_report(
            vec![report_item(
                "reaper",
                UninstallStatus::Failed,
                UninstallMessage::AdministratorStepFailed {
                    path: None,
                    ended_unclearly: false,
                    gone: Vec::new(),
                    left: vec![PathBuf::from(MAC_REAPER_APP)],
                    refused_by_macos: false,
                    detail: None,
                },
            )],
            vec![],
            vec![],
        ),
        mac_report(
            vec![
                report_item(
                    "reaper",
                    UninstallStatus::Failed,
                    UninstallMessage::TrashFailed {
                        error: "denied".to_string(),
                        refused_by_macos: true,
                    },
                ),
                report_item(
                    "sws",
                    UninstallStatus::Failed,
                    UninstallMessage::TrashUnclear {
                        path: PathBuf::from(MAC_REAPER_APP),
                    },
                ),
                report_item(
                    "surge-xt",
                    UninstallStatus::Blocked,
                    UninstallMessage::ElevationFailed {
                        error: "too many steps".to_string(),
                    },
                ),
            ],
            vec![],
            vec![],
        ),
        mac_report(
            vec![UninstallReportItem {
                problems: vec![
                    UninstallProblem::InstallerRecordsRemain {
                        ids: vec!["org.surge-synth-team.surge-xt.app.pkg".to_string()],
                    },
                    UninstallProblem::InstallerRecordsUnchecked {
                        error: "pkgutil failed".to_string(),
                    },
                    UninstallProblem::AdministratorEndedUnclearly { status: None },
                ],
                ..surge_removed_as_administrator()
            }],
            vec![],
            vec![],
        ),
        mac_report(
            vec![report_item(
                "surge-xt",
                UninstallStatus::Blocked,
                UninstallMessage::ChecksFailed {
                    checks: vec![check(PROGRAMS_RUNNING_CHECK, PreflightStatus::Fail)],
                },
            )],
            vec![],
            vec![step(
                None,
                AdministratorStepKind::ProgramsClosed,
                "Surge XT",
                AdministratorStepVerdict::Failed,
            )],
        ),
        mac_report(
            vec![report_item(
                "reaper",
                UninstallStatus::Blocked,
                UninstallMessage::NotRemovable {
                    removability: Removability::ByHand {
                        path: PathBuf::from(MAC_REAPER_APP),
                        reason: ByHandReason::NotTheExpectedApp,
                    },
                },
            )],
            vec![],
            vec![],
        ),
    ];
    let (_, model) = langpack_model(PlanActionKind::Install);
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for report in &reports {
            let package_ids = vec!["surge-xt".to_string()];
            let outcome = wizard_uninstall_outcome(&localizer, &package_ids, report);
            assert_reads(locale, &outcome.heading);
            assert_reads(locale, &outcome.status_text);
            for line in &outcome.detail_lines {
                assert_reads(locale, line);
            }
        }
        let model = WizardModel {
            text: crate::wizard::text::wizard_text(&localizer),
            ..model.clone()
        };
        let outcome = wizard_uninstall_outcome_from_error(
            &model,
            &localizer,
            Path::new(MAC_RESOURCE),
            &ids(&["reaper"]),
            false,
            &RabbitError::UninstallAsRoot,
        );
        assert_reads(locale, &outcome.status_text);
        assert!(
            outcome.status_text.contains("sudo"),
            "{locale}: {}",
            outcome.status_text
        );
    }
}
