use std::fs;
use std::path::{Component, Path, PathBuf, Prefix};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{IoPathContext, JsonPathContext, RabbitError, Result};
use crate::hash::sha256_file;
use crate::preflight::{PreflightOptions, PreflightReport, run_install_preflight};
use crate::receipt::{RECEIPT_RELATIVE_PATH, receipt_path};

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
const BEFORE_RESTORE_REASON: &str = "before-restore";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSet {
    pub id: String,
    pub path: PathBuf,
    pub created_at: Option<String>,
    pub reason: Option<String>,
    pub manifest_path: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    /// Set when part of this set could not be read, most often a damaged
    /// manifest. The set is still listed; restoring it will fail with the
    /// full error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub rabbit_version: String,
    pub created_at: String,
    pub reason: String,
    pub files: Vec<BackupManifestFile>,
    pub receipt_backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifestFile {
    pub package_id: Option<String>,
    /// Where the file came from: relative to the resource folder for files
    /// inside it, absolute for files outside it. Sets written by older
    /// RABBIT versions also hold absolute paths for files inside it.
    pub original_path: PathBuf,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreBackupOptions {
    pub dry_run: bool,
    pub allow_reaper_running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreBackupReport {
    pub resource_path: PathBuf,
    pub backup_id: String,
    pub backup_path: PathBuf,
    pub dry_run: bool,
    pub preflight: PreflightReport,
    pub actions: Vec<RestoreBackupAction>,
    /// Files whose backup never finished. The run that started it stopped
    /// before it changed them, so there is nothing to put back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_targets: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreBackupAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub current_backup_path: Option<PathBuf>,
    pub action: RestoreBackupActionKind,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestoreBackupActionKind {
    WouldRestore,
    Restored,
}

/// Writes one backup set under `RABBIT/backups/<id>/`, keeping its manifest
/// up to date after every step. Installs, the unattended installers' key map
/// and receipt backups, and restore itself all write their sets through it,
/// so every set has the same layout and every set can be restored.
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
        Self {
            resource_path: resource_path.to_path_buf(),
            set_path: backups_root(resource_path).join(set_id),
            manifest: BackupManifest {
                schema_version: 1,
                rabbit_version: env!("CARGO_PKG_VERSION").to_string(),
                created_at: set_id.to_string(),
                reason: reason.to_string(),
                files: Vec::new(),
                receipt_backup_path: None,
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

    /// Copy `original` into the set and return where the copy is. The
    /// original is left as it is.
    pub fn copy_file(&mut self, package_id: Option<&str>, original: &Path) -> Result<PathBuf> {
        let index = self.begin_entry(package_id, original)?;
        let entry = self.manifest.files[index].clone();
        if self.planning {
            return Ok(entry.backup_path);
        }

        if let Some(parent) = entry.backup_path.parent() {
            fs::create_dir_all(parent).with_path(parent)?;
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

    /// Copy RABBIT's receipt into the set, when there is one, and record it
    /// as the set's receipt backup.
    pub fn copy_receipt_if_present(&mut self) -> Result<Option<PathBuf>> {
        let receipt = receipt_path(&self.resource_path);
        if !receipt.is_file() {
            return Ok(None);
        }
        let backup_path = self.copy_file(None, &receipt)?;
        self.manifest.receipt_backup_path = Some(backup_path.clone());
        if !self.planning {
            self.save()?;
        }
        Ok(Some(backup_path))
    }

    /// Record `original` as pending, saved to disk before anything is
    /// copied, and write the origin note for an outside file. Returns the
    /// entry's index.
    fn begin_entry(&mut self, package_id: Option<&str>, original: &Path) -> Result<usize> {
        // Read the original first: one that can't be read leaves no set.
        let (size, sha256) = if self.planning {
            (0, String::new())
        } else {
            (
                fs::metadata(original).with_path(original)?.len(),
                sha256_file(original)?,
            )
        };
        self.start()?;
        let slot = self.slot_for(original)?;
        self.manifest.files.push(BackupManifestFile {
            package_id: package_id.map(ToString::to_string),
            original_path: slot.original_path.clone(),
            backup_path: slot.backup_path,
            size,
            sha256,
            state: BackupEntryState::Pending,
        });
        if !self.planning {
            self.save()?;
            if let Some(note_path) = slot.origin_note {
                if let Some(parent) = note_path.parent() {
                    fs::create_dir_all(parent).with_path(parent)?;
                }
                fs::write(&note_path, slot.original_path.to_string_lossy().as_bytes())
                    .with_path(&note_path)?;
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
    /// writing into the first one's set and replacing its manifest.
    fn start(&mut self) -> Result<()> {
        if self.started || self.planning {
            return Ok(());
        }
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
        self.started = true;
        Ok(())
    }

    /// Where a backup of `original` goes.
    fn slot_for(&self, original: &Path) -> Result<BackupSlot> {
        let absolute = std::path::absolute(original).with_path(original)?;
        let resource = std::path::absolute(&self.resource_path).with_path(&self.resource_path)?;
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
        let number = self
            .manifest
            .files
            .iter()
            .filter(|file| file.original_path.is_absolute())
            .count()
            + 1;
        let folder = self.set_path.join(EXTERNAL_DIR).join(number.to_string());
        Ok(BackupSlot {
            original_path: absolute,
            backup_path: folder.join(&file_name),
            origin_note: Some(folder.join(format!("{file_name}{ORIGIN_SIDECAR_SUFFIX}"))),
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

pub fn list_backup_sets(resource_path: &Path) -> Result<Vec<BackupSet>> {
    let backups_root = backups_root(resource_path);
    if !backups_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut sets = Vec::new();
    for entry in fs::read_dir(&backups_root).with_path(&backups_root)? {
        let entry = entry.with_path(&backups_root)?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
        else {
            continue;
        };
        // One damaged set must not hide all the others: a manifest that
        // can't be read, or files that can't be listed, are reported on that
        // set and the listing goes on.
        let mut warnings = Vec::new();
        let manifest = load_backup_manifest(&path).unwrap_or_else(|error| {
            warnings.push(format!("the backup manifest could not be read: {error}"));
            None
        });
        let mut files = Vec::new();
        if let Err(error) = collect_relative_files(&path, &path, &mut files) {
            warnings.push(format!(
                "not every file in this set could be listed: {error}"
            ));
        }
        files.sort();
        sets.push(BackupSet {
            id,
            manifest_path: manifest.as_ref().map(|_| path.join(BACKUP_MANIFEST_FILE)),
            created_at: manifest
                .as_ref()
                .map(|manifest| manifest.created_at.clone()),
            reason: manifest.as_ref().map(|manifest| manifest.reason.clone()),
            path,
            files,
            warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
        });
    }

    sets.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(sets)
}

pub fn restore_backup_set(
    resource_path: &Path,
    backup_id: &str,
    options: &RestoreBackupOptions,
) -> Result<RestoreBackupReport> {
    restore_backup_set_within(
        resource_path,
        backup_id,
        options,
        &crate::install::outside_install_roots(),
    )
}

/// [`restore_backup_set`] with the folders outside the resource folder that
/// a restore may write to passed in, so tests can use a temporary folder
/// instead of the user's real CLAP folder.
fn restore_backup_set_within(
    resource_path: &Path,
    backup_id: &str,
    options: &RestoreBackupOptions,
    outside_roots: &[PathBuf],
) -> Result<RestoreBackupReport> {
    let backup_path = backup_set_path(resource_path, backup_id)?;
    if !backup_path.is_dir() {
        return Err(RabbitError::BackupNotFound(backup_path));
    }

    let preflight = run_install_preflight(
        resource_path,
        &PreflightOptions {
            dry_run: options.dry_run,
            allow_reaper_running: options.allow_reaper_running,
            target_app_path: None,
        },
    );
    if !preflight.passed {
        return Err(RabbitError::PreflightFailed {
            message: preflight.failure_message(),
        });
    }

    // A restore writes the same files an install does, so it must not run
    // while an install, or another restore, works on this resource folder.
    let _install_lock = if options.dry_run {
        None
    } else {
        Some(crate::lock::acquire_package_install_lock(resource_path)?)
    };

    // Every file is checked before the first one is touched, so a refused
    // or damaged entry stops the restore with nothing changed.
    let plan = plan_restore(resource_path, &backup_path, outside_roots)?;

    let before_restore_id = format!("{}-before-restore", restore_timestamp());
    let mut planned_backups =
        BackupSetWriter::planning(resource_path, &before_restore_id, BEFORE_RESTORE_REASON);
    let mut actions = Vec::new();
    for file in plan.files {
        let current_backup_path = if file.target_path.is_file() {
            Some(planned_backups.copy_file(file.package_id.as_deref(), &file.target_path)?)
        } else {
            None
        };
        actions.push(RestoreBackupAction {
            package_id: file.package_id,
            source_path: file.source_path,
            target_path: file.target_path,
            current_backup_path,
            action: if options.dry_run {
                RestoreBackupActionKind::WouldRestore
            } else {
                RestoreBackupActionKind::Restored
            },
            size: file.size,
            sha256: file.sha256,
        });
    }

    if !options.dry_run {
        // The files a restore replaces go into a set of their own, written
        // like any other, so a restore can be undone the same way.
        let mut backups =
            BackupSetWriter::new(resource_path, &before_restore_id, BEFORE_RESTORE_REASON);
        for action in &mut actions {
            restore_file(action, &mut backups)?;
        }
    }

    Ok(RestoreBackupReport {
        resource_path: resource_path.to_path_buf(),
        backup_id: backup_id.to_string(),
        backup_path,
        dry_run: options.dry_run,
        preflight,
        actions,
        skipped_targets: plan.skipped_targets,
    })
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

fn plan_restore(
    resource_path: &Path,
    backup_path: &Path,
    outside_roots: &[PathBuf],
) -> Result<RestorePlan> {
    match load_backup_manifest(backup_path)? {
        Some(manifest) => {
            plan_restore_from_manifest(resource_path, backup_path, &manifest, outside_roots)
        }
        None => plan_restore_from_layout(resource_path, backup_path),
    }
}

/// Each entry goes back to the `original_path` it records. Older sets mix
/// relative paths (the unattended key map and receipt backups) with
/// absolute ones (install replacements), so both are read here.
fn plan_restore_from_manifest(
    resource_path: &Path,
    backup_path: &Path,
    manifest: &BackupManifest,
    outside_roots: &[PathBuf],
) -> Result<RestorePlan> {
    let mut plan = RestorePlan {
        files: Vec::new(),
        skipped_targets: Vec::new(),
    };
    for entry in &manifest.files {
        let recorded = recorded_set_location(&entry.backup_path);
        let target_path = restore_target(
            resource_path,
            &entry.original_path,
            recorded.as_ref().map(|(resource, _)| resource.as_path()),
            outside_roots,
        )?;
        let source_path = restore_source(
            backup_path,
            &entry.backup_path,
            recorded.as_ref().map(|(_, set)| set.as_path()),
        )?;
        let finished = entry.state == BackupEntryState::Done;

        // A pending entry's copy may never have finished. The run stopped
        // before changing the file then, so the file is still where it
        // was: restore the copy only when it is complete.
        if !source_path.is_file() {
            if finished {
                return Err(RabbitError::BackupNotFound(source_path));
            }
            plan.skipped_targets.push(target_path);
            continue;
        }
        let sha256 = sha256_file(&source_path)?;
        if !entry.sha256.is_empty() && sha256 != entry.sha256 {
            if finished {
                return Err(RabbitError::HashMismatch {
                    path: source_path,
                    expected: entry.sha256.clone(),
                    actual: sha256,
                });
            }
            plan.skipped_targets.push(target_path);
            continue;
        }
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

/// A set without a manifest (one from a run that died before older RABBIT
/// versions wrote it, or a `before-restore` set from them) mirrors the
/// resource folder, so each file goes back to the same relative place.
fn plan_restore_from_layout(resource_path: &Path, backup_path: &Path) -> Result<RestorePlan> {
    let mut relative_files = Vec::new();
    collect_relative_files(backup_path, backup_path, &mut relative_files)?;
    relative_files.sort();

    let mut files = Vec::with_capacity(relative_files.len());
    for relative_path in relative_files {
        let source_path = backup_path.join(&relative_path);
        // Only the manifest says where an outside file belongs, and the
        // relative layout would put it inside the resource folder.
        if first_component_is(&relative_path, EXTERNAL_DIR) {
            return Err(RabbitError::BackupRestoreRefused {
                path: source_path,
                reason: "the set has no manifest, so RABBIT cannot tell where this file \
                         from outside the resource folder belongs"
                    .to_string(),
            });
        }
        let target_path = restore_target(resource_path, &relative_path, None, &[])?;
        files.push(PlannedRestore {
            package_id: None,
            size: fs::metadata(&source_path).with_path(&source_path)?.len(),
            sha256: sha256_file(&source_path)?,
            source_path,
            target_path,
        });
    }
    Ok(RestorePlan {
        files,
        skipped_targets: Vec::new(),
    })
}

/// Where a file goes back to, refusing anything outside the resource folder
/// and the outside folders RABBIT installs into, and anything in RABBIT's
/// own folder except the receipt.
///
/// `recorded_resource` is where the resource folder was when the set was
/// written. A portable REAPER can move (a USB drive gets another letter),
/// and a file recorded inside the old folder goes to the same place inside
/// the current one.
fn restore_target(
    resource_path: &Path,
    original_path: &Path,
    recorded_resource: Option<&Path>,
    outside_roots: &[PathBuf],
) -> Result<PathBuf> {
    let refuse = |reason: &str| RabbitError::BackupRestoreRefused {
        path: original_path.to_path_buf(),
        reason: reason.to_string(),
    };

    let inside_relative = if original_path.is_relative() {
        if !is_plain_relative(original_path) {
            return Err(refuse(
                "the path is not a plain path inside the resource folder",
            ));
        }
        Some(original_path.to_path_buf())
    } else {
        let resource = std::path::absolute(resource_path).with_path(resource_path)?;
        relative_within(original_path, &resource)
            .or_else(|| recorded_resource.and_then(|old| relative_within(original_path, old)))
            .filter(|relative| is_plain_relative(relative))
    };

    if let Some(relative) = inside_relative {
        if first_component_is(&relative, "RABBIT") && !same_path(&relative, RECEIPT_RELATIVE_PATH) {
            return Err(refuse("restore never writes into RABBIT's own folder"));
        }
        return Ok(resource_path.join(relative));
    }

    let in_outside_root = outside_roots.iter().any(|root| {
        relative_within(original_path, root).is_some_and(|relative| is_plain_relative(&relative))
    });
    if in_outside_root {
        return Ok(original_path.to_path_buf());
    }
    Err(refuse(
        "the path is outside the REAPER resource folder and every other folder RABBIT installs into",
    ))
}

/// The copy an entry names, always inside the set being restored. Entries
/// record absolute paths; when the set has moved with its resource folder,
/// the part after the recorded set folder still finds the copy.
fn restore_source(
    backup_path: &Path,
    recorded_backup_path: &Path,
    recorded_set: Option<&Path>,
) -> Result<PathBuf> {
    let relative = if recorded_backup_path.is_relative() {
        Some(recorded_backup_path.to_path_buf())
    } else {
        recorded_set.and_then(|set| relative_within(recorded_backup_path, set))
    };
    match relative {
        Some(relative)
            if is_plain_relative(&relative) && !same_path(&relative, BACKUP_MANIFEST_FILE) =>
        {
            Ok(backup_path.join(relative))
        }
        _ => Err(RabbitError::BackupRestoreRefused {
            path: recorded_backup_path.to_path_buf(),
            reason: "the manifest names a backup copy outside the backup set".to_string(),
        }),
    }
}

/// Where a set was when it recorded `backup_path`: the resource folder and
/// the set folder, read from the last `RABBIT/backups/<id>` in the path.
fn recorded_set_location(backup_path: &Path) -> Option<(PathBuf, PathBuf)> {
    let parts = comparable_components(backup_path)?;
    let components: Vec<Component<'_>> = backup_path.components().collect();
    let rabbit = fold_case("RABBIT");
    let backups = fold_case("backups");
    let start = (0..parts.len().saturating_sub(3))
        .rev()
        .find(|&index| parts[index] == rabbit && parts[index + 1] == backups)?;
    let resource: PathBuf = components[..start].iter().collect();
    let set: PathBuf = components[..start + 3].iter().collect();
    Some((resource, set))
}

fn restore_file(action: &mut RestoreBackupAction, backups: &mut BackupSetWriter) -> Result<()> {
    if let Some(parent) = action.target_path.parent() {
        fs::create_dir_all(parent).with_path(parent)?;
    }

    let temp_path = temporary_restore_path(&action.target_path);
    if temp_path.exists() {
        fs::remove_file(&temp_path).with_path(&temp_path)?;
    }

    fs::copy(&action.source_path, &temp_path).with_path(&temp_path)?;
    let staged_hash = sha256_file(&temp_path)?;
    if staged_hash != action.sha256 {
        let _ = fs::remove_file(&temp_path);
        return Err(RabbitError::HashMismatch {
            path: temp_path,
            expected: action.sha256.clone(),
            actual: staged_hash,
        });
    }

    // The path the dry run predicted can differ if another set took this
    // id meanwhile, so the report records where the copy really went.
    action.current_backup_path = if action.target_path.is_file() {
        match backups.copy_file(action.package_id.as_deref(), &action.target_path) {
            Ok(path) => Some(path),
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
        }
    } else {
        None
    };

    if action.target_path.exists() {
        fs::remove_file(&action.target_path).with_path(&action.target_path)?;
    }

    match fs::rename(&temp_path, &action.target_path) {
        Ok(()) => Ok(()),
        Err(source) => {
            if let Some(current_backup_path) = &action.current_backup_path
                && current_backup_path.is_file()
                && !action.target_path.exists()
            {
                let _ = fs::copy(current_backup_path, &action.target_path);
            }
            let _ = fs::remove_file(&temp_path);
            Err(RabbitError::Io {
                path: action.target_path.clone(),
                source,
            })
        }
    }
}

/// The files of a set, relative to it, leaving out the manifest, its
/// staging file and the origin notes, none of which are backed-up files.
fn collect_relative_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(current).with_path(current)? {
        let entry = entry.with_path(current)?;
        let path = entry.path();
        if path.is_dir() {
            collect_relative_files(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| RabbitError::BackupNotFound(root.to_path_buf()))?;
            if relative == Path::new(BACKUP_MANIFEST_FILE)
                || relative == Path::new(MANIFEST_STAGING_FILE)
                || relative.to_string_lossy().ends_with(ORIGIN_SIDECAR_SUFFIX)
            {
                continue;
            }
            files.push(relative.to_path_buf());
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

fn load_backup_manifest(backup_set: &Path) -> Result<Option<BackupManifest>> {
    let path = backup_set.join(BACKUP_MANIFEST_FILE);
    if !path.is_file() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).with_path(&path)?;
    let manifest = serde_json::from_str(&content).with_json_path(&path)?;
    Ok(Some(manifest))
}

fn backup_set_path(resource_path: &Path, backup_id: &str) -> Result<PathBuf> {
    if backup_id.is_empty()
        || backup_id == "."
        || backup_id == ".."
        || backup_id.contains('/')
        || backup_id.contains('\\')
    {
        return Err(RabbitError::InvalidBackupId(backup_id.to_string()));
    }

    Ok(backups_root(resource_path).join(backup_id))
}

fn backups_root(resource_path: &Path) -> PathBuf {
    resource_path.join("RABBIT").join("backups")
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

fn first_component_is(path: &Path, name: &str) -> bool {
    comparable_components(path)
        .and_then(|parts| parts.into_iter().next())
        .is_some_and(|first| first == fold_case(name))
}

fn same_path(left: &Path, right: &str) -> bool {
    let right = comparable_components(Path::new(right));
    right.is_some() && comparable_components(left) == right
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
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("restore-unix-{seconds}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{
        BACKUP_MANIFEST_FILE, BackupEntryState, BackupManifest, BackupManifestFile,
        BackupSetWriter, RestoreBackupActionKind, RestoreBackupOptions, list_backup_sets,
        load_backup_manifest, relative_within, restore_backup_set, restore_backup_set_within,
        save_backup_manifest,
    };
    use crate::error::RabbitError;
    use crate::hash::sha256_file;

    const APPLY: RestoreBackupOptions = RestoreBackupOptions {
        dry_run: false,
        allow_reaper_running: true,
    };

    fn write(path: &Path, contents: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
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
                reason: reason.to_string(),
                files,
                receipt_backup_path: None,
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

    #[test]
    fn restore_can_restore_receipt_backup() {
        let dir = tempdir().unwrap();
        let receipt = dir.path().join("RABBIT/install-state.json");
        fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        fs::write(
            &receipt,
            br#"{"schema_version":1,"packages":{"reapack":"new"}}"#,
        )
        .unwrap();
        let backup_receipt = dir
            .path()
            .join("RABBIT/backups/unix-1/RABBIT/install-state.json");
        fs::create_dir_all(backup_receipt.parent().unwrap()).unwrap();
        fs::write(
            &backup_receipt,
            br#"{"schema_version":1,"packages":{"reapack":"old"}}"#,
        )
        .unwrap();

        let report = restore_backup_set(
            dir.path(),
            "unix-1",
            &RestoreBackupOptions {
                dry_run: false,
                allow_reaper_running: true,
            },
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(&receipt).unwrap(),
            r#"{"schema_version":1,"packages":{"reapack":"old"}}"#
        );
        assert!(report.actions[0].current_backup_path.is_some());
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
        let mut writer = BackupSetWriter::new(&resource, "unix-1", "install-replacement");
        writer.copy_file(Some("app2clap"), &app2clap).unwrap();
        fs::write(&app2clap, b"new app2clap").unwrap();

        let report =
            restore_backup_set_within(&resource, "unix-1", &APPLY, std::slice::from_ref(&clap_dir))
                .unwrap();

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
        restore_backup_set_within(
            &resource,
            &before_restore_id,
            &APPLY,
            std::slice::from_ref(&clap_dir),
        )
        .unwrap();
        assert_eq!(fs::read(&app2clap).unwrap(), b"new app2clap");
    }

    /// Sets written by earlier versions store app2clap's copy under a
    /// made-up `CLAP/` folder with its absolute original path, as
    /// `unix-1782841261` on the maintainer's machine does. Restore follows
    /// the original path, not the copy's place in the set.
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
            "install-replacement",
            vec![legacy_entry(Some("app2clap"), &app2clap, &copy)],
        );

        restore_backup_set_within(&resource, "unix-1782841261", &APPLY, &[clap_dir]).unwrap();

        assert_eq!(fs::read(&app2clap).unwrap(), b"old app2clap");
        assert!(!resource.join("CLAP").exists());
    }

    /// Unattended sets record a path relative to the resource folder,
    /// install-replacement sets an absolute one. Both are on users' machines
    /// today, and both go back to the same place.
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
            "install-replacement",
            vec![
                legacy_entry(Some("osara"), Path::new("reaper-kb.ini"), &keymap_copy),
                legacy_entry(Some("reakontrol"), &plugin, &plugin_copy),
            ],
        );

        let report = restore_backup_set_within(&resource, "unix-1", &APPLY, &[]).unwrap();

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
            "install-replacement",
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

        restore_backup_set_within(&resource, "unix-1", &APPLY, &[]).unwrap();

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

        let error = restore_backup_set_within(&resource, "unix-1", &APPLY, &[]).unwrap_err();

        assert!(
            matches!(error, RabbitError::BackupRestoreRefused { .. }),
            "{error:?}"
        );
        assert_eq!(fs::read(&plugin).unwrap(), b"new");
        assert!(!resource.join("external").exists());
    }

    /// A manifest can only send files to the resource folder (but not
    /// RABBIT's own folder, except the receipt) and the outside folders
    /// RABBIT installs into. Anything else stops the restore before the
    /// first file is replaced.
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

        for refused in [
            std::path::absolute(dir.path().join("Elsewhere").join("stray.dll")).unwrap(),
            PathBuf::from("../stray.dll"),
            PathBuf::from("RABBIT/backups/unix-2/stray.dll"),
            PathBuf::from("RABBIT/locks/package-install.lock"),
            std::path::absolute(&clap_dir).unwrap(),
        ] {
            save_manifest(
                &set,
                "install-replacement",
                vec![
                    legacy_entry(Some("reapack"), &plugin, &plugin_copy),
                    legacy_entry(None, &refused, &stray_copy),
                ],
            );

            let error = restore_backup_set_within(
                &resource,
                "unix-1",
                &APPLY,
                std::slice::from_ref(&clap_dir),
            )
            .unwrap_err();

            assert!(
                matches!(error, RabbitError::BackupRestoreRefused { .. }),
                "{refused:?}: {error:?}"
            );
            assert_eq!(fs::read(&plugin).unwrap(), b"new", "{refused:?}");
        }
        assert!(!dir.path().join("Elsewhere").exists());
        assert!(!resource.join("RABBIT/locks/package-install.lock").exists());
    }

    /// A finished entry whose copy no longer matches its recorded hash is a
    /// damaged set, and restoring part of it would mix old and new files.
    #[test]
    fn a_damaged_copy_stops_the_restore_before_changing_anything() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let keymap = resource.join("reaper-kb.ini");
        let plugin = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
        write(&keymap, b"osara keymap");
        write(&plugin, b"new");
        let mut writer = BackupSetWriter::new(&resource, "unix-1", "install-replacement");
        writer.copy_file(Some("osara"), &keymap).unwrap();
        let plugin_copy = writer.copy_file(Some("reapack"), &plugin).unwrap();
        fs::write(&plugin_copy, b"bit rot").unwrap();
        fs::write(&keymap, b"newer keymap").unwrap();

        let error = restore_backup_set_within(&resource, "unix-1", &APPLY, &[]).unwrap_err();

        assert!(
            matches!(error, RabbitError::HashMismatch { .. }),
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

        let mut writer = BackupSetWriter::new(&resource, "unix-1", "install-replacement");
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
            restore_backup_set_within(&resource, "unix-1", &APPLY, std::slice::from_ref(&clap_dir))
                .unwrap();

        assert_eq!(fs::read(&app2clap).unwrap(), b"old app2clap");
        assert_eq!(fs::read(&keymap).unwrap(), b"own keymap");
        assert_eq!(report.actions.len(), 1);
        assert_eq!(report.skipped_targets, vec![keymap]);
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
        assert!(matches!(
            restore_backup_set(resource, "unix-2", &APPLY).unwrap_err(),
            RabbitError::Json { .. }
        ));
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
            matches!(error, RabbitError::PackageInstallInProgress { .. }),
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
}
