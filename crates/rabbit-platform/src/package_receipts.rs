//! macOS installer receipts and the Audio Unit list, read and refreshed
//! without administrator rights.
//!
//! Surge XT's installer leaves a receipt per package, which an uninstall
//! forgets in its one administrator command. Only receipts that are really
//! there are sent to root, so RABBIT lists them first, as the user. After
//! the plug-ins are gone, the Audio Unit list of this user's hosts is
//! refreshed the way Surge's own installer does it, by ending the
//! registrar, which starts again when a host next asks for it.
//!
//! macOS only; elsewhere every call returns `Unsupported`.

use std::collections::BTreeSet;
use std::io;

/// The package IDs `pkgutil` lists, one per line, in `pkgutil --pkgs`
/// output. Blank lines and the spaces and returns around a line are
/// ignored. Pure, so it is tested on every platform.
pub fn parse_pkgutil_pkgs(listing: &str) -> BTreeSet<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Every package receipt on the startup volume, by an unprivileged
/// `/usr/sbin/pkgutil --pkgs`. An error when `pkgutil` can't run or
/// fails. macOS only; elsewhere `Unsupported`.
pub fn installed_package_ids() -> io::Result<BTreeSet<String>> {
    platform_installed_package_ids()
}

/// Make this user's hosts read the Audio Unit list again, by ending
/// `AudioComponentRegistrar` with an unprivileged `/usr/bin/killall -q`.
/// macOS starts the registrar again on demand, and it then finds the
/// plug-ins as they are now. No registrar running is not an error.
///
/// It refuses in a test build and when the running program isn't a RABBIT
/// build by [`crate::running_as_rabbit`], because it ends a process of the
/// user's. macOS only; elsewhere `Unsupported`.
pub fn refresh_audio_units() -> io::Result<()> {
    if cfg!(test) {
        return Err(io::Error::other(
            "a test build never ends the Audio Unit registrar",
        ));
    }
    if !crate::running_as_rabbit() {
        return Err(io::Error::other(
            "only RABBIT itself ends the Audio Unit registrar, and this program isn't named like a RABBIT build",
        ));
    }
    platform_refresh_audio_units()
}

/// What `killall`'s exit status means for [`refresh_audio_units`]: 0 is a
/// registrar ended, and 1 is none found (or none this user may end), which
/// leaves nothing to refresh. Anything else, or no status at all, is an
/// error.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn killall_result(status: Option<i32>) -> io::Result<()> {
    match status {
        Some(0 | 1) => Ok(()),
        Some(code) => Err(io::Error::other(format!(
            "killall AudioComponentRegistrar exited with status {code}"
        ))),
        None => Err(io::Error::other(
            "killall AudioComponentRegistrar was ended by a signal",
        )),
    }
}

#[cfg(target_os = "macos")]
fn platform_installed_package_ids() -> io::Result<BTreeSet<String>> {
    use std::process::{Command, Stdio};

    let mut command = Command::new("/usr/sbin/pkgutil");
    command.arg("--pkgs").stdin(Stdio::null());
    crate::process::without_rabbit_secrets(&mut command);
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "pkgutil --pkgs failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(parse_pkgutil_pkgs(&String::from_utf8_lossy(&output.stdout)))
}

#[cfg(target_os = "macos")]
fn platform_refresh_audio_units() -> io::Result<()> {
    use std::process::{Command, Stdio};

    let mut command = Command::new("/usr/bin/killall");
    command
        .args(["-q", "AudioComponentRegistrar"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::process::without_rabbit_secrets(&mut command);
    killall_result(command.status()?.code())
}

#[cfg(not(target_os = "macos"))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "only macOS has installer receipts and Audio Units",
    )
}

#[cfg(not(target_os = "macos"))]
fn platform_installed_package_ids() -> io::Result<BTreeSet<String>> {
    Err(unsupported())
}

#[cfg(not(target_os = "macos"))]
fn platform_refresh_audio_units() -> io::Result<()> {
    Err(unsupported())
}

#[cfg(test)]
mod tests {
    use super::{killall_result, parse_pkgutil_pkgs, refresh_audio_units};

    #[test]
    fn every_listed_package_id_is_read_once_without_blanks() {
        let listing = "com.apple.pkg.CLTools_Executables\r\norg.surge-synth-team.surge-xt.vst3.pkg\n\n  org.surge-synth-team.surge-xt-fx.app.pkg  \norg.surge-synth-team.surge-xt.vst3.pkg\n";
        let ids: Vec<_> = parse_pkgutil_pkgs(listing).into_iter().collect();
        assert_eq!(
            ids,
            [
                "com.apple.pkg.CLTools_Executables",
                "org.surge-synth-team.surge-xt-fx.app.pkg",
                "org.surge-synth-team.surge-xt.vst3.pkg",
            ]
        );
        assert!(parse_pkgutil_pkgs("").is_empty());
        assert!(parse_pkgutil_pkgs("\n \r\n").is_empty());
    }

    #[test]
    fn no_registrar_to_end_is_not_an_error() {
        assert!(killall_result(Some(0)).is_ok());
        assert!(killall_result(Some(1)).is_ok());
        assert!(killall_result(Some(2)).is_err());
        assert!(killall_result(None).is_err());
    }

    /// Ending a process of the user's is a change, so a test build never
    /// does it, on any platform.
    #[test]
    fn a_test_build_never_ends_the_registrar() {
        let error = refresh_audio_units().unwrap_err();
        assert!(error.to_string().contains("a test build never"), "{error}");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn only_macos_lists_receipts() {
        assert_eq!(
            super::installed_package_ids().unwrap_err().kind(),
            std::io::ErrorKind::Unsupported
        );
    }

    /// Listing receipts changes nothing and needs no rights, so it runs
    /// for real on macOS CI.
    #[cfg(target_os = "macos")]
    #[test]
    fn this_macs_receipts_can_be_listed() {
        let ids = super::installed_package_ids().unwrap();
        for id in &ids {
            assert!(!id.is_empty());
            assert!(!id.contains(char::is_whitespace), "{id:?}");
        }
    }
}
