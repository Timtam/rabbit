//! Uninstalling packages: a pipeline of its own, beside install.
//!
//! Uninstall doesn't reuse install's plan. An install plan action always has
//! something to download, and a "remove" action would need a meaning in every
//! place that matches on install actions. Instead this module builds an
//! [`UninstallPlan`] from what detection found, what the receipt records and
//! what each package's manifest says about coming off again. The engine
//! that carries a plan out calls the shared pieces (the lock, the REAPER
//! check, backups, receipts) itself.
//!
//! RABBIT never deletes everything a receipt lists: a receipt can hold the
//! user's key map, a portable `reaper.ini` and whole folders. A package's
//! files are only two kinds of thing:
//! - regular files RABBIT wrote, meaning receipt entries with a hash;
//! - files the package's manifest names, or matches by its plug-in name.
//!
//! The package's settings go too only when the user asks for that ("Also
//! remove settings"), and only those its manifest lists.
//!
//! Building a plan reads only whether files exist and how big they are. It
//! never hashes them: the wizard builds its rows on the UI thread, and
//! hashing FFmpeg's 250 MB there used to stall it for seconds. Hashes are
//! checked right before each file moves, by [`execute_uninstall`] on the
//! worker thread.
//!
//! Removed files move into a backup set, so every removal can be undone one
//! package at a time.

pub mod execute;
#[cfg(test)]
mod execute_tests;
pub mod paths;
pub mod plan;
mod post;
pub mod preflight;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{InstallationKind, Platform};
use crate::package::uninstall::{
    UninstallBackup, UninstallPostStep, UninstallStrategy, UninstallSummary,
};
use crate::preflight::{PreflightCheck, PreflightReport};
use crate::report::{SavedReportPaths, default_report_path, save_json_and_text_reports};
use crate::version::Version;

pub use execute::execute_uninstall;
pub use paths::{PathRefusal, RemovalGuard, UninstallRoots, check_removal};
pub use plan::{UninstallPlanInput, build_uninstall_plan};
pub use preflight::{
    ANY_REAPER_PROCESS_CHECK, BACKUP_SPACE_CHECK, BACKUP_SPACE_MARGIN, PROGRAMS_RUNNING_CHECK,
    UninstallPreflightOptions, run_uninstall_preflight, run_uninstall_preflight_with_processes,
};

/// The note key a package without any plan for this target shows, the same
/// one manifests use while a package's turn hasn't come.
pub const UNINSTALL_NOT_YET_KEY: &str = "uninstall-not-yet";

/// What uninstalling would do for every package on one REAPER target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallPlan {
    pub resource_path: PathBuf,
    pub platform: Platform,
    pub target: InstallationKind,
    /// "Also remove settings" was asked for: each removable package's
    /// `kept_user_data` is in its `files` instead of its `kept` list.
    pub remove_settings: bool,
    /// One item per package offered on this platform, in manifest order. Use
    /// [`UninstallPlan::in_removal_order`] for the order to remove them in.
    pub items: Vec<UninstallPlanItem>,
}

impl UninstallPlan {
    pub fn item(&self, package_id: &str) -> Option<&UninstallPlanItem> {
        self.items.iter().find(|item| item.package_id == package_id)
    }

    /// The items for `package_ids`, in the order to remove them: every
    /// package before the packages it depends on, so extensions go before
    /// REAPER. Packages with no dependency between them keep manifest order.
    pub fn in_removal_order(&self, package_ids: &[String]) -> Vec<&UninstallPlanItem> {
        plan::removal_order(
            self.items
                .iter()
                .filter(|item| package_ids.contains(&item.package_id))
                .collect(),
        )
    }
}

/// What uninstalling one package would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallPlanItem {
    pub package_id: String,
    pub display_name: String,
    pub installed_version: Option<Version>,
    /// RABBIT's receipt has an entry for the package.
    pub has_receipt: bool,
    /// The packages this one installs on top of, from its manifest.
    pub depends_on: Vec<String>,
    pub removability: Removability,
    /// The strategy of the plan that applies, if one does.
    pub strategy: Option<UninstallStrategy>,
    pub backup: UninstallBackup,
    /// The two sentences Review shows.
    pub summary: Option<UninstallSummary>,
    /// What goes. Empty unless the package can be removed.
    pub files: Vec<PlannedRemoval>,
    /// Folders removed after the files, and only if empty by then.
    pub dirs_if_empty: Vec<PathBuf>,
    /// What stays, and why.
    pub kept: Vec<KeptPath>,
    pub post: Vec<UninstallPostStep>,
    /// Programs that must not be running, from the manifest.
    pub blocking_programs: Vec<String>,
    /// No REAPER at all may run, not only the target's: a vendor
    /// uninstaller reacts to any REAPER window, and a machine-wide package
    /// may be loaded by any REAPER.
    pub every_reaper_closed: bool,
    pub requires_elevation: bool,
    /// Something a backup set can't undo: a vendor uninstaller, a removal
    /// that needs administrator rights, or a whole folder going to the Trash
    /// or away for good.
    pub unbacked: bool,
    /// Ask a Yes/No question, with No focused, before starting: a warning
    /// asks for it, or the removal can't be undone from a backup.
    pub confirm: bool,
    pub warnings: Vec<UninstallWarning>,
    pub notes: Vec<UninstallNote>,
}

/// Whether a package can be uninstalled, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Removability {
    Removable,
    /// Removable, though ReaPack once installed (some of) its files.
    /// ReaPack itself is gone, so its records can't go out of date behind
    /// its back any more.
    RemovableOrphanedFromReaPack,
    NotInstalled,
    /// ReaPack manages the package. Removing a file behind its back would
    /// leave its records wrong, so the user removes it in ReaPack.
    ManagedByReaPack {
        path: PathBuf,
        reapack_package: String,
    },
    /// RABBIT couldn't check ReaPack's records for a file, so it treats the
    /// file as ReaPack's.
    ReaPackCheckFailed {
        path: PathBuf,
        reason: String,
    },
    /// A machine-wide package, which a portable target can't remove.
    NeedsStandardTarget,
    /// The manifest only allows removing copies RABBIT installed, and there
    /// is no receipt: this copy is most likely the user's own.
    NoReceipt,
    /// No vendor uninstaller RABBIT can trust was found.
    NoTrustedUninstaller,
    Manual {
        note_key: String,
    },
    NotSupported {
        note_key: String,
    },
    /// The guard refused a path the package would remove.
    UnsafePath {
        path: PathBuf,
        refusal: PathRefusal,
    },
}

impl Removability {
    pub fn is_removable(&self) -> bool {
        matches!(self, Self::Removable | Self::RemovableOrphanedFromReaPack)
    }

    /// Installed, but RABBIT won't remove it. A requested package in this
    /// state makes `--apply` change nothing.
    pub fn is_blocked(&self) -> bool {
        !self.is_removable() && *self != Self::NotInstalled
    }
}

/// One file or folder the plan would remove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedRemoval {
    pub path: PathBuf,
    pub reason: RemovalReason,
    /// The size while planning. `None` for a folder.
    pub size: Option<u64>,
    pub is_dir: bool,
    /// The hash the receipt recorded, if it lists this file. Checked right
    /// before the move: a file only the receipt names that changed since is
    /// kept; one the manifest names goes anyway, after its backup.
    pub receipt_sha256: Option<String>,
    /// What the guard checks again right before the move.
    #[serde(skip)]
    pub guard: RemovalGuard,
}

impl PlannedRemoval {
    /// Run the guard again, as the engine does right before moving the path.
    pub fn check(&self, roots: &UninstallRoots) -> Result<(), PathRefusal> {
        check_removal(roots, &self.path, &self.guard)
    }
}

/// Why a path is in the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemovalReason {
    /// Only RABBIT's receipt lists it, with a hash.
    Receipt,
    /// The manifest's `remove.paths` names it.
    ManifestPath,
    /// Its name matches the package's plug-in prefixes and suffixes.
    MatchingUserPlugin,
    /// A whole folder from the manifest's `remove.trees`.
    Tree,
    /// The package's settings, removed because "Also remove settings" was
    /// asked for.
    Setting,
}

impl RemovalReason {
    /// Whether the manifest vouches for the path, so it goes even if it
    /// changed since RABBIT installed it.
    pub fn named_by_manifest(self) -> bool {
        !matches!(self, Self::Receipt)
    }
}

/// Something that stays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptPath {
    pub path: PathBuf,
    pub reason: KeptReason,
    /// The manifest's note on what this is, such as "your grooves".
    pub note_key: Option<String>,
    pub exists: bool,
    /// "Also remove settings" would take it. Only for the package's own
    /// data that exists and passes the guard; never for a folder root,
    /// REAPER's configuration or anything on the keep list.
    pub removable_with_settings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeptReason {
    /// The package's settings or other data, from `kept_user_data`.
    UserData,
    /// A whole folder such as the resource folder: shown as kept, never
    /// removed.
    FolderRoot,
    /// REAPER's own configuration, `reaper.ini` and `reaper-kb.ini`, and
    /// copies of them. Never removed.
    ReaperConfiguration,
    /// A key map, or the folder of them, listed in `kept_user_data`. Never
    /// taken by "Also remove settings".
    KeyMap,
    /// The plan's `keep` list names it. Never removed.
    KeepList,
    /// In RABBIT's own folder. Never removed.
    RabbitFolder,
    /// The receipt lists it, but it is outside the resource folder and no
    /// manifest path covers it. A receipt is a file anyone can edit, so it
    /// can't point RABBIT somewhere on its own.
    OutsideResourceFolder,
}

/// Something to tell the user before they decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UninstallWarning {
    /// The manifest's `always_warn`, such as OSARA's warning that REAPER
    /// stops speaking.
    Always { message_key: String, confirm: bool },
    /// The manifest's `warn_when_installed`, while that package is installed.
    OtherPackageInstalled {
        package_id: String,
        message_key: String,
    },
    /// Installed packages that install on top of this one stay, and may stop
    /// working.
    DependentsStay { package_ids: Vec<String> },
    /// The package serves more than this REAPER: every user's or every
    /// program's copy goes.
    MachineWide,
    /// Removing ReaPack: these packages, installed with ReaPack, keep their
    /// files in UserPlugins, and nothing keeps them up to date afterwards.
    ReaPackStillManages { reapack_packages: Vec<String> },
    /// Removing ReaPack, but its records couldn't be read to say what it
    /// still manages.
    ReaPackRecordsUnreadable { reason: String },
}

/// A neutral note about how the package comes off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UninstallNote {
    /// RABBIT has no receipt for this copy. It removes the files the
    /// manifest names.
    NoReceipt,
    /// ReaPack once installed these files. ReaPack is gone, so RABBIT
    /// removes them, and ReaPack's records stay. Empty when RABBIT couldn't
    /// read which package the records name.
    OrphanedFromReaPack { reapack_packages: Vec<String> },
    /// The plan's `note_key`: a note on what stays behind.
    Manifest { note_key: String },
}

/// What to uninstall, and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallRequest {
    pub resource_path: PathBuf,
    pub platform: Platform,
    /// The packages to remove. The engine puts them in removal order itself.
    pub package_ids: Vec<String>,
    /// "Also remove settings": each package's `kept_user_data` moves into
    /// the backup set too. Off unless the user asks for it.
    pub remove_settings: bool,
    /// Change nothing: plan, check and hash, and report what would happen.
    pub dry_run: bool,
    /// The target's REAPER, when the caller knows it better than detection.
    pub target_app_path: Option<PathBuf>,
    /// A standard or a portable REAPER. `None` works it out: the standard
    /// resource folder is the standard target, any other folder a portable
    /// one, which can't remove packages that serve the whole computer.
    pub target: Option<InstallationKind>,
}

/// How uninstalling one package went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UninstallStatus {
    /// Its files are gone, into the backup set.
    Removed,
    /// Removed, but detection still finds some of it afterwards.
    RemovedWithLeftovers,
    /// Removed; some files go at the next restart. Only a vendor uninstaller
    /// leaves that, and none runs yet.
    RemovedPendingRestart,
    /// A dry run: this is what would be removed.
    WouldRemove,
    /// Not installed, so there was nothing to do.
    NotInstalled,
    /// RABBIT won't remove it, or a check stopped it before anything of it
    /// changed.
    Blocked,
    /// Removing it went wrong. Whatever had moved was put back.
    Failed,
    /// The user stopped the run before this package.
    Cancelled,
    /// An earlier package stopped the run before this one.
    NotReached,
}

impl UninstallStatus {
    /// The package's files are gone.
    pub fn is_removed(self) -> bool {
        matches!(
            self,
            Self::Removed | Self::RemovedWithLeftovers | Self::RemovedPendingRestart
        )
    }
}

/// What an uninstall run did, or in a dry run would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallReport {
    pub resource_path: PathBuf,
    pub platform: Platform,
    pub target: InstallationKind,
    pub dry_run: bool,
    pub remove_settings: bool,
    pub preflight: PreflightReport,
    /// The backup set the removed files went into, when anything did.
    pub backup_set_id: Option<String>,
    pub backup_set_path: Option<PathBuf>,
    /// One item per requested package, in the order they were removed.
    pub items: Vec<UninstallReportItem>,
    /// The plan the run carried out, built from fresh detection once the
    /// run held the lock.
    pub plan: UninstallPlan,
    /// Where the run saved this report, when it saved it.
    #[serde(skip)]
    pub saved_report: Option<SavedReportPaths>,
}

impl UninstallReport {
    pub fn item(&self, package_id: &str) -> Option<&UninstallReportItem> {
        self.items.iter().find(|item| item.package_id == package_id)
    }

    /// Some package's files are gone.
    pub fn changed_anything(&self) -> bool {
        self.items.iter().any(|item| item.status.is_removed())
    }

    /// A requested package was blocked and nothing was removed: the run
    /// changed nothing. The CLI exits with 2.
    pub fn blocked_before_changes(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.status == UninstallStatus::Blocked)
            && !self.changed_anything()
            && !self.items.iter().any(|item| {
                matches!(
                    item.status,
                    UninstallStatus::Failed | UninstallStatus::Cancelled
                )
            })
    }

    /// A package failed or was stopped, or the run stopped after it had
    /// already removed something. The CLI exits with 1.
    pub fn has_failures(&self) -> bool {
        self.items.iter().any(|item| {
            matches!(
                item.status,
                UninstallStatus::Failed | UninstallStatus::Cancelled
            )
        }) || (self.changed_anything()
            && self.items.iter().any(|item| {
                matches!(
                    item.status,
                    UninstallStatus::Blocked | UninstallStatus::NotReached
                )
            }))
    }

    /// The user stopped the run before every package was reached.
    pub fn was_cancelled(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.status == UninstallStatus::Cancelled)
    }

    /// Packages were removed, but something needs the user's attention:
    /// leftovers, or a step after the removal that didn't work.
    pub fn finished_with_problems(&self) -> bool {
        self.items
            .iter()
            .any(|item| !item.problems.is_empty() || !item.leftovers.is_empty())
    }
}

/// Save `report` as `RABBIT/logs/uninstall-unix-<time>.json` and `.txt`.
pub fn save_uninstall_report(report: &UninstallReport) -> crate::Result<SavedReportPaths> {
    save_json_and_text_reports(
        &default_report_path(&report.resource_path, "uninstall"),
        report,
    )
}

/// How uninstalling one package went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallReportItem {
    pub package_id: String,
    pub display_name: String,
    pub status: UninstallStatus,
    /// What is gone from its place, into the backup set; in a dry run, what
    /// would be.
    pub removed: Vec<RemovedFile>,
    /// What the plan listed but the run kept, and why.
    pub kept: Vec<KeptFile>,
    /// Folders removed because the removal left them empty.
    pub removed_dirs: Vec<PathBuf>,
    /// After a failure: files moved back where they were.
    pub put_back: Vec<PathBuf>,
    /// After a failure: files that couldn't be moved back. They are still
    /// in the backup set, and a restore brings them back.
    pub left_in_backup: Vec<PathBuf>,
    /// What detection still finds of the package after the run.
    pub leftovers: Vec<PathBuf>,
    /// Steps after the removal that didn't work. The package is removed
    /// all the same.
    pub problems: Vec<UninstallProblem>,
    /// English, for the CLI and the saved report. A UI in another language
    /// goes by `message_code`.
    pub message: String,
    pub message_code: UninstallMessage,
}

/// A file the run removed into the backup set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedFile {
    pub path: PathBuf,
    pub reason: RemovalReason,
    pub size: u64,
    /// The receipt recorded another hash. Only files the manifest names go
    /// even then, such as a newer plug-in copied in by hand.
    pub changed_since_install: bool,
    /// Where it is in the backup set. `None` in a dry run.
    pub backup_path: Option<PathBuf>,
}

/// A file the plan listed that the run kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptFile {
    pub path: PathBuf,
    pub reason: KeptFileReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeptFileReason {
    /// Only RABBIT's receipt names it, and it changed since RABBIT installed
    /// it, so it is most likely the user's own now.
    ChangedSinceInstall,
    /// A settings folder holding a symbolic link or junction, which RABBIT
    /// never follows. The whole folder stays.
    HoldsLink,
}

/// Something after the removal that didn't work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum UninstallProblem {
    /// REAPER's language setting still names the language pack that was
    /// removed.
    LanguageResetFailed { error: String },
}

impl UninstallProblem {
    /// English, for the CLI and the saved report.
    pub fn message(&self) -> String {
        match self {
            Self::LanguageResetFailed { error } => format!(
                "RABBIT couldn't set REAPER back to no language pack ({error}). Choose a \
                 language in REAPER's preferences."
            ),
        }
    }
}

/// What happened to a package, for a UI to put in its own language. The
/// English form is the item's `message`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum UninstallMessage {
    /// Removed. `backup_set_id` is the set its files went into.
    Removed {
        backup_set_id: Option<String>,
    },
    WouldRemove,
    NotInstalled,
    /// No package with this id is offered on this platform.
    UnknownPackage,
    /// The plan says RABBIT won't remove it; `removability` says why.
    NotRemovable {
        removability: Removability,
    },
    /// The removal needs a step this version can't carry out yet: a vendor
    /// uninstaller, administrator rights, or a whole folder going to the
    /// Trash. None of these can be undone from a backup.
    NotYetSupported,
    /// Checks failed for this package: REAPER or a program it names is
    /// running, the resource folder isn't writable, or the backup lacks
    /// room.
    ChecksFailed {
        checks: Vec<PreflightCheck>,
    },
    /// Other requested packages are blocked, so the run removed nothing.
    OtherPackageBlocked {
        package_ids: Vec<String>,
    },
    Cancelled,
    /// The run stopped at `package_id`, before this package.
    RunStopped {
        package_id: String,
    },
    /// The guard refused a path right before it moved.
    PathRefused {
        path: PathBuf,
        refusal: PathRefusal,
    },
    /// A file couldn't be read to check its hash, or a settings folder
    /// couldn't be listed.
    Unreadable {
        path: PathBuf,
        error: String,
    },
    MoveFailed {
        path: PathBuf,
        error: String,
    },
    /// Copied to the backup's drive, but the original couldn't be deleted,
    /// most often because another program has it open.
    FileInUse {
        path: PathBuf,
        error: String,
    },
    /// The backup set couldn't be written.
    BackupFailed {
        error: String,
    },
    /// The receipt couldn't be saved after the files moved, so they were
    /// put back.
    ReceiptSaveFailed {
        error: String,
    },
}

/// The part of `path` below the resource folder, with `/` between
/// components, as ReaPack records paths. `None` for a path outside it.
pub(crate) fn resource_relative(resource: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(resource).ok()?;
    let parts: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}
