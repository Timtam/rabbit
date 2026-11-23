use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cancel::CancelToken;
use crate::error::{IoPathContext, RabbitError, Result};
use crate::hash::sha256_file;
use crate::hfs::{fetch_file_list, file_url as hfs_file_url};
use crate::latest::{
    github_asset_version, github_release_url, pick_jaws_for_reaper_version, resolve_github_version,
    resolve_version_rule,
};
use crate::model::{Architecture, Platform};
use crate::package::{
    GithubArtifactKind, GithubReleaseSpec, HttpArtifactSource, HttpArtifactSpec,
    HttpArtifactTarget, PackageChannels, PackageSpec, STABLE_CHANNEL, VersionRule, VersionSource,
    package_specs_by_id_on,
};
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::version::Version;

/// Chunk size for the streaming download loop. 64 KiB is a comfortable
/// trade-off between syscall overhead (smaller chunks → more `read`s and
/// more `write`s) and event latency (larger chunks → the UI sees the
/// progress bar jump in coarser steps). At a realistic 30 MB REAPER dmg
/// this gives ~480 read/write iterations.
const DOWNLOAD_CHUNK_SIZE: usize = 64 * 1024;

/// Minimum wall-clock spacing between `DownloadProgress` events for the
/// same download. The wxdragon UI thread updates the gauge once per
/// event; keeping the rate below ~5 Hz prevents the UI from getting
/// flooded on a fast network where chunked reads return constantly.
const DOWNLOAD_PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(200);

/// Minimum byte-count delta between `DownloadProgress` events. Ensures a
/// fast-enough download still emits enough events to feel smooth even
/// when the interval throttle doesn't fire — e.g. a fast LAN delivering
/// 30 MB in well under a second still produces ~15 progress ticks.
const DOWNLOAD_PROGRESS_MIN_BYTES: u64 = 256 * 1024;

/// How long a download may completely stall (no bytes arriving) before the
/// read is aborted and retried. `reqwest::blocking`'s DEFAULT per-operation
/// timeout is 30 seconds, which field reports showed kills large downloads
/// from slow upstreams (Gyan.dev serving FFmpeg's ~390 MB `.7z` stalls under
/// load) with the cryptic "error decoding response body". 60 seconds
/// tolerates a busy server; the retry-with-resume loop recovers from an
/// abort without losing the bytes already on disk.
const DOWNLOAD_STALL_TIMEOUT: Duration = Duration::from_secs(60);

/// Connection-establishment timeout for downloads (separate from the stall
/// timeout so a dead host still fails fast).
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Total request attempts per artifact (1 initial + 3 retries). Retries
/// resume from the bytes already downloaded when the server supports byte
/// ranges and sent a strong validator.
const DOWNLOAD_MAX_ATTEMPTS: usize = 4;

/// Backoff between download attempts.
const DOWNLOAD_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(5),
];

const USER_AGENT: &str = "RABBIT/0.1 (+https://github.com/Timtam/rabbit)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    Installer,
    Archive,
    /// `.7z` archive — used by the FFmpeg shared builds we ship from
    /// Gyan.dev, which ships `.7z` exclusively for the shared variant; the
    /// install pipeline dispatches to a dedicated 7z extractor since
    /// the `zip` crate can't read these.
    SevenZipArchive,
    DiskImage,
    ExtensionBinary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDescriptor {
    pub package_id: String,
    pub version: Version,
    pub platform: Platform,
    pub architecture: Architecture,
    pub kind: ArtifactKind,
    pub url: String,
    pub file_name: String,
    /// The channel this artifact was resolved on (`dev`, `pr:1454`), or
    /// `None` for stable. It travels with the artifact so whichever install
    /// path lands it can record the channel in the receipt without being
    /// told separately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedArtifact {
    pub descriptor: ArtifactDescriptor,
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub reused_existing_file: bool,
}

pub fn resolve_latest_artifacts(
    package_ids: &[String],
    platform: Platform,
    architecture: Architecture,
) -> Result<Vec<ArtifactDescriptor>> {
    resolve_latest_artifacts_on(package_ids, platform, architecture, &PackageChannels::new())
}

/// [`resolve_latest_artifacts`], with each package resolved on the channel
/// `channels` chooses for it. Every returned artifact records that channel,
/// so the receipt written when it is installed remembers where it came from.
pub fn resolve_latest_artifacts_on(
    package_ids: &[String],
    platform: Platform,
    architecture: Architecture,
    channels: &PackageChannels,
) -> Result<Vec<ArtifactDescriptor>> {
    let on_channels = package_specs_by_id_on(platform, channels);
    let mut artifacts = Vec::new();
    for package_id in package_ids {
        let channel = channels
            .get(package_id)
            .filter(|channel| channel.as_str() != STABLE_CHANNEL)
            .cloned();
        let ids = std::slice::from_ref(package_id);
        match resolve_artifacts_with_specs(ids, platform, architecture, &on_channels) {
            Ok(resolved) => artifacts.extend(resolved.into_iter().map(|mut artifact| {
                artifact.channel = channel.clone();
                artifact
            })),
            // The pull request's build has run out. That is the listing
            // answering "nothing", not an outage, so the package goes back to
            // its regular release - recorded as stable, so the receipt forgets
            // the pull request. A real outage still fails below.
            Err(RabbitError::PullRequestBuildGone { .. }) => {
                artifacts.extend(resolve_artifacts_with_specs(
                    ids,
                    platform,
                    architecture,
                    &package_specs_by_id_on(platform, &PackageChannels::new()),
                )?)
            }
            Err(error) => return Err(error),
        }
    }
    Ok(artifacts)
}

fn resolve_artifacts_with_specs(
    package_ids: &[String],
    platform: Platform,
    architecture: Architecture,
    specs: &std::collections::BTreeMap<String, crate::package::PackageSpec>,
) -> Result<Vec<ArtifactDescriptor>> {
    let client = http_client()?;
    let mut artifacts = Vec::new();
    let architecture = canonicalize_dispatch_arch(architecture);

    for package_id in package_ids {
        // Data-driven packages resolve through the generic engines: a single
        // GitHub release (github_release) or a non-GitHub scrape/fixed
        // source (http_artifact). Only JAWS still has a bespoke resolver.
        let spec = specs.get(package_id);
        // The offline rule first, the one the wizard and the CLI ask before a
        // run starts (see `package_has_build_for`), so every package it
        // refuses is refused here before anything goes online. A package
        // that doesn't list this architecture has no build for it, even
        // where an asset for any architecture would match: Surge XT's only
        // Windows installer is 64-bit.
        if spec.is_some_and(|spec| !has_build_for_dispatch(spec, platform, architecture)) {
            return Err(RabbitError::NoArtifactFound {
                package_id: package_id.clone(),
                platform,
                architecture,
            });
        }
        if let Some(github_release) = spec.and_then(|spec| spec.github_release.as_ref()) {
            artifacts.push(resolve_github_artifact(
                &client,
                package_id,
                github_release,
                platform,
                architecture,
            )?);
            continue;
        }
        if let Some(http_artifact) = spec.and_then(|spec| spec.http_artifact.as_ref()) {
            artifacts.push(resolve_http_artifact(
                &client,
                package_id,
                http_artifact,
                spec.and_then(|spec| spec.version.as_ref()),
                platform,
                architecture,
            )?);
            continue;
        }
        if let Some(hfs_listing) = spec.and_then(|spec| spec.hfs_listing.as_ref()) {
            artifacts.push(resolve_hfs_artifact(
                &client,
                package_id,
                hfs_listing,
                platform,
            )?);
            continue;
        }
        if let Some(actions) = spec.and_then(|spec| spec.github_actions_artifact.as_ref()) {
            let (build, target) =
                crate::actions_artifact::newest_build(&client, actions, package_id, platform)?;
            artifacts.push(ArtifactDescriptor {
                package_id: package_id.clone(),
                version: Version::parse(&build.version).map_err(|_| RabbitError::RemoteData {
                    url: crate::actions_artifact::download_url(actions, &build),
                    message: format!("unreadable build version {:?}", build.version),
                })?,
                platform,
                architecture: target.report_arch,
                kind: github_artifact_kind(target.artifact_kind),
                url: crate::actions_artifact::download_url(actions, &build),
                // GitHub zips every artifact; the name says what is inside.
                file_name: format!("{}.zip", build.artifact_name),
                channel: None,
            });
            continue;
        }
        return Err(RabbitError::NoArtifactFound {
            package_id: package_id.clone(),
            platform,
            architecture,
        });
    }

    Ok(artifacts)
}

pub fn expected_artifact_kind(
    package_id: &str,
    platform: Platform,
    architecture: Architecture,
) -> Result<ArtifactKind> {
    expected_artifact_kind_on(package_id, platform, architecture, None)
}

/// [`expected_artifact_kind`] for `package_id` on `channel` (`None` = stable).
/// A channel may fetch a different kind of file than stable does - OSARA's
/// pull-request builds arrive zipped, for instance.
pub fn expected_artifact_kind_on(
    package_id: &str,
    platform: Platform,
    architecture: Architecture,
    channel: Option<&str>,
) -> Result<ArtifactKind> {
    let architecture = canonicalize_dispatch_arch(architecture);
    let channels: PackageChannels = channel
        .map(|channel| {
            [(package_id.to_string(), channel.to_string())]
                .into_iter()
                .collect()
        })
        .unwrap_or_default();
    let specs = package_specs_by_id_on(platform, &channels);
    // The same offline checks the resolver makes, so a package reports a
    // kind exactly when its download can be resolved for this REAPER.
    let spec = specs
        .get(package_id)
        .filter(|spec| resolves_for(spec, platform, architecture));
    if let Some(github_release) = spec.and_then(|spec| spec.github_release.as_ref()) {
        let selector = select_github_selector(github_release, platform, architecture);
        return github_kind_for(github_release, selector).ok_or(RabbitError::NoArtifactFound {
            package_id: package_id.to_string(),
            platform,
            architecture,
        });
    }
    if let Some(http_artifact) = spec.and_then(|spec| spec.http_artifact.as_ref()) {
        let target = select_http_target(http_artifact, platform, architecture).ok_or(
            RabbitError::NoArtifactFound {
                package_id: package_id.to_string(),
                platform,
                architecture,
            },
        )?;
        return Ok(github_artifact_kind(target.artifact_kind));
    }
    if let Some(hfs_listing) = spec.and_then(|spec| spec.hfs_listing.as_ref()) {
        return Ok(github_artifact_kind(hfs_listing.artifact_kind));
    }
    if let Some(target) = spec
        .and_then(|spec| spec.github_actions_artifact.as_ref())
        .and_then(|actions| crate::actions_artifact::target_for(actions, platform))
    {
        return Ok(github_artifact_kind(target.artifact_kind));
    }
    Err(RabbitError::NoArtifactFound {
        package_id: package_id.to_string(),
        platform,
        architecture,
    })
}

/// Map the manifest's [`GithubArtifactKind`] onto the install pipeline's
/// [`ArtifactKind`].
fn github_artifact_kind(kind: GithubArtifactKind) -> ArtifactKind {
    match kind {
        GithubArtifactKind::Archive => ArtifactKind::Archive,
        GithubArtifactKind::ExtensionBinary => ArtifactKind::ExtensionBinary,
        GithubArtifactKind::Installer => ArtifactKind::Installer,
        GithubArtifactKind::DiskImage => ArtifactKind::DiskImage,
        GithubArtifactKind::SevenZipArchive => ArtifactKind::SevenZipArchive,
    }
}

/// The asset selector a `github_release` picks for `(platform, arch)` — the
/// SAME platform+arch predicate the download resolver uses, so the resolved
/// kind and the kind reported by `expected_artifact_kind` can't diverge.
fn select_github_selector(
    spec: &GithubReleaseSpec,
    platform: Platform,
    architecture: Architecture,
) -> Option<&crate::package::AssetSelector> {
    spec.assets.iter().find(|selector| {
        selector.platform.matches_platform(platform)
            && selector.arch.is_none_or(|arch| arch == architecture)
    })
}

/// Resolve the [`ArtifactKind`] for a `github_release`: a matched selector's
/// per-asset `artifact_kind` wins, else the spec-wide fallback. `None` only
/// when the manifest sets neither (the load-time validator rejects that for
/// the resolve path; `expected_artifact_kind` maps `None` to `NoArtifactFound`
/// for a platform the package doesn't ship).
fn github_kind_for(
    spec: &GithubReleaseSpec,
    selector: Option<&crate::package::AssetSelector>,
) -> Option<ArtifactKind> {
    selector
        .and_then(|selector| selector.artifact_kind)
        .or(spec.artifact_kind)
        .map(github_artifact_kind)
}

/// Resolve the download for a data-driven GitHub-release package: read the
/// release, pick the asset matching this platform/arch selector, and return
/// it as an [`ArtifactDescriptor`]. Generalizes the hand-written
/// `resolve_reakontrol_artifact` / `resolve_app2clap_artifact` resolvers.
fn resolve_github_artifact(
    client: &Client,
    package_id: &str,
    spec: &GithubReleaseSpec,
    platform: Platform,
    architecture: Architecture,
) -> Result<ArtifactDescriptor> {
    let url = github_release_url(spec);
    let body = http_get_text(client, &url)?;
    resolve_github_artifact_from_release_body(&body, &url, package_id, spec, platform, architecture)
}

fn resolve_github_artifact_from_release_body(
    body: &str,
    url: &str,
    package_id: &str,
    spec: &GithubReleaseSpec,
    platform: Platform,
    architecture: Architecture,
) -> Result<ArtifactDescriptor> {
    let value: Value = serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
        url: url.to_string(),
        message: source.to_string(),
    })?;
    let assets = value
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| RabbitError::RemoteData {
            url: url.to_string(),
            message: "missing array field: assets".to_string(),
        })?;

    let selector = spec
        .assets
        .iter()
        .find(|selector| {
            selector.platform.matches_platform(platform)
                && selector.arch.is_none_or(|arch| arch == architecture)
        })
        .ok_or(RabbitError::NoArtifactFound {
            package_id: package_id.to_string(),
            platform,
            architecture,
        })?;

    // `TagName` versions are the same for every asset, so compute once; for
    // `AssetName` each candidate carries its own version and we keep the
    // highest (the rolling-tag asset order is unreliable).
    let tag_version = match &spec.version_from {
        VersionSource::TagName { .. } => Some(resolve_github_version(body, url, spec)?),
        VersionSource::AssetName { .. } => None,
    };

    let mut best: Option<(Version, &str, &str)> = None;
    for asset in assets {
        let Some(name) = asset.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !selector.matches_asset(name) {
            continue;
        }
        let Some(download_url) = asset.get("browser_download_url").and_then(Value::as_str) else {
            continue;
        };
        let version = match (&spec.version_from, &tag_version) {
            (
                VersionSource::AssetName {
                    strip_trailing_dot_segment,
                },
                _,
            ) => {
                // Extract from THIS matched selector's prefix/suffix, so a
                // per-platform package (ReaKontrol) versions each slice with
                // its own prefix.
                match github_asset_version(
                    name,
                    std::slice::from_ref(selector),
                    *strip_trailing_dot_segment,
                ) {
                    Some(version) => version,
                    None => continue,
                }
            }
            (VersionSource::TagName { .. }, Some(version)) => version.clone(),
            (VersionSource::TagName { .. }, None) => continue,
        };
        best = Some(match best {
            Some((current_version, current_name, current_url))
                if current_version.cmp_lenient(&version).is_ge() =>
            {
                (current_version, current_name, current_url)
            }
            _ => (version, name, download_url),
        });
    }

    let (version, file_name, download_url) = best.ok_or(RabbitError::NoArtifactFound {
        package_id: package_id.to_string(),
        platform,
        architecture,
    })?;

    let kind = github_kind_for(spec, Some(selector)).ok_or_else(|| RabbitError::RemoteData {
        url: url.to_string(),
        message: format!("github_release for {package_id} resolved no artifact_kind"),
    })?;

    Ok(ArtifactDescriptor {
        package_id: package_id.to_string(),
        version,
        platform,
        // A selector that pins an arch reports it (ReaPack's per-slice DLLs),
        // or the x64 build it picks for an ARM REAPER (app2clap); an any-arch
        // selector reports Universal, matching the old resolvers.
        architecture: selector
            .report_arch
            .or(selector.arch)
            .unwrap_or(Architecture::Universal),
        kind,
        url: download_url.to_string(),
        file_name: file_name.to_string(),
        channel: None,
    })
}

/// Collapse dispatch-time architecture sentinels (`Universal`, `Unknown`) to a
/// concrete host slice so per-arch resolvers (SWS's per-arch `.dmg`, ReaPack's
/// per-arch `.dylib` / `.dll`) pick a slice REAPER will actually load. Targets
/// shipping a single universal artifact (REAPER's `_universal.dmg`, OSARA,
/// ReaKontrol) ignore the rewrite because their resolvers map every arch to
/// the same file.
///
/// Both sentinels collapse for the same reason: we don't know — or don't need
/// to know — the target's per-arch slice, so the host arch is the safe answer.
/// - `Universal` shows up when REAPER's binary is a Mach-O fat binary (every
///   modern macOS REAPER).
/// - `Unknown` shows up when the binary probe failed — most commonly a fresh
///   first-time install where `/Applications/REAPER.app` doesn't yet exist,
///   but also corrupt or unreadable binaries. Falling back to the host arch
///   matches what the upcoming install will land (REAPER's macOS dmg is
///   universal; the Windows installer is host-arch).
///
/// Strategy:
/// - When RABBIT is running under Rosetta on an Apple Silicon host, force
///   `Arm64` regardless of `Architecture::current()`. `current()` reads
///   `target_arch` and would report `X64` (the slice Rosetta is translating),
///   but REAPER launched normally on the same host runs as `arm64` natively
///   — so the plug-in slice has to match REAPER's runtime arch, not RABBIT's.
///   `is_running_under_rosetta()` is a no-op on non-macOS hosts.
/// - Otherwise return `Architecture::current()`. On a universal RABBIT
///   binary, Apple Silicon hosts run the `arm64` slice and Intel hosts run
///   `x86_64`, which already matches what REAPER will load.
fn canonicalize_dispatch_arch(architecture: Architecture) -> Architecture {
    if matches!(
        architecture,
        Architecture::Universal | Architecture::Unknown
    ) {
        if rabbit_platform::is_running_under_rosetta() {
            return Architecture::Arm64;
        }
        return Architecture::current();
    }
    architecture
}

/// The architecture artifact resolution works with for a REAPER of
/// `architecture`, as [`resolve_latest_artifacts`] and
/// [`expected_artifact_kind`] see it. `Universal` (a macOS universal REAPER)
/// and `Unknown` (no REAPER there yet, or one whose binary couldn't be read)
/// become the slice REAPER runs as on this computer.
pub fn dispatch_architecture(architecture: Architecture) -> Architecture {
    canonicalize_dispatch_arch(architecture)
}

/// Whether `spec` has a build for a REAPER of `architecture` on `platform`,
/// answered from the manifest alone, without the network.
///
/// Artifact resolution asks this same rule first, for the same
/// [`dispatch_architecture`], and refuses what it refuses before it goes
/// online. It asks whether the package is offered on `platform`, lists that
/// architecture in `supported_architectures`, and has a download source with
/// a target for it. That is a `github_release` asset selector, an
/// `http_artifact` target's `match_arches`, the HFS listing (one installer
/// for every architecture), or the platform of a pull request build. Pass
/// `spec` as it resolves on its channel.
///
/// When the dispatch architecture is still unknown, on a computer RABBIT has
/// no build for, nothing is blocked, as before this check existed.
pub fn package_has_build_for(
    spec: &PackageSpec,
    platform: Platform,
    architecture: Architecture,
) -> bool {
    has_build_for_dispatch(spec, platform, canonicalize_dispatch_arch(architecture))
}

/// The packages among `package_ids` that have no build for a REAPER of
/// `architecture` on `platform`, each on the channel `channels` gives it (see
/// [`package_has_build_for`]). Ids `platform` doesn't offer aren't listed:
/// they aren't a question of architecture.
pub fn packages_without_build(
    package_ids: &[String],
    platform: Platform,
    architecture: Architecture,
    channels: &PackageChannels,
) -> Vec<String> {
    let specs = package_specs_by_id_on(platform, channels);
    package_ids
        .iter()
        .filter(|id| {
            specs
                .get(id.as_str())
                .is_some_and(|spec| !package_has_build_for(spec, platform, architecture))
        })
        .cloned()
        .collect()
}

/// [`package_has_build_for`] for an architecture already canonicalized. The
/// resolver asks it for every package before it goes online.
fn has_build_for_dispatch(
    spec: &PackageSpec,
    platform: Platform,
    architecture: Architecture,
) -> bool {
    architecture == Architecture::Unknown || resolves_for(spec, platform, architecture)
}

/// The checks resolution makes before it goes online, for a dispatch
/// architecture: the package is offered on `platform`, lists the
/// architecture, and its source has a target for it.
fn resolves_for(spec: &PackageSpec, platform: Platform, architecture: Architecture) -> bool {
    spec.supports_platform(platform)
        && lists_architecture(spec, architecture)
        && source_has_target(spec, platform, architecture)
}

/// Whether `spec` lists the dispatch `architecture` among the REAPER
/// architectures it has builds for. An unknown dispatch architecture passes,
/// so resolution on a computer RABBIT has no build for goes on as before.
fn lists_architecture(spec: &PackageSpec, architecture: Architecture) -> bool {
    architecture == Architecture::Unknown || spec.supports_architecture(architecture)
}

/// Whether `spec`'s download source has a target for `platform` and the
/// dispatch `architecture`, by the selectors the resolver itself uses. The
/// source is the one the resolver picks, in the same order.
fn source_has_target(spec: &PackageSpec, platform: Platform, architecture: Architecture) -> bool {
    if let Some(github_release) = &spec.github_release {
        return select_github_selector(github_release, platform, architecture).is_some();
    }
    if let Some(http_artifact) = &spec.http_artifact {
        return select_http_target(http_artifact, platform, architecture).is_some();
    }
    // One installer serves every architecture (the JAWS scripts).
    if spec.hfs_listing.is_some() {
        return true;
    }
    spec.github_actions_artifact
        .as_ref()
        .is_some_and(|actions| crate::actions_artifact::target_for(actions, platform).is_some())
}

/// Ephemeral artifact-download cache directory. Defaults to a stable path
/// under the OS temp directory (`%TEMP%\rabbit-cache` on Windows,
/// `$TMPDIR/rabbit-cache` on macOS). Reusing the same temp path across runs
/// keeps `download_artifacts` cheap when the user retries the wizard
/// within the same session, but the OS temp dir is cleaned periodically
/// — RABBIT no longer leaves persistent caches under
/// `%LOCALAPPDATA%\RABBIT\cache\` or `~/Library/Caches/RABBIT/`. Callers who
/// want stricter ephemeral semantics (e.g. a single-process lifetime)
/// can pass their own `tempfile::TempDir::path()` instead.
pub fn default_cache_dir() -> PathBuf {
    env::temp_dir().join("rabbit-cache")
}

pub fn download_artifacts(
    artifacts: &[ArtifactDescriptor],
    cache_dir: &Path,
) -> Result<Vec<CachedArtifact>> {
    download_artifacts_with_progress(
        artifacts,
        cache_dir,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
}

/// Like [`download_artifacts`] but emits per-artifact and per-chunk
/// [`ProgressEvent`]s through `progress`. The no-op overload above
/// exists so callers that don't want progress can keep their existing
/// call signature.
///
/// Downloads run through a small concurrent pool
/// ([`DOWNLOAD_POOL_CONCURRENCY`] workers); results are returned in input
/// order and the first failing artifact (in input order) aborts the batch,
/// cancelling the remaining downloads.
///
/// `cancel` stops the batch the same way, at each worker's next chunk or
/// retry checkpoint. A cancelled download reports
/// [`RabbitError::DownloadInterrupted`]; the caller decides whether that
/// reads as a failure or as "the user asked us to stop".
pub fn download_artifacts_with_progress(
    artifacts: &[ArtifactDescriptor],
    cache_dir: &Path,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<Vec<CachedArtifact>> {
    let (pool, handles) = spawn_download_pool(artifacts, cache_dir, progress, cancel);
    let mut cached = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.wait() {
            Ok(artifact) => cached.push(artifact),
            Err(error) => {
                pool.cancel();
                return Err(error);
            }
        }
    }
    Ok(cached)
}

/// How many artifact downloads run concurrently. Small enough to be polite
/// to upstreams and to keep per-download bandwidth reasonable on thin pipes,
/// large enough that a huge artifact (FFmpeg's ~390 MB `.7z`) starts
/// downloading almost immediately instead of queueing behind everything
/// else — which is what lets installs overlap with it.
const DOWNLOAD_POOL_CONCURRENCY: usize = 3;

/// Handle to one queued download: [`DownloadHandle::wait`] blocks until that
/// artifact's download finished (or failed).
pub(crate) struct DownloadHandle {
    package_id: String,
    receiver: std::sync::mpsc::Receiver<Result<CachedArtifact>>,
}

impl DownloadHandle {
    pub(crate) fn wait(self) -> Result<CachedArtifact> {
        self.receiver.recv().unwrap_or_else(|_| {
            Err(RabbitError::RemoteData {
                url: String::new(),
                message: format!(
                    "the download worker for {} terminated unexpectedly",
                    self.package_id
                ),
            })
        })
    }
}

/// A running pool of download worker threads. Dropping (or [`cancel`]ing)
/// the pool asks in-flight downloads to stop at their next chunk/retry
/// checkpoint, so an operation that bails early doesn't leave threads
/// streaming into the cache and emitting progress events after the
/// operation already returned. Workers finish naturally once the queue is
/// drained; after every handle has been waited on they are already idle.
///
/// [`cancel`]: DownloadPool::cancel
pub(crate) struct DownloadPool {
    cancel: CancelToken,
}

impl DownloadPool {
    pub(crate) fn cancel(&self) {
        self.cancel.cancel();
    }
}

impl Drop for DownloadPool {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Start downloading every artifact through a bounded worker pool. Jobs are
/// picked up in input order (so the caller's install order is also the
/// download priority) and each returns its result through the matching
/// [`DownloadHandle`], letting the caller consume completions one by one —
/// e.g. install a package the moment ITS download is done while later
/// downloads continue in the background.
///
/// Duplicate artifacts (same package/version/file — e.g. a package id
/// repeated on the CLI) are downloaded ONCE: two concurrent workers on the
/// same cache path would truncate each other's `.part` file mid-stream, so
/// duplicates share the first occurrence's job and receive a copy of its
/// result.
pub(crate) fn spawn_download_pool(
    artifacts: &[ArtifactDescriptor],
    cache_dir: &Path,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> (DownloadPool, Vec<DownloadHandle>) {
    type ResultSender = std::sync::mpsc::SyncSender<Result<CachedArtifact>>;

    // A child of the caller's token: the pool stops when the caller cancels
    // the whole operation, and the batch can also stop its own workers on a
    // failed artifact without cancelling anything above it.
    let cancel = cancel.child();
    let mut queue: std::collections::VecDeque<(ArtifactDescriptor, Vec<ResultSender>)> =
        std::collections::VecDeque::with_capacity(artifacts.len());
    let mut job_index_by_target: std::collections::HashMap<(String, String, String), usize> =
        std::collections::HashMap::new();
    let mut handles = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let key = (
            artifact.package_id.clone(),
            artifact.version.raw().to_string(),
            artifact.file_name.clone(),
        );
        match job_index_by_target.get(&key) {
            Some(&index) => queue[index].1.push(sender),
            None => {
                job_index_by_target.insert(key, queue.len());
                queue.push_back((artifact.clone(), vec![sender]));
            }
        }
        handles.push(DownloadHandle {
            package_id: artifact.package_id.clone(),
            receiver,
        });
    }
    let job_count = queue.len();
    let queue = Arc::new(std::sync::Mutex::new(queue));

    let worker_count = DOWNLOAD_POOL_CONCURRENCY.min(job_count);
    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let cancel = cancel.clone();
        let cache_dir = cache_dir.to_path_buf();
        let progress = progress.clone();
        std::thread::spawn(move || {
            // One client per worker so connections are reused across that
            // worker's downloads. A client-construction failure is delivered
            // per job (it is practically impossible and not clonable).
            let client = download_http_client();
            loop {
                if cancel.is_cancelled() {
                    return;
                }
                let job = queue.lock().ok().and_then(|mut queue| queue.pop_front());
                let Some((artifact, senders)) = job else {
                    return;
                };
                let result = match &client {
                    Ok(client) => {
                        download_artifact(client, &artifact, &cache_dir, &progress, &cancel)
                    }
                    Err(_) => Err(RabbitError::RemoteData {
                        url: artifact.url.clone(),
                        message: "could not construct the download HTTP client".to_string(),
                    }),
                };
                // Deliver to every handle sharing this job. `RabbitError`
                // isn't clonable, so duplicates of a FAILED job get a
                // summary error naming the same package; the first handle
                // (the one an operation actually consumes) gets the real
                // error. A receiver dropped early (operation bailed) is
                // fine.
                let mut senders = senders.into_iter();
                let first = senders.next();
                for sender in senders {
                    let duplicate_result = match &result {
                        Ok(cached) => Ok(cached.clone()),
                        Err(_) => Err(RabbitError::RemoteData {
                            url: artifact.url.clone(),
                            message: format!(
                                "the download of {} failed (duplicate request)",
                                artifact.package_id
                            ),
                        }),
                    };
                    let _ = sender.send(duplicate_result);
                }
                if let Some(first) = first {
                    let _ = first.send(result);
                }
            }
        });
    }

    (DownloadPool { cancel }, handles)
}

fn download_artifact(
    client: &Client,
    artifact: &ArtifactDescriptor,
    cache_dir: &Path,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<CachedArtifact> {
    let package_dir = cache_dir
        .join(&artifact.package_id)
        .join(artifact.version.raw().replace(',', "_"));
    fs::create_dir_all(&package_dir).with_path(&package_dir)?;

    let target_path = package_dir.join(&artifact.file_name);
    if target_path.is_file() {
        // Cache hit: tell the UI both that the download "started" and
        // immediately completed, with no bytes-progress events in
        // between. The bracketing pair keeps state machines on the
        // consumer side simple — every package emits the same shape
        // regardless of cache state.
        progress.report(ProgressEvent::DownloadStarted {
            package_id: artifact.package_id.clone(),
            bytes_total: None,
        });
        progress.report(ProgressEvent::DownloadCompleted {
            package_id: artifact.package_id.clone(),
        });
        return cached_artifact(artifact, target_path, true);
    }

    if let Some(source_path) = local_artifact_source_path(&artifact.url)? {
        progress.report(ProgressEvent::DownloadStarted {
            package_id: artifact.package_id.clone(),
            bytes_total: None,
        });
        copy_local_artifact(artifact, &source_path, &target_path)?;
        progress.report(ProgressEvent::DownloadCompleted {
            package_id: artifact.package_id.clone(),
        });
        return cached_artifact(artifact, target_path, false);
    }

    validate_remote_artifact_url(&artifact.url)?;

    let part_path = target_path.with_extension(format!(
        "{}.part",
        target_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("download")
    ));

    fetch_remote_artifact_with_retries(
        client,
        &artifact.url,
        &part_path,
        &artifact.package_id,
        progress,
        &DOWNLOAD_RETRY_DELAYS,
        cancel,
    )?;

    fs::rename(&part_path, &target_path).with_path(&target_path)?;
    progress.report(ProgressEvent::DownloadCompleted {
        package_id: artifact.package_id.clone(),
    });
    cached_artifact(artifact, target_path, false)
}

/// Download `url` into `part_path`, retrying transient network failures
/// (mid-body stalls, connection drops) up to [`DOWNLOAD_MAX_ATTEMPTS`] total
/// attempts. When the first response carried a strong validator (a strong
/// `ETag`, else `Last-Modified`), retries resume from the bytes already on
/// disk via `Range` + `If-Range`; a server that ignores the range (200) or
/// rejects it (416) restarts the download from zero, so the file is never a
/// mix of two upstream versions. Non-transient failures (HTTP error statuses,
/// disk-write errors) abort immediately; exhausted retries surface as
/// [`RabbitError::DownloadInterrupted`] naming the URL, so a flaky network
/// reads as a network problem rather than an I/O error at the cache path.
#[allow(clippy::too_many_arguments)]
fn fetch_remote_artifact_with_retries(
    client: &Client,
    url: &str,
    part_path: &Path,
    package_id: &str,
    progress: &ProgressReporter,
    retry_delays: &[Duration],
    cancel: &CancelToken,
) -> Result<()> {
    let mut bytes_downloaded: u64 = 0;
    let mut bytes_total: Option<u64> = None;
    let mut validator: Option<String> = None;
    let mut started_reported = false;
    let mut last_error: Option<String> = None;
    // Most progress achieved across all attempts, for the final error: the
    // restart arms reset `bytes_downloaded`, which would otherwise report
    // "received 0 bytes" after hundreds of MB of discarded transfer.
    let mut max_bytes_downloaded: u64 = 0;

    for attempt in 0..DOWNLOAD_MAX_ATTEMPTS {
        if cancel.is_cancelled() {
            return Err(RabbitError::DownloadInterrupted {
                url: url.to_string(),
                bytes_downloaded: max_bytes_downloaded,
                message: "the operation was cancelled".to_string(),
            });
        }
        if attempt > 0 {
            let delay = retry_delays
                .get(attempt - 1)
                .or_else(|| retry_delays.last())
                .copied()
                .unwrap_or(Duration::ZERO);
            std::thread::sleep(delay);
        }

        let resuming = bytes_downloaded > 0 && validator.is_some();
        let mut request = client.get(url);
        if resuming {
            request = request
                .header(reqwest::header::RANGE, format!("bytes={bytes_downloaded}-"))
                .header(
                    reqwest::header::IF_RANGE,
                    validator.as_deref().unwrap_or_default(),
                );
        }
        let response = match request.send() {
            Ok(response) => response,
            Err(source) => {
                last_error = Some(source.to_string());
                continue;
            }
        };

        let status = response.status();
        if resuming && status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            // Our resume offset no longer makes sense upstream (the file
            // changed or shrank). Forget the partial state and restart.
            // (A 416 to a plain GET is NOT resume-related and falls through
            // to the fail-fast status handling below.)
            bytes_downloaded = 0;
            validator = None;
            last_error = Some("server rejected the resume range".to_string());
            continue;
        }
        if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            // 5xx / 429 are the canonical transient statuses — an overloaded
            // origin (the exact condition that motivated this retry loop)
            // fronts them for seconds at a time. Retry WITHOUT touching the
            // resume state, so a 503 on a resume attempt doesn't discard the
            // bytes already on disk.
            last_error = Some(format!("server answered {status}"));
            continue;
        }
        let response = match response.error_for_status() {
            Ok(response) => response,
            Err(source) => {
                // Remaining HTTP error statuses (404/403/...) are not fixed
                // by retrying the same URL; surface them directly.
                return Err(RabbitError::Http {
                    url: url.to_string(),
                    source,
                });
            }
        };

        let honored_resume = resuming
            && response.status() == reqwest::StatusCode::PARTIAL_CONTENT
            && content_range_start(response.headers()) == Some(bytes_downloaded);
        if resuming && response.status() == reqwest::StatusCode::PARTIAL_CONTENT && !honored_resume
        {
            // A 206 whose Content-Range does not start exactly at our resume
            // offset would silently corrupt the file if appended. Restart.
            bytes_downloaded = 0;
            validator = None;
            last_error = Some("server answered the resume with a mismatched range".to_string());
            continue;
        }

        let mut file = if honored_resume {
            // Server honored the range and the If-Range validator matched:
            // append the remaining bytes to what we already have. If the
            // original 200 had no Content-Length (chunked), the 206's
            // Content-Range complete-length re-arms the truncation check.
            if bytes_total.is_none() {
                bytes_total = content_range_total(response.headers());
            }
            fs::OpenOptions::new()
                .append(true)
                .open(part_path)
                .with_path(part_path)?
        } else {
            // Fresh download, or the server ignored the range (200 = full
            // body, possibly a different upstream file): start from zero.
            bytes_downloaded = 0;
            bytes_total = response.content_length();
            validator = resume_validator(response.headers());
            fs::File::create(part_path).with_path(part_path)?
        };

        if !started_reported {
            progress.report(ProgressEvent::DownloadStarted {
                package_id: package_id.to_string(),
                bytes_total,
            });
            started_reported = true;
        }

        let stream_result = stream_response_to_file(
            response,
            &mut file,
            package_id,
            &mut bytes_downloaded,
            bytes_total,
            progress,
            cancel,
        );
        max_bytes_downloaded = max_bytes_downloaded.max(bytes_downloaded);
        match stream_result {
            Ok(()) => {
                file.flush().with_path(part_path)?;
                drop(file);
                // Belt and braces: a server that closed the stream cleanly
                // but short of its own Content-Length produced a truncated
                // file — treat as transient and retry rather than caching it.
                if let Some(total) = bytes_total
                    && bytes_downloaded != total
                {
                    last_error = Some(format!("received {bytes_downloaded} of {total} bytes"));
                    continue;
                }
                return Ok(());
            }
            Err(StreamCopyError::Write(source)) => {
                // Disk-side failure (full disk, permissions, security
                // software blocking the write): retrying the network won't
                // help. Classified so an antivirus block reads as one.
                return Err(crate::error::io_error_at(part_path.to_path_buf(), source));
            }
            Err(StreamCopyError::Read(source)) => {
                // Network-side failure mid-body (stall timeout, reset):
                // retry, resuming if we can.
                last_error = Some(source.to_string());
                continue;
            }
        }
    }

    Err(RabbitError::DownloadInterrupted {
        url: url.to_string(),
        bytes_downloaded: max_bytes_downloaded,
        message: last_error.unwrap_or_else(|| "the connection kept dropping".to_string()),
    })
}

/// The validator a retry may present in `If-Range` to guarantee the resumed
/// tail belongs to the same upstream file: a strong `ETag`, or nothing.
/// RFC 9110 §13.1.5 forbids weak validators in `If-Range`, and a
/// `Last-Modified` date is only "strong" under conditions we can't cheaply
/// verify (one-second resolution invites splicing two upstream builds
/// published within the same second). Both artifact upstreams we resume from
/// (gyan.dev, GitHub release assets) send strong ETags, so restricting
/// resumption to those costs essentially nothing. `None` disables resumption
/// — retries restart from zero.
fn resume_validator(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let etag = headers
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())?;
    if etag.starts_with("W/") {
        return None;
    }
    Some(etag.to_string())
}

/// The first byte position of a 206 response's `Content-Range` header
/// (`bytes <start>-<end>/<total>`), or `None` when absent or malformed. Used
/// to confirm a resumed download's tail really starts at our offset before
/// appending it.
fn content_range_start(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let value = headers
        .get(reqwest::header::CONTENT_RANGE)?
        .to_str()
        .ok()?
        .trim();
    let range = value.strip_prefix("bytes ")?;
    let (start, _) = range.split_once('-')?;
    start.trim().parse().ok()
}

/// The complete-length field of a 206 response's `Content-Range` header
/// (`bytes <start>-<end>/<total>`), or `None` when absent, malformed, or the
/// unknown-length `*` form.
fn content_range_total(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let value = headers
        .get(reqwest::header::CONTENT_RANGE)?
        .to_str()
        .ok()?
        .trim();
    let range = value.strip_prefix("bytes ")?;
    let (_, total) = range.split_once('/')?;
    total.trim().parse().ok()
}

/// Why the chunked copy in [`stream_response_to_file`] stopped: a network
/// read failure (retryable) vs a disk write failure (not retryable).
enum StreamCopyError {
    Read(std::io::Error),
    Write(std::io::Error),
}

/// Chunked replacement for `std::io::copy` that fires
/// [`ProgressEvent::DownloadProgress`] as bytes accumulate. Throttles
/// events to one per `DOWNLOAD_PROGRESS_MIN_INTERVAL` or per
/// `DOWNLOAD_PROGRESS_MIN_BYTES`, whichever fires second, so the UI
/// thread never gets flooded on a fast network. Always emits a final
/// event at the end so the bar lands exactly at `bytes_total` even when
/// the last chunk was below the byte-threshold.
#[allow(clippy::too_many_arguments)]
fn stream_response_to_file(
    mut response: reqwest::blocking::Response,
    file: &mut fs::File,
    package_id: &str,
    bytes_downloaded: &mut u64,
    bytes_total: Option<u64>,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> std::result::Result<(), StreamCopyError> {
    let mut buffer = vec![0u8; DOWNLOAD_CHUNK_SIZE];
    let mut bytes_at_last_event: u64 = *bytes_downloaded;
    let mut last_event_at = Instant::now();

    loop {
        if cancel.is_cancelled() {
            return Err(StreamCopyError::Read(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "the operation was cancelled",
            )));
        }
        let read_bytes = response.read(&mut buffer).map_err(StreamCopyError::Read)?;
        if read_bytes == 0 {
            break;
        }
        file.write_all(&buffer[..read_bytes])
            .map_err(StreamCopyError::Write)?;
        *bytes_downloaded += read_bytes as u64;

        let bytes_since_last = *bytes_downloaded - bytes_at_last_event;
        let interval_elapsed = last_event_at.elapsed() >= DOWNLOAD_PROGRESS_MIN_INTERVAL;
        if interval_elapsed && bytes_since_last >= DOWNLOAD_PROGRESS_MIN_BYTES {
            progress.report(ProgressEvent::DownloadProgress {
                package_id: package_id.to_string(),
                bytes_downloaded: *bytes_downloaded,
                bytes_total,
            });
            bytes_at_last_event = *bytes_downloaded;
            last_event_at = Instant::now();
        }
    }

    // Final tick so the gauge always lands on the actual byte count
    // even if the last chunk was small enough to skip the throttle.
    // The trailing `DownloadCompleted` is what tells the UI "we're
    // done"; this event exists purely to settle the bytes display at
    // its final value.
    if *bytes_downloaded > bytes_at_last_event {
        progress.report(ProgressEvent::DownloadProgress {
            package_id: package_id.to_string(),
            bytes_downloaded: *bytes_downloaded,
            bytes_total,
        });
    }

    Ok(())
}

fn copy_local_artifact(
    artifact: &ArtifactDescriptor,
    source_path: &Path,
    target_path: &Path,
) -> Result<()> {
    let part_path = target_path.with_extension(format!(
        "{}.part",
        target_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("download")
    ));

    fs::copy(source_path, &part_path).with_path(source_path)?;
    fs::rename(&part_path, target_path).with_path(target_path)?;
    if !target_path.is_file() {
        return Err(RabbitError::RemoteData {
            url: artifact.url.clone(),
            message: "local artifact copy did not produce a cache file".to_string(),
        });
    }
    Ok(())
}

fn cached_artifact(
    descriptor: &ArtifactDescriptor,
    path: PathBuf,
    reused_existing_file: bool,
) -> Result<CachedArtifact> {
    // An installer that arrived zipped (GitHub zips every workflow artifact)
    // is unwrapped here, once, so everything downstream runs an ordinary
    // installer. No other package delivers an installer as a .zip.
    let path = if matches!(descriptor.kind, ArtifactKind::Installer)
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        crate::archive::extract_zipped_installer(&path)?
    } else {
        path
    };
    let metadata = fs::metadata(&path).with_path(&path)?;
    let sha256 = sha256_file(&path)?;

    Ok(CachedArtifact {
        descriptor: descriptor.clone(),
        path,
        size: metadata.len(),
        sha256,
        reused_existing_file,
    })
}

/// Select the first [`HttpArtifactTarget`] whose platform matches and whose
/// `match_arches` contains `architecture` (already canonicalized). Shared by
/// the resolver and `expected_artifact_kind` so the two never disagree on
/// which target — and therefore which kind — applies.
fn select_http_target(
    spec: &HttpArtifactSpec,
    platform: Platform,
    architecture: Architecture,
) -> Option<&HttpArtifactTarget> {
    spec.targets.iter().find(|target| {
        target.platform.matches_platform(platform) && target.match_arches.contains(&architecture)
    })
}

/// Resolve the download for a data-driven non-GitHub package (REAPER, OSARA,
/// SWS, FFmpeg). Selects the `(platform, arch)` target, then dispatches on its
/// source. The version is the package `VersionRule`. No matching target ->
/// `NoArtifactFound` (FFmpeg x86/macOS).
fn resolve_http_artifact(
    client: &Client,
    package_id: &str,
    spec: &HttpArtifactSpec,
    version_rule: Option<&VersionRule>,
    platform: Platform,
    architecture: Architecture,
) -> Result<ArtifactDescriptor> {
    let target =
        select_http_target(spec, platform, architecture).ok_or(RabbitError::NoArtifactFound {
            package_id: package_id.to_string(),
            platform,
            architecture,
        })?;
    let kind = github_artifact_kind(target.artifact_kind);
    match &target.source {
        HttpArtifactSource::ScrapeHref {
            page_url,
            base_url,
            href_match,
        } => {
            let body = http_get_text(client, page_url)?;
            let href = find_href_with(&body, |href, _context| href_match.matches(href)).ok_or(
                RabbitError::NoArtifactFound {
                    package_id: package_id.to_string(),
                    platform,
                    architecture,
                },
            )?;
            let version = resolve_http_version(client, package_id, version_rule)?;
            artifact_from_href(
                package_id,
                version,
                platform,
                target.report_arch,
                kind,
                base_url,
                &href,
            )
        }
        HttpArtifactSource::FixedUrl { url, file_name } => {
            let version = resolve_http_version(client, package_id, version_rule)?;
            let file_name = file_name
                .clone()
                .or_else(|| file_name_from_url(url))
                .ok_or_else(|| RabbitError::RemoteData {
                    url: url.clone(),
                    message: "fixed artifact URL has no file name".to_string(),
                })?;
            Ok(ArtifactDescriptor {
                package_id: package_id.to_string(),
                version,
                platform,
                architecture: target.report_arch,
                kind,
                url: url.clone(),
                file_name,
                channel: None,
            })
        }
    }
}

/// Version for a scrape/fixed `http_artifact` source: the package
/// `VersionRule`. These sources store the version on the descriptor only (it
/// is never interpolated), so a source without a version rule is a manifest
/// error.
fn resolve_http_version(
    client: &Client,
    package_id: &str,
    version_rule: Option<&VersionRule>,
) -> Result<Version> {
    let rule = version_rule.ok_or_else(|| RabbitError::RemoteData {
        url: String::new(),
        message: format!(
            "http_artifact package {package_id} has no version rule for its scrape/fixed source"
        ),
    })?;
    resolve_version_rule(client, rule)
}

/// Resolve the download for a data-driven HFS-listing package (JAWS): read the
/// rejetto-HFS folder listing, pick the highest-version installer, and build
/// its file URL. The version side ([`crate::latest::resolve_hfs_listing_version`])
/// reads the same listing; both share [`fetch_file_list`] +
/// [`pick_jaws_for_reaper_version`]. JAWS is universal (one Windows installer),
/// so the reported arch is always `Universal`.
fn resolve_hfs_artifact(
    client: &Client,
    package_id: &str,
    spec: &crate::package::HfsListingSpec,
    platform: Platform,
) -> Result<ArtifactDescriptor> {
    let entries = fetch_file_list(client, &spec.base, &spec.folder)?;
    let (version, file_name) =
        pick_jaws_for_reaper_version(&entries).ok_or_else(|| RabbitError::NoArtifactFound {
            package_id: package_id.to_string(),
            platform,
            architecture: Architecture::Universal,
        })?;
    let url = hfs_file_url(&spec.base, &spec.folder, &file_name);
    Ok(ArtifactDescriptor {
        package_id: package_id.to_string(),
        version,
        platform,
        architecture: Architecture::Universal,
        kind: github_artifact_kind(spec.artifact_kind),
        url,
        file_name,
        channel: None,
    })
}

fn artifact_from_href(
    package_id: &str,
    version: Version,
    platform: Platform,
    architecture: Architecture,
    kind: ArtifactKind,
    base_url: &str,
    href: &str,
) -> Result<ArtifactDescriptor> {
    let url = absolute_url(base_url, href);
    let file_name = file_name_from_url(&url).ok_or_else(|| RabbitError::RemoteData {
        url: url.clone(),
        message: "artifact URL does not contain a file name".to_string(),
    })?;

    Ok(ArtifactDescriptor {
        package_id: package_id.to_string(),
        version,
        platform,
        architecture,
        kind,
        url,
        file_name,
        channel: None,
    })
}

/// Client for metadata fetches (release JSON, scrape pages). Keeps
/// `reqwest::blocking`'s default 30-second per-operation timeout — these are
/// small responses and a hung fetch should fail fast.
fn http_client() -> Result<Client> {
    crate::http::client_builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|source| RabbitError::Http {
            url: "client-builder".to_string(),
            source,
        })
}

/// Download `url` into `part_path` with the standard download client and
/// retry/resume policy. Shared with self-update's binary download so every
/// large download in RABBIT gets the same stall tolerance, retries, and
/// network-vs-disk error classification.
pub(crate) fn download_url_with_retries(
    url: &str,
    part_path: &Path,
    package_id: &str,
    progress: &ProgressReporter,
) -> Result<()> {
    fetch_remote_artifact_with_retries(
        &download_http_client()?,
        url,
        part_path,
        package_id,
        progress,
        &DOWNLOAD_RETRY_DELAYS,
        &CancelToken::new(),
    )
}

/// Client for artifact (and self-update) downloads: a more generous stall
/// timeout than the blocking client's 30-second default, which killed slow
/// large downloads mid-body (see [`DOWNLOAD_STALL_TIMEOUT`]). The blocking
/// client's `timeout` is per read/write operation, so this bounds how long a
/// download may sit with NO bytes arriving — it does not cap total duration.
fn download_http_client() -> Result<Client> {
    crate::http::client_builder()
        .user_agent(USER_AGENT)
        .timeout(DOWNLOAD_STALL_TIMEOUT)
        .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
        .build()
        .map_err(|source| RabbitError::Http {
            url: "client-builder".to_string(),
            source,
        })
}

fn http_get_text(client: &Client, url: &str) -> Result<String> {
    let request = crate::http::maybe_apply_github_auth(client.get(url), url);
    let response = request
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|source| RabbitError::Http {
            url: url.to_string(),
            source,
        })?;

    response.text().map_err(|source| RabbitError::Http {
        url: url.to_string(),
        source,
    })
}

fn find_href_with(body: &str, predicate: impl Fn(&str, &str) -> bool) -> Option<String> {
    let mut offset = 0;
    while let Some(relative_start) = body[offset..].find("href=") {
        let href_start = offset + relative_start + "href=".len();
        let quote = body.as_bytes().get(href_start).copied()?;
        if quote != b'\'' && quote != b'"' {
            offset = href_start;
            continue;
        }

        let value_start = href_start + 1;
        let value_end = body[value_start..]
            .find(quote as char)
            .map(|relative_end| value_start + relative_end)?;
        let href = &body[value_start..value_end];
        let context_end = body.len().min(value_end + 400);
        let context = &body[value_end..context_end];

        if predicate(href, context) {
            return Some(decode_basic_entities(href));
        }

        offset = value_end + 1;
    }

    None
}

fn absolute_url(base_url: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else {
        format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            href.trim_start_matches('/')
        )
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

fn decode_basic_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn local_artifact_source_path(url_or_path: &str) -> Result<Option<PathBuf>> {
    if let Some(rest) = url_or_path.strip_prefix("file://") {
        return file_url_path(rest).map(Some);
    }

    let path = PathBuf::from(url_or_path);
    if path.is_file() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

fn validate_remote_artifact_url(url: &str) -> Result<()> {
    if url.starts_with("https://") {
        return Ok(());
    }
    // Loopback is exempt from the HTTPS requirement: traffic to 127.0.0.1
    // never leaves the machine, and it lets integration tests exercise the
    // real download pipeline against a local server.
    if is_loopback_http_url(url) {
        return Ok(());
    }

    let message = if url.contains("://") {
        "remote artifact downloads must use HTTPS"
    } else {
        "artifact URL is neither an existing local file nor an HTTPS URL"
    };
    Err(RabbitError::InvalidArtifactUrl {
        url: url.to_string(),
        message: message.to_string(),
    })
}

/// Whether `url` is plain HTTP to the IPv4 loopback host — and ONLY to it.
/// Parses the RFC 3986 authority instead of prefix-matching so a crafted
/// userinfo (`http://127.0.0.1:80@evil.example/…` — host `evil.example`)
/// cannot smuggle a cleartext download to a non-loopback host past the
/// HTTPS-only rule.
fn is_loopback_http_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.contains('@') {
        return false;
    }
    match authority.split_once(':') {
        None => authority == "127.0.0.1",
        Some((host, port)) => {
            host == "127.0.0.1" && !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
        }
    }
}

fn file_url_path(rest: &str) -> Result<PathBuf> {
    let without_host = rest.strip_prefix("localhost/").unwrap_or(rest);
    let decoded = percent_decode_file_url_path(without_host)?;
    let path = if cfg!(windows) {
        let windows_path = decoded
            .strip_prefix('/')
            .filter(|path| path.as_bytes().get(1) == Some(&b':'))
            .unwrap_or(&decoded);
        PathBuf::from(windows_path.replace('/', "\\"))
    } else {
        PathBuf::from(format!("/{}", decoded.trim_start_matches('/')))
    };
    Ok(path)
}

fn percent_decode_file_url_path(input: &str) -> Result<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(hex) = bytes.get(index + 1..index + 3) else {
                return Err(invalid_file_url(input));
            };
            let hex = std::str::from_utf8(hex).map_err(|_| invalid_file_url(input))?;
            let value = u8::from_str_radix(hex, 16).map_err(|_| invalid_file_url(input))?;
            output.push(value);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(output).map_err(|_| invalid_file_url(input))
}

fn invalid_file_url(input: &str) -> RabbitError {
    RabbitError::RemoteData {
        url: format!("file://{input}"),
        message: "invalid file URL path encoding".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    use crate::package::{
        AssetSelector, GithubArtifactKind, GithubReleaseSelector, GithubReleaseSpec, HrefMatch,
        HttpArtifactSource, HttpArtifactSpec, HttpArtifactTarget, InstallDestination,
        PACKAGE_APP2CLAP, PACKAGE_FFMPEG, PACKAGE_LB_303, PACKAGE_LJUNO_116, PACKAGE_LR_608,
        PACKAGE_LREAPARAMS, PACKAGE_LSAMPLER_24, PACKAGE_OSARA, PACKAGE_REAKONTROL,
        PACKAGE_REAPACK, PACKAGE_REAPER, PACKAGE_SURGE_XT, PACKAGE_SWS, SupportedPlatform,
        VersionSource,
    };
    use tempfile::tempdir;

    /// One scripted connection: the raw response head to send, the body
    /// bytes to send (possibly fewer than the head's Content-Length — a
    /// mid-stream drop), and an optional stall before the final byte.
    struct ScriptedResponse {
        head: String,
        body: Vec<u8>,
        stall_before_last_byte: Option<Duration>,
    }

    /// Local HTTP server that serves one scripted response per connection,
    /// capturing each request's raw head so tests can assert on Range /
    /// If-Range headers. Closes each connection after its scripted body.
    fn spawn_scripted_server(
        responses: Vec<ScriptedResponse>,
    ) -> (
        String,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = requests.clone();
        let handle = thread::spawn(move || {
            for scripted in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut request = vec![0u8; 4096];
                let read = stream.read(&mut request).unwrap_or(0);
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&request[..read]).to_string());
                if stream.write_all(scripted.head.as_bytes()).is_err() {
                    continue;
                }
                let body = &scripted.body;
                if let Some(stall) = scripted.stall_before_last_byte
                    && body.len() > 1
                {
                    if stream.write_all(&body[..body.len() - 1]).is_err() {
                        continue;
                    }
                    let _ = stream.flush();
                    thread::sleep(stall);
                    let _ = stream.write_all(&body[body.len() - 1..]);
                } else {
                    let _ = stream.write_all(body);
                }
                let _ = stream.flush();
                // connection drops here (FIN); a body shorter than the
                // declared Content-Length is a mid-stream drop.
            }
        });
        (format!("http://{addr}/file.bin"), requests, handle)
    }

    /// Position-dependent test payload so any resume-offset mistake
    /// (duplicated or missing bytes) breaks the content assertion, not
    /// just the length.
    fn patterned_body(len: usize) -> Vec<u8> {
        (0..len).map(|index| (index % 251) as u8).collect()
    }

    fn head_200(content_length: usize, etag: Option<&str>) -> String {
        let etag_line = etag
            .map(|value| format!("ETag: {value}\r\n"))
            .unwrap_or_default();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\n{etag_line}Connection: close\r\n\r\n"
        )
    }

    fn fetch_with_zero_delays(url: &str, part_path: &std::path::Path) -> Result<()> {
        fetch_remote_artifact_with_retries(
            &download_http_client().unwrap(),
            url,
            part_path,
            "test-package",
            &ProgressReporter::noop(),
            &[Duration::ZERO, Duration::ZERO, Duration::ZERO],
            &CancelToken::new(),
        )
    }

    #[test]
    fn resumes_interrupted_download_with_range_and_if_range() {
        let body = patterned_body(1000);
        let (url, requests, handle) = spawn_scripted_server(vec![
            // Connection 1: strong ETag, drops after 100 of 1000 bytes.
            ScriptedResponse {
                head: head_200(1000, Some("\"v1\"")),
                body: body[..100].to_vec(),
                stall_before_last_byte: None,
            },
            // Connection 2: honors the range with a 206 for the tail.
            ScriptedResponse {
                head: "HTTP/1.1 206 Partial Content\r\nContent-Length: 900\r\nContent-Range: bytes 100-999/1000\r\nConnection: close\r\n\r\n"
                    .to_string(),
                body: body[100..].to_vec(),
                stall_before_last_byte: None,
            },
        ]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        fetch_with_zero_delays(&url, &part_path).unwrap();
        handle.join().unwrap();

        assert_eq!(fs::read(&part_path).unwrap(), body);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let retry = requests[1].to_ascii_lowercase();
        assert!(
            retry.contains("range: bytes=100-"),
            "retry request: {retry}"
        );
        assert!(retry.contains("if-range: \"v1\""), "retry request: {retry}");
    }

    #[test]
    fn restarts_download_when_server_ignores_the_range() {
        let body = patterned_body(600);
        let (url, requests, handle) = spawn_scripted_server(vec![
            ScriptedResponse {
                head: head_200(600, Some("\"v1\"")),
                body: body[..200].to_vec(),
                stall_before_last_byte: None,
            },
            // Server ignores the Range (validator mismatch or no range
            // support) and replies 200 with the FULL body: the download
            // must restart from zero, not append.
            ScriptedResponse {
                head: head_200(600, Some("\"v2\"")),
                body: body.clone(),
                stall_before_last_byte: None,
            },
        ]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        fetch_with_zero_delays(&url, &part_path).unwrap();
        handle.join().unwrap();

        assert_eq!(fs::read(&part_path).unwrap(), body);
        assert_eq!(requests.lock().unwrap().len(), 2);
    }

    #[test]
    fn does_not_attempt_resume_without_a_validator() {
        let body = patterned_body(400);
        let (url, requests, handle) = spawn_scripted_server(vec![
            // No ETag / Last-Modified: a resume could splice two different
            // upstream files together, so the retry must restart from zero.
            ScriptedResponse {
                head: head_200(400, None),
                body: body[..50].to_vec(),
                stall_before_last_byte: None,
            },
            ScriptedResponse {
                head: head_200(400, None),
                body: body.clone(),
                stall_before_last_byte: None,
            },
        ]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        fetch_with_zero_delays(&url, &part_path).unwrap();
        handle.join().unwrap();

        assert_eq!(fs::read(&part_path).unwrap(), body);
        let requests = requests.lock().unwrap();
        assert!(
            !requests[1].to_ascii_lowercase().contains("range:"),
            "retry must not send Range without a validator: {}",
            requests[1]
        );
    }

    #[test]
    fn restarts_download_when_206_content_range_mismatches_offset() {
        let body = patterned_body(800);
        let (url, _requests, handle) = spawn_scripted_server(vec![
            ScriptedResponse {
                head: head_200(800, Some("\"v1\"")),
                body: body[..100].to_vec(),
                stall_before_last_byte: None,
            },
            // Buggy server: 206, but the range starts at the WRONG offset.
            // Appending it would corrupt the file; the client must restart.
            ScriptedResponse {
                head: "HTTP/1.1 206 Partial Content\r\nContent-Length: 750\r\nContent-Range: bytes 50-799/800\r\nConnection: close\r\n\r\n"
                    .to_string(),
                body: body[50..].to_vec(),
                stall_before_last_byte: None,
            },
            ScriptedResponse {
                head: head_200(800, Some("\"v1\"")),
                body: body.clone(),
                stall_before_last_byte: None,
            },
        ]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        fetch_with_zero_delays(&url, &part_path).unwrap();
        handle.join().unwrap();

        assert_eq!(fs::read(&part_path).unwrap(), body);
    }

    #[test]
    fn restarts_download_after_range_not_satisfiable() {
        let body = patterned_body(300);
        let (url, _requests, handle) = spawn_scripted_server(vec![
            ScriptedResponse {
                head: head_200(300, Some("\"v1\"")),
                body: body[..100].to_vec(),
                stall_before_last_byte: None,
            },
            // Upstream changed and rejects the resume offset outright.
            ScriptedResponse {
                head: "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
                body: Vec::new(),
                stall_before_last_byte: None,
            },
            ScriptedResponse {
                head: head_200(300, Some("\"v2\"")),
                body: body.clone(),
                stall_before_last_byte: None,
            },
        ]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        fetch_with_zero_delays(&url, &part_path).unwrap();
        handle.join().unwrap();

        assert_eq!(fs::read(&part_path).unwrap(), body);
    }

    #[test]
    fn reports_clear_download_interrupted_error_after_exhausted_retries() {
        // Every connection drops mid-body; after DOWNLOAD_MAX_ATTEMPTS the
        // failure must read as a network problem naming the URL, not as an
        // I/O error at the cache path.
        let body = patterned_body(500);
        let responses = (0..DOWNLOAD_MAX_ATTEMPTS)
            .map(|_| ScriptedResponse {
                head: head_200(500, Some("\"v1\"")),
                body: body[..10].to_vec(),
                stall_before_last_byte: None,
            })
            .collect();
        let (url, _requests, handle) = spawn_scripted_server(responses);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        let error = fetch_with_zero_delays(&url, &part_path).unwrap_err();
        handle.join().unwrap();

        assert!(
            matches!(error, RabbitError::DownloadInterrupted { .. }),
            "expected DownloadInterrupted, got: {error}"
        );
        let message = error.to_string();
        assert!(message.contains(&url), "error must name the URL: {message}");
        assert!(
            message.contains("check the internet connection"),
            "error must hint at the network: {message}"
        );
    }

    #[test]
    fn retries_transient_server_errors_without_discarding_resume_state() {
        let body = patterned_body(700);
        let (url, requests, handle) = spawn_scripted_server(vec![
            ScriptedResponse {
                head: head_200(700, Some("\"v1\"")),
                body: body[..250].to_vec(),
                stall_before_last_byte: None,
            },
            // Overloaded origin fronts a 503 on the resume attempt: must be
            // retried WITHOUT throwing away the 250 bytes already on disk.
            ScriptedResponse {
                head: "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_string(),
                body: Vec::new(),
                stall_before_last_byte: None,
            },
            ScriptedResponse {
                head: "HTTP/1.1 206 Partial Content\r\nContent-Length: 450\r\nContent-Range: bytes 250-699/700\r\nConnection: close\r\n\r\n"
                    .to_string(),
                body: body[250..].to_vec(),
                stall_before_last_byte: None,
            },
        ]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        fetch_with_zero_delays(&url, &part_path).unwrap();
        handle.join().unwrap();

        assert_eq!(fs::read(&part_path).unwrap(), body);
        let requests = requests.lock().unwrap();
        // Both the 503'd attempt and the successful one must still resume.
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("range: bytes=250-")
        );
        assert!(
            requests[2]
                .to_ascii_lowercase()
                .contains("range: bytes=250-")
        );
    }

    #[test]
    fn fails_fast_on_416_to_a_plain_get() {
        // A 416 to a request that sent NO Range header is not resume-related;
        // it must fail fast like any other 4xx, not spin through retries.
        let (url, requests, handle) = spawn_scripted_server(vec![ScriptedResponse {
            head: "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
            body: Vec::new(),
            stall_before_last_byte: None,
        }]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        let error = fetch_with_zero_delays(&url, &part_path).unwrap_err();
        drop(handle);

        assert!(matches!(error, RabbitError::Http { .. }), "got: {error}");
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    #[test]
    fn fails_immediately_on_http_error_status() {
        let (url, requests, handle) = spawn_scripted_server(vec![ScriptedResponse {
            head: "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
            body: Vec::new(),
            stall_before_last_byte: None,
        }]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        let error = fetch_with_zero_delays(&url, &part_path).unwrap_err();
        drop(handle);

        assert!(
            matches!(error, RabbitError::Http { .. }),
            "a 404 is not transient and must not be retried: {error}"
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    /// Regression for the reported FFmpeg install failure: with the
    /// metadata client's default 30-second per-operation timeout, a >30s
    /// mid-body stall was killed and surfaced as "error decoding response
    /// body". The download client + retry loop must ride it out.
    #[test]
    #[ignore = "takes ~35s of wall clock; run explicitly"]
    fn survives_a_midbody_stall_longer_than_thirty_seconds() {
        let body = patterned_body(200);
        let (url, _requests, handle) = spawn_scripted_server(vec![ScriptedResponse {
            head: head_200(200, Some("\"v1\"")),
            body: body.clone(),
            stall_before_last_byte: Some(Duration::from_secs(35)),
        }]);

        let dir = tempdir().unwrap();
        let part_path = dir.path().join("file.bin.part");
        fetch_with_zero_delays(&url, &part_path).unwrap();
        handle.join().unwrap();

        assert_eq!(fs::read(&part_path).unwrap(), body);
    }

    /// The app2clap manifest's `github_release` block as a literal, for the
    /// data-driven artifact-resolver tests. Its one x64 zip serves REAPER
    /// on Windows on ARM too, and says it is the x64 build there.
    fn app2clap_github_spec() -> GithubReleaseSpec {
        let zip = |arch| AssetSelector {
            platform: SupportedPlatform::Windows,
            arch: Some(arch),
            report_arch: (arch != Architecture::X64).then_some(Architecture::X64),
            name_prefix: Some("app2clap_".to_string()),
            name_suffix: Some(".zip".to_string()),
            exact_name: None,
            artifact_kind: None,
            ignore_case: false,
        };
        GithubReleaseSpec {
            repo: "jcsteh/app2clap".to_string(),
            release: GithubReleaseSelector::Tag("snapshots".to_string()),
            version_from: VersionSource::AssetName {
                strip_trailing_dot_segment: true,
            },
            assets: vec![
                zip(Architecture::X64),
                zip(Architecture::Arm64),
                zip(Architecture::Arm64Ec),
            ],
            artifact_kind: Some(GithubArtifactKind::Archive),
        }
    }

    /// ReaKontrol's `github_release` block: `/releases/latest`, per-platform
    /// asset prefixes, version from the asset name, no per-arch fan-out.
    fn reakontrol_github_spec() -> GithubReleaseSpec {
        GithubReleaseSpec {
            repo: "jcsteh/reaKontrol".to_string(),
            release: GithubReleaseSelector::Latest,
            version_from: VersionSource::AssetName {
                strip_trailing_dot_segment: true,
            },
            assets: vec![
                AssetSelector {
                    platform: SupportedPlatform::Windows,
                    arch: None,
                    report_arch: None,
                    name_prefix: Some("reaKontrol_windows_".to_string()),
                    name_suffix: Some(".zip".to_string()),
                    exact_name: None,
                    artifact_kind: None,
                    ignore_case: false,
                },
                AssetSelector {
                    platform: SupportedPlatform::Macos,
                    arch: None,
                    report_arch: None,
                    name_prefix: Some("reaKontrol_mac_".to_string()),
                    name_suffix: Some(".zip".to_string()),
                    exact_name: None,
                    artifact_kind: None,
                    ignore_case: false,
                },
            ],
            artifact_kind: Some(GithubArtifactKind::Archive),
        }
    }

    /// ReaPack's `github_release` block: `/releases/latest`, version from the
    /// `tag_name`, per-(platform,arch) exact-name `ExtensionBinary` assets.
    fn reapack_github_spec() -> GithubReleaseSpec {
        let exact = |platform, arch, name: &str| AssetSelector {
            platform,
            arch: Some(arch),
            report_arch: None,
            name_prefix: None,
            name_suffix: None,
            exact_name: Some(name.to_string()),
            artifact_kind: None,
            ignore_case: false,
        };
        GithubReleaseSpec {
            repo: "cfillion/reapack".to_string(),
            release: GithubReleaseSelector::Latest,
            version_from: VersionSource::TagName {
                strip_v_prefix: true,
            },
            assets: vec![
                exact(
                    SupportedPlatform::Windows,
                    Architecture::X64,
                    "reaper_reapack-x64.dll",
                ),
                exact(
                    SupportedPlatform::Macos,
                    Architecture::Arm64,
                    "reaper_reapack-arm64.dylib",
                ),
            ],
            artifact_kind: Some(GithubArtifactKind::ExtensionBinary),
        }
    }

    use super::*;

    #[test]
    fn finds_href_by_fragment() {
        let body = r#"<a href="download/featured/sws-2.14.0.7-Windows-x64.exe">Download</a>"#;
        let href = find_href_with(body, |href, _context| href.contains("Windows-x64.exe")).unwrap();
        assert_eq!(href, "download/featured/sws-2.14.0.7-Windows-x64.exe");
    }

    /// Surge XT's `github_release` block as a literal (mirrors
    /// `80-surge-xt.json`): the static `Nightly` tag, version from the asset
    /// name, per-asset Installer(win) / DiskImage(mac) kinds.
    fn surge_xt_github_spec() -> GithubReleaseSpec {
        GithubReleaseSpec {
            repo: "surge-synthesizer/surge".to_string(),
            release: GithubReleaseSelector::Tag("Nightly".to_string()),
            version_from: VersionSource::AssetName {
                strip_trailing_dot_segment: false,
            },
            assets: vec![
                AssetSelector {
                    platform: SupportedPlatform::Windows,
                    arch: None,
                    report_arch: None,
                    name_prefix: Some("surge-xt-win64-".to_string()),
                    name_suffix: Some("-setup.exe".to_string()),
                    exact_name: None,
                    artifact_kind: Some(GithubArtifactKind::Installer),
                    ignore_case: false,
                },
                AssetSelector {
                    platform: SupportedPlatform::Macos,
                    arch: None,
                    report_arch: None,
                    name_prefix: Some("surge-xt-macOS-".to_string()),
                    name_suffix: Some(".dmg".to_string()),
                    exact_name: None,
                    artifact_kind: Some(GithubArtifactKind::DiskImage),
                    ignore_case: false,
                },
            ],
            artifact_kind: None,
        }
    }

    const SURGE_XT_RELEASE_URL: &str =
        "https://api.github.com/repos/surge-synthesizer/surge/releases/tags/Nightly";

    fn reaper_x86_href_match() -> HrefMatch {
        HrefMatch {
            contains: vec!["-install.exe".to_string()],
            not_contains: vec!["_x64".to_string(), "arm64ec".to_string()],
            ends_with: Some("-install.exe".to_string()),
        }
    }

    #[test]
    fn href_match_disambiguates_reaper_x86_from_x64_and_arm64() {
        // REAPER's x86 fragment `-install.exe` is a SUBSTRING of the x64 and
        // arm64ec hrefs; the not_contains clauses make the match
        // order-INDEPENDENT (the bespoke code relied on document order).
        let x86 = reaper_x86_href_match();
        assert!(x86.matches("files/7.x/reaper776-install.exe"));
        assert!(!x86.matches("files/7.x/reaper776_x64-install.exe"));
        assert!(!x86.matches("files/7.x/reaper776_arm64ec-install.exe"));

        let x64 = HrefMatch {
            contains: vec!["_x64-install.exe".to_string()],
            not_contains: Vec::new(),
            ends_with: Some("-install.exe".to_string()),
        };
        assert!(x64.matches("files/7.x/reaper776_x64-install.exe"));
        assert!(!x64.matches("files/7.x/reaper776-install.exe"));
    }

    #[test]
    fn scrape_href_picks_reaper_x86_regardless_of_page_order() {
        // The x64 link appears FIRST in document order; the x86 matcher must
        // still skip it and land on the plain installer.
        let body = r#"
            <a href="files/7.x/reaper776_x64-install.exe">x64</a>
            <a href="files/7.x/reaper776-install.exe">x86</a>
            <a href="files/7.x/reaper776_arm64ec-install.exe">arm64ec</a>
        "#;
        let m = reaper_x86_href_match();
        let href = find_href_with(body, |href, _context| m.matches(href)).unwrap();
        assert_eq!(href, "files/7.x/reaper776-install.exe");
        let url = absolute_url("https://www.reaper.fm/", &href);
        assert_eq!(url, "https://www.reaper.fm/files/7.x/reaper776-install.exe");
        assert_eq!(file_name_from_url(&url).unwrap(), "reaper776-install.exe");
    }

    /// A synthetic Apache listing laid out like landoleet.org's: one build
    /// set, every platform's file, plus the Linux tarballs, the language-pack
    /// template and the changelog that must NOT be picked.
    fn landoleet_listing(build: &str) -> String {
        let names = [
            "old/".to_string(),
            format!("reaper{build}_win11_arm64ec_beta-install.exe"),
            format!("reaper{build}_x64-install.exe"),
            format!("reaper{build}-install.exe"),
            format!("reaper{build}_universal.dmg"),
            format!("reaper{build}_x86_64.dmg"),
            format!("reaper{build}_i386.dmg"),
            format!("reaper{build}_linux_x86_64.tar.xz"),
            format!("reaper{build}_linux_aarch64.tar.xz"),
            format!("reaper{build}.ReaperLangPack"),
            "whatsnew.txt".to_string(),
        ];
        names
            .iter()
            .map(|name| {
                format!(
                    "<tr><td valign=\"top\"><img src=\"/icons/binary.gif\" alt=\"[   ]\"></td>\
                     <td><a href=\"{name}\">{name}</a></td><td align=\"right\">2026-09-17 17:08  </td>\
                     <td align=\"right\"> 16M</td></tr>\n"
                )
            })
            .collect()
    }

    #[test]
    fn reaper_dev_channel_picks_the_right_landoleet_file_for_every_target() {
        let reaper = crate::package::embedded_package_manifest()
            .packages
            .into_iter()
            .find(|spec| spec.id == crate::package::PACKAGE_REAPER)
            .unwrap()
            .on_channel(Some("dev"));
        let body = landoleet_listing("780+dev0917");
        let expected = [
            (
                SupportedPlatform::Windows,
                Architecture::X86,
                "reaper780+dev0917-install.exe",
            ),
            (
                SupportedPlatform::Windows,
                Architecture::X64,
                "reaper780+dev0917_x64-install.exe",
            ),
            (
                SupportedPlatform::Windows,
                Architecture::Arm64Ec,
                "reaper780+dev0917_win11_arm64ec_beta-install.exe",
            ),
            (
                SupportedPlatform::Macos,
                Architecture::X86,
                "reaper780+dev0917_i386.dmg",
            ),
            (
                SupportedPlatform::Macos,
                Architecture::Universal,
                "reaper780+dev0917_universal.dmg",
            ),
        ];
        let targets = &reaper.http_artifact.as_ref().unwrap().targets;
        assert_eq!(
            targets.len(),
            expected.len(),
            "every stable target has a dev twin"
        );
        for (platform, arch, file) in expected {
            let target = targets
                .iter()
                .find(|target| target.platform == platform && target.report_arch == arch)
                .unwrap_or_else(|| panic!("no dev target for {platform:?}/{arch:?}"));
            let HttpArtifactSource::ScrapeHref {
                base_url,
                href_match,
                ..
            } = &target.source
            else {
                panic!("the dev channel scrapes landoleet");
            };
            let href = find_href_with(&body, |href, _| href_match.matches(href))
                .unwrap_or_else(|| panic!("nothing matched for {platform:?}/{arch:?}"));
            assert_eq!(href, file, "{platform:?}/{arch:?}");
            // The '+' in development build names must survive into the URL
            // and the cached file name untouched.
            let url = absolute_url(base_url, &href);
            assert_eq!(url, format!("https://www.landoleet.org/{file}"));
            assert_eq!(file_name_from_url(&url).unwrap(), file);
        }
    }

    #[test]
    fn reaper_dev_channel_reads_the_version_reaper_itself_reports() {
        let reaper = crate::package::embedded_package_manifest()
            .packages
            .into_iter()
            .find(|spec| spec.id == crate::package::PACKAGE_REAPER)
            .unwrap()
            .on_channel(Some("dev"));
        let Some(VersionRule::Html {
            url,
            pattern,
            format,
        }) = &reaper.version
        else {
            panic!("the dev channel reads its version from the landoleet listing");
        };
        // landoleet cycles through development builds, release candidates and
        // releases. The first case is byte-for-byte the FileVersion a real
        // reaper780+dev0917 reaper.exe reports (checked on Windows), which is
        // what exact comparison against the installed copy relies on.
        for (build, version) in [
            ("780+dev0917", "7.80+dev0917"),
            ("780rc2", "7.80rc2"),
            ("781", "7.81"),
        ] {
            let body = landoleet_listing(build);
            let parsed = crate::latest::resolve_html_version(&body, url, pattern, format).unwrap();
            assert_eq!(parsed.raw(), version, "build {build}");
        }
    }

    #[test]
    fn select_http_target_matches_platform_and_arch() {
        let spec = HttpArtifactSpec {
            targets: vec![
                HttpArtifactTarget {
                    platform: SupportedPlatform::Windows,
                    match_arches: vec![Architecture::X64, Architecture::Unknown],
                    report_arch: Architecture::X64,
                    artifact_kind: GithubArtifactKind::Installer,
                    source: HttpArtifactSource::FixedUrl {
                        url: "https://example.test/win-x64.exe".to_string(),
                        file_name: None,
                    },
                },
                HttpArtifactTarget {
                    platform: SupportedPlatform::Macos,
                    match_arches: vec![Architecture::Arm64],
                    report_arch: Architecture::Arm64,
                    artifact_kind: GithubArtifactKind::DiskImage,
                    source: HttpArtifactSource::FixedUrl {
                        url: "https://example.test/mac-arm64.dmg".to_string(),
                        file_name: None,
                    },
                },
            ],
        };
        assert_eq!(
            select_http_target(&spec, Platform::Windows, Architecture::X64)
                .unwrap()
                .report_arch,
            Architecture::X64
        );
        // No Windows-arm target → None: the package has no build for it.
        assert!(select_http_target(&spec, Platform::Windows, Architecture::Arm64).is_none());
        assert!(select_http_target(&spec, Platform::MacOs, Architecture::X64).is_none());
        assert!(select_http_target(&spec, Platform::MacOs, Architecture::Arm64).is_some());
    }

    #[test]
    fn resolves_relative_urls() {
        assert_eq!(
            absolute_url("https://sws-extension.org/", "download/file.exe"),
            "https://sws-extension.org/download/file.exe"
        );
    }

    #[test]
    fn extracts_file_names_from_urls() {
        assert_eq!(
            file_name_from_url("https://example.test/files/reaper.exe?download=1").unwrap(),
            "reaper.exe"
        );
    }

    #[test]
    fn caches_existing_local_path_artifact() {
        let source_dir = tempdir().unwrap();
        let source_path = source_dir.path().join("osara-test.exe");
        fs::write(&source_path, b"local installer bytes").unwrap();

        let cache_dir = tempdir().unwrap();
        let artifact = ArtifactDescriptor {
            package_id: PACKAGE_OSARA.to_string(),
            version: Version::parse("1.2.3").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: source_path.display().to_string(),
            file_name: "osara-test.exe".to_string(),
            channel: None,
        };

        let cached = download_artifacts(std::slice::from_ref(&artifact), cache_dir.path()).unwrap();
        assert_eq!(cached.len(), 1);
        assert!(!cached[0].reused_existing_file);
        assert_eq!(fs::read(&cached[0].path).unwrap(), b"local installer bytes");

        let cached_again = download_artifacts(&[artifact], cache_dir.path()).unwrap();
        assert!(cached_again[0].reused_existing_file);
    }

    /// The download pool must overlap: a fast artifact's handle resolves
    /// while a slow artifact is still mid-stream. The slow server holds the
    /// second half of its body hostage until the test observed the fast
    /// artifact's completion — deterministic proof of concurrency (with a
    /// 60s watchdog so a serialization regression fails the elapsed-time
    /// assert instead of hanging the suite).
    #[test]
    fn download_pool_overlaps_slow_and_fast_artifacts() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (gate_tx, gate_rx) = std::sync::mpsc::channel::<()>();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            stream.write_all(&[b's'; 50]).unwrap();
            let _ = stream.flush();
            // Hold the tail until the fast artifact completed (watchdog:
            // release after 60s so a regression can't hang the suite).
            let _ = gate_rx.recv_timeout(Duration::from_secs(60));
            let _ = stream.write_all(&[b's'; 50]);
            let _ = stream.flush();
        });

        let source_dir = tempdir().unwrap();
        let fast_source = source_dir.path().join("fast.bin");
        fs::write(&fast_source, b"fast artifact bytes").unwrap();

        let slow = ArtifactDescriptor {
            package_id: "slow-package".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: format!("http://{addr}/slow.bin"),
            file_name: "slow.bin".to_string(),
            channel: None,
        };
        let fast = ArtifactDescriptor {
            package_id: "fast-package".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: fast_source.display().to_string(),
            file_name: "fast.bin".to_string(),
            channel: None,
        };

        let cache_dir = tempdir().unwrap();
        let started = std::time::Instant::now();
        let (_pool, handles) = spawn_download_pool(
            &[slow, fast],
            cache_dir.path(),
            &ProgressReporter::noop(),
            &CancelToken::new(),
        );
        let mut handles = handles.into_iter();
        let slow_handle = handles.next().unwrap();
        let fast_handle = handles.next().unwrap();

        // The fast artifact must complete while the slow one is still
        // gated mid-stream — well under the 60s watchdog.
        let fast_cached = fast_handle.wait().unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "fast artifact should not have waited behind the slow one"
        );
        assert_eq!(fs::read(&fast_cached.path).unwrap(), b"fast artifact bytes");

        // Only now release the slow download's tail.
        let _ = gate_tx.send(());
        let slow_cached = slow_handle.wait().unwrap();
        assert_eq!(fs::read(&slow_cached.path).unwrap().len(), 100);
        server.join().unwrap();
    }

    /// A package id repeated in one batch must not race two workers onto
    /// the same cache `.part` path: duplicates share one download job and
    /// every handle still resolves.
    #[test]
    fn download_pool_deduplicates_identical_artifacts() {
        let source_dir = tempdir().unwrap();
        let source_path = source_dir.path().join("dup.bin");
        fs::write(&source_path, b"deduplicated artifact bytes").unwrap();
        let artifact = ArtifactDescriptor {
            package_id: "dup-package".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: source_path.display().to_string(),
            file_name: "dup.bin".to_string(),
            channel: None,
        };

        let cache_dir = tempdir().unwrap();
        let (_pool, handles) = spawn_download_pool(
            &[artifact.clone(), artifact],
            cache_dir.path(),
            &ProgressReporter::noop(),
            &CancelToken::new(),
        );
        for handle in handles {
            let cached = handle.wait().unwrap();
            assert_eq!(
                fs::read(&cached.path).unwrap(),
                b"deduplicated artifact bytes"
            );
        }
    }

    #[test]
    fn loopback_exemption_rejects_userinfo_smuggling() {
        // Genuine loopback passes…
        assert!(validate_remote_artifact_url("http://127.0.0.1:8080/file.bin").is_ok());
        assert!(validate_remote_artifact_url("http://127.0.0.1/file.bin").is_ok());
        // …but a crafted userinfo (host = evil.example) must not.
        assert!(
            validate_remote_artifact_url("http://127.0.0.1:80@evil.example/payload.exe").is_err()
        );
        assert!(validate_remote_artifact_url("http://127.0.0.1.evil.example/x.bin").is_err());
        assert!(validate_remote_artifact_url("http://127.0.0.12/x.bin").is_err());
    }

    #[test]
    fn caches_file_url_artifact() {
        let source_dir = tempdir().unwrap();
        let source_path = source_dir.path().join("osara test.exe");
        fs::write(&source_path, b"file url installer bytes").unwrap();

        let cache_dir = tempdir().unwrap();
        let artifact = ArtifactDescriptor {
            package_id: PACKAGE_OSARA.to_string(),
            version: Version::parse("1.2.3").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: file_url_for_test(&source_path),
            file_name: "osara-test.exe".to_string(),
            channel: None,
        };

        let cached = download_artifacts(&[artifact], cache_dir.path()).unwrap();
        assert_eq!(
            fs::read(&cached[0].path).unwrap(),
            b"file url installer bytes"
        );
    }

    #[test]
    fn rejects_non_https_remote_artifacts() {
        let cache_dir = tempdir().unwrap();
        let artifact = ArtifactDescriptor {
            package_id: PACKAGE_OSARA.to_string(),
            version: Version::parse("1.2.3").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: "http://example.test/osara-test.exe".to_string(),
            file_name: "osara-test.exe".to_string(),
            channel: None,
        };

        let error = download_artifacts(&[artifact], cache_dir.path()).unwrap_err();
        assert!(error.to_string().contains("HTTPS"));
    }

    #[test]
    fn macos_universal_arch_canonicalizes_to_disk_image_kinds_for_per_arch_packages() {
        // Regression: a universal REAPER install on macOS used to surface
        // `Architecture::Universal` to per-arch resolvers (SWS, ReaPack)
        // that didn't list a Universal arm, producing
        // "no artifact found for sws on MacOs/Universal". The dispatch
        // canonicalizes Universal to the host slice so the per-arch arms
        // match. Asserting `DiskImage` / `ExtensionBinary` (rather than a
        // specific architecture) keeps the test platform-agnostic — the
        // host slice differs between Apple Silicon and Intel, but the
        // artifact kind doesn't.
        assert_eq!(
            expected_artifact_kind(PACKAGE_SWS, Platform::MacOs, Architecture::Universal).unwrap(),
            ArtifactKind::DiskImage
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_REAPACK, Platform::MacOs, Architecture::Universal)
                .unwrap(),
            ArtifactKind::ExtensionBinary
        );
    }

    #[test]
    fn dispatch_arch_canonicalizes_unknown_to_host_slice() {
        // Regression for the macOS bug where a fresh first-time install
        // (no REAPER.app on disk yet → probe returns Unknown) routed SWS
        // and ReaPack through their `Unknown → X64` fallback arms, producing
        // x86_64 artifacts on Apple Silicon hosts where REAPER would run as
        // arm64. Same pattern existed on Windows-on-ARM. The dispatch now
        // collapses Unknown to the host slice for every platform, matching
        // what the upcoming install will actually land.
        let host = canonicalize_dispatch_arch(Architecture::Unknown);
        assert_ne!(
            host,
            Architecture::Unknown,
            "Unknown must be rewritten before per-arch resolvers see it"
        );
        assert_ne!(
            host,
            Architecture::Universal,
            "Universal is itself a sentinel — must collapse further"
        );
        // Identical canonicalization for the Universal sentinel keeps
        // the two paths in lockstep.
        assert_eq!(
            host,
            canonicalize_dispatch_arch(Architecture::Universal),
            "Unknown and Universal must canonicalize identically"
        );
    }

    #[test]
    fn reports_expected_artifact_kind_for_builtin_packages() {
        assert_eq!(
            expected_artifact_kind(PACKAGE_REAPER, Platform::Windows, Architecture::X64).unwrap(),
            ArtifactKind::Installer
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_OSARA, Platform::MacOs, Architecture::Arm64).unwrap(),
            ArtifactKind::Archive
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_SWS, Platform::MacOs, Architecture::X64).unwrap(),
            ArtifactKind::DiskImage
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_REAPACK, Platform::Windows, Architecture::X64).unwrap(),
            ArtifactKind::ExtensionBinary
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_REAKONTROL, Platform::Windows, Architecture::X64)
                .unwrap(),
            ArtifactKind::Archive
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_REAKONTROL, Platform::MacOs, Architecture::Arm64)
                .unwrap(),
            ArtifactKind::Archive
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_FFMPEG, Platform::Windows, Architecture::X64).unwrap(),
            ArtifactKind::SevenZipArchive
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_FFMPEG, Platform::Windows, Architecture::Arm64).unwrap(),
            ArtifactKind::SevenZipArchive
        );
        assert!(matches!(
            expected_artifact_kind(PACKAGE_FFMPEG, Platform::MacOs, Architecture::Arm64),
            Err(RabbitError::NoArtifactFound { .. })
        ));
        assert_eq!(
            expected_artifact_kind(PACKAGE_SURGE_XT, Platform::Windows, Architecture::X64).unwrap(),
            ArtifactKind::Installer
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_SURGE_XT, Platform::Windows, Architecture::Arm64)
                .unwrap(),
            ArtifactKind::Installer
        );
        assert_eq!(
            expected_artifact_kind(PACKAGE_SURGE_XT, Platform::MacOs, Architecture::Arm64).unwrap(),
            ArtifactKind::DiskImage
        );
    }

    #[test]
    fn resolves_surge_xt_nightly_installer_for_platform() {
        let body = r#"{
            "tag_name": "Nightly",
            "assets": [
                {
                    "name": "surge-xt-linux-arm64-NIGHTLY-2026-05-05-a87bdb7.tar.gz",
                    "browser_download_url": "https://github.com/surge-synthesizer/surge/releases/download/Nightly/surge-xt-linux-arm64-NIGHTLY-2026-05-05-a87bdb7.tar.gz"
                },
                {
                    "name": "surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-pluginsonly.zip",
                    "browser_download_url": "https://github.com/surge-synthesizer/surge/releases/download/Nightly/surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-pluginsonly.zip"
                },
                {
                    "name": "surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-setup.exe",
                    "browser_download_url": "https://github.com/surge-synthesizer/surge/releases/download/Nightly/surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-setup.exe"
                },
                {
                    "name": "surge-xt-macOS-NIGHTLY-2026-05-05-a87bdb7.dmg",
                    "browser_download_url": "https://github.com/surge-synthesizer/surge/releases/download/Nightly/surge-xt-macOS-NIGHTLY-2026-05-05-a87bdb7.dmg"
                }
            ]
        }"#;

        let spec = surge_xt_github_spec();
        let windows = resolve_github_artifact_from_release_body(
            body,
            SURGE_XT_RELEASE_URL,
            PACKAGE_SURGE_XT,
            &spec,
            Platform::Windows,
            Architecture::X64,
        )
        .unwrap();
        assert_eq!(windows.package_id, PACKAGE_SURGE_XT);
        assert_eq!(windows.kind, ArtifactKind::Installer);
        assert_eq!(windows.version.raw(), "NIGHTLY-2026-05-05-a87bdb7");
        assert_eq!(
            windows.file_name,
            "surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-setup.exe"
        );
        assert!(windows.url.ends_with("-setup.exe"));
        assert_eq!(windows.architecture, Architecture::Universal);

        // arm64 / arm64-ec REAPER hosts route through the same x64 setup
        // (Windows-on-arm runs the x64 installer under emulation; the Windows
        // selector pins no arch, so any arch resolves the same asset).
        let arm64 = resolve_github_artifact_from_release_body(
            body,
            SURGE_XT_RELEASE_URL,
            PACKAGE_SURGE_XT,
            &spec,
            Platform::Windows,
            Architecture::Arm64,
        )
        .unwrap();
        assert_eq!(arm64.file_name, windows.file_name);

        let mac = resolve_github_artifact_from_release_body(
            body,
            SURGE_XT_RELEASE_URL,
            PACKAGE_SURGE_XT,
            &spec,
            Platform::MacOs,
            Architecture::Arm64,
        )
        .unwrap();
        assert_eq!(mac.kind, ArtifactKind::DiskImage);
        assert_eq!(
            mac.file_name,
            "surge-xt-macOS-NIGHTLY-2026-05-05-a87bdb7.dmg"
        );
        assert!(mac.url.ends_with(".dmg"));
    }

    #[test]
    fn rejects_surge_xt_release_without_platform_asset() {
        let body = r#"{
            "tag_name": "Nightly",
            "assets": [
                {"name": "surge-xt-linux-x86_64-NIGHTLY-2026-05-05-a87bdb7.tar.gz"}
            ]
        }"#;
        let error = resolve_github_artifact_from_release_body(
            body,
            SURGE_XT_RELEASE_URL,
            PACKAGE_SURGE_XT,
            &surge_xt_github_spec(),
            Platform::Windows,
            Architecture::X64,
        )
        .unwrap_err();
        assert!(matches!(error, RabbitError::NoArtifactFound { .. }));
    }

    #[test]
    fn resolves_reakontrol_archive_for_platform() {
        let body = r#"{
            "tag_name": "snapshots",
            "assets": [
                {
                    "name": "reaKontrol_windows_2025.6.6.7.bfbe7606.zip",
                    "browser_download_url": "https://github.com/jcsteh/reaKontrol/releases/download/snapshots/reaKontrol_windows_2025.6.6.7.bfbe7606.zip"
                },
                {
                    "name": "reaKontrol_windows_2026.2.16.100.cafef00d.zip",
                    "browser_download_url": "https://github.com/jcsteh/reaKontrol/releases/download/snapshots/reaKontrol_windows_2026.2.16.100.cafef00d.zip"
                },
                {
                    "name": "reaKontrol_mac_2026.2.16.100.cafef00d.zip",
                    "browser_download_url": "https://github.com/jcsteh/reaKontrol/releases/download/snapshots/reaKontrol_mac_2026.2.16.100.cafef00d.zip"
                }
            ]
        }"#;

        let spec = reakontrol_github_spec();
        let url = "https://api.github.com/repos/jcsteh/reaKontrol/releases/latest";
        let windows = resolve_github_artifact_from_release_body(
            body,
            url,
            PACKAGE_REAKONTROL,
            &spec,
            Platform::Windows,
            Architecture::X64,
        )
        .unwrap();
        assert_eq!(windows.kind, ArtifactKind::Archive);
        assert_eq!(windows.version.raw(), "2026.2.16.100");
        assert_eq!(
            windows.file_name,
            "reaKontrol_windows_2026.2.16.100.cafef00d.zip"
        );
        assert!(
            windows
                .url
                .starts_with("https://github.com/jcsteh/reaKontrol/")
        );
        // No `arch` on the selector → reported Universal, as before.
        assert_eq!(windows.architecture, Architecture::Universal);

        let mac = resolve_github_artifact_from_release_body(
            body,
            url,
            PACKAGE_REAKONTROL,
            &spec,
            Platform::MacOs,
            Architecture::Arm64,
        )
        .unwrap();
        assert_eq!(mac.file_name, "reaKontrol_mac_2026.2.16.100.cafef00d.zip");
        assert_eq!(mac.version.raw(), "2026.2.16.100");
    }

    #[test]
    fn errors_when_reakontrol_release_has_no_matching_assets() {
        let body = r#"{"tag_name": "v0", "assets": []}"#;
        let error = resolve_github_artifact_from_release_body(
            body,
            "https://api.github.com/repos/jcsteh/reaKontrol/releases/latest",
            PACKAGE_REAKONTROL,
            &reakontrol_github_spec(),
            Platform::Windows,
            Architecture::X64,
        )
        .unwrap_err();
        assert!(matches!(error, RabbitError::NoArtifactFound { .. }));
    }

    #[test]
    fn resolves_reapack_extension_binary_by_exact_name_and_tag_version() {
        // ReaPack: version from tag_name (v stripped), per-(platform,arch)
        // exact-name asset, ExtensionBinary (no archive extraction).
        let body = r#"{
            "tag_name": "v1.2.6",
            "assets": [
                {
                    "name": "reaper_reapack-x64.dll",
                    "browser_download_url": "https://github.com/cfillion/reapack/releases/download/v1.2.6/reaper_reapack-x64.dll"
                },
                {
                    "name": "reaper_reapack-arm64.dylib",
                    "browser_download_url": "https://github.com/cfillion/reapack/releases/download/v1.2.6/reaper_reapack-arm64.dylib"
                }
            ]
        }"#;
        let spec = reapack_github_spec();
        let url = "https://api.github.com/repos/cfillion/reapack/releases/latest";
        let windows = resolve_github_artifact_from_release_body(
            body,
            url,
            PACKAGE_REAPACK,
            &spec,
            Platform::Windows,
            Architecture::X64,
        )
        .unwrap();
        assert_eq!(windows.kind, ArtifactKind::ExtensionBinary);
        assert_eq!(windows.version.raw(), "1.2.6");
        assert_eq!(windows.file_name, "reaper_reapack-x64.dll");
        assert_eq!(windows.architecture, Architecture::X64);

        let mac = resolve_github_artifact_from_release_body(
            body,
            url,
            PACKAGE_REAPACK,
            &spec,
            Platform::MacOs,
            Architecture::Arm64,
        )
        .unwrap();
        assert_eq!(mac.file_name, "reaper_reapack-arm64.dylib");
        assert_eq!(mac.version.raw(), "1.2.6");
        assert_eq!(mac.architecture, Architecture::Arm64);
    }

    #[test]
    fn resolves_app2clap_archive_from_snapshots_release() {
        // Assets out of version order on purpose — the resolver must pick the
        // highest, not the last (matches what the live `snapshots` tag does).
        let body = r#"{
            "tag_name": "snapshots",
            "assets": [
                {
                    "name": "app2clap_2025.11.27.30.ca402c1b.zip",
                    "browser_download_url": "https://github.com/jcsteh/app2clap/releases/download/snapshots/app2clap_2025.11.27.30.ca402c1b.zip"
                },
                {
                    "name": "app2clap_2026.5.17.34.b6f558cf.zip",
                    "browser_download_url": "https://github.com/jcsteh/app2clap/releases/download/snapshots/app2clap_2026.5.17.34.b6f558cf.zip"
                },
                {
                    "name": "app2clap_2026.5.16.31.5d1e4007.zip",
                    "browser_download_url": "https://github.com/jcsteh/app2clap/releases/download/snapshots/app2clap_2026.5.16.31.5d1e4007.zip"
                }
            ]
        }"#;

        let spec = app2clap_github_spec();
        let artifact = resolve_github_artifact_from_release_body(
            body,
            "https://api.github.com/repos/jcsteh/app2clap/releases/tags/snapshots",
            PACKAGE_APP2CLAP,
            &spec,
            Platform::Windows,
            Architecture::X64,
        )
        .unwrap();
        // Parity with the deleted bespoke resolver.
        assert_eq!(artifact.package_id, PACKAGE_APP2CLAP);
        assert_eq!(artifact.kind, ArtifactKind::Archive);
        assert_eq!(artifact.version.raw(), "2026.5.17.34");
        assert_eq!(artifact.file_name, "app2clap_2026.5.17.34.b6f558cf.zip");
        assert_eq!(artifact.architecture, Architecture::X64);
        assert!(artifact.url.ends_with("app2clap_2026.5.17.34.b6f558cf.zip"));
    }

    #[test]
    fn errors_when_app2clap_release_has_no_matching_assets() {
        let body = r#"{"tag_name": "snapshots", "assets": [{"name": "README.md"}]}"#;
        let error = resolve_github_artifact_from_release_body(
            body,
            "https://api.github.com/repos/jcsteh/app2clap/releases/tags/snapshots",
            PACKAGE_APP2CLAP,
            &app2clap_github_spec(),
            Platform::Windows,
            Architecture::X64,
        )
        .unwrap_err();
        assert!(matches!(error, RabbitError::NoArtifactFound { .. }));
    }

    /// LReaParams' release as GitHub's API lists it: the bare DLL beside an
    /// installer zip whose name changes from release to release. The
    /// manifest picks the DLL by its exact name, for an x64 REAPER and for
    /// REAPER's ARM64EC build alike, and reads the version from the tag.
    #[test]
    fn resolves_lreaparams_dll_and_version_from_its_latest_release() {
        let body = r#"{
            "tag_name": "v0.0.75",
            "name": "LReaParams 0.0.75 - Accessible Native Render Navigation",
            "draft": false,
            "prerelease": false,
            "assets": [
                {
                    "name": "LReaParams-0.0.75-Windows-x64-SETUP-EXE.11.zip",
                    "content_type": "application/x-zip-compressed",
                    "size": 2668354,
                    "browser_download_url": "https://github.com/Lo-lo78/LReaParams/releases/download/v0.0.75/LReaParams-0.0.75-Windows-x64-SETUP-EXE.11.zip"
                },
                {
                    "name": "reaper_lreaparams.dll",
                    "content_type": "application/x-msdownload",
                    "size": 1881088,
                    "browser_download_url": "https://github.com/Lo-lo78/LReaParams/releases/download/v0.0.75/reaper_lreaparams.dll"
                }
            ]
        }"#;
        let specs = crate::package::package_specs_by_id(Platform::Windows);
        let spec = specs[PACKAGE_LREAPARAMS].github_release.clone().unwrap();
        let url = crate::latest::github_release_url(&spec);
        assert_eq!(
            url,
            "https://api.github.com/repos/Lo-lo78/LReaParams/releases/latest"
        );
        assert_eq!(
            crate::latest::resolve_github_version(body, &url, &spec)
                .unwrap()
                .raw(),
            "0.0.75"
        );

        for architecture in [
            Architecture::X64,
            Architecture::Arm64,
            Architecture::Arm64Ec,
        ] {
            let artifact = resolve_github_artifact_from_release_body(
                body,
                &url,
                PACKAGE_LREAPARAMS,
                &spec,
                Platform::Windows,
                architecture,
            )
            .unwrap();
            assert_eq!(artifact.package_id, PACKAGE_LREAPARAMS);
            assert_eq!(artifact.kind, ArtifactKind::ExtensionBinary);
            assert_eq!(artifact.version.raw(), "0.0.75");
            assert_eq!(artifact.file_name, "reaper_lreaparams.dll");
            assert_eq!(
                artifact.url,
                "https://github.com/Lo-lo78/LReaParams/releases/download/v0.0.75/reaper_lreaparams.dll"
            );
            // The x64 DLL, which REAPER on Windows on ARM loads too.
            assert_eq!(artifact.architecture, Architecture::X64);
        }

        // A 32-bit REAPER can't load a 64-bit DLL, and there is no Mac build.
        for (platform, architecture) in [
            (Platform::Windows, Architecture::X86),
            (Platform::MacOs, Architecture::Arm64),
        ] {
            let error = resolve_github_artifact_from_release_body(
                body,
                &url,
                PACKAGE_LREAPARAMS,
                &spec,
                platform,
                architecture,
            )
            .unwrap_err();
            assert!(matches!(error, RabbitError::NoArtifactFound { .. }));
        }

        // A release without the DLL offers nothing, rather than the zip.
        let without_dll = r#"{
            "tag_name": "v0.0.76",
            "assets": [
                {
                    "name": "LReaParams-0.0.76-Windows-x64-SETUP-EXE.zip",
                    "browser_download_url": "https://github.com/Lo-lo78/LReaParams/releases/download/v0.0.76/LReaParams-0.0.76-Windows-x64-SETUP-EXE.zip"
                }
            ]
        }"#;
        let error = resolve_github_artifact_from_release_body(
            without_dll,
            &url,
            PACKAGE_LREAPARAMS,
            &spec,
            Platform::Windows,
            Architecture::X64,
        )
        .unwrap_err();
        assert!(matches!(error, RabbitError::NoArtifactFound { .. }));
    }

    /// Each of Lo-lo's instruments as GitHub's API lists its latest release:
    /// one zip, named by hand, LB-303's with `.VST3.zip` where the others
    /// have `.vst3.zip`, and LJuno-116's earlier ones as
    /// `LJuno-116-Windows-VST3.zip` and `LJuno-116.vst3.zip`. The manifest
    /// picks it by the product's name and `.zip` for an x64 REAPER and
    /// REAPER's builds for Windows on ARM alike, and reads the version from
    /// the tag without its `v`. Another instrument's zip, and a release
    /// without the zip, offer nothing.
    #[test]
    fn resolves_each_lolo_instrument_zip_and_reads_the_version_from_the_tag() {
        let specs = crate::package::package_specs_by_id(Platform::Windows);
        for (package_id, product, asset, tag) in [
            (
                PACKAGE_LJUNO_116,
                "LJuno-116",
                "LJuno-116.0.99.66.vst3.zip",
                "v0.99.66",
            ),
            (
                PACKAGE_LJUNO_116,
                "LJuno-116",
                "LJuno-116-Windows-VST3.zip",
                "v0.99.2",
            ),
            (
                PACKAGE_LJUNO_116,
                "LJuno-116",
                "LJuno-116.vst3.zip",
                "v0.99.3",
            ),
            (
                PACKAGE_LB_303,
                "LB-303",
                "LB-303.0.99.7.VST3.zip",
                "v0.99.7",
            ),
            (
                PACKAGE_LR_608,
                "LR-608",
                "LR-608.0.99.34.vst3.zip",
                "v0.99.34",
            ),
            (
                PACKAGE_LSAMPLER_24,
                "LSampler-24",
                "LSampler-24.0.99.29.vst3.zip",
                "v0.99.29",
            ),
        ] {
            let download =
                format!("https://github.com/Lo-lo78/{product}/releases/download/{tag}/{asset}");
            let body = serde_json::json!({
                "tag_name": tag,
                "name": format!("{product} {tag}"),
                "body": "What changed in this release.",
                "draft": false,
                "prerelease": false,
                "assets": [
                    {
                        "name": "LMixer-12.0.1.0.vst3.zip",
                        "browser_download_url": "https://github.com/Lo-lo78/LMixer-12/releases/download/v0.1.0/LMixer-12.0.1.0.vst3.zip"
                    },
                    {
                        "name": asset,
                        "content_type": "application/x-zip-compressed",
                        "browser_download_url": download
                    }
                ]
            })
            .to_string();
            let spec = specs[package_id].github_release.clone().unwrap();
            let url = crate::latest::github_release_url(&spec);
            assert_eq!(
                url,
                format!("https://api.github.com/repos/Lo-lo78/{product}/releases/latest")
            );
            assert_eq!(
                crate::latest::resolve_github_version(&body, &url, &spec)
                    .unwrap()
                    .raw(),
                tag.trim_start_matches('v')
            );

            for architecture in [
                Architecture::X64,
                Architecture::Arm64,
                Architecture::Arm64Ec,
            ] {
                let artifact = resolve_github_artifact_from_release_body(
                    &body,
                    &url,
                    package_id,
                    &spec,
                    Platform::Windows,
                    architecture,
                )
                .unwrap();
                assert_eq!(artifact.package_id, package_id);
                assert_eq!(artifact.kind, ArtifactKind::Archive);
                assert_eq!(artifact.version.raw(), tag.trim_start_matches('v'));
                assert_eq!(artifact.file_name, asset);
                assert_eq!(artifact.url, download);
                // The x64 plug-in, which REAPER on Windows on ARM loads too.
                assert_eq!(artifact.architecture, Architecture::X64);
            }

            let error = resolve_github_artifact_from_release_body(
                &body,
                &url,
                package_id,
                &spec,
                Platform::Windows,
                Architecture::X86,
            )
            .unwrap_err();
            assert!(matches!(error, RabbitError::NoArtifactFound { .. }));

            let without_zip = serde_json::json!({
                "tag_name": tag,
                "assets": [
                    {
                        "name": "LMixer-12.0.1.0.vst3.zip",
                        "browser_download_url": "https://github.com/Lo-lo78/LMixer-12/releases/download/v0.1.0/LMixer-12.0.1.0.vst3.zip"
                    }
                ]
            })
            .to_string();
            let error = resolve_github_artifact_from_release_body(
                &without_zip,
                &url,
                package_id,
                &spec,
                Platform::Windows,
                Architecture::X64,
            )
            .unwrap_err();
            assert!(matches!(error, RabbitError::NoArtifactFound { .. }));
        }
    }

    /// Every architecture the wizard and the CLI can hand over for a REAPER.
    const EVERY_ARCHITECTURE: [Architecture; 6] = [
        Architecture::X86,
        Architecture::X64,
        Architecture::Arm64,
        Architecture::Arm64Ec,
        Architecture::Universal,
        Architecture::Unknown,
    ];

    /// A release holding one asset for each of `spec`'s selectors, named so
    /// that selector matches it and its version can be read.
    fn release_with_every_asset(spec: &GithubReleaseSpec) -> String {
        let strip = matches!(
            spec.version_from,
            VersionSource::AssetName {
                strip_trailing_dot_segment: true
            }
        );
        let assets: Vec<serde_json::Value> = spec
            .assets
            .iter()
            .map(|selector| {
                let name = selector.exact_name.clone().unwrap_or_else(|| {
                    format!(
                        "{}1.2.3.4{}{}",
                        selector.name_prefix.as_deref().unwrap_or_default(),
                        if strip { ".cafef00d" } else { "" },
                        selector.name_suffix.as_deref().unwrap_or_default()
                    )
                });
                // A selector that ignores case finds its asset spelt the
                // other way round, as LB-303's `.VST3.zip` beside the other
                // instruments' `.vst3.zip`.
                let name = if selector.ignore_case {
                    swap_ascii_case(&name)
                } else {
                    name
                };
                serde_json::json!({
                    "name": name,
                    "browser_download_url": format!("https://example.test/{name}"),
                })
            })
            .collect();
        let tag = match spec.version_from {
            VersionSource::TagName {
                strip_v_prefix: true,
            } => "v1.2.3",
            _ => "1.2.3",
        };
        serde_json::json!({ "tag_name": tag, "assets": assets }).to_string()
    }

    /// `name` with every ASCII letter in the other case.
    fn swap_ascii_case(name: &str) -> String {
        name.chars()
            .map(|letter| {
                if letter.is_ascii_lowercase() {
                    letter.to_ascii_uppercase()
                } else {
                    letter.to_ascii_lowercase()
                }
            })
            .collect()
    }

    /// The ids of the packages offered on `platform`.
    fn package_ids_on(platform: Platform) -> Vec<String> {
        crate::package::builtin_package_specs(platform)
            .into_iter()
            .map(|spec| spec.id)
            .collect()
    }

    /// The offline rule and artifact resolution agree for every shipped
    /// package, on every channel, platform and architecture. Resolution's
    /// side runs its own selectors on manifest data: a release holding an
    /// asset for every `github_release` selector, the `http_artifact`
    /// targets' `match_arches`, and a pull request build's platform. Where
    /// the rule refuses, the resolver is asked as well, and refuses before it
    /// makes a request. Nothing goes online. Among the packages are
    /// selectors that ignore case, whose assets the release spells the other
    /// way round, whole plug-in bundles installed into `UserPlugins\FX`,
    /// app2clap installed there too, LReaParams, which RABBIT leaves to
    /// ReaPack where ReaPack installed it, and FFmpeg's and SWS's x64
    /// targets, which serve REAPER on Windows on ARM as well.
    #[test]
    fn the_offline_rule_agrees_with_artifact_resolution_for_every_shipped_package() {
        let windows = crate::package::package_specs_by_id(Platform::Windows);
        for id in [
            PACKAGE_LJUNO_116,
            PACKAGE_LB_303,
            PACKAGE_LR_608,
            PACKAGE_LSAMPLER_24,
        ] {
            let spec = &windows[id];
            assert!(spec.plugin_bundle.is_some(), "{id}");
            assert_eq!(spec.install_destination, InstallDestination::UserPluginsFx);
            let release = spec.github_release.as_ref().unwrap();
            assert!(release.assets.iter().all(|asset| asset.ignore_case), "{id}");
        }
        assert_eq!(
            windows[PACKAGE_APP2CLAP].install_destination,
            InstallDestination::UserPluginsFx
        );
        assert!(windows[PACKAGE_LREAPARAMS].leave_to_reapack);
        for id in [PACKAGE_FFMPEG, PACKAGE_SWS] {
            let http = windows[id].http_artifact.as_ref().unwrap();
            let x64 = select_http_target(http, Platform::Windows, Architecture::X64).unwrap();
            assert!(x64.match_arches.contains(&Architecture::Arm64Ec), "{id}");
        }

        for package in crate::package::embedded_package_manifest().packages {
            let channels = std::iter::once(None).chain(package.channels.iter().map(|channel| {
                Some(match channel.parameter {
                    Some(_) => format!("{}:1", channel.id),
                    None => channel.id.clone(),
                })
            }));
            for channel in channels {
                let on_channel = package.on_channel(channel.as_deref());
                for platform in [Platform::Windows, Platform::MacOs] {
                    let spec = on_channel.to_package_spec(platform);
                    if !on_channel.supports_platform(platform) {
                        assert!(
                            !package_has_build_for(&spec, platform, Architecture::X64),
                            "{} isn't offered on {platform:?}",
                            spec.id
                        );
                        continue;
                    }
                    for architecture in EVERY_ARCHITECTURE {
                        let label =
                            format!("{} on {channel:?}, {platform:?}, {architecture:?}", spec.id);
                        let dispatch = canonicalize_dispatch_arch(architecture);
                        if dispatch == Architecture::Unknown {
                            // Only on a computer RABBIT has no build for.
                            assert!(package_has_build_for(&spec, platform, architecture));
                            continue;
                        }
                        let source_resolves = if let Some(release) = &spec.github_release {
                            resolve_github_artifact_from_release_body(
                                &release_with_every_asset(release),
                                "https://example.test/release",
                                &spec.id,
                                release,
                                platform,
                                dispatch,
                            )
                            .is_ok()
                        } else if let Some(http) = &spec.http_artifact {
                            select_http_target(http, platform, dispatch).is_some()
                        } else if spec.hfs_listing.is_some() {
                            true
                        } else if let Some(actions) = &spec.github_actions_artifact {
                            crate::actions_artifact::target_for(actions, platform).is_some()
                        } else {
                            false
                        };
                        let resolves =
                            spec.supported_architectures.contains(&dispatch) && source_resolves;
                        assert_eq!(
                            package_has_build_for(&spec, platform, architecture),
                            resolves,
                            "{label}"
                        );
                        assert_eq!(
                            expected_artifact_kind_on(
                                &spec.id,
                                platform,
                                architecture,
                                channel.as_deref()
                            )
                            .is_ok(),
                            resolves,
                            "{label}"
                        );
                        // The resolver itself refuses what the rule refuses,
                        // before it makes a request.
                        if !resolves {
                            let channels: PackageChannels = channel
                                .iter()
                                .map(|channel| (spec.id.clone(), channel.clone()))
                                .collect();
                            let error = resolve_latest_artifacts_on(
                                std::slice::from_ref(&spec.id),
                                platform,
                                architecture,
                                &channels,
                            )
                            .unwrap_err();
                            assert!(
                                matches!(error, RabbitError::NoArtifactFound { .. }),
                                "{label}: {error}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// A 32-bit REAPER has no FFmpeg, Surge XT, app2clap, LReaParams or any
    /// of Lo-lo's four instruments, and has everything else. Resolution
    /// refuses those before it goes online, Surge XT too, whose Windows
    /// asset matches any architecture.
    #[test]
    fn a_32_bit_reaper_has_no_build_of_ffmpeg_surge_xt_app2clap_or_lolos_packages() {
        let without = packages_without_build(
            &package_ids_on(Platform::Windows),
            Platform::Windows,
            Architecture::X86,
            &PackageChannels::new(),
        );
        assert_eq!(
            without,
            [
                PACKAGE_FFMPEG,
                PACKAGE_SURGE_XT,
                PACKAGE_LJUNO_116,
                PACKAGE_LB_303,
                PACKAGE_LR_608,
                PACKAGE_LSAMPLER_24,
                PACKAGE_APP2CLAP,
                PACKAGE_LREAPARAMS,
            ]
            .map(String::from)
        );
        for package_id in &without {
            let error = resolve_latest_artifacts(
                std::slice::from_ref(package_id),
                Platform::Windows,
                Architecture::X86,
            )
            .unwrap_err();
            assert!(
                matches!(error, RabbitError::NoArtifactFound { .. }),
                "{package_id}: {error}"
            );
        }
    }

    /// REAPER on Windows on ARM and on a Mac keeps every package that has a
    /// build for it. On Windows on ARM that is every Windows package: REAPER
    /// there is ARM64EC, which loads x64 libraries, so FFmpeg, SWS,
    /// app2clap, LReaParams and Lo-lo's instruments offer their x64 builds.
    #[test]
    fn windows_on_arm_and_macos_keep_what_has_a_build_for_them() {
        let without = |platform, architecture| {
            packages_without_build(
                &package_ids_on(platform),
                platform,
                architecture,
                &PackageChannels::new(),
            )
        };
        for arm in [Architecture::Arm64, Architecture::Arm64Ec] {
            assert!(without(Platform::Windows, arm).is_empty(), "{arm:?}");
            assert_eq!(
                expected_artifact_kind(PACKAGE_SWS, Platform::Windows, arm).unwrap(),
                ArtifactKind::Installer
            );
            assert_eq!(
                expected_artifact_kind(PACKAGE_FFMPEG, Platform::Windows, arm).unwrap(),
                ArtifactKind::SevenZipArchive
            );
            assert_eq!(
                expected_artifact_kind(PACKAGE_APP2CLAP, Platform::Windows, arm).unwrap(),
                ArtifactKind::Archive
            );
            assert!(expected_artifact_kind(PACKAGE_SURGE_XT, Platform::Windows, arm).is_ok());
            assert_eq!(
                expected_artifact_kind(PACKAGE_LREAPARAMS, Platform::Windows, arm).unwrap(),
                ArtifactKind::ExtensionBinary
            );
            for instrument in [
                PACKAGE_LJUNO_116,
                PACKAGE_LB_303,
                PACKAGE_LR_608,
                PACKAGE_LSAMPLER_24,
            ] {
                assert_eq!(
                    expected_artifact_kind(instrument, Platform::Windows, arm).unwrap(),
                    ArtifactKind::Archive,
                    "{instrument} on {arm:?}"
                );
            }
        }
        assert!(without(Platform::Windows, Architecture::X64).is_empty());
        // A universal REAPER is judged by the slice it runs as here.
        for architecture in [
            Architecture::X64,
            Architecture::Arm64,
            Architecture::Universal,
        ] {
            assert!(
                without(Platform::MacOs, architecture).is_empty(),
                "{architecture:?}"
            );
        }
        assert_eq!(
            without(Platform::MacOs, Architecture::X86),
            [PACKAGE_SURGE_XT].map(String::from)
        );
    }

    /// An installed REAPER on Windows on ARM is an ARM64EC program, whose
    /// header names x64. Read from its program file in a portable folder, it
    /// is an arm64-ec REAPER: an update fetches REAPER's arm64ec installer
    /// and ReaPack's arm64ec DLL, and every other package has a build for
    /// it. An x64 REAPER in the same place still gets the x64 ones. Nothing
    /// goes online: the resolver's own selectors run on a download page and
    /// a release laid out like the real ones.
    #[test]
    fn an_installed_arm64ec_reaper_gets_the_arm64ec_builds() {
        use crate::arch_probe::PeKind;
        use crate::arch_probe::test_support::pe_image;

        let dir = tempdir().unwrap();
        let download_page = r#"
            <a href="files/7.x/reaper776-install.exe">x86</a>
            <a href="files/7.x/reaper776_x64-install.exe">x64</a>
            <a href="files/7.x/reaper776_arm64ec-install.exe">arm64ec</a>
        "#;
        let specs = crate::package::package_specs_by_id(Platform::Windows);
        let reaper = specs[PACKAGE_REAPER].http_artifact.as_ref().unwrap();
        let reapack = specs[PACKAGE_REAPACK].github_release.as_ref().unwrap();
        for (kind, architecture, installer, reapack_dll) in [
            (
                PeKind::Arm64Ec,
                Architecture::Arm64Ec,
                "reaper776_arm64ec-install.exe",
                "reaper_reapack-arm64ec.dll",
            ),
            (
                PeKind::X64,
                Architecture::X64,
                "reaper776_x64-install.exe",
                "reaper_reapack-x64.dll",
            ),
        ] {
            let folder = dir.path().join(kind.name());
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("reaper.exe"), pe_image(kind)).unwrap();
            fs::write(folder.join("reaper.ini"), "[REAPER]\n").unwrap();

            let target =
                crate::detection::target_reaper_architecture(Platform::Windows, &folder, None);
            assert_eq!(target, architecture, "{kind:?}");
            let dispatch = dispatch_architecture(target);

            let reaper_target = select_http_target(reaper, Platform::Windows, dispatch).unwrap();
            assert_eq!(reaper_target.report_arch, architecture);
            let HttpArtifactSource::ScrapeHref { href_match, .. } = &reaper_target.source else {
                panic!("REAPER's Windows installers come from its download page");
            };
            let href = find_href_with(download_page, |href, _| href_match.matches(href)).unwrap();
            assert_eq!(href, format!("files/7.x/{installer}"));

            let resolved = resolve_github_artifact_from_release_body(
                &release_with_every_asset(reapack),
                "https://example.test/release",
                PACKAGE_REAPACK,
                reapack,
                Platform::Windows,
                dispatch,
            )
            .unwrap();
            assert_eq!(resolved.file_name, reapack_dll);
            assert_eq!(resolved.architecture, architecture);

            assert!(
                packages_without_build(
                    &package_ids_on(Platform::Windows),
                    Platform::Windows,
                    target,
                    &PackageChannels::new(),
                )
                .is_empty(),
                "{kind:?}"
            );
        }
    }

    /// An ARM64EC REAPER loads x64 libraries, so REAPER on Windows on ARM, a
    /// new one (`arm64`) and an installed one (`arm64-ec`), gets the x64
    /// builds of FFmpeg, SWS and app2clap: the download an x64 REAPER gets.
    /// Each reports it as the x64 build it is, and so its receipt records
    /// it. Nothing goes online: the resolver's own selectors run on a
    /// download page and a release laid out like the real ones.
    #[test]
    fn reaper_on_windows_on_arm_gets_the_x64_ffmpeg_sws_and_app2clap() {
        let specs = crate::package::package_specs_by_id(Platform::Windows);
        let ffmpeg = specs[PACKAGE_FFMPEG].http_artifact.as_ref().unwrap();
        let sws = specs[PACKAGE_SWS].http_artifact.as_ref().unwrap();
        let app2clap = specs[PACKAGE_APP2CLAP].github_release.as_ref().unwrap();
        let sws_page = r#"
            <a href="download/featured/sws-2.14.0.7-Windows-x86.exe">x86</a>
            <a href="download/featured/sws-2.14.0.7-Windows-x64.exe">x64</a>
            <a href="download/featured/sws-2.14.0.7-Darwin-x86_64.dmg">Intel</a>
            <a href="download/featured/sws-2.14.0.7-Darwin-arm64.dmg">Apple</a>
        "#;
        let snapshots = r#"{
            "tag_name": "snapshots",
            "assets": [
                {
                    "name": "app2clap_2026.5.17.34.b6f558cf.zip",
                    "browser_download_url": "https://github.com/jcsteh/app2clap/releases/download/snapshots/app2clap_2026.5.17.34.b6f558cf.zip"
                }
            ]
        }"#;
        let x64_ffmpeg = select_http_target(ffmpeg, Platform::Windows, Architecture::X64).unwrap();
        let x64_sws = select_http_target(sws, Platform::Windows, Architecture::X64).unwrap();

        for arm in [Architecture::Arm64, Architecture::Arm64Ec] {
            for id in [PACKAGE_FFMPEG, PACKAGE_SWS, PACKAGE_APP2CLAP] {
                assert!(
                    package_has_build_for(&specs[id], Platform::Windows, arm),
                    "{id} on {arm:?}"
                );
            }

            // FFmpeg: Gyan.dev's x64 build, the one an x64 REAPER gets.
            let target = select_http_target(ffmpeg, Platform::Windows, arm).unwrap();
            assert_eq!(target, x64_ffmpeg, "{arm:?}");
            assert_eq!(target.report_arch, Architecture::X64);
            let HttpArtifactSource::FixedUrl { url, .. } = &target.source else {
                panic!("FFmpeg comes from Gyan.dev's fixed URL");
            };
            assert_eq!(
                url,
                "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z"
            );

            // SWS: the x64 installer, which installs reaper_sws-x64.dll.
            let target = select_http_target(sws, Platform::Windows, arm).unwrap();
            assert_eq!(target, x64_sws, "{arm:?}");
            let HttpArtifactSource::ScrapeHref {
                base_url,
                href_match,
                ..
            } = &target.source
            else {
                panic!("SWS's installers come from its home page");
            };
            let href = find_href_with(sws_page, |href, _| href_match.matches(href)).unwrap();
            let installer = artifact_from_href(
                PACKAGE_SWS,
                Version::parse("2.14.0.7").unwrap(),
                Platform::Windows,
                target.report_arch,
                github_artifact_kind(target.artifact_kind),
                base_url,
                &href,
            )
            .unwrap();
            assert_eq!(installer.file_name, "sws-2.14.0.7-Windows-x64.exe");
            assert_eq!(installer.kind, ArtifactKind::Installer);
            assert_eq!(installer.architecture, Architecture::X64);

            // app2clap: its one x64 zip, which holds app2clap.clap.
            let zip = resolve_github_artifact_from_release_body(
                snapshots,
                "https://api.github.com/repos/jcsteh/app2clap/releases/tags/snapshots",
                PACKAGE_APP2CLAP,
                app2clap,
                Platform::Windows,
                arm,
            )
            .unwrap();
            assert_eq!(zip.file_name, "app2clap_2026.5.17.34.b6f558cf.zip");
            assert_eq!(zip.kind, ArtifactKind::Archive);
            assert_eq!(zip.version.raw(), "2026.5.17.34");
            assert_eq!(zip.architecture, Architecture::X64, "{arm:?}");
        }
    }

    /// A new REAPER on Windows on ARM is resolved for `arm64`, what
    /// `Architecture::current()` gives on an ARM RABBIT, and an installed
    /// one for `arm64-ec`; both are the same ARM64EC REAPER. So every
    /// Windows package, on every channel, has a build for both or for
    /// neither, and picks the same download for both. The Windows on ARM
    /// smoke tables check the `arm64-ec` downloads, and this keeps a new
    /// install on the same ones. An ARM64EC REAPER can't load a plain ARM64
    /// library, so no `http_artifact` target serves it a build that reports
    /// `arm64`, as FFmpeg's target for tordona's ARM64 build did. Nothing
    /// goes online.
    #[test]
    fn a_new_and_an_installed_reaper_on_windows_on_arm_get_the_same_builds() {
        for package in crate::package::embedded_package_manifest().packages {
            let channels = std::iter::once(None).chain(package.channels.iter().map(|channel| {
                Some(match channel.parameter {
                    Some(_) => format!("{}:1", channel.id),
                    None => channel.id.clone(),
                })
            }));
            for channel in channels {
                let on_channel = package.on_channel(channel.as_deref());
                if !on_channel.supports_platform(Platform::Windows) {
                    continue;
                }
                let spec = on_channel.to_package_spec(Platform::Windows);
                let label = format!("{} on {channel:?}", spec.id);
                let has_build =
                    package_has_build_for(&spec, Platform::Windows, Architecture::Arm64Ec);
                assert_eq!(
                    package_has_build_for(&spec, Platform::Windows, Architecture::Arm64),
                    has_build,
                    "{label}"
                );
                if !has_build {
                    continue;
                }
                if let Some(release) = &spec.github_release {
                    let body = release_with_every_asset(release);
                    let [new, installed] =
                        [Architecture::Arm64, Architecture::Arm64Ec].map(|architecture| {
                            let artifact = resolve_github_artifact_from_release_body(
                                &body,
                                "https://example.test/release",
                                &spec.id,
                                release,
                                Platform::Windows,
                                architecture,
                            )
                            .unwrap();
                            (artifact.kind, artifact.file_name, artifact.url)
                        });
                    assert_eq!(new, installed, "{label}");
                }
                if let Some(http) = &spec.http_artifact {
                    let [new, installed] =
                        [Architecture::Arm64, Architecture::Arm64Ec].map(|architecture| {
                            select_http_target(http, Platform::Windows, architecture).map(
                                |target| (target.artifact_kind, &target.source, target.report_arch),
                            )
                        });
                    assert_eq!(new, installed, "{label}");
                    if let Some((_, _, report_arch)) = installed {
                        assert_ne!(report_arch, Architecture::Arm64, "{label}");
                    }
                }
            }
        }
    }

    /// When even the dispatch architecture is unknown, nothing is blocked.
    #[test]
    fn an_architecture_that_cant_be_known_blocks_nothing() {
        for platform in [Platform::Windows, Platform::MacOs] {
            for spec in crate::package::builtin_package_specs(platform) {
                assert!(
                    has_build_for_dispatch(&spec, platform, Architecture::Unknown),
                    "{} on {platform:?}",
                    spec.id
                );
            }
        }
    }

    fn file_url_for_test(path: &Path) -> String {
        if cfg!(windows) {
            format!(
                "file:///{}",
                path.display()
                    .to_string()
                    .replace('\\', "/")
                    .replace(' ', "%20")
            )
        } else {
            format!("file://{}", path.display().to_string().replace(' ', "%20"))
        }
    }
}
