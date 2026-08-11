mod jaws_scripts;
mod osara;
mod reaper;
mod surge_xt;
mod sws;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::artifact::{
    ArtifactDescriptor, ArtifactKind, CachedArtifact, DownloadHandle, expected_artifact_kind,
    resolve_latest_artifacts, spawn_download_pool,
};
use crate::detection::{
    default_standard_installation, detect_components, matching_user_plugin_files,
};
use crate::error::{IoPathContext, RabbitError};
use crate::hash::sha256_file;
use crate::install::{
    InstallFileReport, InstallOptions, InstallReport, install_cached_artifacts_with_progress,
};
use crate::model::{Architecture, ComponentDetection, Platform};
use crate::package::package_specs_by_id;
use crate::plan::PlanActionKind;
use crate::preflight::ensure_resource_path_ready;
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::receipt::{
    InstallState, PackageReceiptParams, RECEIPT_RELATIVE_PATH, load_install_state, receipt_path,
    save_install_state, upsert_package_receipt,
};
use crate::rollback::{BackupManifest, BackupManifestFile, save_backup_manifest};

use self::osara::osara_manual_steps;
use self::reaper::reaper_manual_steps;
use self::sws::sws_manual_steps;
use crate::upstream::{
    execute_planned_execution, verify_planned_execution_freshness, verify_planned_execution_paths,
};

const DEFAULT_UNATTENDED_INSTALL_MESSAGE: &str = "RABBIT ran the upstream installer unattended, verified the expected target paths, and updated the RABBIT receipt.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageOperationOptions {
    pub dry_run: bool,
    pub allow_reaper_running: bool,
    pub stage_unsupported: bool,
    pub replace_osara_keymap: bool,
    pub target_app_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_path: Option<PathBuf>,
    /// Packages whose plan-time `Keep` decision (installed version is
    /// already current) should be promoted to `Update` so the install
    /// pipeline actually reruns the vendor installer / file copy. Used by
    /// the wizard to honor an explicit user re-tick of an already-current
    /// row: the user opted in by checking the box, RABBIT shouldn't silently
    /// no-op just because the version matches.
    #[serde(default)]
    pub force_reinstall_packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageOperationReport {
    pub resource_path: PathBuf,
    pub dry_run: bool,
    pub install_report: Option<InstallReport>,
    pub receipt_backup_path: Option<PathBuf>,
    pub receipt_backup_manifest_path: Option<PathBuf>,
    pub items: Vec<PackageOperationItem>,
}

impl PackageOperationReport {
    /// True when any package failed to install or was skipped because a
    /// dependency (e.g. REAPER) failed. The operation installs everything
    /// it can and returns `Ok` even on partial failure, so callers use this
    /// to drive a "finished with errors" outcome (a non-zero CLI exit, an
    /// error-styled wizard result page).
    pub fn has_failures(&self) -> bool {
        self.items.iter().any(|item| {
            matches!(
                item.status,
                PackageOperationStatus::Failed | PackageOperationStatus::SkippedDependencyFailed
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageOperationItem {
    pub package_id: String,
    pub plan_action: PlanActionKind,
    pub status: PackageOperationStatus,
    pub artifact: ArtifactDescriptor,
    pub cached_artifact: Option<CachedArtifact>,
    pub install_action: Option<InstallFileReport>,
    pub backup_paths: Vec<PathBuf>,
    pub backup_manifest_path: Option<PathBuf>,
    pub planned_execution: Option<PlannedExecutionPlan>,
    pub manual_instruction: Option<ManualInstallInstruction>,
    /// Human-readable English description of what happened. Stable
    /// English form so the saved JSON report reads cleanly without a
    /// localizer. UIs that show the report in another locale should
    /// dispatch on [`PackageOperationItem::message_code`] instead and
    /// resolve a Fluent key, falling back to this field when the
    /// code's variant is unknown.
    pub message: String,
    /// Structured form of `message` so non-English UIs can render a
    /// localized version without re-parsing the English text.
    /// Construction sites set both fields together; the English text
    /// in `message` is the canonical fallback if the locale layer
    /// hasn't shipped a translation for the variant yet.
    #[serde(default)]
    pub message_code: PackageOperationMessage,
}

/// Structured operation-status messages. The wizard / CLI dispatches
/// on the variant to produce a localized string; the saved JSON
/// report serializes the variant for stable consumption by report
/// readers (CI smoke tests, support tooling, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", tag = "code")]
pub enum PackageOperationMessage {
    /// `extension_binary` install path completed: a single user-plugin
    /// DLL was placed into `UserPlugins`.
    ExtensionBinaryInstalled,
    /// Plan kept the installed version because it's current or newer
    /// than the latest upstream release.
    SkippedCurrent {
        installed_version: String,
        available_version: String,
    },
    /// Dry-run preview: this artifact would be downloaded and run
    /// unattended.
    DryRunWouldRunUnattended { artifact_kind: ArtifactKind },
    /// This RABBIT build doesn't yet implement the planned unattended
    /// execution path for this artifact kind, but the artifact was
    /// staged in the cache.
    DeferredUnattendedStaged { artifact_kind: ArtifactKind },
    /// This RABBIT build doesn't yet implement the planned unattended
    /// execution path for this artifact kind, and the artifact was
    /// not downloaded either.
    DeferredUnattendedNotStaged { artifact_kind: ArtifactKind },
    /// Generic post-install success: vendor installer ran, expected
    /// target paths verified, RABBIT receipt updated.
    #[default]
    UnattendedInstalled,
    /// OSARA-specific success: the install also replaced the user's
    /// `reaper-kb.ini` with the OSARA key map and backed up the
    /// previous one.
    OsaraUnattendedInstalledKeymapBackedUp,
    /// OSARA-specific success: the install replaced the user's
    /// `reaper-kb.ini` with the OSARA key map; no backup was needed
    /// because the previous file was missing or already matched.
    OsaraUnattendedInstalledKeymapReplaced,
    /// This package's download or install failed; the operation recorded
    /// the error and kept going. `error` is the canonical English error
    /// text (also surfaced to the user, localized where the UI can).
    /// `antivirus_block` flags the specific case where Windows security
    /// software blocked the file, so the UI can add the "how to allow it"
    /// guidance once for the whole run rather than parsing the error text.
    InstallFailed {
        error: String,
        #[serde(default)]
        antivirus_block: bool,
    },
    /// This package was skipped because `dependency` (a package it installs
    /// on top of, e.g. REAPER) failed or was skipped earlier in this run.
    SkippedDependencyFailed { dependency: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualInstallInstruction {
    pub title: String,
    pub steps: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlannedAutomationKind {
    VendorInstaller,
    ArchiveExtraction,
    DiskImageInstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlannedExecutionKind {
    LaunchInstallerExecutable,
    ExtractArchiveAndRunInstaller,
    ExtractArchiveAndCopyOsaraAssets,
    MountDiskImageAndRunInstaller,
    MountDiskImageAndCopyAppBundle,
    /// macOS: mount a `.dmg`, locate the `productbuild` `.pkg` inside via
    /// a glob (matched against the mounted volume root), invoke
    /// `/usr/sbin/installer -pkg <path> -target /` under elevation, then
    /// detach the image. Used by Surge XT, whose nightly DMG wraps a
    /// distribution `.pkg` rather than a flat app bundle.
    MountDiskImageAndRunPkgInstaller,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedExecutionPlan {
    pub kind: PlannedExecutionKind,
    pub artifact_location: String,
    pub program: Option<String>,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub verification_paths: Vec<PathBuf>,
    /// When `true`, the runner launches `program` through Windows UAC
    /// elevation (`ShellExecuteEx` with the `runas` verb) instead of plain
    /// `CreateProcess`. Required for vendor installers that declare
    /// `RequestExecutionLevel admin` — their `/S` silent path otherwise
    /// no-ops without ever popping a UAC prompt. Defaulted via serde so
    /// older saved reports still parse.
    #[serde(default)]
    pub requires_elevation: bool,
    /// Files the runner expects the installer to *rewrite* (mtime updated
    /// to "now" or later). Used to catch silent no-ops where the installer
    /// returns success but does not actually replace anything on disk —
    /// the JAWS-for-REAPER scripts NSIS package without admin elevation
    /// being the worked example. Plain
    /// [`PlannedExecutionPlan::verification_paths`] only check existence,
    /// which a prior install already satisfies; freshness paths reject the
    /// run when the file's mtime is older than `install_started_at`.
    #[serde(default)]
    pub freshness_paths: Vec<PathBuf>,
}

/// Per-package override used by `planned_execution_for_artifact` when a
/// specific (package, kind, platform) combination needs a different
/// `PlannedExecutionKind` or argument list than the generic Archive/DiskImage
/// fallbacks.
pub(super) struct PlannedExecutionOverride {
    pub(super) kind: PlannedExecutionKind,
    pub(super) arguments: Vec<String>,
    pub(super) use_cached_working_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageAutomationSupport {
    Direct,
    AvailableUnattended(PlannedAutomationKind),
    PlannedUnattended(PlannedAutomationKind),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageOperationStatus {
    InstalledOrChecked,
    PlannedUnattended,
    DeferredUnattended,
    SkippedCurrent,
    /// This package's download or install failed. The operation recorded
    /// the error and continued with the remaining packages instead of
    /// aborting the whole run.
    Failed,
    /// This package was not attempted because a package it depends on
    /// (e.g. REAPER) failed or was itself skipped earlier in this run.
    SkippedDependencyFailed,
}

#[derive(Debug, Default, Clone)]
pub(super) struct UnattendedPostInstallReport {
    pub(super) backup_paths: Vec<PathBuf>,
    pub(super) backup_manifest_path: Option<PathBuf>,
}

pub fn package_automation_support(
    package_id: &str,
    platform: Platform,
    architecture: Architecture,
) -> PackageAutomationSupport {
    let kind = match expected_artifact_kind(package_id, platform, architecture) {
        Ok(kind) => kind,
        Err(_) => return PackageAutomationSupport::Unavailable,
    };
    automation_support_dispatch(package_id, kind, platform)
}

fn automation_support_dispatch(
    package_id: &str,
    kind: ArtifactKind,
    platform: Platform,
) -> PackageAutomationSupport {
    if matches!(kind, ArtifactKind::ExtensionBinary) {
        return PackageAutomationSupport::Direct;
    }
    // Data-driven GitHub-release `.zip` packages are Direct: RABBIT extracts
    // the asset and copies it itself (no vendor installer to launch, no
    // prompt). Covers ReaKontrol + app2clap, replacing what were per-package
    // id checks. (ReaPack ships an ExtensionBinary, already Direct above; the
    // `Archive` guard mirrors the old behavior and avoids reclassifying a
    // package fed a non-archive artifact.)
    if matches!(kind, ArtifactKind::Archive) && is_github_release_package(package_id, platform) {
        return PackageAutomationSupport::Direct;
    }
    // FFmpeg ships as a `.7z` whose `bin/` we extract directly into
    // UserPlugins — no upstream installer to launch, no user prompt to
    // dismiss. Same automation class as the per-file extension-binary
    // case (Direct).
    if package_id == crate::package::PACKAGE_FFMPEG && matches!(kind, ArtifactKind::SevenZipArchive)
    {
        return PackageAutomationSupport::Direct;
    }
    let per_package = match package_id {
        crate::package::PACKAGE_REAPER => reaper::automation_support_for(kind, platform),
        crate::package::PACKAGE_OSARA => osara::automation_support_for(kind, platform),
        crate::package::PACKAGE_SWS => sws::automation_support_for(kind, platform),
        crate::package::PACKAGE_JAWS_SCRIPTS => {
            jaws_scripts::automation_support_for(kind, platform)
        }
        crate::package::PACKAGE_SURGE_XT => surge_xt::automation_support_for(kind, platform),
        _ => None,
    };
    if let Some(verdict) = per_package {
        return verdict;
    }
    match kind {
        ArtifactKind::Installer => {
            PackageAutomationSupport::PlannedUnattended(PlannedAutomationKind::VendorInstaller)
        }
        ArtifactKind::Archive | ArtifactKind::SevenZipArchive => {
            PackageAutomationSupport::PlannedUnattended(PlannedAutomationKind::ArchiveExtraction)
        }
        ArtifactKind::DiskImage => {
            PackageAutomationSupport::PlannedUnattended(PlannedAutomationKind::DiskImageInstall)
        }
        ArtifactKind::ExtensionBinary => unreachable!("ExtensionBinary handled above"),
    }
}

/// Whether `package_id` is defined by a data-driven `github_release` block
/// (RABBIT extracts/copies it itself — Direct automation).
fn is_github_release_package(package_id: &str, platform: Platform) -> bool {
    crate::package::package_specs_by_id(platform)
        .get(package_id)
        .is_some_and(|spec| spec.github_release.is_some())
}

pub fn execute_package_operation(
    resource_path: &Path,
    package_ids: &[String],
    platform: Platform,
    architecture: Architecture,
    cache_dir: &Path,
    options: &PackageOperationOptions,
) -> Result<PackageOperationReport> {
    execute_package_operation_with_progress(
        resource_path,
        package_ids,
        platform,
        architecture,
        cache_dir,
        options,
        &ProgressReporter::noop(),
    )
}

/// Like [`execute_package_operation`] but threads a [`ProgressReporter`]
/// down through the download and install phases so UIs can render a
/// live progress bar. The no-op overload above keeps existing callers on
/// the previous signature.
pub fn execute_package_operation_with_progress(
    resource_path: &Path,
    package_ids: &[String],
    platform: Platform,
    architecture: Architecture,
    cache_dir: &Path,
    options: &PackageOperationOptions,
    progress: &ProgressReporter,
) -> Result<PackageOperationReport> {
    let artifacts = resolve_latest_artifacts(package_ids, platform, architecture)?;
    let detections = detect_components(resource_path, platform)?;
    execute_resolved_package_operation_with_detections_and_progress(
        resource_path,
        artifacts,
        &detections,
        cache_dir,
        options,
        progress,
    )
}

pub fn execute_resolved_package_operation(
    resource_path: &Path,
    artifacts: Vec<ArtifactDescriptor>,
    cache_dir: &Path,
    options: &PackageOperationOptions,
) -> Result<PackageOperationReport> {
    execute_resolved_package_operation_with_detections_and_progress(
        resource_path,
        artifacts,
        &[],
        cache_dir,
        options,
        &ProgressReporter::noop(),
    )
}

/// Progress-aware variant of [`execute_resolved_package_operation`].
pub fn execute_resolved_package_operation_with_progress(
    resource_path: &Path,
    artifacts: Vec<ArtifactDescriptor>,
    cache_dir: &Path,
    options: &PackageOperationOptions,
    progress: &ProgressReporter,
) -> Result<PackageOperationReport> {
    execute_resolved_package_operation_with_detections_and_progress(
        resource_path,
        artifacts,
        &[],
        cache_dir,
        options,
        progress,
    )
}

pub fn execute_resolved_package_operation_with_detections(
    resource_path: &Path,
    artifacts: Vec<ArtifactDescriptor>,
    detections: &[ComponentDetection],
    cache_dir: &Path,
    options: &PackageOperationOptions,
) -> Result<PackageOperationReport> {
    execute_resolved_package_operation_with_detections_and_progress(
        resource_path,
        artifacts,
        detections,
        cache_dir,
        options,
        &ProgressReporter::noop(),
    )
}

/// Inner implementation that all the package-operation entry points
/// funnel through. The progress reporter fires download / install
/// boundary events as packages move through each phase; callers that
/// don't care about the events use one of the wrappers above which
/// pass a [`ProgressReporter::noop`] here.
pub fn execute_resolved_package_operation_with_detections_and_progress(
    resource_path: &Path,
    artifacts: Vec<ArtifactDescriptor>,
    detections: &[ComponentDetection],
    cache_dir: &Path,
    options: &PackageOperationOptions,
    progress: &ProgressReporter,
) -> Result<PackageOperationReport> {
    ensure_resource_path_ready(resource_path, options.dry_run)?;

    let _install_lock = if options.dry_run {
        None
    } else {
        let lock_path = options
            .lock_path
            .clone()
            .unwrap_or_else(|| crate::lock::default_package_install_lock_path(resource_path));
        Some(crate::lock::acquire_package_install_lock_at(&lock_path)?)
    };

    let mut items = Vec::new();
    let mut direct_installable = Vec::new();
    let mut unattended_installable = Vec::new();
    let mut deferred_installable = Vec::new();

    for artifact in artifacts {
        let plan_action = {
            let computed = plan_action_for_artifact(&artifact, detections);
            // Honor the user's explicit re-tick: a Keep computed from
            // "installed version is current" gets promoted to Update so
            // the install pipeline actually reruns instead of silently
            // skipping. Install/Update stay as-is — there's nothing to
            // promote.
            if matches!(computed, PlanActionKind::Keep)
                && options
                    .force_reinstall_packages
                    .iter()
                    .any(|id| id == &artifact.package_id)
            {
                PlanActionKind::Update
            } else {
                computed
            }
        };
        match plan_action {
            PlanActionKind::Install | PlanActionKind::Update => {
                match automation_support_for_artifact(&artifact, options) {
                    PackageAutomationSupport::Direct => direct_installable.push(PlannedArtifact {
                        artifact,
                        plan_action,
                    }),
                    PackageAutomationSupport::AvailableUnattended(_) => unattended_installable
                        .push(PlannedArtifact {
                            artifact,
                            plan_action,
                        }),
                    PackageAutomationSupport::PlannedUnattended(_)
                    | PackageAutomationSupport::Unavailable => {
                        deferred_installable.push(PlannedArtifact {
                            artifact,
                            plan_action,
                        })
                    }
                }
            }
            PlanActionKind::Keep => items.push(skipped_current_item(artifact, detections)),
        }
    }

    // Order both install buckets so every package installs after the
    // packages it depends on, making the dependency gate correct regardless
    // of the caller's package order. REAPER (the only dependency target
    // today) is unattended and the whole unattended bucket runs before the
    // direct one, so cross-bucket dependencies are already satisfied;
    // ordering each bucket additionally covers any future intra-bucket
    // dependency (e.g. one direct package depending on another).
    unattended_installable = dependency_ordered(unattended_installable);
    direct_installable = dependency_ordered(direct_installable);

    // ---- Download pipeline: queue EVERY remote fetch up front on a small
    // concurrent pool, so a huge download (FFmpeg's ~390 MB archive) streams
    // in the background while earlier packages already install. Queue order
    // doubles as download priority and mirrors install order: unattended
    // installers first (REAPER leads by manifest order), then the direct
    // extension files, then staging-only downloads. Installs themselves stay
    // strictly sequential and in input order — that preserves the
    // REAPER-before-extensions barrier, the fail-fast gating, and
    // one-elevated-installer-at-a-time.
    let unattended_needs_download = !options.dry_run && !unattended_installable.is_empty();
    let deferred_needs_download = options.stage_unsupported && !deferred_installable.is_empty();
    let mut download_jobs: Vec<ArtifactDescriptor> = Vec::new();
    if unattended_needs_download {
        download_jobs.extend(
            unattended_installable
                .iter()
                .map(|planned| planned.artifact.clone()),
        );
    }
    download_jobs.extend(
        direct_installable
            .iter()
            .map(|planned| planned.artifact.clone()),
    );
    if deferred_needs_download {
        download_jobs.extend(
            deferred_installable
                .iter()
                .map(|planned| planned.artifact.clone()),
        );
    }
    // Keep the pool guard alive for the whole operation: dropping it (early
    // return included) asks in-flight downloads to stop, so no worker
    // outlives the operation streaming into the cache.
    let (_download_pool, mut download_handles) = if download_jobs.is_empty() {
        (None, Vec::new())
    } else {
        let (pool, handles) = spawn_download_pool(&download_jobs, cache_dir, progress);
        (Some(pool), handles)
    };
    let mut handle_iter = download_handles.drain(..);
    let unattended_handles: Vec<DownloadHandle> = if unattended_needs_download {
        handle_iter
            .by_ref()
            .take(unattended_installable.len())
            .collect()
    } else {
        Vec::new()
    };
    let direct_handles: Vec<DownloadHandle> = handle_iter
        .by_ref()
        .take(direct_installable.len())
        .collect();
    let deferred_handles: Vec<DownloadHandle> = handle_iter.collect();

    let mut receipt_backup_path = None;
    let mut receipt_backup_manifest_path = None;
    let mut unattended_state = if options.dry_run || unattended_installable.is_empty() {
        None
    } else {
        Some(load_install_state(resource_path)?.unwrap_or_default())
    };
    let mut unattended_receipts_updated = false;
    // Package ids whose download/install failed (or that were skipped
    // because a dependency failed) so far in this run. A per-package
    // failure no longer aborts the whole operation — RABBIT records it,
    // keeps installing the packages that don't depend on it, and reports
    // everything at the end. REAPER installs first, so by the time its
    // extensions are considered this set already reflects whether REAPER
    // succeeded, letting `first_failed_dependency` cascade the skip.
    let mut failed: HashSet<String> = HashSet::new();

    if options.dry_run {
        items.extend(unattended_installable.into_iter().map(|planned| {
            planned_unattended_item(
                planned.artifact,
                planned.plan_action,
                resource_path,
                options.target_app_path.as_deref(),
                options.replace_osara_keymap,
            )
        }));
    } else if !unattended_installable.is_empty() {
        // Pipelined consumption: each package's install starts as soon as
        // ITS download handle resolves — while later downloads (FFmpeg…)
        // keep streaming on the pool. Installs remain strictly sequential
        // and in input order. A per-package download/install failure is
        // recorded (as a Failed item, and in `failed`) and the loop moves
        // on to the next package instead of aborting the run; a package
        // whose dependency already failed is skipped without being
        // downloaded or run.
        for (planned, handle) in unattended_installable.iter().zip(unattended_handles) {
            let package_id = planned.artifact.package_id.clone();
            if let Some(dependency) = first_failed_dependency(&package_id, &failed) {
                // Dropping `handle` here asks the pool to stop this
                // package's (now pointless) download.
                items.push(skipped_dependency_item(
                    planned.artifact.clone(),
                    planned.plan_action,
                    &dependency,
                ));
                failed.insert(package_id);
                continue;
            }
            let cached = match handle.wait() {
                Ok(cached) => cached,
                Err(error) => {
                    items.push(failed_operation_item(
                        planned.artifact.clone(),
                        planned.plan_action,
                        &error,
                    ));
                    failed.insert(package_id);
                    continue;
                }
            };
            // The unattended runner (vendor installer, archive extractor,
            // dmg mount + copy) is the work the user is waiting on for
            // these packages — emit start/complete around it so the UI
            // can render a "Installing REAPER…" line distinct from the
            // earlier "Downloading REAPER…".
            progress.report(ProgressEvent::InstallStarted {
                package_id: package_id.clone(),
            });
            match executed_unattended_item(
                planned,
                &cached,
                resource_path,
                options.target_app_path.as_deref(),
                options.replace_osara_keymap,
            ) {
                Ok(item) => items.push(item),
                Err(error) => {
                    items.push(failed_operation_item(
                        planned.artifact.clone(),
                        planned.plan_action,
                        &error,
                    ));
                    failed.insert(package_id.clone());
                    progress.report(ProgressEvent::InstallCompleted {
                        package_id: package_id.clone(),
                    });
                    continue;
                }
            }
            if let Some(state) = &mut unattended_state {
                match upsert_unattended_package_receipt(
                    state,
                    resource_path,
                    &planned.artifact,
                    &cached,
                    options.target_app_path.as_deref(),
                    options.replace_osara_keymap,
                ) {
                    Ok(()) => unattended_receipts_updated = true,
                    Err(error) => {
                        // The package's files DID land (executed_unattended_item
                        // succeeded); only its receipt bookkeeping failed. Treat
                        // it as a failure so the user sees the problem, but the
                        // install itself is on disk.
                        items.push(failed_operation_item(
                            planned.artifact.clone(),
                            planned.plan_action,
                            &error,
                        ));
                        failed.insert(package_id.clone());
                        progress.report(ProgressEvent::InstallCompleted {
                            package_id: package_id.clone(),
                        });
                        continue;
                    }
                }
            }
            progress.report(ProgressEvent::InstallCompleted {
                package_id: package_id.clone(),
            });
        }
        // Best-effort receipt persistence: run the backup + save for the
        // packages that DID install, and never let a failure HERE mask a
        // package error already recorded above.
        let receipt_save_result: Result<()> = if unattended_receipts_updated {
            (|| {
                let backup_id = operation_timestamp();
                let backup_set = resource_path
                    .join("RABBIT")
                    .join("backups")
                    .join(&backup_id);
                receipt_backup_path = backup_receipt_if_present(resource_path, &backup_set)?;
                if let Some(path) = &receipt_backup_path {
                    receipt_backup_manifest_path = Some(write_receipt_backup_manifest(
                        &backup_set,
                        &backup_id,
                        path,
                    )?);
                }
                if let Some(state) = &unattended_state {
                    save_install_state(resource_path, state)?;
                }
                Ok(())
            })()
        } else {
            Ok(())
        };
        // A failure persisting the receipts is a genuine bookkeeping error
        // (RABBIT's on-disk state would be inconsistent) that is not tied to
        // any single package, so it still aborts. Per-package install
        // failures were already recorded as Failed items above and do not
        // reach here.
        receipt_save_result?;
    }

    // Staged-only downloads are collected AFTER the installs above: they are
    // queued last on the pool (lowest priority) and nothing installs them, so
    // waiting on them earlier would only delay the pipeline. The report is
    // sorted by package id at the end, so collection order is invisible.
    let staged_deferred = if deferred_needs_download {
        let mut staged = Vec::with_capacity(deferred_handles.len());
        for handle in deferred_handles {
            // Deferred packages are only staged, never installed here, so a
            // failed staging download must not abort the run: the package
            // simply reports as "not staged" via `skipped_item(None)`.
            if let Ok(cached) = handle.wait() {
                staged.push(cached);
            }
        }
        staged
    } else {
        Vec::new()
    };

    if options.stage_unsupported {
        items.extend(
            deferred_installable
                .iter()
                .map(|planned| {
                    let cached = staged_deferred
                        .iter()
                        .find(|cached| cached.descriptor.package_id == planned.artifact.package_id)
                        .cloned();
                    skipped_item(
                        planned.artifact.clone(),
                        planned.plan_action,
                        resource_path,
                        cached,
                        options.target_app_path.as_deref(),
                        options.replace_osara_keymap,
                    )
                })
                .collect::<Vec<_>>(),
        );
    } else {
        items.extend(
            deferred_installable
                .into_iter()
                .map(|planned| {
                    skipped_item(
                        planned.artifact,
                        planned.plan_action,
                        resource_path,
                        None,
                        options.target_app_path.as_deref(),
                        options.replace_osara_keymap,
                    )
                })
                .collect::<Vec<_>>(),
        );
    }

    // Direct extension files (single-file GitHub-release plugins — SWS,
    // ReaPack, ReaKontrol) were downloading on the pool the whole time.
    // Install each independently so one failure doesn't sink the rest:
    // gate on its dependencies (all of these depend on REAPER, so a failed
    // REAPER install cascades a dependency-skip onto every one), tolerate a
    // failed download or install, and keep going. Each is a single file, so
    // its report carries exactly one action.
    let mut install_report: Option<InstallReport> = None;
    for (planned, handle) in direct_installable.iter().zip(direct_handles) {
        let package_id = planned.artifact.package_id.clone();
        if let Some(dependency) = first_failed_dependency(&package_id, &failed) {
            items.push(skipped_dependency_item(
                planned.artifact.clone(),
                planned.plan_action,
                &dependency,
            ));
            failed.insert(package_id);
            continue;
        }
        let cached = match handle.wait() {
            Ok(cached) => cached,
            Err(error) => {
                items.push(failed_operation_item(
                    planned.artifact.clone(),
                    planned.plan_action,
                    &error,
                ));
                failed.insert(package_id);
                continue;
            }
        };
        match install_cached_artifacts_with_progress(
            resource_path,
            std::slice::from_ref(&cached),
            &InstallOptions {
                dry_run: options.dry_run,
                allow_reaper_running: options.allow_reaper_running,
                target_app_path: options.target_app_path.clone(),
            },
            progress,
        ) {
            Ok(report) => {
                items.push(PackageOperationItem {
                    package_id: cached.descriptor.package_id.clone(),
                    plan_action: planned.plan_action,
                    status: PackageOperationStatus::InstalledOrChecked,
                    artifact: cached.descriptor.clone(),
                    cached_artifact: Some(cached.clone()),
                    install_action: report.actions.first().cloned(),
                    backup_paths: Vec::new(),
                    backup_manifest_path: None,
                    planned_execution: None,
                    manual_instruction: None,
                    message: "Single extension binary handled by RABBIT installer.".to_string(),
                    message_code: PackageOperationMessage::ExtensionBinaryInstalled,
                });
                merge_install_report(&mut install_report, report);
            }
            Err(error) => {
                // A preflight failure is a global precondition (REAPER is
                // running, or the resource dir isn't writable), not this
                // package's fault. Keep it fatal — as the old single-batch
                // install did — instead of demoting it to a per-package
                // failure and mis-reporting "completed with errors" when the
                // real fix is "close REAPER and re-run". Per-package
                // download/copy failures still continue the run.
                if matches!(error, RabbitError::PreflightFailed { .. }) {
                    return Err(error);
                }
                items.push(failed_operation_item(
                    planned.artifact.clone(),
                    planned.plan_action,
                    &error,
                ));
                failed.insert(package_id);
            }
        }
    }

    items.sort_by(|left, right| left.package_id.cmp(&right.package_id));

    Ok(PackageOperationReport {
        resource_path: resource_path.to_path_buf(),
        dry_run: options.dry_run,
        install_report,
        receipt_backup_path,
        receipt_backup_manifest_path,
        items,
    })
}

fn automation_support_for_artifact(
    artifact: &ArtifactDescriptor,
    _options: &PackageOperationOptions,
) -> PackageAutomationSupport {
    automation_support_dispatch(&artifact.package_id, artifact.kind, artifact.platform)
}

#[derive(Debug, Clone)]
struct PlannedArtifact {
    artifact: ArtifactDescriptor,
    plan_action: PlanActionKind,
}

fn plan_action_for_artifact(
    artifact: &ArtifactDescriptor,
    detections: &[ComponentDetection],
) -> PlanActionKind {
    let Some(detection) = detections
        .iter()
        .find(|detection| detection.package_id == artifact.package_id)
    else {
        return PlanActionKind::Install;
    };

    if !detection.installed {
        return PlanActionKind::Install;
    }

    // Installed but version couldn't be read: re-install the upstream
    // version on top of the existing files. The standard backup + receipt
    // mechanism protects the prior install if anything goes wrong.
    let Some(installed_version) = &detection.version else {
        return PlanActionKind::Update;
    };

    if installed_version.cmp_lenient(&artifact.version).is_lt() {
        PlanActionKind::Update
    } else {
        PlanActionKind::Keep
    }
}

fn skipped_current_item(
    artifact: ArtifactDescriptor,
    detections: &[ComponentDetection],
) -> PackageOperationItem {
    let installed_version = detections
        .iter()
        .find(|detection| detection.package_id == artifact.package_id)
        .and_then(|detection| detection.version.as_ref())
        .map(ToString::to_string)
        .unwrap_or_else(|| "unknown".to_string());

    let available_version = artifact.version.to_string();
    PackageOperationItem {
        package_id: artifact.package_id.clone(),
        plan_action: PlanActionKind::Keep,
        status: PackageOperationStatus::SkippedCurrent,
        message: format!(
            "Installed version {installed_version} is current or newer than available version {available_version}.",
        ),
        message_code: PackageOperationMessage::SkippedCurrent {
            installed_version,
            available_version,
        },
        artifact,
        cached_artifact: None,
        install_action: None,
        backup_paths: Vec::new(),
        backup_manifest_path: None,
        planned_execution: None,
        manual_instruction: None,
    }
}

fn skipped_item(
    artifact: ArtifactDescriptor,
    plan_action: PlanActionKind,
    resource_path: &Path,
    cached_artifact: Option<CachedArtifact>,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> PackageOperationItem {
    let planned_execution = Some(planned_execution_for_artifact(
        &artifact,
        cached_artifact.as_ref(),
        resource_path,
        target_app_path,
        replace_osara_keymap,
    ));
    let manual_instruction = Some(manual_instruction_for_artifact(
        &artifact,
        cached_artifact.as_ref(),
        resource_path,
        target_app_path,
        replace_osara_keymap,
    ));
    let staged = cached_artifact.is_some();
    let message_code = if staged {
        PackageOperationMessage::DeferredUnattendedStaged {
            artifact_kind: artifact.kind,
        }
    } else {
        PackageOperationMessage::DeferredUnattendedNotStaged {
            artifact_kind: artifact.kind,
        }
    };
    PackageOperationItem {
        package_id: artifact.package_id.clone(),
        plan_action,
        status: PackageOperationStatus::DeferredUnattended,
        message: if staged {
            format!(
                "This build has not implemented the planned unattended {} execution path yet. RABBIT staged the artifact in the cache but did not run it.",
                planned_automation_description(artifact.kind)
            )
        } else {
            format!(
                "This build has not implemented the planned unattended {} execution path yet. RABBIT did not download or run the artifact.",
                planned_automation_description(artifact.kind)
            )
        },
        message_code,
        artifact,
        cached_artifact,
        install_action: None,
        backup_paths: Vec::new(),
        backup_manifest_path: None,
        planned_execution,
        manual_instruction,
    }
}

/// Build the report item for a package whose download or install failed.
/// The run continues after this; the error text is preserved verbatim so
/// the saved report and the UI can show the real cause (an antivirus block,
/// a network failure, a disk error, …).
fn failed_operation_item(
    artifact: ArtifactDescriptor,
    plan_action: PlanActionKind,
    error: &crate::error::RabbitError,
) -> PackageOperationItem {
    let error_text = error.to_string();
    let antivirus_block = matches!(error, RabbitError::WindowsFileBlockedByAntivirus { .. });
    PackageOperationItem {
        package_id: artifact.package_id.clone(),
        plan_action,
        status: PackageOperationStatus::Failed,
        message: format!("Installation failed: {error_text}"),
        message_code: PackageOperationMessage::InstallFailed {
            error: error_text,
            antivirus_block,
        },
        artifact,
        cached_artifact: None,
        install_action: None,
        backup_paths: Vec::new(),
        backup_manifest_path: None,
        planned_execution: None,
        manual_instruction: None,
    }
}

/// Build the report item for a package skipped because a dependency it
/// installs on top of (e.g. REAPER) failed or was itself skipped in this
/// run. Distinct from a plain failure so the UI can explain the cascade
/// rather than implying this package itself went wrong.
fn skipped_dependency_item(
    artifact: ArtifactDescriptor,
    plan_action: PlanActionKind,
    dependency: &str,
) -> PackageOperationItem {
    PackageOperationItem {
        package_id: artifact.package_id.clone(),
        plan_action,
        status: PackageOperationStatus::SkippedDependencyFailed,
        message: format!(
            "Skipped because {dependency}, which it needs, did not install successfully."
        ),
        message_code: PackageOperationMessage::SkippedDependencyFailed {
            dependency: dependency.to_string(),
        },
        artifact,
        cached_artifact: None,
        install_action: None,
        backup_paths: Vec::new(),
        backup_manifest_path: None,
        planned_execution: None,
        manual_instruction: None,
    }
}

/// The first dependency of `package_id` that has failed (or been skipped)
/// so far in this run, if any. Dependencies that aren't part of the current
/// run never appear in `failed`, so installing an extension into an
/// already-present REAPER is not gated.
fn first_failed_dependency(package_id: &str, failed: &HashSet<String>) -> Option<String> {
    crate::package::package_dependencies(package_id)
        .into_iter()
        .find(|dependency| failed.contains(dependency))
}

/// Stable topological order of `planned` so every package appears after the
/// packages (also in `planned`) it depends on. The dependency gate needs a
/// dependency's install result known before its dependents are considered.
/// Ties keep input order; a dependency cycle — which the manifest never
/// contains — degrades gracefully to input order for the packages left in
/// it. Dependencies outside `planned` are ignored (they're handled by the
/// gate, which only looks at in-run failures).
fn dependency_ordered(planned: Vec<PlannedArtifact>) -> Vec<PlannedArtifact> {
    let present: HashSet<String> = planned
        .iter()
        .map(|item| item.artifact.package_id.clone())
        .collect();
    let mut emitted: HashSet<String> = HashSet::new();
    let mut remaining = planned;
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let mut next_remaining = Vec::with_capacity(remaining.len());
        let mut progressed = false;
        for item in remaining {
            let deps_ready = crate::package::package_dependencies(&item.artifact.package_id)
                .into_iter()
                .filter(|dependency| present.contains(dependency))
                .all(|dependency| emitted.contains(&dependency));
            if deps_ready {
                emitted.insert(item.artifact.package_id.clone());
                ordered.push(item);
                progressed = true;
            } else {
                next_remaining.push(item);
            }
        }
        if !progressed {
            // Cycle among the remaining packages — emit them in input order.
            ordered.extend(next_remaining);
            break;
        }
        remaining = next_remaining;
    }
    ordered
}

/// Fold a single-package install report into the running direct-install
/// report surfaced in the operation result. Actions accumulate; the first
/// receipt-backup path/manifest seen is kept as representative. Each
/// per-package call that replaces files writes its own timestamped backup
/// set, and every set stays on disk and is listed by `RABBIT backups` for
/// rollback — the report just needs one path to show the user.
fn merge_install_report(target: &mut Option<InstallReport>, source: InstallReport) {
    match target {
        None => *target = Some(source),
        Some(existing) => {
            existing.actions.extend(source.actions);
            existing.receipt_written |= source.receipt_written;
            if existing.receipt_backup_path.is_none() {
                existing.receipt_backup_path = source.receipt_backup_path;
            }
            if existing.backup_manifest_path.is_none() {
                existing.backup_manifest_path = source.backup_manifest_path;
            }
        }
    }
}

fn planned_unattended_item(
    artifact: ArtifactDescriptor,
    plan_action: PlanActionKind,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> PackageOperationItem {
    let planned_execution = Some(planned_execution_for_artifact(
        &artifact,
        None,
        resource_path,
        target_app_path,
        replace_osara_keymap,
    ));
    PackageOperationItem {
        package_id: artifact.package_id.clone(),
        plan_action,
        status: PackageOperationStatus::PlannedUnattended,
        message: format!(
            "Dry run: RABBIT would download and run this {} unattended.",
            planned_automation_description(artifact.kind)
        ),
        message_code: PackageOperationMessage::DryRunWouldRunUnattended {
            artifact_kind: artifact.kind,
        },
        artifact,
        cached_artifact: None,
        install_action: None,
        backup_paths: Vec::new(),
        backup_manifest_path: None,
        planned_execution,
        manual_instruction: None,
    }
}

fn executed_unattended_item(
    planned: &PlannedArtifact,
    cached_artifact: &CachedArtifact,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> Result<PackageOperationItem> {
    let planned_execution = planned_execution_for_artifact(
        &planned.artifact,
        Some(cached_artifact),
        resource_path,
        target_app_path,
        replace_osara_keymap,
    );
    // Run the planned execution, the package's post-install fixups, and
    // verify the produced files. Original order is preserved (some
    // post-install steps such as the OSARA keymap replacement produce
    // files that `verify_planned_execution_paths` then checks). We
    // tolerate a non-zero process exit *if* verification confirms the
    // install actually landed: REAPER's NSIS installer (and a few others)
    // can return 1223 / non-zero on Cancel from a post-install offer
    // dialog even when the install itself completed, and the user already
    // has the binaries on disk. Surfacing the process error in that case
    // mis-reports a successful install as a failure.
    //
    // `install_started_at` is captured *before* the runner so the
    // freshness check below can detect silent no-ops: if the installer
    // returns success without rewriting its `freshness_paths`, those
    // files keep their old mtimes and the run is rejected.
    // REAPER (Windows): snapshot the desktop shortcuts that exist *before*
    // the installer runs. REAPER's silent installer always (re)creates a
    // desktop icon with no switch to suppress it, so after the install we
    // remove a freshly-created one unless this is a brand-new standard
    // install or an icon was already there. Captured here, enforced once the
    // install is confirmed below. `None` (and a no-op) off Windows / non-REAPER.
    let reaper_desktop_policy = (planned.artifact.package_id == crate::package::PACKAGE_REAPER)
        .then(|| {
            reaper::capture_desktop_shortcut_policy(
                planned.artifact.platform,
                resource_path,
                target_app_path,
                matches!(planned.plan_action, PlanActionKind::Install),
            )
        })
        .flatten();

    let install_started_at = SystemTime::now();
    let process_result = execute_planned_execution(&planned_execution, false);
    let post_install_result = post_execute_unattended_artifact(
        &planned.artifact,
        resource_path,
        target_app_path,
        replace_osara_keymap,
    );
    let post_install = match verify_planned_execution_paths(&planned_execution) {
        Ok(()) => {
            // Verify confirms the expected files are on disk — accept the
            // run even if the process or post-install steps reported
            // errors. Use the post-install report when available; fall
            // back to a default report (no extra backups recorded) when
            // post-install itself failed but the files we needed are
            // already there.
            post_install_result.unwrap_or_default()
        }
        Err(verify_err) => {
            // Verification didn't see the expected files: surface the
            // most informative error available — process first, then
            // post-install, then verify.
            process_result?;
            post_install_result?;
            return Err(verify_err);
        }
    };
    verify_planned_execution_freshness(&planned_execution, install_started_at)?;

    // The install is confirmed on disk — apply the REAPER desktop-icon policy
    // (remove an unwanted shortcut the installer just created). Best-effort:
    // it never fails the install.
    if let Some(policy) = &reaper_desktop_policy {
        policy.enforce();
    }

    let (message, message_code) = match planned.artifact.package_id.as_str() {
        crate::package::PACKAGE_OSARA => osara::unattended_install_message(
            replace_osara_keymap,
            !post_install.backup_paths.is_empty(),
        )
        .unwrap_or_else(|| {
            (
                DEFAULT_UNATTENDED_INSTALL_MESSAGE.to_string(),
                PackageOperationMessage::UnattendedInstalled,
            )
        }),
        _ => (
            DEFAULT_UNATTENDED_INSTALL_MESSAGE.to_string(),
            PackageOperationMessage::UnattendedInstalled,
        ),
    };

    Ok(PackageOperationItem {
        package_id: planned.artifact.package_id.clone(),
        plan_action: planned.plan_action,
        status: PackageOperationStatus::InstalledOrChecked,
        message,
        message_code,
        artifact: planned.artifact.clone(),
        cached_artifact: Some(cached_artifact.clone()),
        install_action: None,
        backup_paths: post_install.backup_paths,
        backup_manifest_path: post_install.backup_manifest_path,
        planned_execution: Some(planned_execution),
        manual_instruction: None,
    })
}

fn upsert_unattended_package_receipt(
    state: &mut InstallState,
    resource_path: &Path,
    artifact: &ArtifactDescriptor,
    cached_artifact: &CachedArtifact,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> Result<()> {
    let installed_paths = receipt_paths_for_artifact(
        artifact,
        resource_path,
        target_app_path,
        replace_osara_keymap,
    )?;
    upsert_package_receipt(
        state,
        resource_path,
        PackageReceiptParams {
            package_id: &artifact.package_id,
            version: Some(artifact.version.clone()),
            source_url: Some(artifact.url.clone()),
            source_sha256: Some(cached_artifact.sha256.clone()),
            installed_paths: &installed_paths,
            installed_at: Some(operation_timestamp()),
            architecture: Some(artifact.architecture),
        },
    )
}

fn receipt_paths_for_artifact(
    artifact: &ArtifactDescriptor,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> Result<Vec<PathBuf>> {
    let effective_target_app_path =
        effective_target_app_path(artifact, resource_path, target_app_path);
    let mut paths = Vec::new();

    if artifact.package_id == crate::package::PACKAGE_REAPER {
        paths.extend(reaper::receipt_paths(
            resource_path,
            effective_target_app_path.as_deref(),
        ));
    }

    let package_specs = package_specs_by_id(artifact.platform);
    if let Some(spec) = package_specs.get(&artifact.package_id) {
        paths.extend(matching_user_plugin_files(
            resource_path,
            artifact.platform,
            spec,
        )?);
    }

    match artifact.package_id.as_str() {
        crate::package::PACKAGE_OSARA => {
            paths.extend(osara::receipt_paths(resource_path, replace_osara_keymap));
        }
        crate::package::PACKAGE_SWS => {
            paths.extend(sws::receipt_paths(resource_path));
        }
        crate::package::PACKAGE_JAWS_SCRIPTS => {
            paths.extend(jaws_scripts::receipt_paths(resource_path));
        }
        crate::package::PACKAGE_SURGE_XT => {
            paths.extend(surge_xt::receipt_paths(artifact.platform));
        }
        _ => {}
    }

    paths.sort();
    paths.dedup();

    if paths.is_empty() {
        return Err(RabbitError::PostInstallVerificationFailed {
            missing_paths: planned_verification_paths(
                artifact,
                resource_path,
                effective_target_app_path.as_deref(),
                replace_osara_keymap,
            ),
        });
    }

    Ok(paths)
}

fn post_execute_unattended_artifact(
    artifact: &ArtifactDescriptor,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> Result<UnattendedPostInstallReport> {
    if artifact.package_id == crate::package::PACKAGE_OSARA {
        return osara::post_install_unattended(
            resource_path,
            artifact.platform,
            target_app_path,
            replace_osara_keymap,
        );
    }
    Ok(UnattendedPostInstallReport::default())
}

pub(super) fn backup_file_for_unattended_change(
    resource_path: &Path,
    package_id: &str,
    source_path: &Path,
    reason: &str,
) -> Result<(PathBuf, PathBuf)> {
    let relative_path = source_path
        .strip_prefix(resource_path)
        .map_err(|_| crate::error::RabbitError::InvalidPlannedExecution {
            message: format!(
                "backup source is outside the selected resource path: {}",
                source_path.display()
            ),
        })?
        .to_path_buf();
    let backup_id = operation_timestamp();
    let backup_set = resource_path
        .join("RABBIT")
        .join("backups")
        .join(&backup_id);
    let backup_path = backup_set.join(&relative_path);

    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent).with_path(parent)?;
    }
    std::fs::copy(source_path, &backup_path).with_path(&backup_path)?;

    let manifest_path = save_backup_manifest(
        &backup_set,
        &BackupManifest {
            schema_version: 1,
            rabbit_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: backup_id,
            reason: reason.to_string(),
            files: vec![BackupManifestFile {
                package_id: Some(package_id.to_string()),
                original_path: relative_path,
                backup_path: backup_path.clone(),
                size: std::fs::metadata(&backup_path)
                    .with_path(&backup_path)?
                    .len(),
                sha256: sha256_file(&backup_path)?,
            }],
            receipt_backup_path: None,
        },
    )?;

    Ok((backup_path, manifest_path))
}

fn backup_receipt_if_present(resource_path: &Path, backup_set: &Path) -> Result<Option<PathBuf>> {
    let source_path = receipt_path(resource_path);
    if !source_path.is_file() {
        return Ok(None);
    }

    let backup_path = backup_set.join(RECEIPT_RELATIVE_PATH);
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent).with_path(parent)?;
    }
    std::fs::copy(&source_path, &backup_path).with_path(&backup_path)?;
    Ok(Some(backup_path))
}

fn write_receipt_backup_manifest(
    backup_set: &Path,
    created_at: &str,
    receipt_backup_path: &Path,
) -> Result<PathBuf> {
    save_backup_manifest(
        backup_set,
        &BackupManifest {
            schema_version: 1,
            rabbit_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: created_at.to_string(),
            reason: "unattended-receipt-update".to_string(),
            files: vec![BackupManifestFile {
                package_id: None,
                original_path: PathBuf::from(RECEIPT_RELATIVE_PATH),
                backup_path: receipt_backup_path.to_path_buf(),
                size: std::fs::metadata(receipt_backup_path)
                    .with_path(receipt_backup_path)?
                    .len(),
                sha256: sha256_file(receipt_backup_path)?,
            }],
            receipt_backup_path: Some(receipt_backup_path.to_path_buf()),
        },
    )
}

pub(super) fn replace_file_from_source(source_path: &Path, target_path: &Path) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent).with_path(parent)?;
    }

    let temp_path = temporary_target_path(target_path);
    if temp_path.exists() {
        std::fs::remove_file(&temp_path).with_path(&temp_path)?;
    }

    std::fs::copy(source_path, &temp_path).with_path(&temp_path)?;

    if target_path.exists() {
        std::fs::remove_file(target_path).with_path(target_path)?;
    }

    std::fs::rename(&temp_path, target_path).with_path(target_path)
}

fn temporary_target_path(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("target");
    target_path.with_file_name(format!("{file_name}.rabbit-tmp"))
}

fn operation_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "unattended-unix-{}-{:09}",
        duration.as_secs(),
        duration.subsec_nanos()
    )
}

fn planned_execution_for_artifact(
    artifact: &ArtifactDescriptor,
    cached_artifact: Option<&CachedArtifact>,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> PlannedExecutionPlan {
    let artifact_location = cached_artifact
        .map(|cached| cached.path.display().to_string())
        .unwrap_or_else(|| artifact.url.clone());
    let effective_target_app_path =
        effective_target_app_path(artifact, resource_path, target_app_path);
    let verification_paths = planned_verification_paths(
        artifact,
        resource_path,
        effective_target_app_path.as_deref(),
        replace_osara_keymap,
    );
    let freshness_paths = planned_freshness_paths(artifact);
    let requires_elevation = package_requires_elevation(
        artifact,
        resource_path,
        effective_target_app_path.as_deref(),
    );

    if let Some(override_) = planned_execution_override_for_artifact(
        artifact,
        resource_path,
        effective_target_app_path.as_deref(),
    ) {
        return PlannedExecutionPlan {
            kind: override_.kind,
            program: None,
            arguments: override_.arguments,
            working_directory: if override_.use_cached_working_dir {
                cached_artifact.and_then(|cached| cached.path.parent().map(Path::to_path_buf))
            } else {
                None
            },
            artifact_location,
            verification_paths,
            requires_elevation,
            freshness_paths,
        };
    }

    match artifact.kind {
        ArtifactKind::Installer => PlannedExecutionPlan {
            kind: PlannedExecutionKind::LaunchInstallerExecutable,
            program: Some(artifact_location.clone()),
            arguments: installer_arguments_for_artifact(
                artifact,
                resource_path,
                effective_target_app_path.as_deref(),
            ),
            working_directory: cached_artifact
                .and_then(|cached| cached.path.parent().map(Path::to_path_buf)),
            artifact_location,
            verification_paths,
            requires_elevation,
            freshness_paths,
        },
        ArtifactKind::Archive | ArtifactKind::SevenZipArchive => PlannedExecutionPlan {
            kind: PlannedExecutionKind::ExtractArchiveAndRunInstaller,
            program: None,
            arguments: Vec::new(),
            working_directory: cached_artifact
                .and_then(|cached| cached.path.parent().map(Path::to_path_buf)),
            artifact_location,
            verification_paths,
            requires_elevation,
            freshness_paths,
        },
        ArtifactKind::DiskImage => PlannedExecutionPlan {
            kind: PlannedExecutionKind::MountDiskImageAndRunInstaller,
            program: None,
            arguments: Vec::new(),
            working_directory: None,
            artifact_location,
            verification_paths,
            requires_elevation,
            freshness_paths,
        },
        ArtifactKind::ExtensionBinary => PlannedExecutionPlan {
            kind: PlannedExecutionKind::LaunchInstallerExecutable,
            program: Some(artifact_location.clone()),
            arguments: Vec::new(),
            working_directory: cached_artifact
                .and_then(|cached| cached.path.parent().map(Path::to_path_buf)),
            artifact_location,
            verification_paths,
            requires_elevation,
            freshness_paths,
        },
    }
}

/// Per-package decision: does this artifact's runner need to launch through
/// Windows UAC elevation rather than plain `CreateProcess`?
///
/// Two known consumers today:
///   - **JAWS-for-REAPER scripts** — always elevate on Windows. The NSIS
///     script declares `RequestExecutionLevel admin` and silently no-ops in
///     `/S` mode without an elevated parent.
///   - **REAPER** — elevate only for non-portable targets. The standard
///     install writes to `C:\Program Files\REAPER (x64)\` (admin-only);
///     portable installs write to a user-chosen folder and need no UAC.
fn package_requires_elevation(
    artifact: &ArtifactDescriptor,
    resource_path: &Path,
    target_app_path: Option<&Path>,
) -> bool {
    if !matches!(artifact.platform, Platform::Windows) {
        return false;
    }
    if !matches!(artifact.kind, ArtifactKind::Installer) {
        return false;
    }
    // Real vendor installers always ship as `.exe`; test fixtures use `.cmd`
    // / `.bat` script-host helpers. ShellExecuteEx(runas) can't elevate a
    // script-host helper and our tests have no UAC consent dialog, so gate
    // elevation on the file extension before consulting the per-package
    // rules.
    if !artifact.file_name.to_ascii_lowercase().ends_with(".exe") {
        return false;
    }
    match artifact.package_id.as_str() {
        crate::package::PACKAGE_JAWS_SCRIPTS => true,
        crate::package::PACKAGE_REAPER => !target_likely_portable(resource_path, target_app_path),
        _ => false,
    }
}

fn planned_execution_override_for_artifact(
    artifact: &ArtifactDescriptor,
    resource_path: &Path,
    target_app_path: Option<&Path>,
) -> Option<PlannedExecutionOverride> {
    match artifact.package_id.as_str() {
        crate::package::PACKAGE_REAPER => reaper::planned_execution_override(
            artifact.kind,
            artifact.platform,
            resource_path,
            target_app_path,
        ),
        crate::package::PACKAGE_OSARA => {
            osara::planned_execution_override(artifact.kind, artifact.platform, resource_path)
        }
        crate::package::PACKAGE_SURGE_XT => {
            surge_xt::planned_execution_override(artifact.kind, artifact.platform)
        }
        _ => None,
    }
}

fn installer_arguments_for_artifact(
    artifact: &ArtifactDescriptor,
    resource_path: &Path,
    target_app_path: Option<&Path>,
) -> Vec<String> {
    let per_package = match artifact.package_id.as_str() {
        crate::package::PACKAGE_REAPER => reaper::installer_arguments(
            artifact.kind,
            artifact.platform,
            resource_path,
            target_app_path,
        ),
        crate::package::PACKAGE_OSARA => {
            osara::installer_arguments(artifact.kind, artifact.platform, resource_path)
        }
        crate::package::PACKAGE_SWS => {
            sws::installer_arguments(artifact.kind, artifact.platform, resource_path)
        }
        crate::package::PACKAGE_JAWS_SCRIPTS => {
            jaws_scripts::installer_arguments(artifact.kind, artifact.platform)
        }
        crate::package::PACKAGE_SURGE_XT => {
            surge_xt::installer_arguments(artifact.kind, artifact.platform)
        }
        _ => None,
    };
    per_package.unwrap_or_default()
}

fn effective_target_app_path(
    artifact: &ArtifactDescriptor,
    resource_path: &Path,
    target_app_path: Option<&Path>,
) -> Option<PathBuf> {
    target_app_path
        .map(Path::to_path_buf)
        .or_else(|| inferred_target_app_path(artifact.platform, resource_path))
}

fn inferred_target_app_path(platform: Platform, resource_path: &Path) -> Option<PathBuf> {
    if let Some(standard) = default_standard_installation(platform)
        .filter(|installation| installation.resource_path == resource_path)
    {
        return Some(standard.app_path);
    }

    Some(portable_target_app_path(platform, resource_path))
}

fn portable_target_app_path(platform: Platform, resource_path: &Path) -> PathBuf {
    match platform {
        Platform::Windows => resource_path.join("reaper.exe"),
        Platform::MacOs => resource_path.join("REAPER.app"),
    }
}

fn manual_instruction_for_artifact(
    artifact: &ArtifactDescriptor,
    cached_artifact: Option<&CachedArtifact>,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> ManualInstallInstruction {
    let artifact_location = cached_artifact
        .map(|cached| cached.path.display().to_string())
        .unwrap_or_else(|| artifact.url.clone());
    build_manual_instruction(
        &artifact.package_id,
        artifact.kind,
        artifact_access_step(artifact.kind, &artifact_location),
        resource_path,
        target_app_path,
        replace_osara_keymap,
    )
}

pub fn preview_manual_instruction(
    package_id: &str,
    kind: ArtifactKind,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> ManualInstallInstruction {
    build_manual_instruction(
        package_id,
        kind,
        preview_artifact_access_step(kind),
        resource_path,
        target_app_path,
        replace_osara_keymap,
    )
}

fn build_manual_instruction(
    package_id: &str,
    kind: ArtifactKind,
    artifact_access: String,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> ManualInstallInstruction {
    let mut steps = vec![artifact_access];
    let mut notes = vec![
        format!(
            "RABBIT is designed to launch and complete this package through an unattended {} flow, but this build still requires manual completion.",
            planned_automation_description(kind)
        ),
        "Close REAPER before running the installer or copying extension files.".to_string(),
    ];

    match package_id {
        crate::package::PACKAGE_OSARA => {
            steps.extend(osara_manual_steps(
                kind,
                resource_path,
                replace_osara_keymap,
            ));
            notes.extend(osara::manual_install_notes(
                resource_path,
                replace_osara_keymap,
            ));
        }
        crate::package::PACKAGE_SWS => {
            steps.extend(sws_manual_steps(kind, resource_path));
            notes.extend(sws::manual_install_notes(resource_path));
        }
        crate::package::PACKAGE_REAPER => {
            steps.extend(reaper_manual_steps(kind, resource_path, target_app_path));
            notes.extend(reaper::manual_install_notes(resource_path, target_app_path));
        }
        _ => {
            steps.push(format!(
                "Install or extract the package for this REAPER target: {}",
                resource_path.display()
            ));
        }
    }

    steps.push(
        "Return to RABBIT and run detection again to verify the installed version.".to_string(),
    );

    ManualInstallInstruction {
        title: format!(
            "Manual install required for {}",
            package_title_name(package_id)
        ),
        steps,
        notes,
    }
}

fn artifact_access_step(kind: ArtifactKind, artifact_location: &str) -> String {
    match kind {
        ArtifactKind::Installer => format!("Run this installer: {artifact_location}"),
        ArtifactKind::Archive | ArtifactKind::SevenZipArchive => {
            format!("Extract this archive: {artifact_location}")
        }
        ArtifactKind::DiskImage => format!("Open this disk image: {artifact_location}"),
        ArtifactKind::ExtensionBinary => format!("Use this extension file: {artifact_location}"),
    }
}

fn planned_automation_description(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Installer => "vendor installer",
        ArtifactKind::Archive | ArtifactKind::SevenZipArchive => "archive extraction",
        ArtifactKind::DiskImage => "disk image install",
        ArtifactKind::ExtensionBinary => "direct file install",
    }
}

/// Per-package freshness probe: which on-disk files should the runner
/// confirm were rewritten by *this* install run? Empty for packages whose
/// installers are reliable enough that an existence-only verification path
/// is good enough; populated for packages whose `/S` silent path is known to
/// no-op when something goes wrong (today: jaws-scripts, where the NSIS
/// installer's `WriteUninstaller` directive stamps the only file we can
/// reliably tell apart by mtime).
fn planned_freshness_paths(artifact: &ArtifactDescriptor) -> Vec<PathBuf> {
    if artifact.package_id == crate::package::PACKAGE_JAWS_SCRIPTS {
        return jaws_scripts::freshness_paths();
    }
    Vec::new()
}

fn planned_verification_paths(
    artifact: &ArtifactDescriptor,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> Vec<PathBuf> {
    let mut paths = match artifact.package_id.as_str() {
        crate::package::PACKAGE_REAPER => {
            reaper::verification_paths(resource_path, target_app_path)
        }
        crate::package::PACKAGE_OSARA => {
            osara::verification_paths(resource_path, replace_osara_keymap)
        }
        crate::package::PACKAGE_SWS => sws::verification_paths(resource_path, artifact),
        crate::package::PACKAGE_REAPACK | crate::package::PACKAGE_REAKONTROL => {
            vec![resource_path.join("UserPlugins")]
        }
        crate::package::PACKAGE_JAWS_SCRIPTS => jaws_scripts::verification_paths(),
        crate::package::PACKAGE_SURGE_XT => surge_xt::verification_paths(artifact.platform),
        _ => vec![resource_path.to_path_buf()],
    };

    paths.sort();
    paths.dedup();
    paths
}

fn preview_artifact_access_step(kind: ArtifactKind) -> String {
    match kind {
        ArtifactKind::Installer => {
            "RABBIT will download the upstream installer during the run.".to_string()
        }
        ArtifactKind::Archive | ArtifactKind::SevenZipArchive => {
            "RABBIT will download the upstream archive during the run.".to_string()
        }
        ArtifactKind::DiskImage => {
            "RABBIT will download the disk image during the run.".to_string()
        }
        ArtifactKind::ExtensionBinary => {
            "RABBIT will use the extension file resolved for this target during the run."
                .to_string()
        }
    }
}

pub(super) fn target_likely_portable(resource_path: &Path, target_app_path: Option<&Path>) -> bool {
    target_app_path
        .is_some_and(|target_app_path| path_is_same_or_nested(target_app_path, resource_path))
}

fn package_title_name(package_id: &str) -> &'static str {
    match package_id {
        crate::package::PACKAGE_REAPER => reaper::TITLE,
        crate::package::PACKAGE_OSARA => osara::TITLE,
        crate::package::PACKAGE_SWS => sws::TITLE,
        crate::package::PACKAGE_REAPACK => "ReaPack",
        crate::package::PACKAGE_REAKONTROL => "ReaKontrol",
        crate::package::PACKAGE_JAWS_SCRIPTS => jaws_scripts::TITLE,
        crate::package::PACKAGE_SURGE_XT => surge_xt::TITLE,
        _ => "package",
    }
}

fn path_is_same_or_nested(path: &Path, root: &Path) -> bool {
    let (path, root) = matched_normalized_paths(path, root);
    path == root || path.starts_with(&root)
}

fn matched_normalized_paths(path: &Path, root: &Path) -> (PathBuf, PathBuf) {
    if let (Ok(canonical_path), Ok(canonical_root)) =
        (std::fs::canonicalize(path), std::fs::canonicalize(root))
    {
        return (
            strip_windows_verbatim_prefix(canonical_path),
            strip_windows_verbatim_prefix(canonical_root),
        );
    }
    (path.to_path_buf(), root.to_path_buf())
}

fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    let raw = path.display().to_string();
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        PackageAutomationSupport, PackageOperationMessage, PackageOperationOptions,
        PackageOperationStatus, PlannedArtifact, PlannedAutomationKind, PlannedExecutionKind,
        dependency_ordered, execute_resolved_package_operation,
        execute_resolved_package_operation_with_detections, first_failed_dependency,
        plan_action_for_artifact,
    };
    use crate::artifact::{ArtifactDescriptor, ArtifactKind};
    use crate::detection::detect_components;
    use crate::error::RabbitError;
    use crate::model::{Architecture, ComponentDetection, Confidence, Platform};
    use crate::package::{
        PACKAGE_OSARA, PACKAGE_REAKONTROL, PACKAGE_REAPACK, PACKAGE_REAPER, PACKAGE_SURGE_XT,
        PACKAGE_SWS,
    };
    use crate::plan::PlanActionKind;
    use crate::receipt::{InstallState, load_install_state, save_install_state};
    use crate::version::Version;
    use std::collections::HashSet;

    #[test]
    fn skips_deferred_artifacts_without_install_report() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let report = execute_resolved_package_operation(
            dir.path(),
            vec![artifact(
                PACKAGE_REAPER,
                ArtifactKind::DiskImage,
                "reaper.dmg",
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert!(report.install_report.is_none());
        assert_eq!(report.items.len(), 1);
        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::DeferredUnattended
        );
        assert!(report.items[0].manual_instruction.is_some());
        assert!(
            report.items[0]
                .manual_instruction
                .as_ref()
                .unwrap()
                .notes
                .iter()
                .any(|note| note.contains("manual completion"))
        );
    }

    #[test]
    fn sorts_report_items_by_package_id() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let report = execute_resolved_package_operation(
            dir.path(),
            vec![
                artifact(PACKAGE_REAPACK, ArtifactKind::Installer, "reapack.exe"),
                artifact(PACKAGE_OSARA, ArtifactKind::Installer, "osara.exe"),
            ],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(report.items[0].package_id, PACKAGE_OSARA);
        assert_eq!(report.items[1].package_id, PACKAGE_REAPACK);
    }

    #[test]
    fn stages_unsupported_artifacts_when_requested() {
        let resource_dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let source_dir = tempdir().unwrap();
        let source_path = source_dir.path().join("reapack-installer.exe");
        fs::write(&source_path, b"installer").unwrap();
        let report = execute_resolved_package_operation(
            resource_dir.path(),
            vec![artifact_with_url(
                PACKAGE_REAPACK,
                ArtifactKind::Installer,
                "reapack-installer.exe",
                &source_path.display().to_string(),
            )],
            cache_dir.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: true,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert!(report.install_report.is_none());
        assert_eq!(report.items.len(), 1);
        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::DeferredUnattended
        );
        assert!(report.items[0].cached_artifact.is_some());
        assert!(
            report.items[0]
                .message
                .contains("staged the artifact in the cache but did not run it")
        );
    }

    #[test]
    fn staged_installer_exposes_launch_plan_with_cached_path() {
        let resource_dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let source_dir = tempdir().unwrap();
        let source_path = source_dir.path().join("reapack-installer.exe");
        fs::write(&source_path, b"installer").unwrap();

        let report = execute_resolved_package_operation(
            resource_dir.path(),
            vec![artifact_with_url(
                PACKAGE_REAPACK,
                ArtifactKind::Installer,
                "reapack-installer.exe",
                &source_path.display().to_string(),
            )],
            cache_dir.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: true,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        let plan = report.items[0].planned_execution.as_ref().unwrap();
        let cached_path = report.items[0]
            .cached_artifact
            .as_ref()
            .unwrap()
            .path
            .display()
            .to_string();

        assert_eq!(plan.kind, PlannedExecutionKind::LaunchInstallerExecutable);
        assert_eq!(plan.artifact_location, cached_path);
        assert_eq!(plan.program.as_deref(), Some(cached_path.as_str()));
        assert_eq!(
            plan.working_directory.as_deref(),
            report.items[0]
                .cached_artifact
                .as_ref()
                .unwrap()
                .path
                .parent()
        );
        assert!(
            plan.verification_paths
                .contains(&resource_dir.path().join("UserPlugins"))
        );
    }

    #[test]
    fn reports_planned_unattended_support_for_installer_artifacts() {
        assert_eq!(
            super::package_automation_support(PACKAGE_REAPER, Platform::Windows, Architecture::X64),
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::VendorInstaller)
        );
        assert_eq!(
            super::package_automation_support(PACKAGE_OSARA, Platform::Windows, Architecture::X64),
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::VendorInstaller)
        );
        assert_eq!(
            super::package_automation_support(PACKAGE_SWS, Platform::Windows, Architecture::X64),
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::VendorInstaller)
        );
        assert_eq!(
            super::package_automation_support(PACKAGE_OSARA, Platform::MacOs, Architecture::Arm64),
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::ArchiveExtraction)
        );
        assert_eq!(
            super::package_automation_support(
                PACKAGE_REAPACK,
                Platform::Windows,
                Architecture::X64
            ),
            PackageAutomationSupport::Direct
        );
        assert_eq!(
            super::package_automation_support(
                PACKAGE_REAKONTROL,
                Platform::Windows,
                Architecture::X64
            ),
            PackageAutomationSupport::Direct
        );
        assert_eq!(
            super::package_automation_support(
                PACKAGE_REAKONTROL,
                Platform::MacOs,
                Architecture::Arm64
            ),
            PackageAutomationSupport::Direct
        );
        assert_eq!(
            super::package_automation_support(PACKAGE_SWS, Platform::MacOs, Architecture::Arm64),
            PackageAutomationSupport::Direct
        );
        assert_eq!(
            super::package_automation_support(PACKAGE_SWS, Platform::MacOs, Architecture::X64),
            PackageAutomationSupport::Direct
        );
        assert_eq!(
            super::package_automation_support(
                PACKAGE_REAPER,
                Platform::MacOs,
                Architecture::Universal
            ),
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::DiskImageInstall)
        );
        assert_eq!(
            super::package_automation_support(
                PACKAGE_OSARA,
                Platform::MacOs,
                Architecture::Universal
            ),
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::ArchiveExtraction)
        );
    }

    #[test]
    fn osara_macos_archive_planned_execution_carries_resource_path() {
        let resource_path = std::path::Path::new("/Users/me/Library/Application Support/REAPER");
        let descriptor = ArtifactDescriptor {
            package_id: PACKAGE_OSARA.to_string(),
            version: Version::parse("2026.4.27.2160").unwrap(),
            platform: Platform::MacOs,
            architecture: Architecture::Universal,
            kind: ArtifactKind::Archive,
            url: "https://github.com/jcsteh/osara/releases/download/snapshots/osara_2026.4.27.2160.89d559fc.zip".to_string(),
            file_name: "osara_2026.4.27.2160.89d559fc.zip".to_string(),
        };

        let plan =
            super::planned_execution_for_artifact(&descriptor, None, resource_path, None, true);

        assert_eq!(
            plan.kind,
            PlannedExecutionKind::ExtractArchiveAndCopyOsaraAssets
        );
        assert_eq!(plan.arguments.len(), 1);
        assert_eq!(plan.arguments[0], resource_path.display().to_string());
    }

    #[test]
    fn reaper_macos_disk_image_planned_execution_targets_app_bundle() {
        let resource_path = std::path::Path::new("/Users/me/Library/Application Support/REAPER");
        let inferred_target = std::path::Path::new("/Applications/REAPER.app");
        let descriptor = ArtifactDescriptor {
            package_id: PACKAGE_REAPER.to_string(),
            version: Version::parse("7.69").unwrap(),
            platform: Platform::MacOs,
            architecture: Architecture::Universal,
            kind: ArtifactKind::DiskImage,
            url: "https://www.reaper.fm/files/7.x/reaper769_universal.dmg".to_string(),
            file_name: "reaper769_universal.dmg".to_string(),
        };

        let plan = super::planned_execution_for_artifact(
            &descriptor,
            None,
            resource_path,
            Some(inferred_target),
            false,
        );

        assert_eq!(
            plan.kind,
            PlannedExecutionKind::MountDiskImageAndCopyAppBundle
        );
        assert_eq!(plan.arguments.len(), 2);
        assert_eq!(plan.arguments[0], "REAPER.app");
        assert_eq!(plan.arguments[1], "/Applications");
    }

    #[test]
    fn reaper_macos_disk_image_planned_execution_uses_resource_path_for_portable() {
        let resource_path = std::path::Path::new("/Users/me/PortableREAPER");
        let portable_target = resource_path.join("REAPER.app");
        let descriptor = ArtifactDescriptor {
            package_id: PACKAGE_REAPER.to_string(),
            version: Version::parse("7.69").unwrap(),
            platform: Platform::MacOs,
            architecture: Architecture::Universal,
            kind: ArtifactKind::DiskImage,
            url: "https://www.reaper.fm/files/7.x/reaper769_universal.dmg".to_string(),
            file_name: "reaper769_universal.dmg".to_string(),
        };

        let plan = super::planned_execution_for_artifact(
            &descriptor,
            None,
            resource_path,
            Some(portable_target.as_path()),
            false,
        );

        assert_eq!(
            plan.kind,
            PlannedExecutionKind::MountDiskImageAndCopyAppBundle
        );
        assert_eq!(plan.arguments[0], "REAPER.app");
        assert_eq!(plan.arguments[1], resource_path.display().to_string());
    }

    #[test]
    fn dry_run_reaper_windows_uses_unattended_plan() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let resource_path = dir.path().join("PortableREAPER");
        std::fs::create_dir_all(&resource_path).unwrap();
        std::fs::write(resource_path.join("reaper.ini"), b"portable").unwrap();

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact(
                PACKAGE_REAPER,
                ArtifactKind::Installer,
                "reaper-install.exe",
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(report.items.len(), 1);
        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::PlannedUnattended
        );
        assert!(report.items[0].manual_instruction.is_none());
        assert_eq!(
            report.items[0]
                .planned_execution
                .as_ref()
                .unwrap()
                .arguments,
            vec![
                "/PORTABLE".to_string(),
                "/S".to_string(),
                format!("/D={}", resource_path.display()),
            ]
        );
    }

    #[test]
    fn dry_run_reaper_windows_standard_verifies_app_without_resource_directory() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let resource_path = dir.path().join("Roaming").join("REAPER");
        let target_app_path = dir
            .path()
            .join("Program Files")
            .join("REAPER")
            .join("reaper.exe");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact(
                PACKAGE_REAPER,
                ArtifactKind::Installer,
                "reaper-install.exe",
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(target_app_path.clone()),
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        let plan = report.items[0].planned_execution.as_ref().unwrap();

        assert_eq!(plan.kind, PlannedExecutionKind::LaunchInstallerExecutable);
        assert_eq!(
            plan.arguments,
            vec![
                "/S".to_string(),
                format!("/D={}", target_app_path.parent().unwrap().display())
            ]
        );
        assert_eq!(plan.verification_paths, vec![target_app_path]);
    }

    #[test]
    fn dry_run_osara_windows_preserve_uses_unattended_plan() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact(
                PACKAGE_OSARA,
                ArtifactKind::Installer,
                "osara.exe",
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(report.items.len(), 1);
        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::PlannedUnattended
        );
        assert!(report.items[0].manual_instruction.is_none());
        assert_eq!(
            report.items[0]
                .planned_execution
                .as_ref()
                .unwrap()
                .arguments,
            vec!["/S".to_string(), format!("/D={}", resource_path.display()),]
        );
    }

    #[test]
    fn dry_run_osara_windows_replace_uses_unattended_plan() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact(
                PACKAGE_OSARA,
                ArtifactKind::Installer,
                "osara.exe",
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: true,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(report.items.len(), 1);
        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::PlannedUnattended
        );
        assert!(report.items[0].manual_instruction.is_none());
        assert!(
            report.items[0]
                .planned_execution
                .as_ref()
                .unwrap()
                .verification_paths
                .contains(&resource_path.join("reaper-kb.ini"))
        );
    }

    #[test]
    fn dry_run_sws_windows_uses_unattended_plan() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact(
                PACKAGE_SWS,
                ArtifactKind::Installer,
                "sws-installer.exe",
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(report.items.len(), 1);
        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::PlannedUnattended
        );
        assert!(report.items[0].manual_instruction.is_none());
        assert_eq!(
            report.items[0]
                .planned_execution
                .as_ref()
                .unwrap()
                .arguments,
            vec!["/S".to_string(), format!("/D={}", resource_path.display()),]
        );
        assert!(
            report.items[0]
                .planned_execution
                .as_ref()
                .unwrap()
                .verification_paths
                .contains(&resource_path.join("UserPlugins").join("reaper_sws-x64.dll"))
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn executes_reaper_windows_portable_installer_unattended_and_writes_receipt() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let source_path = dir.path().join("reaper-installer.cmd");
        std::fs::write(&source_path, reaper_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact_with_url(
                PACKAGE_REAPER,
                ArtifactKind::Installer,
                "reaper-installer.cmd",
                &source_path.display().to_string(),
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: Some(dir.path().join("install.lock")),
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::InstalledOrChecked
        );
        assert!(
            report.items[0]
                .message
                .contains("updated the RABBIT receipt")
        );

        let state = load_install_state(&resource_path).unwrap().unwrap();
        let receipt = state.packages.get(PACKAGE_REAPER).unwrap();
        assert_eq!(receipt.version.as_ref().unwrap().raw(), "1.2.3");
        assert!(
            receipt
                .installed_files
                .iter()
                .any(|file| file.path == *"reaper.exe")
        );
        assert!(
            receipt
                .installed_files
                .iter()
                .any(|file| file.path == *"reaper.ini")
        );

        let detections = detect_components(&resource_path, Platform::Windows).unwrap();
        let reaper = detections
            .iter()
            .find(|detection| detection.package_id == PACKAGE_REAPER)
            .unwrap();
        assert!(reaper.installed);
        assert_eq!(reaper.detector, "rabbit-receipt");
        assert_eq!(reaper.version.as_ref().unwrap().raw(), "1.2.3");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn executes_reaper_windows_standard_installer_and_receipt_tracks_app_only() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let source_path = dir.path().join("reaper-installer.cmd");
        std::fs::write(&source_path, reaper_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("AppData").join("Roaming").join("REAPER");
        std::fs::create_dir_all(&resource_path).unwrap();
        let target_app_path = dir
            .path()
            .join("Program Files")
            .join("REAPER")
            .join("reaper.exe");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact_with_url(
                PACKAGE_REAPER,
                ArtifactKind::Installer,
                "reaper-installer.cmd",
                &source_path.display().to_string(),
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(target_app_path.clone()),
                lock_path: Some(dir.path().join("install.lock")),
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::InstalledOrChecked
        );
        assert!(target_app_path.is_file());
        assert!(!resource_path.join("reaper.ini").exists());

        let state = load_install_state(&resource_path).unwrap().unwrap();
        let receipt = state.packages.get(PACKAGE_REAPER).unwrap();
        assert_eq!(receipt.installed_files.len(), 1);
        assert_eq!(receipt.installed_files[0].path, target_app_path);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn executes_osara_windows_installer_unattended_and_cleans_portable_uninstaller() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let source_path = dir.path().join("osara-installer.cmd");
        std::fs::write(&source_path, osara_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact_with_url(
                PACKAGE_OSARA,
                ArtifactKind::Installer,
                "osara-installer.cmd",
                &source_path.display().to_string(),
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::InstalledOrChecked
        );
        assert!(resource_path.join("UserPlugins").is_dir());
        assert!(
            resource_path
                .join("KeyMaps")
                .join("OSARA.ReaperKeyMap")
                .is_file()
        );
        assert!(resource_path.join("osara").join("locale").is_dir());
        assert!(!resource_path.join("osara").join("uninstall.exe").exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn executes_osara_windows_installer_unattended_and_replaces_keymap_with_backup() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let source_path = dir.path().join("osara-installer.cmd");
        std::fs::write(&source_path, osara_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("PortableREAPER");
        std::fs::create_dir_all(&resource_path).unwrap();
        std::fs::write(resource_path.join("reaper-kb.ini"), b"old keymap").unwrap();

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact_with_url(
                PACKAGE_OSARA,
                ArtifactKind::Installer,
                "osara-installer.cmd",
                &source_path.display().to_string(),
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: true,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: Some(dir.path().join("install.lock")),
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::InstalledOrChecked
        );
        assert_eq!(
            std::fs::read_to_string(resource_path.join("reaper-kb.ini")).unwrap(),
            "osara keymap\r\n"
        );
        assert_eq!(report.items[0].backup_paths.len(), 1);
        assert_eq!(
            std::fs::read(&report.items[0].backup_paths[0]).unwrap(),
            b"old keymap"
        );
        assert!(report.items[0].backup_manifest_path.is_some());
        assert!(
            report.items[0]
                .message
                .contains("applied the OSARA key map replacement")
        );
        assert!(!resource_path.join("osara").join("uninstall.exe").exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn executes_osara_windows_installer_unattended_and_creates_keymap_for_new_portable_target() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let source_path = dir.path().join("osara-installer.cmd");
        std::fs::write(&source_path, osara_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact_with_url(
                PACKAGE_OSARA,
                ArtifactKind::Installer,
                "osara-installer.cmd",
                &source_path.display().to_string(),
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: true,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: Some(dir.path().join("install.lock")),
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::InstalledOrChecked
        );
        assert_eq!(
            std::fs::read_to_string(resource_path.join("reaper-kb.ini")).unwrap(),
            "osara keymap\r\n"
        );
        assert!(report.items[0].backup_paths.is_empty());
        assert!(report.items[0].backup_manifest_path.is_none());
        assert!(
            report.items[0]
                .message
                .contains("applied the OSARA key map replacement")
        );
        assert!(!resource_path.join("osara").join("uninstall.exe").exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn executes_sws_windows_installer_unattended() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let source_path = dir.path().join("sws-installer.cmd");
        std::fs::write(&source_path, sws_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact_with_url(
                PACKAGE_SWS,
                ArtifactKind::Installer,
                "sws-installer.cmd",
                &source_path.display().to_string(),
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: Some(dir.path().join("install.lock")),
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::InstalledOrChecked
        );
        assert!(
            resource_path
                .join("UserPlugins")
                .join("reaper_sws-x64.dll")
                .is_file()
        );
        assert!(
            resource_path
                .join("Scripts")
                .join("sws_python.py")
                .is_file()
        );
        assert!(resource_path.join("Data").join("Grooves").is_dir());
    }

    /// A later package's download failure no longer aborts the run: OSARA
    /// installs fine, SWS's artifact URL points nowhere. The operation now
    /// returns Ok with SWS marked Failed (carrying the missing-artifact
    /// error) while OSARA is InstalledOrChecked and its receipt is on disk.
    /// SWS declares `depends_on: ["reaper"]`, but REAPER isn't part of this
    /// run, so SWS is attempted (and fails on its own), not dependency-
    /// skipped.
    #[cfg(target_os = "windows")]
    #[test]
    fn continues_and_records_failure_when_a_later_download_fails() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let osara_source = dir.path().join("osara-installer.cmd");
        std::fs::write(&osara_source, osara_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![
                artifact_with_url(
                    PACKAGE_OSARA,
                    ArtifactKind::Installer,
                    "osara-installer.cmd",
                    &osara_source.display().to_string(),
                ),
                artifact_with_url(
                    PACKAGE_SWS,
                    ArtifactKind::Installer,
                    "sws-installer.cmd",
                    &dir.path().join("does-not-exist.cmd").display().to_string(),
                ),
            ],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: Some(dir.path().join("install.lock")),
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        let osara_item = report
            .items
            .iter()
            .find(|item| item.package_id == PACKAGE_OSARA)
            .expect("OSARA item present");
        assert_eq!(
            osara_item.status,
            PackageOperationStatus::InstalledOrChecked
        );

        let sws_item = report
            .items
            .iter()
            .find(|item| item.package_id == PACKAGE_SWS)
            .expect("SWS item present");
        assert_eq!(sws_item.status, PackageOperationStatus::Failed);
        // The failure names the missing artifact, not OSARA.
        assert!(
            sws_item.message.contains("does-not-exist.cmd"),
            "unexpected SWS message: {}",
            sws_item.message
        );
        assert!(matches!(
            sws_item.message_code,
            PackageOperationMessage::InstallFailed { .. }
        ));

        // OSARA really installed and its receipt survived the SWS failure.
        assert!(
            resource_path
                .join("UserPlugins")
                .join("reaper_osara64.dll")
                .is_file()
        );
        let state = load_install_state(&resource_path).unwrap().unwrap();
        assert!(
            state.packages.contains_key(PACKAGE_OSARA),
            "OSARA's receipt must be saved even though SWS's download failed"
        );
        assert!(!state.packages.contains_key(PACKAGE_SWS));
    }

    #[test]
    fn dependency_ordered_moves_reaper_ahead_of_its_extensions() {
        let planned = |package_id: &str| PlannedArtifact {
            artifact: artifact_with_url(
                package_id,
                ArtifactKind::Installer,
                "x.cmd",
                "https://example.test/x.cmd",
            ),
            plan_action: PlanActionKind::Install,
        };
        // Feed the extensions BEFORE REAPER; ordering must still install
        // REAPER first so the dependency gate sees its result.
        let ordered = dependency_ordered(vec![
            planned(PACKAGE_OSARA),
            planned(PACKAGE_SURGE_XT),
            planned(PACKAGE_SWS),
            planned(PACKAGE_REAPER),
        ]);
        let ids: Vec<&str> = ordered
            .iter()
            .map(|item| item.artifact.package_id.as_str())
            .collect();
        let pos = |id: &str| ids.iter().position(|candidate| *candidate == id).unwrap();
        // The dependency invariant: REAPER installs before its extensions.
        assert!(pos(PACKAGE_REAPER) < pos(PACKAGE_OSARA));
        assert!(pos(PACKAGE_REAPER) < pos(PACKAGE_SWS));
        // Stable within a readiness round: the two dependency-free roots
        // (Surge XT, REAPER) keep their input order.
        assert!(pos(PACKAGE_SURGE_XT) < pos(PACKAGE_REAPER));
    }

    #[test]
    fn first_failed_dependency_gates_only_on_in_run_failures() {
        // OSARA depends on REAPER (per the embedded manifest).
        let mut failed = HashSet::new();
        assert_eq!(first_failed_dependency(PACKAGE_OSARA, &failed), None);
        failed.insert(PACKAGE_REAPER.to_string());
        assert_eq!(
            first_failed_dependency(PACKAGE_OSARA, &failed).as_deref(),
            Some(PACKAGE_REAPER)
        );
        // Surge XT is standalone — a failed REAPER never gates it.
        assert_eq!(first_failed_dependency(PACKAGE_SURGE_XT, &failed), None);
    }

    /// When REAPER itself fails, the extensions that install into it are
    /// skipped (not attempted) with a distinct dependency-skip status, so
    /// RABBIT never installs OSARA into a REAPER that didn't land. REAPER's
    /// download points nowhere; OSARA has a working mock installer but must
    /// never run.
    #[cfg(target_os = "windows")]
    #[test]
    fn failed_reaper_cascades_a_dependency_skip_to_its_extensions() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let osara_source = dir.path().join("osara-installer.cmd");
        std::fs::write(&osara_source, osara_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![
                artifact_with_url(
                    PACKAGE_REAPER,
                    ArtifactKind::Installer,
                    "reaper-installer.cmd",
                    &dir.path().join("no-reaper.cmd").display().to_string(),
                ),
                artifact_with_url(
                    PACKAGE_OSARA,
                    ArtifactKind::Installer,
                    "osara-installer.cmd",
                    &osara_source.display().to_string(),
                ),
            ],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: Some(dir.path().join("install.lock")),
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        let reaper_item = report
            .items
            .iter()
            .find(|item| item.package_id == PACKAGE_REAPER)
            .expect("REAPER item present");
        assert_eq!(reaper_item.status, PackageOperationStatus::Failed);

        let osara_item = report
            .items
            .iter()
            .find(|item| item.package_id == PACKAGE_OSARA)
            .expect("OSARA item present");
        assert_eq!(
            osara_item.status,
            PackageOperationStatus::SkippedDependencyFailed
        );
        assert!(matches!(
            &osara_item.message_code,
            PackageOperationMessage::SkippedDependencyFailed { dependency }
                if dependency == PACKAGE_REAPER
        ));

        // OSARA's mock installer must NOT have run.
        assert!(
            !resource_path
                .join("UserPlugins")
                .join("reaper_osara64.dll")
                .is_file()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn unattended_installers_backup_existing_receipt_once_and_merge_package_state() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let osara_source = dir.path().join("osara-installer.cmd");
        std::fs::write(&osara_source, osara_mock_installer_script()).unwrap();
        let sws_source = dir.path().join("sws-installer.cmd");
        std::fs::write(&sws_source, sws_mock_installer_script()).unwrap();
        let resource_path = dir.path().join("PortableREAPER");
        std::fs::create_dir_all(&resource_path).unwrap();
        save_install_state(&resource_path, &InstallState::default()).unwrap();

        let report = execute_resolved_package_operation(
            &resource_path,
            vec![
                artifact_with_url(
                    PACKAGE_OSARA,
                    ArtifactKind::Installer,
                    "osara-installer.cmd",
                    &osara_source.display().to_string(),
                ),
                artifact_with_url(
                    PACKAGE_SWS,
                    ArtifactKind::Installer,
                    "sws-installer.cmd",
                    &sws_source.display().to_string(),
                ),
            ],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(resource_path.join("reaper.exe")),
                lock_path: Some(dir.path().join("install.lock")),
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert!(
            report
                .receipt_backup_path
                .as_ref()
                .is_some_and(|path| path.is_file())
        );
        assert!(
            report
                .receipt_backup_manifest_path
                .as_ref()
                .is_some_and(|path| path.is_file())
        );
        assert_eq!(
            std::fs::read_dir(resource_path.join("RABBIT").join("backups"))
                .unwrap()
                .count(),
            1
        );

        let state = load_install_state(&resource_path).unwrap().unwrap();
        assert!(state.packages.contains_key(PACKAGE_OSARA));
        assert!(state.packages.contains_key(PACKAGE_SWS));
    }

    #[test]
    fn skips_current_artifacts_before_download() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let report = execute_resolved_package_operation_with_detections(
            dir.path(),
            vec![artifact(
                PACKAGE_REAPACK,
                ArtifactKind::ExtensionBinary,
                "reaper_reapack-x64.dll",
            )],
            &[detection(PACKAGE_REAPACK, Some("1.2.3"))],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        assert!(report.install_report.is_none());
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].plan_action, PlanActionKind::Keep);
        assert_eq!(
            report.items[0].status,
            PackageOperationStatus::SkippedCurrent
        );
        assert!(report.items[0].cached_artifact.is_none());
    }

    #[test]
    fn plans_update_for_installed_package_when_version_is_unknown() {
        // RABBIT used to surface "Review manually" / "Manuell prüfen" in the
        // wizard whenever a package was installed but its version could not
        // be read. The plan-action resolver now returns Update for that
        // case so a non-technical user does not have to act on internal
        // detection failures.
        let artifact = artifact(
            PACKAGE_REAPACK,
            ArtifactKind::ExtensionBinary,
            "reaper_reapack-x64.dll",
        );
        let detections = [detection(PACKAGE_REAPACK, None)];

        let action = plan_action_for_artifact(&artifact, &detections);

        assert_eq!(action, PlanActionKind::Update);
    }

    #[test]
    fn osara_manual_instruction_reflects_replace_keymap_choice() {
        let dir = tempdir().unwrap();
        let instruction = super::preview_manual_instruction(
            PACKAGE_OSARA,
            ArtifactKind::Installer,
            dir.path(),
            None,
            true,
        );

        assert!(
            instruction
                .notes
                .iter()
                .any(|note| note.contains("Back up") && note.contains("reaper-kb.ini"))
        );
    }

    #[test]
    fn staged_unsupported_instruction_points_to_cached_artifact() {
        let resource_dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let source_dir = tempdir().unwrap();
        let source_path = source_dir.path().join("reapack-installer.exe");
        fs::write(&source_path, b"installer").unwrap();

        let report = execute_resolved_package_operation(
            resource_dir.path(),
            vec![artifact_with_url(
                PACKAGE_REAPACK,
                ArtifactKind::Installer,
                "reapack-installer.exe",
                &source_path.display().to_string(),
            )],
            cache_dir.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: true,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        let cached_path = report.items[0]
            .cached_artifact
            .as_ref()
            .unwrap()
            .path
            .display()
            .to_string();
        assert!(
            report.items[0]
                .manual_instruction
                .as_ref()
                .unwrap()
                .steps
                .iter()
                .any(|step| step.contains(&cached_path))
        );
    }

    #[test]
    fn reaper_manual_instruction_mentions_portable_install_folder() {
        let dir = tempdir().unwrap();
        let resource_path = dir.path().join("PortableREAPER");
        let instruction = super::preview_manual_instruction(
            PACKAGE_REAPER,
            ArtifactKind::Installer,
            &resource_path,
            Some(&resource_path.join("reaper.exe")),
            false,
        );

        assert!(
            instruction
                .steps
                .iter()
                .any(|step| step.contains("Portable install") && step.contains("PortableREAPER"))
        );
    }

    #[test]
    fn reaper_portable_plan_verifies_app_and_reaper_ini() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let resource_path = dir.path().join("PortableREAPER");
        let target_app_path = resource_path.join("reaper.exe");
        let report = execute_resolved_package_operation(
            &resource_path,
            vec![artifact(
                PACKAGE_REAPER,
                ArtifactKind::Installer,
                "reaper-install.exe",
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(target_app_path.clone()),
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        )
        .unwrap();

        let plan = report.items[0].planned_execution.as_ref().unwrap();

        assert_eq!(plan.kind, PlannedExecutionKind::LaunchInstallerExecutable);
        assert!(
            plan.verification_paths.contains(&target_app_path),
            "missing target app path in verification set: {:?}",
            plan.verification_paths
        );
        assert!(
            plan.verification_paths
                .contains(&resource_path.join("reaper.ini")),
            "missing reaper.ini in verification set: {:?}",
            plan.verification_paths
        );
    }

    #[test]
    fn osara_manual_instruction_mentions_selected_resource_path() {
        let dir = tempdir().unwrap();
        let instruction = super::preview_manual_instruction(
            PACKAGE_OSARA,
            ArtifactKind::Installer,
            dir.path(),
            None,
            false,
        );

        assert!(
            instruction
                .steps
                .iter()
                .any(|step| step.contains(&dir.path().display().to_string()))
        );
    }

    #[test]
    fn preview_manual_instruction_uses_preview_download_step() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("reaper.exe"), b"stub").unwrap();
        let instruction = super::preview_manual_instruction(
            PACKAGE_REAPER,
            ArtifactKind::Installer,
            dir.path(),
            Some(&dir.path().join("reaper.exe")),
            false,
        );

        assert!(instruction.steps[0].contains("download the upstream installer"));
        assert!(
            instruction
                .steps
                .iter()
                .any(|step| step.contains("Portable install"))
        );
    }

    #[test]
    fn fails_target_preflight_before_attempting_download() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let resource_path = dir.path().join("ProtectedREAPER");
        // Capture the original permissions so we can restore them verbatim
        // for tempdir cleanup, rather than calling `set_readonly(false)`
        // (which would widen the dir to world-writable on Unix).
        let original_permissions = fs::metadata(dir.path()).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(dir.path(), permissions).unwrap();

        let result = execute_resolved_package_operation(
            &resource_path,
            vec![artifact_with_url(
                PACKAGE_REAPACK,
                ArtifactKind::ExtensionBinary,
                "reaper_reapack-x64.dll",
                "http://example.test/reaper_reapack-x64.dll",
            )],
            cache.path(),
            &PackageOperationOptions {
                dry_run: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
            },
        );

        fs::set_permissions(dir.path(), original_permissions).unwrap();

        match result.unwrap_err() {
            RabbitError::PreflightFailed { message } => {
                assert!(message.contains("resource-path"));
                assert!(message.contains("read-only"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn artifact(package_id: &str, kind: ArtifactKind, file_name: &str) -> ArtifactDescriptor {
        artifact_with_url(
            package_id,
            kind,
            file_name,
            &format!("https://example.test/{file_name}"),
        )
    }

    fn artifact_with_url(
        package_id: &str,
        kind: ArtifactKind,
        file_name: &str,
        url: &str,
    ) -> ArtifactDescriptor {
        ArtifactDescriptor {
            package_id: package_id.to_string(),
            version: Version::parse("1.2.3").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind,
            url: url.to_string(),
            file_name: file_name.to_string(),
        }
    }

    fn detection(package_id: &str, version: Option<&str>) -> ComponentDetection {
        ComponentDetection {
            package_id: package_id.to_string(),
            display_name: package_id.to_string(),
            installed: true,
            version: version.map(|version| Version::parse(version).unwrap()),
            detector: "test".to_string(),
            confidence: Confidence::High,
            files: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[cfg(target_os = "windows")]
    fn osara_mock_installer_script() -> &'static str {
        r#"@echo off
setlocal EnableExtensions EnableDelayedExpansion
set "DEST="
:next
if "%~1"=="" goto args_done
set "ARG=%~1"
if /I "!ARG:~0,3!"=="/D=" set "DEST=!ARG:~3!"
shift
goto next
:args_done
if "%DEST%"=="" exit /b 4
mkdir "%DEST%\UserPlugins" 2>nul
mkdir "%DEST%\KeyMaps" 2>nul
mkdir "%DEST%\osara\locale" 2>nul
echo osara dll> "%DEST%\UserPlugins\reaper_osara64.dll"
echo osara keymap> "%DEST%\KeyMaps\OSARA.ReaperKeyMap"
echo en locale> "%DEST%\osara\locale\en.po"
echo uninstall> "%DEST%\osara\uninstall.exe"
exit /b 0
"#
    }

    #[cfg(target_os = "windows")]
    fn reaper_mock_installer_script() -> &'static str {
        r#"@echo off
setlocal EnableExtensions EnableDelayedExpansion
set "DEST="
set "PORTABLE=0"
:next
if "%~1"=="" goto args_done
set "ARG=%~1"
if /I "!ARG!"=="/PORTABLE" set "PORTABLE=1"
if /I "!ARG:~0,3!"=="/D=" set "DEST=!ARG:~3!"
shift
goto next
:args_done
if "%DEST%"=="" exit /b 4
mkdir "%DEST%" 2>nul
echo reaper exe> "%DEST%\reaper.exe"
if "%PORTABLE%"=="1" echo portable ini> "%DEST%\reaper.ini"
exit /b 0
"#
    }

    #[cfg(target_os = "windows")]
    fn sws_mock_installer_script() -> &'static str {
        r#"@echo off
setlocal EnableExtensions EnableDelayedExpansion
set "DEST="
:next
if "%~1"=="" goto args_done
set "ARG=%~1"
if /I "!ARG:~0,3!"=="/D=" set "DEST=!ARG:~3!"
shift
goto next
:args_done
if "%DEST%"=="" exit /b 4
mkdir "%DEST%\UserPlugins" 2>nul
mkdir "%DEST%\Scripts" 2>nul
mkdir "%DEST%\Data\Grooves" 2>nul
type nul > "%DEST%\UserPlugins\reaper_sws-x64.dll"
type nul > "%DEST%\Scripts\sws_python.py"
type nul > "%DEST%\Data\Grooves\default.rgt"
exit /b 0
"#
    }
}
