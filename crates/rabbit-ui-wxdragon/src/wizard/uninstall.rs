//! The Uninstall tab: one row for each package installed in the selected
//! REAPER, saying whether RABBIT can remove it, and how ticking a row
//! changes it. Also what an uninstall run looks like before it starts: the
//! wizard's mode, the run button, Review's text, the "Also remove settings"
//! option and the question asked before a removal a backup can't undo.
//!
//! The rows come from the uninstall plan `rabbit-core` builds, the plan
//! `rabbit components` and the dry run of `rabbit uninstall` read. Building
//! it only asks which files exist and how big they are, and never reads a
//! file's contents, so the UI thread can afford it: the version check's
//! handler builds the rows from the detection it has just run.

use std::path::Path;

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

/// The Packages page's tabs, in the notebook's order.
pub const PACKAGES_INSTALL_TAB: usize = 0;
pub const PACKAGES_UNINSTALL_TAB: usize = 1;

/// What the run does. The Packages tab showing when Next is pressed decides
/// it, and it stays fixed through Review, Progress and Done: install and
/// uninstall never mix in one run, and the ticks on the other tab are kept
/// but left out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WizardMode {
    #[default]
    Install,
    Uninstall,
}

impl WizardMode {
    /// The mode for the Packages tab at `selection`, the notebook's
    /// selection. Anything but the Uninstall tab, no selection included,
    /// means install, the tab the page opens on.
    pub fn for_packages_tab(selection: i32) -> Self {
        if usize::try_from(selection) == Ok(PACKAGES_UNINSTALL_TAB) {
            Self::Uninstall
        } else {
            Self::Install
        }
    }

    /// Whether the ReaPack donation page is part of the run, on the way to
    /// Review and on the way back. Only an install or update of ReaPack
    /// goes through it. Ticks on the Install tab survive while the
    /// Uninstall tab runs, so without the mode an uninstall would land on
    /// an install-only page.
    pub fn shows_reapack_acknowledgement(self, reapack_ticked_for_install: bool) -> bool {
        self == Self::Install && reapack_ticked_for_install
    }

    /// Whether Enter starts the run on Review: the run button is the
    /// default button, and Enter in the Review text presses it. Never for
    /// an uninstall, so that it can't be two Enter presses away from most
    /// controls on the Packages page; there the disabled Next stays the
    /// default, as on Progress. The mnemonic and Tab, then Space, still work.
    pub fn enter_starts_run(self) -> bool {
        self == Self::Install
    }

    /// Whether the run button is enabled on Review. An install needs
    /// something to install or update and a Review without blockers. An
    /// uninstall has its own flag, from its own Review: the install plan's
    /// flag is false exactly when everything is installed and current, or
    /// every version check failed, and neither says anything about what can
    /// be removed.
    pub fn run_button_enabled(
        self,
        plan_can_install: bool,
        review_can_install: bool,
        review_can_uninstall: bool,
    ) -> bool {
        match self {
            Self::Install => plan_can_install && review_can_install,
            Self::Uninstall => review_can_uninstall,
        }
    }
}

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
    /// What "Also remove settings" would take as well, in a few words each,
    /// such as "your grooves". Empty when the row can't be ticked or the
    /// package has no settings RABBIT may remove.
    pub settings: Vec<String>,
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
    /// The plan the rows were built from, for what Review and the question
    /// before starting need beyond a row's text. `None` without rows.
    pub plan: Option<UninstallPlan>,
}

impl WizardUninstallList {
    /// The ticked rows, in list order.
    pub fn ticked_rows(&self) -> impl Iterator<Item = &UninstallRow> {
        self.rows.iter().filter(|row| row.selected)
    }

    pub fn ticked_package_ids(&self) -> Vec<String> {
        self.ticked_rows()
            .map(|row| row.package_id.clone())
            .collect()
    }

    /// Whether "Also remove settings" would remove anything: a ticked
    /// package has settings RABBIT may remove. The checkbox can be ticked,
    /// and reached with Tab, only then.
    pub fn settings_option_available(&self) -> bool {
        self.ticked_rows().any(|row| !row.settings.is_empty())
    }

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
            plan: None,
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
        plan: Some(plan.clone()),
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
        settings: if can_tick {
            removable_settings(localizer, item)
        } else {
            Vec::new()
        },
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
    let mut lines = always_warning_lines(localizer, item);
    if let Some(summary) = &item.summary {
        lines.push(text(&summary.removes_key));
    }
    lines.push(text("wizard-uninstall-details-files"));
    if let Some(summary) = &item.summary {
        lines.push(text(&summary.keeps_key));
    }
    lines.extend(other_warning_lines(localizer, item, name));
    lines.extend(note_lines(localizer, item, name));
    lines.join("\n")
}

/// The warnings the manifest says always to show, such as OSARA's that
/// REAPER stops speaking. They come before anything else, on the row and on
/// Review.
fn always_warning_lines(localizer: &Localizer, item: &UninstallPlanItem) -> Vec<String> {
    item.warnings
        .iter()
        .filter_map(|warning| match warning {
            UninstallWarning::Always { message_key, .. } => Some(localizer.text(message_key).value),
            _ => None,
        })
        .collect()
}

/// Every other warning, one sentence each.
fn other_warning_lines(localizer: &Localizer, item: &UninstallPlanItem, name: &str) -> Vec<String> {
    let format = |key: &str, args: &[(&str, &str)]| localizer.format(key, args).value;
    item.warnings
        .iter()
        .filter_map(|warning| match warning {
            UninstallWarning::Always { .. } => None,
            UninstallWarning::OtherPackageInstalled { message_key, .. } => {
                Some(localizer.text(message_key).value)
            }
            UninstallWarning::DependentsStay { package_ids } => {
                let packages = package_ids
                    .iter()
                    .map(|id| localized_package_display_name(localizer, id))
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format(
                    "wizard-uninstall-details-dependents-stay",
                    &[("package", name), ("packages", packages.as_str())],
                ))
            }
            UninstallWarning::MachineWide => Some(format(
                "wizard-uninstall-details-machine-wide",
                &[("package", name)],
            )),
            UninstallWarning::ReaPackStillManages { reapack_packages } => Some(format(
                "wizard-uninstall-details-reapack-owned-extensions",
                &[("packages", reapack_packages.join(", ").as_str())],
            )),
            UninstallWarning::ReaPackRecordsUnreadable { reason } => Some(format(
                "wizard-uninstall-details-reapack-records-unreadable",
                &[("reason", reason.as_str())],
            )),
        })
        .collect()
}

/// The neutral notes on how the package comes off, one sentence each.
fn note_lines(localizer: &Localizer, item: &UninstallPlanItem, name: &str) -> Vec<String> {
    let format = |key: &str, args: &[(&str, &str)]| localizer.format(key, args).value;
    item.notes
        .iter()
        .map(|note| match note {
            UninstallNote::NoReceipt => format(
                "wizard-uninstall-details-no-receipt-note",
                &[("package", name)],
            ),
            UninstallNote::OrphanedFromReaPack { .. } => format(
                "wizard-uninstall-details-reapack-orphan",
                &[("package", name)],
            ),
            UninstallNote::Manifest { note_key } => localizer.text(note_key).value,
        })
        .collect()
}

/// What "Also remove settings" would take for `item`, in the manifest's
/// own few words, each said once: several of SWS's files are all "SWS
/// settings". Only data the plan found and may remove counts; a key map,
/// REAPER's own configuration and a whole folder never do.
fn removable_settings(localizer: &Localizer, item: &UninstallPlanItem) -> Vec<String> {
    let mut settings: Vec<String> = Vec::new();
    for kept in item.kept.iter().filter(|kept| kept.removable_with_settings) {
        let description = match &kept.note_key {
            Some(key) => localizer.text(key).value,
            None => kept
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| kept.path.display().to_string()),
        };
        if !settings.contains(&description) {
            settings.push(description);
        }
    }
    settings
}

/// The read-only note under "Also remove settings": what it would remove
/// for the packages ticked now, or why it can't be ticked.
pub fn uninstall_settings_note(localizer: &Localizer, list: &WizardUninstallList) -> String {
    if list.ticked_rows().next().is_none() {
        return localizer
            .text("wizard-uninstall-settings-note-none-ticked")
            .value;
    }
    let lines: Vec<String> = list
        .ticked_rows()
        .filter(|row| !row.settings.is_empty())
        .map(|row| {
            localizer
                .format(
                    "wizard-uninstall-settings-package",
                    &[
                        ("package", row.display_name.as_str()),
                        ("settings", row.settings.join(", ").as_str()),
                    ],
                )
                .value
        })
        .collect();
    if lines.is_empty() {
        return localizer
            .text("wizard-uninstall-settings-note-none-removable")
            .value;
    }
    localizer
        .format(
            "wizard-uninstall-settings-note-removes",
            &[("settings", lines.join("\n").as_str())],
        )
        .value
}

/// What Review shows before an uninstall, and whether the Uninstall button
/// may start it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardUninstallReviewPreview {
    pub lines: Vec<String>,
    /// A target RABBIT can write to, and at least one row ticked. Only rows
    /// RABBIT can remove can be ticked, so nothing ticked is blocked.
    pub can_uninstall: bool,
}

/// Review's text for uninstalling what is ticked in `list` from `target`,
/// in plain sentences and without file lists, counts or backup paths, which
/// belong to the details and the report:
/// 1. the target folder;
/// 2. one line per package, with what goes and what stays, or, with "Also
///    remove settings" ticked, which of its settings go too;
/// 3. the warnings, those the manifests always show first, and the notes;
/// 4. "close other music programs" for a package that serves all of them;
/// 5. the ticks on the Install tab, which this run leaves out
///    (`install_tab_ticked`, their package names);
/// 6. that the packages can be installed again;
/// 7. why the Uninstall button stays disabled, if it does.
pub fn build_uninstall_review_preview(
    localizer: &Localizer,
    target: Option<&TargetRow>,
    list: &WizardUninstallList,
    remove_settings: bool,
    install_tab_ticked: &[String],
) -> WizardUninstallReviewPreview {
    let text = |key: &str| localizer.text(key).value;
    let format = |key: &str, args: &[(&str, &str)]| localizer.format(key, args).value;
    let Some(target) = target else {
        return WizardUninstallReviewPreview {
            lines: vec![text("wizard-review-no-target")],
            can_uninstall: false,
        };
    };
    let mut lines = vec![
        format(
            "wizard-review-target",
            &[("path", target.path.display().to_string().as_str())],
        ),
        String::new(),
        text("wizard-review-uninstall-packages"),
    ];
    let ticked: Vec<(&UninstallRow, Option<&UninstallPlanItem>)> = list
        .ticked_rows()
        .map(|row| {
            let item = list
                .plan
                .as_ref()
                .and_then(|plan| plan.item(&row.package_id));
            (row, item)
        })
        .collect();
    if ticked.is_empty() {
        lines.push(text("wizard-review-uninstall-no-package"));
    }
    for (row, item) in &ticked {
        let summary = item.and_then(|item| item.summary.as_ref());
        let Some(summary) = summary else {
            // Every plan that removes something has its two sentences; the
            // row's own label is the fallback if one ever hasn't.
            lines.push(row.uninstall_summary.clone());
            continue;
        };
        let stays = if remove_settings && !row.settings.is_empty() {
            format(
                "wizard-review-uninstall-settings-removed",
                &[("settings", row.settings.join(", ").as_str())],
            )
        } else {
            text(&summary.keeps_key)
        };
        lines.push(format(
            "wizard-review-uninstall-package",
            &[
                ("package", row.display_name.as_str()),
                ("removes", text(&summary.removes_key).as_str()),
                ("stays", stays.as_str()),
            ],
        ));
    }

    let planned: Vec<(&UninstallRow, &UninstallPlanItem)> = ticked
        .iter()
        .filter_map(|(row, item)| item.map(|item| (*row, item)))
        .collect();
    let mut notes: Vec<String> = Vec::new();
    let mut add_note = |note: String| {
        if !notes.contains(&note) {
            notes.push(note);
        }
    };
    for (_, item) in &planned {
        always_warning_lines(localizer, item)
            .into_iter()
            .for_each(&mut add_note);
    }
    for (row, item) in &planned {
        other_warning_lines(localizer, item, &row.display_name)
            .into_iter()
            .chain(note_lines(localizer, item, &row.display_name))
            .for_each(&mut add_note);
    }
    let machine_wide = planned
        .iter()
        .any(|(_, item)| item.warnings.contains(&UninstallWarning::MachineWide));
    if machine_wide {
        add_note(text("wizard-review-uninstall-close-programs-note"));
    }
    if let Some(note) = inactive_tab_note(localizer, WizardMode::Uninstall, install_tab_ticked) {
        add_note(note);
    }
    if !ticked.is_empty() {
        add_note(text("wizard-review-uninstall-reinstall-hint"));
    }
    if !notes.is_empty() {
        lines.push(String::new());
        lines.push(text("wizard-review-notes-heading"));
        lines.extend(notes);
    }

    // The Target page only moves on for a target RABBIT can write to, so
    // this is a last line of defence, said plainly rather than left to a
    // failure halfway through.
    if !target.writable {
        lines.push(String::new());
        lines.push(format(
            "wizard-review-uninstall-blocked",
            &[(
                "reason",
                text("wizard-review-uninstall-not-writable").as_str(),
            )],
        ));
    }

    WizardUninstallReviewPreview {
        lines,
        can_uninstall: target.writable && !ticked.is_empty(),
    }
}

/// The note Review adds about the tab that isn't running: what is ticked
/// there stays ticked but isn't part of this run. A tab can change behind
/// the user's back, by Page Down on a checkbox on Windows, so Review says
/// so. `other_tab_ticked` are the package names ticked there; `None` when
/// there are none.
pub fn inactive_tab_note(
    localizer: &Localizer,
    mode: WizardMode,
    other_tab_ticked: &[String],
) -> Option<String> {
    if other_tab_ticked.is_empty() {
        return None;
    }
    let key = match mode {
        WizardMode::Install => "wizard-review-inactive-tab-note-uninstall",
        WizardMode::Uninstall => "wizard-review-inactive-tab-note-install",
    };
    Some(
        localizer
            .format(key, &[("packages", other_tab_ticked.join(", ").as_str())])
            .value,
    )
}

/// Why RABBIT asks once more before an uninstall starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UninstallConfirmReason {
    /// A backup can't undo the package's removal: a vendor uninstaller, a
    /// removal that needs administrator rights, or a whole folder going.
    Unbacked { package_id: String },
    /// The manifest's `always_warn` entry asks for the question, such as
    /// OSARA's warning that REAPER stops speaking.
    Warning {
        package_id: String,
        message_key: String,
    },
}

/// The reasons to ask before uninstalling `package_ids`, in manifest order.
/// Empty for a run of file removals that all go into a backup and that no
/// manifest asks about, which is every package RABBIT can remove so far:
/// the question is for the packages later versions add, and they only add
/// data for it.
pub fn uninstall_confirm_reasons(
    plan: &UninstallPlan,
    package_ids: &[String],
) -> Vec<UninstallConfirmReason> {
    let mut reasons = Vec::new();
    for item in plan
        .items
        .iter()
        .filter(|item| package_ids.contains(&item.package_id))
    {
        for warning in &item.warnings {
            if let UninstallWarning::Always {
                message_key,
                confirm: true,
            } = warning
            {
                reasons.push(UninstallConfirmReason::Warning {
                    package_id: item.package_id.clone(),
                    message_key: message_key.clone(),
                });
            }
        }
        if item.unbacked {
            reasons.push(UninstallConfirmReason::Unbacked {
                package_id: item.package_id.clone(),
            });
        }
    }
    reasons
}

/// The text of the Yes/No question asked before uninstalling `package_ids`:
/// one plain sentence per reason, then "Uninstall now?". `None` when the
/// run needs no question. The plan's own `confirm` flag counts too, so a
/// reason the wizard has no sentence for yet still asks.
pub fn uninstall_confirmation_body(
    localizer: &Localizer,
    plan: &UninstallPlan,
    package_ids: &[String],
) -> Option<String> {
    let reasons = uninstall_confirm_reasons(plan, package_ids);
    let flagged = plan
        .items
        .iter()
        .any(|item| item.confirm && package_ids.contains(&item.package_id));
    if reasons.is_empty() && !flagged {
        return None;
    }
    let mut lines: Vec<String> = Vec::new();
    for reason in &reasons {
        let line = match reason {
            UninstallConfirmReason::Unbacked { package_id } => {
                let name = localized_package_display_name(localizer, package_id);
                localizer
                    .format(
                        "wizard-confirm-uninstall-unbacked",
                        &[("package", name.as_str())],
                    )
                    .value
            }
            UninstallConfirmReason::Warning { message_key, .. } => {
                localizer.text(message_key).value
            }
        };
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines.push(localizer.text("wizard-confirm-uninstall-question").value);
    Some(lines.join("\n\n"))
}

/// The command line the "can't uninstall from the wizard yet" message
/// suggests: the REAPER folder the wizard is on and the packages ticked,
/// with `--remove-settings` when that was ticked too, so it does what the
/// wizard was asked to. `--resource-path` and `--package` are required, so
/// a command without them would fail as typed.
pub fn uninstall_cli_example(
    resource_path: &Path,
    package_ids: &[String],
    remove_settings: bool,
) -> String {
    let mut command = format!(
        "rabbit uninstall --resource-path \"{}\"",
        resource_path.display()
    );
    if package_ids.is_empty() {
        command.push_str(" --package sws");
    }
    for package_id in package_ids {
        command.push_str(" --package ");
        command.push_str(package_id);
    }
    if remove_settings {
        command.push_str(" --remove-settings");
    }
    command
}

/// The message's text, with the example command for `resource_path` and
/// the ticked packages.
pub fn uninstall_not_ready_body(
    localizer: &Localizer,
    resource_path: &Path,
    package_ids: &[String],
    remove_settings: bool,
) -> String {
    let command = uninstall_cli_example(resource_path, package_ids, remove_settings);
    localizer
        .format(
            "wizard-uninstall-not-ready-body",
            &[("command", command.as_str())],
        )
        .value
}
