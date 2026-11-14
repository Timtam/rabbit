//! Why a restore of a backup is refused, as codes.
//!
//! Every refusal a restore makes before it changes anything is one
//! [`RestoreRefusal`]. Each has a stable code, its serde tag in kebab-case,
//! which the wizard maps to a Fluent key of its own; fields that name what
//! it is about, such as set ids, package ids, paths and programs; and an
//! English text for the command line. That text never holds command-line
//! syntax: the CLI adds its own hints after it, such as the command that
//! lists the backups there are.
//!
//! The overview of the backups gives the same refusal where a backup's
//! status, or a file a restore would refuse, says the same thing (see
//! [`crate::backup_overview::BackupEntry::refusal`]), so a row in the
//! wizard and a run agree.
//!
//! A delete of backup sets refuses with the same codes where the reason is
//! the same (see [`crate::backup_delete`]): an id that can't name a set, a
//! link, another account's set, root on macOS, the lock, a set changed
//! since it was listed. Its English comes from
//! [`RestoreRefusal::delete_text`], which says "delete" where the text
//! says "restore".

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::preflight::{CLOSE_REAPER_BEFORE_RESTORING, RunningProcess, process_list};

/// Why a restore was refused, with nothing changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum RestoreRefusal {
    /// An id that can't name a set at all: not exactly one plain folder
    /// name, or one Windows would read as a drive, a stream or a device.
    /// Refused before anything is read.
    #[error(
        "\"{backup_id}\" can't be a backup id: an id is the name of one backup in RABBIT's backups folder, and RABBIT never names a backup like that"
    )]
    InvalidId { backup_id: String },

    /// A well-formed id that names no folder in RABBIT's backups folder,
    /// nor in its folder of unfinished deletions.
    #[error("The backup {backup_id} was deleted or never existed")]
    NotFound { backup_id: String },

    /// An id that names no set exactly, while a set whose name differs only
    /// in upper and lower case is there. Only exact names count, so it isn't
    /// restored, but it isn't gone either.
    #[error(
        "There is no backup named {backup_id}. Backup names must match exactly, upper and lower case included; there is one named {existing}"
    )]
    DiffersInCase { backup_id: String, existing: String },

    /// An id that names no set only because of the dots or spaces at its
    /// end, while the id without them names a set exactly: it was copied
    /// with the period after it, for example. On Windows such an id isn't
    /// even well-formed, since Windows drops those characters.
    #[error(
        "\"{backup_id}\" names no backup: RABBIT never ends a backup's name in a dot or a space. There is one named {existing}"
    )]
    EndsInDotOrSpace { backup_id: String, existing: String },

    /// An id that names only a set whose deletion started and didn't
    /// finish. Part of it may be gone already.
    #[error(
        "The backup {backup_id} is being deleted: a deletion of it started and didn't finish, so RABBIT doesn't restore it"
    )]
    BeingDeleted { backup_id: String },

    /// RABBIT's folder, its backups folder, the set, or something in the
    /// set is a symbolic link or junction. RABBIT never follows one there,
    /// wherever it leads.
    #[error(
        "RABBIT doesn't use {}: it is a symbolic link or junction, and RABBIT never follows one in its backups",
        .path.display()
    )]
    IsLink { path: PathBuf },

    /// One package was asked for from a set without a manifest: only a
    /// manifest says which package a file belongs to.
    #[error(
        "The backup {backup_id} has no manifest, so RABBIT can't tell which of its files belong to {package_id}. It can restore the backup only as a whole"
    )]
    NoManifest {
        backup_id: String,
        package_id: String,
    },

    /// One package was asked for from a set that records no packages. Only
    /// an uninstall's set, an update's set and a restore's set that kept an
    /// earlier version record which receipt entry goes with a package, and
    /// restoring one package's files from any other set would leave
    /// RABBIT's records describing other files.
    #[error(
        "The backup {backup_id} records no packages, so RABBIT can't tell which of its records go with {package_id}, and doesn't restore it alone. It can restore the backup as a whole"
    )]
    NoPackageRecords {
        backup_id: String,
        package_id: String,
    },

    /// One package was asked for that the set doesn't record. `packages`
    /// are the ones it does.
    #[error(
        "The backup {backup_id} holds no record of {package_id}; it holds {}",
        names_in_a_sentence(.packages)
    )]
    PackageNotInSet {
        backup_id: String,
        package_id: String,
        packages: Vec<String>,
    },

    /// Only the set's key map was asked for, and the set holds none that a
    /// restore puts back: no entry whose original is the resource folder's
    /// `reaper-kb.ini`, only ones whose copy never finished, or no manifest
    /// to say where its files came from.
    #[error("The backup {backup_id} holds no key map to restore")]
    NoKeymapInSet { backup_id: String },

    /// The set's manifest at `path` can't be read as one. `detail` is what
    /// the reader said.
    #[error(
        "The manifest of the backup {backup_id} can't be read ({detail}), so RABBIT doesn't restore it"
    )]
    DamagedManifest {
        backup_id: String,
        path: PathBuf,
        detail: String,
    },

    /// A set that holds only a copy of RABBIT's receipt, made after vendor
    /// installers ran. Nothing in it goes back.
    #[error(
        "The backup {backup_id} holds only a copy of RABBIT's records, so there is nothing in it to restore"
    )]
    RecordsOnly { backup_id: String },

    /// A set that holds a copy of RABBIT's receipt, such as one a restore
    /// of an update's set wrote. Copying it back would undo every change to
    /// the receipt since, uninstalls included. `backup_path` is the set's
    /// folder, where the copy stays.
    #[error(
        "The backup {backup_id} holds a copy of RABBIT's records, so RABBIT doesn't restore it: copying the records back would undo every change to them since the backup was made. The copy stays in the backup"
    )]
    ReceiptInSet {
        backup_id: String,
        backup_path: PathBuf,
    },

    /// RABBIT's receipt has another entry for the package than the one the
    /// set recorded: it was installed again since. Putting the old files
    /// under the newer entry would leave a receipt wrong about both.
    /// `other_packages` are the set's other packages with a file in it,
    /// when the whole set was asked for: each can still be restored on its
    /// own.
    #[error(
        "Can't restore {package_id} from the backup {backup_id}: RABBIT's records describe another copy of it, installed after this backup was made, and a restore would mix the two. To go back to this copy, uninstall that one first, then restore again"
    )]
    ReinstalledSince {
        backup_id: String,
        package_id: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        other_packages: Vec<String>,
    },

    /// Packages a set records, an uninstall's or a set of earlier
    /// versions, but none of whose files are in it: the uninstall found
    /// none to move, or the run that wrote the set stopped before any copy
    /// of them finished. Putting back their records alone would make RABBIT
    /// count them as installed with no files there. `package_ids` is empty
    /// when a set of earlier versions holds no file of any package.
    #[error("{}", nothing_of_it_text(.backup_id, .package_ids))]
    NothingOfItInSet {
        backup_id: String,
        package_ids: Vec<String>,
    },

    /// A finished copy the manifest names is gone from the set, or isn't
    /// the file it recorded. Restoring the rest would mix old and new
    /// files. `path` is where the file came from, as the set records it.
    #[error(
        "Part of the backup {backup_id} is missing: its copy of {} is gone, or isn't the file it recorded, so RABBIT doesn't restore it",
        .path.display()
    )]
    PartMissing { backup_id: String, path: PathBuf },

    /// An earlier version a file of which is neither in the set nor on
    /// disk with that version's hash, by its path in RABBIT's records.
    /// Going back to it would leave RABBIT's records claiming a file that
    /// isn't there.
    #[error(
        "{package_id} in the backup {backup_id} isn't complete: {} is neither in the backup nor on disk as that version had it, so RABBIT doesn't restore it",
        .path.display()
    )]
    NotComplete {
        backup_id: String,
        package_id: String,
        path: PathBuf,
    },

    /// An earlier version whose receipt entry RABBIT doesn't know: the
    /// update that made the set recorded none, and the set holds no copy
    /// of RABBIT's records to read it from, or one that can't be read.
    /// Without it, RABBIT can't tell which files that version had.
    #[error(
        "RABBIT can't tell which version of {package_id} the backup {backup_id} holds, so it doesn't restore it"
    )]
    VersionUnknown {
        backup_id: String,
        package_id: String,
    },

    /// An earlier version of a package while another package of its
    /// exclusive group is installed, whose install removed it. Going back
    /// to it would leave both installed.
    #[error(
        "RABBIT doesn't restore {package_id} from the backup {backup_id} while {} is installed: installing one of them removes the other",
        names_in_a_sentence(.installed)
    )]
    OtherLanguagePackInstalled {
        backup_id: String,
        package_id: String,
        installed: Vec<String>,
    },

    /// A file the set would put back at a place RABBIT never writes, for
    /// `reason`. `path` is where the file came from, as the set records it,
    /// or its place in a set without a manifest; for two files for one
    /// place, the place they would both go back to. `other_packages` are the
    /// set's other packages with a file in it, when the whole set was asked
    /// for and the file is one package's: each can still be restored on its
    /// own. For a file of an earlier version that is the key map by its
    /// real place, `path` is that place
    /// ([`TargetRefusal::KeymapAmongVersionFiles`]).
    #[error("RABBIT doesn't put back {}: {reason}", .path.display())]
    TargetRefused {
        path: PathBuf,
        reason: TargetRefusal,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        other_packages: Vec<String>,
    },

    /// A file of the newer version of `package_id` that going back to the
    /// earlier version the set holds would take into the backup of what
    /// the restore replaces, at a place a restore of that backup couldn't
    /// put it back to, for `reason`. `path` is the file as RABBIT's records
    /// list it. The reason is boxed to keep every refusal, and so every
    /// error, small.
    #[error(
        "RABBIT doesn't restore {package_id} from the backup {backup_id}, because it couldn't put {}, a file of the newer version, back from a backup: {reason}",
        .path.display()
    )]
    NewerFileRefused {
        backup_id: String,
        package_id: String,
        path: PathBuf,
        reason: Box<TargetRefusal>,
    },

    /// An earlier version of `package_id` whose receipt entry, as the set
    /// recorded it, lists `path`, a file at a place RABBIT never installs
    /// to, for `reason`. Going back would put that entry into RABBIT's
    /// records, which RABBIT never writes such a place into. The reason is
    /// boxed to keep every refusal, and so every error, small.
    #[error(
        "RABBIT doesn't restore {package_id} from the backup {backup_id}, because the backup's record of that version lists {}, a place RABBIT never installs to: {reason}",
        .path.display()
    )]
    EarlierFileRefused {
        backup_id: String,
        package_id: String,
        path: PathBuf,
        reason: Box<TargetRefusal>,
    },

    /// RABBIT's receipt can't be read, so RABBIT can't tell whether the
    /// set's records fit it. `detail` is what the reader said.
    #[error(
        "RABBIT can't read its records ({detail}), so it can't tell whether the backup fits them, and doesn't restore it"
    )]
    ReceiptUnreadable { detail: String },

    /// A REAPER that must be closed is running: the target's, or, when
    /// `every_reaper_for` names packages whose uninstall needs that, any
    /// REAPER on the computer. `package_names` are those packages by the
    /// names RABBIT shows, for the English. `processes` are the REAPERs
    /// running. `puts_keymap_back` says that the restore puts REAPER's key
    /// map back, which it does only with the target's REAPER closed, even
    /// when told REAPER may run (see
    /// [`RestoreRefusal::KeymapNeedsReaperClosed`]); so for the target's
    /// REAPER, closing it is the only way on.
    #[error("{}", reaper_running_text(.package_names, .processes))]
    ReaperRunning {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        every_reaper_for: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        package_names: Vec<String>,
        processes: Vec<RunningProcess>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        puts_keymap_back: bool,
    },

    /// The restore puts REAPER's key map back, and was told the target's
    /// REAPER may run. REAPER may save its own key map when it quits, over
    /// the one restored, so a key map comes back only with REAPER closed,
    /// which the restore then checks as every restore does.
    #[error(
        "The backup {backup_id} puts back REAPER's key map, which RABBIT does only while REAPER is closed: REAPER may save its own key map when it quits, over the one restored"
    )]
    KeymapNeedsReaperClosed { backup_id: String },

    /// Programs the uninstall plans of `package_ids` name as blocking are
    /// running. `package_names` are those packages by the names RABBIT
    /// shows, for the English. `programs` are the programs' names, as they
    /// run.
    #[error(
        "Close these programs before restoring {}: {}",
        names_in_a_sentence(.package_names),
        .programs.join(", ")
    )]
    ProgramRunning {
        package_ids: Vec<String>,
        package_names: Vec<String>,
        programs: Vec<String>,
    },

    /// RABBIT can't write where the restore puts files back, in the
    /// resource folder at `path` or, on macOS, its plug-ins folder. `check`
    /// is the name of the preflight check that failed, such as
    /// `resource-path`, and `detail` what it found, in English.
    #[error(
        "RABBIT can't write where the restore puts files back, in {}: {detail}",
        .path.display()
    )]
    NotWritable {
        path: PathBuf,
        check: String,
        detail: String,
    },

    /// Another RABBIT holds this REAPER's install lock: it is installing,
    /// uninstalling or restoring there.
    #[error(
        "Another RABBIT is working on this REAPER (lock {}, PID {pid}), so RABBIT doesn't restore anything until it has finished",
        .lock_path.display()
    )]
    LockHeld { lock_path: PathBuf, pid: u32 },

    /// A real restore was started on macOS with RABBIT itself running as
    /// root, as under `sudo`: the files it puts back, the backup of the
    /// files they replace and RABBIT's records would all be root's.
    #[error(
        "RABBIT is running as root, as it does under sudo, and doesn't restore backups like that: the files it puts back, the backup of the files they replace and RABBIT's records would all be root's. Run RABBIT as yourself, not with sudo"
    )]
    AsRoot,

    /// The set belongs to another account than RABBIT's backups folder, as
    /// one made under `sudo` does on a Mac.
    #[error("The backup {backup_id} belongs to another account, so RABBIT doesn't restore it")]
    NotYours { backup_id: String },

    /// The set changed between the reads before the lock and those under
    /// it, as when another RABBIT was still writing it.
    #[error(
        "The backup {backup_id} changed while RABBIT read it, as when another RABBIT was still writing it. Nothing changed; restore it again"
    )]
    ChangedWhileRead { backup_id: String },

    /// The set changed since it was listed: its fingerprint differs from
    /// the one the restore was given (see
    /// [`crate::rollback::RestoreItem::fingerprint`]).
    #[error(
        "The backup {backup_id} changed since RABBIT listed it. Nothing changed; look at the backups again, then restore"
    )]
    ChangedSinceListed { backup_id: String },
}

impl RestoreRefusal {
    /// The refusal's stable code, the same as its serde tag: what the
    /// wizard maps to a Fluent key, and what scripts can rely on.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "invalid-id",
            Self::NotFound { .. } => "not-found",
            Self::DiffersInCase { .. } => "differs-in-case",
            Self::EndsInDotOrSpace { .. } => "ends-in-dot-or-space",
            Self::BeingDeleted { .. } => "being-deleted",
            Self::IsLink { .. } => "is-link",
            Self::NoManifest { .. } => "no-manifest",
            Self::NoPackageRecords { .. } => "no-package-records",
            Self::PackageNotInSet { .. } => "package-not-in-set",
            Self::NoKeymapInSet { .. } => "no-keymap-in-set",
            Self::DamagedManifest { .. } => "damaged-manifest",
            Self::RecordsOnly { .. } => "records-only",
            Self::ReceiptInSet { .. } => "receipt-in-set",
            Self::ReinstalledSince { .. } => "reinstalled-since",
            Self::NothingOfItInSet { .. } => "nothing-of-it-in-set",
            Self::PartMissing { .. } => "part-missing",
            Self::NotComplete { .. } => "not-complete",
            Self::VersionUnknown { .. } => "version-unknown",
            Self::OtherLanguagePackInstalled { .. } => "other-language-pack-installed",
            Self::TargetRefused { .. } => "target-refused",
            Self::NewerFileRefused { .. } => "newer-file-refused",
            Self::EarlierFileRefused { .. } => "earlier-file-refused",
            Self::ReceiptUnreadable { .. } => "receipt-unreadable",
            Self::ReaperRunning { .. } => "reaper-running",
            Self::KeymapNeedsReaperClosed { .. } => "keymap-needs-reaper-closed",
            Self::ProgramRunning { .. } => "program-running",
            Self::NotWritable { .. } => "not-writable",
            Self::LockHeld { .. } => "lock-held",
            Self::AsRoot => "as-root",
            Self::NotYours { .. } => "not-yours",
            Self::ChangedWhileRead { .. } => "changed-while-read",
            Self::ChangedSinceListed { .. } => "changed-since-listed",
        }
    }

    /// The refusal as a delete of backup sets says it in English, for the
    /// command line: the refusals a delete shares with a restore whose text
    /// speaks of restoring say it of deleting, and every other one is said
    /// as a restore says it. Like the restore's text, it holds no
    /// command-line syntax.
    pub fn delete_text(&self) -> String {
        match self {
            Self::NotYours { backup_id } => format!(
                "The backup {backup_id} belongs to another account, so RABBIT doesn't delete it"
            ),
            Self::AsRoot => "RABBIT is running as root, as it does under sudo, and doesn't \
                 delete backups like that: RABBIT's folder of deletions, its lock and its report \
                 would be root's, and root could remove what permissions protect. Run RABBIT as \
                 yourself, not with sudo"
                .to_string(),
            Self::LockHeld { lock_path, pid } => format!(
                "Another RABBIT is working on this REAPER (lock {}, PID {pid}), so RABBIT \
                 doesn't delete anything until it has finished",
                lock_path.display()
            ),
            Self::ChangedSinceListed { backup_id } => format!(
                "The backup {backup_id} changed since RABBIT listed it. Nothing was deleted; \
                 look at the backups again, then delete"
            ),
            other => other.to_string(),
        }
    }
}

/// Why a restore won't put a file back at a place, for
/// [`RestoreRefusal::TargetRefused`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TargetRefusal {
    /// The place is in RABBIT's own folder, the receipt included.
    #[error("a restore never puts a file back into RABBIT's own folder")]
    RabbitFolder,
    /// The way to the place leads into RABBIT's own folder through a link.
    #[error("the way to it leads into RABBIT's own folder")]
    LeadsIntoRabbitFolder,
    /// A name on the way to the place may open another file or folder than
    /// it names: one with a `:`, on Windows one ending in a dot or space, or
    /// a device name.
    #[error("the name \"{name}\" on the way to it may open another file or folder than it names")]
    NameNotAsWritten { name: String },
    /// The place itself is a symbolic link or junction.
    #[error("it is a symbolic link or junction, which RABBIT never replaces")]
    PlaceIsLink,
    /// The way to the place, below a folder RABBIT installs into, leads
    /// through a symbolic link or junction.
    #[error(
        "the way to it leads through {}, a symbolic link or junction, which RABBIT never follows",
        .link.display()
    )]
    ThroughLink { link: PathBuf },
    /// The way to the place leads out of the resource folder.
    #[error("the way to it leads outside the resource folder, {}", .folder.display())]
    LeadsOutsideResourceFolder { folder: PathBuf },
    /// The way to the place leads out of the folder a package's own files
    /// go back into.
    #[error("the way to it leads outside {}", .folder.display())]
    LeadsOutside { folder: PathBuf },
    /// The place is outside the resource folder and every other folder
    /// RABBIT installs into.
    #[error(
        "the path is outside the REAPER resource folder and every other folder RABBIT installs into"
    )]
    OutsideAllowedFolders,
    /// A relative path with `.`, `..` or a root in it.
    #[error("the path is not a plain path inside the resource folder")]
    NotPlainPath,
    /// The folder a package's own files go back into is gone.
    #[error("the folder it goes back into, {}, is gone", .folder.display())]
    GuardFolderGone { folder: PathBuf },
    /// Something on the way couldn't be checked.
    #[error("RABBIT couldn't check {}: {detail}", .path.display())]
    CouldNotCheck { path: PathBuf, detail: String },
    /// The set holds more than one file for this place.
    #[error(
        "the backup holds more than one file for this place, so RABBIT can't tell which one belongs there"
    )]
    TwoFilesForOnePlace,
    /// A file from outside the resource folder in a set without a manifest:
    /// only a manifest could say where it belongs.
    #[error(
        "the set has no manifest, so RABBIT can't tell where this file from outside the resource folder belongs"
    )]
    OutsideWithoutManifest,
    /// The manifest names the file's copy outside the set. `path` of the
    /// refusal is that copy's recorded path.
    #[error("the manifest names its copy outside the backup set")]
    CopyOutsideSet,
    /// A file of an earlier version that would go back over REAPER's key
    /// map, reached under another name, such as a short name on Windows,
    /// or through a link folder inside the resource folder. A version's
    /// files never include the key map: only a restore of the key map puts
    /// it back, keeping the scripts registered since, and never while
    /// REAPER may run. `path` of the refusal is the place.
    #[error(
        "it is REAPER's key map, which RABBIT puts back only as a key map of its own, never among a version's files"
    )]
    KeymapAmongVersionFiles,
}

impl TargetRefusal {
    /// The reason's stable code, the same as its serde tag.
    pub fn code(&self) -> &'static str {
        match self {
            Self::RabbitFolder => "rabbit-folder",
            Self::LeadsIntoRabbitFolder => "leads-into-rabbit-folder",
            Self::NameNotAsWritten { .. } => "name-not-as-written",
            Self::PlaceIsLink => "place-is-link",
            Self::ThroughLink { .. } => "through-link",
            Self::LeadsOutsideResourceFolder { .. } => "leads-outside-resource-folder",
            Self::LeadsOutside { .. } => "leads-outside",
            Self::OutsideAllowedFolders => "outside-allowed-folders",
            Self::NotPlainPath => "not-plain-path",
            Self::GuardFolderGone { .. } => "guard-folder-gone",
            Self::CouldNotCheck { .. } => "could-not-check",
            Self::TwoFilesForOnePlace => "two-files-for-one-place",
            Self::OutsideWithoutManifest => "outside-without-manifest",
            Self::CopyOutsideSet => "copy-outside-set",
            Self::KeymapAmongVersionFiles => "keymap-among-version-files",
        }
    }

    /// The refusal of the file from `path` for this reason.
    pub(crate) fn refuse(self, path: &Path) -> RestoreRefusal {
        RestoreRefusal::TargetRefused {
            path: path.to_path_buf(),
            reason: self,
            other_packages: Vec::new(),
        }
    }
}

/// One reason of every kind for a refused place, for [`sample_refusals`].
fn sample_target_refusals() -> Vec<TargetRefusal> {
    let folder = PathBuf::from("REAPER").join("UserPlugins");
    vec![
        TargetRefusal::RabbitFolder,
        TargetRefusal::LeadsIntoRabbitFolder,
        TargetRefusal::NameNotAsWritten {
            name: "Vendor.".to_string(),
        },
        TargetRefusal::PlaceIsLink,
        TargetRefusal::ThroughLink {
            link: folder.join("linked"),
        },
        TargetRefusal::LeadsOutsideResourceFolder {
            folder: folder.clone(),
        },
        TargetRefusal::LeadsOutside {
            folder: folder.clone(),
        },
        TargetRefusal::OutsideAllowedFolders,
        TargetRefusal::NotPlainPath,
        TargetRefusal::GuardFolderGone {
            folder: folder.clone(),
        },
        TargetRefusal::CouldNotCheck {
            path: folder,
            detail: "access denied".to_string(),
        },
        TargetRefusal::TwoFilesForOnePlace,
        TargetRefusal::OutsideWithoutManifest,
        TargetRefusal::CopyOutsideSet,
        TargetRefusal::KeymapAmongVersionFiles,
    ]
}

/// One refusal of every kind, each reason of a refused place included,
/// with lists of more than one so their sentences are read too: for the
/// tests here, and for the wizard's, which read each refusal in every
/// language. Not for anything a user sees.
#[doc(hidden)]
pub fn sample_refusals() -> Vec<RestoreRefusal> {
    let id = || "uninstall-unix-1".to_string();
    let path = PathBuf::from("UserPlugins").join("reaper_sws-x64.dll");
    let reaper = RunningProcess {
        pid: "1234".to_string(),
        name: "reaper.exe".to_string(),
        executable_path: None,
    };
    let packages = || vec!["ffmpeg".to_string(), "sws".to_string()];
    let mut refusals = vec![
        RestoreRefusal::InvalidId {
            backup_id: "D:".to_string(),
        },
        RestoreRefusal::NotFound { backup_id: id() },
        RestoreRefusal::DiffersInCase {
            backup_id: "Unix-1".to_string(),
            existing: "unix-1".to_string(),
        },
        RestoreRefusal::EndsInDotOrSpace {
            backup_id: "unix-1.".to_string(),
            existing: "unix-1".to_string(),
        },
        RestoreRefusal::BeingDeleted { backup_id: id() },
        RestoreRefusal::IsLink { path: path.clone() },
        RestoreRefusal::NoManifest {
            backup_id: id(),
            package_id: "sws".to_string(),
        },
        RestoreRefusal::NoPackageRecords {
            backup_id: id(),
            package_id: "sws".to_string(),
        },
        RestoreRefusal::PackageNotInSet {
            backup_id: id(),
            package_id: "osara".to_string(),
            packages: packages(),
        },
        RestoreRefusal::DamagedManifest {
            backup_id: id(),
            path: path.clone(),
            detail: "expected value at line 1 column 1".to_string(),
        },
        RestoreRefusal::RecordsOnly { backup_id: id() },
        RestoreRefusal::ReceiptInSet {
            backup_id: id(),
            backup_path: path.clone(),
        },
        RestoreRefusal::ReinstalledSince {
            backup_id: id(),
            package_id: "sws".to_string(),
            other_packages: vec!["ffmpeg".to_string()],
        },
        RestoreRefusal::NothingOfItInSet {
            backup_id: id(),
            package_ids: packages(),
        },
        RestoreRefusal::PartMissing {
            backup_id: id(),
            path: path.clone(),
        },
        RestoreRefusal::NotComplete {
            backup_id: id(),
            package_id: "ffmpeg".to_string(),
            path: path.clone(),
        },
        RestoreRefusal::VersionUnknown {
            backup_id: id(),
            package_id: "osara".to_string(),
        },
        RestoreRefusal::OtherLanguagePackInstalled {
            backup_id: id(),
            package_id: "langpack-de".to_string(),
            installed: vec!["langpack-fr".to_string()],
        },
        RestoreRefusal::ReceiptUnreadable {
            detail: "expected value at line 1 column 1".to_string(),
        },
        RestoreRefusal::ReaperRunning {
            every_reaper_for: Vec::new(),
            package_names: Vec::new(),
            processes: vec![reaper.clone()],
            puts_keymap_back: false,
        },
        RestoreRefusal::ReaperRunning {
            every_reaper_for: Vec::new(),
            package_names: Vec::new(),
            processes: vec![reaper.clone()],
            puts_keymap_back: true,
        },
        RestoreRefusal::ReaperRunning {
            every_reaper_for: vec!["app2clap".to_string(), "jaws-scripts".to_string()],
            package_names: vec!["app2clap".to_string(), "JAWS scripts".to_string()],
            processes: vec![reaper],
            puts_keymap_back: false,
        },
        RestoreRefusal::ProgramRunning {
            package_ids: vec!["surge-xt".to_string()],
            package_names: vec!["Surge XT".to_string()],
            programs: vec!["Surge XT.exe".to_string()],
        },
        RestoreRefusal::NotWritable {
            path: PathBuf::from("REAPER"),
            check: "resource-path".to_string(),
            detail: "REAPER is read-only".to_string(),
        },
        RestoreRefusal::LockHeld {
            lock_path: PathBuf::from("RABBIT").join("package-install.lock"),
            pid: 42,
        },
        RestoreRefusal::AsRoot,
        RestoreRefusal::NotYours { backup_id: id() },
        RestoreRefusal::ChangedWhileRead { backup_id: id() },
        RestoreRefusal::ChangedSinceListed { backup_id: id() },
        RestoreRefusal::NoKeymapInSet { backup_id: id() },
        RestoreRefusal::KeymapNeedsReaperClosed { backup_id: id() },
    ];
    for reason in sample_target_refusals() {
        refusals.push(RestoreRefusal::TargetRefused {
            path: path.clone(),
            reason,
            other_packages: vec!["ffmpeg".to_string()],
        });
    }
    refusals.push(RestoreRefusal::NewerFileRefused {
        backup_id: id(),
        package_id: "sws".to_string(),
        path: path.clone(),
        reason: Box::new(TargetRefusal::RabbitFolder),
    });
    refusals.push(RestoreRefusal::EarlierFileRefused {
        backup_id: id(),
        package_id: "sws".to_string(),
        path: path.clone(),
        reason: Box::new(TargetRefusal::OutsideAllowedFolders),
    });
    refusals
}

/// The refusal `error` is, for tests that check a refusal's code. Panics,
/// naming the error, when it is no refusal.
#[cfg(test)]
pub(crate) fn refusal(error: &crate::error::RabbitError) -> &RestoreRefusal {
    match error {
        crate::error::RabbitError::RestoreRefused(refusal) => refusal,
        other => panic!("not a refusal: {other:?}"),
    }
}

/// What [`RestoreRefusal::NothingOfItInSet`] says: the packages none of
/// whose files are in the set, or, with none named, that nothing in it
/// belongs to a package.
fn nothing_of_it_text(backup_id: &str, package_ids: &[String]) -> String {
    if package_ids.is_empty() {
        format!(
            "Nothing in the backup {backup_id} belongs to a package RABBIT can put back, so there \
             is nothing to restore. RABBIT's records stay as they are"
        )
    } else {
        format!(
            "None of the files of {} are in the backup {backup_id}, so there is nothing to \
             restore. RABBIT's records stay as they are",
            names_in_a_sentence(package_ids)
        )
    }
}

/// Names as a sentence lists them: `a`, `a and b`, `a, b and c`.
pub fn names_in_a_sentence(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [name] => name.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What [`RestoreRefusal::ReaperRunning`] says: close the target's REAPER,
/// as the install preflight's check says it for a restore, or every REAPER
/// for the packages named `package_names`, which need that.
fn reaper_running_text(package_names: &[String], processes: &[RunningProcess]) -> String {
    if package_names.is_empty() {
        format!(
            "{CLOSE_REAPER_BEFORE_RESTORING}: {}",
            process_list(processes)
        )
    } else {
        format!(
            "Close every REAPER on this computer, portable ones included, before restoring {}: {}",
            names_in_a_sentence(package_names),
            process_list(processes)
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        RestoreRefusal, TargetRefusal, names_in_a_sentence, sample_refusals, sample_target_refusals,
    };
    use crate::preflight::RunningProcess;

    /// The number of each variant, so a new one fails to compile here until
    /// [`sample_refusals`] lists it.
    fn variant_number(refusal: &RestoreRefusal) -> usize {
        match refusal {
            RestoreRefusal::InvalidId { .. } => 0,
            RestoreRefusal::NotFound { .. } => 1,
            RestoreRefusal::DiffersInCase { .. } => 2,
            RestoreRefusal::EndsInDotOrSpace { .. } => 3,
            RestoreRefusal::BeingDeleted { .. } => 4,
            RestoreRefusal::IsLink { .. } => 5,
            RestoreRefusal::NoManifest { .. } => 6,
            RestoreRefusal::NoPackageRecords { .. } => 7,
            RestoreRefusal::PackageNotInSet { .. } => 8,
            RestoreRefusal::DamagedManifest { .. } => 9,
            RestoreRefusal::RecordsOnly { .. } => 10,
            RestoreRefusal::ReceiptInSet { .. } => 11,
            RestoreRefusal::ReinstalledSince { .. } => 12,
            RestoreRefusal::NothingOfItInSet { .. } => 13,
            RestoreRefusal::PartMissing { .. } => 14,
            RestoreRefusal::NotComplete { .. } => 15,
            RestoreRefusal::VersionUnknown { .. } => 16,
            RestoreRefusal::OtherLanguagePackInstalled { .. } => 17,
            RestoreRefusal::TargetRefused { .. } => 18,
            RestoreRefusal::NewerFileRefused { .. } => 19,
            RestoreRefusal::ReceiptUnreadable { .. } => 20,
            RestoreRefusal::ReaperRunning { .. } => 21,
            RestoreRefusal::ProgramRunning { .. } => 22,
            RestoreRefusal::NotWritable { .. } => 23,
            RestoreRefusal::LockHeld { .. } => 24,
            RestoreRefusal::AsRoot => 25,
            RestoreRefusal::NotYours { .. } => 26,
            RestoreRefusal::ChangedWhileRead { .. } => 27,
            RestoreRefusal::ChangedSinceListed { .. } => 28,
            RestoreRefusal::NoKeymapInSet { .. } => 29,
            RestoreRefusal::KeymapNeedsReaperClosed { .. } => 30,
            RestoreRefusal::EarlierFileRefused { .. } => 31,
        }
    }

    const VARIANTS: usize = 32;

    /// The number of each reason, as [`variant_number`] numbers refusals.
    fn reason_number(reason: &TargetRefusal) -> usize {
        match reason {
            TargetRefusal::RabbitFolder => 0,
            TargetRefusal::LeadsIntoRabbitFolder => 1,
            TargetRefusal::NameNotAsWritten { .. } => 2,
            TargetRefusal::PlaceIsLink => 3,
            TargetRefusal::ThroughLink { .. } => 4,
            TargetRefusal::LeadsOutsideResourceFolder { .. } => 5,
            TargetRefusal::LeadsOutside { .. } => 6,
            TargetRefusal::OutsideAllowedFolders => 7,
            TargetRefusal::NotPlainPath => 8,
            TargetRefusal::GuardFolderGone { .. } => 9,
            TargetRefusal::CouldNotCheck { .. } => 10,
            TargetRefusal::TwoFilesForOnePlace => 11,
            TargetRefusal::OutsideWithoutManifest => 12,
            TargetRefusal::CopyOutsideSet => 13,
            TargetRefusal::KeymapAmongVersionFiles => 14,
        }
    }

    const REASONS: usize = 15;

    /// The lists above hold every variant, so the tests below read them all.
    #[test]
    fn the_samples_hold_every_refusal_and_every_reason() {
        let refusals: BTreeSet<usize> = sample_refusals().iter().map(variant_number).collect();
        assert_eq!(refusals, (0..VARIANTS).collect());
        let reasons: BTreeSet<usize> = sample_target_refusals().iter().map(reason_number).collect();
        assert_eq!(reasons, (0..REASONS).collect());
    }

    /// No refusal's text holds command-line syntax: the CLI adds its own
    /// hints, and the wizard shows its own words.
    #[test]
    fn no_refusal_text_holds_command_line_syntax() {
        for refusal in sample_refusals() {
            let text = refusal.to_string();
            assert!(!text.contains("--"), "{text}");
            assert!(!text.contains("rabbit "), "{text}");
            assert!(!text.trim().is_empty(), "{refusal:?}");
        }
    }

    /// Each refusal's code is its serde tag, unique, and kebab-case, so the
    /// wizard's Fluent keys and scripts can rely on it. The same holds for
    /// the reasons a place is refused, and both read back as written.
    #[test]
    fn codes_are_the_serde_tags_and_unique() {
        let mut codes = BTreeSet::new();
        for refusal in sample_refusals() {
            let json = serde_json::to_value(&refusal).unwrap();
            assert_eq!(json["code"], refusal.code(), "{refusal:?}");
            let code = refusal.code();
            assert!(
                code.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{code}"
            );
            if let RestoreRefusal::TargetRefused { reason, .. } = &refusal {
                assert_eq!(json["reason"]["kind"], reason.code(), "{reason:?}");
            }
            let back: RestoreRefusal = serde_json::from_value(json).unwrap();
            assert_eq!(back, refusal);
            codes.insert((variant_number(&refusal), code));
        }
        let distinct: BTreeSet<&str> = codes.iter().map(|(_, code)| *code).collect();
        assert_eq!(distinct.len(), VARIANTS);
        let reasons: BTreeSet<&str> = sample_target_refusals()
            .iter()
            .map(TargetRefusal::code)
            .collect();
        assert_eq!(reasons.len(), REASONS);
    }

    /// What a delete says holds no command-line syntax either, for every
    /// refusal. The refusals a delete gives never speak of restoring, and
    /// those whose restore text does say "delete" instead, with the same
    /// code.
    #[test]
    fn a_delete_says_its_refusals_in_words_of_its_own() {
        for refusal in sample_refusals() {
            let text = refusal.delete_text();
            assert!(!text.contains("--"), "{text}");
            assert!(!text.contains("rabbit "), "{text}");
            assert!(!text.trim().is_empty(), "{refusal:?}");
        }
        let given_by_a_delete = |refusal: &RestoreRefusal| {
            matches!(
                refusal,
                RestoreRefusal::InvalidId { .. }
                    | RestoreRefusal::DiffersInCase { .. }
                    | RestoreRefusal::EndsInDotOrSpace { .. }
                    | RestoreRefusal::IsLink { .. }
                    | RestoreRefusal::NotYours { .. }
                    | RestoreRefusal::AsRoot
                    | RestoreRefusal::LockHeld { .. }
                    | RestoreRefusal::ChangedSinceListed { .. }
            )
        };
        let mut reworded = 0;
        for refusal in sample_refusals().iter().filter(|r| given_by_a_delete(r)) {
            let text = refusal.delete_text();
            assert!(!text.to_lowercase().contains("restor"), "{text}");
            if text != refusal.to_string() {
                assert!(text.contains("delet"), "{text}");
                reworded += 1;
            }
        }
        assert_eq!(reworded, 4);
        assert!(
            RestoreRefusal::AsRoot
                .delete_text()
                .ends_with("Run RABBIT as yourself, not with sudo")
        );
    }

    /// A backup that names no set keeps the words Phase 0 gave it. A
    /// running REAPER is said in new words on purpose: before, a restore
    /// said to close REAPER before installing extensions. The packages that
    /// need every REAPER closed, and those that name programs, are said by
    /// the names RABBIT shows, as an uninstall says them.
    #[test]
    fn not_found_keeps_its_words_and_a_running_reaper_says_restore() {
        assert_eq!(
            RestoreRefusal::NotFound {
                backup_id: "unix-1".to_string()
            }
            .to_string(),
            "The backup unix-1 was deleted or never existed"
        );
        let reaper = RunningProcess {
            pid: "7".to_string(),
            name: "reaper.exe".to_string(),
            executable_path: None,
        };
        assert_eq!(
            RestoreRefusal::ReaperRunning {
                every_reaper_for: Vec::new(),
                package_names: Vec::new(),
                processes: vec![reaper.clone()],
                puts_keymap_back: false,
            }
            .to_string(),
            "Close REAPER before restoring a backup: reaper.exe (7)"
        );
        assert_eq!(
            RestoreRefusal::ReaperRunning {
                every_reaper_for: vec!["jaws-scripts".to_string()],
                package_names: vec!["JAWS scripts".to_string()],
                processes: vec![reaper],
                puts_keymap_back: false,
            }
            .to_string(),
            "Close every REAPER on this computer, portable ones included, before restoring JAWS \
             scripts: reaper.exe (7)"
        );
        assert_eq!(
            RestoreRefusal::ProgramRunning {
                package_ids: vec!["surge-xt".to_string()],
                package_names: vec!["Surge XT".to_string()],
                programs: vec!["Surge XT.exe".to_string()],
            }
            .to_string(),
            "Close these programs before restoring Surge XT: Surge XT.exe"
        );
    }

    /// A refusal of packages names them as a sentence lists them.
    #[test]
    fn package_names_read_as_a_sentence() {
        let text = |ids: &[&str]| {
            RestoreRefusal::NothingOfItInSet {
                backup_id: "uninstall-unix-1".to_string(),
                package_ids: ids.iter().map(ToString::to_string).collect(),
            }
            .to_string()
        };

        assert!(
            text(&["sws"]).starts_with("None of the files of sws are in the backup"),
            "{}",
            text(&["sws"])
        );
        assert!(text(&["ffmpeg", "sws"]).starts_with("None of the files of ffmpeg and sws are"));
        assert!(
            text(&["ffmpeg", "reapack", "sws"])
                .starts_with("None of the files of ffmpeg, reapack and sws are")
        );
        assert!(
            text(&[]).starts_with(
                "Nothing in the backup uninstall-unix-1 belongs to a package RABBIT can put back"
            ),
            "{}",
            text(&[])
        );
        assert_eq!(names_in_a_sentence(&[]), "");
    }
}
