//! Review page text: the summary lines and manual-step notes shown before
//! the install starts.

use rabbit_core::artifact::expected_artifact_kind;
use rabbit_core::localization::Localizer;
use rabbit_core::operation::preview_manual_instruction;
use rabbit_core::plan::PlanActionKind;
use rabbit_core::resource::{ResourceInitOptions, initialize_resource_path};
use rabbit_core::setup::setup_requires_extension_support;

use super::model::{OsaraKeymapChoice, PackageRow, TargetRow, WizardModel, WizardReviewPreview};
use super::packages::{osara_selected_for_rows, package_ids_for_rows};

pub fn review_lines_for_indices(
    model: &WizardModel,
    selected_target_index: Option<usize>,
    selected_package_indices: &[usize],
) -> Vec<String> {
    let target = selected_target_index.and_then(|index| model.target_rows.get(index));
    review_lines_for_target(model, target, selected_package_indices)
}

pub fn review_lines_for_target(
    model: &WizardModel,
    target: Option<&TargetRow>,
    selected_package_indices: &[usize],
) -> Vec<String> {
    review_lines_for_package_rows(
        model,
        target,
        selected_package_indices,
        &model.package_rows,
        &model.notes,
    )
}

pub fn review_lines_for_package_rows(
    model: &WizardModel,
    target: Option<&TargetRow>,
    selected_package_indices: &[usize],
    package_rows: &[PackageRow],
    notes: &[String],
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(target) = target {
        lines.push(format!(
            "{}: {}",
            model.text.review_target_prefix,
            target.path.display()
        ));
    } else {
        lines.push(model.text.review_no_target.clone());
    }

    let package_ids = package_ids_for_rows(package_rows, selected_package_indices);
    if package_ids.is_empty() {
        lines.push(model.text.review_no_package.clone());
    } else {
        for package_id in package_ids {
            if let Some(package) = package_rows
                .iter()
                .find(|package| package.package_id == package_id)
            {
                lines.push(format!(
                    "{}: {}",
                    package.display_name, package.action_label
                ));
            }
        }
    }

    lines.extend(notes.iter().cloned());
    lines
}

pub fn build_review_preview_for_package_rows(
    model: &WizardModel,
    target: Option<&TargetRow>,
    selected_package_indices: &[usize],
    package_rows: &[PackageRow],
    notes: &[String],
    osara_keymap_choice: OsaraKeymapChoice,
) -> WizardReviewPreview {
    let Some(target) = target else {
        return WizardReviewPreview {
            lines: vec![model.text.review_no_target.clone()],
            can_install: false,
        };
    };

    let mut lines = vec![format!(
        "{}: {}",
        model.text.review_target_prefix,
        target.path.display()
    )];

    let mut can_install = !selected_package_indices.is_empty();

    // Run the resource-path preflight to surface fatal blockers (read-only
    // target, REAPER currently running, etc.) — those still need to land in
    // the GUI summary so the user knows install is blocked. Successful
    // resource-init details (the long "Create directory…" / "Create file…"
    // list) are now report-only; the GUI just notes that backups will be
    // taken if needed.
    match initialize_resource_path(
        &target.path,
        &ResourceInitOptions {
            dry_run: true,
            portable: target.portable,
            include_extension_support_dirs: target.portable
                || setup_requires_extension_support(
                    &selected_package_indices
                        .iter()
                        .filter_map(|index| package_rows.get(*index))
                        .map(|package| package.package_id.clone())
                        .collect::<Vec<_>>(),
                ),
            allow_reaper_running: false,
            target_app_path: Some(target.planned_app_path.clone()),
        },
    ) {
        Ok(_) => {}
        Err(error) => {
            can_install = false;
            lines.push(format!("{}: {}", model.text.review_preflight_prefix, error));
        }
    }

    lines.push(String::new());
    lines.push(model.text.review_package_heading.clone());
    if selected_package_indices.is_empty() {
        lines.push(model.text.review_no_package.clone());
    } else {
        for index in selected_package_indices {
            if let Some(package) = package_rows.get(*index) {
                lines.push(package.summary.clone());
            }
        }
    }

    if osara_selected_for_rows(package_rows, selected_package_indices) {
        lines.push(String::new());
        lines.push(model.text.review_osara_keymap_heading.clone());
        lines.push(match osara_keymap_choice {
            OsaraKeymapChoice::PreserveCurrent => model.text.review_osara_keymap_preserve.clone(),
            OsaraKeymapChoice::ReplaceCurrent => model.text.review_osara_keymap_replace.clone(),
        });
    }

    if !notes.is_empty() {
        lines.push(String::new());
        lines.push(model.text.review_notes_heading.clone());
        lines.extend(notes.iter().cloned());
    }

    WizardReviewPreview { lines, can_install }
}

pub fn package_requires_manual_attention(
    _model: &WizardModel,
    package: &PackageRow,
    _osara_keymap_choice: OsaraKeymapChoice,
) -> bool {
    matches!(
        package.action,
        PlanActionKind::Install | PlanActionKind::Update
    ) && package.manual_attention_expected
}

pub fn manual_attention_handling_summary(
    _model: &WizardModel,
    package: &PackageRow,
    _osara_keymap_choice: OsaraKeymapChoice,
) -> String {
    package.handling_summary.clone()
}

pub fn preview_manual_instruction_lines(
    model: &WizardModel,
    target: &TargetRow,
    package: &PackageRow,
    osara_keymap_choice: OsaraKeymapChoice,
) -> Vec<String> {
    let Ok(kind) = expected_artifact_kind(&package.package_id, model.platform, model.architecture)
    else {
        return Vec::new();
    };
    let instruction = preview_manual_instruction(
        &package.package_id,
        kind,
        &target.path,
        Some(&target.planned_app_path),
        matches!(osara_keymap_choice, OsaraKeymapChoice::ReplaceCurrent),
    );
    let mut lines = instruction
        .steps
        .into_iter()
        .map(|step| format!("  {step}"))
        .collect::<Vec<_>>();
    lines.extend(
        instruction
            .notes
            .into_iter()
            .map(|note| format!("  Note: {note}")),
    );
    lines
}

pub(crate) fn review_lines(
    localizer: &Localizer,
    target_rows: &[TargetRow],
    package_rows: &[PackageRow],
    notes: &[String],
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(target) = target_rows.iter().find(|target| target.selected) {
        lines.push(
            localizer
                .format(
                    "wizard-review-target",
                    &[("path", &target.path.display().to_string())],
                )
                .value,
        );
    } else {
        lines.push(localizer.text("wizard-review-no-target").value);
    }

    for package in package_rows {
        lines.push(
            localizer
                .format(
                    "wizard-review-package",
                    &[
                        ("package", package.display_name.as_str()),
                        ("action", package.action_label.as_str()),
                    ],
                )
                .value,
        );
    }

    lines.extend(notes.iter().cloned());
    lines
}
