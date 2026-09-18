//! Runs the install the wizard assembled and reports progress back.

use rabbit_core::Result;
use rabbit_core::cancel::CancelToken;
use rabbit_core::setup::{SetupOptions, SetupReport};

use super::model::{OsaraKeymapChoice, WizardInstallRequest};

pub fn execute_wizard_install(request: WizardInstallRequest) -> Result<SetupReport> {
    execute_wizard_install_with_progress(
        request,
        &rabbit_core::progress::ProgressReporter::noop(),
        &CancelToken::new(),
    )
}

/// Like [`execute_wizard_install`] but threads a [`ProgressReporter`]
/// through to the core setup pipeline so the wizard's progress page can
/// render a live status bar. The plain [`execute_wizard_install`]
/// delegates here with a [`ProgressReporter::noop`].
///
/// `cancel` travels the other way: the wizard flips it when the user asks
/// to stop, and the pipeline reads it at each package boundary.
///
/// [`ProgressReporter`]: rabbit_core::progress::ProgressReporter
/// [`ProgressReporter::noop`]: rabbit_core::progress::ProgressReporter::noop
pub fn execute_wizard_install_with_progress(
    request: WizardInstallRequest,
    progress: &rabbit_core::progress::ProgressReporter,
    cancel: &CancelToken,
) -> Result<SetupReport> {
    // Before any download, make Windows Defender ignore RABBIT's own cache
    // folder so a freshly built, low-prevalence (but signed) installer —
    // OSARA's snapshots especially — isn't quarantined as a false positive
    // mid-download. Interactive GUI path only (the CLI stays non-interactive
    // for unattended/CI use and never touches Defender); skipped on dry runs
    // and off Windows. Best-effort: whatever the outcome, the install
    // proceeds — a real block still surfaces the actionable guidance.
    if !request.dry_run {
        let outcome =
            rabbit_core::antivirus::ensure_cache_excluded_from_antivirus(&request.cache_dir);
        if !matches!(
            outcome,
            rabbit_core::antivirus::DefenderExclusionOutcome::Unsupported
        ) {
            eprintln!("antivirus cache exclusion: {outcome:?}");
        }
    }
    let report = rabbit_core::setup::execute_setup_operation_with_progress(
        &request.resource_path,
        &request.package_ids,
        request.platform,
        request.architecture,
        &request.cache_dir,
        &SetupOptions {
            dry_run: request.dry_run,
            portable: request.portable,
            allow_reaper_running: request.allow_reaper_running,
            stage_unsupported: request.stage_unsupported,
            replace_osara_keymap: matches!(
                request.osara_keymap_choice,
                OsaraKeymapChoice::ReplaceCurrent
            ),
            target_app_path: request.target_app_path.clone(),
            lock_path: None,
            force_reinstall_packages: request.force_reinstall_packages.clone(),
            package_variants: request.package_variants.clone(),
            reaper_language_package: request.reaper_language_package.clone(),
            configuration_step_ids: request.configuration_step_ids.clone(),
        },
        progress,
        cancel,
    )?;

    // Remember what the user decided about the packages that remember it,
    // so a default RABBIT picked for them — a language pack matching
    // RABBIT's own language — does not come back ticked on the next launch.
    // The verdict is worked out where the rows are (see
    // `install_request_from_target_and_rows`), because only there can a
    // deliberate "no" be told apart from a row that had nothing to offer.
    // The CLI records nothing: `--package` is a scope for one run, not a
    // standing preference.
    let has_verdict =
        !(request.declined_packages.is_empty() && request.accepted_packages.is_empty());
    if !request.dry_run && has_verdict {
        let recorded = rabbit_core::receipt::record_package_opt_outs(
            &request.resource_path,
            &request.declined_packages,
            &request.accepted_packages,
        );
        // A preference is never worth failing a finished install over.
        if let Err(error) = recorded {
            eprintln!("could not record package opt-outs: {error}");
        }
    }

    Ok(report)
}
