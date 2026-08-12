use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;

use crate::error::{RabbitError, Result};
use crate::hfs::{HfsListEntry, fetch_file_list, parse_get_file_list_response};
use crate::package::{
    GithubReleaseSelector, GithubReleaseSpec, HfsListingSpec, VersionRule, VersionSource,
    WhatsNewRule, embedded_package_manifest,
};
use crate::plan::AvailablePackage;
use crate::version::Version;
use regex::Regex;

const USER_AGENT: &str = "RABBIT/0.1 (+https://github.com/Timtam/rabbit)";

pub const OSARA_UPDATE_URL: &str = "https://osara.reaperaccessibility.com/snapshots/update.json";
/// Gyan.dev's plain-text version stamp for the latest stable
/// `ffmpeg-release-full-shared.7z`. Returns a single line of UTF-8 like
/// `8.1.1` — no JSON, no HTML scraping. We use Gyan as the canonical
/// version source for FFmpeg (and as the x64 artifact source) because
/// BtbN doesn't publish stable tagged releases — only rolling
/// autobuilds — and Gyan is also winget's upstream for FFmpeg.
pub const FFMPEG_GYAN_VERSION_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z.ver";
/// `tordona/ffmpeg-win-arm64` GitHub releases — the ARM64 source for
/// FFmpeg's data-driven `http_artifact` `github_release_max_major` target.
/// Tags are plain `<major>.<minor>.<patch>` (no `n` prefix); we pick
/// the highest non-prerelease tag whose major matches
/// [`FFMPEG_SUPPORTED_MAJOR`].
pub const FFMPEG_TORDONA_ARM64_RELEASES_URL: &str =
    "https://api.github.com/repos/tordona/ffmpeg-win-arm64/releases?per_page=100";

/// Head-fetch budget for a `TextChangelog` What's-New source. REAPER's
/// `whatsnew.txt` is the full release history (~1.4 MB) with the newest
/// version on top. The notes can span every release between the installed
/// version and the newest one (up to [`WHATS_NEW_MAX_SECTIONS`] sections,
/// REAPER's running several KB each), so the budget covers a healthy run of
/// sections — requested via a `Range` header (which reaper.fm honors) and
/// capped on the read regardless, for servers that ignore `Range` and reply
/// with the whole file.
const WHATS_NEW_HEAD_BYTES: u64 = 256 * 1024;

/// Cap on how many release sections a since-installed What's-New renders —
/// a years-old install must not turn the details pane into tens of screens
/// a screen reader user has to wade through.
const WHATS_NEW_MAX_SECTIONS: usize = 10;

/// FFmpeg major version that REAPER's video decoder is known to support.
/// Bump this when a new REAPER release adds support for the next FFmpeg
/// major (e.g. REAPER 7.66 added FFmpeg 8 support → pinned to `8`). The
/// detector and the latest-version provider both reference this so a
/// single bump tracks both code paths.
pub const FFMPEG_SUPPORTED_MAJOR: u64 = 8;

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
    // Every package resolves its version data-driven from its manifest: a
    // `version` rule (HTML / JSON / plain-text snowflakes), a `github_release`
    // block, or an `hfs_listing` block (JAWS). Per-package failure tolerance —
    // one unreachable upstream doesn't sink the rest.
    for spec in embedded_package_manifest().packages {
        let Some(result) = resolve_manifest_version(&client, &spec) else {
            continue;
        };
        match result {
            // The batch path (CLI update check, offline-tolerant model load)
            // only needs versions; What's-New notes are fetched by the GUI's
            // per-package deferred check via fetch_latest_details_for_package.
            Ok(version) => packages.push(AvailablePackage {
                package_id: spec.id.clone(),
                version: Some(version),
                whats_new: None,
            }),
            Err(error) => failures.push(LatestVersionFailure {
                package_id: spec.id.clone(),
                message: error.to_string(),
            }),
        }
    }
    Ok(LatestVersionsReport { packages, failures })
}

/// Resolve a package's latest version from its manifest's data-driven source:
/// a `version` rule, a `github_release` block, or an `hfs_listing` block.
/// Returns `None` only for a package that declares no version source at all.
fn resolve_manifest_version(
    client: &Client,
    spec: &crate::package::EmbeddedPackageSpec,
) -> Option<Result<Version>> {
    if let Some(rule) = &spec.version {
        Some(resolve_version_rule(client, rule))
    } else if let Some(github_release) = &spec.github_release {
        let url = github_release_url(github_release);
        Some(
            http_get_text(client, &url)
                .and_then(|body| resolve_github_version(&body, &url, github_release)),
        )
    } else {
        spec.hfs_listing
            .as_ref()
            .map(|hfs| resolve_hfs_listing_version(client, hfs))
    }
}

/// Version-side of an [`HfsListingSpec`]: read the folder listing and return
/// the highest-version installer it advertises. Shares [`fetch_file_list`] +
/// [`pick_jaws_for_reaper_version`] with the artifact side.
pub(crate) fn resolve_hfs_listing_version(
    client: &Client,
    spec: &HfsListingSpec,
) -> Result<Version> {
    let entries = fetch_file_list(client, &spec.base, &spec.folder)?;
    pick_jaws_for_reaper_version(&entries)
        .map(|(version, _)| version)
        .ok_or_else(|| RabbitError::RemoteData {
            url: hfs_listing_error_url(spec),
            message: "no versioned installer in HFS folder listing".to_string(),
        })
}

/// The URL reported in `RemoteData` errors for an HFS listing, kept stable
/// regardless of the HTTP verb the fetch used.
pub(crate) fn hfs_listing_error_url(spec: &HfsListingSpec) -> String {
    format!(
        "{}/~/api/get_file_list?path={}",
        spec.base.trim_end_matches('/'),
        spec.folder
    )
}

/// Fetch the latest version for a single package. Useful when a UI wants to
/// stream per-package results as they arrive instead of blocking on the full
/// batch.
pub fn fetch_latest_for_package(package_id: &str) -> Result<Version> {
    fetch_latest_details_for_package(package_id, None).map(|details| details.version)
}

/// Latest-version details for one package: the resolved version plus the
/// package's rendered What's-New notes, when it declares a `whats_new`
/// source and that source resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestPackageDetails {
    pub version: Version,
    pub whats_new: Option<String>,
}

/// Like [`fetch_latest_for_package`], but also resolves the package's
/// optional What's-New notes (the manifest's `whats_new` block). Notes are
/// best-effort: any notes-side failure just yields `whats_new: None`, so
/// release notes can never break the version check itself.
///
/// With `installed`, the notes are trimmed to the changes since that
/// version (each engine documents its own cut rule); without it, they cover
/// the newest release only.
pub fn fetch_latest_details_for_package(
    package_id: &str,
    installed: Option<&Version>,
) -> Result<LatestPackageDetails> {
    let manifest = embedded_package_manifest();
    let spec = manifest
        .packages
        .iter()
        .find(|spec| spec.id == package_id)
        .ok_or_else(|| RabbitError::RemoteData {
            url: String::new(),
            message: format!("no package named {package_id}"),
        })?;
    let client = build_http_client()?;
    // When the version rule and the What's-New rule read the same URL
    // (OSARA's update.json carries both the version and the commit feed),
    // fetch the body once and feed both parsers from it instead of hitting
    // the endpoint twice.
    if let (
        Some(VersionRule::Json {
            url: version_url,
            pointer: version_pointer,
        }),
        Some(WhatsNewRule::JsonCommits {
            url: notes_url,
            pointer: notes_pointer,
        }),
    ) = (&spec.version, &spec.whats_new)
        && version_url == notes_url
    {
        let body = http_get_text(&client, version_url)?;
        let version = resolve_json_version(&body, version_url, version_pointer)?;
        let whats_new = resolve_json_commits(&body, notes_url, notes_pointer, installed).ok();
        return Ok(LatestPackageDetails { version, whats_new });
    }
    // Every package resolves its version data-driven: a `version` VersionRule
    // (REAPER/OSARA/SWS/FFmpeg), a `github_release` block (Surge XT, ReaKontrol,
    // ReaPack, app2clap), or an `hfs_listing` block (JAWS).
    let version = resolve_manifest_version(&client, spec).unwrap_or_else(|| {
        Err(RabbitError::RemoteData {
            url: String::new(),
            message: format!("no latest-version source configured for package {package_id}"),
        })
    })?;
    let whats_new = spec
        .whats_new
        .as_ref()
        .and_then(|rule| resolve_whats_new_rule(&client, rule, installed).ok());
    Ok(LatestPackageDetails { version, whats_new })
}

/// Resolve a data-driven [`WhatsNewRule`] into the rendered notes text shown
/// under the What's-New heading in the wizard's package-details pane. With
/// `installed`, each engine trims the notes to the changes since that
/// version; every trim keeps at least the newest release's notes, so an
/// up-to-date install reads the same thing it did before trimming existed.
pub fn resolve_whats_new_rule(
    client: &Client,
    rule: &WhatsNewRule,
    installed: Option<&Version>,
) -> Result<String> {
    match rule {
        WhatsNewRule::JsonCommits { url, pointer } => {
            let body = http_get_text(client, url)?;
            resolve_json_commits(&body, url, pointer, installed)
        }
        WhatsNewRule::TextChangelog { url, section_start } => {
            let body = http_get_text_head(client, url, WHATS_NEW_HEAD_BYTES)?;
            resolve_changelog_sections(&body, url, section_start, installed)
        }
        WhatsNewRule::GithubReleaseBody { url } => {
            let body = http_get_text(client, url)?;
            resolve_github_release_bodies(&body, url, installed)
        }
    }
}

/// Release-notes side of [`resolve_whats_new_rule`], split out so it's
/// unit-testable on a fixture body without an HTTP fetch. Accepts either a
/// newest-first release array (a `/releases` list endpoint) or a single
/// release object (a `/releases/latest` endpoint). Each release renders as
/// its name (tag fallback) on a heading line — so the notes read as a
/// versioned changelog — followed by the body the maintainer wrote.
///
/// With an installed version, the walk keeps every release above it, capped
/// at [`WHATS_NEW_MAX_SECTIONS`]; the newest release always renders, so an
/// up-to-date install still sees the latest notes. Drafts and prereleases
/// are skipped — `/releases/latest` never returns them, and the list walk
/// must match it.
pub(crate) fn resolve_github_release_bodies(
    body: &str,
    url: &str,
    installed: Option<&Version>,
) -> Result<String> {
    let value: Value = serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
        url: url.to_string(),
        message: source.to_string(),
    })?;
    let releases: Vec<&Value> = match &value {
        Value::Array(entries) => entries.iter().collect(),
        single => vec![single],
    };
    let mut sections: Vec<String> = Vec::new();
    for release in releases {
        let flagged = |field: &str| {
            release
                .pointer(field)
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        if flagged("/draft") || flagged("/prerelease") {
            continue;
        }
        if !sections.is_empty() {
            let stop = match installed {
                // Fresh install: no since-version to reach back to — the
                // newest release only, as before trimming existed.
                None => true,
                Some(installed) => {
                    sections.len() >= WHATS_NEW_MAX_SECTIONS
                        || release_version(release)
                            .is_some_and(|version| version_at_or_below(&version, installed))
                }
            };
            if stop {
                break;
            }
        }
        if let Some(section) = render_release_section(release) {
            sections.push(section);
        }
    }
    let rendered = sections.join("\n\n");
    if rendered.is_empty() {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: "no renderable release notes".to_string(),
        });
    }
    Ok(rendered)
}

/// One release as a What's-New section: the release name (tag fallback) as
/// a heading line, then the body with Markdown list markers (`- `, `* `)
/// rendered as the same bullets the commit list uses and other lines left
/// verbatim. `None` when the release carries neither a usable heading nor
/// any body text.
fn render_release_section(release: &Value) -> Option<String> {
    let string_field = |field: &str| {
        release
            .pointer(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
    };
    let heading = string_field("/name").or_else(|| string_field("/tag_name"));
    let notes = release
        .pointer("/body")
        .and_then(Value::as_str)
        .map(render_release_body_lines)
        .filter(|text| !text.is_empty());
    match (heading, notes) {
        (Some(heading), Some(notes)) => Some(format!("{heading}\n{notes}")),
        (Some(heading), None) => Some(heading.to_string()),
        (None, Some(notes)) => Some(notes),
        (None, None) => None,
    }
}

/// The maintainer-written release body with Markdown list markers rendered
/// as bullets, other lines verbatim, surrounding whitespace trimmed.
fn render_release_body_lines(notes: &str) -> String {
    let lines: Vec<String> = notes
        .lines()
        .map(|line| {
            let line = line.trim_end();
            match line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
                Some(item) => format!("• {item}"),
                None => line.to_string(),
            }
        })
        .collect();
    lines.join("\n").trim().to_string()
}

/// The version a release advertises via its `tag_name` (`v1.2.6`). `None`
/// when the tag doesn't parse as a version, which the release walk treats
/// as "not a cut-off point".
fn release_version(release: &Value) -> Option<Version> {
    let tag = release.pointer("/tag_name").and_then(Value::as_str)?;
    Version::parse(tag).ok()
}

/// Commit-list side of [`resolve_whats_new_rule`], split out so it's
/// unit-testable on a fixture body without an HTTP fetch. Reads the array of
/// `[sha, message]` pairs at `pointer` (newest first) and renders one
/// bulleted line per commit, keeping only each message's first line — full
/// bodies can run to paragraphs, and the details pane wants a scannable
/// list.
///
/// OSARA-style snapshot versions embed the built commit's hash after a
/// comma (`2026.4.16.2157,593ff26b`); when `installed` carries one and it
/// appears in the feed, the list stops there so the user only hears commits
/// they don't already run. A trim that would leave nothing (the install is
/// current) falls back to the full list, matching what an up-to-date user
/// saw before trimming existed; no hash, or a hash that never appears, also
/// renders the full list.
pub(crate) fn resolve_json_commits(
    body: &str,
    url: &str,
    pointer: &str,
    installed: Option<&Version>,
) -> Result<String> {
    let value: Value = serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
        url: url.to_string(),
        message: source.to_string(),
    })?;
    let entries = value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| RabbitError::RemoteData {
            url: url.to_string(),
            message: format!("missing commit array at JSON pointer {pointer:?}"),
        })?;
    let cut = installed
        .and_then(installed_snapshot_hash)
        .and_then(|hash| {
            entries.iter().position(|entry| {
                entry
                    .as_array()
                    .and_then(|pair| pair.first())
                    .and_then(Value::as_str)
                    .is_some_and(|sha| commit_sha_matches(sha, &hash))
            })
        });
    let mut lines = render_commit_lines(&entries[..cut.unwrap_or(entries.len())]);
    if lines.is_empty() && cut.is_some() {
        lines = render_commit_lines(entries);
    }
    if lines.is_empty() {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: format!("no commit messages at JSON pointer {pointer:?}"),
        });
    }
    Ok(lines.join("\n"))
}

/// Render `[sha, message]` pairs as cleaned bulleted lines — the shared body
/// of [`resolve_json_commits`]'s trimmed and full-list passes.
///
/// The list is lightly cleaned for end users (a screen reader reads every
/// character out loud): trailing GitHub issue/PR references are stripped,
/// commits using OSARA's `dev:` prefix convention for developer-facing work
/// are skipped, and repeated chore lines ("Update translations.") keep only
/// their first occurrence.
fn render_commit_lines(entries: &[Value]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut lines: Vec<String> = Vec::new();
    for message in entries
        .iter()
        .filter_map(|entry| entry.as_array()?.get(1)?.as_str())
    {
        let Some(first_line) = message.lines().next() else {
            continue;
        };
        let cleaned = strip_trailing_issue_refs(first_line.trim());
        if cleaned.is_empty()
            || cleaned
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("dev:"))
        {
            continue;
        }
        if seen.insert(cleaned.to_string()) {
            lines.push(format!("• {cleaned}"));
        }
    }
    lines
}

/// The commit hash an OSARA-style snapshot version embeds after a comma
/// (`2026.4.16.2157,593ff26b` → `593ff26b`). `None` when the version has no
/// comma segment or the segment doesn't look like a git hash prefix — a
/// short or non-hex segment must not become a trim key, so those render the
/// full list instead of a wrongly-cut one.
fn installed_snapshot_hash(installed: &Version) -> Option<String> {
    let (_, hash) = installed.raw().rsplit_once(',')?;
    let hash = hash.trim();
    (hash.len() >= 7 && hash.chars().all(|ch| ch.is_ascii_hexdigit()))
        .then(|| hash.to_ascii_lowercase())
}

/// Whether feed sha `sha` names the same commit as the (possibly shorter)
/// installed hash prefix.
fn commit_sha_matches(sha: &str, installed_hash: &str) -> bool {
    sha.get(..installed_hash.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(installed_hash))
}

/// Strip GitHub issue/PR references from the end of a commit's first line —
/// `(#1431)`, `(issue #1410, PR #1416)`, `Closes #1424`, `Fixes #997.` —
/// repeatedly, so a combination like `(#1426) Closes #1424` fully unwinds.
/// The references are pure noise when read by a screen reader; anyone
/// chasing a change down goes to the repository anyway. Mid-sentence
/// references are left alone.
fn strip_trailing_issue_refs(line: &str) -> &str {
    let mut cleaned = line.trim_end();
    loop {
        let before = cleaned;
        if let Some(found) = trailing_paren_ref_regex().find(cleaned) {
            cleaned = cleaned[..found.start()].trim_end();
        }
        if let Some(found) = trailing_bare_ref_regex().find(cleaned) {
            cleaned = cleaned[..found.start()].trim_end();
        }
        if cleaned == before {
            return cleaned;
        }
    }
}

/// A trailing parenthesized reference group: `(#1431)`, `(issue #1401, PR
/// #1402)` — only digits and reference keywords inside the parentheses, so a
/// meaningful trailing parenthetical is never eaten.
fn trailing_paren_ref_regex() -> &'static Regex {
    static TRAILING_PAREN_REF: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    TRAILING_PAREN_REF.get_or_init(|| {
        Regex::new(r"(?i)\s*\(\s*(?:(?:issue|pr|closes|fixes|re)?\s*#\d+\s*,?\s*)+\)\.?\s*$")
            .expect("static trailing paren ref regex is valid")
    })
}

/// A trailing bare reference: `Closes #1424`, `Fixes #997.`, `Re #1406`.
fn trailing_bare_ref_regex() -> &'static Regex {
    static TRAILING_BARE_REF: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    TRAILING_BARE_REF.get_or_init(|| {
        Regex::new(r"(?i)\s*\b(?:closes|fixes|re)\s+#\d+\.?\s*$")
            .expect("static trailing bare ref regex is valid")
    })
}

/// Changelog side of [`resolve_whats_new_rule`], split out so it's
/// unit-testable on a fixture body. Cuts the newest run of release sections
/// out of a newest-first plain-text changelog: from the first line matching
/// `section_start` down to, excluding, the first later section whose header
/// version is at or below `installed` — everything from there on is what
/// the user already runs. The newest section is always kept, the run is
/// capped at [`WHATS_NEW_MAX_SECTIONS`], and without an installed version
/// the run is the newest section only. When no cut-off header shows up
/// (e.g. the head-capped fetch ended first), whatever was fetched is kept.
pub(crate) fn resolve_changelog_sections(
    body: &str,
    url: &str,
    section_start: &str,
    installed: Option<&Version>,
) -> Result<String> {
    // `(?m:…)` forces multi-line mode so `^` in the manifest pattern anchors
    // to line starts without every manifest having to remember the flag.
    let regex =
        Regex::new(&format!("(?m:{section_start})")).map_err(|err| RabbitError::RemoteData {
            url: url.to_string(),
            message: format!("invalid section_start regex {section_start:?}: {err}"),
        })?;
    let starts: Vec<usize> = regex.find_iter(body).map(|found| found.start()).collect();
    let Some(&first) = starts.first() else {
        return Err(RabbitError::RemoteData {
            url: url.to_string(),
            message: format!("section pattern {section_start:?} did not match"),
        });
    };
    let mut end = body.len();
    // `sections_kept` is the enumeration index: cutting at header N keeps
    // the N sections before it.
    for (sections_kept, &start) in starts.iter().enumerate().skip(1) {
        let stop = match installed {
            // Fresh install: no since-version to reach back to — the newest
            // section only, as before trimming existed.
            None => true,
            Some(installed) => {
                sections_kept >= WHATS_NEW_MAX_SECTIONS
                    || section_header_version(&body[start..])
                        .is_some_and(|version| version_at_or_below(&version, installed))
            }
        };
        if stop {
            end = start;
            break;
        }
    }
    Ok(body[first..end].trim_end().to_string())
}

/// The version a changelog section header advertises: the first
/// whitespace-delimited token starting at the header line's first digit —
/// `7.78` from REAPER's `v7.78 - July 18 2026`, `2.14.0.7` from SWS's
/// `!v2.14.0.7 featured build (Dec 25, 2024)`. `None` when the line has no
/// parsable version, which the section cut treats as "not a cut-off point".
fn section_header_version(section: &str) -> Option<Version> {
    let header_line = section.lines().next()?;
    let start = header_line.find(|ch: char| ch.is_ascii_digit())?;
    let token = header_line[start..].split_whitespace().next()?;
    Version::parse(token).ok()
}

/// Whether `candidate` is numerically at or below `installed` — the cut-off
/// test for since-installed What's-New trimming. Compares numeric parts
/// only, so formatting differences between a header/tag and a detected
/// version (`v1.2.6` vs `1.2.6`) can't skew the comparison the way
/// [`Version::cmp_lenient`]'s raw-string tie-break would.
fn version_at_or_below(candidate: &Version, installed: &Version) -> bool {
    let candidate_parts = candidate.numeric_parts();
    let installed_parts = installed.numeric_parts();
    let length = candidate_parts.len().max(installed_parts.len());
    for index in 0..length {
        let candidate_part = candidate_parts.get(index).copied().unwrap_or_default();
        let installed_part = installed_parts.get(index).copied().unwrap_or_default();
        match candidate_part.cmp(&installed_part) {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering.is_lt(),
        }
    }
    true
}

/// Resolve a data-driven [`VersionRule`]: fetch its URL and extract a version
/// via regex (HTML), a JSON pointer, or a plain-text trim. This is the
/// generic replacement for the per-package HTML/JSON/plain-text version
/// parsers (REAPER, OSARA, SWS, FFmpeg).
pub fn resolve_version_rule(client: &Client, rule: &VersionRule) -> Result<Version> {
    match rule {
        VersionRule::PlainText { url } => {
            let body = http_get_text(client, url)?;
            resolve_plaintext_version(&body, url)
        }
        VersionRule::Json { url, pointer } => {
            let body = http_get_text(client, url)?;
            resolve_json_version(&body, url, pointer)
        }
        VersionRule::Html {
            url,
            pattern,
            format,
        } => {
            let body = http_get_text(client, url)?;
            resolve_html_version(&body, url, pattern, format)
        }
        VersionRule::ContentHash { url } => {
            let bytes = http_get_bytes(client, url)?;
            Ok(content_hash_version(&bytes))
        }
    }
}

/// The "version" of a [`VersionRule::ContentHash`] artifact: the SHA-256 of
/// its bytes. Only ever compared for equality (see
/// [`VersionComparison::Exact`]), so the short prefix is plenty to
/// distinguish releases while staying readable in the UI and reports. The
/// `h` prefix keeps [`Version::parse`] happy for the astronomically unlikely
/// all-letter digest prefix, and marks the value as a digest rather than a
/// number wherever it's displayed.
pub(crate) fn content_hash_version(bytes: &[u8]) -> Version {
    let digest = crate::hash::sha256_bytes(bytes);
    Version::parse(format!("h{}", &digest[..12]))
        .unwrap_or_else(|_| Version::parse("h0").expect("literal is a valid version"))
}

fn http_get_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    let request = crate::http::maybe_apply_github_auth(client.get(url), url);
    let response = request
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|source| RabbitError::Http {
            url: url.to_string(),
            source,
        })?;

    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|source| RabbitError::Http {
            url: url.to_string(),
            source,
        })
}

/// Plain-text-side of [`resolve_version_rule`], split out so it's unit-testable
/// on a fixture body without an HTTP fetch. Trims the body and parses it as a
/// version — covers FFmpeg's Gyan `*.ver` endpoint (a single line like `8.1.1`).
pub(crate) fn resolve_plaintext_version(body: &str, url: &str) -> Result<Version> {
    let trimmed = body.trim();
    Version::parse(trimmed).map_err(|_| RabbitError::RemoteData {
        url: url.to_string(),
        message: format!("response is not a version: {trimmed:?}"),
    })
}

/// JSON-side of [`resolve_version_rule`], split out so it's unit-testable on a
/// fixture body without an HTTP fetch. Reads the string at the RFC 6901
/// `pointer` and parses it as a version — covers OSARA's `update.json`
/// (`/version`), whose value can carry a trailing `,<gitsha>` that
/// [`Version::parse`] tolerates.
pub(crate) fn resolve_json_version(body: &str, url: &str, pointer: &str) -> Result<Version> {
    let value: Value = serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
        url: url.to_string(),
        message: source.to_string(),
    })?;
    let raw = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| RabbitError::RemoteData {
            url: url.to_string(),
            message: format!("missing string at JSON pointer {pointer:?}"),
        })?;
    Version::parse(raw).map_err(|_| RabbitError::RemoteData {
        url: url.to_string(),
        message: format!("value at {pointer:?} is not a version: {raw:?}"),
    })
}

/// HTML-side of [`resolve_version_rule`], split out so it's unit-testable on
/// a fixture body without an HTTP fetch.
pub(crate) fn resolve_html_version(
    body: &str,
    url: &str,
    pattern: &str,
    format: &str,
) -> Result<Version> {
    let regex = Regex::new(pattern).map_err(|err| RabbitError::RemoteData {
        url: url.to_string(),
        message: format!("invalid version regex {pattern:?}: {err}"),
    })?;
    let captures = regex
        .captures(body)
        .ok_or_else(|| RabbitError::RemoteData {
            url: url.to_string(),
            message: format!("version pattern {pattern:?} did not match"),
        })?;
    let rendered = render_capture_format(format, &captures);
    Version::parse(rendered.trim()).map_err(|_| RabbitError::RemoteData {
        url: url.to_string(),
        message: format!("extracted version is invalid: {rendered:?}"),
    })
}

/// Replace `{N}` placeholders in `format` with capture group N (empty when
/// the group didn't participate). E.g. `"{1}.{2}"` over SWS's base + build.
fn render_capture_format(format: &str, captures: &regex::Captures<'_>) -> String {
    placeholder_regex()
        .replace_all(format, |slot: &regex::Captures<'_>| {
            slot[1]
                .parse::<usize>()
                .ok()
                .and_then(|index| captures.get(index))
                .map(|group| group.as_str())
                .unwrap_or("")
                .to_string()
        })
        .into_owned()
}

fn placeholder_regex() -> &'static Regex {
    static PLACEHOLDER: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PLACEHOLDER.get_or_init(|| Regex::new(r"\{(\d+)\}").expect("static placeholder regex is valid"))
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

/// GET at most `max_bytes` of `url`. The request carries a `Range` header so
/// a cooperating server (reaper.fm does) only sends the head; the reader is
/// capped at `max_bytes` regardless, so a server that ignores `Range` and
/// replies 200 with the full body still costs at most one buffer. When the
/// cap was actually hit, the final — almost certainly truncated — line is
/// dropped so callers only ever see whole lines.
fn http_get_text_head(client: &Client, url: &str, max_bytes: u64) -> Result<String> {
    let request = crate::http::maybe_apply_github_auth(client.get(url), url)
        .header(reqwest::header::RANGE, format!("bytes=0-{}", max_bytes - 1));
    let response = request
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|source| RabbitError::Http {
            url: url.to_string(),
            source,
        })?;

    let mut body = Vec::new();
    response
        .take(max_bytes)
        .read_to_end(&mut body)
        .map_err(|source| RabbitError::RemoteData {
            url: url.to_string(),
            message: source.to_string(),
        })?;
    let capped = body.len() as u64 == max_bytes;
    let mut text = String::from_utf8_lossy(&body).into_owned();
    if capped {
        match text.rfind('\n') {
            Some(last_newline) => text.truncate(last_newline),
            None => text.clear(),
        }
    }
    Ok(text)
}

/// The GitHub API URL a [`GithubReleaseSpec`] reads — `/releases/latest`
/// for a `Latest` selector or `/releases/tags/<tag>` for a fixed/rolling
/// tag. (For app2clap this reproduces the former `APP2CLAP_GITHUB_LATEST_URL`
/// byte-for-byte, so the GitHub-auth URL match still applies.)
pub fn github_release_url(spec: &GithubReleaseSpec) -> String {
    match &spec.release {
        GithubReleaseSelector::Latest => {
            format!("https://api.github.com/repos/{}/releases/latest", spec.repo)
        }
        GithubReleaseSelector::Tag(tag) => {
            format!(
                "https://api.github.com/repos/{}/releases/tags/{}",
                spec.repo, tag
            )
        }
    }
}

/// Resolve the latest version for a data-driven GitHub-release package. For
/// `TagName` the version is the release `tag_name`; for `AssetName` it is the
/// highest `Version` across asset filenames matching the prefix/suffix (the
/// rolling-tag assets are not in version order, so we compare all of them —
/// the same logic the hand-written ReaKontrol/app2clap parsers used).
pub fn resolve_github_version(body: &str, url: &str, spec: &GithubReleaseSpec) -> Result<Version> {
    let value: Value = serde_json::from_str(body).map_err(|source| RabbitError::RemoteData {
        url: url.to_string(),
        message: source.to_string(),
    })?;
    match &spec.version_from {
        VersionSource::TagName { strip_v_prefix } => {
            let tag = value
                .get("tag_name")
                .and_then(Value::as_str)
                .ok_or_else(|| RabbitError::RemoteData {
                    url: url.to_string(),
                    message: "missing string field: tag_name".to_string(),
                })?;
            let tag = if *strip_v_prefix {
                tag.trim_start_matches('v')
            } else {
                tag
            };
            Version::parse(tag).map_err(|_| RabbitError::RemoteData {
                url: url.to_string(),
                message: format!("tag_name is not a version: {tag:?}"),
            })
        }
        VersionSource::AssetName {
            strip_trailing_dot_segment,
        } => {
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
                let Some(version) =
                    github_asset_version(name, &spec.assets, *strip_trailing_dot_segment)
                else {
                    continue;
                };
                latest = Some(match latest {
                    Some(current) if current.cmp_lenient(&version).is_ge() => current,
                    _ => version,
                });
            }
            latest.ok_or_else(|| RabbitError::RemoteData {
                url: url.to_string(),
                message: "no asset matched any selector's prefix/suffix".to_string(),
            })
        }
    }
}

/// The [`Version`] of `asset_name`, extracted via the first prefix/suffix
/// [`AssetSelector`] it matches. Used by both the version side (which scans
/// every asset across all platforms) and the artifact side (which already
/// knows the matched selector). Returns `None` for assets that match no
/// prefix/suffix selector — e.g. `exact_name`-only selectors, which pair
/// with `VersionSource::TagName` rather than `AssetName`.
pub(crate) fn github_asset_version(
    asset_name: &str,
    selectors: &[crate::package::AssetSelector],
    strip_trailing_dot_segment: bool,
) -> Option<Version> {
    let selector = selectors
        .iter()
        .find(|selector| selector.matches_asset(asset_name))?;
    let prefix = selector.name_prefix.as_deref()?;
    let suffix = selector.name_suffix.as_deref().unwrap_or("");
    version_from_asset_name(asset_name, prefix, suffix, strip_trailing_dot_segment)
}

/// Extract a [`Version`] from an asset filename of the form
/// `<prefix><version>[.<trailing>]<suffix>`, e.g.
/// `app2clap_2026.5.17.34.b6f558cf.zip` → `2026.5.17.34` (prefix
/// `app2clap_`, suffix `.zip`, trailing `.<shorthash>` dropped). Returns
/// `None` when the name doesn't match. This is the parameterized form of
/// the former `app2clap_version_from_asset_name` / `reakontrol_version_from_asset_name`.
pub(crate) fn version_from_asset_name(
    name: &str,
    prefix: &str,
    suffix: &str,
    strip_trailing_dot_segment: bool,
) -> Option<Version> {
    let core = name.strip_prefix(prefix)?.strip_suffix(suffix)?;
    let token = if strip_trailing_dot_segment {
        core.rsplit_once('.').map(|(left, _trailing)| left)?
    } else {
        core
    };
    Version::parse(token).ok()
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

#[cfg(test)]
mod tests {
    use super::{
        FFMPEG_GYAN_VERSION_URL, FFMPEG_TORDONA_ARM64_RELEASES_URL, OSARA_UPDATE_URL,
        ffmpeg_version_from_tordona_tag, github_release_url, jaws_for_reaper_version_from_filename,
        parse_jaws_for_reaper_listing, pick_ffmpeg_tordona_release, resolve_github_version,
        resolve_json_version, resolve_plaintext_version, version_from_asset_name,
    };
    use crate::package::{
        AssetSelector, GithubArtifactKind, GithubReleaseSelector, GithubReleaseSpec,
        SupportedPlatform, VersionSource,
    };
    use crate::version::Version;

    /// The app2clap manifest's `github_release` block, mirrored as a literal
    /// for the data-driven engine tests (asset-name versioning on a rolling
    /// `snapshots` tag; the prefix/suffix live on the asset selector).
    fn app2clap_github_spec() -> GithubReleaseSpec {
        GithubReleaseSpec {
            repo: "jcsteh/app2clap".to_string(),
            release: GithubReleaseSelector::Tag("snapshots".to_string()),
            version_from: VersionSource::AssetName {
                strip_trailing_dot_segment: true,
            },
            assets: vec![AssetSelector {
                platform: SupportedPlatform::Windows,
                arch: None,
                name_prefix: Some("app2clap_".to_string()),
                name_suffix: Some(".zip".to_string()),
                exact_name: None,
                artifact_kind: None,
            }],
            artifact_kind: Some(GithubArtifactKind::Archive),
        }
    }

    #[test]
    fn resolves_json_version_from_osara_update_json() {
        // OSARA's update.json `/version` carries a trailing `,<gitsha>` that
        // Version::parse keeps verbatim.
        let version = resolve_json_version(
            r#"{"version":"2026.4.16.2157,593ff26b"}"#,
            OSARA_UPDATE_URL,
            "/version",
        )
        .unwrap();
        assert_eq!(version.raw(), "2026.4.16.2157,593ff26b");
        // A missing pointer and a non-version value are errors, not panics.
        assert!(resolve_json_version(r#"{"nope":"x"}"#, OSARA_UPDATE_URL, "/version").is_err());
        assert!(
            resolve_json_version(
                r#"{"version":"not-a-version"}"#,
                OSARA_UPDATE_URL,
                "/version"
            )
            .is_err()
        );
    }

    #[test]
    fn renders_osara_commits_as_a_cleaned_bulleted_list() {
        // OSARA's update.json `commits` is an array of [sha, message] pairs.
        // The rendering keeps each message's first line only (CRLF bodies can
        // run to paragraphs), strips trailing GitHub issue/PR references,
        // skips OSARA's "dev:"-prefixed internal commits, and keeps only the
        // first occurrence of repeated chore lines.
        let body = concat!(
            r#"{"version":"2026.8.1.2278,857265da","commits":["#,
            r#"["857265da","Fix the slider. (#1431)\r\n\r\nLong body paragraph."],"#,
            r#"["342b988d","dev: Logging improvements. (#1430)"],"#,
            r#"["64ee308c","Add vertical zoom feedback (#1426) Closes #1424"],"#,
            r#"["11111111","Update translations."],"#,
            r#"["22222222","Map lang pack to nb_NO translation. Fixes #997."],"#,
            r#"["33333333","Update translations."],"#,
            r#"["44444444","Localise send parameters. (issue #1411, PR #1412)"]"#,
            "]}"
        );
        let notes = super::resolve_json_commits(body, OSARA_UPDATE_URL, "/commits", None).unwrap();
        assert_eq!(
            notes,
            "• Fix the slider.\n\
             • Add vertical zoom feedback\n\
             • Update translations.\n\
             • Map lang pack to nb_NO translation.\n\
             • Localise send parameters."
        );

        // A mid-sentence reference is not a trailing one — left alone.
        let body = r#"{"commits":[["aa","Fix the #5 slot (see #12) in the mixer"]]}"#;
        let notes = super::resolve_json_commits(body, OSARA_UPDATE_URL, "/commits", None).unwrap();
        assert_eq!(notes, "• Fix the #5 slot (see #12) in the mixer");

        // A missing pointer and an empty list are errors (the caller treats
        // any notes-side error as "no What's-New section"), not panics.
        let missing =
            super::resolve_json_commits(r#"{"nope":1}"#, OSARA_UPDATE_URL, "/commits", None);
        assert!(missing.is_err());
        let empty =
            super::resolve_json_commits(r#"{"commits":[]}"#, OSARA_UPDATE_URL, "/commits", None);
        assert!(empty.is_err());
    }

    #[test]
    fn trims_osara_commits_to_those_after_the_installed_snapshot() {
        // The installed snapshot version embeds its commit hash after the
        // comma; the feed is newest-first, so the list stops right before
        // that commit — the user already runs it and everything older.
        let body = concat!(
            r#"{"commits":["#,
            r#"["857265da11112222","Fix the slider."],"#,
            r#"["64ee308c33334444","Add vertical zoom feedback"],"#,
            r#"["13560ef755556666","Improve track naming"],"#,
            r#"["aabbccdd77778888","Older change"]"#,
            "]}"
        );
        let installed = Version::parse("2024.3.6.1332,13560ef7").unwrap();
        let notes =
            super::resolve_json_commits(body, OSARA_UPDATE_URL, "/commits", Some(&installed))
                .unwrap();
        assert_eq!(notes, "• Fix the slider.\n• Add vertical zoom feedback");

        // A trim that would leave nothing — the newest feed commit is the
        // installed one — falls back to the full list, matching what an
        // up-to-date user saw before trimming existed.
        let installed = Version::parse("2026.8.1.2278,857265da").unwrap();
        let notes =
            super::resolve_json_commits(body, OSARA_UPDATE_URL, "/commits", Some(&installed))
                .unwrap();
        assert!(notes.contains("• Older change"));

        // No hash segment (binary-scan detections carry none), an unknown
        // hash, and a too-short segment all render the full list.
        for raw in [
            "2024.3.6.1332",
            "2024.3.6.1332,0123456789abcdef",
            "2024.3.6.1332,1356",
        ] {
            let installed = Version::parse(raw).unwrap();
            let notes =
                super::resolve_json_commits(body, OSARA_UPDATE_URL, "/commits", Some(&installed))
                    .unwrap();
            assert!(
                notes.contains("• Older change"),
                "raw {raw:?} should not trim"
            );
        }
    }

    #[test]
    fn renders_reapack_release_body_with_bullets() {
        // A single release object renders as its name (tag fallback) on a
        // heading line, then the body: markers are normalized to the same
        // bullets the OSARA commit list uses, other lines stay verbatim.
        let url = "https://api.github.com/repos/cfillion/reapack/releases/latest";
        let body = concat!(
            r#"{"tag_name":"v1.2.6","body":"#,
            r#""- Add Crossfade Editor support\r\n- Update filter synonyms\r\nSee the manual.""#,
            "}"
        );
        let notes = super::resolve_github_release_bodies(body, url, None).unwrap();
        assert_eq!(
            notes,
            "v1.2.6\n• Add Crossfade Editor support\n• Update filter synonyms\nSee the manual."
        );

        // A bodyless release still renders its heading — the version line
        // alone is worth hearing; a release with nothing at all is an error,
        // not a panic.
        let no_body = super::resolve_github_release_bodies(r#"{"tag_name":"v1"}"#, url, None);
        assert_eq!(no_body.unwrap(), "v1");
        let blank = super::resolve_github_release_bodies(r#"{"body":"  \r\n "}"#, url, None);
        assert!(blank.is_err());
    }

    #[test]
    fn walks_release_array_back_to_the_installed_version() {
        // A `/releases` list body is newest-first; with an installed version
        // the walk keeps every release above it, skipping prereleases and
        // drafts the way `/releases/latest` does.
        let url = "https://api.github.com/repos/cfillion/reapack/releases?per_page=30";
        let body = concat!(
            r#"[{"tag_name":"v1.2.6","name":"ReaPack v1.2.6","body":"- New thing"},"#,
            r#"{"tag_name":"v1.2.6-rc1","prerelease":true,"body":"- Candidate"},"#,
            r#"{"tag_name":"v1.2.5","body":"- Fixed thing"},"#,
            r#"{"tag_name":"v1.2.4","body":"- Old thing"}]"#
        );
        let installed = Version::parse("1.2.4").unwrap();
        let notes = super::resolve_github_release_bodies(body, url, Some(&installed)).unwrap();
        assert_eq!(
            notes,
            "ReaPack v1.2.6\n• New thing\n\nv1.2.5\n• Fixed thing"
        );

        // Up to date: the newest release still renders, alone.
        let installed = Version::parse("1.2.6").unwrap();
        let notes = super::resolve_github_release_bodies(body, url, Some(&installed)).unwrap();
        assert_eq!(notes, "ReaPack v1.2.6\n• New thing");

        // No installed version (fresh install): the newest release only.
        let notes = super::resolve_github_release_bodies(body, url, None).unwrap();
        assert_eq!(notes, "ReaPack v1.2.6\n• New thing");
    }

    const REAPER_WHATSNEW_URL: &str = "https://www.reaper.fm/whatsnew.txt";

    #[test]
    fn cuts_newest_section_from_reaper_whatsnew() {
        // whatsnew.txt is newest-first; without an installed version the
        // notes are the first section only, header line included, trailing
        // blank lines trimmed.
        let body = "v7.78 - July 18\n  + Fix one\n  + Fix two\n\nv7.77 - June 2\n  + Older\n";
        let notes =
            super::resolve_changelog_sections(body, REAPER_WHATSNEW_URL, "^v[0-9]", None).unwrap();
        assert_eq!(notes, "v7.78 - July 18\n  + Fix one\n  + Fix two");
    }

    #[test]
    fn changelog_section_without_following_header_keeps_the_rest() {
        // The head-capped fetch can cut the body before the next version
        // header; everything after the first header is then the section.
        let body = "Preamble line\nv7.78 - July 18 2026\n  + Fix one thing";
        let notes =
            super::resolve_changelog_sections(body, REAPER_WHATSNEW_URL, "^v[0-9]", None).unwrap();
        assert_eq!(notes, "v7.78 - July 18 2026\n  + Fix one thing");

        // No matching header at all is an error, not a panic.
        let no_match =
            super::resolve_changelog_sections("nothing here", REAPER_WHATSNEW_URL, "^v[0-9]", None);
        assert!(no_match.is_err());
    }

    #[test]
    fn keeps_changelog_sections_back_to_the_installed_version() {
        // With an installed version, the run reaches down to (excluding) its
        // section: updating 7.76 → 7.78 reads the 7.78 and 7.77 notes.
        let body = concat!(
            "v7.78 - July 18\n  + Fix one\n\n",
            "v7.77 - June 2\n  + Fix two\n\n",
            "v7.76 - June 29\n  + Already installed\n\n",
            "v7.75 - May 1\n  + Ancient\n"
        );
        let installed = Version::parse("7.76").unwrap();
        let notes = super::resolve_changelog_sections(
            body,
            REAPER_WHATSNEW_URL,
            "^v[0-9]",
            Some(&installed),
        )
        .unwrap();
        assert_eq!(
            notes,
            "v7.78 - July 18\n  + Fix one\n\nv7.77 - June 2\n  + Fix two"
        );

        // Up to date: the newest section still renders, alone — same thing
        // an up-to-date user saw before trimming existed.
        let installed = Version::parse("7.78").unwrap();
        let notes = super::resolve_changelog_sections(
            body,
            REAPER_WHATSNEW_URL,
            "^v[0-9]",
            Some(&installed),
        )
        .unwrap();
        assert_eq!(notes, "v7.78 - July 18\n  + Fix one");

        // SWS-style `!v` headers version-match the same way: the header
        // token is read from the line's first digit onward.
        let sws = "!v2.14.0.7 featured build\nNew stuff\n\n!v2.14.0.6 featured build\nOld stuff\n";
        let installed = Version::parse("2.14.0.6").unwrap();
        let notes = super::resolve_changelog_sections(
            sws,
            "https://raw.githubusercontent.com/reaper-oss/sws/master/whatsnew.txt",
            "^!v",
            Some(&installed),
        )
        .unwrap();
        assert_eq!(notes, "!v2.14.0.7 featured build\nNew stuff");
    }

    #[test]
    fn caps_changelog_sections_for_ancient_installs() {
        // An install many versions behind must not read out the whole
        // history: the run stops after WHATS_NEW_MAX_SECTIONS sections.
        let body: String = (0..15)
            .map(|index| format!("v7.{} - some day\n  + Change\n\n", 90 - index))
            .collect();
        let installed = Version::parse("7.10").unwrap();
        let notes = super::resolve_changelog_sections(
            &body,
            REAPER_WHATSNEW_URL,
            "^v[0-9]",
            Some(&installed),
        )
        .unwrap();
        assert_eq!(notes.matches("v7.").count(), super::WHATS_NEW_MAX_SECTIONS);
        assert!(notes.contains("v7.81"));
        assert!(!notes.contains("v7.80"));
    }

    #[test]
    fn resolve_html_version_drives_reaper_and_sws_rules() {
        // REAPER: single-group "Version X.YZ" rule (format defaults to {1}).
        let reaper = super::resolve_html_version(
            "<div class='hdrbottom'>Version 7.76: June 29, 2026</div>",
            "https://www.reaper.fm/download.php",
            r"Version ([0-9][0-9.]*)",
            "{1}",
        )
        .unwrap();
        assert_eq!(reaper.raw(), "7.76");

        // SWS: two-group base + #build combined via the format template —
        // proves the generic engine reproduces the old base.#build parser.
        let sws = super::resolve_html_version(
            "Latest stable version: v2.14.0 (build #7) released 2026-01-01",
            "https://sws-extension.org/",
            r"Latest stable version:[\s\S]*?v([0-9.]+)[\s\S]*?#([0-9]+)",
            "{1}.{2}",
        )
        .unwrap();
        assert_eq!(sws.raw(), "2.14.0.7");

        // No match is an error, not a panic.
        assert!(
            super::resolve_html_version("nothing here", "u", r"Version ([0-9.]+)", "{1}").is_err()
        );
    }

    /// ReaKontrol's `github_release` block: per-platform asset prefixes,
    /// version from the asset name. Proves the engine handles a multi-prefix
    /// package (the case that motivated selector-based version extraction).
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
        }
    }

    #[test]
    fn resolve_github_version_picks_highest_reakontrol_across_platform_prefixes() {
        // Windows and macOS assets carry different prefixes; the engine must
        // version each via its matching selector and keep the global highest.
        let body = r#"{
            "tag_name": "v0",
            "assets": [
                {"name": "reaKontrol_windows_2025.6.6.7.bfbe7606.zip"},
                {"name": "reaKontrol_mac_2026.2.16.100.cafef00d.zip"},
                {"name": "reaKontrol_windows_2026.2.16.100.cafef00d.zip"},
                {"name": "reaKontrol_mac_2025.7.25.10.4ce6b01f.zip"}
            ]
        }"#;
        let spec = reakontrol_github_spec();
        let version = resolve_github_version(body, &github_release_url(&spec), &spec).unwrap();
        assert_eq!(version.raw(), "2026.2.16.100");
        assert_eq!(
            github_release_url(&spec),
            "https://api.github.com/repos/jcsteh/reaKontrol/releases/latest"
        );
    }

    #[test]
    fn version_from_asset_name_strips_prefix_suffix_and_trailing_segment() {
        let version = version_from_asset_name(
            "app2clap_2026.5.17.34.b6f558cf.zip",
            "app2clap_",
            ".zip",
            true,
        )
        .unwrap();
        assert_eq!(version.raw(), "2026.5.17.34");
        let version = version_from_asset_name(
            "app2clap_2025.9.12.2.487c00a3.zip",
            "app2clap_",
            ".zip",
            true,
        )
        .unwrap();
        assert_eq!(version.raw(), "2025.9.12.2");
        assert!(version_from_asset_name("README.md", "app2clap_", ".zip", true).is_none());
        assert!(
            version_from_asset_name(
                "reaKontrol_windows_2025.6.6.7.x.zip",
                "app2clap_",
                ".zip",
                true
            )
            .is_none()
        );
        // Without the trailing-segment drop, the whole core is the version.
        let version =
            version_from_asset_name("ffmpeg_8.1.1.zip", "ffmpeg_", ".zip", false).unwrap();
        assert_eq!(version.raw(), "8.1.1");
    }

    #[test]
    fn resolve_github_version_picks_highest_app2clap_snapshot_unordered() {
        // Real `snapshots` releases list assets out of version order, so the
        // engine must compare every match rather than trust position.
        let body = r#"{
            "tag_name": "snapshots",
            "assets": [
                {"name": "app2clap_2025.11.27.30.ca402c1b.zip"},
                {"name": "app2clap_2025.9.12.2.487c00a3.zip"},
                {"name": "app2clap_2026.5.17.34.b6f558cf.zip"},
                {"name": "app2clap_2026.5.16.31.5d1e4007.zip"}
            ]
        }"#;
        let spec = app2clap_github_spec();
        let version = resolve_github_version(body, &github_release_url(&spec), &spec).unwrap();
        assert_eq!(version.raw(), "2026.5.17.34");
    }

    #[test]
    fn app2clap_github_release_url_matches_the_former_constant() {
        // Parity: the engine must build the exact endpoint the old
        // APP2CLAP_GITHUB_LATEST_URL const used, so GitHub auth still applies.
        assert_eq!(
            github_release_url(&app2clap_github_spec()),
            "https://api.github.com/repos/jcsteh/app2clap/releases/tags/snapshots"
        );
    }

    #[test]
    fn resolve_github_version_rejects_release_with_no_matching_assets() {
        let body = r#"{"tag_name": "snapshots", "assets": [{"name": "README.md"}]}"#;
        let spec = app2clap_github_spec();
        let error = resolve_github_version(body, &github_release_url(&spec), &spec).unwrap_err();
        assert!(error.to_string().contains("no asset matched"));
    }

    #[test]
    fn resolve_github_version_reads_tag_name_with_v_prefix_stripped() {
        // The ReaPack-style version source: version comes from the tag, not
        // an asset name. Proves the engine generalizes beyond app2clap.
        let body = r#"{"tag_name": "v1.2.6", "assets": []}"#;
        let spec = GithubReleaseSpec {
            repo: "cfillion/reapack".to_string(),
            release: GithubReleaseSelector::Latest,
            version_from: VersionSource::TagName {
                strip_v_prefix: true,
            },
            assets: Vec::new(),
            artifact_kind: Some(crate::package::GithubArtifactKind::ExtensionBinary),
        };
        let version = resolve_github_version(body, &github_release_url(&spec), &spec).unwrap();
        assert_eq!(version.raw(), "1.2.6");
        assert_eq!(
            github_release_url(&spec),
            "https://api.github.com/repos/cfillion/reapack/releases/latest"
        );
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
        let version =
            parse_jaws_for_reaper_listing(body, "https://hoard.reaperaccessibility.com/listing")
                .unwrap();
        assert_eq!(version.raw(), "89");
    }

    #[test]
    fn rejects_jaws_for_reaper_listing_without_versioned_installer() {
        let body = r#"{"list": [{"n": "README.txt", "s": 1}]}"#;
        let error =
            parse_jaws_for_reaper_listing(body, "https://hoard.reaperaccessibility.com/listing")
                .unwrap_err();
        assert!(error.to_string().contains("no versioned JAWS-for-REAPER"));
    }

    #[test]
    fn resolves_plaintext_version_from_gyan_ver_payload() {
        assert_eq!(
            resolve_plaintext_version("8.1.1\n", FFMPEG_GYAN_VERSION_URL)
                .unwrap()
                .raw(),
            "8.1.1"
        );
        assert_eq!(
            resolve_plaintext_version("  8.1.1  ", FFMPEG_GYAN_VERSION_URL)
                .unwrap()
                .raw(),
            "8.1.1"
        );
        assert!(resolve_plaintext_version("", FFMPEG_GYAN_VERSION_URL).is_err());
        assert!(resolve_plaintext_version("not-a-version", FFMPEG_GYAN_VERSION_URL).is_err());
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
