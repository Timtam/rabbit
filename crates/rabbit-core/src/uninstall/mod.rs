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
//!
//! Two packages come off through their vendor's own uninstaller instead, on
//! Windows: Surge XT and a standard REAPER. That needs administrator rights,
//! so a run that removes either starts one hidden elevated helper before the
//! first change, and the helper runs both uninstallers when the run reaches
//! them; see [`helper`] and [`vendor`]. Nothing a vendor's uninstaller
//! removes goes into a backup.
//!
//! On macOS a plan's `trees`, whole folders such as app bundles, never go
//! into a backup either. Each package's [`RemovalRoute`] says how it leaves:
//! into the backup set, through a vendor's uninstaller, to the Trash, or
//! through the run's one administrator command. Which way REAPER.app goes is
//! decided while planning, from what this account may do with it; see
//! [`macos_objects`]. The administrator command is the run's first change,
//! behind its one password window; see [`admin_batch`].

pub mod admin_batch;
#[cfg(test)]
mod admin_batch_tests;
#[cfg(test)]
mod admin_run_tests;
pub mod execute;
#[cfg(test)]
mod execute_tests;
pub mod helper;
pub mod macos_objects;
#[cfg(test)]
mod macos_plan_tests;
#[cfg(test)]
mod manifest_tests;
pub mod paths;
pub mod plan;
mod post;
pub mod preflight;
mod registry;
#[cfg(test)]
mod tests;
pub mod vendor;
#[cfg(test)]
mod vendor_run_tests;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use rabbit_platform::HkcuUninstallRoot;
use serde::{Deserialize, Serialize};

use crate::model::{InstallationKind, Platform};
use crate::package::uninstall::{
    UninstallBackup, UninstallPostStep, UninstallStrategy, UninstallSummary,
};
use crate::preflight::{PreflightCheck, PreflightReport};
use crate::report::{SavedReportPaths, default_report_path, save_json_and_text_reports};
use crate::version::Version;

pub use admin_batch::{
    AdminShellEvent, AdminShellRun, AdministratorShell, AdministratorStepKind,
    AdministratorStepRecord, AdministratorStepVerdict, NoAdministratorShell,
    RecordingAdministratorShell, SystemAdministratorShell,
};
pub use execute::{APP_MANAGEMENT_HINT, execute_uninstall, plan_uninstall_at};
pub use helper::{
    ElevatedHelper, ElevatedRunRequest, ElevatedSession, ElevationStartError, HELPER_SUBCOMMAND,
    HelperArgs, InProcessElevatedHelper, MachineFolders, NoElevatedHelper, SystemElevatedHelper,
    run_elevated_helper,
};
pub use macos_objects::{
    FakeMacosObjects, FakeReceipts, MacosObjects, NoMacosObjects, ObjectFacts, ObjectProbe,
    SystemMacosObjects, TrashError, TrashErrorKind, TrashFolderFacts, TreeRoute, tree_route,
};
pub use paths::{
    PathRefusal, RemovalGuard, SYSTEM_ROOT, UninstallRoots, check_administrator_removal,
    check_administrator_trash_move, check_link_removal, check_removal,
};
pub use plan::{UninstallPlanInput, build_uninstall_plan};
pub use preflight::{
    ANY_REAPER_PROCESS_CHECK, BACKUP_SPACE_CHECK, BACKUP_SPACE_MARGIN, PROGRAMS_RUNNING_CHECK,
    UninstallPreflightOptions, run_uninstall_preflight, run_uninstall_preflight_with_processes,
};
pub(crate) use registry::{points_into, program_named_by};
pub use vendor::{
    LeftoverKind, LeftoverOutcome, LeftoverResult, NothingListed, PlannedVendorRun,
    SystemVendorLookup, VendorEnvironment, VendorEvent, VendorJob, VendorLookup, VendorOutcome,
    VendorProblem, VendorRefusal, VendorStatus, plan_vendor_run, run_vendor_uninstaller,
};

/// The note key a package without any plan for this target shows, the same
/// one manifests use while a package's turn hasn't come.
pub const UNINSTALL_NOT_YET_KEY: &str = "uninstall-not-yet";

/// What an uninstall run reads or starts beyond the files of the target:
/// where each vendor's uninstaller is, what starts the run's elevated
/// helper (and with it the run's one Windows permission prompt), Windows'
/// per-user list of installed programs, what macOS says this account may do
/// with an app and what the run does there as the user, and what runs the
/// macOS administrator command (and with it the run's one password window).
///
/// Only RABBIT's own entry points use [`UninstallServices::system`]. Tests
/// hand in stand-ins, so none of them can reach Windows' real lists, the
/// real helper, the real Trash or a real password window; the real ones
/// refuse to start from anything not named like RABBIT anyway.
#[derive(Clone, Copy)]
pub struct UninstallServices<'a> {
    /// Finds and checks each vendor's uninstaller before the run, read only.
    pub vendor_lookup: &'a dyn VendorLookup,
    /// Starts the elevated helper once per run, when a package needs it.
    pub elevated_helper: &'a dyn ElevatedHelper,
    /// Windows' per-user list of installed programs, where a plan's
    /// `registry_cleanup` names entries. `None` leaves every entry alone,
    /// unread.
    pub installed_programs: Option<&'a HkcuUninstallRoot>,
    /// What this account may do with a macOS app, which decides how a
    /// `to_trash` plan's app leaves, and what the run does there as the
    /// user: the Trash, the installer's receipts, the Audio Unit list, and
    /// whether RABBIT runs as root.
    pub macos_objects: &'a dyn MacosObjects,
    /// Runs the macOS administrator command once per run, when a package
    /// needs it.
    pub administrator_shell: &'a dyn AdministratorShell,
}

/// Windows' real per-user list of installed programs.
static INSTALLED_PROGRAMS: LazyLock<HkcuUninstallRoot> =
    LazyLock::new(HkcuUninstallRoot::installed_programs);

impl UninstallServices<'static> {
    /// The real ones, for RABBIT's own entry points on `platform`: HKLM's
    /// list of programs installed for every user, read only, RABBIT's own
    /// elevated helper, on Windows the per-user list of installed programs,
    /// and macOS itself with its administrator password window.
    pub fn system(platform: Platform) -> Self {
        Self {
            vendor_lookup: &SystemVendorLookup,
            elevated_helper: &SystemElevatedHelper,
            installed_programs: (platform == Platform::Windows).then(|| &*INSTALLED_PROGRAMS),
            macos_objects: &SystemMacosObjects,
            administrator_shell: &SystemAdministratorShell,
        }
    }

    /// None at all: no vendor uninstaller is found, no helper starts, no
    /// list of installed programs is read, nothing is asked about a macOS
    /// app or moved to the Trash, and no administrator command runs. For
    /// tests that remove files only.
    pub fn without_elevation() -> Self {
        Self {
            vendor_lookup: &NothingListed,
            elevated_helper: &NoElevatedHelper,
            installed_programs: None,
            macos_objects: &NoMacosObjects,
            administrator_shell: &NoAdministratorShell,
        }
    }
}

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

    /// The items for `package_ids` in the order a run takes them, which is
    /// the order its report lists them in. On macOS the run's one
    /// administrator command is its first change, so the packages it removes
    /// come first, in their removal order, REAPER among them when only root
    /// can move its app. The rest follow in removal order, REAPER last. A
    /// run without such packages, as on Windows, takes
    /// [`in_removal_order`](Self::in_removal_order) as it is.
    pub fn in_run_order(&self, package_ids: &[String]) -> Vec<&UninstallPlanItem> {
        let (administrator, rest): (Vec<&UninstallPlanItem>, Vec<&UninstallPlanItem>) = self
            .in_removal_order(package_ids)
            .into_iter()
            .partition(|item| item.needs_administrator());
        administrator.into_iter().chain(rest).collect()
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
    /// Entries in Windows' per-user list of installed programs that go
    /// once the files have, each only if it still points into this REAPER.
    /// Empty unless the package can be removed.
    pub registry_cleanup: Vec<PlannedRegistryCleanup>,
    /// What the vendor's uninstaller would do, for a `vendor_uninstaller`
    /// plan whose uninstaller RABBIT found and trusts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_run: Option<PlannedVendorRun>,
    /// Programs that must not be running, from the manifest.
    pub blocking_programs: Vec<String>,
    /// macOS installer receipts the run's administrator command forgets
    /// once the package's files are gone, from the manifest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macos_pkg_forget: Vec<String>,
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
    /// A machine-wide package on a portable target. Its plan removes
    /// something, which only a run against the standard REAPER may do. A
    /// machine-wide package whose plan removes nothing reports that plan's
    /// verdict on every target instead.
    NeedsStandardTarget,
    /// The manifest only allows removing copies RABBIT installed, and there
    /// is no receipt: this copy is most likely the user's own.
    NoReceipt,
    /// No vendor uninstaller RABBIT can trust was found; `refusal` says
    /// what was wrong. The user can still uninstall it in Windows Settings.
    NoTrustedUninstaller {
        refusal: VendorRefusal,
    },
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
    /// macOS: RABBIT can't move this app to the Trash, by itself or through
    /// the run's administrator command, so the user moves it in Finder.
    ByHand {
        path: PathBuf,
        reason: ByHandReason,
    },
}

/// Why the user moves a macOS app to the Trash in Finder rather than RABBIT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ByHandReason {
    /// This account may not move it: it belongs to another account or to
    /// the system, or its folder can't be changed by the user. Root may
    /// move it only from `/Applications`, and only for a plan that allows
    /// that.
    NotMovableByYou,
    /// It is Locked in Finder (or carries another flag that stops any
    /// move), which not even root can move past.
    Locked,
    /// Its bundle identifier isn't the one the manifest names, or it isn't
    /// an app bundle at all.
    NotTheExpectedApp,
    /// What this account may do with it couldn't be read.
    CouldNotCheck { error: String },
    /// Only root could move it, but the user's Trash isn't a real folder of
    /// the user's, or isn't there.
    NoTrashFolder,
    /// Only root could move it, but it is on another volume than the user's
    /// Trash, where root's move wouldn't be a plain rename.
    NotOnTrashVolume,
}

impl ByHandReason {
    /// Why, in English, as the end of a sentence that begins "because".
    pub fn message(&self) -> String {
        match self {
            Self::NotMovableByYou => "your account may not move it: it belongs to another \
                                      account or to macOS, or sits in a folder you can't change"
                .to_string(),
            Self::Locked => "it is locked".to_string(),
            Self::NotTheExpectedApp => "it isn't the app RABBIT expects there".to_string(),
            Self::CouldNotCheck { error } => {
                format!("RABBIT couldn't check who may move it ({error})")
            }
            Self::NoTrashFolder => {
                "your Trash isn't a folder of your own that RABBIT can check".to_string()
            }
            Self::NotOnTrashVolume => "it is on another disk than your Trash".to_string(),
        }
    }
}

/// How a package leaves, derived from its plan item. Each route is a
/// different way of removing, with its own steps and its own way back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalRoute {
    /// Its files move into the run's backup set, so a restore can bring
    /// them back.
    BackupSet,
    /// Its vendor's own uninstaller removes it, on Windows, through the
    /// run's elevated helper.
    Vendor,
    /// Its trees go to the Trash on macOS, moved as the user.
    Trash,
    /// The run's one administrator command on macOS removes it: it deletes
    /// the package's trees for good, or moves an app RABBIT can't move
    /// itself into the user's Trash.
    Administrator,
}

impl RemovalRoute {
    /// Whether the route takes each tree as one whole object, such as an
    /// app bundle: never listed, hashed or moved file by file.
    pub fn takes_whole_objects(self) -> bool {
        matches!(self, Self::Trash | Self::Administrator)
    }
}

impl UninstallPlanItem {
    /// The package's own uninstaller removes it, through RABBIT's elevated
    /// helper: a `vendor_uninstaller` plan RABBIT found an uninstaller for
    /// and that needs administrator rights.
    pub fn needs_elevated_helper(&self) -> bool {
        self.strategy == Some(UninstallStrategy::VendorUninstaller)
            && self.vendor_run.is_some()
            && self.requires_elevation
    }

    /// How the package leaves. Derived from the plan item, so it can't
    /// disagree with it:
    /// - a `vendor_uninstaller` plan: its vendor's uninstaller;
    /// - a `remove_files` plan that needs administrator rights: the run's
    ///   administrator command, REAPER.app included when only root can move
    ///   it;
    /// - a `remove_files` plan whose trees go to the Trash: the Trash;
    /// - anything else: the backup set.
    pub fn route(&self) -> RemovalRoute {
        match self.strategy {
            Some(UninstallStrategy::VendorUninstaller) => RemovalRoute::Vendor,
            Some(UninstallStrategy::RemoveFiles) if self.requires_elevation => {
                RemovalRoute::Administrator
            }
            Some(UninstallStrategy::RemoveFiles) if self.backup == UninstallBackup::ToTrash => {
                RemovalRoute::Trash
            }
            _ => RemovalRoute::BackupSet,
        }
    }

    /// The run's one administrator command on macOS removes the package.
    pub fn needs_administrator(&self) -> bool {
        self.route() == RemovalRoute::Administrator
    }

    /// RABBIT can remove the package, and its route has every step it
    /// needs: on the backup set's route, nothing a backup can't undo; a
    /// vendor's uninstaller only through the elevated helper; the Trash and
    /// the administrator command whenever the package is removable.
    pub fn can_remove(&self) -> bool {
        self.removability.is_removable()
            && match self.route() {
                RemovalRoute::BackupSet => !self.unbacked,
                RemovalRoute::Vendor => self.needs_elevated_helper(),
                RemovalRoute::Trash | RemovalRoute::Administrator => true,
            }
    }
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

    /// Why RABBIT won't remove the package called `name`, in English, for
    /// the CLI and the saved report. A UI in another language goes by the
    /// variant.
    pub fn message(&self, name: &str) -> String {
        match self {
            Self::ManagedByReaPack {
                path,
                reapack_package,
            } => format!(
                "ReaPack manages {name}: it installed {} as {reapack_package}. Uninstall it in \
                 ReaPack instead.",
                path.display()
            ),
            Self::ReaPackCheckFailed { path, reason } => format!(
                "RABBIT couldn't check ReaPack's records for {} ({reason}), so it leaves {name} \
                 alone.",
                path.display()
            ),
            Self::NeedsStandardTarget => format!(
                "{name} serves every REAPER on this computer, so RABBIT removes it only from \
                 the standard REAPER, not from a portable one."
            ),
            Self::NoReceipt => format!(
                "RABBIT didn't install this copy of {name}, so it is most likely your own, and \
                 RABBIT leaves it alone."
            ),
            Self::NoTrustedUninstaller { refusal } => format!(
                "RABBIT found no uninstaller for {name} it can trust: {refusal}. You can \
                 uninstall {name} in Windows Settings, under Apps, Installed apps."
            ),
            Self::Manual { .. } => {
                format!("RABBIT can't remove {name} by itself; it has to be removed by hand.")
            }
            Self::NotSupported { .. } => format!("RABBIT can't uninstall {name} yet."),
            Self::UnsafePath { path, refusal } => format!(
                "RABBIT won't remove {}, because {refusal}. It leaves {name} alone rather than \
                 remove only part of it.",
                path.display()
            ),
            // Get Info's Locked clears only the lock a user sets; the other
            // locks RABBIT counts as Locked stay, so the steps can't promise
            // to work every time.
            Self::ByHand { path, reason } => format!(
                "RABBIT can't move {} to the Trash, because {}. To remove {name}, {} in Finder and \
                 press Command-Delete; Finder asks for an administrator's password if it needs \
                 one.{}",
                path.display(),
                reason.message(),
                if *reason == ByHandReason::Locked {
                    "untick Locked in its Get Info window, then select it"
                } else {
                    "select it"
                },
                if *reason == ByHandReason::Locked {
                    " If Locked isn't ticked, or Finder still can't move it, it has a lock that \
                     Finder can't remove."
                } else {
                    ""
                }
            ),
            Self::NotInstalled => format!("{name} isn't installed."),
            Self::Removable | Self::RemovableOrphanedFromReaPack => {
                format!("RABBIT can remove {name}.")
            }
        }
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
    /// For a macOS app from `trees`: the bundle identifier it must have,
    /// from the manifest, checked again right before it moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
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

/// An entry in Windows' per-user list of installed programs
/// (`HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall`) that a
/// removal would leave pointing at an uninstaller that is gone.
///
/// One entry serves every REAPER on the computer: OSARA's installer writes
/// the same `OSARA` entry whichever REAPER it installs into. So the entry
/// goes only while the program its `value` names sits directly in `dir`,
/// this REAPER's folder for it; one that points anywhere else belongs to
/// another REAPER and stays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedRegistryCleanup {
    /// The entry's name in the list, such as `OSARA`.
    pub key: String,
    /// The value naming the entry's program, such as `UninstallString`.
    pub value: String,
    /// The folder that program must sit in, such as the resource folder's
    /// `osara` folder.
    pub dir: PathBuf,
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
    /// REAPER's own configuration: `reaper.ini`, its `reaper-*.ini` files
    /// such as `reaper-kb.ini` and `reaper-extstate.ini`, where every
    /// extension keeps its settings side by side, and copies of them. Never
    /// removed.
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
    /// One of the package's settings files, but ReaPack's records list it,
    /// or couldn't be read to say. ReaPack is installed and keeps track of
    /// what it installed, so the file stays even with "Also remove
    /// settings".
    InstalledByReaPack,
}

/// Something to tell the user before they decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UninstallWarning {
    /// The manifest's `always_warn`: a warning shown whatever else is
    /// installed, with a question before starting when `confirm` says so.
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
    /// leaves that.
    RemovedPendingRestart,
    /// A dry run: this is what would be removed.
    WouldRemove,
    /// Not installed, so there was nothing to do.
    NotInstalled,
    /// RABBIT won't remove it, or a check stopped it before anything of it
    /// changed.
    Blocked,
    /// Removing it went wrong. Whatever had moved into the backup set was
    /// put back; the message says what else, if anything, is gone.
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
    /// Every step the macOS administrator command was given, or in a dry
    /// run would be given, in order, and how each went: what root was asked
    /// to do, for the record. Empty when no package needed it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub administrator_steps: Vec<AdministratorStepRecord>,
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

    /// The user stopped the run before every package was reached, declined
    /// the administrator permission prompt, or cancelled a vendor's
    /// uninstaller.
    pub fn was_cancelled(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.status == UninstallStatus::Cancelled)
    }

    /// The administrator permission prompt was declined, Windows' or
    /// macOS's, so the whole run was cancelled before anything changed.
    pub fn elevation_declined(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.message_code == UninstallMessage::ElevationDeclined)
    }

    /// Some package finishes going only when the computer restarts.
    pub fn needs_restart(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.status == UninstallStatus::RemovedPendingRestart)
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
    /// What became of the plan's entries in Windows' list of installed
    /// programs; in a dry run, what would. An entry RABBIT couldn't read
    /// or remove is a problem instead.
    pub registry_entries: Vec<RegistryEntryOutcome>,
    /// Steps after the removal that didn't work. The package is removed
    /// all the same.
    pub problems: Vec<UninstallProblem>,
    /// Files Windows removes when the computer restarts, after a vendor's
    /// uninstaller couldn't remove them because they were in use.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_restart: Vec<PathBuf>,
    /// How the vendor's own uninstaller went, when one ran or was meant to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<VendorOutcome>,
    /// English, for the CLI and the saved report. A UI in another language
    /// goes by `message_code`.
    pub message: String,
    pub message_code: UninstallMessage,
}

/// A file or folder the run removed: into the backup set, or on macOS to
/// the Trash or through the administrator command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedFile {
    pub path: PathBuf,
    pub reason: RemovalReason,
    pub size: u64,
    /// The receipt recorded another hash. Only files the manifest names go
    /// even then, such as a newer plug-in copied in by hand.
    pub changed_since_install: bool,
    /// Where it is in the backup set. `None` in a dry run, and for what
    /// went to the Trash or through the administrator command.
    pub backup_path: Option<PathBuf>,
    /// Where it is in the Trash, when it went there and RABBIT knows where.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trashed_to: Option<PathBuf>,
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
    /// Found in a settings folder, but ReaPack's records list it, or
    /// couldn't be read to say. ReaPack is installed and keeps track of what
    /// it installed, and moving the file behind its back would leave its
    /// records wrong.
    InstalledByReaPack,
}

/// What became of one entry in Windows' per-user list of installed programs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntryOutcome {
    /// The entry's whole key, such as
    /// `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OSARA`.
    pub key: String,
    pub action: RegistryEntryAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RegistryEntryAction {
    /// It pointed into this REAPER, and it is gone.
    Removed,
    /// A dry run: it points into this REAPER, and would go.
    WouldRemove,
    /// There is no such entry, so there was nothing to do.
    NotFound,
    /// It stays: it points somewhere else, most likely into another REAPER,
    /// or names no program RABBIT can place. `found` is the value as the
    /// entry holds it, if it holds one; `expected` the folder it would have
    /// to point into.
    PointsElsewhere {
        found: Option<String>,
        expected: PathBuf,
    },
}

/// Something after the removal that didn't work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum UninstallProblem {
    /// REAPER's language setting still names the language pack that was
    /// removed.
    LanguageResetFailed { error: String },
    /// An entry in Windows' list of installed programs couldn't be read, so
    /// RABBIT couldn't tell whether it points into this REAPER, and left it.
    RegistryEntryUnreadable { key: String, error: String },
    /// An entry in Windows' list of installed programs pointed into this
    /// REAPER, but couldn't be removed.
    RegistryEntryNotRemoved { key: String, error: String },
    /// A vendor's uninstaller removed the package, but RABBIT's receipt
    /// couldn't be saved without it. Nothing can be put back; the next
    /// detection finds the package gone all the same.
    ReceiptNotSaved { error: String },
    /// The folder under ProgramData that REAPER's uninstaller ran from, as
    /// a copy, couldn't be removed afterwards. Only administrators can
    /// change it, and nothing removes it later.
    VendorWorkingFolderLeft { path: PathBuf, error: String },
    /// Windows' list of files it removes at the next restart couldn't be
    /// read, so RABBIT couldn't tell whether the files still there go then.
    RestartListUnreadable { error: String },
    /// Windows' list of installed programs couldn't be read after the
    /// vendor's uninstaller, so RABBIT couldn't check whether it still
    /// lists the package.
    InstalledListUnreadable { error: String },
    /// macOS's installer still lists these receipts of the package after
    /// the administrator command told it to forget them. They do no harm:
    /// the package's files are gone.
    InstallerRecordsRemain { ids: Vec<String> },
    /// macOS's installer receipts couldn't be listed after the
    /// administrator command, so RABBIT couldn't check that it forgot the
    /// package's.
    InstallerRecordsUnchecked { error: String },
    /// The administrator command ended without saying where it stopped:
    /// `status` is its exit status, `None` when it had none RABBIT could
    /// read. Everything of the package is gone all the same.
    AdministratorEndedUnclearly { status: Option<i32> },
}

impl UninstallProblem {
    /// The problems a vendor run reported, as the report's own.
    pub fn from_vendor(problem: &VendorProblem) -> Self {
        match problem {
            VendorProblem::WorkingFolderNotRemoved { path, error } => {
                Self::VendorWorkingFolderLeft {
                    path: path.clone(),
                    error: error.clone(),
                }
            }
            VendorProblem::PendingRenamesUnreadable { error } => Self::RestartListUnreadable {
                error: error.clone(),
            },
            VendorProblem::ListUnreadable { error } => Self::InstalledListUnreadable {
                error: error.clone(),
            },
        }
    }

    /// English, for the CLI and the saved report.
    pub fn message(&self) -> String {
        match self {
            Self::LanguageResetFailed { error } => format!(
                "RABBIT couldn't set REAPER back to no language pack ({error}). Choose a \
                 language in REAPER's preferences."
            ),
            Self::RegistryEntryUnreadable { key, error } => format!(
                "RABBIT couldn't read {key} in Windows' list of installed programs ({error}), so \
                 it leaves that entry alone."
            ),
            Self::RegistryEntryNotRemoved { key, error } => format!(
                "RABBIT couldn't remove {key} from Windows' list of installed programs \
                 ({error}), so Windows still lists it."
            ),
            Self::ReceiptNotSaved { error } => format!(
                "RABBIT couldn't update its receipt ({error}). The receipt still lists the \
                 package; the next time RABBIT looks, it finds the package gone."
            ),
            Self::VendorWorkingFolderLeft { path, error } => format!(
                "RABBIT couldn't remove {}, the folder it ran the uninstaller from ({error}). \
                 Only an administrator can delete it.",
                path.display()
            ),
            Self::RestartListUnreadable { error } => format!(
                "RABBIT couldn't read which files Windows removes at the next restart \
                 ({error}), so it couldn't tell whether the files still there go then."
            ),
            Self::InstalledListUnreadable { error } => format!(
                "RABBIT couldn't read Windows' list of installed programs afterwards ({error}), \
                 so it couldn't check whether Windows still lists the package."
            ),
            Self::InstallerRecordsRemain { ids } => format!(
                "macOS's installer still lists {}, though the package's files are gone. That \
                 does no harm.",
                ids.join(", ")
            ),
            Self::InstallerRecordsUnchecked { error } => format!(
                "RABBIT couldn't read macOS's list of installer receipts afterwards ({error}), \
                 so it couldn't check that macOS forgot the package's."
            ),
            Self::AdministratorEndedUnclearly { status } => match status {
                Some(status) => format!(
                    "The administrator command ended with status {status} without saying \
                     where it stopped, but everything of the package is gone."
                ),
                None => "The administrator command ended without a status, but everything of \
                         the package is gone."
                    .to_string(),
            },
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
    /// The removal needs a step a backup can't undo, and this version has
    /// no way to carry it out for this plan.
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
    /// The vendor's own uninstaller removed the package. The status says
    /// whether anything is left over or goes at the next restart.
    UninstalledByVendor,
    /// The elevated helper checked the vendor's uninstaller again right
    /// before starting it and didn't trust it. Nothing of the package
    /// changed, and the run stopped there.
    VendorRefused {
        refusal: VendorRefusal,
    },
    /// The user cancelled the vendor's uninstaller.
    VendorCancelled,
    /// The vendor's uninstaller ended with this error code.
    VendorFailed {
        exit_code: i32,
    },
    /// The vendor's uninstaller said it succeeded, but the package is still
    /// there: `still_present` names what is, and `still_listed` says
    /// whether Windows still lists it, `None` when its list couldn't be
    /// read.
    VendorStillThere {
        still_present: Vec<PathBuf>,
        still_listed: Option<bool>,
    },
    /// The vendor's uninstaller was still open after `seconds`. RABBIT left
    /// it running and stopped the run, so nothing else would collide with
    /// it.
    VendorStillOpen {
        seconds: u64,
    },
    /// Starting the vendor's uninstaller, or talking to the elevated helper
    /// before it started, went wrong. The run stopped there.
    VendorRunFailed {
        error: String,
    },
    /// RABBIT lost track of the vendor's uninstaller after it had started:
    /// the helper went away, or waiting for the uninstaller failed. It may
    /// still be running, so what it removed is unknown; the receipt stays
    /// as it was, and the run stopped there.
    VendorLost {
        error: String,
    },
    /// The administrator permission prompt was declined, so nothing was
    /// uninstalled at all: Windows' prompt for the elevated helper, or the
    /// macOS administrator password window, which was cancelled.
    ElevationDeclined,
    /// The elevated helper couldn't start, or the macOS administrator
    /// command didn't run, so nothing was uninstalled.
    ElevationFailed {
        error: String,
    },
    /// Moved to the Trash on macOS, by RABBIT as the user or by the
    /// administrator command. `trashed_to` is where it is now, when RABBIT
    /// knows: macOS named it, or RABBIT could see it there.
    MovedToTrash {
        trashed_to: Option<PathBuf>,
    },
    /// macOS didn't move it to the Trash, so nothing of the package
    /// changed. `refused_by_macos` when macOS refused for lack of
    /// permission, which App Management can be behind.
    TrashFailed {
        error: String,
        refused_by_macos: bool,
    },
    /// macOS said the app moved to the Trash, but `path` is still there,
    /// or RABBIT can't find it where macOS said it went. What became of it
    /// is for the user to check in Finder; the run stopped there.
    TrashUnclear {
        path: PathBuf,
    },
    /// The run's administrator command removed the package for good on
    /// macOS. A backup can't bring it back.
    RemovedAsAdministrator,
    /// The administrator command stopped before the package was fully
    /// removed, or ran and left some of it, and the run stopped there.
    /// `path` is what the failing step worked on, only when that step was
    /// one of this package's. `ended_unclearly` when the command ended
    /// without saying which step stopped it; with neither, the command
    /// stopped at another package's step or ran to its end, and the package
    /// is still there all the same. `gone` and `left` say, from the
    /// filesystem afterwards, which of the package's paths are gone and
    /// which are still there. `refused_by_macos` when macOS said "Operation
    /// not permitted", which App Management can be behind; `detail` is the
    /// step's own message.
    AdministratorStepFailed {
        path: Option<PathBuf>,
        #[serde(default)]
        ended_unclearly: bool,
        gone: Vec<PathBuf>,
        left: Vec<PathBuf>,
        refused_by_macos: bool,
        detail: Option<String>,
    },
    /// After the vendor's uninstaller ran, these files were gone from the
    /// resource folder: REAPER's own settings or RABBIT's receipt. RABBIT
    /// wrote its receipt again from what it held in memory and stopped.
    ResourceFolderDamaged {
        missing: Vec<PathBuf>,
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
