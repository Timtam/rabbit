//! Deleting whole backup sets for good (backup plan 6.6): the core of
//! `rabbit delete-backup`, and later of the wizard's Backups tab.
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
//! Only whole sets are deleted (backup plan D6). The overview's records row
//! stands for several sets; each is deleted as an item of its own (see
//! [`crate::backup_overview::BackupItem::set_ids`]).

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::backup_overview::{BackupItem, SetFingerprint, overview_of_with, reference_owner};
use crate::cancel::CancelToken;
use crate::error::{RabbitError, Result};
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::report::{SavedReportPaths, default_report_path, save_new_json_and_text_reports};
use crate::restore_refusal::RestoreRefusal;
use crate::rollback::{
    BackupEnvironment, DELETING_DIR, FoundBackupSet, as_restore_refusal, backup_id_is_well_formed,
    backups_root, find_backup_set, find_folder_named, is_real_folder, refuse_changed_since_listed,
    set_without_dots_or_spaces_at_the_end,
};

/// One backup set for [`delete_backups`], named by its id exactly as
/// listed, upper and lower case included. The id may name a set in
/// `RABBIT/backups`, an unfinished deletion in `RABBIT/backups-deleting`,
/// or both; everything it names goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteItem {
    pub set_id: String,
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
    /// The set `set_id`, with no fingerprint to compare.
    pub fn new(set_id: &str) -> Self {
        Self {
            set_id: set_id.to_string(),
            fingerprint: None,
        }
    }

    /// The set `set_id`, refused if its fingerprint now differs from
    /// `fingerprint`.
    pub fn listed(set_id: &str, fingerprint: SetFingerprint) -> Self {
        Self {
            set_id: set_id.to_string(),
            fingerprint: Some(fingerprint),
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
    /// if there was one.
    Deleted,
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
    /// The set couldn't be moved out of `RABBIT/backups`, so nothing of it
    /// changed; [`DeleteReportItem::not_deleted`] says why.
    NotDeleted,
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
    /// Whether nothing of what the id named is left, or in a dry run would
    /// be: deleted, finished, already gone, or would be either.
    pub fn is_gone(self) -> bool {
        matches!(
            self,
            Self::Deleted
                | Self::FinishedEarlierDeletion
                | Self::AlreadyGone
                | Self::WouldDelete
                | Self::WouldFinishEarlierDeletion
        )
    }
}

/// Why a set wasn't deleted, with nothing of it changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotDeletedReason {
    /// The set, or a file in it, is in use, so the system refused to move
    /// it. On Windows any file another program has open in the set does
    /// that, whatever sharing the program allowed, as Explorer or an
    /// antivirus scan may.
    InUse,
    /// RABBIT may not move the set.
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
    /// One per item asked for, in the order given.
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
    /// Bytes of the files the run removed, every item together.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteReportItem {
    pub set_id: String,
    pub status: DeleteStatus,
    /// The set's folder in `RABBIT/backups`, when the id named one. For a
    /// refused item, only when the refusal is about it, as for another
    /// account's set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_path: Option<PathBuf>,
    /// An unfinished deletion of the id in `RABBIT/backups-deleting`, when
    /// the id named one, alone or beside the set. For a refused item, only
    /// when the refusal is about it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unfinished_deletion: Option<PathBuf>,
    /// Bytes on disk of everything the id named, both folders together, as
    /// the overview's walk counted them before the run: what deleting it
    /// frees.
    #[serde(default)]
    pub size: u64,
    /// The backups in the set that are the only copy of what they hold,
    /// such as uninstalled software and key maps (see
    /// [`crate::backup_overview::BackupEntry::only_copy`]). A set is
    /// deleted all the same when it is named; this says what goes with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only_copy: Vec<BackupItem>,
    /// Bytes of the files removed for this item.
    #[serde(default)]
    pub bytes_freed: u64,
    /// For [`DeleteStatus::NotDeleted`], why.
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
    /// or why the set couldn't be moved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DeleteReportItem {
    fn new(set_id: &str, status: DeleteStatus) -> Self {
        Self {
            set_id: set_id.to_string(),
            status,
            set_path: None,
            unfinished_deletion: None,
            size: 0,
            only_copy: Vec::new(),
            bytes_freed: 0,
            not_deleted: None,
            left_in: None,
            finished_earlier_deletion: false,
            refusal: None,
            error: None,
        }
    }
}

/// Delete the backup sets `items` name, whole, one after another in the
/// order given, or with `options.dry_run` say what would go.
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
/// have changed ([`RestoreRefusal::ChangedSinceListed`]). When any item is
/// refused, the whole run is: nothing changes, and the report says why
/// ([`DeleteReport::refused`]). A dry run stops after these checks.
///
/// A real run then takes the install lock that installs, uninstalls and
/// restores take (another RABBIT holding it is
/// [`RestoreRefusal::LockHeld`], an error), and checks every item again
/// under it; a refusal there refuses the whole run as before, with nothing
/// changed. Then, per item: an unfinished deletion of the id is finished
/// first, then the set is renamed into `RABBIT/backups-deleting` and the
/// walk removes what is in it. A set that can't be renamed is
/// [`DeleteStatus::NotDeleted`] with nothing of it changed; a walk that
/// leaves something is [`DeleteStatus::PartlyDeleted`]. An id that names
/// nothing is [`DeleteStatus::AlreadyGone`], so a script can run twice.
/// A set marked as the only copy of something is deleted all the same.
/// REAPER may run: deleting a backup never touches REAPER's files.
///
/// `cancel` is read before each item. The one being deleted always
/// finishes; those after it are [`DeleteStatus::NotReached`], and the
/// report says the run was stopped. `progress` hears
/// [`ProgressEvent::DeleteStarted`] and [`ProgressEvent::DeleteCompleted`]
/// for each item a real run reaches.
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
        report.sets = items
            .iter()
            .zip(checked)
            .map(|(item, named)| {
                planned_item(resource_path, item, named, options.dry_run, environment)
            })
            .collect();
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
        report.sets = items
            .iter()
            .zip(checked)
            .map(|(item, named)| planned_item(resource_path, item, named, false, environment))
            .collect();
        return Ok(report);
    }
    // What each set holds is read before any of them goes.
    let described: Vec<Described> = items
        .iter()
        .map(|item| describe(resource_path, &item.set_id, environment))
        .collect();

    for (item, described) in items.iter().zip(described) {
        // Between sets only: the one being deleted always finishes.
        if report.stopped || cancel.is_cancelled() {
            report.stopped = true;
            let mut not_reached = DeleteReportItem::new(&item.set_id, DeleteStatus::NotReached);
            described.fill(&mut not_reached);
            report.sets.push(not_reached);
            continue;
        }
        progress.report(ProgressEvent::DeleteStarted {
            set_id: item.set_id.clone(),
        });
        let mut done = delete_item(resource_path, item, environment);
        // An id named twice is already gone the second time, and held
        // nothing by then.
        if done.status != DeleteStatus::AlreadyGone {
            described.fill(&mut done);
        }
        progress.report(ProgressEvent::DeleteCompleted {
            set_id: item.set_id.clone(),
            status: done.status,
            bytes_freed: done.bytes_freed,
        });
        report.bytes_freed += done.bytes_freed;
        report.sets.push(done);
    }
    // The report names what went and what didn't, so every real run that
    // got this far leaves one, still under the lock. A report that can't be
    // saved changes nothing of the run.
    report.saved_report = save_delete_report(resource_path, &report).ok();
    drop(install_lock);
    Ok(report)
}

/// What an id names: a set in `RABBIT/backups`, an unfinished deletion in
/// `RABBIT/backups-deleting`, both, or neither.
#[derive(Debug, Default)]
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

    /// The report of the refused item `set_id`.
    fn into_report_item(self, set_id: &str) -> DeleteReportItem {
        let mut refused = DeleteReportItem::new(set_id, DeleteStatus::Refused);
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
/// `RABBIT/backups`, or the set changed since it was listed.
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
    Ok(Ok(named))
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

/// The report of an item that a dry run checked, or that a refused run
/// never got to: refused with its refusal, already gone, what a real run
/// would do with it, or for a refused real run not reached.
fn planned_item(
    resource_path: &Path,
    item: &DeleteItem,
    checked: Checked,
    dry_run: bool,
    environment: &BackupEnvironment<'_>,
) -> DeleteReportItem {
    let named = match checked {
        Ok(named) => named,
        Err(refused) => return refused.into_report_item(&item.set_id),
    };
    let status = match (&named.set, &named.deleting, dry_run) {
        (None, None, _) => DeleteStatus::AlreadyGone,
        (Some(_), _, true) => DeleteStatus::WouldDelete,
        (None, Some(_), true) => DeleteStatus::WouldFinishEarlierDeletion,
        (_, _, false) => DeleteStatus::NotReached,
    };
    let mut planned = DeleteReportItem::new(&item.set_id, status);
    planned.set_path = named.set;
    planned.unfinished_deletion = named.deleting;
    if status != DeleteStatus::AlreadyGone {
        describe(resource_path, &item.set_id, environment).fill(&mut planned);
    }
    planned
}

/// What a set holds, as the overview reads it: its size and the backups in
/// it that are the only copy of something.
#[derive(Debug, Default)]
struct Described {
    size: u64,
    only_copy: Vec<BackupItem>,
}

impl Described {
    fn fill(self, item: &mut DeleteReportItem) {
        item.size = self.size;
        item.only_copy = self.only_copy;
    }
}

/// What everything `set_id` names holds, from the overview of that one id:
/// both folders' sizes from the stat walk, and the backups marked the only
/// copy. Nothing is hashed. What can't be read describes nothing; it is
/// only said, and never stops a delete.
fn describe(resource_path: &Path, set_id: &str, environment: &BackupEnvironment<'_>) -> Described {
    let Ok(overview) = overview_of_with(resource_path, set_id, environment) else {
        return Described::default();
    };
    Described {
        size: overview.sets.iter().map(|set| set.size).sum(),
        only_copy: overview
            .entries
            .iter()
            .filter(|entry| entry.only_copy)
            .map(|entry| entry.item.clone())
            .collect(),
    }
}

/// Delete what `item`'s id names, under the lock: look it up again and
/// check it, finish an unfinished deletion of the id, then rename the set
/// and walk it. A refusal now is this item's alone, since the items before
/// it may have gone already.
fn delete_item(
    resource_path: &Path,
    item: &DeleteItem,
    environment: &BackupEnvironment<'_>,
) -> DeleteReportItem {
    let set_id = item.set_id.as_str();
    let named = match check_item(resource_path, item, environment) {
        Ok(Ok(named)) => named,
        Ok(Err(refused)) => return refused.into_report_item(set_id),
        Err(error) => return failed_before_any_change(set_id, &error),
    };
    let mut done = DeleteReportItem::new(set_id, DeleteStatus::AlreadyGone);
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
fn failed_before_any_change(set_id: &str, error: &RabbitError) -> DeleteReportItem {
    let mut failed = DeleteReportItem::new(set_id, DeleteStatus::NotDeleted);
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
/// `RABBIT/backups-deleting/<id>`, and what is inside it.
struct DeleteGuard {
    root: PathBuf,
}

impl DeleteGuard {
    fn new(resource_path: &Path, set_id: &str) -> Self {
        Self {
            root: deleting_folder(resource_path).join(set_id),
        }
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

/// Save `report` as `RABBIT/logs/delete-backup-unix-<time>.json` and
/// `.txt` in `resource_path`, or `-2` and so on when a delete in the same
/// second saved one.
fn save_delete_report(resource_path: &Path, report: &DeleteReport) -> Result<SavedReportPaths> {
    save_new_json_and_text_reports(&default_report_path(resource_path, "delete-backup"), report)
}

/// Test-only hooks at the steps of a delete, so tests can change what is
/// on disk where a run could meet another program: right after the lock,
/// right after a set's rename, and right before the walk lists a folder.
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
