//! Self-update check and apply, plus the summaries the wizard shows for
//! both.

use rabbit_core::localization::Localizer;
use rabbit_core::model::Platform;
use rabbit_core::self_update::{
    ApplySelfUpdateOptions, DEFAULT_SELF_UPDATE_MANIFEST_URL, SelfUpdateApplyReport,
    SelfUpdateCheckReport, apply_self_update, check_self_update, default_self_update_staging_dir,
    relaunch_current_executable, resolve_self_update_release_notes,
    stage_self_update_with_progress,
};
use rabbit_core::{RabbitError, Result};

pub fn run_wizard_self_update_check() -> Result<SelfUpdateCheckReport> {
    let platform = Platform::current().ok_or(RabbitError::UnsupportedPlatform)?;
    check_self_update(platform, DEFAULT_SELF_UPDATE_MANIFEST_URL)
}

/// What's-New notes for a pending RABBIT update, covering every release
/// between the running version and the latest one.
///
/// Called from the same startup worker thread as
/// [`run_wizard_self_update_check`], never from the UI thread: it performs an
/// HTTP round-trip, and the wizard must stay responsive while it runs. The
/// result is best-effort — `None` simply means the prompt falls back to its
/// plain version-to-version wording.
pub fn run_wizard_self_update_release_notes(report: &SelfUpdateCheckReport) -> Option<String> {
    resolve_self_update_release_notes(report)
}

/// Download-then-install for RABBIT itself, reporting both phases through
/// `progress`.
///
/// Runs on a worker thread, so `progress` is invoked off the UI thread and
/// the caller is responsible for forwarding events onto it — the same
/// contract the package install pipeline has. Pass
/// `ProgressReporter::noop()` to run it silently.
///
/// [`ProgressReporter`]: rabbit_core::progress::ProgressReporter
pub fn run_wizard_self_update_apply(
    progress: &rabbit_core::progress::ProgressReporter,
) -> Result<SelfUpdateApplyReport> {
    let platform = Platform::current().ok_or(RabbitError::UnsupportedPlatform)?;
    let staging_dir = default_self_update_staging_dir();
    let stage = stage_self_update_with_progress(
        platform,
        DEFAULT_SELF_UPDATE_MANIFEST_URL,
        &staging_dir,
        progress,
    )?;
    apply_self_update(
        &stage,
        &ApplySelfUpdateOptions {
            install_root: None,
            install_target_basename: None,
            progress: Some(progress.clone()),
        },
    )
}

pub fn relaunch_rabbit_after_apply() -> Result<u32> {
    relaunch_current_executable()
}

pub fn format_self_update_check_summary(
    localizer: &Localizer,
    report: &SelfUpdateCheckReport,
) -> String {
    let current = report.current_version.to_string();
    let latest = report.latest_version.to_string();
    if report.update_available {
        localizer
            .format(
                "self-update-status-update-available",
                &[
                    ("current", current.as_str()),
                    ("latest", latest.as_str()),
                    ("channel", report.channel.as_str()),
                ],
            )
            .value
    } else {
        localizer
            .format(
                "self-update-status-up-to-date",
                &[
                    ("current", current.as_str()),
                    ("channel", report.channel.as_str()),
                ],
            )
            .value
    }
}

pub fn format_self_update_apply_summary(
    localizer: &Localizer,
    report: &SelfUpdateApplyReport,
) -> String {
    let version = report.stage.check.latest_version.to_string();
    if report.replaced_files.is_empty() {
        return localizer
            .format(
                "self-update-apply-no-files-replaced",
                &[("version", version.as_str())],
            )
            .value;
    }

    let count = report.replaced_files.len().to_string();
    let install_root = report.install_root.display().to_string();
    let mut summary = localizer
        .format(
            "self-update-apply-replaced-summary",
            &[
                ("count", count.as_str()),
                ("root", install_root.as_str()),
                ("version", version.as_str()),
            ],
        )
        .value;
    if let Some(signature_summary) = format_signature_verdict_summary(localizer, report) {
        summary.push(' ');
        summary.push_str(&signature_summary);
    }
    summary
}

fn format_signature_verdict_summary(
    localizer: &Localizer,
    report: &SelfUpdateApplyReport,
) -> Option<String> {
    use rabbit_core::signature::SignatureVerdict;

    if report.signature_verdicts.is_empty() {
        return None;
    }
    let mut signed = 0usize;
    let mut unsigned = 0usize;
    for record in &report.signature_verdicts {
        match record.verdict {
            SignatureVerdict::Signed { .. } => signed += 1,
            SignatureVerdict::Unsigned { .. } => unsigned += 1,
            SignatureVerdict::Invalid { .. } => {}
        }
    }
    let signed_str = signed.to_string();
    let unsigned_str = unsigned.to_string();
    let value = match (signed, unsigned) {
        (0, 0) => return None,
        (_, 0) => {
            localizer
                .format(
                    "self-update-apply-signature-summary-signed-only",
                    &[("signed", signed_str.as_str())],
                )
                .value
        }
        (0, _) => {
            localizer
                .format(
                    "self-update-apply-signature-summary-unsigned-only",
                    &[("unsigned", unsigned_str.as_str())],
                )
                .value
        }
        _ => {
            localizer
                .format(
                    "self-update-apply-signature-summary-mixed",
                    &[
                        ("signed", signed_str.as_str()),
                        ("unsigned", unsigned_str.as_str()),
                    ],
                )
                .value
        }
    };
    Some(value)
}
