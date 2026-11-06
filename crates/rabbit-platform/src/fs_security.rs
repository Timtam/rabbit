//! File-system checks and guards for running a vendor uninstaller with
//! administrator rights.
//!
//! - [`create_admin_only_directory`] makes a folder only administrators and
//!   SYSTEM can write to, for REAPER's NSIS uninstaller copy.
//! - [`file_owner_is_trusted`] and [`is_reparse_point`] let the elevated
//!   helper refuse files a standard user could have planted. In a folder
//!   such as `C:\ProgramData\Surge XT\uninstall`, standard users may create
//!   files, and a file a standard user creates is owned by that user.
//! - [`LockedFile`] keeps a file from changing between its last check and
//!   the moment it starts.
//!
//! All of these are Windows only. Elsewhere they fail as `Unsupported`,
//! except [`is_reparse_point`], which reports symbolic links.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// A protected DACL granting full control to SYSTEM and the Administrators
/// group and to nobody else, inherited by everything created inside.
/// "Protected" (`P`) means nothing is inherited from the parent folder, so
/// the rights standard users have in `C:\ProgramData` don't reach in.
pub const ADMINISTRATORS_ONLY_SDDL: &str = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";

/// The owners an elevated step trusts: the Administrators group, SYSTEM
/// and TrustedInstaller. A standard user can't make any of them the owner
/// of a file.
pub const TRUSTED_OWNER_SIDS: [&str; 3] = [
    "S-1-5-32-544",
    "S-1-5-18",
    "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464",
];

/// How many fresh names [`create_admin_only_directory`] tries. Each has
/// 128 random bits, so a second try only ever happens on purpose.
#[cfg(windows)]
const NAME_ATTEMPTS: usize = 4;

/// Create a new folder `<parent>\<name_prefix>-<32 hex digits>` that only
/// SYSTEM and the Administrators group can use ([`ADMINISTRATORS_ONLY_SDDL`]),
/// and return its path.
///
/// The folder is created with that DACL in one step, so it never exists
/// with other rights, and a name that already exists is never reused. Only
/// an elevated process can then put files in it. Under `C:\ProgramData`,
/// standard users may create folders but have no right to delete what
/// others created there, so they can't rename or remove this one either.
///
/// `name_prefix` must be ASCII letters, digits, `-`, `_` or `.`, and
/// `parent` an absolute path. The folder's owner is whoever runs this,
/// usually the Administrators group for an elevated process.
pub fn create_admin_only_directory(parent: &Path, name_prefix: &str) -> io::Result<PathBuf> {
    create_unique_directory(parent, name_prefix, ADMINISTRATORS_ONLY_SDDL)
}

/// `C:\ProgramData` as Windows names it (`FOLDERID_ProgramData`), not as
/// an environment variable says, so a changed environment can't move it.
pub fn program_data_dir() -> io::Result<PathBuf> {
    imp::known_folder(imp::KnownFolder::ProgramData)
}

/// The Program Files folders as Windows names them, the native one first:
/// `C:\Program Files`, then `C:\Program Files (x86)` where Windows has one.
/// Read like [`program_data_dir`], so the elevated helper finds them
/// whatever environment it was started with.
pub fn machine_program_files_dirs() -> io::Result<Vec<PathBuf>> {
    let mut folders = vec![imp::known_folder(imp::KnownFolder::ProgramFiles)?];
    // 64-bit Windows always has the 32-bit folder; a Windows without one
    // simply has nothing more to list.
    if let Ok(x86) = imp::known_folder(imp::KnownFolder::ProgramFilesX86)
        && !folders
            .iter()
            .any(|known| known.as_os_str().eq_ignore_ascii_case(x86.as_os_str()))
    {
        folders.push(x86);
    }
    Ok(folders)
}

/// `C:\Program Files\Common Files` as Windows names it
/// (`FOLDERID_ProgramFilesCommon`), read like [`program_data_dir`].
pub fn machine_common_program_files_dir() -> io::Result<PathBuf> {
    imp::known_folder(imp::KnownFolder::ProgramFilesCommon)
}

/// The user's Documents folder as Windows names it (`FOLDERID_Documents`),
/// wherever the user moved it, OneDrive included. An environment variable
/// can't name it.
pub(crate) fn windows_documents_dir() -> io::Result<PathBuf> {
    imp::known_folder(imp::KnownFolder::Documents)
}

/// Whether `sid` is one of [`TRUSTED_OWNER_SIDS`].
pub fn is_trusted_owner_sid(sid: &str) -> bool {
    TRUSTED_OWNER_SIDS
        .iter()
        .any(|trusted| trusted.eq_ignore_ascii_case(sid))
}

/// The SID that owns `path`, read from `path` itself: a link is never
/// followed, so a link's own owner is what comes back.
pub fn file_owner_sid(path: &Path) -> io::Result<String> {
    imp::file_owner_sid(path)
}

/// Whether `path` is owned by the Administrators group, SYSTEM or
/// TrustedInstaller ([`TRUSTED_OWNER_SIDS`]). Links are never followed.
///
/// A file an administrator created can also be owned by that
/// administrator's own account; this says `false` for it, which is careful
/// rather than wrong. Vendor uninstallers installed by an elevated
/// installer are owned by the Administrators group.
pub fn file_owner_is_trusted(path: &Path) -> io::Result<bool> {
    Ok(is_trusted_owner_sid(&file_owner_sid(path)?))
}

/// Whether `path` itself is a reparse point: a symbolic link, a junction,
/// or anything else Windows redirects, such as a cloud placeholder. Never
/// follows it. Elsewhere, whether it is a symbolic link.
pub fn is_reparse_point(path: &Path) -> io::Result<bool> {
    imp::is_reparse_point(path)
}

/// A file held open so that nobody can change, rename or delete it, or
/// rename the folder it is in, while the guard lives. Others may still
/// read it, and start it as a program.
///
/// Opening fails when anyone already has the file open for writing or
/// deleting, and when `path` is a link or a folder. Hash the file through
/// [`LockedFile::file`], so the hash is of exactly what is locked.
///
/// Drop the guard once the program has started: a vendor uninstaller may
/// want to delete its own file, and can't while this is held.
#[derive(Debug)]
pub struct LockedFile {
    file: File,
    path: PathBuf,
}

impl LockedFile {
    pub fn open(path: &Path) -> io::Result<Self> {
        Ok(Self {
            file: imp::open_locked(path)?,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The open file, for reading. Reading moves the shared position, so
    /// seek back to the start before reading it a second time.
    pub fn file(&self) -> &File {
        &self.file
    }

    /// The SID that owns the locked file, read through the open handle.
    pub fn owner_sid(&self) -> io::Result<String> {
        imp::handle_owner_sid(&self.file)
    }

    /// Whether the locked file's owner is one of [`TRUSTED_OWNER_SIDS`].
    pub fn owner_is_trusted(&self) -> io::Result<bool> {
        Ok(is_trusted_owner_sid(&self.owner_sid()?))
    }
}

fn create_unique_directory(parent: &Path, name_prefix: &str, sddl: &str) -> io::Result<PathBuf> {
    let prefix_ok = !name_prefix.is_empty()
        && name_prefix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if !prefix_ok || name_prefix.starts_with('.') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name_prefix:?} can't start a folder name"),
        ));
    }
    if !parent.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} isn't an absolute path", parent.display()),
        ));
    }
    imp::create_unique_directory(parent, name_prefix, sddl)
}

#[cfg(windows)]
mod imp {
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateDirectoryW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, READ_CONTROL,
    };
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_Documents, FOLDERID_ProgramData, FOLDERID_ProgramFiles,
        FOLDERID_ProgramFilesCommon, FOLDERID_ProgramFilesX86, KF_FLAG_DEFAULT,
        SHGetKnownFolderPath,
    };

    use super::NAME_ATTEMPTS;
    use crate::win_util::{
        attribute_tag, open_no_follow, owner_sid_of, random_hex, security_descriptor_from_sddl,
        wide_path,
    };

    pub(super) fn create_unique_directory(
        parent: &Path,
        name_prefix: &str,
        sddl: &str,
    ) -> io::Result<PathBuf> {
        let descriptor = security_descriptor_from_sddl(sddl)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.as_ptr(),
            bInheritHandle: 0,
        };
        for _ in 0..NAME_ATTEMPTS {
            let path = parent.join(format!("{name_prefix}-{}", random_hex::<16>()?));
            let wide = wide_path(&path)?;
            // SAFETY: `wide` is NUL-terminated, and `attributes` points to a
            // descriptor that lives until the end of this function.
            if unsafe { CreateDirectoryW(wide.as_ptr(), &attributes) } != 0 {
                return Ok(path);
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32) {
                return Err(error);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "every new folder name tried under {} already existed",
                parent.display()
            ),
        ))
    }

    /// The folders RABBIT asks Windows for by name: the machine-wide ones,
    /// and the user's Documents.
    #[derive(Debug, Clone, Copy)]
    pub(super) enum KnownFolder {
        ProgramData,
        ProgramFiles,
        ProgramFilesX86,
        ProgramFilesCommon,
        Documents,
    }

    pub(super) fn known_folder(folder: KnownFolder) -> io::Result<PathBuf> {
        let id = match folder {
            KnownFolder::ProgramData => &FOLDERID_ProgramData,
            KnownFolder::ProgramFiles => &FOLDERID_ProgramFiles,
            KnownFolder::ProgramFilesX86 => &FOLDERID_ProgramFilesX86,
            KnownFolder::ProgramFilesCommon => &FOLDERID_ProgramFilesCommon,
            KnownFolder::Documents => &FOLDERID_Documents,
        };
        let mut path = std::ptr::null_mut();
        // SAFETY: a null token means the current user; `path` is a valid
        // place for the string Windows allocates.
        let result = unsafe {
            SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as u32, std::ptr::null_mut(), &mut path)
        };
        let found = if result >= 0 && !path.is_null() {
            let mut length = 0;
            // SAFETY: on success Windows returned a NUL-terminated string.
            while unsafe { *path.add(length) } != 0 {
                length += 1;
            }
            // SAFETY: the `length` units before the NUL belong to it.
            let units = unsafe { std::slice::from_raw_parts(path, length) };
            Ok(PathBuf::from(std::ffi::OsString::from_wide(units)))
        } else {
            Err(io::Error::from_raw_os_error(result))
        };
        // SAFETY: Windows says to free the string whether or not the call
        // succeeded; freeing null does nothing.
        unsafe { CoTaskMemFree(path.cast()) };
        found
    }

    pub(super) fn file_owner_sid(path: &Path) -> io::Result<String> {
        let file = open_no_follow(path, READ_CONTROL)?;
        handle_owner_sid(&file)
    }

    pub(super) fn handle_owner_sid(file: &File) -> io::Result<String> {
        owner_sid_of(file.as_raw_handle())
    }

    pub(super) fn is_reparse_point(path: &Path) -> io::Result<bool> {
        let file = open_no_follow(path, FILE_READ_ATTRIBUTES)?;
        Ok(attribute_tag(&file)?.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }

    pub(super) fn open_locked(path: &Path) -> io::Result<File> {
        // Sharing only reading refuses every later open for writing or
        // deleting, and fails now if one is already open. Renaming the
        // file, or the folder it is in, needs such an open too.
        let file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        let attributes = attribute_tag(&file)?.FileAttributes;
        if attributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} is a link or a folder, not a program file",
                    path.display()
                ),
            ));
        }
        Ok(file)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::fs::File;
    use std::io;
    use std::path::{Path, PathBuf};

    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "this check only exists on Windows",
        )
    }

    pub(super) fn create_unique_directory(
        _parent: &Path,
        _name_prefix: &str,
        _sddl: &str,
    ) -> io::Result<PathBuf> {
        Err(unsupported())
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) enum KnownFolder {
        ProgramData,
        ProgramFiles,
        ProgramFilesX86,
        ProgramFilesCommon,
        Documents,
    }

    pub(super) fn known_folder(_folder: KnownFolder) -> io::Result<PathBuf> {
        Err(unsupported())
    }

    pub(super) fn file_owner_sid(_path: &Path) -> io::Result<String> {
        Err(unsupported())
    }

    pub(super) fn handle_owner_sid(_file: &File) -> io::Result<String> {
        Err(unsupported())
    }

    pub(super) fn is_reparse_point(path: &Path) -> io::Result<bool> {
        Ok(std::fs::symlink_metadata(path)?.file_type().is_symlink())
    }

    pub(super) fn open_locked(_path: &Path) -> io::Result<File> {
        Err(unsupported())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_administrators_system_and_trusted_installer_are_trusted_owners() {
        assert!(is_trusted_owner_sid("S-1-5-32-544"));
        assert!(is_trusted_owner_sid("S-1-5-18"));
        assert!(is_trusted_owner_sid(
            "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"
        ));
        for sid in [
            // Users, Authenticated Users, Everyone, a user account.
            "S-1-5-32-545",
            "S-1-5-11",
            "S-1-1-0",
            "S-1-5-21-1004336348-1177238915-682003330-1001",
            "S-1-5-32-5440",
            "",
        ] {
            assert!(!is_trusted_owner_sid(sid), "{sid}");
        }
    }

    #[test]
    fn folder_names_and_parents_are_checked_before_anything_is_created() {
        let parent = std::env::temp_dir();
        for prefix in ["", "..", ".hidden", "a\\b", "a/b", "a b", "naïve"] {
            let error = create_admin_only_directory(&parent, prefix).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{prefix:?}");
        }
        let error = create_admin_only_directory(Path::new("relative"), "rabbit").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_the_windows_checks_are_unsupported() {
        let scratch = tempfile::tempdir().unwrap();
        let file = scratch.path().join("file");
        std::fs::write(&file, b"x").unwrap();
        for error in [
            create_admin_only_directory(scratch.path(), "rabbit").unwrap_err(),
            program_data_dir().unwrap_err(),
            machine_program_files_dirs().unwrap_err(),
            machine_common_program_files_dir().unwrap_err(),
            file_owner_sid(&file).unwrap_err(),
            LockedFile::open(&file).unwrap_err(),
        ] {
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        }
        assert!(!is_reparse_point(&file).unwrap());
        let link = scratch.path().join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(is_reparse_point(&link).unwrap());
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use std::fs::{self, OpenOptions};
    use std::io::Read;

    use super::*;
    use crate::win_util::current_user_sid;
    use crate::win_util::test_support::{aces, dacl_sddl_of, descriptor_sddl};

    /// The DACL of `path` in SDDL, read through a handle that doesn't
    /// follow links.
    fn dacl_sddl(path: &Path) -> String {
        use std::os::windows::io::AsRawHandle;

        use windows_sys::Win32::Security::Authorization::SE_FILE_OBJECT;
        use windows_sys::Win32::Storage::FileSystem::READ_CONTROL;

        let file = crate::win_util::open_no_follow(path, READ_CONTROL).unwrap();
        dacl_sddl_of(file.as_raw_handle(), SE_FILE_OBJECT)
    }

    #[test]
    fn the_administrators_only_descriptor_builds_and_names_only_system_and_administrators() {
        use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

        let descriptor =
            crate::win_util::security_descriptor_from_sddl(ADMINISTRATORS_ONLY_SDDL).unwrap();
        let sddl = descriptor_sddl(descriptor.as_ptr(), DACL_SECURITY_INFORMATION);
        assert!(sddl.starts_with("D:P"), "{sddl}");
        assert_eq!(aces(&sddl), ["A;OICI;FA;;;SY", "A;OICI;FA;;;BA"], "{sddl}");
    }

    /// The real descriptor would lock this non-elevated test out of its own
    /// folder, so this one also lets the current user in, which is what
    /// makes the cleanup work. Nothing is created under the real
    /// ProgramData.
    #[test]
    fn a_new_folder_gets_exactly_the_protected_dacl_it_was_created_with() {
        let user = current_user_sid().unwrap();
        let sddl = format!("{ADMINISTRATORS_ONLY_SDDL}(A;OICI;FA;;;{user})");
        let parent = tempfile::tempdir().unwrap();

        let folder = create_unique_directory(parent.path(), "rabbit-uninstall", &sddl).unwrap();

        assert_eq!(folder.parent(), Some(parent.path()));
        let name = folder.file_name().unwrap().to_str().unwrap();
        let random = name.strip_prefix("rabbit-uninstall-").unwrap();
        assert_eq!(random.len(), 32, "{name}");
        assert!(random.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let read_back = dacl_sddl(&folder);
        // Protected: nothing from the temp folder's rights came along.
        assert!(read_back.starts_with("D:P"), "{read_back}");
        assert_eq!(
            aces(&read_back),
            [
                "A;OICI;FA;;;SY".to_string(),
                "A;OICI;FA;;;BA".to_string(),
                format!("A;OICI;FA;;;{user}"),
            ],
            "{read_back}"
        );
        // A file made inside inherits the same three entries.
        let inside = folder.join("Uninstall.exe");
        fs::write(&inside, b"copy").unwrap();
        let inherited = dacl_sddl(&inside);
        assert_eq!(aces(&inherited).len(), 3, "{inherited}");

        // Two calls never share a folder.
        let second = create_unique_directory(parent.path(), "rabbit-uninstall", &sddl).unwrap();
        assert_ne!(folder, second);
    }

    #[test]
    fn program_data_is_the_known_folder() {
        let program_data = program_data_dir().unwrap();
        assert!(program_data.is_absolute(), "{}", program_data.display());
        assert!(program_data.is_dir(), "{}", program_data.display());
    }

    /// The known folders are the ones Windows' own environment variables
    /// name in a normal session, read without asking the environment.
    #[test]
    fn program_files_and_common_files_are_the_known_folders() {
        let same = |known: &Path, variable: &str| {
            let Some(from_environment) = std::env::var_os(variable) else {
                return;
            };
            assert!(
                known.as_os_str().eq_ignore_ascii_case(&from_environment),
                "{variable}: {} vs {}",
                known.display(),
                Path::new(&from_environment).display()
            );
        };
        let program_files = machine_program_files_dirs().unwrap();
        assert!(!program_files.is_empty());
        for folder in &program_files {
            assert!(
                folder.is_absolute() && folder.is_dir(),
                "{}",
                folder.display()
            );
        }
        same(&program_files[0], "ProgramW6432");
        if let Some(x86) = program_files.get(1) {
            same(x86, "ProgramFiles(x86)");
        }
        let common = machine_common_program_files_dir().unwrap();
        assert!(
            common.is_absolute() && common.is_dir(),
            "{}",
            common.display()
        );
        same(&common, "CommonProgramW6432");
    }

    #[test]
    fn a_file_this_test_creates_is_owned_by_this_account_or_by_administrators() {
        let scratch = tempfile::tempdir().unwrap();
        let file = scratch.path().join("planted.exe");
        fs::write(&file, b"not a vendor file").unwrap();

        let owner = file_owner_sid(&file).unwrap();
        // An elevated test run may own new files through the
        // Administrators group; a normal one owns them itself.
        if crate::is_current_process_elevated().unwrap() && owner == "S-1-5-32-544" {
            assert!(file_owner_is_trusted(&file).unwrap());
        } else {
            assert_eq!(owner, current_user_sid().unwrap());
            assert!(!file_owner_is_trusted(&file).unwrap());
        }
        assert!(!is_reparse_point(&file).unwrap());
        assert!(!is_reparse_point(scratch.path()).unwrap());
    }

    /// Reads only, and only when REAPER is installed here.
    #[test]
    fn reapers_installed_uninstaller_is_owned_by_administrators() {
        let uninstaller = Path::new("C:\\Program Files\\REAPER (x64)\\Uninstall.exe");
        if !uninstaller.is_file() {
            return;
        }
        assert_eq!(file_owner_sid(uninstaller).unwrap(), "S-1-5-32-544");
        assert!(file_owner_is_trusted(uninstaller).unwrap());
    }

    #[test]
    fn a_junction_is_a_reparse_point_and_its_owner_is_its_own() {
        let scratch = tempfile::tempdir().unwrap();
        let target = scratch.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = scratch.path().join("link");
        crate::links::make_junction(&link, &target);

        assert!(is_reparse_point(&link).unwrap());
        assert!(!is_reparse_point(&target).unwrap());
        // Readable without following the link.
        file_owner_sid(&link).unwrap();
    }

    #[test]
    fn a_locked_file_cannot_be_changed_renamed_or_deleted_until_the_guard_drops() {
        let scratch = tempfile::tempdir().unwrap();
        let folder = scratch.path().join("vendor");
        fs::create_dir(&folder).unwrap();
        let file = folder.join("unins000.exe");
        fs::write(&file, b"vendor program").unwrap();

        let locked = LockedFile::open(&file).unwrap();
        assert_eq!(locked.path(), file);
        let mut contents = Vec::new();
        locked.file().read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"vendor program");
        // Others may still read it.
        assert_eq!(fs::read(&file).unwrap(), b"vendor program");

        assert!(OpenOptions::new().write(true).open(&file).is_err());
        assert!(fs::remove_file(&file).is_err());
        assert!(fs::rename(&file, folder.join("moved.exe")).is_err());
        assert!(fs::rename(&folder, scratch.path().join("moved")).is_err());
        assert_eq!(locked.owner_sid().unwrap(), file_owner_sid(&file).unwrap());

        drop(locked);

        OpenOptions::new().write(true).open(&file).unwrap();
        fs::rename(&folder, scratch.path().join("moved")).unwrap();
        fs::remove_file(scratch.path().join("moved").join("unins000.exe")).unwrap();
    }

    #[test]
    fn a_file_already_open_for_writing_cannot_be_locked() {
        let scratch = tempfile::tempdir().unwrap();
        let file = scratch.path().join("unins000.exe");
        fs::write(&file, b"vendor program").unwrap();

        let writer = OpenOptions::new().write(true).open(&file).unwrap();
        assert!(LockedFile::open(&file).is_err());
        drop(writer);
        LockedFile::open(&file).unwrap();
    }

    #[test]
    fn links_and_folders_cannot_be_locked() {
        let scratch = tempfile::tempdir().unwrap();
        let target = scratch.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = scratch.path().join("link");
        crate::links::make_junction(&link, &target);

        assert!(LockedFile::open(&link).is_err());
        assert!(LockedFile::open(&target).is_err());
    }

    #[test]
    fn a_locked_program_still_starts() {
        let scratch = tempfile::tempdir().unwrap();
        let copy = scratch.path().join("locked copy.exe");
        fs::copy(std::env::current_exe().unwrap(), &copy).unwrap();
        let locked = LockedFile::open(&copy).unwrap();

        // `--list` makes the test binary print its tests and exit.
        let status = std::process::Command::new(locked.path())
            .args(["--list", "--format=terse"])
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();

        assert!(status.success());
        drop(locked);
    }
}
