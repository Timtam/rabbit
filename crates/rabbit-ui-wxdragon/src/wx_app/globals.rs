//! Process-wide handles the event callbacks need: the localizer, the top
//! frame, the post-install hook, and the version-check dispatcher.

use std::cell::RefCell;
use std::rc::Rc;

use rabbit_core::localization::Localizer;

use wxdragon::prelude::*;

// FluentBundle is !Send, so we keep one Localizer instance per UI thread and
// have call_after bodies read it from this thread-local rather than capturing
// it through worker threads. The wxdragon event loop runs every call_after
// body on the same thread that initialised UI_LOCALIZER (the main thread).
thread_local! {
    static UI_LOCALIZER: RefCell<Option<Rc<Localizer>>> = const { RefCell::new(None) };
    /// Top-level wizard frame, stashed here so transient modal dialogs
    /// (e.g., the once-per-session "RABBIT update available" prompt) can
    /// parent themselves on the wizard window without `Frame` having to
    /// ride inside `Send`-requiring `call_after` closures or the
    /// `WizardWidgets` struct (which is captured by those closures).
    /// `Frame` doesn't impl `Send` because its underlying pointer is
    /// `*mut`; in practice we only ever access it on the UI thread, but
    /// the type system won't accept that as a static promise.
    static UI_FRAME: RefCell<Option<Frame>> = const { RefCell::new(None) };
    /// Post-install rescan hook. The install click handler arms this with a
    /// closure that captures the UI-thread `Rc<RefCell>` shared state for
    /// `package_rows`/`package_notes`/`can_install`. The wizard install
    /// runs on a worker thread; its `call_after` success branch fires the
    /// hook so the cached package state reflects what the just-completed
    /// install left on disk. Lives in a thread-local because the
    /// `Rc<RefCell>` it captures is `!Send` and can't ride inside the
    /// `call_after` `Box<dyn FnOnce + Send>`.
    static POST_INSTALL_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

pub(crate) fn install_ui_localizer(localizer: Localizer) {
    UI_LOCALIZER.with(|cell| {
        *cell.borrow_mut() = Some(Rc::new(localizer));
    });
}

pub(crate) fn with_ui_localizer<F: FnOnce(&Localizer)>(f: F) {
    UI_LOCALIZER.with(|cell| {
        if let Some(localizer) = cell.borrow().as_ref() {
            f(localizer);
        }
    });
}

pub(crate) fn install_ui_frame(frame: Frame) {
    UI_FRAME.with(|cell| {
        *cell.borrow_mut() = Some(frame);
    });
}

pub(crate) fn with_ui_frame<F: FnOnce(&Frame)>(f: F) {
    UI_FRAME.with(|cell| {
        if let Some(frame) = cell.borrow().as_ref() {
            f(frame);
        }
    });
}

pub(crate) fn arm_post_install_hook(callback: impl FnOnce() + 'static) {
    POST_INSTALL_HOOK.with(|cell| {
        *cell.borrow_mut() = Some(Box::new(callback));
    });
}

pub(crate) fn fire_post_install_hook() {
    let callback = POST_INSTALL_HOOK.with(|cell| cell.borrow_mut().take());
    if let Some(callback) = callback {
        callback();
    }
}

/// Stages of the deferred latest-version fetch the wizard runs once the user
/// transitions Target → Packages.
pub(crate) enum VersionCheckEvent {
    /// Per-package outcome: a fetched version plus the package's optional
    /// What's-New notes, or an error message.
    Result {
        package_id: String,
        outcome: std::result::Result<(String, Option<String>), String>,
    },
    /// Worker has finished iterating all packages — the UI should rebuild the
    /// package list with the fetched data and re-enable interaction.
    Finished,
}

/// Dispatcher set up by the Target → Packages click handler so the
/// version-check worker's `call_after` posts can mutate UI-thread-only state
/// (Rc-based package_rows, package_notes, can_install) without violating Send.
pub(crate) type VersionCheckDispatcher = Box<dyn FnMut(VersionCheckEvent)>;

thread_local! {
    static VERSION_CHECK_DISPATCHER: RefCell<Option<VersionCheckDispatcher>> =
        const { RefCell::new(None) };
}

pub(crate) fn install_version_check_dispatcher(dispatcher: VersionCheckDispatcher) {
    VERSION_CHECK_DISPATCHER.with(|cell| {
        *cell.borrow_mut() = Some(dispatcher);
    });
}

pub(crate) fn dispatch_version_check_event(event: VersionCheckEvent) {
    VERSION_CHECK_DISPATCHER.with(|cell| {
        if let Some(dispatcher) = cell.borrow_mut().as_mut() {
            dispatcher(event);
        }
    });
}
