//! The widget handles `run` passes around, plus the small readers and
//! updaters the event callbacks use to keep them in sync.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::{
    OsaraKeymapChoice, PackageRow, TargetRow, WizardModel, WizardOutcomeReport,
    custom_portable_target_row, osara_keymap_note, osara_selected_for_rows, refreshed_target_row,
};
use wxdragon::widgets::SimpleBook;

use wxdragon::prelude::*;

use crate::wx_app::packages_page::{PackagesView, WXK_NUMPAD_ENTER, WXK_RETURN};
use crate::wx_app::{
    DONE_STEP, PACKAGES_STEP, PROGRESS_STEP, REAPACK_ACK_STEP, REVIEW_STEP, TARGET_STEP,
};

#[derive(Clone, Copy)]
pub(crate) struct WizardWidgets {
    pub(crate) target_choice: Choice,
    pub(crate) portable_folder: TextCtrl,
    pub(crate) target_details: TextCtrl,
    pub(crate) version_check_status: StaticText,
    pub(crate) version_check_gauge: Gauge,
    pub(crate) version_check_error_heading: StaticText,
    pub(crate) version_check_error_log: TextCtrl,
    pub(crate) package_checklist: PackagesView,
    pub(crate) package_details: TextCtrl,
    pub(crate) osara_keymap_replace: CheckBox,
    pub(crate) osara_keymap_note: TextCtrl,
    /// Which installed language pack REAPER should use. Enabled only while
    /// at least one language pack is selected; several can be installed
    /// side by side, but only one is active.
    pub(crate) reaper_language_choice: Choice,
    /// Choice between the two Spanish OSARA translations. Enabled only
    /// while the Spanish language pack is selected.
    pub(crate) spanish_variant_choice: Choice,
    pub(crate) reapack_ack_confirm: CheckBox,
    pub(crate) review_text: TextCtrl,
    pub(crate) progress_status: StaticText,
    pub(crate) progress_gauge: Gauge,
    pub(crate) progress_details: TextCtrl,
    pub(crate) done_status: TextCtrl,
    pub(crate) done_details: TextCtrl,
    pub(crate) done_launch_reaper: Button,
    pub(crate) done_open_resource: Button,
    pub(crate) self_update_status: StatusBar,
    /// Child Panel hosting the language picker + restart-note label,
    /// rendered below the wizard buttons. Hidden on every step except
    /// `TARGET_STEP` because switching languages relaunches RABBIT, so the
    /// dropdown is only useful before the user has invested any wizard
    /// progress.
    pub(crate) language_footer: Panel,
}

pub(crate) fn selected_target_details(
    model: &WizardModel,
    choice: &Choice,
    portable_folder: &TextCtrl,
) -> String {
    match choice.get_selection().map(|index| index as usize) {
        Some(index) if index == portable_choice_index(model) => {
            portable_target_details(model, portable_folder)
        }
        Some(index) => target_details_for_index(model, index),
        None => model.text.target_empty.clone(),
    }
}

pub(crate) fn target_details_for_index(model: &WizardModel, index: usize) -> String {
    model
        .target_rows
        .get(index)
        .map(|row| refreshed_target_row(model, row).details)
        .unwrap_or_else(|| model.text.target_empty.clone())
}

pub(crate) fn package_details(row: &crate::PackageRow) -> String {
    row.details.clone()
}

pub(crate) fn progress_details_for_start(
    model: &WizardModel,
    target: Option<&TargetRow>,
    selected_package_indices: &[usize],
    package_rows: &[crate::PackageRow],
    osara_keymap_choice: OsaraKeymapChoice,
    cache_dir: Option<&Path>,
) -> String {
    let mut lines = vec![model.text.progress_details_starting.clone()];
    if let Some(target) = target {
        lines.push(format!(
            "{}: {}",
            model.text.review_target_prefix,
            target.path.display()
        ));
    } else {
        lines.push(model.text.review_no_target.clone());
    }

    if selected_package_indices.is_empty() {
        lines.push(model.text.review_no_package.clone());
    } else {
        for index in selected_package_indices {
            if let Some(row) = package_rows.get(*index) {
                lines.push(format!("{}: {}", row.display_name, row.action_label));
            }
        }
    }

    if osara_selected_for_rows(package_rows, selected_package_indices) {
        lines.push(model.text.review_osara_keymap_heading.clone());
        lines.push(match osara_keymap_choice {
            OsaraKeymapChoice::PreserveCurrent => model.text.review_osara_keymap_preserve.clone(),
            OsaraKeymapChoice::ReplaceCurrent => model.text.review_osara_keymap_replace.clone(),
        });
    }

    if let Some(cache_dir) = cache_dir {
        lines.push(format!(
            "{}: {}",
            model.text.progress_details_cache_prefix,
            cache_dir.display()
        ));
    }

    lines.join("\n")
}

pub(crate) fn step_status(model: &WizardModel, step: usize) -> String {
    model
        .steps
        .get(step)
        .map(|step| step.label.clone())
        .unwrap_or_else(|| model.window_title.clone())
}

pub(crate) fn selected_target_row(
    model: &WizardModel,
    widgets: &WizardWidgets,
) -> Option<TargetRow> {
    let index = widgets.target_choice.get_selection()? as usize;
    if index == portable_choice_index(model) {
        return portable_folder_path(&widgets.portable_folder)
            .map(|path| custom_portable_target_row(model, path, true));
    }
    model
        .target_rows
        .get(index)
        .map(|row| refreshed_target_row(model, row))
}

pub(crate) fn refreshed_target_index(
    model: &WizardModel,
    widgets: &WizardWidgets,
) -> Option<usize> {
    widgets.target_choice.get_selection().map(|index| {
        let index = index as usize;
        if index == portable_choice_index(model) {
            portable_choice_index(model)
        } else {
            index
        }
    })
}

pub(crate) fn refresh_target_choice(
    model: &WizardModel,
    choice: &Choice,
    selected_index: Option<usize>,
    refreshed_target: &TargetRow,
) {
    let selected_index = selected_index.unwrap_or_else(|| portable_choice_index(model));
    choice.clear();
    for (index, row) in model.target_rows.iter().enumerate() {
        if index == selected_index {
            choice.append(&refreshed_target.label);
        } else {
            choice.append(&row.label);
        }
    }
    choice.append(&model.text.target_portable_choice);
    choice.set_selection(selected_index as u32);
}

pub(crate) fn checked_package_indices(rows: &[PackageRow]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.selected)
        .map(|(index, _)| index)
        .collect()
}

pub(crate) fn osara_keymap_choice(checkbox: &CheckBox) -> OsaraKeymapChoice {
    if checkbox.get_value() {
        OsaraKeymapChoice::ReplaceCurrent
    } else {
        OsaraKeymapChoice::PreserveCurrent
    }
}

pub(crate) fn effective_can_install(
    plan_can_install: &Cell<bool>,
    review_can_install: &Cell<bool>,
) -> bool {
    plan_can_install.get() && review_can_install.get()
}

/// Enable the Spanish OSARA-translation choice only while the Spanish
/// language pack is actually selected for install — the setting has no
/// effect otherwise, and a live-but-inert control is a trap for a screen
/// reader. Mirrors how the OSARA key-map choice is gated on OSARA.
/// Refill the "REAPER language after installation" dropdown from the
/// language packs currently ticked, preserving the user's pick when it is
/// still on the list. Disabled when nothing is ticked, since there would be
/// nothing to choose. Several packs can be installed at once, so this is a
/// list rather than a consequence of which pack was chosen.
pub(crate) fn sync_reaper_language_widget(rows: &[crate::PackageRow], choice: &Choice) {
    let selected_indices = checked_package_indices(rows);
    let packs = crate::selected_language_packs(rows, &selected_indices);
    let previous = choice
        .get_selection()
        .and_then(|index| choice.get_string(index));
    choice.clear();
    for (_, display_name) in &packs {
        choice.append(display_name);
    }
    let restored = previous
        .and_then(|name| packs.iter().position(|(_, display)| *display == name))
        .unwrap_or(0);
    if !packs.is_empty() {
        choice.set_selection(restored as u32);
    }
    choice.enable(!packs.is_empty());
    choice.set_can_focus(!packs.is_empty());
    set_optional_choice_shown(choice, REAPER_LANGUAGE_LABEL_NAME, !packs.is_empty());
}

pub(crate) fn sync_spanish_variant_widget(rows: &[crate::PackageRow], choice: &Choice) {
    let selected_indices = checked_package_indices(rows);
    let selected = crate::variant_choice_package_selected(rows, &selected_indices);
    choice.enable(selected);
    choice.set_can_focus(selected);
    set_optional_choice_shown(choice, SPANISH_VARIANT_LABEL_NAME, selected);
}

/// wxWindow names of the labels sitting above the two Packages-page
/// dropdowns that only apply to some selections. `set_optional_choice_shown`
/// looks a label up by name from the dropdown's parent page, which keeps the
/// dozen call sites that already carry a dropdown handle unchanged.
pub(crate) const REAPER_LANGUAGE_LABEL_NAME: &str = "rabbit-reaper-language-label";
pub(crate) const SPANISH_VARIANT_LABEL_NAME: &str = "rabbit-spanish-variant-label";

/// Show or hide a dropdown together with its label, then re-lay out the page
/// so the row's space is reclaimed (or given back).
///
/// A dropdown that doesn't apply to the current selection used to stay on
/// the page, greyed out and — for the REAPER language — empty. Tab already
/// skipped it, but a screen reader walking the page still stopped on a dead
/// control and read out an empty combo box; hiding it removes it from the
/// accessibility tree entirely, and it comes straight back when ticking a
/// package makes the choice mean something again.
pub(crate) fn set_optional_choice_shown(choice: &Choice, label_name: &str, shown: bool) {
    if choice.is_shown() == shown {
        return;
    }
    choice.show(shown);
    let Some(page) = choice.get_parent() else {
        return;
    };
    if let Some(label) = page.find_window_by_name(label_name) {
        label.show(shown);
    }
    // The sizer reads each child's visibility when it lays the page out, so
    // the hidden row only stops taking vertical space once we ask for it.
    page.layout();
}

pub(crate) fn sync_osara_keymap_widgets(
    model: &WizardModel,
    rows: &[crate::PackageRow],
    checkbox: &CheckBox,
    note: &TextCtrl,
) {
    let selected_indices = checked_package_indices(rows);
    let osara_selected = osara_selected_for_rows(rows, &selected_indices);
    checkbox.enable(osara_selected);
    checkbox.set_can_focus(osara_selected);
    note.set_value(&osara_keymap_note(
        model,
        osara_selected,
        osara_keymap_choice(checkbox),
    ));
    note.enable(osara_selected);
    note.set_can_focus(osara_selected);
}

pub(crate) fn portable_choice_index(model: &WizardModel) -> usize {
    model.target_rows.len()
}

pub(crate) fn portable_folder_path(portable_folder: &TextCtrl) -> Option<PathBuf> {
    let path = portable_folder.get_value();
    let path = path.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

pub(crate) fn portable_target_details(model: &WizardModel, portable_folder: &TextCtrl) -> String {
    portable_folder_path(portable_folder)
        .map(|path| custom_portable_target_row(model, path, true).details)
        .unwrap_or_else(|| model.text.target_portable_pending_details.clone())
}

pub(crate) fn target_is_valid(model: &WizardModel, widgets: &WizardWidgets) -> bool {
    selected_target_row(model, widgets)
        .map(|target| target.writable)
        .unwrap_or(false)
}

/// Whether the user has checked the ReaPack-donation acknowledgement on
/// the dedicated wizard page. Used by `update_navigation` to gate the
/// Next button on REAPACK_ACK_STEP — the page never shows up in the run
/// at all when ReaPack isn't being installed/updated, so on every other
/// step this value is irrelevant.
pub(crate) fn reapack_ack_confirmed(widgets: &WizardWidgets) -> bool {
    widgets.reapack_ack_confirm.get_value()
}

pub(crate) fn bind_reapack_ack_navigation_updates(
    widgets: WizardWidgets,
    current_step: &Arc<AtomicUsize>,
    next: &Button,
) {
    let current_step = Arc::clone(current_step);
    let next = *next;
    widgets.reapack_ack_confirm.on_toggled(move |event| {
        if current_step.load(Ordering::SeqCst) == REAPACK_ACK_STEP {
            next.enable(event.is_checked());
        }
    });
}

/// A multiline `wxTextCtrl` claims Enter for itself (it reports
/// `DLGC_WANTALLKEYS` on MSW and the NSTextView swallows the key on macOS),
/// so the window's default button never sees it. The Done page deliberately
/// parks focus on the read-only summary TextCtrl so the screen reader reads
/// the outcome out loud — which would otherwise leave Enter dead on the very
/// last page even with Close as the default button. Re-route Enter from the
/// Done page's two read-only TextCtrls to the Close action.
///
/// Guarded on `DONE_STEP`: these controls only live on the Done page, but the
/// guard is what guarantees Enter can never tear the window down while the
/// install worker is still running — `DONE_STEP` is stored only after the
/// worker finished, or before it is even spawned on a preparation error.
pub(crate) fn bind_done_page_enter_closes(
    text: &TextCtrl,
    frame: &Frame,
    current_step: &Arc<AtomicUsize>,
) {
    let frame = *frame;
    let current_step = Arc::clone(current_step);
    text.on_key_down(move |event| {
        let key_code = if let WindowEventData::Keyboard(kbd) = &event {
            kbd.get_key_code()
        } else {
            None
        };
        if !matches!(key_code, Some(WXK_RETURN) | Some(WXK_NUMPAD_ENTER)) {
            return;
        }
        if current_step.load(Ordering::SeqCst) != DONE_STEP {
            return;
        }
        // Consume the key before closing so the native control never beeps
        // or inserts a newline. `Frame::close(true)` goes through
        // wxEVT_CLOSE_WINDOW → Destroy(), which defers via wxPendingDelete,
        // so tearing the window down from inside a key handler is safe.
        event.skip(false);
        frame.close(true);
    });
}

pub(crate) fn bind_target_navigation_updates(
    model: &Arc<WizardModel>,
    widgets: WizardWidgets,
    current_step: &Arc<AtomicUsize>,
    next: &Button,
) {
    {
        let model = Arc::clone(model);
        let current_step = Arc::clone(current_step);
        let next = *next;
        widgets.target_choice.on_selection_changed(move |_| {
            if current_step.load(Ordering::SeqCst) == TARGET_STEP {
                next.enable(target_is_valid(&model, &widgets));
            }
        });
    }
    {
        let model = Arc::clone(model);
        let current_step = Arc::clone(current_step);
        let next = *next;
        widgets.portable_folder.on_text_changed(move |_| {
            if current_step.load(Ordering::SeqCst) == TARGET_STEP {
                next.enable(target_is_valid(&model, &widgets));
            }
        });
    }
}

pub(crate) fn configure_portable_folder(
    portable_folder: &TextCtrl,
    portable_folder_browse: &Button,
    enabled: bool,
) {
    portable_folder.enable(enabled);
    portable_folder.set_can_focus(enabled);
    portable_folder_browse.enable(enabled);
    portable_folder_browse.set_can_focus(enabled);
}

pub(crate) fn set_last_report(
    state: &Arc<Mutex<Option<WizardOutcomeReport>>>,
    report: Option<WizardOutcomeReport>,
) {
    if let Ok(mut slot) = state.lock() {
        *slot = report;
    }
}

pub(crate) fn set_last_resource_path(state: &Arc<Mutex<Option<PathBuf>>>, path: Option<PathBuf>) {
    set_last_path(state, path);
}

pub(crate) fn clone_last_resource_path(state: &Arc<Mutex<Option<PathBuf>>>) -> Option<PathBuf> {
    clone_last_path(state)
}

pub(crate) fn set_last_path(state: &Arc<Mutex<Option<PathBuf>>>, path: Option<PathBuf>) {
    if let Ok(mut slot) = state.lock() {
        *slot = path;
    }
}

pub(crate) fn clone_last_path(state: &Arc<Mutex<Option<PathBuf>>>) -> Option<PathBuf> {
    state.lock().ok().and_then(|slot| slot.clone())
}

pub(crate) fn planned_reaper_launch_path_for_target(target: &TargetRow) -> PathBuf {
    target.planned_app_path.clone()
}

pub(crate) fn can_launch_reaper_path(path: Option<&Path>) -> bool {
    path.is_some_and(Path::exists)
}

pub(crate) fn can_launch_last_reaper_path(state: &Arc<Mutex<Option<PathBuf>>>) -> bool {
    can_launch_reaper_path(clone_last_path(state).as_deref())
}

pub(crate) fn append_done_status(status: &TextCtrl, message: &str) {
    let current = status.get_value();
    if current.trim().is_empty() {
        status.set_value(message);
    } else {
        status.set_value(&format!("{current}\n\n{message}"));
    }
}

#[allow(clippy::too_many_arguments)] // UI plumbing: one parameter per widget handle.
pub(crate) fn update_navigation(
    step: usize,
    book: &SimpleBook,
    step_label: &StaticText,
    labels: &[String],
    back: &Button,
    next: &Button,
    install: &Button,
    close: &Button,
    language_footer: &Panel,
    can_install: bool,
    target_valid: bool,
    reapack_ack_confirmed: bool,
) {
    book.set_selection(step);
    if let Some(label) = labels.get(step) {
        step_label.set_label(label);
    }
    back.enable(step > TARGET_STEP && step < DONE_STEP);
    next.enable(match step {
        TARGET_STEP => target_valid,
        // VERSION_CHECK_STEP auto-advances on success; never user-driven.
        PACKAGES_STEP | PROGRESS_STEP => true,
        REAPACK_ACK_STEP => reapack_ack_confirmed,
        _ => false,
    });
    install.enable(step == REVIEW_STEP && can_install);
    // Make the step's primary action the window's default button so Enter
    // activates it regardless of which control holds focus (the standard
    // dialog convention on both macOS and Windows). A disabled default
    // button is a no-op on Enter, so an invalid Target step or a Review
    // step that can't install yet won't advance — exactly what we want.
    // Install is primary on Review, Close on Done (the wizard is over there
    // and Next is disabled, which used to leave Enter a dead key). Progress
    // deliberately keeps the *disabled* Next as its default: Enter must
    // never tear the window down while the install worker is running.
    match step {
        REVIEW_STEP => install.set_default(),
        DONE_STEP => close.set_default(),
        _ => next.set_default(),
    }
    // Language picker only matters on the Target step — switching languages
    // relaunches RABBIT and discards wizard progress, so a footer on later
    // pages would just be a tripwire.
    language_footer.show(step == TARGET_STEP);
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{can_launch_reaper_path, planned_reaper_launch_path_for_target};
    use crate::TargetRow;

    #[test]
    fn launchability_requires_existing_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("reaper.exe");

        assert!(!can_launch_reaper_path(Some(&path)));

        fs::write(&path, b"stub").unwrap();

        assert!(can_launch_reaper_path(Some(&path)));
        assert!(!can_launch_reaper_path(None));
    }

    #[test]
    fn planned_launch_path_uses_target_planned_app_path() {
        let target = TargetRow {
            label: "Portable REAPER".to_string(),
            details: String::new(),
            app_path: None,
            planned_app_path: PathBuf::from("C:/PortableREAPER/reaper.exe"),
            path: PathBuf::from("C:/PortableREAPER"),
            version: None,
            portable: true,
            selected: true,
            writable: true,
            architecture: rabbit_core::model::Architecture::current(),
        };

        assert_eq!(
            planned_reaper_launch_path_for_target(&target),
            PathBuf::from("C:/PortableREAPER/reaper.exe")
        );
    }
}
