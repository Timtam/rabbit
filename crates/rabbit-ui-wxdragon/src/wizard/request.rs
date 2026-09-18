//! Turns the user's page selections into a [`WizardInstallRequest`].

use rabbit_core::artifact::default_cache_dir;
use rabbit_core::package::{PACKAGE_OSARA, builtin_package_specs};
use rabbit_core::plan::PlanActionKind;
use rabbit_core::{RabbitError, Result};

use super::configuration::selected_configuration_step_ids;
use super::model::{
    OsaraKeymapChoice, PackageRow, TargetRow, WizardInstallOptions, WizardInstallRequest,
    WizardModel,
};
use super::packages::package_ids_for_rows;

pub fn install_request_from_model(
    model: &WizardModel,
    selected_target_index: Option<usize>,
    selected_package_indices: &[usize],
    options: WizardInstallOptions,
) -> Result<WizardInstallRequest> {
    let target = selected_target_index
        .and_then(|index| model.target_rows.get(index))
        .ok_or_else(|| RabbitError::PreflightFailed {
            message: "No REAPER installation target was selected.".to_string(),
        })?;

    install_request_from_target(model, target, selected_package_indices, options)
}

pub fn install_request_from_target(
    model: &WizardModel,
    target: &TargetRow,
    selected_package_indices: &[usize],
    options: WizardInstallOptions,
) -> Result<WizardInstallRequest> {
    let configuration_step_ids = selected_configuration_step_ids(&model.configuration_rows);
    install_request_from_target_and_rows(
        model,
        target,
        &model.package_rows,
        selected_package_indices,
        configuration_step_ids,
        options,
    )
}

pub fn install_request_from_target_and_rows(
    model: &WizardModel,
    target: &TargetRow,
    package_rows: &[PackageRow],
    selected_package_indices: &[usize],
    configuration_step_ids: Vec<String>,
    options: WizardInstallOptions,
) -> Result<WizardInstallRequest> {
    if !target.writable {
        return Err(RabbitError::PreflightFailed {
            message: format!(
                "Target resource path is not writable: {}",
                target.path.display()
            ),
        });
    }

    let package_ids = package_ids_for_rows(package_rows, selected_package_indices);
    if package_ids.is_empty() && configuration_step_ids.is_empty() {
        return Err(RabbitError::PreflightFailed {
            message: "No package or configuration step was selected for installation or update."
                .to_string(),
        });
    }
    let osara_selected = package_ids.iter().any(|id| id == PACKAGE_OSARA);
    // A row is "force reinstall" when the user has the box checked but
    // the original detection said the package was already current — i.e.
    // the plan would normally Keep it, but the user explicitly opted in
    // to a re-run. Detected via `original_action == Keep` on a checked
    // row (the toggle helper promotes the displayed `action` to Update
    // in that case, but `original_action` keeps the plan-time decision
    // for exactly this disambiguation).
    let force_reinstall_packages = selected_package_indices
        .iter()
        .filter_map(|index| {
            let row = package_rows.get(*index)?;
            if row.original_action == PlanActionKind::Keep {
                Some(row.package_id.clone())
            } else {
                None
            }
        })
        .collect();

    // Only rows that actually offered something carry a verdict: ticked is
    // a yes, unticked is a no. A `Keep` row offered nothing, so it says
    // nothing either way.
    let remembers_opt_out: std::collections::BTreeSet<String> =
        builtin_package_specs(model.platform)
            .into_iter()
            .filter(|spec| spec.remember_opt_out)
            .map(|spec| spec.id)
            .collect();
    let mut declined_packages = Vec::new();
    let mut accepted_packages = Vec::new();
    for (index, row) in package_rows.iter().enumerate() {
        if !remembers_opt_out.contains(&row.package_id) {
            continue;
        }
        if selected_package_indices.contains(&index) {
            accepted_packages.push(row.package_id.clone());
        } else if row.original_action != PlanActionKind::Keep {
            declined_packages.push(row.package_id.clone());
        }
    }

    Ok(WizardInstallRequest {
        resource_path: target.path.clone(),
        package_ids,
        platform: model.platform,
        architecture: target.architecture,
        portable: target.portable,
        target_app_path: Some(target.planned_app_path.clone()),
        dry_run: options.dry_run,
        allow_reaper_running: options.allow_reaper_running,
        stage_unsupported: options.stage_unsupported,
        osara_keymap_choice: if osara_selected {
            options.osara_keymap_choice
        } else {
            OsaraKeymapChoice::PreserveCurrent
        },
        package_variants: options.package_variants.clone(),
        reaper_language_package: options.reaper_language_package.clone(),
        cache_dir: options.cache_dir.unwrap_or_else(default_cache_dir),
        force_reinstall_packages,
        configuration_step_ids,
        declined_packages,
        accepted_packages,
    })
}
