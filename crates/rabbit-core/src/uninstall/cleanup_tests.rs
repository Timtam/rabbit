//! Tests for the machine cleanup.
//!
//! Nothing here reaches the real system's machine-wide places: every
//! registry entry is in a throwaway key, `HKCU\Software\RABBIT-test-<unique>`,
//! standing in for HKLM's list of installed programs; every folder is a
//! fixture in a temporary folder, with a folder and a key name no real
//! installer uses; and the test environment deletes only inside its fixture
//! folder, and never from HKLM.

use std::path::{Path, PathBuf};

use rabbit_platform::UninstallViews;

use super::{
    CleanupJob, CleanupOutcome, CleanupResult, CleanupStatus, CleanupStep, CleanupTarget,
    PlannedMachineCleanup, check_machine_cleanup, run_machine_cleanup,
};
use crate::package::uninstall::{MachineCleanupSpec, RegistryHive, UninstallPath};
use crate::uninstall::paths::UninstallRoots;
use crate::uninstall::vendor::tests::{Runner, TestEnvironment};
use crate::uninstall::vendor::{NothingListed, VendorLookup, VendorRefusal};

/// The key and the folder the tests' cleanup names. No installer uses
/// either, so even a test gone wrong couldn't reach a real one. The helper's
/// tests use them too.
pub(crate) const KEY: &str = "RABBIT-Test-JawsScripts";
pub(crate) const FOLDER: &str = "RABBIT-Test-JawsScripts";
const BY_HAND_KEY: &str = "uninstall-by-hand-jaws-scripts";

/// A cleanup shaped like the JAWS scripts' one, deleting `files` in that
/// order.
fn spec_with_files(files: &[&str]) -> MachineCleanupSpec {
    let files = serde_json::to_string(files).unwrap();
    serde_json::from_str(&format!(
        r#"{{
            "dir": "{{program_files_x86}}/{FOLDER}",
            "files": {files},
            "registry": {{
                "hive": "hklm",
                "key": "{KEY}",
                "view": "bit32",
                "values": {{
                    "UninstallDirectory": "{{program_files_x86}}/{FOLDER}",
                    "Uninstall.dat": "{{program_files_x86}}/{FOLDER}/Uninstall.dat"
                }}
            }},
            "by_hand_key": "{BY_HAND_KEY}"
        }}"#
    ))
    .unwrap()
}

/// The JAWS scripts' kind of cleanup of [`FOLDER`] and [`KEY`].
pub(crate) fn spec() -> MachineCleanupSpec {
    spec_with_files(&["Uninstall.dat", "Uninstall.exe"])
}

/// A runner for environments that must never start anything.
fn panics_if_run() -> Runner {
    Runner::Fake(Box::new(|command| {
        panic!("{} should not have started", command.program.display())
    }))
}

/// An environment whose list of installed programs is a key nobody
/// creates, for cleanups refused before anything is read.
fn untouched_env(sandbox: &Path) -> TestEnvironment {
    TestEnvironment::new(
        UninstallViews::current_user_at("Software\\RABBIT-test-never-created"),
        sandbox,
        panics_if_run(),
    )
}

/// The check and the run both refuse with `expected`, and the run took no
/// step. Off Windows a run refuses before it checks anything.
fn assert_refused_at_once(job: &CleanupJob<'_>, env: &TestEnvironment, expected: VendorRefusal) {
    assert_eq!(check_machine_cleanup(job, env), Err(expected.clone()));
    let outcome = run_machine_cleanup(job, env);
    assert_eq!(outcome.status, CleanupStatus::Refused, "{outcome:?}");
    let expected = if cfg!(windows) {
        expected
    } else {
        VendorRefusal::Unsupported
    };
    assert_eq!(outcome.refusal, Some(expected));
    assert!(outcome.steps.is_empty(), "{outcome:?}");
}

#[test]
fn a_cleanup_outcome_crosses_the_pipe_unchanged() {
    let folder = PathBuf::from(r"C:\Program Files (x86)\Demo Scripts");
    let outcome = CleanupOutcome {
        status: CleanupStatus::Failed,
        steps: vec![
            CleanupStep {
                target: CleanupTarget::File {
                    path: folder.join("Uninstall.dat"),
                },
                result: CleanupResult::Removed,
            },
            CleanupStep {
                target: CleanupTarget::File {
                    path: folder.join("Uninstall.exe"),
                },
                result: CleanupResult::NotFound,
            },
            CleanupStep {
                target: CleanupTarget::Folder {
                    path: folder.clone(),
                },
                result: CleanupResult::Removed,
            },
            CleanupStep {
                target: CleanupTarget::RegistryKey {
                    key:
                        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Demo"
                            .to_string(),
                },
                result: CleanupResult::Failed {
                    error: "denied".to_string(),
                },
            },
        ],
        still_present: vec![r"HKLM\SOFTWARE\WOW6432Node\Demo".to_string()],
        error: Some("denied".to_string()),
        ..CleanupOutcome::refused("demo", VendorRefusal::RegistryKeyHasSubkeys)
    };
    let line = serde_json::to_string(&outcome).unwrap();
    assert!(!line.contains('\n'));
    assert_eq!(
        serde_json::from_str::<CleanupOutcome>(&line).unwrap(),
        outcome
    );
    assert!(outcome.removed_anything());

    let planned = PlannedMachineCleanup {
        folder: Some(folder.clone()),
        files: vec![folder.join("Uninstall.dat"), folder.join("Uninstall.exe")],
        registry_key: Some(r"HKLM\SOFTWARE\WOW6432Node\Demo".to_string()),
        by_hand_key: BY_HAND_KEY.to_string(),
    };
    let line = serde_json::to_string(&planned).unwrap();
    assert_eq!(
        serde_json::from_str::<PlannedMachineCleanup>(&line).unwrap(),
        planned
    );
}

#[test]
fn only_a_removed_step_counts_as_removing_something() {
    assert!(!CleanupOutcome::refused("demo", VendorRefusal::NotListed).removed_anything());
    let lost = CleanupOutcome::lost("demo", "the pipe broke");
    assert_eq!(lost.status, CleanupStatus::Lost);
    assert_eq!(lost.error.as_deref(), Some("the pipe broke"));
    assert!(!lost.removed_anything());
    let gone_already = CleanupOutcome {
        steps: vec![CleanupStep {
            target: CleanupTarget::RegistryKey {
                key: KEY.to_string(),
            },
            result: CleanupResult::NotFound,
        }],
        ..CleanupOutcome::new("demo", CleanupStatus::Removed)
    };
    assert!(!gone_already.removed_anything());
}

/// Whatever environment a test hands in, a cleanup never reads or deletes
/// in Windows' real list. The key and the folder here don't exist either,
/// so even without the refusal nothing could be deleted.
#[test]
fn a_cleanup_never_runs_against_the_real_list_in_a_test() {
    let dir = tempfile::tempdir().unwrap();
    let spec: MachineCleanupSpec = serde_json::from_str(
        &serde_json::to_string(&spec())
            .unwrap()
            .replace(KEY, "RABBIT-Test-Never-Created"),
    )
    .unwrap();
    let roots = UninstallRoots {
        program_files_x86: Some(dir.path().join("Program Files (x86)")),
        ..UninstallRoots::new(&dir.path().join("REAPER"))
    };
    let env = TestEnvironment::new(UninstallViews::local_machine(), dir.path(), panics_if_run());
    let outcome = run_machine_cleanup(
        &CleanupJob {
            package_id: "demo",
            spec: &spec,
            roots: &roots,
        },
        &env,
    );
    assert_eq!(outcome.status, CleanupStatus::Refused, "{outcome:?}");
    assert_eq!(outcome.refusal, Some(VendorRefusal::Unsupported));
    assert!(outcome.steps.is_empty());
    assert!(
        outcome
            .error
            .as_deref()
            .is_some_and(|error| error.contains("tests never")),
        "{outcome:?}"
    );
}

/// A lookup that didn't look can't say the folder and the key are gone, so
/// it refuses a cleanup instead of planning none.
#[test]
fn nothing_listed_refuses_every_machine_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let roots = UninstallRoots {
        program_files_x86: Some(dir.path().join("Program Files (x86)")),
        ..UninstallRoots::new(&dir.path().join("REAPER"))
    };
    let spec = spec();
    let job = CleanupJob {
        package_id: "demo",
        spec: &spec,
        roots: &roots,
    };
    assert_eq!(
        NothingListed.machine_cleanup(&job),
        Err(VendorRefusal::NotListed)
    );
}

/// Without a `{program_files_x86}`, as off Windows, there is no folder to
/// check, so the cleanup refuses and changes nothing.
#[test]
fn a_cleanup_without_program_files_x86_refuses_with_unknown_folder() {
    let dir = tempfile::tempdir().unwrap();
    let roots = UninstallRoots::new(&dir.path().join("REAPER"));
    assert_eq!(roots.program_files_x86, None);
    let spec = spec();
    assert_refused_at_once(
        &CleanupJob {
            package_id: "demo",
            spec: &spec,
            roots: &roots,
        },
        &untouched_env(dir.path()),
        VendorRefusal::UnknownFolder {
            token: "{program_files_x86}".to_string(),
        },
    );
}

/// A cleanup validation would refuse, built by hand, is refused before
/// anything is read: a key outside HKLM, or a folder that isn't exactly one
/// folder strictly below `{program_files_x86}`.
#[test]
fn a_hand_made_cleanup_validation_would_refuse_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let x86 = dir.path().join("Program Files (x86)");
    let roots = UninstallRoots {
        program_files: vec![dir.path().join("Program Files"), x86.clone()],
        program_files_x86: Some(x86),
        program_data: Some(dir.path().join("ProgramData")),
        ..UninstallRoots::new(&dir.path().join("REAPER"))
    };
    let mut in_hkcu = spec();
    in_hkcu.registry.hive = RegistryHive::Hkcu;
    let mut cleanups = vec![in_hkcu];
    // Both Program Files folders, a folder elsewhere, and Program Files
    // (x86) itself.
    for folder in [
        "{program_files}/Demo",
        "{program_data}/Demo",
        "{program_files_x86}",
    ] {
        let mut cleanup = spec();
        cleanup.dir = UninstallPath::new(folder);
        cleanups.push(cleanup);
    }
    let env = untouched_env(dir.path());
    for spec in &cleanups {
        assert_refused_at_once(
            &CleanupJob {
                package_id: "demo",
                spec,
                roots: &roots,
            },
            &env,
            VendorRefusal::NotAVendorPlan,
        );
    }
}

/// Everything that needs Windows: the registry, junctions and owners.
#[cfg(windows)]
pub(crate) mod windows {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use rabbit_platform::{
        HkcuUninstallRoot, LockedFile, PendingFileRename, RegistryValue, RegistryView,
        UninstallEntry, UninstallViews,
    };

    use super::super::{
        CleanupJob, CleanupOutcome, CleanupResult, CleanupStatus, CleanupStep, CleanupTarget,
        PlannedMachineCleanup, check_machine_cleanup, run_machine_cleanup,
    };
    use super::{BY_HAND_KEY, FOLDER, KEY, spec, spec_with_files};
    use crate::package::uninstall::MachineCleanupSpec;
    use crate::uninstall::paths::make_junction;
    use crate::uninstall::vendor::tests::windows::{Fixture, never_runs, write};
    use crate::uninstall::vendor::tests::{Owners, TestEnvironment};
    use crate::uninstall::vendor::{
        LookupOnly, ProgramEnd, RunLimits, VendorCommand, VendorEnvironment, VendorRefusal,
    };

    /// The scripts' uninstaller as their installer leaves it.
    pub(crate) struct Installed {
        pub(crate) folder: PathBuf,
        pub(crate) dat: PathBuf,
        pub(crate) exe: PathBuf,
    }

    /// The cleanup's folder in the fixture's 32-bit Program Files.
    fn scripts_folder(fixture: &Fixture) -> PathBuf {
        fixture.folders.program_files_x86.join(FOLDER)
    }

    /// The folder with its two files, and no registry entry.
    fn install_folder(fixture: &Fixture) -> Installed {
        let folder = scripts_folder(fixture);
        Installed {
            dat: write(&folder.join("Uninstall.dat"), b"uninstall log"),
            exe: write(&folder.join("Uninstall.exe"), b"uninstaller"),
            folder,
        }
    }

    /// The registry entry, its values naming `folder` as the installer
    /// writes them.
    pub(crate) fn write_key(fixture: &Fixture, folder: &Path) {
        fixture.list.write(
            KEY,
            &[
                ("UninstallDirectory", &folder.display().to_string()),
                (
                    "Uninstall.dat",
                    &folder.join("Uninstall.dat").display().to_string(),
                ),
            ],
        );
    }

    /// The folder, its files and the registry entry.
    pub(crate) fn install(fixture: &Fixture) -> Installed {
        let installed = install_folder(fixture);
        write_key(fixture, &installed.folder);
        installed
    }

    fn key_there(fixture: &Fixture) -> bool {
        fixture.list.views().key_exists(KEY).unwrap()
    }

    fn shown_key(fixture: &Fixture) -> String {
        fixture.list.views().display_key(KEY, RegistryView::Bit32)
    }

    fn check_with(
        fixture: &Fixture,
        spec: &MachineCleanupSpec,
        env: &dyn VendorEnvironment,
    ) -> Result<Option<PlannedMachineCleanup>, VendorRefusal> {
        let roots = fixture.roots(None);
        check_machine_cleanup(
            &CleanupJob {
                package_id: "jaws-test",
                spec,
                roots: &roots,
            },
            env,
        )
    }

    fn check(
        fixture: &Fixture,
        env: &dyn VendorEnvironment,
    ) -> Result<Option<PlannedMachineCleanup>, VendorRefusal> {
        check_with(fixture, &spec(), env)
    }

    fn run_with(
        fixture: &Fixture,
        spec: &MachineCleanupSpec,
        env: &dyn VendorEnvironment,
    ) -> CleanupOutcome {
        let roots = fixture.roots(None);
        run_machine_cleanup(
            &CleanupJob {
                package_id: "jaws-test",
                spec,
                roots: &roots,
            },
            env,
        )
    }

    fn run(fixture: &Fixture, env: &dyn VendorEnvironment) -> CleanupOutcome {
        run_with(fixture, &spec(), env)
    }

    fn step(target: CleanupTarget, result: CleanupResult) -> CleanupStep {
        CleanupStep { target, result }
    }

    fn file(path: &Path) -> CleanupTarget {
        CleanupTarget::File {
            path: path.to_path_buf(),
        }
    }

    fn folder_target(path: &Path) -> CleanupTarget {
        CleanupTarget::Folder {
            path: path.to_path_buf(),
        }
    }

    fn key_target(fixture: &Fixture) -> CleanupTarget {
        CleanupTarget::RegistryKey {
            key: shown_key(fixture),
        }
    }

    /// The check and the run both refuse with `expected`, and everything in
    /// `kept` is still there afterwards.
    fn assert_refused(
        fixture: &Fixture,
        env: &TestEnvironment,
        expected: VendorRefusal,
        kept: &[&Path],
    ) {
        assert_eq!(check(fixture, env), Err(expected.clone()));
        let outcome = run(fixture, env);
        assert_eq!(outcome.status, CleanupStatus::Refused, "{outcome:?}");
        assert_eq!(outcome.refusal, Some(expected));
        assert!(outcome.steps.is_empty(), "{outcome:?}");
        for path in kept {
            assert!(
                fs::symlink_metadata(path).is_ok(),
                "{} is gone",
                path.display()
            );
        }
    }

    /// The fixture's environment, with something changed between the
    /// cleanup's checks and its deletes, as a test asks. Everything else
    /// passes through to `inner`, whose sandbox still bounds every delete.
    struct Changing {
        inner: TestEnvironment,
        /// Trusted when their owner is first asked, as the checks before
        /// the first delete do, and untrusted every later time.
        untrusted_later: Vec<PathBuf>,
        owners_asked: Mutex<Vec<PathBuf>>,
        /// Runs after each file is deleted, with its path.
        after_file: Box<dyn Fn(&Path) + Send + Sync>,
        /// Say the folder was deleted, but leave it.
        folder_stays: bool,
        /// Delete the key, but say it was gone already.
        key_gone_already: bool,
    }

    impl Changing {
        fn new(inner: TestEnvironment) -> Self {
            Self {
                inner,
                untrusted_later: Vec::new(),
                owners_asked: Mutex::new(Vec::new()),
                after_file: Box::new(|_| {}),
                folder_stays: false,
                key_gone_already: false,
            }
        }
    }

    impl VendorEnvironment for Changing {
        fn uninstall_views(&self) -> &UninstallViews {
            self.inner.uninstall_views()
        }

        fn owner_is_trusted(&self, path: &Path) -> io::Result<bool> {
            let mut asked = self.owners_asked.lock().unwrap();
            let asked_before = asked.iter().any(|asked| asked == path);
            asked.push(path.to_path_buf());
            if asked_before && self.untrusted_later.iter().any(|later| later == path) {
                return Ok(false);
            }
            self.inner.owner_is_trusted(path)
        }

        fn create_working_folder(&self) -> io::Result<PathBuf> {
            self.inner.create_working_folder()
        }

        fn copy_file(&self, from: &Path, to: &Path) -> io::Result<()> {
            self.inner.copy_file(from, to)
        }

        fn run_program(
            &self,
            command: &VendorCommand,
            limits: RunLimits,
            on_started: &mut dyn FnMut(),
            on_still_waiting: &mut dyn FnMut(),
        ) -> io::Result<ProgramEnd> {
            self.inner
                .run_program(command, limits, on_started, on_still_waiting)
        }

        fn pending_file_renames(&self) -> io::Result<Vec<PendingFileRename>> {
            self.inner.pending_file_renames()
        }

        fn now(&self) -> Instant {
            self.inner.now()
        }

        fn sleep(&self, duration: Duration) {
            self.inner.sleep(duration);
        }

        fn remove_file_no_follow(&self, path: &Path) -> io::Result<()> {
            self.inner.remove_file_no_follow(path)?;
            (self.after_file)(path);
            Ok(())
        }

        fn remove_empty_dir(&self, path: &Path) -> io::Result<()> {
            if self.folder_stays {
                return Ok(());
            }
            self.inner.remove_empty_dir(path)
        }

        fn delete_uninstall_entry(&self, key: &str, view: RegistryView) -> io::Result<bool> {
            let deleted = self.inner.delete_uninstall_entry(key, view)?;
            Ok(deleted && !self.key_gone_already)
        }
    }

    /// The step at `index` deleted nothing, because a check right before
    /// its delete refused.
    fn assert_refused_now(outcome: &CleanupOutcome, index: usize, target: CleanupTarget) {
        let step = &outcome.steps[index];
        assert_eq!(step.target, target, "{outcome:?}");
        assert!(
            matches!(&step.result, CleanupResult::Failed { error }
                if error.contains("RABBIT won't remove it")),
            "{outcome:?}"
        );
    }

    // ---- What is removed, and in which order.

    #[test]
    fn the_cleanup_removes_the_files_in_order_then_the_folder_then_the_key() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let env = fixture.env(never_runs());
        assert_eq!(
            check(&fixture, &env),
            Ok(Some(PlannedMachineCleanup {
                folder: Some(scripts.folder.clone()),
                files: vec![scripts.dat.clone(), scripts.exe.clone()],
                registry_key: Some(shown_key(&fixture)),
                by_hand_key: BY_HAND_KEY.to_string(),
            }))
        );

        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
        assert_eq!(
            outcome.steps,
            vec![
                step(file(&scripts.dat), CleanupResult::Removed),
                step(file(&scripts.exe), CleanupResult::Removed),
                step(folder_target(&scripts.folder), CleanupResult::Removed),
                step(key_target(&fixture), CleanupResult::Removed),
            ]
        );
        assert!(outcome.still_present.is_empty(), "{outcome:?}");
        assert_eq!((outcome.refusal, outcome.error), (None, None));
        assert!(fs::symlink_metadata(&scripts.folder).is_err());
        assert!(!key_there(&fixture));
        assert!(
            fixture.folders.program_files_x86.is_dir(),
            "its parent stays"
        );
        assert!(env.started().is_empty());

        // The manifest's order is the order of the deletes, whatever order
        // the folder lists its files in.
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let reversed = spec_with_files(&["Uninstall.exe", "Uninstall.dat"]);
        let outcome = run_with(&fixture, &reversed, &fixture.env(never_runs()));
        assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
        assert_eq!(
            outcome.steps[..2],
            [
                step(file(&scripts.exe), CleanupResult::Removed),
                step(file(&scripts.dat), CleanupResult::Removed),
            ]
        );
    }

    #[test]
    fn a_cleanup_with_nothing_there_changes_nothing() {
        let fixture = Fixture::new();
        fixture
            .list
            .write("RABBIT-Test-Other", &[("DisplayName", "Other")]);
        let env = fixture.env(never_runs());
        assert_eq!(check(&fixture, &env), Ok(None));

        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::NothingThere, "{outcome:?}");
        assert!(outcome.steps.is_empty());
        assert!(outcome.still_present.is_empty());
        assert!(!outcome.removed_anything());
        assert!(
            fixture
                .list
                .views()
                .key_exists("RABBIT-Test-Other")
                .unwrap()
        );
        assert!(fixture.folders.program_files_x86.is_dir());
    }

    #[test]
    fn a_key_without_its_folder_is_removed_alone() {
        let fixture = Fixture::new();
        write_key(&fixture, &scripts_folder(&fixture));
        let env = fixture.env(never_runs());
        assert_eq!(
            check(&fixture, &env),
            Ok(Some(PlannedMachineCleanup {
                folder: None,
                files: Vec::new(),
                registry_key: Some(shown_key(&fixture)),
                by_hand_key: BY_HAND_KEY.to_string(),
            }))
        );

        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
        assert_eq!(
            outcome.steps,
            vec![step(key_target(&fixture), CleanupResult::Removed)]
        );
        assert!(!key_there(&fixture));
    }

    #[test]
    fn a_folder_without_its_key_is_removed_alone() {
        let fixture = Fixture::new();
        let scripts = install_folder(&fixture);
        let env = fixture.env(never_runs());
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
        assert_eq!(
            outcome.steps,
            vec![
                step(file(&scripts.dat), CleanupResult::Removed),
                step(file(&scripts.exe), CleanupResult::Removed),
                step(folder_target(&scripts.folder), CleanupResult::Removed),
            ]
        );
        assert!(fs::symlink_metadata(&scripts.folder).is_err());

        // A folder that holds only some of the files loses those.
        let fixture = Fixture::new();
        let scripts = install_folder(&fixture);
        fs::remove_file(&scripts.dat).unwrap();
        let env = fixture.env(never_runs());
        assert_eq!(
            check(&fixture, &env),
            Ok(Some(PlannedMachineCleanup {
                folder: Some(scripts.folder.clone()),
                files: vec![scripts.exe.clone()],
                registry_key: None,
                by_hand_key: BY_HAND_KEY.to_string(),
            }))
        );
        let outcome = run(&fixture, &env);
        assert_eq!(
            outcome.steps,
            vec![
                step(file(&scripts.exe), CleanupResult::Removed),
                step(folder_target(&scripts.folder), CleanupResult::Removed),
            ]
        );
    }

    /// `{program_files_x86}` is one folder, so a folder of the same name in
    /// the native Program Files is never looked at.
    #[test]
    fn the_same_folder_in_native_program_files_is_never_touched_with_or_without_a_key() {
        let fixture = Fixture::new();
        let native = fixture.folders.program_files[0].join(FOLDER);
        let native_files = [
            write(&native.join("Uninstall.dat"), b"log"),
            write(&native.join("Uninstall.exe"), b"uninstaller"),
        ];
        let env = fixture.env(never_runs());
        assert_eq!(check(&fixture, &env), Ok(None));
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::NothingThere, "{outcome:?}");

        // With the key there, only the key goes.
        write_key(&fixture, &scripts_folder(&fixture));
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
        assert_eq!(
            outcome.steps,
            vec![step(key_target(&fixture), CleanupResult::Removed)]
        );
        for path in &native_files {
            assert!(path.is_file(), "{} is gone", path.display());
        }
    }

    #[test]
    fn a_key_pointing_into_native_program_files_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install_folder(&fixture);
        let native = fixture.folders.program_files[0].join(FOLDER);
        let native_exe = write(&native.join("Uninstall.exe"), b"uninstaller");
        fixture.list.write(
            KEY,
            &[("UninstallDirectory", &native.display().to_string())],
        );
        let env = fixture.env(never_runs());
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::RegistryValueMismatch {
                found: native.display().to_string(),
            },
            &[&scripts.exe, &scripts.dat, &native_exe],
        );
        assert!(key_there(&fixture));
    }

    // ---- What the folder may hold.

    #[test]
    fn a_foreign_file_in_the_folder_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let planted = write(&scripts.folder.join("version.dll"), b"planted");
        let env = fixture.env(never_runs());
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::ForeignFile {
                path: planted.clone(),
            },
            &[&scripts.dat, &scripts.exe, &planted],
        );
        assert!(key_there(&fixture));
    }

    #[test]
    fn a_subfolder_or_junction_in_the_folder_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let env = fixture.env(never_runs());

        let subfolder = scripts.folder.join("Uninstall");
        fs::create_dir(&subfolder).unwrap();
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::FolderOrLink {
                path: subfolder.clone(),
            },
            &[&scripts.dat, &scripts.exe, &subfolder],
        );
        fs::remove_dir(&subfolder).unwrap();

        let elsewhere = fixture.root.join("Elsewhere");
        let sentinel = write(&elsewhere.join("Uninstall.dat"), b"sentinel");
        let junction = scripts.folder.join("Uninstall.lnk");
        make_junction(&junction, &elsewhere);
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::FolderOrLink {
                path: junction.clone(),
            },
            &[&scripts.dat, &scripts.exe, &junction, &sentinel],
        );
        assert!(key_there(&fixture));
    }

    #[test]
    fn a_folder_reached_through_a_junction_refuses_the_cleanup() {
        // The folder itself is a junction.
        let fixture = Fixture::new();
        let elsewhere = fixture.root.join("Users").join("Someone").join("Scripts");
        let sentinels = [
            write(&elsewhere.join("Uninstall.dat"), b"sentinel"),
            write(&elsewhere.join("Uninstall.exe"), b"sentinel"),
        ];
        let folder = scripts_folder(&fixture);
        make_junction(&folder, &elsewhere);
        write_key(&fixture, &folder);
        let env = fixture.env(never_runs());
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::WrongFolder {
                path: folder.clone(),
            },
            &[&sentinels[0], &sentinels[1], &folder],
        );
        assert!(key_there(&fixture));

        // A folder on the way to it is.
        let fixture = Fixture::new();
        let elsewhere = fixture.root.join("Users").join("Someone").join("x86");
        let sentinel = write(&elsewhere.join(FOLDER).join("Uninstall.exe"), b"sentinel");
        fs::remove_dir(&fixture.folders.program_files_x86).unwrap();
        make_junction(&fixture.folders.program_files_x86, &elsewhere);
        let folder = scripts_folder(&fixture);
        write_key(&fixture, &folder);
        let env = fixture.env(never_runs());
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::WrongFolder { path: folder },
            &[&sentinel],
        );
        assert!(key_there(&fixture));
    }

    /// A standard user may own a folder or a file only if they made it,
    /// and then they can put anything there.
    #[test]
    fn an_untrusted_file_or_folder_owner_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let mut env = fixture.env(never_runs());
        env.owners = Owners::UntrustedOnly(vec![scripts.exe.clone()]);
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::UntrustedOwner {
                path: scripts.exe.clone(),
            },
            &[&scripts.dat, &scripts.exe],
        );

        env.owners = Owners::UntrustedOnly(vec![scripts.folder.clone()]);
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::UntrustedFolder {
                path: scripts.folder.clone(),
            },
            &[&scripts.dat, &scripts.exe],
        );
        assert!(key_there(&fixture));
    }

    // ---- What the registry entry may hold.

    #[test]
    fn a_key_value_pointing_elsewhere_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let env = fixture.env(never_runs());

        let other = fixture.folders.program_files_x86.join("Other");
        fs::create_dir_all(&other).unwrap();
        fixture.list.write(
            KEY,
            &[
                ("UninstallDirectory", &other.display().to_string()),
                (
                    "Uninstall.dat",
                    &scripts.folder.join("Uninstall.dat").display().to_string(),
                ),
            ],
        );
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::RegistryValueMismatch {
                found: other.display().to_string(),
            },
            &[&scripts.dat, &scripts.exe],
        );

        // The right folder, but another file than the value names.
        let wrong_file = scripts.folder.join("Uninstall.exe").display().to_string();
        fixture.list.write(
            KEY,
            &[
                ("UninstallDirectory", &scripts.folder.display().to_string()),
                ("Uninstall.dat", &wrong_file),
            ],
        );
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::RegistryValueMismatch { found: wrong_file },
            &[&scripts.dat, &scripts.exe],
        );
        assert!(key_there(&fixture));

        // Written in other letters, with quotes and a trailing separator,
        // it is still the folder.
        fixture.list.write(
            KEY,
            &[(
                "UninstallDirectory",
                &format!("\"{}\\\"", scripts.folder.display()).to_uppercase(),
            )],
        );
        assert!(matches!(check(&fixture, &env), Ok(Some(_))));
    }

    #[test]
    fn an_unexpected_key_value_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let env = fixture.env(never_runs());
        let folder = scripts.folder.display().to_string();
        fixture.list.write(
            KEY,
            &[
                ("UninstallDirectory", folder.as_str()),
                ("DisplayName", "Something else"),
            ],
        );
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::UnexpectedRegistryValue {
                value: "DisplayName".to_string(),
            },
            &[&scripts.dat, &scripts.exe],
        );
        assert!(key_there(&fixture));

        // Value names ignore case, as Windows' do.
        fixture
            .list
            .write(KEY, &[("UNINSTALLDIRECTORY", folder.as_str())]);
        assert!(matches!(check(&fixture, &env), Ok(Some(_))));
    }

    #[test]
    fn a_key_without_the_value_naming_its_folder_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install_folder(&fixture);
        let env = fixture.env(never_runs());
        fixture.list.write(
            KEY,
            &[("Uninstall.dat", &scripts.dat.display().to_string())],
        );
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::InstallLocationMissing,
            &[&scripts.dat, &scripts.exe],
        );

        fixture.list.write(KEY, &[("UninstallDirectory", "  ")]);
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::InstallLocationMissing,
            &[&scripts.dat, &scripts.exe],
        );
        assert!(key_there(&fixture));
    }

    /// A hand-made cleanup with no value naming its folder can't check the
    /// folder through its key, so it is refused once the key is there.
    #[test]
    fn a_hand_made_cleanup_without_a_value_naming_its_folder_is_refused() {
        let fixture = Fixture::new();
        let scripts = install_folder(&fixture);
        let mut spec = spec();
        spec.registry
            .values
            .retain(|name, _| name != "UninstallDirectory");
        fixture.list.write(
            KEY,
            &[("Uninstall.dat", &scripts.dat.display().to_string())],
        );
        let env = fixture.env(never_runs());
        assert_eq!(
            check_with(&fixture, &spec, &env),
            Err(VendorRefusal::NotAVendorPlan)
        );
        let outcome = run_with(&fixture, &spec, &env);
        assert_eq!(outcome.status, CleanupStatus::Refused, "{outcome:?}");
        assert_eq!(outcome.refusal, Some(VendorRefusal::NotAVendorPlan));
        assert!(outcome.steps.is_empty(), "{outcome:?}");
        assert!(scripts.dat.is_file() && scripts.exe.is_file());
        assert!(key_there(&fixture));
    }

    /// A key the test made below the cleanup's entry, deleted when dropped,
    /// also when the test panics, so the throwaway list can go too.
    struct Subkey {
        parent: HkcuUninstallRoot,
        name: &'static str,
    }

    impl Drop for Subkey {
        fn drop(&mut self) {
            let _ = self.parent.delete_entry(self.name);
        }
    }

    #[test]
    fn a_key_with_a_subkey_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let subkey = Subkey {
            parent: HkcuUninstallRoot::at(format!("{}\\{KEY}", fixture.list.path())),
            name: "Extra",
        };
        subkey
            .parent
            .write_entry(
                subkey.name,
                &UninstallEntry::new(vec![RegistryValue::string("DisplayName", "Extra")]),
            )
            .unwrap();
        let env = fixture.env(never_runs());
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::RegistryKeyHasSubkeys,
            &[&scripts.dat, &scripts.exe],
        );
        assert!(key_there(&fixture));
        assert!(subkey.parent.read_entry(subkey.name).unwrap().is_some());
    }

    #[test]
    fn an_expandable_registry_value_refuses_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install_folder(&fixture);
        let folder = scripts.folder.display().to_string();
        fixture.list.write_values(
            KEY,
            vec![RegistryValue::expandable_string(
                "UninstallDirectory",
                &folder,
            )],
        );
        let env = fixture.env(never_runs());
        assert_refused(
            &fixture,
            &env,
            VendorRefusal::RegistryValueMismatch { found: folder },
            &[&scripts.dat, &scripts.exe],
        );
        assert!(key_there(&fixture));
    }

    // ---- Failures part way.

    /// A file another program holds open stays, and so do the folder and
    /// the key after it. Once it is free, the cleanup finishes what is
    /// left.
    #[test]
    fn a_file_that_cant_be_removed_leaves_the_key() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let env = fixture.env(never_runs());
        let held = LockedFile::open(&scripts.exe).unwrap();

        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Failed, "{outcome:?}");
        assert_eq!(outcome.steps.len(), 2, "{outcome:?}");
        assert_eq!(
            outcome.steps[0],
            step(file(&scripts.dat), CleanupResult::Removed)
        );
        assert_eq!(outcome.steps[1].target, file(&scripts.exe));
        assert!(matches!(
            outcome.steps[1].result,
            CleanupResult::Failed { .. }
        ));
        assert!(outcome.removed_anything());
        assert!(
            outcome
                .error
                .as_deref()
                .is_some_and(|error| error.contains(&scripts.exe.display().to_string())),
            "{outcome:?}"
        );
        assert_eq!(
            outcome.still_present,
            vec![scripts.folder.display().to_string(), shown_key(&fixture)]
        );
        assert!(scripts.exe.is_file());
        assert!(key_there(&fixture));

        drop(held);
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
        assert_eq!(
            outcome.steps,
            vec![
                step(file(&scripts.exe), CleanupResult::Removed),
                step(folder_target(&scripts.folder), CleanupResult::Removed),
                step(key_target(&fixture), CleanupResult::Removed),
            ]
        );
        assert!(!key_there(&fixture));
    }

    /// An owner that changes after the checks stops the deletes at that
    /// file. Before anything is removed, that is a refusal, and nothing
    /// changed; after, the cleanup failed part way, and the key stays.
    #[test]
    fn an_owner_that_changes_after_the_checks_stops_the_deletes() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let mut env = Changing::new(fixture.env(never_runs()));
        env.untrusted_later = vec![scripts.dat.clone()];
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Refused, "{outcome:?}");
        assert_eq!(
            outcome.refusal,
            Some(VendorRefusal::UntrustedOwner {
                path: scripts.dat.clone()
            })
        );
        assert!(outcome.steps.is_empty(), "{outcome:?}");
        assert!(scripts.dat.is_file() && scripts.exe.is_file());
        assert!(key_there(&fixture));

        // The first file goes, so the stop at the second is a failure.
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let mut env = Changing::new(fixture.env(never_runs()));
        env.untrusted_later = vec![scripts.exe.clone()];
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Failed, "{outcome:?}");
        assert_eq!(outcome.refusal, None);
        assert_eq!(outcome.steps.len(), 2, "{outcome:?}");
        assert_eq!(
            outcome.steps[0],
            step(file(&scripts.dat), CleanupResult::Removed)
        );
        assert_refused_now(&outcome, 1, file(&scripts.exe));
        assert_eq!(
            outcome.still_present,
            vec![scripts.folder.display().to_string(), shown_key(&fixture)]
        );
        assert!(!scripts.dat.exists());
        assert!(scripts.exe.is_file());
        assert!(key_there(&fixture));
    }

    /// A folder turned into a junction after the checks is never followed:
    /// what it leads to stays, though it has the same names.
    #[test]
    fn a_folder_turned_into_a_junction_after_the_checks_stops_the_deletes() {
        // The cleanup's folder, once its first file is gone.
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let elsewhere = fixture.root.join("Elsewhere");
        let sentinel = write(&elsewhere.join("Uninstall.exe"), b"sentinel");
        let moved = fixture.root.join("Moved");
        let mut env = Changing::new(fixture.env(never_runs()));
        let (dat, folder, to, target) = (
            scripts.dat.clone(),
            scripts.folder.clone(),
            moved.clone(),
            elsewhere.clone(),
        );
        env.after_file = Box::new(move |path| {
            if path == dat {
                fs::rename(&folder, &to).unwrap();
                make_junction(&folder, &target);
            }
        });
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Failed, "{outcome:?}");
        assert_eq!(outcome.steps.len(), 2, "{outcome:?}");
        assert_eq!(
            outcome.steps[0],
            step(file(&scripts.dat), CleanupResult::Removed)
        );
        assert_refused_now(&outcome, 1, file(&scripts.exe));
        assert!(sentinel.is_file());
        assert!(moved.join("Uninstall.exe").is_file());
        assert_eq!(
            outcome.still_present,
            vec![scripts.folder.display().to_string(), shown_key(&fixture)]
        );
        assert!(key_there(&fixture));

        // Program Files (x86), once the files are gone: the folder of the
        // same name it leads to stays.
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let elsewhere = fixture.root.join("Elsewhere");
        let sentinel = elsewhere.join(FOLDER);
        fs::create_dir_all(&sentinel).unwrap();
        let moved = fixture.root.join("Moved");
        let mut env = Changing::new(fixture.env(never_runs()));
        let (exe, x86, to, target) = (
            scripts.exe.clone(),
            fixture.folders.program_files_x86.clone(),
            moved.clone(),
            elsewhere.clone(),
        );
        env.after_file = Box::new(move |path| {
            if path == exe {
                fs::rename(&x86, &to).unwrap();
                make_junction(&x86, &target);
            }
        });
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Failed, "{outcome:?}");
        assert_eq!(outcome.steps.len(), 3, "{outcome:?}");
        assert_eq!(
            outcome.steps[..2],
            [
                step(file(&scripts.dat), CleanupResult::Removed),
                step(file(&scripts.exe), CleanupResult::Removed),
            ]
        );
        assert_refused_now(&outcome, 2, folder_target(&scripts.folder));
        assert!(sentinel.is_dir());
        assert!(moved.join(FOLDER).is_dir());
        assert!(key_there(&fixture));
    }

    /// A delete that says it worked but leaves the folder is caught
    /// afterwards: the cleanup failed, and says what is still there.
    #[test]
    fn something_still_there_after_every_delete_worked_fails_the_cleanup() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let mut env = Changing::new(fixture.env(never_runs()));
        env.folder_stays = true;
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Failed, "{outcome:?}");
        assert_eq!(
            outcome.steps,
            vec![
                step(file(&scripts.dat), CleanupResult::Removed),
                step(file(&scripts.exe), CleanupResult::Removed),
                step(folder_target(&scripts.folder), CleanupResult::Removed),
                step(key_target(&fixture), CleanupResult::Removed),
            ]
        );
        let folder = scripts.folder.display().to_string();
        assert_eq!(outcome.still_present, vec![folder.clone()]);
        assert_eq!(
            outcome.error,
            Some(format!("still there afterwards: {folder}"))
        );
        assert!(scripts.folder.is_dir());
        assert!(!key_there(&fixture));
    }

    /// A file or a key that went between the checks and its delete counts
    /// as removed.
    #[test]
    fn what_went_between_the_checks_and_its_delete_counts_as_removed() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let mut env = Changing::new(fixture.env(never_runs()));
        let (dat, exe) = (scripts.dat.clone(), scripts.exe.clone());
        env.after_file = Box::new(move |path| {
            if path == dat {
                fs::remove_file(&exe).unwrap();
            }
        });
        env.key_gone_already = true;
        let outcome = run(&fixture, &env);
        assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
        assert_eq!(
            outcome.steps,
            vec![
                step(file(&scripts.dat), CleanupResult::Removed),
                step(file(&scripts.exe), CleanupResult::NotFound),
                step(folder_target(&scripts.folder), CleanupResult::Removed),
                step(key_target(&fixture), CleanupResult::NotFound),
            ]
        );
        assert!(outcome.still_present.is_empty(), "{outcome:?}");
        assert_eq!(outcome.error, None);
        assert!(fs::symlink_metadata(&scripts.folder).is_err());
        assert!(!key_there(&fixture));
    }

    // ---- The environments themselves.

    /// The lookup before a run checks exactly as the helper does, and it
    /// can't delete anything, whatever the run asks of it.
    #[test]
    fn the_lookup_checks_like_the_helper_and_changes_nothing() {
        let fixture = Fixture::new();
        let scripts = install(&fixture);
        let lookup = LookupOnly {
            views: fixture.list.views(),
        };
        // The helper's checks with the real owners, as the lookup sees
        // them: trusted when this test runs elevated, as on CI, and the
        // folder refused otherwise.
        let mut helper = fixture.env(never_runs());
        helper.owners = Owners::RealExcept(Vec::new());
        let looked_up = check(&fixture, &lookup);
        assert_eq!(looked_up, check(&fixture, &helper));
        if rabbit_platform::file_owner_is_trusted(&scripts.folder).unwrap() {
            assert!(matches!(looked_up, Ok(Some(_))), "{looked_up:?}");
        } else {
            assert_eq!(
                looked_up,
                Err(VendorRefusal::UntrustedFolder {
                    path: scripts.folder.clone()
                })
            );
        }

        let outcome = run(&fixture, &lookup);
        assert!(!outcome.removed_anything(), "{outcome:?}");
        for result in [
            lookup.remove_file_no_follow(&scripts.exe),
            lookup.remove_file_no_follow(&scripts.dat),
            lookup.remove_empty_dir(&scripts.folder),
            lookup
                .delete_uninstall_entry(KEY, RegistryView::Bit32)
                .map(|_| ()),
        ] {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        }
        assert!(scripts.dat.is_file());
        assert!(scripts.exe.is_file());
        assert!(key_there(&fixture));
    }

    #[test]
    fn the_test_environment_refuses_deletes_outside_its_sandbox() {
        let fixture = Fixture::new();
        let env = fixture.env(never_runs());
        let outside = tempfile::tempdir().unwrap();
        let sentinel = write(&outside.path().join("sentinel.txt"), b"keep");
        let empty = outside.path().join("Empty");
        fs::create_dir(&empty).unwrap();
        let denied = |result: io::Result<()>| {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        };
        denied(env.remove_file_no_follow(&sentinel));
        denied(env.remove_empty_dir(&empty));

        // Nor through a junction inside it that leads out, nor the
        // fixture folder itself.
        let link = fixture.root.join("Link");
        make_junction(&link, outside.path());
        denied(env.remove_file_no_follow(&link.join("sentinel.txt")));
        denied(env.remove_empty_dir(&link.join("Empty")));
        denied(env.remove_empty_dir(&fixture.root));
        assert!(sentinel.is_file());
        assert!(empty.is_dir());

        // Nor a junction outside it that leads in: the path itself must be
        // inside. The platform would refuse the junction as invalid input,
        // so only the environment answers "denied".
        let inner_empty = fixture.root.join("InnerEmpty");
        fs::create_dir(&inner_empty).unwrap();
        let into = outside.path().join("Into");
        make_junction(&into, &inner_empty);
        denied(env.remove_empty_dir(&into));
        assert!(fs::symlink_metadata(&into).is_ok());
        assert!(inner_empty.is_dir());

        // Inside it, a delete happens.
        let inside = write(&fixture.root.join("inside.txt"), b"gone");
        env.remove_file_no_follow(&inside).unwrap();
        assert!(!inside.exists());

        // HKLM is refused before the platform is asked. rabbit-platform's
        // key-name check refuses this name, with its separator, as invalid
        // input before Windows is asked, so only the test environment can
        // answer "denied".
        let machine =
            TestEnvironment::new(UninstallViews::local_machine(), &fixture.root, never_runs());
        let error = machine
            .delete_uninstall_entry("RABBIT-Test\\Never-Created", RegistryView::Bit32)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        // Under HKCU, only a test's own key: the platform refuses any other
        // root before it deletes anything. This one is no real list either.
        let other_root = TestEnvironment::new(
            UninstallViews::current_user_at("Software\\RABBIT-other-never-created"),
            &fixture.root,
            never_runs(),
        );
        let error = other_root
            .delete_uninstall_entry("RABBIT-Test-Never-Created", RegistryView::Bit32)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
