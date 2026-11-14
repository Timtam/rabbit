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
use rabbit_core::package::uninstall::UninstallBackup;
use rabbit_core::package::{PackageCategory, PackageSpec, builtin_package_specs};
use rabbit_core::receipt::load_install_state;
use rabbit_core::uninstall::{
    ByHandReason, ObjectProbe, Removability, RemovalRoute, UNINSTALL_NOT_YET_KEY, UninstallNote,
    UninstallPlan, UninstallPlanInput, UninstallPlanItem, UninstallRoots, UninstallWarning,
    VendorLookup, VendorRefusal, build_uninstall_plan,
};

use super::backups::{BackupChoice, and_list};
use super::bootstrap::localizer_from_options;
use super::labels::version_text;
use super::model::{ConfigurationRow, PackageRow, TargetRow, WizardModel, WizardText};
use super::packages::localized_package_display_name;

/// The Packages page's tabs, in the notebook's order.
pub const PACKAGES_INSTALL_TAB: usize = 0;
pub const PACKAGES_UNINSTALL_TAB: usize = 1;
pub const PACKAGES_BACKUPS_TAB: usize = 2;

/// One of the Packages page's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagesTab {
    Install,
    Uninstall,
    Backups,
}

impl PackagesTab {
    /// The tabs, in the notebook's order.
    pub const ALL: [PackagesTab; 3] = [Self::Install, Self::Uninstall, Self::Backups];

    /// The tab at `selection`, the notebook's selection. `None` for no
    /// selection, and for an index past the last tab.
    pub fn from_selection(selection: i32) -> Option<Self> {
        match usize::try_from(selection).ok()? {
            PACKAGES_INSTALL_TAB => Some(Self::Install),
            PACKAGES_UNINSTALL_TAB => Some(Self::Uninstall),
            PACKAGES_BACKUPS_TAB => Some(Self::Backups),
            _ => None,
        }
    }
}

/// Whether a restore can start from Review. The Backups tab and its Review
/// are there, but the restore run comes in a later step of the backup plan
/// (2d). Until then Review shows what a restore would do, and the Restore
/// button stays disabled, blockers or not.
pub const RESTORE_RUN_AVAILABLE: bool = false;

/// Whether a delete of backups can start from Review: the same, for the
/// delete run, which comes in step 2c of the backup plan.
pub const DELETE_BACKUPS_RUN_AVAILABLE: bool = false;

/// What the run does. The Packages tab showing when Next is pressed decides
/// it, and on the Backups tab the choice below its list, Restore or Delete.
/// It stays fixed through Review, Progress and Done: runs never mix, and the
/// ticks on the other tabs are kept but left out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WizardMode {
    #[default]
    Install,
    Uninstall,
    /// Restore the backups ticked to restore on the Backups tab.
    Restore,
    /// Delete the backups ticked to delete on the Backups tab.
    DeleteBackups,
}

impl WizardMode {
    /// The mode for the Packages tab at `selection`, the notebook's
    /// selection, with `backup_choice` chosen below the Backups list. The
    /// Uninstall tab means uninstall, the Backups tab restore or delete, as
    /// chosen. Anything else, no selection included, means install, the tab
    /// the page opens on.
    pub fn for_packages_tab(selection: i32, backup_choice: BackupChoice) -> Self {
        match PackagesTab::from_selection(selection) {
            Some(PackagesTab::Uninstall) => Self::Uninstall,
            Some(PackagesTab::Backups) => match backup_choice {
                BackupChoice::Restore => Self::Restore,
                BackupChoice::Delete => Self::DeleteBackups,
            },
            Some(PackagesTab::Install) | None => Self::Install,
        }
    }

    /// The tab whose ticks the run takes.
    pub fn packages_tab(self) -> PackagesTab {
        match self {
            Self::Install => PackagesTab::Install,
            Self::Uninstall => PackagesTab::Uninstall,
            Self::Restore | Self::DeleteBackups => PackagesTab::Backups,
        }
    }

    /// Whether the ReaPack donation page is part of the run, on the way to
    /// Review and on the way back. Only an install or update of ReaPack
    /// goes through it. Ticks on the Install tab survive while another tab
    /// runs, so without the mode an uninstall, a restore or a delete would
    /// land on an install-only page.
    pub fn shows_reapack_acknowledgement(self, reapack_ticked_for_install: bool) -> bool {
        match self {
            Self::Install => reapack_ticked_for_install,
            Self::Uninstall | Self::Restore | Self::DeleteBackups => false,
        }
    }

    /// Whether Enter starts the run on Review: the run button is the
    /// default button, and Enter in the Review text presses it. Only for an
    /// install. Never for an uninstall, a restore or a delete, so that none
    /// of them can be two Enter presses away from most controls on the
    /// Packages page; there the disabled Next stays the default, as on
    /// Progress. The mnemonic and Tab, then Space, still work.
    pub fn enter_starts_run(self) -> bool {
        match self {
            Self::Install => true,
            Self::Uninstall | Self::Restore | Self::DeleteBackups => false,
        }
    }

    /// Whether the run button is enabled on Review. An install needs
    /// something to install or update and a Review without blockers. An
    /// uninstall has its own flag, from its own Review: the install plan's
    /// flag is false exactly when everything is installed and current, or
    /// every version check failed, and neither says anything about what can
    /// be removed. A restore and a delete share one flag, set by whichever
    /// of their Reviews was built last, and start only once their run is
    /// available (see [`RESTORE_RUN_AVAILABLE`] and
    /// [`DELETE_BACKUPS_RUN_AVAILABLE`]).
    pub fn run_button_enabled(
        self,
        plan_can_install: bool,
        review_can_install: bool,
        review_can_uninstall: bool,
        review_can_run_backups: bool,
    ) -> bool {
        match self {
            Self::Install => plan_can_install && review_can_install,
            Self::Uninstall => review_can_uninstall,
            Self::Restore => RESTORE_RUN_AVAILABLE && review_can_run_backups,
            Self::DeleteBackups => DELETE_BACKUPS_RUN_AVAILABLE && review_can_run_backups,
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

    /// The label and the details text that stand in for an empty list: the
    /// Windows list shows the label as a row without a checkbox, macOS says
    /// it (see `tab_announcement`), and both show the details text. `None`
    /// while there are rows.
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

    /// What macOS says when the Uninstall tab is switched to: the tab's
    /// name, then, for an empty list, the placeholder's sentence. The macOS
    /// list has no placeholder row, since any row there either has a
    /// checkbox or reads as a collapsed group, so the sentence the Windows
    /// list reads as its only row is said here instead.
    pub fn tab_announcement(&self, text: &WizardText) -> String {
        match self.placeholder(text) {
            Some((label, _)) => format!("{}. {label}", text.packages_tab_uninstall),
            None => text.packages_tab_uninstall.clone(),
        }
    }
}

/// The uninstall plan for `target`, built from `detections` and the target's
/// receipt the way `rabbit uninstall` builds it. The target's kind and its
/// REAPER come from the target row rather than from guessing by the folder,
/// so the rows and a later run from the wizard agree about them. Reads which
/// files exist and how big they are, looks vendors' uninstallers up through
/// `vendor_lookup` (HKLM in RABBIT, a stand-in in tests), and asks `objects`
/// who may move a macOS app (macOS itself in RABBIT, a stand-in in tests);
/// hashes nothing and changes nothing. RABBIT passes
/// `UninstallRoots::for_target` as `roots`; tests pass folders of their own,
/// so they never plan on what is really installed here.
pub(crate) fn uninstall_plan_with_roots(
    model: &WizardModel,
    target: &TargetRow,
    roots: &UninstallRoots,
    detections: &[ComponentDetection],
    vendor_lookup: &dyn VendorLookup,
    objects: &dyn ObjectProbe,
) -> Result<UninstallPlan> {
    let state = load_install_state(&target.path)?;
    Ok(build_uninstall_plan(&UninstallPlanInput {
        roots,
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
        vendor_lookup,
        objects,
    }))
}

/// The Uninstall tab's rows for `target`. A receipt that can't be read
/// leaves the list empty with the reason, rather than guessing which files
/// are RABBIT's.
pub fn uninstall_list_for_target(
    model: &WizardModel,
    target: &TargetRow,
    detections: &[ComponentDetection],
    vendor_lookup: &dyn VendorLookup,
    objects: &dyn ObjectProbe,
) -> WizardUninstallList {
    let roots =
        UninstallRoots::for_target(model.platform, &target.path, target.app_path.as_deref());
    uninstall_list_with_roots(model, target, &roots, detections, vendor_lookup, objects)
}

/// [`uninstall_list_for_target`] with the folders the plans name taken
/// from `roots` instead of this computer. See
/// [`uninstall_plan_with_roots`] for how the plan is built.
pub(crate) fn uninstall_list_with_roots(
    model: &WizardModel,
    target: &TargetRow,
    roots: &UninstallRoots,
    detections: &[ComponentDetection],
    vendor_lookup: &dyn VendorLookup,
    objects: &dyn ObjectProbe,
) -> WizardUninstallList {
    let Ok(localizer) = localizer_from_options(&model.bootstrap_options) else {
        return WizardUninstallList::default();
    };
    match uninstall_plan_with_roots(model, target, roots, detections, vendor_lookup, objects) {
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
    // The engine refuses a removal a backup can't undo unless its route
    // carries it out (a vendor's uninstaller through the elevated helper,
    // the Trash, or the macOS administrator command), so a row for one
    // would only lead to a refusal later on.
    let can_tick = item.can_remove();
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

/// A row's details text: its label, then a blank line and the explanation,
/// when it has one. The Backups tab's rows use it too.
pub(super) fn details_text(summary: &str, explanation: &str) -> String {
    if explanation.is_empty() {
        summary.to_string()
    } else {
        format!("{summary}\n\n{explanation}")
    }
}

/// The short phrase a row that can't be ticked ends with.
fn unavailability_reason(localizer: &Localizer, item: &UninstallPlanItem) -> String {
    let key = match &item.removability {
        // The details give the steps to remove it by hand: a manual plan's,
        // or those of a package whose uninstaller folder and registry entry
        // RABBIT leaves alone.
        Removability::Manual { .. }
        | Removability::CleanupNotTrusted { .. }
        | Removability::NoFilesForThisAccount { .. } => "wizard-uninstall-row-unavailable-manual",
        // A macOS app RABBIT can't move to the Trash is the user's to move
        // there in Finder, which the row says as plainly.
        Removability::ByHand { .. } => "wizard-uninstall-row-unavailable-by-hand-macos",
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
        Removability::NoTrustedUninstaller { .. } => {
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
        // Then the manifest's steps to remove the package by hand, which a
        // standard plan with a machine cleanup lends a portable target.
        Removability::NeedsStandardTarget => {
            let mut lines = vec![format(
                "wizard-uninstall-details-portable",
                &[("package", name)],
            )];
            lines.extend(item.notes.iter().filter_map(|note| match note {
                UninstallNote::Manifest { note_key } => Some(localizer.text(note_key).value),
                _ => None,
            }));
            lines.join("\n")
        }
        Removability::NoReceipt => {
            format("wizard-uninstall-details-no-receipt", &[("package", name)])
        }
        // What was wrong, in the user's language, and how to remove the
        // package without RABBIT.
        Removability::NoTrustedUninstaller { refusal } => [
            format(
                "wizard-uninstall-details-no-trusted-uninstaller",
                &[("package", name)],
            ),
            refusal_text(localizer, refusal, name),
            format(
                "wizard-uninstall-details-vendor-by-hand",
                &[("package", name)],
            ),
        ]
        .join("\n"),
        // What was wrong with the uninstaller folder or registry entry, in
        // the user's language, then the manifest's steps to remove the
        // package by hand.
        Removability::CleanupNotTrusted {
            refusal,
            by_hand_key,
        } => [
            format(
                "wizard-uninstall-details-cleanup-refused",
                &[("package", name)],
            ),
            refusal_text(localizer, refusal, name),
            localizer.text(by_hand_key).value,
        ]
        .join("\n"),
        Removability::NoFilesForThisAccount { by_hand_key } => [
            format(
                "wizard-uninstall-details-no-files-for-this-account",
                &[("package", name)],
            ),
            localizer.text(by_hand_key).value,
        ]
        .join("\n"),
        Removability::UnsafePath { path, .. } => format(
            "wizard-uninstall-details-unsafe-path",
            &[
                ("package", name),
                ("path", path.display().to_string().as_str()),
            ],
        ),
        Removability::ByHand { path, reason } => {
            by_hand_lines(localizer, path, reason, name).join("\n")
        }
        Removability::Removable
        | Removability::RemovableOrphanedFromReaPack
        | Removability::NotInstalled => localizer.text(UNINSTALL_NOT_YET_KEY).value,
    }
}

/// Why RABBIT can't move the macOS app at `path` to the Trash, by itself or
/// with an administrator's password, then how the user moves it there in
/// Finder: one sentence for the reason, and the steps it calls for. A
/// Locked app has to be unlocked first; an app that isn't the one RABBIT
/// expects is only the user's to remove if it is the package after all.
/// The row's details and the Done page both say it.
pub fn by_hand_lines(
    localizer: &Localizer,
    path: &Path,
    reason: &ByHandReason,
    name: &str,
) -> Vec<String> {
    let path = path.display().to_string();
    let arguments = [("package", name), ("path", path.as_str())];
    let format = |key: &str, args: &[(&str, &str)]| localizer.format(key, args).value;
    let why = match reason {
        ByHandReason::NotMovableByYou => format(
            "wizard-uninstall-details-by-hand-not-movable-macos",
            &arguments,
        ),
        ByHandReason::Locked => format("wizard-uninstall-details-by-hand-locked-macos", &arguments),
        ByHandReason::NotTheExpectedApp => format(
            "wizard-uninstall-details-by-hand-not-the-app-macos",
            &arguments,
        ),
        ByHandReason::CouldNotCheck { error } => format(
            "wizard-uninstall-details-by-hand-unchecked-macos",
            &[
                ("package", name),
                ("path", path.as_str()),
                ("error", error.as_str()),
            ],
        ),
        ByHandReason::NoTrashFolder => format(
            "wizard-uninstall-details-by-hand-no-trash-folder-macos",
            &arguments,
        ),
        ByHandReason::NotOnTrashVolume => format(
            "wizard-uninstall-details-by-hand-other-disk-macos",
            &arguments,
        ),
    };
    let steps = match reason {
        ByHandReason::Locked => "wizard-uninstall-details-by-hand-unlock-macos",
        ByHandReason::NotTheExpectedApp => {
            "wizard-uninstall-details-by-hand-not-the-app-finder-macos"
        }
        ByHandReason::NotMovableByYou
        | ByHandReason::CouldNotCheck { .. }
        | ByHandReason::NoTrashFolder
        | ByHandReason::NotOnTrashVolume => "wizard-uninstall-details-by-hand-finder-macos",
    };
    vec![why, format(steps, &[("package", name)])]
}

/// Why RABBIT doesn't trust a vendor's uninstaller, in the user's language.
pub fn refusal_text(localizer: &Localizer, refusal: &VendorRefusal, name: &str) -> String {
    let arguments = refusal.fluent_arguments(name);
    let borrowed: Vec<(&str, &str)> = arguments
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    localizer.format(refusal.fluent_key(), &borrowed).value
}

/// What uninstalling the package does, one short sentence per line, in the
/// order the plan gives: warnings that always apply first, then what goes,
/// how it goes, what stays, the other warnings and the notes. No file lists
/// or counts; those belong to the report.
///
/// A planned machine cleanup adds its own line on how the package goes:
/// only an administrator may remove the uninstaller folder and registry
/// entry, so Windows asks, and no backup brings them back. It follows the
/// backup line, or stands in for it when none of the package's files are
/// left to move.
fn removal_explanation(localizer: &Localizer, item: &UninstallPlanItem, name: &str) -> String {
    let text = |key: &str| localizer.text(key).value;
    let mut lines = always_warning_lines(localizer, item);
    if let Some(summary) = &item.summary {
        lines.push(text(&summary.removes_key));
    }
    if item.machine_cleanup.is_none() || !item.files.is_empty() {
        lines.push(text(route_details_key(item)));
    }
    if item.machine_cleanup.is_some() {
        lines.push(
            localizer
                .format(
                    "wizard-uninstall-details-machine-cleanup",
                    &[("package", name)],
                )
                .value,
        );
    }
    if let Some(summary) = &item.summary {
        lines.push(text(&summary.keeps_key));
    }
    lines.extend(other_warning_lines(localizer, item, name, &[]));
    lines.extend(note_lines(localizer, item, name));
    lines.join("\n")
}

/// The sentence on how the package leaves, by its route: files go into a
/// backup; a vendor's uninstaller works with administrator rights, after
/// Windows asked; on macOS the administrator command deletes it for good
/// after one password window, or moves an app the user can't move into the
/// user's Trash; an app the user can move goes to the Trash with no
/// password at all, which the summary before it already says, so its line
/// only says how to get it back. A row that can be ticked always has one
/// of these, or the machine cleanup's line instead.
fn route_details_key(item: &UninstallPlanItem) -> &'static str {
    match item.route() {
        RemovalRoute::BackupSet => "wizard-uninstall-details-files",
        RemovalRoute::Vendor => "wizard-uninstall-details-vendor",
        RemovalRoute::Administrator if item.backup == UninstallBackup::ToTrash => {
            "wizard-uninstall-details-trash-elevated-macos"
        }
        RemovalRoute::Administrator => "wizard-uninstall-details-elevated-macos",
        RemovalRoute::Trash => "wizard-uninstall-details-trash",
    }
}

/// The warnings the manifest says always to show, whatever else is
/// installed. They come before anything else, on the row and on Review.
fn always_warning_lines(localizer: &Localizer, item: &UninstallPlanItem) -> Vec<String> {
    item.warnings
        .iter()
        .filter_map(|warning| match warning {
            UninstallWarning::Always { message_key, .. } => Some(localizer.text(message_key).value),
            _ => None,
        })
        .collect()
}

/// Every other warning, one sentence each. `also_removed` are the packages
/// the same run removes: Review leaves them out of the extensions that
/// stay, and drops that line once none is left. A row's own details are
/// about the package alone and pass none.
fn other_warning_lines(
    localizer: &Localizer,
    item: &UninstallPlanItem,
    name: &str,
    also_removed: &[String],
) -> Vec<String> {
    let format = |key: &str, args: &[(&str, &str)]| localizer.format(key, args).value;
    item.warnings
        .iter()
        .filter_map(|warning| match warning {
            UninstallWarning::Always { .. } => None,
            UninstallWarning::OtherPackageInstalled { message_key, .. } => {
                Some(localizer.text(message_key).value)
            }
            UninstallWarning::DependentsStay { package_ids } => {
                let staying: Vec<&String> = package_ids
                    .iter()
                    .filter(|id| !also_removed.contains(id))
                    .collect();
                if staying.is_empty() {
                    return None;
                }
                let packages = staying
                    .into_iter()
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
            UninstallNote::NoVendorRecord => format(
                "wizard-uninstall-details-no-vendor-record",
                &[("package", name)],
            ),
            UninstallNote::Manifest { note_key }
            | UninstallNote::VendorFilesByHand { note_key } => localizer.text(note_key).value,
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

/// What VoiceOver is told when a tick in the Uninstall list makes "Also
/// remove settings" usable or unusable. That happens below the list, away
/// from the row the user is on, so nothing would say it otherwise. `None`
/// while the option stays as it was: its note changing with every tick
/// would only talk over the row.
pub fn uninstall_settings_availability_note(
    localizer: &Localizer,
    was_available: bool,
    list: &WizardUninstallList,
) -> Option<String> {
    let available = list.settings_option_available();
    if available == was_available {
        return None;
    }
    let key = if available {
        "wizard-uninstall-settings-available"
    } else {
        "wizard-uninstall-settings-unavailable"
    };
    let option = localizer.text("wizard-uninstall-remove-settings").value;
    Some(localizer.format(key, &[("option", option.as_str())]).value)
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
///    the extensions that stay leave out those ticked along with them;
/// 4. "close other music programs" for a package that serves all of them,
///    or whose own uninstaller or the macOS administrator command removes
///    machine-wide parts;
/// 5. the permission note: Windows asks once, before anything is removed,
///    a window from a vendor's uninstaller is to be answered, and a backup
///    can't bring back the uninstaller folder and registry entry the
///    elevated helper removes, or, with none of the package's files left to
///    move, the package itself, as the question before starting says; or
///    macOS asks once for an administrator's name and password, in a window
///    that may name osascript, and cancelling it uninstalls nothing;
/// 6. for a package going to the Trash, that it can come back from there
///    until the Trash is emptied;
/// 7. `other_tab_notes`: what is ticked on the other tabs, which this run
///    leaves out (see [`inactive_tab_notes`]);
/// 8. that the packages can be installed again;
/// 9. why the Uninstall button stays disabled, if it does.
pub fn build_uninstall_review_preview(
    localizer: &Localizer,
    target: Option<&TargetRow>,
    list: &WizardUninstallList,
    remove_settings: bool,
    other_tab_notes: &[String],
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
    // An extension ticked along with REAPER doesn't stay, so Review only
    // names the ones that do.
    let ticked_ids = list.ticked_package_ids();
    for (row, item) in &planned {
        other_warning_lines(localizer, item, &row.display_name, &ticked_ids)
            .into_iter()
            .chain(note_lines(localizer, item, &row.display_name))
            .for_each(&mut add_note);
    }
    // REAPER's own uninstaller removes ReaRoute and the file associations
    // every REAPER and other programs use, so it asks this too, as does
    // whatever the macOS administrator command deletes for good: plug-ins
    // other programs may have loaded. An app it moves to the Trash serves
    // nothing else, and the check before the run names the REAPERs to close.
    let machine_wide = planned.iter().any(|(_, item)| {
        item.warnings.contains(&UninstallWarning::MachineWide)
            || item.runs_vendor_uninstaller()
            || (item.needs_administrator() && item.backup != UninstallBackup::ToTrash)
    });
    if machine_wide {
        add_note(text("wizard-review-uninstall-close-programs-note"));
    }
    // One permission prompt covers every package the elevated helper works
    // on. Only a vendor's own uninstaller opens a window of its own; the
    // helper's cleanup of an uninstaller folder and registry entry doesn't.
    if planned.iter().any(|(_, item)| item.needs_elevated_helper()) {
        add_note(text("wizard-review-uninstall-elevation-note-windows"));
    }
    for (row, item) in &planned {
        if item.runs_vendor_uninstaller() {
            add_note(format(
                "wizard-review-uninstall-vendor-window-note",
                &[("package", row.display_name.as_str())],
            ));
        }
        // The package's own line leaves out the uninstaller folder and
        // registry entry the helper removes, so Review says what the
        // question will: what of the package a backup can't bring back.
        if item.machine_cleanup.is_some()
            && let Some(reason) = beyond_backup_reason(item)
        {
            add_note(confirm_reason_line(localizer, &reason));
        }
    }
    // macOS's one password window covers every package the administrator
    // command removes, so it is named once, with all of them. Its title
    // names osascript rather than RABBIT, so the note says why.
    let administrator: Vec<&str> = planned
        .iter()
        .filter(|(_, item)| item.needs_administrator())
        .map(|(row, _)| row.display_name.as_str())
        .collect();
    if !administrator.is_empty() {
        add_note(format(
            "wizard-review-uninstall-elevation-note-macos",
            &[("packages", administrator.join(", ").as_str())],
        ));
    }
    // What goes to the Trash can come back from there, and Review says so:
    // the summary's "Moves the app to the Trash" doesn't say how. An app the
    // administrator command moves there keeps its owner, which changes what
    // Finder offers for it.
    for (row, item) in &planned {
        let key = match item.route() {
            RemovalRoute::Trash => "wizard-review-uninstall-trash-note",
            RemovalRoute::Administrator if item.backup == UninstallBackup::ToTrash => {
                "wizard-review-uninstall-trash-elevated-note-macos"
            }
            RemovalRoute::Administrator | RemovalRoute::BackupSet | RemovalRoute::Vendor => {
                continue;
            }
        };
        add_note(format(key, &[("package", row.display_name.as_str())]));
    }
    other_tab_notes.iter().cloned().for_each(&mut add_note);
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

/// What is ticked on each Packages tab, as Review's notes on the tabs a run
/// leaves out name it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackagesTabTicks {
    /// The Install or update tab's ticks the user made, packages first,
    /// then settings: see [`install_tab_ticks_left_out`].
    pub install: Vec<String>,
    /// The Uninstall tab's ticked packages, by their names.
    pub uninstall: Vec<String>,
    /// How many backups are ticked on the Backups tab to restore, and how
    /// many to delete. Each choice keeps its own ticks, so both can count.
    pub backups_to_restore: usize,
    pub backups_to_delete: usize,
}

/// The notes Review adds about the tabs that aren't running, in tab order:
/// what is ticked there stays ticked but isn't part of this run. A tab can
/// change behind the user's back, by Page Down on a checkbox on Windows, so
/// Review says so. One note per tab with ticks: the Install or update tab's
/// and the Uninstall tab's name what is ticked, and the Backups tab's says
/// how many backups are ticked to restore and to delete. The tab of `mode`
/// gets none; the Backups tab's Reviews say themselves what is ticked for
/// the other choice there.
pub fn inactive_tab_notes(
    localizer: &Localizer,
    mode: WizardMode,
    ticks: &PackagesTabTicks,
) -> Vec<String> {
    let names = |key: &str, names: &[String]| {
        (!names.is_empty()).then(|| {
            localizer
                .format(key, &[("packages", names.join(", ").as_str())])
                .value
        })
    };
    let mut notes = Vec::new();
    for tab in PackagesTab::ALL {
        if tab == mode.packages_tab() {
            continue;
        }
        let note = match tab {
            PackagesTab::Install => {
                names("wizard-review-inactive-tab-note-install", &ticks.install)
            }
            PackagesTab::Uninstall => names(
                "wizard-review-inactive-tab-note-uninstall",
                &ticks.uninstall,
            ),
            PackagesTab::Backups => backups_tab_note(localizer, ticks),
        };
        notes.extend(note);
    }
    notes
}

/// The note on what is ticked on the Backups tab, such as "What is ticked on
/// the Backups tab isn't part of this run: 2 backups to restore and one
/// backup to delete.", or `None` with nothing ticked there.
fn backups_tab_note(localizer: &Localizer, ticks: &PackagesTabTicks) -> Option<String> {
    let counted = |key: &str, count: usize| {
        (count > 0).then(|| localizer.format_with(key, &[("count", count.into())]).value)
    };
    let parts: Vec<String> = [
        counted(
            "wizard-review-inactive-tab-backups-restore",
            ticks.backups_to_restore,
        ),
        counted(
            "wizard-review-inactive-tab-backups-delete",
            ticks.backups_to_delete,
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        return None;
    }
    let backups = and_list(localizer, &parts);
    Some(
        localizer
            .format(
                "wizard-review-inactive-tab-note-backups",
                &[("backups", backups.as_str())],
            )
            .value,
    )
}

/// What Review in any other mode names as left out of the Install or
/// update tab: the ticks there that RABBIT didn't set by itself, packages
/// first, then settings, which the run leaves out just the same.
///
/// RABBIT ticks every update, the recommended packages and the settings
/// that go with them before anyone sees the tab. Naming those told someone
/// who never opened the tab that their ticks were left out, and named the
/// very package Review was about to remove whenever it had an update. A tick
/// the user did make, before a Page Down took them to the other tab, say,
/// is what the note is for.
pub fn install_tab_ticks_left_out(
    package_rows: &[PackageRow],
    configuration_rows: &[ConfigurationRow],
) -> Vec<String> {
    let packages = package_rows
        .iter()
        .filter(|row| row.selected && !row.default_selected)
        .map(|row| row.display_name.clone());
    // The same test as `selected_configuration_step_ids`: a step that can't
    // run or is already in place isn't part of any run, ticked or not.
    let settings = configuration_rows
        .iter()
        .filter(|row| {
            row.selected
                && !row.default_selected
                && row.available_for_target
                && !row.already_applied
        })
        .map(|row| row.display_name.clone());
    packages.chain(settings).collect()
}

/// Take the Install or update tab's ticks as RABBIT's defaults, once the
/// version check has settled them and before the tab shows. The package
/// rows are new by then, but the configuration rows were built at start-up
/// and the version check ticks the recommended ones it makes available, so
/// their defaults are only known now.
pub fn remember_default_ticks(
    package_rows: &mut [PackageRow],
    configuration_rows: &mut [ConfigurationRow],
) {
    for row in package_rows.iter_mut() {
        row.default_selected = row.selected;
    }
    for row in configuration_rows.iter_mut() {
        row.default_selected = row.selected;
    }
}

/// Why RABBIT asks once more before an uninstall starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UninstallConfirmReason {
    /// A backup can't undo the package's removal: a vendor uninstaller, the
    /// macOS administrator command deleting it for good, or the elevated
    /// helper removing the uninstaller folder and registry entry of a
    /// package with none of its files left to move.
    Unbacked { package_id: String },
    /// The package's files go into a backup, but the elevated helper then
    /// removes its uninstaller folder and registry entry with administrator
    /// rights, which no backup brings back: the JAWS scripts while their
    /// installer's folder in Program Files or its entry is there.
    PartlyUnbacked { package_id: String },
    /// The package's app goes to the Trash on macOS, not into a backup:
    /// it can come back from there until the Trash is emptied.
    /// `by_administrator` when the administrator command moves it there,
    /// because the user's account may not; it then keeps its owner, and
    /// taking it back out may need an administrator's password.
    ToTrash {
        package_id: String,
        by_administrator: bool,
    },
    /// The manifest's `always_warn` entry asks for the question.
    Warning {
        package_id: String,
        message_key: String,
    },
}

/// The reasons to ask before uninstalling `package_ids`, in manifest order.
/// Empty for a run of file removals that all go into a backup and that no
/// manifest asks about, OSARA included: the people who use RABBIT know that
/// REAPER stops speaking without it, so asking would only be in their way.
/// A removal no backup can undo always asks: a vendor's uninstaller, the
/// macOS administrator command, an app going to the Trash, or the elevated
/// helper removing a package's uninstaller folder and registry entry, as it
/// does for the JAWS scripts after their files went into the backup. Once
/// that folder and entry are gone, the JAWS scripts' files all go into a
/// backup and nothing asks.
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
        reasons.extend(beyond_backup_reason(item));
    }
    reasons
}

/// What of `item`'s removal a backup can't undo, as the question says it;
/// `None` when every part of it goes into a backup. A planned machine
/// cleanup decides on its own: the package's files still go into the
/// backup, so only the uninstaller folder and registry entry are beyond
/// it, or the whole package when none of its files are left to move.
fn beyond_backup_reason(item: &UninstallPlanItem) -> Option<UninstallConfirmReason> {
    let package_id = item.package_id.clone();
    if item.machine_cleanup.is_some() {
        return Some(if item.files.is_empty() {
            UninstallConfirmReason::Unbacked { package_id }
        } else {
            UninstallConfirmReason::PartlyUnbacked { package_id }
        });
    }
    if !item.unbacked {
        return None;
    }
    // "A backup can't bring it back" would be wrong for an app in the
    // Trash, where it can be found again.
    Some(if item.backup == UninstallBackup::ToTrash {
        UninstallConfirmReason::ToTrash {
            package_id,
            by_administrator: item.needs_administrator(),
        }
    } else {
        UninstallConfirmReason::Unbacked { package_id }
    })
}

/// The question's sentence for `reason`.
fn confirm_reason_line(localizer: &Localizer, reason: &UninstallConfirmReason) -> String {
    let with_package = |key: &str, package_id: &str| {
        let name = localized_package_display_name(localizer, package_id);
        localizer.format(key, &[("package", name.as_str())]).value
    };
    match reason {
        UninstallConfirmReason::Unbacked { package_id } => {
            with_package("wizard-confirm-uninstall-unbacked", package_id)
        }
        UninstallConfirmReason::PartlyUnbacked { package_id } => {
            with_package("wizard-confirm-uninstall-partly-unbacked", package_id)
        }
        UninstallConfirmReason::ToTrash {
            package_id,
            by_administrator,
        } => with_package(
            if *by_administrator {
                "wizard-confirm-uninstall-trash-elevated-macos"
            } else {
                "wizard-confirm-uninstall-trash"
            },
            package_id,
        ),
        UninstallConfirmReason::Warning { message_key, .. } => localizer.text(message_key).value,
    }
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
        let line = confirm_reason_line(localizer, reason);
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines.push(localizer.text("wizard-confirm-uninstall-question").value);
    Some(lines.join("\n\n"))
}
