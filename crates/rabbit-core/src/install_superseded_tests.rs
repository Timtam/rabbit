//! Tests of an update moving the files a package's previous version
//! installed, and the new version doesn't, into the update's backup set
//! (backup plan D16), and of getting that earlier version back and
//! deleting it. Temporary folders only: every folder outside a test's
//! resource folder that the install may look at is the test's own.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::{
    InstallFileAction, InstallFileReport, InstallOptions, InstallReport, KEYMAP_WARNING,
    LINK_WARNING, UNTRACKED_NOTE, install_cached_artifacts_within,
};
use crate::artifact::{ArtifactDescriptor, ArtifactKind, CachedArtifact};
use crate::backup_delete::{DeleteItem, DeleteRunOptions, DeleteStatus, delete_backups_with};
use crate::backup_overview::{
    BackupEntry, BackupGroup, BackupItem, BackupStatus, RestoreScope, overview_with,
};
use crate::cancel::CancelToken;
use crate::hash::{sha256_bytes, sha256_file};
use crate::model::{Architecture, Platform};
use crate::package::{PACKAGE_FFMPEG, PACKAGE_REAKONTROL, PACKAGE_REAPACK, PACKAGE_SWS};
use crate::progress::ProgressReporter;
use crate::receipt::{
    InstallState, InstalledFileReceipt, PackageReceipt, load_install_state, save_install_state,
};
use crate::rollback::restore_run_tests::snapshot;
use crate::rollback::{
    BackupEntryState, BackupEnvironment, BackupManifest, ORIGIN_SIDECAR_SUFFIX,
    OutsideRestoreRules, ReplacedPackageRecord, RestoreBackupOptions, RestoreBackupReport,
    RestoreStatus, UPDATE_BACKUP_REASON, load_backup_manifest, restore_backup_set_within,
    save_backup_manifest,
};
use crate::version::Version;

const LANGPACK_ES: &str = "langpack-es";

const APPLY: InstallOptions = InstallOptions {
    dry_run: false,
    allow_reaper_running: true,
    target_app_path: None,
    package_variants: BTreeMap::new(),
};

const DRY_RUN: InstallOptions = InstallOptions {
    dry_run: true,
    allow_reaper_running: true,
    target_app_path: None,
    package_variants: BTreeMap::new(),
};

/// A temporary folder with a resource folder `REAPER` in it, and the
/// resource folder's path.
fn resource_folder() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    fs::create_dir_all(&resource).unwrap();
    (dir, resource)
}

/// Install `artifacts` into `resource`, where no file is outside the
/// resource folder: the install never asks which folders outside it
/// RABBIT installs into, so it never looks at the real CLAP folder.
fn install(
    resource: &Path,
    artifacts: &[CachedArtifact],
    options: &InstallOptions,
) -> InstallReport {
    install_cached_artifacts_within(
        resource,
        artifacts,
        options,
        &ProgressReporter::noop(),
        &|| -> Vec<PathBuf> {
            panic!("only a file outside the resource folder asks for the folders outside it")
        },
    )
    .unwrap()
}

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A downloaded single file of `package_id`'s `at` version, named
/// `file_name`, holding `contents`, in `cache`.
fn artifact(
    cache: &Path,
    package_id: &str,
    at: &str,
    file_name: &str,
    contents: &[u8],
) -> CachedArtifact {
    let path = cache.join(package_id).join(at).join(file_name);
    write(&path, contents);
    CachedArtifact {
        descriptor: ArtifactDescriptor {
            package_id: package_id.to_string(),
            version: version(at),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::ExtensionBinary,
            url: format!("https://example.test/{file_name}"),
            file_name: file_name.to_string(),
            channel: None,
        },
        sha256: sha256_file(&path).unwrap(),
        size: contents.len() as u64,
        path,
        reused_existing_file: false,
    }
}

/// FFmpeg's `at` version as its `.7z` comes, with `dlls` in its `bin`
/// folder beside a file RABBIT doesn't install.
fn ffmpeg(cache: &Path, at: &str, dlls: &[(&str, &[u8])]) -> CachedArtifact {
    let path = cache.join(format!("ffmpeg-{at}.7z"));
    fs::create_dir_all(cache).unwrap();
    let mut entries = vec![(format!("ffmpeg-{at}/LICENSE.txt"), &b"license"[..])];
    entries.extend(
        dlls.iter()
            .map(|(name, contents)| (format!("ffmpeg-{at}/bin/{name}"), *contents)),
    );
    let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
    writer
        .push_archive_entries(
            entries
                .iter()
                .map(|(name, _)| sevenz_rust2::ArchiveEntry::new_file(name))
                .collect(),
            entries
                .iter()
                .map(|(_, contents)| sevenz_rust2::SourceReader::from(*contents))
                .collect(),
        )
        .unwrap();
    writer.finish().unwrap();
    CachedArtifact {
        descriptor: ArtifactDescriptor {
            package_id: PACKAGE_FFMPEG.to_string(),
            version: version(at),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::SevenZipArchive,
            url: format!("https://example.test/ffmpeg-{at}.7z"),
            file_name: format!("ffmpeg-{at}.7z"),
            channel: None,
        },
        sha256: sha256_file(&path).unwrap(),
        size: fs::metadata(&path).unwrap().len(),
        path,
        reused_existing_file: false,
    }
}

/// Files by their path, relative ones inside the resource folder, with
/// what they hold.
type Files<'a> = [(&'a Path, &'a [u8])];

/// A receipt entry for `id` at `at` listing `files`, as an install records
/// them.
fn entry(id: &str, at: &str, files: &Files<'_>) -> PackageReceipt {
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
                path: path.to_path_buf(),
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

/// `resource`'s receipt with `entries`, each file of them written where it
/// records it, holding what it records.
fn installed(resource: &Path, entries: &[(PackageReceipt, &Files<'_>)]) {
    let mut state = InstallState::default();
    for (entry, files) in entries {
        for (path, contents) in *files {
            let place = if path.is_absolute() {
                path.to_path_buf()
            } else {
                resource.join(path)
            };
            write(&place, contents);
        }
        state.packages.insert(entry.id.clone(), entry.clone());
    }
    save_install_state(resource, &state).unwrap();
}

fn plugins(name: &str) -> PathBuf {
    Path::new("UserPlugins").join(name)
}

/// The manifest of the update set `report` names.
fn update_set(report: &InstallReport) -> BackupManifest {
    let path = report.backup_manifest_path.as_ref().expect("an update set");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The id of the update set `report` names.
fn set_id(report: &InstallReport) -> String {
    let path = report.backup_manifest_path.as_ref().expect("an update set");
    path.parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// Each action of `report` by its file's name.
fn actions_by_name(report: &InstallReport) -> BTreeMap<String, InstallFileAction> {
    report
        .actions
        .iter()
        .map(|action| {
            (
                action
                    .target_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                action.action,
            )
        })
        .collect()
}

/// The action of `report` for the file at `place`.
fn action_at<'a>(report: &'a InstallReport, place: &Path) -> &'a InstallFileReport {
    report
        .actions
        .iter()
        .find(|action| action.target_path == place)
        .unwrap_or_else(|| panic!("no action for {}: {:?}", place.display(), report.actions))
}

/// Each finished file in the set `manifest` describes: where it came from
/// and what its copy holds. Every entry belongs to `package_id`.
fn files_held(manifest: &BackupManifest, package_id: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let mut held: Vec<(PathBuf, Vec<u8>)> = manifest
        .files
        .iter()
        .map(|file| {
            assert_eq!(file.package_id.as_deref(), Some(package_id), "{file:?}");
            assert_eq!(file.state, BackupEntryState::Done, "{file:?}");
            (
                file.original_path.clone(),
                fs::read(&file.backup_path).unwrap(),
            )
        })
        .collect();
    held.sort();
    held
}

/// The sets in `RABBIT/backups`, by name; none when there is no such
/// folder.
fn sets_in(resource: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(resource.join("RABBIT").join("backups")) else {
        return Vec::new();
    };
    let mut sets: Vec<String> = entries
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    sets.sort();
    sets
}

/// An FFmpeg update whose libraries have new names moves the old ones
/// into its backup set, beside the copy of the library it replaced, and
/// installs the new ones. The set records FFmpeg's entry from before, and
/// every file in it is FFmpeg's. The library both versions have, the same
/// in both, is left alone, and the set doesn't hold it.
#[test]
fn an_ffmpeg_update_that_renames_its_dlls_moves_the_old_ones_into_its_backup() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let old = ffmpeg(
        &cache,
        "7.1",
        &[
            ("avcodec-61.dll", b"avcodec 7.1"),
            ("avformat-61.dll", b"avformat 7.1"),
            ("avutil-59.dll", b"avutil 7.1"),
            ("swscale-8.dll", b"swscale"),
        ],
    );
    install(&resource, &[old], &APPLY);
    let before = load_install_state(&resource).unwrap().unwrap().packages[PACKAGE_FFMPEG].clone();
    let swscale = resource.join(plugins("swscale-8.dll"));
    let swscale_changed = fs::metadata(&swscale).unwrap().modified().unwrap();
    let new = ffmpeg(
        &cache,
        "8.0",
        &[
            ("avcodec-62.dll", b"avcodec 8.0"),
            ("avformat-62.dll", b"avformat 8.0"),
            ("avutil-59.dll", b"avutil 8.0"),
            ("swscale-8.dll", b"swscale"),
        ],
    );

    let report = install(&resource, &[new], &APPLY);

    assert_eq!(
        actions_by_name(&report),
        BTreeMap::from([
            (
                "avcodec-61.dll".to_string(),
                InstallFileAction::MovedToBackup
            ),
            ("avcodec-62.dll".to_string(), InstallFileAction::Installed),
            (
                "avformat-61.dll".to_string(),
                InstallFileAction::MovedToBackup
            ),
            ("avformat-62.dll".to_string(), InstallFileAction::Installed),
            ("avutil-59.dll".to_string(), InstallFileAction::Replaced),
            ("swscale-8.dll".to_string(), InstallFileAction::Kept),
        ])
    );
    for (name, contents) in [
        ("avcodec-62.dll", &b"avcodec 8.0"[..]),
        ("avformat-62.dll", b"avformat 8.0"),
        ("avutil-59.dll", b"avutil 8.0"),
    ] {
        assert_eq!(fs::read(resource.join(plugins(name))).unwrap(), contents);
    }
    assert!(!resource.join(plugins("avcodec-61.dll")).exists());
    assert!(!resource.join(plugins("avformat-61.dll")).exists());
    assert_eq!(fs::read(&swscale).unwrap(), b"swscale");
    assert_eq!(
        fs::metadata(&swscale).unwrap().modified().unwrap(),
        swscale_changed
    );

    let manifest = update_set(&report);
    assert_eq!(manifest.reason, UPDATE_BACKUP_REASON);
    assert_eq!(
        manifest.replaced,
        BTreeMap::from([(
            PACKAGE_FFMPEG.to_string(),
            ReplacedPackageRecord {
                receipt: Some(before),
                replaced_by: Some(version("8.0")),
            }
        )])
    );
    assert_eq!(
        files_held(&manifest, PACKAGE_FFMPEG),
        [
            (plugins("avcodec-61.dll"), b"avcodec 7.1".to_vec()),
            (plugins("avformat-61.dll"), b"avformat 7.1".to_vec()),
            (plugins("avutil-59.dll"), b"avutil 7.1".to_vec()),
        ]
    );
    // The report says where each moved file went, and what it held.
    let moved = action_at(&report, &resource.join(plugins("avcodec-61.dll")));
    let entry = manifest
        .files
        .iter()
        .find(|file| file.original_path == plugins("avcodec-61.dll"))
        .unwrap();
    assert_eq!(moved.backup_path.as_ref(), Some(&entry.backup_path));
    assert_eq!(moved.size, 11);
    assert_eq!(moved.sha256, sha256_bytes(b"avcodec 7.1"));
    assert_eq!(moved.warning, None);

    let state = load_install_state(&resource).unwrap().unwrap();
    let mut listed: Vec<PathBuf> = state.packages[PACKAGE_FFMPEG]
        .installed_files
        .iter()
        .map(|file| file.path.clone())
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        [
            plugins("avcodec-62.dll"),
            plugins("avformat-62.dll"),
            plugins("avutil-59.dll"),
            plugins("swscale-8.dll"),
        ]
    );
}

/// An update that replaces no file, because its one file has a new name,
/// still makes a set for the file it moves out of the way, and records
/// the package's entry from before in it first, as for a replaced file.
#[test]
fn an_update_that_replaces_nothing_still_records_a_set_for_the_moved_files() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.5",
            "reaper_reapack64.dll",
            b"reapack 1.2.5",
        )],
        &APPLY,
    );
    let before = load_install_state(&resource).unwrap().unwrap().packages[PACKAGE_REAPACK].clone();
    assert!(sets_in(&resource).is_empty());

    let report = install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.6",
            "reaper_reapack-x64.dll",
            b"reapack 1.2.6",
        )],
        &APPLY,
    );

    assert_eq!(
        actions_by_name(&report),
        BTreeMap::from([
            (
                "reaper_reapack-x64.dll".to_string(),
                InstallFileAction::Installed
            ),
            (
                "reaper_reapack64.dll".to_string(),
                InstallFileAction::MovedToBackup
            ),
        ])
    );
    assert_eq!(sets_in(&resource), [set_id(&report)]);
    let manifest = update_set(&report);
    assert_eq!(manifest.reason, UPDATE_BACKUP_REASON);
    assert_eq!(
        manifest.replaced[PACKAGE_REAPACK],
        ReplacedPackageRecord {
            receipt: Some(before),
            replaced_by: Some(version("1.2.6")),
        }
    );
    assert_eq!(
        files_held(&manifest, PACKAGE_REAPACK),
        [(plugins("reaper_reapack64.dll"), b"reapack 1.2.5".to_vec())]
    );
    assert!(!resource.join(plugins("reaper_reapack64.dll")).exists());
    assert_eq!(
        fs::read(resource.join(plugins("reaper_reapack-x64.dll"))).unwrap(),
        b"reapack 1.2.6"
    );
}

/// Only files the package's own previous entry lists move, and of those
/// neither one the new version installs, by its place, nor one another
/// package's entry lists too. A file the user changed since moves all the
/// same: the set is its only copy. Other spellings of a place have tests
/// of their own below.
#[test]
fn only_files_the_new_version_no_longer_has_move() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let plugin = plugins("reaper_reapack-x64.dll");
    let shared = plugins("reapack_shared.dat");
    let old_only = plugins("reapack_old.dat");
    let reapack = entry(
        PACKAGE_REAPACK,
        "1.2.5",
        &[
            (&plugin, b"reapack"),
            (&shared, b"shared"),
            (&old_only, b"old only"),
        ],
    );
    let sws = entry(PACKAGE_SWS, "2.14", &[(&shared, b"shared")]);
    installed(
        &resource,
        &[
            (
                reapack.clone(),
                &[
                    (&plugin, b"reapack"),
                    (&shared, b"shared"),
                    (&old_only, b"changed by the user"),
                ],
            ),
            (sws, &[]),
        ],
    );
    let plugin_changed = fs::metadata(resource.join(&plugin))
        .unwrap()
        .modified()
        .unwrap();

    let report = install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.6",
            "reaper_reapack-x64.dll",
            b"reapack",
        )],
        &APPLY,
    );

    assert_eq!(
        actions_by_name(&report),
        BTreeMap::from([
            (
                "reapack_old.dat".to_string(),
                InstallFileAction::MovedToBackup
            ),
            (
                "reaper_reapack-x64.dll".to_string(),
                InstallFileAction::Kept
            ),
        ])
    );
    assert_eq!(fs::read(resource.join(&plugin)).unwrap(), b"reapack");
    assert_eq!(
        fs::metadata(resource.join(&plugin))
            .unwrap()
            .modified()
            .unwrap(),
        plugin_changed
    );
    assert_eq!(fs::read(resource.join(&shared)).unwrap(), b"shared");
    assert!(!resource.join(&old_only).exists());
    let manifest = update_set(&report);
    assert_eq!(
        files_held(&manifest, PACKAGE_REAPACK),
        [(old_only, b"changed by the user".to_vec())]
    );
    assert_eq!(manifest.replaced[PACKAGE_REAPACK].receipt, Some(reapack));
}

/// On Windows and macOS a name in another case is the same file: a
/// previous entry that spells the new version's file in another case
/// doesn't move the file the update just installed.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn the_new_file_under_another_spelling_stays() {
    let (dir, resource) = resource_folder();
    if !crate::rollback::restore_run_tests::names_in_another_case_are_one(dir.path()) {
        return;
    }
    let cache = dir.path().join("cache");
    let spelled = plugins("REAPER_REAPACK-X64.DLL");
    installed(
        &resource,
        &[(
            entry(PACKAGE_REAPACK, "1.2.5", &[(&spelled, b"reapack 1.2.5")]),
            &[(&spelled, b"reapack 1.2.5")],
        )],
    );

    let report = install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.6",
            "reaper_reapack-x64.dll",
            b"reapack 1.2.6",
        )],
        &APPLY,
    );

    assert_eq!(
        actions_by_name(&report),
        BTreeMap::from([(
            "reaper_reapack-x64.dll".to_string(),
            InstallFileAction::Replaced
        )])
    );
    assert_eq!(
        fs::read(resource.join(plugins("reaper_reapack-x64.dll"))).unwrap(),
        b"reapack 1.2.6"
    );
    assert_eq!(
        files_held(&update_set(&report), PACKAGE_REAPACK).len(),
        1,
        "only the copy of the replaced file"
    );
}

/// A dry run plans the same moves with the planning writer and says
/// where each file would go, and writes nothing at all.
#[test]
fn a_dry_run_plans_the_moves_and_writes_nothing() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.5",
            "reaper_reapack64.dll",
            b"reapack 1.2.5",
        )],
        &APPLY,
    );
    let new = artifact(
        &cache,
        PACKAGE_REAPACK,
        "1.2.6",
        "reaper_reapack-x64.dll",
        b"reapack 1.2.6",
    );
    let before = snapshot(dir.path());

    let report = install(&resource, &[new], &DRY_RUN);

    assert_eq!(snapshot(dir.path()), before);
    assert_eq!(
        actions_by_name(&report),
        BTreeMap::from([
            (
                "reaper_reapack-x64.dll".to_string(),
                InstallFileAction::WouldInstall
            ),
            (
                "reaper_reapack64.dll".to_string(),
                InstallFileAction::WouldMoveToBackup
            ),
        ])
    );
    let planned = action_at(&report, &resource.join(plugins("reaper_reapack64.dll")));
    let manifest_path = report.backup_manifest_path.clone().unwrap();
    let set = manifest_path.parent().unwrap();
    assert!(set.starts_with(resource.join("RABBIT").join("backups")));
    assert_eq!(
        planned.backup_path.as_deref(),
        Some(set.join(plugins("reaper_reapack64.dll")).as_path())
    );
    assert_eq!(planned.size, 13);
    assert_eq!(planned.sha256, sha256_bytes(b"reapack 1.2.5"));
    assert!(!set.exists());
}

/// A file that can't be moved, because another program holds it open on
/// Windows, or its folder may not be changed on Unix, stays where it is
/// with a warning in its action. The install that put the new version in
/// place succeeds, and the set holds only the file that did move.
#[test]
fn a_file_that_cant_be_moved_stays_with_a_warning_and_the_install_succeeds() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let held = plugins("legacy").join("reapack_helper.dll");
    installed(
        &resource,
        &[(
            entry(
                PACKAGE_REAPACK,
                "1.2.5",
                &[
                    (&plugins("reaper_reapack64.dll"), b"reapack 1.2.5"),
                    (&held, b"helper"),
                ],
            ),
            &[
                (&plugins("reaper_reapack64.dll"), b"reapack 1.2.5"),
                (&held, b"helper"),
            ],
        )],
    );
    let held_place = resource.join(&held);
    let Some(_held) = hold(&held_place) else {
        return;
    };

    let report = install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.6",
            "reaper_reapack-x64.dll",
            b"reapack 1.2.6",
        )],
        &APPLY,
    );

    let left = action_at(&report, &held_place);
    assert_eq!(left.action, InstallFileAction::LeftInPlace);
    assert_eq!(left.backup_path, None);
    // RABBIT read it before it tried to move it.
    assert_eq!(left.size, 6);
    assert_eq!(left.sha256, sha256_bytes(b"helper"));
    let warning = left.warning.as_deref().unwrap();
    #[cfg(windows)]
    assert_eq!(
        warning,
        format!("{} {UNTRACKED_NOTE}", super::IN_USE_WARNING)
    );
    #[cfg(unix)]
    assert!(
        warning.starts_with("RABBIT couldn't move it into the backup: ")
            && warning.ends_with(UNTRACKED_NOTE),
        "{warning}"
    );
    assert_eq!(fs::read(&held_place).unwrap(), b"helper");
    // The other file of the old version moved, and the new one is there.
    assert_eq!(
        action_at(&report, &resource.join(plugins("reaper_reapack64.dll"))).action,
        InstallFileAction::MovedToBackup
    );
    assert_eq!(
        fs::read(resource.join(plugins("reaper_reapack-x64.dll"))).unwrap(),
        b"reapack 1.2.6"
    );
    assert!(report.receipt_written);
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(
        state.packages[PACKAGE_REAPACK].version,
        Some(version("1.2.6"))
    );
    assert_eq!(
        files_held(&update_set(&report), PACKAGE_REAPACK),
        [(plugins("reaper_reapack64.dll"), b"reapack 1.2.5".to_vec())]
    );
}

/// A file that can't be moved, and is the only one of its package to go,
/// leaves no set: the record made for it goes again with the set.
#[test]
fn a_set_whose_only_file_cant_be_moved_is_gone_again() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let held = plugins("legacy").join("reapack_helper.dll");
    let plugin = plugins("reaper_reapack-x64.dll");
    installed(
        &resource,
        &[(
            entry(
                PACKAGE_REAPACK,
                "1.2.5",
                &[(&plugin, b"reapack"), (&held, b"helper")],
            ),
            &[(&plugin, b"reapack"), (&held, b"helper")],
        )],
    );
    let held_place = resource.join(&held);
    let Some(_held) = hold(&held_place) else {
        return;
    };

    let report = install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.6",
            "reaper_reapack-x64.dll",
            b"reapack",
        )],
        &APPLY,
    );

    assert_eq!(
        action_at(&report, &held_place).action,
        InstallFileAction::LeftInPlace
    );
    assert_eq!(report.backup_manifest_path, None);
    assert!(sets_in(&resource).is_empty(), "{:?}", sets_in(&resource));
    assert_eq!(fs::read(&held_place).unwrap(), b"helper");
}

/// A file kept the way another program holding it open would keep it:
/// readable, but impossible to move. Letting go of it ends that.
struct Held {
    #[cfg(windows)]
    _file: fs::File,
    #[cfg(unix)]
    folder: PathBuf,
}

/// Hold the file at `place` (see [`Held`]). Windows refuses to move a file
/// another program opened without sharing deletion. Unix lets anyone move
/// an open file, so a folder that can't be written stands in; root may
/// write it all the same, and then there is nothing to hold, `None`.
///
/// Never a file directly in UserPlugins: on macOS a real run first checks
/// that it can write there, so a read-only UserPlugins would stop the run
/// before the move a test is about. Checked on every platform, so a test
/// that breaks the rule fails here too.
fn hold(place: &Path) -> Option<Held> {
    assert_ne!(
        place.parent().and_then(Path::file_name),
        Some(std::ffi::OsStr::new("UserPlugins")),
        "hold a file outside UserPlugins: {}",
        place.display()
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_SHARE_READ: u32 = 1;
        Some(Held {
            _file: fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(place)
                .unwrap(),
        })
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        if fs::metadata(place).unwrap().uid() == 0 {
            return None;
        }
        let folder = place.parent().unwrap().to_path_buf();
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
        Some(Held { folder })
    }
}

#[cfg(unix)]
impl Drop for Held {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(&self.folder, fs::Permissions::from_mode(0o755));
    }
}

/// A file the previous version installed outside the resource folder, in
/// a folder RABBIT installs into, goes under `external/<n>` in the set,
/// beside a note of where it came from. One outside every such folder
/// stays where it is: no restore could put it back. So does a link at a
/// place in that folder, never followed. Only the test's own folders
/// outside the resource folder are looked at, and nothing else outside
/// the resource folder changes.
#[test]
fn an_outside_file_goes_under_external_with_its_origin_note() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let clap = dir.path().join("CLAP");
    let elsewhere = dir.path().join("Elsewhere");
    let helper = clap.join("reapack_helper.clap");
    let linked = clap.join("reapack_linked.clap");
    let stray = elsewhere.join("reapack_stray.clap");
    let plugin = plugins("reaper_reapack-x64.dll");
    installed(
        &resource,
        &[(
            entry(
                PACKAGE_REAPACK,
                "1.2.5",
                &[
                    (&plugin, b"reapack"),
                    (&helper, b"helper"),
                    (&linked, b"linked"),
                    (&stray, b"stray"),
                ],
            ),
            &[
                (&plugin, b"reapack"),
                (&helper, b"helper"),
                (&stray, b"stray"),
            ],
        )],
    );
    crate::uninstall::paths::link_folder(&linked, &elsewhere);
    let new = artifact(
        &cache,
        PACKAGE_REAPACK,
        "1.2.6",
        "reaper_reapack-x64.dll",
        b"reapack",
    );
    let before = snapshot(dir.path());
    let asked = Cell::new(0);

    let report = install_cached_artifacts_within(
        &resource,
        &[new],
        &APPLY,
        &ProgressReporter::noop(),
        &|| {
            asked.set(asked.get() + 1);
            vec![clap.clone()]
        },
    )
    .unwrap();

    assert_eq!(asked.get(), 1, "read once, for the first outside file");
    // Outside the resource folder, only the moved file is gone.
    let after = snapshot(dir.path());
    let outside_changes: Vec<&PathBuf> = before
        .iter()
        .filter(|found| !after.contains(found))
        .chain(after.iter().filter(|found| !before.contains(found)))
        .map(|(path, _)| path)
        .filter(|path| !path.starts_with(&resource))
        .collect();
    assert_eq!(outside_changes, [&helper]);
    assert!(!helper.exists());
    let manifest = update_set(&report);
    let set = report
        .backup_manifest_path
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let copy = set.join("external").join("1").join("reapack_helper.clap");
    assert_eq!(fs::read(&copy).unwrap(), b"helper");
    let note = copy.with_file_name(format!("reapack_helper.clap{ORIGIN_SIDECAR_SUFFIX}"));
    assert_eq!(fs::read_to_string(&note).unwrap(), helper.to_string_lossy());
    assert_eq!(manifest.files.len(), 1);
    assert_eq!(manifest.files[0].original_path, helper);
    assert_eq!(manifest.files[0].backup_path, copy);
    assert_eq!(
        manifest.files[0].package_id.as_deref(),
        Some(PACKAGE_REAPACK)
    );
    assert_eq!(
        action_at(&report, &helper).action,
        InstallFileAction::MovedToBackup
    );

    let left = action_at(&report, &stray);
    assert_eq!(left.action, InstallFileAction::LeftInPlace);
    let warning = left.warning.as_deref().unwrap();
    assert!(
        warning.starts_with(
            "RABBIT doesn't move it, because no restore of the backup could put it back there: "
        ) && warning.ends_with(UNTRACKED_NOTE),
        "{warning}"
    );
    assert!(stray.is_file());
    let link = action_at(&report, &linked);
    assert_eq!(link.action, InstallFileAction::LeftInPlace);
    assert_eq!(
        link.warning.as_deref(),
        Some(format!("{LINK_WARNING} {UNTRACKED_NOTE}").as_str())
    );
}

/// A symbolic link or junction where the previous version had a file is
/// never followed: it stays as it is, and so does what it leads to. That
/// holds for a link to a folder, and on Unix for a link to a file, which
/// Windows makes only with extra rights. So does a folder where it had a
/// file. Each says why, and the install goes on. Nothing changes but
/// RABBIT's records: no file or folder is added or removed anywhere.
#[test]
fn a_link_where_a_recorded_file_was_is_never_followed() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let elsewhere = dir.path().join("Elsewhere");
    write(&elsewhere.join("inside.dll"), b"not RABBIT's");
    let plugin = plugins("reaper_reapack-x64.dll");
    let linked = plugins("reapack_linked.dll");
    let file_link = plugins("reapack_file_link.dll");
    let folder = plugins("reapack_folder.dll");
    let mut recorded: Vec<(&Path, &[u8])> = vec![
        (plugin.as_path(), &b"reapack"[..]),
        (linked.as_path(), &b"linked"[..]),
        (folder.as_path(), &b"folder"[..]),
    ];
    if cfg!(unix) {
        recorded.push((file_link.as_path(), &b"file link"[..]));
    }
    installed(
        &resource,
        &[(
            entry(PACKAGE_REAPACK, "1.2.5", &recorded),
            &[(&plugin, b"reapack")],
        )],
    );
    crate::uninstall::paths::link_folder(&resource.join(&linked), &elsewhere);
    #[cfg(unix)]
    std::os::unix::fs::symlink(elsewhere.join("inside.dll"), resource.join(&file_link)).unwrap();
    write(&resource.join(&folder).join("kept.txt"), b"kept");
    let new = artifact(
        &cache,
        PACKAGE_REAPACK,
        "1.2.6",
        "reaper_reapack-x64.dll",
        b"reapack",
    );
    let before = snapshot(dir.path());

    let report = install(&resource, &[new], &APPLY);

    let link_warning = format!("{LINK_WARNING} {UNTRACKED_NOTE}");
    let mut links = vec![&linked];
    if cfg!(unix) {
        links.push(&file_link);
    }
    for link in links {
        let action = action_at(&report, &resource.join(link));
        assert_eq!(action.action, InstallFileAction::LeftInPlace);
        assert_eq!(action.warning.as_deref(), Some(link_warning.as_str()));
        assert_eq!((action.size, action.sha256.as_str()), (0, ""));
        assert!(
            fs::symlink_metadata(resource.join(link))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    let kept = action_at(&report, &resource.join(&folder));
    assert_eq!(kept.action, InstallFileAction::LeftInPlace);
    assert_eq!(
        kept.warning.as_deref(),
        Some(format!("It is a folder, not a file, so it isn't moved. {UNTRACKED_NOTE}").as_str())
    );
    assert_eq!(report.backup_manifest_path, None);
    assert!(sets_in(&resource).is_empty());
    // Nothing changed but RABBIT's records, either way.
    let receipt = resource.join("RABBIT").join("install-state.json");
    let but_receipt = |found: &[(PathBuf, crate::rollback::restore_run_tests::Node)]| {
        found
            .iter()
            .filter(|(path, _)| *path != receipt)
            .cloned()
            .collect::<Vec<_>>()
    };
    let after = snapshot(dir.path());
    assert_ne!(after, before, "the receipt changed");
    assert_eq!(but_receipt(&after), but_receipt(&before));
}

/// Switching the Spanish language pack from its es_ES variant to es_MX
/// installs the same content under the other name and moves the old name
/// into the update's set, which records the pack's entry from before.
#[test]
fn a_language_pack_variant_switch_moves_the_old_name_into_the_backup() {
    let (dir, resource) = resource_folder();
    let switched = switched_spanish_pack(dir.path(), &resource);

    let es_es = Path::new("LangPack").join("es_ES.ReaperLangPack");
    let es_mx = Path::new("LangPack").join("es_MX.ReaperLangPack");
    assert!(!resource.join(&es_es).exists());
    assert_eq!(fs::read(resource.join(&es_mx)).unwrap(), b"spanish");
    assert_eq!(
        actions_by_name(&switched.report),
        BTreeMap::from([
            (
                "es_ES.ReaperLangPack".to_string(),
                InstallFileAction::MovedToBackup
            ),
            (
                "es_MX.ReaperLangPack".to_string(),
                InstallFileAction::Installed
            ),
        ])
    );
    let manifest = update_set(&switched.report);
    assert_eq!(
        manifest.replaced[LANGPACK_ES].receipt.as_ref(),
        Some(&switched.before.packages[LANGPACK_ES])
    );
    assert_eq!(
        files_held(&manifest, LANGPACK_ES),
        [(es_es, b"spanish".to_vec())]
    );
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages[LANGPACK_ES].variant.as_deref(), Some("pma"));
    assert_eq!(state.packages[LANGPACK_ES].installed_files[0].path, es_mx);
}

/// The Spanish pack's switch from es_ES to es_MX, and RABBIT's records
/// before and after it.
struct SwitchedPack {
    report: InstallReport,
    before: InstallState,
    after: InstallState,
}

/// Install the Spanish language pack into `resource`, then switch it to
/// es_MX, with downloads in `dir`.
fn switched_spanish_pack(dir: &Path, resource: &Path) -> SwitchedPack {
    let cache = dir.join("cache");
    let pack = artifact(
        &cache,
        LANGPACK_ES,
        "1.0",
        "spanish.ReaperLangPack",
        b"spanish",
    );
    install(resource, std::slice::from_ref(&pack), &APPLY);
    let before = load_install_state(resource).unwrap().unwrap();
    let report = install(
        resource,
        &[pack],
        &InstallOptions {
            package_variants: BTreeMap::from([(LANGPACK_ES.to_string(), "pma".to_string())]),
            ..APPLY
        },
    );
    let after = load_install_state(resource).unwrap().unwrap();
    SwitchedPack {
        report,
        before,
        after,
    }
}

/// The Backups overview's row of `package_id` in the set `set_id`.
fn package_row(resource: &Path, set_id: &str, package_id: &str) -> BackupEntry {
    overview_with(resource, &BackupEnvironment::AS_USER)
        .unwrap()
        .entries
        .into_iter()
        .find(|entry| {
            entry.item
                == BackupItem::Package {
                    set_id: set_id.to_string(),
                    package_id: package_id.to_string(),
                }
        })
        .unwrap()
}

/// Restore `package_id` from the set `set_id` in `resource`, as the user.
fn restore(resource: &Path, set_id: &str, package_id: &str) -> RestoreBackupReport {
    restore_backup_set_within(
        resource,
        set_id,
        &RestoreBackupOptions {
            dry_run: false,
            allow_reaper_running: true,
            package_id: Some(package_id.to_string()),
        },
        &OutsideRestoreRules::default(),
    )
    .unwrap()
}

/// After the switch, the Backups overview lists the es_ES pack as an
/// earlier version that can be restored, counting the moved file. Going
/// back to it puts es_ES back and moves es_MX into a new backup, and the
/// pack's entry in RABBIT's records is the one from before the switch.
/// Restoring that new backup goes forward to es_MX again.
#[test]
fn the_moved_files_make_the_earlier_version_whole_and_restorable() {
    let (dir, resource) = resource_folder();
    let switched = switched_spanish_pack(dir.path(), &resource);
    let set_id = set_id(&switched.report);
    let es_es = resource.join("LangPack").join("es_ES.ReaperLangPack");
    let es_mx = resource.join("LangPack").join("es_MX.ReaperLangPack");

    let row = package_row(&resource, &set_id, LANGPACK_ES);
    assert_eq!(row.group, BackupGroup::EarlierVersions);
    assert_eq!(row.status, BackupStatus::CanRestore, "{row:?}");
    assert_eq!(row.restore_scope, Some(RestoreScope::Package));
    assert_eq!(row.size, b"spanish".len() as u64);
    assert_eq!(row.missing_path, None);

    let back = restore(&resource, &set_id, LANGPACK_ES);

    assert_eq!(back.status, RestoreStatus::Restored, "{back:?}");
    assert_eq!(fs::read(&es_es).unwrap(), b"spanish");
    assert!(!es_mx.exists());
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages, switched.before.packages);
    let swap = back.swap.as_ref().unwrap();
    assert_eq!(swap.moved_out.len(), 1);
    assert_eq!(swap.moved_out[0].path, es_mx);

    let newer = back.before_restore_set_id.clone().unwrap();
    let forward = restore(&resource, &newer, LANGPACK_ES);

    assert_eq!(forward.status, RestoreStatus::Restored, "{forward:?}");
    assert_eq!(fs::read(&es_mx).unwrap(), b"spanish");
    assert!(!es_es.exists());
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages, switched.after.packages);
}

/// Deleting one package out of an update's set takes the files the update
/// moved out of the way with it, and leaves the other package's copy and
/// record in the set.
#[test]
fn deleting_a_package_out_of_its_set_takes_its_moved_files() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.5",
            "reaper_reapack64.dll",
            b"reapack 1.2.5",
        )],
        &APPLY,
    );
    write(
        &resource.join(plugins("reaper_kontrol.dll")),
        b"reakontrol by hand",
    );
    let report = install(
        &resource,
        &[
            artifact(
                &cache,
                PACKAGE_REAPACK,
                "1.2.6",
                "reaper_reapack-x64.dll",
                b"reapack 1.2.6",
            ),
            artifact(
                &cache,
                PACKAGE_REAKONTROL,
                "1.2.6",
                "reaper_kontrol.dll",
                b"reakontrol 1.2.6",
            ),
        ],
        &APPLY,
    );
    let set_id = set_id(&report);
    let set = resource.join("RABBIT").join("backups").join(&set_id);
    let moved = action_at(&report, &resource.join(plugins("reaper_reapack64.dll")))
        .backup_path
        .clone()
        .unwrap();
    assert!(moved.is_file());

    let deleted = delete_backups_with(
        &resource,
        &[DeleteItem::package(&set_id, PACKAGE_REAPACK)],
        &DeleteRunOptions { dry_run: false },
        &BackupEnvironment::AS_USER,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap();

    assert_eq!(deleted.sets[0].status, DeleteStatus::DeletedFromSet);
    assert!(!moved.exists());
    let manifest = load_backup_manifest(&set).unwrap().unwrap();
    assert!(!manifest.replaced.contains_key(PACKAGE_REAPACK));
    assert_eq!(
        files_held(&manifest, PACKAGE_REAKONTROL),
        [(
            plugins("reaper_kontrol.dll"),
            b"reakontrol by hand".to_vec()
        )]
    );
}

/// A moved file whose entry in the set stayed pending, as when RABBIT
/// stopped between the move and marking it done, or couldn't save that,
/// still counts: its copy is whole, and a restore puts it back. So the
/// earlier version lists as one that can be restored, and comes back.
#[test]
fn a_moved_file_whose_entry_stayed_pending_still_counts() {
    let (dir, resource) = resource_folder();
    let switched = switched_spanish_pack(dir.path(), &resource);
    let set_id = set_id(&switched.report);
    let set = resource.join("RABBIT").join("backups").join(&set_id);
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    assert_eq!(manifest.files.len(), 1);
    manifest.files[0].state = BackupEntryState::Pending;
    save_backup_manifest(&set, &manifest).unwrap();
    let es_es = resource.join("LangPack").join("es_ES.ReaperLangPack");

    let row = package_row(&resource, &set_id, LANGPACK_ES);
    assert_eq!(row.status, BackupStatus::CanRestore, "{row:?}");
    let back = restore(&resource, &set_id, LANGPACK_ES);

    assert_eq!(back.status, RestoreStatus::Restored, "{back:?}");
    assert_eq!(fs::read(&es_es).unwrap(), b"spanish");
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(state.packages, switched.before.packages);
}

/// A file the previous entry records without a hash, which RABBIT counts
/// as the user's, never moves, and neither does REAPER's key map, by its
/// name or reached through a link folder inside the resource folder: a
/// version's files never include it, and no restore of the version puts
/// it back. Only a receipt RABBIT didn't write lists such files with a
/// package that updates this way. Nothing changes but RABBIT's records.
#[test]
fn files_recorded_without_a_hash_and_the_key_map_never_move() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let plugin = plugins("reaper_reapack-x64.dll");
    let keymap = PathBuf::from("reaper-kb.ini");
    let through_link = Path::new("Linked").join("reaper-kb.ini");
    let settings = plugins("reapack_settings.ini");
    let mut reapack = entry(
        PACKAGE_REAPACK,
        "1.2.5",
        &[
            (&plugin, b"reapack"),
            (&keymap, b"keys"),
            (&through_link, b"keys"),
            (&settings, b"settings"),
        ],
    );
    for file in &mut reapack.installed_files {
        if file.path == settings {
            file.sha256 = None;
            file.size = None;
        }
    }
    installed(
        &resource,
        &[(
            reapack,
            &[
                (&plugin, b"reapack"),
                (&keymap, b"keys"),
                (&settings, b"settings"),
            ],
        )],
    );
    crate::uninstall::paths::link_folder(&resource.join("Linked"), &resource);
    let new = artifact(
        &cache,
        PACKAGE_REAPACK,
        "1.2.6",
        "reaper_reapack-x64.dll",
        b"reapack",
    );
    let before = snapshot(dir.path());

    let report = install(&resource, &[new], &APPLY);

    let keymap_warning = format!("{KEYMAP_WARNING} {UNTRACKED_NOTE}");
    for place in [resource.join(&keymap), resource.join(&through_link)] {
        let left = action_at(&report, &place);
        assert_eq!(left.action, InstallFileAction::LeftInPlace);
        assert_eq!(left.warning.as_deref(), Some(keymap_warning.as_str()));
    }
    assert_eq!(report.actions.len(), 3, "{:?}", report.actions);
    assert_eq!(report.backup_manifest_path, None);
    let receipt = resource.join("RABBIT").join("install-state.json");
    let but_receipt = |found: Vec<(PathBuf, crate::rollback::restore_run_tests::Node)>| {
        found
            .into_iter()
            .filter(|(path, _)| *path != receipt)
            .collect::<Vec<_>>()
    };
    assert_eq!(but_receipt(snapshot(dir.path())), but_receipt(before));
}

/// The new version's file under its short name on Windows, as a previous
/// entry may spell it, is the same file: found by what it is on disk, it
/// stays, though no spelling of its place matches. A volume that makes no
/// short names has nothing to show.
#[cfg(windows)]
#[test]
fn the_new_file_by_its_short_name_stays() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let plugin = plugins("reaper_reapack-x64.dll");
    let short = plugins("REAPER~1.DLL");
    write(&resource.join(&plugin), b"reapack");
    if !resource.join(&short).is_file() {
        return;
    }
    installed(
        &resource,
        &[(
            entry(PACKAGE_REAPACK, "1.2.5", &[(&short, b"reapack")]),
            &[],
        )],
    );

    let report = install(
        &resource,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.6",
            "reaper_reapack-x64.dll",
            b"reapack",
        )],
        &APPLY,
    );

    assert_eq!(
        actions_by_name(&report),
        BTreeMap::from([(
            "reaper_reapack-x64.dll".to_string(),
            InstallFileAction::Kept
        )])
    );
    assert_eq!(fs::read(resource.join(&plugin)).unwrap(), b"reapack");
    assert!(sets_in(&resource).is_empty());
}

/// A resource folder given with `.` or `..` in its path, as a relative
/// `--resource-path` is, still tells places apart by any spelling of them:
/// a file another package's entry lists in another case stays, and so
/// does the new version's own file, which the previous entry spells in
/// another case.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn a_resource_path_with_dots_compares_places_by_any_spelling() {
    let (dir, resource) = resource_folder();
    if !crate::rollback::restore_run_tests::names_in_another_case_are_one(dir.path()) {
        return;
    }
    let cache = dir.path().join("cache");
    fs::create_dir_all(&cache).unwrap();
    let dotted = cache.join("..").join("REAPER");
    let plugin = plugins("reaper_reapack-x64.dll");
    let shared = plugins("reapack_shared.dat");
    installed(
        &resource,
        &[
            (
                entry(
                    PACKAGE_REAPACK,
                    "1.2.5",
                    &[
                        (&plugins("REAPER_REAPACK-X64.DLL"), b"reapack"),
                        (&shared, b"shared"),
                    ],
                ),
                &[(&plugin, b"reapack"), (&shared, b"shared")],
            ),
            (
                entry(
                    PACKAGE_SWS,
                    "2.14",
                    &[(&plugins("REAPACK_SHARED.DAT"), b"shared")],
                ),
                &[],
            ),
        ],
    );

    let report = install(
        &dotted,
        &[artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.6",
            "reaper_reapack-x64.dll",
            b"reapack",
        )],
        &APPLY,
    );

    assert_eq!(
        actions_by_name(&report),
        BTreeMap::from([(
            "reaper_reapack-x64.dll".to_string(),
            InstallFileAction::Kept
        )])
    );
    assert_eq!(fs::read(resource.join(&plugin)).unwrap(), b"reapack");
    assert_eq!(fs::read(resource.join(&shared)).unwrap(), b"shared");
    assert!(sets_in(&resource).is_empty());
}

/// In one install of several packages, a dry run plans what the real run
/// then does, in either order. A file ReaPack's previous version had and
/// ReaKontrol's new version installs stays for ReaKontrol, which replaces
/// it, when ReaKontrol comes first. When ReaPack comes first, it moves the
/// file into the set, and ReaKontrol installs it new.
#[test]
fn a_dry_run_of_several_packages_plans_what_the_real_run_does() {
    for reakontrol_first in [true, false] {
        let (dir, resource) = resource_folder();
        let cache = dir.path().join("cache");
        let plugin = plugins("reaper_reapack-x64.dll");
        let helper = plugins("shared_helper.dll");
        installed(
            &resource,
            &[(
                entry(
                    PACKAGE_REAPACK,
                    "1.2.5",
                    &[(&plugin, b"reapack"), (&helper, b"helper 1")],
                ),
                &[(&plugin, b"reapack"), (&helper, b"helper 1")],
            )],
        );
        let reapack = artifact(
            &cache,
            PACKAGE_REAPACK,
            "1.2.6",
            "reaper_reapack-x64.dll",
            b"reapack",
        );
        let reakontrol = artifact(
            &cache,
            PACKAGE_REAKONTROL,
            "1.0",
            "shared_helper.dll",
            b"helper 2",
        );
        let artifacts = if reakontrol_first {
            [reakontrol, reapack]
        } else {
            [reapack, reakontrol]
        };
        let before = snapshot(dir.path());

        let planned = install(&resource, &artifacts, &DRY_RUN);
        assert_eq!(snapshot(dir.path()), before);
        let done = install(&resource, &artifacts, &APPLY);

        let mut expected = vec![(
            PACKAGE_REAPACK,
            "reaper_reapack-x64.dll",
            InstallFileAction::Kept,
        )];
        if reakontrol_first {
            expected.insert(
                0,
                (
                    PACKAGE_REAKONTROL,
                    "shared_helper.dll",
                    InstallFileAction::Replaced,
                ),
            );
        } else {
            expected.extend([
                (
                    PACKAGE_REAPACK,
                    "shared_helper.dll",
                    InstallFileAction::MovedToBackup,
                ),
                (
                    PACKAGE_REAKONTROL,
                    "shared_helper.dll",
                    InstallFileAction::Installed,
                ),
            ]);
        }
        let first = if reakontrol_first {
            "ReaKontrol"
        } else {
            "ReaPack"
        };
        assert_eq!(actions_as_done(&done), expected, "{first} first");
        assert_eq!(actions_as_done(&planned), expected, "{first} first");
        assert_eq!(fs::read(resource.join(&helper)).unwrap(), b"helper 2");
        let kept_by = if reakontrol_first {
            PACKAGE_REAKONTROL
        } else {
            PACKAGE_REAPACK
        };
        assert_eq!(
            files_held(&update_set(&done), kept_by),
            [(helper.clone(), b"helper 1".to_vec())]
        );
    }
}

/// Each action of `report` with its package and its file's name, a dry
/// run's as the real run's action that it plans.
fn actions_as_done(report: &InstallReport) -> Vec<(&str, &str, InstallFileAction)> {
    report
        .actions
        .iter()
        .map(|action| {
            let done = match action.action {
                InstallFileAction::WouldInstall => InstallFileAction::Installed,
                InstallFileAction::WouldReplace => InstallFileAction::Replaced,
                InstallFileAction::WouldKeep => InstallFileAction::Kept,
                InstallFileAction::WouldMoveToBackup => InstallFileAction::MovedToBackup,
                other => other,
            };
            (
                action.package_id.as_str(),
                action.target_path.file_name().unwrap().to_str().unwrap(),
                done,
            )
        })
        .collect()
}

/// An install's report keeps the shape it had: an action with no warning
/// has no `warning` key at all, a file left where it was says why, and a
/// report saved before warnings existed still reads.
#[test]
fn an_install_report_keeps_its_json_shape() {
    let installed = InstallFileReport {
        package_id: PACKAGE_REAPACK.to_string(),
        source_path: PathBuf::from("new.dll"),
        target_path: PathBuf::from("old.dll"),
        backup_path: None,
        action: InstallFileAction::Installed,
        size: 3,
        sha256: "abc".to_string(),
        warning: None,
    };
    let saved = serde_json::json!({
        "package_id": PACKAGE_REAPACK,
        "source_path": "new.dll",
        "target_path": "old.dll",
        "backup_path": null,
        "action": "installed",
        "size": 3,
        "sha256": "abc",
    });
    assert_eq!(serde_json::to_value(&installed).unwrap(), saved);
    assert_eq!(
        serde_json::from_value::<InstallFileReport>(saved).unwrap(),
        installed
    );

    let left = InstallFileReport {
        action: InstallFileAction::LeftInPlace,
        warning: Some("It is a folder.".to_string()),
        ..installed
    };
    let json = serde_json::to_value(&left).unwrap();
    assert_eq!(json["action"], "left-in-place");
    assert_eq!(json["warning"], "It is a folder.");
    for (action, text) in [
        (InstallFileAction::WouldMoveToBackup, "would-move-to-backup"),
        (InstallFileAction::MovedToBackup, "moved-to-backup"),
    ] {
        assert_eq!(serde_json::to_value(action).unwrap(), text);
    }
}
