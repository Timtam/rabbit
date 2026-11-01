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
//! checked right before each file moves.
//!
//! Nothing here removes anything yet.

pub mod paths;
pub mod plan;
pub mod preflight;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{InstallationKind, Platform};
use crate::package::uninstall::{
    UninstallBackup, UninstallPostStep, UninstallStrategy, UninstallSummary,
};
use crate::version::Version;

pub use paths::{PathRefusal, RemovalGuard, UninstallRoots, check_removal};
pub use plan::{UninstallPlanInput, build_uninstall_plan};
pub use preflight::{
    ANY_REAPER_PROCESS_CHECK, PROGRAMS_RUNNING_CHECK, UninstallPreflightOptions,
    run_uninstall_preflight, run_uninstall_preflight_with_processes,
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
