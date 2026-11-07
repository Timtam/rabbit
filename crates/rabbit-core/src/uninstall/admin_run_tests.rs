//! The macOS engine end to end, on every platform: a run whose first change
//! is the one administrator command, with REAPER.app going to the Trash
//! last. A temporary folder stands in for `/`, another for the user's
//! Trash; the administrator command is a recording stand-in that carries
//! root's steps out there, and macOS's answers come from a fake. Nothing
//! here asks for a password, touches a real app, the real Trash or a real
//! receipt.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::admin_batch::{
    AdministratorStepKind, AdministratorStepVerdict, MV, PGREP, PKGUTIL, RM,
    RecordingAdministratorShell, TEST, prepare_package,
};
use super::execute::{UninstallEnvironment, english_message, execute_uninstall_in};
use super::macos_plan_tests::{
    MacFixture, OneVolume, REAPER_ID, extension, join, link_folder, mac_spec, my_trash,
    owned_by_me, owned_by_root, reaper_spec, snapshot,
};
use super::preflight::ANY_REAPER_PROCESS_CHECK;
use super::tests::{installed, running};
use super::{
    ByHandReason, FakeMacosObjects, NoElevatedHelper, NothingListed, ObjectFacts, PathRefusal,
    Removability, RemovalReason, TrashErrorKind, TrashFolderFacts, UninstallMessage, UninstallPlan,
    UninstallProblem, UninstallReport, UninstallReportItem, UninstallRequest, UninstallStatus,
};
use crate::cancel::CancelToken;
use crate::error::RabbitError;
use crate::model::{InstallationKind, Platform};
use crate::package::{PACKAGE_REAPER, PackageSpec};
use crate::preflight::{PreflightStatus, RunningProcess};
use crate::progress::{ProgressEvent, ProgressReporter, RemovalMethod};
use crate::receipt::{InstallState, PackageReceipt, load_install_state, save_install_state};

const SURGE: &str = "surge";
const EXT: &str = "ext";
const APP_RECEIPT: &str = "org.example.surge.app.pkg";
const VST3_RECEIPT: &str = "org.example.surge.vst3.pkg";
const OTHER_RECEIPT: &str = "com.apple.pkg.Other";

/// Surge XT's trees, below the stand-in for `/`, in the order its plan
/// removes them: the apps first, the VST3 that detection finds it by last.
const SURGE_TREES: [&str; 5] = [
    "Applications/Surge.app",
    "Applications/Surge Effects.app",
    "Library/Audio/Plug-Ins/Components/Surge.component",
    "Library/Application Support/Surge",
    "Library/Audio/Plug-Ins/VST3/Surge.vst3",
];

/// A package shaped like Surge XT's macOS plan, smaller.
fn surge_xt() -> PackageSpec {
    mac_spec(
        SURGE,
        r#""requires_standard_install": true,"#,
        r#""elevation": "required", "backup": "none",
           "blocking_programs": ["Surge", "Surge Effects"],
           "remove": { "trees": [
               { "path": "/Applications/Surge.app", "must_end_with": ".app", "name_contains": "Surge" },
               { "path": "/Applications/Surge Effects.app", "must_end_with": ".app", "name_contains": "Surge" },
               { "path": "/Library/Audio/Plug-Ins/Components/Surge.component", "must_end_with": ".component", "name_contains": "Surge" },
               { "path": "/Library/Application Support/Surge", "name_contains": "Surge" },
               { "path": "/Library/Audio/Plug-Ins/VST3/Surge.vst3", "must_end_with": ".vst3", "name_contains": "Surge" }
           ] },
           "macos_pkg_forget": ["org.example.surge.app.pkg", "org.example.surge.vst3.pkg"],
           "post": [{ "refresh_audio_units": {} }]"#,
    )
}

/// A fixture with Surge's trees, the extension's plug-in, REAPER.app, a
/// folder standing in for the user's Trash, and a receipt listing all three.
struct Mac {
    fixture: MacFixture,
    trash: PathBuf,
}

impl Mac {
    fn new() -> Self {
        let fixture = MacFixture::new();
        for tree in SURGE_TREES {
            super::tests::write(
                &join(&join(&fixture.system, tree), "Contents/data"),
                b"surge",
            );
        }
        super::tests::write(
            &fixture
                .resource
                .join("UserPlugins")
                .join("reaper_ext.dylib"),
            b"ext",
        );
        let trash = fixture.dir.path().join("Trash");
        fs::create_dir(&trash).unwrap();
        let mut state = InstallState::default();
        for id in [SURGE, EXT, PACKAGE_REAPER] {
            state.packages.insert(id.to_string(), receipt_entry(id));
        }
        save_install_state(&fixture.resource, &state).unwrap();
        Self { fixture, trash }
    }

    fn surge_tree(&self, index: usize) -> PathBuf {
        join(&self.fixture.system, SURGE_TREES[index])
    }

    fn surge_trees(&self) -> Vec<PathBuf> {
        (0..SURGE_TREES.len())
            .map(|index| self.surge_tree(index))
            .collect()
    }

    fn plug_in(&self) -> PathBuf {
        self.fixture
            .resource
            .join("UserPlugins")
            .join("reaper_ext.dylib")
    }

    fn app(&self) -> PathBuf {
        self.fixture.app.clone()
    }

    /// What macOS says: REAPER.app described by `app`, the user's Trash a
    /// real folder of theirs that moves go into, and the installer listing
    /// Surge's two receipts and another.
    fn objects(&self, app: ObjectFacts) -> FakeMacosObjects {
        FakeMacosObjects::new()
            .with_object(&self.fixture.app, app)
            .with_trash(TrashFolderFacts {
                path: self.trash.clone(),
                ..my_trash()
            })
            .with_trash_bin(&self.trash)
            .with_receipts([APP_RECEIPT, VST3_RECEIPT, OTHER_RECEIPT])
    }

    /// The packages detection finds now, by the file each is found by.
    fn installed_now(&self) -> Vec<&'static str> {
        let mut ids = Vec::new();
        if self.surge_tree(4).exists() {
            ids.push(SURGE);
        }
        if self.plug_in().exists() {
            ids.push(EXT);
        }
        if self.app().exists() {
            ids.push(PACKAGE_REAPER);
        }
        ids
    }

    /// Everything a run may change: the stand-in for `/`, the Trash, the
    /// plug-ins and the receipt.
    fn state(&self) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        let mut found = snapshot(&self.fixture.system);
        found.extend(snapshot(&self.trash));
        found.extend(snapshot(&self.fixture.resource.join("UserPlugins")));
        let receipt = crate::receipt::receipt_path(&self.fixture.resource);
        found.insert(receipt.clone(), fs::read(&receipt).ok());
        found
    }

    fn receipt(&self) -> InstallState {
        load_install_state(&self.fixture.resource).unwrap().unwrap()
    }
}

fn receipt_entry(id: &str) -> PackageReceipt {
    PackageReceipt {
        id: id.to_string(),
        version: None,
        source_url: None,
        source_sha256: None,
        variant: None,
        channel: None,
        installed_files: Vec::new(),
        installed_at: None,
        rabbit_version: None,
        architecture: None,
        vendor_added_files: Vec::new(),
    }
}

/// One engine run on a [`Mac`], with everything it asks the computer
/// stood in.
struct Run<'m> {
    mac: &'m Mac,
    specs: Vec<PackageSpec>,
    objects: FakeMacosObjects,
    shell: RecordingAdministratorShell,
    /// The programs running at each scan for named programs, in turn; the
    /// last answer repeats.
    programs: Vec<Vec<RunningProcess>>,
    /// Every REAPER running, at every scan.
    reapers: Vec<RunningProcess>,
    /// Called with each scan's number right before it answers: 0 at the
    /// preflight, after the plan; 1 right before the password window, in
    /// a run with an administrator command. Changes things between the
    /// plan and the run.
    before_scan: Option<Box<dyn Fn(usize)>>,
    remove_settings: bool,
    platform: Platform,
    events: Arc<Mutex<Vec<ProgressEvent>>>,
    cancel: CancelToken,
}

impl<'m> Run<'m> {
    fn new(mac: &'m Mac, specs: Vec<PackageSpec>, objects: FakeMacosObjects) -> Self {
        let shell = RecordingAdministratorShell::new().forgetting_in(objects.receipt_list());
        Self {
            mac,
            specs,
            objects,
            shell,
            programs: vec![Vec::new()],
            reapers: Vec::new(),
            before_scan: None,
            remove_settings: false,
            platform: Platform::MacOs,
            events: Arc::default(),
            cancel: CancelToken::new(),
        }
    }

    /// "Also remove settings" is ticked.
    fn removing_settings(mut self) -> Self {
        self.remove_settings = true;
        self
    }

    /// `change` runs right before scan `at` answers.
    fn changing_before_scan(mut self, at: usize, change: impl Fn() + 'static) -> Self {
        self.before_scan = Some(Box::new(move |scan| {
            if scan == at {
                change();
            }
        }));
        self
    }

    /// The shell, set up as `configure` says, still forgetting receipts
    /// in the fake's list.
    fn with_shell(
        mut self,
        configure: impl FnOnce(RecordingAdministratorShell) -> RecordingAdministratorShell,
    ) -> Self {
        self.shell = configure(
            RecordingAdministratorShell::new().forgetting_in(self.objects.receipt_list()),
        );
        self
    }

    fn try_run(&self, ids: &[&str], dry_run: bool) -> crate::error::Result<UninstallReport> {
        let fixture = &self.mac.fixture;
        let detect = || Ok(installed(&self.mac.installed_now()));
        let reapers = || self.reapers.clone();
        let scans = Cell::new(0);
        let named = |_: &[String]| {
            let scan = scans.get();
            scans.set(scan + 1);
            if let Some(before_scan) = &self.before_scan {
                before_scan(scan);
            }
            self.programs[scan.min(self.programs.len() - 1)].clone()
        };
        let sink = Arc::clone(&self.events);
        let progress = ProgressReporter::new(move |event| sink.lock().unwrap().push(event));
        execute_uninstall_in(
            &UninstallRequest {
                resource_path: fixture.resource.clone(),
                platform: self.platform,
                package_ids: ids.iter().map(ToString::to_string).collect(),
                remove_settings: self.remove_settings,
                dry_run,
                target_app_path: Some(fixture.app.clone()),
                target: Some(InstallationKind::Standard),
            },
            &UninstallEnvironment {
                specs: &self.specs,
                roots: &fixture.roots,
                target: InstallationKind::Standard,
                target_app_path: Some(fixture.app.clone()),
                detect: &detect,
                reaper_processes: &reapers,
                named_processes: &named,
                volumes: &OneVolume,
                installed_programs: None,
                vendor_lookup: &NothingListed,
                elevated_helper: &NoElevatedHelper,
                macos_objects: &self.objects,
                administrator_shell: &self.shell,
            },
            &progress,
            &self.cancel,
        )
    }

    fn run(&self, ids: &[&str]) -> UninstallReport {
        self.try_run(ids, false).unwrap()
    }

    fn events(&self) -> Vec<ProgressEvent> {
        self.events.lock().unwrap().clone()
    }
}

fn every_package() -> Vec<PackageSpec> {
    vec![reaper_spec("never"), extension(), surge_xt()]
}

const EVERY_ID: [&str; 3] = [PACKAGE_REAPER, EXT, SURGE];

fn item<'r>(report: &'r UninstallReport, id: &str) -> &'r UninstallReportItem {
    report
        .item(id)
        .unwrap_or_else(|| panic!("no item for {id}: {report:#?}"))
}

fn statuses(report: &UninstallReport) -> Vec<(&str, UninstallStatus)> {
    report
        .items
        .iter()
        .map(|item| (item.package_id.as_str(), item.status))
        .collect()
}

fn started(id: &str, method: RemovalMethod) -> ProgressEvent {
    ProgressEvent::RemoveStarted {
        package_id: id.to_string(),
        method,
    }
}

fn completed(id: &str, removed: bool) -> ProgressEvent {
    ProgressEvent::RemoveCompleted {
        package_id: id.to_string(),
        removed,
    }
}

fn prompt(ids: &[&str]) -> ProgressEvent {
    ProgressEvent::AdministratorPrompt {
        package_ids: ids.iter().map(ToString::to_string).collect(),
    }
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// Everything goes in the run's order: root's command first, for Surge;
/// then the extension into the backup set; REAPER.app to the Trash last,
/// as the user. One password window for the whole run.
#[test]
fn surge_goes_first_through_the_administrator_command_and_reaper_last_to_the_trash() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()));

    let report = run.run(&EVERY_ID);

    assert_eq!(
        statuses(&report),
        [
            (SURGE, UninstallStatus::Removed),
            (EXT, UninstallStatus::Removed),
            (PACKAGE_REAPER, UninstallStatus::Removed),
        ]
    );
    let surge = item(&report, SURGE);
    assert_eq!(surge.message_code, UninstallMessage::RemovedAsAdministrator);
    assert_eq!(
        surge
            .removed
            .iter()
            .map(|file| (file.path.clone(), file.reason, file.backup_path.clone()))
            .collect::<Vec<_>>(),
        mac.surge_trees()
            .into_iter()
            .map(|path| (path, RemovalReason::Tree, None))
            .collect::<Vec<_>>()
    );
    assert!(surge.problems.is_empty(), "{:?}", surge.problems);
    assert!(
        surge.message.contains("with administrator rights"),
        "{}",
        surge.message
    );
    assert!(mac.surge_trees().iter().all(|tree| !tree.exists()));

    let ext = item(&report, EXT);
    assert!(matches!(
        &ext.message_code,
        UninstallMessage::Removed {
            backup_set_id: Some(_)
        }
    ));
    assert!(!mac.plug_in().exists());

    let reaper = item(&report, PACKAGE_REAPER);
    let trashed = mac.trash.join("REAPER.app");
    assert_eq!(
        reaper.message_code,
        UninstallMessage::MovedToTrash {
            trashed_to: Some(trashed.clone())
        }
    );
    assert_eq!(reaper.removed[0].trashed_to, Some(trashed.clone()));
    assert!(!mac.app().exists());
    assert!(join(&trashed, "Contents/MacOS/REAPER").is_file());
    assert!(
        reaper.message.contains("moved reaper to the Trash")
            && reaper.message.contains("drag it back")
            && !reaper.message.contains("administrator"),
        "{}",
        reaper.message
    );
    assert_eq!(run.objects.trashed(), [mac.app()]);

    // One prompt, with exactly these steps.
    assert_eq!(run.shell.calls(), 1);
    let steps: Vec<(String, Vec<String>)> = run
        .shell
        .steps()
        .iter()
        .map(|step| {
            let command = step.command();
            (
                command.program.display().to_string(),
                command.arguments.clone(),
            )
        })
        .collect();
    let mut expected = vec![(
        PGREP.to_string(),
        ["-x", "-i", "--", "Surge|Surge Effects|REAPER|reaper64"]
            .map(String::from)
            .to_vec(),
    )];
    for tree in mac.surge_trees() {
        expected.push((
            RM.to_string(),
            ["-R", "-f", "-x", text(&tree)].map(String::from).to_vec(),
        ));
    }
    for id in [APP_RECEIPT, VST3_RECEIPT] {
        expected.push((
            PKGUTIL.to_string(),
            ["--forget", id].map(String::from).to_vec(),
        ));
    }
    assert_eq!(steps, expected);
    assert_eq!(
        report
            .administrator_steps
            .iter()
            .map(|record| (record.kind, record.verdict))
            .collect::<Vec<_>>(),
        [AdministratorStepKind::ProgramsClosed]
            .into_iter()
            .chain([AdministratorStepKind::Delete; 5])
            .chain([AdministratorStepKind::Forget; 2])
            .map(|kind| (kind, AdministratorStepVerdict::Ran))
            .collect::<Vec<_>>()
    );
    // The receipts root forgot are gone; the unrelated one stays.
    assert_eq!(
        run.objects
            .receipts()
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        [OTHER_RECEIPT]
    );
    assert_eq!(run.objects.audio_unit_refreshes(), 1);

    assert_eq!(
        run.events(),
        [
            prompt(&[SURGE]),
            started(SURGE, RemovalMethod::Administrator),
            completed(SURGE, true),
            started(EXT, RemovalMethod::Files),
            completed(EXT, true),
            started(PACKAGE_REAPER, RemovalMethod::Trash),
            completed(PACKAGE_REAPER, true),
        ]
    );
    assert!(!report.has_failures());
}

/// The administrator command takes Surge's entry out of the receipt the
/// run holds; the extension's removal saves that same receipt later, so
/// the entry doesn't come back.
#[test]
fn the_receipt_follows_detection_and_a_later_save_doesnt_bring_surge_back() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()));

    run.run(&[SURGE, EXT]);

    let receipt = mac.receipt();
    assert!(!receipt.packages.contains_key(SURGE));
    assert!(!receipt.packages.contains_key(EXT));
    assert!(receipt.packages.contains_key(PACKAGE_REAPER));
    assert!(receipt.declined_packages.contains(SURGE));
    assert!(receipt.declined_packages.contains(EXT));
}

/// Cancelling the password window cancels the whole run before anything
/// changes: not a byte is different afterwards.
#[test]
fn a_cancelled_password_window_cancels_everything_and_changes_nothing() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(RecordingAdministratorShell::declining);
    let before = mac.state();

    let report = run.run(&EVERY_ID);

    for item in &report.items {
        assert_eq!(
            item.status,
            UninstallStatus::Cancelled,
            "{}",
            item.package_id
        );
        assert_eq!(item.message_code, UninstallMessage::ElevationDeclined);
        assert!(
            item.message
                .contains("administrator password prompt was cancelled"),
            "{}",
            item.message
        );
    }
    assert_eq!(mac.state(), before);
    assert!(report.elevation_declined() && !report.changed_anything());
    assert_eq!(report.backup_set_id, None);
    assert!(run.objects.trashed().is_empty());
    assert_eq!(run.shell.calls(), 1);
    assert_eq!(run.events(), [prompt(&[SURGE])]);
    assert!(
        report
            .administrator_steps
            .iter()
            .all(|record| record.verdict == AdministratorStepVerdict::NotRun)
    );
}

/// A password window that failed, or a command RABBIT couldn't start,
/// changes nothing: Surge failed, the rest weren't reached.
#[test]
fn an_administrator_command_that_never_ran_changes_nothing() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.not_run("a wrong password three times"));
    let before = mac.state();

    let report = run.run(&EVERY_ID);

    assert_eq!(
        item(&report, SURGE).message_code,
        UninstallMessage::ElevationFailed {
            error: "a wrong password three times".to_string()
        }
    );
    assert_eq!(item(&report, SURGE).status, UninstallStatus::Failed);
    for id in [EXT, PACKAGE_REAPER] {
        assert_eq!(item(&report, id).status, UninstallStatus::NotReached);
        assert_eq!(
            item(&report, id).message_code,
            UninstallMessage::RunStopped {
                package_id: SURGE.to_string()
            }
        );
    }
    assert!(
        !item(&report, SURGE).message.contains("back in place"),
        "{}",
        item(&report, SURGE).message
    );
    assert_eq!(mac.state(), before);
    assert!(report.has_failures() && !report.changed_anything());
}

/// App Management refuses root's rm of Surge's second app: the first is
/// gone, the plug-ins are whole, the VST3 Surge is detected by is still
/// there, and nothing else of the run happens.
#[test]
fn a_refused_step_leaves_surge_partly_removed_and_stops_the_run_there() {
    let mac = Mac::new();
    let refusal = "rm: /Applications/Surge Effects.app: Operation not permitted";
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.failing_at(2).saying(refusal));

    let report = run.run(&EVERY_ID);

    let surge = item(&report, SURGE);
    assert_eq!(surge.status, UninstallStatus::Failed);
    assert_eq!(
        surge.message_code,
        UninstallMessage::AdministratorStepFailed {
            path: Some(mac.surge_tree(1)),
            gone: vec![mac.surge_tree(0)],
            left: mac.surge_trees()[1..].to_vec(),
            refused_by_macos: true,
            detail: Some(refusal.to_string()),
        }
    );
    for words in [
        "Removed:",
        "Still there:",
        "App Management",
        "RABBIT stopped there",
        // macOS's words end as a sentence before RABBIT's next one.
        "Operation not permitted. If macOS refused it",
    ] {
        assert!(surge.message.contains(words), "{words}: {}", surge.message);
    }
    assert!(
        !surge.message.contains("back in place"),
        "{}",
        surge.message
    );
    assert!(!mac.surge_tree(0).exists());
    assert!(mac.surge_trees()[1..].iter().all(|tree| tree.exists()));
    for id in [EXT, PACKAGE_REAPER] {
        assert_eq!(item(&report, id).status, UninstallStatus::NotReached);
        assert_eq!(
            item(&report, id).message_code,
            UninstallMessage::RunStopped {
                package_id: SURGE.to_string()
            }
        );
    }
    assert!(mac.plug_in().exists() && mac.app().exists());
    assert!(run.objects.trashed().is_empty());
    assert_eq!(report.backup_set_id, None);
    // Detection still finds Surge by its VST3, so its entry stays, and
    // RABBIT can offer the uninstall again.
    let receipt = mac.receipt();
    assert!(receipt.packages.contains_key(SURGE));
    assert!(!receipt.declined_packages.contains(SURGE));
    assert_eq!(
        run.events(),
        [
            prompt(&[SURGE]),
            started(SURGE, RemovalMethod::Administrator),
            completed(SURGE, false),
        ]
    );
    assert_eq!(
        report
            .administrator_steps
            .iter()
            .map(|record| record.verdict)
            .take(4)
            .collect::<Vec<_>>(),
        [
            AdministratorStepVerdict::Ran,
            AdministratorStepVerdict::Ran,
            AdministratorStepVerdict::Failed,
            AdministratorStepVerdict::NotRun,
        ]
    );
    assert!(report.has_failures());
}

#[test]
fn a_refusal_at_surges_first_step_removes_nothing_of_it() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.failing_at(1));
    let before = mac.state();

    let report = run.run(&[SURGE]);

    let surge = item(&report, SURGE);
    assert!(matches!(
        &surge.message_code,
        UninstallMessage::AdministratorStepFailed { path: Some(path), gone, .. }
            if *path == mac.surge_tree(0) && gone.is_empty()
    ));
    assert!(
        surge.message.contains("Nothing of surge was removed"),
        "{}",
        surge.message
    );
    assert_eq!(mac.state(), before);
}

/// Surge's standalone app started while the password window was open:
/// root's first step finds it, nothing runs, and the check RABBIT runs
/// again as the user names it.
#[test]
fn a_program_started_during_the_prompt_blocks_surge_with_nothing_changed() {
    let mac = Mac::new();
    let mut run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.failing_at(0));
    let surge_app = join(&mac.surge_tree(0), "Contents/MacOS/Surge");
    // Not at the preflight, not right before the prompt; then there.
    run.programs = vec![
        Vec::new(),
        Vec::new(),
        vec![running("4242", "Surge", &surge_app)],
    ];
    let before = mac.state();

    let report = run.run(&EVERY_ID);

    let surge = item(&report, SURGE);
    assert_eq!(surge.status, UninstallStatus::Blocked);
    let UninstallMessage::ChecksFailed { checks } = &surge.message_code else {
        panic!("{:?}", surge.message_code);
    };
    assert!(
        checks
            .iter()
            .any(|check| check.status == PreflightStatus::Fail
                && check.message.contains("Surge (4242)")),
        "{checks:?}"
    );
    for id in [EXT, PACKAGE_REAPER] {
        assert_eq!(
            item(&report, id).message_code,
            UninstallMessage::OtherPackageBlocked {
                package_ids: vec![SURGE.to_string()]
            }
        );
    }
    assert_eq!(mac.state(), before);
    assert!(report.blocked_before_changes());
    // Nothing started, so nothing ends either.
    assert_eq!(run.events(), [prompt(&[SURGE])]);
}

/// The program had quit again by the time RABBIT looked as the user: the
/// check names what root looked for.
#[test]
fn a_program_root_found_but_rabbit_no_longer_sees_still_blocks_surge() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.failing_at(0));

    let report = run.run(&[SURGE]);

    let UninstallMessage::ChecksFailed { checks } = &item(&report, SURGE).message_code else {
        panic!("{:?}", item(&report, SURGE).message_code);
    };
    assert!(
        checks[0]
            .message
            .contains("Surge, Surge Effects, REAPER, reaper64"),
        "{checks:?}"
    );
    assert!(mac.surge_trees().iter().all(|tree| tree.exists()));
}

/// A command that ended without saying where, after doing everything:
/// what is gone decides, and the report says how it ended.
#[test]
fn a_command_that_ended_unclearly_is_judged_by_what_is_gone() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.stopping(Some(255)));

    let report = run.run(&EVERY_ID);

    let surge = item(&report, SURGE);
    assert_eq!(surge.status, UninstallStatus::Removed);
    assert_eq!(
        surge.problems,
        [UninstallProblem::AdministratorEndedUnclearly { status: Some(255) }]
    );
    assert!(
        surge.message.contains("ended with status 255"),
        "{}",
        surge.message
    );
    assert_eq!(
        item(&report, PACKAGE_REAPER).status,
        UninstallStatus::Removed
    );
    assert!(
        report
            .administrator_steps
            .iter()
            .all(|record| record.verdict == AdministratorStepVerdict::Unknown)
    );
}

#[test]
fn receipts_macos_still_lists_are_reported_and_unlistable_ones_are_all_sent() {
    // The command ran, but pkgutil forgot nothing.
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|_| RecordingAdministratorShell::new());
    let report = run.run(&[SURGE]);
    let surge = item(&report, SURGE);
    assert_eq!(surge.status, UninstallStatus::Removed);
    assert_eq!(
        surge.problems,
        [UninstallProblem::InstallerRecordsRemain {
            ids: vec![APP_RECEIPT.to_string(), VST3_RECEIPT.to_string()]
        }]
    );
    assert!(surge.message.contains("does no harm"), "{}", surge.message);

    // Nothing listed Surge's receipts: none is sent.
    let mac = Mac::new();
    let objects = mac.objects(owned_by_me()).with_receipts([OTHER_RECEIPT]);
    let run = Run::new(&mac, every_package(), objects);
    let report = run.run(&[SURGE]);
    assert!(
        run.shell
            .steps()
            .iter()
            .all(|step| step.command().program != Path::new(PKGUTIL))
    );
    assert!(item(&report, SURGE).problems.is_empty());

    // Listing failed: every receipt the manifest names is sent, and that
    // nothing could be checked afterwards is said.
    let mac = Mac::new();
    let objects = mac.objects(owned_by_me()).with_unlistable_receipts();
    let run = Run::new(&mac, every_package(), objects);
    let report = run.run(&[SURGE]);
    let forgets: Vec<String> = run
        .shell
        .steps()
        .iter()
        .filter(|step| step.command().program == Path::new(PKGUTIL))
        .map(|step| step.command().arguments[1].clone())
        .collect();
    assert_eq!(forgets, [APP_RECEIPT, VST3_RECEIPT]);
    let surge = item(&report, SURGE);
    assert_eq!(surge.status, UninstallStatus::Removed);
    assert!(matches!(
        surge.problems.as_slice(),
        [UninstallProblem::InstallerRecordsUnchecked { .. }]
    ));
}

/// A REAPER.app only root can move goes into the user's Trash through the
/// administrator command, under a fresh name of RABBIT's, after root
/// checked the Trash is a real folder.
#[test]
fn root_moves_a_reaper_app_only_root_can_move_into_the_users_trash() {
    let mac = Mac::new();
    let run = Run::new(
        &mac,
        vec![reaper_spec("if_needed")],
        mac.objects(owned_by_root()),
    );

    let report = run.run(&[PACKAGE_REAPER]);

    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(
        reaper.status,
        UninstallStatus::Removed,
        "{}",
        reaper.message
    );
    let in_trash: Vec<PathBuf> = fs::read_dir(&mac.trash)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    let [moved] = in_trash.as_slice() else {
        panic!("one app in the Trash: {in_trash:?}");
    };
    let name = moved.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("REAPER 20") && name.ends_with(".app") && name.len() == 30,
        "{name}"
    );
    assert!(join(moved, "Contents/MacOS/REAPER").is_file());
    assert!(!mac.app().exists());
    assert_eq!(
        reaper.message_code,
        UninstallMessage::MovedToTrash {
            trashed_to: Some(moved.clone())
        }
    );
    assert!(
        reaper
            .message
            .contains("emptying the Trash may ask for an administrator's password"),
        "{}",
        reaper.message
    );
    // Root moved it; RABBIT never asked macOS's Trash function.
    assert!(run.objects.trashed().is_empty());

    let steps: Vec<(String, Vec<String>)> = run
        .shell
        .steps()
        .iter()
        .map(|step| {
            (
                step.command().program.display().to_string(),
                step.command().arguments.clone(),
            )
        })
        .collect();
    let trash = text(&mac.trash).to_string();
    assert_eq!(
        steps,
        [
            (
                PGREP.to_string(),
                ["-x", "-i", "--", "REAPER|reaper64"]
                    .map(String::from)
                    .to_vec()
            ),
            (TEST.to_string(), vec!["-d".to_string(), trash.clone()]),
            (
                TEST.to_string(),
                vec!["!".to_string(), "-L".to_string(), trash]
            ),
            (
                MV.to_string(),
                vec![
                    "-h".to_string(),
                    "-n".to_string(),
                    text(&mac.app()).to_string(),
                    text(moved).to_string(),
                ]
            ),
        ]
    );
    assert_eq!(
        run.events(),
        [
            prompt(&[PACKAGE_REAPER]),
            started(PACKAGE_REAPER, RemovalMethod::Administrator),
            completed(PACKAGE_REAPER, true),
        ]
    );
    assert!(!mac.receipt().packages.contains_key(PACKAGE_REAPER));
}

/// The user's Trash folder became a link after RABBIT checked it: root's
/// own check stops the command before anything moves.
#[test]
fn root_moves_nothing_into_a_trash_folder_that_became_a_link() {
    let mac = Mac::new();
    let elsewhere = mac.fixture.dir.path().join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    fs::remove_dir(&mac.trash).unwrap();
    link_folder(&mac.trash, &elsewhere);
    let run = Run::new(
        &mac,
        vec![reaper_spec("if_needed")],
        mac.objects(owned_by_root()),
    );

    let report = run.run(&[PACKAGE_REAPER]);

    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(reaper.status, UninstallStatus::Failed);
    assert_eq!(
        reaper.message_code,
        UninstallMessage::AdministratorStepFailed {
            path: Some(mac.trash.clone()),
            gone: Vec::new(),
            left: vec![mac.app()],
            refused_by_macos: false,
            detail: None,
        }
    );
    assert!(join(&mac.app(), "Contents/MacOS/REAPER").is_file());
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
    assert_eq!(
        report
            .administrator_steps
            .iter()
            .map(|record| record.verdict)
            .collect::<Vec<_>>(),
        [
            AdministratorStepVerdict::Ran,
            AdministratorStepVerdict::Ran,
            AdministratorStepVerdict::Failed,
            AdministratorStepVerdict::NotRun,
        ]
    );
    assert!(mac.receipt().packages.contains_key(PACKAGE_REAPER));
}

/// With REAPER in the administrator command, the command's packages come
/// first in the report, REAPER among them, before the extension.
#[test]
fn reaper_joins_the_administrator_packages_at_the_front_when_only_root_can_move_it() {
    let mac = Mac::new();
    let run = Run::new(
        &mac,
        vec![reaper_spec("if_needed"), extension(), surge_xt()],
        mac.objects(owned_by_root()),
    );

    let report = run.run(&EVERY_ID);

    assert_eq!(
        statuses(&report),
        [
            (SURGE, UninstallStatus::Removed),
            (PACKAGE_REAPER, UninstallStatus::Removed),
            (EXT, UninstallStatus::Removed),
        ]
    );
    assert_eq!(run.shell.calls(), 1);
    assert_eq!(run.events()[0], prompt(&[SURGE, PACKAGE_REAPER]));
}

/// macOS refuses the Trash move: REAPER failed with nothing of it changed,
/// and what was removed before it stays removed.
#[test]
fn a_trash_move_macos_refuses_fails_reaper_with_nothing_of_it_changed() {
    let mac = Mac::new();
    let objects = mac
        .objects(owned_by_me())
        .failing_to_trash(TrashErrorKind::PermissionDenied, "Operation not permitted");
    let run = Run::new(&mac, vec![reaper_spec("never"), extension()], objects);

    let report = run.run(&[PACKAGE_REAPER, EXT]);

    assert_eq!(item(&report, EXT).status, UninstallStatus::Removed);
    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(reaper.status, UninstallStatus::Failed);
    assert_eq!(
        reaper.message_code,
        UninstallMessage::TrashFailed {
            error: "Operation not permitted".to_string(),
            refused_by_macos: true,
        }
    );
    for words in ["nothing of it changed", "App Management"] {
        assert!(
            reaper.message.contains(words),
            "{words}: {}",
            reaper.message
        );
    }
    assert!(
        !reaper.message.contains("back in place"),
        "{}",
        reaper.message
    );
    assert!(join(&mac.app(), "Contents/MacOS/REAPER").is_file());
    assert!(mac.receipt().packages.contains_key(PACKAGE_REAPER));
    // No password was asked for at all.
    assert_eq!(run.shell.calls(), 0);
}

/// macOS said the app moved, but it is still in its place: RABBIT says it
/// can't confirm the move. And an app that left with no word of where it
/// went is unclear too, though detection no longer finds it.
#[test]
fn a_trash_move_rabbit_cant_confirm_is_unclear() {
    let mac = Mac::new();
    let objects = mac
        .objects(owned_by_me())
        .claiming_to_trash_without_moving();
    let run = Run::new(&mac, vec![reaper_spec("never")], objects);
    let report = run.run(&[PACKAGE_REAPER]);
    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(reaper.status, UninstallStatus::Failed);
    assert_eq!(
        reaper.message_code,
        UninstallMessage::TrashUnclear { path: mac.app() }
    );
    assert!(
        reaper.message.contains("can't confirm"),
        "{}",
        reaper.message
    );
    assert!(mac.app().exists());
    assert!(mac.receipt().packages.contains_key(PACKAGE_REAPER));

    let mac = Mac::new();
    let objects = mac.objects(owned_by_me()).trashing_without_saying_where();
    let run = Run::new(&mac, vec![reaper_spec("never")], objects);
    let report = run.run(&[PACKAGE_REAPER]);
    assert_eq!(
        item(&report, PACKAGE_REAPER).message_code,
        UninstallMessage::TrashUnclear { path: mac.app() }
    );
    assert!(!mac.app().exists());
    // The receipt follows what detection finds: REAPER is gone.
    assert!(!mac.receipt().packages.contains_key(PACKAGE_REAPER));
}

/// The dry run records the exact steps root would be given, from the same
/// checks, and runs none of them: no prompt, no Trash, no change.
#[test]
fn a_dry_run_records_what_root_would_be_given_and_runs_nothing() {
    let mac = Mac::new();
    let run = Run::new(
        &mac,
        vec![reaper_spec("if_needed"), extension(), surge_xt()],
        mac.objects(owned_by_root()),
    );
    let before = mac.state();

    let report = run.try_run(&EVERY_ID, true).unwrap();

    assert!(
        report
            .items
            .iter()
            .all(|item| item.status == UninstallStatus::WouldRemove),
        "{:?}",
        statuses(&report)
    );
    use AdministratorStepKind as Kind;
    assert_eq!(
        report
            .administrator_steps
            .iter()
            .map(|record| (record.package_id.as_deref(), record.kind))
            .collect::<Vec<_>>(),
        [(None, Kind::ProgramsClosed)]
            .into_iter()
            .chain([(Some(SURGE), Kind::Delete); 5])
            .chain([(Some(SURGE), Kind::Forget); 2])
            .chain([
                (Some(PACKAGE_REAPER), Kind::TrashIsFolder),
                (Some(PACKAGE_REAPER), Kind::TrashIsNotLink),
                (Some(PACKAGE_REAPER), Kind::MoveToTrash),
            ])
            .collect::<Vec<_>>()
    );
    assert!(
        report
            .administrator_steps
            .iter()
            .all(|record| record.verdict == AdministratorStepVerdict::WouldRun)
    );
    assert_eq!(
        report.administrator_steps[1].target,
        text(&mac.surge_tree(0))
    );
    assert_eq!(run.shell.calls(), 0);
    assert!(run.objects.trashed().is_empty());
    assert_eq!(mac.state(), before);
    assert!(run.events().is_empty());
}

/// Decision 33: under sudo every check, the Trash and the backup set would
/// be root's, so a real macOS uninstall refuses before anything; a dry run
/// changes nothing and still says what would happen.
#[test]
fn a_real_macos_uninstall_refuses_to_run_as_root_but_a_dry_run_still_works() {
    let mac = Mac::new();
    let run = Run::new(
        &mac,
        every_package(),
        mac.objects(owned_by_me()).running_as_root(),
    );
    let before = mac.state();

    let error = run.try_run(&EVERY_ID, false).unwrap_err();

    assert!(matches!(error, RabbitError::UninstallAsRoot), "{error:?}");
    assert!(
        error
            .to_string()
            .contains("Run RABBIT as yourself, not with sudo"),
        "{error}"
    );
    assert_eq!(mac.state(), before);
    assert!(!mac.fixture.resource.join("RABBIT").join("logs").exists());
    // The refusal comes before the install lock, which would leave a
    // root-owned RABBIT/locks folder in the user's resource folder.
    assert!(!mac.fixture.resource.join("RABBIT").join("locks").exists());
    assert_eq!(run.shell.calls(), 0);

    let report = run.try_run(&EVERY_ID, true).unwrap();
    assert!(
        report
            .items
            .iter()
            .all(|item| item.status == UninstallStatus::WouldRemove)
    );
    assert_eq!(mac.state(), before);

    // The rule is macOS's: a Windows run is never refused by it.
    let mut windows = Run::new(
        &mac,
        every_package(),
        mac.objects(owned_by_me()).running_as_root(),
    );
    windows.platform = Platform::Windows;
    assert!(windows.try_run(&[EXT], false).is_ok());
}

/// A stop asked for before the run reached its first change: every
/// package is Cancelled, and no prompt appears.
#[test]
fn a_stop_before_the_first_change_cancels_everything_without_a_prompt() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()));
    run.cancel.cancel();
    let before = mac.state();

    let report = run.run(&EVERY_ID);

    assert!(
        report
            .items
            .iter()
            .all(|item| item.status == UninstallStatus::Cancelled),
        "{:?}",
        statuses(&report)
    );
    assert_eq!(run.shell.calls(), 0);
    assert_eq!(mac.state(), before);
}

/// Right before the prompt, the administrator guard runs again on every
/// path root is given: a plug-in folder that became a link since the plan
/// keeps root away, and so does a REAPER.app whose owner changed.
#[test]
fn the_guard_checks_roots_paths_again_right_before_the_prompt() {
    let mac = Mac::new();
    let objects = mac.objects(owned_by_root());
    let plan = mac.fixture.plan(
        &[reaper_spec("if_needed"), surge_xt()],
        &[PACKAGE_REAPER, SURGE],
        &objects,
        false,
    );
    let vst3 = join(&mac.fixture.system, "Library/Audio/Plug-Ins/VST3");
    let elsewhere = mac.fixture.dir.path().join("external VST3");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::rename(mac.surge_tree(4), elsewhere.join("Surge.vst3")).unwrap();
    fs::remove_dir(&vst3).unwrap();
    link_folder(&vst3, &elsewhere);

    let surge = plan.item(SURGE).unwrap();
    assert_eq!(
        prepare_package(
            surge,
            &mac.fixture.roots,
            &objects,
            std::time::SystemTime::now()
        ),
        Err(UninstallMessage::PathRefused {
            path: mac.surge_tree(4),
            refusal: PathRefusal::LinkedAncestor,
        })
    );

    // The user may move REAPER.app now: who may move it changed, and the
    // run doesn't guess.
    let reaper = plan.item(PACKAGE_REAPER).unwrap();
    assert!(reaper.needs_administrator());
    let now_mine = mac.objects(owned_by_me());
    assert!(matches!(
        prepare_package(
            reaper,
            &mac.fixture.roots,
            &now_mine,
            std::time::SystemTime::now()
        ),
        Err(UninstallMessage::NotRemovable {
            removability: Removability::ByHand {
                reason: ByHandReason::CouldNotCheck { .. },
                ..
            }
        })
    ));
}

/// Surge's standalone app started after the preflight, before the
/// administrator command: the check RABBIT runs again right before the
/// prompt blocks Surge, and no password window appears.
#[test]
fn a_program_started_before_the_prompt_blocks_surge_without_a_prompt() {
    let mac = Mac::new();
    let mut run = Run::new(&mac, every_package(), mac.objects(owned_by_me()));
    let surge_app = join(&mac.surge_tree(0), "Contents/MacOS/Surge");
    // Not at the preflight; right before the prompt, and from then on.
    run.programs = vec![Vec::new(), vec![running("4242", "Surge", &surge_app)]];
    let before = mac.state();

    let report = run.run(&EVERY_ID);

    let surge = item(&report, SURGE);
    assert_eq!(surge.status, UninstallStatus::Blocked);
    let UninstallMessage::ChecksFailed { checks } = &surge.message_code else {
        panic!("{:?}", surge.message_code);
    };
    assert!(
        checks
            .iter()
            .any(|check| check.status == PreflightStatus::Fail
                && check.message.contains("Surge (4242)")),
        "{checks:?}"
    );
    for id in [EXT, PACKAGE_REAPER] {
        assert_eq!(
            item(&report, id).message_code,
            UninstallMessage::OtherPackageBlocked {
                package_ids: vec![SURGE.to_string()]
            }
        );
    }
    assert_eq!(run.shell.calls(), 0);
    assert!(run.events().is_empty(), "{:?}", run.events());
    assert_eq!(mac.state(), before);
    assert!(report.blocked_before_changes());
}

/// A plug-in folder became a link after the preflight: the guard RABBIT
/// runs again right before the prompt keeps root away from it, and no
/// password window appears.
#[test]
fn a_folder_that_became_a_link_before_the_prompt_blocks_surge_without_a_prompt() {
    let mac = Mac::new();
    let vst3 = join(&mac.fixture.system, "Library/Audio/Plug-Ins/VST3");
    let elsewhere = mac.fixture.dir.path().join("external VST3");
    let surge_vst3 = mac.surge_tree(4);
    let run =
        Run::new(&mac, every_package(), mac.objects(owned_by_me())).changing_before_scan(1, {
            let (vst3, elsewhere) = (vst3.clone(), elsewhere.clone());
            move || {
                fs::create_dir_all(&elsewhere).unwrap();
                fs::rename(&surge_vst3, elsewhere.join("Surge.vst3")).unwrap();
                fs::remove_dir(&vst3).unwrap();
                link_folder(&vst3, &elsewhere);
            }
        });

    let report = run.run(&EVERY_ID);

    assert_eq!(
        item(&report, SURGE).message_code,
        UninstallMessage::PathRefused {
            path: mac.surge_tree(4),
            refusal: PathRefusal::LinkedAncestor,
        }
    );
    assert_eq!(item(&report, SURGE).status, UninstallStatus::Blocked);
    for id in [EXT, PACKAGE_REAPER] {
        assert_eq!(
            item(&report, id).message_code,
            UninstallMessage::OtherPackageBlocked {
                package_ids: vec![SURGE.to_string()]
            }
        );
    }
    assert_eq!(run.shell.calls(), 0);
    assert!(run.events().is_empty(), "{:?}", run.events());
    assert!(join(&elsewhere, "Surge.vst3/Contents/data").is_file());
    assert!(mac.surge_trees()[..4].iter().all(|tree| tree.exists()));
}

/// A password window left open is said once, between the prompt and the
/// removal, so a listener knows it may be behind RABBIT; a window then
/// cancelled says so too.
#[test]
fn a_password_window_left_open_is_reported_as_still_waiting() {
    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.still_waiting(30));
    run.run(&[SURGE]);
    assert_eq!(
        run.events(),
        [
            prompt(&[SURGE]),
            ProgressEvent::AdministratorStillWaiting { seconds: 30 },
            started(SURGE, RemovalMethod::Administrator),
            completed(SURGE, true),
        ]
    );

    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.still_waiting(30).declining());
    run.run(&[SURGE]);
    assert_eq!(
        run.events(),
        [
            prompt(&[SURGE]),
            ProgressEvent::AdministratorStillWaiting { seconds: 30 }
        ]
    );
}

/// A trailing slash on the app path, as shell completion types a folder,
/// makes `lstat` and `rename` follow a link on macOS: REAPER.app a link
/// to an app elsewhere is never moved, nor what it points to.
#[test]
fn a_trailing_slash_on_the_app_path_never_lets_root_move_a_links_target() {
    let mut mac = Mac::new();
    let real = mac.fixture.dir.path().join("elsewhere").join("REAPER.app");
    fs::create_dir_all(real.parent().unwrap()).unwrap();
    fs::rename(mac.app(), &real).unwrap();
    link_folder(&mac.app(), &real);
    let mut spelt = mac.app().into_os_string();
    spelt.push(std::path::MAIN_SEPARATOR_STR);
    mac.fixture.roots.app_path = Some(PathBuf::from(spelt));
    let run = Run::new(
        &mac,
        vec![reaper_spec("if_needed")],
        mac.objects(owned_by_root()),
    );

    let report = run.run(&[PACKAGE_REAPER]);

    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(
        reaper.status,
        UninstallStatus::Blocked,
        "{}",
        reaper.message
    );
    assert_eq!(run.shell.calls(), 0);
    assert!(
        run.shell
            .steps()
            .iter()
            .all(|step| step.command().program != Path::new(MV))
    );
    assert!(join(&real, "Contents/MacOS/REAPER").is_file());
    assert_eq!(fs::read_dir(&mac.trash).unwrap().count(), 0);
    assert!(run.events().is_empty(), "{:?}", run.events());
}

/// REAPER.app was deleted after the plan was built: root has nothing left
/// to move, so no password window appears, and nothing is said to have
/// gone to the Trash or with administrator rights.
#[test]
fn an_app_gone_before_roots_turn_needs_no_password_and_isnt_reported_as_moved() {
    let mac = Mac::new();
    let app = mac.app();
    let run = Run::new(
        &mac,
        vec![reaper_spec("if_needed")],
        mac.objects(owned_by_root()),
    )
    .changing_before_scan(0, move || fs::remove_dir_all(&app).unwrap());

    let report = run.run(&[PACKAGE_REAPER]);

    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(
        reaper.status,
        UninstallStatus::Removed,
        "{}",
        reaper.message
    );
    assert_eq!(
        reaper.message_code,
        UninstallMessage::Removed {
            backup_set_id: None
        }
    );
    assert!(reaper.removed.is_empty());
    assert!(
        !reaper.message.contains("Trash") && !reaper.message.contains("administrator"),
        "{}",
        reaper.message
    );
    assert_eq!(run.shell.calls(), 0);
    assert!(
        !run.events()
            .iter()
            .any(|event| matches!(event, ProgressEvent::AdministratorPrompt { .. })),
        "{:?}",
        run.events()
    );
    assert!(report.administrator_steps.is_empty());
    assert_eq!(fs::read_dir(&mac.trash).unwrap().count(), 0);
    assert!(!mac.receipt().packages.contains_key(PACKAGE_REAPER));
}

/// The same as the user: an app gone since the plan isn't moved, and isn't
/// said to be in the Trash.
#[test]
fn an_app_gone_before_its_turn_isnt_reported_as_moved_to_the_trash() {
    let mac = Mac::new();
    let app = mac.app();
    let run = Run::new(&mac, vec![reaper_spec("never")], mac.objects(owned_by_me()))
        .changing_before_scan(0, move || fs::remove_dir_all(&app).unwrap());

    let report = run.run(&[PACKAGE_REAPER]);

    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(
        reaper.status,
        UninstallStatus::Removed,
        "{}",
        reaper.message
    );
    assert_eq!(
        reaper.message_code,
        UninstallMessage::Removed {
            backup_set_id: None
        }
    );
    assert!(reaper.removed.is_empty());
    assert!(
        !reaper.message.contains("Trash") && !reaper.message.contains("administrator"),
        "{}",
        reaper.message
    );
    assert!(run.objects.trashed().is_empty());
}

/// More steps than one prompt takes: the run is refused before any prompt,
/// with nothing changed, and the dry run says the same.
#[test]
fn a_command_root_would_refuse_is_blocked_before_any_prompt_in_both_runs() {
    let mac = Mac::new();
    let trees: Vec<String> = (0..154)
        .map(|n| {
            let name = format!("Surge {n:03}");
            fs::create_dir_all(join(
                &mac.fixture.system,
                &format!("Library/Application Support/{name}"),
            ))
            .unwrap();
            format!(
                r#"{{ "path": "/Library/Application Support/{name}", "name_contains": "Surge" }}"#
            )
        })
        .collect();
    let many = mac_spec(
        SURGE,
        r#""requires_standard_install": true,"#,
        &format!(
            r#""elevation": "required", "backup": "none",
               "blocking_programs": ["Surge"],
               "remove": {{ "trees": [{}] }}"#,
            trees.join(", ")
        ),
    );
    let run = Run::new(&mac, vec![many, extension()], mac.objects(owned_by_me()));
    let before = mac.state();

    let dry = run.try_run(&[SURGE, EXT], true).unwrap();
    let report = run.run(&[SURGE, EXT]);

    for report in [&dry, &report] {
        let surge = item(report, SURGE);
        assert_eq!(surge.status, UninstallStatus::Blocked, "{}", surge.message);
        assert!(
            matches!(
                &surge.message_code,
                UninstallMessage::ElevationFailed { error } if error.contains("155 steps")
            ),
            "{:?}",
            surge.message_code
        );
        assert!(report.administrator_steps.is_empty());
        assert!(report.blocked_before_changes() && !report.has_failures());
    }
    assert_eq!(
        item(&report, EXT).message_code,
        UninstallMessage::OtherPackageBlocked {
            package_ids: vec![SURGE.to_string()]
        }
    );
    assert_eq!(run.shell.calls(), 0);
    assert!(run.events().is_empty(), "{:?}", run.events());
    assert_eq!(mac.state(), before);
}

/// Root's first step looks for every REAPER by name. So on root's route
/// RABBIT asks first, as the user, that every REAPER is closed, a portable
/// one included, and stops there without a password window.
#[test]
fn reaper_on_roots_route_waits_for_every_reaper_to_close_before_any_prompt() {
    let mac = Mac::new();
    let portable = mac
        .fixture
        .dir
        .path()
        .join("Portable")
        .join("REAPER.app")
        .join("Contents")
        .join("MacOS")
        .join("REAPER");
    let mut run = Run::new(
        &mac,
        vec![reaper_spec("if_needed")],
        mac.objects(owned_by_root()),
    );
    run.reapers = vec![running("777", "REAPER", &portable)];
    let before = mac.state();

    let report = run.run(&[PACKAGE_REAPER]);

    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(reaper.status, UninstallStatus::Blocked);
    let UninstallMessage::ChecksFailed { checks } = &reaper.message_code else {
        panic!("{:?}", reaper.message_code);
    };
    assert!(
        checks
            .iter()
            .any(|check| check.name == ANY_REAPER_PROCESS_CHECK
                && check.status == PreflightStatus::Fail),
        "{checks:?}"
    );
    assert_eq!(run.shell.calls(), 0);
    assert!(run.events().is_empty(), "{:?}", run.events());
    assert_eq!(mac.state(), before);

    // Moved as the user, nothing looks for other REAPERs: a portable one
    // may stay open.
    let mut run = Run::new(&mac, vec![reaper_spec("never")], mac.objects(owned_by_me()));
    run.reapers = vec![running("777", "REAPER", &portable)];
    let report = run.run(&[PACKAGE_REAPER]);
    assert_eq!(
        item(&report, PACKAGE_REAPER).status,
        UninstallStatus::Removed
    );
}

/// What root removed stays removed, so a package it removed only in part
/// has lost its Audio Unit all the same, and the list is read again. A
/// package nothing of which went needs no refresh.
#[test]
fn a_partly_removed_package_still_refreshes_its_audio_units() {
    let mac = Mac::new();
    // The VST3, Surge's last tree, is refused; the Audio Unit went before.
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.failing_at(5));
    let report = run.run(&[SURGE]);
    let surge = item(&report, SURGE);
    assert_eq!(surge.status, UninstallStatus::Failed);
    let UninstallMessage::AdministratorStepFailed { gone, left, .. } = &surge.message_code else {
        panic!("{:?}", surge.message_code);
    };
    assert!(gone.contains(&mac.surge_tree(2)), "{gone:?}");
    assert_eq!(left, &[mac.surge_tree(4)]);
    assert_eq!(run.objects.audio_unit_refreshes(), 1);

    let mac = Mac::new();
    let run = Run::new(&mac, every_package(), mac.objects(owned_by_me()))
        .with_shell(|shell| shell.failing_at(1));
    run.run(&[SURGE]);
    assert_eq!(run.objects.audio_unit_refreshes(), 0);
}

/// REAPER's app as the shipped plan has it, with a settings folder that
/// "Also remove settings" takes into the backup set.
fn reaper_with_settings() -> PackageSpec {
    mac_spec(
        PACKAGE_REAPER,
        "",
        &format!(
            r#""elevation": "never", "backup": "to_trash",
               "remove": {{ "trees": [{{ "path": "{{app_path}}", "must_end_with": ".app",
                   "name_contains": "REAPER", "bundle_id": "{REAPER_ID}" }}] }},
               "kept_user_data": [{{ "path": "{{resource}}/Demo", "note_key": "test-kept" }}]"#
        ),
    )
}

/// With "Also remove settings", the settings go into the backup set
/// first, then the app goes to the Trash as the user.
#[test]
fn settings_go_into_the_backup_set_before_the_app_goes_to_the_trash() {
    let mac = Mac::new();
    let demo = mac.fixture.resource.join("Demo");
    super::tests::write(&demo.join("settings.ini"), b"mine");
    let run = Run::new(
        &mac,
        vec![reaper_with_settings()],
        mac.objects(owned_by_me()),
    )
    .removing_settings();

    let report = run.run(&[PACKAGE_REAPER]);

    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(
        reaper.status,
        UninstallStatus::Removed,
        "{}",
        reaper.message
    );
    let trashed = mac.trash.join("REAPER.app");
    assert_eq!(
        reaper.message_code,
        UninstallMessage::MovedToTrash {
            trashed_to: Some(trashed.clone())
        }
    );
    assert!(!demo.join("settings.ini").exists());
    assert!(report.backup_set_id.is_some());
    let set = report.backup_set_path.clone().expect("a backup set");
    assert!(
        snapshot(&set).iter().any(|(path, contents)| {
            path.file_name() == Some("settings.ini".as_ref())
                && contents.as_deref() == Some(&b"mine"[..])
        }),
        "{:?}",
        snapshot(&set)
    );
    assert!(join(&trashed, "Contents/MacOS/REAPER").is_file());
}

/// The app didn't go to the Trash: the settings that went into the backup
/// set first come back byte for byte, and no backup set is left.
#[test]
fn settings_come_back_when_the_app_doesnt_go_to_the_trash() {
    for refused in [true, false] {
        let mac = Mac::new();
        let demo = mac.fixture.resource.join("Demo");
        super::tests::write(&demo.join("settings.ini"), b"mine");
        let before = snapshot(&demo);
        let objects = if refused {
            mac.objects(owned_by_me())
                .failing_to_trash(TrashErrorKind::PermissionDenied, "Operation not permitted")
        } else {
            mac.objects(owned_by_me())
                .claiming_to_trash_without_moving()
        };
        let run = Run::new(&mac, vec![reaper_with_settings()], objects).removing_settings();

        let report = run.run(&[PACKAGE_REAPER]);

        let reaper = item(&report, PACKAGE_REAPER);
        assert_eq!(reaper.status, UninstallStatus::Failed, "{}", reaper.message);
        assert!(
            matches!(
                reaper.message_code,
                UninstallMessage::TrashFailed { .. } | UninstallMessage::TrashUnclear { .. }
            ),
            "{:?}",
            reaper.message_code
        );
        assert_eq!(snapshot(&demo), before, "refused: {refused}");
        assert_eq!(report.backup_set_id, None);
        assert!(
            reaper.put_back.iter().any(|path| path.starts_with(&demo)),
            "{:?}",
            reaper.put_back
        );
        assert!(reaper.left_in_backup.is_empty());
        assert!(
            !reaper.message.contains("back in place"),
            "{}",
            reaper.message
        );
        assert!(join(&mac.app(), "Contents/MacOS/REAPER").is_file());
    }
}

fn plan_for(platform: Platform) -> UninstallPlan {
    UninstallPlan {
        resource_path: PathBuf::from("/Users/me/Library/Application Support/REAPER"),
        platform,
        target: InstallationKind::Standard,
        remove_settings: false,
        items: Vec::new(),
    }
}

fn report_item(status: UninstallStatus, code: UninstallMessage) -> UninstallReportItem {
    UninstallReportItem {
        package_id: SURGE.to_string(),
        display_name: "Surge XT".to_string(),
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
        message: String::new(),
        message_code: code,
    }
}

#[test]
fn a_declined_prompt_is_named_the_way_each_platform_shows_it() {
    let declined = report_item(
        UninstallStatus::Cancelled,
        UninstallMessage::ElevationDeclined,
    );
    assert_eq!(
        english_message(&declined, &plan_for(Platform::Windows)),
        "The Windows permission prompt was declined, so RABBIT uninstalled nothing; Surge XT is \
         still installed."
    );
    assert_eq!(
        english_message(&declined, &plan_for(Platform::MacOs)),
        "The administrator password prompt was cancelled, so RABBIT uninstalled nothing; Surge \
         XT is still installed."
    );
}

/// Only what went into a backup set can be back in place; a failed Trash
/// move or administrator step, or a prompt that failed, never says so.
#[test]
fn only_a_backup_set_failure_says_everything_is_back_in_place() {
    let path = PathBuf::from("/Applications/Surge XT.app");
    let back = |code: UninstallMessage| {
        english_message(
            &report_item(UninstallStatus::Failed, code),
            &plan_for(Platform::MacOs),
        )
        .contains("back in place")
    };
    for code in [
        UninstallMessage::MoveFailed {
            path: path.clone(),
            error: "e".to_string(),
        },
        UninstallMessage::FileInUse {
            path: path.clone(),
            error: "e".to_string(),
        },
        UninstallMessage::BackupFailed {
            error: "e".to_string(),
        },
        UninstallMessage::ReceiptSaveFailed {
            error: "e".to_string(),
        },
        UninstallMessage::Unreadable {
            path: path.clone(),
            error: "e".to_string(),
        },
    ] {
        assert!(back(code.clone()), "{code:?}");
    }
    for code in [
        UninstallMessage::TrashFailed {
            error: "e".to_string(),
            refused_by_macos: false,
        },
        UninstallMessage::TrashUnclear { path: path.clone() },
        UninstallMessage::AdministratorStepFailed {
            path: None,
            gone: Vec::new(),
            left: vec![path.clone()],
            refused_by_macos: false,
            detail: None,
        },
        UninstallMessage::ElevationFailed {
            error: "e".to_string(),
        },
    ] {
        assert!(!back(code.clone()), "{code:?}");
    }
}
