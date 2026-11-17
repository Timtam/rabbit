//! Deleting backups from the wizard, apart from wxWidgets: what the Delete
//! button hands the engine, copied out of the Backups list before the
//! question is asked, what the progress page says while it runs, and what
//! the Done page says afterwards.
//!
//! The engine, `rabbit_core::backup_delete::delete_backups`, saves its own
//! report, `RABBIT/logs/delete-backup-unix-<seconds>.json` and `.txt` in the
//! resource folder, for every real run that got past its checks: the same
//! report `rabbit delete-backup --apply` saves. Done points to that file
//! rather than writing a second one. A run refused before anything changed
//! saves none, from the wizard as from the command line.
//!
//! Done counts backups as Review and the question before the delete count
//! them: one for each row ticked to delete, each on its own. A row gives
//! the engine its own items: a package or the key map out of its set, or a
//! set's own backup, an unfinished deletion or the old copies of RABBIT's
//! records, whose sets go whole. The engine deletes a set whole when the
//! ticked rows name every backup it lists, and then the rows share that
//! result: the first of them carries the bytes freed. A row finds what came
//! of each of its items in the report by set and part. It counts as deleted
//! when everything it names is gone, and a row of several sets of which
//! some went and others didn't is said to be partly deleted, never to be as
//! it was.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use rabbit_core::RabbitError;
use rabbit_core::backup_delete::{
    DeleteItem, DeleteReport, DeleteReportItem, DeleteRunOptions, DeleteStatus, NotDeletedReason,
    delete_backups,
};
use rabbit_core::backup_overview::BackupItem;
use rabbit_core::cancel::CancelToken;
use rabbit_core::localization::Localizer;
use rabbit_core::progress::{ProgressEvent, ProgressReporter};
use rabbit_core::restore_refusal::RestoreRefusal;

use super::backups::{
    BackupChoice, BackupRow, WizardBackupList, and_list, backup_size_text, restore_refusal_text,
    row_delete_items, say, text,
};
use super::packages::localized_package_display_name;
use super::progress::ProgressUiState;

/// One row ticked to delete, copied out of the list for the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupDeleteRow {
    /// What the row calls the backup.
    pub name: String,
    /// The same, with when it was made, as `wizard-backups-row-reference`
    /// says it.
    pub reference: String,
    /// What the progress page and Show details call it: its name, or its
    /// reference when another ticked row has the same name, so that two key
    /// maps or two unfinished deletions can be told apart.
    pub label: String,
    /// What the engine is given for it, with the fingerprints the list
    /// read: one item, or for the old copies of RABBIT's records one for
    /// each of their sets.
    pub items: Vec<DeleteItem>,
    /// The row is an unfinished deletion: what it finishes is its own, never
    /// an earlier deletion of the backup it is kept in.
    pub unfinished_deletion: bool,
}

/// What a delete run is given, copied out of the Backups list before the
/// question, so no borrow of the list is held across a modal dialog or into
/// the worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupDeleteRun {
    /// The REAPER resource folder whose backups go.
    pub resource_path: PathBuf,
    /// What the engine is given: every ticked row's items, row after row,
    /// in list order. The report gives one item back for each, in the same
    /// order.
    pub items: Vec<DeleteItem>,
    /// The ticked rows, in list order.
    pub rows: Vec<BackupDeleteRow>,
}

impl BackupDeleteRun {
    /// The backups ticked to delete, counted as Review and the question
    /// count them: one for each ticked row.
    pub fn backups(&self) -> usize {
        self.rows.len()
    }

    /// The sets the run goes through, each once, for the progress gauge:
    /// the engine starts and ends each set once, however many of its rows
    /// are ticked.
    pub fn set_count(&self) -> usize {
        let mut ids: Vec<&str> = self.items.iter().map(|item| item.set_id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    }
}

/// What a delete of the rows ticked for Delete in `list`, in the REAPER at
/// `resource_path`, takes: each ticked row with its own items and the
/// fingerprints the list read, and all their items for the engine.
pub fn backup_delete_run(
    localizer: &Localizer,
    resource_path: &Path,
    list: &WizardBackupList,
) -> BackupDeleteRun {
    // The rows the question counts, so Done counts the same ones.
    let rows: Vec<BackupDeleteRow> = list
        .ticked_rows(BackupChoice::Delete)
        .map(|row| BackupDeleteRow {
            name: row.name.clone(),
            reference: row_reference(localizer, row),
            label: row_label(localizer, list, row, BackupChoice::Delete),
            items: row_delete_items(row, &list.sets),
            unfinished_deletion: row
                .entry
                .as_ref()
                .is_some_and(|entry| matches!(entry.item, BackupItem::Deleting { .. })),
        })
        .collect();
    BackupDeleteRun {
        resource_path: resource_path.to_path_buf(),
        items: rows.iter().flat_map(|row| row.items.clone()).collect(),
        rows,
    }
}

/// The progress log's first lines for a delete: that it is starting, the
/// target, and each row ticked to delete with what its label says a delete
/// does to it. A row whose name another ticked row has too is named with
/// its date.
pub fn backup_delete_progress_details_for_start(
    localizer: &Localizer,
    resource_path: &Path,
    list: &WizardBackupList,
) -> String {
    let mut lines = vec![
        text(localizer, "wizard-progress-details-delete-starting"),
        say(
            localizer,
            "wizard-review-target",
            &[("path", (&resource_path.display().to_string()).into())],
        ),
    ];
    for row in list.ticked_rows(BackupChoice::Delete) {
        let only_copy = row.entry.as_ref().is_some_and(|entry| entry.only_copy);
        let action = text(
            localizer,
            if only_copy {
                "action-delete-only-copy"
            } else {
                "action-delete"
            },
        );
        lines.push(say(
            localizer,
            "wizard-summary-package-message",
            &[
                (
                    "package",
                    (&row_label(localizer, list, row, BackupChoice::Delete)).into(),
                ),
                ("message", (&action).into()),
            ],
        ));
    }
    lines.join("\n")
}

/// What the progress page calls the ticked rows of one set while a delete
/// runs, or the one row that stands for several sets, and how many sets
/// they are in.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeletingBackups {
    /// The rows' labels, joined in a sentence.
    names: String,
    /// How many rows there are.
    count: usize,
    /// How many sets the run deletes from for them.
    sets: usize,
}

/// What the progress page knows of a delete while it runs, for its status
/// and log lines: what the ticked rows of each set are called, the group of
/// rows each set belongs to, the group whose start it said last, and what
/// came of each group's sets so far. Empty for every other run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackupDeleteProgress {
    groups: Vec<DeletingBackups>,
    group_of: HashMap<String, usize>,
    last_started: Option<usize>,
    ended: Vec<SetsEnded>,
}

/// The sets of one group the run is done with so far.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SetsEnded {
    statuses: Vec<DeleteStatus>,
    freed: u64,
}

/// What the progress page knows of the delete `run` before it starts (see
/// [`ProgressUiState::with_backup_delete`]). The engine starts and ends
/// each set once, for every ticked row of it together, so the rows of one
/// set are one group here, said by all their labels; the row for the old
/// copies of RABBIT's records is a group of its own, of several sets.
pub fn backup_delete_progress(
    localizer: &Localizer,
    run: &BackupDeleteRun,
) -> BackupDeleteProgress {
    let mut progress = BackupDeleteProgress::default();
    let mut labels: Vec<Vec<String>> = Vec::new();
    let mut sets: Vec<usize> = Vec::new();
    for row in &run.rows {
        let group = row
            .items
            .iter()
            .find_map(|item| progress.group_of.get(&item.set_id).copied())
            .unwrap_or_else(|| {
                labels.push(Vec::new());
                sets.push(0);
                labels.len() - 1
            });
        labels[group].push(row.label.clone());
        for item in &row.items {
            if !progress.group_of.contains_key(&item.set_id) {
                progress.group_of.insert(item.set_id.clone(), group);
                sets[group] += 1;
            }
        }
    }
    for (labels, sets) in labels.iter().zip(sets) {
        progress.groups.push(DeletingBackups {
            names: and_list(localizer, labels),
            count: labels.len(),
            sets,
        });
        progress.ended.push(SetsEnded::default());
    }
    progress
}

/// The status line and the log line a delete's `event` adds to the
/// progress page, once [`ProgressUiState::record`] has counted it in:
/// "Deleting the backup: …" when a set starts, naming every ticked row of
/// it, and in the log only, what came of them once the run is done with
/// the set. The row for the old copies of RABBIT's records stands for
/// several sets, and is said once at its start and once at its end. Two
/// rows of the same name in two sets are each said with their dates.
/// Neither line for any other event.
pub fn backup_delete_progress_lines(
    localizer: &Localizer,
    state: &mut ProgressUiState,
    event: &ProgressEvent,
) -> (Option<String>, Option<String>) {
    let progress = &mut state.backup_delete;
    match event {
        ProgressEvent::DeleteStarted { set_id } => {
            let group = progress.group_of.get(set_id).copied();
            if group.is_some() && group == progress.last_started {
                return (None, None);
            }
            progress.last_started = group;
            let (names, count) = match group {
                Some(group) => (
                    progress.groups[group].names.clone(),
                    progress.groups[group].count,
                ),
                // The run sends only the sets it was given; one it wasn't
                // given would be said by its id.
                None => (set_id.clone(), 1),
            };
            let line = say(
                localizer,
                "wizard-progress-status-deleting",
                &[("backups", (&names).into()), ("count", count.into())],
            );
            (Some(line.clone()), Some(line))
        }
        ProgressEvent::DeleteCompleted {
            set_id,
            status,
            bytes_freed,
        } => {
            let Some(group) = progress.group_of.get(set_id).copied() else {
                let end = EndStatus::of(&[*status]);
                return (None, delete_log_line(localizer, set_id, end, *bytes_freed));
            };
            let ended = &mut progress.ended[group];
            ended.statuses.push(*status);
            ended.freed += bytes_freed;
            if ended.statuses.len() < progress.groups[group].sets {
                return (None, None);
            }
            let end = EndStatus::of(&ended.statuses);
            let line = delete_log_line(localizer, &progress.groups[group].names, end, ended.freed);
            (None, line)
        }
        _ => (None, None),
    }
}

/// The log line for the backups called `names` once the run is done with
/// them, which came out as `end` and freed `freed` bytes.
fn delete_log_line(
    localizer: &Localizer,
    names: &str,
    end: EndStatus,
    freed: u64,
) -> Option<String> {
    let key = match end.said() {
        // A backup taken out of a set that stays is deleted too.
        DeleteStatus::Deleted | DeleteStatus::DeletedFromSet => {
            "wizard-progress-log-delete-deleted"
        }
        DeleteStatus::FinishedEarlierDeletion => "wizard-progress-log-delete-finished",
        DeleteStatus::AlreadyGone => "wizard-progress-log-delete-already-gone",
        DeleteStatus::PartlyDeleted => "wizard-progress-log-delete-partly",
        DeleteStatus::NotDeleted | DeleteStatus::NotDeletedFromSet | DeleteStatus::Refused => {
            "wizard-progress-log-delete-failed"
        }
        // A real run never reports these for a set it worked on.
        DeleteStatus::WouldDelete
        | DeleteStatus::WouldFinishEarlierDeletion
        | DeleteStatus::WouldDeleteFromSet
        | DeleteStatus::NotReached => return None,
    };
    let size = backup_size_text(localizer, freed);
    Some(say(
        localizer,
        key,
        &[("backups", names.into()), ("size", (&size).into())],
    ))
}

/// Delete what `run` takes, for good, on the calling thread: the engine's
/// real run, which saves its report in `RABBIT/logs` of the resource
/// folder. `progress` hears each set start and end, and `cancel` stops the
/// run between sets.
pub fn run_backup_delete(
    run: &BackupDeleteRun,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> rabbit_core::Result<DeleteReport> {
    delete_backups(
        &run.resource_path,
        &run.items,
        &DeleteRunOptions { dry_run: false },
        progress,
        cancel,
    )
}

/// How a delete of backups went, as the Done page sums it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupDeleteOutcomeStatus {
    /// Every ticked backup is gone, and RABBIT deleted at least one of
    /// them; the others were already gone.
    Deleted,
    /// Every ticked backup was gone before the run.
    AlreadyGone,
    /// RABBIT deleted some of the ticked backups, and not the others.
    CompletedWithProblems,
    /// RABBIT went through the ticked backups and deleted none of them
    /// whole.
    NothingDeleted,
    /// The user stopped the run. What it deleted before stays deleted.
    Stopped,
    /// RABBIT refused the whole run before anything changed: a ticked
    /// backup can't be deleted, another RABBIT is working on this REAPER,
    /// or RABBIT runs as root.
    Refused,
    /// RABBIT couldn't read the resource folder or its backup folders, or
    /// something else went wrong, before anything changed.
    Failed,
}

/// What the Done page shows after a delete of backups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardBackupDeleteOutcome {
    pub status: BackupDeleteOutcomeStatus,
    /// One sentence: the progress page's last status line, and the first
    /// line of the Done page.
    pub heading: String,
    /// The Done page's status text, which gets focus and is read: the
    /// heading, then why any backup wasn't deleted.
    pub status_text: String,
    /// What "Show details" shows: per backup what happened, then what the
    /// run freed and where its report is.
    pub detail_lines: Vec<String>,
    /// The text report the engine saved; `None` when it saved none.
    pub report_path: Option<PathBuf>,
}

/// What came of a row as a whole, or in the progress log of the rows of one
/// set, from the status each of its items, or sets, ended with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EndStatus {
    /// The first of these any of them has, in this order, so one set that
    /// wasn't deleted keeps a row from counting as deleted. A row the
    /// report says nothing of wasn't reached.
    status: DeleteStatus,
    /// Some of the row's sets went in this run, wholly or in part, while
    /// another wasn't deleted or wasn't reached. Only the row for the old
    /// copies of RABBIT's records, which stands for several sets, can end
    /// so.
    part_went: bool,
}

impl EndStatus {
    fn of(statuses: &[DeleteStatus]) -> Self {
        const ORDER: [DeleteStatus; 11] = [
            DeleteStatus::Refused,
            DeleteStatus::NotDeleted,
            DeleteStatus::NotDeletedFromSet,
            DeleteStatus::PartlyDeleted,
            DeleteStatus::NotReached,
            DeleteStatus::WouldDelete,
            DeleteStatus::WouldFinishEarlierDeletion,
            DeleteStatus::WouldDeleteFromSet,
            DeleteStatus::Deleted,
            DeleteStatus::DeletedFromSet,
            DeleteStatus::FinishedEarlierDeletion,
        ];
        let status = if statuses.is_empty() {
            DeleteStatus::NotReached
        } else {
            ORDER
                .into_iter()
                .find(|status| statuses.contains(status))
                .unwrap_or(DeleteStatus::AlreadyGone)
        };
        let part_went = matches!(status, DeleteStatus::NotDeleted | DeleteStatus::NotReached)
            && statuses.iter().any(|status| {
                matches!(
                    status,
                    DeleteStatus::Deleted
                        | DeleteStatus::DeletedFromSet
                        | DeleteStatus::FinishedEarlierDeletion
                        | DeleteStatus::PartlyDeleted
                )
            });
        Self { status, part_went }
    }

    /// The status Show details and the log give: partly deleted when part
    /// of it went, never "not deleted" or "not reached" then.
    fn said(self) -> DeleteStatus {
        if self.part_went {
            DeleteStatus::PartlyDeleted
        } else {
            self.status
        }
    }
}

/// What came of one ticked row: of each item it gave the engine.
struct RowOutcome<'a> {
    row: &'a BackupDeleteRow,
    /// What the report says of each of the row's items, found by set and
    /// part (see [`report_item`]).
    items: Vec<&'a DeleteReportItem>,
    end: EndStatus,
    /// Bytes the run removed for the row's items.
    freed: u64,
    /// Whether those bytes are the row's own. Not when the row shares the
    /// deletion of a whole set with other ticked rows, as when they name
    /// every backup of it: the engine gives the set's bytes to the first of
    /// them, but they are every row's. Only the total says them then.
    own_bytes: bool,
}

impl<'a> RowOutcome<'a> {
    /// What came of each row of `run`, in its order, as `report` says.
    fn of_run(run: &'a BackupDeleteRun, report: &'a DeleteReport) -> Vec<Self> {
        let mut at = 0;
        let found: Vec<Vec<&DeleteReportItem>> = run
            .rows
            .iter()
            .map(|row| {
                row.items
                    .iter()
                    .filter_map(|item| {
                        let found = report_item(report, at, item);
                        at += 1;
                        found
                    })
                    .collect()
            })
            .collect();
        run.rows
            .iter()
            .zip(found)
            .enumerate()
            .map(|(index, (row, items))| {
                let statuses: Vec<DeleteStatus> = items.iter().map(|item| item.status).collect();
                // A set that went whole, or would, for several ticked rows.
                let shares_a_whole_set = items.iter().any(|ended| {
                    !takes_a_part(ended.status)
                        && run.rows.iter().enumerate().any(|(other, other_row)| {
                            other != index
                                && other_row
                                    .items
                                    .iter()
                                    .any(|item| item.set_id == ended.set_id)
                        })
                });
                Self {
                    row,
                    end: EndStatus::of(&statuses),
                    freed: items.iter().map(|item| item.bytes_freed).sum(),
                    own_bytes: !shares_a_whole_set,
                    items,
                }
            })
            .collect()
    }

    /// The row's items whose status is the row's: those that say why it
    /// came out as it did.
    fn telling_items(&self) -> impl Iterator<Item = &&'a DeleteReportItem> {
        self.items
            .iter()
            .filter(move |item| item.status == self.end.status)
    }

    /// Why the row wasn't deleted, when nothing of it went, and whether
    /// that is about its set's record of what it holds: a package or the
    /// key map stays in its set when that record couldn't be saved without
    /// it.
    fn not_deleted(&self) -> Option<(NotDeletedReason, bool)> {
        let from_set = match self.end.status {
            DeleteStatus::NotDeleted => false,
            DeleteStatus::NotDeletedFromSet => true,
            _ => return None,
        };
        if self.end.part_went {
            return None;
        }
        let reason = self
            .telling_items()
            .find_map(|item| item.not_deleted)
            .unwrap_or(NotDeletedReason::Other);
        Some((reason, from_set))
    }

    /// Whether nothing of the row went only because the engine refused, or
    /// couldn't check, another ticked backup of the same set right before
    /// that set's turn: it then leaves every backup of the set as it was,
    /// and goes on with the next set, so the run didn't stop.
    fn kept_with_another(&self, report: &DeleteReport) -> bool {
        !report.refused
            && self.end.status == DeleteStatus::NotReached
            && !self.end.part_went
            && self.items.iter().any(|item| {
                item.status == DeleteStatus::NotReached
                    && report.sets.iter().any(|other| {
                        other.set_id == item.set_id
                            && matches!(
                                other.status,
                                DeleteStatus::Refused | DeleteStatus::NotDeleted
                            )
                    })
            })
    }

    /// The set whose one outcome the row's status and why are, when they
    /// are the set's rather than the row's own: a whole set that wasn't
    /// deleted or was deleted only in part, or a set whose record of what
    /// it holds couldn't be saved without its parts. Ticked rows of one
    /// such set share all of it, so Show details says it below the first
    /// of them only.
    fn shared_set(&self) -> Option<&'a str> {
        if self.end.part_went
            || !matches!(
                self.end.status,
                DeleteStatus::NotDeleted
                    | DeleteStatus::NotDeletedFromSet
                    | DeleteStatus::PartlyDeleted
            )
        {
            return None;
        }
        let mut sets = self
            .items
            .iter()
            .copied()
            .filter(|item| item.status == self.end.status)
            .map(|item| item.set_id.as_str());
        let first = sets.next()?;
        sets.all(|set| set == first).then_some(first)
    }

    /// Done's sentence for the row when it wasn't deleted, which Done
    /// counts among the rows of the same reason (see [`done_reason`]).
    fn done_reason(&self, report: &DeleteReport) -> Option<(u8, &'static str)> {
        match self.not_deleted() {
            Some((reason, from_set)) => Some(done_reason(reason, from_set)),
            // Show details says which other backup kept it.
            None if self.kept_with_another(report) => {
                Some(done_reason(NotDeletedReason::Other, false))
            }
            None => None,
        }
    }
}

/// Whether `status` is that of a package or the key map going out of a set
/// that stays, or in a dry run would.
fn takes_a_part(status: DeleteStatus) -> bool {
    matches!(
        status,
        DeleteStatus::DeletedFromSet
            | DeleteStatus::WouldDeleteFromSet
            | DeleteStatus::NotDeletedFromSet
    )
}

/// The report's item for `item`, the `at`-th item the run was given, found
/// by its set and its part, never by its set alone: ticked rows of one set
/// name the same set with other parts. The report lists one item for each
/// the run was given, in that order, so it is the one at `at` when that
/// names the same set and part; otherwise the first that does.
fn report_item<'a>(
    report: &'a DeleteReport,
    at: usize,
    item: &DeleteItem,
) -> Option<&'a DeleteReportItem> {
    let same = |ended: &&DeleteReportItem| ended.set_id == item.set_id && ended.part == item.part;
    report
        .sets
        .get(at)
        .filter(same)
        .or_else(|| report.sets.iter().find(same))
}

/// Done's sentence for rows not deleted for `reason`, with its place among
/// the others, as Done says them in the order of those numbers: for a
/// package or the key map that stayed in its set, `from_set`, about the
/// set's record of what it holds rather than the set.
fn done_reason(reason: NotDeletedReason, from_set: bool) -> (u8, &'static str) {
    match (reason, from_set) {
        (NotDeletedReason::InUse, false) => (0, "wizard-done-delete-in-use"),
        (NotDeletedReason::InUse, true) => (1, "wizard-done-delete-record-in-use"),
        (NotDeletedReason::Permission, false) => (2, "wizard-done-delete-permission"),
        (NotDeletedReason::Permission, true) => (3, "wizard-done-delete-record-permission"),
        (NotDeletedReason::EarlierDeletionLeft, _) => {
            (4, "wizard-done-delete-earlier-deletion-left")
        }
        (NotDeletedReason::Other, _) => (5, "wizard-done-delete-other"),
    }
}

/// The Done page's text for the delete `run` that ended in `result`.
pub fn wizard_backup_delete_outcome(
    localizer: &Localizer,
    run: &BackupDeleteRun,
    result: &rabbit_core::Result<DeleteReport>,
) -> WizardBackupDeleteOutcome {
    let report = match result {
        Ok(report) => report,
        Err(error) => return backup_delete_outcome_from_error(localizer, run, error),
    };
    let total = run.backups();
    let outcomes = RowOutcome::of_run(run, report);
    let rows = |wanted: &dyn Fn(&RowOutcome<'_>) -> bool| -> usize {
        outcomes.iter().filter(|outcome| wanted(outcome)).count()
    };
    let deleted = rows(&|outcome| {
        matches!(
            outcome.end.status,
            DeleteStatus::Deleted
                | DeleteStatus::DeletedFromSet
                | DeleteStatus::FinishedEarlierDeletion
        )
    });
    let gone_before = rows(&|outcome| outcome.end.status == DeleteStatus::AlreadyGone);
    let all_gone = outcomes.iter().all(|outcome| outcome.end.status.is_gone());
    // Part of a backup went, even where no whole one did.
    let part_gone = report.bytes_freed > 0
        || outcomes.iter().any(|outcome| {
            outcome.end.part_went || outcome.end.status == DeleteStatus::PartlyDeleted
        });
    let size = backup_size_text(localizer, report.bytes_freed);
    let (status, heading) = if report.refused {
        (
            BackupDeleteOutcomeStatus::Refused,
            text(localizer, "wizard-done-delete-status-nothing"),
        )
    } else if report.stopped && deleted == 0 && part_gone {
        (
            BackupDeleteOutcomeStatus::Stopped,
            say(
                localizer,
                "wizard-done-delete-status-stopped-partway",
                &[("count", total.into())],
            ),
        )
    } else if report.stopped {
        (
            BackupDeleteOutcomeStatus::Stopped,
            say(
                localizer,
                "wizard-done-delete-status-stopped",
                &[("deleted", deleted.into()), ("count", total.into())],
            ),
        )
    } else if all_gone && deleted > 0 {
        (
            BackupDeleteOutcomeStatus::Deleted,
            say(
                localizer,
                "wizard-done-delete-status-success",
                &[("count", deleted.into()), ("size", (&size).into())],
            ),
        )
    } else if all_gone {
        (
            BackupDeleteOutcomeStatus::AlreadyGone,
            say(
                localizer,
                "wizard-done-delete-status-already-gone",
                &[("count", total.into())],
            ),
        )
    } else if deleted > 0 {
        (
            BackupDeleteOutcomeStatus::CompletedWithProblems,
            say(
                localizer,
                "wizard-done-delete-status-partial",
                &[("deleted", deleted.into()), ("count", total.into())],
            ),
        )
    } else {
        (
            BackupDeleteOutcomeStatus::NothingDeleted,
            say(
                localizer,
                "wizard-done-delete-status-none",
                &[("count", total.into())],
            ),
        )
    };

    let mut reasons: Vec<String> = Vec::new();
    if report.refused && total > 1 {
        reasons.push(text(localizer, "wizard-done-delete-refused-run"));
    }
    if status != BackupDeleteOutcomeStatus::AlreadyGone && gone_before > 0 {
        reasons.push(say(
            localizer,
            "wizard-done-delete-already-gone",
            &[("count", gone_before.into())],
        ));
    }
    let mut by_reason: BTreeMap<(u8, &str), usize> = BTreeMap::new();
    for reason in outcomes
        .iter()
        .filter_map(|outcome| outcome.done_reason(report))
    {
        *by_reason.entry(reason).or_default() += 1;
    }
    for ((_, key), count) in by_reason {
        reasons.push(say(
            localizer,
            key,
            &[("count", count.into()), ("total", total.into())],
        ));
    }
    let counted = [
        (
            rows(&|outcome| {
                outcome.end.status == DeleteStatus::NotDeleted && outcome.end.part_went
            }),
            "wizard-done-delete-part-left",
        ),
        (
            rows(&|outcome| outcome.end.status == DeleteStatus::PartlyDeleted),
            "wizard-done-delete-partly",
        ),
        (
            rows(&|outcome| {
                outcome.end.status == DeleteStatus::NotReached && outcome.end.part_went
            }),
            "wizard-done-delete-stopped-partway",
        ),
        // A package or the key map that went out of its set, though files
        // of it stay there and still take up room, which no row lists any
        // more: said here, as the first line counts it as deleted.
        (
            rows(&|outcome| {
                outcome.end.status == DeleteStatus::DeletedFromSet
                    && outcome
                        .telling_items()
                        .any(|item| !item.files_left.is_empty())
            }),
            "wizard-done-delete-files-left",
        ),
    ];
    for (count, key) in counted {
        if count > 0 {
            reasons.push(say(
                localizer,
                key,
                &[("count", count.into()), ("total", total.into())],
            ));
        }
    }
    for outcome in outcomes
        .iter()
        .filter(|outcome| outcome.end.status == DeleteStatus::Refused)
    {
        for reason in outcome
            .telling_items()
            .filter_map(|item| item_refusal_text(localizer, outcome.row, item))
        {
            let line = say(
                localizer,
                "wizard-done-delete-refused-backup",
                &[
                    ("backup", (&outcome.row.reference).into()),
                    ("reason", (&reason).into()),
                ],
            );
            if !reasons.contains(&line) {
                reasons.push(line);
            }
        }
    }

    let mut blocks = vec![heading.clone()];
    if !reasons.is_empty() {
        blocks.push(reasons.join("\n"));
    }
    WizardBackupDeleteOutcome {
        status,
        heading,
        status_text: blocks.join("\n\n"),
        detail_lines: summarize_backup_delete(localizer, run, report, &outcomes),
        report_path: report
            .saved_report
            .as_ref()
            .map(|saved| saved.text_path.clone()),
    }
}

/// The Done page's text for a delete the engine refused or failed before
/// anything changed: another RABBIT holds the lock, RABBIT runs as root, the
/// resource folder or its backups folder can't be read. The engine saved no
/// report.
fn backup_delete_outcome_from_error(
    localizer: &Localizer,
    run: &BackupDeleteRun,
    error: &RabbitError,
) -> WizardBackupDeleteOutcome {
    let heading = text(localizer, "wizard-done-delete-status-nothing");
    let (status, reason, english) = match error {
        RabbitError::RestoreRefused(refusal) => (
            BackupDeleteOutcomeStatus::Refused,
            delete_refusal_text(localizer, refusal),
            refusal.delete_text(),
        ),
        other => {
            let reason = match unreadable_folder(&run.resource_path, other) {
                Some(folder) => say(
                    localizer,
                    "wizard-done-delete-folder-unreadable",
                    &[("path", (&folder.display().to_string()).into())],
                ),
                None => say(
                    localizer,
                    "wizard-done-delete-error",
                    &[("error", (&other.to_string()).into())],
                ),
            };
            (BackupDeleteOutcomeStatus::Failed, reason, other.to_string())
        }
    };
    let not_deleted = delete_status_label(localizer, DeleteStatus::NotDeleted);
    let mut detail_lines = vec![target_line(localizer, &run.resource_path)];
    detail_lines.extend(run.rows.iter().map(|row| {
        say(
            localizer,
            "wizard-summary-delete-backup",
            &[
                ("backup", (&row.label).into()),
                ("status", (&not_deleted).into()),
            ],
        )
    }));
    // The error as it is, in English, for a bug report.
    detail_lines.push(say(
        localizer,
        "wizard-summary-error",
        &[("message", (&english).into())],
    ));
    WizardBackupDeleteOutcome {
        status,
        status_text: format!("{heading}\n\n{reason}"),
        heading,
        detail_lines,
        report_path: None,
    }
}

/// The folder `error` is about, when it is one a delete or a restore reads
/// before it changes anything: the resource folder at `resource_path`, its
/// `RABBIT` folder, or the `backups` or `backups-deleting` folder in that. A
/// run stopped by one of them changed nothing, and the user can do
/// something about it.
pub(super) fn unreadable_folder<'a>(
    resource_path: &Path,
    error: &'a RabbitError,
) -> Option<&'a Path> {
    let RabbitError::Io { path, .. } = error else {
        return None;
    };
    let rabbit = resource_path.join("RABBIT");
    let folders = [
        resource_path.to_path_buf(),
        rabbit.join("backups"),
        rabbit.join("backups-deleting"),
        rabbit,
    ];
    folders.contains(path).then_some(path.as_path())
}

/// "Show details" after a delete: the target, then for each ticked backup
/// what came of it, with what it freed when that is its own and, for one
/// that didn't go, why and what is left where; then what the run freed in
/// all and where its report is.
fn summarize_backup_delete(
    localizer: &Localizer,
    run: &BackupDeleteRun,
    report: &DeleteReport,
    outcomes: &[RowOutcome<'_>],
) -> Vec<String> {
    let mut lines = vec![target_line(localizer, &run.resource_path)];
    // The sets whose unfinished deletion a ticked row of its own finishes,
    // and then those whose finishing the line below a row said already.
    let mut finished_said: BTreeSet<&str> = run
        .rows
        .iter()
        .filter(|row| row.unfinished_deletion)
        .flat_map(|row| row.items.iter().map(|item| item.set_id.as_str()))
        .collect();
    // The sets whose one outcome a row's lines said already (see
    // [`RowOutcome::shared_set`]), with that outcome and the row's name.
    let mut explained: Vec<(&str, DeleteStatus, &str)> = Vec::new();
    for outcome in outcomes {
        lines.push(say(
            localizer,
            "wizard-summary-delete-backup",
            &[
                ("backup", (&outcome.row.label).into()),
                (
                    "status",
                    (&delete_status_label(localizer, outcome.end.said())).into(),
                ),
            ],
        ));
        // The explanation sits indented below the backup's line. The indent
        // isn't in the messages: Fluent drops blanks at the start of a value.
        let mut below: Vec<String> = Vec::new();
        if outcome.own_bytes && outcome.freed > 0 {
            let size = backup_size_text(localizer, outcome.freed);
            below.push(say(
                localizer,
                "wizard-summary-delete-freed",
                &[("size", (&size).into())],
            ));
        }
        let shared = outcome.shared_set();
        let said_above = shared.and_then(|set| {
            explained
                .iter()
                .find(|(other, status, _)| *other == set && *status == outcome.end.status)
        });
        match said_above {
            Some((_, _, first)) => below.push(say(
                localizer,
                "wizard-summary-delete-same-as-above",
                &[("backup", (*first).into())],
            )),
            None => {
                below.extend(explain(localizer, report, outcome));
                if let Some(set) = shared {
                    explained.push((set, outcome.end.status, &outcome.row.label));
                }
            }
        }
        // A set that goes whole takes an unfinished deletion of its id with
        // it, though no ticked row may name that: said once, below the
        // first row of the set, so nothing goes unsaid.
        if outcome
            .items
            .iter()
            .any(|item| item.finished_earlier_deletion && finished_said.insert(&item.set_id))
        {
            below.push(text(
                localizer,
                "wizard-summary-delete-finished-earlier-deletion-too",
            ));
        }
        lines.extend(below.into_iter().map(|line| format!("  {line}")));
    }
    if !report.refused {
        let size = backup_size_text(localizer, report.bytes_freed);
        lines.push(say(
            localizer,
            "wizard-summary-delete-freed-total",
            &[("size", (&size).into())],
        ));
    }
    if let Some(saved) = &report.saved_report {
        lines.push(say(
            localizer,
            "wizard-summary-delete-report",
            &[("path", (&saved.text_path.display().to_string()).into())],
        ));
    }
    lines
}

/// Why a row came out as it did and what is left of it where, one sentence
/// a line, each once: what happened to the backup first, then why, then the
/// error as the computer gave it. For a package or the key map that went
/// out of its set, the files of it that couldn't be removed. Nothing else
/// for a backup that is gone.
fn explain(localizer: &Localizer, report: &DeleteReport, outcome: &RowOutcome<'_>) -> Vec<String> {
    let path = |path: &Option<PathBuf>| {
        path.as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    };
    let error = |item: &DeleteReportItem| {
        item.error.as_ref().map(|error| {
            say(
                localizer,
                "wizard-summary-error",
                &[("message", error.into())],
            )
        })
    };
    let mut lines: Vec<String> = Vec::new();
    match outcome.end.status {
        // A backup that couldn't go out of its set left the set as it was,
        // as a set that couldn't be moved does; what stopped it is the
        // set's record of what it holds, which couldn't be saved without it.
        DeleteStatus::NotDeleted | DeleteStatus::NotDeletedFromSet => {
            let from_set = outcome.end.status == DeleteStatus::NotDeletedFromSet;
            let state = if outcome.end.part_went {
                "wizard-summary-delete-part-left"
            } else {
                "wizard-summary-delete-nothing-changed"
            };
            push_once(&mut lines, Some(text(localizer, state)));
            for item in outcome.telling_items() {
                let reason = match (
                    item.not_deleted.unwrap_or(NotDeletedReason::Other),
                    from_set,
                ) {
                    (NotDeletedReason::InUse, false) => {
                        Some(text(localizer, "wizard-summary-delete-in-use"))
                    }
                    (NotDeletedReason::InUse, true) => {
                        Some(text(localizer, "wizard-summary-delete-record-in-use"))
                    }
                    (NotDeletedReason::Permission, false) => {
                        Some(text(localizer, "wizard-summary-delete-permission"))
                    }
                    (NotDeletedReason::Permission, true) => {
                        Some(text(localizer, "wizard-summary-delete-record-permission"))
                    }
                    (NotDeletedReason::EarlierDeletionLeft, _) => Some(say(
                        localizer,
                        "wizard-summary-delete-earlier-deletion-left",
                        &[("path", (&path(&item.left_in)).into())],
                    )),
                    // The error below says what went wrong.
                    (NotDeletedReason::Other, _) => None,
                };
                push_once(&mut lines, reason);
                push_once(&mut lines, error(item));
            }
        }
        DeleteStatus::PartlyDeleted => {
            for item in outcome.telling_items() {
                push_once(
                    &mut lines,
                    Some(say(
                        localizer,
                        "wizard-summary-delete-left-in",
                        &[("path", (&path(&item.left_in)).into())],
                    )),
                );
                push_once(&mut lines, error(item));
            }
        }
        // Gone out of its set, though files of it that couldn't be removed
        // stay there, with no entry naming them, until the rest of the set
        // goes.
        DeleteStatus::DeletedFromSet => {
            for item in outcome
                .telling_items()
                .filter(|item| !item.files_left.is_empty())
            {
                push_once(
                    &mut lines,
                    Some(say(
                        localizer,
                        "wizard-summary-delete-files-left",
                        &[("count", item.files_left.len().into())],
                    )),
                );
                push_once(&mut lines, error(item));
            }
        }
        DeleteStatus::Refused => {
            for item in outcome.telling_items() {
                push_once(&mut lines, item_refusal_text(localizer, outcome.row, item));
            }
        }
        DeleteStatus::NotReached => push_once(
            &mut lines,
            Some(text(
                localizer,
                if report.refused {
                    "wizard-summary-delete-not-reached-refused"
                } else if outcome.kept_with_another(report) {
                    "wizard-summary-delete-not-reached-kept-with"
                } else if outcome.end.part_went {
                    "wizard-summary-delete-stopped-partway"
                } else {
                    "wizard-summary-delete-not-reached-stopped"
                },
            )),
        ),
        DeleteStatus::Deleted
        | DeleteStatus::FinishedEarlierDeletion
        | DeleteStatus::AlreadyGone
        | DeleteStatus::WouldDelete
        | DeleteStatus::WouldFinishEarlierDeletion
        | DeleteStatus::WouldDeleteFromSet => {}
    }
    lines
}

/// Add `line` to `lines` unless it is there already: several sets of one
/// group often have the same thing said of them.
pub(super) fn push_once(lines: &mut Vec<String>, line: Option<String>) {
    if let Some(line) = line
        && !lines.contains(&line)
    {
        lines.push(line);
    }
}

/// What came of a backup in a delete, in a few words, after its name in
/// "Show details".
pub fn delete_status_label(localizer: &Localizer, status: DeleteStatus) -> String {
    let key = match status {
        // A backup taken out of a set that stays is deleted, or would be,
        // as far as its row goes.
        DeleteStatus::Deleted | DeleteStatus::DeletedFromSet => "wizard-delete-status-deleted",
        DeleteStatus::FinishedEarlierDeletion => "wizard-delete-status-finished-earlier-deletion",
        DeleteStatus::AlreadyGone => "wizard-delete-status-already-gone",
        DeleteStatus::WouldDelete | DeleteStatus::WouldDeleteFromSet => {
            "wizard-delete-status-would-delete"
        }
        DeleteStatus::WouldFinishEarlierDeletion => {
            "wizard-delete-status-would-finish-earlier-deletion"
        }
        DeleteStatus::NotDeleted | DeleteStatus::NotDeletedFromSet => {
            "wizard-delete-status-not-deleted"
        }
        DeleteStatus::PartlyDeleted => "wizard-delete-status-partly-deleted",
        DeleteStatus::Refused => "wizard-delete-status-refused",
        DeleteStatus::NotReached => "wizard-delete-status-not-reached",
    };
    text(localizer, key)
}

/// Why the engine refused `item` of `row`, as a sentence (see
/// [`delete_refusal_text`]). The engine refuses every item of an id whose
/// unfinished deletion belongs to another account, also a package or the
/// key map kept in the set beside it: that is said of the unfinished
/// deletion, never as if the row's backup were another account's.
fn item_refusal_text(
    localizer: &Localizer,
    row: &BackupDeleteRow,
    item: &DeleteReportItem,
) -> Option<String> {
    let refusal = item.refusal.as_ref()?;
    let earlier_deletion_not_yours = matches!(refusal, RestoreRefusal::NotYours { .. })
        && item.set_path.is_none()
        && item.unfinished_deletion.is_some()
        && !row.unfinished_deletion;
    Some(if earlier_deletion_not_yours {
        text(
            localizer,
            "wizard-delete-refusal-not-yours-earlier-deletion",
        )
    } else {
        delete_refusal_text(localizer, refusal)
    })
}

/// A delete's refusal as a sentence in the user's language: of one backup,
/// which the sentence calls "it", or of the whole run. These are the
/// refusals a delete gives, those of a package or the key map that can't
/// go out of its set alone among them; they mean what
/// [`RestoreRefusal::delete_text`] says for the command line, without its
/// command-line syntax.
pub fn delete_refusal_text(localizer: &Localizer, refusal: &RestoreRefusal) -> String {
    let key = format!("wizard-delete-refusal-{}", refusal.code());
    let package = |package_id: &str| localized_package_display_name(localizer, package_id);
    match refusal {
        RestoreRefusal::InvalidId { .. }
        | RestoreRefusal::DiffersInCase { .. }
        | RestoreRefusal::EndsInDotOrSpace { .. }
        | RestoreRefusal::NotYours { .. }
        | RestoreRefusal::ChangedSinceListed { .. }
        | RestoreRefusal::LockHeld { .. }
        | RestoreRefusal::AsRoot
        | RestoreRefusal::BeingDeleted { .. }
        | RestoreRefusal::NoKeymapInSet { .. } => text(localizer, &key),
        RestoreRefusal::IsLink { path } => say(
            localizer,
            &key,
            &[("path", (&path.display().to_string()).into())],
        ),
        RestoreRefusal::NoManifest { package_id, .. }
        | RestoreRefusal::NoPackageRecords { package_id, .. }
        | RestoreRefusal::PackageNotInSet { package_id, .. } => say(
            localizer,
            &key,
            &[("package", (&package(package_id)).into())],
        ),
        RestoreRefusal::DamagedManifest { detail, .. } => {
            say(localizer, &key, &[("detail", detail.into())])
        }
        // A delete refuses for no other reason. Should one come, it is said
        // as a restore says it, as the command line's text does.
        _ => restore_refusal_text(localizer, refusal),
    }
}

/// What the progress page and Show details call `row`, ticked for `choice`
/// in `list`: its name, or its reference when another row ticked for the
/// same choice has the same name, so that two key maps or two unfinished
/// deletions can be told apart. A delete and a restore name their rows so.
pub(super) fn row_label(
    localizer: &Localizer,
    list: &WizardBackupList,
    row: &BackupRow,
    choice: BackupChoice,
) -> String {
    let same_name = list
        .ticked_rows(choice)
        .filter(|other| other.name == row.name)
        .count();
    if same_name > 1 {
        row_reference(localizer, row)
    } else {
        row.name.clone()
    }
}

/// What a row is called with when its backup was made, so two backups of
/// one name can be told apart.
pub(super) fn row_reference(localizer: &Localizer, row: &BackupRow) -> String {
    say(
        localizer,
        "wizard-backups-row-reference",
        &[("backup", (&row.name).into()), ("when", (&row.when).into())],
    )
}

pub(super) fn target_line(localizer: &Localizer, path: &Path) -> String {
    say(
        localizer,
        "wizard-summary-target",
        &[("path", (&path.display().to_string()).into())],
    )
}
