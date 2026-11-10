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

use rabbit_platform::{PendingFileRename, RegistryView, UninstallViews, uninstall_pipe_name};

use super::*;
use crate::model::{InstallationKind, Platform};
use crate::package::{parse_embedded_package_spec, validate_package_spec};
// The key and the folder the tests' machine cleanup names, shared with the
// cleanup's own tests. No installer uses either, so even a test gone wrong
// couldn't reach a real one.
use crate::uninstall::cleanup::tests::{FOLDER as CLEANUP_FOLDER, KEY as CLEANUP_KEY};
use crate::uninstall::cleanup::{CleanupResult, CleanupStatus, CleanupStep, CleanupTarget};
use crate::uninstall::vendor::{
    ProgramEnd, RunLimits, VendorCommand, VendorEnvironment, VendorOutcome, VendorRefusal,
    VendorStatus,
};

const LONG: Duration = Duration::from_secs(20);

/// An Inno uninstaller as a plan's `vendor_uninstaller` names it, with a
/// time limit of 600 seconds.
const INNO: &str = r#""vendor_uninstaller": {
    "kind": "inno",
    "locate": [{ "registry": { "hive": "hklm", "key": "Demo_is1", "value": "UninstallString" } }],
    "must_be_in": { "dir": "{program_data}/Demo/uninstall", "only_files": "unins000.*" },
    "timeout_seconds": 600
}"#;

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

    fn remove_file_no_follow(&self, _path: &Path) -> io::Result<()> {
        Err(unsupported())
    }

    fn remove_empty_dir(&self, _path: &Path) -> io::Result<()> {
        Err(unsupported())
    }

    fn delete_uninstall_entry(&self, _key: &str, _view: RegistryView) -> io::Result<bool> {
        Err(unsupported())
    }
}

fn folders() -> MachineFolders {
    let root = std::env::temp_dir().join("RABBIT-test-machine");
    MachineFolders {
        program_files: vec![root.join("Program Files"), root.join("Program Files (x86)")],
        program_files_x86: root.join("Program Files (x86)"),
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
            HelperEvent::WaitingForFiles {
                package_id: "surge-xt".to_string(),
                seconds: 30,
            },
            r#"{"event":"waiting-for-files","package_id":"surge-xt","seconds":30}"#,
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
    let cleaned_up = HelperEvent::CleanupFinished {
        package_id: "jaws-scripts".to_string(),
        outcome: Box::new(removed_cleanup("jaws-scripts")),
    };
    let line = serde_json::to_string(&cleaned_up).unwrap();
    assert!(
        line.starts_with(r#"{"event":"cleanup-finished","package_id":"jaws-scripts","#),
        "{line}"
    );
    assert!(!line.contains('\n'));
    assert_eq!(
        serde_json::from_str::<HelperEvent>(&line).unwrap(),
        cleaned_up
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
    for device_or_partial in [
        "\\\\?\\UNC\\server\\share\\REAPER",
        "\\\\?\\C:\\REAPER",
        "\\\\.\\C:\\REAPER",
        "\\\\server\\share",
        "\\\\server\\share\\..\\REAPER",
    ] {
        refused(&|args| args.resource_path = PathBuf::from(device_or_partial));
    }
    refused(&|args| args.target_app_path = Some(PathBuf::from("reaper.exe")));
    // REAPER's app is never on a share; its uninstaller only runs from
    // Program Files.
    refused(&|args| {
        args.target_app_path = Some(PathBuf::from("\\\\server\\share\\REAPER\\reaper.exe"));
    });
    refused(&|args| args.package_ids.clear());
    refused(&|args| args.package_ids.push("surge-xt".to_string()));
    refused(&|args| args.package_ids.push("Surge XT".to_string()));
    refused(&|args| args.package_ids.push("..\\reaper".to_string()));
    refused(&|args| args.package_ids.push(String::new()));
}

/// Windows may redirect `%APPDATA%`, and with it the standard REAPER's
/// resource folder, to a network share. The helper takes that folder too:
/// it only keeps away from it, by its text. Refusing it failed every run
/// with Surge XT or REAPER for such a user, after the dry run and Review
/// had said they would go.
#[cfg(windows)]
#[test]
fn a_resource_folder_on_a_network_share_is_accepted() {
    let mut args = args_for(&["surge-xt", "reaper"]);
    args.resource_path = PathBuf::from("\\\\server\\users$\\someone\\AppData\\Roaming\\REAPER");
    assert_eq!(args.validate(), Ok(()));
    let arguments = args.to_arguments().unwrap();
    assert!(arguments.windows(2).any(|pair| {
        pair[0] == "--resource-path"
            && pair[1] == "\\\\server\\users$\\someone\\AppData\\Roaming\\REAPER"
    }));
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

/// A package with the JAWS scripts' kind of plan: a Windows `remove_files`
/// plan whose only removal is the cleanup's tests' machine cleanup of
/// [`CLEANUP_FOLDER`] and [`CLEANUP_KEY`].
pub(crate) fn cleanup_package(id: &str) -> PackageSpec {
    let cleanup = serde_json::to_string(&crate::uninstall::cleanup::tests::spec()).unwrap();
    package(
        id,
        &format!(r#""strategy": "remove_files", "machine_cleanup": {cleanup}"#),
    )
}

/// A cleanup of `package_id` that removed everything there was.
fn removed_cleanup(package_id: &str) -> CleanupOutcome {
    CleanupOutcome {
        package_id: package_id.to_string(),
        status: CleanupStatus::Removed,
        refusal: None,
        steps: vec![CleanupStep {
            target: CleanupTarget::RegistryKey {
                key: CLEANUP_KEY.to_string(),
            },
            result: CleanupResult::Removed,
        }],
        still_present: Vec::new(),
        error: None,
    }
}

#[test]
fn only_a_plan_that_runs_a_vendor_uninstaller_elevated_is_run() {
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

#[test]
fn only_a_machine_cleanup_for_a_standard_reaper_is_run() {
    let mut not_standard = cleanup_package("not-standard");
    not_standard.requires_standard_install = false;
    let specs = [
        cleanup_package("cleanup"),
        not_standard,
        package(
            "elevated",
            &format!(
                r#""strategy": "vendor_uninstaller", "elevation": "required", "backup": "none", {INNO}"#
            ),
        ),
        package(
            "files",
            r#""strategy": "remove_files", "remove": { "paths": ["{program_data}/Demo/a.dll"] }"#,
        ),
    ];
    let cleanup = elevated_cleanup_plan(&specs, "cleanup", InstallationKind::Standard)
        .expect("a cleanup for a standard REAPER");
    assert_eq!(
        cleanup.dir.as_str(),
        format!("{{program_files_x86}}/{CLEANUP_FOLDER}")
    );
    // A cleanup runs no vendor's uninstaller, and a vendor's uninstaller
    // is no cleanup.
    assert!(elevated_vendor_plan(&specs, "cleanup", InstallationKind::Standard).is_none());
    assert!(elevated_cleanup_plan(&specs, "elevated", InstallationKind::Standard).is_none());
    // The folder and the key serve the whole computer.
    assert!(elevated_cleanup_plan(&specs, "not-standard", InstallationKind::Standard).is_none());
    assert!(elevated_cleanup_plan(&specs, "files", InstallationKind::Standard).is_none());
    assert!(elevated_cleanup_plan(&specs, "missing", InstallationKind::Standard).is_none());
}

/// A portable REAPER's uninstall never removes what serves the whole
/// computer: the helper runs nothing for the package, and says so the way
/// it does for a package it has no plan for.
#[test]
fn the_helper_refuses_a_cleanup_for_a_portable_target() {
    let specs = vec![cleanup_package("jaws-test")];
    assert!(elevated_cleanup_plan(&specs, "jaws-test", InstallationKind::Standard).is_some());
    assert!(elevated_cleanup_plan(&specs, "jaws-test", InstallationKind::Portable).is_none());

    let (mut rabbit, mut helper) = MemoryChannel::pair();
    let args = HelperArgs {
        target_app_path: None,
        target: InstallationKind::Portable,
        ..args_for(&["jaws-test"])
    };
    let roots = helper_roots(&args, &folders());
    let serving =
        thread::spawn(move || serve(&mut helper, &args, &specs, &NothingToRun::new(), &roots));
    assert_eq!(
        next_event(&mut rabbit),
        HelperEvent::Ready {
            helper_pid: std::process::id()
        }
    );
    expect_finished_without_a_plan(&mut rabbit, "jaws-test");
    order(&mut rabbit, &HelperOrder::Quit {});
    assert_eq!(serving.join().unwrap(), HELPER_EXIT_DONE);
}

/// The helper loop deletes the cleanup's files, its folder and its registry
/// entry, and answers with `cleanup-finished`; RABBIT's session hands that
/// outcome on as it came. The folders are fixtures and the list of
/// installed programs is a throwaway HKCU key.
#[cfg(windows)]
#[test]
fn the_helper_runs_a_cleanup_plan_and_answers_with_cleanup_finished() {
    use crate::uninstall::cleanup::tests::windows::install;
    use crate::uninstall::vendor::tests::windows::{Fixture, never_runs};

    // The helper loop itself.
    let fixture = Fixture::new();
    let folder = install(&fixture).folder;
    let (mut rabbit, mut helper) = MemoryChannel::pair();
    let args = HelperArgs {
        resource_path: fixture.resource.clone(),
        target_app_path: None,
        ..args_for(&["jaws-test"])
    };
    let roots = helper_roots(&args, &fixture.folders);
    let specs = vec![cleanup_package("jaws-test")];
    let env = fixture.env(never_runs());
    let serving = thread::spawn(move || serve(&mut helper, &args, &specs, &env, &roots));
    assert_eq!(
        next_event(&mut rabbit),
        HelperEvent::Ready {
            helper_pid: std::process::id()
        }
    );
    run_order(&mut rabbit, "jaws-test");
    assert_eq!(
        next_event(&mut rabbit),
        HelperEvent::Started {
            package_id: "jaws-test".to_string()
        }
    );
    let HelperEvent::CleanupFinished {
        package_id,
        outcome,
    } = next_event(&mut rabbit)
    else {
        panic!("the helper didn't answer with cleanup-finished");
    };
    assert_eq!(
        (package_id.as_str(), outcome.package_id.as_str()),
        ("jaws-test", "jaws-test")
    );
    assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
    let shown_key = fixture
        .list
        .views()
        .display_key(CLEANUP_KEY, RegistryView::Bit32);
    assert_eq!(
        outcome.steps,
        vec![
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
                result: CleanupResult::Removed,
            },
            CleanupStep {
                target: CleanupTarget::Folder {
                    path: folder.clone(),
                },
                result: CleanupResult::Removed,
            },
            CleanupStep {
                target: CleanupTarget::RegistryKey { key: shown_key },
                result: CleanupResult::Removed,
            },
        ]
    );
    assert!(std::fs::symlink_metadata(&folder).is_err());
    assert!(!fixture.list.views().key_exists(CLEANUP_KEY).unwrap());
    order(&mut rabbit, &HelperOrder::Quit {});
    assert_eq!(serving.join().unwrap(), HELPER_EXIT_DONE);

    // RABBIT's session, against the same loop on the in-process helper.
    let fixture = Fixture::new();
    let folder = install(&fixture).folder;
    let helper = InProcessElevatedHelper::new(
        Arc::new(fixture.env(never_runs())),
        vec![cleanup_package("jaws-test")],
        fixture.folders.clone(),
    );
    let mut session = helper
        .start(&request(&["jaws-test"], &fixture.resource))
        .unwrap();
    let outcome = session.run_cleanup("jaws-test");
    assert_eq!(outcome.status, CleanupStatus::Removed, "{outcome:?}");
    assert_eq!(outcome.steps.len(), 4, "{outcome:?}");
    assert!(std::fs::symlink_metadata(&folder).is_err());
    assert!(!fixture.list.views().key_exists(CLEANUP_KEY).unwrap());

    // No package is cleaned up twice: the helper refuses, which ends the
    // session.
    let again = session.run_cleanup("jaws-test");
    assert_eq!(again.status, CleanupStatus::Lost, "{again:?}");
    assert!(again.steps.is_empty());
    assert!(
        again.error.as_deref().unwrap().contains("refused"),
        "{again:?}"
    );
    let later = session.run_cleanup("jaws-test");
    assert_eq!(later.status, CleanupStatus::Lost);
    assert!(
        later.error.as_deref().unwrap().contains("already lost"),
        "{later:?}"
    );
    session.finish();
}

/// Run a cleanup of `jaws-test` on a session whose helper answers the order
/// with `lines`, then stays, or goes away if `goes_away`. Returns the
/// outcome, the session and the helper's end, if it stayed.
fn cleanup_answered_with(
    lines: Vec<String>,
    goes_away: bool,
) -> (
    CleanupOutcome,
    ChannelSession<MemoryChannel>,
    Option<MemoryChannel>,
) {
    let (rabbit, mut helper) = MemoryChannel::pair();
    let waits = HashMap::from([("jaws-test".to_string(), LONG)]);
    let mut session = ChannelSession::new(rabbit, HelperHandle::Detached, waits);
    let talking = thread::spawn(move || {
        let order = helper.read_line(LONG).unwrap().unwrap();
        assert_eq!(order, r#"{"order":"run","package_id":"jaws-test"}"#);
        for line in lines {
            helper.send_line(&line, LONG).unwrap();
        }
        (!goes_away).then_some(helper)
    });
    let outcome = session.run_cleanup("jaws-test");
    let helper = talking.join().unwrap();
    (outcome, session, helper)
}

fn lines(events: &[HelperEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| serde_json::to_string(event).unwrap())
        .collect()
}

/// RABBIT takes only `started`, then `cleanup-finished`, both for the
/// package it asked for, as the answer to a cleanup. Any other answer ends
/// the session: the outcome is lost, and no later order goes out.
#[test]
fn a_session_expecting_a_cleanup_loses_contact_on_any_other_answer() {
    let started = || HelperEvent::Started {
        package_id: "jaws-test".to_string(),
    };
    let finished_for = |event_id: &str, outcome_id: &str| HelperEvent::CleanupFinished {
        package_id: event_id.to_string(),
        outcome: Box::new(removed_cleanup(outcome_id)),
    };

    // The one right answer comes back as it was sent.
    let (outcome, session, _helper) = cleanup_answered_with(
        lines(&[started(), finished_for("jaws-test", "jaws-test")]),
        false,
    );
    assert_eq!(outcome, removed_cleanup("jaws-test"));
    assert!(session.lost.is_none());
    Box::new(session).finish();

    let wrong_answers: Vec<(&str, Vec<String>, bool)> = vec![
        (
            "a vendor run's answer",
            lines(&[
                started(),
                HelperEvent::Finished {
                    package_id: "jaws-test".to_string(),
                    outcome: Box::new(VendorOutcome::refused(
                        "jaws-test",
                        VendorRefusal::NotAVendorPlan,
                    )),
                },
            ]),
            false,
        ),
        (
            "finished without starting",
            lines(&[finished_for("jaws-test", "jaws-test")]),
            false,
        ),
        ("started twice", lines(&[started(), started()]), false),
        (
            "another package started",
            lines(&[HelperEvent::Started {
                package_id: "reaper".to_string(),
            }]),
            false,
        ),
        (
            "another package finished",
            lines(&[started(), finished_for("reaper", "jaws-test")]),
            false,
        ),
        (
            "an outcome for another package",
            lines(&[started(), finished_for("jaws-test", "reaper")]),
            false,
        ),
        (
            "a vendor uninstaller launched",
            lines(&[
                started(),
                HelperEvent::VendorLaunched {
                    package_id: "jaws-test".to_string(),
                },
            ]),
            false,
        ),
        (
            "ready again",
            lines(&[HelperEvent::Ready { helper_pid: 1 }]),
            false,
        ),
        (
            "a line that isn't an event",
            vec![r#"{"event":"cleanup-finished","package_id":"jaws-test"}"#.to_string()],
            false,
        ),
        ("the helper went away", lines(&[started()]), true),
        (
            "the helper refused",
            lines(&[HelperEvent::Refused {
                reason: HelperRefusal::AlreadyRun {
                    package_id: "jaws-test".to_string(),
                },
            }]),
            false,
        ),
    ];
    for (case, answer, goes_away) in wrong_answers {
        let (outcome, mut session, helper) = cleanup_answered_with(answer, goes_away);
        assert_eq!(outcome.status, CleanupStatus::Lost, "{case}: {outcome:?}");
        assert_eq!(outcome.package_id, "jaws-test", "{case}");
        assert!(outcome.steps.is_empty(), "{case}: {outcome:?}");
        assert!(outcome.error.is_some(), "{case}");
        assert!(session.lost.is_some(), "{case}");

        let later = session.run_cleanup("jaws-test");
        assert_eq!(later.status, CleanupStatus::Lost, "{case}");
        assert!(
            later.error.as_deref().unwrap().contains("already lost"),
            "{case}: {later:?}"
        );
        if let Some(mut helper) = helper {
            assert!(
                helper.read_line(Duration::from_millis(100)).is_err(),
                "{case}: no order went out"
            );
        }
    }
}

#[test]
fn the_waits_cover_cleanup_packages() {
    let specs = [
        cleanup_package("jaws-test"),
        package(
            "vendor",
            &format!(
                r#""strategy": "vendor_uninstaller", "elevation": "required", "backup": "none", {INNO}"#
            ),
        ),
    ];
    let standard = request(&["jaws-test", "vendor", "unknown"], &std::env::temp_dir());
    assert_eq!(
        package_waits(&specs, &standard),
        HashMap::from([
            ("jaws-test".to_string(), CLEANUP_WAIT),
            ("vendor".to_string(), Duration::from_secs(600 * 2 + 120)),
        ])
    );
    assert_eq!(CLEANUP_WAIT, Duration::from_secs(60));
    // Neither runs for a portable REAPER, so neither has a wait of its own.
    let portable = ElevatedRunRequest {
        target: InstallationKind::Portable,
        ..standard
    };
    assert!(package_waits(&specs, &portable).is_empty());
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
        roots.program_files_x86,
        Some(folders.program_files_x86.clone())
    );
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
        fn remove_file_no_follow(&self, _path: &Path) -> io::Result<()> {
            Err(unsupported())
        }
        fn remove_empty_dir(&self, _path: &Path) -> io::Result<()> {
            Err(unsupported())
        }
        fn delete_uninstall_entry(&self, _key: &str, _view: RegistryView) -> io::Result<bool> {
            Err(unsupported())
        }
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

/// An in-process helper only works with machine-wide folders below the
/// temporary folder, by their real paths, so a test's machine cleanup can't
/// reach the real Program Files however it is put together. It refuses
/// before it would ask for permission.
#[test]
fn an_in_process_helper_refuses_machine_folders_outside_the_temp_folder() {
    let temp = std::env::temp_dir();
    let refused = |change: &dyn Fn(&mut MachineFolders)| {
        let mut folders = folders();
        change(&mut folders);
        let helper = InProcessElevatedHelper::new(
            Arc::new(NothingToRun::new()),
            vec![cleanup_package("jaws-test")],
            folders.clone(),
        )
        .declining();
        match helper.start(&request(&["jaws-test"], &temp)) {
            Err(ElevationStartError::Failed(why)) => {
                assert!(why.contains("temporary folder"), "{folders:?}: {why}");
            }
            Err(other) => panic!("{folders:?}: expected a refusal, got {other:?}"),
            Ok(_) => panic!("{folders:?}: the helper started"),
        }
    };
    let outside = [
        // The temporary folder itself isn't below it...
        temp.clone(),
        // ...nor is anything beside it, there or not...
        temp.parent().unwrap().to_path_buf(),
        temp.parent().unwrap().join("RABBIT-test-machine"),
        // ...nor a way out of it, or a path that isn't a full one.
        temp.join("RABBIT-test-machine").join("..").join(".."),
        PathBuf::from("Program Files (x86)"),
    ];
    for folder in &outside {
        refused(&|folders| folders.program_files_x86 = folder.clone());
        refused(&|folders| folders.program_files[0] = folder.clone());
        refused(&|folders| folders.program_files[1] = folder.clone());
        refused(&|folders| folders.common_program_files = folder.clone());
        refused(&|folders| folders.program_data = folder.clone());
    }

    // Folders below it start a session, whether they are there yet or not.
    let helper = InProcessElevatedHelper::new(
        Arc::new(NothingToRun::new()),
        vec![cleanup_package("jaws-test")],
        folders(),
    );
    let session = helper.start(&request(&["jaws-test"], &temp)).unwrap();
    session.finish();
}

/// A link below the temporary folder whose target is gone could lead
/// anywhere once something is made there, so a folder behind it is refused.
/// The link's target was a second temporary folder, so nothing outside the
/// temporary folder is ever linked to.
#[cfg(windows)]
#[test]
fn an_in_process_helper_refuses_a_machine_folder_behind_a_dangling_link() {
    use crate::uninstall::paths::make_junction;

    let dir = tempfile::tempdir().unwrap();
    let gone = tempfile::tempdir().unwrap();
    let link = dir.path().join("Program Files (x86)");
    make_junction(&link, gone.path());
    drop(gone);
    let mut folders = folders();
    folders.program_files_x86 = link.join("Inside");
    let helper = InProcessElevatedHelper::new(Arc::new(NothingToRun::new()), Vec::new(), folders);
    assert!(matches!(
        helper.start(&request(&["jaws-test"], &std::env::temp_dir())),
        Err(ElevationStartError::Failed(why)) if why.contains("temporary folder")
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

/// Waiting for the files to go after the uninstaller ended crosses the
/// pipe like still waiting does, so RABBIT can tell the user.
#[test]
fn waiting_for_files_reaches_rabbit() {
    let (rabbit, mut helper) = MemoryChannel::pair();
    let mut session = ChannelSession::new(rabbit, HelperHandle::Detached, HashMap::new());
    let talking = thread::spawn(move || {
        helper.read_line(LONG).unwrap().unwrap();
        for event in [
            HelperEvent::Started {
                package_id: "surge-xt".to_string(),
            },
            HelperEvent::VendorLaunched {
                package_id: "surge-xt".to_string(),
            },
            HelperEvent::WaitingForFiles {
                package_id: "surge-xt".to_string(),
                seconds: 30,
            },
            HelperEvent::Finished {
                package_id: "surge-xt".to_string(),
                outcome: Box::new(VendorOutcome {
                    status: VendorStatus::Removed,
                    launched: true,
                    exit_code: Some(0),
                    ..VendorOutcome::new("surge-xt")
                }),
            },
        ] {
            helper
                .send_line(&serde_json::to_string(&event).unwrap(), LONG)
                .unwrap();
        }
    });
    let mut events = Vec::new();
    let outcome = session.run("surge-xt", &mut |event| events.push(event.clone()));
    talking.join().unwrap();
    assert_eq!(outcome.status, VendorStatus::Removed);
    assert_eq!(
        events,
        [
            VendorEvent::Launched,
            VendorEvent::WaitingForFiles { seconds: 30 }
        ]
    );
}
