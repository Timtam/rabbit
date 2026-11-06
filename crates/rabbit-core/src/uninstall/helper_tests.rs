//! Tests for the elevated helper and RABBIT's side of it.
//!
//! No test here starts the real helper or raises a permission prompt: the
//! launcher and the helper's entry point refuse inside tests, and the tests
//! that need a helper run [`serve`] on a thread of their own, against a
//! throwaway registry key and fixture folders.

// Some fixtures serve only the Windows tests below.
#![cfg_attr(not(windows), allow(dead_code))]

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rabbit_platform::{PendingFileRename, UninstallViews, uninstall_pipe_name};

use super::*;
use crate::model::{InstallationKind, Platform};
use crate::package::{parse_embedded_package_spec, validate_package_spec};
use crate::uninstall::vendor::{
    ProgramEnd, RunLimits, VendorCommand, VendorEnvironment, VendorOutcome, VendorRefusal,
    VendorStatus,
};

const LONG: Duration = Duration::from_secs(20);

/// An environment for helpers that are never asked to run an uninstaller:
/// it can't start anything, and its list of installed programs is a key
/// nobody creates.
struct NothingToRun {
    views: UninstallViews,
}

impl NothingToRun {
    fn new() -> Self {
        Self {
            views: UninstallViews::current_user_at("Software\\RABBIT-test-never-created"),
        }
    }
}

fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "this environment runs nothing")
}

impl VendorEnvironment for NothingToRun {
    fn uninstall_views(&self) -> &UninstallViews {
        &self.views
    }

    fn owner_is_trusted(&self, _path: &Path) -> io::Result<bool> {
        Err(unsupported())
    }

    fn create_working_folder(&self) -> io::Result<PathBuf> {
        Err(unsupported())
    }

    fn copy_file(&self, _from: &Path, _to: &Path) -> io::Result<()> {
        Err(unsupported())
    }

    fn run_program(
        &self,
        _command: &VendorCommand,
        _limits: RunLimits,
        _on_started: &mut dyn FnMut(),
        _on_still_waiting: &mut dyn FnMut(),
    ) -> io::Result<ProgramEnd> {
        Err(unsupported())
    }

    fn pending_file_renames(&self) -> io::Result<Vec<PendingFileRename>> {
        Ok(Vec::new())
    }

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

fn folders() -> MachineFolders {
    let root = std::env::temp_dir().join("RABBIT-test-machine");
    MachineFolders {
        program_files: vec![root.join("Program Files")],
        common_program_files: root.join("Program Files").join("Common Files"),
        program_data: root.join("ProgramData"),
    }
}

fn args_for(package_ids: &[&str]) -> HelperArgs {
    HelperArgs {
        pipe: uninstall_pipe_name(4242, &[0xab; 16]),
        parent_pid: 4242,
        resource_path: PathBuf::from("C:\\Users\\Someone\\AppData\\Roaming\\REAPER"),
        target_app_path: Some(PathBuf::from("C:\\Program Files\\REAPER (x64)\\reaper.exe")),
        target: InstallationKind::Standard,
        package_ids: package_ids.iter().map(|id| id.to_string()).collect(),
    }
}

/// A helper loop on its own thread, for `package_ids`, with no package it
/// could run. Returns RABBIT's end of the channel and the thread.
fn serving(package_ids: &[&str]) -> (MemoryChannel, thread::JoinHandle<i32>) {
    let (mut rabbit, mut helper) = MemoryChannel::pair();
    let args = args_for(package_ids);
    let roots = helper_roots(&args, &folders());
    let serving =
        thread::spawn(move || serve(&mut helper, &args, &[], &NothingToRun::new(), &roots));
    let ready = next_event(&mut rabbit);
    assert_eq!(
        ready,
        HelperEvent::Ready {
            helper_pid: std::process::id()
        }
    );
    (rabbit, serving)
}

fn order(channel: &mut MemoryChannel, order: &HelperOrder) {
    channel
        .send_line(&serde_json::to_string(order).unwrap(), LONG)
        .unwrap();
}

fn run_order(channel: &mut MemoryChannel, package_id: &str) {
    order(
        channel,
        &HelperOrder::Run {
            package_id: package_id.to_string(),
        },
    );
}

fn next_event(channel: &mut MemoryChannel) -> HelperEvent {
    let line = channel
        .read_line(LONG)
        .unwrap()
        .expect("the helper went away");
    serde_json::from_str(&line).unwrap()
}

/// Run `package_id` and expect it to finish refused, since this helper has
/// no package it could run.
fn expect_finished_without_a_plan(channel: &mut MemoryChannel, package_id: &str) {
    run_order(channel, package_id);
    assert_eq!(
        next_event(channel),
        HelperEvent::Started {
            package_id: package_id.to_string()
        }
    );
    assert_eq!(
        next_event(channel),
        HelperEvent::Finished {
            package_id: package_id.to_string(),
            outcome: Box::new(VendorOutcome::refused(
                package_id,
                VendorRefusal::NotAVendorPlan
            )),
        }
    );
}

fn expect_refused(
    channel: &mut MemoryChannel,
    serving: thread::JoinHandle<i32>,
    reason: HelperRefusal,
) {
    assert_eq!(next_event(channel), HelperEvent::Refused { reason });
    assert_eq!(serving.join().unwrap(), HELPER_EXIT_REFUSED_ORDER);
    assert_eq!(channel.read_line(LONG).unwrap(), None, "the session ended");
}

#[test]
fn orders_and_events_are_one_json_object_per_line() {
    for (order, line) in [
        (
            HelperOrder::Run {
                package_id: "surge-xt".to_string(),
            },
            r#"{"order":"run","package_id":"surge-xt"}"#,
        ),
        (HelperOrder::Quit {}, r#"{"order":"quit"}"#),
    ] {
        assert_eq!(serde_json::to_string(&order).unwrap(), line);
        assert_eq!(HelperOrder::parse(line).unwrap(), order);
    }
    for (event, line) in [
        (
            HelperEvent::Ready { helper_pid: 4242 },
            r#"{"event":"ready","helper_pid":4242}"#,
        ),
        (
            HelperEvent::VendorLaunched {
                package_id: "reaper".to_string(),
            },
            r#"{"event":"vendor-launched","package_id":"reaper"}"#,
        ),
        (
            HelperEvent::StillWaiting {
                package_id: "reaper".to_string(),
                seconds: 30,
            },
            r#"{"event":"still-waiting","package_id":"reaper","seconds":30}"#,
        ),
        (
            HelperEvent::Refused {
                reason: HelperRefusal::OutOfOrder {
                    package_id: "surge-xt".to_string(),
                },
            },
            r#"{"event":"refused","reason":{"kind":"out-of-order","package_id":"surge-xt"}}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&event).unwrap(), line);
        assert_eq!(serde_json::from_str::<HelperEvent>(line).unwrap(), event);
    }
    let finished = HelperEvent::Finished {
        package_id: "surge-xt".to_string(),
        outcome: Box::new(VendorOutcome {
            status: VendorStatus::Removed,
            launched: true,
            exit_code: Some(0),
            ..VendorOutcome::new("surge-xt")
        }),
    };
    let line = serde_json::to_string(&finished).unwrap();
    assert!(!line.contains('\n'));
    assert_eq!(
        serde_json::from_str::<HelperEvent>(&line).unwrap(),
        finished
    );
}

#[test]
fn an_order_that_is_anything_else_doesnt_read() {
    for line in [
        r#"{"order":"run"}"#,
        r#"{"order":"run","package_id":"reaper","path":"C:\\evil.exe"}"#,
        r#"{"order":"delete","path":"C:\\Windows"}"#,
        r#"{"order":"quit","now":true}"#,
        r#"["run","reaper"]"#,
        "",
    ] {
        assert!(HelperOrder::parse(line).is_err(), "{line}");
    }
}

#[test]
fn the_helpers_command_line_lists_every_argument() {
    let args = args_for(&["surge-xt", "reaper"]);
    assert_eq!(
        args.to_arguments().unwrap(),
        [
            "--pipe",
            args.pipe.as_str(),
            "--parent-pid",
            "4242",
            "--resource-path",
            "C:\\Users\\Someone\\AppData\\Roaming\\REAPER",
            "--target-app-path",
            "C:\\Program Files\\REAPER (x64)\\reaper.exe",
            "--target-kind",
            "standard",
            "--package",
            "surge-xt",
            "--package",
            "reaper",
        ]
    );
    let portable = HelperArgs {
        target_app_path: None,
        target: InstallationKind::Portable,
        ..args
    };
    let arguments = portable.to_arguments().unwrap();
    assert!(
        !arguments
            .iter()
            .any(|argument| argument == "--target-app-path")
    );
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["--target-kind", "portable"])
    );
}

/// The helper takes only a command line RABBIT would write.
#[cfg(windows)]
#[test]
fn the_helpers_command_line_is_checked_before_anything_else() {
    assert_eq!(args_for(&["surge-xt", "reaper"]).validate(), Ok(()));
    let refused = |change: &dyn Fn(&mut HelperArgs)| {
        let mut args = args_for(&["surge-xt", "reaper"]);
        change(&mut args);
        assert!(args.validate().is_err(), "{args:?}");
    };
    refused(&|args| args.pipe = "\\\\.\\pipe\\something-else".to_string());
    refused(&|args| args.pipe = format!("{}x", args.pipe));
    // The pipe must be the parent's own.
    refused(&|args| args.parent_pid = 4243);
    refused(&|args| args.resource_path = PathBuf::from("REAPER"));
    refused(&|args| args.resource_path = PathBuf::from("\\\\server\\share\\REAPER"));
    refused(&|args| args.target_app_path = Some(PathBuf::from("reaper.exe")));
    refused(&|args| args.package_ids.clear());
    refused(&|args| args.package_ids.push("surge-xt".to_string()));
    refused(&|args| args.package_ids.push("Surge XT".to_string()));
    refused(&|args| args.package_ids.push("..\\reaper".to_string()));
    refused(&|args| args.package_ids.push(String::new()));
}

#[test]
fn packages_run_only_from_the_command_line_each_once_and_in_order() {
    let mut book = OrderBook::new(&["a".to_string(), "b".to_string(), "c".to_string()]);
    assert_eq!(
        book.admit("x"),
        Err(HelperRefusal::NotOnCommandLine {
            package_id: "x".to_string()
        })
    );
    // A package may be left out, as when RABBIT's run stops...
    assert_eq!(book.admit("b"), Ok(()));
    // ...but never comes back, and none runs twice.
    assert_eq!(
        book.admit("a"),
        Err(HelperRefusal::OutOfOrder {
            package_id: "a".to_string()
        })
    );
    assert_eq!(
        book.admit("b"),
        Err(HelperRefusal::AlreadyRun {
            package_id: "b".to_string()
        })
    );
    assert_eq!(book.admit("c"), Ok(()));
}

#[test]
fn the_helper_says_it_is_ready_and_ends_when_told() {
    let (mut rabbit, serving) = serving(&["surge-xt"]);
    order(&mut rabbit, &HelperOrder::Quit {});
    assert_eq!(serving.join().unwrap(), HELPER_EXIT_DONE);
}

#[test]
fn the_helper_ends_when_rabbit_goes_away() {
    let (rabbit, serving) = serving(&["surge-xt", "reaper"]);
    drop(rabbit);
    assert_eq!(serving.join().unwrap(), HELPER_EXIT_DONE);
}

#[test]
fn a_package_not_on_the_command_line_is_refused_and_ends_the_session() {
    let (mut rabbit, serving) = serving(&["surge-xt"]);
    run_order(&mut rabbit, "reaper");
    expect_refused(
        &mut rabbit,
        serving,
        HelperRefusal::NotOnCommandLine {
            package_id: "reaper".to_string(),
        },
    );
}

#[test]
fn a_package_out_of_order_is_refused() {
    let (mut rabbit, serving) = serving(&["surge-xt", "reaper"]);
    expect_finished_without_a_plan(&mut rabbit, "reaper");
    run_order(&mut rabbit, "surge-xt");
    expect_refused(
        &mut rabbit,
        serving,
        HelperRefusal::OutOfOrder {
            package_id: "surge-xt".to_string(),
        },
    );
}

#[test]
fn a_package_run_twice_is_refused() {
    let (mut rabbit, serving) = serving(&["surge-xt", "reaper"]);
    expect_finished_without_a_plan(&mut rabbit, "surge-xt");
    run_order(&mut rabbit, "surge-xt");
    expect_refused(
        &mut rabbit,
        serving,
        HelperRefusal::AlreadyRun {
            package_id: "surge-xt".to_string(),
        },
    );
}

#[test]
fn an_unreadable_order_ends_the_session() {
    let (mut rabbit, serving) = serving(&["surge-xt"]);
    rabbit
        .send_line(
            r#"{"order":"run","package_id":"surge-xt","program":"C:\\x.exe"}"#,
            LONG,
        )
        .unwrap();
    assert!(matches!(
        next_event(&mut rabbit),
        HelperEvent::Refused {
            reason: HelperRefusal::UnreadableOrder { .. }
        }
    ));
    assert_eq!(serving.join().unwrap(), HELPER_EXIT_REFUSED_ORDER);
}

/// A package whose Windows plan is `plan`, offered only on Windows and
/// only to a standard REAPER, validated like a shipped manifest.
pub(crate) fn package(id: &str, plan: &str) -> PackageSpec {
    let source = format!(
        r#"{{
            "id": "{id}",
            "display_name": "Demo",
            "display_name_key": "package-demo",
            "display_description_key": "package-demo-description",
            "recommended": false,
            "user_plugin_prefixes": [],
            "user_plugin_suffixes": {{ "windows": [], "macos": [] }},
            "requires_standard_install": true,
            "supported_platforms": ["windows"],
            "uninstall": {{ "plans": [ {{
                "platform": "windows",
                "summary": {{ "removes_key": "a", "keeps_key": "b" }},
                {plan}
            }} ] }}
        }}"#
    );
    let spec =
        parse_embedded_package_spec(&source).unwrap_or_else(|error| panic!("{error}\n{source}"));
    validate_package_spec(&spec).unwrap_or_else(|error| panic!("{error}\n{source}"));
    spec.to_package_spec(Platform::Windows)
}

/// Surge XT's plan, elevated, as a package called `id`.
#[cfg(windows)]
pub(crate) fn elevated_surge(id: &str) -> PackageSpec {
    let vendor =
        serde_json::to_string(&crate::uninstall::vendor::tests::windows::surge_spec(60)).unwrap();
    package(
        id,
        &format!(
            r#""strategy": "vendor_uninstaller", "elevation": "required", "backup": "none",
               "vendor_uninstaller": {vendor}"#
        ),
    )
}

#[test]
fn only_a_plan_that_runs_a_vendor_uninstaller_elevated_is_run() {
    const INNO: &str = r#""vendor_uninstaller": {
        "kind": "inno",
        "locate": [{ "registry": { "hive": "hklm", "key": "Demo_is1", "value": "UninstallString" } }],
        "must_be_in": { "dir": "{program_data}/Demo/uninstall", "only_files": "unins000.*" },
        "timeout_seconds": 600
    }"#;
    let specs = [
        package(
            "elevated",
            &format!(
                r#""strategy": "vendor_uninstaller", "elevation": "required", "backup": "none", {INNO}"#
            ),
        ),
        package(
            "not-elevated",
            &format!(r#""strategy": "vendor_uninstaller", {INNO}"#),
        ),
        package(
            "files",
            r#""strategy": "remove_files", "remove": { "paths": ["{program_data}/Demo/a.dll"] }"#,
        ),
    ];
    assert!(elevated_vendor_plan(&specs, "elevated", InstallationKind::Standard).is_some());
    // A machine-wide package is never removed from a portable REAPER.
    assert!(elevated_vendor_plan(&specs, "elevated", InstallationKind::Portable).is_none());
    assert!(elevated_vendor_plan(&specs, "not-elevated", InstallationKind::Standard).is_none());
    assert!(elevated_vendor_plan(&specs, "files", InstallationKind::Standard).is_none());
    assert!(elevated_vendor_plan(&specs, "missing", InstallationKind::Standard).is_none());

    let outcome = run_package(
        "files",
        InstallationKind::Standard,
        &specs,
        &helper_roots(&args_for(&["files"]), &folders()),
        &NothingToRun::new(),
        &mut |_| panic!("nothing runs"),
    );
    assert_eq!(
        outcome,
        VendorOutcome::refused("files", VendorRefusal::NotAVendorPlan)
    );
}

/// The helper never reads a per-user folder: only the resource folder and
/// REAPER's app from its command line are known, beside the machine-wide
/// folders Windows names. Windows paths, so Windows only.
#[cfg(windows)]
#[test]
fn the_helper_knows_no_per_user_folder() {
    let args = args_for(&["reaper"]);
    let folders = folders();
    let roots = helper_roots(&args, &folders);
    assert_eq!(roots.resource, args.resource_path);
    assert_eq!(roots.app_path, args.target_app_path);
    assert_eq!(
        roots.app_dir.as_deref(),
        Some(Path::new("C:\\Program Files\\REAPER (x64)"))
    );
    assert_eq!(roots.program_files, folders.program_files);
    assert_eq!(
        roots.common_program_files,
        Some(folders.common_program_files.clone())
    );
    assert_eq!(roots.program_data, Some(folders.program_data.clone()));
    assert_eq!(
        (
            &roots.appdata,
            &roots.localappdata,
            &roots.clap_dir,
            &roots.home,
            &roots.documents
        ),
        (&None, &None, &None, &None, &None)
    );
}

fn request(package_ids: &[&str], resource: &Path) -> ElevatedRunRequest {
    ElevatedRunRequest {
        resource_path: resource.to_path_buf(),
        target_app_path: None,
        target: InstallationKind::Standard,
        package_ids: package_ids.iter().map(|id| id.to_string()).collect(),
    }
}

/// Neither the launcher nor the helper's own entry point does anything in
/// a test: no pipe, no prompt, no program started.
#[test]
fn the_real_helper_never_starts_in_a_test() {
    let started = SystemElevatedHelper.start(&request(&["surge-xt"], &std::env::temp_dir()));
    match started {
        Err(ElevationStartError::Failed(why)) => assert!(why.contains("tests"), "{why}"),
        Err(other) => panic!("expected a refusal, got {other:?}"),
        Ok(_) => panic!("the elevated helper started in a test"),
    }
    assert_eq!(
        run_elevated_helper(&args_for(&["surge-xt"])),
        HELPER_EXIT_NOT_ALLOWED
    );
    // No test program is named like a RABBIT build either, which is what
    // keeps other crates' tests, built without this crate's cfg(test), out.
    assert!(!rabbit_platform::running_as_rabbit());
}

#[test]
fn an_in_process_helper_never_reads_windows_real_list() {
    struct RealList(UninstallViews);
    impl VendorEnvironment for RealList {
        fn uninstall_views(&self) -> &UninstallViews {
            &self.0
        }
        fn owner_is_trusted(&self, _path: &Path) -> io::Result<bool> {
            Err(unsupported())
        }
        fn create_working_folder(&self) -> io::Result<PathBuf> {
            Err(unsupported())
        }
        fn copy_file(&self, _from: &Path, _to: &Path) -> io::Result<()> {
            Err(unsupported())
        }
        fn run_program(
            &self,
            _command: &VendorCommand,
            _limits: RunLimits,
            _on_started: &mut dyn FnMut(),
            _on_still_waiting: &mut dyn FnMut(),
        ) -> io::Result<ProgramEnd> {
            panic!("nothing may start")
        }
        fn pending_file_renames(&self) -> io::Result<Vec<PendingFileRename>> {
            Ok(Vec::new())
        }
        fn now(&self) -> Instant {
            Instant::now()
        }
        fn sleep(&self, _duration: Duration) {}
    }

    let helper = InProcessElevatedHelper::new(
        Arc::new(RealList(UninstallViews::local_machine())),
        Vec::new(),
        folders(),
    );
    assert!(matches!(
        helper.start(&request(&["surge-xt"], &std::env::temp_dir())),
        Err(ElevationStartError::Failed(_))
    ));
}

#[test]
fn a_declined_prompt_starts_nothing() {
    let helper = InProcessElevatedHelper::new(Arc::new(NothingToRun::new()), Vec::new(), folders())
        .declining();
    assert!(matches!(
        helper.start(&request(&["surge-xt"], &std::env::temp_dir())),
        Err(ElevationStartError::Declined)
    ));
}

/// A session's orders reach the helper, its answers come back, and a
/// refusal ends the session for every later package.
#[cfg(windows)]
#[test]
fn an_in_process_session_runs_surge_and_tells_rabbit_how_it_went() {
    use crate::uninstall::vendor::tests::windows::{Fixture, install_surge, surge_uninstaller};

    let fixture = Fixture::new();
    let surge = install_surge(&fixture);
    let env = fixture.env(surge_uninstaller(&fixture, &surge));
    let helper = InProcessElevatedHelper::new(
        Arc::new(env),
        vec![elevated_surge("surge-test")],
        fixture.folders.clone(),
    );
    let mut session = helper
        .start(&request(&["surge-test"], &fixture.resource))
        .unwrap();

    let mut events = Vec::new();
    let outcome = session.run("surge-test", &mut |event| events.push(event.clone()));
    assert_eq!(outcome.status, VendorStatus::Removed, "{outcome:?}");
    assert_eq!(events, [VendorEvent::Launched]);
    assert!(!surge.vst3.exists());

    let again = session.run("surge-test", &mut |_| panic!("nothing runs twice"));
    assert_eq!(again.status, VendorStatus::Failed);
    assert!(!again.launched);
    assert!(
        again.error.as_deref().unwrap().contains("refused"),
        "{again:?}"
    );
    let later = session.run("surge-test", &mut |_| panic!("the session is over"));
    assert_eq!(later.status, VendorStatus::Failed);
    session.finish();
}

/// The same loop over a real named pipe: both ends check the other's
/// process ID, here this test's own.
#[cfg(windows)]
#[test]
fn a_session_over_a_real_pipe_reaches_the_helper_and_back() {
    let name = rabbit_platform::new_uninstall_pipe_name().unwrap();
    let server = PipeServer::create(&name).unwrap();
    let args = HelperArgs {
        pipe: name,
        parent_pid: std::process::id(),
        ..args_for(&["surge-xt"])
    };
    assert_eq!(args.validate(), Ok(()));
    let roots = helper_roots(&args, &folders());
    let helper = thread::spawn(move || {
        let mut channel = PipeChannel::connect(&args.pipe, args.parent_pid, LONG).unwrap();
        serve(&mut channel, &args, &[], &NothingToRun::new(), &roots)
    });
    let channel = server.accept(std::process::id(), LONG).unwrap();
    let mut session = ChannelSession::new(channel, HelperHandle::Detached, HashMap::new());
    session.await_ready(std::process::id()).unwrap();

    let outcome = session.run("surge-xt", &mut |_| panic!("nothing runs"));
    assert_eq!(
        outcome,
        VendorOutcome::refused("surge-xt", VendorRefusal::NotAVendorPlan)
    );
    Box::new(session).finish();
    assert_eq!(helper.join().unwrap(), HELPER_EXIT_DONE);
}

/// A helper that names another process ID than the one RABBIT started is
/// refused before any order goes out.
#[test]
fn a_helper_that_isnt_the_one_started_is_refused() {
    let (rabbit, mut helper) = MemoryChannel::pair();
    helper
        .send_line(
            &serde_json::to_string(&HelperEvent::Ready { helper_pid: 1 }).unwrap(),
            LONG,
        )
        .unwrap();
    let mut session = ChannelSession::new(rabbit, HelperHandle::Detached, HashMap::new());
    assert!(matches!(
        session.await_ready(2),
        Err(ElevationStartError::Failed(_))
    ));
}

/// A helper that goes away mid-run leaves RABBIT a failure that says it
/// can't tell how the package ended, and every later package fails
/// without an order going out.
#[test]
fn a_helper_lost_mid_run_fails_the_package_and_the_rest() {
    let (rabbit, mut helper) = MemoryChannel::pair();
    let mut session = ChannelSession::new(rabbit, HelperHandle::Detached, HashMap::new());
    let talking = thread::spawn(move || {
        let order = helper.read_line(LONG).unwrap().unwrap();
        assert_eq!(order, r#"{"order":"run","package_id":"reaper"}"#);
        for event in [
            HelperEvent::Started {
                package_id: "reaper".to_string(),
            },
            HelperEvent::VendorLaunched {
                package_id: "reaper".to_string(),
            },
        ] {
            helper
                .send_line(&serde_json::to_string(&event).unwrap(), LONG)
                .unwrap();
        }
        // ...and the helper is gone.
    });
    let mut events = Vec::new();
    let outcome = session.run("reaper", &mut |event| events.push(event.clone()));
    talking.join().unwrap();
    assert_eq!(outcome.status, VendorStatus::Failed);
    assert!(outcome.launched, "the uninstaller had started");
    assert!(outcome.error.as_deref().unwrap().contains("can't tell"));
    assert_eq!(events, [VendorEvent::Launched]);
    let later = session.run("surge-xt", &mut |_| panic!("no order goes out"));
    assert_eq!(later.status, VendorStatus::Failed);
    assert!(!later.launched);
}
