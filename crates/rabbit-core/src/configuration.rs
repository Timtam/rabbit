//! Post-install configuration steps.
//!
//! A configuration step is a chunk of work that runs *after* the package
//! install pipeline has finished, typically to wire newly-installed
//! packages into REAPER's per-target config files. Today the only
//! builtin step is "add the REAPER Accessibility ReaPack remote to
//! `reapack.ini`"; more steps (CLI prefs, REAPER `.ini` tweaks, etc.)
//! can be added later by extending `ConfigurationStepKind`.
//!
//! The wizard UI surfaces these as a separate "Configuration" group in
//! the same tree the user picks packages in. CLI users opt in via
//! explicit flags.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::package::{PACKAGE_REAPACK, PackageKind, embedded_package_manifest};
use crate::reapack::{RemoteUpsertOutcome, is_remote_configured, upsert_remote};
use crate::reaper_ini::{LangPackSelectionOutcome, select_lang_pack, selected_lang_pack};

/// Stable id for the "configure REAPER Accessibility ReaPack remote"
/// step. Used by callers (CLI, wizard) to identify the step across
/// runs.
pub const CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE: &str =
    "reapack-add-reaper-accessibility-remote";

/// Display name to write into `reapack.ini`'s `remote<N>=<name>|...`
/// entry. ReaPack shows this in its Manage Repositories UI.
const REAPER_ACCESSIBILITY_REPACK_NAME: &str = "REAPER Accessibility";
/// Repository index URL.
const REAPER_ACCESSIBILITY_REPACK_URL: &str =
    "https://github.com/Timtam/reapack/raw/master/index.xml";

/// Stable id for the "configure REAPER Accessible (FR) ReaPack remote"
/// step — the francophone resources repository from the
/// `reaperaccessible` team.
pub const CONFIG_REAPER_ACCESSIBLE_FR_REMOTE: &str = "reapack-add-reaper-accessible-fr-remote";
/// Stable id for the "configure REAPER Accessible (EN) ReaPack remote"
/// step — the anglophone resources repository from the
/// `reaperaccessible` team.
pub const CONFIG_REAPER_ACCESSIBLE_EN_REMOTE: &str = "reapack-add-reaper-accessible-en-remote";

/// Display name / index URL for the `reaperaccessible/rap_fr` repo.
const REAPER_ACCESSIBLE_FR_NAME: &str = "REAPER Accessible (FR)";
const REAPER_ACCESSIBLE_FR_URL: &str =
    "https://github.com/reaperaccessible/rap_fr/raw/main/index.xml";
/// Display name / index URL for the `reaperaccessible/rap_en` repo.
const REAPER_ACCESSIBLE_EN_NAME: &str = "REAPER Accessible (EN)";
const REAPER_ACCESSIBLE_EN_URL: &str =
    "https://github.com/reaperaccessible/rap_en/raw/main/index.xml";

/// One unit of post-install configuration work the wizard / CLI can
/// offer to the user. Steps are declarative — `kind` carries the data
/// `apply_configuration_step` needs to actually perform the work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationStep {
    pub id: String,
    /// Fluent key for the step's display name (shown in the wizard
    /// tree row and the Review/Done summaries).
    pub display_name_key: String,
    /// Fluent key for the human-readable explanation shown in the
    /// wizard's package-details pane.
    pub display_description_key: String,
    /// `true` ⇒ check the wizard row by default and have the CLI's
    /// "auto-apply recommended configuration" path enable it. The user
    /// can still untick it.
    pub recommended: bool,
    /// Package the step depends on. The wizard disables (greys out)
    /// the row when this package isn't already installed *and* isn't
    /// queued for install in the current plan; the CLI refuses to run
    /// the step in the same situation.
    pub requires_package_id: Option<String>,
    pub kind: ConfigurationStepKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ConfigurationStepKind {
    /// Append (or upsert) a remote repository under ReaPack's
    /// `[remotes]` section in `<resource_path>/reapack.ini`. Idempotent
    /// on URL: re-running the wizard doesn't add a duplicate.
    AddReapackRemote { name: String, url: String },
    /// Point REAPER at an installed language pack by writing
    /// `langpack=<file_name>` under `[REAPER]` in `<resource>/reaper.ini`.
    /// Idempotent: selecting the already-active pack writes nothing.
    SetReaperLanguage { file_name: String },
}

/// Outcome of applying a single configuration step. Mirrors the
/// per-package status types so reports can stitch them in alongside
/// `PackageOperationItem`. `message` is a stable English form for
/// the saved JSON report; `message_code` is the structured shape the
/// wizard / CLI dispatch on to produce a localized string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationStepReport {
    pub step_id: String,
    pub status: ConfigurationStatus,
    pub message: String,
    #[serde(default)]
    pub message_code: ConfigurationMessage,
}

/// Structured message variants for [`ConfigurationStepReport`]. The
/// wizard's done-page summary localizes by dispatching on the variant
/// instead of inserting `message` verbatim into a translated wrapper
/// (which would otherwise leave English fragments in a German UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", tag = "code")]
pub enum ConfigurationMessage {
    /// `AddReapackRemote` step ran and the URL was already in
    /// `reapack.ini`'s `[remotes]` section.
    ReapackRemoteAlreadyPresent { name: String, url: String },
    /// `AddReapackRemote` step appended a new remote into an existing
    /// `reapack.ini`.
    ReapackRemoteAdded { name: String, url: String },
    /// `AddReapackRemote` step created `reapack.ini` from scratch.
    ReapackRemoteCreatedFile { name: String, url: String },
    /// Dry-run preview of an `AddReapackRemote` step.
    ReapackRemoteDryRun { name: String, url: String },
    /// `SetReaperLanguage` step ran and REAPER already used this pack.
    ReaperLanguageAlreadySelected { file_name: String },
    /// `SetReaperLanguage` step pointed REAPER at this language pack.
    ReaperLanguageSelected { file_name: String },
    /// Dry-run preview of a `SetReaperLanguage` step.
    ReaperLanguageDryRun { file_name: String },
    /// User opted out of this configuration step.
    Skipped { step_id: String },
    /// The step's `requires_package_id` dependency wasn't satisfied.
    SkippedDependencyMissing { step_id: String, dep_id: String },
    /// Generic "applied with no observable change" fallback used by
    /// [`skipped_step_report`] when called with `Applied`.
    #[default]
    AppliedNoOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigurationStatus {
    /// Step ran and the configuration is now in place (whether we
    /// wrote anything or it was already correct).
    Applied,
    /// User opted out (or didn't opt in for non-recommended steps).
    Skipped,
    /// The step's `requires_package_id` dependency isn't satisfied —
    /// e.g. the user wants to add a ReaPack remote but didn't install
    /// ReaPack and it isn't already on disk.
    SkippedDependencyMissing,
    /// `dry_run` was set; we didn't write anything but report what
    /// would have happened.
    DryRun,
}

/// All configuration steps RABBIT knows how to run. Hardcoded today;
/// can move to JSON later if/when the catalogue grows.
pub fn builtin_configuration_steps() -> Vec<ConfigurationStep> {
    let mut steps = vec![
        ConfigurationStep {
            id: CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE.to_string(),
            display_name_key: "config-reapack-reaper-accessibility-name".to_string(),
            display_description_key: "config-reapack-reaper-accessibility-description".to_string(),
            recommended: true,
            requires_package_id: Some(PACKAGE_REAPACK.to_string()),
            kind: ConfigurationStepKind::AddReapackRemote {
                name: REAPER_ACCESSIBILITY_REPACK_NAME.to_string(),
                url: REAPER_ACCESSIBILITY_REPACK_URL.to_string(),
            },
        },
        ConfigurationStep {
            id: CONFIG_REAPER_ACCESSIBLE_FR_REMOTE.to_string(),
            display_name_key: "config-reapack-reaper-accessible-fr-name".to_string(),
            display_description_key: "config-reapack-reaper-accessible-fr-description".to_string(),
            // Optional, not recommended: the repository carries
            // French-language resources, so it isn't pre-selected for
            // everyone — users who want it opt in on the wizard's
            // configuration page (or via `--config-step`).
            recommended: false,
            requires_package_id: Some(PACKAGE_REAPACK.to_string()),
            kind: ConfigurationStepKind::AddReapackRemote {
                name: REAPER_ACCESSIBLE_FR_NAME.to_string(),
                url: REAPER_ACCESSIBLE_FR_URL.to_string(),
            },
        },
        ConfigurationStep {
            id: CONFIG_REAPER_ACCESSIBLE_EN_REMOTE.to_string(),
            display_name_key: "config-reapack-reaper-accessible-en-name".to_string(),
            display_description_key: "config-reapack-reaper-accessible-en-description".to_string(),
            recommended: true,
            requires_package_id: Some(PACKAGE_REAPACK.to_string()),
            kind: ConfigurationStepKind::AddReapackRemote {
                name: REAPER_ACCESSIBLE_EN_NAME.to_string(),
                url: REAPER_ACCESSIBLE_EN_URL.to_string(),
            },
        },
    ];
    steps.extend(lang_pack_steps());
    steps
}

/// Stable id of the "set REAPER's language" step for a language subtag.
pub fn config_set_reaper_language_id(language: &str) -> String {
    format!("set-reaper-language-{language}")
}

/// One "set REAPER's language" step per language pack in the embedded
/// manifest, so adding a language pack JSON automatically offers to
/// activate it — no extra Rust. Each step depends on its own pack, so the
/// wizard greys it out (and the CLI refuses it) unless that pack is
/// installed or queued in the current plan.
fn lang_pack_steps() -> Vec<ConfigurationStep> {
    embedded_package_manifest()
        .packages
        .into_iter()
        .filter(|spec| spec.package_kind == PackageKind::LanguagePack)
        .filter_map(|spec| {
            let language = spec.language.clone()?;
            let file_name = spec.install_as.clone()?;
            Some(ConfigurationStep {
                id: config_set_reaper_language_id(&language),
                display_name_key: format!("config-set-reaper-language-{language}-name"),
                display_description_key: format!(
                    "config-set-reaper-language-{language}-description"
                ),
                // Ticked by default: a user installing a language pack
                // almost certainly wants REAPER to use it. They can untick
                // to get the file without changing REAPER's setting.
                recommended: true,
                requires_package_id: Some(spec.id.clone()),
                kind: ConfigurationStepKind::SetReaperLanguage { file_name },
            })
        })
        .collect()
}

/// `true` when the on-disk state under `resource_path` already
/// reflects what `step` would write. Used by the wizard to grey out
/// the row (and by the CLI's auto-include path to suppress recommended
/// steps that are already in place) so we don't offer work that would
/// be a no-op. Returns `Ok(false)` for steps whose target doesn't
/// exist yet (e.g. no `reapack.ini` at all).
pub fn is_configuration_step_applied(
    resource_path: &Path,
    step: &ConfigurationStep,
) -> Result<bool> {
    match &step.kind {
        ConfigurationStepKind::AddReapackRemote { url, .. } => {
            is_remote_configured(resource_path, url)
        }
        ConfigurationStepKind::SetReaperLanguage { file_name } => {
            Ok(selected_lang_pack(resource_path)?.as_deref() == Some(file_name.as_str()))
        }
    }
}

/// Apply a single configuration step. Caller decides whether to run it
/// (selection + dependency check live in the wizard / CLI plumbing);
/// this function just performs the work.
pub fn apply_configuration_step(
    resource_path: &Path,
    step: &ConfigurationStep,
    dry_run: bool,
) -> Result<ConfigurationStepReport> {
    if dry_run {
        let (message, message_code) = dry_run_message_for(step);
        return Ok(ConfigurationStepReport {
            step_id: step.id.clone(),
            status: ConfigurationStatus::DryRun,
            message,
            message_code,
        });
    }

    match &step.kind {
        ConfigurationStepKind::AddReapackRemote { name, url } => {
            let outcome = upsert_remote(resource_path, name, url)?;
            let (message, message_code) = match outcome {
                RemoteUpsertOutcome::AlreadyPresent => (
                    format!(
                        "ReaPack remote {name:?} ({url}) is already configured in reapack.ini."
                    ),
                    ConfigurationMessage::ReapackRemoteAlreadyPresent {
                        name: name.clone(),
                        url: url.clone(),
                    },
                ),
                RemoteUpsertOutcome::Added => (
                    format!("Added ReaPack remote {name:?} ({url}) to reapack.ini."),
                    ConfigurationMessage::ReapackRemoteAdded {
                        name: name.clone(),
                        url: url.clone(),
                    },
                ),
                RemoteUpsertOutcome::CreatedFile => (
                    format!(
                        "Created reapack.ini with ReaPack remote {name:?} ({url}). ReaPack will add its default repositories on the next REAPER launch."
                    ),
                    ConfigurationMessage::ReapackRemoteCreatedFile {
                        name: name.clone(),
                        url: url.clone(),
                    },
                ),
            };
            Ok(ConfigurationStepReport {
                step_id: step.id.clone(),
                status: ConfigurationStatus::Applied,
                message,
                message_code,
            })
        }
        ConfigurationStepKind::SetReaperLanguage { file_name } => {
            let outcome = select_lang_pack(resource_path, file_name)?;
            let (message, message_code) = match outcome {
                LangPackSelectionOutcome::AlreadySelected => (
                    format!("REAPER is already set to use {file_name}."),
                    ConfigurationMessage::ReaperLanguageAlreadySelected {
                        file_name: file_name.clone(),
                    },
                ),
                LangPackSelectionOutcome::Replaced
                | LangPackSelectionOutcome::Added
                | LangPackSelectionOutcome::CreatedFile => (
                    format!("Set REAPER's language to {file_name} in reaper.ini."),
                    ConfigurationMessage::ReaperLanguageSelected {
                        file_name: file_name.clone(),
                    },
                ),
            };
            Ok(ConfigurationStepReport {
                step_id: step.id.clone(),
                status: ConfigurationStatus::Applied,
                message,
                message_code,
            })
        }
    }
}

fn dry_run_message_for(step: &ConfigurationStep) -> (String, ConfigurationMessage) {
    match &step.kind {
        ConfigurationStepKind::AddReapackRemote { name, url } => (
            format!("Would add ReaPack remote {name:?} ({url}) to reapack.ini."),
            ConfigurationMessage::ReapackRemoteDryRun {
                name: name.clone(),
                url: url.clone(),
            },
        ),
        ConfigurationStepKind::SetReaperLanguage { file_name } => (
            format!("Would set REAPER's language to {file_name} in reaper.ini."),
            ConfigurationMessage::ReaperLanguageDryRun {
                file_name: file_name.clone(),
            },
        ),
    }
}

/// Build a "skipped" report for the case where the user didn't opt in
/// or the step's dependency is missing. Centralised so callers don't
/// have to hand-roll the message.
pub fn skipped_step_report(
    step: &ConfigurationStep,
    status: ConfigurationStatus,
) -> ConfigurationStepReport {
    let (message, message_code) = match status {
        ConfigurationStatus::Skipped => (
            format!("Configuration step {:?} was not selected.", step.id),
            ConfigurationMessage::Skipped {
                step_id: step.id.clone(),
            },
        ),
        ConfigurationStatus::SkippedDependencyMissing => {
            let dep = step
                .requires_package_id
                .clone()
                .unwrap_or_else(|| "(unknown package)".to_string());
            (
                format!(
                    "Configuration step {:?} skipped because its dependency package {dep:?} was not installed and is not part of this plan.",
                    step.id,
                ),
                ConfigurationMessage::SkippedDependencyMissing {
                    step_id: step.id.clone(),
                    dep_id: dep,
                },
            )
        }
        ConfigurationStatus::Applied => (
            format!("Configuration step {:?} applied without changes.", step.id),
            ConfigurationMessage::AppliedNoOp,
        ),
        ConfigurationStatus::DryRun => dry_run_message_for(step),
    };
    ConfigurationStepReport {
        step_id: step.id.clone(),
        status,
        message,
        message_code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn builtin_steps_include_reaper_accessibility_repack_remote() {
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE)
            .expect("REAPER Accessibility ReaPack remote step is missing");
        assert!(step.recommended);
        assert_eq!(step.requires_package_id.as_deref(), Some(PACKAGE_REAPACK));
        match &step.kind {
            ConfigurationStepKind::AddReapackRemote { name, url } => {
                assert_eq!(name, "REAPER Accessibility");
                assert_eq!(
                    url,
                    "https://github.com/Timtam/reapack/raw/master/index.xml"
                );
            }
            other => panic!("unexpected step kind: {other:?}"),
        }
    }

    #[test]
    fn builtin_steps_include_reaper_accessible_fr_remote() {
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBLE_FR_REMOTE)
            .expect("REAPER Accessible (FR) ReaPack remote step is missing");
        // French-language resources: offered, but opt-in rather than
        // pre-selected.
        assert!(!step.recommended);
        assert_eq!(step.requires_package_id.as_deref(), Some(PACKAGE_REAPACK));
        match &step.kind {
            ConfigurationStepKind::AddReapackRemote { name, url } => {
                assert_eq!(name, "REAPER Accessible (FR)");
                assert_eq!(
                    url,
                    "https://github.com/reaperaccessible/rap_fr/raw/main/index.xml"
                );
            }
            other => panic!("unexpected step kind: {other:?}"),
        }
    }

    #[test]
    fn builtin_steps_include_reaper_accessible_en_remote() {
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBLE_EN_REMOTE)
            .expect("REAPER Accessible (EN) ReaPack remote step is missing");
        assert!(step.recommended);
        assert_eq!(step.requires_package_id.as_deref(), Some(PACKAGE_REAPACK));
        match &step.kind {
            ConfigurationStepKind::AddReapackRemote { name, url } => {
                assert_eq!(name, "REAPER Accessible (EN)");
                assert_eq!(
                    url,
                    "https://github.com/reaperaccessible/rap_en/raw/main/index.xml"
                );
            }
            other => panic!("unexpected step kind: {other:?}"),
        }
    }

    /// Language-pack activation steps are generated from the manifest, so
    /// adding a language pack JSON offers to activate it with no extra Rust.
    /// Each depends on its own pack, so the wizard greys it out unless that
    /// pack is installed or queued.
    #[test]
    fn builtin_steps_include_one_language_step_per_language_pack() {
        use crate::package::{PackageKind, embedded_package_manifest};

        let steps = builtin_configuration_steps();
        let packs: Vec<_> = embedded_package_manifest()
            .packages
            .into_iter()
            .filter(|spec| spec.package_kind == PackageKind::LanguagePack)
            .collect();
        assert!(packs.len() >= 2, "expected the Spanish and German packs");

        for pack in packs {
            let language = pack.language.clone().expect("pack declares a language");
            let step = steps
                .iter()
                .find(|s| s.id == super::config_set_reaper_language_id(&language))
                .unwrap_or_else(|| panic!("no language step for {}", pack.id));
            assert!(step.recommended, "{}: should be ticked by default", pack.id);
            assert_eq!(
                step.requires_package_id.as_deref(),
                Some(pack.id.as_str()),
                "{}: step must depend on its own pack",
                pack.id
            );
            match &step.kind {
                ConfigurationStepKind::SetReaperLanguage { file_name } => {
                    // Writes the same name the package installs under.
                    assert_eq!(Some(file_name.as_str()), pack.install_as.as_deref());
                }
                other => panic!("unexpected step kind: {other:?}"),
            }
        }
    }

    #[test]
    fn apply_sets_and_reports_the_reaper_language() {
        let dir = tempdir().unwrap();
        let resource = dir.path();
        let step = builtin_configuration_steps()
            .into_iter()
            .find(|s| s.id == super::config_set_reaper_language_id("es"))
            .expect("Spanish language step");

        // Nothing selected yet.
        assert!(!is_configuration_step_applied(resource, &step).unwrap());

        let report = apply_configuration_step(resource, &step, false).unwrap();
        assert_eq!(report.status, ConfigurationStatus::Applied);
        assert!(matches!(
            report.message_code,
            ConfigurationMessage::ReaperLanguageSelected { .. }
        ));
        assert_eq!(
            crate::reaper_ini::selected_lang_pack(resource)
                .unwrap()
                .as_deref(),
            Some("es_ES.ReaperLangPack")
        );

        // Now it reads as applied, and re-running is a no-op.
        assert!(is_configuration_step_applied(resource, &step).unwrap());
        let again = apply_configuration_step(resource, &step, false).unwrap();
        assert!(matches!(
            again.message_code,
            ConfigurationMessage::ReaperLanguageAlreadySelected { .. }
        ));
    }

    #[test]
    fn apply_writes_reapack_ini_when_not_dry_run() {
        let dir = tempdir().unwrap();
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE)
            .unwrap();

        let report = apply_configuration_step(dir.path(), step, false).unwrap();
        assert_eq!(report.status, ConfigurationStatus::Applied);
        assert!(
            dir.path()
                .join(crate::reapack::REAPACK_INI_RELATIVE_PATH)
                .is_file()
        );
    }

    #[test]
    fn apply_does_not_touch_disk_when_dry_run() {
        let dir = tempdir().unwrap();
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE)
            .unwrap();

        let report = apply_configuration_step(dir.path(), step, true).unwrap();
        assert_eq!(report.status, ConfigurationStatus::DryRun);
        assert!(
            !dir.path()
                .join(crate::reapack::REAPACK_INI_RELATIVE_PATH)
                .exists()
        );
    }

    #[test]
    fn is_applied_reports_false_when_remote_missing_then_true_after_apply() {
        let dir = tempdir().unwrap();
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE)
            .unwrap();

        assert!(!is_configuration_step_applied(dir.path(), step).unwrap());
        apply_configuration_step(dir.path(), step, false).unwrap();
        assert!(is_configuration_step_applied(dir.path(), step).unwrap());
    }

    #[test]
    fn apply_is_idempotent_across_repeat_runs() {
        let dir = tempdir().unwrap();
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE)
            .unwrap();

        apply_configuration_step(dir.path(), step, false).unwrap();
        let second = apply_configuration_step(dir.path(), step, false).unwrap();
        // Idempotent: still reports Applied, but the message records the
        // already-configured state so reports stay accurate.
        assert_eq!(second.status, ConfigurationStatus::Applied);
        assert!(second.message.contains("already configured"));
    }
}
