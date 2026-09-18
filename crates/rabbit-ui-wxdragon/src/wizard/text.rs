//! Localized labels for every wizard page, including the wx mnemonic
//! (`&`-prefixed access key) encoding native buttons expect.

use rabbit_core::localization::Localizer;

use super::model::{WizardStep, WizardStepLabel, WizardText};
use super::packages::VARIANT_CHOICE_IDS;

pub(crate) fn wizard_text(localizer: &Localizer) -> WizardText {
    WizardText {
        common_yes: localizer.text("common-yes").value,
        common_no: localizer.text("common-no").value,
        target_heading: localizer.text("wizard-target-heading").value,
        target_language_label: localizer.text("wizard-target-language-label").value,
        target_language_restart_note: localizer.text("wizard-target-language-restart-note").value,
        target_choice_label: localizer.text("wizard-target-choice-label").value,
        target_details_label: localizer.text("wizard-target-details-label").value,
        target_empty: localizer.text("wizard-target-empty").value,
        target_portable_choice: localizer.text("wizard-target-portable-choice").value,
        target_portable_folder_label: localizer.text("wizard-target-portable-folder-label").value,
        target_portable_folder_message: localizer
            .text("wizard-target-portable-folder-message")
            .value,
        target_portable_folder_browse_label: localizer
            .text("wizard-target-portable-folder-browse-label")
            .value,
        target_portable_pending_details: localizer
            .text("wizard-target-portable-pending-details")
            .value,
        target_custom_portable_label: localizer.text("wizard-target-custom-portable-label").value,
        target_custom_portable_app_path_label: localizer
            .text("wizard-target-custom-portable-app-path-label")
            .value,
        target_custom_portable_path_label: localizer
            .text("wizard-target-custom-portable-path-label")
            .value,
        target_custom_portable_version_label: localizer
            .text("wizard-target-custom-portable-version-label")
            .value,
        target_custom_portable_writable_label: localizer
            .text("wizard-target-custom-portable-writable-label")
            .value,
        target_custom_portable_note: localizer.text("wizard-target-custom-portable-note").value,
        packages_heading: localizer.text("wizard-packages-heading").value,
        packages_list_label: localizer.text("wizard-packages-list-label").value,
        packages_tree_group_label: localizer.text("wizard-packages-tree-group-label").value,
        additional_software_tree_group_label: localizer
            .text("wizard-additional-software-tree-group-label")
            .value,
        language_tree_group_label: localizer.text("wizard-language-tree-group-label").value,
        configuration_tree_group_label: localizer
            .text("wizard-configuration-tree-group-label")
            .value,
        reapack_ack_heading: localizer.text("wizard-reapack-ack-heading").value,
        reapack_ack_body: localizer.text("wizard-reapack-ack-body").value,
        reapack_ack_link_label: localizer.text("wizard-reapack-ack-link-label").value,
        reapack_ack_confirm_label: localizer.text("wizard-reapack-ack-confirm-label").value,
        version_check_heading: localizer.text("wizard-version-check-heading").value,
        version_check_status_pending: localizer.text("wizard-version-check-status-pending").value,
        version_check_progress_label: localizer.text("wizard-version-check-progress-label").value,
        version_check_error_heading: localizer.text("wizard-version-check-error-heading").value,
        package_details_label: localizer.text("wizard-package-details-label").value,
        packages_osara_keymap_heading: localizer.text("wizard-packages-osara-keymap-heading").value,
        packages_osara_keymap_replace_label: localizer
            .text("wizard-packages-osara-keymap-replace-label")
            .value,
        packages_reaper_language_label: localizer.text("packages-reaper-language-label").value,
        packages_spanish_variant_label: localizer.text("packages-spanish-variant-label").value,
        packages_spanish_variant_options: VARIANT_CHOICE_IDS
            .iter()
            .map(|id| {
                localizer
                    .text(&format!("package-langpack-es-variant-{id}"))
                    .value
            })
            .collect(),
        packages_osara_keymap_unavailable_note: localizer
            .text("wizard-packages-osara-keymap-unavailable-note")
            .value,
        packages_osara_keymap_preserve_note: localizer
            .text("wizard-packages-osara-keymap-preserve-note")
            .value,
        packages_osara_keymap_replace_note: localizer
            .text("wizard-packages-osara-keymap-replace-note")
            .value,
        package_details_handling_prefix: localizer
            .text("wizard-package-details-handling-prefix")
            .value,
        package_handling_automatic: localizer.text("wizard-package-handling-automatic").value,
        package_handling_unattended: localizer.text("wizard-package-handling-unattended").value,
        package_handling_planned: localizer.text("wizard-package-handling-planned").value,
        package_handling_manual: localizer.text("wizard-package-handling-manual").value,
        package_handling_unavailable: localizer.text("wizard-package-handling-unavailable").value,
        review_heading: localizer.text("wizard-review-heading").value,
        review_target_prefix: localizer.text("wizard-review-target-prefix").value,
        review_package_heading: localizer.text("wizard-review-package-heading").value,
        review_osara_keymap_heading: localizer.text("wizard-review-osara-keymap-heading").value,
        review_osara_keymap_preserve: localizer.text("wizard-review-osara-keymap-preserve").value,
        review_osara_keymap_replace: localizer.text("wizard-review-osara-keymap-replace").value,
        review_notes_heading: localizer.text("wizard-review-notes-heading").value,
        review_preflight_prefix: localizer.text("wizard-review-preflight-prefix").value,
        review_no_target: localizer.text("wizard-review-no-target").value,
        review_no_package: localizer.text("wizard-review-no-package").value,
        progress_heading: localizer.text("wizard-progress-heading").value,
        progress_status: localizer.text("wizard-progress-status-idle").value,
        done_heading: localizer.text("wizard-done-heading").value,
        done_status: localizer.text("wizard-done-status-idle").value,
        progress_status_running: localizer.text("wizard-progress-status-running").value,
        progress_status_cancelling: localizer.text("wizard-progress-status-cancelling").value,
        progress_details_label: localizer.text("wizard-progress-details-label").value,
        progress_details_idle: localizer.text("wizard-progress-details-idle").value,
        progress_details_starting: localizer.text("wizard-progress-details-starting").value,
        progress_details_cache_prefix: localizer.text("wizard-progress-details-cache-prefix").value,
        done_status_success: localizer.text("wizard-done-status-success").value,
        done_status_completed_with_errors: localizer
            .text("wizard-done-status-completed-with-errors")
            .value,
        done_status_error: localizer.text("wizard-done-status-error").value,
        done_status_cancelled: localizer.text("wizard-done-status-cancelled").value,
        done_status_no_packages: localizer.text("wizard-done-status-no-packages").value,
        done_show_details_label: localizer.text("wizard-done-show-details").value,
        done_launch_reaper_label: localized_wx_mnemonic_label(
            localizer,
            "wizard-done-launch-reaper",
            "wizard-done-launch-reaper-mnemonic",
        ),
        done_open_resource_label: localized_wx_mnemonic_label(
            localizer,
            "wizard-done-open-resource",
            "wizard-done-open-resource-mnemonic",
        ),
        done_no_reaper_app: localizer.text("wizard-done-no-reaper-app").value,
        done_launch_reaper_error_prefix: localizer
            .text("wizard-done-launch-reaper-error-prefix")
            .value,
        done_open_resource_error_prefix: localizer
            .text("wizard-done-open-resource-error-prefix")
            .value,
        done_self_update_apply_running: localizer
            .text("wizard-done-self-update-apply-running")
            .value,
        done_self_update_error_prefix: localizer.text("wizard-done-self-update-error-prefix").value,
        done_self_update_relaunch_prefix: localizer
            .text("wizard-done-self-update-relaunch-prefix")
            .value,
        self_update_status_checking: localizer.text("wizard-self-update-status-checking").value,
        close_during_install_title: localizer.text("wizard-close-during-install-title").value,
        close_during_install_body: localizer.text("wizard-close-during-install-body").value,
        close_during_self_update_title: localizer
            .text("wizard-close-during-self-update-title")
            .value,
        close_during_self_update_body: localizer.text("wizard-close-during-self-update-body").value,
    }
}

pub(crate) fn localized_wx_mnemonic_label(
    localizer: &Localizer,
    label_id: &str,
    mnemonic_id: &str,
) -> String {
    let label = localizer.text(label_id).value;
    // wxWidgets' OSX backend binds button-label `&` mnemonics as Cmd+letter
    // accelerators, which collides with macOS system shortcuts (Cmd+C
    // copy, Cmd+S save, Cmd+I info, …) and would, e.g., trigger the
    // Close button when the user just wanted to copy. Apple's HIG also
    // doesn't use mnemonics on buttons, so dropping them on macOS is the
    // platform-appropriate behavior in addition to fixing the collision.
    // Other platforms keep the underlined mnemonic + Alt/Option key.
    if cfg!(target_os = "macos") {
        return escape_wx_label(&label);
    }
    wx_mnemonic_label(&label, &localizer.text(mnemonic_id).value)
}

pub(crate) fn wx_mnemonic_label(label: &str, mnemonic: &str) -> String {
    let Some(key) = mnemonic.trim().chars().next() else {
        return escape_wx_label(label);
    };

    let mut output = String::new();
    let mut inserted = false;
    for label_char in label.chars() {
        if !inserted && mnemonic_matches(label_char, key) {
            output.push('&');
            inserted = true;
        }
        push_escaped_wx_label_char(&mut output, label_char);
    }

    if !inserted {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push('(');
        output.push('&');
        push_escaped_wx_label_char(&mut output, key);
        output.push(')');
    }

    output
}

fn escape_wx_label(label: &str) -> String {
    let mut output = String::new();
    for label_char in label.chars() {
        push_escaped_wx_label_char(&mut output, label_char);
    }
    output
}

fn push_escaped_wx_label_char(output: &mut String, label_char: char) {
    if label_char == '&' {
        output.push_str("&&");
    } else {
        output.push(label_char);
    }
}

fn mnemonic_matches(label_char: char, mnemonic: char) -> bool {
    label_char == mnemonic
        || label_char.eq_ignore_ascii_case(&mnemonic)
        || label_char.to_lowercase().to_string() == mnemonic.to_lowercase().to_string()
}

pub(crate) fn wizard_steps(localizer: &Localizer) -> Vec<WizardStepLabel> {
    [
        (WizardStep::Target, "wizard-step-target"),
        (WizardStep::VersionCheck, "wizard-step-version-check"),
        (WizardStep::Packages, "wizard-step-packages"),
        (
            WizardStep::ReapackAcknowledgement,
            "wizard-step-reapack-acknowledgement",
        ),
        (WizardStep::Review, "wizard-step-review"),
        (WizardStep::Progress, "wizard-step-progress"),
        (WizardStep::Done, "wizard-step-done"),
    ]
    .into_iter()
    .map(|(step, key)| WizardStepLabel {
        step,
        label: localizer.text(key).value,
    })
    .collect()
}
