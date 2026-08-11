//! Windows Defender exclusion management.
//!
//! RABBIT downloads third-party installers into a dedicated cache folder
//! (`%TEMP%\rabbit-cache`). Some of them — OSARA's development snapshots are
//! the recurring case — are freshly built, low-prevalence, code-signed
//! binaries that Microsoft Defender occasionally quarantines as a false
//! positive, which fails the install. Excluding RABBIT's *own* download
//! folder (and nothing else) from Defender stops that, without ever turning
//! real-time protection off.
//!
//! The exclusion is scoped as narrowly as possible (the one cache folder),
//! added once (a cheap unelevated read skips the elevated add when it is
//! already present), and never removed by RABBIT. Adding it needs one
//! administrator approval; if that is declined or blocked by policy /
//! Tamper Protection, RABBIT proceeds anyway — a later block still surfaces
//! the actionable "allow it in Protection history" guidance.

use std::path::Path;

/// Outcome of trying to make Defender ignore a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefenderExclusionOutcome {
    /// The path was already excluded — nothing to do, no elevation prompt.
    AlreadyPresent,
    /// RABBIT added the exclusion (an elevated call succeeded).
    Added,
    /// Not applicable on this platform (only Windows has Defender).
    Unsupported,
    /// The exclusion could not be added: the read/elevation failed, the
    /// administrator prompt was declined, or policy / Tamper Protection
    /// blocked it. The string is a short reason for logging; callers
    /// proceed regardless.
    Unavailable(String),
}

/// Ensure `path` is on Windows Defender's exclusion list, adding it (with
/// one administrator prompt) only if it isn't already. Best-effort: callers
/// must treat any outcome as non-fatal and continue.
pub fn ensure_path_excluded(path: &Path) -> DefenderExclusionOutcome {
    platform_ensure_path_excluded(path)
}

/// Case-insensitive, trailing-separator-insensitive membership test used to
/// decide whether the elevated add can be skipped. Pure so it is testable
/// without Defender present.
pub(crate) fn path_already_excluded(path: &Path, existing: &[String]) -> bool {
    let target = normalize_exclusion(&path.to_string_lossy());
    existing
        .iter()
        .any(|entry| normalize_exclusion(entry) == target)
}

fn normalize_exclusion(value: &str) -> String {
    value.trim().trim_end_matches(['\\', '/']).to_lowercase()
}

#[cfg(windows)]
fn platform_ensure_path_excluded(path: &Path) -> DefenderExclusionOutcome {
    // 1) Unelevated read of the current exclusions so the common case
    //    (already added on a previous run) never raises a UAC prompt.
    if let Some(existing) = query_exclusion_paths()
        && path_already_excluded(path, &existing)
    {
        return DefenderExclusionOutcome::AlreadyPresent;
    }
    // 2) Add it under a single administrator prompt. Idempotent if a racing
    //    run or an unreadable query left it already present.
    match add_exclusion_elevated(&path.to_string_lossy()) {
        Ok(()) => DefenderExclusionOutcome::Added,
        Err(reason) => DefenderExclusionOutcome::Unavailable(reason),
    }
}

#[cfg(not(windows))]
fn platform_ensure_path_excluded(_path: &Path) -> DefenderExclusionOutcome {
    DefenderExclusionOutcome::Unsupported
}

/// Full path to the system PowerShell, falling back to the bare name (which
/// `CreateProcess`/`ShellExecute` resolve via `PATH`/App Paths) if
/// `%SystemRoot%` is somehow unset.
#[cfg(windows)]
fn powershell_program() -> std::path::PathBuf {
    match std::env::var_os("SystemRoot") {
        Some(root) => std::path::Path::new(&root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe"),
        None => std::path::PathBuf::from("powershell.exe"),
    }
}

/// Read Defender's current path exclusions without elevation. Returns `None`
/// if PowerShell can't be run or the query fails (e.g. this account can't
/// read the preference), in which case the caller falls through to the
/// idempotent elevated add.
#[cfg(windows)]
fn query_exclusion_paths() -> Option<Vec<String>> {
    use std::os::windows::process::CommandExt;

    // CREATE_NO_WINDOW: no console flashes for the background read.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new(powershell_program())
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-MpPreference).ExclusionPath -join [Environment]::NewLine",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(
        text.lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect(),
    )
}

/// Add a single Defender path exclusion under UAC elevation.
#[cfg(windows)]
fn add_exclusion_elevated(path: &str) -> Result<(), String> {
    use crate::elevation::{ElevationError, run_elevated_and_wait};

    // The path goes inside a PowerShell single-quoted string, so double any
    // embedded single quote. Cache paths never contain one, but be exact.
    let escaped = path.replace('\'', "''");
    let command = format!("Add-MpPreference -ExclusionPath '{escaped}'");
    let program = powershell_program();
    let arguments = vec![
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-WindowStyle".to_string(),
        "Hidden".to_string(),
        "-Command".to_string(),
        command,
    ];
    match run_elevated_and_wait(&program, &arguments, None) {
        Ok(Some(0)) => Ok(()),
        Ok(Some(code)) => Err(format!("powershell exited with code {code}")),
        Ok(None) => Err("powershell returned no exit code".to_string()),
        Err(ElevationError::UserCancelledElevation { .. }) => {
            Err("the administrator approval prompt was declined".to_string())
        }
        Err(other) => Err(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn already_excluded_matches_case_and_trailing_separator_insensitively() {
        let path = PathBuf::from(r"C:\Users\x\AppData\Local\Temp\rabbit-cache");
        assert!(path_already_excluded(
            &path,
            &[r"c:\users\x\appdata\local\temp\rabbit-cache\".to_string()],
        ));
        assert!(path_already_excluded(
            &path,
            &[
                r"C:\Other".to_string(),
                r"C:\Users\x\AppData\Local\Temp\rabbit-cache".to_string(),
            ],
        ));
    }

    #[test]
    fn already_excluded_is_false_for_unrelated_or_empty() {
        let path = PathBuf::from(r"C:\Users\x\AppData\Local\Temp\rabbit-cache");
        assert!(!path_already_excluded(&path, &[]));
        assert!(!path_already_excluded(
            &path,
            &[r"C:\Users\x\AppData\Local\Temp".to_string()],
        ));
    }
}
