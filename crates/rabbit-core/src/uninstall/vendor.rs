//! Running a vendor's own uninstaller: Surge XT's Inno Setup uninstaller and
//! REAPER's NSIS one, on Windows, with administrator rights.
//!
//! RABBIT's elevated helper runs this, never the normal RABBIT: it starts a
//! program with administrator rights, so everything it trusts comes from
//! places only administrators can change. Each run goes through these
//! steps, and any check that fails stops it before anything changed:
//!
//! 1. **Locate.** The first `locate` entry Windows' list of programs
//!    installed for every user (HKLM) holds wins. Its value names the
//!    uninstaller; quotes and arguments are stripped, and only a full path
//!    on a drive letter counts. For an NSIS uninstaller, the entry's install
//!    folder must be the REAPER the user chose, the uninstaller must sit
//!    directly in it, and it must sit strictly below Program Files. Every
//!    registry view is read, and the first view that passes wins.
//! 2. **Trust.** An exact vendor folder (`must_be_in.dir`) must be exactly
//!    that folder, reached through no link, and hold nothing but regular
//!    files matching `only_files` that an administrator, SYSTEM or
//!    TrustedInstaller owns: standard users may create files in
//!    `C:\ProgramData\Surge XT\uninstall`, and a file they plant there is
//!    theirs. A folder root (`must_be_in.under`) must hold the uninstaller's
//!    folder strictly below it, never the root itself.
//!
//!    Neither REAPER's `Uninstall.exe` nor Surge's `unins000.exe` is signed,
//!    so the Authenticode check of decision 20 ("where the vendor signs")
//!    has nothing to check for either; the owner check stands in for it.
//! 3. **Run.** NSIS: RABBIT copies the uninstaller into a new folder only
//!    administrators can write to, checks that the copy and the original
//!    hash the same while both are held open against changes, and starts the
//!    copy with `_?=<install folder>` last and unquoted, so it does the work
//!    itself instead of handing it to a copy in a temporary folder and
//!    exiting at once. Inno: the folder checks run again right before the
//!    uninstaller starts in place. Either way RABBIT waits up to
//!    `timeout_seconds`, says it is still waiting after
//!    `still_waiting_seconds`, and leaves an uninstaller that is still
//!    running alone.
//! 4. **Check.** What `verify_gone` names and the registry entry must be
//!    gone. Inno documents that some of its work may still run after its
//!    exit code returns, so for Inno RABBIT checks every few seconds until
//!    `timeout_seconds` has passed. Files Windows removes at the next
//!    restart count as removed, pending a restart.
//! 5. **Leftovers.** Only once everything is gone does RABBIT remove
//!    `remove_leftovers`, in order, each through the guard again: junctions
//!    as links, never followed, and folders only when empty. What
//!    `report_leftovers` names is only reported.

use std::cell::RefCell;
use std::fs;
use std::io::{self, Seek};
use std::path::{Component, Path, PathBuf, Prefix};
use std::time::{Duration, Instant};

use rabbit_platform::{
    LockedFile, NsisInPlaceArgument, PendingFileRename, RegistryView, TimedCommand,
    TimedRunOutcome, UninstallViews, ViewValues,
};
use serde::{Deserialize, Serialize};

use crate::hash::sha256_reader;
use crate::package::uninstall::{
    InstallLocationCheck, LeftoverRemoval, RegistryHive, UninstallPath, UninstallerFolder,
    UninstallerLocation, VendorUninstallerKind, VendorUninstallerSpec,
    wildcard_matches_ignoring_case,
};

use super::paths::{
    PathRefusal, ResolvedPath, UninstallRoots, check_link_removal, check_removal, inside_or_same,
    resolve_path, same_folder, same_ignoring_case, strictly_inside,
};
use super::registry::program_named_by;
use super::{UninstallStatus, resource_relative};

#[cfg(test)]
#[path = "vendor_tests.rs"]
pub(crate) mod tests;

/// How long between two checks that an Inno uninstaller's work is done.
pub const VERIFY_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// The start of the name of the folder an NSIS uninstaller's copy runs
/// from, under ProgramData.
pub const WORKING_FOLDER_PREFIX: &str = "RABBIT-uninstall";

/// What the vendor runner reads and acts on. The elevated helper's
/// environment reads HKLM and the real file system and starts real
/// programs; tests give a throwaway registry key, fixture folders and
/// fixture programs instead.
pub trait VendorEnvironment: Send + Sync {
    /// The list of installed programs the uninstaller is looked up in:
    /// HKLM's, the one only administrators can change, in the helper.
    fn uninstall_views(&self) -> &UninstallViews;

    /// Whether `path` is owned by the Administrators group, SYSTEM or
    /// TrustedInstaller. Never follows a link.
    fn owner_is_trusted(&self, path: &Path) -> io::Result<bool>;

    /// A new, empty folder for an NSIS uninstaller's copy, which only
    /// administrators and SYSTEM can write to.
    fn create_working_folder(&self) -> io::Result<PathBuf>;

    /// Copy the uninstaller `from` to `to`, which doesn't exist yet.
    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Start `command` and wait for it within `limits`. `on_started` runs
    /// once the program has started, `on_still_waiting` once it has run for
    /// `limits.still_waiting_after`.
    fn run_program(
        &self,
        command: &VendorCommand,
        limits: RunLimits,
        on_started: &mut dyn FnMut(),
        on_still_waiting: &mut dyn FnMut(),
    ) -> io::Result<ProgramEnd>;

    /// The file operations Windows carries out at the next restart.
    fn pending_file_renames(&self) -> io::Result<Vec<PendingFileRename>>;

    fn now(&self) -> Instant;

    fn sleep(&self, duration: Duration);
}

/// A vendor uninstaller to start, with its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorCommand {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    /// NSIS's `_?=` argument, which always comes last, unquoted.
    pub nsis_in_place: Option<NsisInPlaceArgument>,
    pub working_directory: Option<PathBuf>,
}

/// How long to wait for a vendor uninstaller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunLimits {
    pub timeout: Duration,
    pub still_waiting_after: Duration,
}

/// How waiting for a vendor uninstaller ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEnd {
    Exited(i32),
    /// Still running at the time limit. It keeps running: RABBIT never ends
    /// an uninstaller half way.
    StillRunning,
}

/// What the elevated helper's runner works on and with: Windows' real list
/// of installed programs, the real owners of files, a folder under
/// ProgramData only administrators can use, and real programs.
#[derive(Debug)]
pub(super) struct SystemVendorEnvironment {
    views: UninstallViews,
}

impl SystemVendorEnvironment {
    /// Only the elevated helper's entry point makes one; see
    /// [`super::helper::run_elevated_helper`].
    pub(super) fn new() -> Self {
        Self {
            views: UninstallViews::local_machine(),
        }
    }
}

impl VendorEnvironment for SystemVendorEnvironment {
    fn uninstall_views(&self) -> &UninstallViews {
        &self.views
    }

    fn owner_is_trusted(&self, path: &Path) -> io::Result<bool> {
        rabbit_platform::file_owner_is_trusted(path)
    }

    fn create_working_folder(&self) -> io::Result<PathBuf> {
        // ProgramData as Windows names it, never an environment variable
        // and never a per-user temporary folder a standard user could
        // write to.
        let program_data = rabbit_platform::program_data_dir()?;
        rabbit_platform::create_admin_only_directory(&program_data, WORKING_FOLDER_PREFIX)
    }

    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::copy(from, to).map(|_| ())
    }

    fn run_program(
        &self,
        command: &VendorCommand,
        limits: RunLimits,
        on_started: &mut dyn FnMut(),
        on_still_waiting: &mut dyn FnMut(),
    ) -> io::Result<ProgramEnd> {
        run_timed_command(command, &[], limits, on_started, on_still_waiting)
    }

    fn pending_file_renames(&self) -> io::Result<Vec<PendingFileRename>> {
        rabbit_platform::pending_file_renames()
    }

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Looks a vendor's uninstaller up before a run, the way the elevated
/// helper does again right before it starts it, and changes nothing. RABBIT
/// shows the answer before anything happens: a package whose uninstaller it
/// can't trust can't be ticked, and no permission prompt is raised for it.
pub trait VendorLookup {
    /// The uninstaller `job` names, once every check passes, or why not.
    fn locate(&self, job: &VendorJob<'_>) -> Result<PathBuf, VendorRefusal>;
}

/// The real lookup: Windows' list of programs installed for every user
/// (HKLM) and the real folders, read only. Only RABBIT's own entry points
/// use it; it never starts anything.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemVendorLookup;

impl VendorLookup for SystemVendorLookup {
    fn locate(&self, job: &VendorJob<'_>) -> Result<PathBuf, VendorRefusal> {
        if !cfg!(windows) {
            return Err(VendorRefusal::Unsupported);
        }
        let env = LookupOnly {
            views: UninstallViews::local_machine(),
        };
        locate_uninstaller(job, &env).map(|located| located.program)
    }
}

/// A lookup that finds nothing, as on a computer where Windows lists no
/// program for every user. For listings and tests that never run a vendor's
/// uninstaller.
#[derive(Debug, Clone, Copy, Default)]
pub struct NothingListed;

impl VendorLookup for NothingListed {
    fn locate(&self, _job: &VendorJob<'_>) -> Result<PathBuf, VendorRefusal> {
        Err(VendorRefusal::NotListed)
    }
}

/// What [`SystemVendorLookup`] reads through: a list of installed programs
/// and the owners of files. Everything that would change something refuses.
struct LookupOnly {
    views: UninstallViews,
}

fn read_only() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "looking an uninstaller up changes nothing",
    )
}

impl VendorEnvironment for LookupOnly {
    fn uninstall_views(&self) -> &UninstallViews {
        &self.views
    }

    fn owner_is_trusted(&self, path: &Path) -> io::Result<bool> {
        rabbit_platform::file_owner_is_trusted(path)
    }

    fn create_working_folder(&self) -> io::Result<PathBuf> {
        Err(read_only())
    }

    fn copy_file(&self, _from: &Path, _to: &Path) -> io::Result<()> {
        Err(read_only())
    }

    fn run_program(
        &self,
        _command: &VendorCommand,
        _limits: RunLimits,
        _on_started: &mut dyn FnMut(),
        _on_still_waiting: &mut dyn FnMut(),
    ) -> io::Result<ProgramEnd> {
        Err(read_only())
    }

    fn pending_file_renames(&self) -> io::Result<Vec<PendingFileRename>> {
        Ok(Vec::new())
    }

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, _duration: Duration) {}
}

/// What running a vendor's uninstaller would do, as the plan sees it before
/// the run. The elevated helper finds and checks everything again right
/// before it starts the uninstaller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedVendorRun {
    pub kind: VendorUninstallerKind,
    /// The uninstaller RABBIT found and checked.
    pub uninstaller: PathBuf,
    pub arguments: Vec<String>,
    pub timeout_seconds: u64,
    pub still_waiting_seconds: u64,
    /// What must be gone afterwards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verify_gone: Vec<PathBuf>,
    /// What RABBIT removes after the uninstaller, in order: links as links,
    /// folders only when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_leftovers: Vec<PathBuf>,
    /// What RABBIT reports if it is still there afterwards, and never
    /// removes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub report_leftovers: Vec<PathBuf>,
}

/// Look `job`'s uninstaller up through `lookup` and say what running it
/// would do. Every path the run needs afterwards must resolve too, as it
/// must in the helper.
pub fn plan_vendor_run(
    job: &VendorJob<'_>,
    lookup: &dyn VendorLookup,
) -> Result<PlannedVendorRun, VendorRefusal> {
    let paths = after_run_paths(job)?;
    let uninstaller = lookup.locate(job)?;
    // A path the plan names twice, as a link and then as an empty folder in
    // case it isn't one, is listed once.
    let mut remove_leftovers: Vec<PathBuf> = Vec::new();
    for (_, resolved) in paths.remove_leftovers {
        if !remove_leftovers.contains(&resolved.path) {
            remove_leftovers.push(resolved.path);
        }
    }
    Ok(PlannedVendorRun {
        kind: job.spec.kind,
        uninstaller,
        arguments: job.spec.arguments.clone(),
        timeout_seconds: job.spec.timeout_seconds,
        still_waiting_seconds: job.spec.still_waiting_seconds,
        verify_gone: paths.verify_gone,
        remove_leftovers,
        report_leftovers: paths
            .report_leftovers
            .into_iter()
            .map(|resolved| resolved.path)
            .collect(),
    })
}

/// Start `command` as an ordinary child process, with the rights this
/// process has and `environment` added to its environment, and wait for it
/// within `limits`.
pub(crate) fn run_timed_command(
    command: &VendorCommand,
    environment: &[(&str, &std::ffi::OsStr)],
    limits: RunLimits,
    on_started: &mut dyn FnMut(),
    on_still_waiting: &mut dyn FnMut(),
) -> io::Result<ProgramEnd> {
    let mut timed = TimedCommand::new(&command.program);
    timed.args(&command.arguments);
    if let Some(folder) = &command.working_directory {
        timed.current_dir(folder);
    }
    if let Some(argument) = &command.nsis_in_place {
        timed.nsis_in_place(argument.clone());
    }
    for (key, value) in environment {
        timed.env(key, value);
    }
    let running = timed.start()?;
    on_started();
    match running.wait(limits.timeout, limits.still_waiting_after, on_still_waiting)? {
        TimedRunOutcome::Exited(code) => Ok(ProgramEnd::Exited(code)),
        // Dropping the child closes RABBIT's handle and leaves it running.
        TimedRunOutcome::TimedOut(_child) => Ok(ProgramEnd::StillRunning),
    }
}

/// One vendor run: the package and its plan's uninstaller, and what the
/// plan's path tokens stand for.
#[derive(Debug, Clone, Copy)]
pub struct VendorJob<'a> {
    pub package_id: &'a str,
    pub spec: &'a VendorUninstallerSpec,
    /// In the elevated helper: the machine-wide folders it found itself,
    /// and the resource folder and REAPER's app from its command line.
    pub roots: &'a UninstallRoots,
}

/// Something worth telling the user while a vendor uninstaller runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum VendorEvent {
    /// The uninstaller has started.
    Launched,
    /// It has run for `seconds` and hasn't ended. It may be showing a
    /// window that waits for an answer.
    StillWaiting { seconds: u64 },
}

/// Why RABBIT didn't start a vendor's uninstaller. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum VendorRefusal {
    #[error("Windows' list of programs installed for every user doesn't list it")]
    NotListed,
    #[error("RABBIT can't tell which program Windows' list names as its uninstaller: {value}")]
    UnclearCommand { value: String },
    #[error("Windows' list names its uninstaller by a path RABBIT doesn't trust: {}", path.display())]
    NotAFullPath { path: PathBuf },
    #[error("RABBIT can't find the folder {token} stands for")]
    UnknownFolder { token: String },
    #[error("its uninstaller isn't in the folder it belongs in: {}", path.display())]
    WrongFolder { path: PathBuf },
    #[error("its uninstaller's folder holds a file that doesn't belong there: {}", path.display())]
    ForeignFile { path: PathBuf },
    #[error("its uninstaller's folder holds a folder or a link: {}", path.display())]
    FolderOrLink { path: PathBuf },
    #[error("an administrator didn't put this file next to its uninstaller: {}", path.display())]
    UntrustedOwner { path: PathBuf },
    #[error("Windows' list doesn't say which folder it is installed in")]
    InstallLocationMissing,
    #[error("Windows' list places it in {found}, not in the REAPER that was chosen ({})", expected.display())]
    InstallLocationMismatch { found: String, expected: PathBuf },
    #[error("its uninstaller isn't directly in its program folder {}", folder.display())]
    NotInInstallFolder { path: PathBuf, folder: PathBuf },
    #[error("it isn't installed in a folder of its own under Program Files: {}", folder.display())]
    NotBelowProgramFiles { folder: PathBuf },
    #[error("the copy RABBIT made of its uninstaller differs from the original")]
    CopyDiffers,
    #[error("RABBIT couldn't check {what}: {error}")]
    Unreadable { what: String, error: String },
    #[error("its plan has no uninstaller RABBIT runs with administrator rights")]
    NotAVendorPlan,
    #[error("only Windows runs a vendor's uninstaller")]
    Unsupported,
}

impl VendorRefusal {
    /// The Fluent key that says this in the user's language. Its arguments
    /// are [`VendorRefusal::fluent_arguments`].
    pub fn fluent_key(&self) -> &'static str {
        match self {
            Self::NotListed => "uninstall-vendor-refused-not-listed",
            Self::UnclearCommand { .. } => "uninstall-vendor-refused-unclear-command",
            Self::NotAFullPath { .. } => "uninstall-vendor-refused-not-a-full-path",
            Self::UnknownFolder { .. } => "uninstall-vendor-refused-unknown-folder",
            Self::WrongFolder { .. } => "uninstall-vendor-refused-wrong-folder",
            Self::ForeignFile { .. } => "uninstall-vendor-refused-foreign-file",
            Self::FolderOrLink { .. } => "uninstall-vendor-refused-folder-or-link",
            Self::UntrustedOwner { .. } => "uninstall-vendor-refused-untrusted-owner",
            Self::InstallLocationMissing => "uninstall-vendor-refused-install-location-missing",
            Self::InstallLocationMismatch { .. } => {
                "uninstall-vendor-refused-install-location-mismatch"
            }
            Self::NotInInstallFolder { .. } => "uninstall-vendor-refused-not-in-install-folder",
            Self::NotBelowProgramFiles { .. } => "uninstall-vendor-refused-not-below-program-files",
            Self::CopyDiffers => "uninstall-vendor-refused-copy-differs",
            Self::Unreadable { .. } => "uninstall-vendor-refused-unreadable",
            Self::NotAVendorPlan => "uninstall-vendor-refused-not-a-vendor-plan",
            Self::Unsupported => "uninstall-vendor-refused-unsupported",
        }
    }

    /// The arguments [`VendorRefusal::fluent_key`]'s message takes:
    /// `package`, the package's name, and for most also `path`.
    pub fn fluent_arguments(&self, package: &str) -> Vec<(&'static str, String)> {
        let mut arguments = vec![("package", package.to_string())];
        let path = match self {
            Self::UnclearCommand { value } => Some(value.clone()),
            Self::NotAFullPath { path }
            | Self::WrongFolder { path }
            | Self::ForeignFile { path }
            | Self::FolderOrLink { path }
            | Self::UntrustedOwner { path }
            | Self::NotInInstallFolder { path, .. } => Some(path.display().to_string()),
            Self::InstallLocationMismatch { found, .. } => Some(found.clone()),
            Self::NotBelowProgramFiles { folder } => Some(folder.display().to_string()),
            Self::Unreadable { what, .. } => Some(what.clone()),
            Self::UnknownFolder { token } => Some(token.clone()),
            Self::NotListed
            | Self::InstallLocationMissing
            | Self::CopyDiffers
            | Self::NotAVendorPlan
            | Self::Unsupported => None,
        };
        arguments.extend(path.map(|path| ("path", path)));
        arguments
    }

    fn unreadable(what: impl std::fmt::Display, error: &io::Error) -> Self {
        Self::Unreadable {
            what: what.to_string(),
            error: error.to_string(),
        }
    }
}

/// How a vendor run ended, as far as RABBIT can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VendorStatus {
    /// The uninstaller succeeded, and everything it was to remove is gone.
    Removed,
    /// Removed, but something RABBIT reports or meant to remove is still
    /// there.
    RemovedWithLeftovers,
    /// Removed; Windows deletes the rest at the next restart.
    RemovedPendingRestart,
    /// The user cancelled the uninstaller.
    Cancelled,
    /// The uninstaller failed or was aborted, couldn't be started, or the
    /// package is still there afterwards.
    Failed,
    /// Still running at `timeout_seconds`. RABBIT stopped waiting and left
    /// it running; what it did is unknown.
    StillRunning,
    /// RABBIT didn't start it: a check failed. Nothing changed.
    Refused,
}

/// What became of one vendor run. It crosses the pipe from the elevated
/// helper to RABBIT, so it holds plain data only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VendorOutcome {
    pub package_id: String,
    pub status: VendorStatus,
    /// Why RABBIT didn't start the uninstaller, when it didn't.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<VendorRefusal>,
    /// The uninstaller RABBIT found and checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uninstaller: Option<PathBuf>,
    /// The uninstaller started. Once it has, its work can't be undone.
    pub launched: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Starting it, waiting for it or talking to the elevated helper went
    /// wrong, in English.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// What `verify_gone` names that is still there afterwards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub still_present: Vec<PathBuf>,
    /// Windows still lists the program afterwards.
    pub still_listed: bool,
    /// Files Windows removes at the next restart that belong to the
    /// package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_restart: Vec<PathBuf>,
    /// What `report_leftovers` names that is still there. Never removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leftovers: Vec<PathBuf>,
    /// What became of each of `remove_leftovers`, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_leftovers: Vec<LeftoverOutcome>,
    /// Things after the run that didn't work. The status stays what it is.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<VendorProblem>,
}

impl VendorOutcome {
    pub fn new(package_id: &str) -> Self {
        Self {
            package_id: package_id.to_string(),
            status: VendorStatus::Failed,
            refusal: None,
            uninstaller: None,
            launched: false,
            exit_code: None,
            error: None,
            still_present: Vec::new(),
            still_listed: false,
            pending_restart: Vec::new(),
            leftovers: Vec::new(),
            removed_leftovers: Vec::new(),
            problems: Vec::new(),
        }
    }

    /// Nothing ran, because `refusal`.
    pub fn refused(package_id: &str, refusal: VendorRefusal) -> Self {
        Self {
            status: VendorStatus::Refused,
            refusal: Some(refusal),
            ..Self::new(package_id)
        }
    }

    /// Something went wrong with the run itself; `launched` says whether the
    /// uninstaller had started.
    pub fn failed(package_id: &str, launched: bool, error: impl Into<String>) -> Self {
        Self {
            launched,
            error: Some(error.into()),
            ..Self::new(package_id)
        }
    }

    /// The package's status in an uninstall report. A refusal blocks the
    /// package, since nothing of it changed; an uninstaller still running
    /// at its time limit fails it.
    pub fn uninstall_status(&self) -> UninstallStatus {
        match self.status {
            VendorStatus::Removed => UninstallStatus::Removed,
            VendorStatus::RemovedWithLeftovers => UninstallStatus::RemovedWithLeftovers,
            VendorStatus::RemovedPendingRestart => UninstallStatus::RemovedPendingRestart,
            VendorStatus::Cancelled => UninstallStatus::Cancelled,
            VendorStatus::Failed | VendorStatus::StillRunning => UninstallStatus::Failed,
            VendorStatus::Refused => UninstallStatus::Blocked,
        }
    }
}

/// What became of one of `remove_leftovers`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeftoverOutcome {
    pub path: PathBuf,
    pub kind: LeftoverKind,
    pub result: LeftoverResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LeftoverKind {
    Junction,
    EmptyDir,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum LeftoverResult {
    Removed,
    /// It wasn't there.
    NotFound,
    /// A folder that still holds something, such as another program from
    /// the same vendor. It stays, as it should.
    NotEmpty,
    /// The guard refused it, or it isn't the link it should be. It stays.
    Refused {
        refusal: PathRefusal,
    },
    /// Removing it failed. It stays.
    Failed {
        error: String,
    },
    /// Not tried: the uninstaller hadn't finished removing the package.
    Skipped,
}

impl LeftoverResult {
    /// Whether the thing is still there and shouldn't be.
    pub fn is_left_over(&self) -> bool {
        matches!(
            self,
            Self::Refused { .. } | Self::Failed { .. } | Self::Skipped
        )
    }
}

impl LeftoverOutcome {
    /// It should have gone, and it is still there now.
    pub fn still_there(&self) -> bool {
        self.result.is_left_over() && fs::symlink_metadata(&self.path).is_ok()
    }
}

/// Something after a vendor run that didn't work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum VendorProblem {
    /// The folder the NSIS uninstaller's copy ran from couldn't be removed.
    /// It is under ProgramData and only administrators can use it.
    WorkingFolderNotRemoved { path: PathBuf, error: String },
    /// Windows' list of renames at the next restart couldn't be read.
    PendingRenamesUnreadable { error: String },
    /// Windows' list of installed programs couldn't be read afterwards.
    ListUnreadable { error: String },
}

/// How a vendor uninstaller's exit code reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorExit {
    Success,
    Cancelled,
    Failed,
}

/// What an exit code means for `kind`.
///
/// NSIS (Appendix D of its documentation): 0 is success, 1 means the user
/// cancelled, 2 that the script aborted. Anything else is a failure too.
///
/// Inno Setup documents only that its uninstaller returns a non-zero code
/// when the user cancels or a fatal error happens, so that any non-zero
/// code means it didn't run to completion, and advises against checking
/// for particular values ("Uninstaller Exit Codes" in its help). So 0 is
/// success and anything else a failure; a cancel can't be told apart.
pub fn vendor_exit(kind: VendorUninstallerKind, code: i32) -> VendorExit {
    match (kind, code) {
        (_, 0) => VendorExit::Success,
        (VendorUninstallerKind::Nsis, 1) => VendorExit::Cancelled,
        _ => VendorExit::Failed,
    }
}

/// Where the uninstaller is, as Windows' list says and the checks agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedUninstaller {
    pub program: PathBuf,
    /// The entry's key under the Uninstall key, and the registry view it was
    /// found in.
    pub key: String,
    pub view: RegistryView,
    /// For NSIS: the install folder `_?=` names.
    pub install_folder: Option<PathBuf>,
}

/// Find the uninstaller `job` names and check it may run. Changes nothing.
pub fn locate_uninstaller(
    job: &VendorJob<'_>,
    env: &dyn VendorEnvironment,
) -> Result<LocatedUninstaller, VendorRefusal> {
    let views = env.uninstall_views();
    for location in &job.spec.locate {
        // A path, or a value in the user's own list, is something a
        // standard user can change, so it never names a program RABBIT runs
        // with administrator rights.
        let UninstallerLocation::Registry(entry) = location else {
            continue;
        };
        if entry.hive != RegistryHive::Hklm {
            continue;
        }
        let install = job.spec.install_location.as_ref();
        let mut names = vec![entry.value.as_str()];
        let install_in_same_key =
            install.is_some_and(|check| check.registry.key.eq_ignore_ascii_case(&entry.key));
        if let Some(check) = install.filter(|_| install_in_same_key) {
            names.push(check.registry.value.as_str());
        }
        let found = views
            .read_values(&entry.key, &names)
            .map_err(|error| VendorRefusal::unreadable(&entry.key, &error))?;
        let found: Vec<&ViewValues> = found
            .iter()
            .filter(|values| values.value(&entry.value).is_some())
            .collect();
        if found.is_empty() {
            continue;
        }
        // The first entry that exists wins. Each view is a separate
        // install, so the first view whose values pass is the one to run;
        // if none does, the first view's reason is the answer.
        let mut first_refusal = None;
        for values in found {
            let install_folder_value = match install {
                None => None,
                Some(check) if install_in_same_key => {
                    Some(values.value(&check.registry.value).map(str::to_string))
                }
                Some(check) => Some(read_in_view(views, check, values.view())?),
            };
            match check_view(
                job,
                env,
                values.value(&entry.value).unwrap_or_default(),
                install_folder_value,
            ) {
                Ok((program, install_folder)) => {
                    return Ok(LocatedUninstaller {
                        program,
                        key: entry.key.clone(),
                        view: values.view(),
                        install_folder,
                    });
                }
                Err(refusal) => {
                    first_refusal.get_or_insert(refusal);
                }
            }
        }
        return Err(first_refusal.unwrap_or(VendorRefusal::NotListed));
    }
    Err(VendorRefusal::NotListed)
}

/// The install folder value `check` names, in `view`.
fn read_in_view(
    views: &UninstallViews,
    check: &InstallLocationCheck,
    view: RegistryView,
) -> Result<Option<String>, VendorRefusal> {
    let found = views
        .read_values(&check.registry.key, &[check.registry.value.as_str()])
        .map_err(|error| VendorRefusal::unreadable(&check.registry.key, &error))?;
    Ok(found
        .iter()
        .find(|values| values.view() == view)
        .and_then(|values| values.value(&check.registry.value))
        .map(str::to_string))
}

/// Check one view's values: the uninstaller they name, its install folder
/// if the plan has one, and where it sits.
fn check_view(
    job: &VendorJob<'_>,
    env: &dyn VendorEnvironment,
    command: &str,
    install_folder_value: Option<Option<String>>,
) -> Result<(PathBuf, Option<PathBuf>), VendorRefusal> {
    let program = uninstaller_program(command)?;
    let install_folder = match (&job.spec.install_location, install_folder_value) {
        (Some(check), Some(value)) => Some(check_install_location(
            job,
            check,
            value.as_deref(),
            &program,
        )?),
        _ => None,
    };
    check_uninstaller_folder(job, env, &program, install_folder.as_deref())?;
    Ok((program, install_folder))
}

/// The program an `UninstallString` names, without its quotes and
/// arguments. It must be a full path on a drive letter: a relative path
/// could be anywhere, and a network share or a device path is nothing an
/// administrator installed here.
pub fn uninstaller_program(command: &str) -> Result<PathBuf, VendorRefusal> {
    let program = program_named_by(command).ok_or_else(|| VendorRefusal::UnclearCommand {
        value: command.to_string(),
    })?;
    let program = PathBuf::from(program);
    if !is_full_drive_path(&program) || program.file_name().is_none() {
        return Err(VendorRefusal::NotAFullPath { path: program });
    }
    Ok(program)
}

/// Whether `path` is `X:\name...` with nothing but plain names after the
/// drive, and at least one of them: no
/// `\\server\share`, no `\\?\` or `\\.\` device paths, no `.` or `..`.
pub fn is_full_drive_path(path: &Path) -> bool {
    let mut components = path.components();
    let starts_on_a_drive = matches!(
        components.next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_))
    ) && matches!(components.next(), Some(Component::RootDir));
    let rest: Vec<Component<'_>> = components.collect();
    starts_on_a_drive
        && !rest.is_empty()
        && rest
            .iter()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// An install folder as Windows' list holds it, made comparable: no
/// surrounding blanks or quotes, no trailing separator. It must be a full
/// path on a drive letter, like the uninstaller.
fn install_folder_from(value: &str) -> Result<PathBuf, VendorRefusal> {
    let mut text = value.trim();
    if let Some(unquoted) = text
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        text = unquoted.trim();
    }
    let trimmed = text.trim_end_matches(['\\', '/']);
    // Trimmed, `C:\` would read as `C:`, a folder relative to that drive's
    // current folder. Kept as written, it is refused below as a whole
    // drive, which is never an install folder.
    let folder = PathBuf::from(if trimmed.ends_with(':') {
        text
    } else {
        trimmed
    });
    if !is_full_drive_path(&folder) {
        return Err(VendorRefusal::NotAFullPath { path: folder });
    }
    Ok(folder)
}

/// NSIS: the install folder Windows' list records must be the REAPER the
/// user chose (`must_equal`), and the uninstaller must sit directly in it.
/// That it sits below Program Files is checked with the uninstaller's
/// folder.
fn check_install_location(
    job: &VendorJob<'_>,
    check: &InstallLocationCheck,
    value: Option<&str>,
    program: &Path,
) -> Result<PathBuf, VendorRefusal> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Err(VendorRefusal::InstallLocationMissing);
    };
    let folder = install_folder_from(value)?;
    let expected = resolve(&check.must_equal, job.roots)?;
    if !expected
        .iter()
        .any(|expected| same_folder(&folder, &expected.path))
    {
        return Err(VendorRefusal::InstallLocationMismatch {
            found: value.to_string(),
            expected: expected
                .into_iter()
                .next()
                .map(|expected| expected.path)
                .unwrap_or_default(),
        });
    }
    if !program
        .parent()
        .is_some_and(|parent| same_ignoring_case(parent, &folder))
    {
        return Err(VendorRefusal::NotInInstallFolder {
            path: program.to_path_buf(),
            folder,
        });
    }
    Ok(folder)
}

/// `must_be_in`: where the uninstaller, or for NSIS its install folder, may
/// sit, and what may sit beside it.
fn check_uninstaller_folder(
    job: &VendorJob<'_>,
    env: &dyn VendorEnvironment,
    program: &Path,
    install_folder: Option<&Path>,
) -> Result<(), VendorRefusal> {
    let Some(folder) = program.parent() else {
        return Err(VendorRefusal::NotAFullPath {
            path: program.to_path_buf(),
        });
    };
    match &job.spec.must_be_in {
        Some(UninstallerFolder::Exact(exact)) => {
            let expected = resolve(&exact.dir, job.roots)?;
            if !expected
                .iter()
                .any(|expected| same_ignoring_case(folder, &expected.path))
            {
                return Err(VendorRefusal::WrongFolder {
                    path: program.to_path_buf(),
                });
            }
            check_reached_without_links(folder)?;
            check_program_file(env, program)?;
            check_folder_contents(env, folder, &exact.only_files)
        }
        Some(UninstallerFolder::Under(under)) => {
            let roots = resolve(&under.under, job.roots)?;
            // `_?=` makes the install folder the one the uninstaller deletes
            // from, so that is the folder that must sit below the root.
            let checked = install_folder.unwrap_or(folder);
            let real = check_reached_without_links(checked)?;
            let below = roots.iter().any(|root| {
                strictly_inside(checked, &root.path)
                    && fs::canonicalize(&root.path)
                        .is_ok_and(|real_root| strictly_inside(&real, &real_root))
            });
            if !below {
                return Err(match install_folder {
                    Some(folder) => VendorRefusal::NotBelowProgramFiles {
                        folder: folder.to_path_buf(),
                    },
                    None => VendorRefusal::WrongFolder {
                        path: program.to_path_buf(),
                    },
                });
            }
            check_program_file(env, program)
        }
        // Validation requires `must_be_in` for every plan the helper runs;
        // without one there is nothing to trust.
        None => Err(VendorRefusal::NotAVendorPlan),
    }
}

/// `folder` must exist, and its real path must be the path itself, ignoring
/// case: no junction or link anywhere along it, and no folder that only
/// stands in for another. Returns the real path.
fn check_reached_without_links(folder: &Path) -> Result<PathBuf, VendorRefusal> {
    let real = fs::canonicalize(folder)
        .map_err(|error| VendorRefusal::unreadable(folder.display(), &error))?;
    let is_reparse_point = rabbit_platform::is_reparse_point(folder)
        .map_err(|error| VendorRefusal::unreadable(folder.display(), &error))?;
    if is_reparse_point || !same_ignoring_case(&real, folder) {
        return Err(VendorRefusal::WrongFolder {
            path: folder.to_path_buf(),
        });
    }
    Ok(real)
}

/// Every entry in `folder` must be a regular file, not a link or any other
/// reparse point, named like `only_files`, and owned by an administrator.
fn check_folder_contents(
    env: &dyn VendorEnvironment,
    folder: &Path,
    only_files: &str,
) -> Result<(), VendorRefusal> {
    let entries = fs::read_dir(folder)
        .map_err(|error| VendorRefusal::unreadable(folder.display(), &error))?;
    for entry in entries {
        let entry = entry.map_err(|error| VendorRefusal::unreadable(folder.display(), &error))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| VendorRefusal::unreadable(path.display(), &error))?;
        let is_reparse_point = rabbit_platform::is_reparse_point(&path)
            .map_err(|error| VendorRefusal::unreadable(path.display(), &error))?;
        if is_reparse_point || !metadata.file_type().is_file() {
            return Err(VendorRefusal::FolderOrLink { path });
        }
        let named_well = entry
            .file_name()
            .to_str()
            .is_some_and(|name| wildcard_matches_ignoring_case(only_files, name));
        if !named_well {
            return Err(VendorRefusal::ForeignFile { path });
        }
        check_owner(env, &path)?;
    }
    Ok(())
}

/// The uninstaller itself must be a regular file an administrator owns.
fn check_program_file(env: &dyn VendorEnvironment, program: &Path) -> Result<(), VendorRefusal> {
    let metadata = fs::symlink_metadata(program)
        .map_err(|error| VendorRefusal::unreadable(program.display(), &error))?;
    let is_reparse_point = rabbit_platform::is_reparse_point(program)
        .map_err(|error| VendorRefusal::unreadable(program.display(), &error))?;
    if is_reparse_point || !metadata.file_type().is_file() {
        return Err(VendorRefusal::FolderOrLink {
            path: program.to_path_buf(),
        });
    }
    check_owner(env, program)
}

fn check_owner(env: &dyn VendorEnvironment, path: &Path) -> Result<(), VendorRefusal> {
    match env.owner_is_trusted(path) {
        Ok(true) => Ok(()),
        Ok(false) => Err(VendorRefusal::UntrustedOwner {
            path: path.to_path_buf(),
        }),
        Err(error) => Err(VendorRefusal::unreadable(path.display(), &error)),
    }
}

/// `path` resolved for these roots. A token that stands for nothing here
/// means RABBIT can't check what the plan asks for.
fn resolve(
    path: &UninstallPath,
    roots: &UninstallRoots,
) -> Result<Vec<ResolvedPath>, VendorRefusal> {
    match resolve_path(path, roots) {
        Ok(resolved) => Ok(resolved),
        Err(PathRefusal::UnknownRoot { token }) => Err(VendorRefusal::UnknownFolder { token }),
        Err(_) => Err(VendorRefusal::NotAVendorPlan),
    }
}

/// Every path the run needs after the uninstaller ends, resolved before it
/// starts, so a token that stands for nothing stops the run while nothing
/// has changed.
struct AfterRunPaths {
    verify_gone: Vec<PathBuf>,
    report_leftovers: Vec<ResolvedPath>,
    remove_leftovers: Vec<(LeftoverKind, ResolvedPath)>,
}

fn after_run_paths(job: &VendorJob<'_>) -> Result<AfterRunPaths, VendorRefusal> {
    let mut paths = AfterRunPaths {
        verify_gone: Vec::new(),
        report_leftovers: Vec::new(),
        remove_leftovers: Vec::new(),
    };
    for path in &job.spec.verify_gone {
        paths.verify_gone.extend(
            resolve(path, job.roots)?
                .into_iter()
                .map(|resolved| resolved.path),
        );
    }
    for path in &job.spec.report_leftovers {
        paths.report_leftovers.extend(resolve(path, job.roots)?);
    }
    for leftover in &job.spec.remove_leftovers {
        let kind = match leftover {
            LeftoverRemoval::Junction(_) => LeftoverKind::Junction,
            LeftoverRemoval::EmptyDir(_) => LeftoverKind::EmptyDir,
        };
        paths.remove_leftovers.extend(
            resolve(leftover.path(), job.roots)?
                .into_iter()
                .map(|resolved| (kind, resolved)),
        );
    }
    Ok(paths)
}

/// Run the vendor uninstaller `job` names, after every check, and report
/// what became of it. `events` hears when it starts and when RABBIT is
/// still waiting for it.
///
/// A refusal changes nothing. Once the uninstaller has started, it always
/// runs to its end or to `timeout_seconds`, whatever else happens.
pub fn run_vendor_uninstaller(
    job: &VendorJob<'_>,
    env: &dyn VendorEnvironment,
    events: &mut dyn FnMut(VendorEvent),
) -> VendorOutcome {
    if !cfg!(windows) {
        return VendorOutcome::refused(job.package_id, VendorRefusal::Unsupported);
    }
    let paths = match after_run_paths(job) {
        Ok(paths) => paths,
        Err(refusal) => return VendorOutcome::refused(job.package_id, refusal),
    };
    let located = match locate_uninstaller(job, env) {
        Ok(located) => located,
        Err(refusal) => return VendorOutcome::refused(job.package_id, refusal),
    };
    let mut outcome = VendorOutcome::new(job.package_id);
    outcome.uninstaller = Some(located.program.clone());
    let limits = RunLimits {
        timeout: Duration::from_secs(job.spec.timeout_seconds),
        still_waiting_after: Duration::from_secs(job.spec.still_waiting_seconds),
    };
    // Both callbacks tell `events`, one at a time, from this thread.
    let events = RefCell::new(events);
    let still_waiting = job.spec.still_waiting_seconds;
    let mut callbacks = RunCallbacks {
        on_launched: &mut || (*events.borrow_mut())(VendorEvent::Launched),
        on_still_waiting: &mut || {
            (*events.borrow_mut())(VendorEvent::StillWaiting {
                seconds: still_waiting,
            })
        },
    };

    let end = match job.spec.kind {
        VendorUninstallerKind::Nsis => {
            run_nsis(job, env, &located, limits, &mut outcome, &mut callbacks)
        }
        VendorUninstallerKind::Inno => {
            run_inno(job, env, &located, limits, &mut outcome, &mut callbacks)
        }
    };
    let end = match end {
        Ok(end) => end,
        Err(RunError::Refused(refusal)) => {
            return VendorOutcome {
                status: VendorStatus::Refused,
                refusal: Some(refusal),
                ..outcome
            };
        }
        Err(RunError::Failed(error)) => {
            outcome.status = VendorStatus::Failed;
            outcome.error = Some(error);
            return outcome;
        }
    };
    let code = match end {
        ProgramEnd::Exited(code) => code,
        ProgramEnd::StillRunning => {
            outcome.status = VendorStatus::StillRunning;
            return outcome;
        }
    };
    outcome.exit_code = Some(code);
    let exit = vendor_exit(job.spec.kind, code);
    // Inno may still be at work after it returns, so only its success is
    // waited for; any other ending gets one look, for the report.
    let poll = exit == VendorExit::Success && job.spec.kind == VendorUninstallerKind::Inno;
    let verdict = verify_gone(
        env,
        &located,
        &paths.verify_gone,
        limits.timeout,
        poll,
        &mut outcome,
    );
    outcome.status = match exit {
        VendorExit::Cancelled => VendorStatus::Cancelled,
        VendorExit::Failed => VendorStatus::Failed,
        VendorExit::Success => match verdict {
            Verdict::Gone => VendorStatus::Removed,
            Verdict::PendingRestart => VendorStatus::RemovedPendingRestart,
            Verdict::StillThere => VendorStatus::Failed,
        },
    };
    let removed = exit == VendorExit::Success && verdict != Verdict::StillThere;
    if removed {
        outcome.removed_leftovers = paths
            .remove_leftovers
            .iter()
            .map(|(kind, resolved)| LeftoverOutcome {
                path: resolved.path.clone(),
                kind: *kind,
                // Only once everything is gone, as the plan's
                // `remove_leftovers` promises.
                result: if verdict == Verdict::Gone {
                    remove_leftover(job.roots, *kind, resolved)
                } else {
                    LeftoverResult::Skipped
                },
            })
            .collect();
        outcome.leftovers = paths
            .report_leftovers
            .iter()
            .filter(|resolved| fs::symlink_metadata(&resolved.path).is_ok())
            .map(|resolved| resolved.path.clone())
            .collect();
        // Only what is still there counts: a later entry of the list may
        // have removed what an earlier one couldn't, such as a plain empty
        // folder where a junction was expected.
        let left_over = !outcome.leftovers.is_empty()
            || (verdict == Verdict::Gone
                && outcome
                    .removed_leftovers
                    .iter()
                    .any(LeftoverOutcome::still_there));
        if outcome.status == VendorStatus::Removed && left_over {
            outcome.status = VendorStatus::RemovedWithLeftovers;
        }
    }
    outcome
}

/// What a run tells while the uninstaller runs.
struct RunCallbacks<'a> {
    on_launched: &'a mut dyn FnMut(),
    on_still_waiting: &'a mut dyn FnMut(),
}

/// Why a run stopped before or while starting the uninstaller.
enum RunError {
    /// A check failed right before the start. Nothing ran.
    Refused(VendorRefusal),
    /// Something else went wrong; the outcome says whether it had started.
    Failed(String),
}

/// NSIS: copy the uninstaller into a folder only administrators can write
/// to, check the copy, and run it on the install folder.
fn run_nsis(
    job: &VendorJob<'_>,
    env: &dyn VendorEnvironment,
    located: &LocatedUninstaller,
    limits: RunLimits,
    outcome: &mut VendorOutcome,
    callbacks: &mut RunCallbacks<'_>,
) -> Result<ProgramEnd, RunError> {
    let Some(install_folder) = &located.install_folder else {
        return Err(RunError::Refused(VendorRefusal::InstallLocationMissing));
    };
    let in_place = NsisInPlaceArgument::new(install_folder).map_err(|_| {
        RunError::Refused(VendorRefusal::NotAFullPath {
            path: install_folder.clone(),
        })
    })?;
    let Some(file_name) = located.program.file_name() else {
        return Err(RunError::Refused(VendorRefusal::NotAFullPath {
            path: located.program.clone(),
        }));
    };
    let working = env.create_working_folder().map_err(|error| {
        RunError::Failed(format!(
            "RABBIT couldn't make a folder to run the uninstaller from: {error}"
        ))
    })?;
    let copy = working.join(file_name);
    let end = run_nsis_copy(
        job, env, located, &copy, &working, in_place, limits, outcome, callbacks,
    );
    // An uninstaller still running holds its copy open, so its folder stays,
    // and the report says so. It is under ProgramData, and only
    // administrators can use it.
    let removed = if matches!(end, Ok(ProgramEnd::StillRunning)) {
        Err("the uninstaller is still running from it".to_string())
    } else {
        remove_working_folder(&working, &copy).map_err(|error| error.to_string())
    };
    if let Err(error) = removed {
        outcome
            .problems
            .push(VendorProblem::WorkingFolderNotRemoved {
                path: working,
                error,
            });
    }
    end
}

#[allow(clippy::too_many_arguments)]
fn run_nsis_copy(
    job: &VendorJob<'_>,
    env: &dyn VendorEnvironment,
    located: &LocatedUninstaller,
    copy: &Path,
    working: &Path,
    in_place: NsisInPlaceArgument,
    limits: RunLimits,
    outcome: &mut VendorOutcome,
    callbacks: &mut RunCallbacks<'_>,
) -> Result<ProgramEnd, RunError> {
    let refuse_unreadable = |path: &Path, error: io::Error| {
        RunError::Refused(VendorRefusal::unreadable(path.display(), &error))
    };
    // Held from before the copy until both hashes are known, so neither can
    // change in between.
    let original = LockedFile::open(&located.program)
        .map_err(|error| refuse_unreadable(&located.program, error))?;
    env.copy_file(&located.program, copy).map_err(|error| {
        RunError::Failed(format!(
            "RABBIT couldn't copy the uninstaller to {}: {error}",
            working.display()
        ))
    })?;
    let copy_guard = LockedFile::open(copy).map_err(|error| refuse_unreadable(copy, error))?;
    let original_hash =
        hash_locked(&original).map_err(|error| refuse_unreadable(&located.program, error))?;
    let copy_hash = hash_locked(&copy_guard).map_err(|error| refuse_unreadable(copy, error))?;
    if original_hash != copy_hash {
        return Err(RunError::Refused(VendorRefusal::CopyDiffers));
    }
    // The uninstaller deletes the original, so it must be free by now. The
    // copy stays locked until it has started.
    drop(original);
    let command = VendorCommand {
        program: copy.to_path_buf(),
        arguments: job.spec.arguments.clone(),
        nsis_in_place: Some(in_place),
        working_directory: Some(working.to_path_buf()),
    };
    let mut copy_guard = Some(copy_guard);
    let mut launched = false;
    let end = env.run_program(
        &command,
        limits,
        &mut || {
            copy_guard.take();
            launched = true;
            (callbacks.on_launched)();
        },
        callbacks.on_still_waiting,
    );
    outcome.launched = launched;
    end.map_err(|error| {
        RunError::Failed(format!(
            "RABBIT couldn't {} the uninstaller: {error}",
            if launched { "wait for" } else { "start" }
        ))
    })
}

fn hash_locked(file: &LockedFile) -> io::Result<String> {
    let mut reader = file.file();
    reader.rewind()?;
    sha256_reader(&mut reader)
}

/// Remove the copy and its folder, in that order, and nothing else: the
/// folder is removed only once empty.
fn remove_working_folder(folder: &Path, copy: &Path) -> io::Result<()> {
    match fs::remove_file(copy) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::remove_dir(folder)
}

/// Inno: run the uninstaller where it is, after checking its folder once
/// more right before it starts.
fn run_inno(
    job: &VendorJob<'_>,
    env: &dyn VendorEnvironment,
    located: &LocatedUninstaller,
    limits: RunLimits,
    outcome: &mut VendorOutcome,
    callbacks: &mut RunCallbacks<'_>,
) -> Result<ProgramEnd, RunError> {
    // Locked first, so the file checked is the file that starts. A standard
    // user may still create files in the folder, so the folder is checked
    // after the lock, as late as possible.
    let guard = LockedFile::open(&located.program).map_err(|error| {
        RunError::Refused(VendorRefusal::unreadable(located.program.display(), &error))
    })?;
    check_uninstaller_folder(
        job,
        env,
        &located.program,
        located.install_folder.as_deref(),
    )
    .map_err(RunError::Refused)?;
    let command = VendorCommand {
        program: located.program.clone(),
        arguments: job.spec.arguments.clone(),
        nsis_in_place: None,
        working_directory: located.program.parent().map(Path::to_path_buf),
    };
    // Inno's uninstaller deletes itself at the end, so the lock goes as
    // soon as it has started.
    let mut guard = Some(guard);
    let mut launched = false;
    let end = env.run_program(
        &command,
        limits,
        &mut || {
            guard.take();
            launched = true;
            (callbacks.on_launched)();
        },
        callbacks.on_still_waiting,
    );
    outcome.launched = launched;
    end.map_err(|error| {
        RunError::Failed(format!(
            "RABBIT couldn't {} the uninstaller: {error}",
            if launched { "wait for" } else { "start" }
        ))
    })
}

/// Whether what the uninstaller was to remove is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Gone,
    /// The registry entry is gone, and everything still there goes at the
    /// next restart.
    PendingRestart,
    StillThere,
}

/// Check whether `verify_gone` and the registry entry are gone, and with
/// `poll` keep checking every [`VERIFY_POLL_INTERVAL`] until they are or
/// `timeout` has passed. Records what is still there in `outcome`.
fn verify_gone(
    env: &dyn VendorEnvironment,
    located: &LocatedUninstaller,
    verify_gone: &[PathBuf],
    timeout: Duration,
    poll: bool,
    outcome: &mut VendorOutcome,
) -> Verdict {
    let deadline = env.now() + timeout;
    loop {
        let still_present: Vec<PathBuf> = verify_gone
            .iter()
            .filter(|path| fs::symlink_metadata(path).is_ok())
            .cloned()
            .collect();
        let still_listed = match env.uninstall_views().views_with_key(&located.key) {
            Ok(views) => views.contains(&located.view),
            Err(error) => {
                record_once(
                    &mut outcome.problems,
                    VendorProblem::ListUnreadable {
                        error: error.to_string(),
                    },
                );
                true
            }
        };
        outcome.still_present = still_present.clone();
        outcome.still_listed = still_listed;
        if still_present.is_empty() && !still_listed {
            return Verdict::Gone;
        }
        if !still_listed {
            match env.pending_file_renames() {
                Ok(renames) => {
                    let pending = pending_under(&renames, &still_present);
                    let all_pending = still_present
                        .iter()
                        .all(|path| pending.iter().any(|file| inside_or_same(file, path)));
                    outcome.pending_restart = pending;
                    if all_pending {
                        return Verdict::PendingRestart;
                    }
                }
                Err(error) => record_once(
                    &mut outcome.problems,
                    VendorProblem::PendingRenamesUnreadable {
                        error: error.to_string(),
                    },
                ),
            }
        }
        let now = env.now();
        if !poll || now >= deadline {
            return Verdict::StillThere;
        }
        env.sleep(VERIFY_POLL_INTERVAL.min(deadline - now));
    }
}

fn record_once(problems: &mut Vec<VendorProblem>, problem: VendorProblem) {
    if !problems.contains(&problem) {
        problems.push(problem);
    }
}

/// The files and folders Windows deletes or moves at the next restart that
/// are at or below one of `paths`.
fn pending_under(renames: &[PendingFileRename], paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut pending: Vec<PathBuf> = renames
        .iter()
        .filter(|rename| {
            paths
                .iter()
                .any(|path| inside_or_same(&rename.source, path))
        })
        .map(|rename| rename.source.clone())
        .collect();
    pending.sort();
    pending.dedup();
    pending
}

/// Remove one leftover the plan names, through the guard: a junction as a
/// link and nothing else, a folder only when empty. Never follows a link.
fn remove_leftover(
    roots: &UninstallRoots,
    kind: LeftoverKind,
    resolved: &ResolvedPath,
) -> LeftoverResult {
    let path = &resolved.path;
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return LeftoverResult::NotFound,
        Err(error) => {
            return LeftoverResult::Failed {
                error: error.to_string(),
            };
        }
    }
    let checked = match kind {
        LeftoverKind::Junction => check_link_removal(roots, path, &resolved.guard),
        LeftoverKind::EmptyDir => check_removal(roots, path, &resolved.guard),
    };
    if let Err(refusal) = checked {
        return LeftoverResult::Refused { refusal };
    }
    // Leftovers never sit in a REAPER resource folder; a path that does is
    // not one this run may touch, however it got here.
    if resource_relative(&roots.resource, path).is_some() {
        return LeftoverResult::Refused {
            refusal: PathRefusal::OutsideRoot,
        };
    }
    let removed = match kind {
        LeftoverKind::Junction => rabbit_platform::remove_junction(path),
        LeftoverKind::EmptyDir => rabbit_platform::remove_empty_dir(path),
    };
    match (kind, removed) {
        (_, Ok(())) => LeftoverResult::Removed,
        (_, Err(error)) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {
            LeftoverResult::NotEmpty
        }
        // Swapped for something else after the guard looked: it stays.
        (LeftoverKind::Junction, Err(error)) if error.kind() == io::ErrorKind::InvalidInput => {
            LeftoverResult::Refused {
                refusal: PathRefusal::NotALink,
            }
        }
        (_, Err(error)) => LeftoverResult::Failed {
            error: error.to_string(),
        },
    }
}
