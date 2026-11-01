use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{IoPathContext, JsonPathContext, Result};
use crate::hash::sha256_file;
use crate::model::Architecture;
use crate::version::Version;

pub const RECEIPT_RELATIVE_PATH: &str = "RABBIT/install-state.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallState {
    pub schema_version: u32,
    pub packages: BTreeMap<String, PackageReceipt>,
    /// Packages the user turned down and does not want offered again on
    /// this install.
    ///
    /// Only ever the *negative* answer. "Yes" needs no memory — an installed
    /// package is already recorded in `packages` — while "no" was otherwise
    /// forgotten the moment the wizard closed, so a default RABBIT picked
    /// for you (a language pack matching RABBIT's own language) came back
    /// ticked on every launch no matter how often you declined it.
    ///
    /// Two things put a package here: the wizard, for a package whose spec
    /// sets `remember_opt_out` and that the user left unticked, and an
    /// uninstall, for any package (see [`remove_package_receipt`]). Removing
    /// a package is as clear a "no" as leaving it unticked, and one set for
    /// both means a recommended package doesn't come back ticked on the very
    /// next run after you removed it. The readers honour the set for every
    /// package: the wizard doesn't tick a package listed here, and the CLI
    /// leaves it out of its default package list.
    ///
    /// Leaving an optional package unticked once only counts as a "no" for
    /// `remember_opt_out` packages, though. Otherwise someone who skipped
    /// FFmpeg one day would never have its updates offered again.
    ///
    /// Only the user's own verdict takes a package out again: ticking it in
    /// the wizard (see [`record_package_opt_outs`]), naming it with the
    /// CLI's `--package` in a run that installs, or restoring it from the
    /// backup set its uninstall wrote (see [`merge_package_receipt`]).
    /// Installing alone does not clear it (see [`upsert_package_receipt`]).
    ///
    /// Left out of the file entirely when empty, and read with
    /// `#[serde(default)]`, so receipts written by older RABBITs still load
    /// and older RABBITs still load receipts written by this one.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub declined_packages: BTreeSet<String>,
    /// What each configuration step found or changed, by step id.
    ///
    /// Recorded so a later uninstall can undo exactly what RABBIT did and
    /// nothing more: a ReaPack repository the user added themselves, or the
    /// language REAPER used before RABBIT switched it, belong to the user.
    /// Optional in the file, like `declined_packages`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub applied_configuration: BTreeMap<String, AppliedConfiguration>,
}

impl Default for InstallState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            packages: BTreeMap::new(),
            declined_packages: BTreeSet::new(),
            applied_configuration: BTreeMap::new(),
        }
    }
}

/// What one configuration step found or changed. See
/// [`InstallState::applied_configuration`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedConfiguration {
    pub outcome: AppliedConfigurationOutcome,
    /// For a ReaPack repository step: the repository it is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reapack_repository: Option<ReapackRepository>,
    /// For the language step: the language pack file it selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub langpack: Option<String>,
    /// For the language step, when it changed the language: the `langpack=`
    /// value under `[REAPER]` in `reaper.ini` just before, exactly as it was
    /// written (`<>` is REAPER's "no language pack"). `None` when there was
    /// no such line or no `reaper.ini`, and when the step changed nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub langpack_before: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppliedConfigurationOutcome {
    /// RABBIT made the change: it added the ReaPack repository, or it wrote
    /// the `langpack=` value.
    Added,
    /// The change was already there, so RABBIT wrote nothing.
    AlreadyPresent,
}

/// A ReaPack repository, as RABBIT writes it into `reapack.ini`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReapackRepository {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageReceipt {
    pub id: String,
    pub version: Option<Version>,
    pub source_url: Option<String>,
    pub source_sha256: Option<String>,
    /// Which package variant was installed, when the package offers a
    /// choice (the Spanish language pack's OSARA translation). Remembered
    /// so a later run keeps the user's pick instead of silently reverting
    /// to the manifest default. `None` for packages with no variants, and
    /// for receipts written before variants existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// The channel this install came from - `dev` for REAPER's development
    /// builds, `pr:1454` for an OSARA pull-request build. `None` means
    /// stable, which is also what every receipt written before channels
    /// existed reads as. Remembered so the CLI and the wizard's expert mode
    /// keep a package on its channel between runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub installed_files: Vec<InstalledFileReceipt>,
    pub installed_at: Option<String>,
    pub rabbit_version: Option<String>,
    pub architecture: Option<Architecture>,
    /// Files the package's own installer added or changed in the folders
    /// its manifest lists under `uninstall.install_snapshot`, found by
    /// listing those folders right before the installer ran and again
    /// afterwards. Today only the JAWS scripts declare any: their installer
    /// copies about a hundred files into each JAWS version and language
    /// folder, and nothing else tells RABBIT which ones.
    ///
    /// Kept apart from `installed_files` on purpose. Detection checks that
    /// every installed file is still there, and deleting one of the JAWS
    /// side files must not make RABBIT think the scripts are gone.
    ///
    /// A reinstall keeps the list (see [`upsert_package_receipt`]), and that
    /// run's own snapshot then adds to it. Optional in the file, like
    /// `declined_packages`, so receipts from before it existed still load.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendor_added_files: Vec<VendorAddedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledFileReceipt {
    pub path: PathBuf,
    pub sha256: Option<String>,
    pub size: Option<u64>,
}

/// One file in [`PackageReceipt::vendor_added_files`], as the installer
/// left it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VendorAddedFile {
    /// Absolute: these files sit outside the resource folder.
    pub path: PathBuf,
    pub sha256: String,
    pub size: u64,
    /// The file was already there before the installer ran, and RABBIT had
    /// no record of it as the installer's own, so what it replaced may have
    /// been the user's: their own key assignments in `reaper.jkm`, say, or
    /// a file JAWS itself keeps there. An uninstall must never remove such a
    /// file just because it is on this list. Files the installer added leave
    /// this `false`, which is left out of the file. A file it rewrote while
    /// the file still held exactly what an earlier install recorded keeps
    /// that install's mark.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub existed_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptVerification {
    MissingReceipt,
    MissingPackage,
    Verified(PackageReceipt),
    Mismatch(PackageReceipt),
}

pub fn receipt_path(resource_path: &Path) -> PathBuf {
    resource_path.join(RECEIPT_RELATIVE_PATH)
}

/// The channel each package at `resource_path` was last installed from, as
/// its receipt recorded it. Read raw rather than filtered to the channels
/// the manifest still offers: a package sitting on a channel that no longer
/// exists has to be offered the way back to stable, not left stranded.
pub fn installed_channels_at(resource_path: &Path) -> crate::package::PackageChannels {
    load_install_state(resource_path)
        .ok()
        .flatten()
        .map(|state| {
            state
                .packages
                .into_iter()
                .filter_map(|(id, receipt)| receipt.channel.map(|channel| (id, channel)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn load_install_state(resource_path: &Path) -> Result<Option<InstallState>> {
    let path = receipt_path(resource_path);
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).with_path(&path)?;
    let state = serde_json::from_str(&content).with_json_path(&path)?;
    Ok(Some(state))
}

/// The packages this install remembers the user declining.
///
/// Missing receipt, unreadable receipt, or a receipt from an older RABBIT
/// all mean "nothing declined" — a preference is never worth failing a run
/// over.
pub fn declined_packages(resource_path: &Path) -> BTreeSet<String> {
    load_install_state(resource_path)
        .ok()
        .flatten()
        .map(|state| state.declined_packages)
        .unwrap_or_default()
}

/// Record the user's answer for the packages that remember one.
///
/// `declined` are the remembering packages they left unticked; `accepted`
/// are the ones they ticked, whose earlier "no" must be forgotten so a
/// change of mind sticks. Never called for a dry run.
///
/// Being installed is NOT a reason to ignore a refusal: someone who stopped
/// using a language pack and turns down its update has refused it just as
/// meaningfully as someone who never installed it. What must never reach
/// this function is a package that had nothing to offer in the first place
/// — an installed, up-to-date package sits unticked because its row is
/// `Keep`, and that is silence, not a "no". Callers decide that, because
/// only they can see the row's action.
pub fn record_package_opt_outs(
    resource_path: &Path,
    declined: &[String],
    accepted: &[String],
) -> Result<()> {
    let mut state = load_install_state(resource_path)?.unwrap_or_default();
    let before = state.declined_packages.clone();
    for id in declined {
        state.declined_packages.insert(id.clone());
    }
    for id in accepted {
        state.declined_packages.remove(id);
    }
    if state.declined_packages == before {
        return Ok(());
    }
    save_install_state(resource_path, &state)
}

/// Save what configuration step `step_id` found or changed into the
/// receipt at `resource_path`. Never called for a dry run.
pub fn record_applied_configuration(
    resource_path: &Path,
    step_id: &str,
    applied: AppliedConfiguration,
) -> Result<()> {
    let mut state = load_install_state(resource_path)?.unwrap_or_default();
    if merge_applied_configuration(&mut state, step_id, applied) {
        save_install_state(resource_path, &state)?;
    }
    Ok(())
}

/// Put `applied` into `state` and say whether anything changed.
///
/// A step that found its work already done never replaces an earlier
/// record. The step runs again whenever the user picks it, and if RABBIT
/// added a ReaPack repository on an earlier run, finding it there now
/// doesn't make it any less RABBIT's. A step that made a change replaces
/// the record, so the language step always holds the value from just before
/// RABBIT's latest switch.
fn merge_applied_configuration(
    state: &mut InstallState,
    step_id: &str,
    applied: AppliedConfiguration,
) -> bool {
    if applied.outcome == AppliedConfigurationOutcome::AlreadyPresent
        && state.applied_configuration.contains_key(step_id)
    {
        return false;
    }
    if state.applied_configuration.get(step_id) == Some(&applied) {
        return false;
    }
    state
        .applied_configuration
        .insert(step_id.to_string(), applied);
    true
}

pub fn save_install_state(resource_path: &Path, state: &InstallState) -> Result<()> {
    let path = receipt_path(resource_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_path(parent)?;
    }

    let content = serde_json::to_string_pretty(state).with_json_path(&path)?;
    // Write-then-rename so a process killed mid-save (the wizard's Close
    // button hard-exits while an operation runs) can never truncate the
    // receipt file — the old state survives until the rename lands.
    let staged = path.with_extension("json.rabbit-tmp");
    fs::write(&staged, content).with_path(&staged)?;
    fs::rename(&staged, &path).with_path(&path)?;
    Ok(())
}

/// The package-specific inputs for [`upsert_package_receipt`], grouped into a
/// struct so the call site doesn't carry a long positional argument list
/// (`state` and `resource_path` stay separate as the "where" context).
pub struct PackageReceiptParams<'a> {
    /// Variant id to remember; see [`PackageReceipt::variant`].
    pub variant: Option<&'a str>,
    /// Channel to remember; see [`PackageReceipt::channel`]. `None` is stable.
    pub channel: Option<&'a str>,
    pub package_id: &'a str,
    pub version: Option<Version>,
    pub source_url: Option<String>,
    pub source_sha256: Option<String>,
    pub installed_paths: &'a [PathBuf],
    pub installed_at: Option<String>,
    pub architecture: Option<Architecture>,
}

/// Record that `package_id` was installed, replacing any earlier entry for
/// it. Only `vendor_added_files` carries over from the earlier entry: the
/// files an earlier run's installer added are still on disk after this run,
/// and the caller merges this run's snapshot, if it took one, into them.
///
/// This leaves `declined_packages` alone. That set only ever changes on the
/// user's own verdict, which the callers give through
/// [`record_package_opt_outs`]: the wizard for a ticked row, the CLI for a
/// package named with `--package`. An install that comes without a verdict
/// must not clear it, such as a package the CLI picked from its default
/// list.
pub fn upsert_package_receipt(
    state: &mut InstallState,
    resource_path: &Path,
    params: PackageReceiptParams<'_>,
) -> Result<()> {
    let PackageReceiptParams {
        package_id,
        version,
        source_url,
        source_sha256,
        installed_paths,
        installed_at,
        architecture,
        variant,
        channel,
    } = params;
    let mut installed_files = installed_paths
        .iter()
        .map(|path| build_installed_file_receipt(resource_path, path))
        .collect::<Result<Vec<_>>>()?;
    installed_files.sort_by(|left, right| left.path.cmp(&right.path));
    installed_files.dedup_by(|left, right| left.path == right.path);
    let vendor_added_files = state
        .packages
        .get(package_id)
        .map(|earlier| earlier.vendor_added_files.clone())
        .unwrap_or_default();

    state.packages.insert(
        package_id.to_string(),
        PackageReceipt {
            id: package_id.to_string(),
            version,
            source_url,
            source_sha256,
            variant: variant.map(str::to_string),
            // Stable is recorded as nothing at all, so a receipt only ever
            // names a channel when there is one to remember.
            channel: channel
                .filter(|id| *id != crate::package::STABLE_CHANNEL)
                .map(str::to_string),
            installed_files,
            installed_at,
            rabbit_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            architecture,
            vendor_added_files,
        },
    );
    Ok(())
}

/// Forget that `package_id` is installed and remember that the user removed
/// it, by adding it to `declined_packages`, so the next install run doesn't
/// tick it again by itself. Returns the entry that was removed, if there was
/// one. The package is declined either way: a copy RABBIT has no receipt
/// for can be uninstalled too.
pub fn remove_package_receipt(
    state: &mut InstallState,
    package_id: &str,
) -> Option<PackageReceipt> {
    state.declined_packages.insert(package_id.to_string());
    state.packages.remove(package_id)
}

/// Put back the receipt entry an uninstall took out, from the backup set it
/// wrote, and take the package off `declined_packages`: restoring a package
/// is as clear a "yes" as ticking it. `entry` is `None` when RABBIT had no
/// receipt for the copy it removed; the refusal goes all the same.
///
/// Returns `false`, and changes nothing, when the receipt already has an
/// entry for the package. It was installed again since, and that newer
/// entry describes the files on disk.
pub fn merge_package_receipt(
    state: &mut InstallState,
    package_id: &str,
    entry: Option<PackageReceipt>,
) -> bool {
    if state.packages.contains_key(package_id) {
        return false;
    }
    state.declined_packages.remove(package_id);
    if let Some(entry) = entry {
        state.packages.insert(package_id.to_string(), entry);
    }
    true
}

/// "Does this package's on-disk install still match the receipt?"
/// Used by the detection layer to decide whether to report the
/// receipt's stamped version (Verified) or fall back to a file-presence
/// probe (Mismatch).
///
/// Compares only file existence and size — *not* SHA-256. Hashing the
/// full file list on every wizard launch is prohibitively expensive
/// for packages like FFmpeg that drop hundreds of MB of DLLs into
/// `UserPlugins` (avcodec ~70 MB, avformat ~30 MB, …). On Windows the
/// per-file open also triggers an AV scan, so a fresh-binary FFmpeg
/// receipt verification used to stall the UI thread for 10-15 seconds
/// at startup. Size mismatch alone catches every realistic regression
/// we care about for the detection use case (partial overwrites by
/// another installer, truncated files); a byte-identical replacement
/// of the same size would be a deliberate user action and would
/// already be reflected in the receipt if it happened through RABBIT.
pub fn verify_package_receipt(
    resource_path: &Path,
    state: Option<&InstallState>,
    package_id: &str,
) -> Result<ReceiptVerification> {
    let Some(state) = state else {
        return Ok(ReceiptVerification::MissingReceipt);
    };
    let Some(receipt) = state.packages.get(package_id) else {
        return Ok(ReceiptVerification::MissingPackage);
    };

    let mut matches = true;
    for file in &receipt.installed_files {
        let absolute = resource_path.join(&file.path);
        let Ok(metadata) = fs::metadata(&absolute) else {
            matches = false;
            break;
        };

        if let Some(expected_size) = file.size
            && metadata.is_file()
            && !holds_user_content(&file.path)
            && metadata.len() != expected_size
        {
            matches = false;
            break;
        }
    }

    if matches {
        Ok(ReceiptVerification::Verified(receipt.clone()))
    } else {
        Ok(ReceiptVerification::Mismatch(receipt.clone()))
    }
}

/// Files an install puts in place but whose contents belong to REAPER and
/// the user from then on: `reaper.ini` is rewritten every time REAPER runs,
/// and `reaper-kb.ini` every time a key binding changes. Their size says
/// nothing about whether the package is still installed, so the receipt
/// records them without one and the check only asks whether they still
/// exist. Before this, a portable REAPER stopped matching its own receipt
/// the first time it was opened, and RABBIT then read it as not installed
/// and reinstalled it on the next run.
fn holds_user_content(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("reaper.ini") || name.eq_ignore_ascii_case("reaper-kb.ini")
        })
}

fn build_installed_file_receipt(
    resource_path: &Path,
    installed_path: &Path,
) -> Result<InstalledFileReceipt> {
    let absolute_path = if installed_path.is_absolute() {
        installed_path.to_path_buf()
    } else {
        resource_path.join(installed_path)
    };
    let metadata = fs::metadata(&absolute_path).with_path(&absolute_path)?;
    let relative_or_absolute = absolute_path
        .strip_prefix(resource_path)
        .map(|path| {
            if path.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                path.to_path_buf()
            }
        })
        .unwrap_or_else(|_| absolute_path.clone());

    let owned_by_rabbit = metadata.is_file() && !holds_user_content(&relative_or_absolute);
    Ok(InstalledFileReceipt {
        path: relative_or_absolute,
        sha256: owned_by_rabbit
            .then(|| sha256_file(&absolute_path))
            .transpose()?,
        size: owned_by_rabbit.then_some(metadata.len()),
    })
}

#[cfg(test)]
mod tests_user_content {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        InstallState, PackageReceipt, PackageReceiptParams, ReceiptVerification,
        load_install_state, upsert_package_receipt, verify_package_receipt,
    };
    use crate::package::PACKAGE_REAPER;

    /// REAPER rewrites reaper.ini whenever it runs. The receipt must survive
    /// that, or RABBIT reads a portable REAPER it installed itself as gone
    /// and reinstalls it on the next run.
    #[test]
    fn reaper_rewriting_its_own_ini_keeps_the_receipt_valid() {
        let dir = tempdir().unwrap();
        let resource_path = dir.path();
        fs::write(resource_path.join("reaper.exe"), b"reaper binary").unwrap();
        fs::write(
            resource_path.join("reaper.ini"),
            b"[REAPER]
",
        )
        .unwrap();

        let mut state = InstallState::default();
        upsert_package_receipt(
            &mut state,
            resource_path,
            PackageReceiptParams {
                package_id: PACKAGE_REAPER,
                version: Some(crate::version::Version::parse("7.80").unwrap()),
                variant: None,
                channel: None,
                source_url: None,
                source_sha256: None,
                installed_paths: &[
                    resource_path.join("reaper.exe"),
                    resource_path.join("reaper.ini"),
                ],
                installed_at: None,
                architecture: None,
            },
        )
        .unwrap();
        crate::receipt::save_install_state(resource_path, &state).unwrap();

        let state = load_install_state(resource_path).unwrap();
        let receipt: &PackageReceipt = state
            .as_ref()
            .unwrap()
            .packages
            .get(PACKAGE_REAPER)
            .unwrap();
        let ini = receipt
            .installed_files
            .iter()
            .find(|file| file.path.ends_with("reaper.ini"))
            .unwrap();
        assert_eq!(ini.size, None, "reaper.ini is recorded without a size");
        assert_eq!(ini.sha256, None);

        // REAPER runs and rewrites its configuration.
        fs::write(
            resource_path.join("reaper.ini"),
            b"[REAPER]
last_project=demo.rpp
",
        )
        .unwrap();

        assert!(matches!(
            verify_package_receipt(resource_path, state.as_ref(), PACKAGE_REAPER).unwrap(),
            ReceiptVerification::Verified(_)
        ));

        // A file RABBIT does own still has to match.
        fs::write(
            resource_path.join("reaper.exe"),
            b"replaced by something else",
        )
        .unwrap();
        assert!(matches!(
            verify_package_receipt(resource_path, state.as_ref(), PACKAGE_REAPER).unwrap(),
            ReceiptVerification::Mismatch(_)
        ));

        // And it must still be there at all.
        fs::remove_file(resource_path.join("reaper.ini")).unwrap();
        assert!(matches!(
            verify_package_receipt(resource_path, state.as_ref(), PACKAGE_REAPER).unwrap(),
            ReceiptVerification::Mismatch(_)
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{
        InstallState, InstalledFileReceipt, PackageReceipt, RECEIPT_RELATIVE_PATH,
        ReceiptVerification, declined_packages, load_install_state, record_package_opt_outs,
        save_install_state, verify_package_receipt,
    };
    use crate::package::PACKAGE_OSARA;
    use crate::version::Version;

    #[test]
    fn saves_loads_and_verifies_receipts() {
        let dir = tempdir().unwrap();
        let plugin_path = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugin_path).unwrap();
        fs::write(plugin_path.join("reaper_osara64.dll"), b"osara").unwrap();

        let mut packages = BTreeMap::new();
        packages.insert(
            PACKAGE_OSARA.to_string(),
            PackageReceipt {
                id: PACKAGE_OSARA.to_string(),
                version: Some(Version::parse("2024.1").unwrap()),
                source_url: None,
                source_sha256: None,
                variant: None,
                channel: None,
                installed_files: vec![InstalledFileReceipt {
                    path: PathBuf::from("UserPlugins/reaper_osara64.dll"),
                    sha256: None,
                    size: Some(5),
                }],
                installed_at: None,
                rabbit_version: Some("0.1.0".to_string()),
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );

        let state = InstallState {
            schema_version: 1,
            packages,
            declined_packages: Default::default(),
            applied_configuration: Default::default(),
        };
        save_install_state(dir.path(), &state).unwrap();

        let loaded = load_install_state(dir.path()).unwrap().unwrap();
        assert_eq!(loaded, state);
        assert!(matches!(
            verify_package_receipt(dir.path(), Some(&loaded), PACKAGE_OSARA).unwrap(),
            ReceiptVerification::Verified(_)
        ));
    }
    #[test]
    fn records_and_forgets_package_opt_outs() {
        let dir = tempdir().unwrap();
        assert!(declined_packages(dir.path()).is_empty());

        // Turning a package down is remembered...
        record_package_opt_outs(dir.path(), &["langpack-de".to_string()], &[]).unwrap();
        assert!(declined_packages(dir.path()).contains("langpack-de"));

        // ...and a later change of mind forgets it, so the package goes back
        // to being offered by default.
        record_package_opt_outs(dir.path(), &[], &["langpack-de".to_string()]).unwrap();
        assert!(declined_packages(dir.path()).is_empty());
    }

    #[test]
    fn opt_outs_do_not_disturb_installed_package_receipts() {
        let dir = tempdir().unwrap();
        let mut packages = BTreeMap::new();
        packages.insert(
            "osara".to_string(),
            PackageReceipt {
                id: "osara".to_string(),
                version: None,
                variant: None,
                channel: None,
                source_url: None,
                source_sha256: None,
                installed_files: Vec::new(),
                installed_at: None,
                rabbit_version: None,
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );
        save_install_state(
            dir.path(),
            &InstallState {
                schema_version: 1,
                packages,
                declined_packages: Default::default(),
                applied_configuration: Default::default(),
            },
        )
        .unwrap();

        record_package_opt_outs(dir.path(), &["langpack-de".to_string()], &[]).unwrap();
        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert!(
            state.packages.contains_key("osara"),
            "install facts survive"
        );
        assert!(state.declined_packages.contains("langpack-de"));
    }

    #[test]
    fn receipts_stay_compatible_in_both_directions() {
        let dir = tempdir().unwrap();
        // A receipt written by an older RABBIT has no `declined_packages`
        // key at all; it must still load.
        fs::create_dir_all(dir.path().join("RABBIT")).unwrap();
        fs::write(
            dir.path().join(RECEIPT_RELATIVE_PATH),
            r#"{"schema_version":1,"packages":{}}"#,
        )
        .unwrap();
        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert!(state.declined_packages.is_empty());

        // And with nothing declined we write the key out again, so an older
        // RABBIT reading our file sees exactly what it wrote.
        save_install_state(dir.path(), &state).unwrap();
        let text = fs::read_to_string(dir.path().join(RECEIPT_RELATIVE_PATH)).unwrap();
        assert!(
            !text.contains("declined_packages"),
            "an empty opt-out set must not appear in the receipt: {text}"
        );
    }
    #[test]
    fn refusing_an_update_to_an_installed_package_is_remembered() {
        // Someone who stopped using the German pack and turns down its
        // update has refused it. Being installed does not make that any less
        // of a "no" — the previous version of this skipped anything with a
        // receipt, which silently threw the refusal away.
        let dir = tempdir().unwrap();
        let mut packages = BTreeMap::new();
        packages.insert(
            "langpack-de".to_string(),
            PackageReceipt {
                id: "langpack-de".to_string(),
                version: None,
                variant: None,
                channel: None,
                source_url: None,
                source_sha256: None,
                installed_files: Vec::new(),
                installed_at: None,
                rabbit_version: None,
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );
        save_install_state(
            dir.path(),
            &InstallState {
                schema_version: 1,
                packages,
                declined_packages: Default::default(),
                applied_configuration: Default::default(),
            },
        )
        .unwrap();

        record_package_opt_outs(dir.path(), &["langpack-de".to_string()], &[]).unwrap();
        assert!(
            declined_packages(dir.path()).contains("langpack-de"),
            "turning down an update to an installed package must be remembered"
        );

        // And ticking it again clears it, as for any other package.
        record_package_opt_outs(dir.path(), &[], &["langpack-de".to_string()]).unwrap();
        assert!(declined_packages(dir.path()).is_empty());
    }
}

/// What the receipt records for a later uninstall: removed packages and what
/// configuration steps did.
#[cfg(test)]
mod tests_uninstall_records {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        AppliedConfiguration, AppliedConfigurationOutcome, InstallState, PackageReceiptParams,
        RECEIPT_RELATIVE_PATH, ReapackRepository, VendorAddedFile, declined_packages,
        load_install_state, merge_package_receipt, record_applied_configuration,
        record_package_opt_outs, remove_package_receipt, save_install_state,
        upsert_package_receipt,
    };
    use crate::package::{PACKAGE_JAWS_SCRIPTS, PACKAGE_OSARA, PACKAGE_SWS};

    fn install(state: &mut InstallState, package_id: &str) {
        upsert_package_receipt(
            state,
            std::path::Path::new("."),
            PackageReceiptParams {
                variant: None,
                channel: None,
                package_id,
                version: None,
                source_url: None,
                source_sha256: None,
                installed_paths: &[],
                installed_at: None,
                architecture: None,
            },
        )
        .unwrap();
    }

    fn receipt_text(dir: &std::path::Path) -> String {
        fs::read_to_string(dir.join(RECEIPT_RELATIVE_PATH)).unwrap()
    }

    fn repository_step(outcome: AppliedConfigurationOutcome) -> AppliedConfiguration {
        AppliedConfiguration {
            outcome,
            reapack_repository: Some(ReapackRepository {
                name: "REAPER Accessibility".to_string(),
                url: "https://example.test/index.xml".to_string(),
            }),
            langpack: None,
            langpack_before: None,
        }
    }

    fn language_step(langpack: &str, before: Option<&str>) -> AppliedConfiguration {
        AppliedConfiguration {
            outcome: AppliedConfigurationOutcome::Added,
            reapack_repository: None,
            langpack: Some(langpack.to_string()),
            langpack_before: before.map(str::to_string),
        }
    }

    #[test]
    fn removing_a_package_forgets_its_receipt_and_declines_it() {
        let mut state = InstallState::default();
        install(&mut state, PACKAGE_SWS);

        let removed = remove_package_receipt(&mut state, PACKAGE_SWS);

        assert_eq!(
            removed.map(|receipt| receipt.id).as_deref(),
            Some(PACKAGE_SWS)
        );
        assert!(!state.packages.contains_key(PACKAGE_SWS));
        assert!(state.declined_packages.contains(PACKAGE_SWS));

        // A copy RABBIT had no receipt for can be removed too, and is
        // remembered the same way.
        assert_eq!(remove_package_receipt(&mut state, PACKAGE_OSARA), None);
        assert!(state.declined_packages.contains(PACKAGE_OSARA));
    }

    /// A removal is saved like any other refusal, so everything that reads
    /// the user's refusals sees it.
    #[test]
    fn a_removal_is_saved_as_a_refusal() {
        let dir = tempdir().unwrap();
        let mut state = InstallState::default();
        install(&mut state, PACKAGE_SWS);
        remove_package_receipt(&mut state, PACKAGE_SWS);
        save_install_state(dir.path(), &state).unwrap();

        assert_eq!(load_install_state(dir.path()).unwrap().unwrap(), state);
        assert!(declined_packages(dir.path()).contains(PACKAGE_SWS));
    }

    /// Installing a removed package again doesn't clear the refusal by
    /// itself. The user's verdict does: ticking it in the wizard, or naming
    /// it with the CLI's `--package`, both of which record it here.
    #[test]
    fn only_ticking_a_removed_package_clears_the_refusal() {
        let dir = tempdir().unwrap();
        let mut state = InstallState::default();
        install(&mut state, PACKAGE_SWS);
        remove_package_receipt(&mut state, PACKAGE_SWS);
        remove_package_receipt(&mut state, PACKAGE_OSARA);

        install(&mut state, PACKAGE_SWS);
        assert!(state.packages.contains_key(PACKAGE_SWS));
        assert!(state.declined_packages.contains(PACKAGE_SWS));
        save_install_state(dir.path(), &state).unwrap();

        record_package_opt_outs(dir.path(), &[], &[PACKAGE_SWS.to_string()]).unwrap();
        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert!(state.packages.contains_key(PACKAGE_SWS));
        assert!(!state.declined_packages.contains(PACKAGE_SWS));
        assert!(
            state.declined_packages.contains(PACKAGE_OSARA),
            "only the package ticked again is cleared"
        );
    }

    /// Restoring a removed package puts its entry back and takes it off the
    /// refusals, without disturbing anything else; a package installed
    /// again since keeps its newer entry.
    #[test]
    fn merging_a_restored_entry_back_clears_the_refusal() {
        let mut state = InstallState::default();
        install(&mut state, PACKAGE_SWS);
        install(&mut state, PACKAGE_OSARA);
        let entry = remove_package_receipt(&mut state, PACKAGE_SWS);
        remove_package_receipt(&mut state, "langpack-de");

        assert!(merge_package_receipt(
            &mut state,
            PACKAGE_SWS,
            entry.clone()
        ));
        assert_eq!(state.packages.get(PACKAGE_SWS), entry.as_ref());
        assert!(!state.declined_packages.contains(PACKAGE_SWS));
        assert!(state.packages.contains_key(PACKAGE_OSARA));
        assert!(state.declined_packages.contains("langpack-de"));

        // No entry to merge: only the refusal goes.
        assert!(merge_package_receipt(&mut state, "langpack-de", None));
        assert!(!state.packages.contains_key("langpack-de"));
        assert!(state.declined_packages.is_empty());

        // Installed again since: the newer entry stays, and so does any
        // refusal recorded alongside it.
        let mut older = entry.unwrap();
        older.version = Some(crate::version::Version::parse("2.0").unwrap());
        state.declined_packages.insert(PACKAGE_SWS.to_string());
        assert!(!merge_package_receipt(&mut state, PACKAGE_SWS, Some(older)));
        assert_eq!(state.packages[PACKAGE_SWS].version, None);
        assert!(state.declined_packages.contains(PACKAGE_SWS));
    }

    /// A receipt from before these fields existed still loads, and one with
    /// nothing to record in them is written exactly as an older RABBIT
    /// would write it.
    #[test]
    fn older_receipts_without_the_new_fields_still_load() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("RABBIT")).unwrap();
        fs::write(
            dir.path().join(RECEIPT_RELATIVE_PATH),
            r#"{"schema_version":1,"packages":{"osara":{"id":"osara","version":"2024.1",
                "source_url":null,"source_sha256":null,"installed_files":[],
                "installed_at":null,"rabbit_version":"0.9.0","architecture":null}},
                "declined_packages":["langpack-de"]}"#,
        )
        .unwrap();

        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert!(state.applied_configuration.is_empty());
        assert!(state.packages.contains_key(PACKAGE_OSARA));
        assert!(state.packages[PACKAGE_OSARA].vendor_added_files.is_empty());
        assert!(state.declined_packages.contains("langpack-de"));

        save_install_state(dir.path(), &state).unwrap();
        let text = receipt_text(dir.path());
        for field in ["applied_configuration", "vendor_added_files"] {
            assert!(
                !text.contains(field),
                "{field} must stay out when empty: {text}"
            );
        }
    }

    fn vendor_file(path: &str, existed_before: bool) -> VendorAddedFile {
        VendorAddedFile {
            path: std::path::PathBuf::from(path),
            sha256: "ab".repeat(32),
            size: 42,
            existed_before,
        }
    }

    /// The files the JAWS scripts' installer added go into the receipt and
    /// come back out unchanged; the mark for a file that was there before is
    /// written only where it is set.
    #[test]
    fn vendor_added_files_round_trip() {
        let dir = tempdir().unwrap();
        let mut state = InstallState::default();
        install(&mut state, PACKAGE_JAWS_SCRIPTS);
        state
            .packages
            .get_mut(PACKAGE_JAWS_SCRIPTS)
            .unwrap()
            .vendor_added_files = vec![
            vendor_file("/jaws/2026/Settings/deu/reaper.jss", false),
            vendor_file("/jaws/2026/Settings/deu/reaper.jkm", true),
        ];
        save_install_state(dir.path(), &state).unwrap();

        assert_eq!(load_install_state(dir.path()).unwrap().unwrap(), state);
        let text = receipt_text(dir.path());
        assert_eq!(text.matches("vendor_added_files").count(), 1, "{text}");
        assert_eq!(text.matches("existed_before").count(), 1, "{text}");
    }

    /// An update installs the scripts again. The files earlier installs
    /// recorded are still on disk, so the record carries over to the new
    /// entry, for that run's own snapshot to add to.
    #[test]
    fn a_reinstall_keeps_the_vendor_added_files() {
        let mut state = InstallState::default();
        install(&mut state, PACKAGE_JAWS_SCRIPTS);
        let recorded = vec![vendor_file("/jaws/reaper.jss", false)];
        state
            .packages
            .get_mut(PACKAGE_JAWS_SCRIPTS)
            .unwrap()
            .vendor_added_files = recorded.clone();

        install(&mut state, PACKAGE_JAWS_SCRIPTS);
        assert_eq!(
            state.packages[PACKAGE_JAWS_SCRIPTS].vendor_added_files,
            recorded
        );

        // An uninstall forgets the entry, record and all; the backup set
        // keeps it for a restore.
        remove_package_receipt(&mut state, PACKAGE_JAWS_SCRIPTS);
        install(&mut state, PACKAGE_JAWS_SCRIPTS);
        assert!(
            state.packages[PACKAGE_JAWS_SCRIPTS]
                .vendor_added_files
                .is_empty()
        );
    }

    /// Development builds of the uninstall work wrote a `removed_packages`
    /// set and an OSARA `keymap_backup_id`, both dropped since. Neither
    /// struct refuses unknown fields, so such a receipt still loads, and the
    /// next save leaves both out.
    #[test]
    fn receipts_with_dropped_fields_still_load() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("RABBIT")).unwrap();
        fs::write(
            dir.path().join(RECEIPT_RELATIVE_PATH),
            r#"{"schema_version":1,"packages":{"osara":{"id":"osara","version":"2024.1",
                "source_url":null,"source_sha256":null,"installed_files":[],
                "installed_at":null,"rabbit_version":"0.4.0","architecture":null,
                "keymap_backup_id":"unix-1782841261"}},
                "removed_packages":["sws"]}"#,
        )
        .unwrap();

        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert!(state.packages.contains_key(PACKAGE_OSARA));
        assert!(state.declined_packages.is_empty());

        save_install_state(dir.path(), &state).unwrap();
        let text = receipt_text(dir.path());
        for field in ["removed_packages", "keymap_backup_id"] {
            assert!(!text.contains(field), "{field} must be dropped: {text}");
        }
    }

    #[test]
    fn applied_configuration_round_trips() {
        let dir = tempdir().unwrap();
        record_applied_configuration(
            dir.path(),
            "reapack-add-reaper-accessibility-remote",
            repository_step(AppliedConfigurationOutcome::Added),
        )
        .unwrap();
        record_applied_configuration(
            dir.path(),
            "set-reaper-language",
            language_step("DE_(+SWS).ReaperLangPack", Some("<>")),
        )
        .unwrap();

        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert_eq!(
            state.applied_configuration["reapack-add-reaper-accessibility-remote"],
            repository_step(AppliedConfigurationOutcome::Added)
        );
        assert_eq!(
            state.applied_configuration["set-reaper-language"],
            language_step("DE_(+SWS).ReaperLangPack", Some("<>"))
        );
        let text = receipt_text(dir.path());
        assert!(text.contains(r#""outcome": "added""#), "{text}");
        // A repository step has no language fields, and the other way round.
        assert_eq!(text.matches("langpack_before").count(), 1, "{text}");
        assert_eq!(text.matches("reapack_repository").count(), 1, "{text}");
    }

    /// The step runs again whenever the user picks it. Finding the
    /// repository already there must not hide that RABBIT added it.
    #[test]
    fn finding_a_change_already_there_keeps_the_earlier_record() {
        let dir = tempdir().unwrap();
        let step = "reapack-add-reaper-accessibility-remote";
        record_applied_configuration(
            dir.path(),
            step,
            repository_step(AppliedConfigurationOutcome::Added),
        )
        .unwrap();
        record_applied_configuration(
            dir.path(),
            step,
            repository_step(AppliedConfigurationOutcome::AlreadyPresent),
        )
        .unwrap();
        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert_eq!(
            state.applied_configuration[step].outcome,
            AppliedConfigurationOutcome::Added
        );

        // With no earlier record, "already there" is recorded: RABBIT
        // didn't add it, so an uninstall must leave it alone.
        let other = "reapack-add-reaper-accessible-en-remote";
        record_applied_configuration(
            dir.path(),
            other,
            repository_step(AppliedConfigurationOutcome::AlreadyPresent),
        )
        .unwrap();
        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert_eq!(
            state.applied_configuration[other].outcome,
            AppliedConfigurationOutcome::AlreadyPresent
        );
        assert!(receipt_text(dir.path()).contains(r#""outcome": "already-present""#));
    }

    /// A later language switch replaces the record, so it holds the value
    /// from just before RABBIT's latest change.
    #[test]
    fn a_later_language_switch_replaces_the_record() {
        let dir = tempdir().unwrap();
        let step = "set-reaper-language";
        record_applied_configuration(
            dir.path(),
            step,
            language_step("de.ReaperLangPack", Some("<>")),
        )
        .unwrap();
        record_applied_configuration(
            dir.path(),
            step,
            language_step("fr.ReaperLangPack", Some("de.ReaperLangPack")),
        )
        .unwrap();

        let state = load_install_state(dir.path()).unwrap().unwrap();
        assert_eq!(
            state.applied_configuration[step],
            language_step("fr.ReaperLangPack", Some("de.ReaperLangPack"))
        );
    }
}
