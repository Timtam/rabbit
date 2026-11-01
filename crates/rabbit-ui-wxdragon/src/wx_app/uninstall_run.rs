//! What the Uninstall button on Review does: ask once more when the run
//! can't be undone from a backup or a package's manifest asks for it, then
//! start the run.
//!
//! Running the uninstall from the wizard isn't wired up yet. Until it is,
//! the button says so, nothing changes, and the message names the
//! `rabbit uninstall` command that removes what was ticked.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use wxdragon::prelude::*;

use crate::{
    WizardModel, WizardUninstallList, uninstall_confirmation_body, uninstall_not_ready_body,
};

use crate::wx_app::globals::{show_information, with_ui_frame, with_ui_localizer};
use crate::wx_app::widgets::{WizardWidgets, selected_target_row};

/// The Uninstall button on Review. Enter never gets here: Review has no
/// default button in uninstall mode, and Enter in its text starts nothing.
pub(crate) fn start_uninstall(
    model: &WizardModel,
    widgets: &WizardWidgets,
    list: &Rc<RefCell<WizardUninstallList>>,
) {
    // Take what the dialogs need and let the list go first: a modal dialog
    // runs the event loop, and its handlers may read the list.
    let (package_ids, settings_available, question) = {
        let list = list.borrow();
        let package_ids = list.ticked_package_ids();
        let mut question = None;
        if let Some(plan) = &list.plan {
            with_ui_localizer(|localizer| {
                question = uninstall_confirmation_body(localizer, plan, &package_ids);
            });
        }
        (package_ids, list.settings_option_available(), question)
    };
    // The button is disabled with nothing ticked; a stale press changes
    // nothing either.
    if package_ids.is_empty() {
        return;
    }
    if let Some(body) = question
        && !confirm_uninstall(model, &body)
    {
        // Back where the user decided, rather than on the button that
        // asked: the summary says what would have happened.
        widgets.review_text.set_focus();
        return;
    }
    let remove_settings = settings_available && widgets.uninstall_remove_settings.get_value();
    let resource_path = selected_target_row(model, widgets)
        .map(|target| target.path)
        .unwrap_or_default();
    show_uninstall_not_ready(model, &resource_path, &package_ids, remove_settings);
}

/// Ask before an uninstall a backup can't undo, or one a package's manifest
/// asks about, such as OSARA's. `true` when the user said Yes.
///
/// `wxNO_DEFAULT` puts focus on **No**, as when stopping an install: Enter
/// or Space pressed out of habit, or a screen reader user tabbing through
/// the dialog, must not be what removes REAPER for good.
fn confirm_uninstall(model: &WizardModel, body: &str) -> bool {
    let mut confirmed = false;
    with_ui_frame(|frame| {
        let dialog = MessageDialog::builder(frame, body, &model.text.confirm_uninstall_title)
            .with_style(
                MessageDialogStyle::YesNo
                    | MessageDialogStyle::IconWarning
                    | MessageDialogStyle::Centre
                    | MessageDialogStyle::NoDefault,
            )
            .build();
        confirmed = dialog.show_modal() == ID_YES;
    });
    confirmed
}

/// Say that this build can't uninstall from the wizard yet, and name the
/// command that removes what was ticked. Nothing is started, and focus goes
/// back to the Uninstall button when the message closes.
fn show_uninstall_not_ready(
    model: &WizardModel,
    resource_path: &Path,
    package_ids: &[String],
    remove_settings: bool,
) {
    let mut body = String::new();
    with_ui_localizer(|localizer| {
        body = uninstall_not_ready_body(localizer, resource_path, package_ids, remove_settings);
    });
    show_information(&model.text.uninstall_not_ready_title, &body);
}
