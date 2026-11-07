//! RABBIT's elevated uninstall helper, and how RABBIT talks to it.
//!
//! One uninstall run raises at most one Windows permission prompt, before
//! anything changes. When a selected package needs administrator rights,
//! RABBIT starts a copy of itself with them, hidden, through
//! [`HELPER_SUBCOMMAND`]. The run then goes on in its usual order, and the
//! helper does every step that needs those rights, when RABBIT asks.
//!
//! The two talk over a named pipe, one JSON object per line. RABBIT sends
//! [`HelperOrder`]s, the helper answers with [`HelperEvent`]s:
//!
//! ```text
//! helper -> RABBIT  {"event":"ready","helper_pid":4242}
//! RABBIT -> helper  {"order":"run","package_id":"surge-xt"}
//! helper -> RABBIT  {"event":"started","package_id":"surge-xt"}
//! helper -> RABBIT  {"event":"vendor-launched","package_id":"surge-xt"}
//! helper -> RABBIT  {"event":"still-waiting","package_id":"surge-xt","seconds":30}
//! helper -> RABBIT  {"event":"waiting-for-files","package_id":"surge-xt","seconds":30}
//! helper -> RABBIT  {"event":"finished","package_id":"surge-xt","outcome":{...}}
//! RABBIT -> helper  {"order":"quit"}
//! ```
//!
//! The helper trusts very little:
//! - It talks only to the process its command line names as its parent, and
//!   the pipe checks that process ID on both ends.
//! - It runs only the packages its command line names, each at most once,
//!   in that order. Any other order is refused, and ends the session.
//! - It looks every package up itself, in the manifests built into RABBIT,
//!   and repeats every check from HKLM and the real file system before it
//!   starts anything ([`super::vendor`]).
//! - It never reads HKCU, `%APPDATA%` or any other per-user place: when a
//!   standard user types an administrator's password, the helper runs as
//!   that administrator, another account. The machine-wide folders it reads
//!   from what only administrators can change, never from its environment
//!   ([`MachineFolders::of_this_computer`]); the resource folder and
//!   REAPER's app are the only per-user paths, and they come from its
//!   command line.
//! - A vendor uninstaller it has started always runs to its end, or to its
//!   time limit, whatever happens to the pipe.
//! - It loads DLLs from System32 only, never from the folder rabbit.exe
//!   sits in, which for a portable RABBIT in Downloads anyone in the user's
//!   session can write to: rabbit.exe's own imports are linked that way
//!   (`crates/rabbit/build.rs`), and the helper's `main` sets the same rule
//!   for anything loaded later. What stays open is replacing rabbit.exe
//!   itself; the permission prompt then names no verified publisher
//!   instead of RABBIT's signature.
//!
//! Nothing here can start the real helper from a test. The helper's entry
//! point and the launcher both refuse unless the running program is a
//! RABBIT build by its file name, which no cargo test program ever is, and
//! refuse in this crate's own tests too. Tests use
//! [`InProcessElevatedHelper`], which runs the same helper loop on a thread,
//! with no rights beyond the test's own, and refuses Windows' real list of
//! installed programs.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use rabbit_platform::{
    ElevatedChild, ElevatedWindow, ElevationError, PipeChannel, PipeServer, is_uninstall_pipe_name,
    new_uninstall_pipe_name, spawn_elevated,
};
use serde::{Deserialize, Serialize};

use crate::model::{InstallationKind, Platform};
use crate::package::uninstall::{UninstallElevation, UninstallStrategy, VendorUninstallerSpec};
use crate::package::{PackageSpec, builtin_package_specs};

use super::paths::{UninstallRoots, absolute};
use super::vendor::{
    SystemVendorEnvironment, VendorEnvironment, VendorEvent, VendorJob, VendorOutcome,
    VendorRefusal, is_full_drive_path, is_full_share_path, run_vendor_uninstaller,
};

#[cfg(test)]
#[path = "helper_tests.rs"]
pub(crate) mod tests;

/// The hidden command RABBIT starts itself with to run as the helper.
pub const HELPER_SUBCOMMAND: &str = "__uninstall-helper";

/// The helper's exit codes. Clap itself uses 2 for a command line it can't
/// parse.
pub const HELPER_EXIT_DONE: i32 = 0;
/// RABBIT sent an order the helper refused; the session ended there.
pub const HELPER_EXIT_REFUSED_ORDER: i32 = 3;
/// Not a RABBIT build, or a test: the helper never runs there.
pub const HELPER_EXIT_NOT_ALLOWED: i32 = 4;
pub const HELPER_EXIT_BAD_ARGUMENTS: i32 = 5;
/// Started without administrator rights.
pub const HELPER_EXIT_NOT_ELEVATED: i32 = 6;
/// The pipe wasn't there, or its server wasn't the parent.
pub const HELPER_EXIT_NO_PARENT: i32 = 7;
/// The pipe broke or stayed silent too long.
pub const HELPER_EXIT_PIPE_FAILED: i32 = 8;
/// Windows didn't say where Program Files or ProgramData are.
pub const HELPER_EXIT_NO_FOLDERS: i32 = 9;

/// How long the helper waits to connect to RABBIT's pipe, and RABBIT for
/// the helper to connect and say it is ready, once the permission prompt
/// was answered.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the helper waits for RABBIT's next order. RABBIT removes other
/// packages' files between two vendor runs, which can take a while; a
/// RABBIT that has gone closes the pipe, which ends the helper at once.
const ORDER_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// How long a line may take to be taken by the other end.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);
/// How long RABBIT waits for an answer about a package it knows nothing of.
const UNKNOWN_PACKAGE_WAIT: Duration = Duration::from_secs(120);
/// How long RABBIT waits for the helper to end after the last order.
const FINISH_WAIT: Duration = Duration::from_secs(10);

/// What the helper's command line says, after [`HELPER_SUBCOMMAND`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperArgs {
    /// The pipe RABBIT made, `\\.\pipe\rabbit-uninstall-<pid>-<random>`.
    pub pipe: String,
    /// RABBIT's process ID, which must serve that pipe.
    pub parent_pid: u32,
    /// The target REAPER's resource folder.
    pub resource_path: PathBuf,
    /// The target REAPER's `reaper.exe`, for REAPER's own uninstaller.
    pub target_app_path: Option<PathBuf>,
    pub target: InstallationKind,
    /// The packages the helper may run, in the order it may run them.
    pub package_ids: Vec<String>,
}

impl HelperArgs {
    /// The arguments that follow [`HELPER_SUBCOMMAND`] on the helper's
    /// command line. A path that isn't valid Unicode can't be passed on
    /// unchanged, so it is refused.
    pub fn to_arguments(&self) -> Result<Vec<String>, String> {
        let text = |path: &PathBuf| {
            path.to_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{} isn't valid Unicode", path.display()))
        };
        let mut arguments = vec![
            "--pipe".to_string(),
            self.pipe.clone(),
            "--parent-pid".to_string(),
            self.parent_pid.to_string(),
            "--resource-path".to_string(),
            text(&self.resource_path)?,
        ];
        if let Some(app) = &self.target_app_path {
            arguments.push("--target-app-path".to_string());
            arguments.push(text(app)?);
        }
        arguments.push("--target-kind".to_string());
        arguments.push(
            match self.target {
                InstallationKind::Standard => "standard",
                InstallationKind::Portable => "portable",
            }
            .to_string(),
        );
        for id in &self.package_ids {
            arguments.push("--package".to_string());
            arguments.push(id.clone());
        }
        Ok(arguments)
    }

    /// Whether the command line is one RABBIT would have written: a pipe
    /// name of exactly RABBIT's form that carries the parent's process ID,
    /// full paths, and at least one package, each named once. REAPER's app
    /// must be on a drive letter. The resource folder may also be on a
    /// network share, as `%APPDATA%` is when Windows redirects it there:
    /// the helper only compares other paths with it, by their text, and
    /// never opens it ([`super::paths`] doesn't resolve network paths).
    pub fn validate(&self) -> Result<(), String> {
        if !is_uninstall_pipe_name(&self.pipe) {
            return Err(format!("{:?} isn't an uninstall pipe's name", self.pipe));
        }
        if pipe_owner_pid(&self.pipe) != Some(self.parent_pid) {
            return Err(format!(
                "the pipe {:?} wasn't made by process {}",
                self.pipe, self.parent_pid
            ));
        }
        if !is_full_drive_path(&self.resource_path) && !is_full_share_path(&self.resource_path) {
            return Err(format!(
                "{} isn't a full path on a drive or a network share",
                self.resource_path.display()
            ));
        }
        if let Some(app) = &self.target_app_path
            && !is_full_drive_path(app)
        {
            return Err(format!("{} isn't a full path on a drive", app.display()));
        }
        if self.package_ids.is_empty() {
            return Err("no package to run".to_string());
        }
        for (index, id) in self.package_ids.iter().enumerate() {
            let well_formed = !id.is_empty()
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
            if !well_formed {
                return Err(format!("{id:?} isn't a package id"));
            }
            if self.package_ids[..index].contains(id) {
                return Err(format!("{id:?} is named twice"));
            }
        }
        Ok(())
    }
}

/// The process ID an uninstall pipe's name carries.
fn pipe_owner_pid(pipe: &str) -> Option<u32> {
    let rest = pipe.strip_prefix(rabbit_platform::UNINSTALL_PIPE_PREFIX)?;
    rest.split_once('-')?.0.parse().ok()
}

/// What RABBIT asks the helper to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "order", rename_all = "kebab-case", deny_unknown_fields)]
pub enum HelperOrder {
    /// Run the elevated steps of this package.
    Run { package_id: String },
    /// Nothing more to do; end. A struct variant, so that, like `Run`, it
    /// carries nothing it doesn't name.
    Quit {},
}

impl HelperOrder {
    /// Read one order from its line. Only a JSON object naming exactly the
    /// fields of one order reads; serde would also take the same fields as
    /// an array, which RABBIT never sends.
    pub fn parse(line: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|error| error.to_string())?;
        if !value.is_object() {
            return Err("an order is a JSON object".to_string());
        }
        serde_json::from_value(value).map_err(|error| error.to_string())
    }
}

/// What the helper tells RABBIT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub enum HelperEvent {
    /// Connected and waiting for orders. `helper_pid` is the helper's own
    /// process ID, which RABBIT compares with the one it started.
    Ready { helper_pid: u32 },
    /// The package's steps have begun: the checks first.
    Started { package_id: String },
    /// The vendor's uninstaller has started; its work can't be undone.
    VendorLaunched { package_id: String },
    /// The uninstaller has run for `seconds` and hasn't ended.
    StillWaiting { package_id: String, seconds: u64 },
    /// The uninstaller has ended, but `seconds` later its files are still
    /// there, and the helper keeps checking.
    WaitingForFiles { package_id: String, seconds: u64 },
    Finished {
        package_id: String,
        outcome: Box<VendorOutcome>,
    },
    /// The helper refused the last order, and ended the session.
    Refused { reason: HelperRefusal },
}

/// Why the helper refused an order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum HelperRefusal {
    #[error("{package_id} isn't one of the packages the helper was started for")]
    NotOnCommandLine { package_id: String },
    #[error("{package_id} has already been run")]
    AlreadyRun { package_id: String },
    #[error("{package_id} comes before a package that has already been run")]
    OutOfOrder { package_id: String },
    #[error("the order couldn't be read: {error}")]
    UnreadableOrder { error: String },
}

/// One line at a time between RABBIT and its helper: the named pipe in
/// earnest, a channel between two threads for [`InProcessElevatedHelper`].
pub(crate) trait LineChannel: Send {
    fn send_line(&mut self, line: &str, timeout: Duration) -> io::Result<()>;
    /// `Ok(None)` once the other end has gone.
    fn read_line(&mut self, timeout: Duration) -> io::Result<Option<String>>;
}

impl LineChannel for PipeChannel {
    fn send_line(&mut self, line: &str, timeout: Duration) -> io::Result<()> {
        PipeChannel::send_line(self, line, timeout)
    }

    fn read_line(&mut self, timeout: Duration) -> io::Result<Option<String>> {
        PipeChannel::read_line(self, timeout)
    }
}

/// Lines between two threads of one process.
pub(crate) struct MemoryChannel {
    outgoing: mpsc::Sender<String>,
    incoming: mpsc::Receiver<String>,
}

impl MemoryChannel {
    /// Two connected ends.
    pub(crate) fn pair() -> (Self, Self) {
        let (to_second, from_first) = mpsc::channel();
        let (to_first, from_second) = mpsc::channel();
        (
            Self {
                outgoing: to_second,
                incoming: from_second,
            },
            Self {
                outgoing: to_first,
                incoming: from_first,
            },
        )
    }
}

impl LineChannel for MemoryChannel {
    fn send_line(&mut self, line: &str, _timeout: Duration) -> io::Result<()> {
        if line.contains(['\n', '\r']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a message must fit on one line",
            ));
        }
        self.outgoing
            .send(line.to_string())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the other end has gone"))
    }

    fn read_line(&mut self, timeout: Duration) -> io::Result<Option<String>> {
        match self.incoming.recv_timeout(timeout) {
            Ok(line) => Ok(Some(line)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "no message arrived in time",
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
        }
    }
}

fn send<T: Serialize>(channel: &mut dyn LineChannel, message: &T) -> io::Result<()> {
    let line = serde_json::to_string(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    channel.send_line(&line, SEND_TIMEOUT)
}

/// Which orders the helper still takes: the packages from its command line,
/// each at most once, in order. A package may be left out, since RABBIT's
/// run can stop, but never come back.
struct OrderBook {
    package_ids: Vec<String>,
    ran: Vec<bool>,
    next: usize,
}

impl OrderBook {
    fn new(package_ids: &[String]) -> Self {
        Self {
            package_ids: package_ids.to_vec(),
            ran: vec![false; package_ids.len()],
            next: 0,
        }
    }

    fn admit(&mut self, package_id: &str) -> Result<(), HelperRefusal> {
        let refusal_id = package_id.to_string();
        let Some(index) = self.package_ids.iter().position(|id| id == package_id) else {
            return Err(HelperRefusal::NotOnCommandLine {
                package_id: refusal_id,
            });
        };
        if self.ran[index] {
            return Err(HelperRefusal::AlreadyRun {
                package_id: refusal_id,
            });
        }
        if index < self.next {
            return Err(HelperRefusal::OutOfOrder {
                package_id: refusal_id,
            });
        }
        self.ran[index] = true;
        self.next = index + 1;
        Ok(())
    }
}

/// The helper's loop: say it is ready, then carry out RABBIT's orders until
/// RABBIT says to quit, goes away, or sends an order it may not. Returns
/// the helper's exit code.
pub(crate) fn serve(
    channel: &mut dyn LineChannel,
    args: &HelperArgs,
    specs: &[PackageSpec],
    env: &dyn VendorEnvironment,
    roots: &UninstallRoots,
) -> i32 {
    let ready = HelperEvent::Ready {
        helper_pid: std::process::id(),
    };
    if send(channel, &ready).is_err() {
        return HELPER_EXIT_PIPE_FAILED;
    }
    let mut orders = OrderBook::new(&args.package_ids);
    loop {
        let line = match channel.read_line(ORDER_TIMEOUT) {
            Ok(Some(line)) => line,
            Ok(None) => return HELPER_EXIT_DONE,
            Err(_) => return HELPER_EXIT_PIPE_FAILED,
        };
        let order = match HelperOrder::parse(&line) {
            Ok(order) => order,
            Err(error) => {
                let reason = HelperRefusal::UnreadableOrder { error };
                let _ = send(channel, &HelperEvent::Refused { reason });
                return HELPER_EXIT_REFUSED_ORDER;
            }
        };
        let package_id = match order {
            HelperOrder::Quit {} => return HELPER_EXIT_DONE,
            HelperOrder::Run { package_id } => package_id,
        };
        if let Err(reason) = orders.admit(&package_id) {
            let _ = send(channel, &HelperEvent::Refused { reason });
            return HELPER_EXIT_REFUSED_ORDER;
        }
        let started = HelperEvent::Started {
            package_id: package_id.clone(),
        };
        if send(channel, &started).is_err() {
            return HELPER_EXIT_PIPE_FAILED;
        }
        // A broken pipe while the uninstaller runs changes nothing about
        // the run: it goes on to its end, and only the telling stops.
        let outcome = run_package(&package_id, args.target, specs, roots, env, &mut |event| {
            let event = match event {
                VendorEvent::Launched => HelperEvent::VendorLaunched {
                    package_id: package_id.clone(),
                },
                VendorEvent::StillWaiting { seconds } => HelperEvent::StillWaiting {
                    package_id: package_id.clone(),
                    seconds,
                },
                VendorEvent::WaitingForFiles { seconds } => HelperEvent::WaitingForFiles {
                    package_id: package_id.clone(),
                    seconds,
                },
            };
            let _ = send(channel, &event);
        });
        let finished = HelperEvent::Finished {
            package_id: package_id.clone(),
            outcome: Box::new(outcome),
        };
        if send(channel, &finished).is_err() {
            return HELPER_EXIT_PIPE_FAILED;
        }
    }
}

/// The vendor uninstaller plan of `package_id` for a `target` REAPER on
/// Windows, as the manifests built into RABBIT say: only a plan that runs
/// a vendor's uninstaller with administrator rights.
pub fn elevated_vendor_plan<'a>(
    specs: &'a [PackageSpec],
    package_id: &str,
    target: InstallationKind,
) -> Option<&'a VendorUninstallerSpec> {
    let spec = specs.iter().find(|spec| spec.id == package_id)?;
    if spec.requires_standard_install && target != InstallationKind::Standard {
        return None;
    }
    let plan = spec.uninstall.plan_for(Platform::Windows, target)?;
    let elevated_vendor_run = plan.strategy == UninstallStrategy::VendorUninstaller
        && plan.elevation == UninstallElevation::Required;
    elevated_vendor_run
        .then_some(plan.vendor_uninstaller.as_ref())
        .flatten()
}

/// Run one package's elevated steps.
pub(crate) fn run_package(
    package_id: &str,
    target: InstallationKind,
    specs: &[PackageSpec],
    roots: &UninstallRoots,
    env: &dyn VendorEnvironment,
    events: &mut dyn FnMut(VendorEvent),
) -> VendorOutcome {
    let Some(spec) = elevated_vendor_plan(specs, package_id, target) else {
        return VendorOutcome::refused(package_id, VendorRefusal::NotAVendorPlan);
    };
    let job = VendorJob {
        package_id,
        spec,
        roots,
    };
    run_vendor_uninstaller(&job, env, events)
}

/// The machine-wide folders the helper's paths may start with, as Windows
/// names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFolders {
    /// `C:\Program Files`, then `C:\Program Files (x86)`.
    pub program_files: Vec<PathBuf>,
    pub common_program_files: PathBuf,
    pub program_data: PathBuf,
}

impl MachineFolders {
    /// Read from what only administrators can change, never from the
    /// environment, which an elevated process may inherit from the standard
    /// user's session: Program Files and Common Files as Windows names them,
    /// and ProgramData from HKLM with Windows' own drive (see
    /// [`rabbit_platform::program_data_dir`]).
    pub fn of_this_computer() -> io::Result<Self> {
        Ok(Self {
            program_files: rabbit_platform::machine_program_files_dirs()?,
            common_program_files: rabbit_platform::machine_common_program_files_dir()?,
            program_data: rabbit_platform::program_data_dir()?,
        })
    }
}

/// What the helper's path tokens stand for: the machine-wide folders, and
/// the resource folder and REAPER's app from its command line. Every
/// per-user folder stays unknown, so a plan that names one is refused.
pub fn helper_roots(args: &HelperArgs, folders: &MachineFolders) -> UninstallRoots {
    let app_path = args.target_app_path.as_deref().map(absolute);
    UninstallRoots {
        resource: absolute(&args.resource_path),
        app_dir: app_path
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf),
        app_path,
        appdata: None,
        localappdata: None,
        clap_dir: None,
        common_program_files: Some(folders.common_program_files.clone()),
        program_files: folders.program_files.clone(),
        program_data: Some(folders.program_data.clone()),
        home: None,
        documents: None,
        // Only Windows has this helper, and no Windows plan names an
        // absolute macOS path, so this is never resolved.
        system_root: PathBuf::from(super::paths::SYSTEM_ROOT),
    }
}

/// Whether this process may act as the elevated helper: a RABBIT build,
/// on Windows, and never a test.
fn helper_may_run() -> bool {
    !cfg!(test) && cfg!(windows) && rabbit_platform::running_as_rabbit()
}

/// The elevated helper's entry point, for [`HELPER_SUBCOMMAND`]. Returns
/// the exit code; it prints nothing, since it has no console to print to.
///
/// It refuses to do anything unless it is a RABBIT build running with
/// administrator rights, its command line is well formed, and the process
/// serving the pipe is the parent the command line names.
pub fn run_elevated_helper(args: &HelperArgs) -> i32 {
    if !helper_may_run() {
        return HELPER_EXIT_NOT_ALLOWED;
    }
    if args.validate().is_err() {
        return HELPER_EXIT_BAD_ARGUMENTS;
    }
    if !matches!(rabbit_platform::is_current_process_elevated(), Ok(true)) {
        return HELPER_EXIT_NOT_ELEVATED;
    }
    let Ok(folders) = MachineFolders::of_this_computer() else {
        return HELPER_EXIT_NO_FOLDERS;
    };
    let Ok(mut channel) = PipeChannel::connect(&args.pipe, args.parent_pid, CONNECT_TIMEOUT) else {
        return HELPER_EXIT_NO_PARENT;
    };
    let roots = helper_roots(args, &folders);
    let specs = builtin_package_specs(Platform::Windows);
    let env = SystemVendorEnvironment::new();
    serve(&mut channel, args, &specs, &env, &roots)
}

/// What the helper's exit code means, in English, for a report.
pub fn helper_exit_message(code: i32) -> String {
    match code {
        HELPER_EXIT_DONE => "it ended normally".to_string(),
        HELPER_EXIT_REFUSED_ORDER => "it refused an order".to_string(),
        HELPER_EXIT_NOT_ALLOWED => "it isn't allowed to run here".to_string(),
        HELPER_EXIT_BAD_ARGUMENTS => "its command line wasn't one RABBIT writes".to_string(),
        HELPER_EXIT_NOT_ELEVATED => "it didn't get administrator rights".to_string(),
        HELPER_EXIT_NO_PARENT => "it couldn't reach RABBIT".to_string(),
        HELPER_EXIT_PIPE_FAILED => "its connection to RABBIT broke".to_string(),
        HELPER_EXIT_NO_FOLDERS => {
            "Windows didn't say where Program Files or ProgramData is".to_string()
        }
        other => format!("it ended with code {other}"),
    }
}

/// What RABBIT needs to start the helper for one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevatedRunRequest {
    pub resource_path: PathBuf,
    pub target_app_path: Option<PathBuf>,
    pub target: InstallationKind,
    /// Every package of the run that needs administrator rights, in the
    /// order the run reaches them.
    pub package_ids: Vec<String>,
}

/// Why the helper didn't start.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ElevationStartError {
    /// The user declined the Windows permission prompt. The whole run is
    /// cancelled, and nothing has changed.
    #[error("the Windows permission prompt was declined")]
    Declined,
    /// Something else went wrong; nothing has changed either.
    #[error("RABBIT couldn't start its helper with administrator rights: {0}")]
    Failed(String),
}

/// Starts the elevated helper for a run. The run's one permission prompt
/// appears here.
pub trait ElevatedHelper {
    fn start(
        &self,
        request: &ElevatedRunRequest,
    ) -> Result<Box<dyn ElevatedSession>, ElevationStartError>;
}

/// A running helper.
pub trait ElevatedSession {
    /// Run `package_id`'s elevated steps and wait for them. `on_event`
    /// hears when the vendor's uninstaller starts and when RABBIT is still
    /// waiting for it. A session that has lost its helper answers every
    /// later call with a failure that changed nothing.
    fn run(&mut self, package_id: &str, on_event: &mut dyn FnMut(&VendorEvent)) -> VendorOutcome;

    /// Tell the helper to end, and wait a moment for it to.
    fn finish(self: Box<Self>);
}

/// How long to wait for word on each package: its uninstaller's time
/// limit, then as long again for Inno's after-run checks, and a margin.
fn package_waits(specs: &[PackageSpec], request: &ElevatedRunRequest) -> HashMap<String, Duration> {
    request
        .package_ids
        .iter()
        .filter_map(|id| {
            let plan = elevated_vendor_plan(specs, id, request.target)?;
            let seconds = plan.timeout_seconds.saturating_mul(2).saturating_add(120);
            Some((id.clone(), Duration::from_secs(seconds)))
        })
        .collect()
}

/// The helper's process, to wait for at the end.
enum HelperHandle {
    Elevated(ElevatedChild),
    /// [`InProcessElevatedHelper`]'s thread, never waited for: it ends by
    /// itself once the channel closes.
    Detached,
}

/// RABBIT's end of a session, over any [`LineChannel`].
struct ChannelSession<C: LineChannel> {
    channel: C,
    helper: HelperHandle,
    waits: HashMap<String, Duration>,
    /// Set once the session can't be trusted any more.
    lost: Option<String>,
}

impl<C: LineChannel> ChannelSession<C> {
    fn new(channel: C, helper: HelperHandle, waits: HashMap<String, Duration>) -> Self {
        Self {
            channel,
            helper,
            waits,
            lost: None,
        }
    }

    /// Wait for the helper's first word and check it is the process RABBIT
    /// started.
    fn await_ready(&mut self, expected_pid: u32) -> Result<(), ElevationStartError> {
        let line = self
            .channel
            .read_line(CONNECT_TIMEOUT)
            .map_err(|error| {
                ElevationStartError::Failed(format!("the helper didn't answer: {error}"))
            })?
            .ok_or_else(|| ElevationStartError::Failed("the helper ended at once".to_string()))?;
        match serde_json::from_str::<HelperEvent>(&line) {
            Ok(HelperEvent::Ready { helper_pid }) if helper_pid == expected_pid => Ok(()),
            Ok(HelperEvent::Ready { helper_pid }) => Err(ElevationStartError::Failed(format!(
                "the helper says it is process {helper_pid}, but RABBIT started {expected_pid}"
            ))),
            _ => Err(ElevationStartError::Failed(format!(
                "the helper's first message wasn't the one expected: {line}"
            ))),
        }
    }

    /// Mark the session lost and say so for `package_id`.
    fn lose(&mut self, package_id: &str, launched: bool, why: String) -> VendorOutcome {
        let error = format!(
            "RABBIT lost contact with its helper ({why}), so it can't tell how {package_id} \
             ended"
        );
        self.lost = Some(why);
        VendorOutcome::failed(package_id, launched, error)
    }
}

impl<C: LineChannel + 'static> ElevatedSession for ChannelSession<C> {
    fn run(&mut self, package_id: &str, on_event: &mut dyn FnMut(&VendorEvent)) -> VendorOutcome {
        if let Some(why) = &self.lost {
            return VendorOutcome::failed(
                package_id,
                false,
                format!("RABBIT had already lost contact with its helper ({why})"),
            );
        }
        let order = HelperOrder::Run {
            package_id: package_id.to_string(),
        };
        if let Err(error) = send(&mut self.channel, &order) {
            return self.lose(package_id, false, error.to_string());
        }
        let wait = self
            .waits
            .get(package_id)
            .copied()
            .unwrap_or(UNKNOWN_PACKAGE_WAIT);
        let mut launched = false;
        loop {
            let line = match self.channel.read_line(wait) {
                Ok(Some(line)) => line,
                Ok(None) => return self.lose(package_id, launched, "it ended".to_string()),
                Err(error) => return self.lose(package_id, launched, error.to_string()),
            };
            let event = match serde_json::from_str::<HelperEvent>(&line) {
                Ok(event) => event,
                Err(error) => return self.lose(package_id, launched, error.to_string()),
            };
            match event {
                HelperEvent::Started { package_id: id } if id == package_id => {}
                HelperEvent::VendorLaunched { package_id: id } if id == package_id => {
                    launched = true;
                    on_event(&VendorEvent::Launched);
                }
                HelperEvent::StillWaiting {
                    package_id: id,
                    seconds,
                } if id == package_id => on_event(&VendorEvent::StillWaiting { seconds }),
                HelperEvent::WaitingForFiles {
                    package_id: id,
                    seconds,
                } if id == package_id => on_event(&VendorEvent::WaitingForFiles { seconds }),
                HelperEvent::Finished {
                    package_id: id,
                    outcome,
                } if id == package_id && outcome.package_id == package_id => return *outcome,
                HelperEvent::Refused { reason } => {
                    self.lost = Some(reason.to_string());
                    return VendorOutcome::failed(
                        package_id,
                        false,
                        format!("RABBIT's helper refused to run {package_id}: {reason}"),
                    );
                }
                other => {
                    return self.lose(package_id, launched, format!("it sent {other:?}"));
                }
            }
        }
    }

    fn finish(self: Box<Self>) {
        let mut session = *self;
        if session.lost.is_none() {
            let _ = send(&mut session.channel, &HelperOrder::Quit {});
        }
        let ChannelSession {
            channel, helper, ..
        } = session;
        drop(channel);
        if let HelperHandle::Elevated(child) = helper {
            let _ = child.wait_timeout(FINISH_WAIT);
        }
    }
}

/// A helper that never starts, for runs that may not ask for administrator
/// rights: listings, and tests that remove files only. A run that needs the
/// helper then fails before anything changes.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoElevatedHelper;

impl ElevatedHelper for NoElevatedHelper {
    fn start(
        &self,
        _request: &ElevatedRunRequest,
    ) -> Result<Box<dyn ElevatedSession>, ElevationStartError> {
        Err(ElevationStartError::Failed(
            "this run may not ask for administrator rights".to_string(),
        ))
    }
}

/// The real helper: RABBIT itself, started with administrator rights.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemElevatedHelper;

impl ElevatedHelper for SystemElevatedHelper {
    /// Raises the run's one permission prompt. Refuses, without a prompt,
    /// in tests and in any program not named like a RABBIT build.
    fn start(
        &self,
        request: &ElevatedRunRequest,
    ) -> Result<Box<dyn ElevatedSession>, ElevationStartError> {
        if cfg!(test) {
            return Err(ElevationStartError::Failed(
                "tests never start the elevated helper".to_string(),
            ));
        }
        if !rabbit_platform::running_as_rabbit() {
            return Err(ElevationStartError::Failed(
                "only RABBIT itself starts its helper, and this program isn't named like RABBIT"
                    .to_string(),
            ));
        }
        if !cfg!(windows) {
            return Err(ElevationStartError::Failed(
                "only Windows has the elevated helper".to_string(),
            ));
        }
        start_system_helper(request)
    }
}

fn start_system_helper(
    request: &ElevatedRunRequest,
) -> Result<Box<dyn ElevatedSession>, ElevationStartError> {
    let failed = |what: &str, error: &dyn std::fmt::Display| {
        ElevationStartError::Failed(format!("{what}: {error}"))
    };
    let pipe = new_uninstall_pipe_name().map_err(|error| failed("no pipe name", &error))?;
    let server = PipeServer::create(&pipe).map_err(|error| failed("no pipe", &error))?;
    let program = std::env::current_exe().map_err(|error| failed("RABBIT's own path", &error))?;
    let args = HelperArgs {
        pipe,
        parent_pid: std::process::id(),
        resource_path: absolute(&request.resource_path),
        target_app_path: request.target_app_path.as_deref().map(absolute),
        target: request.target,
        package_ids: request.package_ids.clone(),
    };
    args.validate().map_err(ElevationStartError::Failed)?;
    let mut arguments = vec![HELPER_SUBCOMMAND.to_string()];
    arguments.extend(args.to_arguments().map_err(ElevationStartError::Failed)?);
    let child =
        spawn_elevated(&program, &arguments, None, ElevatedWindow::Hidden).map_err(|error| {
            match error {
                ElevationError::UserCancelledElevation { .. } => ElevationStartError::Declined,
                other => failed("the helper didn't start", &other),
            }
        })?;
    let helper_pid = child.pid();
    let channel = match server.accept(helper_pid, CONNECT_TIMEOUT) {
        Ok(channel) => channel,
        Err(error) => {
            let ended = match child.try_wait() {
                Ok(Some(code)) => format!(", and {}", helper_exit_message(code)),
                _ => String::new(),
            };
            return Err(ElevationStartError::Failed(format!(
                "the helper didn't connect: {error}{ended}"
            )));
        }
    };
    let specs = builtin_package_specs(Platform::Windows);
    let mut session = ChannelSession::new(
        channel,
        HelperHandle::Elevated(child),
        package_waits(&specs, request),
    );
    session.await_ready(helper_pid)?;
    Ok(Box::new(session))
}

/// A helper for tests: the same helper loop on a thread of this process,
/// with this process's rights, against the given environment. It refuses an
/// environment that reads Windows' real list of installed programs, so it
/// can never run a vendor's real uninstaller.
pub struct InProcessElevatedHelper {
    env: Arc<dyn VendorEnvironment>,
    specs: Vec<PackageSpec>,
    folders: MachineFolders,
    declines: bool,
}

impl InProcessElevatedHelper {
    pub fn new(
        env: Arc<dyn VendorEnvironment>,
        specs: Vec<PackageSpec>,
        folders: MachineFolders,
    ) -> Self {
        Self {
            env,
            specs,
            folders,
            declines: false,
        }
    }

    /// One that acts as if the user declined the permission prompt.
    pub fn declining(mut self) -> Self {
        self.declines = true;
        self
    }
}

impl ElevatedHelper for InProcessElevatedHelper {
    fn start(
        &self,
        request: &ElevatedRunRequest,
    ) -> Result<Box<dyn ElevatedSession>, ElevationStartError> {
        if self.env.uninstall_views().is_local_machine() {
            return Err(ElevationStartError::Failed(
                "an in-process helper never works from Windows' real list of installed programs"
                    .to_string(),
            ));
        }
        if self.declines {
            return Err(ElevationStartError::Declined);
        }
        let args = HelperArgs {
            pipe: String::new(),
            parent_pid: std::process::id(),
            resource_path: request.resource_path.clone(),
            target_app_path: request.target_app_path.clone(),
            target: request.target,
            package_ids: request.package_ids.clone(),
        };
        let roots = helper_roots(&args, &self.folders);
        let (parent_end, mut helper_end) = MemoryChannel::pair();
        let env = Arc::clone(&self.env);
        let specs = self.specs.clone();
        std::thread::Builder::new()
            .name("rabbit-uninstall-helper".to_string())
            .spawn(move || serve(&mut helper_end, &args, &specs, env.as_ref(), &roots))
            .map_err(|error| ElevationStartError::Failed(error.to_string()))?;
        let mut session = ChannelSession::new(
            parent_end,
            HelperHandle::Detached,
            package_waits(&self.specs, request),
        );
        session.await_ready(std::process::id())?;
        Ok(Box::new(session))
    }
}
