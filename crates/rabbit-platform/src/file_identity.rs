//! Which file a path names, so two names of one file can be told from two
//! files.
//!
//! A path can name a file in more ways than its own spelling: on Windows by
//! its short 8.3 name or in another case, on macOS in another case or
//! another Unicode form, and anywhere through a hard link. A delete that
//! takes one backup out of a set compares what it is about to remove with
//! the files the set still needs, by what each path opens, never by how it
//! is spelled.

use std::io;
use std::path::Path;

/// What a file or folder is, whatever path names it: its volume and its
/// number on that volume. Two paths name the same file when these are
/// equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileIdentity {
    volume: u64,
    index: u128,
}

/// What the file or folder at `path` itself is, never what a link there
/// leads to: a symbolic link or a junction is a file of its own. Folders on
/// the way are followed as usual.
///
/// On Windows that is the volume's serial number and the file's 128-bit
/// id on it, read through a handle that asks only for its attributes,
/// which no other program's sharing refuses. ReFS, Dev Drives included,
/// needs all 128 bits: the older 64-bit file index isn't unique there. A
/// file system that has no 128-bit ids gives the 64-bit index instead,
/// for every file on it alike. A number of 0 or all ones, as some network
/// shares give every file, says nothing of which file it is, and is an
/// error, so two files never count as one. On Unix it is the device and
/// inode numbers from `lstat`. Elsewhere `Unsupported`.
pub fn file_identity(path: &Path) -> io::Result<FileIdentity> {
    platform_file_identity(path)
}

#[cfg(windows)]
fn platform_file_identity(path: &Path) -> io::Result<FileIdentity> {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ID_INFO, FILE_READ_ATTRIBUTES, FileIdInfo,
        GetFileInformationByHandle, GetFileInformationByHandleEx,
    };

    let file = crate::win_util::open_no_follow(path, FILE_READ_ATTRIBUTES)?;
    let mut id = FILE_ID_INFO::default();
    // SAFETY: `id` is a FILE_ID_INFO and the size passed is its size; the
    // handle is open for the whole call.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&raw mut id).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if ok != 0 {
        return numbered(
            id.VolumeSerialNumber,
            u128::from_le_bytes(id.FileId.Identifier),
            u128::MAX,
        );
    }
    // Only a file system that has no 128-bit ids at all gets the 64-bit
    // index, so the two kinds of number never meet for one file system.
    let error = io::Error::last_os_error();
    if !has_no_128_bit_ids(&error) {
        return Err(error);
    }
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `info` is a BY_HANDLE_FILE_INFORMATION Windows fills in, and
    // the handle is open for the whole call.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    numbered(
        u64::from(info.dwVolumeSerialNumber),
        (u128::from(info.nFileIndexHigh) << 32) | u128::from(info.nFileIndexLow),
        u128::from(u64::MAX),
    )
}

/// Whether `error`, from asking a file for its 128-bit id, says that its
/// file system has none, as FAT and some network shares say.
#[cfg(windows)]
fn has_no_128_bit_ids(error: &io::Error) -> bool {
    const ERROR_INVALID_FUNCTION: i32 = 1;
    const ERROR_NOT_SUPPORTED: i32 = 50;
    const ERROR_INVALID_PARAMETER: i32 = 87;
    const ERROR_CALL_NOT_IMPLEMENTED: i32 = 120;
    const ERROR_INVALID_LEVEL: i32 = 124;
    matches!(
        error.raw_os_error(),
        Some(
            ERROR_INVALID_FUNCTION
                | ERROR_NOT_SUPPORTED
                | ERROR_INVALID_PARAMETER
                | ERROR_CALL_NOT_IMPLEMENTED
                | ERROR_INVALID_LEVEL
        )
    )
}

/// The identity of the file numbered `index` on the volume `volume`, or an
/// error when the number is 0 or `none`, all ones, which a file system
/// gives a file it has no number for.
#[cfg(windows)]
fn numbered(volume: u64, index: u128, none: u128) -> io::Result<FileIdentity> {
    if index == 0 || index == none {
        return Err(io::Error::other(
            "the file system gives this file no number of its own, so it can't be told apart \
             from other files",
        ));
    }
    Ok(FileIdentity { volume, index })
}

#[cfg(unix)]
fn platform_file_identity(path: &Path) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = std::fs::symlink_metadata(path)?;
    Ok(FileIdentity {
        volume: metadata.dev(),
        index: u128::from(metadata.ino()),
    })
}

#[cfg(not(any(windows, unix)))]
fn platform_file_identity(_path: &Path) -> io::Result<FileIdentity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "only Windows and Unix say which file a path names",
    ))
}

#[cfg(test)]
mod tests {
    use super::file_identity;

    /// Two names of one file, as a hard link makes, are the same file; two
    /// files with the same contents are not. A file that isn't there is an
    /// error.
    #[test]
    fn two_names_of_one_file_are_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.dll");
        let twin = dir.path().join("twin.dll");
        let other = dir.path().join("other.dll");
        std::fs::write(&first, b"same").unwrap();
        std::fs::write(&other, b"same").unwrap();
        std::fs::hard_link(&first, &twin).unwrap();

        let identity = file_identity(&first).unwrap();

        assert_eq!(file_identity(&twin).unwrap(), identity);
        assert_ne!(file_identity(&other).unwrap(), identity);
        assert!(file_identity(&dir.path().join("missing.dll")).is_err());
    }

    /// A number of 0 or all ones names no file of its own, as some network
    /// shares give every file, so it is an error rather than an identity
    /// two files could share; any other number is one.
    #[cfg(windows)]
    #[test]
    fn a_number_of_0_or_all_ones_is_no_identity() {
        use super::numbered;

        for (index, none) in [
            (0, u128::MAX),
            (u128::MAX, u128::MAX),
            (0, u128::from(u64::MAX)),
            (u128::from(u64::MAX), u128::from(u64::MAX)),
        ] {
            assert!(numbered(7, index, none).is_err(), "{index:#x}");
        }
        assert_eq!(
            numbered(7, 1 << 64, u128::MAX).unwrap(),
            numbered(7, 1 << 64, u128::MAX).unwrap()
        );
        assert_ne!(
            numbered(7, (1 << 64) | 5, u128::MAX).unwrap(),
            numbered(7, 5, u128::MAX).unwrap()
        );
        assert_ne!(
            numbered(7, 5, u128::MAX).unwrap(),
            numbered(8, 5, u128::MAX).unwrap()
        );
    }

    /// Where the file system ignores case, a name in another case is the
    /// same file.
    #[cfg(windows)]
    #[test]
    fn a_name_in_another_case_is_the_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("backup-manifest.json");
        std::fs::write(&file, b"{}").unwrap();

        assert_eq!(
            file_identity(&dir.path().join("Backup-Manifest.JSON")).unwrap(),
            file_identity(&file).unwrap()
        );
    }

    /// A symbolic link is a file of its own, never the file it leads to.
    #[cfg(unix)]
    #[test]
    fn a_link_is_never_the_file_it_leads_to() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("target.dll");
        let link = dir.path().join("link.dll");
        std::fs::write(&file, b"target").unwrap();
        std::os::unix::fs::symlink(&file, &link).unwrap();

        assert_ne!(file_identity(&link).unwrap(), file_identity(&file).unwrap());
    }
}
