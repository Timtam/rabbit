//! Crash logging for the `panic = "abort"` release profile.
//!
//! The release binary aborts on any panic, so without a hook a crash leaves
//! the user with nothing but a dead process (and, on macOS, an opaque
//! `.ips` abort report with stripped symbols). This hook runs before the
//! abort and writes a small plain-text report — RABBIT version, OS, thread,
//! the panic message with its `file:line`, and a best-effort backtrace — to
//! a well-known per-user log directory, then still forwards to the default
//! hook so the message also lands on stderr for Terminal users.
//!
//! Log locations (best effort, silently skipped when unavailable):
//! - macOS: `~/Library/Logs/RABBIT/crash-<unix-ts>.log` (visible in
//!   Console.app under Log Reports' file list and easy to ask users for)
//! - Windows: `%LOCALAPPDATA%\RABBIT\logs\crash-<unix-ts>.log`
//! - fallback: the OS temp dir under `rabbit-crash-<unix-ts>.log`

use std::backtrace::Backtrace;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Install the crash-log panic hook. Call once, first thing in `main`.
pub fn install() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = build_report(info);
        if let Some(path) = crash_log_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, &report);
            // Tell Terminal users where the file went, too.
            eprintln!("crash report written to {}", path.display());
        }
        default_hook(info);
    }));
}

fn build_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let thread = std::thread::current();
    format!(
        "RABBIT {} crash report\n\
         time: {} (unix)\n\
         os: {} / {}\n\
         thread: {}\n\
         panic: {}\n\
         \n\
         backtrace:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        unix_timestamp(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        thread.name().unwrap_or("<unnamed>"),
        info,
        // Force-captured so users don't need RUST_BACKTRACE set. Frames are
        // partially stripped in release builds, but module offsets plus the
        // panic message's file:line are usually enough to localize the bug.
        Backtrace::force_capture()
    )
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn crash_log_path() -> Option<PathBuf> {
    let file_name = format!("crash-{}.log", unix_timestamp());
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return Some(
                PathBuf::from(home)
                    .join("Library")
                    .join("Logs")
                    .join("RABBIT")
                    .join(file_name),
            );
        }
    }
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return Some(
                PathBuf::from(local)
                    .join("RABBIT")
                    .join("logs")
                    .join(file_name),
            );
        }
    }
    Some(std::env::temp_dir().join(format!("rabbit-{file_name}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end: install the hook, panic (caught), and find the crash log
    /// with the panic message and metadata in it. The hook fires on every
    /// panic regardless of catch_unwind, so this exercises the exact code
    /// path an aborting release panic takes.
    #[test]
    fn writes_crash_report_on_panic() {
        let scratch =
            std::env::temp_dir().join(format!("rabbit-crash-test-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        // Point the per-user log location at the scratch dir for this test.
        #[cfg(windows)]
        unsafe {
            std::env::set_var("LOCALAPPDATA", &scratch)
        };
        #[cfg(target_os = "macos")]
        unsafe {
            std::env::set_var("HOME", &scratch)
        };

        install();
        let _ = std::panic::catch_unwind(|| panic!("rabbit crash-log self test"));
        // Restore a quiet default hook so later panicking tests in this
        // process don't keep writing crash files.
        let _ = std::panic::take_hook();

        let mut found = None;
        for entry in walk(&scratch) {
            let content = std::fs::read_to_string(&entry).unwrap_or_default();
            if content.contains("rabbit crash-log self test") {
                found = Some(content);
                break;
            }
        }
        let report = found.expect("crash log file with the panic message should exist");
        assert!(report.contains("RABBIT"));
        assert!(report.contains("panic:"));
        assert!(
            report.contains("crash_log.rs"),
            "panic location missing: {report}"
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }

    fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return files;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk(&path));
            } else {
                files.push(path);
            }
        }
        files
    }
}
