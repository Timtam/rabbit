//! Restoring backups from the wizard, apart from wxWidgets: what the
//! Restore button hands the engine, copied out of the Backups list before
//! anything else happens, what has to be closed before it may start, what
//! the progress page says while it runs, and what the Done page says
//! afterwards.
//!
//! The engine, `rabbit_core::rollback::restore_backups`, saves its own
//! report, `RABBIT/logs/restore-backup-unix-<seconds>.json` and `.txt` in
//! the resource folder, for every real run that got past the checks of the
//! whole run: the same report `rabbit restore-backup --apply` saves. Done
//! points to that file rather than writing a second one. A run refused as a
//! whole, or one whose every backup a check of its own refused, changed
//! nothing and saves none, from the wizard as from the command line.
//!
//! Done counts backups as Review counts them: one for each row ticked to
//! restore. The run restores each all or nothing, so a backup counts as
//! restored only when all of it came back.

use std::path::{Path, PathBuf};

use rabbit_core::RabbitError;
use rabbit_core::backup_overview::{BackupGroup, BackupItem, seconds_in_set_id};
use rabbit_core::cancel::CancelToken;
use rabbit_core::localization::{FluentArgument, Localizer};
use rabbit_core::progress::{ProgressEvent, ProgressReporter};
use rabbit_core::restore_refusal::RestoreRefusal;
use rabbit_core::rollback::{
    MoveOutAction, RestoreBackupActionKind, RestoreItem, RestoreReportItem, RestoreRunOptions,
    RestoreRunReport, RestoreStatus, SwappedEntry, SwappedPackage, restore_backups,
    restore_running_refusal, same_place,
};
use rabbit_core::version::Version;

use super::backups::{
    BackupChoice, BackupRow, RestoreFn, WizardBackupList, and_list, backup_date_text,
    keymap_origin_selector, local_utc_offset_at, package_backup_name, restore_refusal_text,
    same_version_other_variant, say, text, version_beside_text,
};
use super::backups_run::{push_once, row_label, target_line, unreadable_folder};
use super::packages::localized_package_display_name;
use super::progress::ProgressUiState;

/// What a ticked backup brings back, as the progress page and Done name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoredBackupKind {
    /// A package an uninstall moved into the backup.
    Uninstalled,
    /// An earlier version of a package, which the restore goes back to:
    /// the version the backup holds, when the overview knows it.
    Earlier { version: Option<Version> },
    /// A key map: `own` for the user's own, `default-keys` for REAPER's
    /// default keys, `other` for any other, as Review's words choose them.
    Keymap { origin: &'static str },
}

/// One row ticked to restore, as the run names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoringBackup {
    /// What the row calls the backup, such as "SWS Extension 2.14.0.7".
    pub name: String,
    /// What the progress page's log, Show details and Done's lines that
    /// start with it call it: its name, or its reference when another
    /// ticked row has the same name.
    pub label: String,
    /// What Done calls it inside a sentence: its label, but for a key map
    /// a phrase that starts in lower case where the language allows, such
    /// as "your key map from November 1, 2026, 6:40 PM", since a row's
    /// name, "Your key map", starts with a capital.
    pub in_sentence: String,
    /// The localized name of the package it is of, for a package's backup.
    pub package: Option<String>,
    pub kind: RestoredBackupKind,
    /// When the backup was made, as the user's language writes a date.
    pub date: Option<String>,
}

/// What a restore run is given, copied out of the Backups list before the
/// check of what runs, so no borrow of the list is held across a modal
/// dialog or into the worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRestoreRun {
    /// The REAPER resource folder the backups go back into.
    pub resource_path: PathBuf,
    /// What the engine is given: one item per row ticked to restore, in
    /// list order, with the fingerprint the list read, so a set that
    /// changed since is refused.
    pub items: Vec<RestoreItem>,
    /// The ticked rows, one for each item, in the same order.
    pub rows: Vec<RestoringBackup>,
}

impl BackupRestoreRun {
    /// The backups ticked to restore, counted as Review counts them: one
    /// for each ticked row.
    pub fn backups(&self) -> usize {
        self.items.len()
    }
}

/// What a restore of the rows ticked for Restore in `list`, into the
/// REAPER at `resource_path`, takes: the engine's items with their
/// fingerprints (see [`super::backups::backup_restore_items`]), and what the
/// run calls each ticked row.
pub fn backup_restore_run(
    localizer: &Localizer,
    resource_path: &Path,
    list: &WizardBackupList,
) -> BackupRestoreRun {
    let mut items = Vec::new();
    let mut rows = Vec::new();
    for row in list.ticked_rows(BackupChoice::Restore) {
        let Some(item) = row.restore_item.clone() else {
            continue;
        };
        items.push(item);
        rows.push(restoring_backup(localizer, list, row));
    }
    BackupRestoreRun {
        resource_path: resource_path.to_path_buf(),
        items,
        rows,
    }
}

/// What the run calls `row`, ticked to restore in `list`.
fn restoring_backup(
    localizer: &Localizer,
    list: &WizardBackupList,
    row: &BackupRow,
) -> RestoringBackup {
    let entry = row.entry.as_ref();
    let kind = match entry {
        Some(entry) if matches!(entry.item, BackupItem::Keymap { .. }) => {
            RestoredBackupKind::Keymap {
                origin: keymap_origin_selector(entry),
            }
        }
        Some(entry) if entry.group == BackupGroup::EarlierVersions => RestoredBackupKind::Earlier {
            version: entry.version.clone(),
        },
        _ => RestoredBackupKind::Uninstalled,
    };
    let label = row_label(localizer, list, row, BackupChoice::Restore);
    let in_sentence = match kind {
        RestoredBackupKind::Keymap { origin } => match &row.date {
            Some(date) => say(
                localizer,
                "wizard-done-restore-keymap-in-sentence",
                &[("origin", origin.into()), ("date", date.into())],
            ),
            None => say(
                localizer,
                "wizard-done-restore-keymap-in-sentence-undated",
                &[("origin", origin.into())],
            ),
        },
        RestoredBackupKind::Uninstalled | RestoredBackupKind::Earlier { .. } => label.clone(),
    };
    RestoringBackup {
        name: row.name.clone(),
        label,
        in_sentence,
        package: entry
            .and_then(|entry| entry.item.package_id())
            .map(|package_id| localized_package_display_name(localizer, package_id)),
        kind,
        date: row.date.clone(),
    }
}

/// The progress log's first lines for a restore: that it is starting, the
/// target, and each row ticked to restore with "Will restore".
pub fn backup_restore_progress_details_for_start(
    localizer: &Localizer,
    run: &BackupRestoreRun,
) -> String {
    let action = text(localizer, "action-restore");
    let mut lines = vec![
        text(localizer, "wizard-progress-details-restore-starting"),
        say(
            localizer,
            "wizard-review-target",
            &[("path", (&run.resource_path.display().to_string()).into())],
        ),
    ];
    for row in &run.rows {
        lines.push(say(
            localizer,
            "wizard-summary-package-message",
            &[
                ("package", (&row.label).into()),
                ("message", (&action).into()),
            ],
        ));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Before the run: what has to be closed.
// ---------------------------------------------------------------------------

/// The refusal the run of `run` would give right now for what runs: the
/// target's REAPER, every REAPER when a package it puts back serves every
/// REAPER, or a program a package's uninstall plan names. Asked when
/// Restore is pressed, while the user is still on Review and can do
/// something about it: the run checks the same again, by the same rules,
/// but only once the wizard has moved to Progress. It reads manifests and
/// file sizes, hashes nothing and changes nothing.
pub fn backup_restore_running_refusal(run: &BackupRestoreRun) -> Option<RestoreRefusal> {
    backup_restore_running_refusal_with(run, &restore_running_refusal)
}

/// The check before a restore, as [`restore_running_refusal`] makes it,
/// given the resource folder and the backups. The wizard passes that
/// function; its tests pass one on a made-up computer.
pub(crate) type RunningCheckFn<'a> = &'a dyn Fn(&Path, &[RestoreItem]) -> Option<RestoreRefusal>;

/// [`backup_restore_running_refusal`], with `check` asking about what
/// runs, for every backup `run` takes.
pub(crate) fn backup_restore_running_refusal_with(
    run: &BackupRestoreRun,
    check: RunningCheckFn<'_>,
) -> Option<RestoreRefusal> {
    check(&run.resource_path, &run.items)
}

/// The title and the text of the OK-only warning shown instead of starting
/// a restore while `refusal` stands in the way: close REAPER, every copy of
/// REAPER, or the programs named, then press Restore again. There is no
/// question: what a restore replaces goes into a backup first. `None` for
/// a refusal that isn't about what runs, which the run then says on Done.
pub fn restore_running_warning(
    localizer: &Localizer,
    refusal: &RestoreRefusal,
) -> Option<(String, String)> {
    let reaper_title = || text(localizer, "wizard-reaper-running-title");
    match refusal {
        RestoreRefusal::ReaperRunning {
            every_reaper_for, ..
        } if !every_reaper_for.is_empty() => {
            let names: Vec<String> = every_reaper_for
                .iter()
                .map(|id| localized_package_display_name(localizer, id))
                .collect();
            Some((
                reaper_title(),
                say(
                    localizer,
                    "wizard-reaper-running-restore-every-body",
                    &[("packages", (&and_list(localizer, &names)).into())],
                ),
            ))
        }
        RestoreRefusal::ReaperRunning { .. } => Some((
            reaper_title(),
            text(localizer, "wizard-reaper-running-restore-body"),
        )),
        RestoreRefusal::ProgramRunning { programs, .. } => Some((
            text(localizer, "wizard-programs-running-restore-title"),
            say(
                localizer,
                "wizard-programs-running-restore-body",
                &[("programs", (&and_list(localizer, programs)).into())],
            ),
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The run and its progress page.
// ---------------------------------------------------------------------------

/// How the wizard's restore runs: for real, hashing every copy before it
/// goes back, and never while REAPER runs.
pub fn backup_restore_run_options() -> RestoreRunOptions {
    RestoreRunOptions {
        dry_run: false,
        quick: false,
        allow_reaper_running: false,
    }
}

/// Restore what `run` takes, on the calling thread: the engine's real run,
/// which saves its report in `RABBIT/logs` of the resource folder.
/// `progress` hears each backup start and end, and `cancel` stops the run
/// between backups.
pub fn run_backup_restore(
    run: &BackupRestoreRun,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> rabbit_core::Result<RestoreRunReport> {
    run_backup_restore_with(run, progress, cancel, &restore_backups)
}

/// [`run_backup_restore`], with `restore` running the restore: every
/// backup `run` takes, with [`backup_restore_run_options`].
pub(crate) fn run_backup_restore_with(
    run: &BackupRestoreRun,
    progress: &ProgressReporter,
    cancel: &CancelToken,
    restore: RestoreFn<'_>,
) -> rabbit_core::Result<RestoreRunReport> {
    restore(
        &run.resource_path,
        &run.items,
        &backup_restore_run_options(),
        progress,
        cancel,
    )
}

/// What the progress page says of one backup of a restore.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RestoringLines {
    /// The backup's set and package, as the run's events name it. A key
    /// map has no package; no two ticked rows share both.
    set_id: String,
    package_id: Option<String>,
    /// The status and log line when it starts.
    start: String,
    /// What the log line at its end calls it.
    label: String,
}

/// What the progress page knows of a restore while it runs, for its status
/// and log lines: what to say of each backup, by its set and package, never
/// by matching text. Empty for every other run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackupRestoreProgress {
    backups: Vec<RestoringLines>,
}

/// What the progress page knows of the restore `run` before it starts (see
/// [`ProgressUiState::with_backup_restore`]).
pub fn backup_restore_progress(
    localizer: &Localizer,
    run: &BackupRestoreRun,
) -> BackupRestoreProgress {
    BackupRestoreProgress {
        backups: run
            .items
            .iter()
            .zip(&run.rows)
            .map(|(item, row)| RestoringLines {
                set_id: item.set_id.clone(),
                package_id: item.package_id().map(ToString::to_string),
                start: restoring_line(localizer, row),
                label: row.label.clone(),
            })
            .collect(),
    }
}

/// "Restoring SWS Extension 2.14.0.7…", "Going back to ReaPack 1.2.4…" or
/// "Restoring your key map from November 1, 2026, 6:40 PM…".
fn restoring_line(localizer: &Localizer, row: &RestoringBackup) -> String {
    let package_line = |kind: &str| {
        say(
            localizer,
            "wizard-progress-status-restoring",
            &[("kind", kind.into()), ("backup", (&row.label).into())],
        )
    };
    match &row.kind {
        RestoredBackupKind::Uninstalled => package_line("uninstalled"),
        RestoredBackupKind::Earlier { version: Some(_) } => package_line("earlier"),
        RestoredBackupKind::Earlier { version: None } => package_line("earlier-copy"),
        RestoredBackupKind::Keymap { origin } => match &row.date {
            Some(date) => say(
                localizer,
                "wizard-progress-status-restoring-keymap",
                &[("origin", (*origin).into()), ("date", date.into())],
            ),
            None => say(
                localizer,
                "wizard-progress-status-restoring-keymap-undated",
                &[("origin", (*origin).into())],
            ),
        },
    }
}

/// The status line and the log line a restore's `event` adds to the
/// progress page: the backup's start line, spoken, when it starts, and in
/// the log only, what came of it once it ends. Neither for any other
/// event. A backup the run wasn't given, which it never sends, is named by
/// its set.
pub fn backup_restore_progress_lines(
    localizer: &Localizer,
    state: &ProgressUiState,
    event: &ProgressEvent,
) -> (Option<String>, Option<String>) {
    let find = |set_id: &str, package_id: &Option<String>| {
        state
            .backup_restore
            .backups
            .iter()
            .find(|backup| backup.set_id == set_id && backup.package_id == *package_id)
    };
    match event {
        ProgressEvent::RestoreStarted { set_id, package_id } => {
            let line = match find(set_id, package_id) {
                Some(backup) => backup.start.clone(),
                None => say(
                    localizer,
                    "wizard-progress-status-restoring",
                    &[("kind", "uninstalled".into()), ("backup", set_id.into())],
                ),
            };
            (Some(line.clone()), Some(line))
        }
        ProgressEvent::RestoreCompleted {
            set_id,
            package_id,
            status,
        } => {
            let label = find(set_id, package_id).map_or(set_id.as_str(), |backup| &backup.label);
            let key = match status {
                RestoreStatus::Restored => "wizard-progress-log-restore-restored",
                RestoreStatus::NotRestored => "wizard-progress-log-restore-not-restored",
                RestoreStatus::FilesLeft => "wizard-progress-log-restore-files-left",
                RestoreStatus::PartlyRestored => "wizard-progress-log-restore-partly",
                // A real run never ends a backup so.
                RestoreStatus::WouldRestore | RestoreStatus::NotReached => return (None, None),
            };
            (None, Some(say(localizer, key, &[("backup", label.into())])))
        }
        _ => (None, None),
    }
}

// ---------------------------------------------------------------------------
// Done and Show details.
// ---------------------------------------------------------------------------

/// How a restore of backups went, as the Done page sums it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupRestoreOutcomeStatus {
    /// Every ticked backup came back.
    Restored,
    /// Some came back, and others didn't.
    CompletedWithProblems,
    /// None came back. Some may have failed partway, with files that
    /// couldn't go back as they were.
    NothingRestored,
    /// The user stopped the run. What it restored before stays restored.
    Stopped,
    /// RABBIT refused the whole run before anything changed: REAPER or a
    /// program runs, another RABBIT works on this REAPER, RABBIT runs as
    /// root, or a backup folder is a link.
    Refused,
    /// Something else went wrong before anything changed.
    Failed,
}

/// What the Done page shows after a restore of backups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardBackupRestoreOutcome {
    pub status: BackupRestoreOutcomeStatus,
    /// One sentence: the progress page's last status line, and the first
    /// line of the Done page.
    pub heading: String,
    /// The Done page's status text, which gets focus and is read: the
    /// heading, then why any backup didn't come back, then what else the
    /// restore changed, such as REAPER's language.
    pub status_text: String,
    /// What "Show details" shows: per backup what happened, then where the
    /// report is.
    pub detail_lines: Vec<String>,
    /// The text report the engine saved; `None` when it saved none.
    pub report_path: Option<PathBuf>,
}

/// The Done page's text for the restore `run` that ended in `result`, with
/// dates in this computer's local time.
pub fn wizard_backup_restore_outcome(
    localizer: &Localizer,
    run: &BackupRestoreRun,
    result: &rabbit_core::Result<RestoreRunReport>,
) -> WizardBackupRestoreOutcome {
    wizard_backup_restore_outcome_at(localizer, run, result, &local_utc_offset_at)
}

/// [`wizard_backup_restore_outcome`], with each date shifted by what
/// `local_offset` says local time was ahead of UTC at that moment, in
/// seconds.
pub fn wizard_backup_restore_outcome_at(
    localizer: &Localizer,
    run: &BackupRestoreRun,
    result: &rabbit_core::Result<RestoreRunReport>,
    local_offset: &dyn Fn(u64) -> i32,
) -> WizardBackupRestoreOutcome {
    let report = match result {
        Ok(report) => report,
        Err(error) => return backup_restore_outcome_from_error(localizer, run, error),
    };
    let total = run.backups();
    // The report has one backup per item, in the order given.
    let ended: Vec<(&RestoringBackup, Option<&RestoreReportItem>)> = run
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row, report.backups.get(index)))
        .collect();
    let count = |wanted: RestoreStatus| {
        ended
            .iter()
            .filter(|(_, item)| status_of(*item) == wanted)
            .count()
    };
    let restored = count(RestoreStatus::Restored);
    let changed_some = count(RestoreStatus::FilesLeft) + count(RestoreStatus::PartlyRestored) > 0;
    // The one backup, for the forms that name it.
    let only = (total == 1).then(|| &run.rows[0]);
    let named = |key: &str| {
        say(
            localizer,
            key,
            &[
                ("count", total.into()),
                (
                    "backup",
                    only.map_or("", |row| row.in_sentence.as_str()).into(),
                ),
            ],
        )
    };
    let (status, heading) = if report.stopped {
        (
            BackupRestoreOutcomeStatus::Stopped,
            say(
                localizer,
                "wizard-done-restore-status-stopped",
                &[("restored", restored.into()), ("count", total.into())],
            ),
        )
    } else if total > 0 && restored == total {
        let heading = match (only, ended.first()) {
            (Some(row), Some((_, item))) => one_restored_heading(localizer, row, *item),
            _ => say(
                localizer,
                "wizard-done-restore-status-success",
                &[("count", total.into())],
            ),
        };
        (BackupRestoreOutcomeStatus::Restored, heading)
    } else if restored > 0 {
        (
            BackupRestoreOutcomeStatus::CompletedWithProblems,
            say(
                localizer,
                "wizard-done-restore-status-partial",
                &[("restored", restored.into()), ("count", total.into())],
            ),
        )
    } else if changed_some {
        (
            BackupRestoreOutcomeStatus::NothingRestored,
            named("wizard-done-restore-status-none-changed"),
        )
    } else {
        (
            BackupRestoreOutcomeStatus::NothingRestored,
            named("wizard-done-restore-status-none"),
        )
    };

    let mut reasons: Vec<String> = Vec::new();
    let mut start_again = false;
    for (row, item) in &ended {
        let Some(item) = item else {
            continue;
        };
        let reason = |key: &str, extra: Option<(&str, FluentArgument<'_>)>| {
            let mut args: Vec<(&str, FluentArgument<'_>)> = vec![
                ("total", total.into()),
                ("backup", (&row.in_sentence).into()),
            ];
            args.extend(extra);
            say(localizer, key, &args)
        };
        let line = match item.status {
            RestoreStatus::NotRestored => Some(match &item.refusal {
                Some(refusal) => {
                    start_again |= tried_again_from_the_start(refusal);
                    let why = restore_refusal_text(localizer, refusal);
                    reason(
                        "wizard-done-restore-refused",
                        Some(("reason", (&why).into())),
                    )
                }
                None => reason("wizard-done-restore-failed", None),
            }),
            // What the restore replaced is kept as a backup only when it
            // replaced something: files it added where there were none
            // leave no backup.
            RestoreStatus::FilesLeft => Some(reason(
                if item.before_restore_set_id.is_some() {
                    "wizard-done-restore-files-left"
                } else {
                    "wizard-done-restore-files-left-no-backup"
                },
                None,
            )),
            RestoreStatus::PartlyRestored => Some(reason("wizard-done-restore-partly", None)),
            // The heading counts them.
            RestoreStatus::Restored | RestoreStatus::WouldRestore | RestoreStatus::NotReached => {
                None
            }
        };
        push_once(&mut reasons, line);
    }
    if start_again {
        reasons.push(text(localizer, "wizard-done-restore-start-again"));
    }

    let mut notes: Vec<String> = Vec::new();
    // The first backup in the run that puts REAPER's language back sets
    // it; a later one then finds a language chosen and leaves it, which
    // isn't the user's choice, so it adds nothing.
    let mut language_set = false;
    for (_, item) in &ended {
        if let Some(item) = item {
            for note in restore_notes(localizer, item, &mut language_set) {
                push_once(&mut notes, Some(note));
            }
        }
    }

    let mut blocks = vec![heading.clone()];
    if !reasons.is_empty() {
        blocks.push(reasons.join("\n"));
    }
    if !notes.is_empty() {
        blocks.push(notes.join("\n"));
    }
    WizardBackupRestoreOutcome {
        status,
        status_text: blocks.join("\n\n"),
        heading,
        detail_lines: summarize_backup_restore(localizer, run, report, &ended, local_offset),
        report_path: report
            .saved_report
            .as_ref()
            .map(|saved| saved.text_path.clone()),
    }
}

/// Whether the user can try again after `refusal` only by starting RABBIT
/// again: REAPER or a program was open, or another RABBIT was working on
/// this REAPER. Their sentences, written for Review, say "try again" or
/// "wait", and Done has no way back to Review.
fn tried_again_from_the_start(refusal: &RestoreRefusal) -> bool {
    matches!(
        refusal,
        RestoreRefusal::ReaperRunning { .. }
            | RestoreRefusal::ProgramRunning { .. }
            | RestoreRefusal::LockHeld { .. }
    )
}

/// What came of a backup, from its report; one the report doesn't have
/// wasn't reached.
fn status_of(item: Option<&RestoreReportItem>) -> RestoreStatus {
    item.map_or(RestoreStatus::NotReached, |item| item.status)
}

/// Done's first line when the one ticked backup came back: "RABBIT
/// restored SWS Extension 2.14.0.7.", "ReaPack is back to 1.2.4." or "Your
/// key map from November 1, 2026, 6:40 PM is back."
fn one_restored_heading(
    localizer: &Localizer,
    row: &RestoringBackup,
    item: Option<&RestoreReportItem>,
) -> String {
    let package = row.package.as_deref().unwrap_or(&row.name);
    match &row.kind {
        RestoredBackupKind::Uninstalled => say(
            localizer,
            "wizard-done-restore-status-one",
            &[("backup", (&row.in_sentence).into())],
        ),
        RestoredBackupKind::Earlier { version } => {
            // The version the run went back to, as RABBIT's records now say,
            // with its variant for a package that comes in variants, or the
            // variant alone after going back within one version.
            let version = match item
                .and_then(|item| item.swap.as_ref())
                .filter(|swap| swap.version.is_some())
            {
                Some(swap) => version_beside_text(
                    localizer,
                    &swap.package_id,
                    (swap.version.as_ref(), swap.variant.as_deref()),
                    (
                        swap.replaced_version.as_ref(),
                        swap.replaced_variant.as_deref(),
                    ),
                ),
                None => version.as_ref().map(ToString::to_string),
            };
            match version {
                Some(version) => say(
                    localizer,
                    "wizard-done-restore-status-earlier",
                    &[("package", package.into()), ("version", (&version).into())],
                ),
                None => say(
                    localizer,
                    "wizard-done-restore-status-earlier-copy",
                    &[("package", package.into())],
                ),
            }
        }
        RestoredBackupKind::Keymap { origin } => match &row.date {
            Some(date) => say(
                localizer,
                "wizard-done-restore-status-keymap",
                &[("origin", (*origin).into()), ("date", date.into())],
            ),
            None => say(
                localizer,
                "wizard-done-restore-status-keymap-undated",
                &[("origin", (*origin).into())],
            ),
        },
    }
}

/// What the restore of one backup changed besides its files, as the report
/// says it, for Done: REAPER's language back or not and why, a package
/// RABBIT no longer counts as installed by it after going back to a copy it
/// didn't install, and packages a restore of a whole set left out. Only
/// for what came back: a backup that didn't come back changed none of it.
fn restore_notes(
    localizer: &Localizer,
    item: &RestoreReportItem,
    language_set: &mut bool,
) -> Vec<String> {
    let mut notes = Vec::new();
    if item.status == RestoreStatus::Restored {
        notes.extend(language_lines(localizer, item, language_set));
    }
    // Each package of a whole set of earlier versions that went back.
    for swap in item
        .swaps
        .iter()
        .filter(|swap| swap.status == RestoreStatus::Restored)
    {
        notes.extend(language_lines(localizer, swap, language_set));
    }
    for swap in restored_swaps(item) {
        notes.extend(swap_lines(localizer, swap));
    }
    if let Some(line) = left_out_line(localizer, item) {
        notes.push(line);
    }
    notes
}

/// The swaps of `item` that went through: its own, or each of a whole
/// set's that came back.
fn restored_swaps(item: &RestoreReportItem) -> impl Iterator<Item = &SwappedPackage> {
    let own = (item.status == RestoreStatus::Restored)
        .then_some(item.swap.as_ref())
        .flatten();
    own.into_iter().chain(
        item.swaps
            .iter()
            .filter(|swap| swap.status == RestoreStatus::Restored)
            .filter_map(|swap| swap.swap.as_ref()),
    )
}

/// The lines on REAPER's language for a backup that came back: for a
/// package an uninstall removed, whether its language went back, and for
/// an earlier version of a language pack, whether REAPER's language
/// followed its file.
fn language_lines(
    localizer: &Localizer,
    item: &RestoreReportItem,
    language_set: &mut bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(swap) = &item.swap
        && let Some(file) = &swap.langpack
        && !*language_set
    {
        let file_argument = [("file", file.into())];
        lines.push(if swap.langpack_restored {
            *language_set = true;
            say(
                localizer,
                "wizard-done-restore-language-back",
                &file_argument,
            )
        } else if swap.langpack_error.is_some() {
            say(
                localizer,
                "wizard-done-restore-language-failed",
                &file_argument,
            )
        } else {
            text(localizer, "wizard-done-restore-language-unchanged")
        });
    }
    for package in &item.packages {
        let Some(file) = &package.langpack_before else {
            continue;
        };
        if *language_set {
            continue;
        }
        let file_argument = [("file", file.into())];
        lines.push(if package.langpack_restored {
            *language_set = true;
            say(
                localizer,
                "wizard-done-restore-language-back",
                &file_argument,
            )
        } else if package.langpack_error.is_some() {
            say(
                localizer,
                "wizard-done-restore-language-failed",
                &file_argument,
            )
        } else {
            text(localizer, "wizard-done-restore-language-stays")
        });
    }
    lines
}

/// The lines on a package's records after going back to an earlier
/// version: none when its entry stays RABBIT's, otherwise that RABBIT no
/// longer counts it as installed by it, and, when its uninstall needs that
/// entry, that it must be installed again before RABBIT can uninstall it.
fn swap_lines(localizer: &Localizer, swap: &SwappedPackage) -> Vec<String> {
    if swap.entry != SwappedEntry::Removed {
        return Vec::new();
    }
    let package = localized_package_display_name(localizer, &swap.package_id);
    let argument = [("package", (&package).into())];
    let mut lines = vec![say(localizer, "wizard-done-restore-not-counted", &argument)];
    if swap.uninstall_needs_reinstall {
        lines.push(say(
            localizer,
            "wizard-done-restore-needs-reinstall",
            &argument,
        ));
    }
    lines
}

/// The line on packages a restore of a whole set left out, because none of
/// their files were in it.
fn left_out_line(localizer: &Localizer, item: &RestoreReportItem) -> Option<String> {
    if item.skipped_packages.is_empty() {
        return None;
    }
    let names: Vec<String> = item
        .skipped_packages
        .iter()
        .map(|skipped| localized_package_display_name(localizer, &skipped.package_id))
        .collect();
    Some(say(
        localizer,
        "wizard-done-restore-left-out",
        &[
            ("packages", (&and_list(localizer, &names)).into()),
            ("count", names.len().into()),
        ],
    ))
}

/// The Done page's text for a restore the engine refused or failed as a
/// whole, before anything changed: REAPER or a program runs, another
/// RABBIT holds the lock, RABBIT runs as root, a backup folder is a link or
/// can't be read. The engine saved no report.
fn backup_restore_outcome_from_error(
    localizer: &Localizer,
    run: &BackupRestoreRun,
    error: &RabbitError,
) -> WizardBackupRestoreOutcome {
    let heading = text(localizer, "wizard-done-restore-status-nothing");
    let (status, reason) = match error {
        RabbitError::RestoreRefused(refusal) if tried_again_from_the_start(refusal) => (
            BackupRestoreOutcomeStatus::Refused,
            format!(
                "{} {}",
                restore_refusal_text(localizer, refusal),
                text(localizer, "wizard-done-restore-start-again")
            ),
        ),
        RabbitError::RestoreRefused(refusal) => (
            BackupRestoreOutcomeStatus::Refused,
            restore_refusal_text(localizer, refusal),
        ),
        other => (
            BackupRestoreOutcomeStatus::Failed,
            match unreadable_folder(&run.resource_path, other) {
                Some(folder) => say(
                    localizer,
                    "wizard-done-restore-folder-unreadable",
                    &[("path", (&folder.display().to_string()).into())],
                ),
                None => say(
                    localizer,
                    "wizard-done-restore-error",
                    &[("error", (&other.to_string()).into())],
                ),
            },
        ),
    };
    let not_restored = restore_status_label(localizer, RestoreStatus::NotRestored);
    let mut detail_lines = vec![target_line(localizer, &run.resource_path)];
    detail_lines.extend(run.rows.iter().map(|row| {
        say(
            localizer,
            "wizard-summary-restore-backup",
            &[
                ("backup", (&row.label).into()),
                ("status", (&not_restored).into()),
            ],
        )
    }));
    // The error as it is, in English, for a bug report.
    detail_lines.push(say(
        localizer,
        "wizard-summary-error",
        &[("message", (&error.to_string()).into())],
    ));
    WizardBackupRestoreOutcome {
        status,
        status_text: format!("{heading}\n\n{reason}"),
        heading,
        detail_lines,
        report_path: None,
    }
}

/// "Show details" after a restore: the target, then for each ticked backup
/// what came of it, with the lines that say why and what changed indented
/// below it, then where the report is.
fn summarize_backup_restore(
    localizer: &Localizer,
    run: &BackupRestoreRun,
    report: &RestoreRunReport,
    ended: &[(&RestoringBackup, Option<&RestoreReportItem>)],
    local_offset: &dyn Fn(u64) -> i32,
) -> Vec<String> {
    let mut lines = vec![target_line(localizer, &run.resource_path)];
    let mut language_set = false;
    for (row, item) in ended {
        lines.push(say(
            localizer,
            "wizard-summary-restore-backup",
            &[
                ("backup", (&row.label).into()),
                (
                    "status",
                    (&restore_status_label(localizer, status_of(*item))).into(),
                ),
            ],
        ));
        let below = match item {
            Some(item) => explain(
                localizer,
                &run.resource_path,
                item,
                &mut language_set,
                local_offset,
            ),
            None => vec![text(localizer, "wizard-summary-restore-not-reached")],
        };
        // The explanation sits indented below the backup's line. The indent
        // isn't in the messages: Fluent drops blanks at the start of a value.
        lines.extend(below.into_iter().map(|line| format!("  {line}")));
    }
    if let Some(saved) = &report.saved_report {
        lines.push(say(
            localizer,
            "wizard-summary-restore-report",
            &[("path", (&saved.text_path.display().to_string()).into())],
        ));
    }
    lines
}

/// What came of one backup, one sentence or file a line: why it didn't
/// come back, each file that came back or didn't and where it is, what else
/// the restore changed, and the backup that keeps what it replaced. A whole
/// set of earlier versions has each package's own lines, indented below its
/// name.
fn explain(
    localizer: &Localizer,
    resource_path: &Path,
    item: &RestoreReportItem,
    language_set: &mut bool,
    local_offset: &dyn Fn(u64) -> i32,
) -> Vec<String> {
    let path_line = |key: &str, path: &Path| {
        say(
            localizer,
            key,
            &[("path", (&path.display().to_string()).into())],
        )
    };
    let mut lines: Vec<String> = Vec::new();
    if item.status == RestoreStatus::NotReached {
        lines.push(text(localizer, "wizard-summary-restore-not-reached"));
        return lines;
    }
    if let Some(refusal) = &item.refusal {
        lines.push(restore_refusal_text(localizer, refusal));
    }
    if let Some(error) = &item.error {
        push_once(
            &mut lines,
            Some(say(
                localizer,
                "wizard-summary-error",
                &[("message", error.into())],
            )),
        );
    }
    for action in &item.actions {
        let key = match action.action {
            RestoreBackupActionKind::Restored => "wizard-summary-restore-file-restored",
            RestoreBackupActionKind::PutBack => "wizard-summary-restore-file-put-back",
            RestoreBackupActionKind::Left => "wizard-summary-restore-file-left",
            RestoreBackupActionKind::Missing => "wizard-summary-restore-file-missing",
            // As it was: the restore stopped before it got to it.
            RestoreBackupActionKind::NotRestored => continue,
            // A real run never plans a file; only a dry run does.
            RestoreBackupActionKind::WouldRestore => continue,
        };
        lines.push(path_line(key, &action.target_path));
        // Where the file it replaced is, for one that couldn't go back.
        if matches!(
            action.action,
            RestoreBackupActionKind::Left | RestoreBackupActionKind::Missing
        ) && let Some(copy) = &action.current_backup_path
        {
            lines.push(path_line("wizard-summary-restore-replaced-copy", copy));
        }
    }
    for path in &item.skipped_targets {
        lines.push(path_line("wizard-summary-restore-file-unfinished", path));
    }
    if let Some(swap) = &item.swap {
        for moved in &swap.moved_out {
            let key = match moved.action {
                MoveOutAction::MovedOut => "wizard-summary-restore-moved-out",
                MoveOutAction::StillInBackup => "wizard-summary-restore-still-in-backup",
                // Where it was before the restore.
                MoveOutAction::MovedBack | MoveOutAction::NotMoved => continue,
                // A real run never plans a move; only a dry run does.
                MoveOutAction::WouldMoveOut => continue,
            };
            lines.push(path_line(key, &moved.path));
        }
        for path in &swap.changed_left {
            lines.push(path_line("wizard-summary-restore-changed-left", path));
        }
        for path in &swap.left_in_place {
            lines.push(path_line("wizard-summary-restore-left-in-place", path));
        }
        if item.status == RestoreStatus::Restored {
            lines.extend(swap_lines(localizer, swap));
        }
    }
    if item.status == RestoreStatus::Restored
        && let Some(added) = item.keymap.as_ref().and_then(|keymap| keymap.scripts_added)
        && added > 0
    {
        lines.push(say(
            localizer,
            "wizard-summary-restore-scripts-kept",
            &[("count", added.into())],
        ));
    }
    if item.status == RestoreStatus::Restored {
        lines.extend(language_lines(localizer, item, language_set));
    }
    let language_errors = item
        .packages
        .iter()
        .filter_map(|package| package.langpack_error.as_ref())
        .chain(
            item.swap
                .iter()
                .filter(|_| item.status == RestoreStatus::Restored)
                .filter_map(|swap| swap.langpack_error.as_ref()),
        );
    for error in language_errors {
        push_once(
            &mut lines,
            Some(say(
                localizer,
                "wizard-summary-error",
                &[("message", error.into())],
            )),
        );
    }
    if let Some(line) = left_out_line(localizer, item) {
        lines.push(line);
    }
    if let Some(id) = &item.before_restore_set_id {
        let backup = kept_backup_name(localizer, resource_path, item);
        lines.push(
            match seconds_in_set_id(id)
                .and_then(|seconds| backup_date_text(localizer, seconds, local_offset(seconds)))
            {
                Some(date) => say(
                    localizer,
                    "wizard-summary-restore-kept",
                    &[("backup", (&backup).into()), ("date", (&date).into())],
                ),
                None => say(
                    localizer,
                    "wizard-summary-restore-kept-undated",
                    &[("backup", (&backup).into())],
                ),
            },
        );
    }
    for swap in &item.swaps {
        let package = swap
            .package_id
            .as_deref()
            .map(|package_id| localized_package_display_name(localizer, package_id))
            .unwrap_or_else(|| swap.backup_id.clone());
        lines.push(say(
            localizer,
            "wizard-summary-restore-backup",
            &[
                ("backup", (&package).into()),
                (
                    "status",
                    (&restore_status_label(localizer, swap.status)).into(),
                ),
            ],
        ));
        let below = explain(localizer, resource_path, swap, language_set, local_offset);
        lines.extend(below.into_iter().map(|line| format!("  {line}")));
    }
    lines
}

/// What the Backups tab calls the backup a restore of `item` made of what
/// it replaced: the key map it replaced, the version a package had before
/// it went back to an earlier one, or the files it replaced.
fn kept_backup_name(
    localizer: &Localizer,
    resource_path: &Path,
    item: &RestoreReportItem,
) -> String {
    let replaced_keymap = item.keymap.as_ref().is_some_and(|keymap| keymap.replaced)
        || item.actions.iter().any(|action| {
            action.current_backup_path.is_some()
                && same_place(&action.target_path)
                    == same_place(&resource_path.join("reaper-kb.ini"))
        });
    if replaced_keymap {
        return text(localizer, "wizard-backups-name-keymap");
    }
    if let Some(swap) = &item.swap {
        let replaced = (
            swap.replaced_version.as_ref(),
            swap.replaced_variant.as_deref(),
        );
        // After going back within one version, the variant alone tells the
        // backup apart.
        let version = if same_version_other_variant(
            replaced,
            (swap.version.as_ref(), swap.variant.as_deref()),
        ) {
            None
        } else {
            replaced.0
        };
        return package_backup_name(localizer, &swap.package_id, version, replaced.1);
    }
    text(localizer, "wizard-backups-name-before-restore")
}

/// What came of a backup in a restore, in a few words, after its name in
/// "Show details".
pub fn restore_status_label(localizer: &Localizer, status: RestoreStatus) -> String {
    let key = match status {
        RestoreStatus::Restored => "wizard-restore-status-restored",
        RestoreStatus::WouldRestore => "wizard-restore-status-would-restore",
        RestoreStatus::NotRestored => "wizard-restore-status-not-restored",
        RestoreStatus::FilesLeft => "wizard-restore-status-files-left",
        RestoreStatus::NotReached => "wizard-restore-status-not-reached",
        RestoreStatus::PartlyRestored => "wizard-restore-status-partly-restored",
    };
    text(localizer, key)
}
