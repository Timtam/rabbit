//! Decides what closing the window means while an install is running, and
//! asks the user when the answer is "stop the install".

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::WizardModel;
use rabbit_core::cancel::CancelToken;

use wxdragon::prelude::*;

use crate::wx_app::globals::with_ui_frame;
use crate::wx_app::self_update_ui::SELF_UPDATE_APPLYING;
use crate::wx_app::widgets::WizardWidgets;

/// The install worker's half of the close handshake.
///
/// Closing the window used to end the process wherever the worker happened
/// to be: mid-download, mid-extract, or between a vendor installer finishing
/// and RABBIT writing the receipt that records it. Nothing was cleaned up,
/// because nothing unwound — no `Drop` ran, so the install lock file, the
/// extraction temp dirs and any `.part` downloads stayed behind, and the
/// user got no report of what had actually landed.
///
/// Now the close handler talks to the worker instead. It flips `cancel`,
/// leaves `close_when_done` set, and vetoes the close; the pipeline stops at
/// its next package boundary, unwinds normally, writes its report, and the
/// completion closure closes the window for real.
#[derive(Default)]
pub(crate) struct InstallRunState {
    /// The running install's token. `None` before the first install and
    /// after each one finishes.
    pub(crate) cancel: Mutex<Option<CancelToken>>,
    /// True between the Install click and the worker's completion closure.
    pub(crate) running: AtomicBool,
    /// Set when the user asked to close during an install: the completion
    /// closure closes the window once the worker is done.
    pub(crate) close_when_done: AtomicBool,
}

impl InstallRunState {
    pub(crate) fn begin(&self) -> CancelToken {
        let token = CancelToken::new();
        if let Ok(mut slot) = self.cancel.lock() {
            *slot = Some(token.clone());
        }
        self.running.store(true, Ordering::SeqCst);
        token
    }

    pub(crate) fn finish(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.close_when_done.store(false, Ordering::SeqCst);
        if let Ok(mut slot) = self.cancel.lock() {
            *slot = None;
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Ask the pipeline to stop, and remember that the window should close
    /// once it has. Returns `false` when there was nothing left to stop —
    /// the run finished while the confirmation dialog was on screen, whose
    /// nested event loop is what lets the worker's completion closure run
    /// mid-question. Both halves happen under the one lock `finish` takes,
    /// so the answer can't go stale between them.
    pub(crate) fn request_stop_and_close(&self) -> bool {
        let Ok(slot) = self.cancel.lock() else {
            return false;
        };
        match slot.as_ref() {
            Some(token) => {
                self.close_when_done.store(true, Ordering::SeqCst);
                token.cancel();
                true
            }
            None => false,
        }
    }

    pub(crate) fn stop_requested(&self) -> bool {
        self.close_when_done.load(Ordering::SeqCst)
    }
}

/// What closing the window should do right now.
pub(crate) enum CloseVerdict {
    /// Nothing is running: destroy the window.
    Allow,
    /// An install is running and the user has not been asked yet.
    Ask,
    /// An install is running and the user already said "stop": the worker
    /// is unwinding and will close the window when it is done.
    AlreadyStopping,
    /// RABBIT is replacing its own files. There is no safe moment to quit.
    SelfUpdateInProgress,
}

/// Say that RABBIT is stopping, in both places the progress page speaks
/// from: the status heading and the running log.
///
/// The log is a `TextCtrl` and takes focus; the status heading is a
/// `StaticText` and cannot, and a label change on its own is not something
/// a screen reader announces. Moving focus onto the log is what makes the
/// answer to "did it hear me?" audible.
pub(crate) fn announce_stopping(widgets: &WizardWidgets, model: &WizardModel) {
    widgets
        .progress_status
        .set_label(&model.text.progress_status_cancelling);
    widgets
        .progress_details
        .append_text(&format!("\n{}", model.text.progress_status_cancelling));
    widgets.progress_details.set_focus();
}

pub(crate) fn close_verdict(install_run: &InstallRunState) -> CloseVerdict {
    if SELF_UPDATE_APPLYING.load(Ordering::SeqCst) {
        return CloseVerdict::SelfUpdateInProgress;
    }
    if !install_run.is_running() {
        return CloseVerdict::Allow;
    }
    if install_run.stop_requested() {
        return CloseVerdict::AlreadyStopping;
    }
    CloseVerdict::Ask
}

/// Ask before throwing away a running install. Returns `true` when the user
/// confirmed.
///
/// `wxNO_DEFAULT` puts the focused button on **No**: this dialog appears
/// over work the user asked for, so a stray Enter or Space — from a screen
/// reader user tabbing around the progress page, say — must not be what
/// stops their REAPER install halfway. wxdragon's `MessageDialogStyle`
/// doesn't name the flag, so it comes from the raw wx constant.
pub(crate) fn confirm_stop_install(model: &WizardModel) -> bool {
    let no_default = MessageDialogStyle::from_bits_retain(wxdragon::ffi::WXD_NO_DEFAULT);
    let mut confirmed = false;
    with_ui_frame(|frame| {
        let dialog = MessageDialog::builder(
            frame,
            &model.text.close_during_install_body,
            &model.text.close_during_install_title,
        )
        .with_style(
            MessageDialogStyle::YesNo
                | MessageDialogStyle::IconWarning
                | MessageDialogStyle::Centre
                | no_default,
        )
        .build();
        confirmed = dialog.show_modal() == ID_YES;
    });
    confirmed
}

/// Tell the user why the window will not close during a self-update. There
/// is no Yes/No here because there is no "yes": the file swap has no unwind
/// path, so the only honest answer is "wait".
pub(crate) fn show_close_blocked_by_self_update(model: &WizardModel) {
    with_ui_frame(|frame| {
        let dialog = MessageDialog::builder(
            frame,
            &model.text.close_during_self_update_body,
            &model.text.close_during_self_update_title,
        )
        .with_style(
            MessageDialogStyle::OK
                | MessageDialogStyle::IconInformation
                | MessageDialogStyle::Centre,
        )
        .build();
        dialog.show_modal();
    });
}
