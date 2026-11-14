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
//! them: one for each row ticked to delete. The engine deletes sets, and
//! the rows of one set go together, while the row for the old copies of
//! RABBIT's records stands for several sets. So a row counts as deleted
//! when every set it takes is gone, and the rows that go together share
//! what came of their set. A row of several sets of which some went and
//! others didn't is said to be partly deleted, never to be as it was.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use rabbit_core::RabbitError;
use rabbit_core::backup_delete::{
    DeleteItem, DeleteReport, DeleteReportItem, DeleteRunOptions, DeleteStatus, NotDeletedReason,
    delete_backups,
};
use rabbit_core::cancel::CancelToken;
use rabbit_core::localization::Localizer;
use rabbit_core::progress::{ProgressEvent, ProgressReporter};
use rabbit_core::restore_refusal::RestoreRefusal;

use super::backups::{
    BackupChoice, BackupRow, WizardBackupList, and_list, backup_delete_items, backup_size_text,
    restore_refusal_text, say, text, tied_indices,
};
use super::progress::ProgressUiState;

/// Rows ticked to delete that the run takes together: the rows of one set,
/// or the one row that stands for several sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupDeleteGroup {
    /// What the rows call the backups, in list order.
    pub names: Vec<String>,
    /// The same, each with when it was made, as
    /// `wizard-backups-row-reference` says it.
    pub references: Vec<String>,
    /// What the progress page and Show details call each row: its name, or
    /// its reference when another ticked row has the same name, so that two
    /// key maps or two unfinished deletions can be told apart.
    pub labels: Vec<String>,
    /// The sets a delete of them takes, by id, each once.
    pub set_ids: Vec<String>,
}

/// What a delete run is given, copied out of the Backups list before the
/// question, so no borrow of the list is held across a modal dialog or into
/// the worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupDeleteRun {
    /// The REAPER resource folder whose backups go.
    pub resource_path: PathBuf,
    /// What the engine is given: each set once, with the fingerprint the
    /// list read, so a set that changed since is refused.
    pub items: Vec<DeleteItem>,
    /// The ticked rows, in list order, by what the run takes together.
    pub groups: Vec<BackupDeleteGroup>,
}

impl BackupDeleteRun {
    /// The backups ticked to delete, counted as Review and the question
    /// count them: one for each ticked row.
    pub fn backups(&self) -> usize {
        self.groups.iter().map(|group| group.names.len()).sum()
    }
}

/// What a delete of the rows ticked for Delete in `list`, in the REAPER at
/// `resource_path`, takes: the engine's items with their fingerprints, and
/// the ticked rows grouped by what goes together.
pub fn backup_delete_run(
    localizer: &Localizer,
    resource_path: &Path,
    list: &WizardBackupList,
) -> BackupDeleteRun {
    let lists: Vec<&[String]> = list.rows.iter().map(|row| row.set_ids.as_slice()).collect();
    // The rows the question counts, so Done counts the same ones.
    let ticked: Vec<usize> = list.ticked_indices(BackupChoice::Delete).collect();
    let mut grouped: BTreeSet<usize> = BTreeSet::new();
    let mut groups = Vec::new();
    for &index in &ticked {
        if grouped.contains(&index) {
            continue;
        }
        let mut members = vec![index];
        members.extend(
            tied_indices(&lists, index)
                .into_iter()
                .filter(|other| ticked.contains(other)),
        );
        members.sort_unstable();
        grouped.extend(members.iter().copied());
        let rows: Vec<&BackupRow> = members.iter().map(|&member| &list.rows[member]).collect();
        let mut set_ids: Vec<String> = Vec::new();
        for id in rows.iter().flat_map(|row| row.set_ids.iter()) {
            if !set_ids.contains(id) {
                set_ids.push(id.clone());
            }
        }
        groups.push(BackupDeleteGroup {
            names: rows.iter().map(|row| row.name.clone()).collect(),
            references: rows.iter().map(|row| reference(localizer, row)).collect(),
            labels: rows.iter().map(|row| label(localizer, list, row)).collect(),
            set_ids,
        });
    }
    BackupDeleteRun {
        resource_path: resource_path.to_path_buf(),
        items: backup_delete_items(list),
        groups,
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
                ("package", (&label(localizer, list, row)).into()),
                ("message", (&action).into()),
            ],
        ));
    }
    lines.join("\n")
}

/// What the progress page calls one group of ticked rows while a delete
/// runs, and how many sets the group takes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeletingBackups {
    /// The rows' labels, joined in a sentence.
    names: String,
    /// How many rows there are.
    count: usize,
    /// How many sets the run deletes for them.
    sets: usize,
}

/// What the progress page knows of a delete while it runs, for its status
/// and log lines: what each group of ticked rows is called, the group each
/// set belongs to, the group whose start it said last, and what came of
/// each group's sets so far. Empty for every other run.
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
/// [`ProgressUiState::with_backup_delete`]).
pub fn backup_delete_progress(
    localizer: &Localizer,
    run: &BackupDeleteRun,
) -> BackupDeleteProgress {
    let mut progress = BackupDeleteProgress::default();
    for (index, group) in run.groups.iter().enumerate() {
        progress.groups.push(DeletingBackups {
            names: and_list(localizer, &group.labels),
            count: group.labels.len(),
            sets: group.set_ids.len(),
        });
        progress.ended.push(SetsEnded::default());
        for set_id in &group.set_ids {
            progress.group_of.insert(set_id.clone(), index);
        }
    }
    progress
}

/// The status line and the log line a delete's `event` adds to the
/// progress page, once [`ProgressUiState::record`] has counted it in:
/// "Deleting the backup: …" when the first set of a group of ticked rows
/// starts, and in the log only, what came of the group once the run is done
/// with its last set. The row for the old copies of RABBIT's records stands
/// for several sets, and is said once at its start and once at its end.
/// Two rows of the same name are each said, since they are two groups.
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
                let end = GroupStatus::of(&[*status]);
                return (None, delete_log_line(localizer, set_id, end, *bytes_freed));
            };
            let ended = &mut progress.ended[group];
            ended.statuses.push(*status);
            ended.freed += bytes_freed;
            if ended.statuses.len() < progress.groups[group].sets {
                return (None, None);
            }
            let end = GroupStatus::of(&ended.statuses);
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
    end: GroupStatus,
    freed: u64,
) -> Option<String> {
    let key = match end.said() {
        DeleteStatus::Deleted => "wizard-progress-log-delete-deleted",
        DeleteStatus::FinishedEarlierDeletion => "wizard-progress-log-delete-finished",
        DeleteStatus::AlreadyGone => "wizard-progress-log-delete-already-gone",
        DeleteStatus::PartlyDeleted => "wizard-progress-log-delete-partly",
        DeleteStatus::NotDeleted | DeleteStatus::Refused => "wizard-progress-log-delete-failed",
        // A real run never reports these for a set it worked on.
        DeleteStatus::WouldDelete
        | DeleteStatus::WouldFinishEarlierDeletion
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

/// What came of a group of ticked rows as a whole, from the status each set
/// it takes ended with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GroupStatus {
    /// The first of these any set has, in this order, so one set that
    /// wasn't deleted keeps the group from counting as deleted. A group
    /// none of whose sets the report names wasn't reached.
    status: DeleteStatus,
    /// Some of the group's sets went in this run, wholly or in part, while
    /// another wasn't deleted or wasn't reached. Only a group with the row
    /// for the old copies of RABBIT's records, which stands for several
    /// sets, can end so.
    part_went: bool,
}

impl GroupStatus {
    fn of(statuses: &[DeleteStatus]) -> Self {
        const ORDER: [DeleteStatus; 8] = [
            DeleteStatus::Refused,
            DeleteStatus::NotDeleted,
            DeleteStatus::PartlyDeleted,
            DeleteStatus::NotReached,
            DeleteStatus::WouldDelete,
            DeleteStatus::WouldFinishEarlierDeletion,
            DeleteStatus::Deleted,
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
                        | DeleteStatus::FinishedEarlierDeletion
                        | DeleteStatus::PartlyDeleted
                )
            });
        Self { status, part_went }
    }

    /// The status Show details and the log give the group: partly deleted
    /// when part of it went, never "not deleted" or "not reached" then.
    fn said(self) -> DeleteStatus {
        if self.part_went {
            DeleteStatus::PartlyDeleted
        } else {
            self.status
        }
    }
}

/// What came of one group of ticked rows: of every set it takes.
struct GroupOutcome<'a> {
    group: &'a BackupDeleteGroup,
    /// The group's sets as the report lists them.
    sets: Vec<&'a DeleteReportItem>,
    end: GroupStatus,
    /// Bytes the run removed of the group's sets.
    freed: u64,
}

impl<'a> GroupOutcome<'a> {
    fn of(group: &'a BackupDeleteGroup, report: &'a DeleteReport) -> Self {
        let sets: Vec<&DeleteReportItem> = group
            .set_ids
            .iter()
            .filter_map(|id| report.sets.iter().find(|set| &set.set_id == id))
            .collect();
        let statuses: Vec<DeleteStatus> = sets.iter().map(|set| set.status).collect();
        Self {
            group,
            end: GroupStatus::of(&statuses),
            freed: sets.iter().map(|set| set.bytes_freed).sum(),
            sets,
        }
    }

    fn rows(&self) -> usize {
        self.group.names.len()
    }

    /// The sets whose status is the group's: those that say why the group
    /// came out as it did.
    fn telling_sets(&self) -> impl Iterator<Item = &&'a DeleteReportItem> {
        self.sets
            .iter()
            .filter(move |set| set.status == self.end.status)
    }

    /// Why the group wasn't deleted, when nothing of it went.
    fn not_deleted_reason(&self) -> Option<NotDeletedReason> {
        (self.end.status == DeleteStatus::NotDeleted && !self.end.part_went).then(|| {
            self.telling_sets()
                .find_map(|set| set.not_deleted)
                .unwrap_or(NotDeletedReason::Other)
        })
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
    let outcomes: Vec<GroupOutcome<'_>> = run
        .groups
        .iter()
        .map(|group| GroupOutcome::of(group, report))
        .collect();
    let rows = |wanted: &dyn Fn(&GroupOutcome<'_>) -> bool| -> usize {
        outcomes
            .iter()
            .filter(|outcome| wanted(outcome))
            .map(GroupOutcome::rows)
            .sum()
    };
    let deleted = rows(&|outcome| {
        matches!(
            outcome.end.status,
            DeleteStatus::Deleted | DeleteStatus::FinishedEarlierDeletion
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
    let reasons_by_count = [
        (NotDeletedReason::InUse, "wizard-done-delete-in-use"),
        (
            NotDeletedReason::Permission,
            "wizard-done-delete-permission",
        ),
        (
            NotDeletedReason::EarlierDeletionLeft,
            "wizard-done-delete-earlier-deletion-left",
        ),
        (NotDeletedReason::Other, "wizard-done-delete-other"),
    ];
    for (reason, key) in reasons_by_count {
        let count = rows(&|outcome| outcome.not_deleted_reason() == Some(reason));
        if count > 0 {
            reasons.push(say(
                localizer,
                key,
                &[("count", count.into()), ("total", total.into())],
            ));
        }
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
        for refusal in outcome
            .telling_sets()
            .filter_map(|set| set.refusal.as_ref())
        {
            let line = say(
                localizer,
                "wizard-done-delete-refused-backup",
                &[
                    (
                        "backup",
                        (&and_list(localizer, &outcome.group.references)).into(),
                    ),
                    ("count", outcome.rows().into()),
                    ("reason", (&delete_refusal_text(localizer, refusal)).into()),
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
    detail_lines.extend(run.groups.iter().map(|group| {
        say(
            localizer,
            "wizard-summary-delete-backup",
            &[
                ("backups", (&and_list(localizer, &group.labels)).into()),
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

/// The folder `error` is about, when it is one a delete reads before it
/// changes anything: the resource folder at `resource_path`, its `RABBIT`
/// folder, or the `backups` or `backups-deleting` folder in that. A delete
/// stopped by one of them deleted nothing, and the user can do something
/// about it.
fn unreadable_folder<'a>(resource_path: &Path, error: &'a RabbitError) -> Option<&'a Path> {
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

/// "Show details" after a delete: the target, then for each group of
/// ticked backups what came of it, with what it freed and, for one that
/// didn't go, why and what is left where; then what the run freed in all
/// and where its report is.
fn summarize_backup_delete(
    localizer: &Localizer,
    run: &BackupDeleteRun,
    report: &DeleteReport,
    outcomes: &[GroupOutcome<'_>],
) -> Vec<String> {
    let mut lines = vec![target_line(localizer, &run.resource_path)];
    for outcome in outcomes {
        lines.push(say(
            localizer,
            "wizard-summary-delete-backup",
            &[
                (
                    "backups",
                    (&and_list(localizer, &outcome.group.labels)).into(),
                ),
                (
                    "status",
                    (&delete_status_label(localizer, outcome.end.said())).into(),
                ),
            ],
        ));
        // The explanation sits indented below the backup's line. The indent
        // isn't in the messages: Fluent drops blanks at the start of a value.
        let mut below: Vec<String> = Vec::new();
        if outcome.freed > 0 {
            let size = backup_size_text(localizer, outcome.freed);
            below.push(say(
                localizer,
                "wizard-summary-delete-freed",
                &[("size", (&size).into())],
            ));
        }
        let explanation = explain(localizer, report, outcome);
        // Rows that go together are one backup on disk, which the lines
        // after this call "it".
        if outcome.rows() > 1 && !explanation.is_empty() {
            below.push(text(localizer, "wizard-summary-delete-together"));
        }
        below.extend(explanation);
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

/// Why a group came out as it did and what is left of it where, one
/// sentence a line, each once: what happened to the backup first, then
/// why, then the error as the computer gave it. Nothing for a group that is
/// gone.
fn explain(
    localizer: &Localizer,
    report: &DeleteReport,
    outcome: &GroupOutcome<'_>,
) -> Vec<String> {
    let path = |path: &Option<PathBuf>| {
        path.as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    };
    let error = |set: &DeleteReportItem| {
        set.error.as_ref().map(|error| {
            say(
                localizer,
                "wizard-summary-error",
                &[("message", error.into())],
            )
        })
    };
    let mut lines: Vec<String> = Vec::new();
    match outcome.end.status {
        DeleteStatus::NotDeleted => {
            let state = if outcome.end.part_went {
                "wizard-summary-delete-part-left"
            } else {
                "wizard-summary-delete-nothing-changed"
            };
            push_once(&mut lines, Some(text(localizer, state)));
            for set in outcome.telling_sets() {
                let reason = match set.not_deleted.unwrap_or(NotDeletedReason::Other) {
                    NotDeletedReason::InUse => {
                        Some(text(localizer, "wizard-summary-delete-in-use"))
                    }
                    NotDeletedReason::Permission => {
                        Some(text(localizer, "wizard-summary-delete-permission"))
                    }
                    NotDeletedReason::EarlierDeletionLeft => Some(say(
                        localizer,
                        "wizard-summary-delete-earlier-deletion-left",
                        &[("path", (&path(&set.left_in)).into())],
                    )),
                    // The error below says what went wrong.
                    NotDeletedReason::Other => None,
                };
                push_once(&mut lines, reason);
                push_once(&mut lines, error(set));
            }
        }
        DeleteStatus::PartlyDeleted => {
            for set in outcome.telling_sets() {
                push_once(
                    &mut lines,
                    Some(say(
                        localizer,
                        "wizard-summary-delete-left-in",
                        &[("path", (&path(&set.left_in)).into())],
                    )),
                );
                push_once(&mut lines, error(set));
            }
        }
        DeleteStatus::Refused => {
            for set in outcome.telling_sets() {
                let refusal = set
                    .refusal
                    .as_ref()
                    .map(|refusal| delete_refusal_text(localizer, refusal));
                push_once(&mut lines, refusal);
            }
        }
        DeleteStatus::NotReached => push_once(
            &mut lines,
            Some(text(
                localizer,
                if report.refused {
                    "wizard-summary-delete-not-reached-refused"
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
        | DeleteStatus::WouldFinishEarlierDeletion => {}
    }
    lines
}

/// Add `line` to `lines` unless it is there already: several sets of one
/// group often have the same thing said of them.
fn push_once(lines: &mut Vec<String>, line: Option<String>) {
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
        DeleteStatus::Deleted => "wizard-delete-status-deleted",
        DeleteStatus::FinishedEarlierDeletion => "wizard-delete-status-finished-earlier-deletion",
        DeleteStatus::AlreadyGone => "wizard-delete-status-already-gone",
        DeleteStatus::WouldDelete => "wizard-delete-status-would-delete",
        DeleteStatus::WouldFinishEarlierDeletion => {
            "wizard-delete-status-would-finish-earlier-deletion"
        }
        DeleteStatus::NotDeleted => "wizard-delete-status-not-deleted",
        DeleteStatus::PartlyDeleted => "wizard-delete-status-partly-deleted",
        DeleteStatus::Refused => "wizard-delete-status-refused",
        DeleteStatus::NotReached => "wizard-delete-status-not-reached",
    };
    text(localizer, key)
}

/// A delete's refusal as a sentence in the user's language: of one backup,
/// which the sentence calls "it", or of the whole run. These are the
/// refusals a delete gives; they mean what
/// [`RestoreRefusal::delete_text`] says for the command line, without its
/// command-line syntax.
pub fn delete_refusal_text(localizer: &Localizer, refusal: &RestoreRefusal) -> String {
    let key = format!("wizard-delete-refusal-{}", refusal.code());
    match refusal {
        RestoreRefusal::InvalidId { .. }
        | RestoreRefusal::DiffersInCase { .. }
        | RestoreRefusal::EndsInDotOrSpace { .. }
        | RestoreRefusal::NotYours { .. }
        | RestoreRefusal::ChangedSinceListed { .. }
        | RestoreRefusal::LockHeld { .. }
        | RestoreRefusal::AsRoot => text(localizer, &key),
        RestoreRefusal::IsLink { path } => say(
            localizer,
            &key,
            &[("path", (&path.display().to_string()).into())],
        ),
        // A delete refuses for no other reason. Should one come, it is said
        // as a restore says it, as the command line's text does.
        _ => restore_refusal_text(localizer, refusal),
    }
}

/// What the progress page and Show details call `row`, ticked to delete in
/// `list`: its name, or its reference when another row ticked to delete has
/// the same name, so that two key maps or two unfinished deletions can be
/// told apart.
fn label(localizer: &Localizer, list: &WizardBackupList, row: &BackupRow) -> String {
    let same_name = list
        .ticked_rows(BackupChoice::Delete)
        .filter(|other| other.name == row.name)
        .count();
    if same_name > 1 {
        reference(localizer, row)
    } else {
        row.name.clone()
    }
}

/// What a row is called with when its backup was made, so two backups of
/// one name can be told apart.
fn reference(localizer: &Localizer, row: &BackupRow) -> String {
    say(
        localizer,
        "wizard-backups-row-reference",
        &[("backup", (&row.name).into()), ("when", (&row.when).into())],
    )
}

fn target_line(localizer: &Localizer, path: &Path) -> String {
    say(
        localizer,
        "wizard-summary-target",
        &[("path", (&path.display().to_string()).into())],
    )
}
