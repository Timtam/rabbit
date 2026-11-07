//! What this account may do with a file or folder on macOS, read without
//! administrator rights and without changing anything.
//!
//! A macOS uninstall decides before it starts how REAPER.app leaves. If
//! RABBIT can move the bundle itself, it goes to the Trash with no
//! password. If only root can, and nothing about the bundle looks wrong,
//! the run's one administrator command moves it into this user's Trash.
//! Otherwise the user moves it in Finder. These facts are what that
//! decision reads, together with the account's own Trash folder and
//! whether RABBIT itself runs as root.
//!
//! The facts are plain data, so `rabbit-core` builds them in its tests on
//! every platform. Only macOS reads real ones; elsewhere every function
//! here returns `Unsupported`.

use std::io;
use std::path::{Path, PathBuf};

/// What [`object_facts`] found about one path and the folder holding it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectFacts {
    /// The path itself is a directory, not a link to one.
    pub is_dir: bool,
    /// The path itself is a symbolic link.
    pub is_symlink: bool,
    /// The user ID that owns the path itself.
    pub owner_uid: u32,
    /// `owner_uid` is this account's.
    pub owned_by_me: bool,
    /// This account may write the object itself, ACLs and a read-only
    /// volume included. Moving a folder to another folder needs that,
    /// because its `..` entry changes. Never true for a link.
    pub writable: bool,
    /// This account may add names to, and search, the folder holding the
    /// path. It says nothing about taking a name out of that folder: macOS
    /// asks for adding rights alone here, so an append-only folder or an
    /// ACL that denies `delete_child` still passes. See `removable_by_me`.
    pub parent_writable_searchable: bool,
    /// This account may take the path's name out of the folder holding it,
    /// as a move needs, asked of macOS the way `rename` asks: ACLs on the
    /// object and the folder, the sticky rule, and the folder's immutable and
    /// append-only flags included. Never true for a link, because `access`
    /// would describe the link's target.
    pub removable_by_me: bool,
    /// That folder has the sticky bit, as `/tmp` does: then only the owner
    /// of the object or of the folder may move it out.
    pub parent_sticky: bool,
    /// That folder belongs to this account.
    pub parent_owned_by_me: bool,
    /// The object carries a flag that stops any rename until it is cleared,
    /// root's included: `uchg` (Finder's Locked), `schg`, `uappnd` or
    /// `sappnd`.
    pub locked: bool,
    /// The volume the object is on (`st_dev`). A move within one volume is
    /// a rename; a move to another is not.
    pub dev: u64,
    /// `CFBundleIdentifier` from `Contents/Info.plist`, for a directory that
    /// has one. `None` when there is none or it can't be read.
    pub bundle_identifier: Option<String>,
}

/// What [`user_trash_folder_facts`] found about this account's Trash
/// folder. Read with `lstat` alone: RABBIT never looks inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashFolderFacts {
    /// The folder, from this account's entry in the user database.
    pub path: PathBuf,
    /// It is a directory, not a link to one.
    pub is_dir: bool,
    /// It is a symbolic link.
    pub is_symlink: bool,
    /// The user ID that owns it.
    pub owner_uid: u32,
    /// `owner_uid` is this account's.
    pub owned_by_me: bool,
    /// The volume it is on (`st_dev`).
    pub dev: u64,
}

/// What this account may do with `path`, an absolute path, and the folder
/// that holds it. The path itself is read with `lstat`, so a link is
/// described as a link and never followed; the folder holding it is the
/// real folder, links in its own path followed, because its rights decide
/// whether the name can go.
///
/// An error when the path or its folder can't be read, or when the path
/// isn't absolute or has no parent. macOS only; elsewhere `Unsupported`.
pub fn object_facts(path: &Path) -> io::Result<ObjectFacts> {
    platform_object_facts(path)
}

/// This account's Trash folder: `.Trash` in the home folder that the user
/// database names for this process's real user ID.
///
/// Never `$HOME`: an environment variable can say anything, and the folder
/// root may move a bundle into has to be the account's own. macOS only;
/// elsewhere `Unsupported`.
pub fn user_trash_folder() -> io::Result<PathBuf> {
    platform_user_trash_folder()
}

/// What [`user_trash_folder`] is: whether it is a real folder, not a link,
/// who owns it and which volume it is on. `NotFound` when it doesn't exist.
/// macOS only; elsewhere `Unsupported`.
pub fn user_trash_folder_facts() -> io::Result<TrashFolderFacts> {
    platform_user_trash_folder_facts()
}

/// Whether RABBIT itself runs as root, as it does under `sudo`. An
/// uninstall refuses to run then: its checks, its Trash and its backups
/// would all be root's rather than the user's. macOS only; elsewhere
/// `Unsupported`.
pub fn effective_uid_is_root() -> io::Result<bool> {
    platform_effective_uid_is_root()
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{ObjectFacts, TrashFolderFacts};
    use std::ffi::{CStr, CString, OsStr};
    use std::io;
    use std::os::macos::fs::MetadataExt as _;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    /// The flags that stop a rename: Finder's Locked (`uchg`), the system
    /// immutable flag, and both append-only flags.
    const LOCKING_FLAGS: u32 =
        libc::UF_IMMUTABLE | libc::SF_IMMUTABLE | libc::UF_APPEND | libc::SF_APPEND;

    /// `_DELETE_OK` from <sys/unistd.h>, which the libc crate doesn't export
    /// for Apple targets. `access` then asks what `rename` asks: may this
    /// account remove the name from its folder. That brings in the object's
    /// `delete` and the folder's `delete_child` ACL entries, the sticky rule,
    /// and the folder's immutable and append-only flags. It must be asked on
    /// its own: with any of these extended bits set, XNU ignores W_OK and X_OK.
    const DELETE_OK: libc::c_int = 1 << 12;

    /// The largest buffer tried for this account's user database entry.
    /// Real entries need a few hundred bytes.
    const MAX_PASSWD_BUFFER: usize = 1024 * 1024;

    /// This process's real user ID, the one `access` checks against.
    fn real_uid() -> libc::uid_t {
        // SAFETY: getuid takes nothing and always succeeds.
        unsafe { libc::getuid() }
    }

    pub(super) fn effective_uid_is_root() -> bool {
        // SAFETY: geteuid takes nothing and always succeeds.
        unsafe { libc::geteuid() == 0 }
    }

    /// Whether this account may access `path` in `mode`, by `access(2)`,
    /// which follows links and honours ACLs, file flags and read-only
    /// volumes for the rights `mode` asks for. `W_OK` on a folder asks only
    /// to add names to it, never to remove them; that takes [`DELETE_OK`].
    fn may_access(path: &Path, mode: libc::c_int) -> bool {
        let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // SAFETY: `path` is NUL-terminated and lives for the whole call,
        // which only reads it.
        unsafe { libc::access(path.as_ptr(), mode) == 0 }
    }

    fn invalid(path: &Path, why: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{}: {why}", path.display()),
        )
    }

    pub(super) fn object_facts(path: &Path) -> io::Result<ObjectFacts> {
        if !path.is_absolute() {
            return Err(invalid(path, "the path isn't absolute"));
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| invalid(path, "the path has no parent folder"))?;
        let metadata = std::fs::symlink_metadata(path)?;
        let parent_metadata = std::fs::metadata(parent)?;
        let me = real_uid();
        let file_type = metadata.file_type();
        let is_symlink = file_type.is_symlink();
        let is_dir = file_type.is_dir();
        Ok(ObjectFacts {
            is_dir,
            is_symlink,
            owner_uid: metadata.uid(),
            owned_by_me: metadata.uid() == me,
            // `access` would describe the link's target, not the link.
            writable: !is_symlink && may_access(path, libc::W_OK),
            parent_writable_searchable: may_access(parent, libc::W_OK | libc::X_OK),
            // Asked of the path itself: macOS finds its folder and checks
            // both, as it does for `rename`.
            removable_by_me: !is_symlink && may_access(path, DELETE_OK),
            parent_sticky: parent_metadata.mode() & u32::from(libc::S_ISVTX) != 0,
            parent_owned_by_me: parent_metadata.uid() == me,
            locked: metadata.st_flags() & LOCKING_FLAGS != 0,
            dev: metadata.dev(),
            bundle_identifier: if is_dir {
                bundle_identifier(path)
            } else {
                None
            },
        })
    }

    /// `CFBundleIdentifier` from the bundle's `Contents/Info.plist`, read
    /// the way file_version.rs reads its version.
    fn bundle_identifier(bundle: &Path) -> Option<String> {
        let value = plist::Value::from_file(bundle.join("Contents").join("Info.plist")).ok()?;
        let identifier = value
            .as_dictionary()?
            .get("CFBundleIdentifier")?
            .as_string()?
            .trim();
        (!identifier.is_empty()).then(|| identifier.to_string())
    }

    /// The home folder the user database names for `uid`.
    fn home_folder_of(uid: libc::uid_t) -> io::Result<PathBuf> {
        // SAFETY: sysconf only reads a configuration value.
        let suggested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
        let mut size = usize::try_from(suggested)
            .ok()
            .filter(|size| *size > 0)
            .unwrap_or(4096);
        loop {
            let mut buffer = vec![0 as libc::c_char; size];
            // SAFETY: passwd is plain data (integers and pointers), for which
            // all zeroes is a valid value.
            let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
            let mut found: *mut libc::passwd = std::ptr::null_mut();
            // SAFETY: every pointer is to a live local of the right type, and
            // the length passed is the buffer's real length, so the strings
            // getpwuid_r writes stay inside it.
            let error = unsafe {
                libc::getpwuid_r(
                    uid,
                    &mut entry,
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    &mut found,
                )
            };
            if error == libc::ERANGE && size < MAX_PASSWD_BUFFER {
                size *= 2;
                continue;
            }
            if error != 0 {
                return Err(io::Error::from_raw_os_error(error));
            }
            if found.is_null() || entry.pw_dir.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("the user database has no home folder for user ID {uid}"),
                ));
            }
            // SAFETY: getpwuid_r succeeded, so pw_dir points to a
            // NUL-terminated string inside `buffer`, which is still alive.
            let home = unsafe { CStr::from_ptr(entry.pw_dir) };
            let home = PathBuf::from(OsStr::from_bytes(home.to_bytes()));
            if !home.is_absolute() {
                return Err(io::Error::other(format!(
                    "the user database names {} as the home folder of user ID {uid}, which isn't an absolute path",
                    home.display()
                )));
            }
            return Ok(home);
        }
    }

    pub(super) fn user_trash_folder() -> io::Result<PathBuf> {
        Ok(home_folder_of(real_uid())?.join(".Trash"))
    }

    pub(super) fn user_trash_folder_facts() -> io::Result<TrashFolderFacts> {
        let path = user_trash_folder()?;
        let metadata = std::fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        Ok(TrashFolderFacts {
            is_dir: file_type.is_dir(),
            is_symlink: file_type.is_symlink(),
            owner_uid: metadata.uid(),
            owned_by_me: metadata.uid() == real_uid(),
            dev: metadata.dev(),
            path,
        })
    }
}

#[cfg(target_os = "macos")]
fn platform_object_facts(path: &Path) -> io::Result<ObjectFacts> {
    macos::object_facts(path)
}

#[cfg(target_os = "macos")]
fn platform_user_trash_folder() -> io::Result<PathBuf> {
    macos::user_trash_folder()
}

#[cfg(target_os = "macos")]
fn platform_user_trash_folder_facts() -> io::Result<TrashFolderFacts> {
    macos::user_trash_folder_facts()
}

#[cfg(target_os = "macos")]
fn platform_effective_uid_is_root() -> io::Result<bool> {
    Ok(macos::effective_uid_is_root())
}

#[cfg(not(target_os = "macos"))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "only macOS reads these facts this way",
    )
}

#[cfg(not(target_os = "macos"))]
fn platform_object_facts(_path: &Path) -> io::Result<ObjectFacts> {
    Err(unsupported())
}

#[cfg(not(target_os = "macos"))]
fn platform_user_trash_folder() -> io::Result<PathBuf> {
    Err(unsupported())
}

#[cfg(not(target_os = "macos"))]
fn platform_user_trash_folder_facts() -> io::Result<TrashFolderFacts> {
    Err(unsupported())
}

#[cfg(not(target_os = "macos"))]
fn platform_effective_uid_is_root() -> io::Result<bool> {
    Err(unsupported())
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn only_macos_reads_facts() {
        let dir = tempfile::tempdir().unwrap();
        for error in [
            super::object_facts(dir.path()).unwrap_err(),
            super::user_trash_folder().unwrap_err(),
            super::user_trash_folder_facts().unwrap_err(),
            super::effective_uid_is_root().unwrap_err(),
        ] {
            assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        }
    }

    /// Every test works on its own temp folder and changes only what it
    /// created there; the rights and flags it sets are undone before the
    /// folder is deleted.
    #[cfg(target_os = "macos")]
    mod macos {
        use super::super::{
            ObjectFacts, effective_uid_is_root, object_facts, user_trash_folder,
            user_trash_folder_facts,
        };
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        use std::path::{Path, PathBuf};
        use std::process::Command;

        const BUNDLE_ID: &str = "org.example.rabbit-facts-test";

        fn real_temp_dir() -> (tempfile::TempDir, PathBuf) {
            let dir = tempfile::tempdir().unwrap();
            let real = dir.path().canonicalize().unwrap();
            (dir, real)
        }

        fn my_uid() -> u32 {
            // SAFETY: getuid takes nothing and always succeeds.
            unsafe { libc::getuid() }
        }

        fn info_plist(identifier_entry: &str) -> String {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>Facts Test</string>
	{identifier_entry}
</dict>
</plist>
"#
            )
        }

        fn make_bundle(parent: &Path, name: &str, plist: Option<&str>) -> PathBuf {
            let bundle = parent.join(name);
            let contents = bundle.join("Contents");
            std::fs::create_dir_all(contents.join("MacOS")).unwrap();
            if let Some(plist) = plist {
                std::fs::write(contents.join("Info.plist"), plist).unwrap();
            }
            bundle
        }

        fn set_mode(path: &Path, mode: u32) {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        }

        /// Puts a folder's rights back when the test ends, however it ends,
        /// so the temp folder can be deleted.
        struct RestoreMode<'a>(&'a Path, u32);

        impl Drop for RestoreMode<'_> {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(self.0, std::fs::Permissions::from_mode(self.1));
            }
        }

        fn chflags(flag: &str, path: &Path) {
            let status = Command::new("/usr/bin/chflags")
                .arg(flag)
                .arg(path)
                .status()
                .unwrap();
            assert!(status.success(), "chflags {flag} {}", path.display());
        }

        /// Clears Finder's Locked flag when the test ends, however it ends.
        struct Unlock<'a>(&'a Path);

        impl Drop for Unlock<'_> {
            fn drop(&mut self) {
                let _ = Command::new("/usr/bin/chflags")
                    .arg("nouchg")
                    .arg(self.0)
                    .status();
            }
        }

        #[test]
        fn a_bundle_this_account_owns_can_be_moved_by_it() {
            let (_dir, root) = real_temp_dir();
            let entry = format!("<key>CFBundleIdentifier</key>\n\t<string>{BUNDLE_ID}</string>");
            let bundle = make_bundle(&root, "Facts Test.app", Some(&info_plist(&entry)));

            let facts = object_facts(&bundle).unwrap();
            assert_eq!(
                facts,
                ObjectFacts {
                    is_dir: true,
                    is_symlink: false,
                    owner_uid: my_uid(),
                    owned_by_me: true,
                    writable: true,
                    parent_writable_searchable: true,
                    removable_by_me: true,
                    parent_sticky: false,
                    parent_owned_by_me: true,
                    locked: false,
                    dev: std::fs::symlink_metadata(&bundle).unwrap().dev(),
                    bundle_identifier: Some(BUNDLE_ID.to_string()),
                }
            );
        }

        #[test]
        fn a_bundle_without_a_readable_identifier_has_none() {
            let (_dir, root) = real_temp_dir();
            let no_plist = make_bundle(&root, "No Plist.app", None);
            let no_key = make_bundle(&root, "No Key.app", Some(&info_plist("")));
            let garbage = make_bundle(&root, "Garbage.app", Some("not a property list"));
            for bundle in [no_plist, no_key, garbage] {
                assert_eq!(
                    object_facts(&bundle).unwrap().bundle_identifier,
                    None,
                    "{}",
                    bundle.display()
                );
            }
        }

        /// Root may write anything, so the rights tests have nothing to
        /// show when the tests run as root.
        fn running_as_root() -> bool {
            effective_uid_is_root().unwrap()
        }

        #[test]
        fn a_folder_this_account_cannot_write_keeps_the_bundle_in_it() {
            if running_as_root() {
                return;
            }
            let (_dir, root) = real_temp_dir();
            let parent = root.join("read only");
            std::fs::create_dir(&parent).unwrap();
            let bundle = make_bundle(&parent, "Facts Test.app", None);
            set_mode(&parent, 0o555);
            let _restore = RestoreMode(&parent, 0o755);

            let facts = object_facts(&bundle).unwrap();
            assert!(!facts.parent_writable_searchable);
            assert!(!facts.removable_by_me);
            assert!(facts.writable);
            assert!(facts.parent_owned_by_me);
        }

        /// Clears an append-only flag when the test ends, however it ends.
        struct NoAppend<'a>(&'a Path);

        impl Drop for NoAppend<'_> {
            fn drop(&mut self) {
                let _ = Command::new("/usr/bin/chflags")
                    .arg("nouappnd")
                    .arg(self.0)
                    .status();
            }
        }

        /// macOS counts adding a name to an append-only folder as appending,
        /// so the folder still looks writable; taking a name out is refused,
        /// root included.
        #[test]
        fn an_append_only_folder_keeps_the_bundle_in_it() {
            let (_dir, root) = real_temp_dir();
            let parent = root.join("append only");
            std::fs::create_dir(&parent).unwrap();
            let bundle = make_bundle(&parent, "Facts Test.app", None);
            chflags("uappnd", &parent);
            let _restore = NoAppend(&parent);

            let facts = object_facts(&bundle).unwrap();
            assert!(facts.parent_writable_searchable);
            assert!(!facts.removable_by_me);
            assert!(!facts.locked, "the flag is the folder's, not the bundle's");
        }

        /// Takes every ACL entry off a folder when the test ends, however it
        /// ends.
        struct NoAcl<'a>(&'a Path);

        impl Drop for NoAcl<'_> {
            fn drop(&mut self) {
                let _ = Command::new("/bin/chmod").arg("-N").arg(self.0).status();
            }
        }

        #[test]
        fn a_folder_whose_acl_denies_removing_names_keeps_the_bundle_in_it() {
            if running_as_root() {
                return;
            }
            let (_dir, root) = real_temp_dir();
            let parent = root.join("no removing");
            std::fs::create_dir(&parent).unwrap();
            let bundle = make_bundle(&parent, "Facts Test.app", None);
            let me = Command::new("/usr/bin/id").arg("-un").output().unwrap();
            assert!(me.status.success(), "id -un");
            let me = String::from_utf8(me.stdout).unwrap();
            let status = Command::new("/bin/chmod")
                .arg("+a")
                .arg(format!("user:{} deny delete_child", me.trim()))
                .arg(&parent)
                .status()
                .unwrap();
            assert!(status.success(), "chmod +a {}", parent.display());
            let _restore = NoAcl(&parent);

            let facts = object_facts(&bundle).unwrap();
            assert!(facts.parent_writable_searchable);
            assert!(!facts.removable_by_me);
        }

        #[test]
        fn a_bundle_this_account_cannot_write_is_not_writable() {
            if running_as_root() {
                return;
            }
            let (_dir, root) = real_temp_dir();
            let bundle = make_bundle(&root, "Facts Test.app", None);
            set_mode(&bundle, 0o555);
            let _restore = RestoreMode(&bundle, 0o755);

            let facts = object_facts(&bundle).unwrap();
            assert!(!facts.writable);
            assert!(facts.parent_writable_searchable);
        }

        #[test]
        fn a_sticky_folder_is_reported() {
            let (_dir, root) = real_temp_dir();
            let parent = root.join("sticky");
            std::fs::create_dir(&parent).unwrap();
            let bundle = make_bundle(&parent, "Facts Test.app", None);
            set_mode(&parent, 0o1777);
            let _restore = RestoreMode(&parent, 0o755);

            assert!(object_facts(&bundle).unwrap().parent_sticky);
        }

        #[test]
        fn a_locked_bundle_is_reported() {
            let (_dir, root) = real_temp_dir();
            let bundle = make_bundle(&root, "Facts Test.app", None);
            chflags("uchg", &bundle);
            let _unlock = Unlock(&bundle);

            assert!(object_facts(&bundle).unwrap().locked);
        }

        #[test]
        fn a_link_is_described_as_a_link_and_never_followed() {
            let (_dir, root) = real_temp_dir();
            let entry = format!("<key>CFBundleIdentifier</key>\n\t<string>{BUNDLE_ID}</string>");
            let bundle = make_bundle(&root, "Real.app", Some(&info_plist(&entry)));
            let link = root.join("Link.app");
            std::os::unix::fs::symlink(&bundle, &link).unwrap();

            let facts = object_facts(&link).unwrap();
            assert!(facts.is_symlink);
            assert!(!facts.is_dir);
            assert!(!facts.writable);
            assert!(!facts.removable_by_me);
            assert_eq!(facts.bundle_identifier, None);
        }

        #[test]
        fn a_path_that_is_relative_rootless_or_missing_gives_an_error() {
            let (_dir, root) = real_temp_dir();
            for (path, kind) in [
                (
                    PathBuf::from("Facts Test.app"),
                    std::io::ErrorKind::InvalidInput,
                ),
                (PathBuf::from("/"), std::io::ErrorKind::InvalidInput),
                (root.join("missing.app"), std::io::ErrorKind::NotFound),
            ] {
                assert_eq!(
                    object_facts(&path).unwrap_err().kind(),
                    kind,
                    "{}",
                    path.display()
                );
            }
        }

        #[test]
        fn the_trash_folder_comes_from_the_user_database() {
            let trash = user_trash_folder().unwrap();
            assert!(trash.is_absolute(), "{}", trash.display());
            assert_eq!(trash.file_name().unwrap(), ".Trash");
        }

        /// The runner's Trash may not exist yet; when it does, it must be
        /// described from `lstat` alone.
        #[test]
        fn the_trash_folder_facts_describe_that_folder() {
            match user_trash_folder_facts() {
                Ok(facts) => {
                    assert_eq!(facts.path, user_trash_folder().unwrap());
                    assert_eq!(facts.owned_by_me, facts.owner_uid == my_uid());
                }
                Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{error}"),
            }
        }

        #[test]
        fn the_effective_user_can_be_read() {
            // SAFETY: geteuid takes nothing and always succeeds.
            let root = unsafe { libc::geteuid() } == 0;
            assert_eq!(effective_uid_is_root().unwrap(), root);
        }
    }
}
