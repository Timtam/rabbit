//! Windows Uninstall tab: the same native `TreeCtrl` with real checkboxes as
//! the install list, with toggle handling of its own.
//!
//! Only the control is shared with the Install tab. Its toggle code ticks
//! dependencies, ticks a whole group at once and recomputes the
//! configuration rows; none of that applies here, and touching it would put
//! the Install tab at risk. The group rows here are headings without a
//! checkbox: in the install tree, Space on a group ticks every row below it,
//! and one keystroke must never mark everything for removal.

use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use rabbit_core::package::PackageCategory;
use wxdragon::event::tree_events::TreeEventData;
use wxdragon::prelude::*;
use wxdragon::widgets::treectrl::{TreeCtrl, TreeItemId};

use crate::{WizardModel, WizardUninstallList, apply_uninstall_checkbox};

use super::WXK_SPACE;
use super::native::{build_checkbox_tree, native_tree_handle};
use super::native_tree_checkboxes;

use crate::wx_app::pages::add_label;
use crate::wx_app::widgets::WizardWidgets;

/// The Uninstall tab's list widget.
pub(crate) type UninstallView = TreeCtrl;

/// The live item handles of the Uninstall tree, kept so its handlers can
/// tell a row from a heading. `TreeItemId` isn't `Copy`, so they can't sit
/// on `WizardWidgets`.
pub(crate) struct UninstallItems {
    /// The group headings. Only ever compared against.
    headings: Vec<TreeItemId>,
    /// One per row of the list, in the same order, whichever group it
    /// hangs under. `None` for a row the tree refused to add.
    leaves: Vec<Option<TreeItemId>>,
    /// The row, without a checkbox, that stands in for an empty list.
    placeholder: Option<TreeItemId>,
}

pub(crate) type UninstallStateCell = Rc<RefCell<UninstallItems>>;

impl UninstallItems {
    fn empty() -> Self {
        Self {
            headings: Vec::new(),
            leaves: Vec::new(),
            placeholder: None,
        }
    }
}

pub(crate) fn new_uninstall_state() -> UninstallStateCell {
    Rc::new(RefCell::new(UninstallItems::empty()))
}

/// What a tree item is, by its native handle: wxdragon hands out a fresh
/// `TreeItemId` per event, so the wrappers themselves never compare equal.
#[derive(Clone, Copy)]
enum UninstallNode {
    Row(usize),
    /// A heading or the placeholder: never a choice, never ticked.
    NotAChoice,
}

fn classify_handle(items: &UninstallItems, handle: *mut c_void) -> Option<UninstallNode> {
    if handle.is_null() {
        return None;
    }
    if let Some(index) = items.leaves.iter().position(|leaf| {
        leaf.as_ref()
            .is_some_and(|leaf| native_tree_handle(leaf) == handle)
    }) {
        return Some(UninstallNode::Row(index));
    }
    items
        .headings
        .iter()
        .chain(items.placeholder.as_ref())
        .any(|item| native_tree_handle(item) == handle)
        .then_some(UninstallNode::NotAChoice)
}

fn classify(items: &UninstallItems, item: &TreeItemId) -> Option<UninstallNode> {
    classify_handle(items, native_tree_handle(item))
}

/// The details text to start from: the first row's, or the placeholder's.
fn initial_details(model: &WizardModel, list: &WizardUninstallList) -> String {
    match list.rows.first() {
        Some(row) => row.details.clone(),
        None => list
            .placeholder(&model.text)
            .map(|(_, details)| details)
            .unwrap_or_default(),
    }
}

/// Rebuild the tree from `list`: a heading per group that has rows, in the
/// install tree's order, the rows below it, all unticked unless the row
/// says otherwise; or the placeholder alone.
fn populate_uninstall_tree(
    tree: &TreeCtrl,
    items: &UninstallStateCell,
    model: &WizardModel,
    list: &WizardUninstallList,
) {
    // Forget the old handles before the tree lets go of them: deleting and
    // adding rows can run the selection handler, and a deleted handle's
    // value may come back for a new row.
    *items.borrow_mut() = UninstallItems::empty();
    tree.delete_all_items();
    let mut fresh = UninstallItems::empty();
    let Some(root) = tree.add_root("", None, None) else {
        *items.borrow_mut() = fresh;
        return;
    };
    let hwnd = tree.get_handle();
    if let Some((label, _)) = list.placeholder(&model.text) {
        if let Some(item) = tree.append_item(&root, &label, None, None) {
            native_tree_checkboxes::clear_check_state(hwnd, &item);
            fresh.placeholder = Some(item);
        }
        *items.borrow_mut() = fresh;
        return;
    }
    let groups = [
        (PackageCategory::Core, &model.text.packages_tree_group_label),
        (
            PackageCategory::Additional,
            &model.text.additional_software_tree_group_label,
        ),
        (
            PackageCategory::Language,
            &model.text.language_tree_group_label,
        ),
    ];
    fresh.leaves = vec![None; list.rows.len()];
    for (category, label) in groups {
        if !list.rows.iter().any(|row| row.category == category) {
            continue;
        }
        let Some(heading) = tree.append_item(&root, label, None, None) else {
            continue;
        };
        native_tree_checkboxes::clear_check_state(hwnd, &heading);
        for (index, row) in list.rows.iter().enumerate() {
            if row.category != category {
                continue;
            }
            if let Some(item) = tree.append_item(&heading, &row.summary, None, None) {
                native_tree_checkboxes::set_check_state(hwnd, &item, row.selected);
                fresh.leaves[index] = Some(item);
            }
        }
        tree.expand(&heading);
        fresh.headings.push(heading);
    }
    *items.borrow_mut() = fresh;
}

/// Windows: build the Uninstall tab: the label naming the tab, the list and
/// a read-only details text. Ticking a row only changes its text and the
/// details; nothing acts on the ticks yet.
pub(crate) fn build_uninstall_tab(
    page: &Panel,
    model: &WizardModel,
    list: Rc<RefCell<WizardUninstallList>>,
    items: UninstallStateCell,
) -> (UninstallView, TextCtrl) {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    // Created right before the tree: that is the label NVDA and JAWS read
    // when focus lands in the list, and it names the tab.
    add_label(
        page,
        &sizer,
        &model.text.uninstall_list_label,
        "rabbit-uninstall-list-label",
    );
    let tree = build_checkbox_tree(page, "rabbit-uninstall-list");
    populate_uninstall_tree(&tree, &items, model, &list.borrow());
    sizer.add(&tree, 1, SizerFlag::All | SizerFlag::Expand, 6);

    add_label(
        page,
        &sizer,
        &model.text.package_details_label,
        "rabbit-uninstall-details-label",
    );
    let details = TextCtrl::builder(page)
        .with_value(&initial_details(model, &list.borrow()))
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, 120))
        .build();
    details.set_name("rabbit-uninstall-details");
    sizer.add(&details, 0, SizerFlag::All | SizerFlag::Expand, 6);

    {
        let items = Rc::clone(&items);
        let list = Rc::clone(&list);
        let model = model.clone();
        tree.on_selection_changed(move |event| {
            let Some(item) = event.get_item() else {
                return;
            };
            let node = classify(&items.borrow(), &item);
            let text = match node {
                Some(UninstallNode::Row(index)) => {
                    list.borrow().rows.get(index).map(|row| row.details.clone())
                }
                // The placeholder's details; a heading leaves them as they
                // were, as the install tree's groups do.
                Some(UninstallNode::NotAChoice) => list
                    .borrow()
                    .placeholder(&model.text)
                    .map(|(_, details)| details),
                None => None,
            };
            if let Some(text) = text {
                details.set_value(&text);
            }
        });
    }

    // A click on a row's checkbox. The native control has already flipped
    // the box by now, so the handler reads the new state back.
    {
        let items = Rc::clone(&items);
        let list = Rc::clone(&list);
        tree.bind_internal(EventType::TREE_STATE_IMAGE_CLICK, move |event| {
            if let Some(item) = TreeEventData::new(event).get_item() {
                reconcile_toggle(&tree, &items, &list, &details, &item);
            }
        });
    }

    // Keep the native control from flipping a box that must not change: a
    // row that can't be ticked, a heading or the placeholder. Reverting it
    // afterwards would flicker and have the screen reader say "checked"
    // first.
    {
        let items = Rc::clone(&items);
        let list = Rc::clone(&list);
        tree.on_mouse_left_down(move |event| {
            let WindowEventData::MouseButton(button) = &event else {
                return;
            };
            let Some(position) = button.get_position() else {
                return;
            };
            let (flags, handle) =
                native_tree_checkboxes::hit_test(tree.get_handle(), position.x, position.y);
            if flags & native_tree_checkboxes::TVHT_ONITEMSTATEICON == 0 {
                return;
            }
            if !can_toggle(classify_handle(&items.borrow(), handle), &list.borrow()) {
                event.skip(false);
            }
        });
    }

    // Space: let the native control flip a row that can be ticked, and
    // swallow it everywhere else, for the same reason as the click above.
    {
        let items = Rc::clone(&items);
        let list = Rc::clone(&list);
        tree.on_key_down(move |event| {
            if !is_space_key(&event) {
                return;
            }
            let Some(focused) = tree.get_selection() else {
                return;
            };
            if !can_toggle(classify(&items.borrow(), &focused), &list.borrow()) {
                event.skip(false);
            }
        });
    }

    // The native flip on Space reaches no wx event of its own, so the row
    // catches up once the key comes back up and the box shows the new state.
    {
        let items = Rc::clone(&items);
        let list = Rc::clone(&list);
        tree.on_key_up(move |event| {
            if !is_space_key(&event) {
                return;
            }
            if let Some(focused) = tree.get_selection() {
                reconcile_toggle(&tree, &items, &list, &details, &focused);
            }
        });
    }

    page.set_sizer(sizer, true);
    (tree, details)
}

fn is_space_key(event: &WindowEventData) -> bool {
    matches!(event, WindowEventData::Keyboard(keyboard) if keyboard.get_key_code() == Some(WXK_SPACE))
}

/// Whether the user may flip `node`'s checkbox.
fn can_toggle(node: Option<UninstallNode>, list: &WizardUninstallList) -> bool {
    match node {
        Some(UninstallNode::Row(index)) => list.rows.get(index).is_some_and(|row| row.can_tick),
        Some(UninstallNode::NotAChoice) => false,
        // Not one of ours: leave it to the native control.
        None => true,
    }
}

/// Bring a row in line with the checkbox the native control just flipped:
/// its text, and the details when it is the selected row. A row that can't
/// be ticked, a heading or the placeholder goes back to how it was.
/// Applying the same state twice changes nothing, so a toggle that reaches
/// both the click and the key handler is harmless.
fn reconcile_toggle(
    tree: &TreeCtrl,
    items: &UninstallStateCell,
    list: &Rc<RefCell<WizardUninstallList>>,
    details: &TextCtrl,
    item: &TreeItemId,
) {
    let hwnd = tree.get_handle();
    let node = classify(&items.borrow(), item);
    match node {
        Some(UninstallNode::Row(index)) => {
            let checked = native_tree_checkboxes::get_check_state(hwnd, item);
            // Take what the widgets need and let the borrow go before
            // touching them: a widget call can run another handler that
            // reads the list.
            let updated = list.borrow_mut().rows.get_mut(index).map(|row| {
                apply_uninstall_checkbox(row, checked);
                (row.selected, row.summary.clone(), row.details.clone())
            });
            let Some((selected, summary, row_details)) = updated else {
                return;
            };
            native_tree_checkboxes::set_check_state(hwnd, item, selected);
            tree.set_item_text(item, &summary);
            let is_selected = tree.get_selection().is_some_and(|selection| {
                native_tree_handle(&selection) == native_tree_handle(item)
            });
            if is_selected {
                details.set_value(&row_details);
            }
        }
        Some(UninstallNode::NotAChoice) => native_tree_checkboxes::clear_check_state(hwnd, item),
        None => {}
    }
}

/// Windows: rebuild the Uninstall list from fresh rows, as the version check
/// does before the Packages page shows. The tab selection is left alone.
pub(crate) fn rebuild_uninstall_list_widgets(
    widgets: &WizardWidgets,
    items: &UninstallStateCell,
    model: &WizardModel,
    list: &WizardUninstallList,
) {
    populate_uninstall_tree(&widgets.uninstall_list, items, model, list);
    widgets
        .uninstall_details
        .set_value(&initial_details(model, list));
}
