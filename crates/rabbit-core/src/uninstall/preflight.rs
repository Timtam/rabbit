//! The checks before an uninstall changes anything.
//!
//! Install's preflight isn't enough. It looks only at the target's REAPER,
//! and a dry run there treats a running REAPER as allowed. Uninstall has no
//! way to allow it, so a running REAPER blocks the dry run too
//! and the dry run says what `--apply` would say. Some packages need every
//! REAPER closed, not only the target's: REAPER's own uninstaller reacts to
//! any REAPER window, and a machine-wide package such as app2clap may be
//! loaded by any REAPER on the computer.
//!
//! The engine runs the process checks again right before each package,
//! because a program can start in between.

use std::path::PathBuf;

use crate::model::Platform;
use crate::package::uninstall::UninstallStrategy;
use crate::preflight::{
    PreflightCheck, PreflightReport, PreflightStatus, REAPER_PROCESS_CHECK, RunningProcess,
    macos_userplugins_write_check, resource_path_check, running_processes_named,
    running_reaper_for_target_with_processes, running_reaper_processes,
};

use super::paths::inside_or_same;
use super::{UninstallPlan, UninstallPlanItem};

/// The check that fails while any REAPER at all runs, for packages that
/// need every REAPER closed.
pub const ANY_REAPER_PROCESS_CHECK: &str = "any-reaper-process";
/// The check that fails while a program a plan names is running.
pub const PROGRAMS_RUNNING_CHECK: &str = "programs-running";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallPreflightOptions {
    /// A dry run skips only the macOS write rehearsal, which renames files
    /// to find out whether it may.
    pub dry_run: bool,
    /// The target's REAPER, when the caller knows it better than detection.
    pub target_app_path: Option<PathBuf>,
}

/// Run the uninstall preflight for `package_ids` against the processes
/// running now.
pub fn run_uninstall_preflight(
    plan: &UninstallPlan,
    package_ids: &[String],
    options: &UninstallPreflightOptions,
) -> PreflightReport {
    let programs: Vec<String> = checked_items(plan, package_ids)
        .iter()
        .flat_map(|item| item.blocking_programs.iter().cloned())
        .collect();
    run_uninstall_preflight_with_processes(
        plan,
        package_ids,
        options,
        &running_reaper_processes(Platform::current()),
        &running_processes_named(&programs),
    )
}

/// [`run_uninstall_preflight`] with the running processes given:
/// `reaper_processes` are every running REAPER, and `other_processes` the
/// running programs the plans name.
pub fn run_uninstall_preflight_with_processes(
    plan: &UninstallPlan,
    package_ids: &[String],
    options: &UninstallPreflightOptions,
    reaper_processes: &[RunningProcess],
    other_processes: &[RunningProcess],
) -> PreflightReport {
    let items = checked_items(plan, package_ids);
    let mut checks = vec![resource_path_check(&plan.resource_path, options.dry_run)];

    let target_reapers = running_reaper_for_target_with_processes(
        &plan.resource_path,
        options.target_app_path.as_deref(),
        reaper_processes,
    );
    checks.push(process_check(
        REAPER_PROCESS_CHECK,
        &target_reapers,
        "No running REAPER process was detected.",
        "Close REAPER before uninstalling",
    ));

    let need_every_reaper_closed: Vec<&str> = items
        .iter()
        .filter(|item| item.every_reaper_closed)
        .map(|item| item.display_name.as_str())
        .collect();
    if !need_every_reaper_closed.is_empty() {
        checks.push(process_check(
            ANY_REAPER_PROCESS_CHECK,
            reaper_processes,
            "No REAPER is running anywhere on this computer.",
            &format!(
                "Close every REAPER on this computer, portable ones included, before \
                 uninstalling {}",
                need_every_reaper_closed.join(", ")
            ),
        ));
    }

    let blocking: Vec<&str> = items
        .iter()
        .flat_map(|item| item.blocking_programs.iter().map(String::as_str))
        .collect();
    if !blocking.is_empty() {
        let running: Vec<RunningProcess> = other_processes
            .iter()
            .filter(|process| {
                blocking
                    .iter()
                    .any(|program| program.eq_ignore_ascii_case(&process.name))
            })
            .cloned()
            .collect();
        checks.push(process_check(
            PROGRAMS_RUNNING_CHECK,
            &running,
            &format!(
                "None of these programs is running: {}.",
                blocking.join(", ")
            ),
            "Close these programs before uninstalling",
        ));
    }

    // macOS can refuse to move files out of UserPlugins for reasons that
    // have nothing to do with REAPER (Full Disk Access, App Management).
    // Install rehearses that before it changes anything; so does uninstall.
    let moves_resource_files = items.iter().any(|item| {
        item.strategy == Some(UninstallStrategy::RemoveFiles)
            && item
                .files
                .iter()
                .any(|file| inside_or_same(&file.path, &plan.resource_path))
    });
    if !options.dry_run && moves_resource_files && Platform::current() == Some(Platform::MacOs) {
        checks.push(macos_userplugins_write_check(&plan.resource_path));
    }

    let passed = checks
        .iter()
        .all(|check| check.status != PreflightStatus::Fail);
    PreflightReport { passed, checks }
}

/// The requested packages that would actually be removed. The others change
/// nothing, so nothing has to be closed for them.
fn checked_items<'a>(
    plan: &'a UninstallPlan,
    package_ids: &[String],
) -> Vec<&'a UninstallPlanItem> {
    plan.items
        .iter()
        .filter(|item| package_ids.contains(&item.package_id) && item.removability.is_removable())
        .collect()
}

fn process_check(
    name: &str,
    running: &[RunningProcess],
    pass_message: &str,
    fail_message: &str,
) -> PreflightCheck {
    if running.is_empty() {
        return PreflightCheck {
            name: name.to_string(),
            status: PreflightStatus::Pass,
            message: pass_message.to_string(),
        };
    }
    let list = running
        .iter()
        .map(|process| format!("{} ({})", process.name, process.pid))
        .collect::<Vec<_>>()
        .join(", ");
    PreflightCheck {
        name: name.to_string(),
        status: PreflightStatus::Fail,
        message: format!("{fail_message}: {list}."),
    }
}
