//! Package page rows: what is offered for a target, what stays ticked, and
//! how a checkbox toggle changes a row.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rabbit_core::Result;
use rabbit_core::artifact::{dispatch_architecture, package_has_build_for};
use rabbit_core::detection::detect_components;
use rabbit_core::localization::Localizer;
use rabbit_core::model::{Architecture, LeftToReapack, Platform};
use rabbit_core::operation::{PackageAutomationSupport, package_automation_support};
use rabbit_core::osara_keymap::{OsaraKeymapState, replace_by_default};
use rabbit_core::package::{
    HostCapabilities, PACKAGE_OSARA, PackageCategory, PackageKind, PackageSpec,
    builtin_package_specs, detect_host_capabilities, host_supports_package,
};
use rabbit_core::plan::{AvailablePackage, PlanAction, PlanActionKind, build_install_plan};

use super::bootstrap::localizer_from_options;
use super::labels::{action_label, version_text};
use super::model::{
    OsaraKeymapChoice, PackageRow, TargetRow, WizardModel, WizardPackagePlan, WizardText,
};
use super::target::installation_from_target_row;

/// The Fluent key of the heading the Install and Uninstall trees show over a
/// package category's rows. A new category needs a key here and in every
/// locale, and nothing else in the trees.
pub fn package_group_label_key(category: PackageCategory) -> &'static str {
    match category {
        PackageCategory::Core => "wizard-packages-tree-group-label",
        PackageCategory::Instruments => "wizard-instruments-tree-group-label",
        PackageCategory::Tools => "wizard-tools-tree-group-label",
        PackageCategory::Language => "wizard-language-tree-group-label",
    }
}

/// One package group of the Install or Uninstall tree: its category and the
/// indices of its rows, in row order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTreeGroup {
    pub category: PackageCategory,
    pub rows: Vec<usize>,
}

/// The package groups a tree shows for rows of these categories, given in
/// row order: one per category that has rows, in [`PackageCategory::ALL`]
/// order. A category without rows gets no group, so no heading shows empty.
pub fn package_tree_groups(
    categories: impl IntoIterator<Item = PackageCategory>,
) -> Vec<PackageTreeGroup> {
    let categories: Vec<PackageCategory> = categories.into_iter().collect();
    PackageCategory::ALL
        .into_iter()
        .filter_map(|category| {
            let rows: Vec<usize> = categories
                .iter()
                .enumerate()
                .filter(|(_, row_category)| **row_category == category)
                .map(|(index, _)| index)
                .collect();
            (!rows.is_empty()).then_some(PackageTreeGroup { category, rows })
        })
        .collect()
}

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

/// The OSARA key map choice a run starts from. A new OSARA replaces the key
/// map, so it arrives with its own bindings, and so does an OSARA whose key
/// map is still OSARA's own: nothing of the user's is lost. Any other key
/// map stays. OSARA's "Merge OSARA key map" action brings the new bindings
/// into one with the user's own changes, which a replace would throw away
/// (it only keeps a backup). See [`replace_by_default`].
pub fn osara_keymap_default(
    package_rows: &[PackageRow],
    keymap: OsaraKeymapState,
) -> OsaraKeymapChoice {
    let osara_installed = package_rows
        .iter()
        .find(|row| row.package_id == PACKAGE_OSARA)
        .is_some_and(|row| row.original_action != PlanActionKind::Install);
    if replace_by_default(osara_installed, keymap) {
        OsaraKeymapChoice::ReplaceCurrent
    } else {
        OsaraKeymapChoice::PreserveCurrent
    }
}

/// The note under the key map checkbox: what the choice does, in the words
/// that fit the target's key map.
pub fn osara_keymap_note(
    model: &WizardModel,
    osara_selected: bool,
    choice: OsaraKeymapChoice,
    keymap: OsaraKeymapState,
) -> String {
    if !osara_selected {
        return model.text.packages_osara_keymap_unavailable_note.clone();
    }

    match (choice, keymap) {
        (OsaraKeymapChoice::ReplaceCurrent, OsaraKeymapState::UnchangedOsara) => {
            model.text.packages_osara_keymap_unchanged_note.clone()
        }
        (OsaraKeymapChoice::ReplaceCurrent, _) => {
            model.text.packages_osara_keymap_replace_note.clone()
        }
        (OsaraKeymapChoice::PreserveCurrent, OsaraKeymapState::DefaultKeys) => {
            model.text.packages_osara_keymap_default_keys_note.clone()
        }
        (OsaraKeymapChoice::PreserveCurrent, _) => {
            model.text.packages_osara_keymap_preserve_note.clone()
        }
    }
}

/// The key map line on the review page and in the run's details.
pub fn osara_keymap_review_line(
    model: &WizardModel,
    choice: OsaraKeymapChoice,
    keymap: OsaraKeymapState,
) -> String {
    match (choice, keymap) {
        (OsaraKeymapChoice::ReplaceCurrent, OsaraKeymapState::UnchangedOsara) => {
            model.text.review_osara_keymap_replace_unchanged.clone()
        }
        (OsaraKeymapChoice::ReplaceCurrent, _) => model.text.review_osara_keymap_replace.clone(),
        (OsaraKeymapChoice::PreserveCurrent, OsaraKeymapState::DefaultKeys) => {
            model.text.review_osara_keymap_keep_default_keys.clone()
        }
        (OsaraKeymapChoice::PreserveCurrent, _) => model.text.review_osara_keymap_preserve.clone(),
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
    let installed_channels = target
        .map(|target| rabbit_core::receipt::installed_channels_at(&target.path))
        .unwrap_or_default();
    // The target's own architecture, which the install request hands to
    // artifact resolution, rather than the one of the target the wizard
    // opened on.
    let architecture = target.map_or(model.architecture, |target| target.architecture);
    let mut package_rows = package_rows(
        &localizer,
        &model.text,
        model.platform,
        architecture,
        &package_specs,
        &plan.actions,
        available_packages,
        &host,
        &declined,
        &installed_channels,
    );

    // Some packages can't honor a portable target: they install to a fixed
    // location outside any portable REAPER folder. JAWS-for-REAPER scripts'
    // NSIS package hard-codes `%APPDATA%\REAPER\UserPlugins\`; Surge XT's
    // vendor installer writes to the system VST3 root + ProgramData /
    // /Library/Application Support. (app2clap installs into REAPER's own
    // UserPlugins\FX folder, so a portable REAPER gets it too.) Each
    // declares `requires_standard_install` in the manifest, so
    // the gate is data-driven rather than a hardcoded package-id list. Mark
    // those rows unavailable on a portable target so the checklist disables
    // their checkboxes and each row label carries a localized "(requires
    // standard installation)" indicator. A row already unavailable because
    // the package has no build for this REAPER, or because RABBIT leaves it
    // to ReaPack, keeps that reason, which its details already explain.
    if target.is_some_and(|target| target.portable) {
        for row in &mut package_rows {
            if row.requires_standard_install && row.has_build_for_target && !row.left_to_reapack {
                mark_row_unavailable(&localizer, row, "wizard-package-row-unavailable-portable");
            }
        }
    }
    // A copy that every REAPER on the computer loads, in the CLAP folder an
    // earlier RABBIT installed app2clap into: the details say what it means
    // for this REAPER, and so does Review for a ticked row.
    if let Some(target) = target {
        note_copies_in_former_outside_folders(
            &localizer,
            &mut package_rows,
            &package_specs,
            target.portable,
            &former_outside_copies,
        );
    }

    let can_install = package_rows.iter().any(|row| {
        row.available_for_target
            && matches!(row.action, PlanActionKind::Install | PlanActionKind::Update)
    });

    Ok(WizardPackagePlan {
        package_rows,
        notes: plan.notes,
        can_install,
        detections,
    })
}

/// Add a sentence to the details of each row whose package has a copy in a
/// folder it installed into before, outside every resource folder: the
/// CLAP folder, where earlier versions of RABBIT installed app2clap. Every
/// REAPER on the computer loads that copy. On a `portable` REAPER that
/// doesn't have the package yet, the sentence says that installing it there
/// too would load two copies, and that updating or reinstalling it in the
/// standard REAPER first moves the shared copy into a backup; on one that
/// has its own copy already, that it loads both, and how the shared one
/// goes. On the standard REAPER, it says that its update or reinstall moves
/// the copy, and that portable REAPERs and other programs that loaded it
/// from there then no longer find it. The sentence follows the row's
/// summary. The row also gets the sentence Review says should it be
/// ticked: on the standard REAPER that the copy moves into the backup, on a
/// portable REAPER without the package that it will load two copies.
/// A row RABBIT can't install here in any run, as the package has no build
/// for this REAPER or is left to ReaPack, gets neither: each sentence is
/// about what installing, updating or reinstalling it here does, and its
/// details already say why that can't happen.
/// `copies` gives a package's copies there; the wizard passes
/// [`former_outside_copies`].
pub(crate) fn note_copies_in_former_outside_folders(
    localizer: &Localizer,
    rows: &mut [PackageRow],
    specs: &[PackageSpec],
    portable: bool,
    copies: &dyn Fn(&PackageSpec) -> Vec<PathBuf>,
) {
    for row in rows {
        if !row.has_build_for_target || row.left_to_reapack {
            continue;
        }
        let Some(spec) = specs.iter().find(|spec| spec.id == row.package_id) else {
            continue;
        };
        if spec.former_install_destinations.is_empty() || copies(spec).is_empty() {
            continue;
        }
        let installs_here = row.original_action == PlanActionKind::Install;
        let (key, review_key) = match (portable, installs_here) {
            (true, true) => (
                "wizard-package-details-clap-copy-portable",
                Some("wizard-review-clap-copy-twice"),
            ),
            (true, false) => ("wizard-package-details-clap-copy-portable-installed", None),
            (false, _) => (
                "wizard-package-details-clap-copy-standard",
                Some("wizard-review-clap-copy-moves"),
            ),
        };
        let package = [("package", row.display_name.as_str())];
        let sentence = localizer.format(key, &package).value;
        row.former_copy_review_note =
            review_key.map(|review_key| localizer.format(review_key, &package).value);
        row.details_notes.insert(0, sentence);
        row.details = package_row_details(localizer, row);
    }
}

/// A package's copies in the folders it installed into before, outside
/// every resource folder, such as an app2clap in the CLAP folder (see
/// [`rabbit_core::detection::copies_in_former_outside_folders`]). The
/// wizard's own tests never look in the real CLAP folder: each finds the
/// copies `former_copies_seam` gives it, none unless it says otherwise.
#[cfg(not(test))]
pub(crate) fn former_outside_copies(spec: &PackageSpec) -> Vec<PathBuf> {
    rabbit_core::detection::copies_in_former_outside_folders(spec)
}

/// [`former_outside_copies`] in the wizard's tests: the copies
/// [`former_copies_seam`] gives, never the real CLAP folder's.
#[cfg(test)]
pub(crate) fn former_outside_copies(spec: &PackageSpec) -> Vec<PathBuf> {
    former_copies_seam::copies(spec)
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
    row.default_selected = false;
    row.action = PlanActionKind::Keep;
    row.action_label = action_label(localizer, PlanActionKind::Keep);
    let summary = package_row_summary(localizer, row);
    let indicator = localizer
        .format(
            "wizard-package-row-unavailable-suffix",
            &[("reason", reason.as_str())],
        )
        .value;
    row.summary = format!("{summary} {indicator}");
    row.details = package_row_details(localizer, row);
    row.unavailability_reason = Some(reason);
}

/// The short reason a row's label gives, as a Fluent key, and the sentence
/// its details add, for a package RABBIT leaves to ReaPack.
fn left_to_reapack_text(
    localizer: &Localizer,
    left: &LeftToReapack,
    package: &str,
) -> (&'static str, String) {
    match left {
        LeftToReapack::Managed { reapack_package } => (
            "wizard-package-row-unavailable-reapack",
            localizer
                .format(
                    "wizard-package-details-reapack",
                    &[
                        ("package", package),
                        ("reapack_package", reapack_package.as_str()),
                    ],
                )
                .value,
        ),
        LeftToReapack::RecordsUnreadable { error } => (
            "wizard-package-row-unavailable-reapack-check-failed",
            localizer
                .format(
                    "wizard-package-details-reapack-check-failed",
                    &[("package", package), ("error", error.as_str())],
                )
                .value,
        ),
    }
}

/// Apply per-package latest-version-check failures to a built package-row
/// set. Each failed package's row is force-unchecked and disabled (same
/// mechanism as the portable-target restriction) with a localized "version
/// check failed" reason in its label, and a note carrying the package name
/// plus the full error message is appended for the Review page. One
/// unreachable upstream (e.g. the SWS homepage being down) therefore
/// disables just that package instead of blocking the whole update flow.
///
/// A row that is already unavailable keeps its own reason, and gets no
/// note: one with no build for this REAPER, one RABBIT leaves to ReaPack,
/// and one that needs a standard installation on a portable target. RABBIT
/// wouldn't install or update that package here whatever the check found.
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
        let row = package_rows
            .iter_mut()
            .find(|row| row.package_id == *package_id);
        // A row is already unavailable when the package has no build for
        // this REAPER, when RABBIT leaves it to ReaPack, or when it needs a
        // standard installation and the target is portable. No run installs
        // or updates it here, and the row already says why: a failed
        // version check gives it no other reason, and Review no note.
        if row.as_ref().is_some_and(|row| !row.available_for_target) {
            continue;
        }
        if let Some(row) = row {
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

/// Recompute a `PackageRow`'s `action`, `action_label`, `summary`,
/// `details` and `selected` fields to match a new checkbox state. Used by
/// the wizard's CheckListBox toggle handler so the visible
/// "Install/Update/Keep" label follows what the user just clicked. Returns
/// the freshly-formatted summary so the caller can also push it into the
/// CheckListBox label.
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
    row.action = new_action;
    row.action_label = action_label(&localizer, new_action);
    row.summary = package_row_summary(&localizer, row);
    row.details = package_row_details(&localizer, row);
    row.selected = checked;
    Ok(row.summary.clone())
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

/// What the Backups tab needs to know of a package the embedded package
/// manifests describe.
struct KnownPackage {
    /// It is a REAPER language pack.
    language_pack: bool,
    /// Each variant's display-name key, by the variant's id.
    variant_keys: BTreeMap<String, String>,
}

/// The packages the embedded package manifests describe, by id, read once.
fn known_packages() -> &'static BTreeMap<String, KnownPackage> {
    static KNOWN: std::sync::OnceLock<BTreeMap<String, KnownPackage>> = std::sync::OnceLock::new();
    KNOWN.get_or_init(|| {
        rabbit_core::package::embedded_package_manifest()
            .packages
            .into_iter()
            .map(|spec| {
                let known = KnownPackage {
                    language_pack: spec.package_kind == PackageKind::LanguagePack,
                    variant_keys: spec
                        .variants
                        .into_iter()
                        .map(|variant| (variant.id, variant.display_name_key))
                        .collect(),
                };
                (spec.id, known)
            })
            .collect()
    })
}

/// Whether `package_id` is a REAPER language pack, whose file REAPER's
/// language setting names. `false` for a package RABBIT doesn't know.
pub fn is_language_pack(package_id: &str) -> bool {
    known_packages()
        .get(package_id)
        .is_some_and(|known| known.language_pack)
}

/// The localized name of `variant_id`, a variant of `package_id`, such as
/// one of the Spanish language pack's two OSARA translations, by the
/// display-name key the package manifest gives it. A variant RABBIT
/// doesn't know, or one without a translation, is named by its id.
pub fn localized_variant_display_name(
    localizer: &Localizer,
    package_id: &str,
    variant_id: &str,
) -> String {
    known_packages()
        .get(package_id)
        .and_then(|known| known.variant_keys.get(variant_id))
        .map(|key| localizer.text(key))
        .filter(|text| !text.missing)
        .map_or_else(|| variant_id.to_string(), |text| text.value)
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

/// The row tag and details sentence for a package whose build is not the
/// regular release, or which this run takes back to it. `None` for the
/// ordinary case. The sentence says what the install will do, so a row
/// that installs nothing (`acting` false) gets the tag without it.
fn package_channel_note(
    localizer: &Localizer,
    package: &str,
    available_channel: Option<&str>,
    installed_channel: Option<&str>,
    acting: bool,
) -> Option<(String, Option<String>)> {
    let text = |key: &str, args: &[(&str, &str)]| localizer.format(key, args).value;
    match available_channel.map(rabbit_core::package::split_channel) {
        Some(("pr", Some(number))) => Some((
            text("wizard-package-channel-pr", &[("number", number)]),
            acting.then(|| {
                text(
                    "wizard-package-channel-pr-details",
                    &[("package", package), ("number", number)],
                )
            }),
        )),
        Some(_) => Some((
            text("wizard-package-channel-dev", &[]),
            acting.then(|| {
                text(
                    "wizard-package-channel-dev-details",
                    &[("package", package)],
                )
            }),
        )),
        // Installed from a pre-release, and this run puts the regular
        // release back - which is exactly what leaving expert mode does.
        None if installed_channel.is_some() && acting => Some((
            text("wizard-package-channel-back", &[]),
            Some(text(
                "wizard-package-channel-back-details",
                &[("package", package)],
            )),
        )),
        None => None,
    }
}

fn row_installs(action: PlanActionKind) -> bool {
    matches!(action, PlanActionKind::Install | PlanActionKind::Update)
}

/// The Fluent keys of the reason and the details sentence for a package with
/// no build for a REAPER of `architecture`. A 32-bit REAPER is named as
/// such; any other, on Windows on ARM or a Mac, by its processor type.
fn no_build_keys(architecture: Architecture) -> (&'static str, &'static str) {
    if dispatch_architecture(architecture) == Architecture::X86 {
        (
            "wizard-package-row-unavailable-32-bit",
            "wizard-package-details-unavailable-32-bit",
        )
    } else {
        (
            "wizard-package-row-unavailable-processor",
            "wizard-package-details-unavailable-processor",
        )
    }
}

/// A package row's one-line label: package, action and versions, then a tag
/// when the build is not the regular release, or this run takes it back to
/// one. Everything that changes a row's action rebuilds the label through
/// here, so ticking or unticking a row never drops the tag.
fn package_row_summary(localizer: &Localizer, row: &PackageRow) -> String {
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
    match package_channel_note(
        localizer,
        &row.display_name,
        row.available_channel.as_deref(),
        row.installed_channel.as_deref(),
        row_installs(row.action),
    ) {
        Some((tag, _)) => {
            localizer
                .format(
                    "wizard-package-row-channel-suffix",
                    &[("row", summary.as_str()), ("channel", tag.as_str())],
                )
                .value
        }
        None => summary,
    }
}

/// The text of a package row's details pane: the row's label, its
/// `details_notes` (no build for this REAPER, left to ReaPack, a copy in
/// the CLAP folder), what a pre-release build means when this run installs
/// one, the package's description, and the available version's What's-New
/// notes. Built from the row's current action, so everything that changes
/// the action (a tick, an untick, the row becoming unavailable) rebuilds it
/// here and the pane never describes a choice the user has since undone.
fn package_row_details(localizer: &Localizer, row: &PackageRow) -> String {
    let mut parts = vec![row.summary.clone()];
    parts.extend(row.details_notes.iter().cloned());
    if let Some((_, Some(sentence))) = package_channel_note(
        localizer,
        &row.display_name,
        row.available_channel.as_deref(),
        row.installed_channel.as_deref(),
        row_installs(row.action),
    ) {
        parts.push(sentence);
    }
    if !row.description.is_empty() {
        parts.push(row.description.clone());
    }
    // The What's-New notes go under a localized heading, so the pane
    // answers "what changed upstream?" without a trip to the website.
    if let Some(notes) = &row.whats_new {
        let heading = localizer
            .format(
                "wizard-package-whats-new-heading",
                &[("package", row.display_name.as_str())],
            )
            .value;
        parts.push(format!("{heading}\n{notes}"));
    }
    parts.join("\n\n")
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
    installed_channels: &rabbit_core::package::PackageChannels,
) -> Vec<PackageRow> {
    let channel_by_id: BTreeMap<_, _> = available_packages
        .iter()
        .filter_map(|available| {
            available
                .channel
                .as_deref()
                .map(|channel| (available.package_id.as_str(), channel))
        })
        .collect();
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
    // Each package as it resolves on the channel its version was checked
    // on, which is where the run would download it from.
    let channels: rabbit_core::package::PackageChannels = channel_by_id
        .iter()
        .map(|(package_id, channel)| (package_id.to_string(), channel.to_string()))
        .collect();
    let specs_on_channels = rabbit_core::package::package_specs_by_id_on(platform, &channels);
    actions
        .iter()
        .map(|action| {
            let spec = specs_by_id.get(action.package_id.as_str()).copied();
            // A package with no build for this REAPER is listed, but can't
            // be ticked: resolving its download would stop the whole run.
            let no_build = specs_on_channels
                .get(action.package_id.as_str())
                .is_some_and(|spec| !package_has_build_for(spec, platform, architecture));
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
            //  - Update → ticked (the package is already on disk; the user
            //    opted into having it, so keep it current by default), unless
            //    it is a language pack the user turned down or removed (see
            //    below).
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
            // Has the user turned this package down on this install, or
            // uninstalled it? Either keeps an Install row unticked, for
            // every package: a package removed through RABBIT and still gone
            // must not come back ticked on the next run. An Update row
            // honours it only for a package that remembers an opt-out (the
            // language packs). Any other package gets into the set only by
            // an uninstall, and an Update row means it is back: dragged back
            // from the Trash, say, or installed by hand. So it gets its
            // updates like any package you have. The row stays listed and
            // tickable either way: a refusal suppresses the automatic tick,
            // never the package itself.
            let refused = declined.contains(&action.package_id);
            let refuses_updates = refused && spec.is_some_and(|spec| spec.remember_opt_out);
            let recommended = spec
                .map(|spec| {
                    rabbit_core::package::effective_recommended(spec, host)
                        || rabbit_core::package::matches_ui_language(
                            spec,
                            localizer.active_locale(),
                        )
                })
                .unwrap_or(false);
            let initially_selected = !no_build
                && match action.action {
                    // Updates are ticked by default because a package you
                    // have should stay current. A language pack you turned
                    // down or removed is the exception. Someone who stopped
                    // using one and turned its update down would otherwise
                    // be offered it again, ticked, every time the
                    // translators publish a fix.
                    PlanActionKind::Update => !refuses_updates,
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
            // Say in the row itself when a build is not the regular release,
            // or when this run takes a pre-release back to the regular one.
            // The plan's own reason text stays out of the wizard, and the
            // unlock dialog is long gone by the time a later run switches a
            // package back - so the row is the one place it is always heard.
            let available_channel = channel_by_id
                .get(action.package_id.as_str())
                .map(|channel| channel.to_string());
            let installed_channel = installed_channels.get(&action.package_id).cloned();
            let (handling_summary, manual_attention_expected) =
                package_handling_summary(text, &action.package_id, platform, architecture);
            // The plan-reason string ("Installed version is current or
            // newer…") and the handling-summary / automation-kind detail are
            // not localized for end users: both stay on PackageRow as
            // structured fields for the saved report and stay out of the
            // wizard's details pane.
            let mut row = PackageRow {
                package_id: action.package_id.clone(),
                summary: String::new(),
                details: String::new(),
                display_name: display_name.clone(),
                description: description.clone(),
                selected: initially_selected,
                default_selected: initially_selected,
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
                has_build_for_target: !no_build,
                category: spec.map(|spec| spec.category).unwrap_or_default(),
                requires_standard_install: spec
                    .map(|spec| spec.requires_standard_install)
                    .unwrap_or(false),
                available_channel,
                installed_channel,
                left_to_reapack: action.left_to_reapack.is_some(),
                former_copy_review_note: None,
                details_notes: Vec::new(),
                whats_new: whats_new_by_id
                    .get(action.package_id.as_str())
                    .map(|notes| notes.to_string()),
            };
            row.summary = package_row_summary(localizer, &row);
            // The row says why in its label, and its details in a sentence.
            // No build for this REAPER is the first reason a row gives:
            // nothing else, not even a copy ReaPack looks after, would let
            // RABBIT install the package here.
            if no_build {
                let (reason_key, sentence_key) = no_build_keys(architecture);
                mark_row_unavailable(localizer, &mut row, reason_key);
                row.details_notes.push(
                    localizer
                        .format(sentence_key, &[("package", display_name.as_str())])
                        .value,
                );
            }
            // A package ReaPack looks after can't be ticked: the plan keeps
            // it, and the row and its details say why. With no build for
            // this REAPER, the label keeps that reason, and the details
            // still say that ReaPack looks after the package.
            if let Some(left) = &action.left_to_reapack {
                let (reason_key, sentence) = left_to_reapack_text(localizer, left, &display_name);
                if !no_build {
                    mark_row_unavailable(localizer, &mut row, reason_key);
                }
                row.details_notes.push(sentence);
            }
            row.details = package_row_details(localizer, &row);
            row
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

/// What [`former_outside_copies`] finds in the wizard's tests: nothing,
/// unless a test installs copies of its own. Each thread has its own, so
/// tests running at once never see each other's.
#[cfg(test)]
pub(crate) mod former_copies_seam {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use rabbit_core::package::PackageSpec;

    type Copies = Box<dyn Fn(&PackageSpec) -> Vec<PathBuf>>;

    thread_local! {
        static COPIES: RefCell<Option<Copies>> = const { RefCell::new(None) };
    }

    /// Takes the copies away again when dropped.
    pub(crate) struct Installed;

    impl Drop for Installed {
        fn drop(&mut self) {
            COPIES.with(|copies| copies.borrow_mut().take());
        }
    }

    /// Have this thread's wizard find `copies` until what this returns is
    /// dropped.
    pub(crate) fn install(copies: impl Fn(&PackageSpec) -> Vec<PathBuf> + 'static) -> Installed {
        COPIES.with(|slot| *slot.borrow_mut() = Some(Box::new(copies)));
        Installed
    }

    pub(super) fn copies(spec: &PackageSpec) -> Vec<PathBuf> {
        COPIES.with(|copies| {
            copies
                .borrow()
                .as_ref()
                .map_or_else(Vec::new, |copies| copies(spec))
        })
    }
}
