//! Tests of installing a plug-in bundle as a whole tree: LJuno-116's VST3
//! bundle into REAPER's own plug-in folder, an update of it that replaces
//! and drops files, going back to the version before, and the places a
//! bundle refuses to go. Temporary folders only.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use tempfile::tempdir;

use super::{InstallFileAction, InstallOptions, InstallReport, install_cached_artifacts_within};
use crate::archive::write_test_zip;
use crate::artifact::{ArtifactDescriptor, ArtifactKind, CachedArtifact};
use crate::backup_overview::{BackupGroup, BackupItem, BackupStatus, overview_with};
use crate::error::{RabbitError, Result};
use crate::hash::sha256_file;
use crate::model::{Architecture, Platform};
use crate::package::{PACKAGE_LB_303, PACKAGE_LJUNO_116};
use crate::progress::ProgressReporter;
use crate::receipt::load_install_state;
use crate::rollback::restore_run_tests::snapshot;
use crate::rollback::{
    BackupEntryState, BackupEnvironment, BackupManifest, OutsideRestoreRules, RestoreBackupOptions,
    RestoreStatus, restore_backup_set_within,
};
use crate::version::Version;

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

/// The binary inside LJuno-116's bundle.
const BINARY: &str = "Contents/x86_64-win/LJuno-116.vst3";

/// LJuno-116's release `at` as its zip comes, with `files` in its bundle
/// folder, each by its path there with `/`, beside a file outside the
/// bundle that RABBIT ignores, in `cache`.
pub(crate) fn ljuno(cache: &Path, at: &str, files: &[(&str, &[u8])]) -> CachedArtifact {
    bundle_release(cache, PACKAGE_LJUNO_116, "LJuno-116", at, files)
}

/// `package_id`'s release `at` as Lo-lo's zips come, with `files` in the
/// bundle folder `<name>.vst3`, each by its path there with `/`, beside a
/// file outside the bundle that RABBIT ignores, in `cache`.
fn bundle_release(
    cache: &Path,
    package_id: &str,
    name: &str,
    at: &str,
    files: &[(&str, &[u8])],
) -> CachedArtifact {
    let file_name = format!("{name}.{at}.vst3.zip");
    let path = cache.join(&file_name);
    fs::create_dir_all(cache).unwrap();
    let names: Vec<String> = files
        .iter()
        .map(|(relative, _)| format!("{name}.vst3/{relative}"))
        .collect();
    let mut entries: Vec<(&str, &[u8])> = vec![("README.md", b"read me")];
    entries.extend(
        names
            .iter()
            .zip(files)
            .map(|(name, (_, contents))| (name.as_str(), *contents)),
    );
    write_test_zip(&path, &entries);
    CachedArtifact {
        descriptor: ArtifactDescriptor {
            package_id: package_id.to_string(),
            version: Version::parse(at).unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Archive,
            url: format!("https://example.test/{file_name}"),
            file_name,
            channel: None,
        },
        sha256: sha256_file(&path).unwrap(),
        size: fs::metadata(&path).unwrap().len(),
        path,
        reused_existing_file: false,
    }
}

/// Install `artifacts` into `resource`, where every file is inside the
/// resource folder and no REAPER runs.
pub(crate) fn try_install(
    resource: &Path,
    artifacts: &[CachedArtifact],
    options: &InstallOptions,
) -> Result<InstallReport> {
    install_cached_artifacts_within(
        resource,
        artifacts,
        options,
        &ProgressReporter::noop(),
        &|| -> Vec<PathBuf> {
            panic!("only a file outside the resource folder asks for the folders outside it")
        },
        &Vec::new,
        &|_| None,
    )
}

fn install(
    resource: &Path,
    artifacts: &[CachedArtifact],
    options: &InstallOptions,
) -> InstallReport {
    try_install(resource, artifacts, options).unwrap()
}

/// A temporary folder with a resource folder `REAPER` in it.
fn resource_folder() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    fs::create_dir_all(&resource).unwrap();
    (dir, resource)
}

fn fx(resource: &Path) -> PathBuf {
    resource.join("UserPlugins").join("FX")
}

fn bundle(resource: &Path) -> PathBuf {
    fx(resource).join("LJuno-116.vst3")
}

/// `relative`, with `/` between its names, below `root`.
fn below(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, name| path.join(name))
}

/// Where a receipt records a file of the bundle: below the resource
/// folder, one name per component.
fn recorded(relative: &str) -> PathBuf {
    below(
        &Path::new("UserPlugins").join("FX").join("LJuno-116.vst3"),
        relative,
    )
}

/// A file the next version adds, in a folder of its own.
const HELP: &str = "Contents/Resources/Help/Help.txt";

/// The first version: the binary, first in the zip, so the order the
/// install puts files in is its own; a file at the bundle's top; and one
/// in a folder that the next version drops.
const V1: &[(&str, &[u8])] = &[
    (BINARY, b"binary 0.99.66"),
    ("desktop.ini", b"[.ShellClassInfo]"),
    ("Plugin.ico", b"icon"),
    ("Contents/Resources/moduleinfo.json", b"{\"version\": 1}"),
    ("Contents/Resources/Presets/Old.preset", b"old preset"),
];

/// The next version: a new binary, first in the zip again, the same top
/// files, a changed module info, a new file in a new folder, and no preset
/// folder.
const V2: &[(&str, &[u8])] = &[
    (BINARY, b"binary 0.99.67"),
    ("desktop.ini", b"[.ShellClassInfo]"),
    ("Plugin.ico", b"icon"),
    ("Contents/Resources/moduleinfo.json", b"{\"version\": 2}"),
    (HELP, b"help"),
];

/// Note the place of every file this thread's installs put in place, in
/// order, until the guard it returns with them is dropped.
fn note_places() -> (super::install_seam::Installed, Rc<RefCell<Vec<PathBuf>>>) {
    let places = Rc::new(RefCell::new(Vec::new()));
    let noted = Rc::clone(&places);
    let seam = super::install_seam::install(move |place| {
        noted.borrow_mut().push(place.to_path_buf());
        Ok(())
    });
    (seam, places)
}

/// The update sets in `resource`'s backups folder.
fn sets_in(resource: &Path) -> usize {
    fs::read_dir(resource.join("RABBIT").join("backups")).map_or(0, Iterator::count)
}

/// The bundle as `files` lay it out, folders included, as [`snapshot`]
/// lists it.
fn bundle_tree(
    resource: &Path,
    files: &[(&str, &[u8])],
) -> Vec<(PathBuf, crate::rollback::restore_run_tests::Node)> {
    use crate::rollback::restore_run_tests::Node;

    let root = bundle(resource);
    let mut tree = BTreeMap::new();
    for (relative, contents) in files {
        let path = below(&root, relative);
        let mut folder = path.parent();
        while let Some(parent) = folder.filter(|parent| *parent != root) {
            tree.insert(parent.to_path_buf(), Node::Folder);
            folder = parent.parent();
        }
        tree.insert(path, Node::File(contents.to_vec()));
    }
    tree.into_iter().collect()
}

/// The manifest of the update set `report` names.
fn update_set(report: &InstallReport) -> BackupManifest {
    let path = report.backup_manifest_path.as_ref().expect("an update set");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn set_id(report: &InstallReport) -> String {
    let path = report.backup_manifest_path.as_ref().expect("an update set");
    path.parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// A first install writes every file of the bundle at its path inside the
/// bundle's folder in UserPlugins\FX, the binary last, and nothing else:
/// the file beside the bundle in the zip stays out. The receipt lists each
/// file by its path below the resource folder, with its hash and size.
#[test]
fn a_bundle_installs_every_file_and_records_each_by_its_relative_path() {
    let (dir, resource) = resource_folder();
    let release = ljuno(&dir.path().join("cache"), "0.99.66", V1);
    let (seam, placed) = note_places();

    let report = install(&resource, &[release], &APPLY);

    drop(seam);
    let binary = below(&bundle(&resource), BINARY);
    assert_eq!(placed.borrow().len(), V1.len());
    assert_eq!(
        placed.borrow().last(),
        Some(&binary),
        "the binary goes in last, though it is first in the zip"
    );
    assert_eq!(snapshot(&bundle(&resource)), bundle_tree(&resource, V1));
    let in_fx: Vec<PathBuf> = fs::read_dir(fx(&resource))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(in_fx, [bundle(&resource)]);
    assert!(!resource.join("README.md").exists());

    let installed: Vec<(PathBuf, InstallFileAction)> = report
        .actions
        .iter()
        .map(|action| (action.target_path.clone(), action.action))
        .collect();
    assert_eq!(installed.len(), V1.len());
    assert!(
        installed
            .iter()
            .all(|(_, action)| *action == InstallFileAction::Installed)
    );
    assert_eq!(installed.last().unwrap().0, binary);
    assert_eq!(report.backup_manifest_path, None);

    let state = load_install_state(&resource).unwrap().unwrap();
    let entry = &state.packages[PACKAGE_LJUNO_116];
    assert_eq!(entry.version, Some(Version::parse("0.99.66").unwrap()));
    let mut expected: Vec<PathBuf> = V1.iter().map(|(relative, _)| recorded(relative)).collect();
    expected.sort();
    let paths: Vec<PathBuf> = entry
        .installed_files
        .iter()
        .map(|file| file.path.clone())
        .collect();
    assert_eq!(paths, expected);
    for file in &entry.installed_files {
        assert!(file.path.is_relative());
        let (_, contents) = V1
            .iter()
            .find(|(relative, _)| recorded(relative) == file.path)
            .unwrap();
        assert_eq!(file.size, Some(contents.len() as u64));
        assert_eq!(
            file.sha256.as_deref(),
            Some(crate::hash::sha256_bytes(contents).as_str())
        );
    }

    // Detection finds the bundle by the receipt, file for file.
    let spec = crate::package::package_specs_by_id(Platform::Windows)
        .remove(PACKAGE_LJUNO_116)
        .unwrap();
    let detected = crate::detection::detect_component_seeing(
        &resource,
        Platform::Windows,
        &spec,
        Some(&state),
        |_| None,
        &|_| None,
    )
    .unwrap();
    assert!(detected.installed);
    assert_eq!(detected.detector, "rabbit-receipt");
    assert_eq!(detected.version, Some(Version::parse("0.99.66").unwrap()));
    assert_eq!(detected.files.len(), V1.len());

    // The same version again changes nothing.
    let again = install(
        &resource,
        &[ljuno(&dir.path().join("cache2"), "0.99.66", V1)],
        &APPLY,
    );
    assert!(
        again
            .actions
            .iter()
            .all(|action| action.action == InstallFileAction::Kept)
    );
    assert_eq!(again.backup_manifest_path, None);
    assert_eq!(snapshot(&bundle(&resource)), bundle_tree(&resource, V1));
}

/// A dry run plans every file of the bundle where the real run puts it,
/// the binary last, and writes nothing at all.
#[test]
fn a_dry_run_of_a_bundle_plans_every_file_and_writes_nothing() {
    let (dir, resource) = resource_folder();
    let release = ljuno(&dir.path().join("cache"), "0.99.66", V1);
    let before = snapshot(&resource);

    let report = install(&resource, &[release], &DRY_RUN);

    assert_eq!(snapshot(&resource), before);
    let planned: Vec<PathBuf> = report
        .actions
        .iter()
        .map(|action| {
            assert_eq!(action.action, InstallFileAction::WouldInstall);
            action.target_path.clone()
        })
        .collect();
    assert_eq!(planned.len(), V1.len());
    assert_eq!(planned.last(), Some(&below(&bundle(&resource), BINARY)));
}

/// An update copies each file it replaces into its backup set before the
/// new one takes its place, and moves the file the new version no longer
/// has into the same set, whose folder, now empty, goes. The bundle is then
/// exactly the new version, and the set, with the files it kept, is the
/// earlier version whole: the Backups tab lists it with their size.
#[test]
fn an_update_backs_up_what_it_replaces_and_moves_what_it_drops() {
    let (dir, resource) = resource_folder();
    install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.66", V1)],
        &APPLY,
    );
    let before = load_install_state(&resource).unwrap().unwrap();
    let release = ljuno(&dir.path().join("cache"), "0.99.67", V2);

    let report = install(&resource, std::slice::from_ref(&release), &APPLY);

    assert_eq!(snapshot(&bundle(&resource)), bundle_tree(&resource, V2));
    // The package's item names the binary, which the update replaced,
    // rather than whichever file of the bundle comes first.
    let named = crate::operation::package_install_action(&report, &release.descriptor).unwrap();
    assert_eq!(named.target_path, below(&bundle(&resource), BINARY));
    assert_eq!(named.action, InstallFileAction::Replaced);
    let action = |relative: &str| {
        report
            .actions
            .iter()
            .find(|action| action.target_path == below(&bundle(&resource), relative))
            .unwrap_or_else(|| panic!("no action for {relative}"))
            .action
    };
    assert_eq!(action("desktop.ini"), InstallFileAction::Kept);
    assert_eq!(
        action("Contents/Resources/moduleinfo.json"),
        InstallFileAction::Replaced
    );
    assert_eq!(action(HELP), InstallFileAction::Installed);
    assert_eq!(action(BINARY), InstallFileAction::Replaced);
    assert_eq!(
        action("Contents/Resources/Presets/Old.preset"),
        InstallFileAction::MovedToBackup
    );
    assert!(!below(&bundle(&resource), "Contents/Resources/Presets").exists());

    let manifest = update_set(&report);
    let mut held: Vec<(PathBuf, Vec<u8>)> = manifest
        .files
        .iter()
        .map(|file| {
            assert_eq!(file.package_id.as_deref(), Some(PACKAGE_LJUNO_116));
            assert_eq!(file.state, BackupEntryState::Done);
            (
                file.original_path.clone(),
                fs::read(&file.backup_path).unwrap(),
            )
        })
        .collect();
    held.sort();
    let mut expected: Vec<(PathBuf, Vec<u8>)> = [
        "Contents/Resources/moduleinfo.json",
        "Contents/Resources/Presets/Old.preset",
        BINARY,
    ]
    .iter()
    .map(|relative| {
        let (_, contents) = V1.iter().find(|(name, _)| name == relative).unwrap();
        (recorded(relative), contents.to_vec())
    })
    .collect();
    expected.sort();
    assert_eq!(held, expected);
    assert_eq!(
        manifest.replaced[PACKAGE_LJUNO_116].receipt.as_ref(),
        before.packages.get(PACKAGE_LJUNO_116)
    );

    let row = overview_with(&resource, &BackupEnvironment::AS_USER)
        .unwrap()
        .entries
        .into_iter()
        .find(|entry| {
            entry.item
                == BackupItem::Package {
                    set_id: set_id(&report),
                    package_id: PACKAGE_LJUNO_116.to_string(),
                }
        })
        .unwrap();
    assert_eq!(row.group, BackupGroup::EarlierVersions);
    assert_eq!(row.status, BackupStatus::CanRestore, "{row:?}");
    assert_eq!(
        row.size,
        expected
            .iter()
            .map(|(_, contents)| contents.len() as u64)
            .sum::<u64>()
    );
    assert_eq!(row.version, Some(Version::parse("0.99.66").unwrap()));
    assert_eq!(row.replaced_by, Some(Version::parse("0.99.67").unwrap()));
}

/// Going back to the earlier version from the update's set brings the old
/// bundle back exactly, folders and all, with its receipt entry; the newer
/// version's own file moves into a new set, and its folder, empty then,
/// goes. Restoring that set goes forward to the newer version, exactly.
#[test]
fn going_back_brings_the_earlier_bundle_back_exactly() {
    let (dir, resource) = resource_folder();
    install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.66", V1)],
        &APPLY,
    );
    let earlier = snapshot(&bundle(&resource));
    let earlier_state = load_install_state(&resource).unwrap().unwrap();
    let update = install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.67", V2)],
        &APPLY,
    );
    let newer = snapshot(&bundle(&resource));
    let newer_state = load_install_state(&resource).unwrap().unwrap();
    let restore = |set_id: &str| {
        restore_backup_set_within(
            &resource,
            set_id,
            &RestoreBackupOptions {
                dry_run: false,
                allow_reaper_running: false,
                package_id: Some(PACKAGE_LJUNO_116.to_string()),
            },
            &OutsideRestoreRules::default(),
        )
        .unwrap()
    };

    let back = restore(&set_id(&update));

    assert_eq!(back.status, RestoreStatus::Restored, "{back:?}");
    assert_eq!(snapshot(&bundle(&resource)), earlier);
    assert_eq!(
        load_install_state(&resource).unwrap().unwrap().packages,
        earlier_state.packages
    );
    let swap = back.swap.as_ref().unwrap();
    let moved_out: Vec<PathBuf> = swap
        .moved_out
        .iter()
        .map(|file| file.path.clone())
        .collect();
    assert_eq!(moved_out, [below(&bundle(&resource), HELP)]);
    // The earlier binary comes back after every other file of the set.
    assert_eq!(
        back.actions.last().map(|action| action.target_path.clone()),
        Some(below(&bundle(&resource), BINARY))
    );
    assert_eq!(back.actions.len(), 3, "{:?}", back.actions);

    let forward = restore(&back.before_restore_set_id.clone().unwrap());

    assert_eq!(forward.status, RestoreStatus::Restored, "{forward:?}");
    assert_eq!(snapshot(&bundle(&resource)), newer);
    assert_eq!(
        load_install_state(&resource).unwrap().unwrap().packages,
        newer_state.packages
    );
}

/// Should staging fail partway, here because a folder stands where the
/// binary's checked copy would wait, the update changes nothing: the
/// copies staged before it go again, and so does the folder staging made
/// for the new file; the receipt stays as it was, and no update set is
/// written.
#[test]
fn an_update_whose_staging_fails_partway_changes_nothing() {
    let (dir, resource) = resource_folder();
    install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.66", V1)],
        &APPLY,
    );
    let blocker = below(&bundle(&resource), &format!("{BINARY}.rabbit-tmp"));
    fs::create_dir_all(&blocker).unwrap();
    let before = snapshot(&resource);

    let error = try_install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.67", V2)],
        &APPLY,
    )
    .unwrap_err();

    assert!(matches!(error, RabbitError::Io { .. }), "{error}");
    assert_eq!(snapshot(&resource), before);
    assert!(!below(&bundle(&resource), "Contents/Resources/Help").exists());
}

/// Should a file of an update fail to go into its place, as the binary
/// would while REAPER has it loaded, the files that went in before it go
/// back as they were, the new one goes again with its folder, and the
/// update's set, which held every file it replaces before the first one
/// went in, goes too: the bundle is the earlier version, file for file,
/// and the receipt still says so.
#[test]
fn an_update_that_fails_to_put_its_binary_in_place_puts_the_earlier_bundle_back() {
    let (dir, resource) = resource_folder();
    install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.66", V1)],
        &APPLY,
    );
    let earlier = snapshot(&bundle(&resource));
    let earlier_state = load_install_state(&resource).unwrap().unwrap();
    let binary = below(&bundle(&resource), BINARY);
    let failing = binary.clone();
    let backups = resource.join("RABBIT").join("backups");
    // What the set holds when the first file goes in: (where each copy
    // came from, whether it is done).
    let held_first = Rc::new(RefCell::new(None));
    let held = Rc::clone(&held_first);
    let placed = Rc::new(RefCell::new(Vec::new()));
    let noted = Rc::clone(&placed);
    let seam = super::install_seam::install(move |place| {
        if held.borrow().is_none() {
            let set = fs::read_dir(&backups).unwrap().next().unwrap().unwrap();
            let manifest: BackupManifest = serde_json::from_str(
                &fs::read_to_string(set.path().join(crate::rollback::BACKUP_MANIFEST_FILE))
                    .unwrap(),
            )
            .unwrap();
            let mut files: Vec<(PathBuf, bool)> = manifest
                .files
                .iter()
                .map(|file| {
                    (
                        file.original_path.clone(),
                        file.state == BackupEntryState::Done,
                    )
                })
                .collect();
            files.sort();
            *held.borrow_mut() = Some(files);
        }
        noted.borrow_mut().push(place.to_path_buf());
        if place == failing {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "REAPER has it loaded",
            ));
        }
        Ok(())
    });

    let error = try_install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.67", V2)],
        &APPLY,
    )
    .unwrap_err();

    drop(seam);
    assert!(matches!(error, RabbitError::Io { .. }), "{error}");
    assert_eq!(
        *held_first.borrow(),
        Some(vec![
            (recorded("Contents/Resources/moduleinfo.json"), true),
            (recorded(BINARY), true),
        ]),
        "every file it replaces is in the set before the first one goes in"
    );
    assert_eq!(
        *placed.borrow(),
        [
            below(&bundle(&resource), HELP),
            below(&bundle(&resource), "Contents/Resources/moduleinfo.json"),
            binary,
        ]
    );
    assert_eq!(snapshot(&bundle(&resource)), earlier);
    assert_eq!(
        load_install_state(&resource).unwrap().unwrap(),
        earlier_state
    );
    assert_eq!(sets_in(&resource), 0);
}

/// Should a later package of one install call fail, as LB-303's release
/// without its binary does, the packages before it stay in place, and the
/// receipt is saved with their entries before the error ends the run, so
/// RABBIT keeps track of them: LJuno-116's entry lists every file of its
/// bundle, which its uninstall needs. Nothing of LB-303 is written, and it
/// has no entry.
#[test]
fn a_later_package_that_fails_leaves_the_earlier_ones_recorded() {
    let (dir, resource) = resource_folder();
    let cache = dir.path().join("cache");
    let without_binary = bundle_release(
        &cache,
        PACKAGE_LB_303,
        "LB-303",
        "1.0.0",
        &[("desktop.ini", b"[.ShellClassInfo]")],
    );

    let error = try_install(
        &resource,
        &[ljuno(&cache, "0.99.66", V1), without_binary],
        &APPLY,
    )
    .unwrap_err();

    assert!(
        matches!(error, RabbitError::ArchiveMissingBundleBinary { .. }),
        "{error}"
    );
    assert_eq!(snapshot(&bundle(&resource)), bundle_tree(&resource, V1));
    assert!(!fx(&resource).join("LB-303.vst3").exists());
    let state = load_install_state(&resource)
        .unwrap()
        .expect("the receipt is saved");
    let entry = &state.packages[PACKAGE_LJUNO_116];
    assert_eq!(entry.version, Some(Version::parse("0.99.66").unwrap()));
    let mut expected: Vec<PathBuf> = V1.iter().map(|(relative, _)| recorded(relative)).collect();
    expected.sort();
    let paths: Vec<PathBuf> = entry
        .installed_files
        .iter()
        .map(|file| file.path.clone())
        .collect();
    assert_eq!(paths, expected);
    assert!(!state.packages.contains_key(PACKAGE_LB_303));
}

/// Should a file the update replaces fail to be copied into the update's
/// set, as one that can't be read would, nothing changes: no file went
/// into its place yet, so the staged copies go again, with the folder
/// staging made, and so do the copies already in the set, and the set with
/// them. The bundle is the earlier version, file for file, and the receipt
/// still says so.
#[test]
fn an_update_whose_copy_into_its_set_fails_changes_nothing() {
    let (dir, resource) = resource_folder();
    install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.66", V1)],
        &APPLY,
    );
    let earlier = snapshot(&bundle(&resource));
    let earlier_state = load_install_state(&resource).unwrap().unwrap();
    let binary = below(&bundle(&resource), BINARY);
    let copied = Rc::new(RefCell::new(Vec::new()));
    let noted = Rc::clone(&copied);
    let backing_up = super::install_seam::backing_up(move |place| {
        noted.borrow_mut().push(place.to_path_buf());
        if place == binary {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "it can't be read",
            ));
        }
        Ok(())
    });
    let (placing, placed) = note_places();

    let error = try_install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.67", V2)],
        &APPLY,
    )
    .unwrap_err();

    drop(backing_up);
    drop(placing);
    assert!(matches!(error, RabbitError::Io { .. }), "{error}");
    assert_eq!(
        *copied.borrow(),
        [
            below(&bundle(&resource), "Contents/Resources/moduleinfo.json"),
            below(&bundle(&resource), BINARY),
        ],
        "the binary's copy fails after another file's went into the set"
    );
    assert!(placed.borrow().is_empty());
    assert_eq!(snapshot(&bundle(&resource)), earlier);
    assert_eq!(
        load_install_state(&resource).unwrap().unwrap(),
        earlier_state
    );
    assert_eq!(sets_in(&resource), 0);
}

/// Update LJuno-116 from 0.99.66 to 0.99.67 in `resource`, with `fail`
/// called on the binary's place right before it would go in there, as the
/// binary fails to; the other files go in before it. Returns the error,
/// and the bundle and receipt from before the update.
fn update_failing_at_the_binary(
    dir: &Path,
    resource: &Path,
    fail: impl Fn(&Path) + 'static,
) -> (
    RabbitError,
    Vec<(PathBuf, crate::rollback::restore_run_tests::Node)>,
    crate::receipt::InstallState,
) {
    install(
        resource,
        &[ljuno(&dir.join("cache"), "0.99.66", V1)],
        &APPLY,
    );
    let earlier = snapshot(&bundle(resource));
    let earlier_state = load_install_state(resource).unwrap().unwrap();
    let binary = below(&bundle(resource), BINARY);
    let seam = super::install_seam::install(move |place| {
        if place != binary {
            return Ok(());
        }
        fail(place);
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the binary can't take its place",
        ))
    });
    let error = try_install(
        resource,
        &[ljuno(&dir.join("cache"), "0.99.67", V2)],
        &APPLY,
    )
    .unwrap_err();
    drop(seam);
    (error, earlier, earlier_state)
}

/// Should the file at the binary's place be removed before its new copy
/// fails to take the place, as when Windows renames over it only after
/// removing it first, the earlier binary comes back from its copy in the
/// update's set, with the files that went in before it: the bundle is the
/// earlier version, file for file, the receipt still says so, and the set
/// goes again.
#[test]
fn a_binary_removed_before_its_copy_failed_comes_back_from_the_set() {
    let (dir, resource) = resource_folder();

    let (error, earlier, earlier_state) =
        update_failing_at_the_binary(dir.path(), &resource, |place| {
            fs::remove_file(place).unwrap();
        });

    assert!(matches!(error, RabbitError::Io { .. }), "{error}");
    assert_eq!(snapshot(&bundle(&resource)), earlier);
    assert_eq!(
        load_install_state(&resource).unwrap().unwrap(),
        earlier_state
    );
    assert_eq!(sets_in(&resource), 0);
}

/// Should the binary's place hold anything but the earlier binary after
/// its new copy failed to take it, such as a file only marked for deletion
/// on a FAT32 or exFAT drive, which vanishes once the program that holds
/// it open lets go, RABBIT can't vouch for that place. So the update's set
/// keeps its copy of the earlier binary, with the record of the version it
/// was, for a restore to bring back; the files that went in before it are
/// put back as they were, and the receipt still names the earlier version.
#[test]
fn a_binary_whose_place_cant_be_vouched_for_keeps_its_copy_in_the_set() {
    let (dir, resource) = resource_folder();

    let (error, earlier, earlier_state) =
        update_failing_at_the_binary(dir.path(), &resource, |place| {
            fs::write(place, b"about to vanish").unwrap();
        });

    assert!(matches!(error, RabbitError::Io { .. }), "{error}");
    let binary = below(&bundle(&resource), BINARY);
    let mut expected = earlier;
    for (path, node) in &mut expected {
        if *path == binary {
            *node = crate::rollback::restore_run_tests::Node::File(b"about to vanish".to_vec());
        }
    }
    assert_eq!(snapshot(&bundle(&resource)), expected);
    assert_eq!(
        load_install_state(&resource).unwrap().unwrap(),
        earlier_state
    );
    assert_eq!(sets_in(&resource), 1);
    let set = fs::read_dir(resource.join("RABBIT").join("backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let manifest: BackupManifest = serde_json::from_str(
        &fs::read_to_string(set.join(crate::rollback::BACKUP_MANIFEST_FILE)).unwrap(),
    )
    .unwrap();
    let copy = manifest
        .files
        .iter()
        .find(|file| file.original_path == recorded(BINARY))
        .expect("the set keeps the earlier binary");
    assert_eq!(copy.state, BackupEntryState::Done);
    assert_eq!(fs::read(&copy.backup_path).unwrap(), b"binary 0.99.66");
    assert!(manifest.replaced.contains_key(PACKAGE_LJUNO_116));
}

/// A folder inside the bundle that a symbolic link or junction stands for
/// is never gone through: an update leaves a file of the version before
/// that is only reached through one where it is, and what the link leads
/// to stays as it was.
#[test]
fn an_update_never_moves_a_file_through_a_link_in_the_bundle() {
    let (dir, resource) = resource_folder();
    install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.66", V1)],
        &APPLY,
    );
    let presets = below(&bundle(&resource), "Contents/Resources/Presets");
    let elsewhere = resource.join("Data").join("Presets");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::rename(presets.join("Old.preset"), elsewhere.join("Old.preset")).unwrap();
    fs::remove_dir(&presets).unwrap();
    crate::uninstall::paths::link_folder(&presets, &elsewhere);

    let report = install(
        &resource,
        &[ljuno(&dir.path().join("cache"), "0.99.67", V2)],
        &APPLY,
    );

    let left = report
        .actions
        .iter()
        .find(|action| action.target_path.ends_with("Old.preset"))
        .unwrap();
    assert_eq!(left.action, InstallFileAction::LeftInPlace, "{left:?}");
    assert_eq!(
        fs::read(elsewhere.join("Old.preset")).unwrap(),
        b"old preset"
    );
    assert!(
        fs::symlink_metadata(&presets)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let manifest = update_set(&report);
    assert!(
        manifest
            .files
            .iter()
            .all(|file| !file.original_path.ends_with("Old.preset")),
        "{:?}",
        manifest.files
    );
}

/// A bundle never goes through a symbolic link or junction: one at the
/// bundle's folder, at a folder inside it, or at UserPlugins\FX on the way
/// refuses the install before anything is written, and what the link leads
/// to stays as it was.
#[test]
fn a_link_at_the_bundle_or_on_the_way_refuses_it_before_anything_is_written() {
    for link_at in [
        "UserPlugins/FX/LJuno-116.vst3",
        "UserPlugins/FX/LJuno-116.vst3/Contents",
        "UserPlugins/FX",
    ] {
        let (dir, resource) = resource_folder();
        let elsewhere = dir.path().join("Elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(elsewhere.join("kept.txt"), b"not RABBIT's").unwrap();
        let link = below(&resource, link_at);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        crate::uninstall::paths::link_folder(&link, &elsewhere);
        let release = ljuno(&dir.path().join("cache"), "0.99.66", V1);
        let before = snapshot(dir.path());

        for options in [&DRY_RUN, &APPLY] {
            let error =
                try_install(&resource, std::slice::from_ref(&release), options).unwrap_err();

            assert!(
                matches!(&error, RabbitError::InstallPathRefused { path, reason }
                    if *path == link && reason.contains("symbolic link or junction")),
                "{link_at}: {error}"
            );
            assert_eq!(snapshot(dir.path()), before, "{link_at}");
        }
    }
}

/// A file where the bundle needs a folder, such as a single-file VST3 of
/// the same name, and a folder where it needs a file, refuse the install
/// before anything is written, and stay as they are.
#[test]
fn a_file_or_folder_in_the_bundles_way_refuses_it_before_anything_is_written() {
    for (place, is_folder) in [
        ("UserPlugins/FX/LJuno-116.vst3", false),
        ("UserPlugins/FX/LJuno-116.vst3/Contents", false),
        (
            "UserPlugins/FX/LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3",
            true,
        ),
    ] {
        let (dir, resource) = resource_folder();
        let path = below(&resource, place);
        if is_folder {
            fs::create_dir_all(&path).unwrap();
        } else {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"in the way").unwrap();
        }
        let release = ljuno(&dir.path().join("cache"), "0.99.66", V1);
        let before = snapshot(dir.path());

        let error = try_install(&resource, &[release], &APPLY).unwrap_err();

        assert!(
            matches!(&error, RabbitError::InstallPathRefused { path: refused, .. } if *refused == path),
            "{place}: {error}"
        );
        assert_eq!(snapshot(dir.path()), before, "{place}");
    }
}
