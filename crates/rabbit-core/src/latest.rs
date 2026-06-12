use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{RabbitError, Result};
use crate::hfs::{HfsListEntry, fetch_file_list, parse_get_file_list_response};
use crate::package::{
    PACKAGE_FFMPEG, PACKAGE_JAWS_SCRIPTS, PACKAGE_OSARA, PACKAGE_REAKONTROL, PACKAGE_REAPACK,
    PACKAGE_REAPER, PACKAGE_SURGE_XT, PACKAGE_SWS,
};
use crate::plan::AvailablePackage;
use crate::version::Version;

const USER_AGENT: &str = "RABBIT/0.1 (+https://github.com/Timtam/rabbit)";

pub const REAPER_DOWNLOAD_URL: &str = "https://www.reaper.fm/download.php";
pub const OSARA_UPDATE_URL: &str = "https://osara.reaperaccessibility.com/snapshots/update.json";
pub const SWS_HOME_URL: &str = "https://sws-extension.org/";
pub const REAPACK_GITHUB_LATEST_URL: &str =
    "https://api.github.com/repos/cfillion/reapack/releases/latest";
pub const REAKONTROL_GITHUB_LATEST_URL: &str =
    "https://api.github.com/repos/jcsteh/reaKontrol/releases/latest";
/// Gyan.dev's plain-text version stamp for the latest stable
/// `ffmpeg-release-full-shared.7z`. Returns a single line of UTF-8 like
/// `8.1.1` — no JSON, no HTML scraping. We use Gyan as the canonical
/// version source for FFmpeg (and as the x64 artifact source) because
/// BtbN doesn't publish stable tagged releases — only rolling
/// autobuilds — and Gyan is also winget's upstream for FFmpeg.
pub const FFMPEG_GYAN_VERSION_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z.ver";
/// Gyan.dev's stable `ffmpeg-release-full-shared.7z` URL. The path is
/// fixed; the server redirects to the current versioned file.
pub const FFMPEG_GYAN_X64_ARCHIVE_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z";
/// `tordona/ffmpeg-win-arm64` GitHub releases — used for the ARM64
/// fan-out of [`crate::package::ArtifactProvider::FfmpegSharedBuild`].
/// Tags are plain `<major>.<minor>.<patch>` (no `n` prefix); we pick
/// the highest non-prerelease tag whose major matches
/// [`FFMPEG_SUPPORTED_MAJOR`].
pub const FFMPEG_TORDONA_ARM64_RELEASES_URL: &str =
    "https://api.github.com/repos/tordona/ffmpeg-win-arm64/releases?per_page=100";

/// FFmpeg major version that REAPER's video decoder is known to support.
/// Bump this when a new REAPER release adds support for the next FFmpeg
/// major (e.g. REAPER 7.66 added FFmpeg 8 support → pinned to `8`). The
/// detector and the latest-version provider both reference this so a
/// single bump tracks both code paths.
pub const FFMPEG_SUPPORTED_MAJOR: u64 = 8;

/// Surge XT's rolling nightly release. The release tag itself is the
/// static string `Nightly`; the actual build identity (date + commit sha)
/// only appears on the asset filenames. The version parser scans the
/// asset list for the canonical `win64 setup.exe` filename and produces
/// a `NIGHTLY-<YYYY-MM-DD>-<sha>` version. We pull from this channel
/// rather than `surge-synthesizer/releases-xt` because the latter's
/// `1.3.4` (2024-08-11) is the most recent stable and is now ~years
/// behind upstream — the project effectively distributes through
/// nightlies.
pub const SURGE_XT_NIGHTLY_URL: &str =
    "https://api.github.com/repos/surge-synthesizer/surge/releases/tags/Nightly";

/// HFS root that hosts the JAWS-for-REAPER scripts archive (rejetto HFS).
pub const JAWS_FOR_REAPER_HFS_BASE: &str = "https://hoard.reaperaccessibility.com";
/// Folder under that root where the versioned `*.zip` lives. The exact folder
/// name is the only piece that needs to track upstream changes; the parser
/// itself works with any HFS listing.
pub const JAWS_FOR_REAPER_HFS_FOLDER: &str =
    "/Custom%20actions,%20Scripts%20and%20jsfx/Windows%20Scripts/JAWS%20Scripts%20by%20Snowman/";

/// Synthesize the URL we report in `RemoteData` errors so messages stay
/// stable regardless of which HTTP verb the caller used.
fn jaws_for_reaper_listing_url() -> String {
    format!(
        "{}/~/api/get_file_list?path={}",
        JAWS_FOR_REAPER_HFS_BASE.trim_end_matches('/'),
        JAWS_FOR_REAPER_HFS_FOLDER
    )
}

/// One package whose latest-version check failed, with the error rendered
/// as a display string so callers (CLI output, wizard notes) don't need to
/// keep the live error value around.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestVersionFailure {
    pub package_id: String,
    pub message: String,
}

/// Outcome of [`fetch_latest_versions`]: the versions that could be
/// determined plus a per-package record of every provider that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestVersionsReport {
    pub packages: Vec<AvailablePackage>,
    pub failures: Vec<LatestVersionFailure>,
}

/// Query every latest-version provider, tolerating per-provider failures.
///
/// A single unreachable upstream (e.g. the SWS homepage being down) must not
/// take the whole update check with it — every other package can still be
/// checked and updated. Each provider error therefore lands in
/// [`LatestVersionsReport::failures`] instead of aborting the batch; callers
/// surface those failures (CLI warning lines, wizard rows disabled with a
/// reason) and proceed with the packages that did resolve. The only hard
/// `Err` left is failing to construct the HTTP client itself, which is a
/// local environment problem that would fail every provider identically.
pub fn fetch_latest_versions() -> Result<LatestVersionsReport> {
    let client = build_http_client()?;
    let mut packages = Vec::new();
    let mut failures = Vec::new();
    for (package_id, url, parser) in providers() {
        match http_get_text(&client, url).and_then(|body| parser(&body, url)) {
            Ok(version) => packages.push(AvailablePackage {
                package_id: package_id.to_string(),
                version: Some(version),
            }),
            Err(error) => failures.push(LatestVersionFailure {
                package_id: package_id.to_string(),
                message: error.to_string(),
            }),
        }
    }
    match fetch_jaws_for_reaper_latest(&client) {
        Ok(version) => packages.push(AvailablePackage {
            package_id: PACKAGE_JAWS_SCRIPTS.to_string(),
            version: Some(version),
        }),
        Err(error) => failures.push(LatestVersionFailure {
            package_id: PACKAGE_JAWS_SCRIPTS.to_string(),
            message: error.to_string(),
        }),
    }
    Ok(LatestVersionsReport { packages, failures })
}

/// Fetch the latest version for a single package. Useful when a UI wants to
/// stream per-package results as they arrive instead of blocking on the full
/// batch.
pub fn fetch_latest_for_package(package_id: &str) -> Result<Version> {
    if package_id == PACKAGE_JAWS_SCRIPTS {
        let client = build_http_client()?;
        return fetch_jaws_for_reaper_latest(&client);
    }
    let (_, url, parser) = providers()
        .into_iter()
        .find(|(id, _, _)| *id == package_id)
        .ok_or_else(|| RabbitError::RemoteData {
            url: String::new(),
            message: format!("no latest-version provider configured for package {package_id}"),
        })?;
    let client = build_http_client()?;
    let body = http_get_text(&client, url)?;
    parser(&body, url)
}

/// POSTs the HFS listing for the JAWS-for-REAPER scripts folder and returns
/// the highest-version `*.zip` it advertises.
pub fn fetch_jaws_for_reaper_latest(client: &Client) -> Result<Version> {
    let entries = fetch_file_list(client, JAWS_FOR_REAPER_HFS_BASE, JAWS_FOR_REAPER_HFS_FOLDER)?;
    pick_jaws_for_reaper_version(&entries)
        .map(|(version, _)| version)
        .ok_or_else(|| RabbitError::RemoteData {
            url: jaws_for_reaper_listing_url(),
            message: "no versioned JAWS-for-REAPER installer in folder listing".to_string(),
        })
}

/// Pure-data twin of [`fetch_jaws_for_reaper_latest`] for unit tests: parses
/// an HFS listing body and extracts the highest version. Lives next to the
/// extractor so the parser can be exercised without a network call.
pub fn parse_jaws_for_reaper_listing(body: &str, url: &str) -> Result<Version> {
    let entries = parse_get_file_list_response(body, url)?;
    pick_jaws_for_reaper_version(&entries)
        .map(|(version, _)| version)
        .ok_or_else(|| RabbitError::RemoteData {
            url: url.to_string(),
            message: "no versioned JAWS-for-REAPER installer in folder listing".to_string(),
        })
}

/// Walk an HFS listing and return the highest-version `*.exe`, along with
/// the file name so the artifact resolver can build a download URL. The
/// JAWS-for-REAPER scripts are distributed as a single-file Windows
/// installer executable, so we filter on `.exe` rather than archive
/// extensions.
pub(crate) fn pick_jaws_for_reaper_version(entries: &[HfsListEntry]) -> Option<(Version, String)> {
    let mut best: Option<(Version, String)> = None;
    for entry in entries {
        if entry.is_directory {
            continue;
        }
        if !entry.name.to_ascii_lowercase().ends_with(".exe") {
            continue;
        }
        let Some(version) = jaws_for_reaper_version_from_filename(&entry.name) else {
            continue;
        };
        best = Some(match best {
            Some((current_version, current_name))
                if current_version.cmp_lenient(&version).is_ge() =>
            {
                (current_version, current_name)
            }
            _ => (version, entry.name.clone()),
        });
    }
    best
}

/// Extract a version from a JAWS-for-REAPER installer filename. Accepts
/// either a dotted version (`JFRSCRIPTS_v3.18.exe` → `3.18`) or a plain
/// integer build number (`Reaper_JawsScripts_89.exe` → `89`), since the
/// upstream naming is the latter today and the dotted form has been used
/// historically. We pick the **last** digit-or-dot run in the stem so
/// prefixes/suffixes don't confuse the picker.
pub(crate) fn jaws_for_reaper_version_from_filename(name: &str) -> Option<Version> {
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".exe") {
        return None;
    }
    let stem = &name[..name.len() - 4];

    let bytes = stem.as_bytes();
    let mut last: Option<&str> = None;
    let mut cursor = 0;
    while cursor < bytes.len() {
        if !bytes[cursor].is_ascii_digit() {
            cursor += 1;
            continue;
        }
        let start = cursor;
        let mut end = cursor;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            end += 1;
        }
        let mut candidate = &stem[start..end];
        // Trim a trailing dot so something like `3.18.` parses as `3.18`.
        while candidate.ends_with('.') {
            candidate = &candidate[..candidate.len() - 1];
        }
        if !candidate.is_empty() {
            last = Some(candidate);
        }
        cursor = end.max(start + 1);
    }

    last.and_then(|candidate| Version::parse(candidate).ok())
}

fn build_http_client() -> Result<Client> {
    Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|source| RabbitError::Http {
            url: "client-builder".to_string(),
            source,
        })
}

fn providers() -> [(&'static str, &'static str, VersionParser); 7] {
    [
        (
            PACKAGE_REAPER,
            REAPER_DOWNLOAD_URL,
            parse_reaper_latest_version as VersionParser,
        ),
        (
            PACKAGE_OSARA,
            OSARA_UPDATE_URL,
            parse_osara_update_json as VersionParser,
        ),
        (
            PACKAGE_SWS,
            SWS_HOME_URL,
            parse_sws_latest_version as VersionParser,
        ),
        (
            PACKAGE_REAPACK,
            REAPACK_GITHUB_LATEST_URL,
            parse_github_latest_release_json as VersionParser,
        ),
        (
            PACKAGE_REAKONTROL,
            REAKONTROL_GITHUB_LATEST_URL,
            parse_reakontrol_snapshot_version as VersionParser,
        ),
        (
            PACKAGE_FFMPEG,
            FFMPEG_GYAN_VERSION_URL,
            parse_ffmpeg_gyan_release_version as VersionParser,
        ),
        (
            PACKAGE_SURGE_XT,
            SURGE_XT_NIGHTLY_URL,
            parse_surge_xt_nightly_release as VersionParser,
        ),
    ]
}

type VersionParser = fn(&str, &str) -> Result<Version>;

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

pub fn parse_osara_update_json(body: &str, url: &str) -> Result<Version> {
    let value: Value = serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
        url: url.to_string(),
        message: source.to_string(),
    })?;
    let Some(version) = value.get("version").and_then(Value::as_str) else {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: "missing string field: version".to_string(),
        });
    };
    Version::parse(version)
}

pub fn parse_github_latest_release_json(body: &str, url: &str) -> Result<Version> {
    let value: Value = serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
        url: url.to_string(),
        message: source.to_string(),
    })?;
    let Some(tag_name) = value.get("tag_name").and_then(Value::as_str) else {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: "missing string field: tag_name".to_string(),
        });
    };
    Version::parse(tag_name.trim_start_matches('v'))
}

pub fn parse_reakontrol_snapshot_version(body: &str, url: &str) -> Result<Version> {
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

    let mut latest: Option<Version> = None;
    for asset in assets {
        let Some(name) = asset.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(version) = reakontrol_version_from_asset_name(name) else {
            continue;
        };
        latest = Some(match latest {
            Some(current) if current.cmp_lenient(&version).is_ge() => current,
            _ => version,
        });
    }

    latest.ok_or_else(|| RabbitError::RemoteData {
        url: url.to_string(),
        message: "no ReaKontrol snapshot asset matched the expected name pattern".to_string(),
    })
}

pub(crate) fn reakontrol_version_from_asset_name(name: &str) -> Option<Version> {
    let stem = name.strip_suffix(".zip")?;
    let after_platform = stem
        .strip_prefix("reaKontrol_windows_")
        .or_else(|| stem.strip_prefix("reaKontrol_mac_"))?;
    let version_part = after_platform
        .rsplit_once('.')
        .map(|(left, _commit)| left)?;
    Version::parse(version_part).ok()
}

/// Parse Gyan.dev's `*.ver` plain-text payload — a single line like
/// `8.1.1` (sometimes with trailing whitespace / newlines). We trim
/// and parse; anything that doesn't shape like a version is rejected.
pub fn parse_ffmpeg_gyan_release_version(body: &str, url: &str) -> Result<Version> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: "Gyan FFmpeg release-version response was empty".to_string(),
        });
    }
    Version::parse(trimmed).map_err(|_| RabbitError::RemoteData {
        url: url.to_string(),
        message: format!("Gyan FFmpeg release-version response is not a version: {trimmed:?}"),
    })
}

/// Parse the Surge XT `Nightly` release JSON and return a `Version`
/// derived from the canonical win64 setup.exe asset filename
/// (`surge-xt-win64-NIGHTLY-<YYYY-MM-DD>-<sha>-setup.exe`). Falls back
/// to the macOS `.dmg` filename when the Windows asset is mid-re-upload
/// (the nightly publishes both within seconds of each other, but the
/// fallback keeps the wizard resilient if it catches a partial state).
///
/// The returned `Version` is the literal `NIGHTLY-<date>-<sha>` token —
/// `Version::cmp_lenient` picks up the leading date numerics
/// (`[YYYY, MM, DD, …]`) so newer/older comparisons work without a
/// dedicated comparator. The artifact resolver re-parses the same JSON
/// to pick a download URL; that keeps both sides reading the same
/// asset list rather than depending on a state cache between calls.
pub fn parse_surge_xt_nightly_release(body: &str, url: &str) -> Result<Version> {
    let names = surge_xt_release_asset_names(body, url)?;
    if let Some(version) = names
        .iter()
        .find_map(|name| surge_xt_version_from_windows_asset(name))
    {
        return Ok(version);
    }
    if let Some(version) = names
        .iter()
        .find_map(|name| surge_xt_version_from_macos_asset(name))
    {
        return Ok(version);
    }
    Err(RabbitError::RemoteData {
        url: url.to_string(),
        message: "no Surge XT nightly setup/dmg asset matched the expected name pattern"
            .to_string(),
    })
}

/// Collect the `assets[].name` strings from a Surge XT `Nightly` release
/// JSON payload. The artifact resolver shares this helper so both sides
/// see the same asset list.
pub(crate) fn surge_xt_release_asset_names(body: &str, url: &str) -> Result<Vec<String>> {
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
    Ok(assets
        .iter()
        .filter_map(|asset| {
            asset
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect())
}

/// Extract the `NIGHTLY-<YYYY-MM-DD>-<sha>` token from the canonical
/// Windows setup-installer asset filename. Returns `None` when the
/// filename doesn't match the win64 setup.exe pattern.
pub(crate) fn surge_xt_version_from_windows_asset(name: &str) -> Option<Version> {
    let stem = name
        .strip_prefix("surge-xt-win64-")
        .and_then(|rest| rest.strip_suffix("-setup.exe"))?;
    surge_xt_parse_nightly_token(stem)
}

/// Extract the `NIGHTLY-<YYYY-MM-DD>-<sha>` token from the canonical
/// macOS DMG asset filename. Used as a fallback by the version parser
/// and as the macOS-side anchor by the artifact resolver.
pub(crate) fn surge_xt_version_from_macos_asset(name: &str) -> Option<Version> {
    let stem = name
        .strip_prefix("surge-xt-macOS-")
        .and_then(|rest| rest.strip_suffix(".dmg"))?;
    surge_xt_parse_nightly_token(stem)
}

/// Accept a `NIGHTLY-YYYY-MM-DD-sha` substring and return it verbatim as
/// a `Version`. Rejects any non-nightly stem so the rolling `latest` /
/// `pluginsonly` / `beta` flavored assets in the same release don't
/// poison the version pick.
fn surge_xt_parse_nightly_token(stem: &str) -> Option<Version> {
    if !stem.starts_with("NIGHTLY-") {
        return None;
    }
    let parts: Vec<&str> = stem.splitn(5, '-').collect();
    // Expect ["NIGHTLY", "YYYY", "MM", "DD", "<sha>"].
    if parts.len() != 5 {
        return None;
    }
    let [_, year, month, day, sha] = [parts[0], parts[1], parts[2], parts[3], parts[4]];
    if year.len() != 4
        || !year.chars().all(|ch| ch.is_ascii_digit())
        || month.len() != 2
        || !month.chars().all(|ch| ch.is_ascii_digit())
        || day.len() != 2
        || !day.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    if sha.is_empty() || !sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Version::parse(stem).ok()
}

/// Walk the tordona/ffmpeg-win-arm64 releases JSON and return both the
/// highest stable tag whose major matches `FFMPEG_SUPPORTED_MAJOR` and
/// its assets. The ARM64 artifact resolver uses the assets list to
/// pick `ffmpeg-<ver>-full-shared-win-arm64.7z`. Pre-releases (the
/// daily `daily-autobuild-*` autobuilds and the `latest` rolling tag)
/// and majors other than the supported one are skipped.
pub(crate) fn pick_ffmpeg_tordona_release(
    body: &str,
    url: &str,
    supported_major: u64,
) -> Result<Option<TordonaRelease>> {
    let releases = parse_tordona_releases_array(body, url)?;
    let mut best: Option<TordonaRelease> = None;
    for release in releases {
        let parts = release.version.numeric_parts();
        if parts.first().copied() != Some(supported_major) {
            continue;
        }
        best = Some(match best {
            Some(current) if current.version.cmp_lenient(&release.version).is_ge() => current,
            _ => release.clone(),
        });
    }
    Ok(best)
}

#[derive(Debug, Clone)]
pub(crate) struct TordonaRelease {
    pub version: Version,
    pub assets: Vec<TordonaAsset>,
}

#[derive(Debug, Clone)]
pub(crate) struct TordonaAsset {
    pub name: String,
    pub url: String,
}

fn parse_tordona_releases_array(body: &str, url: &str) -> Result<Vec<TordonaRelease>> {
    let value: Value = serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
        url: url.to_string(),
        message: source.to_string(),
    })?;
    let array = value.as_array().ok_or_else(|| RabbitError::RemoteData {
        url: url.to_string(),
        message: "tordona/ffmpeg-win-arm64 releases response was not a JSON array".to_string(),
    })?;

    let mut releases = Vec::with_capacity(array.len());
    for entry in array {
        if entry.get("prerelease").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let Some(tag_name) = entry.get("tag_name").and_then(Value::as_str) else {
            continue;
        };
        let Some(version) = ffmpeg_version_from_tordona_tag(tag_name) else {
            continue;
        };
        let assets = entry
            .get("assets")
            .and_then(Value::as_array)
            .map(|assets| {
                assets
                    .iter()
                    .filter_map(|asset| {
                        let name = asset.get("name").and_then(Value::as_str)?.to_string();
                        let url = asset
                            .get("browser_download_url")
                            .and_then(Value::as_str)?
                            .to_string();
                        Some(TordonaAsset { name, url })
                    })
                    .collect()
            })
            .unwrap_or_default();
        releases.push(TordonaRelease { version, assets });
    }
    Ok(releases)
}

/// Extract a `Version` from a tordona/ffmpeg-win-arm64 release tag.
/// Tags are plain `<major>.<minor>.<patch>` (no `n` prefix, no `v`).
/// Rolling tags (`latest`, `daily-autobuild-…`) return `None`.
pub(crate) fn ffmpeg_version_from_tordona_tag(tag_name: &str) -> Option<Version> {
    if !tag_name.starts_with(|ch: char| ch.is_ascii_digit()) {
        return None;
    }
    Version::parse(tag_name).ok()
}

pub fn parse_sws_latest_version(body: &str, url: &str) -> Result<Version> {
    let marker = "Latest stable version:";
    let Some(marker_start) = body.find(marker) else {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: "missing latest stable version marker".to_string(),
        });
    };
    let tail_start = marker_start + marker.len();
    let tail = &body[tail_start..body.len().min(tail_start + 160)];
    let Some(version_start) = tail.find('v') else {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: "missing SWS version prefix".to_string(),
        });
    };

    let base = collect_version_chars(&tail[version_start + 1..]);
    let build = tail
        .find('#')
        .map(|index| collect_digits(&tail[index + 1..]))
        .filter(|digits| !digits.is_empty());

    if base.is_empty() {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: "missing SWS version number".to_string(),
        });
    }

    let version = match build {
        Some(build) => format!("{base}.{build}"),
        None => base,
    };
    Version::parse(version)
}

pub fn parse_reaper_latest_version(body: &str, url: &str) -> Result<Version> {
    if let Some(version) = version_after_marker(body, "Version ") {
        return Version::parse(version);
    }
    if let Some(version) = version_after_marker(body, "REAPER v") {
        return Version::parse(version);
    }

    Err(RabbitError::RemoteData {
        url: url.to_string(),
        message: "missing REAPER version token".to_string(),
    })
}

fn version_after_marker<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let marker_start = text.find(marker)?;
    let start = marker_start + marker.len();
    first_version_like_token(&text[start..text.len().min(start + 80)])
}

fn first_version_like_token(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_digit() {
            continue;
        }
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            end += 1;
        }
        let candidate = &text[start..end];
        if candidate.contains('.') {
            return Some(candidate);
        }
    }
    None
}

fn collect_version_chars(text: &str) -> String {
    text.chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect()
}

fn collect_digits(text: &str) -> String {
    text.chars()
        .skip_while(|ch| ch.is_ascii_whitespace())
        .take_while(char::is_ascii_digit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        FFMPEG_GYAN_VERSION_URL, FFMPEG_TORDONA_ARM64_RELEASES_URL, OSARA_UPDATE_URL,
        REAKONTROL_GITHUB_LATEST_URL, REAPACK_GITHUB_LATEST_URL, REAPER_DOWNLOAD_URL,
        SURGE_XT_NIGHTLY_URL, SWS_HOME_URL, ffmpeg_version_from_tordona_tag,
        jaws_for_reaper_listing_url, jaws_for_reaper_version_from_filename,
        parse_ffmpeg_gyan_release_version, parse_github_latest_release_json,
        parse_jaws_for_reaper_listing, parse_osara_update_json, parse_reakontrol_snapshot_version,
        parse_reaper_latest_version, parse_surge_xt_nightly_release, parse_sws_latest_version,
        pick_ffmpeg_tordona_release, reakontrol_version_from_asset_name,
        surge_xt_version_from_macos_asset, surge_xt_version_from_windows_asset,
    };

    #[test]
    fn parses_osara_update_json() {
        let version =
            parse_osara_update_json(r#"{"version":"2026.4.16.2157,593ff26b"}"#, OSARA_UPDATE_URL)
                .unwrap();
        assert_eq!(version.raw(), "2026.4.16.2157,593ff26b");
    }

    #[test]
    fn parses_sws_home_page_version() {
        let version = parse_sws_latest_version(
            "## Latest stable version: v2.14.0 #7 - September 07, 2025",
            SWS_HOME_URL,
        )
        .unwrap();
        assert_eq!(version.raw(), "2.14.0.7");
    }

    #[test]
    fn parses_reapack_github_latest_release() {
        let version =
            parse_github_latest_release_json(r#"{"tag_name":"v1.2.6"}"#, REAPACK_GITHUB_LATEST_URL)
                .unwrap();
        assert_eq!(version.raw(), "1.2.6");
    }

    #[test]
    fn parses_reaper_download_page_version() {
        let version = parse_reaper_latest_version(
            "<div class='hdrbottom'>Version 7.69: April 12, 2026</div>",
            REAPER_DOWNLOAD_URL,
        )
        .unwrap();
        assert_eq!(version.raw(), "7.69");
    }

    #[test]
    fn extracts_reakontrol_version_from_asset_name() {
        let version =
            reakontrol_version_from_asset_name("reaKontrol_windows_2025.6.6.7.bfbe7606.zip")
                .unwrap();
        assert_eq!(version.raw(), "2025.6.6.7");
        let version =
            reakontrol_version_from_asset_name("reaKontrol_mac_2026.2.16.100.deadbeef.zip")
                .unwrap();
        assert_eq!(version.raw(), "2026.2.16.100");
        assert!(reakontrol_version_from_asset_name("README.md").is_none());
    }

    #[test]
    fn picks_highest_reakontrol_snapshot_version_from_assets() {
        let body = r#"{
            "tag_name": "snapshots",
            "assets": [
                {"name": "reaKontrol_windows_2025.6.6.7.bfbe7606.zip"},
                {"name": "reaKontrol_mac_2026.2.16.100.cafef00d.zip"},
                {"name": "reaKontrol_windows_2026.2.16.100.cafef00d.zip"},
                {"name": "reaKontrol_mac_2025.7.25.10.4ce6b01f.zip"}
            ]
        }"#;
        let version =
            parse_reakontrol_snapshot_version(body, REAKONTROL_GITHUB_LATEST_URL).unwrap();
        assert_eq!(version.raw(), "2026.2.16.100");
    }

    #[test]
    fn extracts_surge_xt_nightly_version_from_windows_setup_asset() {
        let version = surge_xt_version_from_windows_asset(
            "surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-setup.exe",
        )
        .unwrap();
        assert_eq!(version.raw(), "NIGHTLY-2026-05-05-a87bdb7");
        assert!(
            surge_xt_version_from_windows_asset(
                "surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-pluginsonly.zip"
            )
            .is_none(),
            "pluginsonly assets must not satisfy the windows-setup matcher"
        );
        assert!(
            surge_xt_version_from_windows_asset(
                "surge-xt-win64-juce7-NIGHTLY-2026-05-05-a87bdb7-pluginsonly.zip"
            )
            .is_none()
        );
    }

    #[test]
    fn extracts_surge_xt_nightly_version_from_macos_dmg_asset() {
        let version =
            surge_xt_version_from_macos_asset("surge-xt-macOS-NIGHTLY-2026-05-05-a87bdb7.dmg")
                .unwrap();
        assert_eq!(version.raw(), "NIGHTLY-2026-05-05-a87bdb7");
        assert!(
            surge_xt_version_from_macos_asset(
                "surge-xt-macos-NIGHTLY-2026-05-05-a87bdb7-pluginsonly.zip"
            )
            .is_none()
        );
    }

    #[test]
    fn parses_surge_xt_nightly_release_payload() {
        let body = r#"{
            "tag_name": "Nightly",
            "assets": [
                {"name": "surge-xt-linux-arm64-NIGHTLY-2026-05-05-a87bdb7.tar.gz"},
                {"name": "surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-pluginsonly.zip"},
                {"name": "surge-xt-win64-NIGHTLY-2026-05-05-a87bdb7-setup.exe"},
                {"name": "surge-xt-macOS-NIGHTLY-2026-05-05-a87bdb7.dmg"},
                {"name": "artifact_md5sum.txt"}
            ]
        }"#;
        let version = parse_surge_xt_nightly_release(body, SURGE_XT_NIGHTLY_URL).unwrap();
        assert_eq!(version.raw(), "NIGHTLY-2026-05-05-a87bdb7");
    }

    #[test]
    fn falls_back_to_surge_xt_macos_dmg_when_windows_asset_is_missing() {
        let body = r#"{
            "tag_name": "Nightly",
            "assets": [
                {"name": "surge-xt-macOS-NIGHTLY-2026-05-05-a87bdb7.dmg"}
            ]
        }"#;
        let version = parse_surge_xt_nightly_release(body, SURGE_XT_NIGHTLY_URL).unwrap();
        assert_eq!(version.raw(), "NIGHTLY-2026-05-05-a87bdb7");
    }

    #[test]
    fn rejects_surge_xt_release_with_no_matching_assets() {
        let body = r#"{
            "tag_name": "Nightly",
            "assets": [
                {"name": "surge-xt-linux-x86_64-NIGHTLY-2026-05-05-a87bdb7.tar.gz"},
                {"name": "artifact_md5sum.txt"}
            ]
        }"#;
        let error = parse_surge_xt_nightly_release(body, SURGE_XT_NIGHTLY_URL).unwrap_err();
        assert!(error.to_string().contains("Surge XT"));
    }

    #[test]
    fn rejects_reakontrol_release_with_no_matching_assets() {
        let body = r#"{"tag_name": "snapshots", "assets": [{"name": "README.md"}]}"#;
        let error =
            parse_reakontrol_snapshot_version(body, REAKONTROL_GITHUB_LATEST_URL).unwrap_err();
        assert!(error.to_string().contains("ReaKontrol"));
    }

    #[test]
    fn extracts_jaws_for_reaper_version_from_common_filenames() {
        let cases = [
            // Current upstream naming (single-integer build number).
            ("Reaper_JawsScripts_89.exe", "89"),
            // Historic / hypothetical dotted forms — kept covered so a
            // future rename to a semver-shaped scheme keeps working.
            ("JFRSCRIPTS_v3.18.exe", "3.18"),
            ("JFR_v3.18.0.exe", "3.18.0"),
            ("jaws-for-reaper-3.18.exe", "3.18"),
            ("JAWS_FOR_REAPER_3.18.0_release.exe", "3.18.0"),
        ];
        for (file_name, expected) in cases {
            let version = jaws_for_reaper_version_from_filename(file_name).unwrap();
            assert_eq!(version.raw(), expected, "filename: {file_name}");
        }
        assert!(jaws_for_reaper_version_from_filename("README.txt").is_none());
        assert!(jaws_for_reaper_version_from_filename("NoVersionHere.exe").is_none());
        // Non-.exe artifacts (e.g. a zip sibling) are ignored.
        assert!(jaws_for_reaper_version_from_filename("JFR_v3.18.zip").is_none());
    }

    #[test]
    fn picks_highest_jaws_for_reaper_version_from_hfs_listing() {
        let body = r#"{
            "list": [
                {"n": "Reaper_JawsScripts_88.exe", "s": 100},
                {"n": "Reaper_JawsScripts_89.exe", "s": 110},
                {"n": "Reaper_JawsScripts_85.exe", "s": 90},
                {"n": "old/", "s": null},
                {"n": "README.txt", "s": 5}
            ]
        }"#;
        let version = parse_jaws_for_reaper_listing(body, &jaws_for_reaper_listing_url()).unwrap();
        assert_eq!(version.raw(), "89");
    }

    #[test]
    fn rejects_jaws_for_reaper_listing_without_versioned_installer() {
        let body = r#"{"list": [{"n": "README.txt", "s": 1}]}"#;
        let error =
            parse_jaws_for_reaper_listing(body, &jaws_for_reaper_listing_url()).unwrap_err();
        assert!(error.to_string().contains("no versioned JAWS-for-REAPER"));
    }

    #[test]
    fn parses_gyan_release_version_text_payload() {
        assert_eq!(
            parse_ffmpeg_gyan_release_version("8.1.1\n", FFMPEG_GYAN_VERSION_URL)
                .unwrap()
                .raw(),
            "8.1.1"
        );
        assert_eq!(
            parse_ffmpeg_gyan_release_version("  8.1.1  ", FFMPEG_GYAN_VERSION_URL)
                .unwrap()
                .raw(),
            "8.1.1"
        );
        assert!(parse_ffmpeg_gyan_release_version("", FFMPEG_GYAN_VERSION_URL).is_err());
        assert!(
            parse_ffmpeg_gyan_release_version("not-a-version", FFMPEG_GYAN_VERSION_URL).is_err()
        );
    }

    #[test]
    fn extracts_ffmpeg_version_from_tordona_release_tag() {
        // tordona ships plain `<major>.<minor>.<patch>` tags.
        assert_eq!(
            ffmpeg_version_from_tordona_tag("8.1.1").unwrap().raw(),
            "8.1.1"
        );
        assert_eq!(
            ffmpeg_version_from_tordona_tag("7.1.4").unwrap().raw(),
            "7.1.4"
        );
        // Rolling tags and BtbN-style `n` prefixes are rejected.
        assert!(ffmpeg_version_from_tordona_tag("latest").is_none());
        assert!(ffmpeg_version_from_tordona_tag("daily-autobuild-2026.05.06.0").is_none());
        assert!(ffmpeg_version_from_tordona_tag("n8.1.1").is_none());
    }

    #[test]
    fn picks_highest_stable_n8_release_from_tordona_listing() {
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
                "tag_name": "7.1.4",
                "prerelease": false,
                "assets": []
            },
            {
                "tag_name": "8.0.2",
                "prerelease": false,
                "assets": []
            },
            {
                "tag_name": "8.1.1",
                "prerelease": false,
                "assets": [
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
                "tag_name": "9.0",
                "prerelease": true,
                "assets": []
            }
        ]"#;
        let release = pick_ffmpeg_tordona_release(body, FFMPEG_TORDONA_ARM64_RELEASES_URL, 8)
            .unwrap()
            .expect("an n8.x.y release should be selected");
        assert_eq!(release.version.raw(), "8.1.1");
        // The full-shared asset must remain in the picked release so the
        // artifact resolver can grab the `.browser_download_url`.
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == "ffmpeg-8.1.1-full-shared-win-arm64.7z")
            .expect("full-shared asset should still be carried through");
        assert_eq!(
            asset.url,
            "https://example.test/ffmpeg-8.1.1-full-shared-win-arm64.7z"
        );
    }

    #[test]
    fn errors_when_tordona_listing_has_no_supported_major() {
        let body = r#"[
            {"tag_name": "7.1.4", "prerelease": false, "assets": []},
            {"tag_name": "latest", "prerelease": true, "assets": []}
        ]"#;
        let release =
            pick_ffmpeg_tordona_release(body, FFMPEG_TORDONA_ARM64_RELEASES_URL, 8).unwrap();
        assert!(release.is_none());
    }
}
