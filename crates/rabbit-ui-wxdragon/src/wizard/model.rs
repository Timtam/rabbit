//! Data the wizard renders: one struct per screen plus the install
//! request and outcome types the pages exchange with `rabbit-core`.

use std::path::PathBuf;

use rabbit_core::localization::DEFAULT_LOCALE;
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::plan::{AvailablePackage, PlanActionKind};
use rabbit_core::setup::SetupReport;
use rabbit_core::version::Version;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiBootstrapOptions {
    pub locale: String,
    pub locales_dir: Option<PathBuf>,
    pub portable_roots: Vec<PathBuf>,
    pub online_versions: bool,
}

impl Default for UiBootstrapOptions {
    fn default() -> Self {
        Self {
            locale: DEFAULT_LOCALE.to_string(),
            locales_dir: None,
            portable_roots: Vec::new(),
            online_versions: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardModel {
    pub window_title: String,
    pub platform: Platform,
    pub architecture: Architecture,
    pub text: WizardText,
    pub bootstrap_options: UiBootstrapOptions,
    pub current_step: WizardStep,
    pub steps: Vec<WizardStepLabel>,
    pub target_rows: Vec<TargetRow>,
    pub selected_target_index: Option<usize>,
    pub package_rows: Vec<PackageRow>,
    pub configuration_rows: Vec<ConfigurationRow>,
    pub available_packages: Vec<AvailablePackage>,
    pub review_lines: Vec<String>,
    pub notes: Vec<String>,
    pub controls: WizardControls,
    pub language_options: Vec<LanguageOption>,
    pub current_language: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageOption {
    pub locale: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardText {
    pub common_yes: String,
    pub common_no: String,
    pub target_heading: String,
    pub target_language_label: String,
    pub target_language_restart_note: String,
    pub target_choice_label: String,
    pub target_details_label: String,
    pub target_empty: String,
    pub target_portable_choice: String,
    pub target_portable_folder_label: String,
    pub target_portable_folder_message: String,
    pub target_portable_folder_browse_label: String,
    pub target_portable_pending_details: String,
    pub target_custom_portable_label: String,
    pub target_custom_portable_app_path_label: String,
    pub target_custom_portable_path_label: String,
    pub target_custom_portable_version_label: String,
    pub target_custom_portable_writable_label: String,
    pub target_custom_portable_note: String,
    pub packages_heading: String,
    pub packages_list_label: String,
    pub packages_tree_group_label: String,
    pub additional_software_tree_group_label: String,
    pub language_tree_group_label: String,
    pub configuration_tree_group_label: String,
    pub reapack_ack_heading: String,
    pub reapack_ack_body: String,
    pub reapack_ack_link_label: String,
    pub reapack_ack_confirm_label: String,
    pub version_check_heading: String,
    pub version_check_status_pending: String,
    pub version_check_progress_label: String,
    pub version_check_error_heading: String,
    pub package_details_label: String,
    pub packages_osara_keymap_heading: String,
    pub packages_osara_keymap_replace_label: String,
    pub packages_reaper_language_label: String,
    pub packages_spanish_variant_label: String,
    /// The selectable Spanish OSARA translations, in the same order as
    /// [`VARIANT_CHOICE_IDS`].
    pub packages_spanish_variant_options: Vec<String>,
    pub packages_osara_keymap_unavailable_note: String,
    pub packages_osara_keymap_preserve_note: String,
    pub packages_osara_keymap_replace_note: String,
    pub package_details_handling_prefix: String,
    pub package_handling_automatic: String,
    pub package_handling_unattended: String,
    pub package_handling_planned: String,
    pub package_handling_manual: String,
    pub package_handling_unavailable: String,
    pub review_heading: String,
    pub review_target_prefix: String,
    pub review_package_heading: String,
    pub review_osara_keymap_heading: String,
    pub review_osara_keymap_preserve: String,
    pub review_osara_keymap_replace: String,
    pub review_notes_heading: String,
    pub review_preflight_prefix: String,
    pub review_no_target: String,
    pub review_no_package: String,
    pub progress_heading: String,
    pub progress_status: String,
    pub progress_status_running: String,
    /// Shown from the moment the user confirms the stop until the pipeline
    /// finishes the step it is on. Deliberately not "cancelled": nothing has
    /// stopped yet when it first appears.
    pub progress_status_cancelling: String,
    pub progress_details_label: String,
    pub progress_details_idle: String,
    pub progress_details_starting: String,
    pub progress_details_cache_prefix: String,
    pub done_heading: String,
    pub done_status: String,
    pub done_status_success: String,
    pub done_status_completed_with_errors: String,
    pub done_status_error: String,
    pub done_status_cancelled: String,
    pub done_status_no_packages: String,
    pub done_show_details_label: String,
    pub done_launch_reaper_label: String,
    pub done_open_resource_label: String,
    pub done_no_reaper_app: String,
    pub done_launch_reaper_error_prefix: String,
    pub done_open_resource_error_prefix: String,
    pub done_self_update_apply_running: String,
    pub done_self_update_error_prefix: String,
    pub done_self_update_relaunch_prefix: String,
    pub self_update_status_checking: String,
    pub close_during_install_title: String,
    pub close_during_install_body: String,
    pub close_during_self_update_title: String,
    pub close_during_self_update_body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStep {
    Target,
    VersionCheck,
    Packages,
    ReapackAcknowledgement,
    Review,
    Progress,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardStepLabel {
    pub step: WizardStep,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRow {
    pub label: String,
    pub details: String,
    pub app_path: Option<PathBuf>,
    pub planned_app_path: PathBuf,
    pub path: PathBuf,
    pub version: Option<Version>,
    pub portable: bool,
    pub selected: bool,
    pub writable: bool,
    /// Architecture of the REAPER binary at this target. Populated by the
    /// detection layer's binary-header probe rather than the host arch, so
    /// e.g. an Intel REAPER on an Apple Silicon Mac, or an x86_64 REAPER on
    /// Windows-on-ARM, gets the arch-correct extension binaries (ReaPack,
    /// SWS, OSARA) instead of host-matching ones REAPER would refuse to
    /// load. Falls back to `Architecture::current()` when the binary can't
    /// be probed (synthetic / portable targets without a binary on disk yet).
    pub architecture: Architecture,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRow {
    pub package_id: String,
    pub display_name: String,
    pub description: String,
    pub selected: bool,
    pub summary: String,
    pub details: String,
    pub installed_version: String,
    pub available_version: String,
    pub action: PlanActionKind,
    pub action_label: String,
    /// Plan-time action, captured before any user toggle of the package
    /// checkbox. Used by the wizard's checklist handler to decide whether
    /// re-checking a row means "Install" (originally not installed) or
    /// "Update" (already installed) — the displayed `action` mutates as
    /// the user clicks, but `original_action` is the authoritative anchor.
    pub original_action: PlanActionKind,
    pub reason: String,
    pub handling_summary: String,
    pub manual_attention_expected: bool,
    /// `false` when this package can't be installed against the currently
    /// selected target — the row is shown but its checkbox is disabled and
    /// the row label carries a localized indicator. Today only true → false
    /// flip is "JAWS-for-REAPER scripts on a portable REAPER target", since
    /// the NSIS installer hard-codes `%APPDATA%\REAPER\UserPlugins\` and
    /// can't honor the portable destination.
    pub available_for_target: bool,
    /// Localized reason matching `available_for_target == false`. `None`
    /// when the row is available.
    pub unavailability_reason: Option<String>,
    /// Which wizard UI group this row belongs to ("Packages" vs "Additional
    /// software"). Mirrors the package spec's category.
    pub category: rabbit_core::package::PackageCategory,
    /// Mirrors the spec's `requires_standard_install`: when `true`, the row is
    /// disabled on a portable REAPER target (the package installs to a fixed
    /// location outside any portable folder).
    pub requires_standard_install: bool,
}

/// Wizard-side row for a single [`crate::configuration::ConfigurationStep`]
/// (re-exported from `rabbit-core` as `rabbit_core::configuration::*`).
/// Mirrors the `PackageRow` shape just enough that the tree UI can render
/// it as a sibling leaf under the "Configuration" group, but configuration
/// steps don't have versions / actions / artifacts, so most package
/// fields don't apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationRow {
    /// Stable id of the underlying `ConfigurationStep`.
    pub step_id: String,
    /// Localized step name (the row's primary label).
    pub display_name: String,
    /// Localized free-form description shown in the package-details
    /// pane when the row is selected.
    pub description: String,
    /// Whether the row's checkbox is currently ticked. Initialised from
    /// the step's `recommended` flag intersected with the row's
    /// `available_for_target`.
    pub selected: bool,
    /// Row label as rendered in the tree. For configuration rows the
    /// summary is just `display_name` today; kept as a separate field
    /// so the wizard's tree-refresh helpers can stay symmetric with
    /// PackageRow.
    pub summary: String,
    /// Free-form details shown alongside `description` (status hints,
    /// dependency reasons). Today carries the localized
    /// "(unavailable: …)" sentence when the dependency package isn't
    /// queued for install.
    pub details: String,
    /// `true` iff the step's dependency package (if any) is either
    /// already installed on the selected target or queued for install
    /// in the current package plan. The wizard greys out the row's
    /// checkbox when this is `false`.
    pub available_for_target: bool,
    /// `true` iff the step's effect is already in place on disk under
    /// the selected target (e.g. the ReaPack remote URL is already
    /// listed in `reapack.ini`). The wizard treats this like
    /// `available_for_target == false` for interactivity (the checkbox
    /// is disabled) but uses a different reason string so the user
    /// understands the row isn't unsupported, just done.
    pub already_applied: bool,
    /// Localized reason matching `available_for_target == false` OR
    /// `already_applied == true`. `None` when the row is interactive.
    pub unavailability_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardControls {
    pub back_label: String,
    pub next_label: String,
    pub install_label: String,
    pub close_label: String,
    pub can_go_back: bool,
    pub can_go_next: bool,
    pub can_install: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OsaraKeymapChoice {
    PreserveCurrent,
    ReplaceCurrent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardInstallOptions {
    pub dry_run: bool,
    pub allow_reaper_running: bool,
    pub stage_unsupported: bool,
    pub osara_keymap_choice: OsaraKeymapChoice,
    /// Chosen package flavour per package id; see
    /// [`rabbit_core::package::PackageVariant`].
    pub package_variants: std::collections::BTreeMap<String, String>,
    /// Language pack to make active after installing; see
    /// [`WizardInstallRequest::reaper_language_package`].
    pub reaper_language_package: Option<String>,
    pub cache_dir: Option<PathBuf>,
}

impl Default for WizardInstallOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            allow_reaper_running: false,
            stage_unsupported: true,
            osara_keymap_choice: OsaraKeymapChoice::ReplaceCurrent,
            package_variants: std::collections::BTreeMap::new(),
            reaper_language_package: None,
            cache_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardInstallRequest {
    pub resource_path: PathBuf,
    pub package_ids: Vec<String>,
    pub platform: Platform,
    pub architecture: Architecture,
    pub portable: bool,
    pub target_app_path: Option<PathBuf>,
    pub dry_run: bool,
    pub allow_reaper_running: bool,
    pub stage_unsupported: bool,
    pub osara_keymap_choice: OsaraKeymapChoice,
    /// Chosen package flavour per package id, forwarded to
    /// [`rabbit_core::setup::SetupOptions::package_variants`].
    pub package_variants: std::collections::BTreeMap<String, String>,
    /// Language pack to make active after installing. Several can be
    /// installed at once — REAPER keeps them all — but only one is active.
    pub reaper_language_package: Option<String>,
    pub cache_dir: PathBuf,
    /// Packages whose plan-time decision was `Keep` (already current) but
    /// the user explicitly checked the box anyway, opting in to a
    /// re-install. The setup pipeline promotes these from Keep to Update
    /// so the install step actually runs instead of being silently
    /// skipped.
    pub force_reinstall_packages: Vec<String>,
    /// Configuration step ids the user opted in to. Forwarded straight
    /// through to [`SetupOptions::configuration_step_ids`].
    pub configuration_step_ids: Vec<String>,
    /// Opt-out-remembering packages the user actively turned down: the row
    /// had something to offer (an install or an update) and they left it
    /// unticked. Recorded so the suggestion does not come back next launch.
    ///
    /// A `Keep` row is deliberately absent from this and from
    /// `accepted_packages`. An installed, up-to-date package sits unticked
    /// because there is nothing to do, which is silence rather than a
    /// verdict; reading it as a refusal would eventually turn RABBIT's own
    /// successful install into a "no".
    pub declined_packages: Vec<String>,
    /// Opt-out-remembering packages the user ticked, clearing any refusal
    /// recorded earlier so a change of mind sticks.
    pub accepted_packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardInstallSummary {
    pub status_line: String,
    pub detail_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardPackagePlan {
    pub package_rows: Vec<PackageRow>,
    pub notes: Vec<String>,
    pub can_install: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardReviewPreview {
    pub lines: Vec<String>,
    pub can_install: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WizardOutcomeStatus {
    Success,
    /// The operation finished, but one or more packages failed or were
    /// skipped because a dependency failed. Distinct from `Error` (the whole
    /// operation aborted) so the saved report doesn't claim `success` when
    /// the done page shows "completed with errors".
    CompletedWithErrors,
    Error,
    /// The user stopped the run. Some packages may have installed before
    /// the stop; the ones after it were never touched. Distinct from both
    /// `CompletedWithErrors` (nothing broke) and `Error` (the run did what
    /// it was asked to do, right up to being asked to stop).
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WizardOutcomeReport {
    pub status: WizardOutcomeStatus,
    pub resource_path: PathBuf,
    pub target_app_path: Option<PathBuf>,
    pub package_ids: Vec<String>,
    pub platform: Platform,
    pub architecture: Architecture,
    pub portable: bool,
    pub dry_run: bool,
    pub allow_reaper_running: bool,
    pub stage_unsupported: bool,
    pub cache_dir: PathBuf,
    pub osara_keymap_choice: OsaraKeymapChoice,
    pub status_line: String,
    pub detail_lines: Vec<String>,
    pub error_message: Option<String>,
    pub setup_report: Option<SetupReport>,
}
