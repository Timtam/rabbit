//! Windows registry probes for non-RABBIT-managed installs.
//!
//! Today this exposes `read_uninstall_display_version`, which reads the
//! `DisplayVersion` REG_SZ value from
//! `(HKCU|HKLM)\Software\Microsoft\Windows\CurrentVersion\Uninstall\<key_name>`,
//! the standard location vendor installers write to so Windows' Programs and
//! Features dialog can show the version. We probe `HKCU` first because
//! per-user installers (OSARA's NSIS installer among them) record there;
//! `HKLM` (with both 64-bit and `WoW6432Node` views) is queried as a fallback
//! for machine-wide installs.
//!
//! On non-Windows platforms the function returns `None`.

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_SZ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};

#[cfg(windows)]
const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

#[cfg_attr(not(windows), allow(unused_variables))]
pub fn read_uninstall_display_version(key_name: &str) -> Option<String> {
    read_uninstall_value(key_name, "DisplayVersion")
}

/// Read the `InstallLocation` REG_SZ value off the Programs-and-Features
/// uninstall key for `key_name`. Used to find non-default REAPER install
/// directories (e.g. the default 64-bit `Program Files\REAPER (x64)` path that
/// our hardcoded candidates do not list).
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn read_uninstall_install_location(key_name: &str) -> Option<String> {
    read_uninstall_value(key_name, "InstallLocation")
}

/// Read an arbitrary REG_SZ value off the Programs-and-Features uninstall
/// key for `key_name`. Used when an installer wrote a non-standard value
/// (e.g. NSIS scripts that store the install dir under `UninstallDirectory`
/// rather than `InstallLocation`). Returns `None` when the key, the value,
/// or its REG_SZ payload is missing, and on non-Windows hosts.
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn read_uninstall_value(key_name: &str, value_name: &str) -> Option<String> {
    read_uninstall_value_impl(key_name, value_name)
}

/// `true` when an arbitrary HKLM subkey exists in either the 64-bit or 32-bit
/// (`WOW6432Node`) registry view. `subkey` is a path under HKLM, written
/// without the `HKEY_LOCAL_MACHINE\` prefix (e.g.
/// `"SOFTWARE\\Native Instruments\\Komplete Kontrol"`). Used by the
/// Komplete Kontrol probe; lives here next to the other registry helpers
/// rather than inlined into the probe so future host-detection modules can
/// reuse it. Always `false` on non-Windows hosts.
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn hklm_subkey_exists(subkey: &str) -> bool {
    hklm_subkey_exists_impl(subkey)
}

#[cfg(windows)]
fn read_uninstall_value_impl(key_name: &str, value_name: &str) -> Option<String> {
    let subkey = format!("{UNINSTALL_KEY}\\{key_name}");
    // HKCU has no 32/64-bit redirection, so a single view suffices there.
    // HKLM gets both views to cover 32-bit installers on 64-bit Windows.
    let candidates: [(HKEY, u32); 3] = [
        (HKEY_CURRENT_USER, 0),
        (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
    ];
    for (root, view) in candidates {
        if let Some(value) = query_uninstall_value(root, &subkey, value_name, view) {
            return Some(value);
        }
    }
    None
}

#[cfg(not(windows))]
fn read_uninstall_value_impl(_key_name: &str, _value_name: &str) -> Option<String> {
    None
}

#[cfg(windows)]
fn hklm_subkey_exists_impl(subkey: &str) -> bool {
    let subkey_w = wide_string(subkey);
    for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
        let mut hkey = std::ptr::null_mut();
        let access = KEY_QUERY_VALUE | view;
        let status =
            unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, subkey_w.as_ptr(), 0, access, &mut hkey) };
        if status == 0 && !hkey.is_null() {
            unsafe {
                let _ = RegCloseKey(hkey);
            }
            return true;
        }
    }
    false
}

#[cfg(not(windows))]
fn hklm_subkey_exists_impl(_subkey: &str) -> bool {
    false
}

#[cfg(windows)]
fn query_uninstall_value(root: HKEY, subkey: &str, value_name: &str, view: u32) -> Option<String> {
    let subkey_w = wide_string(subkey);
    let value_w = wide_string(value_name);
    let mut hkey = std::ptr::null_mut();
    let access = KEY_QUERY_VALUE | view;
    let status = unsafe { RegOpenKeyExW(root, subkey_w.as_ptr(), 0, access, &mut hkey) };
    if status != 0 || hkey.is_null() {
        return None;
    }

    let result = read_string_value(hkey, value_w.as_ptr());
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    result
}

#[cfg(windows)]
fn read_string_value(
    hkey: windows_sys::Win32::System::Registry::HKEY,
    value_name: *const u16,
) -> Option<String> {
    let mut value_type = 0u32;
    let mut data_size = 0u32;
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            value_name,
            std::ptr::null_mut(),
            &mut value_type,
            std::ptr::null_mut(),
            &mut data_size,
        )
    };
    if status != 0 || value_type != REG_SZ || data_size == 0 {
        return None;
    }

    // data_size is in bytes; allocate as u16 buffer with rounding up.
    let chars = (data_size as usize).div_ceil(2);
    let mut buffer = vec![0u16; chars];
    let mut data_size_inout = (chars * 2) as u32;
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            value_name,
            std::ptr::null_mut(),
            &mut value_type,
            buffer.as_mut_ptr().cast::<u8>(),
            &mut data_size_inout,
        )
    };
    if status != 0 || value_type != REG_SZ {
        return None;
    }

    // Trim trailing NUL terminator(s) if present.
    while buffer.last().copied() == Some(0) {
        buffer.pop();
    }
    String::from_utf16(&buffer).ok().filter(|s| !s.is_empty())
}

#[cfg(windows)]
fn wide_string(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(Some(0))
        .collect()
}
