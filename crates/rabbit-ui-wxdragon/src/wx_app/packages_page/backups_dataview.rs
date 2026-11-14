//! Non-Windows Backups tab: a third `DataViewCtrl` with a model of its own,
//! as the Uninstall list has, so neither of the other lists' toggle code is
//! in it.
//!
//! The group rows are headings without a checkbox, as on the Uninstall
//! tab: the label is model column 0 and the headings are containers, so they
//! show their text and no checkbox. Every row of the list, heading or
//! backup, is one node, by its index in the list's rows; a backup's parent
//! is the heading before it.
//!
//! An empty list shows no row at all, as the empty Uninstall list does: any
//! row here either has a checkbox or reads as a collapsed group. Switching
//! to the tab says the sentence instead (see
//! `WizardBackupList::tab_announcement`), and the details text holds it.
//!
//! Code never selects a row: `select_row` crashes on this kind of model
//! (see `focus_packages_list_top`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wxdragon::prelude::*;
use wxdragon::widgets::dataview::{
    CustomDataViewTreeModel, DataViewAlign, DataViewCellMode, DataViewColumn, DataViewColumnFlags,
    DataViewCtrl, DataViewItem, DataViewStyle, DataViewTextRenderer, DataViewToggleRenderer,
    Variant, VariantType,
};

use crate::{
    WizardBackupList, WizardModel, apply_backup_checkbox, backups_also_ticked_announcement,
    backups_ticked_announcement, set_backup_choice,
};

use super::{
    BackupsTabWidgets, WXK_SPACE, add_backups_choice, add_backups_details, add_backups_list_label,
    selected_backup_choice, set_text_if_changed, sync_backups_side_widgets,
};

use crate::wx_app::globals::{AnnouncePriority, announce, with_ui_localizer};
use crate::wx_app::widgets::WizardWidgets;

/// The Backups tab's list widget.
pub(crate) type BackupsView = DataViewCtrl;

/// The Backups list's model, once built, for the rebuild after the version
/// check. Shared with the model's own callbacks, which notify the view
/// through it.
pub(crate) type BackupsStateCell = Rc<RefCell<Option<CustomDataViewTreeModel>>>;

pub(crate) fn new_backups_state() -> BackupsStateCell {
    Rc::new(RefCell::new(None))
}

const BACKUPS_COL_LABEL: u32 = 0;
const BACKUPS_COL_TOGGLE: u32 = 1;

/// One row of the list, heading or backup: its index in the list's rows.
#[derive(Debug)]
struct BackupNode {
    index: usize,
}

/// The model's userdata. Pointers to its nodes are the view's item ids, so
/// the nodes must not move while the model holds them: the vector is never
/// resized, only replaced as a whole by a rebuild, which tells the view to
/// forget every item.
struct BackupsTreeData {
    list: Rc<RefCell<WizardBackupList>>,
    nodes: Vec<BackupNode>,
}

impl BackupsTreeData {
    fn nodes_for(len: usize) -> Vec<BackupNode> {
        (0..len).map(|index| BackupNode { index }).collect()
    }

    fn node_ptr(&self, index: usize) -> *const BackupNode {
        &self.nodes[index]
    }

    /// The nodes for `indices`, leaving out any past the nodes there are.
    fn node_ptrs(&self, indices: Vec<usize>) -> Vec<*mut BackupNode> {
        indices
            .into_iter()
            .filter(|index| *index < self.nodes.len())
            .map(|index| self.node_ptr(index) as *mut BackupNode)
            .collect()
    }

    fn is_heading(&self, index: usize) -> bool {
        self.list
            .borrow()
            .rows
            .get(index)
            .is_some_and(|row| row.is_heading())
    }
}

/// Tick or untick a row, with every side effect: the row and every row a
/// Delete tick took with it, in the view, and the details when the selected
/// row is among them, as the Windows tree's toggle does. `None` when
/// nothing could change, because the row can't be ticked for the choice;
/// otherwise what VoiceOver should hear beyond the checkbox, which is the
/// rows from the same set that were ticked or unticked too, if any were.
/// Shared by the model's `set_value` (a click, or VO+Space while
/// interacting with the list) and the Space handler, which wx never routes
/// through `set_value` on macOS, so the two can't drift apart.
type BackupsToggle = Rc<dyn Fn(&BackupsTreeData, usize, bool) -> Option<Option<String>>>;

/// The details text below the list. It is created after the list, because
/// creation order is tab order, so the toggle reaches it through this cell
/// once it exists.
type DetailsCell = Rc<Cell<Option<TextCtrl>>>;

fn new_backups_toggle(
    tree: DataViewCtrl,
    model_cell: BackupsStateCell,
    details: DetailsCell,
) -> BackupsToggle {
    Rc::new(
        move |data: &BackupsTreeData, index: usize, checked: bool| -> Option<Option<String>> {
            // Let the list go before notifying the view, which reads it back.
            let (changed, note) = {
                let mut list = data.list.borrow_mut();
                let choice = list.choice;
                if !list.rows.get(index)?.can_tick(choice) {
                    return None;
                }
                let change = apply_backup_checkbox(&mut list.rows, index, choice, checked);
                let mut note = None;
                with_ui_localizer(|localizer| {
                    note = backups_also_ticked_announcement(localizer, &list.rows, index, &change);
                });
                let changed: Vec<(usize, String)> = std::iter::once(index)
                    .chain(change.tied)
                    .filter_map(|row| Some((row, list.rows.get(row)?.details.clone())))
                    .collect();
                (changed, note)
            };
            let selected = tree
                .get_selection()
                .and_then(|item| item.get_id::<BackupNode>());
            for (row, row_details) in changed {
                if row >= data.nodes.len() {
                    continue;
                }
                let row_ptr = data.node_ptr(row);
                if let Some(model) = model_cell.borrow().as_ref() {
                    // The toggle cell refreshes by itself after `set_value`,
                    // the label, which starts with the action, doesn't, and
                    // neither do the rows a Delete tick took with it.
                    model.item_value_changed(row_ptr, BACKUPS_COL_LABEL);
                    model.item_value_changed(row_ptr, BACKUPS_COL_TOGGLE);
                }
                if selected.is_some_and(|selected| std::ptr::eq(selected, row_ptr))
                    && let Some(details) = details.get()
                {
                    set_text_if_changed(&details, &row_details);
                }
            }
            Some(note)
        },
    )
}

fn build_backups_tree_model(
    data: BackupsTreeData,
    toggle: BackupsToggle,
) -> CustomDataViewTreeModel {
    type CompareFn = fn(&BackupsTreeData, &BackupNode, &BackupNode, u32, bool) -> i32;
    CustomDataViewTreeModel::new(
        data,
        // get_parent: a backup's heading; a heading has none.
        |data: &BackupsTreeData, item: Option<&BackupNode>| -> Option<*mut BackupNode> {
            let node = item?;
            let heading = data.list.borrow().heading_of(node.index)?;
            (heading < data.nodes.len()).then(|| data.node_ptr(heading) as *mut BackupNode)
        },
        // is_container: the headings, so that they show only column 0, their
        // text, and no checkbox. Nothing else may be one: wxOSX gives every
        // container a disclosure triangle, children or not.
        |data: &BackupsTreeData, item: Option<&BackupNode>| -> bool {
            item.is_none_or(|node| data.is_heading(node.index))
        },
        // get_children: no heading, and so no row at all, for an empty list.
        |data: &BackupsTreeData, item: Option<&BackupNode>| -> Vec<*mut BackupNode> {
            let indices = match item {
                None => data.list.borrow().heading_indices(),
                Some(node) => data.list.borrow().rows_under(node.index),
            };
            data.node_ptrs(indices)
        },
        // get_value
        |data: &BackupsTreeData, item: Option<&BackupNode>, col: u32| -> Variant {
            let Some(node) = item else {
                return Variant::from_string("");
            };
            let list = data.list.borrow();
            let row = list.rows.get(node.index);
            if col == BACKUPS_COL_TOGGLE {
                return Variant::from_bool(
                    row.is_some_and(|row| !row.is_heading() && row.is_ticked(list.choice)),
                );
            }
            Variant::from_string(row.map(|row| row.summary.as_str()).unwrap_or_default())
        },
        // set_value
        Some(
            move |data: &BackupsTreeData,
                  item: Option<&BackupNode>,
                  col: u32,
                  var: &Variant|
                  -> bool {
                if col != BACKUPS_COL_TOGGLE {
                    return false;
                }
                let Some(index) = item.map(|node| node.index) else {
                    return false;
                };
                if data.is_heading(index) {
                    return false;
                }
                let Some(note) = toggle(data, index, var.get_bool().unwrap_or(false)) else {
                    return false;
                };
                // A click, or VO+Space: VoiceOver reads the checkbox itself,
                // so only the rows from the same set are left to say, after
                // it.
                if let Some(note) = note {
                    announce(&note, AnnouncePriority::Polite);
                }
                true
            },
        ),
        // is_enabled: a row that can't be ticked for the choice is drawn,
        // and exposed, as disabled. The headings read as plain text.
        Some(
            |data: &BackupsTreeData, item: Option<&BackupNode>, _col: u32| -> bool {
                let Some(node) = item else {
                    return true;
                };
                let list = data.list.borrow();
                list.rows
                    .get(node.index)
                    .is_none_or(|row| row.is_heading() || row.can_tick(list.choice))
            },
        ),
        None::<CompareFn>,
    )
}

/// Expand every heading, so the rows are visible without a click: wx
/// collapses them on every `cleared()`.
fn expand_backup_groups(tree: &DataViewCtrl, model: &CustomDataViewTreeModel) {
    let headings = model
        .with_userdata_mut::<BackupsTreeData, _>(|data| {
            let indices = data.list.borrow().heading_indices();
            data.node_ptrs(indices)
        })
        .unwrap_or_default();
    for ptr in headings {
        let item = DataViewItem::from_id_ptr(ptr as *const BackupNode);
        if item.is_ok() {
            tree.expand(&item);
        }
    }
}

/// Have every backup's row read again: its label and its tick, after the
/// choice below the list changed them all. The headings stay as they are.
fn refresh_every_row(model: &CustomDataViewTreeModel) {
    let rows = model
        .with_userdata_mut::<BackupsTreeData, _>(|data| {
            (0..data.nodes.len())
                .filter(|index| !data.is_heading(*index))
                .map(|index| data.node_ptr(index))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for ptr in rows {
        model.item_value_changed(ptr, BACKUPS_COL_LABEL);
        model.item_value_changed(ptr, BACKUPS_COL_TOGGLE);
    }
}

/// Non-Windows twin of the tree-based helper: nothing to do. A freshly
/// rebuilt DataView has no selection, so reading starts at the top.
pub(crate) fn reset_backups_list_caret_on_focus() {}

/// Non-Windows: build the Backups tab: the label naming the tab and the
/// total, the list, a read-only details text, and the choice below them,
/// Restore or Delete. Ticking a row changes its text, the rows a Delete
/// tick takes with it, and the details; switching the choice relabels every
/// row without moving focus and says how many are ticked for it. Review
/// reads the ticks when Next is pressed.
pub(crate) fn build_backups_tab(
    page: &Panel,
    model: &WizardModel,
    list: Rc<RefCell<WizardBackupList>>,
    items: BackupsStateCell,
) -> BackupsTabWidgets {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    // Created right before the list: wxWidgets 3.3.4 gives the list its
    // VoiceOver name from the label shown just before it, and it names the
    // tab.
    let list_label = add_backups_list_label(page, &sizer, &list.borrow());
    let tree = DataViewCtrl::builder(page)
        .with_style(DataViewStyle::Single | DataViewStyle::RowLines | DataViewStyle::NoHeader)
        .with_size(Size::new(-1, 220))
        .build();
    tree.set_name("rabbit-backups-list");
    // The same floor as the other lists: a list squeezed to no height drops
    // out of the accessibility tree.
    tree.set_min_size(Size::new(-1, 160));

    let details_cell: DetailsCell = Rc::new(Cell::new(None));
    let toggle = new_backups_toggle(tree, Rc::clone(&items), Rc::clone(&details_cell));
    let row_count = list.borrow().rows.len();
    let data = BackupsTreeData {
        list: Rc::clone(&list),
        nodes: BackupsTreeData::nodes_for(row_count),
    };
    let dv_model = build_backups_tree_model(data, Rc::clone(&toggle));
    *items.borrow_mut() = Some(dv_model.clone());

    // View column order matches the other lists, checkbox first; the model
    // columns are the other way round, see the module comment.
    let toggle_renderer = DataViewToggleRenderer::new(
        VariantType::Bool,
        DataViewCellMode::Activatable,
        DataViewAlign::Center,
    );
    tree.append_column(DataViewColumn::new(
        "",
        toggle_renderer,
        BACKUPS_COL_TOGGLE as usize,
        28,
        DataViewAlign::Center,
        DataViewColumnFlags::DefaultNone,
    ));
    let text_renderer = DataViewTextRenderer::new(
        VariantType::String,
        DataViewCellMode::Inert,
        DataViewAlign::Left,
    );
    tree.append_column(DataViewColumn::new(
        "",
        text_renderer,
        BACKUPS_COL_LABEL as usize,
        -1,
        DataViewAlign::Left,
        DataViewColumnFlags::Resizable,
    ));
    tree.associate_model(&dv_model);
    expand_backup_groups(&tree, &dv_model);
    sizer.add(&tree, 1, SizerFlag::All | SizerFlag::Expand, 6);

    let details = add_backups_details(page, &sizer, model, &list.borrow());
    details_cell.set(Some(details));
    let choice = add_backups_choice(page, &sizer, model);

    {
        let list = Rc::clone(&list);
        tree.on_selection_changed(move |event| {
            let Some(node_ptr) = event
                .get_item()
                .and_then(|item| item.get_id::<BackupNode>())
            else {
                return;
            };
            if node_ptr.is_null() {
                return;
            }
            // SAFETY: the pointer came from a BackupNode owned by the
            // model's userdata, which only a rebuild replaces, and a rebuild
            // can't run inside this synchronous handler.
            let node = unsafe { &*node_ptr };
            // A heading's details say what its group is for, so they show
            // too, unlike on the Uninstall tab.
            let value = list
                .borrow()
                .rows
                .get(node.index)
                .map(|row| row.details.clone());
            if let Some(value) = value {
                set_text_if_changed(&details, &value);
            }
        });
    }

    // Space ticks the selected row. wx on macOS only toggles a checkbox cell
    // on a click, so without this the list couldn't be ticked from the
    // keyboard (issue #28, as on the other lists).
    {
        let items = Rc::clone(&items);
        let model_text = model.text.clone();
        tree.on_key_down(move |event| {
            let WindowEventData::Keyboard(keyboard) = &event else {
                return;
            };
            if keyboard.get_key_code() != Some(WXK_SPACE)
                || keyboard.cmd_down()
                || keyboard.control_down()
                || keyboard.alt_down()
                || keyboard.shift_down()
            {
                return;
            }
            let Some(node_ptr) = tree
                .get_selection()
                .and_then(|item| item.get_id::<BackupNode>())
            else {
                return;
            };
            if node_ptr.is_null() {
                return;
            }
            // Consume the key either way: the list has nothing else to do
            // with Space, and letting it through beeps. A heading or a row
            // that can't be ticked just stays as it is.
            event.skip(false);
            let Some(dv_model) = items.borrow().clone() else {
                return;
            };
            // Only hold the userdata borrow long enough to take its address:
            // the toggle notifies the view, which reads the model back
            // through that same RefCell.
            let Some(data_ptr) = dv_model
                .with_userdata_mut::<BackupsTreeData, _>(|data| data as *const BackupsTreeData)
            else {
                return;
            };
            // SAFETY: both pointers address heap storage owned by the model's
            // userdata, which only a rebuild replaces, and no rebuild can run
            // inside this synchronous handler.
            let (data, node) = unsafe { (&*data_ptr, &*node_ptr) };
            let index = node.index;
            let is_ticked = |data: &BackupsTreeData| {
                let list = data.list.borrow();
                list.rows
                    .get(index)
                    .is_some_and(|row| row.is_ticked(list.choice))
            };
            let checked = is_ticked(data);
            let Some(note) = toggle(data, index, !checked) else {
                return;
            };
            // The row VoiceOver is on changed behind it, so it says nothing
            // on its own: say the state the row ended up in, then the rows
            // from the same set that went with it. One announcement, not
            // two: a second would cut the first off.
            let state = if is_ticked(data) {
                &model_text.packages_row_checked
            } else {
                &model_text.packages_row_unchecked
            };
            let message = std::iter::once(state.as_str())
                .chain(note.as_deref())
                .collect::<Vec<_>>()
                .join(". ");
            announce(&message, AnnouncePriority::Interrupt);
        });
    }

    // Restore or Delete: the rows stay the same rows; each shows its action
    // and its tick for the new choice. Focus stays on the choice, so the
    // rows changed away from it: VoiceOver says how many are ticked for the
    // new choice, politely, after it reads the choice itself.
    {
        let items = Rc::clone(&items);
        let list = Rc::clone(&list);
        choice.on_selected(move |_| {
            let chosen = selected_backup_choice(&choice);
            {
                let mut list = list.borrow_mut();
                if list.choice == chosen {
                    return;
                }
                set_backup_choice(&mut list, chosen);
            }
            let Some(dv_model) = items.borrow().clone() else {
                return;
            };
            refresh_every_row(&dv_model);
            // The selected row's details start with its label, which
            // changed with the choice.
            let selected = tree
                .get_selection()
                .and_then(|item| item.get_id::<BackupNode>())
                .filter(|node_ptr| !node_ptr.is_null());
            if let Some(node_ptr) = selected {
                // SAFETY: as in the selection handler above.
                let node = unsafe { &*node_ptr };
                let value = list
                    .borrow()
                    .rows
                    .get(node.index)
                    .map(|row| row.details.clone());
                if let Some(value) = value {
                    set_text_if_changed(&details, &value);
                }
            }
            let mut message = None;
            with_ui_localizer(|localizer| {
                message = Some(backups_ticked_announcement(localizer, &list.borrow()));
            });
            if let Some(message) = message {
                announce(&message, AnnouncePriority::Polite);
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

/// Non-Windows: rebuild the Backups list after the rows were replaced, as
/// the version check does before the Packages page shows, with its label,
/// details and the choice below it in line with them. The model and its
/// association stay; it gets fresh nodes and tells the view everything
/// changed. Never selects a row: see `focus_packages_list_top` for why
/// `select_row` crashes on this kind of model.
pub(crate) fn rebuild_backups_list_widgets(
    widgets: &WizardWidgets,
    items: &BackupsStateCell,
    list: &WizardBackupList,
) {
    let Some(dv_model) = items.borrow().clone() else {
        return;
    };
    let row_count = list.rows.len();
    dv_model.with_userdata_mut::<BackupsTreeData, ()>(|data| {
        data.nodes = BackupsTreeData::nodes_for(row_count);
    });
    dv_model.cleared();
    expand_backup_groups(&widgets.backups_list, &dv_model);
    sync_backups_side_widgets(
        &widgets.backups_list_label,
        &widgets.backups_details,
        &widgets.backups_choice,
        list,
    );
}
