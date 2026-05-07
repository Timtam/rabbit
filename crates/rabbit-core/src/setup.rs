use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::artifact::ArtifactDescriptor;
use crate::configuration::{
    ConfigurationStatus, ConfigurationStepReport, apply_configuration_step,
    builtin_configuration_steps, skipped_step_report,
};
use crate::detection::detect_components;
use crate::model::{Architecture, Platform};
use crate::operation::{
    PackageOperationOptions, PackageOperationReport, execute_package_operation,
    execute_resolved_package_operation,
};
use crate::package::PACKAGE_REAPER;
use crate::resource::{ResourceInitOptions, ResourceInitReport, initialize_resource_path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupOptions {
    pub dry_run: bool,
    pub portable: bool,
    pub allow_reaper_running: bool,
    pub stage_unsupported: bool,
    pub replace_osara_keymap: bool,
    pub target_app_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_path: Option<PathBuf>,
    /// Forwarded to [`PackageOperationOptions::force_reinstall_packages`]:
    /// promotes plan-time `Keep` to `Update` for the listed packages so
    /// an explicit user re-tick actually reruns the install.
    #[serde(default)]
    pub force_reinstall_packages: Vec<String>,
    /// Ids of [`ConfigurationStep`] entries the user opted in to.
    /// Configuration steps run after the package install pipeline; those
    /// whose dependency package is neither installed nor part of this
    /// run get a `SkippedDependencyMissing` report instead of failing
    /// the setup.
    #[serde(default)]
    pub configuration_step_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupReport {
    pub resource_path: PathBuf,
    pub dry_run: bool,
    pub resource_init: ResourceInitReport,
    pub package_operation: PackageOperationReport,
    /// Per-configuration-step results. Empty when the user opted out of
    /// every step.
    #[serde(default)]
    pub configuration_steps: Vec<ConfigurationStepReport>,
}

pub fn setup_requires_extension_support(package_ids: &[String]) -> bool {
    package_ids
        .iter()
        .any(|package_id| package_id != PACKAGE_REAPER)
}

pub fn execute_setup_operation(
    resource_path: &Path,
    package_ids: &[String],
    platform: Platform,
    architecture: Architecture,
    cache_dir: &Path,
    options: &SetupOptions,
) -> Result<SetupReport> {
    let resource_init = initialize_resource_path(
        resource_path,
        &ResourceInitOptions {
            dry_run: options.dry_run,
            portable: options.portable,
            include_extension_support_dirs: options.portable
                || setup_requires_extension_support(package_ids),
            allow_reaper_running: options.allow_reaper_running,
            target_app_path: options.target_app_path.clone(),
        },
    )?;
    let package_operation = execute_package_operation(
        resource_path,
        package_ids,
        platform,
        architecture,
        cache_dir,
        &PackageOperationOptions {
            dry_run: options.dry_run,
            allow_reaper_running: options.allow_reaper_running,
            stage_unsupported: options.stage_unsupported,
            replace_osara_keymap: options.replace_osara_keymap,
            target_app_path: options.target_app_path.clone(),
            lock_path: options.lock_path.clone(),
            force_reinstall_packages: options.force_reinstall_packages.clone(),
        },
    )?;

    let _ = architecture;
    let installed_or_pending = installed_or_pending_packages(resource_path, platform, package_ids);
    let configuration_steps = run_configuration_steps(
        resource_path,
        &options.configuration_step_ids,
        &installed_or_pending,
        options.dry_run,
    )?;

    Ok(SetupReport {
        resource_path: resource_path.to_path_buf(),
        dry_run: options.dry_run,
        resource_init,
        package_operation,
        configuration_steps,
    })
}

pub fn execute_resolved_setup_operation(
    resource_path: &Path,
    artifacts: Vec<ArtifactDescriptor>,
    cache_dir: &Path,
    options: &SetupOptions,
) -> Result<SetupReport> {
    let resource_init = initialize_resource_path(
        resource_path,
        &ResourceInitOptions {
            dry_run: options.dry_run,
            portable: options.portable,
            include_extension_support_dirs: options.portable
                || setup_requires_extension_support_for_artifacts(&artifacts),
            allow_reaper_running: options.allow_reaper_running,
            target_app_path: options.target_app_path.clone(),
        },
    )?;
    let pending_package_ids: Vec<String> = artifacts
        .iter()
        .map(|artifact| artifact.package_id.clone())
        .collect();
    let package_operation = execute_resolved_package_operation(
        resource_path,
        artifacts,
        cache_dir,
        &PackageOperationOptions {
            dry_run: options.dry_run,
            allow_reaper_running: options.allow_reaper_running,
            stage_unsupported: options.stage_unsupported,
            replace_osara_keymap: options.replace_osara_keymap,
            target_app_path: options.target_app_path.clone(),
            lock_path: options.lock_path.clone(),
            force_reinstall_packages: options.force_reinstall_packages.clone(),
        },
    )?;

    // We don't have a platform/architecture handy on this code path
    // (callers only supply `artifacts`), so dependency-resolution falls
    // back to "the package is in this run's plan" — receipt-driven
    // detection of pre-existing installs is skipped. That's fine for
    // the resolved-artifact entry point, which is mainly used by the
    // wizard install button (the wizard knows up-front whether ReaPack
    // is queued and only enables the configuration row when it is).
    let installed_or_pending: BTreeSet<String> = pending_package_ids.into_iter().collect();
    let configuration_steps = run_configuration_steps(
        resource_path,
        &options.configuration_step_ids,
        &installed_or_pending,
        options.dry_run,
    )?;

    Ok(SetupReport {
        resource_path: resource_path.to_path_buf(),
        dry_run: options.dry_run,
        resource_init,
        package_operation,
        configuration_steps,
    })
}

/// Build the "package considered satisfied for configuration-step
/// dependency checks" set: union of "package is on disk per the
/// detection layer" and "package is queued for install in this run".
fn installed_or_pending_packages(
    resource_path: &Path,
    platform: Platform,
    package_ids: &[String],
) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = package_ids.iter().cloned().collect();
    if let Ok(detections) = detect_components(resource_path, platform) {
        for detection in detections {
            if detection.installed {
                set.insert(detection.package_id);
            }
        }
    }
    set
}

/// Run each opted-in [`ConfigurationStep`] whose dependency package
/// is satisfied. Steps the user didn't pick produce a `Skipped`
/// report; steps with missing dependencies produce a
/// `SkippedDependencyMissing` report. Apply errors propagate up so
/// the caller can surface them — configuration is best-effort but
/// failures shouldn't be silently swallowed.
fn run_configuration_steps(
    resource_path: &Path,
    selected_ids: &[String],
    installed_or_pending: &BTreeSet<String>,
    dry_run: bool,
) -> Result<Vec<ConfigurationStepReport>> {
    let selected: BTreeSet<&str> = selected_ids.iter().map(String::as_str).collect();
    let steps = builtin_configuration_steps();
    let mut reports = Vec::with_capacity(steps.len());
    for step in &steps {
        if !selected.contains(step.id.as_str()) {
            reports.push(skipped_step_report(step, ConfigurationStatus::Skipped));
            continue;
        }
        if let Some(required) = &step.requires_package_id {
            if !installed_or_pending.contains(required) {
                reports.push(skipped_step_report(
                    step,
                    ConfigurationStatus::SkippedDependencyMissing,
                ));
                continue;
            }
        }
        reports.push(apply_configuration_step(resource_path, step, dry_run)?);
    }
    Ok(reports)
}

fn setup_requires_extension_support_for_artifacts(artifacts: &[ArtifactDescriptor]) -> bool {
    artifacts
        .iter()
        .any(|artifact| artifact.package_id != PACKAGE_REAPER)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{SetupOptions, execute_resolved_setup_operation};
    use crate::artifact::{ArtifactDescriptor, ArtifactKind};
    use crate::install::InstallFileAction;
    use crate::model::{Architecture, Platform};
    use crate::package::{PACKAGE_REAPACK, PACKAGE_REAPER};
    use crate::version::Version;

    #[test]
    fn dry_run_reports_resource_and_package_actions_without_writing() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let source = dir.path().join("reaper_reapack-x64.dll");
        fs::write(&source, b"reapack").unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_setup_operation(
            &resource_path,
            vec![artifact(&source)],
            cache.path(),
            &SetupOptions {
                dry_run: true,
                portable: true,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
                configuration_step_ids: Vec::new(),
            },
        )
        .unwrap();

        assert!(report.dry_run);
        assert!(!resource_path.exists());
        let install_report = report.package_operation.install_report.unwrap();
        assert_eq!(
            install_report.actions[0].action,
            InstallFileAction::WouldInstall
        );
    }

    #[test]
    fn apply_creates_resource_layout_and_installs_extension() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let source = dir.path().join("reaper_reapack-x64.dll");
        fs::write(&source, b"reapack").unwrap();
        let resource_path = dir.path().join("PortableREAPER");

        let report = execute_resolved_setup_operation(
            &resource_path,
            vec![artifact(&source)],
            cache.path(),
            &SetupOptions {
                dry_run: false,
                portable: true,
                allow_reaper_running: true,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: None,
                lock_path: None,
                force_reinstall_packages: Vec::new(),
                configuration_step_ids: Vec::new(),
            },
        )
        .unwrap();

        assert!(!report.dry_run);
        assert!(resource_path.join("reaper.ini").is_file());
        assert!(
            resource_path
                .join("UserPlugins/reaper_reapack-x64.dll")
                .is_file()
        );
        let install_report = report.package_operation.install_report.unwrap();
        assert_eq!(
            install_report.actions[0].action,
            InstallFileAction::Installed
        );
    }

    #[test]
    fn dry_run_reaper_only_standard_setup_uses_minimal_resource_layout() {
        let dir = tempdir().unwrap();
        let cache = tempdir().unwrap();
        let resource_path = dir.path().join("AppData").join("Roaming").join("REAPER");
        let app_path = dir
            .path()
            .join("Program Files")
            .join("REAPER")
            .join("reaper.exe");

        let report = execute_resolved_setup_operation(
            &resource_path,
            vec![ArtifactDescriptor {
                package_id: PACKAGE_REAPER.to_string(),
                version: Version::parse("7.69").unwrap(),
                platform: Platform::Windows,
                architecture: Architecture::X64,
                kind: ArtifactKind::Installer,
                url: "https://example.test/reaper-install.exe".to_string(),
                file_name: "reaper-install.exe".to_string(),
            }],
            cache.path(),
            &SetupOptions {
                dry_run: true,
                portable: false,
                allow_reaper_running: false,
                stage_unsupported: false,
                replace_osara_keymap: false,
                target_app_path: Some(app_path.clone()),
                lock_path: None,
                force_reinstall_packages: Vec::new(),
                configuration_step_ids: Vec::new(),
            },
        )
        .unwrap();

        let action_paths = report
            .resource_init
            .actions
            .iter()
            .map(|action| action.path.clone())
            .collect::<Vec<_>>();

        assert!(action_paths.contains(&resource_path));
        assert!(action_paths.contains(&resource_path.join("RABBIT")));
        assert!(action_paths.contains(&resource_path.join("RABBIT").join("logs")));
        assert!(action_paths.contains(&resource_path.join("RABBIT").join("backups")));
        assert!(!action_paths.contains(&resource_path.join("UserPlugins")));
        assert!(!action_paths.contains(&resource_path.join("KeyMaps")));
        assert!(!action_paths.contains(&resource_path.join("reaper.ini")));
        assert_eq!(report.package_operation.items.len(), 1);
        assert_eq!(
            report.package_operation.items[0]
                .planned_execution
                .as_ref()
                .unwrap()
                .verification_paths,
            vec![app_path]
        );
    }

    fn artifact(source: &std::path::Path) -> ArtifactDescriptor {
        ArtifactDescriptor {
            package_id: PACKAGE_REAPACK.to_string(),
            version: Version::parse("1.2.6").unwrap(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            kind: ArtifactKind::ExtensionBinary,
            url: source.display().to_string(),
            file_name: "reaper_reapack-x64.dll".to_string(),
        }
    }
}
