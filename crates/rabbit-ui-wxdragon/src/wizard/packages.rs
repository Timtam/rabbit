//! Package page rows: what is offered for a target, what stays ticked, and
//! how a checkbox toggle changes a row.

use std::collections::BTreeMap;

use rabbit_core::Result;
use rabbit_core::detection::detect_components;
use rabbit_core::localization::Localizer;
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::operation::{PackageAutomationSupport, package_automation_support};
use rabbit_core::package::{
    HostCapabilities, PACKAGE_OSARA, PackageSpec, builtin_package_specs, detect_host_capabilities,
    host_supports_package,
};
use rabbit_core::plan::{AvailablePackage, PlanAction, PlanActionKind, build_install_plan};

use super::bootstrap::localizer_from_options;
use super::labels::{action_label, version_text};
use super::model::{
    OsaraKeymapChoice, PackageRow, TargetRow, WizardModel, WizardPackagePlan, WizardText,
};
use super::target::installation_from_target_row;

pub fn package_ids_for_indices(model: &WizardModel, indices: &[usize]) -> Vec<String> {
    package_ids_for_rows(&model.package_rows, indices)
}

pub fn package_ids_for_rows(package_rows: &[PackageRow], indices: &[usize]) -> Vec<String> {
    let mut package_ids = Vec::new();
    for index in indices {
        let Some(row) = package_rows.get(*index) else {
            continue;
        };
        if !package_ids.contains(&row.package_id) {
            package_ids.push(row.package_id.clone());
        }
    }
    package_ids
}

pub fn osara_selected_for_rows(package_rows: &[PackageRow], indices: &[usize]) -> bool {
    indices
        .iter()
        .filter_map(|index| package_rows.get(*index))
        .any(|row| row.package_id == PACKAGE_OSARA)
}

/// Whether a package offering a variant choice is currently selected for
/// install, so the wizard can enable its choice control. Today only the
/// Spanish language pack qualifies: its two variants pick which OSARA
/// translation loads.
pub fn variant_choice_package_selected(package_rows: &[PackageRow], indices: &[usize]) -> bool {
    indices
        .iter()
        .filter_map(|index| package_rows.get(*index))
        .any(|row| row.package_id == VARIANT_CHOICE_PACKAGE_ID)
}

/// Ticked language packs, in row order, as (package id, display name). The
/// wizard's "REAPER language after installation" dropdown lists these: any
/// number can be installed side by side, but exactly one is made active.
pub fn selected_language_packs(
    package_rows: &[PackageRow],
    indices: &[usize],
) -> Vec<(String, String)> {
    indices
        .iter()
        .filter_map(|index| package_rows.get(*index))
        .filter(|row| row.category == rabbit_core::package::PackageCategory::Language)
        .map(|row| (row.package_id.clone(), row.display_name.clone()))
        .collect()
}

/// The one package that currently offers a variant choice in the wizard.
pub const VARIANT_CHOICE_PACKAGE_ID: &str = "langpack-es";
/// The selectable Spanish OSARA translations, in the order the wizard's
/// dropdown lists them. Index 0 is the default. Kept as an explicit list so
/// the dropdown's position maps to a manifest variant id without the UI
/// hard-coding which is which.
pub const VARIANT_CHOICE_IDS: &[&str] = &["rae", "pma"];

/// Build the `package_variants` map from the wizard's dropdown selection.
///
/// Always records an explicit choice. Picking the first entry has to mean
/// "use REAPER Accesible español", not "no opinion" — the install path
/// falls back to the previously-installed variant when no choice is given,
/// so an absent entry would make it impossible to switch back from Team PMA
/// once chosen. An out-of-range index falls back to the first entry.
pub fn package_variants_from_choice(
    selection: Option<u32>,
) -> std::collections::BTreeMap<String, String> {
    let index = selection.unwrap_or(0) as usize;
    let id = VARIANT_CHOICE_IDS
        .get(index)
        .copied()
        .unwrap_or(VARIANT_CHOICE_IDS[0]);
    let mut variants = std::collections::BTreeMap::new();
    variants.insert(VARIANT_CHOICE_PACKAGE_ID.to_string(), id.to_string());
    variants
}

/// Dropdown index matching the Spanish variant currently installed at
/// `resource_path`, so the wizard shows the user's existing choice rather
/// than a default that would silently switch them. Falls back to the first
/// entry when nothing is installed.
pub fn spanish_variant_selection(resource_path: &std::path::Path) -> u32 {
    let installed = rabbit_core::package::effective_variant_id(
        resource_path,
        VARIANT_CHOICE_PACKAGE_ID,
        &std::collections::BTreeMap::new(),
    );
    installed
        .and_then(|id| {
            VARIANT_CHOICE_IDS
                .iter()
                .position(|candidate| *candidate == id)
        })
        .unwrap_or(0) as u32
}

/// Returns true when ReaPack is selected and its planned action would
/// actually stage the package (Install or Update), i.e. the run will need
/// the donation acknowledgement before it proceeds.
pub fn reapack_selected_for_install_or_update(
    package_rows: &[PackageRow],
    indices: &[usize],
) -> bool {
    indices
        .iter()
        .filter_map(|index| package_rows.get(*index))
        .any(|row| {
            row.package_id == rabbit_core::package::PACKAGE_REAPACK
                && matches!(row.action, PlanActionKind::Install | PlanActionKind::Update)
        })
}

pub fn osara_keymap_note(
    model: &WizardModel,
    osara_selected: bool,
    choice: OsaraKeymapChoice,
) -> String {
    if !osara_selected {
        return model.text.packages_osara_keymap_unavailable_note.clone();
    }

    match choice {
        OsaraKeymapChoice::PreserveCurrent => {
            model.text.packages_osara_keymap_preserve_note.clone()
        }
        OsaraKeymapChoice::ReplaceCurrent => model.text.packages_osara_keymap_replace_note.clone(),
    }
}

pub fn wizard_package_plan_for_target(
    model: &WizardModel,
    target: Option<&TargetRow>,
) -> Result<WizardPackagePlan> {
    wizard_package_plan_for_target_with_available(model, target, &model.available_packages)
}

/// Like `wizard_package_plan_for_target`, but uses an explicit
/// `available_packages` list instead of the one stored in the model. This is
/// what the wizard calls after the GUI's background latest-version fetch
/// completes so the package list can be re-rendered with fresh upstream data
/// without rebuilding the whole model.
pub fn wizard_package_plan_for_target_with_available(
    model: &WizardModel,
    target: Option<&TargetRow>,
    available_packages: &[AvailablePackage],
) -> Result<WizardPackagePlan> {
    let localizer = localizer_from_options(&model.bootstrap_options)?;
    let detections = match target {
        Some(target) => detect_components(&target.path, model.platform)?,
        None => Vec::new(),
    };
    let desired = wizard_desired_package_ids(model.platform);
    let plan = build_install_plan(
        target.map(|target| installation_from_target_row(model, target)),
        &detections,
        &desired,
        available_packages,
    );
    let package_specs = builtin_package_specs(model.platform);
    let host = detect_host_capabilities();
    let declined = target
        .map(|target| rabbit_core::receipt::declined_packages(&target.path))
        .unwrap_or_default();
    let mut package_rows = package_rows(
        &localizer,
        &model.text,
        model.platform,
        model.architecture,
        &package_specs,
        &plan.actions,
        available_packages,
        &host,
        &declined,
    );

    // Some packages can't honor a portable target: they install to a fixed
    // location outside any portable REAPER folder. JAWS-for-REAPER scripts'
    // NSIS package hard-codes `%APPDATA%\REAPER\UserPlugins\`; Surge XT's
    // vendor installer writes to the system VST3 root + ProgramData /
    // /Library/Application Support; app2clap installs into the per-user CLAP
    // folder. Each declares `requires_standard_install` in the manifest, so
    // the gate is data-driven rather than a hardcoded package-id list. Mark
    // those rows unavailable on a portable target so the checklist disables
    // their checkboxes and each row label carries a localized "(requires
    // standard installation)" indicator.
    if target.is_some_and(|target| target.portable) {
        for row in &mut package_rows {
            if row.requires_standard_install {
                mark_row_unavailable(&localizer, row, "wizard-package-row-unavailable-portable");
            }
        }
    }

    let can_install = package_rows.iter().any(|row| {
        row.available_for_target
            && matches!(row.action, PlanActionKind::Install | PlanActionKind::Update)
    });

    Ok(WizardPackagePlan {
        package_rows,
        notes: plan.notes,
        can_install,
    })
}

/// Mark `row` as unavailable: force-uncheck it, record the localized reason,
/// and append a localized "(not available: <reason>)" indicator to the row
/// summary so the indicator shows up in the package CheckListBox label.
///
/// Also flips the row's displayed action to `Keep` (mirroring what the
/// auto-untick path in [`package_rows`] does for non-recommended Install/
/// Update rows). Without this, an Install/Update row that becomes unavailable
/// — e.g. JAWS-for-REAPER scripts on a portable target — would still read
/// "Will install" / "Will update" while sitting unticked and disabled.
/// `original_action` is preserved so the row can revert to its plan-time
/// intent if the unavailability is later lifted.
fn mark_row_unavailable(localizer: &Localizer, row: &mut PackageRow, reason_key: &str) {
    let reason = localizer.text(reason_key).value;
    row.available_for_target = false;
    row.selected = false;
    row.action = PlanActionKind::Keep;
    row.action_label = action_label(localizer, PlanActionKind::Keep);
    let summary = localizer
        .format(
            "wizard-package-row",
            &[
                ("package", row.display_name.as_str()),
                ("action", row.action_label.as_str()),
                ("installed", row.installed_version.as_str()),
                ("available", row.available_version.as_str()),
            ],
        )
        .value;
    let indicator = localizer
        .format(
            "wizard-package-row-unavailable-suffix",
            &[("reason", reason.as_str())],
        )
        .value;
    row.summary = format!("{summary} {indicator}");
    row.unavailability_reason = Some(reason);
}

/// Apply per-package latest-version-check failures to a built package-row
/// set. Each failed package's row is force-unchecked and disabled (same
/// mechanism as the portable-target restriction) with a localized "version
/// check failed" reason in its label, and a note carrying the package name
/// plus the full error message is appended for the Review page. One
/// unreachable upstream (e.g. the SWS homepage being down) therefore
/// disables just that package instead of blocking the whole update flow.
///
/// Returns the recomputed "at least one actionable Install/Update row is
/// left" flag, which the caller should store as its can-install state.
pub fn apply_version_check_failures_to_rows(
    localizer: &Localizer,
    package_rows: &mut [PackageRow],
    notes: &mut Vec<String>,
    failures: &[(String, String)],
) -> bool {
    for (package_id, message) in failures {
        if let Some(row) = package_rows
            .iter_mut()
            .find(|row| row.package_id == *package_id)
        {
            mark_row_unavailable(
                localizer,
                row,
                "wizard-package-row-unavailable-version-check",
            );
        }
        let display = localized_package_display_name(localizer, package_id);
        notes.push(
            localizer
                .format(
                    "wizard-version-check-failed-note",
                    &[("package", display.as_str()), ("message", message.as_str())],
                )
                .value,
        );
    }
    package_rows.iter().any(|row| {
        row.available_for_target
            && matches!(row.action, PlanActionKind::Install | PlanActionKind::Update)
    })
}

/// Recompute a `PackageRow`'s `action`, `action_label`, `summary`, and
/// `selected` fields to match a new checkbox state. Used by the wizard's
/// CheckListBox toggle handler so the visible "Install/Update/Keep" label
/// follows what the user just clicked. Returns the freshly-formatted
/// summary so the caller can also push it into the CheckListBox label.
pub fn apply_checkbox_state_to_package_row(
    model: &WizardModel,
    row: &mut PackageRow,
    checked: bool,
) -> Result<String> {
    let localizer = localizer_from_options(&model.bootstrap_options)?;
    let new_action = if checked {
        // Originally-not-installed packages stay "Install" when re-checked;
        // anything else means the package is on disk, so re-checking it
        // means "Update" (re-stage the latest known upstream version).
        match row.original_action {
            PlanActionKind::Install => PlanActionKind::Install,
            _ => PlanActionKind::Update,
        }
    } else {
        PlanActionKind::Keep
    };
    let action_label = action_label(&localizer, new_action);
    let summary = localizer
        .format(
            "wizard-package-row",
            &[
                ("package", row.display_name.as_str()),
                ("action", action_label.as_str()),
                ("installed", row.installed_version.as_str()),
                ("available", row.available_version.as_str()),
            ],
        )
        .value;
    row.action = new_action;
    row.action_label = action_label;
    row.summary = summary.clone();
    row.selected = checked;
    Ok(summary)
}

/// Localized package display name for `package_id`, falling back to the raw id
/// when no Fluent key is available. Used by the version-check progress log.
pub fn localized_package_display_name(localizer: &Localizer, package_id: &str) -> String {
    let key = format!("package-{package_id}");
    let text = localizer.text(&key);
    if text.missing {
        package_id.to_string()
    } else {
        text.value
    }
}

/// List of package ids the wizard cares about for a given platform — exposed
/// so the GUI can iterate them without duplicating builtin_package_specs.
/// Host-conditional packages (e.g. JAWS-for-REAPER scripts) are filtered out
/// when the corresponding host facility isn't available.
pub fn wizard_desired_package_ids(platform: Platform) -> Vec<String> {
    wizard_desired_package_ids_for_host(platform, &detect_host_capabilities())
}

/// Same as [`wizard_desired_package_ids`] but with an explicit host snapshot,
/// so tests can pin "JAWS detected"/"JAWS missing" without touching the real
/// filesystem.
pub fn wizard_desired_package_ids_for_host(
    platform: Platform,
    host: &HostCapabilities,
) -> Vec<String> {
    builtin_package_specs(platform)
        .into_iter()
        .filter(|spec| host_supports_package(spec, host))
        .map(|spec| spec.id)
        .collect()
}

#[allow(clippy::too_many_arguments)] // Row building needs the full wizard context.
pub(crate) fn package_rows(
    localizer: &Localizer,
    text: &WizardText,
    platform: Platform,
    architecture: Architecture,
    package_specs: &[PackageSpec],
    actions: &[PlanAction],
    available_packages: &[AvailablePackage],
    host: &HostCapabilities,
    declined: &std::collections::BTreeSet<String>,
) -> Vec<PackageRow> {
    let specs_by_id: BTreeMap<_, _> = package_specs
        .iter()
        .map(|spec| (spec.id.as_str(), spec))
        .collect();
    let whats_new_by_id: BTreeMap<_, _> = available_packages
        .iter()
        .filter_map(|available| {
            available
                .whats_new
                .as_deref()
                .map(|notes| (available.package_id.as_str(), notes))
        })
        .collect();
    actions
        .iter()
        .map(|action| {
            let spec = specs_by_id.get(action.package_id.as_str()).copied();
            let display_name = spec
                .map(|spec| localizer.text(&spec.display_name_key).value)
                .unwrap_or_else(|| action.package_id.clone());
            let description = spec
                .map(|spec| localizer.text(&spec.display_description_key))
                .filter(|text| !text.missing)
                .map(|text| text.value)
                .unwrap_or_default();
            let installed_version = version_text(localizer, action.installed_version.as_ref());
            let available_version = version_text(localizer, action.available_version.as_ref());
            // Auto-tick rule:
            //  - Update → always ticked (the package is already on disk; the
            //    user opted into having it, so keep it current by default).
            //  - Install → only ticked when the spec is *effectively*
            //    recommended. That's the manifest baseline OR a host-conditional
            //    escalation (e.g. ReaKontrol's `recommended_when:
            //    komplete_kontrol_installed`), so non-recommended packages
            //    (FFmpeg, plain ReaKontrol on a non-KK host) stay unchecked.
            //  - Keep → never ticked (nothing to do).
            // A language pack for the language RABBIT is running in counts
            // as recommended, so a Spanish user gets the Spanish pack ticked
            // without hunting for it. Packs for other languages stay listed
            // but unticked; nothing is offered for English, since REAPER is
            // already English.
            // Has the user turned this package down on this install? It
            // stays listed and tickable either way — a refusal suppresses
            // the automatic tick, never the package itself.
            let refused = spec
                .is_some_and(|spec| spec.remember_opt_out && declined.contains(&action.package_id));
            let recommended = spec
                .map(|spec| {
                    rabbit_core::package::effective_recommended(spec, host)
                        || rabbit_core::package::matches_ui_language(
                            spec,
                            localizer.active_locale(),
                        )
                })
                .unwrap_or(false);
            let initially_selected = match action.action {
                // Updates are ticked by default because a package you have
                // should stay current — but not once you have said no to it.
                // Someone who stopped using a language pack and turned its
                // update down would otherwise be re-offered it, ticked,
                // every time the translators publish a fix.
                PlanActionKind::Update => !refused,
                PlanActionKind::Install => recommended && !refused,
                PlanActionKind::Keep => false,
            };
            // When the auto-tick rule leaves an Install/Update row unticked,
            // mirror what `apply_checkbox_state_to_package_row(checked=false)`
            // does on a manual untick: flip the *displayed* action to Keep so
            // the row label / summary match the checkbox state. `original_action`
            // still records the plan's decision so re-ticking restores Install/
            // Update without losing the underlying intent.
            let initial_action = if initially_selected {
                action.action
            } else {
                PlanActionKind::Keep
            };
            let action_label = action_label(localizer, initial_action);
            let summary = localizer
                .format(
                    "wizard-package-row",
                    &[
                        ("package", display_name.as_str()),
                        ("action", action_label.as_str()),
                        ("installed", installed_version.as_str()),
                        ("available", available_version.as_str()),
                    ],
                )
                .value;
            let (handling_summary, manual_attention_expected) =
                package_handling_summary(text, &action.package_id, platform, architecture);
            // Compose the details text shown in the wizard's package
            // details pane. The localized description follows the summary
            // line so users can see what a package is before deciding
            // what to do with it. The plan-reason string ("Installed
            // version is current or newer…") and the handling-summary /
            // automation-kind detail are not localized for end users —
            // both stay on PackageRow as structured fields for the saved
            // report and stay out of the wizard pane.
            let details = if description.is_empty() {
                summary.clone()
            } else {
                format!("{summary}\n\n{description}")
            };
            // The available version's What's-New notes (resolved by the
            // deferred version check for packages that declare a source)
            // follow under a localized heading, so the pane answers "what
            // changed upstream?" without a trip to the package's website.
            let details = match whats_new_by_id.get(action.package_id.as_str()) {
                Some(notes) => {
                    let heading = localizer
                        .format(
                            "wizard-package-whats-new-heading",
                            &[("package", display_name.as_str())],
                        )
                        .value;
                    format!("{details}\n\n{heading}\n{notes}")
                }
                None => details,
            };
            PackageRow {
                package_id: action.package_id.clone(),
                summary: summary.clone(),
                details,
                display_name: display_name.clone(),
                description,
                selected: initially_selected,
                installed_version,
                available_version,
                action: initial_action,
                action_label,
                original_action: action.action,
                reason: action.reason.clone(),
                handling_summary,
                manual_attention_expected,
                available_for_target: true,
                unavailability_reason: None,
                category: spec.map(|spec| spec.category).unwrap_or_default(),
                requires_standard_install: spec
                    .map(|spec| spec.requires_standard_install)
                    .unwrap_or(false),
            }
        })
        .collect()
}

/// Will this package be on disk after the current wizard run completes?
///
/// Two ways the answer is yes:
///
/// 1. The package was already on disk before the wizard opened. We read
///    that from `original_action` — the plan only emits `Install` for
///    packages that aren't installed, so anything else (`Update`, `Keep`)
///    means the package was on disk when the wizard built its rows.
/// 2. The user has the row ticked and its current action stages it to
///    disk (`Install` or `Update`). `Keep` doesn't move bytes.
///
/// Driving the configuration-row dependency check off this predicate
/// keeps gating coherent when a row's `selected` and `action` disagree —
/// e.g. a non-recommended `Install` row arrives unticked-by-default
/// (so `selected=false` but `action=Install`), and a user-unticked row
/// is flipped to `Keep` (so `selected=false` but `action=Keep`). Both
/// must read as "won't be on disk", which the simpler action-only check
/// got wrong for the latter and our new default broke for the former.
/// Is this package being written to disk *by this run* — as opposed to
/// merely being there already? Steps that change existing configuration
/// rather than adding to it auto-tick off this, not off
/// [`package_row_will_land_on_disk`].
pub(crate) fn package_row_installs_now(row: &PackageRow) -> bool {
    row.available_for_target
        && row.selected
        && matches!(row.action, PlanActionKind::Install | PlanActionKind::Update)
}

pub(crate) fn package_row_will_land_on_disk(row: &PackageRow) -> bool {
    if !row.available_for_target {
        return false;
    }
    let was_installed = !matches!(row.original_action, PlanActionKind::Install);
    let installing_now =
        matches!(row.action, PlanActionKind::Install | PlanActionKind::Update) && row.selected;
    was_installed || installing_now
}

pub(crate) fn package_handling_summary(
    text: &WizardText,
    package_id: &str,
    platform: Platform,
    architecture: Architecture,
) -> (String, bool) {
    match package_automation_support(package_id, platform, architecture) {
        PackageAutomationSupport::Direct => (text.package_handling_automatic.clone(), false),
        PackageAutomationSupport::AvailableUnattended(_) => {
            (text.package_handling_unattended.clone(), false)
        }
        PackageAutomationSupport::PlannedUnattended(_) => {
            (text.package_handling_planned.clone(), true)
        }
        PackageAutomationSupport::Unavailable => (text.package_handling_unavailable.clone(), true),
    }
}

pub(crate) fn package_display_name(model: &WizardModel, package_id: &str) -> String {
    if let Ok(localizer) = localizer_from_options(&model.bootstrap_options)
        && let Some(spec) = builtin_package_specs(model.platform)
            .into_iter()
            .find(|spec| spec.id == package_id)
    {
        return localizer.text(&spec.display_name_key).value;
    }

    builtin_package_specs(model.platform)
        .into_iter()
        .find(|spec| spec.id == package_id)
        .map(|spec| spec.display_name)
        .unwrap_or_else(|| package_id.to_string())
}
