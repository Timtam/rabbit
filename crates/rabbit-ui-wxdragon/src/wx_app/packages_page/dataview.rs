//! Non-Windows packages page: a `DataViewCtrl` tree with a toggle column,
//! backed by a hand-rolled model.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::{
    ConfigurationRow, OsaraKeymapChoice, PackageRow, WizardInstallOptions, WizardModel,
    apply_checkbox_state_to_package_row, localizer_from_options, osara_keymap_note,
    recompute_configuration_row_availability,
};
use rabbit_core::plan::PlanActionKind;
use wxdragon::widgets::dataview::{
    CustomDataViewTreeModel, DataViewAlign, DataViewCellMode, DataViewColumn, DataViewColumnFlags,
    DataViewCtrl, DataViewEventHandler, DataViewStyle, DataViewTextRenderer,
    DataViewToggleRenderer, Variant, VariantType,
};

use wxdragon::prelude::*;

use super::{PackagesStateCell, PackagesView, WXK_NUMPAD_ENTER, WXK_RETURN, WXK_SPACE};

use crate::wx_app::pages::{WizardPage, add_heading, add_label};
use crate::wx_app::widgets::{
    REAPER_LANGUAGE_LABEL_NAME, SPANISH_VARIANT_LABEL_NAME, WizardWidgets, osara_keymap_choice,
    package_details, sync_osara_keymap_widgets, sync_reaper_language_widget,
    sync_spanish_variant_widget,
};

/// Identifies a row in the non-Windows `CustomDataViewTreeModel`. `Package`
/// carries the index into `package_rows`; `Group` is the synthetic
/// "Packages" parent under the invisible root. The `Box<Node>` storage
/// owned by `PackageTreeData` is heap-stable, so `*mut Node` pointers
/// passed across the FFI boundary as opaque item ids stay valid for the
/// model's lifetime.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NodeKind {
    /// The synthetic "Packages" parent under the invisible root.
    PackagesGroup,
    /// The synthetic "Additional software" parent, sibling of PackagesGroup.
    /// Holds the `Additional`-category package leaves (Surge XT, app2clap, …).
    AdditionalSoftwareGroup,
    /// The synthetic "Language packs" parent, sibling of PackagesGroup.
    /// Holds the `Language`-category package leaves (REAPER translations).
    LanguageGroup,
    /// A package leaf — index into `package_rows`. Parented under either
    /// PackagesGroup or AdditionalSoftwareGroup depending on the row's
    /// category; the index is into the full `package_rows` either way.
    Package(usize),
    /// The synthetic "Configuration" parent, sibling of PackagesGroup.
    ConfigurationGroup,
    /// A configuration-step leaf — index into `configuration_rows`.
    Configuration(usize),
}

#[derive(Debug)]
pub(crate) struct Node {
    pub(crate) kind: NodeKind,
}

/// Userdata stored inside the non-Windows `CustomDataViewTreeModel`. Owns
/// the heap-stable node objects we hand to wxDataView as item ids, and
/// holds clones of the shared `package_rows` and `configuration_rows`
/// Rcs so model callbacks can read row state without going through any
/// external lookup.
pub(crate) struct PackageTreeData {
    pub(crate) rows: Rc<RefCell<Vec<crate::PackageRow>>>,
    pub(crate) configuration_rows: Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    pub(crate) packages_group_label: String,
    pub(crate) additional_software_group_label: String,
    pub(crate) language_group_label: String,
    pub(crate) configuration_group_label: String,
    pub(crate) packages_group_node: Box<Node>,
    pub(crate) additional_software_group_node: Box<Node>,
    pub(crate) language_group_node: Box<Node>,
    pub(crate) package_nodes: Vec<Box<Node>>,
    pub(crate) configuration_group_node: Box<Node>,
    pub(crate) configuration_nodes: Vec<Box<Node>>,
}

impl PackageTreeData {
    pub(crate) fn new(
        rows: Rc<RefCell<Vec<crate::PackageRow>>>,
        configuration_rows: Rc<RefCell<Vec<crate::ConfigurationRow>>>,
        packages_group_label: String,
        additional_software_group_label: String,
        language_group_label: String,
        configuration_group_label: String,
    ) -> Self {
        let package_len = rows.borrow().len();
        let package_nodes: Vec<Box<Node>> = (0..package_len)
            .map(|i| {
                Box::new(Node {
                    kind: NodeKind::Package(i),
                })
            })
            .collect();
        let configuration_len = configuration_rows.borrow().len();
        let configuration_nodes: Vec<Box<Node>> = (0..configuration_len)
            .map(|i| {
                Box::new(Node {
                    kind: NodeKind::Configuration(i),
                })
            })
            .collect();
        Self {
            rows,
            configuration_rows,
            packages_group_label,
            additional_software_group_label,
            language_group_label,
            configuration_group_label,
            packages_group_node: Box::new(Node {
                kind: NodeKind::PackagesGroup,
            }),
            additional_software_group_node: Box::new(Node {
                kind: NodeKind::AdditionalSoftwareGroup,
            }),
            language_group_node: Box::new(Node {
                kind: NodeKind::LanguageGroup,
            }),
            package_nodes,
            configuration_group_node: Box::new(Node {
                kind: NodeKind::ConfigurationGroup,
            }),
            configuration_nodes,
        }
    }

    pub(crate) fn packages_group_ptr(&self) -> *const Node {
        self.packages_group_node.as_ref()
    }

    pub(crate) fn additional_software_group_ptr(&self) -> *const Node {
        self.additional_software_group_node.as_ref()
    }

    pub(crate) fn language_group_ptr(&self) -> *const Node {
        self.language_group_node.as_ref()
    }

    /// Whether any package row is in the `Language` category — drives
    /// whether the "Language packs" group node is exposed as a root child
    /// (so it never renders empty).
    pub(crate) fn has_language_packs(&self) -> bool {
        self.rows
            .borrow()
            .iter()
            .any(|r| r.category == rabbit_core::package::PackageCategory::Language)
    }

    /// Whether any package row is in the `Additional` category — drives
    /// whether the "Additional software" group node is exposed as a root
    /// child (so it never renders empty).
    pub(crate) fn has_additional_software(&self) -> bool {
        self.rows
            .borrow()
            .iter()
            .any(|r| r.category == rabbit_core::package::PackageCategory::Additional)
    }

    /// The package leaf pointers whose row matches `category`, in row order.
    /// Used to populate each group's children and to refresh just that
    /// group's leaves after a toggle.
    pub(crate) fn package_ptrs_in_category(
        &self,
        category: rabbit_core::package::PackageCategory,
    ) -> Vec<*const Node> {
        let rows = self.rows.borrow();
        self.package_nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                rows.get(*i)
                    .map(|r| r.category == category)
                    .unwrap_or(false)
            })
            .map(|(_, node)| node.as_ref() as *const Node)
            .collect()
    }

    pub(crate) fn configuration_group_ptr(&self) -> *const Node {
        self.configuration_group_node.as_ref()
    }

    pub(crate) fn package_ptr(&self, idx: usize) -> *const Node {
        self.package_nodes[idx].as_ref()
    }

    pub(crate) fn configuration_ptr(&self, idx: usize) -> *const Node {
        self.configuration_nodes[idx].as_ref()
    }

    pub(crate) fn all_configuration_ptrs(&self) -> Vec<*const Node> {
        self.configuration_nodes
            .iter()
            .map(|b| b.as_ref() as *const Node)
            .collect()
    }
}

/// Model column indices for the non-Windows DataView path.
pub(crate) const PACKAGE_COL_TOGGLE: u32 = 0;
pub(crate) const PACKAGE_COL_LABEL: u32 = 1;

// ===========================================================================
// Non-Windows: wxDataViewCtrl + CustomDataViewTreeModel.
//
// Windows is special-cased via `TVS_CHECKBOXES`; on macOS and GTK the
// equivalent native pattern is "outline view with a check column" — i.e.
// wxDataView with `DataViewToggleRenderer` over `VariantType::Bool`. The
// model carries one synthetic Group node + one leaf per `PackageRow`, the
// toggle column gets `Activatable` mode so Space + click both route through
// `set_value`, and `is_enabled` returns false for unavailable rows so the
// platform draws (and exposes) them as disabled.
// ===========================================================================

/// Non-Windows: build the Packages page using a wxDataViewCtrl driven by a
/// `CustomDataViewTreeModel`. The model exposes a synthetic Packages group
/// + one leaf per `PackageRow`; column 0 is a Bool toggle, column 1 is the
/// row label (the column with the expander triangle). The model's
/// `set_value` callback owns all the toggle side effects.
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

    let tree = DataViewCtrl::builder(page)
        .with_style(DataViewStyle::Single | DataViewStyle::RowLines | DataViewStyle::NoHeader)
        .with_size(Size::new(-1, 220))
        .build();
    tree.set_name("rabbit-package-list");
    // Floor the list height so longer translations (the labels, checkbox, and
    // notes below grow vertically in German/French) can never squeeze the
    // proportion-1 list down to nothing. It still expands to fill free space.
    tree.set_min_size(Size::new(-1, 160));

    // The model is constructed BEFORE associate_model so wx's internal
    // refcount stays sane. `package_items` (the model handle cell) gets
    // populated immediately afterwards so set_value's notification path
    // can find the model the next time the user toggles a row.
    let tree_data = PackageTreeData::new(
        Rc::clone(&package_rows),
        Rc::clone(&configuration_rows),
        model.text.packages_tree_group_label.clone(),
        model.text.additional_software_tree_group_label.clone(),
        model.text.language_tree_group_label.clone(),
        model.text.configuration_tree_group_label.clone(),
    );
    let side_widgets: PackagesSideWidgetsCell = Rc::new(RefCell::new(None));
    let dv_model = build_packages_tree_model(
        tree_data,
        Rc::clone(&package_rows),
        Rc::clone(&configuration_rows),
        Rc::clone(&package_items),
        Rc::clone(&side_widgets),
        Rc::clone(&can_install),
        model.clone(),
    );
    *package_items.borrow_mut() = Some(dv_model.clone());

    let toggle_renderer = DataViewToggleRenderer::new(
        VariantType::Bool,
        DataViewCellMode::Activatable,
        DataViewAlign::Center,
    );
    let toggle_column = DataViewColumn::new(
        "",
        &toggle_renderer,
        PACKAGE_COL_TOGGLE as usize,
        28,
        DataViewAlign::Center,
        DataViewColumnFlags::DefaultNone,
    );
    tree.append_column(&toggle_column);

    let text_renderer = DataViewTextRenderer::new(
        VariantType::String,
        DataViewCellMode::Inert,
        DataViewAlign::Left,
    );
    let text_column = DataViewColumn::new(
        "",
        &text_renderer,
        PACKAGE_COL_LABEL as usize,
        -1,
        DataViewAlign::Left,
        DataViewColumnFlags::Resizable,
    );
    tree.append_column(&text_column);

    tree.associate_model(&dv_model);

    expand_packages_group(&tree, &dv_model);
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

    // Hand the tree model its handles now that the widgets exist. All four
    // types are `Copy`, so the moves into the event closures below still
    // work. This runs before `page.set_sizer`, i.e. before the user can
    // interact with anything.
    *side_widgets.borrow_mut() = Some(PackagesSideWidgets {
        osara_checkbox: osara_keymap_replace,
        osara_note: osara_keymap_note,
        spanish_choice: spanish_variant_choice,
        language_choice: reaper_language_choice,
    });

    {
        let package_rows = Rc::clone(&package_rows);
        let model_text = model.clone();
        let osara_checkbox = osara_keymap_replace;
        let osara_note = osara_keymap_note;
        let spanish_checkbox = spanish_variant_choice;
        let language_choice = reaper_language_choice;
        tree.on_selection_changed(move |event| {
            if let Some(item) = event.get_item() {
                if let Some(node_ptr) = item.get_id::<Node>() {
                    if !node_ptr.is_null() {
                        // SAFETY: node_ptr originated from a Box<Node>
                        // owned by the model's userdata; the model lives
                        // for as long as this closure can fire.
                        let node = unsafe { &*node_ptr };
                        if let NodeKind::Package(idx) = node.kind {
                            if let Some(value) = package_rows.borrow().get(idx).map(package_details)
                            {
                                details.set_value(&value);
                            }
                        }
                    }
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

/// Non-Windows: the widgets that sit *below* the packages tree and whose
/// state depends on which packages are ticked.
///
/// They cannot be captured by the model's `set_value` closure, because
/// creation order is tab order: the tree has to be created before them, and
/// the model is built with the tree. So they arrive afterwards through a
/// cell, exactly like the self-referential model handle in
/// `PackagesStateCell`.
#[derive(Clone, Copy)]
pub(crate) struct PackagesSideWidgets {
    pub(crate) osara_checkbox: CheckBox,
    pub(crate) osara_note: TextCtrl,
    pub(crate) spanish_choice: Choice,
    pub(crate) language_choice: Choice,
}

pub(crate) type PackagesSideWidgetsCell = Rc<RefCell<Option<PackagesSideWidgets>>>;

/// Non-Windows: build the `CustomDataViewTreeModel` that backs the packages
/// tree. The closures capture clones of `package_rows`, `package_items`
/// (the self-referential model handle cell), `can_install`, and the wizard
/// model, so `set_value` can mutate row state, fire item-changed
/// notifications and recompute downstream UI flags without going through
/// any external lookup.
pub(crate) fn build_packages_tree_model(
    data: PackageTreeData,
    rows: Rc<RefCell<Vec<crate::PackageRow>>>,
    configuration_rows: Rc<RefCell<Vec<crate::ConfigurationRow>>>,
    model_cell: PackagesStateCell,
    side_widgets: PackagesSideWidgetsCell,
    can_install: Rc<Cell<bool>>,
    wizard_model: WizardModel,
) -> CustomDataViewTreeModel {
    type CompareFn = fn(&PackageTreeData, &Node, &Node, u32, bool) -> i32;

    let rows_for_get_value = Rc::clone(&rows);
    let rows_for_set_value = Rc::clone(&rows);
    let rows_for_is_enabled = Rc::clone(&rows);
    let configuration_rows_for_get_value = Rc::clone(&configuration_rows);
    let configuration_rows_for_set_value = Rc::clone(&configuration_rows);
    let configuration_rows_for_is_enabled = Rc::clone(&configuration_rows);
    let configuration_rows_for_recompute = Rc::clone(&configuration_rows);
    let wizard_model_for_recompute = wizard_model.clone();
    let model_cell_for_set_value = Rc::clone(&model_cell);
    let side_widgets_for_set_value = Rc::clone(&side_widgets);

    CustomDataViewTreeModel::new(
        data,
        // get_parent
        |data: &PackageTreeData, item: Option<&Node>| -> Option<*mut Node> {
            match item {
                None => None,
                Some(node) => match node.kind {
                    NodeKind::PackagesGroup
                    | NodeKind::AdditionalSoftwareGroup
                    | NodeKind::LanguageGroup
                    | NodeKind::ConfigurationGroup => None,
                    NodeKind::Package(idx) => {
                        let category = data
                            .rows
                            .borrow()
                            .get(idx)
                            .map(|r| r.category)
                            .unwrap_or_default();
                        match category {
                            rabbit_core::package::PackageCategory::Additional => {
                                Some(data.additional_software_group_ptr() as *mut Node)
                            }
                            rabbit_core::package::PackageCategory::Language => {
                                Some(data.language_group_ptr() as *mut Node)
                            }
                            rabbit_core::package::PackageCategory::Core => {
                                Some(data.packages_group_ptr() as *mut Node)
                            }
                        }
                    }
                    NodeKind::Configuration(_) => Some(data.configuration_group_ptr() as *mut Node),
                },
            }
        },
        // is_container
        |_data: &PackageTreeData, item: Option<&Node>| -> bool {
            match item {
                None => true,
                Some(node) => matches!(
                    node.kind,
                    NodeKind::PackagesGroup
                        | NodeKind::AdditionalSoftwareGroup
                        | NodeKind::LanguageGroup
                        | NodeKind::ConfigurationGroup
                ),
            }
        },
        // get_children
        |data: &PackageTreeData, item: Option<&Node>| -> Vec<*mut Node> {
            match item {
                None => {
                    // Root children: Packages, then Additional software (only
                    // when populated, so it never shows empty), then
                    // Configuration.
                    let mut roots = vec![data.packages_group_ptr() as *mut Node];
                    if data.has_additional_software() {
                        roots.push(data.additional_software_group_ptr() as *mut Node);
                    }
                    if data.has_language_packs() {
                        roots.push(data.language_group_ptr() as *mut Node);
                    }
                    roots.push(data.configuration_group_ptr() as *mut Node);
                    roots
                }
                Some(node) => match node.kind {
                    NodeKind::PackagesGroup => data
                        .package_ptrs_in_category(rabbit_core::package::PackageCategory::Core)
                        .into_iter()
                        .map(|p| p as *mut Node)
                        .collect(),
                    NodeKind::AdditionalSoftwareGroup => data
                        .package_ptrs_in_category(rabbit_core::package::PackageCategory::Additional)
                        .into_iter()
                        .map(|p| p as *mut Node)
                        .collect(),
                    NodeKind::LanguageGroup => data
                        .package_ptrs_in_category(rabbit_core::package::PackageCategory::Language)
                        .into_iter()
                        .map(|p| p as *mut Node)
                        .collect(),
                    NodeKind::ConfigurationGroup => data
                        .all_configuration_ptrs()
                        .into_iter()
                        .map(|p| p as *mut Node)
                        .collect(),
                    NodeKind::Package(_) | NodeKind::Configuration(_) => Vec::new(),
                },
            }
        },
        // get_value
        move |data: &PackageTreeData, item: Option<&Node>, col: u32| -> Variant {
            let Some(node) = item else {
                return Variant::from_string("");
            };
            match node.kind {
                NodeKind::PackagesGroup => {
                    if col == PACKAGE_COL_TOGGLE {
                        // Aggregate state: true only if every available row
                        // in this group's category is selected. The standard
                        // toggle renderer can't show a tristate, so a
                        // partially-selected group reads as unchecked.
                        let rows = rows_for_get_value.borrow();
                        let mut any_available = false;
                        let all_checked = rows
                            .iter()
                            .filter(|r| {
                                r.category == rabbit_core::package::PackageCategory::Core
                                    && r.available_for_target
                            })
                            .inspect(|_| any_available = true)
                            .all(|r| r.selected);
                        Variant::from_bool(any_available && all_checked)
                    } else {
                        Variant::from_string(&data.packages_group_label)
                    }
                }
                NodeKind::AdditionalSoftwareGroup => {
                    if col == PACKAGE_COL_TOGGLE {
                        let rows = rows_for_get_value.borrow();
                        let mut any_available = false;
                        let all_checked = rows
                            .iter()
                            .filter(|r| {
                                r.category == rabbit_core::package::PackageCategory::Additional
                                    && r.available_for_target
                            })
                            .inspect(|_| any_available = true)
                            .all(|r| r.selected);
                        Variant::from_bool(any_available && all_checked)
                    } else {
                        Variant::from_string(&data.additional_software_group_label)
                    }
                }
                NodeKind::LanguageGroup => {
                    if col == PACKAGE_COL_TOGGLE {
                        let rows = rows_for_get_value.borrow();
                        let mut any_available = false;
                        let all_checked = rows
                            .iter()
                            .filter(|r| {
                                r.category == rabbit_core::package::PackageCategory::Language
                                    && r.available_for_target
                            })
                            .inspect(|_| any_available = true)
                            .all(|r| r.selected);
                        Variant::from_bool(any_available && all_checked)
                    } else {
                        Variant::from_string(&data.language_group_label)
                    }
                }
                NodeKind::Package(idx) => {
                    let rows = rows_for_get_value.borrow();
                    let Some(row) = rows.get(idx) else {
                        return Variant::from_string("");
                    };
                    if col == PACKAGE_COL_TOGGLE {
                        Variant::from_bool(row.selected)
                    } else {
                        Variant::from_string(&row.summary)
                    }
                }
                NodeKind::ConfigurationGroup => {
                    if col == PACKAGE_COL_TOGGLE {
                        let cfg_rows = configuration_rows_for_get_value.borrow();
                        let mut any_available = false;
                        let all_checked = cfg_rows
                            .iter()
                            // already-applied rows are excluded, exactly as
                            // compute_configuration_group_tristate does on
                            // Windows: they are forced unselected and would
                            // otherwise pin the group to "unchecked" forever.
                            .filter(|r| r.available_for_target && !r.already_applied)
                            .inspect(|_| any_available = true)
                            .all(|r| r.selected);
                        Variant::from_bool(any_available && all_checked)
                    } else {
                        Variant::from_string(&data.configuration_group_label)
                    }
                }
                NodeKind::Configuration(idx) => {
                    let cfg_rows = configuration_rows_for_get_value.borrow();
                    let Some(row) = cfg_rows.get(idx) else {
                        return Variant::from_string("");
                    };
                    if col == PACKAGE_COL_TOGGLE {
                        Variant::from_bool(row.selected)
                    } else {
                        Variant::from_string(&row.summary)
                    }
                }
            }
        },
        // set_value
        Some(
            move |data: &PackageTreeData, item: Option<&Node>, col: u32, var: &Variant| -> bool {
                if col != PACKAGE_COL_TOGGLE {
                    return false;
                }
                let Some(node) = item else {
                    return false;
                };
                let new_state = var.get_bool().unwrap_or(false);

                match node.kind {
                    NodeKind::PackagesGroup
                    | NodeKind::AdditionalSoftwareGroup
                    | NodeKind::LanguageGroup => {
                        // Group toggle propagates to every available leaf in
                        // this group's category; unavailable rows stay
                        // untouched so the install plan never carries
                        // something we can't honor.
                        let category = match node.kind {
                            NodeKind::AdditionalSoftwareGroup => {
                                rabbit_core::package::PackageCategory::Additional
                            }
                            NodeKind::LanguageGroup => {
                                rabbit_core::package::PackageCategory::Language
                            }
                            _ => rabbit_core::package::PackageCategory::Core,
                        };
                        let mut rows = rows_for_set_value.borrow_mut();
                        for row in rows.iter_mut() {
                            if row.category == category && row.available_for_target {
                                let _ = apply_checkbox_state_to_package_row(
                                    &wizard_model,
                                    row,
                                    new_state,
                                );
                            }
                        }
                    }
                    NodeKind::Package(idx) => {
                        let mut rows = rows_for_set_value.borrow_mut();
                        let Some(row) = rows.get_mut(idx) else {
                            return false;
                        };
                        if !row.available_for_target {
                            return false;
                        }
                        let _ = apply_checkbox_state_to_package_row(&wizard_model, row, new_state);
                    }
                    NodeKind::ConfigurationGroup => {
                        let mut cfg_rows = configuration_rows_for_set_value.borrow_mut();
                        for row in cfg_rows.iter_mut() {
                            if row.available_for_target && !row.already_applied {
                                row.selected = new_state;
                            }
                        }
                    }
                    NodeKind::Configuration(idx) => {
                        let mut cfg_rows = configuration_rows_for_set_value.borrow_mut();
                        let Some(row) = cfg_rows.get_mut(idx) else {
                            return false;
                        };
                        if !row.available_for_target || row.already_applied {
                            return false;
                        }
                        row.selected = new_state;
                    }
                }

                let any_install_or_update = rows_for_set_value.borrow().iter().any(|row| {
                    row.available_for_target
                        && matches!(row.action, PlanActionKind::Install | PlanActionKind::Update)
                });
                can_install.set(any_install_or_update);

                // Recompute configuration row availability whenever a
                // package toggle could have flipped a dependency state.
                let recomputed_configuration = matches!(
                    node.kind,
                    NodeKind::PackagesGroup
                        | NodeKind::AdditionalSoftwareGroup
                        | NodeKind::LanguageGroup
                        | NodeKind::Package(_)
                );
                if recomputed_configuration {
                    if let Ok(localizer) =
                        crate::localizer_from_options(&wizard_model_for_recompute.bootstrap_options)
                    {
                        let package_rows_snapshot = rows_for_set_value.borrow();
                        let mut cfg_rows = configuration_rows_for_recompute.borrow_mut();
                        // None for the resource-path argument: a package
                        // toggle can't change `reapack.ini`, so preserve
                        // each row's existing `already_applied` flag.
                        crate::recompute_configuration_row_availability(
                            &localizer,
                            &package_rows_snapshot,
                            None,
                            &mut cfg_rows,
                        );
                    }

                    // Same tail as the Windows `refresh_after_packages_toggle`:
                    // ticking a package can change which OSARA keymap note
                    // applies, whether the Spanish variant picker is usable,
                    // and — because it owns the dropdown's *contents*, not
                    // just its enabled state — what the REAPER-language
                    // dropdown offers. Without this, toggling a row that is
                    // already selected (Space, or clicking its checkbox)
                    // leaves all three stale, because `on_selection_changed`
                    // never fires.
                    if let Some(widgets) = *side_widgets_for_set_value.borrow() {
                        let rows = rows_for_set_value.borrow();
                        sync_osara_keymap_widgets(
                            &wizard_model_for_recompute,
                            &rows,
                            &widgets.osara_checkbox,
                            &widgets.osara_note,
                        );
                        sync_spanish_variant_widget(&rows, &widgets.spanish_choice);
                        sync_reaper_language_widget(&rows, &widgets.language_choice);
                    }
                }

                // Push the cell changes back into the view. SetValue's
                // true return only auto-refreshes the (item, col) we set;
                // we also need to refresh the row's label cell (the action
                // text flips Install/Update/Keep) and the parent group's
                // aggregate cell.
                if let Some(model) = model_cell_for_set_value.borrow().as_ref() {
                    match node.kind {
                        NodeKind::PackagesGroup => {
                            let parent_ptr = data.packages_group_ptr();
                            let leaf_ptrs = data.package_ptrs_in_category(
                                rabbit_core::package::PackageCategory::Core,
                            );
                            model.items_changed(&leaf_ptrs);
                            model.item_value_changed(parent_ptr, PACKAGE_COL_TOGGLE);
                        }
                        NodeKind::AdditionalSoftwareGroup => {
                            let parent_ptr = data.additional_software_group_ptr();
                            let leaf_ptrs = data.package_ptrs_in_category(
                                rabbit_core::package::PackageCategory::Additional,
                            );
                            model.items_changed(&leaf_ptrs);
                            model.item_value_changed(parent_ptr, PACKAGE_COL_TOGGLE);
                        }
                        NodeKind::LanguageGroup => {
                            let parent_ptr = data.language_group_ptr();
                            let leaf_ptrs = data.package_ptrs_in_category(
                                rabbit_core::package::PackageCategory::Language,
                            );
                            model.items_changed(&leaf_ptrs);
                            model.item_value_changed(parent_ptr, PACKAGE_COL_TOGGLE);
                        }
                        NodeKind::Package(idx) => {
                            let leaf_ptr = data.package_ptr(idx);
                            model.item_value_changed(leaf_ptr, PACKAGE_COL_LABEL);
                            // Refresh the aggregate cell of whichever group
                            // this package hangs under.
                            let category = data
                                .rows
                                .borrow()
                                .get(idx)
                                .map(|r| r.category)
                                .unwrap_or_default();
                            let parent_ptr = match category {
                                rabbit_core::package::PackageCategory::Additional => {
                                    data.additional_software_group_ptr()
                                }
                                rabbit_core::package::PackageCategory::Language => {
                                    data.language_group_ptr()
                                }
                                rabbit_core::package::PackageCategory::Core => {
                                    data.packages_group_ptr()
                                }
                            };
                            model.item_value_changed(parent_ptr, PACKAGE_COL_TOGGLE);
                        }
                        NodeKind::ConfigurationGroup => {
                            let parent_ptr = data.configuration_group_ptr();
                            let leaf_ptrs = data.all_configuration_ptrs();
                            model.items_changed(&leaf_ptrs);
                            model.item_value_changed(parent_ptr, PACKAGE_COL_TOGGLE);
                        }
                        NodeKind::Configuration(idx) => {
                            let leaf_ptr = data.configuration_ptr(idx);
                            model.item_value_changed(leaf_ptr, PACKAGE_COL_LABEL);
                            model.item_value_changed(
                                data.configuration_group_ptr(),
                                PACKAGE_COL_TOGGLE,
                            );
                        }
                    }

                    if recomputed_configuration {
                        let cfg_leaf_ptrs = data.all_configuration_ptrs();
                        model.items_changed(&cfg_leaf_ptrs);
                        model
                            .item_value_changed(data.configuration_group_ptr(), PACKAGE_COL_TOGGLE);
                    }
                }

                true
            },
        ),
        // is_enabled — gray out the checkbox + label of unavailable rows.
        Some(
            move |_data: &PackageTreeData, item: Option<&Node>, _col: u32| -> bool {
                let Some(node) = item else {
                    return true;
                };
                match node.kind {
                    NodeKind::PackagesGroup
                    | NodeKind::AdditionalSoftwareGroup
                    | NodeKind::LanguageGroup
                    | NodeKind::ConfigurationGroup => true,
                    NodeKind::Package(idx) => rows_for_is_enabled
                        .borrow()
                        .get(idx)
                        .map(|row| row.available_for_target)
                        .unwrap_or(true),
                    NodeKind::Configuration(idx) => configuration_rows_for_is_enabled
                        .borrow()
                        .get(idx)
                        .map(|row| row.available_for_target && !row.already_applied)
                        .unwrap_or(true),
                }
            },
        ),
        // compare — left at None semantically; the explicit type is needed
        // because the closure-based `Option<CMP>` pattern doesn't infer
        // without it.
        None::<CompareFn>,
    )
}

/// Non-Windows: expand both synthetic group nodes ("Packages" and
/// "Configuration") so all leaves are visible without an extra click.
/// Reads the group pointers from the model's userdata so the model
/// owns the canonical Node addresses.
pub(crate) fn expand_packages_group(tree: &PackagesView, model: &CustomDataViewTreeModel) {
    let mut packages_group_ptr: *const Node = std::ptr::null();
    let mut additional_software_group_ptr: *const Node = std::ptr::null();
    let mut language_group_ptr: *const Node = std::ptr::null();
    let mut configuration_group_ptr: *const Node = std::ptr::null();
    model.with_userdata_mut::<PackageTreeData, ()>(|data| {
        packages_group_ptr = data.packages_group_ptr();
        // Only expand the additional-software group when it's actually
        // exposed as a root child; otherwise its pointer addresses a node
        // the view never asked about.
        if data.has_additional_software() {
            additional_software_group_ptr = data.additional_software_group_ptr();
        }
        // Same guard for the language-packs group.
        if data.has_language_packs() {
            language_group_ptr = data.language_group_ptr();
        }
        configuration_group_ptr = data.configuration_group_ptr();
    });
    for ptr in [
        packages_group_ptr,
        additional_software_group_ptr,
        language_group_ptr,
        configuration_group_ptr,
    ] {
        if ptr.is_null() {
            continue;
        }
        let item = wxdragon::widgets::dataview::DataViewItem::from_id_ptr(ptr);
        if item.is_ok() {
            tree.expand(&item);
        }
    }
}

/// Non-Windows: replace the row set inside the live
/// `CustomDataViewTreeModel`. Reuses the existing model + control
/// association so nothing has to be rewired; the model just gets new
/// userdata, then we tell the view that everything has changed via
/// `cleared()`. After cleared() the control re-queries the model for
/// visible items and the previously-selected row drops away (caller
/// resets `package_details` to the first row).
pub(crate) fn rebuild_packages_tree_model(
    tree: &PackagesView,
    package_items: &PackagesStateCell,
    model: &WizardModel,
    package_rows: &[PackageRow],
    configuration_rows: &[ConfigurationRow],
) {
    let Some(dv_model) = package_items.borrow().as_ref().cloned() else {
        return;
    };
    let packages_group_label = model.text.packages_tree_group_label.clone();
    let additional_software_group_label = model.text.additional_software_tree_group_label.clone();
    let language_group_label = model.text.language_tree_group_label.clone();
    let configuration_group_label = model.text.configuration_tree_group_label.clone();
    dv_model.with_userdata_mut::<PackageTreeData, ()>(|data| {
        // Sync the shared Rc<RefCell<Vec<_>>>s in case the caller
        // hasn't pre-replaced them (the post-install hook does, the
        // version-check finish handler also does — be defensive in
        // case a future caller forgets).
        let pkg_len = package_rows.len();
        if data.rows.borrow().len() != pkg_len {
            *data.rows.borrow_mut() = package_rows.to_vec();
        }
        let cfg_len = configuration_rows.len();
        if data.configuration_rows.borrow().len() != cfg_len {
            *data.configuration_rows.borrow_mut() = configuration_rows.to_vec();
        }
        data.packages_group_label = packages_group_label;
        data.additional_software_group_label = additional_software_group_label;
        data.language_group_label = language_group_label;
        data.configuration_group_label = configuration_group_label;
        data.package_nodes = (0..pkg_len)
            .map(|i| {
                Box::new(Node {
                    kind: NodeKind::Package(i),
                })
            })
            .collect();
        data.configuration_nodes = (0..cfg_len)
            .map(|i| {
                Box::new(Node {
                    kind: NodeKind::Configuration(i),
                })
            })
            .collect();
    });
    dv_model.cleared();
    // wxDataViewCtrl auto-collapses the groups on Cleared; re-expand so
    // the user sees the leaves immediately.
    expand_packages_group(tree, &dv_model);
}

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
    rebuild_packages_tree_model(tree, package_items, model, rows, configuration_rows);
    details.set_value(&rows.first().map(package_details).unwrap_or_default());
    sync_osara_keymap_widgets(model, rows, osara_keymap_replace, osara_keymap_note);
    sync_spanish_variant_widget(rows, spanish_variant_choice);
    sync_reaper_language_widget(rows, reaper_language_choice);
}

pub(crate) fn rebuild_package_list_widgets(
    widgets: &WizardWidgets,
    package_items: &PackagesStateCell,
    model: &WizardModel,
    package_rows: &[PackageRow],
    configuration_rows: &[ConfigurationRow],
) {
    rebuild_packages_tree_model(
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

/// Non-Windows twin of the tree-based helper: give the packages DataView
/// keyboard focus so a screen reader starts reading it from the top.
///
/// Deliberately focus-only, NO selection. wxdragon's
/// `DataViewCtrl::select_row` fabricates a `wxDataViewItem` whose internal
/// pointer IS the integer `row + 1` — the encoding only virtual LIST models
/// use. This view is driven by a custom TREE model whose item ids are real
/// node pointers, so `select_row(0)` hands the native macOS port a garbage
/// `0x1` pointer to dereference and crashes the app (reported by macOS
/// users as a crash right after the first Next, when the version check
/// auto-advances onto this page). A freshly rebuilt DataView has no
/// selection anyway, so reading naturally starts at the top — the
/// caret-drift problem this helper fixes is native-Windows-tree-specific.
pub(crate) fn focus_packages_list_top(widgets: &WizardWidgets, _package_items: &PackagesStateCell) {
    widgets.package_checklist.set_focus();
}
