//! Windows Backups tab: the same native `TreeCtrl` with real checkboxes as
//! the other tabs, with toggle handling of its own, like the Uninstall
//! list's.
//!
//! The group rows are headings without a checkbox, so one key press can't
//! tick a group. Space on a heading, on the placeholder of an empty list or
//! on a row that can't be ticked for the choice below the list does
//! nothing. What a tick does lives in the wx-free model
//! (`apply_backup_checkbox`): for Delete it ticks the rows of the same set
//! too, and the tree shows each row it changed.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;

use wxdragon::event::tree_events::TreeEventData;
use wxdragon::prelude::*;
use wxdragon::widgets::treectrl::{TreeCtrl, TreeItemId};

use crate::{WizardBackupList, WizardModel, apply_backup_checkbox, set_backup_choice};

use super::native::{build_checkbox_tree, clear_tree_without_taking_focus, native_tree_handle};
use super::native_tree_checkboxes;
use super::{
    BackupsTabWidgets, WXK_SPACE, add_backups_choice, add_backups_details, add_backups_list_label,
    selected_backup_choice, set_text_if_changed, sync_backups_side_widgets,
};

use crate::wx_app::globals::with_ui_localizer;
use crate::wx_app::widgets::WizardWidgets;

/// The Backups tab's list widget.
pub(crate) type BackupsView = TreeCtrl;

/// The live item handles of the Backups tree, kept so its handlers can tell
/// a row from a heading. `TreeItemId` isn't `Copy`, so they can't sit on
/// `WizardWidgets`.
pub(crate) struct BackupItems {
    /// One per row of the list, headings included, in the same order.
    /// `None` for a row the tree refused to add.
    nodes: Vec<Option<TreeItemId>>,
    /// The row, without a checkbox, that stands in for an empty list.
    placeholder: Option<TreeItemId>,
}

pub(crate) type BackupsStateCell = Rc<RefCell<BackupItems>>;

impl BackupItems {
    fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            placeholder: None,
        }
    }
}

pub(crate) fn new_backups_state() -> BackupsStateCell {
    Rc::new(RefCell::new(BackupItems::empty()))
}

thread_local! {
    /// Set when the Packages page opens: the caret goes to the top once the
    /// Backups list itself gets focus, as the other lists' do.
    static CARET_RESET_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// Have the Backups list's caret go to its top row when the list next gets
/// focus, not now: moving it while the tab strip has focus would have NVDA
/// read the row right after the tab.
pub(crate) fn reset_backups_list_caret_on_focus() {
    CARET_RESET_PENDING.with(|pending| pending.set(true));
}

/// Put the caret on the list's top row: the first heading, or the row that
/// says the list is empty.
fn move_caret_to_top(tree: &TreeCtrl, items: &BackupsStateCell) {
    // Clone the handle and drop the borrow before touching the tree:
    // `select_item` runs the selection handler, which borrows `items` too.
    let top = {
        let items = items.borrow();
        items
            .placeholder
            .clone()
            .or_else(|| items.nodes.iter().flatten().next().cloned())
    };
    if let Some(top) = top {
        tree.ensure_visible(&top);
        tree.select_item(&top);
        tree.set_focused_item(&top);
    }
}

/// What a tree item is, by its native handle: wxdragon hands out a fresh
/// `TreeItemId` per event, so the wrappers themselves never compare equal.
#[derive(Clone, Copy)]
enum BackupNode {
    /// A row of the list, heading or backup, by its index.
    Row(usize),
    /// The row that stands in for an empty list.
    Placeholder,
}

fn classify_handle(items: &BackupItems, handle: *mut c_void) -> Option<BackupNode> {
    if handle.is_null() {
        return None;
    }
    if let Some(index) = items.nodes.iter().position(|node| {
        node.as_ref()
            .is_some_and(|node| native_tree_handle(node) == handle)
    }) {
        return Some(BackupNode::Row(index));
    }
    items
        .placeholder
        .as_ref()
        .is_some_and(|item| native_tree_handle(item) == handle)
        .then_some(BackupNode::Placeholder)
}

fn classify(items: &BackupItems, item: &TreeItemId) -> Option<BackupNode> {
    classify_handle(items, native_tree_handle(item))
}

/// Whether `item` is the tree's selected item, the one the caret is on.
fn is_selected(tree: &TreeCtrl, item: &TreeItemId) -> bool {
    tree.get_selection()
        .is_some_and(|selection| native_tree_handle(&selection) == native_tree_handle(item))
}

/// Rebuild the tree from `list`: each heading at the top level with its
/// rows below it, every row's checkbox showing its tick for the list's
/// choice, nothing ticked unless the row says otherwise; or the placeholder
/// alone.
fn populate_backups_tree(tree: &TreeCtrl, items: &BackupsStateCell, list: &WizardBackupList) {
    // Forget the old handles before the tree lets go of them: deleting and
    // adding rows can run the selection handler, and a deleted handle's
    // value may come back for a new row.
    *items.borrow_mut() = BackupItems::empty();
    clear_tree_without_taking_focus(tree);
    let mut fresh = BackupItems::empty();
    let Some(root) = tree.add_root("", None, None) else {
        *items.borrow_mut() = fresh;
        return;
    };
    let hwnd = tree.get_handle();
    let mut placeholder = None;
    with_ui_localizer(|localizer| placeholder = list.placeholder(localizer));
    if let Some((label, _)) = placeholder {
        if let Some(item) = tree.append_item(&root, &label, None, None) {
            native_tree_checkboxes::clear_check_state(hwnd, &item);
            fresh.placeholder = Some(item);
        }
        *items.borrow_mut() = fresh;
        return;
    }
    fresh.nodes = vec![None; list.rows.len()];
    let mut headings: Vec<TreeItemId> = Vec::new();
    for (index, row) in list.rows.iter().enumerate() {
        if row.is_heading() {
            if let Some(heading) = tree.append_item(&root, &row.summary, None, None) {
                native_tree_checkboxes::clear_check_state(hwnd, &heading);
                fresh.nodes[index] = Some(heading.clone());
                headings.push(heading);
            }
            continue;
        }
        // Every row follows a heading; one the tree refused leaves its
        // rows at the top level rather than out of the list.
        let parent = list
            .heading_of(index)
            .and_then(|heading| fresh.nodes[heading].clone())
            .unwrap_or_else(|| root.clone());
        if let Some(item) = tree.append_item(&parent, &row.summary, None, None) {
            native_tree_checkboxes::set_check_state(hwnd, &item, row.is_ticked(list.choice));
            fresh.nodes[index] = Some(item);
        }
    }
    for heading in &headings {
        tree.expand(heading);
    }
    *items.borrow_mut() = fresh;
}

/// Windows: build the Backups tab: the label naming the tab and the total,
/// the list, a read-only details text, and the choice below them, Restore
/// or Delete. Ticking a row changes its text, the rows a Delete tick takes
/// with it, and the details; switching the choice relabels every row
/// without moving focus. Review reads the ticks when Next is pressed.
pub(crate) fn build_backups_tab(
    page: &Panel,
    model: &WizardModel,
    list: Rc<RefCell<WizardBackupList>>,
    items: BackupsStateCell,
) -> BackupsTabWidgets {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let list_label = add_backups_list_label(page, &sizer, &list.borrow());
    let tree = build_checkbox_tree(page, "rabbit-backups-list");
    {
        let items = Rc::clone(&items);
        tree.on_set_focus(move |event| {
            // wxMSW runs this before the tree's own focus handling, so the
            // caret is on the top row by the time the tree reports focus,
            // and that row is the one the screen reader reads.
            if CARET_RESET_PENDING.with(|pending| pending.replace(false)) {
                move_caret_to_top(&tree, &items);
            }
            event.skip(true);
        });
    }
    populate_backups_tree(&tree, &items, &list.borrow());
    sizer.add(&tree, 1, SizerFlag::All | SizerFlag::Expand, 6);

    let details = add_backups_details(page, &sizer, model, &list.borrow());
    let choice = add_backups_choice(page, &sizer, model);

    {
        let items = Rc::clone(&items);
        let list = Rc::clone(&list);
        tree.on_selection_changed(move |event| {
            let Some(item) = event.get_item() else {
                return;
            };
            let node = classify(&items.borrow(), &item);
            // A heading's details say what its group is for, so they show
            // too, unlike on the Uninstall tab.
            let text = match node {
                Some(BackupNode::Row(index)) => {
                    list.borrow().rows.get(index).map(|row| row.details.clone())
                }
                Some(BackupNode::Placeholder) => {
                    let mut text = None;
                    with_ui_localizer(|localizer| {
                        text = list
                            .borrow()
                            .placeholder(localizer)
                            .map(|(_, details)| details);
                    });
                    text
                }
                None => None,
            };
            if let Some(text) = text {
                set_text_if_changed(&details, &text);
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
    // row that can't be ticked for the choice, a heading or the
    // placeholder. Reverting it afterwards would flicker and have the
    // screen reader say "checked" first.
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

    // Restore or Delete: the rows stay the same rows; each shows its action
    // and its tick for the new choice. Focus stays on the choice, and the
    // rows are read with their new action when focus comes back to them.
    {
        let items = Rc::clone(&items);
        let list = Rc::clone(&list);
        choice.on_selected(move |_| {
            let chosen = selected_backup_choice(&choice);
            let rows: Vec<(bool, bool, String)> = {
                let mut list = list.borrow_mut();
                if list.choice == chosen {
                    return;
                }
                set_backup_choice(&mut list, chosen);
                list.rows
                    .iter()
                    .map(|row| (row.is_heading(), row.is_ticked(chosen), row.summary.clone()))
                    .collect()
            };
            let nodes: Vec<Option<TreeItemId>> = items.borrow().nodes.clone();
            let hwnd = tree.get_handle();
            for ((heading, ticked, summary), node) in rows.iter().zip(&nodes) {
                let Some(node) = node else {
                    continue;
                };
                if *heading {
                    continue;
                }
                native_tree_checkboxes::set_check_state(hwnd, node, *ticked);
                set_item_text_if_changed(&tree, node, summary);
            }
            // The selected row's details start with its label, which
            // changed with the choice.
            let selected =
                tree.get_selection()
                    .and_then(|item| match classify(&items.borrow(), &item) {
                        Some(BackupNode::Row(index)) => {
                            list.borrow().rows.get(index).map(|row| row.details.clone())
                        }
                        _ => None,
                    });
            if let Some(text) = selected {
                set_text_if_changed(&details, &text);
            }
        });
    }

    page.set_sizer(sizer, true);
    BackupsTabWidgets {
        list_label,
        list: tree,
        details,
        choice,
    }
}

fn is_space_key(event: &WindowEventData) -> bool {
    matches!(event, WindowEventData::Keyboard(keyboard) if keyboard.get_key_code() == Some(WXK_SPACE))
}

/// Whether the user may flip `node`'s checkbox for the choice below the
/// list.
fn can_toggle(node: Option<BackupNode>, list: &WizardBackupList) -> bool {
    match node {
        Some(BackupNode::Row(index)) => list
            .rows
            .get(index)
            .is_some_and(|row| row.can_tick(list.choice)),
        Some(BackupNode::Placeholder) => false,
        // Not one of ours: leave it to the native control.
        None => true,
    }
}

/// Set a row's text only when it says something else, so a screen reader
/// doesn't read it again.
fn set_item_text_if_changed(tree: &TreeCtrl, item: &TreeItemId, text: &str) {
    if tree.get_item_text(item).as_deref() != Some(text) {
        tree.set_item_text(item, text);
    }
}

/// Bring the list in line with the checkbox the native control just
/// flipped: the row and every row a Delete tick took with it get their tick
/// and their text, and the details follow when the selected row is among
/// them. A row that can't be ticked, a heading or the placeholder goes back
/// to how it was. Applying the same state twice changes nothing, so a toggle
/// that reaches both the click and the key handler is harmless.
fn reconcile_toggle(
    tree: &TreeCtrl,
    items: &BackupsStateCell,
    list: &Rc<RefCell<WizardBackupList>>,
    details: &TextCtrl,
    item: &TreeItemId,
) {
    let hwnd = tree.get_handle();
    let node = classify(&items.borrow(), item);
    match node {
        Some(BackupNode::Row(index)) => {
            let is_heading = match list.borrow().rows.get(index) {
                Some(row) => row.is_heading(),
                None => return,
            };
            if is_heading {
                // A heading never carries a checkbox, as on the Uninstall
                // list: take away any the control gave it.
                native_tree_checkboxes::clear_check_state(hwnd, item);
                return;
            }
            let checked = native_tree_checkboxes::get_check_state(hwnd, item);
            // Take what the widgets need and let the borrows go before
            // touching them: a widget call can run another handler that
            // reads the list or the items.
            let changed: Vec<(Option<TreeItemId>, bool, String, String)> = {
                let mut list = list.borrow_mut();
                let choice = list.choice;
                let change = apply_backup_checkbox(&mut list.rows, index, choice, checked);
                let items = items.borrow();
                std::iter::once(index)
                    .chain(change.tied)
                    .filter_map(|row_index| {
                        let row = list.rows.get(row_index)?;
                        Some((
                            items.nodes.get(row_index).cloned().flatten(),
                            row.is_ticked(choice),
                            row.summary.clone(),
                            row.details.clone(),
                        ))
                    })
                    .collect()
            };
            for (node, ticked, summary, row_details) in changed {
                let Some(node) = node else {
                    continue;
                };
                native_tree_checkboxes::set_check_state(hwnd, &node, ticked);
                set_item_text_if_changed(tree, &node, &summary);
                if is_selected(tree, &node) {
                    set_text_if_changed(details, &row_details);
                }
            }
        }
        Some(BackupNode::Placeholder) => native_tree_checkboxes::clear_check_state(hwnd, item),
        None => {}
    }
}

/// Windows: rebuild the Backups list from fresh rows, as the version check
/// does before the Packages page shows, with its label, details and the
/// choice below it in line with them. The tab selection is left alone.
pub(crate) fn rebuild_backups_list_widgets(
    widgets: &WizardWidgets,
    items: &BackupsStateCell,
    list: &WizardBackupList,
) {
    populate_backups_tree(&widgets.backups_list, items, list);
    sync_backups_side_widgets(
        &widgets.backups_list_label,
        &widgets.backups_details,
        &widgets.backups_choice,
        list,
    );
}
