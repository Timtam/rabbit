//! What RABBIT kept in `RABBIT/backups`, as backups a person can get back
//! or delete: the overview behind `rabbit backups`, and later the wizard's
//! Backups tab.
//!
//! A backup set is one folder in `RABBIT/backups`. A backup is one thing
//! that can come back: one package from a set, or one key map. One set can
//! hold several backups. Sets RABBIT can't restore by package, such as a
//! set without a manifest, are one backup each, and every set that holds
//! only a copy of RABBIT's records is part of one backup together.
//!
//! The overview reads each set's manifest, RABBIT's receipt once, the
//! receipt copy in an older update set when it needs one, and file sizes.
//! It never hashes a file: FFmpeg's backups alone are 250 MB, and the
//! wizard lists them on its UI thread. Statuses that need a file's content
//! are judged from the hashes the manifests and the receipt recorded, and
//! from sizes on disk; a restore checks the files themselves when it runs.
//!
//! It never follows a symbolic link or junction: not `RABBIT`, not
//! `RABBIT/backups` or `RABBIT/backups-deleting`, not a set, nothing in it.
//! Outside the sets it looks only at places a restore would write, judged
//! from the paths' text, and at files RABBIT's receipt records: a set
//! could name a path on another computer, which Windows would connect to.

use std::cell::OnceCell;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::error::{IoPathContext, RabbitError, Result};
use crate::receipt::{InstallState, PackageReceipt, load_install_state};
use crate::restore_refusal::{RestoreRefusal, TargetRefusal};
use crate::rollback::{
    BACKUP_MANIFEST_FILE, BEFORE_RESTORE_REASON, BackupEntryState, BackupEnvironment,
    BackupManifest, BackupManifestFile, BackupSet, DELETING_DIR, EntryPlace, KEYMAP_BACKUP_REASON,
    KeymapOrigin, OutsideRestoreRules, RECORDS_BACKUP_REASON, UPDATE_BACKUP_REASON,
    backup_id_is_well_formed, backups_root, copy_in_set, entry_place, find_backup_set,
    inside_place_by_text, is_outside_file_in_layout, is_real_file, is_real_folder, is_receipt,
    is_receipt_entry, is_set_bookkeeping, load_backup_manifest, manifest_holds_receipt,
    manifest_refusal, restore_place_by_text, same_place, set_to_restore,
};
use crate::uninstall::execute::UNINSTALL_BACKUP_REASON;
use crate::version::Version;

/// The key map's name in the resource folder. A set entry whose original
/// is this file is a key map, whatever made the set.
const KEYMAP_FILE: &str = "reaper-kb.ini";

/// What made a backup set, as the overview reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupSetKind {
    /// An uninstall moved packages' files here (`uninstall`).
    Uninstall,
    /// An update kept the files it replaced here (`install-replacement`):
    /// earlier versions.
    Update,
    /// OSARA's key map replaced the key map this set holds
    /// (`osara-keymap-replacement`).
    Keymap,
    /// A restore kept the files it replaced here (`before-restore`).
    BeforeRestore,
    /// Only a copy of RABBIT's receipt, which older versions made after
    /// vendor installers ran (`unattended-receipt-update`).
    Records,
    /// A manifest with a reason this version doesn't know, such as one a
    /// later RABBIT wrote.
    Unknown,
    /// A set without a manifest, as an older RABBIT that stopped early
    /// left one.
    NoManifest,
    /// A set whose manifest can't be read.
    Damaged,
    /// A set whose deletion started and didn't finish, in
    /// `RABBIT/backups-deleting`.
    Deleting,
    /// A symbolic link or junction in `RABBIT/backups`, or a folder there
    /// whose name RABBIT never gives a set. Never looked into.
    NotABackup,
}

/// The groups the overview sorts backups into, in the order they are
/// listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupGroup {
    /// Packages an uninstall moved into a set.
    UninstalledSoftware,
    /// Packages' files an update replaced, or a restore of an earlier
    /// version did.
    EarlierVersions,
    /// Key maps OSARA's key map, or a restore, replaced.
    Keymaps,
    /// Everything RABBIT can't restore by package or as a key map.
    Other,
}

/// Which backup an entry of the overview is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BackupItem {
    /// One package in a set in `RABBIT/backups`.
    Package { set_id: String, package_id: String },
    /// The key map a set in `RABBIT/backups` holds.
    Keymap { set_id: String },
    /// A whole set in `RABBIT/backups`: a set RABBIT can't take apart by
    /// package, a set with files no other backup of it holds, or a link or
    /// a badly named folder there.
    Set { set_id: String },
    /// An unfinished deletion in `RABBIT/backups-deleting`.
    Deleting { set_id: String },
    /// Every set that holds only a copy of RABBIT's records, newest first.
    Records { set_ids: Vec<String> },
}

impl BackupItem {
    /// The one set the backup is in. `None` for the records, which span
    /// several.
    pub fn set_id(&self) -> Option<&str> {
        match self {
            Self::Package { set_id, .. }
            | Self::Keymap { set_id }
            | Self::Set { set_id }
            | Self::Deleting { set_id } => Some(set_id),
            Self::Records { .. } => None,
        }
    }

    /// The package the backup is of, if it is one package's.
    pub fn package_id(&self) -> Option<&str> {
        match self {
            Self::Package { package_id, .. } => Some(package_id),
            _ => None,
        }
    }
}

/// Whether a backup can be restored, and if not, why.
///
/// For earlier versions, the statuses follow the rules of the swap that
/// changes only the restored package's receipt entry (backup plan 6.4).
/// `restore_backup_set` still refuses an update's set as a whole until that
/// swap lands, so such a backup says it can be restored while
/// [`BackupEntry::restore_scope`] stays `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupStatus {
    CanRestore,
    /// An uninstall's backup whose package RABBIT's receipt still lists as
    /// recorded, while some of its files are only in the set: the uninstall
    /// stopped partway through it. It can be restored.
    StoppedPartway,
    /// RABBIT's receipt has the entry this backup recorded, and its files
    /// are on disk with the sizes it recorded. Restoring puts them back
    /// again; deleting loses nothing.
    AlreadyBack,
    /// RABBIT's receipt has another entry for the package: it was installed
    /// again since the uninstall.
    InstalledAgain,
    /// An earlier version a file of which is neither in the set nor on disk
    /// as the receipt records it with that version's hash.
    NotComplete,
    /// A package record with none of its files in the set.
    NothingInIt,
    /// An update's backup that records no earlier entry for the package
    /// and holds no copy of RABBIT's receipt to read it from.
    VersionUnknown,
    /// An earlier version of a package while another of its exclusive group
    /// is installed, whose install removed it. Backup plan 3.4 has this for
    /// language packs, but they no longer form such a group, so it applies
    /// only to a package whose manifest declares one.
    OtherLanguagePack,
    /// A finished copy the manifest names is gone from the set, or has
    /// another size than recorded.
    PartMissing,
    /// RABBIT's receipt can't be read, so RABBIT can't tell.
    RecordsUnreadable,
    /// A backup RABBIT can't restore by package or as a key map: a set
    /// without a manifest, a damaged one, another restore's backup, an
    /// unfinished deletion, or copies of RABBIT's records. The CLI can
    /// restore some of them as a whole (see [`BackupEntry::restore_scope`]).
    NotHere,
    /// The set belongs to another account than `RABBIT/backups` does, as
    /// one made under `sudo` does on a Mac. RABBIT neither restores nor
    /// deletes it.
    NotYours,
    /// A link or junction in `RABBIT/backups`, or a folder whose name
    /// RABBIT never gives a set.
    NotABackup,
}

impl BackupStatus {
    /// Whether the backup can be restored, by its status alone.
    pub fn can_restore(self) -> bool {
        matches!(
            self,
            Self::CanRestore | Self::StoppedPartway | Self::AlreadyBack
        )
    }
}

/// How `restore_backup_set` takes a backup today: one package of an
/// uninstall's set (`--package`), or a whole set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestoreScope {
    Package,
    WholeSet,
}

/// One backup in the overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupEntry {
    pub item: BackupItem,
    pub group: BackupGroup,
    /// What made the set it is in. For the records, `records`.
    pub set_kind: BackupSetKind,
    /// When its set was made, in seconds since 1970 (UTC), when RABBIT can
    /// tell. For the records, when the newest of their sets was made.
    pub created_unix: Option<u64>,
    /// Bytes. For a package or a key map, the sizes its manifest entries
    /// record. For a backup that stands for whole sets, the sets' sizes on
    /// disk, which is what deleting them frees: a set without a readable
    /// manifest has no entries to add up.
    pub size: u64,
    /// The package's version in it, from its receipt entry: the one an
    /// uninstall recorded, the one an update replaced, or the one in an
    /// older update set's copy of RABBIT's receipt. `None` when RABBIT
    /// didn't install that copy or can't tell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<Version>,
    /// For an earlier version, the version that replaced it. Only sets
    /// written from this version on record it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<Version>,
    /// For uninstalled software, whether the uninstall moved its settings
    /// into the set. `None` in sets from earlier versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_settings: Option<bool>,
    /// For a key map, what it was, when the set recorded it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keymap_origin: Option<KeymapOrigin>,
    pub status: BackupStatus,
    /// For [`BackupStatus::PartMissing`] and [`BackupStatus::NotComplete`],
    /// the first file that makes it so, by its original path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_path: Option<PathBuf>,
    /// A file a restore would refuse to put back, which stops the whole
    /// restore: one in RABBIT's own folder, one by a name that may open
    /// another file than it names, one outside every folder RABBIT installs
    /// into, or a second file for one place. By its original path, or its
    /// place in a set without a manifest; for a second file for one place,
    /// by that place. Set only when it is why [`Self::restore_scope`] is
    /// `None`. [`Self::refusal`] says why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused_path: Option<PathBuf>,
    /// Whether this is the only copy of what it holds: uninstalled software
    /// and key maps. Uninstalled software that is already back is not, so
    /// deleting it loses nothing.
    pub only_copy: bool,
    /// The other backups in the same set. Deleting takes whole sets, so
    /// they go together.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shares_set_with: Vec<BackupItem>,
    /// How `rabbit restore-backup` takes this backup today, if it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore_scope: Option<RestoreScope>,
    /// Why a restore refuses this backup, the refusal a run gives for the
    /// same reason, where the overview can tell: a file it would refuse to
    /// put back (see [`Self::refused_path`]); the statuses a run refuses
    /// for (installed again, nothing in it, part missing, not complete,
    /// another of its group installed, RABBIT's records unreadable, another
    /// account's); and for a whole set, what refuses the set itself (being
    /// deleted, a damaged manifest, an update's set, a copy of RABBIT's
    /// records in it, a link in a set without a manifest, or a file from
    /// outside the resource folder there). `None` for the other statuses,
    /// which say on their own why the backup can't be restored, or that it
    /// can, and for the backup of RABBIT's records, which stands for many
    /// sets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RestoreRefusal>,
}

/// What a later run compares to tell whether a set changed since it was
/// listed: the manifest's length and modified time, or without a manifest
/// the number of files and their size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SetFingerprint {
    Manifest {
        length: u64,
        modified: Option<SystemTime>,
    },
    Contents {
        file_count: u64,
        size: u64,
    },
}

/// One set in the overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSetOverview {
    pub id: String,
    /// In `RABBIT/backups`, or in `RABBIT/backups-deleting` for an
    /// unfinished deletion.
    pub path: PathBuf,
    pub kind: BackupSetKind,
    pub created_unix: Option<u64>,
    /// Bytes on disk, every file and link counted and none followed: what
    /// deleting the set frees.
    pub size: u64,
    /// Every regular file in the set, the manifest and notes included.
    pub file_count: u64,
    /// Manifest entries whose copy never finished.
    pub pending_entries: usize,
    /// The set belongs to another account than `RABBIT/backups` (Unix).
    pub owned_by_another_account: bool,
    /// `None` for an entry that is no backup, which is never looked into.
    pub fingerprint: Option<SetFingerprint>,
    pub manifest: Option<BackupManifest>,
    pub manifest_path: Option<PathBuf>,
    /// Where in the set a copy of RABBIT's receipt is, as older versions
    /// made one.
    pub receipt_copy: Option<PathBuf>,
    /// Whether a restore would find a copy of RABBIT's receipt in the set,
    /// and so refuse it.
    pub holds_receipt: bool,
    /// The backed-up files, relative to the set: not the manifest, its
    /// staging file or the origin notes, and no links.
    pub files: Vec<PathBuf>,
    /// Symbolic links and junctions in the set, by their path in it.
    pub links: Vec<PathBuf>,
    /// What couldn't be read, as `rabbit backups` says it.
    pub warning: Option<String>,
}

/// A set, or something in RABBIT's backup folders, the overview couldn't
/// read whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnreadableSet {
    pub id: String,
    pub path: PathBuf,
    pub reason: String,
}

/// Everything RABBIT kept for one REAPER, as sets and as backups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupOverview {
    pub resource_path: PathBuf,
    /// Newest first; sets without a date last.
    pub sets: Vec<BackupSetOverview>,
    /// By group (see [`BackupGroup`]), newest first inside each, backups
    /// without a date last, and the records last of all.
    pub entries: Vec<BackupEntry>,
    /// Bytes on disk of every set.
    pub total_size: u64,
    pub unreadable: Vec<UnreadableSet>,
    /// Why RABBIT's receipt couldn't be read, when it couldn't.
    pub receipt_error: Option<String>,
    /// The files a restore of each backup would write, for those that
    /// write any (see [`Self::restore_targets`]).
    targets: BTreeMap<BackupItem, Vec<PathBuf>>,
}

impl BackupOverview {
    /// The sets as `rabbit backups --json` prints them, each with its
    /// backups.
    pub fn backup_sets(&self) -> Vec<BackupSet> {
        self.sets
            .iter()
            .map(|set| {
                let mut files: Vec<PathBuf> = set.files.iter().chain(&set.links).cloned().collect();
                files.sort();
                BackupSet {
                    id: set.id.clone(),
                    path: set.path.clone(),
                    created_at: set
                        .manifest
                        .as_ref()
                        .map(|manifest| manifest.created_at.clone()),
                    reason: set
                        .manifest
                        .as_ref()
                        .map(|manifest| manifest.reason.clone()),
                    manifest_path: set.manifest_path.clone(),
                    files,
                    warning: set.warning.clone(),
                    kind: Some(set.kind),
                    created_unix: set.created_unix,
                    size: set.size,
                    items: self.entries_in(set).cloned().collect(),
                    fingerprint: set.fingerprint.clone(),
                }
            })
            .collect()
    }

    /// The backups in `set`, one of [`Self::sets`], in the order of
    /// [`Self::entries`]. The records are in each of their sets.
    pub fn entries_in<'a>(
        &'a self,
        set: &'a BackupSetOverview,
    ) -> impl Iterator<Item = &'a BackupEntry> + 'a {
        self.entries
            .iter()
            .filter(move |entry| entry_is_in(entry, set))
    }

    /// The files a restore of `item` would write, read from the manifests
    /// when the overview was made, for telling which backups write the same
    /// file. Nothing is hashed or read again. Empty for a backup RABBIT
    /// never restores (a damaged set, an unfinished deletion, the records,
    /// another account's set, or what is no backup), for one a restore would
    /// refuse a file of, and for one this overview doesn't list.
    pub fn restore_targets(&self, item: &BackupItem) -> Vec<PathBuf> {
        self.targets.get(item).cloned().unwrap_or_default()
    }
}

/// The overview of everything in `RABBIT/backups` and
/// `RABBIT/backups-deleting` of the REAPER at `resource_path`.
///
/// When `RABBIT`, `RABBIT/backups` or `RABBIT/backups-deleting` is a
/// symbolic link or junction, nothing is listed and the error says why, as
/// a restore refuses every set then. A REAPER without those folders has no
/// backups, and gets no folder from looking.
pub fn backup_overview(resource_path: &Path) -> Result<BackupOverview> {
    overview_with(resource_path, &BackupEnvironment::system())
}

/// [`backup_overview`], asking `environment` who owns each set.
pub(crate) fn overview_with(
    resource_path: &Path,
    environment: &BackupEnvironment<'_>,
) -> Result<BackupOverview> {
    let (folders, unreadable) = set_folders(resource_path)?;
    Ok(build_overview(
        resource_path,
        folders,
        unreadable,
        environment,
    ))
}

/// The overview of the one set `backup_id` names, found the way a restore
/// finds it: a well-formed id that equals, byte for byte, a folder in
/// `RABBIT/backups`, `RABBIT/backups-deleting`, or both. Errors as a
/// restore's lookup does, except that an unfinished deletion is listed.
pub fn backup_overview_of(resource_path: &Path, backup_id: &str) -> Result<BackupOverview> {
    overview_of_with(resource_path, backup_id, &BackupEnvironment::system())
}

/// [`backup_overview_of`], asking `environment` who owns the set.
pub(crate) fn overview_of_with(
    resource_path: &Path,
    backup_id: &str,
    environment: &BackupEnvironment<'_>,
) -> Result<BackupOverview> {
    let found = match find_backup_set(resource_path, backup_id) {
        Ok(found) => found,
        Err(RabbitError::RestoreRefused(RestoreRefusal::InvalidId { .. })) => {
            return Err(lookup_error(resource_path, backup_id));
        }
        Err(error) => return Err(error),
    };
    let mut folders = Vec::new();
    for (path, deleting) in [(found.set, false), (found.deleting, true)] {
        if let Some(path) = path {
            folders.push(SetFolder {
                id: backup_id.to_string(),
                path,
                deleting,
                not_a_set: None,
            });
        }
    }
    if folders.is_empty() {
        return Err(lookup_error(resource_path, backup_id));
    }
    Ok(build_overview(
        resource_path,
        folders,
        Vec::new(),
        environment,
    ))
}

/// Why `backup_id` names no set, as a restore says it: not well-formed,
/// with a dot or space too many at its end, in another case than a set's,
/// or deleted or never there.
fn lookup_error(resource_path: &Path, backup_id: &str) -> RabbitError {
    match set_to_restore(resource_path, backup_id) {
        Err(error) => error,
        Ok(_) => RestoreRefusal::NotFound {
            backup_id: backup_id.to_string(),
        }
        .into(),
    }
}

/// The files a restore of `item` from the REAPER at `resource_path` would
/// write: [`BackupOverview::restore_targets`] of the overview of its one
/// set, read from its manifest, or without one from its layout. Nothing is
/// hashed. The set is found the way a restore finds it, and errors the same
/// way. Empty for an unfinished deletion and for the records, which RABBIT
/// never restores.
pub fn backup_restore_targets(resource_path: &Path, item: &BackupItem) -> Result<Vec<PathBuf>> {
    restore_targets_with(resource_path, item, &BackupEnvironment::system())
}

/// [`backup_restore_targets`], asking `environment` about the computer.
pub(crate) fn restore_targets_with(
    resource_path: &Path,
    item: &BackupItem,
    environment: &BackupEnvironment<'_>,
) -> Result<Vec<PathBuf>> {
    let (BackupItem::Package { set_id, .. }
    | BackupItem::Keymap { set_id }
    | BackupItem::Set { set_id }) = item
    else {
        return Ok(Vec::new());
    };
    set_to_restore(resource_path, set_id)?;
    Ok(overview_of_with(resource_path, set_id, environment)?.restore_targets(item))
}

/// The time a set id RABBIT gives holds, in seconds since 1970, for every
/// form an id takes: `unix-<s>` (an update), `unattended-unix-<s>-<ns>`
/// with nine digits of nanoseconds (a key map, or in older versions a copy
/// of the records), `uninstall-unix-<s>`, and
/// `restore-unix-<s>-before-restore`, each also with the `-2`, `-3` and so
/// on that a set gets when its id was taken. `None` for any other name.
pub fn seconds_in_set_id(id: &str) -> Option<u64> {
    enum Tail {
        Nothing,
        Nanoseconds,
        BeforeRestore,
    }
    let forms = [
        ("unattended-unix-", Tail::Nanoseconds),
        ("uninstall-unix-", Tail::Nothing),
        ("restore-unix-", Tail::BeforeRestore),
        ("unix-", Tail::Nothing),
    ];
    let (rest, tail) = forms
        .into_iter()
        .find_map(|(prefix, tail)| Some((id.strip_prefix(prefix)?, tail)))?;
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let seconds = rest[..digits].parse::<u64>().ok()?;
    let rest = &rest[digits..];
    let rest = match tail {
        Tail::Nothing => rest,
        Tail::Nanoseconds => {
            let nanoseconds = rest.strip_prefix('-')?;
            let digits = nanoseconds.get(..9)?;
            if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            &nanoseconds[9..]
        }
        Tail::BeforeRestore => rest.strip_prefix("-before-restore")?,
    };
    let numbered = |suffix: &str| {
        suffix.strip_prefix('-').is_some_and(|number| {
            !number.is_empty()
                && !number.starts_with('0')
                && number.bytes().all(|byte| byte.is_ascii_digit())
        })
    };
    (rest.is_empty() || numbered(rest)).then_some(seconds)
}

/// A size as whole KB or MB, or GB with one decimal, in steps of 1024,
/// rounded to the nearest. Below 1 KB it is "under 1 KB". A size that
/// rounds up to 1024 of a unit counts in the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupSize {
    UnderOneKb,
    Kb(u64),
    Mb(u64),
    Gb { whole: u64, tenth: u64 },
}

impl BackupSize {
    pub fn of(bytes: u64) -> Self {
        const STEP: u128 = 1024;
        let bytes = u128::from(bytes);
        let rounded = |unit: u128| (bytes + unit / 2) / unit;
        let whole = |value: u128| u64::try_from(value).unwrap_or(u64::MAX);
        if bytes < STEP {
            return Self::UnderOneKb;
        }
        let kb = rounded(STEP);
        if kb < STEP {
            return Self::Kb(whole(kb));
        }
        let mb = rounded(STEP * STEP);
        if mb < STEP {
            return Self::Mb(whole(mb));
        }
        let gb = STEP * STEP * STEP;
        let tenths = (bytes * 10 + gb / 2) / gb;
        Self::Gb {
            whole: whole(tenths / 10),
            tenth: whole(tenths % 10),
        }
    }
}

/// One entry of RABBIT's backup folders the overview reads.
struct SetFolder {
    id: String,
    path: PathBuf,
    /// In `RABBIT/backups-deleting`.
    deleting: bool,
    /// Why it is no set, for a link or a badly named folder in
    /// `RABBIT/backups`. It is never looked into.
    not_a_set: Option<&'static str>,
}

const LINK_IS_NO_SET: &str = "this is a symbolic link or junction, not a backup RABBIT made, and \
                              RABBIT never looks inside it";
const NAME_IS_NO_SET: &str = "its name is not one RABBIT gives a backup, so it is not a backup \
                              RABBIT made, and RABBIT never looks inside it or restores it";

/// Every entry in `RABBIT/backups` and `RABBIT/backups-deleting`, without
/// following a link, and what in them can't be a set. Errors when
/// `RABBIT`, either folder or an entry can't be read, or one of the three
/// is a link.
fn set_folders(resource_path: &Path) -> Result<(Vec<SetFolder>, Vec<UnreadableSet>)> {
    let mut folders = Vec::new();
    let mut unreadable = Vec::new();
    let rabbit = resource_path.join("RABBIT");
    if !is_real_folder(&rabbit)? {
        return Ok((folders, unreadable));
    }
    for (folder, deleting) in [
        (backups_root(resource_path), false),
        (rabbit.join(DELETING_DIR), true),
    ] {
        if !is_real_folder(&folder)? {
            continue;
        }
        for entry in fs::read_dir(&folder).with_path(&folder)? {
            let entry = entry.with_path(&folder)?;
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let Some(id) = entry.file_name().to_str().map(ToString::to_string) else {
                unreadable.push(UnreadableSet {
                    id: entry.file_name().to_string_lossy().into_owned(),
                    path,
                    reason: NAME_IS_NO_SET.to_string(),
                });
                continue;
            };
            let not_a_set = if kind.is_symlink() {
                Some(LINK_IS_NO_SET)
            } else if !kind.is_dir() {
                continue;
            } else if !backup_id_is_well_formed(&id) {
                Some(NAME_IS_NO_SET)
            } else {
                None
            };
            // Only delete puts sets into `backups-deleting`, by their ids.
            // Anything else there is no set and is never looked into.
            if let (true, Some(why)) = (deleting, not_a_set) {
                unreadable.push(UnreadableSet {
                    id,
                    path,
                    reason: why.to_string(),
                });
                continue;
            }
            folders.push(SetFolder {
                id,
                path,
                deleting,
                not_a_set,
            });
        }
    }
    Ok((folders, unreadable))
}

/// What a walk through one set found, without following any link.
#[derive(Debug, Default)]
struct SetWalk {
    size: u64,
    file_count: u64,
    files: Vec<PathBuf>,
    links: Vec<PathBuf>,
    error: Option<String>,
}

/// Walk the set at `set` by each entry's own metadata: a link or junction
/// is counted and listed by name, never followed, and no file is opened. A
/// folder or entry that can't be read is noted, and the walk goes on.
fn walk_set(set: &Path) -> SetWalk {
    // The first error is the one said.
    fn note(walk: &mut SetWalk, error: RabbitError) {
        walk.error.get_or_insert_with(|| error.to_string());
    }
    let mut walk = SetWalk::default();
    let mut folders = vec![set.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) => {
                note(&mut walk, crate::error::io_error_at(folder, error));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    note(&mut walk, crate::error::io_error_at(folder.clone(), error));
                    continue;
                }
            };
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(set).map(Path::to_path_buf) else {
                continue;
            };
            // The entry's own type and size: unlike `Path::is_dir` and
            // `fs::metadata`, they never follow a link. Rust reports a
            // junction as a link on Windows.
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(error) => {
                    note(&mut walk, crate::error::io_error_at(path, error));
                    continue;
                }
            };
            if kind.is_dir() {
                folders.push(path);
                continue;
            }
            match entry.metadata() {
                Ok(metadata) => walk.size += metadata.len(),
                Err(error) => note(&mut walk, crate::error::io_error_at(path, error)),
            }
            if kind.is_symlink() {
                walk.links.push(relative);
            } else if kind.is_file() {
                walk.file_count += 1;
                if !is_set_bookkeeping(&relative) {
                    walk.files.push(relative);
                }
            }
        }
    }
    walk.files.sort();
    walk.links.sort();
    walk
}

/// Read every set in `folders` and turn them into backups.
fn build_overview(
    resource_path: &Path,
    folders: Vec<SetFolder>,
    unreadable: Vec<UnreadableSet>,
    environment: &BackupEnvironment<'_>,
) -> BackupOverview {
    let rabbit = resource_path.join("RABBIT");
    let backups = backups_root(resource_path);
    // Sets are compared with the owner of `RABBIT/backups`, not with the
    // account RABBIT runs as, so the overview says the same under sudo.
    let reference_owner = if is_real_folder(&backups).unwrap_or(false) {
        (environment.owner_of)(&backups)
    } else {
        (environment.owner_of)(&rabbit)
    };
    let mut unreadable = unreadable;
    let mut sets: Vec<BackupSetOverview> = folders
        .into_iter()
        .map(|folder| {
            let set = read_set(resource_path, folder, environment, reference_owner);
            if let (Some(warning), false) = (&set.warning, set.kind == BackupSetKind::NotABackup) {
                unreadable.push(UnreadableSet {
                    id: set.id.clone(),
                    path: set.path.clone(),
                    reason: warning.clone(),
                });
            }
            set
        })
        .collect();
    sets.sort_by(|left, right| {
        newest_first(left.created_unix)
            .cmp(&newest_first(right.created_unix))
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| {
                (left.kind == BackupSetKind::Deleting).cmp(&(right.kind == BackupSetKind::Deleting))
            })
    });

    let receipt = load_install_state(resource_path)
        .map(Option::unwrap_or_default)
        .map_err(|error| error.to_string());
    let judge = Judge {
        resource_path,
        environment,
        receipt: &receipt,
        groups: OnceCell::new(),
        outside: OnceCell::new(),
    };
    let mut entries = Vec::new();
    let mut targets = BTreeMap::new();
    let mut records = Vec::new();
    for set in &sets {
        if set.kind == BackupSetKind::Records && !set.owned_by_another_account {
            records.push(set);
            continue;
        }
        for entry in judge.entries_of(set) {
            let writes = judge.targets_of(set, &entry);
            if !writes.is_empty() {
                targets.insert(entry.item.clone(), writes);
            }
            entries.push(entry);
        }
    }
    if !records.is_empty() {
        entries.push(BackupEntry {
            created_unix: records.iter().filter_map(|set| set.created_unix).max(),
            size: records.iter().map(|set| set.size).sum(),
            ..new_entry(
                BackupItem::Records {
                    set_ids: records.iter().map(|set| set.id.clone()).collect(),
                },
                BackupGroup::Other,
                BackupSetKind::Records,
                None,
                BackupStatus::NotHere,
            )
        });
    }
    link_entries_of_one_set(&mut entries);
    entries.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then_with(|| is_records(left).cmp(&is_records(right)))
            .then_with(|| newest_first(left.created_unix).cmp(&newest_first(right.created_unix)))
            .then_with(|| left.item.cmp(&right.item))
    });

    BackupOverview {
        resource_path: resource_path.to_path_buf(),
        total_size: sets.iter().map(|set| set.size).sum(),
        sets,
        entries,
        unreadable,
        receipt_error: receipt.err(),
        targets,
    }
}

/// A key that sorts the newest first and those without a date last.
fn newest_first(created_unix: Option<u64>) -> (bool, Reverse<u64>) {
    (created_unix.is_none(), Reverse(created_unix.unwrap_or(0)))
}

fn is_records(entry: &BackupEntry) -> bool {
    matches!(entry.item, BackupItem::Records { .. })
}

/// Whether `entry` is one of the backups in `set`.
fn entry_is_in(entry: &BackupEntry, set: &BackupSetOverview) -> bool {
    let deleting = set.kind == BackupSetKind::Deleting;
    match &entry.item {
        BackupItem::Deleting { set_id } => deleting && *set_id == set.id,
        BackupItem::Records { set_ids } => !deleting && set_ids.contains(&set.id),
        item => !deleting && item.set_id() == Some(set.id.as_str()),
    }
}

/// Fill in which backups share a set with each other. An unfinished
/// deletion and the records share with nothing.
fn link_entries_of_one_set(entries: &mut [BackupEntry]) {
    let in_set = |item: &BackupItem| match item {
        BackupItem::Package { set_id, .. }
        | BackupItem::Keymap { set_id }
        | BackupItem::Set { set_id } => Some(set_id.clone()),
        BackupItem::Deleting { .. } | BackupItem::Records { .. } => None,
    };
    let mut by_set: BTreeMap<String, Vec<BackupItem>> = BTreeMap::new();
    for entry in entries.iter() {
        if let Some(set_id) = in_set(&entry.item) {
            by_set.entry(set_id).or_default().push(entry.item.clone());
        }
    }
    for entry in entries.iter_mut() {
        if let Some(set_id) = in_set(&entry.item) {
            entry.shares_set_with = by_set[&set_id]
                .iter()
                .filter(|other| **other != entry.item)
                .cloned()
                .collect();
        }
    }
}

/// Read one set: its manifest, a walk through it, its owner. Nothing in a
/// link or a badly named folder is read.
fn read_set(
    resource_path: &Path,
    folder: SetFolder,
    environment: &BackupEnvironment<'_>,
    reference_owner: Option<u32>,
) -> BackupSetOverview {
    let SetFolder {
        id,
        path,
        deleting,
        not_a_set,
    } = folder;
    if let Some(why) = not_a_set {
        return BackupSetOverview {
            id,
            path,
            kind: BackupSetKind::NotABackup,
            created_unix: None,
            size: 0,
            file_count: 0,
            pending_entries: 0,
            owned_by_another_account: false,
            fingerprint: None,
            manifest: None,
            manifest_path: None,
            receipt_copy: None,
            holds_receipt: false,
            files: Vec::new(),
            links: Vec::new(),
            warning: Some(why.to_string()),
        };
    }

    // One damaged set must not hide the others: what can't be read is
    // said on the set, and the overview goes on.
    let mut warnings = Vec::new();
    let manifest = load_backup_manifest(&path);
    if let Err(error) = &manifest {
        warnings.push(format!("the backup manifest could not be read: {error}"));
    }
    let walk = walk_set(&path);
    if let Some(error) = &walk.error {
        warnings.push(format!(
            "not every file in this set could be listed: {error}"
        ));
    }
    if !walk.links.is_empty() {
        let names: Vec<String> = walk
            .links
            .iter()
            .map(|link| link.display().to_string())
            .collect();
        warnings.push(format!(
            "these are symbolic links or junctions, which RABBIT never follows: {}",
            names.join(", ")
        ));
    }

    let kind = match &manifest {
        _ if deleting => BackupSetKind::Deleting,
        Err(_) => BackupSetKind::Damaged,
        Ok(None) => BackupSetKind::NoManifest,
        Ok(Some(manifest)) => match manifest.reason.as_str() {
            UNINSTALL_BACKUP_REASON => BackupSetKind::Uninstall,
            UPDATE_BACKUP_REASON => BackupSetKind::Update,
            KEYMAP_BACKUP_REASON => BackupSetKind::Keymap,
            BEFORE_RESTORE_REASON => BackupSetKind::BeforeRestore,
            RECORDS_BACKUP_REASON => BackupSetKind::Records,
            _ => BackupSetKind::Unknown,
        },
    };
    let manifest = manifest.ok().flatten();
    // The manifest's own record of when, then the id it was asked for;
    // without a manifest, the folder's name.
    let created_unix = match &manifest {
        Some(manifest) => manifest
            .created_unix
            .or_else(|| seconds_in_set_id(&manifest.created_at)),
        None => seconds_in_set_id(&id),
    };
    let manifest_file = path.join(BACKUP_MANIFEST_FILE);
    let fingerprint = manifest_fingerprint(&path).unwrap_or(SetFingerprint::Contents {
        file_count: walk.file_count,
        size: walk.size,
    });
    let (receipt_copy, holds_receipt) = match &manifest {
        Some(manifest) => {
            let recorded = manifest.receipt_backup_path.clone().or_else(|| {
                manifest
                    .files
                    .iter()
                    .find(|entry| is_receipt_entry(resource_path, entry))
                    .map(|entry| entry.backup_path.clone())
            });
            let copy = recorded
                .and_then(|recorded| copy_in_set(&path, &recorded).ok())
                .filter(|copy| is_real_file(copy));
            (copy, manifest_holds_receipt(resource_path, manifest))
        }
        None => {
            let relative = walk.files.iter().find(|relative| is_receipt(relative));
            (
                relative.map(|relative| path.join(relative)),
                relative.is_some(),
            )
        }
    };
    let owner = (environment.owner_of)(&path);
    BackupSetOverview {
        owned_by_another_account: matches!(
            (reference_owner, owner),
            (Some(expected), Some(owner)) if expected != owner
        ),
        created_unix,
        size: walk.size,
        file_count: walk.file_count,
        pending_entries: manifest.as_ref().map_or(0, |manifest| {
            manifest
                .files
                .iter()
                .filter(|entry| entry.state == BackupEntryState::Pending)
                .count()
        }),
        fingerprint: Some(fingerprint),
        manifest_path: manifest.as_ref().map(|_| manifest_file),
        manifest,
        receipt_copy,
        holds_receipt,
        files: walk.files,
        links: walk.links,
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
        id,
        path,
        kind,
    }
}

/// The fingerprint of the set at `set_path` as it is now, taken as the
/// overview takes it (see [`SetFingerprint`]): a restore compares it with
/// the one listed. Without a manifest, the set is walked without following
/// any link, and no file is opened.
pub(crate) fn current_fingerprint(set_path: &Path) -> SetFingerprint {
    manifest_fingerprint(set_path).unwrap_or_else(|| {
        let walk = walk_set(set_path);
        SetFingerprint::Contents {
            file_count: walk.file_count,
            size: walk.size,
        }
    })
}

/// The fingerprint of the set at `set_path` by its manifest, when it has
/// one that is a regular file.
fn manifest_fingerprint(set_path: &Path) -> Option<SetFingerprint> {
    let metadata = fs::symlink_metadata(set_path.join(BACKUP_MANIFEST_FILE)).ok()?;
    metadata.is_file().then(|| SetFingerprint::Manifest {
        length: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

/// An entry with every field but the essentials empty.
fn new_entry(
    item: BackupItem,
    group: BackupGroup,
    set_kind: BackupSetKind,
    created_unix: Option<u64>,
    status: BackupStatus,
) -> BackupEntry {
    BackupEntry {
        item,
        group,
        set_kind,
        created_unix,
        size: 0,
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
        restore_scope: None,
        refusal: None,
    }
}

/// The earlier receipt entry of a package an update replaced: known, and
/// `None` when RABBIT hadn't installed the copy replaced, or unknown.
enum EarlierEntry<'a> {
    Known(Option<&'a PackageReceipt>),
    Unknown,
}

/// What every backup's status is judged against: RABBIT's receipt, read
/// once, and the packages' exclusive groups, read when needed.
struct Judge<'a> {
    resource_path: &'a Path,
    environment: &'a BackupEnvironment<'a>,
    receipt: &'a std::result::Result<InstallState, String>,
    /// Each package's exclusive group, by package id.
    groups: OnceCell<BTreeMap<String, String>>,
    /// Where outside the resource folder a restore may put files back,
    /// asked the first time a set names such a place.
    outside: OnceCell<OutsideRestoreRules>,
}

impl Judge<'_> {
    /// Where a restore would put `entry` back, from the paths' text alone,
    /// or why it refuses the file (see [`restore_place_by_text`]).
    fn restore_place(
        &self,
        entry: &BackupManifestFile,
    ) -> std::result::Result<PathBuf, TargetRefusal> {
        restore_place_by_text(self.resource_path, entry, || {
            self.outside
                .get_or_init(|| (self.environment.outside_rules)(self.resource_path))
        })
    }

    /// The backups in `set`, as backup plan 3.1 maps sets to backups. Every
    /// set gives at least one.
    fn entries_of(&self, set: &BackupSetOverview) -> Vec<BackupEntry> {
        let manifest = match (&set.manifest, set.kind) {
            (
                Some(manifest),
                BackupSetKind::Uninstall
                | BackupSetKind::Update
                | BackupSetKind::Keymap
                | BackupSetKind::BeforeRestore
                | BackupSetKind::Unknown,
            ) => manifest,
            _ => return vec![self.whole_set(set)],
        };
        let mut entries = Vec::new();
        if set.kind == BackupSetKind::Uninstall {
            for (package_id, record) in &manifest.packages {
                entries.push(self.uninstalled(set, manifest, package_id, record));
            }
        } else {
            let keymap: Vec<&BackupManifestFile> = manifest
                .files
                .iter()
                .filter(|entry| is_keymap_entry(self.resource_path, entry))
                .collect();
            if !keymap.is_empty() {
                entries.push(self.keymap(set, manifest, &keymap));
            }
            // Only an update's set, and a restore's set that recorded
            // what it replaced, hold earlier versions.
            let mut packages: BTreeSet<&str> = BTreeSet::new();
            if matches!(
                set.kind,
                BackupSetKind::Update | BackupSetKind::BeforeRestore
            ) {
                packages.extend(manifest.replaced.keys().map(String::as_str));
                if set.kind == BackupSetKind::Update {
                    packages.extend(
                        manifest
                            .files
                            .iter()
                            .filter(|entry| self.belongs_to_a_version(entry))
                            .filter_map(|entry| entry.package_id.as_deref()),
                    );
                }
                let copy = OnceCell::new();
                for package_id in &packages {
                    entries.push(self.earlier_version(set, manifest, package_id, &copy));
                }
            }
            // A file that comes back with no backup above still comes back
            // with the whole set, so the set gets a backup of its own for
            // it: listed, counted, and never restored through the key
            // map's row.
            let held = |entry: &BackupManifestFile| {
                is_keymap_entry(self.resource_path, entry)
                    || is_receipt_entry(self.resource_path, entry)
                    || (self.belongs_to_a_version(entry)
                        && entry
                            .package_id
                            .as_deref()
                            .is_some_and(|package_id| packages.contains(package_id)))
            };
            if !entries.is_empty()
                && manifest
                    .files
                    .iter()
                    .any(|entry| !held(entry) && comes_back(&set.path, entry))
            {
                entries.push(self.whole_set(set));
            }
        }
        if entries.is_empty() {
            entries.push(self.whole_set(set));
        }
        entries
    }

    /// Whether `entry` of a set other than an uninstall's is a file of a
    /// package's earlier version: not the key map, which is a backup of its
    /// own, and not a copy of RABBIT's receipt.
    fn belongs_to_a_version(&self, entry: &BackupManifestFile) -> bool {
        !is_keymap_entry(self.resource_path, entry) && !is_receipt_entry(self.resource_path, entry)
    }

    /// One backup for the whole set: a set RABBIT can't take apart, the
    /// files of a set no other backup holds, an unfinished deletion, or
    /// what isn't a backup at all.
    fn whole_set(&self, set: &BackupSetOverview) -> BackupEntry {
        let item = if set.kind == BackupSetKind::Deleting {
            BackupItem::Deleting {
                set_id: set.id.clone(),
            }
        } else {
            BackupItem::Set {
                set_id: set.id.clone(),
            }
        };
        let status = if set.kind == BackupSetKind::NotABackup {
            BackupStatus::NotABackup
        } else if set.owned_by_another_account {
            BackupStatus::NotYours
        } else {
            BackupStatus::NotHere
        };
        let whole = (status == BackupStatus::NotHere).then(|| self.restores_whole(set));
        let refused = whole
            .as_ref()
            .and_then(|whole| whole.as_ref().err().cloned());
        let refusal = refused
            .clone()
            .or_else(|| self.status_refusal(set, &item, status, None));
        BackupEntry {
            size: set.size,
            restore_scope: matches!(whole, Some(Ok(true))).then_some(RestoreScope::WholeSet),
            refused_path: refused.as_ref().and_then(refused_path_of),
            refusal,
            ..new_entry(item, BackupGroup::Other, set.kind, set.created_unix, status)
        }
    }

    /// Whether `restore_backup_set` takes `set` as a whole today, as far as
    /// the overview can tell without hashing. `Err` is the refusal a
    /// restore of the whole set gives, in the order a restore refuses: an
    /// unfinished deletion, a damaged manifest, an update's set, a set of
    /// RABBIT's records, a copy of RABBIT's receipt in it, then the first
    /// file it would refuse to put back (see [`Self::plan_targets`]).
    /// `Ok(false)` when it can't be restored for another reason: another
    /// account's, which its status says, what else couldn't be read of it,
    /// or nothing to bring back.
    fn restores_whole(&self, set: &BackupSetOverview) -> std::result::Result<bool, RestoreRefusal> {
        if set.owned_by_another_account {
            return Ok(false);
        }
        let backup_id = || set.id.clone();
        match set.kind {
            BackupSetKind::Deleting => {
                return Err(RestoreRefusal::BeingDeleted {
                    backup_id: backup_id(),
                });
            }
            BackupSetKind::Damaged => {
                return manifest_refusal(&set.id, &set.path).map_or(Ok(false), Err);
            }
            BackupSetKind::Update => {
                return Err(RestoreRefusal::EarlierVersionNotYet {
                    backup_id: backup_id(),
                });
            }
            BackupSetKind::Records => {
                return Err(RestoreRefusal::RecordsOnly {
                    backup_id: backup_id(),
                });
            }
            _ => {}
        }
        if set.holds_receipt {
            return Err(RestoreRefusal::ReceiptInSet {
                backup_id: backup_id(),
                backup_path: set.path.clone(),
            });
        }
        // A link in a set without a manifest refuses it as a restore does,
        // in `plan_targets`. Anything else that couldn't be read of a set
        // stops a restore too, but with no refusal of its own.
        let only_links_unread = set.manifest.is_none() && !set.links.is_empty();
        if set.warning.is_some() && !only_links_unread {
            return Ok(false);
        }
        let brings_back = match (set.kind, &set.manifest) {
            (BackupSetKind::NoManifest, None) => !set.files.is_empty() || !set.links.is_empty(),
            (
                BackupSetKind::Uninstall
                | BackupSetKind::Keymap
                | BackupSetKind::BeforeRestore
                | BackupSetKind::Unknown,
                Some(manifest),
            ) => manifest
                .files
                .iter()
                .any(|entry| comes_back(&set.path, entry)),
            _ => false,
        };
        if !brings_back {
            return Ok(false);
        }
        let whole = BackupItem::Set {
            set_id: set.id.clone(),
        };
        self.plan_targets(set, &whole).map(|_| true)
    }

    /// The files a restore of `item` from `set` writes, sorted, or the
    /// refusal of the first file the restore would refuse to put back,
    /// which stops it whole, for the reason the restore gives: in RABBIT's
    /// own folder, by a name that may open another file, outside every
    /// folder RABBIT installs into, a second file for one place, or without
    /// a manifest a link or a file from outside the resource folder. The
    /// refused file is named by its original path, or its place in a set
    /// without a manifest; a second file for one place by that place, and a
    /// link by its path in `RABBIT/backups`, as a restore names them. Only
    /// what comes back counts, as for the restore. Nothing is hashed, and
    /// nothing outside the set is looked at.
    fn plan_targets(
        &self,
        set: &BackupSetOverview,
        item: &BackupItem,
    ) -> std::result::Result<Vec<PathBuf>, RestoreRefusal> {
        let mut planned: Vec<(PathBuf, PathBuf)> = Vec::new();
        match &set.manifest {
            // Without a manifest only the whole set restores, by its layout.
            None => {
                if !matches!(item, BackupItem::Set { .. }) {
                    return Ok(Vec::new());
                }
                if let Some(link) = set.links.first() {
                    return Err(RestoreRefusal::IsLink {
                        path: set.path.join(link),
                    });
                }
                for relative in &set.files {
                    let place = if is_outside_file_in_layout(relative) {
                        Err(TargetRefusal::OutsideWithoutManifest)
                    } else {
                        inside_place_by_text(self.resource_path, relative)
                    }
                    .map_err(|reason| reason.refuse(relative))?;
                    planned.push((place, relative.clone()));
                }
            }
            Some(manifest) => {
                for entry in manifest.files.iter().filter(|entry| {
                    restores_entry(self.resource_path, manifest, item, entry)
                        && comes_back(&set.path, entry)
                }) {
                    let place = self
                        .restore_place(entry)
                        .map_err(|reason| reason.refuse(&entry.original_path))?;
                    planned.push((place, entry.original_path.clone()));
                }
            }
        }
        let mut places = BTreeSet::new();
        if let Some(place) = planned
            .iter()
            .map(|(place, _)| place)
            .find(|place| !places.insert(same_place(place)))
        {
            return Err(TargetRefusal::TwoFilesForOnePlace.refuse(place));
        }
        let mut targets: Vec<PathBuf> = planned.into_iter().map(|(place, _)| place).collect();
        targets.sort();
        Ok(targets)
    }

    /// The files a restore of `entry`, a backup of `set`, writes, for
    /// [`BackupOverview::restore_targets`]. Empty for a backup RABBIT never
    /// restores, and for one a restore would refuse a file of.
    fn targets_of(&self, set: &BackupSetOverview, entry: &BackupEntry) -> Vec<PathBuf> {
        let never = matches!(
            entry.status,
            BackupStatus::NotYours | BackupStatus::NotABackup
        ) || matches!(
            set.kind,
            BackupSetKind::Damaged
                | BackupSetKind::Deleting
                | BackupSetKind::Records
                | BackupSetKind::NotABackup
        );
        if never {
            return Vec::new();
        }
        self.plan_targets(set, &entry.item).unwrap_or_default()
    }

    /// The backup of one package an uninstall moved into `set`.
    fn uninstalled(
        &self,
        set: &BackupSetOverview,
        manifest: &BackupManifest,
        package_id: &str,
        record: &crate::rollback::BackupPackageRecord,
    ) -> BackupEntry {
        let files: Vec<&BackupManifestFile> = manifest
            .files
            .iter()
            .filter(|entry| entry.package_id.as_deref() == Some(package_id))
            .collect();
        let (status, missing_path) = self.uninstalled_status(set, package_id, record, &files);
        let item = BackupItem::Package {
            set_id: set.id.clone(),
            package_id: package_id.to_string(),
        };
        let plan =
            (status.can_restore() && !set.holds_receipt).then(|| self.plan_targets(set, &item));
        let refused = plan.as_ref().and_then(|plan| plan.as_ref().err().cloned());
        // A restore refuses a set holding a copy of RABBIT's receipt before
        // it looks at any package of it.
        let receipt_in_set = (set.holds_receipt && status != BackupStatus::NotYours).then(|| {
            RestoreRefusal::ReceiptInSet {
                backup_id: set.id.clone(),
                backup_path: set.path.clone(),
            }
        });
        let refusal = receipt_in_set
            .or_else(|| refused.clone())
            .or_else(|| self.status_refusal(set, &item, status, missing_path.as_ref()));
        BackupEntry {
            size: files.iter().map(|entry| entry.size).sum(),
            version: record
                .receipt
                .as_ref()
                .and_then(|entry| entry.version.clone()),
            with_settings: record.with_settings,
            restore_scope: matches!(plan, Some(Ok(_))).then_some(RestoreScope::Package),
            refused_path: refused.as_ref().and_then(refused_path_of),
            refusal,
            missing_path,
            // Files already back are on disk too: deleting loses nothing.
            only_copy: status != BackupStatus::AlreadyBack,
            ..new_entry(
                item,
                BackupGroup::UninstalledSoftware,
                set.kind,
                set.created_unix,
                status,
            )
        }
    }

    /// Backup plan 3.4 for uninstalled software, with the restore's own
    /// rules: a package installed again since is refused, and one whose
    /// entry is still the recorded one is no reinstall.
    fn uninstalled_status(
        &self,
        set: &BackupSetOverview,
        package_id: &str,
        record: &crate::rollback::BackupPackageRecord,
        files: &[&BackupManifestFile],
    ) -> (BackupStatus, Option<PathBuf>) {
        if set.owned_by_another_account {
            return (BackupStatus::NotYours, None);
        }
        if let Some(missing) = first_copy_gone(&set.path, files) {
            return (BackupStatus::PartMissing, Some(missing));
        }
        let coming: Vec<&BackupManifestFile> = files
            .iter()
            .copied()
            .filter(|entry| comes_back(&set.path, entry))
            .collect();
        if coming.is_empty() {
            return (BackupStatus::NothingInIt, None);
        }
        let Ok(state) = self.receipt else {
            return (BackupStatus::RecordsUnreadable, None);
        };
        let current = state.packages.get(package_id);
        if current.is_some_and(|current| record.receipt.as_ref() != Some(current)) {
            return (BackupStatus::InstalledAgain, None);
        }
        let as_recorded = current == record.receipt.as_ref();
        let status = if as_recorded && self.all_on_disk(&coming) {
            BackupStatus::AlreadyBack
        } else if as_recorded && record.receipt.is_some() {
            BackupStatus::StoppedPartway
        } else {
            BackupStatus::CanRestore
        };
        (status, None)
    }

    /// The key map `set` holds, in `keymap`'s entries.
    fn keymap(
        &self,
        set: &BackupSetOverview,
        manifest: &BackupManifest,
        keymap: &[&BackupManifestFile],
    ) -> BackupEntry {
        let (status, missing_path) = if set.owned_by_another_account {
            (BackupStatus::NotYours, None)
        } else if let Some(missing) = first_copy_gone(&set.path, keymap) {
            (BackupStatus::PartMissing, Some(missing))
        } else if !keymap.iter().any(|entry| comes_back(&set.path, entry)) {
            (BackupStatus::NothingInIt, None)
        } else {
            (BackupStatus::CanRestore, None)
        };
        // The command line restores a key map only with its whole set, so
        // only while the key map is all that comes back of the set: a
        // restore of the set would put any other file back too. An
        // update's set is refused whole.
        let alone = manifest
            .files
            .iter()
            .filter(|entry| comes_back(&set.path, entry))
            .all(|entry| is_keymap_entry(self.resource_path, entry));
        let whole = (status == BackupStatus::CanRestore && alone).then(|| self.restores_whole(set));
        let refused = whole
            .as_ref()
            .and_then(|whole| whole.as_ref().err().cloned());
        let item = BackupItem::Keymap {
            set_id: set.id.clone(),
        };
        let refusal = refused
            .clone()
            .or_else(|| self.status_refusal(set, &item, status, missing_path.as_ref()));
        BackupEntry {
            size: keymap.iter().map(|entry| entry.size).sum(),
            keymap_origin: manifest.keymap_origin,
            restore_scope: matches!(whole, Some(Ok(true))).then_some(RestoreScope::WholeSet),
            refused_path: refused.as_ref().and_then(refused_path_of),
            refusal,
            missing_path,
            ..new_entry(
                item,
                BackupGroup::Keymaps,
                set.kind,
                set.created_unix,
                status,
            )
        }
    }

    /// The earlier version of `package_id` an update, or a restore of an
    /// earlier version, kept in `set`. `copy` holds the set's copy of
    /// RABBIT's receipt once read; only an older update's set has one.
    fn earlier_version(
        &self,
        set: &BackupSetOverview,
        manifest: &BackupManifest,
        package_id: &str,
        copy: &OnceCell<Option<InstallState>>,
    ) -> BackupEntry {
        let files: Vec<&BackupManifestFile> = manifest
            .files
            .iter()
            .filter(|entry| {
                entry.package_id.as_deref() == Some(package_id) && self.belongs_to_a_version(entry)
            })
            .collect();
        let replaced = manifest.replaced.get(package_id);
        let earlier = match replaced {
            Some(record) => EarlierEntry::Known(record.receipt.as_ref()),
            None if set.kind == BackupSetKind::Update => {
                match copy.get_or_init(|| read_receipt_copy(set)) {
                    Some(copy) => EarlierEntry::Known(copy.packages.get(package_id)),
                    None => EarlierEntry::Unknown,
                }
            }
            None => EarlierEntry::Unknown,
        };
        let (status, missing_path) =
            self.earlier_version_status(set, manifest, package_id, &earlier, &files);
        let item = BackupItem::Package {
            set_id: set.id.clone(),
            package_id: package_id.to_string(),
        };
        BackupEntry {
            size: files.iter().map(|entry| entry.size).sum(),
            version: match &earlier {
                EarlierEntry::Known(Some(entry)) => entry.version.clone(),
                _ => None,
            },
            replaced_by: replaced.and_then(|record| record.replaced_by.clone()),
            refusal: self.status_refusal(set, &item, status, missing_path.as_ref()),
            missing_path,
            // `restore_backup_set` refuses earlier versions until the swap
            // lands.
            restore_scope: None,
            ..new_entry(
                item,
                BackupGroup::EarlierVersions,
                set.kind,
                set.created_unix,
                status,
            )
        }
    }

    /// Backup plan 3.4 for an earlier version, by the swap's rules (6.4):
    /// the package's entry would become the earlier one, so every file of
    /// that entry must be in the set or still on disk as recorded.
    fn earlier_version_status(
        &self,
        set: &BackupSetOverview,
        manifest: &BackupManifest,
        package_id: &str,
        earlier: &EarlierEntry<'_>,
        files: &[&BackupManifestFile],
    ) -> (BackupStatus, Option<PathBuf>) {
        if set.owned_by_another_account {
            return (BackupStatus::NotYours, None);
        }
        if let Some(missing) = first_copy_gone(&set.path, files) {
            return (BackupStatus::PartMissing, Some(missing));
        }
        let coming: Vec<&BackupManifestFile> = files
            .iter()
            .copied()
            .filter(|entry| comes_back(&set.path, entry))
            .collect();
        if coming.is_empty() {
            return (BackupStatus::NothingInIt, None);
        }
        let EarlierEntry::Known(earlier) = earlier else {
            return (BackupStatus::VersionUnknown, None);
        };
        let Ok(state) = self.receipt else {
            return (BackupStatus::RecordsUnreadable, None);
        };
        if !self
            .others_of_its_group_installed(package_id, state)
            .is_empty()
        {
            return (BackupStatus::OtherLanguagePack, None);
        }
        if same_entry_but_channel(state.packages.get(package_id), *earlier)
            && self.all_on_disk(&coming)
        {
            return (BackupStatus::AlreadyBack, None);
        }
        if let Some(missing) =
            earlier.and_then(|earlier| self.first_file_not_kept(manifest, earlier, state))
        {
            return (BackupStatus::NotComplete, Some(missing));
        }
        (BackupStatus::CanRestore, None)
    }

    /// The first file of `earlier`, an earlier receipt entry, that would be
    /// missing after a swap: neither in the set with its hash, nor recorded
    /// on disk with it by RABBIT's current receipt and there with its size.
    /// Files the receipt records without a hash, such as `reaper.ini`,
    /// belong to the user and count as kept.
    fn first_file_not_kept(
        &self,
        manifest: &BackupManifest,
        earlier: &PackageReceipt,
        state: &InstallState,
    ) -> Option<PathBuf> {
        let resource = self.resource_path;
        earlier
            .installed_files
            .iter()
            .find(|file| {
                let Some(sha256) = &file.sha256 else {
                    return false;
                };
                let place = same_place(&receipt_file_path(resource, &file.path));
                let in_set = manifest.files.iter().any(|entry| {
                    entry.state == BackupEntryState::Done
                        && entry.sha256 == *sha256
                        && entry_place(resource, entry)
                            .is_some_and(|found| same_place(&found.path(resource)) == place)
                });
                // Only a place the current receipt records is looked at,
                // by the path it records: never one only a set names.
                let on_disk = || {
                    state
                        .packages
                        .values()
                        .flat_map(|entry| &entry.installed_files)
                        .map(|current| (current, receipt_file_path(resource, &current.path)))
                        .find(|(current, path)| {
                            current.sha256.as_ref() == Some(sha256) && same_place(path) == place
                        })
                        .and_then(|(_, path)| place_metadata(&path))
                        .is_some_and(|metadata| {
                            metadata.is_file()
                                && file.size.is_none_or(|size| metadata.len() == size)
                        })
                };
                !in_set && !on_disk()
            })
            .map(|file| file.path.clone())
    }

    /// Whether every file in `entries` is where a restore would put it
    /// back, with the size its entry recorded. Sizes only; nothing is
    /// hashed. A file a restore would refuse counts as not there, and its
    /// place is never looked at: a set could name a path on another
    /// computer, which Windows would connect to.
    fn all_on_disk(&self, entries: &[&BackupManifestFile]) -> bool {
        entries.iter().all(|entry| {
            self.restore_place(entry)
                .ok()
                .and_then(|place| place_metadata(&place))
                .is_some_and(|metadata| metadata.is_file() && metadata.len() == entry.size)
        })
    }

    /// The other packages of `package_id`'s exclusive group that `state`
    /// lists, whose install would remove this one's. Language packs were
    /// such a group and no longer are: REAPER keeps every pack and uses the
    /// one chosen. No package the manifests ship declares a group today.
    fn others_of_its_group_installed(&self, package_id: &str, state: &InstallState) -> Vec<String> {
        let groups = self
            .groups
            .get_or_init(|| (self.environment.exclusive_groups)());
        let Some(group) = groups.get(package_id) else {
            return Vec::new();
        };
        state
            .packages
            .keys()
            .filter(|other| *other != package_id && groups.get(*other) == Some(group))
            .cloned()
            .collect()
    }

    /// The refusal a restore gives for what `status` says about `item`, a
    /// backup of `set`, when the status is a reason a restore refuses for,
    /// in the same code (see [`BackupEntry::refusal`]). `missing_path` is
    /// the entry's own.
    fn status_refusal(
        &self,
        set: &BackupSetOverview,
        item: &BackupItem,
        status: BackupStatus,
        missing_path: Option<&PathBuf>,
    ) -> Option<RestoreRefusal> {
        let backup_id = set.id.clone();
        let package_id = item.package_id().map(ToString::to_string);
        Some(match status {
            BackupStatus::InstalledAgain => RestoreRefusal::ReinstalledSince {
                backup_id,
                package_id: package_id?,
                other_packages: Vec::new(),
            },
            BackupStatus::NothingInIt => RestoreRefusal::NothingOfItInSet {
                backup_id,
                package_ids: vec![package_id?],
            },
            BackupStatus::PartMissing => RestoreRefusal::PartMissing {
                backup_id,
                path: missing_path?.clone(),
            },
            BackupStatus::NotComplete => RestoreRefusal::NotComplete {
                backup_id,
                package_id: package_id?,
                path: missing_path?.clone(),
            },
            BackupStatus::OtherLanguagePack => {
                let package_id = package_id?;
                let installed = self
                    .receipt
                    .as_ref()
                    .map(|state| self.others_of_its_group_installed(&package_id, state))
                    .unwrap_or_default();
                RestoreRefusal::OtherLanguagePackInstalled {
                    backup_id,
                    package_id,
                    installed,
                }
            }
            BackupStatus::RecordsUnreadable => RestoreRefusal::ReceiptUnreadable {
                detail: self.receipt.as_ref().err()?.clone(),
            },
            BackupStatus::NotYours => RestoreRefusal::NotYours { backup_id },
            BackupStatus::CanRestore
            | BackupStatus::StoppedPartway
            | BackupStatus::AlreadyBack
            | BackupStatus::VersionUnknown
            | BackupStatus::NotHere
            | BackupStatus::NotABackup => return None,
        })
    }
}

/// The file `refusal` names, for [`BackupEntry::refused_path`]: the one a
/// restore would refuse to put back for where it goes. A file from outside
/// the resource folder in a set without a manifest, and a link in such a
/// set, are no such file: what refuses them is the set itself, which the
/// set's kind and warning say.
fn refused_path_of(refusal: &RestoreRefusal) -> Option<PathBuf> {
    match refusal {
        RestoreRefusal::TargetRefused {
            reason: TargetRefusal::OutsideWithoutManifest,
            ..
        } => None,
        RestoreRefusal::TargetRefused { path, .. } => Some(path.clone()),
        _ => None,
    }
}

/// Whether two receipt entries are the same but for their channel: a swap
/// keeps the current entry's channel (backup plan 6.4). Two missing
/// entries are the same.
fn same_entry_but_channel(
    current: Option<&PackageReceipt>,
    earlier: Option<&PackageReceipt>,
) -> bool {
    match (current, earlier) {
        (None, None) => true,
        (Some(current), Some(earlier)) => {
            let mut current = current.clone();
            current.channel.clone_from(&earlier.channel);
            current == *earlier
        }
        _ => false,
    }
}

/// A receipt's path for a file as a path on disk: relative ones are inside
/// the resource folder.
fn receipt_file_path(resource_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        resource_path.join(path)
    }
}

/// The copy of RABBIT's receipt an older update set holds, read. `None`
/// when it has none, or it can't be read.
fn read_receipt_copy(set: &BackupSetOverview) -> Option<InstallState> {
    let copy = set.receipt_copy.as_ref()?;
    let content = fs::read_to_string(copy).ok()?;
    serde_json::from_str(&content).ok()
}

/// Whether `entry` is the key map: its original is the resource folder's
/// `reaper-kb.ini`.
fn is_keymap_entry(resource_path: &Path, entry: &BackupManifestFile) -> bool {
    matches!(
        entry_place(resource_path, entry),
        Some(EntryPlace::Inside(relative))
            if same_place(&relative) == same_place(Path::new(KEYMAP_FILE))
    )
}

/// The metadata of the file at `path`, a place a restore would write or the
/// receipt records, by its own metadata: a link is never followed. Tests
/// see every place looked at (see [`places_looked_at_on_this_thread`]).
fn place_metadata(path: &Path) -> Option<fs::Metadata> {
    #[cfg(test)]
    PLACES_LOOKED_AT.with(|places| places.borrow_mut().push(path.to_path_buf()));
    fs::symlink_metadata(path).ok()
}

#[cfg(test)]
thread_local! {
    static PLACES_LOOKED_AT: std::cell::RefCell<Vec<PathBuf>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Every place outside the sets that this thread's overviews looked at,
/// for tests that prove a place is never looked at.
#[cfg(test)]
fn places_looked_at_on_this_thread() -> Vec<PathBuf> {
    PLACES_LOOKED_AT.with(|places| places.borrow().clone())
}

/// Whether a restore of `item` takes `entry` of `manifest`, as far as the
/// set's layout goes: a package's own files (in an update's or restore's
/// set, neither the key map nor a copy of RABBIT's receipt, which are no
/// file of a version), the key map, or every file of a whole set.
fn restores_entry(
    resource_path: &Path,
    manifest: &BackupManifest,
    item: &BackupItem,
    entry: &BackupManifestFile,
) -> bool {
    match item {
        BackupItem::Package { package_id, .. } => {
            entry.package_id.as_deref() == Some(package_id.as_str())
                && (manifest.reason == UNINSTALL_BACKUP_REASON
                    || (!is_keymap_entry(resource_path, entry)
                        && !is_receipt_entry(resource_path, entry)))
        }
        BackupItem::Keymap { .. } => is_keymap_entry(resource_path, entry),
        BackupItem::Set { .. } => true,
        BackupItem::Deleting { .. } | BackupItem::Records { .. } => false,
    }
}

/// The size of `entry`'s copy in the set at `set_path`, when it is a
/// regular file there, reached through no link.
fn copy_size(set_path: &Path, entry: &BackupManifestFile) -> Option<u64> {
    let copy = copy_in_set(set_path, &entry.backup_path).ok()?;
    let metadata = fs::symlink_metadata(copy).ok()?;
    metadata.is_file().then_some(metadata.len())
}

/// Whether a restore would put `entry`'s copy back, judged without hashing:
/// a finished entry always counts, as for the restore, and a pending one
/// when its copy is there with the size recorded. The restore itself also
/// checks a pending copy's hash, so it may still leave out a copy of the
/// right size that never finished.
fn comes_back(set_path: &Path, entry: &BackupManifestFile) -> bool {
    entry.state == BackupEntryState::Done || copy_size(set_path, entry) == Some(entry.size)
}

/// The original path of the first finished entry among `entries` whose
/// copy is gone from the set at `set_path`, or has another size than
/// recorded: a restore would stop on it.
fn first_copy_gone(set_path: &Path, entries: &[&BackupManifestFile]) -> Option<PathBuf> {
    entries
        .iter()
        .find(|entry| {
            entry.state == BackupEntryState::Done && copy_size(set_path, entry) != Some(entry.size)
        })
        .map(|entry| entry.original_path.clone())
}

#[cfg(test)]
#[path = "backup_overview_tests.rs"]
mod tests;
