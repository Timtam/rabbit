//! Removing what a Windows installer leaves in machine-wide places to find
//! its own uninstaller: a folder below `{program_files_x86}` and an entry in
//! HKLM's 32-bit list of installed programs. The JAWS scripts' installer
//! leaves both, and its own uninstaller isn't one RABBIT runs.
//!
//! Only an administrator may delete either, so RABBIT's elevated helper
//! does, with the folder and the key from its own embedded manifest; RABBIT
//! never sends it a path. [`check_machine_cleanup`] checks everything first,
//! and a check that fails changes nothing:
//!
//! - the folder is exactly one folder below `{program_files_x86}`, as
//!   Windows names it, never from the environment or the registry; if it is
//!   there, it is reached through no link, an administrator owns it, and it
//!   holds nothing but the manifest's `files`, each a regular file an
//!   administrator owns;
//! - the key is read in HKLM's 32-bit view only; if it is there, it has no
//!   subkeys and holds only the manifest's values, each a plain `REG_SZ`
//!   naming exactly the path the manifest gives, and the value naming the
//!   folder is among them.
//!
//! There is no Authenticode check: the JAWS scripts' `Uninstall.exe` isn't
//! signed, and RABBIT never runs it. Ownership stands in, as it does for
//! vendor uninstallers.
//!
//! [`run_machine_cleanup`] checks again, then deletes the files in the
//! manifest's order, the folder once it is empty, and the key last, each
//! through [`VendorEnvironment`], and stops at the first failure. Right
//! before each file and the folder it checks once more that the folder is
//! reached through no link, and that an administrator still owns the file.
//! So the key outlives everything else, and a package whose cleanup failed
//! is still found through it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rabbit_platform::RegistryView;
use serde::{Deserialize, Serialize};

use crate::package::uninstall::{
    MachineCleanupSpec, RegistryHive, RegistryViewSpec, UninstallPath, UninstallPathToken,
};

use super::paths::{
    PathRefusal, RemovalGuard, UninstallRoots, check_removal, same_ignoring_case, strictly_inside,
};
use super::vendor::{
    VendorEnvironment, VendorRefusal, check_folder_holds_only, check_owner,
    check_reached_without_links, install_folder_from, resolve,
};

#[cfg(test)]
#[path = "cleanup_tests.rs"]
pub(crate) mod tests;

/// One machine cleanup: the package, its plan's `machine_cleanup`, and
/// what the plan's path tokens stand for.
#[derive(Debug, Clone, Copy)]
pub struct CleanupJob<'a> {
    pub package_id: &'a str,
    pub spec: &'a MachineCleanupSpec,
    /// In the elevated helper: the machine-wide folders it found itself.
    pub roots: &'a UninstallRoots,
}

/// What a machine cleanup would remove, as the plan sees it before the run.
/// The elevated helper checks everything again right before it deletes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMachineCleanup {
    /// The uninstaller folder, when it is there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
    /// The files in that folder, in the order they are deleted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<PathBuf>,
    /// The registry entry as regedit names it, when it is there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_key: Option<String>,
    /// The steps to remove the package by hand, for when RABBIT can't.
    pub by_hand_key: String,
}

/// What became of one machine cleanup. It crosses the pipe from the
/// elevated helper to RABBIT, so it holds plain data only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupOutcome {
    pub package_id: String,
    pub status: CleanupStatus,
    /// Why RABBIT deleted nothing, when a check failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<VendorRefusal>,
    /// Every delete RABBIT tried, in order. A failed one is the last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<CleanupStep>,
    /// The folder and the registry entry, as far as they are still there
    /// afterwards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub still_present: Vec<String>,
    /// What went wrong, in English.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CleanupOutcome {
    fn new(package_id: &str, status: CleanupStatus) -> Self {
        Self {
            package_id: package_id.to_string(),
            status,
            refusal: None,
            steps: Vec::new(),
            still_present: Vec::new(),
            error: None,
        }
    }

    /// Nothing was deleted, because `refusal`.
    pub fn refused(package_id: &str, refusal: VendorRefusal) -> Self {
        Self {
            refusal: Some(refusal),
            ..Self::new(package_id, CleanupStatus::Refused)
        }
    }

    /// RABBIT lost contact with the elevated helper, so what it deleted is
    /// unknown.
    pub fn lost(package_id: &str, error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::new(package_id, CleanupStatus::Lost)
        }
    }

    /// Whether any step removed something, which a backup can't bring back.
    pub fn removed_anything(&self) -> bool {
        any_removed(&self.steps)
    }
}

fn any_removed(steps: &[CleanupStep]) -> bool {
    steps
        .iter()
        .any(|step| step.result == CleanupResult::Removed)
}

/// How a machine cleanup ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CleanupStatus {
    /// Everything that was there is gone.
    Removed,
    /// Neither the folder nor the registry entry was there. Nothing
    /// changed.
    NothingThere,
    /// A check failed before anything was removed: one of the checks
    /// before the first delete, or one right before a delete. Nothing
    /// changed.
    Refused,
    /// A delete failed, a check right before one failed once something was
    /// removed, or something is still there afterwards. What `steps` lists
    /// as removed is gone.
    Failed,
    /// RABBIT lost contact with the elevated helper. What it deleted is
    /// unknown.
    Lost,
}

/// One delete of a machine cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupStep {
    pub target: CleanupTarget,
    pub result: CleanupResult,
}

/// What one step deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CleanupTarget {
    File {
        path: PathBuf,
    },
    Folder {
        path: PathBuf,
    },
    /// The registry entry, as regedit names it.
    RegistryKey {
        key: String,
    },
}

/// How one step went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CleanupResult {
    Removed,
    /// It was gone already, which counts as removed.
    NotFound,
    /// It is still there.
    Failed {
        error: String,
    },
}

/// What [`check_machine_cleanup`] found, with the folders the deletes need.
struct Checked {
    folder: PathBuf,
    program_files_x86: PathBuf,
    /// The files in the folder, in deletion order; `None` when the folder
    /// isn't there.
    files: Option<Vec<PathBuf>>,
    /// The registry entry as regedit names it, when it is there.
    registry_key: Option<String>,
}

/// Check `job`'s folder and registry entry as the elevated helper does
/// right before it deletes them, and say what it would remove: `None` when
/// neither is there. Reads only.
pub fn check_machine_cleanup(
    job: &CleanupJob<'_>,
    env: &dyn VendorEnvironment,
) -> Result<Option<PlannedMachineCleanup>, VendorRefusal> {
    let checked = check(job, env)?;
    if checked.files.is_none() && checked.registry_key.is_none() {
        return Ok(None);
    }
    Ok(Some(PlannedMachineCleanup {
        folder: checked.files.is_some().then_some(checked.folder),
        files: checked.files.unwrap_or_default(),
        registry_key: checked.registry_key,
        by_hand_key: job.spec.by_hand_key.clone(),
    }))
}

fn check(job: &CleanupJob<'_>, env: &dyn VendorEnvironment) -> Result<Checked, VendorRefusal> {
    let (folder, program_files_x86) = cleanup_folder(job)?;
    let registry_key = check_entry(job, env, &folder)?;
    let files = check_folder(job, env, &folder)?;
    Ok(Checked {
        folder,
        program_files_x86,
        files,
        registry_key,
    })
}

/// The cleanup's folder, which must be exactly one folder strictly below
/// `{program_files_x86}`, and that folder. Without `{program_files_x86}`,
/// as off Windows, there is no folder to check. Validation already demands
/// the rest, so failing that means a hand-made plan.
fn cleanup_folder(job: &CleanupJob<'_>) -> Result<(PathBuf, PathBuf), VendorRefusal> {
    let Some(program_files_x86) = job.roots.program_files_x86.clone() else {
        return Err(VendorRefusal::UnknownFolder {
            token: format!("{{{}}}", UninstallPathToken::ProgramFilesX86.name()),
        });
    };
    let folder = one_path(&job.spec.dir, job.roots)?;
    if !strictly_inside(&folder, &program_files_x86) {
        return Err(VendorRefusal::NotAVendorPlan);
    }
    Ok((folder, program_files_x86))
}

/// `path` resolved for these roots, which must give exactly one path.
fn one_path(path: &UninstallPath, roots: &UninstallRoots) -> Result<PathBuf, VendorRefusal> {
    let mut resolved = resolve(path, roots)?;
    match (resolved.pop(), resolved.is_empty()) {
        (Some(only), true) => Ok(only.path),
        _ => Err(VendorRefusal::NotAVendorPlan),
    }
}

/// The view a manifest's key lives in.
fn registry_view(view: RegistryViewSpec) -> RegistryView {
    match view {
        RegistryViewSpec::Bit32 => RegistryView::Bit32,
    }
}

/// Check the cleanup's registry entry in its one view of the environment's
/// list, which is HKLM's in the helper. `None` when it isn't there, its name
/// as regedit shows it when every check passes.
fn check_entry(
    job: &CleanupJob<'_>,
    env: &dyn VendorEnvironment,
    folder: &Path,
) -> Result<Option<String>, VendorRefusal> {
    let registry = &job.spec.registry;
    // Only HKLM's list is one only administrators can change.
    if registry.hive != RegistryHive::Hklm {
        return Err(VendorRefusal::NotAVendorPlan);
    }
    let views = env.uninstall_views();
    let view = registry_view(registry.view);
    let shown = views.display_key(&registry.key, view);
    let entry = views
        .read_entry_in_view(&registry.key, view)
        .map_err(|error| VendorRefusal::unreadable(&shown, &error))?;
    let Some(entry) = entry else {
        return Ok(None);
    };
    if entry.subkeys != 0 {
        return Err(VendorRefusal::RegistryKeyHasSubkeys);
    }
    let expected = registry
        .values
        .iter()
        .map(|(name, path)| Ok((name.as_str(), one_path(path, job.roots)?)))
        .collect::<Result<Vec<(&str, PathBuf)>, VendorRefusal>>()?;
    let expected_path = |name: &str| {
        expected
            .iter()
            .find(|(listed, _)| listed.eq_ignore_ascii_case(name))
            .map(|(_, path)| path)
    };
    let values = entry.values.values();
    if let Some(unexpected) = values
        .iter()
        .find(|value| expected_path(value.name()).is_none())
    {
        return Err(VendorRefusal::UnexpectedRegistryValue {
            value: unexpected.name().to_string(),
        });
    }

    // The value naming the folder must be there: the folder is checked
    // through it.
    let Some((anchor, _)) = expected
        .iter()
        .find(|(_, path)| same_ignoring_case(path, folder))
    else {
        return Err(VendorRefusal::NotAVendorPlan);
    };
    let anchor_there = values.iter().any(|value| {
        value.name().eq_ignore_ascii_case(anchor)
            && value.as_string().is_none_or(|text| !text.trim().is_empty())
    });
    if !anchor_there {
        return Err(VendorRefusal::InstallLocationMissing);
    }

    for value in values {
        // A value that holds no text names no path at all.
        let Some(text) = value.as_string() else {
            return Err(VendorRefusal::UnexpectedRegistryValue {
                value: value.name().to_string(),
            });
        };
        let mismatch = || VendorRefusal::RegistryValueMismatch {
            found: text.clone(),
        };
        // A REG_EXPAND_SZ value may name variables each reader expands its
        // own way.
        if !value.is_plain_string() {
            return Err(mismatch());
        }
        let path = install_folder_from(&text).map_err(|_| mismatch())?;
        if !expected_path(value.name()).is_some_and(|expected| same_ignoring_case(&path, expected))
        {
            return Err(mismatch());
        }
    }
    Ok(Some(shown))
}

/// Check the cleanup's folder. `None` when it isn't there; the files it
/// holds, in deletion order, when every check passes.
fn check_folder(
    job: &CleanupJob<'_>,
    env: &dyn VendorEnvironment,
    folder: &Path,
) -> Result<Option<Vec<PathBuf>>, VendorRefusal> {
    let metadata = match fs::symlink_metadata(folder) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(VendorRefusal::unreadable(folder.display(), &error)),
    };
    check_reached_without_links(folder)?;
    if !metadata.is_dir() {
        return Err(VendorRefusal::WrongFolder {
            path: folder.to_path_buf(),
        });
    }
    // Whoever owns the folder may put anything in it later.
    match env.owner_is_trusted(folder) {
        Ok(true) => {}
        Ok(false) => {
            return Err(VendorRefusal::UntrustedFolder {
                path: folder.to_path_buf(),
            });
        }
        Err(error) => return Err(VendorRefusal::unreadable(folder.display(), &error)),
    }
    check_folder_holds_only(env, folder, &job.spec.files).map(Some)
}

/// Check `job`'s folder and registry entry again, then delete them through
/// `env`: the files in the manifest's order, the folder once it is empty,
/// and the registry entry last. Stops at the first delete that fails, so the
/// entry outlives the rest. Something already gone counts as removed.
///
/// A check that refuses while nothing is removed yet, before the first
/// delete or right before one, gives `Refused`: nothing changed. In this
/// crate's own tests,
/// nothing is deleted from Windows' real list, whatever environment a test
/// hands in. Other crates' tests must go through
/// [`super::helper::InProcessElevatedHelper`], which never cleans up HKLM's
/// list or folders outside the temporary folder.
pub fn run_machine_cleanup(job: &CleanupJob<'_>, env: &dyn VendorEnvironment) -> CleanupOutcome {
    if cfg!(test) && env.uninstall_views().is_local_machine() {
        return CleanupOutcome {
            error: Some("tests never clean up Windows' real list of installed programs".into()),
            ..CleanupOutcome::refused(job.package_id, VendorRefusal::Unsupported)
        };
    }
    if !cfg!(windows) {
        return CleanupOutcome::refused(job.package_id, VendorRefusal::Unsupported);
    }
    let checked = match check(job, env) {
        Ok(checked) if checked.files.is_none() && checked.registry_key.is_none() => {
            return CleanupOutcome::new(job.package_id, CleanupStatus::NothingThere);
        }
        Ok(checked) => checked,
        Err(refusal) => return CleanupOutcome::refused(job.package_id, refusal),
    };
    let mut outcome = CleanupOutcome::new(job.package_id, CleanupStatus::Removed);
    let failed = match delete_checked(job, env, &checked, &mut outcome.steps) {
        Ok(()) => None,
        Err(Stopped::Refused(refusal)) => {
            return CleanupOutcome::refused(job.package_id, refusal);
        }
        Err(Stopped::Failed(error)) => Some(error),
    };
    let (still_present, unverified) = still_there(job, env, &checked.folder);
    outcome.still_present = still_present;
    if let Some(error) = failed {
        outcome.status = CleanupStatus::Failed;
        outcome.error = Some(error);
    } else if !outcome.still_present.is_empty() {
        outcome.status = CleanupStatus::Failed;
        outcome.error = Some(unverified.unwrap_or_else(|| {
            format!(
                "still there afterwards: {}",
                outcome.still_present.join(", ")
            )
        }));
    }
    outcome
}

/// Why the deletes stopped.
enum Stopped {
    /// A check right before a delete refused while nothing was removed yet,
    /// so nothing changed.
    Refused(VendorRefusal),
    /// A delete failed, or a check right before one refused once something
    /// was removed: what failed, and why.
    Failed(String),
}

/// Delete what `checked` found, in order, recording each step. Stops at the
/// first failure.
fn delete_checked(
    job: &CleanupJob<'_>,
    env: &dyn VendorEnvironment,
    checked: &Checked,
    steps: &mut Vec<CleanupStep>,
) -> Result<(), Stopped> {
    if let Some(files) = &checked.files {
        let folder = &checked.folder;
        for file in files {
            let result = delete_file(job.roots, env, folder, file);
            record(steps, CleanupTarget::File { path: file.clone() }, result)?;
        }
        let result = delete_folder(job.roots, env, &checked.program_files_x86, folder);
        record(
            steps,
            CleanupTarget::Folder {
                path: folder.clone(),
            },
            result,
        )?;
    }
    if let Some(shown) = &checked.registry_key {
        let registry = &job.spec.registry;
        let result = match env.delete_uninstall_entry(&registry.key, registry_view(registry.view)) {
            Ok(true) => CleanupResult::Removed,
            Ok(false) => CleanupResult::NotFound,
            Err(error) => CleanupResult::Failed {
                error: error.to_string(),
            },
        };
        record(
            steps,
            CleanupTarget::RegistryKey { key: shown.clone() },
            Ok(result),
        )?;
    }
    Ok(())
}

/// Add one step, and turn a failed one into what stops the run. A check
/// right before the delete that refused stops it as a refusal while nothing
/// is removed yet, and adds no step; once something is removed, it is a
/// failed step like any other.
fn record(
    steps: &mut Vec<CleanupStep>,
    target: CleanupTarget,
    result: Result<CleanupResult, VendorRefusal>,
) -> Result<(), Stopped> {
    let result = match result {
        Ok(result) => result,
        Err(refusal) if !any_removed(steps) => return Err(Stopped::Refused(refusal)),
        Err(refusal) => CleanupResult::Failed {
            error: format!("RABBIT won't remove it: {refusal}"),
        },
    };
    let failed = match (&target, &result) {
        (_, CleanupResult::Removed | CleanupResult::NotFound) => None,
        (
            CleanupTarget::File { path } | CleanupTarget::Folder { path },
            CleanupResult::Failed { error },
        ) => Some(format!("{}: {error}", path.display())),
        (CleanupTarget::RegistryKey { key }, CleanupResult::Failed { error }) => {
            Some(format!("{key}: {error}"))
        }
    };
    steps.push(CleanupStep { target, result });
    failed.map_or(Ok(()), |error| Err(Stopped::Failed(error)))
}

/// Delete one of the folder's files, once the folder is still reached
/// through no link, the guard passes, and an administrator still owns the
/// file. `Err` when one of these checks refuses: something changed since the
/// checks before the first delete.
fn delete_file(
    roots: &UninstallRoots,
    env: &dyn VendorEnvironment,
    folder: &Path,
    file: &Path,
) -> Result<CleanupResult, VendorRefusal> {
    if is_gone(file) {
        return Ok(CleanupResult::NotFound);
    }
    // The guard compares the file's folder with the folder by their real
    // paths, so a folder turned into a junction since the checks would
    // pass it. The delete follows links above the file, so this must catch
    // one.
    check_reached_without_links(folder)?;
    check_removal(roots, file, &RemovalGuard::under(folder)).map_err(|refusal| match refusal {
        PathRefusal::Link => VendorRefusal::FolderOrLink {
            path: file.to_path_buf(),
        },
        _ => VendorRefusal::WrongFolder {
            path: file.to_path_buf(),
        },
    })?;
    check_owner(env, file)?;
    Ok(removed(env.remove_file_no_follow(file)))
}

/// Delete the emptied folder, once it is still reached through no link and
/// the guard passes. `Err` when one of these checks refuses.
fn delete_folder(
    roots: &UninstallRoots,
    env: &dyn VendorEnvironment,
    program_files_x86: &Path,
    folder: &Path,
) -> Result<CleanupResult, VendorRefusal> {
    if is_gone(folder) {
        return Ok(CleanupResult::NotFound);
    }
    check_reached_without_links(folder)?;
    check_removal(roots, folder, &RemovalGuard::under(program_files_x86)).map_err(|_| {
        VendorRefusal::WrongFolder {
            path: folder.to_path_buf(),
        }
    })?;
    Ok(removed(env.remove_empty_dir(folder)))
}

fn is_gone(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(error) if error.kind() == io::ErrorKind::NotFound)
}

fn removed(result: io::Result<()>) -> CleanupResult {
    match result {
        Ok(()) => CleanupResult::Removed,
        Err(error) if error.kind() == io::ErrorKind::NotFound => CleanupResult::NotFound,
        Err(error) => CleanupResult::Failed {
            error: error.to_string(),
        },
    }
}

/// The folder and the registry entry, as far as they are still there, and
/// why RABBIT couldn't tell for the entry, if it couldn't. An entry whose
/// list can't be read counts as still there.
fn still_there(
    job: &CleanupJob<'_>,
    env: &dyn VendorEnvironment,
    folder: &Path,
) -> (Vec<String>, Option<String>) {
    let mut left = Vec::new();
    let mut unverified = None;
    if !is_gone(folder) {
        left.push(folder.display().to_string());
    }
    let registry = &job.spec.registry;
    let view = registry_view(registry.view);
    let views = env.uninstall_views();
    let shown = views.display_key(&registry.key, view);
    match views.views_with_key(&registry.key) {
        Ok(found) if !found.contains(&view) => {}
        Ok(_) => left.push(shown),
        Err(error) => {
            unverified = Some(format!(
                "RABBIT couldn't check whether {shown} is gone: {error}"
            ));
            left.push(shown);
        }
    }
    (left, unverified)
}
