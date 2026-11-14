//! The Backups tab without wxWidgets: its rows, the choice below the list
//! and its ticks, both Reviews, the question before a delete, and the
//! dates, sizes and refusals they say, in every language.

use std::fs;
use std::path::PathBuf;

use rabbit_core::RabbitError;
use rabbit_core::backup_overview::{
    BackupEntry, BackupGroup, BackupItem, BackupSetKind, BackupStatus, ModifiedTime, RestoreScope,
    SetFingerprint, backup_overview,
};
use rabbit_core::localization::{DEFAULT_LOCALE, Localizer, embedded_locales};
use rabbit_core::preflight::{
    PreflightCheck, PreflightReport, PreflightStatus, REAPER_PROCESS_CHECK,
};
use rabbit_core::receipt::PackageReceipt;
use rabbit_core::restore_refusal::{RestoreRefusal, TargetRefusal, sample_refusals};
use rabbit_core::rollback::{
    BackupPackageRecord, BackupSetWriter, KeymapOrigin, RestorePart, RestoreReportItem,
    RestoreRunReport, RestoreStatus, RestoredPackage, SwappedEntry, SwappedPackage,
};
use rabbit_core::uninstall::ANY_REAPER_PROCESS_CHECK;
use rabbit_core::uninstall::execute::UNINSTALL_BACKUP_REASON;
use rabbit_core::version::Version;
use tempfile::tempdir;

use super::support::*;
use super::uninstall::target;
use crate::wizard::*;

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

/// November 9, 2026, 14:05 UTC.
const NOV_9_1405: u64 = 1_794_233_100;
/// November 3, 2026, 09:12 UTC.
const NOV_3_0912: u64 = 1_793_697_120;
/// November 1, 2026, 18:40 UTC.
const NOV_1_1840: u64 = 1_793_558_400;
/// October 2, 2026, 11:00 UTC.
const OCT_2_1100: u64 = 1_790_938_800;
/// November 2, 2026, 10:00 UTC.
const NOV_2_1000: u64 = 1_793_613_600;

const KB: u64 = 1024;
const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * 1024 * 1024;

fn en() -> Localizer {
    Localizer::embedded(DEFAULT_LOCALE).unwrap()
}

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

/// A backup as the overview gives one, of `item` in `group`, made when
/// `created_unix` says, with nothing against restoring it.
fn backup(item: BackupItem, group: BackupGroup, created_unix: Option<u64>) -> BackupEntry {
    let (set_kind, restore_scope) = match (&item, group) {
        (BackupItem::Package { .. }, BackupGroup::UninstalledSoftware) => {
            (BackupSetKind::Uninstall, Some(RestoreScope::Package))
        }
        (BackupItem::Package { .. }, _) => (BackupSetKind::Update, Some(RestoreScope::Package)),
        (BackupItem::Keymap { .. }, _) => (BackupSetKind::Keymap, Some(RestoreScope::Keymap)),
        (BackupItem::Records { .. }, _) => (BackupSetKind::Records, None),
        (BackupItem::Deleting { .. }, _) => (BackupSetKind::Deleting, None),
        (BackupItem::Set { .. }, _) => (BackupSetKind::NoManifest, Some(RestoreScope::WholeSet)),
    };
    let status = if group == BackupGroup::Other {
        BackupStatus::NotHere
    } else {
        BackupStatus::CanRestore
    };
    BackupEntry {
        item,
        group,
        set_kind,
        created_unix,
        size: 4 * MB,
        version: None,
        replaced_by: None,
        with_settings: None,
        keymap_origin: None,
        status,
        missing_path: None,
        refused_path: None,
        only_copy: matches!(
            group,
            BackupGroup::UninstalledSoftware | BackupGroup::Keymaps
        ),
        shares_set_with: Vec::new(),
        restore_scope,
        refusal: None,
    }
}

fn package(set_id: &str, package_id: &str) -> BackupItem {
    BackupItem::Package {
        set_id: set_id.to_string(),
        package_id: package_id.to_string(),
    }
}

/// SWS 2.14.0.7 as an uninstall on November 9 kept it, with its settings.
fn sws() -> BackupEntry {
    BackupEntry {
        version: Some(version("2.14.0.7")),
        with_settings: Some(true),
        ..backup(
            package("uninstall-unix-1794233100", "sws"),
            BackupGroup::UninstalledSoftware,
            Some(NOV_9_1405),
        )
    }
}

/// FFmpeg 7.1, from the same uninstall as [`sws`].
fn ffmpeg() -> BackupEntry {
    BackupEntry {
        version: Some(version("7.1")),
        with_settings: Some(false),
        size: 250 * MB,
        ..backup(
            package("uninstall-unix-1794233100", "ffmpeg"),
            BackupGroup::UninstalledSoftware,
            Some(NOV_9_1405),
        )
    }
}

/// ReaPack 1.2.4, which an update to 1.2.5 replaced on November 3.
fn reapack() -> BackupEntry {
    BackupEntry {
        version: Some(version("1.2.4")),
        replaced_by: Some(version("1.2.5")),
        size: 2 * MB,
        ..backup(
            package("unix-1793697120", "reapack"),
            BackupGroup::EarlierVersions,
            Some(NOV_3_0912),
        )
    }
}

/// The user's own key map, which OSARA's replaced on November 1.
fn keymap() -> BackupEntry {
    BackupEntry {
        keymap_origin: Some(KeymapOrigin::OwnKeymap),
        size: 38 * KB,
        ..backup(
            BackupItem::Keymap {
                set_id: "unattended-unix-1793558400-000000001".to_string(),
            },
            BackupGroup::Keymaps,
            Some(NOV_1_1840),
        )
    }
}

/// Every old copy of RABBIT's records, 23 sets.
fn records() -> BackupEntry {
    BackupEntry {
        size: 420 * KB,
        status: BackupStatus::NotHere,
        ..backup(
            BackupItem::Records {
                set_ids: (1..=23)
                    .map(|n| format!("unattended-unix-17900000{n:02}-000000001"))
                    .collect(),
            },
            BackupGroup::Other,
            Some(OCT_2_1100),
        )
    }
}

fn fingerprint(length: u64) -> SetFingerprint {
    SetFingerprint::Manifest {
        length,
        modified: Some(ModifiedTime {
            seconds: 1_794_000_000,
            nanos: 0,
        }),
    }
}

/// A source with `entries`, one set for each id they name, each 1 MB on
/// disk and with a fingerprint of its own, and an unfinished deletion for
/// each `Deleting` backup.
fn source(entries: Vec<BackupEntry>) -> BackupListSource {
    let mut sets: Vec<BackupSetFacts> = Vec::new();
    for entry in &entries {
        let deleting = matches!(entry.item, BackupItem::Deleting { .. });
        for id in entry.item.set_ids() {
            if sets
                .iter()
                .any(|set| set.id == id && set.deleting == deleting)
            {
                continue;
            }
            let folder = if deleting {
                "backups-deleting"
            } else {
                "backups"
            };
            sets.push(BackupSetFacts {
                id: id.to_string(),
                path: PathBuf::from("C:/REAPER/RABBIT").join(folder).join(id),
                deleting,
                size: MB,
                fingerprint: Some(fingerprint(
                    sets.len() as u64 + if deleting { 1000 } else { 1 },
                )),
            });
        }
    }
    BackupListSource {
        total_size: sets.iter().map(|set| set.size).sum(),
        entries,
        sets,
        targets: Default::default(),
    }
}

fn utc(_: u64) -> i32 {
    0
}

fn list_in(localizer: &Localizer, entries: Vec<BackupEntry>) -> WizardBackupList {
    backup_list_from_source(localizer, &source(entries), &utc)
}

fn backup_list(entries: Vec<BackupEntry>) -> WizardBackupList {
    list_in(&en(), entries)
}

fn index_of(list: &WizardBackupList, name: &str) -> usize {
    list.rows
        .iter()
        .position(|row| row.name == name)
        .unwrap_or_else(|| panic!("no row named {name}"))
}

fn tick(list: &mut WizardBackupList, name: &str, choice: BackupChoice) -> BackupTickChange {
    let index = index_of(list, name);
    apply_backup_checkbox(&mut list.rows, index, choice, true)
}

/// That `text` reads in `locale`, as [`assert_reads`] checks it, and holds
/// none of the message ids this tab adds that don't start with "wizard-".
fn assert_reads_here(locale: &str, text: &str) {
    assert_reads(locale, text);
    for marker in [
        "backup-date",
        "size-",
        "action-",
        "restore-refusal",
        "restore-target",
    ] {
        assert!(!text.contains(marker), "{locale}: {text}");
    }
}

/// How many statuses there are. A new status fails to compile in
/// [`status_number`]; add it there, count it here, and list it in
/// [`every_status`], or `the_samples_hold_every_status` fails.
const STATUSES: usize = 13;

/// The number of each status, from 0 to [`STATUSES`] - 1.
fn status_number(status: BackupStatus) -> usize {
    match status {
        BackupStatus::CanRestore => 0,
        BackupStatus::StoppedPartway => 1,
        BackupStatus::AlreadyBack => 2,
        BackupStatus::InstalledAgain => 3,
        BackupStatus::NotComplete => 4,
        BackupStatus::NothingInIt => 5,
        BackupStatus::VersionUnknown => 6,
        BackupStatus::OtherLanguagePack => 7,
        BackupStatus::PartMissing => 8,
        BackupStatus::RecordsUnreadable => 9,
        BackupStatus::NotHere => 10,
        BackupStatus::NotYours => 11,
        BackupStatus::NotABackup => 12,
    }
}

fn every_status() -> Vec<BackupStatus> {
    vec![
        BackupStatus::CanRestore,
        BackupStatus::StoppedPartway,
        BackupStatus::AlreadyBack,
        BackupStatus::InstalledAgain,
        BackupStatus::NotComplete,
        BackupStatus::NothingInIt,
        BackupStatus::VersionUnknown,
        BackupStatus::OtherLanguagePack,
        BackupStatus::PartMissing,
        BackupStatus::RecordsUnreadable,
        BackupStatus::NotHere,
        BackupStatus::NotYours,
        BackupStatus::NotABackup,
    ]
}

/// The German language pack 1.0, which an update to 1.1 replaced on
/// November 3.
fn langpack_de() -> BackupEntry {
    BackupEntry {
        version: Some(version("1.0")),
        replaced_by: Some(version("1.1")),
        ..backup(
            package("unix-1793697120", "langpack-de"),
            BackupGroup::EarlierVersions,
            Some(NOV_3_0912),
        )
    }
}

/// The file the overview names for a backup with a part missing, or not
/// complete.
fn missing_file(entry: &BackupEntry) -> PathBuf {
    match entry.item.package_id() {
        Some("reapack") => PathBuf::from("C:/REAPER/UserPlugins/reaper_reapack-x64.dll"),
        _ => PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-x64.dll"),
    }
}

/// The refusal the overview attaches to a backup with `status`, as its
/// `status_refusal` gives it: the one a run gives for the same reason.
/// `None` for the statuses that say on their own why, or that a restore
/// can take it.
fn status_refusal(entry: &BackupEntry, status: BackupStatus) -> Option<RestoreRefusal> {
    let backup_id = entry.item.set_id()?.to_string();
    let package_id = entry.item.package_id().map(ToString::to_string);
    Some(match status {
        BackupStatus::InstalledAgain => RestoreRefusal::ReinstalledSince {
            backup_id,
            package_id: package_id?,
            other_packages: Vec::new(),
        },
        BackupStatus::NothingInIt if matches!(entry.item, BackupItem::Keymap { .. }) => {
            RestoreRefusal::NoKeymapInSet { backup_id }
        }
        BackupStatus::NothingInIt => RestoreRefusal::NothingOfItInSet {
            backup_id,
            package_ids: vec![package_id?],
        },
        BackupStatus::PartMissing => RestoreRefusal::PartMissing {
            backup_id,
            path: missing_file(entry),
        },
        BackupStatus::NotComplete => RestoreRefusal::NotComplete {
            backup_id,
            package_id: package_id?,
            path: missing_file(entry),
        },
        BackupStatus::VersionUnknown => RestoreRefusal::VersionUnknown {
            backup_id,
            package_id: package_id?,
        },
        BackupStatus::OtherLanguagePack => RestoreRefusal::OtherLanguagePackInstalled {
            backup_id,
            package_id: package_id?,
            installed: vec!["langpack-fr".to_string()],
        },
        BackupStatus::RecordsUnreadable => RestoreRefusal::ReceiptUnreadable {
            detail: "expected value at line 1 column 1".to_string(),
        },
        BackupStatus::NotYours => RestoreRefusal::NotYours { backup_id },
        BackupStatus::CanRestore
        | BackupStatus::StoppedPartway
        | BackupStatus::AlreadyBack
        | BackupStatus::NotHere
        | BackupStatus::NotABackup => return None,
    })
}

/// A backup with `status`, as the overview gives one: what it can't
/// restore has no restore scope and carries the refusal a run would give,
/// the file that is missing is named, a version RABBIT doesn't know has no
/// number, and what is no backup is of its own kind.
fn with_status(entry: BackupEntry, status: BackupStatus) -> BackupEntry {
    let missing = matches!(
        status,
        BackupStatus::PartMissing | BackupStatus::NotComplete
    );
    let unknown = status == BackupStatus::VersionUnknown;
    BackupEntry {
        status,
        restore_scope: if status.can_restore() {
            entry.restore_scope
        } else {
            None
        },
        only_copy: entry.only_copy && status != BackupStatus::AlreadyBack,
        refusal: status_refusal(&entry, status),
        missing_path: missing.then(|| missing_file(&entry)),
        version: if unknown { None } else { entry.version.clone() },
        replaced_by: if unknown {
            None
        } else {
            entry.replaced_by.clone()
        },
        set_kind: if status == BackupStatus::NotABackup {
            BackupSetKind::NotABackup
        } else {
            entry.set_kind
        },
        ..entry
    }
}

/// One backup for each status, in the group the overview puts it in.
fn one_backup_per_status() -> Vec<BackupEntry> {
    every_status()
        .into_iter()
        .map(|status| {
            let entry = match status {
                BackupStatus::NotComplete | BackupStatus::VersionUnknown => reapack(),
                BackupStatus::OtherLanguagePack => langpack_de(),
                BackupStatus::NotHere | BackupStatus::NotABackup => backup(
                    BackupItem::Set {
                        set_id: format!("set-{}", status_number(status)),
                    },
                    BackupGroup::Other,
                    None,
                ),
                _ => sws(),
            };
            with_status(entry, status)
        })
        .collect()
}

/// The backup with `status` from [`one_backup_per_status`].
fn backup_with(status: BackupStatus) -> BackupEntry {
    one_backup_per_status()
        .into_iter()
        .find(|entry| entry.status == status)
        .unwrap()
}

fn swap(package_id: &str, earlier: Option<&str>, current: Option<&str>) -> SwappedPackage {
    SwappedPackage {
        package_id: package_id.to_string(),
        version: earlier.map(version),
        replaced_version: current.map(version),
        entry: SwappedEntry::Replaced,
        uninstall_needs_reinstall: false,
        undeclined: false,
        moved_out: Vec::new(),
        changed_left: Vec::new(),
        left_in_place: Vec::new(),
    }
}

/// What the quick plan reports for `item`, as a dry run would restore it.
fn planned(item: &rabbit_core::rollback::RestoreItem) -> RestoreReportItem {
    RestoreReportItem {
        backup_id: item.set_id.clone(),
        package_id: item.package_id().map(ToString::to_string),
        keymap_only: matches!(item.part, RestorePart::Keymap),
        status: RestoreStatus::WouldRestore,
        backup_path: None,
        actions: Vec::new(),
        skipped_targets: Vec::new(),
        packages: Vec::new(),
        skipped_packages: Vec::new(),
        before_restore_set_id: None,
        swap: None,
        swaps: Vec::new(),
        keymap: None,
        refusal: None,
        error: None,
    }
}

fn quick_plan(backups: Vec<RestoreReportItem>, checks: Vec<PreflightCheck>) -> RestoreQuickPlan {
    RestoreQuickPlan::Planned(RestoreRunReport {
        resource_path: PathBuf::from("C:/REAPER"),
        dry_run: true,
        quick: true,
        preflight: PreflightReport {
            passed: true,
            checks,
        },
        backups,
        stopped: false,
        saved_report: None,
    })
}

/// A quick plan that found nothing against any backup ticked to restore in
/// `list`, with no check failing.
fn plan_all(list: &WizardBackupList) -> RestoreQuickPlan {
    quick_plan(
        backup_restore_items(list).iter().map(planned).collect(),
        Vec::new(),
    )
}

fn check(name: &str, status: PreflightStatus) -> PreflightCheck {
    PreflightCheck {
        name: name.to_string(),
        status,
        message: String::new(),
    }
}

// ---------------------------------------------------------------------------
// Rows, groups and order.
// ---------------------------------------------------------------------------

#[test]
fn rows_come_by_group_newest_first_with_a_heading_for_each_group() {
    let older_uninstall = BackupEntry {
        version: Some(version("1.0")),
        ..backup(
            package("uninstall-unix-1793613600", "reakontrol"),
            BackupGroup::UninstalledSoftware,
            Some(NOV_2_1000),
        )
    };
    let undated = backup(
        BackupItem::Set {
            set_id: "my-own-folder".to_string(),
        },
        BackupGroup::Other,
        None,
    );
    let damaged = BackupEntry {
        set_kind: BackupSetKind::Damaged,
        restore_scope: None,
        ..backup(
            BackupItem::Set {
                set_id: "unix-1790938800".to_string(),
            },
            BackupGroup::Other,
            Some(OCT_2_1100),
        )
    };
    // Given in no particular order: the list sorts them as the overview does.
    let list = backup_list(vec![
        records(),
        undated,
        keymap(),
        older_uninstall,
        damaged,
        reapack(),
        sws(),
    ]);

    let names: Vec<&str> = list.rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Software you uninstalled",
            "SWS Extension 2.14.0.7",
            "ReaKontrol 1.0",
            "Earlier versions",
            "ReaPack 1.2.4",
            "Key maps",
            "Your key map",
            "Other backups",
            "Damaged backup",
            "Backup with no record of what it holds",
            "Old copies of RABBIT's records",
        ]
    );
    for row in list.rows.iter().filter(|row| row.is_heading()) {
        assert!(!row.can_tick(BackupChoice::Restore), "{}", row.name);
        assert!(!row.can_tick(BackupChoice::Delete), "{}", row.name);
        assert_eq!(row.summary, row.name);
    }
    let headings: Vec<Option<BackupGroup>> = list
        .rows
        .iter()
        .filter(|row| row.is_heading())
        .map(|row| row.heading)
        .collect();
    assert_eq!(
        headings,
        [
            Some(BackupGroup::UninstalledSoftware),
            Some(BackupGroup::EarlierVersions),
            Some(BackupGroup::Keymaps),
            Some(BackupGroup::Other),
        ]
    );
    // Nothing is ticked, and the rows are labelled for Restore.
    assert_eq!(list.choice, BackupChoice::Restore);
    assert!(
        list.rows
            .iter()
            .all(|row| !row.restore_ticked && !row.delete_ticked)
    );
}

#[test]
fn a_group_without_backups_has_no_heading() {
    let list = backup_list(vec![keymap()]);
    let names: Vec<&str> = list.rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["Key maps", "Your key map"]);
}

#[test]
fn headings_explain_what_each_group_is_for() {
    let list = backup_list(vec![sws(), reapack(), keymap(), records()]);
    let details: Vec<&str> = list
        .rows
        .iter()
        .filter(|row| row.is_heading())
        .map(|row| row.details.as_str())
        .collect();
    assert_eq!(
        details,
        [
            "Software you uninstalled\n\nWhen RABBIT uninstalls a package, it moves the files here \
             instead of deleting them. Each one is the only copy. What a package's own \
             uninstaller, the Trash or an administrator's password removed isn't here.",
            "Earlier versions\n\nWhen an update replaces a package's files, RABBIT keeps the old \
             ones here. Updates by a package's own installer, such as REAPER's, keep none.",
            "Key maps\n\nWhen OSARA's key map replaces yours, RABBIT keeps the old one here. \
             Usually only the oldest is your own.",
            "Other backups\n\nRABBIT can't restore these here. You can delete them.",
        ]
    );
}

#[test]
fn rows_say_the_action_then_when_and_how_big() {
    let older_update = BackupEntry {
        replaced_by: None,
        ..reapack()
    };
    let mut list = backup_list(vec![sws(), reapack(), keymap(), records()]);
    let summary =
        |list: &WizardBackupList, name: &str| list.rows[index_of(list, name)].summary.clone();

    assert_eq!(
        summary(&list, "SWS Extension 2.14.0.7"),
        "SWS Extension 2.14.0.7: Won't touch. Uninstalled November 9, 2026, 2:05 PM, 4 MB"
    );
    assert_eq!(
        summary(&list, "ReaPack 1.2.4"),
        "ReaPack 1.2.4: Won't touch. Replaced by 1.2.5 on November 3, 2026, 9:12 AM, 2 MB"
    );
    assert_eq!(
        summary(&list, "Your key map"),
        "Your key map: Won't touch. Replaced by OSARA's on November 1, 2026, 6:40 PM, 38 KB"
    );
    assert_eq!(
        summary(&list, "Old copies of RABBIT's records"),
        "Old copies of RABBIT's records: Won't touch. 23 copies, 420 KB \
         (not available: can't restore it here)"
    );

    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Restore);
    assert_eq!(
        summary(&list, "ReaPack 1.2.4"),
        "ReaPack 1.2.4: Will restore. Replaced by 1.2.5 on November 3, 2026, 9:12 AM, 2 MB"
    );

    set_backup_choice(&mut list, BackupChoice::Delete);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Delete);
    tick(
        &mut list,
        "Old copies of RABBIT's records",
        BackupChoice::Delete,
    );
    assert_eq!(
        summary(&list, "SWS Extension 2.14.0.7"),
        "SWS Extension 2.14.0.7: Will delete the only copy. Uninstalled November 9, 2026, \
         2:05 PM, 4 MB"
    );
    assert_eq!(
        summary(&list, "ReaPack 1.2.4"),
        "ReaPack 1.2.4: Will delete. Replaced by 1.2.5 on November 3, 2026, 9:12 AM, 2 MB"
    );
    assert_eq!(
        summary(&list, "Old copies of RABBIT's records"),
        "Old copies of RABBIT's records: Will delete. 23 copies, 420 KB"
    );

    // An update from before RABBIT recorded what replaced it names no
    // version.
    let older = backup_list(vec![older_update]);
    assert_eq!(
        older.rows[1].summary,
        "ReaPack 1.2.4: Won't touch. Replaced on November 3, 2026, 9:12 AM, 2 MB"
    );
}

#[test]
fn a_package_rabbit_no_longer_knows_is_named_by_its_id() {
    let unknown = BackupEntry {
        version: Some(version("3.0")),
        ..backup(
            package("uninstall-unix-1794233100", "retired-tool"),
            BackupGroup::UninstalledSoftware,
            Some(NOV_9_1405),
        )
    };
    let list = backup_list(vec![unknown]);
    assert_eq!(list.rows[1].name, "retired-tool 3.0");
}

#[test]
fn key_maps_are_named_by_what_they_were() {
    let named = |origin: Option<KeymapOrigin>, set_kind: BackupSetKind| {
        let entry = BackupEntry {
            keymap_origin: origin,
            set_kind,
            ..keymap()
        };
        backup_list(vec![entry]).rows[1].name.clone()
    };
    assert_eq!(
        named(Some(KeymapOrigin::OwnKeymap), BackupSetKind::Keymap),
        "Your key map"
    );
    assert_eq!(
        named(Some(KeymapOrigin::DefaultKeys), BackupSetKind::Keymap),
        "REAPER's default keys"
    );
    assert_eq!(
        named(Some(KeymapOrigin::UnchangedOsara), BackupSetKind::Keymap),
        "An earlier OSARA key map"
    );
    assert_eq!(named(None, BackupSetKind::Keymap), "Key map");
    assert_eq!(
        named(Some(KeymapOrigin::Unknown), BackupSetKind::Keymap),
        "Key map"
    );
    assert_eq!(named(None, BackupSetKind::BeforeRestore), "Key map");
}

#[test]
fn links_unfinished_deletions_and_other_sets_are_other_backups() {
    let link = BackupEntry {
        status: BackupStatus::NotABackup,
        set_kind: BackupSetKind::NotABackup,
        restore_scope: None,
        size: 0,
        ..backup(
            BackupItem::Set {
                set_id: "linked".to_string(),
            },
            BackupGroup::Other,
            None,
        )
    };
    let deleting = BackupEntry {
        refusal: Some(RestoreRefusal::BeingDeleted {
            backup_id: "uninstall-unix-1790938800".to_string(),
        }),
        restore_scope: None,
        ..backup(
            BackupItem::Deleting {
                set_id: "uninstall-unix-1790938800".to_string(),
            },
            BackupGroup::Other,
            Some(OCT_2_1100),
        )
    };
    let mut list = backup_list(vec![link, deleting]);
    let link = index_of(&list, "Not a backup RABBIT made");
    let deleting = index_of(&list, "Unfinished deletion");
    assert_eq!(
        list.rows[link].summary,
        "Not a backup RABBIT made: Won't touch. Date unknown, under 1 KB \
         (not available: RABBIT leaves it alone)"
    );
    assert!(!list.rows[link].can_tick(BackupChoice::Restore));
    assert!(!list.rows[link].can_tick(BackupChoice::Delete));
    assert!(
        list.rows[link]
            .details
            .contains("This isn't a backup RABBIT made"),
        "{}",
        list.rows[link].details
    );

    // An unfinished deletion can't be restored, and Delete finishes it.
    assert!(!list.rows[deleting].can_tick(BackupChoice::Restore));
    set_backup_choice(&mut list, BackupChoice::Delete);
    assert!(tick(&mut list, "Unfinished deletion", BackupChoice::Delete).changed);
    assert_eq!(
        list.rows[deleting].summary,
        "Unfinished deletion: Will delete. Made on October 2, 2026, 11:00 AM, 4 MB"
    );
    assert!(
        list.rows[deleting]
            .details
            .ends_with("RABBIT can't restore it here.\nDeleting it finishes that deletion."),
        "{}",
        list.rows[deleting].details
    );
}

// ---------------------------------------------------------------------------
// Statuses.
// ---------------------------------------------------------------------------

#[test]
fn the_samples_hold_every_status() {
    let numbers: Vec<usize> = every_status().into_iter().map(status_number).collect();
    assert_eq!(numbers, (0..STATUSES).collect::<Vec<_>>());
}

#[test]
fn each_status_ticks_for_restore_and_delete_as_backup_plan_3_4_says() {
    // (status, can tick for Restore, can tick for Delete)
    let expected = [
        (BackupStatus::CanRestore, true, true),
        (BackupStatus::StoppedPartway, true, true),
        (BackupStatus::AlreadyBack, true, true),
        (BackupStatus::InstalledAgain, false, true),
        (BackupStatus::NotComplete, false, true),
        (BackupStatus::NothingInIt, false, true),
        (BackupStatus::VersionUnknown, false, true),
        (BackupStatus::OtherLanguagePack, false, true),
        (BackupStatus::PartMissing, false, true),
        (BackupStatus::RecordsUnreadable, false, true),
        (BackupStatus::NotHere, false, true),
        (BackupStatus::NotYours, false, false),
        (BackupStatus::NotABackup, false, false),
    ];
    for (entry, (status, restore, delete)) in one_backup_per_status().into_iter().zip(expected) {
        assert_eq!(entry.status, status);
        let list = backup_list(vec![entry]);
        let row = &list.rows[1];
        assert_eq!(row.can_tick(BackupChoice::Restore), restore, "{status:?}");
        assert_eq!(row.can_tick(BackupChoice::Delete), delete, "{status:?}");
    }
}

#[test]
fn a_row_that_cant_be_ticked_says_why_in_its_label_and_details() {
    let suffix = |status: BackupStatus, choice: BackupChoice| {
        let mut list = backup_list(vec![backup_with(status)]);
        set_backup_choice(&mut list, choice);
        let summary = list.rows[1].summary.clone();
        summary
            .split_once(" (not available: ")
            .map(|(_, reason)| reason.trim_end_matches(')').to_string())
    };
    let restore = |status| suffix(status, BackupChoice::Restore);
    assert_eq!(
        restore(BackupStatus::InstalledAgain).as_deref(),
        Some("you have SWS Extension again")
    );
    assert_eq!(
        restore(BackupStatus::NotComplete).as_deref(),
        Some("part of it isn't in the backup")
    );
    assert_eq!(
        restore(BackupStatus::NothingInIt).as_deref(),
        Some("none of its files are in this backup")
    );
    assert_eq!(
        restore(BackupStatus::VersionUnknown).as_deref(),
        Some("RABBIT doesn't know which version this was")
    );
    assert_eq!(
        restore(BackupStatus::OtherLanguagePack).as_deref(),
        Some("another language pack is installed")
    );
    assert_eq!(
        restore(BackupStatus::PartMissing).as_deref(),
        Some("part of it is missing")
    );
    assert_eq!(
        restore(BackupStatus::RecordsUnreadable).as_deref(),
        Some("RABBIT can't read its records")
    );
    assert_eq!(
        restore(BackupStatus::NotHere).as_deref(),
        Some("can't restore it here")
    );
    assert_eq!(
        restore(BackupStatus::NotYours).as_deref(),
        Some("belongs to another account")
    );
    assert_eq!(restore(BackupStatus::CanRestore), None);
    assert_eq!(restore(BackupStatus::AlreadyBack), None);
    // With Delete, only another account's backups and what is no backup
    // can't be ticked.
    for status in every_status() {
        let expected = match status {
            BackupStatus::NotYours => Some("belongs to another account"),
            BackupStatus::NotABackup => Some("RABBIT leaves it alone"),
            _ => None,
        };
        assert_eq!(
            suffix(status, BackupChoice::Delete).as_deref(),
            expected,
            "{status:?}"
        );
    }
}

#[test]
fn details_say_why_for_every_status_in_both_choices() {
    let sws_start = "SWS Extension 2.14.0.7, as it was before you uninstalled it.\n\
                     You uninstalled it on November 9, 2026, 2:05 PM.\n\
                     It holds its files and your SWS Extension settings, 4 MB.\n";
    let sws_lost = "If you delete this backup, RABBIT can't bring this copy back.";
    let reapack_start = "ReaPack 1.2.4, an earlier version.\n\
                         An update to 1.2.5 replaced it on November 3, 2026, 9:12 AM.\n\
                         It holds the files that were replaced, 2 MB.\n";
    let other_start = "A backup with no record of what it holds, as an earlier RABBIT left one \
                       when it stopped early.\n\
                       RABBIT can't tell when this backup was made.\n\
                       It takes up 4 MB.\n";
    let folder = PathBuf::from("C:/REAPER/RABBIT").join("backups");
    let expected = |status: BackupStatus| -> String {
        match status {
            BackupStatus::CanRestore => format!(
                "{sws_start}It is the only copy.\n\
                 Restoring puts them back, and RABBIT counts SWS Extension as installed \
                 again.\n{sws_lost}"
            ),
            BackupStatus::StoppedPartway => format!(
                "{sws_start}It is the only copy.\n\
                 The uninstall stopped partway, so some of its files are only in this backup. \
                 Restoring puts them back.\n{sws_lost}"
            ),
            BackupStatus::AlreadyBack => format!(
                "{sws_start}SWS Extension is back already. Restoring puts its files back \
                 again.\n\
                 Its files are on disk as well, so deleting it loses nothing."
            ),
            BackupStatus::InstalledAgain => format!(
                "{sws_start}It is the only copy.\n\
                 You have SWS Extension again, a copy installed after this backup was made. \
                 Uninstall the one you have first, on the Uninstall tab, then restore.\n\
                 {sws_lost}"
            ),
            BackupStatus::NotComplete => format!(
                "{reapack_start}It isn't complete: C:/REAPER/UserPlugins/reaper_reapack-x64.dll \
                 is neither in the backup nor on disk as that version had it.\n\
                 If you delete this backup, you can't go back to 1.2.4."
            ),
            BackupStatus::NothingInIt => format!(
                "{sws_start}It is the only copy.\n\
                 None of the files of SWS Extension are in it, so there is nothing to \
                 restore.\n{sws_lost}"
            ),
            BackupStatus::VersionUnknown => "ReaPack, an earlier version.\n\
                 An update replaced it on November 3, 2026, 9:12 AM.\n\
                 It holds the files that were replaced, 2 MB.\n\
                 RABBIT doesn't know which version of ReaPack it holds.\n\
                 If you delete this backup, you can't go back to this copy."
                .to_string(),
            BackupStatus::OtherLanguagePack => "REAPER in German 1.0, an earlier version.\n\
                 An update to 1.1 replaced it on November 3, 2026, 9:12 AM.\n\
                 It holds the files that were replaced, 4 MB.\n\
                 RABBIT doesn't bring REAPER in German back while this is installed: REAPER in \
                 French. Installing one of them removes the other.\n\
                 If you delete this backup, you can't go back to 1.0."
                .to_string(),
            BackupStatus::PartMissing => format!(
                "{sws_start}It is the only copy.\n\
                 Part of it is missing: its copy of C:/REAPER/UserPlugins/reaper_sws-x64.dll is \
                 gone, or isn't the file it recorded.\n{sws_lost}"
            ),
            BackupStatus::RecordsUnreadable => format!(
                "{sws_start}It is the only copy.\n\
                 RABBIT can't read its records (expected value at line 1 column 1), so it can't \
                 tell whether this backup fits them.\n{sws_lost}"
            ),
            BackupStatus::NotHere => format!(
                "{other_start}RABBIT can't restore it here.\n\
                 If you delete it, it is gone for good."
            ),
            BackupStatus::NotYours => format!(
                "{sws_start}It is the only copy.\n\
                 It belongs to another account, so RABBIT neither restores nor deletes it.\n\
                 To remove it, in Finder choose Go > Go to Folder, enter {}, then select \
                 uninstall-unix-1794233100 and press Command-Delete. Finder asks for an \
                 administrator's name and password if it needs one.",
                folder.display()
            ),
            BackupStatus::NotABackup => "This isn't a backup RABBIT made: it is a link, or a \
                 folder whose name RABBIT never gives a backup.\n\
                 RABBIT can't tell when this backup was made.\n\
                 RABBIT never looks inside it, and neither restores nor deletes it."
                .to_string(),
        }
    };
    for entry in one_backup_per_status() {
        let status = entry.status;
        let mut list = backup_list(vec![entry]);
        let restore_summary = list.rows[1].summary.clone();
        assert_eq!(
            list.rows[1].details,
            format!("{restore_summary}\n\n{}", expected(status)),
            "{status:?}"
        );
        // With Delete, only the label changes.
        set_backup_choice(&mut list, BackupChoice::Delete);
        let delete_summary = list.rows[1].summary.clone();
        assert_eq!(
            list.rows[1].details,
            format!("{delete_summary}\n\n{}", expected(status)),
            "{status:?}"
        );
        let what_it_is = restore_summary.split_once(':').unwrap().0;
        assert!(
            delete_summary.starts_with(&format!("{what_it_is}: Won't touch.")),
            "{status:?}: {delete_summary}"
        );
    }
}

#[test]
fn a_status_the_overview_found_no_refusal_for_says_why_by_itself() {
    // The overview attaches no refusal when it lacks what one names, such
    // as the package: the status's own sentence says why then.
    let installed_again = BackupEntry {
        refusal: None,
        ..with_status(sws(), BackupStatus::InstalledAgain)
    };
    let list = backup_list(vec![installed_again]);
    assert!(!list.rows[1].can_tick(BackupChoice::Restore));
    assert!(
        list.rows[1].details.contains(
            "\nYou have SWS Extension again. Uninstall the one you have first, on the Uninstall \
             tab, then restore.\n"
        ),
        "{}",
        list.rows[1].details
    );

    // A status a restore could take, with nothing that names why the
    // overview gave it no restore scope.
    let no_scope = BackupEntry {
        restore_scope: None,
        ..sws()
    };
    let list = backup_list(vec![no_scope]);
    assert!(!list.rows[1].can_tick(BackupChoice::Restore));
    assert!(
        list.rows[1]
            .summary
            .ends_with("(not available: RABBIT can't restore it)"),
        "{}",
        list.rows[1].summary
    );
    assert!(
        list.rows[1]
            .details
            .contains("\nRABBIT can't restore it.\n"),
        "{}",
        list.rows[1].details
    );
}

#[test]
fn details_say_how_records_change_only_where_the_overview_shows_it() {
    // Without a version, the uninstall may have recorded no entry to put
    // back, so the details don't promise one.
    let unversioned = BackupEntry {
        version: None,
        ..sws()
    };
    let list = backup_list(vec![unversioned]);
    assert!(
        list.rows[1]
            .details
            .contains("\nRestoring puts them back.\n"),
        "{}",
        list.rows[1].details
    );
    assert!(
        !list.rows[1].details.contains("installed again"),
        "{}",
        list.rows[1].details
    );

    // An earlier copy without a version may go back to no entry, or to one
    // without a number: Review's quick plan says which.
    let copy = BackupEntry {
        version: None,
        ..reapack()
    };
    let list = backup_list(vec![copy]);
    assert!(
        list.rows[1].details.contains(
            "\nRestoring puts this copy back and keeps the version you have now as a backup, so \
             you can go forward again.\n"
        ),
        "{}",
        list.rows[1].details
    );
    assert!(
        !list.rows[1].details.contains("counts"),
        "{}",
        list.rows[1].details
    );
}

#[test]
fn another_accounts_backup_says_how_to_remove_it_in_finder_and_never_sudo() {
    let mine_not = with_status(sws(), BackupStatus::NotYours);
    let list = backup_list(vec![mine_not]);
    let details = &list.rows[1].details;
    assert!(
        details
            .contains("It belongs to another account, so RABBIT neither restores nor deletes it."),
        "{details}"
    );
    // The set is in the hidden Library folder on macOS, so the steps go
    // through Go to Folder to the folder that holds it, then the set.
    let folder = PathBuf::from("C:/REAPER/RABBIT").join("backups");
    assert!(
        details.contains(&format!(
            "To remove it, in Finder choose Go > Go to Folder, enter {}, then select \
             uninstall-unix-1794233100 and press Command-Delete.",
            folder.display()
        )),
        "{details}"
    );
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let list = list_in(&localizer, vec![with_status(sws(), BackupStatus::NotYours)]);
        let details = &list.rows[1].details;
        assert!(!details.contains("sudo"), "{locale}: {details}");
        assert!(
            details.contains(&folder.display().to_string())
                && details.contains("uninstall-unix-1794233100"),
            "{locale}: {details}"
        );
    }
}

#[test]
fn a_refusal_the_overview_found_keeps_a_backup_from_ticking_for_restore() {
    let refused = BackupEntry {
        restore_scope: None,
        refusal: Some(RestoreRefusal::ReceiptInSet {
            backup_id: "uninstall-unix-1794233100".to_string(),
            backup_path: PathBuf::from("C:/REAPER/RABBIT/backups/uninstall-unix-1794233100"),
        }),
        ..sws()
    };
    let file_refused = BackupEntry {
        restore_scope: None,
        refusal: Some(RestoreRefusal::TargetRefused {
            path: PathBuf::from("UserPlugins/reaper_reapack-x64.dll"),
            reason: TargetRefusal::PlaceIsLink,
            other_packages: Vec::new(),
        }),
        ..reapack()
    };
    let list = backup_list(vec![refused, file_refused]);
    let sws = &list.rows[index_of(&list, "SWS Extension 2.14.0.7")];
    assert!(!sws.can_tick(BackupChoice::Restore));
    assert!(sws.can_tick(BackupChoice::Delete));
    assert!(
        sws.summary
            .ends_with("(not available: it holds a copy of RABBIT's records)"),
        "{}",
        sws.summary
    );
    assert!(
        sws.details.contains(
            "It holds a copy of RABBIT's records. Copying that back would undo every change to \
             them since, so RABBIT doesn't restore it."
        ),
        "{}",
        sws.details
    );
    assert!(
        !sws.details.contains("Restoring puts them back"),
        "{}",
        sws.details
    );

    let reapack = &list.rows[index_of(&list, "ReaPack 1.2.4")];
    assert!(
        reapack
            .summary
            .ends_with("(not available: RABBIT won't put back one of its files)"),
        "{}",
        reapack.summary
    );
    assert!(
        reapack.details.contains(
            "RABBIT doesn't put back UserPlugins/reaper_reapack-x64.dll: it is a symbolic link \
             or junction, which RABBIT never replaces."
        ),
        "{}",
        reapack.details
    );
}

#[test]
fn every_status_reads_in_both_choices_in_every_locale() {
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for entry in one_backup_per_status() {
            let status = entry.status;
            let mut list = list_in(&localizer, vec![entry]);
            for choice in [BackupChoice::Restore, BackupChoice::Delete] {
                set_backup_choice(&mut list, choice);
                apply_backup_checkbox(&mut list.rows, 1, choice, true);
                for row in &list.rows {
                    assert_reads_here(locale, &row.summary);
                    assert_reads_here(locale, &row.details);
                    assert!(
                        row.is_heading() || row.details.lines().count() >= 4,
                        "{locale} {status:?}: {}",
                        row.details
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Details.
// ---------------------------------------------------------------------------

#[test]
fn details_say_what_when_what_it_holds_and_what_restoring_and_deleting_do() {
    let older = BackupEntry {
        with_settings: None,
        ..ffmpeg()
    };
    let default_keys = BackupEntry {
        keymap_origin: Some(KeymapOrigin::DefaultKeys),
        ..keymap()
    };
    let list = backup_list(vec![sws(), reapack(), keymap()]);
    let details = |list: &WizardBackupList, name: &str| {
        let row = &list.rows[index_of(list, name)];
        row.details
            .split_once("\n\n")
            .map(|(_, explanation)| explanation.to_string())
            .unwrap()
    };

    assert_eq!(
        details(&list, "SWS Extension 2.14.0.7"),
        "SWS Extension 2.14.0.7, as it was before you uninstalled it.\n\
         You uninstalled it on November 9, 2026, 2:05 PM.\n\
         It holds its files and your SWS Extension settings, 4 MB.\n\
         It is the only copy.\n\
         Restoring puts them back, and RABBIT counts SWS Extension as installed again.\n\
         If you delete this backup, RABBIT can't bring this copy back."
    );
    assert_eq!(
        details(&list, "ReaPack 1.2.4"),
        "ReaPack 1.2.4, an earlier version.\n\
         An update to 1.2.5 replaced it on November 3, 2026, 9:12 AM.\n\
         It holds the files that were replaced, 2 MB.\n\
         Restoring goes back to 1.2.4 and keeps the version you have now as a backup, so you \
         can go forward again. Your settings stay as they are.\n\
         If you delete this backup, you can't go back to 1.2.4."
    );
    assert_eq!(
        details(&list, "Your key map"),
        "Your key map, from before OSARA's key map replaced it.\n\
         OSARA's key map replaced it on November 1, 2026, 6:40 PM.\n\
         It holds the key map, 38 KB.\n\
         It is the only copy.\n\
         Restoring makes it REAPER's key map again and keeps the current one as a backup. \
         Scripts you installed since stay in REAPER's Actions list.\n\
         If you delete this backup, RABBIT can't bring this key map back."
    );

    let older = backup_list(vec![older]);
    assert!(
        older.rows[1]
            .details
            .contains("It holds its files and anything else that uninstall moved, 250 MB."),
        "{}",
        older.rows[1].details
    );
    let default_keys = backup_list(vec![default_keys]);
    assert!(
        default_keys.rows[1].details.contains(
            "Restoring brings back REAPER's own keys, not a key map of yours, and keeps the \
             current key map as a backup."
        ),
        "{}",
        default_keys.rows[1].details
    );
}

#[test]
fn details_of_a_backup_already_back_say_deleting_it_loses_nothing() {
    let back = with_status(sws(), BackupStatus::AlreadyBack);
    let list = backup_list(vec![back]);
    let details = &list.rows[1].details;
    assert!(
        details.contains("SWS Extension is back already. Restoring puts its files back again."),
        "{details}"
    );
    assert!(
        details.contains("Its files are on disk as well, so deleting it loses nothing."),
        "{details}"
    );
    assert!(!details.contains("It is the only copy."), "{details}");
}

#[test]
fn the_records_row_says_deleting_changes_nothing_that_is_installed() {
    let list = backup_list(vec![records()]);
    let row = &list.rows[1];
    assert_eq!(
        row.details,
        "Old copies of RABBIT's records: Won't touch. 23 copies, 420 KB (not available: can't \
         restore it here)\n\n\
         Copies of RABBIT's list of what it installed, from earlier runs. Deleting them changes \
         nothing that is installed.\n\
         The newest copy is from October 2, 2026, 11:00 AM.\n\
         There are 23 copies, 420 KB in all.\n\
         RABBIT can't restore it here."
    );
}

#[test]
fn rows_of_one_set_say_which_others_they_go_with() {
    let list = backup_list(vec![sws(), ffmpeg()]);
    let sws = &list.rows[index_of(&list, "SWS Extension 2.14.0.7")];
    assert!(
        sws.details.ends_with(
            "This uninstall's backup also holds FFmpeg for Improved Video Support 7.1. Until a \
             later version, RABBIT deletes them together."
        ),
        "{}",
        sws.details
    );
    let single = backup_list(vec![reapack()]);
    assert!(!single.rows[1].details.contains("together"));
}

#[test]
fn an_undated_backup_says_rabbit_cant_tell_when() {
    let undated = BackupEntry {
        created_unix: None,
        ..sws()
    };
    let list = backup_list(vec![undated]);
    assert_eq!(
        list.rows[1].summary,
        "SWS Extension 2.14.0.7: Won't touch. Date unknown, 4 MB"
    );
    assert!(
        list.rows[1]
            .details
            .contains("RABBIT can't tell when this backup was made."),
        "{}",
        list.rows[1].details
    );
}

// ---------------------------------------------------------------------------
// The choice and the ticks.
// ---------------------------------------------------------------------------

#[test]
fn each_choice_keeps_its_own_ticks() {
    let mut list = backup_list(vec![sws(), reapack(), keymap()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    tick(&mut list, "Your key map", BackupChoice::Restore);
    assert_eq!(list.ticked_rows(BackupChoice::Restore).count(), 2);

    set_backup_choice(&mut list, BackupChoice::Delete);
    assert_eq!(list.ticked_rows(BackupChoice::Delete).count(), 0);
    assert!(
        list.rows
            .iter()
            .filter(|row| !row.is_heading())
            .all(|row| row.summary.contains(": Won't touch.")),
        "{:#?}",
        list.rows
    );
    assert_eq!(
        backups_ticked_announcement(&en(), &list),
        "No backups ticked to delete."
    );

    set_backup_choice(&mut list, BackupChoice::Restore);
    let ticked: Vec<&str> = list
        .ticked_rows(BackupChoice::Restore)
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(ticked, ["SWS Extension 2.14.0.7", "Your key map"]);
    assert!(
        list.rows[index_of(&list, "Your key map")]
            .summary
            .starts_with("Your key map: Will restore.")
    );
    assert_eq!(
        backups_ticked_announcement(&en(), &list),
        "2 backups ticked to restore."
    );
    untick_and_check_one(&mut list);
}

fn untick_and_check_one(list: &mut WizardBackupList) {
    let index = index_of(list, "Your key map");
    apply_backup_checkbox(&mut list.rows, index, BackupChoice::Restore, false);
    assert_eq!(
        backups_ticked_announcement(&en(), list),
        "One backup ticked to restore."
    );
}

#[test]
fn a_delete_tick_ties_the_rows_of_one_set_both_ways() {
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack()]);
    let sws = index_of(&list, "SWS Extension 2.14.0.7");
    let ffmpeg = index_of(&list, "FFmpeg for Improved Video Support 7.1");
    set_backup_choice(&mut list, BackupChoice::Delete);

    let change = apply_backup_checkbox(&mut list.rows, ffmpeg, BackupChoice::Delete, true);
    assert_eq!(
        change,
        BackupTickChange {
            changed: true,
            tied: vec![sws],
        }
    );
    assert!(list.rows[sws].delete_ticked && list.rows[ffmpeg].delete_ticked);
    assert!(
        list.rows[sws]
            .summary
            .starts_with("SWS Extension 2.14.0.7: Will delete the only copy.")
    );
    assert_eq!(
        backups_also_ticked_announcement(&en(), &list.rows, ffmpeg, &change).as_deref(),
        Some("SWS Extension 2.14.0.7 from the same uninstall is ticked too.")
    );
    assert!(!list.rows[index_of(&list, "ReaPack 1.2.4")].delete_ticked);

    // Ticking it again changes nothing, so nothing is refreshed or said.
    let again = apply_backup_checkbox(&mut list.rows, ffmpeg, BackupChoice::Delete, true);
    assert_eq!(again, BackupTickChange::default());
    assert_eq!(
        backups_also_ticked_announcement(&en(), &list.rows, ffmpeg, &again),
        None
    );

    // Unticking the other one unticks both.
    let change = apply_backup_checkbox(&mut list.rows, sws, BackupChoice::Delete, false);
    assert_eq!(
        change,
        BackupTickChange {
            changed: true,
            tied: vec![ffmpeg],
        }
    );
    assert!(!list.rows[sws].delete_ticked && !list.rows[ffmpeg].delete_ticked);
    assert_eq!(
        backups_also_ticked_announcement(&en(), &list.rows, sws, &change).as_deref(),
        Some("FFmpeg for Improved Video Support 7.1 from the same uninstall is unticked too.")
    );
    let again = apply_backup_checkbox(&mut list.rows, sws, BackupChoice::Delete, false);
    assert_eq!(again, BackupTickChange::default());

    // A row on its own ticks alone, and says nothing more.
    let reapack = index_of(&list, "ReaPack 1.2.4");
    let change = apply_backup_checkbox(&mut list.rows, reapack, BackupChoice::Delete, true);
    assert_eq!(
        change,
        BackupTickChange {
            changed: true,
            tied: Vec::new(),
        }
    );
    assert_eq!(
        backups_also_ticked_announcement(&en(), &list.rows, reapack, &change),
        None
    );
}

#[test]
fn a_delete_tick_names_every_other_row_it_ticks() {
    let reakontrol = BackupEntry {
        item: package("uninstall-unix-1794233100", "reakontrol"),
        version: Some(version("1.0")),
        ..sws()
    };
    let mut list = backup_list(vec![sws(), ffmpeg(), reakontrol]);
    let first = index_of(&list, "SWS Extension 2.14.0.7");
    let change = apply_backup_checkbox(&mut list.rows, first, BackupChoice::Delete, true);
    assert_eq!(change.tied.len(), 2);
    assert_eq!(
        backups_also_ticked_announcement(&en(), &list.rows, first, &change).as_deref(),
        Some(
            "FFmpeg for Improved Video Support 7.1 and ReaKontrol 1.0 from the same uninstall are \
             ticked too."
        )
    );
}

#[test]
fn restore_ticks_are_never_tied() {
    let mut list = backup_list(vec![sws(), ffmpeg()]);
    let change = tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    assert_eq!(
        change,
        BackupTickChange {
            changed: true,
            tied: Vec::new(),
        }
    );
    assert!(!list.rows[index_of(&list, "FFmpeg for Improved Video Support 7.1")].restore_ticked);
    // Ticking it again changes nothing.
    assert_eq!(
        tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore),
        BackupTickChange::default()
    );
    // And a delete tick leaves the restore ticks alone.
    tick(
        &mut list,
        "FFmpeg for Improved Video Support 7.1",
        BackupChoice::Delete,
    );
    assert!(list.rows[index_of(&list, "SWS Extension 2.14.0.7")].restore_ticked);
    assert!(!list.rows[index_of(&list, "FFmpeg for Improved Video Support 7.1")].restore_ticked);
}

#[test]
fn headings_and_rows_that_cant_be_ticked_stay_unticked() {
    let mine_not = BackupEntry {
        item: package("uninstall-unix-1", "reakontrol"),
        version: Some(version("1.0")),
        ..with_status(sws(), BackupStatus::NotYours)
    };
    let mut list = backup_list(vec![
        with_status(sws(), BackupStatus::InstalledAgain),
        mine_not,
    ]);
    for choice in [BackupChoice::Restore, BackupChoice::Delete] {
        let change = apply_backup_checkbox(&mut list.rows, 0, choice, true);
        assert_eq!(change, BackupTickChange::default());
        assert!(!list.rows[0].is_ticked(choice));
    }
    let installed_again = index_of(&list, "SWS Extension 2.14.0.7");
    let change =
        apply_backup_checkbox(&mut list.rows, installed_again, BackupChoice::Restore, true);
    assert_eq!(change, BackupTickChange::default());
    assert!(!list.rows[installed_again].restore_ticked);
    assert!(
        list.rows[installed_again]
            .summary
            .contains(": Won't touch.")
    );
    let not_yours = index_of(&list, "ReaKontrol 1.0");
    assert_eq!(
        apply_backup_checkbox(&mut list.rows, not_yours, BackupChoice::Delete, true),
        BackupTickChange::default()
    );
    assert!(!list.rows[not_yours].delete_ticked);
    // An index past the end changes nothing.
    assert_eq!(
        apply_backup_checkbox(&mut list.rows, 99, BackupChoice::Delete, true),
        BackupTickChange::default()
    );
}

#[test]
fn an_unfinished_deletion_goes_with_the_set_of_the_same_id() {
    let deleting = BackupEntry {
        restore_scope: None,
        ..backup(
            BackupItem::Deleting {
                set_id: "uninstall-unix-1794233100".to_string(),
            },
            BackupGroup::Other,
            Some(NOV_9_1405),
        )
    };
    let mut list = backup_list(vec![sws(), deleting]);
    let change = tick(&mut list, "Unfinished deletion", BackupChoice::Delete);
    assert_eq!(change.tied, vec![index_of(&list, "SWS Extension 2.14.0.7")]);
    // One delete of the id takes both; it carries the fingerprint of the
    // set in RABBIT/backups, as a delete checks that one.
    let items = backup_delete_items(&list);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].set_id, "uninstall-unix-1794233100");
    assert_eq!(items[0].fingerprint, Some(fingerprint(1)));
    // Both folders are freed.
    assert_eq!(backup_delete_bytes(&list), 2 * MB);
}

#[test]
fn a_row_that_goes_with_another_accounts_backup_cant_be_deleted() {
    let deleting = BackupEntry {
        restore_scope: None,
        ..with_status(
            backup(
                BackupItem::Deleting {
                    set_id: "uninstall-unix-1794233100".to_string(),
                },
                BackupGroup::Other,
                Some(NOV_9_1405),
            ),
            BackupStatus::NotYours,
        )
    };
    let mut list = backup_list(vec![sws(), deleting]);
    let sws = index_of(&list, "SWS Extension 2.14.0.7");
    assert!(!list.rows[sws].can_tick(BackupChoice::Delete));
    assert!(list.rows[sws].can_tick(BackupChoice::Restore));
    set_backup_choice(&mut list, BackupChoice::Delete);
    assert!(
        list.rows[sws]
            .summary
            .ends_with("(not available: goes with a backup RABBIT can't delete)"),
        "{}",
        list.rows[sws].summary
    );
    assert!(
        list.rows[sws]
            .details
            .contains("One of them can't be deleted, so RABBIT can't delete this one either."),
        "{}",
        list.rows[sws].details
    );
}

#[test]
fn a_reset_clears_both_choices_and_chooses_restore() {
    let mut list = backup_list(vec![sws(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    set_backup_choice(&mut list, BackupChoice::Delete);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Delete);
    reset_backup_ticks(&mut list);
    assert_eq!(list.choice, BackupChoice::Restore);
    assert!(
        list.rows
            .iter()
            .all(|row| !row.restore_ticked && !row.delete_ticked)
    );
    assert!(
        list.rows[index_of(&list, "ReaPack 1.2.4")]
            .summary
            .contains(": Won't touch.")
    );
}

#[test]
fn the_runs_get_what_is_ticked_with_the_listed_fingerprints() {
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack(), keymap(), records()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    tick(&mut list, "Your key map", BackupChoice::Restore);
    let items = backup_restore_items(&list);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].set_id, "uninstall-unix-1794233100");
    assert_eq!(
        items[0].part,
        RestorePart::Package {
            package_id: "sws".to_string()
        }
    );
    assert_eq!(items[0].fingerprint, Some(fingerprint(1)));
    assert_eq!(items[1].part, RestorePart::Keymap);
    assert!(items[1].fingerprint.is_some());

    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(
        &mut list,
        "Old copies of RABBIT's records",
        BackupChoice::Delete,
    );
    let items = backup_delete_items(&list);
    // SWS and FFmpeg share a set; the records stand for 23.
    assert_eq!(items.len(), 1 + 23);
    assert_eq!(items[0].set_id, "uninstall-unix-1794233100");
    assert!(items.iter().all(|item| item.fingerprint.is_some()));
    assert_eq!(backup_delete_bytes(&list), 24 * MB);
}

// ---------------------------------------------------------------------------
// Empty, unreadable, and the list's label.
// ---------------------------------------------------------------------------

#[test]
fn an_empty_list_says_rabbit_has_no_backups_and_when_it_makes_them() {
    let list = backup_list(Vec::new());
    assert!(list.rows.is_empty());
    let (label, details) = list.placeholder(&en()).unwrap();
    assert_eq!(label, "RABBIT has no backups for this REAPER.");
    assert!(
        details.starts_with("RABBIT makes a backup when it uninstalls a package"),
        "{details}"
    );
    // Nothing else on an empty tab says what never has a backup.
    assert!(
        details.ends_with(
            "What a package's own installer or uninstaller, the Trash or an administrator's \
             password changed never has a backup."
        ),
        "{details}"
    );
    assert_eq!(
        list.tab_announcement(&en()),
        "Backups. RABBIT has no backups for this REAPER."
    );
    assert_eq!(
        list.list_label(&en()),
        "Backups: what RABBIT can restore or delete"
    );
}

#[test]
fn an_unreadable_backups_folder_says_why() {
    let error = RabbitError::RestoreRefused(RestoreRefusal::IsLink {
        path: PathBuf::from("C:/REAPER/RABBIT/backups"),
    });
    let list = backup_list_unreadable(&error);
    let (label, details) = list.placeholder(&en()).unwrap();
    assert_eq!(label, "RABBIT couldn't read the backups of this REAPER.");
    assert!(
        details.starts_with("RABBIT couldn't read its backups folder (RABBIT doesn't use"),
        "{details}"
    );
    assert!(
        details.ends_with("so it can't list what it could restore or delete."),
        "{details}"
    );
    assert_eq!(
        list.tab_announcement(&en()),
        "Backups. RABBIT couldn't read the backups of this REAPER."
    );
}

/// Make `link` point to the folder `target`: a junction on Windows, which
/// needs no administrator rights, a symbolic link elsewhere.
fn link_folder(link: &std::path::Path, target: &std::path::Path) {
    #[cfg(windows)]
    {
        // cmd reads a `/` in a path as the start of an option.
        let windows_path = |path: &std::path::Path| path.to_string_lossy().replace('/', "\\");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(windows_path(link))
            .arg(windows_path(target))
            .output()
            .unwrap();
        assert!(made.status.success(), "{made:?}");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[test]
fn a_backups_folder_that_is_a_link_leaves_the_list_empty_and_says_why() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir_all(resource.join("RABBIT")).unwrap();
    fs::create_dir_all(&elsewhere).unwrap();
    link_folder(&resource.join("RABBIT").join("backups"), &elsewhere);

    let list = backup_list_at(&en(), &resource);
    assert!(list.rows.is_empty());
    assert!(list.unreadable.is_some());
    let (label, details) = list.placeholder(&en()).unwrap();
    assert_eq!(label, "RABBIT couldn't read the backups of this REAPER.");
    assert!(
        details.starts_with("RABBIT couldn't read its backups folder ("),
        "{details}"
    );
    assert!(
        details.ends_with("so it can't list what it could restore or delete."),
        "{details}"
    );
}

#[test]
fn the_list_label_names_the_tab_and_the_total() {
    let mut source = source(vec![sws()]);
    source.total_size = 742 * MB;
    let list = backup_list_from_source(&en(), &source, &utc);
    assert_eq!(
        list.list_label(&en()),
        "Backups: what RABBIT can restore or delete, 742 MB in all"
    );
    assert!(list.placeholder(&en()).is_none());
    assert_eq!(list.tab_announcement(&en()), "Backups");
}

#[test]
fn a_reaper_without_backups_lists_none_and_gets_no_folder() {
    let dir = tempdir().unwrap();
    let list = backup_list_at(&en(), dir.path());
    assert!(list.rows.is_empty());
    assert!(list.unreadable.is_none());
    assert!(!dir.path().join("RABBIT").exists());
}

/// Uninstall SWS 2.14.0.7 and FFmpeg 7.1 from `resource` the way an
/// uninstall moves them: one set, `uninstall-unix-1794233100`, that holds
/// both, each with its record.
fn write_uninstall_set(resource: &std::path::Path) {
    let plugins = resource.join("UserPlugins");
    fs::create_dir_all(&plugins).unwrap();
    fs::write(plugins.join("reaper_sws-x64.dll"), b"sws").unwrap();
    fs::write(plugins.join("avcodec-61.dll"), b"ffmpeg").unwrap();
    let mut writer = BackupSetWriter::new(
        resource,
        "uninstall-unix-1794233100",
        UNINSTALL_BACKUP_REASON,
    );
    let receipt = |package_id: &str, version_text: &str| PackageReceipt {
        id: package_id.to_string(),
        version: Some(version(version_text)),
        source_url: None,
        source_sha256: None,
        variant: None,
        channel: None,
        installed_files: Vec::new(),
        installed_at: None,
        rabbit_version: None,
        architecture: None,
        vendor_added_files: Vec::new(),
    };
    for (package_id, file, version_text) in [
        ("sws", "reaper_sws-x64.dll", "2.14.0.7"),
        ("ffmpeg", "avcodec-61.dll", "7.1"),
    ] {
        writer
            .record_package(
                package_id,
                BackupPackageRecord {
                    receipt: Some(receipt(package_id, version_text)),
                    langpack_before: None,
                    with_settings: Some(false),
                },
            )
            .unwrap();
        writer
            .move_file(Some(package_id), &plugins.join(file), None, false)
            .unwrap();
    }
}

#[test]
fn the_rows_come_from_a_real_uninstall_set() {
    let dir = tempdir().unwrap();
    let resource = dir.path();
    write_uninstall_set(resource);

    let overview = backup_overview(resource).unwrap();
    let mut list = backup_list_from_overview(&en(), &overview);
    let names: Vec<&str> = list.rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Software you uninstalled",
            "FFmpeg for Improved Video Support 7.1",
            "SWS Extension 2.14.0.7",
        ]
    );
    let sws = index_of(&list, "SWS Extension 2.14.0.7");
    assert!(list.rows[sws].can_tick(BackupChoice::Restore));
    assert!(!list.rows[sws].restore_targets().is_empty());
    assert!(
        list.rows[sws]
            .summary
            .starts_with("SWS Extension 2.14.0.7: Won't touch. Uninstalled "),
        "{}",
        list.rows[sws].summary
    );

    // The two go together for Delete, as one set holds both.
    let change = tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    assert_eq!(change.tied.len(), 1);
    let items = backup_delete_items(&list);
    assert_eq!(items.len(), 1);
    // The set has a fingerprint, so the run gets one to compare.
    assert!(overview.sets[0].fingerprint.is_some());
    assert_eq!(items[0].fingerprint, overview.sets[0].fingerprint);
    assert_eq!(backup_delete_bytes(&list), overview.sets[0].size);

    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    let items = backup_restore_items(&list);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].fingerprint, overview.sets[0].fingerprint);
}

// ---------------------------------------------------------------------------
// Dates and sizes.
// ---------------------------------------------------------------------------

#[test]
fn dates_read_in_every_locale() {
    let date = |locale: &str, unix: u64, offset: i32| {
        backup_date_text(&Localizer::embedded(locale).unwrap(), unix, offset).unwrap()
    };
    assert_eq!(date("en-US", NOV_9_1405, 0), "November 9, 2026, 2:05 PM");
    assert_eq!(date("de-DE", NOV_9_1405, 0), "9. November 2026, 14:05 Uhr");
    assert_eq!(
        date("es-ES", NOV_9_1405, 0),
        "9 de noviembre de 2026, 14:05"
    );
    assert_eq!(date("fr-FR", NOV_9_1405, 0), "9 novembre 2026 à 14 h 05");
    assert_eq!(date("it-IT", NOV_9_1405, 0), "9 novembre 2026, 14:05");
    assert_eq!(date("pt-BR", NOV_9_1405, 0), "9 de novembro de 2026, 14:05");

    // Local time: an hour ahead of UTC.
    assert_eq!(date("en-US", NOV_9_1405, 3600), "November 9, 2026, 3:05 PM");
    assert_eq!(
        date("de-DE", NOV_9_1405, 3600),
        "9. November 2026, 15:05 Uhr"
    );
    // The 12-hour clock at midnight and noon, and the first of a month.
    let midnight = NOV_9_1405 - 14 * 3600;
    let noon = NOV_9_1405 - 2 * 3600 - 5 * 60;
    assert_eq!(date("en-US", midnight, 0), "November 9, 2026, 12:05 AM");
    assert_eq!(date("en-US", noon, 0), "November 9, 2026, 12:00 PM");
    assert_eq!(date("de-DE", midnight, 0), "9. November 2026, 0:05 Uhr");
    assert_eq!(date("en-US", NOV_1_1840, 0), "November 1, 2026, 6:40 PM");
    assert_eq!(date("fr-FR", NOV_1_1840, 0), "1er novembre 2026 à 18 h 40");

    // Every month reads in every locale, by its own name: the 15th of each
    // month of 2026, at noon.
    let month_names = |locale: &str| -> [&str; 12] {
        match locale {
            "en-US" => [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ],
            "de-DE" => [
                "Januar",
                "Februar",
                "März",
                "April",
                "Mai",
                "Juni",
                "Juli",
                "August",
                "September",
                "Oktober",
                "November",
                "Dezember",
            ],
            "es-ES" => [
                "enero",
                "febrero",
                "marzo",
                "abril",
                "mayo",
                "junio",
                "julio",
                "agosto",
                "septiembre",
                "octubre",
                "noviembre",
                "diciembre",
            ],
            "fr-FR" => [
                "janvier",
                "février",
                "mars",
                "avril",
                "mai",
                "juin",
                "juillet",
                "août",
                "septembre",
                "octobre",
                "novembre",
                "décembre",
            ],
            "it-IT" => [
                "gennaio",
                "febbraio",
                "marzo",
                "aprile",
                "maggio",
                "giugno",
                "luglio",
                "agosto",
                "settembre",
                "ottobre",
                "novembre",
                "dicembre",
            ],
            "pt-BR" => [
                "janeiro",
                "fevereiro",
                "março",
                "abril",
                "maio",
                "junho",
                "julho",
                "agosto",
                "setembro",
                "outubro",
                "novembro",
                "dezembro",
            ],
            other => panic!("no month names for {other}"),
        }
    };
    // Days before the 1st of each month of 2026.
    let days_before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for (month, (name, days)) in month_names(locale).iter().zip(days_before).enumerate() {
            let unix = 1_767_225_600 + (days + 14) * 86_400 + 12 * 3600;
            let text = backup_date_text(&localizer, unix, 0).unwrap();
            assert_reads_here(locale, &text);
            assert!(text.contains("2026"), "{locale}: {text}");
            assert!(text.contains("15"), "{locale}: {text}");
            // Compared by whole words, so a name inside a longer word
            // doesn't count.
            let words: Vec<&str> = text
                .split(|c: char| !c.is_alphabetic())
                .filter(|word| !word.is_empty())
                .collect();
            assert!(
                words.contains(name),
                "{locale}, month {}: {text}",
                month + 1
            );
        }
        assert_reads_here(locale, &localizer.text("backup-date-unknown").value);
    }
}

#[test]
fn a_moment_too_far_from_1970_reads_as_an_unknown_date() {
    assert_eq!(backup_date_text(&en(), u64::MAX, 0), None);
    let far = BackupEntry {
        created_unix: Some(u64::MAX),
        ..sws()
    };
    assert!(
        backup_list(vec![far]).rows[1]
            .summary
            .contains("Date unknown")
    );
}

#[test]
fn sizes_read_at_their_boundaries() {
    let size =
        |locale: &str, bytes: u64| backup_size_text(&Localizer::embedded(locale).unwrap(), bytes);
    assert_eq!(size("en-US", 0), "under 1 KB");
    assert_eq!(size("en-US", 1023), "under 1 KB");
    assert_eq!(size("en-US", KB), "1 KB");
    assert_eq!(size("en-US", MB), "1 MB");
    assert_eq!(size("en-US", GB), "1.0 GB");
    assert_eq!(size("en-US", GB + GB / 2), "1.5 GB");
    assert_eq!(size("de-DE", GB + GB / 2), "1,5 GB");
    assert_eq!(size("de-DE", 1023), "unter 1 KB");
    assert_eq!(size("fr-FR", GB + GB / 2), "1,5 Go");
    for &locale in embedded_locales() {
        for bytes in [0, KB, 742 * MB, GB + GB / 2] {
            assert_reads_here(locale, &size(locale, bytes));
        }
    }
}

// ---------------------------------------------------------------------------
// Review before a restore.
// ---------------------------------------------------------------------------

#[test]
fn restore_review_says_what_comes_back_and_what_else_changes() {
    let mut list = backup_list(vec![sws(), reapack(), keymap(), records()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Restore);
    tick(&mut list, "Your key map", BackupChoice::Restore);
    tick(
        &mut list,
        "Old copies of RABBIT's records",
        BackupChoice::Delete,
    );
    let items = backup_restore_items(&list);
    let mut sws_plan = planned(&items[0]);
    sws_plan.packages.push(RestoredPackage {
        package_id: "sws".to_string(),
        receipt_merged: true,
        receipt_already_current: false,
        langpack_before: Some("de_DE.ReaperLangPack".to_string()),
        langpack_restored: true,
        langpack_error: None,
    });
    let mut reapack_plan = planned(&items[1]);
    reapack_plan.swap = Some(swap("reapack", Some("1.2.4"), Some("1.2.5")));
    let plan = quick_plan(
        vec![sws_plan, reapack_plan, planned(&items[2])],
        vec![check(REAPER_PROCESS_CHECK, PreflightStatus::Fail)],
    );

    let review = build_backup_restore_review(
        &en(),
        Some(&target(true)),
        &list,
        Some(&plan),
        &[
            "What is ticked on the Uninstall tab isn't part of this run: SWS Extension."
                .to_string(),
        ],
    );
    assert_eq!(review.heading, "Ready to restore");
    assert_eq!(
        review.lines,
        [
            "Target: C:/REAPER",
            "",
            "SWS Extension 2.14.0.7 comes back as it was on November 9, 2026, 2:05 PM, with your \
             SWS Extension settings.",
            "ReaPack goes back from 1.2.5 to 1.2.4. RABBIT keeps 1.2.5 as a backup.",
            "Your key map from November 1, 2026, 6:40 PM replaces the current one, which RABBIT \
             keeps as a backup.",
            "",
            "Notes",
            "REAPER's language goes back to de_DE.ReaperLangPack.",
            "Close REAPER first.",
            "What is ticked on the Uninstall tab isn't part of this run: SWS Extension.",
            "One backup ticked to delete isn't part of this run.",
        ]
    );
    assert!(review.can_restore);
    assert_eq!(review.items, items);
}

#[test]
fn restore_review_names_an_earlier_version_without_the_versions_from_the_quick_plan() {
    let copy = BackupEntry {
        version: None,
        ..reapack()
    };
    let mut list = backup_list(vec![reapack()]);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Restore);
    let review = build_backup_restore_review(
        &en(),
        Some(&target(true)),
        &list,
        Some(&plan_all(&list)),
        &[],
    );
    assert_eq!(
        review.lines,
        [
            "Target: C:/REAPER",
            "",
            "ReaPack goes back to 1.2.4. RABBIT keeps the version you have now as a backup.",
        ]
    );
    assert!(review.can_restore);
    let mut copy = backup_list(vec![copy]);
    apply_backup_checkbox(&mut copy.rows, 1, BackupChoice::Restore, true);
    let review = build_backup_restore_review(
        &en(),
        Some(&target(true)),
        &copy,
        Some(&plan_all(&copy)),
        &[],
    );
    assert_eq!(
        review.lines[2],
        "ReaPack goes back to the copy an update replaced. RABBIT keeps the version you have now \
         as a backup."
    );
}

#[test]
fn restore_review_says_the_language_goes_back_once() {
    // Two uninstalls each took a language away. The quick plan checks each
    // against REAPER's settings as they are, so both say it goes back; the
    // run puts back only the first in list order.
    let earlier_french = BackupEntry {
        item: package("uninstall-unix-1790938800", "langpack-fr"),
        version: Some(version("1.0")),
        created_unix: Some(OCT_2_1100),
        ..sws()
    };
    let german = BackupEntry {
        item: package("uninstall-unix-1794233100", "langpack-de"),
        version: Some(version("1.0")),
        ..sws()
    };
    let mut list = backup_list(vec![german, earlier_french]);
    tick(&mut list, "REAPER in German 1.0", BackupChoice::Restore);
    tick(&mut list, "REAPER in French 1.0", BackupChoice::Restore);
    let items = backup_restore_items(&list);
    let with_language = |item, package_id: &str, file: &str| {
        let mut plan = planned(item);
        plan.packages.push(RestoredPackage {
            package_id: package_id.to_string(),
            receipt_merged: true,
            receipt_already_current: false,
            langpack_before: Some(file.to_string()),
            langpack_restored: true,
            langpack_error: None,
        });
        plan
    };
    let plan = quick_plan(
        vec![
            with_language(&items[0], "langpack-de", "de_DE.ReaperLangPack"),
            with_language(&items[1], "langpack-fr", "fr_CA.ReaperLangPack"),
        ],
        Vec::new(),
    );
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&plan), &[]);
    let notes: Vec<&str> = review
        .lines
        .iter()
        .skip_while(|line| *line != "Notes")
        .skip(1)
        .map(String::as_str)
        .collect();
    assert_eq!(
        notes,
        ["REAPER's language goes back to de_DE.ReaperLangPack."]
    );
    assert!(review.can_restore);
}

#[test]
fn restore_review_says_what_the_quick_plan_reports_beyond_the_files() {
    let mut list = backup_list(vec![sws(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Restore);
    let items = backup_restore_items(&list);
    let mut sws_plan = planned(&items[0]);
    sws_plan.packages.push(RestoredPackage {
        package_id: "sws".to_string(),
        receipt_merged: true,
        receipt_already_current: false,
        langpack_before: Some("de_DE.ReaperLangPack".to_string()),
        langpack_restored: false,
        langpack_error: None,
    });
    let mut reapack_plan = planned(&items[1]);
    reapack_plan.swap = Some(SwappedPackage {
        entry: SwappedEntry::Removed,
        uninstall_needs_reinstall: true,
        ..swap("reapack", None, Some("1.2.5"))
    });
    let plan = quick_plan(
        vec![sws_plan, reapack_plan],
        vec![
            check(REAPER_PROCESS_CHECK, PreflightStatus::Pass),
            check(ANY_REAPER_PROCESS_CHECK, PreflightStatus::Warn),
        ],
    );
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&plan), &[]);
    let notes: Vec<&str> = review
        .lines
        .iter()
        .skip_while(|line| *line != "Notes")
        .skip(1)
        .map(String::as_str)
        .collect();
    assert_eq!(
        notes,
        [
            "REAPER's language stays as it is, because another one was chosen since.",
            "RABBIT no longer counts ReaPack as installed by it.",
            "To uninstall ReaPack with RABBIT later, install it again first.",
            "Close every copy of REAPER first.",
        ]
    );
    assert!(
        review
            .lines
            .contains(&"ReaPack goes back to the copy an update replaced. RABBIT keeps the version you have now as a backup.".to_string()),
        "{:#?}",
        review.lines
    );
    assert!(review.can_restore);
}

#[test]
fn restore_review_blocks_when_nothing_is_ticked_or_there_is_no_target() {
    let list = backup_list(vec![sws()]);
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, None, &[]);
    assert_eq!(
        review.lines,
        [
            "Target: C:/REAPER",
            "",
            "Can't restore yet: Nothing is ticked to restore.",
        ]
    );
    assert!(!review.can_restore);

    let review = build_backup_restore_review(&en(), None, &list, None, &[]);
    assert_eq!(review.lines, ["No target selected."]);
    assert!(!review.can_restore);
}

#[test]
fn restore_review_blocks_two_backups_of_one_package_naming_both() {
    let earlier_sws = BackupEntry {
        item: package("uninstall-unix-1790938800", "sws"),
        version: Some(version("2.13")),
        created_unix: Some(OCT_2_1100),
        ..sws()
    };
    // Both write the same file, as two backups of one package do; the
    // blocker for the package says it already.
    let mut source = source(vec![sws(), earlier_sws.clone()]);
    let place = PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-x64.dll");
    source.targets.insert(sws().item, vec![place.clone()]);
    source.targets.insert(earlier_sws.item.clone(), vec![place]);
    let mut list = backup_list_from_source(&en(), &source, &utc);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    tick(&mut list, "SWS Extension 2.13", BackupChoice::Restore);
    let review = build_backup_restore_review(
        &en(),
        Some(&target(true)),
        &list,
        Some(&plan_all(&list)),
        &[],
    );
    let blockers: Vec<&String> = review
        .lines
        .iter()
        .filter(|line| line.starts_with("Can't restore"))
        .collect();
    assert_eq!(
        blockers,
        [
            "Can't restore yet: SWS Extension 2.14.0.7 (Uninstalled November 9, 2026, 2:05 PM) \
             and SWS Extension 2.13 (Uninstalled October 2, 2026, 11:00 AM) are both backups of \
             SWS Extension. Tick only one of them."
        ]
    );
    assert!(!review.can_restore);
}

#[test]
fn restore_review_blocks_two_backups_that_write_the_same_file_naming_both() {
    let older_keymap = BackupEntry {
        item: BackupItem::Keymap {
            set_id: "restore-unix-1790938800-before-restore".to_string(),
        },
        set_kind: BackupSetKind::BeforeRestore,
        keymap_origin: None,
        created_unix: Some(OCT_2_1100),
        ..keymap()
    };
    let mut source = source(vec![keymap(), older_keymap.clone(), sws()]);
    let place = PathBuf::from("C:/REAPER/reaper-kb.ini");
    source.targets.insert(keymap().item, vec![place.clone()]);
    // The same file, written in another case.
    source.targets.insert(
        older_keymap.item.clone(),
        vec![PathBuf::from("C:/REAPER/REAPER-kb.ini")],
    );
    source.targets.insert(
        sws().item,
        vec![PathBuf::from("C:/REAPER/UserPlugins/reaper_sws-x64.dll")],
    );
    let mut list = backup_list_from_source(&en(), &source, &utc);
    tick(&mut list, "Your key map", BackupChoice::Restore);
    tick(&mut list, "Key map", BackupChoice::Restore);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    assert_eq!(
        list.rows[index_of(&list, "Your key map")].restore_targets(),
        [place]
    );
    let review = build_backup_restore_review(
        &en(),
        Some(&target(true)),
        &list,
        Some(&plan_all(&list)),
        &[],
    );
    let blockers: Vec<&String> = review
        .lines
        .iter()
        .filter(|line| line.starts_with("Can't restore"))
        .collect();
    assert_eq!(
        blockers,
        [
            "Can't restore yet: Your key map (Replaced by OSARA's on November 1, 2026, 6:40 PM) \
             and Key map (Replaced by a restore on October 2, 2026, 11:00 AM) both put back the \
             same file. Tick only one of them."
        ]
    );
    assert!(!review.can_restore);
}

/// Older sets can record a place with `/`, newer ones with `\`, and a place
/// may carry the `\\?\` prefix: on Windows each names one file, as a
/// restore compares them.
#[cfg(windows)]
#[test]
fn restore_review_finds_the_same_file_written_with_other_separators_or_a_prefix() {
    let older_keymap = BackupEntry {
        item: BackupItem::Keymap {
            set_id: "restore-unix-1790938800-before-restore".to_string(),
        },
        set_kind: BackupSetKind::BeforeRestore,
        keymap_origin: None,
        created_unix: Some(OCT_2_1100),
        ..keymap()
    };
    for other in [
        r"C:\REAPER\UserPlugins\x.dll",
        r"\\?\C:\REAPER\UserPlugins\X.DLL",
    ] {
        let mut source = source(vec![keymap(), older_keymap.clone()]);
        source.targets.insert(
            keymap().item,
            vec![PathBuf::from("C:/REAPER/UserPlugins/x.dll")],
        );
        source
            .targets
            .insert(older_keymap.item.clone(), vec![PathBuf::from(other)]);
        let mut list = backup_list_from_source(&en(), &source, &utc);
        tick(&mut list, "Your key map", BackupChoice::Restore);
        tick(&mut list, "Key map", BackupChoice::Restore);
        let review = build_backup_restore_review(
            &en(),
            Some(&target(true)),
            &list,
            Some(&plan_all(&list)),
            &[],
        );
        assert!(
            review
                .lines
                .iter()
                .any(|line| line.ends_with("both put back the same file. Tick only one of them.")),
            "{other}: {:#?}",
            review.lines
        );
        assert!(!review.can_restore);
    }
}

#[test]
fn restore_review_blocks_a_target_rabbit_cant_write_to() {
    let mut list = backup_list(vec![sws()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    let review = build_backup_restore_review(
        &en(),
        Some(&target(false)),
        &list,
        Some(&plan_all(&list)),
        &[],
    );
    assert_eq!(
        review.lines.last().unwrap(),
        "Can't restore yet: RABBIT can't write to this REAPER's folder."
    );
    assert!(!review.can_restore);
}

#[test]
fn restore_review_blocks_backups_the_quick_plan_did_not_check() {
    let mut list = backup_list(vec![sws(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    let not_checked = "Can't restore yet: RABBIT hasn't checked what is ticked.";

    // Before the quick plan ran.
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, None, &[]);
    assert_eq!(review.lines.last().unwrap(), not_checked);
    assert!(!review.can_restore);

    // A plan from before another row was ticked: it misses ReaPack.
    let earlier = plan_all(&list);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Restore);
    let review =
        build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&earlier), &[]);
    assert_eq!(review.lines.last().unwrap(), not_checked);
    assert!(!review.can_restore);

    // A plan from before a row was unticked: it holds one too many.
    let both = plan_all(&list);
    let reapack = index_of(&list, "ReaPack 1.2.4");
    apply_backup_checkbox(&mut list.rows, reapack, BackupChoice::Restore, false);
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&both), &[]);
    assert_eq!(review.lines.last().unwrap(), not_checked);
    assert!(!review.can_restore);

    // A plan of what is ticked now lets the restore start.
    let review = build_backup_restore_review(
        &en(),
        Some(&target(true)),
        &list,
        Some(&plan_all(&list)),
        &[],
    );
    assert!(review.can_restore, "{:#?}", review.lines);
    assert!(!review.lines.iter().any(|line| line == not_checked));
}

#[test]
fn restore_review_blocks_what_the_quick_plan_refused() {
    let mut list = backup_list(vec![sws(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Restore);
    let items = backup_restore_items(&list);
    let mut sws_plan = planned(&items[0]);
    sws_plan.status = RestoreStatus::NotRestored;
    sws_plan.refusal = Some(RestoreRefusal::ChangedSinceListed {
        backup_id: items[0].set_id.clone(),
    });
    let mut reapack_plan = planned(&items[1]);
    reapack_plan.status = RestoreStatus::NotRestored;
    reapack_plan.error = Some("access denied".to_string());
    let plan = quick_plan(vec![sws_plan, reapack_plan], Vec::new());
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&plan), &[]);
    let blockers: Vec<&String> = review
        .lines
        .iter()
        .filter(|line| line.starts_with("Can't restore"))
        .collect();
    assert_eq!(
        blockers,
        [
            "Can't restore SWS Extension 2.14.0.7 (Uninstalled November 9, 2026, 2:05 PM) yet. \
             It changed since RABBIT listed it. Start RABBIT again to see the backups as they \
             are now.",
            "Can't restore ReaPack 1.2.4 (Replaced by 1.2.5 on November 3, 2026, 9:12 AM) yet. \
             RABBIT couldn't check it (access denied).",
        ]
    );
    assert!(!review.can_restore);

    // A refusal of the whole run, and a check that failed.
    let refused =
        RestoreQuickPlan::from_result(Err(RabbitError::RestoreRefused(RestoreRefusal::LockHeld {
            lock_path: PathBuf::from("C:/REAPER/RABBIT/package-install.lock"),
            pid: 7,
        })));
    let review =
        build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&refused), &[]);
    assert_eq!(
        review.lines.last().unwrap(),
        "Can't restore yet: Another copy of RABBIT is working on this REAPER. Wait until it has \
         finished."
    );
    assert!(!review.can_restore);
    let failed = RestoreQuickPlan::Failed("disk on fire".to_string());
    let review = build_backup_restore_review(&en(), Some(&target(true)), &list, Some(&failed), &[]);
    assert_eq!(
        review.lines.last().unwrap(),
        "Can't restore yet: RABBIT couldn't check the restore (disk on fire)."
    );
}

#[test]
fn the_quick_plan_runs_dry_without_reading_files() {
    let options = backup_quick_plan_options();
    assert!(options.dry_run && options.quick && !options.allow_reaper_running);
}

// ---------------------------------------------------------------------------
// Review before a delete, and the question.
// ---------------------------------------------------------------------------

#[test]
fn delete_review_says_what_goes_and_what_it_frees() {
    let no_manifest = backup(
        BackupItem::Set {
            set_id: "unix-1790938800".to_string(),
        },
        BackupGroup::Other,
        Some(OCT_2_1100),
    );
    let mut list = backup_list(vec![
        sws(),
        ffmpeg(),
        reapack(),
        keymap(),
        no_manifest,
        records(),
    ]);
    set_backup_choice(&mut list, BackupChoice::Delete);
    tick(
        &mut list,
        "FFmpeg for Improved Video Support 7.1",
        BackupChoice::Delete,
    );
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Delete);
    tick(&mut list, "Your key map", BackupChoice::Delete);
    tick(
        &mut list,
        "Backup with no record of what it holds",
        BackupChoice::Delete,
    );
    tick(
        &mut list,
        "Old copies of RABBIT's records",
        BackupChoice::Delete,
    );
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Restore);

    let review = build_backup_delete_review(&en(), Some(&target(true)), &list, &[]);
    assert_eq!(review.heading, "Ready to delete backups");
    assert_eq!(
        review.lines,
        [
            "Target: C:/REAPER",
            "",
            "These backups go for good and free 27 MB:",
            "SWS Extension 2.14.0.7, uninstalled November 9, 2026, 2:05 PM: the only copy.",
            "FFmpeg for Improved Video Support 7.1, uninstalled November 9, 2026, 2:05 PM: the \
             only copy.",
            "ReaPack 1.2.4: you can't go back to it after this.",
            "Your key map from November 1, 2026, 6:40 PM: the only copy.",
            "Backup with no record of what it holds: it is gone for good.",
            "Old copies of RABBIT's records: nothing that is installed changes.",
            "SWS Extension 2.14.0.7 and FFmpeg for Improved Video Support 7.1, uninstalled \
             November 9, 2026, 2:05 PM, go together: one backup holds both.",
            "",
            "Notes",
            "One backup ticked to restore isn't part of this run.",
        ]
    );
    assert!(review.can_delete);
    assert_eq!(review.bytes_freed, 27 * MB);
    assert_eq!(review.items.len(), 4 + 23);
}

#[test]
fn delete_review_says_what_made_backups_that_go_together_and_when() {
    let together = |entries: Vec<BackupEntry>| {
        let mut list = backup_list(entries);
        let first = list.rows.iter().position(|row| !row.is_heading()).unwrap();
        apply_backup_checkbox(&mut list.rows, first, BackupChoice::Delete, true);
        let review = build_backup_delete_review(&en(), Some(&target(true)), &list, &[]);
        review
            .lines
            .iter()
            .find(|line| line.contains("go together"))
            .cloned()
            .unwrap_or_else(|| panic!("{:#?}", review.lines))
    };
    // An update that replaced two packages.
    let earlier_sws = BackupEntry {
        item: package("unix-1793697120", "sws"),
        version: Some(version("2.13")),
        ..reapack()
    };
    assert_eq!(
        together(vec![reapack(), earlier_sws]),
        "ReaPack 1.2.4 and SWS Extension 2.13, replaced November 3, 2026, 9:12 AM, go together: \
         one backup holds both."
    );
    // A restore's backup, of a version and a key map.
    let restored = BackupEntry {
        item: package("restore-unix-1790938800-before-restore", "reapack"),
        set_kind: BackupSetKind::BeforeRestore,
        created_unix: Some(OCT_2_1100),
        ..reapack()
    };
    let restored_keymap = BackupEntry {
        item: BackupItem::Keymap {
            set_id: "restore-unix-1790938800-before-restore".to_string(),
        },
        set_kind: BackupSetKind::BeforeRestore,
        keymap_origin: None,
        created_unix: Some(OCT_2_1100),
        ..keymap()
    };
    assert_eq!(
        together(vec![restored, restored_keymap]),
        "ReaPack 1.2.4 and Key map, from October 2, 2026, 11:00 AM, go together: one backup \
         holds both."
    );
    // Without a date, and three at once.
    let undated = |package_id: &str| BackupEntry {
        item: package("uninstall-unix-1", package_id),
        version: None,
        created_unix: None,
        ..sws()
    };
    assert_eq!(
        together(vec![undated("sws"), undated("ffmpeg"), undated("reapack")]),
        "SWS Extension, FFmpeg for Improved Video Support and ReaPack go together: one backup \
         holds all 3."
    );
}

#[test]
fn delete_review_blocks_when_nothing_is_ticked() {
    let list = backup_list(vec![sws()]);
    let review =
        build_backup_delete_review(&en(), Some(&target(true)), &list, &["Note.".to_string()]);
    assert_eq!(
        review.lines,
        [
            "Target: C:/REAPER",
            "",
            "Notes",
            "Note.",
            "",
            "Can't delete yet: Nothing is ticked to delete.",
        ]
    );
    assert!(!review.can_delete);
    assert!(review.items.is_empty());
}

#[test]
fn the_delete_question_counts_backups_space_and_only_copies() {
    let mut list = backup_list(vec![sws(), ffmpeg(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Delete);
    let (title, body) = delete_backups_question(&en(), &list);
    assert_eq!(title, "Delete backups for good?");
    assert_eq!(
        body,
        "RABBIT deletes 3 backups and frees 2 MB. 2 of them are the only copy of what they \
         hold. A deleted backup can't be brought back. Delete now?"
    );

    let mut one = backup_list(vec![keymap()]);
    apply_backup_checkbox(&mut one.rows, 1, BackupChoice::Delete, true);
    let (_, body) = delete_backups_question(&en(), &one);
    assert_eq!(
        body,
        "RABBIT deletes one backup and frees 1 MB. It is the only copy of what it holds. A \
         deleted backup can't be brought back. Delete now?"
    );

    let mut none_only = backup_list(vec![reapack()]);
    apply_backup_checkbox(&mut none_only.rows, 1, BackupChoice::Delete, true);
    let (_, body) = delete_backups_question(&en(), &none_only);
    assert_eq!(
        body,
        "RABBIT deletes one backup and frees 1 MB. A deleted backup can't be brought back. \
         Delete now?"
    );
}

// ---------------------------------------------------------------------------
// Every language.
// ---------------------------------------------------------------------------

#[test]
fn every_restore_refusal_reads_in_every_locale() {
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for refusal in sample_refusals() {
            let key = format!("wizard-restore-refusal-{}", refusal.code());
            assert!(!localizer.text(&key).missing, "{locale}: {key}");
            let text = restore_refusal_text(&localizer, &refusal);
            assert_reads_here(locale, &text);
            if let RestoreRefusal::TargetRefused { reason, .. } = &refusal {
                let key = format!("wizard-restore-target-{}", reason.code());
                assert!(!localizer.text(&key).missing, "{locale}: {key}");
            }
        }
        // As Review says them, for a backup and for the whole run.
        let mut list = list_in(&localizer, vec![sws()]);
        apply_backup_checkbox(&mut list.rows, 1, BackupChoice::Restore, true);
        let items = backup_restore_items(&list);
        for refusal in sample_refusals() {
            let mut plan = planned(&items[0]);
            plan.refusal = Some(refusal.clone());
            let review = build_backup_restore_review(
                &localizer,
                Some(&target(true)),
                &list,
                Some(&quick_plan(vec![plan], Vec::new())),
                &[],
            );
            assert!(!review.can_restore);
            review
                .lines
                .iter()
                .filter(|line| !line.is_empty())
                .for_each(|line| assert_reads_here(locale, line));
            let review = build_backup_restore_review(
                &localizer,
                Some(&target(true)),
                &list,
                Some(&RestoreQuickPlan::Refused(refusal)),
                &[],
            );
            assert!(!review.can_restore);
            review
                .lines
                .iter()
                .filter(|line| !line.is_empty())
                .for_each(|line| assert_reads_here(locale, line));
        }
    }
}

/// One backup of each kind the tab words differently, so every row,
/// details and Review sentence is formatted, and the sources' targets,
/// where two key maps write the same file.
fn every_kind_of_backup() -> BackupListSource {
    let deleting = BackupEntry {
        restore_scope: None,
        ..backup(
            BackupItem::Deleting {
                set_id: "uninstall-unix-1".to_string(),
            },
            BackupGroup::Other,
            Some(OCT_2_1100),
        )
    };
    let undated_keymap = BackupEntry {
        item: BackupItem::Keymap {
            set_id: "unattended-unix-1-000000001".to_string(),
        },
        created_unix: None,
        ..keymap()
    };
    let kinds: Vec<BackupEntry> = [
        BackupSetKind::NoManifest,
        BackupSetKind::Damaged,
        BackupSetKind::BeforeRestore,
        BackupSetKind::Update,
        BackupSetKind::Uninstall,
        BackupSetKind::Keymap,
        BackupSetKind::Unknown,
        BackupSetKind::Records,
    ]
    .into_iter()
    .enumerate()
    .map(|(n, set_kind)| BackupEntry {
        set_kind,
        ..backup(
            BackupItem::Set {
                set_id: format!("other-{n}"),
            },
            BackupGroup::Other,
            Some(OCT_2_1100),
        )
    })
    .collect();
    let key_maps: Vec<BackupEntry> = [
        (Some(KeymapOrigin::DefaultKeys), BackupSetKind::Keymap),
        (Some(KeymapOrigin::UnchangedOsara), BackupSetKind::Keymap),
        (Some(KeymapOrigin::Unknown), BackupSetKind::Keymap),
        (None, BackupSetKind::Update),
    ]
    .into_iter()
    .enumerate()
    .map(|(n, (keymap_origin, set_kind))| BackupEntry {
        item: BackupItem::Keymap {
            set_id: format!("keymap-{n}"),
        },
        keymap_origin,
        set_kind,
        ..keymap()
    })
    .collect();
    // A restore's backup of a version, with the key map it replaced.
    let restored_version = BackupEntry {
        item: package("restore-unix-1-before-restore", "reakontrol"),
        set_kind: BackupSetKind::BeforeRestore,
        ..reapack()
    };
    let restored_keymap = BackupEntry {
        item: BackupItem::Keymap {
            set_id: "restore-unix-1-before-restore".to_string(),
        },
        set_kind: BackupSetKind::BeforeRestore,
        keymap_origin: None,
        created_unix: Some(NOV_3_0912),
        ..keymap()
    };
    // Earlier versions without the version that replaced them, or without
    // their own.
    let restored_unversioned = BackupEntry {
        item: package("restore-unix-5-before-restore", "reapack"),
        set_kind: BackupSetKind::BeforeRestore,
        replaced_by: None,
        ..reapack()
    };
    let update_unreplaced = BackupEntry {
        item: package("unix-6", "osara"),
        replaced_by: None,
        ..reapack()
    };
    let copy_version = BackupEntry {
        item: package("unix-7", "langpack-es"),
        version: None,
        ..reapack()
    };
    // A second package of the update ReaPack's backup is from.
    let update_sws = BackupEntry {
        item: package("unix-1793697120", "sws"),
        version: Some(version("2.13")),
        ..reapack()
    };
    // Two packages of an uninstall without a date, one from before RABBIT
    // recorded the settings.
    let undated_reakontrol = BackupEntry {
        with_settings: None,
        item: package("uninstall-unix-2", "reakontrol"),
        created_unix: None,
        ..sws()
    };
    let undated_sws = BackupEntry {
        item: package("uninstall-unix-2", "sws"),
        created_unix: None,
        ..sws()
    };
    let unversioned = BackupEntry {
        item: package("uninstall-unix-8", "reapack"),
        version: None,
        ..sws()
    };
    let already_back = with_status(
        BackupEntry {
            item: package("uninstall-unix-3", "langpack-de"),
            ..sws()
        },
        BackupStatus::AlreadyBack,
    );
    let already_back_earlier = with_status(
        BackupEntry {
            item: package("unix-4", "langpack-fr"),
            ..reapack()
        },
        BackupStatus::AlreadyBack,
    );
    let no_scope = BackupEntry {
        restore_scope: None,
        item: package("uninstall-unix-9", "osara"),
        ..sws()
    };
    let mut entries = vec![
        sws(),
        ffmpeg(),
        reapack(),
        update_sws,
        keymap(),
        records(),
        deleting,
        undated_keymap.clone(),
        restored_version,
        restored_keymap,
        restored_unversioned,
        update_unreplaced,
        copy_version,
        undated_reakontrol,
        undated_sws,
        unversioned,
        already_back,
        already_back_earlier,
        no_scope,
    ];
    entries.extend(kinds);
    entries.extend(key_maps);
    let mut source = source(entries);
    let key_map_file = PathBuf::from("C:/REAPER/reaper-kb.ini");
    source
        .targets
        .insert(keymap().item, vec![key_map_file.clone()]);
    source
        .targets
        .insert(undated_keymap.item, vec![key_map_file]);
    source
}

/// A quick plan of every backup ticked to restore in `list`, each as
/// `change` makes it, with `checks`.
fn plan_each(
    list: &WizardBackupList,
    checks: Vec<PreflightCheck>,
    change: &dyn Fn(&mut RestoreReportItem),
) -> RestoreQuickPlan {
    let backups = backup_restore_items(list)
        .iter()
        .map(|item| {
            let mut plan = planned(item);
            change(&mut plan);
            plan
        })
        .collect();
    quick_plan(backups, checks)
}

fn language(restored: bool) -> RestoredPackage {
    RestoredPackage {
        package_id: "sws".to_string(),
        receipt_merged: true,
        receipt_already_current: false,
        langpack_before: Some("de_DE.ReaperLangPack".to_string()),
        langpack_restored: restored,
        langpack_error: None,
    }
}

#[test]
fn every_row_review_and_question_reads_in_every_locale() {
    let source = every_kind_of_backup();
    let is = |plan: &RestoreReportItem, set_id: &str, package_id: &str| {
        plan.backup_id == set_id && plan.package_id.as_deref() == Some(package_id)
    };
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        let mut said: Vec<String> = Vec::new();
        let mut list = backup_list_from_source(&localizer, &source, &utc);
        said.push(list.list_label(&localizer));
        for index in 0..list.rows.len() {
            apply_backup_checkbox(&mut list.rows, index, BackupChoice::Restore, true);
        }
        set_backup_choice(&mut list, BackupChoice::Delete);
        for index in 0..list.rows.len() {
            let change = apply_backup_checkbox(&mut list.rows, index, BackupChoice::Delete, true);
            said.extend(backups_also_ticked_announcement(
                &localizer, &list.rows, index, &change,
            ));
        }
        for row in &list.rows {
            said.push(row.summary.clone());
            said.push(row.details.clone());
        }
        said.push(backups_ticked_announcement(&localizer, &list));
        let delete = build_backup_delete_review(&localizer, Some(&target(true)), &list, &[]);
        assert!(delete.can_delete);
        said.push(delete.heading);
        said.extend(delete.lines.into_iter().filter(|line| !line.is_empty()));
        let (title, body) = delete_backups_question(&localizer, &list);
        said.push(title);
        said.push(body);

        let first_deleted = list.rows.iter().position(|row| row.delete_ticked).unwrap();
        let change =
            apply_backup_checkbox(&mut list.rows, first_deleted, BackupChoice::Delete, false);
        said.extend(backups_also_ticked_announcement(
            &localizer,
            &list.rows,
            first_deleted,
            &change,
        ));
        set_backup_choice(&mut list, BackupChoice::Restore);
        for row in &list.rows {
            said.push(row.summary.clone());
        }
        said.push(backups_ticked_announcement(&localizer, &list));

        // Restore's Review, from four quick plans: one that says the
        // language goes back, a version goes back from one to another, an
        // item couldn't be checked and REAPER is open; one that says the
        // language stays and every REAPER is open; one that failed; none.
        let first = plan_each(
            &list,
            vec![check(REAPER_PROCESS_CHECK, PreflightStatus::Fail)],
            &|plan| {
                if is(plan, "uninstall-unix-1794233100", "sws") {
                    plan.packages.push(language(true));
                    plan.swap = Some(SwappedPackage {
                        entry: SwappedEntry::Removed,
                        uninstall_needs_reinstall: true,
                        ..swap("ffmpeg", None, Some("7.1"))
                    });
                }
                if is(plan, "unix-1793697120", "reapack") {
                    plan.swap = Some(swap("reapack", Some("1.2.4"), Some("1.2.5")));
                }
                if is(plan, "unix-6", "osara") {
                    plan.status = RestoreStatus::NotRestored;
                    plan.error = Some("access denied".to_string());
                }
            },
        );
        let second = plan_each(
            &list,
            vec![check(ANY_REAPER_PROCESS_CHECK, PreflightStatus::Warn)],
            &|plan| {
                if is(plan, "uninstall-unix-1794233100", "sws") {
                    plan.packages.push(language(false));
                }
            },
        );
        let failed = RestoreQuickPlan::Failed("disk on fire".to_string());
        let notes = ["What is ticked on the Uninstall tab isn't part of this run.".to_string()];
        for (target_row, plan) in [
            (target(false), Some(&first)),
            (target(true), Some(&second)),
            (target(true), Some(&failed)),
            (target(true), None),
        ] {
            let review =
                build_backup_restore_review(&localizer, Some(&target_row), &list, plan, &notes);
            assert!(!review.can_restore, "{locale}: {:#?}", review.lines);
            said.push(review.heading);
            said.extend(review.lines.into_iter().filter(|line| !line.is_empty()));
        }

        // Delete's Review with what is ticked to restore and on other tabs.
        set_backup_choice(&mut list, BackupChoice::Delete);
        let delete = build_backup_delete_review(&localizer, Some(&target(true)), &list, &notes);
        said.extend(delete.lines.into_iter().filter(|line| !line.is_empty()));

        let empty = list_in(&localizer, Vec::new());
        let (label, details) = empty.placeholder(&localizer).unwrap();
        said.extend([label, details, empty.list_label(&localizer)]);
        let (label, details) =
            backup_list_unreadable(&RabbitError::RestoreRefused(RestoreRefusal::AsRoot))
                .placeholder(&localizer)
                .unwrap();
        said.extend([label, details]);
        said.push(localizer.text("wizard-packages-tab-backups").value);
        for single in [1, 2] {
            let mut few = list_in(&localizer, vec![keymap(), sws(), ffmpeg()]);
            for index in 1..=single {
                apply_backup_checkbox(&mut few.rows, index, BackupChoice::Delete, true);
            }
            said.push(delete_backups_question(&localizer, &few).1);
            said.extend(
                build_backup_delete_review(&localizer, Some(&target(true)), &few, &[])
                    .lines
                    .into_iter()
                    .filter(|line| !line.is_empty()),
            );
        }

        for text in &said {
            assert_reads_here(locale, text);
        }

        // In English, that the fixture reaches every form, so the other
        // languages are formatted in each of them too.
        if locale == DEFAULT_LOCALE {
            let all = said.join("\n");
            for reached in [
                // Rows and details.
                "Replaced on November 3, 2026, 9:12 AM",
                "A restore replaced it on",
                "(not available: RABBIT can't restore it)",
                "\nRABBIT can't restore it.\n",
                "\nRestoring puts them back.\n",
                "is at this version already. Restoring puts its files back again.",
                "Restoring puts this copy back and keeps the version you have now as a backup",
                // The tie announcements.
                "from the same uninstall is ticked too.",
                "from the same uninstall is unticked too.",
                // Restore's Review.
                "goes back from 1.2.5 to 1.2.4.",
                "ReaKontrol goes back to 1.2.4. RABBIT keeps the version you have now",
                "goes back to the copy an update replaced.",
                "REAPER's language goes back to de_DE.ReaperLangPack.",
                "REAPER's language stays as it is",
                "RABBIT no longer counts FFmpeg for Improved Video Support as installed by it.",
                "Close REAPER first.",
                "Close every copy of REAPER first.",
                "backups ticked to delete aren't part of this run.",
                "backups ticked to restore aren't part of this run.",
                "What is ticked on the Uninstall tab isn't part of this run.",
                "both put back the same file. Tick only one of them.",
                "are both backups of SWS Extension. Tick only one of them.",
                "RABBIT couldn't check it (access denied).",
                "RABBIT couldn't check the restore (disk on fire).",
                "RABBIT can't write to this REAPER's folder.",
                "RABBIT hasn't checked what is ticked.",
                // Delete's Review.
                ": its files are on disk as well, so nothing is lost.",
                ": it is gone for good.",
                ": this finishes a deletion that started earlier.",
                ": nothing that is installed changes.",
                "ReaKontrol 2.14.0.7: the only copy.",
                ", uninstalled November 9, 2026, 2:05 PM, go together",
                ", replaced November 3, 2026, 9:12 AM, go together",
                ", from November 3, 2026, 9:12 AM, go together",
                "ReaKontrol 2.14.0.7 and SWS Extension 2.14.0.7 go together",
            ] {
                assert!(all.contains(reached), "not reached: {reached}\n{all}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The tab shell: the choice's RadioBox, the list's shape, the quick plan
// before Review, and the notes on the other tabs.
// ---------------------------------------------------------------------------

#[test]
fn the_choice_below_the_list_maps_to_and_from_its_radio_box() {
    assert_eq!(
        BackupChoice::ALL,
        [BackupChoice::Restore, BackupChoice::Delete]
    );
    for (index, choice) in BackupChoice::ALL.into_iter().enumerate() {
        assert_eq!(choice.selection(), index as i32);
        assert_eq!(BackupChoice::from_selection(index as i32), choice);
    }
    // No selection, or one past the choices: Restore, where the tab starts.
    for selection in [-1, 2, 7] {
        assert_eq!(
            BackupChoice::from_selection(selection),
            BackupChoice::Restore
        );
    }
    assert_eq!(BackupChoice::default(), BackupChoice::Restore);
}

#[test]
fn rows_sit_under_the_heading_before_them() {
    let list = backup_list(vec![sws(), ffmpeg(), reapack(), keymap()]);
    let headings = list.heading_indices();
    let names = |indices: &[usize]| -> Vec<String> {
        indices
            .iter()
            .map(|&index| list.rows[index].name.clone())
            .collect()
    };
    assert_eq!(
        names(&headings),
        ["Software you uninstalled", "Earlier versions", "Key maps"]
    );
    for heading in &headings {
        assert_eq!(list.heading_of(*heading), None);
        for row in list.rows_under(*heading) {
            assert!(!list.rows[row].is_heading());
            assert_eq!(list.heading_of(row), Some(*heading));
        }
    }
    // Every row is under exactly one heading.
    let under: usize = headings
        .iter()
        .map(|heading| list.rows_under(*heading).len())
        .sum();
    assert_eq!(under + headings.len(), list.rows.len());
    assert_eq!(names(&list.rows_under(headings[1])), ["ReaPack 1.2.4"]);
    // A row is no heading, and an index past the end has nothing.
    let sws = index_of(&list, "SWS Extension 2.14.0.7");
    assert!(list.rows_under(sws).is_empty());
    assert_eq!(list.heading_of(list.rows.len()), None);
    assert!(list.rows_under(list.rows.len()).is_empty());
}

#[test]
fn the_tab_starts_with_the_first_rows_details_or_the_placeholders() {
    let list = backup_list(vec![sws()]);
    assert_eq!(list.initial_details(&en()), list.rows[0].details);
    assert!(list.rows[0].is_heading());

    let empty = backup_list(Vec::new());
    assert_eq!(
        empty.initial_details(&en()),
        empty.placeholder(&en()).unwrap().1
    );
    let unreadable = backup_list_unreadable(&RabbitError::RestoreRefused(RestoreRefusal::IsLink {
        path: PathBuf::from("C:/REAPER/RABBIT/backups"),
    }));
    assert!(
        unreadable
            .initial_details(&en())
            .starts_with("RABBIT couldn't read its backups folder ("),
        "{}",
        unreadable.initial_details(&en())
    );
}

/// Every file and folder under `root`, with each file's content, to tell
/// that something changed nothing.
fn snapshot(root: &std::path::Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if fs::symlink_metadata(&path).unwrap().is_dir() {
                pending.push(path);
                found.push((relative, None));
            } else {
                found.push((relative, Some(fs::read(&path).unwrap())));
            }
        }
    }
    found.sort();
    found
}

// The three quick-plan tests below run `backup_quick_plan` as the wizard
// does, through `restore_backups`, which always asks this computer what
// REAPER and other programs run and where its standard REAPER is, unlike
// rabbit-core's own restore tests. They only read, and the only files they
// touch are in their own temporary folder, but the answers differ between
// computers: a REAPER running here adds Review's "Close REAPER first."
// note. So they must never assert on notes about REAPER or other programs,
// or on the preflight checks.

#[test]
fn the_quick_plan_runs_only_with_something_ticked_to_restore() {
    let dir = tempdir().unwrap();
    write_uninstall_set(dir.path());
    let mut list = backup_list_at(&en(), dir.path());
    assert_eq!(backup_quick_plan(dir.path(), &list), None);
    // Ticks for Delete don't count.
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Delete);
    assert_eq!(backup_quick_plan(dir.path(), &list), None);
}

#[test]
fn the_quick_plan_of_a_real_set_lets_review_restore_and_changes_nothing() {
    let dir = tempdir().unwrap();
    let resource = dir.path();
    write_uninstall_set(resource);
    let mut list = backup_list_at(&en(), resource);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    let before = snapshot(resource);

    let plan = backup_quick_plan(resource, &list).unwrap();
    assert_eq!(snapshot(resource), before);
    // No lock, no report and no before-restore set.
    assert!(!resource.join("RABBIT").join("logs").exists());
    let RestoreQuickPlan::Planned(report) = &plan else {
        panic!("{plan:?}");
    };
    assert!(report.dry_run && report.quick);
    assert_eq!(report.backups.len(), 1);
    assert_eq!(report.backups[0].package_id.as_deref(), Some("sws"));
    assert_eq!(report.backups[0].status, RestoreStatus::WouldRestore);

    let target = TargetRow {
        path: resource.to_path_buf(),
        ..target(true)
    };
    let review = build_backup_restore_review(&en(), Some(&target), &list, Some(&plan), &[]);
    assert!(review.can_restore, "{:#?}", review.lines);
    assert_eq!(review.items, backup_restore_items(&list));
}

#[test]
fn a_quick_plan_of_a_set_gone_since_listing_blocks_review() {
    let dir = tempdir().unwrap();
    let resource = dir.path();
    write_uninstall_set(resource);
    let mut list = backup_list_at(&en(), resource);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    // Gone since the tab listed it, as after a delete by hand. Only this
    // test's own temporary folder.
    fs::remove_dir_all(
        resource
            .join("RABBIT")
            .join("backups")
            .join("uninstall-unix-1794233100"),
    )
    .unwrap();

    let plan = backup_quick_plan(resource, &list).unwrap();
    let target = TargetRow {
        path: resource.to_path_buf(),
        ..target(true)
    };
    let review = build_backup_restore_review(&en(), Some(&target), &list, Some(&plan), &[]);
    assert!(!review.can_restore);
    assert!(
        review
            .lines
            .iter()
            .any(|line| line.starts_with("Can't restore SWS Extension 2.14.0.7 (")),
        "{:#?}",
        review.lines
    );
}

#[test]
fn both_backup_reviews_name_the_ticks_on_the_other_two_tabs() {
    let ticks = PackagesTabTicks {
        install: vec!["OSARA".to_string()],
        uninstall: vec!["SWS Extension".to_string()],
        backups_to_restore: 1,
        backups_to_delete: 1,
    };
    let mut list = backup_list(vec![sws(), reapack()]);
    tick(&mut list, "SWS Extension 2.14.0.7", BackupChoice::Restore);
    tick(&mut list, "ReaPack 1.2.4", BackupChoice::Delete);
    let install = "What is ticked on the Install or update tab isn't part of this run: OSARA.";
    let uninstall = "What is ticked on the Uninstall tab isn't part of this run: SWS Extension.";

    let notes = inactive_tab_notes(&en(), WizardMode::Restore, &ticks);
    let restore = build_backup_restore_review(
        &en(),
        Some(&target(true)),
        &list,
        Some(&plan_all(&list)),
        &notes,
    );
    let notes = inactive_tab_notes(&en(), WizardMode::DeleteBackups, &ticks);
    let delete = build_backup_delete_review(&en(), Some(&target(true)), &list, &notes);
    for lines in [&restore.lines, &delete.lines] {
        assert!(lines.iter().any(|line| line == install), "{lines:#?}");
        assert!(lines.iter().any(|line| line == uninstall), "{lines:#?}");
        // The Backups tab's own ticks are said by the Review itself, for the
        // other choice, not as another tab's.
        assert!(
            !lines.iter().any(|line| line.contains("Backups tab")),
            "{lines:#?}"
        );
    }
}
