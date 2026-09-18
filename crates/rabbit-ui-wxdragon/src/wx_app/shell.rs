//! Hands work to the OS: open a URL or folder, launch REAPER, relaunch
//! RABBIT in another language.

use std::path::Path;
use std::process::Command;

/// macOS: tell Cocoa what language this process is running in by setting the
/// `AppleLanguages` env var, which `[NSBundle preferredLocalizations]` honors
/// before falling back to the user's system-wide language preferences. The
/// bundle's `CFBundleLocalizations` (set in `packaging/macos/Info.plist`)
/// must list the same language codes for this to take effect — without that,
/// Cocoa refuses the override and falls through to its English default. The
/// payoff is VoiceOver picking a voice that matches the in-app UI language;
/// without it, the German UI gets read with the English voice on a system
/// configured for English.
///
/// Uses the BCP-47 language subtag only (`de-DE` → `de`) because that's what
/// matches the `.lproj` directory names and avoids needing region-specific
/// voices to exist on the host. Caller is `run`, before any AppKit init has
/// happened — `AppleLanguages` is read on first access and cached.
#[cfg(target_os = "macos")]
pub(crate) fn seat_macos_apple_languages(locale: &str) {
    let language = locale.split('-').next().unwrap_or(locale).trim();
    if language.is_empty() {
        return;
    }
    // Property-list array literal — Cocoa's preferred encoding for
    // `AppleLanguages` env var values. Single-language form is enough; we
    // don't ship a fallback chain.
    let value = format!("({language})");
    // SAFETY: `run` is called from `main` before any threads are spawned;
    // edition-2024 `set_var` only requires unsafe to flag the cross-thread
    // hazard, which doesn't apply at this point in startup.
    unsafe {
        std::env::set_var("AppleLanguages", value);
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn seat_macos_apple_languages(_locale: &str) {}

/// Relaunch the running RABBIT executable with `RABBIT_LOCALE=<locale>` set so the
/// new locale takes effect immediately, then exit. Errors during relaunch are
/// printed to stderr and the current process keeps running so the user is not
/// left without a UI.
pub(crate) fn relaunch_with_locale(locale: &str) {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            eprintln!("could not resolve current executable for relaunch: {error}");
            return;
        }
    };
    match Command::new(&exe).env("RABBIT_LOCALE", locale).spawn() {
        Ok(_) => std::process::exit(0),
        Err(error) => {
            eprintln!("could not relaunch RABBIT with locale {locale}: {error}");
        }
    }
}

pub(crate) fn open_external_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        // `cmd` is a console program, and RABBIT's release build owns no
        // console to lend it, so without this a black window flashes up
        // every time the user follows a link. The browser `start` hands the
        // URL to is a separate process and still opens normally.
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        rabbit_platform::process::without_console_window(&mut command).spawn()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = url;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "opening URLs is only implemented on Windows and macOS",
        ))
    }
}

pub(crate) fn open_resource_folder(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe").arg(path).spawn()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = path;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "opening folders is only implemented on Windows and macOS",
        ))
    }
}

pub(crate) fn launch_reaper(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new(path).spawn()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        {
            Command::new("open").arg(path).spawn()?;
        } else {
            Command::new(path).spawn()?;
        }
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = path;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "launching REAPER is only implemented on Windows and macOS",
        ))
    }
}
