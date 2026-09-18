//! The wxWidgets wizard shell: `run` builds the frame, wires every page's
//! events, and hands the work to the modules beside it.
mod close_guard;
mod globals;
mod packages_page;
mod pages;
mod progress_ui;
mod self_update_ui;
mod shell;
mod version_check;
mod widgets;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::{
    UiBootstrapOptions, WizardInstallOptions, WizardOutcomeReport,
    build_review_preview_for_package_rows, execute_wizard_install_with_progress,
    install_request_from_target_and_rows, load_wizard_model, localizer_from_options,
    reapack_selected_for_install_or_update, recompute_configuration_row_availability,
    refreshed_target_row, run_wizard_self_update_check, run_wizard_self_update_release_notes,
    save_wizard_outcome_report, selected_configuration_step_ids, wizard_outcome_report_from_error,
    wizard_outcome_report_from_success, wizard_package_plan_for_target,
};
use rabbit_core::localization::resolve_runtime_locale;
use rabbit_core::progress::ProgressReporter;

use wxdragon::prelude::*;
use wxdragon::widgets::SimpleBook;

use crate::wx_app::close_guard::{
    CloseVerdict, InstallRunState, announce_stopping, close_verdict, confirm_stop_install,
    show_close_blocked_by_self_update,
};
use crate::wx_app::globals::{
    arm_post_install_hook, fire_post_install_hook, install_ui_frame, install_ui_localizer,
    with_ui_frame, with_ui_localizer,
};
use crate::wx_app::packages_page::{
    PackagesStateCell, new_packages_state, refresh_package_checklist,
};
use crate::wx_app::pages::{add_pages, build_language_footer};
use crate::wx_app::progress_ui::{ProgressUiState, apply_progress_event_to_ui};
use crate::wx_app::self_update_ui::{SelfUpdateUiState, render_self_update_status};
use crate::wx_app::shell::{launch_reaper, open_resource_folder, seat_macos_apple_languages};
use crate::wx_app::version_check::{VersionCheckUi, start_version_check};
use crate::wx_app::widgets::{
    append_done_status, bind_done_page_enter_closes, bind_reapack_ack_navigation_updates,
    bind_target_navigation_updates, can_launch_last_reaper_path, checked_package_indices,
    clone_last_path, clone_last_resource_path, effective_can_install, osara_keymap_choice,
    planned_reaper_launch_path_for_target, progress_details_for_start, reapack_ack_confirmed,
    refresh_target_choice, refreshed_target_index, selected_target_row, set_last_path,
    set_last_report, set_last_resource_path, step_status, target_is_valid, update_navigation,
};

pub(crate) const TARGET_STEP: usize = 0;
pub(crate) const VERSION_CHECK_STEP: usize = 1;
pub(crate) const PACKAGES_STEP: usize = 2;
pub(crate) const REAPACK_ACK_STEP: usize = 3;
pub(crate) const REVIEW_STEP: usize = 4;
pub(crate) const PROGRESS_STEP: usize = 5;
pub(crate) const DONE_STEP: usize = 6;

pub fn run() {
    // Pre-seat Cocoa's per-process language so VoiceOver picks a voice that
    // matches the in-app Fluent locale. Has to happen before `wxdragon::main`
    // because that brings NSApplication / NSBundle up, and `AppleLanguages`
    // is only consulted on first read of `[NSBundle preferredLocalizations]`.
    // No-op off macOS.
    seat_macos_apple_languages(&resolve_runtime_locale());

    let _ = wxdragon::main(|_| {
        let bootstrap = UiBootstrapOptions {
            locale: resolve_runtime_locale(),
            online_versions: false,
            ..UiBootstrapOptions::default()
        };
        match localizer_from_options(&bootstrap) {
            Ok(localizer) => install_ui_localizer(localizer),
            Err(error) => {
                eprintln!("{error}");
                return;
            }
        }
        let model = match load_wizard_model(bootstrap) {
            Ok(model) => model,
            Err(error) => {
                eprintln!("{error}");
                return;
            }
        };

        let frame = Frame::builder()
            .with_title(&model.window_title)
            .with_size(Size::new(820, 680))
            .build();
        frame.set_name("rabbit-main-window");
        // Keep a floor on the window size: the wizard pages stack fixed-height
        // controls (lists, detail panes, notes) that wrap to more lines in
        // longer translations, so shrinking below this clips bottom content.
        frame.set_min_size(Size::new(760, 600));
        install_ui_frame(frame);

        let root_panel = Panel::builder(&frame).build();
        root_panel.set_name("rabbit-root-panel");

        let root = BoxSizer::builder(Orientation::Vertical).build();
        let step_label = StaticText::builder(&root_panel)
            .with_label(&step_status(&model, TARGET_STEP))
            .build();
        step_label.set_name("rabbit-step-status");
        root.add(&step_label, 0, SizerFlag::All | SizerFlag::Expand, 12);

        // Use the frame's wxStatusBar for self-update status. NVDA's "Report
        // status bar" command (NVDA+End) reads exactly this control, JAWS
        // exposes it via its status-bar review keys, and Narrator/UIA expose
        // the StatusBar role natively. Updating via SetStatusText fires the
        // platform notifications that screen readers auto-announce.
        let self_update_status = frame.create_status_bar(1, 0, 0, "rabbit-self-update-status");
        self_update_status.set_status_text(&model.text.self_update_status_checking, 0);

        let book = SimpleBook::builder(&root_panel).build();
        book.set_name("rabbit-wizard-pages");
        let package_rows = Rc::new(RefCell::new(model.package_rows.clone()));
        let package_notes = Rc::new(RefCell::new(model.notes.clone()));
        let configuration_rows = Rc::new(RefCell::new(model.configuration_rows.clone()));
        // Per-platform shared state for the package list — see
        // `PackagesStateCell`. Populated by `build_packages_page` on the
        // first run and refreshed by `populate_packages_tree` /
        // `rebuild_packages_tree_model` on subsequent rebuilds (deferred
        // version-check finish, post-install rescan).
        let package_items: PackagesStateCell = new_packages_state();
        let can_install = Rc::new(Cell::new(model.controls.can_install));
        let review_can_install = Rc::new(Cell::new(false));
        let last_report = Arc::new(Mutex::new(None::<WizardOutcomeReport>));
        let last_reaper_app_path = Arc::new(Mutex::new(None::<PathBuf>));
        let last_resource_path = Arc::new(Mutex::new(None::<PathBuf>));
        let install_run = Arc::new(InstallRunState::default());
        // Build the wizard pages first, the buttons row, then the language
        // footer. Footer is constructed after the buttons so its widgets
        // come *after* the buttons in tab order, but it needs to exist
        // *before* `add_pages` so the WizardWidgets struct can capture
        // its Panel handle.
        root.add(&book, 1, SizerFlag::All | SizerFlag::Expand, 12);

        let buttons = BoxSizer::builder(Orientation::Horizontal).build();
        buttons.add_stretch_spacer(1);

        let back = Button::builder(&root_panel)
            .with_label(&model.controls.back_label)
            .build();
        back.set_name("rabbit-back-button");
        back.add_style(WindowStyle::TabStop);
        back.set_can_focus(true);
        buttons.add(&back, 0, SizerFlag::All, 6);

        let next = Button::builder(&root_panel)
            .with_label(&model.controls.next_label)
            .build();
        next.set_name("rabbit-next-button");
        next.add_style(WindowStyle::TabStop);
        next.set_can_focus(true);
        buttons.add(&next, 0, SizerFlag::All, 6);

        let install = Button::builder(&root_panel)
            .with_label(&model.controls.install_label)
            .build();
        install.set_name("rabbit-install-button");
        install.add_style(WindowStyle::TabStop);
        install.set_can_focus(true);
        buttons.add(&install, 0, SizerFlag::All, 6);

        let close = Button::builder(&root_panel)
            .with_label(&model.controls.close_label)
            .build();
        close.set_name("rabbit-close-button");
        close.add_style(WindowStyle::TabStop);
        close.set_can_focus(true);
        buttons.add(&close, 0, SizerFlag::All, 6);

        root.add_sizer(&buttons, 0, SizerFlag::All | SizerFlag::Expand, 6);

        let language_footer = build_language_footer(&root_panel, &root, &model);
        let wizard_widgets = add_pages(
            &book,
            &model,
            Rc::clone(&package_rows),
            Rc::clone(&configuration_rows),
            Rc::clone(&package_items),
            Rc::clone(&can_install),
            self_update_status,
            language_footer,
        );

        root_panel.set_sizer(root, true);

        let frame_sizer = BoxSizer::builder(Orientation::Vertical).build();
        frame_sizer.add(&root_panel, 1, SizerFlag::Expand, 0);
        frame.set_sizer(frame_sizer, true);

        let current_step = Arc::new(AtomicUsize::new(TARGET_STEP));
        let labels = Arc::new(
            (TARGET_STEP..=DONE_STEP)
                .map(|step| step_status(&model, step))
                .collect::<Vec<_>>(),
        );
        let model = Arc::new(model);

        update_navigation(
            TARGET_STEP,
            &book,
            &step_label,
            labels.as_slice(),
            &back,
            &next,
            &install,
            &close,
            &language_footer,
            effective_can_install(&can_install, &review_can_install),
            target_is_valid(&model, &wizard_widgets),
            reapack_ack_confirmed(&wizard_widgets),
        );
        bind_target_navigation_updates(&model, wizard_widgets, &current_step, &next);
        bind_reapack_ack_navigation_updates(wizard_widgets, &current_step, &next);
        // The Done page parks focus on its read-only summary TextCtrl, which
        // eats Enter before the default Close button can see it — bind both
        // of that page's text boxes so Enter dismisses the wizard from
        // wherever the user happens to be standing.
        bind_done_page_enter_closes(&wizard_widgets.done_status, &frame, &current_step);
        bind_done_page_enter_closes(&wizard_widgets.done_details, &frame, &current_step);

        {
            let current_step = Arc::clone(&current_step);
            let labels = Arc::clone(&labels);
            let model = Arc::clone(&model);
            let widgets = wizard_widgets;
            let can_install = Rc::clone(&can_install);
            let review_can_install = Rc::clone(&review_can_install);
            let back_package_rows = Rc::clone(&package_rows);
            back.on_click(move |_| {
                // Custom Back routing:
                // - PACKAGES_STEP → TARGET_STEP (skip version check; re-running
                //   the fetch from a Back press isn't what the user asked for).
                // - REAPACK_ACK_STEP → PACKAGES_STEP and clear the
                //   acknowledgement (going back resets the explicit consent).
                // - REVIEW_STEP → REAPACK_ACK_STEP if ReaPack is in the
                //   currently-selected plan; otherwise PACKAGES_STEP, again to
                //   skip the now-irrelevant ack page.
                let current = current_step.load(Ordering::SeqCst);
                let step = match current {
                    PACKAGES_STEP => TARGET_STEP,
                    REAPACK_ACK_STEP => {
                        widgets.reapack_ack_confirm.set_value(false);
                        PACKAGES_STEP
                    }
                    REVIEW_STEP => {
                        let rows = back_package_rows.borrow();
                        let checked = checked_package_indices(&rows);
                        if reapack_selected_for_install_or_update(&rows, &checked) {
                            REAPACK_ACK_STEP
                        } else {
                            PACKAGES_STEP
                        }
                    }
                    other => other.saturating_sub(1),
                };
                current_step.store(step, Ordering::SeqCst);
                update_navigation(
                    step,
                    &book,
                    &step_label,
                    labels.as_slice(),
                    &back,
                    &next,
                    &install,
                    &close,
                    &widgets.language_footer,
                    effective_can_install(&can_install, &review_can_install),
                    target_is_valid(&model, &widgets),
                    reapack_ack_confirmed(&widgets),
                );
            });
        }

        {
            let current_step = Arc::clone(&current_step);
            let labels = Arc::clone(&labels);
            let model = Arc::clone(&model);
            let widgets = wizard_widgets;
            let package_rows = Rc::clone(&package_rows);
            let package_notes = Rc::clone(&package_notes);
            let package_items = Rc::clone(&package_items);
            let configuration_rows = Rc::clone(&configuration_rows);
            let can_install = Rc::clone(&can_install);
            let review_can_install = Rc::clone(&review_can_install);
            next.on_click(move |_| {
                let step = match current_step.load(Ordering::SeqCst) {
                    TARGET_STEP => {
                        let Some(selected_target) = selected_target_row(&model, &widgets) else {
                            return;
                        };
                        // No offline plan computation here: it would call
                        // `detect_components` (file-system + registry
                        // probes for every builtin package), blocking
                        // the UI thread for ~1–2s before the page
                        // transition fires — long enough that the
                        // screen reader and the page-flip both lag
                        // visibly. The version-check Finished handler
                        // already calls `wizard_package_plan_for_target_with_available`
                        // once latest versions are fetched, which does
                        // the same `detect_components` + `build_install_plan`
                        // work; doing it twice (offline first, then
                        // online) was redundant. Reset
                        // `review_can_install` so the Install button
                        // can't fire from a stale Review state, and let
                        // the worker thread do the heavy lifting.
                        review_can_install.set(false);
                        start_version_check(VersionCheckUi {
                            widgets,
                            model: Arc::clone(&model),
                            package_rows: Rc::clone(&package_rows),
                            package_notes: Rc::clone(&package_notes),
                            configuration_rows: Rc::clone(&configuration_rows),
                            package_items: Rc::clone(&package_items),
                            can_install: Rc::clone(&can_install),
                            review_can_install: Rc::clone(&review_can_install),
                            target: selected_target,
                            book,
                            step_label,
                            labels: Arc::clone(&labels),
                            back,
                            next,
                            install,
                            close,
                            current_step: Arc::clone(&current_step),
                        });
                        VERSION_CHECK_STEP
                    }
                    PACKAGES_STEP => {
                        let selected_target = selected_target_row(&model, &widgets);
                        let rows = package_rows.borrow();
                        let notes = package_notes.borrow();
                        let checked = checked_package_indices(&rows);
                        let review_preview = build_review_preview_for_package_rows(
                            &model,
                            selected_target.as_ref(),
                            &checked,
                            &rows,
                            &notes,
                            osara_keymap_choice(&widgets.osara_keymap_replace),
                        );
                        review_can_install.set(review_preview.can_install);
                        widgets
                            .review_text
                            .set_value(&review_preview.lines.join("\n"));
                        // Route through the ReaPack donation acknowledgement
                        // page when the user has ReaPack in the install/update
                        // plan; everyone else goes straight to Review.
                        if reapack_selected_for_install_or_update(&rows, &checked) {
                            REAPACK_ACK_STEP
                        } else {
                            REVIEW_STEP
                        }
                    }
                    REAPACK_ACK_STEP => REVIEW_STEP,
                    PROGRESS_STEP => DONE_STEP,
                    other => other,
                };
                current_step.store(step, Ordering::SeqCst);
                update_navigation(
                    step,
                    &book,
                    &step_label,
                    labels.as_slice(),
                    &back,
                    &next,
                    &install,
                    &close,
                    &widgets.language_footer,
                    effective_can_install(&can_install, &review_can_install),
                    target_is_valid(&model, &widgets),
                    reapack_ack_confirmed(&widgets),
                );
                if step == VERSION_CHECK_STEP {
                    // Pull the screen reader onto the progress bar so the
                    // user hears that a check is running. Without this,
                    // focus would stay on the Next button from the Target
                    // page and the version-check progress wouldn't be
                    // announced until the auto-advance to Packages fires.
                    widgets.version_check_gauge.set_focus();
                }
            });
        }

        {
            let current_step = Arc::clone(&current_step);
            let labels = Arc::clone(&labels);
            let model = Arc::clone(&model);
            let widgets = wizard_widgets;
            let package_rows = Rc::clone(&package_rows);
            let package_notes = Rc::clone(&package_notes);
            let package_items = Rc::clone(&package_items);
            let configuration_rows = Rc::clone(&configuration_rows);
            let can_install = Rc::clone(&can_install);
            let review_can_install = Rc::clone(&review_can_install);
            let last_report = Arc::clone(&last_report);
            let last_reaper_app_path = Arc::clone(&last_reaper_app_path);
            let last_resource_path = Arc::clone(&last_resource_path);
            let install_run = Arc::clone(&install_run);
            install.on_click(move |_| {
                current_step.store(PROGRESS_STEP, Ordering::SeqCst);
                update_navigation(
                    PROGRESS_STEP,
                    &book,
                    &step_label,
                    labels.as_slice(),
                    &back,
                    &next,
                    &install,
                    &close,
                    &widgets.language_footer,
                    effective_can_install(&can_install, &review_can_install),
                    target_is_valid(&model, &widgets),
                    reapack_ack_confirmed(&widgets),
                );
                back.enable(false);
                next.enable(false);
                install.enable(false);
                widgets.done_launch_reaper.enable(false);
                widgets.done_open_resource.enable(false);
                widgets
                    .progress_status
                    .set_label(&model.text.progress_status_running);
                widgets.progress_gauge.set_value(10);
                set_last_report(&last_report, None);

                let selected_target = selected_target_row(&model, &widgets);
                set_last_path(
                    &last_reaper_app_path,
                    selected_target
                        .as_ref()
                        .map(planned_reaper_launch_path_for_target),
                );
                set_last_resource_path(
                    &last_resource_path,
                    selected_target.as_ref().map(|target| target.path.clone()),
                );
                let rows = package_rows.borrow();
                let selected_packages = checked_package_indices(&rows);
                widgets
                    .progress_details
                    .set_value(&progress_details_for_start(
                        &model,
                        selected_target.as_ref(),
                        &selected_packages,
                        &rows,
                        osara_keymap_choice(&widgets.osara_keymap_replace),
                        None,
                    ));
                let request = match selected_target
                    .as_ref()
                    .ok_or_else(|| rabbit_core::RabbitError::PreflightFailed {
                        message: model.text.review_no_target.clone(),
                    })
                    .and_then(|target| {
                        let configuration_step_ids =
                            selected_configuration_step_ids(&configuration_rows.borrow());
                        install_request_from_target_and_rows(
                            &model,
                            target,
                            &rows,
                            &selected_packages,
                            configuration_step_ids,
                            WizardInstallOptions {
                                osara_keymap_choice: osara_keymap_choice(
                                    &widgets.osara_keymap_replace,
                                ),
                                // Ticked = Team PMA's Spanish OSARA
                                // translation (installs as es_MX); unticked
                                // = the default REAPER Accesible español
                                // (es_ES).
                                package_variants: crate::package_variants_from_choice(
                                    widgets.spanish_variant_choice.get_selection(),
                                ),
                                // Which of the installed language packs REAPER
                                // starts in. The dropdown lists the ticked
                                // packs in row order, so the selection index
                                // maps straight back to a package id.
                                reaper_language_package: crate::selected_language_packs(
                                    &rows,
                                    &selected_packages,
                                )
                                .get(widgets.reaper_language_choice.get_selection().unwrap_or(0)
                                    as usize)
                                .map(|(id, _)| id.clone()),
                                ..WizardInstallOptions::default()
                            },
                        )
                    }) {
                    Ok(request) => request,
                    Err(error) => {
                        widgets.progress_gauge.set_value(100);
                        widgets
                            .progress_status
                            .set_label(&model.text.done_status_error);
                        // Done page: short reason on the always-visible
                        // status TextCtrl; full error text in the
                        // collapsible details below.
                        widgets.done_status.set_value(&model.text.done_status_error);
                        widgets.done_details.set_value(&error.to_string());
                        widgets
                            .progress_details
                            .set_value(&format!("{}\n\n{}", model.text.done_status_error, error));
                        widgets
                            .done_open_resource
                            .enable(clone_last_resource_path(&last_resource_path).is_some());
                        widgets
                            .done_launch_reaper
                            .enable(can_launch_last_reaper_path(&last_reaper_app_path));
                        current_step.store(DONE_STEP, Ordering::SeqCst);
                        update_navigation(
                            DONE_STEP,
                            &book,
                            &step_label,
                            labels.as_slice(),
                            &back,
                            &next,
                            &install,
                            &close,
                            &widgets.language_footer,
                            effective_can_install(&can_install, &review_can_install),
                            target_is_valid(&model, &widgets),
                            reapack_ack_confirmed(&widgets),
                        );
                        // Focus the always-visible status TextCtrl so the
                        // screen reader reads the success/failure summary
                        // immediately, and so Tab from there moves on to
                        // the Show-details CheckBox / action buttons
                        // instead of cycling back through earlier widgets.
                        widgets.done_status.set_focus();
                        return;
                    }
                };
                widgets
                    .progress_details
                    .set_value(&progress_details_for_start(
                        &model,
                        selected_target.as_ref(),
                        &selected_packages,
                        &rows,
                        osara_keymap_choice(&widgets.osara_keymap_replace),
                        Some(&request.cache_dir),
                    ));
                drop(rows);

                // Arm the post-install rescan hook. The hook captures the
                // UI-thread `Rc<RefCell>` shared state so the call_after
                // success arm can refresh it without smuggling non-Send
                // references across threads. The hook closure runs on the
                // UI thread; it re-detects the selected target, runs the
                // offline package plan against the now-fresh receipts, and
                // updates both the cached state and the on-screen package
                // list — so navigating Back from the Done page (or
                // re-opening the Packages step via Rescan) shows the
                // post-install version without the user having to click
                // anything.
                {
                    let model = Arc::clone(&model);
                    let package_rows = Rc::clone(&package_rows);
                    let package_notes = Rc::clone(&package_notes);
                    let package_items = Rc::clone(&package_items);
                    let configuration_rows = Rc::clone(&configuration_rows);
                    let can_install = Rc::clone(&can_install);
                    let review_can_install = Rc::clone(&review_can_install);
                    let last_reaper_app_path = Arc::clone(&last_reaper_app_path);
                    let last_resource_path = Arc::clone(&last_resource_path);
                    arm_post_install_hook(move || {
                        let Some(target) = selected_target_row(&model, &widgets) else {
                            return;
                        };
                        let refreshed_target = refreshed_target_row(&model, &target);
                        let Ok(plan) =
                            wizard_package_plan_for_target(&model, Some(&refreshed_target))
                        else {
                            return;
                        };
                        *package_rows.borrow_mut() = plan.package_rows;
                        *package_notes.borrow_mut() = plan.notes;
                        // Recompute configuration row availability against
                        // the freshly-rebuilt package plan (e.g. ReaPack
                        // just got installed → REAPER Accessibility step
                        // becomes available).
                        if let Ok(localizer) = localizer_from_options(&model.bootstrap_options) {
                            recompute_configuration_row_availability(
                                &localizer,
                                &package_rows.borrow(),
                                Some(&refreshed_target.path),
                                &mut configuration_rows.borrow_mut(),
                            );
                        }
                        can_install.set(plan.can_install);
                        review_can_install.set(false);
                        refresh_package_checklist(
                            &widgets.package_checklist,
                            &package_items,
                            &widgets.package_details,
                            &widgets.osara_keymap_replace,
                            &widgets.osara_keymap_note,
                            &widgets.spanish_variant_choice,
                            &widgets.reaper_language_choice,
                            &model,
                            &package_rows.borrow(),
                            &configuration_rows.borrow(),
                        );
                        refresh_target_choice(
                            &model,
                            &widgets.target_choice,
                            refreshed_target_index(&model, &widgets),
                            &refreshed_target,
                        );
                        widgets.target_details.set_value(&refreshed_target.details);
                        set_last_path(
                            &last_reaper_app_path,
                            Some(planned_reaper_launch_path_for_target(&refreshed_target)),
                        );
                        set_last_resource_path(
                            &last_resource_path,
                            Some(refreshed_target.path.clone()),
                        );
                    });
                }

                let ui_model = Arc::clone(&model);
                let ui_current_step = Arc::clone(&current_step);
                let ui_labels = Arc::clone(&labels);
                let ui_last_report = Arc::clone(&last_report);
                let ui_last_reaper_app_path = Arc::clone(&last_reaper_app_path);
                let ui_last_resource_path = Arc::clone(&last_resource_path);
                let can_install = effective_can_install(&can_install, &review_can_install);
                let request_for_report = request.clone();

                // Build progress lookup maps + the per-install UI state now,
                // on the UI thread, where the Rc-based package_rows /
                // configuration_rows are still in scope. The maps are
                // Send+Sync (Arc<HashMap<String, String>>) so they ride along
                // with the worker thread's progress callback into each
                // call_after closure that runs back on the UI thread.
                let configuration_rows_for_progress = configuration_rows.borrow();
                let rows_for_progress = package_rows.borrow();
                let package_display_names: Arc<HashMap<String, String>> = Arc::new(
                    request
                        .package_ids
                        .iter()
                        .filter_map(|package_id| {
                            rows_for_progress
                                .iter()
                                .find(|row| &row.package_id == package_id)
                                .map(|row| (row.package_id.clone(), row.display_name.clone()))
                        })
                        .collect(),
                );
                let configuration_display_names: Arc<HashMap<String, String>> = Arc::new(
                    request
                        .configuration_step_ids
                        .iter()
                        .filter_map(|step_id| {
                            configuration_rows_for_progress
                                .iter()
                                .find(|row| &row.step_id == step_id)
                                .map(|row| (row.step_id.clone(), row.display_name.clone()))
                        })
                        .collect(),
                );
                drop(configuration_rows_for_progress);
                drop(rows_for_progress);

                let progress_state = Arc::new(Mutex::new(ProgressUiState::new(
                    request.package_ids.len(),
                    request.configuration_step_ids.len(),
                )));
                let progress_widgets = widgets;
                let progress_state_for_reporter = Arc::clone(&progress_state);
                let package_display_names_for_reporter = Arc::clone(&package_display_names);
                let configuration_display_names_for_reporter =
                    Arc::clone(&configuration_display_names);
                // Once the operation returned, background download workers
                // may still emit a few final events before they notice the
                // cancel flag — gate them out so nothing repaints the gauge
                // or status after the completion closure wrote the outcome.
                let operation_finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let operation_finished_for_reporter = Arc::clone(&operation_finished);
                let install_run_for_reporter = Arc::clone(&install_run);
                let progress = ProgressReporter::new(move |event| {
                    // The reporter fires on the worker thread; forward each
                    // event to the UI thread so the gauge / status / log can
                    // be touched safely. call_after serialises closures on
                    // the UI thread, so ProgressUiState mutations happen one
                    // event at a time despite the Arc<Mutex<…>> wrapper.
                    if operation_finished_for_reporter.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    let state = Arc::clone(&progress_state_for_reporter);
                    let package_display_names = Arc::clone(&package_display_names_for_reporter);
                    let configuration_display_names =
                        Arc::clone(&configuration_display_names_for_reporter);
                    let widgets = progress_widgets;
                    let status_frozen = install_run_for_reporter.stop_requested();
                    wxdragon::call_after(Box::new(move || {
                        apply_progress_event_to_ui(
                            &state,
                            &widgets,
                            &package_display_names,
                            &configuration_display_names,
                            event,
                            status_frozen,
                        );
                    }));
                });

                let cancel = install_run.begin();
                let install_run_for_worker = Arc::clone(&install_run);
                std::thread::spawn(move || {
                    let result = execute_wizard_install_with_progress(request, &progress, &cancel);
                    operation_finished.store(true, std::sync::atomic::Ordering::Relaxed);
                    wxdragon::call_after(Box::new(move || {
                        widgets.progress_gauge.set_value(100);
                        match result {
                            Ok(report) => {
                                let outcome_report = wizard_outcome_report_from_success(
                                    &ui_model,
                                    &request_for_report,
                                    &report,
                                );
                                widgets.progress_details.set_value(&format!(
                                    "{}\n\n{}",
                                    outcome_report.status_line,
                                    outcome_report.detail_lines.join("\n")
                                ));
                                set_last_resource_path(
                                    &ui_last_resource_path,
                                    Some(report.resource_path.clone()),
                                );
                                set_last_report(&ui_last_report, Some(outcome_report.clone()));
                                // Auto-save the outcome report under
                                // <resource>/RABBIT/logs/ so users always have
                                // a JSON+text trail without having to
                                // remember to click "Save report". Best
                                // effort: log to stderr and continue if the
                                // save itself fails.
                                if let Err(error) = save_wizard_outcome_report(&outcome_report) {
                                    eprintln!("could not auto-save wizard outcome report: {error}");
                                }
                                // The operation returns Ok even when some
                                // packages failed (it installs everything it
                                // can), and when the user stopped it partway.
                                // Pick an honest heading: a plain "success"
                                // line would contradict the "finished with
                                // errors" summary otherwise, and a stopped run
                                // is neither of those.
                                let heading = if report.was_cancelled() {
                                    &ui_model.text.done_status_cancelled
                                } else if report.package_operation.has_failures() {
                                    &ui_model.text.done_status_completed_with_errors
                                } else {
                                    &ui_model.text.done_status_success
                                };
                                widgets.progress_status.set_label(heading);
                                // Done page: show the outcome heading on the
                                // status TextCtrl and the full setup-report
                                // detail block in the collapsible TextCtrl.
                                widgets.done_status.set_value(&format!(
                                    "{}\n\n{}",
                                    heading, outcome_report.status_line,
                                ));
                                widgets
                                    .done_details
                                    .set_value(&outcome_report.detail_lines.join("\n"));
                                set_last_path(
                                    &ui_last_reaper_app_path,
                                    request_for_report
                                        .target_app_path
                                        .as_ref()
                                        .filter(|path| path.exists())
                                        .cloned(),
                                );
                                widgets
                                    .done_launch_reaper
                                    .enable(can_launch_last_reaper_path(&ui_last_reaper_app_path));
                                widgets.done_open_resource.enable(true);
                                // Auto-rescan: the install pipeline just
                                // wrote a fresh receipt for whatever
                                // landed, and the cached package_rows
                                // still reflect pre-install state. Fire
                                // the post-install hook the click handler
                                // armed earlier so navigating back from
                                // the Done page (or via Rescan) reflects
                                // the new on-disk state without the user
                                // having to click anything.
                                fire_post_install_hook();
                            }
                            Err(error) => {
                                let outcome_report = wizard_outcome_report_from_error(
                                    &ui_model,
                                    &request_for_report,
                                    &error,
                                );
                                set_last_report(&ui_last_report, Some(outcome_report.clone()));
                                // Same auto-save policy as the success path:
                                // failure runs are exactly when a saved log
                                // helps users diagnose what went wrong.
                                if let Err(save_error) = save_wizard_outcome_report(&outcome_report)
                                {
                                    eprintln!(
                                        "could not auto-save wizard outcome report: {save_error}"
                                    );
                                }
                                widgets.progress_details.set_value(&format!(
                                    "{}\n\n{}",
                                    outcome_report.status_line,
                                    outcome_report.detail_lines.join("\n")
                                ));
                                widgets
                                    .progress_status
                                    .set_label(&ui_model.text.done_status_error);
                                widgets.done_status.set_value(&outcome_report.status_line);
                                widgets
                                    .done_details
                                    .set_value(&outcome_report.detail_lines.join("\n"));
                                widgets
                                    .done_launch_reaper
                                    .enable(can_launch_last_reaper_path(&ui_last_reaper_app_path));
                                widgets.done_open_resource.enable(
                                    clone_last_resource_path(&ui_last_resource_path).is_some(),
                                );
                            }
                        }
                        ui_current_step.store(DONE_STEP, Ordering::SeqCst);
                        update_navigation(
                            DONE_STEP,
                            &book,
                            &step_label,
                            ui_labels.as_slice(),
                            &back,
                            &next,
                            &install,
                            &close,
                            &widgets.language_footer,
                            can_install,
                            target_is_valid(&ui_model, &widgets),
                            reapack_ack_confirmed(&widgets),
                        );
                        // Focus the always-visible status TextCtrl so the
                        // screen reader announces the install result and
                        // Tab moves forward to the Show-details CheckBox
                        // and action buttons instead of cycling back to
                        // an earlier widget.
                        widgets.done_status.set_focus();
                        // The worker is done and everything it held has been
                        // dropped, so the window can go now. Force the close:
                        // there is nothing left to ask the user about, and the
                        // close handler would otherwise re-open the same
                        // question it already answered.
                        let close_now = install_run_for_worker.stop_requested();
                        install_run_for_worker.finish();
                        if close_now {
                            with_ui_frame(|frame| frame.close(true));
                        }
                    }));
                });
            });
        }

        let frame_for_close = frame;
        close.on_click(move |_| {
            // Not forced: the frame's close handler has to be able to stop
            // this while an install is running.
            frame_for_close.close(false);
        });

        // Every route out of the wizard — the Close button, the window's
        // close box, Alt+F4, Cmd+Q — arrives here, which is the one place
        // that knows whether it is safe to go.
        {
            let model_for_close = Arc::clone(&model);
            let install_run = Arc::clone(&install_run);
            let widgets = wizard_widgets;
            let close_button = close;
            frame.on_close(move |event| {
                match close_verdict(&install_run) {
                    CloseVerdict::Allow => {
                        // Let wxWidgets' own frame handler run and destroy
                        // the window. Nothing is in flight, so there is
                        // nothing to unwind.
                        event.skip(true);
                    }
                    CloseVerdict::SelfUpdateInProgress => {
                        // Not a question: there is no answer that makes
                        // quitting safe here, so say what is happening
                        // instead of offering a choice RABBIT can't honour.
                        show_close_blocked_by_self_update(&model_for_close);
                    }
                    CloseVerdict::AlreadyStopping => {
                        // A second press while the first stop is still
                        // unwinding. Re-announce rather than re-ask.
                        announce_stopping(&widgets, &model_for_close);
                    }
                    CloseVerdict::Ask => {
                        if !confirm_stop_install(&model_for_close) {
                            return;
                        }
                        if !install_run.request_stop_and_close() {
                            // The install finished while the question was on
                            // screen, so there is nothing left to stop and
                            // nobody left to close the window for us. Queue
                            // the close for the next turn of the event loop
                            // rather than re-entering this handler from
                            // inside itself.
                            wxdragon::call_after(Box::new(|| {
                                with_ui_frame(|frame| frame.close(true));
                            }));
                            return;
                        }
                        // The button that raised the question is now the
                        // wrong thing to press again.
                        close_button.enable(false);
                        announce_stopping(&widgets, &model_for_close);
                    }
                }
                // Every branch except `Allow` falls through without
                // skipping, which is what keeps the window alive: the
                // default handler that would destroy it never runs.
            });
        }

        // Handle the application-menu Quit item: the macOS menu bar
        // installed below carries a stock wxID_EXIT item, which wxOSX
        // relocates into the application menu as "Quit rabbit" (Cmd+Q).
        // Selecting it emits a plain menu command with that id at the
        // frame — route it to the same teardown as the wizard's Close
        // button and the window close box. Bound on every platform, but
        // only macOS installs a menu that can emit it.
        {
            let frame_for_quit = frame;
            frame.on_menu_selected(move |event| {
                if event.get_id() == wxdragon::id::ID_EXIT {
                    frame_for_quit.close(false);
                }
            });
        }

        // Without any menu bar, wxOSX installs no functional main menu at
        // all: the wizard's application menu had a dead Quit item and Cmd+Q
        // did nothing. Installing a minimal menu bar makes wxWidgets build
        // a real application menu and wire its Quit item to the stock
        // wxID_EXIT command handled above. `Ctrl+Q` in a wx accelerator
        // string means Cmd+Q on macOS. macOS-only: on Windows this would
        // add a visible File menu to a wizard that doesn't want one, and
        // Alt+F4 already closes the window natively there.
        #[cfg(target_os = "macos")]
        {
            let file_menu = Menu::builder()
                .append_item(wxdragon::id::ID_EXIT, "E&xit\tCtrl+Q", "")
                .build();
            let menu_bar = MenuBar::builder().append(file_menu, "&File").build();
            frame.set_menu_bar(menu_bar);
        }

        {
            let model = Arc::clone(&model);
            let widgets = wizard_widgets;
            let last_reaper_app_path = Arc::clone(&last_reaper_app_path);
            let frame_for_launch = frame;
            widgets.done_launch_reaper.on_click(move |_| {
                let Some(app_path) = clone_last_path(&last_reaper_app_path) else {
                    append_done_status(&widgets.done_status, &model.text.done_no_reaper_app);
                    return;
                };
                if let Err(error) = launch_reaper(&app_path) {
                    append_done_status(
                        &widgets.done_status,
                        &format!("{}: {}", model.text.done_launch_reaper_error_prefix, error),
                    );
                    return;
                }
                frame_for_launch.close(true);
            });
        }

        {
            let model = Arc::clone(&model);
            let widgets = wizard_widgets;
            let last_resource_path = Arc::clone(&last_resource_path);
            widgets.done_open_resource.on_click(move |_| {
                let Some(path) = clone_last_resource_path(&last_resource_path) else {
                    append_done_status(&widgets.done_status, &model.text.review_no_target);
                    return;
                };
                if let Err(error) = open_resource_folder(&path) {
                    append_done_status(
                        &widgets.done_status,
                        &format!("{}: {}", model.text.done_open_resource_error_prefix, error),
                    );
                }
            });
        }

        // (The "Save report" button used to live on the Done page so the
        // user could re-save the outcome JSON+text manually. RABBIT already
        // auto-saves under `<resource>/RABBIT/logs/` on every run — both
        // success and failure paths — so the manual button was redundant
        // and added clutter on a page meant to read like a destination,
        // not a dashboard.)

        let self_update_state = Arc::new(Mutex::new(SelfUpdateUiState::default()));

        // One-shot startup probe: runs the self-update manifest check
        // and stores the result into the shared state, then renders.
        // (Used to also poll a global package-install lock — that lock
        // is now per-target, so the cross-target probe is gone.)
        {
            let model = Arc::clone(&model);
            let widgets = wizard_widgets;
            let state = Arc::clone(&self_update_state);
            std::thread::spawn(move || {
                let check = run_wizard_self_update_check();
                // Resolve the What's-New notes on this same worker, while
                // the status line still reads "Checking for RABBIT
                // updates…": the prompt is only raised once the whole probe
                // lands on the UI thread, so fetching notes here costs the
                // user nothing and spares the UI thread an HTTP round-trip.
                let release_notes = check
                    .as_ref()
                    .ok()
                    .and_then(run_wizard_self_update_release_notes);
                {
                    let mut state = state.lock().unwrap();
                    state.release_notes = release_notes;
                    state.check = Some(match check {
                        Ok(report) => Ok(report),
                        Err(error) => Err(error.to_string()),
                    });
                }
                let render_state = Arc::clone(&state);
                let render_model = Arc::clone(&model);
                wxdragon::call_after(Box::new(move || {
                    with_ui_localizer(|localizer| {
                        render_self_update_status(widgets, &render_model, localizer, &render_state);
                    });
                }));
            });
        }

        // (Used to also spawn a polling thread that re-checked a global
        // install lock and re-rendered when another RABBIT process started
        // an install. With per-target locks there's no global lock to
        // poll; if a same-target race happens, the install path surfaces
        // it as a `PackageInstallInProgress` error at acquire time.)

        // (The Done page used to host an "Apply RABBIT update" button as
        // an always-reachable fallback to the once-per-session prompt. It
        // was removed because users couldn't find it before completing an
        // install — the modal at startup is now the only entry point, and
        // a user who picks "No" gets re-prompted by relaunching RABBIT.)

        // (The "Rescan target" button used to live here so the user could
        // re-detect installed components on the Done page and jump back
        // to the Packages step. With the post-install auto-rescan hook,
        // package_rows is already up to date by the time the user lands
        // on Done — manual rescan is a debugging affordance. Users who
        // want to re-detect can just relaunch RABBIT.)

        frame.centre();
        frame.show(true);
    });
}
