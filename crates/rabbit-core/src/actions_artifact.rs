//! Pull-request test builds published as GitHub Actions workflow artifacts.
//!
//! OSARA's CI builds every pull request and uploads the result as workflow
//! artifacts named `osara_windows_pr1454-534,240c4663` and
//! `osara_mac_pr1454-534,240c4663` - the pull request, the CI run and the
//! merge commit. GitHub lists a repository's artifacts to anyone, but only
//! lets a signed-in user download one, so the download goes through
//! nightly.link: the same link OSARA's own pull-request bot posts for testers.
//!
//! Only an open pull request's build is offered. Once the pull request is
//! merged or closed, its changes are in the regular snapshot or were turned
//! down. Artifacts also expire after 90 days. Either way, resolution reports
//! [`RabbitError::PullRequestBuildGone`] rather than a generic failure, so the
//! package can go back to its regular release. Anything that leaves the
//! answer in doubt stays an error, so a failure never downgrades anything.

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{RabbitError, Result};
use crate::model::Platform;
use crate::package::{GithubActionsArtifactSpec, GithubActionsArtifactTarget};

/// How many pages of 100 artifacts to read before giving up on a pull
/// request. Paging normally stops much earlier, at the first expired build,
/// since every build after it has expired too. Each CI run uploads three
/// artifacts, so eight pages reach back about 260 runs while staying well
/// inside GitHub's 60-requests-an-hour anonymous limit.
const MAX_LISTING_PAGES: u32 = 8;
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

/// One page of GitHub's artifact listing.
#[derive(Debug)]
pub(crate) struct ListingPage {
    /// The live pull-request builds on it.
    pub builds: Vec<PullRequestBuild>,
    /// How many artifacts it held in all. A short page is the last one.
    pub listed: usize,
    /// Whether it held an expired build of this kind. The listing runs newest
    /// first and builds of one kind share a retention period, so every build
    /// after an expired one has expired too.
    pub reached_expired: bool,
}

/// The live pull-request builds in one page of GitHub's artifact listing.
pub(crate) fn builds_in_listing(body: &str, url: &str, prefix: &str) -> Result<ListingPage> {
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
    let expired = |artifact: &&Value| {
        artifact
            .get("expired")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let reached_expired = artifacts.iter().filter(expired).any(|artifact| {
        artifact
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| parse_build_name(name, prefix).is_some())
    });
    let builds = artifacts
        .iter()
        .filter(|artifact| !expired(artifact))
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
    Ok(ListingPage {
        builds,
        listed: artifacts.len(),
        reached_expired,
    })
}

fn listing_url(repo: &str, page: u32) -> String {
    format!(
        "https://api.github.com/repos/{repo}/actions/artifacts?per_page={PAGE_SIZE}&page={page}"
    )
}

/// Live pull-request builds for `platform`, newest run first, reading at most
/// `pages` pages of the listing. `stop_at` ends the paging early once a build
/// of that pull request has been seen. The flag says whether every live build
/// was seen, which is what makes "no build" a safe conclusion.
fn collect_builds(
    client: &Client,
    spec: &GithubActionsArtifactSpec,
    target: &GithubActionsArtifactTarget,
    pages: u32,
    stop_at: Option<u32>,
) -> Result<(Vec<PullRequestBuild>, bool)> {
    let mut builds = Vec::new();
    let mut complete = false;
    for page in 1..=pages {
        let url = listing_url(&spec.repo, page);
        let body = crate::latest::http_get_text(client, &url)?;
        let page = builds_in_listing(&body, &url, &target.name_prefix)?;
        let reached_target =
            stop_at.is_some_and(|number| page.builds.iter().any(|b| b.pull_request == number));
        builds.extend(page.builds);
        complete = page.reached_expired || page.listed < PAGE_SIZE as usize;
        if reached_target || complete {
            break;
        }
    }
    builds.sort_by_key(|build| std::cmp::Reverse(build.run));
    Ok((builds, complete))
}

/// The live pull-request builds for `platform`, one entry per pull request
/// (its latest run), highest pull request number first. This is what the
/// wizard's expert mode offers to choose from.
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
    let (builds, _) = collect_builds(&client, spec, target, 2, None)?;
    Ok(one_per_pull_request(builds))
}

/// Each pull request's newest build, ordered by pull request number, highest
/// first. Numbers go up as pull requests are opened, so that is newest first
/// and stays put, where ordering by latest build would reshuffle the list
/// every time CI rebuilds an older pull request.
pub(crate) fn one_per_pull_request(mut builds: Vec<PullRequestBuild>) -> Vec<PullRequestBuild> {
    builds.sort_by_key(|build| std::cmp::Reverse((build.pull_request, build.run)));
    builds.dedup_by_key(|build| build.pull_request);
    builds
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
/// `platform`, highest pull request number first, with their titles. For the wizard's expert-mode
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
    // Only open pull requests are offered: a merged one's changes are in the
    // regular snapshot, and a closed one was turned down. If the list of open
    // pull requests can't be read, every build is offered without a title.
    // Resolution checks the state again, so a closed one still falls back.
    let open = crate::latest::build_http_client()
        .and_then(|client| {
            let url = format!(
                "https://api.github.com/repos/{}/pulls?state=open&per_page=100",
                spec.repo
            );
            crate::latest::http_get_text(&client, &url)
        })
        .ok()
        .and_then(|body| open_pull_requests(&body));
    Ok(builds
        .into_iter()
        .filter_map(|build| {
            let title = match &open {
                Some(open) => open.get(&build.pull_request)?.clone(),
                None => None,
            };
            Some(PullRequestChoice {
                number: build.pull_request,
                title,
            })
        })
        .collect())
}

/// Open pull request number -> its title (`None` when blank), from GitHub's
/// `pulls?state=open` listing. `None` when the body is not a listing.
pub(crate) fn open_pull_requests(
    body: &str,
) -> Option<std::collections::BTreeMap<u32, Option<String>>> {
    let listing = serde_json::from_str::<Value>(body).ok()?;
    Some(
        listing
            .as_array()?
            .iter()
            .filter_map(|pull| {
                let number = u32::try_from(pull.get("number")?.as_u64()?).ok()?;
                let title = pull
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                    .map(str::to_string);
                Some((number, title))
            })
            .collect(),
    )
}

/// Whether pull request `number` is still open. A merged, closed or missing
/// one is `Ok(false)`, which makes its build count as gone. A network failure
/// or GitHub's rate limit stays an error.
fn pull_request_is_open(client: &Client, repo: &str, number: u32) -> Result<bool> {
    let url = format!("https://api.github.com/repos/{repo}/pulls/{number}");
    let response = crate::http::maybe_apply_github_auth(client.get(&url), &url)
        .send()
        .map_err(|source| RabbitError::Http {
            url: url.clone(),
            source,
        })?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(false);
    }
    let body = response
        .error_for_status()
        .and_then(|response| response.text())
        .map_err(|source| RabbitError::Http {
            url: url.clone(),
            source,
        })?;
    pull_request_state_is_open(&body).ok_or_else(|| RabbitError::RemoteData {
        url,
        message: "pull request has no state".to_string(),
    })
}

fn pull_request_state_is_open(body: &str) -> Option<bool> {
    let pull = serde_json::from_str::<Value>(body).ok()?;
    Some(pull.get("state")?.as_str()? == "open")
}

/// The newest live build of the pull request the spec was put on
/// (`spec.pull_request`), with its target. `PullRequestBuildGone` when the
/// pull request is no longer open, or every live build was seen and none was
/// its. Running out of pages first is an error, not a missing build.
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
    let gone = || RabbitError::PullRequestBuildGone {
        package_id: package_id.to_string(),
        pull_request,
    };
    if !pull_request_is_open(client, &spec.repo, pull_request)? {
        return Err(gone());
    }
    let (builds, complete) =
        collect_builds(client, spec, target, MAX_LISTING_PAGES, Some(pull_request))?;
    match builds
        .into_iter()
        .find(|build| build.pull_request == pull_request)
    {
        Some(build) => Ok((build, target)),
        None if complete => Err(gone()),
        None => Err(RabbitError::RemoteData {
            url: listing_url(&spec.repo, MAX_LISTING_PAGES),
            message: format!(
                "no build of pull request {pull_request} among the newest {} artifacts, and older ones may still hold one",
                MAX_LISTING_PAGES * PAGE_SIZE
            ),
        }),
    }
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
        let page = builds_in_listing(body, "test", WINDOWS).unwrap();
        assert_eq!(page.listed, 3);
        assert!(page.reached_expired);
        assert_eq!(page.builds.len(), 1);
        assert_eq!(page.builds[0].pull_request, 1454);
        assert_eq!(page.builds[0].artifact_id, 1);
        assert_eq!(
            page.builds[0].expires_at.as_deref(),
            Some("2026-12-02T00:00:00Z")
        );
    }

    #[test]
    fn only_an_expired_build_of_the_same_kind_ends_the_live_ones() {
        // An expired artifact of another kind (the debug symbols, or the
        // other platform) says nothing about how far back live builds go.
        let body = r#"{"artifacts": [
            {"id": 1, "name": "osara_windows_pr1454-534,240c4663", "expired": false},
            {"id": 2, "name": "windows_debug_symbols", "expired": true},
            {"id": 3, "name": "osara_mac_pr1395-387,aaaaaaaa", "expired": true}
        ]}"#;
        let page = builds_in_listing(body, "test", WINDOWS).unwrap();
        assert!(!page.reached_expired);
    }

    #[test]
    fn the_wizard_lists_each_pull_request_once_highest_number_first() {
        let build = |pull_request: u32, run: u64| PullRequestBuild {
            pull_request,
            run,
            version: format!("pr{pull_request}-{run},240c4663"),
            artifact_id: run,
            artifact_name: String::new(),
            expires_at: None,
        };
        // As collect_builds returns them: newest run first. The old pull
        // request 1404 was rebuilt last, and 1454 has two runs.
        let builds = one_per_pull_request(vec![
            build(1404, 540),
            build(1454, 534),
            build(1448, 520),
            build(1454, 512),
        ]);
        let order: Vec<(u32, u64)> = builds
            .iter()
            .map(|build| (build.pull_request, build.run))
            .collect();
        assert_eq!(order, vec![(1454, 534), (1448, 520), (1404, 540)]);
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
    fn open_pull_requests_are_read_with_their_titles() {
        let open = open_pull_requests(
            r#"[{"number": 1454, "title": "Report the track's arm state"},
                {"number": 1448, "title": "  "},
                {"number": "oops"}]"#,
        )
        .unwrap();
        assert_eq!(
            open.get(&1454).cloned().flatten().as_deref(),
            Some("Report the track's arm state")
        );
        assert_eq!(
            open.get(&1448),
            Some(&None),
            "a blank title is still an open pull request, shown by number"
        );
        assert_eq!(open.len(), 2);
        assert!(open_pull_requests("not json").is_none());
    }

    #[test]
    fn only_an_open_pull_request_keeps_its_build() {
        assert_eq!(
            pull_request_state_is_open(r#"{"state": "open"}"#),
            Some(true)
        );
        assert_eq!(
            pull_request_state_is_open(r#"{"state": "closed", "merged": true}"#),
            Some(false)
        );
        assert_eq!(
            pull_request_state_is_open(r#"{"message": "API rate limit"}"#),
            None
        );
    }
}
