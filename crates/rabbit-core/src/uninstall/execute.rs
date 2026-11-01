//! Carrying out an uninstall: the `remove_files` engine.
//!
//! [`execute_uninstall`] takes the install lock, builds the plan again from
//! fresh detection, and runs the uninstall preflight. If any requested
//! package is blocked at that point, nothing changes at all. Otherwise it
//! removes the packages one at a time, every package before the packages it
//! depends on, each one like this:
//!
//! 1. Check the cancel token, and check again that nothing the package needs
//!    closed has started since.
//! 2. Record the package's receipt entry in the run's backup set.
//! 3. For each file: run the guard again, hash it, and move it into the
//!    backup set. A file only the receipt names that changed since RABBIT
//!    installed it stays; one the manifest names goes all the same.
//! 4. Remove the folders the plan says to remove once empty.
//! 5. Take the package out of the receipt and add it to `declined_packages`,
//!    so the next install run doesn't tick it again.
//! 6. Run the plan's post helpers, such as setting REAPER back to no
//!    language pack.
//!
//! When a step up to 5 fails, every file the package moved goes back where it
//! was and the receipt stays as it was: the receipt never claims files that
//! are gone, and never loses an entry for files that are still there. A
//! post helper that fails leaves the package removed, with a problem in the
//! report. Packages removed earlier in the run stay removed; each can be
//! restored from the backup set on its own.

use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cancel::CancelToken;
use crate::detection::{default_standard_installation, detect_components};
use crate::error::Result;
use crate::lock::acquire_package_install_lock;
use crate::model::{ComponentDetection, InstallationKind, Platform};
use crate::package::uninstall::UninstallPostStep;
use crate::package::{PackageSpec, builtin_package_specs};
use crate::preflight::{
    PreflightCheck, PreflightStatus, REAPER_PROCESS_CHECK, RunningProcess,
    ensure_resource_path_ready, portable_target_app_path, running_processes_named,
    running_reaper_processes,
};
use crate::progress::{ProgressEvent, ProgressReporter};
use crate::receipt::{
    InstallState, load_install_state, remove_package_receipt, save_install_state,
};
use crate::rollback::{BackupPackageRecord, BackupSetWriter, FileDigest, MoveFileError, MovedFile};

use super::paths::{UninstallRoots, absolute, same_ignoring_case};
use super::plan::{UninstallPlanInput, build_uninstall_plan};
use super::post::reset_reaper_langpack_if_selected;
use super::preflight::{
    ANY_REAPER_PROCESS_CHECK, BACKUP_SPACE_CHECK, PROGRAMS_RUNNING_CHECK, SystemVolumes,
    UninstallPreflightOptions, VolumeProbe, backup_space_check, backup_volume_folder,
    blocking_programs, crosses_volumes, process_checks, run_uninstall_preflight_with,
};
use super::{
    KeptFile, KeptFileReason, Removability, RemovalReason, RemovedFile, UninstallMessage,
    UninstallPlan, UninstallPlanItem, UninstallProblem, UninstallReport, UninstallReportItem,
    UninstallRequest, UninstallStatus, save_uninstall_report,
};

/// The reason an uninstall's backup set records.
pub const UNINSTALL_BACKUP_REASON: &str = "uninstall";

/// Uninstall `request.package_ids` from the REAPER at
/// `request.resource_path`, or with `request.dry_run` only say what would
/// happen. Runs on a worker thread: it hashes every file it removes.
///
/// Returns `Err` only for a hard error before anything changed, such as
/// another RABBIT holding the install lock or a receipt that can't be read.
/// Everything after that, failures included, is in the report. A run that
/// isn't a dry run saves its report under `RABBIT/logs`.
pub fn execute_uninstall(
    request: &UninstallRequest,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<UninstallReport> {
    let (target, target_app_path) = target_of(request);
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
        },
        progress,
        cancel,
    )
}

/// Everything [`execute_uninstall`] reads from the computer, so tests can
/// stand in their own packages, detection, processes and volumes.
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
}

/// Whether the request's resource folder belongs to the standard REAPER or a
/// portable one, and where the target's REAPER app is.
fn target_of(request: &UninstallRequest) -> (InstallationKind, Option<PathBuf>) {
    let standard = default_standard_installation(request.platform).filter(|installation| {
        same_ignoring_case(
            &absolute(&installation.resource_path),
            &absolute(&request.resource_path),
        )
    });
    let target = request.target.unwrap_or(if standard.is_some() {
        InstallationKind::Standard
    } else {
        InstallationKind::Portable
    });
    let app_path = request.target_app_path.clone().or_else(|| match target {
        InstallationKind::Standard => standard.map(|installation| installation.app_path),
        InstallationKind::Portable => {
            portable_target_app_path(&request.resource_path, Some(request.platform))
        }
    });
    (target, app_path)
}

pub(crate) fn execute_uninstall_in(
    request: &UninstallRequest,
    environment: &UninstallEnvironment<'_>,
    progress: &ProgressReporter,
    cancel: &CancelToken,
) -> Result<UninstallReport> {
    let resource = environment.roots.resource.clone();
    ensure_resource_path_ready(&resource, request.dry_run)?;
    // Held from before the plan is built until the report is written, so an
    // install can't change the files between the plan and the moves. A
    // resource folder that doesn't exist has nothing to remove, and taking
    // the lock would create it.
    let _install_lock = if request.dry_run || !resource.is_dir() {
        None
    } else {
        Some(acquire_package_install_lock(&resource)?)
    };

    let detections = (environment.detect)()?;
    let state = load_install_state(&resource)?;
    let plan = build_uninstall_plan(&UninstallPlanInput {
        roots: environment.roots,
        platform: request.platform,
        target: environment.target,
        specs: environment.specs,
        detections: &detections,
        state: state.as_ref(),
        remove_settings: request.remove_settings,
    });

    let mut requested: Vec<String> = Vec::new();
    for id in &request.package_ids {
        if !requested.contains(id) {
            requested.push(id.clone());
        }
    }
    let to_check: Vec<String> = requested
        .iter()
        .filter(|id| {
            plan.item(id)
                .is_some_and(|item| item.removability.is_removable() && !item.unbacked)
        })
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
    // ones still open are removed below, and the report keeps the order of
    // removal throughout.
    let mut slots: Vec<(&UninstallPlanItem, Option<UninstallReportItem>)> = Vec::new();
    for plan_item in plan.in_removal_order(&requested) {
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
        } else if plan_item.unbacked {
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
    if request.dry_run {
        for plan_item in to_remove {
            outcomes.push(survey(plan_item, environment, &backup_folder));
        }
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
        let mut run = RemovalRun {
            plan: &plan,
            environment,
            request,
            backup_folder: &backup_folder,
            backups: BackupSetWriter::new(&resource, &uninstall_set_id(), UNINSTALL_BACKUP_REASON),
            state: state.unwrap_or_default(),
            progress,
        };
        let mut stopped: Option<UninstallMessage> = None;
        for plan_item in to_remove {
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
                    let item = run.remove(plan_item);
                    if item.status == UninstallStatus::Blocked {
                        stopped = Some(UninstallMessage::RunStopped {
                            package_id: plan_item.package_id.clone(),
                        });
                    }
                    item
                }
            };
            outcomes.push(item);
        }
        if !run.backups.discard_if_empty() {
            backup_set = Some((run.backups.set_id(), run.backups.path().to_path_buf()));
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
        item.message = english_message(item);
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
        plan,
        saved_report: None,
    };
    // A report of what was removed is the user's way back to the backup
    // set, so every real run leaves one. A dry run writes nothing at all.
    if !request.dry_run && resource.is_dir() {
        report.saved_report = save_uninstall_report(&report).ok();
    }
    Ok(report)
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
    progress: &'a ProgressReporter,
}

impl RemovalRun<'_> {
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

        self.progress.report(ProgressEvent::RemoveStarted {
            package_id: id.clone(),
        });
        let item = self.remove_started(plan_item);
        self.progress.report(ProgressEvent::RemoveCompleted {
            package_id: id,
            removed: item.status.is_removed(),
        });
        item
    }

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
        );
        let mut outcome = removal.remove_files();
        if outcome.is_ok() {
            removal.remove_dirs_if_empty();
            outcome = save_receipt_without(&mut self.state, &self.plan.resource_path, id);
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
            return item;
        }
        let mut item = removal.into_item(
            UninstallStatus::Removed,
            UninstallMessage::Removed {
                backup_set_id: None,
            },
        );

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
            }
        }
        item
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
/// hashed as the real run would, and those the real run would keep.
fn survey(
    plan_item: &UninstallPlanItem,
    environment: &UninstallEnvironment<'_>,
    backup_folder: &Path,
) -> UninstallReportItem {
    let mut removal = PackageRemoval::new(plan_item, environment, backup_folder, None);
    match removal.remove_files() {
        Ok(()) => removal.into_item(UninstallStatus::WouldRemove, UninstallMessage::WouldRemove),
        // The real run would fail here before removing anything for good.
        Err(code) => removal.into_item(UninstallStatus::Blocked, code),
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
    ) -> Self {
        Self {
            item,
            roots: environment.roots,
            volumes: environment.volumes,
            backup_folder,
            backups,
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
            if file.is_dir {
                self.remove_folder(&file.path, file.reason)?;
            } else {
                self.remove_file(&file.path, file.reason, file.receipt_sha256.as_deref())?;
            }
        }
        Ok(())
    }

    fn remove_file(
        &mut self,
        path: &Path,
        reason: RemovalReason,
        receipt_sha256: Option<&str>,
    ) -> std::result::Result<(), UninstallMessage> {
        let digest = FileDigest::of(path).map_err(|error| UninstallMessage::Unreadable {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?;
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
        });
        Ok(())
    }

    /// A settings folder goes file by file, so the backup set lists each
    /// file and a restore can put each one back. A folder holding a link
    /// stays whole: RABBIT never follows one.
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

fn new_item(
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
        problems: Vec::new(),
        message: String::new(),
        message_code: code,
    }
}

/// The item's English message: what happened, then anything kept, left
/// over or gone wrong afterwards.
fn english_message(item: &UninstallReportItem) -> String {
    let name = &item.display_name;
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
        UninstallMessage::NotRemovable { removability } => {
            not_removable_message(name, removability)
        }
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
    };
    if item.status == UninstallStatus::Failed && item.left_in_backup.is_empty() {
        message.push_str(&format!(" Everything of {name} is back in place."));
    }
    if !item.left_in_backup.is_empty() {
        message.push_str(&format!(
            " These files couldn't be moved back and are still in the backup set, where \
             rabbit restore-backup finds them: {}.",
            path_list(&item.left_in_backup)
        ));
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
    if !item.leftovers.is_empty() {
        message.push_str(&format!(
            " RABBIT still finds some of {name}, and leaves it alone: {}.",
            path_list(&item.leftovers)
        ));
    }
    for problem in &item.problems {
        message.push(' ');
        message.push_str(&problem.message());
    }
    message
}

fn not_removable_message(name: &str, removability: &Removability) -> String {
    match removability {
        Removability::ManagedByReaPack {
            path,
            reapack_package,
        } => format!(
            "ReaPack manages {name}: it installed {} as {reapack_package}. Uninstall it in \
             ReaPack instead.",
            path.display()
        ),
        Removability::ReaPackCheckFailed { path, reason } => format!(
            "RABBIT couldn't check ReaPack's records for {} ({reason}), so it leaves {name} \
             alone.",
            path.display()
        ),
        Removability::NeedsStandardTarget => format!(
            "{name} serves every REAPER on this computer, so RABBIT removes it only from the \
             standard REAPER, not from a portable one."
        ),
        Removability::NoReceipt => format!(
            "RABBIT didn't install this copy of {name}, so it is most likely your own, and \
             RABBIT leaves it alone."
        ),
        Removability::NoTrustedUninstaller => {
            format!("RABBIT found no uninstaller for {name} it can trust.")
        }
        Removability::Manual { .. } => {
            format!("RABBIT can't remove {name} by itself; it has to be removed by hand.")
        }
        Removability::NotSupported { .. } => format!("RABBIT can't uninstall {name} yet."),
        Removability::UnsafePath { path, refusal } => format!(
            "RABBIT won't remove {}, because {refusal}. It leaves {name} alone rather than \
             remove only part of it.",
            path.display()
        ),
        Removability::Removable
        | Removability::RemovableOrphanedFromReaPack
        | Removability::NotInstalled => format!("RABBIT can remove {name}."),
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
