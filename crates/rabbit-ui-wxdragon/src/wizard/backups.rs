//! The Backups tab, without wxWidgets: one row for each backup RABBIT kept
//! for the selected REAPER, grouped by what you get back, and what ticking a
//! row does in either choice below the list, Restore or Delete. Also what a
//! restore or a delete looks like before it starts: Review's text and its
//! blockers, the run's input, and the question asked before a delete.
//!
//! A backup set is one folder in `RABBIT/backups`. A backup, here, is one
//! thing that can come back: one package from a set, or one key map. One set
//! can hold several backups, so it can give several rows. Until a later
//! version, a delete takes whole sets, so rows from one set tick together
//! for Delete; ticks for Restore never do.
//!
//! The rows come from `rabbit-core`'s overview of the backups, the one
//! `rabbit backups` prints, and carry its verdicts: a row can be ticked for
//! Restore only when the overview found nothing that would refuse the
//! restore, so a row and a run agree. Building the overview reads manifests,
//! RABBIT's records and file sizes and hashes nothing, so the version check's
//! handler can build the rows on the UI thread, next to the Uninstall rows.
//!
//! Nothing is ticked until the user ticks it. Each choice keeps its own
//! ticks: switching relabels the rows and never moves a tick from one choice
//! to the other.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use rabbit_core::RabbitError;
use rabbit_core::backup_delete::DeleteItem;
use rabbit_core::backup_overview::{
    BackupEntry, BackupGroup, BackupItem, BackupOverview, BackupSetKind, BackupSize, BackupStatus,
    RestoreScope, SetFingerprint, backup_overview,
};
use rabbit_core::localization::{FluentArgument, Localizer};
use rabbit_core::preflight::{PreflightStatus, REAPER_PROCESS_CHECK};
use rabbit_core::restore_refusal::{RestoreRefusal, TargetRefusal};
use rabbit_core::rollback::{
    KeymapOrigin, RestoreItem, RestorePart, RestoreReportItem, RestoreRunOptions, RestoreRunReport,
    SwappedEntry, same_place,
};
use rabbit_core::uninstall::ANY_REAPER_PROCESS_CHECK;
use rabbit_core::version::Version;

use super::bootstrap::localizer_from_options;
use super::model::{TargetRow, WizardModel};
use super::packages::localized_package_display_name;
use super::uninstall::details_text;

/// What Next does with the ticked backups: the choice below the list.
/// Restore is where the tab starts, and where the version check sets it
/// back to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BackupChoice {
    #[default]
    Restore,
    Delete,
}

/// One folder of RABBIT's backup folders, as Review and a run need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSetFacts {
    pub id: String,
    /// In `RABBIT/backups`, or in `RABBIT/backups-deleting` for an
    /// unfinished deletion.
    pub path: PathBuf,
    pub deleting: bool,
    /// Bytes on disk, from the overview's walk: what deleting it frees.
    pub size: u64,
    /// What a run compares to tell whether it changed since it was listed.
    pub fingerprint: Option<SetFingerprint>,
}

/// What the Backups list is built from: the overview's backups, with the
/// sets and the files each restore writes, copied out of it. Tests build
/// one by hand to reach every status.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackupListSource {
    pub entries: Vec<BackupEntry>,
    pub sets: Vec<BackupSetFacts>,
    /// The files a restore of each backup writes, for Review's check that
    /// two ticked backups don't write the same file. Backups that write
    /// none are left out.
    pub targets: BTreeMap<BackupItem, Vec<PathBuf>>,
    /// Bytes on disk of every set.
    pub total_size: u64,
}

impl BackupListSource {
    pub fn from_overview(overview: &BackupOverview) -> Self {
        Self {
            entries: overview.entries.clone(),
            sets: overview
                .sets
                .iter()
                .map(|set| BackupSetFacts {
                    id: set.id.clone(),
                    path: set.path.clone(),
                    deleting: set.kind == BackupSetKind::Deleting,
                    size: set.size,
                    fingerprint: set.fingerprint.clone(),
                })
                .collect(),
            targets: overview
                .entries
                .iter()
                .map(|entry| (entry.item.clone(), overview.restore_targets(&entry.item)))
                .filter(|(_, targets)| !targets.is_empty())
                .collect(),
            total_size: overview.total_size,
        }
    }
}

/// The labels of one row, for each choice, ticked and not. Ticking and
/// switching only swap them, so neither needs a localizer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BackupRowLabels {
    keep_for_restore: String,
    restore: String,
    keep_for_delete: String,
    delete: String,
}

impl BackupRowLabels {
    fn get(&self, choice: BackupChoice, ticked: bool) -> &str {
        match (choice, ticked) {
            (BackupChoice::Restore, false) => &self.keep_for_restore,
            (BackupChoice::Restore, true) => &self.restore,
            (BackupChoice::Delete, false) => &self.keep_for_delete,
            (BackupChoice::Delete, true) => &self.delete,
        }
    }

    fn all(label: &str) -> Self {
        Self {
            keep_for_restore: label.to_string(),
            restore: label.to_string(),
            keep_for_delete: label.to_string(),
            delete: label.to_string(),
        }
    }
}

/// One row of the Backups list: a group's heading, which has no checkbox,
/// so one key press can't tick a group, or one backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRow {
    /// The group this row heads. `None` for a backup's row.
    pub heading: Option<BackupGroup>,
    /// The backup, as the overview lists it. `None` for a heading.
    pub entry: Option<BackupEntry>,
    /// What the backup is, as its row names it, such as "SWS Extension
    /// 2.14.0.7" or "Your key map". A heading's is the group's name.
    pub name: String,
    /// When the backup was made, as its row says it after the action, such
    /// as "Uninstalled November 9, 2026, 2:05 PM". Empty for a heading.
    pub when: String,
    /// The row can be ticked while Restore is chosen. Only backups the
    /// overview found nothing against can, so the run agrees.
    pub can_restore: bool,
    /// The row can be ticked while Delete is chosen: every backup but
    /// another account's, what is no backup, and what goes with either.
    pub can_delete: bool,
    /// Ticked to restore. Kept while Delete is chosen.
    pub restore_ticked: bool,
    /// Ticked to delete. Kept while Restore is chosen.
    pub delete_ticked: bool,
    /// The row's label for the choice it was last labelled for. It starts
    /// with the action, so a screen reader says what will happen first.
    pub summary: String,
    /// The details text: the label again, then what the backup is, when it
    /// was made, what it holds, whether it is the only copy, what restoring
    /// does or why it can't, and what deleting loses.
    pub details: String,
    pub(crate) labels: BackupRowLabels,
    pub(crate) explanation: String,
    /// When the backup was made, as the user's language writes a date.
    /// `None` without a date, and for a heading.
    pub(crate) date: Option<String>,
    /// The sets a delete of this backup takes, by id: its own, or the
    /// records' several.
    pub(crate) set_ids: Vec<String>,
    /// The files a restore of it writes.
    pub(crate) targets: Vec<PathBuf>,
    /// What a restore run is given for it, when it can be restored.
    pub(crate) restore_item: Option<RestoreItem>,
}

impl BackupRow {
    pub fn is_heading(&self) -> bool {
        self.heading.is_some()
    }

    /// Whether the row can be ticked while `choice` is chosen. A heading
    /// never can.
    pub fn can_tick(&self, choice: BackupChoice) -> bool {
        !self.is_heading()
            && match choice {
                BackupChoice::Restore => self.can_restore,
                BackupChoice::Delete => self.can_delete,
            }
    }

    /// Whether the row is ticked for `choice`.
    pub fn is_ticked(&self, choice: BackupChoice) -> bool {
        match choice {
            BackupChoice::Restore => self.restore_ticked,
            BackupChoice::Delete => self.delete_ticked,
        }
    }

    /// The sets a delete of this backup takes, by id.
    pub fn set_ids(&self) -> &[String] {
        &self.set_ids
    }

    /// The files a restore of this backup writes.
    pub fn restore_targets(&self) -> &[PathBuf] {
        &self.targets
    }

    fn set_ticked(&mut self, choice: BackupChoice, ticked: bool) {
        match choice {
            BackupChoice::Restore => self.restore_ticked = ticked,
            BackupChoice::Delete => self.delete_ticked = ticked,
        }
    }

    /// Give the row the label and details of `choice`, as it is ticked.
    fn relabel(&mut self, choice: BackupChoice) {
        self.summary = self.labels.get(choice, self.is_ticked(choice)).to_string();
        self.details = details_text(&self.summary, &self.explanation);
    }
}

/// What the Backups tab lists for a target.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WizardBackupList {
    /// Headings and backups, in list order: the groups in their order, each
    /// only when it has backups, newest first inside a group, backups
    /// without a date last, and the old copies of RABBIT's records last of
    /// all.
    pub rows: Vec<BackupRow>,
    /// The choice the rows are labelled for.
    pub choice: BackupChoice,
    /// Why there are no rows when RABBIT couldn't read its backup folders,
    /// as the error said it. `None` with no rows means there are none.
    pub unreadable: Option<String>,
    /// Bytes on disk of every set, for the list's label.
    pub total_size: u64,
    pub(crate) sets: Vec<BackupSetFacts>,
}

impl WizardBackupList {
    /// The rows ticked for `choice`, in list order. Only rows that can be
    /// ticked for it count.
    pub fn ticked_rows(&self, choice: BackupChoice) -> impl Iterator<Item = &BackupRow> {
        self.rows
            .iter()
            .filter(move |row| row.can_tick(choice) && row.is_ticked(choice))
    }

    /// The label right before the list. It names the tab first and gives
    /// the size of every backup together.
    pub fn list_label(&self, localizer: &Localizer) -> String {
        if self.rows.is_empty() {
            return text(localizer, "wizard-backups-list-label-empty");
        }
        let size = backup_size_text(localizer, self.total_size);
        say(
            localizer,
            "wizard-backups-list-label",
            &[("size", (&size).into())],
        )
    }

    /// The label and the details text that stand in for an empty list: the
    /// Windows list shows the label as a row without a checkbox, macOS says
    /// it (see [`Self::tab_announcement`]), and both show the details text.
    /// `None` while there are rows.
    pub fn placeholder(&self, localizer: &Localizer) -> Option<(String, String)> {
        if !self.rows.is_empty() {
            return None;
        }
        Some(match &self.unreadable {
            Some(error) => (
                text(localizer, "wizard-backups-unreadable"),
                say(
                    localizer,
                    "wizard-backups-unreadable-details",
                    &[("error", error.into())],
                ),
            ),
            None => (
                text(localizer, "wizard-backups-empty"),
                text(localizer, "wizard-backups-empty-details"),
            ),
        })
    }

    /// What macOS says when the Backups tab is switched to: the tab's name,
    /// then, for an empty list, the placeholder's sentence, which the macOS
    /// list has no row for.
    pub fn tab_announcement(&self, localizer: &Localizer) -> String {
        let tab = text(localizer, "wizard-packages-tab-backups");
        match self.placeholder(localizer) {
            Some((label, _)) => format!("{tab}. {label}"),
            None => tab,
        }
    }
}

/// The Backups tab's rows for `target`, in the language the wizard runs in.
pub fn backup_list_for_target(model: &WizardModel, target: &TargetRow) -> WizardBackupList {
    let Ok(localizer) = localizer_from_options(&model.bootstrap_options) else {
        return WizardBackupList::default();
    };
    backup_list_at(&localizer, &target.path)
}

/// The Backups tab's rows for the REAPER at `resource_path`, from the
/// overview `rabbit backups` prints, with dates in this computer's local
/// time. Backup folders that can't be read, or that are links, leave the
/// list empty with the reason.
pub fn backup_list_at(localizer: &Localizer, resource_path: &Path) -> WizardBackupList {
    match backup_overview(resource_path) {
        Ok(overview) => backup_list_from_overview(localizer, &overview),
        Err(error) => backup_list_unreadable(&error),
    }
}

/// The rows for every backup `overview` lists, with dates in this
/// computer's local time.
pub fn backup_list_from_overview(
    localizer: &Localizer,
    overview: &BackupOverview,
) -> WizardBackupList {
    backup_list_from_source(
        localizer,
        &BackupListSource::from_overview(overview),
        &local_utc_offset_at,
    )
}

/// An empty list that says why RABBIT couldn't read its backup folders.
pub fn backup_list_unreadable(error: &RabbitError) -> WizardBackupList {
    WizardBackupList {
        unreadable: Some(error.to_string()),
        ..WizardBackupList::default()
    }
}

/// The rows for every backup in `source`, with each date shifted by what
/// `local_offset` says local time was ahead of UTC at that moment, in
/// seconds. Nothing is ticked, and the rows are labelled for Restore.
pub fn backup_list_from_source(
    localizer: &Localizer,
    source: &BackupListSource,
    local_offset: &dyn Fn(u64) -> i32,
) -> WizardBackupList {
    let mut entries: Vec<&BackupEntry> = source.entries.iter().collect();
    // The overview sorts them so already; a list built by hand is sorted
    // the same way. The sort is stable, so equal ones keep their order.
    entries.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then_with(|| is_records(left).cmp(&is_records(right)))
            .then_with(|| newest_first(left.created_unix).cmp(&newest_first(right.created_unix)))
    });
    let mut drafts: Vec<RowDraft> = entries
        .into_iter()
        .map(|entry| RowDraft::new(localizer, source, entry, local_offset))
        .collect();
    tie_drafts(localizer, &mut drafts);

    let mut rows = Vec::new();
    for draft in drafts {
        if rows.last().map(row_group) != Some(draft.entry.group) {
            rows.push(heading_row(localizer, draft.entry.group));
        }
        rows.push(draft.into_row(localizer));
    }
    WizardBackupList {
        rows,
        choice: BackupChoice::Restore,
        unreadable: None,
        total_size: source.total_size,
        sets: source.sets.clone(),
    }
}

/// What changed when a row was ticked or unticked, for the two list
/// implementations to refresh.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackupTickChange {
    /// The row asked for changed.
    pub changed: bool,
    /// The other rows whose tick changed with it, by index, in list order:
    /// rows from the same set, which a delete takes together.
    pub tied: Vec<usize>,
}

/// Tick or untick the row at `index` for `choice`. A heading, or a row that
/// can't be ticked for `choice`, stays unticked. For Delete, every row from
/// the same set ticks and unticks with it, since a delete takes whole sets;
/// ticks for Restore are never tied. Every row ends up labelled for
/// `choice` as it is ticked, so the list can show it as it is.
pub fn apply_backup_checkbox(
    rows: &mut [BackupRow],
    index: usize,
    choice: BackupChoice,
    checked: bool,
) -> BackupTickChange {
    let Some(row) = rows.get(index) else {
        return BackupTickChange::default();
    };
    if !row.can_tick(choice) {
        rows[index].set_ticked(choice, false);
        rows[index].relabel(choice);
        return BackupTickChange::default();
    }
    let tied = match choice {
        BackupChoice::Restore => Vec::new(),
        BackupChoice::Delete => {
            let lists: Vec<&[String]> = rows.iter().map(|row| row.set_ids.as_slice()).collect();
            tied_indices(&lists, index)
        }
    };
    let mut change = BackupTickChange {
        changed: rows[index].is_ticked(choice) != checked,
        tied: Vec::new(),
    };
    rows[index].set_ticked(choice, checked);
    rows[index].relabel(choice);
    for other in tied {
        let row = &mut rows[other];
        if !row.can_tick(choice) {
            continue;
        }
        if row.is_ticked(choice) != checked {
            change.tied.push(other);
        }
        row.set_ticked(choice, checked);
        row.relabel(choice);
    }
    change
}

/// Switch the choice below the list. The ticks of both choices stay as
/// they are; only the rows' labels change.
pub fn set_backup_choice(list: &mut WizardBackupList, choice: BackupChoice) {
    list.choice = choice;
    for row in &mut list.rows {
        row.relabel(choice);
    }
}

/// Clear the ticks of both choices and choose Restore again, as the version
/// check does when it rebuilds the tab.
pub fn reset_backup_ticks(list: &mut WizardBackupList) {
    list.choice = BackupChoice::Restore;
    for row in &mut list.rows {
        row.restore_ticked = false;
        row.delete_ticked = false;
        row.relabel(BackupChoice::Restore);
    }
}

/// What VoiceOver says after a switch of the choice, such as "2 backups
/// ticked to delete.": the rows changed away from focus, so nothing would
/// say it otherwise.
pub fn backups_ticked_announcement(localizer: &Localizer, list: &WizardBackupList) -> String {
    let count = list.ticked_rows(list.choice).count();
    let choice = match list.choice {
        BackupChoice::Restore => "restore",
        BackupChoice::Delete => "delete",
    };
    say(
        localizer,
        "wizard-backups-ticked-macos",
        &[("choice", choice.into()), ("count", count.into())],
    )
}

/// What VoiceOver adds after "Checked" or "Unchecked" when a Delete tick on
/// the row at `index` ticked or unticked rows from the same set too, such
/// as "SWS Extension 2.14.0.7 from the same uninstall is ticked too.".
/// `None` when no other row changed.
pub fn backups_also_ticked_announcement(
    localizer: &Localizer,
    rows: &[BackupRow],
    index: usize,
    change: &BackupTickChange,
) -> Option<String> {
    if change.tied.is_empty() {
        return None;
    }
    let names: Vec<String> = change
        .tied
        .iter()
        .filter_map(|&other| rows.get(other))
        .map(|row| row.name.clone())
        .collect();
    let ticked = rows.get(index).is_some_and(|row| row.delete_ticked);
    let origin = rows
        .get(index)
        .and_then(|row| row.entry.as_ref())
        .map_or("other", |entry| origin_of(entry.set_kind));
    let backups = and_list(localizer, &names);
    Some(say(
        localizer,
        if ticked {
            "wizard-backups-also-ticked-macos"
        } else {
            "wizard-backups-also-unticked-macos"
        },
        &[
            ("backups", (&backups).into()),
            ("count", names.len().into()),
            ("origin", origin.into()),
        ],
    ))
}

/// What a restore run is given: each row ticked for Restore, in list order,
/// as the backup and part of its set it restores, with the set's
/// fingerprint from the overview, so a set that changed since is refused.
pub fn backup_restore_items(list: &WizardBackupList) -> Vec<RestoreItem> {
    list.ticked_rows(BackupChoice::Restore)
        .filter_map(|row| row.restore_item.clone())
        .collect()
}

/// What a delete run is given: each set the rows ticked for Delete take,
/// once, in list order, with its fingerprint from the overview. An id that
/// names a set in `RABBIT/backups` carries that set's fingerprint, also when
/// only its unfinished deletion was ticked, since a delete takes both.
pub fn backup_delete_items(list: &WizardBackupList) -> Vec<DeleteItem> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for row in list.ticked_rows(BackupChoice::Delete) {
        for set_id in &row.set_ids {
            if seen.insert(set_id.clone()) {
                items.push(DeleteItem {
                    set_id: set_id.clone(),
                    fingerprint: set_fingerprint(&list.sets, set_id),
                });
            }
        }
    }
    items
}

/// The bytes a delete of the rows ticked for Delete frees: every folder of
/// each set they take, unfinished deletions included, as the overview's
/// walk measured them.
pub fn backup_delete_bytes(list: &WizardBackupList) -> u64 {
    let ids: BTreeSet<&str> = list
        .ticked_rows(BackupChoice::Delete)
        .flat_map(|row| row.set_ids.iter().map(String::as_str))
        .collect();
    list.sets
        .iter()
        .filter(|set| ids.contains(set.id.as_str()))
        .map(|set| set.size)
        .sum()
}

/// How the quick plan before Review runs: a dry run that reads no file's
/// content, with REAPER not allowed to run, so it reports a running REAPER
/// without refusing for it.
pub fn backup_quick_plan_options() -> RestoreRunOptions {
    RestoreRunOptions {
        dry_run: true,
        quick: true,
        allow_reaper_running: false,
    }
}

/// What the quick plan of a restore found, for Review: its report, or why
/// it refused the whole run, or what else went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreQuickPlan {
    Planned(RestoreRunReport),
    Refused(RestoreRefusal),
    Failed(String),
}

impl RestoreQuickPlan {
    /// What `restore_backups` returned for the quick plan.
    pub fn from_result(result: rabbit_core::Result<RestoreRunReport>) -> Self {
        match result {
            Ok(report) => Self::Planned(report),
            Err(RabbitError::RestoreRefused(refusal)) => Self::Refused(refusal),
            Err(error) => Self::Failed(error.to_string()),
        }
    }

    fn report(&self) -> Option<&RestoreRunReport> {
        match self {
            Self::Planned(report) => Some(report),
            Self::Refused(_) | Self::Failed(_) => None,
        }
    }
}

/// What Review shows before a restore, and the run it would start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardRestoreReviewPreview {
    pub heading: String,
    pub lines: Vec<String>,
    /// A target, at least one row ticked to restore, and no blocker.
    pub can_restore: bool,
    /// The run's input, copied out of the list (see
    /// [`backup_restore_items`]).
    pub items: Vec<RestoreItem>,
}

/// Review's text for restoring what is ticked for Restore in `list` into
/// `target`, in plain sentences without set ids or file lists:
/// 1. the target folder;
/// 2. one sentence per ticked backup, in list order, saying what comes back
///    and what RABBIT keeps as a backup;
/// 3. the notes: what the quick plan says changes besides the files, such
///    as REAPER's language, then "Close REAPER first." when a REAPER it
///    needs closed is running, `other_tab_notes` (what is ticked on the
///    other tabs, which the caller words), and the backups ticked for
///    Delete, which this run leaves out;
/// 4. the blockers, each starting "Can't restore yet": nothing ticked, two
///    ticked backups of one package, two that write the same file, a target
///    RABBIT can't write to, a quick plan that is missing or covers other
///    backups than the ticked ones, and every refusal of the quick plan.
///
/// `quick_plan` is what a quick dry run of [`backup_restore_items`] found
/// (see [`backup_quick_plan_options`]). `None`, before it ran, adds no notes
/// and blocks the restore, as does a plan of other backups than those
/// ticked now: Restore is never enabled for backups nothing checked.
pub fn build_backup_restore_review(
    localizer: &Localizer,
    target: Option<&TargetRow>,
    list: &WizardBackupList,
    quick_plan: Option<&RestoreQuickPlan>,
    other_tab_notes: &[String],
) -> WizardRestoreReviewPreview {
    let heading = text(localizer, "wizard-review-restore-heading");
    let items = backup_restore_items(list);
    let Some(target) = target else {
        return WizardRestoreReviewPreview {
            heading,
            lines: vec![text(localizer, "wizard-review-no-target")],
            can_restore: false,
            items,
        };
    };
    let ticked: Vec<&BackupRow> = list.ticked_rows(BackupChoice::Restore).collect();
    let report = quick_plan.and_then(RestoreQuickPlan::report);
    let planned = |row: &BackupRow| {
        report.and_then(|report| {
            row.restore_item
                .as_ref()
                .and_then(|item| report_item(report, item))
        })
    };

    let mut lines = vec![say(
        localizer,
        "wizard-review-target",
        &[("path", (&target.path.display().to_string()).into())],
    )];
    if !ticked.is_empty() {
        lines.push(String::new());
    }
    for row in &ticked {
        lines.push(restore_line(localizer, row, planned(row)));
    }

    let mut notes: Vec<String> = Vec::new();
    let mut add_note = |note: String| {
        if !notes.contains(&note) {
            notes.push(note);
        }
    };
    // The first restore in list order that puts REAPER's language back
    // sets it; the later ones then find a language chosen and leave it.
    let mut language_set = false;
    for row in &ticked {
        if let Some(item) = planned(row) {
            restore_notes(localizer, item, &mut language_set)
                .into_iter()
                .for_each(&mut add_note);
        }
    }
    if let Some(report) = report.filter(|_| !ticked.is_empty()) {
        let running = |name: &str| {
            report
                .preflight
                .checks
                .iter()
                .any(|check| check.name == name && check.status != PreflightStatus::Pass)
        };
        if running(ANY_REAPER_PROCESS_CHECK) {
            add_note(text(localizer, "wizard-review-restore-close-every-reaper"));
        } else if running(REAPER_PROCESS_CHECK) {
            add_note(text(localizer, "wizard-review-restore-close-reaper"));
        }
    }
    other_tab_notes.iter().cloned().for_each(&mut add_note);
    let ticked_to_delete = list.ticked_rows(BackupChoice::Delete).count();
    if ticked_to_delete > 0 {
        add_note(say(
            localizer,
            "wizard-review-restore-other-choice-note",
            &[("count", ticked_to_delete.into())],
        ));
    }
    if !notes.is_empty() {
        lines.push(String::new());
        lines.push(text(localizer, "wizard-review-notes-heading"));
        lines.extend(notes);
    }

    let blockers = restore_blockers(localizer, target, &ticked, &items, quick_plan, &planned);
    if !blockers.is_empty() {
        lines.push(String::new());
        lines.extend(blockers.iter().cloned());
    }
    WizardRestoreReviewPreview {
        heading,
        lines,
        can_restore: blockers.is_empty(),
        items,
    }
}

/// What Review shows before a delete, and the run it would start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardDeleteReviewPreview {
    pub heading: String,
    pub lines: Vec<String>,
    /// A target and at least one row ticked to delete.
    pub can_delete: bool,
    /// The run's input, copied out of the list (see
    /// [`backup_delete_items`]).
    pub items: Vec<DeleteItem>,
    /// What the delete frees (see [`backup_delete_bytes`]).
    pub bytes_freed: u64,
}

/// Review's text for deleting what is ticked for Delete in `list`:
/// 1. the target folder;
/// 2. "These backups go for good and free 520 MB:", then one line per
///    ticked backup saying what is lost, and one line for each group of
///    backups from one set, which go together, with what made the set and
///    when;
/// 3. the notes: `other_tab_notes` (what is ticked on the other tabs), and
///    the backups ticked for Restore, which this run leaves out;
/// 4. the blocker "Can't delete yet" when nothing is ticked.
pub fn build_backup_delete_review(
    localizer: &Localizer,
    target: Option<&TargetRow>,
    list: &WizardBackupList,
    other_tab_notes: &[String],
) -> WizardDeleteReviewPreview {
    let heading = text(localizer, "wizard-review-delete-heading");
    let items = backup_delete_items(list);
    let bytes_freed = backup_delete_bytes(list);
    let Some(target) = target else {
        return WizardDeleteReviewPreview {
            heading,
            lines: vec![text(localizer, "wizard-review-no-target")],
            can_delete: false,
            items,
            bytes_freed,
        };
    };
    let ticked: Vec<(usize, &BackupRow)> = list
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.can_tick(BackupChoice::Delete) && row.delete_ticked)
        .collect();
    let mut lines = vec![say(
        localizer,
        "wizard-review-target",
        &[("path", (&target.path.display().to_string()).into())],
    )];
    if !ticked.is_empty() {
        let size = backup_size_text(localizer, bytes_freed);
        lines.push(String::new());
        lines.push(say(
            localizer,
            "wizard-review-delete-intro",
            &[("count", ticked.len().into()), ("size", (&size).into())],
        ));
        for (_, row) in &ticked {
            lines.push(delete_line(localizer, row));
        }
        // One line for each set whose rows go together, after the rows'
        // own lines: the rows of one set can sit in different groups.
        let lists: Vec<&[String]> = list.rows.iter().map(|row| row.set_ids.as_slice()).collect();
        let mut said: BTreeSet<usize> = BTreeSet::new();
        for (index, _) in &ticked {
            if said.contains(index) {
                continue;
            }
            let mut together = vec![*index];
            together.extend(tied_indices(&lists, *index));
            together.sort_unstable();
            said.extend(together.iter().copied());
            if together.len() < 2 {
                continue;
            }
            let names: Vec<String> = together
                .iter()
                .map(|&other| list.rows[other].name.clone())
                .collect();
            let backups = and_list(localizer, &names);
            // The rows of one set share its date; the first row says what
            // made the set.
            let first = &list.rows[together[0]];
            let origin = first
                .entry
                .as_ref()
                .map_or("other", |entry| origin_of(entry.set_kind));
            let mut args: Vec<(&str, FluentArgument<'_>)> = vec![
                ("backups", (&backups).into()),
                ("count", names.len().into()),
                ("origin", origin.into()),
            ];
            lines.push(match first.date.as_deref() {
                Some(date) => {
                    args.push(("date", date.into()));
                    say(localizer, "wizard-review-delete-together", &args)
                }
                None => say(localizer, "wizard-review-delete-together-undated", &args),
            });
        }
    }

    let mut notes: Vec<String> = Vec::new();
    for note in other_tab_notes {
        if !notes.contains(note) {
            notes.push(note.clone());
        }
    }
    let ticked_to_restore = list.ticked_rows(BackupChoice::Restore).count();
    if ticked_to_restore > 0 {
        notes.push(say(
            localizer,
            "wizard-review-delete-other-choice-note",
            &[("count", ticked_to_restore.into())],
        ));
    }
    if !notes.is_empty() {
        lines.push(String::new());
        lines.push(text(localizer, "wizard-review-notes-heading"));
        lines.extend(notes);
    }
    if ticked.is_empty() {
        lines.push(String::new());
        lines.push(say(
            localizer,
            "wizard-review-delete-blocked",
            &[(
                "reason",
                (&text(localizer, "wizard-review-delete-nothing-ticked")).into(),
            )],
        ));
    }
    WizardDeleteReviewPreview {
        heading,
        lines,
        can_delete: !ticked.is_empty(),
        items,
        bytes_freed,
    }
}

/// The title and the text of the Yes/No question asked before a delete,
/// whose No is the focused button: how many backups go, what that frees,
/// how many of them are the only copy of what they hold, and that a deleted
/// backup can't be brought back.
pub fn delete_backups_question(localizer: &Localizer, list: &WizardBackupList) -> (String, String) {
    let ticked: Vec<&BackupRow> = list.ticked_rows(BackupChoice::Delete).collect();
    let only = ticked
        .iter()
        .filter(|row| row.entry.as_ref().is_some_and(|entry| entry.only_copy))
        .count();
    let size = backup_size_text(localizer, backup_delete_bytes(list));
    let mut body = vec![say(
        localizer,
        "wizard-confirm-delete-backups-body",
        &[("count", ticked.len().into()), ("size", (&size).into())],
    )];
    if only > 0 {
        body.push(if ticked.len() == 1 {
            text(localizer, "wizard-confirm-delete-backups-only-copy-single")
        } else {
            say(
                localizer,
                "wizard-confirm-delete-backups-only-copy",
                &[("only", only.into())],
            )
        });
    }
    body.push(text(localizer, "wizard-confirm-delete-backups-question"));
    (
        text(localizer, "wizard-confirm-delete-backups-title"),
        body.join(" "),
    )
}

/// A restore refusal as a sentence in the user's language, about the
/// backup it refuses, which the sentence calls "it". Review puts it after
/// the backup's name, and a row's details after what the backup is.
pub fn restore_refusal_text(localizer: &Localizer, refusal: &RestoreRefusal) -> String {
    let key = format!("wizard-restore-refusal-{}", refusal.code());
    let path = |path: &Path| path.display().to_string();
    let name = |id: &str| localized_package_display_name(localizer, id);
    let names = |ids: &[String]| {
        let names: Vec<String> = ids.iter().map(|id| name(id)).collect();
        and_list(localizer, &names)
    };
    match refusal {
        RestoreRefusal::InvalidId { .. }
        | RestoreRefusal::NotFound { .. }
        | RestoreRefusal::DiffersInCase { .. }
        | RestoreRefusal::EndsInDotOrSpace { .. }
        | RestoreRefusal::BeingDeleted { .. }
        | RestoreRefusal::NoKeymapInSet { .. }
        | RestoreRefusal::RecordsOnly { .. }
        | RestoreRefusal::ReceiptInSet { .. }
        | RestoreRefusal::KeymapNeedsReaperClosed { .. }
        | RestoreRefusal::LockHeld { .. }
        | RestoreRefusal::AsRoot
        | RestoreRefusal::NotYours { .. }
        | RestoreRefusal::ChangedWhileRead { .. }
        | RestoreRefusal::ChangedSinceListed { .. } => text(localizer, &key),
        RestoreRefusal::IsLink { path: link } => {
            say(localizer, &key, &[("path", (&path(link)).into())])
        }
        RestoreRefusal::NoManifest { package_id, .. }
        | RestoreRefusal::NoPackageRecords { package_id, .. }
        | RestoreRefusal::PackageNotInSet { package_id, .. }
        | RestoreRefusal::ReinstalledSince { package_id, .. }
        | RestoreRefusal::VersionUnknown { package_id, .. } => {
            say(localizer, &key, &[("package", (&name(package_id)).into())])
        }
        RestoreRefusal::DamagedManifest { detail, .. }
        | RestoreRefusal::ReceiptUnreadable { detail } => {
            say(localizer, &key, &[("detail", detail.into())])
        }
        RestoreRefusal::NothingOfItInSet { package_ids, .. } if package_ids.is_empty() => text(
            localizer,
            "wizard-restore-refusal-nothing-of-it-in-set-none",
        ),
        RestoreRefusal::NothingOfItInSet { package_ids, .. } => say(
            localizer,
            &key,
            &[("packages", (&names(package_ids)).into())],
        ),
        RestoreRefusal::PartMissing { path: file, .. }
        | RestoreRefusal::NotComplete { path: file, .. } => {
            say(localizer, &key, &[("path", (&path(file)).into())])
        }
        RestoreRefusal::OtherLanguagePackInstalled {
            package_id,
            installed,
            ..
        } => say(
            localizer,
            &key,
            &[
                ("package", (&name(package_id)).into()),
                ("packages", (&names(installed)).into()),
            ],
        ),
        RestoreRefusal::TargetRefused {
            path: file, reason, ..
        } => say(
            localizer,
            &key,
            &[
                ("path", (&path(file)).into()),
                ("reason", (&target_refusal_text(localizer, reason)).into()),
            ],
        ),
        RestoreRefusal::NewerFileRefused {
            package_id,
            path: file,
            reason,
            ..
        }
        | RestoreRefusal::EarlierFileRefused {
            package_id,
            path: file,
            reason,
            ..
        } => say(
            localizer,
            &key,
            &[
                ("package", (&name(package_id)).into()),
                ("path", (&path(file)).into()),
                ("reason", (&target_refusal_text(localizer, reason)).into()),
            ],
        ),
        RestoreRefusal::ReaperRunning {
            every_reaper_for, ..
        } if !every_reaper_for.is_empty() => say(
            localizer,
            "wizard-restore-refusal-reaper-running-every",
            &[("packages", (&names(every_reaper_for)).into())],
        ),
        RestoreRefusal::ReaperRunning { .. } => text(localizer, &key),
        RestoreRefusal::ProgramRunning { programs, .. } => say(
            localizer,
            &key,
            &[("programs", (&programs.join(", ")).into())],
        ),
        RestoreRefusal::NotWritable {
            path: folder,
            detail,
            ..
        } => say(
            localizer,
            &key,
            &[("path", (&path(folder)).into()), ("detail", detail.into())],
        ),
    }
}

/// Why a restore won't put a file back at a place, as the short phrase
/// that ends a refusal's sentence.
pub fn target_refusal_text(localizer: &Localizer, reason: &TargetRefusal) -> String {
    let key = format!("wizard-restore-target-{}", reason.code());
    let path = |path: &Path| path.display().to_string();
    match reason {
        TargetRefusal::RabbitFolder
        | TargetRefusal::LeadsIntoRabbitFolder
        | TargetRefusal::PlaceIsLink
        | TargetRefusal::OutsideAllowedFolders
        | TargetRefusal::NotPlainPath
        | TargetRefusal::TwoFilesForOnePlace
        | TargetRefusal::OutsideWithoutManifest
        | TargetRefusal::CopyOutsideSet
        | TargetRefusal::KeymapAmongVersionFiles => text(localizer, &key),
        TargetRefusal::NameNotAsWritten { name } => say(localizer, &key, &[("name", name.into())]),
        TargetRefusal::ThroughLink { link: folder }
        | TargetRefusal::LeadsOutsideResourceFolder { folder }
        | TargetRefusal::LeadsOutside { folder }
        | TargetRefusal::GuardFolderGone { folder } => {
            say(localizer, &key, &[("path", (&path(folder)).into())])
        }
        TargetRefusal::CouldNotCheck {
            path: place,
            detail,
        } => say(
            localizer,
            &key,
            &[("path", (&path(place)).into()), ("detail", detail.into())],
        ),
    }
}

/// A moment as the user's language writes a date and a time, such as
/// "November 9, 2026, 2:05 PM" or "9. November 2026, 14:05 Uhr", in a time
/// zone `offset_seconds` ahead of UTC. `None` for a moment too far from 1970
/// to name, which reads as an unknown date.
pub fn backup_date_text(
    localizer: &Localizer,
    unix_seconds: u64,
    offset_seconds: i32,
) -> Option<String> {
    let time = rabbit_platform::civil_date_time(i64::try_from(unix_seconds).ok()?, offset_seconds)?;
    let hour12 = match time.hour % 12 {
        0 => 12,
        hour => hour,
    };
    let period = if time.hour < 12 { "am" } else { "pm" };
    let minute = format!("{:02}", time.minute);
    Some(say(
        localizer,
        "backup-date-time",
        &[
            ("year", time.year.into()),
            ("month", time.month.into()),
            ("day", time.day.into()),
            ("hour", time.hour.into()),
            ("minute", (&minute).into()),
            ("hour12", hour12.into()),
            ("period", period.into()),
        ],
    ))
}

/// How many seconds local time was ahead of UTC at `unix_seconds`, by this
/// computer's time zone and its daylight saving rules then; 0, so UTC, when
/// that can't be read.
pub fn local_utc_offset_at(unix_seconds: u64) -> i32 {
    UNIX_EPOCH
        .checked_add(Duration::from_secs(unix_seconds))
        .and_then(|at| rabbit_platform::local_utc_offset_seconds(at).ok())
        .unwrap_or(0)
}

/// A size as the user's language writes it: "under 1 KB", whole KB and MB,
/// and GB with one decimal, in steps of 1024.
pub fn backup_size_text(localizer: &Localizer, bytes: u64) -> String {
    match BackupSize::of(bytes) {
        BackupSize::UnderOneKb => text(localizer, "size-under-kb"),
        BackupSize::Kb(count) => say(localizer, "size-kb", &[("count", count.into())]),
        BackupSize::Mb(count) => say(localizer, "size-mb", &[("count", count.into())]),
        BackupSize::Gb { whole, tenth } => say(
            localizer,
            "size-gb",
            &[("whole", whole.into()), ("tenth", tenth.into())],
        ),
    }
}

// ---------------------------------------------------------------------------
// Building the rows.
// ---------------------------------------------------------------------------

/// Why a row can't be ticked for one choice: the short phrase its label
/// ends with.
type Unavailable = Option<String>;

/// A backup's row while the list is being built: everything but what
/// depends on the other rows of its set.
struct RowDraft {
    entry: BackupEntry,
    name: String,
    date: Option<String>,
    when: String,
    size: String,
    restore_unavailable: Unavailable,
    delete_unavailable: Unavailable,
    /// The details text's sentences, one per line.
    lines: Vec<String>,
    set_ids: Vec<String>,
    targets: Vec<PathBuf>,
    restore_item: Option<RestoreItem>,
}

impl RowDraft {
    fn new(
        localizer: &Localizer,
        source: &BackupListSource,
        entry: &BackupEntry,
        local_offset: &dyn Fn(u64) -> i32,
    ) -> Self {
        let date = entry
            .created_unix
            .and_then(|unix| backup_date_text(localizer, unix, local_offset(unix)));
        let name = backup_name(localizer, entry);
        let size = backup_size_text(localizer, entry.size);
        let restore_item = restorable(entry)
            .then(|| restore_item_of(entry, &source.sets))
            .flatten();
        let restore_unavailable = restore_item
            .is_none()
            .then(|| restore_reason(localizer, entry));
        let delete_unavailable = match entry.status {
            BackupStatus::NotYours => Some(text(localizer, "wizard-backups-unavailable-not-yours")),
            BackupStatus::NotABackup => {
                Some(text(localizer, "wizard-backups-unavailable-not-a-backup"))
            }
            _ => None,
        };
        let folder = source
            .sets
            .iter()
            .find(|set| {
                Some(set.id.as_str()) == entry.item.set_id()
                    && set.deleting == matches!(entry.item, BackupItem::Deleting { .. })
            })
            .map(|set| set.path.as_path());
        let lines = explanation_lines(
            localizer,
            entry,
            &name,
            date.as_deref(),
            &size,
            restore_item.is_some(),
            folder,
        );
        Self {
            when: when_text(localizer, entry, date.as_deref()),
            set_ids: entry
                .item
                .set_ids()
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            targets: source.targets.get(&entry.item).cloned().unwrap_or_default(),
            entry: entry.clone(),
            name,
            date,
            size,
            restore_unavailable,
            delete_unavailable,
            lines,
            restore_item,
        }
    }

    fn into_row(self, localizer: &Localizer) -> BackupRow {
        let label = |action_key: &str, unavailable: &Unavailable| {
            let action = text(localizer, action_key);
            let row = match &self.entry.item {
                BackupItem::Records { set_ids } => say(
                    localizer,
                    "wizard-backups-row-records",
                    &[
                        ("action", (&action).into()),
                        ("count", set_ids.len().into()),
                        ("size", (&self.size).into()),
                    ],
                ),
                _ => say(
                    localizer,
                    "wizard-backups-row",
                    &[
                        ("backup", (&self.name).into()),
                        ("action", (&action).into()),
                        ("when", (&self.when).into()),
                        ("size", (&self.size).into()),
                    ],
                ),
            };
            match unavailable {
                Some(reason) => {
                    let suffix = say(
                        localizer,
                        "wizard-package-row-unavailable-suffix",
                        &[("reason", reason.into())],
                    );
                    format!("{row} {suffix}")
                }
                None => row,
            }
        };
        let delete_action = if self.entry.only_copy {
            "action-delete-only-copy"
        } else {
            "action-delete"
        };
        let labels = BackupRowLabels {
            keep_for_restore: label("action-keep", &self.restore_unavailable),
            restore: label("action-restore", &self.restore_unavailable),
            keep_for_delete: label("action-keep", &self.delete_unavailable),
            delete: label(delete_action, &self.delete_unavailable),
        };
        let mut row = BackupRow {
            heading: None,
            can_restore: self.restore_unavailable.is_none(),
            can_delete: self.delete_unavailable.is_none(),
            restore_ticked: false,
            delete_ticked: false,
            summary: String::new(),
            details: String::new(),
            labels,
            explanation: self.lines.join("\n"),
            name: self.name,
            date: self.date,
            when: self.when,
            set_ids: self.set_ids,
            targets: self.targets,
            restore_item: self.restore_item,
            entry: Some(self.entry),
        };
        row.relabel(BackupChoice::Restore);
        row
    }
}

/// What depends on the other rows of a row's set: rows of one set go
/// together for Delete, so each says which others it goes with, and when
/// one of them can't be deleted, none of them can.
fn tie_drafts(localizer: &Localizer, drafts: &mut [RowDraft]) {
    let ties: Vec<Vec<usize>> = {
        let lists: Vec<&[String]> = drafts
            .iter()
            .map(|draft| draft.set_ids.as_slice())
            .collect();
        (0..drafts.len())
            .map(|index| tied_indices(&lists, index))
            .collect()
    };
    for (index, tied) in ties.iter().enumerate() {
        if tied.is_empty() {
            continue;
        }
        let names: Vec<String> = tied
            .iter()
            .map(|&other| drafts[other].name.clone())
            .collect();
        let held_back = drafts[index].delete_unavailable.is_none()
            && tied
                .iter()
                .any(|&other| drafts[other].delete_unavailable.is_some());
        let draft = &mut drafts[index];
        let origin = origin_of(draft.entry.set_kind);
        draft.lines.push(say(
            localizer,
            "wizard-backups-details-together",
            &[
                ("backups", (&and_list(localizer, &names)).into()),
                ("count", names.len().into()),
                ("origin", origin.into()),
            ],
        ));
        if held_back {
            draft.delete_unavailable = Some(text(localizer, "wizard-backups-unavailable-tied"));
            draft
                .lines
                .push(text(localizer, "wizard-backups-details-tied"));
        }
    }
}

/// The other rows that go with the row at `index` for Delete, in list
/// order: those whose sets, in `lists`, share an id with it, and so on, as a
/// delete of one takes every set of the others too. Headings, with no set,
/// go with nothing.
fn tied_indices(lists: &[&[String]], index: usize) -> Vec<usize> {
    let Some(own) = lists.get(index) else {
        return Vec::new();
    };
    let mut ids: BTreeSet<&str> = own.iter().map(String::as_str).collect();
    let mut members = BTreeSet::from([index]);
    loop {
        let mut grew = false;
        for (other, list) in lists.iter().enumerate() {
            if members.contains(&other) || !list.iter().any(|id| ids.contains(id.as_str())) {
                continue;
            }
            members.insert(other);
            ids.extend(list.iter().map(String::as_str));
            grew = true;
        }
        if !grew {
            break;
        }
    }
    members.remove(&index);
    members.into_iter().collect()
}

fn heading_row(localizer: &Localizer, group: BackupGroup) -> BackupRow {
    let (name_key, details_key) = match group {
        BackupGroup::UninstalledSoftware => (
            "wizard-backups-group-removed",
            "wizard-backups-group-removed-details",
        ),
        BackupGroup::EarlierVersions => (
            "wizard-backups-group-versions",
            "wizard-backups-group-versions-details",
        ),
        BackupGroup::Keymaps => (
            "wizard-backups-group-keymaps",
            "wizard-backups-group-keymaps-details",
        ),
        BackupGroup::Other => (
            "wizard-backups-group-other",
            "wizard-backups-group-other-details",
        ),
    };
    let name = text(localizer, name_key);
    let explanation = text(localizer, details_key);
    BackupRow {
        heading: Some(group),
        entry: None,
        when: String::new(),
        can_restore: false,
        can_delete: false,
        restore_ticked: false,
        delete_ticked: false,
        summary: name.clone(),
        details: details_text(&name, &explanation),
        labels: BackupRowLabels::all(&name),
        explanation,
        name,
        date: None,
        set_ids: Vec::new(),
        targets: Vec::new(),
        restore_item: None,
    }
}

fn row_group(row: &BackupRow) -> BackupGroup {
    row.heading
        .or_else(|| row.entry.as_ref().map(|entry| entry.group))
        .unwrap_or(BackupGroup::Other)
}

fn is_records(entry: &BackupEntry) -> bool {
    matches!(entry.item, BackupItem::Records { .. })
}

/// A key that sorts the newest first and those without a date last.
fn newest_first(created_unix: Option<u64>) -> (bool, std::cmp::Reverse<u64>) {
    (
        created_unix.is_none(),
        std::cmp::Reverse(created_unix.unwrap_or(0)),
    )
}

/// What made a set, for the sentences that say rows go together.
fn origin_of(kind: BackupSetKind) -> &'static str {
    match kind {
        BackupSetKind::Uninstall => "uninstall",
        BackupSetKind::Update => "update",
        _ => "other",
    }
}

/// Whether the overview found nothing against restoring `entry` through
/// the tab: a package of uninstalled software or an earlier version, or a
/// key map, that a restore takes as such, with a status that can be
/// restored and no refusal.
fn restorable(entry: &BackupEntry) -> bool {
    entry.group != BackupGroup::Other
        && entry.status.can_restore()
        && entry.refusal.is_none()
        && matches!(
            (&entry.item, entry.restore_scope),
            (BackupItem::Package { .. }, Some(RestoreScope::Package))
                | (BackupItem::Keymap { .. }, Some(RestoreScope::Keymap))
        )
}

/// What a restore run is given for `entry`, with its set's fingerprint.
fn restore_item_of(entry: &BackupEntry, sets: &[BackupSetFacts]) -> Option<RestoreItem> {
    let mut item = match &entry.item {
        BackupItem::Package { set_id, package_id } => RestoreItem::package(set_id, package_id),
        BackupItem::Keymap { set_id } => RestoreItem::keymap(set_id),
        _ => return None,
    };
    item.fingerprint = set_fingerprint(sets, &item.set_id);
    Some(item)
}

/// The fingerprint a run compares for the set `id`: its folder's in
/// `RABBIT/backups`, or without one its unfinished deletion's.
fn set_fingerprint(sets: &[BackupSetFacts], id: &str) -> Option<SetFingerprint> {
    sets.iter()
        .filter(|set| set.id == id)
        .min_by_key(|set| set.deleting)
        .and_then(|set| set.fingerprint.clone())
}

/// The localized name of the package `entry` is of, if it is one's.
fn package_name(localizer: &Localizer, entry: &BackupEntry) -> Option<String> {
    entry
        .item
        .package_id()
        .map(|package_id| localized_package_display_name(localizer, package_id))
}

/// What the row calls the backup.
fn backup_name(localizer: &Localizer, entry: &BackupEntry) -> String {
    let key = match &entry.item {
        BackupItem::Package { package_id, .. } => {
            let package = localized_package_display_name(localizer, package_id);
            return match &entry.version {
                Some(version) => say(
                    localizer,
                    "wizard-backups-name-package",
                    &[
                        ("package", (&package).into()),
                        ("version", (&version.to_string()).into()),
                    ],
                ),
                None => package,
            };
        }
        BackupItem::Keymap { .. } => match entry.keymap_origin {
            Some(KeymapOrigin::OwnKeymap) => "wizard-backups-name-keymap-own",
            Some(KeymapOrigin::DefaultKeys) => "wizard-backups-name-keymap-default",
            Some(KeymapOrigin::UnchangedOsara) => "wizard-backups-name-keymap-osara",
            Some(KeymapOrigin::Unknown) | None => "wizard-backups-name-keymap",
        },
        BackupItem::Records { .. } => "wizard-backups-name-records",
        BackupItem::Deleting { .. } => "wizard-backups-name-deleting",
        BackupItem::Set { .. } if entry.status == BackupStatus::NotABackup => {
            "wizard-backups-name-not-a-backup"
        }
        BackupItem::Set { .. } => match entry.set_kind {
            BackupSetKind::NoManifest => "wizard-backups-name-no-manifest",
            BackupSetKind::Damaged => "wizard-backups-name-damaged",
            BackupSetKind::BeforeRestore => "wizard-backups-name-before-restore",
            BackupSetKind::Update => "wizard-backups-name-update-files",
            BackupSetKind::Uninstall => "wizard-backups-name-uninstall-files",
            BackupSetKind::Keymap => "wizard-backups-name-keymap-files",
            BackupSetKind::Unknown => "wizard-backups-name-unknown",
            BackupSetKind::Records => "wizard-backups-name-records-copy",
            BackupSetKind::Deleting => "wizard-backups-name-deleting",
            BackupSetKind::NotABackup => "wizard-backups-name-not-a-backup",
        },
    };
    text(localizer, key)
}

/// When the backup was made, as the row says it, by what made it.
fn when_text(localizer: &Localizer, entry: &BackupEntry, date: Option<&str>) -> String {
    let Some(date) = date else {
        return text(localizer, "backup-date-unknown");
    };
    let key = match (entry.group, &entry.replaced_by, entry.set_kind) {
        (BackupGroup::UninstalledSoftware, ..) => "wizard-backups-when-uninstalled",
        (BackupGroup::EarlierVersions, Some(version), _) => {
            return say(
                localizer,
                "wizard-backups-when-replaced-by",
                &[
                    ("version", (&version.to_string()).into()),
                    ("date", date.into()),
                ],
            );
        }
        (BackupGroup::EarlierVersions, None, _) => "wizard-backups-when-replaced",
        (BackupGroup::Keymaps, _, BackupSetKind::Keymap) => "wizard-backups-when-keymap-osara",
        (BackupGroup::Keymaps, _, BackupSetKind::BeforeRestore) => {
            "wizard-backups-when-keymap-restore"
        }
        _ => "wizard-backups-when-made",
    };
    say(localizer, key, &[("date", date.into())])
}

/// Why `entry` can't be ticked for Restore, as the short phrase its label
/// ends with: what its status says, or for a status that could be restored,
/// what the overview found against it.
fn restore_reason(localizer: &Localizer, entry: &BackupEntry) -> String {
    let key = match entry.status {
        _ if entry.group == BackupGroup::Other && entry.status.can_restore() => {
            "wizard-backups-unavailable-not-here"
        }
        BackupStatus::NotHere => "wizard-backups-unavailable-not-here",
        BackupStatus::NotABackup => "wizard-backups-unavailable-not-a-backup",
        BackupStatus::NotYours => "wizard-backups-unavailable-not-yours",
        BackupStatus::InstalledAgain => "wizard-backups-unavailable-installed-again",
        BackupStatus::NotComplete => "wizard-backups-unavailable-not-complete",
        BackupStatus::NothingInIt => "wizard-backups-unavailable-nothing-in-it",
        BackupStatus::VersionUnknown => "wizard-backups-unavailable-version-unknown",
        BackupStatus::OtherLanguagePack => "wizard-backups-unavailable-other-language-pack",
        BackupStatus::PartMissing => "wizard-backups-unavailable-part-missing",
        BackupStatus::RecordsUnreadable => "wizard-backups-unavailable-records-unreadable",
        BackupStatus::CanRestore | BackupStatus::StoppedPartway | BackupStatus::AlreadyBack => {
            match &entry.refusal {
                Some(RestoreRefusal::ReceiptInSet { .. }) => {
                    "wizard-backups-unavailable-receipt-in-set"
                }
                Some(
                    RestoreRefusal::TargetRefused { .. }
                    | RestoreRefusal::NewerFileRefused { .. }
                    | RestoreRefusal::EarlierFileRefused { .. },
                ) => "wizard-backups-unavailable-file-refused",
                _ => "wizard-backups-unavailable-cannot-restore",
            }
        }
    };
    let package = package_name(localizer, entry).unwrap_or_else(|| backup_name(localizer, entry));
    say(localizer, key, &[("package", (&package).into())])
}

/// The details text's sentences, in the order backup plan 3.5 gives: what
/// the backup is; when it was made; what it holds and how big; whether it
/// is the only copy; what restoring does, or why it can't; what deleting
/// loses. For another account's set, then how to remove it in Finder.
fn explanation_lines(
    localizer: &Localizer,
    entry: &BackupEntry,
    name: &str,
    date: Option<&str>,
    size: &str,
    can_restore: bool,
    folder: Option<&Path>,
) -> Vec<String> {
    let package = package_name(localizer, entry).unwrap_or_else(|| name.to_string());
    let version = entry.version.as_ref().map(ToString::to_string);
    let sentence = |key: &str| {
        say(
            localizer,
            key,
            &[
                ("backup", name.into()),
                ("package", (&package).into()),
                ("size", size.into()),
                ("version", version.as_deref().unwrap_or_default().into()),
            ],
        )
    };
    let mut lines = vec![sentence(what_key(entry))];
    lines.push(when_sentence(localizer, entry, date));
    if let Some(key) = holds_key(entry) {
        lines.push(match &entry.item {
            BackupItem::Records { set_ids } => say(
                localizer,
                key,
                &[("count", set_ids.len().into()), ("size", size.into())],
            ),
            _ => sentence(key),
        });
    }
    if entry.only_copy {
        lines.push(text(localizer, "wizard-backups-details-only-copy"));
    }
    // Other backups, another account's and what is no backup say why by
    // what they are; any other backup that can't come back says what the
    // overview found against it, in the words a run would use.
    let by_status = can_restore
        || entry.group == BackupGroup::Other
        || matches!(
            entry.status,
            BackupStatus::NotYours | BackupStatus::NotABackup
        );
    lines.push(if by_status {
        sentence(status_key(entry))
    } else if let Some(refusal) = &entry.refusal {
        restore_refusal_text(localizer, refusal)
    } else if entry.status.can_restore() {
        text(localizer, "wizard-backups-details-cannot-restore")
    } else {
        sentence(status_key(entry))
    });
    if let Some(key) = delete_key(entry) {
        lines.push(sentence(key));
    }
    // The steps go through Finder's Go to Folder, as the set is in the
    // hidden Library folder, which Finder doesn't show to select.
    if let (BackupStatus::NotYours, Some(parent), Some(name)) = (
        entry.status,
        folder.and_then(Path::parent),
        folder.and_then(Path::file_name),
    ) {
        lines.push(say(
            localizer,
            "wizard-backups-details-not-yours-finder-macos",
            &[
                ("folder", (&parent.display().to_string()).into()),
                ("name", (&name.to_string_lossy().into_owned()).into()),
            ],
        ));
    }
    lines
}

/// The sentence on what the backup is.
fn what_key(entry: &BackupEntry) -> &'static str {
    match (&entry.item, entry.group) {
        (BackupItem::Package { .. }, BackupGroup::UninstalledSoftware) => {
            "wizard-backups-details-uninstalled"
        }
        (BackupItem::Package { .. }, _) => "wizard-backups-details-earlier",
        (BackupItem::Keymap { .. }, _) => match (entry.set_kind, entry.keymap_origin) {
            (BackupSetKind::BeforeRestore, _) => "wizard-backups-details-keymap-restore",
            (BackupSetKind::Keymap, Some(KeymapOrigin::OwnKeymap)) => {
                "wizard-backups-details-keymap-own"
            }
            (BackupSetKind::Keymap, Some(KeymapOrigin::DefaultKeys)) => {
                "wizard-backups-details-keymap-default"
            }
            (BackupSetKind::Keymap, Some(KeymapOrigin::UnchangedOsara)) => {
                "wizard-backups-details-keymap-osara"
            }
            (BackupSetKind::Keymap, _) => "wizard-backups-details-keymap-unknown",
            _ => "wizard-backups-details-keymap-other",
        },
        (BackupItem::Records { .. }, _) => "wizard-backups-details-records",
        (BackupItem::Deleting { .. }, _) => "wizard-backups-details-deleting",
        (BackupItem::Set { .. }, _) if entry.status == BackupStatus::NotABackup => {
            "wizard-backups-details-not-a-backup"
        }
        (BackupItem::Set { .. }, _) => match entry.set_kind {
            BackupSetKind::NoManifest => "wizard-backups-details-no-manifest",
            BackupSetKind::Damaged => "wizard-backups-details-damaged",
            BackupSetKind::BeforeRestore => "wizard-backups-details-before-restore",
            BackupSetKind::Update => "wizard-backups-details-update-files",
            BackupSetKind::Uninstall => "wizard-backups-details-uninstall-files",
            BackupSetKind::Keymap => "wizard-backups-details-keymap-files",
            BackupSetKind::Unknown => "wizard-backups-details-unknown",
            BackupSetKind::Records => "wizard-backups-details-records-copy",
            BackupSetKind::Deleting => "wizard-backups-details-deleting",
            BackupSetKind::NotABackup => "wizard-backups-details-not-a-backup",
        },
    }
}

/// The sentence on when the backup was made, by what made it.
fn when_sentence(localizer: &Localizer, entry: &BackupEntry, date: Option<&str>) -> String {
    let Some(date) = date else {
        return text(localizer, "wizard-backups-details-when-unknown");
    };
    let replaced_by = entry.replaced_by.as_ref().map(Version::to_string);
    let key = match (&entry.item, entry.group, entry.set_kind, &replaced_by) {
        (BackupItem::Records { .. }, ..) => "wizard-backups-details-when-records",
        (_, BackupGroup::UninstalledSoftware, ..) => "wizard-backups-details-when-uninstalled",
        (_, BackupGroup::EarlierVersions, BackupSetKind::BeforeRestore, Some(_)) => {
            "wizard-backups-details-when-restored-to"
        }
        (_, BackupGroup::EarlierVersions, BackupSetKind::BeforeRestore, None) => {
            "wizard-backups-details-when-restored"
        }
        (_, BackupGroup::EarlierVersions, _, Some(_)) => "wizard-backups-details-when-updated-to",
        (_, BackupGroup::EarlierVersions, _, None) => "wizard-backups-details-when-updated",
        (_, BackupGroup::Keymaps, BackupSetKind::Keymap, _) => "wizard-backups-details-when-osara",
        (_, BackupGroup::Keymaps, BackupSetKind::BeforeRestore, _) => {
            "wizard-backups-details-when-keymap-restored"
        }
        _ => "wizard-backups-details-when-made",
    };
    say(
        localizer,
        key,
        &[
            ("date", date.into()),
            ("version", replaced_by.as_deref().unwrap_or_default().into()),
        ],
    )
}

/// The sentence on what the backup holds and how big it is. None for what
/// is no backup, which RABBIT never looks into.
fn holds_key(entry: &BackupEntry) -> Option<&'static str> {
    Some(match (&entry.item, entry.group) {
        (BackupItem::Set { .. }, _) if entry.status == BackupStatus::NotABackup => return None,
        (BackupItem::Package { .. }, BackupGroup::UninstalledSoftware) => match entry.with_settings
        {
            Some(true) => "wizard-backups-details-holds-with-settings",
            Some(false) => "wizard-backups-details-holds-files",
            None => "wizard-backups-details-holds-older",
        },
        (BackupItem::Package { .. }, _) => "wizard-backups-details-holds-replaced",
        (BackupItem::Keymap { .. }, _) => "wizard-backups-details-holds-keymap",
        (BackupItem::Records { .. }, _) => "wizard-backups-details-holds-records",
        _ => "wizard-backups-details-holds-size",
    })
}

/// The sentence on what restoring does, for a backup that can be restored,
/// or on why it can't, by its status alone. A refusal the overview found
/// is said in its own words instead (see [`restore_refusal_text`]), so the
/// `wizard-backups-details-status-*` sentences of a backup in a group a
/// restore can take are said only when the overview found no refusal.
///
/// A sentence says how RABBIT's records change only where the overview
/// shows it: a version means the uninstall recorded the package's entry,
/// which a restore puts back. Without a version the entry may be missing,
/// and for an earlier version Review's quick plan says what happens to it.
fn status_key(entry: &BackupEntry) -> &'static str {
    match entry.status {
        _ if entry.group == BackupGroup::Other && entry.status.can_restore() => {
            "wizard-backups-details-not-here"
        }
        BackupStatus::CanRestore => match (entry.group, &entry.version, entry.keymap_origin) {
            (BackupGroup::UninstalledSoftware, Some(_), _) => {
                "wizard-backups-details-restore-uninstalled"
            }
            (BackupGroup::UninstalledSoftware, None, _) => {
                "wizard-backups-details-restore-uninstalled-files"
            }
            (BackupGroup::EarlierVersions, Some(_), _) => "wizard-backups-details-restore-earlier",
            (BackupGroup::EarlierVersions, None, _) => {
                "wizard-backups-details-restore-earlier-copy"
            }
            (BackupGroup::Keymaps, _, Some(KeymapOrigin::DefaultKeys)) => {
                "wizard-backups-details-restore-default-keys"
            }
            (BackupGroup::Keymaps, ..) => "wizard-backups-details-restore-keymap",
            (BackupGroup::Other, ..) => "wizard-backups-details-not-here",
        },
        BackupStatus::StoppedPartway => "wizard-backups-details-restore-stopped-partway",
        BackupStatus::AlreadyBack if entry.group == BackupGroup::EarlierVersions => {
            "wizard-backups-details-restore-earlier-already-back"
        }
        BackupStatus::AlreadyBack => "wizard-backups-details-restore-already-back",
        BackupStatus::NotHere => "wizard-backups-details-not-here",
        BackupStatus::NotABackup => "wizard-backups-details-status-not-a-backup",
        BackupStatus::NotYours => "wizard-backups-details-status-not-yours",
        BackupStatus::InstalledAgain => "wizard-backups-details-status-installed-again",
        BackupStatus::NotComplete => "wizard-backups-details-status-not-complete",
        BackupStatus::NothingInIt => "wizard-backups-details-status-nothing-in-it",
        BackupStatus::VersionUnknown => "wizard-backups-details-status-version-unknown",
        BackupStatus::OtherLanguagePack => "wizard-backups-details-status-other-language-pack",
        BackupStatus::PartMissing => "wizard-backups-details-status-part-missing",
        BackupStatus::RecordsUnreadable => "wizard-backups-details-status-records-unreadable",
    }
}

/// The sentence on what deleting the backup loses. None for what can't be
/// deleted, and for the records, whose first sentence says it.
fn delete_key(entry: &BackupEntry) -> Option<&'static str> {
    Some(match (&entry.item, entry.status) {
        (_, BackupStatus::NotYours | BackupStatus::NotABackup) => return None,
        (BackupItem::Records { .. }, _) => return None,
        (_, BackupStatus::AlreadyBack) => "wizard-backups-details-delete-already-back",
        (BackupItem::Package { .. }, _) => match (entry.group, &entry.version) {
            (BackupGroup::UninstalledSoftware, _) => "wizard-backups-details-delete-uninstalled",
            (_, Some(_)) => "wizard-backups-details-delete-earlier",
            (_, None) => "wizard-backups-details-delete-earlier-copy",
        },
        (BackupItem::Keymap { .. }, _) => "wizard-backups-details-delete-keymap",
        (BackupItem::Deleting { .. }, _) => "wizard-backups-details-delete-deleting",
        (BackupItem::Set { .. }, _) => "wizard-backups-details-delete-other",
    })
}

// ---------------------------------------------------------------------------
// Review.
// ---------------------------------------------------------------------------

/// The quick plan's report of the backup `item` asks for.
fn report_item<'a>(
    report: &'a RestoreRunReport,
    item: &RestoreItem,
) -> Option<&'a RestoreReportItem> {
    let keymap = matches!(item.part, RestorePart::Keymap);
    let same = |backup: &&RestoreReportItem| {
        backup.backup_id == item.set_id && backup.package_id.as_deref() == item.package_id()
    };
    report
        .backups
        .iter()
        .find(|backup| same(backup) && backup.keymap_only == keymap)
        .or_else(|| report.backups.iter().find(same))
}

/// Review's sentence for one ticked backup: what comes back, and what
/// RABBIT keeps as a backup. An earlier version names the versions from the
/// quick plan when it has them.
fn restore_line(
    localizer: &Localizer,
    row: &BackupRow,
    planned: Option<&RestoreReportItem>,
) -> String {
    let Some(entry) = &row.entry else {
        return row.summary.clone();
    };
    let date = row.date.as_deref();
    let package = package_name(localizer, entry).unwrap_or_else(|| row.name.clone());
    match &entry.item {
        BackupItem::Keymap { .. } => {
            let origin = match entry.keymap_origin {
                Some(KeymapOrigin::OwnKeymap) => "own",
                Some(KeymapOrigin::DefaultKeys) => "default-keys",
                _ => "other",
            };
            match date {
                Some(date) => say(
                    localizer,
                    "wizard-review-restore-keymap",
                    &[("origin", origin.into()), ("date", date.into())],
                ),
                None => say(
                    localizer,
                    "wizard-review-restore-keymap-undated",
                    &[("origin", origin.into())],
                ),
            }
        }
        _ if entry.group == BackupGroup::EarlierVersions => {
            let swap = planned.and_then(|item| item.swap.as_ref());
            let earlier = swap
                .map(|swap| swap.version.clone())
                .unwrap_or_else(|| entry.version.clone());
            let current = swap.and_then(|swap| swap.replaced_version.clone());
            match (earlier, current) {
                (Some(earlier), Some(current)) => say(
                    localizer,
                    "wizard-review-restore-earlier",
                    &[
                        ("package", (&package).into()),
                        ("version", (&earlier.to_string()).into()),
                        ("current", (&current.to_string()).into()),
                    ],
                ),
                (Some(earlier), None) => say(
                    localizer,
                    "wizard-review-restore-earlier-to",
                    &[
                        ("package", (&package).into()),
                        ("version", (&earlier.to_string()).into()),
                    ],
                ),
                (None, _) => say(
                    localizer,
                    "wizard-review-restore-earlier-copy",
                    &[("package", (&package).into())],
                ),
            }
        }
        _ => {
            let settings = if entry.with_settings == Some(true) {
                "yes"
            } else {
                "no"
            };
            let mut args: Vec<(&str, FluentArgument<'_>)> = vec![
                ("backup", (&row.name).into()),
                ("package", (&package).into()),
                ("settings", settings.into()),
            ];
            match date {
                Some(date) => {
                    args.push(("date", date.into()));
                    say(localizer, "wizard-review-restore-uninstalled", &args)
                }
                None => say(
                    localizer,
                    "wizard-review-restore-uninstalled-undated",
                    &args,
                ),
            }
        }
    }
}

/// What the quick plan says the restore of one backup changes besides its
/// files, and the core reports: REAPER's language going back or staying,
/// and a package RABBIT no longer counts as installed by it after going
/// back to a copy it didn't install.
///
/// The quick plan checks each backup against REAPER's settings as they are
/// now, so every backup that recorded a language says it goes back. The run
/// puts back only the first one: `language_set` says whether an earlier
/// backup in list order did, and then the later ones add no note, as the
/// first note already names the language REAPER ends up with.
fn restore_notes(
    localizer: &Localizer,
    item: &RestoreReportItem,
    language_set: &mut bool,
) -> Vec<String> {
    let mut notes = Vec::new();
    for package in &item.packages {
        let Some(file) = &package.langpack_before else {
            continue;
        };
        if *language_set {
            continue;
        }
        notes.push(if package.langpack_restored {
            *language_set = true;
            say(
                localizer,
                "wizard-review-restore-language-back",
                &[("file", file.into())],
            )
        } else {
            text(localizer, "wizard-review-restore-language-stays")
        });
    }
    let swaps = item
        .swap
        .iter()
        .chain(item.swaps.iter().filter_map(|swap| swap.swap.as_ref()));
    for swap in swaps {
        if swap.entry != SwappedEntry::Removed {
            continue;
        }
        let package = localized_package_display_name(localizer, &swap.package_id);
        notes.push(say(
            localizer,
            "wizard-review-restore-not-counted",
            &[("package", (&package).into())],
        ));
        if swap.uninstall_needs_reinstall {
            notes.push(say(
                localizer,
                "wizard-review-restore-needs-reinstall",
                &[("package", (&package).into())],
            ));
        }
    }
    notes
}

/// Why a restore of the ticked rows can't start, each line starting "Can't
/// restore yet", or "Can't restore" with the backup's name for what the
/// quick plan refused of it. `items` is what the run would be given for
/// `ticked`, which the quick plan must have checked, no more and no less.
fn restore_blockers<'a>(
    localizer: &Localizer,
    target: &TargetRow,
    ticked: &[&'a BackupRow],
    items: &[RestoreItem],
    quick_plan: Option<&RestoreQuickPlan>,
    planned: &dyn Fn(&BackupRow) -> Option<&'a RestoreReportItem>,
) -> Vec<String> {
    let blocked = |reason: String| {
        say(
            localizer,
            "wizard-review-restore-blocked",
            &[("reason", (&reason).into())],
        )
    };
    let reference = |row: &BackupRow| {
        say(
            localizer,
            "wizard-backups-row-reference",
            &[("backup", (&row.name).into()), ("when", (&row.when).into())],
        )
    };
    let mut blockers = Vec::new();
    if ticked.is_empty() {
        blockers.push(blocked(text(
            localizer,
            "wizard-review-restore-nothing-ticked",
        )));
    }

    // Two backups of one package: each would make the package's record its
    // own. Each later one is named with the first.
    for second in 1..ticked.len() {
        let Some(package_id) = package_of(ticked[second]) else {
            continue;
        };
        let Some(first) = (0..second).find(|&first| package_of(ticked[first]) == Some(package_id))
        else {
            continue;
        };
        blockers.push(blocked(say(
            localizer,
            "wizard-review-restore-same-package",
            &[
                ("first", (&reference(ticked[first])).into()),
                ("second", (&reference(ticked[second])).into()),
                (
                    "package",
                    (&localized_package_display_name(localizer, package_id)).into(),
                ),
            ],
        )));
    }

    // Two backups that write the same file: the second would put back its
    // own copy over the first one's. Two backups of one package are named
    // above already. Paths are compared as a restore compares them, by
    // their components, so `/` and `\`, case on Windows and macOS, and a
    // `\\?\` prefix don't hide that two are one file.
    let places = |row: &BackupRow| -> BTreeSet<Vec<String>> {
        row.targets.iter().map(|path| same_place(path)).collect()
    };
    for first in 0..ticked.len() {
        let mine = places(ticked[first]);
        for second in first + 1..ticked.len() {
            let one_package = package_of(ticked[first]).is_some()
                && package_of(ticked[first]) == package_of(ticked[second]);
            if one_package || mine.is_disjoint(&places(ticked[second])) {
                continue;
            }
            blockers.push(blocked(say(
                localizer,
                "wizard-review-restore-same-file",
                &[
                    ("first", (&reference(ticked[first])).into()),
                    ("second", (&reference(ticked[second])).into()),
                ],
            )));
        }
    }

    if !target.writable {
        blockers.push(blocked(text(
            localizer,
            "wizard-review-restore-not-writable",
        )));
    }

    match quick_plan {
        Some(RestoreQuickPlan::Refused(refusal)) => {
            blockers.push(blocked(restore_refusal_text(localizer, refusal)));
        }
        Some(RestoreQuickPlan::Failed(error)) => blockers.push(blocked(say(
            localizer,
            "wizard-review-restore-check-failed",
            &[("error", error.into())],
        ))),
        // Restore is never enabled for backups the quick plan didn't check:
        // before it ran, or when it checked other ticks than these.
        Some(RestoreQuickPlan::Planned(report)) if !ticked.is_empty() => {
            if !plan_covers(report, items) {
                blockers.push(blocked(text(
                    localizer,
                    "wizard-review-restore-not-checked",
                )));
            }
        }
        None if !ticked.is_empty() => {
            blockers.push(blocked(text(
                localizer,
                "wizard-review-restore-not-checked",
            )));
        }
        Some(RestoreQuickPlan::Planned(_)) | None => {}
    }
    for row in ticked {
        let Some(item) = planned(row) else {
            continue;
        };
        let reason = match (&item.refusal, &item.error) {
            (Some(refusal), _) => restore_refusal_text(localizer, refusal),
            (None, Some(error)) => say(
                localizer,
                "wizard-review-restore-plan-failed",
                &[("error", error.into())],
            ),
            (None, None) => continue,
        };
        blockers.push(say(
            localizer,
            "wizard-review-restore-blocked-backup",
            &[
                ("backup", (&reference(row)).into()),
                ("reason", (&reason).into()),
            ],
        ));
    }
    blockers
}

/// Whether `report` is of exactly the backups in `items`, each once: the
/// same set, package and part, in any order.
fn plan_covers(report: &RestoreRunReport, items: &[RestoreItem]) -> bool {
    let mut planned: Vec<(&str, Option<&str>, bool)> = report
        .backups
        .iter()
        .map(|backup| {
            (
                backup.backup_id.as_str(),
                backup.package_id.as_deref(),
                backup.keymap_only,
            )
        })
        .collect();
    let mut asked: Vec<(&str, Option<&str>, bool)> = items
        .iter()
        .map(|item| {
            (
                item.set_id.as_str(),
                item.package_id(),
                matches!(item.part, RestorePart::Keymap),
            )
        })
        .collect();
    planned.sort_unstable();
    asked.sort_unstable();
    planned == asked
}

/// The package a row's backup is of, if it is one package's.
fn package_of(row: &BackupRow) -> Option<&str> {
    row.entry.as_ref().and_then(|entry| entry.item.package_id())
}

/// Review's line for one backup ticked to delete: what is lost with it.
fn delete_line(localizer: &Localizer, row: &BackupRow) -> String {
    let Some(entry) = &row.entry else {
        return row.name.clone();
    };
    let date = row.date.as_deref();
    let backup = |key: &str| say(localizer, key, &[("backup", (&row.name).into())]);
    let dated = |key: &str, date: &str| {
        say(
            localizer,
            key,
            &[("backup", (&row.name).into()), ("date", date.into())],
        )
    };
    match (&entry.item, entry.status) {
        (_, BackupStatus::AlreadyBack) => backup("wizard-review-delete-already-back"),
        (BackupItem::Records { .. }, _) => backup("wizard-review-delete-records"),
        (BackupItem::Deleting { .. }, _) => backup("wizard-review-delete-deleting"),
        (BackupItem::Package { .. }, _) if entry.group == BackupGroup::EarlierVersions => {
            backup("wizard-review-delete-earlier")
        }
        (BackupItem::Package { .. }, _) if entry.only_copy => match date {
            Some(date) => dated("wizard-review-delete-uninstalled", date),
            None => backup("wizard-review-delete-only-copy"),
        },
        (BackupItem::Keymap { .. }, _) if entry.only_copy => match date {
            Some(date) => dated("wizard-review-delete-keymap", date),
            None => backup("wizard-review-delete-only-copy"),
        },
        _ if entry.only_copy => backup("wizard-review-delete-only-copy"),
        _ => backup("wizard-review-delete-other"),
    }
}

// ---------------------------------------------------------------------------
// Words.
// ---------------------------------------------------------------------------

fn text(localizer: &Localizer, key: &str) -> String {
    localizer.text(key).value
}

fn say(localizer: &Localizer, key: &str, arguments: &[(&str, FluentArgument<'_>)]) -> String {
    localizer.format_with(key, arguments).value
}

/// Names as a sentence lists them in the user's language: "a", "a and b",
/// "a, b and c".
fn and_list(localizer: &Localizer, names: &[String]) -> String {
    match names {
        [] => String::new(),
        [name] => name.clone(),
        [rest @ .., last] => say(
            localizer,
            "wizard-backups-list-and",
            &[("rest", (&rest.join(", ")).into()), ("last", last.into())],
        ),
    }
}
