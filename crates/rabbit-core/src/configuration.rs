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
    /// Packages the step depends on, with ANY-of semantics: the step is
    /// available when at least one of them is already installed or queued in
    /// the current plan, and the wizard greys the row out otherwise. A list
    /// rather than a single id because "set REAPER's language" applies to
    /// whichever language pack the user picked — one step for all of them,
    /// instead of one step per language cluttering the configuration group.
    pub requires_packages: Vec<String>,
    /// Localization key naming this step's dependency as a group, for steps
    /// that any one of several packages satisfies.
    ///
    /// "Set REAPER's language" lists every language pack, so naming one of
    /// them - whichever happened to sort first - told users the step
    /// "requires REAPER en espanol" when any pack at all would do. `None`
    /// means the step has a single dependency worth naming directly.
    pub dependency_name_key: Option<String>,
    /// Treat this step's dependency as satisfied only by a package being
    /// installed or updated in THIS run, not by one that merely happens to
    /// be on disk already.
    ///
    /// For additive steps (adding a ReaPack remote) "the package is there"
    /// is reason enough. Changing REAPER's interface language is not
    /// additive, and it is meaningless without a pack in the run: the
    /// wizard's "REAPER language after installation" dropdown lists exactly
    /// the packs you ticked, so with none ticked the step has nothing to
    /// activate. Tying the two together keeps the step and the dropdown
    /// telling the same story — untick every language pack and the step
    /// greys out and clears itself; tick one and it comes back ticked,
    /// ready to be unticked if you want the files without the switch.
    pub requires_fresh_dependency: bool,
    pub kind: ConfigurationStepKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ConfigurationStepKind {
    /// Append (or upsert) a remote repository under ReaPack's
    /// `[remotes]` section in `<resource_path>/reapack.ini`. Idempotent
    /// on URL: re-running the wizard doesn't add a duplicate.
    AddReapackRemote { name: String, url: String },
    /// Point REAPER at one installed language pack by writing
    /// `langpack=<file name>` under `[REAPER]` in `<resource>/reaper.ini`.
    ///
    /// A single step covers every language, rather than one step per
    /// language: several packs can be installed side by side (REAPER keeps
    /// them all in `LangPack/`), but exactly one is *active*, so this is one
    /// decision, not N. Which pack it activates comes from the run's
    /// `reaper_language_package` choice, and the file name is read from that
    /// package's install receipt at apply time — so it always matches what
    /// was actually installed, including which Spanish variant was picked.
    /// Idempotent: selecting the already-active pack writes nothing.
    SetReaperLanguage,
}

/// Per-run inputs a configuration step needs beyond the step definition
/// itself. Grouped so adding another doesn't grow every signature.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigurationContext<'a> {
    /// Which language pack the user chose to activate. `None` means no
    /// explicit choice — the step then activates the sole installed pack if
    /// there is exactly one, and reports "skipped" if the choice is
    /// ambiguous, rather than picking a language for the user.
    pub reaper_language_package: Option<&'a str>,
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
    /// None of the step's `requires_packages` was satisfied.
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
    /// None of the step's `requires_packages` is satisfied —
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
            requires_packages: vec![PACKAGE_REAPACK.to_string()],
            dependency_name_key: None,
            requires_fresh_dependency: false,
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
            requires_packages: vec![PACKAGE_REAPACK.to_string()],
            dependency_name_key: None,
            requires_fresh_dependency: false,
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
            requires_packages: vec![PACKAGE_REAPACK.to_string()],
            dependency_name_key: None,
            requires_fresh_dependency: false,
            kind: ConfigurationStepKind::AddReapackRemote {
                name: REAPER_ACCESSIBLE_EN_NAME.to_string(),
                url: REAPER_ACCESSIBLE_EN_URL.to_string(),
            },
        },
    ];
    steps.extend(lang_pack_steps());
    steps
}

/// Stable id of the single "set REAPER's language" step.
pub const CONFIG_SET_REAPER_LANGUAGE: &str = "set-reaper-language";

/// The one "set REAPER's language" step, offered when any language pack is
/// installed or queued. Deliberately ONE step for all languages: packs
/// coexist in `LangPack/` but only one can be active, so this is a single
/// decision — and a step per language would add a row and two translated
/// strings per locale for every language ever added, for a choice the user
/// makes once.
fn lang_pack_steps() -> Vec<ConfigurationStep> {
    let language_packs: Vec<String> = embedded_package_manifest()
        .packages
        .into_iter()
        .filter(|spec| spec.package_kind == PackageKind::LanguagePack)
        .map(|spec| spec.id)
        .collect();
    if language_packs.is_empty() {
        return Vec::new();
    }
    vec![ConfigurationStep {
        id: CONFIG_SET_REAPER_LANGUAGE.to_string(),
        display_name_key: "config-set-reaper-language-name".to_string(),
        display_description_key: "config-set-reaper-language-description".to_string(),
        // Ticked by default: someone installing a language pack almost
        // certainly wants REAPER to use it. Untick to get the file only.
        recommended: true,
        requires_packages: language_packs,
        dependency_name_key: Some("config-dependency-language-pack".to_string()),
        requires_fresh_dependency: true,
        kind: ConfigurationStepKind::SetReaperLanguage,
    }]
}

/// The `.ReaperLangPack` file to activate: the run's chosen package, or —
/// when no choice was made — the sole installed language pack. Returns
/// `None` when nothing is installed, and also when several packs are
/// installed but no choice was given, because guessing a language for the
/// user would be worse than leaving REAPER's setting alone.
fn chosen_lang_pack_file(
    resource_path: &Path,
    step: &ConfigurationStep,
    context: &ConfigurationContext<'_>,
) -> Result<Option<String>> {
    if let Some(package_id) = context.reaper_language_package {
        return installed_lang_pack_file(resource_path, package_id);
    }
    let mut installed = Vec::new();
    for package_id in &step.requires_packages {
        if let Some(file) = installed_lang_pack_file(resource_path, package_id)? {
            installed.push(file);
        }
    }
    Ok(if installed.len() == 1 {
        installed.pop()
    } else {
        None
    })
}

/// The `.ReaperLangPack` file `package_id`'s receipt says RABBIT installed,
/// if any. Resolving from the receipt (rather than a name baked into the
/// step) keeps activation correct whichever variant the user chose.
fn installed_lang_pack_file(resource_path: &Path, package_id: &str) -> Result<Option<String>> {
    let Some(state) = crate::receipt::load_install_state(resource_path)? else {
        return Ok(None);
    };
    Ok(state.packages.get(package_id).and_then(|receipt| {
        receipt.installed_files.iter().find_map(|file| {
            let name = file.path.file_name()?.to_str()?;
            name.to_ascii_lowercase()
                .ends_with(".reaperlangpack")
                .then(|| name.to_string())
        })
    }))
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
    context: &ConfigurationContext<'_>,
) -> Result<bool> {
    match &step.kind {
        ConfigurationStepKind::AddReapackRemote { url, .. } => {
            is_remote_configured(resource_path, url)
        }
        ConfigurationStepKind::SetReaperLanguage => {
            let Some(file_name) = chosen_lang_pack_file(resource_path, step, context)? else {
                // Nothing installed yet: there is no work we could already
                // have done, so the step is not "applied".
                return Ok(false);
            };
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
    context: &ConfigurationContext<'_>,
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
        ConfigurationStepKind::SetReaperLanguage => {
            let Some(file_name) = chosen_lang_pack_file(resource_path, step, context)? else {
                // The pack isn't on disk — most likely its install failed, or
                // it was skipped because REAPER itself failed. Report that and
                // move on rather than erroring: this step is best-effort, and
                // failing here aborts the whole run with a message about the
                // language pack that buries the actual cause the user needs to
                // see. The caller already filters steps whose package didn't
                // land; this is the backstop for anything that slips through.
                return Ok(ConfigurationStepReport {
                    step_id: step.id.clone(),
                    status: ConfigurationStatus::SkippedDependencyMissing,
                    message: "Skipped: no installed language pack to activate, so REAPER's language was left unchanged.".to_string(),
                    message_code: ConfigurationMessage::SkippedDependencyMissing {
                        step_id: step.id.clone(),
                        dep_id: step.requires_packages.join(", "),
                    },
                });
            };
            let file_name = &file_name;
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
        ConfigurationStepKind::SetReaperLanguage => {
            let file_name = step
                .requires_packages
                .first()
                .cloned()
                .unwrap_or_else(|| "language pack".to_string());
            (
                format!("Would set REAPER's language to the installed {file_name} language pack."),
                ConfigurationMessage::ReaperLanguageDryRun { file_name },
            )
        }
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
            let dep = if step.requires_packages.is_empty() {
                "(unknown package)".to_string()
            } else {
                step.requires_packages.join(", ")
            };
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
        assert_eq!(step.requires_packages, vec![PACKAGE_REAPACK.to_string()]);
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
        assert_eq!(step.requires_packages, vec![PACKAGE_REAPACK.to_string()]);
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
        assert_eq!(step.requires_packages, vec![PACKAGE_REAPACK.to_string()]);
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

    /// ONE "set REAPER's language" step covers every language, rather than
    /// one per language: packs coexist in LangPack/ but only one is active,
    /// so this is a single decision. A step per language also cost a row and
    /// two translated strings per locale for every language ever added.
    #[test]
    fn a_single_language_step_covers_every_language_pack() {
        use crate::package::{PackageKind, embedded_package_manifest};

        let packs: Vec<String> = embedded_package_manifest()
            .packages
            .into_iter()
            .filter(|spec| spec.package_kind == PackageKind::LanguagePack)
            .map(|spec| spec.id)
            .collect();
        assert!(packs.len() >= 2, "expected several language packs");

        let steps = builtin_configuration_steps();
        let language_steps: Vec<_> = steps
            .iter()
            .filter(|s| matches!(s.kind, ConfigurationStepKind::SetReaperLanguage))
            .collect();
        assert_eq!(
            language_steps.len(),
            1,
            "exactly one language step, however many packs exist"
        );

        let step = language_steps[0];
        assert_eq!(step.id, super::CONFIG_SET_REAPER_LANGUAGE);
        assert!(step.recommended);
        // ANY-of: every pack is listed, so the step lights up for whichever
        // one the user picked.
        for pack in &packs {
            assert!(
                step.requires_packages.contains(pack),
                "{pack} should satisfy the language step"
            );
        }
    }

    #[test]
    fn apply_writes_reapack_ini_when_not_dry_run() {
        let dir = tempdir().unwrap();
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE)
            .unwrap();

        let report =
            apply_configuration_step(dir.path(), step, &Default::default(), false).unwrap();
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

        let report = apply_configuration_step(dir.path(), step, &Default::default(), true).unwrap();
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

        assert!(!is_configuration_step_applied(dir.path(), step, &Default::default()).unwrap());
        apply_configuration_step(dir.path(), step, &Default::default(), false).unwrap();
        assert!(is_configuration_step_applied(dir.path(), step, &Default::default()).unwrap());
    }

    #[test]
    fn apply_is_idempotent_across_repeat_runs() {
        let dir = tempdir().unwrap();
        let steps = builtin_configuration_steps();
        let step = steps
            .iter()
            .find(|s| s.id == CONFIG_REAPER_ACCESSIBILITY_REPACK_REMOTE)
            .unwrap();

        apply_configuration_step(dir.path(), step, &Default::default(), false).unwrap();
        let second =
            apply_configuration_step(dir.path(), step, &Default::default(), false).unwrap();
        // Idempotent: still reports Applied, but the message records the
        // already-configured state so reports stay accurate.
        assert_eq!(second.status, ConfigurationStatus::Applied);
        assert!(second.message.contains("already configured"));
    }
}
