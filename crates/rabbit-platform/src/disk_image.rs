//! Disk-image mount + app-bundle install support.
//!
//! `mount_disk_image` shells out to macOS' `hdiutil attach` (auto-accepting
//! SLA prompts via stdin "Y\n"); `MountedDiskImage` is a RAII handle that
//! `hdiutil detach`es on drop. `install_app_bundle_from_disk_image` chains
//! mount + bundle copy + detach and returns the path the bundle was copied
//! to. The pure filesystem helpers (`find_app_bundle_in_directory`,
//! `copy_directory_recursive`) are also exposed so `rabbit-core` can share them
//! for `PackageSpec`-driven user-plugin lookups.

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::process::Command;

const DIRECTORY_SEARCH_MAX_DEPTH: usize = 6;

#[derive(Debug)]
pub enum DiskImageError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    HdiutilFailed {
        phase: &'static str,
        image: PathBuf,
        code: Option<i32>,
        stderr: String,
        stdout: String,
    },
    NoMountPoint {
        image: PathBuf,
        stdout: String,
    },
    AppBundleNotFound {
        image: PathBuf,
        bundle: String,
    },
    /// No `.pkg` file matched the supplied filename suffix anywhere
    /// under the mounted volume's directory tree. Used by
    /// [`run_pkg_installer_from_disk_image`] when the upstream DMG layout
    /// shifts.
    PkgNotFound {
        image: PathBuf,
        suffix: String,
    },
    /// The macOS `installer` command (or its admin wrapper) returned a
    /// non-zero exit status. Distinct from `HdiutilFailed` so callers can
    /// surface a clearer message and so the elevation cancel signal can
    /// bubble up unmodified.
    InstallerFailed {
        image: PathBuf,
        code: Option<i32>,
    },
    /// The user dismissed the macOS admin-authorization dialog. Surfaces
    /// as a distinct case so the wizard can prompt them to re-run and
    /// approve, matching the Windows UAC-cancel error.
    UserCancelledElevation {
        image: PathBuf,
    },
    Unsupported {
        image: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for DiskImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "I/O error at {}: {source}", path.display()),
            Self::HdiutilFailed {
                phase,
                image,
                code,
                stderr,
                stdout,
            } => write!(
                f,
                "hdiutil {phase} failed for {} with status {code:?}; stderr: {stderr}; stdout: {stdout}",
                image.display()
            ),
            Self::NoMountPoint { image, stdout } => write!(
                f,
                "hdiutil attach for {} produced no /Volumes mount point; stdout: {stdout}",
                image.display()
            ),
            Self::AppBundleNotFound { image, bundle } => write!(
                f,
                "disk image {} did not contain the expected app bundle {bundle}",
                image.display()
            ),
            Self::PkgNotFound { image, suffix } => write!(
                f,
                "disk image {} did not contain a *{suffix} installer pkg",
                image.display()
            ),
            Self::InstallerFailed { image, code } => write!(
                f,
                "macOS installer for {} returned exit status {code:?}",
                image.display()
            ),
            Self::UserCancelledElevation { image } => write!(
                f,
                "the macOS administrator authorization prompt for installing {} was cancelled or declined",
                image.display()
            ),
            Self::Unsupported { image, message } => {
                write!(f, "disk image {} unsupported: {message}", image.display())
            }
        }
    }
}

impl std::error::Error for DiskImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct MountedDiskImage {
    image_path: PathBuf,
    mount_point: PathBuf,
    detached: bool,
}

impl MountedDiskImage {
    pub fn mount_point(&self) -> &Path {
        &self.mount_point
    }

    pub fn image_path(&self) -> &Path {
        &self.image_path
    }

    pub fn detach(mut self) -> Result<(), DiskImageError> {
        self.detach_inner()
    }

    fn detach_inner(&mut self) -> Result<(), DiskImageError> {
        if self.detached {
            return Ok(());
        }
        self.detached = true;
        run_hdiutil_detach(&self.mount_point, &self.image_path)
    }
}

impl Drop for MountedDiskImage {
    fn drop(&mut self) {
        if !self.detached {
            let _ = self.detach_inner();
        }
    }
}

pub fn mount_disk_image(image_path: &Path) -> Result<MountedDiskImage, DiskImageError> {
    let mount_point = run_hdiutil_attach(image_path)?;
    Ok(MountedDiskImage {
        image_path: image_path.to_path_buf(),
        mount_point,
        detached: false,
    })
}

pub fn install_app_bundle_from_disk_image(
    image_path: &Path,
    install_destination_dir: &Path,
    bundle_basename: &str,
) -> Result<PathBuf, DiskImageError> {
    let mount = mount_disk_image(image_path)?;
    let source =
        find_app_bundle_in_directory(mount.mount_point(), bundle_basename).ok_or_else(|| {
            DiskImageError::AppBundleNotFound {
                image: image_path.to_path_buf(),
                bundle: bundle_basename.to_string(),
            }
        })?;

    fs::create_dir_all(install_destination_dir).map_err(|source| DiskImageError::Io {
        path: install_destination_dir.to_path_buf(),
        source,
    })?;
    let target = install_destination_dir.join(bundle_basename);
    if target.exists() {
        remove_path_recursive(&target)?;
    }
    copy_directory_recursive(&source, &target)?;

    mount.detach()?;
    Ok(target)
}

/// Mount `image_path`, locate the `.pkg` whose filename ends with
/// `pkg_suffix` (e.g. `.pkg` for "any pkg", or
/// `surge-xt-macOS-NIGHTLY-….pkg` for a specific filename), invoke
/// `/usr/sbin/installer -pkg <pkg> -target /` under admin authorization,
/// and detach the image. The detach runs whether the installer
/// succeeded or failed so the volume doesn't stay mounted.
///
/// Returns the absolute path of the `.pkg` that was run, so callers can
/// log it in the saved report. Errors map onto the existing
/// `DiskImageError` variants — `InstallerFailed` for non-zero installer
/// exits, `UserCancelledElevation` when the user dismisses the auth
/// dialog, `PkgNotFound` when the DMG layout no longer matches.
///
/// macOS-only. Other platforms return `Unsupported`.
pub fn run_pkg_installer_from_disk_image(
    image_path: &Path,
    pkg_suffix: &str,
) -> Result<PathBuf, DiskImageError> {
    platform_run_pkg_installer_from_disk_image(image_path, pkg_suffix)
}

#[cfg(target_os = "macos")]
fn platform_run_pkg_installer_from_disk_image(
    image_path: &Path,
    pkg_suffix: &str,
) -> Result<PathBuf, DiskImageError> {
    use crate::elevation::{ElevatedWindow, ElevationError, run_elevated_and_wait};

    let mount = mount_disk_image(image_path)?;
    let pkg = find_pkg_in_directory(mount.mount_point(), pkg_suffix).ok_or_else(|| {
        DiskImageError::PkgNotFound {
            image: image_path.to_path_buf(),
            suffix: pkg_suffix.to_string(),
        }
    })?;

    let installer_args = vec![
        "-pkg".to_string(),
        pkg.display().to_string(),
        "-target".to_string(),
        "/".to_string(),
    ];

    // `Show`: `installer(8)` is the vendor's own package running under the
    // user's eyes, same reasoning as the Windows vendor installers. The value
    // is moot on macOS, where the elevated call runs through `osascript` and
    // shows no terminal either way.
    let exit = run_elevated_and_wait(
        Path::new("/usr/sbin/installer"),
        &installer_args,
        None,
        ElevatedWindow::Show,
    );

    // Detach regardless of installer success so the volume doesn't stay
    // mounted under /Volumes. Best-effort: if the detach itself fails we
    // surface that, but we still drop the elevation result first so the
    // user-cancel signal isn't masked.
    let detach_result = mount.detach();

    match exit {
        Ok(Some(0)) => detach_result?,
        Ok(code) => {
            // If the volume is still mounted, try to free it before
            // reporting the installer failure. The user's first instinct
            // on a failed install is "what does the DMG look like" — a
            // stuck mount is worse than the install failure itself.
            let _ = detach_result;
            return Err(DiskImageError::InstallerFailed {
                image: image_path.to_path_buf(),
                code,
            });
        }
        Err(ElevationError::UserCancelledElevation { .. }) => {
            let _ = detach_result;
            return Err(DiskImageError::UserCancelledElevation {
                image: image_path.to_path_buf(),
            });
        }
        Err(error) => {
            let _ = detach_result;
            return Err(DiskImageError::Unsupported {
                image: image_path.to_path_buf(),
                message: error.to_string(),
            });
        }
    }

    Ok(pkg)
}

#[cfg(not(target_os = "macos"))]
fn platform_run_pkg_installer_from_disk_image(
    image_path: &Path,
    _pkg_suffix: &str,
) -> Result<PathBuf, DiskImageError> {
    Err(DiskImageError::Unsupported {
        image: image_path.to_path_buf(),
        message: "running a .pkg installer from a disk image is only supported on macOS"
            .to_string(),
    })
}

/// Locate the first `.pkg` whose filename ends with `suffix` under
/// `root`. Walks up to [`DIRECTORY_SEARCH_MAX_DEPTH`] levels deep so the
/// Surge XT DMG layout (`Surge XT/surge-xt-macOS-NIGHTLY-….pkg`) is
/// found even though the `.pkg` lives one folder inside the volume root.
#[cfg(target_os = "macos")]
fn find_pkg_in_directory(root: &Path, suffix: &str) -> Option<PathBuf> {
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > DIRECTORY_SEARCH_MAX_DEPTH {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        let mut child_dirs = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if file_type.is_file() && name.ends_with(suffix) {
                return Some(path);
            }
            // The Surge XT DMG mounts as `/Volumes/<title>/Surge XT/<pkg>`
            // — i.e. the `.pkg` is one directory inside the volume root.
            // macOS' `.pkg` files are *files*, not bundles, so we descend
            // into every plain directory rather than treating any of them
            // as opaque (unlike `.app` bundles).
            if file_type.is_dir() && !skip_directory(name) {
                child_dirs.push(path);
            }
        }
        for child in child_dirs {
            stack.push((child, depth + 1));
        }
    }
    None
}

pub fn find_app_bundle_in_directory(root: &Path, basename: &str) -> Option<PathBuf> {
    let target = basename.to_ascii_lowercase();
    if let Some(exact) = find_app_bundle_matching(root, |name| name.to_ascii_lowercase() == target)
    {
        return Some(exact);
    }

    let prefix = target.strip_suffix(".app").unwrap_or(&target);
    if prefix.is_empty() {
        return None;
    }

    find_app_bundle_matching(root, |name| {
        let lower = name.to_ascii_lowercase();
        let stem = match lower.strip_suffix(".app") {
            Some(stem) => stem,
            None => return false,
        };
        if stem == prefix {
            return true;
        }
        let Some(rest) = stem.strip_prefix(prefix) else {
            return false;
        };
        rest.bytes()
            .next()
            .is_some_and(|byte| matches!(byte, b'-' | b'_' | b' ' | b'0'..=b'9'))
    })
}

fn find_app_bundle_matching<F>(root: &Path, predicate: F) -> Option<PathBuf>
where
    F: Fn(&str) -> bool,
{
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > DIRECTORY_SEARCH_MAX_DEPTH {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        let mut child_dirs = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if !file_type.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if predicate(name) {
                return Some(path);
            }
            if !skip_directory(name) {
                child_dirs.push(path);
            }
        }
        for child in child_dirs {
            stack.push((child, depth + 1));
        }
    }
    None
}

pub fn copy_directory_recursive(source: &Path, dest: &Path) -> Result<(), DiskImageError> {
    fs::create_dir_all(dest).map_err(|err| DiskImageError::Io {
        path: dest.to_path_buf(),
        source: err,
    })?;
    let entries = fs::read_dir(source).map_err(|err| DiskImageError::Io {
        path: source.to_path_buf(),
        source: err,
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| DiskImageError::Io {
            path: source.to_path_buf(),
            source: err,
        })?;
        let entry_path = entry.path();
        let entry_name = entry.file_name();
        let target_path = dest.join(&entry_name);
        let file_type = entry.file_type().map_err(|err| DiskImageError::Io {
            path: entry_path.clone(),
            source: err,
        })?;
        if file_type.is_dir() {
            copy_directory_recursive(&entry_path, &target_path)?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let link_target = fs::read_link(&entry_path).map_err(|err| DiskImageError::Io {
                    path: entry_path.clone(),
                    source: err,
                })?;
                std::os::unix::fs::symlink(&link_target, &target_path).map_err(|err| {
                    DiskImageError::Io {
                        path: target_path.clone(),
                        source: err,
                    }
                })?;
            }
            #[cfg(not(unix))]
            {
                fs::copy(&entry_path, &target_path).map_err(|err| DiskImageError::Io {
                    path: target_path.clone(),
                    source: err,
                })?;
            }
        } else {
            fs::copy(&entry_path, &target_path).map_err(|err| DiskImageError::Io {
                path: target_path.clone(),
                source: err,
            })?;
        }
    }
    Ok(())
}

fn remove_path_recursive(path: &Path) -> Result<(), DiskImageError> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|err| DiskImageError::Io {
            path: path.to_path_buf(),
            source: err,
        })
    } else if path.exists() {
        fs::remove_file(path).map_err(|err| DiskImageError::Io {
            path: path.to_path_buf(),
            source: err,
        })
    } else {
        Ok(())
    }
}

fn skip_directory(name: &str) -> bool {
    matches!(
        name,
        ".Trashes" | ".fseventsd" | ".Spotlight-V100" | ".DocumentRevisions-V100"
    )
}

#[cfg(target_os = "macos")]
fn run_hdiutil_attach(image_path: &Path) -> Result<PathBuf, DiskImageError> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("hdiutil")
        .arg("attach")
        .arg("-nobrowse")
        .arg("-readonly")
        .arg("-noautoopen")
        .arg(image_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| DiskImageError::Io {
            path: image_path.to_path_buf(),
            source: err,
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"Y\n");
    }

    let output = child.wait_with_output().map_err(|err| DiskImageError::Io {
        path: image_path.to_path_buf(),
        source: err,
    })?;

    if !output.status.success() {
        return Err(DiskImageError::HdiutilFailed {
            phase: "attach",
            image: image_path.to_path_buf(),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    parse_hdiutil_attach_output(&stdout).ok_or_else(|| DiskImageError::NoMountPoint {
        image: image_path.to_path_buf(),
        stdout: stdout.trim().to_string(),
    })
}

#[cfg(not(target_os = "macos"))]
fn run_hdiutil_attach(image_path: &Path) -> Result<PathBuf, DiskImageError> {
    Err(DiskImageError::Unsupported {
        image: image_path.to_path_buf(),
        message: "disk image mounting is only supported on macOS".to_string(),
    })
}

#[cfg(target_os = "macos")]
fn run_hdiutil_detach(mount_point: &Path, image_path: &Path) -> Result<(), DiskImageError> {
    let output = Command::new("hdiutil")
        .arg("detach")
        .arg("-force")
        .arg(mount_point)
        .output()
        .map_err(|err| DiskImageError::Io {
            path: image_path.to_path_buf(),
            source: err,
        })?;
    if !output.status.success() {
        return Err(DiskImageError::HdiutilFailed {
            phase: "detach",
            image: image_path.to_path_buf(),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        });
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_hdiutil_detach(_mount_point: &Path, _image_path: &Path) -> Result<(), DiskImageError> {
    Ok(())
}

#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(crate) fn parse_hdiutil_attach_output(stdout: &str) -> Option<PathBuf> {
    for line in stdout.lines() {
        if let Some(start) = line.find("/Volumes/") {
            let candidate = line[start..].trim_end();
            if !candidate.is_empty() {
                return Some(PathBuf::from(candidate));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        copy_directory_recursive, find_app_bundle_in_directory, parse_hdiutil_attach_output,
    };

    #[test]
    fn finds_app_bundle_at_root_of_directory_tree() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("REAPER.app");
        fs::create_dir_all(bundle.join("Contents")).unwrap();
        fs::write(bundle.join("Contents").join("Info.plist"), b"<plist/>").unwrap();

        let found = find_app_bundle_in_directory(dir.path(), "REAPER.app");
        assert_eq!(found.as_deref(), Some(bundle.as_path()));
    }

    #[test]
    fn matches_app_bundle_basename_case_insensitively() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("Reaper.app");
        fs::create_dir_all(&bundle).unwrap();

        let found = find_app_bundle_in_directory(dir.path(), "REAPER.app");
        assert_eq!(found.as_deref(), Some(bundle.as_path()));
    }

    #[test]
    fn returns_none_for_missing_app_bundle() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("README")).unwrap();
        let found = find_app_bundle_in_directory(dir.path(), "REAPER.app");
        assert!(found.is_none());
    }

    #[test]
    fn finds_arch_specific_reaper_bundle_when_canonical_name_is_missing() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("REAPER-ARM.app");
        fs::create_dir_all(&bundle).unwrap();

        let found = find_app_bundle_in_directory(dir.path(), "REAPER.app");
        assert_eq!(found.as_deref(), Some(bundle.as_path()));
    }

    #[test]
    fn finds_numeric_suffixed_reaper_bundle_when_canonical_name_is_missing() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("REAPER64.app");
        fs::create_dir_all(&bundle).unwrap();

        let found = find_app_bundle_in_directory(dir.path(), "REAPER.app");
        assert_eq!(found.as_deref(), Some(bundle.as_path()));
    }

    #[test]
    fn prefers_exact_bundle_match_over_variant() {
        let dir = tempdir().unwrap();
        let exact = dir.path().join("REAPER.app");
        fs::create_dir_all(&exact).unwrap();
        fs::create_dir_all(dir.path().join("REAPER-ARM.app")).unwrap();

        let found = find_app_bundle_in_directory(dir.path(), "REAPER.app");
        assert_eq!(found.as_deref(), Some(exact.as_path()));
    }

    #[test]
    fn does_not_match_unrelated_app_bundles_as_reaper_variants() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("REAPERemote.app")).unwrap();
        fs::create_dir_all(dir.path().join("Notepad.app")).unwrap();

        let found = find_app_bundle_in_directory(dir.path(), "REAPER.app");
        assert!(found.is_none());
    }

    #[test]
    fn copies_directory_tree_with_nested_files() {
        let source_root = tempdir().unwrap();
        let source = source_root.path().join("REAPER.app");
        fs::create_dir_all(source.join("Contents").join("MacOS")).unwrap();
        fs::write(source.join("Contents").join("Info.plist"), b"<plist/>").unwrap();
        fs::write(
            source.join("Contents").join("MacOS").join("REAPER"),
            b"binary",
        )
        .unwrap();

        let dest_root = tempdir().unwrap();
        let dest = dest_root.path().join("REAPER.app");
        copy_directory_recursive(&source, &dest).unwrap();

        assert_eq!(
            fs::read(dest.join("Contents").join("MacOS").join("REAPER")).unwrap(),
            b"binary"
        );
        assert_eq!(
            fs::read(dest.join("Contents").join("Info.plist")).unwrap(),
            b"<plist/>"
        );
    }

    #[test]
    fn parses_volumes_line_from_hdiutil_attach_output() {
        let output = "/dev/disk5          \tApple_partition_scheme\t\n\
                      /dev/disk5s1        \tApple_partition_map   \t\n\
                      /dev/disk5s2        \tApple_HFS             \t/Volumes/SWS Extension\n";
        let mount = parse_hdiutil_attach_output(output).unwrap();
        assert_eq!(mount.to_str().unwrap(), "/Volumes/SWS Extension");
    }

    #[test]
    fn returns_no_mount_point_for_unrelated_output() {
        assert!(parse_hdiutil_attach_output("hdiutil: attach: error\n").is_none());
    }
}
