//! Small Windows helpers shared by the uninstall primitives: wide paths,
//! the OS random source, security descriptors and SIDs, and opening a path
//! without following a link.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows_sys::Win32::Foundation::{HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, OWNER_SECURITY_INFORMATION, PSID, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileAttributeTagInfo,
    GetFileInformationByHandleEx,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// `path` as a NUL-terminated UTF-16 string. A path with a NUL inside is
/// refused: Windows would read only the part before it.
pub(crate) fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    wide_str(path.as_os_str())
}

pub(crate) fn wide_str(text: &std::ffi::OsStr) -> io::Result<Vec<u16>> {
    let wide: Vec<u16> = text.encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a path or name contains a NUL character",
        ));
    }
    Ok(wide.into_iter().chain(Some(0)).collect())
}

/// `N` bytes from the OS random source, written as lowercase hex.
pub(crate) fn random_hex<const N: usize>() -> io::Result<String> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Memory Windows allocated with `LocalAlloc` for RABBIT, freed on drop.
/// Wrap only pointers an API says to free with `LocalFree`, or null.
pub(crate) struct LocalMemory(pub(crate) *mut core::ffi::c_void);

impl LocalMemory {
    pub(crate) fn as_ptr(&self) -> *mut core::ffi::c_void {
        self.0
    }
}

impl Drop for LocalMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer came from an API that says to free it with
            // LocalFree, and only this drop frees it.
            unsafe { LocalFree(self.0) };
        }
    }
}

/// A self-relative security descriptor built from an SDDL string.
pub(crate) fn security_descriptor_from_sddl(sddl: &str) -> io::Result<LocalMemory> {
    let wide = wide_str(std::ffi::OsStr::new(sddl))?;
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: `wide` is NUL-terminated and outlives the call; the size
    // pointer may be null.
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(LocalMemory(descriptor))
}

/// The string form of a SID, such as `S-1-5-32-544`.
///
/// # Safety
///
/// `sid` must point to a valid SID for the whole call.
pub(crate) unsafe fn sid_to_string(sid: PSID) -> io::Result<String> {
    let mut text = std::ptr::null_mut();
    // SAFETY: the caller vouches for `sid`; `text` is a valid place for the
    // pointer Windows allocates.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let text = LocalMemory(text.cast());
    let start = text.as_ptr().cast::<u16>();
    let mut length = 0;
    // SAFETY: Windows returned a NUL-terminated string, read up to its NUL.
    while unsafe { *start.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: the `length` units before the NUL are part of the string.
    let units = unsafe { std::slice::from_raw_parts(start, length) };
    String::from_utf16(units)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "a SID isn't valid UTF-16"))
}

/// The SID of the account this process runs as.
pub(crate) fn current_user_sid() -> io::Result<String> {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};

    let mut raw_token = std::ptr::null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo handle that needs no
    // closing, and `raw_token` is a valid place for the token handle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call succeeded, so the handle is ours to close.
    let token = unsafe { OwnedHandle::from_raw_handle(raw_token) };

    // TOKEN_USER is followed by the SID it points to, so ask for the size.
    let mut needed = 0u32;
    // SAFETY: a null buffer of length 0 only asks for the size.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if needed == 0 {
        return Err(io::Error::last_os_error());
    }
    // u64 elements keep the buffer aligned for TOKEN_USER's pointer.
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    // SAFETY: the buffer holds at least `needed` bytes.
    let ok = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * 8) as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Windows filled the buffer with a TOKEN_USER whose SID lies
    // inside the same buffer, which lives until the end of this function.
    unsafe {
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        sid_to_string(user.User.Sid)
    }
}

/// Open `path` itself, never what a link there points to: a symbolic
/// link, a junction or any other reparse point opens as the link. Folders
/// open too. Other programs may keep reading, writing and deleting it.
pub(crate) fn open_no_follow(path: &Path, access: u32) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(access)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

/// The attributes and reparse tag of an open file or folder.
pub(crate) fn attribute_tag(file: &File) -> io::Result<FILE_ATTRIBUTE_TAG_INFO> {
    let mut info = FILE_ATTRIBUTE_TAG_INFO::default();
    // SAFETY: `info` is a FILE_ATTRIBUTE_TAG_INFO and the size passed is
    // its size; the handle is open for the whole call.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileAttributeTagInfo,
            (&raw mut info).cast(),
            std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info)
}

/// The SID that owns an open file or folder. The handle needs
/// `READ_CONTROL`.
pub(crate) fn owner_sid_of(handle: HANDLE) -> io::Result<String> {
    let mut owner: PSID = std::ptr::null_mut();
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: only the owner is asked for; the other out pointers may be
    // null. `owner` points into `descriptor`, which is freed after use.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let descriptor = LocalMemory(descriptor);
    if owner.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the file has no owner",
        ));
    }
    // SAFETY: `owner` points into `descriptor`, which is still alive.
    let sid = unsafe { sid_to_string(owner) };
    drop(descriptor);
    sid
}

/// Reading security descriptors back, for tests that check what was set.
#[cfg(test)]
pub(crate) mod test_support {
    use std::io;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
        SE_OBJECT_TYPE,
    };
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

    use super::LocalMemory;

    /// The DACL of an open object in SDDL. The handle needs `READ_CONTROL`.
    pub(crate) fn dacl_sddl_of(handle: HANDLE, object_type: SE_OBJECT_TYPE) -> String {
        let mut descriptor = std::ptr::null_mut();
        // SAFETY: only the descriptor is asked for; it is freed below.
        let status = unsafe {
            GetSecurityInfo(
                handle,
                object_type,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(status, 0, "{}", io::Error::from_raw_os_error(status as i32));
        let descriptor = LocalMemory(descriptor);
        descriptor_sddl(descriptor.as_ptr(), DACL_SECURITY_INFORMATION)
    }

    /// The parts `parts` of a security descriptor in SDDL.
    pub(crate) fn descriptor_sddl(descriptor: *mut core::ffi::c_void, parts: u32) -> String {
        let mut text = std::ptr::null_mut();
        // SAFETY: `descriptor` is a valid descriptor; the length pointer
        // may be null.
        let ok = unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                SDDL_REVISION_1,
                parts,
                &mut text,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(ok, 0, "{}", io::Error::last_os_error());
        let text = LocalMemory(text.cast());
        let start = text.as_ptr().cast::<u16>();
        let mut length = 0;
        // SAFETY: Windows returned a NUL-terminated string.
        unsafe {
            while *start.add(length) != 0 {
                length += 1;
            }
            String::from_utf16(std::slice::from_raw_parts(start, length)).unwrap()
        }
    }

    /// The access control entries of an SDDL DACL, in order.
    pub(crate) fn aces(sddl: &str) -> Vec<String> {
        sddl.split('(')
            .skip(1)
            .map(|ace| ace.trim_end_matches(')').to_string())
            .collect()
    }
}
