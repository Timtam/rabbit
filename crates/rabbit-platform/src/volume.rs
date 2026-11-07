//! Which volume a path is on, and how much room that volume has.
//!
//! An uninstall moves each file into a backup set inside the resource
//! folder. On the same volume that is a rename, instant even for FFmpeg's
//! 250 MB. A file on another volume has to be copied, checked and deleted
//! instead, and then the backup's volume needs room for it first: a copy
//! that runs out of space halfway helps nobody. These two questions decide
//! that before anything moves.
//!
//! Both answer `None` when they can't tell, and callers treat that as the
//! careful case: a volume they can't compare counts as another volume, and
//! space they can't measure isn't assumed to be there.

use std::io;
use std::path::Path;

/// Whether `left` and `right`, which must both exist, are on the same
/// volume, so a rename between them moves a file without copying it.
///
/// On Windows this compares the folders the volumes are mounted at, so a
/// volume mounted twice reads as two volumes; the answer is careful, never
/// wrong in the dangerous direction. On macOS it compares device numbers.
pub fn same_volume(left: &Path, right: &Path) -> Option<bool> {
    platform_same_volume(left, right)
}

/// The bytes this user may still write on the volume holding `path`, which
/// must exist. On Windows that respects disk quotas; on macOS it leaves out
/// the blocks only the administrator may use.
pub fn available_space(path: &Path) -> Option<u64> {
    platform_available_space(path)
}

/// The volume `path` itself is on (`st_dev`), read with `lstat`, so a link
/// is never followed. A folder another volume is mounted on reads as that
/// volume, unlike the folder holding it.
///
/// The macOS administrator command deletes with `rm -R -x`, which stays out
/// of a volume mounted below the folder it is given but not out of one
/// mounted on that folder itself: comparing a folder's volume with its
/// parent's tells the two apart. Unix only; elsewhere `Unsupported`.
pub fn volume_id(path: &Path) -> io::Result<u64> {
    platform_volume_id(path)
}

#[cfg(windows)]
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// The folder the volume holding `path` is mounted at, such as `C:\`.
#[cfg(windows)]
fn volume_mount_point(path: &Path) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW;

    let path = wide(path);
    // The longest answer is a path, and Windows' long paths stay below
    // 32,767 characters.
    let mut buffer = vec![0_u16; 32_768];
    // SAFETY: `path` is NUL-terminated and the buffer length passed is the
    // buffer's real length, so Windows writes only inside it.
    let found =
        unsafe { GetVolumePathNameW(path.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
    if found == 0 {
        return None;
    }
    let length = buffer.iter().position(|unit| *unit == 0)?;
    Some(String::from_utf16_lossy(&buffer[..length]).to_lowercase())
}

#[cfg(windows)]
fn platform_same_volume(left: &Path, right: &Path) -> Option<bool> {
    Some(volume_mount_point(left)? == volume_mount_point(right)?)
}

#[cfg(unix)]
fn platform_same_volume(left: &Path, right: &Path) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;

    Some(left.metadata().ok()?.dev() == right.metadata().ok()?.dev())
}

#[cfg(not(any(windows, unix)))]
fn platform_same_volume(_left: &Path, _right: &Path) -> Option<bool> {
    None
}

#[cfg(unix)]
fn platform_volume_id(path: &Path) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;

    Ok(std::fs::symlink_metadata(path)?.dev())
}

#[cfg(not(unix))]
fn platform_volume_id(_path: &Path) -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "only Unix names the volume a path is on by a device number",
    ))
}

#[cfg(windows)]
fn platform_available_space(path: &Path) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let path = wide(path);
    let mut available: u64 = 0;
    // SAFETY: `path` is NUL-terminated, `available` is a valid place for one
    // u64, and the two totals RABBIT doesn't need may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(available)
}

#[cfg(target_os = "macos")]
fn platform_available_space(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    // statfs rather than statvfs: on macOS statvfs counts blocks in 32 bits,
    // which overflows on a large drive.
    // SAFETY: statfs is plain data, for which all zeros is a valid value.
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is NUL-terminated and `stat` is a whole, writable
    // statfs for the call to fill in.
    if unsafe { libc::statfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    u64::from(stat.f_bsize).checked_mul(stat.f_bavail)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_available_space(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::{available_space, same_volume, volume_id};

    #[test]
    fn a_folder_and_its_parent_share_a_volume_id_only_on_unix() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        if cfg!(unix) {
            assert_eq!(volume_id(&inner).unwrap(), volume_id(dir.path()).unwrap());
            assert_eq!(
                volume_id(&dir.path().join("missing")).unwrap_err().kind(),
                std::io::ErrorKind::NotFound
            );
        } else {
            assert_eq!(
                volume_id(&inner).unwrap_err().kind(),
                std::io::ErrorKind::Unsupported
            );
        }
    }

    #[test]
    fn a_folder_is_on_its_own_volume_and_has_some_room() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        if cfg!(any(windows, target_os = "macos")) {
            assert_eq!(same_volume(dir.path(), &inner), Some(true));
            assert!(available_space(dir.path()).is_some_and(|bytes| bytes > 0));
        }
    }

    #[test]
    fn a_path_that_does_not_exist_has_no_answer() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        if cfg!(unix) {
            assert_eq!(same_volume(&missing, dir.path()), None);
        }
        if cfg!(target_os = "macos") {
            assert_eq!(available_space(&missing), None);
        }
    }
}
