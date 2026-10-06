//! Configuration page rows: the optional post-install steps and the
//! package dependencies that decide whether each one is available.

use std::collections::BTreeMap;
use std::path::Path;

use rabbit_core::localization::Localizer;

use super::model::{ConfigurationRow, PackageRow};
use super::packages::{package_row_installs_now, package_row_will_land_on_disk};

/// Build [`ConfigurationRow`]s from `rabbit-core`'s builtin step
/// catalogue, gating each row on whether its dependency package is
/// either already installed (action `Keep`) or queued for install /
/// update in the current package plan. Re-checked whenever the
/// package plan is rebuilt (target switch, post-install rescan, etc.).
pub fn configuration_rows(
    localizer: &Localizer,
    package_rows: &[PackageRow],
    target_resource_path: Option<&Path>,
) -> Vec<ConfigurationRow> {
    let installed_or_pending: BTreeMap<&str, bool> = package_rows
        .iter()
        .map(|row| (row.package_id.as_str(), package_row_will_land_on_disk(row)))
        .collect();
    let installing_now: BTreeMap<&str, bool> = package_rows
        .iter()
        .map(|row| (row.package_id.as_str(), package_row_installs_now(row)))
        .collect();

    rabbit_core::configuration::builtin_configuration_steps()
        .into_iter()
        .map(|step| {
            let display_name = localizer.text(&step.display_name_key).value;
            let description = {
                let text = localizer.text(&step.display_description_key);
                if text.missing {
                    String::new()
                } else {
                    text.value
                }
            };
            let dependency_satisfied =
                configuration_dependency_met(&step, &installed_or_pending, &installing_now);
            let already_applied = target_resource_path
                .and_then(|path| {
                    rabbit_core::configuration::is_configuration_step_applied(
                        path,
                        &step,
                        &Default::default(),
                    )
                    .ok()
                })
                .unwrap_or(false);

            let unavailability_reason = build_configuration_unavailability_reason(
                localizer,
                &step,
                dependency_satisfied,
                already_applied,
            );

            let summary = configuration_row_summary(
                localizer,
                &step,
                &display_name,
                dependency_satisfied,
                already_applied,
            );
            let details = if description.is_empty() {
                summary.clone()
            } else {
                format!("{summary}\n\n{description}")
            };

            ConfigurationRow {
                step_id: step.id.clone(),
                display_name,
                description,
                selected: dependency_satisfied && !already_applied && step.recommended,
                summary,
                details,
                available_for_target: dependency_satisfied,
                already_applied,
                unavailability_reason,
            }
        })
        .collect()
}

/// Build the tree-row label for a configuration step: the localized
/// display name on its own when the row is actionable, otherwise the
/// display name plus a short parenthesised status tag (`(requires
/// ReaPack)`, `(already applied)`) so the indicator is visible without
/// the user having to focus the row to read its details.
/// How to refer to a configuration step's dependency in a message.
///
/// A step satisfied by any one of several packages names the group ("a
/// language pack") rather than whichever package sorts first, which is how
/// "Set REAPER's language" came to advertise itself as requiring Spanish.
/// Steps with a single dependency keep naming it directly.
fn configuration_dependency_label(
    localizer: &Localizer,
    step: &rabbit_core::configuration::ConfigurationStep,
) -> String {
    if let Some(key) = step.dependency_name_key.as_deref() {
        let text = localizer.text(key);
        if !text.missing {
            return text.value;
        }
    }
    let dep_id = step.requires_packages.first().cloned().unwrap_or_default();
    let dep_name = localizer.text(&format!("package-{dep_id}"));
    if dep_name.missing {
        dep_id
    } else {
        dep_name.value
    }
}

pub(crate) fn configuration_row_summary(
    localizer: &Localizer,
    step: &rabbit_core::configuration::ConfigurationStep,
    display_name: &str,
    dependency_satisfied: bool,
    already_applied: bool,
) -> String {
    let status = if !dependency_satisfied {
        let dep_label = configuration_dependency_label(localizer, step);
        Some(
            localizer
                .format(
                    "wizard-configuration-row-status-requires",
                    &[("package", dep_label.as_str())],
                )
                .value,
        )
    } else if already_applied {
        Some(
            localizer
                .text("wizard-configuration-row-status-already-applied")
                .value,
        )
    } else {
        None
    };
    match status {
        Some(reason) => {
            let suffix = localizer
                .format(
                    "wizard-configuration-row-summary-suffix",
                    &[("reason", reason.as_str())],
                )
                .value;
            format!("{display_name} {suffix}")
        }
        None => display_name.to_string(),
    }
}

/// Build the localized "(unavailable: …)" / "(already configured)"
/// sentence shown on a configuration row that isn't actionable.
/// `dependency_satisfied == false` takes precedence over
/// `already_applied`: if the dep is missing and the row is also already
/// applied, we surface the dep error so the user knows the row is
/// gated rather than complete.
pub(crate) fn build_configuration_unavailability_reason(
    localizer: &Localizer,
    step: &rabbit_core::configuration::ConfigurationStep,
    dependency_satisfied: bool,
    already_applied: bool,
) -> Option<String> {
    if !dependency_satisfied {
        let dep_label = configuration_dependency_label(localizer, step);
        return Some(
            localizer
                .format(
                    "wizard-configuration-row-unavailable",
                    &[("package", dep_label.as_str())],
                )
                .value,
        );
    }
    if already_applied {
        return Some(
            localizer
                .text("wizard-configuration-row-already-applied")
                .value,
        );
    }
    None
}

/// Re-evaluate each [`ConfigurationRow`]'s `available_for_target` /
/// `selected` / `unavailability_reason` against the current package
/// rows. Called by the wizard whenever the user toggles a package row
/// — e.g. unticking ReaPack should immediately disable the
/// "configure REAPER Accessibility ReaPack remote" row, and re-ticking
/// it should re-enable + re-recommend the row.
///
/// Selection is preserved across the recompute *unless* the row goes
/// from available → unavailable, in which case it's force-unticked
/// (we never want to ship the install with a configuration step
/// queued whose dependency isn't available).
/// ANY-of dependency check for a configuration step: satisfied when at least
/// one required package will be on disk, or when the step requires none.
/// "Set REAPER's language" lists every language pack, so it lights up
/// whichever one the user picked.
fn configuration_dependency_satisfied(
    step: &rabbit_core::configuration::ConfigurationStep,
    mut is_present: impl FnMut(&str) -> bool,
) -> bool {
    step.requires_packages.is_empty()
        || step
            .requires_packages
            .iter()
            .any(|pkg| is_present(pkg.as_str()))
}

/// Is this step's dependency satisfied?
///
/// Steps flagged `requires_fresh_dependency` count only packages being
/// installed or updated in this run; every other step also counts packages
/// that are already on disk.
fn configuration_dependency_met(
    step: &rabbit_core::configuration::ConfigurationStep,
    landing: &BTreeMap<&str, bool>,
    installing: &BTreeMap<&str, bool>,
) -> bool {
    let source = if step.requires_fresh_dependency {
        installing
    } else {
        landing
    };
    configuration_dependency_satisfied(step, |pkg| source.get(pkg).copied().unwrap_or(false))
}

pub fn recompute_configuration_row_availability(
    localizer: &Localizer,
    package_rows: &[PackageRow],
    target_resource_path: Option<&Path>,
    configuration_rows: &mut [ConfigurationRow],
) {
    let installed_or_pending: BTreeMap<&str, bool> = package_rows
        .iter()
        .map(|row| (row.package_id.as_str(), package_row_will_land_on_disk(row)))
        .collect();
    let installing_now: BTreeMap<&str, bool> = package_rows
        .iter()
        .map(|row| (row.package_id.as_str(), package_row_installs_now(row)))
        .collect();
    let steps = rabbit_core::configuration::builtin_configuration_steps();
    for row in configuration_rows.iter_mut() {
        let Some(step) = steps.iter().find(|step| step.id == row.step_id) else {
            continue;
        };
        let dependency_satisfied =
            configuration_dependency_met(step, &installed_or_pending, &installing_now);
        let already_applied = target_resource_path
            .and_then(|path| {
                rabbit_core::configuration::is_configuration_step_applied(
                    path,
                    step,
                    &Default::default(),
                )
                .ok()
            })
            .unwrap_or(row.already_applied);
        let was_actionable = row.available_for_target && !row.already_applied;
        row.available_for_target = dependency_satisfied;
        row.already_applied = already_applied;
        row.unavailability_reason = build_configuration_unavailability_reason(
            localizer,
            step,
            dependency_satisfied,
            already_applied,
        );
        // Refresh the row's tree-label so the inline status tag matches
        // the new state (e.g. unticking ReaPack while the row is
        // visible adds "(requires ReaPack)"; re-ticking it removes the
        // tag).
        row.summary = configuration_row_summary(
            localizer,
            step,
            &row.display_name,
            dependency_satisfied,
            already_applied,
        );
        row.details = if row.description.is_empty() {
            row.summary.clone()
        } else {
            format!("{}\n\n{}", row.summary, row.description)
        };
        let actionable_now = dependency_satisfied && !already_applied;
        if !actionable_now {
            // Nothing to act on, so nothing ticked. Unticking the last
            // language pack clears "Set REAPER's language" through this
            // branch, rather than leaving a ticked step pointing at no pack.
            row.selected = false;
        } else if !was_actionable {
            // Just became actionable — e.g. a language pack was ticked.
            // Restore the recommended default so the user doesn't have to
            // hunt for the step; unticking it afterwards survives, because
            // by then the row was already actionable.
            row.selected = step.recommended;
        }
    }
}

/// Return the step ids of configuration rows that are both actionable
/// (available + not already applied) and currently selected. Mirrors
/// `package_ids_for_rows` but for configuration rows.
pub fn selected_configuration_step_ids(configuration_rows: &[ConfigurationRow]) -> Vec<String> {
    configuration_rows
        .iter()
        .filter(|row| row.available_for_target && !row.already_applied && row.selected)
        .map(|row| row.step_id.clone())
        .collect()
}
