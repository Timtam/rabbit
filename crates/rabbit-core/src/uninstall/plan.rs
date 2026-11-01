//! Building an uninstall plan: which packages can come off, and exactly what
//! would go and what would stay.
//!
//! Candidates come from three places, as the package's plan allows: receipt
//! entries with a hash, files whose names match the package's plug-in
//! prefixes and suffixes, and paths the manifest names. Then everything that
//! must stay is taken out, every remaining path passes the guard, and
//! ReaPack's records get their say. A path the guard refuses blocks the whole
//! package rather than leaving it half removed.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::detection::user_plugin_name_matches;
use crate::model::{ComponentDetection, InstallationKind, Platform};
use crate::package::uninstall::{
    ReapackOwnedPolicy, UninstallBackup, UninstallElevation, UninstallPath, UninstallPathToken,
    UninstallPlanSpec, UninstallStrategy,
};
use crate::package::{InstallDestination, PACKAGE_REAPACK, PackageSpec};
use crate::reapack::{ReapackOwnership, reapack_ownership, reapack_packages_with_files_under};
use crate::receipt::{InstallState, PackageReceipt};

use super::paths::{
    RemovalGuard, ResolvedPath, UninstallRoots, check_removal, expand_wildcard, inside_or_same,
    is_in_rabbit_folder, resolve_path, same_ignoring_case,
};
use super::{
    KeptPath, KeptReason, PathRefusal, PlannedRemoval, Removability, RemovalReason,
    UNINSTALL_NOT_YET_KEY, UninstallNote, UninstallPlan, UninstallPlanItem, UninstallWarning,
    resource_relative,
};

/// Everything [`build_uninstall_plan`] needs.
pub struct UninstallPlanInput<'a> {
    /// What each path token stands for; holds the resource folder.
    pub roots: &'a UninstallRoots,
    pub platform: Platform,
    pub target: InstallationKind,
    pub specs: &'a [PackageSpec],
    /// Fresh detection for the target, one per package.
    pub detections: &'a [ComponentDetection],
    /// The target's receipt, if it has one.
    pub state: Option<&'a InstallState>,
    /// "Also remove settings": each package's `kept_user_data` goes too.
    pub remove_settings: bool,
}

/// Build the plan for every package offered on `input.platform`. Reads
/// which files exist and how big they are, and ReaPack's records; hashes
/// nothing and changes nothing.
pub fn build_uninstall_plan(input: &UninstallPlanInput<'_>) -> UninstallPlan {
    // Without a detection for ReaPack, RABBIT can't tell that it is gone, so
    // it counts as installed and its records keep their hold.
    let reapack_installed = input
        .detections
        .iter()
        .find(|detection| detection.package_id == PACKAGE_REAPACK)
        .is_none_or(|detection| detection.installed);
    let items = input
        .specs
        .iter()
        .filter(|spec| spec.supports_platform(input.platform))
        .map(|spec| plan_item(input, spec, reapack_installed))
        .collect();
    UninstallPlan {
        resource_path: input.roots.resource.clone(),
        platform: input.platform,
        target: input.target,
        remove_settings: input.remove_settings,
        items,
    }
}

fn is_installed(input: &UninstallPlanInput<'_>, package_id: &str) -> bool {
    input
        .detections
        .iter()
        .any(|detection| detection.package_id == package_id && detection.installed)
}

fn plan_item(
    input: &UninstallPlanInput<'_>,
    spec: &PackageSpec,
    reapack_installed: bool,
) -> UninstallPlanItem {
    let detection = input
        .detections
        .iter()
        .find(|detection| detection.package_id == spec.id);
    let receipt = input.state.and_then(|state| state.packages.get(&spec.id));
    // A machine-wide package serves every REAPER on the computer, so a
    // portable target can't remove it: the standard REAPER would lose it
    // too. Manifests needn't even have a plan for that case.
    let needs_standard_target =
        spec.requires_standard_install && input.target == InstallationKind::Portable;
    let plan = if needs_standard_target {
        None
    } else {
        spec.uninstall.plan_for(input.platform, input.target)
    };
    let strategy = plan.map(|plan| plan.strategy);
    let changes_something = matches!(
        strategy,
        Some(UninstallStrategy::RemoveFiles | UninstallStrategy::VendorUninstaller)
    );
    let requires_elevation = changes_something
        && plan.is_some_and(|plan| match plan.elevation {
            UninstallElevation::Never => false,
            UninstallElevation::Required => true,
            // The ownership check that decides this for REAPER.app on macOS
            // comes with that removal. Until then the plan asks for the
            // prompt it may need: an unneeded permission note is harmless,
            // a missing one isn't.
            UninstallElevation::IfNeeded => true,
        });
    let unbacked = changes_something
        && plan.is_some_and(|plan| {
            plan.strategy == UninstallStrategy::VendorUninstaller
                || requires_elevation
                || !plan.remove.trees.is_empty()
                || plan.backup != UninstallBackup::Move
        });

    let mut item = UninstallPlanItem {
        package_id: spec.id.clone(),
        display_name: spec.display_name.clone(),
        installed_version: detection.and_then(|detection| detection.version.clone()),
        has_receipt: receipt.is_some(),
        depends_on: spec.depends_on.clone(),
        removability: Removability::NotInstalled,
        strategy,
        backup: plan.map(|plan| plan.backup).unwrap_or_default(),
        summary: plan.and_then(|plan| plan.summary.clone()),
        files: Vec::new(),
        dirs_if_empty: Vec::new(),
        kept: Vec::new(),
        post: plan.map(|plan| plan.post.clone()).unwrap_or_default(),
        blocking_programs: plan
            .map(|plan| plan.blocking_programs.clone())
            .unwrap_or_default(),
        // The engine adds this itself, so a manifest can't forget it: a
        // vendor uninstaller reacts to any REAPER window, and a machine-wide
        // package may be loaded by any REAPER.
        every_reaper_closed: spec.requires_standard_install
            || strategy == Some(UninstallStrategy::VendorUninstaller),
        requires_elevation,
        unbacked,
        confirm: false,
        warnings: package_warnings(input, spec),
        notes: Vec::new(),
    };
    item.confirm = item.unbacked
        || item
            .warnings
            .iter()
            .any(|warning| matches!(warning, UninstallWarning::Always { confirm: true, .. }));

    if !detection.is_some_and(|detection| detection.installed) {
        return item;
    }
    if needs_standard_target {
        item.removability = Removability::NeedsStandardTarget;
        return item;
    }
    let Some(plan) = plan else {
        item.removability = Removability::NotSupported {
            note_key: UNINSTALL_NOT_YET_KEY.to_string(),
        };
        return item;
    };
    let note_key = || {
        plan.note_key
            .clone()
            .unwrap_or_else(|| UNINSTALL_NOT_YET_KEY.to_string())
    };
    match plan.strategy {
        UninstallStrategy::Manual => {
            item.removability = Removability::Manual {
                note_key: note_key(),
            };
            return item;
        }
        UninstallStrategy::NotSupported => {
            item.removability = Removability::NotSupported {
                note_key: note_key(),
            };
            return item;
        }
        UninstallStrategy::RemoveFiles | UninstallStrategy::VendorUninstaller => {}
    }
    if spec.uninstall.requires_receipt && receipt.is_none() {
        item.removability = Removability::NoReceipt;
        return item;
    }

    let planner = PackagePlanner {
        roots: input.roots,
        remove_settings: input.remove_settings,
        spec,
        plan,
        receipt,
    };
    if plan.strategy == UninstallStrategy::VendorUninstaller {
        // Locating a vendor uninstaller and deciding whether to trust it
        // (HKLM only, exact folders, signatures) is the vendor runner's job,
        // which doesn't exist yet. Until it does, no uninstaller is trusted.
        item.kept = planner.user_data(false).1;
        item.removability = Removability::NoTrustedUninstaller;
        return item;
    }

    let collected = match planner.collect() {
        Ok(collected) => collected,
        Err(blocked) => {
            item.removability = blocked;
            item.kept = planner.user_data(false).1;
            return item;
        }
    };
    let orphaned_from = match planner.reapack_verdict(&collected.files, reapack_installed) {
        Ok(orphaned_from) => orphaned_from,
        Err(blocked) => {
            item.removability = blocked;
            item.kept = planner.user_data(false).1;
            item.kept.extend(collected.kept);
            return item;
        }
    };
    // Only now is the package known to be removable, so only now may "Also
    // remove settings" take its data. ReaPack's records aren't asked about
    // the settings: they are the user's, never something ReaPack installed.
    let (settings, user_data) = planner.user_data(input.remove_settings);
    item.kept = user_data;
    item.kept.extend(collected.kept);
    if spec.id == PACKAGE_REAPACK {
        item.warnings
            .extend(reapack_still_manages(input.roots, &collected.files));
    }
    item.removability = match orphaned_from {
        Some(_) => Removability::RemovableOrphanedFromReaPack,
        None => Removability::Removable,
    };
    if receipt.is_none() {
        item.notes.push(UninstallNote::NoReceipt);
    }
    if let Some(reapack_packages) = orphaned_from {
        item.notes
            .push(UninstallNote::OrphanedFromReaPack { reapack_packages });
    }
    if let Some(note_key) = &plan.note_key {
        item.notes.push(UninstallNote::Manifest {
            note_key: note_key.clone(),
        });
    }
    item.files = collected.files;
    item.files.extend(settings);
    item.dirs_if_empty = collected.dirs_if_empty;
    item
}

/// The warnings that don't depend on which files would go.
fn package_warnings(input: &UninstallPlanInput<'_>, spec: &PackageSpec) -> Vec<UninstallWarning> {
    let mut warnings: Vec<UninstallWarning> = spec
        .uninstall
        .always_warn
        .iter()
        .map(|warning| UninstallWarning::Always {
            message_key: warning.message_key.clone(),
            confirm: warning.confirm,
        })
        .collect();
    warnings.extend(
        spec.uninstall
            .warn_when_installed
            .iter()
            .filter(|warning| is_installed(input, &warning.package))
            .map(|warning| UninstallWarning::OtherPackageInstalled {
                package_id: warning.package.clone(),
                message_key: warning.message_key.clone(),
            }),
    );
    // Removing a package never removes what installs on top of it: REAPER's
    // extensions stay in the resource folder, and work again once REAPER is
    // back. The user is told they stay.
    let dependents: Vec<String> = input
        .specs
        .iter()
        .filter(|other| other.depends_on.contains(&spec.id) && is_installed(input, &other.id))
        .map(|other| other.id.clone())
        .collect();
    if !dependents.is_empty() {
        warnings.push(UninstallWarning::DependentsStay {
            package_ids: dependents,
        });
    }
    if spec.requires_standard_install {
        warnings.push(UninstallWarning::MachineWide);
    }
    warnings
}

/// Before ReaPack goes: the packages it installed that still have files in
/// UserPlugins. Nothing keeps them up to date afterwards. ReaPack's own entry
/// is left out, recognised by a file this plan removes.
fn reapack_still_manages(
    roots: &UninstallRoots,
    own_files: &[PlannedRemoval],
) -> Option<UninstallWarning> {
    let own: Vec<String> = own_files
        .iter()
        .filter_map(|file| resource_relative(&roots.resource, &file.path))
        .map(|path| path.to_lowercase())
        .collect();
    match reapack_packages_with_files_under(&roots.resource, "UserPlugins") {
        Ok(packages) => {
            let reapack_packages: Vec<String> = packages
                .into_iter()
                .filter(|package| {
                    !package
                        .files
                        .iter()
                        .any(|file| own.contains(&file.to_lowercase()))
                })
                .map(|package| package.package)
                .collect();
            (!reapack_packages.is_empty())
                .then_some(UninstallWarning::ReaPackStillManages { reapack_packages })
        }
        Err(error) => Some(UninstallWarning::ReaPackRecordsUnreadable {
            reason: error.to_string(),
        }),
    }
}

/// What a `remove_files` plan would take and leave for one package, apart
/// from its `kept_user_data`.
struct Collected {
    files: Vec<PlannedRemoval>,
    dirs_if_empty: Vec<PathBuf>,
    /// Candidates that stay, and why.
    kept: Vec<KeptPath>,
}

struct PackagePlanner<'p> {
    roots: &'p UninstallRoots,
    remove_settings: bool,
    spec: &'p PackageSpec,
    plan: &'p UninstallPlanSpec,
    receipt: Option<&'p PackageReceipt>,
}

impl PackagePlanner<'_> {
    /// Collect the candidates, take out what stays, and guard the rest.
    fn collect(&self) -> Result<Collected, Removability> {
        let keep = self.resolve_all(&self.plan.keep)?;
        let user_data: Vec<ResolvedPath> = self
            .plan
            .kept_user_data
            .iter()
            .filter_map(|kept| resolve_path(&kept.path, self.roots).ok())
            .flatten()
            .collect();

        let mut candidates: Vec<PlannedRemoval> = Vec::new();
        let mut kept: Vec<KeptPath> = Vec::new();
        self.add_manifest_paths(&mut candidates)?;
        self.add_matching_user_plugins(&mut candidates)?;
        self.add_trees(&mut candidates)?;
        self.add_receipt_files(&mut candidates, &mut kept);

        let mut files = Vec::new();
        for candidate in candidates {
            let reason = if is_in_rabbit_folder(self.roots, &candidate.path) {
                Some(KeptReason::RabbitFolder)
            } else if is_reaper_configuration(self.roots, &candidate.path) {
                Some(KeptReason::ReaperConfiguration)
            } else if matches_any(&candidate.path, &keep) {
                Some(KeptReason::KeepList)
            } else {
                None
            };
            if let Some(reason) = reason {
                kept.push(KeptPath {
                    path: candidate.path,
                    reason,
                    note_key: None,
                    exists: true,
                    removable_with_settings: false,
                });
                continue;
            }
            // A file only the receipt names, inside the package's own data,
            // stays with that data unless "Also remove settings" was asked
            // for; its kept_user_data entry already says so. A file the
            // manifest names there is the manifest's explicit word, and goes.
            if !self.remove_settings
                && !candidate.reason.named_by_manifest()
                && matches_any(&candidate.path, &user_data)
            {
                continue;
            }
            candidate
                .check(self.roots)
                .map_err(|refusal| Removability::UnsafePath {
                    path: candidate.path.clone(),
                    refusal,
                })?;
            files.push(candidate);
        }

        Ok(Collected {
            files,
            dirs_if_empty: self.dirs_if_empty()?,
            kept,
        })
    }

    /// The package's `kept_user_data`, split into what "Also remove
    /// settings" removes and what is listed as kept. With `take_settings`,
    /// every entry the option may take is in the first list only; without
    /// it, all are in the second, those the option could take marked so. A
    /// package that can't be removed passes `false`: nothing of it goes.
    /// Entries whose token can't be resolved here are left out: RABBIT can't
    /// say where they are.
    fn user_data(&self, take_settings: bool) -> (Vec<PlannedRemoval>, Vec<KeptPath>) {
        // A keep entry RABBIT can't place can't match anything it can place
        // either, so it is skipped here; `collect` refuses the package for
        // it before any setting could go.
        let keep: Vec<ResolvedPath> = self
            .plan
            .keep
            .iter()
            .filter_map(|path| resolve_path(path, self.roots).ok())
            .flatten()
            .collect();
        let mut settings = Vec::new();
        let mut kept = Vec::new();
        for entry in &self.plan.kept_user_data {
            let Ok(resolved) = resolve_path(&entry.path, self.roots) else {
                continue;
            };
            for resolved in resolved {
                let metadata = fs::symlink_metadata(&resolved.path).ok();
                // Some things stay even with the option: a
                // folder root such as REAPER's resource folder, REAPER's own
                // configuration, key maps, anything the plan keeps, and
                // RABBIT's folder. The manifests leave them out of
                // kept_user_data; this holds even if one doesn't.
                let reason = if resolved.is_root {
                    KeptReason::FolderRoot
                } else if is_in_rabbit_folder(self.roots, &resolved.path) {
                    KeptReason::RabbitFolder
                } else if is_reaper_configuration(self.roots, &resolved.path) {
                    KeptReason::ReaperConfiguration
                } else if matches_any(&resolved.path, &keep) {
                    KeptReason::KeepList
                } else if holds_key_maps(self.roots, &resolved.path) {
                    KeptReason::KeyMap
                } else {
                    KeptReason::UserData
                };
                let removable = reason == KeptReason::UserData
                    && metadata.is_some()
                    && check_removal(self.roots, &resolved.path, &resolved.guard).is_ok();
                if removable && take_settings {
                    let is_dir = metadata.as_ref().is_some_and(fs::Metadata::is_dir);
                    settings.push(PlannedRemoval {
                        path: resolved.path,
                        reason: RemovalReason::Setting,
                        size: metadata
                            .as_ref()
                            .filter(|metadata| metadata.is_file())
                            .map(fs::Metadata::len),
                        is_dir,
                        receipt_sha256: None,
                        guard: resolved.guard,
                    });
                } else {
                    kept.push(KeptPath {
                        path: resolved.path,
                        reason,
                        note_key: Some(entry.note_key.clone()),
                        exists: metadata.is_some(),
                        removable_with_settings: removable,
                    });
                }
            }
        }
        (settings, kept)
    }

    /// Resolve manifest paths, refusing the package when one can't be: a
    /// path RABBIT can't place is a path it can't check.
    fn resolve_all(&self, paths: &[UninstallPath]) -> Result<Vec<ResolvedPath>, Removability> {
        let mut resolved = Vec::new();
        for path in paths {
            resolved.extend(self.resolve(path)?);
        }
        Ok(resolved)
    }

    fn resolve(&self, path: &UninstallPath) -> Result<Vec<ResolvedPath>, Removability> {
        resolve_path(path, self.roots).map_err(|refusal| Removability::UnsafePath {
            path: PathBuf::from(path.as_str()),
            refusal,
        })
    }

    fn add_manifest_paths(&self, candidates: &mut Vec<PlannedRemoval>) -> Result<(), Removability> {
        for path in &self.plan.remove.paths {
            for resolved in self.resolve(path)? {
                let found = if resolved.wildcard {
                    expand_wildcard(&resolved)
                        .map_err(|error| unreadable(&resolved.path, &error))?
                } else {
                    vec![resolved.path.clone()]
                };
                for path in found {
                    add_file(
                        candidates,
                        path,
                        RemovalReason::ManifestPath,
                        &resolved.guard,
                    );
                }
            }
        }
        Ok(())
    }

    /// The same scan detection runs: files in the package's install folder
    /// whose names match its plug-in prefixes and suffixes. Links are listed
    /// too, so the guard refuses them rather than the scan passing them by.
    fn add_matching_user_plugins(
        &self,
        candidates: &mut Vec<PlannedRemoval>,
    ) -> Result<(), Removability> {
        if !self.plan.remove.matching_user_plugins {
            return Ok(());
        }
        let (token, folder) = match self.spec.install_destination {
            InstallDestination::UserPlugins => (UninstallPathToken::Resource, Some("UserPlugins")),
            InstallDestination::LangPack => (UninstallPathToken::Resource, Some("LangPack")),
            InstallDestination::WindowsClapDir => (UninstallPathToken::ClapDir, None),
        };
        let roots = self.roots.resolve(token);
        if roots.is_empty() {
            return Err(Removability::UnsafePath {
                path: PathBuf::from(format!("{{{}}}", token.name())),
                refusal: PathRefusal::UnknownRoot {
                    token: format!("{{{}}}", token.name()),
                },
            });
        }
        for root in roots {
            let scan_dir = folder.map_or_else(|| root.clone(), |folder| root.join(folder));
            let entries = match fs::read_dir(&scan_dir) {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(unreadable(&scan_dir, &error)),
            };
            let mut found = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|error| unreadable(&scan_dir, &error))?;
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| user_plugin_name_matches(self.spec, name))
                {
                    found.push(entry.path());
                }
            }
            found.sort();
            let guard = RemovalGuard::under(root);
            for path in found {
                add_file(candidates, path, RemovalReason::MatchingUserPlugin, &guard);
            }
        }
        Ok(())
    }

    fn add_trees(&self, candidates: &mut Vec<PlannedRemoval>) -> Result<(), Removability> {
        for tree in &self.plan.remove.trees {
            for resolved in self.resolve(&tree.path)? {
                let Ok(metadata) = fs::symlink_metadata(&resolved.path) else {
                    continue;
                };
                let guard = RemovalGuard {
                    must_end_with: tree.must_end_with.clone(),
                    name_contains: tree.name_contains.clone(),
                    ..resolved.guard
                };
                candidates.push(PlannedRemoval {
                    path: resolved.path,
                    reason: RemovalReason::Tree,
                    size: None,
                    is_dir: metadata.is_dir(),
                    receipt_sha256: None,
                    guard,
                });
            }
        }
        Ok(())
    }

    /// Receipt entries with a hash. Each one first lends its hash to a
    /// candidate the manifest already found at the same path. Only entries
    /// inside the resource folder become candidates of their own: a receipt
    /// is a file anyone can edit, so a path outside it counts only when a
    /// manifest path covers it, and then the manifest has found it already.
    fn add_receipt_files(&self, candidates: &mut Vec<PlannedRemoval>, kept: &mut Vec<KeptPath>) {
        let Some(receipt) = self.receipt else {
            return;
        };
        let resource = &self.roots.resource;
        for entry in &receipt.installed_files {
            let Some(sha256) = &entry.sha256 else {
                // Folders and user content such as reaper.ini are recorded
                // without a hash, and they are never a package's to remove.
                continue;
            };
            let plain = entry.path.is_absolute() || is_plain_relative(&entry.path);
            let path = resource.join(&entry.path);
            if plain
                && let Some(existing) = candidates
                    .iter_mut()
                    .find(|candidate| same_ignoring_case(&candidate.path, &path))
            {
                existing.receipt_sha256 = Some(sha256.clone());
                continue;
            }
            if !self.plan.remove.receipt_files {
                continue;
            }
            let inside =
                plain && inside_or_same(&path, resource) && !same_ignoring_case(&path, resource);
            if !inside {
                kept.push(KeptPath {
                    exists: plain && fs::symlink_metadata(&path).is_ok(),
                    path,
                    reason: KeptReason::OutsideResourceFolder,
                    note_key: None,
                    removable_with_settings: false,
                });
                continue;
            }
            let added = candidates.len();
            add_file(
                candidates,
                path,
                RemovalReason::Receipt,
                &RemovalGuard::under(resource.clone()),
            );
            if let Some(candidate) = candidates.get_mut(added) {
                candidate.receipt_sha256 = Some(sha256.clone());
            }
        }
    }

    fn dirs_if_empty(&self) -> Result<Vec<PathBuf>, Removability> {
        let mut dirs = Vec::new();
        for path in &self.plan.remove.dirs_if_empty {
            for resolved in self.resolve(path)? {
                // Best effort: a folder that isn't there, or that the guard
                // refuses (a junction, say), is simply left alone.
                let is_dir =
                    fs::symlink_metadata(&resolved.path).is_ok_and(|metadata| metadata.is_dir());
                if is_dir && check_removal(self.roots, &resolved.path, &resolved.guard).is_ok() {
                    dirs.push(resolved.path);
                }
            }
        }
        Ok(dirs)
    }

    /// Ask ReaPack's records about every file. While ReaPack is installed,
    /// a file it owns blocks the package, and so does a file RABBIT couldn't
    /// check. Once ReaPack is gone, nothing keeps its records up to date
    /// any more, so those files are orphans and may go. Returns the ReaPack
    /// packages the orphans came from, or `None` when there are none.
    fn reapack_verdict(
        &self,
        files: &[PlannedRemoval],
        reapack_installed: bool,
    ) -> Result<Option<Vec<String>>, Removability> {
        // ReaPack lists its own file in its records.
        if self.spec.uninstall.reapack_owned == ReapackOwnedPolicy::Ignore {
            return Ok(None);
        }
        let mut orphaned_from: Option<Vec<String>> = None;
        for file in files {
            let reapack_package = match reapack_ownership(&self.roots.resource, &file.path) {
                ReapackOwnership::NotOwned => continue,
                ReapackOwnership::Owned { package } if reapack_installed => {
                    return Err(Removability::ManagedByReaPack {
                        path: file.path.clone(),
                        reapack_package: package,
                    });
                }
                ReapackOwnership::Undecided { reason } if reapack_installed => {
                    return Err(Removability::ReaPackCheckFailed {
                        path: file.path.clone(),
                        reason,
                    });
                }
                ReapackOwnership::Owned { package } => Some(package),
                ReapackOwnership::Undecided { .. } => None,
            };
            let orphans = orphaned_from.get_or_insert_with(Vec::new);
            if let Some(package) = reapack_package
                && !orphans.contains(&package)
            {
                orphans.push(package);
            }
        }
        Ok(orphaned_from)
    }
}

/// Add `path` as a candidate if it is there: a file, or a link for the guard
/// to refuse. A folder where a file was expected isn't a file the package
/// installed, so it is passed by. A path found twice keeps the strongest
/// reason, because a file the manifest names goes even if it changed.
fn add_file(
    candidates: &mut Vec<PlannedRemoval>,
    path: PathBuf,
    reason: RemovalReason,
    guard: &RemovalGuard,
) {
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return;
    };
    if metadata.is_dir() {
        return;
    }
    if let Some(existing) = candidates
        .iter_mut()
        .find(|candidate| same_ignoring_case(&candidate.path, &path))
    {
        if reason_rank(reason) > reason_rank(existing.reason) {
            existing.reason = reason;
            existing.guard = guard.clone();
        }
        return;
    }
    candidates.push(PlannedRemoval {
        path,
        reason,
        size: metadata.is_file().then_some(metadata.len()),
        is_dir: false,
        receipt_sha256: None,
        guard: guard.clone(),
    });
}

fn reason_rank(reason: RemovalReason) -> u8 {
    match reason {
        RemovalReason::Receipt => 0,
        RemovalReason::MatchingUserPlugin => 1,
        RemovalReason::ManifestPath | RemovalReason::Tree | RemovalReason::Setting => 2,
    }
}

/// REAPER's own configuration anywhere in the resource folder: `reaper.ini`,
/// `reaper-kb.ini`, and copies of them such as
/// `reaper-kb.ini.osara-merge-backup`. REAPER rewrites them, they hold the
/// user's settings and keys, and no package's removal ever takes them, not
/// even with "Also remove settings".
fn is_reaper_configuration(roots: &UninstallRoots, path: &Path) -> bool {
    inside_or_same(path, &roots.resource)
        && path
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .is_some_and(|name| {
                ["reaper.ini", "reaper-kb.ini"]
                    .iter()
                    .any(|config| name == *config || name.starts_with(&format!("{config}.")))
            })
}

/// Key maps hold the user's keys, as `reaper-kb.ini` does: a `.ReaperKeyMap`
/// file in the resource folder, or the `KeyMaps` folder that holds them. A
/// package's own key map may be on its remove list, but "Also remove
/// settings" never takes one, so key map backups such as OSARA's
/// `OSARAReplacedBackup.ReaperKeyMap` stay.
fn holds_key_maps(roots: &UninstallRoots, path: &Path) -> bool {
    inside_or_same(path, &roots.resource)
        && (same_ignoring_case(path, &roots.resource.join("KeyMaps"))
            || path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ReaperKeyMap")))
}

/// Whether `path` is one of `paths`, or below one of them, or matches one's
/// `*` pattern. Compared by text, and again by real path where both exist,
/// so a folder reached two ways still counts.
fn matches_any(path: &Path, paths: &[ResolvedPath]) -> bool {
    let real = real_path(path);
    paths.iter().any(|entry| {
        if entry.wildcard {
            let (Some(folder), Some(pattern)) = (
                entry.path.parent(),
                entry.path.file_name().and_then(|name| name.to_str()),
            ) else {
                return false;
            };
            let same_folder = path.parent().is_some_and(|parent| {
                same_ignoring_case(parent, folder)
                    || real
                        .as_deref()
                        .and_then(Path::parent)
                        .zip(fs::canonicalize(folder).ok())
                        .is_some_and(|(real_parent, real_folder)| {
                            same_ignoring_case(real_parent, &real_folder)
                        })
            });
            same_folder
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        crate::package::uninstall::wildcard_matches_ignoring_case(pattern, name)
                    })
        } else {
            inside_or_same(path, &entry.path)
                || real
                    .as_deref()
                    .zip(real_path(&entry.path))
                    .is_some_and(|(real, real_entry)| inside_or_same(real, &real_entry))
        }
    })
}

/// `path` with its folder canonicalized and its own name kept, so a link at
/// the end isn't followed. `None` when the folder isn't there.
fn real_path(path: &Path) -> Option<PathBuf> {
    let parent = fs::canonicalize(path.parent()?).ok()?;
    Some(parent.join(path.file_name()?))
}

/// A relative path made only of names: no root, no drive, no `.` or `..`.
fn is_plain_relative(path: &Path) -> bool {
    path.components().next().is_some()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn unreadable(path: &Path, error: &io::Error) -> Removability {
    Removability::UnsafePath {
        path: path.to_path_buf(),
        refusal: PathRefusal::Unreadable {
            message: format!("{}: {error}", path.display()),
        },
    }
}

/// `items` in the order to remove them: every package before the packages it
/// depends on, so nothing is removed from under a package still there.
/// Packages with no dependency between them keep their order. A cycle, which
/// the manifests never contain, leaves the packages in it in their order.
pub(super) fn removal_order(items: Vec<&UninstallPlanItem>) -> Vec<&UninstallPlanItem> {
    let mut remaining = items;
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        // Ready: no package still waiting depends on it.
        let (ready, waiting): (Vec<&UninstallPlanItem>, Vec<&UninstallPlanItem>) =
            remaining.iter().copied().partition(|item| {
                !remaining
                    .iter()
                    .any(|other| other.depends_on.contains(&item.package_id))
            });
        if ready.is_empty() {
            ordered.extend(waiting);
            break;
        }
        ordered.extend(ready);
        remaining = waiting;
    }
    ordered
}
