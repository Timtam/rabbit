//! The Uninstall tab: one row for each package installed in the selected
//! REAPER, saying whether RABBIT can remove it, and how ticking a row
//! changes it.
//!
//! The rows come from the uninstall plan `rabbit-core` builds, the plan
//! `rabbit components` and the dry run of `rabbit uninstall` read. Building
//! it only asks which files exist and how big they are, and never reads a
//! file's contents, so the UI thread can afford it: the version check's
//! handler builds the rows from the detection it has just run.

use rabbit_core::Result;
use rabbit_core::localization::Localizer;
use rabbit_core::model::{ComponentDetection, InstallationKind};
use rabbit_core::package::{PackageCategory, PackageSpec, builtin_package_specs};
use rabbit_core::receipt::load_install_state;
use rabbit_core::uninstall::{
    Removability, UNINSTALL_NOT_YET_KEY, UninstallNote, UninstallPlan, UninstallPlanInput,
    UninstallPlanItem, UninstallRoots, UninstallWarning, build_uninstall_plan,
};

use super::bootstrap::localizer_from_options;
use super::labels::version_text;
use super::model::{TargetRow, WizardModel, WizardText};
use super::packages::localized_package_display_name;

/// One installed package on the Uninstall tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallRow {
    pub package_id: String,
    /// The localized package name.
    pub display_name: String,
    /// The install tree's group the row sits in.
    pub category: PackageCategory,
    /// RABBIT can remove the package, and this version can carry the removal
    /// out. A row that can't be ticked stays in the list all the same, so
    /// the user learns why RABBIT leaves the package alone.
    pub can_tick: bool,
    /// Ticked to uninstall. Nothing is ticked until the user ticks it.
    pub selected: bool,
    /// Why the row can't be ticked, as the short phrase its label ends with.
    pub unavailability_reason: Option<String>,
    /// The row's label. It starts with the action, as the install rows do,
    /// so a screen reader says what will happen before anything else.
    pub summary: String,
    /// The details text: the label again, then why the row can't be ticked,
    /// or what uninstalling removes, what stays and what to look out for.
    pub details: String,
    /// The label while unticked and while ticked. Ticking only swaps them,
    /// so it needs neither the plan nor a localizer.
    pub(crate) keep_summary: String,
    pub(crate) uninstall_summary: String,
    /// The details text below the label, the same ticked or not.
    pub(crate) explanation: String,
}

/// What the Uninstall tab lists for a target.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WizardUninstallList {
    /// One row per installed package, in manifest order. The tab groups them
    /// by category, as the install tree does.
    pub rows: Vec<UninstallRow>,
    /// Why there are no rows when RABBIT couldn't read its own records for
    /// the target: without the receipt it can't tell its files from the
    /// user's. `None` with no rows means nothing it knows is installed.
    pub unreadable: Option<String>,
}

impl WizardUninstallList {
    /// The label and the details text of the row, without a checkbox, that
    /// stands in for an empty list. `None` while there are rows.
    pub fn placeholder(&self, text: &WizardText) -> Option<(String, String)> {
        if !self.rows.is_empty() {
            return None;
        }
        Some(match &self.unreadable {
            Some(message) => (message.clone(), message.clone()),
            None => (
                text.uninstall_empty.clone(),
                text.uninstall_empty_details.clone(),
            ),
        })
    }
}

/// The uninstall plan for `target`, built from `detections` and the target's
/// receipt the way `rabbit uninstall` builds it. The target's kind and its
/// REAPER come from the target row rather than from guessing by the folder,
/// so the rows and a later run from the wizard agree about them. Reads which
/// files exist and how big they are; hashes nothing and changes nothing.
pub fn uninstall_plan_for_target(
    model: &WizardModel,
    target: &TargetRow,
    detections: &[ComponentDetection],
) -> Result<UninstallPlan> {
    let roots =
        UninstallRoots::for_target(model.platform, &target.path, target.app_path.as_deref());
    let state = load_install_state(&target.path)?;
    Ok(build_uninstall_plan(&UninstallPlanInput {
        roots: &roots,
        platform: model.platform,
        target: if target.portable {
            InstallationKind::Portable
        } else {
            InstallationKind::Standard
        },
        specs: &builtin_package_specs(model.platform),
        detections,
        state: state.as_ref(),
        remove_settings: false,
    }))
}

/// The Uninstall tab's rows for `target`. A receipt that can't be read
/// leaves the list empty with the reason, rather than guessing which files
/// are RABBIT's.
pub fn uninstall_list_for_target(
    model: &WizardModel,
    target: &TargetRow,
    detections: &[ComponentDetection],
) -> WizardUninstallList {
    let Ok(localizer) = localizer_from_options(&model.bootstrap_options) else {
        return WizardUninstallList::default();
    };
    match uninstall_plan_for_target(model, target, detections) {
        Ok(plan) => {
            uninstall_list_from_plan(&localizer, &builtin_package_specs(model.platform), &plan)
        }
        Err(error) => WizardUninstallList {
            rows: Vec::new(),
            unreadable: Some(
                localizer
                    .format(
                        "wizard-uninstall-unreadable",
                        &[("error", error.to_string().as_str())],
                    )
                    .value,
            ),
        },
    }
}

/// The rows for every package `plan` finds installed, ticked or not, in
/// manifest order. Packages that aren't installed get no row.
pub fn uninstall_list_from_plan(
    localizer: &Localizer,
    specs: &[PackageSpec],
    plan: &UninstallPlan,
) -> WizardUninstallList {
    WizardUninstallList {
        rows: plan
            .items
            .iter()
            .filter(|item| item.removability != Removability::NotInstalled)
            .map(|item| uninstall_row(localizer, specs, item))
            .collect(),
        unreadable: None,
    }
}

/// Tick or untick `row`. A row that can't be ticked stays unticked. Returns
/// whether the row changed; either way `row` ends up matching what it shows.
pub fn apply_uninstall_checkbox(row: &mut UninstallRow, checked: bool) -> bool {
    let checked = checked && row.can_tick;
    let changed = row.selected != checked;
    row.selected = checked;
    row.summary = if checked {
        row.uninstall_summary.clone()
    } else {
        row.keep_summary.clone()
    };
    row.details = details_text(&row.summary, &row.explanation);
    changed
}

fn uninstall_row(
    localizer: &Localizer,
    specs: &[PackageSpec],
    item: &UninstallPlanItem,
) -> UninstallRow {
    let spec = specs.iter().find(|spec| spec.id == item.package_id);
    let display_name = spec
        .map(|spec| localizer.text(&spec.display_name_key))
        .filter(|text| !text.missing)
        .map(|text| text.value)
        .unwrap_or_else(|| item.display_name.clone());
    let installed = version_text(localizer, item.installed_version.as_ref());
    // The engine refuses a removal a backup can't undo until the phase that
    // brings it, so a row for one would only lead to a refusal later on.
    let can_tick = item.removability.is_removable() && !item.unbacked;
    let unavailability_reason = (!can_tick).then(|| unavailability_reason(localizer, item));
    let label = |action_key: &str| {
        let action = localizer.text(action_key).value;
        let row = localizer
            .format(
                "wizard-uninstall-row",
                &[
                    ("package", display_name.as_str()),
                    ("action", action.as_str()),
                    ("installed", installed.as_str()),
                ],
            )
            .value;
        match &unavailability_reason {
            Some(reason) => {
                let suffix = localizer
                    .format(
                        "wizard-package-row-unavailable-suffix",
                        &[("reason", reason.as_str())],
                    )
                    .value;
                format!("{row} {suffix}")
            }
            None => row,
        }
    };
    let keep_summary = label("action-keep");
    let uninstall_summary = label("action-uninstall");
    let explanation = if can_tick {
        removal_explanation(localizer, item, &display_name)
    } else {
        refusal_explanation(localizer, item, &display_name)
    };
    UninstallRow {
        package_id: item.package_id.clone(),
        category: spec.map(|spec| spec.category).unwrap_or_default(),
        can_tick,
        selected: false,
        unavailability_reason,
        details: details_text(&keep_summary, &explanation),
        summary: keep_summary.clone(),
        display_name,
        keep_summary,
        uninstall_summary,
        explanation,
    }
}

fn details_text(summary: &str, explanation: &str) -> String {
    if explanation.is_empty() {
        summary.to_string()
    } else {
        format!("{summary}\n\n{explanation}")
    }
}

/// The short phrase a row that can't be ticked ends with.
fn unavailability_reason(localizer: &Localizer, item: &UninstallPlanItem) -> String {
    let key = match &item.removability {
        Removability::Manual { .. } => "wizard-uninstall-row-unavailable-manual",
        Removability::NotSupported { note_key } if note_key != UNINSTALL_NOT_YET_KEY => {
            "wizard-uninstall-row-unavailable-not-supported"
        }
        Removability::ManagedByReaPack { .. } => "wizard-uninstall-row-unavailable-reapack",
        Removability::ReaPackCheckFailed { .. } => {
            "wizard-uninstall-row-unavailable-reapack-check-failed"
        }
        // The same reason, and the same words, as the install row of a
        // package a portable REAPER can't have.
        Removability::NeedsStandardTarget => "wizard-package-row-unavailable-portable",
        Removability::NoReceipt => "wizard-uninstall-row-unavailable-no-receipt",
        Removability::NoTrustedUninstaller => {
            "wizard-uninstall-row-unavailable-no-trusted-uninstaller"
        }
        Removability::UnsafePath { .. } => "wizard-uninstall-row-unavailable-unsafe-path",
        // Removable, but only through a step this version can't take yet;
        // or not supported until a later version. A package that isn't
        // installed gets no row at all.
        Removability::NotSupported { .. }
        | Removability::Removable
        | Removability::RemovableOrphanedFromReaPack
        | Removability::NotInstalled => "wizard-uninstall-row-unavailable-not-yet",
    };
    localizer.text(key).value
}

/// Why RABBIT leaves the package alone, in full sentences: the same reasons
/// the CLI prints, in the user's language, and the manifest's own
/// instructions for a package to remove by hand.
fn refusal_explanation(localizer: &Localizer, item: &UninstallPlanItem, name: &str) -> String {
    let format = |key: &str, args: &[(&str, &str)]| localizer.format(key, args).value;
    match &item.removability {
        Removability::Manual { note_key } | Removability::NotSupported { note_key } => {
            localizer.text(note_key).value
        }
        Removability::ManagedByReaPack {
            path,
            reapack_package,
        } => format(
            "wizard-uninstall-details-reapack",
            &[
                ("package", name),
                ("path", path.display().to_string().as_str()),
                ("reapack_package", reapack_package.as_str()),
            ],
        ),
        Removability::ReaPackCheckFailed { path, reason } => format(
            "wizard-uninstall-details-reapack-check-failed",
            &[
                ("package", name),
                ("path", path.display().to_string().as_str()),
                ("reason", reason.as_str()),
            ],
        ),
        Removability::NeedsStandardTarget => {
            format("wizard-uninstall-details-portable", &[("package", name)])
        }
        Removability::NoReceipt => {
            format("wizard-uninstall-details-no-receipt", &[("package", name)])
        }
        Removability::NoTrustedUninstaller => format(
            "wizard-uninstall-details-no-trusted-uninstaller",
            &[("package", name)],
        ),
        Removability::UnsafePath { path, .. } => format(
            "wizard-uninstall-details-unsafe-path",
            &[
                ("package", name),
                ("path", path.display().to_string().as_str()),
            ],
        ),
        Removability::Removable
        | Removability::RemovableOrphanedFromReaPack
        | Removability::NotInstalled => localizer.text(UNINSTALL_NOT_YET_KEY).value,
    }
}

/// What uninstalling the package does, one short sentence per line, in the
/// order the plan gives: warnings that always apply first, then what goes,
/// what stays, the other warnings and the notes. No file lists or counts;
/// those belong to the report.
fn removal_explanation(localizer: &Localizer, item: &UninstallPlanItem, name: &str) -> String {
    let text = |key: &str| localizer.text(key).value;
    let format = |key: &str, args: &[(&str, &str)]| localizer.format(key, args).value;
    let mut lines: Vec<String> = item
        .warnings
        .iter()
        .filter_map(|warning| match warning {
            UninstallWarning::Always { message_key, .. } => Some(text(message_key)),
            _ => None,
        })
        .collect();
    if let Some(summary) = &item.summary {
        lines.push(text(&summary.removes_key));
    }
    lines.push(text("wizard-uninstall-details-files"));
    if let Some(summary) = &item.summary {
        lines.push(text(&summary.keeps_key));
    }
    for warning in &item.warnings {
        match warning {
            UninstallWarning::Always { .. } => {}
            UninstallWarning::OtherPackageInstalled { message_key, .. } => {
                lines.push(text(message_key));
            }
            UninstallWarning::DependentsStay { package_ids } => {
                let packages = package_ids
                    .iter()
                    .map(|id| localized_package_display_name(localizer, id))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format(
                    "wizard-uninstall-details-dependents-stay",
                    &[("package", name), ("packages", packages.as_str())],
                ));
            }
            UninstallWarning::MachineWide => lines.push(format(
                "wizard-uninstall-details-machine-wide",
                &[("package", name)],
            )),
            UninstallWarning::ReaPackStillManages { reapack_packages } => lines.push(format(
                "wizard-uninstall-details-reapack-owned-extensions",
                &[("packages", reapack_packages.join(", ").as_str())],
            )),
            UninstallWarning::ReaPackRecordsUnreadable { reason } => lines.push(format(
                "wizard-uninstall-details-reapack-records-unreadable",
                &[("reason", reason.as_str())],
            )),
        }
    }
    for note in &item.notes {
        lines.push(match note {
            UninstallNote::NoReceipt => format(
                "wizard-uninstall-details-no-receipt-note",
                &[("package", name)],
            ),
            UninstallNote::OrphanedFromReaPack { .. } => format(
                "wizard-uninstall-details-reapack-orphan",
                &[("package", name)],
            ),
            UninstallNote::Manifest { note_key } => text(note_key),
        });
    }
    lines.join("\n")
}

/// The command line the "can't uninstall from the wizard yet" dialog
/// suggests, for the REAPER folder the wizard is on. `--resource-path` is
/// required, so a command without it would fail as typed.
pub fn uninstall_cli_example(resource_path: &std::path::Path) -> String {
    format!(
        "rabbit uninstall --resource-path \"{}\" --package sws",
        resource_path.display()
    )
}

/// The dialog's text, with the example command for `resource_path`.
pub fn uninstall_not_ready_body(
    localizer: &rabbit_core::localization::Localizer,
    resource_path: &std::path::Path,
) -> String {
    let command = uninstall_cli_example(resource_path);
    localizer
        .format(
            "wizard-uninstall-not-ready-body",
            &[("command", command.as_str())],
        )
        .value
}
