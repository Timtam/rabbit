//! Removing junctions and empty folders without ever following a link.
//!
//! Surge XT's installer makes `SurgeXTData` junctions in the CLAP and VST3
//! `Surge Synth Team` folders that point into `C:\ProgramData\Surge XT`,
//! and its uninstaller leaves them behind. RABBIT removes them as links,
//! never touching what they point to, and then the `Surge Synth Team`
//! folders, but only when they are empty.
//!
//! Each function opens the path itself, never a link's target, checks what
//! it opened, and deletes through that same handle, so the thing checked is
//! the thing deleted. Links further up the path are followed as usual: the
//! caller's root guard checks those.
//!
//! Windows only; elsewhere both fail as `Unsupported`.

use std::io;
use std::path::Path;

/// Remove the junction at `path`: the link itself, never the folder it
/// points to or anything in it.
///
/// Refused, as `InvalidInput`, unless `path` is a junction (a folder
/// reparse point tagged `IO_REPARSE_TAG_MOUNT_POINT`). A plain folder, a
/// file, a symbolic link or any other reparse point stays.
pub fn remove_junction(path: &Path) -> io::Result<()> {
    imp::remove_junction(path)
}

/// Remove the folder at `path` if it is empty. Not recursive: a folder
/// with anything in it fails with `DirectoryNotEmpty` and stays. Refused,
/// as `InvalidInput`, when `path` is a reparse point of any kind, or not a
/// folder.
pub fn remove_empty_dir(path: &Path) -> io::Result<()> {
    imp::remove_empty_dir(path)
}

#[cfg(windows)]
mod imp {
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DISPOSITION_INFO,
        FILE_READ_ATTRIBUTES, FileDispositionInfo, SetFileInformationByHandle,
    };

    use crate::win_util::{attribute_tag, open_no_follow};

    /// `IO_REPARSE_TAG_MOUNT_POINT` from `winnt.h`: junctions, and folders
    /// a volume is mounted on. Spelled out because it is the only thing
    /// this module needs from windows-sys's large SystemServices module.
    const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;

    pub(super) fn remove_junction(path: &Path) -> io::Result<()> {
        let handle = open_no_follow(path, DELETE | FILE_READ_ATTRIBUTES)?;
        let info = attribute_tag(&handle)?;
        let is_junction = info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            && info.FileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
            && info.ReparseTag == IO_REPARSE_TAG_MOUNT_POINT;
        if !is_junction {
            return Err(refused(path, "isn't a junction"));
        }
        delete_on_close(&handle)
    }

    pub(super) fn remove_empty_dir(path: &Path) -> io::Result<()> {
        let handle = open_no_follow(path, DELETE | FILE_READ_ATTRIBUTES)?;
        let attributes = attribute_tag(&handle)?.FileAttributes;
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(refused(path, "is a link"));
        }
        if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(refused(path, "isn't a folder"));
        }
        // Windows refuses this for a folder that isn't empty.
        delete_on_close(&handle)
    }

    /// Mark the open file or folder for deletion. It goes when the last
    /// handle to it closes; RABBIT's closes when `handle` drops.
    fn delete_on_close(handle: &File) -> io::Result<()> {
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        // SAFETY: the handle is open with DELETE access, and `disposition`
        // is a FILE_DISPOSITION_INFO of the size passed.
        let ok = unsafe {
            SetFileInformationByHandle(
                handle.as_raw_handle(),
                FileDispositionInfo,
                (&raw const disposition).cast(),
                std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn refused(path: &Path, why: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("RABBIT won't remove {}: it {why}", path.display()),
        )
    }
}

#[cfg(not(windows))]
mod imp {
    use std::io;
    use std::path::Path;

    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "junctions only exist on Windows",
        )
    }

    pub(super) fn remove_junction(_path: &Path) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn remove_empty_dir(_path: &Path) -> io::Result<()> {
        Err(unsupported())
    }
}

/// Make `link` a junction to the folder `target` with `mklink /J`, which
/// needs no administrator rights, unlike a symbolic link.
#[cfg(all(test, windows))]
pub(crate) fn make_junction(link: &Path, target: &Path) {
    // cmd reads a `/` in a path as the start of an option.
    let windows_path = |path: &Path| path.to_string_lossy().replace('/', "\\");
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(windows_path(link))
        .arg(windows_path(target))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    #[test]
    fn off_windows_nothing_is_removed() {
        let scratch = tempfile::tempdir().unwrap();
        let folder = scratch.path().join("empty");
        std::fs::create_dir(&folder).unwrap();
        for error in [
            super::remove_junction(&folder).unwrap_err(),
            super::remove_empty_dir(&folder).unwrap_err(),
        ] {
            assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        }
        assert!(folder.is_dir());
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use super::*;

    /// A Surge-like layout in a temp folder: `data\SurgeXTData` with a
    /// file in it, and `plugins\Surge Synth Team\SurgeXTData` as a junction
    /// to it.
    struct Layout {
        _scratch: tempfile::TempDir,
        data_file: PathBuf,
        vendor_folder: PathBuf,
        junction: PathBuf,
    }

    fn layout() -> Layout {
        let scratch = tempfile::tempdir().unwrap();
        let data = scratch.path().join("data").join("SurgeXTData");
        fs::create_dir_all(&data).unwrap();
        let data_file = data.join("patch.fxp");
        fs::write(&data_file, b"user patch").unwrap();
        let vendor_folder = scratch.path().join("plugins").join("Surge Synth Team");
        fs::create_dir_all(&vendor_folder).unwrap();
        let junction = vendor_folder.join("SurgeXTData");
        make_junction(&junction, &data);
        Layout {
            _scratch: scratch,
            data_file,
            vendor_folder,
            junction,
        }
    }

    fn exists_itself(path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok()
    }

    #[test]
    fn removing_a_junction_removes_only_the_link() {
        let layout = layout();

        remove_junction(&layout.junction).unwrap();

        assert!(!exists_itself(&layout.junction));
        assert_eq!(fs::read(&layout.data_file).unwrap(), b"user patch");
        // The folder it sat in is now empty, and goes too.
        remove_empty_dir(&layout.vendor_folder).unwrap();
        assert!(!exists_itself(&layout.vendor_folder));
        assert_eq!(fs::read(&layout.data_file).unwrap(), b"user patch");
    }

    #[test]
    fn only_junctions_are_removed_as_junctions() {
        let layout = layout();
        let plain_file = layout.vendor_folder.join("readme.txt");
        fs::write(&plain_file, b"text").unwrap();
        let data_folder = layout.data_file.parent().unwrap();

        for path in [
            &layout.vendor_folder,
            &plain_file,
            &data_folder.to_path_buf(),
        ] {
            let error = remove_junction(path).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{path:?}");
            assert!(exists_itself(path), "{path:?}");
        }
        assert_eq!(fs::read(&layout.data_file).unwrap(), b"user patch");
        assert_eq!(
            remove_junction(&layout.vendor_folder.join("missing"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn an_empty_folder_check_never_goes_through_a_junction() {
        let layout = layout();

        // The junction leads to a folder with a file in it; as a folder
        // removal it is refused for being a link, whatever is behind it.
        let error = remove_empty_dir(&layout.junction).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(exists_itself(&layout.junction));
        assert_eq!(fs::read(&layout.data_file).unwrap(), b"user patch");

        // A folder that still holds the junction isn't empty.
        let error = remove_empty_dir(&layout.vendor_folder).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::DirectoryNotEmpty);
        assert!(exists_itself(&layout.junction));

        // A file isn't a folder.
        let error = remove_empty_dir(&layout.data_file).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(layout.data_file.is_file());
    }

    #[test]
    fn a_junction_to_a_missing_folder_is_still_removed() {
        let layout = layout();
        fs::remove_file(&layout.data_file).unwrap();
        fs::remove_dir(layout.data_file.parent().unwrap()).unwrap();

        remove_junction(&layout.junction).unwrap();

        assert!(!exists_itself(&layout.junction));
    }
}
