//! The run's one administrator command on macOS: what goes into it, what
//! runs it, and what the run makes of it afterwards.
//!
//! `osascript` asks for an administrator's password and runs a shell command
//! as root in one blocking call, and a second call asks again. So one call
//! carries every root step of a run, and it is the run's first change: it
//! comes after every check RABBIT makes as the user, and before anything
//! else moves (decision 31). Cancelling the password window changes nothing.
//!
//! Root is given only these steps, each program by its absolute path:
//! - first, `/usr/bin/pgrep -x -i -- <names>`, which must find nothing: no
//!   program the packages need closed, and no REAPER where that matters,
//!   started while the password window was open;
//! - `/bin/rm -R -f -x <path>` for each tree a package removes for good,
//!   exactly the path the administrator guard checked moments before;
//! - `/usr/sbin/pkgutil --forget <id>` for the package's installer receipts,
//!   whose failure the command tolerates;
//! - for a REAPER app only root can move (decision 32): `/bin/test -d` and
//!   `/bin/test ! -L` of the user's Trash folder, then `/bin/mv -h -n` of the
//!   app into it under a fresh name of RABBIT's own.
//!
//! What a package's status says comes from the filesystem afterwards, read
//! as the user: the command's exit status only says where it stopped.
//!
//! Everything here is platform-neutral. The shell itself is reached through
//! [`AdministratorShell`]: macOS's in RABBIT, a stand-in that records the
//! steps and carries them out on a temporary folder in tests.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rabbit_platform::{
    ADMINISTRATOR_SHELL_MAX_STEPS, AdministratorShellRequest, ElevationError, ShellCommand,
    ShellLinesOutcome, ShellStep, administrator_shell_text,
};
use serde::{Deserialize, Serialize};

use super::execute::{
    UninstallEnvironment, blocked_run_items, new_item, receipt_follows_detection, refused_run_items,
};
use super::helper::ElevationStartError;
use super::macos_objects::{FakeReceipts, MacosObjects, TreeRoute, tree_route};
use super::paths::{
    PathRefusal, UninstallRoots, check_administrator_removal, check_administrator_trash_move,
};
use super::preflight::{PROGRAMS_RUNNING_CHECK, blocking_programs, process_checks};
use super::{
    ByHandReason, PlannedRemoval, Removability, RemovalReason, RemovedFile, UninstallMessage,
    UninstallPlan, UninstallPlanItem, UninstallProblem, UninstallReportItem, UninstallStatus,
};
use crate::cancel::CancelToken;
use crate::package::uninstall::{UninstallBackup, UninstallPostStep};
use crate::preflight::{MACOS_REAPER_PROCESS_NAMES, PreflightCheck, PreflightStatus};
use crate::progress::{ProgressEvent, ProgressReporter, RemovalMethod};
use crate::receipt::{InstallState, save_install_state};

/// The programs root runs, each by its absolute path on macOS's sealed
/// system volume, so the root shell never looks a name up.
pub const PGREP: &str = "/usr/bin/pgrep";
pub const RM: &str = "/bin/rm";
pub const PKGUTIL: &str = "/usr/sbin/pkgutil";
pub const TEST: &str = "/bin/test";
pub const MV: &str = "/bin/mv";

/// How long the password window may stay open before the progress page
/// says RABBIT is still waiting for it.
pub const STILL_WAITING_AFTER: Duration = Duration::from_secs(30);

/// What kind of step the administrator command was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdministratorStepKind {
    /// No program the packages need closed is running (`pgrep`).
    ProgramsClosed,
    /// A tree deleted for good (`rm -R -f -x`).
    Delete,
    /// An installer receipt forgotten (`pkgutil --forget`).
    Forget,
    /// The user's Trash is a folder (`test -d`).
    TrashIsFolder,
    /// The user's Trash isn't a link (`test ! -L`).
    TrashIsNotLink,
    /// An app moved into the user's Trash (`mv -h -n`).
    MoveToTrash,
}

/// How one step of the administrator command went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdministratorStepVerdict {
    /// A dry run: the step a real run would give root.
    WouldRun,
    /// It ran, and the command went on.
    Ran,
    /// It ran and stopped the command.
    Failed,
    /// It never ran: the prompt was cancelled or failed, or an earlier step
    /// stopped the command.
    NotRun,
    /// The command ended without saying where it stopped, so whether this
    /// step ran is unknown; the package's status says what is gone.
    Unknown,
}

/// One step root was given, or in a dry run would be, for the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdministratorStepRecord {
    /// The package the step is for; `None` for the check that no program
    /// is running, which is for all of them.
    pub package_id: Option<String>,
    pub kind: AdministratorStepKind,
    /// What the step works on: the path it deletes or moves, the Trash it
    /// checks, the receipt it forgets, or the programs it looks for.
    pub target: String,
    /// Where a move puts the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    pub verdict: AdministratorStepVerdict,
}

/// One package's part of the administrator command, built only from paths
/// the administrator guard checked right before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminPackage {
    pub package_id: String,
    /// The programs that must not run, from the manifest.
    pub blocking_programs: Vec<String>,
    /// No REAPER at all may run while root works: the package serves every
    /// REAPER, or root moves REAPER's own app, which pgrep can't tell from
    /// a portable one. The plan says so, so RABBIT's own checks before the
    /// prompt ask the same.
    pub every_reaper_closed: bool,
    pub work: AdminWork,
}

/// What root does for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminWork {
    /// Delete each tree for good, in this order, then forget these
    /// installer receipts. Each tree is the exact text the guard checked.
    Delete {
        trees: Vec<String>,
        forget: Vec<String>,
    },
    /// Move REAPER's app into the user's Trash under a fresh name. `None`
    /// when the app is already gone.
    MoveToTrash(Option<TrashMove>),
}

/// A move of an app into the user's Trash by root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashMove {
    /// The app, as the guard checked it.
    pub app: String,
    /// The user's Trash folder, from the user database, never `$HOME`.
    pub trash: String,
    /// Where the app goes: a name in the Trash that RABBIT made up.
    pub destination: String,
}

/// What one step of the command is for, so its outcome can be read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminStepMeaning {
    /// None of `names` is running.
    ProgramsClosed {
        names: Vec<String>,
    },
    Delete {
        package_id: String,
        path: PathBuf,
    },
    Forget {
        package_id: String,
        id: String,
    },
    TrashIsFolder {
        package_id: String,
        trash: PathBuf,
    },
    TrashIsNotLink {
        package_id: String,
        trash: PathBuf,
    },
    MoveToTrash {
        package_id: String,
        from: PathBuf,
        to: PathBuf,
    },
}

impl AdminStepMeaning {
    /// The package the step is for, `None` for the check that no program
    /// is running.
    pub fn package_id(&self) -> Option<&str> {
        match self {
            Self::ProgramsClosed { .. } => None,
            Self::Delete { package_id, .. }
            | Self::Forget { package_id, .. }
            | Self::TrashIsFolder { package_id, .. }
            | Self::TrashIsNotLink { package_id, .. }
            | Self::MoveToTrash { package_id, .. } => Some(package_id),
        }
    }

    /// The path the step works on, if it works on one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Delete { path, .. } => Some(path),
            Self::TrashIsFolder { trash, .. } | Self::TrashIsNotLink { trash, .. } => Some(trash),
            Self::MoveToTrash { from, .. } => Some(from),
            Self::ProgramsClosed { .. } | Self::Forget { .. } => None,
        }
    }

    fn record(&self, verdict: AdministratorStepVerdict) -> AdministratorStepRecord {
        let text = |path: &Path| path.display().to_string();
        let (kind, target, destination) = match self {
            Self::ProgramsClosed { names } => (
                AdministratorStepKind::ProgramsClosed,
                names.join(", "),
                None,
            ),
            Self::Delete { path, .. } => (AdministratorStepKind::Delete, text(path), None),
            Self::Forget { id, .. } => (AdministratorStepKind::Forget, id.clone(), None),
            Self::TrashIsFolder { trash, .. } => {
                (AdministratorStepKind::TrashIsFolder, text(trash), None)
            }
            Self::TrashIsNotLink { trash, .. } => {
                (AdministratorStepKind::TrashIsNotLink, text(trash), None)
            }
            Self::MoveToTrash { from, to, .. } => (
                AdministratorStepKind::MoveToTrash,
                text(from),
                Some(text(to)),
            ),
        };
        AdministratorStepRecord {
            package_id: self.package_id().map(str::to_string),
            kind,
            target,
            destination,
            verdict,
        }
    }
}

/// The whole administrator command: its steps, and what each one means.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdminBatch {
    /// The packages, in the order the command takes them.
    pub packages: Vec<String>,
    pub steps: Vec<ShellStep>,
    /// What each step is for, at the same index as the step.
    pub meaning: Vec<AdminStepMeaning>,
}

impl AdminBatch {
    fn push(&mut self, step: ShellStep, meaning: AdminStepMeaning) {
        self.steps.push(step);
        self.meaning.push(meaning);
    }

    /// The steps for the report, each with the verdict `verdict` gives its
    /// index.
    pub fn records(
        &self,
        verdict: impl Fn(usize) -> AdministratorStepVerdict,
    ) -> Vec<AdministratorStepRecord> {
        self.meaning
            .iter()
            .enumerate()
            .map(|(index, meaning)| meaning.record(verdict(index)))
            .collect()
    }
}

/// How each step went, by its index, for a command that ended with
/// `outcome`.
pub fn verdicts_after(outcome: ShellLinesOutcome) -> impl Fn(usize) -> AdministratorStepVerdict {
    move |index| match outcome {
        ShellLinesOutcome::Completed => AdministratorStepVerdict::Ran,
        ShellLinesOutcome::FailedAt { index: failed } if index < failed => {
            AdministratorStepVerdict::Ran
        }
        ShellLinesOutcome::FailedAt { index: failed } if index == failed => {
            AdministratorStepVerdict::Failed
        }
        ShellLinesOutcome::FailedAt { .. } => AdministratorStepVerdict::NotRun,
        ShellLinesOutcome::Stopped { .. } => AdministratorStepVerdict::Unknown,
    }
}

fn command(program: &str, arguments: &[&str]) -> ShellCommand {
    ShellCommand::new(program, arguments.iter().copied())
}

/// The steps of the administrator command for `packages`, in the order
/// given, which is the run's: first the check that no program they need
/// closed is running, then each package's own steps.
///
/// A package that removes trees deletes them in its manifest's order,
/// then forgets its installer receipts: only those in `listed`, the
/// receipts an unprivileged `pkgutil --pkgs` listed, or all of them when
/// listing failed (`None`). A forget that fails changes nothing that
/// matters, so the command goes on past it. REAPER's app, which only root
/// can move, goes into the user's Trash after root has checked, right
/// before, that the Trash is a folder and not a link.
///
/// The names pgrep looks for are the blocking programs of the packages
/// root has something to do for, and REAPER's own when one of those needs
/// every REAPER closed. A package whose trees are all gone already, with no
/// receipt left to forget, adds no step at all.
/// They become one pattern, after `--` so none can be read as an option;
/// each must hold only letters, digits, spaces, `_` and `-`, so the
/// pattern can't carry regular expression syntax. A receipt ID must start
/// with a letter or digit and hold only letters, digits, `.`, `_` and `-`.
/// Either refused, more steps than one command takes, or a word the
/// administrator shell would refuse (a line break or NUL in a path), is an
/// error, given before any prompt.
pub fn administrator_steps(
    packages: &[AdminPackage],
    listed: Option<&BTreeSet<String>>,
) -> Result<AdminBatch, String> {
    let mut batch = AdminBatch {
        packages: packages
            .iter()
            .map(|package| package.package_id.clone())
            .collect(),
        ..AdminBatch::default()
    };
    let with_work: Vec<&AdminPackage> = packages
        .iter()
        .filter(|package| has_root_work(package, listed))
        .collect();
    let names = programs_to_find(&with_work)?;
    if !names.is_empty() {
        let pattern = names.join("|");
        batch.push(
            ShellStep::RunExpectingNoMatch(command(PGREP, &["-x", "-i", "--", &pattern])),
            AdminStepMeaning::ProgramsClosed { names },
        );
    }
    for package in packages {
        let package_id = &package.package_id;
        match &package.work {
            AdminWork::Delete { trees, forget } => {
                for tree in trees {
                    batch.push(
                        ShellStep::Run(command(RM, &["-R", "-f", "-x", tree])),
                        AdminStepMeaning::Delete {
                            package_id: package_id.clone(),
                            path: PathBuf::from(tree),
                        },
                    );
                }
                for id in forget
                    .iter()
                    .filter(|id| listed.is_none_or(|listed| listed.contains(*id)))
                {
                    if !is_receipt_id(id) {
                        return Err(format!(
                            "{package_id}'s installer receipt {id:?} isn't a name RABBIT hands \
                             to pkgutil"
                        ));
                    }
                    batch.push(
                        ShellStep::RunIgnoringFailure(command(PKGUTIL, &["--forget", id])),
                        AdminStepMeaning::Forget {
                            package_id: package_id.clone(),
                            id: id.clone(),
                        },
                    );
                }
            }
            AdminWork::MoveToTrash(None) => {}
            AdminWork::MoveToTrash(Some(trash_move)) => {
                let trash = PathBuf::from(&trash_move.trash);
                batch.push(
                    ShellStep::Run(command(TEST, &["-d", &trash_move.trash])),
                    AdminStepMeaning::TrashIsFolder {
                        package_id: package_id.clone(),
                        trash: trash.clone(),
                    },
                );
                batch.push(
                    ShellStep::Run(command(TEST, &["!", "-L", &trash_move.trash])),
                    AdminStepMeaning::TrashIsNotLink {
                        package_id: package_id.clone(),
                        trash,
                    },
                );
                batch.push(
                    ShellStep::Run(command(
                        MV,
                        &["-h", "-n", &trash_move.app, &trash_move.destination],
                    )),
                    AdminStepMeaning::MoveToTrash {
                        package_id: package_id.clone(),
                        from: PathBuf::from(&trash_move.app),
                        to: PathBuf::from(&trash_move.destination),
                    },
                );
            }
        }
    }
    if batch.steps.len() > ADMINISTRATOR_SHELL_MAX_STEPS {
        return Err(format!(
            "the administrator command would need {} steps, and one prompt takes at most {}",
            batch.steps.len(),
            ADMINISTRATOR_SHELL_MAX_STEPS
        ));
    }
    // Every word rabbit-platform would refuse, such as a line break in a
    // path or a program that isn't absolute, is refused here too: the run
    // stops before it says a password window is coming, and the dry run
    // reports the same refusal.
    if let Err(error) = administrator_shell_text(&batch.steps) {
        return Err(format!(
            "the administrator command can't be given to root: {error}"
        ));
    }
    Ok(batch)
}

/// Whether root has anything to do for `package`: a tree still there to
/// delete or move, or a receipt to forget that the installer still lists
/// (`listed`), or may (`None`, listing failed).
fn has_root_work(package: &AdminPackage, listed: Option<&BTreeSet<String>>) -> bool {
    match &package.work {
        AdminWork::MoveToTrash(trash_move) => trash_move.is_some(),
        AdminWork::Delete { trees, forget } => {
            !trees.is_empty()
                || forget
                    .iter()
                    .any(|id| listed.is_none_or(|listed| listed.contains(id)))
        }
    }
}

/// The names pgrep looks for, each once, ignoring case, in the order the
/// packages give them, REAPER's last.
fn programs_to_find(packages: &[&AdminPackage]) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = Vec::new();
    let mut add = |name: &str| {
        if !names.iter().any(|known| known.eq_ignore_ascii_case(name)) {
            names.push(name.to_string());
        }
    };
    for package in packages {
        for name in &package.blocking_programs {
            if !is_program_name(name) {
                return Err(format!(
                    "{}'s program name {name:?} isn't one RABBIT hands to pgrep",
                    package.package_id
                ));
            }
            add(name);
        }
    }
    if packages.iter().any(|package| package.every_reaper_closed) {
        for name in MACOS_REAPER_PROCESS_NAMES {
            add(name);
        }
    }
    Ok(names)
}

/// A program name pgrep may look for: letters, digits, spaces, `_` and
/// `-`, as the manifest's validation allows on macOS.
fn is_program_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
}

/// An installer receipt ID pkgutil may be given: starting with a letter or
/// digit, so it can't be read as an option, then letters, digits, `.`, `_`
/// and `-`, as the manifest's validation allows.
fn is_receipt_id(id: &str) -> bool {
    id.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The name RABBIT gives an app root moves into the Trash: the app's own
/// name with the local time `at` added, such as `REAPER 2026-11-07
/// 15.30.12.app` for `REAPER.app`, so it never takes the name of something
/// already there. A real run takes `at` once, just before the password
/// window (a dry run when it lists the steps), so the name says when the
/// run started, not the moment of the move.
/// `utc_offset_seconds` is how far local time is ahead of UTC at `at`.
/// `None` for a path without a name that is valid text, or a moment too
/// far from 1970 to count.
pub fn trash_name(app: &Path, at: SystemTime, utc_offset_seconds: i32) -> Option<String> {
    let stem = app.file_stem()?.to_str()?;
    let extension = match app.extension() {
        Some(extension) => Some(extension.to_str()?),
        None => None,
    };
    let unix = match at.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).ok()?,
        Err(before) => -i64::try_from(before.duration().as_secs()).ok()?,
    };
    let local = rabbit_platform::civil_date_time(unix, utc_offset_seconds)?;
    let mut name = format!(
        "{stem} {:04}-{:02}-{:02} {:02}.{:02}.{:02}",
        local.year, local.month, local.day, local.hour, local.minute, local.second
    );
    if let Some(extension) = extension {
        name.push('.');
        name.push_str(extension);
    }
    Some(name)
}

/// What the failing step said, and whether macOS refused it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepError {
    /// macOS said "Operation not permitted": App Management, System
    /// Integrity Protection or a file flag refused even root.
    pub refused_by_macos: bool,
    /// The step's own message, if it wrote one.
    pub detail: Option<String>,
}

/// Read the failing step's message. "Operation not permitted" is EPERM,
/// which root gets only from macOS's own protections, App Management among
/// them; the user can do something about that one.
pub fn classify_step_error(error_text: Option<&str>) -> StepError {
    let detail = error_text
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string);
    StepError {
        refused_by_macos: detail
            .as_deref()
            .is_some_and(|text| text.contains("Operation not permitted")),
        detail,
    }
}

/// What became of one package of the administrator command, read from the
/// filesystem afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminPackageResult {
    /// Every path of it is gone. `trashed_to` is where root moved its app,
    /// only when RABBIT can see it there. `ended_unclearly` holds the exit
    /// status of a command that ended without naming a step.
    Removed {
        gone: Vec<PathBuf>,
        trashed_to: Option<PathBuf>,
        ended_unclearly: Option<Option<i32>>,
    },
    /// Some or all of it is still there after its steps ran, or may have.
    /// `path` is what the failing step worked on, only when that step was
    /// this package's; `ended_unclearly` when the command didn't say which
    /// step stopped it.
    Failed {
        path: Option<PathBuf>,
        ended_unclearly: bool,
        gone: Vec<PathBuf>,
        left: Vec<PathBuf>,
        refused_by_macos: bool,
        detail: Option<String>,
    },
    /// The app at `path` has left its place, but root's move didn't run,
    /// or whether it ran is unknown, and RABBIT can't see it in the Trash:
    /// something RABBIT can't name may have moved it.
    TrashUnclear { path: PathBuf },
    /// The command stopped at `stopped_at` before this package, and
    /// nothing of it is gone.
    NotReached { stopped_at: String },
    /// The first step found a program running that must be closed, so
    /// nothing ran at all.
    ProgramRunning,
}

/// What became of each package of `batch`, after a command that ran and
/// ended with `outcome`, the failing step having said `error_text`.
/// `presence` reads a path as the user: `Some(true)` there, `Some(false)`
/// gone, `None` when it can't be told, which never counts as gone.
///
/// The filesystem always wins over the exit status:
/// - a package whose paths are all gone is Removed, whatever the status;
///   a command that didn't say where it stopped adds a note;
/// - except an app root was to move, which is gone while its move didn't
///   run, or may not have, and RABBIT can't see it in the Trash: that is
///   TrashUnclear, because something else may have moved it;
/// - one whose paths are partly gone is Failed, with what is gone and what
///   is left;
/// - one with nothing gone is Failed if its step stopped the command, or if
///   it is the first package not fully removed; after the package whose
///   step stopped the command, or the first package not fully removed, a
///   package with nothing gone wasn't reached;
/// - a failing first step, the check that no program runs, means nothing
///   ran, for every package.
///
/// Where root moved an app is claimed only when RABBIT sees something
/// there and nothing of the app's name inside it: BSD `mv` moves into a
/// folder already at the destination, and then the destination isn't the
/// app.
pub fn administrator_results(
    batch: &AdminBatch,
    outcome: ShellLinesOutcome,
    error_text: Option<&str>,
    presence: &dyn Fn(&Path) -> Option<bool>,
) -> Vec<(String, AdminPackageResult)> {
    let failed_step = match outcome {
        ShellLinesOutcome::FailedAt { index } => batch.meaning.get(index),
        _ => None,
    };
    if matches!(failed_step, Some(AdminStepMeaning::ProgramsClosed { .. })) {
        return batch
            .packages
            .iter()
            .map(|id| (id.clone(), AdminPackageResult::ProgramRunning))
            .collect();
    }
    let failing_package = failed_step.and_then(AdminStepMeaning::package_id);
    let ended_unclearly = match outcome {
        ShellLinesOutcome::Stopped { code } => Some(code),
        // A step index past the end can't come from the shell; read it
        // like a command that didn't say.
        ShellLinesOutcome::FailedAt { .. } if failed_step.is_none() => Some(None),
        _ => None,
    };
    let some_steps_unrun = outcome != ShellLinesOutcome::Completed;
    let step_error = classify_step_error(error_text);
    let mut stopped_at: Option<&str> = None;
    let mut results = Vec::new();
    for id in &batch.packages {
        let mut gone = Vec::new();
        let mut left = Vec::new();
        // The app root was to move, where to, and whether the move ran.
        let mut moved: Option<(&Path, &Path, bool)> = None;
        for (index, meaning) in batch
            .meaning
            .iter()
            .enumerate()
            .filter(|(_, meaning)| meaning.package_id() == Some(id.as_str()))
        {
            let path = match meaning {
                AdminStepMeaning::Delete { path, .. } => path,
                AdminStepMeaning::MoveToTrash { from, to, .. } => {
                    let ran = match outcome {
                        ShellLinesOutcome::Completed => true,
                        ShellLinesOutcome::FailedAt { index: failed } => index < failed,
                        ShellLinesOutcome::Stopped { .. } => false,
                    };
                    moved = Some((from.as_path(), to.as_path(), ran));
                    from
                }
                _ => continue,
            };
            if presence(path) == Some(false) {
                gone.push(path.clone());
            } else {
                left.push(path.clone());
            }
        }
        let here = failing_package == Some(id.as_str());
        // The command stopped at this package's step, even when all of its
        // paths are gone anyway: a later package with nothing gone wasn't
        // reached.
        if here {
            stopped_at.get_or_insert(id.as_str());
        }
        // BSD `mv` moves into a folder already at the destination, and then
        // the app is in there under its own name: the destination is the
        // app only when it is there and holds nothing of that name.
        let trashed_to = moved.and_then(|(from, to, _)| {
            let inside = to.join(from.file_name()?);
            (presence(to) == Some(true) && presence(&inside) == Some(false))
                .then(|| to.to_path_buf())
        });
        // Gone, but not by a move RABBIT knows ran, and not where RABBIT
        // can see it: something RABBIT can't name may have moved it.
        let unconfirmed = moved.filter(|&(from, _, ran)| {
            !ran && trashed_to.is_none() && gone.iter().any(|path| path.as_path() == from)
        });
        let result = if let Some((from, _, _)) = unconfirmed {
            stopped_at.get_or_insert(id.as_str());
            AdminPackageResult::TrashUnclear {
                path: from.to_path_buf(),
            }
        } else if left.is_empty() {
            AdminPackageResult::Removed {
                gone,
                trashed_to,
                ended_unclearly,
            }
        } else if let Some(stop) =
            stopped_at.filter(|_| gone.is_empty() && some_steps_unrun && !here)
        {
            AdminPackageResult::NotReached {
                stopped_at: stop.to_string(),
            }
        } else {
            // Only the step that stopped the command is named as where it
            // stopped. A package still there after a command that ran to its
            // end, or stopped at another package's step, names none: what is
            // left says the rest.
            let path = if here {
                failed_step
                    .and_then(AdminStepMeaning::path)
                    .map(Path::to_path_buf)
            } else {
                None
            };
            let says = here || ended_unclearly.is_some();
            stopped_at.get_or_insert(id.as_str());
            AdminPackageResult::Failed {
                path,
                ended_unclearly: ended_unclearly.is_some(),
                gone,
                left,
                refused_by_macos: says && step_error.refused_by_macos,
                detail: if says {
                    step_error.detail.clone()
                } else {
                    None
                },
            }
        };
        results.push((id.clone(), result));
    }
    results
}

/// Something that happened while the administrator command ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminShellEvent {
    /// The password window has been open for `seconds` without an answer.
    StillWaiting { seconds: u64 },
}

/// How a call of the administrator command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminShellRun {
    /// The password window was cancelled; nothing ran.
    Declined,
    /// Nothing ran, for `reason`: the window failed (a wrong password three
    /// times, an account that isn't an administrator, no window over SSH),
    /// `osascript` couldn't start, RABBIT refused the command, or this
    /// program may not ask at all.
    NotRun { reason: String },
    /// The command ran; `outcome` says how far, `error_text` what the
    /// failing step said.
    Ran {
        outcome: ShellLinesOutcome,
        error_text: Option<String>,
    },
}

/// Runs the administrator command: one password window, then root's
/// steps, in one blocking call.
pub trait AdministratorShell {
    /// Run `request`'s steps as root. `on_event` hears that RABBIT is still
    /// waiting for the password window.
    fn run(
        &self,
        request: &AdministratorShellRequest,
        on_event: &mut dyn FnMut(AdminShellEvent),
    ) -> AdminShellRun;
}

/// macOS's administrator command, through `osascript`: RABBIT itself.
///
/// It refuses, before any prompt, in a test build and in any program not
/// named like RABBIT, as the elevated helper does; `rabbit-platform`
/// refuses again. Elsewhere than macOS it answers that nothing ran.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemAdministratorShell;

impl AdministratorShell for SystemAdministratorShell {
    fn run(
        &self,
        request: &AdministratorShellRequest,
        on_event: &mut dyn FnMut(AdminShellEvent),
    ) -> AdminShellRun {
        if cfg!(test) {
            return AdminShellRun::NotRun {
                reason: "a test build never asks for administrator rights".to_string(),
            };
        }
        if !rabbit_platform::running_as_rabbit() {
            return AdminShellRun::NotRun {
                reason: "only RABBIT itself asks for administrator rights, and this program \
                         isn't named like RABBIT"
                    .to_string(),
            };
        }
        let seconds = STILL_WAITING_AFTER.as_secs();
        let mut still_waiting = || on_event(AdminShellEvent::StillWaiting { seconds });
        match rabbit_platform::run_administrator_shell(
            request,
            STILL_WAITING_AFTER,
            &mut still_waiting,
        ) {
            Ok(report) => AdminShellRun::Ran {
                outcome: report.outcome,
                error_text: report.error_text,
            },
            Err(error) => shell_run_from_error(error),
        }
    }
}

/// What an error from `rabbit-platform` means for the run.
pub fn shell_run_from_error(error: ElevationError) -> AdminShellRun {
    match error {
        ElevationError::UserCancelledElevation { .. } => AdminShellRun::Declined,
        // RABBIT stopped hearing from `osascript` after it may have started
        // the command: only the filesystem can say what changed.
        ElevationError::WaitFailed { message, .. } => AdminShellRun::Ran {
            outcome: ShellLinesOutcome::Stopped { code: None },
            error_text: Some(message),
        },
        other => AdminShellRun::NotRun {
            reason: other.to_string(),
        },
    }
}

/// An administrator command that never runs, for runs that may not ask
/// for administrator rights.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAdministratorShell;

impl AdministratorShell for NoAdministratorShell {
    fn run(
        &self,
        _request: &AdministratorShellRequest,
        _on_event: &mut dyn FnMut(AdminShellEvent),
    ) -> AdminShellRun {
        AdminShellRun::NotRun {
            reason: "this run may not ask for administrator rights".to_string(),
        }
    }
}

/// How a [`RecordingAdministratorShell`] answers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum RecordedAnswer {
    #[default]
    Run,
    Decline,
    NotRun(String),
}

/// A stand-in for tests, in this crate and the UI's and the CLI's: it
/// records every request, and carries each step out with the user's own
/// rights on the paths it names, which a test points at a temporary folder
/// standing in for `/`:
/// - `rm -R -f -x` deletes the tree, and a missing one is fine;
/// - `mv -h -n` renames. A file or link already at the destination leaves
///   both alone, and the step still succeeds. A real folder there takes
///   the app inside it under its own name, as BSD `mv` does;
/// - `test -d` and `test ! -L` check the folder, a junction counting as a
///   link on Windows;
/// - `pkgutil --forget` takes the ID out of the receipts given with
///   [`forgetting_in`](Self::forgetting_in);
/// - `pgrep` finds nothing.
///
/// Like the real one, it refuses a request whose text the administrator
/// shell would refuse, before any window. It can answer as a cancelled
/// window, as a command that never ran, stop at a step without carrying it
/// out, or end without naming a step, and it can say the window is still
/// open before any answer. It refuses to do anything inside RABBIT itself.
#[derive(Debug, Default)]
pub struct RecordingAdministratorShell {
    answer: RecordedAnswer,
    failing_at: Option<usize>,
    stop_code: Option<Option<i32>>,
    error_text: Option<String>,
    receipts: Option<FakeReceipts>,
    still_waiting: Option<u64>,
    requests: Mutex<Vec<Vec<ShellStep>>>,
}

impl RecordingAdministratorShell {
    pub fn new() -> Self {
        Self::default()
    }

    /// The password window is cancelled.
    pub fn declining(mut self) -> Self {
        self.answer = RecordedAnswer::Decline;
        self
    }

    /// Nothing runs, for `reason`, as after a failed password window.
    pub fn not_run(mut self, reason: &str) -> Self {
        self.answer = RecordedAnswer::NotRun(reason.to_string());
        self
    }

    /// The step at `index` stops the command without doing anything.
    pub fn failing_at(mut self, index: usize) -> Self {
        self.failing_at = Some(index);
        self
    }

    /// The command ends with `code`, or no code, without naming a step,
    /// after the steps it got to.
    pub fn stopping(mut self, code: Option<i32>) -> Self {
        self.stop_code = Some(code);
        self
    }

    /// What a failing step says on its error output.
    pub fn saying(mut self, error_text: &str) -> Self {
        self.error_text = Some(error_text.to_string());
        self
    }

    /// `pkgutil --forget` takes IDs out of `receipts`, those of a
    /// [`FakeMacosObjects`](super::FakeMacosObjects).
    pub fn forgetting_in(mut self, receipts: FakeReceipts) -> Self {
        self.receipts = Some(receipts);
        self
    }

    /// The password window stays open for `seconds` before it is answered.
    pub fn still_waiting(mut self, seconds: u64) -> Self {
        self.still_waiting = Some(seconds);
        self
    }

    /// How often the command was asked for: each call is one password
    /// window.
    pub fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    /// The steps of the last call, empty if there was none.
    pub fn steps(&self) -> Vec<ShellStep> {
        self.requests
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }

    /// Carry out one step on the paths it names; whether it succeeded.
    fn apply(&self, step: &ShellStep) -> bool {
        let command = step.command();
        let arguments: Vec<&str> = command.arguments.iter().map(String::as_str).collect();
        match (command.program.to_str(), arguments.as_slice()) {
            // Nothing is running, which is what this step wants.
            (Some(PGREP), _) => matches!(step, ShellStep::RunExpectingNoMatch(_)),
            (Some(RM), ["-R", "-f", "-x", path]) => delete_tree(Path::new(path)).is_ok(),
            (Some(PKGUTIL), ["--forget", id]) => {
                if let Some(receipts) = &self.receipts
                    && let Some(listed) = receipts.lock().unwrap().as_mut()
                {
                    listed.remove(*id);
                }
                true
            }
            (Some(TEST), ["-d", path]) => fs::metadata(path).is_ok_and(|found| found.is_dir()),
            (Some(TEST), ["!", "-L", path]) => {
                !fs::symlink_metadata(path).is_ok_and(|found| found.file_type().is_symlink())
            }
            (Some(MV), ["-h", "-n", from, to]) => {
                let from = Path::new(from);
                // BSD mv: a real folder at the destination takes the app
                // inside it, under its own name; with -h a link there
                // doesn't count as a folder.
                let to = match fs::symlink_metadata(to) {
                    Ok(found) if found.is_dir() && !found.file_type().is_symlink() => {
                        match from.file_name() {
                            Some(name) => Path::new(to).join(name),
                            None => return false,
                        }
                    }
                    _ => PathBuf::from(to),
                };
                // -n: a taken name leaves both alone, and the step still
                // succeeds.
                fs::symlink_metadata(&to).is_ok() || fs::rename(from, &to).is_ok()
            }
            _ => false,
        }
    }
}

/// `rm -R -f` of one path, without following a link: a missing path is
/// fine.
fn delete_tree(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(found) if found.is_dir() && !found.file_type().is_symlink() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path).or_else(|_| fs::remove_dir(path)),
    }
}

impl AdministratorShell for RecordingAdministratorShell {
    fn run(
        &self,
        request: &AdministratorShellRequest,
        on_event: &mut dyn FnMut(AdminShellEvent),
    ) -> AdminShellRun {
        self.requests.lock().unwrap().push(request.steps.clone());
        if rabbit_platform::running_as_rabbit() {
            return AdminShellRun::NotRun {
                reason: "a stand-in for tests never runs inside RABBIT itself".to_string(),
            };
        }
        // As the real one: a text the administrator shell refuses never
        // gets a window, and nothing runs.
        if let Err(error) = administrator_shell_text(&request.steps) {
            return shell_run_from_error(error);
        }
        // The real shell says this while the window is still open, so
        // before any answer, a cancel included.
        if let Some(seconds) = self.still_waiting {
            on_event(AdminShellEvent::StillWaiting { seconds });
        }
        match &self.answer {
            RecordedAnswer::Decline => return AdminShellRun::Declined,
            RecordedAnswer::NotRun(reason) => {
                return AdminShellRun::NotRun {
                    reason: reason.clone(),
                };
            }
            RecordedAnswer::Run => {}
        }
        let mut failed_at = None;
        for (index, step) in request.steps.iter().enumerate() {
            let tolerated = matches!(step, ShellStep::RunIgnoringFailure(_));
            let succeeded = self.failing_at != Some(index) && self.apply(step);
            if !succeeded && !tolerated {
                failed_at = Some(index);
                break;
            }
        }
        let outcome = match (self.stop_code, failed_at) {
            (Some(code), _) => ShellLinesOutcome::Stopped { code },
            (None, Some(index)) => ShellLinesOutcome::FailedAt { index },
            (None, None) => ShellLinesOutcome::Completed,
        };
        AdminShellRun::Ran {
            outcome,
            error_text: (outcome != ShellLinesOutcome::Completed)
                .then(|| self.error_text.clone())
                .flatten(),
        }
    }
}

/// Whether `path` is there, read with `lstat` as the user: `None` when it
/// can't be told, such as for a place macOS keeps private.
fn presence(path: &Path) -> Option<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Some(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
    }
}

fn is_gone(path: &Path) -> bool {
    presence(path) == Some(false)
}

/// The part of the administrator command for `item`, from the guard's
/// checks run right now, or why the run must stop before any prompt.
///
/// A tree root deletes passes [`check_administrator_removal`]. REAPER's
/// app is asked about again and must still be one only root can move,
/// passes [`check_administrator_trash_move`], and goes into the user's
/// Trash under a fresh name. A tree gone since the plan has nothing left to
/// remove.
pub(super) fn prepare_package(
    item: &UninstallPlanItem,
    roots: &UninstallRoots,
    objects: &dyn MacosObjects,
    at: SystemTime,
) -> Result<AdminPackage, UninstallMessage> {
    let refused = |path: &Path, refusal: PathRefusal| UninstallMessage::PathRefused {
        path: path.to_path_buf(),
        refusal,
    };
    let trees: Vec<&PlannedRemoval> = item
        .files
        .iter()
        .filter(|file| file.reason == RemovalReason::Tree && !is_gone(&file.path))
        .collect();
    let work = if item.backup == UninstallBackup::ToTrash {
        match trees.as_slice() {
            [] => AdminWork::MoveToTrash(None),
            [app] => AdminWork::MoveToTrash(Some(trash_move(app, roots, objects, at)?)),
            // A plan root may move an app with names `{app_path}` alone.
            _ => return Err(UninstallMessage::NotYetSupported),
        }
    } else {
        let mut texts = Vec::new();
        for tree in trees {
            texts.push(
                check_administrator_removal(roots, tree).map_err(|why| refused(&tree.path, why))?,
            );
        }
        AdminWork::Delete {
            trees: texts,
            forget: item.macos_pkg_forget.clone(),
        }
    };
    Ok(AdminPackage {
        package_id: item.package_id.clone(),
        blocking_programs: item.blocking_programs.clone(),
        every_reaper_closed: item.every_reaper_closed,
        work,
    })
}

/// Root's move of `app` into the user's Trash, checked again right now.
fn trash_move(
    app: &PlannedRemoval,
    roots: &UninstallRoots,
    objects: &dyn MacosObjects,
    at: SystemTime,
) -> Result<TrashMove, UninstallMessage> {
    let by_hand = |reason: ByHandReason| UninstallMessage::NotRemovable {
        removability: Removability::ByHand {
            path: app.path.clone(),
            reason,
        },
    };
    let trash = objects.user_trash_folder_facts();
    match tree_route(
        &app.path,
        &objects.facts(&app.path),
        &trash,
        app.bundle_id.as_deref(),
        &roots.system_root,
    ) {
        TreeRoute::TrashAsAdministrator => {}
        TreeRoute::ByHand(reason) => return Err(by_hand(reason)),
        // The user may move it now after all: something changed since the
        // plan, and the run doesn't guess which route is right.
        TreeRoute::Trash => {
            return Err(by_hand(ByHandReason::CouldNotCheck {
                error: "who may move it changed while RABBIT was checking it".to_string(),
            }));
        }
    }
    let app_text = check_administrator_trash_move(roots, app).map_err(|refusal| {
        UninstallMessage::PathRefused {
            path: app.path.clone(),
            refusal,
        }
    })?;
    // tree_route only says TrashAsAdministrator for a Trash it could read.
    let trash = trash.map_err(|error| {
        by_hand(ByHandReason::CouldNotCheck {
            error: error.to_string(),
        })
    })?;
    let not_text = |path: &Path| UninstallMessage::PathRefused {
        path: path.to_path_buf(),
        refusal: PathRefusal::NotUtf8,
    };
    let trash_text = trash.path.to_str().ok_or_else(|| not_text(&trash.path))?;
    let offset = rabbit_platform::local_utc_offset_seconds(at).unwrap_or(0);
    let name = trash_name(&app.path, at, offset).ok_or_else(|| not_text(&app.path))?;
    let destination = trash.path.join(name);
    let destination_text = destination.to_str().ok_or_else(|| not_text(&destination))?;
    Ok(TrashMove {
        app: app_text,
        trash: trash_text.to_string(),
        destination: destination_text.to_string(),
    })
}

/// The receipts the installer lists now, as the user, when any package
/// forgets some; `None` when none does.
fn listed_receipts(
    packages: &[AdminPackage],
    objects: &dyn MacosObjects,
) -> Option<io::Result<BTreeSet<String>>> {
    packages
        .iter()
        .any(|package| matches!(&package.work, AdminWork::Delete { forget, .. } if !forget.is_empty()))
        .then(|| objects.installed_package_ids())
}

/// The administrator command's packages of `to_remove`, which the run
/// takes first.
fn batch_items<'p>(to_remove: &[&'p UninstallPlanItem]) -> Vec<&'p UninstallPlanItem> {
    to_remove
        .iter()
        .copied()
        .filter(|item| item.needs_administrator())
        .collect()
}

/// Run the administrator command for the packages of `to_remove` that need
/// it, as the run's first change, and read what it did.
///
/// Before the one prompt, in order: the cancel token; the process checks
/// again, for these packages; the administrator guard on every path root
/// is given, and for REAPER's app the question of who may move it; then
/// the installer's receipts, listed as the user; then the steps themselves,
/// which must be ones the administrator shell takes. Anything that stops
/// the run here stops it with nothing changed: the package it concerns is
/// Blocked and the rest weren't reached.
///
/// Then [`ProgressEvent::AdministratorPrompt`], and the command, unless
/// everything root would delete or move is gone already: then there is no
/// window at all. A cancelled password window cancels every package; a
/// command that didn't run fails these packages and leaves the rest
/// unreached. After a command that ran, each package's status comes from
/// the filesystem (see [`administrator_results`]), its receipt entry
/// follows fresh detection, its installer receipts are listed again, and
/// an Audio Unit refresh it asks for is made once anything of it is gone.
///
/// Returns the items of these packages when all of them are removed, for
/// the run to go on with the rest. Otherwise the run stops, and the items
/// of every package of `to_remove`, in its order, come back as the error.
/// The steps root was given, and how each went, go into `records`.
pub(super) fn run_administrator_batch(
    to_remove: &[&UninstallPlanItem],
    plan: &UninstallPlan,
    environment: &UninstallEnvironment<'_>,
    state: &mut InstallState,
    progress: &ProgressReporter,
    cancel: &CancelToken,
    records: &mut Vec<AdministratorStepRecord>,
) -> Result<Vec<UninstallReportItem>, Vec<UninstallReportItem>> {
    let batch_items = batch_items(to_remove);
    if batch_items.is_empty() {
        return Ok(Vec::new());
    }
    let batch_ids: Vec<String> = batch_items
        .iter()
        .map(|item| item.package_id.clone())
        .collect();
    if cancel.is_cancelled() {
        return Err(to_remove
            .iter()
            .map(|item| {
                new_item(
                    item,
                    UninstallStatus::Cancelled,
                    UninstallMessage::Cancelled,
                )
            })
            .collect());
    }

    let blocked = failing_process_checks(plan, &batch_items, environment);
    if !blocked.is_empty() {
        return Err(blocked_run_items(to_remove, blocked));
    }
    let at = SystemTime::now();
    let mut packages = Vec::new();
    let mut blocked = Vec::new();
    for item in &batch_items {
        match prepare_package(item, environment.roots, environment.macos_objects, at) {
            Ok(package) => packages.push(package),
            Err(code) => blocked.push((item.package_id.clone(), code)),
        }
    }
    if !blocked.is_empty() {
        return Err(blocked_run_items(to_remove, blocked));
    }
    let listed = listed_receipts(&packages, environment.macos_objects);
    let batch = match administrator_steps(
        &packages,
        listed.as_ref().and_then(|listed| listed.as_ref().ok()),
    ) {
        Ok(batch) => batch,
        // Refused before any prompt, with nothing changed: Blocked, as the
        // dry run reports it.
        Err(reason) => {
            return Err(blocked_run_items(
                to_remove,
                batch_ids
                    .iter()
                    .map(|id| {
                        (
                            id.clone(),
                            UninstallMessage::ElevationFailed {
                                error: reason.clone(),
                            },
                        )
                    })
                    .collect(),
            ));
        }
    };

    // Everything root would delete or move is gone already, so there is
    // nothing to ask a password for: the checks RABBIT made as the user
    // just now stand for root's own.
    let root_work = batch
        .meaning
        .iter()
        .any(|meaning| meaning.package_id().is_some());
    let (outcome, error_text) = if root_work {
        progress.report(ProgressEvent::AdministratorPrompt {
            package_ids: batch_ids.clone(),
        });
        let request = AdministratorShellRequest {
            steps: batch.steps.clone(),
        };
        let run = environment
            .administrator_shell
            .run(&request, &mut |event| match event {
                AdminShellEvent::StillWaiting { seconds } => {
                    progress.report(ProgressEvent::AdministratorStillWaiting { seconds });
                }
            });
        let (outcome, error_text) = match run {
            AdminShellRun::Declined => {
                records.extend(batch.records(|_| AdministratorStepVerdict::NotRun));
                return Err(refused_run_items(
                    to_remove,
                    &batch_ids,
                    &ElevationStartError::Declined,
                ));
            }
            AdminShellRun::NotRun { reason } => {
                records.extend(batch.records(|_| AdministratorStepVerdict::NotRun));
                return Err(refused_run_items(
                    to_remove,
                    &batch_ids,
                    &ElevationStartError::Failed(reason),
                ));
            }
            AdminShellRun::Ran {
                outcome,
                error_text,
            } => (outcome, error_text),
        };
        records.extend(batch.records(verdicts_after(outcome)));
        (outcome, error_text)
    } else {
        (ShellLinesOutcome::Completed, None)
    };

    let results = administrator_results(&batch, outcome, error_text.as_deref(), &presence);
    if results
        .iter()
        .any(|(_, result)| *result == AdminPackageResult::ProgramRunning)
    {
        return Err(programs_running_items(
            to_remove,
            plan,
            &batch_items,
            &batch,
            environment,
        ));
    }

    let mut items: Vec<UninstallReportItem> = Vec::new();
    for ((plan_item, package), (_, result)) in batch_items.iter().zip(&packages).zip(results) {
        items.push(batch_item(plan_item, package, result));
    }
    follow_detection(&mut items, plan, environment, state);
    check_forgotten_receipts(&mut items, &batch, environment.macos_objects);
    for (item, plan_item) in items.iter().zip(&batch_items) {
        // Root's removals are never put back, so a package it removed only
        // in part may still have lost its Audio Unit.
        let something_gone = match &item.message_code {
            UninstallMessage::AdministratorStepFailed { gone, .. } => !gone.is_empty(),
            _ => item.status == UninstallStatus::Removed,
        };
        if !something_gone {
            continue;
        }
        for step in &plan_item.post {
            match step {
                // Asked as the user, after the plug-ins are gone. A refresh
                // that can't happen only means a host may list a plug-in
                // that is gone until it next looks: never a problem to
                // report.
                UninstallPostStep::RefreshAudioUnits {} => {
                    let _ = environment.macos_objects.refresh_audio_units();
                }
                // Only a package whose files go into the backup set resets
                // REAPER's language; none on this route does.
                UninstallPostStep::ResetReaperLangpackIfSelected {} => {}
            }
        }
    }
    // The command ran as one, but the progress page counts packages: each
    // gets its start and its end. Only a package root had steps for is said
    // to have gone through the administrator command; one whose files were
    // all gone before root's turn, with no window at all or with none of the
    // command for it, reads like any other removal.
    for item in &items {
        if matches!(
            item.status,
            UninstallStatus::Removed | UninstallStatus::Failed
        ) {
            let by_root = batch
                .meaning
                .iter()
                .any(|meaning| meaning.package_id() == Some(item.package_id.as_str()));
            progress.report(ProgressEvent::RemoveStarted {
                package_id: item.package_id.clone(),
                method: if by_root {
                    RemovalMethod::Administrator
                } else {
                    RemovalMethod::Files
                },
            });
            progress.report(ProgressEvent::RemoveCompleted {
                package_id: item.package_id.clone(),
                removed: item.status.is_removed(),
            });
        }
    }

    let Some(stopped_at) = items
        .iter()
        .find(|item| item.status != UninstallStatus::Removed)
        .map(|item| item.package_id.clone())
    else {
        return Ok(items);
    };
    // Nothing else has changed yet: the command was the run's first change.
    items.extend(
        to_remove
            .iter()
            .filter(|item| !item.needs_administrator())
            .map(|item| {
                new_item(
                    item,
                    UninstallStatus::NotReached,
                    UninstallMessage::RunStopped {
                        package_id: stopped_at.clone(),
                    },
                )
            }),
    );
    Err(items)
}

/// The packages of `batch_items` that a process check stops, with why.
fn failing_process_checks(
    plan: &UninstallPlan,
    batch_items: &[&UninstallPlanItem],
    environment: &UninstallEnvironment<'_>,
) -> Vec<(String, UninstallMessage)> {
    let ids: Vec<String> = batch_items
        .iter()
        .map(|item| item.package_id.clone())
        .collect();
    let reapers = (environment.reaper_processes)();
    let others = (environment.named_processes)(&blocking_programs(plan, &ids));
    batch_items
        .iter()
        .filter_map(|item| {
            let checks: Vec<PreflightCheck> = process_checks(
                plan,
                std::slice::from_ref(&item.package_id),
                environment.target_app_path.as_deref(),
                &reapers,
                &others,
            )
            .into_iter()
            .filter(|check| check.status == PreflightStatus::Fail)
            .collect();
            (!checks.is_empty()).then(|| {
                (
                    item.package_id.clone(),
                    UninstallMessage::ChecksFailed { checks },
                )
            })
        })
        .collect()
}

/// The run's items after root's first step found a program running, so
/// nothing ran. The process checks, asked again as the user, name it for
/// the packages that need it closed; if it has gone again by then, every
/// package of the command is Blocked with the names root looked for.
fn programs_running_items(
    to_remove: &[&UninstallPlanItem],
    plan: &UninstallPlan,
    batch_items: &[&UninstallPlanItem],
    batch: &AdminBatch,
    environment: &UninstallEnvironment<'_>,
) -> Vec<UninstallReportItem> {
    let mut blocked = failing_process_checks(plan, batch_items, environment);
    if blocked.is_empty() {
        let names = match batch.meaning.first() {
            Some(AdminStepMeaning::ProgramsClosed { names }) => names.join(", "),
            _ => String::new(),
        };
        let check = PreflightCheck {
            name: PROGRAMS_RUNNING_CHECK.to_string(),
            status: PreflightStatus::Fail,
            message: format!(
                "One of these programs was running when the administrator command started, or \
                 it couldn't check for them, so it changed nothing: {names}. Close any that is \
                 open before uninstalling."
            ),
        };
        blocked = batch_items
            .iter()
            .map(|item| {
                (
                    item.package_id.clone(),
                    UninstallMessage::ChecksFailed {
                        checks: vec![check.clone()],
                    },
                )
            })
            .collect();
    }
    blocked_run_items(to_remove, blocked)
}

/// The report item for one package of the command, from what the
/// filesystem says of it.
fn batch_item(
    plan_item: &UninstallPlanItem,
    package: &AdminPackage,
    result: AdminPackageResult,
) -> UninstallReportItem {
    let moved = matches!(package.work, AdminWork::MoveToTrash(_));
    match result {
        AdminPackageResult::Removed {
            gone,
            trashed_to,
            ended_unclearly,
        } => {
            let code = if gone.is_empty() {
                // Everything of it was gone before root's turn: root moved
                // and deleted nothing, so nothing may be said to have gone
                // to the Trash or with administrator rights.
                UninstallMessage::Removed {
                    backup_set_id: None,
                }
            } else if moved {
                UninstallMessage::MovedToTrash {
                    trashed_to: trashed_to.clone(),
                }
            } else {
                UninstallMessage::RemovedAsAdministrator
            };
            let mut item = new_item(plan_item, UninstallStatus::Removed, code);
            item.removed = gone
                .into_iter()
                .map(|path| RemovedFile {
                    path,
                    reason: RemovalReason::Tree,
                    size: 0,
                    changed_since_install: false,
                    backup_path: None,
                    trashed_to: if moved { trashed_to.clone() } else { None },
                })
                .collect();
            if let Some(status) = ended_unclearly {
                item.problems
                    .push(UninstallProblem::AdministratorEndedUnclearly { status });
            }
            item
        }
        AdminPackageResult::Failed {
            path,
            ended_unclearly,
            gone,
            left,
            refused_by_macos,
            detail,
        } => new_item(
            plan_item,
            UninstallStatus::Failed,
            UninstallMessage::AdministratorStepFailed {
                path,
                ended_unclearly,
                gone,
                left,
                refused_by_macos,
                detail,
            },
        ),
        AdminPackageResult::TrashUnclear { path } => new_item(
            plan_item,
            UninstallStatus::Failed,
            UninstallMessage::TrashUnclear { path },
        ),
        AdminPackageResult::NotReached { stopped_at } => new_item(
            plan_item,
            UninstallStatus::NotReached,
            UninstallMessage::RunStopped {
                package_id: stopped_at,
            },
        ),
        // Handled before any item is made: nothing ran.
        AdminPackageResult::ProgramRunning => new_item(
            plan_item,
            UninstallStatus::Blocked,
            UninstallMessage::ChecksFailed { checks: Vec::new() },
        ),
    }
}

/// Every package root worked on, removed or not, keeps its receipt entry
/// only while fresh detection still finds it. The receipt is saved once;
/// a save that fails is a problem of each package it concerns, which
/// stays removed: what root did can't be undone.
fn follow_detection(
    items: &mut [UninstallReportItem],
    plan: &UninstallPlan,
    environment: &UninstallEnvironment<'_>,
    state: &mut InstallState,
) {
    let detections = (environment.detect)().ok();
    let mut changed = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if !matches!(
            item.status,
            UninstallStatus::Removed | UninstallStatus::Failed
        ) {
            continue;
        }
        let still_installed = detections.as_ref().map(|detections| {
            detections
                .iter()
                .any(|detection| detection.package_id == item.package_id && detection.installed)
        });
        if receipt_follows_detection(state, &item.package_id, still_installed) {
            changed.push(index);
        }
    }
    if changed.is_empty() {
        return;
    }
    if let Err(error) = save_install_state(&plan.resource_path, state) {
        for index in changed {
            items[index]
                .problems
                .push(UninstallProblem::ReceiptNotSaved {
                    error: error.to_string(),
                });
        }
    }
}

/// List the installer's receipts again, as the user, and note on each
/// removed package any receipt root was told to forget that is still
/// listed, or that the list couldn't be read.
fn check_forgotten_receipts(
    items: &mut [UninstallReportItem],
    batch: &AdminBatch,
    objects: &dyn MacosObjects,
) {
    let forgotten = |package_id: &str| -> Vec<String> {
        batch
            .meaning
            .iter()
            .filter_map(|meaning| match meaning {
                AdminStepMeaning::Forget {
                    package_id: id,
                    id: receipt,
                } if id == package_id => Some(receipt.clone()),
                _ => None,
            })
            .collect()
    };
    let mut listed = None;
    for item in items
        .iter_mut()
        .filter(|item| item.status == UninstallStatus::Removed)
    {
        let sent = forgotten(&item.package_id);
        if sent.is_empty() {
            continue;
        }
        match listed.get_or_insert_with(|| objects.installed_package_ids()) {
            Ok(listed) => {
                let ids: Vec<String> = sent.into_iter().filter(|id| listed.contains(id)).collect();
                if !ids.is_empty() {
                    item.problems
                        .push(UninstallProblem::InstallerRecordsRemain { ids });
                }
            }
            Err(error) => item
                .problems
                .push(UninstallProblem::InstallerRecordsUnchecked {
                    error: error.to_string(),
                }),
        }
    }
}

/// What a dry run records of the administrator command: the steps a real
/// run would give root, built from the same checks, and never run.
/// `outcomes` holds the dry run's items, at the same index as `to_remove`;
/// a package whose checks refuse it there becomes Blocked, as the real run
/// would stop at it.
pub(super) fn dry_run_administrator_steps(
    to_remove: &[&UninstallPlanItem],
    outcomes: &mut [UninstallReportItem],
    environment: &UninstallEnvironment<'_>,
) -> Vec<AdministratorStepRecord> {
    let at = SystemTime::now();
    let mut packages = Vec::new();
    let mut in_batch = Vec::new();
    for (index, item) in to_remove.iter().enumerate() {
        if !item.needs_administrator()
            || outcomes
                .get(index)
                .is_none_or(|outcome| outcome.status != UninstallStatus::WouldRemove)
        {
            continue;
        }
        match prepare_package(item, environment.roots, environment.macos_objects, at) {
            Ok(package) => {
                packages.push(package);
                in_batch.push(index);
            }
            Err(code) => block(&mut outcomes[index], code),
        }
    }
    if packages.is_empty() {
        return Vec::new();
    }
    let listed = listed_receipts(&packages, environment.macos_objects);
    match administrator_steps(
        &packages,
        listed.as_ref().and_then(|listed| listed.as_ref().ok()),
    ) {
        // A real run asks for no password when root has nothing left to do
        // for any package, so it would give root nothing at all.
        Ok(batch)
            if !batch
                .meaning
                .iter()
                .any(|meaning| meaning.package_id().is_some()) =>
        {
            Vec::new()
        }
        Ok(batch) => batch.records(|_| AdministratorStepVerdict::WouldRun),
        Err(error) => {
            for index in in_batch {
                block(
                    &mut outcomes[index],
                    UninstallMessage::ElevationFailed {
                        error: error.clone(),
                    },
                );
            }
            Vec::new()
        }
    }
}

/// Mark a dry run's item as stopped by `code`.
fn block(item: &mut UninstallReportItem, code: UninstallMessage) {
    item.status = UninstallStatus::Blocked;
    item.message_code = code;
    item.removed.clear();
}
