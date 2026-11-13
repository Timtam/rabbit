//! Tests of restoring several backups in one run: all or nothing per
//! backup, stopping between backups, fingerprints, the quick plan and the
//! report every real run saves. Temporary folders only.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tempfile::tempdir;

use super::restore_seam::{self, Step};
use super::{
    BackupEnvironment, BackupPackageRecord, BackupSetWriter, OutsideRestoreRules,
    RestoreBackupActionKind, RestoreBackupOptions, RestoreItem, RestoreReportItem,
    RestoreRunOptions, RestoreRunReport, RestoreStatus, load_backup_manifest,
    restore_backup_set_within, restore_backups_with, save_backup_manifest,
};
use crate::backup_overview::{BackupItem, overview_with};
use crate::cancel::CancelToken;
use crate::error::RabbitError;
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::reaper_ini::lang_pack_setting;
use crate::receipt::{InstallState, PackageReceipt, load_install_state, save_install_state};
use crate::restore_refusal::{RestoreRefusal, TargetRefusal, refusal};
use crate::uninstall::paths::link_folder;

/// A real run that may restore while REAPER runs; no REAPER runs in tests.
const APPLY: RestoreRunOptions = RestoreRunOptions {
    dry_run: false,
    quick: false,
    allow_reaper_running: true,
};

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A receipt entry for `id`, as an install of version 1.0 writes one.
fn receipt_entry(id: &str) -> PackageReceipt {
    PackageReceipt {
        id: id.to_string(),
        version: Some(crate::version::Version::parse("1.0").unwrap()),
        source_url: None,
        source_sha256: None,
        variant: None,
        channel: None,
        installed_files: Vec::new(),
        installed_at: Some("unix-1".to_string()),
        rabbit_version: Some("0.4.4".to_string()),
        architecture: None,
        vendor_added_files: Vec::new(),
    }
}

/// An uninstall's set `set_id` in `resource`, holding `package_id` with
/// its receipt entry and `files`, each written with its contents and then
/// moved into the set. Each file is gone from its place, and so is each
/// folder that leaves empty.
fn uninstalled(resource: &Path, set_id: &str, package_id: &str, files: &[(&str, &[u8])]) {
    let record = BackupPackageRecord {
        receipt: Some(receipt_entry(package_id)),
        langpack_before: None,
        with_settings: None,
    };
    uninstalled_with(resource, set_id, package_id, record, files);
}

/// [`uninstalled`], with `record` as what the uninstall recorded of the
/// package.
fn uninstalled_with(
    resource: &Path,
    set_id: &str,
    package_id: &str,
    record: BackupPackageRecord,
    files: &[(&str, &[u8])],
) {
    let mut writer = BackupSetWriter::new(resource, set_id, "uninstall");
    writer.record_package(package_id, record).unwrap();
    for (relative, contents) in files {
        let place = resource.join(relative);
        write(&place, contents);
        writer
            .move_file(Some(package_id), &place, None, false)
            .unwrap();
        let mut folder = place.parent();
        while let Some(path) = folder.filter(|path| *path != resource) {
            if fs::remove_dir(path).is_err() {
                break;
            }
            folder = path.parent();
        }
    }
}

/// One package of the set `set_id`, with no fingerprint to compare.
fn item(set_id: &str, package_id: &str) -> RestoreItem {
    RestoreItem::package(set_id, package_id)
}

/// Every event `run` reports, and the reporter that collects them.
fn collecting() -> (Arc<Mutex<Vec<ProgressEvent>>>, ProgressReporter) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let reporter = ProgressReporter::new(move |event| sink.lock().unwrap().push(event));
    (events, reporter)
}

/// Restore `items` from `resource` as the user, with nowhere outside the
/// resource folder to write to.
fn run(
    resource: &Path,
    items: &[RestoreItem],
    options: &RestoreRunOptions,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> RestoreRunReport {
    restore_backups_with(
        resource,
        items,
        options,
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
        progress,
        cancel,
    )
    .unwrap()
}

fn statuses(report: &RestoreRunReport) -> Vec<RestoreStatus> {
    report.backups.iter().map(|backup| backup.status).collect()
}

fn kinds(backup: &RestoreReportItem) -> Vec<RestoreBackupActionKind> {
    backup.actions.iter().map(|action| action.action).collect()
}

/// The sets in `resource`'s `RABBIT/backups`, by name.
fn sets(resource: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(resource.join("RABBIT").join("backups"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// The report `report` says it saved, as saved, and check its text file is
/// there too.
fn saved(report: &RestoreRunReport) -> serde_json::Value {
    let saved = report.saved_report.clone().expect("a saved report");
    let name = saved.json_path.file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("restore-backup-unix-"), "{name}");
    assert!(saved.text_path.is_file());
    let envelope: crate::report::SavedJsonReport =
        serde_json::from_str(&fs::read_to_string(&saved.json_path).unwrap()).unwrap();
    envelope.report
}

/// Every file and link below `root`, each file with what it holds, found
/// without following any link.
pub(super) fn snapshot(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut found = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            if kind.is_symlink() {
                found.push((path, None));
            } else if kind.is_dir() {
                folders.push(path);
            } else {
                found.push((path.clone(), Some(fs::read(&path).unwrap())));
            }
        }
    }
    found.sort();
    found
}

/// Three uninstall sets in `resource`: SWS, then ReaPack with three files,
/// then FFmpeg. Since the uninstall, ReaPack's first file and its third
/// are there again, as newer copies; its second, in a folder of its own,
/// isn't.
fn three_uninstalls(resource: &Path) {
    uninstalled(
        resource,
        "uninstall-unix-1",
        "sws",
        &[("UserPlugins/reaper_sws.dll", b"sws")],
    );
    uninstalled(
        resource,
        "uninstall-unix-2",
        "reapack",
        &[
            ("UserPlugins/reaper_reapack.dll", b"old reapack"),
            ("Data/ReaPack/cache/index.xml", b"old index"),
            ("UserPlugins/reapack_third.dll", b"old third"),
        ],
    );
    uninstalled(
        resource,
        "uninstall-unix-3",
        "ffmpeg",
        &[("UserPlugins/avcodec.dll", b"ffmpeg")],
    );
    write(
        &resource.join("UserPlugins/reaper_reapack.dll"),
        b"newer reapack",
    );
    write(
        &resource.join("UserPlugins/reapack_third.dll"),
        b"newer third",
    );
}

/// Two backups in one run, the second failing at its third file after
/// that file's old copy was removed: the first stays restored. The second
/// puts back the file it replaced and the one that failed, from its
/// before-restore set, removes the file it added and the folders it made
/// for it, leaves RABBIT's records without its entry and no set of its
/// own. The third still runs. The report, saved, says so per backup and
/// per file, and each backup was heard starting and ending.
#[test]
fn a_backup_that_fails_partway_is_put_back_and_the_run_goes_on() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let reapack_set = resource.join("RABBIT/backups/uninstall-unix-2");
    let reapack_manifest = fs::read(reapack_set.join("backup-manifest.json")).unwrap();
    let third = resource.join("UserPlugins/reapack_third.dll");
    let failing = third.clone();
    let _hook = restore_seam::install(move |step, path| {
        if step == Step::Replacing && path == failing {
            return Err(std::io::Error::other("the disk is full"));
        }
        Ok(())
    });
    let (events, progress) = collecting();

    let report = run(
        &resource,
        &[
            item("uninstall-unix-1", "sws"),
            item("uninstall-unix-2", "reapack"),
            item("uninstall-unix-3", "ffmpeg"),
        ],
        &APPLY,
        &progress,
        &CancelToken::new(),
    );

    assert_eq!(
        statuses(&report),
        [
            RestoreStatus::Restored,
            RestoreStatus::NotRestored,
            RestoreStatus::Restored
        ]
    );
    assert!(!report.stopped);
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_sws.dll")).unwrap(),
        b"sws"
    );
    assert_eq!(
        fs::read(resource.join("UserPlugins/avcodec.dll")).unwrap(),
        b"ffmpeg"
    );

    let reapack = &report.backups[1];
    assert_eq!(
        kinds(reapack),
        [
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::PutBack
        ]
    );
    assert!(
        reapack
            .error
            .as_deref()
            .is_some_and(|error| error.contains("the disk is full")),
        "{reapack:?}"
    );
    assert_eq!(reapack.refusal, None);
    assert!(reapack.packages.is_empty());
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_reapack.dll")).unwrap(),
        b"newer reapack"
    );
    assert_eq!(fs::read(&third).unwrap(), b"newer third");
    assert!(!resource.join("Data").exists());
    assert!(
        reapack
            .actions
            .iter()
            .all(|action| action.current_backup_path.is_none()),
        "{reapack:?}"
    );
    // Its before-restore set went again; the restores before and after it
    // replaced nothing, so no set but the three uninstalls is left.
    assert_eq!(reapack.before_restore_set_id, None);
    assert_eq!(
        sets(&resource),
        ["uninstall-unix-1", "uninstall-unix-2", "uninstall-unix-3"]
    );
    assert_eq!(
        fs::read(reapack_set.join("backup-manifest.json")).unwrap(),
        reapack_manifest
    );

    let state = load_install_state(&resource).unwrap().unwrap();
    let mut packages: Vec<&str> = state.packages.keys().map(String::as_str).collect();
    packages.sort_unstable();
    assert_eq!(packages, ["ffmpeg", "sws"]);

    let json = saved(&report);
    assert_eq!(json["backups"][0]["status"], "restored");
    assert_eq!(json["backups"][1]["status"], "not-restored");
    assert_eq!(json["backups"][1]["actions"][0]["action"], "put-back");
    assert_eq!(json["backups"][2]["status"], "restored");

    let events = events.lock().unwrap().clone();
    let expected: Vec<ProgressEvent> = [
        ("uninstall-unix-1", "sws", RestoreStatus::Restored),
        ("uninstall-unix-2", "reapack", RestoreStatus::NotRestored),
        ("uninstall-unix-3", "ffmpeg", RestoreStatus::Restored),
    ]
    .into_iter()
    .flat_map(|(set_id, package_id, status)| {
        [
            ProgressEvent::RestoreStarted {
                set_id: set_id.to_string(),
                package_id: Some(package_id.to_string()),
            },
            ProgressEvent::RestoreCompleted {
                set_id: set_id.to_string(),
                package_id: Some(package_id.to_string()),
                status,
            },
        ]
    })
    .collect();
    assert_eq!(events, expected);
}

/// A restore of a whole set that fails partway, here at its third file
/// before that file changes, puts back the file it replaced from its
/// before-restore set, which then goes again, removes the file it added,
/// and leaves the file that failed untouched. RABBIT's records stay as
/// they were, and the report it saves says so.
#[test]
fn restore_backup_set_is_all_or_nothing_and_saves_its_report() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    save_install_state(&resource, &InstallState::default()).unwrap();
    let receipt = fs::read(resource.join("RABBIT/install-state.json")).unwrap();
    let failing = resource.join("UserPlugins/reapack_third.dll");
    let _hook = restore_seam::install(move |step, path| {
        if step == Step::Restoring && path == failing {
            return Err(std::io::Error::other("the copy broke off"));
        }
        Ok(())
    });

    let report = restore_backup_set_within(
        &resource,
        "uninstall-unix-2",
        &RestoreBackupOptions {
            dry_run: false,
            allow_reaper_running: true,
            package_id: None,
        },
        &OutsideRestoreRules::default(),
    )
    .unwrap();

    assert_eq!(report.status, RestoreStatus::NotRestored);
    let kinds: Vec<RestoreBackupActionKind> =
        report.actions.iter().map(|action| action.action).collect();
    assert_eq!(
        kinds,
        [
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::NotRestored
        ]
    );
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_reapack.dll")).unwrap(),
        b"newer reapack"
    );
    assert!(!resource.join("Data").exists());
    assert_eq!(
        fs::read(resource.join("UserPlugins/reapack_third.dll")).unwrap(),
        b"newer third"
    );
    assert_eq!(
        fs::read(resource.join("RABBIT/install-state.json")).unwrap(),
        receipt
    );
    assert_eq!(report.before_restore_set_id, None);
    assert_eq!(
        sets(&resource),
        ["uninstall-unix-1", "uninstall-unix-2", "uninstall-unix-3"]
    );
    let saved = report.saved_report.clone().expect("a saved report");
    let envelope: crate::report::SavedJsonReport =
        serde_json::from_str(&fs::read_to_string(&saved.json_path).unwrap()).unwrap();
    assert_eq!(envelope.report["status"], "not-restored");
    assert!(
        envelope.report["error"]
            .as_str()
            .is_some_and(|error| error.contains("the copy broke off")),
        "{:#}",
        envelope.report
    );
}

/// When RABBIT's records can't be saved once every file is back, every
/// file is put back as it was, and the records stay as they were: the
/// backup isn't restored. REAPER's language has a test of its own.
#[test]
fn records_that_cant_be_saved_put_every_file_back() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let _hook = restore_seam::install(|step, _| {
        if step == Step::SavingRecords {
            return Err(std::io::Error::other("the records can't be written"));
        }
        Ok(())
    });

    let report = run(
        &resource,
        &[item("uninstall-unix-2", "reapack")],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    let reapack = &report.backups[0];
    assert_eq!(reapack.status, RestoreStatus::NotRestored);
    assert_eq!(
        kinds(reapack),
        [
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::PutBack
        ]
    );
    assert!(reapack.packages.is_empty());
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_reapack.dll")).unwrap(),
        b"newer reapack"
    );
    assert_eq!(
        fs::read(resource.join("UserPlugins/reapack_third.dll")).unwrap(),
        b"newer third"
    );
    assert!(!resource.join("Data").exists());
    assert!(load_install_state(&resource).unwrap().is_none());
    assert_eq!(reapack.before_restore_set_id, None);
    assert_eq!(
        sets(&resource),
        ["uninstall-unix-1", "uninstall-unix-2", "uninstall-unix-3"]
    );
}

/// A file that can't be put back as it was is left as the backup had it,
/// and named: the backup has files left, its before-restore set stays with
/// the only copy of the file it replaced, and the report names that set
/// and that copy. The file it added goes again; RABBIT's records stay.
#[test]
fn a_file_that_cant_go_back_is_left_and_named() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let first = resource.join("UserPlugins/reaper_reapack.dll");
    let third = resource.join("UserPlugins/reapack_third.dll");
    let (held, failing) = (first.clone(), third.clone());
    let _hook = restore_seam::install(move |step, path| match step {
        Step::Restoring if path == failing => Err(std::io::Error::other("the copy broke off")),
        Step::PuttingBack if path == held => Err(std::io::Error::other("it is in use")),
        _ => Ok(()),
    });

    let report = run(
        &resource,
        &[item("uninstall-unix-2", "reapack")],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    let reapack = &report.backups[0];
    assert_eq!(reapack.status, RestoreStatus::FilesLeft);
    assert_eq!(
        kinds(reapack),
        [
            RestoreBackupActionKind::Left,
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::NotRestored
        ]
    );
    let left = &reapack.actions[0];
    assert!(
        left.error
            .as_deref()
            .is_some_and(|error| error.contains("it is in use")),
        "{left:?}"
    );
    assert_eq!(fs::read(&first).unwrap(), b"old reapack");
    let copy = left.current_backup_path.clone().expect("the replaced copy");
    assert_eq!(fs::read(&copy).unwrap(), b"newer reapack");
    let set_id = reapack
        .before_restore_set_id
        .clone()
        .expect("the set that keeps it");
    let set = resource.join("RABBIT/backups").join(&set_id);
    assert!(copy.starts_with(&set), "{copy:?}");
    assert!(load_backup_manifest(&set).unwrap().is_some());
    assert!(!resource.join("Data").exists());
    assert_eq!(fs::read(&third).unwrap(), b"newer third");
    assert!(reapack.actions[2].current_backup_path.is_none());
    assert!(load_install_state(&resource).unwrap().is_none());
    let json = saved(&report);
    assert_eq!(json["backups"][0]["status"], "files-left");
    assert_eq!(json["backups"][0]["actions"][0]["action"], "left");
    assert_eq!(json["stopped"], false);
}

/// The run reads its stop between backups: the backup being restored
/// finishes, those after it aren't reached and stay as they were, and the
/// report, which says the run was stopped, is saved. A stop before the
/// first backup reaches none. A real run hashes every copy, also when it
/// is asked for the quick plan.
#[test]
fn a_stop_between_backups_leaves_the_rest_as_they_were() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let items = [
        item("uninstall-unix-1", "sws"),
        item("uninstall-unix-3", "ffmpeg"),
    ];
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let progress = ProgressReporter::new(move |event| {
        // Pressed while the first backup runs.
        if matches!(event, ProgressEvent::RestoreStarted { .. }) {
            stop.cancel();
        }
        sink.lock().unwrap().push(event);
    });

    let hashed = crate::hash::hashes_on_this_thread();
    let report = run(
        &resource,
        &items,
        &RestoreRunOptions {
            quick: true,
            ..APPLY
        },
        &progress,
        &cancel,
    );

    assert!(crate::hash::hashes_on_this_thread() > hashed);
    assert!(!report.quick);
    assert_eq!(
        statuses(&report),
        [RestoreStatus::Restored, RestoreStatus::NotReached]
    );
    assert!(report.stopped);
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_sws.dll")).unwrap(),
        b"sws"
    );
    assert!(!resource.join("UserPlugins/avcodec.dll").exists());
    assert_eq!(events.lock().unwrap().len(), 2);
    let json = saved(&report);
    assert_eq!(json["stopped"], true);
    assert_eq!(json["backups"][1]["status"], "not-reached");
    let state = load_install_state(&resource).unwrap().unwrap();
    assert!(!state.packages.contains_key("ffmpeg"));

    // Stopped before it starts: nothing changes, and the report says so.
    let before = snapshot(&resource.join("UserPlugins"));
    let report = run(
        &resource,
        &[item("uninstall-unix-3", "ffmpeg")],
        &APPLY,
        &ProgressReporter::noop(),
        &cancel,
    );
    assert_eq!(statuses(&report), [RestoreStatus::NotReached]);
    assert!(report.stopped);
    assert_eq!(snapshot(&resource.join("UserPlugins")), before);
    saved(&report);
}

/// A set whose fingerprint differs from the one its item names is refused
/// alone, before anything of it changes, and the other backups come back:
/// one changed before the run, and one changed while the backup before it
/// was restored, which is found under the lock, right before its turn.
#[test]
fn a_set_changed_since_listed_is_refused_alone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let overview = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let listed = |set_id: &str, package_id: &str| RestoreItem {
        fingerprint: overview
            .sets
            .iter()
            .find(|set| set.id == set_id)
            .and_then(|set| set.fingerprint.clone()),
        ..item(set_id, package_id)
    };
    let items = [
        listed("uninstall-unix-1", "sws"),
        listed("uninstall-unix-2", "reapack"),
        listed("uninstall-unix-3", "ffmpeg"),
    ];
    assert!(items.iter().all(|item| item.fingerprint.is_some()));
    // A longer manifest is another fingerprint, whatever the clock says.
    let change = |set: &Path| {
        let mut manifest = load_backup_manifest(set).unwrap().unwrap();
        manifest.rabbit_version.push_str("-changed");
        save_backup_manifest(set, &manifest).unwrap();
    };
    let backups = resource.join("RABBIT/backups");
    change(&backups.join("uninstall-unix-2"));
    let (sws, ffmpeg_set) = (
        resource.join("UserPlugins/reaper_sws.dll"),
        backups.join("uninstall-unix-3"),
    );
    let _hook = restore_seam::install(move |step, path| {
        if step == Step::Restoring && path == sws {
            let mut manifest = load_backup_manifest(&ffmpeg_set).unwrap().unwrap();
            manifest.rabbit_version.push_str("-changed");
            save_backup_manifest(&ffmpeg_set, &manifest).unwrap();
        }
        Ok(())
    });

    let report = run(
        &resource,
        &items,
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert_eq!(
        statuses(&report),
        [
            RestoreStatus::Restored,
            RestoreStatus::NotRestored,
            RestoreStatus::NotRestored
        ]
    );
    for (backup, set_id) in report.backups[1..]
        .iter()
        .zip(["uninstall-unix-2", "uninstall-unix-3"])
    {
        assert_eq!(
            backup.refusal,
            Some(RestoreRefusal::ChangedSinceListed {
                backup_id: set_id.to_string()
            })
        );
        assert!(backup.actions.is_empty());
    }
    assert!(!resource.join("UserPlugins/avcodec.dll").exists());
    assert!(!resource.join("Data").exists());
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_reapack.dll")).unwrap(),
        b"newer reapack"
    );
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages.keys().collect::<Vec<_>>(), ["sws"]);
    assert_eq!(
        saved(&report)["backups"][1]["refusal"]["code"],
        "changed-since-listed"
    );
}

/// The quick plan reads no file's content, and says what a full dry run
/// says wherever content isn't needed: the same statuses, refusals,
/// targets and copies of what would be replaced. Neither changes anything
/// nor saves a report. Its targets are those the overview lists.
#[test]
fn the_quick_plan_hashes_nothing_and_agrees_with_a_full_dry_run() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    // ffmpeg was installed again since its uninstall.
    let mut state = InstallState::default();
    let mut again = receipt_entry("ffmpeg");
    again.installed_at = Some("unix-2".to_string());
    state.packages.insert("ffmpeg".to_string(), again);
    save_install_state(&resource, &state).unwrap();
    // A set without a manifest, whose copy is read only by a full plan.
    write(
        &resource.join("RABBIT/backups/unix-5/UserPlugins/loose.dll"),
        b"loose",
    );
    // A set that would put a file back into RABBIT's own folder.
    let notes = resource.join("RABBIT").join("notes.txt");
    write(&notes, b"notes");
    BackupSetWriter::new(&resource, "restore-unix-6-before-restore", "before-restore")
        .copy_file(None, &notes)
        .unwrap();
    // ReaPack's backup is missing a copy it says it finished.
    fs::remove_file(resource.join("RABBIT/backups/uninstall-unix-2/UserPlugins/reapack_third.dll"))
        .unwrap();
    let whole = RestoreItem::whole_set;
    let items = [
        item("uninstall-unix-1", "sws"),
        item("uninstall-unix-2", "reapack"),
        item("uninstall-unix-3", "ffmpeg"),
        whole("unix-5"),
        whole("restore-unix-6-before-restore"),
        whole("unix-404"),
    ];
    let before = snapshot(dir.path());

    let hashed = crate::hash::hashes_on_this_thread();
    let quick = run(
        &resource,
        &items,
        &RestoreRunOptions {
            dry_run: true,
            quick: true,
            allow_reaper_running: true,
        },
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    assert_eq!(crate::hash::hashes_on_this_thread(), hashed);
    let full = run(
        &resource,
        &items,
        &RestoreRunOptions {
            dry_run: true,
            quick: false,
            allow_reaper_running: true,
        },
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    assert!(crate::hash::hashes_on_this_thread() > hashed);
    assert_eq!(snapshot(dir.path()), before);
    assert!(quick.quick && !full.quick);
    assert_eq!(quick.saved_report, None);
    assert_eq!(full.saved_report, None);

    assert_eq!(
        statuses(&quick),
        [
            RestoreStatus::WouldRestore,
            RestoreStatus::NotRestored,
            RestoreStatus::NotRestored,
            RestoreStatus::WouldRestore,
            RestoreStatus::NotRestored,
            RestoreStatus::NotRestored
        ]
    );
    assert_eq!(statuses(&quick), statuses(&full));
    let codes: Vec<Option<&str>> = quick
        .backups
        .iter()
        .map(|backup| backup.refusal.as_ref().map(RestoreRefusal::code))
        .collect();
    assert_eq!(
        codes,
        [
            None,
            Some("part-missing"),
            Some("reinstalled-since"),
            None,
            Some("target-refused"),
            Some("not-found")
        ]
    );
    assert!(matches!(
        &quick.backups[4].refusal,
        Some(RestoreRefusal::TargetRefused {
            reason: TargetRefusal::RabbitFolder,
            ..
        })
    ));
    for (quick, full) in quick.backups.iter().zip(&full.backups) {
        assert_eq!(quick.refusal, full.refusal);
        assert_eq!(quick.skipped_targets, full.skipped_targets);
        assert_eq!(quick.packages, full.packages);
        let places = |backup: &RestoreReportItem| -> Vec<(PathBuf, PathBuf, Option<PathBuf>)> {
            backup
                .actions
                .iter()
                .map(|action| {
                    (
                        action.source_path.clone(),
                        action.target_path.clone(),
                        action.current_backup_path.clone(),
                    )
                })
                .collect()
        };
        assert_eq!(places(quick), places(full));
    }
    // A copy's hash comes from the manifest in the quick plan, and isn't
    // known without one.
    assert_eq!(
        quick.backups[0].actions[0].sha256,
        full.backups[0].actions[0].sha256
    );
    assert_eq!(quick.backups[3].actions[0].sha256, "");
    assert!(!full.backups[3].actions[0].sha256.is_empty());

    let overview = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let mut targets: Vec<PathBuf> = quick.backups[0]
        .actions
        .iter()
        .map(|action| action.target_path.clone())
        .collect();
    targets.sort();
    assert_eq!(
        overview.restore_targets(&BackupItem::Package {
            set_id: "uninstall-unix-1".to_string(),
            package_id: "sws".to_string(),
        }),
        targets
    );
}

/// A check that holds for the whole run, here another RABBIT holding the
/// lock, refuses it before anything changes, and saves no report. So does
/// a refused restore of one backup on its own. A run whose every backup is
/// refused by its own checks reports each, takes no lock, changes nothing
/// and saves no report, as does a run with no backup, and one whose
/// `RABBIT` is a link, which it never writes through.
#[test]
fn a_whole_run_refusal_changes_nothing_and_saves_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let items = [
        item("uninstall-unix-1", "sws"),
        item("uninstall-unix-3", "ffmpeg"),
    ];
    let held = crate::lock::acquire_package_install_lock(&resource).unwrap();
    let before = snapshot(dir.path());

    let error = restore_backups_with(
        &resource,
        &items,
        &APPLY,
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(
        matches!(refusal(&error), RestoreRefusal::LockHeld { .. }),
        "{error:?}"
    );
    assert_eq!(snapshot(dir.path()), before);
    drop(held);

    // Each backup refused on its own: the run reports both and restores
    // nothing, so it saves nothing either.
    let before = snapshot(dir.path());
    let report = run(
        &resource,
        &[item("uninstall-unix-1", "osara"), item("unix-404", "sws")],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    let codes: Vec<&str> = report
        .backups
        .iter()
        .map(|backup| backup.refusal.as_ref().unwrap().code())
        .collect();
    assert_eq!(codes, ["package-not-in-set", "not-found"]);
    assert!(report.preflight.checks.is_empty());
    assert_eq!(report.saved_report, None);
    let none = run(
        &resource,
        &[],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    assert!(none.backups.is_empty());
    assert_eq!(none.saved_report, None);
    assert_eq!(snapshot(dir.path()), before);
    assert!(!resource.join("RABBIT/logs").exists());
    assert!(matches!(
        restore_backup_set_within(
            &resource,
            "unix-404",
            &RestoreBackupOptions::default(),
            &OutsideRestoreRules::default()
        ),
        Err(RabbitError::RestoreRefused(RestoreRefusal::NotFound { .. }))
    ));

    // A linked RABBIT refuses every backup, as a link, and nothing is
    // written through it.
    let rabbit = resource.join("RABBIT");
    let moved = dir.path().join("moved-rabbit");
    fs::rename(&rabbit, &moved).unwrap();
    link_folder(&rabbit, &moved);
    let before = snapshot(&moved);
    let report = run(
        &resource,
        &items,
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    assert!(
        report
            .backups
            .iter()
            .all(|backup| backup.refusal.as_ref().map(RestoreRefusal::code) == Some("is-link")),
        "{report:?}"
    );
    assert_eq!(report.saved_report, None);
    assert_eq!(snapshot(&moved), before);
    assert!(!moved.join("logs").exists());
}

/// On Windows, a file another program holds open with no sharing fails
/// the backup before that file changes: the file the restore replaced
/// before it goes back, and so does everything else.
#[cfg(windows)]
#[test]
fn a_file_held_open_fails_the_backup_and_what_it_replaced_goes_back() {
    use std::os::windows::fs::OpenOptionsExt as _;

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let third = resource.join("UserPlugins/reapack_third.dll");
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&third)
        .unwrap();

    let report = run(
        &resource,
        &[item("uninstall-unix-2", "reapack")],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    drop(held);

    let reapack = &report.backups[0];
    assert_eq!(reapack.status, RestoreStatus::NotRestored, "{reapack:?}");
    assert_eq!(
        kinds(reapack),
        [
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::NotRestored
        ]
    );
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_reapack.dll")).unwrap(),
        b"newer reapack"
    );
    assert_eq!(fs::read(&third).unwrap(), b"newer third");
    assert!(!resource.join("Data").exists());
    assert_eq!(reapack.before_restore_set_id, None);
    assert!(load_install_state(&resource).unwrap().is_none());
    saved(&report);
}

/// On Unix, a folder RABBIT can't write to fails the backup before its
/// file there changes, and what the restore changed before goes back.
/// Root may write anywhere, so then there is nothing to test.
#[cfg(unix)]
#[test]
fn a_folder_that_cant_be_written_fails_the_backup_and_what_it_replaced_goes_back() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    uninstalled(
        &resource,
        "uninstall-unix-2",
        "reapack",
        &[
            ("UserPlugins/reaper_reapack.dll", b"old reapack"),
            ("Locked/reapack.ini", b"old settings"),
        ],
    );
    write(
        &resource.join("UserPlugins/reaper_reapack.dll"),
        b"newer reapack",
    );
    let locked = resource.join("Locked");
    write(&locked.join("reapack.ini"), b"newer settings");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(locked.join("probe"), b"").is_ok() {
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let report = run(
        &resource,
        &[item("uninstall-unix-2", "reapack")],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    let reapack = &report.backups[0];
    assert_eq!(reapack.status, RestoreStatus::NotRestored, "{reapack:?}");
    assert_eq!(
        kinds(reapack),
        [
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::NotRestored
        ]
    );
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_reapack.dll")).unwrap(),
        b"newer reapack"
    );
    assert_eq!(
        fs::read(locked.join("reapack.ini")).unwrap(),
        b"newer settings"
    );
    assert_eq!(reapack.before_restore_set_id, None);
    saved(&report);
}

/// A file whose old copy was removed when the restore failed at it, and
/// that can't be put back either, is left missing, not as the backup had
/// it: nothing is at its place, and the report names the copy of the file
/// it replaced, in the before-restore set, which stays.
#[test]
fn a_file_that_fails_after_its_old_copy_went_and_cant_be_put_back_is_missing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let third = resource.join("UserPlugins/reapack_third.dll");
    let failing = third.clone();
    let _hook = restore_seam::install(move |step, path| match step {
        Step::Replacing if path == failing => Err(std::io::Error::other("the disk is full")),
        Step::PuttingBack if path == failing => Err(std::io::Error::other("it is in use")),
        _ => Ok(()),
    });

    let report = run(
        &resource,
        &[item("uninstall-unix-2", "reapack")],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    let reapack = &report.backups[0];
    assert_eq!(reapack.status, RestoreStatus::FilesLeft);
    assert_eq!(
        kinds(reapack),
        [
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::Missing
        ]
    );
    let missing = &reapack.actions[2];
    assert!(
        missing
            .error
            .as_deref()
            .is_some_and(|error| error.contains("it is in use")),
        "{missing:?}"
    );
    assert!(fs::symlink_metadata(&third).is_err());
    let copy = missing
        .current_backup_path
        .clone()
        .expect("the replaced copy");
    assert_eq!(fs::read(&copy).unwrap(), b"newer third");
    let set_id = reapack
        .before_restore_set_id
        .clone()
        .expect("the set that keeps it");
    assert!(
        copy.starts_with(resource.join("RABBIT/backups").join(&set_id)),
        "{copy:?}"
    );
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_reapack.dll")).unwrap(),
        b"newer reapack"
    );
    assert!(!resource.join("Data").exists());
    assert!(load_install_state(&resource).unwrap().is_none());
    let json = saved(&report);
    assert_eq!(json["backups"][0]["status"], "files-left");
    assert_eq!(json["backups"][0]["actions"][2]["action"], "missing");
}

/// Each backup's set is found again under the lock, right before its
/// turn: a set that became a symbolic link or junction while the backup
/// before it was restored is refused as one, alone, before anything is
/// read through it, and the backup before it stays restored.
#[test]
fn a_set_that_became_a_link_during_the_run_is_refused_alone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let ffmpeg_set = resource
        .join("RABBIT")
        .join("backups")
        .join("uninstall-unix-3");
    let moved = dir.path().join("moved-set");
    let sws = resource.join("UserPlugins/reaper_sws.dll");
    let (set, to) = (ffmpeg_set.clone(), moved.clone());
    let _hook = restore_seam::install(move |step, path| {
        if step == Step::Restoring && path == sws {
            fs::rename(&set, &to)?;
            link_folder(&set, &to);
        }
        Ok(())
    });

    let report = run(
        &resource,
        &[
            item("uninstall-unix-1", "sws"),
            item("uninstall-unix-3", "ffmpeg"),
        ],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert_eq!(
        statuses(&report),
        [RestoreStatus::Restored, RestoreStatus::NotRestored]
    );
    assert_eq!(
        report.backups[1].refusal,
        Some(RestoreRefusal::IsLink { path: ffmpeg_set })
    );
    assert!(report.backups[1].actions.is_empty());
    assert!(!resource.join("UserPlugins/avcodec.dll").exists());
    assert_eq!(
        fs::read(moved.join("UserPlugins/avcodec.dll")).unwrap(),
        b"ffmpeg"
    );
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages.keys().collect::<Vec<_>>(), ["sws"]);
    saved(&report);
}

/// A before-restore set that a link turned up in while the restore put
/// its files back is kept whole: no copy in it is removed through a
/// symbolic link or junction, whether `RABBIT/backups` became one or a
/// folder inside the set did. The file the restore replaced still goes
/// back, checked against the hash the set recorded, and the report names
/// the set and the copy.
#[test]
fn a_before_restore_set_behind_a_link_is_kept() {
    for inside_the_set in [false, true] {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        three_uninstalls(&resource);
        let backups = resource.join("RABBIT").join("backups");
        let moved = dir.path().join("moved");
        let second = resource.join("Data/ReaPack/cache/index.xml");
        let (from_backups, to) = (backups.clone(), moved.clone());
        let _hook = restore_seam::install(move |step, path| {
            if step == Step::Restoring && path == second {
                let link = if inside_the_set {
                    fs::read_dir(&from_backups)?
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .find(|set| set.to_string_lossy().ends_with("-before-restore"))
                        .expect("the before-restore set")
                        .join("UserPlugins")
                } else {
                    from_backups.clone()
                };
                fs::rename(&link, &to)?;
                link_folder(&link, &to);
                return Err(std::io::Error::other("the copy broke off"));
            }
            Ok(())
        });

        let report = run(
            &resource,
            &[item("uninstall-unix-2", "reapack")],
            &APPLY,
            &ProgressReporter::noop(),
            &CancelToken::new(),
        );

        let reapack = &report.backups[0];
        assert_eq!(reapack.status, RestoreStatus::NotRestored, "{reapack:?}");
        assert_eq!(
            kinds(reapack),
            [
                RestoreBackupActionKind::PutBack,
                RestoreBackupActionKind::NotRestored,
                RestoreBackupActionKind::NotRestored
            ]
        );
        assert_eq!(
            fs::read(resource.join("UserPlugins/reaper_reapack.dll")).unwrap(),
            b"newer reapack"
        );
        let set_id = reapack
            .before_restore_set_id
            .clone()
            .expect("the set, kept");
        let copy = if inside_the_set {
            moved.join("reaper_reapack.dll")
        } else {
            moved
                .join(&set_id)
                .join("UserPlugins")
                .join("reaper_reapack.dll")
        };
        assert_eq!(
            fs::read(&copy).unwrap(),
            b"newer reapack",
            "{inside_the_set}"
        );
        assert!(reapack.actions[0].current_backup_path.is_some());
        saved(&report);
    }
}

/// REAPER's language comes back only with a backup that is restored:
/// neither a backup that failed at a file nor one whose records couldn't
/// be saved puts its `langpack=` value back. So the line still says `<>`
/// when a later backup in the same run is restored, and that backup's
/// value goes back.
#[test]
fn the_language_comes_back_only_with_a_restored_backup() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    for (set_id, package_id, file, language) in [
        (
            "uninstall-unix-1",
            "reapack",
            "UserPlugins/reaper_reapack.dll",
            "first.ReaperLangPack",
        ),
        (
            "uninstall-unix-2",
            "sws",
            "UserPlugins/reaper_sws.dll",
            "second.ReaperLangPack",
        ),
        (
            "uninstall-unix-3",
            "ffmpeg",
            "UserPlugins/avcodec.dll",
            "third.ReaperLangPack",
        ),
    ] {
        let record = BackupPackageRecord {
            receipt: Some(receipt_entry(package_id)),
            langpack_before: Some(language.to_string()),
            with_settings: None,
        };
        uninstalled_with(
            &resource,
            set_id,
            package_id,
            record,
            &[(file, package_id.as_bytes())],
        );
    }
    write(
        &resource.join(crate::reaper_ini::REAPER_INI_RELATIVE_PATH),
        b"[REAPER]\nlangpack=<>\n",
    );
    let failing = resource.join("UserPlugins/reaper_reapack.dll");
    let mut saves = 0;
    let _hook = restore_seam::install(move |step, path| match step {
        Step::Restoring if path == failing => Err(std::io::Error::other("the copy broke off")),
        Step::SavingRecords => {
            saves += 1;
            if saves == 1 {
                Err(std::io::Error::other("the records can't be written"))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    });

    let report = run(
        &resource,
        &[
            item("uninstall-unix-1", "reapack"),
            item("uninstall-unix-2", "sws"),
            item("uninstall-unix-3", "ffmpeg"),
        ],
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert_eq!(
        statuses(&report),
        [
            RestoreStatus::NotRestored,
            RestoreStatus::NotRestored,
            RestoreStatus::Restored
        ]
    );
    assert!(report.backups[0].packages.is_empty());
    assert!(report.backups[1].packages.is_empty());
    let ffmpeg = &report.backups[2].packages[0];
    assert_eq!(
        ffmpeg.langpack_before.as_deref(),
        Some("third.ReaperLangPack")
    );
    assert!(ffmpeg.langpack_restored);
    assert_eq!(
        lang_pack_setting(&resource).unwrap().as_deref(),
        Some("third.ReaperLangPack")
    );
}

/// Each backup is checked against RABBIT's records as the backups before
/// it in the run left them. Two backups of SWS, from two uninstalls whose
/// entries differ: once the first is restored, its entry is SWS's, so the
/// second would put its files under another entry and is refused alone. A
/// dry run changes no records, so it checks both against the records as
/// they are, and both would restore.
#[test]
fn each_backup_is_checked_against_the_records_the_ones_before_it_left() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    for (set_id, installed_at, contents) in [
        ("uninstall-unix-1", "unix-1", b"first sws".as_slice()),
        ("uninstall-unix-2", "unix-2", b"second sws".as_slice()),
    ] {
        let mut receipt = receipt_entry("sws");
        receipt.installed_at = Some(installed_at.to_string());
        let record = BackupPackageRecord {
            receipt: Some(receipt),
            langpack_before: None,
            with_settings: None,
        };
        uninstalled_with(
            &resource,
            set_id,
            "sws",
            record,
            &[("UserPlugins/reaper_sws.dll", contents)],
        );
    }
    let items = [
        item("uninstall-unix-1", "sws"),
        item("uninstall-unix-2", "sws"),
    ];

    let dry_run = run(
        &resource,
        &items,
        &RestoreRunOptions {
            dry_run: true,
            ..APPLY
        },
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    assert_eq!(
        statuses(&dry_run),
        [RestoreStatus::WouldRestore, RestoreStatus::WouldRestore]
    );

    let report = run(
        &resource,
        &items,
        &APPLY,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    assert_eq!(
        statuses(&report),
        [RestoreStatus::Restored, RestoreStatus::NotRestored]
    );
    assert_eq!(
        report.backups[1].refusal.as_ref().map(RestoreRefusal::code),
        Some("reinstalled-since")
    );
    assert_eq!(
        fs::read(resource.join("UserPlugins/reaper_sws.dll")).unwrap(),
        b"first sws"
    );
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(
        state.packages["sws"].installed_at.as_deref(),
        Some("unix-1")
    );
}

/// SWS as this platform's package manifests describe it, with `program`
/// as a program its uninstall needs closed, as the only package a test's
/// computer knows.
#[cfg(any(windows, target_os = "macos"))]
pub(super) fn sws_needing_closed(program: &str) -> Vec<crate::package::PackageSpec> {
    let platform = crate::model::Platform::current().expect("tests run on Windows or macOS");
    let mut spec = crate::package::builtin_package_specs(platform)
        .into_iter()
        .find(|spec| spec.id == "sws")
        .unwrap();
    for plan in &mut spec.uninstall.plans {
        plan.blocking_programs = vec![program.to_string()];
    }
    vec![spec]
}

/// The program [`sws_needing_closed`] names, running.
#[cfg(any(windows, target_os = "macos"))]
pub(super) fn demo_tool() -> crate::preflight::RunningProcess {
    crate::preflight::RunningProcess {
        pid: "300".to_string(),
        name: "demo tool.exe".to_string(),
        executable_path: None,
    }
}

/// A program that only a later backup's package needs closed refuses the
/// whole run before anything changes, the earlier backup's file included,
/// and the run saves no report: when it runs at the checks before the
/// lock, and when it starts only before the scan under the lock.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn a_program_one_backup_needs_closed_refuses_the_whole_run() {
    let known = sws_needing_closed("Demo Tool.exe");
    let specs = move |_: crate::model::Platform| known.clone();
    for found_before_the_lock in [true, false] {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        three_uninstalls(&resource);
        let scans = std::cell::Cell::new(0);
        let running = |_: &[String]| {
            scans.set(scans.get() + 1);
            if found_before_the_lock || scans.get() > 1 {
                vec![demo_tool()]
            } else {
                Vec::new()
            }
        };
        let environment = BackupEnvironment {
            named_processes: &running,
            package_specs: &specs,
            ..BackupEnvironment::AS_USER
        };
        let before = snapshot(dir.path());

        let error = restore_backups_with(
            &resource,
            &[
                item("uninstall-unix-3", "ffmpeg"),
                item("uninstall-unix-1", "sws"),
            ],
            &APPLY,
            &environment,
            OutsideRestoreRules::default,
            &ProgressReporter::noop(),
            &CancelToken::new(),
        )
        .unwrap_err();

        assert!(
            matches!(refusal(&error), RestoreRefusal::ProgramRunning {
                package_ids,
                programs,
                ..
            } if package_ids == &["sws"] && programs == &["demo tool.exe"]),
            "{found_before_the_lock}: {error:?}"
        );
        assert_eq!(scans.get(), if found_before_the_lock { 1 } else { 2 });
        assert_eq!(snapshot(dir.path()), before);
        assert!(!resource.join("RABBIT/logs").exists());
    }
}

/// What runs is scanned again before each backup after the first, for
/// that backup's own packages: a program that started while the first
/// backup was restored refuses the next backup that needs it closed, alone,
/// before anything of it changes. The first stays restored, and the run
/// saves its report.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn a_program_started_during_the_run_refuses_the_next_backup_alone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    three_uninstalls(&resource);
    let known = sws_needing_closed("Demo Tool.exe");
    let specs = move |_: crate::model::Platform| known.clone();
    let scans = std::cell::Cell::new(0);
    // Not at the checks before the lock, nor at the scan under it.
    let running = |_: &[String]| {
        scans.set(scans.get() + 1);
        if scans.get() > 2 {
            vec![demo_tool()]
        } else {
            Vec::new()
        }
    };
    let environment = BackupEnvironment {
        named_processes: &running,
        package_specs: &specs,
        ..BackupEnvironment::AS_USER
    };

    let report = restore_backups_with(
        &resource,
        &[
            item("uninstall-unix-3", "ffmpeg"),
            item("uninstall-unix-1", "sws"),
        ],
        &APPLY,
        &environment,
        OutsideRestoreRules::default,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(
        statuses(&report),
        [RestoreStatus::Restored, RestoreStatus::NotRestored]
    );
    assert!(
        matches!(&report.backups[1].refusal, Some(RestoreRefusal::ProgramRunning {
            package_ids,
            ..
        }) if package_ids == &["sws"]),
        "{report:?}"
    );
    assert_eq!(scans.get(), 3);
    assert_eq!(
        fs::read(resource.join("UserPlugins/avcodec.dll")).unwrap(),
        b"ffmpeg"
    );
    assert!(!resource.join("UserPlugins/reaper_sws.dll").exists());
    saved(&report);
}
