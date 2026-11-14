//! The packages page and its three tabs. Windows drives a native `TreeCtrl`
//! with real checkboxes; everywhere else a `DataViewCtrl` with a toggle
//! column. Each tab's list has its own module per platform.

use std::cell::RefCell;
use std::rc::Rc;

use wxdragon::prelude::*;
#[cfg(target_os = "windows")]
use wxdragon::widgets::treectrl::TreeCtrl;

use crate::wx_app::globals::with_ui_localizer;
use crate::wx_app::pages::add_label;
use crate::wx_app::widgets::sync_uninstall_settings_widgets;
use crate::{
    BackupChoice, WizardBackupList, WizardModel, WizardUninstallList,
    uninstall_settings_availability_note,
};

#[cfg(not(target_os = "windows"))]
use wxdragon::widgets::dataview::{CustomDataViewTreeModel, DataViewCtrl};

#[cfg(target_os = "windows")]
mod native;
#[cfg(target_os = "windows")]
pub(crate) mod native_tree_checkboxes;

#[cfg(not(target_os = "windows"))]
mod dataview;

#[cfg(not(target_os = "windows"))]
mod uninstall_dataview;
#[cfg(target_os = "windows")]
mod uninstall_native;

#[cfg(not(target_os = "windows"))]
mod backups_dataview;
#[cfg(target_os = "windows")]
mod backups_native;

#[cfg(target_os = "windows")]
use native::PackageItems;
#[cfg(target_os = "windows")]
pub(crate) use native::*;

#[cfg(not(target_os = "windows"))]
pub(crate) use dataview::*;

#[cfg(not(target_os = "windows"))]
pub(crate) use uninstall_dataview::{
    UninstallStateCell, UninstallView, build_uninstall_tab, new_uninstall_state,
    rebuild_uninstall_list_widgets, reset_uninstall_list_caret_on_focus,
};
#[cfg(target_os = "windows")]
pub(crate) use uninstall_native::{
    UninstallStateCell, UninstallView, build_uninstall_tab, new_uninstall_state,
    rebuild_uninstall_list_widgets, reset_uninstall_list_caret_on_focus,
};

#[cfg(not(target_os = "windows"))]
pub(crate) use backups_dataview::{
    BackupsStateCell, BackupsView, build_backups_tab, new_backups_state,
    rebuild_backups_list_widgets, reset_backups_list_caret_on_focus,
};
#[cfg(target_os = "windows")]
pub(crate) use backups_native::{
    BackupsStateCell, BackupsView, build_backups_tab, new_backups_state,
    rebuild_backups_list_widgets, reset_backups_list_caret_on_focus,
};

/// `wx/defs.h`: `WXK_SPACE = 32` (just the ASCII value). Kept around as a
/// fallback intercept on platforms without TVS_CHECKBOXES; on Windows the
/// native tree handles Space toggles internally.
#[allow(dead_code)]
pub(crate) const WXK_SPACE: i32 = 32;

/// `wx/defs.h`: `WXK_RETURN = 13` (the ASCII value) and, from the keycode
/// enum that starts at `WXK_START = 300`, `WXK_NUMPAD_ENTER = 370`.
/// Redeclared here for the same reason as `WXK_SPACE` above.
pub(crate) const WXK_RETURN: i32 = 13;
pub(crate) const WXK_NUMPAD_ENTER: i32 = 370;

/// The widgets below the Uninstall list that follow its ticks. Both list
/// modules hand them to their toggle handling, so a tick on either platform
/// updates the same things.
#[derive(Clone, Copy)]
pub(crate) struct UninstallSideWidgets {
    pub(crate) details: TextCtrl,
    pub(crate) remove_settings: CheckBox,
    pub(crate) settings_note: TextCtrl,
}

impl UninstallSideWidgets {
    /// Bring the settings option in line with `list`'s ticks. Returns what
    /// to tell VoiceOver when the tick made the option usable or unusable,
    /// which nothing else says: it happens below the list.
    pub(crate) fn sync_settings(&self, list: &WizardUninstallList) -> Option<String> {
        // Only the sync enables or disables the checkbox, so its state is
        // what the option was before this tick.
        let was_available = self.remove_settings.is_enabled();
        sync_uninstall_settings_widgets(list, &self.remove_settings, &self.settings_note);
        let mut note = None;
        with_ui_localizer(|localizer| {
            note = uninstall_settings_availability_note(localizer, was_available, list);
        });
        note
    }
}

/// Build "Also remove settings" below the Uninstall list's details: a
/// heading, the checkbox, unticked, and a read-only note saying what it
/// would remove. Like the key map checkbox on the other tab, it is always
/// there, and disabled and out of the Tab order while it can't apply,
/// rather than hidden: a control that comes and goes is harder to find
/// again than one that says why it can't be used.
pub(crate) fn build_uninstall_settings(
    page: &Panel,
    sizer: &BoxSizer,
    model: &WizardModel,
    list: &WizardUninstallList,
) -> (CheckBox, TextCtrl) {
    add_label(
        page,
        sizer,
        &model.text.uninstall_settings_heading,
        "rabbit-uninstall-settings-heading",
    );
    let label = &model.text.uninstall_remove_settings_label;
    let remove_settings = CheckBox::builder(page).with_label(label).build();
    // Name and label both, as for the key map checkbox: on Windows the
    // accessible name follows the wx name, and the builder's label doesn't
    // reliably stick.
    remove_settings.set_name(label);
    remove_settings.set_label(label);
    remove_settings.add_style(WindowStyle::TabStop);
    remove_settings.set_value(false);
    sizer.add(&remove_settings, 0, SizerFlag::All | SizerFlag::Expand, 6);

    let note = TextCtrl::builder(page)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 80))
        .build();
    note.set_name("rabbit-uninstall-settings-note");
    sizer.add(&note, 0, SizerFlag::All | SizerFlag::Expand, 6);
    sync_uninstall_settings_widgets(list, &remove_settings, &note);
    (remove_settings, note)
}

/// The Backups tab's controls, as its builder hands them back: the label
/// right before the list, the list, the details text and the choice below
/// them.
pub(crate) struct BackupsTabWidgets {
    pub(crate) list_label: StaticText,
    pub(crate) list: BackupsView,
    pub(crate) details: TextCtrl,
    pub(crate) choice: RadioBox,
}

/// Add the label right before the Backups list. It names the tab first and
/// gives the size of every backup together. Created right before the list:
/// NVDA and JAWS read it when focus lands in the list, and wxWidgets gives
/// the macOS list its VoiceOver name from the label shown just before it.
/// Kept, unlike the other tabs' labels, because the total changes with the
/// target.
pub(crate) fn add_backups_list_label(
    page: &Panel,
    sizer: &BoxSizer,
    list: &WizardBackupList,
) -> StaticText {
    let label = StaticText::builder(page)
        .with_label(&backups_list_label(list))
        .build();
    label.set_name("rabbit-backups-list-label");
    sizer.add(
        &label,
        0,
        SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
        6,
    );
    label
}

/// The label for the Backups list as it is now.
fn backups_list_label(list: &WizardBackupList) -> String {
    let mut label = String::new();
    with_ui_localizer(|localizer| label = list.list_label(localizer));
    label
}

/// Add the read-only details text below the Backups list, with its label.
/// It follows the focused row, as on the other tabs.
pub(crate) fn add_backups_details(
    page: &Panel,
    sizer: &BoxSizer,
    model: &WizardModel,
    list: &WizardBackupList,
) -> TextCtrl {
    add_label(
        page,
        sizer,
        &model.text.package_details_label,
        "rabbit-backups-details-label",
    );
    let details = TextCtrl::builder(page)
        .with_value(&backups_initial_details(list))
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 120))
        .build();
    details.set_name("rabbit-backups-details");
    sizer.add(&details, 0, SizerFlag::All | SizerFlag::Expand, 6);
    details
}

/// The details text the Backups tab starts with.
fn backups_initial_details(list: &WizardBackupList) -> String {
    let mut details = String::new();
    with_ui_localizer(|localizer| details = list.initial_details(localizer));
    details
}

/// Add the choice below the Backups list and its details: a native
/// RadioBox, "What to do with the ticked backups", Restore or Delete, on one
/// row, with Restore chosen. The choices have no access keys; only buttons
/// carry them. It sits below the list, so Tab from the tab strip still goes
/// straight into the list, which the macOS Tab handling assumes.
pub(crate) fn add_backups_choice(page: &Panel, sizer: &BoxSizer, model: &WizardModel) -> RadioBox {
    let choices: Vec<&str> = BackupChoice::ALL
        .iter()
        .map(|choice| match choice {
            BackupChoice::Restore => model.text.backups_choice_restore.as_str(),
            BackupChoice::Delete => model.text.backups_choice_delete.as_str(),
        })
        .collect();
    let choice = RadioBox::builder(page, &choices)
        .with_label(&model.text.backups_choice_label)
        .with_major_dimension(1)
        .with_style(RadioBoxStyle::SpecifyRows)
        .build();
    choice.set_name("rabbit-backups-choice");
    choice.set_selection(BackupChoice::Restore.selection());
    sizer.add(&choice, 0, SizerFlag::All | SizerFlag::Expand, 6);
    choice
}

/// The choice the RadioBox below the Backups list shows.
pub(crate) fn selected_backup_choice(choice: &RadioBox) -> BackupChoice {
    BackupChoice::from_selection(choice.get_selection())
}

/// Bring the Backups list's label, its details and the choice below it in
/// line with fresh rows, as the version check does: the label's total, the
/// details the tab starts with, and the choice the rows are labelled for,
/// which is Restore after a reset. Each only when it changes, so screen
/// readers don't say it again.
pub(crate) fn sync_backups_side_widgets(
    label: &StaticText,
    details: &TextCtrl,
    choice: &RadioBox,
    list: &WizardBackupList,
) {
    let text = backups_list_label(list);
    if label.get_label() != text {
        label.set_label(&text);
    }
    set_text_if_changed(details, &backups_initial_details(list));
    if selected_backup_choice(choice) != list.choice {
        choice.set_selection(list.choice.selection());
    }
}

/// Give a read-only text `value`, only when it says something else: a
/// screen reader may read every change of the text it is on.
pub(crate) fn set_text_if_changed(text: &TextCtrl, value: &str) {
    if text.get_value() != value {
        text.set_value(value);
    }
}

/// Per-platform state handle that the orchestrator (run, button click
/// handlers, post-install hook, version-check dispatcher) holds onto and
/// passes through to `build_packages_page` / `refresh_package_checklist` /
/// `rebuild_package_list_widgets` without caring which widget is on the
/// page. On Windows it carries the live `TreeItemId`s for the native
/// TreeCtrl rows; elsewhere it carries the `CustomDataViewTreeModel`
/// handle so the refresh helpers can re-emit notifications and rebuild
/// the model's userdata in place.
#[cfg(target_os = "windows")]
pub(crate) type PackagesStateCell = Rc<RefCell<PackageItems>>;
#[cfg(not(target_os = "windows"))]
pub(crate) type PackagesStateCell = Rc<RefCell<Option<CustomDataViewTreeModel>>>;

/// Type alias used by `WizardWidgets` for the package list widget itself.
/// Windows: native `wxTreeCtrl` (`SysTreeView32` underneath, with
/// `TVS_CHECKBOXES` enabled by `native_tree_checkboxes::enable_checkboxes`).
/// Non-Windows: `wxDataViewCtrl` driven by a `CustomDataViewTreeModel`
/// with a `DataViewToggleRenderer` for the checkbox column.
#[cfg(target_os = "windows")]
pub(crate) type PackagesView = TreeCtrl;
#[cfg(not(target_os = "windows"))]
pub(crate) type PackagesView = DataViewCtrl;

/// Build the empty per-platform state container that lives for the
/// lifetime of the wizard. On Windows it starts with no leaf TreeItemIds
/// (populated during `build_packages_page`); on non-Windows it starts
/// with `None` for the model handle (populated immediately after the
/// model is constructed in `build_packages_page`).
pub(crate) fn new_packages_state() -> PackagesStateCell {
    #[cfg(target_os = "windows")]
    {
        Rc::new(RefCell::new(PackageItems::empty()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Rc::new(RefCell::new(None))
    }
}
