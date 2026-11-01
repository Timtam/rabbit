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
//!
//! One check is about room rather than programs. Removed files move into a
//! backup set in the resource folder. A file on another volume can't be
//! renamed there; it is copied, checked and deleted, and the copy needs
//! room first.

use std::fs;
use std::path::{Path, PathBuf};

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
/// The check that fails when the backup's volume lacks room for the files
/// that have to be copied there from another volume.
pub const BACKUP_SPACE_CHECK: &str = "backup-space";
/// Room the backup's volume must keep beyond the copied files themselves:
/// enough for the set's manifest and notes, and so that a removal never
/// fills a drive to its last byte.
pub const BACKUP_SPACE_MARGIN: u64 = 64 * 1024 * 1024;

/// What the free-space check asks about volumes. The real one asks the
/// operating system; tests stand in their own, since one test machine
/// rarely has two volumes to move between.
pub(crate) trait VolumeProbe {
    /// Whether two existing paths are on the same volume; `None` when that
    /// can't be told.
    fn same_volume(&self, left: &Path, right: &Path) -> Option<bool>;
    /// The bytes this user may still write on the volume holding `path`.
    fn available_space(&self, path: &Path) -> Option<u64>;
}

/// The volumes of this computer.
pub(crate) struct SystemVolumes;

impl VolumeProbe for SystemVolumes {
    fn same_volume(&self, left: &Path, right: &Path) -> Option<bool> {
        rabbit_platform::same_volume(left, right)
    }

    fn available_space(&self, path: &Path) -> Option<u64> {
        rabbit_platform::available_space(path)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallPreflightOptions {
    /// A dry run skips only the macOS write rehearsal, which renames files
    /// to find out whether it may.
    pub dry_run: bool,
    /// The target's REAPER, when the caller knows it better than detection.
    pub target_app_path: Option<PathBuf>,
}

/// Run the uninstall preflight for `package_ids` against the processes
/// running now and this computer's volumes.
pub fn run_uninstall_preflight(
    plan: &UninstallPlan,
    package_ids: &[String],
    options: &UninstallPreflightOptions,
) -> PreflightReport {
    run_uninstall_preflight_with(
        plan,
        package_ids,
        options,
        &running_reaper_processes(Platform::current()),
        &running_processes_named(&blocking_programs(plan, package_ids)),
        &SystemVolumes,
    )
}

/// [`run_uninstall_preflight`] with the running processes given:
/// `reaper_processes` are every running REAPER, and `other_processes` the
/// running programs the plans name. It leaves out the free-space check,
/// which asks about this computer's volumes.
pub fn run_uninstall_preflight_with_processes(
    plan: &UninstallPlan,
    package_ids: &[String],
    options: &UninstallPreflightOptions,
    reaper_processes: &[RunningProcess],
    other_processes: &[RunningProcess],
) -> PreflightReport {
    let items = checked_items(plan, package_ids);
    let mut checks = vec![resource_path_check(&plan.resource_path, options.dry_run)];
    checks.extend(process_checks(
        plan,
        package_ids,
        options.target_app_path.as_deref(),
        reaper_processes,
        other_processes,
    ));

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

    preflight_report(checks)
}

/// The whole preflight, with the processes and the volumes given.
pub(crate) fn run_uninstall_preflight_with(
    plan: &UninstallPlan,
    package_ids: &[String],
    options: &UninstallPreflightOptions,
    reaper_processes: &[RunningProcess],
    other_processes: &[RunningProcess],
    volumes: &dyn VolumeProbe,
) -> PreflightReport {
    let mut checks = run_uninstall_preflight_with_processes(
        plan,
        package_ids,
        options,
        reaper_processes,
        other_processes,
    )
    .checks;
    if let Some((check, _)) = backup_space_check(plan, package_ids, volumes) {
        checks.push(check);
    }
    preflight_report(checks)
}

fn preflight_report(checks: Vec<PreflightCheck>) -> PreflightReport {
    let passed = checks
        .iter()
        .all(|check| check.status != PreflightStatus::Fail);
    PreflightReport { passed, checks }
}

/// The programs the plans of `package_ids` say must be closed.
pub(crate) fn blocking_programs(plan: &UninstallPlan, package_ids: &[String]) -> Vec<String> {
    checked_items(plan, package_ids)
        .iter()
        .flat_map(|item| item.blocking_programs.iter().cloned())
        .collect()
}

/// Only the process rules for `package_ids`: the target's REAPER, every
/// REAPER where a package needs that, and the programs the plans name. The
/// engine runs these again right before each package.
pub(crate) fn process_checks(
    plan: &UninstallPlan,
    package_ids: &[String],
    target_app_path: Option<&Path>,
    reaper_processes: &[RunningProcess],
    other_processes: &[RunningProcess],
) -> Vec<PreflightCheck> {
    let items = checked_items(plan, package_ids);
    let mut checks = Vec::new();
    let target_reapers = running_reaper_for_target_with_processes(
        &plan.resource_path,
        target_app_path,
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
    checks
}

/// Where the backup set's files will be: the nearest folder that exists on
/// the way to `RABBIT/backups`, normally the resource folder itself.
pub(crate) fn backup_volume_folder(resource_path: &Path) -> PathBuf {
    let backups = resource_path
        .join(crate::package::uninstall::RABBIT_FOLDER)
        .join("backups");
    backups
        .ancestors()
        .find(|folder| folder.is_dir())
        .map_or_else(|| resource_path.to_path_buf(), Path::to_path_buf)
}

/// Whether moving `path` into the backup set means a copy to another
/// volume. A volume that can't be compared counts as another one: a checked
/// copy is always safe, only slower.
pub(crate) fn crosses_volumes(
    path: &Path,
    backup_folder: &Path,
    volumes: &dyn VolumeProbe,
) -> bool {
    let folder = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    volumes.same_volume(folder, backup_folder) != Some(true)
}

/// The free-space check, when any file of `package_ids` has to be copied
/// to the backup's volume from another one, with the packages that need
/// it. `None` when everything moves by rename.
pub(crate) fn backup_space_check(
    plan: &UninstallPlan,
    package_ids: &[String],
    volumes: &dyn VolumeProbe,
) -> Option<(PreflightCheck, Vec<String>)> {
    let backup_folder = backup_volume_folder(&plan.resource_path);
    let mut needed: u64 = 0;
    let mut packages = Vec::new();
    for item in checked_items(plan, package_ids) {
        let crossing: Vec<u64> = item
            .files
            .iter()
            .filter(|file| crosses_volumes(&file.path, &backup_folder, volumes))
            .map(|file| file.size.unwrap_or_else(|| size_on_disk(&file.path)))
            .collect();
        if !crossing.is_empty() {
            needed = crossing.into_iter().fold(needed, u64::saturating_add);
            packages.push(item.package_id.clone());
        }
    }
    if packages.is_empty() {
        return None;
    }
    let needed_with_margin = needed.saturating_add(BACKUP_SPACE_MARGIN);
    let check = match volumes.available_space(&backup_folder) {
        Some(available) if available < needed_with_margin => PreflightCheck {
            name: BACKUP_SPACE_CHECK.to_string(),
            status: PreflightStatus::Fail,
            message: format!(
                "Not enough free space for the backup: {} of files on another drive must be \
                 copied to the drive holding {}, which needs {} free and has {}.",
                format_bytes(needed),
                backup_folder.display(),
                format_bytes(needed_with_margin),
                format_bytes(available)
            ),
        },
        Some(available) => PreflightCheck {
            name: BACKUP_SPACE_CHECK.to_string(),
            status: PreflightStatus::Pass,
            message: format!(
                "The drive holding {} has room for the {} of files copied there from another \
                 drive ({} free).",
                backup_folder.display(),
                format_bytes(needed),
                format_bytes(available)
            ),
        },
        // A copy that runs out of room fails and leaves the original where
        // it was, so not knowing is a warning, not a blocker.
        None => PreflightCheck {
            name: BACKUP_SPACE_CHECK.to_string(),
            status: PreflightStatus::Warn,
            message: format!(
                "RABBIT couldn't tell how much space is free on the drive holding {}; {} of \
                 files on another drive must be copied there.",
                backup_folder.display(),
                format_bytes(needed)
            ),
        },
    };
    Some((check, packages))
}

/// The size of a file, or of everything in a folder, without following
/// links. A folder RABBIT can't read counts as empty here; moving it fails
/// cleanly later.
fn size_on_disk(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| size_on_disk(&entry.path()))
                .fold(0_u64, u64::saturating_add)
        })
        .unwrap_or(0)
}

/// Bytes the way the CLI prints them, in binary units.
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["bytes", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} bytes")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
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
