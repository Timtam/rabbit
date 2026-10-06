//! Windows packages page: a native `TreeCtrl` whose checkboxes come from
//! the Win32 control itself, so screen readers announce them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::{
    ConfigurationRow, OsaraKeymapChoice, PackageRow, WizardInstallOptions, WizardModel,
    apply_checkbox_state_to_package_row, localizer_from_options,
    recompute_configuration_row_availability,
};
use rabbit_core::plan::PlanActionKind;
use wxdragon::event::tree_events::TreeEventData;
use wxdragon::widgets::treectrl::{TreeCtrl, TreeCtrlStyle, TreeItemId};

use super::native_tree_checkboxes;
use wxdragon::prelude::*;

use super::{PackagesStateCell, PackagesView, WXK_SPACE};

use crate::wx_app::pages::{WizardPage, add_heading, add_label};
use crate::wx_app::widgets::{
    REAPER_LANGUAGE_LABEL_NAME, SPANISH_VARIANT_LABEL_NAME, WizardWidgets, package_details,
    sync_osara_keymap_widgets, sync_reaper_language_widget, sync_spanish_variant_widget,
};

/// Live wxTreeItemId handles for both top-level groups in the Packages
/// tree — "Packages" with its package leaves, and "Configuration" with
/// its configuration-step leaves. Index `i` in each leaves vec
/// corresponds to index `i` in the matching `Vec<PackageRow>` /
/// `Vec<ConfigurationRow>`. Kept in an `Rc<RefCell>` so the closures
/// that handle the state-image-click event, the LEFT_UP fallback, the
/// keyboard fallbacks, and the post-install / version-check rebuild
/// helpers can all reach the same TreeItemIds the populate routine
/// handed out. `TreeItemId` is not `Copy` (it owns a pointer with
/// custom Drop), so we can't store it on the `Copy`-derived
/// `WizardWidgets` directly.
pub(crate) struct PackageItems {
    /// The "Packages" group node under the (hidden) virtual root.
    /// Becomes `None` between `populate_packages_tree` calls; populated
    /// immediately after each rebuild.
    pub(crate) packages_group: Option<TreeItemId>,
    /// The "Additional software" group node (Surge XT, app2clap, …), a
    /// sibling of the Packages group. `None` when no package has the
    /// `Additional` category — the group node isn't created then.
    pub(crate) additional_software_group: Option<TreeItemId>,
    /// The "Language packs" group node (REAPER translations), a sibling of
    /// the Packages group. `None` when no package has the `Language`
    /// category — the group node isn't created then.
    pub(crate) language_group: Option<TreeItemId>,
    /// One TreeItemId per package row, in the same order as `package_rows`
    /// (covering BOTH the Packages and Additional-software groups). The leaf
    /// for `package_rows[i]` is at `packages_leaves[i]` regardless of which
    /// group node it visually hangs under, so leaf↔row index stays 1:1.
    pub(crate) packages_leaves: Vec<TreeItemId>,
    /// The "Configuration" group node sitting alongside the Packages
    /// group under the virtual root.
    pub(crate) configuration_group: Option<TreeItemId>,
    /// One TreeItemId per configuration row, in the same order as
    /// `configuration_rows`.
    pub(crate) configuration_leaves: Vec<TreeItemId>,
}

impl PackageItems {
    pub(crate) fn empty() -> Self {
        Self {
            packages_group: None,
            additional_software_group: None,
            language_group: None,
            packages_leaves: Vec::new(),
            configuration_group: None,
            configuration_leaves: Vec::new(),
        }
    }
}

/// Re-render the package list after the deferred fetch repopulates
/// `package_rows`. Invoked on successful version check, just before the
/// auto-advance to the Packages step. Two implementations: Windows rebuilds
/// the native TreeCtrl from scratch; non-Windows mutates the DataView
/// model's userdata in place and emits a `cleared()` notification.
pub(crate) fn rebuild_package_list_widgets(
    widgets: &WizardWidgets,
    package_items: &PackagesStateCell,
    model: &WizardModel,
    package_rows: &[PackageRow],
    configuration_rows: &[ConfigurationRow],
) {
    populate_packages_tree(
        &widgets.package_checklist,
        package_items,
        model,
        package_rows,
        configuration_rows,
    );
    let initial = package_rows
        .first()
        .map(package_details)
        .unwrap_or_default();
    widgets.package_details.set_value(&initial);
}

/// Put keyboard focus on the packages list with the screen-reader caret on
/// its TOP-MOST entry — the "Packages" group header. After the list is
/// repopulated (delete-all + re-append + per-item check-state writes) the
/// native `wxTreeCtrl`'s caret ends up on an arbitrary row a few items
/// down, so a screen-reader user entering the page starts mid-list instead
/// of at the top. Called when the wizard auto-advances onto the packages
/// page; deliberate Back navigation is left alone so it keeps the user's
/// previous position.
pub(crate) fn focus_packages_list_top(widgets: &WizardWidgets, package_items: &PackagesStateCell) {
    // Clone the item id and DROP the borrow before touching the tree:
    // `select_item` dispatches the selection-changed handler synchronously
    // on MSW, and that handler borrows `package_items` too.
    let group = package_items.borrow().packages_group.clone();
    if let Some(group) = group {
        let tree = &widgets.package_checklist;
        tree.ensure_visible(&group);
        tree.select_item(&group);
        tree.set_focused_item(&group);
    }
    widgets.package_checklist.set_focus();
}

/// Windows-only: tear down the existing native tree and rebuild both
/// top-level groups ("Packages" and "Configuration") from
/// `package_rows` + `configuration_rows`. Each leaf gets its native
/// `TVS_CHECKBOXES` state set to match its row's `selected`; each
/// group gets a tristate reflecting its children's aggregate.
pub(crate) fn populate_packages_tree(
    tree: &TreeCtrl,
    package_items: &PackagesStateCell,
    model: &WizardModel,
    package_rows: &[PackageRow],
    configuration_rows: &[ConfigurationRow],
) {
    tree.delete_all_items();
    {
        let mut items = package_items.borrow_mut();
        items.packages_group = None;
        items.additional_software_group = None;
        items.language_group = None;
        items.packages_leaves.clear();
        items.configuration_group = None;
        items.configuration_leaves.clear();
    }

    let Some(root) = tree.add_root("", None, None) else {
        return;
    };

    // Package rows split into two sibling groups by category: "Packages"
    // (Core — the REAPER accessibility stack) and "Additional software"
    // (Surge XT, app2clap, … — extras not tied to REAPER itself). The
    // Additional-software group node is only created when at least one
    // Additional-category row exists, so it never renders empty.
    let Some(packages_group) =
        tree.append_item(&root, &model.text.packages_tree_group_label, None, None)
    else {
        return;
    };
    let has_additional = package_rows
        .iter()
        .any(|row| row.category == rabbit_core::package::PackageCategory::Additional);
    let additional_software_group = if has_additional {
        tree.append_item(
            &root,
            &model.text.additional_software_tree_group_label,
            None,
            None,
        )
    } else {
        None
    };
    let has_language = package_rows
        .iter()
        .any(|row| row.category == rabbit_core::package::PackageCategory::Language);
    let language_group = if has_language {
        tree.append_item(&root, &model.text.language_tree_group_label, None, None)
    } else {
        None
    };
    // One leaf per row, in row order, parented under the group that matches
    // the row's category. `packages_leaves[i]` stays the leaf for
    // `package_rows[i]` regardless of which group it hangs under, so every
    // leaf↔row index lookup elsewhere remains 1:1.
    let mut packages_leaves = Vec::with_capacity(package_rows.len());
    for row in package_rows.iter() {
        let parent = match row.category {
            rabbit_core::package::PackageCategory::Additional => additional_software_group
                .as_ref()
                .unwrap_or(&packages_group),
            rabbit_core::package::PackageCategory::Language => {
                language_group.as_ref().unwrap_or(&packages_group)
            }
            rabbit_core::package::PackageCategory::Core => &packages_group,
        };
        let label = format_row_label(&row.summary, row.selected);
        if let Some(item) = tree.append_item(parent, &label, None, None) {
            native_tree_checkboxes::set_check_state(tree.get_handle(), &item, row.selected);
            packages_leaves.push(item);
        }
    }
    // Tristate reflecting each group's available children's aggregate.
    let packages_state = compute_packages_group_tristate(package_rows);
    native_tree_checkboxes::set_check_state_tri(tree.get_handle(), &packages_group, packages_state);
    if let Some(group) = additional_software_group.as_ref() {
        let additional_state = compute_additional_software_group_tristate(package_rows);
        native_tree_checkboxes::set_check_state_tri(tree.get_handle(), group, additional_state);
    }
    if let Some(group) = language_group.as_ref() {
        let language_state = compute_language_group_tristate(package_rows);
        native_tree_checkboxes::set_check_state_tri(tree.get_handle(), group, language_state);
    }

    // Configuration group + leaves. Always created, even if no
    // configuration rows are recommended for this run — keeps the tree
    // shape stable so the user can find the section if/when it
    // populates after a target switch or post-install rescan.
    let Some(configuration_group) = tree.append_item(
        &root,
        &model.text.configuration_tree_group_label,
        None,
        None,
    ) else {
        return;
    };
    let mut configuration_leaves = Vec::with_capacity(configuration_rows.len());
    for row in configuration_rows.iter() {
        let label = format_row_label(&row.summary, row.selected);
        if let Some(item) = tree.append_item(&configuration_group, &label, None, None) {
            native_tree_checkboxes::set_check_state(tree.get_handle(), &item, row.selected);
            configuration_leaves.push(item);
        }
    }
    let configuration_state = compute_configuration_group_tristate(configuration_rows);
    native_tree_checkboxes::set_check_state_tri(
        tree.get_handle(),
        &configuration_group,
        configuration_state,
    );

    {
        let mut items = package_items.borrow_mut();
        items.packages_group = Some(packages_group.clone());
        items.additional_software_group = additional_software_group.clone();
        items.language_group = language_group.clone();
        items.packages_leaves = packages_leaves;
        items.configuration_group = Some(configuration_group.clone());
        items.configuration_leaves = configuration_leaves;
    }

    tree.expand(&packages_group);
    if let Some(group) = additional_software_group.as_ref() {
        tree.expand(group);
    }
    if let Some(group) = language_group.as_ref() {
        tree.expand(group);
    }
    tree.expand(&configuration_group);
}

/// Windows-only: format a tree-row label. The native `TVS_CHECKBOXES`
/// style draws the checkbox for us, so this is currently just the summary
/// — kept as a single point so we can later add a status glyph or icon
/// without auditing every call site.
pub(crate) fn format_row_label(summary: &str, _selected: bool) -> String {
    summary.to_string()
}

/// Windows-only: aggregate the per-row `selected` flags into a tristate for
/// the synthetic "Packages" (Core) group node. Unavailable rows don't count
/// for either side because they can't enter the install plan and toggling
/// them is a no-op — we only look at the rows the user can actually flip.
pub(crate) fn compute_packages_group_tristate(
    rows: &[crate::PackageRow],
) -> native_tree_checkboxes::TriState {
    compute_package_category_tristate(rows, rabbit_core::package::PackageCategory::Core)
}

/// Windows-only: the same aggregate for the "Additional software" group node.
pub(crate) fn compute_additional_software_group_tristate(
    rows: &[crate::PackageRow],
) -> native_tree_checkboxes::TriState {
    compute_package_category_tristate(rows, rabbit_core::package::PackageCategory::Additional)
}

/// Windows-only: the same aggregate for the "Language packs" group node.
pub(crate) fn compute_language_group_tristate(
    rows: &[crate::PackageRow],
) -> native_tree_checkboxes::TriState {
    compute_package_category_tristate(rows, rabbit_core::package::PackageCategory::Language)
}

/// Windows-only: tristate over the available rows of a single package
/// category, so each top-level group's checkbox reflects only its own
/// children.
pub(crate) fn compute_package_category_tristate(
    rows: &[crate::PackageRow],
    category: rabbit_core::package::PackageCategory,
) -> native_tree_checkboxes::TriState {
    let mut any = false;
    let mut all = true;
    let mut any_checked = false;
    for row in rows
        .iter()
        .filter(|r| r.category == category && r.available_for_target)
    {
        any = true;
        if row.selected {
            any_checked = true;
        } else {
            all = false;
        }
    }
    if !any {
        // No selectable rows in this category (e.g. everything's unavailable
        // for this target). Render the group as unchecked rather than mixed —
        // there's nothing to toggle.
        return native_tree_checkboxes::TriState::Unchecked;
    }
    if all {
        native_tree_checkboxes::TriState::Checked
    } else if any_checked {
        native_tree_checkboxes::TriState::Mixed
    } else {
        native_tree_checkboxes::TriState::Unchecked
    }
}

/// Windows-only: aggregate the per-row `selected` flags of every
/// actionable [`ConfigurationRow`] into a tristate for the synthetic
/// "Configuration" group node. Same convention as
/// `compute_packages_group_tristate` — unavailable rows AND
/// already-applied rows are excluded (the user can't toggle either),
/// and an empty actionable-set renders as Unchecked.
pub(crate) fn compute_configuration_group_tristate(
    rows: &[crate::ConfigurationRow],
) -> native_tree_checkboxes::TriState {
    let mut any = false;
    let mut all = true;
    let mut any_checked = false;
    for row in rows
        .iter()
        .filter(|r| r.available_for_target && !r.already_applied)
    {
        any = true;
        if row.selected {
            any_checked = true;
        } else {
            all = false;
        }
    }
    if !any {
        return native_tree_checkboxes::TriState::Unchecked;
    }
    if all {
        native_tree_checkboxes::TriState::Checked
    } else if any_checked {
        native_tree_checkboxes::TriState::Mixed
    } else {
        native_tree_checkboxes::TriState::Unchecked
    }
}

/// Windows: native `wxTreeCtrl` driving `SysTreeView32` with
/// `TVS_CHECKBOXES`. Each row exposes UIA Toggle pattern, screen readers
/// announce checked state, Space toggles natively. See
/// `native_tree_checkboxes` for the raw Win32 plumbing that flips the
/// style after wx has created the control.
pub(crate) fn build_packages_page(
    page: &WizardPage,
    model: &WizardModel,
    package_rows: Rc<RefCell<Vec<crate::PackageRow>>>,
    configuration_rows: Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    package_items: PackagesStateCell,
    can_install: Rc<Cell<bool>>,
) -> (PackagesView, TextCtrl, CheckBox, TextCtrl, Choice, Choice) {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    add_heading(
        page,
        &sizer,
        &model.text.packages_heading,
        "rabbit-packages-heading",
    );
    add_label(
        page,
        &sizer,
        &model.text.packages_list_label,
        "rabbit-packages-list-label",
    );

    // wxTreeCtrl is a thin wrapper around the platform's native tree:
    // SysTreeView32 on Windows, NSOutlineView on macOS, GtkTreeView on GTK.
    // HasButtons + LinesAtRoot give the standard expand/collapse affordance;
    // HideRoot keeps the synthetic root invisible so the "Packages" group
    // appears as the top-level branch the user navigates first.
    let tree = TreeCtrl::builder(page)
        .with_style(
            TreeCtrlStyle::HasButtons
                | TreeCtrlStyle::LinesAtRoot
                | TreeCtrlStyle::Single
                | TreeCtrlStyle::HideRoot,
        )
        .with_size(Size::new(-1, 220))
        .build();
    tree.set_name("rabbit-package-list");
    // Floor the list height so longer translations (the labels, checkbox, and
    // notes below grow vertically in German/French) can never squeeze the
    // proportion-1 list down to nothing. It still expands to fill free space.
    tree.set_min_size(Size::new(-1, 160));

    // Switch the underlying SysTreeView32 to TVS_CHECKBOXES so each tree
    // row gets a real native checkbox — UIA exposes a Toggle pattern on
    // each TreeItem, screen readers announce the checked state, Space
    // toggles natively, and the visual is indistinguishable from File
    // Explorer's "items to copy" tree.
    native_tree_checkboxes::enable_checkboxes(tree.get_handle());

    populate_packages_tree(
        &tree,
        &package_items,
        model,
        &package_rows.borrow(),
        &configuration_rows.borrow(),
    );
    sizer.add(&tree, 1, SizerFlag::All | SizerFlag::Expand, 6);

    add_label(
        page,
        &sizer,
        &model.text.package_details_label,
        "rabbit-package-details-label",
    );
    let initial_details = package_rows
        .borrow()
        .first()
        .map(package_details)
        .unwrap_or_default();
    let details = TextCtrl::builder(page)
        .with_value(&initial_details)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 120))
        .build();
    details.set_name("rabbit-package-details");
    sizer.add(&details, 0, SizerFlag::All | SizerFlag::Expand, 6);

    add_label(
        page,
        &sizer,
        &model.text.packages_osara_keymap_heading,
        "rabbit-osara-keymap-heading",
    );
    let osara_keymap_replace = CheckBox::builder(page)
        .with_label(&model.text.packages_osara_keymap_replace_label)
        .build();
    osara_keymap_replace.set_name(&model.text.packages_osara_keymap_replace_label);
    osara_keymap_replace.set_label(&model.text.packages_osara_keymap_replace_label);
    osara_keymap_replace.add_style(WindowStyle::TabStop);
    osara_keymap_replace.set_value(matches!(
        WizardInstallOptions::default().osara_keymap_choice,
        OsaraKeymapChoice::ReplaceCurrent
    ));
    osara_keymap_replace.set_can_focus(false);
    sizer.add(
        &osara_keymap_replace,
        0,
        SizerFlag::All | SizerFlag::Expand,
        6,
    );

    let osara_keymap_note = TextCtrl::builder(page)
        .with_value(&model.text.packages_osara_keymap_unavailable_note)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 68))
        .build();
    osara_keymap_note.set_name("rabbit-osara-keymap-note");
    osara_keymap_note.enable(false);
    osara_keymap_note.set_can_focus(false);
    sizer.add(&osara_keymap_note, 0, SizerFlag::All | SizerFlag::Expand, 6);

    // Which installed language pack REAPER starts in. Several packs can be
    // installed at once (REAPER keeps them all in LangPack/ and you can
    // switch inside REAPER later), so this only picks the one active
    // straight after installing.
    add_label(
        page,
        &sizer,
        &model.text.packages_reaper_language_label,
        REAPER_LANGUAGE_LABEL_NAME,
    );
    let reaper_language_choice = Choice::builder(page).build();
    reaper_language_choice.set_name(&model.text.packages_reaper_language_label);
    sizer.add(
        &reaper_language_choice,
        0,
        SizerFlag::All | SizerFlag::Expand,
        6,
    );

    // Which of the two Spanish OSARA translations to install. Only the
    // installed FILE NAME differs (es_ES vs es_MX) — that is what OSARA
    // reads to pick its translation — so this is a plain either/or rather
    // than a separate package.
    add_label(
        page,
        &sizer,
        &model.text.packages_spanish_variant_label,
        SPANISH_VARIANT_LABEL_NAME,
    );
    let spanish_variant_choice = Choice::builder(page).build();
    spanish_variant_choice.set_name(&model.text.packages_spanish_variant_label);
    for label in &model.text.packages_spanish_variant_options {
        spanish_variant_choice.append(label);
    }
    spanish_variant_choice.set_selection(0);
    sizer.add(
        &spanish_variant_choice,
        0,
        SizerFlag::All | SizerFlag::Expand,
        6,
    );

    sync_osara_keymap_widgets(
        model,
        &package_rows.borrow(),
        &osara_keymap_replace,
        &osara_keymap_note,
    );
    sync_spanish_variant_widget(&package_rows.borrow(), &spanish_variant_choice);
    sync_reaper_language_widget(&package_rows.borrow(), &reaper_language_choice);

    // Selection-change updates the package details text. The event fires
    // when the focused row changes via mouse or arrow keys; we use the
    // wxTreeItemId from the event to find the matching index in
    // `package_items.leaves`.
    {
        let package_rows = Rc::clone(&package_rows);
        let configuration_rows = Rc::clone(&configuration_rows);
        let package_items = Rc::clone(&package_items);
        let model_text = model.clone();
        let osara_checkbox = osara_keymap_replace;
        let osara_note = osara_keymap_note;
        let spanish_checkbox = spanish_variant_choice;
        let language_choice = reaper_language_choice;
        tree.on_selection_changed(move |event| {
            if let Some(item) = event.get_item() {
                match classify_leaf(&package_items.borrow(), &item) {
                    Some(WhichLeaf::Packages(idx)) => {
                        if let Some(value) = package_rows.borrow().get(idx).map(package_details) {
                            details.set_value(&value);
                        }
                    }
                    Some(WhichLeaf::Configuration(idx)) => {
                        if let Some(row) = configuration_rows.borrow().get(idx) {
                            details.set_value(&row.details);
                        }
                    }
                    None => {}
                }
            }
            sync_osara_keymap_widgets(
                &model_text,
                &package_rows.borrow(),
                &osara_checkbox,
                &osara_note,
            );
            sync_spanish_variant_widget(&package_rows.borrow(), &spanish_checkbox);
            sync_reaper_language_widget(&package_rows.borrow(), &language_choice);
        });
    }

    // Native checkbox toggle handling for LEAVES: SysTreeView32 fires
    // `wxEVT_TREE_STATE_IMAGE_CLICK` whenever the user activates the
    // checkbox area of a leaf item — both mouse click and Space go through
    // the same notification. The typed `TreeEvents` trait doesn't expose
    // this variant, so we bind the raw `EventType::TREE_STATE_IMAGE_CLICK`
    // ourselves.
    {
        let tree_widget = tree;
        let package_rows = Rc::clone(&package_rows);
        let configuration_rows = Rc::clone(&configuration_rows);
        let package_items = Rc::clone(&package_items);
        let can_install = Rc::clone(&can_install);
        let wizard_model = model.clone();
        let osara_checkbox = osara_keymap_replace;
        let osara_note = osara_keymap_note;
        let spanish_checkbox = spanish_variant_choice;
        let language_choice = reaper_language_choice;
        tree.bind_internal(EventType::TREE_STATE_IMAGE_CLICK, move |event| {
            handle_native_checkbox_toggle(
                &tree_widget,
                &package_items,
                &package_rows,
                &configuration_rows,
                &can_install,
                &wizard_model,
                &details,
                &osara_checkbox,
                &osara_note,
                &spanish_checkbox,
                &language_choice,
                TreeEventData::new(event).get_item(),
            );
        });
    }

    // Pre-empt mouse clicks on not-actionable leaves' state icons —
    // unavailable packages, already-applied / unavailable configuration
    // steps. SysTreeView32 cycles TVS_CHECKBOXES on WM_LBUTTONDOWN
    // (which gives the click immediate visual feedback); without this
    // pre-empt, the click flips the state image and the accessibility
    // layer announces "checked", and we then have to revert in the
    // TREE_STATE_IMAGE_CLICK path — the user sees a flicker plus a
    // doubled screen-reader announcement. Eating the event before
    // native sees it keeps the row's state stable. We deliberately
    // do NOT pre-empt clicks on the parent group's state icon here
    // (the LEFT_UP handler below handles those — we still want the
    // native side effects of focus/selection to run on a parent
    // click).
    {
        let tree_widget = tree;
        let package_rows = Rc::clone(&package_rows);
        let configuration_rows = Rc::clone(&configuration_rows);
        let package_items = Rc::clone(&package_items);
        tree.on_mouse_left_down(move |event| {
            let WindowEventData::MouseButton(mb) = &event else {
                return;
            };
            let Some(pos) = mb.get_position() else { return };
            let hwnd = tree_widget.get_handle();
            let (flags, h_item) = native_tree_checkboxes::hit_test(hwnd, pos.x, pos.y);
            if (flags & native_tree_checkboxes::TVHT_ONITEMSTATEICON) == 0 || h_item.is_null() {
                return;
            }
            let items = package_items.borrow();
            let blocked = items
                .packages_leaves
                .iter()
                .position(|leaf| native_tree_handle(leaf) == h_item)
                .and_then(|idx| package_rows.borrow().get(idx).cloned())
                .map(|row| !row.available_for_target)
                .or_else(|| {
                    items
                        .configuration_leaves
                        .iter()
                        .position(|leaf| native_tree_handle(leaf) == h_item)
                        .and_then(|idx| configuration_rows.borrow().get(idx).cloned())
                        .map(|row| !row.available_for_target || row.already_applied)
                })
                .unwrap_or(false);
            drop(items);
            if blocked {
                event.skip(false);
            }
        });
    }

    // Parent-group toggle fallback: wxEVT_TREE_STATE_IMAGE_CLICK doesn't
    // fire reliably for the parent group (the native control's auto-cycle
    // on state image 3 doesn't propagate to wx in our setup), so we hit-
    // test on every left-button release and propagate manually if the
    // click landed on the group's state icon. Leaves are still handled
    // by the TREE_STATE_IMAGE_CLICK binding above; this handler ignores
    // them (see the early-return on `is_group` inside).
    {
        let tree_widget = tree;
        let package_rows = Rc::clone(&package_rows);
        let configuration_rows = Rc::clone(&configuration_rows);
        let package_items = Rc::clone(&package_items);
        let can_install = Rc::clone(&can_install);
        let wizard_model = model.clone();
        let osara_checkbox = osara_keymap_replace;
        let osara_note = osara_keymap_note;
        let spanish_checkbox = spanish_variant_choice;
        let language_choice = reaper_language_choice;
        tree.on_mouse_left_up(move |event| {
            if let WindowEventData::MouseButton(mb) = &event
                && let Some(pos) = mb.get_position()
            {
                handle_packages_left_up(
                    &tree_widget,
                    &package_items,
                    &package_rows,
                    &configuration_rows,
                    &can_install,
                    &wizard_model,
                    &details,
                    &osara_checkbox,
                    &osara_note,
                    &spanish_checkbox,
                    &language_choice,
                    pos,
                );
            }
            event.skip(true);
        });
    }

    // Keyboard parent-toggle: Space on the parent group needs to
    // propagate just like a mouse click on its checkbox. Native
    // TVS_CHECKBOXES auto-cycles state on Space too, but its NM_CLICK
    // → wxEVT_TREE_STATE_IMAGE_CLICK dispatch has the same parent-skip
    // problem we hit with the mouse, so we intercept Space at key-down
    // time, propagate, and consume the event so the native cycle
    // doesn't get a chance to leave the parent in some weird half-state.
    {
        let tree_widget = tree;
        let package_rows = Rc::clone(&package_rows);
        let configuration_rows = Rc::clone(&configuration_rows);
        let package_items = Rc::clone(&package_items);
        let can_install = Rc::clone(&can_install);
        let wizard_model = model.clone();
        let osara_checkbox = osara_keymap_replace;
        let osara_note = osara_keymap_note;
        let spanish_checkbox = spanish_variant_choice;
        let language_choice = reaper_language_choice;
        tree.on_key_down(move |event| {
            let key_code = if let WindowEventData::Keyboard(kbd) = &event {
                kbd.get_key_code()
            } else {
                None
            };
            if key_code != Some(WXK_SPACE) {
                return;
            }
            let Some(focused) = tree_widget.get_selection() else {
                return;
            };
            // Pre-empt Space on a leaf the user can't toggle —
            // unavailable packages, or already-applied / unavailable
            // configuration steps. If we let native TVS_CHECKBOXES
            // see the keystroke, it cycles the state image (and the
            // accessibility layer announces "checked"), and our key-up
            // handler then has to revert. The user experiences that
            // as a flicker plus a doubled screen-reader announcement.
            // Consuming the event here keeps the row's state stable.
            if let Some(leaf) = classify_leaf(&package_items.borrow(), &focused) {
                let blocked = match leaf {
                    WhichLeaf::Packages(idx) => package_rows
                        .borrow()
                        .get(idx)
                        .is_some_and(|row| !row.available_for_target),
                    WhichLeaf::Configuration(idx) => configuration_rows
                        .borrow()
                        .get(idx)
                        .is_some_and(|row| !row.available_for_target || row.already_applied),
                };
                if blocked {
                    event.skip(false);
                    return;
                }
                // Actionable leaf: let native cycle run; on_key_up
                // reconciles row.selected with the post-cycle state.
                return;
            }
            let group = classify_group(&package_items.borrow(), &focused);
            let Some(group) = group else {
                return;
            };
            propagate_group_toggle_to_leaves(
                &tree_widget,
                &package_items,
                group,
                &package_rows,
                &configuration_rows,
                &wizard_model,
            );
            refresh_after_packages_toggle(
                &tree_widget,
                &package_items,
                &package_rows,
                &configuration_rows,
                &can_install,
                &wizard_model,
                &details,
                &osara_checkbox,
                &osara_note,
                &spanish_checkbox,
                &language_choice,
            );
            // Consume the event so the native control doesn't *also*
            // toggle the parent's state image after us.
            event.skip(false);
        });
    }

    // Keyboard leaf-toggle: wxEVT_TREE_STATE_IMAGE_CLICK fires only off
    // NM_CLICK (mouse), not for keyboard Space — the native control's
    // TVS_CHECKBOXES auto-cycle on Space flips the visual but doesn't
    // route through any wx event we can hook. Without this handler, a
    // Space toggle on a leaf updates the visual but never updates
    // `package_rows`, leaving the row out of sync with the checkbox
    // and (as a knock-on) the parent's tristate stuck on whatever it
    // was before. We bind KEY_UP because by then the native auto-cycle
    // has already run, so reading `get_check_state` gives us the new
    // post-cycle state.
    {
        let tree_widget = tree;
        let package_rows = Rc::clone(&package_rows);
        let configuration_rows = Rc::clone(&configuration_rows);
        let package_items = Rc::clone(&package_items);
        let can_install = Rc::clone(&can_install);
        let wizard_model = model.clone();
        let osara_checkbox = osara_keymap_replace;
        let osara_note = osara_keymap_note;
        let spanish_checkbox = spanish_variant_choice;
        let language_choice = reaper_language_choice;
        tree.on_key_up(move |event| {
            let key_code = if let WindowEventData::Keyboard(kbd) = &event {
                kbd.get_key_code()
            } else {
                None
            };
            if key_code != Some(WXK_SPACE) {
                return;
            }
            let Some(focused) = tree_widget.get_selection() else {
                return;
            };
            // Parent Space is already handled in on_key_down (which
            // consumed the event before native processing); only
            // leaves need post-cycle reconciliation here.
            if classify_group(&package_items.borrow(), &focused).is_some() {
                return;
            }
            let leaf = classify_leaf(&package_items.borrow(), &focused);
            let new_state =
                native_tree_checkboxes::get_check_state(tree_widget.get_handle(), &focused);
            match leaf {
                Some(WhichLeaf::Packages(idx)) => {
                    let unavailable = package_rows
                        .borrow()
                        .get(idx)
                        .is_some_and(|row| !row.available_for_target);
                    if unavailable {
                        native_tree_checkboxes::set_check_state(
                            tree_widget.get_handle(),
                            &focused,
                            false,
                        );
                        return;
                    }
                    if let Some(row) = package_rows.borrow_mut().get_mut(idx) {
                        let _ = apply_checkbox_state_to_package_row(&wizard_model, row, new_state);
                    }
                    if let Some(row) = package_rows.borrow().get(idx) {
                        let label = format_row_label(&row.summary, row.selected);
                        tree_widget.set_item_text(&focused, &label);
                    }
                }
                Some(WhichLeaf::Configuration(idx)) => {
                    let not_actionable = configuration_rows
                        .borrow()
                        .get(idx)
                        .is_some_and(|row| !row.available_for_target || row.already_applied);
                    if not_actionable {
                        native_tree_checkboxes::set_check_state(
                            tree_widget.get_handle(),
                            &focused,
                            false,
                        );
                        return;
                    }
                    if let Some(row) = configuration_rows.borrow_mut().get_mut(idx) {
                        row.selected = new_state;
                    }
                }
                None => return,
            }
            refresh_after_packages_toggle(
                &tree_widget,
                &package_items,
                &package_rows,
                &configuration_rows,
                &can_install,
                &wizard_model,
                &details,
                &osara_checkbox,
                &osara_note,
                &spanish_checkbox,
                &language_choice,
            );
        });
    }

    // Enter / double-click handler — Enter on the parent group needs to
    // propagate the toggle to all leaves (the native control fires
    // wxEVT_TREE_ITEM_ACTIVATED for Enter, regardless of TVS_CHECKBOXES).
    //
    // We deliberately ignore the leaf case here: wxMSW also dispatches
    // ITEM_ACTIVATED for Space on a focused leaf, and toggling the leaf
    // here would race with the TREE_STATE_IMAGE_CLICK leaf path (the
    // native auto-cycle has already flipped the state image, our
    // STATE_IMAGE_CLICK handler reads + applies the new state, and a
    // second flip from this handler would then leave the row out of
    // sync with the visual). Space + click on leaves continue to work
    // through the existing TREE_STATE_IMAGE_CLICK binding; Enter on a
    // leaf is intentionally a no-op.
    {
        let tree_widget = tree;
        let package_rows = Rc::clone(&package_rows);
        let configuration_rows = Rc::clone(&configuration_rows);
        let package_items = Rc::clone(&package_items);
        let can_install = Rc::clone(&can_install);
        let wizard_model = model.clone();
        let osara_checkbox = osara_keymap_replace;
        let osara_note = osara_keymap_note;
        let spanish_checkbox = spanish_variant_choice;
        let language_choice = reaper_language_choice;
        tree.on_item_activated(move |event| {
            let Some(item) = event.get_item() else {
                return;
            };
            let Some(group) = classify_group(&package_items.borrow(), &item) else {
                return;
            };
            propagate_group_toggle_to_leaves(
                &tree_widget,
                &package_items,
                group,
                &package_rows,
                &configuration_rows,
                &wizard_model,
            );
            refresh_after_packages_toggle(
                &tree_widget,
                &package_items,
                &package_rows,
                &configuration_rows,
                &can_install,
                &wizard_model,
                &details,
                &osara_checkbox,
                &osara_note,
                &spanish_checkbox,
                &language_choice,
            );
        });
    }

    {
        let model_text = model.clone();
        let rows = Rc::clone(&package_rows);
        let osara_checkbox = osara_keymap_replace;
        let osara_note = osara_keymap_note;
        let spanish_checkbox = spanish_variant_choice;
        let language_choice = reaper_language_choice;
        osara_keymap_replace.on_toggled(move |_| {
            sync_osara_keymap_widgets(&model_text, &rows.borrow(), &osara_checkbox, &osara_note);
            sync_spanish_variant_widget(&rows.borrow(), &spanish_checkbox);
            sync_reaper_language_widget(&rows.borrow(), &language_choice);
        });
    }

    page.set_sizer(sizer, true);
    (
        tree,
        details,
        osara_keymap_replace,
        osara_keymap_note,
        spanish_variant_choice,
        reaper_language_choice,
    )
}

/// Windows-only: which top-level group a tree item belongs to, if any.
/// Used by the toggle handlers to dispatch on Packages-vs-Configuration
/// without rebuilding the HTREEITEM-comparison plumbing at each call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WhichGroup {
    Packages,
    AdditionalSoftware,
    Language,
    Configuration,
}

/// Windows-only: a leaf's owning group + its row index. Mirrors the
/// shape of the row vec the index applies to, so callers can reach into
/// the right `Vec<PackageRow>` / `Vec<ConfigurationRow>` without an
/// extra branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WhichLeaf {
    Packages(usize),
    Configuration(usize),
}

/// Windows-only: identify which top-level group node a tree item is.
/// Returns `None` for leaves and for the (hidden) virtual root. Compares
/// via the native `HTREEITEM` (`m_pItem`) because wxdragon's
/// `TreeItemId` wraps a fresh allocation per event call — pointer
/// equality on the Rust wrappers wouldn't match our stored handles.
pub(crate) fn classify_group(items: &PackageItems, candidate: &TreeItemId) -> Option<WhichGroup> {
    let candidate_handle = native_tree_handle(candidate);
    if candidate_handle.is_null() {
        return None;
    }
    if items
        .packages_group
        .as_ref()
        .is_some_and(|group| native_tree_handle(group) == candidate_handle)
    {
        return Some(WhichGroup::Packages);
    }
    if items
        .additional_software_group
        .as_ref()
        .is_some_and(|group| native_tree_handle(group) == candidate_handle)
    {
        return Some(WhichGroup::AdditionalSoftware);
    }
    if items
        .language_group
        .as_ref()
        .is_some_and(|group| native_tree_handle(group) == candidate_handle)
    {
        return Some(WhichGroup::Language);
    }
    if items
        .configuration_group
        .as_ref()
        .is_some_and(|group| native_tree_handle(group) == candidate_handle)
    {
        return Some(WhichGroup::Configuration);
    }
    None
}

/// Windows-only: identify which group's leaves a tree item belongs to,
/// and at which index within that vec. Returns `None` for the group
/// nodes themselves, the hidden root, and items that aren't part of
/// our current row sets.
pub(crate) fn classify_leaf(items: &PackageItems, candidate: &TreeItemId) -> Option<WhichLeaf> {
    let candidate_handle = native_tree_handle(candidate);
    if candidate_handle.is_null() {
        return None;
    }
    if let Some(idx) = items
        .packages_leaves
        .iter()
        .position(|stored| native_tree_handle(stored) == candidate_handle)
    {
        return Some(WhichLeaf::Packages(idx));
    }
    if let Some(idx) = items
        .configuration_leaves
        .iter()
        .position(|stored| native_tree_handle(stored) == candidate_handle)
    {
        return Some(WhichLeaf::Configuration(idx));
    }
    None
}

/// Windows-only: read the native `HTREEITEM` behind a wxdragon
/// `TreeItemId`. SAFETY contract is the same as `native_tree_checkboxes`:
/// `TreeItemId` is a single-field `repr(Rust)` wrapper around
/// `*mut wxd_TreeItemId_t`, and that pointer is a `reinterpret_cast` of
/// `wxTreeItemId*` which holds a single `void* m_pItem` member.
pub(crate) fn native_tree_handle(item: &TreeItemId) -> *mut std::ffi::c_void {
    if !item.is_ok() {
        return std::ptr::null_mut();
    }
    // Read the wrapper's private `ptr` field by transmuting `&TreeItemId`
    // into a borrow of its inner pointer.
    let inner: *mut std::ffi::c_void = unsafe { std::mem::transmute_copy(item) };
    if inner.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { *(inner as *const *mut std::ffi::c_void) }
}

/// Windows-only: handle a `wxEVT_TREE_STATE_IMAGE_CLICK` for a leaf row
/// in either the Packages or the Configuration group.
///
/// Parent-group state-icon clicks are intentionally NOT handled here —
/// they're routed through the dedicated `LEFT_UP` + hit-test fallback
/// (`handle_packages_left_up`) because wxMSW's NM_CLICK →
/// wxEVT_TREE_STATE_IMAGE_CLICK dispatch isn't reliable for parent items
/// in our setup (the native auto-cycle sees state image index 3 and may
/// not propagate the event).
///
/// For leaves: `TVS_CHECKBOXES` has already flipped the state image by
/// the time this event fires, so we read the post-click state from the
/// native control rather than computing it ourselves. Package leaves
/// route through `apply_checkbox_state_to_package_row` (so action labels
/// flip Install/Update/Keep); configuration leaves just toggle
/// `row.selected`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_native_checkbox_toggle(
    tree: &TreeCtrl,
    package_items: &PackagesStateCell,
    package_rows: &Rc<RefCell<Vec<crate::PackageRow>>>,
    configuration_rows: &Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    can_install: &Rc<Cell<bool>>,
    wizard_model: &WizardModel,
    details: &TextCtrl,
    osara_checkbox: &CheckBox,
    osara_note: &TextCtrl,
    spanish_checkbox: &Choice,
    language_choice: &Choice,
    item: Option<TreeItemId>,
) {
    let Some(item) = item else {
        return;
    };

    // Parent-group clicks defer to the LEFT_UP handler.
    if classify_group(&package_items.borrow(), &item).is_some() {
        return;
    }

    let leaf = classify_leaf(&package_items.borrow(), &item);
    let new_state = native_tree_checkboxes::get_check_state(tree.get_handle(), &item);
    match leaf {
        Some(WhichLeaf::Packages(idx)) => {
            let unavailable = package_rows
                .borrow()
                .get(idx)
                .is_some_and(|row| !row.available_for_target);
            if unavailable {
                native_tree_checkboxes::set_check_state(tree.get_handle(), &item, false);
                return;
            }
            if let Some(row) = package_rows.borrow_mut().get_mut(idx) {
                let _ = apply_checkbox_state_to_package_row(wizard_model, row, new_state);
            }
            if let Some(row) = package_rows.borrow().get(idx) {
                let label = format_row_label(&row.summary, row.selected);
                tree.set_item_text(&item, &label);
            }
        }
        Some(WhichLeaf::Configuration(idx)) => {
            let not_actionable = configuration_rows
                .borrow()
                .get(idx)
                .is_some_and(|row| !row.available_for_target || row.already_applied);
            if not_actionable {
                native_tree_checkboxes::set_check_state(tree.get_handle(), &item, false);
                return;
            }
            if let Some(row) = configuration_rows.borrow_mut().get_mut(idx) {
                row.selected = new_state;
            }
            // Configuration row labels don't include action text, so no
            // re-format is needed; the row's `summary` already matches
            // `display_name`.
        }
        None => return,
    }

    refresh_after_packages_toggle(
        tree,
        package_items,
        package_rows,
        configuration_rows,
        can_install,
        wizard_model,
        details,
        osara_checkbox,
        osara_note,
        spanish_checkbox,
        language_choice,
    );
}

/// Windows-only: hit-test a `wxEVT_LEFT_UP` mouse-up against the tree.
/// If the click landed on either group's state icon, propagate the
/// toggle to that group's available leaves. The native control may
/// have auto-cycled the parent's image to a state that disagrees with
/// the row aggregate, but we always rewrite the parent state via
/// `set_check_state_tri` at the end so the visual matches the data.
///
/// We deliberately ignore leaf state-icon clicks here — they go through
/// `wxEVT_TREE_STATE_IMAGE_CLICK` which is reliable for leaf items and
/// has the post-cycle state already populated, so duplicating the work
/// here would either double-toggle or fight the leaf path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_packages_left_up(
    tree: &TreeCtrl,
    package_items: &PackagesStateCell,
    package_rows: &Rc<RefCell<Vec<crate::PackageRow>>>,
    configuration_rows: &Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    can_install: &Rc<Cell<bool>>,
    wizard_model: &WizardModel,
    details: &TextCtrl,
    osara_checkbox: &CheckBox,
    osara_note: &TextCtrl,
    spanish_checkbox: &Choice,
    language_choice: &Choice,
    pos: Point,
) {
    let hwnd = tree.get_handle();
    let (flags, h_item) = native_tree_checkboxes::hit_test(hwnd, pos.x, pos.y);
    if (flags & native_tree_checkboxes::TVHT_ONITEMSTATEICON) == 0 || h_item.is_null() {
        return;
    }
    // Identify which group's state icon was hit by comparing the raw
    // HTREEITEM directly against each stored group's native handle.
    // We don't go through `classify_group` here because we don't have
    // a `TreeItemId` wrapper yet — the hit-test message returns only
    // the native handle.
    let group = {
        let items = package_items.borrow();
        if items
            .packages_group
            .as_ref()
            .is_some_and(|g| native_tree_handle(g) == h_item)
        {
            Some(WhichGroup::Packages)
        } else if items
            .additional_software_group
            .as_ref()
            .is_some_and(|g| native_tree_handle(g) == h_item)
        {
            Some(WhichGroup::AdditionalSoftware)
        } else if items
            .language_group
            .as_ref()
            .is_some_and(|g| native_tree_handle(g) == h_item)
        {
            Some(WhichGroup::Language)
        } else if items
            .configuration_group
            .as_ref()
            .is_some_and(|g| native_tree_handle(g) == h_item)
        {
            Some(WhichGroup::Configuration)
        } else {
            None
        }
    };
    let Some(group) = group else {
        return;
    };

    propagate_group_toggle_to_leaves(
        tree,
        package_items,
        group,
        package_rows,
        configuration_rows,
        wizard_model,
    );
    refresh_after_packages_toggle(
        tree,
        package_items,
        package_rows,
        configuration_rows,
        can_install,
        wizard_model,
        details,
        osara_checkbox,
        osara_note,
        spanish_checkbox,
        language_choice,
    );
}

/// Windows-only: refresh both groups' tristate visuals + plan-level
/// `can_install` flag + OSARA widgets + details pane after any toggle
/// (leaf or group) that mutated `package_rows` or
/// `configuration_rows`. Also re-evaluates configuration-row
/// availability against the latest package plan so that, e.g.,
/// unchecking ReaPack greys out the REAPER Accessibility row in real
/// time.
#[allow(clippy::too_many_arguments)]
pub(crate) fn refresh_after_packages_toggle(
    tree: &TreeCtrl,
    package_items: &PackagesStateCell,
    package_rows: &Rc<RefCell<Vec<crate::PackageRow>>>,
    configuration_rows: &Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    can_install: &Rc<Cell<bool>>,
    wizard_model: &WizardModel,
    details: &TextCtrl,
    osara_checkbox: &CheckBox,
    osara_note: &TextCtrl,
    spanish_checkbox: &Choice,
    language_choice: &Choice,
) {
    // Configuration rows depend on the package plan (e.g. ReaPack must
    // be installed/queued for the REAPER Accessibility step). Re-evaluate
    // before refreshing the tree so the leaves' selected/state-image
    // values match what's in `configuration_rows`.
    if let Ok(localizer) = localizer_from_options(&wizard_model.bootstrap_options) {
        // None for the resource-path argument: a package toggle can't
        // change `reapack.ini`, so preserve each row's existing
        // `already_applied` flag rather than re-reading from disk on
        // every click.
        recompute_configuration_row_availability(
            &localizer,
            &package_rows.borrow(),
            None,
            &mut configuration_rows.borrow_mut(),
        );
        // Push the recomputed leaf states into the tree visual so the
        // user sees the live re-evaluation.
        let items = package_items.borrow();
        let configuration_rows_borrowed = configuration_rows.borrow();
        for (idx, leaf) in items.configuration_leaves.iter().enumerate() {
            let Some(row) = configuration_rows_borrowed.get(idx) else {
                continue;
            };
            native_tree_checkboxes::set_check_state(
                tree.get_handle(),
                leaf,
                row.selected && row.available_for_target && !row.already_applied,
            );
            let label = format_row_label(&row.summary, row.selected);
            tree.set_item_text(leaf, &label);
        }
    }

    {
        let items = package_items.borrow();
        if let Some(group) = items.packages_group.as_ref() {
            let group_state = compute_packages_group_tristate(&package_rows.borrow());
            native_tree_checkboxes::set_check_state_tri(tree.get_handle(), group, group_state);
        }
        if let Some(group) = items.additional_software_group.as_ref() {
            let group_state = compute_additional_software_group_tristate(&package_rows.borrow());
            native_tree_checkboxes::set_check_state_tri(tree.get_handle(), group, group_state);
        }
        if let Some(group) = items.configuration_group.as_ref() {
            let group_state = compute_configuration_group_tristate(&configuration_rows.borrow());
            native_tree_checkboxes::set_check_state_tri(tree.get_handle(), group, group_state);
        }
    }

    let any_install_or_update = package_rows.borrow().iter().any(|row| {
        row.available_for_target
            && matches!(row.action, PlanActionKind::Install | PlanActionKind::Update)
    });
    can_install.set(any_install_or_update);

    if let Some(selected) = tree.get_selection() {
        match classify_leaf(&package_items.borrow(), &selected) {
            Some(WhichLeaf::Packages(idx)) => {
                if let Some(row) = package_rows.borrow().get(idx) {
                    details.set_value(&package_details(row));
                }
            }
            Some(WhichLeaf::Configuration(idx)) => {
                if let Some(row) = configuration_rows.borrow().get(idx) {
                    details.set_value(&row.details);
                }
            }
            None => {}
        }
    }

    sync_osara_keymap_widgets(
        wizard_model,
        &package_rows.borrow(),
        osara_checkbox,
        osara_note,
    );
    sync_spanish_variant_widget(&package_rows.borrow(), spanish_checkbox);
    sync_reaper_language_widget(&package_rows.borrow(), language_choice);
}

/// Windows-only: implement the parent-checkbox propagation for the
/// requested group.
///
/// Convention (matches Windows Explorer / Visual Studio Installer):
/// - clicking a fully-checked parent → uncheck all available children;
/// - clicking an unchecked or mixed parent → check all available children.
///
/// Package leaves route mutations through `apply_checkbox_state_to_package_row`
/// so action labels flip Install / Update / Keep; configuration leaves
/// just flip `row.selected`. Unavailable rows in either group are left
/// untouched.
pub(crate) fn propagate_group_toggle_to_leaves(
    tree: &TreeCtrl,
    package_items: &PackagesStateCell,
    target_group: WhichGroup,
    package_rows: &Rc<RefCell<Vec<crate::PackageRow>>>,
    configuration_rows: &Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    wizard_model: &WizardModel,
) {
    match target_group {
        WhichGroup::Packages | WhichGroup::AdditionalSoftware | WhichGroup::Language => {
            // All three package groups share the same `packages_leaves` vec
            // (row-indexed); the category decides which rows this toggle
            // owns so the click only flips its own group's children.
            let category = match target_group {
                WhichGroup::AdditionalSoftware => rabbit_core::package::PackageCategory::Additional,
                WhichGroup::Language => rabbit_core::package::PackageCategory::Language,
                _ => rabbit_core::package::PackageCategory::Core,
            };
            let pre_state = compute_package_category_tristate(&package_rows.borrow(), category);
            let target = !matches!(pre_state, native_tree_checkboxes::TriState::Checked);
            let leaves: Vec<TreeItemId> = package_items.borrow().packages_leaves.to_vec();
            {
                let mut rows = package_rows.borrow_mut();
                for row in rows
                    .iter_mut()
                    .filter(|r| r.category == category && r.available_for_target)
                {
                    let _ = apply_checkbox_state_to_package_row(wizard_model, row, target);
                }
            }
            let rows = package_rows.borrow();
            let hwnd = tree.get_handle();
            for (idx, leaf) in leaves.iter().enumerate() {
                let Some(row) = rows.get(idx) else { continue };
                if row.category == category && row.available_for_target {
                    native_tree_checkboxes::set_check_state(hwnd, leaf, row.selected);
                    let label = format_row_label(&row.summary, row.selected);
                    tree.set_item_text(leaf, &label);
                }
            }
        }
        WhichGroup::Configuration => {
            let pre_state = compute_configuration_group_tristate(&configuration_rows.borrow());
            let target = !matches!(pre_state, native_tree_checkboxes::TriState::Checked);
            let leaves: Vec<TreeItemId> = package_items.borrow().configuration_leaves.to_vec();
            {
                let mut rows = configuration_rows.borrow_mut();
                for row in rows
                    .iter_mut()
                    .filter(|r| r.available_for_target && !r.already_applied)
                {
                    row.selected = target;
                }
            }
            let rows = configuration_rows.borrow();
            let hwnd = tree.get_handle();
            for (idx, leaf) in leaves.iter().enumerate() {
                let Some(row) = rows.get(idx) else { continue };
                if row.available_for_target && !row.already_applied {
                    native_tree_checkboxes::set_check_state(hwnd, leaf, row.selected);
                    let label = format_row_label(&row.summary, row.selected);
                    tree.set_item_text(leaf, &label);
                }
            }
        }
    }
}

/// Windows: re-render the native TreeCtrl after a row replacement.
#[allow(clippy::too_many_arguments)] // UI plumbing: one parameter per widget handle.
pub(crate) fn refresh_package_checklist(
    tree: &PackagesView,
    package_items: &PackagesStateCell,
    details: &TextCtrl,
    osara_keymap_replace: &CheckBox,
    osara_keymap_note: &TextCtrl,
    spanish_variant_choice: &Choice,
    reaper_language_choice: &Choice,
    model: &WizardModel,
    rows: &[crate::PackageRow],
    configuration_rows: &[ConfigurationRow],
) {
    populate_packages_tree(tree, package_items, model, rows, configuration_rows);
    details.set_value(&rows.first().map(package_details).unwrap_or_default());
    sync_osara_keymap_widgets(model, rows, osara_keymap_replace, osara_keymap_note);
    sync_spanish_variant_widget(rows, spanish_variant_choice);
    sync_reaper_language_widget(rows, reaper_language_choice);
}
