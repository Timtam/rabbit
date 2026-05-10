//! Host-CPU architecture probes that the cross-platform engine can't get
//! from Rust's `target_arch` cfg alone — namely Rosetta detection on
//! Apple Silicon Macs.

/// Returns `true` when the current process is being translated by macOS
/// Rosetta (an `x86_64` binary running on an Apple Silicon host).
///
/// Used by the artifact dispatcher to disambiguate `Architecture::Universal`:
/// when REAPER is universal and RABBIT happens to be running under Rosetta,
/// `Architecture::current()` returns `X64` (the slice the kernel handed
/// the translator), but REAPER launched normally will run as `arm64`. So
/// per-arch plug-ins must be the `arm64` ones, not `x86_64`.
///
/// Best-effort: any failure to read `sysctl.proc_translated` (key missing,
/// sysctl returning a non-success exit, unparseable stdout) is treated as
/// "not under Rosetta". The call is cheap — one short-lived `sysctl(8)`
/// spawn — so callers can use it inline without caching.
///
/// Always `false` on non-macOS hosts. Intel Macs and Apple Silicon Macs
/// running native binaries also return `false`.
pub fn is_running_under_rosetta() -> bool {
    #[cfg(target_os = "macos")]
    {
        rosetta_via_sysctl()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
fn rosetta_via_sysctl() -> bool {
    // `/usr/sbin/sysctl` is part of the base system on every supported
    // macOS — hardcoding the absolute path skips PATH lookup and avoids
    // a Homebrew-installed `sysctl` shadowing the system one.
    let output = match std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "sysctl.proc_translated"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim() == "1"
}

#[cfg(test)]
mod tests {
    use super::is_running_under_rosetta;

    #[test]
    fn never_reports_rosetta_on_non_macos_targets() {
        if cfg!(target_os = "macos") {
            return;
        }
        assert!(!is_running_under_rosetta());
    }
}
