//! Crash logging for the `panic = "abort"` release profile.
//!
//! The release binary aborts on any panic, so without a hook a crash leaves
//! the user with nothing but a dead process (and, on macOS, an opaque
//! `.ips` abort report with stripped symbols). This hook runs before the
//! abort and writes a small plain-text report — RABBIT version, OS, thread,
//! the panic message with its `file:line`, and a best-effort backtrace —
//! then still forwards to the default hook so the message also lands on
//! stderr for Terminal users.
//!
//! RABBIT is fully portable, so the report is written RIGHT NEXT TO the
//! RABBIT executable (`crash-<unix-ts>.log`) — on macOS next to the
//! `RABBIT.app` bundle, not inside it (a file under `Contents/MacOS` would
//! be invisible to users and break the bundle signature). Locations are
//! tried in order until one accepts the write:
//! 1. `$RABBIT_CRASH_LOG_DIR` when set (support/testing override)
//! 2. the portable install directory (next to the exe / `.app` bundle)
//! 3. the OS temp dir under `rabbit-crash-<unix-ts>.log` — an ephemeral
//!    last resort for read-only installs (DMG mount, Program Files),
//!    deliberately NOT a per-user data directory so a portable RABBIT
//!    never leaves persistent traces outside its own folder.

use std::backtrace::Backtrace;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Install the crash-log panic hook. Call once, first thing in `main`.
pub fn install() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = build_report(info);
        for path in crash_log_candidates() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::write(&path, &report).is_ok() {
                // Tell Terminal users where the file went, too.
                eprintln!("crash report written to {}", path.display());
                break;
            }
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

/// Crash-log destinations in preference order; the hook writes to the
/// first one that succeeds. See the module docs for the rationale.
fn crash_log_candidates() -> Vec<PathBuf> {
    let file_name = format!("crash-{}.log", unix_timestamp());
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("RABBIT_CRASH_LOG_DIR") {
        candidates.push(PathBuf::from(dir).join(&file_name));
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| portable_install_dir(&exe))
    {
        candidates.push(dir.join(&file_name));
    }
    candidates.push(std::env::temp_dir().join(format!("rabbit-{file_name}")));
    candidates
}

/// The user-facing directory RABBIT runs from: the executable's parent —
/// or, when the executable lives inside a macOS `.app` bundle
/// (`…/RABBIT.app/Contents/MacOS/rabbit`), the directory CONTAINING the
/// bundle. Writing inside `Contents/MacOS` would hide the report from the
/// user and invalidate the bundle's code signature.
fn portable_install_dir(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?;
    if dir.ends_with("Contents/MacOS")
        && let Some(bundle) = dir.parent().and_then(Path::parent)
        && bundle
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
    {
        return bundle.parent().map(Path::to_path_buf);
    }
    Some(dir.to_path_buf())
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
        // The highest-priority candidate is the override dir — point it at
        // the scratch dir so the test doesn't litter the test-binary's own
        // directory (the portable-install candidate under `cargo test`).
        unsafe { std::env::set_var("RABBIT_CRASH_LOG_DIR", &scratch) };

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

    #[test]
    fn portable_install_dir_is_the_exe_parent() {
        let exe = PathBuf::from("D:/Tools/RABBIT/rabbit.exe");
        assert_eq!(
            portable_install_dir(&exe),
            Some(PathBuf::from("D:/Tools/RABBIT"))
        );
    }

    /// Inside a macOS bundle the report must land NEXT TO the `.app`, never
    /// inside `Contents/MacOS` (invisible to users, breaks the signature).
    #[test]
    fn portable_install_dir_escapes_macos_app_bundles() {
        let exe = PathBuf::from("/Applications/RABBIT.app/Contents/MacOS/rabbit");
        assert_eq!(
            portable_install_dir(&exe),
            Some(PathBuf::from("/Applications"))
        );
        // A plain directory that merely LOOKS bundle-ish stays untouched.
        let plain = PathBuf::from("/opt/Contents/MacOS/rabbit");
        assert_eq!(
            portable_install_dir(&plain),
            Some(PathBuf::from("/opt/Contents/MacOS"))
        );
    }
}
