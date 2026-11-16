//! Deleting backups for good (backup plan 6.6): whole backup sets, or one
//! package or the key map out of a set. The core of `rabbit delete-backup`
//! and of the wizard's Backups tab.
//!
//! A delete first renames a set from `RABBIT/backups/<id>` to
//! `RABBIT/backups-deleting/<id>`, one rename on the same volume. From then
//! on no restore finds it, and older RABBITs never look there. Only then
//! does it walk the folder and remove what is in it, deepest first, by each
//! entry's own metadata: a symbolic link or a junction is removed as itself
//! and never followed, so nothing it points to is touched. What the walk
//! can't remove stays in `RABBIT/backups-deleting`, where the overview lists
//! it as an unfinished deletion, and a later delete of the same id finishes
//! it.
//!
//! Each folder is checked again right before the walk lists it, and left
//! as it is when it became a link. On Windows the walk also holds each
//! folder open, from that check until everything in it had its turn, in a
//! way that lets no other program move or delete it; while a folder is
//! open, Windows doesn't move a folder above it either. So no folder on the
//! way can be swapped for a junction while the walk is inside it. macOS and
//! Linux have no such hold: a folder swapped for a link between that check
//! and the listing would still be followed. That takes another program of
//! the same account racing the walk, one that could remove those files
//! itself.
//!
//! A delete changes nothing outside `RABBIT/backups` and
//! `RABBIT/backups-deleting` of the resource folder, besides the install
//! lock it holds and the report it saves in `RABBIT/logs`. It finds sets as
//! a restore does: an id is used only when it is well-formed and equals,
//! byte for byte, an entry `read_dir` lists, and `RABBIT`, both folders and
//! the set must be real folders, never links. Before it removes anything,
//! it checks that the path lies inside `RABBIT/backups-deleting/<id>`.
//!
//! One package or the key map can also go out of a set that stays (see
//! [`DeletePart`]). The set's manifest is saved first without the part's
//! file entries and records, in one rename, so from then on no restore and
//! no listing knows the part. Only then are its files removed from the set:
//! each copy no remaining entry uses, and the origin note beside a copy of
//! a file from outside the resource folder, then the folders that end up
//! empty, deepest first, never the set's own folder. A file is compared
//! with the manifest and with every file a remaining entry uses by what it
//! is on disk, not by how its name is spelled, so a name in another case or
//! a short name never removes one of those. Nothing is followed here
//! either: a link where a copy should be is removed as a link, and a folder
//! on the way that is a link is left as it is, with what is below it. On
//! Windows `RABBIT`, `RABBIT/backups`, the set and the folders on the way
//! to each file are held open, as the walk holds its folders. macOS and
//! Linux have no such hold here either: a folder swapped for a link between
//! the check and the removal would still be followed, as in the walk. A
//! file that can't be removed, such as one another program has open on
//! Windows, stays in the set without an entry, and the report names it. So
//! does what a crash right after the manifest's save leaves. The set's
//! size, from the overview's walk, still counts such files, and they go
//! when the set is deleted whole. When the parts a run names are every
//! backup the overview lists in a set, or an item names the whole set, the
//! set goes whole instead, renamed and walked as above. A set's own backup,
//! the overview's row for files of it no package or key map holds, is never
//! a part, so a set that lists one stays when its packages and key map go.
//! The overview's records row stands for several sets; each is deleted as
//! an item of its own (see [`DeleteItem::for_backup`]).

use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use rabbit_platform::{FileIdentity, file_identity};
use serde::{Deserialize, Serialize};

use crate::backup_overview::{
    BackupItem, BackupOverview, SetFingerprint, overview_of_with, reference_owner, restores_entry,
};
use crate::cancel::CancelToken;
use crate::error::{RabbitError, Result};
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::report::{SavedReportPaths, default_report_path, save_new_json_and_text_reports};
use crate::restore_refusal::RestoreRefusal;
use crate::rollback::{
    BACKUP_MANIFEST_FILE, BackupEnvironment, BackupManifest, DELETING_DIR, FoundBackupSet,
    MANIFEST_STAGING_FILE, ORIGIN_SIDECAR_SUFFIX, as_restore_refusal, backup_id_is_well_formed,
    backups_root, copy_relative_in_set, find_backup_set, find_folder_named,
    is_outside_file_in_layout, is_real_folder, load_manifest_to_restore, opens_as_written,
    read_backup_manifest_text, refuse_changed_since_listed, same_place,
    set_without_dots_or_spaces_at_the_end,
};

/// One backup for [`delete_backups`]: a backup set, named by its id exactly
/// as listed, upper and lower case included, as a whole, or one package or
/// the key map out of it. For a whole set, the id may name a set in
/// `RABBIT/backups`, an unfinished deletion in `RABBIT/backups-deleting`,
/// or both; everything it names goes. A part goes only out of a set in
/// `RABBIT/backups`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteItem {
    pub set_id: String,
    /// What of the set goes. Never left out: an item without it is an error
    /// rather than the whole set, so a delete never widens to more than was
    /// asked for.
    pub part: DeletePart,
    /// The set's fingerprint as the caller listed it (see
    /// [`crate::backup_overview::BackupSetOverview::fingerprint`]): the
    /// set's in `RABBIT/backups` when the id names one there, otherwise the
    /// unfinished deletion's. A set that changed since is refused, with
    /// [`RestoreRefusal::ChangedSinceListed`], and so is the whole run,
    /// before anything changes. `None` checks nothing, as for the CLI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<SetFingerprint>,
}

impl DeleteItem {
    /// The whole set `set_id`, with no fingerprint to compare.
    pub fn new(set_id: &str) -> Self {
        Self::of(set_id, DeletePart::WholeSet)
    }

    /// The whole set `set_id`, refused if its fingerprint now differs from
    /// `fingerprint`.
    pub fn listed(set_id: &str, fingerprint: SetFingerprint) -> Self {
        Self::new(set_id).with_fingerprint(fingerprint)
    }

    /// Only the backup of `package_id` out of the set `set_id`, with no
    /// fingerprint to compare.
    pub fn package(set_id: &str, package_id: &str) -> Self {
        Self::of(
            set_id,
            DeletePart::Package {
                package_id: package_id.to_string(),
            },
        )
    }

    /// Only the key map out of the set `set_id`, with no fingerprint to
    /// compare.
    pub fn keymap(set_id: &str) -> Self {
        Self::of(set_id, DeletePart::Keymap)
    }

    /// The items that delete `backup`, a backup the overview lists: one per
    /// set it is in. A package or a key map goes out of its set alone; a
    /// set's own backup and an unfinished deletion take their set whole, and
    /// the records take each of their sets whole. None of them has a
    /// fingerprint yet (see [`Self::with_fingerprint`]).
    pub fn for_backup(backup: &BackupItem) -> Vec<Self> {
        match backup {
            BackupItem::Package { set_id, package_id } => vec![Self::package(set_id, package_id)],
            BackupItem::Keymap { set_id } => vec![Self::keymap(set_id)],
            BackupItem::Set { set_id } | BackupItem::Deleting { set_id } => vec![Self::new(set_id)],
            BackupItem::Records { set_ids } => set_ids.iter().map(|id| Self::new(id)).collect(),
        }
    }

    /// The same item, refused if its set's fingerprint now differs from
    /// `fingerprint`.
    pub fn with_fingerprint(self, fingerprint: SetFingerprint) -> Self {
        Self {
            fingerprint: Some(fingerprint),
            ..self
        }
    }

    fn of(set_id: &str, part: DeletePart) -> Self {
        Self {
            set_id: set_id.to_string(),
            part,
            fingerprint: None,
        }
    }
}

/// What of a set a delete takes. There is no default: an item always says
/// which, so nothing ever widens to the whole set unasked.
///
/// A package or the key map goes out of a set in `RABBIT/backups` only when
/// the set has a manifest that can be read and the overview lists that
/// backup in it. It takes the files the backup's row counts (see
/// [`crate::backup_overview::BackupEntry::size`]) and what the set records
/// of it. When the parts a run names are every backup the overview lists in
/// a set, the set goes whole. The set's own backup, which the overview
/// lists for files of it that no package or key map holds, is never a
/// part: a set that lists one keeps it, and its files, when its packages
/// and key map go, and only a delete of the whole set takes them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DeletePart {
    /// The whole set, with an unfinished deletion of its id, as
    /// `delete-backup` without `--package` or `--key-map` deletes it.
    WholeSet,
    /// Only this package's backup, as `delete-backup --package` deletes it:
    /// the package's files in the set and what the set records of it, its
    /// receipt entry, the version that replaced it and whether its settings
    /// went with it. From an uninstall's set every file of the package
    /// goes; from a set of earlier versions its files of that version, but
    /// not the key map, which is a backup of its own.
    Package { package_id: String },
    /// Only the set's key map, the resource folder's `reaper-kb.ini`, and
    /// what the set records of what it was, as `delete-backup --key-map`
    /// deletes it.
    Keymap,
}

impl DeletePart {
    /// The one package asked for, if one was.
    pub fn package_id(&self) -> Option<&str> {
        match self {
            Self::Package { package_id } => Some(package_id),
            Self::WholeSet | Self::Keymap => None,
        }
    }

    /// The backup of the set `set_id` this part is, as the overview lists
    /// it. `None` for the whole set.
    pub fn backup_in(&self, set_id: &str) -> Option<BackupItem> {
        match self {
            Self::WholeSet => None,
            Self::Package { package_id } => Some(BackupItem::Package {
                set_id: set_id.to_string(),
                package_id: package_id.clone(),
            }),
            Self::Keymap => Some(BackupItem::Keymap {
                set_id: set_id.to_string(),
            }),
        }
    }
}

/// How [`delete_backups`] runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteRunOptions {
    /// Only check the sets and say what would go, with their sizes and
    /// which backups in them are the only copy of something. Nothing is
    /// changed, no lock is taken and no report is saved.
    pub dry_run: bool,
}

/// What came of one item of a delete, or in a dry run would.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteStatus {
    /// The set is gone, and so is an unfinished deletion of the same id,
    /// if there was one. Also for a part of a set that went whole with it,
    /// because the run named every backup of the set or the set itself.
    Deleted,
    /// The package or key map the item named is gone out of its set, which
    /// stays with its other backups: no entry of the set's manifest names
    /// it any more, so no restore and no listing knows it.
    /// [`DeleteReportItem::files_left`] names any file of it that couldn't
    /// be removed; such a file stays in the set until the set is deleted
    /// whole.
    DeletedFromSet,
    /// The id named only an unfinished deletion, and it is finished now.
    /// It never counts as already gone.
    FinishedEarlierDeletion,
    /// The id is well-formed and names nothing in either folder, as when
    /// the set was deleted before, so a script can run twice.
    AlreadyGone,
    /// A dry run: the set would be deleted.
    WouldDelete,
    /// A dry run: the id names only an unfinished deletion, which a real
    /// run would finish.
    WouldFinishEarlierDeletion,
    /// A dry run: the package or key map would go out of its set, which
    /// would stay with its other backups.
    WouldDeleteFromSet,
    /// The set couldn't be moved out of `RABBIT/backups`, so nothing of it
    /// changed; [`DeleteReportItem::not_deleted`] says why.
    NotDeleted,
    /// The package or key map couldn't go out of its set: the set's
    /// manifest couldn't be saved without it, so nothing of the set
    /// changed; [`DeleteReportItem::not_deleted`] says why, where
    /// [`NotDeletedReason::InUse`] and [`NotDeletedReason::Permission`] are
    /// about the manifest.
    NotDeletedFromSet,
    /// Part of the set is gone, and what the walk couldn't remove stays in
    /// `RABBIT/backups-deleting/<id>`, listed as an unfinished deletion. No
    /// restore sees it any more. A later delete of the id finishes it.
    PartlyDeleted,
    /// A check refused the set; [`DeleteReportItem::refusal`] says why.
    Refused,
    /// The run didn't get to the set: the user stopped it before, or a
    /// check refused another item, which refuses the whole run.
    NotReached,
}

impl DeleteStatus {
    /// Whether nothing of what the item named is left, or in a dry run would
    /// be: deleted, out of its set too, finished, already gone, or would be
    /// either. A part with files left counts as gone: no entry names them,
    /// so no restore and no listing knows the part any more.
    pub fn is_gone(self) -> bool {
        matches!(
            self,
            Self::Deleted
                | Self::DeletedFromSet
                | Self::FinishedEarlierDeletion
                | Self::AlreadyGone
                | Self::WouldDelete
                | Self::WouldFinishEarlierDeletion
                | Self::WouldDeleteFromSet
        )
    }
}

/// Why a set wasn't deleted, or a part of it, with nothing of it changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotDeletedReason {
    /// The set, or a file in it, is in use, so the system refused to move
    /// it. On Windows any file another program has open in the set does
    /// that, whatever sharing the program allowed, as Explorer or an
    /// antivirus scan may. For a part, the set's manifest is in use.
    InUse,
    /// RABBIT may not move the set, or for a part change its manifest.
    Permission,
    /// An unfinished deletion of the same id is still in
    /// `RABBIT/backups-deleting` and couldn't be finished, so the set has
    /// no free name to move to. Folders are never merged.
    EarlierDeletionLeft,
    /// Something else went wrong; [`DeleteReportItem::error`] says what.
    Other,
}

/// What [`delete_backups`] did, or in a dry run would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteReport {
    pub resource_path: PathBuf,
    pub dry_run: bool,
    /// One per item asked for, in the order given. Items of one set share
    /// what came of it.
    pub sets: Vec<DeleteReportItem>,
    /// A check refused at least one item before anything changed, so
    /// nothing was deleted: the items refused say why, and the others are
    /// [`DeleteStatus::NotReached`], or in a dry run say what would happen
    /// to them on their own.
    #[serde(default)]
    pub refused: bool,
    /// The user stopped the run before it got to every item: those it
    /// didn't reach are [`DeleteStatus::NotReached`]. The set being deleted
    /// when the stop came was finished.
    #[serde(default)]
    pub stopped: bool,
    /// Bytes of the files the run removed, every item together, each file
    /// counted once.
    #[serde(default)]
    pub bytes_freed: u64,
    /// Where the run saved this report: a real run that got past its
    /// checks always tries, also when every id was already gone. A dry run
    /// never does, nor a refused run, wherever the refusal came from, nor a
    /// run in a resource folder with no `RABBIT` folder, where there is
    /// nothing to delete. None of those changed anything.
    #[serde(skip)]
    pub saved_report: Option<SavedReportPaths>,
}

/// One item of a [`DeleteReport`].
///
/// Several items may name one set, as when a run names two packages of it.
/// They share what came of the set. When it went whole, or in a dry run
/// would, the first of them carries the set's size, the backups it held the
/// only copy of and the bytes freed, and the others carry none of these, so
/// the items add up to what the run freed. When parts of it went, each
/// part's first item carries that part's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteReportItem {
    pub set_id: String,
    pub status: DeleteStatus,
    /// What of the set the item named.
    pub part: DeletePart,
    /// The set's folder in `RABBIT/backups`, when the id named one. For a
    /// refused item, only when the refusal is about it, as for another
    /// account's set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_path: Option<PathBuf>,
    /// An unfinished deletion of the id in `RABBIT/backups-deleting`, when
    /// the id named one, alone or beside the set, and the set goes whole.
    /// For a refused item, only when the refusal is about it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unfinished_deletion: Option<PathBuf>,
    /// Bytes on disk of everything the id named, both folders together, as
    /// the overview's walk counted them before the run: what deleting it
    /// frees. For a part that goes out of its set, the sizes its manifest
    /// entries record, as its row in the overview says.
    #[serde(default)]
    pub size: u64,
    /// The backups in the set that are the only copy of what they hold,
    /// such as uninstalled software and key maps (see
    /// [`crate::backup_overview::BackupEntry::only_copy`]); for a part that
    /// goes out of its set, the part itself when it is one. A backup is
    /// deleted all the same when it is named; this says what goes with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only_copy: Vec<BackupItem>,
    /// Bytes of the files removed for this item.
    #[serde(default)]
    pub bytes_freed: u64,
    /// For [`DeleteStatus::DeletedFromSet`]: the files and links of the
    /// part that couldn't be removed, such as a file another program holds
    /// open on Windows, or one below a folder that is a link, which RABBIT
    /// never follows. They stay in the set with no entry naming them; the
    /// set's size still counts them, and they go when the set is deleted
    /// whole. [`Self::error`] says why the first of them stayed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_left: Vec<PathBuf>,
    /// For [`DeleteStatus::NotDeleted`] and
    /// [`DeleteStatus::NotDeletedFromSet`], why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_deleted: Option<NotDeletedReason>,
    /// What stays in `RABBIT/backups-deleting` after the walk couldn't
    /// remove all of it: the set's folder there for
    /// [`DeleteStatus::PartlyDeleted`], or the earlier deletion for
    /// [`NotDeletedReason::EarlierDeletionLeft`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_in: Option<PathBuf>,
    /// Whether the run finished an unfinished deletion of the id: the one
    /// the id named alone, for [`DeleteStatus::FinishedEarlierDeletion`],
    /// or the one beside the set, before the set was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub finished_earlier_deletion: bool,
    /// For [`DeleteStatus::Refused`], the refusal, by the code a restore
    /// gives for the same reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RestoreRefusal>,
    /// What went wrong: the first file or folder that couldn't be removed,
    /// or why the set couldn't be moved, or for a part its manifest saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DeleteReportItem {
    /// The report of `item`, which came out as `status`, with nothing else
    /// said yet.
    fn new(item: &DeleteItem, status: DeleteStatus) -> Self {
        Self {
            set_id: item.set_id.clone(),
            status,
            part: item.part.clone(),
            set_path: None,
            unfinished_deletion: None,
            size: 0,
            only_copy: Vec::new(),
            bytes_freed: 0,
            files_left: Vec::new(),
            not_deleted: None,
            left_in: None,
            finished_earlier_deletion: false,
            refusal: None,
            error: None,
        }
    }
}

/// Delete the backups `items` name, one set after another in the order
/// given, whole or a part out of it, or with `options.dry_run` say what
/// would go.
///
/// First, for the whole run and before anything changes: on macOS a real
/// delete as root is refused ([`RestoreRefusal::AsRoot`], an error), as an
/// uninstall and a restore are. A resource folder that isn't there, as
/// after a typo, is an error too: every id would count as already gone
/// while the sets are still in the folder meant. Then every item is
/// checked: its id must be
/// well-formed ([`RestoreRefusal::InvalidId`]), name its set exactly
/// ([`RestoreRefusal::DiffersInCase`],
/// [`RestoreRefusal::EndsInDotOrSpace`]) and lead through no link
/// ([`RestoreRefusal::IsLink`]; a linked `RABBIT`, `RABBIT/backups` or
/// `RABBIT/backups-deleting` refuses every item), the set must belong to
/// the account `RABBIT/backups` belongs to ([`RestoreRefusal::NotYours`],
/// never compared with the account RABBIT runs as, so a dry run as root
/// says the truth), and its fingerprint, when the item names one, must not
/// have changed ([`RestoreRefusal::ChangedSinceListed`]). A package or the
/// key map goes only out of a set in `RABBIT/backups` with a manifest that
/// can be read and lists that backup: otherwise the item is refused with
/// [`RestoreRefusal::PackageNotInSet`] for a package the set holds no
/// backup of, [`RestoreRefusal::NoPackageRecords`] for one whose files a
/// set that records no packages holds, [`RestoreRefusal::NoKeymapInSet`]
/// for a key map it doesn't hold or, without a manifest, can't tell apart,
/// [`RestoreRefusal::NoManifest`] or [`RestoreRefusal::DamagedManifest`]
/// for a set whose manifest can't say what belongs to the part, and
/// [`RestoreRefusal::BeingDeleted`] for an unfinished deletion, which goes
/// only whole. A part of an id that names nothing is already gone, as the
/// set is. When any item is refused, the whole run is: nothing changes, and
/// the report says why ([`DeleteReport::refused`]). A dry run stops after
/// these checks.
///
/// A real run then takes the install lock that installs, uninstalls and
/// restores take (another RABBIT holding it is
/// [`RestoreRefusal::LockHeld`], an error), and checks every item again
/// under it; a refusal there refuses the whole run as before, with nothing
/// changed. Then it goes set by set, in the order the items first name
/// them, every item of one set together, each checked once more right
/// before its set goes. A set an item names whole, or whose every backup
/// the overview lists the items name, goes whole: an unfinished deletion of
/// the id is finished first, then the set is renamed into
/// `RABBIT/backups-deleting` and the walk removes what is in it. A set that
/// can't be renamed is [`DeleteStatus::NotDeleted`] with nothing of it
/// changed; a walk that leaves something is [`DeleteStatus::PartlyDeleted`].
/// Otherwise only the parts named go out of the set, in one save of its
/// manifest and then their files (see [`DeletePart`]), and the set stays
/// ([`DeleteStatus::DeletedFromSet`]); a manifest that can't be saved
/// leaves the set as it was ([`DeleteStatus::NotDeletedFromSet`]). A part
/// a run before took out of a set that stays is no longer in it, so naming
/// it again is refused as above. An id that names nothing is
/// [`DeleteStatus::AlreadyGone`], so a script can run twice. A backup
/// marked as the only copy of something is deleted all the same. REAPER
/// may run: deleting a backup never touches REAPER's files.
///
/// `cancel` is read before each set, never while one is deleted. The set
/// being deleted always finishes; the items of those after it are
/// [`DeleteStatus::NotReached`], and the report says the run was stopped.
/// `progress` hears [`ProgressEvent::DeleteStarted`] and
/// [`ProgressEvent::DeleteCompleted`] once for each set a real run reaches,
/// however many of its items the run names.
///
/// Every real run that gets past its checks saves its report in
/// `RABBIT/logs`, under a name no other report has, also when an item
/// failed, the run was stopped or every id was already gone. A dry run and
/// a run refused before the lock take no lock; a run refused under it
/// saves no report either. A run whose every id names nothing, in a
/// resource folder with no `RABBIT` folder, takes no lock and saves no
/// report, so it never makes a `RABBIT` folder in a REAPER RABBIT never
/// used.
pub fn delete_backups(
    resource_path: &Path,
    items: &[DeleteItem],
    options: &DeleteRunOptions,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<DeleteReport> {
    delete_backups_with(
        resource_path,
        items,
        options,
        &BackupEnvironment::system(),
        progress,
        cancel,
    )
}

/// [`delete_backups`], asking `environment` whether RABBIT runs as root and
/// who owns each set. Tests pass their own answers.
pub(crate) fn delete_backups_with(
    resource_path: &Path,
    items: &[DeleteItem],
    options: &DeleteRunOptions,
    environment: &BackupEnvironment<'_>,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<DeleteReport> {
    let mut report = DeleteReport {
        resource_path: resource_path.to_path_buf(),
        dry_run: options.dry_run,
        sets: Vec::new(),
        refused: false,
        stopped: false,
        bytes_freed: 0,
        saved_report: None,
    };
    // Before anything else: under root, the folder of deletions, the lock
    // and the report would be root's, and root could remove what
    // permissions protect. A dry run changes nothing, so it may still say
    // what a delete would do.
    if !options.dry_run && (environment.runs_as_root)() {
        return Err(RestoreRefusal::AsRoot.into());
    }
    check_resource_folder(resource_path)?;

    let checked = check_items(resource_path, items, environment)?;
    report.refused = checked.iter().any(std::result::Result::is_err);
    let names_anything = checked
        .iter()
        .any(|named| named.as_ref().is_ok_and(Named::names_anything));
    // Without a `RABBIT` folder there is nothing to delete, and neither the
    // lock nor the report makes one.
    let has_rabbit_folder = matches!(is_real_folder(&resource_path.join("RABBIT")), Ok(true));
    if options.dry_run || report.refused || !(names_anything || has_rabbit_folder) {
        report.sets = planned_items(resource_path, items, checked, options.dry_run, environment);
        return Ok(report);
    }

    // Deleting changes RABBIT's backup folders, which installs, uninstalls
    // and restores write and read, so it never runs beside one of them.
    let install_lock =
        crate::lock::acquire_package_install_lock(resource_path).map_err(as_restore_refusal)?;
    #[cfg(test)]
    delete_seam::at(delete_seam::Step::Locked, resource_path);
    // The sets may have changed between the checks and the lock, so every
    // item is checked again under it, before anything changes. A refusal
    // now changed nothing either, so it saves no report, as one before the
    // lock.
    let checked = check_items(resource_path, items, environment)?;
    if checked.iter().any(std::result::Result::is_err) {
        report.refused = true;
        report.sets = planned_items(resource_path, items, checked, false, environment);
        return Ok(report);
    }
    let named: Vec<Named> = checked
        .into_iter()
        .filter_map(std::result::Result::ok)
        .collect();

    let mut done: Vec<Option<DeleteReportItem>> = vec![None; items.len()];
    for group in groups_of(items) {
        // Between sets only: the one being deleted always finishes.
        let ended = if report.stopped || cancel.is_cancelled() {
            report.stopped = true;
            let members: Vec<(usize, Named)> = group
                .indices
                .iter()
                .map(|&index| (index, named[index].clone()))
                .collect();
            plan_group(resource_path, items, &members, false, environment)
        } else {
            progress.report(ProgressEvent::DeleteStarted {
                set_id: group.set_id.clone(),
            });
            let ended = delete_group(resource_path, items, &group, environment);
            progress.report(ProgressEvent::DeleteCompleted {
                set_id: group.set_id.clone(),
                status: ended.status,
                bytes_freed: ended.bytes_freed,
            });
            report.bytes_freed += ended.bytes_freed;
            ended.items
        };
        for (index, item) in ended {
            done[index] = Some(item);
        }
    }
    report.sets = done.into_iter().flatten().collect();
    // The report names what went and what didn't, so every real run that
    // got this far leaves one, still under the lock. A report that can't be
    // saved changes nothing of the run.
    report.saved_report = save_delete_report(resource_path, &report).ok();
    drop(install_lock);
    Ok(report)
}

/// What an id names: a set in `RABBIT/backups`, an unfinished deletion in
/// `RABBIT/backups-deleting`, both, or neither.
#[derive(Debug, Clone, Default)]
struct Named {
    set: Option<PathBuf>,
    deleting: Option<PathBuf>,
}

impl Named {
    fn names_anything(&self) -> bool {
        self.set.is_some() || self.deleting.is_some()
    }
}

/// A check's refusal of one item, and what the id names that the refusal
/// is about: the set, the unfinished deletion, both, or, for an id that
/// names nothing RABBIT may use, neither.
#[derive(Debug)]
struct ItemRefusal {
    refusal: RestoreRefusal,
    about: Named,
}

impl ItemRefusal {
    /// A refusal about no folder.
    fn alone(refusal: RestoreRefusal) -> Self {
        Self {
            refusal,
            about: Named::default(),
        }
    }

    /// The report of the refused `item`.
    fn into_report_item(self, item: &DeleteItem) -> DeleteReportItem {
        let mut refused = DeleteReportItem::new(item, DeleteStatus::Refused);
        refused.refusal = Some(self.refusal);
        refused.set_path = self.about.set;
        refused.unfinished_deletion = self.about.deleting;
        refused
    }
}

/// What checking one item found: what its id names, or why it is refused.
type Checked = std::result::Result<Named, ItemRefusal>;

/// Check every item as [`delete_backups`] says, without changing anything:
/// a refusal of an item is its own; any other error, such as a backups
/// folder that can't be read, stops the whole run.
fn check_items(
    resource_path: &Path,
    items: &[DeleteItem],
    environment: &BackupEnvironment<'_>,
) -> Result<Vec<Checked>> {
    items
        .iter()
        .map(|item| check_item(resource_path, item, environment))
        .collect()
}

/// What `item`'s id names, refused when the id isn't one RABBIT uses, a
/// link is on the way, a folder it names belongs to another account than
/// `RABBIT/backups`, the set changed since it was listed, or the part the
/// item names can't go out of the set alone (see [`part_refusal`]).
fn check_item(
    resource_path: &Path,
    item: &DeleteItem,
    environment: &BackupEnvironment<'_>,
) -> Result<Checked> {
    let set_id = item.set_id.as_str();
    let named = match find_set_to_delete(resource_path, set_id) {
        Ok(named) => named,
        Err(RabbitError::RestoreRefused(refusal)) => return Ok(Err(ItemRefusal::alone(refusal))),
        Err(error) => return Err(error),
    };
    let expected = reference_owner(resource_path, environment);
    let folders = [(&named.set, false), (&named.deleting, true)];
    for (folder, in_deleting) in folders {
        let Some(folder) = folder else { continue };
        if let (Some(expected), Some(owner)) = (expected, (environment.owner_of)(folder))
            && expected != owner
        {
            // The refusal names the folder that is another account's, so
            // the CLI can say where to find it.
            let about = Named {
                set: (!in_deleting).then(|| folder.clone()),
                deleting: in_deleting.then(|| folder.clone()),
            };
            return Ok(Err(ItemRefusal {
                refusal: RestoreRefusal::NotYours {
                    backup_id: set_id.to_string(),
                },
                about,
            }));
        }
    }
    if let Some(path) = named.set.as_ref().or(named.deleting.as_ref()) {
        match refuse_changed_since_listed(set_id, path, item.fingerprint.as_ref()) {
            Ok(()) => {}
            Err(RabbitError::RestoreRefused(refusal)) => {
                return Ok(Err(ItemRefusal {
                    refusal,
                    about: named,
                }));
            }
            Err(error) => return Err(error),
        }
    }
    if let Some(refusal) = part_refusal(resource_path, item, &named, environment)? {
        return Ok(Err(ItemRefusal {
            refusal,
            about: named,
        }));
    }
    Ok(Ok(named))
}

/// Why a delete refuses to take `item`'s part out of the set its id names,
/// `named`, if it does. A package or the key map goes only out of a set in
/// `RABBIT/backups` whose manifest can be read and whose overview lists
/// that backup, so a delete never guesses which files belong to it. A
/// package the overview doesn't list is one the set holds no backup of
/// ([`RestoreRefusal::PackageNotInSet`], with the packages it does list),
/// unless the set records no packages and holds files of it besides the
/// key map ([`RestoreRefusal::NoPackageRecords`]). A key map the overview
/// doesn't list, or one in a set without a manifest, which can't say which
/// file is one, is [`RestoreRefusal::NoKeymapInSet`]. An id that names
/// only an unfinished deletion goes whole or not at all
/// ([`RestoreRefusal::BeingDeleted`]). Nothing for a whole set, nor for an
/// id that names nothing: the part is gone with its set, so a script can
/// run twice.
fn part_refusal(
    resource_path: &Path,
    item: &DeleteItem,
    named: &Named,
    environment: &BackupEnvironment<'_>,
) -> Result<Option<RestoreRefusal>> {
    if item.part == DeletePart::WholeSet {
        return Ok(None);
    }
    let backup_id = || item.set_id.clone();
    let Some(set) = &named.set else {
        return Ok(named
            .deleting
            .as_ref()
            .map(|_| RestoreRefusal::BeingDeleted {
                backup_id: backup_id(),
            }));
    };
    // The refusals a restore gives a manifest that can't be read, or is a
    // link, since nothing then says which files belong to the part.
    let manifest = match load_manifest_to_restore(&item.set_id, set).map_err(as_restore_refusal) {
        Ok(manifest) => manifest,
        Err(RabbitError::RestoreRefused(refusal)) => return Ok(Some(refusal)),
        Err(error) => return Err(error),
    };
    let package_id = item.part.package_id().map(ToString::to_string);
    let Some(manifest) = manifest else {
        return Ok(Some(match package_id {
            Some(package_id) => RestoreRefusal::NoManifest {
                backup_id: backup_id(),
                package_id,
            },
            None => RestoreRefusal::NoKeymapInSet {
                backup_id: backup_id(),
            },
        }));
    };
    let listed = Described::of(
        &overview_of_with(resource_path, &item.set_id, environment)?,
        &item.set_id,
    );
    if listed.lists(&item.set_id, &item.part) {
        return Ok(None);
    }
    let Some(package_id) = package_id else {
        return Ok(Some(RestoreRefusal::NoKeymapInSet {
            backup_id: backup_id(),
        }));
    };
    let packages = listed.package_ids();
    // A set that records no packages may still hold files of this one,
    // which it can't tell apart; the key map is a backup of its own.
    let keymap = BackupItem::Keymap {
        set_id: item.set_id.clone(),
    };
    let holds_files_of_it = manifest.files.iter().any(|entry| {
        entry.package_id.as_deref() == Some(package_id.as_str())
            && !restores_entry(resource_path, &manifest, &keymap, entry)
    });
    Ok(Some(if packages.is_empty() && holds_files_of_it {
        RestoreRefusal::NoPackageRecords {
            backup_id: backup_id(),
            package_id,
        }
    } else {
        RestoreRefusal::PackageNotInSet {
            backup_id: backup_id(),
            package_id,
            packages,
        }
    }))
}

/// Find what `set_id` names, the way a restore finds a set (see
/// [`find_backup_set`]), in both folders. An id that isn't well-formed is
/// refused, and so is one that names nothing only because of its case or
/// a dot or space at its end while a set's or an unfinished deletion's name
/// differs from it only so: counting it as already gone would tell a
/// script the set is gone while it is still there. A link on the way is
/// refused.
fn find_set_to_delete(resource_path: &Path, set_id: &str) -> Result<Named> {
    let found = match find_backup_set(resource_path, set_id) {
        Ok(found) => found,
        Err(RabbitError::RestoreRefused(RestoreRefusal::InvalidId { backup_id })) => {
            let refusal = named_without_dots_or_spaces_at_the_end(resource_path, &backup_id)
                .unwrap_or(RestoreRefusal::InvalidId { backup_id });
            return Err(refusal.into());
        }
        Err(error) => return Err(as_restore_refusal(error)),
    };
    let FoundBackupSet {
        set,
        deleting,
        other_case,
        deleting_other_case,
    } = found;
    if set.is_some() || deleting.is_some() {
        return Ok(Named { set, deleting });
    }
    // An unfinished deletion's name in another case counts as much as a
    // set's.
    if let Some(existing) = other_case.or(deleting_other_case) {
        return Err(RestoreRefusal::DiffersInCase {
            backup_id: set_id.to_string(),
            existing,
        }
        .into());
    }
    if let Some(refusal) = named_without_dots_or_spaces_at_the_end(resource_path, set_id) {
        return Err(refusal.into());
    }
    Ok(Named::default())
}

/// [`RestoreRefusal::EndsInDotOrSpace`] when `set_id` ends in dots or
/// spaces and, without them, names a set in `RABBIT/backups` or an
/// unfinished deletion in `RABBIT/backups-deleting` exactly. What it names
/// is only named, never used.
fn named_without_dots_or_spaces_at_the_end(
    resource_path: &Path,
    set_id: &str,
) -> Option<RestoreRefusal> {
    if let Some(refusal) = set_without_dots_or_spaces_at_the_end(resource_path, set_id) {
        return Some(refusal);
    }
    let trimmed = set_id.trim_end_matches(['.', ' ']);
    if trimmed == set_id || !backup_id_is_well_formed(trimmed) {
        return None;
    }
    match find_backup_set(resource_path, trimmed) {
        Ok(FoundBackupSet {
            deleting: Some(_), ..
        }) => Some(RestoreRefusal::EndsInDotOrSpace {
            backup_id: set_id.to_string(),
            existing: trimmed.to_string(),
        }),
        _ => None,
    }
}

/// The items of a run that name one set, by their places among the items,
/// in the order given.
#[derive(Debug)]
struct SetGroup {
    set_id: String,
    indices: Vec<usize>,
}

/// The sets `items` name, each once, in the order the items first name
/// them, with every item that names it.
fn groups_of(items: &[DeleteItem]) -> Vec<SetGroup> {
    let mut groups: Vec<SetGroup> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match groups.iter_mut().find(|group| group.set_id == item.set_id) {
            Some(group) => group.indices.push(index),
            None => groups.push(SetGroup {
                set_id: item.set_id.clone(),
                indices: vec![index],
            }),
        }
    }
    groups
}

/// Whether the items of one set, which name `parts` of what `named` found,
/// take the set whole: an item names the whole set, the id names no set in
/// `RABBIT/backups`, so there is no part to take out of one, or the parts
/// are every backup the overview lists in the set, `listed`.
fn takes_whole(set_id: &str, parts: &[&DeletePart], named: &Named, listed: &Described) -> bool {
    named.set.is_none()
        || parts.contains(&&DeletePart::WholeSet)
        || listed.all_named_by(set_id, parts)
}

/// The reports of the items of one set that a dry run checked, or that a
/// real run never got to, `members`, each with what its id names: already
/// gone, what a real run would do with them, or not reached. Items of a set
/// that goes whole share its status (see [`share_whole`]); each part that
/// goes out of its set says its own size.
fn plan_group(
    resource_path: &Path,
    items: &[DeleteItem],
    members: &[(usize, Named)],
    dry_run: bool,
    environment: &BackupEnvironment<'_>,
) -> Vec<(usize, DeleteReportItem)> {
    let Some((first, named)) = members.first() else {
        return Vec::new();
    };
    let indices: Vec<usize> = members.iter().map(|(index, _)| *index).collect();
    if !named.names_anything() {
        return indices
            .iter()
            .map(|&index| {
                let gone = DeleteReportItem::new(&items[index], DeleteStatus::AlreadyGone);
                (index, gone)
            })
            .collect();
    }
    let set_id = items[*first].set_id.as_str();
    let described = describe(resource_path, set_id, environment);
    let parts: Vec<&DeletePart> = indices.iter().map(|&index| &items[index].part).collect();
    if takes_whole(set_id, &parts, named, &described) {
        let status = match (&named.set, dry_run) {
            (Some(_), true) => DeleteStatus::WouldDelete,
            (None, true) => DeleteStatus::WouldFinishEarlierDeletion,
            (_, false) => DeleteStatus::NotReached,
        };
        let mut whole = DeleteReportItem::new(&DeleteItem::new(set_id), status);
        whole.set_path.clone_from(&named.set);
        whole.unfinished_deletion.clone_from(&named.deleting);
        described.fill(&mut whole);
        return share_whole(items, &indices, &whole);
    }
    let status = if dry_run {
        DeleteStatus::WouldDeleteFromSet
    } else {
        DeleteStatus::NotReached
    };
    let mut said = BTreeSet::new();
    indices
        .iter()
        .map(|&index| {
            let item = &items[index];
            let mut planned = DeleteReportItem::new(item, status);
            planned.set_path.clone_from(&named.set);
            if said.insert(&item.part) {
                described.fill_part(set_id, &item.part, &mut planned);
            }
            (index, planned)
        })
        .collect()
}

/// The reports of the items of a dry run, or of a run refused before
/// anything changed, in the order given: each refused item with its
/// refusal, and the others, set by set, as [`plan_group`] plans them.
fn planned_items(
    resource_path: &Path,
    items: &[DeleteItem],
    checked: Vec<Checked>,
    dry_run: bool,
    environment: &BackupEnvironment<'_>,
) -> Vec<DeleteReportItem> {
    let mut checked: Vec<Option<Checked>> = checked.into_iter().map(Some).collect();
    let mut planned: Vec<Option<DeleteReportItem>> = vec![None; items.len()];
    for group in groups_of(items) {
        let mut members = Vec::new();
        for &index in &group.indices {
            match checked[index].take() {
                Some(Ok(named)) => members.push((index, named)),
                Some(Err(refused)) => {
                    planned[index] = Some(refused.into_report_item(&items[index]));
                }
                None => {}
            }
        }
        for (index, item) in plan_group(resource_path, items, &members, dry_run, environment) {
            planned[index] = Some(item);
        }
    }
    planned.into_iter().flatten().collect()
}

/// The reports of the items `indices` of one set that went whole, or would
/// go, as `whole` says: each with its own part and the set's status and
/// folders. The first carries the set's size, the backups it held the only
/// copy of and the bytes freed; the others carry none, so the items add up
/// to what the run freed.
fn share_whole(
    items: &[DeleteItem],
    indices: &[usize],
    whole: &DeleteReportItem,
) -> Vec<(usize, DeleteReportItem)> {
    indices
        .iter()
        .enumerate()
        .map(|(nth, &index)| {
            let mut each = whole.clone();
            each.part = items[index].part.clone();
            if nth > 0 {
                each.size = 0;
                each.only_copy.clear();
                each.bytes_freed = 0;
            }
            (index, each)
        })
        .collect()
}

/// What an id holds, as the overview reads it: its size, the backups in it
/// that are the only copy of something, and each backup the overview lists
/// in the set in `RABBIT/backups`.
#[derive(Debug, Default)]
struct Described {
    size: u64,
    only_copy: Vec<BackupItem>,
    backups: Vec<ListedBackup>,
}

/// One backup the overview lists in a set in `RABBIT/backups`: a package,
/// the key map, or the set's own.
#[derive(Debug)]
struct ListedBackup {
    item: BackupItem,
    size: u64,
    only_copy: bool,
}

impl Described {
    /// What `overview`, the overview of the one id `set_id`, says of it.
    fn of(overview: &BackupOverview, set_id: &str) -> Self {
        Self {
            size: overview.sets.iter().map(|set| set.size).sum(),
            only_copy: overview
                .entries
                .iter()
                .filter(|entry| entry.only_copy)
                .map(|entry| entry.item.clone())
                .collect(),
            backups: overview
                .entries
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.item,
                        BackupItem::Package { .. }
                            | BackupItem::Keymap { .. }
                            | BackupItem::Set { .. }
                    ) && entry.item.set_id() == Some(set_id)
                })
                .map(|entry| ListedBackup {
                    item: entry.item.clone(),
                    size: entry.size,
                    only_copy: entry.only_copy,
                })
                .collect(),
        }
    }

    /// Fill in what the whole id holds.
    fn fill(&self, item: &mut DeleteReportItem) {
        item.size = self.size;
        item.only_copy.clone_from(&self.only_copy);
    }

    /// Fill in what `part` of the set `set_id` holds, as its row says.
    fn fill_part(&self, set_id: &str, part: &DeletePart, item: &mut DeleteReportItem) {
        let Some(wanted) = part.backup_in(set_id) else {
            return;
        };
        if let Some(backup) = self.backups.iter().find(|backup| backup.item == wanted) {
            item.size = backup.size;
            if backup.only_copy {
                item.only_copy = vec![wanted];
            }
        }
    }

    /// Whether the overview lists `part` as a backup of the set `set_id`.
    fn lists(&self, set_id: &str, part: &DeletePart) -> bool {
        part.backup_in(set_id)
            .is_some_and(|wanted| self.backups.iter().any(|backup| backup.item == wanted))
    }

    /// The packages the overview lists in the set, by id.
    fn package_ids(&self) -> Vec<String> {
        self.backups
            .iter()
            .filter_map(|backup| backup.item.package_id().map(ToString::to_string))
            .collect()
    }

    /// Whether `parts` name every backup the overview lists in the set
    /// `set_id`: then nothing anyone can tell apart would be left, and the
    /// set goes whole. Every listed row counts, the set's own backup among
    /// them, which the overview lists beside a package or the key map for
    /// files of the set none of them holds. That one is never a part, so a
    /// set that lists it stays with it when its packages and key map go,
    /// and a delete never takes those files unasked.
    fn all_named_by(&self, set_id: &str, parts: &[&DeletePart]) -> bool {
        let named: Vec<BackupItem> = parts
            .iter()
            .filter_map(|part| part.backup_in(set_id))
            .collect();
        !self.backups.is_empty()
            && self
                .backups
                .iter()
                .all(|backup| named.contains(&backup.item))
    }
}

/// What everything `set_id` names holds, from the overview of that one id:
/// both folders' sizes from the stat walk, the backups marked the only
/// copy, and each backup the overview lists in the set. Nothing is hashed.
/// What can't be read describes nothing; it is only said, and never stops
/// a delete.
fn describe(resource_path: &Path, set_id: &str, environment: &BackupEnvironment<'_>) -> Described {
    overview_of_with(resource_path, set_id, environment)
        .map(|overview| Described::of(&overview, set_id))
        .unwrap_or_default()
}

/// What came of one set of a real run: each of its items' reports, and
/// what [`ProgressEvent::DeleteCompleted`] says of the set.
struct GroupEnd {
    items: Vec<(usize, DeleteReportItem)>,
    status: DeleteStatus,
    bytes_freed: u64,
}

/// Delete what the items of `group` name of their set, under the lock:
/// check each again, then take the set whole or the parts named out of it.
/// A refusal or an error now is this set's alone, since the sets before it
/// may have gone already; nothing of it changes then, the refused item
/// says why, and the set's other items aren't reached.
fn delete_group(
    resource_path: &Path,
    items: &[DeleteItem],
    group: &SetGroup,
    environment: &BackupEnvironment<'_>,
) -> GroupEnd {
    let mut named = Named::default();
    let mut stopped: Vec<(usize, DeleteReportItem)> = Vec::new();
    for &index in &group.indices {
        let item = &items[index];
        match check_item(resource_path, item, environment) {
            Ok(Ok(found)) => named = found,
            Ok(Err(refused)) => stopped.push((index, refused.into_report_item(item))),
            Err(error) => stopped.push((index, failed_before_any_change(item, &error))),
        }
    }
    if let Some(status) = stopped.first().map(|(_, first)| first.status) {
        let mut ended: Vec<(usize, DeleteReportItem)> = group
            .indices
            .iter()
            .filter(|&&index| stopped.iter().all(|(other, _)| *other != index))
            .map(|&index| {
                let not_reached = DeleteReportItem::new(&items[index], DeleteStatus::NotReached);
                (index, not_reached)
            })
            .collect();
        ended.extend(stopped);
        return GroupEnd {
            items: ended,
            status,
            bytes_freed: 0,
        };
    }

    let set_id = group.set_id.as_str();
    let parts: Vec<&DeletePart> = group
        .indices
        .iter()
        .map(|&index| &items[index].part)
        .collect();
    let described = describe(resource_path, set_id, environment);
    match &named.set {
        Some(set) if !takes_whole(set_id, &parts, &named, &described) => {
            delete_parts_of_set(resource_path, items, group, set, &described)
        }
        _ => {
            let mut whole = delete_whole_set(resource_path, set_id, &named);
            if whole.status != DeleteStatus::AlreadyGone {
                described.fill(&mut whole);
            }
            GroupEnd {
                status: whole.status,
                bytes_freed: whole.bytes_freed,
                items: share_whole(items, &group.indices, &whole),
            }
        }
    }
}

/// Take the parts the items of `group` name out of their set at `set`, in
/// one save of its manifest (see [`remove_parts`]), and say what came of
/// each item: every part's first item carries what removing that part
/// freed and left, and its size as `described` lists it.
fn delete_parts_of_set(
    resource_path: &Path,
    items: &[DeleteItem],
    group: &SetGroup,
    set: &Path,
    described: &Described,
) -> GroupEnd {
    let mut parts: Vec<&DeletePart> = Vec::new();
    for &index in &group.indices {
        if !parts.contains(&&items[index].part) {
            parts.push(&items[index].part);
        }
    }
    let removed = remove_parts(resource_path, &group.set_id, set, &parts);
    let mut said = BTreeSet::new();
    let ended: Vec<(usize, DeleteReportItem)> = group
        .indices
        .iter()
        .map(|&index| {
            let item = &items[index];
            let mut done = DeleteReportItem::new(item, DeleteStatus::DeletedFromSet);
            done.set_path = Some(set.to_path_buf());
            match &removed {
                Err((reason, error)) => {
                    done.status = DeleteStatus::NotDeletedFromSet;
                    done.not_deleted = Some(*reason);
                    done.error = Some(error.clone());
                }
                Ok(removals) if said.insert(&item.part) => {
                    described.fill_part(&group.set_id, &item.part, &mut done);
                    if let Some(removal) = parts
                        .iter()
                        .position(|part| **part == item.part)
                        .and_then(|nth| removals.get(nth))
                    {
                        done.bytes_freed = removal.freed;
                        done.files_left.clone_from(&removal.left);
                        done.error.clone_from(&removal.first_error);
                    }
                }
                Ok(_) => {}
            }
            (index, done)
        })
        .collect();
    let status = if removed.is_ok() {
        DeleteStatus::DeletedFromSet
    } else {
        DeleteStatus::NotDeletedFromSet
    };
    GroupEnd {
        status,
        bytes_freed: ended.iter().map(|(_, done)| done.bytes_freed).sum(),
        items: ended,
    }
}

/// Delete everything the id `set_id` names, `named`, under the lock and
/// once its items were checked: finish an unfinished deletion of the id,
/// then rename the set and walk it.
fn delete_whole_set(resource_path: &Path, set_id: &str, named: &Named) -> DeleteReportItem {
    let mut done = DeleteReportItem::new(&DeleteItem::new(set_id), DeleteStatus::AlreadyGone);
    done.set_path.clone_from(&named.set);
    done.unfinished_deletion.clone_from(&named.deleting);

    // An unfinished deletion goes first, so the set gets a free name in
    // `RABBIT/backups-deleting`; two folders are never merged.
    if let Some(deleting) = &named.deleting {
        let removal = remove_set_folder(resource_path, set_id, deleting);
        done.bytes_freed += removal.freed;
        if !removal.complete {
            done.error = removal.first_error;
            done.left_in = Some(deleting.clone());
            if named.set.is_some() {
                done.status = DeleteStatus::NotDeleted;
                done.not_deleted = Some(NotDeletedReason::EarlierDeletionLeft);
            } else {
                done.status = DeleteStatus::PartlyDeleted;
            }
            return done;
        }
        done.finished_earlier_deletion = true;
        if named.set.is_none() {
            done.status = DeleteStatus::FinishedEarlierDeletion;
            return done;
        }
    }
    let Some(set) = &named.set else {
        return done;
    };

    let target = match move_out_of_backups(resource_path, set_id, set) {
        Ok(target) => target,
        Err(MoveFailure::Refused(refusal)) => {
            done.status = DeleteStatus::Refused;
            done.refusal = Some(refusal);
            return done;
        }
        Err(MoveFailure::Failed {
            reason,
            error,
            left_in,
        }) => {
            done.status = DeleteStatus::NotDeleted;
            done.not_deleted = Some(reason);
            done.error = Some(error);
            done.left_in = left_in;
            return done;
        }
    };
    #[cfg(test)]
    delete_seam::at(delete_seam::Step::Renamed, &target);
    let removal = remove_set_folder(resource_path, set_id, &target);
    done.bytes_freed += removal.freed;
    if removal.complete {
        done.status = DeleteStatus::Deleted;
    } else {
        done.status = DeleteStatus::PartlyDeleted;
        done.left_in = Some(target);
        done.error = removal.first_error;
    }
    done
}

/// The report of an item whose check under the lock failed, right before
/// its turn, with an error other than a refusal: not deleted, with the
/// error. Nothing of it changed.
fn failed_before_any_change(item: &DeleteItem, error: &RabbitError) -> DeleteReportItem {
    let mut failed = DeleteReportItem::new(item, DeleteStatus::NotDeleted);
    failed.not_deleted = Some(NotDeletedReason::Other);
    failed.error = Some(error.to_string());
    failed
}

/// Why a set couldn't be moved into `RABBIT/backups-deleting`. Nothing of
/// it changed either way.
enum MoveFailure {
    /// `RABBIT/backups-deleting` is a link.
    Refused(RestoreRefusal),
    Failed {
        reason: NotDeletedReason,
        error: String,
        /// For [`NotDeletedReason::EarlierDeletionLeft`], that deletion.
        left_in: Option<PathBuf>,
    },
}

/// Rename the set `set_id` at `set` to `RABBIT/backups-deleting/<id>`,
/// making that folder first if it isn't there, and return where it went.
/// The folder must be a real one, never a link, and the name in it free.
fn move_out_of_backups(
    resource_path: &Path,
    set_id: &str,
    set: &Path,
) -> std::result::Result<PathBuf, MoveFailure> {
    let failed = |reason: NotDeletedReason, error: String| MoveFailure::Failed {
        reason,
        error,
        left_in: None,
    };
    let backups = backups_root(resource_path);
    if set.parent() != Some(backups.as_path()) || set.file_name() != Some(OsStr::new(set_id)) {
        return Err(failed(
            NotDeletedReason::Other,
            format!("{} isn't a set in {}", set.display(), backups.display()),
        ));
    }
    let deleting = deleting_folder(resource_path);
    let made = match fs::create_dir(&deleting) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => {
            return Err(failed(
                NotDeletedReason::Other,
                format!("{}: {error}", deleting.display()),
            ));
        }
    };
    // Made just now or there before, it must be a real folder: a junction
    // there could lead anywhere.
    match is_real_folder(&deleting) {
        Ok(true) => {}
        Ok(false) => {
            return Err(failed(
                NotDeletedReason::Other,
                format!("{} isn't a folder", deleting.display()),
            ));
        }
        Err(RabbitError::BackupPathIsLink { path }) => {
            return Err(MoveFailure::Refused(RestoreRefusal::IsLink { path }));
        }
        Err(error) => return Err(failed(NotDeletedReason::Other, error.to_string())),
    }
    let target = deleting.join(set_id);
    // A rename onto an empty folder replaces it on Unix. Under the lock,
    // only a delete puts anything here, so a taken name is never merged.
    if fs::symlink_metadata(&target).is_ok() {
        return Err(name_taken(&deleting, set_id, &target));
    }
    match fs::rename(set, &target) {
        Ok(()) => Ok(target),
        Err(error) => {
            // Nothing moved, so the folder of deletions goes again when it
            // was made just now.
            if made {
                let _ = fs::remove_dir(&deleting);
            }
            Err(failed(
                rename_refused_because(set, &error),
                format!("{}: {error}", set.display()),
            ))
        }
    }
}

/// Why the set `set_id` can't take the name `target` in `deleting`, the
/// folder of deletions, where something is already there. Only an
/// unfinished deletion of the id itself is one a later run of the same id
/// finishes. Anything else there, such as a file of that name or a folder
/// whose name differs only in case where the file system counts such names
/// as the same, the id never names, so it is said with its path and left
/// alone.
fn name_taken(deleting: &Path, set_id: &str, target: &Path) -> MoveFailure {
    match find_folder_named(deleting, set_id) {
        Ok((Some(unfinished), _)) => MoveFailure::Failed {
            reason: NotDeletedReason::EarlierDeletionLeft,
            error: format!("{} is taken", unfinished.display()),
            left_in: Some(unfinished),
        },
        Ok((None, Some(other_case))) => MoveFailure::Failed {
            reason: NotDeletedReason::Other,
            error: format!(
                "{} takes the name the backup set would get there, since this file system \
                 counts names that differ only in case as the same; it is an unfinished \
                 deletion of {other_case}",
                deleting.join(&other_case).display()
            ),
            left_in: None,
        },
        Ok((None, None)) | Err(_) => MoveFailure::Failed {
            reason: NotDeletedReason::Other,
            error: format!(
                "{} is there already and isn't an unfinished deletion RABBIT can finish, so \
                 RABBIT doesn't move the backup set onto it",
                target.display()
            ),
            left_in: None,
        },
    }
}

/// Why the rename of the set at `set` failed. Windows refuses to rename a
/// folder while a file in it is open, with "access denied", whatever
/// sharing the program that opened it allowed; a sharing or lock violation
/// says so too. "Access denied" counts as in use when RABBIT may delete the
/// folder itself, and as permission otherwise.
fn rename_refused_because(set: &Path, error: &io::Error) -> NotDeletedReason {
    #[cfg(windows)]
    {
        const ERROR_ACCESS_DENIED: i32 = 5;
        const ERROR_SHARING_VIOLATION: i32 = 32;
        const ERROR_LOCK_VIOLATION: i32 = 33;
        match error.raw_os_error() {
            Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION) => {
                return NotDeletedReason::InUse;
            }
            Some(ERROR_ACCESS_DENIED) if may_delete_folder(set) => {
                return NotDeletedReason::InUse;
            }
            _ => {}
        }
    }
    #[cfg(not(windows))]
    let _ = set;
    match error.kind() {
        io::ErrorKind::PermissionDenied => NotDeletedReason::Permission,
        io::ErrorKind::ResourceBusy => NotDeletedReason::InUse,
        _ => NotDeletedReason::Other,
    }
}

/// Whether RABBIT may delete the folder at `folder` itself: it opens it with
/// delete access, as a link if it is one, and closes it again at once.
/// Opening changes nothing.
#[cfg(windows)]
fn may_delete_folder(folder: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt as _;
    const DELETE: u32 = 0x0001_0000;
    fs::OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(SHARE_EVERYTHING)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(folder)
        .is_ok()
}

/// `RABBIT/backups-deleting` of the resource folder.
fn deleting_folder(resource_path: &Path) -> PathBuf {
    resource_path.join("RABBIT").join(DELETING_DIR)
}

/// An error when `resource_path` isn't a folder: a typo there would make
/// every id count as already gone. The folder may be a link, as REAPER's
/// own resource folder sometimes is; only RABBIT's folders inside it must
/// be real ones.
fn check_resource_folder(resource_path: &Path) -> Result<()> {
    match fs::metadata(resource_path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(RabbitError::Io {
            path: resource_path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::NotADirectory, "it isn't a folder"),
        }),
        Err(source) => Err(RabbitError::Io {
            path: resource_path.to_path_buf(),
            source,
        }),
    }
}

/// The one folder a deletion of the id may remove anything in:
/// `RABBIT/backups-deleting/<id>`, and what is inside it, for a whole set;
/// `RABBIT/backups/<id>` for a part taken out of a set that stays, and only
/// what is inside it, never the set's folder itself.
struct DeleteGuard {
    root: PathBuf,
}

impl DeleteGuard {
    fn new(resource_path: &Path, set_id: &str) -> Self {
        Self {
            root: deleting_folder(resource_path).join(set_id),
        }
    }

    /// The guard of the set `set_id` in `RABBIT/backups`, out of which a
    /// delete takes a part (see [`Self::allows_inside`]).
    fn in_backups(resource_path: &Path, set_id: &str) -> Self {
        Self {
            root: backups_root(resource_path).join(set_id),
        }
    }

    /// Whether `path` lies inside the guarded folder, by its components,
    /// and isn't the folder itself: plain names only, at least one. It
    /// reads the path, not the disk.
    fn allows_inside(&self, path: &Path) -> bool {
        path.strip_prefix(&self.root).is_ok_and(|rest| {
            rest.components().next().is_some()
                && rest
                    .components()
                    .all(|component| matches!(component, Component::Normal(_)))
        })
    }

    /// Whether `path` is the guarded folder or lies inside it, by its
    /// components: the folder's own, then plain names only. It reads the
    /// path, not the disk; that no folder on the way is a link is the
    /// walk's to make sure (see [`remove_tree`]).
    fn allows(&self, path: &Path) -> bool {
        path.strip_prefix(&self.root).is_ok_and(|rest| {
            rest.components()
                .all(|component| matches!(component, Component::Normal(_)))
        })
    }
}

/// What a walk through a folder in `RABBIT/backups-deleting` did.
#[derive(Debug, Default)]
struct TreeRemoval {
    /// Bytes of the regular files it removed.
    freed: u64,
    /// The folder is gone.
    complete: bool,
    /// The first file or folder that couldn't be removed, and why.
    first_error: Option<String>,
}

impl TreeRemoval {
    fn note(&mut self, path: &Path, error: &io::Error) {
        self.first_error
            .get_or_insert_with(|| format!("{}: {error}", path.display()));
    }
}

/// Remove the folder `folder`, which must be `RABBIT/backups-deleting/<id>`
/// itself and a real folder, and everything in it, deepest first. Nothing
/// is followed: each entry is judged by its own metadata, and a link or a
/// junction is removed as itself. A file or folder that can't be removed
/// is noted and stays, and the walk goes on with the rest.
fn remove_set_folder(resource_path: &Path, set_id: &str, folder: &Path) -> TreeRemoval {
    let guard = DeleteGuard::new(resource_path, set_id);
    let mut removal = TreeRemoval::default();
    if folder != guard.root {
        removal.first_error = Some(format!(
            "{} isn't the folder a deletion of {set_id} removes",
            folder.display()
        ));
        return removal;
    }
    match fs::symlink_metadata(folder) {
        Ok(metadata) if matches!(EntryKind::of(&metadata), EntryKind::Folder) => {}
        Ok(_) => {
            removal.first_error = Some(format!(
                "{} isn't a real folder, so RABBIT doesn't look into it",
                folder.display()
            ));
            return removal;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            removal.complete = true;
            return removal;
        }
        Err(error) => {
            removal.note(folder, &error);
            return removal;
        }
    }
    remove_tree(folder, &guard, &mut removal);
    removal.complete = matches!(
        fs::symlink_metadata(folder),
        Err(error) if error.kind() == io::ErrorKind::NotFound
    );
    removal
}

/// One step of the walk: list a folder, or remove it once everything in
/// it had its turn, letting go of it first.
enum Step {
    Open(PathBuf),
    Close(PathBuf, HeldFolder),
}

/// Walk `root` and remove everything in it, then `root` itself, adding to
/// `removal`. Every path is checked against `guard` before anything is
/// removed. Each folder is checked again right before it is listed, and
/// held from then until it is removed (see [`hold_real_folder`]); one that
/// is no longer a real folder by then is noted and left as it is.
fn remove_tree(root: &Path, guard: &DeleteGuard, removal: &mut TreeRemoval) {
    let mut steps = vec![Step::Open(root.to_path_buf())];
    while let Some(step) = steps.pop() {
        match step {
            Step::Open(folder) => {
                #[cfg(test)]
                delete_seam::at(delete_seam::Step::Opening, &folder);
                let held = match hold_real_folder(&folder) {
                    Ok(held) => held,
                    Err(error) => {
                        removal.note(&folder, &error);
                        continue;
                    }
                };
                let entries = fs::read_dir(&folder);
                // Removed after everything in it, which goes on the stack
                // above it.
                steps.push(Step::Close(folder.clone(), held));
                let entries = match entries {
                    Ok(entries) => entries,
                    Err(error) => {
                        removal.note(&folder, &error);
                        continue;
                    }
                };
                for entry in entries {
                    let path = match entry {
                        Ok(entry) => entry.path(),
                        Err(error) => {
                            removal.note(&folder, &error);
                            continue;
                        }
                    };
                    if !guard.allows(&path) {
                        removal.note(&path, &outside_the_set());
                        continue;
                    }
                    // Its own metadata: unlike `fs::metadata`, it never
                    // follows a link.
                    let metadata = match fs::symlink_metadata(&path) {
                        Ok(metadata) => metadata,
                        Err(error) => {
                            removal.note(&path, &error);
                            continue;
                        }
                    };
                    match EntryKind::of(&metadata) {
                        EntryKind::Link { as_folder } => {
                            if let Err(error) = remove_link(&path, as_folder) {
                                removal.note(&path, &error);
                            }
                        }
                        EntryKind::Folder => steps.push(Step::Open(path)),
                        EntryKind::File => match remove_real_file(&path, &metadata) {
                            Ok(()) => removal.freed += metadata.len(),
                            Err(error) => removal.note(&path, &error),
                        },
                    }
                }
            }
            Step::Close(folder, held) => {
                held.let_go();
                if !guard.allows(&folder) {
                    removal.note(&folder, &outside_the_set());
                } else if let Err(error) = remove_real_folder(&folder) {
                    removal.note(&folder, &error);
                }
            }
        }
    }
}

/// The error noted for a path the guard refuses.
fn outside_the_set() -> io::Error {
    io::Error::other("it isn't inside the set being deleted, so RABBIT leaves it alone")
}

/// The error noted for a folder that became a link, or something else,
/// after the walk found it.
fn no_longer_a_real_folder() -> io::Error {
    io::Error::other("it is no longer a real folder, so RABBIT leaves it as it is")
}

/// A folder the walk is in. On Windows it holds the folder open, see
/// [`hold_real_folder`]; elsewhere it holds nothing.
struct HeldFolder {
    #[cfg(windows)]
    _handle: fs::File,
}

impl HeldFolder {
    /// Let go of the folder, so the walk can remove it.
    fn let_go(self) {}
}

/// Check that `folder` is still a real folder, never a link, right before
/// the walk lists it, and hold it until the walk removes it.
///
/// On Windows the check is made on a handle opened on the entry itself,
/// which stays open: it shares reading and writing but not deleting, so no
/// other program can move or delete the folder meanwhile, and Windows moves
/// no folder above it while it is open. The walk's paths below it so lead
/// where the walk looked.
#[cfg(windows)]
fn hold_real_folder(folder: &Path) -> io::Result<HeldFolder> {
    use std::os::windows::fs::OpenOptionsExt as _;
    // Listing the folder is access that counts for sharing; reading its
    // attributes alone wouldn't keep anyone out.
    const FILE_LIST_DIRECTORY: u32 = 0x1;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const SHARE_READ_AND_WRITE: u32 = 0x1 | 0x2;
    let handle = fs::OpenOptions::new()
        .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES)
        .share_mode(SHARE_READ_AND_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(folder)?;
    if !matches!(EntryKind::of(&handle.metadata()?), EntryKind::Folder) {
        return Err(no_longer_a_real_folder());
    }
    Ok(HeldFolder { _handle: handle })
}

#[cfg(not(windows))]
fn hold_real_folder(folder: &Path) -> io::Result<HeldFolder> {
    if !matches!(
        EntryKind::of(&fs::symlink_metadata(folder)?),
        EntryKind::Folder
    ) {
        return Err(no_longer_a_real_folder());
    }
    Ok(HeldFolder {})
}

/// What the walk does with an entry, by the entry's own metadata.
enum EntryKind {
    /// A symbolic link or a junction, removed as itself and never followed:
    /// as a folder where Windows marks it as one, which removes the link and
    /// not what it leads to.
    Link {
        as_folder: bool,
    },
    /// A real folder. On Windows that includes a folder that is a reparse
    /// point of another kind, such as a cloud placeholder: it leads nowhere
    /// else, Rust doesn't count it as a link, and the overview and a restore
    /// treat it as a folder too, so the walk goes into it.
    Folder,
    File,
}

impl EntryKind {
    /// Rust counts symbolic links and junctions as links, and on Windows
    /// every reparse point that names another entry, but no other kind.
    fn of(metadata: &fs::Metadata) -> Self {
        if metadata.file_type().is_symlink() {
            Self::Link {
                as_folder: marked_as_folder(metadata),
            }
        } else if metadata.is_dir() {
            Self::Folder
        } else {
            Self::File
        }
    }
}

/// Whether a link is removed as a folder: on Windows, when its own
/// attributes say it is one, as for a junction. Unix removes every link
/// as a file.
#[cfg(windows)]
fn marked_as_folder(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0
}

#[cfg(not(windows))]
fn marked_as_folder(_metadata: &fs::Metadata) -> bool {
    false
}

/// Remove the link at `path` itself, never what it leads to.
fn remove_link(path: &Path, as_folder: bool) -> io::Result<()> {
    if as_folder {
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

/// Remove the regular file at `path`, whose own metadata is `metadata`.
/// Rust's removal already removes a read-only file on the Windows versions
/// it can; should one refuse because the file is read-only, as copies keep
/// that attribute, the file loses the attribute and is removed again.
fn remove_real_file(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    let removed = fs::remove_file(path);
    #[cfg(windows)]
    if let Err(error) = &removed
        && error.kind() == io::ErrorKind::PermissionDenied
        && metadata.permissions().readonly()
    {
        clear_read_only(path)?;
        return fs::remove_file(path);
    }
    #[cfg(not(windows))]
    let _ = metadata;
    removed
}

/// Remove the empty real folder at `path`. On Windows, a folder a removal
/// refuses because it is read-only loses the attribute and is removed
/// again, while it is still a real folder.
fn remove_real_folder(path: &Path) -> io::Result<()> {
    let removed = fs::remove_dir(path);
    #[cfg(windows)]
    if let Err(error) = &removed
        && error.kind() == io::ErrorKind::PermissionDenied
        && fs::symlink_metadata(path).is_ok_and(|metadata| {
            matches!(EntryKind::of(&metadata), EntryKind::Folder)
                && metadata.permissions().readonly()
        })
    {
        clear_read_only(path)?;
        return fs::remove_dir(path);
    }
    removed
}

/// Clear the read-only attribute of the file or folder at `path`, through
/// a handle opened on the entry itself, never on what a link leads to, and
/// only while it is still no link. `fs::set_permissions` would follow a
/// link, so it is never used here.
#[cfg(windows)]
fn clear_read_only(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_WRITE_ATTRIBUTES: u32 = 0x100;
    let entry = fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES)
        .share_mode(SHARE_EVERYTHING)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = entry.metadata()?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(
            "it became a link, so RABBIT doesn't change it",
        ));
    }
    let mut permissions = metadata.permissions();
    // On Windows this clears only the read-only attribute, of this entry.
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    entry.set_permissions(permissions)
}

#[cfg(windows)]
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
/// Read, write and delete sharing, so opening an entry to look at it
/// never gets in the way of another program.
#[cfg(windows)]
const SHARE_EVERYTHING: u32 = 0x1 | 0x2 | 0x4;

/// Why a part couldn't go out of its set, with nothing of the set changed:
/// the reason, and what went wrong in words.
type NotRemoved = (NotDeletedReason, String);

/// What taking one part out of its set removed, and what of it stayed.
#[derive(Debug, Default)]
struct PartRemoval {
    /// Bytes of the regular files removed.
    freed: u64,
    /// The files and links of the part that couldn't be removed, by their
    /// path in the set.
    left: Vec<PathBuf>,
    /// Why the first of them stayed.
    first_error: Option<String>,
}

/// Take `parts` out of the set `set_id` at `set`, a set in `RABBIT/backups`
/// that stays, and say per part what that removed and left.
///
/// First the set's manifest is saved without the parts' file entries and
/// what it records of them: a package's record in `packages` (its receipt
/// entry, language line and whether its settings went with it) and in
/// `replaced` (its earlier entry and the version that replaced it), and for
/// the key map where it came from. The entries are those a restore of the
/// part takes, which are also those its row counts. Only those go: the
/// manifest is changed as its file holds it, so whatever else is in it,
/// such as fields a newer RABBIT wrote that this one doesn't know, stays as
/// it was. The save is one rename of a file written in full (see
/// [`save_manifest_in_place`]), so a crash leaves the old manifest or the
/// new one. From then on no restore and no listing knows the parts, and a
/// crash leaves only files no entry names.
///
/// Then each part's copies go, but only those no remaining entry uses,
/// since two entries can name one copy, and the origin note beside a copy
/// of a file from outside the resource folder, under `external/<n>`. Then
/// the folders that end up empty, deepest first, never the set's own
/// folder. Every path is checked to lie inside the set (see
/// [`DeleteGuard::allows_inside`]), and a name that may open another file
/// than it names is never used (see [`remove_from_set`]). The set's own
/// files stay too: its manifest, the manifest's staging file and an older
/// set's copy of RABBIT's records. Names are compared as the file system
/// tells places apart (see [`same_place`]), and each file about to go is
/// compared with all of those by what it is on disk (see
/// [`rabbit_platform::file_identity`]), so another spelling of one, as a
/// name in another case, a Windows short name or another Unicode form can
/// be, never removes it. A copy the manifest names outside the set is not
/// the set's to remove, and is left alone. A file that can't be removed
/// stays in the set, and its part says so; the part is gone all the same.
///
/// On Windows `RABBIT`, `RABBIT/backups` and the set are held open from
/// before the manifest is read until the end, each checked to be a real
/// folder, so none of them can be moved or swapped for a link meanwhile.
/// macOS and Linux have no such hold: a folder swapped for a link between
/// a check and a removal would still be followed, as in the walk. A
/// failure before the manifest is saved changes nothing, and is `Err`.
fn remove_parts(
    resource_path: &Path,
    set_id: &str,
    set: &Path,
    parts: &[&DeletePart],
) -> std::result::Result<Vec<PartRemoval>, NotRemoved> {
    let guard = DeleteGuard::in_backups(resource_path, set_id);
    if set != guard.root {
        return Err((
            NotDeletedReason::Other,
            format!(
                "{} isn't the backup set {set_id} in {}",
                set.display(),
                backups_root(resource_path).display()
            ),
        ));
    }
    let held = hold_way_to_set(resource_path, set_id).map_err(|(path, error)| {
        (
            NotDeletedReason::Other,
            format!("{}: {error}", path.display()),
        )
    })?;
    let manifest_path = set.join(BACKUP_MANIFEST_FILE);
    let unreadable = |error: &dyn std::fmt::Display| {
        (
            NotDeletedReason::Other,
            format!("{}: {error}", manifest_path.display()),
        )
    };
    // One read, two readings: what RABBIT knows of the manifest picks the
    // entries, and the manifest as written is what is saved again.
    let text = match read_backup_manifest_text(set) {
        Ok(Some(text)) => text,
        Ok(None) => {
            return Err((
                NotDeletedReason::Other,
                format!("{} has no manifest any more", set.display()),
            ));
        }
        Err(error) => return Err((NotDeletedReason::Other, error.to_string())),
    };
    let manifest: BackupManifest =
        serde_json::from_str(&text).map_err(|error| unreadable(&error))?;
    let mut as_written: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| unreadable(&error))?;
    let backups: Vec<Option<BackupItem>> =
        parts.iter().map(|part| part.backup_in(set_id)).collect();
    let mut taken = Vec::new();
    let mut taken_at = BTreeSet::new();
    let mut kept = Vec::new();
    for (index, entry) in manifest.files.iter().enumerate() {
        let part = backups.iter().position(|backup| {
            backup
                .as_ref()
                .is_some_and(|backup| restores_entry(resource_path, &manifest, backup, entry))
        });
        match part {
            Some(part) => {
                taken.push((part, entry));
                taken_at.insert(index);
            }
            None => kept.push(entry),
        }
    }
    without_parts(&mut as_written, &taken_at, parts);
    if let Err(error) = save_manifest_in_place(set, &as_written) {
        return Err((
            saving_refused_because(set, &error),
            format!("{}: {error}", manifest_path.display()),
        ));
    }
    #[cfg(test)]
    delete_seam::at(delete_seam::Step::PartsSaved, set);

    // What the set still needs is never removed: the copies the entries
    // that stay use, their origin notes, and the set's own files.
    let still_used: Vec<PathBuf> = kept
        .iter()
        .filter_map(|entry| copy_relative_in_set(&entry.backup_path))
        .chain(
            manifest
                .receipt_backup_path
                .as_deref()
                .and_then(copy_relative_in_set),
        )
        .flat_map(|copy| {
            let note = origin_note_of(&copy);
            std::iter::once(copy).chain(note)
        })
        .chain([
            PathBuf::from(BACKUP_MANIFEST_FILE),
            PathBuf::from(MANIFEST_STAGING_FILE),
        ])
        .collect();
    let in_use: BTreeSet<Vec<String>> = still_used
        .iter()
        .map(|relative| same_place(relative))
        .collect();
    let needed: BTreeSet<FileIdentity> = still_used
        .iter()
        .filter_map(|relative| file_identity(&set.join(relative)).ok())
        .collect();
    let mut removals: Vec<PartRemoval> = parts.iter().map(|_| PartRemoval::default()).collect();
    let mut gone = BTreeSet::new();
    let mut folders = BTreeSet::new();
    for (part, entry) in taken {
        let Some(copy) = copy_relative_in_set(&entry.backup_path) else {
            continue;
        };
        let note = origin_note_of(&copy);
        for relative in std::iter::once(copy).chain(note) {
            let place = same_place(&relative);
            if in_use.contains(&place) || !gone.insert(place) {
                continue;
            }
            folders.extend(
                relative
                    .ancestors()
                    .skip(1)
                    .filter(|folder| !folder.as_os_str().is_empty())
                    .map(Path::to_path_buf),
            );
            let removal = &mut removals[part];
            match remove_from_set(set, &relative, &guard, &needed) {
                Ok(freed) => removal.freed += freed,
                Err((path, error)) => {
                    removal
                        .first_error
                        .get_or_insert_with(|| format!("{}: {error}", path.display()));
                    removal.left.push(set.join(&relative));
                }
            }
        }
    }
    remove_empty_folders(set, &folders, &guard);
    drop(held);
    Ok(removals)
}

/// Take out of `manifest`, a set's manifest as its file holds it, the file
/// entries at the places `taken` and what it records of `parts`: each
/// package's record in `packages` and `replaced`, a record left empty
/// itself, as the writer leaves an empty one out, and for the key map
/// `keymap_origin`. Everything else stays as it is.
fn without_parts(manifest: &mut serde_json::Value, taken: &BTreeSet<usize>, parts: &[&DeletePart]) {
    let Some(fields) = manifest.as_object_mut() else {
        return;
    };
    if let Some(serde_json::Value::Array(files)) = fields.get_mut("files") {
        let mut index = 0;
        files.retain(|_| {
            let keep = !taken.contains(&index);
            index += 1;
            keep
        });
    }
    for part in parts {
        match part {
            DeletePart::Package { package_id } => {
                for records in ["packages", "replaced"] {
                    let Some(serde_json::Value::Object(by_package)) = fields.get_mut(records)
                    else {
                        continue;
                    };
                    by_package.remove(package_id.as_str());
                    if by_package.is_empty() {
                        fields.remove(records);
                    }
                }
            }
            DeletePart::Keymap => {
                fields.remove("keymap_origin");
            }
            DeletePart::WholeSet => {}
        }
    }
}

/// Where the origin note of the copy at `copy`, a path in a set, is: beside
/// it, named after it, for a copy of a file from outside the resource
/// folder, which the writer puts under `external/<n>` with such a note.
/// `None` for any other copy, which has no note.
fn origin_note_of(copy: &Path) -> Option<PathBuf> {
    if !is_outside_file_in_layout(copy) {
        return None;
    }
    let mut name = copy.file_name()?.to_os_string();
    name.push(ORIGIN_SIDECAR_SUFFIX);
    Some(copy.with_file_name(name))
}

/// Check that `RABBIT`, `RABBIT/backups` and the set `set_id` in it are
/// real folders, never links, and hold them, on Windows so that no other
/// program can move or delete them until what this returns is dropped (see
/// [`hold_real_folder`]). The first that isn't one, and why.
fn hold_way_to_set(
    resource_path: &Path,
    set_id: &str,
) -> std::result::Result<Vec<HeldFolder>, (PathBuf, io::Error)> {
    let rabbit = resource_path.join("RABBIT");
    let backups = backups_root(resource_path);
    let set = backups.join(set_id);
    [rabbit, backups, set]
        .into_iter()
        .map(|folder| hold_real_folder(&folder).map_err(|error| (folder, error)))
        .collect()
}

/// Hold each folder on the way from the set at `set`, which the caller
/// holds, down to `folder`, a plain path inside it, each checked to be a
/// real folder right before it is held (see [`hold_real_folder`]), so the
/// way to an entry in `folder` leads where it was checked to lead. The
/// first folder that isn't one, and why.
fn hold_way_down(
    set: &Path,
    folder: &Path,
) -> std::result::Result<Vec<HeldFolder>, (PathBuf, io::Error)> {
    let mut held = Vec::new();
    let mut current = set.to_path_buf();
    for name in folder.components() {
        current.push(name);
        match hold_real_folder(&current) {
            Ok(folder) => held.push(folder),
            Err(error) => return Err((current, error)),
        }
    }
    Ok(held)
}

/// Remove the file at `relative` in the set at `set` and return the bytes
/// it held, without following any link: a link there is removed as itself
/// and frees nothing, and a folder there is left alone, since a copy is a
/// file. Nothing there is nothing to remove, and so is a file that is one
/// of `needed`, the files the set still needs, by what it is on disk: it
/// stays, as a copy another entry names does. A name on the way that may
/// open another file than it names, as `copy.dll.` does on Windows, is
/// never used, nor a path the guard refuses. `Err` names the path that
/// stopped it, and why.
fn remove_from_set(
    set: &Path,
    relative: &Path,
    guard: &DeleteGuard,
    needed: &BTreeSet<FileIdentity>,
) -> std::result::Result<u64, (PathBuf, io::Error)> {
    let path = set.join(relative);
    if !guard.allows_inside(&path) {
        return Err((path, outside_the_set()));
    }
    if !relative
        .components()
        .all(|name| opens_as_written(name.as_os_str()))
    {
        return Err((
            path,
            io::Error::other("its name may open another file than it names, so RABBIT leaves it"),
        ));
    }
    let held = match hold_way_down(set, relative.parent().unwrap_or(Path::new(""))) {
        Ok(held) => held,
        Err((_, error)) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(failed) => return Err(failed),
    };
    // Its own metadata: unlike `fs::metadata`, it never follows a link.
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err((path, error)),
    };
    let removed = match EntryKind::of(&metadata) {
        EntryKind::Link { as_folder } => remove_link(&path, as_folder).map(|()| 0),
        EntryKind::File => match file_identity(&path) {
            Ok(identity) if needed.contains(&identity) => Ok(0),
            Ok(_) => remove_real_file(&path, &metadata).map(|()| metadata.len()),
            Err(error) => Err(error),
        },
        EntryKind::Folder => Err(io::Error::other(
            "it is a folder, not the file the backup recorded, so RABBIT leaves it",
        )),
    };
    drop(held);
    removed.map_err(|error| (path, error))
}

/// Remove each of `folders`, plain paths in the set at `set`, that is a
/// real folder with nothing in it, deepest first. The folders on the way
/// to each are checked and held as for a file (see [`hold_way_down`]). The
/// set's own folder is never one of them. A folder that can't be removed
/// holds nothing of a part, so it is only left.
fn remove_empty_folders(set: &Path, folders: &BTreeSet<PathBuf>, guard: &DeleteGuard) {
    let mut deepest_first: Vec<&PathBuf> = folders.iter().collect();
    deepest_first.sort_by_key(|folder| Reverse(folder.components().count()));
    for relative in deepest_first {
        let path = set.join(relative);
        if !guard.allows_inside(&path) {
            continue;
        }
        let Ok(held) = hold_way_down(set, relative.parent().unwrap_or(Path::new(""))) else {
            continue;
        };
        let empty_real_folder = fs::symlink_metadata(&path)
            .is_ok_and(|metadata| matches!(EntryKind::of(&metadata), EntryKind::Folder))
            && fs::read_dir(&path).is_ok_and(|mut entries| entries.next().is_none());
        if empty_real_folder {
            let _ = remove_real_folder(&path);
        }
        drop(held);
    }
}

/// Save `manifest` as the manifest of the set at `set`, all or nothing: it
/// is written in full to the manifest's staging file, then renamed over
/// the manifest, as a restore saves one, so a crash leaves the old manifest
/// or the new one, never part of one. The staging file is made new and
/// never written through a link; one an earlier crash left, a file or a
/// link, is removed as itself first. A failure leaves the old manifest.
///
/// On Unix the set's folder is then flushed to disk, so that after a power
/// cut the rename is there before any copy's removal is: the old manifest
/// never comes back naming copies that are gone. NTFS writes its changes to
/// names in order, so Windows needs no such step. A folder that can't be
/// flushed doesn't undo the save, which is done by then.
fn save_manifest_in_place(set: &Path, manifest: &serde_json::Value) -> io::Result<()> {
    use std::io::Write as _;

    let content = serde_json::to_string_pretty(manifest).map_err(io::Error::other)?;
    let staged = set.join(MANIFEST_STAGING_FILE);
    match fs::symlink_metadata(&staged) {
        Ok(metadata) => match EntryKind::of(&metadata) {
            EntryKind::Link { as_folder } => remove_link(&staged, as_folder)?,
            EntryKind::File => remove_real_file(&staged, &metadata)?,
            EntryKind::Folder => {
                return Err(io::Error::other(format!(
                    "{} is a folder, where RABBIT writes the manifest first",
                    staged.display()
                )));
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let saved = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)
        .and_then(|mut file| {
            file.write_all(content.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&staged, set.join(BACKUP_MANIFEST_FILE)));
    if saved.is_err() {
        let _ = fs::remove_file(&staged);
        return saved;
    }
    #[cfg(unix)]
    if let Ok(folder) = fs::File::open(set) {
        let _ = folder.sync_all();
    }
    Ok(())
}

/// Why the manifest of the set at `set` couldn't be saved without a part,
/// as a reason a part isn't deleted: another program holds it, or RABBIT
/// may not change it. Windows refuses to replace a file another program
/// holds open without letting others delete it with "access denied", as
/// it refuses a lack of permission; that error counts as in use when
/// opening the manifest to delete it fails because another program holds
/// it.
fn saving_refused_because(set: &Path, error: &io::Error) -> NotDeletedReason {
    #[cfg(windows)]
    {
        const ERROR_ACCESS_DENIED: i32 = 5;
        const ERROR_SHARING_VIOLATION: i32 = 32;
        const ERROR_LOCK_VIOLATION: i32 = 33;
        match error.raw_os_error() {
            Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION) => {
                return NotDeletedReason::InUse;
            }
            Some(ERROR_ACCESS_DENIED)
                if held_by_another_program(&set.join(BACKUP_MANIFEST_FILE)) =>
            {
                return NotDeletedReason::InUse;
            }
            _ => {}
        }
    }
    #[cfg(not(windows))]
    let _ = set;
    match error.kind() {
        io::ErrorKind::PermissionDenied => NotDeletedReason::Permission,
        io::ErrorKind::ResourceBusy => NotDeletedReason::InUse,
        _ => NotDeletedReason::Other,
    }
}

/// Whether another program holds the file at `path` open in a way that
/// keeps it from being deleted or replaced: opening it with delete access,
/// as a link if it is one, fails for sharing. Opening changes nothing, and
/// the handle closes at once.
#[cfg(windows)]
fn held_by_another_program(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt as _;
    const DELETE: u32 = 0x0001_0000;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    matches!(
        fs::OpenOptions::new()
            .access_mode(DELETE)
            .share_mode(SHARE_EVERYTHING)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path),
        Err(error) if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION)
    )
}

/// Save `report` as `RABBIT/logs/delete-backup-unix-<time>.json` and
/// `.txt` in `resource_path`, or `-2` and so on when a delete in the same
/// second saved one.
fn save_delete_report(resource_path: &Path, report: &DeleteReport) -> Result<SavedReportPaths> {
    save_new_json_and_text_reports(&default_report_path(resource_path, "delete-backup"), report)
}

/// Test-only hooks at the steps of a delete, so tests can change what is
/// on disk where a run could meet another program: right after the lock,
/// right after a set's rename, right before the walk lists a folder, and
/// right after a set's manifest was saved without the parts going.
#[cfg(test)]
pub(crate) mod delete_seam {
    use std::cell::RefCell;
    use std::path::Path;

    /// A step of a delete.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Step {
        /// Right after a real run took the lock, before it checks every
        /// item again. The path is the resource folder.
        Locked,
        /// Right after a set was moved into `RABBIT/backups-deleting`,
        /// before the walk. The path is where it went.
        Renamed,
        /// Right before the walk checks a folder again and lists it. The
        /// path is the folder.
        Opening,
        /// Right after a set's manifest was saved without the parts a run
        /// takes out of it, before any of their files goes. The path is the
        /// set.
        PartsSaved,
    }

    type Hook = Box<dyn FnMut(Step, &Path)>;

    thread_local! {
        static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    /// Takes the hook away again when dropped.
    pub(crate) struct Installed;

    impl Drop for Installed {
        fn drop(&mut self) {
            HOOK.with(|hook| hook.borrow_mut().take());
        }
    }

    /// Call `hook` at every step of every delete on this thread, with the
    /// path the step is about, until what this returns is dropped.
    pub(crate) fn install(hook: impl FnMut(Step, &Path) + 'static) -> Installed {
        HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
        Installed
    }

    pub(super) fn at(step: Step, path: &Path) {
        HOOK.with(|hook| {
            if let Some(hook) = hook.borrow_mut().as_mut() {
                hook(step, path);
            }
        });
    }
}

#[cfg(test)]
#[path = "backup_delete_tests.rs"]
mod tests;
