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
    index: u64,
}

/// What the file or folder at `path` itself is, never what a link there
/// leads to: a symbolic link or a junction is a file of its own. Folders on
/// the way are followed as usual.
///
/// On Windows that is the volume's serial number and the file's index on
/// it, read through a handle that asks only for its attributes, which no
/// other program's sharing refuses. On Unix it is the device and inode
/// numbers from `lstat`. Elsewhere `Unsupported`.
pub fn file_identity(path: &Path) -> io::Result<FileIdentity> {
    platform_file_identity(path)
}

#[cfg(windows)]
fn platform_file_identity(path: &Path) -> io::Result<FileIdentity> {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_READ_ATTRIBUTES, GetFileInformationByHandle,
    };

    let file = crate::win_util::open_no_follow(path, FILE_READ_ATTRIBUTES)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `info` is a BY_HANDLE_FILE_INFORMATION Windows fills in, and
    // the handle is open for the whole call.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FileIdentity {
        volume: u64::from(info.dwVolumeSerialNumber),
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

#[cfg(unix)]
fn platform_file_identity(path: &Path) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = std::fs::symlink_metadata(path)?;
    Ok(FileIdentity {
        volume: metadata.dev(),
        index: metadata.ino(),
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
