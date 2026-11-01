//! The uninstall engine on temporary resource folders: what moves into the
//! backup set, what stays, how the receipt changes, and how a failure puts
//! everything back. Packages, detection, running programs and volumes are
//! all stood in by each test; nothing here touches a real REAPER folder.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::execute::{UNINSTALL_BACKUP_REASON, UninstallEnvironment, execute_uninstall_in};
use super::preflight::VolumeProbe;
use super::tests::{Fixture, installed, remove_files, remove_files_with, running, spec, write};
use super::{
    BACKUP_SPACE_CHECK, KeptFileReason, RemovalReason, UninstallMessage, UninstallProblem,
    UninstallReport, UninstallRequest, UninstallStatus,
};
use crate::cancel::CancelToken;
use crate::error::RabbitError;
use crate::hash::sha256_file;
use crate::model::{ComponentDetection, InstallationKind, Platform};
use crate::package::{PACKAGE_SWS, PackageSpec, builtin_package_specs};
use crate::preflight::RunningProcess;
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::receipt::{
    InstallState, InstalledFileReceipt, PackageReceipt, load_install_state, save_install_state,
};
use crate::rollback::{
    BACKUP_MANIFEST_FILE, BackupEntryState, BackupManifest, RestoreBackupOptions,
    RestoreBackupReport, restore_backup_set,
};

/// Volumes as a test wants them: one volume for everything, or the backup
/// on another one with this much room.
struct Volumes {
    same: bool,
    free: Option<u64>,
}

impl VolumeProbe for Volumes {
    fn same_volume(&self, _left: &Path, _right: &Path) -> Option<bool> {
        Some(self.same)
    }

    fn available_space(&self, _path: &Path) -> Option<u64> {
        self.free
    }
}

const ONE_VOLUME: Volumes = Volumes {
    same: true,
    free: Some(u64::MAX),
};

/// One engine run on a fixture, with everything it asks the computer stood
/// in.
struct Harness<'f> {
    fixture: &'f Fixture,
    specs: Vec<PackageSpec>,
    /// Detection before the run; afterwards nothing is installed.
    before: Vec<ComponentDetection>,
    after: Vec<ComponentDetection>,
    volumes: Volumes,
    /// The REAPERs running at each scan, in turn; the last answer repeats.
    reapers: Vec<Vec<RunningProcess>>,
    progress: ProgressReporter,
    cancel: CancelToken,
}

impl<'f> Harness<'f> {
    fn new(fixture: &'f Fixture, specs: Vec<PackageSpec>, installed_ids: &[&str]) -> Self {
        Self {
            fixture,
            specs,
            before: installed(installed_ids),
            after: installed(&[]),
            volumes: ONE_VOLUME,
            reapers: vec![Vec::new()],
            progress: ProgressReporter::noop(),
            cancel: CancelToken::new(),
        }
    }

    fn app(&self) -> PathBuf {
        self.fixture.dir.path().join("Program").join("reaper.exe")
    }

    fn request(&self, ids: &[&str]) -> UninstallRequest {
        UninstallRequest {
            resource_path: self.fixture.resource.clone(),
            platform: Platform::Windows,
            package_ids: ids.iter().map(ToString::to_string).collect(),
            remove_settings: false,
            dry_run: false,
            target_app_path: Some(self.app()),
            target: Some(InstallationKind::Standard),
        }
    }

    fn apply(&self, ids: &[&str]) -> UninstallReport {
        self.run(&self.request(ids))
    }

    fn dry_run(&self, ids: &[&str]) -> UninstallReport {
        self.run(&UninstallRequest {
            dry_run: true,
            ..self.request(ids)
        })
    }

    fn run(&self, request: &UninstallRequest) -> UninstallReport {
        let detections = Cell::new(0);
        let detect = || {
            let call = detections.get();
            detections.set(call + 1);
            Ok(if call == 0 {
                self.before.clone()
            } else {
                self.after.clone()
            })
        };
        let scans = Cell::new(0);
        let reapers = || {
            let scan = scans.get();
            scans.set(scan + 1);
            self.reapers[scan.min(self.reapers.len() - 1)].clone()
        };
        let named = |_: &[String]| Vec::new();
        execute_uninstall_in(
            request,
            &UninstallEnvironment {
                specs: &self.specs,
                roots: &self.fixture.roots,
                target: InstallationKind::Standard,
                target_app_path: Some(self.app()),
                detect: &detect,
                reaper_processes: &reapers,
                named_processes: &named,
                volumes: &self.volumes,
            },
            &self.progress,
            &self.cancel,
        )
        .unwrap()
    }
}

/// `relative`, with `/` between its components, inside the resource folder.
fn at(fixture: &Fixture, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(fixture.resource.clone(), |path, part| path.join(part))
}

/// Save a receipt listing, for each package, its files with their real
/// hashes, as an install would have written it.
fn save_receipt(fixture: &Fixture, packages: &[(&str, &[&str])]) -> InstallState {
    let mut state = InstallState::default();
    for (id, files) in packages {
        let installed_files = files
            .iter()
            .map(|relative| {
                let path = at(fixture, relative);
                InstalledFileReceipt {
                    path: relative.split('/').collect(),
                    sha256: Some(sha256_file(&path).unwrap()),
                    size: Some(fs::metadata(&path).unwrap().len()),
                }
            })
            .collect();
        state.packages.insert(
            id.to_string(),
            PackageReceipt {
                id: id.to_string(),
                version: None,
                source_url: None,
                source_sha256: None,
                variant: None,
                channel: None,
                installed_files,
                installed_at: None,
                rabbit_version: None,
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );
    }
    save_install_state(&fixture.resource, &state).unwrap();
    state
}

fn manifest(report: &UninstallReport) -> BackupManifest {
    let set = report.backup_set_path.as_ref().expect("a backup set");
    serde_json::from_str(&fs::read_to_string(set.join(BACKUP_MANIFEST_FILE)).unwrap()).unwrap()
}

fn status(report: &UninstallReport, id: &str) -> UninstallStatus {
    report
        .item(id)
        .unwrap_or_else(|| panic!("no item for {id}"))
        .status
}

/// Every file and folder below `root`, with each file's contents.
fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut found = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.insert(path.clone(), None);
            found.extend(tree(&path));
        } else {
            found.insert(path.clone(), Some(fs::read(&path).unwrap()));
        }
    }
    found
}

fn recorded_events() -> (ProgressReporter, Arc<Mutex<Vec<ProgressEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let reporter = ProgressReporter::new(move |event| sink.lock().unwrap().push(event));
    (reporter, events)
}

/// Keep `path` the way another program holding it open would: readable,
/// but impossible to move or delete. Letting go of the guard ends that.
/// Windows refuses to move a file another program opened without sharing
/// deletion; Unix lets anyone move an open file, so a folder that can't be
/// written stands in.
struct Held {
    #[cfg(windows)]
    _file: fs::File,
    #[cfg(unix)]
    folder: PathBuf,
}

fn hold(path: &Path) -> Held {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_SHARE_READ: u32 = 1;
        Held {
            _file: fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(path)
                .unwrap(),
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let folder = path.parent().unwrap().to_path_buf();
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
        Held { folder }
    }
}

#[cfg(unix)]
impl Drop for Held {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;

        let _ = fs::set_permissions(&self.folder, fs::Permissions::from_mode(0o755));
    }
}

fn demo_spec(id: &str) -> PackageSpec {
    spec(
        id,
        "",
        &remove_files(r#""remove": { "receipt_files": true, "matching_user_plugins": true }"#),
    )
}

/// The whole way through: the files move into one backup set that records
/// the receipt entry, the receipt forgets the package and remembers it was
/// removed, the report is saved, and today's restore brings the files back.
#[test]
fn removed_files_move_into_a_backup_set_and_the_receipt_forgets_the_package() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo-x64.dll");
    let script = fixture.file("Scripts/demo.py");
    let other = fixture.file("UserPlugins/reaper_other.dll");
    let state = save_receipt(
        &fixture,
        &[
            (
                "demo",
                &["UserPlugins/reaper_demo-x64.dll", "Scripts/demo.py"],
            ),
            ("other", &["UserPlugins/reaper_other.dll"]),
        ],
    );
    let (progress, events) = recorded_events();
    let harness = Harness {
        progress,
        ..Harness::new(&fixture, vec![demo_spec("demo")], &["demo"])
    };

    let report = harness.apply(&["demo"]);

    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::Removed, "{}", demo.message);
    assert!(!plugin.exists() && !script.exists());
    assert!(other.is_file(), "another package's file stays");
    let mut removed: Vec<PathBuf> = demo.removed.iter().map(|file| file.path.clone()).collect();
    removed.sort();
    let mut expected = vec![
        at(&fixture, "UserPlugins/reaper_demo-x64.dll"),
        at(&fixture, "Scripts/demo.py"),
    ];
    expected.sort();
    assert_eq!(removed, expected);
    assert!(demo.removed.iter().all(|file| !file.changed_since_install));
    assert!(!report.has_failures() && !report.blocked_before_changes());

    let set_id = report.backup_set_id.clone().unwrap();
    assert!(set_id.starts_with("uninstall-unix-"));
    assert!(demo.message.contains(&set_id), "{}", demo.message);
    let manifest = manifest(&report);
    assert_eq!(manifest.reason, UNINSTALL_BACKUP_REASON);
    assert_eq!(manifest.files.len(), 2);
    for entry in &manifest.files {
        assert_eq!(entry.state, BackupEntryState::Done);
        assert_eq!(entry.package_id.as_deref(), Some("demo"));
        assert_eq!(fs::read(&entry.backup_path).unwrap(), b"file contents");
    }
    assert_eq!(
        manifest.packages["demo"].receipt.as_ref(),
        state.packages.get("demo")
    );
    assert_eq!(manifest.packages["demo"].langpack_before, None);

    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    assert!(!receipt.packages.contains_key("demo"));
    assert!(receipt.declined_packages.contains("demo"));
    assert!(receipt.packages.contains_key("other"));

    assert_eq!(
        *events.lock().unwrap(),
        vec![
            ProgressEvent::RemoveStarted {
                package_id: "demo".to_string()
            },
            ProgressEvent::RemoveCompleted {
                package_id: "demo".to_string(),
                removed: true
            },
        ]
    );
    let saved = report.saved_report.as_ref().expect("a saved report");
    assert!(saved.json_path.is_file() && saved.text_path.is_file());
    assert!(
        saved
            .json_path
            .starts_with(fixture.resource.join("RABBIT").join("logs"))
    );

    // A set written by moving restores like any other. The receipt entry
    // the set recorded goes back into the receipt, and the package is no
    // longer remembered as removed.
    restore_backup_set(
        &fixture.resource,
        &set_id,
        &RestoreBackupOptions {
            dry_run: false,
            allow_reaper_running: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(fs::read(&plugin).unwrap(), b"file contents");
    assert_eq!(fs::read(&script).unwrap(), b"file contents");
    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    assert_eq!(receipt, state);
}

/// A file only the receipt names that changed since RABBIT installed it is
/// most likely the user's own now, so it stays. A file the manifest names
/// goes even when it changed, after its backup.
#[test]
fn a_changed_receipt_file_stays_and_a_changed_manifest_file_goes() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let data = fixture.file("Data/demo.dat");
    save_receipt(
        &fixture,
        &[("demo", &["UserPlugins/reaper_demo.dll", "Data/demo.dat"])],
    );
    fs::write(&plugin, b"a newer plug-in copied in by hand").unwrap();
    fs::write(&data, b"the user's own changes").unwrap();
    let harness = Harness::new(&fixture, vec![demo_spec("demo")], &["demo"]);

    let preview = harness.dry_run(&["demo"]);
    let demo = preview.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::WouldRemove);
    assert_eq!(demo.removed.len(), 1);
    assert_eq!(demo.kept.len(), 1);

    let report = harness.apply(&["demo"]);
    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::Removed);
    assert_eq!(demo.removed.len(), 1);
    assert_eq!(demo.removed[0].path, plugin);
    assert_eq!(demo.removed[0].reason, RemovalReason::MatchingUserPlugin);
    assert!(demo.removed[0].changed_since_install);
    assert!(!plugin.exists());
    assert_eq!(
        fs::read(demo.removed[0].backup_path.as_ref().unwrap()).unwrap(),
        b"a newer plug-in copied in by hand"
    );

    assert_eq!(demo.kept.len(), 1);
    assert_eq!(demo.kept[0].path, at(&fixture, "Data/demo.dat"));
    assert_eq!(demo.kept[0].reason, KeptFileReason::ChangedSinceInstall);
    assert_eq!(fs::read(&data).unwrap(), b"the user's own changes");
    assert!(
        demo.message.contains("changed since RABBIT installed"),
        "{}",
        demo.message
    );
}

/// When a file can't be moved, the files that already moved go back, the
/// receipt stays as it was, and the backup set disappears again.
#[test]
fn a_failed_move_puts_back_what_already_moved() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let script = fixture.file("Scripts/demo.py");
    let state = save_receipt(&fixture, &[("demo", &["Scripts/demo.py"])]);
    let (progress, events) = recorded_events();
    let harness = Harness {
        progress,
        ..Harness::new(&fixture, vec![demo_spec("demo")], &["demo"])
    };

    let held = hold(&script);
    let report = harness.apply(&["demo"]);
    drop(held);

    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::Failed);
    assert!(
        matches!(&demo.message_code, UninstallMessage::MoveFailed { path, .. } if *path == at(&fixture, "Scripts/demo.py")),
        "{:?}",
        demo.message_code
    );
    assert_eq!(demo.put_back, vec![plugin.clone()]);
    assert!(demo.left_in_backup.is_empty() && demo.removed.is_empty());
    assert!(demo.message.contains("back in place"), "{}", demo.message);
    assert_eq!(fs::read(&plugin).unwrap(), b"file contents");
    assert_eq!(fs::read(&script).unwrap(), b"file contents");
    assert_eq!(
        load_install_state(&fixture.resource).unwrap().unwrap(),
        state
    );
    assert_eq!(report.backup_set_id, None);
    assert!(
        fs::read_dir(fixture.resource.join("RABBIT").join("backups"))
            .map(|entries| entries.count() == 0)
            .unwrap_or(true),
        "the empty set is gone again"
    );
    assert!(report.has_failures());
    assert_eq!(
        events.lock().unwrap().last(),
        Some(&ProgressEvent::RemoveCompleted {
            package_id: "demo".to_string(),
            removed: false
        })
    );
}

/// A file on another drive is copied, checked and deleted. When another
/// program holds it, the delete fails, the copy goes again and the file
/// stays where it was.
#[test]
fn a_file_in_use_on_another_volume_stays_where_it_was() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let harness = Harness {
        volumes: Volumes {
            same: false,
            free: Some(u64::MAX),
        },
        ..Harness::new(&fixture, vec![demo_spec("demo")], &["demo"])
    };

    let held = hold(&plugin);
    let report = harness.apply(&["demo"]);
    drop(held);

    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::Failed);
    assert!(
        matches!(&demo.message_code, UninstallMessage::FileInUse { path, .. } if *path == plugin),
        "{:?}",
        demo.message_code
    );
    assert!(demo.message.contains("in use by another program"));
    assert_eq!(fs::read(&plugin).unwrap(), b"file contents");
    assert_eq!(report.backup_set_id, None);

    // Not held, the same file moves as a checked copy.
    let report = harness.apply(&["demo"]);
    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::Removed, "{}", demo.message);
    assert!(!plugin.exists());
    assert_eq!(
        fs::read(demo.removed[0].backup_path.as_ref().unwrap()).unwrap(),
        b"file contents"
    );
}

/// Before anything moves to another drive, the backup's drive must have room
/// for it. Without that room the package is blocked and nothing changes.
#[test]
fn the_free_space_check_blocks_a_move_to_another_volume() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let harness = Harness {
        volumes: Volumes {
            same: false,
            free: Some(1024),
        },
        ..Harness::new(&fixture, vec![demo_spec("demo")], &["demo"])
    };

    for report in [harness.dry_run(&["demo"]), harness.apply(&["demo"])] {
        let demo = report.item("demo").unwrap();
        assert_eq!(demo.status, UninstallStatus::Blocked);
        let UninstallMessage::ChecksFailed { checks } = &demo.message_code else {
            panic!("{:?}", demo.message_code);
        };
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, BACKUP_SPACE_CHECK);
        assert!(!report.preflight.passed);
        assert!(report.blocked_before_changes());
        assert!(plugin.is_file());
    }
}

/// The cancel token is read between packages: the package that is running
/// finishes, and the next one isn't started.
#[test]
fn cancelling_stops_between_packages() {
    let fixture = Fixture::new();
    let first = fixture.file("UserPlugins/reaper_first.dll");
    let second = fixture.file("UserPlugins/reaper_second.dll");
    let state = save_receipt(&fixture, &[("second", &["UserPlugins/reaper_second.dll"])]);
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let progress = ProgressReporter::new(move |event| {
        if matches!(event, ProgressEvent::RemoveCompleted { ref package_id, .. } if package_id == "first")
        {
            stop.cancel();
        }
    });
    let harness = Harness {
        progress,
        cancel,
        ..Harness::new(
            &fixture,
            vec![demo_spec("first"), demo_spec("second")],
            &["first", "second"],
        )
    };

    let report = harness.apply(&["first", "second"]);

    assert_eq!(status(&report, "first"), UninstallStatus::Removed);
    assert_eq!(status(&report, "second"), UninstallStatus::Cancelled);
    assert!(!first.exists());
    assert!(second.is_file());
    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    assert_eq!(receipt.packages, state.packages);
    assert!(receipt.declined_packages.contains("first"));
    assert!(!receipt.declined_packages.contains("second"));
    assert!(report.was_cancelled() && report.has_failures());
}

/// A dry run plans, checks and hashes, and changes nothing at all: no lock,
/// no backup set, no receipt change, no report file.
#[test]
fn a_dry_run_leaves_the_tree_exactly_as_it_was() {
    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    fixture.file("Scripts/demo.py");
    fixture.file("Demo.ini");
    fixture.file("DemoData/presets/one.preset");
    fixture.file("reaper.ini");
    save_receipt(&fixture, &[("demo", &["Scripts/demo.py"])]);
    let specs = vec![spec(
        "demo",
        "",
        &remove_files(
            r#""remove": { "receipt_files": true, "matching_user_plugins": true },
               "kept_user_data": [
                   { "path": "{resource}/Demo.ini", "note_key": "test-settings" },
                   { "path": "{resource}/DemoData", "note_key": "test-data" }
               ]"#,
        ),
    )];
    let harness = Harness::new(&fixture, specs, &["demo"]);
    let before = tree(fixture.dir.path());

    let report = harness.run(&UninstallRequest {
        dry_run: true,
        remove_settings: true,
        ..harness.request(&["demo"])
    });

    assert_eq!(tree(fixture.dir.path()), before);
    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::WouldRemove);
    let mut removed: Vec<PathBuf> = demo.removed.iter().map(|file| file.path.clone()).collect();
    removed.sort();
    let mut expected = vec![
        at(&fixture, "UserPlugins/reaper_demo.dll"),
        at(&fixture, "Scripts/demo.py"),
        at(&fixture, "Demo.ini"),
        at(&fixture, "DemoData/presets/one.preset"),
    ];
    expected.sort();
    assert_eq!(removed, expected);
    assert!(demo.removed.iter().all(|file| file.backup_path.is_none()));
    assert!(report.dry_run && report.backup_set_id.is_none());
    assert!(report.saved_report.is_none());
}

/// The language reset runs only for a pack file the run actually removed,
/// and records the old value in the backup set first.
#[test]
fn the_language_is_reset_only_when_its_pack_was_removed() {
    let pack_spec = || {
        spec(
            "langpack-demo",
            r#""install_destination": "lang_pack","#,
            &remove_files_with(
                r#""requires_receipt": true,"#,
                r#""remove": { "receipt_files": true },
                   "post": [ { "reset_reaper_langpack_if_selected": {} } ]"#,
            ),
        )
    };
    let selected = "[REAPER]\r\nwnd_state=1\r\nlangpack=de_DE.ReaperLangPack\r\n";

    // Removed: REAPER goes back to no language pack.
    let fixture = Fixture::new();
    let pack = fixture.file("LangPack/de_DE.ReaperLangPack");
    save_receipt(
        &fixture,
        &[("langpack-demo", &["LangPack/de_DE.ReaperLangPack"])],
    );
    let ini = write(&fixture.resource.join("reaper.ini"), selected.as_bytes());
    let report =
        Harness::new(&fixture, vec![pack_spec()], &["langpack-demo"]).apply(&["langpack-demo"]);
    assert_eq!(status(&report, "langpack-demo"), UninstallStatus::Removed);
    assert!(!pack.exists());
    assert_eq!(
        fs::read_to_string(&ini).unwrap(),
        "[REAPER]\r\nwnd_state=1\r\nlangpack=<>\r\n"
    );
    assert_eq!(
        manifest(&report).packages["langpack-demo"]
            .langpack_before
            .as_deref(),
        Some("de_DE.ReaperLangPack")
    );
    assert!(report.item("langpack-demo").unwrap().problems.is_empty());

    // Kept because it changed: REAPER keeps using it.
    let fixture = Fixture::new();
    let pack = fixture.file("LangPack/de_DE.ReaperLangPack");
    save_receipt(
        &fixture,
        &[("langpack-demo", &["LangPack/de_DE.ReaperLangPack"])],
    );
    fs::write(&pack, b"translated by the user").unwrap();
    let ini = write(&fixture.resource.join("reaper.ini"), selected.as_bytes());
    let report =
        Harness::new(&fixture, vec![pack_spec()], &["langpack-demo"]).apply(&["langpack-demo"]);
    let item = report.item("langpack-demo").unwrap();
    assert_eq!(item.status, UninstallStatus::Removed);
    assert!(item.removed.is_empty());
    assert_eq!(item.kept[0].reason, KeptFileReason::ChangedSinceInstall);
    assert!(pack.is_file());
    assert_eq!(fs::read_to_string(&ini).unwrap(), selected);
    assert_eq!(
        manifest(&report).packages["langpack-demo"].langpack_before,
        None
    );

    // Removed, but REAPER uses another pack: that one is the user's choice.
    let fixture = Fixture::new();
    fixture.file("LangPack/de_DE.ReaperLangPack");
    save_receipt(
        &fixture,
        &[("langpack-demo", &["LangPack/de_DE.ReaperLangPack"])],
    );
    let other = "[REAPER]\r\nlangpack=Deutsch.ReaperLangPack\r\n";
    let ini = write(&fixture.resource.join("reaper.ini"), other.as_bytes());
    Harness::new(&fixture, vec![pack_spec()], &["langpack-demo"]).apply(&["langpack-demo"]);
    assert_eq!(fs::read_to_string(&ini).unwrap(), other);
}

/// A language reset that fails leaves the pack removed: the files are gone
/// and the receipt says so. The report tells the user to pick a language in
/// REAPER, and the set records no value to put back.
#[test]
fn a_failed_language_reset_leaves_the_pack_removed() {
    let fixture = Fixture::new();
    let pack = fixture.file("LangPack/de_DE.ReaperLangPack");
    save_receipt(
        &fixture,
        &[("langpack-demo", &["LangPack/de_DE.ReaperLangPack"])],
    );
    let selected = "[REAPER]\r\nlangpack=de_DE.ReaperLangPack\r\n";
    let ini = write(&fixture.resource.join("reaper.ini"), selected.as_bytes());
    let mut read_only = fs::metadata(&ini).unwrap().permissions();
    read_only.set_readonly(true);
    fs::set_permissions(&ini, read_only.clone()).unwrap();
    let specs = vec![spec(
        "langpack-demo",
        r#""install_destination": "lang_pack","#,
        &remove_files_with(
            r#""requires_receipt": true,"#,
            r#""remove": { "receipt_files": true },
               "post": [ { "reset_reaper_langpack_if_selected": {} } ]"#,
        ),
    )];

    let report = Harness::new(&fixture, specs, &["langpack-demo"]).apply(&["langpack-demo"]);

    #[allow(clippy::permissions_set_readonly_false)]
    read_only.set_readonly(false);
    fs::set_permissions(&ini, read_only).unwrap();
    let item = report.item("langpack-demo").unwrap();
    assert_eq!(item.status, UninstallStatus::Removed);
    assert!(matches!(
        item.problems.as_slice(),
        [UninstallProblem::LanguageResetFailed { .. }]
    ));
    assert!(
        item.message.contains("REAPER's preferences"),
        "{}",
        item.message
    );
    assert!(report.finished_with_problems() && !report.has_failures());
    assert!(!pack.exists());
    assert_eq!(fs::read_to_string(&ini).unwrap(), selected);
    assert!(
        !load_install_state(&fixture.resource)
            .unwrap()
            .unwrap()
            .packages
            .contains_key("langpack-demo")
    );
    assert_eq!(
        manifest(&report).packages["langpack-demo"].langpack_before,
        None
    );
}

/// One requested package that can't be removed makes the whole run change
/// nothing, so the user is never left with half of what they asked for.
#[test]
fn a_blocked_package_makes_the_run_change_nothing() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let pack = fixture.file("LangPack/de_DE.ReaperLangPack");
    let specs = vec![
        demo_spec("demo"),
        spec(
            "langpack-demo",
            r#""install_destination": "lang_pack","#,
            &remove_files_with(
                r#""requires_receipt": true,"#,
                r#""remove": { "receipt_files": true }"#,
            ),
        ),
    ];
    let harness = Harness::new(&fixture, specs, &["demo", "langpack-demo"]);

    let report = harness.apply(&["demo", "langpack-demo", "nonsense"]);

    assert_eq!(status(&report, "langpack-demo"), UninstallStatus::Blocked);
    assert_eq!(status(&report, "nonsense"), UninstallStatus::Blocked);
    assert_eq!(
        report.item("nonsense").unwrap().message_code,
        UninstallMessage::UnknownPackage
    );
    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::NotReached);
    assert_eq!(
        demo.message_code,
        UninstallMessage::OtherPackageBlocked {
            package_ids: vec!["langpack-demo".to_string(), "nonsense".to_string()]
        }
    );
    assert!(plugin.is_file() && pack.is_file());
    assert_eq!(report.backup_set_id, None);
    assert!(load_install_state(&fixture.resource).unwrap().is_none());
    assert!(report.blocked_before_changes() && !report.has_failures());

    // The dry run still says what the removable package would do.
    let preview = harness.dry_run(&["demo", "langpack-demo"]);
    assert_eq!(status(&preview, "demo"), UninstallStatus::WouldRemove);
    assert_eq!(status(&preview, "langpack-demo"), UninstallStatus::Blocked);
}

/// REAPER starting while the run goes on stops it before the next package:
/// what was removed stays removed, and the rest isn't touched.
#[test]
fn reaper_starting_between_packages_stops_the_run() {
    let fixture = Fixture::new();
    let first = fixture.file("UserPlugins/reaper_first.dll");
    let second = fixture.file("UserPlugins/reaper_second.dll");
    let third = fixture.file("UserPlugins/reaper_third.dll");
    let mut harness = Harness::new(
        &fixture,
        vec![demo_spec("first"), demo_spec("second"), demo_spec("third")],
        &["first", "second", "third"],
    );
    let reaper = running("7", "reaper.exe", &harness.app());
    // The preflight and the check before the first package find nothing;
    // the check before the second finds REAPER.
    harness.reapers = vec![Vec::new(), Vec::new(), vec![reaper]];

    let report = harness.apply(&["first", "second", "third"]);

    assert_eq!(status(&report, "first"), UninstallStatus::Removed);
    let second_item = report.item("second").unwrap();
    assert_eq!(second_item.status, UninstallStatus::Blocked);
    assert!(matches!(
        second_item.message_code,
        UninstallMessage::ChecksFailed { .. }
    ));
    assert_eq!(
        report.item("third").unwrap().message_code,
        UninstallMessage::RunStopped {
            package_id: "second".to_string()
        }
    );
    assert_eq!(status(&report, "third"), UninstallStatus::NotReached);
    assert!(!first.exists() && second.is_file() && third.is_file());
    assert!(report.has_failures() && !report.blocked_before_changes());
}

/// When the receipt can't be saved after the files moved, they move back:
/// the receipt must never claim files that are gone, nor lose an entry for
/// files still there.
#[test]
fn a_receipt_that_cant_be_saved_puts_the_files_back() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let state = save_receipt(&fixture, &[("demo", &["UserPlugins/reaper_demo.dll"])]);
    // A folder where the receipt's temporary file goes makes every save
    // fail, while the receipt itself still loads.
    fixture.file("RABBIT/install-state.json.rabbit-tmp/blocker");
    let harness = Harness::new(&fixture, vec![demo_spec("demo")], &["demo"]);

    let report = harness.apply(&["demo"]);

    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::Failed);
    assert!(matches!(
        demo.message_code,
        UninstallMessage::ReceiptSaveFailed { .. }
    ));
    assert_eq!(
        demo.put_back,
        vec![at(&fixture, "UserPlugins/reaper_demo.dll")]
    );
    assert_eq!(fs::read(&plugin).unwrap(), b"file contents");
    assert_eq!(
        load_install_state(&fixture.resource).unwrap().unwrap(),
        state
    );
    assert_eq!(report.backup_set_id, None);
}

/// "Also remove settings" moves the package's settings into the same set,
/// a folder of them file by file; without it they stay. A settings folder
/// holding a link stays whole.
#[test]
fn settings_move_into_the_set_only_when_asked() {
    let specs = || {
        vec![spec(
            "demo",
            "",
            &remove_files(
                r#""remove": { "matching_user_plugins": true },
                   "kept_user_data": [
                       { "path": "{resource}/Demo.ini", "note_key": "test-settings" },
                       { "path": "{resource}/DemoData", "note_key": "test-data" }
                   ]"#,
            ),
        )]
    };

    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    let settings = fixture.file("Demo.ini");
    let preset = fixture.file("DemoData/presets/one.preset");
    let harness = Harness::new(&fixture, specs(), &["demo"]);
    let report = harness.apply(&["demo"]);
    assert_eq!(status(&report, "demo"), UninstallStatus::Removed);
    assert!(
        settings.is_file() && preset.is_file(),
        "settings stay by default"
    );

    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    let settings = fixture.file("Demo.ini");
    let preset = fixture.file("DemoData/presets/one.preset");
    let harness = Harness::new(&fixture, specs(), &["demo"]);
    let report = harness.run(&UninstallRequest {
        remove_settings: true,
        ..harness.request(&["demo"])
    });
    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::Removed, "{}", demo.message);
    assert!(!settings.exists() && !preset.exists());
    assert!(!fixture.resource.join("DemoData").exists());
    assert!(
        demo.removed_dirs
            .contains(&fixture.resource.join("DemoData"))
    );
    let set = report.backup_set_path.clone().unwrap();
    assert_eq!(
        fs::read(set.join("DemoData").join("presets").join("one.preset")).unwrap(),
        b"file contents"
    );
    assert!(
        demo.removed
            .iter()
            .filter(|file| file.reason == RemovalReason::Setting)
            .count()
            == 2
    );

    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    let preset = fixture.file("DemoData/presets/one.preset");
    let elsewhere = fixture.dir.path().join("Elsewhere");
    let outside = write(&elsewhere.join("mine.preset"), b"mine");
    #[cfg(windows)]
    super::paths::make_junction(
        &fixture.resource.join("DemoData").join("linked"),
        &elsewhere,
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(&elsewhere, fixture.resource.join("DemoData").join("linked"))
        .unwrap();
    let harness = Harness::new(&fixture, specs(), &["demo"]);
    let report = harness.run(&UninstallRequest {
        remove_settings: true,
        ..harness.request(&["demo"])
    });
    let demo = report.item("demo").unwrap();
    assert_eq!(demo.status, UninstallStatus::Removed);
    assert_eq!(demo.kept.len(), 1);
    assert_eq!(demo.kept[0].reason, KeptFileReason::HoldsLink);
    assert!(preset.is_file() && outside.is_file());
}

/// The shipped plans carried out, not only planned: SWS and the German
/// language pack come off, the grooves, the settings and a pack placed by
/// hand stay, REAPER goes back to no language pack, and both packages are
/// remembered as removed.
#[test]
fn the_shipped_plans_for_sws_and_a_language_pack_carry_out() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_sws-x64.dll");
    let bridge = fixture.file("Scripts/sws_python.py");
    let bridge64 = fixture.file("Scripts/sws_python64.py");
    let groove = fixture.file("Data/Grooves/Swing.rgt");
    let settings = fixture.file("S&M.ini");
    let pack = fixture.file("LangPack/de_DE.ReaperLangPack");
    let by_hand = fixture.file("LangPack/Deutsch.ReaperLangPack");
    let ini = write(
        &fixture.resource.join("reaper.ini"),
        b"[REAPER]\r\nlangpack=de_DE.ReaperLangPack\r\n",
    );
    save_receipt(
        &fixture,
        &[
            (
                PACKAGE_SWS,
                &["UserPlugins/reaper_sws-x64.dll", "Scripts/sws_python.py"],
            ),
            ("langpack-de", &["LangPack/de_DE.ReaperLangPack"]),
        ],
    );
    let ids = [PACKAGE_SWS, "langpack-de"];
    let harness = Harness::new(&fixture, builtin_package_specs(Platform::Windows), &ids);

    let report = harness.apply(&ids);

    for id in ids {
        let item = report.item(id).unwrap();
        assert_eq!(item.status, UninstallStatus::Removed, "{}", item.message);
        assert!(item.problems.is_empty(), "{:?}", item.problems);
    }
    assert!(!plugin.exists() && !bridge.exists() && !bridge64.exists() && !pack.exists());
    assert!(groove.is_file() && settings.is_file() && by_hand.is_file());
    assert_eq!(
        fs::read_to_string(&ini).unwrap(),
        "[REAPER]\r\nlangpack=<>\r\n"
    );
    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    for id in ids {
        assert!(!receipt.packages.contains_key(id));
        assert!(receipt.declined_packages.contains(id));
    }
    assert_eq!(manifest(&report).files.len(), 4);
}

/// What a restore of `package_id` alone, from the set `set_id`, says or does.
fn restore_package(
    fixture: &Fixture,
    set_id: &str,
    package_id: &str,
    dry_run: bool,
) -> crate::Result<RestoreBackupReport> {
    restore_backup_set(
        &fixture.resource,
        set_id,
        &RestoreBackupOptions {
            dry_run,
            allow_reaper_running: true,
            package_id: Some(package_id.to_string()),
        },
    )
}

/// Restoring one package from an uninstall set brings back its files and
/// nothing else. Its receipt entry goes back into the receipt as it is now,
/// rather than the whole file going back to how it was, and the package
/// comes off `declined_packages`.
#[test]
fn restoring_one_package_merges_its_receipt_entry_back() {
    let fixture = Fixture::new();
    let demo = fixture.file("UserPlugins/reaper_demo.dll");
    let other = fixture.file("UserPlugins/reaper_other.dll");
    let state = save_receipt(
        &fixture,
        &[
            ("demo", &["UserPlugins/reaper_demo.dll"]),
            ("other", &["UserPlugins/reaper_other.dll"]),
        ],
    );
    let harness = Harness::new(
        &fixture,
        vec![demo_spec("demo"), demo_spec("other")],
        &["demo", "other"],
    );
    let report = harness.apply(&["demo", "other"]);
    assert!(report.changed_anything() && !report.has_failures());
    let set_id = report.backup_set_id.clone().unwrap();

    // The receipt moves on after the uninstall: another package arrives.
    let mut later = load_install_state(&fixture.resource).unwrap().unwrap();
    later.packages.insert(
        "later".to_string(),
        PackageReceipt {
            id: "later".to_string(),
            ..state.packages["demo"].clone()
        },
    );
    save_install_state(&fixture.resource, &later).unwrap();

    let preview = restore_package(&fixture, &set_id, "demo", true).unwrap();
    assert_eq!(preview.actions.len(), 1);
    assert_eq!(preview.actions[0].target_path, demo);
    assert_eq!(preview.packages.len(), 1);
    assert!(preview.packages[0].receipt_merged);
    assert!(!demo.exists(), "a dry run restores nothing");
    assert_eq!(
        load_install_state(&fixture.resource).unwrap().unwrap(),
        later
    );

    let restored = restore_package(&fixture, &set_id, "demo", false).unwrap();
    assert_eq!(restored.package_id.as_deref(), Some("demo"));
    assert_eq!(fs::read(&demo).unwrap(), b"file contents");
    assert!(!other.exists(), "only the package asked for comes back");
    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    assert_eq!(receipt.packages.get("demo"), state.packages.get("demo"));
    assert!(
        receipt.packages.contains_key("later"),
        "the receipt is merged into, not replaced"
    );
    assert!(!receipt.packages.contains_key("other"));
    assert!(!receipt.declined_packages.contains("demo"));
    assert!(receipt.declined_packages.contains("other"));

    // A second restore would put the old copy under a receipt entry that
    // now describes the files in place.
    let again = restore_package(&fixture, &set_id, "demo", true).unwrap_err();
    assert!(
        matches!(&again, RabbitError::PackageRestoreRefused { package_id, .. } if package_id == "demo"),
        "{again}"
    );
    // So would the whole set, which holds demo too; it says how to restore
    // the rest, and restores nothing.
    let whole = restore_backup_set(
        &fixture.resource,
        &set_id,
        &RestoreBackupOptions {
            dry_run: false,
            allow_reaper_running: true,
            package_id: None,
        },
    )
    .unwrap_err();
    assert!(whole.to_string().contains("--package"), "{whole}");
    assert!(!other.exists());
    // A package the set doesn't hold is refused, naming the ones it does.
    let unknown = restore_package(&fixture, &set_id, "nosuch", true).unwrap_err();
    assert!(unknown.to_string().contains("demo, other"), "{unknown}");

    restore_package(&fixture, &set_id, "other", false).unwrap();
    assert!(other.is_file());
    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    assert_eq!(receipt.packages.get("other"), state.packages.get("other"));
    assert!(receipt.declined_packages.is_empty());
}

/// A package RABBIT had no receipt for comes back without one, and still
/// comes off `declined_packages`.
#[test]
fn restoring_a_package_without_a_receipt_only_clears_the_refusal() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let report = Harness::new(&fixture, vec![demo_spec("demo")], &["demo"]).apply(&["demo"]);
    assert_eq!(status(&report, "demo"), UninstallStatus::Removed);
    let set_id = report.backup_set_id.clone().unwrap();
    assert!(manifest(&report).packages["demo"].receipt.is_none());

    let restored = restore_package(&fixture, &set_id, "demo", false).unwrap();
    assert!(!restored.packages[0].receipt_merged);
    assert!(plugin.is_file());
    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    assert!(!receipt.packages.contains_key("demo"));
    assert!(!receipt.declined_packages.contains("demo"));
}

fn langpack_spec() -> PackageSpec {
    spec(
        "langpack-demo",
        r#""install_destination": "lang_pack","#,
        &remove_files_with(
            r#""requires_receipt": true,"#,
            r#""remove": { "receipt_files": true },
               "post": [ { "reset_reaper_langpack_if_selected": {} } ]"#,
        ),
    )
}

/// Restoring a language pack puts REAPER's language back to it, but only
/// while REAPER still uses no language pack: one chosen since is the
/// user's.
#[test]
fn restoring_a_language_pack_puts_its_language_back_while_none_was_chosen() {
    let selected = "[REAPER]\r\nwnd_state=1\r\nlangpack=de_DE.ReaperLangPack\r\n";
    let uninstall = |fixture: &Fixture| {
        fixture.file("LangPack/de_DE.ReaperLangPack");
        save_receipt(
            fixture,
            &[("langpack-demo", &["LangPack/de_DE.ReaperLangPack"])],
        );
        let ini = write(&fixture.resource.join("reaper.ini"), selected.as_bytes());
        let report = Harness::new(fixture, vec![langpack_spec()], &["langpack-demo"])
            .apply(&["langpack-demo"]);
        assert_eq!(
            fs::read_to_string(&ini).unwrap(),
            "[REAPER]\r\nwnd_state=1\r\nlangpack=<>\r\n"
        );
        (ini, report.backup_set_id.unwrap())
    };

    let fixture = Fixture::new();
    let (ini, set_id) = uninstall(&fixture);
    let preview = restore_package(&fixture, &set_id, "langpack-demo", true).unwrap();
    assert_eq!(
        preview.packages[0].langpack_before.as_deref(),
        Some("de_DE.ReaperLangPack")
    );
    assert!(preview.packages[0].langpack_restored);
    assert_eq!(
        fs::read_to_string(&ini).unwrap(),
        "[REAPER]\r\nwnd_state=1\r\nlangpack=<>\r\n"
    );
    let restored = restore_package(&fixture, &set_id, "langpack-demo", false).unwrap();
    assert!(restored.packages[0].langpack_restored);
    assert!(restored.packages[0].langpack_error.is_none());
    assert_eq!(fs::read_to_string(&ini).unwrap(), selected);
    assert!(at(&fixture, "LangPack/de_DE.ReaperLangPack").is_file());

    let fixture = Fixture::new();
    let (ini, set_id) = uninstall(&fixture);
    let chosen = "[REAPER]\r\nwnd_state=1\r\nlangpack=fr_CA.ReaperLangPack\r\n";
    fs::write(&ini, chosen).unwrap();
    let restored = restore_package(&fixture, &set_id, "langpack-demo", false).unwrap();
    assert!(!restored.packages[0].langpack_restored);
    assert_eq!(fs::read_to_string(&ini).unwrap(), chosen);
    assert!(at(&fixture, "LangPack/de_DE.ReaperLangPack").is_file());
}
