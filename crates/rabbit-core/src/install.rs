use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rabbit_platform::{FileIdentity, file_identity};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use crate::archive::{
    extract_bin_directory_from_seven_zip_archive, extract_user_plugin_from_archive,
};
use crate::artifact::{ArtifactKind, CachedArtifact};
use crate::disk_image::extract_user_plugin_from_disk_image;
use crate::error::{IoPathContext, RabbitError, Result};
use crate::hash::sha256_file;
use crate::model::{InstallationKind, Platform};
use crate::package::{
    InstallDestination, PACKAGE_FFMPEG, PackageSpec, builtin_package_specs, package_specs_by_id,
};
use crate::preflight::{
    PreflightOptions, PreflightReport, RunningProcess, run_install_preflight_with_processes,
    running_reaper_processes,
};
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::reaper_ini::{lang_pack_is_one_of, lang_pack_setting, switch_lang_pack_if_selected};
use crate::receipt::{
    InstallState, PackageReceipt, PackageReceiptParams, load_install_state, save_install_state,
    upsert_package_receipt,
};
use crate::restore_refusal::TargetRefusal;
use crate::rollback::{
    BackupSetWriter, FileDigest, KEYMAP_FILE, MoveFileError, MovedFile, OutsideRestoreRules,
    ReplacedPackageRecord, UPDATE_BACKUP_REASON, absolute_without_dots, is_keymap_place,
    lang_pack_file_at, lang_pack_names_ignore_case, link_at_place, receipt_file_path,
    receipt_place_by_text, same_place, update_backup_place,
};
use crate::uninstall::paths::{UninstallRoots, resolve_pattern};
use crate::version::Version;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallOptions {
    pub dry_run: bool,
    pub allow_reaper_running: bool,
    pub target_app_path: Option<PathBuf>,
    /// Chosen package variant per package id; see
    /// [`crate::package::PackageVariant`]. Empty means "package defaults".
    #[serde(default)]
    pub package_variants: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReport {
    pub resource_path: PathBuf,
    pub dry_run: bool,
    pub preflight: PreflightReport,
    pub receipt_written: bool,
    /// Always `None` now. Earlier versions copied RABBIT's whole receipt
    /// into the update's backup set and named the copy here. The set records
    /// each replaced package's own entry instead (see
    /// [`crate::rollback::BackupManifest::replaced`]). Kept so saved reports
    /// and `--json` output keep their shape.
    pub receipt_backup_path: Option<PathBuf>,
    pub backup_manifest_path: Option<PathBuf>,
    pub actions: Vec<InstallFileReport>,
    /// What became of REAPER's language for each package whose update moved
    /// a language pack file into its backup set while the new version
    /// installs one of its own, such as the Spanish pack's
    /// `es_ES.ReaperLangPack` after a switch to es_MX (see
    /// [`ReaperLanguageReport`]). Empty for every other install, and in
    /// reports from earlier versions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reaper_language: Vec<ReaperLanguageReport>,
}

/// REAPER's language after an update moved a language pack file into its
/// backup set, or in a dry run would. REAPER picks its language pack by
/// the file name `langpack=` under `[REAPER]` in `reaper.ini` gives, and
/// OSARA its own translation by that name too. So while the line names a
/// file that moved, it is set to the new version's own file, as going back
/// to an earlier version sets it, and the update's set records the value it
/// replaced (see [`ReplacedPackageRecord::langpack_before`]), so restoring
/// that earlier version puts it back. Another pack, `<>` or no line stays
/// as it is, and then there is no report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaperLanguageReport {
    pub package_id: String,
    /// The new version's language pack file, by name, as the language step
    /// writes it: what `langpack=` is set to.
    pub langpack: String,
    /// The `langpack=` value, exactly as written, that named a file that
    /// moved into the backup: the value [`Self::langpack`] replaced, or in a
    /// dry run would replace, or the one that stays. `None` when RABBIT
    /// couldn't read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub langpack_before: Option<String>,
    /// Whether `langpack=` was set to [`Self::langpack`], or in a dry run
    /// would be.
    #[serde(default)]
    pub switched: bool,
    /// Why RABBIT couldn't read or set `langpack=`. The new version is in
    /// place all the same, and the user can pick the language in REAPER's
    /// preferences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The install let the target's REAPER run (`allow_reaper_running`),
    /// so `langpack=` stays as it is, or in a dry run would: REAPER may
    /// write its own setting back when it quits. The user picks
    /// [`Self::langpack`] in REAPER's preferences once REAPER is closed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reaper_may_run: bool,
}

/// What an install did with one file, or in a dry run would do. Most
/// actions are about a file the new version installs. The ones
/// [`InstallFileAction::is_superseded_file`] names are about a file the
/// package's previous version installed and the new one doesn't: then
/// `source_path` and `target_path` are both where that file was, and `size`
/// and `sha256` describe it as far as RABBIT read it. Both are empty for a
/// file it didn't read, such as a link at its place, and `sha256` is empty
/// for one it couldn't hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallFileReport {
    pub package_id: String,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    /// Where the file the install replaced, or moved out of the way, is in
    /// the update's backup set.
    pub backup_path: Option<PathBuf>,
    pub action: InstallFileAction,
    pub size: u64,
    pub sha256: String,
    /// Why a file of the package's previous version stays where it is
    /// ([`InstallFileAction::LeftInPlace`]). `None` for every other action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// The same reason as a code, for the wizard to say in the user's
    /// language: `warning` is English. `None` for every other action, and
    /// in reports from earlier versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_reason: Option<LeftInPlaceReason>,
}

/// Why a file of a package's previous version stays where it is
/// ([`InstallFileAction::LeftInPlace`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LeftInPlaceReason {
    /// A symbolic link or junction stands at its place, which RABBIT never
    /// follows.
    Link,
    /// A folder stands at its place.
    Folder,
    /// It is REAPER's key map, which an update never moves.
    Keymap,
    /// No restore of the update's set could put it back at its place, such
    /// as one outside every folder RABBIT installs into.
    Refused,
    /// Another program has it open.
    InUse,
    /// RABBIT couldn't read it.
    Unreadable,
    /// It couldn't be moved for another reason. A reason a later version
    /// adds reads as this too.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallFileAction {
    WouldInstall,
    WouldReplace,
    WouldKeep,
    Installed,
    Replaced,
    Kept,
    /// A dry run: a file the package's previous version installed, which
    /// the new version doesn't, would move into the update's backup set.
    WouldMoveToBackup,
    /// A file the package's previous version installed, which the new
    /// version doesn't, moved into the update's backup set, so the earlier
    /// version stays whole there and can be restored.
    MovedToBackup,
    /// Such a file stays where it is, and `warning` says why: a link or a
    /// folder is at its place, it is REAPER's key map, a restore couldn't
    /// put it back from the set, or it couldn't be moved, as when another
    /// program holds it open. The package's new receipt entry doesn't list
    /// it, so RABBIT no longer keeps track of it, and the earlier version
    /// can't be restored. In a dry run, a file a real run would leave there
    /// too.
    LeftInPlace,
}

impl InstallFileAction {
    /// Whether the action is about a file the package's previous version
    /// installed and the new one doesn't, rather than a file it installs.
    pub fn is_superseded_file(self) -> bool {
        matches!(
            self,
            Self::WouldMoveToBackup | Self::MovedToBackup | Self::LeftInPlace
        )
    }
}

pub fn install_cached_artifacts(
    resource_path: &Path,
    artifacts: &[CachedArtifact],
    options: &InstallOptions,
) -> Result<InstallReport> {
    install_cached_artifacts_with_progress(
        resource_path,
        artifacts,
        options,
        &ProgressReporter::noop(),
    )
}

/// Like [`install_cached_artifacts`] but emits `InstallStarted` /
/// `InstallCompleted` [`ProgressEvent`]s around each per-package
/// iteration so a UI can render a "now copying X" line. The no-op
/// overload above keeps existing callers (tests, the CLI) on the
/// untouched call signature.
///
/// An update keeps the version it replaces in one backup set (see
/// [`BackupSetWriter`]): each file it replaces is copied in just before the
/// new one takes its place, and each file the package's previous version
/// installed that the new one doesn't is moved in once the new version's
/// files are all in place (see [`UpdateRun::back_up_superseded_files`]).
/// The package's receipt entry from before the update is recorded in the
/// set before its first file goes in either way.
pub fn install_cached_artifacts_with_progress(
    resource_path: &Path,
    artifacts: &[CachedArtifact],
    options: &InstallOptions,
    progress: &ProgressReporter,
) -> Result<InstallReport> {
    install_cached_artifacts_within(
        resource_path,
        artifacts,
        options,
        progress,
        &outside_install_roots,
        &|| running_reaper_processes(Platform::current()),
    )
}

/// [`install_cached_artifacts_with_progress`], with `outside_roots` as the
/// folders outside the resource folder that RABBIT installs into, and
/// `reapers` as every REAPER running on this computer, which the preflight
/// checks the target's among. The folders are asked only when a file of a
/// previous version is outside the resource folder. Tests give their own
/// of both, so they never touch the real CLAP folder and never depend on
/// what runs on the computer that tests them.
pub(crate) fn install_cached_artifacts_within(
    resource_path: &Path,
    artifacts: &[CachedArtifact],
    options: &InstallOptions,
    progress: &ProgressReporter,
    outside_roots: &dyn Fn() -> Vec<PathBuf>,
    reapers: &dyn Fn() -> Vec<RunningProcess>,
) -> Result<InstallReport> {
    let preflight = run_install_preflight_with_processes(
        resource_path,
        &PreflightOptions {
            dry_run: options.dry_run,
            allow_reaper_running: options.allow_reaper_running,
            target_app_path: options.target_app_path.clone(),
        },
        &reapers(),
    );
    if !preflight.passed {
        return Err(RabbitError::PreflightFailed {
            message: preflight.failure_message(),
        });
    }

    let timestamp = install_timestamp();
    let mut report = InstallReport {
        resource_path: resource_path.to_path_buf(),
        dry_run: options.dry_run,
        preflight,
        receipt_written: false,
        receipt_backup_path: None,
        backup_manifest_path: None,
        actions: Vec::new(),
        reaper_language: Vec::new(),
    };

    let mut state = load_install_state(resource_path)?.unwrap_or_default();
    // Created on the first file this run replaces or moves out of the way,
    // so a run that only adds files leaves no empty set behind. A dry run
    // plans the same paths without writing anything. One set holds every
    // package this call keeps files of, and records each of them (see
    // `UpdateBackup`).
    let mut replacement_backups: Option<BackupSetWriter> = None;
    let run = UpdateRun::new(resource_path, &timestamp, options.dry_run, outside_roots);
    let new_backup_set = || run.new_backup_set();
    // What the packages before each one did this run, so a dry run judges
    // each package as the real run would.
    let mut places = RunPlaces::default();

    for artifact in artifacts {
        progress.report(ProgressEvent::InstallStarted {
            package_id: artifact.descriptor.package_id.clone(),
        });
        let package_id = artifact.descriptor.package_id.as_str();
        // The package's entry from before this install. The update's set
        // records it before the first file of the package goes in.
        let previous = state.packages.get(package_id).cloned();
        let prepared = prepare_install_source(artifact)?;
        let mut artifact_target_paths = Vec::with_capacity(prepared.files.len());
        // Install destination is per-package data: the package's root-level
        // `install_destination`, defaulting to UserPlugins for everything
        // that doesn't declare one. Root-level so it applies to every
        // artifact source, not just GitHub releases — language packs resolve
        // through `http_artifact` and install to `LangPack/`.
        let install_spec = lookup_install_spec(
            &artifact.descriptor.package_id,
            artifact.descriptor.platform,
        )
        .ok();
        let destination = install_spec
            .as_ref()
            .map(|spec| spec.install_destination)
            .unwrap_or_default();
        // `install_as` renames the installed file, because REAPER selects a
        // language pack BY FILE NAME (and OSARA reads that name to pick its
        // own translation). Guarded to single-file packages: applying one
        // name to a multi-file package — FFmpeg ships a whole set of DLLs —
        // would collapse them all onto the same target.
        // Explicit choice for this run, else what the last install recorded,
        // else the manifest default — so an update keeps the variant the
        // user picked rather than reverting them to the default.
        let effective_variant = crate::package::effective_variant_id(
            resource_path,
            &artifact.descriptor.package_id,
            &options.package_variants,
        );
        let install_as = install_spec
            .as_ref()
            .and_then(|spec| {
                crate::package::resolved_install_as(spec, effective_variant.as_deref())
            })
            .filter(|_| prepared.files.len() == 1);
        // The variant this install puts in place, for a package that comes
        // in variants: the update's set records it with the version, so a
        // switch of variant at the same version reads apart.
        let new_variant = install_spec
            .as_ref()
            .and_then(|spec| crate::package::recorded_variant(spec, effective_variant.as_deref()))
            .map(|variant| variant.id.clone());

        for file in &prepared.files {
            let target_path = resolve_install_target(
                destination,
                &artifact.descriptor.package_id,
                resource_path,
                install_as.as_deref().unwrap_or(&file.target_file_name),
            )?;
            // In a dry run, a file an earlier package of the run would have
            // moved into the set is still there, but the real run finds it
            // gone and installs this one as a new file.
            let moved_earlier = options.dry_run && places.moved.contains(&run.place(&target_path));
            let target_exists = !moved_earlier && target_path.is_file();
            let target_matches = target_exists && sha256_file(&target_path)? == file.source_sha256;
            let replaces = target_exists && !target_matches;

            let backup_path = if options.dry_run {
                if replaces {
                    let backups = replacement_backups.get_or_insert_with(new_backup_set);
                    Some(backups.copy_file(Some(package_id), &target_path)?)
                } else {
                    None
                }
            } else if target_matches {
                None
            } else {
                let backups = if replaces {
                    Some(UpdateBackup {
                        writer: replacement_backups.get_or_insert_with(new_backup_set),
                        package_id,
                        replaced: ReplacedPackageRecord {
                            receipt: previous.clone(),
                            replaced_by: Some(artifact.descriptor.version.clone()),
                            replaced_by_variant: new_variant.clone(),
                            langpack_before: None,
                        },
                    })
                } else {
                    None
                };
                install_extension_file(
                    &file.source_path,
                    &file.source_sha256,
                    &target_path,
                    backups,
                )?
            };

            report.actions.push(InstallFileReport {
                package_id: package_id.to_string(),
                source_path: file.source_path.clone(),
                target_path: target_path.clone(),
                backup_path,
                action: classify_action(options.dry_run, target_exists, target_matches),
                size: file.source_size,
                sha256: file.source_sha256.clone(),
                warning: None,
                left_reason: None,
            });
            artifact_target_paths.push(target_path);
        }

        if !options.dry_run {
            upsert_package_receipt(
                &mut state,
                resource_path,
                PackageReceiptParams {
                    variant: effective_variant.as_deref(),
                    channel: artifact.descriptor.channel.as_deref(),
                    package_id: &artifact.descriptor.package_id,
                    version: Some(artifact.descriptor.version.clone()),
                    source_url: Some(artifact.descriptor.url.clone()),
                    source_sha256: Some(artifact.sha256.clone()),
                    installed_paths: &artifact_target_paths,
                    installed_at: Some(install_timestamp()),
                    architecture: Some(artifact.descriptor.architecture),
                },
            )?;
        }
        // The new version's files are all in place now, so the files only
        // the previous version had go into the set; a dry run plans it.
        // They come after the package's own install actions, which the
        // package operation relies on (see `operation::mod`).
        let superseded = run.back_up_superseded_files(
            &UpdatedPackage {
                package_id,
                previous: previous.as_ref(),
                version: &artifact.descriptor.version,
                variant: new_variant.as_deref(),
                installed: &artifact_target_paths,
            },
            &state,
            &places,
            &mut replacement_backups,
        );
        places.record(&run, package_id, &artifact_target_paths, &superseded);
        // REAPER's language follows a language pack file that moved, while
        // it names it, as the files on disk are now.
        report.reaper_language.extend(run.follow_language(
            package_id,
            &artifact_target_paths,
            &superseded,
            options.allow_reaper_running,
            &mut replacement_backups,
        ));
        report.actions.extend(superseded);
        if !options.dry_run {
            remove_exclusive_group_siblings(
                &mut state,
                resource_path,
                package_id,
                &artifact_target_paths,
            );
        }
        progress.report(ProgressEvent::InstallCompleted {
            package_id: artifact.descriptor.package_id.clone(),
        });
    }

    // No copy of the receipt joins the set: it records each replaced
    // package's own entry from before, and a restore never copies a receipt
    // back, which would undo every change to it since. A set whose only
    // file couldn't be moved in after all is gone again, and isn't named.
    if let Some(backups) = &replacement_backups
        && backups.holds_files()
    {
        report.backup_manifest_path = Some(backups.manifest_path());
    }
    if !options.dry_run && !artifacts.is_empty() {
        save_install_state(resource_path, &state)?;
        report.receipt_written = true;
    }

    Ok(report)
}

/// What one install call shares between its packages for the update's
/// backup set.
struct UpdateRun<'a> {
    resource_path: &'a Path,
    /// `resource_path` made absolute, with no `.` or `..` in it, so places
    /// compare by any spelling also when it was given as `.` (see
    /// [`Self::place`]).
    resource: PathBuf,
    /// The id the set asks for (see [`BackupSetWriter::new`]).
    timestamp: &'a str,
    dry_run: bool,
    /// The folders outside the resource folder that RABBIT installs into
    /// (see [`outside_install_roots`]).
    outside_roots: &'a dyn Fn() -> Vec<PathBuf>,
    /// Where outside the resource folder a file of a previous version may
    /// come back to from the set, read the first time one is outside it.
    outside: OnceCell<OutsideRestoreRules>,
}

/// One package of an install, as moving the files of its previous version
/// out of the way needs it.
struct UpdatedPackage<'a> {
    package_id: &'a str,
    /// The package's receipt entry from before this install.
    previous: Option<&'a PackageReceipt>,
    /// The version this install puts in place.
    version: &'a Version,
    /// Its variant, by id, for a package that comes in variants.
    variant: Option<&'a str>,
    /// Where this install put the new version's files, or in a dry run
    /// would.
    installed: &'a [PathBuf],
}

/// What the packages an install call handled so far did to places, each
/// as [`UpdateRun::place`] gives it. A real run has the new versions'
/// entries in RABBIT's records in memory and finds the moved files gone; a
/// dry run changes neither, so both judge the next package by this.
#[derive(Default)]
struct RunPlaces {
    /// Each package handled so far, with the places of its new version's
    /// files.
    installed: BTreeMap<String, BTreeSet<Vec<String>>>,
    /// The places of files moved into the set, or in a dry run planned to,
    /// that no package installed again since.
    moved: BTreeSet<Vec<String>>,
}

impl RunPlaces {
    /// Note what the install of `package_id` did: it put its new version's
    /// files at `installed`, and `superseded` says which files of its
    /// previous version went into the set.
    fn record(
        &mut self,
        run: &UpdateRun<'_>,
        package_id: &str,
        installed: &[PathBuf],
        superseded: &[InstallFileReport],
    ) {
        let installed: BTreeSet<Vec<String>> =
            installed.iter().map(|path| run.place(path)).collect();
        self.moved.retain(|place| !installed.contains(place));
        self.moved.extend(
            superseded
                .iter()
                .filter(|action| {
                    matches!(
                        action.action,
                        InstallFileAction::MovedToBackup | InstallFileAction::WouldMoveToBackup
                    )
                })
                .map(|action| run.place(&action.target_path)),
        );
        self.installed.insert(package_id.to_string(), installed);
    }
}

impl<'a> UpdateRun<'a> {
    fn new(
        resource_path: &'a Path,
        timestamp: &'a str,
        dry_run: bool,
        outside_roots: &'a dyn Fn() -> Vec<PathBuf>,
    ) -> Self {
        Self {
            resource_path,
            resource: absolute_without_dots(resource_path)
                .unwrap_or_else(|_| resource_path.to_path_buf()),
            timestamp,
            dry_run,
            outside_roots,
            outside: OnceCell::new(),
        }
    }
}

impl UpdateRun<'_> {
    /// The update's set, made on its first file: a planning writer in a dry
    /// run, which writes nothing.
    fn new_backup_set(&self) -> BackupSetWriter {
        if self.dry_run {
            BackupSetWriter::planning(self.resource_path, self.timestamp, UPDATE_BACKUP_REASON)
        } else {
            BackupSetWriter::new(self.resource_path, self.timestamp, UPDATE_BACKUP_REASON)
        }
    }

    /// The places outside the resource folder that a restore of the set
    /// would put a file back to: below a folder RABBIT installs into, for
    /// any package. Only those, since only RABBIT's own file installs come
    /// here, never a vendor installer's files.
    fn outside(&self) -> &OutsideRestoreRules {
        self.outside.get_or_init(|| OutsideRestoreRules {
            shared_roots: (self.outside_roots)(),
            vendor_added: Vec::new(),
        })
    }

    /// `path` as places are compared: made absolute, with no `.` or `..`,
    /// then by [`same_place`], so two spellings of one place are equal.
    fn place(&self, path: &Path) -> Vec<String> {
        same_place(&absolute_without_dots(path).unwrap_or_else(|_| path.to_path_buf()))
    }

    /// [`Self::place`] of a path a receipt records: a relative one is
    /// inside the resource folder.
    fn receipt_place(&self, path: &Path) -> Vec<String> {
        self.place(&receipt_file_path(&self.resource, path))
    }

    /// Move each file `package`'s previous version installed, and the new
    /// version doesn't, into the update's set, `backups`, so the earlier
    /// version stays whole there and a restore can bring it back: an FFmpeg
    /// update whose DLLs have new names, or the Spanish language pack
    /// installed under another name after a switch from es_ES to es_MX. A
    /// dry run plans the same moves and changes nothing. Returns what
    /// became of each such file that is still there.
    ///
    /// Only files the package's own previous receipt entry lists with a
    /// hash are looked at: one recorded without a hash, such as
    /// `reaper.ini` or a folder, is the user's, as a restore treats it (see
    /// [`crate::rollback::files_only_in_newer`]). Never a file the new
    /// version installs, by any spelling of its place, nor one another
    /// package lists, as `places` and `state` say the run leaves them (see
    /// [`RunPlaces`]). A file the user changed since goes in too: it is the
    /// only copy. The set records the previous entry before the package's
    /// first file, exactly as for a file it replaces (see
    /// [`UpdateBackup`]), so a set that holds only moved files is a whole
    /// earlier version too.
    ///
    /// Called once the new version's files are all in place and its entry
    /// changed in memory, before the receipt is saved. So whatever happens
    /// from here, the package's new files are there and its old ones are
    /// on disk or in the set: each move saves its manifest entry as pending
    /// first and marks it done once the file is in the set (see
    /// [`BackupSetWriter::move_file`]). Should RABBIT stop before the
    /// receipt is saved, the receipt still lists the moved files, and the
    /// next update finds them gone. A file that isn't moved stays where it
    /// is, with a warning in its action, and never fails the install: the
    /// new version is in place already. The new entry doesn't list it, so
    /// RABBIT no longer keeps track of it, and that earlier version isn't
    /// complete.
    fn back_up_superseded_files(
        &self,
        package: &UpdatedPackage<'_>,
        state: &InstallState,
        places: &RunPlaces,
        backups: &mut Option<BackupSetWriter>,
    ) -> Vec<InstallFileReport> {
        let Some(previous) = package.previous else {
            return Vec::new();
        };
        let mut skipped: BTreeSet<Vec<String>> = package
            .installed
            .iter()
            .map(|path| self.place(path))
            .collect();
        // Every other package's files as the run leaves them: the new
        // version's of a package it handled already, else the ones its
        // receipt entry lists.
        for (id, entry) in &state.packages {
            if id != package.package_id && !places.installed.contains_key(id) {
                skipped.extend(
                    entry
                        .installed_files
                        .iter()
                        .map(|file| self.receipt_place(&file.path)),
                );
            }
        }
        for (id, installed) in &places.installed {
            if id != package.package_id {
                skipped.extend(installed.iter().cloned());
            }
        }
        let new_files: Vec<FileIdentity> = package
            .installed
            .iter()
            .filter_map(|path| file_identity(path).ok())
            .collect();
        let mut actions = Vec::new();
        for file in &previous.installed_files {
            // Each place once: it is skipped from now on.
            if file.sha256.is_none() || !skipped.insert(self.receipt_place(&file.path)) {
                continue;
            }
            actions.extend(self.back_up_superseded_file(package, &file.path, &new_files, backups));
        }
        actions
    }

    /// Move the file a package's previous receipt entry records at `path`
    /// into the set, as [`Self::back_up_superseded_files`] says. `None`
    /// when nothing is there, or the file there is one of `new_files`.
    fn back_up_superseded_file(
        &self,
        package: &UpdatedPackage<'_>,
        path: &Path,
        new_files: &[FileIdentity],
        backups: &mut Option<BackupSetWriter>,
    ) -> Option<InstallFileReport> {
        let recorded = receipt_file_path(self.resource_path, path);
        let left = |place: &Path, digest: Option<FileDigest>, reason, why: &str| {
            let mut action = superseded_action(
                package.package_id,
                place,
                None,
                InstallFileAction::LeftInPlace,
                digest.unwrap_or(FileDigest {
                    size: 0,
                    sha256: String::new(),
                }),
                Some(format!("{why} {UNTRACKED_NOTE}")),
            );
            action.left_reason = Some(reason);
            Some(action)
        };
        let nowhere_outside = OutsideRestoreRules::default();
        let outside = || {
            if path.is_absolute() {
                self.outside()
            } else {
                &nowhere_outside
            }
        };
        // The place must be one a restore of the set could write the file
        // back to, so the set never holds what it can't bring back. First
        // by the text alone, so a place no restore writes to, such as one
        // outside every folder RABBIT installs into or on another computer,
        // is never opened.
        if let Err(reason) =
            receipt_place_by_text(self.resource_path, path, package.package_id, outside)
        {
            return left(
                &recorded,
                None,
                LeftInPlaceReason::Refused,
                &refused_warning(&reason),
            );
        }
        let place =
            match update_backup_place(self.resource_path, path, package.package_id, outside()) {
                Ok(place) => place,
                // A link at the place is no file of the version, and it is
                // never followed: it stays, and the action says so.
                Err(reason) if link_at_place(&reason, self.resource_path, path) => {
                    return left(&recorded, None, LeftInPlaceReason::Link, LINK_WARNING);
                }
                // Refused by what is on the way, such as a link folder that
                // leads out of the resource folder.
                Err(reason) => {
                    return match fs::symlink_metadata(&recorded) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                        _ => left(
                            &recorded,
                            None,
                            LeftInPlaceReason::Refused,
                            &refused_warning(&reason),
                        ),
                    };
                }
            };
        let metadata = match fs::symlink_metadata(&place) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => {
                return left(
                    &place,
                    None,
                    LeftInPlaceReason::Unreadable,
                    &unreadable_warning(&error),
                );
            }
        };
        if metadata.file_type().is_symlink() {
            return left(&place, None, LeftInPlaceReason::Link, LINK_WARNING);
        }
        if !metadata.is_file() {
            return left(
                &place,
                None,
                LeftInPlaceReason::Folder,
                "It is a folder, not a file, so it isn't moved.",
            );
        }
        // REAPER's key map is never a file of a version: only a restore of
        // the key map puts it back, keeping the scripts registered since.
        if self.is_keymap(path, &place) {
            return left(&place, None, LeftInPlaceReason::Keymap, KEYMAP_WARNING);
        }
        // The same file as one the new version installs, under another
        // name, such as a short name on Windows. The spelling check above
        // already finds a name in another case.
        if file_identity(&place).is_ok_and(|identity| new_files.contains(&identity)) {
            return None;
        }
        let digest = match FileDigest::of(&place) {
            Ok(digest) => digest,
            Err(error) => {
                let size = FileDigest {
                    size: metadata.len(),
                    sha256: String::new(),
                };
                return left(
                    &place,
                    Some(size),
                    LeftInPlaceReason::Unreadable,
                    &unreadable_warning(&error),
                );
            }
        };
        let backup = UpdateBackup {
            writer: backups.get_or_insert_with(|| self.new_backup_set()),
            package_id: package.package_id,
            replaced: ReplacedPackageRecord {
                receipt: package.previous.cloned(),
                replaced_by: Some(package.version.clone()),
                replaced_by_variant: package.variant.map(str::to_string),
                langpack_before: None,
            },
        };
        match backup.move_in(&place, digest.clone()) {
            Ok(moved) => Some(superseded_action(
                package.package_id,
                &place,
                Some(moved.backup_path),
                if self.dry_run {
                    InstallFileAction::WouldMoveToBackup
                } else {
                    InstallFileAction::MovedToBackup
                },
                digest,
                None,
            )),
            Err(MoveFileError::InUse { .. }) => left(
                &place,
                Some(digest),
                LeftInPlaceReason::InUse,
                IN_USE_WARNING,
            ),
            Err(MoveFileError::NotMoved(error)) => left(
                &place,
                Some(digest),
                LeftInPlaceReason::Other,
                &format!("RABBIT couldn't move it into the backup: {error}."),
            ),
        }
    }

    /// Whether `place`, where a receipt records `path`, is REAPER's key
    /// map: by a name the file system reads as `reaper-kb.ini` in the
    /// resource folder, or as that same file under another name or
    /// through a link folder.
    fn is_keymap(&self, path: &Path, place: &Path) -> bool {
        (path.is_relative() && is_keymap_place(path))
            || file_identity(&self.resource_path.join(KEYMAP_FILE))
                .is_ok_and(|keymap| file_identity(place).is_ok_and(|identity| identity == keymap))
    }

    /// Make REAPER's language follow `package_id`'s language pack, as the
    /// files on disk are now (see [`ReaperLanguageReport`]): when
    /// `superseded` moved a language pack file out of REAPER's `LangPack`
    /// folder into the update's set, `installed` put one of the new
    /// version's there, and `langpack=` names a file that moved, the line is
    /// set to the new file, as going back to an earlier version sets it.
    /// Otherwise REAPER would point at a file that is only in the backup,
    /// and start without the language. The value it replaces goes into the
    /// package's record in `backups` first, so restoring that earlier
    /// version puts it back exactly as it was written, and it goes again
    /// when the line didn't change. With `allow_reaper_running` the line
    /// stays, as a REAPER that may run writes its own setting back when it
    /// quits; a dry run plans the same and writes nothing. `None` when the
    /// language has nothing to do with it.
    fn follow_language(
        &self,
        package_id: &str,
        installed: &[PathBuf],
        superseded: &[InstallFileReport],
        allow_reaper_running: bool,
        backups: &mut Option<BackupSetWriter>,
    ) -> Option<ReaperLanguageReport> {
        let moved: Vec<String> = superseded
            .iter()
            .filter(|action| {
                matches!(
                    action.action,
                    InstallFileAction::MovedToBackup | InstallFileAction::WouldMoveToBackup
                )
            })
            .filter_map(|action| lang_pack_file_at(self.resource_path, &action.target_path))
            .collect();
        if moved.is_empty() {
            return None;
        }
        // A name with a control character in it would break the line.
        let langpack = installed
            .iter()
            .find_map(|path| lang_pack_file_at(self.resource_path, path))
            .filter(|name| !name.chars().any(char::is_control))?;
        let mut report = ReaperLanguageReport {
            package_id: package_id.to_string(),
            langpack,
            langpack_before: None,
            switched: false,
            error: None,
            reaper_may_run: false,
        };
        let ignore_case = lang_pack_names_ignore_case();
        let current = match lang_pack_setting(self.resource_path) {
            Ok(current) => current,
            Err(error) => {
                report.error = Some(error.to_string());
                return Some(report);
            }
        };
        let current =
            current.filter(|current| lang_pack_is_one_of(current, &moved, ignore_case))?;
        report.langpack_before = Some(current.clone());
        if allow_reaper_running {
            report.reaper_may_run = true;
            return Some(report);
        }
        if self.dry_run {
            report.switched = true;
            return Some(report);
        }
        // The set holds the moved file, so it has the package's record.
        let Some(writer) = backups.as_mut() else {
            report.error = Some(format!(
                "the update's backup has no record of {package_id} to keep REAPER's language \
                 setting in"
            ));
            return Some(report);
        };
        if let Err(error) = writer.set_replaced_langpack_before(package_id, Some(current.clone())) {
            report.error = Some(error.to_string());
            return Some(report);
        }
        match switch_lang_pack_if_selected(
            self.resource_path,
            &moved,
            &report.langpack,
            ignore_case,
        ) {
            Ok(Some(replaced)) => {
                // The value recorded just before is the same, unless the
                // line changed in between: then the record takes what was
                // replaced, as far as it can be saved. Either names a file
                // that moved.
                let _ = writer.set_replaced_langpack_before(package_id, Some(replaced.clone()));
                report.langpack_before = Some(replaced);
                report.switched = true;
            }
            // The line changed in between and names no file that moved.
            Ok(None) => {
                let _ = writer.set_replaced_langpack_before(package_id, None);
                return None;
            }
            Err(error) => {
                // Nothing to put back while the line is as it was. A write
                // that failed partway may have changed it, and then the
                // record keeps the value.
                if lang_pack_setting(self.resource_path).ok().flatten() == Some(current) {
                    let _ = writer.set_replaced_langpack_before(package_id, None);
                }
                report.error = Some(error.to_string());
            }
        }
        Some(report)
    }
}

/// Said after why a file of a package's previous version stays where it
/// is: the new receipt entry doesn't list it, and the update's set doesn't
/// hold it.
const UNTRACKED_NOTE: &str = "After the update, RABBIT no longer keeps track of it, and that earlier version can't be restored.";

/// Why a file of a package's previous version stays where it is, when a
/// symbolic link or junction stands at its place.
const LINK_WARNING: &str =
    "It is a symbolic link or junction, which RABBIT never follows, so it isn't moved.";

/// Why REAPER's key map stays where it is, should a package's previous
/// receipt entry list it.
const KEYMAP_WARNING: &str = "It is REAPER's key map, which an update never moves.";

/// Why a file of a package's previous version stays where it is, when
/// another program has it open.
const IN_USE_WARNING: &str =
    "Another program has it open, so RABBIT couldn't move it into the backup.";

/// Why a file of a package's previous version stays where it is, when no
/// restore of the set could put it back there, for `reason`.
fn refused_warning(reason: &TargetRefusal) -> String {
    format!(
        "RABBIT doesn't move it, because no restore of the backup could put it back there: {reason}."
    )
}

/// Why a file of a package's previous version that RABBIT couldn't read
/// stays where it is.
fn unreadable_warning(error: &dyn std::fmt::Display) -> String {
    format!("RABBIT couldn't read it, so it isn't moved: {error}.")
}

/// The action of a file of a package's previous version at `place`.
fn superseded_action(
    package_id: &str,
    place: &Path,
    backup_path: Option<PathBuf>,
    action: InstallFileAction,
    digest: FileDigest,
    warning: Option<String>,
) -> InstallFileReport {
    InstallFileReport {
        package_id: package_id.to_string(),
        source_path: place.to_path_buf(),
        target_path: place.to_path_buf(),
        backup_path,
        action,
        size: digest.size,
        sha256: digest.sha256,
        warning,
        left_reason: None,
    }
}

/// After installing `package_id`, remove any package in the same exclusive
/// group that RABBIT previously installed: its files come off disk and its
/// receipt is dropped. REAPER loads exactly one language pack, so switching
/// from German to Spanish — or between the two Spanish OSARA variants —
/// must not leave the old `.ReaperLangPack` behind.
///
/// Deliberately receipt-driven: only files RABBIT recorded installing are
/// removed, so a language pack the user put in `LangPack/` by hand is never
/// touched. Paths the new install just wrote are skipped, so a sibling that
/// happens to share a target file name can't delete the file we just put
/// there. Best-effort — a file that won't delete (locked, already gone)
/// leaves the receipt alone so a later run retries, and never fails the
/// install that already succeeded.
///
/// Unlike the files of the package's own previous version, which go into
/// the update's set (see [`UpdateRun::back_up_superseded_files`]), a
/// sibling's files are deleted with no backup. No package declares an
/// exclusive group any more: the language packs coexist, and a switch
/// between the Spanish variants is the same package's own previous file.
fn remove_exclusive_group_siblings(
    state: &mut InstallState,
    resource_path: &Path,
    package_id: &str,
    just_installed: &[PathBuf],
) {
    remove_exclusive_group_siblings_in(
        state,
        resource_path,
        &crate::package::exclusive_group_siblings(package_id),
        package_id,
        just_installed,
    );
}

/// Mechanics of [`remove_exclusive_group_siblings`] with the sibling list
/// passed in, so the removal rules are testable without depending on the
/// shipped manifest's grouping.
fn remove_exclusive_group_siblings_in(
    state: &mut InstallState,
    resource_path: &Path,
    siblings: &[String],
    package_id: &str,
    just_installed: &[PathBuf],
) {
    for sibling in siblings {
        let Some(receipt) = state.packages.get(sibling) else {
            continue;
        };
        let mut all_removed = true;
        for file in &receipt.installed_files {
            // Receipt paths are relative to the resource path when the file
            // lives inside it, and absolute otherwise (e.g. the per-user CLAP
            // folder) — resolve before touching the filesystem, or removal
            // silently targets the process working directory instead.
            let path = if file.path.is_absolute() {
                file.path.clone()
            } else {
                resource_path.join(&file.path)
            };
            if just_installed.iter().any(|installed| installed == &path) {
                continue;
            }
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    eprintln!(
                        "warning: could not remove {} superseded by {package_id}: {error}",
                        path.display()
                    );
                    all_removed = false;
                }
            }
        }
        if all_removed {
            state.packages.remove(sibling);
        }
    }
}

/// Resolve where a package's file installs from its declared
/// [`InstallDestination`]. The default is `<resource>/UserPlugins/<file>`;
/// `WindowsClapDir` packages (app2clap) install into the per-user CLAP
/// folder (`%LOCALAPPDATA%\Programs\Common\CLAP`), outside the REAPER
/// resource path. A replaced copy of an outside file goes under the backup
/// set's `external/` folder (see [`BackupSetWriter`]).
fn resolve_install_target(
    destination: InstallDestination,
    package_id: &str,
    resource_path: &Path,
    file_name: &str,
) -> Result<PathBuf> {
    match destination {
        InstallDestination::WindowsClapDir => {
            let clap_dir = rabbit_platform::windows_clap_dir().ok_or_else(|| {
                RabbitError::InstallLocationUnavailable {
                    package_id: package_id.to_string(),
                    reason: "the Windows CLAP folder (%LOCALAPPDATA%\\Programs\\Common\\CLAP) \
                             could not be resolved"
                        .to_string(),
                }
            })?;
            Ok(clap_dir.join(file_name))
        }
        // REAPER reads translations from <resource>/LangPack/. Inside the
        // resource path, so portable installs work unchanged.
        InstallDestination::LangPack => Ok(resource_path.join("LangPack").join(file_name)),
        InstallDestination::UserPlugins => Ok(resource_path.join("UserPlugins").join(file_name)),
    }
}

/// The folders outside any resource folder that RABBIT copies files into
/// itself, and so the outside places a backup restore may write any
/// package's files to. Vendor installers write elsewhere too, but RABBIT
/// never backs up what they replace; only an uninstall moves files of
/// theirs, which [`outside_restore_rules`] lets back for their package
/// alone. A new outside [`InstallDestination`] belongs here too.
pub(crate) fn outside_install_roots() -> Vec<PathBuf> {
    rabbit_platform::windows_clap_dir().into_iter().collect()
}

/// Where outside the resource folder at `resource` a restore may put files
/// back on this computer: the folders of [`outside_install_roots`] for any
/// package, and each built-in package's `vendor_added_files` patterns for
/// that package alone, such as the JAWS scripts' files in JAWS's settings
/// folder.
pub(crate) fn outside_restore_rules(resource: &Path) -> OutsideRestoreRules {
    let Some(platform) = Platform::current() else {
        return OutsideRestoreRules {
            shared_roots: outside_install_roots(),
            vendor_added: Vec::new(),
        };
    };
    outside_restore_rules_with(
        &builtin_package_specs(platform),
        platform,
        &UninstallRoots::for_target(platform, resource, None),
        outside_install_roots(),
    )
}

/// [`outside_restore_rules`] for these packages, with their paths' tokens
/// standing for `roots`. Only the plan for a standard REAPER can have
/// `vendor_added_files`, and a pattern whose token stands for nothing here
/// allows nothing.
pub(crate) fn outside_restore_rules_with(
    specs: &[PackageSpec],
    platform: Platform,
    roots: &UninstallRoots,
    shared_roots: Vec<PathBuf>,
) -> OutsideRestoreRules {
    let mut vendor_added = Vec::new();
    for spec in specs {
        let Some(files) = spec
            .uninstall
            .plan_for(platform, InstallationKind::Standard)
            .and_then(|plan| plan.remove.vendor_added_files.as_ref())
        else {
            continue;
        };
        for pattern in &files.files {
            let resolved = resolve_pattern(pattern, roots).unwrap_or_default();
            vendor_added.extend(resolved.into_iter().map(|found| (spec.id.clone(), found)));
        }
    }
    OutsideRestoreRules {
        shared_roots,
        vendor_added,
    }
}

fn classify_action(dry_run: bool, target_exists: bool, target_matches: bool) -> InstallFileAction {
    match (dry_run, target_exists, target_matches) {
        (true, false, _) => InstallFileAction::WouldInstall,
        (true, true, true) => InstallFileAction::WouldKeep,
        (true, true, false) => InstallFileAction::WouldReplace,
        (false, false, _) => InstallFileAction::Installed,
        (false, true, true) => InstallFileAction::Kept,
        (false, true, false) => InstallFileAction::Replaced,
    }
}

struct PreparedInstallFile {
    source_path: PathBuf,
    source_sha256: String,
    source_size: u64,
    target_file_name: String,
}

struct PreparedInstallSource {
    files: Vec<PreparedInstallFile>,
    _extraction_dir: Option<TempDir>,
}

fn prepare_install_source(artifact: &CachedArtifact) -> Result<PreparedInstallSource> {
    match artifact.descriptor.kind {
        ArtifactKind::ExtensionBinary => Ok(PreparedInstallSource {
            files: vec![PreparedInstallFile {
                source_path: artifact.path.clone(),
                source_sha256: artifact.sha256.clone(),
                source_size: artifact.size,
                target_file_name: artifact.descriptor.file_name.clone(),
            }],
            _extraction_dir: None,
        }),
        ArtifactKind::Archive => {
            let spec = lookup_install_spec(
                &artifact.descriptor.package_id,
                artifact.descriptor.platform,
            )?;
            let extraction_dir = TempDir::new().map_err(|source| RabbitError::Io {
                path: PathBuf::from("rabbit-archive-extract"),
                source,
            })?;
            let extracted =
                extract_user_plugin_from_archive(&artifact.path, &spec, extraction_dir.path())?;
            Ok(PreparedInstallSource {
                files: vec![prepared_install_file_from_extracted(extracted)?],
                _extraction_dir: Some(extraction_dir),
            })
        }
        ArtifactKind::SevenZipArchive => {
            // FFmpeg ships every runtime DLL under `bin/` in the
            // Gyan.dev (x64) and tordona/ffmpeg-win-arm64 (ARM64)
            // archives — extract all of them so REAPER's video
            // decoder gets every avformat / avcodec / sw* sibling
            // alongside the top-level prefix-matched DLL. SevenZipArchive
            // is the only kind whose extractor handles `.7z`; the
            // package-id guard is a safety net in case a future
            // package gets the same kind without bin/-style layout.
            if artifact.descriptor.package_id != PACKAGE_FFMPEG {
                return Err(RabbitError::UnsupportedArtifactKind {
                    package_id: artifact.descriptor.package_id.clone(),
                    kind: ArtifactKind::SevenZipArchive,
                });
            }
            let spec = lookup_install_spec(
                &artifact.descriptor.package_id,
                artifact.descriptor.platform,
            )?;
            let extraction_dir = TempDir::new().map_err(|source| RabbitError::Io {
                path: PathBuf::from("rabbit-7z-archive-extract"),
                source,
            })?;
            let extracted = extract_bin_directory_from_seven_zip_archive(
                &artifact.path,
                &spec,
                extraction_dir.path(),
            )?;
            Ok(PreparedInstallSource {
                files: extracted
                    .into_iter()
                    .map(prepared_install_file_from_extracted)
                    .collect::<Result<Vec<_>>>()?,
                _extraction_dir: Some(extraction_dir),
            })
        }
        ArtifactKind::DiskImage => {
            let spec = lookup_install_spec(
                &artifact.descriptor.package_id,
                artifact.descriptor.platform,
            )?;
            let extraction_dir = TempDir::new().map_err(|source| RabbitError::Io {
                path: PathBuf::from("rabbit-disk-image-extract"),
                source,
            })?;
            let extracted =
                extract_user_plugin_from_disk_image(&artifact.path, &spec, extraction_dir.path())?;
            Ok(PreparedInstallSource {
                files: vec![prepared_install_file_from_extracted(extracted)?],
                _extraction_dir: Some(extraction_dir),
            })
        }
        kind => Err(RabbitError::UnsupportedArtifactKind {
            package_id: artifact.descriptor.package_id.clone(),
            kind,
        }),
    }
}

fn prepared_install_file_from_extracted(
    extracted: crate::archive::ExtractedUserPlugin,
) -> Result<PreparedInstallFile> {
    let source_sha256 = sha256_file(&extracted.extracted_path)?;
    let source_size = fs::metadata(&extracted.extracted_path)
        .with_path(&extracted.extracted_path)?
        .len();
    Ok(PreparedInstallFile {
        source_path: extracted.extracted_path,
        source_sha256,
        source_size,
        target_file_name: extracted.file_name,
    })
}

fn lookup_install_spec(package_id: &str, platform: crate::model::Platform) -> Result<PackageSpec> {
    package_specs_by_id(platform)
        .remove(package_id)
        .ok_or_else(|| RabbitError::UnsupportedArtifactKind {
            package_id: package_id.to_string(),
            kind: ArtifactKind::Archive,
        })
}

/// Where an update keeps a file it is about to replace, or one of the
/// package's previous version that it moves out of the way, and what it
/// records about the file's package first.
struct UpdateBackup<'a> {
    writer: &'a mut BackupSetWriter,
    package_id: &'a str,
    /// The package's receipt entry from before this install and the version
    /// replacing it. The set records it before the package's first file.
    replaced: ReplacedPackageRecord,
}

impl UpdateBackup<'_> {
    /// Record the package, if the set hasn't yet, then copy `target_path`
    /// into the set. When the record or the copy fails before the set
    /// noted the file, the record goes again, and so does a set left empty.
    fn copy(self, target_path: &Path) -> Result<PathBuf> {
        let Self {
            writer,
            package_id,
            replaced,
        } = self;
        writer
            .record_replaced(package_id, replaced)
            .inspect_err(|_| writer.forget_replaced_without_entry(package_id))?;
        writer
            .copy_file(Some(package_id), target_path)
            .inspect_err(|_| writer.forget_replaced_without_entry(package_id))
    }

    /// Record the package, if the set hasn't yet, then move `place`, a file
    /// of the package's previous version whose size and hash are `digest`,
    /// into the set (see [`BackupSetWriter::move_file`]). Across volumes it
    /// goes as a checked copy, and the original is removed only after. When
    /// the record or the move fails, the file is still where it was; the
    /// record goes again if the set holds no file of the package, and so
    /// does a set left empty.
    fn move_in(
        self,
        place: &Path,
        digest: FileDigest,
    ) -> std::result::Result<MovedFile, MoveFileError> {
        let Self {
            writer,
            package_id,
            replaced,
        } = self;
        writer
            .record_replaced(package_id, replaced)
            .inspect_err(|_| writer.forget_replaced_without_entry(package_id))
            .map_err(MoveFileError::NotMoved)?;
        writer
            .move_file(Some(package_id), place, Some(digest), false)
            .inspect_err(|_| writer.forget_replaced_without_entry(package_id))
    }
}

/// Copy a verified file into place. With `backups`, the file it replaces is
/// copied into that backup set first, after the new file is staged and
/// checked, so a download that fails its check leaves no backup behind.
/// Returns where the replaced file's copy went.
fn install_extension_file(
    source_path: &Path,
    source_sha256: &str,
    target_path: &Path,
    backups: Option<UpdateBackup<'_>>,
) -> Result<Option<PathBuf>> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).with_path(parent)?;
    }

    let temp_path = temporary_target_path(target_path);
    if temp_path.exists() {
        fs::remove_file(&temp_path).with_path(&temp_path)?;
    }

    fs::copy(source_path, &temp_path).with_path(&temp_path)?;
    let staged_hash = sha256_file(&temp_path)?;
    if staged_hash != source_sha256 {
        let _ = fs::remove_file(&temp_path);
        return Err(RabbitError::HashMismatch {
            path: temp_path,
            expected: source_sha256.to_string(),
            actual: staged_hash,
        });
    }

    let backup_path = match backups {
        Some(backup) => match backup.copy(target_path) {
            Ok(path) => Some(path),
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
        },
        None => None,
    };

    // Removing the existing extension and renaming the new one into its place
    // are the two steps that touch the (possibly OS-protected) target file, so
    // both route their errors through `classify_install_write_error`, which
    // turns a macOS permission block into actionable guidance instead of a bare
    // "OS error 1".
    if target_path.exists()
        && let Err(source) = fs::remove_file(target_path)
    {
        return Err(classify_install_write_error(target_path, source));
    }

    match fs::rename(&temp_path, target_path) {
        Ok(()) => Ok(backup_path),
        Err(source) => {
            if let Some(backup_path) = &backup_path
                && backup_path.is_file()
                && !target_path.exists()
            {
                let _ = fs::copy(backup_path, target_path);
            }
            let _ = fs::remove_file(&temp_path);
            Err(classify_install_write_error(target_path, source))
        }
    }
}

/// Map a failed overwrite of an installed extension file to the clearest error.
///
/// On macOS, EPERM (errno 1, "operation not permitted") or EACCES (errno 13)
/// when replacing a file like `reaper_kontrol.dylib` is almost never REAPER
/// holding it open — that case is ETXTBSY (errno 26). It's a macOS
/// permission/modification gate: App Management (Sonoma+) refusing to let one
/// app modify another's installed file, an immutable file flag, or ownership.
/// Closing REAPER won't fix any of those, so we surface
/// [`RabbitError::MacOsWriteDenied`] (which points at Full Disk Access / App
/// Management) instead of a bare `Io` error. Every other error — and every
/// other platform — passes through as `Io`.
fn classify_install_write_error(path: &Path, source: std::io::Error) -> RabbitError {
    #[cfg(target_os = "macos")]
    {
        if matches!(source.raw_os_error(), Some(1) | Some(13)) {
            return RabbitError::MacOsWriteDenied {
                path: path.to_path_buf(),
                source,
            };
        }
    }
    crate::error::io_error_at(path.to_path_buf(), source)
}

fn temporary_target_path(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("extension");
    target_path.with_file_name(format!("{file_name}.rabbit-tmp"))
}

fn install_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix-{seconds}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{
        InstallFileAction, InstallOptions, UpdateBackup, install_cached_artifacts,
        install_extension_file, resolve_install_target,
    };
    use crate::artifact::{ArtifactDescriptor, ArtifactKind, CachedArtifact};
    use crate::error::RabbitError;
    use crate::hash::sha256_file;
    use crate::model::{Architecture, Platform};
    #[cfg(target_os = "windows")]
    use crate::package::PACKAGE_APP2CLAP;
    use crate::package::{InstallDestination, PACKAGE_OSARA, PACKAGE_REAKONTROL, PACKAGE_REAPACK};
    use crate::receipt::{
        InstallState, InstalledFileReceipt, PackageReceipt, load_install_state, save_install_state,
    };
    use crate::rollback::{
        BackupManifest, BackupSetWriter, ReplacedPackageRecord, UPDATE_BACKUP_REASON,
    };
    use crate::version::Version;

    #[test]
    fn install_target_defaults_to_userplugins() {
        let resource = PathBuf::from("/some/reaper");
        let target = resolve_install_target(
            InstallDestination::UserPlugins,
            PACKAGE_REAKONTROL,
            &resource,
            "reaper_kontrol.dll",
        )
        .expect("default UserPlugins target resolves");
        assert_eq!(
            target,
            resource.join("UserPlugins").join("reaper_kontrol.dll")
        );
    }

    /// Installing one member of an exclusive group removes the files and
    /// receipt of the previously-installed member (German -> Spanish), but
    /// never touches a pack the user placed in LangPack/ themselves, because
    /// removal is driven purely by RABBIT's own receipts.
    ///
    /// Uses RELATIVE receipt paths, which is what `build_installed_file_receipt`
    /// actually writes for files inside the resource path — an earlier version
    /// of this test invented absolute paths and so passed while the real
    /// removal silently resolved against the process working directory,
    /// dropping the receipt but leaving the file on disk.
    #[test]
    fn exclusive_group_removal_is_receipt_driven() {
        use crate::receipt::{InstalledFileReceipt, PackageReceipt};

        let dir = tempdir().unwrap();
        let resource = dir.path();
        let lang_dir = resource.join("LangPack");
        std::fs::create_dir_all(&lang_dir).unwrap();

        let rabbit_installed = lang_dir.join("DE_(+SWS).ReaperLangPack");
        let user_installed = lang_dir.join("Nederlands.ReaperLangPack");
        let newly_installed = lang_dir.join("es_ES.ReaperLangPack");
        for path in [&rabbit_installed, &user_installed, &newly_installed] {
            std::fs::write(path, b"x").unwrap();
        }

        let mut state = InstallState::default();
        state.packages.insert(
            "langpack-de".to_string(),
            PackageReceipt {
                id: "langpack-de".to_string(),
                version: None,
                source_url: None,
                source_sha256: None,
                variant: None,
                channel: None,
                installed_files: vec![InstalledFileReceipt {
                    // Relative, exactly as receipts store in-resource files.
                    path: PathBuf::from("LangPack").join("DE_(+SWS).ReaperLangPack"),
                    sha256: None,
                    size: None,
                }],
                installed_at: None,
                rabbit_version: None,
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );

        // Pretend langpack-de shares langpack-es's group. The real grouping
        // comes from the manifest; this asserts the removal mechanics.
        super::remove_exclusive_group_siblings_in(
            &mut state,
            resource,
            &["langpack-de".to_string()],
            "langpack-es",
            std::slice::from_ref(&newly_installed),
        );

        assert!(!rabbit_installed.exists(), "RABBIT-installed pack removed");
        assert!(
            !state.packages.contains_key("langpack-de"),
            "receipt dropped"
        );
        assert!(user_installed.exists(), "user's own pack untouched");
        assert!(newly_installed.exists(), "the new pack survives");
    }

    #[test]
    fn install_target_for_a_language_pack_is_the_langpack_folder() {
        // REAPER reads translations from <resource>/LangPack/ — verified
        // against real installs, which keep e.g. Deutsch.ReaperLangPack there.
        let resource = PathBuf::from("/some/reaper");
        let target = resolve_install_target(
            InstallDestination::LangPack,
            "langpack-es",
            &resource,
            "es_ES.ReaperLangPack",
        )
        .expect("LangPack target resolves");
        assert_eq!(
            target,
            resource.join("LangPack").join("es_ES.ReaperLangPack")
        );
        // Inside the resource path, so portable installs work unchanged.
        assert!(target.starts_with(&resource));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn install_target_for_app2clap_is_the_clap_folder() {
        let resource = PathBuf::from("C:/some/reaper");
        let target = resolve_install_target(
            InstallDestination::WindowsClapDir,
            PACKAGE_APP2CLAP,
            &resource,
            "app2clap.clap",
        )
        .expect("CLAP folder resolves on Windows");
        let clap_dir = rabbit_platform::windows_clap_dir().expect("CLAP dir on Windows");
        assert_eq!(target, clap_dir.join("app2clap.clap"));
        // The install lands outside the REAPER resource path entirely.
        assert!(!target.starts_with(&resource));
        // So a restore may write there, and nowhere else outside it.
        assert_eq!(super::outside_install_roots(), vec![clap_dir]);
    }

    #[test]
    fn macos_write_denied_message_points_to_full_disk_access() {
        // The user-facing message for a blocked dylib overwrite must steer the
        // user to the right macOS setting and make clear it is NOT the
        // "close REAPER" case. The variant exists on all platforms, so the
        // message is assertable everywhere.
        let err = RabbitError::MacOsWriteDenied {
            path: PathBuf::from(
                "/Users/x/Library/Application Support/REAPER/UserPlugins/reaper_kontrol.dylib",
            ),
            source: std::io::Error::from_raw_os_error(1),
        };
        let message = err.to_string();
        assert!(message.contains("Full Disk Access"), "got: {message}");
        assert!(message.contains("App Management"), "got: {message}");
        assert!(
            message.contains("not REAPER being open"),
            "the message must distinguish this from the in-use case; got: {message}"
        );
    }

    #[test]
    fn classifies_non_permission_errors_as_generic_io() {
        // ENOENT (errno 2) is never a permission block on any platform — it
        // stays a generic Io error.
        let path = std::path::Path::new("/tmp/reaper_kontrol.dylib");
        let err = super::classify_install_write_error(path, std::io::Error::from_raw_os_error(2));
        assert!(matches!(err, RabbitError::Io { .. }));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_macos_permission_errors_as_write_denied() {
        let path = std::path::Path::new(
            "/Users/x/Library/Application Support/REAPER/UserPlugins/reaper_kontrol.dylib",
        );
        // EPERM (1, App Management / immutable / privileged) and EACCES (13,
        // POSIX perms) both warrant the Settings guidance.
        for errno in [1, 13] {
            let err =
                super::classify_install_write_error(path, std::io::Error::from_raw_os_error(errno));
            assert!(
                matches!(err, RabbitError::MacOsWriteDenied { .. }),
                "errno {errno} on macOS should map to MacOsWriteDenied"
            );
        }
        // ETXTBSY (26) is the in-use case, handled by the close-REAPER path —
        // it must NOT be reported as a permission/Settings problem.
        let busy = super::classify_install_write_error(path, std::io::Error::from_raw_os_error(26));
        assert!(matches!(busy, RabbitError::Io { .. }));
    }

    #[test]
    fn installs_extension_binary_and_writes_receipt() {
        let dir = tempdir().unwrap();
        let artifact = cached_artifact(
            dir.path(),
            PACKAGE_REAPACK,
            "reaper_reapack-x64.dll",
            b"new",
        );

        let report = install_cached_artifacts(
            dir.path(),
            &[artifact],
            &InstallOptions {
                dry_run: false,
                allow_reaper_running: true,
                target_app_path: None,
                package_variants: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, InstallFileAction::Installed);
        assert!(
            dir.path()
                .join("UserPlugins/reaper_reapack-x64.dll")
                .is_file()
        );

        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert!(state.packages.contains_key(PACKAGE_REAPACK));
    }

    #[test]
    fn backs_up_existing_extension_before_replacing() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(plugins.join("reaper_reapack-x64.dll"), b"old").unwrap();
        let mut packages = BTreeMap::new();
        packages.insert(
            PACKAGE_REAPACK.to_string(),
            PackageReceipt {
                id: PACKAGE_REAPACK.to_string(),
                version: Some(Version::parse("1.2.5").unwrap()),
                source_url: Some("https://example.test/old.dll".to_string()),
                source_sha256: Some(sha256_file(&plugins.join("reaper_reapack-x64.dll")).unwrap()),
                variant: None,
                channel: None,
                installed_files: vec![InstalledFileReceipt {
                    path: PathBuf::from("UserPlugins/reaper_reapack-x64.dll"),
                    sha256: None,
                    size: Some(3),
                }],
                installed_at: Some("unix-old".to_string()),
                rabbit_version: Some("0.1.0".to_string()),
                architecture: Some(Architecture::X64),
                vendor_added_files: Vec::new(),
            },
        );
        save_install_state(
            dir.path(),
            &InstallState {
                schema_version: 1,
                packages: packages.clone(),
                declined_packages: Default::default(),
                applied_configuration: Default::default(),
            },
        )
        .unwrap();
        let artifact = cached_artifact(
            dir.path(),
            PACKAGE_REAPACK,
            "reaper_reapack-x64.dll",
            b"new",
        );

        let report = install_cached_artifacts(
            dir.path(),
            &[artifact],
            &InstallOptions {
                dry_run: false,
                allow_reaper_running: true,
                target_app_path: None,
                package_variants: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, InstallFileAction::Replaced);
        let backup = report.actions[0].backup_path.as_ref().unwrap();
        assert_eq!(fs::read(backup).unwrap(), b"old");
        // The set holds the replaced file and records ReaPack's entry from
        // before, not a copy of the whole receipt.
        assert_eq!(report.receipt_backup_path, None);
        let backup_manifest = update_set(&report);
        assert_eq!(backup_manifest.reason, "install-replacement");
        assert_eq!(backup_manifest.files.len(), 1);
        assert_eq!(backup_manifest.receipt_backup_path, None);
        assert_eq!(
            backup_manifest.replaced[PACKAGE_REAPACK],
            ReplacedPackageRecord {
                receipt: Some(packages[PACKAGE_REAPACK].clone()),
                replaced_by: Some(Version::parse("1.2.6").unwrap()),
                ..Default::default()
            }
        );
        assert_eq!(
            fs::read(plugins.join("reaper_reapack-x64.dll")).unwrap(),
            b"new"
        );
        let current_state = load_install_state(dir.path()).unwrap().unwrap();
        assert_eq!(
            current_state.packages[PACKAGE_REAPACK]
                .version
                .as_ref()
                .unwrap()
                .raw(),
            "1.2.6"
        );
        assert!(!plugins.join("reaper_reapack-x64.dll.rabbit-tmp").exists());
    }

    /// An update's backup takes the package back to the version before
    /// it: its file, and RABBIT's entry for it, exactly as it was before the
    /// update. A dry run changes nothing. The newer version goes into a
    /// backup of its own, which takes the package forward again.
    #[test]
    fn an_update_s_backup_goes_back_to_the_version_before() {
        use crate::rollback::{
            OutsideRestoreRules, RestoreBackupOptions, RestoreStatus, restore_backup_set_within,
        };

        let dir = tempdir().unwrap();
        let options = InstallOptions {
            dry_run: false,
            allow_reaper_running: true,
            target_app_path: None,
            package_variants: Default::default(),
        };
        let mut old = cached_artifact(
            dir.path(),
            PACKAGE_REAPACK,
            "reaper_reapack-x64.dll",
            b"old",
        );
        old.descriptor.version = Version::parse("1.2.5").unwrap();
        install_cached_artifacts(dir.path(), &[old], &options).unwrap();
        let before = load_install_state(dir.path()).unwrap().unwrap();
        let new = cached_artifact(
            dir.path(),
            PACKAGE_REAPACK,
            "reaper_reapack-x64.dll",
            b"new",
        );
        let report = install_cached_artifacts(dir.path(), &[new], &options).unwrap();
        assert_eq!(report.receipt_backup_path, None);
        assert!(update_set(&report).replaced.contains_key(PACKAGE_REAPACK));
        let set = report.backup_manifest_path.unwrap();
        let set_id = set.parent().unwrap().file_name().unwrap().to_str().unwrap();
        let plugin = dir
            .path()
            .join("UserPlugins")
            .join("reaper_reapack-x64.dll");
        let receipt = dir.path().join("RABBIT").join("install-state.json");
        let after = load_install_state(dir.path()).unwrap().unwrap();
        let receipt_now = fs::read(&receipt).unwrap();
        let restore = |dry_run: bool, set_id: &str| {
            restore_backup_set_within(
                dir.path(),
                set_id,
                &RestoreBackupOptions {
                    dry_run,
                    allow_reaper_running: true,
                    package_id: Some(PACKAGE_REAPACK.to_string()),
                },
                &OutsideRestoreRules::default(),
            )
            .unwrap()
        };

        let preview = restore(true, set_id);
        assert_eq!(preview.status, RestoreStatus::WouldRestore);
        assert_eq!(fs::read(&plugin).unwrap(), b"new");
        assert_eq!(fs::read(&receipt).unwrap(), receipt_now);

        let back = restore(false, set_id);
        assert_eq!(back.status, RestoreStatus::Restored);
        assert_eq!(fs::read(&plugin).unwrap(), b"old");
        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert_eq!(state.packages, before.packages);
        let swap = back.swap.as_ref().unwrap();
        assert_eq!(swap.version, Some(Version::parse("1.2.5").unwrap()));
        assert_eq!(
            swap.replaced_version,
            Some(Version::parse("1.2.6").unwrap())
        );
        assert_eq!(swap.entry, crate::rollback::SwappedEntry::Replaced);

        let forward = restore(false, back.before_restore_set_id.as_deref().unwrap());
        assert_eq!(forward.status, RestoreStatus::Restored);
        assert_eq!(fs::read(&plugin).unwrap(), b"new");
        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert_eq!(state.packages, after.packages);
    }

    /// The manifest of the update set `report` names.
    fn update_set(report: &super::InstallReport) -> BackupManifest {
        let path = report.backup_manifest_path.as_ref().expect("an update set");
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    const APPLY: InstallOptions = InstallOptions {
        dry_run: false,
        allow_reaper_running: true,
        target_app_path: None,
        package_variants: BTreeMap::new(),
    };

    /// An update over a copy RABBIT didn't install, which the receipt has no
    /// entry for, records that there was none, and the version that
    /// replaced it.
    #[test]
    fn an_update_over_a_copy_installed_by_hand_records_no_entry() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(plugins.join("reaper_reapack-x64.dll"), b"by hand").unwrap();
        let artifact = cached_artifact(
            dir.path(),
            PACKAGE_REAPACK,
            "reaper_reapack-x64.dll",
            b"new",
        );

        let report = install_cached_artifacts(dir.path(), &[artifact], &APPLY).unwrap();

        let manifest = update_set(&report);
        assert_eq!(
            manifest.replaced[PACKAGE_REAPACK],
            ReplacedPackageRecord {
                receipt: None,
                replaced_by: Some(Version::parse("1.2.6").unwrap()),
                ..Default::default()
            }
        );
        assert_eq!(
            fs::read(&manifest.files[0].backup_path).unwrap(),
            b"by hand"
        );
        assert_eq!(report.receipt_backup_path, None);
        assert!(
            manifest
                .files
                .iter()
                .all(|file| !file.original_path.starts_with("RABBIT"))
        );
    }

    /// One install of several packages, as `rabbit install-extension` makes,
    /// writes one set for the two whose files it replaces. It records each
    /// one's entry from before (one RABBIT installed, one by hand) and the
    /// version that replaced it, keeps no copy of the receipt, and leaves
    /// out OSARA, whose file it only added.
    #[test]
    fn one_install_records_every_package_it_replaced_in_one_set() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(plugins.join("reaper_reapack-x64.dll"), b"reapack 1.0").unwrap();
        fs::write(plugins.join("reaper_kontrol.dll"), b"reakontrol by hand").unwrap();
        let mut state = InstallState::default();
        let reapack_before = PackageReceipt {
            id: PACKAGE_REAPACK.to_string(),
            version: Some(Version::parse("1.0").unwrap()),
            source_url: None,
            source_sha256: None,
            variant: None,
            channel: None,
            installed_files: vec![InstalledFileReceipt {
                path: PathBuf::from("UserPlugins/reaper_reapack-x64.dll"),
                sha256: None,
                size: Some(11),
            }],
            installed_at: Some("unix-1".to_string()),
            rabbit_version: Some("0.4.4".to_string()),
            architecture: Some(Architecture::X64),
            vendor_added_files: Vec::new(),
        };
        state
            .packages
            .insert(PACKAGE_REAPACK.to_string(), reapack_before.clone());
        save_install_state(dir.path(), &state).unwrap();
        let artifacts = [
            cached_artifact(
                dir.path(),
                PACKAGE_REAPACK,
                "reaper_reapack-x64.dll",
                b"new",
            ),
            cached_artifact(dir.path(), PACKAGE_REAKONTROL, "reaper_kontrol.dll", b"new"),
            cached_artifact(dir.path(), PACKAGE_OSARA, "reaper_osara64.dll", b"new"),
        ];

        let report = install_cached_artifacts(dir.path(), &artifacts, &APPLY).unwrap();

        let manifest = update_set(&report);
        let sets = fs::read_dir(dir.path().join("RABBIT").join("backups"))
            .unwrap()
            .count();
        assert_eq!(sets, 1);
        let version = Some(Version::parse("1.2.6").unwrap());
        assert_eq!(
            manifest.replaced,
            BTreeMap::from([
                (
                    PACKAGE_REAPACK.to_string(),
                    ReplacedPackageRecord {
                        receipt: Some(reapack_before),
                        replaced_by: version.clone(),
                        ..Default::default()
                    }
                ),
                (
                    PACKAGE_REAKONTROL.to_string(),
                    ReplacedPackageRecord {
                        receipt: None,
                        replaced_by: version,
                        ..Default::default()
                    }
                ),
            ])
        );
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(manifest.receipt_backup_path, None);
        assert!(
            manifest
                .files
                .iter()
                .all(|file| !file.original_path.starts_with("RABBIT"))
        );
        assert_eq!(report.receipt_backup_path, None);
        let state = load_install_state(dir.path()).unwrap().unwrap();
        for package in [PACKAGE_REAPACK, PACKAGE_REAKONTROL, PACKAGE_OSARA] {
            assert_eq!(
                state.packages[package].version.as_ref().unwrap().raw(),
                "1.2.6"
            );
        }
    }

    /// What an update records of a package: no entry, and the version
    /// replacing it.
    fn replaced_by_1_2_6() -> ReplacedPackageRecord {
        ReplacedPackageRecord {
            receipt: None,
            replaced_by: Some(Version::parse("1.2.6").unwrap()),
            ..Default::default()
        }
    }

    /// Should `RABBIT` or `RABBIT/backups` become a link after the checks
    /// before the run, the update stops before it replaces the file: the
    /// old file stays byte for byte, the staged new one goes again, and
    /// nothing is written where the link leads.
    #[test]
    fn an_update_replaces_nothing_when_its_backup_would_go_behind_a_link() {
        for linked in [&["RABBIT"][..], &["RABBIT", "backups"][..]] {
            let dir = tempdir().unwrap();
            let resource = dir.path().join("REAPER");
            let target = resource.join("UserPlugins").join("reaper_reapack-x64.dll");
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, b"old").unwrap();
            let source = dir.path().join("reaper_reapack-x64.dll");
            fs::write(&source, b"new").unwrap();
            let link = linked
                .iter()
                .fold(resource.clone(), |path, name| path.join(name));
            fs::create_dir_all(link.parent().unwrap()).unwrap();
            let elsewhere = dir.path().join("Elsewhere");
            fs::create_dir_all(&elsewhere).unwrap();
            crate::uninstall::paths::link_folder(&link, &elsewhere);
            let mut writer = BackupSetWriter::new(&resource, "unix-1", UPDATE_BACKUP_REASON);

            let error = install_extension_file(
                &source,
                &sha256_file(&source).unwrap(),
                &target,
                Some(UpdateBackup {
                    writer: &mut writer,
                    package_id: PACKAGE_REAPACK,
                    replaced: replaced_by_1_2_6(),
                }),
            )
            .unwrap_err();

            assert!(
                matches!(&error, RabbitError::BackupPathIsLink { path } if *path == link),
                "{linked:?}: {error:?}"
            );
            assert_eq!(fs::read(&target).unwrap(), b"old");
            assert!(
                !target
                    .with_file_name("reaper_reapack-x64.dll.rabbit-tmp")
                    .exists()
            );
            assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0, "{linked:?}");
        }
    }

    /// When the first file of a package can't be copied into the set, the
    /// package's record goes again: a set the failure would leave empty is
    /// removed, and one that holds another package's file keeps only that
    /// package's record.
    #[test]
    fn a_first_copy_that_fails_leaves_no_record_of_its_package() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let plugins = resource.join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        let missing = plugins.join("reaper_reapack-x64.dll");
        let kontrol = plugins.join("reaper_kontrol.dll");
        fs::write(&kontrol, b"reakontrol by hand").unwrap();
        fn backup<'a>(writer: &'a mut BackupSetWriter, package_id: &'a str) -> UpdateBackup<'a> {
            UpdateBackup {
                writer,
                package_id,
                replaced: replaced_by_1_2_6(),
            }
        }

        let mut alone = BackupSetWriter::new(&resource, "unix-1", UPDATE_BACKUP_REASON);
        backup(&mut alone, PACKAGE_REAPACK)
            .copy(&missing)
            .unwrap_err();
        assert!(!alone.path().exists());

        let mut shared = BackupSetWriter::new(&resource, "unix-2", UPDATE_BACKUP_REASON);
        backup(&mut shared, PACKAGE_REAKONTROL)
            .copy(&kontrol)
            .unwrap();
        backup(&mut shared, PACKAGE_REAPACK)
            .copy(&missing)
            .unwrap_err();
        let manifest: BackupManifest =
            serde_json::from_str(&fs::read_to_string(shared.manifest_path()).unwrap()).unwrap();
        assert_eq!(
            manifest.replaced,
            BTreeMap::from([(PACKAGE_REAKONTROL.to_string(), replaced_by_1_2_6())])
        );
        assert_eq!(manifest.files.len(), 1);
    }

    #[test]
    fn dry_run_does_not_write_files_or_receipts() {
        let dir = tempdir().unwrap();
        let artifact = cached_artifact(
            dir.path(),
            PACKAGE_REAPACK,
            "reaper_reapack-x64.dll",
            b"new",
        );

        let report = install_cached_artifacts(
            dir.path(),
            &[artifact],
            &InstallOptions {
                dry_run: true,
                allow_reaper_running: false,
                target_app_path: None,
                package_variants: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, InstallFileAction::WouldInstall);
        assert!(
            !dir.path()
                .join("UserPlugins/reaper_reapack-x64.dll")
                .exists()
        );
        assert!(load_install_state(dir.path()).unwrap().is_none());
    }

    #[test]
    fn rejects_non_extension_binary_artifacts() {
        let dir = tempdir().unwrap();
        let mut artifact = cached_artifact(dir.path(), PACKAGE_OSARA, "osara.exe", b"installer");
        artifact.descriptor.kind = ArtifactKind::Installer;

        let error = install_cached_artifacts(
            dir.path(),
            &[artifact],
            &InstallOptions {
                dry_run: false,
                allow_reaper_running: true,
                target_app_path: None,
                package_variants: Default::default(),
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("not supported"));
    }

    #[test]
    fn hash_mismatch_does_not_replace_existing_extension() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        let target = plugins.join("reaper_reapack-x64.dll");
        fs::write(&target, b"old").unwrap();
        let mut artifact = cached_artifact(
            dir.path(),
            PACKAGE_REAPACK,
            "reaper_reapack-x64.dll",
            b"new",
        );
        artifact.sha256 = "wrong-hash".to_string();

        let error = install_cached_artifacts(
            dir.path(),
            &[artifact],
            &InstallOptions {
                dry_run: false,
                allow_reaper_running: true,
                target_app_path: None,
                package_variants: Default::default(),
            },
        )
        .unwrap_err();

        assert!(matches!(error, RabbitError::HashMismatch { .. }));
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert!(!plugins.join("reaper_reapack-x64.dll.rabbit-tmp").exists());
    }

    fn cached_artifact(
        root: &std::path::Path,
        package_id: &str,
        file_name: &str,
        contents: &[u8],
    ) -> CachedArtifact {
        let cache_dir = root.join("cache");
        fs::create_dir_all(&cache_dir).unwrap();
        let path = cache_dir.join(file_name);
        fs::write(&path, contents).unwrap();
        let sha256 = sha256_file(&path).unwrap();

        CachedArtifact {
            descriptor: ArtifactDescriptor {
                package_id: package_id.to_string(),
                version: Version::parse("1.2.6").unwrap(),
                platform: Platform::Windows,
                architecture: Architecture::X64,
                kind: ArtifactKind::ExtensionBinary,
                url: format!("https://example.test/{file_name}"),
                file_name: file_name.to_string(),
                channel: None,
            },
            path,
            size: contents.len() as u64,
            sha256,
            reused_existing_file: false,
        }
    }

    #[test]
    fn extracts_and_installs_user_plugin_from_archive_artifact() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let dir = tempdir().unwrap();
        let cache_dir = dir.path().join("cache");
        fs::create_dir_all(&cache_dir).unwrap();
        let archive_path = cache_dir.join("reaKontrol_windows_2026.2.16.100.cafef00d.zip");
        let plugin_bytes = b"reakontrol-binary-bytes";
        {
            let file = fs::File::create(&archive_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            writer.start_file("README.txt", options).unwrap();
            writer.write_all(b"docs").unwrap();
            writer.start_file("reaper_kontrol.dll", options).unwrap();
            writer.write_all(plugin_bytes).unwrap();
            writer.finish().unwrap();
        }
        let archive_sha = sha256_file(&archive_path).unwrap();
        let archive_size = fs::metadata(&archive_path).unwrap().len();

        let artifact = CachedArtifact {
            descriptor: ArtifactDescriptor {
                package_id: PACKAGE_REAKONTROL.to_string(),
                version: Version::parse("2026.2.16.100").unwrap(),
                platform: Platform::Windows,
                architecture: Architecture::Universal,
                kind: ArtifactKind::Archive,
                url: "https://example.test/reaKontrol_windows_2026.2.16.100.cafef00d.zip"
                    .to_string(),
                file_name: "reaKontrol_windows_2026.2.16.100.cafef00d.zip".to_string(),
                channel: None,
            },
            path: archive_path,
            size: archive_size,
            sha256: archive_sha.clone(),
            reused_existing_file: false,
        };

        let report = install_cached_artifacts(
            dir.path(),
            &[artifact],
            &InstallOptions {
                dry_run: false,
                allow_reaper_running: true,
                target_app_path: None,
                package_variants: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, InstallFileAction::Installed);
        assert_eq!(
            report.actions[0].target_path,
            dir.path().join("UserPlugins").join("reaper_kontrol.dll")
        );
        assert_eq!(report.actions[0].size, plugin_bytes.len() as u64);
        let target = dir.path().join("UserPlugins").join("reaper_kontrol.dll");
        assert_eq!(fs::read(&target).unwrap(), plugin_bytes);

        let state = load_install_state(dir.path()).unwrap().unwrap();
        let receipt = state.packages.get(PACKAGE_REAKONTROL).unwrap();
        assert_eq!(receipt.source_sha256.as_deref(), Some(archive_sha.as_str()));
        assert_eq!(receipt.installed_files.len(), 1);
        assert_eq!(
            receipt.installed_files[0].path,
            PathBuf::from("UserPlugins/reaper_kontrol.dll")
        );
    }

    #[test]
    fn dry_run_archive_artifact_reports_planned_target_without_writing() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("reaKontrol_mac_test.zip");
        {
            let file = fs::File::create(&archive_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            writer.start_file("reaper_kontrol.dylib", options).unwrap();
            writer.write_all(b"mac-bytes").unwrap();
            writer.finish().unwrap();
        }
        let archive_sha = sha256_file(&archive_path).unwrap();
        let archive_size = fs::metadata(&archive_path).unwrap().len();

        let artifact = CachedArtifact {
            descriptor: ArtifactDescriptor {
                package_id: PACKAGE_REAKONTROL.to_string(),
                version: Version::parse("2026.2.16.100").unwrap(),
                platform: Platform::MacOs,
                architecture: Architecture::Universal,
                kind: ArtifactKind::Archive,
                url: "https://example.test/reaKontrol_mac_test.zip".to_string(),
                file_name: "reaKontrol_mac_test.zip".to_string(),
                channel: None,
            },
            path: archive_path,
            size: archive_size,
            sha256: archive_sha,
            reused_existing_file: false,
        };

        let report = install_cached_artifacts(
            dir.path(),
            &[artifact],
            &InstallOptions {
                dry_run: true,
                allow_reaper_running: true,
                target_app_path: None,
                package_variants: Default::default(),
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, InstallFileAction::WouldInstall);
        assert_eq!(
            report.actions[0].target_path,
            dir.path().join("UserPlugins").join("reaper_kontrol.dylib")
        );
        assert!(!dir.path().join("UserPlugins").exists());
        assert!(load_install_state(dir.path()).unwrap().is_none());
    }
    /// A CLI install writes the receipt through load-modify-save, so the
    /// wizard's remembered opt-outs have to survive it. If an install ever
    /// rebuilt the receipt from scratch instead, a single `rabbit setup`
    /// would silently forget every package the user had turned down.
    #[test]
    fn installing_a_package_preserves_remembered_opt_outs() {
        let dir = tempdir().unwrap();
        crate::receipt::record_package_opt_outs(dir.path(), &["langpack-de".to_string()], &[])
            .unwrap();

        let mut state = load_install_state(dir.path()).unwrap().unwrap();
        state.packages.insert(
            "osara".to_string(),
            PackageReceipt {
                id: "osara".to_string(),
                version: None,
                variant: None,
                channel: None,
                source_url: None,
                source_sha256: None,
                installed_files: Vec::new(),
                installed_at: None,
                rabbit_version: None,
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );
        save_install_state(dir.path(), &state).unwrap();

        let after = load_install_state(dir.path()).unwrap().unwrap();
        assert!(after.packages.contains_key("osara"));
        assert!(
            after.declined_packages.contains("langpack-de"),
            "installing an unrelated package must not forget a declined one"
        );
    }
}

#[cfg(test)]
#[path = "install_superseded_tests.rs"]
mod superseded_tests;
