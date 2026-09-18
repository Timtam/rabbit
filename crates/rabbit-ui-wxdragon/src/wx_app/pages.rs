//! Builds each wizard page's widgets and adds them to the book.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::{WizardModel, wizard_desired_package_ids};
use wxdragon::widgets::SimpleBook;

use wxdragon::prelude::*;

use crate::wx_app::packages_page::{PackagesStateCell, build_packages_page};
use crate::wx_app::shell::{open_external_url, relaunch_with_locale};
use crate::wx_app::widgets::{
    WizardWidgets, configure_portable_folder, portable_choice_index, portable_target_details,
    selected_target_details, target_details_for_index,
};
use crate::wx_app::{
    DONE_STEP, PACKAGES_STEP, PROGRESS_STEP, REAPACK_ACK_STEP, REVIEW_STEP, TARGET_STEP,
    VERSION_CHECK_STEP,
};

/// A wizard page: a scrolling panel rather than a plain one.
///
/// Every page stacks fixed-height controls — lists, detail panes, notes —
/// and their total height grows with the UI language, since longer
/// translations wrap the labels between them onto extra lines. A plain
/// `wxPanel` cannot scroll, so whatever no longer fits is silently clipped,
/// and a `BoxSizer` short of room takes the whole deficit out of its only
/// proportion-1 child. On the Packages page that child is the package list
/// itself: it collapsed to nothing on a window that wasn't maximized, and a
/// control of zero height is not in the platform's accessibility tree at
/// all, so screen readers stopped seeing the list. Scrolling keeps every
/// control laid out at its real size and reachable whatever the window size.
pub(crate) type WizardPage = ScrolledWindow;

/// Vertical scroll step for a wizard page, in pixels. The horizontal step is
/// 0, which turns horizontal scrolling off: page content expands to the page
/// width instead of extending past it.
pub(crate) const WIZARD_PAGE_SCROLL_STEP: i32 = 10;

/// Build one empty wizard page inside the book.
pub(crate) fn new_wizard_page(book: &SimpleBook) -> WizardPage {
    let page = ScrolledWindow::builder(book).build();
    page.set_scroll_rate(0, WIZARD_PAGE_SCROLL_STEP);
    // wxScrolledWindow is created without wxTAB_TRAVERSAL, which wxPanel
    // gets by default — without it Tab stops moving between the controls of
    // a page. Put the flag back before any child is created.
    page.set_style_raw(page.get_style_raw() | PanelStyle::TabTraversal.bits());
    page
}

#[allow(clippy::too_many_arguments)] // UI plumbing: one parameter per widget handle.
pub(crate) fn add_pages(
    book: &SimpleBook,
    model: &WizardModel,
    package_rows: Rc<RefCell<Vec<crate::PackageRow>>>,
    configuration_rows: Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    package_items: PackagesStateCell,
    can_install: Rc<Cell<bool>>,
    self_update_status: StatusBar,
    language_footer: Panel,
) -> WizardWidgets {
    let target_page = new_wizard_page(book);
    let (target_choice, portable_folder, target_details) = build_target_page(&target_page, model);
    book.add_page(&target_page, &model.steps[TARGET_STEP].label, true, None);

    let version_check_page = new_wizard_page(book);
    let (
        version_check_status,
        version_check_gauge,
        version_check_error_heading,
        version_check_error_log,
    ) = build_version_check_page(
        &version_check_page,
        model,
        wizard_desired_package_ids(model.platform).len() as i32,
    );
    book.add_page(
        &version_check_page,
        &model.steps[VERSION_CHECK_STEP].label,
        false,
        None,
    );

    let packages_page = new_wizard_page(book);
    let (
        package_checklist,
        package_details,
        osara_keymap_replace,
        osara_keymap_note,
        spanish_variant_choice,
        reaper_language_choice,
    ) = build_packages_page(
        &packages_page,
        model,
        package_rows,
        configuration_rows,
        package_items,
        can_install,
    );
    book.add_page(
        &packages_page,
        &model.steps[PACKAGES_STEP].label,
        false,
        None,
    );

    let reapack_ack_page = new_wizard_page(book);
    let (_reapack_donate_link, reapack_ack_confirm) =
        build_reapack_ack_page(&reapack_ack_page, model);
    book.add_page(
        &reapack_ack_page,
        &model.steps[REAPACK_ACK_STEP].label,
        false,
        None,
    );

    let review_page = new_wizard_page(book);
    let review_text = build_review_page(&review_page, model);
    book.add_page(&review_page, &model.steps[REVIEW_STEP].label, false, None);

    let progress_page = new_wizard_page(book);
    let (progress_status, progress_gauge, progress_details) =
        build_progress_page(&progress_page, model);
    book.add_page(
        &progress_page,
        &model.steps[PROGRESS_STEP].label,
        false,
        None,
    );

    let done_page = new_wizard_page(book);
    let (done_status, done_details, done_launch_reaper, done_open_resource) =
        build_done_page(&done_page, model);
    book.add_page(&done_page, &model.steps[DONE_STEP].label, false, None);

    WizardWidgets {
        target_choice,
        portable_folder,
        target_details,
        version_check_status,
        version_check_gauge,
        version_check_error_heading,
        version_check_error_log,
        package_checklist,
        package_details,
        osara_keymap_replace,
        osara_keymap_note,
        spanish_variant_choice,
        reaper_language_choice,
        reapack_ack_confirm,
        review_text,
        progress_status,
        progress_gauge,
        progress_details,
        done_status,
        done_details,
        done_launch_reaper,
        done_open_resource,
        self_update_status,
        language_footer,
    }
}

pub(crate) fn build_target_page(
    page: &WizardPage,
    model: &WizardModel,
) -> (Choice, TextCtrl, TextCtrl) {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    add_heading(
        page,
        &sizer,
        &model.text.target_heading,
        "rabbit-target-heading",
    );

    add_label(
        page,
        &sizer,
        &model.text.target_choice_label,
        "rabbit-target-choice-label",
    );

    let choice = Choice::builder(page).build();
    choice.set_name("rabbit-target-choice");
    for row in &model.target_rows {
        choice.append(&row.label);
    }
    let portable_index = portable_choice_index(model);
    choice.append(&model.text.target_portable_choice);
    choice.set_selection(model.selected_target_index.unwrap_or(portable_index) as u32);
    sizer.add(&choice, 0, SizerFlag::All | SizerFlag::Expand, 6);

    add_label(
        page,
        &sizer,
        &model.text.target_portable_folder_label,
        "rabbit-target-portable-folder-label",
    );

    // We build the path input as a TextCtrl + Browse button instead of
    // wxDirPickerCtrl: wxdragon doesn't expose the picker's inner wxTextCtrl,
    // so the screen reader has no way to read a label off it. Mirroring the
    // picker's composition by hand lets us name the editable field directly,
    // and the user gets a real text input they can paste/type into.
    let portable_row = BoxSizer::builder(Orientation::Horizontal).build();
    let portable_folder = TextCtrl::builder(page).build();
    // Same wxdragon quirk as the ReaPack-ack checkbox below: the screen
    // reader reads the wxWindow *name*, not the preceding StaticText, so
    // set the name to the localized label instead of an internal id.
    portable_folder.set_name(&model.text.target_portable_folder_label);
    portable_folder.add_style(WindowStyle::TabStop);
    portable_row.add(&portable_folder, 1, SizerFlag::Expand | SizerFlag::Right, 6);

    let portable_folder_browse = Button::builder(page)
        .with_label(&model.text.target_portable_folder_browse_label)
        .build();
    portable_folder_browse.set_name(&model.text.target_portable_folder_browse_label);
    portable_folder_browse.add_style(WindowStyle::TabStop);
    portable_row.add(
        &portable_folder_browse,
        0,
        SizerFlag::AlignCenterVertical,
        0,
    );

    sizer.add_sizer(&portable_row, 0, SizerFlag::All | SizerFlag::Expand, 6);

    configure_portable_folder(
        &portable_folder,
        &portable_folder_browse,
        choice
            .get_selection()
            .map(|index| index as usize == portable_index)
            .unwrap_or(false),
    );

    add_label(
        page,
        &sizer,
        &model.text.target_details_label,
        "rabbit-target-details-label",
    );
    let initial_details = selected_target_details(model, &choice, &portable_folder);
    let details = TextCtrl::builder(page)
        .with_value(&initial_details)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 120))
        .build();
    details.set_name("rabbit-target-details");
    sizer.add(&details, 1, SizerFlag::All | SizerFlag::Expand, 6);

    {
        let choice_model = model.clone();
        let choice_portable_folder = portable_folder;
        let choice_portable_browse = portable_folder_browse;
        let choice_details = details;
        choice.on_selection_changed(move |event| {
            if let Some(index) = event.get_selection() {
                let index = index as usize;
                let portable_selected = index == portable_choice_index(&choice_model);
                configure_portable_folder(
                    &choice_portable_folder,
                    &choice_portable_browse,
                    portable_selected,
                );
                let value = if portable_selected {
                    portable_target_details(&choice_model, &choice_portable_folder)
                } else {
                    target_details_for_index(&choice_model, index)
                };
                choice_details.set_value(&value);
            }
        });
    }

    {
        let model = model.clone();
        let dir_choice = choice;
        let dir_details = details;
        let dir_portable_folder = portable_folder;
        let dir_portable_browse = portable_folder_browse;
        // Fires both for keyboard input AND for `set_value` from the Browse
        // button below — wxTextCtrl::SetValue generates wxEVT_TEXT — so this
        // single handler handles typing and the picker dialog uniformly.
        portable_folder.on_text_changed(move |_| {
            let portable_index = portable_choice_index(&model);
            if dir_choice
                .get_selection()
                .map(|index| index as usize != portable_index)
                .unwrap_or(true)
            {
                dir_choice.set_selection(portable_index as u32);
                configure_portable_folder(&dir_portable_folder, &dir_portable_browse, true);
            }
            dir_details.set_value(&portable_target_details(&model, &dir_portable_folder));
        });
    }

    {
        let dialog_parent = *page;
        let model_for_browse = model.clone();
        let browse_target = portable_folder;
        portable_folder_browse.on_click(move |_| {
            let current = browse_target.get_value();
            let dialog = DirDialog::builder(
                &dialog_parent,
                &model_for_browse.text.target_portable_folder_message,
                &current,
            )
            .build();
            if dialog.show_modal() == ID_OK
                && let Some(path) = dialog.get_path()
            {
                // Fires on_text_changed, which runs the same flip-to-portable
                // + update-details logic typing does.
                browse_target.set_value(&path);
            }
        });
    }

    page.set_sizer(sizer, true);
    choice.set_focus();
    (choice, portable_folder, details)
}

/// Base id for the language popup menu's radio items. Item id at index `i`
/// in `WizardModel::language_options` is `LANGUAGE_MENU_ID_BASE + i`.
pub(crate) const LANGUAGE_MENU_ID_BASE: i32 = 13700;

/// Build the language-picker footer inside a child Panel that lives below
/// the wizard buttons. The footer is only meaningful on the Target page —
/// switching languages relaunches RABBIT, so a switch from a later step
/// would discard the user's wizard progress anyway. Returning the child
/// Panel here lets the caller hide/show it via `update_navigation` based
/// on the current step. Adding it as a sibling of the button row means
/// tab order naturally reaches it after the last button (rather than
/// partway through the page), then wraps back to the page's first
/// focusable widget.
pub(crate) fn build_language_footer(
    root_panel: &Panel,
    root: &BoxSizer,
    model: &WizardModel,
) -> Panel {
    let footer = Panel::builder(root_panel).build();
    footer.set_name("rabbit-language-footer");
    let footer_sizer = BoxSizer::builder(Orientation::Vertical).build();

    add_label(
        &footer,
        &footer_sizer,
        &model.text.target_language_label,
        "rabbit-target-language-label",
    );

    let current_display_name = model
        .language_options
        .iter()
        .find(|option| option.locale == model.current_language)
        .map(|option| option.display_name.clone())
        .unwrap_or_else(|| model.current_language.clone());

    let language_button = Button::builder(&footer)
        .with_label(&current_display_name)
        .build();
    language_button.set_name("rabbit-target-language");
    language_button.add_style(WindowStyle::TabStop);
    language_button.set_can_focus(true);
    footer_sizer.add(&language_button, 0, SizerFlag::All | SizerFlag::Expand, 6);

    add_label(
        &footer,
        &footer_sizer,
        &model.text.target_language_restart_note,
        "rabbit-target-language-restart-note",
    );

    footer.set_sizer(footer_sizer, true);
    root.add(&footer, 0, SizerFlag::All | SizerFlag::Expand, 6);

    let language_options = model.language_options.clone();
    let current_locale = model.current_language.clone();

    // The popup menu dispatches its EVT_MENU to the popup's owner window
    // (the footer Panel here), not to the button that opened it.
    {
        let language_options = language_options.clone();
        let current_locale = current_locale.clone();
        footer.on_menu_selected(move |event| {
            let id = event.get_id();
            let raw_index = id - LANGUAGE_MENU_ID_BASE;
            if raw_index < 0 || (raw_index as usize) >= language_options.len() {
                return;
            }
            let Some(option) = language_options.get(raw_index as usize) else {
                return;
            };
            if option.locale == current_locale {
                return;
            }
            relaunch_with_locale(&option.locale);
        });
    }

    let menu_owner = footer;
    language_button.on_click(move |_| {
        let mut builder = Menu::builder();
        for (index, option) in language_options.iter().enumerate() {
            let id = LANGUAGE_MENU_ID_BASE + index as i32;
            builder = builder.append_radio_item(id, &option.display_name, "");
        }
        let menu = builder.build();
        for (index, option) in language_options.iter().enumerate() {
            if option.locale == current_locale {
                let id = LANGUAGE_MENU_ID_BASE + index as i32;
                menu.check_item(id, true);
            }
        }
        let mut menu = menu;
        menu_owner.popup_menu(&mut menu, None);
    });

    footer
}

pub(crate) fn build_version_check_page(
    page: &WizardPage,
    model: &WizardModel,
    package_count: i32,
) -> (StaticText, Gauge, StaticText, TextCtrl) {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    add_heading(
        page,
        &sizer,
        &model.text.version_check_heading,
        "rabbit-version-check-heading",
    );
    let status = StaticText::builder(page)
        .with_label(&model.text.version_check_status_pending)
        .build();
    status.set_name("rabbit-version-check-status");
    sizer.add(&status, 0, SizerFlag::All | SizerFlag::Expand, 6);

    add_label(
        page,
        &sizer,
        &model.text.version_check_progress_label,
        "rabbit-version-check-progress-label",
    );
    let gauge = Gauge::builder(page)
        .with_range(package_count.max(1))
        .build();
    gauge.set_name("rabbit-version-check-progress");
    sizer.add(&gauge, 0, SizerFlag::All | SizerFlag::Expand, 6);

    let error_heading = StaticText::builder(page)
        .with_label(&model.text.version_check_error_heading)
        .build();
    error_heading.set_name("rabbit-version-check-error-heading");
    sizer.add(&error_heading, 0, SizerFlag::All | SizerFlag::Expand, 6);
    let error_log = TextCtrl::builder(page)
        .with_value("")
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 120))
        .build();
    error_log.set_name("rabbit-version-check-error-log");
    sizer.add(&error_log, 1, SizerFlag::All | SizerFlag::Expand, 6);

    // Hide the error region until something fails so screen readers do not
    // see an empty Failed-checks/error-log pair while a check is in progress.
    // Show()/Hide() removes the controls from the tab order and the
    // accessibility tree; we re-Show() them in render_version_check_errors.
    error_heading.hide();
    error_log.hide();

    page.set_sizer(sizer, true);
    (status, gauge, error_heading, error_log)
}

/// Build the ReaPack donation-acknowledgement page. The page is only ever
/// shown when ReaPack is in the install/update plan — the Packages → Review
/// transition routes through it conditionally. The Continue button stays
/// disabled until the user checks the acknowledgement; that gating happens
/// in `update_navigation` based on `reapack_ack_confirm.get_value()`.
pub(crate) fn build_reapack_ack_page(page: &WizardPage, model: &WizardModel) -> (Button, CheckBox) {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    add_heading(
        page,
        &sizer,
        &model.text.reapack_ack_heading,
        "rabbit-reapack-ack-heading",
    );
    let body = TextCtrl::builder(page)
        .with_value(&model.text.reapack_ack_body)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 120))
        .build();
    body.set_name("rabbit-reapack-ack-body");
    sizer.add(&body, 0, SizerFlag::All | SizerFlag::Expand, 6);

    let donate_link = Button::builder(page)
        .with_label(&model.text.reapack_ack_link_label)
        .build();
    donate_link.set_name("rabbit-reapack-ack-donate-link");
    donate_link.add_style(WindowStyle::TabStop);
    donate_link.set_can_focus(true);
    sizer.add(&donate_link, 0, SizerFlag::All, 6);
    donate_link.on_click(move |_| {
        // Best-effort: open the donation page in the user's default browser
        // so the donation hint surfaces on a real, current upstream page
        // rather than a stale cached blurb in the wizard.
        let _ = open_external_url("https://reapack.com/donate");
    });

    let confirm = CheckBox::builder(page)
        .with_label(&model.text.reapack_ack_confirm_label)
        .build();
    // Mirror the OSARA-keymap / done-page CheckBox pattern: on this
    // wxdragon version the accessible name is driven by the wxWindow
    // *name* on Windows, not the visible `with_label` argument, so set
    // both `name` and `label` to the localized string. Without this the
    // screen reader announces the literal Fluent key
    // (`rabbit-reapack-ack-confirm`) instead of the translated label.
    confirm.set_name(&model.text.reapack_ack_confirm_label);
    confirm.set_label(&model.text.reapack_ack_confirm_label);
    confirm.add_style(WindowStyle::TabStop);
    confirm.set_value(false);
    sizer.add(&confirm, 0, SizerFlag::All, 6);

    page.set_sizer(sizer, true);
    (donate_link, confirm)
}

pub(crate) fn build_review_page(page: &WizardPage, model: &WizardModel) -> TextCtrl {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    add_heading(
        page,
        &sizer,
        &model.text.review_heading,
        "rabbit-review-heading",
    );
    let review = TextCtrl::builder(page)
        .with_value(&model.review_lines.join("\n"))
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .build();
    review.set_name("rabbit-review-text");
    sizer.add(&review, 1, SizerFlag::All | SizerFlag::Expand, 6);
    page.set_sizer(sizer, true);
    review
}

pub(crate) fn build_progress_page(
    page: &WizardPage,
    model: &WizardModel,
) -> (StaticText, Gauge, TextCtrl) {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    add_heading(
        page,
        &sizer,
        &model.text.progress_heading,
        "rabbit-progress-heading",
    );
    let status = StaticText::builder(page)
        .with_label(&model.text.progress_status)
        .build();
    status.set_name("rabbit-progress-status");
    sizer.add(&status, 0, SizerFlag::All | SizerFlag::Expand, 6);
    let gauge = Gauge::builder(page).with_range(100).build();
    gauge.set_name("rabbit-progress-gauge");
    sizer.add(&gauge, 0, SizerFlag::All | SizerFlag::Expand, 6);

    add_label(
        page,
        &sizer,
        &model.text.progress_details_label,
        "rabbit-progress-details-label",
    );
    let details = TextCtrl::builder(page)
        .with_value(&model.text.progress_details_idle)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .build();
    details.set_name("rabbit-progress-details");
    sizer.add(&details, 1, SizerFlag::All | SizerFlag::Expand, 6);

    page.set_sizer(sizer, true);
    (status, gauge, details)
}

pub(crate) fn build_done_page(
    page: &WizardPage,
    model: &WizardModel,
) -> (TextCtrl, TextCtrl, Button, Button) {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    add_heading(
        page,
        &sizer,
        &model.text.done_heading,
        "rabbit-done-heading",
    );
    // One short status TextCtrl (always visible) carries the success /
    // failure sentence + any follow-up status updates ("Report saved at …",
    // "REAPER could not be launched: …"). Power-user details live in the
    // collapsible TextCtrl below — kept hidden by default per the
    // streamlined wizard design.
    let status = TextCtrl::builder(page)
        .with_value(&model.text.done_status)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 80))
        .build();
    status.set_name("rabbit-done-status");
    sizer.add(&status, 0, SizerFlag::All | SizerFlag::Expand, 6);

    let show_details = CheckBox::builder(page)
        .with_label(&model.text.done_show_details_label)
        .build();
    // Mirror the OSARA-keymap checkbox pattern: on this wxdragon version
    // the visible label appears to be driven by the wxWindow *name* on
    // Windows (the `with_label` builder argument doesn't reliably stick),
    // so set both name and label to the same localized string and the
    // checkbox renders correctly in every locale.
    show_details.set_name(&model.text.done_show_details_label);
    show_details.set_label(&model.text.done_show_details_label);
    show_details.add_style(WindowStyle::TabStop);
    show_details.set_value(false);
    sizer.add(&show_details, 0, SizerFlag::All, 6);

    let details = TextCtrl::builder(page)
        .with_value("")
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .build();
    details.set_name("rabbit-done-details");
    details.hide();
    sizer.add(&details, 1, SizerFlag::All | SizerFlag::Expand, 6);

    let toggle_details = details;
    let toggle_page = *page;
    show_details.on_toggled(move |event| {
        let visible = event.is_checked();
        toggle_details.show(visible);
        toggle_page.layout();
        // Move keyboard focus into the details TextCtrl as soon as the
        // user reveals it. Screen readers (NVDA, JAWS) announce the
        // newly-focused control, which both confirms the checkbox click
        // and reads out the install report without the user having to
        // hunt for it via Tab.
        if visible {
            toggle_details.set_focus();
        }
    });

    let actions = BoxSizer::builder(Orientation::Horizontal).build();
    actions.add_stretch_spacer(1);

    let launch_reaper = Button::builder(page)
        .with_label(&model.text.done_launch_reaper_label)
        .build();
    launch_reaper.set_name("rabbit-done-launch-reaper");
    launch_reaper.add_style(WindowStyle::TabStop);
    launch_reaper.set_can_focus(true);
    launch_reaper.enable(false);
    actions.add(&launch_reaper, 0, SizerFlag::All, 6);

    let open_resource = Button::builder(page)
        .with_label(&model.text.done_open_resource_label)
        .build();
    open_resource.set_name("rabbit-done-open-resource");
    open_resource.add_style(WindowStyle::TabStop);
    open_resource.set_can_focus(true);
    open_resource.enable(false);
    actions.add(&open_resource, 0, SizerFlag::All, 6);

    sizer.add_sizer(&actions, 0, SizerFlag::All | SizerFlag::Expand, 0);
    page.set_sizer(sizer, true);
    (status, details, launch_reaper, open_resource)
}

pub(crate) fn add_heading<W: WxWidget>(page: &W, sizer: &BoxSizer, label: &str, name: &str) {
    let heading = StaticText::builder(page).with_label(label).build();
    heading.set_name(name);
    sizer.add(&heading, 0, SizerFlag::All | SizerFlag::Expand, 6);
}

pub(crate) fn add_label<W: WxWidget>(page: &W, sizer: &BoxSizer, label: &str, name: &str) {
    let widget = StaticText::builder(page).with_label(label).build();
    widget.set_name(name);
    sizer.add(
        &widget,
        0,
        SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
        6,
    );
}
