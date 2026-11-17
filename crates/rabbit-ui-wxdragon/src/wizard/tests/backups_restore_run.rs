//! Restoring backups from the Backups tab: what the run is given, what has
//! to be closed before it starts, what the progress page says, what Done
//! and Show details say for every way a restore can end, in every language,
//! and a real restore of sets in a temporary folder.
//!
//! Every test that really restores uses sets whose every file lies in the
//! temporary resource folder, on a made-up computer where nothing runs
//! unless the test says so and no file goes back outside that folder, so no
//! test depends on what runs on the computer that tests it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rabbit_core::RabbitError;
use rabbit_core::backup_overview::{BackupEntry, BackupItem, BackupStatus};
use rabbit_core::cancel::CancelToken;
use rabbit_core::localization::{Localizer, embedded_locales};
use rabbit_core::preflight::{PreflightReport, RunningProcess};
use rabbit_core::progress::{ProgressEvent, ProgressReporter};
use rabbit_core::receipt::load_install_state;
use rabbit_core::report::SavedReportPaths;
use rabbit_core::restore_refusal::{RestoreRefusal, sample_refusals};
use rabbit_core::rollback::{
    BACKUP_MANIFEST_FILE, BackupSetWriter, KeymapOrigin, MoveOutAction, MovedOutFile,
    RestoreBackupAction, RestoreBackupActionKind, RestoreItem, RestoreReportItem,
    RestoreRunOptions, RestoreRunReport, RestoreStatus, RestoredKeymap, RestoredPackage,
    SkippedPackage, SkippedPackageReason, SwappedEntry, SwappedPackage, TestComputer,
    restore_backups_on, restore_running_refusal_on,
};
use tempfile::tempdir;

use super::backups::{
    OCT_2_1100, en, ffmpeg, index_of, keymap, list_in, package, plan_all, reapack, sws, tick,
    version, write_uninstall_set,
};
use super::support::{assert_reads, langpack_model};
use super::uninstall::target;
use crate::wizard::*;

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

const RESOURCE: &str = "C:/REAPER";
const SWS: &str = "SWS Extension 2.14.0.7";
const FFMPEG: &str = "FFmpeg for Improved Video Support 7.1";
const REAPACK: &str = "ReaPack 1.2.4";
const KEYMAP: &str = "Your key map";
/// When a restore of November 10, 2026, 2:05 PM UTC made its backup of
/// what it replaced.
const BEFORE_RESTORE: &str = "restore-unix-1794319500-before-restore";
const BEFORE_RESTORE_DATE: &str = "November 10, 2026, 2:05 PM";
/// A backup of what a restore replaced, under a name RABBIT reads no date
/// from.
const UNDATED_BEFORE_RESTORE: &str = "restore-unix-x-before-restore";

fn utc(_: u64) -> i32 {
    0
}

/// An earlier version of ReaPack whose row names no version.
fn reapack_unversioned() -> BackupEntry {
    BackupEntry {
        version: None,
        replaced_by: None,
        ..reapack()
    }
}

/// REAPER's default keys, from an undated key map set of their own.
fn default_keys_undated() -> BackupEntry {
    BackupEntry {
        item: BackupItem::Keymap {
            set_id: "unattended-unix-x".to_string(),
        },
        keymap_origin: Some(KeymapOrigin::DefaultKeys),
        created_unix: None,
        ..keymap()
    }
}

/// A key map of `origin`, made when [`keymap`] was, or with no date RABBIT
/// can tell.
fn keymap_of(origin: Option<KeymapOrigin>, dated: bool) -> BackupEntry {
    let entry = BackupEntry {
        keymap_origin: origin,
        ..keymap()
    };
    if dated {
        return entry;
    }
    BackupEntry {
        item: BackupItem::Keymap {
            set_id: "unattended-unix-y".to_string(),
        },
        created_unix: None,
        ..entry
    }
}

/// An earlier OSARA key map from October 2, which Done calls "the key map".
fn osara_keymap() -> BackupEntry {
    BackupEntry {
        item: BackupItem::Keymap {
            set_id: "unattended-unix-1790938800-000000001".to_string(),
        },
        keymap_origin: Some(KeymapOrigin::UnchangedOsara),
        created_unix: Some(OCT_2_1100),
        ..keymap()
    }
}

/// A list of `entries` in `localizer`'s language with every backup that
/// can be ticked to restore ticked, as the rows tick.
fn all_ticked(localizer: &Localizer, entries: Vec<BackupEntry>) -> WizardBackupList {
    let mut list = list_in(localizer, entries);
    let rows: Vec<usize> = (0..list.rows.len())
        .filter(|&index| list.rows[index].can_tick(BackupChoice::Restore))
        .collect();
    for index in rows {
        apply_backup_checkbox(&mut list.rows, index, BackupChoice::Restore, true);
    }
    list
}

/// What a restore of what is ticked to restore in `list` takes, in the
/// REAPER at `C:/REAPER`.
fn run_of(localizer: &Localizer, list: &WizardBackupList) -> BackupRestoreRun {
    backup_restore_run(localizer, Path::new(RESOURCE), list)
}

/// SWS, ReaPack and your key map, all ticked to restore, in English.
fn three() -> BackupRestoreRun {
    run_of(&en(), &all_ticked(&en(), vec![sws(), reapack(), keymap()]))
}

/// The one backup named `name`, ticked to restore, in English.
fn one(entry: BackupEntry) -> BackupRestoreRun {
    run_of(&en(), &all_ticked(&en(), vec![entry]))
}

/// The report of `item` that came out as `status`, with nothing else said.
fn item(restore: &RestoreItem, status: RestoreStatus) -> RestoreReportItem {
    RestoreReportItem {
        backup_id: restore.set_id.clone(),
        package_id: restore.package_id().map(ToString::to_string),
        keymap_only: restore.package_id().is_none(),
        status,
        backup_path: None,
        actions: Vec::new(),
        skipped_targets: Vec::new(),
        packages: Vec::new(),
        skipped_packages: Vec::new(),
        before_restore_set_id: None,
        swap: None,
        swaps: Vec::new(),
        keymap: None,
        refusal: None,
        error: None,
    }
}

/// A file the restore of a backup worked on, and what came of it.
fn action(target: &str, kind: RestoreBackupActionKind, copy: Option<&str>) -> RestoreBackupAction {
    RestoreBackupAction {
        package_id: None,
        source_path: PathBuf::from(format!("C:/REAPER/RABBIT/backups/x/{target}")),
        target_path: PathBuf::from(format!("{RESOURCE}/{target}")),
        current_backup_path: copy.map(PathBuf::from),
        action: kind,
        size: 3,
        sha256: "0".repeat(64),
        error: None,
    }
}

/// SWS restored: its plug-in came back.
fn sws_restored(restore: &RestoreItem) -> RestoreReportItem {
    RestoreReportItem {
        actions: vec![action(
            "UserPlugins/reaper_sws-x64.dll",
            RestoreBackupActionKind::Restored,
            None,
        )],
        ..item(restore, RestoreStatus::Restored)
    }
}

/// A file of ReaPack 1.2.5 that going back to 1.2.4 moves into the new
/// backup, and what came of it.
fn moved(name: &str, action: MoveOutAction) -> MovedOutFile {
    MovedOutFile {
        path: PathBuf::from(format!("{RESOURCE}/UserPlugins/{name}")),
        backup_path: Some(PathBuf::from(format!("C:/REAPER/RABBIT/backups/b/{name}"))),
        size: 3,
        sha256: "0".repeat(64),
        action,
        error: None,
    }
}

/// What going back to ReaPack 1.2.4 from 1.2.5 did to its records. Going
/// back to a copy RABBIT didn't install, which removes its entry, also
/// leaves a file of 1.2.5 where it is, with a copy in the new backup.
fn reapack_swap(entry: SwappedEntry) -> SwappedPackage {
    let removed = entry == SwappedEntry::Removed;
    SwappedPackage {
        package_id: "reapack".to_string(),
        version: Some(version("1.2.4")),
        replaced_version: Some(version("1.2.5")),
        variant: None,
        replaced_variant: None,
        entry,
        uninstall_needs_reinstall: removed,
        undeclined: false,
        moved_out: vec![moved("reapack_new.dll", MoveOutAction::MovedOut)],
        changed_left: vec![PathBuf::from("C:/REAPER/UserPlugins/reapack_changed.dll")],
        left_in_place: if removed {
            vec![PathBuf::from("C:/REAPER/UserPlugins/reapack_kept.dll")]
        } else {
            Vec::new()
        },
        langpack: None,
        langpack_before: None,
        langpack_restored: false,
        langpack_error: None,
        langpack_reaper_may_run: false,
    }
}

/// ReaPack back to 1.2.4.
fn reapack_restored(restore: &RestoreItem, entry: SwappedEntry) -> RestoreReportItem {
    RestoreReportItem {
        actions: vec![action(
            "UserPlugins/reaper_reapack-x64.dll",
            RestoreBackupActionKind::Restored,
            Some("C:/REAPER/RABBIT/backups/b/reaper_reapack-x64.dll"),
        )],
        before_restore_set_id: Some(BEFORE_RESTORE.to_string()),
        swap: Some(reapack_swap(entry)),
        ..item(restore, RestoreStatus::Restored)
    }
}

/// Going back to ReaPack 1.2.4 that failed partway: its plug-in couldn't
/// be put back as it was, and a file of 1.2.5 it had moved out couldn't go
/// back to its place. The backup of what it replaced has a name RABBIT
/// reads no date from.
fn reapack_files_left(restore: &RestoreItem) -> RestoreReportItem {
    let mut swap = reapack_swap(SwappedEntry::Replaced);
    swap.moved_out = vec![
        moved("reapack_new.dll", MoveOutAction::MovedBack),
        moved("reapack_stuck.dll", MoveOutAction::StillInBackup),
    ];
    RestoreReportItem {
        actions: vec![action(
            "UserPlugins/reaper_reapack-x64.dll",
            RestoreBackupActionKind::Left,
            Some("C:/REAPER/RABBIT/backups/b/reaper_reapack-x64.dll"),
        )],
        before_restore_set_id: Some(UNDATED_BEFORE_RESTORE.to_string()),
        swap: Some(swap),
        error: Some(
            "C:/REAPER/UserPlugins/reapack_stuck.dll: Access is denied. (os error 5)".to_string(),
        ),
        ..item(restore, RestoreStatus::FilesLeft)
    }
}

/// Your key map back, keeping two scripts registered since.
fn keymap_restored(restore: &RestoreItem) -> RestoreReportItem {
    RestoreReportItem {
        actions: vec![action(
            "reaper-kb.ini",
            RestoreBackupActionKind::Restored,
            Some("C:/REAPER/RABBIT/backups/b/reaper-kb.ini"),
        )],
        before_restore_set_id: Some(BEFORE_RESTORE.to_string()),
        keymap: Some(RestoredKeymap {
            path: PathBuf::from("C:/REAPER/reaper-kb.ini"),
            replaced: true,
            scripts_added: Some(2),
        }),
        ..item(restore, RestoreStatus::Restored)
    }
}

/// A restore that failed partway, with one file that couldn't be put back
/// as it was and one that is gone, and what they replaced in a set of its
/// own. One file's copy in the backup never finished, so it wasn't
/// touched.
fn files_left(restore: &RestoreItem) -> RestoreReportItem {
    RestoreReportItem {
        actions: vec![
            action(
                "UserPlugins/reaper_sws-x64.dll",
                RestoreBackupActionKind::Left,
                Some("C:/REAPER/RABBIT/backups/b/reaper_sws-x64.dll"),
            ),
            action(
                "UserPlugins/sws_python64.dll",
                RestoreBackupActionKind::Missing,
                Some("C:/REAPER/RABBIT/backups/b/sws_python64.dll"),
            ),
            action(
                "UserPlugins/sws_other.dll",
                RestoreBackupActionKind::PutBack,
                None,
            ),
            action(
                "UserPlugins/sws_last.dll",
                RestoreBackupActionKind::NotRestored,
                None,
            ),
        ],
        skipped_targets: vec![PathBuf::from("C:/REAPER/UserPlugins/sws_unfinished.dll")],
        before_restore_set_id: Some(BEFORE_RESTORE.to_string()),
        error: Some(
            "C:/REAPER/UserPlugins/sws_python64.dll: Access is denied. (os error 5)".to_string(),
        ),
        ..item(restore, RestoreStatus::FilesLeft)
    }
}

/// A restore that failed partway with a file it had added, where there was
/// none, that it couldn't take out again. It had replaced nothing, so it
/// made no backup of what it replaced.
fn files_left_no_backup(restore: &RestoreItem) -> RestoreReportItem {
    RestoreReportItem {
        actions: vec![
            action(
                "UserPlugins/reaper_sws-x64.dll",
                RestoreBackupActionKind::Left,
                None,
            ),
            action(
                "UserPlugins/sws_python64.dll",
                RestoreBackupActionKind::NotRestored,
                None,
            ),
        ],
        error: Some(
            "C:/REAPER/UserPlugins/reaper_sws-x64.dll: Access is denied. (os error 5)".to_string(),
        ),
        ..item(restore, RestoreStatus::FilesLeft)
    }
}

/// A restore that failed before it changed anything.
fn failed(restore: &RestoreItem) -> RestoreReportItem {
    RestoreReportItem {
        error: Some("C:/REAPER/RABBIT/backups/x/manifest.json: Access is denied.".to_string()),
        ..item(restore, RestoreStatus::NotRestored)
    }
}

/// A backup a check refused for `refusal`.
fn refused(restore: &RestoreItem, refusal: RestoreRefusal) -> RestoreReportItem {
    RestoreReportItem {
        refusal: Some(refusal),
        ..item(restore, RestoreStatus::NotRestored)
    }
}

/// The refusal for SWS being installed again since its backup was made.
fn reinstalled(restore: &RestoreItem) -> RestoreRefusal {
    RestoreRefusal::ReinstalledSince {
        backup_id: restore.set_id.clone(),
        package_id: "sws".to_string(),
        other_packages: Vec::new(),
    }
}

/// A real run's report of `backups`, saved where a real run saves it.
fn report(backups: Vec<RestoreReportItem>) -> RestoreRunReport {
    RestoreRunReport {
        resource_path: PathBuf::from(RESOURCE),
        dry_run: false,
        quick: false,
        preflight: PreflightReport {
            passed: true,
            checks: Vec::new(),
        },
        backups,
        stopped: false,
        saved_report: Some(SavedReportPaths {
            json_path: PathBuf::from("C:/REAPER/RABBIT/logs/restore-backup-unix-1.json"),
            text_path: PathBuf::from("C:/REAPER/RABBIT/logs/restore-backup-unix-1.txt"),
        }),
    }
}

/// A run the user stopped.
fn stopped(backups: Vec<RestoreReportItem>) -> RestoreRunReport {
    RestoreRunReport {
        stopped: true,
        ..report(backups)
    }
}

/// The Done page for `result` of `run`, in English, with dates in UTC.
fn done(
    run: &BackupRestoreRun,
    result: rabbit_core::Result<RestoreRunReport>,
) -> WizardBackupRestoreOutcome {
    wizard_backup_restore_outcome_at(&en(), run, &result, &utc)
}

const REPORT_LINE: &str = "Report: C:/REAPER/RABBIT/logs/restore-backup-unix-1.txt";

/// One refusal for the code `code`, from the core's samples.
fn sample(code: &str) -> RestoreRefusal {
    sample_refusals()
        .into_iter()
        .find(|refusal| refusal.code() == code)
        .unwrap_or_else(|| panic!("no sample refusal {code}"))
}

// ---------------------------------------------------------------------------
// What the run is given.
// ---------------------------------------------------------------------------

#[test]
fn the_run_is_copied_out_of_the_list_with_the_listed_fingerprints() {
    let mut list = list_in(&en(), vec![sws(), ffmpeg(), reapack(), keymap()]);
    tick(&mut list, SWS, BackupChoice::Restore);
    tick(&mut list, KEYMAP, BackupChoice::Restore);
    tick(&mut list, REAPACK, BackupChoice::Restore);
    // A tick to delete is no part of a restore.
    tick(&mut list, FFMPEG, BackupChoice::Delete);

    let run = run_of(&en(), &list);
    assert_eq!(run.resource_path, PathBuf::from(RESOURCE));
    // The items Review's quick plan checked, in list order, each with the
    // fingerprint the list read.
    assert_eq!(run.items, backup_restore_items(&list));
    assert!(run.items.iter().all(|item| item.fingerprint.is_some()));
    // Counted as Review counts them: one per ticked row.
    assert_eq!(run.backups(), 3);
    assert_eq!(
        run.backups(),
        list.ticked_rows(BackupChoice::Restore).count()
    );
    let names: Vec<&str> = run.rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, [SWS, REAPACK, KEYMAP]);
    // No other ticked row has their names, so they are said without dates.
    for row in &run.rows {
        assert_eq!(row.label, row.name);
    }
    // Inside a sentence a package is named so too, and a key map in lower
    // case, with its date.
    assert_eq!(run.rows[0].in_sentence, SWS);
    assert_eq!(run.rows[1].in_sentence, REAPACK);
    assert_eq!(
        run.rows[2].in_sentence,
        "your key map from November 1, 2026, 6:40 PM"
    );
    assert_eq!(run.rows[0].kind, RestoredBackupKind::Uninstalled);
    assert_eq!(run.rows[0].package.as_deref(), Some("SWS Extension"));
    assert_eq!(
        run.rows[1].kind,
        RestoredBackupKind::Earlier {
            version: Some(version("1.2.4"))
        }
    );
    assert_eq!(
        run.rows[2].kind,
        RestoredBackupKind::Keymap { origin: "own" }
    );
    assert_eq!(
        run.rows[2].date.as_deref(),
        Some("November 1, 2026, 6:40 PM")
    );
}

#[test]
fn nothing_ticked_to_restore_gives_a_run_of_nothing() {
    let mut list = list_in(&en(), vec![sws(), reapack()]);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let run = run_of(&en(), &list);
    assert!(run.items.is_empty());
    assert!(run.rows.is_empty());
    assert_eq!(run.backups(), 0);
}

#[test]
fn two_ticked_rows_of_one_name_are_named_with_their_dates() {
    let older = BackupEntry {
        item: package("uninstall-unix-1790938800", "sws"),
        created_unix: Some(OCT_2_1100),
        ..sws()
    };
    let run = run_of(&en(), &all_ticked(&en(), vec![sws(), older]));
    let labels: Vec<&str> = run.rows.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "SWS Extension 2.14.0.7 (Uninstalled November 9, 2026, 2:05 PM)",
            "SWS Extension 2.14.0.7 (Uninstalled October 2, 2026, 11:00 AM)",
        ]
    );
    assert!(run.rows.iter().all(|row| row.in_sentence == row.label));
}

#[test]
fn the_progress_log_starts_with_the_target_and_what_comes_back() {
    assert_eq!(
        backup_restore_progress_details_for_start(&en(), &three()),
        "Starting to restore backups.\n\
         Target: C:/REAPER\n\
         SWS Extension 2.14.0.7: Will restore\n\
         ReaPack 1.2.4: Will restore\n\
         Your key map: Will restore"
    );
}

#[test]
fn the_restore_runs_for_real_and_never_while_reaper_runs() {
    let options = backup_restore_run_options();
    assert!(!options.dry_run);
    assert!(!options.quick);
    assert!(!options.allow_reaper_running);
}

// ---------------------------------------------------------------------------
// Before the run: what has to be closed.
// ---------------------------------------------------------------------------

/// A REAPER running from the portable REAPER at `resource`, the target of
/// a restore there, with its app made in the resource folder, as the core's
/// tests make it: its `reaper.exe` on Windows, its app on macOS.
fn targets_reaper(resource: &Path) -> RunningProcess {
    #[cfg(windows)]
    let (name, app) = ("reaper.exe", resource.join("reaper.exe"));
    #[cfg(not(windows))]
    let (name, app) = (
        "REAPER",
        resource
            .join("REAPER.app")
            .join("Contents")
            .join("MacOS")
            .join("REAPER"),
    );
    fs::create_dir_all(app.parent().unwrap()).unwrap();
    fs::write(&app, b"reaper").unwrap();
    RunningProcess {
        pid: "100".to_string(),
        name: name.to_string(),
        executable_path: Some(app),
    }
}

/// Another REAPER, a portable one elsewhere in `dir`.
#[cfg(any(windows, target_os = "macos"))]
fn reaper_elsewhere(dir: &Path) -> RunningProcess {
    RunningProcess {
        pid: "200".to_string(),
        name: "reaper.exe".to_string(),
        executable_path: Some(dir.join("Elsewhere").join("reaper.exe")),
    }
}

/// A program SWS's uninstall plan is made to name, running.
#[cfg(any(windows, target_os = "macos"))]
fn demo_tool() -> RunningProcess {
    RunningProcess {
        pid: "300".to_string(),
        name: "demo tool.exe".to_string(),
        executable_path: None,
    }
}

/// SWS as this platform's package manifests describe it, changed by
/// `change`, as the only package the made-up computer knows.
#[cfg(any(windows, target_os = "macos"))]
fn sws_spec(
    change: impl FnOnce(&mut rabbit_core::package::PackageSpec),
) -> Vec<rabbit_core::package::PackageSpec> {
    let platform = rabbit_core::model::Platform::current().unwrap();
    let mut spec = rabbit_core::package::builtin_package_specs(platform)
        .into_iter()
        .find(|spec| spec.id == "sws")
        .unwrap();
    change(&mut spec);
    vec![spec]
}

/// Every file under `root` with its contents, for "nothing changed".
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path.clone());
                files.push((path, Vec::new()));
            } else {
                let contents = fs::read(&path).unwrap();
                files.push((path, contents));
            }
        }
    }
    files.sort();
    files
}

/// The check made when Restore is pressed decides from what runs, by the
/// rules the run follows, and refuses exactly what the run would refuse:
/// the target's REAPER, any REAPER for a package that needs every REAPER
/// closed, a program the package's uninstall plan names, and nothing when
/// nothing in the way runs. Real sets in a temporary folder; the processes
/// are made up, so the test never depends on what runs here. Neither the
/// check nor the refused run changes anything.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn the_check_before_a_restore_decides_from_what_runs_as_the_run_does() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    write_uninstall_set(&resource);
    let mut list = backup_list_at(&en(), &resource);
    tick(&mut list, SWS, BackupChoice::Restore);
    let run = backup_restore_run(&en(), &resource, &list);
    assert_eq!(run.items.len(), 1);
    let target = targets_reaper(&resource);
    let elsewhere = reaper_elsewhere(dir.path());
    let machine_wide = sws_spec(|spec| spec.requires_standard_install = true);
    let naming_a_program = sws_spec(|spec| {
        for plan in &mut spec.uninstall.plans {
            plan.blocking_programs = vec!["Demo Tool.exe".to_string()];
        }
    });

    let cases = [
        (
            "the target's REAPER",
            TestComputer {
                reapers: vec![target.clone()],
                ..TestComputer::default()
            },
            true,
        ),
        (
            "another REAPER, SWS serving every REAPER",
            TestComputer {
                reapers: vec![elsewhere.clone()],
                package_specs: Some(machine_wide.clone()),
                ..TestComputer::default()
            },
            true,
        ),
        (
            "another REAPER, SWS as shipped",
            TestComputer {
                reapers: vec![elsewhere.clone()],
                ..TestComputer::default()
            },
            false,
        ),
        (
            "a program SWS's uninstall names",
            TestComputer {
                programs: vec![demo_tool()],
                package_specs: Some(naming_a_program.clone()),
                ..TestComputer::default()
            },
            true,
        ),
        (
            "a program nothing names",
            TestComputer {
                programs: vec![demo_tool()],
                ..TestComputer::default()
            },
            false,
        ),
        ("nothing", TestComputer::default(), false),
    ];
    for (name, computer, refuses) in cases {
        let before = snapshot(dir.path());
        let refusal = running_refusal(&run, &computer);
        assert_eq!(refusal.is_some(), refuses, "{name}: {refusal:?}");
        assert_eq!(
            snapshot(dir.path()),
            before,
            "{name}: the check changed something"
        );
        let Some(refusal) = refusal else {
            continue;
        };
        // Each refusal it gives has its warning.
        assert!(
            restore_running_warning(&en(), &refusal).is_some(),
            "{name}: {refusal:?}"
        );
        // The run, on the same computer, refuses the very same, and changes
        // nothing.
        let error = restore_backups_on(
            &run.resource_path,
            &run.items,
            &backup_restore_run_options(),
            &computer,
            &ProgressReporter::noop(),
            &CancelToken::new(),
        )
        .unwrap_err();
        assert!(
            matches!(&error, RabbitError::RestoreRefused(run_refusal) if *run_refusal == refusal),
            "{name}: {error:?}"
        );
        assert_eq!(
            snapshot(dir.path()),
            before,
            "{name}: the refused run changed something"
        );
    }

    // What each one names.
    let check = |computer: &TestComputer| running_refusal(&run, computer).unwrap();
    assert!(matches!(
        check(&TestComputer {
            reapers: vec![target.clone()],
            ..TestComputer::default()
        }),
        RestoreRefusal::ReaperRunning { every_reaper_for, processes, .. }
            if every_reaper_for.is_empty() && processes == [target.clone()]
    ));
    assert!(matches!(
        check(&TestComputer {
            reapers: vec![elsewhere.clone()],
            package_specs: Some(machine_wide),
            ..TestComputer::default()
        }),
        RestoreRefusal::ReaperRunning { every_reaper_for, .. } if every_reaper_for == ["sws"]
    ));
    assert!(matches!(
        check(&TestComputer {
            programs: vec![demo_tool()],
            package_specs: Some(naming_a_program),
            ..TestComputer::default()
        }),
        RestoreRefusal::ProgramRunning { programs, .. } if programs == ["demo tool.exe"]
    ));
}

/// A key map always needs the target's REAPER closed.
#[test]
fn a_key_map_restore_needs_the_targets_reaper_closed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    write_keymap_set(&resource);
    let mut list = backup_list_at(&en(), &resource);
    tick(&mut list, KEYMAP, BackupChoice::Restore);
    let run = backup_restore_run(&en(), &resource, &list);
    assert_eq!(run.items.len(), 1);
    let computer = TestComputer {
        reapers: vec![targets_reaper(&resource)],
        ..TestComputer::default()
    };
    assert!(matches!(
        running_refusal(&run, &computer),
        Some(RestoreRefusal::ReaperRunning {
            puts_keymap_back: true,
            ..
        })
    ));
    assert_eq!(running_refusal(&run, &TestComputer::default()), None);
}

/// [`backup_restore_running_refusal`] as the Restore button asks it, on
/// `computer`: the wizard's own function hands the check the run's folder
/// and backups.
fn running_refusal(run: &BackupRestoreRun, computer: &TestComputer) -> Option<RestoreRefusal> {
    backup_restore_running_refusal_with(run, &|path, items| {
        assert_eq!(path, run.resource_path.as_path());
        assert_eq!(items, run.items.as_slice());
        restore_running_refusal_on(path, items, computer)
    })
}

#[test]
fn the_warning_says_what_to_close_then_press_restore_again() {
    let reaper = sample("reaper-running");
    let RestoreRefusal::ReaperRunning {
        package_names,
        processes,
        ..
    } = reaper.clone()
    else {
        panic!("{reaper:?}");
    };
    let target_only = RestoreRefusal::ReaperRunning {
        every_reaper_for: Vec::new(),
        package_names: Vec::new(),
        processes: processes.clone(),
        puts_keymap_back: false,
    };
    assert_eq!(
        restore_running_warning(&en(), &target_only),
        Some((
            "REAPER is still open".to_string(),
            "Close REAPER, then press Restore again. RABBIT can't restore backups into REAPER \
             while it is running."
                .to_string()
        ))
    );
    let every = RestoreRefusal::ReaperRunning {
        every_reaper_for: vec!["ffmpeg".to_string(), "sws".to_string()],
        package_names,
        processes,
        puts_keymap_back: false,
    };
    assert_eq!(
        restore_running_warning(&en(), &every),
        Some((
            "REAPER is still open".to_string(),
            "Close every copy of REAPER on this computer, including portable ones, then press \
             Restore again. Every REAPER on this computer can use FFmpeg for Improved Video \
             Support and SWS Extension."
                .to_string()
        ))
    );
    let programs = RestoreRefusal::ProgramRunning {
        package_ids: vec!["surge-xt".to_string()],
        package_names: vec!["Surge XT".to_string()],
        programs: vec!["Surge XT.exe".to_string(), "surge-xt-cli.exe".to_string()],
    };
    assert_eq!(
        restore_running_warning(&en(), &programs),
        Some((
            "Close these programs first".to_string(),
            "Close Surge XT.exe and surge-xt-cli.exe, then press Restore again.".to_string()
        ))
    );
    // A refusal about anything but what runs is the run's to say.
    assert_eq!(restore_running_warning(&en(), &sample("lock-held")), None);
}

// ---------------------------------------------------------------------------
// The progress page.
// ---------------------------------------------------------------------------

fn started(restore: &RestoreItem) -> ProgressEvent {
    ProgressEvent::RestoreStarted {
        set_id: restore.set_id.clone(),
        package_id: restore.package_id().map(ToString::to_string),
    }
}

fn completed(restore: &RestoreItem, status: RestoreStatus) -> ProgressEvent {
    ProgressEvent::RestoreCompleted {
        set_id: restore.set_id.clone(),
        package_id: restore.package_id().map(ToString::to_string),
        status,
    }
}

/// The progress page's state at the start of the restore `run`, in
/// `localizer`'s language.
fn progress_of(localizer: &Localizer, run: &BackupRestoreRun) -> ProgressUiState {
    ProgressUiState::new(WizardMode::Restore, run.items.len(), 0)
        .with_backup_restore(backup_restore_progress(localizer, run))
}

/// Count `event` in, as the progress page does, and give its lines.
fn hear(state: &mut ProgressUiState, event: &ProgressEvent) -> (Option<String>, Option<String>) {
    state.record(event);
    backup_restore_progress_lines(&en(), state, event)
}

#[test]
fn a_restore_says_each_backup_it_starts_and_logs_what_came_of_it() {
    let run = three();
    let mut state = progress_of(&en(), &run);
    let expected = [
        (
            "Restoring SWS Extension 2.14.0.7…",
            RestoreStatus::Restored,
            "SWS Extension 2.14.0.7: restored.",
            33,
        ),
        (
            "Going back to ReaPack 1.2.4…",
            RestoreStatus::NotRestored,
            "ReaPack 1.2.4: not restored, left as it was.",
            67,
        ),
        (
            "Restoring your key map from November 1, 2026, 6:40 PM…",
            RestoreStatus::FilesLeft,
            "Your key map: not restored, and some files it had changed couldn't be put back as \
             they were.",
            100,
        ),
    ];
    for (restore, (start, status, end, percentage)) in run.items.iter().zip(expected) {
        let event = started(restore);
        assert!(is_spoken(&event));
        let (status_line, log) = hear(&mut state, &event);
        assert_eq!(status_line.as_deref(), Some(start));
        assert_eq!(log.as_deref(), Some(start));
        assert_eq!(
            state.current_package.as_deref(),
            Some(restore.set_id.as_str())
        );
        assert!(!state.moves_focus_to_log(&event));

        let event = completed(restore, status);
        assert!(!is_spoken(&event));
        assert_eq!(hear(&mut state, &event), (None, Some(end.to_string())));
        assert_eq!(state.percentage(), percentage);
        assert!(state.current_package.is_none());
        assert!(!state.moves_focus_to_log(&event));
    }
}

#[test]
fn every_kind_of_backup_has_its_start_line_and_every_end_its_log_line() {
    let starts = [
        (
            reapack_unversioned(),
            "Going back to the copy of ReaPack that an update replaced…",
        ),
        (default_keys_undated(), "Restoring REAPER's default keys…"),
        (
            osara_keymap(),
            "Restoring the key map from October 2, 2026, 11:00 AM…",
        ),
        (
            keymap_of(Some(KeymapOrigin::Unknown), false),
            "Restoring the key map…",
        ),
    ];
    for (entry, line) in starts {
        let run = one(entry);
        let mut state = progress_of(&en(), &run);
        assert_eq!(
            hear(&mut state, &started(&run.items[0])).0.as_deref(),
            Some(line)
        );
    }
    let run = one(sws());
    let restore = &run.items[0];
    let mut state = progress_of(&en(), &run);
    for (status, line) in [
        (
            RestoreStatus::Restored,
            Some("SWS Extension 2.14.0.7: restored."),
        ),
        (
            RestoreStatus::PartlyRestored,
            Some("SWS Extension 2.14.0.7: only partly restored."),
        ),
        (RestoreStatus::WouldRestore, None),
        (RestoreStatus::NotReached, None),
    ] {
        assert_eq!(
            hear(&mut state, &completed(restore, status)).1.as_deref(),
            line,
            "{status:?}"
        );
    }
}

#[test]
fn a_key_map_and_a_package_of_one_set_are_told_apart_by_their_part() {
    // ReaPack's update set also holds a key map: two backups of one set.
    let keymap_of_reapacks_set = BackupEntry {
        item: BackupItem::Keymap {
            set_id: "unix-1793697120".to_string(),
        },
        ..keymap()
    };
    let run = run_of(
        &en(),
        &all_ticked(&en(), vec![reapack(), keymap_of_reapacks_set]),
    );
    assert_eq!(run.items[0].set_id, run.items[1].set_id);
    let mut state = progress_of(&en(), &run);
    assert_eq!(
        hear(&mut state, &started(&run.items[1])).0.as_deref(),
        Some("Restoring your key map from November 1, 2026, 6:40 PM…")
    );
    assert_eq!(
        hear(&mut state, &started(&run.items[0])).0.as_deref(),
        Some("Going back to ReaPack 1.2.4…")
    );
}

#[test]
fn only_a_restores_events_have_restore_lines() {
    let run = three();
    let state = progress_of(&en(), &run);
    for event in [
        ProgressEvent::InstallStarted {
            package_id: "reapack".to_string(),
        },
        ProgressEvent::DeleteStarted {
            set_id: run.items[0].set_id.clone(),
        },
    ] {
        assert_eq!(
            backup_restore_progress_lines(&en(), &state, &event),
            (None, None),
            "{event:?}"
        );
    }
    // A delete's state knows nothing of a restore: its events have no
    // restore lines but the set's name.
    let empty = ProgressUiState::new(WizardMode::DeleteBackups, 1, 0);
    assert_eq!(
        backup_restore_progress_lines(&en(), &empty, &started(&run.items[0])).0,
        Some(format!("Restoring {}…", run.items[0].set_id))
    );
}

// ---------------------------------------------------------------------------
// Done and Show details, in English.
// ---------------------------------------------------------------------------

#[test]
fn one_package_restored_names_it_and_says_what_came_back() {
    let run = one(sws());
    let mut backup = sws_restored(&run.items[0]);
    backup.packages = vec![RestoredPackage {
        package_id: "sws".to_string(),
        receipt_merged: true,
        receipt_already_current: false,
        langpack_before: Some("de_DE.ReaperLangPack".to_string()),
        langpack_restored: true,
        langpack_error: None,
    }];
    let outcome = done(&run, Ok(report(vec![backup])));
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Restored);
    assert_eq!(outcome.heading, "RABBIT restored SWS Extension 2.14.0.7.");
    assert_eq!(
        outcome.status_text,
        "RABBIT restored SWS Extension 2.14.0.7.\n\nREAPER's language is back to \
         de_DE.ReaperLangPack."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER",
            "SWS Extension 2.14.0.7: Restored",
            "  Restored: C:/REAPER/UserPlugins/reaper_sws-x64.dll",
            "  REAPER's language is back to de_DE.ReaperLangPack.",
            REPORT_LINE,
        ]
    );
    assert_eq!(
        outcome.report_path,
        Some(PathBuf::from(
            "C:/REAPER/RABBIT/logs/restore-backup-unix-1.txt"
        ))
    );
}

#[test]
fn an_earlier_version_restored_says_which_version_is_back() {
    let run = one(reapack());
    let outcome = done(
        &run,
        Ok(report(vec![reapack_restored(
            &run.items[0],
            SwappedEntry::Replaced,
        )])),
    );
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Restored);
    assert_eq!(outcome.status_text, "ReaPack is back to 1.2.4.");
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER".to_string(),
            "ReaPack 1.2.4: Restored".to_string(),
            "  Restored: C:/REAPER/UserPlugins/reaper_reapack-x64.dll".to_string(),
            "  Moved into the new backup, because the earlier version doesn't have it: \
             C:/REAPER/UserPlugins/reapack_new.dll"
                .to_string(),
            "  Stays where it is, because it changed since RABBIT installed it: \
             C:/REAPER/UserPlugins/reapack_changed.dll"
                .to_string(),
            format!(
                "  What it replaced is kept as a backup on the Backups tab: ReaPack 1.2.5, from \
                 {BEFORE_RESTORE_DATE}."
            ),
            REPORT_LINE.to_string(),
        ]
    );

    // Going back to a copy RABBIT didn't install: RABBIT no longer counts
    // it as installed by it, and says how to uninstall it later.
    let outcome = done(
        &run,
        Ok(report(vec![reapack_restored(
            &run.items[0],
            SwappedEntry::Removed,
        )])),
    );
    assert_eq!(
        outcome.status_text,
        "ReaPack is back to 1.2.4.\n\nRABBIT no longer counts ReaPack as installed by it.\nTo \
         uninstall ReaPack with RABBIT later, install it again first."
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"  RABBIT no longer counts ReaPack as installed by it.".to_string())
    );
    assert!(
        outcome.detail_lines.contains(
            &"  Stays where it is, with a copy in the new backup: \
              C:/REAPER/UserPlugins/reapack_kept.dll"
                .to_string()
        ),
        "{:#?}",
        outcome.detail_lines
    );

    // Without a version, the copy an update replaced.
    let run = one(reapack_unversioned());
    let mut backup = reapack_restored(&run.items[0], SwappedEntry::Replaced);
    backup.swap.as_mut().unwrap().version = None;
    assert_eq!(
        done(&run, Ok(report(vec![backup]))).heading,
        "ReaPack is back to the copy that an update replaced."
    );
}

#[test]
fn a_key_map_restored_says_it_is_back_and_what_it_kept() {
    let run = one(keymap());
    let outcome = done(&run, Ok(report(vec![keymap_restored(&run.items[0])])));
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Restored);
    assert_eq!(
        outcome.status_text,
        "Your key map from November 1, 2026, 6:40 PM is back."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER".to_string(),
            "Your key map: Restored".to_string(),
            "  Restored: C:/REAPER/reaper-kb.ini".to_string(),
            "  RABBIT kept 2 scripts registered since then in REAPER's Actions list.".to_string(),
            format!(
                "  What it replaced is kept as a backup on the Backups tab: Key map, from \
                 {BEFORE_RESTORE_DATE}."
            ),
            REPORT_LINE.to_string(),
        ]
    );
    let run = one(default_keys_undated());
    assert_eq!(
        done(&run, Ok(report(vec![keymap_restored(&run.items[0])]))).heading,
        "REAPER's default keys are back."
    );
    let run = one(osara_keymap());
    assert_eq!(
        done(&run, Ok(report(vec![keymap_restored(&run.items[0])]))).heading,
        "The key map from October 2, 2026, 11:00 AM is back."
    );
}

/// Done names a key map inside a sentence in lower case, with its date, in
/// every sentence that names a backup that didn't come back.
#[test]
fn a_key_map_that_didnt_come_back_is_named_in_lower_case() {
    let run = one(keymap());
    let restore = &run.items[0];
    let changed = sample("changed-since-listed");
    assert_eq!(
        done(&run, Ok(report(vec![refused(restore, changed.clone())]))).heading,
        "RABBIT couldn't restore your key map from November 1, 2026, 6:40 PM, so it left it as \
         it was."
    );
    assert_eq!(
        done(&run, Ok(report(vec![files_left(restore)]))).heading,
        "RABBIT couldn't restore your key map from November 1, 2026, 6:40 PM."
    );
    let run = one(default_keys_undated());
    assert_eq!(
        done(&run, Ok(report(vec![failed(&run.items[0])]))).heading,
        "RABBIT couldn't restore REAPER's default keys, so it left it as it was."
    );
    let run = one(osara_keymap());
    assert_eq!(
        done(&run, Ok(report(vec![failed(&run.items[0])]))).heading,
        "RABBIT couldn't restore the key map from October 2, 2026, 11:00 AM, so it left it as it \
         was."
    );

    // Among others: the sentences on why it didn't come back.
    let run = run_of(&en(), &all_ticked(&en(), vec![sws(), keymap()]));
    let with = |backup: RestoreReportItem| {
        done(&run, Ok(report(vec![sws_restored(&run.items[0]), backup]))).status_text
    };
    assert_eq!(
        with(refused(&run.items[1], changed)),
        "RABBIT restored 1 of 2 backups.\n\nRABBIT didn't restore your key map from November 1, \
         2026, 6:40 PM. It changed since RABBIT listed it. Start RABBIT again to see the backups \
         as they are now."
    );
    assert_eq!(
        with(failed(&run.items[1])),
        "RABBIT restored 1 of 2 backups.\n\nSomething went wrong with your key map from November \
         1, 2026, 6:40 PM, so RABBIT left it as it was. Show details says what."
    );
    assert_eq!(
        with(files_left(&run.items[1])),
        "RABBIT restored 1 of 2 backups.\n\nRABBIT couldn't restore your key map from November 1, \
         2026, 6:40 PM, and some files it had already changed couldn't be put back as they were. \
         The files they replaced are kept as a backup on the Backups tab. Show details says which."
    );
    // Show details starts its line with the row's name.
    let outcome = done(
        &run,
        Ok(report(vec![
            sws_restored(&run.items[0]),
            failed(&run.items[1]),
        ])),
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"Your key map: Not restored".to_string())
    );
}

/// A failed going back to an earlier version says what couldn't go back:
/// the files left as the backup has them, and a file of the newer version
/// still in the new backup, which keeps what it replaced.
#[test]
fn a_failed_swap_says_what_couldnt_go_back() {
    let run = one(reapack());
    let outcome = done(&run, Ok(report(vec![reapack_files_left(&run.items[0])])));
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::NothingRestored);
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't restore ReaPack 1.2.4.\n\nSome files RABBIT had already changed couldn't \
         be put back as they were. The files they replaced are kept as a backup on the Backups \
         tab. Show details says which."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER",
            "ReaPack 1.2.4: Not restored, and some files couldn't be put back as they were",
            "  Error: C:/REAPER/UserPlugins/reapack_stuck.dll: Access is denied. (os error 5)",
            "  Couldn't be put back as it was, so it is now the backup's copy: \
             C:/REAPER/UserPlugins/reaper_reapack-x64.dll",
            "  A copy of what was there before is in \
             C:/REAPER/RABBIT/backups/b/reaper_reapack-x64.dll",
            "  Couldn't go back to its place, and is still in the new backup: \
             C:/REAPER/UserPlugins/reapack_stuck.dll",
            "  Stays where it is, because it changed since RABBIT installed it: \
             C:/REAPER/UserPlugins/reapack_changed.dll",
            "  What it replaced is kept as a backup on the Backups tab: ReaPack 1.2.5.",
            REPORT_LINE,
        ]
    );
}

#[test]
fn several_restored_are_counted_as_review_counts_them() {
    let run = three();
    let outcome = done(
        &run,
        Ok(report(vec![
            sws_restored(&run.items[0]),
            reapack_restored(&run.items[1], SwappedEntry::Replaced),
            keymap_restored(&run.items[2]),
        ])),
    );
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Restored);
    assert_eq!(outcome.status_text, "RABBIT restored 3 backups.");
    let names: Vec<&String> = outcome
        .detail_lines
        .iter()
        .filter(|line| !line.starts_with(' '))
        .collect();
    assert_eq!(
        names,
        [
            "Target: C:/REAPER",
            "SWS Extension 2.14.0.7: Restored",
            "ReaPack 1.2.4: Restored",
            "Your key map: Restored",
            REPORT_LINE,
        ]
    );
}

#[test]
fn a_partial_restore_says_how_many_came_back_and_why_the_others_didnt() {
    let run = three();
    let outcome = done(
        &run,
        Ok(report(vec![
            sws_restored(&run.items[0]),
            refused(&run.items[1], sample("changed-since-listed")),
            files_left(&run.items[2]),
        ])),
    );
    assert_eq!(
        outcome.status,
        BackupRestoreOutcomeStatus::CompletedWithProblems
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT restored 1 of 3 backups.\n\n\
         RABBIT didn't restore ReaPack 1.2.4. It changed since RABBIT listed it. Start RABBIT \
         again to see the backups as they are now.\n\
         RABBIT couldn't restore your key map from November 1, 2026, 6:40 PM, and some files it \
         had already changed couldn't be put back as they were. The files they replaced are kept \
         as a backup on the Backups tab. Show details says which."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER".to_string(),
            "SWS Extension 2.14.0.7: Restored".to_string(),
            "  Restored: C:/REAPER/UserPlugins/reaper_sws-x64.dll".to_string(),
            "ReaPack 1.2.4: Not restored".to_string(),
            "  It changed since RABBIT listed it. Start RABBIT again to see the backups as they \
             are now."
                .to_string(),
            "Your key map: Not restored, and some files couldn't be put back as they were"
                .to_string(),
            "  Error: C:/REAPER/UserPlugins/sws_python64.dll: Access is denied. (os error 5)"
                .to_string(),
            "  Couldn't be put back as it was, so it is now the backup's copy: \
             C:/REAPER/UserPlugins/reaper_sws-x64.dll"
                .to_string(),
            "  A copy of what was there before is in C:/REAPER/RABBIT/backups/b/reaper_sws-x64.dll"
                .to_string(),
            "  Gone, and couldn't be put back: C:/REAPER/UserPlugins/sws_python64.dll".to_string(),
            "  A copy of what was there before is in C:/REAPER/RABBIT/backups/b/sws_python64.dll"
                .to_string(),
            "  Put back as it was: C:/REAPER/UserPlugins/sws_other.dll".to_string(),
            "  Not restored, because its copy in the backup never finished: \
             C:/REAPER/UserPlugins/sws_unfinished.dll"
                .to_string(),
            format!(
                "  What it replaced is kept as a backup on the Backups tab: Files a restore \
                 replaced, from {BEFORE_RESTORE_DATE}."
            ),
            REPORT_LINE.to_string(),
        ]
    );

    // An error before anything of it changed.
    let outcome = done(
        &run,
        Ok(report(vec![
            sws_restored(&run.items[0]),
            reapack_restored(&run.items[1], SwappedEntry::Replaced),
            failed(&run.items[2]),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT restored 2 of 3 backups.\n\nSomething went wrong with your key map from November \
         1, 2026, 6:40 PM, so RABBIT left it as it was. Show details says what."
    );
    assert!(outcome.detail_lines.contains(
        &"  Error: C:/REAPER/RABBIT/backups/x/manifest.json: Access is denied.".to_string()
    ));
}

#[test]
fn a_backup_that_didnt_come_back_alone_is_called_it() {
    let run = one(sws());
    let restore = &run.items[0];
    let outcome = done(
        &run,
        Ok(report(vec![refused(restore, reinstalled(restore))])),
    );
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::NothingRestored);
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't restore SWS Extension 2.14.0.7, so it left it as it was.\n\nYou have \
         SWS Extension again, a copy installed after this backup was made. Uninstall the one \
         you have first, on the Uninstall tab, then restore."
    );
    let outcome = done(&run, Ok(report(vec![failed(restore)])));
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't restore SWS Extension 2.14.0.7, so it left it as it was.\n\nSomething \
         went wrong. Show details says what."
    );
    // Files it couldn't put back: it didn't leave everything as it was.
    let outcome = done(&run, Ok(report(vec![files_left(restore)])));
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::NothingRestored);
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't restore SWS Extension 2.14.0.7.\n\nSome files RABBIT had already \
         changed couldn't be put back as they were. The files they replaced are kept as a \
         backup on the Backups tab. Show details says which."
    );
    // A restore that had replaced nothing made no backup, so Done points to
    // none, and neither does Show details.
    let outcome = done(&run, Ok(report(vec![files_left_no_backup(restore)])));
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't restore SWS Extension 2.14.0.7.\n\nSome files RABBIT had already \
         changed couldn't be put back as they were. Show details says which."
    );
    assert!(
        !outcome
            .detail_lines
            .iter()
            .any(|line| line.contains("kept as a backup")),
        "{:#?}",
        outcome.detail_lines
    );
    let run = three();
    let outcome = done(
        &run,
        Ok(report(vec![
            sws_restored(&run.items[0]),
            reapack_restored(&run.items[1], SwappedEntry::Replaced),
            files_left_no_backup(&run.items[2]),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT restored 2 of 3 backups.\n\nRABBIT couldn't restore your key map from November 1, \
         2026, 6:40 PM, and some files it had already changed couldn't be put back as they were. \
         Show details says which."
    );
}

#[test]
fn none_of_several_restored_says_so() {
    let run = three();
    let outcome = done(
        &run,
        Ok(report(
            run.items
                .iter()
                .map(|restore| refused(restore, sample("not-found")))
                .collect(),
        )),
    );
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::NothingRestored);
    assert!(
        outcome.status_text.starts_with(
            "RABBIT couldn't restore the backups, so it left them as they were.\n\nRABBIT \
             didn't restore SWS Extension 2.14.0.7. It was deleted, or never existed."
        ),
        "{}",
        outcome.status_text
    );
}

#[test]
fn a_stopped_restore_says_how_far_it_got_and_that_the_rest_stays() {
    let run = three();
    let outcome = done(
        &run,
        Ok(stopped(vec![
            sws_restored(&run.items[0]),
            item(&run.items[1], RestoreStatus::NotReached),
            item(&run.items[2], RestoreStatus::NotReached),
        ])),
    );
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Stopped);
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped after restoring 1 of 3 backups. Backups it hadn't reached stay as they \
         are."
    );
    assert_eq!(
        &outcome.detail_lines[3..],
        [
            "ReaPack 1.2.4: Not reached",
            "  RABBIT stopped before it, so it stays as it was.",
            "Your key map: Not reached",
            "  RABBIT stopped before it, so it stays as it was.",
            REPORT_LINE,
        ]
    );
    let outcome = done(
        &run,
        Ok(stopped(
            run.items
                .iter()
                .map(|restore| item(restore, RestoreStatus::NotReached))
                .collect(),
        )),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped before it restored a backup. Backups it hadn't reached stay as they are."
    );
}

#[test]
fn a_run_refused_or_failed_as_a_whole_says_nothing_was_restored_and_why() {
    let run = three();
    let outcome = done(&run, Err(sample("lock-held").into()));
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Refused);
    assert_eq!(
        outcome.status_text,
        "Nothing was restored.\n\nAnother copy of RABBIT is working on this REAPER. Wait until it \
         has finished. To try again, close RABBIT and start it again."
    );
    assert_eq!(outcome.report_path, None);
    assert_eq!(
        &outcome.detail_lines[..4],
        [
            "Target: C:/REAPER",
            "SWS Extension 2.14.0.7: Not restored",
            "ReaPack 1.2.4: Not restored",
            "Your key map: Not restored",
        ]
    );
    assert!(outcome.detail_lines[4].starts_with("Error: "));

    let outcome = done(&run, Err(sample("reaper-running").into()));
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Refused);
    assert_eq!(
        outcome.status_text,
        "Nothing was restored.\n\nREAPER is open. Close it, then try again. To try again, close \
         RABBIT and start it again."
    );
    let outcome = done(&run, Err(sample("as-root").into()));
    assert!(
        outcome
            .status_text
            .ends_with("Quit RABBIT and start it as yourself."),
        "{}",
        outcome.status_text
    );
    // A refusal whose sentence says what to do already needs no more.
    let outcome = done(&run, Err(sample("changed-since-listed").into()));
    assert!(
        !outcome.status_text.contains("To try again"),
        "{}",
        outcome.status_text
    );

    let outcome = done(
        &run,
        Err(RabbitError::Io {
            path: PathBuf::from("C:/REAPER/RABBIT/backups"),
            source: std::io::Error::other("broken"),
        }),
    );
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Failed);
    assert_eq!(
        outcome.status_text,
        "Nothing was restored.\n\nRABBIT couldn't read the folder C:/REAPER/RABBIT/backups. Check \
         that it is there and that you can open it, then run RABBIT again."
    );
    let outcome = done(
        &run,
        Err(RabbitError::Io {
            path: PathBuf::from("C:/REAPER/RABBIT/locks/x.lock"),
            source: std::io::Error::other("broken"),
        }),
    );
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Failed);
    assert!(
        outcome.status_text.starts_with(
            "Nothing was restored.\n\nSomething went wrong before RABBIT restored anything: "
        ),
        "{}",
        outcome.status_text
    );
}

/// A backup refused partway because REAPER or a program started says so,
/// and Done says once how to try again: Done has no way back to Review.
#[test]
fn a_backup_refused_for_what_started_says_to_start_rabbit_again() {
    let run = three();
    let outcome = done(
        &run,
        Ok(report(vec![
            sws_restored(&run.items[0]),
            refused(&run.items[1], sample("reaper-running")),
            refused(&run.items[2], sample("program-running")),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT restored 1 of 3 backups.\n\nRABBIT didn't restore ReaPack 1.2.4. REAPER is open. \
         Close it, then try again.\nRABBIT didn't restore your key map from November 1, 2026, \
         6:40 PM. Close these programs, then try again: Surge XT.exe.\nTo try again, close RABBIT \
         and start it again."
    );
}

#[test]
fn the_language_is_said_once_and_what_went_wrong_with_it() {
    let langpack = |restored: bool, error: Option<&str>| RestoredPackage {
        package_id: "sws".to_string(),
        receipt_merged: true,
        receipt_already_current: false,
        langpack_before: Some("de_DE.ReaperLangPack".to_string()),
        langpack_restored: restored,
        langpack_error: error.map(ToString::to_string),
    };
    let run = one(sws());
    let with = |package: RestoredPackage| {
        let mut backup = sws_restored(&run.items[0]);
        backup.packages = vec![package];
        done(&run, Ok(report(vec![backup])))
    };
    let outcome = with(langpack(false, None));
    assert_eq!(
        outcome.status_text,
        "RABBIT restored SWS Extension 2.14.0.7.\n\nREAPER's language stays as it is, because \
         another one was chosen since."
    );
    let outcome = with(langpack(false, Some("reaper.ini: Access is denied.")));
    assert_eq!(
        outcome.status_text,
        "RABBIT restored SWS Extension 2.14.0.7.\n\nRABBIT couldn't set REAPER's language back \
         to de_DE.ReaperLangPack. Show details says why."
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"  Error: reaper.ini: Access is denied.".to_string())
    );

    // The first restore that puts the language back sets it; the next one
    // finds it chosen, which isn't the user's choice, so says nothing.
    let older = BackupEntry {
        item: package("uninstall-unix-1790938800", "ffmpeg"),
        created_unix: Some(OCT_2_1100),
        ..ffmpeg()
    };
    let run = run_of(&en(), &all_ticked(&en(), vec![sws(), older]));
    let mut first = sws_restored(&run.items[0]);
    first.packages = vec![langpack(true, None)];
    let mut second = sws_restored(&run.items[1]);
    second.packages = vec![RestoredPackage {
        package_id: "ffmpeg".to_string(),
        ..langpack(false, None)
    }];
    let outcome = done(&run, Ok(report(vec![first, second])));
    assert_eq!(
        outcome.status_text,
        "RABBIT restored 2 backups.\n\nREAPER's language is back to de_DE.ReaperLangPack."
    );
}

#[test]
fn packages_a_whole_set_left_out_and_its_packages_are_said() {
    // The tab restores single packages and key maps; a whole set of earlier
    // versions, which only the command line restores, still reads.
    let run = one(reapack());
    let mut backup = item(&run.items[0], RestoreStatus::PartlyRestored);
    backup.skipped_packages = vec![SkippedPackage {
        package_id: "sws".to_string(),
        reason: SkippedPackageReason::NothingOfItInSet,
    }];
    backup.swaps = vec![
        reapack_restored(&run.items[0], SwappedEntry::Removed),
        RestoreReportItem {
            package_id: Some("ffmpeg".to_string()),
            ..item(&run.items[0], RestoreStatus::NotReached)
        },
    ];
    let outcome = done(&run, Ok(report(vec![backup])));
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::NothingRestored);
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't restore ReaPack 1.2.4.\n\nOnly part of it came back. Show details says \
         which part.\n\nRABBIT no longer counts ReaPack as installed by it.\nTo uninstall \
         ReaPack with RABBIT later, install it again first.\nRABBIT left out SWS Extension, \
         because none of its files were in the backup."
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"  ReaPack: Restored".to_string()),
        "{:#?}",
        outcome.detail_lines
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"  FFmpeg for Improved Video Support: Not reached".to_string())
    );
    assert!(
        outcome
            .detail_lines
            .contains(&"    RABBIT stopped before it, so it stays as it was.".to_string())
    );
}

#[test]
fn every_restore_status_has_its_label() {
    let labels: Vec<String> = [
        RestoreStatus::Restored,
        RestoreStatus::WouldRestore,
        RestoreStatus::NotRestored,
        RestoreStatus::FilesLeft,
        RestoreStatus::NotReached,
        RestoreStatus::PartlyRestored,
    ]
    .into_iter()
    .map(|status| restore_status_label(&en(), status))
    .collect();
    assert_eq!(
        labels,
        [
            "Restored",
            "Would be restored",
            "Not restored",
            "Not restored, and some files couldn't be put back as they were",
            "Not reached",
            "Partly restored",
        ]
    );
}

// ---------------------------------------------------------------------------
// Every language.
// ---------------------------------------------------------------------------

const STATUSES: [RestoreStatus; 6] = [
    RestoreStatus::Restored,
    RestoreStatus::WouldRestore,
    RestoreStatus::NotRestored,
    RestoreStatus::FilesLeft,
    RestoreStatus::NotReached,
    RestoreStatus::PartlyRestored,
];

/// That `text` reads in `locale`: no message id, no placeable left.
fn assert_reads_here(locale: &str, text: &str) {
    assert_reads(locale, text);
    for marker in [
        "backup-date",
        "size-",
        "action-",
        "restore-refusal",
        "restore-target",
        "restore-status",
    ] {
        assert!(!text.contains(marker), "{locale}: {text}");
    }
}

fn assert_outcome_reads(locale: &str, outcome: &WizardBackupRestoreOutcome) {
    assert_reads_here(locale, &outcome.heading);
    assert!(
        outcome.status_text.starts_with(&outcome.heading),
        "{locale}: {}",
        outcome.status_text
    );
    for line in outcome.status_text.lines().filter(|line| !line.is_empty()) {
        assert_reads_here(locale, line);
    }
    for line in &outcome.detail_lines {
        assert_reads_here(locale, line);
    }
}

#[test]
fn every_restore_status_and_warning_reads_in_every_locale() {
    let warnings = [
        RestoreRefusal::ReaperRunning {
            every_reaper_for: Vec::new(),
            package_names: Vec::new(),
            processes: Vec::new(),
            puts_keymap_back: true,
        },
        RestoreRefusal::ReaperRunning {
            every_reaper_for: vec!["sws".to_string(), "jaws-scripts".to_string()],
            package_names: Vec::new(),
            processes: Vec::new(),
            puts_keymap_back: false,
        },
        RestoreRefusal::ProgramRunning {
            package_ids: vec!["surge-xt".to_string()],
            package_names: Vec::new(),
            programs: vec!["Surge XT.exe".to_string()],
        },
    ];
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for status in STATUSES {
            assert_reads_here(locale, &restore_status_label(&localizer, status));
        }
        for refusal in &warnings {
            let (title, body) = restore_running_warning(&localizer, refusal).unwrap();
            assert_reads_here(locale, &title);
            assert_reads_here(locale, &body);
        }
        // Every refusal of a run is another's to say.
        for refusal in sample_refusals() {
            let warns = restore_running_warning(&localizer, &refusal).is_some();
            assert_eq!(
                warns,
                matches!(
                    refusal,
                    RestoreRefusal::ReaperRunning { .. } | RestoreRefusal::ProgramRunning { .. }
                ),
                "{locale}: {refusal:?}"
            );
        }
        // The progress page's heading and first status line.
        let text = crate::wizard::text::wizard_text(&localizer);
        assert_reads_here(locale, &text.progress_restore_heading);
        assert_reads_here(locale, &text.progress_status_restore_running);
    }
}

#[test]
fn every_done_page_show_details_and_progress_line_reads_in_every_locale() {
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let entries = vec![sws(), reapack(), keymap()];
        let run = run_of(&localizer, &all_ticked(&localizer, entries));
        let check_run = |run: &BackupRestoreRun, result: rabbit_core::Result<RestoreRunReport>| {
            let outcome = wizard_backup_restore_outcome_at(&localizer, run, &result, &utc);
            assert_outcome_reads(locale, &outcome);
            outcome
        };
        let check = |result: rabbit_core::Result<RestoreRunReport>| check_run(&run, result);
        let all_restored = || {
            vec![
                sws_restored(&run.items[0]),
                reapack_restored(&run.items[1], SwappedEntry::Removed),
                keymap_restored(&run.items[2]),
            ]
        };

        // All back; every status for one among others; each problem.
        check(Ok(report(all_restored())));
        for status in STATUSES {
            let mut backups = all_restored();
            backups[1] = item(&run.items[1], status);
            check(Ok(report(backups)));
            check(Ok(report(
                run.items
                    .iter()
                    .map(|restore| item(restore, status))
                    .collect(),
            )));
        }
        let mut backups = all_restored();
        backups[0] = files_left(&run.items[0]);
        backups[1] = failed(&run.items[1]);
        check(Ok(report(backups)));
        check(Ok(report(run.items.iter().map(files_left).collect())));
        check(Ok(report(
            run.items.iter().map(files_left_no_backup).collect(),
        )));
        let mut backups = all_restored();
        backups[1] = reapack_files_left(&run.items[1]);
        backups[2] = files_left_no_backup(&run.items[2]);
        check(Ok(report(backups)));
        // Every refusal, of one backup and of the whole run.
        for refusal in sample_refusals() {
            let mut backups = all_restored();
            backups[2] = refused(&run.items[2], refusal.clone());
            let outcome = check(Ok(report(backups)));
            assert_eq!(
                outcome.status,
                BackupRestoreOutcomeStatus::CompletedWithProblems
            );
            let outcome = check(Err(refusal.into()));
            assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Refused);
        }
        // Stopped, before anything and after one.
        let mut backups: Vec<RestoreReportItem> = run
            .items
            .iter()
            .map(|restore| item(restore, RestoreStatus::NotReached))
            .collect();
        check(Ok(stopped(backups.clone())));
        backups[0] = sws_restored(&run.items[0]);
        let outcome = check(Ok(stopped(backups)));
        assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Stopped);
        // A folder the restore reads first, and any other error.
        for path in [
            RESOURCE,
            "C:/REAPER/RABBIT/backups",
            "C:/REAPER/RABBIT/x.lock",
        ] {
            let outcome = check(Err(RabbitError::Io {
                path: PathBuf::from(path),
                source: std::io::Error::other("broken"),
            }));
            assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Failed);
        }

        // One backup of each kind alone, for the forms that name it and
        // those that call it "it", with each note.
        let langpack = |restored: bool, error: Option<&str>| RestoredPackage {
            package_id: "sws".to_string(),
            receipt_merged: true,
            receipt_already_current: false,
            langpack_before: Some("de_DE.ReaperLangPack".to_string()),
            langpack_restored: restored,
            langpack_error: error.map(ToString::to_string),
        };
        let mut entries = vec![sws(), reapack(), reapack_unversioned(), osara_keymap()];
        // A key map of every origin, with a date and without.
        for origin in [
            Some(KeymapOrigin::OwnKeymap),
            Some(KeymapOrigin::DefaultKeys),
            Some(KeymapOrigin::UnchangedOsara),
            Some(KeymapOrigin::Unknown),
            None,
        ] {
            for dated in [true, false] {
                entries.push(keymap_of(origin, dated));
            }
        }
        for entry in entries {
            let alone = run_of(&localizer, &all_ticked(&localizer, vec![entry]));
            let restore = &alone.items[0];
            let mut restored = match &alone.rows[0].kind {
                RestoredBackupKind::Uninstalled => sws_restored(restore),
                RestoredBackupKind::Earlier { version } => {
                    let mut restored = reapack_restored(restore, SwappedEntry::Removed);
                    // Back to the copy an update replaced, when no version is
                    // known.
                    restored.swap.as_mut().unwrap().version = version.clone();
                    restored
                }
                RestoredBackupKind::Keymap { .. } => keymap_restored(restore),
            };
            for package in [
                langpack(true, None),
                langpack(false, None),
                langpack(false, Some("broken")),
            ] {
                restored.packages = vec![package];
                restored.skipped_packages = vec![SkippedPackage {
                    package_id: "ffmpeg".to_string(),
                    reason: SkippedPackageReason::NothingOfItInSet,
                }];
                check_run(&alone, Ok(report(vec![restored.clone()])));
            }
            for refusal in sample_refusals() {
                check_run(&alone, Ok(report(vec![refused(restore, refusal)])));
            }
            for backup in [
                failed(restore),
                files_left(restore),
                files_left_no_backup(restore),
                reapack_files_left(restore),
                item(restore, RestoreStatus::PartlyRestored),
            ] {
                check_run(&alone, Ok(report(vec![backup])));
            }

            // The progress page's lines.
            let mut state = ProgressUiState::new(WizardMode::Restore, 1, 0)
                .with_backup_restore(backup_restore_progress(&localizer, &alone));
            let (status_line, log) =
                backup_restore_progress_lines(&localizer, &state, &started(restore));
            assert_reads_here(locale, &status_line.unwrap());
            assert_reads_here(locale, &log.unwrap());
            for status in STATUSES {
                state.record(&completed(restore, status));
                if let (_, Some(log)) =
                    backup_restore_progress_lines(&localizer, &state, &completed(restore, status))
                {
                    assert_reads_here(locale, &log);
                }
            }
            assert_reads_here(
                locale,
                &backup_restore_progress_details_for_start(&localizer, &alone),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The run button and the close question.
// ---------------------------------------------------------------------------

#[test]
fn the_restore_review_decides_whether_the_button_is_enabled() {
    let mut list = list_in(&en(), vec![sws(), reapack()]);
    // Nothing ticked: the button stays off.
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, None, &[]);
    assert!(!review.can_restore);
    assert!(!WizardMode::Restore.run_button_enabled(true, true, true, review.can_restore));

    tick(&mut list, SWS, BackupChoice::Restore);
    // Ticked, but no quick plan checked it: still off.
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, None, &[]);
    assert!(!review.can_restore);
    assert!(!WizardMode::Restore.run_button_enabled(true, true, true, review.can_restore));

    // Checked, with nothing in the way: on, whatever the other flags say.
    let plan = plan_all(&list);
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&plan), &[]);
    assert!(review.can_restore);
    assert!(WizardMode::Restore.run_button_enabled(false, false, false, review.can_restore));
    // A run of what Review checked.
    assert_eq!(run_of(&en(), &list).items, review.items);

    // A quick plan of other ticks than these keeps it off, and one of these
    // ticks turns it on again.
    tick(&mut list, REAPACK, BackupChoice::Restore);
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&plan), &[]);
    assert!(!review.can_restore);
    let plan = plan_all(&list);
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&plan), &[]);
    assert!(review.can_restore);
    // Enter in Review never starts a restore.
    assert!(!WizardMode::Restore.enter_starts_run());
}

#[test]
fn closing_the_window_during_a_restore_asks_in_the_restores_words() {
    for &locale in embedded_locales() {
        let text = crate::wizard::text::wizard_text(&Localizer::embedded(locale).unwrap());
        assert_eq!(
            stop_question(&text, WizardMode::Restore),
            (
                text.close_during_restore_title.as_str(),
                text.close_during_restore_body.as_str()
            ),
            "{locale}"
        );
        assert_reads(locale, &text.close_during_restore_title);
        assert_reads(locale, &text.close_during_restore_body);
        // A restore stops after the backup it is on, and says so with the
        // install's line.
        assert_eq!(
            stopping_line(&text, WizardMode::Restore),
            text.progress_status_cancelling,
            "{locale}"
        );
    }
    let text = crate::wizard::text::wizard_text(&en());
    assert_eq!(text.close_during_restore_title, "Stop restoring?");
    assert_eq!(
        text.close_during_restore_body,
        "RABBIT is restoring. If you stop now, RABBIT finishes the backup it is restoring, then \
         stops and closes. Backups it hasn't reached stay as they are. Stop restoring?"
    );
    assert_eq!(text.progress_restore_heading, "Restoring backups");
    assert_eq!(
        text.progress_status_restore_running,
        "Restoring the ticked backups."
    );
}

// ---------------------------------------------------------------------------
// A real restore, in a temporary folder.
// ---------------------------------------------------------------------------

/// The key map set OSARA's key map replacement wrote on November 1.
const KEYMAP_SET: &str = "unattended-unix-1793558400-000000001";
const OWN_KEYS: &[u8] = b"KEY 1 65 40044 0\r\n";
const OSARA_KEYS: &[u8] = b"KEY 1 65 40001 0\r\nSCR 4 0 RSnew \"Custom: new.lua\" new.lua\r\n";

/// Your own key map in `resource`, which OSARA's key map replaced: a key
/// map set holding yours, and OSARA's in its place, with a script ReaPack
/// registered since.
fn write_keymap_set(resource: &Path) {
    fs::create_dir_all(resource).unwrap();
    let keymap = resource.join("reaper-kb.ini");
    fs::write(&keymap, OWN_KEYS).unwrap();
    let mut writer = BackupSetWriter::new(resource, KEYMAP_SET, "osara-keymap-replacement");
    writer.set_keymap_origin(KeymapOrigin::OwnKeymap).unwrap();
    writer.copy_file(Some("osara"), &keymap).unwrap();
    fs::write(&keymap, OSARA_KEYS).unwrap();
}

const UNINSTALL_SET: &str = "uninstall-unix-1794233100";

fn set_path(resource: &Path, set_id: &str) -> PathBuf {
    resource.join("RABBIT").join("backups").join(set_id)
}

/// Restore what `run` takes on a computer where nothing runs, through
/// [`run_backup_restore`]'s own code, which hands the engine the run's
/// folder and backups with the wizard's options.
fn restore(
    run: &BackupRestoreRun,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> rabbit_core::Result<RestoreRunReport> {
    let on_test_computer = |path: &Path,
                            items: &[RestoreItem],
                            options: &RestoreRunOptions,
                            progress: &ProgressReporter,
                            cancel: &CancelToken| {
        assert_eq!(path, run.resource_path.as_path());
        assert_eq!(items, run.items.as_slice());
        // A real run, never a dry one, that refuses a running REAPER.
        assert_eq!(*options, backup_restore_run_options());
        assert!(!options.dry_run && !options.quick && !options.allow_reaper_running);
        restore_backups_on(
            path,
            items,
            options,
            &TestComputer::default(),
            progress,
            cancel,
        )
    };
    run_backup_restore_with(run, progress, cancel, &on_test_computer)
}

#[test]
fn a_real_restore_brings_back_exactly_the_ticked_backups_and_saves_its_report_there() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    write_uninstall_set(&resource);
    write_keymap_set(&resource);
    let plugins = resource.join("UserPlugins");
    assert!(!plugins.join("reaper_sws-x64.dll").exists());

    let mut list = backup_list_at(&en(), &resource);
    tick(&mut list, SWS, BackupChoice::Restore);
    tick(&mut list, KEYMAP, BackupChoice::Restore);
    let run = backup_restore_run(&en(), &resource, &list);
    assert_eq!(run.backups(), 2);

    let events: Arc<Mutex<Vec<ProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let progress = {
        let events = Arc::clone(&events);
        ProgressReporter::new(move |event| events.lock().unwrap().push(event))
    };
    let result = restore(&run, &progress, &CancelToken::new());
    let report = result.as_ref().unwrap();
    assert!(!report.stopped);
    assert!(
        report
            .backups
            .iter()
            .all(|backup| backup.status == RestoreStatus::Restored),
        "{report:#?}"
    );

    // SWS is back, FFmpeg of the same set isn't, and both stay in the set.
    assert_eq!(
        fs::read(plugins.join("reaper_sws-x64.dll")).unwrap(),
        b"sws"
    );
    assert!(!plugins.join("avcodec-61.dll").exists());
    let manifest = set_path(&resource, UNINSTALL_SET).join(BACKUP_MANIFEST_FILE);
    assert!(manifest.is_file());
    let records = load_install_state(&resource).unwrap().unwrap();
    assert!(records.packages.contains_key("sws"));
    assert!(!records.packages.contains_key("ffmpeg"));
    // Your key map is back, with the script registered since.
    let keys = fs::read_to_string(resource.join("reaper-kb.ini")).unwrap();
    assert!(keys.starts_with("KEY 1 65 40044 0"), "{keys}");
    assert!(keys.contains("RSnew"), "{keys}");
    assert!(set_path(&resource, KEYMAP_SET).is_dir());

    // The report is in RABBIT/logs of this REAPER, as the command line's.
    let saved = report.saved_report.as_ref().unwrap();
    let logs = resource.join("RABBIT").join("logs");
    assert_eq!(saved.json_path.parent(), Some(logs.as_path()));
    assert!(saved.json_path.is_file() && saved.text_path.is_file());
    let name = saved.json_path.file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("restore-backup-unix-"), "{name}");

    // The progress page heard each backup start and end, in order.
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 4);
    let mut state = ProgressUiState::new(WizardMode::Restore, run.items.len(), 0)
        .with_backup_restore(backup_restore_progress(&en(), &run));
    let mut heard = Vec::new();
    for event in events.iter() {
        state.record(event);
        let (status_line, log) = backup_restore_progress_lines(&en(), &state, event);
        heard.extend(status_line.into_iter().chain(log));
    }
    assert_eq!(heard[0], "Restoring SWS Extension 2.14.0.7…");
    assert_eq!(heard[2], "SWS Extension 2.14.0.7: restored.");
    assert!(
        heard[3].starts_with("Restoring your key map from "),
        "{}",
        heard[3]
    );
    assert_eq!(heard[5], "Your key map: restored.");
    assert_eq!(state.percentage(), 100);

    // Done counts the two ticked rows, says what it kept of the key map it
    // replaced, and points to the report.
    let outcome = wizard_backup_restore_outcome(&en(), &run, &result);
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Restored);
    assert_eq!(outcome.status_text, "RABBIT restored 2 backups.");
    assert_eq!(outcome.report_path.as_ref(), Some(&saved.text_path));
    assert_eq!(
        outcome.detail_lines.last(),
        Some(&format!("Report: {}", saved.text_path.display()))
    );
    assert!(
        outcome.detail_lines.iter().any(|line| line.starts_with(
            "  What it replaced is kept as a backup on the Backups tab: Key map, from "
        )),
        "{:#?}",
        outcome.detail_lines
    );
    assert!(
        outcome
            .detail_lines
            .iter()
            .any(|line| line.starts_with("  Restored: ") && line.ends_with("reaper_sws-x64.dll")),
        "{:#?}",
        outcome.detail_lines
    );

    // The rescan after the run reads the live list again: SWS is back, the
    // key map it replaced is a backup of its own, no tick is kept, and
    // Restore is chosen.
    let (_, model) = langpack_model(rabbit_core::plan::PlanActionKind::Install);
    let target = TargetRow {
        path: resource.clone(),
        ..target(true)
    };
    reread_backup_list(&mut list, &model, &target);
    assert_eq!(list.choice, BackupChoice::Restore);
    assert!(
        list.rows
            .iter()
            .all(|row| !row.restore_ticked && !row.delete_ticked)
    );
    let sws_row = &list.rows[index_of(&list, SWS)];
    assert_eq!(
        sws_row.entry.as_ref().map(|entry| entry.status),
        Some(BackupStatus::AlreadyBack)
    );
    let keymaps = list
        .rows
        .iter()
        .filter(|row| {
            row.entry
                .as_ref()
                .is_some_and(|entry| matches!(entry.item, BackupItem::Keymap { .. }))
        })
        .count();
    assert_eq!(keymaps, 2, "the key map the restore replaced is listed");
    // Show details names that backup as the rescanned tab's row does.
    let kept_set = report.backups[1].before_restore_set_id.as_ref().unwrap();
    let kept_row = list
        .rows
        .iter()
        .find(|row| {
            row.entry.as_ref().is_some_and(
                |entry| matches!(&entry.item, BackupItem::Keymap { set_id } if set_id == kept_set),
            )
        })
        .unwrap();
    let kept_line = format!(
        "  What it replaced is kept as a backup on the Backups tab: {}, from ",
        kept_row.name
    );
    assert!(
        outcome
            .detail_lines
            .iter()
            .any(|line| line.starts_with(&kept_line)),
        "{kept_line}: {:#?}",
        outcome.detail_lines
    );
}

#[test]
fn a_restore_stopped_before_the_first_backup_restores_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    write_uninstall_set(&resource);
    write_keymap_set(&resource);
    let mut list = backup_list_at(&en(), &resource);
    tick(&mut list, SWS, BackupChoice::Restore);
    tick(&mut list, KEYMAP, BackupChoice::Restore);
    let run = backup_restore_run(&en(), &resource, &list);
    let keys = fs::read(resource.join("reaper-kb.ini")).unwrap();

    let cancel = CancelToken::new();
    cancel.cancel();
    let result = restore(&run, &ProgressReporter::noop(), &cancel);
    let report = result.as_ref().unwrap();
    assert!(report.stopped);
    assert!(
        report
            .backups
            .iter()
            .all(|backup| backup.status == RestoreStatus::NotReached)
    );
    assert!(
        !resource
            .join("UserPlugins")
            .join("reaper_sws-x64.dll")
            .exists()
    );
    assert_eq!(fs::read(resource.join("reaper-kb.ini")).unwrap(), keys);
    assert!(load_install_state(&resource).unwrap().is_none());

    let outcome = wizard_backup_restore_outcome(&en(), &run, &result);
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::Stopped);
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped before it restored a backup. Backups it hadn't reached stay as they are."
    );
}

#[test]
fn a_set_changed_since_listing_is_refused_and_nothing_changes() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    write_uninstall_set(&resource);
    let mut list = backup_list_at(&en(), &resource);
    tick(&mut list, SWS, BackupChoice::Restore);
    let run = backup_restore_run(&en(), &resource, &list);
    // Another program changes the set after the list was read.
    let manifest = set_path(&resource, UNINSTALL_SET).join(BACKUP_MANIFEST_FILE);
    let mut text = fs::read_to_string(&manifest).unwrap();
    text.push_str("\n\n");
    fs::write(&manifest, text).unwrap();
    let before = snapshot(dir.path());

    let result = restore(&run, &ProgressReporter::noop(), &CancelToken::new());
    let report = result.as_ref().unwrap();
    assert_eq!(report.backups[0].status, RestoreStatus::NotRestored);
    assert!(matches!(
        report.backups[0].refusal,
        Some(RestoreRefusal::ChangedSinceListed { .. })
    ));
    // Every backup was refused on its own: no lock, no report, no change.
    assert!(report.saved_report.is_none());
    assert_eq!(snapshot(dir.path()), before);

    let outcome = wizard_backup_restore_outcome(&en(), &run, &result);
    assert_eq!(outcome.status, BackupRestoreOutcomeStatus::NothingRestored);
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't restore SWS Extension 2.14.0.7, so it left it as it was.\n\nIt changed \
         since RABBIT listed it. Start RABBIT again to see the backups as they are now."
    );
    assert_eq!(outcome.report_path, None);
}

// ---------------------------------------------------------------------------
// Variants and REAPER's language.
// ---------------------------------------------------------------------------

/// How REAPER's language ended after going back to the Spanish pack's es_ES
/// variant.
#[derive(Clone, Copy, Debug)]
enum SpanishLanguage {
    /// Set to es_ES, as REAPER used es_MX.
    Set,
    /// Left as it was, as REAPER used another pack, or none.
    Unchanged,
    /// Setting it failed.
    Failed,
}

/// Going back from the Spanish pack's es_MX variant to es_ES, at the same
/// version: es_ES came back, es_MX moved into the new backup, and REAPER's
/// language ended as `language` says.
fn spanish_restored(restore: &RestoreItem, language: SpanishLanguage) -> RestoreReportItem {
    let set = matches!(language, SpanishLanguage::Set);
    RestoreReportItem {
        actions: vec![action(
            "LangPack/es_ES.ReaperLangPack",
            RestoreBackupActionKind::Restored,
            None,
        )],
        before_restore_set_id: Some(BEFORE_RESTORE.to_string()),
        swap: Some(SwappedPackage {
            package_id: "langpack-es".to_string(),
            version: Some(version("1.0")),
            replaced_version: Some(version("1.0")),
            variant: Some("rae".to_string()),
            replaced_variant: Some("pma".to_string()),
            entry: SwappedEntry::Replaced,
            uninstall_needs_reinstall: false,
            undeclined: false,
            moved_out: vec![MovedOutFile {
                path: PathBuf::from(format!("{RESOURCE}/LangPack/es_MX.ReaperLangPack")),
                backup_path: Some(PathBuf::from(
                    "C:/REAPER/RABBIT/backups/b/LangPack/es_MX.ReaperLangPack",
                )),
                size: 7,
                sha256: "0".repeat(64),
                action: MoveOutAction::MovedOut,
                error: None,
            }],
            changed_left: Vec::new(),
            left_in_place: Vec::new(),
            langpack: Some("es_ES.ReaperLangPack".to_string()),
            langpack_before: set.then(|| "es_MX.ReaperLangPack".to_string()),
            langpack_restored: set,
            langpack_error: matches!(language, SpanishLanguage::Failed)
                .then(|| "C:/REAPER/reaper.ini: Access is denied. (os error 5)".to_string()),
            langpack_reaper_may_run: false,
        }),
        ..item(restore, RestoreStatus::Restored)
    }
}

/// Done names the variant the Spanish pack went back to, alone as both
/// variants are the same version, says what became of REAPER's language,
/// and Show details names the variant kept as a backup and why the language
/// couldn't be set.
#[test]
fn going_back_to_another_variant_says_the_variant_and_reapers_language() {
    let run = one(super::backups::spanish_es());
    let heading = "REAPER in Spanish is back to REAPER Accesible español (es_ES).";
    let outcome = |language| {
        done(
            &run,
            Ok(report(vec![spanish_restored(&run.items[0], language)])),
        )
    };

    let set = outcome(SpanishLanguage::Set);
    assert_eq!(set.status, BackupRestoreOutcomeStatus::Restored);
    assert_eq!(
        set.status_text,
        format!("{heading}\n\nREAPER's language is back to es_ES.ReaperLangPack.")
    );
    for line in [
        "  REAPER's language is back to es_ES.ReaperLangPack.".to_string(),
        "  Moved into the new backup, because the earlier version doesn't have it: \
         C:/REAPER/LangPack/es_MX.ReaperLangPack"
            .to_string(),
        format!(
            "  What it replaced is kept as a backup on the Backups tab: REAPER in Spanish, Team \
             PMA (es_MX), from {BEFORE_RESTORE_DATE}."
        ),
    ] {
        assert!(
            set.detail_lines.contains(&line),
            "{line:?} not in {:#?}",
            set.detail_lines
        );
    }

    let unchanged = outcome(SpanishLanguage::Unchanged);
    assert_eq!(
        unchanged.status_text,
        format!("{heading}\n\nREAPER's language stays as it is.")
    );

    let failed = outcome(SpanishLanguage::Failed);
    assert_eq!(
        failed.status_text,
        format!(
            "{heading}\n\nRABBIT couldn't set REAPER's language back to es_ES.ReaperLangPack. \
             Show details says why."
        )
    );
    assert!(
        failed
            .detail_lines
            .contains(&"  Error: C:/REAPER/reaper.ini: Access is denied. (os error 5)".to_string()),
        "{:#?}",
        failed.detail_lines
    );
}

/// A whole set of earlier versions says what became of REAPER's language
/// for the language pack that went back with it.
#[test]
fn a_whole_set_says_reapers_language_for_its_language_pack() {
    let run = one(super::backups::spanish_es());
    let mut whole = item(&run.items[0], RestoreStatus::Restored);
    whole.package_id = None;
    whole.swaps = vec![spanish_restored(&run.items[0], SpanishLanguage::Set)];
    let outcome = done(&run, Ok(report(vec![whole])));
    assert!(
        outcome
            .status_text
            .ends_with("\n\nREAPER's language is back to es_ES.ReaperLangPack."),
        "{}",
        outcome.status_text
    );
}

/// Every way REAPER's language can end after going back to another variant
/// reads in every language, Done and Show details, and names the variant.
#[test]
fn reapers_language_after_another_variant_reads_in_every_locale() {
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let run = run_of(
            &localizer,
            &all_ticked(&localizer, vec![super::backups::spanish_es()]),
        );
        let rae = localizer.text("package-langpack-es-variant-rae").value;
        for (language, key) in [
            (SpanishLanguage::Set, "wizard-done-restore-language-back"),
            (
                SpanishLanguage::Unchanged,
                "wizard-done-restore-language-unchanged",
            ),
            (
                SpanishLanguage::Failed,
                "wizard-done-restore-language-failed",
            ),
        ] {
            let outcome = wizard_backup_restore_outcome_at(
                &localizer,
                &run,
                &Ok(report(vec![spanish_restored(&run.items[0], language)])),
                &utc,
            );
            let line = localizer
                .format(key, &[("file", "es_ES.ReaperLangPack")])
                .value;
            assert!(
                outcome.status_text.contains(&line),
                "{locale} {language:?}: {}",
                outcome.status_text
            );
            assert!(
                outcome.heading.contains(&rae),
                "{locale}: {}",
                outcome.heading
            );
            assert_reads(locale, &outcome.status_text);
            for line in &outcome.detail_lines {
                assert_reads(locale, line);
            }
        }
    }
}

/// Going back within one variant, as after an update of the Spanish pack
/// from one digest to the next, Done names the version by its digest
/// alone: it is the same variant as before.
#[test]
fn going_back_within_one_variant_names_the_version_alone() {
    let entry = BackupEntry {
        version: Some(version("h1a2b3c4d5e6f")),
        replaced_by: Some(version("h9f8e7d6c5b4a")),
        replaced_by_variant: Some("rae".to_string()),
        ..super::backups::spanish_es()
    };
    let run = one(entry);
    let mut restored = spanish_restored(&run.items[0], SpanishLanguage::Unchanged);
    if let Some(swap) = &mut restored.swap {
        swap.version = Some(version("h1a2b3c4d5e6f"));
        swap.replaced_version = Some(version("h9f8e7d6c5b4a"));
        swap.replaced_variant = Some("rae".to_string());
        swap.langpack = None;
        swap.moved_out = Vec::new();
    }
    assert_eq!(
        done(&run, Ok(report(vec![restored]))).heading,
        "REAPER in Spanish is back to h1a2b3c4d5e6f."
    );
}

/// An uninstalled language pack whose language REAPER no longer uses,
/// restored together with an earlier version of another language pack:
/// Done says once what became of REAPER's language. When the earlier
/// version set it, nothing says it stays; when neither did, it stays is
/// said once.
#[test]
fn done_says_reapers_language_once_for_two_language_packs() {
    let german = BackupEntry {
        version: Some(version("1.0")),
        with_settings: Some(false),
        item: package("uninstall-unix-1794233100", "langpack-de"),
        ..sws()
    };
    let run = run_of(
        &en(),
        &all_ticked(&en(), vec![german, super::backups::spanish_es()]),
    );
    let mut german_back = sws_restored(&run.items[0]);
    german_back.packages = vec![RestoredPackage {
        package_id: "langpack-de".to_string(),
        receipt_merged: true,
        receipt_already_current: false,
        langpack_before: Some("de_DE.ReaperLangPack".to_string()),
        langpack_restored: false,
        langpack_error: None,
    }];
    for (spanish, expected) in [
        (
            SpanishLanguage::Set,
            "RABBIT restored 2 backups.\n\nREAPER's language is back to es_ES.ReaperLangPack.",
        ),
        (
            SpanishLanguage::Unchanged,
            "RABBIT restored 2 backups.\n\nREAPER's language stays as it is, because another \
             one was chosen since.",
        ),
    ] {
        let outcome = done(
            &run,
            Ok(report(vec![
                german_back.clone(),
                spanish_restored(&run.items[1], spanish),
            ])),
        );
        assert_eq!(outcome.status_text, expected, "{spanish:?}");
    }
}

/// An earlier version of a language pack that wasn't installed, as after
/// it was uninstalled, came back: Done says REAPER's language stays as it
/// is, as Review and the row's details said.
#[test]
fn an_earlier_language_pack_that_wasnt_installed_leaves_the_language() {
    let run = one(super::backups::spanish_es());
    let mut restored = spanish_restored(&run.items[0], SpanishLanguage::Unchanged);
    if let Some(swap) = &mut restored.swap {
        swap.entry = SwappedEntry::Added;
        swap.replaced_version = None;
        swap.replaced_variant = None;
        swap.langpack = None;
        swap.moved_out = Vec::new();
    }
    let outcome = done(&run, Ok(report(vec![restored])));
    assert_eq!(
        outcome.status_text,
        "REAPER in Spanish is back to 1.0, REAPER Accesible español (es_ES).\n\nREAPER's \
         language stays as it is."
    );
}
