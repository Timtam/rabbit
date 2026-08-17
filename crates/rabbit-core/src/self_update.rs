use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::error::{IoPathContext, RabbitError};
use crate::hash::sha256_file;
use crate::model::{Architecture, Platform};
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::signature::{SignatureVerdict, verify_executable_signature};
use crate::version::Version;

const ROLLBACK_SUFFIX: &str = "rabbit-old";

const USER_AGENT: &str = concat!(
    "RABBIT/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Timtam/rabbit)"
);

pub const DEFAULT_SELF_UPDATE_MANIFEST_URL: &str =
    "https://github.com/Timtam/rabbit/releases/latest/download/rabbit-update-stable.json";

/// GitHub's release listing for RABBIT itself — the What's-New source behind
/// the notes shown next to an available self-update. It is the same endpoint
/// shape ReaPack's `whats_new` rule reads, so
/// [`crate::latest::resolve_github_release_bodies`] renders both the same
/// way: one heading line per release, then the body the maintainer wrote.
/// `per_page` is capped well above the ten sections the renderer stops at,
/// so asking for more would only cost bandwidth. Deliberately separate from
/// [`DEFAULT_SELF_UPDATE_MANIFEST_URL`]: the manifest is a release asset
/// carrying download URLs and checksums, while the notes come from the
/// release metadata GitHub keeps for every published tag.
pub const DEFAULT_SELF_UPDATE_RELEASE_NOTES_URL: &str =
    "https://api.github.com/repos/Timtam/rabbit/releases?per_page=20";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfUpdateManifest {
    pub version: Version,
    pub channel: String,
    pub published_at: String,
    pub release_notes_url: Option<String>,
    pub minimum_supported_previous_version: Option<Version>,
    pub assets: SelfUpdateAssets,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfUpdateAssets {
    /// Legacy single-arch slot kept for backward compatibility with RABBIT
    /// releases that predate the per-arch schema. New publishers should
    /// continue to populate this with the primary-arch asset (x86_64 on
    /// Windows) so old clients keep working.
    pub windows: Option<SelfUpdateAsset>,
    /// Legacy single-arch slot, mirrors `windows`. Primary arch on macOS
    /// is aarch64 (Apple Silicon).
    pub macos: Option<SelfUpdateAsset>,
    /// Authoritative per-arch table when present. Keys are
    /// `<platform>-<arch>` (e.g. `windows-x86_64`, `macos-aarch64`); the
    /// arch tokens match `Architecture::release_artifact_token()`. New
    /// clients prefer this over the legacy fields; old clients ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platforms: Option<BTreeMap<String, SelfUpdateAsset>>,
    /// The zipped, notarized universal `Rabbit.app` bundle. macOS clients
    /// running from inside an `.app` prefer this over the bare binary so
    /// the whole bundle is replaced and the Developer ID signature +
    /// notarization survive the update. Lives OUTSIDE `platforms` on
    /// purpose: clients up to 0.3.3 hard-validate every `platforms` key
    /// against an `<os>-<arch>` grammar and would reject the entire
    /// manifest over an unknown key, while unknown sibling FIELDS are
    /// ignored by their deserializer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macos_app: Option<SelfUpdateAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfUpdateAsset {
    pub url: String,
    pub sha256: String,
}

/// What shape the selected self-update asset has, deciding how `apply`
/// installs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SelfUpdateAssetKind {
    /// A bare executable copied over the install target: Windows, macOS
    /// installs outside an `.app` bundle, and every legacy manifest.
    #[default]
    Binary,
    /// A zipped, Developer-ID-signed and notarized `.app` bundle that
    /// replaces the installed bundle wholesale. Keeps the signature and
    /// notarization intact across updates, so Gatekeeper stays happy and
    /// macOS permission grants (keyed to the code signature) survive.
    MacAppBundle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfUpdateAssetSelection {
    pub platform: Platform,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub kind: SelfUpdateAssetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfUpdateCheckReport {
    pub manifest_url: String,
    pub current_version: Version,
    pub latest_version: Version,
    pub channel: String,
    pub published_at: String,
    pub release_notes_url: Option<String>,
    pub minimum_supported_previous_version: Option<Version>,
    pub update_available: bool,
    pub requires_manual_transition: bool,
    pub asset: SelfUpdateAssetSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfUpdateStageReport {
    pub check: SelfUpdateCheckReport,
    pub staging_dir: PathBuf,
    pub staged_asset_path: Option<PathBuf>,
    pub downloaded: bool,
    pub reused_existing_file: bool,
    pub verified_sha256: Option<String>,
    pub ready_to_apply: bool,
    pub status_message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ApplySelfUpdateOptions {
    /// Override the directory the swap operates in. Defaults to the parent
    /// of `current_exe()` (`current_install_root`).
    pub install_root: Option<PathBuf>,
    /// Override the install target's filename. Defaults to the basename of
    /// `current_exe()`. The new artifact filename
    /// (`rabbit-<version>-<os>-<arch>[.exe]`) does not have to match the
    /// install target — RABBIT swaps in place under the user's existing
    /// filename regardless of what the download was called.
    pub install_target_basename: Option<String>,
    /// Where to send the apply phase's progress events. `None` — the
    /// default — reports nothing, which is what the CLI and the tests want.
    /// A GUI passes a reporter so its progress dialog can say "Installing"
    /// once the download is done and close itself when the swap lands.
    pub progress: Option<ProgressReporter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfUpdateApplyReport {
    pub stage: SelfUpdateStageReport,
    pub install_root: PathBuf,
    pub replaced_files: Vec<ReplacedFile>,
    pub skipped_files: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signature_verdicts: Vec<SignatureVerdictRecord>,
    pub status_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureVerdictRecord {
    pub source_path: PathBuf,
    pub verdict: SignatureVerdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacedFile {
    pub install_path: PathBuf,
    pub backup_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawSelfUpdateManifest {
    version: String,
    channel: String,
    published_at: String,
    release_notes_url: Option<String>,
    minimum_supported_previous_version: Option<String>,
    assets: RawSelfUpdateAssets,
}

#[derive(Debug, Deserialize)]
struct RawSelfUpdateAssets {
    windows: Option<RawSelfUpdateAsset>,
    macos: Option<RawSelfUpdateAsset>,
    #[serde(default)]
    platforms: Option<BTreeMap<String, RawSelfUpdateAsset>>,
    #[serde(default)]
    macos_app: Option<RawSelfUpdateAsset>,
}

#[derive(Debug, Deserialize)]
struct RawSelfUpdateAsset {
    url: String,
    sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemanticVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

pub fn current_rabbit_version() -> Result<Version> {
    parse_semantic_version(
        env!("CARGO_PKG_VERSION"),
        "build-metadata",
        "current_version",
    )
}

/// Ephemeral staging directory for the self-update download. Lives under
/// the OS temp dir (cleaned periodically by the OS) so RABBIT doesn't leave
/// persistent files in `%LOCALAPPDATA%` / `~/Library/Caches/`. Callers
/// generally don't need to keep this around between runs — the download is
/// validated, swapped in place, and the staging dir is removed at the end
/// of `apply_self_update`.
pub fn default_self_update_staging_dir() -> PathBuf {
    env::temp_dir().join("rabbit-self-update")
}

pub fn fetch_self_update_manifest(manifest_url: &str) -> Result<SelfUpdateManifest> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|source| RabbitError::Http {
            url: "client-builder".to_string(),
            source,
        })?;

    let body = client
        .get(manifest_url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|source| RabbitError::Http {
            url: manifest_url.to_string(),
            source,
        })?
        .text()
        .map_err(|source| RabbitError::Http {
            url: manifest_url.to_string(),
            source,
        })?;

    parse_self_update_manifest(&body, manifest_url)
}

pub fn parse_self_update_manifest(body: &str, manifest_url: &str) -> Result<SelfUpdateManifest> {
    let raw: RawSelfUpdateManifest =
        serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
            url: manifest_url.to_string(),
            message: source.to_string(),
        })?;

    let version = parse_semantic_version(&raw.version, manifest_url, "version")?;
    let minimum_supported_previous_version = raw
        .minimum_supported_previous_version
        .as_deref()
        .map(|value| {
            parse_semantic_version(value, manifest_url, "minimum_supported_previous_version")
        })
        .transpose()?;
    let platforms = raw
        .assets
        .platforms
        .as_ref()
        .map(|raw_platforms| {
            raw_platforms
                .iter()
                .map(|(key, asset)| {
                    validate_platform_key(key, manifest_url)?;
                    let parsed = parse_asset(asset, manifest_url, key)?;
                    Ok::<_, RabbitError>((key.clone(), parsed))
                })
                .collect::<Result<BTreeMap<_, _>>>()
        })
        .transpose()?;
    let assets = SelfUpdateAssets {
        windows: raw
            .assets
            .windows
            .as_ref()
            .map(|asset| parse_asset(asset, manifest_url, "windows"))
            .transpose()?,
        macos: raw
            .assets
            .macos
            .as_ref()
            .map(|asset| parse_asset(asset, manifest_url, "macos"))
            .transpose()?,
        platforms,
        macos_app: raw
            .assets
            .macos_app
            .as_ref()
            .map(|asset| parse_asset(asset, manifest_url, "macos_app"))
            .transpose()?,
    };

    Ok(SelfUpdateManifest {
        version,
        channel: raw.channel,
        published_at: raw.published_at,
        release_notes_url: raw.release_notes_url,
        minimum_supported_previous_version,
        assets,
    })
}

pub fn check_self_update(platform: Platform, manifest_url: &str) -> Result<SelfUpdateCheckReport> {
    let manifest = fetch_self_update_manifest(manifest_url)?;
    evaluate_self_update_report(
        platform,
        Architecture::current(),
        manifest_url,
        current_rabbit_version()?,
        &manifest,
    )
}

/// The release notes covering every RABBIT release above `installed`, newest
/// first, rendered the same way the packages page renders a package's
/// What's-New notes. Someone who skipped two releases reads all three sets of
/// notes rather than only the newest, which is the whole point of showing
/// them at the update prompt: the answer to "what do I get if I say yes?"
/// spans everything they missed, not just the last tag.
///
/// The section cap and the draft/prerelease skip live in
/// [`crate::latest::resolve_github_release_bodies`]; RABBIT knows its own
/// installed version exactly (it is compiled in), so the trim is precise
/// here in a way it can never be for a third-party package detected on disk.
pub fn fetch_self_update_release_notes(notes_url: &str, installed: &Version) -> Result<String> {
    let client = crate::latest::build_http_client()?;
    let body = crate::latest::http_get_text(&client, notes_url)?;
    crate::latest::resolve_github_release_bodies(&body, notes_url, Some(installed))
}

/// Best-effort [`fetch_self_update_release_notes`] for a completed check.
/// `None` when there is nothing to update to, or when the notes can't be
/// fetched or rendered.
///
/// Notes are decoration around the update prompt, never a precondition for
/// it: a GitHub outage, an API rate limit, or a release published without a
/// body must not keep the user from updating, so every failure degrades to
/// the plain prompt instead of surfacing an error. This mirrors how the
/// packages page treats a package's What's-New source.
pub fn resolve_self_update_release_notes(report: &SelfUpdateCheckReport) -> Option<String> {
    if !report.update_available {
        return None;
    }
    fetch_self_update_release_notes(
        DEFAULT_SELF_UPDATE_RELEASE_NOTES_URL,
        &report.current_version,
    )
    .ok()
}

/// Progress identity for RABBIT's own update, standing where a package id
/// stands for an install. RABBIT is not a package — it is never in the
/// manifest and never in an install plan — but the download engine and the
/// UI both speak [`ProgressEvent`], so borrowing the slot lets the update
/// reuse that pipeline whole rather than growing a parallel one.
pub const SELF_UPDATE_PROGRESS_ID: &str = "rabbit-self-update";

pub fn stage_self_update(
    platform: Platform,
    manifest_url: &str,
    staging_dir: &Path,
) -> Result<SelfUpdateStageReport> {
    stage_self_update_with_progress(
        platform,
        manifest_url,
        staging_dir,
        &ProgressReporter::noop(),
    )
}

/// [`stage_self_update`] reporting download progress as it goes.
///
/// The events are the ones the install pipeline emits — `DownloadStarted`,
/// `DownloadProgress`, `DownloadCompleted` — all carrying
/// [`SELF_UPDATE_PROGRESS_ID`], and they arrive on the calling thread. A UI
/// that wants to show a progress bar therefore forwards them to its own
/// thread exactly as it does for a package install.
pub fn stage_self_update_with_progress(
    platform: Platform,
    manifest_url: &str,
    staging_dir: &Path,
    progress: &ProgressReporter,
) -> Result<SelfUpdateStageReport> {
    let report = check_self_update(platform, manifest_url)?;
    stage_self_update_from_report_with_progress(&report, staging_dir, progress)
}

pub fn relaunch_current_executable() -> Result<u32> {
    let exe = env::current_exe().map_err(|source| RabbitError::Io {
        path: PathBuf::from("current_exe"),
        source,
    })?;
    let child = std::process::Command::new(&exe)
        .spawn()
        .map_err(|source| RabbitError::Io { path: exe, source })?;
    Ok(child.id())
}

pub fn current_install_root() -> Result<PathBuf> {
    let exe = env::current_exe().map_err(|source| RabbitError::Io {
        path: PathBuf::from("current_exe"),
        source,
    })?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| RabbitError::InvalidPlannedExecution {
            message: format!(
                "current executable {} has no parent directory",
                exe.display()
            ),
        })
}

pub fn apply_self_update(
    stage: &SelfUpdateStageReport,
    options: &ApplySelfUpdateOptions,
) -> Result<SelfUpdateApplyReport> {
    if !stage.ready_to_apply {
        return Err(RabbitError::InvalidPlannedExecution {
            message: format!(
                "self-update is not ready to apply: {}",
                stage.status_message
            ),
        });
    }

    // (Old behavior: refuse to apply while *any* package install was
    // running, via a global LocalAppData lock. The lock is now per-target
    // — RABBIT doesn't have a single resource path to ask about during
    // self-update — so the cross-target check is gone. Two concurrent
    // self-updates would race the file rename below, which is rare
    // enough that we let it surface as a normal IO error rather than
    // adding a separate global mutex.)

    let staged_asset =
        stage
            .staged_asset_path
            .as_ref()
            .ok_or_else(|| RabbitError::InvalidPlannedExecution {
                message: "self-update apply requires a staged asset path".to_string(),
            })?;

    let observed_sha256 = sha256_file(staged_asset)?;
    if observed_sha256 != stage.check.asset.sha256 {
        return Err(RabbitError::HashMismatch {
            path: staged_asset.clone(),
            expected: stage.check.asset.sha256.clone(),
            actual: observed_sha256,
        });
    }

    let install_target = resolve_install_target(options)?;
    let install_root = install_target
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| match options.install_root.clone() {
            Some(root) => root,
            None => PathBuf::new(),
        });

    // Everything above is verification; from here on files move. The
    // install phase is a single opaque step to a progress bar — a bundle
    // swap has no meaningful sub-steps to count — so it is bracketed as one
    // started/completed pair.
    if let Some(progress) = options.progress.as_ref() {
        progress.report(ProgressEvent::InstallStarted {
            package_id: SELF_UPDATE_PROGRESS_ID.to_string(),
        });
    }

    let mut replaced = Vec::new();
    let mut skipped = Vec::new();
    let mut signature_verdicts = Vec::new();
    match stage.check.asset.kind {
        SelfUpdateAssetKind::Binary => {
            // The release pipeline publishes the bare RABBIT executable as a
            // single-file asset, so the staged file *is* the new binary — no
            // zip flat-extract step. The download's filename may differ from
            // the install target (e.g. `rabbit-0.2.0-windows-x86_64.exe`
            // vs. `RABBIT.exe`); the swap copies bytes regardless of either
            // name.
            if let Some(record) = verify_replacement_signature(staged_asset, &install_target)? {
                signature_verdicts.push(record);
            }
            if let Err(error) =
                swap_install_file(staged_asset, &install_target, &mut replaced, &mut skipped)
            {
                rollback_replaced_files(&replaced);
                return Err(error);
            }
            // Best-effort: re-seal the surrounding `.app` bundle on macOS so
            // the bundle's signature matches the just-swapped binary. No-op
            // on every other platform, and on macOS for standalone-CLI
            // installs that don't live inside an `.app`.
            resign_macos_bundle_if_applicable(&install_target);
            // …and lift the whole bundle out of quarantine. The re-sign
            // above is ad-hoc (users don't have the Developer ID key), and
            // Gatekeeper refuses a quarantined app whose signature is no
            // longer the notarized one it originally approved — "RABBIT
            // could not be opened" on the next Finder launch, even though
            // the in-place relaunch right after the update works (direct
            // spawn, no Gatekeeper assessment). With the quarantine
            // attribute gone, Gatekeeper no longer demands notarization and
            // the valid ad-hoc signature is sufficient. The replacement
            // bytes were verified against the manifest's sha256 before the
            // swap, so this doesn't bypass any integrity check RABBIT
            // relies on.
            dequarantine_macos_bundle_if_applicable(&install_target);
        }
        SelfUpdateAssetKind::MacAppBundle => {
            // The staged asset is the zipped, notarized `Rabbit.app`.
            // Replacing the whole bundle keeps the Developer ID signature
            // and stapled notarization intact, so no re-sign and no
            // quarantine games are needed — Gatekeeper sees exactly what
            // Apple notarized, and permission grants keyed to the code
            // signature survive the update.
            apply_macos_app_bundle_update(
                staged_asset,
                &install_target,
                &mut replaced,
                &mut skipped,
                &mut signature_verdicts,
            )?;
        }
    }

    let signed_count = signature_verdicts
        .iter()
        .filter(|record| matches!(record.verdict, SignatureVerdict::Signed { .. }))
        .count();
    let status_message = if replaced.is_empty() {
        "Self-update did not match any binary in the install directory.".to_string()
    } else if signed_count > 0 {
        format!(
            "Replaced {} file(s) with RABBIT {} ({} signed); rollback copies retained as .{}.",
            replaced.len(),
            stage.check.latest_version,
            signed_count,
            ROLLBACK_SUFFIX
        )
    } else {
        format!(
            "Replaced {} file(s) with RABBIT {}; rollback copies retained as .{}.",
            replaced.len(),
            stage.check.latest_version,
            ROLLBACK_SUFFIX
        )
    };

    // Only on the success path: an error return short-circuits without a
    // completion event, matching how the install pipeline treats failures.
    if let Some(progress) = options.progress.as_ref() {
        progress.report(ProgressEvent::InstallCompleted {
            package_id: SELF_UPDATE_PROGRESS_ID.to_string(),
        });
    }

    Ok(SelfUpdateApplyReport {
        stage: stage.clone(),
        install_root,
        replaced_files: replaced,
        skipped_files: skipped,
        signature_verdicts,
        status_message,
    })
}

fn resolve_install_target(options: &ApplySelfUpdateOptions) -> Result<PathBuf> {
    if options.install_root.is_none() && options.install_target_basename.is_none() {
        return env::current_exe().map_err(|source| RabbitError::Io {
            path: PathBuf::from("current_exe"),
            source,
        });
    }
    let root = match &options.install_root {
        Some(root) => root.clone(),
        None => current_install_root()?,
    };
    let basename = match options.install_target_basename.clone() {
        Some(name) => name,
        None => current_exe_basename()?,
    };
    Ok(root.join(basename))
}

fn current_exe_basename() -> Result<String> {
    let exe = env::current_exe().map_err(|source| RabbitError::Io {
        path: PathBuf::from("current_exe"),
        source,
    })?;
    exe.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .ok_or_else(|| RabbitError::InvalidPlannedExecution {
            message: "current executable has no file name".to_string(),
        })
}

fn verify_replacement_signature(
    source: &Path,
    install_target: &Path,
) -> Result<Option<SignatureVerdictRecord>> {
    if !install_target.is_file() {
        return Ok(None);
    }
    let verdict = verify_executable_signature(source)?;
    if let SignatureVerdict::Invalid { reason } = &verdict {
        return Err(RabbitError::SelfUpdateSignatureInvalid {
            path: source.to_path_buf(),
            reason: reason.clone(),
        });
    }
    Ok(Some(SignatureVerdictRecord {
        source_path: source.to_path_buf(),
        verdict,
    }))
}

fn swap_install_file(
    source: &Path,
    install_target: &Path,
    replaced: &mut Vec<ReplacedFile>,
    skipped: &mut Vec<PathBuf>,
) -> Result<()> {
    if !install_target.is_file() {
        skipped.push(install_target.to_path_buf());
        return Ok(());
    }
    let backup_path = backup_path_for(install_target);
    if backup_path.exists() {
        fs::remove_file(&backup_path).with_path(&backup_path)?;
    }
    fs::rename(install_target, &backup_path).with_path(install_target)?;
    if let Err(error) = fs::copy(source, install_target) {
        let _ = fs::rename(&backup_path, install_target);
        return Err(RabbitError::Io {
            path: install_target.to_path_buf(),
            source: error,
        });
    }
    // The staged source comes off a GitHub release download, which strips
    // the Unix execute bit — `fs::copy` then propagates the resulting
    // 0644 mode onto the install target. macOS Finder labels the file
    // "document" (not "Unix executable") and the bundle refuses to launch,
    // even with a valid ad-hoc signature. Re-assert 0o755 so the swapped
    // binary stays executable. No-op on Windows.
    ensure_install_target_executable(install_target);
    clear_macos_quarantine(install_target);
    replaced.push(ReplacedFile {
        install_path: install_target.to_path_buf(),
        backup_path,
    });
    Ok(())
}

/// Restore the Unix execute bit on the freshly swapped install target.
/// HTTPS downloads don't carry filesystem mode bits, so the staged source
/// arrives as 0644; `fs::copy` mirrors that mode onto the destination.
/// Without `+x`, macOS treats `Rabbit.app/Contents/MacOS/rabbit` as a
/// non-executable "document" and the bundle becomes unlaunchable
/// (issue #5). Best-effort — a failure here doesn't roll back the swap
/// because the user is no worse off than before this fix existed.
#[cfg(unix)]
fn ensure_install_target_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn ensure_install_target_executable(_path: &Path) {}

/// Strip the `com.apple.quarantine` extended attribute from the freshly
/// swapped binary. Some macOS configurations re-quarantine files written by
/// processes whose own binary still carries the attribute; clearing it here
/// keeps post-update launches from re-triggering Gatekeeper. Failure is
/// ignored — the attribute may simply not be present.
#[cfg(target_os = "macos")]
fn clear_macos_quarantine(path: &Path) {
    let _ = std::process::Command::new("xattr")
        .arg("-d")
        .arg("com.apple.quarantine")
        .arg(path)
        .status();
}

#[cfg(not(target_os = "macos"))]
fn clear_macos_quarantine(_path: &Path) {}

/// macOS only: when the install target lives inside a `.app` bundle, re-sign
/// the bundle ad-hoc so its `_CodeSignature/CodeResources` and bundle-level
/// signature seal match the just-swapped binary. Without this step,
/// `codesign --verify Rabbit.app` reports the bundle as corrupt because
/// the binary's hash differs from the value sealed at bundle-build time,
/// and Gatekeeper refuses to launch it from Finder on macOS 15 (Sequoia)
/// and 26 (Tahoe).
///
/// Best-effort: failures (`/usr/bin/codesign` missing, malformed bundle,
/// permissions on a system-protected install location) are logged to
/// stderr but don't fail the apply, since the binary swap itself
/// succeeded — the user falls back to the manual "Open Anyway" flow on
/// next Finder launch, which is no worse than a fresh download.
///
/// Standalone-CLI installs without an `.app` ancestor skip codesign
/// entirely; the bare-binary release artifact is ad-hoc signed in the
/// release pipeline before publication, so there's no further work.
#[cfg(target_os = "macos")]
fn resign_macos_bundle_if_applicable(install_target: &Path) {
    let Some(bundle) = enclosing_app_bundle(install_target) else {
        return;
    };
    let output = std::process::Command::new("/usr/bin/codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(&bundle)
        .output();
    match output {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "warning: ad-hoc re-sign of {} failed (exit {}): {}",
                bundle.display(),
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }
        Err(error) => {
            eprintln!(
                "warning: could not run codesign to re-sign {}: {}",
                bundle.display(),
                error
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn resign_macos_bundle_if_applicable(_install_target: &Path) {}

/// macOS only: strip `com.apple.quarantine` from the ENTIRE `.app` bundle
/// after the post-swap ad-hoc re-sign. Gatekeeper's original approval was
/// recorded against the notarized Developer ID signature the bundle shipped
/// with; the self-update swap + ad-hoc re-sign changes the code hash, so a
/// still-quarantined bundle is re-assessed on the next LaunchServices
/// launch (Finder, Dock, Spotlight) and rejected — ad-hoc isn't notarized.
/// Removing the quarantine attribute takes the bundle out of Gatekeeper's
/// scope entirely: launches then only require the valid (ad-hoc) code
/// signature the re-sign just produced.
///
/// Quarantine lives on every file the original download carried, not just
/// the main binary — hence `-r` over the bundle root, not the single-file
/// strip `swap_install_file` already does. Best-effort like the re-sign:
/// on failure the user is no worse off than before this fix.
#[cfg(target_os = "macos")]
fn dequarantine_macos_bundle_if_applicable(install_target: &Path) {
    let Some(bundle) = enclosing_app_bundle(install_target) else {
        return;
    };
    let output = std::process::Command::new("/usr/bin/xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(&bundle)
        .output();
    match output {
        // `xattr -d` also exits non-zero when the attribute simply isn't
        // present — that's the healthy case, not a failure worth warning
        // about, so only surface genuinely unexpected stderr chatter.
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            if !stderr.is_empty() && !stderr.contains("No such xattr") {
                eprintln!(
                    "warning: could not clear quarantine on {}: {}",
                    bundle.display(),
                    stderr
                );
            }
        }
        Err(error) => {
            eprintln!(
                "warning: could not run xattr to clear quarantine on {}: {}",
                bundle.display(),
                error
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn dequarantine_macos_bundle_if_applicable(_install_target: &Path) {}

/// Walk up the path looking for an ancestor with a `.app` extension —
/// the macOS bundle root that contains the install target. Returns
/// `None` if the path lives outside any `.app` (e.g., a standalone CLI
/// install in `/usr/local/bin`). Pure path logic, also used on other
/// platforms by the asset selector (where it is trivially `None`).
fn enclosing_app_bundle(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent()?;
    loop {
        if current.extension().and_then(|extension| extension.to_str()) == Some("app") {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

/// Apply a [`SelfUpdateAssetKind::MacAppBundle`] update.
///
/// Normal case — the install target lives inside an `.app` bundle: extract
/// the staged zip next to the installed bundle (same volume, so the swap
/// renames are atomic), sanity-check and signature-check the extracted app,
/// then swap the installed bundle for it, keeping the old bundle as the
/// `.rabbit-old` rollback sibling. The extracted app takes over the
/// installed bundle's path, so a user-renamed `RABBIT.app` keeps its name.
///
/// Cross-install case — the target is NOT inside a bundle (a bundled RABBIT
/// updating a bare-binary install elsewhere, e.g. `--install-root
/// /usr/local/bin`): the bundle's inner binary is extracted and applied
/// through the plain binary swap, preserving the pre-bundle-asset behavior.
#[cfg(target_os = "macos")]
fn apply_macos_app_bundle_update(
    staged_zip: &Path,
    install_target: &Path,
    replaced: &mut Vec<ReplacedFile>,
    skipped: &mut Vec<PathBuf>,
    signature_verdicts: &mut Vec<SignatureVerdictRecord>,
) -> Result<()> {
    let Some(bundle) = enclosing_app_bundle(install_target) else {
        return apply_bundle_inner_binary_to_bare_target(
            staged_zip,
            install_target,
            replaced,
            skipped,
            signature_verdicts,
        );
    };
    let parent = bundle
        .parent()
        .ok_or_else(|| RabbitError::InvalidPlannedExecution {
            message: format!("bundle {} has no parent directory", bundle.display()),
        })?;

    // A crash mid-apply strands the scratch dir (its Drop guard never runs
    // on a kill), and each one holds a full extracted app — reclaim any
    // stale ones from previous attempts before creating this run's.
    sweep_stale_update_scratch_dirs(parent);

    // Extract into a sibling scratch directory: same volume as the bundle,
    // so the swap renames below cannot fail with cross-device errors.
    let extract_dir = parent.join(format!(".rabbit-update-{}", unix_millis()));
    let _extract_guard = RemoveDirOnDrop(extract_dir.clone());
    let extracted_app = extract_staged_app_zip(staged_zip, &extract_dir)?;
    // Structural sanity: the bundle must carry exactly one main executable.
    let _inner_binary = extracted_main_binary(&extracted_app)?;
    // Verify the WHOLE extracted bundle (codesign accepts bundle paths and
    // validates the complete seal — main binary, resources, nested code),
    // which is stronger than checking the inner binary alone. Same policy
    // as the binary path: an Invalid signature aborts, other verdicts are
    // recorded in the report.
    let verdict = verify_executable_signature(&extracted_app)?;
    if let SignatureVerdict::Invalid { reason } = &verdict {
        return Err(RabbitError::SelfUpdateSignatureInvalid {
            path: extracted_app.clone(),
            reason: reason.clone(),
        });
    }
    signature_verdicts.push(SignatureVerdictRecord {
        source_path: extracted_app.clone(),
        verdict,
    });

    let backup_path = backup_path_for(&bundle);
    if backup_path.exists() {
        fs::remove_dir_all(&backup_path).with_path(&backup_path)?;
    }
    // Prefer macOS' atomic directory exchange (`renamex_np` + RENAME_SWAP):
    // the install path holds a complete bundle at every instant, so even a
    // power loss mid-swap can't leave the user without an app. Filesystems
    // without swap support fall back to the portable two-rename dance.
    let backup_path = match atomic_swap_directories(&bundle, &extracted_app) {
        Ok(()) => {
            // The scratch path now holds the OLD bundle; park it in the
            // rollback slot before the scratch guard would delete it. If
            // this fails the NEW app is already installed and healthy, so
            // don't fail the apply — the rollback copy is simply lost.
            if let Err(error) = fs::rename(&extracted_app, &backup_path) {
                eprintln!(
                    "warning: could not keep the previous bundle as {}: {error}",
                    backup_path.display()
                );
            }
            backup_path
        }
        Err(_) => swap_bundle_directories(&bundle, &extracted_app)?,
    };
    replaced.push(ReplacedFile {
        install_path: bundle,
        backup_path,
    });
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn apply_macos_app_bundle_update(
    _staged_zip: &Path,
    _install_target: &Path,
    _replaced: &mut Vec<ReplacedFile>,
    _skipped: &mut Vec<PathBuf>,
    _signature_verdicts: &mut Vec<SignatureVerdictRecord>,
) -> Result<()> {
    Err(RabbitError::InvalidPlannedExecution {
        message: "app-bundle self-update assets only apply on macOS".to_string(),
    })
}

/// Cross-install fallback: the staged asset is the app-bundle zip but the
/// install target is a bare binary outside any `.app`. Extract the bundle's
/// inner binary (Developer-ID signed standalone by the release pipeline)
/// and run the plain binary swap with it.
#[cfg(target_os = "macos")]
fn apply_bundle_inner_binary_to_bare_target(
    staged_zip: &Path,
    install_target: &Path,
    replaced: &mut Vec<ReplacedFile>,
    skipped: &mut Vec<PathBuf>,
    signature_verdicts: &mut Vec<SignatureVerdictRecord>,
) -> Result<()> {
    // Extraction feeds a plain file COPY here (no directory renames), so
    // the staging area's volume doesn't matter — use the zip's own folder.
    let extract_dir = staged_zip
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(env::temp_dir)
        .join(format!(".rabbit-app-extract-{}", unix_millis()));
    let _extract_guard = RemoveDirOnDrop(extract_dir.clone());
    let extracted_app = extract_staged_app_zip(staged_zip, &extract_dir)?;
    let inner_binary = extracted_main_binary(&extracted_app)?;
    let verdict = verify_executable_signature(&inner_binary)?;
    if let SignatureVerdict::Invalid { reason } = &verdict {
        return Err(RabbitError::SelfUpdateSignatureInvalid {
            path: inner_binary.clone(),
            reason: reason.clone(),
        });
    }
    signature_verdicts.push(SignatureVerdictRecord {
        source_path: inner_binary.clone(),
        verdict,
    });
    if let Err(error) = swap_install_file(&inner_binary, install_target, replaced, skipped) {
        rollback_replaced_files(replaced);
        return Err(error);
    }
    Ok(())
}

/// Milliseconds since the Unix epoch, for scratch-directory names.
#[cfg(target_os = "macos")]
fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}

/// Extract the staged app zip into `extract_dir` and return the single
/// `.app` bundle it contains. `ditto -x -k` instead of a Rust zip crate: it
/// preserves the symlinks, permissions, and extended attributes an `.app`
/// bundle relies on, and it is what Apple's own tooling zips bundles with.
#[cfg(target_os = "macos")]
fn extract_staged_app_zip(staged_zip: &Path, extract_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(extract_dir).with_path(extract_dir)?;
    let output = std::process::Command::new("/usr/bin/ditto")
        .arg("-x")
        .arg("-k")
        .arg(staged_zip)
        .arg(extract_dir)
        .output()
        .map_err(|source| RabbitError::Io {
            path: PathBuf::from("/usr/bin/ditto"),
            source,
        })?;
    if !output.status.success() {
        return Err(RabbitError::InvalidPlannedExecution {
            message: format!(
                "extracting the update bundle failed (ditto exit {}): {}",
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    locate_extracted_app(extract_dir)
}

/// Atomically exchange two directories via macOS' `renamex_np(RENAME_SWAP)`.
/// Both paths exist before AND after the call — there is no instant with a
/// missing bundle. Errors (e.g. a filesystem without swap support) make the
/// caller fall back to sequential renames.
#[cfg(target_os = "macos")]
fn atomic_swap_directories(first: &Path, second: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let first = std::ffi::CString::new(first.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let second = std::ffi::CString::new(second.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let status = unsafe { libc::renamex_np(first.as_ptr(), second.as_ptr(), libc::RENAME_SWAP) };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Remove leftover `.rabbit-update-*` scratch directories from previous
/// interrupted applies (each holds a full extracted app copy). They are
/// exclusively RABBIT's own, hidden, and reproducible, so sweeping is safe.
/// Best-effort — a locked entry just stays for the next sweep.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn sweep_stale_update_scratch_dirs(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_scratch = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".rabbit-update-"));
        if is_scratch && path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        }
    }
}

/// Best-effort scratch-directory cleanup on scope exit.
#[cfg(target_os = "macos")]
struct RemoveDirOnDrop(PathBuf);

#[cfg(target_os = "macos")]
impl Drop for RemoveDirOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The single `.app` directory inside the extraction scratch dir. The
/// signed release zip contains `Rabbit.app` at its root; anything else
/// (several apps, none, the unsigned fork layout with a wrapper folder) is
/// rejected rather than guessed at.
#[cfg(any(target_os = "macos", test))]
fn locate_extracted_app(extract_dir: &Path) -> Result<PathBuf> {
    let mut apps = Vec::new();
    for entry in fs::read_dir(extract_dir).with_path(extract_dir)? {
        let entry = entry.with_path(extract_dir)?;
        let path = entry.path();
        if path.is_dir()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        {
            apps.push(path);
        }
    }
    match apps.len() {
        1 => Ok(apps.remove(0)),
        0 => Err(RabbitError::InvalidPlannedExecution {
            message: format!(
                "the update zip did not contain an .app bundle at its root ({})",
                extract_dir.display()
            ),
        }),
        _ => Err(RabbitError::InvalidPlannedExecution {
            message: format!(
                "the update zip contained more than one .app bundle ({})",
                extract_dir.display()
            ),
        }),
    }
}

/// The extracted bundle's main executable: `Contents/MacOS/<CFBundleExecutable>`
/// is overkill to parse here — the pipeline's bundle has exactly one file in
/// `Contents/MacOS`, and requiring exactly one keeps a malformed zip from
/// slipping through.
#[cfg(any(target_os = "macos", test))]
fn extracted_main_binary(app: &Path) -> Result<PathBuf> {
    let macos_dir = app.join("Contents").join("MacOS");
    let mut binaries = Vec::new();
    for entry in fs::read_dir(&macos_dir).with_path(&macos_dir)? {
        let entry = entry.with_path(&macos_dir)?;
        let path = entry.path();
        if path.is_file() {
            binaries.push(path);
        }
    }
    match binaries.len() {
        1 => Ok(binaries.remove(0)),
        count => Err(RabbitError::InvalidPlannedExecution {
            message: format!(
                "expected exactly one executable in {} but found {count}",
                macos_dir.display()
            ),
        }),
    }
}

/// Swap the installed bundle directory for the freshly extracted one:
/// installed → `<name>.rabbit-old` (replacing any previous rollback copy),
/// extracted → the installed path. Rolls the first rename back if the
/// second fails, so the install is never left without a bundle. Returns
/// the rollback path. Pure directory renames — split out so the swap
/// semantics are unit-testable on every platform.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn swap_bundle_directories(installed_bundle: &Path, extracted_app: &Path) -> Result<PathBuf> {
    let backup_path = backup_path_for(installed_bundle);
    if backup_path.exists() {
        fs::remove_dir_all(&backup_path).with_path(&backup_path)?;
    }
    fs::rename(installed_bundle, &backup_path).with_path(installed_bundle)?;
    if let Err(source) = fs::rename(extracted_app, installed_bundle) {
        if fs::rename(&backup_path, installed_bundle).is_err() {
            // Both the swap AND the restore failed: tell the user exactly
            // where their previous app still lives instead of leaving an
            // empty install path with a generic I/O error.
            return Err(RabbitError::InvalidPlannedExecution {
                message: format!(
                    "installing the new bundle at {} failed ({source}) and restoring the previous one also failed; the previous app is preserved at {}",
                    installed_bundle.display(),
                    backup_path.display()
                ),
            });
        }
        return Err(RabbitError::Io {
            path: installed_bundle.to_path_buf(),
            source,
        });
    }
    Ok(backup_path)
}

fn rollback_replaced_files(replaced: &[ReplacedFile]) {
    for entry in replaced.iter().rev() {
        let _ = fs::remove_file(&entry.install_path);
        let _ = fs::rename(&entry.backup_path, &entry.install_path);
    }
}

fn backup_path_for(install_path: &Path) -> PathBuf {
    let file_name = install_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("rabbit-target");
    install_path.with_file_name(format!("{file_name}.{ROLLBACK_SUFFIX}"))
}

fn evaluate_self_update_report(
    platform: Platform,
    architecture: Architecture,
    manifest_url: &str,
    current_version: Version,
    manifest: &SelfUpdateManifest,
) -> Result<SelfUpdateCheckReport> {
    let current_semver =
        semantic_version_from_version(&current_version, manifest_url, "current_version")?;
    let latest_semver = semantic_version_from_version(&manifest.version, manifest_url, "version")?;
    let minimum_supported_previous_version = manifest.minimum_supported_previous_version.clone();
    let requires_manual_transition = minimum_supported_previous_version
        .as_ref()
        .map(|minimum| {
            semantic_version_from_version(
                minimum,
                manifest_url,
                "minimum_supported_previous_version",
            )
            .map(|minimum| current_semver < minimum)
        })
        .transpose()?
        .unwrap_or(false);

    Ok(SelfUpdateCheckReport {
        manifest_url: manifest_url.to_string(),
        current_version,
        latest_version: manifest.version.clone(),
        channel: manifest.channel.clone(),
        published_at: manifest.published_at.clone(),
        release_notes_url: manifest.release_notes_url.clone(),
        minimum_supported_previous_version,
        update_available: latest_semver > current_semver,
        requires_manual_transition,
        asset: select_asset_for_platform(platform, architecture, manifest, manifest_url)?,
    })
}

/// Staging without progress, kept as the shape every existing caller and
/// test already uses.
/// Progress-free wrapper kept for the tests, which don't exercise the
/// progress reporting; production paths call the `_with_progress` form.
#[cfg(test)]
fn stage_self_update_from_report(
    report: &SelfUpdateCheckReport,
    staging_dir: &Path,
) -> Result<SelfUpdateStageReport> {
    stage_self_update_from_report_with_progress(report, staging_dir, &ProgressReporter::noop())
}

fn stage_self_update_from_report_with_progress(
    report: &SelfUpdateCheckReport,
    staging_dir: &Path,
    progress: &ProgressReporter,
) -> Result<SelfUpdateStageReport> {
    if !report.update_available {
        return Ok(SelfUpdateStageReport {
            check: report.clone(),
            staging_dir: staging_dir.to_path_buf(),
            staged_asset_path: None,
            downloaded: false,
            reused_existing_file: false,
            verified_sha256: None,
            ready_to_apply: false,
            status_message: "Current RABBIT version is already up to date.".to_string(),
        });
    }

    if report.requires_manual_transition {
        return Ok(SelfUpdateStageReport {
            check: report.clone(),
            staging_dir: staging_dir.to_path_buf(),
            staged_asset_path: None,
            downloaded: false,
            reused_existing_file: false,
            verified_sha256: None,
            ready_to_apply: false,
            status_message:
                "This RABBIT update requires a manual transition before staging can continue."
                    .to_string(),
        });
    }

    let (file_name, local_source_path) = resolve_update_asset_source(&report.asset.url)?;
    let version_dir = staging_dir.join(report.latest_version.raw());
    fs::create_dir_all(&version_dir).with_path(&version_dir)?;

    let target_path = version_dir.join(file_name);
    if target_path.is_file() {
        let existing_sha256 = sha256_file(&target_path)?;
        if existing_sha256 == report.asset.sha256 {
            // Already staged by an earlier run: report the pair anyway so a
            // UI sees a download phase open and close instead of jumping
            // straight to installing with an empty bar behind it.
            progress.report(ProgressEvent::DownloadStarted {
                package_id: SELF_UPDATE_PROGRESS_ID.to_string(),
                bytes_total: None,
            });
            progress.report(ProgressEvent::DownloadCompleted {
                package_id: SELF_UPDATE_PROGRESS_ID.to_string(),
            });
            return Ok(SelfUpdateStageReport {
                check: report.clone(),
                staging_dir: staging_dir.to_path_buf(),
                staged_asset_path: Some(target_path),
                downloaded: false,
                reused_existing_file: true,
                verified_sha256: Some(existing_sha256),
                ready_to_apply: true,
                status_message: format!(
                    "Verified existing staged RABBIT update {}.",
                    report.latest_version
                ),
            });
        }

        fs::remove_file(&target_path).with_path(&target_path)?;
    }

    download_self_update_asset(
        &report.asset.url,
        local_source_path.as_deref(),
        &target_path,
        progress,
    )?;
    // The checksum runs over ~10 MB and is not instant on a slow disk, so
    // the download is only reported complete once the file is known good —
    // a UI that hides its progress bar on `DownloadCompleted` would
    // otherwise sit blank through the verification.
    let verified_sha256 = sha256_file(&target_path)?;
    if verified_sha256 != report.asset.sha256 {
        let _ = fs::remove_file(&target_path);
        return Err(RabbitError::HashMismatch {
            path: target_path,
            expected: report.asset.sha256.clone(),
            actual: verified_sha256,
        });
    }

    progress.report(ProgressEvent::DownloadCompleted {
        package_id: SELF_UPDATE_PROGRESS_ID.to_string(),
    });

    Ok(SelfUpdateStageReport {
        check: report.clone(),
        staging_dir: staging_dir.to_path_buf(),
        staged_asset_path: Some(target_path),
        downloaded: true,
        reused_existing_file: false,
        verified_sha256: Some(report.asset.sha256.clone()),
        ready_to_apply: true,
        status_message: format!(
            "Downloaded and verified staged RABBIT update {}.",
            report.latest_version
        ),
    })
}

fn select_asset_for_platform(
    platform: Platform,
    architecture: Architecture,
    manifest: &SelfUpdateManifest,
    manifest_url: &str,
) -> Result<SelfUpdateAssetSelection> {
    let in_app_bundle = env::current_exe()
        .ok()
        .is_some_and(|exe| enclosing_app_bundle(&exe).is_some());
    select_asset_for_platform_with_context(
        platform,
        architecture,
        manifest,
        manifest_url,
        in_app_bundle,
    )
}

/// [`select_asset_for_platform`] with the "are we running inside a macOS
/// `.app` bundle?" fact injected, so tests can pin both selections.
fn select_asset_for_platform_with_context(
    platform: Platform,
    architecture: Architecture,
    manifest: &SelfUpdateManifest,
    manifest_url: &str,
    in_app_bundle: bool,
) -> Result<SelfUpdateAssetSelection> {
    // A macOS install living inside an `.app` prefers the full bundle
    // asset: swapping the whole bundle preserves the Developer ID
    // signature and notarization, where a binary swap would force an
    // ad-hoc downgrade. Installs outside a bundle (bare CLI) and old
    // manifests without the asset keep the binary path.
    if platform == Platform::MacOs
        && in_app_bundle
        && let Some(app_asset) = &manifest.assets.macos_app
    {
        return Ok(SelfUpdateAssetSelection {
            platform,
            url: app_asset.url.clone(),
            sha256: app_asset.sha256.clone(),
            kind: SelfUpdateAssetKind::MacAppBundle,
        });
    }

    // Prefer the per-arch `platforms` table when the manifest carries one.
    // Its presence means the publisher has explicitly enumerated which
    // (platform, arch) combinations are supported, so a missing entry is
    // an authoritative "no asset for this arch" rather than a reason to
    // fall back to a possibly-wrong-arch legacy field.
    if let Some(platforms) = &manifest.assets.platforms {
        let arch_token =
            architecture
                .release_artifact_token()
                .ok_or_else(|| RabbitError::RemoteData {
                    url: manifest_url.to_string(),
                    message: format!(
                        "no manifest asset for {platform:?} on architecture {architecture:?}: \
                     architecture is not produced by the RABBIT release pipeline."
                    ),
                })?;
        let key = format!("{}-{}", platform_token(platform), arch_token);
        let asset = platforms.get(&key).ok_or_else(|| RabbitError::RemoteData {
            url: manifest_url.to_string(),
            message: format!(
                "manifest does not list a {key} asset; \
                 download the matching build from the GitHub releases page manually."
            ),
        })?;
        return Ok(SelfUpdateAssetSelection {
            platform,
            url: asset.url.clone(),
            sha256: asset.sha256.clone(),
            kind: SelfUpdateAssetKind::Binary,
        });
    }

    // Legacy schema fallback: platform-level slot only, arch implicit. The
    // safety net below catches the case where a RABBIT instance running on
    // a non-default arch (Windows ARM, Intel Mac) would otherwise overwrite
    // its native binary with one for the wrong CPU.
    let asset = match platform {
        Platform::Windows => manifest.assets.windows.as_ref(),
        Platform::MacOs => manifest.assets.macos.as_ref(),
    }
    .ok_or_else(|| RabbitError::RemoteData {
        url: manifest_url.to_string(),
        message: format!("missing asset entry for platform {platform:?}"),
    })?;

    if let (Some(expected), Some(actual)) = (
        architecture.release_artifact_token(),
        arch_token_from_asset_url(&asset.url),
    ) && expected != actual
    {
        return Err(RabbitError::RemoteData {
            url: manifest_url.to_string(),
            message: format!(
                "self-update asset is built for {actual} but RABBIT is running on {expected}; \
                 refusing to overwrite this binary with one for the wrong architecture. \
                 Download the matching build from the GitHub releases page manually."
            ),
        });
    }

    Ok(SelfUpdateAssetSelection {
        platform,
        url: asset.url.clone(),
        sha256: asset.sha256.clone(),
        kind: SelfUpdateAssetKind::Binary,
    })
}

fn platform_token(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "windows",
        Platform::MacOs => "macos",
    }
}

/// Validates that a `platforms` map key has the canonical `<os>-<arch>`
/// shape with a known platform token and a release-pipeline arch token.
/// Unknown shapes are rejected at parse time so downstream lookup logic
/// can stay simple.
fn validate_platform_key(key: &str, manifest_url: &str) -> Result<()> {
    let (os, arch) = key.split_once('-').ok_or_else(|| RabbitError::RemoteData {
        url: manifest_url.to_string(),
        message: format!("manifest platforms key '{key}' must be '<os>-<arch>'"),
    })?;
    let os_ok = matches!(os, "windows" | "macos");
    let arch_ok = matches!(arch, "x86_64" | "aarch64" | "i686" | "armv7");
    if !os_ok || !arch_ok {
        return Err(RabbitError::RemoteData {
            url: manifest_url.to_string(),
            message: format!(
                "manifest platforms key '{key}' uses an unrecognised os or arch token"
            ),
        });
    }
    Ok(())
}

/// Extracts the trailing arch token from a release artifact URL whose filename
/// follows the canonical `rabbit-<version>-<os>-<arch>[.exe]` pattern. Returns
/// `None` for any other shape — non-conforming filenames simply skip the
/// arch-mismatch check rather than tripping false positives.
fn arch_token_from_asset_url(url: &str) -> Option<&str> {
    let basename = url.rsplit_once('/').map(|(_, name)| name).unwrap_or(url);
    let stem = basename.strip_suffix(".exe").unwrap_or(basename);
    let rest = stem.strip_prefix("rabbit-")?;
    let (_, arch) = rest.rsplit_once('-')?;
    match arch {
        "x86_64" | "aarch64" | "i686" | "armv7" => Some(arch),
        _ => None,
    }
}

fn parse_asset(
    asset: &RawSelfUpdateAsset,
    manifest_url: &str,
    field: &str,
) -> Result<SelfUpdateAsset> {
    if !asset.url.starts_with("https://") {
        return Err(RabbitError::RemoteData {
            url: manifest_url.to_string(),
            message: format!("{field} asset url must use https: {}", asset.url),
        });
    }
    if !is_valid_sha256(&asset.sha256) {
        return Err(RabbitError::RemoteData {
            url: manifest_url.to_string(),
            message: format!("{field} asset sha256 must be 64 lowercase hexadecimal characters"),
        });
    }

    Ok(SelfUpdateAsset {
        url: asset.url.clone(),
        sha256: asset.sha256.clone(),
    })
}

fn download_self_update_asset(
    url: &str,
    local_source_path: Option<&Path>,
    target_path: &Path,
    progress: &ProgressReporter,
) -> Result<()> {
    let part_path = target_path.with_extension(format!(
        "{}.part",
        target_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("download")
    ));

    if let Some(source_path) = local_source_path {
        // A local file (test fixture, file:// manifest) has no byte
        // progress to report, but the started/completed pair still has to
        // bracket it: a UI that only shows its progress bar between the two
        // would otherwise never show one at all.
        progress.report(ProgressEvent::DownloadStarted {
            package_id: SELF_UPDATE_PROGRESS_ID.to_string(),
            bytes_total: None,
        });
        fs::copy(source_path, &part_path).with_path(source_path)?;
        fs::rename(&part_path, target_path).with_path(target_path)?;
        return Ok(());
    }

    validate_remote_self_update_url(url)?;
    // Same download engine as package artifacts: stall-tolerant client,
    // retry with resume, and network failures classified as download
    // interruptions instead of I/O errors at the .part path. (The staged
    // file's sha256 is verified against the manifest afterwards, so a
    // resumed download can never swap in a mismatched binary.) It emits
    // `DownloadStarted` and the byte-progress ticks; the completion event
    // is ours to send once the checksum has been verified.
    crate::artifact::download_url_with_retries(url, &part_path, SELF_UPDATE_PROGRESS_ID, progress)?;

    fs::rename(&part_path, target_path).with_path(target_path)?;
    Ok(())
}

fn resolve_update_asset_source(url_or_path: &str) -> Result<(String, Option<PathBuf>)> {
    if let Some(path) = local_update_asset_source_path(url_or_path)? {
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| RabbitError::RemoteData {
                url: url_or_path.to_string(),
                message: "self-update asset path does not contain a file name".to_string(),
            })?;
        return Ok((file_name.to_string(), Some(path)));
    }

    validate_remote_self_update_url(url_or_path)?;
    let file_name = file_name_from_url(url_or_path).ok_or_else(|| RabbitError::RemoteData {
        url: url_or_path.to_string(),
        message: "self-update asset URL does not contain a file name".to_string(),
    })?;
    Ok((file_name, None))
}

fn local_update_asset_source_path(url_or_path: &str) -> Result<Option<PathBuf>> {
    let path = PathBuf::from(url_or_path);
    if path.is_file() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

fn validate_remote_self_update_url(url: &str) -> Result<()> {
    if url.starts_with("https://") {
        Ok(())
    } else {
        Err(RabbitError::InvalidArtifactUrl {
            url: url.to_string(),
            message: "self-update downloads must use HTTPS".to_string(),
        })
    }
}

fn file_name_from_url(url: &str) -> Option<String> {
    let without_query = url.split_once('?').map_or(url, |(path, _query)| path);
    without_query
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
}

fn parse_semantic_version(raw: &str, url: &str, field: &str) -> Result<Version> {
    semantic_version_from_str(raw, url, field)?;
    Version::parse(raw)
}

fn semantic_version_from_version(
    version: &Version,
    url: &str,
    field: &str,
) -> Result<SemanticVersion> {
    semantic_version_from_str(version.raw(), url, field)
}

fn semantic_version_from_str(raw: &str, url: &str, field: &str) -> Result<SemanticVersion> {
    let trimmed = raw.trim();
    let parts = trimmed.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: format!("{field} must use semantic versioning (major.minor.patch): {trimmed}"),
        });
    }

    let parse_part = |name: &str, value: &str| {
        value.parse::<u64>().map_err(|_| RabbitError::RemoteData {
            url: url.to_string(),
            message: format!("{field} contains a non-numeric {name} segment: {trimmed}"),
        })
    };

    Ok(SemanticVersion {
        major: parse_part("major", parts[0])?,
        minor: parse_part("minor", parts[1])?,
        patch: parse_part("patch", parts[2])?,
    })
}

fn is_valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        ApplySelfUpdateOptions, DEFAULT_SELF_UPDATE_RELEASE_NOTES_URL, SelfUpdateAssetKind,
        SelfUpdateAssetSelection, SelfUpdateCheckReport, SelfUpdateManifest, SelfUpdateStageReport,
        apply_self_update, arch_token_from_asset_url, current_rabbit_version, enclosing_app_bundle,
        evaluate_self_update_report, extracted_main_binary, locate_extracted_app,
        parse_self_update_manifest, resolve_self_update_release_notes,
        select_asset_for_platform_with_context, stage_self_update_from_report,
        swap_bundle_directories, sweep_stale_update_scratch_dirs,
    };
    use crate::RabbitError;
    use crate::hash::sha256_file;
    use crate::model::{Architecture, Platform};
    use crate::version::Version;
    use std::path::{Path, PathBuf};

    const MANIFEST_URL: &str = "https://example.test/rabbit-update-stable.json";

    #[test]
    fn parses_valid_self_update_manifest() {
        let manifest = parse_self_update_manifest(
            r#"{
              "version": "0.2.0",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "release_notes_url": "https://example.test/releases/v0.2.0",
              "minimum_supported_previous_version": "0.1.0",
              "assets": {
                "windows": {
                  "url": "https://example.test/RABBIT-windows.zip",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "macos": {
                  "url": "https://example.test/RABBIT-macos.zip",
                  "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap();

        assert_eq!(manifest.version.raw(), "0.2.0");
        assert_eq!(manifest.channel, "stable");
        assert_eq!(
            manifest
                .minimum_supported_previous_version
                .as_ref()
                .unwrap()
                .raw(),
            "0.1.0"
        );
    }

    #[test]
    fn parses_manifest_with_macos_app_bundle_asset() {
        let manifest = parse_self_update_manifest(
            r#"{
              "version": "0.4.0",
              "channel": "stable",
              "published_at": "2026-07-14T00:00:00Z",
              "release_notes_url": null,
              "minimum_supported_previous_version": null,
              "assets": {
                "windows": {
                  "url": "https://example.test/rabbit-0.4.0-windows-x86_64.exe",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "macos": {
                  "url": "https://example.test/rabbit-0.4.0-macos-universal",
                  "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                },
                "macos_app": {
                  "url": "https://example.test/rabbit-0.4.0-macos-universal.app.zip",
                  "sha256": "1111111111111111111111111111111111111111111111111111111111111111"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap();

        let app = manifest.assets.macos_app.as_ref().unwrap();
        assert!(app.url.ends_with(".app.zip"));
        // Manifests without the field (every release up to 0.3.x) parse to None.
        let legacy = parse_self_update_manifest(
            r#"{
              "version": "0.3.2",
              "channel": "stable",
              "published_at": "2026-07-03T00:00:00Z",
              "assets": {
                "windows": {
                  "url": "https://example.test/rabbit-0.3.2-windows-x86_64.exe",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "macos": {
                  "url": "https://example.test/rabbit-0.3.2-macos-universal",
                  "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap();
        assert!(legacy.assets.macos_app.is_none());
    }

    fn manifest_with_macos_app() -> SelfUpdateManifest {
        parse_self_update_manifest(
            r#"{
              "version": "0.4.0",
              "channel": "stable",
              "published_at": "2026-07-14T00:00:00Z",
              "assets": {
                "windows": {
                  "url": "https://example.test/rabbit-0.4.0-windows-x86_64.exe",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "macos": {
                  "url": "https://example.test/rabbit-0.4.0-macos-universal",
                  "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                },
                "platforms": {
                  "windows-x86_64": {
                    "url": "https://example.test/rabbit-0.4.0-windows-x86_64.exe",
                    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                  },
                  "macos-aarch64": {
                    "url": "https://example.test/rabbit-0.4.0-macos-universal",
                    "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                  }
                },
                "macos_app": {
                  "url": "https://example.test/rabbit-0.4.0-macos-universal.app.zip",
                  "sha256": "1111111111111111111111111111111111111111111111111111111111111111"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap()
    }

    #[test]
    fn selects_app_bundle_asset_only_for_macos_installs_inside_a_bundle() {
        let manifest = manifest_with_macos_app();

        // macOS + running inside an .app -> the bundle asset.
        let selection = select_asset_for_platform_with_context(
            Platform::MacOs,
            Architecture::Arm64,
            &manifest,
            MANIFEST_URL,
            true,
        )
        .unwrap();
        assert_eq!(selection.kind, SelfUpdateAssetKind::MacAppBundle);
        assert!(selection.url.ends_with(".app.zip"));

        // macOS outside a bundle (bare CLI install) -> the bare binary.
        let selection = select_asset_for_platform_with_context(
            Platform::MacOs,
            Architecture::Arm64,
            &manifest,
            MANIFEST_URL,
            false,
        )
        .unwrap();
        assert_eq!(selection.kind, SelfUpdateAssetKind::Binary);
        assert!(!selection.url.ends_with(".app.zip"));

        // Windows never selects the macOS bundle even when present.
        let selection = select_asset_for_platform_with_context(
            Platform::Windows,
            Architecture::X64,
            &manifest,
            MANIFEST_URL,
            false,
        )
        .unwrap();
        assert_eq!(selection.kind, SelfUpdateAssetKind::Binary);
        assert!(selection.url.contains("windows"));
    }

    #[test]
    fn swap_bundle_directories_swaps_and_keeps_rollback() {
        let dir = tempdir().unwrap();
        let installed = dir.path().join("RABBIT.app");
        std::fs::create_dir_all(installed.join("Contents").join("MacOS")).unwrap();
        std::fs::write(installed.join("Contents").join("old-marker"), b"old").unwrap();
        let extracted = dir.path().join("scratch").join("Rabbit.app");
        std::fs::create_dir_all(extracted.join("Contents").join("MacOS")).unwrap();
        std::fs::write(extracted.join("Contents").join("new-marker"), b"new").unwrap();
        // A stale rollback copy from a previous update must be replaced.
        let stale_backup = dir.path().join("RABBIT.app.rabbit-old");
        std::fs::create_dir_all(&stale_backup).unwrap();
        std::fs::write(stale_backup.join("stale"), b"stale").unwrap();

        let backup = swap_bundle_directories(&installed, &extracted).unwrap();

        assert_eq!(backup, dir.path().join("RABBIT.app.rabbit-old"));
        // Installed path now holds the NEW bundle (renamed to keep the
        // installed bundle name), backup holds the OLD one; stale gone.
        assert!(installed.join("Contents").join("new-marker").is_file());
        assert!(backup.join("Contents").join("old-marker").is_file());
        assert!(!backup.join("stale").exists());
        assert!(!extracted.exists());
    }

    #[test]
    fn sweeps_only_stale_update_scratch_dirs() {
        let dir = tempdir().unwrap();
        let stale_a = dir.path().join(".rabbit-update-1000");
        let stale_b = dir.path().join(".rabbit-update-2000");
        std::fs::create_dir_all(stale_a.join("Rabbit.app")).unwrap();
        std::fs::create_dir_all(&stale_b).unwrap();
        let unrelated_dir = dir.path().join("RABBIT.app");
        std::fs::create_dir_all(&unrelated_dir).unwrap();
        let unrelated_file = dir.path().join(".rabbit-update-notes.txt");
        std::fs::write(&unrelated_file, b"keep").unwrap();

        sweep_stale_update_scratch_dirs(dir.path());

        assert!(!stale_a.exists());
        assert!(!stale_b.exists());
        assert!(unrelated_dir.exists());
        // Only DIRECTORIES with the scratch prefix are swept.
        assert!(unrelated_file.exists());
    }

    #[test]
    fn locate_extracted_app_requires_exactly_one_bundle() {
        let dir = tempdir().unwrap();
        // None -> error.
        assert!(locate_extracted_app(dir.path()).is_err());
        // Exactly one -> that bundle.
        let app = dir.path().join("Rabbit.app");
        std::fs::create_dir_all(&app).unwrap();
        assert_eq!(locate_extracted_app(dir.path()).unwrap(), app);
        // A second .app (or the unsigned-fork wrapper layout) -> error.
        std::fs::create_dir_all(dir.path().join("Other.app")).unwrap();
        assert!(locate_extracted_app(dir.path()).is_err());
    }

    #[test]
    fn extracted_main_binary_requires_exactly_one_executable() {
        let dir = tempdir().unwrap();
        let app = dir.path().join("Rabbit.app");
        let macos_dir = app.join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos_dir).unwrap();
        assert!(extracted_main_binary(&app).is_err());
        std::fs::write(macos_dir.join("rabbit"), b"binary").unwrap();
        assert_eq!(
            extracted_main_binary(&app).unwrap(),
            macos_dir.join("rabbit")
        );
        std::fs::write(macos_dir.join("second"), b"binary").unwrap();
        assert!(extracted_main_binary(&app).is_err());
    }

    #[test]
    fn rejects_non_semantic_manifest_version() {
        let error = parse_self_update_manifest(
            r#"{
              "version": "0.2",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "assets": {
                "windows": {
                  "url": "https://example.test/RABBIT-windows.zip",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap_err();

        assert!(error.to_string().contains("semantic versioning"));
    }

    #[test]
    fn rejects_non_https_asset_url() {
        let error = parse_self_update_manifest(
            r#"{
              "version": "0.2.0",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "assets": {
                "windows": {
                  "url": "http://example.test/RABBIT-windows.zip",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap_err();

        assert!(error.to_string().contains("must use https"));
    }

    #[test]
    fn reports_update_available_for_newer_version() {
        let manifest = sample_manifest();

        let report = evaluate_self_update_report(
            Platform::Windows,
            Architecture::X64,
            MANIFEST_URL,
            Version::parse("0.1.0").unwrap(),
            &manifest,
        )
        .unwrap();

        assert!(report.update_available);
        assert!(!report.requires_manual_transition);
        assert_eq!(report.asset.platform, Platform::Windows);
        assert!(report.asset.url.contains("RABBIT-windows.zip"));
    }

    #[test]
    fn reports_manual_transition_requirement() {
        let manifest = sample_manifest();

        let report = evaluate_self_update_report(
            Platform::Windows,
            Architecture::X64,
            MANIFEST_URL,
            Version::parse("0.0.9").unwrap(),
            &manifest,
        )
        .unwrap();

        assert!(report.update_available);
        assert!(report.requires_manual_transition);
    }

    #[test]
    fn release_notes_are_skipped_when_no_update_is_available() {
        // The `update_available` guard is what keeps an up-to-date RABBIT
        // from spending a GitHub API request on notes nobody will see — and
        // it is what keeps this test offline, since returning early is the
        // only path through `resolve_self_update_release_notes` that never
        // reaches the network.
        let manifest = sample_manifest();
        let report = evaluate_self_update_report(
            Platform::Windows,
            Architecture::X64,
            MANIFEST_URL,
            manifest.version.clone(),
            &manifest,
        )
        .unwrap();

        assert!(!report.update_available);
        assert_eq!(resolve_self_update_release_notes(&report), None);
    }

    #[test]
    fn release_notes_url_points_at_the_github_release_listing() {
        // A single-release endpoint (`/releases/latest`) would silently
        // reduce the notes to the newest version only, defeating the whole
        // point of spanning everything the user skipped. The renderer
        // accepts either shape, so nothing else would catch the mistake.
        assert!(DEFAULT_SELF_UPDATE_RELEASE_NOTES_URL.starts_with("https://api.github.com/"));
        assert!(DEFAULT_SELF_UPDATE_RELEASE_NOTES_URL.contains("/repos/Timtam/rabbit/releases"));
        assert!(!DEFAULT_SELF_UPDATE_RELEASE_NOTES_URL.contains("/releases/latest"));
    }

    #[test]
    fn arch_token_parser_extracts_known_archs() {
        assert_eq!(
            arch_token_from_asset_url("https://example.test/rabbit-0.2.0-windows-x86_64.exe"),
            Some("x86_64")
        );
        assert_eq!(
            arch_token_from_asset_url("https://example.test/rabbit-0.2.0-macos-aarch64"),
            Some("aarch64")
        );
        // Non-conforming filenames produce None so the safety net stays
        // off rather than tripping false positives on legacy / synthetic URLs.
        assert_eq!(
            arch_token_from_asset_url("https://example.test/RABBIT-windows.zip"),
            None
        );
        assert_eq!(
            arch_token_from_asset_url("https://example.test/rabbit-0.2.0-linux-riscv64"),
            None
        );
    }

    #[test]
    fn refuses_self_update_when_asset_arch_mismatches_runtime() {
        let manifest = parse_self_update_manifest(
            r#"{
              "version": "0.2.0",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "assets": {
                "windows": {
                  "url": "https://example.test/rabbit-0.2.0-windows-x86_64.exe",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap();

        let error = evaluate_self_update_report(
            Platform::Windows,
            Architecture::Arm64,
            MANIFEST_URL,
            Version::parse("0.1.0").unwrap(),
            &manifest,
        )
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("x86_64"), "message was: {message}");
        assert!(message.contains("aarch64"), "message was: {message}");
    }

    #[test]
    fn allows_self_update_when_asset_arch_matches_runtime() {
        let manifest = parse_self_update_manifest(
            r#"{
              "version": "0.2.0",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "assets": {
                "macos": {
                  "url": "https://example.test/rabbit-0.2.0-macos-aarch64",
                  "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap();

        let report = evaluate_self_update_report(
            Platform::MacOs,
            Architecture::Arm64,
            MANIFEST_URL,
            Version::parse("0.1.0").unwrap(),
            &manifest,
        )
        .unwrap();

        assert!(report.update_available);
        assert!(report.asset.url.ends_with("rabbit-0.2.0-macos-aarch64"));
    }

    #[test]
    fn per_arch_platforms_table_is_authoritative_when_present() {
        let manifest = parse_self_update_manifest(
            r#"{
              "version": "0.2.0",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "assets": {
                "windows": {
                  "url": "https://example.test/rabbit-0.2.0-windows-x86_64.exe",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "macos": {
                  "url": "https://example.test/rabbit-0.2.0-macos-aarch64",
                  "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                },
                "platforms": {
                  "windows-x86_64": {
                    "url": "https://example.test/rabbit-0.2.0-windows-x86_64.exe",
                    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                  },
                  "windows-aarch64": {
                    "url": "https://example.test/rabbit-0.2.0-windows-aarch64.exe",
                    "sha256": "1111111111111111111111111111111111111111111111111111111111111111"
                  },
                  "macos-aarch64": {
                    "url": "https://example.test/rabbit-0.2.0-macos-aarch64",
                    "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                  },
                  "macos-x86_64": {
                    "url": "https://example.test/rabbit-0.2.0-macos-x86_64",
                    "sha256": "2222222222222222222222222222222222222222222222222222222222222222"
                  }
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap();

        // Windows ARM picks the per-arch entry, NOT the legacy x86_64 slot.
        let windows_arm = evaluate_self_update_report(
            Platform::Windows,
            Architecture::Arm64,
            MANIFEST_URL,
            Version::parse("0.1.0").unwrap(),
            &manifest,
        )
        .unwrap();
        assert!(windows_arm.asset.url.ends_with("windows-aarch64.exe"));

        // Intel Mac picks its per-arch entry — under the old schema this
        // would have errored out due to the arch-mismatch safety net.
        let macos_intel = evaluate_self_update_report(
            Platform::MacOs,
            Architecture::X64,
            MANIFEST_URL,
            Version::parse("0.1.0").unwrap(),
            &manifest,
        )
        .unwrap();
        assert!(macos_intel.asset.url.ends_with("macos-x86_64"));
    }

    #[test]
    fn per_arch_platforms_table_errors_for_missing_arch() {
        let manifest = parse_self_update_manifest(
            r#"{
              "version": "0.2.0",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "assets": {
                "windows": {
                  "url": "https://example.test/rabbit-0.2.0-windows-x86_64.exe",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "platforms": {
                  "windows-x86_64": {
                    "url": "https://example.test/rabbit-0.2.0-windows-x86_64.exe",
                    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                  }
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap();

        let error = evaluate_self_update_report(
            Platform::Windows,
            Architecture::Arm64,
            MANIFEST_URL,
            Version::parse("0.1.0").unwrap(),
            &manifest,
        )
        .unwrap_err();
        assert!(error.to_string().contains("windows-aarch64"));
    }

    #[test]
    fn rejects_manifest_with_unknown_platforms_key() {
        let error = parse_self_update_manifest(
            r#"{
              "version": "0.2.0",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "assets": {
                "platforms": {
                  "linux-x86_64": {
                    "url": "https://example.test/rabbit-0.2.0-linux-x86_64",
                    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                  }
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unrecognised"));
    }

    #[test]
    fn current_build_version_is_semantic() {
        let version = current_rabbit_version().unwrap();

        assert_eq!(version.raw(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn stages_update_from_local_asset_and_verifies_hash() {
        let source_dir = tempdir().unwrap();
        let staging_dir = tempdir().unwrap();
        let asset_path = source_dir.path().join("RABBIT-windows.zip");
        fs::write(&asset_path, b"rabbit-update").unwrap();
        let expected_sha256 = sha256_file(&asset_path).unwrap();

        let report = stage_self_update_from_report(
            &sample_check_report(asset_path.display().to_string(), &expected_sha256),
            staging_dir.path(),
        )
        .unwrap();

        assert!(report.downloaded);
        assert!(!report.reused_existing_file);
        assert!(report.ready_to_apply);
        assert_eq!(
            report.staged_asset_path.as_ref().unwrap(),
            &staging_dir.path().join("0.2.0").join("RABBIT-windows.zip")
        );
        assert_eq!(
            report.verified_sha256.as_deref(),
            Some(expected_sha256.as_str())
        );
    }

    #[test]
    fn reuses_existing_staged_update_when_hash_matches() {
        let source_dir = tempdir().unwrap();
        let staging_dir = tempdir().unwrap();
        let asset_path = source_dir.path().join("RABBIT-windows.zip");
        fs::write(&asset_path, b"rabbit-update").unwrap();
        let expected_sha256 = sha256_file(&asset_path).unwrap();
        let check = sample_check_report(asset_path.display().to_string(), &expected_sha256);

        let first = stage_self_update_from_report(&check, staging_dir.path()).unwrap();
        let second = stage_self_update_from_report(&check, staging_dir.path()).unwrap();

        assert!(first.downloaded);
        assert!(!first.reused_existing_file);
        assert!(second.reused_existing_file);
        assert!(!second.downloaded);
        assert!(second.ready_to_apply);
    }

    #[test]
    fn does_not_stage_when_current_version_is_already_latest() {
        let staging_dir = tempdir().unwrap();
        let mut check = sample_check_report(
            "https://example.test/RABBIT-windows.zip".to_string(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );
        check.update_available = false;

        let report = stage_self_update_from_report(&check, staging_dir.path()).unwrap();

        assert!(!report.ready_to_apply);
        assert!(report.staged_asset_path.is_none());
        assert!(report.status_message.contains("up to date"));
    }

    #[test]
    fn removes_bad_staged_file_when_hash_mismatch_is_detected() {
        let source_dir = tempdir().unwrap();
        let staging_dir = tempdir().unwrap();
        let asset_path = source_dir.path().join("RABBIT-windows.zip");
        fs::write(&asset_path, b"rabbit-update").unwrap();

        let error = stage_self_update_from_report(
            &sample_check_report(
                asset_path.display().to_string(),
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            ),
            staging_dir.path(),
        )
        .unwrap_err();

        let staged_path = staging_dir.path().join("0.2.0").join("RABBIT-windows.zip");
        assert!(matches!(error, RabbitError::HashMismatch { .. }));
        assert!(!staged_path.exists());
    }

    fn sample_manifest() -> SelfUpdateManifest {
        parse_self_update_manifest(
            r#"{
              "version": "0.2.0",
              "channel": "stable",
              "published_at": "2026-04-25T00:00:00Z",
              "release_notes_url": "https://example.test/releases/v0.2.0",
              "minimum_supported_previous_version": "0.1.0",
              "assets": {
                "windows": {
                  "url": "https://example.test/RABBIT-windows.zip",
                  "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "macos": {
                  "url": "https://example.test/RABBIT-macos.zip",
                  "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                }
              }
            }"#,
            MANIFEST_URL,
        )
        .unwrap()
    }

    fn sample_check_report(url: String, sha256: &str) -> SelfUpdateCheckReport {
        SelfUpdateCheckReport {
            manifest_url: MANIFEST_URL.to_string(),
            current_version: Version::parse("0.1.0").unwrap(),
            latest_version: Version::parse("0.2.0").unwrap(),
            channel: "stable".to_string(),
            published_at: "2026-04-25T00:00:00Z".to_string(),
            release_notes_url: Some("https://example.test/releases/v0.2.0".to_string()),
            minimum_supported_previous_version: Some(Version::parse("0.1.0").unwrap()),
            update_available: true,
            requires_manual_transition: false,
            asset: SelfUpdateAssetSelection {
                platform: Platform::Windows,
                url,
                sha256: sha256.to_string(),
                kind: SelfUpdateAssetKind::Binary,
            },
        }
    }

    fn write_test_release_binary(path: &std::path::Path, contents: &[u8]) {
        fs::write(path, contents).unwrap();
    }

    fn staged_report_for_binary(
        binary_path: &std::path::Path,
        staging_dir: &std::path::Path,
    ) -> SelfUpdateStageReport {
        let binary_sha = sha256_file(binary_path).unwrap();
        let mut check = sample_check_report(binary_path.display().to_string(), &binary_sha);
        check.asset.url = binary_path.display().to_string();
        SelfUpdateStageReport {
            check,
            staging_dir: staging_dir.to_path_buf(),
            staged_asset_path: Some(binary_path.to_path_buf()),
            downloaded: true,
            reused_existing_file: false,
            verified_sha256: Some(binary_sha),
            ready_to_apply: true,
            status_message: "ready".to_string(),
        }
    }

    #[test]
    fn apply_self_update_replaces_install_file_using_versioned_source_name() {
        let staging_root = tempdir().unwrap();
        let install_root = tempdir().unwrap();
        // The staged source file follows the new versioned naming
        // (`rabbit-<version>-<os>-<arch>.exe`); the install target is
        // whatever the user named their binary on disk (`RABBIT.exe`). The
        // swap should not require the two names to match.
        let staged_binary_path = staging_root
            .path()
            .join("0.2.0")
            .join("rabbit-0.2.0-windows-x86_64.exe");
        fs::create_dir_all(staged_binary_path.parent().unwrap()).unwrap();
        write_test_release_binary(&staged_binary_path, b"new-rabbit-binary");

        fs::write(install_root.path().join("RABBIT.exe"), b"old-rabbit-binary").unwrap();

        let stage = staged_report_for_binary(&staged_binary_path, staging_root.path());
        let report = apply_self_update(
            &stage,
            &ApplySelfUpdateOptions {
                install_root: Some(install_root.path().to_path_buf()),
                install_target_basename: Some("RABBIT.exe".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(report.replaced_files.len(), 1);
        assert_eq!(
            fs::read(install_root.path().join("RABBIT.exe")).unwrap(),
            b"new-rabbit-binary"
        );
        assert_eq!(
            fs::read(install_root.path().join("RABBIT.exe.rabbit-old")).unwrap(),
            b"old-rabbit-binary"
        );
        assert!(report.skipped_files.is_empty());
    }

    #[test]
    fn apply_self_update_skips_missing_install_target_without_writing() {
        let staging_root = tempdir().unwrap();
        let install_root = tempdir().unwrap();
        let staged_binary_path = staging_root
            .path()
            .join("0.2.0")
            .join("rabbit-0.2.0-macos-aarch64");
        fs::create_dir_all(staged_binary_path.parent().unwrap()).unwrap();
        write_test_release_binary(&staged_binary_path, b"new-mac-binary");

        // Install root does not contain a `RABBIT` file yet — the swap step
        // should record it as skipped without creating one.
        let stage = staged_report_for_binary(&staged_binary_path, staging_root.path());
        let report = apply_self_update(
            &stage,
            &ApplySelfUpdateOptions {
                install_root: Some(install_root.path().to_path_buf()),
                install_target_basename: Some("RABBIT".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(report.replaced_files.is_empty());
        assert!(
            report
                .skipped_files
                .iter()
                .any(|path| path.ends_with("RABBIT"))
        );
        assert!(!install_root.path().join("RABBIT").exists());
    }

    #[test]
    fn apply_self_update_rejects_hash_mismatch_without_touching_install() {
        let staging_root = tempdir().unwrap();
        let install_root = tempdir().unwrap();
        let staged_binary_path = staging_root
            .path()
            .join("0.2.0")
            .join("rabbit-0.2.0-windows-x86_64.exe");
        fs::create_dir_all(staged_binary_path.parent().unwrap()).unwrap();
        write_test_release_binary(&staged_binary_path, b"new-rabbit-binary");

        fs::write(install_root.path().join("RABBIT.exe"), b"old-rabbit-binary").unwrap();

        let mut stage = staged_report_for_binary(&staged_binary_path, staging_root.path());
        stage.check.asset.sha256 =
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();

        let error = apply_self_update(
            &stage,
            &ApplySelfUpdateOptions {
                install_root: Some(install_root.path().to_path_buf()),
                install_target_basename: Some("RABBIT.exe".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(matches!(error, RabbitError::HashMismatch { .. }));
        assert_eq!(
            fs::read(install_root.path().join("RABBIT.exe")).unwrap(),
            b"old-rabbit-binary"
        );
        assert!(!install_root.path().join("RABBIT.exe.rabbit-old").exists());
    }

    #[test]
    fn apply_self_update_rejects_when_stage_is_not_ready() {
        let staging_root = tempdir().unwrap();
        let install_root = tempdir().unwrap();
        let mut stage = sample_check_report(
            "https://example.test/rabbit-0.2.0-windows-x86_64.exe".to_string(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );
        stage.update_available = false;

        let stage_report = SelfUpdateStageReport {
            check: stage,
            staging_dir: staging_root.path().to_path_buf(),
            staged_asset_path: None,
            downloaded: false,
            reused_existing_file: false,
            verified_sha256: None,
            ready_to_apply: false,
            status_message: "Current RABBIT version is already up to date.".to_string(),
        };

        let error = apply_self_update(
            &stage_report,
            &ApplySelfUpdateOptions {
                install_root: Some(install_root.path().to_path_buf()),
                install_target_basename: Some("RABBIT.exe".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(matches!(error, RabbitError::InvalidPlannedExecution { .. }));
    }

    // (`apply_self_update_refuses_when_package_install_lock_is_held`
    // used to assert that a global package-install lock blocked the
    // self-update apply path. The lock is now per-target so the cross-
    // target check is gone — see `apply_self_update`'s comment for the
    // rationale.)

    #[cfg(unix)]
    #[test]
    fn apply_self_update_restores_execute_bit_on_install_target() {
        // Regression for issue #5: the staged source has 0o644 (as it would
        // after coming off a GitHub release download — HTTPS strips Unix
        // mode bits), and the freshly copied install target should still
        // end up executable so the macOS .app bundle stays launchable.
        use std::os::unix::fs::PermissionsExt;

        let staging_root = tempdir().unwrap();
        let install_root = tempdir().unwrap();
        let staged_binary_path = staging_root
            .path()
            .join("0.2.0")
            .join("rabbit-0.2.0-macos-aarch64");
        fs::create_dir_all(staged_binary_path.parent().unwrap()).unwrap();
        write_test_release_binary(&staged_binary_path, b"new-mac-binary");
        fs::set_permissions(&staged_binary_path, fs::Permissions::from_mode(0o644)).unwrap();

        let install_target = install_root.path().join("rabbit");
        fs::write(&install_target, b"old-mac-binary").unwrap();
        fs::set_permissions(&install_target, fs::Permissions::from_mode(0o755)).unwrap();

        let stage = staged_report_for_binary(&staged_binary_path, staging_root.path());
        let report = apply_self_update(
            &stage,
            &ApplySelfUpdateOptions {
                install_root: Some(install_root.path().to_path_buf()),
                install_target_basename: Some("rabbit".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(report.replaced_files.len(), 1);
        let mode = fs::metadata(&install_target).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o755,
            "post-swap install target should be executable, got {mode:o}"
        );
    }

    #[test]
    fn finds_enclosing_app_bundle_for_macos_install_targets() {
        // Standard macOS layout: install target is the Mach-O inside
        // `<bundle>/Contents/MacOS/`. The walk should land on the bundle
        // root regardless of how deep the bundle sits in the filesystem.
        let bundle = Path::new("/Applications/Rabbit.app");
        let install_target = bundle.join("Contents/MacOS/rabbit");
        assert_eq!(
            enclosing_app_bundle(&install_target),
            Some(bundle.to_path_buf())
        );

        // Bundle nested under a user's Downloads folder. Same shape, just
        // deeper — the walk still has to reach `Rabbit.app`.
        let nested_bundle = PathBuf::from("/Users/alice/Downloads/Rabbit/Rabbit.app");
        assert_eq!(
            enclosing_app_bundle(&nested_bundle.join("Contents/MacOS/rabbit")),
            Some(nested_bundle.clone())
        );

        // Standalone CLI install (no `.app` ancestor): function returns
        // `None` so the caller skips bundle re-signing.
        assert_eq!(
            enclosing_app_bundle(Path::new("/usr/local/bin/rabbit")),
            None
        );
        assert_eq!(
            enclosing_app_bundle(Path::new(
                "/Users/alice/projects/rabbit/target/release/rabbit"
            )),
            None
        );
    }
}
