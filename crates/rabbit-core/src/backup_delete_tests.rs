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
    DeleteGuard, DeleteItem, DeletePart, DeleteReport, DeleteRunOptions, DeleteStatus,
    NotDeletedReason, delete_backups_with, remove_set_folder,
};
use crate::backup_overview::{
    BackupItem, BackupSetKind, BackupStatus, RestoreScope, overview_of_with, overview_with,
};
use crate::cancel::CancelToken;
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::receipt::{PackageReceipt, load_install_state};
use crate::restore_refusal::{RestoreRefusal, refusal};
use crate::rollback::restore_run_tests::{Node, snapshot, snapshot_but_lock_folder};
use crate::rollback::{
    BEFORE_RESTORE_REASON, BackupEnvironment, BackupManifest, BackupPackageRecord, BackupSetWriter,
    DELETING_DIR, KEYMAP_BACKUP_REASON, KeymapOrigin, OutsideRestoreRules, RECORDS_BACKUP_REASON,
    ReplacedPackageRecord, RestoreBackupOptions, RestoreStatus, UPDATE_BACKUP_REASON,
    load_backup_manifest, restore_backup_set_within, save_backup_manifest, set_to_restore,
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
        .filter_map(|(_, node)| match node {
            Node::File(contents) => Some(contents.len() as u64),
            _ => None,
        })
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

/// Every file, folder and link below `root` except those in RABBIT's two
/// backup folders, its logs, which hold the report a real run saves, and
/// its locks, whose folder the lock a real run takes leaves behind.
fn everything_else(resource: &Path) -> Vec<(PathBuf, Node)> {
    let rabbit = resource.join("RABBIT");
    let ours = [
        rabbit.join("backups"),
        rabbit.join(DELETING_DIR),
        rabbit.join("logs"),
        rabbit.join("locks"),
    ];
    snapshot(resource)
        .into_iter()
        .filter(|(path, _)| !ours.iter().any(|folder| path.starts_with(folder)))
        .collect()
}

/// Every file, folder and link below `root`, by its path below it, with
/// what each file holds.
fn relative_snapshot(root: &Path) -> Vec<(PathBuf, Node)> {
    snapshot(root)
        .into_iter()
        .map(|(path, contents)| (path.strip_prefix(root).unwrap().to_path_buf(), contents))
        .collect()
}

/// What a test saw right after a set's rename: where the set went, what it
/// held there, whether it was still in `RABBIT/backups`, and the code a
/// restore of it gave.
type SeenAfterRename = (PathBuf, Vec<(PathBuf, Node)>, bool, String);

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
    let outside_after: Vec<_> = snapshot(dir.path())
        .into_iter()
        .filter(|(path, _)| path.starts_with(&elsewhere))
        .collect();
    assert_eq!(outside_after, outside);
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
    assert!(!resource.join("RABBIT/locks").exists());

    // A linked RABBIT the same, and nothing is written through it, not even
    // the lock, whose empty folder would stay.
    let other = dir.path().join("Other");
    let rabbit_elsewhere = dir.path().join("RabbitElsewhere");
    write(&rabbit_elsewhere.join("backups/unix-1/a.dll"), b"a");
    fs::create_dir_all(&other).unwrap();
    link_folder(&other.join("RABBIT"), &rabbit_elsewhere);
    let before = snapshot(dir.path());

    let report = run(&other, &["unix-1"], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Refused]);
    assert_eq!(snapshot(dir.path()), before);
    for folder in ["locks", "logs", DELETING_DIR] {
        assert!(!rabbit_elsewhere.join(folder).exists(), "{folder}");
    }
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

/// An id that names a set and an unfinished deletion is fingerprinted by
/// its set, which a caller listing both rows must know: the fingerprint of
/// the unfinished deletion's row is refused as changed since listed, with
/// nothing changed, while the set's deletes both, whatever changed inside
/// the unfinished deletion alone since.
#[test]
fn an_id_in_both_folders_is_fingerprinted_by_its_set() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let set = keymap_set(&resource, "unix-1");
    let left = deleting_path(&resource, "unix-1");
    write(&left.join("left.dll"), b"left");
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let fingerprint = |deleting: bool| {
        listed
            .sets
            .iter()
            .find(|row| row.id == "unix-1" && (row.kind == BackupSetKind::Deleting) == deleting)
            .and_then(|row| row.fingerprint.clone())
            .unwrap()
    };
    let (of_set, of_deletion) = (fingerprint(false), fingerprint(true));
    assert_ne!(of_set, of_deletion);
    let before = snapshot(dir.path());
    let delete = |fingerprint| {
        run_with(
            &resource,
            &[DeleteItem::listed("unix-1", fingerprint)],
            &APPLY,
            &BackupEnvironment::AS_USER,
            &ProgressReporter::noop(),
            &CancelToken::new(),
        )
    };

    let report = delete(of_deletion);

    assert!(report.refused);
    assert_eq!(statuses(&report), [DeleteStatus::Refused]);
    assert_eq!(
        report.sets[0].refusal.as_ref().unwrap().code(),
        "changed-since-listed"
    );
    assert!(report.saved_report.is_none());
    assert_eq!(snapshot(dir.path()), before);

    write(&left.join("more.dll"), b"more");
    let report = delete(of_set);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted], "{report:#?}");
    assert!(report.sets[0].finished_earlier_deletion);
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
    let after: Vec<_> = snapshot_but_lock_folder(dir.path(), &resource)
        .into_iter()
        .filter(|(path, _)| !path.starts_with(resource.join("RABBIT/logs")))
        .collect();
    assert_eq!(after, before);
}

/// Windows, and a Mac's volume unless it was made case-sensitive, count
/// names that differ only in case as the same. An unfinished deletion
/// named so takes the name a set would get, so the set isn't deleted, and
/// the report names that deletion, which the id never names, instead of
/// asking for another run of the same id.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn an_unfinished_deletion_in_another_case_leaves_the_set_alone() {
    let dir = tempdir().unwrap();
    if !crate::rollback::restore_run_tests::names_in_another_case_are_one(dir.path()) {
        return;
    }
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
    // The snapshots list folders too; these are checked apart all the same.
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
/// so a script can run twice. An id named twice is one set, deleted once:
/// both items share what came of it, and only the first carries the bytes
/// freed. In a REAPER with no `RABBIT` folder, a run in which every id
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
            DeleteStatus::Deleted,
            DeleteStatus::AlreadyGone
        ]
    );
    assert_eq!(report.sets[0].bytes_freed, 1);
    assert_eq!(report.sets[1].bytes_freed, 0);
    assert_eq!(report.bytes_freed, 1);
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

// One backup out of a set: a package or the key map.

/// A package's id and its files, each by its path in the resource folder,
/// with what it holds.
type PackageFiles<'a> = (&'a str, &'a [(&'a str, &'a [u8])]);

/// An uninstall's set `id` holding each of `packages`, with each package's
/// files written in the resource folder and moved in, as one uninstall of
/// them all leaves it.
fn uninstall_set_of(resource: &Path, id: &str, packages: &[PackageFiles<'_>]) -> PathBuf {
    let mut writer = BackupSetWriter::new(resource, id, UNINSTALL_BACKUP_REASON);
    for (package_id, files) in packages {
        writer
            .record_package(package_id, uninstall_record(package_id))
            .unwrap();
        for (relative, contents) in *files {
            let place = resource.join(relative);
            write(&place, contents);
            writer
                .move_file(Some(package_id), &place, None, false)
                .unwrap();
        }
    }
    writer.path().to_path_buf()
}

/// What an uninstall records of `package_id`, settings moved too.
fn uninstall_record(package_id: &str) -> BackupPackageRecord {
    BackupPackageRecord {
        receipt: Some(receipt_entry(package_id)),
        langpack_before: None,
        with_settings: Some(true),
    }
}

/// SWS and FFmpeg, as one uninstall moved them into the set `id`: SWS's
/// plug-in and grooves, 18 bytes, and FFmpeg's codec, 12 bytes.
fn sws_and_ffmpeg(resource: &Path, id: &str) -> PathBuf {
    uninstall_set_of(
        resource,
        id,
        &[
            (
                "sws",
                &[
                    ("UserPlugins/reaper_sws-x64.dll", b"sws plug-in"),
                    ("Data/SWS/deep/grooves.txt", b"grooves"),
                ],
            ),
            ("ffmpeg", &[("UserPlugins/avcodec-61.dll", b"ffmpeg codec")]),
        ],
    )
}

/// An update's set `id` that also holds the key map OSARA's replaced, as
/// one update of OSARA and ReaPack writes it: ReaPack 1.0's plug-in, 13
/// bytes, recorded as replaced by 1.2.5, and your own key map, 8 bytes.
fn update_set_with_keymap(resource: &Path, id: &str) -> PathBuf {
    let reapack = resource.join("UserPlugins/reaper_reapack-x64.dll");
    write(&reapack, b"reapack 1.0.0");
    let keymap = resource.join("reaper-kb.ini");
    write(&keymap, b"own keys");
    let mut writer = BackupSetWriter::new(resource, id, UPDATE_BACKUP_REASON);
    writer
        .record_replaced(
            "reapack",
            ReplacedPackageRecord {
                receipt: Some(receipt_entry("reapack")),
                replaced_by: Some(crate::version::Version::parse("1.2.5").unwrap()),
            },
        )
        .unwrap();
    writer.copy_file(Some("reapack"), &reapack).unwrap();
    writer.set_keymap_origin(KeymapOrigin::OwnKeymap).unwrap();
    writer.copy_file(Some("osara"), &keymap).unwrap();
    writer.path().to_path_buf()
}

fn package(set_id: &str, package_id: &str) -> BackupItem {
    BackupItem::Package {
        set_id: set_id.to_string(),
        package_id: package_id.to_string(),
    }
}

fn keymap(set_id: &str) -> BackupItem {
    BackupItem::Keymap {
        set_id: set_id.to_string(),
    }
}

/// Delete `items` from `resource` as the user, with `options`.
fn run_items(resource: &Path, items: &[DeleteItem], options: &DeleteRunOptions) -> DeleteReport {
    run_with(
        resource,
        items,
        options,
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
}

/// The backups the overview lists in the set `id`, in its order.
fn listed_backups(resource: &Path, id: &str) -> Vec<BackupItem> {
    overview_of_with(resource, id, &BackupEnvironment::AS_USER)
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.item)
        .collect()
}

/// Restore `package_id` from the set `id` for real, as the user, putting
/// nothing back outside the resource folder.
fn restore_package(resource: &Path, id: &str, package_id: &str) -> RestoreStatus {
    restore_backup_set_within(
        resource,
        id,
        &RestoreBackupOptions {
            dry_run: false,
            allow_reaper_running: true,
            package_id: Some(package_id.to_string()),
        },
        &OutsideRestoreRules::default(),
    )
    .unwrap()
    .status
}

/// Deleting one package out of an uninstall's set saves the set's manifest
/// without its files and record first, then removes its files and the
/// folders they leave empty. The other package stays, listed as the set's
/// one backup, and restores; the set's size is what is left on disk. The
/// run sends one start and one end for the set, and saves its report.
#[test]
fn one_package_goes_out_of_an_uninstall_set_and_the_other_still_restores() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let (events, progress) = collecting();

    let report = run_with(
        &resource,
        &[DeleteItem::package(id, "sws")],
        &APPLY,
        &BackupEnvironment::AS_USER,
        &progress,
        &CancelToken::new(),
    );

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    let done = &report.sets[0];
    assert_eq!(
        done.part,
        DeletePart::Package {
            package_id: "sws".to_string()
        }
    );
    assert_eq!(done.set_path.as_deref(), Some(set.as_path()));
    assert_eq!(done.size, 18);
    assert_eq!(done.bytes_freed, 18);
    assert_eq!(report.bytes_freed, 18);
    assert_eq!(done.only_copy, [package(id, "sws")]);
    assert!(done.files_left.is_empty() && done.error.is_none());
    assert!(!report.refused && !report.stopped);
    // Its files and the folders they leave empty are gone; FFmpeg's stay,
    // and so does the set's folder.
    assert!(!set.join("UserPlugins/reaper_sws-x64.dll").exists());
    assert!(!set.join("Data").exists());
    assert_eq!(
        fs::read(set.join("UserPlugins/avcodec-61.dll")).unwrap(),
        b"ffmpeg codec"
    );
    assert!(!set.join("backup-manifest.json.rabbit-tmp").exists());
    let manifest = load_backup_manifest(&set).unwrap().unwrap();
    assert_eq!(manifest.packages.keys().collect::<Vec<_>>(), ["ffmpeg"]);
    assert_eq!(manifest.files.len(), 1);
    assert_eq!(manifest.files[0].package_id.as_deref(), Some("ffmpeg"));
    assert!(!deleting_path(&resource, id).exists());

    // Listed as an uninstall's set that holds FFmpeg, as big as what is
    // left in it.
    let listed = overview_of_with(&resource, id, &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(listed.sets.len(), 1);
    assert_eq!(listed.sets[0].kind, BackupSetKind::Uninstall);
    assert_eq!(listed.sets[0].size, file_bytes(&set));
    assert_eq!(listed.sets[0].warning, None);
    assert_eq!(listed.entries.len(), 1);
    assert_eq!(listed.entries[0].item, package(id, "ffmpeg"));
    assert_eq!(listed.entries[0].status, BackupStatus::CanRestore);
    assert_eq!(
        *events.lock().unwrap(),
        [
            ProgressEvent::DeleteStarted {
                set_id: id.to_string()
            },
            ProgressEvent::DeleteCompleted {
                set_id: id.to_string(),
                status: DeleteStatus::DeletedFromSet,
                bytes_freed: 18,
            },
        ]
    );
    let json = saved(&report);
    assert_eq!(json["sets"][0]["status"], "deleted-from-set");
    assert_eq!(json["sets"][0]["part"]["kind"], "package");
    assert_eq!(json["sets"][0]["part"]["package_id"], "sws");

    // FFmpeg comes back as it was; SWS doesn't.
    assert_eq!(
        restore_package(&resource, id, "ffmpeg"),
        RestoreStatus::Restored
    );
    assert_eq!(
        fs::read(resource.join("UserPlugins/avcodec-61.dll")).unwrap(),
        b"ffmpeg codec"
    );
    assert!(!resource.join("UserPlugins/reaper_sws-x64.dll").exists());
    let state = load_install_state(&resource).unwrap().unwrap();
    assert!(state.packages.contains_key("ffmpeg"));
    assert!(!state.packages.contains_key("sws"));
}

/// The key map goes out of an update's set that also holds an earlier
/// version: its copy and the record of what it was go, the earlier version
/// stays as it was listed, and the set is still an update's set, not a
/// damaged one. The key map in REAPER's folder is never touched.
#[test]
fn the_key_map_goes_out_of_a_set_that_also_holds_a_package() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "unix-1";
    let set = update_set_with_keymap(&resource, id);
    let listed = overview_of_with(&resource, id, &BackupEnvironment::AS_USER).unwrap();
    let earlier = listed
        .entries
        .iter()
        .find(|entry| entry.item == package(id, "reapack"))
        .cloned()
        .unwrap();
    assert_eq!(listed.entries.len(), 2);

    let report = run_items(&resource, &[DeleteItem::keymap(id)], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert_eq!(report.sets[0].part, DeletePart::Keymap);
    assert_eq!(report.sets[0].bytes_freed, 8);
    assert_eq!(report.sets[0].only_copy, [keymap(id)]);
    assert!(!set.join("reaper-kb.ini").exists());
    assert!(set.join("UserPlugins/reaper_reapack-x64.dll").is_file());
    assert_eq!(
        fs::read(resource.join("reaper-kb.ini")).unwrap(),
        b"own keys"
    );
    let manifest = load_backup_manifest(&set).unwrap().unwrap();
    assert_eq!(manifest.keymap_origin, None);
    assert!(manifest.replaced.contains_key("reapack"));
    assert_eq!(manifest.files.len(), 1);

    let listed = overview_of_with(&resource, id, &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(listed.sets[0].kind, BackupSetKind::Update);
    assert_eq!(listed.entries.len(), 1);
    let after = &listed.entries[0];
    assert_eq!(after.item, earlier.item);
    assert_eq!(after.status, earlier.status);
    assert_eq!(after.restore_scope, earlier.restore_scope);
    assert_eq!(after.size, earlier.size);
}

/// The earlier version goes out of the same kind of set, and the key map
/// stays: a set of earlier versions left with no version and no record of
/// one is no damaged set, and its key map still lists and restores alone.
#[test]
fn a_set_left_with_its_key_map_alone_still_restores_it() {
    use crate::rollback::{RestoreItem, RestoreRunOptions, restore_backup_item_with};
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "unix-1";
    let set = update_set_with_keymap(&resource, id);

    let report = run_items(&resource, &[DeleteItem::package(id, "reapack")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert_eq!(report.sets[0].bytes_freed, 13);
    assert!(!set.join("UserPlugins").exists());
    let manifest = load_backup_manifest(&set).unwrap().unwrap();
    assert!(manifest.replaced.is_empty());
    assert_eq!(manifest.keymap_origin, Some(KeymapOrigin::OwnKeymap));
    let listed = overview_of_with(&resource, id, &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(listed.sets[0].kind, BackupSetKind::Update);
    assert_eq!(listed.entries.len(), 1);
    assert_eq!(listed.entries[0].item, keymap(id));
    assert_eq!(listed.entries[0].status, BackupStatus::CanRestore);
    assert_eq!(listed.entries[0].restore_scope, Some(RestoreScope::Keymap));
    assert_eq!(
        listed.entries[0].keymap_origin,
        Some(KeymapOrigin::OwnKeymap)
    );
    let restore = restore_backup_item_with(
        &resource,
        &RestoreItem::keymap(id),
        &RestoreRunOptions {
            dry_run: true,
            quick: false,
            allow_reaper_running: false,
        },
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
    )
    .unwrap();
    assert_eq!(restore.status, RestoreStatus::WouldRestore);
}

/// Naming every backup of a set in one run takes the set whole, as naming
/// the set does: renamed into `RABBIT/backups-deleting` first, then walked
/// away, with no save of its manifest. Both items share the result; the
/// first carries the set's size, the backups it held the only copy of and
/// the bytes freed, so the items add up to the run's total.
#[test]
fn every_part_of_a_set_in_one_run_takes_the_set_whole() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let bytes = file_bytes(&set);
    let steps: Arc<Mutex<Vec<(Step, PathBuf)>>> = Arc::default();
    let seen = Arc::clone(&steps);
    let _seam = delete_seam::install(move |step, path| {
        if matches!(step, Step::Renamed | Step::PartsSaved) {
            seen.lock().unwrap().push((step, path.to_path_buf()));
        }
    });
    let (events, progress) = collecting();

    let report = run_with(
        &resource,
        &[
            DeleteItem::package(id, "sws"),
            DeleteItem::package(id, "ffmpeg"),
        ],
        &APPLY,
        &BackupEnvironment::AS_USER,
        &progress,
        &CancelToken::new(),
    );

    assert_eq!(
        statuses(&report),
        [DeleteStatus::Deleted, DeleteStatus::Deleted]
    );
    assert_eq!(
        *steps.lock().unwrap(),
        [(Step::Renamed, deleting_path(&resource, id))]
    );
    assert!(!set.exists());
    assert!(!deleting_path(&resource, id).exists());
    assert_eq!(
        report.sets[1].part,
        DeletePart::Package {
            package_id: "ffmpeg".to_string()
        }
    );
    assert_eq!(report.sets[0].bytes_freed, bytes);
    assert_eq!(report.sets[1].bytes_freed, 0);
    assert_eq!(report.bytes_freed, bytes);
    assert_eq!(report.sets[0].size, bytes);
    assert_eq!(report.sets[0].only_copy.len(), 2);
    assert!(report.sets[1].only_copy.is_empty());
    assert_eq!(report.sets[1].set_path.as_deref(), Some(set.as_path()));
    assert_eq!(events.lock().unwrap().len(), 2);
}

/// A part and its whole set in one run take the set whole, in a dry run
/// too, where the first item says the whole set's size.
#[test]
fn a_part_and_its_whole_set_in_one_run_take_the_set_whole() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let items = [DeleteItem::package(id, "sws"), DeleteItem::new(id)];

    let report = run_items(&resource, &items, &DRY_RUN);

    assert_eq!(
        statuses(&report),
        [DeleteStatus::WouldDelete, DeleteStatus::WouldDelete]
    );
    assert_eq!(report.sets[0].size, file_bytes(&set));
    assert_eq!(report.sets[1].size, 0);

    let report = run_items(&resource, &items, &APPLY);

    assert_eq!(
        statuses(&report),
        [DeleteStatus::Deleted, DeleteStatus::Deleted]
    );
    assert!(!set.exists());
    assert!(listed_backups_of_resource(&resource).is_empty());
}

/// Every backup the overview lists in `resource`.
fn listed_backups_of_resource(resource: &Path) -> Vec<BackupItem> {
    overview_with(resource, &BackupEnvironment::AS_USER)
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.item)
        .collect()
}

/// Two parts of one set, listed with one fingerprint, go out of it in one
/// save of its manifest: the fingerprint is compared before it, not between
/// the two. Each item says what its own part freed. The third package
/// stays.
#[test]
fn two_parts_of_a_set_go_in_one_save_of_its_manifest() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = uninstall_set_of(
        &resource,
        id,
        &[
            ("sws", &[("UserPlugins/reaper_sws-x64.dll", b"sws plug-in")]),
            ("ffmpeg", &[("UserPlugins/avcodec-61.dll", b"ffmpeg codec")]),
            (
                "reapack",
                &[("UserPlugins/reaper_reapack-x64.dll", b"reapack")],
            ),
        ],
    );
    let fingerprint = overview_of_with(&resource, id, &BackupEnvironment::AS_USER)
        .unwrap()
        .sets[0]
        .fingerprint
        .clone()
        .unwrap();
    let saves = Arc::new(Mutex::new(0));
    let counted = Arc::clone(&saves);
    let _seam = delete_seam::install(move |step, _| {
        if step == Step::PartsSaved {
            *counted.lock().unwrap() += 1;
        }
    });
    let (events, progress) = collecting();

    let report = run_with(
        &resource,
        &[
            DeleteItem::package(id, "sws").with_fingerprint(fingerprint.clone()),
            DeleteItem::package(id, "ffmpeg").with_fingerprint(fingerprint),
        ],
        &APPLY,
        &BackupEnvironment::AS_USER,
        &progress,
        &CancelToken::new(),
    );

    assert_eq!(
        statuses(&report),
        [DeleteStatus::DeletedFromSet, DeleteStatus::DeletedFromSet]
    );
    assert_eq!(*saves.lock().unwrap(), 1);
    assert_eq!(report.sets[0].bytes_freed, 11);
    assert_eq!(report.sets[1].bytes_freed, 12);
    assert_eq!(report.bytes_freed, 23);
    assert_eq!(report.sets[1].size, 12);
    assert_eq!(listed_backups(&resource, id), [package(id, "reapack")]);
    assert!(set.join("UserPlugins/reaper_reapack-x64.dll").is_file());
    assert_eq!(
        *events.lock().unwrap(),
        [
            ProgressEvent::DeleteStarted {
                set_id: id.to_string()
            },
            ProgressEvent::DeleteCompleted {
                set_id: id.to_string(),
                status: DeleteStatus::DeletedFromSet,
                bytes_freed: 23,
            },
        ]
    );
}

/// A set that records no packages, as a restore's set of files it replaced
/// does, holding a file of SWS, which no backup the overview lists holds
/// apart: the set lists only its own backup.
fn set_of_files_that_records_no_packages(resource: &Path, id: &str) -> PathBuf {
    let plugin = resource.join("UserPlugins/reaper_sws-x64.dll");
    write(&plugin, b"sws plug-in");
    let mut writer = BackupSetWriter::new(resource, id, BEFORE_RESTORE_REASON);
    writer.copy_file(Some("sws"), &plugin).unwrap();
    writer.path().to_path_buf()
}

/// A part a set can't give up alone refuses the whole run before anything
/// changes, in a dry run too: a package the set holds no backup of, as in a
/// key map's set or the records, a key map it doesn't hold, a package whose
/// files a set that records no packages holds, a part of a set without a
/// manifest, with a damaged one or with one that is a link, which is never
/// followed, and of an unfinished deletion, which goes only whole. A part
/// of an id that names nothing is already gone, as its set is.
#[test]
fn a_part_the_set_cant_give_up_alone_refuses_the_whole_run() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    sws_and_ffmpeg(&resource, "uninstall-unix-1");
    keymap_set(&resource, "unattended-unix-2-000000000");
    layout_set(&resource, "unix-3", &[("UserPlugins/a.dll", b"a")]);
    let damaged = layout_set(&resource, "unix-4", &[("UserPlugins/b.dll", b"b")]);
    fs::write(damaged.join("backup-manifest.json"), b"{ not json").unwrap();
    records_set(&resource, "unattended-unix-5-000000000");
    write(
        &deleting_path(&resource, "unix-6").join("left.dll"),
        b"left",
    );
    layout_set(&resource, "unix-7", &[("c.dll", b"c")]);
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("keep.dll"), b"keep");
    let linked = layout_set(&resource, "unix-8", &[("UserPlugins/d.dll", b"d")]);
    link_folder(&linked.join("backup-manifest.json"), &elsewhere);
    set_of_files_that_records_no_packages(&resource, "unix-9");
    assert_eq!(
        listed_backups(&resource, "unix-9"),
        [BackupItem::Set {
            set_id: "unix-9".to_string()
        }]
    );
    let before = snapshot(dir.path());

    for (item, code) in [
        (
            DeleteItem::package("uninstall-unix-1", "osara"),
            "package-not-in-set",
        ),
        (DeleteItem::keymap("uninstall-unix-1"), "no-keymap-in-set"),
        (
            DeleteItem::package("unattended-unix-2-000000000", "osara"),
            "package-not-in-set",
        ),
        (DeleteItem::package("unix-3", "sws"), "no-manifest"),
        (DeleteItem::keymap("unix-3"), "no-keymap-in-set"),
        (DeleteItem::package("unix-4", "sws"), "damaged-manifest"),
        (DeleteItem::keymap("unix-4"), "damaged-manifest"),
        (
            DeleteItem::package("unattended-unix-5-000000000", "sws"),
            "package-not-in-set",
        ),
        (
            DeleteItem::keymap("unattended-unix-5-000000000"),
            "no-keymap-in-set",
        ),
        (DeleteItem::package("unix-6", "sws"), "being-deleted"),
        (DeleteItem::keymap("unix-6"), "being-deleted"),
        (DeleteItem::package("unix-8", "sws"), "is-link"),
        (DeleteItem::keymap("unix-8"), "is-link"),
        (DeleteItem::package("unix-9", "sws"), "no-package-records"),
        (DeleteItem::package("unix-9", "osara"), "package-not-in-set"),
    ] {
        for options in [APPLY, DRY_RUN] {
            let report = run_items(
                &resource,
                &[DeleteItem::new("unix-7"), item.clone()],
                &options,
            );

            assert!(report.refused, "{item:?}");
            let other = if options.dry_run {
                DeleteStatus::WouldDelete
            } else {
                DeleteStatus::NotReached
            };
            assert_eq!(
                statuses(&report),
                [other, DeleteStatus::Refused],
                "{item:?}"
            );
            assert_eq!(report.sets[1].part, item.part, "{item:?}");
            assert_eq!(
                report.sets[1].refusal.as_ref().unwrap().code(),
                code,
                "{item:?}"
            );
            assert!(report.saved_report.is_none());
            assert_eq!(snapshot(dir.path()), before, "{item:?}");
        }
    }
    let report = run_items(
        &resource,
        &[DeleteItem::package("uninstall-unix-1", "osara")],
        &APPLY,
    );
    assert!(matches!(
        report.sets[0].refusal,
        Some(RestoreRefusal::PackageNotInSet { ref packages, .. }) if packages == &["ffmpeg", "sws"]
    ));
    // The key map's set holds no package at all to name.
    let report = run_items(
        &resource,
        &[DeleteItem::package("unattended-unix-2-000000000", "osara")],
        &APPLY,
    );
    assert!(matches!(
        report.sets[0].refusal,
        Some(RestoreRefusal::PackageNotInSet { ref packages, .. }) if packages.is_empty()
    ));
    // What the linked manifest leads to was never read as one, nor changed.
    assert_eq!(fs::read(elsewhere.join("keep.dll")).unwrap(), b"keep");

    let report = run_items(
        &resource,
        &[DeleteItem::package("uninstall-unix-404", "sws")],
        &APPLY,
    );
    assert_eq!(statuses(&report), [DeleteStatus::AlreadyGone]);
    assert!(!report.refused);
}

/// Two entries can name one copy. A copy an entry that stays still names
/// stays in the set, while the part's other files go.
#[test]
fn a_copy_a_backup_that_stays_still_uses_stays() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = uninstall_set_of(
        &resource,
        id,
        &[
            (
                "sws",
                &[
                    ("UserPlugins/reaper_sws-x64.dll", b"sws plug-in"),
                    ("S&M.ini", b"settings"),
                ],
            ),
            ("ffmpeg", &[("UserPlugins/avcodec-61.dll", b"ffmpeg codec")]),
        ],
    );
    // FFmpeg's record names SWS's plug-in's copy too.
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    let mut shared = manifest
        .files
        .iter()
        .find(|entry| entry.original_path.ends_with("reaper_sws-x64.dll"))
        .cloned()
        .unwrap();
    shared.package_id = Some("ffmpeg".to_string());
    manifest.files.push(shared);
    save_backup_manifest(&set, &manifest).unwrap();

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert_eq!(report.sets[0].bytes_freed, 8);
    assert!(report.sets[0].files_left.is_empty());
    assert_eq!(
        fs::read(set.join("UserPlugins/reaper_sws-x64.dll")).unwrap(),
        b"sws plug-in"
    );
    assert!(!set.join("S&M.ini").exists());
    let manifest = load_backup_manifest(&set).unwrap().unwrap();
    assert_eq!(manifest.files.len(), 2);
    assert!(
        manifest
            .files
            .iter()
            .all(|entry| entry.package_id.as_deref() == Some("ffmpeg"))
    );
}

/// A file from outside the resource folder sits in the set under
/// `external/<n>`, with a note of where it came from beside it. Both go
/// with their part, and so do the folders they leave empty; another
/// part's outside copy and its note stay.
#[test]
fn an_outside_files_origin_note_goes_with_its_copy() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    fs::create_dir_all(&resource).unwrap();
    let clap = dir.path().join("CLAP");
    let id = "uninstall-unix-1";
    let mut writer = BackupSetWriter::new(&resource, id, UNINSTALL_BACKUP_REASON);
    for (package_id, name) in [("sws", "sws.clap"), ("ffmpeg", "ffmpeg.clap")] {
        let file = clap.join(name);
        write(&file, name.as_bytes());
        writer
            .record_package(package_id, uninstall_record(package_id))
            .unwrap();
        writer
            .move_file(Some(package_id), &file, None, false)
            .unwrap();
    }
    let set = writer.path().to_path_buf();
    let note = set.join("external/1/sws.clap.rabbit-origin");
    assert!(note.is_file());
    let note_bytes = fs::metadata(&note).unwrap().len();

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert_eq!(report.sets[0].bytes_freed, 8 + note_bytes);
    assert!(!set.join("external/1").exists());
    assert!(set.join("external/2/ffmpeg.clap").is_file());
    assert!(set.join("external/2/ffmpeg.clap.rabbit-origin").is_file());
    assert_eq!(listed_backups(&resource, id), [package(id, "ffmpeg")]);
}

/// Folders a part leaves empty go, deepest first, but never a folder that
/// still holds a file, nor the set's own folder, which only a delete of the
/// whole set takes. The guard of a set that stays allows only what is
/// inside it.
#[test]
fn empty_folders_go_but_never_the_set_folder() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = uninstall_set_of(
        &resource,
        id,
        &[
            (
                "sws",
                &[
                    ("UserPlugins/sws/deep/er/a.dll", b"a"),
                    ("UserPlugins/sws/b.dll", b"b"),
                    ("c.ini", b"c"),
                ],
            ),
            ("ffmpeg", &[("UserPlugins/avcodec-61.dll", b"ffmpeg")]),
        ],
    );

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert!(!set.join("UserPlugins/sws").exists());
    assert!(set.join("UserPlugins/avcodec-61.dll").is_file());
    assert!(!set.join("c.ini").exists());
    assert!(set.is_dir());
    let guard = DeleteGuard::in_backups(&resource, id);
    assert!(!guard.allows_inside(&set));
    assert!(guard.allows_inside(&set.join("UserPlugins").join("a.dll")));
    assert!(!guard.allows_inside(&set.join("..").join("unix-2")));
    assert!(!guard.allows_inside(&set_path(&resource, "uninstall-unix-10")));
    assert!(!guard.allows_inside(&deleting_path(&resource, id).join("a.dll")));
    assert!(!guard.allows_inside(&resource.join("RABBIT")));
}

/// A link where a part's copy should be is removed as a link, and what it
/// leads to stays as it was. A folder on the way that is a link is never
/// followed: the file below it stays where it is, the link stays in the
/// set, and the report names the file. A delete of the whole set later
/// removes that link as a link too.
#[test]
fn links_in_a_part_are_never_followed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("precious.dll"), &[7; 64]);
    write(&elsewhere.join("grooves.txt"), b"keep me");
    let id = "uninstall-unix-1";
    let set = uninstall_set_of(
        &resource,
        id,
        &[
            (
                "sws",
                &[
                    ("UserPlugins/reaper_sws-x64.dll", b"sws plug-in"),
                    ("Data/SWS/grooves.txt", b"grooves"),
                ],
            ),
            ("ffmpeg", &[("UserPlugins/avcodec-61.dll", b"ffmpeg codec")]),
        ],
    );
    let plugin = set.join("UserPlugins/reaper_sws-x64.dll");
    fs::remove_file(&plugin).unwrap();
    link_folder(&plugin, &elsewhere);
    let sws_data = set.join("Data/SWS");
    fs::remove_dir_all(&sws_data).unwrap();
    link_folder(&sws_data, &elsewhere);
    let outside = snapshot(&elsewhere);

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(
        statuses(&report),
        [DeleteStatus::DeletedFromSet],
        "{report:#?}"
    );
    let done = &report.sets[0];
    assert_eq!(done.bytes_freed, 0);
    assert_eq!(done.files_left, [set.join("Data/SWS/grooves.txt")]);
    assert!(done.error.as_deref().unwrap().contains("SWS"));
    assert!(fs::symlink_metadata(&plugin).is_err());
    assert!(
        fs::symlink_metadata(&sws_data)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(snapshot(&elsewhere), outside);
    assert!(set.join("UserPlugins/avcodec-61.dll").is_file());
    assert_eq!(listed_backups(&resource, id), [package(id, "ffmpeg")]);
    assert_eq!(saved(&report)["sets"][0]["files_left"][0], {
        let left = set.join("Data").join("SWS").join("grooves.txt");
        serde_json::Value::String(left.to_string_lossy().into_owned())
    });

    let report = run(&resource, &[id], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    assert!(!set.exists());
    assert_eq!(snapshot(&elsewhere), outside);
}

/// What a test saw right after a set's manifest was saved without a part:
/// the backups the set listed, and whether the part's file was still there.
type SeenAtSave = (Vec<BackupItem>, bool);

/// A crash right after the manifest was saved without a part leaves its
/// files in the set, with no entry naming them. The set then lists as the
/// backups that stay, its size counts those files, and what stays
/// restores. A delete of the whole set takes them too.
#[test]
fn files_a_crash_left_count_in_the_set_and_go_with_it() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);

    // Where a run could stop: the manifest is saved, and no file of the
    // part has gone yet. The set already lists without it.
    let seen: Arc<Mutex<Vec<SeenAtSave>>> = Arc::default();
    let sink = Arc::clone(&seen);
    let resource_seen = resource.clone();
    let seam = delete_seam::install(move |step, path| {
        if step == Step::PartsSaved {
            sink.lock().unwrap().push((
                listed_backups(&resource_seen, "uninstall-unix-2"),
                path.join("UserPlugins/reaper_sws-x64.dll").is_file(),
            ));
        }
    });
    let other = sws_and_ffmpeg(&resource, "uninstall-unix-2");
    run_items(
        &resource,
        &[DeleteItem::package("uninstall-unix-2", "sws")],
        &APPLY,
    );
    drop(seam);
    assert_eq!(
        *seen.lock().unwrap(),
        [(vec![package("uninstall-unix-2", "ffmpeg")], true)]
    );
    assert!(!other.join("UserPlugins/reaper_sws-x64.dll").exists());

    // What a crash at that point leaves: SWS's entries and record are gone
    // from the manifest, and its files are still in the set.
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    manifest.packages.remove("sws");
    manifest
        .files
        .retain(|entry| entry.package_id.as_deref() != Some("sws"));
    save_backup_manifest(&set, &manifest).unwrap();
    let on_disk = file_bytes(&set);
    let listed = overview_of_with(&resource, id, &BackupEnvironment::AS_USER).unwrap();

    assert_eq!(listed.sets[0].kind, BackupSetKind::Uninstall);
    assert_eq!(listed.sets[0].size, on_disk);
    assert!(set.join("UserPlugins/reaper_sws-x64.dll").is_file());
    assert_eq!(listed.sets[0].warning, None);
    assert_eq!(listed.entries.len(), 1);
    assert_eq!(listed.entries[0].item, package(id, "ffmpeg"));
    assert_eq!(listed.entries[0].status, BackupStatus::CanRestore);
    assert_eq!(listed.entries[0].size, 12);

    let report = run(&resource, &[id], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    assert_eq!(report.sets[0].bytes_freed, on_disk);
    assert!(!set.exists());
}

/// A dry run of a part says it would go out of its set, how big it is and
/// that it is the only copy, and writes nothing at all. A dry run of the
/// last backup a set holds says the whole set would go.
#[test]
fn a_dry_run_of_a_part_writes_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let before = snapshot(dir.path());

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &DRY_RUN);

    assert_eq!(statuses(&report), [DeleteStatus::WouldDeleteFromSet]);
    let would = &report.sets[0];
    assert_eq!(would.size, 18);
    assert_eq!(would.bytes_freed, 0);
    assert_eq!(would.only_copy, [package(id, "sws")]);
    assert_eq!(would.set_path.as_deref(), Some(set.as_path()));
    assert!(report.saved_report.is_none());
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["sets"][0]["status"], "would-delete-from-set");

    let report = run_items(
        &resource,
        &[
            DeleteItem::package(id, "ffmpeg"),
            DeleteItem::package(id, "sws"),
        ],
        &DRY_RUN,
    );

    assert_eq!(
        statuses(&report),
        [DeleteStatus::WouldDelete, DeleteStatus::WouldDelete]
    );
    assert_eq!(report.sets[0].size, file_bytes(&set));
    assert_eq!(snapshot(dir.path()), before);
    let rabbit = resource.join("RABBIT");
    for folder in [DELETING_DIR, "locks", "logs"] {
        assert!(!rabbit.join(folder).exists(), "{folder}");
    }
}

/// A part of a set that changed since it was listed refuses the whole run
/// before anything changes.
#[test]
fn a_part_of_a_changed_set_is_refused() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let fingerprint = overview_of_with(&resource, id, &BackupEnvironment::AS_USER)
        .unwrap()
        .sets[0]
        .fingerprint
        .clone()
        .unwrap();
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    manifest.packages.get_mut("ffmpeg").unwrap().with_settings = Some(false);
    save_backup_manifest(&set, &manifest).unwrap();
    let before = snapshot(dir.path());

    let report = run_items(
        &resource,
        &[DeleteItem::package(id, "sws").with_fingerprint(fingerprint)],
        &APPLY,
    );

    assert!(report.refused);
    assert_eq!(
        report.sets[0].refusal.as_ref().unwrap().code(),
        "changed-since-listed"
    );
    assert_eq!(snapshot(dir.path()), before);
}

/// A set of another account, as one made with sudo on a Mac, refuses a
/// part of it as it refuses the set, before anything changes, in a dry run
/// too. The check is the environment's, so this runs on every platform.
#[test]
fn another_accounts_set_refuses_a_part_of_it_too() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    sws_and_ffmpeg(&resource, id);
    let roots = set_path(&resource, id);
    let owner_of = |path: &Path| Some(if path == roots { 0 } else { 501 });
    let under_sudo = BackupEnvironment {
        owner_of: &owner_of,
        ..BackupEnvironment::AS_USER
    };
    let before = snapshot(dir.path());

    for options in [APPLY, DRY_RUN] {
        let report = run_with(
            &resource,
            &[DeleteItem::keymap(id), DeleteItem::package(id, "sws")],
            &options,
            &under_sudo,
            &ProgressReporter::noop(),
            &CancelToken::new(),
        );

        assert!(report.refused);
        for set in &report.sets {
            assert_eq!(set.status, DeleteStatus::Refused);
            assert_eq!(set.refusal.as_ref().unwrap().code(), "not-yours");
        }
        assert_eq!(snapshot(dir.path()), before);
    }
}

/// An unfinished deletion of the set's id is no part of the set: taking a
/// package out leaves it as it is, and the report doesn't name it. A delete
/// of the whole id finishes it.
#[test]
fn a_part_leaves_an_unfinished_deletion_of_its_id_alone() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let left = deleting_path(&resource, id);
    write(&left.join("left.dll"), b"left");

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert_eq!(report.sets[0].unfinished_deletion, None);
    assert!(!report.sets[0].finished_earlier_deletion);
    assert!(left.join("left.dll").is_file());

    let report = run(&resource, &[id], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    assert!(report.sets[0].finished_earlier_deletion);
    assert!(!set.exists() && !left.exists());
}

/// The stop is read between sets only: the parts of the set being taken
/// apart all go, and the next set isn't reached.
#[test]
fn a_stop_comes_between_sets_never_inside_one() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = uninstall_set_of(
        &resource,
        id,
        &[
            ("sws", &[("UserPlugins/reaper_sws-x64.dll", b"sws plug-in")]),
            ("ffmpeg", &[("UserPlugins/avcodec-61.dll", b"ffmpeg codec")]),
            (
                "reapack",
                &[("UserPlugins/reaper_reapack-x64.dll", b"reapack")],
            ),
        ],
    );
    let second = layout_set(&resource, "unix-2", &[("b.dll", b"b")]);
    let second_before = snapshot(&second);
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let progress = ProgressReporter::new(move |event| {
        if matches!(event, ProgressEvent::DeleteStarted { .. }) {
            stop.cancel();
        }
    });

    let report = run_with(
        &resource,
        &[
            DeleteItem::package(id, "sws"),
            DeleteItem::new("unix-2"),
            DeleteItem::package(id, "ffmpeg"),
        ],
        &APPLY,
        &BackupEnvironment::AS_USER,
        &progress,
        &cancel,
    );

    assert!(report.stopped);
    assert_eq!(
        statuses(&report),
        [
            DeleteStatus::DeletedFromSet,
            DeleteStatus::NotReached,
            DeleteStatus::DeletedFromSet
        ]
    );
    assert_eq!(listed_backups(&resource, id), [package(id, "reapack")]);
    assert!(!set.join("UserPlugins/avcodec-61.dll").exists());
    assert_eq!(snapshot(&second), second_before);
    assert_eq!(report.sets[1].size, file_bytes(&second));
}

/// On Windows, a file of the part that another program holds open can't be
/// removed: the part is gone from the manifest all the same, its other
/// files go, and the report names the file that stays. The set's size
/// still counts it, and a delete of the whole set takes it once it is
/// closed.
#[cfg(windows)]
#[test]
fn an_open_file_of_a_part_stays_in_the_set_and_the_report_says_so() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let held_file = set.join("UserPlugins/reaper_sws-x64.dll");
    let held = hold_open(&held_file, 0);

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);
    drop(held);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    let done = &report.sets[0];
    assert_eq!(done.bytes_freed, 7);
    assert_eq!(done.files_left, std::slice::from_ref(&held_file));
    assert!(
        done.error
            .as_deref()
            .unwrap()
            .contains("reaper_sws-x64.dll")
    );
    assert!(held_file.is_file());
    assert!(!set.join("Data").exists());
    let listed = overview_of_with(&resource, id, &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(listed.entries.len(), 1);
    assert_eq!(listed.entries[0].item, package(id, "ffmpeg"));
    assert_eq!(listed.sets[0].size, file_bytes(&set));
    assert_eq!(
        saved(&report)["sets"][0]["files_left"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let report = run(&resource, &[id], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    assert!(!set.exists());
}

/// On Unix, a file of the part in a folder RABBIT may not change stays: the
/// part is gone from the manifest all the same, its other files go, and
/// the report names the file that stays. A delete of the whole set takes it
/// once the folder may be changed.
#[cfg(unix)]
#[test]
fn a_file_of_a_part_rabbit_may_not_remove_stays_in_the_set() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempdir().unwrap();
    if running_as_root(dir.path()) {
        return;
    }
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = uninstall_set_of(
        &resource,
        id,
        &[
            (
                "sws",
                &[
                    ("Locked/reaper_sws.dylib", b"sws plug-in"),
                    ("Data/grooves.txt", b"grooves"),
                ],
            ),
            ("ffmpeg", &[("UserPlugins/avcodec.dylib", b"ffmpeg codec")]),
        ],
    );
    let locked = set.join("Locked");
    let _mode_back = ModeBack(vec![locked.clone()]);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert_eq!(report.sets[0].bytes_freed, 7);
    assert_eq!(report.sets[0].files_left, [locked.join("reaper_sws.dylib")]);
    assert!(locked.join("reaper_sws.dylib").is_file());
    assert!(!set.join("Data").exists());
    assert_eq!(listed_backups(&resource, id), [package(id, "ffmpeg")]);

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let report = run(&resource, &[id], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::Deleted]);
    assert!(!set.exists());
}

/// Each backup the overview lists maps to the items that delete exactly
/// it: a package or a key map alone, a set's own backup and an unfinished
/// deletion their set whole, and the records each of their sets whole. An
/// item names its part as a restore's item does, and one that names none
/// is no item at all, so it never widens to a whole set.
#[test]
fn each_backup_maps_to_the_items_that_delete_exactly_it() {
    use crate::rollback::RestoreItem;
    assert_eq!(
        DeleteItem::for_backup(&package("uninstall-unix-1", "sws")),
        [DeleteItem::package("uninstall-unix-1", "sws")]
    );
    assert_eq!(
        DeleteItem::for_backup(&keymap("unix-1")),
        [DeleteItem::keymap("unix-1")]
    );
    for whole in [
        BackupItem::Set {
            set_id: "unix-1".to_string(),
        },
        BackupItem::Deleting {
            set_id: "unix-1".to_string(),
        },
    ] {
        assert_eq!(DeleteItem::for_backup(&whole), [DeleteItem::new("unix-1")]);
    }
    assert_eq!(
        DeleteItem::for_backup(&BackupItem::Records {
            set_ids: vec!["a-1".to_string(), "a-2".to_string()]
        }),
        [DeleteItem::new("a-1"), DeleteItem::new("a-2")]
    );

    let json = serde_json::to_value(DeleteItem::package("uninstall-unix-1", "sws")).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "set_id": "uninstall-unix-1",
            "part": { "kind": "package", "package_id": "sws" }
        })
    );
    assert_eq!(
        json["part"],
        serde_json::to_value(RestoreItem::package("uninstall-unix-1", "sws")).unwrap()["part"]
    );
    for (item, restore) in [
        (DeleteItem::keymap("unix-1"), RestoreItem::keymap("unix-1")),
        (DeleteItem::new("unix-1"), RestoreItem::whole_set("unix-1")),
    ] {
        assert_eq!(
            serde_json::to_value(item).unwrap()["part"],
            serde_json::to_value(restore).unwrap()["part"]
        );
    }
    for without_part in [
        serde_json::json!({ "set_id": "unix-1" }),
        serde_json::json!({ "set_id": "unix-1", "package_id": "sws" }),
    ] {
        assert!(serde_json::from_value::<DeleteItem>(without_part).is_err());
    }
    let back: DeleteItem = serde_json::from_value(json).unwrap();
    assert_eq!(back, DeleteItem::package("uninstall-unix-1", "sws"));
}

/// A restore's set that holds the key map and a file no package or key map
/// of it holds lists both: the key map, and the set's own backup for that
/// file. Taking the key map out leaves the set and its own backup in
/// place, in one save of its manifest and no rename: a set goes whole only
/// when every backup it lists is named, its own among them. Naming the set
/// with the key map takes it whole.
#[test]
fn a_set_that_lists_its_own_backup_stays_when_its_key_map_goes() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let keymap_and_more = |id: &str| {
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"own keys");
        let other = resource.join("UserPlugins/other.dll");
        write(&other, b"other plug-in");
        let mut writer = BackupSetWriter::new(&resource, id, BEFORE_RESTORE_REASON);
        writer.copy_file(Some("osara"), &keymap).unwrap();
        writer.copy_file(None, &other).unwrap();
        writer.path().to_path_buf()
    };
    let id = "unix-1";
    let set = keymap_and_more(id);
    let own = BackupItem::Set {
        set_id: id.to_string(),
    };
    assert_eq!(listed_backups(&resource, id), [keymap(id), own.clone()]);
    let steps: Arc<Mutex<Vec<Step>>> = Arc::default();
    let seen = Arc::clone(&steps);
    let seam = delete_seam::install(move |step, _| {
        if matches!(step, Step::Renamed | Step::PartsSaved) {
            seen.lock().unwrap().push(step);
        }
    });

    let report = run_items(&resource, &[DeleteItem::keymap(id)], &APPLY);
    drop(seam);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert_eq!(*steps.lock().unwrap(), [Step::PartsSaved]);
    assert_eq!(report.sets[0].bytes_freed, 8);
    assert!(!set.join("reaper-kb.ini").exists());
    assert_eq!(
        fs::read(set.join("UserPlugins/other.dll")).unwrap(),
        b"other plug-in"
    );
    let manifest = load_backup_manifest(&set).unwrap().unwrap();
    assert_eq!(manifest.files.len(), 1);
    assert!(manifest.files[0].original_path.ends_with("other.dll"));
    assert_eq!(listed_backups(&resource, id), [own]);

    let id = "unix-2";
    let set = keymap_and_more(id);

    let report = run_items(
        &resource,
        &[DeleteItem::keymap(id), DeleteItem::new(id)],
        &APPLY,
    );

    assert_eq!(
        statuses(&report),
        [DeleteStatus::Deleted, DeleteStatus::Deleted]
    );
    assert!(!set.exists());
}

/// On Unix, a set whose manifest RABBIT may not save again, because it may
/// not write in the set's folder, keeps the part: the part isn't deleted
/// for permission, nothing of the set changes, no staging file is left,
/// and the set still lists the part. The run still saves its report.
#[cfg(unix)]
#[test]
fn a_manifest_rabbit_may_not_save_leaves_the_set_as_it_was() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempdir().unwrap();
    if running_as_root(dir.path()) {
        return;
    }
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let before = snapshot(&set);
    let _mode_back = ModeBack(vec![set.clone()]);
    fs::set_permissions(&set, fs::Permissions::from_mode(0o555)).unwrap();

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::NotDeletedFromSet]);
    let done = &report.sets[0];
    assert_eq!(done.not_deleted, Some(NotDeletedReason::Permission));
    assert_eq!(done.bytes_freed, 0);
    assert!(
        done.error
            .as_deref()
            .unwrap()
            .contains("backup-manifest.json")
    );
    assert!(done.files_left.is_empty());
    assert_eq!(snapshot(&set), before);
    assert!(!set.join("backup-manifest.json.rabbit-tmp").exists());
    assert_eq!(
        listed_backups(&resource, id),
        [package(id, "ffmpeg"), package(id, "sws")]
    );
    let json = saved(&report);
    assert_eq!(json["sets"][0]["status"], "not-deleted-from-set");
    assert_eq!(json["sets"][0]["not_deleted"], "permission");
}

/// On Windows, a set whose manifest another program holds open, letting
/// others only read it, keeps the part: Windows refuses to replace the
/// manifest, the part isn't deleted because the manifest is in use, and
/// nothing of the set changes, with no staging file left.
#[cfg(windows)]
#[test]
fn a_manifest_another_program_holds_open_leaves_the_set_as_it_was() {
    const SHARE_READ: u32 = 0x1;
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let before = snapshot(&set);
    let held = hold_open(&set.join("backup-manifest.json"), SHARE_READ);

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);
    drop(held);

    assert_eq!(statuses(&report), [DeleteStatus::NotDeletedFromSet]);
    let done = &report.sets[0];
    assert_eq!(done.not_deleted, Some(NotDeletedReason::InUse));
    assert_eq!(done.bytes_freed, 0);
    assert!(done.error.is_some());
    assert_eq!(snapshot(&set), before);
    assert!(!set.join("backup-manifest.json.rabbit-tmp").exists());
    assert_eq!(
        listed_backups(&resource, id),
        [package(id, "ffmpeg"), package(id, "sws")]
    );
    assert_eq!(saved(&report)["sets"][0]["not_deleted"], "in-use");
}

/// A staging file an earlier crash left beside the manifest, or a link
/// there, is removed as itself before the manifest is saved: the save
/// never writes through a link, and what a link there leads to stays as it
/// was.
#[test]
fn a_leftover_staging_file_or_link_goes_as_itself_before_the_save() {
    for as_link in [false, true] {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let elsewhere = dir.path().join("Elsewhere");
        write(&elsewhere.join("keep.dll"), b"keep");
        let id = "uninstall-unix-1";
        let set = sws_and_ffmpeg(&resource, id);
        let staged = set.join("backup-manifest.json.rabbit-tmp");
        if as_link {
            link_folder(&staged, &elsewhere);
        } else {
            write(&staged, b"{ half a manifest");
        }
        let outside = snapshot(&elsewhere);

        let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

        assert_eq!(
            statuses(&report),
            [DeleteStatus::DeletedFromSet],
            "{as_link}"
        );
        assert!(fs::symlink_metadata(&staged).is_err(), "{as_link}");
        let manifest = load_backup_manifest(&set).unwrap().unwrap();
        assert_eq!(manifest.packages.keys().collect::<Vec<_>>(), ["ffmpeg"]);
        assert_eq!(snapshot(&elsewhere), outside, "{as_link}");
        assert_eq!(listed_backups(&resource, id), [package(id, "ffmpeg")]);
    }
}

/// A part's entry may name, by another spelling, a file the set still
/// needs: its manifest, or a copy an entry that stays uses, in another case
/// or, on Windows, by its short name, or on macOS in another Unicode form.
/// Such a file is compared by what it is on disk and stays, so the set
/// keeps its manifest and the backups in it, and the part goes all the
/// same.
#[test]
fn the_manifest_and_copies_that_stay_are_never_removed_by_another_name() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    let sws_entry = manifest
        .files
        .iter()
        .find(|entry| entry.package_id.as_deref() == Some("sws"))
        .cloned()
        .unwrap();
    #[allow(unused_mut)]
    let mut aliases = vec![
        set.join("Backup-Manifest.JSON"),
        set.join("UserPlugins").join("AVCODEC-61.DLL"),
    ];
    // Short names, where the volume makes them. Where it doesn't, these
    // name nothing, which is nothing to remove either.
    #[cfg(windows)]
    aliases.extend([
        set.join("BACKUP~1.JSO"),
        set.join("UserPlugins").join("AVCODE~1.DLL"),
    ]);
    // A copy an entry that stays uses, named in another Unicode form, which
    // macOS's file systems open as the same file.
    #[cfg(target_os = "macos")]
    {
        let accented = set.join("UserPlugins").join("caf\u{e9}.dll");
        write(&accented, b"cafe");
        let mut kept = sws_entry.clone();
        kept.package_id = Some("ffmpeg".to_string());
        kept.original_path = PathBuf::from("UserPlugins/caf\u{e9}.dll");
        kept.backup_path = accented;
        kept.size = 4;
        kept.sha256 = crate::hash::sha256_bytes(b"cafe");
        manifest.files.push(kept);
        aliases.push(set.join("UserPlugins").join("cafe\u{301}.dll"));
    }
    for (nth, alias) in aliases.into_iter().enumerate() {
        let mut entry = sws_entry.clone();
        entry.original_path = PathBuf::from(format!("UserPlugins/alias-{nth}.dll"));
        entry.backup_path = alias;
        manifest.files.push(entry);
    }
    save_backup_manifest(&set, &manifest).unwrap();

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(
        statuses(&report),
        [DeleteStatus::DeletedFromSet],
        "{report:#?}"
    );
    assert_eq!(report.sets[0].bytes_freed, 18);
    assert!(report.sets[0].files_left.is_empty(), "{report:#?}");
    assert!(!set.join("UserPlugins/reaper_sws-x64.dll").exists());
    let manifest = load_backup_manifest(&set).unwrap().unwrap();
    assert_eq!(manifest.packages.keys().collect::<Vec<_>>(), ["ffmpeg"]);
    assert!(
        manifest
            .files
            .iter()
            .all(|entry| entry.package_id.as_deref() == Some("ffmpeg"))
    );
    assert_eq!(
        fs::read(set.join("UserPlugins/avcodec-61.dll")).unwrap(),
        b"ffmpeg codec"
    );
    #[cfg(target_os = "macos")]
    assert_eq!(
        fs::read(set.join("UserPlugins").join("caf\u{e9}.dll")).unwrap(),
        b"cafe"
    );
    assert_eq!(listed_backups(&resource, id), [package(id, "ffmpeg")]);
    assert_eq!(
        restore_package(&resource, id, "ffmpeg"),
        RestoreStatus::Restored
    );
}

/// A part's save changes only what the part takes out of the manifest:
/// fields a newer RABBIT wrote, which this one doesn't know, stay as they
/// are, at the top, in the records of a package that stays and in its
/// entries, and so does a key map's origin this one doesn't know.
#[test]
fn what_this_rabbit_doesnt_know_of_a_manifest_stays_in_it() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let path = set.join("backup-manifest.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    json["written_by_a_newer_rabbit"] = serde_json::json!({ "kept": true });
    json["keymap_origin"] = serde_json::json!("an-origin-from-later");
    json["packages"]["ffmpeg"]["newer_record"] = serde_json::json!(7);
    json["packages"]["sws"]["newer_record"] = serde_json::json!(8);
    for entry in json["files"].as_array_mut().unwrap() {
        entry["newer_entry"] = entry["package_id"].clone();
    }
    fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    let after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        after["written_by_a_newer_rabbit"],
        serde_json::json!({ "kept": true })
    );
    assert_eq!(after["keymap_origin"], "an-origin-from-later");
    assert_eq!(after["packages"]["ffmpeg"]["newer_record"], 7);
    assert!(after["packages"].get("sws").is_none());
    let files = after["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["newer_entry"], "ffmpeg");
    assert_eq!(listed_backups(&resource, id), [package(id, "ffmpeg")]);
}

/// On Windows, the last package of a set takes the set whole, and when a
/// file of it is opened right after the rename, the walk leaves part of
/// it: the package's item says so as the set's, partly deleted, with its
/// own part and the set's folders, so the CLI and the tab can tell the
/// whole set went. Naming the same package again is refused, since only a
/// delete of the whole id finishes such a deletion, which it then does.
#[cfg(windows)]
#[test]
fn a_last_part_that_leaves_its_set_partly_deleted_says_so_as_the_set() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);
    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    let held: std::rc::Rc<std::cell::RefCell<Option<fs::File>>> = std::rc::Rc::default();
    let holder = std::rc::Rc::clone(&held);
    let seam = delete_seam::install(move |step, folder| {
        if step == Step::Renamed {
            *holder.borrow_mut() = Some(hold_open(&folder.join("UserPlugins/avcodec-61.dll"), 0));
        }
    });

    let report = run_items(&resource, &[DeleteItem::package(id, "ffmpeg")], &APPLY);
    drop(seam);

    assert_eq!(statuses(&report), [DeleteStatus::PartlyDeleted]);
    let left = deleting_path(&resource, id);
    let ended = &report.sets[0];
    assert_eq!(
        ended.part,
        DeletePart::Package {
            package_id: "ffmpeg".to_string()
        }
    );
    assert_eq!(ended.set_path.as_deref(), Some(set.as_path()));
    assert_eq!(ended.left_in.as_deref(), Some(left.as_path()));
    assert!(!set.exists());

    let again = run_items(&resource, &[DeleteItem::package(id, "ffmpeg")], &APPLY);
    assert!(again.refused);
    assert!(
        matches!(
            &again.sets[0].refusal,
            Some(RestoreRefusal::BeingDeleted { backup_id }) if backup_id == id
        ),
        "{again:#?}"
    );
    held.borrow_mut().take();

    let report = run(&resource, &[id], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::FinishedEarlierDeletion]);
    assert!(!left.exists());
}

/// On Windows, a name that ends in a dot or a space opens the folder
/// without it, so a part's entry under `Sub.` or `Other ` that RABBIT
/// leaves never has `Sub` or `Other` removed as a folder it left empty,
/// though both are empty. The entries' files are named as ones that stay.
#[cfg(windows)]
#[test]
fn a_folder_name_that_opens_another_folder_is_never_removed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    fs::create_dir_all(set.join("Sub")).unwrap();
    fs::create_dir_all(set.join("Other")).unwrap();
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    let sws_entry = manifest
        .files
        .iter()
        .find(|entry| entry.package_id.as_deref() == Some("sws"))
        .cloned()
        .unwrap();
    for (nth, folder) in ["Sub.", "Other "].into_iter().enumerate() {
        let mut entry = sws_entry.clone();
        entry.original_path = PathBuf::from(format!("UserPlugins/odd-{nth}.dll"));
        entry.backup_path = set.join(folder).join("x.dll");
        manifest.files.push(entry);
    }
    save_backup_manifest(&set, &manifest).unwrap();

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert!(set.join("Sub").is_dir());
    assert!(set.join("Other").is_dir());
    assert!(!set.join("UserPlugins/reaper_sws-x64.dll").exists());
    assert_eq!(report.sets[0].files_left.len(), 2, "{report:#?}");
}

/// On Windows, the manifest a part's delete saved and the set's folder are
/// flushed to disk while the run holds the set and the folders above it,
/// so the save is on disk before any copy goes, also on FAT or exFAT.
#[cfg(windows)]
#[test]
fn the_saved_manifest_and_its_set_are_flushed_while_held() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let held = super::hold_way_to_set(&resource, id).unwrap();

    super::flush_saved_manifest(&set).unwrap();
    drop(held);
}

/// An update's set `id` as RABBIT 0.4.x wrote one: the earlier versions of
/// ReaPack and SWS it replaced, 13 and 8 bytes, a copy of RABBIT's records
/// with an entry for it, under `receipt_of` when given, and the manifest's
/// `receipt_backup_path` naming that copy.
fn older_update_set(resource: &Path, id: &str, receipt_of: Option<&str>) -> PathBuf {
    let mut writer = BackupSetWriter::new(resource, id, UPDATE_BACKUP_REASON);
    for (package_id, file, contents) in [
        (
            "reapack",
            "UserPlugins/reaper_reapack-x64.dll",
            &b"reapack 1.0.0"[..],
        ),
        ("sws", "UserPlugins/reaper_sws-x64.dll", &b"sws 2.13"[..]),
    ] {
        let original = resource.join(file);
        write(&original, contents);
        writer.copy_file(Some(package_id), &original).unwrap();
    }
    let set = writer.path().to_path_buf();
    let copy = set.join("RABBIT").join("install-state.json");
    write(&copy, b"records of 0.4.4");
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    manifest.files.push(crate::rollback::BackupManifestFile {
        package_id: receipt_of.map(ToString::to_string),
        original_path: PathBuf::from("RABBIT").join("install-state.json"),
        backup_path: copy.clone(),
        size: 16,
        sha256: crate::hash::sha256_bytes(b"records of 0.4.4"),
        state: crate::rollback::BackupEntryState::Done,
    });
    manifest.receipt_backup_path = Some(copy);
    save_backup_manifest(&set, &manifest).unwrap();
    set
}

/// One package out of an older update's set, which holds a copy of
/// RABBIT's records, keeps that copy, also when its entry was recorded
/// under that package: the copy is the set's own, never a part of the
/// package's backup. The manifest still names it, and the other earlier
/// version stays listed.
#[test]
fn a_part_out_of_an_older_update_set_keeps_its_receipt_copy() {
    for receipt_of in [None, Some("sws")] {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let id = "unix-1";
        let set = older_update_set(&resource, id, receipt_of);
        let copy = set.join("RABBIT").join("install-state.json");
        assert_eq!(
            listed_backups(&resource, id),
            [package(id, "reapack"), package(id, "sws")]
        );

        let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

        assert_eq!(
            statuses(&report),
            [DeleteStatus::DeletedFromSet],
            "{receipt_of:?}"
        );
        assert!(report.sets[0].files_left.is_empty(), "{report:#?}");
        assert_eq!(fs::read(&copy).unwrap(), b"records of 0.4.4");
        assert!(!set.join("UserPlugins/reaper_sws-x64.dll").exists());
        assert_eq!(
            fs::read(set.join("UserPlugins/reaper_reapack-x64.dll")).unwrap(),
            b"reapack 1.0.0"
        );
        let manifest = load_backup_manifest(&set).unwrap().unwrap();
        assert_eq!(
            manifest.receipt_backup_path.as_deref(),
            Some(copy.as_path())
        );
        assert!(
            manifest.files.iter().any(|entry| entry.backup_path == copy),
            "{receipt_of:?}"
        );
        assert_eq!(listed_backups(&resource, id), [package(id, "reapack")]);
    }
}

/// A part's delete changes nothing outside its own set: not the resource
/// folder, not another set, not even a file a manifest entry of the part
/// names outside the set, nor one it names in another set's folder, which
/// is read as the same place in this set.
#[test]
fn a_part_changes_nothing_outside_its_set() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let id = "uninstall-unix-1";
    let set = sws_and_ffmpeg(&resource, id);
    let other = sws_and_ffmpeg(&resource, "uninstall-unix-2");
    let live = resource.join("UserPlugins/live.dll");
    write(&live, b"in use by REAPER");
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    let sws_entry = manifest
        .files
        .iter()
        .find(|entry| entry.package_id.as_deref() == Some("sws"))
        .cloned()
        .unwrap();
    for (nth, backup_path) in [
        live.clone(),
        other.join("UserPlugins").join("reaper_sws-x64.dll"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut entry = sws_entry.clone();
        entry.original_path = PathBuf::from(format!("UserPlugins/outside-{nth}.dll"));
        entry.backup_path = backup_path;
        manifest.files.push(entry);
    }
    save_backup_manifest(&set, &manifest).unwrap();
    let outside = everything_else(&resource);
    let other_before = snapshot(&other);

    let report = run_items(&resource, &[DeleteItem::package(id, "sws")], &APPLY);

    assert_eq!(statuses(&report), [DeleteStatus::DeletedFromSet]);
    assert!(!set.join("UserPlugins/reaper_sws-x64.dll").exists());
    assert_eq!(everything_else(&resource), outside);
    assert_eq!(snapshot(&other), other_before);
    assert_eq!(fs::read(&live).unwrap(), b"in use by REAPER");
    assert_eq!(listed_backups(&resource, id), [package(id, "ffmpeg")]);
}
