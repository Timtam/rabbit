//! How far this computer's local time is from UTC, and calendar dates, for
//! names and dates a person reads.
//!
//! When the run's administrator command moves REAPER's app into the user's
//! Trash, RABBIT gives it a name of its own with the time in it, such as
//! `REAPER 2026-11-07 15.30.12.app`. `rabbit backups` says when each backup
//! was made. Both times should be the ones on the user's clock, not UTC.
//!
//! macOS and Windows read the offset; elsewhere [`local_utc_offset_seconds`]
//! returns `Unsupported`, and callers use UTC. The workspace has no date
//! crate, so [`civil_date`] and [`civil_date_time`] turn seconds into a
//! calendar date here.

use std::io;
use std::time::SystemTime;

/// How many seconds local time is ahead of UTC at `at`, by this computer's
/// time zone and its daylight saving rules for that moment: 3600 for
/// Central European winter time, -18000 for New York in winter. macOS and
/// Windows; elsewhere `Unsupported`.
pub fn local_utc_offset_seconds(at: SystemTime) -> io::Result<i32> {
    platform_local_utc_offset_seconds(at)
}

/// Seconds since 1970 (UTC) at `at`, negative before 1970.
#[cfg(any(target_os = "macos", windows))]
fn unix_seconds(at: SystemTime) -> io::Result<i64> {
    match at.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).map_err(io::Error::other),
        Err(before) => i64::try_from(before.duration().as_secs())
            .map(|seconds| -seconds)
            .map_err(io::Error::other),
    }
}

#[cfg(target_os = "macos")]
fn platform_local_utc_offset_seconds(at: SystemTime) -> io::Result<i32> {
    let time: libc::time_t = unix_seconds(at)?;
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

/// Windows converts the moment to local time by the current time zone's
/// rules for that year (`SystemTimeToTzSpecificLocalTimeEx`), and the
/// offset is the difference. A `FILETIME` counts 100-nanosecond ticks since
/// 1601-01-01 (UTC), so the seconds are whole on both sides.
#[cfg(windows)]
fn platform_local_utc_offset_seconds(at: SystemTime) -> io::Result<i32> {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{
        DYNAMIC_TIME_ZONE_INFORMATION, FileTimeToSystemTime, GetDynamicTimeZoneInformation,
        SystemTimeToFileTime, SystemTimeToTzSpecificLocalTimeEx, TIME_ZONE_ID_INVALID,
    };

    const TICKS_PER_SECOND: i64 = 10_000_000;
    const SECONDS_FROM_1601_TO_1970: i64 = 11_644_473_600;
    let ticks = unix_seconds(at)?
        .checked_add(SECONDS_FROM_1601_TO_1970)
        .and_then(|seconds| seconds.checked_mul(TICKS_PER_SECOND))
        .and_then(|ticks| u64::try_from(ticks).ok())
        .ok_or_else(|| io::Error::other("the moment is outside what Windows can convert"))?;
    let utc_file = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut utc = SYSTEMTIME::default();
    // SAFETY: both pointers are to locals that live across the call.
    if unsafe { FileTimeToSystemTime(&utc_file, &mut utc) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
    // SAFETY: the pointer is to a local that lives across the call, which
    // only writes it.
    if unsafe { GetDynamicTimeZoneInformation(&mut zone) } == TIME_ZONE_ID_INVALID {
        return Err(io::Error::last_os_error());
    }
    let mut local = SYSTEMTIME::default();
    // SAFETY: all three pointers are to locals that live across the call.
    if unsafe { SystemTimeToTzSpecificLocalTimeEx(&zone, &utc, &mut local) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut local_file = FILETIME::default();
    // SAFETY: both pointers are to locals that live across the call.
    if unsafe { SystemTimeToFileTime(&local, &mut local_file) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let local_ticks =
        (u64::from(local_file.dwHighDateTime) << 32) | u64::from(local_file.dwLowDateTime);
    let difference = (i128::from(local_ticks) - i128::from(ticks)) / i128::from(TICKS_PER_SECOND);
    i32::try_from(difference).map_err(io::Error::other)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_local_utc_offset_seconds(_at: SystemTime) -> io::Result<i32> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "only macOS and Windows read the local time zone here",
    ))
}

/// The calendar date `days` days after 1970-01-01, as year, month (1 to
/// 12) and day (1 to 31), in the proleptic Gregorian calendar (Howard
/// Hinnant's `civil_from_days`). Negative days are before 1970.
pub fn civil_date(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// A moment as a calendar date and a time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilDateTime {
    pub year: i64,
    /// 1 to 12.
    pub month: i64,
    /// 1 to 31.
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
}

/// The date and time `unix_seconds` after 1970-01-01 00:00 UTC, in a time
/// zone `offset_seconds` ahead of UTC (0 for UTC itself, or what
/// [`local_utc_offset_seconds`] says for local time). `None` for a moment
/// so far from 1970 that the arithmetic would overflow.
pub fn civil_date_time(unix_seconds: i64, offset_seconds: i32) -> Option<CivilDateTime> {
    let local = unix_seconds.checked_add(i64::from(offset_seconds))?;
    let (year, month, day) = civil_date(local.div_euclid(86_400));
    let seconds_of_day = local.rem_euclid(86_400);
    Some(CivilDateTime {
        year,
        month,
        day,
        hour: seconds_of_day / 3600,
        minute: seconds_of_day % 3600 / 60,
        second: seconds_of_day % 60,
    })
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::{CivilDateTime, civil_date, civil_date_time, local_utc_offset_seconds};

    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn only_macos_and_windows_read_the_offset() {
        assert_eq!(
            local_utc_offset_seconds(SystemTime::now())
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::Unsupported
        );
    }

    /// Every time zone on Earth is within 14 hours of UTC, and in whole
    /// quarter hours: now, in 1970, in winter and summer of one year, and
    /// after 2038, where 32-bit seconds end.
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn the_offset_is_one_a_time_zone_can_have() {
        use std::time::Duration;
        let moments = [
            SystemTime::now(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(43_200),
            // 15 January and 15 July 2026, noon UTC.
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_768_478_400),
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_784_116_800),
            // 2040-01-01.
            SystemTime::UNIX_EPOCH + Duration::from_secs(2_208_988_800),
        ];
        for at in moments {
            let offset = local_utc_offset_seconds(at).unwrap();
            assert!(offset.abs() <= 14 * 3600, "{at:?}: {offset}");
            assert_eq!(offset % (15 * 60), 0, "{at:?}: {offset}");
        }
    }

    #[test]
    fn civil_date_knows_the_calendar() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(-1), (1969, 12, 31));
        assert_eq!(civil_date(10_957), (2000, 1, 1));
        // 2000 is a leap year, as every fourth century is.
        assert_eq!(civil_date(10_957 + 59), (2000, 2, 29));
        assert_eq!(civil_date(10_957 + 60), (2000, 3, 1));
        assert_eq!(civil_date(19_723 + 59), (2024, 2, 29));
        // 2^31 seconds after 1970, where 32-bit time ends.
        assert_eq!(civil_date(24_855), (2038, 1, 19));
        // 2100 is no leap year.
        assert_eq!(civil_date(47_482), (2100, 1, 1));
        assert_eq!(civil_date(47_482 + 59), (2100, 3, 1));
    }

    #[test]
    fn civil_date_time_adds_the_offset_across_midnight() {
        let at = |year, month, day, hour, minute, second| CivilDateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
        };
        assert_eq!(
            civil_date_time(1_709_208_000, 0),
            Some(at(2024, 2, 29, 12, 0, 0))
        );
        assert_eq!(
            civil_date_time(1_709_208_000, 3600),
            Some(at(2024, 2, 29, 13, 0, 0))
        );
        assert_eq!(civil_date_time(0, -3600), Some(at(1969, 12, 31, 23, 0, 0)));
        assert_eq!(
            civil_date_time(2_147_483_648, 0),
            Some(at(2038, 1, 19, 3, 14, 8))
        );
        assert_eq!(
            civil_date_time(1_709_251_199, 5 * 3600 + 45 * 60),
            Some(at(2024, 3, 1, 5, 44, 59))
        );
        assert_eq!(civil_date_time(i64::MAX, 3600), None);
    }
}
