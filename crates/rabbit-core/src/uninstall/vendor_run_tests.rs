//! The uninstall engine with packages their vendor's own uninstaller
//! removes: one elevated helper per run, started before anything changes,
//! and each row of the partial-failure table for vendor runs.
//!
//! No test here can reach Windows' real lists or start a real program with
//! administrator rights: the helper is a scripted stand-in (or, in the one
//! end-to-end test, the helper loop on a thread of this process against a
//! throwaway registry key and fixture uninstallers), the vendor lookup finds
//! fixture paths, and every folder is a temporary one.

use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::execute::{UninstallEnvironment, execute_uninstall_in};
use super::helper::{ElevatedHelper, ElevatedRunRequest, ElevatedSession, ElevationStartError};
use super::preflight::VolumeProbe;
use super::tests::{FoundAt, installed, write};
use super::vendor::{VendorEvent, VendorLookup, VendorOutcome, VendorRefusal, VendorStatus};
use super::{
    ANY_REAPER_PROCESS_CHECK, NothingListed, Removability, UninstallMessage, UninstallReport,
    UninstallRequest, UninstallRoots, UninstallStatus,
};
use crate::cancel::CancelToken;
use crate::model::{ComponentDetection, InstallationKind, Platform};
use crate::package::{
    PACKAGE_REAPER, PACKAGE_SURGE_XT, PACKAGE_SWS, PackageSpec, builtin_package_specs,
};
use crate::preflight::RunningProcess;
use crate::progress::{ProgressEvent, ProgressReporter, RemovalMethod};
use crate::receipt::{
    InstallState, InstalledFileReceipt, PackageReceipt, load_install_state, save_install_state,
};

/// Everything on one volume, with room to spare.
struct OneVolume;

impl VolumeProbe for OneVolume {
    fn same_volume(&self, _left: &Path, _right: &Path) -> Option<bool> {
        Some(true)
    }

    fn available_space(&self, _path: &Path) -> Option<u64> {
        Some(u64::MAX)
    }
}

/// What one scripted vendor run does: the files it removes, as the real
/// uninstaller would, whether it says it is still waiting, or waiting for
/// its files to go once it has ended, and how it ends.
struct Script {
    removes: Vec<PathBuf>,
    still_waiting: Option<u64>,
    waiting_for_files: Option<u64>,
    cancels: Option<CancelToken>,
    outcome: VendorOutcome,
}

impl Script {
    fn ends(outcome: VendorOutcome) -> Self {
        Self {
            removes: Vec::new(),
            still_waiting: None,
            waiting_for_files: None,
            cancels: None,
            outcome,
        }
    }

    fn removing(mut self, paths: &[&Path]) -> Self {
        self.removes = paths.iter().map(|path| path.to_path_buf()).collect();
        self
    }
}

/// An outcome with `status`, the uninstaller started and ended with
/// `exit_code`.
fn outcome(package_id: &str, status: VendorStatus, exit_code: Option<i32>) -> VendorOutcome {
    VendorOutcome {
        status,
        launched: true,
        exit_code,
        ..VendorOutcome::new(package_id)
    }
}

/// A stand-in for the elevated helper: it starts, declines or fails as told,
/// runs each package's script once, and writes down every call.
struct ScriptedHelper {
    start: Result<(), ElevationStartError>,
    scripts: Mutex<HashMap<String, Script>>,
    log: Arc<Mutex<Vec<String>>>,
}

impl ScriptedHelper {
    fn new(scripts: Vec<(&str, Script)>) -> Self {
        Self {
            start: Ok(()),
            scripts: Mutex::new(
                scripts
                    .into_iter()
                    .map(|(id, script)| (id.to_string(), script))
                    .collect(),
            ),
            log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn failing_to_start(start: ElevationStartError) -> Self {
        Self {
            start: Err(start),
            ..Self::new(Vec::new())
        }
    }

    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl ElevatedHelper for ScriptedHelper {
    fn start(
        &self,
        request: &ElevatedRunRequest,
    ) -> Result<Box<dyn ElevatedSession>, ElevationStartError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("start {}", request.package_ids.join(",")));
        self.start.clone()?;
        Ok(Box::new(ScriptedSession {
            scripts: std::mem::take(&mut *self.scripts.lock().unwrap()),
            log: Arc::clone(&self.log),
        }))
    }
}

struct ScriptedSession {
    scripts: HashMap<String, Script>,
    log: Arc<Mutex<Vec<String>>>,
}

impl ElevatedSession for ScriptedSession {
    fn run(&mut self, package_id: &str, on_event: &mut dyn FnMut(&VendorEvent)) -> VendorOutcome {
        self.log.lock().unwrap().push(format!("run {package_id}"));
        let script = self
            .scripts
            .remove(package_id)
            .unwrap_or_else(|| panic!("nothing scripted for {package_id}"));
        on_event(&VendorEvent::Launched);
        if let Some(seconds) = script.still_waiting {
            on_event(&VendorEvent::StillWaiting { seconds });
        }
        if let Some(seconds) = script.waiting_for_files {
            on_event(&VendorEvent::WaitingForFiles { seconds });
        }
        for path in &script.removes {
            if path.is_dir() {
                fs::remove_dir_all(path).unwrap();
            } else {
                fs::remove_file(path).unwrap();
            }
        }
        if let Some(cancel) = &script.cancels {
            cancel.cancel();
        }
        script.outcome
    }

    fn finish(self: Box<Self>) {
        self.log.lock().unwrap().push("finish".to_string());
    }
}

/// A standard REAPER in a temporary folder, with Surge XT installed for the
/// whole pretend computer, SWS in the resource folder, and a receipt for
/// all three.
struct Computer {
    dir: tempfile::TempDir,
    roots: UninstallRoots,
    specs: Vec<PackageSpec>,
    sws: PathBuf,
    surge_vst3: PathBuf,
    reaper_ini: PathBuf,
}

impl Computer {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let resource = root.join("Users").join("Someone").join("REAPER");
        let mut roots = UninstallRoots::new(&resource);
        roots.common_program_files = Some(root.join("Common Files"));
        roots.program_data = Some(root.join("ProgramData"));
        roots.program_files = vec![root.join("Program Files")];
        roots.documents = Some(root.join("Documents"));
        let app = root
            .join("Program Files")
            .join("REAPER (x64)")
            .join("reaper.exe");
        write(&app, b"reaper");
        roots.app_dir = app.parent().map(Path::to_path_buf);
        roots.app_path = Some(app);
        let sws = write(
            &resource.join("UserPlugins").join("reaper_sws-x64.dll"),
            b"sws",
        );
        let reaper_ini = write(&resource.join("reaper.ini"), b"[REAPER]");
        let surge_vst3 = root
            .join("Common Files")
            .join("VST3")
            .join("Surge Synth Team")
            .join("Surge XT.vst3");
        write(
            &surge_vst3
                .join("Contents")
                .join("x86_64-win")
                .join("Surge XT.vst3"),
            b"vst3",
        );
        let specs = builtin_package_specs(Platform::Windows)
            .into_iter()
            .filter(|spec| {
                [PACKAGE_REAPER, PACKAGE_SURGE_XT, PACKAGE_SWS].contains(&spec.id.as_str())
            })
            .collect();
        let computer = Self {
            dir,
            roots,
            specs,
            sws,
            surge_vst3,
            reaper_ini,
        };
        computer.save_receipt();
        computer
    }

    fn resource(&self) -> &Path {
        &self.roots.resource
    }

    fn app(&self) -> PathBuf {
        self.roots.app_path.clone().unwrap()
    }

    fn uninstaller(&self, id: &str) -> PathBuf {
        self.dir.path().join(format!("{id}-uninstall.exe"))
    }

    fn save_receipt(&self) {
        let mut state = InstallState::default();
        for (id, path) in [
            (PACKAGE_SWS, self.sws.clone()),
            (PACKAGE_REAPER, self.app()),
            (PACKAGE_SURGE_XT, self.surge_vst3.clone()),
        ] {
            state.packages.insert(
                id.to_string(),
                PackageReceipt {
                    id: id.to_string(),
                    version: None,
                    variant: None,
                    channel: None,
                    source_url: None,
                    source_sha256: None,
                    installed_files: vec![InstalledFileReceipt {
                        path,
                        sha256: None,
                        size: None,
                    }],
                    installed_at: None,
                    rabbit_version: None,
                    architecture: None,
                    vendor_added_files: Vec::new(),
                },
            );
        }
        save_install_state(self.resource(), &state).unwrap();
    }

    fn receipt(&self) -> InstallState {
        load_install_state(self.resource()).unwrap().unwrap()
    }
}

/// One run of the engine on a [`Computer`].
struct Run<'c> {
    computer: &'c Computer,
    helper: &'c dyn ElevatedHelper,
    lookup: Box<dyn VendorLookup + 'c>,
    /// Detection after the uninstallers ran: by default, whatever is still
    /// on disk.
    still_installed: Box<dyn Fn() -> Vec<&'static str> + 'c>,
    /// Whole detections after the uninstallers ran, files included, in
    /// place of `still_installed`.
    detections_after: Option<Box<dyn Fn() -> Vec<ComponentDetection> + 'c>>,
    reapers: Vec<RunningProcess>,
    progress: ProgressReporter,
    cancel: CancelToken,
    dry_run: bool,
}

impl<'c> Run<'c> {
    fn new(computer: &'c Computer, helper: &'c dyn ElevatedHelper) -> Self {
        Self {
            computer,
            helper,
            lookup: Box::new(FoundAtEach(computer)),
            still_installed: Box::new(move || {
                let mut ids = Vec::new();
                if computer.sws.exists() {
                    ids.push(PACKAGE_SWS);
                }
                if computer.surge_vst3.exists() {
                    ids.push(PACKAGE_SURGE_XT);
                }
                if computer.app().exists() {
                    ids.push(PACKAGE_REAPER);
                }
                ids
            }),
            detections_after: None,
            reapers: Vec::new(),
            progress: ProgressReporter::noop(),
            cancel: CancelToken::new(),
            dry_run: false,
        }
    }

    fn uninstall(&self, ids: &[&str]) -> UninstallReport {
        let first = Cell::new(true);
        let detect = || -> crate::error::Result<Vec<ComponentDetection>> {
            // Everything asked for is installed before the run.
            if first.replace(false) {
                return Ok(installed(&[PACKAGE_SWS, PACKAGE_SURGE_XT, PACKAGE_REAPER]));
            }
            if let Some(detections) = &self.detections_after {
                return Ok(detections());
            }
            Ok(installed(&(self.still_installed)()))
        };
        let reapers = || self.reapers.clone();
        let named = |_: &[String]| Vec::new();
        execute_uninstall_in(
            &UninstallRequest {
                resource_path: self.computer.resource().to_path_buf(),
                platform: Platform::Windows,
                package_ids: ids.iter().map(ToString::to_string).collect(),
                remove_settings: false,
                dry_run: self.dry_run,
                target_app_path: Some(self.computer.app()),
                target: Some(InstallationKind::Standard),
            },
            &UninstallEnvironment {
                specs: &self.computer.specs,
                roots: &self.computer.roots,
                target: InstallationKind::Standard,
                target_app_path: Some(self.computer.app()),
                detect: &detect,
                reaper_processes: &reapers,
                named_processes: &named,
                volumes: &OneVolume,
                installed_programs: None,
                vendor_lookup: self.lookup.as_ref(),
                elevated_helper: self.helper,
            },
            &self.progress,
            &self.cancel,
        )
        .unwrap()
    }
}

/// Finds each package's uninstaller at its own fixture path.
struct FoundAtEach<'c>(&'c Computer);

impl VendorLookup for FoundAtEach<'_> {
    fn locate(&self, job: &super::VendorJob<'_>) -> Result<PathBuf, VendorRefusal> {
        Ok(self.0.uninstaller(job.package_id))
    }
}

fn item<'r>(report: &'r UninstallReport, id: &str) -> &'r super::UninstallReportItem {
    report
        .item(id)
        .unwrap_or_else(|| panic!("no item for {id}: {report:#?}"))
}

fn recorded_events() -> (ProgressReporter, Arc<Mutex<Vec<ProgressEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    (
        ProgressReporter::new(move |event| sink.lock().unwrap().push(event)),
        events,
    )
}

/// Exit 0 and everything gone: Removed, its receipt entry goes and the
/// package is remembered as removed. The helper started once, for every
/// package that needs it, before the first removal, and was told to quit at
/// the end. Nothing went into a backup set.
#[test]
fn a_package_its_uninstaller_removed_leaves_the_receipt() {
    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![
        (
            PACKAGE_SURGE_XT,
            Script::ends(outcome(PACKAGE_SURGE_XT, VendorStatus::Removed, Some(0)))
                .removing(&[&computer.surge_vst3]),
        ),
        (
            PACKAGE_REAPER,
            Script::ends(outcome(PACKAGE_REAPER, VendorStatus::Removed, Some(0)))
                .removing(&[&computer.app()]),
        ),
    ]);
    let (progress, events) = recorded_events();
    let mut run = Run::new(&computer, &helper);
    run.progress = progress;

    let report = run.uninstall(&[PACKAGE_REAPER, PACKAGE_SWS, PACKAGE_SURGE_XT]);

    assert_eq!(
        report
            .items
            .iter()
            .map(|item| (item.package_id.as_str(), item.status))
            .collect::<Vec<_>>(),
        [
            (PACKAGE_SWS, UninstallStatus::Removed),
            (PACKAGE_SURGE_XT, UninstallStatus::Removed),
            (PACKAGE_REAPER, UninstallStatus::Removed),
        ],
        "REAPER last: {report:#?}"
    );
    assert_eq!(
        item(&report, PACKAGE_SURGE_XT).message_code,
        UninstallMessage::UninstalledByVendor
    );
    assert!(item(&report, PACKAGE_SURGE_XT).vendor.is_some());
    assert_eq!(
        helper.log(),
        [
            "start surge-xt,reaper",
            "run surge-xt",
            "run reaper",
            "finish"
        ]
    );
    let receipt = computer.receipt();
    for id in [PACKAGE_SWS, PACKAGE_SURGE_XT, PACKAGE_REAPER] {
        assert!(!receipt.packages.contains_key(id), "{id}");
        assert!(receipt.declined_packages.contains(id), "{id}");
    }
    // Only SWS's files went into the backup set; the set records only SWS.
    let set = report.backup_set_path.as_ref().expect("SWS's backup set");
    let manifest = fs::read_to_string(set.join(crate::rollback::BACKUP_MANIFEST_FILE)).unwrap();
    assert!(manifest.contains(PACKAGE_SWS) && !manifest.contains(PACKAGE_SURGE_XT));
    assert!(!report.has_failures());

    let events = events.lock().unwrap();
    assert!(events.contains(&ProgressEvent::RemoveStarted {
        package_id: PACKAGE_SURGE_XT.to_string(),
        method: RemovalMethod::VendorUninstaller,
    }));
    assert!(events.contains(&ProgressEvent::RemoveStarted {
        package_id: PACKAGE_SWS.to_string(),
        method: RemovalMethod::Files,
    }));
}

/// A run with only file removals never starts the helper.
#[test]
fn a_run_without_a_vendor_uninstaller_asks_for_no_permission() {
    let computer = Computer::new();
    let helper = ScriptedHelper::new(Vec::new());
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SWS]);
    assert_eq!(item(&report, PACKAGE_SWS).status, UninstallStatus::Removed);
    assert!(helper.log().is_empty());
}

/// Exit 0, the entry gone from Windows' list, files pending a rename at the
/// next restart: RemovedPendingRestart, listed, and the receipt entry goes.
#[test]
fn files_left_for_the_next_restart_still_count_as_removed() {
    let computer = Computer::new();
    let pending = computer
        .surge_vst3
        .join("Contents")
        .join("x86_64-win")
        .join("Surge XT.vst3");
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(VendorOutcome {
            pending_restart: vec![pending.clone()],
            ..outcome(
                PACKAGE_SURGE_XT,
                VendorStatus::RemovedPendingRestart,
                Some(0),
            )
        }),
    )]);
    let mut run = Run::new(&computer, &helper);
    // Detection finds no Surge any more: the entry and the bundle's name
    // are gone, only a file waiting for the restart is left.
    run.still_installed = Box::new(Vec::new);

    let report = run.uninstall(&[PACKAGE_SURGE_XT]);

    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(surge.status, UninstallStatus::RemovedPendingRestart);
    assert_eq!(surge.pending_restart, [pending]);
    assert!(surge.message.contains("restart"), "{}", surge.message);
    assert!(report.needs_restart() && !report.has_failures());
    assert!(!computer.receipt().packages.contains_key(PACKAGE_SURGE_XT));
}

/// What RABBIT's own detection says about `id` on `computer` right now,
/// starting from its receipt, as a real run asks. Windows' list of
/// installed programs is never read.
fn detected_now(computer: &Computer, id: &str) -> ComponentDetection {
    let spec = computer.specs.iter().find(|spec| spec.id == id).unwrap();
    let state = load_install_state(computer.resource()).unwrap();
    crate::detection::detect_component_with_probes(
        computer.resource(),
        Platform::Windows,
        spec,
        state.as_ref(),
        |_| None,
    )
    .unwrap()
}

/// The usual restart case: another music program has Surge's VST3 loaded,
/// so Windows deletes the plug-in inside the bundle at the next restart,
/// and the bundle folder stays until then. Detection still finds Surge
/// through that bundle, here through the receipt, which lists it. That is
/// no reason to call the removal failed: it is RemovedPendingRestart, the
/// receipt entry goes and Surge joins `declined_packages`.
#[test]
fn a_bundle_waiting_for_the_restart_is_still_removed_pending_the_restart() {
    let computer = Computer::new();
    let pending = computer
        .surge_vst3
        .join("Contents")
        .join("x86_64-win")
        .join("Surge XT.vst3");
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(VendorOutcome {
            still_present: vec![computer.surge_vst3.clone()],
            pending_restart: vec![pending.clone()],
            ..outcome(
                PACKAGE_SURGE_XT,
                VendorStatus::RemovedPendingRestart,
                Some(0),
            )
        }),
    )]);
    let mut run = Run::new(&computer, &helper);
    run.detections_after = Some(Box::new(|| {
        let surge = detected_now(&computer, PACKAGE_SURGE_XT);
        assert!(surge.installed, "the bundle is still there: {surge:#?}");
        let mut detections = installed(&[PACKAGE_SWS, PACKAGE_REAPER]);
        detections.push(surge);
        detections
    }));

    let report = run.uninstall(&[PACKAGE_SURGE_XT]);

    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(
        surge.status,
        UninstallStatus::RemovedPendingRestart,
        "{report:#?}"
    );
    assert_eq!(surge.message_code, UninstallMessage::UninstalledByVendor);
    assert_eq!(surge.pending_restart, [pending]);
    assert!(report.needs_restart() && !report.has_failures());
    let receipt = computer.receipt();
    assert!(!receipt.packages.contains_key(PACKAGE_SURGE_XT));
    assert!(receipt.declined_packages.contains(PACKAGE_SURGE_XT));
}

/// A restart only finishes what Windows has queued. If detection finds the
/// package through something that isn't waiting for the restart, the
/// removal failed as before, and the entry stays.
#[test]
fn a_package_found_outside_what_the_restart_removes_still_failed() {
    let computer = Computer::new();
    let pending = computer
        .surge_vst3
        .join("Contents")
        .join("x86_64-win")
        .join("Surge XT.vst3");
    let effects = write(
        &computer
            .dir
            .path()
            .join("Common Files")
            .join("VST3")
            .join("Surge Synth Team")
            .join("Surge XT Effects.vst3"),
        b"effects",
    );
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(VendorOutcome {
            still_present: vec![computer.surge_vst3.clone()],
            pending_restart: vec![pending],
            ..outcome(
                PACKAGE_SURGE_XT,
                VendorStatus::RemovedPendingRestart,
                Some(0),
            )
        }),
    )]);
    let mut run = Run::new(&computer, &helper);
    run.detections_after = Some(Box::new(|| {
        let mut surge = super::tests::detection(PACKAGE_SURGE_XT, true);
        surge.files = vec![effects.clone()];
        let mut detections = installed(&[PACKAGE_SWS, PACKAGE_REAPER]);
        detections.push(surge);
        detections
    }));

    let report = run.uninstall(&[PACKAGE_SURGE_XT]);

    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(surge.status, UninstallStatus::Failed, "{report:#?}");
    assert!(matches!(
        surge.message_code,
        UninstallMessage::VendorStillThere { .. }
    ));
    assert!(computer.receipt().packages.contains_key(PACKAGE_SURGE_XT));
}

/// Exit 0 with leftovers, the package no longer detected:
/// RemovedWithLeftovers, the leftovers listed and never deleted.
#[test]
fn leftovers_are_listed_and_never_deleted() {
    let computer = Computer::new();
    let leftover = write(
        &computer
            .dir
            .path()
            .join("Common Files")
            .join("CLAP")
            .join("Surge Synth Team")
            .join("SurgeXTData")
            .join("keep.txt"),
        b"mine",
    )
    .parent()
    .unwrap()
    .to_path_buf();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(VendorOutcome {
            leftovers: vec![leftover.clone()],
            ..outcome(
                PACKAGE_SURGE_XT,
                VendorStatus::RemovedWithLeftovers,
                Some(0),
            )
        })
        .removing(&[&computer.surge_vst3]),
    )]);

    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT]);

    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(surge.status, UninstallStatus::RemovedWithLeftovers);
    assert_eq!(surge.leftovers, std::slice::from_ref(&leftover));
    assert!(leftover.join("keep.txt").exists());
    assert!(report.finished_with_problems() && !report.has_failures());
    assert!(!computer.receipt().packages.contains_key(PACKAGE_SURGE_XT));
}

/// Exit 0, but RABBIT still detects the package: Failed, and its receipt
/// entry stays. So does an uninstaller that said it succeeded while the
/// files it was to remove are still there.
#[test]
fn a_package_still_detected_afterwards_failed_and_keeps_its_entry() {
    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(outcome(PACKAGE_SURGE_XT, VendorStatus::Removed, Some(0))),
    )]);
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT]);
    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(surge.status, UninstallStatus::Failed);
    assert!(matches!(
        surge.message_code,
        UninstallMessage::VendorStillThere { .. }
    ));
    assert!(computer.receipt().packages.contains_key(PACKAGE_SURGE_XT));
    assert!(
        !computer
            .receipt()
            .declined_packages
            .contains(PACKAGE_SURGE_XT)
    );

    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(VendorOutcome {
            still_present: vec![computer.surge_vst3.clone()],
            still_listed: true,
            ..outcome(PACKAGE_SURGE_XT, VendorStatus::Failed, Some(0))
        }),
    )]);
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT]);
    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(surge.status, UninstallStatus::Failed);
    assert_eq!(
        surge.message_code,
        UninstallMessage::VendorStillThere {
            still_present: vec![computer.surge_vst3.clone()],
            still_listed: Some(true),
        }
    );
    assert!(computer.receipt().packages.contains_key(PACKAGE_SURGE_XT));
}

/// Exit 1 from REAPER's NSIS uninstaller is the user's cancel; exit 2, or
/// any other code, a failure. Either way the receipt follows detection, and
/// the run goes on with the next package.
#[test]
fn a_cancelled_or_failed_uninstaller_follows_detection_and_the_run_goes_on() {
    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![
        (
            PACKAGE_SURGE_XT,
            Script::ends(outcome(PACKAGE_SURGE_XT, VendorStatus::Failed, Some(2))),
        ),
        (
            PACKAGE_REAPER,
            Script::ends(outcome(PACKAGE_REAPER, VendorStatus::Cancelled, Some(1))),
        ),
    ]);
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT, PACKAGE_REAPER]);

    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(surge.status, UninstallStatus::Failed);
    assert_eq!(
        surge.message_code,
        UninstallMessage::VendorFailed { exit_code: 2 }
    );
    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(reaper.status, UninstallStatus::Cancelled);
    assert_eq!(reaper.message_code, UninstallMessage::VendorCancelled);
    assert_eq!(
        helper.log(),
        [
            "start surge-xt,reaper",
            "run surge-xt",
            "run reaper",
            "finish"
        ]
    );
    // Both are still there, so both keep their entries.
    let receipt = computer.receipt();
    assert!(receipt.packages.contains_key(PACKAGE_SURGE_XT));
    assert!(receipt.packages.contains_key(PACKAGE_REAPER));
    assert!(report.has_failures());

    // An uninstaller that failed after it had removed the package anyway:
    // the entry goes with it.
    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(outcome(PACKAGE_SURGE_XT, VendorStatus::Failed, Some(5)))
            .removing(&[&computer.surge_vst3]),
    )]);
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT]);
    assert_eq!(
        item(&report, PACKAGE_SURGE_XT).status,
        UninstallStatus::Failed
    );
    assert!(!computer.receipt().packages.contains_key(PACKAGE_SURGE_XT));
}

/// Still running at its time limit: Failed, "still open", and the run stops
/// so nothing collides with it. The receipt stays as it was, and the
/// helper is told to quit all the same.
#[test]
fn an_uninstaller_still_open_at_its_time_limit_stops_the_run() {
    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(VendorOutcome {
            exit_code: None,
            ..outcome(PACKAGE_SURGE_XT, VendorStatus::StillRunning, None)
        })
        .removing(&[&computer.surge_vst3]),
    )]);
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT, PACKAGE_REAPER]);

    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(surge.status, UninstallStatus::Failed);
    assert_eq!(
        surge.message_code,
        UninstallMessage::VendorStillOpen { seconds: 600 }
    );
    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(reaper.status, UninstallStatus::NotReached);
    assert_eq!(
        reaper.message_code,
        UninstallMessage::RunStopped {
            package_id: PACKAGE_SURGE_XT.to_string()
        }
    );
    assert_eq!(
        helper.log(),
        ["start surge-xt,reaper", "run surge-xt", "finish"]
    );
    // Even with the bundle gone, nobody can say yet what the uninstaller
    // still running removes: the receipt is left alone.
    assert!(computer.receipt().packages.contains_key(PACKAGE_SURGE_XT));
    assert!(computer.app().exists());
}

/// A helper that is gone, or answers out of turn: Failed, and the run
/// stops. A check the helper repeats right before starting the uninstaller
/// refuses it: Blocked, nothing of it changed, and the run stops too.
#[test]
fn a_lost_helper_or_a_refusal_stops_the_run() {
    for (script, status) in [
        (
            VendorOutcome::failed(
                PACKAGE_SURGE_XT,
                false,
                "RABBIT lost contact with its helper",
            ),
            UninstallStatus::Failed,
        ),
        (
            VendorOutcome::refused(
                PACKAGE_SURGE_XT,
                VendorRefusal::UntrustedOwner {
                    path: PathBuf::from("C:\\ProgramData\\Surge XT\\uninstall\\planted.dll"),
                },
            ),
            UninstallStatus::Blocked,
        ),
    ] {
        let computer = Computer::new();
        let helper = ScriptedHelper::new(vec![(PACKAGE_SURGE_XT, Script::ends(script))]);
        let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT, PACKAGE_REAPER]);
        let surge = item(&report, PACKAGE_SURGE_XT);
        assert_eq!(surge.status, status, "{report:#?}");
        assert!(matches!(
            surge.message_code,
            UninstallMessage::VendorRunFailed { .. } | UninstallMessage::VendorRefused { .. }
        ));
        assert_eq!(
            item(&report, PACKAGE_REAPER).status,
            UninstallStatus::NotReached
        );
        assert_eq!(helper.log().last().map(String::as_str), Some("finish"));
        assert!(computer.receipt().packages.contains_key(PACKAGE_SURGE_XT));
        // A refusal changed nothing, as a blocked run; a lost helper may
        // have, and fails the run.
        if status == UninstallStatus::Blocked {
            assert!(report.blocked_before_changes());
        } else {
            assert!(report.has_failures());
        }
    }
}

/// A helper lost once the uninstaller had started can't say how it ended,
/// and the uninstaller may still be at work. Failed, the run stops, and the
/// receipt is left alone even where the package already looks gone, as for
/// an uninstaller still open at its time limit. The message says nobody
/// knows yet what it removed; for REAPER, that includes its settings.
#[test]
fn a_helper_lost_after_the_uninstaller_started_leaves_the_receipt_alone() {
    for id in [PACKAGE_SURGE_XT, PACKAGE_REAPER] {
        let computer = Computer::new();
        let gone = if id == PACKAGE_REAPER {
            computer.app()
        } else {
            computer.surge_vst3.clone()
        };
        let helper = ScriptedHelper::new(vec![(
            id,
            Script::ends(VendorOutcome::failed(
                id,
                true,
                "RABBIT lost contact with its helper (it ended)",
            ))
            .removing(&[&gone]),
        )]);

        // Surge goes before REAPER, and SWS before REAPER too.
        let other = if id == PACKAGE_REAPER {
            PACKAGE_SWS
        } else {
            PACKAGE_REAPER
        };
        let report = Run::new(&computer, &helper).uninstall(&[id, other]);

        let lost = item(&report, id);
        assert_eq!(lost.status, UninstallStatus::Failed, "{report:#?}");
        assert_eq!(
            lost.message_code,
            UninstallMessage::VendorLost {
                error: "RABBIT lost contact with its helper (it ended)".to_string()
            }
        );
        assert!(
            lost.message.contains("may still be running"),
            "{}",
            lost.message
        );
        assert_eq!(
            lost.message.contains("reaper.ini"),
            id == PACKAGE_REAPER,
            "{}",
            lost.message
        );
        assert!(computer.receipt().packages.contains_key(id), "{id}");
        assert!(!computer.receipt().declined_packages.contains(id), "{id}");
        let expected = if id == PACKAGE_REAPER {
            UninstallStatus::Removed
        } else {
            UninstallStatus::NotReached
        };
        assert_eq!(item(&report, other).status, expected, "{report:#?}");
        assert!(report.has_failures());
    }
}

/// What went wrong around a vendor run reaches the user, not only the JSON:
/// a working folder left behind is named, and a list RABBIT couldn't read
/// afterwards is a check it couldn't make, never "Windows still lists it".
#[test]
fn problems_around_a_vendor_run_reach_the_report() {
    use super::UninstallProblem;
    use super::vendor::VendorProblem;

    let computer = Computer::new();
    let folder = computer
        .dir
        .path()
        .join("ProgramData")
        .join("RABBIT-uninstall-1");
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_REAPER,
        Script::ends(VendorOutcome {
            problems: vec![VendorProblem::WorkingFolderNotRemoved {
                path: folder.clone(),
                error: "in use".to_string(),
            }],
            ..outcome(PACKAGE_REAPER, VendorStatus::Removed, Some(0))
        })
        .removing(&[&computer.app()]),
    )]);
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_REAPER]);
    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(reaper.status, UninstallStatus::Removed);
    assert_eq!(
        reaper.problems,
        [UninstallProblem::VendorWorkingFolderLeft {
            path: folder.clone(),
            error: "in use".to_string(),
        }]
    );
    assert!(
        reaper.message.contains(&folder.display().to_string()),
        "{}",
        reaper.message
    );
    assert!(report.finished_with_problems());

    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(VendorOutcome {
            still_present: vec![computer.surge_vst3.clone()],
            // What the runner reported when the list couldn't be read.
            still_listed: true,
            problems: vec![VendorProblem::ListUnreadable {
                error: "access denied".to_string(),
            }],
            ..outcome(PACKAGE_SURGE_XT, VendorStatus::Failed, Some(0))
        }),
    )]);
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT]);
    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(
        surge.message_code,
        UninstallMessage::VendorStillThere {
            still_present: vec![computer.surge_vst3.clone()],
            still_listed: None,
        }
    );
    assert_eq!(
        surge.problems,
        [UninstallProblem::InstalledListUnreadable {
            error: "access denied".to_string(),
        }]
    );
    assert!(
        !surge.message.contains("Windows still lists it"),
        "{}",
        surge.message
    );
    assert!(
        surge.message.contains("couldn't check"),
        "{}",
        surge.message
    );

    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script::ends(VendorOutcome {
            still_present: vec![computer.surge_vst3.clone()],
            problems: vec![VendorProblem::PendingRenamesUnreadable {
                error: "access denied".to_string(),
            }],
            ..outcome(PACKAGE_SURGE_XT, VendorStatus::Failed, Some(0))
        }),
    )]);
    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SURGE_XT]);
    let surge = item(&report, PACKAGE_SURGE_XT);
    assert_eq!(
        surge.message_code,
        UninstallMessage::VendorStillThere {
            still_present: vec![computer.surge_vst3.clone()],
            still_listed: Some(false),
        }
    );
    assert_eq!(
        surge.problems,
        [UninstallProblem::RestartListUnreadable {
            error: "access denied".to_string(),
        }]
    );
    assert!(surge.message.contains("restart"), "{}", surge.message);
}

/// Declining the Windows permission prompt cancels the whole run before
/// anything changes, file removals included: every package is Cancelled,
/// no file moves, the receipt stays, and no backup set is made.
#[test]
fn a_declined_prompt_cancels_the_whole_run_and_changes_nothing() {
    let computer = Computer::new();
    let receipt_before = fs::read(crate::receipt::receipt_path(computer.resource())).unwrap();
    let helper = ScriptedHelper::failing_to_start(ElevationStartError::Declined);

    let report =
        Run::new(&computer, &helper).uninstall(&[PACKAGE_SWS, PACKAGE_SURGE_XT, PACKAGE_REAPER]);

    for id in [PACKAGE_SWS, PACKAGE_SURGE_XT, PACKAGE_REAPER] {
        let item = item(&report, id);
        assert_eq!(item.status, UninstallStatus::Cancelled, "{id}");
        assert_eq!(
            item.message_code,
            UninstallMessage::ElevationDeclined,
            "{id}"
        );
        assert!(
            item.message.contains("permission prompt was declined"),
            "{}",
            item.message
        );
    }
    assert!(report.elevation_declined() && report.was_cancelled() && report.has_failures());
    assert!(!report.changed_anything());
    assert!(computer.sws.exists() && computer.surge_vst3.exists() && computer.app().exists());
    assert_eq!(
        fs::read(crate::receipt::receipt_path(computer.resource())).unwrap(),
        receipt_before
    );
    assert!(report.backup_set_id.is_none());
    assert_eq!(helper.log(), ["start surge-xt,reaper"]);
}

/// A helper that can't start fails the packages that need it, leaves the
/// others unreached, and changes nothing, the way a blocked run does.
#[test]
fn a_helper_that_cant_start_changes_nothing() {
    let computer = Computer::new();
    let helper =
        ScriptedHelper::failing_to_start(ElevationStartError::Failed("no pipe".to_string()));

    let report =
        Run::new(&computer, &helper).uninstall(&[PACKAGE_SWS, PACKAGE_SURGE_XT, PACKAGE_REAPER]);

    for id in [PACKAGE_SURGE_XT, PACKAGE_REAPER] {
        assert_eq!(item(&report, id).status, UninstallStatus::Failed, "{id}");
        assert_eq!(
            item(&report, id).message_code,
            UninstallMessage::ElevationFailed {
                error: "no pipe".to_string()
            }
        );
    }
    let sws = item(&report, PACKAGE_SWS);
    assert_eq!(sws.status, UninstallStatus::NotReached);
    assert_eq!(
        sws.message_code,
        UninstallMessage::RunStopped {
            package_id: PACKAGE_SURGE_XT.to_string()
        }
    );
    assert!(computer.sws.exists());
    assert!(computer.receipt().packages.contains_key(PACKAGE_SWS));
    assert!(!report.changed_anything() && report.has_failures());
}

/// The post-run guard: REAPER's uninstaller must leave the resource folder
/// alone. If reaper.ini or RABBIT's receipt is gone afterwards, REAPER is
/// Failed, the run stops, and the receipt is written again from the copy the
/// run kept in memory, without what the run removed before.
#[test]
fn reaper_taking_its_settings_or_the_receipt_fails_loudly_and_the_receipt_comes_back() {
    let computer = Computer::new();
    let receipt = crate::receipt::receipt_path(computer.resource());
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_REAPER,
        Script::ends(outcome(PACKAGE_REAPER, VendorStatus::Removed, Some(0))).removing(&[
            &computer.app(),
            &computer.reaper_ini,
            &receipt,
        ]),
    )]);

    let report = Run::new(&computer, &helper).uninstall(&[PACKAGE_SWS, PACKAGE_REAPER]);

    assert_eq!(item(&report, PACKAGE_SWS).status, UninstallStatus::Removed);
    let reaper = item(&report, PACKAGE_REAPER);
    assert_eq!(reaper.status, UninstallStatus::Failed);
    assert_eq!(
        reaper.message_code,
        UninstallMessage::ResourceFolderDamaged {
            missing: vec![computer.reaper_ini.clone(), receipt.clone()]
        }
    );
    assert!(
        reaper.message.starts_with("IMPORTANT"),
        "{}",
        reaper.message
    );
    assert!(report.has_failures());
    let rewritten = computer.receipt();
    assert!(
        !rewritten.packages.contains_key(PACKAGE_SWS),
        "SWS was removed earlier in the run"
    );
    assert!(rewritten.declined_packages.contains(PACKAGE_SWS));
    assert!(rewritten.packages.contains_key(PACKAGE_SURGE_XT));
    // REAPER itself is gone, so its entry follows detection.
    assert!(!rewritten.packages.contains_key(PACKAGE_REAPER));
}

/// The still-waiting status reaches the progress page once, between the
/// package's start and its end, and so does waiting for the files to go
/// after the uninstaller ended.
#[test]
fn a_long_uninstaller_sends_still_waiting_once() {
    let computer = Computer::new();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script {
            still_waiting: Some(30),
            waiting_for_files: Some(30),
            ..Script::ends(outcome(PACKAGE_SURGE_XT, VendorStatus::Removed, Some(0)))
                .removing(&[&computer.surge_vst3])
        },
    )]);
    let (progress, events) = recorded_events();
    let mut run = Run::new(&computer, &helper);
    run.progress = progress;
    run.uninstall(&[PACKAGE_SURGE_XT]);

    assert_eq!(
        *events.lock().unwrap(),
        [
            ProgressEvent::RemoveStarted {
                package_id: PACKAGE_SURGE_XT.to_string(),
                method: RemovalMethod::VendorUninstaller,
            },
            ProgressEvent::VendorStillWaiting {
                package_id: PACKAGE_SURGE_XT.to_string(),
                seconds: 30,
            },
            ProgressEvent::VendorWaitingForFiles {
                package_id: PACKAGE_SURGE_XT.to_string(),
                seconds: 30,
            },
            ProgressEvent::RemoveCompleted {
                package_id: PACKAGE_SURGE_XT.to_string(),
                removed: true,
            },
        ]
    );
}

/// The helper is always told to quit: after a stop asked for while a
/// vendor's uninstaller ran, the next package is Cancelled and the session
/// still finishes. A run stopped before it started never asks for
/// permission at all.
#[test]
fn the_helper_is_finished_however_the_run_ends() {
    let computer = Computer::new();
    let cancel = CancelToken::new();
    let helper = ScriptedHelper::new(vec![(
        PACKAGE_SURGE_XT,
        Script {
            cancels: Some(cancel.clone()),
            ..Script::ends(outcome(PACKAGE_SURGE_XT, VendorStatus::Removed, Some(0)))
                .removing(&[&computer.surge_vst3])
        },
    )]);
    let mut run = Run::new(&computer, &helper);
    run.cancel = cancel;
    let report = run.uninstall(&[PACKAGE_SURGE_XT, PACKAGE_REAPER]);
    assert_eq!(
        item(&report, PACKAGE_SURGE_XT).status,
        UninstallStatus::Removed
    );
    assert_eq!(
        item(&report, PACKAGE_REAPER).status,
        UninstallStatus::Cancelled
    );
    assert_eq!(
        helper.log(),
        ["start surge-xt,reaper", "run surge-xt", "finish"]
    );

    let computer = Computer::new();
    let helper = ScriptedHelper::new(Vec::new());
    let run = Run::new(&computer, &helper);
    run.cancel.cancel();
    let report = run.uninstall(&[PACKAGE_SURGE_XT]);
    assert_eq!(
        item(&report, PACKAGE_SURGE_XT).status,
        UninstallStatus::Cancelled
    );
    assert!(
        helper.log().is_empty(),
        "no prompt for a run already stopped"
    );
}

/// A dry run plans the vendor run and says it needs administrator rights,
/// and starts nothing. A REAPER running anywhere, a portable one included,
/// blocks a vendor's uninstaller in the dry run and the real run alike,
/// before any permission prompt.
#[test]
fn a_dry_run_and_a_running_reaper_never_start_the_helper() {
    let computer = Computer::new();
    let helper = ScriptedHelper::new(Vec::new());
    let mut run = Run::new(&computer, &helper);
    run.dry_run = true;
    let report = run.uninstall(&[PACKAGE_SURGE_XT, PACKAGE_REAPER]);
    for id in [PACKAGE_SURGE_XT, PACKAGE_REAPER] {
        assert_eq!(
            item(&report, id).status,
            UninstallStatus::WouldRemove,
            "{id}"
        );
        let planned = report.plan.item(id).unwrap();
        assert!(planned.needs_elevated_helper(), "{id}");
        assert_eq!(
            planned.vendor_run.as_ref().unwrap().uninstaller,
            computer.uninstaller(id)
        );
    }

    let portable = computer.dir.path().join("Portable").join("reaper.exe");
    for dry_run in [true, false] {
        let mut run = Run::new(&computer, &helper);
        run.dry_run = dry_run;
        run.reapers = vec![super::tests::running("7", "reaper.exe", &portable)];
        let report = run.uninstall(&[PACKAGE_SURGE_XT]);
        let surge = item(&report, PACKAGE_SURGE_XT);
        assert_eq!(surge.status, UninstallStatus::Blocked, "{report:#?}");
        let UninstallMessage::ChecksFailed { checks } = &surge.message_code else {
            panic!("{:?}", surge.message_code);
        };
        assert!(
            checks
                .iter()
                .any(|check| check.name == ANY_REAPER_PROCESS_CHECK)
        );
        assert!(report.blocked_before_changes());
    }
    assert!(helper.log().is_empty());
    assert!(computer.surge_vst3.exists() && computer.app().exists());
}

/// Without an uninstaller RABBIT trusts, the package is blocked and the run
/// changes nothing, with no prompt.
#[test]
fn no_trusted_uninstaller_blocks_the_run_before_any_prompt() {
    let computer = Computer::new();
    let helper = ScriptedHelper::new(Vec::new());
    let mut run = Run::new(&computer, &helper);
    run.lookup = Box::new(NothingListed);
    let report = run.uninstall(&[PACKAGE_SWS, PACKAGE_SURGE_XT]);
    assert_eq!(
        item(&report, PACKAGE_SURGE_XT).message_code,
        UninstallMessage::NotRemovable {
            removability: Removability::NoTrustedUninstaller {
                refusal: VendorRefusal::NotListed
            }
        }
    );
    assert_eq!(
        item(&report, PACKAGE_SWS).status,
        UninstallStatus::NotReached
    );
    assert!(computer.sws.exists());
    assert!(helper.log().is_empty());
    assert!(report.blocked_before_changes());
}

/// The whole way on Windows: the engine plans Surge XT through a lookup,
/// starts the helper loop (on a thread here, against a throwaway registry
/// key standing in for HKLM), and the vendor runner checks and runs a
/// fixture uninstaller, removes the junctions it leaves and the empty
/// vendor folders, and reports back.
#[cfg(windows)]
#[test]
fn surge_goes_through_the_helper_and_the_vendor_runner_end_to_end() {
    use super::vendor::tests::windows::{Fixture, install_surge, surge_uninstaller};

    let fixture = Fixture::new();
    let surge = install_surge(&fixture);
    let env = fixture.env(surge_uninstaller(&fixture, &surge));
    let report = surge_end_to_end(&fixture, &surge, env);

    let item = item(&report, "surge-test");
    assert_eq!(item.status, UninstallStatus::Removed, "{report:#?}");
    assert_eq!(item.message_code, UninstallMessage::UninstalledByVendor);
    assert!(!surge.vst3.exists() && !surge.clap.exists());
    assert!(!surge.clap_junction.exists() && !surge.vst3_junction.exists());
    assert!(!surge.clap_vendor_dir.exists() && !surge.vst3_vendor_dir.exists());
    assert_eq!(
        fs::read(&surge.data_file).unwrap(),
        b"patch",
        "the junctions went as links; what they pointed to stays"
    );
    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    assert!(receipt.declined_packages.contains("surge-test"));
}

/// The usual restart case the whole way on Windows: another music program
/// has Surge's VST3 loaded, so Windows deletes it at the next restart, and
/// detection still finds Surge through that bundle until then. The helper
/// removes the junctions and the emptied CLAP folder all the same, since
/// nothing would after the restart, so the run only asks for the restart:
/// nothing is left over, and nothing went wrong.
#[cfg(windows)]
#[test]
fn surge_finishing_at_the_restart_leaves_nothing_over_end_to_end() {
    use super::vendor::tests::windows::{Fixture, install_surge, surge_with_its_vst3_in_use};

    let fixture = Fixture::new();
    let surge = install_surge(&fixture);
    let (env, in_use) = surge_with_its_vst3_in_use(&fixture, &surge);
    let report = surge_end_to_end(&fixture, &surge, env);

    let item = item(&report, "surge-test");
    assert_eq!(
        item.status,
        UninstallStatus::RemovedPendingRestart,
        "{report:#?}"
    );
    assert_eq!(item.message_code, UninstallMessage::UninstalledByVendor);
    assert_eq!(item.pending_restart, [in_use]);
    assert!(item.leftovers.is_empty(), "{:#?}", item.leftovers);
    assert!(report.needs_restart());
    assert!(!report.finished_with_problems() && !report.has_failures());
    assert!(fs::symlink_metadata(&surge.clap_junction).is_err());
    assert!(fs::symlink_metadata(&surge.vst3_junction).is_err());
    assert!(!surge.clap_vendor_dir.exists());
    assert!(surge.vst3.exists(), "the restart removes it, not RABBIT");
    assert_eq!(fs::read(&surge.data_file).unwrap(), b"patch");
    let receipt = load_install_state(&fixture.resource).unwrap().unwrap();
    assert!(receipt.declined_packages.contains("surge-test"));
}

/// Uninstall the fixture's Surge XT, as the package `surge-test`, through
/// the helper loop on a thread and the vendor runner in `env`. Detection
/// finds it through its VST3 bundle while that is there.
#[cfg(windows)]
fn surge_end_to_end(
    fixture: &super::vendor::tests::windows::Fixture,
    surge: &super::vendor::tests::windows::Surge,
    env: super::vendor::tests::TestEnvironment,
) -> UninstallReport {
    use super::InProcessElevatedHelper;
    use super::helper::tests::elevated_surge;

    let spec = elevated_surge("surge-test");
    let helper =
        InProcessElevatedHelper::new(Arc::new(env), vec![spec.clone()], fixture.folders.clone());
    let roots = fixture.roots(None);
    let vst3 = surge.vst3.clone();
    let detect = || -> crate::error::Result<Vec<ComponentDetection>> {
        if !vst3.exists() {
            return Ok(installed(&[]));
        }
        let mut detections = installed(&["surge-test"]);
        for detection in &mut detections {
            if detection.package_id == "surge-test" {
                detection.files = vec![vst3.clone()];
            }
        }
        Ok(detections)
    };
    let nothing = || Vec::new();
    let named = |_: &[String]| Vec::new();
    execute_uninstall_in(
        &UninstallRequest {
            resource_path: fixture.resource.clone(),
            platform: Platform::Windows,
            package_ids: vec!["surge-test".to_string()],
            remove_settings: false,
            dry_run: false,
            target_app_path: None,
            target: Some(InstallationKind::Standard),
        },
        &UninstallEnvironment {
            specs: &[spec],
            roots: &roots,
            target: InstallationKind::Standard,
            target_app_path: None,
            detect: &detect,
            reaper_processes: &nothing,
            named_processes: &named,
            volumes: &OneVolume,
            installed_programs: None,
            vendor_lookup: &FoundAt(surge.uninstaller.clone()),
            elevated_helper: &helper,
        },
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap()
}
