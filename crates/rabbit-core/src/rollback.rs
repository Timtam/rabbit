use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::backup_overview::SetFingerprint;
use crate::cancel::CancelToken;
use crate::error::{IoPathContext, JsonPathContext, RabbitError, Result};
use crate::hash::sha256_file;
use crate::model::{InstallationKind, Platform};
use crate::package::PackageSpec;
use crate::preflight::{
    BACKUP_FOLDER_CHECK, PreflightCheck, PreflightOptions, PreflightReport, PreflightStatus,
    REAPER_PROCESS_CHECK, RunningProcess, is_one_of, run_restore_preflight_with_processes,
    running_processes_named, running_reaper_for_target_with_processes, running_reaper_processes,
};
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::reaper_ini::{lang_pack_setting, restore_lang_pack_if_cleared};
use crate::receipt::{
    InstallState, PackageReceipt, RECEIPT_RELATIVE_PATH, load_install_state, merge_package_receipt,
    save_install_state,
};
use crate::report::{SavedReportPaths, default_report_path, save_new_json_and_text_reports};
use crate::restore_refusal::{RestoreRefusal, TargetRefusal};
use crate::uninstall::paths::{ResolvedPattern, pattern_matches};
use crate::uninstall::preflight::{
    ANY_REAPER_PROCESS_CHECK, NO_REAPER_ANYWHERE, PROGRAMS_RUNNING_CHECK,
    none_of_these_programs_running,
};
use crate::version::Version;

pub const BACKUP_MANIFEST_FILE: &str = "backup-manifest.json";

/// Files from outside the resource folder go under this folder of a set,
/// one numbered subfolder each, so two outside files with the same name
/// never collide.
const EXTERNAL_DIR: &str = "external";

/// Suffix of the note beside each outside file in a set. The note holds the
/// file's original absolute path, so a person can still put the file back
/// if the manifest is ever lost.
pub const ORIGIN_SIDECAR_SUFFIX: &str = ".rabbit-origin";

/// The manifest is written here first and then renamed over the real one.
const MANIFEST_STAGING_FILE: &str = "backup-manifest.json.rabbit-tmp";

/// The reason recorded for the set that holds the files a restore replaced.
pub(crate) const BEFORE_RESTORE_REASON: &str = "before-restore";

/// The reason recorded for the set that holds the files an update replaced,
/// with what it replaced of each package (see [`BackupManifest::replaced`]).
/// Earlier versions put a copy of RABBIT's whole receipt from before the
/// update there instead.
pub(crate) const UPDATE_BACKUP_REASON: &str = "install-replacement";

/// The reason recorded for a set that holds only a copy of RABBIT's
/// receipt. Earlier versions made one after vendor installers ran; RABBIT
/// makes none any more, and a restore refuses the old ones.
pub(crate) const RECORDS_BACKUP_REASON: &str = "unattended-receipt-update";

/// The reason recorded for the set that holds `reaper-kb.ini` from just
/// before OSARA's key map replaced it.
pub(crate) const KEYMAP_BACKUP_REASON: &str = "osara-keymap-replacement";

/// The folder beside `RABBIT/backups` where delete (still to come) moves a
/// set before it removes anything, so a restore never finds half a set.
/// Restore and the overview only look here, and never create it.
pub(crate) const DELETING_DIR: &str = "backups-deleting";

/// One backup set as `rabbit backups --json` prints it, built from the
/// overview (see [`crate::backup_overview`]). The fields after `warning`
/// came with the overview; the ones before keep their meaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSet {
    pub id: String,
    /// The set's folder: in `RABBIT/backups`, or in
    /// `RABBIT/backups-deleting` for an unfinished deletion.
    pub path: PathBuf,
    pub created_at: Option<String>,
    pub reason: Option<String>,
    pub manifest_path: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    /// Set when part of this set could not be read, most often a damaged
    /// manifest; when the entry or something in it is a symbolic link or
    /// junction, which RABBIT never follows; or when its name is not one
    /// RABBIT gives a backup. The set is still listed. Restoring it may be
    /// refused, with the full error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// What made the set, as the overview reads it. Always there in what
    /// this version prints; `None` only in JSON from earlier versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::backup_overview::BackupSetKind>,
    /// When the set was made, in seconds since 1970 (UTC), when RABBIT can
    /// tell: from the manifest, or from the set's name.
    #[serde(default)]
    pub created_unix: Option<u64>,
    /// Bytes on disk, every file and link in the set counted, none
    /// followed: what deleting it frees.
    #[serde(default)]
    pub size: u64,
    /// The backups this set holds, as the overview lists them.
    #[serde(default)]
    pub items: Vec<crate::backup_overview::BackupEntry>,
    /// What a later run compares to tell whether the set changed since it
    /// was listed. `None` for an entry that is no backup, which is never
    /// looked into.
    #[serde(default)]
    pub fingerprint: Option<crate::backup_overview::SetFingerprint>,
}

/// A set's `backup-manifest.json`. Fields added since the first version
/// are optional in the file, so older manifests still load, and
/// `schema_version` stays 1: older versions ignore fields they don't know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub rabbit_version: String,
    /// The id RABBIT asked for when it made the set. The folder is named
    /// `<id>-2` and so on when a set already had that id.
    pub created_at: String,
    /// When the set was made, in seconds since 1970 (UTC). `None` in sets
    /// from earlier versions, whose only date is the one in their id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_unix: Option<u64>,
    pub reason: String,
    pub files: Vec<BackupManifestFile>,
    /// Where a copy of RABBIT's whole receipt is in the set. Only earlier
    /// versions made one: in an update's set, and in a set of its own after
    /// vendor installers ran. A restore never copies it back.
    pub receipt_backup_path: Option<PathBuf>,
    /// The packages an uninstall removed into this set, by id. Empty, and
    /// left out of the file, for every other kind of set.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub packages: BTreeMap<String, BackupPackageRecord>,
    /// For a set that keeps files something replaced: each package whose
    /// files it holds, by id, with what RABBIT's receipt said about it
    /// before and the version that replaced it. An update's set records it
    /// now, and a before-restore set may too (see [`BackupSetWriter::record_replaced`]).
    /// Not in `packages`: older versions read every record there as an
    /// uninstall's, whatever the set's reason, and would merge the old entry
    /// back while only the replaced files came back. Empty, and left out of
    /// the file, in uninstall and key map sets, and in every set from
    /// earlier versions; their update sets hold a copy of the whole receipt
    /// instead.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub replaced: BTreeMap<String, ReplacedPackageRecord>,
    /// For the set that holds `reaper-kb.ini` from before OSARA's key map
    /// replaced it: what that key map was, as RABBIT read it just before
    /// OSARA's install. `None` for every other kind of set, and in key map
    /// sets from earlier versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keymap_origin: Option<KeymapOrigin>,
}

/// What an uninstall set records about one package it removed, so a later
/// restore of just that package can put it back properly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupPackageRecord {
    /// The package's receipt entry from just before the uninstall. A restore
    /// merges it back into the receipt instead of copying the whole receipt
    /// file, which has moved on since. `None` when RABBIT had no receipt for
    /// the copy it removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<PackageReceipt>,
    /// The `langpack=` value under `[REAPER]` in `reaper.ini` that the
    /// uninstall replaced with `<>`, because it named a language pack file
    /// the uninstall removed. `reaper.ini` itself never goes into a set:
    /// restoring a whole copy would undo every REAPER setting changed since,
    /// so a restore puts back only this one line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub langpack_before: Option<String>,
    /// Whether the uninstall moved the package's settings into the set, as
    /// "Also remove settings" and `--remove-settings` ask. `false` when it
    /// moved none, also when they were asked for and there were none to
    /// move. `None` in sets from earlier versions, which didn't record it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_settings: Option<bool>,
}

/// What a set records about one package whose files something replaced:
/// an update, in its `install-replacement` set. The record is saved before
/// the first of the package's files is copied into the set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacedPackageRecord {
    /// The package's receipt entry from just before its files were
    /// replaced. `None` when the receipt had none: the copy replaced wasn't
    /// one RABBIT installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<PackageReceipt>,
    /// The version that replaced it, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<Version>,
}

/// What `reaper-kb.ini` was before OSARA's key map replaced it, as RABBIT
/// read it just before OSARA's install (see
/// [`crate::osara_keymap::OsaraKeymapState`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeymapOrigin {
    /// The user's own key map: it differed from the OSARA key map installed
    /// then.
    OwnKeymap,
    /// OSARA's key map, unchanged apart from script registrations.
    UnchangedOsara,
    /// REAPER's default keys: a `reaper-kb.ini` without key bindings or
    /// custom actions. It still goes into a set, because the file exists.
    DefaultKeys,
    /// RABBIT couldn't tell: there was no OSARA key map to compare it with,
    /// or one of the two couldn't be read. A value a later version wrote,
    /// which this one doesn't know, reads as this too, so its manifest
    /// still loads.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifestFile {
    pub package_id: Option<String>,
    /// Where the file came from: relative to the resource folder for files
    /// inside it, absolute for files outside it. Sets written by older
    /// RABBIT versions also hold absolute paths for files inside it.
    pub original_path: PathBuf,
    /// Where the copy is, always `<resource>/RABBIT/backups/<id>/...`. This
    /// version records it absolute. Earlier ones recorded it relative to the
    /// folder RABBIT was started in when `--resource-path` was relative, so
    /// restore finds the copy by the `RABBIT/backups/<id>` part either way.
    pub backup_path: PathBuf,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub state: BackupEntryState,
}

/// How far the copy an entry describes got. The entry is saved before the
/// copy starts, so a run that dies halfway still leaves a record of every
/// file it touched.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupEntryState {
    /// Saved before the copy started. The copy may be missing or partial.
    Pending,
    /// The copy finished and matched the original. Manifests from before
    /// this field existed were written after every copy, so they read as
    /// done.
    #[default]
    Done,
}

/// A file's size and SHA-256, when the caller has just read them and the
/// writer needn't read the file again. FFmpeg's DLLs are 250 MB together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDigest {
    pub size: u64,
    pub sha256: String,
}

impl FileDigest {
    pub fn of(path: &Path) -> Result<Self> {
        Ok(Self {
            size: fs::metadata(path).with_path(path)?.len(),
            sha256: sha256_file(path)?,
        })
    }
}

/// A file [`BackupSetWriter::move_file`] moved into a set, with what
/// [`BackupSetWriter::put_back`] needs to move it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedFile {
    pub original: PathBuf,
    pub backup_path: PathBuf,
    pub size: u64,
    pub sha256: String,
    /// It went to another volume as a checked copy rather than a rename, so
    /// it comes back the same way.
    pub copied: bool,
}

/// Why [`BackupSetWriter::move_file`] didn't move a file. Either way the
/// file is still where it was and the set doesn't claim it.
#[derive(Debug, thiserror::Error)]
pub enum MoveFileError {
    #[error(transparent)]
    NotMoved(RabbitError),
    /// Another program has the file open. On the set's own volume, Windows
    /// refused to rename it. On another volume, the copy was made and
    /// checked, but the original couldn't be deleted, so the copy was
    /// removed again.
    #[error("{} is in use by another program: {source}", path.display())]
    InUse {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreBackupOptions {
    pub dry_run: bool,
    pub allow_reaper_running: bool,
    /// Restore only this package, from a set an uninstall wrote. `None`
    /// restores the whole set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreBackupReport {
    pub resource_path: PathBuf,
    pub backup_id: String,
    pub backup_path: PathBuf,
    pub dry_run: bool,
    pub preflight: PreflightReport,
    /// Whether the backup came back. A restore is all or nothing: one that
    /// failed partway put back what it had changed, as far as it could,
    /// and each action says how its file ended up.
    pub status: RestoreStatus,
    /// Why the backup wasn't restored, when a file or RABBIT's records
    /// failed partway. A refusal before anything changed is an error of
    /// [`restore_backup_set`] instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub actions: Vec<RestoreBackupAction>,
    /// Files whose backup never finished. The run that started it stopped
    /// before it changed them, so there is nothing to put back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_targets: Vec<PathBuf>,
    /// The one package restored, when the restore asked for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    /// For a set an uninstall wrote: what the restore did for each package
    /// besides putting its files back, or in a dry run would do.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<RestoredPackage>,
    /// For a set an uninstall wrote: the packages it records that a restore
    /// of the whole set left out, and why. Their receipt entries,
    /// `declined_packages` and REAPER's language stay as they are.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_packages: Vec<SkippedPackage>,
    /// The set this restore wrote, in `RABBIT/backups`, with the files it
    /// replaced as they were before it. Restoring that set puts them back.
    /// `None` in a dry run, when the restore replaced no file, and when it
    /// failed and put every file back, which leaves no set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_restore_set_id: Option<String>,
    /// Where the restore saved this report, when it saved it: a real
    /// restore that gets past its checks always tries, also when the backup
    /// wasn't restored; a dry run never does.
    #[serde(skip)]
    pub saved_report: Option<SavedReportPaths>,
}

/// What came of one backup a restore brought back, or in a dry run would.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestoreStatus {
    /// Every file is back, and so are the backup's records.
    Restored,
    /// A dry run: the backup would be restored.
    WouldRestore,
    /// The backup wasn't restored, or in a dry run wouldn't be, and
    /// everything of it is as it was: a check refused it before anything
    /// changed, or a file or RABBIT's records failed partway and every file
    /// it had replaced or added was put back as it was.
    NotRestored,
    /// The restore of the backup failed partway, and some files couldn't
    /// be put back as they were: those marked
    /// [`RestoreBackupActionKind::Left`] or
    /// [`RestoreBackupActionKind::Missing`]. What they replaced is in the
    /// before-restore set, which stays. RABBIT's records stay as they were.
    /// No stop does this: a stop only keeps the run from backups it hasn't
    /// reached.
    FilesLeft,
    /// The user stopped the run before it got to this backup, which stays
    /// as it was.
    NotReached,
}

/// One backup for [`restore_backups`]: a set in `RABBIT/backups`, named by
/// its id exactly as listed, upper and lower case included, as a whole or
/// one package of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreItem {
    pub set_id: String,
    /// Only this package of the set, as `restore-backup --package` restores
    /// it. `None` restores the whole set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    /// The set's fingerprint as the caller listed it (see
    /// [`crate::backup_overview::BackupSetOverview::fingerprint`]). A set
    /// that changed since is refused, with
    /// [`RestoreRefusal::ChangedSinceListed`], before anything of it
    /// changes. `None` checks nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<SetFingerprint>,
}

/// How [`restore_backups`] runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreRunOptions {
    pub dry_run: bool,
    /// With `dry_run`, the quick plan: no file's content is read or hashed,
    /// as for the wizard's Review. A copy counts as there with the size its
    /// entry recorded, as the overview of the backups judges it, so only a
    /// copy whose content changed at the same size goes unnoticed. A real
    /// run ignores it and hashes every copy before it puts one back.
    #[serde(default)]
    pub quick: bool,
    pub allow_reaper_running: bool,
}

impl RestoreRunOptions {
    /// How the run's plan reads the copies in a set.
    fn copy_check(self) -> CopyCheck {
        if self.dry_run && self.quick {
            CopyCheck::Size
        } else {
            CopyCheck::Hash
        }
    }

    /// The options of a restore of one backup in this run, of only
    /// `package_id` when one is given.
    fn for_backup(self, package_id: Option<&str>) -> RestoreBackupOptions {
        RestoreBackupOptions {
            dry_run: self.dry_run,
            allow_reaper_running: self.allow_reaper_running,
            package_id: package_id.map(ToString::to_string),
        }
    }
}

/// What [`restore_backups`] did, or in a dry run would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreRunReport {
    pub resource_path: PathBuf,
    pub dry_run: bool,
    /// A dry run that read no file's content (see
    /// [`RestoreRunOptions::quick`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quick: bool,
    /// The checks before anything changed, for every backup of the run
    /// together. Empty when a check of its own refused every backup.
    pub preflight: PreflightReport,
    /// One per backup asked for, in the order given.
    pub backups: Vec<RestoreReportItem>,
    /// The user stopped the run before it got to every backup: those it
    /// didn't reach are [`RestoreStatus::NotReached`]. A backup that failed
    /// partway is no stop; its own status says how it ended.
    #[serde(default)]
    pub stopped: bool,
    /// Where the run saved this report: a real run that got past the
    /// checks of the whole run always tries, also when a backup wasn't
    /// restored or the run was stopped. A dry run never does, and neither
    /// does a run in which every backup was refused by its own checks,
    /// which changed nothing.
    #[serde(skip)]
    pub saved_report: Option<SavedReportPaths>,
}

/// One backup of a [`RestoreRunReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReportItem {
    pub backup_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    pub status: RestoreStatus,
    /// The set's folder in `RABBIT/backups`, once the restore found it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<PathBuf>,
    /// Each file, and how it ended up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<RestoreBackupAction>,
    /// See [`RestoreBackupReport::skipped_targets`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_targets: Vec<PathBuf>,
    /// For a set an uninstall wrote: what came back of each package besides
    /// its files, the language line included. Empty when the backup wasn't
    /// restored.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<RestoredPackage>,
    /// See [`RestoreBackupReport::skipped_packages`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_packages: Vec<SkippedPackage>,
    /// The set with the files this backup's restore replaced, as they were
    /// before it. `None` in a dry run, when it replaced nothing, and when
    /// it wasn't restored and put every file back, which leaves no set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_restore_set_id: Option<String>,
    /// Why a check refused the backup before anything of it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RestoreRefusal>,
    /// What went wrong otherwise: an error before anything of it changed,
    /// or the file or the records that failed partway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A package a restore of a whole uninstall set left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedPackage {
    pub package_id: String,
    pub reason: SkippedPackageReason,
}

/// Why a restore left a package out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkippedPackageReason {
    /// None of its files are in the set: the uninstall recorded the package
    /// but found none of them to move, or stopped before any copy of them
    /// finished. Putting back its record alone would make RABBIT count it
    /// as installed with no files there.
    NothingOfItInSet,
}

/// A package a restore brought back from a set an uninstall wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoredPackage {
    pub package_id: String,
    /// The receipt entry the uninstall saved went back into the receipt.
    /// `false` when RABBIT had no receipt for the copy it removed, and when
    /// the entry was still there. Either way the package is off
    /// `declined_packages`.
    pub receipt_merged: bool,
    /// The receipt still held the very entry the uninstall saved, so there
    /// was nothing to merge: the uninstall stopped before it got as far as
    /// the receipt, or an earlier restore from this set put the entry back.
    #[serde(default)]
    pub receipt_already_current: bool,
    /// The `langpack=` value the uninstall replaced with `<>`, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub langpack_before: Option<String>,
    /// Whether that value went back into `reaper.ini`. Only while the line
    /// still says `<>`: a language chosen since is the user's.
    #[serde(default)]
    pub langpack_restored: bool,
    /// Why putting the value back failed. The package's files and receipt
    /// entry are back all the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub langpack_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreBackupAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    /// Where the restore copied the file it replaced, in its before-restore
    /// set. `None` when there was no file to replace, when the file wasn't
    /// reached, and when the restore put everything back and removed that
    /// set again.
    pub current_backup_path: Option<PathBuf>,
    pub action: RestoreBackupActionKind,
    pub size: u64,
    /// The copy's SHA-256. The quick plan reads no file, so it gives the
    /// one the manifest recorded, and none for a set without a manifest.
    pub sha256: String,
    /// Why a file couldn't be put back as it was
    /// ([`RestoreBackupActionKind::Left`] or
    /// [`RestoreBackupActionKind::Missing`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestoreBackupActionKind {
    WouldRestore,
    Restored,
    /// The restore of the backup failed, at this file, a later one or
    /// RABBIT's records, so this file went back as it was before the
    /// restore: the file it had replaced, or no file where it had added
    /// one.
    PutBack,
    /// The restore of the backup failed, at this file, a later one or
    /// RABBIT's records, and this file couldn't be put back as it was: it
    /// is as the backup had it. [`RestoreBackupAction::current_backup_path`]
    /// names the copy of the file it replaced, if it replaced one.
    Left,
    /// The restore of the backup failed at this file after the file it
    /// replaces was removed, and that file couldn't be put back: nothing is
    /// at its place now. [`RestoreBackupAction::current_backup_path`] names
    /// the copy of it.
    Missing,
    /// The restore failed before it changed this file, which is as it was.
    NotRestored,
}

/// Writes one backup set under `RABBIT/backups/<id>/`, keeping its manifest
/// up to date after every step. Updates, OSARA's key map replacement,
/// restore itself and uninstall all write their sets through it, so every
/// set has the same layout and a restore reads them all the same way. Which
/// sets a restore takes is up to the restore (see [`restore_backup_set`]).
///
/// Every set records when it was made (`created_unix`). What else a set
/// records depends on what made it: an uninstall records each package it
/// removes ([`Self::record_package`]), an update what replaced each package
/// ([`Self::record_replaced`]), and OSARA's key map replacement what the
/// key map was ([`Self::set_keymap_origin`]). Each record is saved before
/// the first file it belongs to goes into the set.
///
/// The writer never writes a set behind a symbolic link or junction: while
/// `RABBIT`, `RABBIT/backups` or the set's own folder is one, every record
/// and every file is refused before it is written (see
/// [`Self::start_or_check`]).
///
/// Installs and restore copy files in and leave the originals alone.
/// Uninstall moves them in (see [`BackupSetWriter::move_file`]) and can move
/// them back while its run still goes on.
///
/// Files inside the resource folder keep their relative path in the set.
/// Files outside it (app2clap's CLAP folder) go under
/// `external/<n>/<file name>`, beside a `<file name>.rabbit-origin` note
/// holding the original absolute path.
///
/// Each entry is saved as pending before its copy starts and marked done
/// once the copy matches the original, and each save writes a temp file and
/// renames it over the manifest. A run that dies halfway leaves a whole
/// manifest that names every file it touched, and restore can tell a
/// finished copy from an unfinished one.
///
/// A planning writer (for dry runs) works out the same paths and writes
/// nothing.
#[derive(Debug)]
pub struct BackupSetWriter {
    resource_path: PathBuf,
    set_path: PathBuf,
    manifest: BackupManifest,
    planning: bool,
    started: bool,
}

impl BackupSetWriter {
    /// A writer for the set `set_id`. Nothing is written until the first
    /// file is backed up, so a run that replaces nothing leaves no empty set.
    pub fn new(resource_path: &Path, set_id: &str, reason: &str) -> Self {
        Self::with_mode(resource_path, set_id, reason, false)
    }

    /// A writer that reports where files would go and writes nothing.
    pub fn planning(resource_path: &Path, set_id: &str, reason: &str) -> Self {
        Self::with_mode(resource_path, set_id, reason, true)
    }

    fn with_mode(resource_path: &Path, set_id: &str, reason: &str, planning: bool) -> Self {
        // A relative resource path (`--resource-path .`) would make every
        // path the manifest records relative to the folder RABBIT was
        // started in, and a restore started anywhere else couldn't find the
        // copies. Only an empty path or an unreadable current folder can
        // fail here, and then the path is used as given, as before.
        let resource_path =
            absolute_without_dots(resource_path).unwrap_or_else(|_| resource_path.to_path_buf());
        Self {
            set_path: backups_root(&resource_path).join(set_id),
            resource_path,
            manifest: BackupManifest {
                schema_version: 1,
                rabbit_version: env!("CARGO_PKG_VERSION").to_string(),
                created_at: set_id.to_string(),
                // Set when the set's folder is made (see `start`).
                created_unix: None,
                reason: reason.to_string(),
                files: Vec::new(),
                receipt_backup_path: None,
                packages: BTreeMap::new(),
                replaced: BTreeMap::new(),
                keymap_origin: None,
            },
            planning,
            started: false,
        }
    }

    /// The set's folder. It can change once, on the first write, if a set
    /// with the same id already exists.
    pub fn path(&self) -> &Path {
        &self.set_path
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.set_path.join(BACKUP_MANIFEST_FILE)
    }

    /// The set's id: the name of its folder, which is the id asked for or,
    /// when a set already had that, `<id>-2` and so on.
    pub fn set_id(&self) -> String {
        self.set_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Whether anything has been written yet.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// The SHA-256 the set recorded for the finished copy at `backup_path`.
    pub(crate) fn recorded_sha256(&self, backup_path: &Path) -> Option<String> {
        self.manifest
            .files
            .iter()
            .find(|entry| entry.state == BackupEntryState::Done && entry.backup_path == backup_path)
            .map(|entry| entry.sha256.clone())
    }

    /// Remove the set again after every file it copied in is back where it
    /// came from, as a restore that failed partway leaves it: each copy is
    /// then the same as the file on disk. Only what the manifest names goes,
    /// each copy and an outside file's origin note, then the manifest and
    /// the empty folders. An entry whose copy can't be removed stays, with
    /// the manifest. Nothing is removed while `RABBIT`, `RABBIT/backups` or
    /// the set's folder is a symbolic link or junction, which no writer
    /// follows, and no copy is removed through a folder in the set that has
    /// become one. Returns whether no set is left. Best effort.
    pub(crate) fn discard_copies(&mut self) -> bool {
        if !self.started || self.planning {
            return true;
        }
        if check_backups_folder_is_real(&self.resource_path).is_err()
            || !matches!(is_real_folder(&self.set_path), Ok(true))
        {
            return false;
        }
        let set_path = self.set_path.clone();
        self.manifest.files.retain(|entry| {
            // The writer made every copy path inside its own new folder.
            // The origin note is beside the copy, so one check covers both.
            let Ok(relative) = entry.backup_path.strip_prefix(&set_path) else {
                return true;
            };
            if check_no_link_in_set(&set_path, relative).is_err() {
                return true;
            }
            if entry.original_path.is_absolute()
                && let Some(name) = entry.backup_path.file_name()
            {
                let mut note = name.to_os_string();
                note.push(ORIGIN_SIDECAR_SUFFIX);
                let _ = fs::remove_file(entry.backup_path.with_file_name(note));
            }
            match fs::symlink_metadata(&entry.backup_path) {
                Ok(metadata) if metadata.is_file() => fs::remove_file(&entry.backup_path).is_err(),
                Ok(_) => true,
                Err(_) => false,
            }
        });
        if !self.manifest.files.is_empty() {
            let _ = self.save();
            return false;
        }
        self.manifest.packages.clear();
        self.manifest.replaced.clear();
        self.discard_if_empty()
    }

    /// Copy `original` into the set and return where the copy is. The
    /// original is left as it is.
    pub fn copy_file(&mut self, package_id: Option<&str>, original: &Path) -> Result<PathBuf> {
        let index = self.begin_entry(package_id, original)?;
        let entry = self.manifest.files[index].clone();
        if self.planning {
            return Ok(entry.backup_path);
        }

        // A file inside the resource folder that this set already copied in
        // full keeps that first copy: it is the file as it was before the
        // run, and a second copy would hold what the run itself wrote. So an
        // update that replaces one file twice goes on, and the set keeps the
        // older copy. The new entry goes again.
        if !entry.original_path.is_absolute() {
            let place = same_place(&entry.backup_path);
            let earlier = self.manifest.files[..index]
                .iter()
                .find(|earlier| {
                    earlier.state == BackupEntryState::Done
                        && same_place(&earlier.backup_path) == place
                })
                .map(|earlier| earlier.backup_path.clone());
            if let Some(earlier) = earlier
                && is_real_file(&earlier)
            {
                self.drop_entry(index)?;
                return Ok(earlier);
            }
        }
        if let Some(parent) = entry.backup_path.parent() {
            fs::create_dir_all(parent).with_path(parent)?;
        }
        // Otherwise a copy never goes over a file the set already holds, as
        // a move never does (see `relocate_into_set`): inside the resource
        // folder another entry's copy may sit there, also under a name that
        // differs only in case, and an outside file's place is a fresh one.
        if fs::symlink_metadata(&entry.backup_path).is_ok() {
            let _ = self.drop_entry(index);
            return Err(RabbitError::BackupSetWriteRefused {
                path: entry.backup_path,
                reason: "the backup set already holds a file at this path".to_string(),
            });
        }
        fs::copy(original, &entry.backup_path).with_path(&entry.backup_path)?;
        let copied_sha256 = sha256_file(&entry.backup_path)?;
        if copied_sha256 != entry.sha256 {
            // The entry stays pending, which tells restore not to trust
            // whatever is left at the backup path.
            let _ = fs::remove_file(&entry.backup_path);
            return Err(RabbitError::HashMismatch {
                path: entry.backup_path,
                expected: entry.sha256,
                actual: copied_sha256,
            });
        }
        self.finish_entry(index)?;
        Ok(entry.backup_path)
    }

    /// Move `original` into the set and say where it went. Uninstall's way
    /// in: the file leaves its place, and the set is the only copy.
    ///
    /// The entry is saved as pending before the file moves and marked done
    /// after, as for a copy. On the same volume the move is a rename. With
    /// `across_volumes`, or when the rename finds the set on another volume
    /// after all, the file is copied, the copy is checked against `digest`,
    /// and only then is the original deleted. Checking free space for that
    /// copy is the caller's job, before the run starts.
    ///
    /// `digest` is the size and hash the caller just read; `None` reads them
    /// here. When the move fails, the file is still where it was and the
    /// entry is taken out again.
    pub fn move_file(
        &mut self,
        package_id: Option<&str>,
        original: &Path,
        digest: Option<FileDigest>,
        across_volumes: bool,
    ) -> std::result::Result<MovedFile, MoveFileError> {
        let digest = match digest {
            Some(digest) => digest,
            None if self.planning => FileDigest {
                size: 0,
                sha256: String::new(),
            },
            None => FileDigest::of(original).map_err(MoveFileError::NotMoved)?,
        };
        let index = self
            .begin_known_entry(package_id, original, digest.clone())
            .map_err(MoveFileError::NotMoved)?;
        let backup_path = self.manifest.files[index].backup_path.clone();
        let mut moved = MovedFile {
            original: original.to_path_buf(),
            backup_path,
            size: digest.size,
            sha256: digest.sha256,
            copied: false,
        };
        if self.planning {
            return Ok(moved);
        }
        match relocate_into_set(original, &moved.backup_path, &moved.sha256, across_volumes) {
            Ok(copied) => moved.copied = copied,
            Err(error) => {
                let _ = self.drop_entry(index);
                return Err(error);
            }
        }
        // A failed save leaves the entry pending on disk. Restore still
        // trusts a pending entry whose copy is whole, so the file stays
        // recoverable and the move counts as done.
        let _ = self.finish_entry(index);
        Ok(moved)
    }

    /// Move a file [`Self::move_file`] moved back to where it was, and take
    /// its entry out of the set. Refuses when something is at the original
    /// path again, rather than replacing it.
    ///
    /// The entry goes back to pending first: should RABBIT stop halfway, a
    /// whole copy in the set is still restored, and a copy that already went
    /// home is skipped. When moving back fails, the entry is done again and
    /// the set still holds the file.
    pub fn put_back(&mut self, moved: &MovedFile) -> Result<()> {
        if self.planning {
            return Ok(());
        }
        let index = self
            .manifest
            .files
            .iter()
            .position(|entry| entry.backup_path == moved.backup_path)
            .ok_or_else(|| RabbitError::BackupNotFound(moved.backup_path.clone()))?;
        if fs::symlink_metadata(&moved.original).is_ok() {
            return Err(RabbitError::BackupSetWriteRefused {
                path: moved.original.clone(),
                reason: "something is at this path again, and RABBIT never replaces it".to_string(),
            });
        }
        self.manifest.files[index].state = BackupEntryState::Pending;
        self.save()?;
        match move_back(moved) {
            Ok(()) => {
                let _ = self.drop_entry(index);
                remove_empty_parents(&moved.backup_path, &self.set_path);
                Ok(())
            }
            Err(error) => {
                self.manifest.files[index].state = BackupEntryState::Done;
                let _ = self.save();
                Err(error)
            }
        }
    }

    /// Record that an uninstall is removing `package_id` into this set, with
    /// what a restore of just that package needs. Saved before any of the
    /// package's files move.
    ///
    /// The set is the only copy of the files an uninstall moves, so it is
    /// never made behind a link: when `RABBIT` or `RABBIT/backups` is a
    /// symbolic link or junction, which a restore never follows (see
    /// [`find_backup_set`]), the record is refused with
    /// [`RabbitError::BackupPathIsLink`] before the package's first file
    /// moves (see [`Self::start_or_check`]). Uninstall's checks before
    /// changes refuse such a run already; this holds should the folder
    /// become a link in between.
    pub fn record_package(&mut self, package_id: &str, record: BackupPackageRecord) -> Result<()> {
        if self.planning {
            return Ok(());
        }
        self.start_or_check()?;
        self.manifest
            .packages
            .insert(package_id.to_string(), record);
        let saved = self.save();
        if saved.is_err() {
            // Not on disk, so not in the set: the package's files won't move.
            self.manifest.packages.remove(package_id);
        }
        saved
    }

    /// Record the `langpack=` value the uninstall of `package_id` is about
    /// to replace, or forget it again with `None`. Saved before
    /// `reaper.ini` changes, so the value is never lost.
    pub fn set_langpack_before(&mut self, package_id: &str, value: Option<String>) -> Result<()> {
        if self.planning {
            return Ok(());
        }
        let Some(record) = self.manifest.packages.get_mut(package_id) else {
            return Ok(());
        };
        record.langpack_before = value;
        self.save()
    }

    /// Record whether the uninstall moved `package_id`'s settings into the
    /// set, for a package recorded with [`Self::record_package`]. Saved when
    /// it changes. The uninstall records `false` with the package, and
    /// `true` once a settings file has moved.
    pub fn set_with_settings(&mut self, package_id: &str, with_settings: bool) -> Result<()> {
        if self.planning {
            return Ok(());
        }
        let Some(record) = self.manifest.packages.get_mut(package_id) else {
            return Ok(());
        };
        if record.with_settings == Some(with_settings) {
            return Ok(());
        }
        record.with_settings = Some(with_settings);
        self.save()
    }

    /// Record what replaced `package_id`'s files in this set: RABBIT's
    /// receipt entry for the package from just before, and the version that
    /// replaced it. Saved before the first of the package's files is copied
    /// in, as an uninstall records a package before it moves any, so the set
    /// never holds a file of the package without the record. Refused, as
    /// every write is, while the set would be behind a link (see
    /// [`Self::start_or_check`]).
    ///
    /// A package recorded already keeps its first record, which describes
    /// the package as it was before this run. Any set that keeps a
    /// package's files because something replaced them can record the
    /// package this way, a before-restore set too.
    pub fn record_replaced(
        &mut self,
        package_id: &str,
        record: ReplacedPackageRecord,
    ) -> Result<()> {
        if self.planning || self.manifest.replaced.contains_key(package_id) {
            return Ok(());
        }
        self.start_or_check()?;
        self.manifest
            .replaced
            .insert(package_id.to_string(), record);
        let saved = self.save();
        if saved.is_err() {
            // Not on disk, so not in the set: the package's files won't go
            // in either.
            self.manifest.replaced.remove(package_id);
        }
        saved
    }

    /// Forget what [`Self::record_replaced`] recorded of `package_id` when
    /// the set holds no entry of the package: copying its first file failed
    /// before the entry was saved, as when the file couldn't be read. A set
    /// left with nothing in it is removed again, so such a failure leaves no
    /// set, as it did before the record came first. Best effort.
    pub(crate) fn forget_replaced_without_entry(&mut self, package_id: &str) {
        if !self.started
            || self.planning
            || self
                .manifest
                .files
                .iter()
                .any(|entry| entry.package_id.as_deref() == Some(package_id))
        {
            return;
        }
        if self.manifest.replaced.remove(package_id).is_some() {
            let _ = self.save();
        }
        self.discard_if_empty();
    }

    /// Record what the key map this set is about to hold was, as RABBIT
    /// read it before OSARA's install. Call it before the key map is copied
    /// in, so the set never holds the file without it.
    pub fn set_keymap_origin(&mut self, origin: KeymapOrigin) -> Result<()> {
        self.manifest.keymap_origin = Some(origin);
        if self.started && !self.planning {
            self.save()?;
        }
        Ok(())
    }

    /// Forget a package recorded with [`Self::record_package`], after its
    /// files were put back: the set no longer holds it.
    pub fn forget_package(&mut self, package_id: &str) -> Result<()> {
        if self.planning || self.manifest.packages.remove(package_id).is_none() {
            return Ok(());
        }
        self.save()
    }

    /// Whether the set holds `package_id`: a record of it from
    /// [`Self::record_package`], and a file of it that a restore would put
    /// back, by the rule a restore uses (see [`entry_comes_back`]). An
    /// uninstall's report names the set only for such a package. It reads
    /// the writer's own manifest in memory, which the writer saves after
    /// every change, not the file.
    pub(crate) fn holds_package(&self, package_id: &str) -> bool {
        self.started
            && !self.planning
            && self.manifest.packages.contains_key(package_id)
            && holds_a_file_of(&self.set_path, &self.manifest, package_id, CopyCheck::Hash)
    }

    /// Forget every recorded package when the set holds no file at all:
    /// records alone bring nothing back, and a restore refuses such a set
    /// ([`RestoreRefusal::NothingOfItInSet`]). [`Self::discard_if_empty`] then
    /// removes it. An uninstall records each package before it moves
    /// anything, so a run whose packages moved no file would otherwise leave
    /// a set to name that nothing can come back from. A set with any file
    /// entry stays as it is.
    pub(crate) fn forget_packages_if_no_file(&mut self) {
        if !self.started
            || self.planning
            || !self.manifest.files.is_empty()
            || self.manifest.packages.is_empty()
        {
            return;
        }
        self.manifest.packages.clear();
        let _ = self.save();
    }

    /// Remove the set again when it holds nothing: every file was put back
    /// and every package forgotten. Returns whether no set is left, which is
    /// also the case when nothing was ever written. Best effort; a set that
    /// can't be removed is merely empty.
    pub fn discard_if_empty(&mut self) -> bool {
        if !self.started || self.planning {
            return true;
        }
        if !self.manifest.files.is_empty()
            || !self.manifest.packages.is_empty()
            || !self.manifest.replaced.is_empty()
        {
            return false;
        }
        let _ = fs::remove_file(self.manifest_path());
        remove_empty_dirs(&self.set_path);
        !self.set_path.exists()
    }

    /// Take the entry at `index` out of the manifest, with the origin note
    /// of an outside file, after its file went back or never left.
    fn drop_entry(&mut self, index: usize) -> Result<()> {
        let entry = self.manifest.files.remove(index);
        if entry.original_path.is_absolute()
            && let Some(name) = entry.backup_path.file_name()
        {
            let mut note = name.to_os_string();
            note.push(ORIGIN_SIDECAR_SUFFIX);
            let _ = fs::remove_file(entry.backup_path.with_file_name(note));
            remove_empty_parents(&entry.backup_path, &self.set_path);
        }
        self.save()
    }

    /// Record `original` as pending, saved to disk before anything is
    /// copied, and write the origin note for an outside file. Returns the
    /// entry's index.
    fn begin_entry(&mut self, package_id: Option<&str>, original: &Path) -> Result<usize> {
        // Read the original first: one that can't be read leaves no set.
        let digest = if self.planning {
            FileDigest {
                size: 0,
                sha256: String::new(),
            }
        } else {
            FileDigest::of(original)?
        };
        self.begin_known_entry(package_id, original, digest)
    }

    /// [`Self::begin_entry`] for a file whose size and hash the caller
    /// already read.
    fn begin_known_entry(
        &mut self,
        package_id: Option<&str>,
        original: &Path,
        digest: FileDigest,
    ) -> Result<usize> {
        self.start_or_check()?;
        let slot = self.slot_for(original)?;
        self.manifest.files.push(BackupManifestFile {
            package_id: package_id.map(ToString::to_string),
            original_path: slot.original_path.clone(),
            backup_path: slot.backup_path,
            size: digest.size,
            sha256: digest.sha256,
            state: BackupEntryState::Pending,
        });
        if !self.planning {
            self.save()?;
            if let Some(note_path) = slot.origin_note
                && let Err(error) = write_origin_note(&note_path, &slot.original_path)
            {
                // The note may be another file's: only the entry goes again.
                self.manifest.files.pop();
                let _ = self.save();
                return Err(error);
            }
        }
        Ok(self.manifest.files.len() - 1)
    }

    fn finish_entry(&mut self, index: usize) -> Result<()> {
        self.manifest.files[index].state = BackupEntryState::Done;
        self.save()
    }

    /// Claim the set's folder on the first write. Two runs in the same
    /// second ask for the same id; the second gets `<id>-2` instead of
    /// writing into the first one's set and replacing its manifest. The
    /// time the folder is made goes into the manifest as `created_unix`.
    ///
    /// Refused with [`RabbitError::BackupPathIsLink`], before anything is
    /// written, when `RABBIT` or `RABBIT/backups` is a symbolic link or
    /// junction: a restore never follows one there, so the set couldn't
    /// bring anything back. Every run that writes a set refuses such a
    /// folder before it changes anything; this holds should the folder
    /// become a link in between, and [`Self::start_or_check`] checks again
    /// before every later write.
    fn start(&mut self) -> Result<()> {
        if self.started || self.planning {
            return Ok(());
        }
        check_backups_folder_is_real(&self.resource_path)?;
        let backups = backups_root(&self.resource_path);
        fs::create_dir_all(&backups).with_path(&backups)?;
        let set_id = self.manifest.created_at.clone();
        let mut attempt = 1;
        loop {
            match fs::create_dir(&self.set_path) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempt += 1;
                    self.set_path = backups.join(format!("{set_id}-{attempt}"));
                }
                Err(error) => return Err(crate::error::io_error_at(self.set_path.clone(), error)),
            }
        }
        self.manifest.created_unix = seconds_since_1970();
        self.started = true;
        Ok(())
    }

    /// Start the set on its first write (see [`Self::start`]), and before
    /// every later one check again that it isn't behind a link: `RABBIT`,
    /// `RABBIT/backups` or the set's own folder may have become one since.
    /// Refused with [`RabbitError::BackupPathIsLink`] before anything is
    /// written, so a set that started in a real folder never writes a
    /// record or a file through a link later either.
    fn start_or_check(&mut self) -> Result<()> {
        if self.planning || !self.started {
            return self.start();
        }
        check_backups_folder_is_real(&self.resource_path)?;
        // A set folder that is gone is made again by the write, as before;
        // only a link is refused.
        is_real_folder(&self.set_path)?;
        Ok(())
    }

    /// Where a backup of `original` goes.
    fn slot_for(&self, original: &Path) -> Result<BackupSlot> {
        // Both without `..`, or a file reached through `../REAPER` would
        // never count as inside the resource folder on macOS.
        let absolute = absolute_without_dots(original).with_path(original)?;
        let resource = absolute_without_dots(&self.resource_path).with_path(&self.resource_path)?;
        if let Some(relative) = relative_within(&absolute, &resource)
            && is_plain_relative(&relative)
            && !collides_with_set_layout(&relative)
        {
            return Ok(BackupSlot {
                backup_path: self.set_path.join(&relative),
                original_path: relative,
                origin_note: None,
            });
        }

        let file_name = absolute
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".to_string());
        let number = self.next_external_number()?;
        let folder = self.set_path.join(EXTERNAL_DIR).join(number.to_string());
        Ok(BackupSlot {
            original_path: absolute,
            backup_path: folder.join(&file_name),
            origin_note: Some(folder.join(format!("{file_name}{ORIGIN_SIDECAR_SUFFIX}"))),
        })
    }

    /// The number of the next outside file's folder, `external/<n>`: one
    /// above the highest in use, from 1. In use means named by an entry of
    /// the manifest, or a name under `external/` on disk once the set is
    /// started. Counting entries would hand a number out again after a
    /// put-back took an entry out (see [`Self::put_back`]), and a copy that
    /// couldn't be removed may still sit in its folder without an entry.
    ///
    /// A folder that can't be listed refuses the file, and so does a
    /// number too high to go up by one: a number guessed from the manifest
    /// alone could be another file's. Should one be taken all the same, the
    /// origin note and the copy are never written over (see
    /// [`write_origin_note`] and [`relocate_into_set`]), so the file is
    /// refused there.
    fn next_external_number(&self) -> Result<u64> {
        let external = self.set_path.join(EXTERNAL_DIR);
        let number = |name: &OsStr| name.to_str().and_then(|name| name.parse::<u64>().ok());
        let mut highest = self
            .manifest
            .files
            .iter()
            .filter_map(|file| {
                relative_within(&file.backup_path, &external)?
                    .components()
                    .next()
                    .and_then(|first| number(first.as_os_str()))
            })
            .max()
            .unwrap_or(0);
        // Only the started set's own folder is read. A planning writer has
        // none: whatever is at its path belongs to another set.
        if self.started && !self.planning {
            match fs::read_dir(&external) {
                Ok(entries) => {
                    for entry in entries {
                        let entry = entry.with_path(&external)?;
                        if let Some(found) = number(&entry.file_name()) {
                            highest = highest.max(found);
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(crate::error::io_error_at(external, error)),
            }
        }
        highest
            .checked_add(1)
            .ok_or_else(|| RabbitError::BackupSetWriteRefused {
                path: external.join(highest.to_string()),
                reason: "the backup set has no number left for another file from outside the \
                         resource folder"
                    .to_string(),
            })
    }

    fn save(&self) -> Result<()> {
        save_backup_manifest(&self.set_path, &self.manifest).map(|_| ())
    }
}

/// Where [`BackupSetWriter`] puts one file.
struct BackupSlot {
    /// What the manifest records: relative inside the resource folder,
    /// absolute outside it.
    original_path: PathBuf,
    backup_path: PathBuf,
    /// For an outside file, the note that holds its absolute path.
    origin_note: Option<PathBuf>,
}

/// Write the note of an outside file's original path at `note_path`, never
/// over a file already there: that would be another file's note, and its
/// copy would lose the path it came from. A note left half written is
/// removed again.
fn write_origin_note(note_path: &Path, original: &Path) -> Result<()> {
    use std::io::Write as _;

    if let Some(parent) = note_path.parent() {
        fs::create_dir_all(parent).with_path(parent)?;
    }
    let mut note = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(note_path)
        .with_path(note_path)?;
    if let Err(error) = note.write_all(original.to_string_lossy().as_bytes()) {
        drop(note);
        let _ = fs::remove_file(note_path);
        return Err(crate::error::io_error_at(note_path.to_path_buf(), error));
    }
    Ok(())
}

/// Move `original` to `backup_path`, a fresh path in a set. Returns whether
/// it went as a checked copy (to another volume) rather than a rename.
fn relocate_into_set(
    original: &Path,
    backup_path: &Path,
    sha256: &str,
    across_volumes: bool,
) -> std::result::Result<bool, MoveFileError> {
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)
            .with_path(parent)
            .map_err(MoveFileError::NotMoved)?;
    }
    // A rename would replace whatever is there; a set never loses a file
    // it already holds.
    if fs::symlink_metadata(backup_path).is_ok() {
        return Err(MoveFileError::NotMoved(
            RabbitError::BackupSetWriteRefused {
                path: backup_path.to_path_buf(),
                reason: "the backup set already holds a file at this path".to_string(),
            },
        ));
    }
    if !across_volumes {
        match fs::rename(original, backup_path) {
            Ok(()) => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {}
            Err(source) if held_open_by_another_program(&source) => {
                return Err(MoveFileError::InUse {
                    path: original.to_path_buf(),
                    source,
                });
            }
            Err(error) => {
                return Err(MoveFileError::NotMoved(crate::error::io_error_at(
                    original.to_path_buf(),
                    error,
                )));
            }
        }
    }
    copy_checked(original, backup_path, sha256).map_err(MoveFileError::NotMoved)?;
    if let Err(source) = fs::remove_file(original) {
        let _ = fs::remove_file(backup_path);
        return Err(MoveFileError::InUse {
            path: original.to_path_buf(),
            source,
        });
    }
    Ok(true)
}

/// Whether a rename failed because another program has the file open:
/// Windows refuses to rename a file another program opened without sharing
/// deletion, or one it locked. Nothing has moved then. Unix lets anyone
/// rename an open file.
fn held_open_by_another_program(error: &io::Error) -> bool {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    cfg!(windows)
        && matches!(
            error.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
        )
}

/// Move a file back from a set to where it was, the way it came.
fn move_back(moved: &MovedFile) -> Result<()> {
    if let Some(parent) = moved.original.parent() {
        fs::create_dir_all(parent).with_path(parent)?;
    }
    if !moved.copied {
        match fs::rename(&moved.backup_path, &moved.original) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {}
            Err(error) => return Err(crate::error::io_error_at(moved.original.clone(), error)),
        }
    }
    // Copied under a temporary name and renamed into place, so a copy that
    // breaks off never leaves half a file where the real one belongs.
    let staged = temporary_restore_path(&moved.original);
    copy_checked(&moved.backup_path, &staged, &moved.sha256)?;
    fs::rename(&staged, &moved.original).map_err(|error| {
        let _ = fs::remove_file(&staged);
        crate::error::io_error_at(moved.original.clone(), error)
    })?;
    let _ = fs::remove_file(&moved.backup_path);
    Ok(())
}

/// Copy `from` to `to` and check the copy's hash. A copy that fails or
/// doesn't match is removed again.
fn copy_checked(from: &Path, to: &Path, sha256: &str) -> Result<()> {
    if let Err(error) = fs::copy(from, to) {
        let _ = fs::remove_file(to);
        return Err(crate::error::io_error_at(to.to_path_buf(), error));
    }
    let copied = match sha256_file(to) {
        Ok(copied) => copied,
        Err(error) => {
            let _ = fs::remove_file(to);
            return Err(error);
        }
    };
    if copied != sha256 {
        let _ = fs::remove_file(to);
        return Err(RabbitError::HashMismatch {
            path: to.to_path_buf(),
            expected: sha256.to_string(),
            actual: copied,
        });
    }
    Ok(())
}

/// Remove the folders between `path` and `stop_at` that are empty, nearest
/// first, leaving `stop_at` itself.
fn remove_empty_parents(path: &Path, stop_at: &Path) {
    let mut current = path.parent();
    while let Some(folder) = current {
        if folder == stop_at || !folder.starts_with(stop_at) || fs::remove_dir(folder).is_err() {
            break;
        }
        current = folder.parent();
    }
}

/// Remove `folder` and every folder inside it that holds nothing but empty
/// folders. Files are never removed.
fn remove_empty_dirs(folder: &Path) {
    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                remove_empty_dirs(&entry.path());
            }
        }
    }
    let _ = fs::remove_dir(folder);
}

/// Every set in `RABBIT/backups`, and every unfinished deletion in
/// `RABBIT/backups-deleting`, newest first, as the overview lists them
/// (see [`crate::backup_overview::backup_overview`]).
///
/// No symbolic link or junction is ever followed. When `RABBIT`,
/// `RABBIT/backups` or `RABBIT/backups-deleting` is one, nothing is listed
/// and the error says why: the sets it leads to may belong to another
/// REAPER, or to nobody, and an empty list would say there are none. A
/// link directly in `RABBIT/backups` is listed with a warning that it isn't
/// a backup RABBIT made, and a link inside a set is listed by name. Neither
/// is looked into.
///
/// A folder whose name restore would refuse (see
/// [`backup_id_is_well_formed`]) is listed the same way as a link: with a
/// warning, and never looked into. On Windows a path through `unix-1.`
/// would open `unix-1`, so its listing would show another set's files.
pub fn list_backup_sets(resource_path: &Path) -> Result<Vec<BackupSet>> {
    Ok(crate::backup_overview::backup_overview(resource_path)?.backup_sets())
}

/// What a restore, and the overview of the backups, ask of the computer
/// they run on, beyond the backup sets themselves. [`restore_backup_set`]
/// and [`crate::backup_overview::backup_overview`] pass
/// [`BackupEnvironment::system`], and tests pass their own answers.
#[derive(Clone, Copy)]
pub(crate) struct BackupEnvironment<'a> {
    /// Whether RABBIT itself runs as root, as it does under `sudo`. Only a
    /// Mac answers `true`: the rule about root is macOS's alone, as for
    /// uninstall.
    pub runs_as_root: &'a dyn Fn() -> bool,
    /// The account that owns the file or folder at a path, by its own
    /// metadata, never through a link: its user id on macOS. `None` when
    /// that can't be read, and always on Windows, where a backup never
    /// counts as another account's.
    pub owner_of: &'a dyn Fn(&Path) -> Option<u32>,
    /// Each package's exclusive group, by package id: packages whose
    /// installs remove each other. From the embedded package manifests.
    pub exclusive_groups: &'a dyn Fn() -> BTreeMap<String, String>,
    /// Where outside the resource folder at a path a restore may put files
    /// back. Only the overview asks this, the first time a set names such a
    /// place; a restore is handed its own (see `restore_backup_set_with`)
    /// once the set is one it may restore.
    pub outside_rules: &'a dyn Fn(&Path) -> OutsideRestoreRules,
    /// Every REAPER running on this computer, as the uninstall preflight
    /// scans them. A restore checks the target's REAPER among them, and all
    /// of them for a package whose uninstall needs every REAPER closed.
    pub reaper_processes: &'a dyn Fn() -> Vec<RunningProcess>,
    /// The running programs with one of these names, as the uninstall
    /// preflight scans them: the `blocking_programs` of the restored
    /// packages' uninstall plans. Asked only when there are any.
    pub named_processes: &'a dyn Fn(&[String]) -> Vec<RunningProcess>,
    /// The packages RABBIT knows on a platform, with their uninstall plans,
    /// from the embedded package manifests, as an uninstall reads them.
    pub package_specs: &'a dyn Fn(Platform) -> Vec<PackageSpec>,
    /// Whether the REAPER at a resource folder is the standard one or a
    /// portable one, as an uninstall decides it. Picks the uninstall plan
    /// whose rules a restore follows.
    pub target_kind: &'a dyn Fn(Platform, &Path) -> InstallationKind,
}

impl BackupEnvironment<'static> {
    /// This computer, asked the way an uninstall asks it.
    pub(crate) fn system() -> Self {
        Self {
            runs_as_root: &system_runs_as_root,
            owner_of: &system_owner_of,
            exclusive_groups: &system_exclusive_groups,
            outside_rules: &crate::install::outside_restore_rules,
            reaper_processes: &system_reaper_processes,
            named_processes: &running_processes_named,
            package_specs: &crate::package::builtin_package_specs,
            target_kind: &crate::uninstall::execute::target_kind_of,
        }
    }

    /// A computer where RABBIT runs as the user, as it should. Owners are
    /// read as on this computer. No place outside the resource folder takes
    /// a file back, so tests never name the user's real CLAP or vendor
    /// folders. No REAPER and no other program runs, and every target is a
    /// portable REAPER, so tests never depend on what runs on the computer
    /// that tests them.
    #[cfg(test)]
    pub(crate) const AS_USER: Self = Self {
        runs_as_root: &|| false,
        owner_of: &system_owner_of,
        exclusive_groups: &system_exclusive_groups,
        outside_rules: &no_outside_places,
        reaper_processes: &Vec::new,
        named_processes: &|_| Vec::new(),
        package_specs: &crate::package::builtin_package_specs,
        target_kind: &|_, _| InstallationKind::Portable,
    };
}

/// Every REAPER running on this computer.
fn system_reaper_processes() -> Vec<RunningProcess> {
    running_reaper_processes(Platform::current())
}

/// Rules that let no file back outside the resource folder, for tests.
#[cfg(test)]
fn no_outside_places(_resource_path: &Path) -> OutsideRestoreRules {
    OutsideRestoreRules::default()
}

/// Each package's exclusive group, by package id, as the embedded package
/// manifests declare them.
fn system_exclusive_groups() -> BTreeMap<String, String> {
    crate::package::embedded_package_manifest()
        .packages
        .into_iter()
        .filter_map(|spec| Some((spec.id, spec.exclusive_group?)))
        .collect()
}

/// The user id that owns `path`, from its own metadata: a link is never
/// followed. Unix only; Windows has no single id to compare.
fn system_owner_of(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        fs::symlink_metadata(path)
            .ok()
            .map(|metadata| metadata.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// The root check uninstall makes, through the same
/// [`SystemMacosObjects`](crate::uninstall::SystemMacosObjects), so the two
/// never differ: `geteuid` on macOS, and `false` everywhere else.
fn system_runs_as_root() -> bool {
    use crate::uninstall::MacosObjects;
    crate::uninstall::SystemMacosObjects.runs_as_root()
}

/// Restore the set `backup_id` names in `RABBIT/backups`, or with
/// `options.dry_run` say what a restore would do: one backup of
/// [`restore_backups`], the whole set or `options.package_id` from it.
///
/// Every refusal is a [`RabbitError::RestoreRefused`], whose code says why,
/// and comes before anything changes. A restore never copies a receipt file
/// over RABBIT's receipt: that would undo every change to it since the set
/// was made, uninstalls included. So, in a dry run too, it refuses a set an
/// update wrote ([`RestoreRefusal::EarlierVersionNotYet`]), a set that holds
/// only a copy of the receipt ([`RestoreRefusal::RecordsOnly`]), and any
/// other set that holds a copy of it ([`RestoreRefusal::ReceiptInSet`]). A
/// set an uninstall wrote merges each package's own entry into the receipt
/// instead, for every package with a file in the set. A package with none
/// is left out of a restore of the whole set, and refused on its own
/// ([`RestoreRefusal::NothingOfItInSet`]).
///
/// Before the lock, and again under it, a restore checks what each restored
/// package's own uninstall checks: the target's REAPER, every REAPER on the
/// computer when the package's uninstall needs that, and the programs its
/// uninstall plan names (see [`restore_preflight`]).
/// `options.allow_reaper_running` lets only the target's REAPER run. A dry
/// run reports what is running and refuses nothing for it, as the install
/// preflight's dry run does.
///
/// On macOS, a real restore refuses to run as root
/// ([`RestoreRefusal::AsRoot`]), as an uninstall does, and any restore
/// refuses a set that belongs to another account
/// ([`RestoreRefusal::NotYours`]). A dry run as root still says what a
/// restore would do.
///
/// A restore is all or nothing. When a file fails, or RABBIT's records
/// can't be saved, every file it had already replaced goes back from the
/// before-restore set, every file it added goes again, and the records stay
/// as they were. That is no refusal, since files changed in between: the
/// report comes back with [`RestoreStatus::NotRestored`], or
/// [`RestoreStatus::FilesLeft`] when a file couldn't be put back, and a
/// real restore saves it, as every real restore that gets past its checks
/// does.
pub fn restore_backup_set(
    resource_path: &Path,
    backup_id: &str,
    options: &RestoreBackupOptions,
) -> Result<RestoreBackupReport> {
    restore_backup_set_with(
        resource_path,
        backup_id,
        options,
        &BackupEnvironment::system(),
        || crate::install::outside_restore_rules(resource_path),
    )
}

/// [`restore_backup_set`], asking `environment` about the computer, and
/// with the places outside the resource folder that a restore may write to
/// coming from `outside`. Tests pass their own answers, and temporary
/// folders instead of the user's real CLAP and vendor folders.
///
/// The set is found, and refused if it is another account's or one a
/// restore never takes, before anything else is read. The package refusals
/// come next (see [`packages_to_restore`]), then the refusal of a real
/// restore as root. All of them come before `outside` is called, before the
/// REAPER and program checks, the lock and any change, so `outside` is only
/// called once the id named a set that may be restored. A refused restore
/// saves no report.
pub(crate) fn restore_backup_set_with(
    resource_path: &Path,
    backup_id: &str,
    options: &RestoreBackupOptions,
    environment: &BackupEnvironment<'_>,
    outside: impl FnOnce() -> OutsideRestoreRules,
) -> Result<RestoreBackupReport> {
    let item = RestoreItem {
        set_id: backup_id.to_string(),
        package_id: options.package_id.clone(),
        fingerprint: None,
    };
    let run_options = RestoreRunOptions {
        dry_run: options.dry_run,
        quick: false,
        allow_reaper_running: options.allow_reaper_running,
    };
    let RestoreRun {
        preflight,
        mut outcomes,
        // Held until the report is saved.
        install_lock: _install_lock,
        ..
    } = run_restore(
        resource_path,
        std::slice::from_ref(&item),
        run_options,
        environment,
        outside,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )?;
    let backup = match outcomes.pop() {
        Some(ItemOutcome::Ran(backup)) => *backup,
        Some(ItemOutcome::NotStarted(error)) => return Err(error),
        Some(ItemOutcome::NotReached) | None => {
            unreachable!("a restore of one backup has it and is never stopped")
        }
    };
    let mut report = RestoreBackupReport {
        resource_path: resource_path.to_path_buf(),
        backup_id: backup.backup_id,
        // A backup that ran was found, so its folder is known.
        backup_path: backup.backup_path.unwrap_or_default(),
        dry_run: options.dry_run,
        preflight,
        status: backup.status,
        error: backup.error,
        actions: backup.actions,
        skipped_targets: backup.skipped_targets,
        package_id: options.package_id.clone(),
        packages: backup.packages,
        skipped_packages: backup.skipped_packages,
        before_restore_set_id: backup.before_restore_set_id,
        saved_report: None,
    };
    // The report names the set that can undo the restore, or says what
    // was put back after a failure, so every real restore that gets here
    // leaves one, as every real uninstall does, still under the lock. A
    // report that can't be saved changes nothing of the restore. A dry run
    // writes nothing.
    if !options.dry_run {
        report.saved_report = save_restore_report(resource_path, &report).ok();
    }
    Ok(report)
}

/// Restore `items`, one backup after another in the order given, under one
/// install lock, or with `options.dry_run` say what a restore would do.
/// Each backup comes back the way [`restore_backup_set`] restores one, with
/// a before-restore set of its own, and all or nothing.
///
/// Checks that hold for the whole run refuse it, as an error, before
/// anything changes: another RABBIT holding the lock, a real run as root on
/// macOS, a REAPER or a program that the restored packages need closed,
/// and a resource folder RABBIT can't write to. A check of one backup
/// refuses only that backup: it is [`RestoreStatus::NotRestored`] with its
/// refusal, and the run goes on with the next. So does a backup that fails
/// partway, after what it changed is put back. Among those checks, a set
/// whose fingerprint differs from the one its item names
/// ([`RestoreRefusal::ChangedSinceListed`]), checked before the lock and
/// again under it, right before the backup's turn, when the set is also
/// found again in `RABBIT/backups`. Before each backup after the first,
/// what runs is scanned again for that backup's packages, since a REAPER
/// may have started while the backups before it were restored. Each
/// backup is checked against RABBIT's records as the backups before it in
/// the run left them.
///
/// `cancel` is read before each backup. The one being restored always
/// finishes; those after it are [`RestoreStatus::NotReached`], and the
/// report says the run was stopped. `progress` hears
/// [`ProgressEvent::RestoreStarted`] and [`ProgressEvent::RestoreCompleted`]
/// for each backup the run reaches.
///
/// Every real run that gets past the checks of the whole run saves its
/// report in `RABBIT/logs`, under a name no other report has, also when a
/// backup wasn't restored or the run was stopped. A run in which every
/// backup was refused by its own checks, or that names none, takes no lock,
/// changes nothing and saves no report, as a refused restore of one backup
/// saves none. A dry run saves nothing, and with `options.quick` reads no
/// file's content.
pub fn restore_backups(
    resource_path: &Path,
    items: &[RestoreItem],
    options: &RestoreRunOptions,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<RestoreRunReport> {
    restore_backups_with(
        resource_path,
        items,
        options,
        &BackupEnvironment::system(),
        || crate::install::outside_restore_rules(resource_path),
        progress,
        cancel,
    )
}

/// [`restore_backups`], asking `environment` about the computer, with the
/// places outside the resource folder that a restore may write to coming
/// from `outside`, as for [`restore_backup_set_with`].
pub(crate) fn restore_backups_with(
    resource_path: &Path,
    items: &[RestoreItem],
    options: &RestoreRunOptions,
    environment: &BackupEnvironment<'_>,
    outside: impl FnOnce() -> OutsideRestoreRules,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<RestoreRunReport> {
    let RestoreRun {
        preflight,
        outcomes,
        stopped,
        // Held until the report is saved.
        install_lock,
    } = run_restore(
        resource_path,
        items,
        *options,
        environment,
        outside,
        progress,
        cancel,
    )?;
    let mut report = RestoreRunReport {
        resource_path: resource_path.to_path_buf(),
        dry_run: options.dry_run,
        quick: options.copy_check() == CopyCheck::Size,
        preflight,
        backups: items
            .iter()
            .zip(outcomes)
            .map(|(item, outcome)| outcome.into_report_item(item))
            .collect(),
        stopped,
        saved_report: None,
    };
    // Only a real run that took the lock may have changed anything. One
    // whose every backup was refused by its own checks took none, and
    // nothing checked that it may write in `RABBIT` at all: as root, or
    // through a `RABBIT` that is a link, which refuses every backup.
    if install_lock.is_some() {
        report.saved_report = save_restore_report(resource_path, &report).ok();
    }
    drop(install_lock);
    Ok(report)
}

/// What [`run_restore`] did: the checks before anything changed, what came
/// of each backup in the order asked for, whether a stop kept it from some,
/// and the install lock a real run holds until its report is saved.
struct RestoreRun {
    preflight: PreflightReport,
    outcomes: Vec<ItemOutcome>,
    stopped: bool,
    install_lock: Option<crate::lock::PackageInstallLock>,
}

/// What came of one backup in [`run_restore`].
enum ItemOutcome {
    /// Refused, or failed, before anything of it changed.
    NotStarted(RabbitError),
    /// A stop kept the run from getting to it.
    NotReached,
    /// It was restored, or tried and put back, or in a dry run planned.
    Ran(Box<RestoreReportItem>),
}

impl ItemOutcome {
    /// What the backup's status is, for [`ProgressEvent::RestoreCompleted`].
    fn status(&self) -> RestoreStatus {
        match self {
            Self::NotStarted(_) => RestoreStatus::NotRestored,
            Self::NotReached => RestoreStatus::NotReached,
            Self::Ran(backup) => backup.status,
        }
    }

    /// The report of the backup `item` asked for.
    fn into_report_item(self, item: &RestoreItem) -> RestoreReportItem {
        let empty = |status: RestoreStatus| RestoreReportItem {
            backup_id: item.set_id.clone(),
            package_id: item.package_id.clone(),
            status,
            backup_path: None,
            actions: Vec::new(),
            skipped_targets: Vec::new(),
            packages: Vec::new(),
            skipped_packages: Vec::new(),
            before_restore_set_id: None,
            refusal: None,
            error: None,
        };
        match self {
            Self::Ran(backup) => *backup,
            Self::NotReached => empty(RestoreStatus::NotReached),
            Self::NotStarted(RabbitError::RestoreRefused(refusal)) => RestoreReportItem {
                refusal: Some(refusal),
                ..empty(RestoreStatus::NotRestored)
            },
            Self::NotStarted(error) => RestoreReportItem {
                error: Some(error.to_string()),
                ..empty(RestoreStatus::NotRestored)
            },
        }
    }
}

/// Check, plan and restore `items` in order, as [`restore_backups`] says,
/// and report on each without saving the report. An error is a refusal of
/// the whole run, or an error that stopped it, before anything changed.
///
/// Each backup's own checks come first, before the lock: the set is found
/// and refused if it is another account's, changed since it was listed, or
/// one a restore never takes, then the package refusals (see
/// [`packages_to_restore`]). A real run as root is refused next, once a
/// backup passed them, then `outside` is called, once, and the REAPER and
/// program checks of every package the run brings back are made together.
/// Under the lock, what runs is scanned again, and then each backup in turn
/// is found again, checked again and planned right before it is restored,
/// with what runs scanned again for its own packages before every backup
/// after the first.
fn run_restore(
    resource_path: &Path,
    items: &[RestoreItem],
    options: RestoreRunOptions,
    environment: &BackupEnvironment<'_>,
    outside: impl FnOnce() -> OutsideRestoreRules,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<RestoreRun> {
    // A refusal says that nothing changed, so only an error from before the
    // first change becomes one. Once files go back, an error stays as it
    // is, even one a restore refuses for beforehand: a link made since
    // where RABBIT keeps its backups stops the backup with files already
    // back, and is reported on it (see `run_planned_restore`).
    let check = options.copy_check();
    let found: Vec<Result<FoundSet<'_>>> = items
        .iter()
        .map(|item| {
            find_set_to_restore(resource_path, item, check, environment).map_err(as_restore_refusal)
        })
        .collect();
    let run_options = options.for_backup(None);

    let (preflight, outside, install_lock) = if found.iter().any(Result::is_ok) {
        // After the set and package refusals, which hold for any account,
        // and before the REAPER check and the lock, whose file would be
        // root's in the user's resource folder. A dry run changes nothing,
        // so it may still say what a restore would do.
        if !options.dry_run && (environment.runs_as_root)() {
            return Err(RestoreRefusal::AsRoot.into());
        }
        let outside = outside();
        let restored: std::collections::BTreeSet<String> = found
            .iter()
            .flatten()
            .flat_map(FoundSet::restored_package_ids)
            .collect();
        let restored: Vec<String> = restored.into_iter().collect();
        // The REAPER and program checks come before the lock, as the
        // install preflight always did. A dry run makes them too, and
        // reports what runs without refusing for it.
        let rules = restore_process_rules(resource_path, &restored, environment);
        let (preflight, refusal) =
            restore_preflight(resource_path, &rules, &run_options, environment);
        if let Some(refusal) = refusal {
            return Err(refusal.into());
        }
        // A restore writes the same files an install does, so it must not
        // run while an install, or another restore, works on this resource
        // folder.
        let install_lock = if options.dry_run {
            None
        } else {
            let lock = crate::lock::acquire_package_install_lock(resource_path)
                .map_err(as_restore_refusal)?;
            // A REAPER or a program may have started since the checks above,
            // so what runs is checked again under the lock, and again before
            // each backup after the first (below), as an uninstall checks it
            // again right before each package. Only the scans are made
            // again; the checks of where the files go stand.
            if let Some(refusal) =
                running_blockers(resource_path, &rules, environment).refusal(&run_options)
            {
                return Err(refusal.into());
            }
            Some(lock)
        };
        (preflight, outside, install_lock)
    } else {
        // Every backup was refused on its own, or none was asked for:
        // nothing runs, nothing else is checked, no lock is taken, and so
        // no report is saved (see `restore_backups_with`).
        let nothing_checked = PreflightReport {
            passed: true,
            checks: Vec::new(),
        };
        (nothing_checked, OutsideRestoreRules::default(), None)
    };

    let mut outcomes = Vec::with_capacity(items.len());
    let mut stopped = false;
    for (index, (item, found)) in items.iter().zip(found).enumerate() {
        // Between backups only: the one being restored always finishes, so
        // a stop never leaves half a backup.
        if stopped || cancel.is_cancelled() {
            stopped = true;
            outcomes.push(ItemOutcome::NotReached);
            continue;
        }
        progress.report(ProgressEvent::RestoreStarted {
            set_id: item.set_id.clone(),
            package_id: item.package_id.clone(),
        });
        let backup_options = options.for_backup(item.package_id.as_deref());
        let outcome = match found {
            Err(error) => ItemOutcome::NotStarted(error),
            Ok(found) => {
                // The backups before this one may have taken minutes, and a
                // REAPER or a program may have started meanwhile. So what
                // runs is scanned again for this backup's own packages, as
                // an uninstall scans before each package; the first backup
                // has the scan under the lock. What it refuses is this
                // backup's own: it stays as it was, and the run goes on.
                let blocked = if options.dry_run || index == 0 {
                    None
                } else {
                    let rules = restore_process_rules(
                        resource_path,
                        &found.restored_package_ids(),
                        environment,
                    );
                    running_blockers(resource_path, &rules, environment).refusal(&backup_options)
                };
                match blocked {
                    Some(refusal) => ItemOutcome::NotStarted(refusal.into()),
                    None => restore_found_set(
                        resource_path,
                        item,
                        found,
                        &backup_options,
                        &outside,
                        check,
                    ),
                }
            }
        };
        progress.report(ProgressEvent::RestoreCompleted {
            set_id: item.set_id.clone(),
            package_id: item.package_id.clone(),
            status: outcome.status(),
        });
        outcomes.push(outcome);
    }
    Ok(RestoreRun {
        preflight,
        outcomes,
        stopped,
        install_lock,
    })
}

/// Find the set `item` names and make the checks of it that come before
/// the lock (see [`run_restore`]). Nothing changes.
fn find_set_to_restore<'a>(
    resource_path: &Path,
    item: &'a RestoreItem,
    check: CopyCheck,
    environment: &BackupEnvironment<'_>,
) -> Result<FoundSet<'a>> {
    let backup_id = item.set_id.as_str();
    let package_id = item.package_id.as_deref();
    let backup_path = set_to_restore(resource_path, backup_id)?;
    refuse_another_accounts_set(resource_path, backup_id, &backup_path, environment)?;
    refuse_changed_since_listed(backup_id, &backup_path, item.fingerprint.as_ref())?;
    let manifest = load_manifest_to_restore(backup_id, &backup_path)?;
    refuse_sets_a_restore_does_not_take(resource_path, backup_id, &backup_path, manifest.as_ref())?;
    let packages = packages_to_restore(
        backup_id,
        &backup_path,
        manifest.as_ref(),
        package_id,
        check,
    )?;
    Ok(FoundSet {
        backup_id,
        package_id,
        backup_path,
        manifest,
        packages,
    })
}

/// Refuse the set `backup_id` at `backup_path` when its fingerprint now
/// differs from `listed`, the one its item names
/// ([`RestoreRefusal::ChangedSinceListed`]): what the caller was shown of
/// it may no longer hold. Nothing is checked without one.
fn refuse_changed_since_listed(
    backup_id: &str,
    backup_path: &Path,
    listed: Option<&SetFingerprint>,
) -> Result<()> {
    match listed {
        Some(listed) if crate::backup_overview::current_fingerprint(backup_path) != *listed => {
            Err(RestoreRefusal::ChangedSinceListed {
                backup_id: backup_id.to_string(),
            }
            .into())
        }
        _ => Ok(()),
    }
}

/// Check again, under the lock, and plan the backup `item` asked for, of
/// the set `found`, then restore it, or in a dry run say what a restore
/// would do (see [`plan_found_set`] and [`run_planned_restore`]).
fn restore_found_set(
    resource_path: &Path,
    item: &RestoreItem,
    found: FoundSet<'_>,
    options: &RestoreBackupOptions,
    outside: &OutsideRestoreRules,
    check: CopyCheck,
) -> ItemOutcome {
    match plan_found_set(
        resource_path,
        item.fingerprint.as_ref(),
        found,
        options,
        outside,
        check,
    ) {
        Ok(planned) => ItemOutcome::Ran(Box::new(run_planned_restore(
            resource_path,
            planned,
            options.dry_run,
        ))),
        Err(error) => ItemOutcome::NotStarted(as_restore_refusal(error)),
    }
}

/// `error` as the refusal it is, for the errors a restore shares with
/// RABBIT's other runs, which say them in words of their own: a symbolic
/// link or junction where RABBIT keeps its backups
/// ([`RestoreRefusal::IsLink`]), and the lock another RABBIT holds
/// ([`RestoreRefusal::LockHeld`]). Anything else stays as it is. Only for
/// an error from before a restore changes anything (see
/// [`restore_backup_set_with`]).
fn as_restore_refusal(error: RabbitError) -> RabbitError {
    match error {
        RabbitError::BackupPathIsLink { path } => RestoreRefusal::IsLink { path }.into(),
        RabbitError::PackageInstallInProgress { lock_path, pid } => {
            RestoreRefusal::LockHeld { lock_path, pid }.into()
        }
        other => other,
    }
}

/// Refuse a set at `backup_path` that belongs to another account than
/// `RABBIT/backups`, as one made under `sudo` does on a Mac, in a dry run
/// too: the overview of the backups compares the same owners, so a backup
/// listed as another account's is never restored. Windows has no owner to
/// compare.
fn refuse_another_accounts_set(
    resource_path: &Path,
    backup_id: &str,
    backup_path: &Path,
    environment: &BackupEnvironment<'_>,
) -> Result<()> {
    let expected = (environment.owner_of)(&backups_root(resource_path));
    let owner = (environment.owner_of)(backup_path);
    match (expected, owner) {
        (Some(expected), Some(owner)) if expected != owner => Err(RestoreRefusal::NotYours {
            backup_id: backup_id.to_string(),
        }
        .into()),
        _ => Ok(()),
    }
}

/// The manifest of the set `backup_id` at `backup_path`, or `None` without
/// one. A manifest that can't be read as one is refused
/// ([`RestoreRefusal::DamagedManifest`]); any other error stays as it is.
fn load_manifest_to_restore(backup_id: &str, backup_path: &Path) -> Result<Option<BackupManifest>> {
    load_backup_manifest(backup_path).map_err(|error| match error {
        RabbitError::Json { path, source } => RestoreRefusal::DamagedManifest {
            backup_id: backup_id.to_string(),
            path,
            detail: source.to_string(),
        }
        .into(),
        other => other,
    })
}

/// The refusal a restore gives the set `backup_id` at `backup_path` for its
/// manifest, if it gives one: a manifest that can't be read as one, or one
/// that is a symbolic link or junction. The overview of the backups gives a
/// damaged set's row the same.
pub(crate) fn manifest_refusal(backup_id: &str, backup_path: &Path) -> Option<RestoreRefusal> {
    match load_manifest_to_restore(backup_id, backup_path).map_err(as_restore_refusal) {
        Err(RabbitError::RestoreRefused(refusal)) => Some(refusal),
        _ => None,
    }
}

/// Refuse a set whose restore would put back an earlier copy of RABBIT's
/// receipt, or files that only go with one:
/// - a set an update wrote, whether or not it holds a receipt copy: its
///   files belong to the receipt entry from before the update, which no
///   restore puts back yet;
/// - a set that holds only a copy of the receipt;
/// - any other set that holds a copy of it, by its manifest or, without
///   one, by its layout (see [`set_holds_receipt`]).
///
/// [`restore_target`] never puts a file back into RABBIT's folder anyway.
/// This says why in plain words, and refuses the whole set before any file
/// of it is read or changed.
fn refuse_sets_a_restore_does_not_take(
    resource_path: &Path,
    backup_id: &str,
    backup_path: &Path,
    manifest: Option<&BackupManifest>,
) -> Result<()> {
    let backup_id = backup_id.to_string();
    let refusal = match manifest.map(|manifest| manifest.reason.as_str()) {
        Some(UPDATE_BACKUP_REASON) => RestoreRefusal::EarlierVersionNotYet { backup_id },
        Some(RECORDS_BACKUP_REASON) => RestoreRefusal::RecordsOnly { backup_id },
        _ if set_holds_receipt(resource_path, backup_path, manifest)? => {
            RestoreRefusal::ReceiptInSet {
                backup_id,
                backup_path: backup_path.to_path_buf(),
            }
        }
        _ => return Ok(()),
    };
    Err(refusal.into())
}

/// Whether a set holds a copy of RABBIT's receipt. With a manifest: an
/// entry whose original, read the way [`restore_target`] reads it, is the
/// receipt, or a `receipt_backup_path`. Without one: the receipt's place in
/// the set's layout, which mirrors the resource folder. A name the file
/// system would open as the receipt's counts too (see [`may_open_as`]).
fn set_holds_receipt(
    resource_path: &Path,
    backup_path: &Path,
    manifest: Option<&BackupManifest>,
) -> Result<bool> {
    let Some(manifest) = manifest else {
        let mut files = Vec::new();
        let mut links = Vec::new();
        collect_relative_files(backup_path, backup_path, &mut files, &mut links)?;
        return Ok(files.iter().any(|relative| is_receipt(relative)));
    };
    Ok(manifest_holds_receipt(resource_path, manifest))
}

/// Whether `manifest` names a copy of RABBIT's receipt: a
/// `receipt_backup_path`, or an entry whose original is the receipt (see
/// [`set_holds_receipt`]).
pub(crate) fn manifest_holds_receipt(resource_path: &Path, manifest: &BackupManifest) -> bool {
    manifest.receipt_backup_path.is_some()
        || manifest
            .files
            .iter()
            .any(|entry| is_receipt_entry(resource_path, entry))
}

/// Whether `entry`'s original, read the way [`restore_target`] reads it,
/// is RABBIT's receipt. A path this can't place is refused by
/// `restore_target` too.
pub(crate) fn is_receipt_entry(resource_path: &Path, entry: &BackupManifestFile) -> bool {
    matches!(
        entry_place(resource_path, entry),
        Some(EntryPlace::Inside(relative)) if is_receipt(&relative)
    )
}

/// Where a set's entry came from, and where a restore puts it back: inside
/// the resource folder, by its path there, or outside it, by its absolute
/// path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryPlace {
    Inside(PathBuf),
    Outside(PathBuf),
}

impl EntryPlace {
    /// The place as a path a restore would write: inside the resource
    /// folder at `resource_path`, or the outside path as recorded.
    pub(crate) fn path(&self, resource_path: &Path) -> PathBuf {
        match self {
            Self::Inside(relative) => resource_path.join(relative),
            Self::Outside(absolute) => absolute.clone(),
        }
    }
}

/// Where `entry`'s file came from, read the way a restore reads it (see
/// [`recorded_original`] and [`inside_resource_folder`]), also for sets an
/// older RABBIT wrote while it had a relative resource path. `None` for a
/// relative path that isn't a plain path, which a restore refuses. Nothing
/// on disk is read.
pub(crate) fn entry_place(resource_path: &Path, entry: &BackupManifestFile) -> Option<EntryPlace> {
    let (original_path, recorded) = recorded_original(entry);
    match inside_resource_folder(
        resource_path,
        &original_path,
        recorded
            .as_ref()
            .map(|recorded| recorded.resource.as_path()),
    ) {
        Ok(Some(relative)) => Some(EntryPlace::Inside(relative)),
        Ok(None) => Some(EntryPlace::Outside(original_path)),
        Err(_) => None,
    }
}

/// Where in the set at `set_path` the copy is that a manifest recorded at
/// `recorded_backup_path`, found the way a restore finds it (see
/// [`restore_source`]): always inside the set, and never through a
/// symbolic link or junction there. Only metadata is read.
pub(crate) fn copy_in_set(set_path: &Path, recorded_backup_path: &Path) -> Result<PathBuf> {
    restore_source(
        set_path,
        recorded_backup_path,
        recorded_set_location(recorded_backup_path).as_ref(),
    )
}

/// Where outside the resource folder a restore may put a file back.
#[derive(Debug, Clone, Default)]
pub(crate) struct OutsideRestoreRules {
    /// Folders RABBIT copies files into itself, such as the CLAP folder:
    /// any package's file may go back below one.
    pub shared_roots: Vec<PathBuf>,
    /// Each package's `vendor_added_files` patterns, with its id: a file
    /// goes back where one matches only if it belongs to that package, and
    /// only through real folders inside the pattern's guard.
    pub vendor_added: Vec<(String, ResolvedPattern)>,
}

/// [`restore_backup_set`] with the places outside the resource folder that
/// a restore may write to passed in, so tests can use temporary folders
/// instead of the user's real CLAP and vendor folders. It runs as the user
/// ([`BackupEnvironment::AS_USER`]), on every platform.
#[cfg(test)]
pub(crate) fn restore_backup_set_within(
    resource_path: &Path,
    backup_id: &str,
    options: &RestoreBackupOptions,
    outside: &OutsideRestoreRules,
) -> Result<RestoreBackupReport> {
    restore_backup_set_with(
        resource_path,
        backup_id,
        options,
        &BackupEnvironment::AS_USER,
        || outside.clone(),
    )
}

/// A set a restore found and read, and did not refuse: its id, the package
/// asked for, if one was, its folder in `RABBIT/backups`, its manifest, and
/// the packages it brings back.
struct FoundSet<'a> {
    backup_id: &'a str,
    package_id: Option<&'a str>,
    backup_path: PathBuf,
    manifest: Option<BackupManifest>,
    packages: PackagesToRestore,
}

impl FoundSet<'_> {
    /// The packages whose files the restore puts back (see
    /// [`restored_package_ids`]), whose uninstall's checks it makes.
    fn restored_package_ids(&self) -> Vec<String> {
        restored_package_ids(self.manifest.as_ref(), &self.packages, self.package_id)
    }
}

/// A backup checked and planned under the lock, with nothing of it changed
/// yet: what [`run_planned_restore`] needs.
struct PlannedBackup {
    backup_id: String,
    package_id: Option<String>,
    backup_path: PathBuf,
    packages: Vec<(String, BackupPackageRecord)>,
    skipped_packages: Vec<SkippedPackage>,
    /// RABBIT's receipt, read when a package's records come back.
    state: Option<InstallState>,
    actions: Vec<RestoreBackupAction>,
    skipped_targets: Vec<PathBuf>,
    before_restore_id: String,
}

/// Check again and plan the restore of the set `found`, writing outside the
/// resource folder only where `outside` allows, and change nothing. A real
/// run holds the lock by now; a dry run gets here at the same point.
///
/// A set an uninstall wrote records each package it removed. Its files come
/// back like any set's, but the receipt doesn't: the uninstall never copied
/// the receipt file, which has moved on since anyway. Each package's own
/// entry is merged back into the current receipt instead (unless it is
/// still there, from an uninstall that never got that far), the package
/// comes off `declined_packages`, and REAPER's language goes back if the
/// uninstall reset it. That is only for `packages.restore`: the packages
/// in `packages.skipped` have no file in the set, and their records stay
/// as they are. With `options.package_id`, only that package comes back.
fn plan_found_set(
    resource_path: &Path,
    listed: Option<&SetFingerprint>,
    found: FoundSet<'_>,
    options: &RestoreBackupOptions,
    outside: &OutsideRestoreRules,
    check: CopyCheck,
) -> Result<PlannedBackup> {
    let FoundSet {
        backup_id,
        package_id,
        backup_path,
        manifest,
        packages:
            PackagesToRestore {
                restore: packages,
                skipped: skipped_packages,
            },
    } = found;

    // The set may have changed since the checks before the lock, as when a
    // backup before it in the run took long. So it is found again here, as
    // `RABBIT/backups` lists it, right before its turn: a set, or a
    // `RABBIT` or `RABBIT/backups`, that became a symbolic link or junction
    // since, or a set that went, is refused before anything of it is read
    // through it. Then its fingerprint is compared again.
    if set_to_restore(resource_path, backup_id)? != backup_path {
        return Err(backup_changed_while_read(backup_id));
    }
    refuse_changed_since_listed(backup_id, &backup_path, listed)?;

    // The packages and the refusals above come from the manifest as it was
    // read before the lock. An uninstall that was still writing the set may
    // have recorded more files since, and a plan from the old manifest would
    // miss them: a package's entry would be merged back with most of its
    // files still in the set. So the manifest is read again here, under the
    // lock, and in a dry run at the same point; any change stops the
    // restore with nothing changed.
    if load_manifest_to_restore(backup_id, &backup_path)? != manifest {
        return Err(backup_changed_while_read(backup_id));
    }

    // A package installed again since its uninstall has a newer receipt
    // entry describing newer files. Putting the old files under it would
    // leave a receipt that is wrong about both, so that stops the restore
    // before anything changes, like a refused file. The receipt is read
    // here, as the backups before this one in the run left it.
    //
    // An entry that is still the one the set recorded is no reinstall. The
    // uninstall stopped before it reached the receipt: RABBIT was ended in
    // the middle of the package, or a failed package's files couldn't all be
    // moved back. Or an earlier restore from this set merged the entry back
    // already. Either way the set's files are the ones the entry describes,
    // and the write-ahead manifest exists so that they can come back.
    //
    // A package left out, with none of its files in the set, puts no old
    // file under anything, so a newer entry for it refuses nothing.
    let state = if packages.is_empty() {
        None
    } else {
        let state = load_install_state(resource_path)
            .map_err(|error| RestoreRefusal::ReceiptUnreadable {
                detail: error.to_string(),
            })?
            .unwrap_or_default();
        if let Some((reinstalled, _)) = packages.iter().find(|(package_id, record)| {
            state
                .packages
                .get(package_id)
                .is_some_and(|current| record.receipt.as_ref() != Some(current))
        }) {
            // Only packages with a file in the set restore on their own; a
            // package left out would be refused alone.
            let other_packages = if package_id.is_none() {
                packages
                    .iter()
                    .map(|(other, _)| other.clone())
                    .filter(|other| other != reinstalled)
                    .collect()
            } else {
                Vec::new()
            };
            return Err(RestoreRefusal::ReinstalledSince {
                backup_id: backup_id.to_string(),
                package_id: reinstalled.clone(),
                other_packages,
            }
            .into());
        }
        Some(state)
    };

    // Every file is checked before the first one is touched, so a refused
    // or damaged entry stops the restore with nothing changed.
    let restoring: Vec<&str> = packages
        .iter()
        .map(|(package_id, _)| package_id.as_str())
        .collect();
    let plan = plan_restore(
        resource_path,
        &backup_path,
        manifest.as_ref(),
        package_id,
        &restoring,
        outside,
        check,
    )?;
    check_plan_matches_packages(backup_id, &plan, &restoring, &skipped_packages)?;

    let before_restore_id = format!("{}-before-restore", restore_timestamp());
    let mut planned_backups =
        BackupSetWriter::planning(resource_path, &before_restore_id, BEFORE_RESTORE_REASON);
    let mut actions = Vec::new();
    for file in plan.files {
        let current_backup_path = if is_real_file(&file.target_path) {
            Some(planned_backups.copy_file(file.package_id.as_deref(), &file.target_path)?)
        } else {
            None
        };
        actions.push(RestoreBackupAction {
            package_id: file.package_id,
            source_path: file.source_path,
            target_path: file.target_path,
            current_backup_path,
            // A real restore marks each file as it goes.
            action: if options.dry_run {
                RestoreBackupActionKind::WouldRestore
            } else {
                RestoreBackupActionKind::NotRestored
            },
            size: file.size,
            sha256: file.sha256,
            error: None,
        });
    }

    Ok(PlannedBackup {
        backup_id: backup_id.to_string(),
        package_id: package_id.map(ToString::to_string),
        backup_path,
        packages,
        skipped_packages,
        state,
        actions,
        skipped_targets: plan.skipped_targets,
        before_restore_id,
    })
}

/// Put back what `planned` planned, then merge the restored packages'
/// records, and report; or with `dry_run` change nothing and say what a
/// restore would do. The caller holds the lock.
///
/// All or nothing: each file the restore replaces goes into the backup's
/// before-restore set first, and when a file fails, or the records can't be
/// saved, every file the restore changed goes back as it was, newest first
/// (see [`put_back_what_changed`]). The records are merged only once every
/// file is back, so they never claim a file that isn't. A set whose every
/// copy went back where it came from holds nothing anyone needs, and goes
/// again; one that holds the only copy of a file that couldn't be put back
/// stays, and the report names it.
fn run_planned_restore(
    resource_path: &Path,
    planned: PlannedBackup,
    dry_run: bool,
) -> RestoreReportItem {
    let PlannedBackup {
        backup_id,
        package_id,
        backup_path,
        packages,
        skipped_packages,
        state,
        actions,
        skipped_targets,
        before_restore_id,
    } = planned;
    let mut report = RestoreReportItem {
        backup_id,
        package_id,
        status: RestoreStatus::WouldRestore,
        backup_path: Some(backup_path),
        actions,
        skipped_targets,
        packages: Vec::new(),
        skipped_packages,
        before_restore_set_id: None,
        refusal: None,
        error: None,
    };
    if dry_run {
        if let Some(state) = state {
            report.packages =
                package_records_a_restore_would_restore(resource_path, &packages, &state);
        }
        return report;
    }

    // The files a restore replaces go into a set of their own, written like
    // any other, so a restore can be undone the same way.
    let mut backups =
        BackupSetWriter::new(resource_path, &before_restore_id, BEFORE_RESTORE_REASON);
    let mut steps: Vec<FileStep> = Vec::with_capacity(report.actions.len());
    let mut failure = None;
    for action in &mut report.actions {
        let mut step = FileStep::default();
        let restored = restore_file(action, &mut backups, &mut step);
        steps.push(step);
        match restored {
            Ok(()) => action.action = RestoreBackupActionKind::Restored,
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    // After the files, so the receipt never claims a file that isn't back.
    // The receipt is saved whole or not at all, so a failure here leaves it
    // as it was, and the files go back too.
    if failure.is_none()
        && let Some(state) = state
    {
        // Tests may fail the save here, as a full disk would.
        #[cfg(test)]
        let seam = {
            let receipt = resource_path.join(RECEIPT_RELATIVE_PATH);
            restore_seam::at(restore_seam::Step::SavingRecords, &receipt).with_path(&receipt)
        };
        #[cfg(not(test))]
        let seam: Result<()> = Ok(());
        match seam.and_then(|()| restore_package_records(resource_path, &packages, state)) {
            Ok(restored) => report.packages = restored,
            Err(error) => failure = Some(error),
        }
    }

    match failure {
        None => {
            report.status = RestoreStatus::Restored;
            // The writer makes the set with the first file it backs up, and
            // may have named it `<id>-2`. A restore that replaced nothing
            // made none.
            report.before_restore_set_id = backups.is_started().then(|| backups.set_id());
        }
        Some(error) => {
            report.error = Some(error.to_string());
            let all_back = put_back_what_changed(&mut report.actions, &steps, &backups);
            report.status = if all_back {
                RestoreStatus::NotRestored
            } else {
                RestoreStatus::FilesLeft
            };
            let discarded = all_back && backups.discard_copies();
            // A copy that went with the set is named no more.
            for action in &mut report.actions {
                if action
                    .current_backup_path
                    .as_deref()
                    .is_some_and(|copy| !is_real_file(copy))
                {
                    action.current_backup_path = None;
                }
            }
            report.before_restore_set_id =
                (backups.is_started() && !discarded).then(|| backups.set_id());
        }
    }
    report
}

/// Save `report` as `RABBIT/logs/restore-backup-unix-<time>.json` and
/// `.txt` in `resource_path`, or `-2` and so on when a restore in the same
/// second saved one: each report is the record of the sets that undo its
/// restore, and of what went back after a failure.
fn save_restore_report<T: Serialize + ?Sized>(
    resource_path: &Path,
    report: &T,
) -> Result<SavedReportPaths> {
    save_new_json_and_text_reports(
        &default_report_path(resource_path, "restore-backup"),
        report,
    )
}

/// The packages whose files a restore of the set puts back, by its
/// manifest: those of an uninstall's set it brings back, and each package an
/// entry names, of only `package_id` when one was asked for. A package left
/// out, with none of its files in the set, puts nothing back. A set without
/// a manifest names no package.
fn restored_package_ids(
    manifest: Option<&BackupManifest>,
    packages: &PackagesToRestore,
    package_id: Option<&str>,
) -> Vec<String> {
    let mut ids: std::collections::BTreeSet<String> = packages
        .restore
        .iter()
        .map(|(package_id, _)| package_id.clone())
        .collect();
    if let Some(manifest) = manifest {
        ids.extend(
            manifest
                .files
                .iter()
                .filter_map(|entry| entry.package_id.as_ref())
                .filter(|id| package_id.is_none_or(|wanted| wanted == id.as_str()))
                .filter(|id| {
                    !packages
                        .skipped
                        .iter()
                        .any(|skipped| skipped.package_id == **id)
                })
                .cloned(),
        );
    }
    ids.into_iter().collect()
}

/// What a restore of some packages needs closed beyond the target's
/// REAPER, by what each package's own uninstall needs closed (see
/// [`crate::uninstall::plan::uninstall_process_rules`]).
#[derive(Debug, Default, PartialEq, Eq)]
struct RestoreProcessRules {
    /// The packages whose uninstall needs every REAPER on the computer
    /// closed, portable ones included.
    every_reaper_for: Vec<NamedPackage>,
    /// Each package whose uninstall plan names programs that must not run,
    /// with those programs.
    blocking: Vec<(NamedPackage, Vec<String>)>,
}

/// A package by its id, and by the name RABBIT shows for it, which the
/// English of a refusal says, as an uninstall's checks do.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedPackage {
    id: String,
    name: String,
}

/// The ids of `packages`, and their names, in the same order.
fn ids_and_names(packages: &[NamedPackage]) -> (Vec<String>, Vec<String>) {
    packages
        .iter()
        .map(|package| (package.id.clone(), package.name.clone()))
        .unzip()
}

/// The rules of the packages `package_ids` for the REAPER at
/// `resource_path`, from the package manifests for this platform and that
/// kind of REAPER, as an uninstall reads them. A package RABBIT doesn't
/// know adds nothing.
fn restore_process_rules(
    resource_path: &Path,
    package_ids: &[String],
    environment: &BackupEnvironment<'_>,
) -> RestoreProcessRules {
    let mut rules = RestoreProcessRules::default();
    let Some(platform) = Platform::current() else {
        return rules;
    };
    if package_ids.is_empty() {
        return rules;
    }
    let specs = (environment.package_specs)(platform);
    let target = std::cell::OnceCell::new();
    for package_id in package_ids {
        let Some(spec) = specs.iter().find(|spec| spec.id == *package_id) else {
            continue;
        };
        let target = *target.get_or_init(|| (environment.target_kind)(platform, resource_path));
        let (every_reaper_closed, programs) =
            crate::uninstall::plan::uninstall_process_rules(spec, platform, target);
        let package = NamedPackage {
            id: package_id.clone(),
            name: spec.display_name.clone(),
        };
        if every_reaper_closed {
            rules.every_reaper_for.push(package.clone());
        }
        if !programs.is_empty() {
            rules.blocking.push((package, programs));
        }
    }
    rules
}

/// What runs that a restore must have closed, by one scan of every REAPER
/// and, when a package names any, one of its programs, through the
/// restore's environment.
struct RunningBlockers {
    /// The target's REAPER, among every REAPER running.
    target_reapers: Vec<RunningProcess>,
    /// Any REAPER running, refused for the packages whose uninstall needs
    /// every REAPER closed. `None` when no REAPER runs or no package needs
    /// that.
    every_reaper: Option<RestoreRefusal>,
    /// The programs the packages' uninstall plans name, each once.
    programs_named: Vec<String>,
    /// Those of them that run, refused for the packages that name them.
    programs: Option<RestoreRefusal>,
}

impl RunningBlockers {
    /// What `allow_reaper_running` never lets through: any REAPER for a
    /// package that needs every REAPER closed, then the programs.
    fn beyond_target(&self) -> Option<RestoreRefusal> {
        self.every_reaper.clone().or_else(|| self.programs.clone())
    }

    /// The target's REAPER, which `options.allow_reaper_running` lets run.
    /// `None` in a dry run, which refuses nothing for what runs.
    fn target(&self, options: &RestoreBackupOptions) -> Option<RestoreRefusal> {
        (!options.dry_run && !options.allow_reaper_running && !self.target_reapers.is_empty()).then(
            || RestoreRefusal::ReaperRunning {
                every_reaper_for: Vec::new(),
                package_names: Vec::new(),
                processes: self.target_reapers.clone(),
            },
        )
    }

    /// The refusal of a restore for what runs, the target's REAPER last.
    /// `None` in a dry run.
    fn refusal(&self, options: &RestoreBackupOptions) -> Option<RestoreRefusal> {
        if options.dry_run {
            return None;
        }
        self.beyond_target().or_else(|| self.target(options))
    }
}

/// Scan what runs that the packages with `rules` need closed, for the
/// REAPER at `resource_path`. Every REAPER is scanned once; the programs
/// only when a package names any.
fn running_blockers(
    resource_path: &Path,
    rules: &RestoreProcessRules,
    environment: &BackupEnvironment<'_>,
) -> RunningBlockers {
    let reapers = (environment.reaper_processes)();
    let target_reapers = running_reaper_for_target_with_processes(resource_path, None, &reapers);
    let every_reaper = (!rules.every_reaper_for.is_empty() && !reapers.is_empty()).then(|| {
        let (every_reaper_for, package_names) = ids_and_names(&rules.every_reaper_for);
        RestoreRefusal::ReaperRunning {
            every_reaper_for,
            package_names,
            processes: reapers,
        }
    });

    let mut programs_named: Vec<String> = rules
        .blocking
        .iter()
        .flat_map(|(_, programs)| programs.iter().cloned())
        .collect();
    programs_named.sort();
    programs_named.dedup();
    let running: Vec<RunningProcess> = if programs_named.is_empty() {
        Vec::new()
    } else {
        (environment.named_processes)(&programs_named)
            .into_iter()
            .filter(|process| is_one_of(&process.name, &programs_named))
            .collect()
    };
    let programs = (!running.is_empty()).then(|| {
        let naming: Vec<NamedPackage> = rules
            .blocking
            .iter()
            .filter(|(_, programs)| {
                running
                    .iter()
                    .any(|process| is_one_of(&process.name, programs))
            })
            .map(|(package, _)| package.clone())
            .collect();
        let (package_ids, package_names) = ids_and_names(&naming);
        let mut programs: Vec<String> = running.into_iter().map(|process| process.name).collect();
        programs.sort();
        programs.dedup();
        RestoreRefusal::ProgramRunning {
            package_ids,
            package_names,
            programs,
        }
    });

    RunningBlockers {
        target_reapers,
        every_reaper,
        programs_named,
        programs,
    }
}

/// The checks before a restore of the packages with `rules` changes
/// anything, as the report's `preflight` lists them, and the refusal they
/// give the restore. Each failed check gives one, so the refusal is `None`
/// exactly when the report passed.
///
/// They are the install preflight's checks, whose REAPER check looks at the
/// target's REAPER and is worded for a restore, and what each package's own
/// uninstall checks besides (see [`restore_process_rules`]): every REAPER
/// on the computer for a package whose uninstall needs that, and the
/// programs its uninstall plan names. Each of these says what its refusal
/// says. `options.allow_reaper_running` lets only the target's REAPER run.
/// A dry run reports what is running, as a warning, and refuses nothing
/// for it, as the install preflight's dry run always did; the other checks
/// fail it as before.
///
/// The refusal is for one failed check: first what
/// `options.allow_reaper_running` never lets through, then a place the
/// restore can't write to, and only then the target's REAPER. So a refusal
/// for the target's REAPER, which `allow_reaper_running` lets through,
/// comes only when nothing else stands in the way. A dry run's report
/// names everything that must be closed.
fn restore_preflight(
    resource_path: &Path,
    rules: &RestoreProcessRules,
    options: &RestoreBackupOptions,
    environment: &BackupEnvironment<'_>,
) -> (PreflightReport, Option<RestoreRefusal>) {
    let blockers = running_blockers(resource_path, rules, environment);
    let mut report = run_restore_preflight_with_processes(
        resource_path,
        &PreflightOptions {
            dry_run: options.dry_run,
            allow_reaper_running: options.allow_reaper_running,
            target_app_path: None,
        },
        &blockers.target_reapers,
    );

    let held = if options.dry_run {
        PreflightStatus::Warn
    } else {
        PreflightStatus::Fail
    };
    let check = |name: &str, refusal: Option<&RestoreRefusal>, passed: String| match refusal {
        None => PreflightCheck {
            name: name.to_string(),
            status: PreflightStatus::Pass,
            message: passed,
        },
        Some(refusal) => PreflightCheck {
            name: name.to_string(),
            status: held,
            message: format!("{refusal}."),
        },
    };
    if !rules.every_reaper_for.is_empty() {
        report.checks.push(check(
            ANY_REAPER_PROCESS_CHECK,
            blockers.every_reaper.as_ref(),
            NO_REAPER_ANYWHERE.to_string(),
        ));
    }
    if !blockers.programs_named.is_empty() {
        report.checks.push(check(
            PROGRAMS_RUNNING_CHECK,
            blockers.programs.as_ref(),
            none_of_these_programs_running(&blockers.programs_named),
        ));
    }
    report.passed = report
        .checks
        .iter()
        .all(|check| check.status != PreflightStatus::Fail);
    if report.passed {
        return (report, None);
    }

    let beyond_target = if options.dry_run {
        None
    } else {
        blockers.beyond_target()
    };
    let refusal = beyond_target
        .or_else(|| failed_check_refusal(resource_path, &report))
        .or_else(|| blockers.target(options));
    (report, refusal)
}

/// The refusal for the first check of `report` that failed for something
/// other than what runs: `RABBIT` or `RABBIT/backups` a symbolic link or
/// junction ([`RestoreRefusal::IsLink`]), or a place the restore can't
/// write to ([`RestoreRefusal::NotWritable`]).
fn failed_check_refusal(resource_path: &Path, report: &PreflightReport) -> Option<RestoreRefusal> {
    let about_what_runs = [
        REAPER_PROCESS_CHECK,
        ANY_REAPER_PROCESS_CHECK,
        PROGRAMS_RUNNING_CHECK,
    ];
    let failed = report.checks.iter().find(|check| {
        check.status == PreflightStatus::Fail && !about_what_runs.contains(&check.name.as_str())
    })?;
    if failed.name == BACKUP_FOLDER_CHECK {
        // `set_to_restore` refuses a linked folder before this, so only one
        // that became a link since gets here.
        return Some(RestoreRefusal::IsLink {
            path: linked_backups_folder(resource_path)
                .unwrap_or_else(|| backups_root(resource_path)),
        });
    }
    Some(RestoreRefusal::NotWritable {
        path: resource_path.to_path_buf(),
        check: failed.name.clone(),
        detail: failed.message.trim_end_matches('.').to_string(),
    })
}

/// The packages of an uninstall set a restore brings back, with what the
/// set recorded about each, and those it leaves out. See
/// [`packages_to_restore`].
#[derive(Debug, Default)]
struct PackagesToRestore {
    restore: Vec<(String, BackupPackageRecord)>,
    skipped: Vec<SkippedPackage>,
}

/// The packages of an uninstall set this restore brings back, with what the
/// set recorded about each: every one of them, or only `package_id`. Empty
/// for every other kind of set, whose files are all a restore needs. The
/// copies are read as `check` says.
///
/// An uninstall records a package before it moves anything, and keeps the
/// record even when it found none of the package's files to move, or
/// stopped before any copy of them finished. A package counts as having a
/// file in the set when the restore puts back one of its entries (see
/// [`entry_comes_back`]). One without is refused on its own
/// ([`RestoreRefusal::NothingOfItInSet`]), and a restore of the whole set
/// leaves it out: its receipt entry isn't merged, it stays on
/// `declined_packages`, its language isn't put back, and the report names
/// it. Merging its entry would make RABBIT count it as installed with no
/// files there. A whole set with no file at all to put back is refused the
/// same way, as a set that holds only a copy of the receipt is: a restore
/// that brings nothing back is no restore. All before anything changes, in
/// a dry run too. The plan, made under the lock, is checked against this
/// again (see [`check_plan_matches_packages`]).
fn packages_to_restore(
    backup_id: &str,
    backup_path: &Path,
    manifest: Option<&BackupManifest>,
    package_id: Option<&str>,
    check: CopyCheck,
) -> Result<PackagesToRestore> {
    let Some(manifest) = manifest.filter(|manifest| !manifest.packages.is_empty()) else {
        let Some(package_id) = package_id else {
            return Ok(PackagesToRestore::default());
        };
        // Only an uninstall set knows which receipt entry goes with a
        // package. Restoring one package's files from any other set would
        // leave the receipt describing other files. An update's set never
        // gets here.
        let backup_id = backup_id.to_string();
        let package_id = package_id.to_string();
        return Err(match manifest {
            None => RestoreRefusal::NoManifest {
                backup_id,
                package_id,
            },
            Some(_) => RestoreRefusal::NoPackageRecords {
                backup_id,
                package_id,
            },
        }
        .into());
    };
    let has_file = |package_id: &str| holds_a_file_of(backup_path, manifest, package_id, check);
    let nothing_of = |package_ids: Vec<String>| -> RabbitError {
        RestoreRefusal::NothingOfItInSet {
            backup_id: backup_id.to_string(),
            package_ids,
        }
        .into()
    };

    if let Some(package_id) = package_id {
        let Some(record) = manifest.packages.get(package_id) else {
            return Err(RestoreRefusal::PackageNotInSet {
                backup_id: backup_id.to_string(),
                package_id: package_id.to_string(),
                packages: manifest.packages.keys().cloned().collect(),
            }
            .into());
        };
        if !has_file(package_id) {
            return Err(nothing_of(vec![package_id.to_string()]));
        }
        return Ok(PackagesToRestore {
            restore: vec![(package_id.to_string(), record.clone())],
            skipped: Vec::new(),
        });
    }

    let mut packages = PackagesToRestore::default();
    for (package_id, record) in &manifest.packages {
        if has_file(package_id) {
            packages.restore.push((package_id.clone(), record.clone()));
        } else {
            packages.skipped.push(SkippedPackage {
                package_id: package_id.clone(),
                reason: SkippedPackageReason::NothingOfItInSet,
            });
        }
    }
    // With no package to restore, only an entry of no recorded package can
    // still come back. Those of the skipped packages can't, by definition.
    let anything_else_comes_back = || {
        manifest.files.iter().any(|entry| {
            entry
                .package_id
                .as_ref()
                .is_none_or(|package_id| !manifest.packages.contains_key(package_id))
                && entry_comes_back(backup_path, entry, check)
        })
    };
    if packages.restore.is_empty() && !anything_else_comes_back() {
        return Err(nothing_of(
            packages
                .skipped
                .into_iter()
                .map(|skipped| skipped.package_id)
                .collect(),
        ));
    }
    Ok(packages)
}

/// Whether `manifest`, that of the set at `backup_path`, holds a file of
/// `package_id` that a restore puts back (see [`entry_comes_back`]), its
/// copies read as `check` says. A restore and an uninstall's report both
/// judge a package by it.
fn holds_a_file_of(
    backup_path: &Path,
    manifest: &BackupManifest,
    package_id: &str,
    check: CopyCheck,
) -> bool {
    manifest.files.iter().any(|entry| {
        entry.package_id.as_deref() == Some(package_id)
            && entry_comes_back(backup_path, entry, check)
    })
}

/// Whether a restore puts `entry`'s copy back, by the rule
/// [`plan_restore_from_manifest`] follows (see [`copy_to_restore`]): a
/// finished entry always counts, since the restore puts it back or stops;
/// a pending one only when its copy is complete, by its hash or, for the
/// quick plan, its size (see [`CopyCheck`]). A pending entry whose copy
/// can't be checked counts too, so the restore reaches it and says what is
/// wrong in its usual order. Only a pending entry's copy is read.
fn entry_comes_back(backup_path: &Path, entry: &BackupManifestFile, check: CopyCheck) -> bool {
    if entry.state == BackupEntryState::Done {
        return true;
    }
    let recorded = recorded_set_location(&entry.backup_path);
    !matches!(
        copy_to_restore(backup_path, entry, recorded.as_ref(), check),
        Ok(None)
    )
}

/// Refuse a plan that disagrees with [`packages_to_restore`] about which
/// packages have a file in the set: each package in `restoring` needs a
/// file in the plan, a package in `skipped` none, and a set whose packages
/// were all left out a file of no package.
///
/// Both follow the same rule, but [`packages_to_restore`] reads the set
/// before the lock, and the plan is made under it. The manifest is read
/// again under the lock and must be the same (see [`plan_found_set`]),
/// but an uninstall that was still writing the set may have finished a
/// pending entry's copy in between, which changes no manifest. Going ahead
/// would then put back a file of a package whose records stay as they are,
/// or, had a copy gone, merge a receipt entry with none of its files back.
/// Nothing has changed yet.
fn check_plan_matches_packages(
    backup_id: &str,
    plan: &RestorePlan,
    restoring: &[&str],
    skipped: &[SkippedPackage],
) -> Result<()> {
    let planned = |package_id: &str| {
        plan.files
            .iter()
            .any(|file| file.package_id.as_deref() == Some(package_id))
    };
    let all_left_out = restoring.is_empty() && !skipped.is_empty();
    if restoring.iter().all(|package_id| planned(package_id))
        && !skipped.iter().any(|skipped| planned(&skipped.package_id))
        && !(all_left_out && plan.files.is_empty())
    {
        return Ok(());
    }
    Err(backup_changed_while_read(backup_id))
}

/// The refusal of a restore whose set changed between the reads before the
/// lock and those under it. Nothing has changed yet.
fn backup_changed_while_read(backup_id: &str) -> RabbitError {
    RestoreRefusal::ChangedWhileRead {
        backup_id: backup_id.to_string(),
    }
    .into()
}

/// What a restore does with each package's records, before it does it:
/// whose receipt entry it merges back and which `langpack=` value it puts
/// back. Nothing is written.
///
/// The caller has refused any package whose current entry differs from the
/// one its set recorded, so an entry still in `state` is that same one and
/// stays as it is.
fn package_records_to_restore(
    packages: &[(String, BackupPackageRecord)],
    state: &InstallState,
) -> Vec<RestoredPackage> {
    packages
        .iter()
        .map(|(package_id, record)| {
            let already_current = state.packages.contains_key(package_id);
            RestoredPackage {
                package_id: package_id.clone(),
                receipt_merged: record.receipt.is_some() && !already_current,
                receipt_already_current: already_current,
                langpack_before: record.langpack_before.clone(),
                langpack_restored: false,
                langpack_error: None,
            }
        })
        .collect()
}

/// What a dry run says a restore would do with each package's records
/// (see [`restore_package_records`]). Nothing is written.
fn package_records_a_restore_would_restore(
    resource_path: &Path,
    packages: &[(String, BackupPackageRecord)],
    state: &InstallState,
) -> Vec<RestoredPackage> {
    let mut restored = package_records_to_restore(packages, state);
    // Only the first value would go back: after it, the line no longer
    // says `<>`.
    let mut still_none = restored.iter().any(|item| item.langpack_before.is_some())
        && lang_pack_setting(resource_path).ok().flatten().as_deref() == Some("<>");
    for item in &mut restored {
        if item.langpack_before.is_some() {
            item.langpack_restored = still_none;
            still_none = false;
        }
    }
    restored
}

/// Merge each restored package's receipt entry back into `state`, take it
/// off `declined_packages`, save, and put back the `langpack=` value its
/// uninstall replaced (see [`package_records_to_restore`]).
fn restore_package_records(
    resource_path: &Path,
    packages: &[(String, BackupPackageRecord)],
    mut state: InstallState,
) -> Result<Vec<RestoredPackage>> {
    let mut restored = package_records_to_restore(packages, &state);
    for (package_id, record) in packages {
        // Restoring is the user's yes to the package, so it comes off
        // declined_packages even when its entry never left and there was
        // nothing to merge.
        if !merge_package_receipt(&mut state, package_id, record.receipt.clone()) {
            state.declined_packages.remove(package_id);
        }
    }
    save_install_state(resource_path, &state)?;
    // The package is back either way. A language that couldn't be put back
    // is reported, and the user can pick it in REAPER's preferences.
    for item in &mut restored {
        let Some(value) = &item.langpack_before else {
            continue;
        };
        match restore_lang_pack_if_cleared(resource_path, value) {
            Ok(done) => item.langpack_restored = done,
            Err(error) => item.langpack_error = Some(error.to_string()),
        }
    }
    Ok(restored)
}

struct RestorePlan {
    files: Vec<PlannedRestore>,
    skipped_targets: Vec<PathBuf>,
}

struct PlannedRestore {
    package_id: Option<String>,
    source_path: PathBuf,
    target_path: PathBuf,
    size: u64,
    sha256: String,
}

/// `package_id` limits the plan to that package's files. Only a manifest
/// records which package a file belongs to, and the caller has made sure
/// there is one. `restoring` are the packages the restore brings back, the
/// ones with a file in the set (see [`packages_to_restore`]). The copies
/// are read as `check` says.
fn plan_restore(
    resource_path: &Path,
    backup_path: &Path,
    manifest: Option<&BackupManifest>,
    package_id: Option<&str>,
    restoring: &[&str],
    outside: &OutsideRestoreRules,
    check: CopyCheck,
) -> Result<RestorePlan> {
    let plan = match manifest {
        Some(manifest) => plan_restore_from_manifest(
            resource_path,
            backup_path,
            manifest,
            package_id,
            restoring,
            outside,
            check,
        ),
        None => plan_restore_from_layout(resource_path, backup_path, check),
    }?;
    refuse_two_files_for_one_place(&plan)?;
    Ok(plan)
}

/// Refuse a plan that puts two files back at the same place, by names the
/// file system reads as the same (see [`same_place`]). RABBIT never writes
/// such a set, but a damaged manifest could name one file twice. The
/// second file would replace the first, and the restore's own backup of
/// what it replaced would then hold the first file instead of the one that
/// was there before the restore.
fn refuse_two_files_for_one_place(plan: &RestorePlan) -> Result<()> {
    let mut places = std::collections::BTreeSet::new();
    for file in &plan.files {
        if !places.insert(same_place(&file.target_path)) {
            return Err(TargetRefusal::TwoFilesForOnePlace
                .refuse(&file.target_path)
                .into());
        }
    }
    Ok(())
}

/// `path` as the file system tells places apart: by its components, with
/// upper and lower case alike on Windows and macOS, and `\\?\C:\` read as
/// `C:\`.
pub(crate) fn same_place(path: &Path) -> Vec<String> {
    comparable_components(path)
        .unwrap_or_else(|| vec![path.to_string_lossy().into_owned()])
        .into_iter()
        .map(|part| {
            if cfg!(target_os = "macos") {
                part.to_lowercase()
            } else {
                part
            }
        })
        .collect()
}

/// Each entry goes back to the `original_path` it records. Older sets mix
/// relative paths (the unattended key map and receipt backups) with
/// absolute ones (install replacements), so both are read here.
fn plan_restore_from_manifest(
    resource_path: &Path,
    backup_path: &Path,
    manifest: &BackupManifest,
    package_id: Option<&str>,
    restoring: &[&str],
    outside: &OutsideRestoreRules,
    check: CopyCheck,
) -> Result<RestorePlan> {
    let mut plan = RestorePlan {
        files: Vec::new(),
        skipped_targets: Vec::new(),
    };
    for entry in manifest
        .files
        .iter()
        .filter(|entry| package_id.is_none() || entry.package_id.as_deref() == package_id)
    {
        let (original_path, recorded) = recorded_original(entry);
        let target = || {
            restore_target(
                resource_path,
                &original_path,
                recorded
                    .as_ref()
                    .map(|recorded| recorded.resource.as_path()),
                entry.package_id.as_deref(),
                outside,
            )
        };
        // An entry whose copy doesn't come back is left out and nothing is
        // written for it, so a place it couldn't go back to stops nothing:
        // a package left out of the whole set, with none of its files in
        // it, never stops the rest. The report names where it would have
        // gone, or the path it records when that place is refused.
        let Some((source_path, sha256)) =
            copy_to_restore(backup_path, entry, recorded.as_ref(), check)?
        else {
            plan.skipped_targets
                .push(target().unwrap_or_else(|_| original_path.clone()));
            continue;
        };
        // One refused file that comes back stops the whole set. Each other
        // package with a file in the set can still come back on its own,
        // with only its own files, and the refusal names them; a package
        // left out would be refused alone.
        let target_path = target().map_err(|error| match error {
            RabbitError::RestoreRefused(RestoreRefusal::TargetRefused { path, reason, .. })
                if package_id.is_none() && entry.package_id.is_some() =>
            {
                RestoreRefusal::TargetRefused {
                    path,
                    reason,
                    other_packages: restoring
                        .iter()
                        .filter(|other| Some(**other) != entry.package_id.as_deref())
                        .map(ToString::to_string)
                        .collect(),
                }
                .into()
            }
            other => other,
        })?;
        plan.files.push(PlannedRestore {
            package_id: entry.package_id.clone(),
            size: fs::metadata(&source_path).with_path(&source_path)?.len(),
            source_path,
            target_path,
            sha256,
        });
    }
    Ok(plan)
}

/// How a restore's plan reads the copies in a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyCheck {
    /// Hash each copy, as a real restore does: a copy matches only with the
    /// hash its entry recorded.
    Hash,
    /// Read no file's content, for the quick plan: a copy matches with the
    /// size its entry recorded, as the overview of the backups judges it.
    Size,
}

/// The copy a restore puts back for `entry`, with its SHA-256, or `None`
/// when the restore leaves the entry out. With [`CopyCheck::Size`] nothing
/// is hashed, and the SHA-256 is the one the entry recorded.
///
/// A pending entry's copy may never have finished. The run stopped before
/// changing the file then, so the file is still where it was: the copy
/// comes back only when it is there and matches. A finished entry whose
/// copy is missing or doesn't match is a damaged set, and restoring part of
/// it would mix old and new files, so that is refused
/// ([`RestoreRefusal::PartMissing`]), as the overview lists such a backup.
fn copy_to_restore(
    backup_path: &Path,
    entry: &BackupManifestFile,
    recorded: Option<&RecordedSet>,
    check: CopyCheck,
) -> Result<Option<(PathBuf, String)>> {
    let source_path = restore_source(backup_path, &entry.backup_path, recorded)?;
    let finished = entry.state == BackupEntryState::Done;
    let part_missing = || -> RabbitError {
        RestoreRefusal::PartMissing {
            backup_id: backup_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: entry.original_path.clone(),
        }
        .into()
    };
    if !source_path.is_file() {
        if finished {
            return Err(part_missing());
        }
        return Ok(None);
    }
    let (matches, sha256) = match check {
        CopyCheck::Hash => {
            let sha256 = sha256_file(&source_path)?;
            (entry.sha256.is_empty() || sha256 == entry.sha256, sha256)
        }
        CopyCheck::Size => {
            let size = fs::metadata(&source_path).with_path(&source_path)?.len();
            (size == entry.size, entry.sha256.clone())
        }
    };
    if !matches {
        if finished {
            return Err(part_missing());
        }
        return Ok(None);
    }
    Ok(Some((source_path, sha256)))
}

/// A set without a manifest (one from a run that died before older RABBIT
/// versions wrote it, or a `before-restore` set from them) mirrors the
/// resource folder, so each file goes back to the same relative place. With
/// [`CopyCheck::Size`] no file is hashed, and no SHA-256 is known.
fn plan_restore_from_layout(
    resource_path: &Path,
    backup_path: &Path,
    check: CopyCheck,
) -> Result<RestorePlan> {
    let mut relative_files = Vec::new();
    let mut links = Vec::new();
    collect_relative_files(backup_path, backup_path, &mut relative_files, &mut links)?;
    // A link in the set could lead anywhere, so the whole set is refused.
    if let Some(link) = links.into_iter().min() {
        return Err(RabbitError::BackupPathIsLink {
            path: backup_path.join(link),
        });
    }
    relative_files.sort();

    let mut files = Vec::with_capacity(relative_files.len());
    for relative_path in relative_files {
        let source_path = backup_path.join(&relative_path);
        // Only the manifest says where an outside file belongs, and the
        // relative layout would put it inside the resource folder.
        if first_component_is(&relative_path, EXTERNAL_DIR) {
            return Err(TargetRefusal::OutsideWithoutManifest
                .refuse(&relative_path)
                .into());
        }
        let target_path = restore_target(
            resource_path,
            &relative_path,
            None,
            None,
            &OutsideRestoreRules::default(),
        )?;
        files.push(PlannedRestore {
            package_id: None,
            size: fs::metadata(&source_path).with_path(&source_path)?.len(),
            sha256: match check {
                CopyCheck::Hash => sha256_file(&source_path)?,
                CopyCheck::Size => String::new(),
            },
            source_path,
            target_path,
        });
    }
    Ok(RestorePlan {
        files,
        skipped_targets: Vec::new(),
    })
}

/// Where a file goes back to, refusing anything in RABBIT's own folder,
/// the receipt included (see [`is_in_rabbit_folder`]), and anything outside
/// the resource folder except:
/// - below a folder RABBIT installs into itself, for any package, reached
///   through real folders only below it, by names the file system opens as
///   written (see [`opens_as_written`]). That folder may be missing; the
///   restore makes it again;
/// - where one of `package_id`'s own `vendor_added_files` patterns matches,
///   reached through real folders only below the pattern's guard (see
///   [`check_reached_through_real_folders`]), by names the file system opens
///   as written. Another package's file never goes there.
///
/// Inside the resource folder, every name must open as written too, and
/// the way there may lead through a link only while it stays inside the
/// resource folder and outside RABBIT's (see
/// [`check_stays_in_resource_folder`]).
///
/// The path returned is the one checked, and the one a restore writes.
///
/// `recorded_resource` is where the resource folder was when the set was
/// written. A portable REAPER can move (a USB drive gets another letter),
/// and a file recorded inside the old folder goes to the same place inside
/// the current one.
fn restore_target(
    resource_path: &Path,
    original_path: &Path,
    recorded_resource: Option<&Path>,
    package_id: Option<&str>,
    outside: &OutsideRestoreRules,
) -> Result<PathBuf> {
    let refuse = |reason: TargetRefusal| RabbitError::from(reason.refuse(original_path));

    // The receipt is no exception: copying an old one back would undo
    // every change to it since. Sets that hold one are refused before this
    // with a reason of their own (see `refuse_sets_a_restore_does_not_take`).
    if let Some(relative) = inside_resource_folder(resource_path, original_path, recorded_resource)?
    {
        let target = inside_place_by_text(resource_path, &relative).map_err(refuse)?;
        check_stays_in_resource_folder(resource_path, &relative).map_err(refuse)?;
        return Ok(target);
    }

    let shared_root = outside.shared_roots.iter().find_map(|root| {
        relative_within(original_path, root)
            .filter(|below| is_plain_relative(below))
            .map(|below| (root, below))
    });
    if let Some((root, below)) = shared_root {
        refuse_names_not_as_written(&below).map_err(refuse)?;
        // The path checked is the path written: the folder with the names
        // below it, never another spelling of it, such as a `\\?\` one.
        let target = root.join(&below);
        // A folder that isn't there has nothing below it to follow, and the
        // restore makes it again, as an install would.
        let missing = matches!(
            fs::symlink_metadata(root),
            Err(error) if error.kind() == io::ErrorKind::NotFound
        );
        if !missing {
            check_reached_through_real_folders(&target, root).map_err(refuse)?;
        }
        return Ok(target);
    }
    let own_pattern = package_id.and_then(|package_id| {
        outside
            .vendor_added
            .iter()
            .find(|(owner, pattern)| owner == package_id && pattern_matches(pattern, original_path))
            .map(|(_, pattern)| pattern)
    });
    if let Some(pattern) = own_pattern {
        let guard_root = pattern.guard.root.as_deref().unwrap_or(&pattern.base);
        // As below the CLAP folder: the path checked is the path written,
        // the guard's folder with the names below it, never the recorded
        // spelling, which may be a `\\?\` one that keeps a trailing dot.
        let below = below_guard(original_path, guard_root).map_err(refuse)?;
        refuse_names_not_as_written(&below).map_err(refuse)?;
        let target = guard_root.join(&below);
        check_reached_through_real_folders(&target, guard_root).map_err(refuse)?;
        return Ok(target);
    }
    Err(refuse(TargetRefusal::OutsideAllowedFolders))
}

/// Refuse a name on the way below a folder that the file system would open
/// as another one: it could make a check look at one place while the file
/// goes to another, or open a device (see [`opens_as_written`]).
fn refuse_names_not_as_written(below: &Path) -> std::result::Result<(), TargetRefusal> {
    match below
        .components()
        .map(|component| component.as_os_str())
        .find(|name| !opens_as_written(name))
    {
        Some(name) => Err(TargetRefusal::NameNotAsWritten {
            name: name.to_string_lossy().into_owned(),
        }),
        None => Ok(()),
    }
}

/// The part of `original_path` below `guard_root`, the folder a package's
/// own files go back into, as a plain path; refused when the path leads
/// outside it.
fn below_guard(
    original_path: &Path,
    guard_root: &Path,
) -> std::result::Result<PathBuf, TargetRefusal> {
    relative_within(original_path, guard_root)
        .filter(|below| is_plain_relative(below))
        .ok_or_else(|| TargetRefusal::LeadsOutside {
            folder: guard_root.to_path_buf(),
        })
}

/// Where a restore would put `entry` back, judged by the rules of
/// [`restore_target`] but from the paths' text alone: inside the resource
/// folder and outside RABBIT's own, or below a folder `outside` allows for
/// the entry's package, by names that open as written. Otherwise the reason
/// a restore refuses the file for, the same as the restore's own. Nothing
/// on disk is read, so a path on another computer, such as
/// `\\host\share\x.dll`, is never opened. The overview of the backups
/// judges with this, and only looks at the places it returns. `outside` is
/// called only for a place outside the resource folder.
pub(crate) fn restore_place_by_text<'a>(
    resource_path: &Path,
    entry: &BackupManifestFile,
    outside: impl FnOnce() -> &'a OutsideRestoreRules,
) -> std::result::Result<PathBuf, TargetRefusal> {
    let original_path =
        match entry_place(resource_path, entry).ok_or(TargetRefusal::NotPlainPath)? {
            EntryPlace::Inside(relative) => return inside_place_by_text(resource_path, &relative),
            EntryPlace::Outside(original_path) => original_path,
        };
    let outside = outside();
    let shared_root = outside.shared_roots.iter().find_map(|root| {
        relative_within(&original_path, root)
            .filter(|below| is_plain_relative(below))
            .map(|below| (root, below))
    });
    if let Some((root, below)) = shared_root {
        refuse_names_not_as_written(&below)?;
        return Ok(root.join(below));
    }
    let pattern = entry.package_id.as_deref().and_then(|package_id| {
        outside
            .vendor_added
            .iter()
            .find(|(owner, pattern)| {
                owner == package_id && pattern_matches(pattern, &original_path)
            })
            .map(|(_, pattern)| pattern)
    });
    let Some(pattern) = pattern else {
        return Err(TargetRefusal::OutsideAllowedFolders);
    };
    let guard_root = pattern.guard.root.as_deref().unwrap_or(&pattern.base);
    let below = below_guard(&original_path, guard_root)?;
    refuse_names_not_as_written(&below)?;
    Ok(guard_root.join(below))
}

/// Where a restore would put back a file whose place in the resource folder
/// at `resource_path` is `relative`, a plain path, judged from its text as
/// [`restore_target`] judges it: refused in RABBIT's own folder, or with a
/// name that may open another file or folder than it names.
pub(crate) fn inside_place_by_text(
    resource_path: &Path,
    relative: &Path,
) -> std::result::Result<PathBuf, TargetRefusal> {
    if is_in_rabbit_folder(relative) {
        return Err(TargetRefusal::RabbitFolder);
    }
    refuse_names_not_as_written(relative)?;
    Ok(resource_path.join(relative))
}

/// Where `original_path`, as a set records it, lies inside the resource
/// folder: a relative path as it is, and an absolute one when it lies
/// inside `resource_path`, or inside `recorded_resource`, where the
/// resource folder was when the set was written. `None` for an absolute
/// path outside both. A relative path that isn't a plain path is refused.
fn inside_resource_folder(
    resource_path: &Path,
    original_path: &Path,
    recorded_resource: Option<&Path>,
) -> Result<Option<PathBuf>> {
    if original_path.is_relative() {
        if !is_plain_relative(original_path) {
            return Err(TargetRefusal::NotPlainPath.refuse(original_path).into());
        }
        return Ok(Some(original_path.to_path_buf()));
    }
    let resource = absolute_without_dots(resource_path).with_path(resource_path)?;
    Ok(relative_within(original_path, &resource)
        .or_else(|| recorded_resource.and_then(|old| relative_within(original_path, old)))
        .filter(|relative| is_plain_relative(relative)))
}

/// Check that a file may go back to `relative` inside the resource folder
/// at `resource_path` the way uninstall's guard checks a file before it
/// moves one: the file's own place is no symbolic link or junction, which a
/// restore would read the old file through and could write through, and
/// the deepest part of the way to it that exists lies, by its real path,
/// inside the resource folder's real path and outside RABBIT's folder,
/// however that is spelt or reached. A link further up is allowed as long
/// as that holds, so `UserPlugins` may be a link to another folder inside
/// the resource folder, but never to `RABBIT` or outside it. A resource
/// folder that isn't there has nothing in it to follow. The error says
/// which failed.
fn check_stays_in_resource_folder(
    resource_path: &Path,
    relative: &Path,
) -> std::result::Result<(), TargetRefusal> {
    use crate::uninstall::paths::inside_or_same;

    let real_resource = match fs::canonicalize(resource_path) {
        Ok(real) => real,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(could_not_check(resource_path, &error)),
    };
    let names = relative.components().count();
    let mut deepest = resource_path.to_path_buf();
    let mut current = resource_path.to_path_buf();
    for (index, component) in relative.components().enumerate() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() && index + 1 == names => {
                return Err(TargetRefusal::PlaceIsLink);
            }
            Ok(_) => deepest.clone_from(&current),
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(could_not_check(&current, &error)),
        }
    }
    let real = fs::canonicalize(&deepest).map_err(|error| could_not_check(&deepest, &error))?;
    if !inside_or_same(&real, &real_resource) {
        return Err(TargetRefusal::LeadsOutsideResourceFolder {
            folder: resource_path.to_path_buf(),
        });
    }
    // RABBIT's folder as the resource folder's real path spells it, and,
    // should `RABBIT` itself be a link, as its target's real path does.
    let rabbit = real_resource.join("RABBIT");
    let linked_rabbit = fs::canonicalize(resource_path.join("RABBIT")).ok();
    if inside_or_same(&real, &rabbit)
        || linked_rabbit.is_some_and(|linked| inside_or_same(&real, &linked))
    {
        return Err(TargetRefusal::LeadsIntoRabbitFolder);
    }
    Ok(())
}

/// The reason for a place that couldn't be checked: `path` couldn't be
/// read.
fn could_not_check(path: &Path, error: &io::Error) -> TargetRefusal {
    TargetRefusal::CouldNotCheck {
        path: path.to_path_buf(),
        detail: error.to_string(),
    }
}

/// Check that a file may go back to `target`, below `guard_root`, without
/// following anything there: `guard_root` exists, nothing that exists on
/// the way from it down to `target` is a symbolic link or junction (nor
/// `target` itself), and the deepest part of that way that exists is, by
/// its real path, `guard_root` or inside it. The error says which failed.
fn check_reached_through_real_folders(
    target: &Path,
    guard_root: &Path,
) -> std::result::Result<(), TargetRefusal> {
    let real_root = match fs::canonicalize(guard_root) {
        Ok(real_root) => real_root,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(TargetRefusal::GuardFolderGone {
                folder: guard_root.to_path_buf(),
            });
        }
        Err(error) => return Err(could_not_check(guard_root, &error)),
    };
    let outside = || TargetRefusal::LeadsOutside {
        folder: guard_root.to_path_buf(),
    };
    let Some(below) = relative_within(target, guard_root) else {
        return Err(outside());
    };
    let mut deepest = guard_root.to_path_buf();
    let mut current = guard_root.to_path_buf();
    for component in below.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(TargetRefusal::ThroughLink { link: current });
            }
            Ok(_) => deepest.clone_from(&current),
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(could_not_check(&current, &error)),
        }
    }
    match fs::canonicalize(&deepest) {
        Ok(real) if real.starts_with(&real_root) => Ok(()),
        Ok(_) => Err(outside()),
        Err(error) => Err(could_not_check(&deepest, &error)),
    }
}

/// The copy an entry names, always inside the set being restored.
///
/// Every writer records the copy below `RABBIT/backups/<id>`: absolute, or
/// relative to the folder RABBIT was started in when it was given a relative
/// resource path. Either way the part after the set folder finds the copy,
/// also when the set has moved with its resource folder or the restore runs
/// from another folder. Only a recorded path without that part is read as
/// relative to the set itself. Nothing on the way to the copy, nor the copy
/// itself, may be a symbolic link or junction.
fn restore_source(
    backup_path: &Path,
    recorded_backup_path: &Path,
    recorded: Option<&RecordedSet>,
) -> Result<PathBuf> {
    let relative = match recorded {
        Some(recorded) => Some(recorded.inside.clone()),
        None if recorded_backup_path.is_relative() => Some(recorded_backup_path.to_path_buf()),
        None => None,
    };
    match relative {
        Some(relative)
            if is_plain_relative(&relative) && !same_path(&relative, BACKUP_MANIFEST_FILE) =>
        {
            check_no_link_in_set(backup_path, &relative)?;
            Ok(backup_path.join(relative))
        }
        _ => Err(TargetRefusal::CopyOutsideSet
            .refuse(recorded_backup_path)
            .into()),
    }
}

/// Where a set was when it recorded a copy's path, read from the last
/// `RABBIT/backups/<id>` in that path. See [`recorded_set_location`].
struct RecordedSet {
    /// The resource folder as recorded: absolute, or relative to the folder
    /// RABBIT was started in, such as `REAPER`, `.` or `../REAPER`.
    resource: PathBuf,
    /// The copy's path inside the set.
    inside: PathBuf,
}

impl RecordedSet {
    /// The file's path inside the resource folder, for an entry an older
    /// install wrote while RABBIT had a relative resource path.
    ///
    /// Those installs recorded the original as the resource path joined with
    /// the file's path, `REAPER/UserPlugins/x.dll` or `./UserPlugins/x.dll`,
    /// which reads like a path inside the resource folder but isn't. They
    /// put the copy at the same path inside the set, and that is how such an
    /// entry is told apart: its original is exactly the recorded resource
    /// folder joined with the copy's path in the set. Every other relative
    /// original is already inside the resource folder, and its copy's path
    /// in the set is the original itself, so it can't match by accident.
    fn resource_relative_original(&self, original: &Path) -> Option<PathBuf> {
        (original.is_relative()
            && !self.resource.as_os_str().is_empty()
            && same_components(original, &self.resource.join(&self.inside)))
        .then(|| self.inside.clone())
    }
}

/// Where the file of a set's entry came from, as restore reads it, and
/// where the set was when it recorded the entry. See
/// [`RecordedSet::resource_relative_original`].
fn recorded_original(entry: &BackupManifestFile) -> (PathBuf, Option<RecordedSet>) {
    let recorded = recorded_set_location(&entry.backup_path);
    let original = recorded
        .as_ref()
        .and_then(|recorded| recorded.resource_relative_original(&entry.original_path))
        .unwrap_or_else(|| entry.original_path.clone());
    (original, recorded)
}

/// Where a set was when it recorded `backup_path`, read from the last
/// `RABBIT/backups/<id>` in the path. Relative paths count too, with any
/// `.` or `..` before that part, because sets written while RABBIT had a
/// relative resource path record them that way.
fn recorded_set_location(backup_path: &Path) -> Option<RecordedSet> {
    let components: Vec<Component<'_>> = backup_path.components().collect();
    let names: Vec<Option<String>> = components
        .iter()
        .map(|component| match component {
            Component::Normal(name) => Some(fold_case(&name.to_string_lossy())),
            _ => None,
        })
        .collect();
    let rabbit = fold_case("RABBIT");
    let backups = fold_case("backups");
    let start = (0..names.len().saturating_sub(3)).rev().find(|&index| {
        names[index].as_deref() == Some(rabbit.as_str())
            && names[index + 1].as_deref() == Some(backups.as_str())
            && names[index + 2].is_some()
    })?;
    Some(RecordedSet {
        resource: components[..start].iter().collect(),
        inside: components[start + 3..].iter().collect(),
    })
}

/// Whether `path` is a regular file, by its own metadata: never through a
/// symbolic link or junction.
pub(crate) fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// What [`restore_file`] did to one file before it finished or failed:
/// what a restore that fails later needs to put the file back as it was.
#[derive(Debug, Default)]
struct FileStep {
    /// The file at the target is no longer the one that was there before:
    /// the one it replaced was removed, or the restore's copy took its
    /// place, or took a place where there was none.
    touched: bool,
    /// The restore's copy took the target's place. While `touched` and
    /// not `landed`, nothing is there: the file it replaces was removed,
    /// and its copy in the before-restore set is the only one.
    landed: bool,
    /// The outermost folder on the way to the target that the restore
    /// made, because it wasn't there.
    created_folder: Option<PathBuf>,
}

/// Put one file back, noting in `step` what changed. Nothing at the target
/// or at its temporary name is followed: the temporary file is made new, so
/// a link there stops the restore instead of being written through, and
/// only a regular file at the target is backed up and replaced. Anything
/// else there stops the restore before the file changes.
///
/// On an error the temporary file is gone again, and `step` says whether
/// the file the restore replaces is gone from its place too; then the copy
/// in `backups` is the only one, and [`put_back_file`] brings it back.
fn restore_file(
    action: &mut RestoreBackupAction,
    backups: &mut BackupSetWriter,
    step: &mut FileStep,
) -> Result<()> {
    let target = action.target_path.clone();
    #[cfg(test)]
    restore_seam::at(restore_seam::Step::Restoring, &target).with_path(&target)?;
    if let Some(parent) = target.parent() {
        step.created_folder = first_missing_folder(parent);
        fs::create_dir_all(parent).with_path(parent)?;
    }
    let temp_path = stage_copy(&action.source_path, &target, &action.sha256)?;
    let give_up = |error: RabbitError| {
        let _ = fs::remove_file(&temp_path);
        error
    };

    // The path the dry run predicted can differ if another set took this
    // id meanwhile, so the report records where the copy really went.
    action.current_backup_path = if is_real_file(&target) {
        Some(
            backups
                .copy_file(action.package_id.as_deref(), &target)
                .map_err(give_up)?,
        )
    } else {
        None
    };

    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.is_file() => {
            fs::remove_file(&target)
                .with_path(&target)
                .map_err(give_up)?;
            // From here on the file it replaces is only in the set.
            step.touched = true;
        }
        Ok(_) => {
            return Err(give_up(crate::error::io_error_at(
                target,
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "something other than a file is there, and a restore never replaces it",
                ),
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(give_up(crate::error::io_error_at(target, error))),
    }
    #[cfg(test)]
    restore_seam::at(restore_seam::Step::Replacing, &target)
        .with_path(&target)
        .map_err(give_up)?;
    fs::rename(&temp_path, &target)
        .with_path(&target)
        .map_err(give_up)?;
    step.touched = true;
    step.landed = true;
    Ok(())
}

/// Copy `source` to the temporary name beside `target` and check the copy
/// against `sha256`. A temporary file an earlier restore left behind goes;
/// anything else there stays, and stops this, since the copy is made new
/// and never through a link. A copy that fails or doesn't match is removed
/// again. Returns the temporary file, for the caller to rename into place.
fn stage_copy(source: &Path, target: &Path, sha256: &str) -> Result<PathBuf> {
    let temp_path = temporary_restore_path(target);
    if is_real_file(&temp_path) {
        fs::remove_file(&temp_path).with_path(&temp_path)?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .with_path(&temp_path)?;
    copy_checked(source, &temp_path, sha256)?;
    Ok(temp_path)
}

/// The outermost folder on the way to `folder`, `folder` itself included,
/// that isn't there: the first one making `folder` makes. `None` when
/// `folder` is there.
fn first_missing_folder(folder: &Path) -> Option<PathBuf> {
    let mut missing = None;
    let mut current = Some(folder);
    while let Some(path) = current {
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing = Some(path.to_path_buf());
                current = path.parent();
            }
            _ => break,
        }
    }
    missing
}

/// Put back every file of a backup whose restore failed partway that the
/// restore changed, newest first, and mark how each file ended up: put back,
/// left as the backup had it, missing, where the file it replaced was
/// removed and its copy never took its place, or not restored, for a file
/// the restore never changed. `steps` say what [`restore_file`] did to each
/// file it got to. Returns whether every file is as it was before the
/// restore.
fn put_back_what_changed(
    actions: &mut [RestoreBackupAction],
    steps: &[FileStep],
    backups: &BackupSetWriter,
) -> bool {
    let reached = steps.len().min(actions.len());
    let mut all_back = true;
    for (action, step) in actions[..reached].iter_mut().zip(steps).rev() {
        if !step.touched {
            action.action = RestoreBackupActionKind::NotRestored;
            action.current_backup_path = None;
            remove_created_folders(&action.target_path, step);
            continue;
        }
        match put_back_file(action, step, backups) {
            Ok(()) => action.action = RestoreBackupActionKind::PutBack,
            Err(error) => {
                // A put-back that fails changes nothing at the target, so
                // it holds what the restore left there.
                action.action = if step.landed {
                    RestoreBackupActionKind::Left
                } else {
                    RestoreBackupActionKind::Missing
                };
                action.error = Some(error.to_string());
                all_back = false;
            }
        }
    }
    for action in &mut actions[reached..] {
        action.action = RestoreBackupActionKind::NotRestored;
        action.current_backup_path = None;
    }
    all_back
}

/// Put one file a restore changed back as it was: the file it replaced,
/// copied back from the before-restore set `backups` and checked against
/// the hash the set recorded, or no file, where the restore added one,
/// with the folders it made for it once they are empty again.
fn put_back_file(
    action: &RestoreBackupAction,
    step: &FileStep,
    backups: &BackupSetWriter,
) -> Result<()> {
    let target = &action.target_path;
    #[cfg(test)]
    restore_seam::at(restore_seam::Step::PuttingBack, target).with_path(target)?;
    if let Some(copy) = &action.current_backup_path {
        let sha256 = backups
            .recorded_sha256(copy)
            .ok_or_else(|| RabbitError::BackupNotFound(copy.clone()))?;
        let temp_path = stage_copy(copy, target, &sha256)?;
        return fs::rename(&temp_path, target)
            .with_path(target)
            .inspect_err(|_| {
                let _ = fs::remove_file(&temp_path);
            });
    }
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.is_file() => fs::remove_file(target).with_path(target)?,
        Ok(_) => {
            return Err(crate::error::io_error_at(
                target.clone(),
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "something other than the restored file is there now",
                ),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(crate::error::io_error_at(target.clone(), error)),
    }
    remove_created_folders(target, step);
    Ok(())
}

/// Remove the folders the restore made on the way to `target` while they
/// are empty, from the file's own folder up to the first one it made.
fn remove_created_folders(target: &Path, step: &FileStep) {
    if let Some(above) = step.created_folder.as_deref().and_then(Path::parent) {
        remove_empty_parents(target, above);
    }
}

/// A way for tests to fail a step of a restore, or to change something
/// right before it, as another program holding a file open would. Each
/// thread has its own hook, so tests running at once never see each
/// other's.
#[cfg(test)]
pub(crate) mod restore_seam {
    use std::cell::RefCell;
    use std::io;
    use std::path::Path;

    /// A step of a restore of one backup.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Step {
        /// Before anything of the file happens.
        Restoring,
        /// Right before the restore's copy takes the file's place, after
        /// the file it replaces was removed.
        Replacing,
        /// Before the file is put back as it was, after it, a later file or
        /// the records failed.
        PuttingBack,
        /// Before RABBIT's records are saved, once every file is back. The
        /// place is the receipt's.
        SavingRecords,
    }

    type Hook = Box<dyn FnMut(Step, &Path) -> io::Result<()>>;

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

    /// Call `hook` at every step of every backup this thread restores, with
    /// the place of the file the step is about, until what this returns is
    /// dropped. An error it returns fails that step.
    pub(crate) fn install(hook: impl FnMut(Step, &Path) -> io::Result<()> + 'static) -> Installed {
        HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
        Installed
    }

    pub(super) fn at(step: Step, path: &Path) -> io::Result<()> {
        HOOK.with(|hook| match hook.borrow_mut().as_mut() {
            Some(hook) => hook(step, path),
            None => Ok(()),
        })
    }
}

/// Whether `relative`, a path inside a set, names one of the set's own
/// files rather than a backed-up one: the manifest, its staging file, or an
/// origin note.
pub(crate) fn is_set_bookkeeping(relative: &Path) -> bool {
    relative == Path::new(BACKUP_MANIFEST_FILE)
        || relative == Path::new(MANIFEST_STAGING_FILE)
        || relative.to_string_lossy().ends_with(ORIGIN_SIDECAR_SUFFIX)
}

/// The files of a set, relative to it, leaving out the manifest, its
/// staging file and the origin notes, none of which are backed-up files.
/// Symbolic links and junctions go into `links` instead, by their path in
/// the set, and are never followed.
fn collect_relative_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
    links: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(current).with_path(current)? {
        let entry = entry.with_path(current)?;
        let path = entry.path();
        // The entry's own type: unlike `Path::is_dir`, it doesn't follow a
        // link. Rust reports a junction as a link on Windows.
        let kind = entry.file_type().with_path(&path)?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| RabbitError::BackupNotFound(root.to_path_buf()))?;
        if kind.is_symlink() {
            links.push(relative.to_path_buf());
        } else if kind.is_dir() {
            collect_relative_files(root, &path, files, links)?;
        } else if kind.is_file() && !is_set_bookkeeping(relative) {
            files.push(relative.to_path_buf());
        }
    }
    Ok(())
}

/// Check that nothing on the way from the set folder `set` down to
/// `relative` inside it, `relative` itself included, is a symbolic link or
/// junction. A part that isn't there ends the check: there is nothing below
/// it to follow.
fn check_no_link_in_set(set: &Path, relative: &Path) -> Result<()> {
    let mut current = set.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RabbitError::BackupPathIsLink { path: current });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(error).with_path(current),
        }
    }
    Ok(())
}

/// Save a set's manifest. It is written to a temp file and renamed over the
/// old one, as the receipt is, so a crash mid-save leaves the previous
/// manifest whole instead of a truncated one.
pub fn save_backup_manifest(backup_set: &Path, manifest: &BackupManifest) -> Result<PathBuf> {
    fs::create_dir_all(backup_set).with_path(backup_set)?;
    let path = backup_set.join(BACKUP_MANIFEST_FILE);
    let content = serde_json::to_string_pretty(manifest).with_json_path(&path)?;
    let staged = backup_set.join(MANIFEST_STAGING_FILE);
    fs::write(&staged, content).with_path(&staged)?;
    fs::rename(&staged, &path).with_path(&path)?;
    Ok(path)
}

/// A set's manifest, or `None` when it has none. A manifest that is a
/// symbolic link or junction is refused, never read.
pub(crate) fn load_backup_manifest(backup_set: &Path) -> Result<Option<BackupManifest>> {
    let path = backup_set.join(BACKUP_MANIFEST_FILE);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(RabbitError::BackupPathIsLink { path });
        }
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_path(path),
    }

    let content = fs::read_to_string(&path).with_path(&path)?;
    let manifest = serde_json::from_str(&content).with_json_path(&path)?;
    Ok(Some(manifest))
}

/// Where a backup set id leads, as [`find_backup_set`] found it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FoundBackupSet {
    /// The set's folder in `RABBIT/backups`.
    pub(crate) set: Option<PathBuf>,
    /// The folder of the same name in `RABBIT/backups-deleting`: a set
    /// whose deletion started and didn't finish.
    pub(crate) deleting: Option<PathBuf>,
    /// When `set` is `None`, the name of a set in `RABBIT/backups` that
    /// differs from the id only in upper and lower case. It is only named,
    /// so the person learns the set is still there, and never used.
    pub(crate) other_case: Option<String>,
}

/// Find the set `backup_id` names, in `RABBIT/backups` and in
/// `RABBIT/backups-deleting`, without following any link.
///
/// The id must be well-formed (see [`backup_id_is_well_formed`]), or it is
/// refused before anything is read. It is then used only where it equals,
/// byte for byte, the name of an entry `read_dir` lists in one of those
/// folders, so a name that differs only in case is no match, even where
/// the file system would open it; such a set in `RABBIT/backups` is only
/// named, in `other_case`. Nothing else is ever joined to either
/// folder. `RABBIT`, both folders and each set found must be real folders,
/// not symbolic links or junctions; a link anywhere there is an error. A
/// folder that isn't there counts as empty, and none is created.
pub(crate) fn find_backup_set(resource_path: &Path, backup_id: &str) -> Result<FoundBackupSet> {
    if !backup_id_is_well_formed(backup_id) {
        return Err(RestoreRefusal::InvalidId {
            backup_id: backup_id.to_string(),
        }
        .into());
    }
    let rabbit = resource_path.join("RABBIT");
    if !is_real_folder(&rabbit)? {
        return Ok(FoundBackupSet::default());
    }
    let (set, other_case) = find_folder_named(&rabbit.join("backups"), backup_id)?;
    let (deleting, _) = find_folder_named(&rabbit.join(DELETING_DIR), backup_id)?;
    Ok(FoundBackupSet {
        set,
        deleting,
        other_case,
    })
}

/// The set `backup_id` names in `RABBIT/backups`, for a restore (see
/// [`find_backup_set`]). A set found only in `RABBIT/backups-deleting` is
/// being deleted and is refused. An id found in neither names a set that
/// was deleted or never existed, unless a set's name differs from it only
/// in case; then that set is named instead, and still not restored.
///
/// An id that is refused, or names no set, only because of dots or spaces
/// at its end, while the id without them names a set exactly, names that
/// set too ([`RestoreRefusal::EndsInDotOrSpace`]), and still doesn't
/// restore it: the id was most likely copied with the period after it, and
/// Windows would have opened the set without them.
pub(crate) fn set_to_restore(resource_path: &Path, backup_id: &str) -> Result<PathBuf> {
    let found = match find_backup_set(resource_path, backup_id) {
        Ok(found) => found,
        Err(RabbitError::RestoreRefused(RestoreRefusal::InvalidId { backup_id })) => {
            let refusal = set_without_dots_or_spaces_at_the_end(resource_path, &backup_id)
                .unwrap_or(RestoreRefusal::InvalidId { backup_id });
            return Err(refusal.into());
        }
        Err(error) => return Err(error),
    };
    let backup_id = backup_id.to_string();
    let refusal = match found {
        FoundBackupSet { set: Some(set), .. } => return Ok(set),
        FoundBackupSet {
            deleting: Some(_), ..
        } => RestoreRefusal::BeingDeleted { backup_id },
        FoundBackupSet {
            other_case: Some(existing),
            ..
        } => RestoreRefusal::DiffersInCase {
            backup_id,
            existing,
        },
        FoundBackupSet { .. } => set_without_dots_or_spaces_at_the_end(resource_path, &backup_id)
            .unwrap_or(RestoreRefusal::NotFound { backup_id }),
    };
    Err(refusal.into())
}

/// [`RestoreRefusal::EndsInDotOrSpace`] when `backup_id` ends in dots or
/// spaces and, without them, is a well-formed id that names a set in
/// `RABBIT/backups` exactly (see [`find_backup_set`]). The set is only
/// named, never used.
fn set_without_dots_or_spaces_at_the_end(
    resource_path: &Path,
    backup_id: &str,
) -> Option<RestoreRefusal> {
    let trimmed = backup_id.trim_end_matches(['.', ' ']);
    if trimmed == backup_id || !backup_id_is_well_formed(trimmed) {
        return None;
    }
    match find_backup_set(resource_path, trimmed) {
        Ok(FoundBackupSet { set: Some(_), .. }) => Some(RestoreRefusal::EndsInDotOrSpace {
            backup_id: backup_id.to_string(),
            existing: trimmed.to_string(),
        }),
        _ => None,
    }
}

/// The entry of `folder` whose name is exactly `name`, when `folder` and
/// that entry are both real folders. `None` when either isn't there or
/// isn't a folder; an error when either is a symbolic link or junction.
///
/// With no exact match, also the name of a real folder there whose name
/// differs from `name` only in upper and lower case and could be a set's,
/// the first by byte order if there are several. It is only named, never
/// used.
fn find_folder_named(folder: &Path, name: &str) -> Result<(Option<PathBuf>, Option<String>)> {
    if !is_real_folder(folder)? {
        return Ok((None, None));
    }
    let lowercase = name.to_lowercase();
    let mut other_case: Option<String> = None;
    for entry in fs::read_dir(folder).with_path(folder)? {
        let entry = entry.with_path(folder)?;
        let entry_name = entry.file_name();
        if entry_name == OsStr::new(name) {
            let path = entry.path();
            return Ok((is_real_folder(&path)?.then_some(path), None));
        }
        // A real folder whose name differs only in case is noted, never
        // used. Its type is the entry's own, which never follows a link.
        if let Some(entry_name) = entry_name.to_str()
            && entry_name.to_lowercase() == lowercase
            && backup_id_is_well_formed(entry_name)
            && entry.file_type().is_ok_and(|kind| kind.is_dir())
            && other_case.as_deref().is_none_or(|found| entry_name < found)
        {
            other_case = Some(entry_name.to_string());
        }
    }
    Ok((None, other_case))
}

/// Refuse, with [`RabbitError::BackupPathIsLink`], when `RABBIT` or
/// `RABBIT/backups` in `resource_path` is a symbolic link or junction: a
/// restore and `rabbit backups` never follow one there (see
/// [`find_backup_set`]), so a set written behind it couldn't be restored.
/// A folder that isn't there yet is no link; the writer makes it.
pub(crate) fn check_backups_folder_is_real(resource_path: &Path) -> Result<()> {
    is_real_folder(&resource_path.join("RABBIT"))?;
    is_real_folder(&backups_root(resource_path))?;
    Ok(())
}

/// `RABBIT` or `RABBIT/backups` in `resource_path`, whichever is a symbolic
/// link or junction (see [`check_backups_folder_is_real`]), or `None` when
/// neither is one or they can't be read: a folder that can't be read stops
/// the writer later, with its own error.
pub(crate) fn linked_backups_folder(resource_path: &Path) -> Option<PathBuf> {
    match check_backups_folder_is_real(resource_path) {
        Err(RabbitError::BackupPathIsLink { path }) => Some(path),
        _ => None,
    }
}

/// Whether `path` is a real folder, by its own metadata: `false` when
/// nothing is there or something other than a folder is, and an error when
/// it is a symbolic link or junction, which is never followed. Rust reports
/// a junction as a link on Windows.
pub(crate) fn is_real_folder(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(RabbitError::BackupPathIsLink {
            path: path.to_path_buf(),
        }),
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_path(path),
    }
}

/// Whether `backup_id` can name a set at all: exactly one plain name, with
/// no `:`, `/`, `\` or NUL in it. Joined to the backups folder, `D:` and
/// `C:foo` would leave it for a drive's current folder on Windows, and so
/// would `x:stream`, for drive X. `unix-1:stream` would name a stream of
/// the folder `unix-1`. On Windows the name also
/// mustn't end in a dot or a space, which Windows drops, so `unix-1.` would
/// open `unix-1`, nor be a device name such as `CON`, which opens the
/// device in any folder.
pub(crate) fn backup_id_is_well_formed(backup_id: &str) -> bool {
    let mut components = Path::new(backup_id).components();
    let one_name = matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(name)), None) if name == OsStr::new(backup_id)
    );
    one_name
        && !backup_id.contains([':', '/', '\\', '\0'])
        && !(cfg!(windows)
            && (backup_id.ends_with(['.', ' ']) || is_windows_device_name(backup_id)))
}

/// Whether Windows opens a device for `name` rather than a file or folder
/// of that name: `CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, or `COM`
/// or `LPT` followed by one digit or a superscript 1, 2 or 3. Any case,
/// alone or before an extension, with any spaces before the extension:
/// `con.txt` and `NUL .log` are devices too.
fn is_windows_device_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let numbered = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|number| {
            matches!(number, "¹" | "²" | "³")
                || (number.len() == 1 && number.bytes().all(|byte| byte.is_ascii_digit()))
        });
    numbered
        || matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        )
}

pub(crate) fn backups_root(resource_path: &Path) -> PathBuf {
    resource_path.join("RABBIT").join("backups")
}

/// `path` made absolute, with no `.` or `..` left in it.
///
/// `std::path::absolute` already resolves both on Windows, but keeps `..`
/// elsewhere. It is resolved here by the text: the current folder comes from
/// the operating system with every symbolic link already followed, so a `..`
/// right after it means its parent. Only a link inside the typed path itself
/// followed by `..` would end up somewhere else. Canonicalizing instead
/// would spell the same folder two ways (macOS's `/var` and `/private/var`)
/// depending on whether a file exists yet, and then files inside the
/// resource folder would stop counting as inside it.
pub(crate) fn absolute_without_dots(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    if !absolute
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Ok(absolute);
    }
    let mut normal = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    Ok(normal)
}

/// A resource-relative path that would land on the set's own files (its
/// manifest, or the `external` folder) goes under `external/` like an
/// outside file, with its absolute path recorded.
fn collides_with_set_layout(relative: &Path) -> bool {
    first_component_is(relative, EXTERNAL_DIR)
        || same_path(relative, BACKUP_MANIFEST_FILE)
        || same_path(relative, MANIFEST_STAGING_FILE)
}

/// A relative path made only of names: no root, no drive, no `.` or `..`.
fn is_plain_relative(path: &Path) -> bool {
    path.components().next().is_some()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Whether `relative`, a file's path in a set without a manifest, lies in
/// the set's `external` folder: a file from outside the resource folder,
/// which only a manifest can place.
pub(crate) fn is_outside_file_in_layout(relative: &Path) -> bool {
    first_component_is(relative, EXTERNAL_DIR)
}

fn first_component_is(path: &Path, name: &str) -> bool {
    comparable_components(path)
        .and_then(|parts| parts.into_iter().next())
        .is_some_and(|first| first == fold_case(name))
}

/// Whether `relative`, a plain path inside the resource folder, leads into
/// RABBIT's own folder, by a first name the file system may open as
/// `RABBIT` (see [`may_open_as`]).
pub(crate) fn is_in_rabbit_folder(relative: &Path) -> bool {
    matches!(
        relative.components().next(),
        Some(Component::Normal(name)) if may_open_as(name, "RABBIT")
    )
}

/// Whether `relative`, a path inside the resource folder or a set, names
/// RABBIT's receipt, `RABBIT/install-state.json`, by names the file system
/// may open as those (see [`may_open_as`]).
pub(crate) fn is_receipt(relative: &Path) -> bool {
    let receipt: Vec<Component<'_>> = Path::new(RECEIPT_RELATIVE_PATH).components().collect();
    let names: Vec<Component<'_>> = relative.components().collect();
    names.len() == receipt.len()
        && names.iter().zip(&receipt).all(|pair| match pair {
            (Component::Normal(name), Component::Normal(expected)) => {
                may_open_as(name, &expected.to_string_lossy())
            }
            _ => false,
        })
}

/// Whether the file system may open `name`, one name of a path, as
/// `expected`, one of RABBIT's own names. Windows and macOS usually ignore
/// upper and lower case. Windows also drops trailing dots and spaces, and
/// reads from the first `:` on as a stream of the same file or folder, so
/// `rabbit`, `RABBIT.` and `RABBIT::$INDEX_ALLOCATION` all open `RABBIT`
/// there. A refusal built on this may also catch a name that is really
/// another one, which is harmless; missing one could let a restore write
/// into RABBIT's folder.
fn may_open_as(name: &OsStr, expected: &str) -> bool {
    let name = name.to_string_lossy();
    let name = if cfg!(windows) {
        name.split(':')
            .next()
            .unwrap_or_default()
            .trim_end_matches(['.', ' '])
    } else {
        name.as_ref()
    };
    if cfg!(any(windows, target_os = "macos")) {
        name.to_uppercase() == expected.to_uppercase()
            || name.to_lowercase() == expected.to_lowercase()
    } else {
        name == expected
    }
}

/// Whether the file system opens `name`, one name of a path, as written.
/// Not when it holds a `:`, which Windows reads as the start of a stream of
/// a file or folder, and which no plug-in name needs anywhere. On Windows,
/// not when it ends in a dot or space either: Windows drops those, except
/// in a `\\?\` path, so the same name could open two different entries.
/// Nor when it is a device name such as `CON` or `COM1.clap` (see
/// [`is_windows_device_name`]), which may open the device instead.
fn opens_as_written(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    !name.contains(':')
        && !(cfg!(windows) && (name.ends_with(['.', ' ']) || is_windows_device_name(&name)))
}

fn same_path(left: &Path, right: &str) -> bool {
    let right = comparable_components(Path::new(right));
    right.is_some() && comparable_components(left) == right
}

/// Whether two paths have the same components, `.` and `..` included,
/// ignoring case on Windows.
fn same_components(left: &Path, right: &Path) -> bool {
    let left: Vec<Component<'_>> = left.components().collect();
    let right: Vec<Component<'_>> = right.components().collect();
    left.len() == right.len()
        && left.iter().zip(&right).all(|pair| match pair {
            (Component::Normal(left), Component::Normal(right)) => {
                fold_case(&left.to_string_lossy()) == fold_case(&right.to_string_lossy())
            }
            (left, right) => left == right,
        })
}

/// The part of `path` below `root`, or `None` when `path` is not inside it.
/// Compared component by component, ignoring case on Windows as its file
/// systems do, and treating `\\?\C:\` like `C:\`. A path with `.` or `..`
/// in it is never inside anything: a check on the text can't tell where it
/// ends up.
fn relative_within(path: &Path, root: &Path) -> Option<PathBuf> {
    let path_parts = comparable_components(path)?;
    let root_parts = comparable_components(root)?;
    if root_parts.is_empty()
        || path_parts.len() < root_parts.len()
        || path_parts[..root_parts.len()] != root_parts[..]
    {
        return None;
    }
    Some(path.components().skip(root_parts.len()).collect())
}

fn comparable_components(path: &Path) -> Option<Vec<String>> {
    path.components()
        .map(|component| match component {
            Component::Prefix(prefix) => Some(match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                    format!("{}:", char::from(letter))
                }
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => format!(
                    r"\\{}\{}",
                    server.to_string_lossy(),
                    share.to_string_lossy()
                ),
                _ => prefix.as_os_str().to_string_lossy().into_owned(),
            }),
            Component::RootDir => Some("/".to_string()),
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            Component::CurDir | Component::ParentDir => None,
        })
        .map(|part| part.map(|part| fold_case(&part)))
        .collect()
}

fn fold_case(text: &str) -> String {
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.to_string()
    }
}

fn temporary_restore_path(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("restore");
    target_path.with_file_name(format!("{file_name}.rabbit-restore-tmp"))
}

fn restore_timestamp() -> String {
    format!("restore-unix-{}", seconds_since_1970().unwrap_or_default())
}

/// The time now, in seconds since 1970 (UTC). `None` on a clock set before
/// 1970.
fn seconds_since_1970() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{
        BACKUP_MANIFEST_FILE, BEFORE_RESTORE_REASON, BackupEntryState, BackupEnvironment,
        BackupManifest, BackupManifestFile, BackupPackageRecord, BackupSetWriter, CopyCheck,
        DELETING_DIR, FoundBackupSet, KEYMAP_BACKUP_REASON, KeymapOrigin, OutsideRestoreRules,
        PlannedRestore, RECORDS_BACKUP_REASON, ReplacedPackageRecord, RestoreBackupActionKind,
        RestoreBackupOptions, RestorePlan, SkippedPackage, SkippedPackageReason,
        UPDATE_BACKUP_REASON, backup_id_is_well_formed, check_plan_matches_packages,
        find_backup_set, is_receipt, is_windows_device_name, linked_backups_folder,
        list_backup_sets, load_backup_manifest, packages_to_restore, relative_within,
        restore_backup_set, restore_backup_set_with, restore_backup_set_within, restore_target,
        save_backup_manifest,
    };
    use crate::error::RabbitError;
    use crate::hash::sha256_file;
    use crate::model::Platform;
    use crate::package::PackageSpec;
    use crate::package::uninstall::UninstallPath;
    use crate::preflight::{PreflightCheck, PreflightStatus, RunningProcess};
    use crate::receipt::{InstallState, PackageReceipt, load_install_state, save_install_state};
    use crate::restore_refusal::{RestoreRefusal, TargetRefusal, refusal};
    use crate::uninstall::paths::{UninstallRoots, link_folder, resolve_pattern};
    use crate::uninstall::preflight::{ANY_REAPER_PROCESS_CHECK, PROGRAMS_RUNNING_CHECK};

    const APPLY: RestoreBackupOptions = RestoreBackupOptions {
        dry_run: false,
        allow_reaper_running: true,
        package_id: None,
    };

    /// The reason of the sets the tests of paths and copies write. A
    /// restore reads every set it takes the same way. Older versions wrote
    /// most of these paths into update sets, which it refuses for now
    /// (`EarlierVersionNotYet`), so these use the other kind of set that
    /// holds any package's files: the one a restore writes.
    const RESTORABLE_REASON: &str = BEFORE_RESTORE_REASON;

    fn write(path: &Path, contents: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// A restore may write below `root`, a temporary folder standing in for
    /// the CLAP folder, and nowhere else outside the resource folder.
    fn shared(root: &Path) -> OutsideRestoreRules {
        OutsideRestoreRules {
            shared_roots: vec![root.to_path_buf()],
            vendor_added: Vec::new(),
        }
    }

    /// A manifest entry the way earlier versions wrote them, after the copy.
    fn legacy_entry(
        package_id: Option<&str>,
        original: &Path,
        backup: &Path,
    ) -> BackupManifestFile {
        BackupManifestFile {
            package_id: package_id.map(ToString::to_string),
            original_path: original.to_path_buf(),
            backup_path: backup.to_path_buf(),
            size: fs::metadata(backup).unwrap().len(),
            sha256: sha256_file(backup).unwrap(),
            state: BackupEntryState::Done,
        }
    }

    fn save_manifest(set: &Path, reason: &str, files: Vec<BackupManifestFile>) {
        save_backup_manifest(
            set,
            &BackupManifest {
                schema_version: 1,
                rabbit_version: "0.4.4".to_string(),
                created_at: set.file_name().unwrap().to_string_lossy().into_owned(),
                created_unix: None,
                reason: reason.to_string(),
                files,
                receipt_backup_path: None,
                packages: Default::default(),
                replaced: Default::default(),
                keymap_origin: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn lists_backup_sets_with_relative_files() {
        let dir = tempdir().unwrap();
        let backup_file = dir
            .path()
            .join("RABBIT/backups/unix-1/UserPlugins/reaper_reapack-x64.dll");
        fs::create_dir_all(backup_file.parent().unwrap()).unwrap();
        fs::write(&backup_file, b"old").unwrap();
        save_backup_manifest(
            &dir.path().join("RABBIT/backups/unix-1"),
            &BackupManifest {
                schema_version: 1,
                rabbit_version: "0.1.0".to_string(),
                created_at: "unix-1".to_string(),
                created_unix: None,
                reason: "install-replacement".to_string(),
                files: vec![BackupManifestFile {
                    package_id: Some("reapack".to_string()),
                    original_path: std::path::PathBuf::from("UserPlugins/reaper_reapack-x64.dll"),
                    backup_path: backup_file.clone(),
                    size: 3,
                    sha256: sha256_file(&backup_file).unwrap(),
                    state: BackupEntryState::Done,
                }],
                receipt_backup_path: None,
                packages: Default::default(),
                replaced: Default::default(),
                keymap_origin: None,
            },
        )
        .unwrap();

        let sets = list_backup_sets(dir.path()).unwrap();

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].id, "unix-1");
        assert_eq!(sets[0].created_at.as_deref(), Some("unix-1"));
        assert_eq!(sets[0].reason.as_deref(), Some("install-replacement"));
        assert!(sets[0].manifest_path.is_some());
        assert!(sets[0].warning.is_none());
        assert_eq!(
            sets[0].files[0],
            std::path::PathBuf::from("UserPlugins/reaper_reapack-x64.dll")
        );
        assert_eq!(sets[0].files.len(), 1);
    }

    #[test]
    fn dry_run_reports_restore_without_writing() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("UserPlugins/reaper_reapack-x64.dll");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"new").unwrap();
        let backup_file = dir
            .path()
            .join("RABBIT/backups/unix-1/UserPlugins/reaper_reapack-x64.dll");
        fs::create_dir_all(backup_file.parent().unwrap()).unwrap();
        fs::write(&backup_file, b"old").unwrap();

        let report = restore_backup_set(
            dir.path(),
            "unix-1",
            &RestoreBackupOptions {
                dry_run: true,
                allow_reaper_running: false,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            report.actions[0].action,
            RestoreBackupActionKind::WouldRestore
        );
        assert_eq!(fs::read(target).unwrap(), b"new");
        // The dry run names a backup for the current file but writes none.
        assert!(report.actions[0].current_backup_path.is_some());
        assert_eq!(list_backup_sets(dir.path()).unwrap().len(), 1);
    }

    #[test]
    fn restore_replaces_target_and_backs_up_current_file() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("UserPlugins/reaper_reapack-x64.dll");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"new").unwrap();
        let backup_file = dir
            .path()
            .join("RABBIT/backups/unix-1/UserPlugins/reaper_reapack-x64.dll");
        fs::create_dir_all(backup_file.parent().unwrap()).unwrap();
        fs::write(&backup_file, b"old").unwrap();

        let report = restore_backup_set(
            dir.path(),
            "unix-1",
            &RestoreBackupOptions {
                dry_run: false,
                allow_reaper_running: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, RestoreBackupActionKind::Restored);
        assert_eq!(fs::read(&target).unwrap(), b"old");
        let current_backup = report.actions[0].current_backup_path.as_ref().unwrap();
        assert_eq!(fs::read(current_backup).unwrap(), b"new");
        assert!(
            !target
                .with_file_name("reaper_reapack-x64.dll.rabbit-restore-tmp")
                .exists()
        );
    }

    /// Only an uninstall set records which receipt entry goes with a
    /// package. Restoring one package from any other set, here one a
    /// restore wrote, would leave the receipt describing other files, so it
    /// is refused, and nothing changes. A set without a manifest says
    /// nothing about packages at all, and is refused for that.
    #[test]
    fn only_an_uninstall_set_restores_one_package() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("UserPlugins/reaper_reapack-x64.dll");
        write(&target, b"new");
        let set = dir.path().join("RABBIT/backups/unix-1");
        let backup = set.join("UserPlugins/reaper_reapack-x64.dll");
        write(&backup, b"old");
        save_manifest(
            &set,
            BEFORE_RESTORE_REASON,
            vec![legacy_entry(Some("reapack"), &target, &backup)],
        );
        write(
            &dir.path()
                .join("RABBIT/backups/unix-2/UserPlugins/reaper_reapack-x64.dll"),
            b"old",
        );
        let one_package = |id: &str| {
            restore_backup_set(
                dir.path(),
                id,
                &RestoreBackupOptions {
                    package_id: Some("reapack".to_string()),
                    ..APPLY
                },
            )
            .unwrap_err()
        };

        let error = one_package("unix-1");
        assert!(
            matches!(refusal(&error), RestoreRefusal::NoPackageRecords { backup_id, package_id }
                if backup_id == "unix-1" && package_id == "reapack"),
            "{error:?}"
        );
        let error = one_package("unix-2");
        assert!(
            matches!(refusal(&error), RestoreRefusal::NoManifest { backup_id, package_id }
                if backup_id == "unix-2" && package_id == "reapack"),
            "{error:?}"
        );
        assert_eq!(fs::read(&target).unwrap(), b"new");
        // Restoring the set as a whole still works as it always did.
        let report = restore_backup_set(dir.path(), "unix-1", &APPLY).unwrap();
        assert!(report.packages.is_empty() && report.package_id.is_none());
        assert_eq!(fs::read(&target).unwrap(), b"old");
    }

    /// A set without a manifest that holds a copy of RABBIT's receipt, as
    /// a before-restore set from an older version can, is refused: copying
    /// the copy back would undo every change to the receipt since. The
    /// receipt stays as it is, byte for byte, and so does the copy, in a dry
    /// run too.
    #[test]
    fn a_set_without_a_manifest_holding_the_receipt_is_refused() {
        let dir = tempdir().unwrap();
        let receipt = dir.path().join("RABBIT/install-state.json");
        let current = br#"{"schema_version":1,"packages":{"reapack":"new"}}"#;
        write(&receipt, current);
        let plugin = dir.path().join("UserPlugins/reaper_reapack-x64.dll");
        write(&plugin, b"new");
        let set = dir.path().join("RABBIT/backups/unix-1");
        let copy = set.join("RABBIT/install-state.json");
        let old = br#"{"schema_version":1,"packages":{"reapack":"old"}}"#;
        write(&copy, old);
        write(&set.join("UserPlugins/reaper_reapack-x64.dll"), b"old");

        let error = refused_with(dir.path(), dir.path(), "unix-1", None);

        assert!(
            matches!(refusal(&error), RestoreRefusal::ReceiptInSet { backup_id, backup_path }
                if backup_id == "unix-1" && *backup_path == set),
            "{error:?}"
        );
        assert_eq!(fs::read(&receipt).unwrap(), current);
        assert_eq!(fs::read(&copy).unwrap(), old);
        assert_eq!(fs::read(&plugin).unwrap(), b"new");
    }

    /// Add to the set `id` what earlier versions put into an update's set,
    /// and into a set of its own after vendor installers ran: a copy of
    /// RABBIT's receipt, with an entry for it and the manifest's
    /// `receipt_backup_path`. Makes the set when it isn't there.
    fn add_receipt_copy(resource: &Path, id: &str, reason: &str) {
        let set = resource.join("RABBIT").join("backups").join(id);
        let mut manifest = load_backup_manifest(&set)
            .unwrap()
            .unwrap_or_else(|| BackupManifest {
                schema_version: 1,
                rabbit_version: "0.4.4".to_string(),
                created_at: id.to_string(),
                created_unix: None,
                reason: reason.to_string(),
                files: Vec::new(),
                receipt_backup_path: None,
                packages: Default::default(),
                replaced: Default::default(),
                keymap_origin: None,
            });
        let copy = set.join("RABBIT").join("install-state.json");
        write(
            &copy,
            &fs::read(resource.join("RABBIT").join("install-state.json")).unwrap(),
        );
        manifest.files.push(legacy_entry(
            None,
            &Path::new("RABBIT").join("install-state.json"),
            &copy,
        ));
        manifest.receipt_backup_path = Some(copy);
        save_backup_manifest(&set, &manifest).unwrap();
    }

    /// What an update records of ReaPack when version 1.2.5 replaces the
    /// version 1.0 the receipt lists.
    fn reapack_replaced() -> ReplacedPackageRecord {
        ReplacedPackageRecord {
            receipt: Some(receipt_entry("reapack")),
            replaced_by: Some(crate::version::Version::parse("1.2.5").unwrap()),
        }
    }

    /// The sets an update writes: the old copy of each file it replaced,
    /// with a copy of the receipt from before, when there was one, as
    /// earlier versions wrote them, or with each package's record, as this
    /// one does. Restoring one whole, or one package of it, is refused in a
    /// dry run and for real, and nothing changes, the receipt least of all.
    #[test]
    fn an_update_set_is_refused_until_earlier_versions_can_come_back() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins/reaper_reapack-x64.dll");
        let receipt = resource.join("RABBIT/install-state.json");
        // The first update over a file installed by hand found no receipt
        // to copy.
        write(&plugin, b"reapack by hand");
        BackupSetWriter::new(&resource, "unix-0", UPDATE_BACKUP_REASON)
            .copy_file(Some("reapack"), &plugin)
            .unwrap();
        write(&plugin, b"reapack 1.2.4");
        write(&receipt, b"records from before the update");
        BackupSetWriter::new(&resource, "unix-1", UPDATE_BACKUP_REASON)
            .copy_file(Some("reapack"), &plugin)
            .unwrap();
        add_receipt_copy(&resource, "unix-1", UPDATE_BACKUP_REASON);
        let mut writer = BackupSetWriter::new(&resource, "unix-2", UPDATE_BACKUP_REASON);
        writer
            .record_replaced("reapack", reapack_replaced())
            .unwrap();
        writer.copy_file(Some("reapack"), &plugin).unwrap();
        fs::write(&plugin, b"reapack 1.2.5").unwrap();
        fs::write(&receipt, b"records from after the update").unwrap();

        for id in ["unix-0", "unix-1", "unix-2"] {
            for package in [None, Some("reapack")] {
                let error = refused_with(dir.path(), &resource, id, package);
                assert!(
                    matches!(refusal(&error), RestoreRefusal::EarlierVersionNotYet { backup_id } if backup_id == id),
                    "{id} {package:?}: {error:?}"
                );
            }
        }
        assert_eq!(
            fs::read(&receipt).unwrap(),
            b"records from after the update"
        );
        assert_eq!(fs::read(&plugin).unwrap(), b"reapack 1.2.5");
    }

    /// A set an update writes now keeps its records apart from an
    /// uninstall's, so the rule older versions read sets by finds no
    /// package in it: they restore its files only, and never merge the
    /// recorded entry back.
    #[test]
    fn an_update_set_s_records_are_no_uninstall_records() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins/reaper_reapack-x64.dll");
        write(&plugin, b"reapack 1.2.4");
        let mut writer = BackupSetWriter::new(&resource, "unix-1", UPDATE_BACKUP_REASON);
        writer
            .record_replaced("reapack", reapack_replaced())
            .unwrap();
        writer.copy_file(Some("reapack"), &plugin).unwrap();
        let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();

        let packages = packages_to_restore(
            "unix-1",
            writer.path(),
            Some(&manifest),
            None,
            CopyCheck::Hash,
        )
        .unwrap();

        assert!(manifest.packages.is_empty());
        assert_eq!(manifest.replaced["reapack"], reapack_replaced());
        assert!(packages.restore.is_empty() && packages.skipped.is_empty());
    }

    /// The set written after vendor installers ran, by earlier versions,
    /// holds only a copy of the receipt, so there is nothing to restore
    /// from it.
    #[test]
    fn a_records_only_set_has_nothing_to_restore() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let receipt = resource.join("RABBIT/install-state.json");
        write(&receipt, b"records from before the installers");
        let id = "unattended-unix-1-2";
        add_receipt_copy(&resource, id, RECORDS_BACKUP_REASON);
        fs::write(&receipt, b"records from after the installers").unwrap();

        let error = refused_with(dir.path(), &resource, id, None);

        assert!(
            matches!(refusal(&error), RestoreRefusal::RecordsOnly { backup_id } if backup_id == id),
            "{error:?}"
        );
        assert_eq!(
            fs::read(&receipt).unwrap(),
            b"records from after the installers"
        );
    }

    /// A restore of an update's set used to copy its receipt back as an
    /// ordinary file, so the current receipt went into the set that restore
    /// wrote. Restoring that set would bring it back, so it is refused,
    /// whole and for one package, however its manifest records the receipt:
    /// relative, as now; absolute, as older versions did, also inside the
    /// folder a portable REAPER has moved from since; through a relative
    /// resource path an older RABBIT was given; or only as the manifest's
    /// receipt backup. Any other kind of set holding a copy is refused the
    /// same way.
    #[test]
    fn a_before_restore_set_holding_the_receipt_is_refused() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins/reaper_reapack-x64.dll");
        let receipt = resource.join("RABBIT/install-state.json");
        write(&plugin, b"reapack 1.2.5");
        write(&receipt, b"records from after the update");
        let id = "restore-unix-1-before-restore";
        let mut writer = BackupSetWriter::new(&resource, id, BEFORE_RESTORE_REASON);
        writer.copy_file(Some("reapack"), &plugin).unwrap();
        writer.copy_file(None, &receipt).unwrap();
        fs::write(&plugin, b"reapack 1.2.4").unwrap();
        fs::write(&receipt, b"records changed since").unwrap();
        let set = writer.path().to_path_buf();

        for package in [None, Some("reapack")] {
            let error = refused_with(dir.path(), &resource, id, package);
            assert!(
                matches!(refusal(&error), RestoreRefusal::ReceiptInSet { backup_id, backup_path }
                    if backup_id == id && *backup_path == set),
                "{package:?}: {error:?}"
            );
        }

        let plugin_entry = legacy_entry(
            Some("reapack"),
            &Path::new("UserPlugins").join("reaper_reapack-x64.dll"),
            &set.join("UserPlugins").join("reaper_reapack-x64.dll"),
        );
        let receipt_copy = set.join("RABBIT").join("install-state.json");
        let absolute = legacy_entry(None, &std::path::absolute(&receipt).unwrap(), &receipt_copy);
        let through_relative_resource = BackupManifestFile {
            original_path: Path::new("REAPER")
                .join("RABBIT")
                .join("install-state.json"),
            backup_path: Path::new("REAPER")
                .join("RABBIT")
                .join("backups")
                .join(id)
                .join("RABBIT")
                .join("install-state.json"),
            ..absolute.clone()
        };
        // A portable REAPER that moved since: the entry names the receipt
        // inside the folder it was in when the set was written.
        let old_resource = std::path::absolute(dir.path().join("OldREAPER")).unwrap();
        let in_moved_folder = BackupManifestFile {
            original_path: old_resource.join("RABBIT").join("install-state.json"),
            backup_path: old_resource
                .join("RABBIT")
                .join("backups")
                .join(id)
                .join("RABBIT")
                .join("install-state.json"),
            ..absolute.clone()
        };
        let mut manifests = Vec::new();
        for receipt_entry in [absolute, through_relative_resource, in_moved_folder] {
            manifests.push((
                BEFORE_RESTORE_REASON,
                vec![plugin_entry.clone(), receipt_entry],
                None,
            ));
        }
        manifests.push((
            BEFORE_RESTORE_REASON,
            vec![plugin_entry.clone()],
            Some(receipt_copy.clone()),
        ));
        for reason in ["uninstall", "osara-keymap-replacement", "something-newer"] {
            let receipt_entry =
                legacy_entry(None, Path::new("RABBIT/install-state.json"), &receipt_copy);
            manifests.push((reason, vec![plugin_entry.clone(), receipt_entry], None));
        }
        for (reason, files, receipt_backup_path) in manifests {
            save_backup_manifest(
                &set,
                &BackupManifest {
                    schema_version: 1,
                    rabbit_version: "0.4.4".to_string(),
                    created_at: id.to_string(),
                    created_unix: None,
                    reason: reason.to_string(),
                    files: files.clone(),
                    receipt_backup_path,
                    packages: Default::default(),
                    replaced: Default::default(),
                    keymap_origin: None,
                },
            )
            .unwrap();

            let error = refused_with(dir.path(), &resource, id, None);

            assert!(
                matches!(refusal(&error), RestoreRefusal::ReceiptInSet { .. }),
                "{reason} {files:?}: {error:?}"
            );
        }
        assert_eq!(fs::read(&receipt).unwrap(), b"records changed since");
        assert_eq!(fs::read(&plugin).unwrap(), b"reapack 1.2.4");
    }

    /// A set a restore wrote that holds no receipt copy, such as one from a
    /// restore of an uninstall's set, restores as before, and the receipt
    /// stays as it is.
    #[test]
    fn a_before_restore_set_without_the_receipt_still_restores() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins/reaper_sws-x64.dll");
        let receipt = resource.join("RABBIT/install-state.json");
        write(&plugin, b"sws before the restore");
        write(&receipt, b"records");
        let id = "restore-unix-1-before-restore";
        BackupSetWriter::new(&resource, id, BEFORE_RESTORE_REASON)
            .copy_file(Some("sws"), &plugin)
            .unwrap();
        fs::write(&plugin, b"sws the restore brought back").unwrap();

        let report =
            restore_backup_set_within(&resource, id, &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        assert_eq!(fs::read(&plugin).unwrap(), b"sws before the restore");
        assert_eq!(report.actions.len(), 1);
        assert!(report.packages.is_empty());
        assert_eq!(fs::read(&receipt).unwrap(), b"records");
    }

    /// The key map OSARA's replaced restores as before, and the receipt
    /// stays as it is.
    #[test]
    fn a_key_map_set_still_restores() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let receipt = resource.join("RABBIT/install-state.json");
        write(&receipt, b"records");

        restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
            .unwrap();

        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert_eq!(fs::read(&receipt).unwrap(), b"records");
    }

    /// A receipt entry for `id`, as an install of version 1.0 writes one.
    fn receipt_entry(id: &str) -> PackageReceipt {
        PackageReceipt {
            id: id.to_string(),
            version: Some(crate::version::Version::parse("1.0").unwrap()),
            source_url: None,
            source_sha256: None,
            variant: None,
            channel: None,
            installed_files: Vec::new(),
            installed_at: Some("unix-1".to_string()),
            rabbit_version: Some("0.4.4".to_string()),
            architecture: None,
            vendor_added_files: Vec::new(),
        }
    }

    /// An uninstall's set holds no receipt copy. Restoring it merges the
    /// package's own entry into the current receipt, which keeps every
    /// entry it gained since, and saves the receipt through that merge.
    #[test]
    fn an_uninstall_set_merges_its_package_into_the_current_receipt() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins/reaper_sws-x64.dll");
        write(&plugin, b"sws");
        let mut writer = BackupSetWriter::new(&resource, "uninstall-unix-1", "uninstall");
        writer
            .record_package(
                "sws",
                BackupPackageRecord {
                    receipt: Some(receipt_entry("sws")),
                    langpack_before: None,
                    with_settings: None,
                },
            )
            .unwrap();
        writer.move_file(Some("sws"), &plugin, None, false).unwrap();
        // Since the uninstall, ReaPack came in, and SWS is declined.
        let mut since = InstallState::default();
        since
            .packages
            .insert("reapack".to_string(), receipt_entry("reapack"));
        since.declined_packages.insert("sws".to_string());
        save_install_state(&resource, &since).unwrap();

        let report =
            restore_backup_set_within(&resource, "uninstall-unix-1", &APPLY, &Default::default())
                .unwrap();

        assert_eq!(fs::read(&plugin).unwrap(), b"sws");
        assert!(report.packages[0].receipt_merged, "{report:?}");
        let state = load_install_state(&resource).unwrap().unwrap();
        assert_eq!(state.packages.get("sws"), Some(&receipt_entry("sws")));
        assert_eq!(
            state.packages.get("reapack"),
            Some(&receipt_entry("reapack"))
        );
        assert!(state.declined_packages.is_empty());
    }

    /// The id of the set [`set_with_an_empty_record`] writes.
    const EMPTY_RECORD_SET: &str = "uninstall-unix-1";

    /// A resource folder `REAPER` in `dir` with an uninstall's set, as an
    /// uninstall of FFmpeg and the German language pack leaves it when the
    /// language pack's file was gone already: both are recorded, with their
    /// receipt entries, and the language line the language pack's record
    /// says it replaced, but only FFmpeg's file went into the set. Since
    /// then, both are on `declined_packages`, and REAPER has no language
    /// pack. Returns the resource folder and FFmpeg's file.
    fn set_with_an_empty_record(dir: &Path) -> (PathBuf, PathBuf) {
        let resource = dir.join("REAPER");
        let ffmpeg = resource.join("UserPlugins").join("avcodec-61.dll");
        write(&ffmpeg, b"ffmpeg");
        write(&resource.join("reaper.ini"), b"[REAPER]\nlangpack=<>\n");
        let mut writer = BackupSetWriter::new(&resource, EMPTY_RECORD_SET, "uninstall");
        for id in ["ffmpeg", "langpack-de"] {
            let record = BackupPackageRecord {
                receipt: Some(receipt_entry(id)),
                langpack_before: (id == "langpack-de").then(|| "German.ReaperLangPack".to_string()),
                with_settings: None,
            };
            writer.record_package(id, record).unwrap();
        }
        writer
            .move_file(Some("ffmpeg"), &ffmpeg, None, false)
            .unwrap();
        let mut since = InstallState::default();
        since
            .declined_packages
            .extend(["ffmpeg".to_string(), "langpack-de".to_string()]);
        save_install_state(&resource, &since).unwrap();
        (resource, ffmpeg)
    }

    /// A package the set records but none of whose files are in it has
    /// nothing to restore: `--package` is refused before anything changes,
    /// in a dry run too, and RABBIT's records stay as they are. The set's
    /// other package still comes back on its own.
    #[test]
    fn a_package_with_none_of_its_files_in_the_set_is_refused_on_its_own() {
        let dir = tempdir().unwrap();
        let (resource, ffmpeg) = set_with_an_empty_record(dir.path());

        let error = refused_with(dir.path(), &resource, EMPTY_RECORD_SET, Some("langpack-de"));

        assert!(
            matches!(refusal(&error), RestoreRefusal::NothingOfItInSet { backup_id, package_ids }
                if backup_id == EMPTY_RECORD_SET && *package_ids == ["langpack-de"]),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "None of the files of langpack-de are in the backup uninstall-unix-1, so there is \
             nothing to restore. RABBIT's records stay as they are"
        );
        let report = restore_backup_set_within(
            &resource,
            EMPTY_RECORD_SET,
            &RestoreBackupOptions {
                package_id: Some("ffmpeg".to_string()),
                ..APPLY
            },
            &OutsideRestoreRules::default(),
        )
        .unwrap();
        assert_eq!(fs::read(&ffmpeg).unwrap(), b"ffmpeg");
        assert_eq!(report.packages.len(), 1);
        assert!(report.skipped_packages.is_empty());
        let state = load_install_state(&resource).unwrap().unwrap();
        assert!(!state.packages.contains_key("langpack-de"));
        assert!(state.declined_packages.contains("langpack-de"));
    }

    /// A restore of the whole set leaves out a package none of whose files
    /// are in it, and says so: its receipt entry isn't merged, it stays on
    /// `declined_packages`, its language line isn't put back, and a newer
    /// install of it since is no reason to refuse the set. The rest of the
    /// set comes back as usual, in a dry run and for real alike.
    #[test]
    fn a_whole_set_restore_leaves_out_a_package_with_none_of_its_files_in_it() {
        let dir = tempdir().unwrap();
        let (resource, ffmpeg) = set_with_an_empty_record(dir.path());
        let reaper_ini = fs::read(resource.join("reaper.ini")).unwrap();
        // ReaPack is recorded with no file too, and was installed again
        // since.
        let set = resource.join("RABBIT/backups").join(EMPTY_RECORD_SET);
        let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
        manifest.packages.insert(
            "reapack".to_string(),
            BackupPackageRecord {
                receipt: Some(receipt_entry("reapack")),
                langpack_before: None,
                with_settings: None,
            },
        );
        save_backup_manifest(&set, &manifest).unwrap();
        let mut state = load_install_state(&resource).unwrap().unwrap();
        let newer_reapack = PackageReceipt {
            installed_at: Some("unix-2".to_string()),
            ..receipt_entry("reapack")
        };
        state
            .packages
            .insert("reapack".to_string(), newer_reapack.clone());
        save_install_state(&resource, &state).unwrap();
        let skipped: Vec<SkippedPackage> = ["langpack-de", "reapack"]
            .into_iter()
            .map(|id| SkippedPackage {
                package_id: id.to_string(),
                reason: SkippedPackageReason::NothingOfItInSet,
            })
            .collect();

        let dry_run = restore_backup_set_within(
            &resource,
            EMPTY_RECORD_SET,
            &RestoreBackupOptions {
                dry_run: true,
                ..APPLY
            },
            &OutsideRestoreRules::default(),
        )
        .unwrap();
        let report = restore_backup_set_within(
            &resource,
            EMPTY_RECORD_SET,
            &APPLY,
            &OutsideRestoreRules::default(),
        )
        .unwrap();

        for report in [&dry_run, &report] {
            assert_eq!(report.skipped_packages, skipped);
            let restored: Vec<&str> = report
                .packages
                .iter()
                .map(|package| package.package_id.as_str())
                .collect();
            assert_eq!(restored, ["ffmpeg"]);
            assert_eq!(report.actions.len(), 1);
        }
        assert_eq!(
            serde_json::to_value(&report).unwrap()["skipped_packages"],
            serde_json::json!([
                {"package_id": "langpack-de", "reason": "nothing-of-it-in-set"},
                {"package_id": "reapack", "reason": "nothing-of-it-in-set"},
            ])
        );
        assert_eq!(fs::read(&ffmpeg).unwrap(), b"ffmpeg");
        let state = load_install_state(&resource).unwrap().unwrap();
        assert_eq!(state.packages.get("ffmpeg"), Some(&receipt_entry("ffmpeg")));
        assert!(!state.packages.contains_key("langpack-de"));
        assert_eq!(state.packages.get("reapack"), Some(&newer_reapack));
        let declined: Vec<&str> = state.declined_packages.iter().map(String::as_str).collect();
        assert_eq!(declined, ["langpack-de"]);
        assert_eq!(fs::read(resource.join("reaper.ini")).unwrap(), reaper_ini);
    }

    /// A pending entry counts as a package's file only when its copy is
    /// complete, as the restore itself decides: here SWS's run died after
    /// its copy and ReaPack's before it. ReaPack then has nothing in the set:
    /// `--package` is refused, and the whole set brings back SWS alone.
    #[test]
    fn a_pending_entry_counts_as_a_file_only_when_its_copy_is_complete() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let sws = resource.join("UserPlugins").join("reaper_sws-x64.dll");
        let reapack = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        write(&sws, b"sws");
        write(&reapack, b"reapack");
        let id = "uninstall-unix-1";
        let mut writer = BackupSetWriter::new(&resource, id, "uninstall");
        for package in ["reapack", "sws"] {
            let record = BackupPackageRecord {
                receipt: Some(receipt_entry(package)),
                langpack_before: None,
                with_settings: None,
            };
            writer.record_package(package, record).unwrap();
        }
        let copied = writer.begin_entry(Some("sws"), &sws).unwrap();
        let copy = writer.manifest.files[copied].backup_path.clone();
        fs::create_dir_all(copy.parent().unwrap()).unwrap();
        fs::rename(&sws, &copy).unwrap();
        writer.begin_entry(Some("reapack"), &reapack).unwrap();
        drop(writer);

        let error = refused_with(dir.path(), &resource, id, Some("reapack"));
        assert!(
            matches!(refusal(&error), RestoreRefusal::NothingOfItInSet { package_ids, .. }
                if *package_ids == ["reapack"]),
            "{error:?}"
        );

        let report =
            restore_backup_set_within(&resource, id, &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        assert_eq!(fs::read(&sws).unwrap(), b"sws");
        assert_eq!(fs::read(&reapack).unwrap(), b"reapack");
        assert_eq!(report.skipped_targets, vec![reapack]);
        assert_eq!(
            report.skipped_packages,
            vec![SkippedPackage {
                package_id: "reapack".to_string(),
                reason: SkippedPackageReason::NothingOfItInSet,
            }]
        );
        let state = load_install_state(&resource).unwrap().unwrap();
        assert!(state.packages.contains_key("sws"));
        assert!(!state.packages.contains_key("reapack"));
    }

    /// A set whose packages all have none of their files in it, and that
    /// holds no other file, has nothing to restore. Its restore is refused
    /// like `--package`, naming every package, rather than reported as done
    /// while bringing nothing back. A file of no package still comes back
    /// on its own, and the packages are left out.
    #[test]
    fn a_set_with_nothing_but_records_without_files_is_refused() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"own keymap");
        let record_both = |writer: &mut BackupSetWriter| {
            for id in ["sws", "reapack"] {
                let record = BackupPackageRecord {
                    receipt: Some(receipt_entry(id)),
                    langpack_before: None,
                    with_settings: None,
                };
                writer.record_package(id, record).unwrap();
            }
        };
        record_both(&mut BackupSetWriter::new(
            &resource,
            "uninstall-unix-1",
            "uninstall",
        ));

        let error = refused_with(dir.path(), &resource, "uninstall-unix-1", None);

        assert!(
            matches!(refusal(&error), RestoreRefusal::NothingOfItInSet { package_ids, .. }
                if *package_ids == ["reapack", "sws"]),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "None of the files of reapack and sws are in the backup uninstall-unix-1, so there \
             is nothing to restore. RABBIT's records stay as they are"
        );

        let mut writer = BackupSetWriter::new(&resource, "uninstall-unix-2", "uninstall");
        record_both(&mut writer);
        writer.copy_file(None, &keymap).unwrap();
        fs::write(&keymap, b"osara keymap").unwrap();
        let report = restore_backup_set_within(
            &resource,
            "uninstall-unix-2",
            &APPLY,
            &OutsideRestoreRules::default(),
        )
        .unwrap();
        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert!(report.packages.is_empty());
        assert_eq!(report.skipped_packages.len(), 2);
        assert!(load_install_state(&resource).unwrap().is_none());
    }

    /// A pending entry whose copy can't be checked counts as a file of its
    /// package, so the restore reaches it and says what is wrong, here a
    /// link on the way to the copy, rather than that nothing of the package
    /// is in the set.
    #[test]
    fn a_pending_copy_behind_a_link_is_refused_as_a_link() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let sws = resource.join("UserPlugins").join("reaper_sws-x64.dll");
        write(&sws, b"sws");
        let id = "uninstall-unix-1";
        let mut writer = BackupSetWriter::new(&resource, id, "uninstall");
        let record = BackupPackageRecord {
            receipt: Some(receipt_entry("sws")),
            langpack_before: None,
            with_settings: None,
        };
        writer.record_package("sws", record).unwrap();
        let entry = writer.begin_entry(Some("sws"), &sws).unwrap();
        let copy = writer.manifest.files[entry].backup_path.clone();
        drop(writer);
        // The copy's folder in the set is a link to a folder elsewhere,
        // which holds a complete copy.
        let elsewhere = dir.path().join("Elsewhere");
        write(&elsewhere.join(copy.file_name().unwrap()), b"sws");
        link_folder(copy.parent().unwrap(), &elsewhere);
        let before = snapshot(dir.path());

        for dry_run in [true, false] {
            let options = RestoreBackupOptions {
                dry_run,
                package_id: Some("sws".to_string()),
                ..APPLY
            };
            let error =
                restore_backup_set_within(&resource, id, &options, &OutsideRestoreRules::default())
                    .unwrap_err();
            assert!(
                matches!(refusal(&error), RestoreRefusal::IsLink { .. }),
                "{dry_run}: {error:?}"
            );
            assert_eq!(snapshot(dir.path()), before, "{dry_run}");
        }
    }

    /// An entry of a package the set no longer records, as one an uninstall
    /// forgot after putting its files back can leave, comes back with the
    /// whole set like a file of no package. A package the set records with
    /// none of its files is still left out.
    #[test]
    fn a_file_of_a_package_the_set_no_longer_records_still_comes_back() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"own keymap");
        let id = "uninstall-unix-1";
        let mut writer = BackupSetWriter::new(&resource, id, "uninstall");
        for package in ["sws", "forgotten"] {
            let record = BackupPackageRecord {
                receipt: Some(receipt_entry(package)),
                langpack_before: None,
                with_settings: None,
            };
            writer.record_package(package, record).unwrap();
        }
        writer.copy_file(Some("forgotten"), &keymap).unwrap();
        writer.forget_package("forgotten").unwrap();
        drop(writer);
        fs::write(&keymap, b"osara keymap").unwrap();

        let report =
            restore_backup_set_within(&resource, id, &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert!(report.packages.is_empty());
        assert_eq!(
            report.skipped_packages,
            vec![SkippedPackage {
                package_id: "sws".to_string(),
                reason: SkippedPackageReason::NothingOfItInSet,
            }]
        );
        assert!(load_install_state(&resource).unwrap().is_none());
    }

    /// A refusal of a whole set names the set's other packages, which can
    /// be restored one at a time, only when another package has a file in
    /// the set: one left out would be refused on its own too. Here FFmpeg's
    /// file is refused, first because FFmpeg was installed again since,
    /// then because of where the file goes, and the language pack has no
    /// file in the set.
    #[test]
    fn a_whole_set_refusal_names_only_packages_with_a_file_in_the_set() {
        let dir = tempdir().unwrap();
        let (resource, _) = set_with_an_empty_record(dir.path());
        let mut state = load_install_state(&resource).unwrap().unwrap();
        let newer_ffmpeg = PackageReceipt {
            installed_at: Some("unix-2".to_string()),
            ..receipt_entry("ffmpeg")
        };
        state.packages.insert("ffmpeg".to_string(), newer_ffmpeg);
        save_install_state(&resource, &state).unwrap();
        let restore = || {
            restore_backup_set_within(
                &resource,
                EMPTY_RECORD_SET,
                &APPLY,
                &OutsideRestoreRules::default(),
            )
            .unwrap_err()
        };

        let error = restore();
        assert!(
            matches!(refusal(&error), RestoreRefusal::ReinstalledSince { package_id, other_packages, .. }
                if package_id == "ffmpeg" && other_packages.is_empty()),
            "{error:?}"
        );

        let set = resource.join("RABBIT/backups").join(EMPTY_RECORD_SET);
        let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
        manifest.files[0].original_path = dir.path().join("Elsewhere").join("avcodec-61.dll");
        save_backup_manifest(&set, &manifest).unwrap();
        state.packages.remove("ffmpeg");
        save_install_state(&resource, &state).unwrap();

        let error = restore();
        assert!(
            matches!(refusal(&error), RestoreRefusal::TargetRefused {
                reason: TargetRefusal::OutsideAllowedFolders,
                other_packages,
                ..
            } if other_packages.is_empty()),
            "{error:?}"
        );
    }

    /// The plan, made under the lock, must agree with the packages chosen
    /// before it about which have a file in the set. When an uninstall
    /// finished writing the set, or took a copy away, in between, the two
    /// differ, and the restore stops before anything changes.
    #[test]
    fn a_plan_that_disagrees_with_the_packages_chosen_is_refused() {
        let plan = |ids: &[&str]| RestorePlan {
            files: ids
                .iter()
                .map(|id| PlannedRestore {
                    package_id: Some(id.to_string()),
                    source_path: PathBuf::from("copy"),
                    target_path: PathBuf::from("file"),
                    size: 0,
                    sha256: String::new(),
                })
                .collect(),
            skipped_targets: Vec::new(),
        };
        let left_out = |id: &str| SkippedPackage {
            package_id: id.to_string(),
            reason: SkippedPackageReason::NothingOfItInSet,
        };
        let check = |plan: &RestorePlan, restoring: &[&str], skipped: &[SkippedPackage]| {
            check_plan_matches_packages("set", plan, restoring, skipped)
        };

        check(&plan(&["sws"]), &["sws"], &[left_out("reapack")]).unwrap();
        check(&plan(&[]), &[], &[]).unwrap();
        // A package to restore with no file in the plan, a package left out
        // with one, and no file at all where every package was left out.
        let disagreeing = [
            (plan(&["sws"]), vec!["sws", "reapack"], vec![]),
            (
                plan(&["sws", "reapack"]),
                vec!["sws"],
                vec![left_out("reapack")],
            ),
            (plan(&[]), vec![], vec![left_out("reapack")]),
        ];
        for (plan, restoring, skipped) in disagreeing {
            let error = check(&plan, &restoring, &skipped).unwrap_err();
            assert!(
                matches!(refusal(&error), RestoreRefusal::ChangedWhileRead { backup_id }
                    if backup_id == "set"),
                "{restoring:?}, {skipped:?}: {error:?}"
            );
        }
    }

    /// On a Mac, a real restore as root, as under sudo, is refused as an
    /// uninstall is: after the set is found and the set and package
    /// refusals, before the REAPER check, the lock and any change. A dry run
    /// still says what a restore would do. The check is the environment's,
    /// so this runs on every platform.
    #[test]
    fn a_real_restore_as_root_is_refused_and_a_dry_run_still_works() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let as_root = BackupEnvironment {
            runs_as_root: &|| true,
            ..BackupEnvironment::AS_USER
        };
        let before = snapshot(dir.path());

        let error = restore_backup_set_with(&resource, "unix-1", &APPLY, &as_root, || {
            panic!("a restore as root looked up where outside files go")
        })
        .unwrap_err();

        assert!(
            matches!(refusal(&error), RestoreRefusal::AsRoot),
            "{error:?}"
        );
        let text = error.to_string();
        assert!(
            text.contains("not with sudo") && !text.contains("--"),
            "{text}"
        );
        assert_eq!(snapshot(dir.path()), before);
        assert!(!resource.join("RABBIT").join("locks").exists());

        let dry_run = RestoreBackupOptions {
            dry_run: true,
            ..APPLY
        };
        let report = restore_backup_set_with(
            &resource,
            "unix-1",
            &dry_run,
            &as_root,
            OutsideRestoreRules::default,
        )
        .unwrap();
        assert_eq!(
            report.actions[0].action,
            RestoreBackupActionKind::WouldRestore
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");

        restore_backup_set_with(
            &resource,
            "unix-1",
            &APPLY,
            &BackupEnvironment::AS_USER,
            OutsideRestoreRules::default,
        )
        .unwrap();
        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        // This computer's own answer, which is uninstall's: off a Mac never
        // root, and on one this test runs as the user.
        assert!(!(BackupEnvironment::system().runs_as_root)());
    }

    /// A resource folder `REAPER` in `dir`, a portable REAPER's, with an
    /// uninstall's set `uninstall-unix-1` that holds SWS's plug-in, gone
    /// from its place. Returns the resource folder and the plug-in.
    fn sws_uninstalled(dir: &Path) -> (PathBuf, PathBuf) {
        let resource = dir.join("REAPER");
        let plugin = resource.join("UserPlugins").join("reaper_sws-x64.dll");
        write(&plugin, b"sws");
        let mut writer = BackupSetWriter::new(&resource, "uninstall-unix-1", "uninstall");
        writer
            .record_package(
                "sws",
                BackupPackageRecord {
                    receipt: Some(receipt_entry("sws")),
                    langpack_before: None,
                    with_settings: None,
                },
            )
            .unwrap();
        writer.move_file(Some("sws"), &plugin, None, false).unwrap();
        (resource, plugin)
    }

    /// A REAPER running from the portable REAPER at `resource`, the target
    /// of a restore there: its `reaper.exe` on Windows, its app on macOS.
    /// The app is made, so the target is found in the resource folder and
    /// the computer's own REAPER is never looked for.
    fn targets_reaper(resource: &Path) -> RunningProcess {
        #[cfg(windows)]
        let (name, app) = ("reaper.exe", resource.join("reaper.exe"));
        #[cfg(not(windows))]
        let (name, app) = (
            "REAPER",
            resource
                .join("REAPER.app")
                .join("Contents")
                .join("MacOS")
                .join("REAPER"),
        );
        write(&app, b"reaper");
        RunningProcess {
            pid: "100".to_string(),
            name: name.to_string(),
            executable_path: Some(app),
        }
    }

    /// Another REAPER on the computer, a portable one elsewhere in `dir`.
    fn another_reaper(dir: &Path) -> RunningProcess {
        RunningProcess {
            pid: "200".to_string(),
            name: "reaper.exe".to_string(),
            executable_path: Some(dir.join("Elsewhere").join("reaper.exe")),
        }
    }

    /// SWS as this platform's package manifests describe it, changed by
    /// `change`, as the only package a test's computer knows.
    fn sws_spec(change: impl FnOnce(&mut PackageSpec)) -> Vec<PackageSpec> {
        let platform = Platform::current().expect("tests run on Windows or macOS");
        let mut spec = crate::package::builtin_package_specs(platform)
            .into_iter()
            .find(|spec| spec.id == "sws")
            .unwrap();
        change(&mut spec);
        vec![spec]
    }

    /// The check named `name` in a restore's report.
    fn check_named<'a>(report: &'a super::RestoreBackupReport, name: &str) -> &'a PreflightCheck {
        report
            .preflight
            .checks
            .iter()
            .find(|check| check.name == name)
            .unwrap_or_else(|| panic!("no {name} check in {:#?}", report.preflight))
    }

    /// A restore checks what the package's own uninstall checks. A package
    /// whose uninstall needs every REAPER closed, as a machine-wide one
    /// does, is refused while any REAPER runs, not only the target's, and
    /// `allow_reaper_running` doesn't let it through. The refusal names the
    /// package by the name RABBIT shows. A dry run reports the REAPER and
    /// refuses nothing for it. Nothing changes, and the lock is never
    /// taken. The same package, as the manifests ship it, restores while a
    /// REAPER other than the target's runs.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn a_package_whose_uninstall_needs_every_reaper_closed_is_refused_while_any_runs() {
        let dir = tempdir().unwrap();
        let (resource, plugin) = sws_uninstalled(dir.path());
        targets_reaper(&resource);
        let elsewhere = another_reaper(dir.path());
        let running = {
            let elsewhere = elsewhere.clone();
            move || vec![elsewhere.clone()]
        };
        let machine_wide = sws_spec(|spec| spec.requires_standard_install = true);
        let name = machine_wide[0].display_name.clone();
        let specs = move |_: Platform| machine_wide.clone();
        let environment = BackupEnvironment {
            reaper_processes: &running,
            package_specs: &specs,
            ..BackupEnvironment::AS_USER
        };
        let id = "uninstall-unix-1";
        let before = snapshot(dir.path());

        for allow_reaper_running in [false, true] {
            let options = RestoreBackupOptions {
                allow_reaper_running,
                ..APPLY
            };
            let error = restore_backup_set_with(
                &resource,
                id,
                &options,
                &environment,
                OutsideRestoreRules::default,
            )
            .unwrap_err();

            assert!(
                matches!(refusal(&error), RestoreRefusal::ReaperRunning {
                    every_reaper_for,
                    package_names,
                    processes,
                } if every_reaper_for == &["sws"]
                    && package_names == std::slice::from_ref(&name)
                    && processes == std::slice::from_ref(&elsewhere)),
                "{allow_reaper_running}: {error:?}"
            );
            assert_eq!(snapshot(dir.path()), before, "{allow_reaper_running}");
            assert!(!resource.join("RABBIT").join("locks").exists());
        }

        let dry_run = RestoreBackupOptions {
            dry_run: true,
            allow_reaper_running: false,
            package_id: None,
        };
        let report = restore_backup_set_with(
            &resource,
            id,
            &dry_run,
            &environment,
            OutsideRestoreRules::default,
        )
        .unwrap();
        assert!(report.preflight.passed);
        let every = check_named(&report, ANY_REAPER_PROCESS_CHECK);
        assert_eq!(every.status, PreflightStatus::Warn);
        assert!(every.message.contains(&name), "{}", every.message);
        assert_eq!(
            check_named(&report, crate::preflight::REAPER_PROCESS_CHECK).status,
            PreflightStatus::Pass
        );
        assert_eq!(snapshot(dir.path()), before);

        let as_shipped = BackupEnvironment {
            reaper_processes: &running,
            ..BackupEnvironment::AS_USER
        };
        let report = restore_backup_set_with(
            &resource,
            id,
            &RestoreBackupOptions {
                allow_reaper_running: false,
                ..APPLY
            },
            &as_shipped,
            OutsideRestoreRules::default,
        )
        .unwrap();
        assert_eq!(fs::read(&plugin).unwrap(), b"sws");
        assert!(
            !report
                .preflight
                .checks
                .iter()
                .any(|check| check.name == ANY_REAPER_PROCESS_CHECK)
        );
    }

    /// A restore is refused while a program the package's uninstall plan
    /// names runs, whatever `allow_reaper_running` says, and only those
    /// programs are asked about. A dry run reports them and refuses
    /// nothing. A package whose plan names none asks about no program at
    /// all.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn a_restore_is_refused_while_a_program_its_uninstall_names_runs() {
        let dir = tempdir().unwrap();
        let (resource, plugin) = sws_uninstalled(dir.path());
        targets_reaper(&resource);
        let blocked = sws_spec(|spec| {
            for plan in &mut spec.uninstall.plans {
                plan.blocking_programs = vec!["Demo Tool.exe".to_string()];
            }
        });
        let name = blocked[0].display_name.clone();
        let specs = move |_: Platform| blocked.clone();
        let asked = std::cell::RefCell::new(Vec::new());
        let running = |names: &[String]| {
            asked.borrow_mut().push(names.to_vec());
            ["demo tool.exe", "notepad.exe"]
                .into_iter()
                .map(|name| RunningProcess {
                    pid: "300".to_string(),
                    name: name.to_string(),
                    executable_path: None,
                })
                .collect()
        };
        let environment = BackupEnvironment {
            named_processes: &running,
            package_specs: &specs,
            ..BackupEnvironment::AS_USER
        };
        let id = "uninstall-unix-1";
        let before = snapshot(dir.path());

        let error = restore_backup_set_with(
            &resource,
            id,
            &APPLY,
            &environment,
            OutsideRestoreRules::default,
        )
        .unwrap_err();

        assert!(
            matches!(refusal(&error), RestoreRefusal::ProgramRunning {
                package_ids,
                package_names,
                programs,
            } if package_ids == &["sws"]
                && package_names == std::slice::from_ref(&name)
                && programs == &["demo tool.exe"]),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            format!("Close these programs before restoring {name}: demo tool.exe")
        );
        assert_eq!(*asked.borrow(), [vec!["Demo Tool.exe".to_string()]]);
        assert_eq!(snapshot(dir.path()), before);

        let report = restore_backup_set_with(
            &resource,
            id,
            &RestoreBackupOptions {
                dry_run: true,
                ..APPLY
            },
            &environment,
            OutsideRestoreRules::default,
        )
        .unwrap();
        assert!(report.preflight.passed);
        let programs = check_named(&report, PROGRAMS_RUNNING_CHECK);
        assert_eq!(programs.status, PreflightStatus::Warn);
        assert_eq!(programs.message, format!("{}.", refusal(&error)));

        let no_programs_asked = |_: &[String]| -> Vec<RunningProcess> {
            panic!("a package whose uninstall names no program asked about programs")
        };
        let as_shipped = BackupEnvironment {
            named_processes: &no_programs_asked,
            ..BackupEnvironment::AS_USER
        };
        restore_backup_set_with(
            &resource,
            id,
            &APPLY,
            &as_shipped,
            OutsideRestoreRules::default,
        )
        .unwrap();
        assert_eq!(fs::read(&plugin).unwrap(), b"sws");
    }

    /// A restore follows the uninstall plan for the kind of REAPER it
    /// restores into, as an uninstall does: a program only the standard
    /// REAPER's plan names refuses a restore into the standard REAPER, and
    /// not one into a portable REAPER. RABBIT tells the two apart by the
    /// path alone, so a temporary folder is a portable REAPER.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn a_restore_follows_the_uninstall_plan_for_its_kind_of_reaper() {
        use crate::model::InstallationKind;
        use crate::package::uninstall::UninstallTarget;

        let dir = tempdir().unwrap();
        let (resource, plugin) = sws_uninstalled(dir.path());
        let platform = Platform::current().unwrap();
        let standard_only = sws_spec(|spec| {
            let mut plan = spec
                .uninstall
                .plan_for(platform, InstallationKind::Portable)
                .expect("SWS comes off a portable REAPER")
                .clone();
            plan.target = UninstallTarget::Standard;
            plan.blocking_programs = vec!["Demo Tool.exe".to_string()];
            spec.uninstall.plans.insert(0, plan);
        });
        let specs = move |_: Platform| standard_only.clone();
        let running = |_: &[String]| {
            vec![RunningProcess {
                pid: "300".to_string(),
                name: "Demo Tool.exe".to_string(),
                executable_path: None,
            }]
        };
        let as_kind =
            |kind: InstallationKind| move |_: Platform, _: &Path| -> InstallationKind { kind };
        let standard = as_kind(InstallationKind::Standard);
        let portable = as_kind(InstallationKind::Portable);
        let restore = |target_kind: &dyn Fn(Platform, &Path) -> InstallationKind| {
            restore_backup_set_with(
                &resource,
                "uninstall-unix-1",
                &APPLY,
                &BackupEnvironment {
                    named_processes: &running,
                    package_specs: &specs,
                    target_kind,
                    ..BackupEnvironment::AS_USER
                },
                OutsideRestoreRules::default,
            )
        };

        let error = restore(&standard).unwrap_err();
        assert!(
            matches!(refusal(&error), RestoreRefusal::ProgramRunning { package_ids, .. }
                if package_ids == &["sws"]),
            "{error:?}"
        );
        assert!(!plugin.exists());

        restore(&portable).unwrap();
        assert_eq!(fs::read(&plugin).unwrap(), b"sws");
        assert_eq!(
            crate::uninstall::execute::target_kind_of(platform, &resource),
            InstallationKind::Portable
        );
    }

    /// An uninstall's set `uninstall-unix-1` in a portable REAPER `REAPER`
    /// in `dir`, holding the plug-ins of SWS and ReaPack, both gone from
    /// their place. Returns the resource folder.
    fn sws_and_reapack_uninstalled(dir: &Path) -> PathBuf {
        let resource = dir.join("REAPER");
        let mut writer = BackupSetWriter::new(&resource, "uninstall-unix-1", "uninstall");
        for (package_id, file) in [
            ("sws", "reaper_sws-x64.dll"),
            ("reapack", "reaper_reapack-x64.dll"),
        ] {
            let plugin = resource.join("UserPlugins").join(file);
            write(&plugin, package_id.as_bytes());
            writer
                .record_package(
                    package_id,
                    BackupPackageRecord {
                        receipt: Some(receipt_entry(package_id)),
                        langpack_before: None,
                        with_settings: None,
                    },
                )
                .unwrap();
            writer
                .move_file(Some(package_id), &plugin, None, false)
                .unwrap();
        }
        resource
    }

    /// Only the packages a restore brings back count: a program another
    /// package of the set names refuses the whole set, and never a restore
    /// of one package without it.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn only_the_packages_a_restore_brings_back_have_their_programs_checked() {
        let dir = tempdir().unwrap();
        let resource = sws_and_reapack_uninstalled(dir.path());
        let platform = Platform::current().unwrap();
        let specs: Vec<PackageSpec> = crate::package::builtin_package_specs(platform)
            .into_iter()
            .filter(|spec| spec.id == "sws" || spec.id == "reapack")
            .map(|mut spec| {
                if spec.id == "reapack" {
                    for plan in &mut spec.uninstall.plans {
                        plan.blocking_programs = vec!["Demo Tool.exe".to_string()];
                    }
                }
                spec
            })
            .collect();
        let specs = move |_: Platform| specs.clone();
        let asked = std::cell::Cell::new(0);
        let running = |_: &[String]| {
            asked.set(asked.get() + 1);
            vec![RunningProcess {
                pid: "300".to_string(),
                name: "Demo Tool.exe".to_string(),
                executable_path: None,
            }]
        };
        let environment = BackupEnvironment {
            named_processes: &running,
            package_specs: &specs,
            ..BackupEnvironment::AS_USER
        };
        let restore = |package_id: Option<&str>| {
            restore_backup_set_with(
                &resource,
                "uninstall-unix-1",
                &RestoreBackupOptions {
                    package_id: package_id.map(ToString::to_string),
                    ..APPLY
                },
                &environment,
                OutsideRestoreRules::default,
            )
        };

        let error = restore(None).unwrap_err();
        assert!(
            matches!(refusal(&error), RestoreRefusal::ProgramRunning { package_ids, .. }
                if package_ids == &["reapack"]),
            "{error:?}"
        );
        assert_eq!(asked.get(), 1);

        restore(Some("sws")).unwrap();
        assert_eq!(
            fs::read(resource.join("UserPlugins").join("reaper_sws-x64.dll")).unwrap(),
            b"sws"
        );
        assert!(
            !resource
                .join("UserPlugins")
                .join("reaper_reapack-x64.dll")
                .exists()
        );
        assert_eq!(
            asked.get(),
            1,
            "a program only ReaPack names was asked about"
        );
    }

    /// `allow_reaper_running` lets only the target's REAPER run, as it
    /// always did. Without it a real restore is refused while the target's
    /// REAPER runs, in words that say restore, not install; a dry run
    /// reports it and refuses nothing, as the install preflight's dry run
    /// does. Another REAPER refuses nothing for a package whose uninstall
    /// doesn't need every REAPER closed.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn allow_reaper_running_lets_only_the_targets_reaper_run() {
        let dir = tempdir().unwrap();
        let (resource, plugin) = sws_uninstalled(dir.path());
        let target = targets_reaper(&resource);
        let both = [target.clone(), another_reaper(dir.path())];
        let running = move || both.to_vec();
        let environment = BackupEnvironment {
            reaper_processes: &running,
            ..BackupEnvironment::AS_USER
        };
        let id = "uninstall-unix-1";
        let before = snapshot(dir.path());
        let restore = |options: &RestoreBackupOptions| {
            restore_backup_set_with(
                &resource,
                id,
                options,
                &environment,
                OutsideRestoreRules::default,
            )
        };

        let error = restore(&RestoreBackupOptions {
            allow_reaper_running: false,
            ..APPLY
        })
        .unwrap_err();
        assert!(
            matches!(refusal(&error), RestoreRefusal::ReaperRunning {
                every_reaper_for,
                package_names,
                processes,
            } if every_reaper_for.is_empty()
                && package_names.is_empty()
                && processes == std::slice::from_ref(&target)),
            "{error:?}"
        );
        assert!(
            error
                .to_string()
                .starts_with("Close REAPER before restoring a backup: "),
            "{error}"
        );
        assert_eq!(snapshot(dir.path()), before);

        let report = restore(&RestoreBackupOptions {
            dry_run: true,
            allow_reaper_running: false,
            package_id: None,
        })
        .unwrap();
        assert!(report.preflight.passed);
        assert_eq!(
            check_named(&report, crate::preflight::REAPER_PROCESS_CHECK).status,
            PreflightStatus::Warn
        );
        assert_eq!(snapshot(dir.path()), before);

        let report = restore(&APPLY).unwrap();
        assert_eq!(fs::read(&plugin).unwrap(), b"sws");
        assert_eq!(
            check_named(&report, crate::preflight::REAPER_PROCESS_CHECK).status,
            PreflightStatus::Warn
        );
        assert!(
            !report
                .preflight
                .checks
                .iter()
                .any(|check| check.name == ANY_REAPER_PROCESS_CHECK
                    || check.name == PROGRAMS_RUNNING_CHECK)
        );
    }

    /// What runs is checked again under the lock: a REAPER that started
    /// after the checks before the lock still stops the restore, with
    /// nothing changed and the lock given back.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn what_runs_is_checked_again_under_the_lock() {
        let dir = tempdir().unwrap();
        let (resource, plugin) = sws_uninstalled(dir.path());
        let target = targets_reaper(&resource);
        let scans = std::cell::Cell::new(0);
        let running = || {
            scans.set(scans.get() + 1);
            if scans.get() == 1 {
                Vec::new()
            } else {
                vec![target.clone()]
            }
        };
        let environment = BackupEnvironment {
            reaper_processes: &running,
            ..BackupEnvironment::AS_USER
        };

        let error = restore_backup_set_with(
            &resource,
            "uninstall-unix-1",
            &RestoreBackupOptions {
                allow_reaper_running: false,
                ..APPLY
            },
            &environment,
            OutsideRestoreRules::default,
        )
        .unwrap_err();

        assert!(
            matches!(refusal(&error), RestoreRefusal::ReaperRunning { processes, .. }
                if processes == std::slice::from_ref(&target)),
            "{error:?}"
        );
        assert_eq!(scans.get(), 2);
        assert!(!plugin.exists());
        assert!(
            !resource
                .join("RABBIT")
                .join("locks")
                .join("package-install.lock")
                .exists()
        );
        assert_eq!(list_backup_sets(&resource).unwrap().len(), 1);
    }

    /// Puts a folder's permissions back as they were when it is dropped,
    /// so a test that made it read-only leaves a folder its temporary
    /// directory can remove, even when it fails.
    struct PermissionsBack(PathBuf, fs::Permissions);

    impl Drop for PermissionsBack {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, self.1.clone());
        }
    }

    /// A resource folder RABBIT can't write to stops a restore before
    /// anything changes, a dry run too, with the check that failed: the
    /// install preflight's own check of the resource folder.
    #[test]
    fn a_resource_folder_that_cant_be_written_to_is_refused() {
        let dir = tempdir().unwrap();
        let (resource, plugin) = sws_uninstalled(dir.path());
        let permissions = fs::metadata(&resource).unwrap().permissions();
        let _back = PermissionsBack(resource.clone(), permissions.clone());
        let mut read_only = permissions;
        read_only.set_readonly(true);
        fs::set_permissions(&resource, read_only).unwrap();

        for dry_run in [false, true] {
            let error = restore_backup_set_within(
                &resource,
                "uninstall-unix-1",
                &RestoreBackupOptions { dry_run, ..APPLY },
                &OutsideRestoreRules::default(),
            )
            .unwrap_err();
            assert!(
                matches!(refusal(&error), RestoreRefusal::NotWritable { path, check, .. }
                    if *path == resource && check == "resource-path"),
                "{dry_run}: {error:?}"
            );
            assert_eq!(refusal(&error).code(), "not-writable");
        }
        assert!(!plugin.exists());
    }

    /// Only an error before the first change is a refusal. Here the restore
    /// puts one file back, then finds `RABBIT/backups` turned into a link
    /// when it backs up the file the second one replaces: that error is
    /// reported as it is, not as the refusal it would be beforehand, and
    /// the restore is undone: the first file, which it added, goes again.
    #[test]
    fn a_link_found_after_a_file_went_back_is_no_refusal() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let set = resource.join("RABBIT/backups/unix-1");
        let first = Path::new("UserPlugins").join("first.dll");
        let second = Path::new("UserPlugins").join("second.dll");
        write(&set.join(&first), b"first");
        write(&set.join(&second), b"older second");
        write(&resource.join(&second), b"newer second");
        save_manifest(
            &set,
            RESTORABLE_REASON,
            vec![
                legacy_entry(None, &first, &set.join(&first)),
                legacy_entry(None, &second, &set.join(&second)),
            ],
        );
        let backups = resource.join("RABBIT").join("backups");
        let moved = dir.path().join("moved-backups");
        // Right before the second file, once the first is back.
        let second_place = resource.join(&second);
        let (from, to) = (backups.clone(), moved.clone());
        let _hook = super::restore_seam::install(move |step, path| {
            if step == super::restore_seam::Step::Restoring && path == second_place {
                fs::rename(&from, &to)?;
                link_folder(&from, &to);
            }
            Ok(())
        });

        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        assert_eq!(report.status, super::RestoreStatus::NotRestored);
        let expected = RabbitError::BackupPathIsLink {
            path: backups.clone(),
        }
        .to_string();
        assert_eq!(report.error.as_deref(), Some(expected.as_str()));
        assert!(!resource.join(&first).exists());
        assert_eq!(fs::read(resource.join(&second)).unwrap(), b"newer second");
        let kinds: Vec<RestoreBackupActionKind> =
            report.actions.iter().map(|action| action.action).collect();
        assert_eq!(
            kinds,
            [
                RestoreBackupActionKind::PutBack,
                RestoreBackupActionKind::NotRestored
            ]
        );
        assert_eq!(report.before_restore_set_id, None);
    }

    /// A dry run's report that carries warnings, here the target's REAPER
    /// running, and a package left out keeps the shape scripts read: the
    /// same fields, with only `status` added, each check as its name,
    /// status and message, and the package left out by the same code a
    /// refusal of it alone gives.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn a_dry_run_report_with_warnings_keeps_its_json_shape() {
        let dir = tempdir().unwrap();
        let (resource, _) = set_with_an_empty_record(dir.path());
        let target = targets_reaper(&resource);
        let running = move || vec![target.clone()];
        let environment = BackupEnvironment {
            reaper_processes: &running,
            ..BackupEnvironment::AS_USER
        };

        let report = restore_backup_set_with(
            &resource,
            EMPTY_RECORD_SET,
            &RestoreBackupOptions {
                dry_run: true,
                allow_reaper_running: false,
                package_id: None,
            },
            &environment,
            OutsideRestoreRules::default,
        )
        .unwrap();

        let json = serde_json::to_value(&report).unwrap();
        let keys = |value: &serde_json::Value| {
            let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
            keys.sort();
            keys
        };
        assert_eq!(
            keys(&json),
            [
                "actions",
                "backup_id",
                "backup_path",
                "dry_run",
                "packages",
                "preflight",
                "resource_path",
                "skipped_packages",
                "status"
            ]
        );
        assert_eq!(json["status"], "would-restore");
        assert_eq!(keys(&json["preflight"]), ["checks", "passed"]);
        for check in json["preflight"]["checks"].as_array().unwrap() {
            assert_eq!(keys(check), ["message", "name", "status"]);
        }
        assert_eq!(json["preflight"]["passed"], true);
        assert!(
            json["preflight"]["checks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|check| check["name"] == "reaper-process" && check["status"] == "warn"),
            "{json:#}"
        );
        assert_eq!(
            json["skipped_packages"],
            serde_json::json!([{"package_id": "langpack-de", "reason": "nothing-of-it-in-set"}])
        );
        let alone = RestoreRefusal::NothingOfItInSet {
            backup_id: EMPTY_RECORD_SET.to_string(),
            package_ids: vec!["langpack-de".to_string()],
        };
        assert_eq!(json["skipped_packages"][0]["reason"], alone.code());
    }

    /// Restore puts no file back into RABBIT's own folder, the receipt
    /// included, whether a set records it relative or absolute, and other
    /// files of the resource folder still go back.
    #[test]
    fn restore_puts_no_file_back_into_rabbit_s_folder_the_receipt_included() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let absolute = super::absolute_without_dots(&resource).unwrap();
        let refused = [
            PathBuf::from("RABBIT/install-state.json"),
            absolute.join("RABBIT").join("install-state.json"),
            PathBuf::from("RABBIT/logs/report.json"),
        ];
        assert_refused_as_rabbit_s_folder(&resource, &refused);

        assert_eq!(
            restore_target(
                &resource,
                Path::new("reaper-kb.ini"),
                None,
                None,
                &OutsideRestoreRules::default()
            )
            .unwrap(),
            resource.join("reaper-kb.ini")
        );
        assert!(is_receipt(Path::new("RABBIT/install-state.json")));
        for other in [
            "RABBIT/install-state.json.rabbit-tmp",
            "RABBIT/logs/install-state.json",
            "install-state.json",
            "UserPlugins/RABBIT/install-state.json",
        ] {
            assert!(!is_receipt(Path::new(other)), "{other}");
        }
    }

    /// Check that `restore_target` refuses each of `paths` as a path into
    /// RABBIT's own folder.
    fn assert_refused_as_rabbit_s_folder(resource: &Path, paths: &[PathBuf]) {
        for path in paths {
            let error = restore_target(resource, path, None, None, &OutsideRestoreRules::default())
                .unwrap_err();
            assert!(
                matches!(refusal(&error), RestoreRefusal::TargetRefused {
                    path: refused,
                    reason: TargetRefusal::RabbitFolder,
                    ..
                } if refused == path),
                "{path:?}: {error:?}"
            );
        }
    }

    /// Windows and macOS usually ignore upper and lower case, so a path
    /// into `rabbit` is a path into RABBIT's folder there, and a copy of
    /// `rabbit/INSTALL-STATE.JSON` is a copy of the receipt, in a set with
    /// a manifest and in one without.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn a_receipt_in_another_case_is_still_the_receipt() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let receipt = resource.join("RABBIT/install-state.json");
        write(&receipt, b"records");
        let absolute = super::absolute_without_dots(&resource).unwrap();
        assert_refused_as_rabbit_s_folder(
            &resource,
            &[
                PathBuf::from("rabbit/INSTALL-STATE.JSON"),
                absolute.join("Rabbit").join("Install-State.json"),
            ],
        );
        assert!(is_receipt(Path::new("rabbit/INSTALL-STATE.JSON")));

        let layout_set = resource.join("RABBIT/backups/unix-1");
        write(
            &layout_set.join("rabbit/INSTALL-STATE.JSON"),
            b"old records",
        );
        let error = refused_with(dir.path(), &resource, "unix-1", None);
        assert!(
            matches!(refusal(&error), RestoreRefusal::ReceiptInSet { .. }),
            "{error:?}"
        );

        let manifest_set = resource.join("RABBIT/backups/unix-2");
        let copy = manifest_set.join("rabbit/INSTALL-STATE.JSON");
        write(&copy, b"old records");
        save_manifest(
            &manifest_set,
            BEFORE_RESTORE_REASON,
            vec![legacy_entry(
                None,
                Path::new("rabbit/INSTALL-STATE.JSON"),
                &copy,
            )],
        );
        let error = refused_with(dir.path(), &resource, "unix-2", None);
        assert!(
            matches!(refusal(&error), RestoreRefusal::ReceiptInSet { .. }),
            "{error:?}"
        );
        assert_eq!(fs::read(&receipt).unwrap(), b"records");
    }

    /// Windows also opens a name without its trailing dots and spaces, and
    /// takes what follows a `:` as a stream of the same file or folder, so
    /// those spellings lead into RABBIT's folder too. So does an absolute
    /// path to the receipt whose resource folder is in another case.
    #[cfg(windows)]
    #[test]
    fn windows_refuses_every_spelling_it_opens_as_the_receipt() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let receipt = resource.join("RABBIT/install-state.json");
        write(&receipt, b"records");
        let absolute = super::absolute_without_dots(&resource).unwrap();
        let shouting = PathBuf::from(absolute.to_string_lossy().to_uppercase())
            .join("RABBIT")
            .join("INSTALL-STATE.JSON");
        let spellings = [
            "RABBIT./install-state.json",
            "RABBIT /install-state.json",
            "RABBIT::$INDEX_ALLOCATION/install-state.json",
            "RABBIT/install-state.json::$DATA",
            "RABBIT/install-state.json. .",
        ]
        .map(PathBuf::from);
        assert_refused_as_rabbit_s_folder(&resource, &spellings);
        assert_refused_as_rabbit_s_folder(&resource, std::slice::from_ref(&shouting));
        for spelling in &spellings {
            assert!(is_receipt(spelling), "{spelling:?}");
        }

        let set = resource.join("RABBIT/backups/unix-1");
        let copy = set.join("RABBIT/install-state.json");
        write(&copy, b"old records");
        for original in [shouting, spellings[0].clone()] {
            save_manifest(
                &set,
                BEFORE_RESTORE_REASON,
                vec![legacy_entry(None, &original, &copy)],
            );
            let error = refused_with(dir.path(), &resource, "unix-1", None);
            assert!(
                matches!(refusal(&error), RestoreRefusal::ReceiptInSet { .. }),
                "{original:?}: {error:?}"
            );
        }
        assert_eq!(fs::read(&receipt).unwrap(), b"records");
    }

    /// The writer's layout: files inside the resource folder keep their
    /// relative path, files outside it get a numbered folder under
    /// `external/` and a note of where they came from, and every entry ends
    /// up done.
    #[test]
    fn the_writer_puts_outside_files_under_external_with_an_origin_note() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let inside = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        let outside = dir.path().join("CLAP").join("app2clap.clap");
        let other_outside = dir.path().join("Other").join("app2clap.clap");
        write(&inside, b"reapack");
        write(&outside, b"app2clap");
        write(&other_outside, b"another app2clap");

        let mut writer = BackupSetWriter::new(&resource, "unix-1", "install-replacement");
        let inside_backup = writer.copy_file(Some("reapack"), &inside).unwrap();
        let outside_backup = writer.copy_file(Some("app2clap"), &outside).unwrap();
        let other_backup = writer.copy_file(Some("app2clap"), &other_outside).unwrap();

        let set = resource.join("RABBIT/backups/unix-1");
        assert_eq!(writer.path(), set);
        assert_eq!(
            inside_backup,
            set.join("UserPlugins").join("reaper_reapack-x64.dll")
        );
        assert_eq!(
            outside_backup,
            set.join("external").join("1").join("app2clap.clap")
        );
        assert_eq!(
            other_backup,
            set.join("external").join("2").join("app2clap.clap")
        );
        assert_eq!(fs::read(&outside_backup).unwrap(), b"app2clap");
        assert_eq!(
            fs::read_to_string(set.join("external/1/app2clap.clap.rabbit-origin")).unwrap(),
            std::path::absolute(&outside).unwrap().to_string_lossy()
        );

        let manifest = load_backup_manifest(&set).unwrap().unwrap();
        assert_eq!(manifest.reason, "install-replacement");
        assert_eq!(
            manifest.files[0].original_path,
            Path::new("UserPlugins").join("reaper_reapack-x64.dll")
        );
        assert_eq!(
            manifest.files[1].original_path,
            std::path::absolute(&outside).unwrap()
        );
        assert!(
            manifest
                .files
                .iter()
                .all(|file| file.state == BackupEntryState::Done)
        );
        // The origin notes are not backed-up files, so the listing leaves
        // them out.
        let listed = &list_backup_sets(&resource).unwrap()[0].files;
        assert_eq!(listed.len(), 3, "{listed:?}");
        assert!(
            listed
                .iter()
                .all(|file| !file.to_string_lossy().ends_with(".rabbit-origin"))
        );
    }

    /// An outside file's number is one above the highest in use, not the
    /// count of outside entries. After an earlier file of a failed package
    /// goes back while a later one stays, the earlier entry is gone, and
    /// counting would give the next outside file the number the later one
    /// still holds: with the same name, the move would be refused. A number
    /// on disk without an entry, as a copy that couldn't be removed leaves,
    /// counts too.
    #[test]
    fn an_outside_file_after_a_put_back_gets_a_fresh_number() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let outside: Vec<PathBuf> = ["one", "two", "three", "four"]
            .iter()
            .map(|folder| dir.path().join(folder).join("app2clap.clap"))
            .collect();
        for (path, contents) in outside.iter().zip(["1", "2", "3", "4"]) {
            write(path, contents.as_bytes());
        }

        let mut writer = BackupSetWriter::new(&resource, "uninstall-unix-1", "uninstall");
        let first = writer
            .move_file(Some("app2clap"), &outside[0], None, false)
            .unwrap();
        let second = writer
            .move_file(Some("app2clap"), &outside[1], None, false)
            .unwrap();
        let external = writer.path().join("external");
        assert_eq!(first.backup_path, external.join("1").join("app2clap.clap"));
        assert_eq!(second.backup_path, external.join("2").join("app2clap.clap"));
        writer.put_back(&first).unwrap();
        assert_eq!(fs::read(&outside[0]).unwrap(), b"1");
        assert!(!external.join("1").exists());

        let third = writer
            .move_file(Some("app2clap"), &outside[2], None, false)
            .unwrap();
        assert_eq!(third.backup_path, external.join("3").join("app2clap.clap"));
        assert!(!outside[2].exists());
        assert_eq!(fs::read(&third.backup_path).unwrap(), b"3");
        assert_eq!(fs::read(&second.backup_path).unwrap(), b"2");
        let note = |number: &str| {
            fs::read_to_string(external.join(number).join("app2clap.clap.rabbit-origin")).unwrap()
        };
        let absolute = |path: &Path| std::path::absolute(path).unwrap();
        assert_eq!(note("2"), absolute(&outside[1]).to_string_lossy());
        assert_eq!(note("3"), absolute(&outside[2]).to_string_lossy());

        fs::create_dir_all(external.join("7")).unwrap();
        let fourth = writer.copy_file(Some("app2clap"), &outside[3]).unwrap();
        assert_eq!(fourth, external.join("8").join("app2clap.clap"));
        assert_eq!(fs::read(&fourth).unwrap(), b"4");

        let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
        let copies: Vec<&Path> = manifest
            .files
            .iter()
            .map(|entry| entry.backup_path.as_path())
            .collect();
        assert_eq!(
            copies,
            [
                second.backup_path.as_path(),
                third.backup_path.as_path(),
                fourth.as_path()
            ]
        );
    }

    /// A planning writer numbers outside files from its own manifest alone:
    /// whatever is at its path on disk belongs to another set. So its
    /// second outside file gets 2, even with an `external/5` there, and it
    /// writes nothing.
    #[test]
    fn a_planning_writer_numbers_outside_files_from_its_manifest() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let first = dir.path().join("one").join("app2clap.clap");
        let second = dir.path().join("two").join("app2clap.clap");
        write(&first, b"1");
        write(&second, b"2");
        let external = resource
            .join("RABBIT")
            .join("backups")
            .join("unix-1")
            .join("external");
        fs::create_dir_all(external.join("5")).unwrap();

        let mut writer = BackupSetWriter::planning(&resource, "unix-1", RESTORABLE_REASON);
        assert_eq!(writer.path(), external.parent().unwrap());
        assert_eq!(
            writer.copy_file(None, &first).unwrap(),
            external.join("1").join("app2clap.clap")
        );
        assert_eq!(
            writer.copy_file(None, &second).unwrap(),
            external.join("2").join("app2clap.clap")
        );
        assert!(!external.join("1").exists() && !external.join("2").exists());
    }

    /// A folder under `external/` with the highest number there is refuses
    /// the next outside file, rather than wrapping round to a number that
    /// may be in use. The file stays where it is, and the set records only
    /// what it held before.
    #[test]
    fn an_outside_file_after_the_highest_number_there_is_is_refused() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let inside = resource.join("UserPlugins").join("reaper_demo.dll");
        let outside = dir.path().join("CLAP").join("app2clap.clap");
        write(&inside, b"demo");
        write(&outside, b"app2clap");

        let mut writer = BackupSetWriter::new(&resource, "unix-1", RESTORABLE_REASON);
        writer.copy_file(Some("demo"), &inside).unwrap();
        let external = writer.path().join("external");
        fs::create_dir_all(external.join(u64::MAX.to_string())).unwrap();

        let error = writer.copy_file(Some("app2clap"), &outside).unwrap_err();
        assert!(
            matches!(error, RabbitError::BackupSetWriteRefused { .. }),
            "{error}"
        );
        let error = writer
            .move_file(Some("app2clap"), &outside, None, false)
            .unwrap_err();
        assert!(
            matches!(
                error,
                super::MoveFileError::NotMoved(RabbitError::BackupSetWriteRefused { .. })
            ),
            "{error:?}"
        );
        assert_eq!(fs::read(&outside).unwrap(), b"app2clap");
        let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
        assert_eq!(manifest.files.len(), 1);
    }

    /// A set to restore that replaces one file: `unix-1`, whose copy of
    /// `UserPlugins/reaper_demo.dll` says "backup", while the file in the
    /// resource folder says "current". Returns the resource folder and that
    /// file.
    fn set_replacing_one_file(dir: &Path) -> (PathBuf, PathBuf) {
        let resource = dir.join("REAPER");
        let target = resource.join("UserPlugins").join("reaper_demo.dll");
        write(&target, b"current");
        let set = resource.join("RABBIT").join("backups").join("unix-1");
        let copy = set.join("UserPlugins").join("reaper_demo.dll");
        write(&copy, b"backup");
        save_manifest(
            &set,
            RESTORABLE_REASON,
            vec![legacy_entry(
                None,
                &Path::new("UserPlugins").join("reaper_demo.dll"),
                &copy,
            )],
        );
        (resource, target)
    }

    /// The report names the set a real restore wrote with the files it
    /// replaced. A set of exactly that name is in `RABBIT/backups`, and it
    /// holds the replaced file as it was. A dry run names none, and so does
    /// a restore that replaced nothing, which writes no set.
    #[test]
    fn the_report_names_the_backup_of_what_the_restore_replaced() {
        let dir = tempdir().unwrap();
        let (resource, target) = set_replacing_one_file(dir.path());
        let backups = resource.join("RABBIT").join("backups");
        let sets = || -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&backups)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .collect();
            names.sort();
            names
        };
        let restore = |dry_run: bool| {
            restore_backup_set_within(
                &resource,
                "unix-1",
                &RestoreBackupOptions { dry_run, ..APPLY },
                &OutsideRestoreRules::default(),
            )
            .unwrap()
        };

        assert_eq!(restore(true).before_restore_set_id, None);
        assert_eq!(sets(), ["unix-1"]);

        let report = restore(false);
        let set_id = report
            .before_restore_set_id
            .clone()
            .expect("the backup of what the restore replaced");
        assert!(
            set_id.starts_with("restore-unix-") && set_id.contains("-before-restore"),
            "{set_id}"
        );
        assert_eq!(sets(), [set_id.clone(), "unix-1".to_string()]);
        assert_eq!(fs::read(&target).unwrap(), b"backup");
        let replaced = backups
            .join(&set_id)
            .join("UserPlugins")
            .join("reaper_demo.dll");
        assert_eq!(fs::read(&replaced).unwrap(), b"current");
        let manifest = load_backup_manifest(&backups.join(&set_id))
            .unwrap()
            .unwrap();
        assert_eq!(manifest.reason, BEFORE_RESTORE_REASON);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["before_restore_set_id"], set_id.as_str());

        // The file isn't there now, so a restore puts it back and replaces
        // nothing.
        fs::remove_file(&target).unwrap();
        let report = restore(false);
        assert_eq!(report.before_restore_set_id, None);
        assert_eq!(fs::read(&target).unwrap(), b"backup");
        assert_eq!(sets(), [set_id, "unix-1".to_string()]);
        let json = serde_json::to_value(&report).unwrap();
        assert!(json.get("before_restore_set_id").is_none(), "{json}");
    }

    /// Every real restore saves its report as
    /// `RABBIT/logs/restore-backup-unix-<time>`, `.json` and `.txt`, as a
    /// real uninstall does, and the saved report names the backup of what
    /// the restore replaced. A dry run saves nothing.
    #[test]
    fn a_real_restore_saves_its_report_and_a_dry_run_doesnt() {
        let dir = tempdir().unwrap();
        let (resource, _) = set_replacing_one_file(dir.path());
        let logs = resource.join("RABBIT").join("logs");
        let restore = |dry_run: bool| {
            restore_backup_set_within(
                &resource,
                "unix-1",
                &RestoreBackupOptions { dry_run, ..APPLY },
                &OutsideRestoreRules::default(),
            )
            .unwrap()
        };

        let preview = restore(true);
        assert_eq!(preview.saved_report, None);
        assert!(!logs.exists());

        let report = restore(false);
        let saved = report.saved_report.clone().expect("a saved report");
        let mut logged: Vec<PathBuf> = fs::read_dir(&logs)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        logged.sort();
        assert_eq!(logged, [saved.json_path.clone(), saved.text_path.clone()]);
        let name = saved.json_path.file_name().unwrap().to_string_lossy();
        assert!(
            name.starts_with("restore-backup-unix-") && name.ends_with(".json"),
            "{name}"
        );
        let envelope: crate::report::SavedJsonReport =
            serde_json::from_str(&fs::read_to_string(&saved.json_path).unwrap()).unwrap();
        let set_id = report.before_restore_set_id.clone().unwrap();
        assert_eq!(envelope.report["before_restore_set_id"], set_id.as_str());
        assert_eq!(envelope.report["backup_id"], "unix-1");
        let text = fs::read_to_string(&saved.text_path).unwrap();
        assert!(text.contains(&set_id), "{text}");

        // A second restore, as soon after as a script runs it, saves a
        // report of its own, and the first stays as it was.
        let first = fs::read(&saved.json_path).unwrap();
        let again = restore(false);
        let saved_again = again.saved_report.clone().expect("a second saved report");
        assert_ne!(saved_again.json_path, saved.json_path);
        assert_eq!(fs::read(&saved.json_path).unwrap(), first);
        assert_eq!(fs::read_dir(&logs).unwrap().count(), 4);
    }

    /// The report names the folder the before-restore set went into, which
    /// is `<id>-2` when a set of the name it asked for is already there, as
    /// after another restore in the same second. That folder holds the
    /// replaced file, and the sets already there stay as they were.
    #[test]
    fn the_report_names_the_before_restore_set_by_its_folder() {
        let dir = tempdir().unwrap();
        let (resource, _) = set_replacing_one_file(dir.path());
        let backups = resource.join("RABBIT").join("backups");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let taken: Vec<String> = (now..=now + 5)
            .map(|seconds| format!("restore-unix-{seconds}-before-restore"))
            .collect();
        for name in &taken {
            fs::create_dir(backups.join(name)).unwrap();
        }

        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        let set_id = report.before_restore_set_id.clone().unwrap();
        let asked_for = set_id.strip_suffix("-2").expect("a `-2` set");
        assert!(taken.iter().any(|name| name == asked_for), "{set_id}");
        assert_eq!(
            fs::read(
                backups
                    .join(&set_id)
                    .join("UserPlugins")
                    .join("reaper_demo.dll")
            )
            .unwrap(),
            b"current"
        );
        for name in &taken {
            assert_eq!(
                fs::read_dir(backups.join(name)).unwrap().count(),
                0,
                "{name}"
            );
        }
    }

    /// The regression test for the restore bug: a file from outside the
    /// resource folder, app2clap's CLAP folder, goes back to that folder and
    /// not into a new `CLAP` or `external` folder inside the resource folder.
    /// The file the restore replaced lands in a set that can itself be
    /// restored.
    #[test]
    fn restores_a_file_from_outside_the_resource_folder_where_it_came_from() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap_dir = dir.path().join("CLAP");
        let app2clap = clap_dir.join("app2clap.clap");
        write(&app2clap, b"old app2clap");
        fs::create_dir_all(&resource).unwrap();
        let mut writer = BackupSetWriter::new(&resource, "unix-1", RESTORABLE_REASON);
        writer.copy_file(Some("app2clap"), &app2clap).unwrap();
        fs::write(&app2clap, b"new app2clap").unwrap();

        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &shared(&clap_dir)).unwrap();

        assert_eq!(fs::read(&app2clap).unwrap(), b"old app2clap");
        assert!(!resource.join("CLAP").exists());
        assert!(!resource.join("external").exists());
        assert_eq!(report.actions.len(), 1);
        assert_eq!(report.actions[0].target_path, app2clap);
        assert_eq!(report.actions[0].package_id.as_deref(), Some("app2clap"));
        let replaced = report.actions[0].current_backup_path.clone().unwrap();
        assert_eq!(fs::read(&replaced).unwrap(), b"new app2clap");

        // Undo the restore through the set it wrote.
        let before_restore_id = replaced
            .strip_prefix(resource.join("RABBIT/backups"))
            .unwrap()
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy()
            .into_owned();
        restore_backup_set_within(&resource, &before_restore_id, &APPLY, &shared(&clap_dir))
            .unwrap();
        assert_eq!(fs::read(&app2clap).unwrap(), b"new app2clap");
    }

    /// Sets written by earlier versions store app2clap's copy under a
    /// made-up `CLAP/` folder with its absolute original path, as the
    /// update set `unix-1782841261` on the maintainer's machine does.
    /// Restore follows the original path, not the copy's place in the set.
    /// This is tested on a before-restore set: an update set with these
    /// paths is refused until earlier versions can come back (see
    /// `RESTORABLE_REASON`).
    #[test]
    fn restores_an_older_app2clap_backup_to_its_absolute_original_path() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap_dir = dir.path().join("Local/Programs/Common/CLAP");
        let app2clap = clap_dir.join("app2clap.clap");
        write(&app2clap, b"new app2clap");
        let set = resource.join("RABBIT/backups/unix-1782841261");
        let copy = set.join("CLAP").join("app2clap.clap");
        write(&copy, b"old app2clap");
        save_manifest(
            &set,
            RESTORABLE_REASON,
            vec![legacy_entry(Some("app2clap"), &app2clap, &copy)],
        );

        restore_backup_set_within(&resource, "unix-1782841261", &APPLY, &shared(&clap_dir))
            .unwrap();

        assert_eq!(fs::read(&app2clap).unwrap(), b"old app2clap");
        assert!(!resource.join("CLAP").exists());
    }

    /// Unattended sets record a path relative to the resource folder, and
    /// older versions' update sets an absolute one. Every set a restore
    /// takes reads both forms the same way, and both go back to the same
    /// place. Update sets themselves are refused for now (see
    /// `RESTORABLE_REASON`).
    #[test]
    fn restores_relative_and_absolute_original_paths() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        let plugin = resource.join("UserPlugins").join("reaper_kontrol.dll");
        write(&keymap, b"osara keymap");
        write(&plugin, b"new kontrol");
        let set = resource.join("RABBIT/backups/unix-1");
        let keymap_copy = set.join("reaper-kb.ini");
        let plugin_copy = set.join("UserPlugins").join("reaper_kontrol.dll");
        write(&keymap_copy, b"own keymap");
        write(&plugin_copy, b"old kontrol");
        save_manifest(
            &set,
            RESTORABLE_REASON,
            vec![
                legacy_entry(Some("osara"), Path::new("reaper-kb.ini"), &keymap_copy),
                legacy_entry(Some("reakontrol"), &plugin, &plugin_copy),
            ],
        );

        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert_eq!(fs::read(&plugin).unwrap(), b"old kontrol");
        assert_eq!(report.actions[0].target_path, keymap);
        assert_eq!(report.actions[1].target_path, plugin);
    }

    /// A portable REAPER can move, and its sets' manifests still name the
    /// old folder. A file that was inside the old folder goes to the same
    /// place inside the new one, and the copies are found in the set where
    /// it is now.
    #[test]
    fn a_moved_resource_folder_still_restores_its_sets() {
        let dir = tempdir().unwrap();
        let old_resource = std::path::absolute(dir.path().join("OldREAPER")).unwrap();
        let set = old_resource.join("RABBIT/backups/unix-1");
        let keymap_copy = set.join("reaper-kb.ini");
        let plugin_copy = set.join("UserPlugins").join("reaper_kontrol.dll");
        write(&keymap_copy, b"own keymap");
        write(&plugin_copy, b"old kontrol");
        save_manifest(
            &set,
            RESTORABLE_REASON,
            vec![
                legacy_entry(Some("osara"), Path::new("reaper-kb.ini"), &keymap_copy),
                legacy_entry(
                    Some("reakontrol"),
                    &old_resource.join("UserPlugins").join("reaper_kontrol.dll"),
                    &plugin_copy,
                ),
            ],
        );
        let resource = dir.path().join("NewREAPER");
        fs::rename(&old_resource, &resource).unwrap();

        restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
            .unwrap();

        assert_eq!(
            fs::read(resource.join("reaper-kb.ini")).unwrap(),
            b"own keymap"
        );
        assert_eq!(
            fs::read(resource.join("UserPlugins").join("reaper_kontrol.dll")).unwrap(),
            b"old kontrol"
        );
        assert!(!old_resource.exists());
    }

    /// A temporary folder named by a path relative to the current folder, the
    /// way `--resource-path .` or `--resource-path REAPER` names one. Tests
    /// run in the package's own folder, so it sits in the workspace's
    /// `target` folder, reached through `..`. Returns the folder, which
    /// removes itself when dropped, and its relative path.
    fn relative_tempdir() -> (tempfile::TempDir, PathBuf) {
        let base = Path::new("..").join("..").join("target");
        fs::create_dir_all(&base).unwrap();
        let dir = tempfile::tempdir_in(&base).unwrap();
        let relative = base.join(dir.path().file_name().unwrap());
        assert!(relative.is_relative() && relative.is_dir(), "{relative:?}");
        (dir, relative)
    }

    /// A set written through a relative resource path records where things
    /// are without depending on the folder RABBIT ran in, so it restores
    /// through that relative path and through the absolute one alike, and
    /// so does the set the restore itself writes.
    #[test]
    fn a_set_written_with_a_relative_resource_path_restores_from_anywhere() {
        let (_dir, base) = relative_tempdir();
        let resource = base.join("REAPER");
        let plugin = resource.join("UserPlugins").join("reaper_demo.dll");
        write(&plugin, b"old plugin");

        let mut writer = BackupSetWriter::new(&resource, "unix-1", RESTORABLE_REASON);
        writer.copy_file(Some("demo"), &plugin).unwrap();
        assert!(writer.path().is_absolute(), "{:?}", writer.path());
        let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
        assert!(manifest.files[0].backup_path.is_absolute());
        assert_eq!(
            manifest.files[0].original_path,
            Path::new("UserPlugins").join("reaper_demo.dll")
        );
        fs::write(&plugin, b"new plugin").unwrap();

        restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
            .unwrap();
        assert_eq!(fs::read(&plugin).unwrap(), b"old plugin");

        // Undo it through the absolute path, from the set the restore wrote.
        let absolute_resource = super::absolute_without_dots(&resource).unwrap();
        let undo = list_backup_sets(&resource)
            .unwrap()
            .into_iter()
            .find(|set| set.id != "unix-1" && set.reason.as_deref() == Some(BEFORE_RESTORE_REASON))
            .expect("the restore wrote a before-restore set");
        restore_backup_set_within(
            &absolute_resource,
            &undo.id,
            &APPLY,
            &OutsideRestoreRules::default(),
        )
        .unwrap();
        assert_eq!(fs::read(&plugin).unwrap(), b"new plugin");

        restore_backup_set_within(
            &absolute_resource,
            "unix-1",
            &APPLY,
            &OutsideRestoreRules::default(),
        )
        .unwrap();
        assert_eq!(fs::read(&plugin).unwrap(), b"old plugin");
    }

    /// Earlier versions given a relative resource path recorded relative
    /// paths: every copy as `<that path>/RABBIT/backups/<id>/...`, and an
    /// install replacement's original as `<that path>/UserPlugins/...`,
    /// while the unattended key map backup recorded its original inside the
    /// resource folder. Such sets are on users' machines. Every set a
    /// restore takes reads these paths the same way, and restores again, to
    /// the right place, through a relative or an absolute path. Update sets
    /// that hold them are refused for now (see `RESTORABLE_REASON`), so a
    /// before-restore set stands in for one here.
    #[test]
    fn older_sets_with_relative_paths_restore_again() {
        let (_dir, base) = relative_tempdir();
        let resource = base.join("REAPER");
        let absolute_resource = super::absolute_without_dots(&resource).unwrap();
        let keymap = resource.join("reaper-kb.ini");
        let plugin_inside = Path::new("UserPlugins").join("reaper_demo.dll");
        let plugin = resource.join(&plugin_inside);

        let write_old_set = |id: &str, recorded_resource: &Path| {
            let set = resource.join("RABBIT/backups").join(id);
            write(&set.join("reaper-kb.ini"), b"own keymap");
            write(&set.join(&plugin_inside), b"old plugin");
            let recorded_set = recorded_resource.join("RABBIT").join("backups").join(id);
            let entry = |original: PathBuf, inside: &Path| BackupManifestFile {
                package_id: None,
                original_path: original,
                backup_path: recorded_set.join(inside),
                size: fs::metadata(set.join(inside)).unwrap().len(),
                sha256: sha256_file(&set.join(inside)).unwrap(),
                state: BackupEntryState::Done,
            };
            save_manifest(
                &set,
                RESTORABLE_REASON,
                vec![
                    entry(PathBuf::from("reaper-kb.ini"), Path::new("reaper-kb.ini")),
                    entry(recorded_resource.join(&plugin_inside), &plugin_inside),
                ],
            );
        };

        for (id, recorded_resource, restore_through) in [
            ("unix-1", resource.clone(), resource.clone()),
            ("unix-2", resource.clone(), absolute_resource.clone()),
            ("unix-3", PathBuf::from("."), resource.clone()),
            ("unix-4", PathBuf::from("REAPER"), absolute_resource.clone()),
            ("unix-5", Path::new("..").join("REAPER"), resource.clone()),
        ] {
            write(&keymap, b"osara keymap");
            write(&plugin, b"new plugin");
            write_old_set(id, &recorded_resource);

            let report = restore_backup_set_within(
                &restore_through,
                id,
                &APPLY,
                &OutsideRestoreRules::default(),
            )
            .unwrap_or_else(|error| panic!("{id}: {error}"));

            assert_eq!(fs::read(&keymap).unwrap(), b"own keymap", "{id}");
            assert_eq!(fs::read(&plugin).unwrap(), b"old plugin", "{id}");
            assert_eq!(
                report.actions[1].target_path,
                restore_through.join(&plugin_inside),
                "{id}"
            );
        }
        // Nothing went to the resource path joined onto itself.
        assert!(!resource.join("REAPER").exists());
    }

    #[test]
    fn absolute_paths_lose_their_dots() {
        let current = std::env::current_dir().unwrap();
        let dotted = Path::new("a").join("..").join("b").join(".").join("c");
        assert_eq!(
            super::absolute_without_dots(&dotted).unwrap(),
            current.join("b").join("c")
        );
        assert_eq!(
            super::absolute_without_dots(&current).unwrap(),
            std::path::absolute(&current).unwrap()
        );
    }

    /// Without a manifest the set's layout mirrors the resource folder,
    /// which says nothing about where an outside file belongs, so such a
    /// set is refused as a whole before any file is touched.
    #[test]
    fn a_set_without_a_manifest_refuses_files_from_outside_the_resource_folder() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        write(&plugin, b"new");
        let set = resource.join("RABBIT/backups/unix-1");
        write(
            &set.join("UserPlugins").join("reaper_reapack-x64.dll"),
            b"old",
        );
        write(&set.join("external/1/app2clap.clap"), b"app2clap");

        let error =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
                .unwrap_err();

        assert!(
            matches!(
                refusal(&error),
                RestoreRefusal::TargetRefused {
                    reason: TargetRefusal::OutsideWithoutManifest,
                    ..
                }
            ),
            "{error:?}"
        );
        assert_eq!(fs::read(&plugin).unwrap(), b"new");
        assert!(!resource.join("external").exists());
    }

    /// A manifest can only send files to the resource folder (but not
    /// RABBIT's own folder) and the outside folders RABBIT installs into.
    /// Anything else stops the restore before the first file is replaced,
    /// each for its own reason. Two reasons have no test of their own:
    /// `leads-outside` needs a folder whose real path leaves the guard's
    /// with no link on the way, and `could-not-check` a folder whose
    /// metadata can't be read, neither of which a temporary folder gives
    /// on every platform.
    #[test]
    fn refuses_targets_outside_known_roots_before_changing_anything() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap_dir = dir.path().join("CLAP");
        let plugin = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        write(&plugin, b"new");
        let set = resource.join("RABBIT/backups/unix-1");
        let plugin_copy = set.join("UserPlugins").join("reaper_reapack-x64.dll");
        write(&plugin_copy, b"old");
        let stray_copy = set.join("external/1/stray.dll");
        write(&stray_copy, b"stray");

        for (refused, reason) in [
            (
                std::path::absolute(dir.path().join("Elsewhere").join("stray.dll")).unwrap(),
                "outside-allowed-folders",
            ),
            (PathBuf::from("../stray.dll"), "not-plain-path"),
            (
                PathBuf::from("RABBIT/backups/unix-2/stray.dll"),
                "rabbit-folder",
            ),
            (
                PathBuf::from("RABBIT/locks/package-install.lock"),
                "rabbit-folder",
            ),
            (
                std::path::absolute(&clap_dir).unwrap(),
                "outside-allowed-folders",
            ),
        ] {
            save_manifest(
                &set,
                RESTORABLE_REASON,
                vec![
                    legacy_entry(Some("reapack"), &plugin, &plugin_copy),
                    legacy_entry(None, &refused, &stray_copy),
                ],
            );

            let error = restore_backup_set_within(&resource, "unix-1", &APPLY, &shared(&clap_dir))
                .unwrap_err();

            assert!(
                matches!(refusal(&error), RestoreRefusal::TargetRefused { path, reason: found, .. }
                    if *path == refused && found.code() == reason),
                "{refused:?}: {error:?}"
            );
            assert_eq!(fs::read(&plugin).unwrap(), b"new", "{refused:?}");
        }

        // A manifest that names a file's copy outside the set is refused
        // too, by that copy's recorded path.
        let elsewhere = dir.path().join("Elsewhere").join("copy.dll");
        write(&elsewhere, b"stray");
        save_manifest(
            &set,
            RESTORABLE_REASON,
            vec![
                legacy_entry(Some("reapack"), &plugin, &plugin_copy),
                legacy_entry(None, Path::new("UserPlugins/stray.dll"), &elsewhere),
            ],
        );
        let error =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &shared(&clap_dir)).unwrap_err();
        assert!(
            matches!(refusal(&error), RestoreRefusal::TargetRefused {
                path,
                reason: TargetRefusal::CopyOutsideSet,
                ..
            } if *path == elsewhere),
            "{error:?}"
        );
        assert_eq!(fs::read(&plugin).unwrap(), b"new");
        fs::remove_dir_all(dir.path().join("Elsewhere")).unwrap();
        assert!(!dir.path().join("Elsewhere").exists());
        assert!(!resource.join("RABBIT/locks/package-install.lock").exists());
    }

    /// The package `scripts` may put files back where the JAWS scripts'
    /// kind of pattern, `{appdata}/Freedom Scientific/JAWS/*/Settings/*/*`,
    /// matches below `appdata`, a temporary folder standing in for
    /// `%APPDATA%`. No other package may, and nothing else outside the
    /// resource folder is allowed.
    fn scripts_rules(appdata: &Path) -> OutsideRestoreRules {
        let roots = UninstallRoots {
            appdata: Some(appdata.to_path_buf()),
            ..UninstallRoots::new(&appdata.join("REAPER"))
        };
        let patterns = resolve_pattern(
            &UninstallPath::new("{appdata}/Freedom Scientific/JAWS/*/Settings/*/*"),
            &roots,
        )
        .unwrap();
        OutsideRestoreRules {
            shared_roots: Vec::new(),
            vendor_added: patterns
                .into_iter()
                .map(|pattern| ("scripts".to_string(), pattern))
                .collect(),
        }
    }

    /// `name` in JAWS 2025's English settings folder below `appdata`.
    fn script_file(appdata: &Path, name: &str) -> PathBuf {
        appdata
            .join("Freedom Scientific")
            .join("JAWS")
            .join("2025")
            .join("Settings")
            .join("enu")
            .join(name)
    }

    /// A set holding a copy of `path` as `owner`'s file, which is then gone
    /// from its place, as after an uninstall.
    fn backed_up_and_gone(resource: &Path, set_id: &str, owner: Option<&str>, path: &Path) {
        write(path, b"script");
        let mut writer = BackupSetWriter::new(resource, set_id, "uninstall");
        writer.copy_file(owner, path).unwrap();
        fs::remove_file(path).unwrap();
    }

    /// A package's pattern lets only that package's files back: the scripts'
    /// file comes back where it was, while the same place refuses another
    /// package's file and a file of no package, before anything changes.
    #[test]
    fn a_vendor_pattern_lets_only_its_own_package_restore_there() {
        let dir = tempdir().unwrap();
        let appdata = dir.path().to_path_buf();
        let resource = appdata.join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let rules = scripts_rules(&appdata);
        let script = script_file(&appdata, "reaper.jss");

        for (set_id, owner) in [("unix-1", Some("other")), ("unix-2", None)] {
            backed_up_and_gone(&resource, set_id, owner, &script);
            let error = restore_backup_set_within(&resource, set_id, &APPLY, &rules).unwrap_err();
            assert!(
                matches!(
                    refusal(&error),
                    RestoreRefusal::TargetRefused {
                        reason: TargetRefusal::OutsideAllowedFolders,
                        ..
                    }
                ),
                "{owner:?}: {error:?}"
            );
            assert!(!script.exists(), "{owner:?}");
        }

        backed_up_and_gone(&resource, "unix-3", Some("scripts"), &script);
        let report = restore_backup_set_within(&resource, "unix-3", &APPLY, &rules).unwrap();
        assert_eq!(fs::read(&script).unwrap(), b"script");
        assert_eq!(report.actions.len(), 1);
        assert_eq!(report.actions[0].target_path, script);
        assert!(!resource.join("external").exists());
    }

    /// A pattern opens only the places it matches, for its own package, and
    /// only while the folder its guard names is there: anything else
    /// outside the resource folder is still refused, before anything
    /// changes.
    #[test]
    fn restore_still_refuses_paths_outside_every_known_folder() {
        let dir = tempdir().unwrap();
        let appdata = dir.path().join("AppData");
        let resource = appdata.join("REAPER");
        let plugin = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        write(&plugin, b"new");
        let set = resource.join("RABBIT/backups/unix-1");
        let plugin_copy = set.join("UserPlugins").join("reaper_reapack-x64.dll");
        write(&plugin_copy, b"old");
        let stray_copy = set.join("external/1/stray.jss");
        write(&stray_copy, b"stray");
        let jaws = appdata.join("Freedom Scientific").join("JAWS");
        fs::create_dir_all(jaws.join("2025").join("Settings").join("enu")).unwrap();
        let rules = scripts_rules(&appdata);

        let refused = [
            // One folder too shallow, and one too deep.
            jaws.join("2025").join("Settings").join("stray.jss"),
            script_file(&appdata, "x").join("stray.jss"),
            // Another vendor folder, and somewhere else altogether.
            appdata
                .join("Freedom Scientific")
                .join("Other")
                .join("stray.jss"),
            dir.path().join("Elsewhere").join("stray.jss"),
        ];
        for refused in refused {
            save_manifest(
                &set,
                "uninstall",
                vec![
                    legacy_entry(Some("reapack"), &plugin, &plugin_copy),
                    legacy_entry(Some("scripts"), &refused, &stray_copy),
                ],
            );
            let error = restore_backup_set_within(&resource, "unix-1", &APPLY, &rules).unwrap_err();
            assert!(
                matches!(
                    refusal(&error),
                    RestoreRefusal::TargetRefused {
                        reason: TargetRefusal::OutsideAllowedFolders,
                        ..
                    }
                ),
                "{}: {error:?}",
                refused.display()
            );
            assert_eq!(fs::read(&plugin).unwrap(), b"new", "{}", refused.display());
            assert!(!refused.exists(), "{}", refused.display());
        }

        // A place the pattern matches, once the JAWS folder itself is gone,
        // as after JAWS was uninstalled. The refusal says so, and that the
        // set's other package can still come back on its own.
        fs::remove_dir_all(&jaws).unwrap();
        let script = script_file(&appdata, "reaper.jss");
        save_backup_manifest(
            &set,
            &BackupManifest {
                schema_version: 1,
                rabbit_version: "0.4.4".to_string(),
                created_at: "unix-1".to_string(),
                created_unix: None,
                reason: "uninstall".to_string(),
                files: vec![
                    legacy_entry(Some("reapack"), &plugin, &plugin_copy),
                    legacy_entry(Some("scripts"), &script, &stray_copy),
                ],
                receipt_backup_path: None,
                packages: ["reapack", "scripts"]
                    .into_iter()
                    .map(|id| (id.to_string(), BackupPackageRecord::default()))
                    .collect(),
                replaced: Default::default(),
                keymap_origin: None,
            },
        )
        .unwrap();
        let error = restore_backup_set_within(&resource, "unix-1", &APPLY, &rules).unwrap_err();
        assert!(
            matches!(refusal(&error), RestoreRefusal::TargetRefused {
                reason: TargetRefusal::GuardFolderGone { folder },
                other_packages,
                ..
            } if *folder == jaws && other_packages == &["reapack"]),
            "{error:?}"
        );
        assert_eq!(fs::read(&plugin).unwrap(), b"new");
        assert!(!jaws.exists());
    }

    /// A script file goes back only through real folders below the JAWS
    /// folder, never through a junction there, which could lead anywhere.
    #[cfg(windows)]
    #[test]
    fn restore_refuses_a_target_behind_a_junction_under_the_vendor_folder() {
        use crate::uninstall::paths::make_junction;

        let dir = tempdir().unwrap();
        let appdata = dir.path().join("AppData");
        let resource = appdata.join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let script = script_file(&appdata, "reaper.jss");
        backed_up_and_gone(&resource, "unix-1", Some("scripts"), &script);
        // JAWS 2025's folder is now a junction to a folder elsewhere with the
        // same layout.
        let version = appdata.join("Freedom Scientific").join("JAWS").join("2025");
        fs::remove_dir_all(&version).unwrap();
        let elsewhere = dir.path().join("Elsewhere");
        fs::create_dir_all(elsewhere.join("Settings").join("enu")).unwrap();
        make_junction(&version, &elsewhere);

        let error =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &scripts_rules(&appdata))
                .unwrap_err();

        assert!(
            matches!(
                refusal(&error),
                RestoreRefusal::TargetRefused {
                    reason: TargetRefusal::ThroughLink { .. },
                    ..
                }
            ),
            "{error:?}"
        );
        assert!(
            !elsewhere
                .join("Settings")
                .join("enu")
                .join("reaper.jss")
                .exists()
        );
        // The junction goes as a link, before the folder it points to.
        fs::remove_dir(&version).unwrap();
    }

    /// A junction below the JAWS folder is refused even where it leads to
    /// another folder inside it, laid out the same: RABBIT follows no link
    /// there, wherever it points.
    #[cfg(windows)]
    #[test]
    fn restore_refuses_a_junction_under_the_vendor_folder_that_stays_inside_it() {
        use crate::uninstall::paths::make_junction;

        let dir = tempdir().unwrap();
        let appdata = dir.path().join("AppData");
        let resource = appdata.join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let script = script_file(&appdata, "reaper.jss");
        backed_up_and_gone(&resource, "unix-1", Some("scripts"), &script);
        // JAWS 2025's folder is now a junction to JAWS 2026's, which has the
        // same layout.
        let jaws = appdata.join("Freedom Scientific").join("JAWS");
        let version = jaws.join("2025");
        fs::remove_dir_all(&version).unwrap();
        let newer = jaws.join("2026");
        fs::create_dir_all(newer.join("Settings").join("enu")).unwrap();
        make_junction(&version, &newer);

        let error =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &scripts_rules(&appdata))
                .unwrap_err();

        assert!(
            matches!(
                refusal(&error),
                RestoreRefusal::TargetRefused {
                    reason: TargetRefusal::ThroughLink { .. },
                    ..
                }
            ),
            "{error:?}"
        );
        assert!(
            !newer
                .join("Settings")
                .join("enu")
                .join("reaper.jss")
                .exists()
        );
        fs::remove_dir(&version).unwrap();
    }

    /// A file goes back below the CLAP folder only through real folders. A
    /// junction or symbolic link there could lead anywhere, so the restore
    /// is refused before anything changes, in a dry run too, and nothing is
    /// written where the link leads.
    #[test]
    fn restore_never_writes_through_a_link_under_the_clap_folder() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap_dir = dir.path().join("CLAP");
        let vendor = clap_dir.join("Vendor");
        let plugin = vendor.join("plugin.clap");
        write(&plugin, b"old plugin");
        fs::create_dir_all(&resource).unwrap();
        BackupSetWriter::new(&resource, "unix-1", RESTORABLE_REASON)
            .copy_file(Some("app2clap"), &plugin)
            .unwrap();
        // The vendor's folder is now a link to a folder elsewhere.
        fs::remove_dir_all(&vendor).unwrap();
        let elsewhere = dir.path().join("Elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        link_folder(&vendor, &elsewhere);
        let before = snapshot(dir.path());

        for dry_run in [true, false] {
            let options = RestoreBackupOptions { dry_run, ..APPLY };
            let error =
                restore_backup_set_within(&resource, "unix-1", &options, &shared(&clap_dir))
                    .unwrap_err();
            assert!(
                matches!(
                    refusal(&error),
                    RestoreRefusal::TargetRefused {
                        reason: TargetRefusal::ThroughLink { .. },
                        ..
                    }
                ),
                "{dry_run}: {error:?}"
            );
            assert_eq!(snapshot(dir.path()), before, "{dry_run}");
        }
        assert!(!elsewhere.join("plugin.clap").exists());
    }

    /// The CLAP folder may be gone, as after its last plug-in went. There is
    /// no link below it to follow then: the restore makes it again and puts
    /// the file back, as an install would.
    #[test]
    fn a_clap_folder_that_is_gone_is_made_again() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap_dir = dir.path().join("CLAP");
        let app2clap = clap_dir.join("app2clap.clap");
        write(&app2clap, b"app2clap");
        fs::create_dir_all(&resource).unwrap();
        BackupSetWriter::new(&resource, "unix-1", RESTORABLE_REASON)
            .copy_file(Some("app2clap"), &app2clap)
            .unwrap();
        fs::remove_dir_all(&clap_dir).unwrap();

        restore_backup_set_within(&resource, "unix-1", &APPLY, &shared(&clap_dir)).unwrap();

        assert_eq!(fs::read(&app2clap).unwrap(), b"app2clap");
    }

    /// Below the CLAP folder, a name with a `:` is refused on every
    /// platform: Windows reads it as a stream of a file or folder. A plain
    /// name still goes back, to the CLAP folder with the names below it.
    #[test]
    fn restore_refuses_a_name_with_a_colon_under_the_clap_folder() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap_dir = dir.path().join("CLAP");
        fs::create_dir_all(clap_dir.join("Vendor")).unwrap();
        let target = |path: &Path| {
            restore_target(&resource, path, None, Some("app2clap"), &shared(&clap_dir))
        };

        for path in [
            clap_dir.join("plugin.clap:hidden"),
            clap_dir.join("Vendor:x").join("plugin.clap"),
        ] {
            let error = target(&path).unwrap_err();
            assert!(
                matches!(
                    refusal(&error),
                    RestoreRefusal::TargetRefused {
                        reason: TargetRefusal::NameNotAsWritten { .. },
                        ..
                    }
                ),
                "{}: {error:?}",
                path.display()
            );
        }
        let plain = clap_dir.join("Vendor").join("plugin.clap");
        assert_eq!(target(&plain).unwrap(), plain);
    }

    /// Below the CLAP folder, a name Windows would open as another one is
    /// refused before anything changes, in a dry run too. Here a `\\?\`
    /// path names a junction `Vendor.` next to the real folder `Vendor`:
    /// through a plain path, a check would look at `Vendor`, while the file
    /// would go through the junction. Nothing is written where it leads.
    #[cfg(windows)]
    #[test]
    fn restore_refuses_a_name_windows_reads_as_another_under_the_clap_folder() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap_dir = dir.path().join("CLAP");
        fs::create_dir_all(clap_dir.join("Vendor")).unwrap();
        let elsewhere = dir.path().join("Elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        // mklink drops the trailing dot, so the junction is made under
        // another name and renamed through a `\\?\` path, which keeps it.
        let verbatim_clap = PathBuf::from(format!(r"\\?\{}", clap_dir.display()));
        let junction = verbatim_clap.join("Vendor.");
        link_folder(&clap_dir.join("Junction"), &elsewhere);
        fs::rename(clap_dir.join("Junction"), &junction).unwrap();
        assert!(
            fs::symlink_metadata(&junction)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let set = resource.join("RABBIT/backups/unix-1");
        let copy = set.join("external/1/plugin.clap");
        write(&copy, b"old plugin");
        save_manifest(
            &set,
            RESTORABLE_REASON,
            vec![legacy_entry(
                Some("app2clap"),
                &junction.join("plugin.clap"),
                &copy,
            )],
        );
        let before = snapshot(dir.path());

        for dry_run in [true, false] {
            let options = RestoreBackupOptions { dry_run, ..APPLY };
            let error =
                restore_backup_set_within(&resource, "unix-1", &options, &shared(&clap_dir))
                    .unwrap_err();
            assert!(
                matches!(refusal(&error), RestoreRefusal::TargetRefused {
                    reason: TargetRefusal::NameNotAsWritten { name },
                    ..
                } if name == "Vendor."),
                "{dry_run}: {error:?}"
            );
            assert_eq!(snapshot(dir.path()), before, "{dry_run}");
        }
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
        assert_eq!(fs::read_dir(clap_dir.join("Vendor")).unwrap().count(), 0);
        // Only the junction goes, not what it leads to.
        fs::remove_dir(&junction).unwrap();
    }

    /// A finished entry whose copy no longer matches its recorded hash is a
    /// damaged set, and restoring part of it would mix old and new files.
    /// It is refused as part of the backup missing, by the file's original
    /// path, as the overview lists it.
    #[test]
    fn a_damaged_copy_stops_the_restore_before_changing_anything() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        let plugin = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        write(&keymap, b"osara keymap");
        write(&plugin, b"new");
        let mut writer = BackupSetWriter::new(&resource, "unix-1", RESTORABLE_REASON);
        writer.copy_file(Some("osara"), &keymap).unwrap();
        let plugin_copy = writer.copy_file(Some("reapack"), &plugin).unwrap();
        fs::write(&plugin_copy, b"bit rot").unwrap();
        fs::write(&keymap, b"newer keymap").unwrap();

        let error =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
                .unwrap_err();

        assert!(
            matches!(refusal(&error), RestoreRefusal::PartMissing { backup_id, path }
                if backup_id == "unix-1" && *path == Path::new("UserPlugins").join("reaper_reapack-x64.dll")),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"newer keymap");
    }

    /// Write-ahead: entries reach the manifest before their copy starts. A
    /// run that dies after one copy and before marking it done, and another
    /// that dies before copying at all, both leave a manifest (and the
    /// origin note) behind. Restore uses the finished copy and leaves the
    /// file whose copy never happened alone, since that run never changed
    /// it.
    #[test]
    fn write_ahead_entries_survive_a_failure_and_restore_uses_finished_copies() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap_dir = dir.path().join("CLAP");
        let app2clap = clap_dir.join("app2clap.clap");
        let keymap = resource.join("reaper-kb.ini");
        write(&app2clap, b"old app2clap");
        write(&keymap, b"own keymap");

        let mut writer = BackupSetWriter::new(&resource, "unix-1", RESTORABLE_REASON);
        let copied = writer.begin_entry(Some("app2clap"), &app2clap).unwrap();
        let copy = writer.manifest.files[copied].backup_path.clone();
        fs::create_dir_all(copy.parent().unwrap()).unwrap();
        fs::copy(&app2clap, &copy).unwrap();
        // The next file's run dies before it copies anything.
        writer.begin_entry(Some("osara"), &keymap).unwrap();
        drop(writer);

        let set = resource.join("RABBIT/backups/unix-1");
        let manifest = load_backup_manifest(&set).unwrap().unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert!(
            manifest
                .files
                .iter()
                .all(|file| file.state == BackupEntryState::Pending)
        );
        assert!(set.join("external/1/app2clap.clap.rabbit-origin").is_file());
        assert!(!set.join("backup-manifest.json.rabbit-tmp").exists());

        fs::write(&app2clap, b"new app2clap").unwrap();
        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &shared(&clap_dir)).unwrap();

        assert_eq!(fs::read(&app2clap).unwrap(), b"old app2clap");
        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert_eq!(report.actions.len(), 1);
        assert_eq!(report.skipped_targets, vec![keymap]);
    }

    /// Uninstall's way in: a file moves into the set, by rename or, from
    /// another volume, as a checked copy, and can move back while the run
    /// goes on. A file that went back leaves no entry and no origin note, and
    /// a set that ends up holding nothing goes away.
    #[test]
    fn moved_files_can_go_back_and_an_empty_set_goes_away() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let inside = resource.join("UserPlugins").join("reaper_demo.dll");
        let outside = dir.path().join("CLAP").join("demo.clap");
        write(&inside, b"demo");
        write(&outside, b"clap");

        let mut writer = BackupSetWriter::new(&resource, "uninstall-unix-1", "uninstall");
        writer
            .record_package("demo", BackupPackageRecord::default())
            .unwrap();
        let renamed = writer
            .move_file(Some("demo"), &inside, None, false)
            .unwrap();
        let copied = writer
            .move_file(Some("demo"), &outside, None, true)
            .unwrap();
        let set = resource.join("RABBIT/backups/uninstall-unix-1");
        assert!(!renamed.copied && copied.copied);
        assert!(!inside.exists() && !outside.exists());
        assert_eq!(renamed.backup_path, set.join("UserPlugins/reaper_demo.dll"));
        assert_eq!(fs::read(&copied.backup_path).unwrap(), b"clap");
        assert!(set.join("external/1/demo.clap.rabbit-origin").is_file());
        let manifest = load_backup_manifest(&set).unwrap().unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert!(
            manifest
                .files
                .iter()
                .all(|file| file.state == BackupEntryState::Done)
        );
        assert!(manifest.packages.contains_key("demo"));

        // Something new at the original path is never replaced.
        write(&inside, b"new");
        assert!(writer.put_back(&renamed).is_err());
        assert_eq!(fs::read(&inside).unwrap(), b"new");
        assert_eq!(
            load_backup_manifest(&set).unwrap().unwrap().files[0].state,
            BackupEntryState::Done,
            "the set still holds the file"
        );
        fs::remove_file(&inside).unwrap();

        writer.put_back(&copied).unwrap();
        writer.put_back(&renamed).unwrap();
        assert_eq!(fs::read(&inside).unwrap(), b"demo");
        assert_eq!(fs::read(&outside).unwrap(), b"clap");
        assert!(!set.join("external").exists());
        assert!(
            load_backup_manifest(&set)
                .unwrap()
                .unwrap()
                .files
                .is_empty()
        );

        assert!(!writer.discard_if_empty(), "the package is still recorded");
        writer.forget_package("demo").unwrap();
        assert!(writer.discard_if_empty());
        assert!(!set.exists());
    }

    /// Two sets asked for in the same second get separate folders, so the
    /// second never replaces the first one's manifest.
    #[test]
    fn two_sets_with_the_same_id_never_share_a_folder() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"keymap");

        let mut first = BackupSetWriter::new(&resource, "unix-5", "osara-keymap-replacement");
        first.copy_file(Some("osara"), &keymap).unwrap();
        let mut second = BackupSetWriter::new(&resource, "unix-5", "osara-keymap-replacement");
        second.copy_file(Some("osara"), &keymap).unwrap();

        assert_eq!(first.path(), resource.join("RABBIT/backups/unix-5"));
        assert_eq!(second.path(), resource.join("RABBIT/backups/unix-5-2"));
        for set in [first.path(), second.path()] {
            assert_eq!(load_backup_manifest(set).unwrap().unwrap().files.len(), 1);
        }
    }

    /// One damaged manifest used to fail the whole listing. Now that set
    /// carries a warning and every other set is still listed; restoring the
    /// damaged one still fails, rather than guessing.
    #[test]
    fn a_damaged_manifest_is_reported_on_its_set_and_the_others_still_list() {
        let dir = tempdir().unwrap();
        let resource = dir.path();
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"keymap");
        BackupSetWriter::new(resource, "unix-1", "osara-keymap-replacement")
            .copy_file(Some("osara"), &keymap)
            .unwrap();
        let damaged = resource.join("RABBIT/backups/unix-2");
        write(&damaged.join("reaper-kb.ini"), b"keymap");
        write(&damaged.join(BACKUP_MANIFEST_FILE), b"{ not json");

        let sets = list_backup_sets(resource).unwrap();

        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].id, "unix-1");
        assert!(sets[0].warning.is_none());
        assert_eq!(sets[0].reason.as_deref(), Some("osara-keymap-replacement"));
        assert_eq!(sets[1].id, "unix-2");
        assert!(sets[1].manifest_path.is_none());
        assert_eq!(sets[1].files, vec![PathBuf::from("reaper-kb.ini")]);
        let warning = sets[1].warning.as_deref().unwrap();
        assert!(warning.contains("manifest"), "{warning}");
        let error = restore_backup_set(resource, "unix-2", &APPLY).unwrap_err();
        assert!(
            matches!(refusal(&error), RestoreRefusal::DamagedManifest { backup_id, path, .. }
                if backup_id == "unix-2" && *path == damaged.join(BACKUP_MANIFEST_FILE)),
            "{error:?}"
        );
    }

    /// A restore changes the same files an install does, so it waits its
    /// turn behind a running install. A dry run changes nothing and needs no
    /// lock.
    #[test]
    fn restore_refuses_to_run_while_an_install_holds_the_lock() {
        let dir = tempdir().unwrap();
        let resource = dir.path();
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"own keymap");
        BackupSetWriter::new(resource, "unix-1", "osara-keymap-replacement")
            .copy_file(Some("osara"), &keymap)
            .unwrap();
        fs::write(&keymap, b"osara keymap").unwrap();
        let _install = crate::lock::acquire_package_install_lock(resource).unwrap();

        let error = restore_backup_set(resource, "unix-1", &APPLY).unwrap_err();

        assert!(
            matches!(refusal(&error), RestoreRefusal::LockHeld { pid, .. }
                if *pid == std::process::id()),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
        let dry_run = RestoreBackupOptions {
            dry_run: true,
            ..APPLY
        };
        assert!(restore_backup_set(resource, "unix-1", &dry_run).is_ok());
    }

    #[test]
    fn paths_are_compared_by_component() {
        let root = std::path::absolute("REAPER").unwrap();
        assert_eq!(
            relative_within(&root.join("UserPlugins").join("a.dll"), &root),
            Some(Path::new("UserPlugins").join("a.dll"))
        );
        assert_eq!(relative_within(&root, &root), Some(PathBuf::new()));
        // A sibling whose name only starts the same is not inside.
        let sibling = root.with_file_name("REAPER2").join("a.dll");
        assert_eq!(relative_within(&sibling, &root), None);
        // `..` could lead anywhere, so it never counts as inside.
        assert_eq!(relative_within(&root.join("..").join("a.dll"), &root), None);
    }

    /// Windows compares paths without regard to case, and a `\\?\` path
    /// names the same file as the plain one.
    #[cfg(windows)]
    #[test]
    fn windows_paths_ignore_case_and_the_verbatim_prefix() {
        let root = Path::new(r"C:\Users\Someone\AppData\Roaming\REAPER");
        for path in [
            r"c:\users\someone\appdata\roaming\reaper\UserPlugins\a.dll",
            r"\\?\C:\Users\Someone\AppData\Roaming\REAPER\UserPlugins\a.dll",
            r"C:/Users/Someone/AppData/Roaming/REAPER/UserPlugins/a.dll",
        ] {
            assert_eq!(
                relative_within(Path::new(path), root),
                Some(Path::new("UserPlugins").join("a.dll")),
                "{path}"
            );
        }
        assert_eq!(
            relative_within(
                Path::new(r"D:\Users\Someone\AppData\Roaming\REAPER\a.dll"),
                root
            ),
            None
        );
    }

    /// A resource folder in `dir` whose set `unix-1` holds the key map
    /// OSARA's replaced. `reaper-kb.ini` is OSARA's now, so a restore of
    /// `unix-1` would change it back to `own keymap`.
    fn keymap_set(dir: &Path) -> (PathBuf, PathBuf) {
        let resource = dir.join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"own keymap");
        BackupSetWriter::new(&resource, "unix-1", "osara-keymap-replacement")
            .copy_file(Some("osara"), &keymap)
            .unwrap();
        fs::write(&keymap, b"osara keymap").unwrap();
        (resource, keymap)
    }

    /// Another resource folder in `dir`, `Other`, with a set `unix-1` of its
    /// own, holding another key map. A restore that followed a link to it
    /// would put that key map into the REAPER being restored.
    fn other_resource(dir: &Path) -> PathBuf {
        let other = dir.join("Other");
        let keymap = other.join("reaper-kb.ini");
        write(&keymap, b"another keymap");
        BackupSetWriter::new(&other, "unix-1", "osara-keymap-replacement")
            .copy_file(Some("osara"), &keymap)
            .unwrap();
        other
    }

    /// Every file and link below `root`, each file with what it holds,
    /// found without following any link. Folders are left out: a refused
    /// restore may still have made the empty folder its lock goes in.
    fn snapshot(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        let mut found = Vec::new();
        let mut folders = vec![root.to_path_buf()];
        while let Some(folder) = folders.pop() {
            for entry in fs::read_dir(&folder).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let kind = entry.file_type().unwrap();
                if kind.is_symlink() {
                    found.push((path, None));
                } else if kind.is_dir() {
                    folders.push(path);
                } else {
                    let contents = fs::read(&path).unwrap();
                    found.push((path, Some(contents)));
                }
            }
        }
        found.sort();
        found
    }

    /// Restore `backup_id` from `resource`, or only `package_id` from it, in
    /// a dry run and for real, expecting the same refusal both times, and
    /// check that no file or link below `dir` changed. The refusal comes
    /// before the places outside the resource folder are even looked up,
    /// and its text is plain English, with no command line in it.
    fn refused_with(
        dir: &Path,
        resource: &Path,
        backup_id: &str,
        package_id: Option<&str>,
    ) -> RabbitError {
        let before = snapshot(dir);
        let mut errors = Vec::new();
        for dry_run in [true, false] {
            let options = RestoreBackupOptions {
                dry_run,
                package_id: package_id.map(ToString::to_string),
                ..APPLY
            };
            errors.push(
                restore_backup_set_with(
                    resource,
                    backup_id,
                    &options,
                    &BackupEnvironment::AS_USER,
                    || panic!("a refused set looked up where outside files go"),
                )
                .unwrap_err(),
            );
            assert_eq!(snapshot(dir), before, "{backup_id:?} changed something");
        }
        let refused = errors.pop().unwrap();
        let text = refused.to_string();
        assert_eq!(errors[0].to_string(), text, "the dry run said otherwise");
        assert!(!text.contains("--") && !text.contains("rabbit "), "{text}");
        refused
    }

    /// Restore `backup_id` from `resource` for real, expecting a refusal,
    /// and check that no file or link below `dir` changed.
    fn refused(dir: &Path, resource: &Path, backup_id: &str) -> RabbitError {
        let before = snapshot(dir);
        let error =
            restore_backup_set_within(resource, backup_id, &APPLY, &OutsideRestoreRules::default())
                .unwrap_err();
        assert_eq!(snapshot(dir), before, "{backup_id:?} changed something");
        error
    }

    /// Restore `backup_id` from `resource`, in a dry run and for real,
    /// expecting a refusal of the plan both times, and check that no file
    /// or link below `dir` changed. Unlike [`refused_with`], the places
    /// outside the resource folder are looked up: there are none.
    fn plan_refused(dir: &Path, resource: &Path, backup_id: &str) -> RabbitError {
        let before = snapshot(dir);
        let mut errors = Vec::new();
        for dry_run in [true, false] {
            let options = RestoreBackupOptions { dry_run, ..APPLY };
            errors.push(
                restore_backup_set_within(
                    resource,
                    backup_id,
                    &options,
                    &OutsideRestoreRules::default(),
                )
                .unwrap_err(),
            );
            assert_eq!(snapshot(dir), before, "{backup_id:?} changed something");
        }
        let refusal = errors.pop().unwrap();
        assert_eq!(
            errors[0].to_string(),
            refusal.to_string(),
            "the dry run said otherwise"
        );
        refusal
    }

    #[test]
    fn a_backup_id_is_exactly_one_plain_name() {
        for id in [
            "unix-1",
            "unix-5-2",
            "uninstall-unix-1700000000",
            "unattended-unix-1700000000-123456789",
            "restore-unix-1700000000-before-restore",
        ] {
            assert!(backup_id_is_well_formed(id), "{id:?}");
        }
        for id in [
            "",
            ".",
            "..",
            "a/b",
            r"a\b",
            "/unix-1",
            "unix-1/",
            "D:",
            "C:foo",
            "x:stream",
            "unix-1:stream",
            "unix-1::$INDEX_ALLOCATION",
            r"\\?\C:\x",
            r"\\server\share",
            "a\0b",
        ] {
            assert!(!backup_id_is_well_formed(id), "{id:?}");
        }
        #[cfg(windows)]
        for id in ["unix-1.", "unix-1 ", "CON", "NUL", "con.txt", "Lpt1.log"] {
            assert!(!backup_id_is_well_formed(id), "{id:?}");
        }
    }

    #[test]
    fn windows_device_names_are_known_in_any_case_and_before_an_extension() {
        for name in [
            "CON",
            "con",
            "Con.txt",
            "PRN",
            "AUX",
            "NUL",
            "nul.tar.gz",
            "NUL .log",
            "COM1",
            "com9",
            "COM0",
            "LPT1",
            "lpt9.txt",
            "COM¹",
            "LPT³",
            "CONIN$",
            "conout$",
        ] {
            assert!(is_windows_device_name(name), "{name:?}");
        }
        for name in [
            "unix-1",
            "CONSOLE",
            "con-1",
            "xnul",
            "COM",
            "COM10",
            "LPTx",
            "uninstall-unix-1700000000",
        ] {
            assert!(!is_windows_device_name(name), "{name:?}");
        }
    }

    /// An id that could make the path leave `RABBIT/backups`, such as `D:`
    /// on Windows, is refused before anything is read, with nothing
    /// changed. Before, `D:` loaded a manifest from D:'s current folder, or
    /// restored that folder's files into REAPER.
    #[test]
    fn ids_that_could_leave_the_backups_folder_are_refused() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());

        for id in [
            "",
            ".",
            "..",
            "a/b",
            r"a\b",
            "unix-1/",
            "../backups/unix-1",
            "D:",
            "C:foo",
            "x:stream",
            "unix-1:stream",
            "unix-1::$INDEX_ALLOCATION",
            r"\\?\C:\x",
            "a\0b",
        ] {
            let error = refused(dir.path(), &resource, id);
            assert!(
                matches!(refusal(&error), RestoreRefusal::InvalidId { backup_id: refused } if refused == id),
                "{id:?}: {error:?}"
            );
        }
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
    }

    /// Windows drops a trailing dot or space, so `unix-1.` opened `unix-1`,
    /// and a device name opens the device in any folder. Both are refused.
    /// An id that names a set once its dots and spaces at the end are gone
    /// names that set, and still isn't restored.
    #[cfg(windows)]
    #[test]
    fn windows_refuses_ids_it_would_read_as_another_name_or_a_device() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());

        for id in [
            "unix-2.", "unix-2 ", "CON", "NUL", "con.txt", "NUL .txt", "lpt1", "COM9.log", "COM¹",
        ] {
            let error = refused(dir.path(), &resource, id);
            assert!(
                matches!(refusal(&error), RestoreRefusal::InvalidId { backup_id: refused } if refused == id),
                "{id:?}: {error:?}"
            );
        }
        for id in ["unix-1.", "unix-1 ", "unix-1. "] {
            let error = refused(dir.path(), &resource, id);
            assert!(
                matches!(refusal(&error), RestoreRefusal::EndsInDotOrSpace { backup_id, existing }
                    if backup_id == id && existing == "unix-1"),
                "{id:?}: {error:?}"
            );
        }
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
    }

    /// Through a `\\?\` path, a folder named like a device can be made in
    /// `RABBIT/backups`, as on Windows 11. It is listed as no backup, after
    /// the sets, since it has no date, and never looked into, and its name
    /// is still refused as an id, so the device is never opened in its
    /// place. Where the folder can't be made, the id is refused all the
    /// same.
    #[cfg(windows)]
    #[test]
    fn a_folder_named_like_a_device_is_never_restored() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let backups = std::path::absolute(resource.join("RABBIT").join("backups")).unwrap();
        let made = fs::create_dir(format!(r"\\?\{}\CON", backups.display())).is_ok();

        if made {
            let sets = list_backup_sets(&resource).unwrap();
            let ids: Vec<&str> = sets.iter().map(|set| set.id.as_str()).collect();
            assert_eq!(ids, ["unix-1", "CON"]);
            assert!(sets[1].manifest_path.is_none() && sets[1].files.is_empty());
            let warning = sets[1].warning.as_deref().unwrap();
            assert!(
                warning.contains("not one RABBIT gives a backup"),
                "{warning}"
            );
            assert!(sets[0].warning.is_none() && sets[0].manifest_path.is_some());
        }
        let error = refused(dir.path(), &resource, "CON");

        assert!(
            matches!(refusal(&error), RestoreRefusal::InvalidId { backup_id: id } if id == "CON"),
            "made: {made}, {error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
    }

    /// Windows opens `unix-1` for a path through `unix-1.`, so a folder
    /// named `unix-1.`, which only a `\\?\` path can make, would be listed
    /// with the manifest and files of `unix-1`. It is listed as no backup,
    /// never looked into, and its name is refused as an id.
    #[cfg(windows)]
    #[test]
    fn a_folder_whose_name_windows_would_change_is_never_looked_into() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let backups = std::path::absolute(resource.join("RABBIT").join("backups")).unwrap();
        fs::create_dir(format!(r"\\?\{}\unix-1.", backups.display())).unwrap();

        let sets = list_backup_sets(&resource).unwrap();

        let ids: Vec<&str> = sets.iter().map(|set| set.id.as_str()).collect();
        assert_eq!(ids, ["unix-1", "unix-1."]);
        assert!(sets[0].warning.is_none() && sets[0].manifest_path.is_some());
        let dotted = &sets[1];
        assert!(dotted.manifest_path.is_none() && dotted.reason.is_none());
        assert!(dotted.files.is_empty(), "{:?}", dotted.files);
        let warning = dotted.warning.as_deref().unwrap();
        assert!(
            warning.contains("not one RABBIT gives a backup"),
            "{warning}"
        );

        // Refused, and named after the set it would have opened.
        let error = refused(dir.path(), &resource, "unix-1.");
        assert!(
            matches!(refusal(&error), RestoreRefusal::EndsInDotOrSpace { backup_id, existing }
                if backup_id == "unix-1." && existing == "unix-1"),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
    }

    /// Only the exact name of a set counts, byte for byte, even where the
    /// file system ignores case and would open the set. The error names
    /// the set that is there, so nobody is told it is gone.
    #[test]
    fn a_name_that_differs_only_in_case_is_no_set() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());

        for id in ["UNIX-1", "Unix-1"] {
            let error = refused(dir.path(), &resource, id);
            assert!(
                matches!(refusal(&error), RestoreRefusal::DiffersInCase { backup_id, existing }
                        if backup_id == id && existing == "unix-1"
                ),
                "{id:?}: {error:?}"
            );
            let message = error.to_string();
            assert!(message.contains("there is one named unix-1"), "{message}");
            assert!(!message.contains("deleted"), "{message}");
        }
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
    }

    #[test]
    fn an_id_with_no_set_was_deleted_or_never_existed() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());

        let error = refused(dir.path(), &resource, "unix-2");

        assert!(
            matches!(refusal(&error), RestoreRefusal::NotFound { backup_id } if backup_id == "unix-2"),
            "{error:?}"
        );
        assert!(
            error.to_string().contains("deleted or never existed"),
            "{error}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");

        // A REAPER with no RABBIT folder has no sets, and gets none of
        // RABBIT's folders from looking.
        let fresh = dir.path().join("Fresh");
        fs::create_dir_all(&fresh).unwrap();
        let error = refused(dir.path(), &fresh, "unix-1");
        assert!(
            matches!(refusal(&error), RestoreRefusal::NotFound { .. }),
            "{error:?}"
        );
        assert!(!fresh.join("RABBIT").exists());
    }

    /// A set whose deletion started and didn't finish sits in
    /// `RABBIT/backups-deleting`. Part of it may be gone, so restore refuses
    /// it. A set of the same name back in `RABBIT/backups` restores, and
    /// the unfinished deletion is left alone.
    #[test]
    fn a_set_found_only_among_unfinished_deletions_is_being_deleted() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let rabbit = resource.join("RABBIT");
        let deleting = rabbit.join(DELETING_DIR).join("unix-1");
        fs::create_dir_all(deleting.parent().unwrap()).unwrap();
        fs::rename(rabbit.join("backups").join("unix-1"), &deleting).unwrap();

        let error = refused(dir.path(), &resource, "unix-1");

        assert!(
            matches!(refusal(&error), RestoreRefusal::BeingDeleted { backup_id } if backup_id == "unix-1"),
            "{error:?}"
        );
        assert_eq!(
            find_backup_set(&resource, "unix-1").unwrap(),
            FoundBackupSet {
                set: None,
                deleting: Some(deleting.clone()),
                other_case: None,
            }
        );

        fs::write(&keymap, b"own keymap").unwrap();
        BackupSetWriter::new(&resource, "unix-1", "osara-keymap-replacement")
            .copy_file(Some("osara"), &keymap)
            .unwrap();
        fs::write(&keymap, b"osara keymap").unwrap();
        let set = rabbit.join("backups").join("unix-1");
        assert_eq!(
            find_backup_set(&resource, "unix-1").unwrap(),
            FoundBackupSet {
                set: Some(set.clone()),
                deleting: Some(deleting.clone()),
                other_case: None,
            }
        );
        let unfinished = snapshot(&deleting);

        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        assert_eq!(report.backup_path, set);
        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert_eq!(snapshot(&deleting), unfinished);
    }

    /// The set an id names exactly restores as before, and looking for
    /// unfinished deletions never makes their folder.
    #[test]
    fn the_set_an_id_names_restores_and_no_deletions_folder_appears() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());

        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        assert_eq!(
            report.backup_path,
            resource.join("RABBIT").join("backups").join("unix-1")
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert!(!resource.join("RABBIT").join(DELETING_DIR).exists());
    }

    /// A junction or symbolic link in `RABBIT/backups` is no set RABBIT
    /// made. It is listed as such, never looked into, and never restored,
    /// wherever it leads: here to another REAPER's set.
    #[test]
    fn a_set_that_is_a_link_is_listed_as_no_backup_and_never_restored() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let other = other_resource(dir.path());
        let link = resource.join("RABBIT").join("backups").join("unix-9");
        link_folder(&link, &other.join("RABBIT").join("backups").join("unix-1"));

        let error = refused(dir.path(), &resource, "unix-9");

        assert!(
            matches!(refusal(&error), RestoreRefusal::IsLink { path } if *path == link),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
        let sets = list_backup_sets(&resource).unwrap();
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].id, "unix-1");
        assert!(sets[0].warning.is_none());
        assert_eq!(sets[1].id, "unix-9");
        assert!(sets[1].manifest_path.is_none() && sets[1].reason.is_none());
        assert!(sets[1].files.is_empty());
        let warning = sets[1].warning.as_deref().unwrap();
        assert!(warning.contains("not a backup RABBIT made"), "{warning}");
    }

    /// When `RABBIT/backups` is a junction or symbolic link, nothing in it
    /// is listed or restored: its sets may be another REAPER's.
    #[test]
    fn a_backups_folder_that_is_a_link_lists_and_restores_nothing() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"osara keymap");
        let other = other_resource(dir.path());
        let backups = resource.join("RABBIT").join("backups");
        fs::create_dir_all(backups.parent().unwrap()).unwrap();
        link_folder(&backups, &other.join("RABBIT").join("backups"));

        let error = refused(dir.path(), &resource, "unix-1");

        assert!(
            matches!(refusal(&error), RestoreRefusal::IsLink { path } if *path == backups),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
        let error = list_backup_sets(&resource).unwrap_err();
        assert!(
            matches!(&error, RabbitError::BackupPathIsLink { path } if *path == backups),
            "{error:?}"
        );
    }

    /// The same when `RABBIT` itself is a junction or symbolic link.
    #[test]
    fn a_rabbit_folder_that_is_a_link_lists_and_restores_nothing() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"osara keymap");
        let other = other_resource(dir.path());
        let rabbit = resource.join("RABBIT");
        link_folder(&rabbit, &other.join("RABBIT"));

        let error = refused(dir.path(), &resource, "unix-1");

        assert!(
            matches!(refusal(&error), RestoreRefusal::IsLink { path } if *path == rabbit),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
        let error = list_backup_sets(&resource).unwrap_err();
        assert!(
            matches!(&error, RabbitError::BackupPathIsLink { path } if *path == rabbit),
            "{error:?}"
        );
    }

    /// Restore looks among unfinished deletions too, and follows no link
    /// there either.
    #[test]
    fn a_deletions_folder_that_is_a_link_stops_a_restore() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let other = other_resource(dir.path());
        let deleting = resource.join("RABBIT").join(DELETING_DIR);
        link_folder(&deleting, &other.join("RABBIT").join("backups"));

        let error = refused(dir.path(), &resource, "unix-1");

        assert!(
            matches!(refusal(&error), RestoreRefusal::IsLink { path } if *path == deleting),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
    }

    /// A junction or symbolic link inside a set is listed by name and
    /// never looked into. A set restored by its layout is refused whole,
    /// and so is a copy its manifest names behind such a link.
    #[test]
    fn a_link_inside_a_set_is_listed_by_name_and_never_followed() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"osara keymap");
        let elsewhere = dir.path().join("Elsewhere");
        write(&elsewhere.join("reaper-kb.ini"), b"another keymap");
        // A set without a manifest, restored by its layout.
        let layout_set = resource.join("RABBIT").join("backups").join("unix-1");
        write(&layout_set.join("reaper-kb.ini"), b"own keymap");
        let layout_link = layout_set.join("Linked");
        link_folder(&layout_link, &elsewhere);

        let sets = list_backup_sets(&resource).unwrap();
        assert_eq!(
            sets[0].files,
            vec![PathBuf::from("Linked"), PathBuf::from("reaper-kb.ini")]
        );
        let warning = sets[0].warning.as_deref().unwrap();
        assert!(warning.contains("Linked"), "{warning}");

        let error = refused(dir.path(), &resource, "unix-1");
        assert!(
            matches!(refusal(&error), RestoreRefusal::IsLink { path } if *path == layout_link),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");

        // A set whose manifest names a copy in a folder that is now a link.
        let plugin = resource.join("UserPlugins").join("reaper_demo.dll");
        write(&plugin, b"old");
        let mut writer = BackupSetWriter::new(&resource, "unix-2", RESTORABLE_REASON);
        writer.copy_file(Some("demo"), &plugin).unwrap();
        fs::write(&plugin, b"new").unwrap();
        let manifest_set = writer.path().to_path_buf();
        let plugins_link = manifest_set.join("UserPlugins");
        let moved = dir.path().join("MovedPlugins");
        fs::rename(&plugins_link, &moved).unwrap();
        link_folder(&plugins_link, &moved);

        let error = refused(dir.path(), &resource, "unix-2");
        assert!(
            matches!(refusal(&error), RestoreRefusal::IsLink { path } if *path == plugins_link),
            "{error:?}"
        );
        assert_eq!(fs::read(&plugin).unwrap(), b"new");
    }

    /// A copy, or a manifest, that is a symbolic link to a file is never
    /// read, even when the file it leads to holds the very bytes the
    /// manifest expects. Windows needs extra rights for links to files, so
    /// this runs where they need none.
    #[cfg(unix)]
    #[test]
    fn a_file_link_in_a_set_is_never_read() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let set = resource.join("RABBIT").join("backups").join("unix-1");
        let copy = set.join("reaper-kb.ini");
        let elsewhere = dir.path().join("elsewhere-kb.ini");
        fs::rename(&copy, &elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &copy).unwrap();

        let error = refused(dir.path(), &resource, "unix-1");
        assert!(
            matches!(refusal(&error), RestoreRefusal::IsLink { path } if *path == copy),
            "{error:?}"
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
        let sets = list_backup_sets(&resource).unwrap();
        assert_eq!(sets[0].files, vec![PathBuf::from("reaper-kb.ini")]);
        let warning = sets[0].warning.as_deref().unwrap();
        assert!(warning.contains("reaper-kb.ini"), "{warning}");

        let manifest = set.join(BACKUP_MANIFEST_FILE);
        let moved_manifest = dir.path().join("elsewhere-manifest.json");
        fs::rename(&manifest, &moved_manifest).unwrap();
        std::os::unix::fs::symlink(&moved_manifest, &manifest).unwrap();

        let error = refused(dir.path(), &resource, "unix-1");
        assert!(
            matches!(refusal(&error), RestoreRefusal::IsLink { path } if *path == manifest),
            "{error:?}"
        );
        let sets = list_backup_sets(&resource).unwrap();
        assert!(sets[0].manifest_path.is_none());
        let warning = sets[0].warning.as_deref().unwrap();
        let link_error = RabbitError::BackupPathIsLink { path: manifest };
        assert!(
            warning.contains(&format!(
                "the backup manifest could not be read: {link_error}"
            )),
            "{warning}"
        );
    }

    /// No writer makes a set behind a link a restore wouldn't follow. An
    /// uninstall's set is the only copy of the files it moves; an update's
    /// or a key map's holds the only copy of what they replace. So the first
    /// write is refused, whatever it is: an uninstall's package record, an
    /// update's record, a file copied or moved in. Nothing is written where
    /// the link leads, and the file stays where it was.
    #[test]
    fn no_set_is_ever_made_behind_a_link() {
        type FirstWrite = fn(&mut BackupSetWriter, &Path) -> super::Result<()>;
        let first_writes: [(&str, FirstWrite); 4] = [
            ("uninstall record", |writer, _| {
                writer.record_package("sws", BackupPackageRecord::default())
            }),
            ("update record", |writer, _| {
                writer.record_replaced("sws", ReplacedPackageRecord::default())
            }),
            ("copy", |writer, plugin| {
                writer.copy_file(Some("sws"), plugin).map(|_| ())
            }),
            ("move", |writer, plugin| {
                writer
                    .move_file(Some("sws"), plugin, None, false)
                    .map(|_| ())
                    .map_err(|error| match error {
                        super::MoveFileError::NotMoved(error) => error,
                        other => panic!("{other:?}"),
                    })
            }),
        ];
        let dir = tempdir().unwrap();
        for (index, linked) in [&["RABBIT"][..], &["RABBIT", "backups"][..]]
            .into_iter()
            .enumerate()
        {
            for (what, first_write) in first_writes {
                let resource = dir.path().join(format!("REAPER-{index}-{what}"));
                let plugin = resource.join("UserPlugins").join("reaper_sws-x64.dll");
                write(&plugin, b"sws");
                let link = linked
                    .iter()
                    .fold(resource.clone(), |path, name| path.join(name));
                fs::create_dir_all(link.parent().unwrap()).unwrap();
                let elsewhere = dir.path().join(format!("Elsewhere-{index}-{what}"));
                fs::create_dir_all(&elsewhere).unwrap();
                link_folder(&link, &elsewhere);

                assert_eq!(linked_backups_folder(&resource).as_ref(), Some(&link));
                let mut writer = BackupSetWriter::new(&resource, "unix-1", "any");
                let error = first_write(&mut writer, &plugin).unwrap_err();

                assert!(
                    matches!(&error, RabbitError::BackupPathIsLink { path } if *path == link),
                    "{linked:?} {what}: {error:?}"
                );
                assert!(!writer.is_started(), "{linked:?} {what}");
                assert_eq!(
                    fs::read_dir(&elsewhere).unwrap().count(),
                    0,
                    "{linked:?} {what}"
                );
                assert_eq!(fs::read(&plugin).unwrap(), b"sws", "{linked:?} {what}");
            }
        }
        // A real folder, or none yet, is no link, and the set is made.
        for (what, first_write) in first_writes {
            let resource = dir.path().join(format!("REAPER-{what}"));
            let plugin = resource.join("UserPlugins").join("reaper_sws-x64.dll");
            write(&plugin, b"sws");
            assert_eq!(linked_backups_folder(&resource), None);
            let mut writer = BackupSetWriter::new(&resource, "unix-1", "any");
            first_write(&mut writer, &plugin).unwrap();
            assert!(writer.path().is_dir(), "{what}");
            assert_eq!(linked_backups_folder(&resource), None);
        }
    }

    /// A set that started in a real folder writes nothing more once `RABBIT`,
    /// `RABBIT/backups` or the set's own folder has become a link, with
    /// what was there moved behind it: each later write is refused before
    /// it is written, whatever it is, nothing changes where the link leads,
    /// and the file stays where it was.
    #[test]
    fn a_started_set_writes_nothing_through_a_link_made_since() {
        type LaterWrite = fn(&mut BackupSetWriter, &Path) -> super::Result<()>;
        let later_writes: [(&str, LaterWrite); 4] = [
            ("uninstall record", |writer, _| {
                writer.record_package("reapack", BackupPackageRecord::default())
            }),
            ("update record", |writer, _| {
                writer.record_replaced("reapack", ReplacedPackageRecord::default())
            }),
            ("copy", |writer, plugin| {
                writer.copy_file(Some("reapack"), plugin).map(|_| ())
            }),
            ("move", |writer, plugin| {
                writer
                    .move_file(Some("reapack"), plugin, None, false)
                    .map(|_| ())
                    .map_err(|error| match error {
                        super::MoveFileError::NotMoved(error) => error,
                        other => panic!("{other:?}"),
                    })
            }),
        ];
        let dir = tempdir().unwrap();
        for linked in ["RABBIT", "backups", "set"] {
            for (what, later_write) in later_writes {
                let resource = dir.path().join(format!("REAPER-{linked}-{what}"));
                let sws = resource.join("UserPlugins").join("reaper_sws-x64.dll");
                let reapack = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
                write(&sws, b"sws");
                write(&reapack, b"reapack");
                let mut writer = BackupSetWriter::new(&resource, "unix-1", "any");
                writer.copy_file(Some("sws"), &sws).unwrap();
                let link = match linked {
                    "RABBIT" => resource.join("RABBIT"),
                    "backups" => resource.join("RABBIT").join("backups"),
                    _ => writer.path().to_path_buf(),
                };
                let elsewhere = dir.path().join(format!("Elsewhere-{linked}-{what}"));
                fs::rename(&link, &elsewhere).unwrap();
                link_folder(&link, &elsewhere);
                let before = snapshot(&elsewhere);

                let error = later_write(&mut writer, &reapack).unwrap_err();

                assert!(
                    matches!(&error, RabbitError::BackupPathIsLink { path } if *path == link),
                    "{linked} {what}: {error:?}"
                );
                assert_eq!(snapshot(&elsewhere), before, "{linked} {what}");
                assert_eq!(fs::read(&reapack).unwrap(), b"reapack", "{linked} {what}");
            }
        }
    }

    /// Every set records when it was made, in seconds since 1970, whatever
    /// writes it first: a file, an uninstall's package record or an
    /// update's record. `created_at` stays the id asked for, also when the
    /// folder got `-2`. A planning writer writes nothing.
    #[test]
    fn every_set_records_when_it_was_made() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins").join("reaper_sws-x64.dll");
        write(&plugin, b"sws");
        let now = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        };

        let before = now();
        let mut by_file = BackupSetWriter::new(&resource, "unix-1", UPDATE_BACKUP_REASON);
        by_file.copy_file(Some("sws"), &plugin).unwrap();
        let mut by_package = BackupSetWriter::new(&resource, "unix-1", "uninstall");
        by_package
            .record_package("sws", BackupPackageRecord::default())
            .unwrap();
        let mut by_update = BackupSetWriter::new(&resource, "unix-1", UPDATE_BACKUP_REASON);
        by_update
            .record_replaced("sws", ReplacedPackageRecord::default())
            .unwrap();
        let after = now();

        for (writer, id) in [
            (&by_file, "unix-1"),
            (&by_package, "unix-1-2"),
            (&by_update, "unix-1-3"),
        ] {
            assert_eq!(writer.set_id(), id);
            let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
            assert_eq!(manifest.created_at, "unix-1");
            let made = manifest.created_unix.expect("a time");
            assert!(before <= made && made <= after, "{before} {made} {after}");
        }
        let mut planning = BackupSetWriter::planning(&resource, "unix-9", UPDATE_BACKUP_REASON);
        planning
            .record_replaced("sws", ReplacedPackageRecord::default())
            .unwrap();
        planning.copy_file(Some("sws"), &plugin).unwrap();
        assert!(!planning.path().exists());
    }

    /// An update's set records each package before the first of its files
    /// goes in: its receipt entry from before, or none for a copy RABBIT
    /// didn't install, and the version that replaced it. A second record of
    /// the same package keeps the first. In the file, a missing entry is
    /// simply left out.
    #[test]
    fn an_update_set_records_each_package_before_its_files() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let reapack = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        let sws = resource.join("UserPlugins").join("reaper_sws-x64.dll");
        write(&reapack, b"reapack 1.0");
        write(&sws, b"sws by hand");
        let by_hand = ReplacedPackageRecord {
            receipt: None,
            replaced_by: Some(crate::version::Version::parse("2.14").unwrap()),
        };
        let mut writer = BackupSetWriter::new(&resource, "unix-1", UPDATE_BACKUP_REASON);

        writer
            .record_replaced("reapack", reapack_replaced())
            .unwrap();
        let on_disk = load_backup_manifest(writer.path()).unwrap().unwrap();
        assert!(on_disk.files.is_empty(), "the record comes first");
        assert_eq!(on_disk.replaced["reapack"], reapack_replaced());
        writer.copy_file(Some("reapack"), &reapack).unwrap();
        writer.record_replaced("sws", by_hand.clone()).unwrap();
        writer.copy_file(Some("sws"), &sws).unwrap();
        writer
            .record_replaced("reapack", ReplacedPackageRecord::default())
            .unwrap();

        let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
        assert_eq!(manifest.reason, UPDATE_BACKUP_REASON);
        assert_eq!(manifest.replaced.len(), 2);
        assert_eq!(manifest.replaced["reapack"], reapack_replaced());
        assert_eq!(manifest.replaced["sws"], by_hand);
        assert_eq!(manifest.receipt_backup_path, None);
        assert!(manifest.packages.is_empty());
        let json: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(writer.path().join(BACKUP_MANIFEST_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(json["replaced"]["reapack"]["replaced_by"], "1.2.5");
        assert_eq!(json["replaced"]["reapack"]["receipt"]["id"], "reapack");
        assert_eq!(
            json["replaced"]["sws"],
            serde_json::json!({ "replaced_by": "2.14" })
        );
        assert!(json.get("packages").is_none() && json.get("keymap_origin").is_none());
    }

    /// When the first file of a package can't be copied, as when it can't
    /// be read, the update's record of the package goes again, and a set
    /// with nothing else in it with it: no set holds a record without any
    /// file. A record whose package already has a file in the set stays.
    #[test]
    fn a_failed_first_copy_leaves_no_record_behind() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let reapack = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        let gone = resource.join("UserPlugins").join("reaper_sws-x64.dll");
        let mut writer = BackupSetWriter::new(&resource, "unix-1", UPDATE_BACKUP_REASON);

        writer
            .record_replaced("sws", ReplacedPackageRecord::default())
            .unwrap();
        assert!(writer.copy_file(Some("sws"), &gone).is_err());
        writer.forget_replaced_without_entry("sws");
        assert!(!writer.path().exists());

        write(&reapack, b"reapack 1.0");
        let mut writer = BackupSetWriter::new(&resource, "unix-2", UPDATE_BACKUP_REASON);
        writer
            .record_replaced("reapack", reapack_replaced())
            .unwrap();
        writer.copy_file(Some("reapack"), &reapack).unwrap();
        writer
            .record_replaced("sws", ReplacedPackageRecord::default())
            .unwrap();
        assert!(writer.copy_file(Some("sws"), &gone).is_err());
        writer.forget_replaced_without_entry("sws");
        writer.forget_replaced_without_entry("reapack");

        let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
        let recorded: Vec<&String> = manifest.replaced.keys().collect();
        assert_eq!(recorded, ["reapack"]);
        assert_eq!(manifest.files.len(), 1);
    }

    /// A key map set records what the key map was, for each state, as a
    /// kebab-case word, and reads it back. A word a later version may write
    /// reads as unknown, so its manifest still loads.
    #[test]
    fn a_key_map_set_records_what_the_key_map_was() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        write(&keymap, b"KEY 1 65 40001 0\r\n");
        for (index, (origin, word)) in [
            (KeymapOrigin::OwnKeymap, "own-keymap"),
            (KeymapOrigin::UnchangedOsara, "unchanged-osara"),
            (KeymapOrigin::DefaultKeys, "default-keys"),
            (KeymapOrigin::Unknown, "unknown"),
        ]
        .into_iter()
        .enumerate()
        {
            let id = format!("unattended-unix-{index}-0");
            let mut writer = BackupSetWriter::new(&resource, &id, KEYMAP_BACKUP_REASON);
            writer.set_keymap_origin(origin).unwrap();
            writer.copy_file(Some("osara"), &keymap).unwrap();

            let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
            assert_eq!(manifest.keymap_origin, Some(origin));
            let json: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(writer.path().join(BACKUP_MANIFEST_FILE)).unwrap(),
            )
            .unwrap();
            assert_eq!(json["keymap_origin"], word);
        }
        let later: KeymapOrigin = serde_json::from_str("\"a-later-kind\"").unwrap();
        assert_eq!(later, KeymapOrigin::Unknown);
    }

    /// Manifests written before these fields existed still load, with each
    /// new field empty, and restore exactly as before: an uninstall's set
    /// merges its package back, a key map set puts the key map back, and an
    /// update's set is refused. Saved again, they gain none of the fields.
    #[test]
    fn older_manifests_without_the_new_fields_still_load_and_restore() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins").join("reaper_sws-x64.dll");
        let keymap = resource.join("reaper-kb.ini");
        let reapack = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        let backups = std::path::absolute(resource.join("RABBIT").join("backups")).unwrap();
        let entry = |package: &str, original: &str, set: &str, contents: &[u8]| {
            let copy = original
                .split('/')
                .fold(backups.join(set), |path, name| path.join(name));
            write(&copy, contents);
            serde_json::json!({
                "package_id": package,
                "original_path": original,
                "backup_path": copy,
                "size": contents.len(),
                "sha256": sha256_file(&copy).unwrap(),
                "state": "done",
            })
        };
        let manifests = [
            serde_json::json!({
                "schema_version": 1,
                "rabbit_version": "0.5.0",
                "created_at": "uninstall-unix-1",
                "reason": "uninstall",
                "files": [entry("sws", "UserPlugins/reaper_sws-x64.dll", "uninstall-unix-1", b"sws")],
                "receipt_backup_path": null,
                "packages": { "sws": { "receipt": receipt_entry("sws") } },
            }),
            serde_json::json!({
                "schema_version": 1,
                "rabbit_version": "0.4.4",
                "created_at": "unattended-unix-2-0",
                "reason": KEYMAP_BACKUP_REASON,
                "files": [entry("osara", "reaper-kb.ini", "unattended-unix-2-0", b"own keymap")],
                "receipt_backup_path": null,
            }),
            serde_json::json!({
                "schema_version": 1,
                "rabbit_version": "0.4.4",
                "created_at": "unix-3",
                "reason": UPDATE_BACKUP_REASON,
                "files": [entry(
                    "reapack",
                    "UserPlugins/reaper_reapack-x64.dll",
                    "unix-3",
                    b"reapack 1.0",
                )],
                "receipt_backup_path": null,
            }),
        ];
        for manifest in &manifests {
            let set = backups.join(manifest["created_at"].as_str().unwrap());
            fs::write(
                set.join(BACKUP_MANIFEST_FILE),
                serde_json::to_string_pretty(manifest).unwrap(),
            )
            .unwrap();
            let loaded = load_backup_manifest(&set).unwrap().unwrap();
            assert_eq!(loaded.created_unix, None);
            assert!(loaded.replaced.is_empty());
            assert_eq!(loaded.keymap_origin, None);
            assert!(
                loaded
                    .packages
                    .values()
                    .all(|record| record.with_settings.is_none())
            );
            let again = serde_json::to_value(&loaded).unwrap();
            for field in ["created_unix", "replaced", "keymap_origin"] {
                assert!(again.get(field).is_none(), "{field}");
            }
            assert_eq!(again.get("packages"), manifest.get("packages"));
        }
        write(&keymap, b"osara keymap");
        write(&reapack, b"reapack 1.2.5");
        let mut since = InstallState::default();
        since.declined_packages.insert("sws".to_string());
        save_install_state(&resource, &since).unwrap();

        let rules = OutsideRestoreRules::default();
        let report =
            restore_backup_set_within(&resource, "uninstall-unix-1", &APPLY, &rules).unwrap();
        assert!(report.packages[0].receipt_merged);
        restore_backup_set_within(&resource, "unattended-unix-2-0", &APPLY, &rules).unwrap();
        let refused = refused_with(dir.path(), &resource, "unix-3", None);

        assert_eq!(fs::read(&plugin).unwrap(), b"sws");
        let state = load_install_state(&resource).unwrap().unwrap();
        assert_eq!(state.packages.get("sws"), Some(&receipt_entry("sws")));
        assert!(state.declined_packages.is_empty());
        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert!(
            matches!(
                refusal(&refused),
                RestoreRefusal::EarlierVersionNotYet { .. }
            ),
            "{refused:?}"
        );
        assert_eq!(fs::read(&reapack).unwrap(), b"reapack 1.2.5");
    }

    /// The manifest is read again under the lock. An uninstall still at
    /// work that moved another file of the package into the set after the
    /// restore first read it stops the restore, in a dry run and for real,
    /// with nothing changed: from the old manifest, it would have put back
    /// the first file alone and counted the package as installed.
    #[test]
    fn a_set_that_changed_before_the_lock_is_not_restored_in_part() {
        for dry_run in [true, false] {
            let dir = tempdir().unwrap();
            let resource = dir.path().join("REAPER");
            let first = resource.join("UserPlugins").join("reaper_sws-x64.dll");
            let second = resource.join("UserPlugins").join("sws_python64.py");
            write(&first, b"sws");
            write(&second, b"python");
            let mut writer = BackupSetWriter::new(&resource, "uninstall-unix-1", "uninstall");
            let record = BackupPackageRecord {
                receipt: Some(receipt_entry("sws")),
                langpack_before: None,
                with_settings: None,
            };
            writer.record_package("sws", record).unwrap();
            writer.move_file(Some("sws"), &first, None, false).unwrap();
            let mut since = InstallState::default();
            since.declined_packages.insert("sws".to_string());
            save_install_state(&resource, &since).unwrap();
            let receipt = resource.join("RABBIT").join("install-state.json");
            let records = fs::read(&receipt).unwrap();
            let options = RestoreBackupOptions { dry_run, ..APPLY };

            let error = restore_backup_set_with(
                &resource,
                "uninstall-unix-1",
                &options,
                &BackupEnvironment::AS_USER,
                || {
                    // Called after the first read, before the lock.
                    writer.move_file(Some("sws"), &second, None, false).unwrap();
                    OutsideRestoreRules::default()
                },
            )
            .unwrap_err();

            assert!(
                matches!(refusal(&error), RestoreRefusal::ChangedWhileRead { .. }),
                "{dry_run}: {error:?}"
            );
            assert!(!first.exists() && !second.exists(), "{dry_run}");
            assert_eq!(fs::read(&receipt).unwrap(), records, "{dry_run}");
            assert!(!resource.join("RABBIT").join("logs").exists(), "{dry_run}");
        }
    }

    /// A package left out of a whole-set restore, with none of its files in
    /// the set, never stops the rest, even where its files couldn't go
    /// back: here the JAWS scripts' only entry was saved and its copy never
    /// made, and JAWS's settings folder is gone since. OSARA comes back,
    /// and the report names the scripts and where their file would go.
    #[test]
    fn a_package_left_out_never_stops_the_rest_where_its_files_cant_go_back() {
        let dir = tempdir().unwrap();
        let appdata = dir.path().join("AppData");
        let resource = appdata.join("REAPER");
        let osara = resource.join("UserPlugins").join("reaper_osara64.dll");
        write(&osara, b"osara");
        let script = script_file(&appdata, "reaper.jss");
        write(&script, b"script");
        let mut writer = BackupSetWriter::new(&resource, "uninstall-unix-1", "uninstall");
        for id in ["osara", "scripts"] {
            let record = BackupPackageRecord {
                receipt: Some(receipt_entry(id)),
                langpack_before: None,
                with_settings: None,
            };
            writer.record_package(id, record).unwrap();
        }
        writer
            .move_file(Some("osara"), &osara, None, false)
            .unwrap();
        // RABBIT was ended right after it saved the scripts' entry.
        writer.begin_entry(Some("scripts"), &script).unwrap();
        fs::remove_dir_all(appdata.join("Freedom Scientific")).unwrap();
        let rules = scripts_rules(&appdata);

        for options in [
            RestoreBackupOptions {
                dry_run: true,
                ..APPLY
            },
            APPLY,
        ] {
            let report =
                restore_backup_set_within(&resource, "uninstall-unix-1", &options, &rules).unwrap();

            let restored: Vec<&str> = report
                .packages
                .iter()
                .map(|package| package.package_id.as_str())
                .collect();
            assert_eq!(restored, ["osara"]);
            assert_eq!(
                report.skipped_packages,
                [SkippedPackage {
                    package_id: "scripts".to_string(),
                    reason: SkippedPackageReason::NothingOfItInSet,
                }]
            );
            assert_eq!(report.skipped_targets, std::slice::from_ref(&script));
            assert_eq!(report.actions.len(), 1);
        }
        assert_eq!(fs::read(&osara).unwrap(), b"osara");
        assert!(!appdata.join("Freedom Scientific").exists());
        let state = load_install_state(&resource).unwrap().unwrap();
        assert!(state.packages.contains_key("osara"));
        assert!(!state.packages.contains_key("scripts"));
    }

    /// A set whose manifest names one place twice is refused before
    /// anything changes, in a dry run too: the second file would replace
    /// the first, and the restore's backup of what it replaced would then
    /// lose the file that was there before. On Windows the same holds for
    /// two names that differ only in case.
    #[test]
    fn two_files_for_one_place_stop_the_restore() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins").join("reaper_demo.dll");
        write(&plugin, b"current");
        let set = resource.join("RABBIT/backups/unix-1");
        let relative = Path::new("UserPlugins").join("reaper_demo.dll");
        let copy = set.join(&relative);
        write(&copy, b"older");
        let entry = legacy_entry(None, &relative, &copy);
        save_manifest(&set, RESTORABLE_REASON, vec![entry.clone(), entry]);
        #[cfg(windows)]
        let sets = {
            let other_case = Path::new("userplugins").join("REAPER_DEMO.DLL");
            let set = resource.join("RABBIT/backups/unix-2");
            let first = set.join(&relative);
            write(&first, b"older");
            let second = set.join(&other_case);
            save_manifest(
                &set,
                RESTORABLE_REASON,
                vec![
                    legacy_entry(None, &relative, &first),
                    legacy_entry(None, &other_case, &second),
                ],
            );
            ["unix-1", "unix-2"]
        };
        #[cfg(not(windows))]
        let sets = ["unix-1"];

        for set_id in sets {
            let error = plan_refused(dir.path(), &resource, set_id);
            assert!(
                matches!(
                    refusal(&error),
                    RestoreRefusal::TargetRefused {
                        reason: TargetRefusal::TwoFilesForOnePlace,
                        ..
                    }
                ),
                "{set_id}: {error:?}"
            );
        }
        assert_eq!(fs::read(&plugin).unwrap(), b"current");
    }

    /// The writer never copies over a file its set already holds, inside
    /// the resource folder too: the first copy is the file as it was
    /// before the run. Copying the same file again keeps that first copy
    /// and goes on, so an update that replaces a file twice doesn't fail.
    /// On Windows a name in another case is the same place. A copy without
    /// an entry at the place is refused, never replaced.
    #[test]
    fn the_writer_never_copies_over_a_file_the_set_holds() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugin = resource.join("UserPlugins").join("reaper_demo.dll");
        write(&plugin, b"before");
        let mut writer = BackupSetWriter::new(&resource, "unix-1", RESTORABLE_REASON);
        let copy = writer.copy_file(None, &plugin).unwrap();
        fs::write(&plugin, b"between").unwrap();

        // The same file once more, as an update that replaces it twice: the
        // set keeps its first copy, the file as it was before the run.
        let mut again = vec![plugin.clone()];
        if cfg!(windows) {
            again.push(resource.join("userplugins").join("REAPER_DEMO.DLL"));
        }
        for original in again {
            match writer.copy_file(None, &original) {
                Ok(kept) => assert!(
                    super::same_place(&kept) == super::same_place(&copy),
                    "{kept:?}"
                ),
                // A spelling the set can't match to its first copy is
                // refused rather than copied over it.
                Err(RabbitError::BackupSetWriteRefused { reason, .. }) => {
                    assert!(reason.contains("already holds a file at this path"))
                }
                Err(error) => panic!("{}: {error:?}", original.display()),
            }
            assert_eq!(fs::read(&copy).unwrap(), b"before");
        }
        let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].backup_path, copy);

        // A place that holds a file the set doesn't name, such as a copy
        // whose entry went, is never copied over.
        let other = resource.join("UserPlugins").join("other.dll");
        write(&other, b"other now");
        let stray = writer.path().join("UserPlugins").join("other.dll");
        fs::write(&stray, b"left behind").unwrap();
        let error = writer.copy_file(None, &other).unwrap_err();
        assert!(
            matches!(&error, RabbitError::BackupSetWriteRefused { reason, .. }
                if reason.contains("already holds a file at this path")),
            "{error:?}"
        );
        assert_eq!(fs::read(&stray).unwrap(), b"left behind");
        let manifest = load_backup_manifest(writer.path()).unwrap().unwrap();
        assert_eq!(manifest.files.len(), 1);
    }

    /// Inside the resource folder, a restore never writes through a link
    /// that leads into RABBIT's folder, where `Data/r/install-state.json`
    /// would be RABBIT's records, nor through one that leads out of the
    /// resource folder, nor over a link at the file's own place. Each is
    /// refused before anything changes, and nothing is written where a
    /// link leads. A link that stays inside the resource folder, outside
    /// RABBIT's, is followed, as uninstall follows it.
    #[test]
    fn restore_never_goes_through_a_link_into_rabbit_s_folder_or_out_of_the_resource_folder() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let receipt = resource.join("RABBIT").join("install-state.json");
        write(&receipt, b"current records");
        let elsewhere = dir.path().join("Elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let shared_folder = resource.join("Shared");
        fs::create_dir_all(&shared_folder).unwrap();
        let data = resource.join("Data");
        fs::create_dir_all(&data).unwrap();
        link_folder(&data.join("r"), &resource.join("RABBIT"));
        link_folder(&data.join("out"), &elsewhere);
        link_folder(&data.join("in"), &shared_folder);
        fs::create_dir_all(resource.join("UserPlugins")).unwrap();
        link_folder(&resource.join("UserPlugins").join("x.dll"), &elsewhere);
        let set_holding = |set_id: &str, original: &str| {
            let set = resource.join("RABBIT/backups").join(set_id);
            let relative: PathBuf = original.split('/').collect();
            let copy = set.join(&relative);
            write(&copy, b"old");
            save_manifest(
                &set,
                RESTORABLE_REASON,
                vec![legacy_entry(None, &relative, &copy)],
            );
        };

        for (set_id, original, why) in [
            (
                "unix-1",
                "Data/r/install-state.json",
                "leads-into-rabbit-folder",
            ),
            ("unix-2", "Data/out/x.txt", "leads-outside-resource-folder"),
            ("unix-3", "UserPlugins/x.dll", "place-is-link"),
        ] {
            set_holding(set_id, original);
            let error = plan_refused(dir.path(), &resource, set_id);
            assert!(
                matches!(refusal(&error), RestoreRefusal::TargetRefused { reason, .. }
                    if reason.code() == why),
                "{set_id}: {error:?}"
            );
        }
        assert_eq!(fs::read(&receipt).unwrap(), b"current records");
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);

        set_holding("unix-4", "Data/in/y.txt");
        restore_backup_set_within(&resource, "unix-4", &APPLY, &OutsideRestoreRules::default())
            .unwrap();
        assert_eq!(fs::read(shared_folder.join("y.txt")).unwrap(), b"old");
    }

    /// A link at the temporary name a restore writes first is never
    /// written through, wherever it leads: the restore stops on it, before
    /// the file it would have written changes, and the backup isn't
    /// restored. Windows needs extra rights for links to files, so this
    /// runs where they need none.
    #[cfg(unix)]
    #[test]
    fn a_link_at_the_temporary_name_stops_the_restore() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());
        let elsewhere = dir.path().join("elsewhere-kb.ini");
        let temporary = resource.join("reaper-kb.ini.rabbit-restore-tmp");
        std::os::unix::fs::symlink(&elsewhere, &temporary).unwrap();

        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, &OutsideRestoreRules::default())
                .unwrap();

        assert_eq!(report.status, super::RestoreStatus::NotRestored);
        let error = report.error.as_deref().unwrap_or_default();
        assert!(
            error.starts_with(&format!("I/O error at {}: ", temporary.display())),
            "{report:?}"
        );
        let kinds: Vec<RestoreBackupActionKind> =
            report.actions.iter().map(|action| action.action).collect();
        assert_eq!(kinds, [RestoreBackupActionKind::NotRestored]);
        assert_eq!(report.before_restore_set_id, None);
        assert!(fs::symlink_metadata(&elsewhere).is_err());
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
    }

    /// Inside the resource folder, as below the CLAP folder, a name with a
    /// `:` is refused, and so, on Windows, is one that ends in a dot or a
    /// space or names a device, such as `CON` or `COM1.clap`, wherever it
    /// is on the way, also below a CLAP folder that isn't there.
    #[test]
    fn restore_refuses_names_the_file_system_may_not_open_as_written() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let clap_dir = dir.path().join("CLAP");
        let target = |path: &Path| {
            restore_target(&resource, path, None, Some("app2clap"), &shared(&clap_dir))
        };

        let mut refused_paths = vec![
            Path::new("UserPlugins").join("x.dll:hidden"),
            Path::new("UserPlugins:x").join("x.dll"),
        ];
        if cfg!(windows) {
            refused_paths.extend([
                Path::new("UserPlugins").join("x.dll."),
                Path::new("UserPlugins ").join("x.dll"),
                Path::new("UserPlugins").join("nul.dll"),
                PathBuf::from("CONOUT$"),
                clap_dir.join("COM1.clap"),
                clap_dir.join("CON").join("plugin.clap"),
            ]);
        }
        for path in refused_paths {
            let error = target(&path).unwrap_err();
            assert!(
                matches!(
                    refusal(&error),
                    RestoreRefusal::TargetRefused {
                        reason: TargetRefusal::NameNotAsWritten { .. },
                        ..
                    }
                ),
                "{}: {error:?}",
                path.display()
            );
        }
        let plain = Path::new("UserPlugins").join("x.dll");
        assert_eq!(target(&plain).unwrap(), resource.join(&plain));
        assert!(!clap_dir.exists());
    }

    /// Below a vendor pattern's folder, a name Windows would open as
    /// another one is refused before anything changes, in a dry run too.
    /// Here a `\\?\` path names a junction `enu.` beside the real folder
    /// `enu` in JAWS's settings: through a plain path, the check would look
    /// at `enu`, while the file would go through the junction. Nothing is
    /// written where it leads.
    #[cfg(windows)]
    #[test]
    fn restore_refuses_a_name_windows_reads_as_another_under_the_vendor_folder() {
        let dir = tempdir().unwrap();
        let appdata = dir.path().join("AppData");
        let resource = appdata.join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let enu = script_file(&appdata, "reaper.jss")
            .parent()
            .unwrap()
            .to_path_buf();
        fs::create_dir_all(&enu).unwrap();
        let settings = enu.parent().unwrap().to_path_buf();
        let elsewhere = dir.path().join("Elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        // mklink drops the trailing dot, so the junction is made under
        // another name and renamed through a `\\?\` path, which keeps it.
        let junction = PathBuf::from(format!(r"\\?\{}", settings.display())).join("enu.");
        link_folder(&settings.join("Junction"), &elsewhere);
        fs::rename(settings.join("Junction"), &junction).unwrap();
        let set = resource.join("RABBIT/backups/unix-1");
        let copy = set.join("external/1/reaper.jss");
        write(&copy, b"script");
        save_manifest(
            &set,
            RESTORABLE_REASON,
            vec![legacy_entry(
                Some("scripts"),
                &junction.join("reaper.jss"),
                &copy,
            )],
        );
        let before = snapshot(dir.path());

        for dry_run in [true, false] {
            let options = RestoreBackupOptions { dry_run, ..APPLY };
            let error =
                restore_backup_set_within(&resource, "unix-1", &options, &scripts_rules(&appdata))
                    .unwrap_err();
            assert!(
                matches!(refusal(&error), RestoreRefusal::TargetRefused {
                    reason: TargetRefusal::NameNotAsWritten { name },
                    ..
                } if name == "enu."),
                "{dry_run}: {error:?}"
            );
            assert_eq!(snapshot(dir.path()), before, "{dry_run}");
        }
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&enu).unwrap().count(), 0);
        // Only the junction goes, not what it leads to.
        fs::remove_dir(&junction).unwrap();
    }

    /// An id copied with the period after it, or a space, names no set, but
    /// the error names the set it would name without them, on every
    /// platform, and nothing is restored. Without such a set, the id is
    /// refused as before.
    #[test]
    fn an_id_copied_with_the_period_after_it_names_the_set() {
        let dir = tempdir().unwrap();
        let (resource, keymap) = keymap_set(dir.path());

        for id in ["unix-1.", "unix-1 ", "unix-1. ", "unix-1.."] {
            let error = refused(dir.path(), &resource, id);
            assert!(
                matches!(refusal(&error), RestoreRefusal::EndsInDotOrSpace { backup_id, existing }
                    if backup_id == id && existing == "unix-1"),
                "{id:?}: {error:?}"
            );
            let message = error.to_string();
            assert!(message.contains("There is one named unix-1"), "{message}");
            assert!(!message.contains("deleted"), "{message}");
        }
        let error = refused(dir.path(), &resource, "unix-2.");
        if cfg!(windows) {
            assert!(
                matches!(refusal(&error), RestoreRefusal::InvalidId { backup_id: id } if id == "unix-2."),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(refusal(&error), RestoreRefusal::NotFound { backup_id }
                    if backup_id == "unix-2."),
                "{error:?}"
            );
        }
        assert_eq!(fs::read(&keymap).unwrap(), b"osara keymap");
    }
}

#[cfg(test)]
#[path = "restore_run_tests.rs"]
mod restore_run_tests;
