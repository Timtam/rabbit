//! Localized labels shared by the row builders and the run summary.

use rabbit_core::localization::Localizer;
use rabbit_core::model::Architecture;
use rabbit_core::operation::{PackageOperationStatus, PlannedExecutionKind};
use rabbit_core::plan::PlanActionKind;

use super::bootstrap::localizer_from_options;
use super::model::WizardModel;

pub(crate) fn format_localized_message(
    localizer: Option<&Localizer>,
    id: &str,
    args: &[(&str, String)],
    fallback: String,
) -> String {
    let Some(localizer) = localizer else {
        return fallback;
    };
    let borrowed_args = args
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect::<Vec<_>>();
    localizer.format(id, &borrowed_args).value
}

pub(crate) fn planned_execution_runner_label(
    localizer: Option<&Localizer>,
    kind: PlannedExecutionKind,
) -> String {
    let (id, fallback) = match kind {
        PlannedExecutionKind::LaunchInstallerExecutable => (
            "wizard-planned-runner-launch-installer",
            "Launch installer executable",
        ),
        PlannedExecutionKind::ExtractArchiveAndRunInstaller => (
            "wizard-planned-runner-extract-archive",
            "Extract archive and run contained installer",
        ),
        PlannedExecutionKind::ExtractArchiveAndCopyOsaraAssets => (
            "wizard-planned-runner-extract-archive-copy-osara",
            "Extract archive and copy OSARA installer assets",
        ),
        PlannedExecutionKind::MountDiskImageAndRunInstaller => (
            "wizard-planned-runner-mount-disk-image",
            "Mount disk image and run contained installer",
        ),
        PlannedExecutionKind::MountDiskImageAndCopyAppBundle => (
            "wizard-planned-runner-mount-disk-image-copy-app",
            "Mount disk image and copy contained app bundle",
        ),
        PlannedExecutionKind::MountDiskImageAndRunPkgInstaller => (
            "wizard-planned-runner-mount-disk-image-run-pkg",
            "Mount disk image and run contained pkg installer",
        ),
    };
    localizer
        .map(|localizer| localizer.text(id).value)
        .unwrap_or_else(|| fallback.to_string())
}

/// Localized "Install / Update / Keep" label resolver scoped to the saved
/// summary report. Mirrors the wizard's `action_label` but works against an
/// `Option<&Localizer>` so the summarizer can degrade gracefully when no
/// localizer is available.
pub(crate) fn action_label_for_summary(
    localizer: Option<&Localizer>,
    action: PlanActionKind,
) -> String {
    let (id, fallback) = match action {
        PlanActionKind::Install => ("action-install", "Install"),
        PlanActionKind::Update => ("action-update", "Update"),
        PlanActionKind::Keep => ("action-keep", "Keep"),
    };
    localizer
        .map(|localizer| localizer.text(id).value)
        .unwrap_or_else(|| fallback.to_string())
}

/// Localized status-label resolver for `PackageOperationStatus` values
/// surfaced by the saved summary report.
pub(crate) fn status_label_for_summary(
    localizer: Option<&Localizer>,
    status: PackageOperationStatus,
) -> String {
    let (id, fallback) = match status {
        PackageOperationStatus::InstalledOrChecked => {
            ("status-installed-or-checked", "Installed or checked")
        }
        PackageOperationStatus::PlannedUnattended => {
            ("status-planned-unattended", "Planned unattended")
        }
        PackageOperationStatus::DeferredUnattended => {
            ("status-deferred-unattended", "Deferred unattended")
        }
        PackageOperationStatus::SkippedCurrent => {
            ("status-skipped-current", "Skipped (already current)")
        }
        PackageOperationStatus::Failed => ("status-failed", "Failed"),
        PackageOperationStatus::SkippedDependencyFailed => (
            "status-skipped-dependency-failed",
            "Skipped (dependency failed)",
        ),
        PackageOperationStatus::Cancelled => ("status-cancelled", "Not installed (you stopped)"),
    };
    localizer
        .map(|localizer| localizer.text(id).value)
        .unwrap_or_else(|| fallback.to_string())
}

/// Translate a [`rabbit_core::operation::PackageOperationMessage`] into a
/// localized sentence using Fluent. Falls back to the message's English
/// form (`fallback_english`) when the locale doesn't have the key the
/// variant maps to — that's the same English string rabbit-core stamps
/// into the JSON report, so the saved report remains stable while the
/// wizard renders the user's locale.
pub(crate) fn localized_package_operation_message(
    localizer: &Localizer,
    code: &rabbit_core::operation::PackageOperationMessage,
    fallback_english: &str,
) -> String {
    use rabbit_core::operation::PackageOperationMessage as Msg;
    let message = match code {
        Msg::ExtensionBinaryInstalled => {
            localizer.text("package-status-extension-binary-installed")
        }
        Msg::SkippedCurrent {
            installed_version,
            available_version,
        } => localizer.format(
            "package-status-skipped-current",
            &[
                ("installed", installed_version.as_str()),
                ("available", available_version.as_str()),
            ],
        ),
        Msg::SkippedContentUnchanged => localizer.text("package-status-skipped-content-unchanged"),
        Msg::DryRunWouldRunUnattended { artifact_kind } => localizer.format(
            "package-status-dry-run-would-run-unattended",
            &[(
                "automation",
                localized_automation_description(localizer, *artifact_kind).as_str(),
            )],
        ),
        Msg::DeferredUnattendedStaged { artifact_kind } => localizer.format(
            "package-status-deferred-unattended-staged",
            &[(
                "automation",
                localized_automation_description(localizer, *artifact_kind).as_str(),
            )],
        ),
        Msg::DeferredUnattendedNotStaged { artifact_kind } => localizer.format(
            "package-status-deferred-unattended-not-staged",
            &[(
                "automation",
                localized_automation_description(localizer, *artifact_kind).as_str(),
            )],
        ),
        Msg::UnattendedInstalled => localizer.text("package-status-unattended-installed"),
        Msg::OsaraUnattendedInstalledKeymapBackedUp => {
            localizer.text("package-status-osara-unattended-keymap-backed-up")
        }
        Msg::OsaraUnattendedInstalledKeymapReplaced => {
            localizer.text("package-status-osara-unattended-keymap-replaced")
        }
        Msg::InstallFailed { error, .. } => localizer.format(
            "package-status-install-failed",
            &[("error", error.as_str())],
        ),
        Msg::SkippedDependencyFailed { dependency } => localizer.format(
            "package-status-skipped-dependency-failed",
            &[("dependency", dependency.as_str())],
        ),
        Msg::Cancelled => localizer.text("package-status-cancelled"),
    };
    if message.missing {
        fallback_english.to_string()
    } else {
        message.value
    }
}

/// Localize the short "vendor installer" / "archive extraction" /
/// "disk image install" / "direct file install" automation-kind label
/// used inside the dry-run / deferred-unattended status messages.
fn localized_automation_description(
    localizer: &Localizer,
    kind: rabbit_core::artifact::ArtifactKind,
) -> String {
    use rabbit_core::artifact::ArtifactKind;
    let key = match kind {
        ArtifactKind::Installer => "package-automation-installer",
        // `.zip` and `.7z` end up extracted into UserPlugins by the same
        // user-facing operation; the extractor differs but the
        // user-facing description doesn't.
        ArtifactKind::Archive | ArtifactKind::SevenZipArchive => "package-automation-archive",
        ArtifactKind::DiskImage => "package-automation-disk-image",
        ArtifactKind::ExtensionBinary => "package-automation-extension-binary",
    };
    let text = localizer.text(key);
    if text.missing {
        match kind {
            ArtifactKind::Installer => "vendor installer".to_string(),
            ArtifactKind::Archive | ArtifactKind::SevenZipArchive => {
                "archive extraction".to_string()
            }
            ArtifactKind::DiskImage => "disk image install".to_string(),
            ArtifactKind::ExtensionBinary => "direct file install".to_string(),
        }
    } else {
        text.value
    }
}

/// Translate a [`rabbit_core::configuration::ConfigurationMessage`] into a
/// localized sentence using Fluent, with the same English-fallback shape
/// as [`localized_package_operation_message`].
pub(crate) fn localized_configuration_message(
    localizer: &Localizer,
    code: &rabbit_core::configuration::ConfigurationMessage,
    fallback_english: &str,
) -> String {
    use rabbit_core::configuration::ConfigurationMessage as Msg;
    let message = match code {
        Msg::ReapackRemoteAlreadyPresent { name, url } => localizer.format(
            "config-message-reapack-remote-already-present",
            &[("name", name.as_str()), ("url", url.as_str())],
        ),
        Msg::ReapackRemoteAdded { name, url } => localizer.format(
            "config-message-reapack-remote-added",
            &[("name", name.as_str()), ("url", url.as_str())],
        ),
        Msg::ReapackRemoteCreatedFile { name, url } => localizer.format(
            "config-message-reapack-remote-created-file",
            &[("name", name.as_str()), ("url", url.as_str())],
        ),
        Msg::ReapackRemoteDryRun { name, url } => localizer.format(
            "config-message-reapack-remote-dry-run",
            &[("name", name.as_str()), ("url", url.as_str())],
        ),
        Msg::ReaperLanguageAlreadySelected { file_name } => localizer.format(
            "config-message-reaper-language-already-selected",
            &[("file", file_name.as_str())],
        ),
        Msg::ReaperLanguageSelected { file_name } => localizer.format(
            "config-message-reaper-language-selected",
            &[("file", file_name.as_str())],
        ),
        Msg::ReaperLanguageDryRun { file_name } => localizer.format(
            "config-message-reaper-language-dry-run",
            &[("file", file_name.as_str())],
        ),
        Msg::Skipped { step_id } => {
            localizer.format("config-message-skipped", &[("step", step_id.as_str())])
        }
        Msg::SkippedDependencyMissing { step_id, dep_id } => localizer.format(
            "config-message-skipped-dependency-missing",
            &[("step", step_id.as_str()), ("dependency", dep_id.as_str())],
        ),
        Msg::AppliedNoOp => localizer.text("config-message-applied-no-op"),
        Msg::Cancelled { .. } => localizer.text("config-message-cancelled"),
    };
    if message.missing {
        fallback_english.to_string()
    } else {
        message.value
    }
}

/// Look up a configuration step's localized display name from the
/// builtin manifest. Falls back to the raw step id when the step
/// isn't in the manifest (forward-compat for unknown ids loaded from
/// an older receipt).
pub(crate) fn localized_configuration_step_name(
    localizer: Option<&Localizer>,
    step_id: &str,
) -> String {
    let steps = rabbit_core::configuration::builtin_configuration_steps();
    let display_key = steps
        .iter()
        .find(|step| step.id == step_id)
        .map(|step| step.display_name_key.clone());
    match (localizer, display_key) {
        (Some(localizer), Some(key)) => {
            let text = localizer.text(&key);
            if text.missing {
                step_id.to_string()
            } else {
                text.value
            }
        }
        _ => step_id.to_string(),
    }
}

/// Localize a [`rabbit_core::configuration::ConfigurationStatus`] for
/// the summary's "  Status: …" sub-line. Mirrors
/// [`status_label_for_summary`] for `PackageOperationStatus`.
pub(crate) fn configuration_status_label_for_summary(
    localizer: Option<&Localizer>,
    status: rabbit_core::configuration::ConfigurationStatus,
) -> String {
    use rabbit_core::configuration::ConfigurationStatus;
    let (id, fallback) = match status {
        ConfigurationStatus::Applied => ("config-status-applied", "Applied"),
        ConfigurationStatus::Skipped => ("config-status-skipped", "Skipped"),
        ConfigurationStatus::SkippedDependencyMissing => (
            "config-status-skipped-dependency-missing",
            "Skipped (dependency missing)",
        ),
        ConfigurationStatus::DryRun => ("config-status-dry-run", "Dry run"),
        ConfigurationStatus::Cancelled => ("config-status-cancelled", "Did not run (you stopped)"),
    };
    localizer
        .map(|localizer| localizer.text(id).value)
        .unwrap_or_else(|| fallback.to_string())
}

/// Format the wizard's detected architecture as a stable short token used in
/// the summary report. Not localized — these are the same identifiers the
/// CLI's `rabbit detect` output uses, so external tooling can grep for them.
pub(crate) fn architecture_label_for_summary(architecture: Architecture) -> String {
    match architecture {
        Architecture::X86 => "x86".to_string(),
        Architecture::X64 => "x64".to_string(),
        Architecture::Arm64 => "arm64".to_string(),
        Architecture::Arm64Ec => "arm64ec".to_string(),
        Architecture::Universal => "universal".to_string(),
        Architecture::Unknown => "unknown".to_string(),
    }
}

pub(crate) fn version_text(
    localizer: &Localizer,
    version: Option<&rabbit_core::version::Version>,
) -> String {
    version
        .map(ToString::to_string)
        .unwrap_or_else(|| localizer.text("detect-version-unknown").value)
}

pub(crate) fn unknown_version_text(model: &WizardModel) -> String {
    localizer_from_options(&model.bootstrap_options)
        .map(|localizer| localizer.text("detect-version-unknown").value)
        .unwrap_or_else(|_| "Version unknown".to_string())
}

pub(crate) fn action_label(localizer: &Localizer, action: PlanActionKind) -> String {
    let key = match action {
        PlanActionKind::Install => "action-install",
        PlanActionKind::Update => "action-update",
        PlanActionKind::Keep => "action-keep",
    };
    localizer.text(key).value
}

pub(crate) fn yes_no(localizer: &Localizer, value: bool) -> String {
    if value {
        localizer.text("common-yes").value
    } else {
        localizer.text("common-no").value
    }
}
