//! Tests of going back to an earlier version, the swap of backup plan 6.4:
//! only the package's own receipt entry changes, the newer version's own
//! files move out of the way, all or nothing, and it can be undone the same
//! way. Temporary folders only.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::restore_run_tests::{snapshot, snapshot_but_lock_folder};
use super::restore_seam::{self, Step};
use super::{
    BEFORE_RESTORE_REASON, BackupEnvironment, BackupSetWriter, MoveOutAction, OutsideRestoreRules,
    ReplacedPackageRecord, RestoreBackupActionKind, RestoreBackupOptions, RestoreBackupReport,
    RestoreItem, RestoreReportItem, RestoreRunOptions, RestoreStatus, SwappedEntry,
    UPDATE_BACKUP_REASON, load_backup_manifest, restore_backup_set_with, restore_backup_set_within,
    restore_backups_with, save_backup_manifest,
};
use crate::backup_overview::{
    BackupEntry, BackupGroup, BackupItem, BackupStatus, RestoreScope, overview_with,
};
use crate::cancel::CancelToken;
use crate::error::RabbitError;
use crate::hash::sha256_bytes;
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::receipt::{
    AppliedConfiguration, AppliedConfigurationOutcome, InstallState, InstalledFileReceipt,
    PackageReceipt, ReapackRepository, load_install_state, save_install_state,
};
use crate::restore_refusal::{RestoreRefusal, TargetRefusal, refusal};
use crate::uninstall::paths::link_folder;
use crate::version::Version;

pub(super) const PLUGIN: &str = "UserPlugins/reaper_reapack-x64.dll";
pub(super) const SHARED: &str = "UserPlugins/reapack_shared.dat";
pub(super) const NEW_ONLY: &str = "UserPlugins/reapack_new.dll";
const SET: &str = "unix-100";

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

/// A receipt entry for `id` at `at`, listing `files` inside the resource
/// folder with what they hold, as an install records them.
fn entry(id: &str, at: &str, files: &[(&str, &[u8])]) -> PackageReceipt {
    PackageReceipt {
        id: id.to_string(),
        version: Some(version(at)),
        source_url: None,
        source_sha256: None,
        variant: None,
        channel: None,
        installed_files: files
            .iter()
            .map(|(path, contents)| InstalledFileReceipt {
                path: PathBuf::from(path),
                sha256: Some(sha256_bytes(contents)),
                size: Some(contents.len() as u64),
            })
            .collect(),
        installed_at: Some(format!("unix-{at}")),
        rabbit_version: Some("0.6.0".to_string()),
        architecture: None,
        vendor_added_files: Vec::new(),
    }
}

/// ReaPack 1.2.4 as RABBIT installed it: its plug-in and a data file.
pub(super) fn reapack_old() -> PackageReceipt {
    entry(
        "reapack",
        "1.2.4",
        &[(PLUGIN, b"reapack 1.2.4"), (SHARED, b"shared")],
    )
}

/// ReaPack 1.2.5, from the dev channel: a new plug-in, the same data file,
/// and a file 1.2.4 didn't have.
pub(super) fn reapack_new() -> PackageReceipt {
    let mut new = entry(
        "reapack",
        "1.2.5",
        &[
            (PLUGIN, b"reapack 1.2.5"),
            (SHARED, b"shared"),
            (NEW_ONLY, b"new in 1.2.5"),
        ],
    );
    new.channel = Some("dev".to_string());
    new
}

/// RABBIT's records besides ReaPack's entry: another package, two
/// packages declined, ReaPack among them, and a configuration step.
pub(super) fn records_with(reapack: Option<PackageReceipt>) -> InstallState {
    let mut state = InstallState::default();
    state
        .packages
        .insert("sws".to_string(), entry("sws", "2.14", &[]));
    if let Some(reapack) = reapack {
        state.packages.insert("reapack".to_string(), reapack);
    }
    state.declined_packages.insert("reapack".to_string());
    state.declined_packages.insert("langpack-de".to_string());
    state.applied_configuration.insert(
        "reapack-repository".to_string(),
        AppliedConfiguration {
            outcome: AppliedConfigurationOutcome::Added,
            reapack_repository: Some(ReapackRepository {
                name: "OSARA".to_string(),
                url: "https://example.test/index.xml".to_string(),
            }),
            langpack: None,
            langpack_before: None,
        },
    );
    state
}

/// The update from ReaPack 1.2.4 to 1.2.5, as RABBIT does it now: the set
/// `SET` records 1.2.4's entry and holds its plug-in, which the update
/// replaced. On disk is 1.2.5, with its new file; the data file stayed.
fn updated_reapack(resource: &Path, earlier: Option<PackageReceipt>) {
    write(&resource.join(PLUGIN), b"reapack 1.2.4");
    write(&resource.join(SHARED), b"shared");
    let mut writer = BackupSetWriter::new(resource, SET, UPDATE_BACKUP_REASON);
    writer
        .record_replaced(
            "reapack",
            ReplacedPackageRecord {
                receipt: earlier,
                replaced_by: Some(version("1.2.5")),
            },
        )
        .unwrap();
    writer
        .copy_file(Some("reapack"), &resource.join(PLUGIN))
        .unwrap();
    write(&resource.join(PLUGIN), b"reapack 1.2.5");
    write(&resource.join(NEW_ONLY), b"new in 1.2.5");
    save_install_state(resource, &records_with(Some(reapack_new()))).unwrap();
}

fn options(dry_run: bool, package_id: Option<&str>) -> RestoreBackupOptions {
    RestoreBackupOptions {
        dry_run,
        allow_reaper_running: true,
        package_id: package_id.map(ToString::to_string),
    }
}

/// Restore `package_id` of the set `set_id`, or the whole set, as the user.
fn restore(
    resource: &Path,
    set_id: &str,
    package_id: Option<&str>,
    dry_run: bool,
) -> RestoreBackupReport {
    restore_backup_set_within(
        resource,
        set_id,
        &options(dry_run, package_id),
        &OutsideRestoreRules::default(),
    )
    .unwrap()
}

/// The quick plan of going back to `package_id` of the set `SET`, or of
/// the whole set, as the overview's restore would ask for it: a dry run
/// that reads no file's content.
fn quick_plan(resource: &Path, package_id: Option<&str>) -> RestoreReportItem {
    let mut report = restore_backups_with(
        resource,
        &[RestoreItem::new(SET, package_id)],
        &RestoreRunOptions {
            dry_run: true,
            quick: true,
            allow_reaper_running: true,
        },
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap();
    report.backups.remove(0)
}

/// Add `path`, holding `contents`, to ReaPack's entry in RABBIT's records
/// as a file 1.2.5 has and 1.2.4 doesn't, and write it there. A relative
/// path is inside the resource folder.
fn add_newer_file(resource: &Path, path: &Path, contents: &[u8]) {
    let place = if path.is_absolute() {
        path.to_path_buf()
    } else {
        resource.join(path)
    };
    write(&place, contents);
    let mut state = load_install_state(resource).unwrap().unwrap();
    state
        .packages
        .get_mut("reapack")
        .unwrap()
        .installed_files
        .push(InstalledFileReceipt {
            path: path.to_path_buf(),
            sha256: Some(sha256_bytes(contents)),
            size: Some(contents.len() as u64),
        });
    save_install_state(resource, &state).unwrap();
}

/// The overview's row for ReaPack in the set `SET`.
fn reapack_row(resource: &Path) -> BackupEntry {
    overview_with(resource, &BackupEnvironment::AS_USER)
        .unwrap()
        .entries
        .into_iter()
        .find(|entry| {
            entry.item
                == BackupItem::Package {
                    set_id: SET.to_string(),
                    package_id: "reapack".to_string(),
                }
        })
        .unwrap()
}

/// The sets in `RABBIT/backups`, by name.
fn sets_in(resource: &Path) -> Vec<String> {
    let mut sets: Vec<String> = fs::read_dir(resource.join("RABBIT/backups"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    sets.sort();
    sets
}

/// Restore `set_id`, or `package_id` of it, in a dry run and for real as
/// the user with `environment`, expecting the same refusal both times, and
/// check that nothing below `dir` changed: no file, folder or link, besides
/// the empty folder of the lock a real run refused under it leaves.
fn refused(
    dir: &Path,
    resource: &Path,
    set_id: &str,
    package_id: Option<&str>,
    environment: &BackupEnvironment<'_>,
) -> RabbitError {
    let before = snapshot(dir);
    let unlocked = snapshot_but_lock_folder(dir, resource);
    let mut errors = Vec::new();
    for dry_run in [true, false] {
        errors.push(
            restore_backup_set_with(
                resource,
                set_id,
                &options(dry_run, package_id),
                environment,
                OutsideRestoreRules::default,
            )
            .unwrap_err(),
        );
        let (after, expected) = if dry_run {
            (snapshot(dir), &before)
        } else {
            (snapshot_but_lock_folder(dir, resource), &unlocked)
        };
        assert_eq!(
            &after, expected,
            "{set_id} {package_id:?} changed something"
        );
    }
    let refused = errors.pop().unwrap();
    assert_eq!(errors[0].to_string(), refused.to_string());
    refused
}

fn read(resource: &Path, relative: &str) -> Vec<u8> {
    fs::read(resource.join(relative)).unwrap()
}

/// Going back to ReaPack 1.2.4 puts its plug-in back, moves 1.2.5's own
/// file out into the new backup, leaves the file both versions share, and
/// makes ReaPack's entry exactly 1.2.4's, on the channel it is on now, and
/// takes it off the packages declined. No other entry, nothing else
/// declined and no configuration step changes. The new backup records
/// 1.2.5's entry, lists as an earlier version, and going back to it puts
/// 1.2.5 back as it was, its own file included, while 1.2.4's plug-in goes
/// into another new backup.
#[test]
fn an_earlier_version_comes_back_and_the_newer_one_too() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));

    let back = restore(&resource, SET, Some("reapack"), false);

    assert_eq!(back.status, RestoreStatus::Restored);
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
    assert_eq!(read(&resource, SHARED), b"shared");
    assert!(!resource.join(NEW_ONLY).exists());
    let state = load_install_state(&resource).unwrap().unwrap();
    let mut expected = records_with(Some(reapack_old()));
    expected
        .packages
        .get_mut("reapack")
        .unwrap()
        .channel
        .clone_from(&reapack_new().channel);
    expected.declined_packages.remove("reapack");
    assert_eq!(state, expected);

    let swap = back.swap.clone().unwrap();
    assert_eq!(swap.package_id, "reapack");
    assert_eq!(swap.version, Some(version("1.2.4")));
    assert_eq!(swap.replaced_version, Some(version("1.2.5")));
    assert_eq!(swap.entry, SwappedEntry::Replaced);
    assert!(swap.undeclined);
    assert!(!swap.uninstall_needs_reinstall);
    assert!(swap.changed_left.is_empty() && swap.left_in_place.is_empty());
    assert_eq!(swap.moved_out.len(), 1);
    let moved = &swap.moved_out[0];
    assert_eq!(moved.path, resource.join(NEW_ONLY));
    assert_eq!(moved.action, MoveOutAction::MovedOut);
    assert_eq!(
        fs::read(moved.backup_path.as_ref().unwrap()).unwrap(),
        b"new in 1.2.5"
    );
    let kinds: Vec<RestoreBackupActionKind> =
        back.actions.iter().map(|action| action.action).collect();
    assert_eq!(kinds, [RestoreBackupActionKind::Restored]);

    // The new backup is an earlier version of its own: 1.2.5.
    let newer = back.before_restore_set_id.clone().unwrap();
    let manifest = load_backup_manifest(&resource.join("RABBIT/backups").join(&newer))
        .unwrap()
        .unwrap();
    assert_eq!(manifest.reason, BEFORE_RESTORE_REASON);
    assert_eq!(
        manifest.replaced["reapack"],
        ReplacedPackageRecord {
            receipt: Some(reapack_new()),
            replaced_by: Some(version("1.2.4")),
        }
    );
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let row = listed
        .entries
        .iter()
        .find(|entry| {
            entry.item
                == BackupItem::Package {
                    set_id: newer.clone(),
                    package_id: "reapack".to_string(),
                }
        })
        .unwrap();
    assert_eq!(row.group, BackupGroup::EarlierVersions);
    assert_eq!(row.status, BackupStatus::CanRestore);
    assert_eq!(row.version, Some(version("1.2.5")));
    assert_eq!(row.replaced_by, Some(version("1.2.4")));
    assert_eq!(row.restore_scope, Some(RestoreScope::Package));
    // The set going back came from now lists 1.2.4 as already back.
    let first = listed
        .entries
        .iter()
        .find(|entry| entry.item.set_id() == Some(SET))
        .unwrap();
    assert_eq!(first.status, BackupStatus::AlreadyBack);

    let forward = restore(&resource, &newer, Some("reapack"), false);

    assert_eq!(forward.status, RestoreStatus::Restored);
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.5");
    assert_eq!(read(&resource, NEW_ONLY), b"new in 1.2.5");
    assert_eq!(read(&resource, SHARED), b"shared");
    let state = load_install_state(&resource).unwrap().unwrap();
    let mut expected = records_with(Some(reapack_new()));
    expected.declined_packages.remove("reapack");
    assert_eq!(state, expected);
    let swap = forward.swap.unwrap();
    assert!(swap.moved_out.is_empty());
    assert_eq!(swap.version, Some(version("1.2.5")));
    // Off the list since the first restore.
    assert!(!swap.undeclined);
    let older = forward.before_restore_set_id.unwrap();
    assert_ne!(older, newer);
    let older_set = resource.join("RABBIT/backups").join(&older);
    let manifest = load_backup_manifest(&older_set).unwrap().unwrap();
    assert_eq!(manifest.files.len(), 1);
    assert_eq!(fs::read(older_set.join(PLUGIN)).unwrap(), b"reapack 1.2.4");
    assert_eq!(
        manifest.replaced["reapack"]
            .receipt
            .as_ref()
            .unwrap()
            .version,
        Some(version("1.2.4"))
    );
}

/// A dry run says what going back would do, the files that would move
/// out included, and changes nothing. The quick plan reads no file's
/// content, and says the same.
#[test]
fn a_dry_run_and_the_quick_plan_change_nothing_and_hash_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    let before = snapshot(dir.path());

    let full = restore(&resource, SET, Some("reapack"), true);
    assert_eq!(snapshot(dir.path()), before);
    // Not even an empty before-restore set, which would list as a backup.
    assert_eq!(sets_in(&resource), [SET]);
    assert_eq!(full.status, RestoreStatus::WouldRestore);
    let swap = full.swap.as_ref().unwrap();
    assert_eq!(swap.moved_out.len(), 1);
    assert_eq!(swap.moved_out[0].action, MoveOutAction::WouldMoveOut);
    assert!(swap.moved_out[0].backup_path.is_some());

    let item = |package_id: Option<&str>| RestoreItem::new(SET, package_id);
    let hashed = crate::hash::hashes_on_this_thread();
    let quick = restore_backups_with(
        &resource,
        &[item(Some("reapack")), item(None)],
        &RestoreRunOptions {
            dry_run: true,
            quick: true,
            allow_reaper_running: true,
        },
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(crate::hash::hashes_on_this_thread(), hashed);
    assert_eq!(snapshot(dir.path()), before);
    assert_eq!(sets_in(&resource), [SET]);
    assert_eq!(quick.saved_report, None);
    let one = &quick.backups[0];
    assert_eq!(one.status, RestoreStatus::WouldRestore);
    // Each before-restore set is named by the second it is planned in, so
    // only the place inside it is compared.
    let inside = |path: &Option<PathBuf>| -> Option<PathBuf> {
        let path = path.as_ref()?;
        let set = path.ancestors().find(|folder| {
            folder
                .parent()
                .is_some_and(|parent| parent.ends_with("backups"))
        })?;
        path.strip_prefix(set).ok().map(Path::to_path_buf)
    };
    let moved = |files: &[super::MovedOutFile]| -> Vec<(PathBuf, Option<PathBuf>, u64, String)> {
        files
            .iter()
            .map(|file| {
                (
                    file.path.clone(),
                    inside(&file.backup_path),
                    file.size,
                    file.sha256.clone(),
                )
            })
            .collect()
    };
    assert_eq!(
        moved(&one.swap.as_ref().unwrap().moved_out),
        moved(&swap.moved_out)
    );
    assert_eq!(
        inside(&swap.moved_out[0].backup_path),
        Some(PathBuf::from(NEW_ONLY))
    );
    let places = |actions: &[super::RestoreBackupAction]| -> Vec<(PathBuf, Option<PathBuf>)> {
        actions
            .iter()
            .map(|action| {
                (
                    action.target_path.clone(),
                    inside(&action.current_backup_path),
                )
            })
            .collect()
    };
    assert_eq!(places(&one.actions), places(&full.actions));
    // The whole set, one package of it.
    let whole = &quick.backups[1];
    assert_eq!(whole.status, RestoreStatus::WouldRestore);
    assert_eq!(whole.package_id, None);
    assert_eq!(whole.swaps.len(), 1);
    assert_eq!(whole.swaps[0].package_id.as_deref(), Some("reapack"));
}

/// A file the newer version has and the earlier one doesn't, that changed
/// since it was installed, isn't RABBIT's to move: it stays where it is,
/// and the report says so. A real run and a full dry run hash it, so a
/// change that keeps its size counts too; the quick plan compares sizes
/// only, and would move such a file. One that is gone is no matter.
#[test]
fn a_changed_file_of_the_newer_version_stays_and_is_reported() {
    for (changed, same_size) in [
        (b"changed by hand" as &[u8], false),
        (b"new in 1.2.6", true),
    ] {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        updated_reapack(&resource, Some(reapack_old()));
        write(&resource.join(NEW_ONLY), changed);

        let quick = quick_plan(&resource, Some("reapack"));
        let quick = quick.swap.unwrap();
        assert_eq!(quick.moved_out.len(), usize::from(same_size), "{same_size}");
        assert_eq!(quick.changed_left.is_empty(), same_size, "{same_size}");
        let full = restore(&resource, SET, Some("reapack"), true).swap.unwrap();
        assert!(full.moved_out.is_empty(), "{same_size}");
        assert_eq!(full.changed_left, [resource.join(NEW_ONLY)], "{same_size}");

        let back = restore(&resource, SET, Some("reapack"), false);

        assert_eq!(back.status, RestoreStatus::Restored);
        let swap = back.swap.unwrap();
        assert!(swap.moved_out.is_empty(), "{same_size}");
        assert_eq!(swap.changed_left, [resource.join(NEW_ONLY)], "{same_size}");
        assert_eq!(read(&resource, NEW_ONLY), changed);
        assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
        let state = load_install_state(&resource).unwrap().unwrap();
        assert_eq!(state.packages["reapack"].version, Some(version("1.2.4")));
    }
}

/// A symbolic link or junction where a file of the newer version should be
/// is no file of it: going back leaves it where it is, never follows it,
/// and reports it with the changed files. The overview offers the earlier
/// version as usual.
#[test]
fn a_link_at_a_file_of_the_newer_version_stays_and_is_never_followed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("kept.txt"), b"not RABBIT's");
    fs::remove_file(resource.join(NEW_ONLY)).unwrap();
    link_folder(&resource.join(NEW_ONLY), &elsewhere);
    assert_eq!(
        reapack_row(&resource).restore_scope,
        Some(RestoreScope::Package)
    );

    let back = restore(&resource, SET, Some("reapack"), false);

    assert_eq!(back.status, RestoreStatus::Restored);
    let swap = back.swap.unwrap();
    assert!(swap.moved_out.is_empty());
    assert_eq!(swap.changed_left, [resource.join(NEW_ONLY)]);
    assert!(
        fs::symlink_metadata(resource.join(NEW_ONLY))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read(elsewhere.join("kept.txt")).unwrap(),
        b"not RABBIT's"
    );
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
}

/// A file of the newer version at a place a restore refuses, in RABBIT's
/// own folder or outside every folder RABBIT installs into, couldn't come
/// back from the new backup, so going back is refused, in a dry run and
/// for real, and nothing changes. The overview gives the row the same
/// refusal and names the file, offers no command for it, and none for the
/// whole set. Below a junction that leads out of the resource folder, the
/// run refuses it as well; the overview judges paths by their text alone,
/// and can't see that one.
#[test]
fn a_newer_file_at_a_place_a_restore_refuses_refuses_going_back() {
    let in_rabbit = PathBuf::from("RABBIT/reapack_extra.dll");
    for case in ["rabbit-folder", "outside", "through-junction"] {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        updated_reapack(&resource, Some(reapack_old()));
        let path = match case {
            "rabbit-folder" => in_rabbit.clone(),
            "outside" => dir.path().join("Elsewhere/reapack_extra.dll"),
            _ => {
                let outside = dir.path().join("Outside");
                fs::create_dir_all(&outside).unwrap();
                link_folder(&resource.join("UserPlugins/linked"), &outside);
                PathBuf::from("UserPlugins/linked/reapack_extra.dll")
            }
        };
        add_newer_file(&resource, &path, b"extra in 1.2.5");

        let error = refused(
            dir.path(),
            &resource,
            SET,
            Some("reapack"),
            &BackupEnvironment::AS_USER,
        );

        let RestoreRefusal::NewerFileRefused {
            backup_id,
            package_id,
            path: refused_path,
            reason,
        } = refusal(&error)
        else {
            panic!("{case}: {error:?}");
        };
        assert_eq!(
            (backup_id.as_str(), package_id.as_str(), refused_path),
            (SET, "reapack", &path),
            "{case}"
        );
        let whole = refused(
            dir.path(),
            &resource,
            SET,
            None,
            &BackupEnvironment::AS_USER,
        );
        assert_eq!(refusal(&whole), refusal(&error), "{case}");
        let row = reapack_row(&resource);
        let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
        match case {
            "through-junction" => {
                assert!(
                    matches!(**reason, TargetRefusal::LeadsOutsideResourceFolder { .. }),
                    "{reason:?}"
                );
                assert_eq!(row.restore_scope, Some(RestoreScope::Package));
            }
            _ => {
                let expected = if case == "outside" {
                    TargetRefusal::OutsideAllowedFolders
                } else {
                    TargetRefusal::RabbitFolder
                };
                assert_eq!(**reason, expected, "{case}");
                assert_eq!(row.refusal.as_ref(), Some(refusal(&error)), "{case}");
                assert_eq!(row.refused_path.as_ref(), Some(&path), "{case}");
                assert_eq!(row.restore_scope, None, "{case}");
                assert!(!listed.sets[0].swaps_whole, "{case}");
            }
        }
    }
}

/// An earlier version a file of which is neither in the set nor on disk as
/// that version had it isn't complete: going back is refused, in a dry run
/// and for real, and nothing changes. The overview gives its backup the
/// same refusal.
#[test]
fn an_earlier_version_that_isnt_complete_is_refused() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    // The data file changed since, keeping its size, so 1.2.4's copy of it
    // is gone. A real run and a full dry run hash it and refuse; the quick
    // plan compares sizes only, as the overview does, and goes ahead.
    write(&resource.join(SHARED), b"sharef");
    let same_size = refused(
        dir.path(),
        &resource,
        SET,
        Some("reapack"),
        &BackupEnvironment::AS_USER,
    );
    assert!(
        matches!(refusal(&same_size), RestoreRefusal::NotComplete { path, .. }
            if path == Path::new(SHARED)),
        "{same_size:?}"
    );
    let quick = quick_plan(&resource, Some("reapack"));
    assert_eq!(quick.status, RestoreStatus::WouldRestore, "{quick:?}");
    assert_eq!(quick.refusal, None);
    // Now it has another size too.
    write(&resource.join(SHARED), b"shared, changed");

    let error = refused(
        dir.path(),
        &resource,
        SET,
        Some("reapack"),
        &BackupEnvironment::AS_USER,
    );

    let expected = RestoreRefusal::NotComplete {
        backup_id: SET.to_string(),
        package_id: "reapack".to_string(),
        path: PathBuf::from(SHARED),
    };
    assert_eq!(refusal(&error), &expected);
    let whole = refused(
        dir.path(),
        &resource,
        SET,
        None,
        &BackupEnvironment::AS_USER,
    );
    assert_eq!(refusal(&whole), &expected);
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let row = &listed.entries[0];
    assert_eq!(row.status, BackupStatus::NotComplete);
    assert_eq!(row.refusal.as_ref(), Some(&expected));
    assert_eq!(row.restore_scope, None);
    let quick = quick_plan(&resource, Some("reapack"));
    assert_eq!(quick.refusal.as_ref(), Some(&expected));
}

/// An earlier entry RABBIT would never write doesn't go into its records.
/// One whose id names another package is no entry of ReaPack, so its
/// version is unknown. One that records a file at a place RABBIT never
/// installs to, outside every folder it installs into, with `..` in it or
/// in RABBIT's own folder, is refused even for a file with no hash, which
/// counts as the user's and is never checked for being complete. Each in
/// a dry run and for real, alone and in the whole set, and the row says the
/// same; nothing changes. A file at `.`, the resource folder itself, and a
/// folder inside it, as a receipt records them with no hash, are places
/// RABBIT installs to, and the version comes back with them.
#[test]
fn an_earlier_entry_rabbit_would_never_write_is_refused() {
    let edit = |resource: &Path, change: &dyn Fn(&mut PackageReceipt)| {
        let set = resource.join("RABBIT/backups").join(SET);
        let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
        change(
            manifest
                .replaced
                .get_mut("reapack")
                .unwrap()
                .receipt
                .as_mut()
                .unwrap(),
        );
        save_backup_manifest(&set, &manifest).unwrap();
    };
    let users = |path: &Path| InstalledFileReceipt {
        path: path.to_path_buf(),
        sha256: None,
        size: None,
    };

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    edit(&resource, &|entry| entry.id = "sws".to_string());
    let expected = RestoreRefusal::VersionUnknown {
        backup_id: SET.to_string(),
        package_id: "reapack".to_string(),
    };
    for package_id in [Some("reapack"), None] {
        let error = refused(
            dir.path(),
            &resource,
            SET,
            package_id,
            &BackupEnvironment::AS_USER,
        );
        assert_eq!(refusal(&error), &expected, "{package_id:?}");
    }
    let row = reapack_row(&resource);
    assert_eq!(row.status, BackupStatus::VersionUnknown);
    assert_eq!(row.refusal, Some(expected));

    for case in ["outside", "parent", "rabbit-folder"] {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        updated_reapack(&resource, Some(reapack_old()));
        let (path, reason) = match case {
            "outside" => {
                let documents = dir.path().join("Documents").join("thesis.docx");
                write(&documents, b"thesis");
                (documents, TargetRefusal::OutsideAllowedFolders)
            }
            "parent" => (
                PathBuf::from("..").join("Desktop").join("notes.txt"),
                TargetRefusal::NotPlainPath,
            ),
            _ => (
                PathBuf::from("RABBIT").join("install-state.json"),
                TargetRefusal::RabbitFolder,
            ),
        };
        edit(&resource, &|entry| entry.installed_files.push(users(&path)));
        let expected = RestoreRefusal::EarlierFileRefused {
            backup_id: SET.to_string(),
            package_id: "reapack".to_string(),
            path: path.clone(),
            reason: Box::new(reason),
        };

        for package_id in [Some("reapack"), None] {
            let error = refused(
                dir.path(),
                &resource,
                SET,
                package_id,
                &BackupEnvironment::AS_USER,
            );
            assert_eq!(refusal(&error), &expected, "{case} {package_id:?}");
        }
        let row = reapack_row(&resource);
        assert_eq!(row.refusal.as_ref(), Some(&expected), "{case}");
        assert_eq!(row.refused_path.as_ref(), Some(&path), "{case}");
        assert_eq!(row.restore_scope, None, "{case}");
        let quick = quick_plan(&resource, Some("reapack"));
        assert_eq!(quick.refusal.as_ref(), Some(&expected), "{case}");
    }

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    let kept = [PathBuf::from("."), PathBuf::from("Data").join("Grooves")];
    edit(&resource, &|entry| {
        entry
            .installed_files
            .extend(kept.iter().map(|path| users(path)));
    });
    assert_eq!(reapack_row(&resource).refusal, None);

    let report = restore(&resource, SET, Some("reapack"), false);

    assert_eq!(report.status, RestoreStatus::Restored, "{report:?}");
    let records = load_install_state(&resource).unwrap().unwrap();
    let paths: Vec<&Path> = records.packages["reapack"]
        .installed_files
        .iter()
        .map(|file| file.path.as_path())
        .collect();
    assert!(
        paths.ends_with(&[kept[0].as_path(), kept[1].as_path()]),
        "{paths:?}"
    );
}

/// A copy the set holds at the place of a file of the earlier version
/// counts only when the restore puts it back: one of another package, or
/// the key map's, doesn't make the version complete.
#[test]
fn only_copies_the_restore_puts_back_make_an_earlier_version_complete() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    // 1.2.4's data file is gone from disk, and the set holds a copy of it
    // only as SWS's.
    write(&resource.join(SHARED), b"shared, changed");
    let set = resource.join("RABBIT/backups").join(SET);
    let copy = set.join(SHARED);
    write(&copy, b"shared");
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    manifest.files.push(super::BackupManifestFile {
        package_id: Some("sws".to_string()),
        original_path: resource.join(SHARED),
        backup_path: copy,
        size: 6,
        sha256: sha256_bytes(b"shared"),
        state: super::BackupEntryState::Done,
    });
    save_backup_manifest(&set, &manifest).unwrap();

    let error = refused(
        dir.path(),
        &resource,
        SET,
        Some("reapack"),
        &BackupEnvironment::AS_USER,
    );

    let expected = RestoreRefusal::NotComplete {
        backup_id: SET.to_string(),
        package_id: "reapack".to_string(),
        path: PathBuf::from(SHARED),
    };
    assert_eq!(refusal(&error), &expected);
    assert_eq!(reapack_row(&resource).refusal, Some(expected));
}

/// An update over a copy RABBIT didn't install recorded no entry: going
/// back puts that copy back and takes ReaPack's entry out of RABBIT's
/// records, and the report says so. RABBIT can't tell which of the newer
/// version's other files that copy had too, here the data file it had and
/// the file it didn't, so none of them moves: each stays, and a copy of it
/// goes into the new backup, with the plug-in the set's copy replaces.
/// Restoring that backup goes forward again: ReaPack's entry is 1.2.5's
/// once more, on its own channel, and every file is as before. The quick
/// plan says the same and hashes nothing, and a copy that fails leaves
/// everything as it was.
#[test]
fn going_back_to_a_copy_rabbit_didnt_install_removes_the_entry() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, None);
    let before = snapshot(dir.path());

    let hashed = crate::hash::hashes_on_this_thread();
    let quick = quick_plan(&resource, Some("reapack"));
    assert_eq!(crate::hash::hashes_on_this_thread(), hashed);
    assert_eq!(
        quick.swap.unwrap().left_in_place,
        [resource.join(SHARED), resource.join(NEW_ONLY)]
    );
    {
        let failing = resource.join(NEW_ONLY);
        let _hook = restore_seam::install(move |step, path| {
            if step == Step::CopyingIn && path == failing {
                return Err(std::io::Error::other("the disk is full"));
            }
            Ok(())
        });
        let failed = restore(&resource, SET, Some("reapack"), false);
        assert_eq!(failed.status, RestoreStatus::NotRestored);
        assert_eq!(failed.before_restore_set_id, None);
    }
    let after_failure: Vec<_> = snapshot_but_lock_folder(dir.path(), &resource)
        .into_iter()
        .filter(|(path, _)| !path.starts_with(resource.join("RABBIT/logs")))
        .collect();
    assert_eq!(after_failure, before);

    let back = restore(&resource, SET, Some("reapack"), false);

    assert_eq!(back.status, RestoreStatus::Restored);
    let swap = back.swap.unwrap();
    assert_eq!(swap.entry, SwappedEntry::Removed);
    assert!(swap.undeclined);
    // As shipped, ReaPack's uninstall needs no entry.
    assert!(!swap.uninstall_needs_reinstall);
    assert_eq!(swap.version, None);
    assert!(swap.moved_out.is_empty());
    assert_eq!(
        swap.left_in_place,
        [resource.join(SHARED), resource.join(NEW_ONLY)]
    );
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
    assert_eq!(read(&resource, SHARED), b"shared");
    assert_eq!(read(&resource, NEW_ONLY), b"new in 1.2.5");
    let state = load_install_state(&resource).unwrap().unwrap();
    let mut expected = records_with(None);
    expected.declined_packages.remove("reapack");
    assert_eq!(state, expected);
    let newer = back.before_restore_set_id.unwrap();
    let newer_set = resource.join("RABBIT/backups").join(&newer);
    for (file, contents) in [
        (PLUGIN, b"reapack 1.2.5" as &[u8]),
        (SHARED, b"shared"),
        (NEW_ONLY, b"new in 1.2.5"),
    ] {
        assert_eq!(fs::read(newer_set.join(file)).unwrap(), contents, "{file}");
    }

    let forward = restore(&resource, &newer, Some("reapack"), false);

    assert_eq!(forward.status, RestoreStatus::Restored);
    let swap = forward.swap.unwrap();
    assert_eq!(swap.entry, SwappedEntry::Added);
    assert!(swap.moved_out.is_empty() && swap.left_in_place.is_empty());
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.5");
    assert_eq!(read(&resource, SHARED), b"shared");
    assert_eq!(read(&resource, NEW_ONLY), b"new in 1.2.5");
    let state = load_install_state(&resource).unwrap().unwrap();
    let mut expected = records_with(Some(reapack_new()));
    expected.declined_packages.remove("reapack");
    assert_eq!(state, expected);
}

/// Going back to a copy RABBIT didn't install, of a package whose uninstall
/// needs its entry in RABBIT's records, as FFmpeg's does, says that RABBIT
/// can't uninstall it until it is installed again.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn going_back_to_a_copy_rabbit_didnt_install_says_when_its_uninstall_needs_the_entry() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, None);
    let platform = crate::model::Platform::current().expect("tests run on Windows or macOS");
    let mut spec = crate::package::builtin_package_specs(platform)
        .into_iter()
        .find(|spec| spec.id == "reapack")
        .unwrap();
    spec.uninstall.requires_receipt = true;
    let specs = move |_: crate::model::Platform| vec![spec.clone()];
    let environment = BackupEnvironment {
        package_specs: &specs,
        ..BackupEnvironment::AS_USER
    };

    let preview = restore_backup_set_with(
        &resource,
        SET,
        &options(true, Some("reapack")),
        &environment,
        OutsideRestoreRules::default,
    )
    .unwrap();

    let swap = preview.swap.unwrap();
    assert_eq!(swap.entry, SwappedEntry::Removed);
    assert!(swap.uninstall_needs_reinstall);
}

/// An older update's set whose copy of RABBIT's records doesn't list a
/// package says RABBIT hadn't installed the copy it replaced: going back
/// takes that package's entry out, and nothing else changes, the copy
/// included.
#[test]
fn an_older_update_set_whose_copy_lacks_a_package_removes_its_entry() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_both(&resource, false);
    let set = resource.join("RABBIT/backups").join(SET);
    let copy = set.join("RABBIT/install-state.json");
    let mut before: InstallState = serde_json::from_slice(&fs::read(&copy).unwrap()).unwrap();
    before.packages.remove("sws");
    let copied = serde_json::to_vec(&before).unwrap();
    fs::write(&copy, &copied).unwrap();
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    let entry = manifest
        .files
        .iter_mut()
        .find(|entry| entry.package_id.is_none())
        .unwrap();
    entry.size = copied.len() as u64;
    entry.sha256 = sha256_bytes(&copied);
    save_backup_manifest(&set, &manifest).unwrap();

    let back = restore(&resource, SET, Some("sws"), false);

    assert_eq!(back.status, RestoreStatus::Restored);
    let swap = back.swap.unwrap();
    assert_eq!(swap.entry, SwappedEntry::Removed);
    assert_eq!(swap.replaced_version, Some(version("2.15")));
    assert!(!swap.undeclined);
    assert_eq!(
        read(&resource, "UserPlugins/reaper_sws-x64.dll"),
        b"sws 2.14"
    );
    let state = load_install_state(&resource).unwrap().unwrap();
    let mut expected = records_with(Some(reapack_new()));
    expected.packages.remove("sws");
    assert_eq!(state, expected);
    assert_eq!(fs::read(&copy).unwrap(), copied);
}

/// Going back where the newer version's files are gone already writes no
/// file over another and moves none out, so the new backup would hold only
/// the record of the newer version: it goes again, and none is named.
#[test]
fn going_back_over_nothing_leaves_no_new_backup() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    fs::remove_file(resource.join(PLUGIN)).unwrap();
    fs::remove_file(resource.join(NEW_ONLY)).unwrap();

    let back = restore(&resource, SET, Some("reapack"), false);

    assert_eq!(back.status, RestoreStatus::Restored);
    assert_eq!(back.before_restore_set_id, None);
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
    assert_eq!(sets_in(&resource), [SET]);
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages["reapack"].version, Some(version("1.2.4")));
}

/// While another package of its exclusive group is installed, going back
/// to an earlier version is refused, in a dry run and for real, and the
/// overview says the same. Without such a group, nothing stops it.
#[test]
fn another_installed_package_of_its_group_refuses_the_earlier_version() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    let groups = || {
        BTreeMap::from([
            ("reapack".to_string(), "managers".to_string()),
            ("sws".to_string(), "managers".to_string()),
        ])
    };
    let grouped = BackupEnvironment {
        exclusive_groups: &groups,
        ..BackupEnvironment::AS_USER
    };

    let error = refused(dir.path(), &resource, SET, Some("reapack"), &grouped);

    let expected = RestoreRefusal::OtherLanguagePackInstalled {
        backup_id: SET.to_string(),
        package_id: "reapack".to_string(),
        installed: vec!["sws".to_string()],
    };
    assert_eq!(refusal(&error), &expected);
    let listed = overview_with(&resource, &grouped).unwrap();
    assert_eq!(listed.entries[0].refusal.as_ref(), Some(&expected));
    assert!(!listed.sets[0].swaps_whole);
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    assert!(listed.sets[0].swaps_whole);
}

/// SWS 2.14 as RABBIT installed it, for the whole-set tests, which update
/// it to 2.15 along with ReaPack.
fn sws_old() -> PackageReceipt {
    entry(
        "sws",
        "2.14",
        &[("UserPlugins/reaper_sws-x64.dll", b"sws 2.14")],
    )
}

/// SWS 2.15, which replaced its plug-in.
fn sws_new() -> PackageReceipt {
    entry(
        "sws",
        "2.15",
        &[("UserPlugins/reaper_sws-x64.dll", b"sws 2.15")],
    )
}

/// The update of ReaPack and SWS in one run, into the set `SET`: with
/// `records`, as RABBIT writes it now, or else as earlier versions did,
/// with a copy of RABBIT's records from before the update instead.
fn updated_both(resource: &Path, records: bool) {
    let sws = "UserPlugins/reaper_sws-x64.dll";
    write(&resource.join(PLUGIN), b"reapack 1.2.4");
    write(&resource.join(SHARED), b"shared");
    write(&resource.join(sws), b"sws 2.14");
    let mut before = InstallState::default();
    before.packages.insert("reapack".to_string(), reapack_old());
    before.packages.insert("sws".to_string(), sws_old());
    let mut writer = BackupSetWriter::new(resource, SET, UPDATE_BACKUP_REASON);
    for (package_id, earlier, newer) in [
        ("reapack", reapack_old(), "1.2.5"),
        ("sws", sws_old(), "2.15"),
    ] {
        if records {
            writer
                .record_replaced(
                    package_id,
                    ReplacedPackageRecord {
                        receipt: Some(earlier),
                        replaced_by: Some(version(newer)),
                    },
                )
                .unwrap();
        }
        let file = if package_id == "sws" { sws } else { PLUGIN };
        writer
            .copy_file(Some(package_id), &resource.join(file))
            .unwrap();
    }
    if !records {
        let set = resource.join("RABBIT/backups").join(SET);
        let copy = set.join("RABBIT/install-state.json");
        let copied = serde_json::to_vec(&before).unwrap();
        write(&copy, &copied);
        let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
        manifest.files.push(super::BackupManifestFile {
            package_id: None,
            original_path: PathBuf::from("RABBIT/install-state.json"),
            backup_path: copy.clone(),
            size: copied.len() as u64,
            sha256: sha256_bytes(&copied),
            state: super::BackupEntryState::Done,
        });
        manifest.receipt_backup_path = Some(copy);
        save_backup_manifest(&set, &manifest).unwrap();
    }
    write(&resource.join(PLUGIN), b"reapack 1.2.5");
    write(&resource.join(NEW_ONLY), b"new in 1.2.5");
    write(&resource.join(sws), b"sws 2.15");
    let mut after = records_with(Some(reapack_new()));
    after.packages.insert("sws".to_string(), sws_new());
    save_install_state(resource, &after).unwrap();
}

/// A whole update's set goes back package by package, each with a
/// before-restore set of its own, whether it records each package's entry
/// or holds a copy of RABBIT's records from before, as earlier versions
/// wrote it. The copy is never touched, and each package's entry becomes
/// the one from before the update, and nothing else in the records
/// changes. The overview offers the whole set.
#[test]
fn a_whole_update_set_goes_back_package_by_package() {
    for records in [true, false] {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        updated_both(&resource, records);
        let set = resource.join("RABBIT/backups").join(SET);
        let copy = set.join("RABBIT/install-state.json");
        let copy_before = fs::read(&copy).ok();
        let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
        assert!(listed.sets[0].swaps_whole, "{records}");

        let back = restore(&resource, SET, None, false);

        assert_eq!(back.status, RestoreStatus::Restored, "{records}");
        assert!(back.actions.is_empty() && back.swap.is_none());
        assert_eq!(back.before_restore_set_id, None);
        let packages: Vec<Option<&str>> = back
            .swaps
            .iter()
            .map(|swap| swap.package_id.as_deref())
            .collect();
        assert_eq!(packages, [Some("reapack"), Some("sws")]);
        let sets: Vec<&String> = back
            .swaps
            .iter()
            .map(|swap| swap.before_restore_set_id.as_ref().unwrap())
            .collect();
        assert_ne!(sets[0], sets[1]);
        assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
        assert_eq!(
            read(&resource, "UserPlugins/reaper_sws-x64.dll"),
            b"sws 2.14"
        );
        assert!(!resource.join(NEW_ONLY).exists());
        let state = load_install_state(&resource).unwrap().unwrap();
        let mut expected = records_with(Some(reapack_old()));
        expected.packages.insert("sws".to_string(), sws_old());
        // Each keeps the channel it is on.
        expected.packages.get_mut("reapack").unwrap().channel = Some("dev".to_string());
        expected.declined_packages.remove("reapack");
        assert_eq!(state, expected, "{records}");
        assert_eq!(fs::read(&copy).ok(), copy_before, "{records}");
    }
}

/// Two earlier versions of one update's set in one run, as two ticked
/// rows: each package goes back on its own, with the fingerprint listed
/// and a before-restore set of its own, and the set doesn't change.
#[test]
fn two_earlier_versions_of_one_set_come_back_in_one_run() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_both(&resource, true);
    let set = resource.join("RABBIT/backups").join(SET);
    let set_before = snapshot(&set);
    let listed = overview_with(&resource, &BackupEnvironment::AS_USER).unwrap();
    let fingerprint = listed.sets[0].fingerprint.clone();
    assert!(fingerprint.is_some());
    let item = |package_id: &str| RestoreItem {
        fingerprint: fingerprint.clone(),
        ..RestoreItem::package(SET, package_id)
    };

    let report = restore_backups_with(
        &resource,
        &[item("reapack"), item("sws")],
        &RestoreRunOptions {
            dry_run: false,
            quick: false,
            allow_reaper_running: true,
        },
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap();

    let statuses: Vec<RestoreStatus> = report.backups.iter().map(|item| item.status).collect();
    assert_eq!(
        statuses,
        [RestoreStatus::Restored, RestoreStatus::Restored],
        "{report:#?}"
    );
    let before_restore: Vec<&str> = report
        .backups
        .iter()
        .map(|item| item.before_restore_set_id.as_deref().unwrap())
        .collect();
    assert_ne!(before_restore[0], before_restore[1]);
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
    assert_eq!(
        read(&resource, "UserPlugins/reaper_sws-x64.dll"),
        b"sws 2.14"
    );
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages["reapack"].version, Some(version("1.2.4")));
    assert_eq!(state.packages["sws"], sws_old());
    assert_eq!(snapshot(&set), set_before);
}

/// A file that fails partway through going back, after the newer
/// version's own file moved out and the plug-in went back, leaves the
/// package as it was: the plug-in is put back, the moved file comes back
/// to its place, RABBIT's records don't change, and no before-restore set
/// is left.
#[test]
fn a_failure_partway_puts_the_newer_version_back_whole() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let second = "UserPlugins/reapack_second.dll";
    write(&resource.join(PLUGIN), b"reapack 1.2.4");
    write(&resource.join(SHARED), b"shared");
    write(&resource.join(second), b"second 1.2.4");
    let mut writer = BackupSetWriter::new(&resource, SET, UPDATE_BACKUP_REASON);
    writer
        .record_replaced(
            "reapack",
            ReplacedPackageRecord {
                receipt: Some(entry(
                    "reapack",
                    "1.2.4",
                    &[
                        (PLUGIN, b"reapack 1.2.4"),
                        (SHARED, b"shared"),
                        (second, b"second 1.2.4"),
                    ],
                )),
                replaced_by: Some(version("1.2.5")),
            },
        )
        .unwrap();
    for file in [PLUGIN, second] {
        writer
            .copy_file(Some("reapack"), &resource.join(file))
            .unwrap();
    }
    write(&resource.join(PLUGIN), b"reapack 1.2.5");
    write(&resource.join(second), b"second 1.2.5");
    write(&resource.join(NEW_ONLY), b"new in 1.2.5");
    let newer = entry(
        "reapack",
        "1.2.5",
        &[
            (PLUGIN, b"reapack 1.2.5"),
            (SHARED, b"shared"),
            (second, b"second 1.2.5"),
            (NEW_ONLY, b"new in 1.2.5"),
        ],
    );
    save_install_state(&resource, &records_with(Some(newer))).unwrap();
    let receipt = fs::read(resource.join("RABBIT/install-state.json")).unwrap();
    let failing = resource.join(second);
    let _hook = restore_seam::install(move |step, path| {
        if step == Step::Restoring && path == failing {
            return Err(std::io::Error::other("the disk is full"));
        }
        Ok(())
    });

    let report = restore(&resource, SET, Some("reapack"), false);

    assert_eq!(report.status, RestoreStatus::NotRestored);
    assert!(
        report
            .error
            .as_deref()
            .is_some_and(|error| error.contains("the disk is full")),
        "{report:?}"
    );
    let kinds: Vec<RestoreBackupActionKind> =
        report.actions.iter().map(|action| action.action).collect();
    assert_eq!(
        kinds,
        [
            RestoreBackupActionKind::PutBack,
            RestoreBackupActionKind::NotRestored
        ]
    );
    let swap = report.swap.as_ref().unwrap();
    assert_eq!(swap.moved_out.len(), 1);
    assert_eq!(swap.moved_out[0].action, MoveOutAction::MovedBack);
    assert_eq!(swap.moved_out[0].backup_path, None);
    assert_eq!(read(&resource, NEW_ONLY), b"new in 1.2.5");
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.5");
    assert_eq!(read(&resource, second), b"second 1.2.5");
    assert_eq!(
        fs::read(resource.join("RABBIT/install-state.json")).unwrap(),
        receipt
    );
    assert_eq!(report.before_restore_set_id, None);
    assert_eq!(sets_in(&resource), [SET]);
}

/// A file of the newer version that can't move out, after another one did,
/// leaves the package as it was: the one that moved goes back to its
/// place, no file of the set is written, RABBIT's records don't change, and
/// no before-restore set is left.
#[test]
fn a_file_that_cant_move_out_puts_the_moved_ones_back() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    let second = "UserPlugins/reapack_second_new.dll";
    add_newer_file(&resource, Path::new(second), b"second new in 1.2.5");
    let receipt = fs::read(resource.join("RABBIT/install-state.json")).unwrap();
    let failing = resource.join(second);
    let _hook = restore_seam::install(move |step, path| {
        if step == Step::MovingOut && path == failing {
            return Err(std::io::Error::other("the file is in use"));
        }
        Ok(())
    });

    let report = restore(&resource, SET, Some("reapack"), false);

    assert_eq!(report.status, RestoreStatus::NotRestored);
    let moved = &report.swap.as_ref().unwrap().moved_out;
    assert_eq!(moved.len(), 2);
    assert_eq!(moved[0].path, resource.join(NEW_ONLY));
    assert_eq!(moved[0].action, MoveOutAction::MovedBack);
    assert_eq!(moved[0].backup_path, None);
    assert_eq!(moved[1].path, resource.join(second));
    assert_eq!(moved[1].action, MoveOutAction::NotMoved);
    assert_eq!(moved[1].backup_path, None);
    assert!(
        moved[1]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("the file is in use")),
        "{moved:?}"
    );
    let kinds: Vec<RestoreBackupActionKind> =
        report.actions.iter().map(|action| action.action).collect();
    assert_eq!(kinds, [RestoreBackupActionKind::NotRestored]);
    assert_eq!(read(&resource, NEW_ONLY), b"new in 1.2.5");
    assert_eq!(read(&resource, second), b"second new in 1.2.5");
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.5");
    assert_eq!(
        fs::read(resource.join("RABBIT/install-state.json")).unwrap(),
        receipt
    );
    assert_eq!(report.before_restore_set_id, None);
    assert_eq!(sets_in(&resource), [SET]);
}

/// A whole set whose second package fails partway keeps the first one
/// restored: each package is all or nothing on its own. The set as a whole
/// is partly restored, and each package says how it ended.
#[test]
fn a_whole_set_whose_second_package_fails_is_partly_restored() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_both(&resource, true);
    let sws = resource.join("UserPlugins/reaper_sws-x64.dll");
    let watched = sws.clone();
    let _hook = restore_seam::install(move |step, path| {
        if step == Step::Replacing && path == watched {
            return Err(std::io::Error::other("the file is in use"));
        }
        Ok(())
    });

    let report = restore(&resource, SET, None, false);

    assert_eq!(report.status, RestoreStatus::PartlyRestored);
    let statuses: Vec<RestoreStatus> = report.swaps.iter().map(|swap| swap.status).collect();
    assert_eq!(
        statuses,
        [RestoreStatus::Restored, RestoreStatus::NotRestored]
    );
    assert!(report.swaps[1].error.is_some());
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
    assert_eq!(fs::read(&sws).unwrap(), b"sws 2.15");
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages["reapack"].version, Some(version("1.2.4")));
    assert_eq!(state.packages["sws"], sws_new());
    let saved = report.saved_report.expect("a saved report");
    let envelope: crate::report::SavedJsonReport =
        serde_json::from_str(&fs::read_to_string(saved.json_path).unwrap()).unwrap();
    assert_eq!(envelope.report["status"], "partly-restored");
    assert_eq!(envelope.report["swaps"][1]["status"], "not-restored");
}

/// A file that moved out and can't go back after a failure stays in the
/// before-restore set, which stays and is named: the package's files are
/// left, and RABBIT's records don't change.
#[test]
fn a_moved_file_that_cant_go_back_stays_in_the_backup() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    let receipt = fs::read(resource.join("RABBIT/install-state.json")).unwrap();
    let _hook = restore_seam::install(|step, _| match step {
        Step::SavingRecords => Err(std::io::Error::other("the records can't be written")),
        Step::MovingBack => Err(std::io::Error::other("access denied")),
        _ => Ok(()),
    });

    let report = restore(&resource, SET, Some("reapack"), false);

    assert_eq!(report.status, RestoreStatus::FilesLeft);
    let moved = &report.swap.as_ref().unwrap().moved_out[0];
    assert_eq!(moved.action, MoveOutAction::StillInBackup);
    let copy = moved.backup_path.clone().unwrap();
    assert_eq!(fs::read(&copy).unwrap(), b"new in 1.2.5");
    assert!(!resource.join(NEW_ONLY).exists());
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.5");
    assert_eq!(
        fs::read(resource.join("RABBIT/install-state.json")).unwrap(),
        receipt
    );
    let kept = report.before_restore_set_id.unwrap();
    assert!(copy.starts_with(resource.join("RABBIT/backups").join(kept)));
}

/// A package the set doesn't hold, or holds nothing of, is refused on its
/// own; the whole set leaves out a package with nothing of it and names
/// it. A finished copy that is gone refuses the package and the whole set.
#[test]
fn packages_a_set_of_earlier_versions_cant_put_back_are_refused() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_reapack(&resource, Some(reapack_old()));
    let set = resource.join("RABBIT/backups").join(SET);
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    manifest.replaced.insert(
        "jaws-scripts".to_string(),
        ReplacedPackageRecord {
            receipt: None,
            replaced_by: Some(version("2026.1")),
        },
    );
    save_backup_manifest(&set, &manifest).unwrap();
    let as_user = BackupEnvironment::AS_USER;

    let error = refused(dir.path(), &resource, SET, Some("osara"), &as_user);
    assert!(
        matches!(refusal(&error), RestoreRefusal::PackageNotInSet { packages, .. }
            if *packages == ["jaws-scripts", "reapack"]),
        "{error:?}"
    );
    let error = refused(dir.path(), &resource, SET, Some("jaws-scripts"), &as_user);
    assert_eq!(
        refusal(&error),
        &RestoreRefusal::NothingOfItInSet {
            backup_id: SET.to_string(),
            package_ids: vec!["jaws-scripts".to_string()],
        }
    );
    let preview = restore(&resource, SET, None, true);
    assert_eq!(preview.status, RestoreStatus::WouldRestore);
    assert_eq!(preview.skipped_packages.len(), 1);
    assert_eq!(preview.skipped_packages[0].package_id, "jaws-scripts");
    assert_eq!(preview.swaps.len(), 1);

    fs::remove_file(set.join(PLUGIN)).unwrap();
    let missing = RestoreRefusal::PartMissing {
        backup_id: SET.to_string(),
        path: PathBuf::from(PLUGIN),
    };
    for package_id in [Some("reapack"), None] {
        let error = refused(dir.path(), &resource, SET, package_id, &as_user);
        assert_eq!(refusal(&error), &missing, "{package_id:?}");
    }
    let listed = overview_with(&resource, &as_user).unwrap();
    let row = listed
        .entries
        .iter()
        .find(|entry| entry.item.package_id() == Some("reapack"))
        .unwrap();
    assert_eq!(row.refusal.as_ref(), Some(&missing));
}

/// The user's stop between two packages of a whole set: the first stays
/// back, the second isn't reached and is as it was, and the set as a whole
/// is partly restored.
#[test]
fn a_stop_between_packages_of_a_whole_set_leaves_the_rest_as_they_were() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_both(&resource, true);
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let progress = ProgressReporter::new(move |event| {
        // Pressed while the first package goes back.
        if matches!(event, ProgressEvent::RestoreStarted { .. }) {
            stop.cancel();
        }
    });

    let report = restore_backups_with(
        &resource,
        &[RestoreItem::whole_set(SET)],
        &RestoreRunOptions {
            dry_run: false,
            quick: false,
            allow_reaper_running: true,
        },
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
        &progress,
        &cancel,
    )
    .unwrap();

    assert!(report.stopped);
    let whole = &report.backups[0];
    assert_eq!(whole.status, RestoreStatus::PartlyRestored);
    let statuses: Vec<RestoreStatus> = whole.swaps.iter().map(|swap| swap.status).collect();
    assert_eq!(
        statuses,
        [RestoreStatus::Restored, RestoreStatus::NotReached]
    );
    assert_eq!(read(&resource, PLUGIN), b"reapack 1.2.4");
    assert_eq!(
        read(&resource, "UserPlugins/reaper_sws-x64.dll"),
        b"sws 2.15"
    );
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages["sws"], sws_new());
}

/// What runs is scanned again before each package of a whole set after the
/// first: a program SWS's uninstall needs closed, started once ReaPack went
/// back, refuses SWS alone, which stays as it was.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn a_program_started_during_a_whole_set_refuses_only_the_next_package() {
    use super::restore_run_tests::{demo_tool, sws_needing_closed};

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    updated_both(&resource, true);
    let known = sws_needing_closed("Demo Tool.exe");
    let specs = move |_: crate::model::Platform| known.clone();
    let plugin = resource.join(PLUGIN);
    let running = move |_: &[String]| {
        if fs::read(&plugin).is_ok_and(|contents| contents == b"reapack 1.2.4") {
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

    let report = restore_backup_set_with(
        &resource,
        SET,
        &options(false, None),
        &environment,
        OutsideRestoreRules::default,
    )
    .unwrap();

    assert_eq!(report.status, RestoreStatus::PartlyRestored);
    let statuses: Vec<RestoreStatus> = report.swaps.iter().map(|swap| swap.status).collect();
    assert_eq!(
        statuses,
        [RestoreStatus::Restored, RestoreStatus::NotRestored]
    );
    assert!(
        matches!(&report.swaps[1].refusal, Some(RestoreRefusal::ProgramRunning {
            package_ids,
            ..
        }) if package_ids == &["sws"]),
        "{report:?}"
    );
    assert_eq!(
        read(&resource, "UserPlugins/reaper_sws-x64.dll"),
        b"sws 2.15"
    );
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages["sws"], sws_new());
}
