//! The packages page and its two tabs. Windows drives a native `TreeCtrl`
//! with real checkboxes; everywhere else a `DataViewCtrl` with a toggle
//! column. Each tab's list has its own module per platform.

use std::cell::RefCell;
use std::rc::Rc;

use wxdragon::prelude::*;
#[cfg(target_os = "windows")]
use wxdragon::widgets::treectrl::TreeCtrl;

use crate::wx_app::pages::add_label;
use crate::wx_app::widgets::sync_uninstall_settings_widgets;
use crate::{WizardModel, WizardUninstallList};

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

#[cfg(target_os = "windows")]
use native::PackageItems;
#[cfg(target_os = "windows")]
pub(crate) use native::*;

#[cfg(not(target_os = "windows"))]
pub(crate) use dataview::*;

#[cfg(not(target_os = "windows"))]
pub(crate) use uninstall_dataview::{
    UninstallStateCell, UninstallView, build_uninstall_tab, new_uninstall_state,
    rebuild_uninstall_list_widgets,
};
#[cfg(target_os = "windows")]
pub(crate) use uninstall_native::{
    UninstallStateCell, UninstallView, build_uninstall_tab, new_uninstall_state,
    rebuild_uninstall_list_widgets,
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
    /// Bring the settings option in line with `list`'s ticks.
    pub(crate) fn sync_settings(&self, list: &WizardUninstallList) {
        sync_uninstall_settings_widgets(list, &self.remove_settings, &self.settings_note);
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
