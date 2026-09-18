//! Target page rows: the detected installations plus the custom portable
//! folder the user can point at.

use std::fs;
use std::path::{Path, PathBuf};

use rabbit_core::arch_probe::probe_executable_architecture;
use rabbit_core::localization::Localizer;
use rabbit_core::metadata::file_version;
use rabbit_core::model::{Architecture, Confidence, Installation, InstallationKind, Platform};

use super::bootstrap::localizer_from_options;
use super::labels::{unknown_version_text, yes_no};
use super::model::{TargetRow, WizardModel};

pub(crate) fn target_rows(
    localizer: &Localizer,
    installations: &[Installation],
    selected_target_index: Option<usize>,
) -> Vec<TargetRow> {
    installations
        .iter()
        .enumerate()
        .map(|(index, installation)| {
            target_row(
                localizer,
                installation,
                Some(index) == selected_target_index,
            )
        })
        .collect()
}

fn target_row(localizer: &Localizer, installation: &Installation, selected: bool) -> TargetRow {
    let version = installation
        .version
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| localizer.text("detect-version-unknown").value);
    // Dropdown label shows the *install directory* (where reaper.exe
    // lives), not the resource folder. For a standard install that's
    // typically `C:\Program Files\REAPER (x64)`; for portable it's the
    // portable folder itself. The resource folder still appears in the
    // expanded "Target details" pane via `wizard-target-details`.
    let install_dir = installation
        .app_path
        .parent()
        .map(|parent| parent.display().to_string())
        .unwrap_or_else(|| installation.app_path.display().to_string());
    TargetRow {
        label: localizer
            .format(
                "wizard-target-row",
                &[
                    ("version", version.as_str()),
                    ("path", install_dir.as_str()),
                ],
            )
            .value,
        details: localizer
            .format(
                "wizard-target-details",
                &[
                    ("app_path", &installation.app_path.display().to_string()),
                    ("version", version.as_str()),
                    ("path", &installation.resource_path.display().to_string()),
                    (
                        "writable",
                        yes_no(localizer, installation.writable).as_str(),
                    ),
                ],
            )
            .value,
        app_path: installation
            .app_path
            .exists()
            .then(|| installation.app_path.clone()),
        planned_app_path: installation.app_path.clone(),
        path: installation.resource_path.clone(),
        version: installation.version.clone(),
        portable: installation.kind == InstallationKind::Portable,
        selected,
        writable: installation.writable,
        architecture: installation
            .architecture
            .unwrap_or_else(Architecture::current),
    }
}

pub fn custom_portable_target_row(model: &WizardModel, path: PathBuf, selected: bool) -> TargetRow {
    let writable = is_probably_writable(&path);
    let writable_text = if writable {
        model.text.common_yes.clone()
    } else {
        model.text.common_no.clone()
    };
    let app_path = portable_reaper_app_path(model.platform, &path);
    let version = app_path
        .as_ref()
        .and_then(|path| file_version(path).ok().flatten());
    let version_text = version
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| unknown_version_text(model));
    TargetRow {
        label: format!(
            "{}: {}",
            model.text.target_custom_portable_label,
            path.display()
        ),
        details: format!(
            "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}",
            model.text.target_custom_portable_app_path_label,
            app_path
                .as_ref()
                .unwrap_or(&default_portable_reaper_app_path(model.platform, &path))
                .display(),
            model.text.target_custom_portable_path_label,
            path.display(),
            model.text.target_custom_portable_version_label,
            version_text,
            model.text.target_custom_portable_writable_label,
            writable_text,
            model.text.target_custom_portable_note
        ),
        app_path: app_path.clone(),
        planned_app_path: app_path
            .clone()
            .unwrap_or_else(|| default_portable_reaper_app_path(model.platform, &path)),
        path,
        version,
        portable: true,
        selected,
        writable,
        // Probe the portable target's REAPER binary if it exists on disk;
        // otherwise inherit the host arch (the user is staging a fresh
        // portable, so the upcoming install will land a host-arch REAPER).
        architecture: app_path
            .as_deref()
            .map(probe_executable_architecture)
            .unwrap_or_else(Architecture::current),
    }
}

pub fn refreshed_target_row(model: &WizardModel, target: &TargetRow) -> TargetRow {
    if target.portable {
        return custom_portable_target_row(model, target.path.clone(), target.selected);
    }

    // Re-probe the target binary's architecture instead of inheriting the
    // host arch — `target.architecture` may be stale if the user swapped
    // REAPER builds (Intel ↔ Apple Silicon, x64 ↔ ARM) under the same
    // install path between wizard launches.
    let probed_architecture = probe_executable_architecture(&target.planned_app_path);
    let installation = Installation {
        kind: InstallationKind::Standard,
        platform: model.platform,
        app_path: target.planned_app_path.clone(),
        resource_path: target.path.clone(),
        version: file_version(&target.planned_app_path).ok().flatten(),
        architecture: Some(probed_architecture),
        writable: is_probably_writable(&target.path),
        confidence: Confidence::Medium,
        evidence: Vec::new(),
    };

    match localizer_from_options(&model.bootstrap_options) {
        Ok(localizer) => target_row(&localizer, &installation, target.selected),
        Err(_) => TargetRow {
            label: target.label.clone(),
            details: target.details.clone(),
            app_path: installation
                .app_path
                .exists()
                .then(|| installation.app_path.clone()),
            planned_app_path: installation.app_path.clone(),
            path: installation.resource_path.clone(),
            version: installation.version.clone(),
            portable: false,
            selected: target.selected,
            writable: installation.writable,
            architecture: probed_architecture,
        },
    }
}

pub(crate) fn installation_from_target_row(
    model: &WizardModel,
    target: &TargetRow,
) -> Installation {
    Installation {
        kind: if target.portable {
            InstallationKind::Portable
        } else {
            InstallationKind::Standard
        },
        platform: model.platform,
        app_path: target.planned_app_path.clone(),
        resource_path: target.path.clone(),
        version: target.version.clone(),
        architecture: Some(target.architecture),
        writable: target.writable,
        confidence: Confidence::Medium,
        evidence: Vec::new(),
    }
}

fn portable_reaper_app_path(platform: Platform, resource_path: &Path) -> Option<PathBuf> {
    match platform {
        Platform::Windows => {
            let app_path = resource_path.join("reaper.exe");
            app_path.is_file().then_some(app_path)
        }
        Platform::MacOs => fs::read_dir(resource_path)
            .ok()?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.to_ascii_lowercase().contains("reaper"))
            }),
    }
}

fn default_portable_reaper_app_path(platform: Platform, resource_path: &Path) -> PathBuf {
    match platform {
        Platform::Windows => resource_path.join("reaper.exe"),
        Platform::MacOs => resource_path.join("REAPER.app"),
    }
}

fn is_probably_writable(path: &Path) -> bool {
    let existing_path = if path.exists() {
        path
    } else {
        path.parent().unwrap_or(path)
    };

    fs::metadata(existing_path)
        .map(|metadata| !metadata.permissions().readonly())
        .unwrap_or(false)
}
