//! How far this computer's local time is from UTC, for names a person
//! reads.
//!
//! When the run's administrator command moves REAPER's app into the user's
//! Trash, RABBIT gives it a name of its own with the time in it, such as
//! `REAPER 2026-11-07 15.30.12.app`. That time should be the one on the
//! user's clock, not UTC.
//!
//! Only macOS needs this; elsewhere [`local_utc_offset_seconds`] returns
//! `Unsupported`, and callers use UTC.

use std::io;
use std::time::SystemTime;

/// How many seconds local time is ahead of UTC at `at`, by this computer's
/// time zone and its daylight saving rules: 3600 for Central European
/// winter time, -18000 for New York in winter. macOS only; elsewhere
/// `Unsupported`.
pub fn local_utc_offset_seconds(at: SystemTime) -> io::Result<i32> {
    platform_local_utc_offset_seconds(at)
}

#[cfg(target_os = "macos")]
fn platform_local_utc_offset_seconds(at: SystemTime) -> io::Result<i32> {
    let seconds = match at.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).map_err(io::Error::other)?,
        Err(before) => -i64::try_from(before.duration().as_secs()).map_err(io::Error::other)?,
    };
    let time: libc::time_t = seconds;
    // SAFETY: `tm` is plain data, for which all zero bytes are a valid
    // value; localtime_r only writes it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are to locals that live across the call, and
    // localtime_r keeps neither.
    let filled = unsafe { libc::localtime_r(&time, &mut tm) };
    if filled.is_null() {
        return Err(io::Error::last_os_error());
    }
    i32::try_from(tm.tm_gmtoff).map_err(io::Error::other)
}

#[cfg(not(target_os = "macos"))]
fn platform_local_utc_offset_seconds(_at: SystemTime) -> io::Result<i32> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "only macOS reads the local time zone this way",
    ))
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::local_utc_offset_seconds;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn only_macos_reads_the_offset() {
        assert_eq!(
            local_utc_offset_seconds(SystemTime::now())
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::Unsupported
        );
    }

    /// Every time zone on Earth is within 14 hours of UTC, and in whole
    /// quarter hours.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_offset_is_one_a_time_zone_can_have() {
        let offset = local_utc_offset_seconds(SystemTime::now()).unwrap();
        assert!(offset.abs() <= 14 * 3600, "{offset}");
        assert_eq!(offset % (15 * 60), 0, "{offset}");
    }
}
