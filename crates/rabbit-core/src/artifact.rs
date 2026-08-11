use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{IoPathContext, RabbitError, Result};
use crate::hash::sha256_file;
use crate::hfs::{fetch_file_list, file_url as hfs_file_url};
use crate::latest::{
    github_asset_version, github_release_url, pick_ffmpeg_tordona_release,
    pick_jaws_for_reaper_version, resolve_github_version, resolve_version_rule,
};
use crate::model::{Architecture, Platform};
use crate::package::{
    AssetMatch, GithubArtifactKind, GithubReleaseSpec, HttpArtifactSource, HttpArtifactSpec,
    HttpArtifactTarget, VersionRule, VersionSource, package_specs_by_id,
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
    /// Gyan.dev (x64) and `tordona/ffmpeg-win-arm64` (ARM64). Both
    /// upstreams ship `.7z` exclusively for the shared variant; the
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
    let client = http_client()?;
    let mut artifacts = Vec::new();
    let architecture = canonicalize_dispatch_arch(architecture);
    let specs = package_specs_by_id(platform);

    for package_id in package_ids {
        // Data-driven packages resolve through the generic engines: a single
        // GitHub release (github_release) or a non-GitHub scrape/fixed/max-
        // major source (http_artifact). Only JAWS still has a bespoke resolver.
        let spec = specs.get(package_id);
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
    let architecture = canonicalize_dispatch_arch(architecture);
    let specs = package_specs_by_id(platform);
    let spec = specs.get(package_id);
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
        // A selector that pins an arch reports it (ReaPack's per-slice DLLs);
        // an any-arch selector reports Universal, matching the old resolvers.
        architecture: selector.arch.unwrap_or(Architecture::Universal),
        kind,
        url: download_url.to_string(),
        file_name: file_name.to_string(),
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
    download_artifacts_with_progress(artifacts, cache_dir, &ProgressReporter::noop())
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
pub fn download_artifacts_with_progress(
    artifacts: &[ArtifactDescriptor],
    cache_dir: &Path,
    progress: &ProgressReporter,
) -> Result<Vec<CachedArtifact>> {
    let (pool, handles) = spawn_download_pool(artifacts, cache_dir, progress);
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
    cancel: Arc<AtomicBool>,
}

impl DownloadPool {
    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
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
) -> (DownloadPool, Vec<DownloadHandle>) {
    type ResultSender = std::sync::mpsc::SyncSender<Result<CachedArtifact>>;

    let cancel = Arc::new(AtomicBool::new(false));
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
        let cancel = Arc::clone(&cancel);
        let cache_dir = cache_dir.to_path_buf();
        let progress = progress.clone();
        std::thread::spawn(move || {
            // One client per worker so connections are reused across that
            // worker's downloads. A client-construction failure is delivered
            // per job (it is practically impossible and not clonable).
            let client = download_http_client();
            loop {
                if cancel.load(Ordering::Relaxed) {
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
    cancel: &AtomicBool,
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
    cancel: &AtomicBool,
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
        if cancel.load(Ordering::Relaxed) {
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
    cancel: &AtomicBool,
) -> std::result::Result<(), StreamCopyError> {
    let mut buffer = vec![0u8; DOWNLOAD_CHUNK_SIZE];
    let mut bytes_at_last_event: u64 = *bytes_downloaded;
    let mut last_event_at = Instant::now();

    loop {
        if cancel.load(Ordering::Relaxed) {
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
/// source. The version is the package `VersionRule` for scrape/fixed sources,
/// or the picked release's own version for a max-major GitHub source. No
/// matching target -> `NoArtifactFound` (SWS-on-WinArm, FFmpeg x86/macOS).
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
            })
        }
        HttpArtifactSource::GithubReleaseMaxMajor {
            repo,
            supported_major,
            asset,
        } => {
            let url = github_releases_list_url(repo);
            let body = http_get_text(client, &url)?;
            resolve_github_max_major_from_body(
                &body,
                &url,
                package_id,
                *supported_major,
                asset,
                kind,
                target.report_arch,
                platform,
                architecture,
            )
        }
    }
}

/// Version for a scrape/fixed `http_artifact` source: the package
/// `VersionRule`. These sources store the version on the descriptor only (it
/// is never interpolated), so a source without a version rule is a manifest
/// error. The `GithubReleaseMaxMajor` arm does NOT use this — it takes the
/// version from the picked release tag.
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

fn github_releases_list_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}/releases?per_page=100")
}

/// Body seam for [`HttpArtifactSource::GithubReleaseMaxMajor`]: the highest
/// stable release whose major == `supported_major`, then its asset matching
/// `asset_match`. The descriptor version is the release's own. Split out so it
/// can be exercised against a fixture body without a live request.
#[allow(clippy::too_many_arguments)]
fn resolve_github_max_major_from_body(
    body: &str,
    url: &str,
    package_id: &str,
    supported_major: u64,
    asset_match: &AssetMatch,
    kind: ArtifactKind,
    report_arch: Architecture,
    platform: Platform,
    architecture: Architecture,
) -> Result<ArtifactDescriptor> {
    let release = pick_ffmpeg_tordona_release(body, url, supported_major)?.ok_or(
        RabbitError::NoArtifactFound {
            package_id: package_id.to_string(),
            platform,
            architecture,
        },
    )?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset_match.matches(&asset.name))
        .ok_or(RabbitError::NoArtifactFound {
            package_id: package_id.to_string(),
            platform,
            architecture,
        })?;
    Ok(ArtifactDescriptor {
        package_id: package_id.to_string(),
        version: release.version,
        platform,
        architecture: report_arch,
        kind,
        url: asset.url.clone(),
        file_name: asset.name.clone(),
    })
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
    })
}

/// Client for metadata fetches (release JSON, scrape pages). Keeps
/// `reqwest::blocking`'s default 30-second per-operation timeout — these are
/// small responses and a hung fetch should fail fast.
fn http_client() -> Result<Client> {
    Client::builder()
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
    static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);
    fetch_remote_artifact_with_retries(
        &download_http_client()?,
        url,
        part_path,
        package_id,
        progress,
        &DOWNLOAD_RETRY_DELAYS,
        &NEVER_CANCELLED,
    )
}

/// Client for artifact (and self-update) downloads: a more generous stall
/// timeout than the blocking client's 30-second default, which killed slow
/// large downloads mid-body (see [`DOWNLOAD_STALL_TIMEOUT`]). The blocking
/// client's `timeout` is per read/write operation, so this bounds how long a
/// download may sit with NO bytes arriving — it does not cap total duration.
fn download_http_client() -> Result<Client> {
    Client::builder()
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
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    use crate::package::{
        AssetMatch, AssetSelector, GithubArtifactKind, GithubReleaseSelector, GithubReleaseSpec,
        HrefMatch, HttpArtifactSource, HttpArtifactSpec, HttpArtifactTarget, InstallDestination,
        PACKAGE_APP2CLAP, PACKAGE_FFMPEG, PACKAGE_OSARA, PACKAGE_REAKONTROL, PACKAGE_REAPACK,
        PACKAGE_REAPER, PACKAGE_SURGE_XT, PACKAGE_SWS, SupportedPlatform, VersionSource,
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
        static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);
        fetch_remote_artifact_with_retries(
            &download_http_client().unwrap(),
            url,
            part_path,
            "test-package",
            &ProgressReporter::noop(),
            &[Duration::ZERO, Duration::ZERO, Duration::ZERO],
            &NEVER_CANCELLED,
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
    /// data-driven artifact-resolver tests.
    fn app2clap_github_spec() -> GithubReleaseSpec {
        GithubReleaseSpec {
            repo: "jcsteh/app2clap".to_string(),
            release: GithubReleaseSelector::Tag("snapshots".to_string()),
            version_from: VersionSource::AssetName {
                strip_trailing_dot_segment: true,
            },
            assets: vec![AssetSelector {
                platform: SupportedPlatform::Windows,
                arch: Some(Architecture::X64),
                name_prefix: Some("app2clap_".to_string()),
                name_suffix: Some(".zip".to_string()),
                exact_name: None,
                artifact_kind: None,
            }],
            artifact_kind: Some(GithubArtifactKind::Archive),
            install_destination: InstallDestination::WindowsClapDir,
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
                    name_prefix: Some("reaKontrol_windows_".to_string()),
                    name_suffix: Some(".zip".to_string()),
                    exact_name: None,
                    artifact_kind: None,
                },
                AssetSelector {
                    platform: SupportedPlatform::Macos,
                    arch: None,
                    name_prefix: Some("reaKontrol_mac_".to_string()),
                    name_suffix: Some(".zip".to_string()),
                    exact_name: None,
                    artifact_kind: None,
                },
            ],
            artifact_kind: Some(GithubArtifactKind::Archive),
            install_destination: InstallDestination::UserPlugins,
        }
    }

    /// ReaPack's `github_release` block: `/releases/latest`, version from the
    /// `tag_name`, per-(platform,arch) exact-name `ExtensionBinary` assets.
    fn reapack_github_spec() -> GithubReleaseSpec {
        let exact = |platform, arch, name: &str| AssetSelector {
            platform,
            arch: Some(arch),
            name_prefix: None,
            name_suffix: None,
            exact_name: Some(name.to_string()),
            artifact_kind: None,
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
            install_destination: InstallDestination::UserPlugins,
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
                    name_prefix: Some("surge-xt-win64-".to_string()),
                    name_suffix: Some("-setup.exe".to_string()),
                    exact_name: None,
                    artifact_kind: Some(GithubArtifactKind::Installer),
                },
                AssetSelector {
                    platform: SupportedPlatform::Macos,
                    arch: None,
                    name_prefix: Some("surge-xt-macOS-".to_string()),
                    name_suffix: Some(".dmg".to_string()),
                    exact_name: None,
                    artifact_kind: Some(GithubArtifactKind::DiskImage),
                },
            ],
            artifact_kind: None,
            install_destination: InstallDestination::default(),
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
        // No Windows-arm target → None (this is how SWS rejects Windows-on-ARM).
        assert!(select_http_target(&spec, Platform::Windows, Architecture::Arm64).is_none());
        assert!(select_http_target(&spec, Platform::MacOs, Architecture::X64).is_none());
        assert!(select_http_target(&spec, Platform::MacOs, Architecture::Arm64).is_some());
    }

    #[test]
    fn asset_match_isolates_full_shared_arm64_7z() {
        let m = AssetMatch {
            contains: vec!["-full-shared-win-arm64".to_string()],
            ends_with: Some(".7z".to_string()),
        };
        assert!(m.matches("ffmpeg-8.1.1-full-shared-win-arm64.7z"));
        assert!(!m.matches("ffmpeg-8.1.1-essentials-shared-win-arm64.7z"));
        assert!(!m.matches("ffmpeg-8.1.1-full-shared-win-arm64.zip"));
    }

    /// The manifest FFmpeg `supported_major` must track the detector's
    /// `FFMPEG_SUPPORTED_MAJOR`; a divergence would resolve the wrong major.
    #[test]
    fn ffmpeg_manifest_supported_major_matches_constant() {
        let spec = crate::package::builtin_package_specs(Platform::Windows)
            .into_iter()
            .find(|spec| spec.id == PACKAGE_FFMPEG)
            .unwrap();
        let http = spec.http_artifact.expect("ffmpeg uses http_artifact");
        let arm = http
            .targets
            .iter()
            .find_map(|target| match &target.source {
                HttpArtifactSource::GithubReleaseMaxMajor {
                    supported_major, ..
                } => Some(*supported_major),
                _ => None,
            })
            .expect("ffmpeg has a github_release_max_major target");
        assert_eq!(arm, crate::latest::FFMPEG_SUPPORTED_MAJOR);
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
        };
        let fast = ArtifactDescriptor {
            package_id: "fast-package".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::Installer,
            url: fast_source.display().to_string(),
            file_name: "fast.bin".to_string(),
        };

        let cache_dir = tempdir().unwrap();
        let started = std::time::Instant::now();
        let (_pool, handles) =
            spawn_download_pool(&[slow, fast], cache_dir.path(), &ProgressReporter::noop());
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
        };

        let cache_dir = tempdir().unwrap();
        let (_pool, handles) = spawn_download_pool(
            &[artifact.clone(), artifact],
            cache_dir.path(),
            &ProgressReporter::noop(),
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
    fn resolves_ffmpeg_arm64_asset_from_tordona_releases() {
        let body = r#"[
            {
                "tag_name": "daily-autobuild-2026.05.06.0",
                "prerelease": false,
                "assets": [
                    {
                        "name": "ffmpeg-master-latest-full-shared-win-arm64.7z",
                        "browser_download_url": "https://example.test/ffmpeg-master-latest-full-shared-win-arm64.7z"
                    }
                ]
            },
            {
                "tag_name": "8.1.1",
                "prerelease": false,
                "assets": [
                    {
                        "name": "ffmpeg-8.1.1-essentials-shared-win-arm64.7z",
                        "browser_download_url": "https://example.test/ffmpeg-8.1.1-essentials-shared-win-arm64.7z"
                    },
                    {
                        "name": "ffmpeg-8.1.1-full-shared-win-arm64.7z",
                        "browser_download_url": "https://example.test/ffmpeg-8.1.1-full-shared-win-arm64.7z"
                    },
                    {
                        "name": "ffmpeg-8.1.1-full-static-win-arm64.7z",
                        "browser_download_url": "https://example.test/ffmpeg-8.1.1-full-static-win-arm64.7z"
                    }
                ]
            },
            {
                "tag_name": "8.0.2",
                "prerelease": false,
                "assets": [
                    {
                        "name": "ffmpeg-8.0.2-full-shared-win-arm64.7z",
                        "browser_download_url": "https://example.test/ffmpeg-8.0.2-full-shared-win-arm64.7z"
                    }
                ]
            },
            {
                "tag_name": "7.1.4",
                "prerelease": false,
                "assets": [
                    {
                        "name": "ffmpeg-7.1.4-full-shared-win-arm64.7z",
                        "browser_download_url": "https://example.test/ffmpeg-7.1.4-full-shared-win-arm64.7z"
                    }
                ]
            }
        ]"#;

        let releases_url =
            "https://api.github.com/repos/tordona/ffmpeg-win-arm64/releases?per_page=100";
        let asset = AssetMatch {
            contains: vec!["-full-shared-win-arm64".to_string()],
            ends_with: Some(".7z".to_string()),
        };
        let arm64 = resolve_github_max_major_from_body(
            body,
            releases_url,
            PACKAGE_FFMPEG,
            8,
            &asset,
            ArtifactKind::SevenZipArchive,
            Architecture::Arm64,
            Platform::Windows,
            Architecture::Arm64,
        )
        .unwrap();
        assert_eq!(arm64.package_id, PACKAGE_FFMPEG);
        assert_eq!(arm64.kind, ArtifactKind::SevenZipArchive);
        assert_eq!(arm64.version.raw(), "8.1.1");
        assert_eq!(arm64.file_name, "ffmpeg-8.1.1-full-shared-win-arm64.7z");
        assert_eq!(arm64.architecture, Architecture::Arm64);

        // Same body with no n8 stable tags must surface NoArtifactFound
        // rather than silently picking the autobuild.
        let only_autobuild_body = r#"[
            {
                "tag_name": "daily-autobuild-2026.05.06.0",
                "prerelease": false,
                "assets": [
                    {
                        "name": "ffmpeg-master-latest-full-shared-win-arm64.7z",
                        "browser_download_url": "https://example.test/ffmpeg-master-latest-full-shared-win-arm64.7z"
                    }
                ]
            }
        ]"#;
        let error = resolve_github_max_major_from_body(
            only_autobuild_body,
            releases_url,
            PACKAGE_FFMPEG,
            8,
            &asset,
            ArtifactKind::SevenZipArchive,
            Architecture::Arm64,
            Platform::Windows,
            Architecture::Arm64,
        )
        .unwrap_err();
        assert!(matches!(error, RabbitError::NoArtifactFound { .. }));
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
