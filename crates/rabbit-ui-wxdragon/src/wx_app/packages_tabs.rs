//! The Packages page's two tabs, Install or update and Uninstall: building
//! the notebook, where focus goes when the page opens, which run Next
//! starts, and switching tabs from the keyboard on macOS.
//!
//! The notebook is a native tab control, SysTabControl32 on Windows and
//! NSTabView on macOS, so screen readers present it as tabs. Its selection is
//! changed from code in one place only, on arrival from the version check,
//! once the page shows: wxOSX moves focus into a tab's page on every
//! selection change, even on a wizard page that is hidden.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use wxdragon::prelude::*;

use crate::{WizardMode, WizardModel, WizardUninstallList};

use crate::wx_app::PACKAGES_STEP;
use crate::wx_app::globals::{AnnouncePriority, announce};
use crate::wx_app::packages_page::{
    PackagesStateCell, PackagesView, UninstallStateCell, UninstallView, build_packages_page,
    build_uninstall_tab, focus_packages_list_top, reset_packages_list_caret_on_focus,
};
use crate::wx_app::pages::{WizardPage, add_heading};
use crate::wx_app::widgets::{INSTALL_TAB_NAME, UNINSTALL_TAB_NAME, WizardWidgets};

pub(crate) const INSTALL_TAB: usize = crate::PACKAGES_INSTALL_TAB;
pub(crate) const UNINSTALL_TAB: usize = crate::PACKAGES_UNINSTALL_TAB;

/// `wx/defs.h`: `WXK_TAB = 9`, the ASCII value.
const WXK_TAB: i32 = 9;

/// The widgets the Packages page's builder hands back, in the order
/// `WizardWidgets` lists them.
pub(crate) struct PackagesPageWidgets {
    pub(crate) notebook: Notebook,
    pub(crate) package_checklist: PackagesView,
    pub(crate) package_details: TextCtrl,
    pub(crate) osara_keymap_replace: CheckBox,
    pub(crate) osara_keymap_note: TextCtrl,
    pub(crate) spanish_variant_choice: Choice,
    pub(crate) reaper_language_choice: Choice,
    pub(crate) uninstall_list: UninstallView,
    pub(crate) uninstall_details: TextCtrl,
    pub(crate) uninstall_remove_settings: CheckBox,
    pub(crate) uninstall_settings_note: TextCtrl,
}

/// Everything the Packages page's lists share with the rest of the wizard.
pub(crate) struct PackagesPageState {
    pub(crate) package_rows: Rc<RefCell<Vec<crate::PackageRow>>>,
    pub(crate) configuration_rows: Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    pub(crate) package_items: PackagesStateCell,
    pub(crate) can_install: Rc<std::cell::Cell<bool>>,
    pub(crate) uninstall_list: Rc<RefCell<WizardUninstallList>>,
    pub(crate) uninstall_items: UninstallStateCell,
}

/// Build the Packages page: the heading, then the notebook with today's
/// package list on the Install or update tab and the Uninstall tab beside
/// it.
pub(crate) fn build_packages_tabs(
    page: &WizardPage,
    model: &WizardModel,
    state: PackagesPageState,
) -> PackagesPageWidgets {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    add_heading(
        page,
        &sizer,
        &model.text.packages_heading,
        "rabbit-packages-heading",
    );
    let notebook = Notebook::builder(page).build();
    notebook.set_name("rabbit-packages-tabs");

    // Each tab is a plain panel holding its own sizer. A panel's accessible
    // name on Windows is its window text, which SetLabel sets and SetName
    // doesn't, so the label carries the tab's name and the name stays an
    // internal id, which `set_optional_choice_shown` looks for.
    let install_tab = Panel::builder(&notebook).build();
    install_tab.set_name(INSTALL_TAB_NAME);
    install_tab.set_label(&model.text.packages_tab_install);
    let (
        package_checklist,
        package_details,
        osara_keymap_replace,
        osara_keymap_note,
        spanish_variant_choice,
        reaper_language_choice,
    ) = build_packages_page(
        &install_tab,
        model,
        state.package_rows,
        state.configuration_rows,
        state.package_items,
        state.can_install,
    );

    let uninstall_tab = Panel::builder(&notebook).build();
    uninstall_tab.set_name(UNINSTALL_TAB_NAME);
    uninstall_tab.set_label(&model.text.packages_tab_uninstall);
    let (uninstall_list, uninstall_details, uninstall_remove_settings, uninstall_settings_note) =
        build_uninstall_tab(
            &uninstall_tab,
            model,
            state.uninstall_list,
            state.uninstall_items,
        );

    notebook.add_page(&install_tab, &model.text.packages_tab_install, true, None);
    notebook.add_page(
        &uninstall_tab,
        &model.text.packages_tab_uninstall,
        false,
        None,
    );
    // Proportion 1 and the lists' own minimum heights keep both tabs at
    // their real size inside the scrolled page, so neither list can be
    // squeezed out of the accessibility tree.
    sizer.add(&notebook, 1, SizerFlag::All | SizerFlag::Expand, 6);
    page.set_sizer(sizer, true);

    PackagesPageWidgets {
        notebook,
        package_checklist,
        package_details,
        osara_keymap_replace,
        osara_keymap_note,
        spanish_variant_choice,
        reaper_language_choice,
        uninstall_list,
        uninstall_details,
        uninstall_remove_settings,
        uninstall_settings_note,
    }
}

/// The run Next on the Packages page starts: the tab showing decides it.
/// Install and uninstall never mix.
pub(crate) fn selected_wizard_mode(widgets: &WizardWidgets) -> WizardMode {
    WizardMode::for_packages_tab(widgets.packages_notebook.selection())
}

/// Focus the Uninstall list, on the row it was on, for Back from Review in
/// uninstall mode. Focus would otherwise stay on Back, outside the wizard
/// page, and the tab and its caret are left as they were.
pub(crate) fn focus_uninstall_list(widgets: &WizardWidgets) {
    widgets.uninstall_list.set_focus();
}

/// Land on the Packages page after the version check, once it is the page
/// showing: the Install tab selected, and focus on the tab strip, so screen
/// readers start with "Install or update, tab, 1 of 2" and the Uninstall
/// tab is found without a hint. Tab then enters the install list, which
/// starts at its top.
pub(crate) fn arrive_on_packages_page(
    model: &WizardModel,
    widgets: &WizardWidgets,
    package_items: &PackagesStateCell,
) {
    let notebook = widgets.packages_notebook;
    // ChangeSelection rather than SetSelection: it sends no page-changed
    // event, so the macOS handler doesn't focus a list and announce the tab
    // on top of what the screen reader is about to read.
    notebook.change_selection(INSTALL_TAB);
    // The list's caret goes back to its top once the list gets focus, not
    // now: on Windows, moving it while the tab strip has focus made NVDA
    // read the list's row right after the tab.
    reset_packages_list_caret_on_focus();
    notebook.set_focus();
    if !notebook.has_focus() {
        // An NSTabView most likely takes keyboard focus only with macOS's
        // Full Keyboard Access on. Without it, focus goes to the Install
        // tab's list, and the tab's name is said, as after a tab switch.
        focus_packages_list_top(widgets, package_items);
        announce(
            &model.text.packages_tab_install,
            AnnouncePriority::Interrupt,
        );
    }
}

/// Wire the tab switching that native controls don't give macOS:
/// - Physical Ctrl+Tab and Ctrl+Shift+Tab switch tabs while the Packages page
///   shows. wxOSX switches only on Command+Tab and treats Ctrl+Tab as plain
///   Tab.
/// - After any switch, by those keys or by VoiceOver selecting a tab, focus
///   goes to the new tab's list and the tab's name is said: VoiceOver
///   doesn't speak a change made from code, and the list it lands in would
///   otherwise be the only clue.
///
/// Windows needs none of it: the native tab control switches on Ctrl+Tab
/// and Ctrl+Page Down and moves focus into the new tab by itself.
pub(crate) fn bind_packages_tab_switching(
    frame: &Frame,
    model: &Arc<WizardModel>,
    widgets: WizardWidgets,
    current_step: &Arc<AtomicUsize>,
) {
    if !cfg!(target_os = "macos") {
        return;
    }
    {
        let model = Arc::clone(model);
        let current_step = Arc::clone(current_step);
        widgets.packages_notebook.on_page_changed(move |event| {
            if current_step.load(Ordering::SeqCst) != PACKAGES_STEP {
                return;
            }
            if let Some(tab) = event.get_selection() {
                show_tab(&model, &widgets, tab as usize);
            }
        });
    }
    let current_step = Arc::clone(current_step);
    // wxEVT_CHAR_HOOK reaches the window before the focused control, so
    // the keys work wherever focus is on the page, as the expert-mode chord
    // does.
    frame.bind_internal(EventType::CHAR_HOOK, move |event| {
        if current_step.load(Ordering::SeqCst) == PACKAGES_STEP && is_tab_switch_chord(&event) {
            event.skip(false);
            // The page-changed handler above does the rest.
            widgets
                .packages_notebook
                .advance_selection(!event.shift_down());
        } else {
            // Every other key carries on to the focused control.
            event.skip(true);
        }
    });
}

/// Focus the list on tab `tab` and say the tab's name.
fn show_tab(model: &WizardModel, widgets: &WizardWidgets, tab: usize) {
    let name = match tab {
        INSTALL_TAB => {
            widgets.package_checklist.set_focus();
            &model.text.packages_tab_install
        }
        UNINSTALL_TAB => {
            widgets.uninstall_list.set_focus();
            &model.text.packages_tab_uninstall
        }
        _ => return,
    };
    announce(name, AnnouncePriority::Interrupt);
}

/// Physical Ctrl+Tab, with or without Shift. Command and Option must not be
/// down: Command+Tab switches applications, and Control+Option is the
/// VoiceOver modifier.
fn is_tab_switch_chord(event: &Event) -> bool {
    event.get_key_code() == Some(WXK_TAB)
        && !event.cmd_down()
        && !event.alt_down()
        && physical_control_down()
}

/// Whether the physical Control key is down. wxDragon's `control_down` is
/// Command on macOS, so it can't say.
fn physical_control_down() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::macos_keyboard::control_key_down()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
