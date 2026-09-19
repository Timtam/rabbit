use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::{ComponentDetection, Installation};
use crate::package::PACKAGE_REAPER;
use crate::version::Version;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailablePackage {
    pub package_id: String,
    pub version: Option<Version>,
    /// Rendered What's-New notes for the available version, when the package
    /// declares a `whats_new` source and the wizard's deferred check resolved
    /// it. Carried next to the version so the package-details pane can show
    /// them; the planner itself ignores this field.
    #[serde(default)]
    pub whats_new: Option<String>,
    /// The channel this version was checked on (`dev`, `pr:1454`), or `None`
    /// for stable. When it differs from the channel the installed copy came
    /// from, the planner offers the switch even if the version alone would
    /// say "keep" - going back from a development build to the stable one
    /// is a downgrade, and without this it would never be offered.
    #[serde(default)]
    pub channel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub target: Option<Installation>,
    pub actions: Vec<PlanAction>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanAction {
    pub package_id: String,
    pub action: PlanActionKind,
    pub installed_version: Option<Version>,
    pub available_version: Option<Version>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanActionKind {
    Install,
    Update,
    Keep,
}

pub fn build_install_plan(
    target: Option<Installation>,
    detections: &[ComponentDetection],
    desired_package_ids: &[String],
    available_packages: &[AvailablePackage],
) -> InstallPlan {
    let detections_by_id: BTreeMap<_, _> = detections
        .iter()
        .map(|detection| (detection.package_id.as_str(), detection))
        .collect();
    let available_by_id: BTreeMap<_, _> = available_packages
        .iter()
        .map(|available| (available.package_id.as_str(), available))
        .collect();
    // What channel each installed package came from, as the last RABBIT
    // install recorded it. Read raw rather than filtered to the channels the
    // manifest still offers: a package sitting on a channel that no longer
    // exists has to be offered the way back to stable, not left stranded.
    let installed_channels = target
        .as_ref()
        .and_then(|target| {
            crate::receipt::load_install_state(&target.resource_path)
                .ok()
                .flatten()
        })
        .map(|state| {
            state
                .packages
                .into_iter()
                .filter_map(|(id, receipt)| receipt.channel.map(|channel| (id, channel)))
                .collect::<BTreeMap<String, String>>()
        })
        .unwrap_or_default();

    let mut actions = Vec::new();
    for package_id in desired_package_ids {
        let available = available_by_id.get(package_id.as_str()).copied();
        let detection = detections_by_id.get(package_id.as_str()).copied();
        let (installed, installed_version) = if package_id == PACKAGE_REAPER {
            target_reaper_state(target.as_ref())
        } else {
            (
                detection.is_some_and(|detection| detection.installed),
                detection.and_then(|detection| detection.version.clone()),
            )
        };
        let available_version = available.and_then(|available| available.version.clone());
        let available_channel = available.and_then(|available| available.channel.clone());
        let installed_channel = installed_channels.get(package_id.as_str()).cloned();
        // A switch between channels is offered as an update whatever the
        // versions say. The case that needs it is the way back: a development
        // build of REAPER is newer than the stable release it is based on, so
        // an ordered comparison would call "go back to stable" a Keep and the
        // switch would never happen.
        let switching_channel =
            installed && available_version.is_some() && installed_channel != available_channel;

        let (action, reason) = if !installed {
            (
                PlanActionKind::Install,
                "Package is not installed in the selected REAPER resource path.".to_string(),
            )
        } else if switching_channel {
            (
                PlanActionKind::Update,
                channel_switch_reason(installed_channel.as_deref(), available_channel.as_deref()),
            )
        } else if let (Some(installed), Some(available)) = (&installed_version, &available_version)
        {
            let comparison =
                crate::package::version_comparison_on(package_id, available_channel.as_deref());
            if crate::package::version_needs_update(installed, available, comparison) {
                (
                    PlanActionKind::Update,
                    match comparison {
                        // A content-hash package has no ordering to speak of:
                        // the digest simply changed upstream.
                        crate::package::VersionComparison::Exact => {
                            "Available version differs from the installed one.".to_string()
                        }
                        crate::package::VersionComparison::Ordered => {
                            "Installed version is older than the available version.".to_string()
                        }
                    },
                )
            } else {
                (
                    PlanActionKind::Keep,
                    "Installed version is current or newer than the available version.".to_string(),
                )
            }
        } else if installed_version.is_none() && available_version.is_some() {
            // The package is on disk but its installed version couldn't be
            // read. Rather than asking a non-technical user to "review
            // manually", treat it as Update: re-install the latest known
            // upstream version on top, with the standard backup/receipt
            // safety net protecting the prior files.
            (
                PlanActionKind::Update,
                "Package is installed but its version could not be detected; updating to the latest available version."
                    .to_string(),
            )
        } else {
            (
                PlanActionKind::Keep,
                "Package is installed; no available version metadata was provided.".to_string(),
            )
        };

        actions.push(PlanAction {
            package_id: package_id.clone(),
            action,
            installed_version,
            available_version,
            reason,
        });
    }

    let mut notes = Vec::new();
    if target.is_none() {
        notes.push("No REAPER installation target was selected.".to_string());
    }
    if available_packages.is_empty() {
        notes.push("Latest-version providers are not implemented yet; the plan only identifies missing packages and packages with known supplied versions.".to_string());
    }

    InstallPlan {
        target,
        actions,
        notes,
    }
}

/// The plan reason for moving a package between channels. `None` is stable.
fn channel_switch_reason(from: Option<&str>, to: Option<&str>) -> String {
    match (from, to) {
        (Some(from), None) => {
            format!("Installed from the {from} channel; switching back to the stable release.")
        }
        (None, Some(to)) => format!("Switching from the stable release to the {to} channel."),
        (Some(from), Some(to)) => format!("Switching from the {from} channel to the {to} channel."),
        (None, None) => "Switching channel.".to_string(),
    }
}

fn target_reaper_state(target: Option<&Installation>) -> (bool, Option<Version>) {
    let Some(target) = target else {
        return (false, None);
    };

    let installed = target_reaper_app_exists(&target.app_path);
    let installed_version = installed.then(|| target.version.clone()).flatten();
    (installed, installed_version)
}

fn target_reaper_app_exists(app_path: &Path) -> bool {
    app_path.is_file()
        || app_path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
            && app_path.exists()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::model::{ComponentDetection, Confidence, Installation, InstallationKind, Platform};
    use crate::package::{PACKAGE_OSARA, PACKAGE_REAPACK, PACKAGE_REAPER};
    use crate::plan::{AvailablePackage, PlanActionKind, build_install_plan};
    use crate::version::Version;

    #[test]
    fn plans_install_for_missing_package() {
        let desired = vec![PACKAGE_OSARA.to_string()];
        let plan = build_install_plan(None, &[], &desired, &[]);

        assert_eq!(plan.actions[0].action, PlanActionKind::Install);
    }

    #[test]
    fn plans_update_when_available_version_is_newer() {
        let detections = vec![ComponentDetection {
            package_id: PACKAGE_OSARA.to_string(),
            display_name: "OSARA".to_string(),
            installed: true,
            version: Some(Version::parse("2024.1").unwrap()),
            detector: "test".to_string(),
            confidence: Confidence::High,
            files: Vec::new(),
            notes: Vec::new(),
        }];
        let available = vec![AvailablePackage {
            package_id: PACKAGE_OSARA.to_string(),
            version: Some(Version::parse("2024.2").unwrap()),
            whats_new: None,
            channel: None,
        }];
        let desired = vec![PACKAGE_OSARA.to_string()];

        let plan = build_install_plan(None, &detections, &desired, &available);

        assert_eq!(plan.actions[0].action, PlanActionKind::Update);
    }

    #[test]
    fn plans_update_when_installed_version_is_unknown_but_available_is_known() {
        // When the package is on disk but its version is unreadable, the
        // wizard should NOT push a "Review manually" decision onto the user.
        // RABBIT plans an Update instead (re-install on top, with backup +
        // receipt protecting the prior files).
        let detections = vec![ComponentDetection {
            package_id: PACKAGE_REAPACK.to_string(),
            display_name: "ReaPack".to_string(),
            installed: true,
            version: None,
            detector: "test".to_string(),
            confidence: Confidence::Medium,
            files: Vec::new(),
            notes: Vec::new(),
        }];
        let available = vec![AvailablePackage {
            package_id: PACKAGE_REAPACK.to_string(),
            version: Some(Version::parse("1.2.6").unwrap()),
            whats_new: None,
            channel: None,
        }];
        let desired = vec![PACKAGE_REAPACK.to_string()];

        let plan = build_install_plan(None, &detections, &desired, &available);

        assert_eq!(plan.actions[0].action, PlanActionKind::Update);
        assert!(
            plan.actions[0].reason.contains("could not be detected"),
            "expected the reason text to explain the version-detection fallback, got {:?}",
            plan.actions[0].reason
        );
    }

    #[test]
    fn plans_install_for_reaper_when_target_app_is_missing() {
        let dir = tempdir().unwrap();
        let installation = fake_reaper_installation(
            dir.path().join("reaper.exe"),
            dir.path().to_path_buf(),
            None,
        );
        let desired = vec![PACKAGE_REAPER.to_string()];
        let available = vec![AvailablePackage {
            package_id: PACKAGE_REAPER.to_string(),
            version: Some(Version::parse("7.70").unwrap()),
            whats_new: None,
            channel: None,
        }];

        let plan = build_install_plan(Some(installation), &[], &desired, &available);

        assert_eq!(plan.actions[0].action, PlanActionKind::Install);
        assert_eq!(plan.actions[0].installed_version, None);
        assert_eq!(
            plan.actions[0].available_version,
            Some(Version::parse("7.70").unwrap())
        );
    }

    #[test]
    fn plans_keep_for_reaper_when_target_app_exists() {
        let dir = tempdir().unwrap();
        let app_path = dir.path().join("reaper.exe");
        fs::write(&app_path, b"").unwrap();
        let installation = fake_reaper_installation(
            app_path,
            dir.path().to_path_buf(),
            Some(Version::parse("7.69").unwrap()),
        );
        let desired = vec![PACKAGE_REAPER.to_string()];

        let plan = build_install_plan(Some(installation), &[], &desired, &[]);

        assert_eq!(plan.actions[0].action, PlanActionKind::Keep);
        assert_eq!(
            plan.actions[0].installed_version,
            Some(Version::parse("7.69").unwrap())
        );
        assert!(plan.actions[0].available_version.is_none());
    }

    #[test]
    fn plans_update_for_reaper_when_available_version_is_newer() {
        let dir = tempdir().unwrap();
        let app_path = dir.path().join("reaper.exe");
        fs::write(&app_path, b"").unwrap();
        let installation = fake_reaper_installation(
            app_path,
            dir.path().to_path_buf(),
            Some(Version::parse("7.68").unwrap()),
        );
        let desired = vec![PACKAGE_REAPER.to_string()];
        let available = vec![AvailablePackage {
            package_id: PACKAGE_REAPER.to_string(),
            version: Some(Version::parse("7.70").unwrap()),
            whats_new: None,
            channel: None,
        }];

        let plan = build_install_plan(Some(installation), &[], &desired, &available);

        assert_eq!(plan.actions[0].action, PlanActionKind::Update);
        assert_eq!(
            plan.actions[0].installed_version,
            Some(Version::parse("7.68").unwrap())
        );
        assert_eq!(
            plan.actions[0].available_version,
            Some(Version::parse("7.70").unwrap())
        );
    }

    /// A portable REAPER at `dir` reporting `installed`, whose receipt says it
    /// came from `channel` (`None` = stable), planned against `available`
    /// checked on `available_channel`.
    fn plan_reaper_between_channels(
        installed: &str,
        channel: Option<&str>,
        available: &str,
        available_channel: Option<&str>,
    ) -> super::PlanAction {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("reaper.exe");
        fs::write(&app, b"stub").unwrap();
        let mut packages = std::collections::BTreeMap::new();
        packages.insert(
            PACKAGE_REAPER.to_string(),
            crate::receipt::PackageReceipt {
                id: PACKAGE_REAPER.to_string(),
                version: Some(Version::parse(installed).unwrap()),
                variant: None,
                channel: channel.map(str::to_string),
                source_url: None,
                source_sha256: None,
                installed_files: Vec::new(),
                installed_at: None,
                rabbit_version: None,
                architecture: None,
            },
        );
        crate::receipt::save_install_state(
            dir.path(),
            &crate::receipt::InstallState {
                schema_version: 1,
                packages,
                declined_packages: Default::default(),
            },
        )
        .unwrap();
        let plan = build_install_plan(
            Some(fake_reaper_installation(
                app,
                dir.path().to_path_buf(),
                Some(Version::parse(installed).unwrap()),
            )),
            &[],
            &[PACKAGE_REAPER.to_string()],
            &[AvailablePackage {
                package_id: PACKAGE_REAPER.to_string(),
                version: Some(Version::parse(available).unwrap()),
                whats_new: None,
                channel: available_channel.map(str::to_string),
            }],
        );
        plan.actions.into_iter().next().unwrap()
    }

    #[test]
    fn going_back_from_a_dev_build_to_stable_is_offered_even_though_it_is_older() {
        // The development build is newer than the stable release it is based
        // on, so an ordered comparison alone says "keep". Without the channel
        // switch, a user leaving expert mode would never be taken back.
        let action = plan_reaper_between_channels("7.80+dev0917", Some("dev"), "7.80", None);
        assert_eq!(action.action, PlanActionKind::Update);
        assert!(
            action.reason.contains("back to the stable"),
            "{}",
            action.reason
        );
    }

    #[test]
    fn switching_from_stable_to_the_dev_channel_is_offered() {
        let action = plan_reaper_between_channels("7.80", None, "7.80+dev0917", Some("dev"));
        assert_eq!(action.action, PlanActionKind::Update);
    }

    #[test]
    fn on_the_dev_channel_any_different_build_is_an_update_and_the_same_one_is_kept() {
        // Exact comparison: December's +dev1230 must still be replaced by
        // January's +dev0102, which ordering by the numbers alone gets wrong.
        let wrapped =
            plan_reaper_between_channels("7.85+dev1230", Some("dev"), "7.85+dev0102", Some("dev"));
        assert_eq!(wrapped.action, PlanActionKind::Update);
        let same =
            plan_reaper_between_channels("7.80+dev0917", Some("dev"), "7.80+dev0917", Some("dev"));
        assert_eq!(same.action, PlanActionKind::Keep);
    }

    #[test]
    fn stable_to_stable_still_uses_the_ordered_comparison() {
        let newer = plan_reaper_between_channels("7.79", None, "7.80", None);
        assert_eq!(newer.action, PlanActionKind::Update);
        let current = plan_reaper_between_channels("7.80", None, "7.80", None);
        assert_eq!(current.action, PlanActionKind::Keep);
    }

    fn fake_reaper_installation(
        app_path: std::path::PathBuf,
        resource_path: std::path::PathBuf,
        version: Option<Version>,
    ) -> Installation {
        Installation {
            kind: InstallationKind::Portable,
            platform: Platform::Windows,
            app_path,
            resource_path,
            version,
            architecture: None,
            writable: true,
            confidence: Confidence::High,
            evidence: Vec::new(),
        }
    }
}
