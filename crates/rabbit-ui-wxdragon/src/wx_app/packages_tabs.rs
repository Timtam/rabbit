//! The Packages page's two tabs, Install or update and Uninstall: building
//! the notebook, where focus goes when the page opens, which run Next
//! starts, and the keyboard on macOS: switching tabs, and Tab to and from
//! the tab strip.
//!
//! The notebook is a native tab control, SysTabControl32 on Windows and
//! NSTabView on macOS, so screen readers present it as tabs. wxOSX moves
//! focus into a tab's page on every selection change, even on a wizard page
//! that is hidden. Adding the first tab while the window is built is such a
//! change too, so on macOS `add_pages` gives the start focus back to the
//! REAPER installation choice afterwards. After that, the selection is
//! changed from code in one place only: on arrival from the version check,
//! once the page shows.

use std::cell::{Cell, RefCell};
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
    reset_uninstall_list_caret_on_focus,
};
use crate::wx_app::pages::{WizardPage, add_heading};
use crate::wx_app::widgets::{INSTALL_TAB_NAME, UNINSTALL_TAB_NAME, WizardWidgets};

pub(crate) const INSTALL_TAB: usize = crate::PACKAGES_INSTALL_TAB;
pub(crate) const UNINSTALL_TAB: usize = crate::PACKAGES_UNINSTALL_TAB;

/// `wx/defs.h`: `WXK_TAB = 9`, the ASCII value.
const WXK_TAB: i32 = 9;

/// How long after the Packages page opens the macOS hint is said. By then
/// VoiceOver has begun reading the tab or the list, so the hint waits
/// behind that instead of being cut off by it.
const TAB_SWITCH_HINT_DELAY_MS: i32 = 700;

thread_local! {
    /// Whether VoiceOver has said, in this run of RABBIT, how to switch
    /// tabs. It says so once only, the first time the page opens.
    static TAB_SWITCH_HINT_GIVEN: Cell<bool> = const { Cell::new(false) };
    /// The timer that says that hint, macOS only. `bind_tab_switch_hint`
    /// sets it once, and it lasts as long as RABBIT runs.
    static TAB_SWITCH_HINT_TIMER: Cell<Option<&'static Timer<App>>> = const { Cell::new(None) };
}

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
/// uninstall mode. Showing the page puts focus back on the tab strip, the
/// page's last focused control, or leaves it on Back where the strip can't
/// take focus (macOS without keyboard navigation). The tab and its caret
/// are left as they were.
pub(crate) fn focus_uninstall_list(widgets: &WizardWidgets) {
    widgets.uninstall_list.set_focus();
}

/// Land on the Packages page after the version check, once it is the page
/// showing: the Install tab selected, and focus on the tab strip, so screen
/// readers start with "Install or update, tab, 1 of 2" and the Uninstall
/// tab is found without a hint. Tab then enters the install list, which
/// starts at its top. On macOS, a moment after the first arrival in a run,
/// VoiceOver also says that Control-Tab switches tabs.
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
    // Each list's caret goes back to its top once that list gets focus,
    // not now: on Windows, moving it while the tab strip has focus made
    // NVDA read the list's row right after the tab.
    reset_packages_list_caret_on_focus();
    reset_uninstall_list_caret_on_focus();
    notebook.set_focus();
    if !notebook.has_focus() {
        // macOS without Full Keyboard Access (keyboard navigation in System
        // Settings): an NSTabView takes no keyboard focus then. With it, the
        // strip keeps focus only because `bind_packages_tab_switching` stops
        // wxOSX's notebook from passing it on into the selected tab. Here,
        // focus goes to the Install tab's list, and the tab's name is said,
        // as after a tab switch.
        focus_packages_list_top(widgets, package_items);
        announce(
            &model.text.packages_tab_install,
            AnnouncePriority::Interrupt,
        );
    }
    if cfg!(target_os = "macos") && !TAB_SWITCH_HINT_GIVEN.get() {
        // Nothing on screen says how to switch tabs, and without Full
        // Keyboard Access the tabs take no focus, so their arrow keys are
        // out of reach. The timer says it a moment later.
        if let Some(timer) = TAB_SWITCH_HINT_TIMER.get() {
            timer.start(TAB_SWITCH_HINT_DELAY_MS, true);
        }
    }
}

/// Set up the timer that says the Control-Tab hint on macOS. Said at once,
/// the hint would race VoiceOver's reading of the new focus, which can cut
/// it off. Said a moment later and Polite, it waits for that reading to
/// end. It counts as given only if the Packages page still shows then;
/// otherwise the next arrival tries again.
fn bind_tab_switch_hint(model: &WizardModel, current_step: &Arc<AtomicUsize>) {
    let Some(app) = get_app_instance() else {
        return;
    };
    // The app owns the timer, so it never fires for a window that is gone.
    // The timer is never dropped, so it can't be destroyed after wxWidgets
    // shuts down.
    let timer: &'static Timer<App> = Box::leak(Box::new(Timer::new(&app)));
    let hint = model.text.packages_tabs_switch_hint_macos.clone();
    let current_step = Arc::clone(current_step);
    timer.on_tick(move |_| {
        if current_step.load(Ordering::SeqCst) == PACKAGES_STEP
            && !TAB_SWITCH_HINT_GIVEN.replace(true)
        {
            announce(&hint, AnnouncePriority::Polite);
        }
    });
    TAB_SWITCH_HINT_TIMER.set(Some(timer));
}

/// Wire the keyboard handling that native controls don't give macOS:
/// - The tab strip keeps focus when it gets it. wxOSX's notebook passes its
///   focus straight on into the selected tab, so the strip could never be
///   a Tab stop, and the page never opened with focus on it.
/// - Tab from the strip goes into the selected tab's list, Shift+Tab from
///   that list goes back to the strip, and Tab from Close goes round to the
///   strip, as on Windows. wxOSX's own Tab order goes past the strip, into
///   the tab and back out of it. The strip takes focus only with Full
///   Keyboard Access on; without it these keys move focus as before.
/// - Physical Ctrl+Tab and Ctrl+Shift+Tab switch tabs while the Packages page
///   shows. wxOSX switches only on Command+Tab and treats Ctrl+Tab as plain
///   Tab.
/// - After any switch, by those keys, by the arrow keys on the strip or by
///   VoiceOver selecting a tab, focus goes to the new tab's list and the
///   tab's name is said: VoiceOver doesn't speak a change made from code,
///   and the list it lands in would otherwise be the only clue. An empty
///   Uninstall list has no row to read, so its sentence is said with the
///   name.
/// - The timer for the one-time Control-Tab hint, which
///   `arrive_on_packages_page` starts.
///
/// Windows needs none of it: the native tab control keeps focus, switches
/// on Ctrl+Tab and Ctrl+Page Down and moves focus into the new tab by
/// itself.
pub(crate) fn bind_packages_tab_switching(
    frame: &Frame,
    close: &Button,
    model: &Arc<WizardModel>,
    widgets: WizardWidgets,
    uninstall_list: &Rc<RefCell<WizardUninstallList>>,
    current_step: &Arc<AtomicUsize>,
) {
    if !cfg!(target_os = "macos") {
        return;
    }
    // wxNotebook::OnSetFocus, in wxOSX's static event table, hands the
    // notebook's focus on to the selected tab. A handler bound at run time
    // runs before the static table, and one that doesn't skip the event
    // ends it there, so the strip keeps focus. Nothing else is lost: the
    // event that tells the page which control had focus last is a separate
    // one, and a tab change focuses the new tab by itself.
    widgets
        .packages_notebook
        .bind_internal(EventType::SET_FOCUS, |event| event.skip(false));
    bind_tab_switch_hint(model, current_step);
    {
        let model = Arc::clone(model);
        let uninstall_list = Rc::clone(uninstall_list);
        let current_step = Arc::clone(current_step);
        widgets.packages_notebook.on_page_changed(move |event| {
            if current_step.load(Ordering::SeqCst) != PACKAGES_STEP {
                return;
            }
            if let Some(tab) = event.get_selection() {
                show_tab(&model, &widgets, &uninstall_list.borrow(), tab as usize);
            }
        });
    }
    let close = *close;
    let current_step = Arc::clone(current_step);
    // wxEVT_CHAR_HOOK reaches the window before the focused control, and
    // before wxWidgets turns Tab into a focus move, so the keys work
    // wherever focus is on the page, as the expert-mode chord does.
    frame.bind_internal(EventType::CHAR_HOOK, move |event| {
        let key = if current_step.load(Ordering::SeqCst) == PACKAGES_STEP
            && event.get_key_code() == Some(WXK_TAB)
        {
            tab_key(event.cmd_down(), event.alt_down(), physical_control_down())
        } else {
            None
        };
        let handled = match key {
            Some(TabKey::SwitchTabs) => {
                // The page-changed handler above does the rest.
                widgets
                    .packages_notebook
                    .advance_selection(!event.shift_down());
                true
            }
            Some(TabKey::MoveFocus) => move_focus_for_tab(&widgets, &close, event.shift_down()),
            None => false,
        };
        // Every other key, and a Tab left to wxWidgets' own order, carries
        // on to the focused control.
        event.skip(!handled);
    });
}

/// What a Tab key press on the Packages page is for, by its modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabKey {
    /// Physical Ctrl+Tab or Ctrl+Shift+Tab: switch tabs.
    SwitchTabs,
    /// Tab or Shift+Tab alone: move focus.
    MoveFocus,
}

/// Sort a Tab key press by its modifiers. Command and Option must not be
/// down: Command+Tab switches applications, and Control+Option is the
/// VoiceOver modifier. `physical_control` is the real Control key, not
/// wxWidgets' "control", which is Command on macOS.
fn tab_key(command: bool, option: bool, physical_control: bool) -> Option<TabKey> {
    if command || option {
        None
    } else if physical_control {
        Some(TabKey::SwitchTabs)
    } else {
        Some(TabKey::MoveFocus)
    }
}

/// Where focus is when Tab is pressed on the Packages page, as far as the
/// tab strip's place in the Tab order goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabFocus {
    TabStrip,
    /// The list on the tab showing.
    SelectedList,
    /// The Close button, the last control in the Tab order there.
    Close,
    Elsewhere,
}

/// Where Tab or Shift+Tab sends focus instead of wxWidgets' own order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabMove {
    IntoSelectedList,
    OntoTabStrip,
    /// wxWidgets moves focus as usual.
    AsUsual,
}

/// The Tab order around the tab strip: strip, the tab's controls from its
/// list on, the buttons, and from Close round to the strip again. wxOSX
/// gets the rest right by itself, Shift+Tab from the strip to Close
/// included.
fn tab_move(focus: TabFocus, shift: bool) -> TabMove {
    match (focus, shift) {
        (TabFocus::TabStrip, false) => TabMove::IntoSelectedList,
        (TabFocus::SelectedList, true) | (TabFocus::Close, false) => TabMove::OntoTabStrip,
        _ => TabMove::AsUsual,
    }
}

/// Move focus for a plain Tab or Shift+Tab on the Packages page where
/// wxOSX's own order would skip the tab strip. Returns whether focus got
/// there, so the key is used up only then. Without Full Keyboard Access the
/// strip takes no focus, and wxWidgets moves focus as before.
fn move_focus_for_tab(widgets: &WizardWidgets, close: &Button, shift: bool) -> bool {
    let notebook = widgets.packages_notebook;
    let list = selected_tab_list(widgets);
    let focus = if notebook.has_focus() {
        TabFocus::TabStrip
    } else if list.has_focus() {
        TabFocus::SelectedList
    } else if close.has_focus() {
        TabFocus::Close
    } else {
        TabFocus::Elsewhere
    };
    match tab_move(focus, shift) {
        TabMove::IntoSelectedList => {
            list.set_focus();
            list.has_focus()
        }
        TabMove::OntoTabStrip => {
            notebook.set_focus();
            notebook.has_focus()
        }
        TabMove::AsUsual => false,
    }
}

/// The list on the tab showing.
fn selected_tab_list(widgets: &WizardWidgets) -> &dyn WxWidget {
    match selected_wizard_mode(widgets) {
        WizardMode::Install => &widgets.package_checklist,
        WizardMode::Uninstall => &widgets.uninstall_list,
    }
}

/// Focus the list on tab `tab` and say the tab's name, and for an empty
/// Uninstall list why it is empty.
fn show_tab(
    model: &WizardModel,
    widgets: &WizardWidgets,
    uninstall_list: &WizardUninstallList,
    tab: usize,
) {
    let message = match tab {
        INSTALL_TAB => {
            widgets.package_checklist.set_focus();
            model.text.packages_tab_install.clone()
        }
        UNINSTALL_TAB => {
            widgets.uninstall_list.set_focus();
            uninstall_list.tab_announcement(&model.text)
        }
        _ => return,
    };
    announce(&message, AnnouncePriority::Interrupt);
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

#[cfg(test)]
mod tests {
    use super::{TabFocus, TabKey, TabMove, tab_key, tab_move};

    #[test]
    fn only_tab_without_command_or_option_is_ours() {
        for physical_control in [false, true] {
            assert_eq!(tab_key(true, false, physical_control), None);
            assert_eq!(tab_key(false, true, physical_control), None);
            assert_eq!(tab_key(true, true, physical_control), None);
        }
        assert_eq!(tab_key(false, false, true), Some(TabKey::SwitchTabs));
        assert_eq!(tab_key(false, false, false), Some(TabKey::MoveFocus));
    }

    #[test]
    fn tab_goes_from_the_strip_into_the_list_and_from_close_to_the_strip() {
        assert_eq!(
            tab_move(TabFocus::TabStrip, false),
            TabMove::IntoSelectedList
        );
        assert_eq!(tab_move(TabFocus::Close, false), TabMove::OntoTabStrip);
        assert_eq!(tab_move(TabFocus::SelectedList, false), TabMove::AsUsual);
        assert_eq!(tab_move(TabFocus::Elsewhere, false), TabMove::AsUsual);
    }

    #[test]
    fn shift_tab_goes_from_the_list_to_the_strip_and_on_as_usual() {
        assert_eq!(
            tab_move(TabFocus::SelectedList, true),
            TabMove::OntoTabStrip
        );
        // wxOSX takes Shift+Tab from the strip on to Close by itself.
        assert_eq!(tab_move(TabFocus::TabStrip, true), TabMove::AsUsual);
        assert_eq!(tab_move(TabFocus::Close, true), TabMove::AsUsual);
        assert_eq!(tab_move(TabFocus::Elsewhere, true), TabMove::AsUsual);
    }
}
