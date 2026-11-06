//! Tests for the vendor runner.
//!
//! Nothing here reaches the real system's elevated paths: every run reads a
//! throwaway key, `HKCU\Software\RABBIT-test-<unique>`, that stands in for
//! HKLM's list of installed programs; every folder is a fixture in a
//! temporary folder; and [`TestEnvironment`] refuses to start any program
//! outside its fixture folder. The programs it starts are fakes or this
//! test binary itself, told by environment variables to act as an
//! uninstaller.

// Most fixtures serve only the Windows tests below.
#![cfg_attr(not(windows), allow(dead_code))]

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rabbit_platform::{PendingFileRename, UninstallViews};

use super::{
    LeftoverResult, ProgramEnd, RunLimits, VendorCommand, VendorEnvironment, VendorExit,
    VendorOutcome, VendorRefusal, VendorStatus, run_timed_command, vendor_exit,
};
use crate::localization::{Localizer, embedded_locales};
use crate::package::uninstall::VendorUninstallerKind;
use crate::uninstall::UninstallStatus;
use crate::uninstall::paths::PathRefusal;

/// Who owns what, as the runner's owner check sees it.
pub(crate) enum Owners {
    /// Everything is owned by administrators.
    Trusted,
    /// These are owned by administrators, standing in for the files a
    /// vendor's installer made; everything else is asked of Windows.
    RealExcept(Vec<PathBuf>),
}

/// What a fake uninstaller does: it gets the command, and says how it ended
/// and whether it ran long enough for "still waiting".
pub(crate) type FakeProgram = Box<dyn Fn(&VendorCommand) -> FakeRun + Send + Sync>;

pub(crate) struct FakeRun {
    pub(crate) end: ProgramEnd,
    pub(crate) still_waiting: bool,
}

impl FakeRun {
    pub(crate) fn exits(code: i32) -> Self {
        Self {
            end: ProgramEnd::Exited(code),
            still_waiting: false,
        }
    }
}

pub(crate) enum Runner {
    /// Start the program for real, with these variables in its
    /// environment.
    Real(Vec<(String, OsString)>),
    Fake(FakeProgram),
}

/// A clock that only moves when the runner sleeps, and a hook that sees
/// each new time, so a test can make an uninstaller's files disappear a
/// few seconds into the checks without waiting for them.
pub(crate) struct VirtualClock {
    start: Instant,
    elapsed: Mutex<Duration>,
    on_sleep: Box<dyn Fn(Duration) + Send + Sync>,
}

impl VirtualClock {
    pub(crate) fn new(on_sleep: impl Fn(Duration) + Send + Sync + 'static) -> Self {
        Self {
            start: Instant::now(),
            elapsed: Mutex::new(Duration::ZERO),
            on_sleep: Box::new(on_sleep),
        }
    }

    pub(crate) fn elapsed(&self) -> Duration {
        *self.elapsed.lock().unwrap()
    }
}

/// The vendor runner's environment in tests.
pub(crate) struct TestEnvironment {
    pub(crate) views: UninstallViews,
    pub(crate) owners: Owners,
    /// Where working folders go; never ProgramData.
    pub(crate) working_parent: PathBuf,
    /// The only folder programs may be started from.
    pub(crate) sandbox: PathBuf,
    pub(crate) runner: Runner,
    pub(crate) pending: Vec<PendingFileRename>,
    pub(crate) clock: Option<VirtualClock>,
    /// Change every copy after it is made, as if it had been swapped.
    pub(crate) corrupt_copies: bool,
    /// Every command the runner asked to start.
    pub(crate) started: Mutex<Vec<VendorCommand>>,
    next_folder: Mutex<usize>,
}

impl TestEnvironment {
    pub(crate) fn new(views: UninstallViews, sandbox: &Path, runner: Runner) -> Self {
        Self {
            views,
            owners: Owners::Trusted,
            working_parent: sandbox.join("work"),
            sandbox: sandbox.to_path_buf(),
            runner,
            pending: Vec::new(),
            clock: None,
            corrupt_copies: false,
            started: Mutex::new(Vec::new()),
            next_folder: Mutex::new(0),
        }
    }

    pub(crate) fn started(&self) -> Vec<VendorCommand> {
        self.started.lock().unwrap().clone()
    }
}

impl VendorEnvironment for TestEnvironment {
    fn uninstall_views(&self) -> &UninstallViews {
        &self.views
    }

    fn owner_is_trusted(&self, path: &Path) -> io::Result<bool> {
        match &self.owners {
            Owners::Trusted => Ok(true),
            Owners::RealExcept(trusted) => {
                if trusted.iter().any(|trusted| trusted == path) {
                    Ok(true)
                } else {
                    rabbit_platform::file_owner_is_trusted(path)
                }
            }
        }
    }

    fn create_working_folder(&self) -> io::Result<PathBuf> {
        let mut next = self.next_folder.lock().unwrap();
        *next += 1;
        std::fs::create_dir_all(&self.working_parent)?;
        let folder = self
            .working_parent
            .join(format!("{}-{next}", super::WORKING_FOLDER_PREFIX));
        std::fs::create_dir(&folder)?;
        Ok(folder)
    }

    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::copy(from, to)?;
        if self.corrupt_copies {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(to)?
                .write_all(b"swapped")?;
        }
        Ok(())
    }

    fn run_program(
        &self,
        command: &VendorCommand,
        limits: RunLimits,
        on_started: &mut dyn FnMut(),
        on_still_waiting: &mut dyn FnMut(),
    ) -> io::Result<ProgramEnd> {
        // However a test is put together, it starts nothing but fixtures.
        let inside_sandbox = std::fs::canonicalize(&command.program)
            .ok()
            .zip(std::fs::canonicalize(&self.sandbox).ok())
            .is_some_and(|(program, sandbox)| program.starts_with(sandbox));
        if !inside_sandbox {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "tests only start programs in their fixture folder, not {}",
                    command.program.display()
                ),
            ));
        }
        self.started.lock().unwrap().push(command.clone());
        match &self.runner {
            Runner::Real(environment) => {
                let environment: Vec<(&str, &std::ffi::OsStr)> = environment
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_os_str()))
                    .collect();
                run_timed_command(command, &environment, limits, on_started, on_still_waiting)
            }
            Runner::Fake(program) => {
                on_started();
                let run = program(command);
                if run.still_waiting {
                    on_still_waiting();
                }
                Ok(run.end)
            }
        }
    }

    fn pending_file_renames(&self) -> io::Result<Vec<PendingFileRename>> {
        Ok(self.pending.clone())
    }

    fn now(&self) -> Instant {
        match &self.clock {
            Some(clock) => clock.start + clock.elapsed(),
            None => Instant::now(),
        }
    }

    fn sleep(&self, duration: Duration) {
        match &self.clock {
            Some(clock) => {
                let now = {
                    let mut elapsed = clock.elapsed.lock().unwrap();
                    *elapsed += duration;
                    *elapsed
                };
                (clock.on_sleep)(now);
            }
            None => std::thread::sleep(duration),
        }
    }
}

#[test]
fn exit_codes_read_as_each_installer_documents_them() {
    use VendorUninstallerKind::{Inno, Nsis};
    assert_eq!(vendor_exit(Nsis, 0), VendorExit::Success);
    assert_eq!(vendor_exit(Nsis, 1), VendorExit::Cancelled);
    assert_eq!(vendor_exit(Nsis, 2), VendorExit::Failed);
    assert_eq!(vendor_exit(Nsis, 1223), VendorExit::Failed);
    assert_eq!(vendor_exit(Nsis, -1), VendorExit::Failed);
    // Inno only promises that anything but 0 didn't finish.
    assert_eq!(vendor_exit(Inno, 0), VendorExit::Success);
    for code in [1, 2, 3, 5, 6, 7, -1] {
        assert_eq!(vendor_exit(Inno, code), VendorExit::Failed, "{code}");
    }
}

#[test]
fn outcomes_become_uninstall_statuses() {
    let status = |status| {
        VendorOutcome {
            status,
            ..VendorOutcome::new("demo")
        }
        .uninstall_status()
    };
    assert_eq!(status(VendorStatus::Removed), UninstallStatus::Removed);
    assert_eq!(
        status(VendorStatus::RemovedWithLeftovers),
        UninstallStatus::RemovedWithLeftovers
    );
    assert_eq!(
        status(VendorStatus::RemovedPendingRestart),
        UninstallStatus::RemovedPendingRestart
    );
    assert_eq!(status(VendorStatus::Cancelled), UninstallStatus::Cancelled);
    assert_eq!(status(VendorStatus::Failed), UninstallStatus::Failed);
    assert_eq!(status(VendorStatus::StillRunning), UninstallStatus::Failed);
    // Nothing changed, so the package is blocked rather than failed.
    assert_eq!(status(VendorStatus::Refused), UninstallStatus::Blocked);
}

#[test]
fn an_outcome_crosses_the_pipe_unchanged() {
    let outcome = VendorOutcome {
        status: VendorStatus::RemovedWithLeftovers,
        uninstaller: Some(PathBuf::from(
            "C:\\ProgramData\\Demo\\uninstall\\unins000.exe",
        )),
        launched: true,
        exit_code: Some(0),
        still_present: vec![PathBuf::from("C:\\Demo.vst3")],
        leftovers: vec![PathBuf::from("C:\\Demo\\Data")],
        removed_leftovers: vec![super::LeftoverOutcome {
            path: PathBuf::from("C:\\Demo\\Link"),
            kind: super::LeftoverKind::Junction,
            result: LeftoverResult::Refused {
                refusal: PathRefusal::NotALink,
            },
        }],
        problems: vec![super::VendorProblem::WorkingFolderNotRemoved {
            path: PathBuf::from("C:\\ProgramData\\RABBIT-uninstall-1"),
            error: "busy".to_string(),
        }],
        ..VendorOutcome::refused("demo", VendorRefusal::CopyDiffers)
    };
    let line = serde_json::to_string(&outcome).unwrap();
    assert!(!line.contains('\n'));
    assert_eq!(
        serde_json::from_str::<VendorOutcome>(&line).unwrap(),
        outcome
    );
}

/// One of each refusal, with a path where it has one.
fn every_refusal() -> Vec<VendorRefusal> {
    let path = || PathBuf::from("C:\\ProgramData\\Demo\\uninstall\\planted.dll");
    vec![
        VendorRefusal::NotListed,
        VendorRefusal::UnclearCommand {
            value: "\"C:\\Demo".to_string(),
        },
        VendorRefusal::NotAFullPath { path: path() },
        VendorRefusal::UnknownFolder {
            token: "{app_dir}".to_string(),
        },
        VendorRefusal::WrongFolder { path: path() },
        VendorRefusal::ForeignFile { path: path() },
        VendorRefusal::FolderOrLink { path: path() },
        VendorRefusal::UntrustedOwner { path: path() },
        VendorRefusal::InstallLocationMissing,
        VendorRefusal::InstallLocationMismatch {
            found: "C:\\Other".to_string(),
            expected: path(),
        },
        VendorRefusal::NotInInstallFolder {
            path: path(),
            folder: path(),
        },
        VendorRefusal::NotBelowProgramFiles { folder: path() },
        VendorRefusal::CopyDiffers,
        VendorRefusal::Unreadable {
            what: path().display().to_string(),
            error: "denied".to_string(),
        },
        VendorRefusal::NotAVendorPlan,
        VendorRefusal::Unsupported,
    ]
}

/// Every refusal can be said in every language RABBIT speaks, with the
/// package's name and, where there is one, the path that caused it.
#[test]
fn every_refusal_reads_in_every_language() {
    let mut keys: Vec<&str> = every_refusal()
        .iter()
        .map(VendorRefusal::fluent_key)
        .collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), every_refusal().len(), "one key per refusal");
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for refusal in every_refusal() {
            let arguments = refusal.fluent_arguments("Surge XT");
            let borrowed: Vec<(&str, &str)> = arguments
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect();
            let text = localizer.format(refusal.fluent_key(), &borrowed);
            assert!(!text.missing, "{locale} lacks {}", refusal.fluent_key());
            for (name, value) in &arguments {
                if *name == "path" {
                    continue;
                }
                assert!(
                    text.value.contains(value.as_str()),
                    "{locale} {}: {:?} lacks {value}",
                    refusal.fluent_key(),
                    text.value
                );
            }
            // A token such as {app_dir} means nothing to the user, so that
            // message leaves it out.
            if let Some((_, path)) = arguments.iter().find(|(name, _)| *name == "path")
                && refusal.fluent_key() != "uninstall-vendor-refused-unknown-folder"
            {
                assert!(
                    text.value.contains(path.as_str()),
                    "{locale} {}: {:?} lacks {path}",
                    refusal.fluent_key(),
                    text.value
                );
            }
        }
        let nothing_changed = localizer.format(
            "uninstall-vendor-refused-nothing-changed",
            &[("package", "Surge XT")],
        );
        assert!(!nothing_changed.missing, "{locale}");
        assert!(nothing_changed.value.contains("Surge XT"), "{locale}");
    }
}

/// Everything that needs Windows: the registry, junctions, owners and
/// programs.
#[cfg(windows)]
pub(crate) mod windows {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use rabbit_platform::{
        HkcuUninstallRoot, PendingFileRename, RegistryValue, UninstallEntry, UninstallViews,
    };
    use tempfile::TempDir;

    use super::super::{
        LeftoverKind, LeftoverOutcome, LeftoverResult, ProgramEnd, VendorEvent, VendorJob,
        VendorOutcome, VendorRefusal, VendorStatus, locate_uninstaller, run_vendor_uninstaller,
        uninstaller_program,
    };
    use super::{FakeRun, Owners, Runner, TestEnvironment, VirtualClock};
    use crate::model::InstallationKind;
    use crate::package::uninstall::VendorUninstallerSpec;
    use crate::uninstall::helper::{HelperArgs, MachineFolders, helper_roots};
    use crate::uninstall::paths::{PathRefusal, UninstallRoots, make_junction};

    /// `path`'s real path, written the ordinary way (`C:\...`): what an
    /// installer would have written into the registry. Temporary folders
    /// can be spelt with short 8.3 names, which the runner rightly refuses
    /// as not the folder itself.
    pub(crate) fn plain(path: &Path) -> PathBuf {
        let real = fs::canonicalize(path).unwrap();
        let text = real.to_str().unwrap();
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text))
    }

    /// A key of one test's own, `HKCU\Software\RABBIT-test-<unique>`,
    /// standing in for HKLM's list of installed programs. Dropping it, also
    /// when the test panics, deletes every entry the test wrote, then the
    /// key.
    pub(crate) struct ThrowawayList {
        name: String,
        entries: std::sync::Mutex<Vec<String>>,
    }

    impl ThrowawayList {
        pub(crate) fn create() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default();
            Self {
                name: format!(
                    "RABBIT-test-{}-{nanos}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ),
                entries: std::sync::Mutex::new(Vec::new()),
            }
        }

        /// The key's path under HKCU.
        pub(crate) fn path(&self) -> String {
            format!("Software\\{}", self.name)
        }

        pub(crate) fn root(&self) -> HkcuUninstallRoot {
            HkcuUninstallRoot::at(self.path())
        }

        pub(crate) fn views(&self) -> UninstallViews {
            UninstallViews::current_user_at(self.path())
        }

        pub(crate) fn write(&self, key: &str, values: &[(&str, &str)]) {
            let entry = UninstallEntry::new(
                values
                    .iter()
                    .map(|(name, text)| RegistryValue::string(*name, text))
                    .collect(),
            );
            self.root().write_entry(key, &entry).unwrap();
            self.entries.lock().unwrap().push(key.to_string());
        }
    }

    impl Drop for ThrowawayList {
        fn drop(&mut self) {
            // Plain deletes, each refused for a key that still has a
            // subkey, so nothing here reaches past what the test made.
            let root = self.root();
            for entry in self.entries.lock().unwrap().iter() {
                let _ = root.delete_entry(entry);
            }
            let _ = HkcuUninstallRoot::at("Software").delete_entry(&self.name);
        }
    }

    /// A pretend computer in a temporary folder: Program Files with Common
    /// Files in it, ProgramData, a resource folder, and a list of installed
    /// programs.
    pub(crate) struct Fixture {
        _dir: TempDir,
        pub(crate) root: PathBuf,
        pub(crate) folders: MachineFolders,
        pub(crate) resource: PathBuf,
        pub(crate) list: ThrowawayList,
    }

    impl Fixture {
        pub(crate) fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = plain(dir.path());
            let program_files = root.join("Program Files");
            let folders = MachineFolders {
                program_files: vec![program_files.clone(), root.join("Program Files (x86)")],
                common_program_files: program_files.join("Common Files"),
                program_data: root.join("ProgramData"),
            };
            for folder in folders
                .program_files
                .iter()
                .chain([&folders.common_program_files, &folders.program_data])
            {
                fs::create_dir_all(folder).unwrap();
            }
            let resource = root.join("Users").join("Someone").join("REAPER");
            fs::create_dir_all(&resource).unwrap();
            Self {
                _dir: dir,
                root,
                folders,
                resource,
                list: ThrowawayList::create(),
            }
        }

        /// The roots the helper would use, with REAPER's app at
        /// `app_path`.
        pub(crate) fn roots(&self, app_path: Option<&Path>) -> UninstallRoots {
            helper_roots(
                &HelperArgs {
                    pipe: String::new(),
                    parent_pid: 0,
                    resource_path: self.resource.clone(),
                    target_app_path: app_path.map(Path::to_path_buf),
                    target: InstallationKind::Standard,
                    package_ids: Vec::new(),
                },
                &self.folders,
            )
        }

        pub(crate) fn env(&self, runner: Runner) -> TestEnvironment {
            TestEnvironment::new(self.list.views(), &self.root, runner)
        }

        pub(crate) fn common(&self) -> &Path {
            &self.folders.common_program_files
        }
    }

    pub(crate) fn write(path: &Path, contents: &[u8]) -> PathBuf {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        path.to_path_buf()
    }

    pub(crate) fn quoted(path: &Path) -> String {
        format!("\"{}\"", path.display())
    }

    pub(crate) const SURGE_KEY: &str = "RABBIT-Test-Surge_is1";

    /// Surge XT as its Inno installer leaves it, plus the junctions it
    /// makes into its data folder.
    pub(crate) struct Surge {
        pub(crate) uninstaller: PathBuf,
        pub(crate) uninstall_dir: PathBuf,
        pub(crate) vst3: PathBuf,
        pub(crate) clap: PathBuf,
        pub(crate) data_file: PathBuf,
        pub(crate) clap_vendor_dir: PathBuf,
        pub(crate) vst3_vendor_dir: PathBuf,
        pub(crate) clap_junction: PathBuf,
        pub(crate) vst3_junction: PathBuf,
    }

    pub(crate) fn install_surge(fixture: &Fixture) -> Surge {
        let data = fixture.folders.program_data.join("Surge XT");
        let uninstall_dir = data.join("uninstall");
        let uninstaller = write(&uninstall_dir.join("unins000.exe"), b"inno uninstaller");
        write(&uninstall_dir.join("unins000.dat"), b"inno log");
        let data_file = write(&data.join("patches_factory").join("Init.fxp"), b"patch");
        let clap_vendor_dir = fixture.common().join("CLAP").join("Surge Synth Team");
        let vst3_vendor_dir = fixture.common().join("VST3").join("Surge Synth Team");
        let clap = write(&clap_vendor_dir.join("Surge XT.clap"), b"clap");
        let vst3 = vst3_vendor_dir.join("Surge XT.vst3");
        write(
            &vst3
                .join("Contents")
                .join("x86_64-win")
                .join("Surge XT.vst3"),
            b"vst3",
        );
        let clap_junction = clap_vendor_dir.join("SurgeXTData");
        let vst3_junction = vst3_vendor_dir.join("SurgeXTData");
        make_junction(&clap_junction, &data);
        make_junction(&vst3_junction, &data);
        fixture.list.write(
            SURGE_KEY,
            &[
                ("DisplayName", "Surge XT"),
                ("UninstallString", &quoted(&uninstaller)),
                (
                    "QuietUninstallString",
                    &format!("{} /SILENT", quoted(&uninstaller)),
                ),
            ],
        );
        Surge {
            uninstaller,
            uninstall_dir,
            vst3,
            clap,
            data_file,
            clap_vendor_dir,
            vst3_vendor_dir,
            clap_junction,
            vst3_junction,
        }
    }

    /// Surge XT's plan, as its manifest will say it, with `timeout`.
    pub(crate) fn surge_spec(timeout: u64) -> VendorUninstallerSpec {
        serde_json::from_str(&format!(
            r#"{{
                "kind": "inno",
                "locate": [
                    {{ "registry": {{ "hive": "hklm", "key": "{SURGE_KEY}", "value": "UninstallString" }} }}
                ],
                "must_be_in": {{ "dir": "{{program_data}}/Surge XT/uninstall", "only_files": "unins000.*" }},
                "arguments": ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"],
                "timeout_seconds": {timeout},
                "verify_gone": ["{{common_program_files}}/VST3/Surge Synth Team/Surge XT.vst3"],
                "remove_leftovers": [
                    {{ "junction": "{{common_program_files}}/CLAP/Surge Synth Team/SurgeXTData" }},
                    {{ "junction": "{{common_program_files}}/VST3/Surge Synth Team/SurgeXTData" }},
                    {{ "empty_dir": "{{common_program_files}}/CLAP/Surge Synth Team" }},
                    {{ "empty_dir": "{{common_program_files}}/VST3/Surge Synth Team" }}
                ]
            }}"#
        ))
        .unwrap()
    }

    /// A fake Surge uninstaller that removes the plug-ins and its entry in
    /// the list of installed programs, as the real one does, and leaves
    /// the junctions behind, as the real one does too.
    pub(crate) fn surge_uninstaller(fixture: &Fixture, surge: &Surge) -> Runner {
        let (vst3, clap, root) = (surge.vst3.clone(), surge.clap.clone(), fixture.list.root());
        Runner::Fake(Box::new(move |_| {
            fs::remove_dir_all(&vst3).unwrap();
            fs::remove_file(&clap).unwrap();
            root.delete_entry(SURGE_KEY).unwrap();
            FakeRun::exits(0)
        }))
    }

    /// A fake uninstaller that must never start.
    pub(crate) fn never_runs() -> Runner {
        Runner::Fake(Box::new(|command| {
            panic!("{} should not have started", command.program.display())
        }))
    }

    pub(crate) const REAPER_KEY: &str = "RABBIT-Test-REAPER";

    /// A standard REAPER as its NSIS installer leaves it.
    pub(crate) struct Reaper {
        pub(crate) install: PathBuf,
        pub(crate) uninstaller: PathBuf,
        pub(crate) app: PathBuf,
    }

    /// REAPER in Program Files, with `uninstaller` as its `Uninstall.exe`,
    /// and its entry in the list of installed programs.
    pub(crate) fn install_reaper(fixture: &Fixture, uninstaller: &[u8]) -> Reaper {
        let install = fixture.folders.program_files[0].join("REAPER (x64)");
        let app = write(&install.join("reaper.exe"), b"reaper");
        let uninstaller = write(&install.join("Uninstall.exe"), uninstaller);
        fixture.list.write(
            REAPER_KEY,
            &[
                ("DisplayName", "REAPER (x64)"),
                ("DisplayVersion", "7.82"),
                ("InstallLocation", &install.display().to_string()),
                ("UninstallString", &quoted(&uninstaller)),
            ],
        );
        Reaper {
            install,
            uninstaller,
            app,
        }
    }

    /// REAPER's plan, as its manifest will say it, with `arguments`.
    pub(crate) fn reaper_spec(arguments: &[&str]) -> VendorUninstallerSpec {
        let arguments = serde_json::to_string(arguments).unwrap();
        serde_json::from_str(&format!(
            r#"{{
                "kind": "nsis",
                "locate": [
                    {{ "registry": {{ "hive": "hklm", "key": "{REAPER_KEY}", "value": "UninstallString" }} }}
                ],
                "install_location": {{
                    "registry": {{ "hive": "hklm", "key": "{REAPER_KEY}", "value": "InstallLocation" }},
                    "must_equal": "{{app_dir}}"
                }},
                "must_be_in": {{ "under": "{{program_files}}" }},
                "arguments": {arguments},
                "timeout_seconds": 120,
                "still_waiting_seconds": 120,
                "verify_gone": ["{{app_path}}"]
            }}"#
        ))
        .unwrap()
    }

    fn run(
        spec: &VendorUninstallerSpec,
        roots: &UninstallRoots,
        env: &TestEnvironment,
    ) -> (VendorOutcome, Vec<VendorEvent>) {
        let mut events = Vec::new();
        let outcome = run_vendor_uninstaller(
            &VendorJob {
                package_id: "demo",
                spec,
                roots,
            },
            env,
            &mut |event| events.push(event),
        );
        (outcome, events)
    }

    fn locate(
        spec: &VendorUninstallerSpec,
        roots: &UninstallRoots,
        env: &TestEnvironment,
    ) -> Result<PathBuf, VendorRefusal> {
        locate_uninstaller(
            &VendorJob {
                package_id: "demo",
                spec,
                roots,
            },
            env,
        )
        .map(|located| located.program)
    }

    fn refused(outcome: &VendorOutcome) -> &VendorRefusal {
        assert_eq!(outcome.status, VendorStatus::Refused, "{outcome:?}");
        assert!(!outcome.launched);
        outcome.refusal.as_ref().unwrap()
    }

    // ---- Locating the uninstaller.

    #[test]
    fn the_program_is_what_the_entry_names_without_quotes_or_arguments() {
        for (value, program) in [
            (
                r#""C:\Program Files\REAPER (x64)\Uninstall.exe""#,
                r"C:\Program Files\REAPER (x64)\Uninstall.exe",
            ),
            (
                r#""C:\ProgramData\Surge XT\uninstall\unins000.exe" /SILENT"#,
                r"C:\ProgramData\Surge XT\uninstall\unins000.exe",
            ),
            (
                r"C:\Program Files\REAPER (x64)\Uninstall.exe /S _?=C:\Program Files\REAPER (x64)",
                r"C:\Program Files\REAPER (x64)\Uninstall.exe",
            ),
        ] {
            assert_eq!(
                uninstaller_program(value).unwrap(),
                PathBuf::from(program),
                "{value}"
            );
        }
    }

    #[test]
    fn only_a_full_path_on_a_drive_names_an_uninstaller() {
        for value in [
            "Uninstall.exe",
            r"REAPER\Uninstall.exe",
            r"\Program Files\REAPER\Uninstall.exe",
            r"C:Uninstall.exe",
            r"\\server\share\REAPER\Uninstall.exe",
            r"\\?\C:\Program Files\REAPER\Uninstall.exe",
            r"\\.\C:\Program Files\REAPER\Uninstall.exe",
            r"%ProgramFiles%\REAPER\Uninstall.exe",
            r"C:\Program Files\REAPER\..\Uninstall.exe",
            r"C:\",
        ] {
            assert!(
                matches!(
                    uninstaller_program(value),
                    Err(VendorRefusal::NotAFullPath { .. })
                ),
                "{value}: {:?}",
                uninstaller_program(value)
            );
        }
        for value in [r#""C:\Program Files\REAPER\Uninstall.exe"#, r#""""#, ""] {
            assert!(
                matches!(
                    uninstaller_program(value),
                    Err(VendorRefusal::UnclearCommand { .. })
                ),
                "{value}"
            );
        }
    }

    #[test]
    fn surges_uninstaller_is_found_and_trusted_in_its_exact_folder() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let env = fixture.env(never_runs());
        assert_eq!(
            locate(&surge_spec(60), &fixture.roots(None), &env),
            Ok(surge.uninstaller.clone())
        );

        // Quoted and with arguments, or unquoted with arguments, it is the
        // same program.
        for value in [
            format!("{} /VERYSILENT /NORESTART", quoted(&surge.uninstaller)),
            format!("{} /SILENT", surge.uninstaller.display()),
        ] {
            fixture
                .list
                .write(SURGE_KEY, &[("UninstallString", value.as_str())]);
            assert_eq!(
                locate(&surge_spec(60), &fixture.roots(None), &env),
                Ok(surge.uninstaller.clone()),
                "{value}"
            );
        }
    }

    #[test]
    fn no_entry_in_the_machine_list_means_nothing_to_run() {
        let fixture = Fixture::new();
        install_surge(&fixture);
        let env = fixture.env(never_runs());
        let roots = fixture.roots(None);

        // An entry the plan doesn't name.
        let mut spec = surge_spec(60);
        let json = serde_json::to_string(&spec)
            .unwrap()
            .replace(SURGE_KEY, "RABBIT-Test-Missing");
        spec = serde_json::from_str(&json).unwrap();
        assert_eq!(locate(&spec, &roots, &env), Err(VendorRefusal::NotListed));

        // A value in the user's own list, or a plain path, is never
        // believed, even where it would point at the right file.
        let json = serde_json::to_string(&surge_spec(60))
            .unwrap()
            .replace(r#""hive":"hklm""#, r#""hive":"hkcu""#);
        let user_list: VendorUninstallerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(
            locate(&user_list, &roots, &env),
            Err(VendorRefusal::NotListed)
        );
        let json = serde_json::to_string(&surge_spec(60)).unwrap().replace(
            &format!(
                r#"{{"registry":{{"hive":"hklm","key":"{SURGE_KEY}","value":"UninstallString"}}}}"#
            ),
            r#"{"path":"{program_data}/Surge XT/uninstall/unins000.exe"}"#,
        );
        let by_path: VendorUninstallerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(
            locate(&by_path, &roots, &env),
            Err(VendorRefusal::NotListed)
        );
    }

    #[test]
    fn the_first_locate_entry_that_exists_wins() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let env = fixture.env(never_runs());
        let json = serde_json::to_string(&surge_spec(60)).unwrap().replace(
            r#""locate":["#,
            r#""locate":[{"registry":{"hive":"hklm","key":"RABBIT-Test-Missing","value":"UninstallString"}},"#,
        );
        let spec: VendorUninstallerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec.locate.len(), 2);
        assert_eq!(
            locate(&spec, &fixture.roots(None), &env),
            Ok(surge.uninstaller)
        );
    }

    // ---- Trusting Surge XT's exact folder.

    #[test]
    fn a_file_that_doesnt_belong_in_the_folder_refuses_the_run() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let planted = write(&surge.uninstall_dir.join("version.dll"), b"planted");
        let env = fixture.env(never_runs());

        let (outcome, events) = run(&surge_spec(60), &fixture.roots(None), &env);
        assert_eq!(
            refused(&outcome),
            &VendorRefusal::ForeignFile { path: planted }
        );
        assert!(events.is_empty());
        assert!(env.started().is_empty());
        assert!(surge.vst3.exists(), "nothing changed");
    }

    /// A standard user may create files in the vendor's folder, and a file
    /// they create is theirs, even one named like the vendor's files.
    #[test]
    fn a_file_a_standard_user_planted_is_refused_by_its_owner() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let planted = write(&surge.uninstall_dir.join("unins000.dll"), b"planted");
        if rabbit_platform::file_owner_is_trusted(&planted).unwrap() {
            // Run elevated, as on CI, a new file belongs to Administrators,
            // so there is no standard user's file to plant.
            eprintln!("skipped: this test runs with administrator rights");
            return;
        }
        let mut env = fixture.env(never_runs());
        env.owners = Owners::RealExcept(vec![
            surge.uninstaller.clone(),
            surge.uninstall_dir.join("unins000.dat"),
        ]);

        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);
        assert_eq!(
            refused(&outcome),
            &VendorRefusal::UntrustedOwner { path: planted }
        );
        assert!(env.started().is_empty());
    }

    #[test]
    fn a_folder_or_a_link_in_the_folder_refuses_the_run() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let env = fixture.env(never_runs());

        let subfolder = surge.uninstall_dir.join("unins000.d");
        fs::create_dir(&subfolder).unwrap();
        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);
        assert_eq!(
            refused(&outcome),
            &VendorRefusal::FolderOrLink {
                path: subfolder.clone()
            }
        );
        fs::remove_dir(&subfolder).unwrap();

        let elsewhere = fixture.root.join("Elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let junction = surge.uninstall_dir.join("unins000.lnk");
        make_junction(&junction, &elsewhere);
        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);
        assert_eq!(
            refused(&outcome),
            &VendorRefusal::FolderOrLink { path: junction }
        );
        assert!(env.started().is_empty());
    }

    #[test]
    fn an_uninstaller_outside_its_exact_folder_is_refused() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let env = fixture.env(never_runs());

        // One folder up, still under ProgramData.
        let upstairs = write(
            &surge.uninstall_dir.parent().unwrap().join("unins000.exe"),
            b"elsewhere",
        );
        fixture
            .list
            .write(SURGE_KEY, &[("UninstallString", &quoted(&upstairs))]);
        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);
        assert_eq!(
            refused(&outcome),
            &VendorRefusal::WrongFolder { path: upstairs }
        );
        assert!(env.started().is_empty());
    }

    /// The folder's name is right, but it is reached through a junction to
    /// somewhere a standard user could have filled.
    #[test]
    fn an_exact_folder_reached_through_a_junction_is_refused() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let env = fixture.env(never_runs());

        let real = fixture.root.join("Users").join("Someone").join("Surge XT");
        write(&real.join("uninstall").join("unins000.exe"), b"elsewhere");
        let vendor = fixture.folders.program_data.join("Surge XT");
        fs::remove_dir_all(&vendor).unwrap();
        make_junction(&vendor, &real);
        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);
        assert_eq!(
            refused(&outcome),
            &VendorRefusal::WrongFolder {
                path: surge.uninstall_dir
            }
        );
        assert!(env.started().is_empty());
    }

    // ---- Trusting REAPER's install folder.

    #[test]
    fn reapers_uninstaller_must_sit_directly_in_its_install_folder() {
        let fixture = Fixture::new();
        let reaper = install_reaper(&fixture, b"nsis");
        let env = fixture.env(never_runs());
        let roots = fixture.roots(Some(&reaper.app));
        assert_eq!(
            locate(&reaper_spec(&["/S"]), &roots, &env),
            Ok(reaper.uninstaller.clone())
        );

        let deeper = write(&reaper.install.join("Plugins").join("Uninstall.exe"), b"x");
        fixture.list.write(
            REAPER_KEY,
            &[
                ("InstallLocation", &reaper.install.display().to_string()),
                ("UninstallString", &quoted(&deeper)),
            ],
        );
        assert_eq!(
            locate(&reaper_spec(&["/S"]), &roots, &env),
            Err(VendorRefusal::NotInInstallFolder {
                path: deeper,
                folder: reaper.install.clone(),
            })
        );
    }

    #[test]
    fn an_install_folder_that_isnt_the_chosen_reaper_is_refused() {
        let fixture = Fixture::new();
        let reaper = install_reaper(&fixture, b"nsis");
        let env = fixture.env(never_runs());
        let roots = fixture.roots(Some(&reaper.app));

        let other = fixture.folders.program_files[0].join("REAPER");
        fs::create_dir_all(&other).unwrap();
        fixture.list.write(
            REAPER_KEY,
            &[
                ("InstallLocation", &other.display().to_string()),
                ("UninstallString", &quoted(&reaper.uninstaller)),
            ],
        );
        assert_eq!(
            locate(&reaper_spec(&["/S"]), &roots, &env),
            Err(VendorRefusal::InstallLocationMismatch {
                found: other.display().to_string(),
                expected: reaper.install.clone(),
            })
        );

        fixture.list.write(
            REAPER_KEY,
            &[("UninstallString", &quoted(&reaper.uninstaller))],
        );
        assert_eq!(
            locate(&reaper_spec(&["/S"]), &roots, &env),
            Err(VendorRefusal::InstallLocationMissing)
        );

        // Without REAPER's app, there is nothing to compare with.
        fixture.list.write(
            REAPER_KEY,
            &[
                ("InstallLocation", &reaper.install.display().to_string()),
                ("UninstallString", &quoted(&reaper.uninstaller)),
            ],
        );
        assert_eq!(
            locate(&reaper_spec(&["/S"]), &fixture.roots(None), &env),
            Err(VendorRefusal::UnknownFolder {
                token: "{app_dir}".to_string()
            })
        );
    }

    /// The install folder may be spelt with a trailing separator, in quotes
    /// or in another case, and is still the same folder.
    #[test]
    fn an_install_folder_is_compared_however_it_is_spelt() {
        let fixture = Fixture::new();
        let reaper = install_reaper(&fixture, b"nsis");
        let env = fixture.env(never_runs());
        let roots = fixture.roots(Some(&reaper.app));
        let install = reaper.install.display().to_string();
        for spelt in [
            format!("{install}\\"),
            format!("\"{install}\""),
            install.to_uppercase(),
        ] {
            fixture.list.write(
                REAPER_KEY,
                &[
                    ("InstallLocation", spelt.as_str()),
                    ("UninstallString", &quoted(&reaper.uninstaller)),
                ],
            );
            assert!(
                locate(&reaper_spec(&["/S"]), &roots, &env).is_ok(),
                "{spelt}"
            );
        }
    }

    /// `_?=` makes the install folder the one the uninstaller deletes from,
    /// so Program Files itself is never one.
    #[test]
    fn program_files_itself_is_never_an_install_folder() {
        let fixture = Fixture::new();
        let program_files = fixture.folders.program_files[0].clone();
        let app = write(&program_files.join("reaper.exe"), b"reaper");
        let uninstaller = write(&program_files.join("Uninstall.exe"), b"nsis");
        fixture.list.write(
            REAPER_KEY,
            &[
                ("InstallLocation", &program_files.display().to_string()),
                ("UninstallString", &quoted(&uninstaller)),
            ],
        );
        let env = fixture.env(never_runs());
        let (outcome, _) = run(&reaper_spec(&["/S"]), &fixture.roots(Some(&app)), &env);
        assert_eq!(
            refused(&outcome),
            &VendorRefusal::NotBelowProgramFiles {
                folder: program_files
            }
        );

        // Nor is a folder outside it.
        let outside = fixture.root.join("Tools").join("REAPER");
        let app = write(&outside.join("reaper.exe"), b"reaper");
        let uninstaller = write(&outside.join("Uninstall.exe"), b"nsis");
        fixture.list.write(
            REAPER_KEY,
            &[
                ("InstallLocation", &outside.display().to_string()),
                ("UninstallString", &quoted(&uninstaller)),
            ],
        );
        let (outcome, _) = run(&reaper_spec(&["/S"]), &fixture.roots(Some(&app)), &env);
        assert_eq!(
            refused(&outcome),
            &VendorRefusal::NotBelowProgramFiles { folder: outside }
        );
        assert!(env.started().is_empty());
    }

    // ---- Running REAPER's NSIS uninstaller.

    const REPORT_TO: &str = "RABBIT_TEST_VENDOR_REPORT_COMMAND_LINE_TO";
    const DELETE_FILE: &str = "RABBIT_TEST_VENDOR_DELETE_FILE";
    const DELETE_ENTRY_IN: &str = "RABBIT_TEST_VENDOR_DELETE_ENTRY_IN";
    const DELETE_ENTRY: &str = "RABBIT_TEST_VENDOR_DELETE_ENTRY";
    const CHILD: &str = "uninstall::vendor::tests::windows::child_acts_as_an_uninstaller";

    /// Does nothing in a normal run. Started by a test as a fixture
    /// uninstaller, it reports its exact command line, deletes the file and
    /// the throwaway registry entry it was told to, and exits.
    #[test]
    fn child_acts_as_an_uninstaller() {
        let Some(report) = std::env::var_os(REPORT_TO) else {
            return;
        };
        // SAFETY: GetCommandLineW returns a NUL-terminated string that
        // lives as long as the process.
        let line = unsafe {
            let start = windows_sys::Win32::System::Environment::GetCommandLineW();
            let mut length = 0;
            while *start.add(length) != 0 {
                length += 1;
            }
            String::from_utf16(std::slice::from_raw_parts(start, length)).unwrap()
        };
        fs::write(report, line).unwrap();
        if let Some(file) = std::env::var_os(DELETE_FILE) {
            fs::remove_file(file).unwrap();
        }
        if let (Ok(list), Ok(entry)) = (std::env::var(DELETE_ENTRY_IN), std::env::var(DELETE_ENTRY))
        {
            assert!(list.starts_with("Software\\RABBIT-test-"), "{list}");
            HkcuUninstallRoot::at(list).delete_entry(&entry).unwrap();
        }
        std::process::exit(0);
    }

    /// The copy runs from its own new folder, `_?=` names the install
    /// folder last and unquoted, and the folder is gone again afterwards.
    #[test]
    fn reapers_uninstaller_runs_as_a_checked_copy_with_its_folder_last_and_unquoted() {
        let fixture = Fixture::new();
        let this_binary = fs::read(std::env::current_exe().unwrap()).unwrap();
        let reaper = install_reaper(&fixture, &this_binary);
        let report = fixture.root.join("command line.txt");
        let env = fixture.env(Runner::Real(vec![
            (REPORT_TO.to_string(), report.clone().into()),
            (DELETE_FILE.to_string(), reaper.app.clone().into()),
            (DELETE_ENTRY_IN.to_string(), fixture.list.path().into()),
            (DELETE_ENTRY.to_string(), REAPER_KEY.into()),
        ]));
        let spec = reaper_spec(&["--exact", CHILD, "--test-threads=1", "--quiet", "/S"]);

        let (outcome, events) = run(&spec, &fixture.roots(Some(&reaper.app)), &env);

        assert_eq!(outcome.status, VendorStatus::Removed, "{outcome:?}");
        assert_eq!(outcome.exit_code, Some(0));
        assert!(outcome.launched);
        assert_eq!(outcome.uninstaller.as_ref(), Some(&reaper.uninstaller));
        assert_eq!(events, [VendorEvent::Launched]);
        let [started] = env.started().try_into().unwrap();
        let working = started.program.parent().unwrap().to_path_buf();
        assert_eq!(working.parent(), Some(env.working_parent.as_path()));
        assert_eq!(started.working_directory.as_ref(), Some(&working));
        let line = fs::read_to_string(&report).unwrap();
        assert!(
            line.starts_with(&quoted(&started.program)),
            "the copy ran, not the original: {line}"
        );
        assert!(
            line.ends_with(&format!(" /S _?={}", reaper.install.display())),
            "{line}"
        );
        assert!(!working.exists(), "the working folder is removed");
        assert!(outcome.problems.is_empty(), "{:?}", outcome.problems);
        assert!(reaper.uninstaller.exists(), "the original is never run");
    }

    #[test]
    fn a_copy_that_differs_from_the_original_never_runs() {
        let fixture = Fixture::new();
        let reaper = install_reaper(&fixture, b"nsis");
        let mut env = fixture.env(never_runs());
        env.corrupt_copies = true;

        let (outcome, events) = run(
            &reaper_spec(&["/S"]),
            &fixture.roots(Some(&reaper.app)),
            &env,
        );
        assert_eq!(refused(&outcome), &VendorRefusal::CopyDiffers);
        assert!(events.is_empty());
        assert!(env.started().is_empty());
        assert!(
            fs::read_dir(&env.working_parent).unwrap().next().is_none(),
            "the working folder is removed"
        );
        assert!(reaper.app.exists());
    }

    #[test]
    fn nsis_exit_codes_say_cancelled_or_failed() {
        let fixture = Fixture::new();
        let reaper = install_reaper(&fixture, b"nsis");
        let roots = fixture.roots(Some(&reaper.app));
        for (code, status) in [
            (1, VendorStatus::Cancelled),
            (2, VendorStatus::Failed),
            (5, VendorStatus::Failed),
        ] {
            let env = fixture.env(Runner::Fake(Box::new(move |_| FakeRun::exits(code))));
            let (outcome, events) = run(&reaper_spec(&["/S"]), &roots, &env);
            assert_eq!(outcome.status, status, "{code}");
            assert_eq!(outcome.exit_code, Some(code));
            assert!(outcome.launched);
            assert_eq!(events, [VendorEvent::Launched]);
            // Nothing went, and the report says so.
            assert_eq!(outcome.still_present, std::slice::from_ref(&reaper.app));
            assert!(outcome.still_listed);
        }
    }

    #[test]
    fn exit_zero_with_reaper_still_there_is_a_failure() {
        let fixture = Fixture::new();
        let reaper = install_reaper(&fixture, b"nsis");
        let env = fixture.env(Runner::Fake(Box::new(|_| FakeRun::exits(0))));
        let (outcome, _) = run(
            &reaper_spec(&["/S"]),
            &fixture.roots(Some(&reaper.app)),
            &env,
        );
        assert_eq!(outcome.status, VendorStatus::Failed);
        assert_eq!(outcome.still_present, [reaper.app]);
        assert!(outcome.still_listed);
    }

    #[test]
    fn an_uninstaller_still_running_at_the_time_limit_is_left_alone() {
        let fixture = Fixture::new();
        let reaper = install_reaper(&fixture, b"nsis");
        let env = fixture.env(Runner::Fake(Box::new(|_| FakeRun {
            end: ProgramEnd::StillRunning,
            still_waiting: true,
        })));
        let (outcome, events) = run(
            &reaper_spec(&["/S"]),
            &fixture.roots(Some(&reaper.app)),
            &env,
        );
        assert_eq!(outcome.status, VendorStatus::StillRunning);
        assert_eq!(outcome.exit_code, None);
        assert!(outcome.launched);
        assert_eq!(
            events,
            [
                VendorEvent::Launched,
                VendorEvent::StillWaiting { seconds: 120 }
            ]
        );
        // Its copy is in use, so its folder stays, and the report says so.
        assert!(matches!(
            outcome.problems.as_slice(),
            [super::super::VendorProblem::WorkingFolderNotRemoved { .. }]
        ));
    }

    // ---- Running Surge XT's Inno uninstaller.

    #[test]
    fn surge_goes_and_its_junctions_go_as_links_without_touching_their_target() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let env = fixture.env(surge_uninstaller(&fixture, &surge));

        let (outcome, events) = run(&surge_spec(60), &fixture.roots(None), &env);

        assert_eq!(outcome.status, VendorStatus::Removed, "{outcome:?}");
        assert_eq!(events, [VendorEvent::Launched]);
        let [started] = env.started().try_into().unwrap();
        assert_eq!(started.program, surge.uninstaller, "Inno runs in place");
        assert_eq!(
            started.arguments,
            ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]
        );
        assert_eq!(started.nsis_in_place, None);
        assert_eq!(
            outcome.removed_leftovers,
            [
                LeftoverOutcome {
                    path: surge.clap_junction.clone(),
                    kind: LeftoverKind::Junction,
                    result: LeftoverResult::Removed,
                },
                LeftoverOutcome {
                    path: surge.vst3_junction.clone(),
                    kind: LeftoverKind::Junction,
                    result: LeftoverResult::Removed,
                },
                LeftoverOutcome {
                    path: surge.clap_vendor_dir.clone(),
                    kind: LeftoverKind::EmptyDir,
                    result: LeftoverResult::Removed,
                },
                LeftoverOutcome {
                    path: surge.vst3_vendor_dir.clone(),
                    kind: LeftoverKind::EmptyDir,
                    result: LeftoverResult::Removed,
                },
            ]
        );
        assert!(fs::symlink_metadata(&surge.clap_junction).is_err());
        assert!(!surge.clap_vendor_dir.exists() && !surge.vst3_vendor_dir.exists());
        // What the junctions pointed to is untouched.
        assert_eq!(fs::read(&surge.data_file).unwrap(), b"patch");
        assert!(surge.uninstaller.exists());
    }

    /// Another program from the same vendor keeps its folder; something
    /// that should be a junction and isn't stays, and is reported.
    #[test]
    fn a_folder_with_something_else_in_it_and_a_folder_posing_as_a_junction_stay() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let other = write(&surge.clap_vendor_dir.join("Shortcircuit XT.clap"), b"sxt");
        fs::remove_dir(&surge.vst3_junction).unwrap();
        let posing = write(&surge.vst3_junction.join("keep.txt"), b"mine");
        let env = fixture.env(surge_uninstaller(&fixture, &surge));

        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);

        assert_eq!(outcome.status, VendorStatus::RemovedWithLeftovers);
        let results: Vec<&LeftoverResult> = outcome
            .removed_leftovers
            .iter()
            .map(|leftover| &leftover.result)
            .collect();
        assert_eq!(
            results,
            [
                &LeftoverResult::Removed,
                &LeftoverResult::Refused {
                    refusal: PathRefusal::NotALink
                },
                &LeftoverResult::NotEmpty,
                &LeftoverResult::NotEmpty,
            ]
        );
        assert!(other.exists());
        assert_eq!(fs::read(&posing).unwrap(), b"mine");
        assert_eq!(fs::read(&surge.data_file).unwrap(), b"patch");
    }

    /// Inno may still be at work after it returns, so RABBIT checks every
    /// few seconds until the files and the entry are gone.
    #[test]
    fn surge_is_checked_again_until_its_files_are_gone() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let (vst3, clap, list) = (surge.vst3.clone(), surge.clap.clone(), fixture.list.root());
        let mut env = fixture.env(Runner::Fake(Box::new(|_| FakeRun::exits(0))));
        env.clock = Some(VirtualClock::new(move |elapsed| {
            if elapsed >= Duration::from_secs(6) && vst3.exists() {
                fs::remove_dir_all(&vst3).unwrap();
                fs::remove_file(&clap).unwrap();
                list.delete_entry(SURGE_KEY).unwrap();
            }
        }));

        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);

        assert_eq!(outcome.status, VendorStatus::Removed, "{outcome:?}");
        let waited = env.clock.as_ref().unwrap().elapsed();
        assert!(
            waited >= Duration::from_secs(6) && waited < Duration::from_secs(10),
            "{waited:?}"
        );
        assert!(!surge.clap_vendor_dir.exists());
    }

    #[test]
    fn surge_still_there_at_the_time_limit_fails_and_its_leftovers_stay() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let mut env = fixture.env(Runner::Fake(Box::new(|_| FakeRun::exits(0))));
        env.clock = Some(VirtualClock::new(|_| {}));

        let (outcome, _) = run(&surge_spec(20), &fixture.roots(None), &env);

        assert_eq!(outcome.status, VendorStatus::Failed);
        assert_eq!(outcome.still_present, std::slice::from_ref(&surge.vst3));
        assert!(outcome.still_listed);
        assert_eq!(
            env.clock.as_ref().unwrap().elapsed(),
            Duration::from_secs(20)
        );
        assert!(outcome.removed_leftovers.is_empty());
        assert!(fs::symlink_metadata(&surge.clap_junction).is_ok());
    }

    #[test]
    fn files_windows_removes_at_the_next_restart_count_as_pending() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let (clap, list) = (surge.clap.clone(), fixture.list.root());
        let mut env = fixture.env(Runner::Fake(Box::new(move |_| {
            // The VST3 is in use: it goes at the next restart.
            fs::remove_file(&clap).unwrap();
            list.delete_entry(SURGE_KEY).unwrap();
            FakeRun::exits(0)
        })));
        let in_use = surge
            .vst3
            .join("Contents")
            .join("x86_64-win")
            .join("Surge XT.vst3");
        env.pending = vec![
            PendingFileRename {
                source: in_use.clone(),
                target: None,
            },
            PendingFileRename {
                source: fixture.root.join("Unrelated.dll"),
                target: None,
            },
        ];
        env.clock = Some(VirtualClock::new(|_| {}));

        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);

        assert_eq!(outcome.status, VendorStatus::RemovedPendingRestart);
        assert_eq!(outcome.pending_restart, [in_use]);
        assert!(!outcome.still_listed);
        assert_eq!(
            env.clock.as_ref().unwrap().elapsed(),
            Duration::ZERO,
            "no need to keep checking"
        );
        // Leftovers wait until everything is gone.
        assert!(
            outcome
                .removed_leftovers
                .iter()
                .all(|leftover| leftover.result == LeftoverResult::Skipped)
        );
        assert!(fs::symlink_metadata(&surge.clap_junction).is_ok());
    }

    #[test]
    fn an_inno_failure_reads_as_failed_and_removes_no_leftovers() {
        let fixture = Fixture::new();
        let surge = install_surge(&fixture);
        let env = fixture.env(Runner::Fake(Box::new(|_| FakeRun::exits(5))));
        let (outcome, _) = run(&surge_spec(60), &fixture.roots(None), &env);
        assert_eq!(outcome.status, VendorStatus::Failed);
        assert_eq!(outcome.exit_code, Some(5));
        assert!(outcome.removed_leftovers.is_empty());
        assert!(fs::symlink_metadata(&surge.vst3_junction).is_ok());
    }

    /// The test environment itself never starts anything outside its
    /// fixture folder, whatever a plan names.
    #[test]
    fn tests_never_start_a_program_outside_their_fixture() {
        let fixture = Fixture::new();
        let env = fixture.env(Runner::Real(Vec::new()));
        let command = crate::uninstall::vendor::VendorCommand {
            program: PathBuf::from(r"C:\Windows\System32\cmd.exe"),
            arguments: Vec::new(),
            nsis_in_place: None,
            working_directory: None,
        };
        let error = crate::uninstall::vendor::VendorEnvironment::run_program(
            &env,
            &command,
            crate::uninstall::vendor::RunLimits {
                timeout: Duration::from_secs(1),
                still_waiting_after: Duration::from_secs(1),
            },
            &mut || panic!("started"),
            &mut || {},
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    /// Checks this computer's real Surge XT and REAPER uninstallers as the
    /// elevated helper would, reading HKLM and the real folders, and runs
    /// nothing: it only locates and checks. Ignored, because it depends on
    /// what is installed; run it by hand with `--ignored` before a hardware
    /// test.
    #[test]
    #[ignore = "reads this computer's real uninstallers; run it by hand"]
    fn the_real_uninstallers_pass_the_checks_without_running() {
        use crate::uninstall::vendor::SystemVendorEnvironment;

        let env = SystemVendorEnvironment::new();
        let machine = UninstallViews::local_machine();
        let surge_key = "69F3FE96-DEEC-4C7C-B72D-E8957EC8411B_is1";
        let surge: VendorUninstallerSpec = serde_json::from_str(
            &serde_json::to_string(&surge_spec(600))
                .unwrap()
                .replace(SURGE_KEY, surge_key),
        )
        .unwrap();
        let reaper: VendorUninstallerSpec = serde_json::from_str(
            &serde_json::to_string(&reaper_spec(&["/S"]))
                .unwrap()
                .replace(REAPER_KEY, "REAPER"),
        )
        .unwrap();
        let app_path = rabbit_platform::read_uninstall_install_location("REAPER")
            .map(|folder| PathBuf::from(folder).join("reaper.exe"));
        let resource = rabbit_platform::user_appdata_dir().unwrap().join("REAPER");
        let roots = helper_roots(
            &HelperArgs {
                pipe: String::new(),
                parent_pid: 0,
                resource_path: resource,
                target_app_path: app_path,
                target: InstallationKind::Standard,
                package_ids: Vec::new(),
            },
            &MachineFolders::of_this_computer().unwrap(),
        );
        for (key, spec) in [(surge_key, &surge), ("REAPER", &reaper)] {
            let located = locate_uninstaller(
                &VendorJob {
                    package_id: key,
                    spec,
                    roots: &roots,
                },
                &env,
            );
            eprintln!("{key}: {located:?}");
            if machine.key_exists(key).unwrap() {
                assert!(located.is_ok(), "{key}: {located:?}");
            }
        }
    }
}
