//! Moving a file or folder to the Trash on macOS, as Finder does.
//!
//! An uninstall moves REAPER.app to the Trash rather than deleting it, so
//! the user can drag it back until they empty the Trash. RABBIT asks
//! Foundation's `NSFileManager` to do it, as the user, with no password.
//! macOS picks the Trash for the item's volume, creates it if needed, and
//! renames the item if the Trash already holds that name, which is why the
//! answer is the item's new path.
//!
//! The call goes through the Objective-C runtime directly, in the style of
//! voiceover.rs and arch.rs, with no new crates. CoreFoundation's `CFURL`
//! and `CFError` are the same objects as Foundation's `NSURL` and `NSError`
//! (toll-free bridged), so only two Objective-C messages are sent, each
//! through `objc_msgSend` cast to that message's exact signature, which
//! Apple Silicon requires.
//!
//! Only macOS has this; elsewhere [`move_to_trash`] returns `Unsupported`.

use std::path::{Path, PathBuf};

/// Why [`move_to_trash`] didn't move something, or couldn't say where it
/// went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashErrorKind {
    /// macOS refused for lack of permission: the folder or the item can't
    /// be changed by this account, or App Management or other privacy
    /// protection stopped it. Nothing moved.
    PermissionDenied,
    /// Nothing was at the path. Nothing moved.
    NotFound,
    /// The volume is read-only. Nothing moved.
    ReadOnly,
    /// macOS reported the move as done but gave no new path RABBIT could
    /// read. The item may well be in the Trash; only the original path can
    /// tell whether it is still there.
    Unclear,
    /// RABBIT refused before asking macOS: a test build, or a program not
    /// named like RABBIT. Nothing moved.
    Refused,
    /// Not macOS. Nothing moved.
    Unsupported,
    /// Any other failure; the message has macOS's own words. Nothing moved.
    Other,
}

/// A [`move_to_trash`] that didn't succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashError {
    pub kind: TrashErrorKind,
    /// What went wrong, in macOS's words where it gave any, with its error
    /// domain and code.
    pub message: String,
}

impl TrashError {
    fn new(kind: TrashErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TrashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TrashError {}

/// Move `path`, an absolute path to a file or folder, to the Trash as this
/// user, and return where it is now.
///
/// It refuses, before asking macOS, in a test build and when the running
/// program isn't a RABBIT build by [`crate::running_as_rabbit`]: no test,
/// here or in a crate that calls this, can move anything of the user's.
/// macOS only; elsewhere `Unsupported`.
pub fn move_to_trash(path: &Path) -> Result<PathBuf, TrashError> {
    if cfg!(test) {
        return Err(TrashError::new(
            TrashErrorKind::Refused,
            "a test build never moves anything to the Trash",
        ));
    }
    if !crate::running_as_rabbit() {
        return Err(TrashError::new(
            TrashErrorKind::Refused,
            "only RABBIT itself moves things to the Trash, and this program isn't named like a RABBIT build",
        ));
    }
    platform_move_to_trash(path)
}

#[cfg(target_os = "macos")]
fn platform_move_to_trash(path: &Path) -> Result<PathBuf, TrashError> {
    macos::move_to_trash(path)
}

#[cfg(not(target_os = "macos"))]
fn platform_move_to_trash(_path: &Path) -> Result<PathBuf, TrashError> {
    Err(TrashError::new(
        TrashErrorKind::Unsupported,
        "only macOS has a Trash RABBIT can move things to",
    ))
}

/// Which family an error number belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum ErrorDomain {
    /// `NSCocoaErrorDomain`: Foundation's own file errors.
    Cocoa,
    /// `NSPOSIXErrorDomain`: an `errno` value.
    Posix,
    Other,
}

/// Foundation's file error codes (FoundationErrors.h) that say more than
/// "it failed".
const NS_FILE_NO_SUCH_FILE_ERROR: isize = 4;
const NS_FILE_READ_NO_PERMISSION_ERROR: isize = 257;
const NS_FILE_READ_NO_SUCH_FILE_ERROR: isize = 260;
const NS_FILE_WRITE_NO_PERMISSION_ERROR: isize = 513;
const NS_FILE_WRITE_VOLUME_READ_ONLY_ERROR: isize = 642;

/// The `errno` values that do, as macOS numbers them. Spelled out rather
/// than taken from libc so the mapping is tested on every platform.
const EPERM: isize = 1;
const ENOENT: isize = 2;
const EACCES: isize = 13;
const EROFS: isize = 30;

/// The kind of a failed move: by the error itself, or, when that is
/// Foundation's general "couldn't", by the error underneath it, which is
/// often the `errno` that really stopped it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn trash_error_kind(
    domain: ErrorDomain,
    code: isize,
    underlying: Option<(ErrorDomain, isize)>,
) -> TrashErrorKind {
    match error_kind(domain, code) {
        TrashErrorKind::Other => underlying
            .map(|(domain, code)| error_kind(domain, code))
            .unwrap_or(TrashErrorKind::Other),
        kind => kind,
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn error_kind(domain: ErrorDomain, code: isize) -> TrashErrorKind {
    match (domain, code) {
        (ErrorDomain::Cocoa, NS_FILE_NO_SUCH_FILE_ERROR | NS_FILE_READ_NO_SUCH_FILE_ERROR)
        | (ErrorDomain::Posix, ENOENT) => TrashErrorKind::NotFound,
        (
            ErrorDomain::Cocoa,
            NS_FILE_READ_NO_PERMISSION_ERROR | NS_FILE_WRITE_NO_PERMISSION_ERROR,
        )
        | (ErrorDomain::Posix, EPERM | EACCES) => TrashErrorKind::PermissionDenied,
        (ErrorDomain::Cocoa, NS_FILE_WRITE_VOLUME_READ_ONLY_ERROR)
        | (ErrorDomain::Posix, EROFS) => TrashErrorKind::ReadOnly,
        _ => TrashErrorKind::Other,
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{ErrorDomain, TrashError, TrashErrorKind, trash_error_kind};
    use std::ffi::{OsString, c_char, c_void};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use std::ptr;

    /// An Objective-C object, `id`.
    type Id = *mut c_void;
    /// An Objective-C method name, `SEL`.
    type Sel = *const c_void;
    /// Any CoreFoundation object.
    type CfRef = *const c_void;
    type CfIndex = isize;
    /// CoreFoundation's `Boolean`, an unsigned char on every Mac.
    type CfBoolean = u8;
    /// CoreFoundation's `CFTypeID`, an unsigned long.
    type CfTypeId = usize;

    /// Objective-C's `BOOL`: a C `bool` on Apple Silicon but a `signed
    /// char` on Intel Macs, so a reply is read in the width it was sent in.
    #[cfg(target_arch = "aarch64")]
    type ObjcBool = bool;
    #[cfg(not(target_arch = "aarch64"))]
    type ObjcBool = i8;

    #[cfg(target_arch = "aarch64")]
    fn is_yes(value: ObjcBool) -> bool {
        value
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn is_yes(value: ObjcBool) -> bool {
        value != 0
    }

    const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    /// Room for the item's new path. macOS paths stop at 1024 bytes
    /// (MAXPATHLEN); a Trash name only adds a little to the old one.
    const PATH_BUFFER_SIZE: usize = 4 * 1024;

    // SAFETY: these match <objc/runtime.h> and <objc/message.h>. The pool
    // functions are the ones clang calls for `@autoreleasepool`.
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        /// Declared without a signature on purpose: every call casts it to
        /// the exact signature of the message it sends.
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);
    }

    // SAFETY: these match Foundation's NSError.h. Naming them also links
    // Foundation, and with it NSFileManager, into the command-line build,
    // which has no AppKit to bring it in.
    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {
        static NSCocoaErrorDomain: CfRef;
        static NSPOSIXErrorDomain: CfRef;
    }

    // SAFETY: these match CoreFoundation's public C headers.
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFErrorUnderlyingErrorKey: CfRef;
        fn CFURLCreateFromFileSystemRepresentation(
            allocator: CfRef,
            buffer: *const u8,
            length: CfIndex,
            is_directory: CfBoolean,
        ) -> CfRef;
        fn CFURLGetFileSystemRepresentation(
            url: CfRef,
            resolve_against_base: CfBoolean,
            buffer: *mut u8,
            max_length: CfIndex,
        ) -> CfBoolean;
        fn CFErrorGetDomain(error: CfRef) -> CfRef;
        fn CFErrorGetCode(error: CfRef) -> CfIndex;
        fn CFErrorCopyDescription(error: CfRef) -> CfRef;
        fn CFErrorCopyUserInfo(error: CfRef) -> CfRef;
        fn CFErrorGetTypeID() -> CfTypeId;
        fn CFGetTypeID(object: CfRef) -> CfTypeId;
        fn CFDictionaryGetValue(dictionary: CfRef, key: CfRef) -> CfRef;
        fn CFEqual(left: CfRef, right: CfRef) -> CfBoolean;
        fn CFStringGetLength(string: CfRef) -> CfIndex;
        fn CFStringGetMaximumSizeForEncoding(length: CfIndex, encoding: u32) -> CfIndex;
        fn CFStringGetCString(
            string: CfRef,
            buffer: *mut c_char,
            size: CfIndex,
            encoding: u32,
        ) -> CfBoolean;
        fn CFRelease(object: CfRef);
    }

    /// `+[NSFileManager defaultManager]`.
    type DefaultManagerMessage = unsafe extern "C" fn(Id, Sel) -> Id;
    /// `-[NSFileManager trashItemAtURL:resultingItemURL:error:]`.
    type TrashItemMessage = unsafe extern "C" fn(Id, Sel, Id, *mut Id, *mut Id) -> ObjcBool;

    /// A CoreFoundation object this code created or copied, released when
    /// it goes out of scope.
    struct Owned(CfRef);

    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the object came from a Create or Copy call, so
                // this code holds one reference, released once here.
                unsafe { CFRelease(self.0) };
            }
        }
    }

    /// An autorelease pool for the length of one move. Foundation hands
    /// back the new URL and any error autoreleased; RABBIT's threads have
    /// no pool of their own, so without this they would leak.
    struct AutoreleasePool(*mut c_void);

    impl AutoreleasePool {
        fn push() -> Self {
            // SAFETY: pushing a pool has no preconditions.
            Self(unsafe { objc_autoreleasePoolPush() })
        }
    }

    impl Drop for AutoreleasePool {
        fn drop(&mut self) {
            // SAFETY: the pool was pushed on this thread by `push`, and
            // pools pushed inside it have all been popped.
            unsafe { objc_autoreleasePoolPop(self.0) };
        }
    }

    fn io_error_kind(error: &std::io::Error) -> TrashErrorKind {
        match error.kind() {
            std::io::ErrorKind::NotFound => TrashErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied => TrashErrorKind::PermissionDenied,
            std::io::ErrorKind::ReadOnlyFilesystem => TrashErrorKind::ReadOnly,
            _ => TrashErrorKind::Other,
        }
    }

    pub(super) fn move_to_trash(path: &Path) -> Result<PathBuf, TrashError> {
        if !path.is_absolute() {
            return Err(TrashError::new(
                TrashErrorKind::Other,
                format!("{} isn't an absolute path", path.display()),
            ));
        }
        let bytes = path.as_os_str().as_bytes();
        if bytes.contains(&0) {
            return Err(TrashError::new(
                TrashErrorKind::Other,
                format!("{} contains a NUL character", path.display()),
            ));
        }
        // Tells CoreFoundation whether to make a folder URL, and stops a
        // missing path before any Objective-C runs.
        let is_directory = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata.is_dir(),
            Err(error) => {
                return Err(TrashError::new(
                    io_error_kind(&error),
                    format!("{}: {error}", path.display()),
                ));
            }
        };
        let _pool = AutoreleasePool::push();
        // SAFETY: the pool lives until the end of this function, and the
        // result holds only Rust values.
        unsafe { trash_in_pool(bytes, is_directory) }
    }

    /// # Safety
    ///
    /// Must run inside an autorelease pool that outlives the call: the new
    /// URL and the error come back autoreleased.
    unsafe fn trash_in_pool(path: &[u8], is_directory: bool) -> Result<PathBuf, TrashError> {
        let other = |message: &str| TrashError::new(TrashErrorKind::Other, message);
        // SAFETY: `path` is a live buffer of exactly this length.
        let url = Owned(unsafe {
            CFURLCreateFromFileSystemRepresentation(
                ptr::null(),
                path.as_ptr(),
                path.len() as CfIndex,
                CfBoolean::from(is_directory),
            )
        });
        if url.0.is_null() {
            return Err(other("macOS couldn't make a file URL of the path"));
        }
        // SAFETY: the names are NUL-terminated, and the runtime keeps
        // classes and selectors for the life of the process.
        let (file_manager_class, default_manager, trash_item) = unsafe {
            (
                objc_getClass(c"NSFileManager".as_ptr()),
                sel_registerName(c"defaultManager".as_ptr()),
                sel_registerName(c"trashItemAtURL:resultingItemURL:error:".as_ptr()),
            )
        };
        if file_manager_class.is_null() {
            return Err(other("Foundation's NSFileManager isn't available"));
        }
        // SAFETY: objc_msgSend must be called with the exact signature of
        // the method it reaches, and these are those two methods' own:
        // `+ (NSFileManager *)defaultManager` and `- (BOOL)trashItemAtURL:
        // (NSURL *) resultingItemURL:(NSURL **) error:(NSError **)`.
        let (send_default_manager, send_trash_item) = unsafe {
            (
                std::mem::transmute::<unsafe extern "C" fn(), DefaultManagerMessage>(objc_msgSend),
                std::mem::transmute::<unsafe extern "C" fn(), TrashItemMessage>(objc_msgSend),
            )
        };
        // SAFETY: the receiver is NSFileManager's class, which answers
        // `defaultManager` with the shared manager, safe on any thread.
        let manager = unsafe { send_default_manager(file_manager_class, default_manager) };
        if manager.is_null() {
            return Err(other("NSFileManager has no default manager"));
        }
        let mut resulting_url: Id = ptr::null_mut();
        let mut error: Id = ptr::null_mut();
        // SAFETY: `manager` is an NSFileManager; a CFURL is an NSURL; both
        // out-parameters point to live locals. What comes back is
        // autoreleased and read before the caller's pool ends.
        let moved = unsafe {
            send_trash_item(
                manager,
                trash_item,
                url.0.cast_mut(),
                &mut resulting_url,
                &mut error,
            )
        };
        if !is_yes(moved) {
            // SAFETY: an NSError is a CFError, alive until the pool ends.
            return Err(unsafe { trash_error(error.cast_const()) });
        }
        if resulting_url.is_null() {
            return Err(TrashError::new(
                TrashErrorKind::Unclear,
                "macOS reported the move as done but didn't say where the item went",
            ));
        }
        // SAFETY: the reply is an NSURL, and so a CFURL, alive until the
        // pool ends.
        unsafe { file_system_path(resulting_url.cast_const()) }.ok_or_else(|| {
            TrashError::new(
                TrashErrorKind::Unclear,
                "macOS reported the move as done, but its new path couldn't be read",
            )
        })
    }

    /// # Safety
    ///
    /// `url` is a live CFURL or NSURL.
    unsafe fn file_system_path(url: CfRef) -> Option<PathBuf> {
        let mut buffer = vec![0_u8; PATH_BUFFER_SIZE];
        // SAFETY: the length passed is the buffer's real length, so the
        // path written, NUL included, stays inside it.
        let written = unsafe {
            CFURLGetFileSystemRepresentation(url, 1, buffer.as_mut_ptr(), buffer.len() as CfIndex)
        };
        if written == 0 {
            return None;
        }
        let end = buffer.iter().position(|byte| *byte == 0)?;
        buffer.truncate(end);
        let path = PathBuf::from(OsString::from_vec(buffer));
        path.is_absolute().then_some(path)
    }

    /// # Safety
    ///
    /// `error` is null or a live CFError or NSError.
    unsafe fn trash_error(error: CfRef) -> TrashError {
        if error.is_null() {
            return TrashError::new(
                TrashErrorKind::Other,
                "macOS didn't move it and gave no reason",
            );
        }
        // SAFETY: `error` is a live CFError, and the description is a copy
        // this code releases.
        let (domain, code, underlying, description, domain_name) = unsafe {
            let (domain, code) = domain_and_code(error);
            let description = Owned(CFErrorCopyDescription(error));
            (
                domain,
                code,
                underlying_error(error),
                cf_string(description.0),
                cf_string(CFErrorGetDomain(error)),
            )
        };
        TrashError::new(
            trash_error_kind(domain, code, underlying),
            format!(
                "{} ({} {code})",
                description
                    .as_deref()
                    .unwrap_or("macOS gave no description"),
                domain_name.as_deref().unwrap_or("unknown domain"),
            ),
        )
    }

    /// # Safety
    ///
    /// `error` is a live CFError or NSError.
    unsafe fn domain_and_code(error: CfRef) -> (ErrorDomain, isize) {
        // SAFETY: an error always has a domain, and the two domain
        // constants are Foundation's own, alive for the whole process.
        unsafe {
            let domain = CFErrorGetDomain(error);
            let domain = if domain.is_null() {
                ErrorDomain::Other
            } else if CFEqual(domain, NSCocoaErrorDomain) != 0 {
                ErrorDomain::Cocoa
            } else if CFEqual(domain, NSPOSIXErrorDomain) != 0 {
                ErrorDomain::Posix
            } else {
                ErrorDomain::Other
            };
            (domain, CFErrorGetCode(error))
        }
    }

    /// The domain and code of the error under `error`, when it has one.
    ///
    /// # Safety
    ///
    /// `error` is a live CFError or NSError.
    unsafe fn underlying_error(error: CfRef) -> Option<(ErrorDomain, isize)> {
        // SAFETY: the user info is a copy this code releases, and the value
        // read from it is used before that, and only once it is known to be
        // an error.
        unsafe {
            let info = Owned(CFErrorCopyUserInfo(error));
            if info.0.is_null() {
                return None;
            }
            let value = CFDictionaryGetValue(info.0, kCFErrorUnderlyingErrorKey);
            if value.is_null() || CFGetTypeID(value) != CFErrorGetTypeID() {
                return None;
            }
            Some(domain_and_code(value))
        }
    }

    /// # Safety
    ///
    /// `string` is null or a live CFString or NSString.
    unsafe fn cf_string(string: CfRef) -> Option<String> {
        if string.is_null() {
            return None;
        }
        // SAFETY: `string` is a live CFString, and the buffer passed is as
        // long as the size given, which is the most UTF-8 the string can
        // need plus its NUL.
        unsafe {
            let length = CFStringGetLength(string);
            // kCFNotFound (-1) when the size would overflow.
            let most = CFStringGetMaximumSizeForEncoding(length, CF_STRING_ENCODING_UTF8);
            if most < 0 {
                return None;
            }
            let size = most.checked_add(1)?;
            let mut buffer = vec![0_u8; usize::try_from(size).ok()?];
            if CFStringGetCString(
                string,
                buffer.as_mut_ptr().cast(),
                size,
                CF_STRING_ENCODING_UTF8,
            ) == 0
            {
                return None;
            }
            let end = buffer.iter().position(|byte| *byte == 0)?;
            buffer.truncate(end);
            String::from_utf8(buffer).ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EACCES, ENOENT, EPERM, EROFS, ErrorDomain, TrashErrorKind, move_to_trash, trash_error_kind,
    };
    use std::path::Path;

    #[test]
    fn foundation_and_posix_errors_get_their_kind() {
        use ErrorDomain::{Cocoa, Other, Posix};
        for (domain, code, kind) in [
            (Cocoa, 4, TrashErrorKind::NotFound),
            (Cocoa, 260, TrashErrorKind::NotFound),
            (Posix, ENOENT, TrashErrorKind::NotFound),
            (Cocoa, 257, TrashErrorKind::PermissionDenied),
            (Cocoa, 513, TrashErrorKind::PermissionDenied),
            (Posix, EPERM, TrashErrorKind::PermissionDenied),
            (Posix, EACCES, TrashErrorKind::PermissionDenied),
            (Cocoa, 642, TrashErrorKind::ReadOnly),
            (Posix, EROFS, TrashErrorKind::ReadOnly),
            (Cocoa, 512, TrashErrorKind::Other),
            (Cocoa, 3328, TrashErrorKind::Other),
            // The same numbers in another domain mean something else.
            (Other, 513, TrashErrorKind::Other),
            (Other, EPERM, TrashErrorKind::Other),
        ] {
            assert_eq!(
                trash_error_kind(domain, code, None),
                kind,
                "{domain:?} {code}"
            );
        }
    }

    /// Foundation's general write error often wraps the `errno` that really
    /// stopped the move, App Management's EPERM among them.
    #[test]
    fn a_general_error_takes_the_kind_of_the_one_underneath() {
        assert_eq!(
            trash_error_kind(ErrorDomain::Cocoa, 512, Some((ErrorDomain::Posix, EPERM))),
            TrashErrorKind::PermissionDenied
        );
        assert_eq!(
            trash_error_kind(ErrorDomain::Cocoa, 512, Some((ErrorDomain::Posix, EROFS))),
            TrashErrorKind::ReadOnly
        );
        assert_eq!(
            trash_error_kind(ErrorDomain::Cocoa, 512, Some((ErrorDomain::Posix, 5))),
            TrashErrorKind::Other
        );
        // A specific error keeps its own kind whatever is underneath.
        assert_eq!(
            trash_error_kind(ErrorDomain::Cocoa, 4, Some((ErrorDomain::Posix, EPERM))),
            TrashErrorKind::NotFound
        );
    }

    /// The path is relative, which the macOS code refuses too. So if the
    /// guard ever went missing, this test would still move nothing: it
    /// would fail with another kind instead.
    #[test]
    fn a_test_build_never_moves_anything_to_the_trash() {
        let error = move_to_trash(Path::new("Never Moved.app")).unwrap_err();
        assert_eq!(error.kind, TrashErrorKind::Refused);
        assert_eq!(
            error.to_string(),
            "a test build never moves anything to the Trash"
        );
    }

    /// These stop before any Objective-C runs, because nothing is at the
    /// path, so they are safe to run on every push.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_missing_or_relative_path_is_refused_before_macos_is_asked() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("Missing.app");
        assert_eq!(
            super::macos::move_to_trash(&missing).unwrap_err().kind,
            TrashErrorKind::NotFound
        );
        assert_eq!(
            super::macos::move_to_trash(Path::new("Missing.app"))
                .unwrap_err()
                .kind,
            TrashErrorKind::Other
        );
    }

    /// The one test that really moves something: a throwaway bundle from a
    /// temp folder goes to this account's Trash through NSFileManager, the
    /// way an uninstall moves REAPER.app. It is ignored, and even with
    /// `--ignored` it moves nothing unless RABBIT_TRASH_SMOKE is 1, which
    /// only the macos-smoke workflow sets, and only when started by hand.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "moves a throwaway folder to this account's Trash; run by the macos-smoke workflow with RABBIT_TRASH_SMOKE=1"]
    fn a_throwaway_bundle_goes_to_the_trash_and_says_where() {
        if std::env::var_os("RABBIT_TRASH_SMOKE").as_deref() != Some(std::ffi::OsStr::new("1")) {
            eprintln!("RABBIT_TRASH_SMOKE isn't 1, so nothing was moved");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let name = format!("RABBIT trash smoke {}", std::process::id());
        let bundle = dir.path().join(format!("{name}.app"));
        std::fs::create_dir_all(bundle.join("Contents").join("MacOS")).unwrap();
        std::fs::write(bundle.join("Contents").join("Info.plist"), "<plist/>").unwrap();

        let trashed = super::macos::move_to_trash(&bundle).expect("the bundle moves to the Trash");

        assert!(
            std::fs::symlink_metadata(&bundle).is_err(),
            "the original is gone"
        );
        assert!(trashed.is_absolute(), "{}", trashed.display());
        assert!(
            trashed
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&name),
            "{}",
            trashed.display()
        );
        assert!(
            trashed
                .components()
                .any(|part| part.as_os_str() == ".Trash" || part.as_os_str() == ".Trashes"),
            "{}",
            trashed.display()
        );
        match std::fs::symlink_metadata(&trashed) {
            Ok(metadata) => {
                assert!(metadata.is_dir(), "{}", trashed.display());
                std::fs::remove_dir_all(&trashed).unwrap();
            }
            // ~/.Trash is privacy-protected: without Full Disk Access the
            // test can't look inside, and the throwaway bundle stays in the
            // runner's Trash.
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("{} moved, but can't be read: {error}", trashed.display());
            }
            Err(error) => panic!("{}: {error}", trashed.display()),
        }
    }
}
