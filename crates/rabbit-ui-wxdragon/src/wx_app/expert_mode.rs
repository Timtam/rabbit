//! Expert mode's wxWidgets side: the hidden unlock (Ctrl+Shift+E, or
//! RABBIT_EXPERT at launch), the build choices it adds to the target page,
//! and the run's channels and versions the version check and install share.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rabbit_core::package::PACKAGE_REAPER;
use rabbit_core::plan::AvailablePackage;
use wxdragon::prelude::*;

use crate::WizardModel;
use crate::wx_app::globals::{with_ui_frame, with_ui_localizer};
use crate::wx_app::widgets::{WizardWidgets, selected_target_row, set_optional_choice_shown};
use crate::wx_app::{DONE_STEP, PROGRESS_STEP, TARGET_STEP};

/// Expert mode for this session: the hidden unlock for pre-release builds.
/// Never saved - RABBIT keeps no settings file - so it starts from
/// `RABBIT_EXPERT` and is otherwise toggled with Ctrl+Shift+E (Cmd+Shift+E).
/// A process-wide flag rather than a thread-local because the self-update
/// relaunch runs on a worker thread and has to pass it on.
pub(crate) static EXPERT_MODE: AtomicBool = AtomicBool::new(false);

pub(crate) fn expert_mode() -> bool {
    EXPERT_MODE.load(Ordering::SeqCst)
}

thread_local! {
    /// The pull request behind each OSARA build choice; entry 0 is the
    /// regular snapshot. Rebuilt whenever the list is fetched.
    pub(crate) static OSARA_PULL_REQUESTS: RefCell<Vec<Option<u32>>> = RefCell::new(vec![None]);
    /// The pull request the selected target's OSARA was last installed from,
    /// to select once the list of builds arrives.
    pub(crate) static OSARA_REMEMBERED_PULL_REQUEST: Cell<Option<u32>> = const { Cell::new(None) };
    /// The OSARA build choice, so the background listing can fill it in on
    /// the UI thread.
    pub(crate) static OSARA_BUILD_CHOICE: Cell<Option<Choice>> = const { Cell::new(None) };
    /// The channels chosen when leaving the target page. The version check
    /// and the install both read this, so they always agree on the builds.
    pub(crate) static RUN_CHANNELS: RefCell<rabbit_core::package::PackageChannels> =
        const { RefCell::new(rabbit_core::package::PackageChannels::new()) };
    /// The versions the last version check found, on this run's channels.
    /// Re-planning after an install uses these rather than the versions
    /// fetched at launch, which knew nothing of the build choices.
    pub(crate) static RUN_AVAILABLE: RefCell<Option<Vec<AvailablePackage>>> = const { RefCell::new(None) };
}

pub(crate) const REAPER_BUILDS_LABEL_NAME: &str = "rabbit-reaper-builds-label";
pub(crate) const OSARA_BUILDS_LABEL_NAME: &str = "rabbit-osara-builds-label";
/// REAPER's development channel, as its manifest entry names it.
pub(crate) const REAPER_DEV_CHANNEL: &str = "dev";

/// The window title, with the expert-mode suffix while it is on. Reachable
/// at any time with the screen reader's "read title" command, which is how
/// the state stays discoverable after the confirmation dialog is gone.
pub(crate) fn window_title(model: &WizardModel) -> String {
    let mut title = model.window_title.clone();
    if expert_mode() {
        with_ui_localizer(|localizer| {
            title = localizer
                .format(
                    "wizard-window-title-expert",
                    &[("title", model.window_title.as_str())],
                )
                .value;
        });
    }
    title
}

pub(crate) fn apply_window_title(model: &WizardModel) {
    let title = window_title(model);
    with_ui_frame(|frame| frame.set_title(&title));
}

/// Ask before turning expert mode on. No is the default, so a stray Enter
/// never unlocks pre-release builds.
pub(crate) fn confirm_expert_mode(model: &WizardModel) -> bool {
    let no_default = MessageDialogStyle::from_bits_retain(wxdragon::ffi::WXD_NO_DEFAULT);
    let mut confirmed = false;
    with_ui_frame(|frame| {
        let dialog = MessageDialog::builder(
            frame,
            &model.text.expert_enable_body,
            &model.text.expert_enable_title,
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

/// A plain OK message. Every screen reader reads a native message box when
/// it opens, which a status-bar line would not guarantee.
pub(crate) fn show_expert_mode_message(title: &str, body: &str) {
    with_ui_frame(|frame| {
        let dialog = MessageDialog::builder(frame, body, title)
            .with_style(
                MessageDialogStyle::OK
                    | MessageDialogStyle::IconInformation
                    | MessageDialogStyle::Centre,
            )
            .build();
        dialog.show_modal();
    });
}

/// The channel REAPER's build choice asks for (`None` = regular releases).
pub(crate) fn reaper_build_channel(widgets: &WizardWidgets) -> Option<String> {
    (widgets.reaper_build_choice.get_selection() == Some(1)).then(|| REAPER_DEV_CHANNEL.to_string())
}

/// The channel OSARA's build choice asks for (`None` = regular snapshots).
pub(crate) fn osara_build_channel(widgets: &WizardWidgets) -> Option<String> {
    let index = widgets.osara_build_choice.get_selection()? as usize;
    OSARA_PULL_REQUESTS
        .with(|list| list.borrow().get(index).copied().flatten())
        .map(|number| format!("pr:{number}"))
}

/// Start the build choices from what `target_path` was last installed from,
/// so someone on development builds sees them selected rather than being
/// quietly switched back.
pub(crate) fn seed_build_choices(target_path: Option<&Path>, reaper_choice: &Choice) {
    let remembered = |package: &str| {
        target_path.and_then(|path| rabbit_core::package::remembered_channel(path, package))
    };
    let on_dev = remembered(PACKAGE_REAPER).is_some_and(|channel| channel == REAPER_DEV_CHANNEL);
    reaper_choice.set_selection(u32::from(on_dev));
    let pull_request = remembered(rabbit_core::package::PACKAGE_OSARA).and_then(|channel| {
        match rabbit_core::package::split_channel(&channel) {
            ("pr", Some(number)) => number.parse().ok(),
            _ => None,
        }
    });
    OSARA_REMEMBERED_PULL_REQUEST.set(pull_request);
    select_remembered_osara_build();
}

/// Select the pull request the target's OSARA was last installed from.
pub(crate) fn select_remembered_osara_build() {
    select_osara_build(OSARA_REMEMBERED_PULL_REQUEST.get());
}

/// The pull request OSARA's build choice has selected (`None` = snapshots).
pub(crate) fn selected_osara_build() -> Option<u32> {
    let choice = OSARA_BUILD_CHOICE.get()?;
    let index = choice.get_selection()? as usize;
    OSARA_PULL_REQUESTS.with(|list| list.borrow().get(index).copied().flatten())
}

/// Select pull request `wanted` (`None` = the regular snapshot) in OSARA's
/// build choice. A pull request missing from the list is added under its
/// number. The list only reaches back so far, and it can fail to load or not
/// have arrived yet. None of those may quietly take a tester off the build
/// they are on. If that build really is gone, resolution falls back to the
/// snapshot and the package row says so.
pub(crate) fn select_osara_build(wanted: Option<u32>) {
    let Some(choice) = OSARA_BUILD_CHOICE.get() else {
        return;
    };
    let listed =
        OSARA_PULL_REQUESTS.with(|list| list.borrow().iter().position(|number| *number == wanted));
    let index = match (listed, wanted) {
        (Some(index), _) => index,
        (None, None) => 0,
        (None, Some(number)) => {
            let number_text = number.to_string();
            let mut label = number_text.clone();
            with_ui_localizer(|localizer| {
                label = localizer
                    .format(
                        "wizard-expert-osara-builds-pr-untitled",
                        &[("number", number_text.as_str())],
                    )
                    .value;
            });
            OSARA_PULL_REQUESTS.with(|list| {
                let mut list = list.borrow_mut();
                let index = osara_build_insert_position(&list, number);
                choice.insert(&label, index);
                list.insert(index, Some(number));
                index
            })
        }
    };
    choice.set_selection(index as u32);
}

/// Where pull request `number` belongs in OSARA's build list: after the
/// regular snapshot (always first) and every higher pull request number, so
/// one added under its number keeps the list's highest-first order.
pub(crate) fn osara_build_insert_position(list: &[Option<u32>], number: u32) -> usize {
    list.iter()
        .position(|listed| listed.is_some_and(|listed| listed < number))
        .unwrap_or(list.len())
}

/// List OSARA's pull requests that have a test build, in the background.
/// Until the list arrives, the choice says it is looking, which still means
/// "regular snapshots", and it already offers the pull request the target is
/// on, so moving on early keeps that build.
pub(crate) fn start_osara_build_listing(model: &WizardModel, widgets: &WizardWidgets) {
    let choice = widgets.osara_build_choice;
    OSARA_BUILD_CHOICE.set(Some(choice));
    OSARA_PULL_REQUESTS.with(|list| *list.borrow_mut() = vec![None]);
    choice.clear();
    choice.append(&model.text.expert_osara_builds_loading);
    choice.set_selection(0);
    select_remembered_osara_build();
    let platform = model.platform;
    std::thread::spawn(move || {
        let result = rabbit_core::actions_artifact::pull_request_choices(
            rabbit_core::package::PACKAGE_OSARA,
            platform,
        )
        .map_err(|error| error.to_string());
        wxdragon::call_after(Box::new(move || fill_osara_build_choice(result)));
    });
}

pub(crate) fn fill_osara_build_choice(
    result: std::result::Result<Vec<rabbit_core::actions_artifact::PullRequestChoice>, String>,
) {
    let Some(choice) = OSARA_BUILD_CHOICE.get() else {
        return;
    };
    // Whatever is selected now stays selected. That is the remembered pull
    // request, or whatever the user has picked since. A second listing (expert
    // mode turned off and on again) must not undo their pick.
    let keep = selected_osara_build();
    with_ui_localizer(|localizer| {
        choice.clear();
        let mut numbers = vec![None];
        match &result {
            Ok(builds) => {
                choice.append(&localizer.text("wizard-expert-osara-builds-snapshot").value);
                for build in builds {
                    let number = build.number.to_string();
                    let label = match &build.title {
                        Some(title) => localizer.format(
                            "wizard-expert-osara-builds-pr",
                            &[("number", number.as_str()), ("title", title.as_str())],
                        ),
                        None => localizer.format(
                            "wizard-expert-osara-builds-pr-untitled",
                            &[("number", number.as_str())],
                        ),
                    };
                    choice.append(&label.value);
                    numbers.push(Some(build.number));
                }
            }
            Err(error) => {
                eprintln!("could not list OSARA pull request builds: {error}");
                choice.append(
                    &localizer
                        .text("wizard-expert-osara-builds-unavailable")
                        .value,
                );
            }
        }
        OSARA_PULL_REQUESTS.with(|list| *list.borrow_mut() = numbers);
    });
    select_osara_build(keep);
}

/// Show or hide everything expert mode adds, and bring it up to date.
pub(crate) fn apply_expert_mode(model: &WizardModel, widgets: &WizardWidgets) {
    let on = expert_mode();
    apply_window_title(model);
    set_optional_choice_shown(&widgets.reaper_build_choice, REAPER_BUILDS_LABEL_NAME, on);
    set_optional_choice_shown(&widgets.osara_build_choice, OSARA_BUILDS_LABEL_NAME, on);
    if on {
        let target = selected_target_row(model, widgets);
        seed_build_choices(
            target.as_ref().map(|target| target.path.as_path()),
            &widgets.reaper_build_choice,
        );
        start_osara_build_listing(model, widgets);
    }
}

/// Handle the expert-mode chord. It only switches on the target page, where
/// the build choices live: changing it later would leave a version check
/// that no longer matches the chosen builds. Anywhere else it explains
/// rather than doing nothing, because a blind user gets no other signal.
pub(crate) fn toggle_expert_mode(model: &WizardModel, widgets: &WizardWidgets, step: usize) {
    let text = &model.text;
    match step {
        TARGET_STEP => {}
        PROGRESS_STEP | DONE_STEP => {
            show_expert_mode_message(&text.expert_first_page_title, &text.expert_busy_body);
            return;
        }
        _ => {
            show_expert_mode_message(&text.expert_first_page_title, &text.expert_first_page_body);
            return;
        }
    }
    // Focus stays where the user pressed the keys: the dialog hands it back
    // when it closes. Only a build choice that is about to be hidden gives it
    // up, to the REAPER installation choice above it, before it disappears.
    if expert_mode() {
        if widgets.reaper_build_choice.has_focus() || widgets.osara_build_choice.has_focus() {
            widgets.target_choice.set_focus();
        }
        EXPERT_MODE.store(false, Ordering::SeqCst);
        apply_expert_mode(model, widgets);
        show_expert_mode_message(&text.expert_disabled_title, &text.expert_disabled_body);
    } else {
        if !confirm_expert_mode(model) {
            return;
        }
        EXPERT_MODE.store(true, Ordering::SeqCst);
        apply_expert_mode(model, widgets);
    }
}

/// Ctrl+Shift+E, or Cmd+Shift+E on macOS (`cmd_down` is Ctrl elsewhere).
/// Alt/Option must NOT be down: Ctrl+Alt is AltGr on German and many other
/// layouts, and Ctrl+Option is the VoiceOver modifier.
pub(crate) fn is_expert_mode_chord(event: &Event) -> bool {
    matches!(event.get_key_code(), Some(code) if code == 'E' as i32 || code == 'e' as i32)
        && event.cmd_down()
        && event.shift_down()
        && !event.alt_down()
}

/// Listen for the expert-mode chord on the whole window. wxEVT_CHAR_HOOK
/// reaches the top-level window before the focused control sees the key,
/// which is what lets one binding work wherever focus is.
pub(crate) fn bind_expert_mode_chord(
    frame: &Frame,
    model: &Arc<WizardModel>,
    widgets: WizardWidgets,
    current_step: &Arc<AtomicUsize>,
) {
    let model = Arc::clone(model);
    let current_step = Arc::clone(current_step);
    frame.bind_internal(EventType::CHAR_HOOK, move |event| {
        if is_expert_mode_chord(&event) {
            event.skip(false);
            toggle_expert_mode(&model, &widgets, current_step.load(Ordering::SeqCst));
        } else {
            // Every other key carries on to the focused control. A hook that
            // swallowed keys would break typing and arrowing everywhere.
            event.skip(true);
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_pull_request_added_later_keeps_the_list_highest_number_first() {
        let list = [None, Some(1454), Some(1448), Some(1404)];
        assert_eq!(super::osara_build_insert_position(&list, 1460), 1);
        assert_eq!(super::osara_build_insert_position(&list, 1450), 2);
        assert_eq!(super::osara_build_insert_position(&list, 1300), 4);
        // Only the snapshot so far (the list is still loading or failed).
        assert_eq!(super::osara_build_insert_position(&[None], 1454), 1);
    }
}
