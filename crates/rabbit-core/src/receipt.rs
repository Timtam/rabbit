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
    /// Only ever the *negative* answer, and only for packages whose spec
    /// sets `remember_opt_out`. "Yes" needs no memory — an installed
    /// package is already recorded in `packages` — while "no" was otherwise
    /// forgotten the moment the wizard closed, so a default RABBIT picked
    /// for you (a language pack matching RABBIT's own language) came back
    /// ticked on every launch no matter how often you declined it.
    ///
    /// Left out of the file entirely when empty, and read with
    /// `#[serde(default)]`, so receipts written by older RABBITs still load
    /// and older RABBITs still load receipts written by this one.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub declined_packages: BTreeSet<String>,
}

impl Default for InstallState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            packages: BTreeMap::new(),
            declined_packages: BTreeSet::new(),
        }
    }
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
    pub installed_files: Vec<InstalledFileReceipt>,
    pub installed_at: Option<String>,
    pub rabbit_version: Option<String>,
    pub architecture: Option<Architecture>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledFileReceipt {
    pub path: PathBuf,
    pub sha256: Option<String>,
    pub size: Option<u64>,
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
    pub package_id: &'a str,
    pub version: Option<Version>,
    pub source_url: Option<String>,
    pub source_sha256: Option<String>,
    pub installed_paths: &'a [PathBuf],
    pub installed_at: Option<String>,
    pub architecture: Option<Architecture>,
}

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
    } = params;
    let mut installed_files = installed_paths
        .iter()
        .map(|path| build_installed_file_receipt(resource_path, path))
        .collect::<Result<Vec<_>>>()?;
    installed_files.sort_by(|left, right| left.path.cmp(&right.path));
    installed_files.dedup_by(|left, right| left.path == right.path);

    state.packages.insert(
        package_id.to_string(),
        PackageReceipt {
            id: package_id.to_string(),
            version,
            source_url,
            source_sha256,
            variant: variant.map(str::to_string),
            installed_files,
            installed_at,
            rabbit_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            architecture,
        },
    );
    Ok(())
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

    Ok(InstalledFileReceipt {
        path: relative_or_absolute,
        sha256: metadata
            .is_file()
            .then(|| sha256_file(&absolute_path))
            .transpose()?,
        size: metadata.is_file().then_some(metadata.len()),
    })
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
                installed_files: vec![InstalledFileReceipt {
                    path: PathBuf::from("UserPlugins/reaper_osara64.dll"),
                    sha256: None,
                    size: Some(5),
                }],
                installed_at: None,
                rabbit_version: Some("0.1.0".to_string()),
                architecture: None,
            },
        );

        let state = InstallState {
            schema_version: 1,
            packages,
            declined_packages: Default::default(),
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
                source_url: None,
                source_sha256: None,
                installed_files: Vec::new(),
                installed_at: None,
                rabbit_version: None,
                architecture: None,
            },
        );
        save_install_state(
            dir.path(),
            &InstallState {
                schema_version: 1,
                packages,
                declined_packages: Default::default(),
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
                source_url: None,
                source_sha256: None,
                installed_files: Vec::new(),
                installed_at: None,
                rabbit_version: None,
                architecture: None,
            },
        );
        save_install_state(
            dir.path(),
            &InstallState {
                schema_version: 1,
                packages,
                declined_packages: Default::default(),
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
