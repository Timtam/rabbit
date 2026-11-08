//! Carrying out an uninstall: the `remove_files` engine.
//!
//! [`execute_uninstall`] takes the install lock, builds the plan again from
//! fresh detection, and runs the uninstall preflight. Then it reads every
//! file it would remove, as the dry run does: the guard checks each path
//! and each file is hashed. If any requested package is blocked at that
//! point, a file that can't be read included, nothing changes at all.
//! Otherwise it removes the packages one at a time, every package before the
//! packages it depends on, each one like this:
//!
//! 1. Check the cancel token, and check again that nothing the package needs
//!    closed has started since.
//! 2. Record the package's receipt entry in the run's backup set.
//! 3. For each file: run the guard again and move it into the backup set,
//!    with the hash read before the run when the file can't have changed
//!    since. A file only the receipt names that changed since RABBIT
//!    installed it stays; one the manifest names goes all the same.
//! 4. Remove the folders the plan says to remove once empty.
//! 5. With a planned machine cleanup, have the elevated helper remove the
//!    uninstaller folder and registry entry the package's installer left in
//!    machine-wide places (see [`super::cleanup`]).
//! 6. Take the package out of the receipt and add it to `declined_packages`,
//!    so the next install run doesn't tick it again.
//! 7. Run the plan's post helpers, such as setting REAPER back to no
//!    language pack.
//! 8. Remove the plan's entries in Windows' list of installed programs, each
//!    only while it still points into this REAPER.
//!
//! When a step up to 6 fails, every file the package moved goes back where it
//! was and the receipt stays as it was: the receipt never claims files that
//! are gone, and never loses an entry for files that are still there. A
//! post helper or a registry entry that fails leaves the package removed,
//! with a problem in the report. Packages removed earlier in the run stay
//! removed; each can be restored from the backup set on its own.
//!
//! A machine cleanup runs only once every file moved, since a backup can't
//! bring back what it removes. Once it removed anything, the package counts
//! as removed: a receipt that can't be saved then is a problem to report,
//! and nothing goes back. A cleanup that fails part way puts the files back
//! and the run goes on; one the helper refuses, or loses contact during,
//! puts them back and stops the run.
//!
//! A package whose vendor's uninstaller removes it (Surge XT, a standard
//! REAPER) needs administrator rights, and so does a planned machine
//! cleanup (the JAWS scripts). Once every check has passed and before the
//! first change, the run starts one elevated helper for all of them: the
//! run's only Windows permission prompt. Declined, the whole run
//! is cancelled with nothing changed; if the helper can't start, nothing
//! changes either. When the run reaches such a package, the helper runs its
//! uninstaller, and the package's status follows what the uninstaller did
//! and what fresh detection finds afterwards; so does its receipt entry.
//! An uninstaller still open at its time limit, a helper that stops
//! answering, or a resource folder that lost REAPER's settings or RABBIT's
//! receipt stops the run there. The helper is told to quit at the end of
//! every run, however it ends.
//!
//! A macOS plan's trees, such as REAPER.app or Surge XT's bundles, leave
//! whole: through the Trash or the run's administrator command, never into a
//! backup set. The read before the run, and a dry run, check each one with
//! the guard and see that it is there, and never list, hash or walk it.
//!
//! On macOS the run's one administrator command is its first change, once
//! every check has passed: one password window, then root deletes what the
//! packages that need it remove, and moves a REAPER.app only root can move
//! into the user's Trash (see [`super::admin_batch`]). Cancelled, it
//! cancels the whole run with nothing changed; if any of its packages isn't
//! fully removed afterwards, the run stops there, before anything else
//! changes. Its packages' receipt entries follow fresh detection. A
//! REAPER.app the user may move goes to the Trash as the user, last, with
//! no password; it can't come back from a backup set, only from the Trash.
//!
//! A real macOS uninstall refuses to start at all while RABBIT runs as root,
//! as under `sudo`: the checks of what the user may do, the Trash and the
//! backup set would all be root's.

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rabbit_platform::HkcuUninstallRoot;

use crate::cancel::CancelToken;
use crate::detection::{default_standard_installation, detect_components};
use crate::error::{RabbitError, Result};
use crate::lock::{acquire_package_install_lock, default_package_install_lock_path};
use crate::model::{ComponentDetection, InstallationKind, Platform};
use crate::package::uninstall::{UninstallBackup, UninstallPostStep};
use crate::package::{PACKAGE_REAPER, PackageSpec, builtin_package_specs};
use crate::preflight::{
    PreflightCheck, PreflightStatus, REAPER_PROCESS_CHECK, RunningProcess,
    ensure_resource_path_ready, portable_target_app_path, running_processes_named,
    running_reaper_processes,
};
use crate::progress::{ProgressEvent, ProgressReporter, RemovalMethod};
use crate::receipt::{
    InstallState, RECEIPT_RELATIVE_PATH, load_install_state, remove_package_receipt,
    save_install_state,
};
use crate::rollback::{BackupPackageRecord, BackupSetWriter, FileDigest, MoveFileError, MovedFile};

use super::admin_batch::{
    AdministratorShell, AdministratorStepRecord, dry_run_administrator_steps,
    run_administrator_batch,
};
use super::cleanup::{
    CleanupOutcome, CleanupResult, CleanupStatus, CleanupTarget, PlannedMachineCleanup,
};
use super::helper::{ElevatedHelper, ElevatedRunRequest, ElevatedSession, ElevationStartError};
use super::macos_objects::{MacosObjects, ObjectProbe, TreeRoute, tree_route};
use super::paths::{UninstallRoots, absolute, inside_or_same, same_ignoring_case};
use super::plan::{
    UninstallPlanInput, build_uninstall_plan, installed_by_reapack, reapack_counts_as_installed,
};
use super::post::reset_reaper_langpack_if_selected;
use super::preflight::{
    ANY_REAPER_PROCESS_CHECK, BACKUP_SPACE_CHECK, PROGRAMS_RUNNING_CHECK, SystemVolumes,
    UninstallPreflightOptions, VolumeProbe, backup_space_check, backup_volume_folder,
    blocking_programs, crosses_volumes, process_checks, run_uninstall_preflight_with,
};
use super::registry::clean_registry_entries;
use super::vendor::{
    VendorEvent, VendorLookup, VendorOutcome, VendorProblem, VendorRefusal, VendorStatus,
};
use super::{
    KeptFile, KeptFileReason, RegistryEntryAction, Removability, RemovalReason, RemovalRoute,
    RemovedFile, UninstallMessage, UninstallPlan, UninstallPlanItem, UninstallProblem,
    UninstallReport, UninstallReportItem, UninstallRequest, UninstallServices, UninstallStatus,
    save_uninstall_report,
};

/// The reason an uninstall's backup set records.
pub const UNINSTALL_BACKUP_REASON: &str = "uninstall";

/// How much older than the read before the run a file's modification time
/// must be for that read's hash to be used again when the file moves.
/// File systems store the time in steps, two seconds on FAT, so a file
/// written again within one step of being read could keep its old time and
/// size. A file last changed well before the read can't hide a change that
/// way. FFmpeg's 250 MB, written when it was installed, always qualifies; a
/// file written moments ago is simply read again.
const DIGEST_REUSE_MARGIN: Duration = Duration::from_secs(5);

/// Uninstall `request.package_ids` from the REAPER at
/// `request.resource_path`, or with `request.dry_run` only say what would
/// happen. Runs on a worker thread: it hashes every file it removes, and it
/// waits for vendors' uninstallers. `services` says where vendor
/// uninstallers are looked up and what starts the elevated helper:
/// [`UninstallServices::system`] in RABBIT itself, stand-ins in tests.
///
/// Returns `Err` only for a hard error before anything changed, such as
/// another RABBIT holding the install lock, a receipt that can't be read,
/// or a real macOS run while RABBIT runs as root. Everything after that,
/// failures included, is in the report. A run that isn't a dry run saves
/// its report under `RABBIT/logs`.
pub fn execute_uninstall(
    request: &UninstallRequest,
    services: &UninstallServices<'_>,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<UninstallReport> {
    let (target, target_app_path) = target_of(
        request.platform,
        &request.resource_path,
        request.target_app_path.as_deref(),
        request.target,
    );
    let roots = UninstallRoots::for_target(
        request.platform,
        &request.resource_path,
        target_app_path.as_deref(),
    );
    let specs = builtin_package_specs(request.platform);
    let detect = || detect_components(&roots.resource, request.platform);
    let reaper_processes = || running_reaper_processes(Platform::current());
    let named_processes = |names: &[String]| running_processes_named(names);
    execute_uninstall_in(
        request,
        &UninstallEnvironment {
            specs: &specs,
            roots: &roots,
            target,
            target_app_path,
            detect: &detect,
            reaper_processes: &reaper_processes,
            named_processes: &named_processes,
            volumes: &SystemVolumes,
            installed_programs: services.installed_programs,
            vendor_lookup: services.vendor_lookup,
            elevated_helper: services.elevated_helper,
            macos_objects: services.macos_objects,
            administrator_shell: services.administrator_shell,
        },
        progress,
        cancel,
    )
}

/// Everything [`execute_uninstall`] reads from the computer, so tests can
/// stand in their own packages, detection, processes, volumes and registry,
/// and on macOS the Trash and the administrator command.
pub(crate) struct UninstallEnvironment<'a> {
    pub specs: &'a [PackageSpec],
    pub roots: &'a UninstallRoots,
    pub target: InstallationKind,
    pub target_app_path: Option<PathBuf>,
    /// Fresh detection, asked once before the run and once after it.
    pub detect: &'a dyn Fn() -> Result<Vec<ComponentDetection>>,
    /// Every running REAPER.
    pub reaper_processes: &'a dyn Fn() -> Vec<RunningProcess>,
    /// The running programs with one of these names.
    pub named_processes: &'a dyn Fn(&[String]) -> Vec<RunningProcess>,
    pub volumes: &'a dyn VolumeProbe,
    /// Windows' per-user list of installed programs, or a test's stand-in
    /// for it. `None` leaves every entry a plan names alone, unread.
    pub installed_programs: Option<&'a HkcuUninstallRoot>,
    /// Finds and checks each vendor's uninstaller while planning.
    pub vendor_lookup: &'a dyn VendorLookup,
    /// Starts the run's elevated helper.
    pub elevated_helper: &'a dyn ElevatedHelper,
    /// What this account may do with a macOS app, asked while planning and
    /// again right before it moves, and what the run does there as the
    /// user.
    pub macos_objects: &'a dyn MacosObjects,
    /// Runs the macOS administrator command.
    pub administrator_shell: &'a dyn AdministratorShell,
}

/// The uninstall plan for every package at `resource_path`, built from
/// `detections` and the receipt the way [`execute_uninstall`] builds it,
/// for a listing of what RABBIT could uninstall there. Reads which files
/// exist and how big they are, looks vendors' uninstallers up through
/// `vendor_lookup`, and asks `objects` who may move a macOS app; hashes
/// nothing and changes nothing.
pub fn plan_uninstall_at(
    resource_path: &Path,
    platform: Platform,
    detections: &[ComponentDetection],
    vendor_lookup: &dyn VendorLookup,
    objects: &dyn ObjectProbe,
) -> Result<UninstallPlan> {
    let (target, target_app_path) = target_of(platform, resource_path, None, None);
    let roots = UninstallRoots::for_target(platform, resource_path, target_app_path.as_deref());
    let state = load_install_state(resource_path)?;
    Ok(build_uninstall_plan(&UninstallPlanInput {
        roots: &roots,
        platform,
        target,
        specs: &builtin_package_specs(platform),
        detections,
        state: state.as_ref(),
        remove_settings: false,
        vendor_lookup,
        objects,
    }))
}

/// Whether `resource_path` belongs to the standard REAPER or a portable one,
/// unless `target` says, and where the target's REAPER app is, unless
/// `target_app_path` says.
fn target_of(
    platform: Platform,
    resource_path: &Path,
    target_app_path: Option<&Path>,
    target: Option<InstallationKind>,
) -> (InstallationKind, Option<PathBuf>) {
    let standard = default_standard_installation(platform).filter(|installation| {
        same_ignoring_case(
            &absolute(&installation.resource_path),
            &absolute(resource_path),
        )
    });
    let target = target.unwrap_or(if standard.is_some() {
        InstallationKind::Standard
    } else {
        InstallationKind::Portable
    });
    let app_path = target_app_path
        .map(Path::to_path_buf)
        .or_else(|| match target {
            InstallationKind::Standard => standard.map(|installation| installation.app_path),
            InstallationKind::Portable => portable_target_app_path(resource_path, Some(platform)),
        });
    (target, app_path)
}

pub(crate) fn execute_uninstall_in(
    request: &UninstallRequest,
    environment: &UninstallEnvironment<'_>,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<UninstallReport> {
    // Before anything else, the lock included, which would leave a file of
    // root's in the user's resource folder. A dry run changes nothing, so
    // it may still say what a run would do.
    if request.platform == Platform::MacOs
        && !request.dry_run
        && environment.macos_objects.runs_as_root()
    {
        return Err(RabbitError::UninstallAsRoot);
    }
    let resource = environment.roots.resource.clone();
    ensure_resource_path_ready(&resource, request.dry_run)?;
    // Held from before the plan is built until the report is written, so an
    // install can't change the files between the plan and the moves. A
    // real run takes it even where the resource folder doesn't exist yet,
    // which creates the folder: a standard REAPER that was never opened
    // still has its app outside the folder to remove, and the receipt (the
    // refusal) and the report are saved there afterwards. Folders made only
    // for the lock go again when the run changed nothing, so a mistyped
    // --resource-path leaves no trace. A dry run writes nothing.
    let lock_folders_made = if request.dry_run {
        Vec::new()
    } else {
        missing_folders(&default_package_install_lock_path(&resource))
    };
    let install_lock = if request.dry_run {
        None
    } else {
        Some(acquire_package_install_lock(&resource)?)
    };

    let detections = (environment.detect)()?;
    let reapack_installed = reapack_counts_as_installed(&detections);
    let state = load_install_state(&resource)?;
    let plan = build_uninstall_plan(&UninstallPlanInput {
        roots: environment.roots,
        platform: request.platform,
        target: environment.target,
        specs: environment.specs,
        detections: &detections,
        state: state.as_ref(),
        remove_settings: request.remove_settings,
        vendor_lookup: environment.vendor_lookup,
        objects: environment.macos_objects,
    });

    let mut requested: Vec<String> = Vec::new();
    for id in &request.package_ids {
        if !requested.contains(id) {
            requested.push(id.clone());
        }
    }
    let to_check: Vec<String> = requested
        .iter()
        .filter(|id| plan.item(id).is_some_and(UninstallPlanItem::can_remove))
        .cloned()
        .collect();

    let options = UninstallPreflightOptions {
        dry_run: request.dry_run,
        target_app_path: environment.target_app_path.clone(),
    };
    let reapers = (environment.reaper_processes)();
    let others = (environment.named_processes)(&blocking_programs(&plan, &to_check));
    let preflight = run_uninstall_preflight_with(
        &plan,
        &to_check,
        &options,
        &reapers,
        &others,
        environment.volumes,
    );
    let needs_space: Vec<String> = backup_space_check(&plan, &to_check, environment.volumes)
        .filter(|(check, _)| check.status == PreflightStatus::Fail)
        .map(|(_, packages)| packages)
        .unwrap_or_default();
    // Checks that concern the whole run, such as a resource folder that
    // can't be written. The process checks and the space check are asked
    // per package below.
    let run_failures: Vec<PreflightCheck> = preflight
        .checks
        .iter()
        .filter(|check| {
            check.status == PreflightStatus::Fail
                && ![
                    REAPER_PROCESS_CHECK,
                    ANY_REAPER_PROCESS_CHECK,
                    PROGRAMS_RUNNING_CHECK,
                    BACKUP_SPACE_CHECK,
                ]
                .contains(&check.name.as_str())
        })
        .cloned()
        .collect();

    // Every requested package gets its verdict before anything changes. The
    // ones still open are removed below, and the report keeps the order the
    // run takes them in throughout.
    let mut slots: Vec<(&UninstallPlanItem, Option<UninstallReportItem>)> = Vec::new();
    for plan_item in plan.in_run_order(&requested) {
        let id = plan_item.package_id.as_str();
        let verdict = if plan_item.removability == Removability::NotInstalled {
            Some((
                UninstallStatus::NotInstalled,
                UninstallMessage::NotInstalled,
            ))
        } else if !plan_item.removability.is_removable() {
            Some((
                UninstallStatus::Blocked,
                UninstallMessage::NotRemovable {
                    removability: plan_item.removability.clone(),
                },
            ))
        } else if !plan_item.can_remove() {
            Some((UninstallStatus::Blocked, UninstallMessage::NotYetSupported))
        } else {
            let mut checks = run_failures.clone();
            checks.extend(
                process_checks(
                    &plan,
                    &[id.to_string()],
                    environment.target_app_path.as_deref(),
                    &reapers,
                    &others,
                )
                .into_iter()
                .filter(|check| check.status == PreflightStatus::Fail),
            );
            if needs_space.iter().any(|package| package == id) {
                checks.extend(
                    preflight
                        .checks
                        .iter()
                        .filter(|check| check.name == BACKUP_SPACE_CHECK)
                        .cloned(),
                );
            }
            (!checks.is_empty()).then_some((
                UninstallStatus::Blocked,
                UninstallMessage::ChecksFailed { checks },
            ))
        };
        let verdict = verdict.map(|(status, code)| new_item(plan_item, status, code));
        slots.push((plan_item, verdict));
    }
    let unknown: Vec<UninstallReportItem> = requested
        .iter()
        .filter(|id| plan.item(id).is_none())
        .map(|id| {
            new_item_for(
                id,
                UninstallStatus::Blocked,
                UninstallMessage::UnknownPackage,
            )
        })
        .collect();
    let blocked: Vec<String> = slots
        .iter()
        .filter_map(|(_, item)| item.as_ref())
        .chain(&unknown)
        .filter(|item| item.status == UninstallStatus::Blocked)
        .map(|item| item.package_id.clone())
        .collect();
    let to_remove: Vec<&UninstallPlanItem> = slots
        .iter()
        .filter(|(_, item)| item.is_none())
        .map(|(plan_item, _)| *plan_item)
        .collect();

    let backup_folder = backup_volume_folder(&resource);
    let mut backup_set = None;
    let mut outcomes: Vec<UninstallReportItem> = Vec::new();
    let mut administrator_steps: Vec<AdministratorStepRecord> = Vec::new();
    if request.dry_run {
        for plan_item in &to_remove {
            let (mut item, _) = survey(plan_item, environment, &backup_folder, reapack_installed);
            if item.status == UninstallStatus::WouldRemove {
                clean_registry(&mut item, plan_item, environment, false);
            }
            outcomes.push(item);
        }
        // What root would be given, from the same checks; never run.
        administrator_steps = dry_run_administrator_steps(&to_remove, &mut outcomes, environment);
    } else if !blocked.is_empty() {
        for plan_item in to_remove {
            outcomes.push(new_item(
                plan_item,
                UninstallStatus::NotReached,
                UninstallMessage::OtherPackageBlocked {
                    package_ids: blocked.clone(),
                },
            ));
        }
    } else if !to_remove.is_empty() {
        match survey_before_removal(
            &to_remove,
            environment,
            &backup_folder,
            reapack_installed,
            cancel,
        ) {
            Err(items) => outcomes = items,
            Ok(digests) => {
                // One receipt for the whole run, from before its first
                // change: what the administrator command took out of it
                // stays out when a later package saves it.
                let mut state = state.unwrap_or_default();
                match run_administrator_batch(
                    &to_remove,
                    &plan,
                    environment,
                    &mut state,
                    progress,
                    cancel,
                    &mut administrator_steps,
                ) {
                    Err(items) => outcomes = items,
                    Ok(batch_items) => {
                        // The administrator command's packages are done and
                        // come first in the report. No platform has both
                        // that command and the elevated helper.
                        outcomes = batch_items;
                        let rest: Vec<&UninstallPlanItem> = to_remove
                            .iter()
                            .copied()
                            .filter(|item| !item.needs_administrator())
                            .collect();
                        match start_helper(&rest, environment, &resource, progress, cancel) {
                            Err(items) => outcomes.extend(items),
                            Ok(session) => {
                                let mut run = RemovalRun {
                                    plan: &plan,
                                    environment,
                                    request,
                                    backup_folder: &backup_folder,
                                    backups: BackupSetWriter::new(
                                        &resource,
                                        &uninstall_set_id(),
                                        UNINSTALL_BACKUP_REASON,
                                    ),
                                    state,
                                    digests,
                                    reapack_installed,
                                    progress,
                                    session,
                                };
                                outcomes.extend(run.remove_all(&rest, cancel));
                                run.session.finish();
                                if !run.backups.discard_if_empty() {
                                    backup_set = Some((
                                        run.backups.set_id(),
                                        run.backups.path().to_path_buf(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut outcomes = outcomes.into_iter();
    let mut items: Vec<UninstallReportItem> = slots
        .into_iter()
        .filter_map(|(_, item)| item.or_else(|| outcomes.next()))
        .collect();
    items.extend(unknown);

    if items.iter().any(|item| item.status.is_removed()) {
        report_leftovers(&mut items, environment);
    }
    let backup_set_id = backup_set.as_ref().map(|(id, _)| id.clone());
    for item in &mut items {
        if let UninstallMessage::Removed { backup_set_id: id } = &mut item.message_code {
            id.clone_from(&backup_set_id);
        }
        item.message = english_message(item, &plan);
    }

    let mut report = UninstallReport {
        resource_path: resource.clone(),
        platform: request.platform,
        target: environment.target,
        dry_run: request.dry_run,
        remove_settings: request.remove_settings,
        preflight,
        backup_set_id,
        backup_set_path: backup_set.map(|(_, path)| path),
        items,
        administrator_steps,
        plan,
        saved_report: None,
    };
    // A report of what was removed is the user's way back to the backup
    // set, so every real run leaves one, except one that changed nothing in
    // a folder only its lock made. A dry run writes nothing at all.
    let changed_nothing = report.items.iter().all(|item| {
        matches!(
            item.status,
            UninstallStatus::NotInstalled
                | UninstallStatus::Blocked
                | UninstallStatus::Cancelled
                | UninstallStatus::NotReached
        )
    });
    let leave_no_trace = !lock_folders_made.is_empty() && changed_nothing;
    if !request.dry_run && !leave_no_trace && resource.is_dir() {
        report.saved_report = save_uninstall_report(&report).ok();
    }
    if leave_no_trace {
        drop(install_lock);
        // Deepest first; remove_dir only ever removes an empty folder, so
        // anything another program put there in the meantime stays.
        for folder in &lock_folders_made {
            let _ = fs::remove_dir(folder);
        }
    }
    Ok(report)
}

/// The folders above `path` that don't exist yet, deepest first: what
/// creating `path`'s folder would make.
fn missing_folders(path: &Path) -> Vec<PathBuf> {
    path.ancestors()
        .skip(1)
        .take_while(|folder| !folder.as_os_str().is_empty() && !folder.exists())
        .map(Path::to_path_buf)
        .collect()
}

/// Start the run's elevated helper if a package needs it: the run's one
/// Windows permission prompt, raised once every check has passed and before
/// anything changes. Returns the report items of a run that can't go on: a
/// declined prompt cancels every package, and a helper that can't start
/// fails the packages that need it and leaves the rest unreached. Nothing
/// has changed in either case.
fn start_helper(
    to_remove: &[&UninstallPlanItem],
    environment: &UninstallEnvironment<'_>,
    resource: &Path,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> std::result::Result<HelperSession, Vec<UninstallReportItem>> {
    let elevated: Vec<String> = to_remove
        .iter()
        .filter(|item| item.needs_elevated_helper())
        .map(|item| item.package_id.clone())
        .collect();
    // A run stopped already ends at its first package; no prompt for it.
    if elevated.is_empty() || cancel.is_cancelled() {
        return Ok(HelperSession(None));
    }
    let request = ElevatedRunRequest {
        resource_path: resource.to_path_buf(),
        target_app_path: environment.target_app_path.clone(),
        target: environment.target,
        package_ids: elevated.clone(),
    };
    progress.report(ProgressEvent::AdministratorPrompt {
        package_ids: elevated.clone(),
    });
    environment
        .elevated_helper
        .start(&request)
        .map(|session| HelperSession(Some(session)))
        .map_err(|error| refused_run_items(to_remove, &elevated, &error))
}

/// The report items of a run whose administrator prompt was refused, with
/// nothing changed: declined, every package is Cancelled; otherwise
/// `elevated`, the packages that needed it, are Failed with the error, and
/// the rest weren't reached. For Windows' elevated helper and macOS's
/// administrator command alike.
pub(super) fn refused_run_items(
    to_remove: &[&UninstallPlanItem],
    elevated: &[String],
    error: &ElevationStartError,
) -> Vec<UninstallReportItem> {
    to_remove
        .iter()
        .map(|item| match error {
            ElevationStartError::Declined => new_item(
                item,
                UninstallStatus::Cancelled,
                UninstallMessage::ElevationDeclined,
            ),
            ElevationStartError::Failed(error) if elevated.contains(&item.package_id) => new_item(
                item,
                UninstallStatus::Failed,
                UninstallMessage::ElevationFailed {
                    error: error.clone(),
                },
            ),
            ElevationStartError::Failed(_) => new_item(
                item,
                UninstallStatus::NotReached,
                UninstallMessage::RunStopped {
                    package_id: elevated.first().cloned().unwrap_or_default(),
                },
            ),
        })
        .collect()
}

/// The report items of a run that stops before its first change because
/// of `blocked`, packages with what stops each: those are Blocked, and
/// every other package of `to_remove` wasn't reached.
pub(super) fn blocked_run_items(
    to_remove: &[&UninstallPlanItem],
    blocked: Vec<(String, UninstallMessage)>,
) -> Vec<UninstallReportItem> {
    let package_ids: Vec<String> = blocked.iter().map(|(id, _)| id.clone()).collect();
    to_remove
        .iter()
        .map(
            |plan_item| match blocked.iter().find(|(id, _)| *id == plan_item.package_id) {
                Some((_, code)) => new_item(plan_item, UninstallStatus::Blocked, code.clone()),
                None => new_item(
                    plan_item,
                    UninstallStatus::NotReached,
                    UninstallMessage::OtherPackageBlocked {
                        package_ids: package_ids.clone(),
                    },
                ),
            },
        )
        .collect()
}

/// After a removal RABBIT can't undo, the receipt follows fresh detection:
/// the package's entry goes, and it is remembered as removed, once
/// detection no longer finds it (`still_installed` is `Some(false)`).
/// While anything of it is found, or detection couldn't say, the entry
/// stays. Returns whether the receipt changed and needs saving.
pub(super) fn receipt_follows_detection(
    state: &mut InstallState,
    package_id: &str,
    still_installed: Option<bool>,
) -> bool {
    if still_installed == Some(false) {
        remove_package_receipt(state, package_id);
        true
    } else {
        false
    }
}

/// Whether fresh detection still finds `package_id`, `None` when it can't
/// say.
fn still_detected(environment: &UninstallEnvironment<'_>, package_id: &str) -> Option<bool> {
    (environment.detect)().ok().map(|detections| {
        detections
            .iter()
            .any(|detection| detection.package_id == package_id && detection.installed)
    })
}

/// The run's elevated helper, if it needs one. It is told to quit after the
/// last package, and also when the run ends any other way, unwinding
/// included: a helper is never left waiting for orders.
struct HelperSession(Option<Box<dyn ElevatedSession>>);

impl HelperSession {
    fn run(&mut self, package_id: &str, on_event: &mut dyn FnMut(&VendorEvent)) -> VendorOutcome {
        match &mut self.0 {
            Some(session) => session.run(package_id, on_event),
            None => {
                VendorOutcome::failed(package_id, false, "RABBIT's elevated helper isn't running")
            }
        }
    }

    /// Have the helper run the package's machine cleanup. Without a helper,
    /// which happens only if the run never started one for this package,
    /// it is answered like a lost one: the files go back and the run stops.
    fn run_cleanup(&mut self, package_id: &str) -> CleanupOutcome {
        match &mut self.0 {
            Some(session) => session.run_cleanup(package_id),
            None => CleanupOutcome::lost(package_id, "RABBIT's elevated helper isn't running"),
        }
    }

    fn finish(&mut self) {
        if let Some(session) = self.0.take() {
            session.finish();
        }
    }
}

impl Drop for HelperSession {
    fn drop(&mut self) {
        self.finish();
    }
}

/// Whether a package's outcome ends the run: a check stopped it, or its
/// vendor's uninstaller is still open, can't be followed any more, or took
/// files it must never take. The next package could collide with an
/// uninstaller still at work, so it isn't started. A machine cleanup ends
/// it too when the helper refused it, since something changed after
/// planning, or when RABBIT lost contact with the helper during it. On
/// macOS, so does a package the administrator command left half done, an
/// app macOS may or may not have moved to the Trash, and a Trash move macOS
/// itself refused: what refused that would refuse the next package too, and
/// the user has something to do first.
fn stops_the_run(item: &UninstallReportItem) -> bool {
    item.status == UninstallStatus::Blocked
        || matches!(
            item.message_code,
            UninstallMessage::VendorStillOpen { .. }
                | UninstallMessage::VendorRunFailed { .. }
                | UninstallMessage::VendorLost { .. }
                | UninstallMessage::CleanupRefused { .. }
                | UninstallMessage::CleanupLost { .. }
                | UninstallMessage::ResourceFolderDamaged { .. }
                | UninstallMessage::AdministratorStepFailed { .. }
                | UninstallMessage::TrashUnclear { .. }
                | UninstallMessage::TrashFailed {
                    refused_by_macos: true,
                    ..
                }
        )
}

/// The files no vendor's uninstaller may take, which exist now: REAPER's
/// own settings and RABBIT's receipt in the resource folder.
fn guarded_files(resource: &Path) -> Vec<PathBuf> {
    let receipt = RECEIPT_RELATIVE_PATH
        .split('/')
        .fold(resource.to_path_buf(), |path, part| path.join(part));
    [resource.join("reaper.ini"), receipt]
        .into_iter()
        .filter(|path| fs::symlink_metadata(path).is_ok())
        .collect()
}

/// Whether fresh detection finds the package only through what Windows
/// removes at the next restart, after its uninstaller reported exactly
/// that. Something detection found must be at or under a path the restart
/// takes: one of `still_present`, each of which has a removal queued below
/// it, or one of `pending_restart`. And nothing it found that the plan's
/// `verify_gone` names may be anything else. Its other files, such as a
/// vendor data folder a receipt lists next to the plug-in, don't place the
/// package on their own: a receipt only counts while every file it lists is
/// there.
fn found_only_pending_files(
    detection: &ComponentDetection,
    outcome: &VendorOutcome,
    plan_item: &UninstallPlanItem,
) -> bool {
    let pending = |path: &Path| {
        outcome
            .still_present
            .iter()
            .chain(&outcome.pending_restart)
            .any(|going| inside_or_same(path, going))
    };
    let to_go = |path: &Path| {
        plan_item.vendor_run.as_ref().is_some_and(|run| {
            run.verify_gone
                .iter()
                .any(|gone| inside_or_same(path, gone))
        })
    };
    let found: Vec<&PathBuf> = detection
        .files
        .iter()
        .filter(|path| fs::symlink_metadata(path).is_ok())
        .collect();
    found.iter().any(|path| pending(path))
        && found
            .iter()
            .filter(|path| to_go(path))
            .all(|path| pending(path))
}

/// What a vendor run left behind that should have gone: what
/// `report_leftovers` names, and what RABBIT meant to remove afterwards but
/// couldn't.
fn vendor_leftovers(outcome: &VendorOutcome) -> Vec<PathBuf> {
    let mut leftovers: BTreeSet<PathBuf> = outcome.leftovers.iter().cloned().collect();
    leftovers.extend(
        outcome
            .removed_leftovers
            .iter()
            .filter(|leftover| leftover.still_there())
            .map(|leftover| leftover.path.clone()),
    );
    leftovers.into_iter().collect()
}

/// The packages of one run that is removing things, with what they share.
struct RemovalRun<'a> {
    plan: &'a UninstallPlan,
    environment: &'a UninstallEnvironment<'a>,
    request: &'a UninstallRequest,
    backup_folder: &'a Path,
    backups: BackupSetWriter,
    /// The receipt as this run keeps it: loaded once, and saved after each
    /// package.
    state: InstallState,
    /// What the read before the run found, for the moves to use again.
    digests: Digests,
    reapack_installed: bool,
    progress: &'a ProgressReporter,
    /// The elevated helper, for packages a vendor's uninstaller removes
    /// and for machine cleanups.
    session: HelperSession,
}

impl RemovalRun<'_> {
    /// Remove `to_remove` in order, reading the cancel token before each
    /// package. After a stop, or a package a check blocks, the rest stay as
    /// they are.
    fn remove_all(
        &mut self,
        to_remove: &[&UninstallPlanItem],
        cancel: &CancelToken,
    ) -> Vec<UninstallReportItem> {
        let mut stopped: Option<UninstallMessage> = None;
        let mut outcomes = Vec::new();
        for plan_item in to_remove.iter().copied() {
            let item = match &stopped {
                Some(UninstallMessage::Cancelled) => new_item(
                    plan_item,
                    UninstallStatus::Cancelled,
                    UninstallMessage::Cancelled,
                ),
                Some(stop) => new_item(plan_item, UninstallStatus::NotReached, stop.clone()),
                None if cancel.is_cancelled() => {
                    stopped = Some(UninstallMessage::Cancelled);
                    new_item(
                        plan_item,
                        UninstallStatus::Cancelled,
                        UninstallMessage::Cancelled,
                    )
                }
                None => {
                    let item = self.remove(plan_item);
                    if stops_the_run(&item) {
                        stopped = Some(UninstallMessage::RunStopped {
                            package_id: plan_item.package_id.clone(),
                        });
                    }
                    item
                }
            };
            outcomes.push(item);
        }
        outcomes
    }

    /// Remove one package, from the last process check to the post helpers.
    fn remove(&mut self, plan_item: &UninstallPlanItem) -> UninstallReportItem {
        let id = plan_item.package_id.clone();
        // A program can start between the preflight and this package, or
        // between two packages.
        let failing: Vec<PreflightCheck> = process_checks(
            self.plan,
            std::slice::from_ref(&id),
            self.environment.target_app_path.as_deref(),
            &(self.environment.reaper_processes)(),
            &(self.environment.named_processes)(&plan_item.blocking_programs),
        )
        .into_iter()
        .filter(|check| check.status == PreflightStatus::Fail)
        .collect();
        if !failing.is_empty() {
            return new_item(
                plan_item,
                UninstallStatus::Blocked,
                UninstallMessage::ChecksFailed { checks: failing },
            );
        }

        let route = plan_item.route();
        self.progress.report(ProgressEvent::RemoveStarted {
            package_id: id.clone(),
            method: match route {
                RemovalRoute::Vendor => RemovalMethod::VendorUninstaller,
                RemovalRoute::Trash => RemovalMethod::Trash,
                RemovalRoute::Administrator => RemovalMethod::Administrator,
                RemovalRoute::BackupSet => RemovalMethod::Files,
            },
        });
        let item = match route {
            RemovalRoute::Vendor => self.remove_with_vendor(plan_item),
            RemovalRoute::Trash => self.remove_to_trash(plan_item),
            // The administrator command took its packages before this run
            // started; one reaching here would mean a step went missing.
            RemovalRoute::Administrator => new_item(
                plan_item,
                UninstallStatus::Failed,
                UninstallMessage::NotYetSupported,
            ),
            RemovalRoute::BackupSet => self.remove_started(plan_item),
        };
        self.progress.report(ProgressEvent::RemoveCompleted {
            package_id: id,
            removed: item.status.is_removed(),
        });
        item
    }

    /// Have the elevated helper run the package's own uninstaller, then
    /// read what it did: the status follows the uninstaller's outcome and
    /// fresh detection, and so does the receipt entry, which goes only once
    /// the package is gone.
    fn remove_with_vendor(&mut self, plan_item: &UninstallPlanItem) -> UninstallReportItem {
        let id = plan_item.package_id.as_str();
        let resource = self.plan.resource_path.clone();
        let guarded = guarded_files(&resource);
        let progress = self.progress;
        let outcome = self.session.run(id, &mut |event| match event {
            VendorEvent::Launched => {}
            VendorEvent::StillWaiting { seconds } => {
                progress.report(ProgressEvent::VendorStillWaiting {
                    package_id: id.to_string(),
                    seconds: *seconds,
                });
            }
            VendorEvent::WaitingForFiles { seconds } => {
                progress.report(ProgressEvent::VendorWaitingForFiles {
                    package_id: id.to_string(),
                    seconds: *seconds,
                });
            }
        });
        let missing: Vec<PathBuf> = guarded
            .into_iter()
            .filter(|path| fs::symlink_metadata(path).is_err())
            .collect();
        // `None` when detection itself fails: then nothing is concluded
        // from it, and the receipt stays as it is.
        let detected = (self.environment.detect)().ok().map(|detections| {
            detections
                .into_iter()
                .find(|detection| detection.package_id == id && detection.installed)
                // Detection still sees the files Windows removes at the
                // next restart; finding the package only through them is
                // what "removed pending a restart" means, not a failure.
                .filter(|detection| {
                    outcome.status != VendorStatus::RemovedPendingRestart
                        || !found_only_pending_files(detection, &outcome, plan_item)
                })
        });
        let still_installed = detected.as_ref().map(Option::is_some);
        // Windows' list after the run, as far as RABBIT could read it.
        let still_listed = (!outcome
            .problems
            .iter()
            .any(|problem| matches!(problem, VendorProblem::ListUnreadable { .. })))
        .then_some(outcome.still_listed);
        // The uninstaller started and nobody saw it end: the helper went
        // away, or waiting for it failed. It may still be at work.
        let end_unknown = outcome.launched && outcome.exit_code.is_none();

        let (status, code) = match outcome.status {
            VendorStatus::Refused => (
                UninstallStatus::Blocked,
                UninstallMessage::VendorRefused {
                    refusal: outcome
                        .refusal
                        .clone()
                        .unwrap_or(VendorRefusal::NotAVendorPlan),
                },
            ),
            VendorStatus::StillRunning => (
                UninstallStatus::Failed,
                UninstallMessage::VendorStillOpen {
                    seconds: plan_item
                        .vendor_run
                        .as_ref()
                        .map_or(0, |run| run.timeout_seconds),
                },
            ),
            VendorStatus::Cancelled => (
                UninstallStatus::Cancelled,
                UninstallMessage::VendorCancelled,
            ),
            VendorStatus::Failed => (
                UninstallStatus::Failed,
                match (&outcome.error, outcome.exit_code) {
                    (Some(error), _) if end_unknown => UninstallMessage::VendorLost {
                        error: error.clone(),
                    },
                    (Some(error), _) => UninstallMessage::VendorRunFailed {
                        error: error.clone(),
                    },
                    // It finished, by its exit code, and the package is
                    // still there.
                    (None, Some(exit_code))
                        if exit_code == 0
                            || plan_item
                                .vendor_run
                                .as_ref()
                                .is_some_and(|run| run.success_exit_codes.contains(&exit_code)) =>
                    {
                        UninstallMessage::VendorStillThere {
                            still_present: outcome.still_present.clone(),
                            still_listed,
                        }
                    }
                    (None, Some(exit_code)) => UninstallMessage::VendorFailed { exit_code },
                    (None, None) => UninstallMessage::VendorRunFailed {
                        error: "the uninstaller ended without saying how".to_string(),
                    },
                },
            ),
            VendorStatus::Removed
            | VendorStatus::RemovedWithLeftovers
            | VendorStatus::RemovedPendingRestart => match &detected {
                // The uninstaller is done, but RABBIT still finds the
                // package: it isn't removed, and its entry stays.
                Some(Some(detection)) => (
                    UninstallStatus::Failed,
                    UninstallMessage::VendorStillThere {
                        still_present: detection
                            .files
                            .iter()
                            .filter(|path| fs::symlink_metadata(path).is_ok())
                            .cloned()
                            .collect(),
                        // A removal is only reported once the entry is
                        // gone from the list the runner read.
                        still_listed: Some(false),
                    },
                ),
                _ => (
                    outcome.uninstall_status(),
                    UninstallMessage::UninstalledByVendor,
                ),
            },
        };

        let mut item = new_item(plan_item, status, code);
        item.pending_restart = outcome.pending_restart.clone();
        item.leftovers = vendor_leftovers(&outcome);
        item.problems
            .extend(outcome.problems.iter().map(UninstallProblem::from_vendor));

        // Once the uninstaller has ended, the receipt follows fresh
        // detection: the entry goes when the package is gone, and stays
        // while anything of it is found. An uninstaller that may still be
        // at work, still open at its time limit or lost track of, leaves
        // the receipt alone; nobody can say yet what it removes.
        let follows_detection = outcome.launched && !end_unknown;
        let receipt_changed =
            follows_detection && receipt_follows_detection(&mut self.state, id, still_installed);
        // An uninstaller must never take REAPER's settings or RABBIT's
        // receipt. If one did, the receipt is written again from the copy
        // this run holds, which also knows what the run removed so far;
        // REAPER's settings can't be brought back, and the report says so.
        let damaged = outcome.launched && !missing.is_empty();
        if (receipt_changed || damaged)
            && let Err(error) = save_install_state(&resource, &self.state)
        {
            item.problems.push(UninstallProblem::ReceiptNotSaved {
                error: error.to_string(),
            });
        }
        if damaged {
            item.status = UninstallStatus::Failed;
            item.message_code = UninstallMessage::ResourceFolderDamaged { missing };
        }
        item.vendor = Some(outcome);
        item
    }

    /// Move the package's files into the backup set, then, with a planned
    /// machine cleanup, have the elevated helper remove the uninstaller
    /// folder and registry entry, then take the package out of the receipt.
    fn remove_started(&mut self, plan_item: &UninstallPlanItem) -> UninstallReportItem {
        let id = plan_item.package_id.as_str();
        let record = BackupPackageRecord {
            receipt: self.state.packages.get(id).cloned(),
            langpack_before: None,
        };
        if let Err(error) = self.backups.record_package(id, record) {
            return new_item(
                plan_item,
                UninstallStatus::Failed,
                UninstallMessage::BackupFailed {
                    error: error.to_string(),
                },
            );
        }

        let mut removal = PackageRemoval::new(
            plan_item,
            self.environment,
            self.backup_folder,
            Some(&mut self.backups),
            &mut self.digests,
            self.reapack_installed,
        );
        let mut cleanup: Option<CleanupOutcome> = None;
        let mut receipt_problem = None;
        let mut outcome = removal.remove_files();
        if outcome.is_ok() {
            removal.remove_dirs_if_empty();
            // Only once every file is in the backup set: nothing a backup
            // can't bring back happens unless all of that worked.
            if plan_item.machine_cleanup.is_some() {
                let done = self.session.run_cleanup(id);
                outcome = cleanup_result(&done);
                cleanup = Some(done);
            }
        }
        if outcome.is_ok() {
            if cleanup
                .as_ref()
                .is_some_and(CleanupOutcome::removed_anything)
            {
                // The folder or the entry is gone for good, so the package
                // is removed whatever the save does, and the receipt in
                // memory stays without it: a later package's save then
                // writes that too, as after a move to the Trash.
                remove_package_receipt(&mut self.state, id);
                if let Err(error) = save_install_state(&self.plan.resource_path, &self.state) {
                    receipt_problem = Some(UninstallProblem::ReceiptNotSaved {
                        error: error.to_string(),
                    });
                }
            } else {
                outcome = save_receipt_without(&mut self.state, &self.plan.resource_path, id);
            }
        }
        if let Err(code) = outcome {
            let (put_back, left_in_backup) = removal.put_everything_back();
            let mut item = removal.into_item(UninstallStatus::Failed, code);
            item.removed.clear();
            item.removed_dirs.clear();
            item.put_back = put_back;
            if left_in_backup.is_empty() {
                let _ = self.backups.forget_package(id);
            }
            item.left_in_backup = left_in_backup;
            item.cleanup = cleanup;
            return item;
        }
        let mut item = removal.into_item(
            UninstallStatus::Removed,
            UninstallMessage::Removed {
                backup_set_id: None,
            },
        );
        item.cleanup = cleanup;
        item.problems.extend(receipt_problem);
        self.run_post_steps(plan_item, &mut item);
        // Only once the receipt no longer claims the package: an entry that
        // can't go is a problem to report, never a reason to put the files
        // back.
        clean_registry(&mut item, plan_item, self.environment, true);
        item
    }

    /// The plan's post helpers, after the package is removed. One that
    /// fails is a problem to report, never a reason to undo the removal.
    fn run_post_steps(&mut self, plan_item: &UninstallPlanItem, item: &mut UninstallReportItem) {
        let id = plan_item.package_id.as_str();
        for step in &plan_item.post {
            match step {
                UninstallPostStep::ResetReaperLangpackIfSelected {} => {
                    let removed: Vec<PathBuf> =
                        item.removed.iter().map(|file| file.path.clone()).collect();
                    if let Err(error) = reset_reaper_langpack_if_selected(
                        &self.plan.resource_path,
                        &removed,
                        self.request.platform == Platform::Windows,
                        &mut self.backups,
                        id,
                    ) {
                        item.problems.push(UninstallProblem::LanguageResetFailed {
                            error: error.to_string(),
                        });
                    }
                }
                // Asked as the user, after the files are gone. macOS starts
                // the registrar again when a host next needs it, so a
                // refresh that can't happen only means a host may list a
                // plug-in that is gone until it next looks: never a reason
                // to undo the removal, nor one to report.
                UninstallPostStep::RefreshAudioUnits {} => {
                    let _ = self.environment.macos_objects.refresh_audio_units();
                }
            }
        }
    }

    /// Move a package's app to the Trash as the user, on macOS: REAPER.app
    /// when this account may move it, last in the run.
    ///
    /// Anything of the package that goes into the backup set, such as its
    /// settings with "Also remove settings", goes first, the usual way, so
    /// it can still be put back. Then each app: the guard and the question
    /// of who may move it are asked again, and the answer must still be
    /// that the user may, or nothing more happens; then macOS moves it.
    /// It counts as moved only once its old place is empty and macOS named
    /// where it went: a refusal is TrashFailed, anything less certain
    /// TrashUnclear. On either, what went into the backup set goes back.
    /// An app gone since the plan isn't moved, and the package is reported
    /// as removed, not as moved to the Trash.
    ///
    /// The receipt then follows fresh detection, as after a vendor's
    /// uninstaller: a move to the Trash can't be put back by RABBIT, so a
    /// receipt that can't be saved is only a problem to report.
    fn remove_to_trash(&mut self, plan_item: &UninstallPlanItem) -> UninstallReportItem {
        let id = plan_item.package_id.as_str();
        let environment = self.environment;
        let into_backup = plan_item
            .files
            .iter()
            .any(|file| file.reason != RemovalReason::Tree);
        if into_backup {
            let record = BackupPackageRecord {
                receipt: self.state.packages.get(id).cloned(),
                langpack_before: None,
            };
            if let Err(error) = self.backups.record_package(id, record) {
                return new_item(
                    plan_item,
                    UninstallStatus::Failed,
                    UninstallMessage::BackupFailed {
                        error: error.to_string(),
                    },
                );
            }
        }

        let mut removal = PackageRemoval::new(
            plan_item,
            environment,
            self.backup_folder,
            Some(&mut self.backups),
            &mut self.digests,
            self.reapack_installed,
        );
        let mut trashed = Vec::new();
        let mut outcome = removal.remove_files();
        if outcome.is_ok() {
            outcome = trash_trees(plan_item, environment, &mut trashed);
        }
        if let Err(code) = outcome {
            let (put_back, left_in_backup) = removal.put_everything_back();
            let mut item = removal.into_item(UninstallStatus::Failed, code);
            item.removed.clear();
            item.removed_dirs.clear();
            item.put_back = put_back;
            if into_backup && left_in_backup.is_empty() {
                let _ = self.backups.forget_package(id);
            }
            item.left_in_backup = left_in_backup;
            // An app macOS may have moved after all: what is gone is for
            // detection to say.
            if matches!(item.message_code, UninstallMessage::TrashUnclear { .. })
                && receipt_follows_detection(&mut self.state, id, still_detected(environment, id))
                && let Err(error) = save_install_state(&self.plan.resource_path, &self.state)
            {
                item.problems.push(UninstallProblem::ReceiptNotSaved {
                    error: error.to_string(),
                });
            }
            return item;
        }
        removal.remove_dirs_if_empty();
        // Every app was gone before its turn: nothing went to the Trash, so
        // nothing may be said to have moved there. The run fills in the
        // backup set for settings that went into one.
        let code = match trashed.first() {
            Some(file) => UninstallMessage::MovedToTrash {
                trashed_to: file.trashed_to.clone(),
            },
            None => UninstallMessage::Removed {
                backup_set_id: None,
            },
        };
        let mut item = removal.into_item(UninstallStatus::Removed, code);
        item.removed.extend(trashed);
        if receipt_follows_detection(&mut self.state, id, still_detected(environment, id))
            && let Err(error) = save_install_state(&self.plan.resource_path, &self.state)
        {
            item.problems.push(UninstallProblem::ReceiptNotSaved {
                error: error.to_string(),
            });
        }
        self.run_post_steps(plan_item, &mut item);
        clean_registry(&mut item, plan_item, environment, true);
        item
    }
}

/// Move each tree of `plan_item` to the Trash as the user, noting each on
/// `trashed`. Stops at the first that doesn't go, with why; see
/// [`RemovalRun::remove_to_trash`].
fn trash_trees(
    plan_item: &UninstallPlanItem,
    environment: &UninstallEnvironment<'_>,
    trashed: &mut Vec<RemovedFile>,
) -> std::result::Result<(), UninstallMessage> {
    let objects = environment.macos_objects;
    for file in plan_item
        .files
        .iter()
        .filter(|file| file.reason == RemovalReason::Tree)
    {
        // Gone since the plan was built: nothing left to move.
        if matches!(fs::symlink_metadata(&file.path), Err(error) if error.kind() == io::ErrorKind::NotFound)
        {
            continue;
        }
        file.check(environment.roots)
            .map_err(|refusal| UninstallMessage::PathRefused {
                path: file.path.clone(),
                refusal,
            })?;
        let route = tree_route(
            &file.path,
            &objects.facts(&file.path),
            &objects.user_trash_folder_facts(),
            file.bundle_id.as_deref(),
            &environment.roots.system_root,
        );
        if route != TreeRoute::Trash {
            let why = match route {
                TreeRoute::ByHand(reason) => reason.message(),
                _ => "only an administrator may move it now".to_string(),
            };
            return Err(UninstallMessage::TrashFailed {
                error: format!(
                    "{} changed since RABBIT checked it: {why}",
                    file.path.display()
                ),
                refused_by_macos: false,
            });
        }
        let moved = objects.move_to_trash(&file.path);
        let original_gone = matches!(
            fs::symlink_metadata(&file.path),
            Err(error) if error.kind() == io::ErrorKind::NotFound
        );
        match moved {
            // macOS named where it went. The Trash is a place macOS may keep
            // private, so a place RABBIT can't read is taken at macOS's
            // word; only one that plainly isn't there makes it unclear.
            Ok(resulting)
                if original_gone
                    && !matches!(
                        fs::symlink_metadata(&resulting),
                        Err(error) if error.kind() == io::ErrorKind::NotFound
                    ) =>
            {
                trashed.push(RemovedFile {
                    path: file.path.clone(),
                    reason: RemovalReason::Tree,
                    size: 0,
                    changed_since_install: false,
                    backup_path: None,
                    trashed_to: Some(resulting),
                });
            }
            Ok(_) => {
                return Err(UninstallMessage::TrashUnclear {
                    path: file.path.clone(),
                });
            }
            // macOS says it failed, but the app has left its place.
            Err(_) if original_gone => {
                return Err(UninstallMessage::TrashUnclear {
                    path: file.path.clone(),
                });
            }
            Err(error) => {
                return Err(UninstallMessage::TrashFailed {
                    refused_by_macos: error.kind
                        == rabbit_platform::TrashErrorKind::PermissionDenied,
                    error: error.message,
                });
            }
        }
    }
    Ok(())
}

/// Remove the plan's entries in Windows' list of installed programs that
/// still point into this REAPER, or with `remove` false say which would go,
/// and note what became of each on `item`.
fn clean_registry(
    item: &mut UninstallReportItem,
    plan_item: &UninstallPlanItem,
    environment: &UninstallEnvironment<'_>,
    remove: bool,
) {
    let Some(list) = environment.installed_programs else {
        return;
    };
    let (entries, problems) = clean_registry_entries(list, &plan_item.registry_cleanup, remove);
    item.registry_entries.extend(entries);
    item.problems.extend(problems);
}

/// What a machine cleanup's outcome means for its package: it goes on when
/// everything that was there is gone, or nothing was; otherwise its files
/// go back, with the message that says why.
fn cleanup_result(cleanup: &CleanupOutcome) -> std::result::Result<(), UninstallMessage> {
    let error = || {
        cleanup
            .error
            .clone()
            .unwrap_or_else(|| "the helper didn't say why".to_string())
    };
    match cleanup.status {
        CleanupStatus::Removed | CleanupStatus::NothingThere => Ok(()),
        CleanupStatus::Refused => Err(UninstallMessage::CleanupRefused {
            refusal: cleanup
                .refusal
                .clone()
                .unwrap_or(VendorRefusal::NotAVendorPlan),
        }),
        CleanupStatus::Failed => Err(UninstallMessage::CleanupFailed { error: error() }),
        CleanupStatus::Lost => Err(UninstallMessage::CleanupLost { error: error() }),
    }
}

/// Take `package_id` out of the receipt, remember that the user removed it,
/// and save. When the save fails, the receipt in memory goes back to how it
/// was, matching the file, which a failed save leaves alone.
fn save_receipt_without(
    state: &mut InstallState,
    resource: &Path,
    package_id: &str,
) -> std::result::Result<(), UninstallMessage> {
    let was_declined = state.declined_packages.contains(package_id);
    let entry = remove_package_receipt(state, package_id);
    if let Err(error) = save_install_state(resource, state) {
        if let Some(entry) = entry {
            state.packages.insert(package_id.to_string(), entry);
        }
        if !was_declined {
            state.declined_packages.remove(package_id);
        }
        return Err(UninstallMessage::ReceiptSaveFailed {
            error: error.to_string(),
        });
    }
    Ok(())
}

/// What a dry run reports for a package it would remove: the exact files,
/// hashed as the real run would, and those the real run would keep. Also
/// the hashes it read, for the real run to use again.
fn survey(
    plan_item: &UninstallPlanItem,
    environment: &UninstallEnvironment<'_>,
    backup_folder: &Path,
    reapack_installed: bool,
) -> (UninstallReportItem, Digests) {
    let mut digests = Digests::new();
    let mut removal = PackageRemoval::new(
        plan_item,
        environment,
        backup_folder,
        None,
        &mut digests,
        reapack_installed,
    );
    let item = match removal.remove_files() {
        Ok(()) => removal.into_item(UninstallStatus::WouldRemove, UninstallMessage::WouldRemove),
        // The real run would stop here; see `survey_before_removal`.
        Err(code) => removal.into_item(UninstallStatus::Blocked, code),
    };
    (item, digests)
}

/// Before a real run moves anything, read every file of every package the
/// way the dry run does. A path the guard refuses, or a file or settings
/// folder that can't be read, then blocks the whole run with nothing
/// changed, just as the dry run said. Found only while removing, it would
/// stop the run after the packages before it were already gone.
///
/// Returns the hashes read, which the moves use again (see
/// [`DIGEST_REUSE_MARGIN`]), or the report items for a blocked run: the
/// packages that failed the read are Blocked and the others NotReached.
fn survey_before_removal(
    to_remove: &[&UninstallPlanItem],
    environment: &UninstallEnvironment<'_>,
    backup_folder: &Path,
    reapack_installed: bool,
    cancel: &CancelToken,
) -> std::result::Result<Digests, Vec<UninstallReportItem>> {
    let mut digests = Digests::new();
    let mut blocked: Vec<(String, UninstallMessage)> = Vec::new();
    for plan_item in to_remove {
        // The removal loop turns a stop into Cancelled items before it
        // touches anything; reading the rest first would only delay that.
        if cancel.is_cancelled() {
            break;
        }
        let (item, found) = survey(plan_item, environment, backup_folder, reapack_installed);
        if item.status == UninstallStatus::Blocked {
            blocked.push((item.package_id, item.message_code));
        }
        digests.extend(found);
    }
    if blocked.is_empty() {
        return Ok(digests);
    }
    Err(blocked_run_items(to_remove, blocked))
}

/// What the read before a run found about one file: its size and hash, and
/// the modification time it had just before it was read.
pub(super) struct SurveyedDigest {
    digest: FileDigest,
    modified: SystemTime,
}

/// The files the read before a run hashed, by path.
type Digests = HashMap<PathBuf, SurveyedDigest>;

impl SurveyedDigest {
    /// Keep `digest` for later only when the file, as `before` describes it
    /// just before it was read at `read_at`, was last changed at least
    /// [`DIGEST_REUSE_MARGIN`] earlier and still had the size read.
    pub(super) fn new(
        before: &fs::Metadata,
        read_at: SystemTime,
        digest: FileDigest,
    ) -> Option<Self> {
        let modified = before.modified().ok()?;
        let settled = read_at
            .duration_since(modified)
            .is_ok_and(|age| age >= DIGEST_REUSE_MARGIN);
        (settled && before.len() == digest.size).then_some(Self { digest, modified })
    }

    /// The file at `path` still has the size and modification time it had
    /// when it was read, so it holds the bytes that were hashed then.
    pub(super) fn still_matches(&self, path: &Path) -> bool {
        fs::metadata(path).is_ok_and(|now| {
            now.len() == self.digest.size && now.modified().ok() == Some(self.modified)
        })
    }
}

/// One package's files on their way into the backup set, and what moved so
/// far, so it can all go back.
struct PackageRemoval<'a> {
    item: &'a UninstallPlanItem,
    roots: &'a UninstallRoots,
    volumes: &'a dyn VolumeProbe,
    backup_folder: &'a Path,
    /// `None` in a dry run: nothing moves, the files are only listed.
    backups: Option<&'a mut BackupSetWriter>,
    /// Without `backups`, where the hashes read go; with them, where the
    /// moves find them again.
    digests: &'a mut Digests,
    reapack_installed: bool,
    moved: Vec<MovedFile>,
    removed: Vec<RemovedFile>,
    kept: Vec<KeptFile>,
    removed_dirs: Vec<PathBuf>,
}

impl<'a> PackageRemoval<'a> {
    fn new(
        item: &'a UninstallPlanItem,
        environment: &'a UninstallEnvironment<'a>,
        backup_folder: &'a Path,
        backups: Option<&'a mut BackupSetWriter>,
        digests: &'a mut Digests,
        reapack_installed: bool,
    ) -> Self {
        Self {
            item,
            roots: environment.roots,
            volumes: environment.volumes,
            backup_folder,
            backups,
            digests,
            reapack_installed,
            moved: Vec::new(),
            removed: Vec::new(),
            kept: Vec::new(),
            removed_dirs: Vec::new(),
        }
    }

    /// Every file of the plan, in order: the package's own files, then its
    /// settings when "Also remove settings" was asked for.
    fn remove_files(&mut self) -> std::result::Result<(), UninstallMessage> {
        let item = self.item;
        let whole_objects = item.route().takes_whole_objects();
        for file in &item.files {
            // Gone since the plan was built: nothing left to remove.
            if matches!(fs::symlink_metadata(&file.path), Err(error) if error.kind() == io::ErrorKind::NotFound)
            {
                continue;
            }
            // Files can change between the plan and the move, so the guard
            // has its say again right before it.
            file.check(self.roots)
                .map_err(|refusal| UninstallMessage::PathRefused {
                    path: file.path.clone(),
                    refusal,
                })?;
            if whole_objects && file.reason == RemovalReason::Tree {
                self.take_whole(&file.path)?;
            } else if file.is_dir {
                self.remove_folder(&file.path, file.reason)?;
            } else {
                self.remove_file(&file.path, file.reason, file.receipt_sha256.as_deref())?;
            }
        }
        Ok(())
    }

    /// A tree that leaves whole, through the Trash or the administrator
    /// command: the guard has checked it and it is there, and that is all.
    /// It is never listed, hashed or walked: an app bundle holds thousands
    /// of files and often links, none of which goes into a backup set. Its
    /// size isn't counted either, so the report gives it none.
    fn take_whole(&mut self, path: &Path) -> std::result::Result<(), UninstallMessage> {
        // Never into the backup set: a real run removes such a tree through
        // its own route, after the files that do go into the set.
        if self.backups.is_some() {
            return Ok(());
        }
        self.removed.push(RemovedFile {
            path: path.to_path_buf(),
            reason: RemovalReason::Tree,
            size: 0,
            changed_since_install: false,
            backup_path: None,
            trashed_to: None,
        });
        Ok(())
    }

    fn remove_file(
        &mut self,
        path: &Path,
        reason: RemovalReason,
        receipt_sha256: Option<&str>,
    ) -> std::result::Result<(), UninstallMessage> {
        let digest = self.digest_of(path)?;
        let changed =
            receipt_sha256.is_some_and(|expected| !expected.eq_ignore_ascii_case(&digest.sha256));
        // Only RABBIT's receipt vouches for this file, and it isn't the file
        // RABBIT wrote any more: most likely the user's own version now.
        if changed && !reason.named_by_manifest() {
            self.kept.push(KeptFile {
                path: path.to_path_buf(),
                reason: KeptFileReason::ChangedSinceInstall,
            });
            return Ok(());
        }
        let size = digest.size;
        let backup_path = match self.backups.as_deref_mut() {
            None => None,
            Some(backups) => {
                let across = crosses_volumes(path, self.backup_folder, self.volumes);
                let moved = backups
                    .move_file(Some(&self.item.package_id), path, Some(digest), across)
                    .map_err(|error| match error {
                        MoveFileError::InUse { path, source } => UninstallMessage::FileInUse {
                            path,
                            error: source.to_string(),
                        },
                        MoveFileError::NotMoved(error) => UninstallMessage::MoveFailed {
                            path: path.to_path_buf(),
                            error: error.to_string(),
                        },
                    })?;
                let backup_path = moved.backup_path.clone();
                self.moved.push(moved);
                Some(backup_path)
            }
        };
        self.removed.push(RemovedFile {
            path: path.to_path_buf(),
            reason,
            size,
            changed_since_install: changed,
            backup_path,
            trashed_to: None,
        });
        Ok(())
    }

    /// `path`'s size and hash. Moving a file uses what the read before the
    /// run found when the file can't have changed since, so FFmpeg's 250 MB
    /// is hashed once, not twice. The read itself hashes the file and keeps
    /// the answer.
    fn digest_of(&mut self, path: &Path) -> std::result::Result<FileDigest, UninstallMessage> {
        let unreadable = |error: String| UninstallMessage::Unreadable {
            path: path.to_path_buf(),
            error,
        };
        if self.backups.is_some() {
            if let Some(surveyed) = self.digests.remove(path)
                && surveyed.still_matches(path)
            {
                return Ok(surveyed.digest);
            }
            return FileDigest::of(path).map_err(|error| unreadable(error.to_string()));
        }
        let before = fs::metadata(path).map_err(|error| unreadable(error.to_string()))?;
        let read_at = SystemTime::now();
        let digest = FileDigest::of(path).map_err(|error| unreadable(error.to_string()))?;
        if let Some(surveyed) = SurveyedDigest::new(&before, read_at, digest.clone()) {
            self.digests.insert(path.to_path_buf(), surveyed);
        }
        Ok(digest)
    }

    /// A settings folder goes file by file, so the backup set lists each
    /// file and a restore can put each one back. A folder holding a link
    /// stays whole: RABBIT never follows one. While ReaPack is installed, a
    /// file its records claim stays: a ReaPack data package can install into
    /// a folder a manifest lists as the package's settings.
    fn remove_folder(
        &mut self,
        folder: &Path,
        reason: RemovalReason,
    ) -> std::result::Result<(), UninstallMessage> {
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        match list_folder(folder, &mut files, &mut dirs) {
            Ok(true) => {}
            Ok(false) => {
                self.kept.push(KeptFile {
                    path: folder.to_path_buf(),
                    reason: KeptFileReason::HoldsLink,
                });
                return Ok(());
            }
            Err(error) => {
                return Err(UninstallMessage::Unreadable {
                    path: folder.to_path_buf(),
                    error: error.to_string(),
                });
            }
        }
        for file in &files {
            if self.reapack_installed && installed_by_reapack(self.roots, file) {
                self.kept.push(KeptFile {
                    path: file.clone(),
                    reason: KeptFileReason::InstalledByReaPack,
                });
                continue;
            }
            self.remove_file(file, reason, None)?;
        }
        if self.backups.is_some() {
            dirs.sort_by_key(|dir| Reverse(dir.components().count()));
            dirs.push(folder.to_path_buf());
            for dir in dirs {
                if fs::remove_dir(&dir).is_ok() {
                    self.removed_dirs.push(dir);
                }
            }
        }
        Ok(())
    }

    /// The plan's `dirs_if_empty`, innermost first, with a non-recursive
    /// remove that fails, harmlessly, on a folder that still holds anything.
    fn remove_dirs_if_empty(&mut self) {
        let mut dirs = self.item.dirs_if_empty.clone();
        dirs.sort_by_key(|dir| Reverse(dir.components().count()));
        for dir in dirs {
            let plain_folder = fs::symlink_metadata(&dir)
                .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink());
            if plain_folder && fs::remove_dir(&dir).is_ok() {
                self.removed_dirs.push(dir);
            }
        }
    }

    /// Move every file this package moved back, last first. Returns the
    /// files that went back and those still in the backup set.
    fn put_everything_back(&mut self) -> (Vec<PathBuf>, Vec<PathBuf>) {
        let mut put_back = Vec::new();
        let mut left = Vec::new();
        let Some(backups) = self.backups.as_deref_mut() else {
            return (put_back, left);
        };
        for moved in self.moved.drain(..).rev() {
            match backups.put_back(&moved) {
                Ok(()) => put_back.push(moved.original),
                Err(_) => left.push(moved.original),
            }
        }
        put_back.reverse();
        left.reverse();
        (put_back, left)
    }

    fn into_item(self, status: UninstallStatus, code: UninstallMessage) -> UninstallReportItem {
        UninstallReportItem {
            removed: self.removed,
            kept: self.kept,
            removed_dirs: self.removed_dirs,
            ..new_item(self.item, status, code)
        }
    }
}

/// The files and folders inside `folder`, in name order, without following
/// anything. `Ok(false)` as soon as a symbolic link or junction turns up.
fn list_folder(
    folder: &Path,
    files: &mut Vec<PathBuf>,
    dirs: &mut Vec<PathBuf>,
) -> io::Result<bool> {
    let mut entries: Vec<PathBuf> = fs::read_dir(folder)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<_>>()?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Ok(false);
        }
        if metadata.is_dir() {
            dirs.push(path.clone());
            if !list_folder(&path, files, dirs)? {
                return Ok(false);
            }
        } else {
            files.push(path);
        }
    }
    Ok(true)
}

/// Ask detection again, and mark removed packages it still finds as removed
/// with leftovers. Leftovers are only reported, never deleted.
fn report_leftovers(items: &mut [UninstallReportItem], environment: &UninstallEnvironment<'_>) {
    let Ok(detections) = (environment.detect)() else {
        return;
    };
    for item in items
        .iter_mut()
        .filter(|item| item.status == UninstallStatus::Removed)
    {
        let Some(detection) = detections
            .iter()
            .find(|detection| detection.package_id == item.package_id && detection.installed)
        else {
            continue;
        };
        item.status = UninstallStatus::RemovedWithLeftovers;
        let leftovers: BTreeSet<PathBuf> = detection
            .files
            .iter()
            .filter(|path| fs::symlink_metadata(path).is_ok())
            .cloned()
            .collect();
        item.leftovers = leftovers.into_iter().collect();
    }
}

pub(super) fn new_item(
    plan_item: &UninstallPlanItem,
    status: UninstallStatus,
    code: UninstallMessage,
) -> UninstallReportItem {
    UninstallReportItem {
        display_name: plan_item.display_name.clone(),
        ..new_item_for(&plan_item.package_id, status, code)
    }
}

fn new_item_for(
    package_id: &str,
    status: UninstallStatus,
    code: UninstallMessage,
) -> UninstallReportItem {
    UninstallReportItem {
        package_id: package_id.to_string(),
        display_name: package_id.to_string(),
        status,
        removed: Vec::new(),
        kept: Vec::new(),
        removed_dirs: Vec::new(),
        put_back: Vec::new(),
        left_in_backup: Vec::new(),
        leftovers: Vec::new(),
        registry_entries: Vec::new(),
        problems: Vec::new(),
        pending_restart: Vec::new(),
        vendor: None,
        cleanup: None,
        message: String::new(),
        message_code: code,
    }
}

/// What to do when macOS itself refused a step: App Management protects
/// apps from changes by programs the user hasn't allowed, root's included.
/// The report's English names both RABBIT and the app the CLI runs from;
/// the CLI swaps in a sentence of its own, which only names the latter.
pub const APP_MANAGEMENT_HINT: &str = "If macOS refused it, turn RABBIT on under System Settings, \
     Privacy & Security, App Management (or the app you run rabbit from, such as Terminal), quit \
     RABBIT, start it again and uninstall again.";

/// `text` closed as a sentence: macOS's own words (rm's error output, an
/// NSError's description) seldom end in a full stop, and RABBIT's next
/// sentence follows them, which a screen reader would read as one.
fn closed_sentence(text: &str) -> String {
    let text = text.trim_end();
    if text.ends_with(['.', '!', '?']) {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

/// The item's English message, for `plan`'s platform: what happened, then
/// anything kept, left over or gone wrong afterwards.
pub(super) fn english_message(item: &UninstallReportItem, plan: &UninstallPlan) -> String {
    let name = &item.display_name;
    let mac = plan.platform == Platform::MacOs;
    let mut message = match &item.message_code {
        UninstallMessage::Removed { backup_set_id } => {
            match (item.removed.is_empty(), backup_set_id) {
                (true, _) => format!(
                    "RABBIT removed no files of {name}, and no longer counts it as installed."
                ),
                (false, Some(id)) => {
                    format!("RABBIT removed {name}. Its files are in the backup set {id}.")
                }
                (false, None) => format!("RABBIT removed {name}."),
            }
        }
        UninstallMessage::WouldRemove => format!("RABBIT would remove {name}."),
        UninstallMessage::NotInstalled => {
            format!("{name} isn't installed, so there is nothing to remove.")
        }
        UninstallMessage::UnknownPackage => {
            format!("RABBIT knows no package called {name} on this computer.")
        }
        UninstallMessage::NotRemovable { removability } => removability.message(name),
        UninstallMessage::NotYetSupported => format!(
            "This version of RABBIT can't remove {name} yet: it takes a step a backup can't \
             undo."
        ),
        UninstallMessage::ChecksFailed { checks } => format!(
            "{name} can't be removed right now: {}.",
            checks
                .iter()
                .map(|check| check.message.trim_end_matches('.'))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        UninstallMessage::OtherPackageBlocked { package_ids } => format!(
            "Nothing was removed, because RABBIT can't remove {}. Leave {} out to remove \
             {name}.",
            package_ids.join(", "),
            if package_ids.len() == 1 { "it" } else { "them" }
        ),
        UninstallMessage::Cancelled => {
            format!("The run was stopped before {name}, so it is still installed.")
        }
        UninstallMessage::RunStopped { package_id } => {
            format!("The run stopped at {package_id}, before {name}, so {name} is still installed.")
        }
        UninstallMessage::PathRefused { path, refusal } => format!(
            "RABBIT stopped removing {name}: it won't remove {}, because {refusal}.",
            path.display()
        ),
        UninstallMessage::Unreadable { path, error } => format!(
            "RABBIT couldn't read {}, so it stopped removing {name}: {error}",
            path.display()
        ),
        UninstallMessage::MoveFailed { path, error } => format!(
            "RABBIT couldn't move {} into the backup set, so it stopped removing {name}: \
             {error}",
            path.display()
        ),
        // Only the JAWS scripts' installer adds such files. JAWS may hold
        // one open, and the program to close would be the user's screen
        // reader, so this says to wait and try again instead.
        UninstallMessage::FileInUse { path, error }
            if plan
                .item(&item.package_id)
                .is_some_and(|planned| planned.lists_vendor_added(path)) =>
        {
            format!(
                "JAWS or another program is using {}, so RABBIT couldn't remove {name}. Wait a \
                 moment, then run RABBIT again; if that doesn't help, restart JAWS first. \
                 ({error})",
                path.display()
            )
        }
        UninstallMessage::FileInUse { path, error } => format!(
            "{} is in use by another program, so RABBIT couldn't remove {name}. Close that \
             program and try again. ({error})",
            path.display()
        ),
        UninstallMessage::BackupFailed { error } => format!(
            "RABBIT couldn't write the backup set, so it removed nothing of {name}: {error}"
        ),
        UninstallMessage::ReceiptSaveFailed { error } => format!(
            "RABBIT couldn't save its receipt after moving {name}'s files, so it moved them \
             back: {error}"
        ),
        UninstallMessage::UninstalledByVendor => match item.status {
            UninstallStatus::RemovedPendingRestart => format!(
                "{name}'s own uninstaller removed it. Some of its files go when you restart the \
                 computer: {}.",
                path_list(&item.pending_restart)
            ),
            _ => format!("{name}'s own uninstaller removed it. A backup can't bring it back."),
        },
        UninstallMessage::VendorRefused { refusal } => format!(
            "RABBIT didn't start {name}'s own uninstaller: {refusal}. Nothing of {name} changed, \
             and the run stopped there. You can uninstall {name} in Windows Settings, under \
             Apps, Installed apps."
        ),
        UninstallMessage::VendorCancelled => {
            format!("{name}'s own uninstaller was cancelled before it finished.")
        }
        UninstallMessage::VendorFailed { exit_code } => format!(
            "{name}'s own uninstaller ended with error code {exit_code}, so {name} may not be \
             fully removed."
        ),
        UninstallMessage::VendorStillThere {
            still_present,
            still_listed,
        } => {
            // Nothing found and Windows not listing it either, as far as
            // RABBIT could tell: only the check fell short, as the wizard
            // says too.
            let mut message = if still_present.is_empty() && *still_listed != Some(true) {
                format!(
                    "{name}'s own uninstaller finished, but RABBIT can't confirm that {name} is gone."
                )
            } else {
                format!("{name}'s own uninstaller finished, but {name} is still installed.")
            };
            if !still_present.is_empty() {
                message.push_str(&format!(" Still there: {}.", path_list(still_present)));
            }
            // When the list couldn't be read, a problem says so.
            match still_listed {
                Some(true) => {
                    message.push_str(" Windows still lists it among the installed programs.");
                }
                Some(false) => message.push_str(
                    " Windows no longer lists it among the installed programs, so it isn't in \
                     Windows Settings either.",
                ),
                None => {}
            }
            message
        }
        UninstallMessage::VendorStillOpen { seconds } => format!(
            "{name}'s own uninstaller was still open after {seconds} seconds. RABBIT left it \
             running and stopped, so the packages after it weren't touched. It may be showing \
             a question in a window of its own."
        ),
        UninstallMessage::VendorRunFailed { error } => format!(
            "RABBIT couldn't finish running {name}'s own uninstaller, so it stopped there: \
             {error}"
        ),
        UninstallMessage::VendorLost { error } => {
            let mut message = format!(
                "RABBIT lost track of {name}'s own uninstaller after it started ({error}). It \
                 may still be running, so RABBIT can't tell what it removed. RABBIT stopped \
                 there, so the packages after it weren't touched, and its receipt still lists \
                 {name}."
            );
            if item.package_id == PACKAGE_REAPER {
                message.push_str(
                    " Once the uninstaller has finished, check that reaper.ini is still in \
                     REAPER's resource folder: it holds your REAPER settings.",
                );
            }
            message
        }
        UninstallMessage::CleanupRefused { refusal } => format!(
            "RABBIT checked {name}'s uninstaller folder in Program Files and its registry entry \
             again and didn't trust what it found ({refusal}), so it removed neither and \
             stopped there."
        ),
        // Part of the folder or the entry may be gone, which no backup
        // brings back, so this never says everything is back in place.
        UninstallMessage::CleanupFailed { error } => format!(
            "RABBIT couldn't remove all of {name}'s uninstaller folder and registry entry \
             ({error}), so it put back what it had moved."
        ),
        UninstallMessage::CleanupLost { error } => format!(
            "RABBIT lost contact with its helper while it removed {name}'s uninstaller folder \
             and registry entry ({error}), so it put back what it had moved and stopped there. \
             Part of that folder or its registry entry may be gone."
        ),
        UninstallMessage::ElevationDeclined if mac => format!(
            "The administrator password prompt was cancelled, so RABBIT uninstalled nothing; \
             {name} is still installed."
        ),
        UninstallMessage::ElevationDeclined => format!(
            "The Windows permission prompt was declined, so RABBIT uninstalled nothing; {name} \
             is still installed."
        ),
        UninstallMessage::ElevationFailed { error } => format!(
            "RABBIT couldn't get administrator rights to uninstall {name}, so it uninstalled \
             nothing: {error}"
        ),
        UninstallMessage::MovedToTrash { trashed_to } => {
            let mut message = match trashed_to {
                Some(path) => format!(
                    "RABBIT moved {name} to the Trash, as {}. Until you empty the Trash, you can \
                     drag it back.",
                    path.display()
                ),
                None => format!(
                    "RABBIT moved {name} to the Trash. Until you empty the Trash, you can drag it \
                     back."
                ),
            };
            if plan
                .item(&item.package_id)
                .is_some_and(UninstallPlanItem::needs_administrator)
            {
                // The move keeps the owner it had, which RABBIT never
                // checked: root, another account, or this one.
                message.push_str(
                    " It keeps the owner it had in Applications, which may not be you, so \
                     emptying the Trash may ask for an administrator's password, and so may \
                     dragging it back to Applications. Finder offers no Put Back for it.",
                );
            }
            message
        }
        UninstallMessage::TrashFailed {
            error,
            refused_by_macos,
        } => {
            let mut message = format!(
                "RABBIT couldn't move {name} to the Trash, so nothing of it changed: {}",
                closed_sentence(error)
            );
            if *refused_by_macos {
                message.push(' ');
                message.push_str(APP_MANAGEMENT_HINT);
            }
            message
        }
        UninstallMessage::TrashUnclear { path } => format!(
            "RABBIT asked macOS to move {} to the Trash, and can't confirm that it is there now. \
             Look for {name} in the Trash and in its folder in Finder. RABBIT stopped there.",
            path.display()
        ),
        UninstallMessage::RemovedAsAdministrator => format!(
            "RABBIT removed {name} with administrator rights. A backup can't bring it back; to \
             have it again, install it again."
        ),
        UninstallMessage::AdministratorStepFailed {
            path,
            ended_unclearly,
            gone,
            left,
            refused_by_macos,
            detail,
        } => {
            // `rm -R -f` deletes all it can inside a tree before it fails on
            // a folder, so the tree the command stopped at while deleting it
            // for good may be half gone, though it is still there. REAPER's
            // move is one rename: its app went or it didn't.
            let partly_there = gone.is_empty()
                && path.is_some()
                && plan.item(&item.package_id).is_some_and(|planned| {
                    planned.needs_administrator() && planned.backup != UninstallBackup::ToTrash
                });
            // Where it stopped only when its step was this package's, and
            // "without saying" only when the command didn't say: otherwise it
            // stopped at another package's step, or ran to its end, and this
            // is still there all the same.
            let mut message = match (path, *ended_unclearly, left.first()) {
                (Some(path), _, _) if partly_there => format!(
                    "The administrator command stopped at {}, which may now be only partly \
                     there. Nothing else of {name} was removed.",
                    path.display()
                ),
                (Some(path), _, _) => format!(
                    "The administrator command stopped at {}, so {name} isn't fully removed.",
                    path.display()
                ),
                (None, true, _) => format!(
                    "The administrator command ended without saying where it stopped, and \
                     {name} isn't fully removed."
                ),
                (None, false, Some(first)) => format!(
                    "The administrator command ran, but {} is still there, or RABBIT can't see \
                     that it is gone, so {name} isn't fully removed.",
                    first.display()
                ),
                (None, false, None) => {
                    format!("The administrator command ran, and {name} isn't fully removed.")
                }
            };
            if !gone.is_empty() {
                message.push_str(&format!(" Removed: {}.", path_list(gone)));
            } else if !partly_there {
                message.push_str(&format!(" Nothing of {name} was removed."));
            }
            if !left.is_empty() {
                message.push_str(&format!(" Still there: {}.", path_list(left)));
            }
            if let Some(detail) = detail {
                message.push_str(&format!(" macOS said: {}", closed_sentence(detail)));
            }
            if *refused_by_macos {
                message.push(' ');
                message.push_str(APP_MANAGEMENT_HINT);
            }
            message.push_str(" RABBIT stopped there.");
            message
        }
        UninstallMessage::ResourceFolderDamaged { missing } => format!(
            "IMPORTANT: after {name}'s own uninstaller ran, these files were gone from REAPER's \
             resource folder: {}. RABBIT stopped and wrote its receipt again. REAPER's settings \
             in reaper.ini can't be brought back by RABBIT; if you have a copy, put it back \
             before you start REAPER.",
            path_list(missing)
        ),
    };
    // Only a removal into the backup set can be put back; what a vendor's
    // uninstaller, the Trash or the administrator command did stays done,
    // and their messages say what is where.
    if item.status == UninstallStatus::Failed
        && item.left_in_backup.is_empty()
        && matches!(
            item.message_code,
            UninstallMessage::MoveFailed { .. }
                | UninstallMessage::FileInUse { .. }
                | UninstallMessage::BackupFailed { .. }
                | UninstallMessage::ReceiptSaveFailed { .. }
                | UninstallMessage::PathRefused { .. }
                | UninstallMessage::Unreadable { .. }
                | UninstallMessage::CleanupRefused { .. }
        )
    {
        message.push_str(&format!(" Everything of {name} is back in place."));
    }
    if !item.left_in_backup.is_empty() {
        message.push_str(&format!(
            " These files couldn't be moved back and are still in the backup set, where \
             rabbit restore-backup finds them: {}.",
            path_list(&item.left_in_backup)
        ));
    }
    // What the elevated helper removed, or in a dry run would remove, one
    // sentence per step. A dry run asks no helper; it says what the plan
    // found.
    if item.status == UninstallStatus::WouldRemove {
        if let Some(planned) = plan
            .item(&item.package_id)
            .and_then(|planned| planned.machine_cleanup.as_ref())
        {
            push_planned_cleanup(&mut message, planned);
        }
    } else if let Some(cleanup) = &item.cleanup {
        push_cleanup_steps(&mut message, cleanup);
    }
    let changed: Vec<PathBuf> = item
        .kept
        .iter()
        .filter(|kept| kept.reason == KeptFileReason::ChangedSinceInstall)
        .map(|kept| kept.path.clone())
        .collect();
    if !changed.is_empty() {
        let verb = if item.status == UninstallStatus::WouldRemove {
            "would stay"
        } else {
            "stayed"
        };
        message.push_str(&format!(
            " These files {verb}, because they changed since RABBIT installed them: {}.",
            path_list(&changed)
        ));
    }
    let linked: Vec<PathBuf> = item
        .kept
        .iter()
        .filter(|kept| kept.reason == KeptFileReason::HoldsLink)
        .map(|kept| kept.path.clone())
        .collect();
    if !linked.is_empty() {
        message.push_str(&format!(
            " These settings folders stay whole, because they hold a link RABBIT never \
             follows: {}.",
            path_list(&linked)
        ));
    }
    let reapack: Vec<PathBuf> = item
        .kept
        .iter()
        .filter(|kept| kept.reason == KeptFileReason::InstalledByReaPack)
        .map(|kept| kept.path.clone())
        .collect();
    if !reapack.is_empty() {
        let verb = if item.status == UninstallStatus::WouldRemove {
            "would stay"
        } else {
            "stayed"
        };
        message.push_str(&format!(
            " These files {verb}, because ReaPack installed them and keeps track of them: {}.",
            path_list(&reapack)
        ));
    }
    if !item.leftovers.is_empty() {
        message.push_str(&format!(
            " RABBIT still finds some of {name}, and leaves it alone: {}.",
            path_list(&item.leftovers)
        ));
    }
    for entry in &item.registry_entries {
        let key = &entry.key;
        match &entry.action {
            RegistryEntryAction::Removed => message.push_str(&format!(
                " RABBIT also removed {key} from Windows' list of installed programs."
            )),
            RegistryEntryAction::WouldRemove => message.push_str(&format!(
                " RABBIT would also remove {key} from Windows' list of installed programs."
            )),
            RegistryEntryAction::PointsElsewhere {
                found: Some(found),
                expected,
            } => message.push_str(&format!(
                " {key} stays in Windows' list of installed programs: it points at {found}, \
                 not into {}.",
                expected.display()
            )),
            RegistryEntryAction::PointsElsewhere {
                found: None,
                expected,
            } => message.push_str(&format!(
                " {key} stays in Windows' list of installed programs: it doesn't say where its \
                 program is, so RABBIT can't tell that it points into {}.",
                expected.display()
            )),
            RegistryEntryAction::NotFound => {}
        }
    }
    for problem in &item.problems {
        message.push(' ');
        message.push_str(&problem.message());
    }
    message
}

/// One sentence for each thing a planned machine cleanup would remove, in
/// the order the helper removes them.
fn push_planned_cleanup(message: &mut String, planned: &PlannedMachineCleanup) {
    for file in &planned.files {
        message.push_str(&format!(
            " With administrator rights, RABBIT would also remove {}.",
            file.display()
        ));
    }
    if let Some(folder) = &planned.folder {
        message.push_str(&format!(
            " With administrator rights, RABBIT would also remove the folder {} once it is empty.",
            folder.display()
        ));
    }
    if let Some(key) = &planned.registry_key {
        message.push_str(&format!(
            " With administrator rights, RABBIT would also remove the registry entry {key}."
        ));
    }
    message.push_str(" Windows asks for permission first, and a backup can't bring these back.");
}

/// One sentence for each thing a machine cleanup removed, then what is
/// still there after one that failed. Something gone already before its
/// step isn't mentioned, and a step that failed is in the message itself.
fn push_cleanup_steps(message: &mut String, cleanup: &CleanupOutcome) {
    for step in &cleanup.steps {
        if step.result != CleanupResult::Removed {
            continue;
        }
        message.push_str(&match &step.target {
            CleanupTarget::File { path } => format!(
                " With administrator rights, RABBIT removed {}.",
                path.display()
            ),
            CleanupTarget::Folder { path } => format!(
                " With administrator rights, RABBIT removed the folder {}.",
                path.display()
            ),
            CleanupTarget::RegistryKey { key } => {
                format!(" With administrator rights, RABBIT removed the registry entry {key}.")
            }
        });
    }
    if cleanup.removed_anything() {
        message.push_str(" A backup can't bring these back.");
    }
    if cleanup.status == CleanupStatus::Failed && !cleanup.still_present.is_empty() {
        message.push_str(&format!(
            " Still there: {}.",
            cleanup.still_present.join(", ")
        ));
    }
}

fn path_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn uninstall_set_id() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("uninstall-unix-{seconds}")
}
