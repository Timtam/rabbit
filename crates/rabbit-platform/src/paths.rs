//! Per-user OS-specific path lookups.
//!
//! These helpers wrap the env-var probes that used to be scattered across
//! `rabbit-core` (`APPDATA`, `LOCALAPPDATA`, `HOME`, `ProgramFiles*`). Each
//! function is callable on any platform and returns `None`/empty when the
//! variable isn't set, so callers don't have to spread `cfg(target_os)` around
//! their own code paths.

use std::env;
use std::path::PathBuf;

/// `%APPDATA%` on Windows (`Roaming` per-user app-data root). Returns `None`
/// when the variable isn't exported (most non-Windows hosts).
pub fn user_appdata_dir() -> Option<PathBuf> {
    env::var_os("APPDATA").map(PathBuf::from)
}

/// `%LOCALAPPDATA%` on Windows (per-user, machine-local app-data root).
pub fn user_local_appdata_dir() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

/// `$HOME` on Unix-like hosts.
pub fn user_home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

/// The user's Documents folder: the one Windows names as such on Windows,
/// wherever the user moved it, and `~/Documents` on macOS. `None` where it
/// can't be told. Programs keep the user's own work there, such as Surge
/// XT's patches.
pub fn user_documents_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        crate::fs_security::windows_documents_dir().ok()
    } else {
        user_home_dir().map(|home| home.join("Documents"))
    }
}

/// `%ProgramFiles%` and `%ProgramFiles(x86)%` (in that order) on Windows.
/// Empty on hosts where neither variable is set.
pub fn windows_program_files_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(program_files) = env::var_os("ProgramFiles") {
        dirs.push(PathBuf::from(program_files));
    }
    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        dirs.push(PathBuf::from(program_files_x86));
    }
    dirs
}

/// `%CommonProgramFiles%` and `%CommonProgramFiles(x86)%` (in that order)
/// on Windows. The 64-bit (`%CommonProgramFiles%`) entry resolves to
/// `C:\Program Files\Common Files` for a native x64 process and is the
/// same location Inno Setup's `{commoncf64}` constant resolves to; the
/// x86 entry resolves to `C:\Program Files (x86)\Common Files`
/// (`{commoncf32}`). Used by *detection* code that wants to scan both
/// architecture roots for an existing install — *install-target* code
/// should use [`windows_common_program_files_dir`] instead so it
/// asserts only the path the installer actually wrote to. Empty on
/// hosts where neither variable is set.
pub fn windows_common_program_files_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(common) = env::var_os("CommonProgramFiles") {
        dirs.push(PathBuf::from(common));
    }
    if let Some(common_x86) = env::var_os("CommonProgramFiles(x86)") {
        dirs.push(PathBuf::from(common_x86));
    }
    dirs
}

/// `%CommonProgramFiles%` only — the native (64-bit on modern Windows)
/// VST3 root that Inno Setup's `{commoncf64}` resolves to. Use this for
/// install-target / verification code that needs to assert *the* path a
/// 64-bit installer wrote into, rather than scanning both architecture
/// roots. Returns `None` on hosts where the variable isn't set.
pub fn windows_common_program_files_dir() -> Option<PathBuf> {
    env::var_os("CommonProgramFiles").map(PathBuf::from)
}

/// `%PROGRAMDATA%` (with `%ALLUSERSPROFILE%` as the documented fallback)
/// on Windows. The all-users data root (`C:\ProgramData`) — Inno Setup's
/// `{commonappdata}` constant resolves to the same path.
pub fn windows_program_data_dir() -> Option<PathBuf> {
    env::var_os("PROGRAMDATA")
        .or_else(|| env::var_os("ALLUSERSPROFILE"))
        .map(PathBuf::from)
}

/// `%USERPROFILE%\Desktop` — the invoking user's desktop folder on Windows.
/// Resolved from the env var to match the rest of this module; this misses a
/// Desktop redirected by Known Folder move (e.g. into OneDrive), which is an
/// accepted limitation of the env-var approach. Returns `None` when
/// `%USERPROFILE%` isn't set (most non-Windows hosts).
pub fn windows_user_desktop_dir() -> Option<PathBuf> {
    env::var_os("USERPROFILE").map(|profile| PathBuf::from(profile).join("Desktop"))
}

/// `%PUBLIC%\Desktop` — the all-users (common) desktop folder on Windows,
/// `C:\Users\Public\Desktop`, where an installer running for all users drops
/// its desktop shortcut. Returns `None` when `%PUBLIC%` isn't set.
pub fn windows_public_desktop_dir() -> Option<PathBuf> {
    env::var_os("PUBLIC").map(|public| PathBuf::from(public).join("Desktop"))
}

/// `%LOCALAPPDATA%\Programs\Common\CLAP` — the per-user CLAP plug-in folder
/// on Windows, one of the standard locations CLAP hosts (REAPER included)
/// scan. RABBIT installs app2clap here so it needs no elevation and stays
/// scoped to the invoking user. Returns `None` when `%LOCALAPPDATA%` isn't
/// set (most non-Windows hosts).
pub fn windows_clap_dir() -> Option<PathBuf> {
    user_local_appdata_dir().map(|local| local.join("Programs").join("Common").join("CLAP"))
}
