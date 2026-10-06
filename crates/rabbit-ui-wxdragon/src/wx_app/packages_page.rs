//! The packages page. Windows drives a native `TreeCtrl` with real
//! checkboxes; everywhere else a `DataViewCtrl` with a toggle column.

use std::cell::RefCell;
use std::rc::Rc;

#[cfg(target_os = "windows")]
use wxdragon::widgets::treectrl::TreeCtrl;

#[cfg(not(target_os = "windows"))]
use wxdragon::widgets::dataview::{CustomDataViewTreeModel, DataViewCtrl};

#[cfg(target_os = "windows")]
mod native;
#[cfg(target_os = "windows")]
pub(crate) mod native_tree_checkboxes;

#[cfg(not(target_os = "windows"))]
mod dataview;

#[cfg(target_os = "windows")]
use native::PackageItems;
#[cfg(target_os = "windows")]
pub(crate) use native::*;

#[cfg(not(target_os = "windows"))]
pub(crate) use dataview::*;

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
