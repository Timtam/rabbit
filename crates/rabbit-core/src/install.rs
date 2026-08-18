use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use crate::archive::{
    extract_bin_directory_from_seven_zip_archive, extract_user_plugin_from_archive,
};
use crate::artifact::{ArtifactKind, CachedArtifact};
use crate::disk_image::extract_user_plugin_from_disk_image;
use crate::error::{IoPathContext, RabbitError, Result};
use crate::hash::sha256_file;
use crate::package::{InstallDestination, PACKAGE_FFMPEG, PackageSpec, package_specs_by_id};
use crate::preflight::{PreflightOptions, PreflightReport, run_install_preflight};
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::receipt::{
    InstallState, PackageReceiptParams, RECEIPT_RELATIVE_PATH, load_install_state, receipt_path,
    save_install_state, upsert_package_receipt,
};
use crate::rollback::{BackupManifest, BackupManifestFile, save_backup_manifest};

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
    pub receipt_backup_path: Option<PathBuf>,
    pub backup_manifest_path: Option<PathBuf>,
    pub actions: Vec<InstallFileReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallFileReport {
    pub package_id: String,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub action: InstallFileAction,
    pub size: u64,
    pub sha256: String,
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
pub fn install_cached_artifacts_with_progress(
    resource_path: &Path,
    artifacts: &[CachedArtifact],
    options: &InstallOptions,
    progress: &ProgressReporter,
) -> Result<InstallReport> {
    let preflight = run_install_preflight(
        resource_path,
        &PreflightOptions {
            dry_run: options.dry_run,
            allow_reaper_running: options.allow_reaper_running,
            target_app_path: options.target_app_path.clone(),
        },
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
    };

    let mut state = load_install_state(resource_path)?.unwrap_or_default();
    let mut replacement_backup_set: Option<PathBuf> = None;

    for artifact in artifacts {
        progress.report(ProgressEvent::InstallStarted {
            package_id: artifact.descriptor.package_id.clone(),
        });
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

        for file in &prepared.files {
            let install_target = resolve_install_target(
                destination,
                &artifact.descriptor.package_id,
                resource_path,
                install_as.as_deref().unwrap_or(&file.target_file_name),
            )?;
            let target_path = install_target.target_path;
            let target_exists = target_path.is_file();
            let target_matches = target_exists && sha256_file(&target_path)? == file.source_sha256;
            let backup_path = if target_exists && !target_matches {
                let backup_set = resource_path
                    .join("RABBIT")
                    .join("backups")
                    .join(&timestamp);
                replacement_backup_set.get_or_insert_with(|| backup_set.clone());
                Some(backup_set.join(&install_target.backup_relative))
            } else {
                None
            };

            let action = classify_action(options.dry_run, target_exists, target_matches);
            report.actions.push(InstallFileReport {
                package_id: artifact.descriptor.package_id.clone(),
                source_path: file.source_path.clone(),
                target_path: target_path.clone(),
                backup_path: backup_path.clone(),
                action,
                size: file.source_size,
                sha256: file.source_sha256.clone(),
            });

            if options.dry_run {
                artifact_target_paths.push(target_path);
                continue;
            }

            if !target_matches {
                install_extension_file(
                    &file.source_path,
                    &file.source_sha256,
                    &target_path,
                    backup_path.as_deref(),
                )?;
            }
            artifact_target_paths.push(target_path);
        }

        if !options.dry_run {
            // Files this package installed LAST time. Anything still listed
            // there but absent from the new set is stale — most visibly when
            // a language pack is reinstalled under a different name because
            // the user switched variant (es_ES -> es_MX), where the content
            // is identical and only the name changes.
            let previous_paths: Vec<PathBuf> = state
                .packages
                .get(&artifact.descriptor.package_id)
                .map(|receipt| {
                    receipt
                        .installed_files
                        .iter()
                        .map(|file| {
                            if file.path.is_absolute() {
                                file.path.clone()
                            } else {
                                resource_path.join(&file.path)
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            upsert_package_receipt(
                &mut state,
                resource_path,
                PackageReceiptParams {
                    variant: effective_variant.as_deref(),
                    package_id: &artifact.descriptor.package_id,
                    version: Some(artifact.descriptor.version.clone()),
                    source_url: Some(artifact.descriptor.url.clone()),
                    source_sha256: Some(artifact.sha256.clone()),
                    installed_paths: &artifact_target_paths,
                    installed_at: Some(install_timestamp()),
                    architecture: Some(artifact.descriptor.architecture),
                },
            )?;
            remove_superseded_own_files(&previous_paths, &artifact_target_paths);
            remove_exclusive_group_siblings(
                &mut state,
                resource_path,
                &artifact.descriptor.package_id,
                &artifact_target_paths,
            );
        }
        progress.report(ProgressEvent::InstallCompleted {
            package_id: artifact.descriptor.package_id.clone(),
        });
    }

    if !options.dry_run && !artifacts.is_empty() {
        if let Some(backup_set) = &replacement_backup_set {
            report.receipt_backup_path = backup_receipt_if_present(resource_path, backup_set)?;
            report.backup_manifest_path = Some(write_backup_manifest(
                backup_set,
                &timestamp,
                &report.actions,
                report.receipt_backup_path.as_ref(),
            )?);
        }
        save_install_state(resource_path, &state)?;
        report.receipt_written = true;
    } else if options.dry_run
        && let Some(backup_set) = &replacement_backup_set
    {
        let source_path = receipt_path(resource_path);
        if source_path.is_file() {
            report.receipt_backup_path = Some(backup_set.join(RECEIPT_RELATIVE_PATH));
        }
        report.backup_manifest_path = Some(backup_set.join(crate::rollback::BACKUP_MANIFEST_FILE));
    }

    Ok(report)
}

/// Remove files a package installed previously that it no longer installs.
/// Keeps a rename from orphaning the old file — switching the Spanish
/// language pack between its es_ES and es_MX variants reinstalls identical
/// content under a new name, so without this both would sit in `LangPack/`.
/// Best-effort and conservative: only paths the package's own receipt
/// recorded are considered, and anything in the new set is kept.
fn remove_superseded_own_files(previous: &[PathBuf], current: &[PathBuf]) {
    for path in previous {
        if current.iter().any(|kept| kept == path) {
            continue;
        }
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                eprintln!(
                    "warning: could not remove superseded {}: {error}",
                    path.display()
                );
            }
        }
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

/// Where a prepared file installs (`target_path`) and the path used to
/// stash a replaced copy under the backup set (`backup_relative`). Most
/// packages drop into `<resource>/UserPlugins/`, but a few install to a
/// fixed system location outside the REAPER resource path.
struct InstallTarget {
    target_path: PathBuf,
    backup_relative: PathBuf,
}

/// Resolve a package's install destination from its declared
/// [`InstallDestination`]. The default is `<resource>/UserPlugins/<file>`;
/// `WindowsClapDir` packages (app2clap) install into the per-user CLAP
/// folder (`%LOCALAPPDATA%\Programs\Common\CLAP`), outside the REAPER
/// resource path. For the off-resource case the backup-relative path is
/// synthesized under a `CLAP/` subfolder so a replaced copy stays inside the
/// backup set (joining an absolute path would otherwise escape it).
fn resolve_install_target(
    destination: InstallDestination,
    package_id: &str,
    resource_path: &Path,
    file_name: &str,
) -> Result<InstallTarget> {
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
            Ok(InstallTarget {
                target_path: clap_dir.join(file_name),
                backup_relative: PathBuf::from("CLAP").join(file_name),
            })
        }
        InstallDestination::LangPack => {
            // REAPER reads translations from <resource>/LangPack/. Inside the
            // resource path, so portable installs work unchanged.
            let relative = PathBuf::from("LangPack").join(file_name);
            Ok(InstallTarget {
                target_path: resource_path.join(&relative),
                backup_relative: relative,
            })
        }
        InstallDestination::UserPlugins => {
            let relative = PathBuf::from("UserPlugins").join(file_name);
            Ok(InstallTarget {
                target_path: resource_path.join(&relative),
                backup_relative: relative,
            })
        }
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

fn install_extension_file(
    source_path: &Path,
    source_sha256: &str,
    target_path: &Path,
    backup_path: Option<&Path>,
) -> Result<()> {
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

    if let Some(backup_path) = backup_path {
        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent).with_path(parent)?;
        }
        fs::copy(target_path, backup_path).with_path(backup_path)?;
    }

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
        Ok(()) => Ok(()),
        Err(source) => {
            if let Some(backup_path) = backup_path
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

fn backup_receipt_if_present(resource_path: &Path, backup_set: &Path) -> Result<Option<PathBuf>> {
    let source_path = receipt_path(resource_path);
    if !source_path.is_file() {
        return Ok(None);
    }

    let backup_path = backup_set.join(RECEIPT_RELATIVE_PATH);
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).with_path(parent)?;
    }
    fs::copy(&source_path, &backup_path).with_path(&backup_path)?;
    Ok(Some(backup_path))
}

fn write_backup_manifest(
    backup_set: &Path,
    created_at: &str,
    actions: &[InstallFileReport],
    receipt_backup_path: Option<&PathBuf>,
) -> Result<PathBuf> {
    let mut files = Vec::new();
    for action in actions {
        let Some(backup_path) = &action.backup_path else {
            continue;
        };

        files.push(BackupManifestFile {
            package_id: Some(action.package_id.clone()),
            original_path: action.target_path.clone(),
            backup_path: backup_path.clone(),
            size: fs::metadata(backup_path).with_path(backup_path)?.len(),
            sha256: sha256_file(backup_path)?,
        });
    }

    let receipt_backup_path = receipt_backup_path.cloned();
    if let Some(path) = &receipt_backup_path {
        files.push(BackupManifestFile {
            package_id: None,
            original_path: PathBuf::from(RECEIPT_RELATIVE_PATH),
            backup_path: path.clone(),
            size: fs::metadata(path).with_path(path)?.len(),
            sha256: sha256_file(path)?,
        });
    }

    save_backup_manifest(
        backup_set,
        &BackupManifest {
            schema_version: 1,
            rabbit_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: created_at.to_string(),
            reason: "install-replacement".to_string(),
            files,
            receipt_backup_path,
        },
    )
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
        InstallFileAction, InstallOptions, install_cached_artifacts, resolve_install_target,
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
            target.target_path,
            resource.join("UserPlugins").join("reaper_kontrol.dll")
        );
        assert_eq!(
            target.backup_relative,
            PathBuf::from("UserPlugins").join("reaper_kontrol.dll")
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
                installed_files: vec![InstalledFileReceipt {
                    // Relative, exactly as receipts store in-resource files.
                    path: PathBuf::from("LangPack").join("DE_(+SWS).ReaperLangPack"),
                    sha256: None,
                    size: None,
                }],
                installed_at: None,
                rabbit_version: None,
                architecture: None,
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
            target.target_path,
            resource.join("LangPack").join("es_ES.ReaperLangPack")
        );
        // Inside the resource path, so portable installs work unchanged.
        assert!(target.target_path.starts_with(&resource));
        assert_eq!(
            target.backup_relative,
            PathBuf::from("LangPack").join("es_ES.ReaperLangPack")
        );
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
        assert_eq!(target.target_path, clap_dir.join("app2clap.clap"));
        // The install lands outside the REAPER resource path entirely.
        assert!(!target.target_path.starts_with(&resource));
        // Replaced copies still get stashed inside the backup set.
        assert_eq!(
            target.backup_relative,
            PathBuf::from("CLAP").join("app2clap.clap")
        );
        assert!(target.backup_relative.is_relative());
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
                installed_files: vec![InstalledFileReceipt {
                    path: PathBuf::from("UserPlugins/reaper_reapack-x64.dll"),
                    sha256: None,
                    size: Some(3),
                }],
                installed_at: Some("unix-old".to_string()),
                rabbit_version: Some("0.1.0".to_string()),
                architecture: Some(Architecture::X64),
            },
        );
        save_install_state(
            dir.path(),
            &InstallState {
                schema_version: 1,
                packages,
                declined_packages: Default::default(),
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
        let receipt_backup = report.receipt_backup_path.as_ref().unwrap();
        let backed_up_state: InstallState =
            serde_json::from_str(&fs::read_to_string(receipt_backup).unwrap()).unwrap();
        assert_eq!(
            backed_up_state.packages[PACKAGE_REAPACK]
                .version
                .as_ref()
                .unwrap()
                .raw(),
            "1.2.5"
        );
        let backup_manifest_path = report.backup_manifest_path.as_ref().unwrap();
        let backup_manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(backup_manifest_path).unwrap()).unwrap();
        assert_eq!(backup_manifest["reason"], "install-replacement");
        assert_eq!(backup_manifest["files"].as_array().unwrap().len(), 2);
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
                source_url: None,
                source_sha256: None,
                installed_files: Vec::new(),
                installed_at: None,
                rabbit_version: None,
                architecture: None,
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
