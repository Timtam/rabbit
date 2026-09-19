//! Pull-request test builds published as GitHub Actions workflow artifacts.
//!
//! OSARA's CI builds every pull request and uploads the result as workflow
//! artifacts named `osara_windows_pr1454-534,240c4663` and
//! `osara_mac_pr1454-534,240c4663` - the pull request, the CI run and the
//! merge commit. GitHub lists a repository's artifacts to anyone, but only
//! lets a signed-in user download one, so the download goes through
//! nightly.link: the same link OSARA's own pull-request bot posts for testers.
//!
//! Artifacts expire after 90 days. When a pull request has no live build left,
//! resolution reports [`RabbitError::PullRequestBuildGone`] rather than a
//! generic failure, so the package can go back to its regular release.

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{RabbitError, Result};
use crate::model::Platform;
use crate::package::{GithubActionsArtifactSpec, GithubActionsArtifactTarget};

/// How many pages of 100 artifacts to read before giving up on a pull
/// request. Each CI run uploads three artifacts, so five pages reach back
/// roughly 160 runs - far past any pull request still being tested - while
/// staying well inside GitHub's 60-requests-an-hour anonymous limit.
const MAX_LISTING_PAGES: u32 = 5;
const PAGE_SIZE: u32 = 100;

/// One pull request's test build for one platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestBuild {
    pub pull_request: u32,
    /// The CI run number. Later runs of the same pull request supersede
    /// earlier ones.
    pub run: u64,
    /// `pr1454-534,240c4663` - what OSARA itself reports as its version, so
    /// the receipt and OSARA agree.
    pub version: String,
    pub artifact_id: u64,
    pub artifact_name: String,
    /// When GitHub will delete the build (RFC 3339), if it said.
    pub expires_at: Option<String>,
}

/// The target for `platform`, if the package publishes one there.
pub(crate) fn target_for(
    spec: &GithubActionsArtifactSpec,
    platform: Platform,
) -> Option<&GithubActionsArtifactTarget> {
    spec.targets
        .iter()
        .find(|target| target.platform.matches_platform(platform))
}

/// Split an artifact name into (pull request, run, version), or `None` when
/// it is not a pull-request build for this prefix - a master-branch build, the
/// debug-symbols artifact, or another platform's.
pub(crate) fn parse_build_name(name: &str, prefix: &str) -> Option<(u32, u64, String)> {
    let version = name.strip_prefix(prefix)?;
    let rest = version.strip_prefix("pr")?;
    let (pull_request, rest) = rest.split_once('-')?;
    let (run, commit) = rest.split_once(',')?;
    let pull_request: u32 = pull_request.parse().ok()?;
    let run: u64 = run.parse().ok()?;
    let is_commit = !commit.is_empty() && commit.chars().all(|ch| ch.is_ascii_hexdigit());
    (pull_request > 0 && is_commit).then(|| (pull_request, run, version.to_string()))
}

/// The live pull-request builds in one page of GitHub's artifact listing, and
/// how many artifacts the page held in total (to know when to stop paging).
pub(crate) fn builds_in_listing(
    body: &str,
    url: &str,
    prefix: &str,
) -> Result<(Vec<PullRequestBuild>, usize)> {
    let listing: Value = serde_json::from_str(body).map_err(|err| RabbitError::RemoteData {
        url: url.to_string(),
        message: format!("artifact listing is not JSON: {err}"),
    })?;
    let artifacts = listing
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| RabbitError::RemoteData {
            url: url.to_string(),
            message: "artifact listing has no artifacts array".to_string(),
        })?;
    let builds = artifacts
        .iter()
        .filter(|artifact| {
            !artifact
                .get("expired")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|artifact| {
            let name = artifact.get("name")?.as_str()?;
            let artifact_id = artifact.get("id")?.as_u64()?;
            let (pull_request, run, version) = parse_build_name(name, prefix)?;
            Some(PullRequestBuild {
                pull_request,
                run,
                version,
                artifact_id,
                artifact_name: name.to_string(),
                expires_at: artifact
                    .get("expires_at")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect();
    Ok((builds, artifacts.len()))
}

fn listing_url(repo: &str, page: u32) -> String {
    format!(
        "https://api.github.com/repos/{repo}/actions/artifacts?per_page={PAGE_SIZE}&page={page}"
    )
}

/// Every live pull-request build for `platform`, newest run first, reading at
/// most `pages` pages of the listing. `stop_at` ends the paging early once a
/// build of that pull request has been seen.
fn collect_builds(
    client: &Client,
    spec: &GithubActionsArtifactSpec,
    target: &GithubActionsArtifactTarget,
    pages: u32,
    stop_at: Option<u32>,
) -> Result<Vec<PullRequestBuild>> {
    let mut builds = Vec::new();
    for page in 1..=pages {
        let url = listing_url(&spec.repo, page);
        let body = crate::latest::http_get_text(client, &url)?;
        let (found, listed) = builds_in_listing(&body, &url, &target.name_prefix)?;
        let reached_target =
            stop_at.is_some_and(|number| found.iter().any(|b| b.pull_request == number));
        builds.extend(found);
        if reached_target || listed < PAGE_SIZE as usize {
            break;
        }
    }
    builds.sort_by_key(|build| std::cmp::Reverse(build.run));
    Ok(builds)
}

/// The live pull-request builds for `platform`, newest run first, one entry
/// per pull request (its latest run). This is what the wizard's expert mode
/// offers to choose from.
pub fn pull_request_builds(
    spec: &GithubActionsArtifactSpec,
    platform: Platform,
) -> Result<Vec<PullRequestBuild>> {
    let Some(target) = target_for(spec, platform) else {
        return Ok(Vec::new());
    };
    let client = crate::latest::build_http_client()?;
    // Two pages cover the pull requests anyone is likely testing right now,
    // and keep a wizard refresh cheap on the anonymous rate limit.
    let mut builds = collect_builds(&client, spec, target, 2, None)?;
    let mut seen = std::collections::BTreeSet::new();
    builds.retain(|build| seen.insert(build.pull_request));
    Ok(builds)
}

/// A pull request that currently has a test build, as the wizard offers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestChoice {
    pub number: u32,
    /// The pull request's title, when GitHub's pull request listing could
    /// be read; the choice falls back to the bare number otherwise.
    pub title: Option<String>,
}

/// The pull requests of `package_id` that have a live test build for
/// `platform`, newest first, with their titles. For the wizard's expert-mode
/// build choice. Empty when the package offers no pull-request channel.
pub fn pull_request_choices(
    package_id: &str,
    platform: Platform,
) -> Result<Vec<PullRequestChoice>> {
    let Some(spec) = crate::package::embedded_package_manifest()
        .packages
        .into_iter()
        .find(|spec| spec.id == package_id)
        .and_then(|spec| {
            spec.channels
                .into_iter()
                .find_map(|channel| channel.github_actions_artifact)
        })
    else {
        return Ok(Vec::new());
    };
    let builds = pull_request_builds(&spec, platform)?;
    // Titles are a nicety: a failure here must not hide the builds.
    let titles = crate::latest::build_http_client()
        .and_then(|client| {
            let url = format!(
                "https://api.github.com/repos/{}/pulls?state=all&sort=updated&direction=desc&per_page=100",
                spec.repo
            );
            crate::latest::http_get_text(&client, &url)
        })
        .map(|body| titles_in_listing(&body))
        .unwrap_or_default();
    Ok(builds
        .into_iter()
        .map(|build| PullRequestChoice {
            number: build.pull_request,
            title: titles.get(&build.pull_request).cloned(),
        })
        .collect())
}

/// Pull request number -> title, from GitHub's pull request listing.
pub(crate) fn titles_in_listing(body: &str) -> std::collections::BTreeMap<u32, String> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|listing| listing.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|pull| {
            let number = u32::try_from(pull.get("number")?.as_u64()?).ok()?;
            let title = pull.get("title")?.as_str()?.trim();
            (!title.is_empty()).then(|| (number, title.to_string()))
        })
        .collect()
}

/// The newest live build of the pull request the spec was put on
/// (`spec.pull_request`), with its target. `PullRequestBuildGone` when the
/// listing was read but held nothing for it.
pub(crate) fn newest_build<'a>(
    client: &Client,
    spec: &'a GithubActionsArtifactSpec,
    package_id: &str,
    platform: Platform,
) -> Result<(PullRequestBuild, &'a GithubActionsArtifactTarget)> {
    let pull_request = spec.pull_request.ok_or_else(|| RabbitError::RemoteData {
        url: listing_url(&spec.repo, 1),
        message: format!(
            "{package_id} was asked for a pull-request build without a pull request number"
        ),
    })?;
    let target = target_for(spec, platform).ok_or_else(|| RabbitError::RemoteData {
        url: listing_url(&spec.repo, 1),
        message: format!("{package_id} publishes no pull-request build for {platform:?}"),
    })?;
    collect_builds(client, spec, target, MAX_LISTING_PAGES, Some(pull_request))?
        .into_iter()
        .find(|build| build.pull_request == pull_request)
        .map(|build| (build, target))
        .ok_or_else(|| RabbitError::PullRequestBuildGone {
            package_id: package_id.to_string(),
            pull_request,
        })
}

/// Where to download `build` from.
pub(crate) fn download_url(spec: &GithubActionsArtifactSpec, build: &PullRequestBuild) -> String {
    spec.download_url
        .replace("{repo}", &spec.repo)
        .replace("{id}", &build.artifact_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS: &str = "osara_windows_";

    #[test]
    fn only_pull_request_builds_for_the_right_platform_parse() {
        assert_eq!(
            parse_build_name("osara_windows_pr1454-534,240c4663", WINDOWS),
            Some((1454, 534, "pr1454-534,240c4663".to_string()))
        );
        for other in [
            // a master-branch snapshot
            "osara_windows_2026.8.29.2333,dd330f65",
            // the other platform, and the debug symbols of every run
            "osara_mac_pr1454-534,240c4663",
            "windows_debug_symbols",
            // malformed numbers or commit
            "osara_windows_prX-534,240c4663",
            "osara_windows_pr1454-534,",
            "osara_windows_pr1454-534,nothex!",
            "osara_windows_pr0-534,240c4663",
        ] {
            assert_eq!(parse_build_name(other, WINDOWS), None, "{other:?}");
        }
    }

    #[test]
    fn expired_builds_are_not_offered() {
        let body = r#"{"total_count": 3, "artifacts": [
            {"id": 1, "name": "osara_windows_pr1454-534,240c4663", "expired": false, "expires_at": "2026-12-02T00:00:00Z"},
            {"id": 2, "name": "osara_windows_pr1395-387,aaaaaaaa", "expired": true},
            {"id": 3, "name": "windows_debug_symbols", "expired": false}
        ]}"#;
        let (builds, listed) = builds_in_listing(body, "test", WINDOWS).unwrap();
        assert_eq!(listed, 3);
        assert_eq!(builds.len(), 1);
        assert_eq!(builds[0].pull_request, 1454);
        assert_eq!(builds[0].artifact_id, 1);
        assert_eq!(
            builds[0].expires_at.as_deref(),
            Some("2026-12-02T00:00:00Z")
        );
    }

    #[test]
    fn a_nightly_link_url_names_the_artifact() {
        let spec = GithubActionsArtifactSpec {
            repo: "jcsteh/osara".to_string(),
            download_url: "https://nightly.link/{repo}/actions/artifacts/{id}.zip".to_string(),
            targets: Vec::new(),
            pull_request: Some(1454),
        };
        let build = PullRequestBuild {
            pull_request: 1454,
            run: 534,
            version: "pr1454-534,240c4663".to_string(),
            artifact_id: 9888938295,
            artifact_name: "osara_windows_pr1454-534,240c4663".to_string(),
            expires_at: None,
        };
        assert_eq!(
            download_url(&spec, &build),
            "https://nightly.link/jcsteh/osara/actions/artifacts/9888938295.zip"
        );
    }
    #[test]
    fn pull_request_titles_are_read_best_effort() {
        let titles = titles_in_listing(
            r#"[{"number": 1454, "title": "Report the track's arm state"},
                {"number": 1448, "title": "  "},
                {"number": "oops"}]"#,
        );
        assert_eq!(
            titles.get(&1454).map(String::as_str),
            Some("Report the track's arm state")
        );
        assert!(
            !titles.contains_key(&1448),
            "a blank title falls back to the number"
        );
        assert!(titles_in_listing("not json").is_empty());
    }
}
