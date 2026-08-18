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
    PackageOperationOptions, PackageOperationReport, execute_package_operation_with_progress,
    execute_resolved_package_operation_with_progress,
};
use crate::package::PACKAGE_REAPER;
use crate::progress::{ProgressEvent, ProgressReporter};
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
    /// Forwarded to [`PackageOperationOptions::package_variants`]: the
    /// chosen flavour per package (the Spanish language pack's es_ES vs
    /// es_MX OSARA translation today).
    #[serde(default)]
    pub package_variants: std::collections::BTreeMap<String, String>,
    /// Which language pack the "set REAPER's language" step should activate.
    /// Several packs can be installed side by side — REAPER keeps them all in
    /// `LangPack/` — but only one is active, so this is the user's single
    /// choice of which. `None` activates the sole installed pack when there
    /// is exactly one, and leaves REAPER's setting alone when ambiguous.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaper_language_package: Option<String>,
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
    execute_setup_operation_with_progress(
        resource_path,
        package_ids,
        platform,
        architecture,
        cache_dir,
        options,
        &ProgressReporter::noop(),
    )
}

/// Like [`execute_setup_operation`] but threads a [`ProgressReporter`]
/// through to the download, install, and configuration phases. Wired
/// up by the wxdragon wizard to drive a live progress bar; the no-op
/// overload above is what the CLI and tests use.
pub fn execute_setup_operation_with_progress(
    resource_path: &Path,
    package_ids: &[String],
    platform: Platform,
    architecture: Architecture,
    cache_dir: &Path,
    options: &SetupOptions,
    progress: &ProgressReporter,
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
    let package_operation = execute_package_operation_with_progress(
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
            package_variants: options.package_variants.clone(),
        },
        progress,
    )?;

    let _ = architecture;
    let mut installed_or_pending =
        installed_or_pending_packages(resource_path, platform, package_ids);
    retain_packages_that_landed(&mut installed_or_pending, &package_operation);
    let configuration_steps = run_configuration_steps(
        resource_path,
        &options.configuration_step_ids,
        &installed_or_pending,
        &crate::configuration::ConfigurationContext {
            reaper_language_package: options.reaper_language_package.as_deref(),
        },
        options.dry_run,
        progress,
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
    execute_resolved_setup_operation_with_progress(
        resource_path,
        artifacts,
        cache_dir,
        options,
        &ProgressReporter::noop(),
    )
}

/// Progress-aware variant of [`execute_resolved_setup_operation`].
pub fn execute_resolved_setup_operation_with_progress(
    resource_path: &Path,
    artifacts: Vec<ArtifactDescriptor>,
    cache_dir: &Path,
    options: &SetupOptions,
    progress: &ProgressReporter,
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
    let package_operation = execute_resolved_package_operation_with_progress(
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
            package_variants: options.package_variants.clone(),
        },
        progress,
    )?;

    // We don't have a platform/architecture handy on this code path
    // (callers only supply `artifacts`), so dependency-resolution falls
    // back to "the package is in this run's plan" — receipt-driven
    // detection of pre-existing installs is skipped. That's fine for
    // the resolved-artifact entry point, which is mainly used by the
    // wizard install button (the wizard knows up-front whether ReaPack
    // is queued and only enables the configuration row when it is).
    let mut installed_or_pending: BTreeSet<String> = pending_package_ids.into_iter().collect();
    retain_packages_that_landed(&mut installed_or_pending, &package_operation);
    let configuration_steps = run_configuration_steps(
        resource_path,
        &options.configuration_step_ids,
        &installed_or_pending,
        &crate::configuration::ConfigurationContext {
            reaper_language_package: options.reaper_language_package.as_deref(),
        },
        options.dry_run,
        progress,
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
/// Drop from the dependency set any package this run did NOT actually put on
/// disk. The set is otherwise built from the *plan* — what the user asked
/// for — which is wrong the moment a package fails or is skipped because its
/// own dependency failed: the configuration step would then run against
/// something that isn't there.
///
/// This is what made a failed REAPER install surface as
/// "cannot set REAPER's language: no installed language-pack file is recorded
/// for langpack-de" — the language pack was correctly skipped, but its
/// activation step ran anyway and buried the real cause.
fn retain_packages_that_landed(
    installed_or_pending: &mut BTreeSet<String>,
    report: &crate::operation::PackageOperationReport,
) {
    use crate::operation::PackageOperationStatus;

    for item in &report.items {
        if matches!(
            item.status,
            PackageOperationStatus::Failed | PackageOperationStatus::SkippedDependencyFailed
        ) {
            installed_or_pending.remove(&item.package_id);
        }
    }
}

fn run_configuration_steps(
    resource_path: &Path,
    selected_ids: &[String],
    installed_or_pending: &BTreeSet<String>,
    context: &crate::configuration::ConfigurationContext<'_>,
    dry_run: bool,
    progress: &ProgressReporter,
) -> Result<Vec<ConfigurationStepReport>> {
    let selected: BTreeSet<&str> = selected_ids.iter().map(String::as_str).collect();
    let steps = builtin_configuration_steps();
    let mut reports = Vec::with_capacity(steps.len());
    for step in &steps {
        if !selected.contains(step.id.as_str()) {
            reports.push(skipped_step_report(step, ConfigurationStatus::Skipped));
            continue;
        }
        // ANY-of: one satisfied dependency is enough. An empty list means
        // the step has no package dependency at all.
        if !step.requires_packages.is_empty()
            && !step
                .requires_packages
                .iter()
                .any(|required| installed_or_pending.contains(required))
        {
            reports.push(skipped_step_report(
                step,
                ConfigurationStatus::SkippedDependencyMissing,
            ));
            continue;
        }
        progress.report(ProgressEvent::ConfigurationStarted {
            step_id: step.id.clone(),
        });
        reports.push(apply_configuration_step(
            resource_path,
            step,
            context,
            dry_run,
        )?);
        progress.report(ProgressEvent::ConfigurationCompleted {
            step_id: step.id.clone(),
        });
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
    /// A configuration step must not run when the package it depends on did
    /// not actually land — the dependency set is otherwise built from the
    /// PLAN, so a failed package still looked satisfied. That is what made a
    /// failed REAPER install surface to a user as "cannot set REAPER's
    /// language: no installed language-pack file is recorded for langpack-de",
    /// hiding the real cause.
    #[test]
    fn config_step_dependencies_follow_what_actually_installed() {
        use super::retain_packages_that_landed;
        use crate::artifact::{ArtifactDescriptor, ArtifactKind};
        use crate::model::{Architecture, Platform};
        use crate::operation::{
            PackageOperationItem, PackageOperationMessage, PackageOperationReport,
            PackageOperationStatus,
        };
        use crate::plan::PlanActionKind;
        use crate::version::Version;
        use std::collections::BTreeSet;

        let item = |id: &str, status| PackageOperationItem {
            package_id: id.to_string(),
            plan_action: PlanActionKind::Install,
            status,
            artifact: ArtifactDescriptor {
                package_id: id.to_string(),
                version: Version::parse("1.0.0").unwrap(),
                platform: Platform::Windows,
                architecture: Architecture::X64,
                kind: ArtifactKind::ExtensionBinary,
                url: "https://example.test/x".to_string(),
                file_name: "x".to_string(),
            },
            cached_artifact: None,
            install_action: None,
            backup_paths: Vec::new(),
            backup_manifest_path: None,
            planned_execution: None,
            manual_instruction: None,
            message: String::new(),
            message_code: PackageOperationMessage::UnattendedInstalled,
        };

        let report = PackageOperationReport {
            resource_path: std::path::PathBuf::from("x"),
            dry_run: false,
            install_report: None,
            receipt_backup_path: None,
            receipt_backup_manifest_path: None,
            items: vec![
                item("reaper", PackageOperationStatus::Failed),
                item(
                    "langpack-de",
                    PackageOperationStatus::SkippedDependencyFailed,
                ),
                item("osara", PackageOperationStatus::InstalledOrChecked),
                item("sws", PackageOperationStatus::SkippedCurrent),
            ],
        };

        let mut set: BTreeSet<String> = ["reaper", "langpack-de", "osara", "sws"]
            .into_iter()
            .map(String::from)
            .collect();
        retain_packages_that_landed(&mut set, &report);

        // Gone: the failure and the package skipped because of it.
        assert!(!set.contains("reaper"));
        assert!(!set.contains("langpack-de"));
        // Kept: installed, and already-current (which IS on disk).
        assert!(set.contains("osara"));
        assert!(set.contains("sws"));
    }

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
                package_variants: Default::default(),
                reaper_language_package: None,
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
                package_variants: Default::default(),
                reaper_language_package: None,
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
                package_variants: Default::default(),
                reaper_language_package: None,
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
