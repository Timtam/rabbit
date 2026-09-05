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
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn path_already_excluded(path: &Path, existing: &[String]) -> bool {
    let target = normalize_exclusion(&path.to_string_lossy());
    existing
        .iter()
        .any(|entry| normalize_exclusion(entry) == target)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn normalize_exclusion(value: &str) -> String {
    value.trim().trim_end_matches(['\\', '/']).to_lowercase()
}

/// True when `path` contains a character that must never be interpolated
/// into the elevated PowerShell command. The ASCII apostrophe is NOT here —
/// it's safely doubled inside the single-quoted string, so a legitimate
/// `C:\Users\O'Brien\…\rabbit-cache` still works. But PowerShell's lexer
/// treats the Unicode single-quote family as string delimiters too, so those
/// could close the single-quoted string and inject code that then runs as
/// Administrator; control characters are rejected for the same defense.
/// Such characters never appear in a real cache path, so rejecting the whole
/// exclusion for them is safe. Pure, so it is testable without Defender.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn has_unsafe_exclusion_chars(path: &str) -> bool {
    path.chars().any(|ch| {
        matches!(ch, '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') || ch.is_control()
    })
}

#[cfg(windows)]
fn platform_ensure_path_excluded(path: &Path) -> DefenderExclusionOutcome {
    // Decide whether the exclusion is already in place without raising a UAC
    // prompt. Prefer an unelevated read of Defender's list (authoritative);
    // if that can't be read — e.g. a managed machine hides exclusions from
    // local users via policy — fall back to a marker RABBIT wrote when it
    // last added the exclusion, so "added once" survives a blind read
    // instead of re-prompting on every install.
    match query_exclusion_paths() {
        Some(existing) => {
            if path_already_excluded(path, &existing) {
                return DefenderExclusionOutcome::AlreadyPresent;
            }
            // The read succeeded and says the path is absent — trust it over
            // any (possibly stale) marker and re-add.
        }
        None => {
            if marker_matches(path) {
                return DefenderExclusionOutcome::AlreadyPresent;
            }
        }
    }
    // Add it under a single administrator prompt. Idempotent if a racing run
    // or an unreadable query left it already present; record a marker on
    // success so a future blind read still skips the prompt.
    match add_exclusion_elevated(&path.to_string_lossy()) {
        Ok(()) => {
            write_marker(path);
            DefenderExclusionOutcome::Added
        }
        Err(reason) => DefenderExclusionOutcome::Unavailable(reason),
    }
}

/// Path of the sentinel RABBIT writes after successfully adding the
/// exclusion, under `%LOCALAPPDATA%\RABBIT\`. Holds the excluded path so a
/// later run for a *different* cache dir doesn't match a stale marker.
#[cfg(windows)]
fn marker_path() -> Option<std::path::PathBuf> {
    crate::paths::user_local_appdata_dir()
        .map(|dir| dir.join("RABBIT").join(".cache-exclusion-added"))
}

/// True when a marker exists and records this exact path (normalized).
#[cfg(windows)]
fn marker_matches(path: &Path) -> bool {
    let Some(marker) = marker_path() else {
        return false;
    };
    match std::fs::read_to_string(&marker) {
        Ok(content) => {
            normalize_exclusion(&content) == normalize_exclusion(&path.to_string_lossy())
        }
        Err(_) => false,
    }
}

/// Best-effort: record that the exclusion for `path` was added.
#[cfg(windows)]
fn write_marker(path: &Path) {
    if let Some(marker) = marker_path() {
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&marker, path.to_string_lossy().as_bytes());
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
    let mut command = std::process::Command::new(powershell_program());
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "(Get-MpPreference).ExclusionPath -join [Environment]::NewLine",
    ]);
    let output = crate::process::without_console_window(&mut command)
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
    use crate::elevation::{ElevatedWindow, ElevationError, run_elevated_and_wait};

    // Refuse to interpolate a path that could break out of the PowerShell
    // single-quoted string and inject a command that would then run as
    // Administrator. Real cache paths never contain these characters.
    if has_unsafe_exclusion_chars(path) {
        return Err("cache path contains characters unsafe for the exclusion command".to_string());
    }
    // The path goes inside a PowerShell single-quoted string, so double any
    // embedded ASCII apostrophe (legitimate in a Windows user name).
    let escaped = path.replace('\'', "''");
    let command = format!("Add-MpPreference -ExclusionPath '{escaped}'");
    let program = powershell_program();
    // `-WindowStyle Hidden` only takes effect once the PowerShell host is
    // already up, so on its own it still let a console window appear and
    // vanish. `ElevatedWindow::Hidden` is what actually keeps it off screen;
    // the switch stays as a second line of defence.
    let arguments = vec![
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-WindowStyle".to_string(),
        "Hidden".to_string(),
        "-Command".to_string(),
        command,
    ];
    match run_elevated_and_wait(&program, &arguments, None, ElevatedWindow::Hidden) {
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

    #[test]
    fn unsafe_exclusion_chars_flags_powershell_quote_breakouts() {
        // The Unicode single-quote family can close a PowerShell single-
        // quoted string and inject a command; a real payload uses U+2019.
        for injected in [
            "C:\\Temp\\x\u{2019};Start-Process calc;\u{2019}z\\rabbit-cache",
            "C:\\Temp\\x\u{2018}z\\rabbit-cache",
            "C:\\Temp\\x\u{201A}z\\rabbit-cache",
            "C:\\Temp\\x\u{201B}z\\rabbit-cache",
            "C:\\Temp\\x\nz\\rabbit-cache",
        ] {
            assert!(
                has_unsafe_exclusion_chars(injected),
                "should reject: {injected:?}"
            );
        }
    }

    #[test]
    fn unsafe_exclusion_chars_allows_ordinary_paths_including_apostrophe() {
        // The ASCII apostrophe is safely doubled, not rejected — a Windows
        // user named O'Brien has one in their temp path.
        assert!(!has_unsafe_exclusion_chars(
            r"C:\Users\O'Brien\AppData\Local\Temp\rabbit-cache"
        ));
        assert!(!has_unsafe_exclusion_chars(
            r"C:\Users\x\AppData\Local\Temp\rabbit-cache"
        ));
        // Non-ASCII letters in a path are fine (only the quote family isn't).
        assert!(!has_unsafe_exclusion_chars(
            r"C:\Users\Ünïcödé\AppData\Local\Temp\rabbit-cache"
        ));
    }
}
