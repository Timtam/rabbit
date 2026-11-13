//! Tests of deleting whole backup sets: the rename and then the walk,
//! links removed as links, unfinished deletions, the checks that refuse a
//! whole run before anything changes, and the report every real run saves.
//! Temporary folders only.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tempfile::tempdir;

use super::delete_seam::{self, Step};
use super::{
    DeleteGuard, DeleteItem, DeleteReport, DeleteRunOptions, DeleteStatus, NotDeletedReason,
    delete_backups_with, remove_set_folder,
};
use crate::backup_overview::{BackupItem, BackupSetKind, overview_of_with, overview_with};
use crate::cancel::CancelToken;
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::receipt::PackageReceipt;
use crate::restore_refusal::{RestoreRefusal, refusal};
use crate::rollback::restore_run_tests::snapshot;
use crate::rollback::{
    BackupEnvironment, BackupManifest, BackupPackageRecord, BackupSetWriter, DELETING_DIR,
    KEYMAP_BACKUP_REASON, RECORDS_BACKUP_REASON, save_backup_manifest, set_to_restore,
};
use crate::uninstall::execute::UNINSTALL_BACKUP_REASON;
use crate::uninstall::paths::link_folder;

const APPLY: DeleteRunOptions = DeleteRunOptions { dry_run: false };
const DRY_RUN: DeleteRunOptions = DeleteRunOptions { dry_run: true };

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn set_path(resource: &Path, id: &str) -> PathBuf {
    resource.join("RABBIT").join("backups").join(id)
}

fn deleting_path(resource: &Path, id: &str) -> PathBuf {
    resource.join("RABBIT").join(DELETING_DIR).join(id)
}

/// A set an older RABBIT wrote by hand into `RABBIT/backups/<id>`: the
/// files of `files` at their place in the set, and no manifest.
fn layout_set(resource: &Path, id: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let set = set_path(resource, id);
    fs::create_dir_all(&set).unwrap();
    for (path, contents) in files {
        write(&set.join(path), contents);
    }
    set
}

/// An uninstall's set `id` holding `package_id`, with each of `files`
/// written in the resource folder and moved in. Its backup of the package
/// is the only copy of it.
fn uninstall_set(resource: &Path, id: &str, package_id: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let mut writer = BackupSetWriter::new(resource, id, UNINSTALL_BACKUP_REASON);
    writer
        .record_package(
            package_id,
            BackupPackageRecord {
                receipt: Some(receipt_entry(package_id)),
                langpack_before: None,
                with_settings: Some(false),
            },
        )
        .unwrap();
    for (relative, contents) in files {
        let place = resource.join(relative);
        write(&place, contents);
        writer
            .move_file(Some(package_id), &place, None, false)
            .unwrap();
    }
    writer.path().to_path_buf()
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

/// The set of the key map OSARA's replaced, `id`, holding `reaper-kb.ini`.
fn keymap_set(resource: &Path, id: &str) -> PathBuf {
    let keymap = resource.join("reaper-kb.ini");
    write(&keymap, b"own keys");
    let mut writer = BackupSetWriter::new(resource, id, KEYMAP_BACKUP_REASON);
    writer.copy_file(Some("osara"), &keymap).unwrap();
    writer.path().to_path_buf()
}

/// A set that holds only a copy of RABBIT's records, as older versions
/// made one after vendor installers ran.
fn records_set(resource: &Path, id: &str) -> PathBuf {
    let set = set_path(resource, id);
    let copy = set.join("RABBIT").join("install-state.json");
    write(&copy, b"{}");
    let manifest = BackupManifest {
        schema_version: 1,
        rabbit_version: "0.4.4".to_string(),
        created_at: id.to_string(),
        created_unix: None,
        reason: RECORDS_BACKUP_REASON.to_string(),
        files: Vec::new(),
        receipt_backup_path: Some(copy),
        packages: std::collections::BTreeMap::new(),
        replaced: std::collections::BTreeMap::new(),
        keymap_origin: None,
    };
    save_backup_manifest(&set, &manifest).unwrap();
    set
}

/// The bytes every regular file below `root` holds, found without
/// following any link.
fn file_bytes(root: &Path) -> u64 {
    snapshot(root)
        .iter()
        .filter_map(|(_, contents)| contents.as_ref())
        .map(|contents| contents.len() as u64)
        .sum()
}

fn items(ids: &[&str]) -> Vec<DeleteItem> {
    ids.iter().map(|id| DeleteItem::new(id)).collect()
}

/// Delete `ids` from `resource` as the user, with `options`.
fn run(resource: &Path, ids: &[&str], options: &DeleteRunOptions) -> DeleteReport {
    run_with(
        resource,
        &items(ids),
        options,
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
}

fn run_with(
    resource: &Path,
    items: &[DeleteItem],
    options: &DeleteRunOptions,
    environment: &BackupEnvironment<'_>,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> DeleteReport {
    delete_backups_with(resource, items, options, environment, progress, cancel).unwrap()
}

fn statuses(report: &DeleteReport) -> Vec<DeleteStatus> {
    report.sets.iter().map(|set| set.status).collect()
}

/// Every event `run` reports, and the reporter that collects them.
fn collecting() -> (Arc<Mutex<Vec<ProgressEvent>>>, ProgressReporter) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let reporter = ProgressReporter::new(move |event| sink.lock().unwrap().push(event));
    (events, reporter)
}

/// The report `report` says it saved, as saved, and check its text file is
/// there too.
fn saved(report: &DeleteReport) -> serde_json::Value {
    let saved = report.saved_report.clone().expect("a saved report");
    let name = saved.json_path.file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("delete-backup-unix-"), "{name}");
    assert!(saved.text_path.is_file());
    let envelope: crate::report::SavedJsonReport =
        serde_json::from_str(&fs::read_to_string(&saved.json_path).unwrap()).unwrap();
    envelope.report
}

/// Every file and link below `root` except those in RABBIT's two backup
/// folders and its logs, which hold the report a real run saves.
fn everything_else(resource: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let rabbit = resource.join("RABBIT");
    let ours = [
        rabbit.join("backups"),
        rabbit.join(DELETING_DIR),
        rabbit.join("logs"),
    ];
    snapshot(resource)
        .into_iter()
        .filter(|(path, _)| !ours.iter().any(|folder| path.starts_with(folder)))
        .collect()
}

/// Every file and link below `root`, by its path below it, with what each
/// file holds.
fn relative_snapshot(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    snapshot(root)
        .into_iter()
        .map(|(path, contents)| (path.strip_prefix(root).unwrap().to_path_buf(), contents))
        .collect()
}

/// What a test saw right after a set's rename: where the set went, what it
/// held there, whether it was still in `RABBIT/backups`, and the code a
/// restore of it gave.
type SeenAfterRename = (PathBuf, Vec<(PathBuf, Option<Vec<u8>>)>, bool, String);

/// A delete moves the set into `RABBIT/backups-deleting` first, whole, so
/// no restore finds it any more, and only then walks it, removing every
/// file and folder in it, deepest first, and the set's folder last. The
/// report counts the bytes of the files removed and is saved; the progress
/// hears the set start and end.
#[test]
fn a_whole_set_is_renamed_then_walked_away() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = uninstall_set(
        &resource,
        "uninstall-unix-1",
        "sws",
        &[
            ("UserPlugins/reaper_sws-x64.dll", b"sws plug-in"),
            ("Data/SWS/deep/er/grooves.txt", b"grooves"),
        ],
    );
    let in_set = relative_snapshot(&set);
    let bytes = file_bytes(&set);
    let renamed: Arc<Mutex<Vec<SeenAfterRename>>> = Arc::default();
    let seen = Arc::clone(&renamed);
    let resource_seen = resource.clone();
    let _seam = delete_seam::install(move |step, folder| {
        if step != Step::Renamed {
            return;
        }
        // Right after the rename: the whole set is there, nothing removed
        // yet, it is gone from RABBIT/backups, and a restore says it is
        // being deleted.
        let restore = set_to_restore(&resource_seen, "uninstall-unix-1").unwrap_err();
        seen.lock().unwrap().push((
            folder.to_path_buf(),
            relative_snapshot(folder),
            set_path(&resource_seen, "uninstall-unix-1").exists(),
            refusal(&restore).code().to_string(),
        ));
    });
    let (events, progress) = collecting();

    let report = run_with(
        &resource,
        &items(&["uninstall-unix-1"]),
        &APPLY,
        &BackupEnvironment::AS_USER,
        &progress,
        &CancelToken::new(),
    );

    assert_eq!(
        *renamed.lock().unwrap(),
        [(
            deleting_path(&resource, "uninstall-unix-1"),
            in_set,
            false,
            "being-deleted".to_string()
        )]
    );
    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    let deleted = &report.sets[0];
    assert_eq!(deleted.set_path.as_deref(), Some(set.as_path()));
    assert_eq!(deleted.bytes_freed, bytes);
    assert_eq!(report.bytes_freed, bytes);
    assert_eq!(deleted.size, bytes);
    assert_eq!(
        deleted.only_copy,
        [BackupItem::Package {
            set_id: "uninstall-unix-1".to_string(),
            package_id: "sws".to_string()
        }]
    );
    assert!(!report.refused && !report.stopped);
    assert!(!set.exists());
    assert!(!deleting_path(&resource, "uninstall-unix-1").exists());
    assert_eq!(
        *events.lock().unwrap(),
        [
            ProgressEvent::DeleteStarted {
                set_id: "uninstall-unix-1".to_string()
            },
            ProgressEvent::DeleteCompleted {
                set_id: "uninstall-unix-1".to_string(),
                status: DeleteStatus::Deleted,
                bytes_freed: bytes,
            },
        ]
    );
    let json = saved(&report);
    assert_eq!(json["sets"][0]["status"], "deleted");
    assert_eq!(json["sets"][0]["bytes_freed"], bytes);
    assert_eq!(json["sets"][0]["only_copy"][0]["package_id"], "sws");
    assert!(!resource.join("RABBIT/locks/package-install.lock").exists());
}

/// A symbolic link or junction in a set is removed as a link: what it
/// points to, outside the set, stays exactly as it was, and its bytes
/// don't count as freed. So does a link to a file, where this computer
/// lets a test make one.
#[test]
fn links_are_removed_as_links_and_their_targets_stay() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("precious.dll"), &[7; 4096]);
    write(&elsewhere.join("inner/notes.txt"), b"keep me");
    let set = layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    fs::create_dir_all(set.join("deep")).unwrap();
    link_folder(&set.join("linked-folder"), &elsewhere);
    link_folder(&set.join("deep/linked-again"), &elsewhere.join("inner"));
    let file_link = set.join("linked-file.dll");
    #[cfg(unix)]
    let file_link_made = {
        std::os::unix::fs::symlink(elsewhere.join("precious.dll"), &file_link).unwrap();
        true
    };
    // Windows lets only administrators and developer mode make one.
    #[cfg(windows)]
    let file_link_made =
        std::os::windows::fs::symlink_file(elsewhere.join("precious.dll"), &file_link).is_ok();
    assert!(
        !file_link_made
            || fs::symlink_metadata(&file_link)
                .unwrap()
                .file_type()
                .is_symlink()
    );
    let outside = snapshot(dir.path())
        .into_iter()
        .filter(|(path, _)| path.starts_with(&elsewhere))
        .collect::<Vec<_>>();

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    assert_eq!(report.sets[0].bytes_freed, 1);
    assert!(!set.exists());
    assert!(!deleting_path(&resource, "unix-1").exists());
    assert_eq!(snapshot(&elsewhere), outside);
    assert!(!file_link_made || fs::symlink_metadata(&file_link).is_err());
}

/// A junction or link where RABBIT keeps its deletions could lead
/// anywhere, so a delete refuses every set while it is there, before
/// anything changes, and saves nothing. So does a dry run.
#[test]
fn a_linked_folder_of_deletions_refuses_the_whole_run() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("unix-1/keep.txt"), b"keep");
    fs::create_dir_all(resource.join("RABBIT")).unwrap();
    link_folder(&resource.join("RABBIT").join(DELETING_DIR), &elsewhere);
    let before = snapshot(dir.path());

    for options in [APPLY, DRY_RUN] {
        let report = run(&resource, &["unix-1", "unix-404"], &options);

        assert!(report.refused, "{report:#?}");
        assert_eq!(
            statuses(&report),
            [DeleteStatus::Refused, DeleteStatus::Refused]
        );
        for set in &report.sets {
            assert_eq!(set.refusal.as_ref().unwrap().code(), "is-link");
        }
        assert!(report.saved_report.is_none());
        assert_eq!(snapshot(dir.path()), before);
        assert!(!resource.join("RABBIT/locks").exists());
        assert!(!resource.join("RABBIT/logs").exists());
    }
}

/// A linked `RABBIT/backups`, or `RABBIT`, refuses every item, an id that
/// names nothing included, and nothing behind it is touched.
#[test]
fn a_linked_backups_folder_refuses_everything() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("unix-1/a.dll"), b"a");
    fs::create_dir_all(resource.join("RABBIT")).unwrap();
    link_folder(&resource.join("RABBIT").join("backups"), &elsewhere);
    let before = snapshot(dir.path());

    let report = run(&resource, &["unix-1", "unix-404"], &APPLY);

    assert!(report.refused);
    assert!(
        report
            .sets
            .iter()
            .all(|set| set.status == DeleteStatus::Refused
                && set.refusal.as_ref().unwrap().code() == "is-link"),
        "{report:#?}"
    );
    assert_eq!(snapshot(dir.path()), before);

    // A linked RABBIT the same.
    let other = dir.path().join("Other");
    let rabbit_elsewhere = dir.path().join("RabbitElsewhere");
    write(&rabbit_elsewhere.join("backups/unix-1/a.dll"), b"a");
    fs::create_dir_all(&other).unwrap();
    link_folder(&other.join("RABBIT"), &rabbit_elsewhere);
    let before = snapshot(dir.path());

    let report = run(&other, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Refused]);
    assert_eq!(snapshot(dir.path()), before);
}

/// A copy keeps a file's read-only attribute on Windows, and a folder can
/// have it too. Both are removed: the file by Rust's own removal, which
/// ignores the attribute on this Windows, the folder once RABBIT cleared
/// its attribute.
#[cfg(windows)]
#[test]
fn a_read_only_file_and_folder_are_removed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = layout_set(
        &resource,
        "unix-1",
        &[("locked/read-only.dll", b"read only"), ("b.txt", b"b")],
    );
    for path in [set.join("locked/read-only.dll"), set.join("locked")] {
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
    }

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted], "{report:#?}");
    assert_eq!(report.sets[0].bytes_freed, 10);
    assert!(!set.exists());
    assert!(!deleting_path(&resource, "unix-1").exists());
}

/// Open `path` and keep it open with `share_mode`, as another program
/// would.
#[cfg(windows)]
fn hold_open(path: &Path, share_mode: u32) -> fs::File {
    use std::os::windows::fs::OpenOptionsExt as _;
    fs::OpenOptions::new()
        .read(true)
        .share_mode(share_mode)
        .open(path)
        .unwrap()
}

/// On Windows, a file in the set that another program holds open with no
/// sharing stops the rename: the set isn't deleted, because it is in use,
/// and nothing of it changes. The run still saves its report.
#[cfg(windows)]
#[test]
fn a_file_held_open_without_sharing_leaves_the_set_whole() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = layout_set(&resource, "unix-1", &[("sub/a.dll", b"a"), ("b.txt", b"b")]);
    let before = snapshot(&set);
    let held = hold_open(&set.join("sub/a.dll"), 0);

    let report = run(&resource, &["unix-1"], &APPLY);
    drop(held);

    assert_eq!(statuses(&report), [DeleteStatus::NotDeleted]);
    assert_eq!(
        report.sets[0].not_deleted,
        Some(super::NotDeletedReason::InUse)
    );
    assert_eq!(report.sets[0].bytes_freed, 0);
    assert!(report.sets[0].error.is_some());
    assert_eq!(snapshot(&set), before);
    assert!(!resource.join("RABBIT").join(DELETING_DIR).exists());
    assert_eq!(saved(&report)["sets"][0]["not_deleted"], "in-use");
}

/// The plan expected a file held open with delete sharing, as std opens
/// files, not to stop the rename. Windows 11 refuses to rename a folder
/// with any file in it open, whatever the sharing, so this pins what it
/// does: the set isn't deleted, because it is in use, and nothing of it
/// changes.
#[cfg(windows)]
#[test]
fn a_file_held_open_with_delete_sharing_still_stops_the_rename() {
    const SHARE_READ_WRITE_DELETE: u32 = 0x1 | 0x2 | 0x4;
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = layout_set(&resource, "unix-1", &[("sub/a.dll", b"a"), ("b.txt", b"b")]);
    let before = snapshot(&set);
    let held = hold_open(&set.join("sub/a.dll"), SHARE_READ_WRITE_DELETE);

    let report = run(&resource, &["unix-1"], &APPLY);
    drop(held);

    assert_eq!(statuses(&report), [DeleteStatus::NotDeleted]);
    assert_eq!(
        report.sets[0].not_deleted,
        Some(super::NotDeletedReason::InUse)
    );
    assert_eq!(snapshot(&set), before);
}

/// On Windows, a file another program opens with no sharing after the
/// rename stops the walk at that file: the rest goes, and what is left
/// stays in `RABBIT/backups-deleting`, where the overview lists it as an
/// unfinished deletion. Once the file is closed, naming the id again
/// finishes it, which is no "already gone".
#[cfg(windows)]
#[test]
fn a_file_opened_after_the_rename_leaves_a_partly_deleted_set() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    layout_set(
        &resource,
        "unix-1",
        &[("sub/held.dll", b"held"), ("free.txt", b"free")],
    );
    let held: std::rc::Rc<std::cell::RefCell<Option<fs::File>>> = std::rc::Rc::default();
    let holder = std::rc::Rc::clone(&held);
    let seam = delete_seam::install(move |step, folder| {
        if step == Step::Renamed {
            *holder.borrow_mut() = Some(hold_open(&folder.join("sub/held.dll"), 0));
        }
    });

    let report = run(&resource, &["unix-1"], &APPLY);
    drop(seam);

    assert_eq!(statuses(&report), [DeleteStatus::PartlyDeleted]);
    let left = deleting_path(&resource, "unix-1");
    assert_eq!(report.sets[0].left_in.as_deref(), Some(left.as_path()));
    assert_eq!(report.sets[0].bytes_freed, 4);
    assert!(
        report.sets[0]
            .error
            .as_deref()
            .unwrap()
            .contains("held.dll")
    );
    assert!(!set_path(&resource, "unix-1").exists());
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(listed.sets.len(), 1);
    assert_eq!(listed.sets[0].kind, BackupSetKind::Deleting);
    held.borrow_mut().take();

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::FinishedEarlierDeletion]);
    assert_eq!(report.sets[0].bytes_freed, 4);
    assert!(!left.exists());
}

/// Puts a folder's permissions back when dropped, so the temporary folder
/// can go, wherever the folder is by then.
#[cfg(unix)]
struct ModeBack(Vec<PathBuf>);

#[cfg(unix)]
impl Drop for ModeBack {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt as _;
        for path in &self.0 {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
        }
    }
}

/// Whether the tests run as root, who may remove what permissions forbid.
#[cfg(unix)]
fn running_as_root(dir: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    fs::metadata(dir).unwrap().uid() == 0
}

/// On Unix, a folder in the set that RABBIT may not look into stops the
/// walk there: the rest goes, and what is left stays in
/// `RABBIT/backups-deleting`, listed as an unfinished deletion. Once the
/// folder may be read again, naming the id finishes it, which is no
/// "already gone".
#[cfg(unix)]
#[test]
fn a_folder_without_permission_leaves_a_partly_deleted_set_that_a_later_run_finishes() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempdir().unwrap();
    if running_as_root(dir.path()) {
        return;
    }
    let resource = dir.path().join("REAPER");
    let set = layout_set(
        &resource,
        "unix-1",
        &[("locked/kept.dll", b"kept"), ("free.txt", b"free")],
    );
    let left = deleting_path(&resource, "unix-1");
    let _mode_back = ModeBack(vec![set.join("locked"), left.join("locked")]);
    fs::set_permissions(set.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::PartlyDeleted]);
    assert_eq!(report.sets[0].left_in.as_deref(), Some(left.as_path()));
    assert_eq!(report.sets[0].bytes_freed, 4);
    assert!(report.sets[0].error.is_some());
    assert!(!set.exists());
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(listed.sets.len(), 1);
    assert_eq!(listed.sets[0].kind, BackupSetKind::Deleting);
    assert_eq!(listed.sets[0].path, left);
    assert_eq!(
        listed.entries[0].item,
        BackupItem::Deleting {
            set_id: "unix-1".to_string()
        }
    );
    assert!(saved(&report)["sets"][0]["left_in"].is_string());

    fs::set_permissions(left.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::FinishedEarlierDeletion]);
    assert_eq!(report.sets[0].bytes_freed, 4);
    assert!(!left.exists());
}

/// An id found only in `RABBIT/backups-deleting` names an unfinished
/// deletion: a dry run says a real run would finish it, and a real run
/// does. It never counts as already gone.
#[test]
fn an_unfinished_deletion_named_by_id_is_finished() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let left = deleting_path(&resource, "unix-1");
    write(&left.join("sub/left.dll"), b"left over");
    let before = snapshot(dir.path());

    let report = run(&resource, &["unix-1"], &DRY_RUN);

    assert_eq!(
        statuses(&report),
        [DeleteStatus::WouldFinishEarlierDeletion]
    );
    assert_eq!(
        report.sets[0].unfinished_deletion.as_deref(),
        Some(left.as_path())
    );
    assert_eq!(report.sets[0].size, 9);
    assert_eq!(snapshot(dir.path()), before);

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::FinishedEarlierDeletion]);
    assert!(report.sets[0].finished_earlier_deletion);
    assert_eq!(report.sets[0].bytes_freed, 9);
    assert!(!left.exists());
    assert_eq!(saved(&report)["sets"][0]["finished_earlier_deletion"], true);
}

/// An id that names a set and an unfinished deletion finishes the
/// deletion first, so the set gets a free name, then deletes the set. Two
/// folders are never merged.
#[test]
fn an_id_in_both_folders_finishes_the_earlier_deletion_then_deletes_the_set() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = keymap_set(&resource, "unix-1");
    let left = deleting_path(&resource, "unix-1");
    write(&left.join("left.dll"), b"left");
    let bytes = file_bytes(&set) + 4;

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    let deleted = &report.sets[0];
    assert!(deleted.finished_earlier_deletion);
    assert_eq!(deleted.bytes_freed, bytes);
    assert_eq!(deleted.unfinished_deletion.as_deref(), Some(left.as_path()));
    assert!(!set.exists() && !left.exists());
}

/// When the unfinished deletion beside a set can't be finished, the set
/// stays exactly as it was in `RABBIT/backups`: it has no free name to
/// move to, and two folders are never merged. On Windows a file of the
/// deletion is held open; on Unix a folder in it may not be read.
#[test]
fn an_unfinished_deletion_that_cant_be_finished_leaves_its_set_whole() {
    let dir = tempdir().unwrap();
    #[cfg(unix)]
    {
        if running_as_root(dir.path()) {
            return;
        }
    }
    let resource = dir.path().join("REAPER");
    let set = keymap_set(&resource, "unix-1");
    let left = deleting_path(&resource, "unix-1");
    write(&left.join("locked/left.dll"), b"left");
    let set_before = snapshot(&set);
    #[cfg(windows)]
    let held = hold_open(&left.join("locked/left.dll"), 0);
    #[cfg(unix)]
    let _mode_back = {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(left.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
        ModeBack(vec![left.join("locked")])
    };

    let report = run(&resource, &["unix-1"], &APPLY);
    #[cfg(windows)]
    drop(held);

    assert_eq!(statuses(&report), [DeleteStatus::NotDeleted], "{report:#?}");
    let item = &report.sets[0];
    assert_eq!(
        item.not_deleted,
        Some(NotDeletedReason::EarlierDeletionLeft)
    );
    assert_eq!(item.left_in.as_deref(), Some(left.as_path()));
    assert!(!item.finished_earlier_deletion);
    assert!(item.error.is_some());
    assert_eq!(snapshot(&set), set_before);
    assert!(set_path(&resource, "unix-1").is_dir());
    assert!(report.saved_report.is_some());
}

/// An entry in `RABBIT/backups-deleting` named after the id that is itself
/// a junction or link, leading out of RABBIT's folders, is refused before
/// anything changes, in a dry run too, and what it leads to stays. Handed
/// to the walk directly, it isn't looked into either.
#[test]
fn a_linked_unfinished_deletion_is_refused_and_never_followed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("precious.dll"), b"precious");
    write(&elsewhere.join("inner/notes.txt"), b"keep me");
    layout_set(&resource, "unix-2", &[("a.dll", b"a")]);
    fs::create_dir_all(resource.join("RABBIT").join(DELETING_DIR)).unwrap();
    link_folder(&deleting_path(&resource, "unix-1"), &elsewhere);
    let before = snapshot(dir.path());

    for options in [APPLY, DRY_RUN] {
        let report = run(&resource, &["unix-2", "unix-1"], &options);

        assert!(report.refused, "{report:#?}");
        assert_eq!(report.sets[1].status, DeleteStatus::Refused);
        assert_eq!(report.sets[1].refusal.as_ref().unwrap().code(), "is-link");
        assert!(report.saved_report.is_none());
        assert_eq!(snapshot(dir.path()), before);
    }

    let removal = remove_set_folder(&resource, "unix-1", &deleting_path(&resource, "unix-1"));

    assert!(!removal.complete);
    assert!(removal.first_error.is_some());
    assert_eq!(snapshot(dir.path()), before);
}

/// A folder in a set that another program swaps for a junction or link,
/// leading out of the set, after the walk found it and before the walk
/// lists it, is never followed: it is checked again right before it is
/// listed and left as it is, so what it leads to stays, and the set is
/// partly deleted. A later run removes the link as a link.
#[test]
fn a_folder_swapped_for_a_link_during_the_walk_is_never_followed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("precious.dll"), b"precious");
    write(&elsewhere.join("inner/notes.txt"), b"keep me");
    layout_set(&resource, "unix-1", &[("sub/a.dll", b"a"), ("b.dll", b"b")]);
    let outside = snapshot(&elsewhere);
    let swapped = deleting_path(&resource, "unix-1").join("sub");
    let target = elsewhere.clone();
    let mut first_time = true;
    let _seam = delete_seam::install(move |step, folder| {
        if step == Step::Opening && folder == swapped && std::mem::take(&mut first_time) {
            fs::remove_dir_all(folder).unwrap();
            link_folder(folder, &target);
        }
    });

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(
        statuses(&report),
        [DeleteStatus::PartlyDeleted],
        "{report:#?}"
    );
    assert!(
        report.sets[0]
            .error
            .as_deref()
            .unwrap()
            .contains("no longer a real folder")
    );
    assert_eq!(snapshot(&elsewhere), outside);

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::FinishedEarlierDeletion]);
    assert!(!deleting_path(&resource, "unix-1").exists());
    assert_eq!(snapshot(&elsewhere), outside);
}

/// On Unix, a set RABBIT may not move, because it may not write in
/// `RABBIT/backups`, isn't deleted for lack of permission, and nothing of
/// it changes: the folder of deletions the run made for it goes again.
#[cfg(unix)]
#[test]
fn a_set_rabbit_may_not_move_is_not_deleted_for_permission() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempdir().unwrap();
    if running_as_root(dir.path()) {
        return;
    }
    let resource = dir.path().join("REAPER");
    let set = layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let before = snapshot(&set);
    let backups = resource.join("RABBIT").join("backups");
    let _mode_back = ModeBack(vec![backups.clone()]);
    fs::set_permissions(&backups, fs::Permissions::from_mode(0o555)).unwrap();

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::NotDeleted], "{report:#?}");
    assert_eq!(
        report.sets[0].not_deleted,
        Some(NotDeletedReason::Permission)
    );
    assert_eq!(snapshot(&set), before);
    assert!(!resource.join("RABBIT").join(DELETING_DIR).exists());
    assert_eq!(saved(&report)["sets"][0]["not_deleted"], "permission");
}

/// Something in `RABBIT/backups-deleting` that takes the set's name but
/// isn't an unfinished deletion of the id, such as a file, is never
/// removed or moved onto: the set isn't deleted, nothing changes, and the
/// report names what is in the way instead of asking for another run.
#[test]
fn a_file_with_the_sets_name_among_the_deletions_is_left_alone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let in_the_way = deleting_path(&resource, "unix-1");
    write(&in_the_way, b"not a set");
    let before = snapshot(dir.path());

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::NotDeleted], "{report:#?}");
    let item = &report.sets[0];
    assert_eq!(item.not_deleted, Some(NotDeletedReason::Other));
    assert_eq!(item.left_in, None);
    assert!(
        item.error
            .as_deref()
            .unwrap()
            .contains(&in_the_way.display().to_string())
    );
    assert!(set.is_dir());
    assert_eq!(fs::read(&in_the_way).unwrap(), b"not a set");
    let after: Vec<_> = snapshot(dir.path())
        .into_iter()
        .filter(|(path, _)| !path.starts_with(resource.join("RABBIT/logs")))
        .collect();
    assert_eq!(after, before);
}

/// Windows counts names that differ only in case as the same. An
/// unfinished deletion named so takes the name a set would get, so the set
/// isn't deleted, and the report names that deletion, which the id never
/// names, instead of asking for another run of the same id.
#[cfg(windows)]
#[test]
fn an_unfinished_deletion_in_another_case_leaves_the_set_alone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let other_case = deleting_path(&resource, "UNIX-1");
    write(&other_case.join("left.dll"), b"left");

    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::NotDeleted], "{report:#?}");
    let item = &report.sets[0];
    assert_eq!(item.not_deleted, Some(NotDeletedReason::Other));
    assert_eq!(item.left_in, None);
    assert!(item.error.as_deref().unwrap().contains("UNIX-1"));
    assert!(set.is_dir());
    assert_eq!(fs::read(other_case.join("left.dll")).unwrap(), b"left");
}

/// A set that belongs to another account than `RABBIT/backups`, as one
/// made with sudo on a Mac, refuses the whole run before anything changes,
/// in a dry run too. The others say what a dry run would do with them, and
/// a real run never reaches them. The report says why in a code.
#[test]
fn another_accounts_set_refuses_the_whole_run() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let roots = layout_set(&resource, "unix-2", &[("b.dll", b"b")]);
    let owner_of = |path: &Path| Some(if path == roots { 0 } else { 501 });
    let under_sudo = BackupEnvironment {
        owner_of: &owner_of,
        ..BackupEnvironment::AS_USER
    };
    let before = snapshot(dir.path());

    let report = run_with(
        &resource,
        &items(&["unix-1", "unix-2"]),
        &APPLY,
        &under_sudo,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert!(report.refused);
    assert_eq!(
        statuses(&report),
        [DeleteStatus::NotReached, DeleteStatus::Refused]
    );
    assert!(matches!(
        report.sets[1].refusal,
        Some(RestoreRefusal::NotYours { ref backup_id }) if backup_id == "unix-2"
    ));
    // The refused item names the folder that is another account's.
    assert_eq!(report.sets[1].set_path.as_deref(), Some(roots.as_path()));
    assert_eq!(report.sets[1].unfinished_deletion, None);
    assert_eq!(snapshot(dir.path()), before);
    assert!(report.saved_report.is_none());
    assert!(!resource.join("RABBIT/locks").exists());
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["refused"], true);
    assert_eq!(json["sets"][1]["status"], "refused");
    assert_eq!(json["sets"][1]["refusal"]["code"], "not-yours");
    assert_eq!(json["sets"][0]["status"], "not-reached");

    // A dry run as root still compares with the owner of RABBIT/backups,
    // not with root, so it says the truth about both.
    let as_root_too = BackupEnvironment {
        runs_as_root: &|| true,
        ..under_sudo
    };
    let report = run_with(
        &resource,
        &items(&["unix-1", "unix-2"]),
        &DRY_RUN,
        &as_root_too,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert_eq!(
        statuses(&report),
        [DeleteStatus::WouldDelete, DeleteStatus::Refused]
    );
    assert_eq!(snapshot(dir.path()), before);
}

/// An unfinished deletion that belongs to another account is refused the
/// same way, and the refused item names it as the unfinished deletion, so
/// the CLI can say it is in `RABBIT/backups-deleting`.
#[test]
fn another_accounts_unfinished_deletion_is_named_as_one() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let roots = deleting_path(&resource, "unix-2");
    write(&roots.join("b.dll"), b"b");
    let owner_of = |path: &Path| Some(if path == roots { 0 } else { 501 });
    let under_sudo = BackupEnvironment {
        owner_of: &owner_of,
        ..BackupEnvironment::AS_USER
    };
    let before = snapshot(dir.path());

    let report = run_with(
        &resource,
        &items(&["unix-2"]),
        &APPLY,
        &under_sudo,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert!(report.refused);
    assert_eq!(report.sets[0].refusal.as_ref().unwrap().code(), "not-yours");
    assert_eq!(report.sets[0].set_path, None);
    assert_eq!(
        report.sets[0].unfinished_deletion.as_deref(),
        Some(roots.as_path())
    );
    assert_eq!(snapshot(dir.path()), before);
}

/// On a Mac, a real delete as root is refused before anything else, even
/// before an id that can't be one, and changes nothing. A dry run as root
/// still says what would go. The check is the environment's, so this runs
/// on every platform.
#[test]
fn a_real_delete_as_root_is_refused_first() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let as_root = BackupEnvironment {
        runs_as_root: &|| true,
        ..BackupEnvironment::AS_USER
    };
    let before = snapshot(dir.path());

    let error = delete_backups_with(
        &resource,
        &items(&["unix-1", "a/b"]),
        &APPLY,
        &as_root,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(
        matches!(refusal(&error), RestoreRefusal::AsRoot),
        "{error:?}"
    );
    assert_eq!(snapshot(dir.path()), before);
    assert!(!resource.join("RABBIT/locks").exists());

    let report = run_with(
        &resource,
        &items(&["unix-1"]),
        &DRY_RUN,
        &as_root,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );
    assert_eq!(statuses(&report), [DeleteStatus::WouldDelete]);
}

/// Another RABBIT working on this REAPER holds the lock: the delete is an
/// error before anything changes.
#[test]
fn the_lock_held_refuses_before_any_change() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let held = crate::lock::acquire_package_install_lock(&resource).unwrap();
    let before = snapshot(dir.path());

    let error = delete_backups_with(
        &resource,
        &items(&["unix-1"]),
        &APPLY,
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap_err();

    assert!(
        matches!(refusal(&error), RestoreRefusal::LockHeld { .. }),
        "{error:?}"
    );
    assert_eq!(snapshot(dir.path()), before);
    assert!(!resource.join("RABBIT").join(DELETING_DIR).exists());
    assert!(!resource.join("RABBIT/logs").exists());
    drop(held);
}

/// The sets are checked again under the lock. A listed set that changed
/// between the first checks and the lock refuses the whole run there, as
/// it would before the lock: nothing changes, and no report is saved.
#[test]
fn a_set_changed_under_the_lock_refuses_the_run_and_saves_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let first = layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let second = layout_set(&resource, "unix-2", &[("b.dll", b"b")]);
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let listed_items: Vec<DeleteItem> = ["unix-1", "unix-2"]
        .iter()
        .map(|id| {
            let set = listed.sets.iter().find(|set| set.id == *id).unwrap();
            DeleteItem::listed(id, set.fingerprint.clone().unwrap())
        })
        .collect();
    let changed = second.clone();
    let _seam = delete_seam::install(move |step, _| {
        if step == Step::Locked {
            write(&changed.join("added.dll"), b"added");
        }
    });

    let report = run_with(
        &resource,
        &listed_items,
        &APPLY,
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert!(report.refused, "{report:#?}");
    assert_eq!(
        statuses(&report),
        [DeleteStatus::NotReached, DeleteStatus::Refused]
    );
    assert_eq!(
        report.sets[1].refusal.as_ref().unwrap().code(),
        "changed-since-listed"
    );
    assert!(report.saved_report.is_none());
    assert_eq!(fs::read(first.join("a.dll")).unwrap(), b"a");
    assert_eq!(fs::read(second.join("b.dll")).unwrap(), b"b");
    assert!(!resource.join("RABBIT").join(DELETING_DIR).exists());
    assert!(!resource.join("RABBIT/logs").exists());
}

/// A listed set that changes while a set before it goes is refused alone,
/// at its turn: the set before it is gone, and the report says both.
#[test]
fn a_set_changed_while_another_goes_is_refused_alone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let first = layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let second = layout_set(&resource, "unix-2", &[("b.dll", b"b")]);
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let listed_items: Vec<DeleteItem> = ["unix-1", "unix-2"]
        .iter()
        .map(|id| {
            let set = listed.sets.iter().find(|set| set.id == *id).unwrap();
            DeleteItem::listed(id, set.fingerprint.clone().unwrap())
        })
        .collect();
    let changed = second.clone();
    let _seam = delete_seam::install(move |step, _| {
        if step == Step::Renamed {
            write(&changed.join("added.dll"), b"added");
        }
    });

    let report = run_with(
        &resource,
        &listed_items,
        &APPLY,
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert!(!report.refused);
    assert_eq!(
        statuses(&report),
        [DeleteStatus::Deleted, DeleteStatus::Refused]
    );
    assert_eq!(
        report.sets[1].refusal.as_ref().unwrap().code(),
        "changed-since-listed"
    );
    assert!(!first.exists());
    assert_eq!(fs::read(second.join("b.dll")).unwrap(), b"b");
    assert_eq!(saved(&report)["sets"][1]["status"], "refused");
}

/// A set whose fingerprint differs from the one its item was listed with
/// refuses the whole run before anything changes. One that still matches
/// is deleted. An unfinished deletion compares its own.
#[test]
fn a_changed_fingerprint_refuses_before_any_change() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    layout_set(&resource, "unix-2", &[("b.dll", b"b")]);
    write(&deleting_path(&resource, "unix-3").join("c.dll"), b"c");
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let fingerprint = |id: &str| {
        listed
            .sets
            .iter()
            .find(|set| set.id == id)
            .and_then(|set| set.fingerprint.clone())
            .unwrap()
    };
    let listed_items = vec![
        DeleteItem::listed("unix-1", fingerprint("unix-1")),
        DeleteItem::listed("unix-2", fingerprint("unix-2")),
        DeleteItem::listed("unix-3", fingerprint("unix-3")),
    ];
    write(&set_path(&resource, "unix-2").join("added.dll"), b"added");
    let before = snapshot(dir.path());

    let report = run_with(
        &resource,
        &listed_items,
        &APPLY,
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert!(report.refused);
    assert_eq!(
        statuses(&report),
        [
            DeleteStatus::NotReached,
            DeleteStatus::Refused,
            DeleteStatus::NotReached
        ]
    );
    assert_eq!(
        report.sets[1].refusal.as_ref().unwrap().code(),
        "changed-since-listed"
    );
    assert_eq!(snapshot(dir.path()), before);

    let report = run_with(
        &resource,
        &[listed_items[0].clone(), listed_items[2].clone()],
        &APPLY,
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert_eq!(
        statuses(&report),
        [DeleteStatus::Deleted, DeleteStatus::FinishedEarlierDeletion]
    );

    // An unfinished deletion that changed since it was listed is refused.
    let left = deleting_path(&resource, "unix-4");
    write(&left.join("d.dll"), b"d");
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let fingerprint = listed
        .sets
        .iter()
        .find(|set| set.id == "unix-4")
        .and_then(|set| set.fingerprint.clone())
        .unwrap();
    write(&left.join("e.dll"), b"e");

    let report = run_with(
        &resource,
        &[DeleteItem::listed("unix-4", fingerprint)],
        &APPLY,
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    );

    assert!(report.refused);
    assert_eq!(
        report.sets[0].refusal.as_ref().unwrap().code(),
        "changed-since-listed"
    );
    assert!(left.join("d.dll").is_file() && left.join("e.dll").is_file());
}

/// A dry run says what would go, how big each set is and which backups in
/// it are the only copy of something, and writes nothing at all: no file,
/// no folder of deletions, no lock, no report.
#[test]
fn a_dry_run_writes_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    write(&resource.join("reaper.ini"), b"[REAPER]");
    let uninstall = uninstall_set(
        &resource,
        "uninstall-unix-1",
        "sws",
        &[("UserPlugins/reaper_sws-x64.dll", b"sws")],
    );
    keymap_set(&resource, "unattended-unix-2-000000000");
    let before = snapshot(dir.path());

    let report = run(
        &resource,
        &[
            "uninstall-unix-1",
            "unattended-unix-2-000000000",
            "unix-404",
        ],
        &DRY_RUN,
    );

    assert_eq!(
        statuses(&report),
        [
            DeleteStatus::WouldDelete,
            DeleteStatus::WouldDelete,
            DeleteStatus::AlreadyGone
        ]
    );
    assert!(report.dry_run && !report.refused);
    let would = &report.sets[0];
    assert_eq!(would.set_path.as_deref(), Some(uninstall.as_path()));
    assert_eq!(would.size, file_bytes(&uninstall));
    assert_eq!(would.bytes_freed, 0);
    assert_eq!(
        would.only_copy,
        [BackupItem::Package {
            set_id: "uninstall-unix-1".to_string(),
            package_id: "sws".to_string()
        }]
    );
    assert_eq!(
        report.sets[1].only_copy,
        [BackupItem::Keymap {
            set_id: "unattended-unix-2-000000000".to_string()
        }]
    );
    assert!(report.saved_report.is_none());
    assert_eq!(snapshot(dir.path()), before);
    let rabbit = resource.join("RABBIT");
    for folder in [DELETING_DIR, "locks", "logs"] {
        assert!(!rabbit.join(folder).exists(), "{folder}");
    }
}

/// A real delete changes nothing outside `RABBIT/backups` and
/// `RABBIT/backups-deleting`, besides the report it saves in
/// `RABBIT/logs`: not REAPER's files, not RABBIT's records, not another
/// set, not an older report, not an empty folder.
#[test]
fn nothing_outside_the_two_folders_changes() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    write(&resource.join("reaper.ini"), b"[REAPER]");
    write(&resource.join("UserPlugins/reaper_osara64.dll"), b"osara");
    write(&resource.join("RABBIT/install-state.json"), b"{}");
    write(&resource.join("RABBIT/logs/uninstall-unix-1.json"), b"{}");
    // The snapshots list files only, so empty folders are checked apart.
    let empty_folders = [resource.join("Effects"), resource.join("RABBIT/cache")];
    for folder in &empty_folders {
        fs::create_dir_all(folder).unwrap();
    }
    uninstall_set(
        &resource,
        "uninstall-unix-1",
        "sws",
        &[("UserPlugins/reaper_sws-x64.dll", b"sws")],
    );
    let kept = layout_set(&resource, "unix-2", &[("a.dll", b"a")]);
    let outside = dir.path().join("Beside");
    write(&outside.join("file.txt"), b"beside");
    let before = everything_else(&resource);
    let kept_before = snapshot(&kept);
    let beside_before = snapshot(&outside);
    let old_report = fs::read(resource.join("RABBIT/logs/uninstall-unix-1.json")).unwrap();

    let report = run(&resource, &["uninstall-unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    assert_eq!(everything_else(&resource), before);
    for folder in &empty_folders {
        assert!(folder.is_dir(), "{}", folder.display());
    }
    assert_eq!(snapshot(&kept), kept_before);
    assert_eq!(snapshot(&outside), beside_before);
    assert_eq!(
        fs::read(resource.join("RABBIT/logs/uninstall-unix-1.json")).unwrap(),
        old_report
    );
    let logs: Vec<String> = fs::read_dir(resource.join("RABBIT/logs"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name != "uninstall-unix-1.json")
        .collect();
    assert_eq!(logs.len(), 2, "{logs:?}");
    assert!(
        logs.iter()
            .all(|name| name.starts_with("delete-backup-unix-"))
    );
}

/// A well-formed id that names nothing in either folder is already gone,
/// so a script can run twice, and an id named twice is gone the second
/// time. In a REAPER with no `RABBIT` folder, a run in which every id
/// names nothing changes nothing at all: no lock, no report, not even a
/// `RABBIT` folder. With one, it saves its report as every real run does.
/// An id that isn't one, or names a set or an unfinished deletion only in
/// another case or with a dot or space at its end, is refused.
#[test]
fn an_id_that_names_nothing_is_already_gone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    fs::create_dir_all(&resource).unwrap();

    let report = run(&resource, &["unix-404", "uninstall-unix-9"], &APPLY);

    assert_eq!(
        statuses(&report),
        [DeleteStatus::AlreadyGone, DeleteStatus::AlreadyGone]
    );
    assert!(!report.refused);
    assert!(report.saved_report.is_none());
    assert!(!resource.join("RABBIT").exists());

    layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let report = run(&resource, &["unix-1", "unix-1", "unix-404"], &APPLY);

    assert_eq!(
        statuses(&report),
        [
            DeleteStatus::Deleted,
            DeleteStatus::AlreadyGone,
            DeleteStatus::AlreadyGone
        ]
    );
    assert!(report.saved_report.is_some());

    // Run again, the same id is already gone, and the run says so in a
    // report of its own.
    let report = run(&resource, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::AlreadyGone]);
    assert_eq!(saved(&report)["sets"][0]["status"], "already-gone");

    layout_set(&resource, "unix-5", &[("a.dll", b"a")]);
    write(&deleting_path(&resource, "unix-6").join("b.dll"), b"b");
    let before = snapshot(dir.path());
    for (id, code) in [
        ("UNIX-5", "differs-in-case"),
        ("UNIX-6", "differs-in-case"),
        ("a/b", "invalid-id"),
        ("..", "invalid-id"),
        ("", "invalid-id"),
    ] {
        let report = run(&resource, &[id, "unix-5"], &APPLY);
        assert!(report.refused, "{id}");
        assert_eq!(
            report.sets[0].refusal.as_ref().unwrap().code(),
            code,
            "{id}"
        );
        assert_eq!(report.sets[1].status, DeleteStatus::NotReached, "{id}");
    }
    #[cfg(windows)]
    for (id, code) in [
        ("D:", "invalid-id"),
        ("C:foo", "invalid-id"),
        ("unix-5.", "ends-in-dot-or-space"),
        ("unix-6.", "ends-in-dot-or-space"),
        ("CON", "invalid-id"),
    ] {
        let report = run(&resource, &[id], &APPLY);
        assert_eq!(
            report.sets[0].refusal.as_ref().unwrap().code(),
            code,
            "{id}"
        );
    }
    // A name that is well-formed on Unix, which keeps a dot at its end.
    #[cfg(unix)]
    for id in ["unix-5.", "unix-6."] {
        let report = run(&resource, &[id], &APPLY);
        assert_eq!(
            report.sets[0].refusal.as_ref().unwrap().code(),
            "ends-in-dot-or-space",
            "{id}"
        );
    }
    assert_eq!(snapshot(dir.path()), before);
}

/// A resource folder that isn't there, as after a typo in its path, is an
/// error, in a dry run too: counting every id as already gone would say
/// the sets are gone while they are still in the folder meant.
#[test]
fn a_resource_folder_that_isnt_there_is_an_error() {
    let dir = tempdir().unwrap();
    let typo = dir.path().join("REAPPER");
    let file = dir.path().join("reaper.ini");
    write(&file, b"[REAPER]");

    for resource in [&typo, &file] {
        for options in [APPLY, DRY_RUN] {
            let error = delete_backups_with(
                resource,
                &items(&["unix-1"]),
                &options,
                &BackupEnvironment::AS_USER,
                &ProgressReporter::noop(),
                &CancelToken::new(),
            )
            .unwrap_err();
            assert!(
                matches!(&error, crate::error::RabbitError::Io { path, .. } if path == resource),
                "{error:?}"
            );
        }
    }
    assert!(!typo.exists());
    assert_eq!(fs::read(&file).unwrap(), b"[REAPER]");
}

/// The run reads its stop between sets: the set being deleted finishes,
/// those after it aren't reached and stay as they were, and the report,
/// which says the run was stopped, is saved.
#[test]
fn a_stop_between_sets_leaves_the_rest() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let first = layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let second = layout_set(&resource, "unix-2", &[("b.dll", b"b")]);
    let second_before = snapshot(&second);
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let progress = ProgressReporter::new(move |event| {
        // Pressed while the first set goes.
        if matches!(event, ProgressEvent::DeleteStarted { .. }) {
            stop.cancel();
        }
        sink.lock().unwrap().push(event);
    });

    let report = run_with(
        &resource,
        &items(&["unix-1", "unix-2"]),
        &APPLY,
        &BackupEnvironment::AS_USER,
        &progress,
        &cancel,
    );

    assert_eq!(
        statuses(&report),
        [DeleteStatus::Deleted, DeleteStatus::NotReached]
    );
    assert!(report.stopped);
    assert!(!first.exists());
    assert_eq!(snapshot(&second), second_before);
    assert_eq!(events.lock().unwrap().len(), 2);
    let json = saved(&report);
    assert_eq!(json["stopped"], true);
    assert_eq!(json["sets"][1]["status"], "not-reached");
}

/// Several sets with one that can't be deleted: the others go, each status
/// says what came of its set, and the report is saved. On Windows the one
/// has a file held open; on Unix a folder RABBIT may not look into.
#[test]
fn several_sets_with_one_failing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    #[cfg(unix)]
    {
        if running_as_root(dir.path()) {
            return;
        }
    }
    let first = layout_set(&resource, "unix-1", &[("a.dll", b"aa")]);
    let failing = layout_set(&resource, "unix-2", &[("sub/b.dll", b"b"), ("c.dll", b"c")]);
    let third = layout_set(&resource, "unix-3", &[("d.dll", b"ddd")]);
    #[cfg(windows)]
    let held = hold_open(&failing.join("sub/b.dll"), 0);
    #[cfg(unix)]
    let _mode_back = {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(failing.join("sub"), fs::Permissions::from_mode(0o000)).unwrap();
        ModeBack(vec![
            failing.join("sub"),
            deleting_path(&resource, "unix-2").join("sub"),
        ])
    };

    let report = run(&resource, &["unix-1", "unix-2", "unix-3"], &APPLY);
    #[cfg(windows)]
    drop(held);

    #[cfg(windows)]
    let failed = DeleteStatus::NotDeleted;
    #[cfg(unix)]
    let failed = DeleteStatus::PartlyDeleted;
    assert_eq!(
        statuses(&report),
        [DeleteStatus::Deleted, failed, DeleteStatus::Deleted]
    );
    assert!(!report.refused && !report.stopped);
    assert_eq!(report.sets[0].bytes_freed, 2);
    assert_eq!(report.sets[2].bytes_freed, 3);
    assert_eq!(
        report.bytes_freed,
        report.sets.iter().map(|set| set.bytes_freed).sum::<u64>()
    );
    assert!(!first.exists() && !third.exists());
    let json = saved(&report);
    let saved_statuses: Vec<&str> = json["sets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|set| set["status"].as_str().unwrap())
        .collect();
    #[cfg(windows)]
    assert_eq!(saved_statuses, ["deleted", "not-deleted", "deleted"]);
    #[cfg(unix)]
    assert_eq!(saved_statuses, ["deleted", "partly-deleted", "deleted"]);
}

/// A set that became a link while the sets before it went is refused on
/// its own, at its turn: the sets before and after it go, and what it
/// leads to stays.
#[test]
fn a_set_that_became_a_link_is_refused_alone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let second = layout_set(&resource, "unix-2", &[("b.dll", b"b")]);
    layout_set(&resource, "unix-3", &[("c.dll", b"c")]);
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("keep.dll"), b"keep");
    let mut first_time = true;
    let link_target = elsewhere.clone();
    let _seam = delete_seam::install(move |step, _| {
        if step == Step::Renamed && std::mem::take(&mut first_time) {
            fs::remove_dir_all(&second).unwrap();
            link_folder(&second, &link_target);
        }
    });

    let report = run(&resource, &["unix-1", "unix-2", "unix-3"], &APPLY);

    assert_eq!(
        statuses(&report),
        [
            DeleteStatus::Deleted,
            DeleteStatus::Refused,
            DeleteStatus::Deleted
        ]
    );
    assert_eq!(report.sets[1].refusal.as_ref().unwrap().code(), "is-link");
    assert!(!report.refused);
    assert_eq!(fs::read(elsewhere.join("keep.dll")).unwrap(), b"keep");
    assert!(report.saved_report.is_some());
}

/// The overview's records row stands for every set that holds only a copy
/// of RABBIT's records, and gives each id, so each is deleted as an item
/// of its own.
#[test]
fn the_records_row_deletes_each_of_its_sets() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    records_set(&resource, "unattended-unix-100-000000000");
    records_set(&resource, "unattended-unix-200-000000000");
    layout_set(&resource, "unix-3", &[("a.dll", b"a")]);
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let records = listed
        .entries
        .iter()
        .find(|entry| matches!(entry.item, BackupItem::Records { .. }))
        .unwrap();
    let ids = records.item.set_ids();
    assert_eq!(ids.len(), 2);

    let report = run(&resource, &ids, &APPLY);

    assert_eq!(
        statuses(&report),
        [DeleteStatus::Deleted, DeleteStatus::Deleted]
    );
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(listed.sets.len(), 1);
    assert_eq!(listed.sets[0].id, "unix-3");
    assert_eq!(
        BackupItem::Set {
            set_id: "unix-3".to_string()
        }
        .set_ids(),
        ["unix-3"]
    );
}

/// The walk works only inside `RABBIT/backups-deleting/<id>`: handed a set
/// still in `RABBIT/backups`, or any other folder, it removes nothing. The
/// guard allows the folder and plain names inside it, nothing else.
#[test]
fn the_walk_never_leaves_the_folder_of_its_set() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = layout_set(&resource, "unix-1", &[("a.dll", b"a")]);
    let before = snapshot(dir.path());

    let removal = remove_set_folder(&resource, "unix-1", &set);
    assert!(!removal.complete);
    assert!(removal.first_error.is_some());
    let removal = remove_set_folder(&resource, "unix-2", &deleting_path(&resource, "unix-1"));
    assert!(!removal.complete);
    assert_eq!(snapshot(dir.path()), before);

    let guard = DeleteGuard::new(&resource, "unix-1");
    let root = deleting_path(&resource, "unix-1");
    assert!(guard.allows(&root));
    assert!(guard.allows(&root.join("sub").join("a.dll")));
    assert!(!guard.allows(&root.join("..").join("unix-2")));
    assert!(!guard.allows(&deleting_path(&resource, "unix-10")));
    assert!(!guard.allows(&deleting_path(&resource, "unix-1").with_file_name("unix-2")));
    assert!(!guard.allows(&set));
    assert!(!guard.allows(&resource.join("RABBIT")));
}

/// Looking at an id's sets for a delete reads them as the overview does,
/// and the overview of one id lists an unfinished deletion beside its set.
#[test]
fn an_unfinished_deletion_is_described_beside_its_set() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = keymap_set(&resource, "unix-1");
    write(
        &deleting_path(&resource, "unix-1").join("left.dll"),
        b"left",
    );

    let report = run(&resource, &["unix-1"], &DRY_RUN);

    assert_eq!(statuses(&report), [DeleteStatus::WouldDelete]);
    let one = overview_of_with(&resource, "unix-1", &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(
        report.sets[0].size,
        one.sets.iter().map(|set| set.size).sum::<u64>()
    );
    assert!(report.sets[0].size >= file_bytes(&set) + 4);
    assert!(report.sets[0].unfinished_deletion.is_some());
}
