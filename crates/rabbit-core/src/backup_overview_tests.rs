use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::{
    BackupEntry, BackupGroup, BackupItem, BackupOverview, BackupSetKind, BackupSize, BackupStatus,
    RestoreScope, SetFingerprint, backup_restore_targets, overview_of_with, overview_with,
    places_looked_at_on_this_thread, restore_targets_with, seconds_in_set_id,
};
use crate::error::RabbitError;
use crate::hash::{hashes_on_this_thread, sha256_bytes};
use crate::receipt::{InstallState, InstalledFileReceipt, PackageReceipt, save_install_state};
use crate::restore_refusal::{RestoreRefusal, TargetRefusal, refusal};
use crate::rollback::{
    BEFORE_RESTORE_REASON, BackupEntryState, BackupEnvironment, BackupManifest, BackupManifestFile,
    BackupPackageRecord, BackupSetWriter, DELETING_DIR, KEYMAP_BACKUP_REASON, KeymapOrigin,
    OutsideRestoreRules, RECORDS_BACKUP_REASON, ReplacedPackageRecord, RestoreBackupOptions,
    UPDATE_BACKUP_REASON, list_backup_sets, load_backup_manifest, restore_backup_set_with,
    restore_backup_set_within, save_backup_manifest,
};
use crate::uninstall::execute::UNINSTALL_BACKUP_REASON;
use crate::uninstall::paths::link_folder;
use crate::version::Version;

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

/// A receipt entry for `id` at `version`, listing `files` inside the
/// resource folder with what they hold, as an install records them.
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
        installed_at: Some("unix-1".to_string()),
        rabbit_version: Some("0.5.0".to_string()),
        architecture: None,
        vendor_added_files: Vec::new(),
    }
}

fn save_receipt(resource: &Path, packages: Vec<PackageReceipt>) {
    let mut state = InstallState::default();
    for package in packages {
        state.packages.insert(package.id.clone(), package);
    }
    save_install_state(resource, &state).unwrap();
}

/// The overview, with owners and groups read as on this computer.
fn overview(resource: &Path) -> BackupOverview {
    overview_with(resource, &BackupEnvironment::AS_USER).unwrap()
}

fn package(set_id: &str, package_id: &str) -> BackupItem {
    BackupItem::Package {
        set_id: set_id.to_string(),
        package_id: package_id.to_string(),
    }
}

fn whole(set_id: &str) -> BackupItem {
    BackupItem::Set {
        set_id: set_id.to_string(),
    }
}

fn keymap(set_id: &str) -> BackupItem {
    BackupItem::Keymap {
        set_id: set_id.to_string(),
    }
}

fn find<'a>(overview: &'a BackupOverview, item: &BackupItem) -> &'a BackupEntry {
    overview
        .entries
        .iter()
        .find(|entry| entry.item == *item)
        .unwrap_or_else(|| panic!("no {item:?} in {:#?}", overview.entries))
}

fn set_path(resource: &Path, id: &str) -> PathBuf {
    resource.join("RABBIT").join("backups").join(id)
}

/// A manifest with nothing in it yet, for a set written by hand.
fn manifest(created_at: &str, created_unix: Option<u64>, reason: &str) -> BackupManifest {
    BackupManifest {
        schema_version: 1,
        rabbit_version: "0.4.4".to_string(),
        created_at: created_at.to_string(),
        created_unix,
        reason: reason.to_string(),
        files: Vec::new(),
        receipt_backup_path: None,
        packages: BTreeMap::new(),
        replaced: BTreeMap::new(),
        keymap_origin: None,
    }
}

/// Change what the manifest of the set at `set` says.
fn edit_manifest(set: &Path, change: impl FnOnce(&mut BackupManifest)) {
    let mut manifest = load_backup_manifest(set).unwrap().unwrap();
    change(&mut manifest);
    save_backup_manifest(set, &manifest).unwrap();
}

/// An uninstall's set `id`: each package recorded with `receipt`, and the
/// files at `paths` inside the resource folder moved in. Returns its folder.
fn uninstall_set(
    resource: &Path,
    id: &str,
    packages: &[(&str, Option<PackageReceipt>, &[&str])],
) -> PathBuf {
    let mut writer = BackupSetWriter::new(resource, id, UNINSTALL_BACKUP_REASON);
    for (package_id, receipt, paths) in packages {
        writer
            .record_package(
                package_id,
                BackupPackageRecord {
                    receipt: receipt.clone(),
                    langpack_before: None,
                    with_settings: Some(false),
                },
            )
            .unwrap();
        for path in *paths {
            writer
                .move_file(Some(package_id), &resource.join(path), None, false)
                .unwrap();
        }
    }
    writer.path().to_path_buf()
}

/// An update's set `id` as RABBIT writes one now: for each package, its
/// earlier entry and the version that replaced it, then a copy of each
/// file at `paths`. Returns its folder.
fn update_set(
    resource: &Path,
    id: &str,
    packages: &[(&str, Option<PackageReceipt>, &str, &[&str])],
) -> PathBuf {
    let mut writer = BackupSetWriter::new(resource, id, UPDATE_BACKUP_REASON);
    for (package_id, earlier, newer, paths) in packages {
        writer
            .record_replaced(
                package_id,
                ReplacedPackageRecord {
                    receipt: earlier.clone(),
                    replaced_by: Some(version(newer)),
                },
            )
            .unwrap();
        for path in *paths {
            writer
                .copy_file(Some(package_id), &resource.join(path))
                .unwrap();
        }
    }
    writer.path().to_path_buf()
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

#[test]
fn dates_come_from_every_id_form() {
    let seconds = 1_782_841_261;
    for id in [
        "unix-1782841261",
        "unix-1782841261-2",
        "unattended-unix-1782841261-004215907",
        "unattended-unix-1782841261-004215907-2",
        "uninstall-unix-1782841261",
        "uninstall-unix-1782841261-3",
        "restore-unix-1782841261-before-restore",
        "restore-unix-1782841261-before-restore-12",
    ] {
        assert_eq!(seconds_in_set_id(id), Some(seconds), "{id}");
    }
    for id in [
        "",
        "unix-",
        "unix-x",
        "unix-12x",
        "unix-1-",
        "unix-1--2",
        "unix-1-02",
        "unix-1-0",
        "UNIX-1",
        "unattended-unix-1-2",
        "unattended-unix-1-1234567890",
        "unattended-unix-1",
        "uninstall-unix--1",
        "restore-unix-1",
        "restore-unix-1-before",
        "restore-unix-1-before-restore-",
        "my-backup",
        "unix-99999999999999999999999",
    ] {
        assert_eq!(seconds_in_set_id(id), None, "{id}");
    }
}

/// A set's date comes from its manifest's `created_unix`, then from its
/// manifest's `created_at`, which holds the id asked for and so no `-2`,
/// and only without a manifest from its folder's name. A name that doesn't
/// parse has no date, and sets without one come last.
#[test]
fn the_manifest_dates_a_set_before_its_name() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let by_hand = |id: &str, manifest: BackupManifest| {
        save_backup_manifest(&set_path(&resource, id), &manifest).unwrap();
    };
    by_hand("unix-100", manifest("unix-50", None, BEFORE_RESTORE_REASON));
    by_hand(
        "unix-200",
        manifest("unix-250", Some(300), BEFORE_RESTORE_REASON),
    );
    by_hand(
        "unix-400-2",
        manifest("unix-400", None, BEFORE_RESTORE_REASON),
    );
    by_hand(
        "unix-700",
        manifest("whatever", None, BEFORE_RESTORE_REASON),
    );
    layout_set(&resource, "uninstall-unix-500", &[("a.dll", b"a")]);
    layout_set(
        &resource,
        "unattended-unix-600-000000001-2",
        &[("reaper-kb.ini", b"keys")],
    );
    layout_set(&resource, "backup-by-hand", &[("b.dll", b"b")]);

    let overview = overview(&resource);

    let dates: Vec<(&str, Option<u64>)> = overview
        .sets
        .iter()
        .map(|set| (set.id.as_str(), set.created_unix))
        .collect();
    assert_eq!(
        dates,
        [
            ("unattended-unix-600-000000001-2", Some(600)),
            ("uninstall-unix-500", Some(500)),
            ("unix-400-2", Some(400)),
            ("unix-200", Some(300)),
            ("unix-100", Some(50)),
            ("backup-by-hand", None),
            ("unix-700", None),
        ]
    );
    // Each backup carries its set's date.
    assert_eq!(find(&overview, &whole("unix-100")).created_unix, Some(50));
}

/// Each kind of set gives the backups backup plan 3.1 says, in its group,
/// and the groups come in their order, the records last of all.
#[test]
fn each_kind_goes_to_its_group() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let sws = "UserPlugins/reaper_sws-x64.dll";
    let reapack = "UserPlugins/reaper_reapack-x64.dll";
    let kb = "reaper-kb.ini";
    write(&resource.join(sws), b"sws");
    write(&resource.join(reapack), b"reapack 1.2.4");
    write(&resource.join(kb), b"own keys");

    uninstall_set(
        &resource,
        "uninstall-unix-100",
        &[("sws", Some(entry("sws", "2.14", &[(sws, b"sws")])), &[sws])],
    );
    update_set(
        &resource,
        "unix-200",
        &[(
            "reapack",
            Some(entry("reapack", "1.2.4", &[(reapack, b"reapack 1.2.4")])),
            "1.2.5",
            &[reapack],
        )],
    );
    let mut writer = BackupSetWriter::new(
        &resource,
        "unattended-unix-300-000000000",
        KEYMAP_BACKUP_REASON,
    );
    writer.set_keymap_origin(KeymapOrigin::OwnKeymap).unwrap();
    writer.copy_file(Some("osara"), &resource.join(kb)).unwrap();
    // An older restore's backup that holds the key map it replaced.
    BackupSetWriter::new(
        &resource,
        "restore-unix-400-before-restore",
        BEFORE_RESTORE_REASON,
    )
    .copy_file(Some("osara"), &resource.join(kb))
    .unwrap();
    // A restore's backup of an earlier version, as the swap will write.
    let mut swap = BackupSetWriter::new(
        &resource,
        "restore-unix-500-before-restore",
        BEFORE_RESTORE_REASON,
    );
    swap.record_replaced(
        "reapack",
        ReplacedPackageRecord {
            receipt: Some(entry("reapack", "1.2.5", &[])),
            replaced_by: Some(version("1.2.4")),
        },
    )
    .unwrap();
    swap.copy_file(Some("reapack"), &resource.join(reapack))
        .unwrap();
    // Another restore's backup, of a plug-in.
    BackupSetWriter::new(
        &resource,
        "restore-unix-600-before-restore",
        BEFORE_RESTORE_REASON,
    )
    .copy_file(Some("sws"), &resource.join(reapack))
    .unwrap();
    layout_set(&resource, "unix-700", &[(reapack, b"old")]);
    let damaged = layout_set(&resource, "unix-800", &[(reapack, b"old")]);
    write(&damaged.join("backup-manifest.json"), b"{ not json");
    let mut records = manifest("unattended-unix-900-000000000", None, RECORDS_BACKUP_REASON);
    let copy =
        set_path(&resource, "unattended-unix-900-000000000").join("RABBIT/install-state.json");
    write(&copy, b"{}");
    records.receipt_backup_path = Some(copy);
    save_backup_manifest(
        &set_path(&resource, "unattended-unix-900-000000000"),
        &records,
    )
    .unwrap();
    save_backup_manifest(
        &set_path(&resource, "unix-1000"),
        &manifest("unix-1000", None, "something-new"),
    )
    .unwrap();
    let deleting = resource.join("RABBIT").join(DELETING_DIR).join("unix-1100");
    write(&deleting.join(reapack), b"old");
    let elsewhere = dir.path().join("Elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    link_folder(&set_path(&resource, "unix-1200"), &elsewhere);

    let overview = overview(&resource);

    let groups: Vec<(BackupItem, BackupGroup, BackupSetKind)> = overview
        .entries
        .iter()
        .map(|entry| (entry.item.clone(), entry.group, entry.set_kind))
        .collect();
    use BackupGroup::{EarlierVersions, Keymaps, Other, UninstalledSoftware};
    use BackupSetKind as Kind;
    assert_eq!(
        groups,
        [
            (
                package("uninstall-unix-100", "sws"),
                UninstalledSoftware,
                Kind::Uninstall
            ),
            (
                package("restore-unix-500-before-restore", "reapack"),
                EarlierVersions,
                Kind::BeforeRestore
            ),
            (
                package("unix-200", "reapack"),
                EarlierVersions,
                Kind::Update
            ),
            (
                keymap("restore-unix-400-before-restore"),
                Keymaps,
                Kind::BeforeRestore
            ),
            (
                keymap("unattended-unix-300-000000000"),
                Keymaps,
                Kind::Keymap
            ),
            (
                whole("restore-unix-600-before-restore"),
                Other,
                Kind::BeforeRestore
            ),
            (
                BackupItem::Deleting {
                    set_id: "unix-1100".to_string()
                },
                Other,
                Kind::Deleting
            ),
            (whole("unix-1000"), Other, Kind::Unknown),
            (whole("unix-800"), Other, Kind::Damaged),
            (whole("unix-700"), Other, Kind::NoManifest),
            (whole("unix-1200"), Other, Kind::NotABackup),
            (
                BackupItem::Records {
                    set_ids: vec!["unattended-unix-900-000000000".to_string()]
                },
                Other,
                Kind::Records
            ),
        ]
    );
    // Uninstalled software and key maps are the only copy.
    for entry in &overview.entries {
        assert_eq!(
            entry.only_copy,
            matches!(entry.group, UninstalledSoftware | Keymaps),
            "{entry:?}"
        );
    }
    // The command line restores another restore's backup and a set without
    // a manifest as a whole, and a dry run of that restore agrees; nothing
    // else in Other backups.
    let whole_sets = ["restore-unix-600-before-restore", "unix-700"];
    for entry in &overview.entries {
        let restores = matches!(&entry.item, BackupItem::Set { set_id } if whole_sets.contains(&set_id.as_str()));
        if entry.group == Other {
            assert_eq!(
                entry.restore_scope,
                restores.then_some(RestoreScope::WholeSet),
                "{entry:?}"
            );
        }
    }
    for id in whole_sets {
        restore_backup_set_within(
            &resource,
            id,
            &RestoreBackupOptions {
                dry_run: true,
                allow_reaper_running: true,
                package_id: None,
            },
            &OutsideRestoreRules::default(),
        )
        .unwrap();
    }
    assert_eq!(
        find(&overview, &keymap("unattended-unix-300-000000000")).keymap_origin,
        Some(KeymapOrigin::OwnKeymap)
    );
    assert_eq!(
        find(&overview, &keymap("restore-unix-400-before-restore")).keymap_origin,
        None
    );
    let swapped = find(
        &overview,
        &package("restore-unix-500-before-restore", "reapack"),
    );
    assert_eq!(swapped.version, Some(version("1.2.5")));
    assert_eq!(swapped.replaced_by, Some(version("1.2.4")));
}

/// A set's size comes from a walk through it: every file, the manifest and
/// a copy with no entry included, which is what deleting it frees. A
/// backup's size adds up its own manifest entries.
#[test]
fn sizes_come_from_the_walk_and_from_the_manifests() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let sws = "UserPlugins/reaper_sws-x64.dll";
    let ffmpeg = "UserPlugins/avcodec-61.dll";
    write(&resource.join(sws), &[1; 10]);
    write(&resource.join(ffmpeg), &[2; 3000]);
    let set = uninstall_set(
        &resource,
        "uninstall-unix-100",
        &[
            ("sws", Some(entry("sws", "2.14", &[])), &[sws]),
            ("ffmpeg", Some(entry("ffmpeg", "7.1", &[])), &[ffmpeg]),
        ],
    );
    // A copy a crash left without an entry.
    write(&set.join("UserPlugins/left-over.dll"), &[3; 500]);
    let manifest_size = fs::metadata(set.join("backup-manifest.json"))
        .unwrap()
        .len();
    layout_set(
        &resource,
        "unix-200",
        &[("a.dll", &[4; 40]), ("b/c.dll", &[5; 60])],
    );

    let overview = overview(&resource);

    let uninstall = overview
        .sets
        .iter()
        .find(|set| set.id == "uninstall-unix-100")
        .unwrap();
    assert_eq!(uninstall.size, 10 + 3000 + 500 + manifest_size);
    assert_eq!(uninstall.file_count, 4);
    assert_eq!(
        find(&overview, &package("uninstall-unix-100", "sws")).size,
        10
    );
    assert_eq!(
        find(&overview, &package("uninstall-unix-100", "ffmpeg")).size,
        3000
    );
    // A set without a manifest is one backup, as big as the set.
    assert_eq!(find(&overview, &whole("unix-200")).size, 100);
    assert_eq!(overview.total_size, uninstall.size + 100);
    assert_eq!(
        overview
            .sets
            .iter()
            .find(|set| set.id == "unix-200")
            .unwrap()
            .fingerprint,
        Some(SetFingerprint::Contents {
            file_count: 2,
            size: 100
        })
    );
    assert!(matches!(
        uninstall.fingerprint,
        Some(SetFingerprint::Manifest { length, .. }) if length == manifest_size
    ));
}

/// Every status uninstalled software can have, from the set, the receipt
/// and what is on disk, and the command-line restore only for those that
/// can be restored.
#[test]
fn uninstalled_software_gets_the_status_its_case_calls_for() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let files: [(&str, &[u8]); 7] = [
        ("UserPlugins/reaper_sws-x64.dll", b"sws"),
        ("S&M.ini", b"settings"),
        ("UserPlugins/reaper_reapack-x64.dll", b"reapack 1.2.4"),
        ("UserPlugins/avcodec-61.dll", b"avcodec"),
        ("UserPlugins/avformat-61.dll", b"avformat"),
        ("UserPlugins/reaper_kontrol.dll", b"kontrol"),
        ("UserPlugins/reaper_osara64.dll", b"osara"),
    ];
    for (path, contents) in files {
        write(&resource.join(path), contents);
    }
    let [sws, settings, reapack, avcodec, _, kontrol, osara] = files.map(|(path, _)| path);
    let sws_entry = entry("sws", "2.14.0.7", &files[..1]);
    let ffmpeg_entry = entry("ffmpeg", "7.1", &files[3..5]);
    let kontrol_entry = entry("reakontrol", "1.0", &files[5..6]);
    let id = "uninstall-unix-100";
    let set = uninstall_set(
        &resource,
        id,
        &[
            ("sws", Some(sws_entry.clone()), &[sws, settings]),
            (
                "reapack",
                Some(entry("reapack", "1.2.4", &files[2..3])),
                &[reapack],
            ),
            // Stopped after the first of FFmpeg's files.
            ("ffmpeg", Some(ffmpeg_entry.clone()), &[avcodec]),
            ("reakontrol", Some(kontrol_entry.clone()), &[kontrol]),
            // Its file was gone already.
            ("langpack-de", Some(entry("langpack-de", "1", &[])), &[]),
            ("osara", Some(entry("osara", "2024.1", &[])), &[osara]),
        ],
    );
    edit_manifest(&set, |manifest| {
        manifest.packages.get_mut("sws").unwrap().with_settings = Some(true);
    });
    // ReaPack was installed again since; ReaKontrol was restored; OSARA's
    // copy went missing from the set.
    write(&resource.join(reapack), b"reapack 1.2.5");
    write(&resource.join(kontrol), b"kontrol");
    fs::remove_file(set.join(osara)).unwrap();
    save_receipt(
        &resource,
        vec![
            entry("reapack", "1.2.5", &[(reapack, b"reapack 1.2.5")]),
            ffmpeg_entry,
            kontrol_entry,
        ],
    );

    let listed = overview(&resource);

    let status = |package_id: &str| {
        let entry = find(&listed, &package(id, package_id));
        (entry.status, entry.restore_scope)
    };
    use BackupStatus::*;
    let by_package = Some(RestoreScope::Package);
    assert_eq!(status("sws"), (CanRestore, by_package));
    assert_eq!(status("reapack"), (InstalledAgain, None));
    assert_eq!(status("ffmpeg"), (StoppedPartway, by_package));
    assert_eq!(status("reakontrol"), (AlreadyBack, by_package));
    assert_eq!(status("langpack-de"), (NothingInIt, None));
    assert_eq!(status("osara"), (PartMissing, None));
    let osara_entry = find(&listed, &package(id, "osara"));
    assert_eq!(osara_entry.missing_path.as_deref(), Some(Path::new(osara)));
    let sws_row = find(&listed, &package(id, "sws"));
    // What is already back is no longer the only copy.
    assert!(sws_row.only_copy);
    assert!(!find(&listed, &package(id, "reakontrol")).only_copy);
    assert_eq!(sws_row.version, Some(version("2.14.0.7")));
    assert_eq!(sws_row.with_settings, Some(true));
    assert_eq!(sws_row.size, 3 + 8);
    // Every row of the set goes with the others, and names them.
    assert_eq!(sws_row.shares_set_with.len(), 5);
    assert!(sws_row.shares_set_with.contains(&package(id, "osara")));
    assert!(!sws_row.shares_set_with.contains(&package(id, "sws")));
    assert!(listed.receipt_error.is_none());

    // Whatever the overview says can be restored, a dry run restores, and
    // what it says can't be, a dry run refuses with the refusal the row
    // carries, code and all.
    let dry_run = |package_id: &str| {
        restore_backup_set_within(
            &resource,
            id,
            &RestoreBackupOptions {
                dry_run: true,
                allow_reaper_running: true,
                package_id: Some(package_id.to_string()),
            },
            &OutsideRestoreRules::default(),
        )
    };
    for package_id in ["sws", "ffmpeg", "reakontrol"] {
        dry_run(package_id).unwrap();
        assert_eq!(find(&listed, &package(id, package_id)).refusal, None);
    }
    let agrees = |listed: &BackupOverview, package_id: &str, code: &str| {
        let row = find(listed, &package(id, package_id));
        let error = dry_run(package_id).unwrap_err();
        assert_eq!(refusal(&error).code(), code, "{package_id}: {error:?}");
        assert_eq!(row.refusal.as_ref(), Some(refusal(&error)), "{package_id}");
    };
    agrees(&listed, "reapack", "reinstalled-since");
    agrees(&listed, "langpack-de", "nothing-of-it-in-set");
    agrees(&listed, "osara", "part-missing");

    // Unreadable records: only what the set alone settles stays.
    write(&resource.join("RABBIT/install-state.json"), b"{ not json");
    let listed = overview(&resource);
    let unreadable = |package_id: &str| find(&listed, &package(id, package_id)).status;
    for package_id in ["sws", "reapack", "ffmpeg", "reakontrol"] {
        assert_eq!(unreadable(package_id), RecordsUnreadable, "{package_id}");
    }
    assert_eq!(unreadable("langpack-de"), NothingInIt);
    assert_eq!(unreadable("osara"), PartMissing);
    assert!(listed.receipt_error.is_some());
    agrees(&listed, "sws", "receipt-unreadable");
}

/// Earlier versions follow the swap's rules (backup plan 6.4). Those that
/// can be restored are restored by package, and a dry run of each agrees:
/// it goes ahead where the row says it can, and otherwise refuses with the
/// refusal the row carries. The whole set isn't offered while one of its
/// packages is refused, and a dry run of it is refused the same way.
#[test]
fn earlier_versions_follow_the_swap_rules() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let reapack = "UserPlugins/reaper_reapack-x64.dll";
    let sws = "UserPlugins/reaper_sws-x64.dll";
    let sws_helper = "UserPlugins/sws_helper.dll";
    let kontrol = "UserPlugins/reaper_kontrol.dll";
    let kontrol_maps = "UserPlugins/kontrol_maps.dll";
    let ffmpeg = "UserPlugins/avcodec-61.dll";
    let german = "LangPack/de_DE.ReaperLangPack";
    let before: [(&str, &[u8]); 7] = [
        (reapack, b"reapack 1.2.4"),
        (sws, b"sws 2.14"),
        (sws_helper, b"sws helper 2.14"),
        (kontrol, b"kontrol 1.0"),
        (kontrol_maps, b"maps"),
        (ffmpeg, b"ffmpeg 7.0"),
        (german, b"german 1"),
    ];
    for (path, contents) in before {
        write(&resource.join(path), contents);
    }
    let reapack_old = entry("reapack", "1.2.4", &before[..1]);
    let sws_old = entry("sws", "2.14", &before[1..3]);
    let kontrol_old = entry("reakontrol", "1.0", &before[3..5]);
    let ffmpeg_old = entry("ffmpeg", "7.0", &before[5..6]);
    let german_old = entry("langpack-de", "1", &before[6..7]);
    let id = "unix-100";
    update_set(
        &resource,
        id,
        &[
            ("reapack", Some(reapack_old), "1.2.5", &[reapack]),
            // The update replaced SWS's plug-in and deleted its helper.
            ("sws", Some(sws_old), "2.15", &[sws]),
            // It replaced ReaKontrol's plug-in and left its maps alone.
            ("reakontrol", Some(kontrol_old), "1.1", &[kontrol]),
            ("ffmpeg", Some(ffmpeg_old.clone()), "7.1", &[ffmpeg]),
            ("langpack-de", Some(german_old), "2", &[german]),
            // A record whose file never went in.
            ("jaws-scripts", None, "2026.1", &[]),
        ],
    );
    write(&resource.join(reapack), b"reapack 1.2.5");
    write(&resource.join(sws), b"sws 2.15");
    fs::remove_file(resource.join(sws_helper)).unwrap();
    write(&resource.join(kontrol), b"kontrol 1.1");
    write(&resource.join(german), b"german 2");
    // FFmpeg went back to 7.0 since, from a stable channel to dev.
    let mut ffmpeg_now = ffmpeg_old.clone();
    ffmpeg_now.channel = Some("dev".to_string());
    save_receipt(
        &resource,
        vec![
            entry("reapack", "1.2.5", &[(reapack, b"reapack 1.2.5")]),
            entry("sws", "2.15", &[(sws, b"sws 2.15")]),
            entry(
                "reakontrol",
                "1.1",
                &[(kontrol, b"kontrol 1.1"), (kontrol_maps, b"maps")],
            ),
            ffmpeg_now,
            entry("langpack-de", "2", &[(german, b"german 2")]),
            entry("langpack-es", "1", &[]),
        ],
    );

    let listed = overview(&resource);

    let row = |package_id: &str| find(&listed, &package(id, package_id));
    use BackupStatus::*;
    assert_eq!(row("reapack").status, CanRestore);
    assert_eq!(row("reapack").version, Some(version("1.2.4")));
    assert_eq!(row("reapack").replaced_by, Some(version("1.2.5")));
    assert_eq!(row("sws").status, NotComplete);
    assert_eq!(
        row("sws").missing_path.as_deref(),
        Some(Path::new(sws_helper))
    );
    // The refusal a restore of it gives, with the file that makes it so.
    assert_eq!(
        row("sws").refusal,
        Some(RestoreRefusal::NotComplete {
            backup_id: id.to_string(),
            package_id: "sws".to_string(),
            path: PathBuf::from(sws_helper),
        })
    );
    assert_eq!(row("reakontrol").status, CanRestore);
    assert_eq!(row("ffmpeg").status, AlreadyBack);
    // Language packs coexist: another one installed stops nothing.
    assert_eq!(row("langpack-de").status, CanRestore);
    assert_eq!(row("jaws-scripts").status, NothingInIt);
    for entry in &listed.entries {
        assert_eq!(entry.group, BackupGroup::EarlierVersions);
        assert_eq!(
            entry.restore_scope,
            entry.status.can_restore().then_some(RestoreScope::Package),
            "{entry:?}"
        );
        assert!(!entry.only_copy);
    }
    let dry_run = |environment: &BackupEnvironment<'_>, package_id: Option<&str>| {
        restore_backup_set_with(
            &resource,
            id,
            &RestoreBackupOptions {
                dry_run: true,
                allow_reaper_running: true,
                package_id: package_id.map(ToString::to_string),
            },
            environment,
            OutsideRestoreRules::default,
        )
    };
    let agrees =
        |listed: &BackupOverview, environment: &BackupEnvironment<'_>, package_id: &str| {
            let row = find(listed, &package(id, package_id));
            match dry_run(environment, Some(package_id)) {
                Ok(report) => {
                    assert_eq!(
                        row.restore_scope,
                        Some(RestoreScope::Package),
                        "{package_id}"
                    );
                    assert_eq!(row.refusal, None, "{package_id}");
                    let mut written: Vec<PathBuf> = report
                        .actions
                        .iter()
                        .map(|action| action.target_path.clone())
                        .collect();
                    written.sort();
                    assert!(!written.is_empty(), "{package_id}");
                    assert_eq!(listed.restore_targets(&row.item), written, "{package_id}");
                    assert_eq!(
                        restore_targets_with(&resource, &row.item, environment).unwrap(),
                        written,
                        "{package_id}"
                    );
                }
                Err(error) => {
                    assert_eq!(row.restore_scope, None, "{package_id}");
                    assert_eq!(row.refusal.as_ref(), Some(refusal(&error)), "{package_id}");
                }
            }
        };
    let as_user = BackupEnvironment::AS_USER;
    for package_id in [
        "reapack",
        "sws",
        "reakontrol",
        "ffmpeg",
        "langpack-de",
        "jaws-scripts",
    ] {
        agrees(&listed, &as_user, package_id);
    }
    let set = listed.sets.iter().find(|set| set.id == id).unwrap();
    assert!(!set.swaps_whole);
    assert_eq!(
        refusal(&dry_run(&as_user, None).unwrap_err()),
        row("sws").refusal.as_ref().unwrap()
    );

    // A package whose exclusive group has another member installed.
    let groups = || {
        BTreeMap::from([
            ("langpack-de".to_string(), "langpacks".to_string()),
            ("langpack-es".to_string(), "langpacks".to_string()),
        ])
    };
    let grouped = BackupEnvironment {
        exclusive_groups: &groups,
        ..BackupEnvironment::AS_USER
    };
    let listed = overview_with(&resource, &grouped).unwrap();
    let german = find(&listed, &package(id, "langpack-de"));
    assert_eq!(german.status, OtherLanguagePack);
    assert_eq!(
        german.refusal,
        Some(RestoreRefusal::OtherLanguagePackInstalled {
            backup_id: id.to_string(),
            package_id: "langpack-de".to_string(),
            installed: vec!["langpack-es".to_string()],
        })
    );
    assert_eq!(find(&listed, &package(id, "reapack")).status, CanRestore);
    agrees(&listed, &grouped, "langpack-de");
    agrees(&listed, &grouped, "reapack");

    // Unreadable records.
    write(&resource.join("RABBIT/install-state.json"), b"[");
    let listed = overview(&resource);
    assert_eq!(
        find(&listed, &package(id, "reapack")).status,
        RecordsUnreadable
    );
    assert_eq!(
        find(&listed, &package(id, "jaws-scripts")).status,
        NothingInIt
    );
    agrees(&listed, &as_user, "reapack");
    agrees(&listed, &as_user, "jaws-scripts");
}

/// An update's set from before records were kept: its earlier entries come
/// from its copy of RABBIT's receipt, which is read, and without one the
/// version is unknown. A package the copy doesn't list wasn't RABBIT's.
#[test]
fn older_update_sets_are_read_through_their_copy_of_the_records() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let clap = "UserPlugins/app2clap.dll";
    let demo = "UserPlugins/demo.dll";
    let osara = "UserPlugins/reaper_osara64.dll";
    write(&resource.join(clap), b"app2clap 1.0");
    write(&resource.join(demo), b"demo");
    write(&resource.join(osara), b"osara 1");
    let mut writer = BackupSetWriter::new(&resource, "unix-100", UPDATE_BACKUP_REASON);
    writer
        .copy_file(Some("app2clap"), &resource.join(clap))
        .unwrap();
    writer
        .copy_file(Some("demo"), &resource.join(demo))
        .unwrap();
    let with_copy = writer.path().to_path_buf();
    let mut copy_of_records = InstallState::default();
    copy_of_records.packages.insert(
        "app2clap".to_string(),
        entry("app2clap", "1.0", &[(clap, b"app2clap 1.0")]),
    );
    let copy = with_copy.join("RABBIT/install-state.json");
    let copied = serde_json::to_vec(&copy_of_records).unwrap();
    write(&copy, &copied);
    edit_manifest(&with_copy, |manifest| {
        manifest.files.push(BackupManifestFile {
            package_id: None,
            original_path: PathBuf::from("RABBIT/install-state.json"),
            backup_path: copy.clone(),
            size: copied.len() as u64,
            sha256: sha256_bytes(&copied),
            state: BackupEntryState::Done,
        });
        manifest.receipt_backup_path = Some(copy.clone());
    });
    BackupSetWriter::new(&resource, "unix-200", UPDATE_BACKUP_REASON)
        .copy_file(Some("osara"), &resource.join(osara))
        .unwrap();
    write(&resource.join(clap), b"app2clap 1.1");
    write(&resource.join(demo), b"demo 2");
    save_receipt(
        &resource,
        vec![
            entry("app2clap", "1.1", &[(clap, b"app2clap 1.1")]),
            entry("demo", "2", &[(demo, b"demo 2")]),
        ],
    );

    let listed = overview(&resource);

    let app2clap = find(&listed, &package("unix-100", "app2clap"));
    assert_eq!(app2clap.status, BackupStatus::CanRestore);
    assert_eq!(app2clap.version, Some(version("1.0")));
    assert_eq!(app2clap.replaced_by, None);
    let demo_row = find(&listed, &package("unix-100", "demo"));
    assert_eq!(demo_row.status, BackupStatus::CanRestore);
    assert_eq!(demo_row.version, None);
    assert_eq!(
        find(&listed, &package("unix-200", "osara")).status,
        BackupStatus::VersionUnknown
    );
    // The copy of the records is no backup of its own.
    assert_eq!(
        listed
            .entries
            .iter()
            .filter(|entry| entry.item.set_id() == Some("unix-100"))
            .count(),
        2
    );
    let set = listed.sets.iter().find(|set| set.id == "unix-100").unwrap();
    assert_eq!(set.receipt_copy.as_deref(), Some(copy.as_path()));
    assert!(set.holds_receipt);
}

/// A set that belongs to another account than `RABBIT/backups` can't be
/// restored, whatever it holds; on Windows no set ever does. A link in
/// `RABBIT/backups` is no backup.
#[test]
fn another_accounts_sets_and_links_are_never_restored() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let sws = "UserPlugins/reaper_sws-x64.dll";
    let kb = "reaper-kb.ini";
    write(&resource.join(sws), b"sws");
    write(&resource.join(kb), b"keys");
    uninstall_set(
        &resource,
        "uninstall-unix-100",
        &[("sws", Some(entry("sws", "1", &[])), &[sws])],
    );
    BackupSetWriter::new(
        &resource,
        "unattended-unix-200-000000000",
        KEYMAP_BACKUP_REASON,
    )
    .copy_file(Some("osara"), &resource.join(kb))
    .unwrap();
    layout_set(&resource, "unix-300", &[("a.dll", b"a")]);
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("big.dll"), &[0; 4096]);
    link_folder(&set_path(&resource, "unix-400"), &elsewhere);
    let roots_set = set_path(&resource, "uninstall-unix-100");
    let owner_of = |path: &Path| {
        Some(if path == roots_set || path.ends_with("unix-300") {
            0
        } else {
            501
        })
    };
    let under_sudo = BackupEnvironment {
        owner_of: &owner_of,
        ..BackupEnvironment::AS_USER
    };

    let listed = overview_with(&resource, &under_sudo).unwrap();

    let uninstalled = find(&listed, &package("uninstall-unix-100", "sws"));
    assert_eq!(uninstalled.status, BackupStatus::NotYours);
    assert_eq!(uninstalled.restore_scope, None);
    assert_eq!(
        find(&listed, &whole("unix-300")).status,
        BackupStatus::NotYours
    );
    let keys = find(&listed, &keymap("unattended-unix-200-000000000"));
    assert_eq!(keys.status, BackupStatus::CanRestore);
    assert_eq!(keys.restore_scope, Some(RestoreScope::Keymap));
    let link = find(&listed, &whole("unix-400"));
    assert_eq!(link.status, BackupStatus::NotABackup);
    assert_eq!((link.size, link.restore_scope), (0, None));
    let link_set = listed.sets.iter().find(|set| set.id == "unix-400").unwrap();
    assert!(link_set.files.is_empty() && link_set.fingerprint.is_none());
    assert!(
        link_set
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("not a backup RABBIT made"))
    );
    assert!(listed.sets.iter().all(|set| set.owned_by_another_account
        == matches!(set.id.as_str(), "uninstall-unix-100" | "unix-300")));

    // A restore refuses another account's set as the row says, in a dry
    // run too, before anything is read of it.
    for (item, package_id) in [
        (package("uninstall-unix-100", "sws"), Some("sws")),
        (whole("unix-300"), None),
    ] {
        let error = crate::rollback::restore_backup_set_with(
            &resource,
            item.set_id().unwrap(),
            &RestoreBackupOptions {
                dry_run: true,
                allow_reaper_running: true,
                package_id: package_id.map(ToString::to_string),
            },
            &under_sudo,
            OutsideRestoreRules::default,
        )
        .unwrap_err();
        assert_eq!(refusal(&error).code(), "not-yours", "{item:?}");
        assert_eq!(
            find(&listed, &item).refusal.as_ref(),
            Some(refusal(&error)),
            "{item:?}"
        );
    }

    // As this computer reads owners, every set is the user's own.
    let listed = overview(&resource);
    assert!(listed.sets.iter().all(|set| !set.owned_by_another_account));
    assert_eq!(
        find(&listed, &package("uninstall-unix-100", "sws")).status,
        BackupStatus::CanRestore
    );
}

/// No link or junction is followed: not one in `RABBIT/backups`, not one
/// inside a set, and nothing is listed while `RABBIT/backups` or
/// `RABBIT/backups-deleting` is one.
#[test]
fn links_are_never_followed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("big.dll"), &[0; 8192]);
    let set = layout_set(&resource, "unix-100", &[("a.dll", &[1; 10])]);
    link_folder(&set.join("Linked"), &elsewhere);
    link_folder(&set_path(&resource, "unix-200"), &elsewhere);

    let listed = overview(&resource);

    let inside = listed.sets.iter().find(|set| set.id == "unix-100").unwrap();
    assert!(inside.size < 8192, "{}", inside.size);
    assert_eq!(inside.files, [PathBuf::from("a.dll")]);
    assert_eq!(inside.links, [PathBuf::from("Linked")]);
    assert!(
        inside
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("Linked"))
    );
    // A set with a link in it isn't restored as a whole.
    assert_eq!(find(&listed, &whole("unix-100")).restore_scope, None);
    let linked = listed.sets.iter().find(|set| set.id == "unix-200").unwrap();
    assert_eq!((linked.kind, linked.size), (BackupSetKind::NotABackup, 0));
    assert_eq!(listed.total_size, inside.size);
    assert_eq!(fs::read(elsewhere.join("big.dll")).unwrap(), vec![0; 8192]);

    // A backups or deletions folder that is a link lists nothing.
    for folder in ["backups", DELETING_DIR] {
        let other = dir.path().join(format!("Other-{folder}"));
        let rabbit = other.join("RABBIT");
        fs::create_dir_all(&rabbit).unwrap();
        let linked = rabbit.join(folder);
        link_folder(&linked, &resource.join("RABBIT").join("backups"));
        let error = overview_with(&other, &BackupEnvironment::AS_USER).unwrap_err();
        assert!(
            matches!(&error, RabbitError::BackupPathIsLink { path } if *path == linked),
            "{error:?}"
        );
    }
}

/// A damaged manifest never hides the other sets: the set is listed as
/// damaged, with a warning, by its walk, and among what couldn't be read.
#[test]
fn a_damaged_manifest_is_still_listed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let kb = "reaper-kb.ini";
    write(&resource.join(kb), b"keys");
    BackupSetWriter::new(&resource, "unix-1", KEYMAP_BACKUP_REASON)
        .copy_file(Some("osara"), &resource.join(kb))
        .unwrap();
    let damaged = layout_set(&resource, "unix-2", &[(kb, b"keys")]);
    write(&damaged.join("backup-manifest.json"), b"{ not json");

    let listed = overview(&resource);

    assert_eq!(listed.sets.len(), 2);
    let set = listed.sets.iter().find(|set| set.id == "unix-2").unwrap();
    assert_eq!(set.kind, BackupSetKind::Damaged);
    assert!(set.manifest.is_none() && set.manifest_path.is_none());
    assert_eq!(set.created_unix, Some(2));
    assert!(matches!(
        set.fingerprint,
        Some(SetFingerprint::Manifest { .. })
    ));
    assert!(
        set.warning
            .as_deref()
            .is_some_and(|warning| warning.contains("manifest"))
    );
    let row = find(&listed, &whole("unix-2"));
    assert_eq!(
        (row.status, row.restore_scope),
        (BackupStatus::NotHere, None)
    );
    assert_eq!(listed.unreadable.len(), 1);
    assert_eq!(listed.unreadable[0].id, "unix-2");
    assert_eq!(
        find(&listed, &keymap("unix-1")).status,
        BackupStatus::CanRestore
    );
}

/// The overview hashes nothing, whatever it has to judge: pending copies,
/// earlier versions checked for completeness against the receipt, a copy
/// of the records read for an older update, sets without a manifest; and
/// neither do the restore targets or the JSON listing.
#[test]
fn nothing_is_hashed() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let sws = "UserPlugins/reaper_sws-x64.dll";
    let reapack = "UserPlugins/reaper_reapack-x64.dll";
    let kb = "reaper-kb.ini";
    let files: [(&str, &[u8]); 3] = [(sws, b"sws"), (reapack, b"reapack 1"), (kb, b"keys")];
    for (path, contents) in files {
        write(&resource.join(path), contents);
    }
    let uninstall = uninstall_set(
        &resource,
        "uninstall-unix-100",
        &[("sws", Some(entry("sws", "1", &[(sws, b"sws")])), &[sws])],
    );
    // An entry whose copy never finished, with its copy whole.
    edit_manifest(&uninstall, |manifest| {
        manifest.files[0].state = BackupEntryState::Pending;
    });
    update_set(
        &resource,
        "unix-200",
        &[(
            "reapack",
            Some(entry(
                "reapack",
                "1",
                &[(reapack, b"reapack 1"), ("UserPlugins/other.dll", b"o")],
            )),
            "2",
            &[reapack],
        )],
    );
    let mut older = BackupSetWriter::new(&resource, "unix-300", UPDATE_BACKUP_REASON);
    older
        .copy_file(Some("reapack"), &resource.join(reapack))
        .unwrap();
    let copy = older.path().join("RABBIT/install-state.json");
    write(&copy, b"{\"schema_version\":1,\"packages\":{}}");
    edit_manifest(older.path(), |manifest| {
        manifest.receipt_backup_path = Some(copy.clone());
    });
    BackupSetWriter::new(&resource, "unix-400", KEYMAP_BACKUP_REASON)
        .copy_file(Some("osara"), &resource.join(kb))
        .unwrap();
    layout_set(&resource, "unix-500", &[("a.dll", b"a")]);
    write(&resource.join(reapack), b"reapack 2");
    save_receipt(
        &resource,
        vec![entry(
            "reapack",
            "2",
            &[(reapack, b"reapack 2"), ("UserPlugins/other.dll", b"o")],
        )],
    );
    write(&resource.join("UserPlugins/other.dll"), b"o");
    let before = hashes_on_this_thread();

    let listed = overview(&resource);
    // The overview and the lookup of one set name the same targets.
    for entry in &listed.entries {
        let read_again = restore_targets_with(&resource, &entry.item, &BackupEnvironment::AS_USER)
            .unwrap_or_default();
        assert_eq!(listed.restore_targets(&entry.item), read_again, "{entry:?}");
        let _ = backup_restore_targets(&resource, &entry.item);
    }
    list_backup_sets(&resource).unwrap();

    assert_eq!(hashes_on_this_thread(), before);
    assert_eq!(
        find(&listed, &package("uninstall-unix-100", "sws")).status,
        BackupStatus::CanRestore
    );
    assert_eq!(
        find(&listed, &package("unix-200", "reapack")).status,
        BackupStatus::CanRestore
    );
    assert_eq!(listed.entries.len(), 5);
}

/// The files `backup_restore_targets` names for a package of an
/// uninstall's set are the files a real restore of that package writes,
/// and the overview names the same without reading the set again.
#[test]
fn restore_targets_are_what_a_restore_writes() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let sws = ["UserPlugins/reaper_sws-x64.dll", "S&M.ini", "Grooves/a.rgt"];
    let ffmpeg = "UserPlugins/avcodec-61.dll";
    for path in sws.iter().chain([&ffmpeg]) {
        write(&resource.join(path), path.as_bytes());
    }
    let id = "uninstall-unix-100";
    uninstall_set(
        &resource,
        id,
        &[
            ("sws", Some(entry("sws", "1", &[])), &sws),
            ("ffmpeg", Some(entry("ffmpeg", "7", &[])), &[ffmpeg]),
        ],
    );
    let item = package(id, "sws");
    let targets = backup_restore_targets(&resource, &item).unwrap();
    assert_eq!(overview(&resource).restore_targets(&item), targets);
    assert_eq!(targets.len(), 3);

    let report = restore_backup_set_within(
        &resource,
        id,
        &RestoreBackupOptions {
            dry_run: false,
            allow_reaper_running: true,
            package_id: Some("sws".to_string()),
        },
        &OutsideRestoreRules::default(),
    )
    .unwrap();

    let mut written: Vec<PathBuf> = report
        .actions
        .iter()
        .map(|action| action.target_path.clone())
        .collect();
    written.sort();
    assert_eq!(written, targets);
    for path in sws {
        assert_eq!(fs::read(resource.join(path)).unwrap(), path.as_bytes());
    }
    // The set's other package, a backup the overview doesn't list, the
    // records, a damaged set and unfinished deletions.
    assert_eq!(
        backup_restore_targets(&resource, &package(id, "ffmpeg")).unwrap(),
        [resource.join(ffmpeg)]
    );
    assert!(
        backup_restore_targets(&resource, &whole(id))
            .unwrap()
            .is_empty()
    );
    let damaged = layout_set(&resource, "unix-200", &[(ffmpeg, b"old")]);
    write(&damaged.join("backup-manifest.json"), b"{ not json");
    assert!(
        backup_restore_targets(&resource, &whole("unix-200"))
            .unwrap()
            .is_empty()
    );
    assert!(
        overview(&resource)
            .restore_targets(&whole("unix-200"))
            .is_empty()
    );
    let records = BackupItem::Records {
        set_ids: vec![id.to_string()],
    };
    assert!(
        backup_restore_targets(&resource, &records)
            .unwrap()
            .is_empty()
    );
    let error = backup_restore_targets(&resource, &package("unix-9", "sws")).unwrap_err();
    assert_eq!(refusal(&error).code(), "not-found", "{error:?}");
}

/// Every set that holds only a copy of RABBIT's records is part of one
/// backup, last of all, as big as all of them, naming them newest first.
/// One that belongs to another account stands alone.
#[test]
fn the_records_are_one_backup_last_of_all() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    for (id, size) in [
        ("unattended-unix-100-000000000", 10),
        ("unattended-unix-300-000000000", 20),
        ("unattended-unix-200-000000000", 30),
    ] {
        let set = set_path(&resource, id);
        let copy = set.join("RABBIT/install-state.json");
        write(&copy, &vec![b' '; size]);
        let mut records = manifest(id, None, RECORDS_BACKUP_REASON);
        records.receipt_backup_path = Some(copy);
        save_backup_manifest(&set, &records).unwrap();
    }
    layout_set(&resource, "backup-by-hand", &[("a.dll", b"a")]);
    let not_mine = set_path(&resource, "unattended-unix-200-000000000");
    let owner_of = |path: &Path| Some(if path == not_mine { 0 } else { 501 });
    let environment = BackupEnvironment {
        owner_of: &owner_of,
        ..BackupEnvironment::AS_USER
    };

    let listed = overview_with(&resource, &environment).unwrap();

    let last = listed.entries.last().unwrap();
    assert_eq!(
        last.item,
        BackupItem::Records {
            set_ids: vec![
                "unattended-unix-300-000000000".to_string(),
                "unattended-unix-100-000000000".to_string(),
            ]
        }
    );
    assert_eq!(
        (last.group, last.status, last.created_unix, last.only_copy),
        (BackupGroup::Other, BackupStatus::NotHere, Some(300), false)
    );
    let sizes: u64 = listed
        .sets
        .iter()
        .filter(|set| set.id != "backup-by-hand" && set.path != not_mine)
        .map(|set| set.size)
        .sum();
    assert_eq!(last.size, sizes);
    assert_eq!(
        find(&listed, &whole("unattended-unix-200-000000000")).status,
        BackupStatus::NotYours
    );
    // The undated set comes after the dated ones, but before the records.
    assert_eq!(
        listed.entries[listed.entries.len() - 2].item,
        whole("backup-by-hand")
    );
    // In the JSON listing, each records set names the backup it is part of.
    let sets = listed.backup_sets();
    let newest = sets
        .iter()
        .find(|set| set.id == "unattended-unix-300-000000000")
        .unwrap();
    assert_eq!(newest.items, std::slice::from_ref(last));
}

/// Newest first inside each group, backups without a date last.
#[test]
fn backups_are_newest_first_inside_their_group() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let sws = "UserPlugins/reaper_sws-x64.dll";
    write(&resource.join(sws), b"sws");
    for (id, created) in [
        ("uninstall-unix-100", Some(100)),
        ("uninstall-unix-300", Some(300)),
        ("uninstall-unix-200", None),
    ] {
        let set = uninstall_set(&resource, id, &[("sws", None, &[sws])]);
        edit_manifest(&set, |manifest| {
            manifest.created_unix = created;
            if created.is_none() {
                manifest.created_at = "by hand".to_string();
            }
        });
        write(&resource.join(sws), b"sws");
    }

    let listed = overview(&resource);

    let order: Vec<BackupItem> = listed
        .entries
        .iter()
        .map(|entry| entry.item.clone())
        .collect();
    assert_eq!(
        order,
        [
            package("uninstall-unix-300", "sws"),
            package("uninstall-unix-100", "sws"),
            package("uninstall-unix-200", "sws"),
        ]
    );
    let sets: Vec<&str> = listed.sets.iter().map(|set| set.id.as_str()).collect();
    assert_eq!(
        sets,
        [
            "uninstall-unix-300",
            "uninstall-unix-100",
            "uninstall-unix-200"
        ]
    );
}

/// An unfinished deletion in `RABBIT/backups-deleting` is listed as one,
/// and looking a set up by id finds it as a restore would, beside a set of
/// the same name; ids that name nothing are refused as a restore refuses
/// them.
#[test]
fn unfinished_deletions_are_listed_and_a_set_is_looked_up_strictly() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let kb = "reaper-kb.ini";
    write(&resource.join(kb), b"keys");
    BackupSetWriter::new(&resource, "unix-1", KEYMAP_BACKUP_REASON)
        .copy_file(Some("osara"), &resource.join(kb))
        .unwrap();
    let deleting = resource.join("RABBIT").join(DELETING_DIR);
    write(&deleting.join("unix-1").join("left.dll"), b"left");
    write(&deleting.join("unix-2").join(kb), b"keys");

    let listed = overview(&resource);

    let kinds: Vec<(&str, BackupSetKind)> = listed
        .sets
        .iter()
        .map(|set| (set.id.as_str(), set.kind))
        .collect();
    assert_eq!(kinds.len(), 3);
    assert!(kinds.contains(&("unix-1", BackupSetKind::Keymap)));
    assert!(kinds.contains(&("unix-1", BackupSetKind::Deleting)));
    assert!(kinds.contains(&("unix-2", BackupSetKind::Deleting)));
    let unfinished = find(
        &listed,
        &BackupItem::Deleting {
            set_id: "unix-2".to_string(),
        },
    );
    assert_eq!(
        (
            unfinished.status,
            unfinished.restore_scope,
            unfinished.group
        ),
        (BackupStatus::NotHere, None, BackupGroup::Other)
    );
    assert!(unfinished.shares_set_with.is_empty());
    assert!(listed.restore_targets(&unfinished.item).is_empty());

    let one = overview_of_with(&resource, "unix-1", &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(one.sets.len(), 2);
    assert_eq!(one.entries.len(), 2);
    let two = overview_of_with(&resource, "unix-2", &BackupEnvironment::AS_USER).unwrap();
    assert_eq!(two.sets[0].kind, BackupSetKind::Deleting);
    assert_eq!(two.sets[0].path, deleting.join("unix-2"));

    let refused = |id: &str| {
        let error = overview_of_with(&resource, id, &BackupEnvironment::AS_USER).unwrap_err();
        refusal(&error).code()
    };
    assert_eq!(refused("unix-3"), "not-found");
    assert_eq!(refused("UNIX-1"), "differs-in-case");
    assert_eq!(refused("a/b"), "invalid-id");
    assert_eq!(refused(".."), "invalid-id");
    #[cfg(windows)]
    assert_eq!(refused("unix-1."), "ends-in-dot-or-space");
}

/// A key map is found by what it holds, whatever made its set: an older
/// restore's backup of the key map lists as a key map, restored on its own.
#[test]
fn a_key_map_is_found_by_its_content() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let kb = "reaper-kb.ini";
    write(&resource.join(kb), b"keys");
    let set = set_path(&resource, "restore-unix-100-before-restore");
    let copy = set.join(kb);
    write(&copy, b"keys");
    let mut older = manifest(
        "restore-unix-100-before-restore",
        None,
        BEFORE_RESTORE_REASON,
    );
    // An older RABBIT recorded the key map's absolute path.
    older.files.push(BackupManifestFile {
        package_id: None,
        original_path: std::path::absolute(resource.join(kb)).unwrap(),
        backup_path: copy,
        size: 4,
        sha256: sha256_bytes(b"keys"),
        state: BackupEntryState::Done,
    });
    save_backup_manifest(&set, &older).unwrap();

    let listed = overview(&resource);

    let row = find(&listed, &keymap("restore-unix-100-before-restore"));
    assert_eq!(row.group, BackupGroup::Keymaps);
    assert_eq!(row.restore_scope, Some(RestoreScope::Keymap));
    assert_eq!(row.size, 4);
    assert_eq!(listed.entries.len(), 1);
    assert_eq!(listed.restore_targets(&row.item), [resource.join(kb)]);
}

/// `list_backup_sets` is the overview's sets, newest first, with every
/// backup in each.
#[test]
fn the_listing_is_built_on_the_overview() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let sws = "UserPlugins/reaper_sws-x64.dll";
    write(&resource.join(sws), b"sws");
    let set = uninstall_set(
        &resource,
        "uninstall-unix-100",
        &[("sws", Some(entry("sws", "1", &[])), &[sws])],
    );
    edit_manifest(&set, |manifest| manifest.created_unix = Some(100));
    layout_set(&resource, "unix-50", &[("a.dll", b"a")]);
    layout_set(&resource, "unix-200", &[("b.dll", b"b")]);

    let sets = list_backup_sets(&resource).unwrap();

    assert_eq!(sets, overview(&resource).backup_sets());
    let ids: Vec<&str> = sets.iter().map(|set| set.id.as_str()).collect();
    assert_eq!(ids, ["unix-200", "uninstall-unix-100", "unix-50"]);
    let uninstall = &sets[1];
    assert_eq!(uninstall.kind, Some(BackupSetKind::Uninstall));
    assert_eq!(uninstall.created_unix, Some(100));
    assert_eq!(uninstall.reason.as_deref(), Some(UNINSTALL_BACKUP_REASON));
    assert_eq!(uninstall.created_at.as_deref(), Some("uninstall-unix-100"));
    assert_eq!(uninstall.files, [PathBuf::from(sws)]);
    assert_eq!(uninstall.items.len(), 1);
    assert_eq!(
        uninstall.items[0].item,
        package("uninstall-unix-100", "sws")
    );
    assert!(uninstall.size > 3 && uninstall.fingerprint.is_some());
    // A REAPER RABBIT never touched has no backups, and gets no folder.
    let fresh = dir.path().join("Fresh");
    fs::create_dir_all(&fresh).unwrap();
    assert!(list_backup_sets(&fresh).unwrap().is_empty());
    assert!(!fresh.join("RABBIT").exists());
}

#[test]
fn sizes_read_in_whole_kb_and_mb_and_gb_with_a_tenth() {
    const KB: u64 = 1024;
    for (bytes, size) in [
        (0, BackupSize::UnderOneKb),
        (1023, BackupSize::UnderOneKb),
        (1024, BackupSize::Kb(1)),
        (1535, BackupSize::Kb(1)),
        (1536, BackupSize::Kb(2)),
        (38 * KB, BackupSize::Kb(38)),
        (1023 * KB, BackupSize::Kb(1023)),
        // 1023.5 KB rounds to 1024 KB, which is 1 MB.
        (1023 * KB + 512, BackupSize::Mb(1)),
        (250 * KB * KB, BackupSize::Mb(250)),
        (
            1023 * KB * KB + 512 * KB,
            BackupSize::Gb { whole: 1, tenth: 0 },
        ),
        (KB * KB * KB * 3 / 2, BackupSize::Gb { whole: 1, tenth: 5 }),
        (
            KB * KB * KB * 42 + KB * KB * 1000,
            BackupSize::Gb {
                whole: 43,
                tenth: 0,
            },
        ),
        (
            u64::MAX,
            BackupSize::Gb {
                whole: 17_179_869_184,
                tenth: 0,
            },
        ),
    ] {
        assert_eq!(BackupSize::of(bytes), size, "{bytes}");
    }
}

/// The command line restores a set without a manifest, or a restore's own
/// backup, as a whole only when a restore would take it: never one holding
/// RABBIT's receipt, a file from outside the resource folder without a
/// manifest to place it, a file a restore never puts back, which the
/// backup names, a damaged manifest, or two files for one place, which the
/// backup names by that place. A dry run of each restore agrees, and
/// refuses with the refusal the row carries, code, fields and all.
#[test]
fn whole_sets_restore_only_as_a_restore_would_take_them() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let plugin = "UserPlugins/old.dll";
    write(&resource.join(plugin), b"new");
    layout_set(&resource, "unix-100", &[(plugin, b"old")]);
    layout_set(
        &resource,
        "unix-200",
        &[(plugin, b"old"), ("RABBIT/install-state.json", b"{}")],
    );
    layout_set(
        &resource,
        "unix-300",
        &[(plugin, b"old"), ("external/1/x.dll", b"x")],
    );
    layout_set(
        &resource,
        "unix-400",
        &[(plugin, b"old"), ("RABBIT/logs/old.json", b"{}")],
    );
    // A restore's backup whose manifest names a file in RABBIT's folder.
    let mut writer = BackupSetWriter::new(&resource, "unix-500", BEFORE_RESTORE_REASON);
    writer.copy_file(None, &resource.join(plugin)).unwrap();
    write(&resource.join("RABBIT/logs/new.json"), b"{}");
    writer
        .copy_file(None, &resource.join("RABBIT/logs/new.json"))
        .unwrap();
    BackupSetWriter::new(&resource, "unix-600", BEFORE_RESTORE_REASON)
        .copy_file(None, &resource.join(plugin))
        .unwrap();
    layout_set(
        &resource,
        "unix-700",
        &[("backup-manifest.json", b"{ not json")],
    );
    // A restore's backup whose manifest names one file twice.
    let mut writer = BackupSetWriter::new(&resource, "unix-800", BEFORE_RESTORE_REASON);
    writer.copy_file(None, &resource.join(plugin)).unwrap();
    edit_manifest(writer.path(), |manifest| {
        let again = manifest.files[0].clone();
        manifest.files.push(again);
    });

    let listed = overview(&resource);

    let logs = |name: &str| Path::new("RABBIT").join("logs").join(name);
    // The set, how a restore takes it, the file the row names, and the
    // code of the refusal.
    type Case<'a> = (
        &'a str,
        Option<RestoreScope>,
        Option<PathBuf>,
        Option<&'a str>,
    );
    let cases: [Case<'_>; 8] = [
        ("unix-100", Some(RestoreScope::WholeSet), None, None),
        ("unix-200", None, None, Some("receipt-in-set")),
        ("unix-300", None, None, Some("target-refused")),
        (
            "unix-400",
            None,
            Some(logs("old.json")),
            Some("target-refused"),
        ),
        (
            "unix-500",
            None,
            Some(logs("new.json")),
            Some("target-refused"),
        ),
        ("unix-600", Some(RestoreScope::WholeSet), None, None),
        ("unix-700", None, None, Some("damaged-manifest")),
        (
            "unix-800",
            None,
            Some(PathBuf::from(plugin)),
            Some("target-refused"),
        ),
    ];
    for (id, scope, refused, code) in cases {
        let row = find(&listed, &whole(id));
        assert_eq!(row.status, BackupStatus::NotHere, "{id}");
        assert_eq!(row.restore_scope, scope, "{id}");
        // A manifest records the file's original path; a set without one
        // has only its place in the set.
        match (&row.refused_path, &refused) {
            (Some(path), Some(expected)) => assert!(path.ends_with(expected), "{id}: {path:?}"),
            (found, expected) => assert_eq!(found, expected, "{id}"),
        }
        assert_eq!(
            listed.restore_targets(&row.item).is_empty(),
            scope.is_none(),
            "{id}"
        );
        let dry_run = restore_backup_set_within(
            &resource,
            id,
            &RestoreBackupOptions {
                dry_run: true,
                allow_reaper_running: true,
                package_id: None,
            },
            &OutsideRestoreRules::default(),
        );
        assert_eq!(dry_run.is_ok(), scope.is_some(), "{id}: {dry_run:?}");
        let run_refusal = dry_run.as_ref().err().map(refusal);
        assert_eq!(run_refusal.map(RestoreRefusal::code), code, "{id}");
        assert_eq!(row.refusal.as_ref(), run_refusal, "{id}");
    }
    let reason = |id: &str| match &find(&listed, &whole(id)).refusal {
        Some(RestoreRefusal::TargetRefused { reason, .. }) => reason.clone(),
        other => panic!("{id}: {other:?}"),
    };
    assert_eq!(reason("unix-300"), TargetRefusal::OutsideWithoutManifest);
    assert_eq!(reason("unix-400"), TargetRefusal::RabbitFolder);
    assert_eq!(reason("unix-500"), TargetRefusal::RabbitFolder);
    assert_eq!(reason("unix-800"), TargetRefusal::TwoFilesForOnePlace);
    assert_eq!(
        listed.restore_targets(&whole("unix-100")),
        [resource.join(plugin)]
    );
}

/// A key map is a backup of its own, restored alone whatever else its set
/// holds, with its own size and targets. A set that holds other files too
/// gets a backup of its own for everything in it, which can be restored
/// whole. A copy of RABBIT's records in the set, which RABBIT never copies
/// back, refuses nothing of the key map. A set that holds only the key map
/// has only the key map's backup.
#[test]
fn a_key_map_is_restored_alone_whatever_else_its_set_holds() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let kb = "reaper-kb.ini";
    let osara = "UserPlugins/reaper_osara64.dll";
    write(&resource.join(kb), b"own keys");
    write(&resource.join(osara), &[1; 2000]);
    let id = "restore-unix-100-before-restore";
    let mut writer = BackupSetWriter::new(&resource, id, BEFORE_RESTORE_REASON);
    writer.copy_file(Some("osara"), &resource.join(kb)).unwrap();
    writer
        .copy_file(Some("osara"), &resource.join(osara))
        .unwrap();
    let alone = "restore-unix-200-before-restore";
    BackupSetWriter::new(&resource, alone, BEFORE_RESTORE_REASON)
        .copy_file(Some("osara"), &resource.join(kb))
        .unwrap();
    let with_records = "restore-unix-300-before-restore";
    let receipt = resource.join("RABBIT").join("install-state.json");
    write(&receipt, b"{}");
    let mut writer = BackupSetWriter::new(&resource, with_records, BEFORE_RESTORE_REASON);
    writer.copy_file(None, &resource.join(kb)).unwrap();
    writer.copy_file(None, &receipt).unwrap();

    let listed = overview(&resource);

    let keys = find(&listed, &keymap(id));
    assert_eq!(keys.status, BackupStatus::CanRestore);
    assert_eq!(
        (keys.restore_scope, keys.refusal.as_ref()),
        (Some(RestoreScope::Keymap), None)
    );
    assert_eq!(keys.size, 8);
    assert_eq!(listed.restore_targets(&keys.item), [resource.join(kb)]);
    assert_eq!(
        backup_restore_targets(&resource, &keys.item).unwrap(),
        [resource.join(kb)]
    );
    let everything = find(&listed, &whole(id));
    assert_eq!(everything.group, BackupGroup::Other);
    assert_eq!(everything.restore_scope, Some(RestoreScope::WholeSet));
    let mut both = vec![resource.join(kb), resource.join(osara)];
    both.sort();
    assert_eq!(listed.restore_targets(&everything.item), both);
    assert_eq!(keys.shares_set_with, [whole(id)]);
    assert_eq!(
        find(&listed, &keymap(alone)).restore_scope,
        Some(RestoreScope::Keymap)
    );
    assert!(
        !listed
            .entries
            .iter()
            .any(|entry| entry.item == whole(alone))
    );
    let records_keys = find(&listed, &keymap(with_records));
    assert_eq!(
        (records_keys.restore_scope, records_keys.refusal.as_ref()),
        (Some(RestoreScope::Keymap), None)
    );
    assert!(
        !listed
            .entries
            .iter()
            .any(|entry| entry.item == whole(with_records))
    );
}

/// A set can name a place outside the resource folder. The overview looks
/// at it only where a restore would write it, below a folder RABBIT
/// installs into. Anywhere else, such as on another computer, the place is
/// never looked at and never a target, and its backup is never already
/// back; the backup names the file a restore would refuse.
#[test]
fn places_outside_are_looked_at_only_where_a_restore_writes() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let clap = dir.path().join("CLAP");
    let elsewhere = dir.path().join("Elsewhere");
    let plugin = clap.join("app2clap.clap");
    let stray = elsewhere.join("x.dll");
    write(&plugin, b"clap");
    write(&stray, b"x");
    let id = "uninstall-unix-100";
    let mut writer = BackupSetWriter::new(&resource, id, UNINSTALL_BACKUP_REASON);
    for (package_id, file) in [("app2clap", &plugin), ("stray", &stray)] {
        writer
            .record_package(package_id, BackupPackageRecord::default())
            .unwrap();
        writer
            .move_file(Some(package_id), file, None, false)
            .unwrap();
    }
    // Both files are back where they were.
    write(&plugin, b"clap");
    write(&stray, b"x");
    #[cfg(windows)]
    let far = PathBuf::from(r"\\nonexistent.invalid\share\x.dll");
    #[cfg(windows)]
    {
        let set = writer.path().to_path_buf();
        let copy = set.join("external").join("9").join("x.dll");
        write(&copy, b"x");
        edit_manifest(&set, |manifest| {
            manifest
                .packages
                .insert("far".to_string(), BackupPackageRecord::default());
            manifest.files.push(BackupManifestFile {
                package_id: Some("far".to_string()),
                original_path: far.clone(),
                backup_path: copy,
                size: 1,
                sha256: sha256_bytes(b"x"),
                state: BackupEntryState::Done,
            });
        });
    }
    let rules = OutsideRestoreRules {
        shared_roots: vec![clap.clone()],
        vendor_added: Vec::new(),
    };
    let outside_rules = |_: &Path| rules.clone();
    let with_clap = BackupEnvironment {
        outside_rules: &outside_rules,
        ..BackupEnvironment::AS_USER
    };
    let before = places_looked_at_on_this_thread().len();

    let listed = overview_with(&resource, &with_clap).unwrap();
    let without = overview(&resource);

    let looked_at = places_looked_at_on_this_thread().split_off(before);
    assert!(looked_at.contains(&plugin), "{looked_at:?}");
    assert!(!looked_at.contains(&stray), "{looked_at:?}");
    #[cfg(windows)]
    assert!(!looked_at.contains(&far), "{looked_at:?}");
    let app2clap = find(&listed, &package(id, "app2clap"));
    assert_eq!(app2clap.status, BackupStatus::AlreadyBack);
    assert_eq!(app2clap.restore_scope, Some(RestoreScope::Package));
    assert_eq!(
        listed.restore_targets(&app2clap.item),
        std::slice::from_ref(&plugin)
    );
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut refused = vec![("stray", &stray)];
    #[cfg(windows)]
    refused.push(("far", &far));
    for (package_id, original) in refused {
        let row = find(&listed, &package(id, package_id));
        assert_eq!(row.status, BackupStatus::CanRestore, "{package_id}");
        assert_eq!(row.restore_scope, None, "{package_id}");
        assert_eq!(row.refused_path.as_ref(), Some(original), "{package_id}");
        assert!(listed.restore_targets(&row.item).is_empty(), "{package_id}");
        assert!(
            restore_targets_with(&resource, &row.item, &with_clap)
                .unwrap()
                .is_empty()
        );
    }
    // Without the CLAP folder among the places a restore writes, the
    // plug-in is refused too, and so never already back.
    let app2clap = find(&without, &package(id, "app2clap"));
    assert_eq!(app2clap.status, BackupStatus::CanRestore);
    assert_eq!(app2clap.refused_path.as_ref(), Some(&plugin));

    // A dry run of each restore agrees.
    let dry_run = |package_id: &str| {
        restore_backup_set_within(
            &resource,
            id,
            &RestoreBackupOptions {
                dry_run: true,
                allow_reaper_running: true,
                package_id: Some(package_id.to_string()),
            },
            &rules,
        )
    };
    let report = dry_run("app2clap").unwrap();
    let written: Vec<&Path> = report
        .actions
        .iter()
        .map(|action| action.target_path.as_path())
        .collect();
    assert_eq!(written, [plugin.as_path()]);
    // The row says why in the code the run gives.
    let error = dry_run("stray").unwrap_err();
    assert!(
        matches!(
            refusal(&error),
            RestoreRefusal::TargetRefused {
                reason: TargetRefusal::OutsideAllowedFolders,
                ..
            }
        ),
        "{error:?}"
    );
    let row = find(&listed, &package(id, "stray"));
    assert_eq!(row.refusal.as_ref(), Some(refusal(&error)));
}
