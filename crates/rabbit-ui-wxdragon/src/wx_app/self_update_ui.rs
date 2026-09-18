//! Self-update in the wizard: the status line, the prompt with release
//! notes, and the progress window the apply step drives.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::{
    WizardModel, format_self_update_apply_summary, format_self_update_check_summary,
    relaunch_rabbit_after_apply, run_wizard_self_update_apply,
};
use rabbit_core::localization::Localizer;
use rabbit_core::progress::{ProgressEvent, ProgressReporter};
use rabbit_core::self_update::SelfUpdateCheckReport;

use wxdragon::prelude::*;

use crate::wx_app::globals::{with_ui_frame, with_ui_localizer};
use crate::wx_app::packages_page::{WXK_NUMPAD_ENTER, WXK_RETURN};
use crate::wx_app::progress_ui::format_bytes_human;
use crate::wx_app::widgets::{WizardWidgets, append_done_status};

/// Set while the self-update worker is swapping RABBIT's own files. That
/// swap has no unwind path — the whole point of the progress window having
/// no Cancel button — so the close handler refuses to quit under it rather
/// than leaving a half-replaced executable behind. A plain static because
/// `start_self_update_apply` is reachable from the startup prompt and from
/// the Done page, neither of which shares the wizard's state.
pub(crate) static SELF_UPDATE_APPLYING: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
pub(crate) struct SelfUpdateUiState {
    /// Result of the one-shot manifest check at startup. `None` while the
    /// startup probe is still running; `Some(Ok)` on success; `Some(Err)`
    /// carries the formatted error message (RabbitError isn't Clone).
    pub(crate) check: Option<std::result::Result<SelfUpdateCheckReport, String>>,
    /// What's-New notes for the pending update, resolved by the same startup
    /// worker as `check`. `None` when there is no update, or when the notes
    /// couldn't be fetched — the prompt then falls back to its plain form.
    pub(crate) release_notes: Option<String>,
    /// Last status string written to the status bar — used to suppress
    /// screen-reader re-announcements when nothing has changed.
    pub(crate) last_status: String,
    /// `true` once the once-per-session "RABBIT update available" prompt
    /// dialog has been shown. Re-renders that follow the same check
    /// result (e.g., a step change that re-invokes render) skip the
    /// modal so the user isn't re-prompted after dismissing it.
    pub(crate) prompted: bool,
}

pub(crate) fn render_self_update_status(
    widgets: WizardWidgets,
    model: &Arc<WizardModel>,
    localizer: &Localizer,
    state: &Arc<Mutex<SelfUpdateUiState>>,
) {
    let mut state_guard = state.lock().unwrap();
    // Clone the check result up-front so the rest of the function can
    // freely mutate `state_guard` without fighting the borrow checker
    // over an `as_ref()` view of `state_guard.check`. The clone is
    // cheap (a single `Result<SelfUpdateCheckReport, String>`) and the
    // function only runs on completion of a one-shot manifest probe.
    let Some(check) = state_guard.check.clone() else {
        // Startup probe hasn't completed yet; leave the initial
        // "Checking for RABBIT updates…" placeholder in place.
        return;
    };

    // (The package-install lock used to be a single LocalAppData path so
    // RABBIT could warn that another install was in progress before
    // applying a self-update. With locks now scoped per-target we don't
    // have a single global lock to consult here, so the cross-target
    // status line is gone. Concurrent self-update + install on the same
    // target still races at the file rename and surfaces a normal IO
    // error.)
    let status = match &check {
        Ok(report) => format_self_update_check_summary(localizer, report),
        Err(error) => format!("{}: {}", model.text.done_self_update_error_prefix, error),
    };
    let apply_enabled = matches!(&check, Ok(report) if report.update_available);

    let status_changed = status != state_guard.last_status;
    if status_changed {
        widgets.self_update_status.set_status_text(&status, 0);
        state_guard.last_status = status;
    }

    // Once-per-session prompt: if an update is available, ask up front
    // instead of forcing the user to navigate to the Done page to find
    // the apply button. The Done-page button stays around as a fallback
    // for users who pick "No" here and change their mind later.
    if !apply_enabled || state_guard.prompted {
        return;
    }
    state_guard.prompted = true;
    let Ok(report) = check else { return };
    let release_notes = state_guard.release_notes.clone();
    // Drop the lock before showing the modal — `MessageDialog::show_modal`
    // runs a nested wxWidgets event loop, and any UI-thread callback that
    // re-enters `render_self_update_status` while the modal is open would
    // deadlock on a still-held mutex.
    drop(state_guard);

    let title = localizer.text("wizard-self-update-prompt-title").value;
    let current = report.current_version.to_string();
    let latest = report.latest_version.to_string();
    let body = localizer
        .format(
            "wizard-self-update-prompt-body",
            &[("current", current.as_str()), ("latest", latest.as_str())],
        )
        .value;

    // Pull the frame from the UI-thread-local rather than from the
    // captured `widgets` so we don't have to send a non-`Send` `Frame`
    // through the `call_after` closure that wraps this function. The
    // closure runs on the UI thread, so the thread-local was populated
    // by `run()` before any worker fired.
    with_ui_frame(|frame| {
        let accepted = match release_notes.as_deref() {
            Some(notes) => {
                show_self_update_prompt_with_notes(frame, localizer, &title, &body, &current, notes)
            }
            // No notes to show (fetch failed, or the release carries no
            // body): the plain native message box, exactly as before.
            None => {
                let dialog = MessageDialog::builder(frame, &body, &title)
                    .with_style(
                        MessageDialogStyle::YesNo
                            | MessageDialogStyle::IconQuestion
                            | MessageDialogStyle::Centre,
                    )
                    .build();
                dialog.show_modal() == ID_YES
            }
        };

        if accepted {
            start_self_update_apply(
                widgets.done_status,
                widgets.self_update_status,
                Arc::clone(model),
            );
        }
    });
}

/// The update prompt in its rich form: the same question the plain message
/// box asks, plus the release notes for everything the update brings, so the
/// answer to "should I say yes?" is on screen when the question is asked.
/// Returns `true` when the user accepted.
///
/// A `wxMessageDialog` can't host this — its message is a single unscrollable
/// static label, and RABBIT's release notes routinely run past a screenful.
/// Hence a plain `Dialog` carrying a read-only multiline `TextCtrl`, which is
/// the same control (and therefore the same screen-reader behaviour) the
/// packages page already uses for a package's What's-New notes.
///
/// Accessibility notes, in the spirit of the Done page:
/// - Focus parks on the notes control, so the screen reader reads what the
///   update contains instead of announcing a bare button.
/// - The notes control's accessible name carries the heading, since a
///   `StaticText` label sits outside the tab order and would otherwise never
///   be announced.
/// - Update is the default button, so Enter accepts from anywhere in the
///   dialog, and Escape maps to Later.
pub(crate) fn show_self_update_prompt_with_notes(
    frame: &Frame,
    localizer: &Localizer,
    title: &str,
    body: &str,
    current: &str,
    notes: &str,
) -> bool {
    let notes_heading = localizer
        .format(
            "wizard-self-update-prompt-notes-heading",
            &[("current", current)],
        )
        .value;
    let update_label = localizer
        .text("wizard-self-update-prompt-update-button")
        .value;
    let later_label = localizer
        .text("wizard-self-update-prompt-later-button")
        .value;

    let dialog = Dialog::builder(frame, title)
        // Resizable so a long set of notes can be opened up, rather than
        // forcing everything through one fixed-height scroll region.
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(560, 420)
        .build();
    // Controls go on a child Panel rather than straight onto the dialog:
    // that's what gives MSW its dialog-navigation behaviour (Tab/arrow
    // traversal between the notes and the buttons) for free.
    let panel = Panel::builder(&dialog).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let question = StaticText::builder(&panel).with_label(body).build();
    question.set_name("rabbit-self-update-prompt-question");
    sizer.add(&question, 0, SizerFlag::All | SizerFlag::Expand, 6);

    let heading = StaticText::builder(&panel)
        .with_label(&notes_heading)
        .build();
    heading.set_name("rabbit-self-update-prompt-notes-heading");
    sizer.add(&heading, 0, SizerFlag::All | SizerFlag::Expand, 6);

    let notes_text = TextCtrl::builder(&panel)
        .with_value(notes)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .build();
    notes_text.set_name(&notes_heading);
    sizer.add(&notes_text, 1, SizerFlag::All | SizerFlag::Expand, 6);

    // The buttons carry the standard yes/no ids rather than generated ones
    // so `set_escape_id` below has something to act on: wxWidgets answers
    // Escape by emulating a click on the button holding the escape id, and
    // does nothing at all when no button has it.
    //
    // `TabStop` + `set_can_focus` for the same reason the wizard's own
    // navigation buttons carry them: without it macOS leaves buttons out of
    // the Tab ring unless Full Keyboard Access is on.
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let update = Button::builder(&panel)
        .with_id(ID_YES)
        .with_label(&update_label)
        .build();
    update.set_name("rabbit-self-update-prompt-update");
    update.add_style(WindowStyle::TabStop);
    update.set_can_focus(true);
    buttons.add(&update, 0, SizerFlag::All, 6);
    let later = Button::builder(&panel)
        .with_id(ID_NO)
        .with_label(&later_label)
        .build();
    later.set_name("rabbit-self-update-prompt-later");
    later.add_style(WindowStyle::TabStop);
    later.set_can_focus(true);
    buttons.add(&later, 0, SizerFlag::All, 6);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight, 0);

    panel.set_sizer(sizer, true);
    // Same frame → panel → content nesting `run()` builds for the wizard
    // window: the dialog's own sizer is what makes the panel track the
    // dialog when it is resized.
    let dialog_sizer = BoxSizer::builder(Orientation::Vertical).build();
    dialog_sizer.add(&panel, 1, SizerFlag::Expand, 0);
    dialog.set_sizer(dialog_sizer, true);

    // `Dialog` is `Copy`, so each `move` handler closes over its own copy
    // and the original stays usable below. Neither handler skips the event,
    // so it stops at the button and wxDialog's own button handling never
    // gets a second go at ending the modal.
    update.on_click(move |_| dialog.end_modal(ID_YES));
    later.on_click(move |_| dialog.end_modal(ID_NO));
    dialog.set_affirmative_id(ID_YES);
    dialog.set_escape_id(ID_NO);
    update.set_default();

    // A multiline TextCtrl claims Enter for itself (DLGC_WANTALLKEYS on
    // MSW, the NSTextView swallows it on macOS), so the default button
    // never sees the key while focus is parked on the notes — the same
    // trap `bind_done_page_enter_closes` works around on the Done page.
    notes_text.on_key_down(move |event| {
        let key_code = if let WindowEventData::Keyboard(kbd) = &event {
            kbd.get_key_code()
        } else {
            None
        };
        if !matches!(key_code, Some(WXK_RETURN) | Some(WXK_NUMPAD_ENTER)) {
            return;
        }
        // Consume the key so the read-only control neither beeps nor
        // tries to insert a newline before the dialog closes.
        event.skip(false);
        dialog.end_modal(ID_YES);
    });

    notes_text.set_focus();
    let accepted = dialog.show_modal() == ID_YES;
    dialog.destroy();
    accepted
}

/// Trigger the self-update apply pipeline on a worker thread, routing
/// progress (start, summary, relaunch / error) to both the Done page's
/// `done_status` text control and the always-visible `self_update_status`
/// status bar. Two surfaces because the apply can be invoked from two
/// places: the Done page button (where `done_status` is the natural
/// detail surface and `self_update_status` is a redundant short-form),
/// and the once-per-session "RABBIT update available" prompt at startup
/// (where the user is on the Target step and only `self_update_status`
/// is visible). The duplication keeps both call sites simple — neither
/// has to know which surface their user can see.
///
/// Takes individual widget handles rather than the full `WizardWidgets`
/// because that struct now holds a `Frame` (for parenting modal
/// dialogs) and `Frame` isn't `Send` — capturing the whole struct
/// into the spawned worker would break the closure's `Send` bound.
pub(crate) fn start_self_update_apply(
    done_status: TextCtrl,
    self_update_status: StatusBar,
    model: Arc<WizardModel>,
) {
    append_done_status(&done_status, &model.text.done_self_update_apply_running);
    self_update_status.set_status_text(&model.text.done_self_update_apply_running, 0);
    // Put the progress window up before the worker starts, so the very
    // first event has somewhere to land. It is modeless: the worker's
    // completion path runs through `call_after` on this same thread, which
    // a modal dialog's nested event loop would let run but would also trap
    // the exit-after-relaunch behind its own dismissal.
    open_self_update_progress_window(&model);
    // Held for the whole swap so the close handler refuses to quit under a
    // half-replaced RABBIT. Cleared on every exit from the worker, including
    // the failure paths — the relaunch path exits the process instead.
    SELF_UPDATE_APPLYING.store(true, Ordering::SeqCst);
    let model_for_thread = Arc::clone(&model);
    std::thread::spawn(move || {
        // Every event crosses back to the UI thread — widgets must not be
        // touched from here — and `ProgressEvent` is `Send`, so it rides
        // inside the `call_after` closure.
        let progress = ProgressReporter::new(|event| {
            wxdragon::call_after(Box::new(move || {
                update_self_update_progress_window(&event);
            }));
        });
        let result = run_wizard_self_update_apply(&progress);
        SELF_UPDATE_APPLYING.store(false, Ordering::SeqCst);
        wxdragon::call_after(Box::new(close_self_update_progress_window));
        wxdragon::call_after(Box::new(move || match result {
            Ok(report) => {
                with_ui_localizer(|localizer| {
                    let summary = format_self_update_apply_summary(localizer, &report);
                    append_done_status(&done_status, &summary);
                    self_update_status.set_status_text(&summary, 0);
                });
                if !report.replaced_files.is_empty() {
                    match relaunch_rabbit_after_apply() {
                        Ok(pid) => {
                            let msg = format!(
                                "{}: PID {}",
                                model_for_thread.text.done_self_update_relaunch_prefix, pid
                            );
                            append_done_status(&done_status, &msg);
                            self_update_status.set_status_text(&msg, 0);
                            // Mirror relaunch_with_locale: hand off to the new
                            // process and exit, otherwise the pre-update GUI
                            // sticks around next to the freshly-launched copy.
                            std::process::exit(0);
                        }
                        Err(error) => {
                            let msg = format!(
                                "{}: {}",
                                model_for_thread.text.done_self_update_error_prefix, error
                            );
                            append_done_status(&done_status, &msg);
                            self_update_status.set_status_text(&msg, 0);
                        }
                    }
                }
            }
            Err(error) => {
                let msg = format!(
                    "{}: {}",
                    model_for_thread.text.done_self_update_error_prefix, error
                );
                append_done_status(&done_status, &msg);
                self_update_status.set_status_text(&msg, 0);
            }
        }));
    });
}

/// Widgets of the live self-update progress window. Held in a thread-local
/// because none of them are `Send`: the worker thread never touches them,
/// it only posts `ProgressEvent`s through `call_after`, and the closure
/// that runs on the UI thread picks the widgets up from here.
pub(crate) struct SelfUpdateProgressWindow {
    pub(crate) dialog: Dialog,
    pub(crate) status: StaticText,
    pub(crate) gauge: Gauge,
    pub(crate) log: TextCtrl,
    /// Localized product name, resolved once at construction so each event
    /// doesn't have to go back through the localizer for it.
    pub(crate) app_name: String,
}

thread_local! {
    static SELF_UPDATE_PROGRESS: RefCell<Option<SelfUpdateProgressWindow>> =
        const { RefCell::new(None) };
}

/// Share of the bar given to the download. The remainder covers the
/// install, which is a single opaque step: the bar jumps to this mark when
/// the bytes are in and finishes when the swap is done.
pub(crate) const SELF_UPDATE_DOWNLOAD_SHARE: i32 = 90;

/// Put up the update progress window: a status line, a bar, and a running
/// log of the phases as they complete.
///
/// The same three-part shape as the wizard's own progress page, for the
/// same reason — the bar is what a sighted user reads, while the log is
/// what a screen reader can be walked through afterwards, line by line,
/// which a bar alone never affords. Modeless, and it takes no buttons: the
/// update cannot be cancelled once started (nothing in the core can unwind
/// a half-applied swap), so a Cancel button would be a lie.
pub(crate) fn open_self_update_progress_window(model: &Arc<WizardModel>) {
    close_self_update_progress_window();
    with_ui_frame(|frame| {
        with_ui_localizer(|localizer| {
            let title = localizer.text("wizard-self-update-progress-title").value;
            let app_name = localizer.text("app-short-name").value;
            let dialog = Dialog::builder(frame, &title)
                .with_style(DialogStyle::Caption | DialogStyle::ResizeBorder)
                .with_size(460, 260)
                .build();
            let panel = Panel::builder(&dialog).build();
            let sizer = BoxSizer::builder(Orientation::Vertical).build();

            let status = StaticText::builder(&panel)
                .with_label(&model.text.done_self_update_apply_running)
                .build();
            status.set_name("rabbit-self-update-progress-status");
            sizer.add(&status, 0, SizerFlag::All | SizerFlag::Expand, 6);

            let gauge = Gauge::builder(&panel).with_range(100).build();
            gauge.set_name("rabbit-self-update-progress-gauge");
            sizer.add(&gauge, 0, SizerFlag::All | SizerFlag::Expand, 6);

            let log = TextCtrl::builder(&panel)
                .with_value("")
                .with_style(
                    TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap,
                )
                .build();
            log.set_name(&title);
            sizer.add(&log, 1, SizerFlag::All | SizerFlag::Expand, 6);

            panel.set_sizer(sizer, true);
            let dialog_sizer = BoxSizer::builder(Orientation::Vertical).build();
            dialog_sizer.add(&panel, 1, SizerFlag::Expand, 0);
            dialog.set_sizer(dialog_sizer, true);

            // No Cancel button also means Escape does nothing: wxWidgets
            // answers it by emulating a click on the escape-id button, and
            // there is none here. That is the behaviour we want — dismissing
            // the window would leave the update running unreported.
            dialog.show(true);
            log.set_focus();

            SELF_UPDATE_PROGRESS.with(|cell| {
                *cell.borrow_mut() = Some(SelfUpdateProgressWindow {
                    dialog,
                    status,
                    gauge,
                    log,
                    app_name,
                });
            });
        });
    });
}

/// Fold one progress event into the window. Silently does nothing when no
/// window is up — events can still be in flight when it has been closed.
pub(crate) fn update_self_update_progress_window(event: &ProgressEvent) {
    SELF_UPDATE_PROGRESS.with(|cell| {
        let borrowed = cell.borrow();
        let Some(window) = borrowed.as_ref() else {
            return;
        };
        with_ui_localizer(|localizer| {
            let package = window.app_name.as_str();
            match event {
                ProgressEvent::DownloadStarted { .. } => {
                    window.gauge.set_value(0);
                    window.status.set_label(
                        &localizer
                            .format(
                                "wizard-progress-status-downloading",
                                &[("package", package)],
                            )
                            .value,
                    );
                    // Only the phase change is logged; the byte ticks below
                    // would flood a screen reader reading the log.
                    append_self_update_progress_log(
                        window,
                        &localizer
                            .format(
                                "wizard-progress-log-download-started",
                                &[("package", package)],
                            )
                            .value,
                    );
                }
                ProgressEvent::DownloadProgress {
                    bytes_downloaded,
                    bytes_total,
                    ..
                } => {
                    // Without a Content-Length there is no fraction to show,
                    // so the bar stays where it is and the byte counter in
                    // the status line carries the news instead.
                    if let Some(total) = bytes_total.filter(|total| *total > 0) {
                        let ratio = (*bytes_downloaded as f64 / total as f64).clamp(0.0, 1.0);
                        window
                            .gauge
                            .set_value((ratio * SELF_UPDATE_DOWNLOAD_SHARE as f64) as i32);
                    }
                    let downloaded = format_bytes_human(*bytes_downloaded);
                    let total = bytes_total.map_or_else(|| "?".to_string(), format_bytes_human);
                    window.status.set_label(
                        &localizer
                            .format(
                                "wizard-progress-status-downloading-with-bytes",
                                &[
                                    ("package", package),
                                    ("downloaded", downloaded.as_str()),
                                    ("total", total.as_str()),
                                ],
                            )
                            .value,
                    );
                }
                ProgressEvent::DownloadCompleted { .. } => {
                    window.gauge.set_value(SELF_UPDATE_DOWNLOAD_SHARE);
                    append_self_update_progress_log(
                        window,
                        &localizer
                            .format(
                                "wizard-progress-log-download-completed",
                                &[("package", package)],
                            )
                            .value,
                    );
                }
                ProgressEvent::InstallStarted { .. } => {
                    window.status.set_label(
                        &localizer
                            .format("wizard-progress-status-installing", &[("package", package)])
                            .value,
                    );
                    append_self_update_progress_log(
                        window,
                        &localizer
                            .format(
                                "wizard-progress-log-install-started",
                                &[("package", package)],
                            )
                            .value,
                    );
                }
                ProgressEvent::InstallCompleted { .. } => {
                    window.gauge.set_value(100);
                    append_self_update_progress_log(
                        window,
                        &localizer
                            .format(
                                "wizard-progress-log-install-completed",
                                &[("package", package)],
                            )
                            .value,
                    );
                }
                // Configuration steps belong to package installs; RABBIT's
                // own update never emits them.
                ProgressEvent::ConfigurationStarted { .. }
                | ProgressEvent::ConfigurationCompleted { .. } => {}
            }
        });
    });
}

/// Append one line to the progress window's log, keeping the newest line in
/// view.
pub(crate) fn append_self_update_progress_log(window: &SelfUpdateProgressWindow, line: &str) {
    let existing = window.log.get_value();
    let separator = if existing.is_empty() { "" } else { "\n" };
    window
        .log
        .set_value(&format!("{existing}{separator}{line}"));
}

/// Tear the progress window down. Safe to call when none is up, and safe to
/// call twice — the second call finds the slot empty.
pub(crate) fn close_self_update_progress_window() {
    let window = SELF_UPDATE_PROGRESS.with(|cell| cell.borrow_mut().take());
    if let Some(window) = window {
        window.dialog.destroy();
    }
}
