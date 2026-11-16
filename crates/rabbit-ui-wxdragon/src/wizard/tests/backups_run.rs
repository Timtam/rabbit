//! Deleting backups from the Backups tab: what the run is given, what the
//! progress page says, what Done and Show details say for every way a
//! delete can end, in every language, and a real delete of sets in a
//! temporary folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rabbit_core::RabbitError;
use rabbit_core::backup_delete::{DeleteReport, DeleteReportItem, DeleteStatus, NotDeletedReason};
use rabbit_core::backup_overview::{BackupEntry, BackupItem};
use rabbit_core::cancel::CancelToken;
use rabbit_core::localization::{Localizer, embedded_locales};
use rabbit_core::progress::{ProgressEvent, ProgressReporter};
use rabbit_core::receipt::PackageReceipt;
use rabbit_core::report::SavedReportPaths;
use rabbit_core::restore_refusal::{RestoreRefusal, sample_refusals};
use rabbit_core::rollback::{BACKUP_MANIFEST_FILE, BackupPackageRecord, BackupSetWriter};
use rabbit_core::uninstall::execute::UNINSTALL_BACKUP_REASON;
use tempfile::tempdir;

use super::backups::{
    KB, MB, OCT_2_1100, backup_list, en, ffmpeg, index_of, keymap, list_in, package, reapack,
    records, sws, tick, version, write_uninstall_set,
};
use super::support::{assert_reads, langpack_model};
use super::uninstall::target;
use crate::wizard::*;

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

const RESOURCE: &str = "C:/REAPER";

/// FFmpeg 7.1 from an uninstall of its own on October 2, so it goes alone.
fn ffmpeg_alone() -> BackupEntry {
    BackupEntry {
        item: package("uninstall-unix-1790938800", "ffmpeg"),
        created_unix: Some(OCT_2_1100),
        ..ffmpeg()
    }
}

/// Another key map of yours, which OSARA's replaced on October 2: a set of
/// its own, and a row with the same name as [`keymap`]'s.
fn older_keymap() -> BackupEntry {
    BackupEntry {
        item: BackupItem::Keymap {
            set_id: "unattended-unix-1790938800-000000001".to_string(),
        },
        created_unix: Some(OCT_2_1100),
        ..keymap()
    }
}

/// The set id the row named `name` takes.
fn set_of(list: &WizardBackupList, name: &str) -> String {
    list.rows[index_of(list, name)].set_ids()[0].clone()
}

/// What a delete of what is ticked to delete in `list` takes, in the
/// REAPER at `C:/REAPER`.
fn run_of(localizer: &Localizer, list: &WizardBackupList) -> BackupDeleteRun {
    backup_delete_run(localizer, Path::new(RESOURCE), list)
}

/// A report item of the set `set_id` that came out as `status`, freeing
/// `bytes_freed`.
fn item(set_id: &str, status: DeleteStatus, bytes_freed: u64) -> DeleteReportItem {
    DeleteReportItem {
        set_id: set_id.to_string(),
        status,
        set_path: None,
        unfinished_deletion: None,
        size: bytes_freed,
        only_copy: Vec::new(),
        bytes_freed,
        not_deleted: None,
        left_in: None,
        finished_earlier_deletion: false,
        refusal: None,
        error: None,
    }
}

/// A set that wasn't deleted, for `reason`.
fn not_deleted(set_id: &str, reason: NotDeletedReason) -> DeleteReportItem {
    let mut item = item(set_id, DeleteStatus::NotDeleted, 0);
    item.not_deleted = Some(reason);
    item.error = Some(format!(
        "C:/REAPER/RABBIT/backups/{set_id}: Access is denied. (os error 5)"
    ));
    if reason == NotDeletedReason::EarlierDeletionLeft {
        item.left_in = Some(PathBuf::from(format!(
            "C:/REAPER/RABBIT/backups-deleting/{set_id}"
        )));
    }
    item
}

/// A set the walk couldn't remove all of, freeing `bytes_freed`.
fn partly_deleted(set_id: &str, bytes_freed: u64) -> DeleteReportItem {
    let mut item = item(set_id, DeleteStatus::PartlyDeleted, bytes_freed);
    item.left_in = Some(PathBuf::from(format!(
        "C:/REAPER/RABBIT/backups-deleting/{set_id}"
    )));
    item.error = Some("reaper_sws-x64.dll: Access is denied. (os error 5)".to_string());
    item
}

/// A set a check refused.
fn refused(set_id: &str, refusal: RestoreRefusal) -> DeleteReportItem {
    let mut item = item(set_id, DeleteStatus::Refused, 0);
    item.refusal = Some(refusal);
    item
}

/// A real run's report of `sets`, saved where a real run saves it.
fn report(sets: Vec<DeleteReportItem>) -> DeleteReport {
    DeleteReport {
        resource_path: PathBuf::from(RESOURCE),
        dry_run: false,
        bytes_freed: sets.iter().map(|set| set.bytes_freed).sum(),
        sets,
        refused: false,
        stopped: false,
        saved_report: Some(SavedReportPaths {
            json_path: PathBuf::from("C:/REAPER/RABBIT/logs/delete-backup-unix-1.json"),
            text_path: PathBuf::from("C:/REAPER/RABBIT/logs/delete-backup-unix-1.txt"),
        }),
    }
}

/// A run whose checks refused it before anything changed: no report saved.
fn refused_report(sets: Vec<DeleteReportItem>) -> DeleteReport {
    DeleteReport {
        refused: true,
        saved_report: None,
        ..report(sets)
    }
}

/// A run the user stopped.
fn stopped_report(sets: Vec<DeleteReportItem>) -> DeleteReport {
    DeleteReport {
        stopped: true,
        ..report(sets)
    }
}

/// A list of `entries` in `localizer`'s language, with Delete chosen and
/// every backup ticked to delete.
fn all_ticked(localizer: &Localizer, entries: Vec<BackupEntry>) -> WizardBackupList {
    let mut list = list_in(localizer, entries);
    set_backup_choice(&mut list, BackupChoice::Delete);
    let rows: Vec<usize> = (0..list.rows.len())
        .filter(|&index| !list.rows[index].is_heading())
        .collect();
    for index in rows {
        apply_backup_checkbox(&mut list.rows, index, BackupChoice::Delete, true);
    }
    list
}

/// FFmpeg alone, ReaPack and your key map, each a set of its own, all
/// ticked to delete, in `localizer`'s language.
fn three_alone(localizer: &Localizer) -> WizardBackupList {
    all_ticked(localizer, vec![ffmpeg_alone(), reapack(), keymap()])
}

const REAPACK: &str = "ReaPack 1.2.4";
const KEYMAP: &str = "Your key map";
const RECORDS: &str = "Old copies of RABBIT's records";
const KEYMAP_NOV_1: &str = "Your key map (Replaced by OSARA's on November 1, 2026, 6:40 PM)";
const KEYMAP_OCT_2: &str = "Your key map (Replaced by OSARA's on October 2, 2026, 11:00 AM)";

/// The Done page for `result` of the run of `list`, in English.
fn done(
    list: &WizardBackupList,
    result: rabbit_core::Result<DeleteReport>,
) -> WizardBackupDeleteOutcome {
    wizard_backup_delete_outcome(&en(), &run_of(&en(), list), &result)
}

/// The records row's 23 sets, of which those at `ended` ended as given,
/// freeing a KB each when gone; the others as `rest`.
fn records_sets(
    run: &BackupDeleteRun,
    group: usize,
    ended: &dyn Fn(usize, &str) -> Option<DeleteReportItem>,
    rest: DeleteStatus,
) -> Vec<DeleteReportItem> {
    run.groups[group]
        .set_ids
        .iter()
        .enumerate()
        .map(|(index, id)| ended(index, id).unwrap_or_else(|| item(id, rest, 0)))
        .collect()
}

// ---------------------------------------------------------------------------
// What the run is given.
// ---------------------------------------------------------------------------

#[test]
fn the_run_is_copied_out_of_the_list_by_what_goes_together() {
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack(), keymap(), records()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    tick(&mut list, RECORDS, BackupChoice::Delete);
    // A tick for Restore is no part of a delete.
    tick(&mut list, KEYMAP, BackupChoice::Restore);

    let run = run_of(&en(), &list);
    assert_eq!(run.resource_path, PathBuf::from(RESOURCE));
    assert_eq!(run.items, backup_delete_items(&list));
    // Counted as Review and the question count: one per ticked row.
    assert_eq!(run.backups(), 4);
    assert_eq!(run.groups.len(), 3);
    assert_eq!(
        run.groups[0].names,
        [
            "SWS Extension 2.14.0.7",
            "FFmpeg for Improved Video Support 7.1"
        ]
    );
    assert_eq!(run.groups[0].set_ids, ["uninstall-unix-1794233100"]);
    assert_eq!(
        run.groups[0].references[0],
        "SWS Extension 2.14.0.7 (Uninstalled November 9, 2026, 2:05 PM)"
    );
    // No other ticked row has their names, so they are said without dates.
    assert_eq!(run.groups[0].labels, run.groups[0].names);
    assert_eq!(run.groups[1].names, [REAPACK]);
    assert_eq!(run.groups[2].names, [RECORDS]);
    assert_eq!(run.groups[2].set_ids.len(), 23);
    // Every fingerprint the list read goes with it.
    assert!(run.items.iter().all(|item| item.fingerprint.is_some()));
}

#[test]
fn review_the_question_and_done_count_the_same_backups() {
    // A pair that shares a set, a row alone, the records row and a tick
    // for Restore only.
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack(), keymap(), records()]);
    tick(
        &mut list,
        "FFmpeg for Improved Video Support 7.1",
        BackupChoice::Delete,
    );
    tick(&mut list, REAPACK, BackupChoice::Delete);
    tick(&mut list, RECORDS, BackupChoice::Delete);
    tick(&mut list, KEYMAP, BackupChoice::Restore);
    let run = run_of(&en(), &list);
    assert_eq!(
        run.backups(),
        list.ticked_rows(BackupChoice::Delete).count()
    );
    assert_eq!(run.backups(), 4);

    let (_, question) = delete_backups_question(&en(), &list);
    assert!(
        question.starts_with("RABBIT deletes 4 backups and frees "),
        "{question}"
    );
    let all_deleted: Vec<DeleteReportItem> = run
        .items
        .iter()
        .map(|delete| item(&delete.set_id, DeleteStatus::Deleted, KB))
        .collect();
    let outcome = wizard_backup_delete_outcome(&en(), &run, &Ok(report(all_deleted)));
    assert!(
        outcome
            .heading
            .starts_with("RABBIT deleted 4 backups and freed "),
        "{}",
        outcome.heading
    );
}

#[test]
fn nothing_ticked_to_delete_gives_a_run_of_nothing() {
    let mut list = backup_list(vec![sws(), reapack()]);
    tick(&mut list, REAPACK, BackupChoice::Restore);
    let run = run_of(&en(), &list);
    assert!(run.items.is_empty());
    assert!(run.groups.is_empty());
    assert_eq!(run.backups(), 0);
}

#[test]
fn the_progress_log_starts_with_the_target_and_what_goes() {
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    assert_eq!(
        backup_delete_progress_details_for_start(&en(), Path::new(RESOURCE), &list),
        "Starting to delete backups.\n\
         Target: C:/REAPER\n\
         SWS Extension 2.14.0.7: Will delete the only copy\n\
         FFmpeg for Improved Video Support 7.1: Will delete the only copy\n\
         ReaPack 1.2.4: Will delete"
    );
}

// ---------------------------------------------------------------------------
// The progress page.
// ---------------------------------------------------------------------------

fn started(set_id: &str) -> ProgressEvent {
    ProgressEvent::DeleteStarted {
        set_id: set_id.to_string(),
    }
}

fn completed(set_id: &str, status: DeleteStatus, bytes_freed: u64) -> ProgressEvent {
    ProgressEvent::DeleteCompleted {
        set_id: set_id.to_string(),
        status,
        bytes_freed,
    }
}

/// The progress page's state at the start of the delete `run`, in
/// `localizer`'s language.
fn progress_of(localizer: &Localizer, run: &BackupDeleteRun) -> ProgressUiState {
    ProgressUiState::new(WizardMode::DeleteBackups, run.items.len(), 0)
        .with_backup_delete(backup_delete_progress(localizer, run))
}

/// Count `event` in, as the progress page does, and give its lines.
fn hear(state: &mut ProgressUiState, event: &ProgressEvent) -> (Option<String>, Option<String>) {
    state.record(event);
    backup_delete_progress_lines(&en(), state, event)
}

#[test]
fn a_delete_says_each_backup_it_starts_and_logs_what_came_of_it() {
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let run = run_of(&en(), &list);
    assert_eq!(run.items.len(), 2);
    let mut state = progress_of(&en(), &run);

    let both = "uninstall-unix-1794233100";
    let event = started(both);
    assert!(is_spoken(&event));
    let (status, log) = hear(&mut state, &event);
    let line = "Deleting the backups: SWS Extension 2.14.0.7 and FFmpeg for Improved Video Support \
                7.1…";
    assert_eq!(status.as_deref(), Some(line));
    assert_eq!(log.as_deref(), Some(line));
    assert_eq!(state.percentage(), 0);
    assert!(!state.moves_focus_to_log(&event));

    let event = completed(both, DeleteStatus::Deleted, 254 * MB);
    assert!(!is_spoken(&event));
    assert_eq!(
        hear(&mut state, &event),
        (
            None,
            Some(
                "SWS Extension 2.14.0.7 and FFmpeg for Improved Video Support 7.1: deleted, 254 MB \
                 freed."
                    .to_string()
            )
        )
    );
    assert_eq!(state.percentage(), 50);

    let reapack_set = set_of(&list, REAPACK);
    assert_eq!(
        hear(&mut state, &started(&reapack_set)).0,
        Some("Deleting the backup: ReaPack 1.2.4…".to_string())
    );
    let event = completed(&reapack_set, DeleteStatus::NotDeleted, 0);
    assert_eq!(
        hear(&mut state, &event).1,
        Some("ReaPack 1.2.4: not deleted.".to_string())
    );
    assert_eq!(state.percentage(), 100);
    // A focus move is only for an uninstaller that keeps RABBIT waiting.
    assert!(!state.moves_focus_to_log(&event));
}

#[test]
fn every_end_of_a_set_has_its_log_line() {
    let list = three_alone(&en());
    let run = run_of(&en(), &list);
    let set = set_of(&list, REAPACK);
    let cases = [
        (
            DeleteStatus::Deleted,
            Some("ReaPack 1.2.4: deleted, 2 MB freed."),
        ),
        (
            DeleteStatus::FinishedEarlierDeletion,
            Some("ReaPack 1.2.4: finished a deletion that started earlier, 2 MB freed."),
        ),
        (
            DeleteStatus::AlreadyGone,
            Some("ReaPack 1.2.4: already gone."),
        ),
        (
            DeleteStatus::PartlyDeleted,
            Some("ReaPack 1.2.4: only partly deleted, 2 MB freed."),
        ),
        (
            DeleteStatus::NotDeleted,
            Some("ReaPack 1.2.4: not deleted."),
        ),
        (DeleteStatus::Refused, Some("ReaPack 1.2.4: not deleted.")),
        // A real run sends no end for these.
        (DeleteStatus::NotReached, None),
        (DeleteStatus::WouldDelete, None),
        (DeleteStatus::WouldFinishEarlierDeletion, None),
    ];
    for (status, line) in cases {
        let mut state = progress_of(&en(), &run);
        let (_, log) = hear(&mut state, &completed(&set, status, 2 * MB));
        assert_eq!(log.as_deref(), line, "{status:?}");
    }
}

#[test]
fn the_records_row_is_said_once_at_its_start_and_once_at_its_end() {
    let mut list = backup_list(vec![records(), reapack()]);
    tick(&mut list, RECORDS, BackupChoice::Delete);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let run = run_of(&en(), &list);
    let record_sets: Vec<String> = run.groups[1].set_ids.clone();
    assert_eq!(record_sets.len(), 23);

    // Every set of the row goes, but the last, which is in use.
    let mut state = progress_of(&en(), &run);
    let mut said = Vec::new();
    let mut logged = Vec::new();
    for (index, set) in record_sets.iter().enumerate() {
        let (status, log) = hear(&mut state, &started(set));
        said.extend(status);
        logged.extend(log);
        let event = if index == 22 {
            completed(set, DeleteStatus::NotDeleted, 0)
        } else {
            completed(set, DeleteStatus::Deleted, KB)
        };
        let (status, log) = hear(&mut state, &event);
        assert!(status.is_none());
        logged.extend(log);
    }
    assert_eq!(
        said,
        ["Deleting the backup: Old copies of RABBIT's records…"]
    );
    assert_eq!(
        logged,
        [
            "Deleting the backup: Old copies of RABBIT's records…",
            "Old copies of RABBIT's records: only partly deleted, 22 KB freed.",
        ]
    );
    // The next row is said again.
    assert!(
        hear(&mut state, &started(&set_of(&list, REAPACK)))
            .0
            .is_some()
    );

    // All of them gone: one end line with all they freed.
    let mut state = progress_of(&en(), &run);
    let mut logged = Vec::new();
    for set in &record_sets {
        hear(&mut state, &started(set));
        logged.extend(hear(&mut state, &completed(set, DeleteStatus::Deleted, KB)).1);
    }
    assert_eq!(
        logged,
        ["Old copies of RABBIT's records: deleted, 23 KB freed."]
    );
}

#[test]
fn two_backups_of_one_name_are_each_said_with_their_dates() {
    let mut list = backup_list(vec![keymap(), older_keymap()]);
    let first = index_of(&list, KEYMAP);
    apply_backup_checkbox(&mut list.rows, first, BackupChoice::Delete, true);
    apply_backup_checkbox(&mut list.rows, first + 1, BackupChoice::Delete, true);
    assert_eq!(list.rows[first + 1].name, KEYMAP);
    let run = run_of(&en(), &list);
    assert_eq!(run.groups.len(), 2);
    assert_eq!(run.groups[0].labels, [KEYMAP_NOV_1]);
    assert_eq!(run.groups[1].labels, [KEYMAP_OCT_2]);
    let newer = run.groups[0].set_ids[0].clone();
    let older = run.groups[1].set_ids[0].clone();
    assert_eq!(
        backup_delete_progress_details_for_start(&en(), Path::new(RESOURCE), &list),
        format!(
            "Starting to delete backups.\nTarget: C:/REAPER\n{KEYMAP_NOV_1}: Will delete the only \
             copy\n{KEYMAP_OCT_2}: Will delete the only copy"
        )
    );

    let mut state = progress_of(&en(), &run);
    let mut said = Vec::new();
    let mut logged = Vec::new();
    for event in [
        started(&newer),
        completed(&newer, DeleteStatus::Deleted, 38 * KB),
        started(&older),
        completed(&older, DeleteStatus::NotDeleted, 0),
    ] {
        let (status, log) = hear(&mut state, &event);
        said.extend(status);
        logged.extend(log);
    }
    let start_newer = format!("Deleting the backup: {KEYMAP_NOV_1}…");
    let start_older = format!("Deleting the backup: {KEYMAP_OCT_2}…");
    assert_eq!(said, [start_newer.clone(), start_older.clone()]);
    assert_eq!(
        logged,
        [
            start_newer,
            format!("{KEYMAP_NOV_1}: deleted, 38 KB freed."),
            start_older,
            format!("{KEYMAP_OCT_2}: not deleted."),
        ]
    );

    // Show details tells them apart too.
    let outcome = wizard_backup_delete_outcome(
        &en(),
        &run,
        &Ok(report(vec![
            item(&newer, DeleteStatus::Deleted, 38 * KB),
            not_deleted(&older, NotDeletedReason::InUse),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT deleted 1 of 2 backups.\n\nOne couldn't be deleted, because a file in it is open \
         in another program. Close that program, then run RABBIT again to delete it."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER".to_string(),
            format!("{KEYMAP_NOV_1}: Deleted"),
            "  Freed: 38 KB".to_string(),
            format!("{KEYMAP_OCT_2}: Not deleted"),
            "  Nothing of it changed.".to_string(),
            "  A file in it is open in another program, so it couldn't be moved.".to_string(),
            format!("  Error: C:/REAPER/RABBIT/backups/{older}: Access is denied. (os error 5)"),
            "Freed in all: 38 KB".to_string(),
            "Report: C:/REAPER/RABBIT/logs/delete-backup-unix-1.txt".to_string(),
        ]
    );
}

#[test]
fn a_delete_counts_one_phase_per_set() {
    assert_eq!(phases_per_package(WizardMode::DeleteBackups), 1);
    let mut state = ProgressUiState::new(WizardMode::DeleteBackups, 4, 0);
    for set in ["a", "b", "c"] {
        state.record(&started(set));
        assert_eq!(state.current_package.as_deref(), Some(set));
        state.record(&completed(set, DeleteStatus::Deleted, 0));
        assert!(state.current_package.is_none());
    }
    assert_eq!(state.percentage(), 75);
}

#[test]
fn only_a_deletes_events_have_delete_lines() {
    let mut state = progress_of(&en(), &run_of(&en(), &three_alone(&en())));
    for event in [
        ProgressEvent::InstallStarted {
            package_id: "reapack".to_string(),
        },
        ProgressEvent::RestoreStarted {
            set_id: "uninstall-unix-1".to_string(),
            package_id: Some("sws".to_string()),
        },
    ] {
        assert_eq!(
            backup_delete_progress_lines(&en(), &mut state, &event),
            (None, None),
            "{event:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Done and Show details, in English.
// ---------------------------------------------------------------------------

#[test]
fn a_full_delete_says_how_many_backups_went_and_what_it_freed() {
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack(), keymap()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let outcome = done(
        &list,
        Ok(report(vec![
            item("uninstall-unix-1794233100", DeleteStatus::Deleted, 400 * MB),
            item("unix-1793697120", DeleteStatus::Deleted, 120 * MB),
        ])),
    );
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Deleted);
    // Three backups, as Review and the question counted them, in two sets.
    assert_eq!(
        outcome.heading,
        "RABBIT deleted 3 backups and freed 520 MB."
    );
    assert_eq!(outcome.status_text, outcome.heading);
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER",
            "SWS Extension 2.14.0.7 and FFmpeg for Improved Video Support 7.1: Deleted",
            "  Freed: 400 MB",
            "ReaPack 1.2.4: Deleted",
            "  Freed: 120 MB",
            "Freed in all: 520 MB",
            "Report: C:/REAPER/RABBIT/logs/delete-backup-unix-1.txt",
        ]
    );
    assert_eq!(
        outcome.report_path,
        Some(PathBuf::from(
            "C:/REAPER/RABBIT/logs/delete-backup-unix-1.txt"
        ))
    );
}

#[test]
fn one_backup_deleted_is_said_as_one() {
    let mut list = backup_list(vec![reapack()]);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let outcome = done(
        &list,
        Ok(report(vec![item(
            "unix-1793697120",
            DeleteStatus::Deleted,
            2 * MB,
        )])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT deleted one backup and freed 2 MB."
    );
}

/// Done and Show details for three backups of their own, of which your key
/// map wasn't deleted for `reason`, as Done says it and as Show details
/// says it below the key map's line.
fn partial(reason: NotDeletedReason) -> WizardBackupDeleteOutcome {
    let list = three_alone(&en());
    done(
        &list,
        Ok(report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::Deleted, 250 * MB),
            item("unix-1793697120", DeleteStatus::Deleted, 2 * MB),
            not_deleted(&set_of(&list, KEYMAP), reason),
        ])),
    )
}

/// The lines Show details has below the line of the backup `name`.
fn below<'a>(outcome: &'a WizardBackupDeleteOutcome, name: &str) -> Vec<&'a str> {
    let start = outcome
        .detail_lines
        .iter()
        .position(|line| line.starts_with(&format!("{name}: ")))
        .unwrap_or_else(|| panic!("{:#?}", outcome.detail_lines));
    outcome.detail_lines[start + 1..]
        .iter()
        .take_while(|line| line.starts_with("  "))
        .map(|line| line.trim_start())
        .collect()
}

/// What Show details says came of the backup `name`.
fn label_of<'a>(outcome: &'a WizardBackupDeleteOutcome, name: &str) -> &'a str {
    let prefix = format!("{name}: ");
    outcome
        .detail_lines
        .iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("{:#?}", outcome.detail_lines))
}

#[test]
fn a_partial_delete_says_how_many_went_and_why_one_didnt() {
    let error = "Error: C:/REAPER/RABBIT/backups/unattended-unix-1793558400-000000001: Access is \
                 denied. (os error 5)";
    let outcome = partial(NotDeletedReason::InUse);
    assert_eq!(
        outcome.status,
        BackupDeleteOutcomeStatus::CompletedWithProblems
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT deleted 2 of 3 backups.\n\nOne couldn't be deleted, because a file in it is open \
         in another program. Close that program, then run RABBIT again to delete it."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER",
            "FFmpeg for Improved Video Support 7.1: Deleted",
            "  Freed: 250 MB",
            "ReaPack 1.2.4: Deleted",
            "  Freed: 2 MB",
            "Your key map: Not deleted",
            "  Nothing of it changed.",
            "  A file in it is open in another program, so it couldn't be moved.",
            &format!("  {error}"),
            "Freed in all: 252 MB",
            "Report: C:/REAPER/RABBIT/logs/delete-backup-unix-1.txt",
        ]
    );

    let outcome = partial(NotDeletedReason::Permission);
    assert!(
        outcome.status_text.ends_with(
            "\n\nOne couldn't be deleted, because RABBIT isn't allowed to move it. It stays as \
             it was."
        ),
        "{}",
        outcome.status_text
    );
    assert_eq!(
        below(&outcome, KEYMAP),
        [
            "Nothing of it changed.",
            "RABBIT isn't allowed to move it.",
            error,
        ]
    );

    let outcome = partial(NotDeletedReason::EarlierDeletionLeft);
    assert!(
        outcome.status_text.ends_with(
            "\n\nOne couldn't be deleted, because an earlier deletion of it didn't finish and \
             RABBIT still couldn't finish it. It stays as it was. Run RABBIT again to try once \
             more."
        ),
        "{}",
        outcome.status_text
    );
    assert_eq!(
        below(&outcome, KEYMAP)[..2],
        [
            "Nothing of it changed.",
            "An earlier deletion of it didn't finish, and RABBIT still couldn't finish it. What is \
             left of that deletion is in \
             C:/REAPER/RABBIT/backups-deleting/unattended-unix-1793558400-000000001."
        ]
    );

    let outcome = partial(NotDeletedReason::Other);
    assert!(
        outcome
            .status_text
            .ends_with("\n\nOne couldn't be deleted, and stays as it was. Show details says why."),
        "{}",
        outcome.status_text
    );
    assert_eq!(below(&outcome, KEYMAP), ["Nothing of it changed.", error]);
}

#[test]
fn a_partly_deleted_backup_says_what_is_left_and_where() {
    let list = three_alone(&en());
    let outcome = done(
        &list,
        Ok(report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::Deleted, 250 * MB),
            item("unix-1793697120", DeleteStatus::Deleted, 2 * MB),
            partly_deleted(&set_of(&list, KEYMAP), 20 * KB),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT deleted 2 of 3 backups.\n\nOne was only partly deleted. What is left of it is an \
         unfinished deletion, which no restore uses any more. To finish it, run RABBIT again \
         and delete it on the Backups tab."
    );
    assert_eq!(label_of(&outcome, KEYMAP), "Partly deleted");
    assert_eq!(
        below(&outcome, KEYMAP),
        [
            "Freed: 20 KB",
            "What RABBIT couldn't remove stays in \
             C:/REAPER/RABBIT/backups-deleting/unattended-unix-1793558400-000000001. The Backups \
             tab lists it as an unfinished deletion, which no restore uses any more; deleting it \
             there finishes it.",
            "Error: reaper_sws-x64.dll: Access is denied. (os error 5)",
        ]
    );
}

#[test]
fn reasons_are_counted_and_a_single_backup_is_called_it() {
    // Two of three in use: one sentence, counted.
    let list = three_alone(&en());
    let outcome = done(
        &list,
        Ok(report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::Deleted, 250 * MB),
            not_deleted("unix-1793697120", NotDeletedReason::InUse),
            not_deleted(&set_of(&list, KEYMAP), NotDeletedReason::InUse),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT deleted 1 of 3 backups.\n\n2 couldn't be deleted, because a file in them is open \
         in another program. Close that program, then run RABBIT again to delete them."
    );

    // None of three.
    let outcome = done(
        &list,
        Ok(report(vec![
            not_deleted("uninstall-unix-1790938800", NotDeletedReason::Permission),
            not_deleted("unix-1793697120", NotDeletedReason::InUse),
            partly_deleted(&set_of(&list, KEYMAP), KB),
        ])),
    );
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::NothingDeleted);
    assert!(
        outcome.status_text.starts_with(
            "RABBIT couldn't delete the backups.\n\nOne couldn't be deleted, because a file"
        ),
        "{}",
        outcome.status_text
    );
    assert_eq!(
        outcome.status_text.lines().count(),
        5,
        "{}",
        outcome.status_text
    );

    // A single backup: the first line calls it "the backup", and the
    // reason calls it "it".
    let mut one = backup_list(vec![keymap()]);
    tick(&mut one, KEYMAP, BackupChoice::Delete);
    let set = set_of(&one, KEYMAP);
    let outcome = done(
        &one,
        Ok(report(vec![not_deleted(&set, NotDeletedReason::InUse)])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't delete the backup.\n\nA file in it is open in another program. Close \
         that program, then run RABBIT again to delete it."
    );
    let outcome = done(&one, Ok(report(vec![partly_deleted(&set, KB)])));
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't delete the backup.\n\nRABBIT deleted only part of it. What is left is \
         an unfinished deletion, which no restore uses any more. To finish it, run RABBIT again \
         and delete it on the Backups tab."
    );
}

#[test]
fn a_stopped_delete_says_how_far_it_got_and_that_the_rest_is_kept() {
    let list = three_alone(&en());
    let outcome = done(
        &list,
        Ok(stopped_report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::Deleted, 250 * MB),
            item("unix-1793697120", DeleteStatus::NotReached, 0),
            item(&set_of(&list, KEYMAP), DeleteStatus::NotReached, 0),
        ])),
    );
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Stopped);
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped after deleting 1 of 3 backups. Backups it hadn't reached are kept."
    );
    assert_eq!(
        below(&outcome, REAPACK),
        ["RABBIT stopped before it, so it is kept."]
    );
    assert_eq!(
        outcome.detail_lines.last().map(String::as_str),
        Some("Report: C:/REAPER/RABBIT/logs/delete-backup-unix-1.txt")
    );

    let outcome = done(
        &list,
        Ok(stopped_report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::NotReached, 0),
            item("unix-1793697120", DeleteStatus::NotReached, 0),
            item(&set_of(&list, KEYMAP), DeleteStatus::NotReached, 0),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped before it deleted a backup. Backups it hadn't reached are kept."
    );

    // A backup gone before the run is no backup "kept".
    let outcome = done(
        &list,
        Ok(stopped_report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::AlreadyGone, 0),
            item("unix-1793697120", DeleteStatus::Deleted, 2 * MB),
            item(&set_of(&list, KEYMAP), DeleteStatus::NotReached, 0),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped after deleting 1 of 3 backups. Backups it hadn't reached are kept.\n\n\
         One was already gone."
    );
}

#[test]
fn the_records_row_stopped_partway_says_part_of_it_went() {
    let mut list = backup_list(vec![records()]);
    tick(&mut list, RECORDS, BackupChoice::Delete);
    let run = run_of(&en(), &list);
    let deleted_five =
        |index: usize, id: &str| (index < 5).then(|| item(id, DeleteStatus::Deleted, KB));
    let sets = records_sets(&run, 0, &deleted_five, DeleteStatus::NotReached);
    let outcome = wizard_backup_delete_outcome(&en(), &run, &Ok(stopped_report(sets)));
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Stopped);
    // Never "before it deleted a backup" once part of one is gone.
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped before it had deleted all of the backup.\n\nRABBIT deleted part of it \
         before it stopped. The rest is kept."
    );
    assert_eq!(label_of(&outcome, RECORDS), "Partly deleted");
    assert_eq!(
        below(&outcome, RECORDS),
        [
            "Freed: 5 KB",
            "RABBIT deleted part of it, then stopped. The rest is kept.",
        ]
    );

    // After a whole backup went.
    let mut list = backup_list(vec![reapack(), records()]);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    tick(&mut list, RECORDS, BackupChoice::Delete);
    let run = run_of(&en(), &list);
    let mut sets = vec![item("unix-1793697120", DeleteStatus::Deleted, 2 * MB)];
    sets.extend(records_sets(
        &run,
        1,
        &deleted_five,
        DeleteStatus::NotReached,
    ));
    let outcome = wizard_backup_delete_outcome(&en(), &run, &Ok(stopped_report(sets)));
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped after deleting 1 of 2 backups. Backups it hadn't reached are kept.\n\n\
         One was deleted only in part before RABBIT stopped. The rest of it is kept."
    );
    assert_eq!(label_of(&outcome, RECORDS), "Partly deleted");
}

#[test]
fn the_records_row_with_one_set_left_is_partly_deleted_never_as_it_was() {
    let mut list = backup_list(vec![records()]);
    tick(&mut list, RECORDS, BackupChoice::Delete);
    let run = run_of(&en(), &list);
    let last = run.groups[0].set_ids[22].clone();
    let one_in_use = |index: usize, id: &str| {
        Some(if index == 22 {
            not_deleted(id, NotDeletedReason::InUse)
        } else {
            item(id, DeleteStatus::Deleted, KB)
        })
    };
    let sets = records_sets(&run, 0, &one_in_use, DeleteStatus::NotReached);
    let outcome = wizard_backup_delete_outcome(&en(), &run, &Ok(report(sets)));
    // The row counts as deleted only when every set it takes is gone.
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::NothingDeleted);
    assert_eq!(
        outcome.status_text,
        "RABBIT couldn't delete the backup.\n\nRABBIT deleted part of it, but couldn't delete \
         the rest, which stays as it was. Show details says why."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER".to_string(),
            format!("{RECORDS}: Partly deleted"),
            "  Freed: 22 KB".to_string(),
            "  RABBIT deleted part of it. The rest couldn't be deleted and stays as it was."
                .to_string(),
            "  A file in it is open in another program, so it couldn't be moved.".to_string(),
            format!("  Error: C:/REAPER/RABBIT/backups/{last}: Access is denied. (os error 5)"),
            "Freed in all: 22 KB".to_string(),
            "Report: C:/REAPER/RABBIT/logs/delete-backup-unix-1.txt".to_string(),
        ]
    );

    // Beside a backup that went: one of two, the other only in part.
    let mut list = backup_list(vec![reapack(), records()]);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    tick(&mut list, RECORDS, BackupChoice::Delete);
    let run = run_of(&en(), &list);
    let mut sets = vec![item("unix-1793697120", DeleteStatus::Deleted, 2 * MB)];
    sets.extend(records_sets(&run, 1, &one_in_use, DeleteStatus::NotReached));
    let outcome = wizard_backup_delete_outcome(&en(), &run, &Ok(report(sets)));
    assert_eq!(
        outcome.status,
        BackupDeleteOutcomeStatus::CompletedWithProblems
    );
    assert_eq!(
        outcome.status_text,
        "RABBIT deleted 1 of 2 backups.\n\nOne was deleted only in part: RABBIT couldn't delete \
         the rest of it, which stays as it was. Show details says why."
    );
}

#[test]
fn a_row_whose_sets_went_some_before_the_run_counts_as_deleted() {
    let mut list = backup_list(vec![records()]);
    tick(&mut list, RECORDS, BackupChoice::Delete);
    let run = run_of(&en(), &list);
    let some_gone_before = |index: usize, id: &str| {
        Some(if index < 10 {
            item(id, DeleteStatus::AlreadyGone, 0)
        } else {
            item(id, DeleteStatus::Deleted, KB)
        })
    };
    let sets = records_sets(&run, 0, &some_gone_before, DeleteStatus::NotReached);
    let outcome = wizard_backup_delete_outcome(&en(), &run, &Ok(report(sets)));
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Deleted);
    assert_eq!(
        outcome.status_text,
        "RABBIT deleted one backup and freed 13 KB."
    );
    assert_eq!(label_of(&outcome, RECORDS), "Deleted");
    assert_eq!(below(&outcome, RECORDS), ["Freed: 13 KB"]);
}

#[test]
fn backups_already_gone_or_finished_count_as_no_longer_there() {
    let list = three_alone(&en());
    let outcome = done(
        &list,
        Ok(report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::AlreadyGone, 0),
            item("unix-1793697120", DeleteStatus::FinishedEarlierDeletion, MB),
            item(&set_of(&list, KEYMAP), DeleteStatus::Deleted, MB),
        ])),
    );
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Deleted);
    assert_eq!(
        outcome.status_text,
        "RABBIT deleted 2 backups and freed 2 MB.\n\nOne was already gone."
    );
    assert_eq!(
        &outcome.detail_lines[1..6],
        [
            "FFmpeg for Improved Video Support 7.1: Already gone",
            "ReaPack 1.2.4: Deleted, finishing a deletion that started earlier",
            "  Freed: 1 MB",
            "Your key map: Deleted",
            "  Freed: 1 MB",
        ]
    );

    let outcome = done(
        &list,
        Ok(report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::AlreadyGone, 0),
            item("unix-1793697120", DeleteStatus::AlreadyGone, 0),
            item(&set_of(&list, KEYMAP), DeleteStatus::AlreadyGone, 0),
        ])),
    );
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::AlreadyGone);
    assert_eq!(
        outcome.status_text,
        "The backups were already gone, so there was nothing to delete."
    );
}

#[test]
fn a_refused_run_deletes_nothing_and_says_which_backup_and_why() {
    let list = three_alone(&en());
    let keymap_set = set_of(&list, KEYMAP);
    let outcome = done(
        &list,
        Ok(refused_report(vec![
            item("uninstall-unix-1790938800", DeleteStatus::NotReached, 0),
            item("unix-1793697120", DeleteStatus::NotReached, 0),
            refused(
                &keymap_set,
                RestoreRefusal::ChangedSinceListed {
                    backup_id: keymap_set.clone(),
                },
            ),
        ])),
    );
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Refused);
    assert_eq!(
        outcome.status_text,
        "Nothing was deleted.\n\nRABBIT deletes the ticked backups only when it can delete \
         every one of them.\nRABBIT can't delete this backup: Your key map (Replaced by OSARA's on November \
         1, 2026, 6:40 PM). It changed since RABBIT listed it. Start RABBIT again to see the \
         backups as they are now."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER",
            "FFmpeg for Improved Video Support 7.1: Not reached",
            "  RABBIT deleted nothing in this run, so it is kept.",
            "ReaPack 1.2.4: Not reached",
            "  RABBIT deleted nothing in this run, so it is kept.",
            "Your key map: Can't be deleted",
            "  It changed since RABBIT listed it. Start RABBIT again to see the backups as they \
             are now.",
        ]
    );
    // A refused run saves no report.
    assert!(outcome.report_path.is_none());

    // Alone, the backup isn't said to be one of several.
    let mut one = backup_list(vec![keymap()]);
    tick(&mut one, KEYMAP, BackupChoice::Delete);
    let outcome = done(
        &one,
        Ok(refused_report(vec![refused(
            &keymap_set,
            RestoreRefusal::NotYours {
                backup_id: keymap_set.clone(),
            },
        )])),
    );
    assert_eq!(
        outcome.status_text,
        "Nothing was deleted.\n\nRABBIT can't delete this backup: Your key map (Replaced by OSARA's on \
         November 1, 2026, 6:40 PM). It belongs to another account, so RABBIT doesn't delete it."
    );
}

#[test]
fn a_refused_set_of_several_rows_is_said_as_the_one_backup_that_holds_them() {
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let both = "uninstall-unix-1794233100";
    let outcome = done(
        &list,
        Ok(refused_report(vec![
            refused(
                both,
                RestoreRefusal::ChangedSinceListed {
                    backup_id: both.to_string(),
                },
            ),
            item("unix-1793697120", DeleteStatus::NotReached, 0),
        ])),
    );
    assert_eq!(
        outcome.status_text,
        "Nothing was deleted.\n\nRABBIT deletes the ticked backups only when it can delete \
         every one of them.\nRABBIT can't delete SWS Extension 2.14.0.7 (Uninstalled November 9, \
         2026, 2:05 PM) and FFmpeg for Improved Video Support 7.1 (Uninstalled November 9, \
         2026, 2:05 PM), which one backup holds together. It changed since RABBIT listed it. \
         Start RABBIT again to see the backups as they are now."
    );
    assert_eq!(
        below(
            &outcome,
            "SWS Extension 2.14.0.7 and FFmpeg for Improved Video Support 7.1"
        ),
        [
            "They go together: one backup holds them.",
            "It changed since RABBIT listed it. Start RABBIT again to see the backups as they are \
             now.",
        ]
    );
}

#[test]
fn a_run_refused_or_failed_before_it_started_says_nothing_was_deleted_and_why() {
    let list = three_alone(&en());
    let lock = RestoreRefusal::LockHeld {
        lock_path: PathBuf::from("C:/REAPER/RABBIT/package-install.lock"),
        pid: 42,
    };
    let outcome = done(&list, Err(lock.clone().into()));
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Refused);
    assert_eq!(
        outcome.status_text,
        "Nothing was deleted.\n\nAnother copy of RABBIT is working on this REAPER. Wait until \
         it has finished, then run RABBIT again."
    );
    assert_eq!(
        outcome.detail_lines,
        [
            "Target: C:/REAPER",
            "FFmpeg for Improved Video Support 7.1: Not deleted",
            "ReaPack 1.2.4: Not deleted",
            "Your key map: Not deleted",
            &format!("Error: {}", lock.delete_text()),
        ]
    );
    assert!(outcome.report_path.is_none());

    let outcome = done(&list, Err(RestoreRefusal::AsRoot.into()));
    assert_eq!(
        outcome.status_text,
        "Nothing was deleted.\n\nRABBIT is running as root, as it does under sudo, and doesn't \
         delete backups like that. Quit RABBIT and start it as yourself."
    );

    // A folder the delete reads first: said in the user's language, and
    // as the computer said it in Show details.
    let io = |path: &str, message: &str| RabbitError::Io {
        path: PathBuf::from(path),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, message.to_string()),
    };
    let outcome = done(&list, Err(io(RESOURCE, "not found")));
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Failed);
    assert_eq!(
        outcome.status_text,
        "Nothing was deleted.\n\nRABBIT couldn't read the folder C:/REAPER. Check that it is \
         there and that you can open it, then run RABBIT again."
    );
    assert_eq!(
        outcome.detail_lines.last().map(String::as_str),
        Some("Error: I/O error at C:/REAPER: not found")
    );
    for folder in [
        "C:/REAPER/RABBIT",
        "C:/REAPER/RABBIT/backups",
        "C:/REAPER/RABBIT/backups-deleting",
    ] {
        let outcome = done(&list, Err(io(folder, "denied")));
        assert_eq!(
            outcome.status_text,
            format!(
                "Nothing was deleted.\n\nRABBIT couldn't read the folder {folder}. Check that it \
                 is there and that you can open it, then run RABBIT again."
            )
        );
    }

    // Anything else is said as it came.
    let outcome = done(
        &list,
        Err(io("C:/REAPER/RABBIT/package-install.lock", "broken")),
    );
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Failed);
    assert_eq!(
        outcome.status_text,
        "Nothing was deleted.\n\nSomething went wrong before RABBIT deleted anything: I/O error \
         at C:/REAPER/RABBIT/package-install.lock: broken"
    );
}

#[test]
fn delete_refusals_have_words_of_their_own_and_the_rest_a_restores() {
    assert_eq!(
        delete_refusal_text(
            &en(),
            &RestoreRefusal::IsLink {
                path: PathBuf::from("C:/REAPER/RABBIT/backups")
            }
        ),
        "RABBIT doesn't use C:/REAPER/RABBIT/backups: it is a symbolic link or junction, and \
         RABBIT never follows one in its backups."
    );
    // Never a restore's words where a delete has its own.
    for refusal in delete_refusals() {
        let text = delete_refusal_text(&en(), &refusal);
        assert!(!text.contains("restore"), "{text}");
    }
    // A refusal a delete never gives is said as a restore says it.
    let reinstalled = RestoreRefusal::ReinstalledSince {
        backup_id: "uninstall-unix-1".to_string(),
        package_id: "sws".to_string(),
        other_packages: Vec::new(),
    };
    assert_eq!(
        delete_refusal_text(&en(), &reinstalled),
        restore_refusal_text(&en(), &reinstalled)
    );
}

// ---------------------------------------------------------------------------
// Every language.
// ---------------------------------------------------------------------------

/// The codes of every refusal `delete_backups` gives: of the whole run
/// (root, the lock) and of one item (its id, a link, another account's set,
/// a set changed since it was listed).
const DELETE_REFUSAL_CODES: [&str; 8] = [
    "invalid-id",
    "differs-in-case",
    "ends-in-dot-or-space",
    "is-link",
    "not-yours",
    "changed-since-listed",
    "lock-held",
    "as-root",
];

/// One refusal of each kind `delete_backups` gives.
fn delete_refusals() -> Vec<RestoreRefusal> {
    let refusals: Vec<RestoreRefusal> = sample_refusals()
        .into_iter()
        .filter(|refusal| DELETE_REFUSAL_CODES.contains(&refusal.code()))
        .collect();
    assert_eq!(refusals.len(), DELETE_REFUSAL_CODES.len());
    refusals
}

/// Every status a delete can give a set.
const STATUSES: [DeleteStatus; 9] = [
    DeleteStatus::Deleted,
    DeleteStatus::FinishedEarlierDeletion,
    DeleteStatus::AlreadyGone,
    DeleteStatus::WouldDelete,
    DeleteStatus::WouldFinishEarlierDeletion,
    DeleteStatus::NotDeleted,
    DeleteStatus::PartlyDeleted,
    DeleteStatus::Refused,
    DeleteStatus::NotReached,
];

/// Every reason a set isn't deleted.
const REASONS: [NotDeletedReason; 4] = [
    NotDeletedReason::InUse,
    NotDeletedReason::Permission,
    NotDeletedReason::EarlierDeletionLeft,
    NotDeletedReason::Other,
];

/// That `text` reads in `locale`: no message id, no placeable left.
fn assert_reads_here(locale: &str, text: &str) {
    assert_reads(locale, text);
    for marker in [
        "backup-date",
        "size-",
        "action-",
        "delete-refusal",
        "restore-refusal",
        "delete-status",
    ] {
        assert!(!text.contains(marker), "{locale}: {text}");
    }
}

fn assert_outcome_reads(locale: &str, outcome: &WizardBackupDeleteOutcome) {
    assert_reads_here(locale, &outcome.heading);
    assert!(
        outcome.status_text.starts_with(&outcome.heading),
        "{locale}: {}",
        outcome.status_text
    );
    for line in outcome.status_text.lines().filter(|line| !line.is_empty()) {
        assert_reads_here(locale, line);
    }
    for line in &outcome.detail_lines {
        assert_reads_here(locale, line);
    }
}

#[test]
fn every_delete_status_and_refusal_reads_in_every_locale() {
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for status in STATUSES {
            assert_reads_here(locale, &delete_status_label(&localizer, status));
        }
        for code in DELETE_REFUSAL_CODES {
            let key = format!("wizard-delete-refusal-{code}");
            assert!(!localizer.text(&key).missing, "{locale}: {key}");
        }
        // Every refusal reads, a delete's own and any other.
        for refusal in sample_refusals() {
            assert_reads_here(locale, &delete_refusal_text(&localizer, &refusal));
        }
    }
}

#[test]
fn every_done_page_and_show_details_reads_in_every_locale() {
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let list = three_alone(&localizer);
        let run = backup_delete_run(&localizer, Path::new(RESOURCE), &list);
        let ids: Vec<String> = run.items.iter().map(|item| item.set_id.clone()).collect();
        let check_run = |run: &BackupDeleteRun, result: rabbit_core::Result<DeleteReport>| {
            let outcome = wizard_backup_delete_outcome(&localizer, run, &result);
            assert_outcome_reads(locale, &outcome);
            outcome
        };
        let check = |result: rabbit_core::Result<DeleteReport>| check_run(&run, result);
        let each = |make: &dyn Fn(&str) -> DeleteReportItem| -> Vec<DeleteReportItem> {
            ids.iter().map(|id| make(id)).collect()
        };

        // Every status, for all three and for one among others.
        for status in STATUSES {
            check(Ok(report(each(&|id| item(id, status, 3 * MB)))));
            let mut sets = each(&|id| item(id, DeleteStatus::Deleted, MB));
            sets[1] = item(&ids[1], status, KB);
            check(Ok(report(sets)));
        }
        // Every reason, for all three, for one of three and for two.
        for reason in REASONS {
            check(Ok(report(each(&|id| not_deleted(id, reason)))));
            let mut sets = each(&|id| item(id, DeleteStatus::Deleted, MB));
            sets[2] = not_deleted(&ids[2], reason);
            check(Ok(report(sets.clone())));
            sets[1] = not_deleted(&ids[1], reason);
            check(Ok(report(sets)));
        }
        let mut sets = each(&|id| item(id, DeleteStatus::Deleted, MB));
        sets[0] = partly_deleted(&ids[0], KB);
        check(Ok(report(sets)));
        check(Ok(report(each(&|id| partly_deleted(id, KB)))));
        // Gone before, alone and among others, also in a stopped run.
        let mut sets = each(&|id| item(id, DeleteStatus::Deleted, MB));
        sets[0] = item(&ids[0], DeleteStatus::AlreadyGone, 0);
        check(Ok(report(sets.clone())));
        sets[2] = item(&ids[2], DeleteStatus::NotReached, 0);
        check(Ok(stopped_report(sets)));
        // Stopped, before anything and after one.
        check(Ok(stopped_report(each(&|id| {
            item(id, DeleteStatus::NotReached, 0)
        }))));
        let mut sets = each(&|id| item(id, DeleteStatus::NotReached, 0));
        sets[0] = item(&ids[0], DeleteStatus::Deleted, MB);
        let outcome = check(Ok(stopped_report(sets)));
        assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Stopped);
        // Every refusal, of one item in a refused run and of the whole run.
        for refusal in delete_refusals() {
            let mut sets = each(&|id| item(id, DeleteStatus::NotReached, 0));
            sets[2] = refused(&ids[2], refusal.clone());
            let outcome = check(Ok(refused_report(sets)));
            assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Refused);
            let outcome = check(Err(refusal.into()));
            assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Refused);
        }
        // A folder the delete reads first, and any other error.
        for path in [
            RESOURCE,
            "C:/REAPER/RABBIT/backups",
            "C:/REAPER/RABBIT/x.lock",
        ] {
            let outcome = check(Err(RabbitError::Io {
                path: PathBuf::from(path),
                source: std::io::Error::other("broken"),
            }));
            assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Failed);
        }

        // One backup alone, for the forms that call it "it".
        let one = all_ticked(&localizer, vec![keymap()]);
        let run_one = backup_delete_run(&localizer, Path::new(RESOURCE), &one);
        let id = run_one.items[0].set_id.clone();
        let mut alone: Vec<DeleteReportItem> = REASONS
            .iter()
            .map(|&reason| not_deleted(&id, reason))
            .collect();
        alone.push(partly_deleted(&id, KB));
        alone.push(item(&id, DeleteStatus::AlreadyGone, 0));
        alone.push(item(&id, DeleteStatus::Deleted, KB));
        for set in alone {
            check_run(&run_one, Ok(report(vec![set])));
        }

        // The records row, part of it gone: in use, then stopped; alone
        // and beside another backup.
        for entries in [vec![records()], vec![reapack(), records()]] {
            let records_list = all_ticked(&localizer, entries);
            let run = backup_delete_run(&localizer, Path::new(RESOURCE), &records_list);
            let group = run.groups.len() - 1;
            let ended = |index: usize, id: &str| {
                Some(match index {
                    0 => not_deleted(id, NotDeletedReason::InUse),
                    1 => partly_deleted(id, KB),
                    _ => item(id, DeleteStatus::Deleted, KB),
                })
            };
            let first = |run: &BackupDeleteRun| -> Vec<DeleteReportItem> {
                run.groups[..group]
                    .iter()
                    .map(|group| item(&group.set_ids[0], DeleteStatus::Deleted, MB))
                    .collect()
            };
            let mut sets = first(&run);
            sets.extend(records_sets(&run, group, &ended, DeleteStatus::NotReached));
            check_run(&run, Ok(report(sets)));
            let five =
                |index: usize, id: &str| (index < 5).then(|| item(id, DeleteStatus::Deleted, KB));
            let mut sets = first(&run);
            sets.extend(records_sets(&run, group, &five, DeleteStatus::NotReached));
            let outcome = check_run(&run, Ok(stopped_report(sets)));
            assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Stopped);
        }

        // Rows that go together, refused: "they", then the backup as "it".
        let mut tied = list_in(&localizer, vec![sws(), ffmpeg(), reapack()]);
        apply_backup_checkbox(&mut tied.rows, 1, BackupChoice::Delete, true);
        let run_tied = backup_delete_run(&localizer, Path::new(RESOURCE), &tied);
        assert_eq!(run_tied.groups[0].names.len(), 2);
        let both = run_tied.items[0].set_id.clone();
        for refusal in delete_refusals() {
            check_run(&run_tied, Ok(refused_report(vec![refused(&both, refusal)])));
        }
        check_run(
            &run_tied,
            Ok(report(vec![not_deleted(&both, NotDeletedReason::InUse)])),
        );

        // Two key maps: each said with its date.
        let two = all_ticked(&localizer, vec![keymap(), older_keymap()]);
        let run_two = backup_delete_run(&localizer, Path::new(RESOURCE), &two);
        assert_ne!(run_two.groups[0].labels, run_two.groups[1].labels);
        let sets: Vec<DeleteReportItem> = run_two
            .items
            .iter()
            .map(|item| not_deleted(&item.set_id, NotDeletedReason::Permission))
            .collect();
        check_run(&run_two, Ok(report(sets)));

        // The progress page's lines.
        let progress_runs = [&run, &run_tied, &run_two];
        for run in progress_runs {
            for status in STATUSES {
                let mut state = ProgressUiState::new(WizardMode::DeleteBackups, 3, 0)
                    .with_backup_delete(backup_delete_progress(&localizer, run));
                let set = &run.items[0].set_id;
                let (status_line, log) =
                    backup_delete_progress_lines(&localizer, &mut state, &started(set));
                assert_reads_here(locale, &status_line.unwrap());
                assert_reads_here(locale, &log.unwrap());
                if let (_, Some(log)) = backup_delete_progress_lines(
                    &localizer,
                    &mut state,
                    &completed(set, status, MB),
                ) {
                    assert_reads_here(locale, &log);
                }
            }
        }
        assert_reads_here(
            locale,
            &backup_delete_progress_details_for_start(&localizer, Path::new(RESOURCE), &list),
        );

        // The progress page's heading and first status line.
        let text = crate::wizard::text::wizard_text(&localizer);
        assert_reads_here(locale, &text.progress_delete_heading);
        assert_reads_here(locale, &text.progress_status_delete_running);
    }
}

// ---------------------------------------------------------------------------
// The run button and the close question.
// ---------------------------------------------------------------------------

#[test]
fn the_delete_review_decides_whether_the_button_is_enabled() {
    let mut list = backup_list(vec![sws(), reapack()]);
    let review = build_backup_delete_review(&en(), Some(&target(true)), &list, &[]);
    assert!(!review.can_delete);
    assert!(!WizardMode::DeleteBackups.run_button_enabled(true, true, true, review.can_delete));
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let review = build_backup_delete_review(&en(), Some(&target(true)), &list, &[]);
    assert!(review.can_delete);
    assert!(WizardMode::DeleteBackups.run_button_enabled(false, false, false, review.can_delete));
}

#[test]
fn closing_the_window_during_a_delete_asks_in_the_deletes_words() {
    for &locale in embedded_locales() {
        let text = crate::wizard::text::wizard_text(&Localizer::embedded(locale).unwrap());
        assert_eq!(
            stop_question(&text, WizardMode::DeleteBackups),
            (
                text.close_during_delete_title.as_str(),
                text.close_during_delete_body.as_str()
            ),
            "{locale}"
        );
        assert_eq!(
            stop_question(&text, WizardMode::Restore),
            (
                text.close_during_restore_title.as_str(),
                text.close_during_restore_body.as_str()
            ),
            "{locale}"
        );
        assert_eq!(
            stop_question(&text, WizardMode::Uninstall),
            (
                text.close_during_uninstall_title.as_str(),
                text.close_during_uninstall_body.as_str()
            ),
            "{locale}"
        );
        assert_eq!(
            stop_question(&text, WizardMode::Install),
            (
                text.close_during_install_title.as_str(),
                text.close_during_install_body.as_str()
            ),
            "{locale}"
        );
        // A delete stops after the backup it is on, and says so with the
        // install's line.
        assert_eq!(
            stopping_line(&text, WizardMode::DeleteBackups),
            text.progress_status_cancelling,
            "{locale}"
        );
        assert_eq!(
            stopping_line(&text, WizardMode::Uninstall),
            text.progress_status_uninstall_cancelling,
            "{locale}"
        );
    }
    let text = crate::wizard::text::wizard_text(&en());
    assert_eq!(text.close_during_delete_title, "Stop deleting backups?");
    assert_eq!(text.progress_delete_heading, "Deleting backups");
    assert_eq!(
        text.progress_status_delete_running,
        "Deleting the ticked backups."
    );
}

// ---------------------------------------------------------------------------
// A real delete, in a temporary folder.
// ---------------------------------------------------------------------------

/// An uninstall set `set_id` in `resource` holding ReaPack 1.2.4, moved
/// there the way an uninstall moves it.
fn write_reapack_set(resource: &Path, set_id: &str) {
    let scripts = resource.join("UserPlugins");
    fs::create_dir_all(&scripts).unwrap();
    let file = scripts.join("reaper_reapack-x64.dll");
    fs::write(&file, b"reapack").unwrap();
    let mut writer = BackupSetWriter::new(resource, set_id, UNINSTALL_BACKUP_REASON);
    writer
        .record_package(
            "reapack",
            BackupPackageRecord {
                receipt: Some(PackageReceipt {
                    id: "reapack".to_string(),
                    version: Some(version("1.2.4")),
                    source_url: None,
                    source_sha256: None,
                    variant: None,
                    channel: None,
                    installed_files: Vec::new(),
                    installed_at: None,
                    rabbit_version: None,
                    architecture: None,
                    vendor_added_files: Vec::new(),
                }),
                langpack_before: None,
                with_settings: Some(false),
            },
        )
        .unwrap();
    writer
        .move_file(Some("reapack"), &file, None, false)
        .unwrap();
}

const BOTH_SET: &str = "uninstall-unix-1794233100";
const REAPACK_SET: &str = "uninstall-unix-1790938800";

fn set_path(resource: &Path, set_id: &str) -> PathBuf {
    resource.join("RABBIT").join("backups").join(set_id)
}

#[test]
fn a_real_delete_takes_exactly_the_ticked_sets_and_saves_its_report_there() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    fs::create_dir_all(&resource).unwrap();
    write_uninstall_set(&resource);
    write_reapack_set(&resource, REAPACK_SET);
    assert!(set_path(&resource, BOTH_SET).is_dir());
    assert!(set_path(&resource, REAPACK_SET).is_dir());

    let mut list = backup_list_at(&en(), &resource);
    set_backup_choice(&mut list, BackupChoice::Delete);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    let run = backup_delete_run(&en(), &resource, &list);
    assert_eq!(run.items.len(), 1);
    assert_eq!(run.backups(), 2);

    let events: Arc<Mutex<Vec<ProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let progress = {
        let events = Arc::clone(&events);
        ProgressReporter::new(move |event| events.lock().unwrap().push(event))
    };
    let result = run_backup_delete(&run, &progress, &CancelToken::new());
    let report = result.as_ref().unwrap();
    assert!(!report.refused && !report.stopped);

    // The ticked set is gone, and nothing of it is left to finish; the
    // other set is as it was.
    assert!(!set_path(&resource, BOTH_SET).exists());
    assert!(
        !resource
            .join("RABBIT")
            .join("backups-deleting")
            .join(BOTH_SET)
            .exists()
    );
    assert!(
        set_path(&resource, REAPACK_SET)
            .join(BACKUP_MANIFEST_FILE)
            .is_file()
    );

    // The report is in RABBIT/logs of this REAPER, as the command line's.
    let saved = report.saved_report.as_ref().unwrap();
    let logs = resource.join("RABBIT").join("logs");
    assert_eq!(saved.json_path.parent(), Some(logs.as_path()));
    assert!(saved.json_path.is_file() && saved.text_path.is_file());
    let name = saved.json_path.file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("delete-backup-unix-"), "{name}");

    // The progress page heard the set start and end.
    let events = events.lock().unwrap();
    assert!(matches!(
        &events[..],
        [
            ProgressEvent::DeleteStarted { set_id },
            ProgressEvent::DeleteCompleted {
                status: DeleteStatus::Deleted,
                ..
            },
        ] if set_id == BOTH_SET
    ));

    // Done counts the two rows of the set, and points to the report.
    let outcome = wizard_backup_delete_outcome(&en(), &run, &result);
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Deleted);
    assert!(
        outcome
            .heading
            .starts_with("RABBIT deleted 2 backups and freed "),
        "{}",
        outcome.heading
    );
    assert_eq!(outcome.report_path.as_ref(), Some(&saved.text_path));
    assert_eq!(
        outcome.detail_lines.last(),
        Some(&format!("Report: {}", saved.text_path.display()))
    );

    // The rescan after the run reads the live list again: the deleted rows
    // are gone, no tick is kept, and Restore is chosen again.
    let (_, model) = langpack_model(rabbit_core::plan::PlanActionKind::Install);
    let target = TargetRow {
        path: resource.clone(),
        ..target(true)
    };
    assert_eq!(list.choice, BackupChoice::Delete);
    assert_eq!(list.ticked_rows(BackupChoice::Delete).count(), 2);
    reread_backup_list(&mut list, &model, &target);
    let names: Vec<&str> = list.rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["Software you uninstalled", "ReaPack 1.2.4"]);
    assert_eq!(list.choice, BackupChoice::Restore);
    assert!(
        list.rows
            .iter()
            .all(|row| !row.restore_ticked && !row.delete_ticked)
    );
}

#[test]
fn a_delete_stopped_before_the_first_set_deletes_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    fs::create_dir_all(&resource).unwrap();
    write_uninstall_set(&resource);
    write_reapack_set(&resource, REAPACK_SET);

    let mut list = backup_list_at(&en(), &resource);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let run = backup_delete_run(&en(), &resource, &list);
    assert_eq!(run.items.len(), 2);

    let cancel = CancelToken::new();
    cancel.cancel();
    let result = run_backup_delete(&run, &ProgressReporter::noop(), &cancel);
    let report = result.as_ref().unwrap();
    assert!(report.stopped);
    assert!(
        report
            .sets
            .iter()
            .all(|set| set.status == DeleteStatus::NotReached)
    );
    assert!(set_path(&resource, BOTH_SET).is_dir());
    assert!(set_path(&resource, REAPACK_SET).is_dir());

    let outcome = wizard_backup_delete_outcome(&en(), &run, &result);
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Stopped);
    assert_eq!(
        outcome.status_text,
        "RABBIT stopped before it deleted a backup. Backups it hadn't reached are kept."
    );
}

#[test]
fn a_set_changed_since_listing_refuses_the_run_and_deletes_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    fs::create_dir_all(&resource).unwrap();
    write_uninstall_set(&resource);
    write_reapack_set(&resource, REAPACK_SET);

    let mut list = backup_list_at(&en(), &resource);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, REAPACK, BackupChoice::Delete);
    let run = backup_delete_run(&en(), &resource, &list);
    // Another program changes one set after the list was read.
    let manifest = set_path(&resource, REAPACK_SET).join(BACKUP_MANIFEST_FILE);
    let mut text = fs::read_to_string(&manifest).unwrap();
    text.push_str("\n\n");
    fs::write(&manifest, text).unwrap();

    let result = run_backup_delete(&run, &ProgressReporter::noop(), &CancelToken::new());
    let report = result.as_ref().unwrap();
    assert!(report.refused);
    assert!(report.saved_report.is_none());
    assert!(!resource.join("RABBIT").join("logs").exists());
    assert!(set_path(&resource, BOTH_SET).is_dir());
    assert!(set_path(&resource, REAPACK_SET).is_dir());

    let outcome = wizard_backup_delete_outcome(&en(), &run, &result);
    assert_eq!(outcome.status, BackupDeleteOutcomeStatus::Refused);
    assert!(
        outcome.status_text.starts_with(
            "Nothing was deleted.\n\nRABBIT deletes the ticked backups only when it can delete \
             every one of them.\nRABBIT can't delete this backup: ReaPack 1.2.4 (Uninstalled "
        ),
        "{}",
        outcome.status_text
    );
    assert!(
        outcome
            .status_text
            .ends_with("It changed since RABBIT listed it. Start RABBIT again to see the backups as they are now."),
        "{}",
        outcome.status_text
    );
}
