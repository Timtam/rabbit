//! Non-Windows Uninstall tab: a second `DataViewCtrl` with a model of its
//! own beside the install list's, so the install list's toggle code stays
//! out of it.
//!
//! The group rows are headings without a checkbox. wx shows only model
//! column 0 of a container row unless the model asks for more, so the label
//! is column 0 here and the headings are containers: they show their text
//! and no checkbox. The install model does the opposite, on purpose, so its
//! group rows can tick everything below them.
//!
//! An empty list shows no row at all, unlike the Windows list's placeholder
//! row. Whether a cell has a value follows from being a container alone, and
//! wxOSX tells NSOutlineView every container is expandable: a leaf would show
//! a checkbox VoiceOver reads as "unchecked", a container a disclosure
//! triangle it reads as "collapsed", over nothing. The details text holds
//! the sentence instead, and switching to the tab says it (see
//! `WizardUninstallList::tab_announcement`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rabbit_core::package::PackageCategory;
use wxdragon::prelude::*;
use wxdragon::widgets::dataview::{
    CustomDataViewTreeModel, DataViewAlign, DataViewCellMode, DataViewColumn, DataViewColumnFlags,
    DataViewCtrl, DataViewItem, DataViewStyle, DataViewTextRenderer, DataViewToggleRenderer,
    Variant, VariantType,
};

use crate::{WizardModel, WizardText, WizardUninstallList, apply_uninstall_checkbox};

use super::{UninstallSideWidgets, WXK_SPACE, build_uninstall_settings};

use crate::wx_app::globals::{AnnouncePriority, announce};
use crate::wx_app::pages::add_label;
use crate::wx_app::widgets::{WizardWidgets, sync_uninstall_settings_widgets};

/// The Uninstall tab's list widget.
pub(crate) type UninstallView = DataViewCtrl;

/// The Uninstall list's model, once built, for the rebuild after the version
/// check. Shared with the model's own callbacks, which notify the view
/// through it.
pub(crate) type UninstallStateCell = Rc<RefCell<Option<CustomDataViewTreeModel>>>;

pub(crate) fn new_uninstall_state() -> UninstallStateCell {
    Rc::new(RefCell::new(None))
}

const UNINSTALL_COL_LABEL: u32 = 0;
const UNINSTALL_COL_TOGGLE: u32 = 1;

/// The groups, in the install tree's order.
const GROUP_ORDER: [PackageCategory; 3] = [
    PackageCategory::Core,
    PackageCategory::Additional,
    PackageCategory::Language,
];

#[derive(Clone, Copy, Debug)]
enum UninstallNodeKind {
    /// A heading: the index into `GROUP_ORDER`.
    Group(usize),
    /// An index into the list's rows.
    Row(usize),
}

#[derive(Debug)]
struct UninstallNode {
    kind: UninstallNodeKind,
}

/// The model's userdata. Pointers to its nodes are the view's item ids, so
/// the nodes must not move while the model holds them: the boxed ones stay
/// where they are on the heap, and the row vector is never resized, only
/// replaced as a whole by a rebuild, which tells the view to forget every
/// item.
struct UninstallTreeData {
    list: Rc<RefCell<WizardUninstallList>>,
    text: WizardText,
    group_nodes: [Box<UninstallNode>; 3],
    row_nodes: Vec<UninstallNode>,
}

impl UninstallTreeData {
    fn row_nodes_for(len: usize) -> Vec<UninstallNode> {
        (0..len)
            .map(|index| UninstallNode {
                kind: UninstallNodeKind::Row(index),
            })
            .collect()
    }

    fn group_label(&self, group: usize) -> &str {
        match GROUP_ORDER[group] {
            PackageCategory::Core => &self.text.packages_tree_group_label,
            PackageCategory::Additional => &self.text.additional_software_tree_group_label,
            PackageCategory::Language => &self.text.language_tree_group_label,
        }
    }

    fn group_ptr(&self, group: usize) -> *const UninstallNode {
        self.group_nodes[group].as_ref()
    }

    fn row_ptr(&self, index: usize) -> *const UninstallNode {
        &self.row_nodes[index]
    }

    /// The groups that have rows, as indices into `GROUP_ORDER`.
    fn groups_with_rows(&self) -> Vec<usize> {
        let list = self.list.borrow();
        (0..GROUP_ORDER.len())
            .filter(|group| {
                list.rows
                    .iter()
                    .any(|row| row.category == GROUP_ORDER[*group])
            })
            .collect()
    }

    fn rows_in_group(&self, group: usize) -> Vec<*mut UninstallNode> {
        let list = self.list.borrow();
        list.rows
            .iter()
            .enumerate()
            .filter(|(index, row)| {
                row.category == GROUP_ORDER[group] && *index < self.row_nodes.len()
            })
            .map(|(index, _)| self.row_ptr(index) as *mut UninstallNode)
            .collect()
    }
}

/// The details text to start from: the first row's, or, for an empty list,
/// why it is empty.
fn initial_details(model: &WizardModel, list: &WizardUninstallList) -> String {
    match list.rows.first() {
        Some(row) => row.details.clone(),
        None => list
            .placeholder(&model.text)
            .map(|(_, details)| details)
            .unwrap_or_default(),
    }
}

/// Tick or untick a row, with every side effect: the row's text, the view,
/// the details when it is the selected row, and the settings option, as
/// the Windows tree's toggle does. `None` when nothing could change,
/// because the row can't be ticked; otherwise what the caller should tell
/// VoiceOver beyond the checkbox, which is the settings option coming or
/// going, if it did. Shared by the model's `set_value` (a click, or
/// VO+Space while interacting with the list) and the Space handler, which
/// wx never routes through `set_value` on macOS, so the two can't drift
/// apart.
type UninstallToggle = Rc<dyn Fn(&UninstallTreeData, usize, bool) -> Option<Vec<String>>>;

/// The widgets below the list. They are created after it, because creation
/// order is tab order, so they reach the toggle through this cell once they
/// exist, as the install list's do.
type UninstallSideCell = Rc<Cell<Option<UninstallSideWidgets>>>;

fn new_uninstall_toggle(
    tree: DataViewCtrl,
    model_cell: UninstallStateCell,
    side: UninstallSideCell,
) -> UninstallToggle {
    Rc::new(
        move |data: &UninstallTreeData, index: usize, checked: bool| -> Option<Vec<String>> {
            // Let the list go before notifying the view, which reads it back.
            let row_details = data
                .list
                .borrow_mut()
                .rows
                .get_mut(index)
                .filter(|row| row.can_tick)
                .map(|row| {
                    apply_uninstall_checkbox(row, checked);
                    row.details.clone()
                })?;
            if index >= data.row_nodes.len() {
                return None;
            }
            let row_ptr = data.row_ptr(index);
            if let Some(model) = model_cell.borrow().as_ref() {
                // The toggle cell refreshes by itself after `set_value`, the
                // label, which starts with the action, doesn't.
                model.item_value_changed(row_ptr, UNINSTALL_COL_LABEL);
                model.item_value_changed(row_ptr, UNINSTALL_COL_TOGGLE);
            }
            let Some(side) = side.get() else {
                return Some(Vec::new());
            };
            let is_selected = tree
                .get_selection()
                .and_then(|item| item.get_id::<UninstallNode>())
                .is_some_and(|selected| std::ptr::eq(selected, row_ptr));
            if is_selected {
                side.details.set_value(&row_details);
            }
            Some(
                side.sync_settings(&data.list.borrow())
                    .into_iter()
                    .collect(),
            )
        },
    )
}

fn build_uninstall_tree_model(
    data: UninstallTreeData,
    toggle: UninstallToggle,
) -> CustomDataViewTreeModel {
    type CompareFn = fn(&UninstallTreeData, &UninstallNode, &UninstallNode, u32, bool) -> i32;
    CustomDataViewTreeModel::new(
        data,
        // get_parent
        |data: &UninstallTreeData, item: Option<&UninstallNode>| -> Option<*mut UninstallNode> {
            let node = item?;
            match node.kind {
                UninstallNodeKind::Row(index) => {
                    let category = data.list.borrow().rows.get(index)?.category;
                    let group = GROUP_ORDER.iter().position(|group| *group == category)?;
                    Some(data.group_ptr(group) as *mut UninstallNode)
                }
                UninstallNodeKind::Group(_) => None,
            }
        },
        // is_container: the headings, so that they show only column 0, their
        // text, and no checkbox. Nothing else may be one: wxOSX gives every
        // container a disclosure triangle, children or not.
        |_data: &UninstallTreeData, item: Option<&UninstallNode>| -> bool {
            item.is_none_or(|node| matches!(node.kind, UninstallNodeKind::Group(_)))
        },
        // get_children: no heading, and so no row at all, for an empty list.
        |data: &UninstallTreeData, item: Option<&UninstallNode>| -> Vec<*mut UninstallNode> {
            match item.map(|node| node.kind) {
                None => data
                    .groups_with_rows()
                    .into_iter()
                    .map(|group| data.group_ptr(group) as *mut UninstallNode)
                    .collect(),
                Some(UninstallNodeKind::Group(group)) => data.rows_in_group(group),
                Some(UninstallNodeKind::Row(_)) => Vec::new(),
            }
        },
        // get_value
        |data: &UninstallTreeData, item: Option<&UninstallNode>, col: u32| -> Variant {
            let Some(node) = item else {
                return Variant::from_string("");
            };
            let list = data.list.borrow();
            if col == UNINSTALL_COL_TOGGLE {
                return Variant::from_bool(match node.kind {
                    UninstallNodeKind::Row(index) => {
                        list.rows.get(index).is_some_and(|row| row.selected)
                    }
                    UninstallNodeKind::Group(_) => false,
                });
            }
            match node.kind {
                UninstallNodeKind::Group(group) => Variant::from_string(data.group_label(group)),
                UninstallNodeKind::Row(index) => Variant::from_string(
                    list.rows
                        .get(index)
                        .map(|row| row.summary.as_str())
                        .unwrap_or_default(),
                ),
            }
        },
        // set_value
        Some(
            move |data: &UninstallTreeData,
                  item: Option<&UninstallNode>,
                  col: u32,
                  var: &Variant|
                  -> bool {
                if col != UNINSTALL_COL_TOGGLE {
                    return false;
                }
                let Some(UninstallNodeKind::Row(index)) = item.map(|node| node.kind) else {
                    return false;
                };
                let Some(notes) = toggle(data, index, var.get_bool().unwrap_or(false)) else {
                    return false;
                };
                // A click, or VO+Space: VoiceOver reads the checkbox itself,
                // so only the settings option coming or going is left to
                // say, after it.
                if !notes.is_empty() {
                    announce(&notes.join(" "), AnnouncePriority::Polite);
                }
                true
            },
        ),
        // is_enabled: a row that can't be ticked is drawn, and exposed, as
        // disabled. The headings read as plain text.
        Some(
            |data: &UninstallTreeData, item: Option<&UninstallNode>, _col: u32| -> bool {
                match item.map(|node| node.kind) {
                    Some(UninstallNodeKind::Row(index)) => data
                        .list
                        .borrow()
                        .rows
                        .get(index)
                        .is_none_or(|row| row.can_tick),
                    _ => true,
                }
            },
        ),
        None::<CompareFn>,
    )
}

/// Expand every heading, so the rows are visible without a click: wx
/// collapses them on every `cleared()`.
fn expand_uninstall_groups(tree: &DataViewCtrl, model: &CustomDataViewTreeModel) {
    let groups = model
        .with_userdata_mut::<UninstallTreeData, _>(|data| {
            data.groups_with_rows()
                .into_iter()
                .map(|group| data.group_ptr(group))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for ptr in groups {
        let item = DataViewItem::from_id_ptr(ptr);
        if item.is_ok() {
            tree.expand(&item);
        }
    }
}

/// Non-Windows: build the Uninstall tab: the label naming the tab, the
/// list, a read-only details text and "Also remove settings". Ticking a row
/// changes its text, the details and the settings option; the run reads the
/// ticks when it starts.
pub(crate) fn build_uninstall_tab(
    page: &Panel,
    model: &WizardModel,
    list: Rc<RefCell<WizardUninstallList>>,
    items: UninstallStateCell,
) -> (UninstallView, TextCtrl, CheckBox, TextCtrl) {
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    // Created right before the list: wxWidgets 3.3.4 gives the list its
    // VoiceOver name from the label shown just before it, and it names the
    // tab.
    add_label(
        page,
        &sizer,
        &model.text.uninstall_list_label,
        "rabbit-uninstall-list-label",
    );
    let tree = DataViewCtrl::builder(page)
        .with_style(DataViewStyle::Single | DataViewStyle::RowLines | DataViewStyle::NoHeader)
        .with_size(Size::new(-1, 220))
        .build();
    tree.set_name("rabbit-uninstall-list");
    // The same floor as the install list: a list squeezed to no height drops
    // out of the accessibility tree.
    tree.set_min_size(Size::new(-1, 160));

    let side_cell: UninstallSideCell = Rc::new(Cell::new(None));
    let toggle = new_uninstall_toggle(tree, Rc::clone(&items), Rc::clone(&side_cell));
    let row_count = list.borrow().rows.len();
    let data = UninstallTreeData {
        list: Rc::clone(&list),
        text: model.text.clone(),
        group_nodes: [0, 1, 2].map(|group| {
            Box::new(UninstallNode {
                kind: UninstallNodeKind::Group(group),
            })
        }),
        row_nodes: UninstallTreeData::row_nodes_for(row_count),
    };
    let dv_model = build_uninstall_tree_model(data, Rc::clone(&toggle));
    *items.borrow_mut() = Some(dv_model.clone());

    // View column order matches the install list, checkbox first; the model
    // columns are the other way round, see the module comment.
    let toggle_renderer = DataViewToggleRenderer::new(
        VariantType::Bool,
        DataViewCellMode::Activatable,
        DataViewAlign::Center,
    );
    tree.append_column(DataViewColumn::new(
        "",
        toggle_renderer,
        UNINSTALL_COL_TOGGLE as usize,
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
        UNINSTALL_COL_LABEL as usize,
        -1,
        DataViewAlign::Left,
        DataViewColumnFlags::Resizable,
    ));
    tree.associate_model(&dv_model);
    expand_uninstall_groups(&tree, &dv_model);
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

    let (remove_settings, settings_note) =
        build_uninstall_settings(page, &sizer, model, &list.borrow());
    side_cell.set(Some(UninstallSideWidgets {
        details,
        remove_settings,
        settings_note,
    }));

    {
        let list = Rc::clone(&list);
        tree.on_selection_changed(move |event| {
            let Some(node_ptr) = event
                .get_item()
                .and_then(|item| item.get_id::<UninstallNode>())
            else {
                return;
            };
            if node_ptr.is_null() {
                return;
            }
            // SAFETY: the pointer came from an UninstallNode owned by the
            // model's userdata, which only a rebuild replaces, and a rebuild
            // can't run inside this synchronous handler.
            let node = unsafe { &*node_ptr };
            let value = match node.kind {
                UninstallNodeKind::Row(index) => {
                    list.borrow().rows.get(index).map(|row| row.details.clone())
                }
                // A heading leaves the details as they were, as the install
                // list's groups do.
                UninstallNodeKind::Group(_) => None,
            };
            if let Some(value) = value {
                details.set_value(&value);
            }
        });
    }

    // Space ticks the selected row. wx on macOS only toggles a checkbox cell
    // on a click, so without this the list couldn't be ticked from the
    // keyboard (issue #28, as on the install list).
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
                .and_then(|item| item.get_id::<UninstallNode>())
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
                .with_userdata_mut::<UninstallTreeData, _>(|data| data as *const UninstallTreeData)
            else {
                return;
            };
            // SAFETY: both pointers address heap storage owned by the model's
            // userdata, which only a rebuild replaces, and no rebuild can run
            // inside this synchronous handler.
            let (data, node) = unsafe { (&*data_ptr, &*node_ptr) };
            let UninstallNodeKind::Row(index) = node.kind else {
                return;
            };
            let checked = data
                .list
                .borrow()
                .rows
                .get(index)
                .is_some_and(|row| row.selected);
            let Some(notes) = toggle(data, index, !checked) else {
                return;
            };
            // The row VoiceOver is on changed behind it, so it says nothing
            // on its own: say the state the row ended up in, then the
            // settings option if it came or went. One announcement, not
            // two: a second would cut the first off.
            let now_checked = data
                .list
                .borrow()
                .rows
                .get(index)
                .is_some_and(|row| row.selected);
            let state = if now_checked {
                &model_text.packages_row_checked
            } else {
                &model_text.packages_row_unchecked
            };
            let message = std::iter::once(state.as_str())
                .chain(notes.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(". ");
            announce(&message, AnnouncePriority::Interrupt);
        });
    }

    page.set_sizer(sizer, true);
    (tree, details, remove_settings, settings_note)
}

/// Non-Windows: rebuild the Uninstall list after the rows were replaced, as
/// the version check does before the Packages page shows. The model and its
/// association stay; it gets fresh nodes and tells the view everything
/// changed, and the settings option starts over with the fresh, unticked
/// rows. Never selects a row: see `focus_packages_list_top` for why
/// `select_row` crashes on this kind of model.
pub(crate) fn rebuild_uninstall_list_widgets(
    widgets: &WizardWidgets,
    items: &UninstallStateCell,
    model: &WizardModel,
    list: &WizardUninstallList,
) {
    let Some(dv_model) = items.borrow().clone() else {
        return;
    };
    let row_count = list.rows.len();
    dv_model.with_userdata_mut::<UninstallTreeData, ()>(|data| {
        data.row_nodes = UninstallTreeData::row_nodes_for(row_count);
    });
    dv_model.cleared();
    expand_uninstall_groups(&widgets.uninstall_list, &dv_model);
    widgets
        .uninstall_details
        .set_value(&initial_details(model, list));
    sync_uninstall_settings_widgets(
        list,
        &widgets.uninstall_remove_settings,
        &widgets.uninstall_settings_note,
    );
}
