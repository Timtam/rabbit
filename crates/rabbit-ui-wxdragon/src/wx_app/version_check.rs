//! The version-check page: fans out upstream version lookups on worker
//! threads and folds the answers back into the package rows.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{
    ConfigurationRow, PackageRow, TargetRow, WizardModel, apply_version_check_failures_to_rows,
    localized_package_display_name, localizer_from_options,
    recompute_configuration_row_availability, wizard_desired_package_ids,
    wizard_package_plan_for_target_with_available,
};
use rabbit_core::detection::detect_components;
use rabbit_core::latest::fetch_latest_details_for_package;
use rabbit_core::model::Platform;
use rabbit_core::package::PACKAGE_REAPER;
use rabbit_core::plan::AvailablePackage;
use rabbit_core::version::Version;
use wxdragon::widgets::SimpleBook;

use wxdragon::prelude::*;

use crate::wx_app::PACKAGES_STEP;
use crate::wx_app::globals::{
    VersionCheckEvent, dispatch_version_check_event, install_version_check_dispatcher,
    with_ui_localizer,
};
use crate::wx_app::packages_page::{
    PackagesStateCell, focus_packages_list_top, rebuild_package_list_widgets,
};
use crate::wx_app::widgets::{
    WizardWidgets, effective_can_install, reapack_ack_confirmed, update_navigation,
};

/// Captures everything the version-check dispatcher needs to drive the
/// dedicated version-check page: widgets, model, package-row state for the
/// auto-rebuild on success, and the navigation handles needed to advance to
/// the Packages step.
pub(crate) struct VersionCheckUi {
    pub(crate) widgets: WizardWidgets,
    pub(crate) model: Arc<WizardModel>,
    pub(crate) package_rows: Rc<RefCell<Vec<PackageRow>>>,
    pub(crate) package_notes: Rc<RefCell<Vec<String>>>,
    pub(crate) configuration_rows: Rc<RefCell<Vec<ConfigurationRow>>>,
    pub(crate) package_items: PackagesStateCell,
    pub(crate) can_install: Rc<Cell<bool>>,
    pub(crate) review_can_install: Rc<Cell<bool>>,
    pub(crate) target: TargetRow,
    pub(crate) book: SimpleBook,
    pub(crate) step_label: StaticText,
    pub(crate) labels: Arc<Vec<String>>,
    pub(crate) back: Button,
    pub(crate) next: Button,
    pub(crate) install: Button,
    pub(crate) close: Button,
    pub(crate) current_step: Arc<AtomicUsize>,
}

/// Reset the version-check page to its starting state, install the dispatcher
/// that handles per-package events on the UI thread, and spawn the worker
/// thread. The dispatcher auto-advances to the Packages step on full success;
/// on any failure it stays on the version-check page with the error log
/// populated and the Back button enabled.
pub(crate) fn start_version_check(ui: VersionCheckUi) {
    let package_ids = wizard_desired_package_ids(ui.model.platform);
    let package_count = package_ids.len() as i32;
    // Snapshot what the worker's since-installed What's-New trim needs
    // before `ui` moves into the dispatcher closure below.
    let target_resource_path = ui.target.path.clone();
    let target_platform = ui.model.platform;
    let target_reaper_version = ui.target.version.clone();
    ui.widgets
        .version_check_status
        .set_label(&ui.model.text.version_check_status_pending);
    ui.widgets.version_check_gauge.set_value(0);
    ui.widgets
        .version_check_gauge
        .set_range(package_count.max(1));
    ui.widgets.version_check_error_log.set_value("");
    // The error region stays out of the tab order and the a11y tree until a
    // check actually fails — see render_version_check_errors for the show.
    ui.widgets.version_check_error_heading.hide();
    ui.widgets.version_check_error_log.hide();

    let mut accumulated: Vec<AvailablePackage> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut completed: i32 = 0;

    let dispatcher = move |event: VersionCheckEvent| match event {
        VersionCheckEvent::Result {
            package_id,
            outcome,
        } => {
            completed += 1;
            ui.widgets.version_check_gauge.set_value(completed);
            with_ui_localizer(|localizer| {
                let line = localizer
                    .format(
                        "wizard-version-check-status-progress",
                        &[
                            ("done", completed.to_string().as_str()),
                            ("total", package_count.to_string().as_str()),
                        ],
                    )
                    .value;
                ui.widgets.version_check_status.set_label(&line);
            });
            match outcome {
                Ok((version_str, whats_new)) => {
                    match rabbit_core::version::Version::parse(&version_str) {
                        Ok(version) => {
                            accumulated.push(AvailablePackage {
                                package_id,
                                version: Some(version),
                                whats_new,
                            });
                        }
                        Err(error) => {
                            errors.push((package_id, error.to_string()));
                        }
                    }
                }
                Err(message) => {
                    errors.push((package_id, message));
                }
            }
        }
        VersionCheckEvent::Finished => {
            // Per-package failures (an upstream being down, a parse error)
            // no longer block the wizard: the plan is built from whatever
            // versions did resolve, and each failed package's row is
            // disabled with a localized reason + a Review-page note carrying
            // the full error. Only a failure to build the plan itself keeps
            // the wizard on this page with the error log shown.
            {
                match wizard_package_plan_for_target_with_available(
                    &ui.model,
                    Some(&ui.target),
                    &accumulated,
                ) {
                    Ok(mut plan) => {
                        if !errors.is_empty()
                            && let Ok(localizer) =
                                localizer_from_options(&ui.model.bootstrap_options)
                        {
                            plan.can_install = apply_version_check_failures_to_rows(
                                &localizer,
                                &mut plan.package_rows,
                                &mut plan.notes,
                                &errors,
                            );
                        }
                        *ui.package_rows.borrow_mut() = plan.package_rows;
                        *ui.package_notes.borrow_mut() = plan.notes;
                        // The deferred fetch may have promoted ReaPack to
                        // Update (or vice versa); refresh configuration
                        // row availability against the fresh plan.
                        if let Ok(localizer) = localizer_from_options(&ui.model.bootstrap_options) {
                            recompute_configuration_row_availability(
                                &localizer,
                                &ui.package_rows.borrow(),
                                Some(&ui.target.path),
                                &mut ui.configuration_rows.borrow_mut(),
                            );
                        }
                        ui.can_install.set(plan.can_install);
                        ui.review_can_install.set(false);
                        // Show the Spanish variant the target already has
                        // installed, so a Team PMA user isn't presented with
                        // an unticked box that would switch them back.
                        ui.widgets
                            .spanish_variant_choice
                            .set_selection(crate::spanish_variant_selection(&ui.target.path));
                        rebuild_package_list_widgets(
                            &ui.widgets,
                            &ui.package_items,
                            &ui.model,
                            &ui.package_rows.borrow(),
                            &ui.configuration_rows.borrow(),
                        );
                        ui.current_step.store(PACKAGES_STEP, Ordering::SeqCst);
                        update_navigation(
                            PACKAGES_STEP,
                            &ui.book,
                            &ui.step_label,
                            ui.labels.as_slice(),
                            &ui.back,
                            &ui.next,
                            &ui.install,
                            &ui.close,
                            &ui.widgets.language_footer,
                            effective_can_install(&ui.can_install, &ui.review_can_install),
                            true,
                            reapack_ack_confirmed(&ui.widgets),
                        );
                        // The wizard just auto-advanced onto the packages
                        // page: land the screen reader on the top of the
                        // list (the "Packages" group header), not wherever
                        // the native control left its caret after the
                        // repopulation above. Mirrors the explicit gauge
                        // focus on entering the version-check page.
                        focus_packages_list_top(&ui.widgets, &ui.package_items);
                    }
                    Err(error) => {
                        errors.push((String::new(), error.to_string()));
                        render_version_check_errors(&ui, &errors);
                    }
                }
            }
        }
    };

    install_version_check_dispatcher(Box::new(dispatcher));
    spawn_version_check_worker(
        package_ids,
        target_resource_path,
        target_platform,
        target_reaper_version,
    );
}

/// Render error lines to the version-check page's error TextCtrl and update
/// the status text to point the user at Back/Close.
pub(crate) fn render_version_check_errors(ui: &VersionCheckUi, errors: &[(String, String)]) {
    with_ui_localizer(|localizer| {
        let mut lines = Vec::with_capacity(errors.len());
        for (package_id, message) in errors {
            let display = if package_id.is_empty() {
                String::new()
            } else {
                localized_package_display_name(localizer, package_id)
            };
            let line = localizer
                .format(
                    "wizard-version-check-error-line",
                    &[("package", display.as_str()), ("message", message.as_str())],
                )
                .value;
            lines.push(line);
        }
        ui.widgets
            .version_check_error_log
            .set_value(&lines.join("\n"));
        // Surface the error region now that there is content for screen
        // readers + the tab order to expose.
        ui.widgets.version_check_error_heading.show(true);
        ui.widgets.version_check_error_log.show(true);
        let status = localizer
            .format(
                "wizard-version-check-status-error",
                &[("error_count", errors.len().to_string().as_str())],
            )
            .value;
        ui.widgets.version_check_status.set_label(&status);
    });
}

/// Spawn the deferred latest-version fetch on a background thread. Each
/// per-package outcome is forwarded to the UI thread via `call_after`, which
/// invokes the dispatcher installed by the click handler.
///
/// Before fetching, the worker detects what's already installed at the
/// target so each package's What's-New notes can be trimmed to the changes
/// since the installed version. Detection here is best-effort: a failure
/// just means untrimmed notes, never a failed check.
/// How many latest-version checks run at once. Matches the download pool's
/// modest fan-out: enough to hide one slow check behind the others without
/// hammering hosts that serve several packages.
pub(crate) const VERSION_CHECK_CONCURRENCY: usize = 4;

pub(crate) fn spawn_version_check_worker(
    package_ids: Vec<String>,
    resource_path: PathBuf,
    platform: Platform,
    target_reaper_version: Option<Version>,
) {
    std::thread::spawn(move || {
        let mut installed_versions: HashMap<String, Version> =
            detect_components(&resource_path, platform)
                .map(|detections| {
                    detections
                        .into_iter()
                        .filter(|detection| detection.installed)
                        .filter_map(|detection| Some((detection.package_id, detection.version?)))
                        .collect()
                })
                .unwrap_or_default();
        // REAPER itself is versioned by the selected target row, not by the
        // component detections that cover the extensions.
        if let Some(version) = target_reaper_version {
            installed_versions.insert(PACKAGE_REAPER.to_string(), version);
        }
        // Check packages CONCURRENTLY. The checks are independent network
        // fetches against different hosts, and their costs are wildly
        // uneven — most resolve in about a second, but a language pack is
        // identified by the hash of its contents, so checking one downloads
        // the whole file (the German pack is ~1.7 MB and takes several
        // seconds). Run sequentially that one check stalled the whole
        // progress bar; overlapped, the run costs roughly the slowest single
        // check instead of the sum of all of them.
        //
        // Bounded pool rather than one thread per package: a handful of
        // parallel requests is polite to the upstream hosts (several
        // packages share one) and is where the wall-clock win already
        // saturates.
        let queue = std::sync::Arc::new(std::sync::Mutex::new(
            package_ids
                .into_iter()
                .collect::<std::collections::VecDeque<_>>(),
        ));
        let installed_versions = std::sync::Arc::new(installed_versions);
        let worker_count =
            VERSION_CHECK_CONCURRENCY.min(queue.lock().map(|q| q.len()).unwrap_or(1).max(1));
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let queue = std::sync::Arc::clone(&queue);
            let installed_versions = std::sync::Arc::clone(&installed_versions);
            workers.push(std::thread::spawn(move || {
                loop {
                    // Scope the lock so it is never held across a fetch.
                    let next = queue.lock().ok().and_then(|mut q| q.pop_front());
                    let Some(package_id) = next else {
                        break;
                    };
                    let installed = installed_versions.get(&package_id);
                    let outcome = match fetch_latest_details_for_package(&package_id, installed) {
                        Ok(details) => Ok((details.version.to_string(), details.whats_new)),
                        Err(error) => Err(error.to_string()),
                    };

                    let id_for_result = package_id.clone();
                    wxdragon::call_after(Box::new(move || {
                        dispatch_version_check_event(VersionCheckEvent::Result {
                            package_id: id_for_result,
                            outcome,
                        });
                    }));
                }
            }));
        }
        for worker in workers {
            let _ = worker.join();
        }
        wxdragon::call_after(Box::new(move || {
            dispatch_version_check_event(VersionCheckEvent::Finished);
        }));
    });
}
