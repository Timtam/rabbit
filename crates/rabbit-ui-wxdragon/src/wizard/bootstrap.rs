//! Startup path: detect installations, build the install plan, and turn it
//! into the first [`WizardModel`] the pages render.

use rabbit_core::detection::{
    DiscoveryOptions, colocated_portable_root, default_standard_installation, detect_components,
    discover_installations,
};
use rabbit_core::latest::fetch_latest_versions;
use rabbit_core::localization::{Localizer, embedded_locales};
use rabbit_core::model::{Architecture, Installation, InstallationKind, Platform};
use rabbit_core::package::{builtin_package_specs, detect_host_capabilities};
use rabbit_core::plan::{AvailablePackage, InstallPlan, PlanActionKind, build_install_plan};
use rabbit_core::{RabbitError, Result};

use super::configuration::configuration_rows;
use super::model::{LanguageOption, UiBootstrapOptions, WizardControls, WizardModel, WizardStep};
use super::packages::{
    apply_version_check_failures_to_rows, package_rows, wizard_desired_package_ids,
};
use super::review::review_lines;
use super::target::target_rows;
use super::text::{localized_wx_mnemonic_label, wizard_steps, wizard_text};

pub fn load_wizard_model(options: UiBootstrapOptions) -> Result<WizardModel> {
    let platform = Platform::current().ok_or(RabbitError::UnsupportedPlatform)?;
    let localizer = localizer_from_options(&options)?;
    // If RABBIT was dropped next to a portable REAPER, fold that folder into
    // the portable roots so it's discovered as a target the user can update
    // without reaching for the Browse button.
    let colocated_portable = colocated_portable_root();
    let mut portable_roots = options.portable_roots.clone();
    if let Some(root) = &colocated_portable
        && !portable_roots.contains(root)
    {
        portable_roots.push(root.clone());
    }
    let discovered_installations = discover_installations(&DiscoveryOptions {
        include_standard: true,
        portable_roots,
    })?;
    let installations = selectable_installations(platform, discovered_installations);
    // Prefer the co-located portable install as the default selection so the
    // "drop RABBIT next to a portable REAPER" flow lands on it automatically;
    // otherwise fall back to the first writable target.
    let selected_target_index = colocated_portable
        .as_ref()
        .and_then(|root| {
            installations.iter().position(|installation| {
                installation.kind == InstallationKind::Portable
                    && &installation.resource_path == root
            })
        })
        .or_else(|| {
            installations
                .iter()
                .position(|installation| installation.writable)
        });
    let target = selected_target_index.and_then(|index| installations.get(index).cloned());
    // Default the model's architecture to the initially-selected target's
    // probed binary arch — that's what the artifact resolver actually
    // wants. Falls back to the host arch when no writable target was
    // discovered (the user will pick one manually before any artifact
    // work runs anyway).
    let architecture = target
        .as_ref()
        .and_then(|installation| installation.architecture)
        .unwrap_or_else(Architecture::current);
    let detections = match target.as_ref() {
        Some(target) => detect_components(&target.resource_path, platform)?,
        None => Vec::new(),
    };
    // Per-provider failures don't abort the model load: the packages whose
    // upstream answered keep their update flow, and the failed ones get
    // their rows disabled below with the failure recorded as a note.
    let (available, version_check_failures) = if options.online_versions {
        let report = fetch_latest_versions()?;
        (report.packages, report.failures)
    } else {
        (Vec::new(), Vec::new())
    };
    let desired = wizard_desired_package_ids(platform);
    let plan = build_install_plan(target, &detections, &desired, &available);

    let mut model = model_from_plan_with_options(
        &localizer,
        options,
        PlanModelInputs {
            platform,
            architecture,
            installations,
            selected_target_index,
            available_packages: available,
            plan,
        },
    );
    if !version_check_failures.is_empty() {
        let failures: Vec<(String, String)> = version_check_failures
            .into_iter()
            .map(|failure| (failure.package_id, failure.message))
            .collect();
        model.controls.can_install = apply_version_check_failures_to_rows(
            &localizer,
            &mut model.package_rows,
            &mut model.notes,
            &failures,
        );
    }
    Ok(model)
}

pub(crate) fn selectable_installations(
    platform: Platform,
    mut installations: Vec<Installation>,
) -> Vec<Installation> {
    if !installations
        .iter()
        .any(|installation| installation.kind == InstallationKind::Standard)
        && let Some(standard) = default_standard_installation(platform)
    {
        installations.push(standard);
    }
    installations
}

pub fn localizer_from_options(options: &UiBootstrapOptions) -> Result<Localizer> {
    match &options.locales_dir {
        Some(locales_dir) => Localizer::from_locale_dir(locales_dir, &options.locale),
        None => Localizer::embedded(&options.locale),
    }
}

pub fn model_from_plan(
    localizer: &Localizer,
    platform: Platform,
    architecture: Architecture,
    installations: Vec<Installation>,
    selected_target_index: Option<usize>,
    plan: InstallPlan,
) -> WizardModel {
    model_from_plan_with_options(
        localizer,
        UiBootstrapOptions::default(),
        PlanModelInputs {
            platform,
            architecture,
            installations,
            selected_target_index,
            available_packages: Vec::new(),
            plan,
        },
    )
}

/// The per-run inputs to [`model_from_plan_with_options`], grouped so the
/// call doesn't carry a long positional argument list.
struct PlanModelInputs {
    platform: Platform,
    architecture: Architecture,
    installations: Vec<Installation>,
    selected_target_index: Option<usize>,
    available_packages: Vec<AvailablePackage>,
    plan: InstallPlan,
}

fn model_from_plan_with_options(
    localizer: &Localizer,
    bootstrap_options: UiBootstrapOptions,
    inputs: PlanModelInputs,
) -> WizardModel {
    let PlanModelInputs {
        platform,
        architecture,
        installations,
        selected_target_index,
        available_packages,
        plan,
    } = inputs;
    let package_specs = builtin_package_specs(platform);
    let text = wizard_text(localizer);
    let target_rows = target_rows(localizer, &installations, selected_target_index);
    let host = detect_host_capabilities();
    let target_resource_path = selected_target_index
        .and_then(|idx| target_rows.get(idx))
        .map(|row| row.path.clone());
    let declined = target_resource_path
        .as_deref()
        .map(rabbit_core::receipt::declined_packages)
        .unwrap_or_default();
    let package_rows = package_rows(
        localizer,
        &text,
        platform,
        architecture,
        &package_specs,
        &plan.actions,
        &available_packages,
        &host,
        &declined,
    );
    let configuration_rows =
        configuration_rows(localizer, &package_rows, target_resource_path.as_deref());
    let review_lines = review_lines(localizer, &target_rows, &package_rows, &plan.notes);
    let can_install = package_rows
        .iter()
        .any(|row| matches!(row.action, PlanActionKind::Install | PlanActionKind::Update));

    WizardModel {
        window_title: format!(
            "{} v{}",
            localizer.text("app-title").value,
            env!("CARGO_PKG_VERSION")
        ),
        platform,
        architecture,
        bootstrap_options,
        current_step: WizardStep::Target,
        steps: wizard_steps(localizer),
        selected_target_index,
        target_rows,
        package_rows,
        configuration_rows,
        available_packages,
        review_lines,
        notes: plan.notes,
        text,
        controls: WizardControls {
            back_label: localized_wx_mnemonic_label(
                localizer,
                "wizard-button-back",
                "wizard-button-back-mnemonic",
            ),
            next_label: localized_wx_mnemonic_label(
                localizer,
                "wizard-button-next",
                "wizard-button-next-mnemonic",
            ),
            install_label: localized_wx_mnemonic_label(
                localizer,
                "wizard-button-install",
                "wizard-button-install-mnemonic",
            ),
            close_label: localized_wx_mnemonic_label(
                localizer,
                "wizard-button-close",
                "wizard-button-close-mnemonic",
            ),
            can_go_back: false,
            can_go_next: selected_target_index.is_some(),
            can_install,
        },
        language_options: language_options(localizer),
        current_language: localizer.active_locale().to_string(),
    }
}

pub(crate) fn language_options(localizer: &Localizer) -> Vec<LanguageOption> {
    let mut options: Vec<LanguageOption> = embedded_locales()
        .iter()
        .map(|locale| {
            let key = format!("wizard-locale-name-{locale}");
            let text = localizer.text(&key);
            let display_name = if text.missing {
                (*locale).to_string()
            } else {
                text.value
            };
            LanguageOption {
                locale: (*locale).to_string(),
                display_name,
            }
        })
        .collect();
    options.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    options
}
