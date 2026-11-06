//! Windows registry access for uninstall entries.
//!
//! The readers look up single values such as `DisplayVersion` in
//! `(HKCU|HKLM)\Software\Microsoft\Windows\CurrentVersion\Uninstall\<key_name>`,
//! the standard location vendor installers write to so Windows' Programs and
//! Features dialog can show the version. We probe `HKCU` first because
//! per-user installers (OSARA's NSIS installer among them) record there;
//! `HKLM` (with both 64-bit and `WoW6432Node` views) is queried as a fallback
//! for machine-wide installs.
//!
//! [`HkcuUninstallRoot`] also changes per-user entries: it reads one whole,
//! writes one back exactly as it was, and deletes one. Nothing here writes to
//! HKLM.
//!
//! On non-Windows platforms the readers return `None`, an entry is never
//! there, and writing one fails as unsupported.

use std::io;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_SZ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};

const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

/// The registry's type codes for the two string types (`winnt.h`), spelled
/// out here so the value helpers below work, and are tested, off Windows too.
const REG_SZ_KIND: u32 = 1;
const REG_EXPAND_SZ_KIND: u32 = 2;

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

/// A key under `HKEY_CURRENT_USER` whose subkeys are uninstall entries, the
/// way Windows keeps its per-user list of installed programs. RABBIT only
/// ever changes per-user entries, so the hive is fixed; the path is explicit
/// so a test can point at a throwaway key instead of the real list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HkcuUninstallRoot {
    path: String,
}

impl HkcuUninstallRoot {
    /// Windows' real per-user list,
    /// `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall`.
    pub fn installed_programs() -> Self {
        Self::at(UNINSTALL_KEY)
    }

    /// Any other key under HKCU, written without the `HKEY_CURRENT_USER\`
    /// prefix.
    pub fn at(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Every value of the entry `name`, or `None` when there is no such
    /// entry. Always `None` off Windows.
    pub fn read_entry(&self, name: &str) -> io::Result<Option<UninstallEntry>> {
        imp::read_entry(&self.entry_path(name)?)
    }

    /// Make the entry `name` hold exactly `entry`'s values: create it when
    /// it is missing, delete the values `entry` doesn't have, and write
    /// every value it has, type and bytes unchanged. Value names ignore case,
    /// as Windows' do; a value already there keeps its spelling, because
    /// Windows keeps it even through a delete and a new write. Subkeys are
    /// left alone. Fails as unsupported off Windows.
    pub fn write_entry(&self, name: &str, entry: &UninstallEntry) -> io::Result<()> {
        imp::write_entry(&self.entry_path(name)?, entry)
    }

    /// Delete the entry `name`. `Ok(false)` when there was none, and always
    /// off Windows. An entry with subkeys fails and stays: no installer
    /// RABBIT runs writes one, so whatever it holds isn't theirs.
    pub fn delete_entry(&self, name: &str) -> io::Result<bool> {
        imp::delete_entry(&self.entry_path(name)?)
    }

    /// `<root>\<name>`. An empty root or name, a root with a separator at
    /// either end, and a name with a separator are refused, so a call only
    /// ever reaches one entry right below the root: never the root itself,
    /// a key above it, or one deeper down.
    fn entry_path(&self, name: &str) -> io::Result<String> {
        let root = self.path.as_str();
        if root.is_empty() || root.starts_with('\\') || root.ends_with('\\') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{root:?} isn't a registry key path under HKCU"),
            ));
        }
        if name.is_empty() || name.contains('\\') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name:?} isn't one key name"),
            ));
        }
        Ok(format!("{root}\\{name}"))
    }
}

/// The values of one uninstall entry, each with its registry type and its
/// bytes as Windows reads them, so an entry read before an installer runs
/// can be written back unchanged after it. One exception is out of RABBIT's
/// hands: Windows sometimes reads a `REG_EXPAND_SZ` stored without its
/// closing NUL with one added, which reads as the same text. Installers
/// write the NUL. The values are kept in name order: the registry gives them
/// no order that means anything, and two reads of the same entry should
/// compare equal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallEntry {
    values: Vec<RegistryValue>,
}

impl UninstallEntry {
    /// An entry with these values, one per name.
    pub fn new(mut values: Vec<RegistryValue>) -> Self {
        values.sort_by(|left, right| left.name.cmp(&right.name));
        Self { values }
    }

    pub fn values(&self) -> &[RegistryValue] {
        &self.values
    }

    /// The text of the string value `name`; see [`RegistryValue::as_string`].
    /// Registry value names ignore case, so this does too.
    pub fn string_value(&self, name: &str) -> Option<String> {
        self.values
            .iter()
            .find(|value| same_value_name(&value.name, name))
            .and_then(RegistryValue::as_string)
    }
}

/// One registry value: its name, its type code, and its data as raw bytes.
/// The empty name is the key's default value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryValue {
    name: String,
    kind: u32,
    data: Vec<u8>,
}

impl RegistryValue {
    /// A `REG_SZ` value, stored the way Windows stores one: UTF-16 with a
    /// closing NUL.
    pub fn string(name: impl Into<String>, text: &str) -> Self {
        Self {
            name: name.into(),
            kind: REG_SZ_KIND,
            data: text
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The text of a `REG_SZ` or `REG_EXPAND_SZ` value, without its closing
    /// NULs and with nothing expanded. `None` for other types and for data
    /// that isn't UTF-16.
    pub fn as_string(&self) -> Option<String> {
        if self.kind != REG_SZ_KIND && self.kind != REG_EXPAND_SZ_KIND {
            return None;
        }
        // A stray odd byte at the end isn't part of any character.
        let (pairs, _) = self.data.as_chunks::<2>();
        let mut units: Vec<u16> = pairs.iter().copied().map(u16::from_le_bytes).collect();
        while units.last() == Some(&0) {
            units.pop();
        }
        String::from_utf16(&units).ok()
    }
}

fn same_value_name(left: &str, right: &str) -> bool {
    left.chars()
        .flat_map(char::to_lowercase)
        .eq(right.chars().flat_map(char::to_lowercase))
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::ptr;

    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND,
        ERROR_SUCCESS, WIN32_ERROR,
    };
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE,
        RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteValueW, RegEnumValueW, RegOpenKeyExW,
        RegQueryInfoKeyW, RegSetValueExW,
    };

    use super::{RegistryValue, UninstallEntry, same_value_name, wide_string};

    /// How often reading one value may find it grew since its size was
    /// asked for, before the read gives up.
    const GROWN_VALUE_RETRIES: u32 = 8;

    /// An open registry key, closed when dropped so no error path leaks it.
    struct OpenKey(HKEY);

    impl Drop for OpenKey {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful open or create, and
            // only this drop closes it.
            unsafe { RegCloseKey(self.0) };
        }
    }

    pub(super) fn read_entry(path: &str) -> io::Result<Option<UninstallEntry>> {
        let path = wide_string(path);
        let mut hkey: HKEY = ptr::null_mut();
        // SAFETY: `path` is NUL-terminated and outlives the call, and `hkey`
        // is a valid place for the handle.
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut hkey,
            )
        };
        if is_missing(status) {
            return Ok(None);
        }
        check(status)?;
        let key = OpenKey(hkey);
        Ok(Some(UninstallEntry::new(read_values(&key)?)))
    }

    pub(super) fn write_entry(path: &str, entry: &UninstallEntry) -> io::Result<()> {
        let path = wide_string(path);
        let mut hkey: HKEY = ptr::null_mut();
        // SAFETY: as in `read_entry`; the class, the security attributes and
        // the disposition may all be null.
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                ptr::null(),
                &mut hkey,
                ptr::null_mut(),
            )
        };
        check(status)?;
        let key = OpenKey(hkey);
        for present in read_values(&key)? {
            if !entry
                .values
                .iter()
                .any(|value| same_value_name(&value.name, &present.name))
            {
                let name = wide_string(&present.name);
                // SAFETY: `name` is NUL-terminated and outlives the call.
                check(unsafe { RegDeleteValueW(key.0, name.as_ptr()) })?;
            }
        }
        for value in &entry.values {
            let name = wide_string(&value.name);
            let length = u32::try_from(value.data.len()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "registry value too large")
            })?;
            // SAFETY: `name` is NUL-terminated, and `data` holds `length`
            // bytes; both outlive the call.
            check(unsafe {
                RegSetValueExW(
                    key.0,
                    name.as_ptr(),
                    0,
                    value.kind,
                    value.data.as_ptr(),
                    length,
                )
            })?;
        }
        Ok(())
    }

    pub(super) fn delete_entry(path: &str) -> io::Result<bool> {
        let path = wide_string(path);
        // RegDeleteKeyW, not RegDeleteTreeW: it refuses a key that has
        // subkeys, which is what `delete_entry` promises.
        // SAFETY: `path` is NUL-terminated and outlives the call.
        let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, path.as_ptr()) };
        if is_missing(status) {
            return Ok(false);
        }
        check(status).map(|()| true)
    }

    fn read_values(key: &OpenKey) -> io::Result<Vec<RegistryValue>> {
        let (mut name_capacity, mut data_capacity) = value_capacities(key)?;
        let mut values = Vec::new();
        let mut index = 0u32;
        let mut retries = 0;
        loop {
            let mut name = vec![0u16; name_capacity];
            let mut data = vec![0u8; data_capacity];
            let mut name_length = name_capacity as u32;
            let mut data_length = data_capacity as u32;
            let mut kind = 0u32;
            // SAFETY: each buffer holds at least the length passed with it,
            // and the reserved pointer is null as required.
            let status = unsafe {
                RegEnumValueW(
                    key.0,
                    index,
                    name.as_mut_ptr(),
                    &mut name_length,
                    ptr::null(),
                    &mut kind,
                    data.as_mut_ptr(),
                    &mut data_length,
                )
            };
            match status {
                ERROR_SUCCESS => {
                    name.truncate(name_length as usize);
                    data.truncate(data_length as usize);
                    let name = String::from_utf16(&name).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "a registry value's name isn't valid UTF-16",
                        )
                    })?;
                    values.push(RegistryValue { name, kind, data });
                    index += 1;
                    retries = 0;
                }
                ERROR_NO_MORE_ITEMS => return Ok(values),
                // Another program made a value bigger after its size was
                // asked for. Ask again, with at least twice the room.
                ERROR_MORE_DATA if retries < GROWN_VALUE_RETRIES => {
                    retries += 1;
                    let (names, data) = value_capacities(key)?;
                    name_capacity = names.max(name_capacity * 2);
                    data_capacity = data.max(data_capacity * 2);
                }
                status => return Err(os_error(status)),
            }
        }
    }

    /// Buffer sizes that fit the key's longest value name, in UTF-16 units
    /// with room for the closing NUL, and its largest value, in bytes. The
    /// data buffer gets at least one byte, so its pointer is always real.
    fn value_capacities(key: &OpenKey) -> io::Result<(usize, usize)> {
        let mut longest_name = 0u32;
        let mut largest_data = 0u32;
        // SAFETY: every pointer is either null, which the API allows for
        // the facts not asked for, or a valid place for a count.
        check(unsafe {
            RegQueryInfoKeyW(
                key.0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut longest_name,
                &mut largest_data,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        })?;
        Ok((longest_name as usize + 1, (largest_data as usize).max(1)))
    }

    fn is_missing(status: WIN32_ERROR) -> bool {
        status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND
    }

    fn check(status: WIN32_ERROR) -> io::Result<()> {
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(os_error(status))
        }
    }

    fn os_error(status: WIN32_ERROR) -> io::Error {
        io::Error::from_raw_os_error(status as i32)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::io;

    use super::UninstallEntry;

    pub(super) fn read_entry(_path: &str) -> io::Result<Option<UninstallEntry>> {
        Ok(None)
    }

    pub(super) fn write_entry(_path: &str, _entry: &UninstallEntry) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "only Windows has a registry",
        ))
    }

    pub(super) fn delete_entry(_path: &str) -> io::Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16_bytes(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn the_real_root_is_the_per_user_list_of_installed_programs() {
        assert_eq!(
            HkcuUninstallRoot::installed_programs().path(),
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall"
        );
    }

    #[test]
    fn calls_reach_only_one_entry_right_below_the_root() {
        let root = HkcuUninstallRoot::at("Software\\RABBIT-test-paths");
        assert_eq!(
            root.entry_path("OSARA").unwrap(),
            "Software\\RABBIT-test-paths\\OSARA"
        );
        for name in ["", "OSARA\\Child", "\\OSARA", "OSARA\\"] {
            let error = root.delete_entry(name).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name:?}");
            let error = root.read_entry(name).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name:?}");
        }
        for path in ["", "\\Software", "Software\\"] {
            let error = HkcuUninstallRoot::at(path)
                .write_entry("OSARA", &UninstallEntry::default())
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{path:?}");
        }
    }

    #[test]
    fn string_values_read_as_windows_reads_them() {
        let entry = UninstallEntry::new(vec![
            RegistryValue::string("UninstallString", "\"C:\\REAPER\\osara\\uninstall.exe\""),
            // Some programs write strings without the closing NUL.
            RegistryValue {
                name: "DisplayIcon".to_string(),
                kind: REG_EXPAND_SZ_KIND,
                data: utf16_bytes("%APPDATA%\\REAPER\\osara.ico"),
            },
            RegistryValue {
                name: "NoModify".to_string(),
                kind: 4,
                data: 1u32.to_le_bytes().to_vec(),
            },
        ]);

        assert_eq!(
            entry.string_value("uninstallstring").as_deref(),
            Some("\"C:\\REAPER\\osara\\uninstall.exe\"")
        );
        assert_eq!(
            entry.string_value("DISPLAYICON").as_deref(),
            Some("%APPDATA%\\REAPER\\osara.ico")
        );
        assert_eq!(entry.string_value("NoModify"), None);
        assert_eq!(entry.string_value("Publisher"), None);
    }

    #[test]
    fn entries_compare_equal_whatever_order_their_values_came_in() {
        let display_name = RegistryValue::string("DisplayName", "OSARA");
        let publisher = RegistryValue::string("Publisher", "NV Access");
        assert_eq!(
            UninstallEntry::new(vec![display_name.clone(), publisher.clone()]),
            UninstallEntry::new(vec![publisher, display_name])
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_there_is_never_an_entry_and_none_can_be_written() {
        let root = HkcuUninstallRoot::at("Software\\RABBIT-test-elsewhere");
        assert_eq!(root.read_entry("OSARA").unwrap(), None);
        assert!(!root.delete_entry("OSARA").unwrap());
        let error = root
            .write_entry("OSARA", &UninstallEntry::default())
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_DWORD, REG_EXPAND_SZ, RegDeleteKeyW, RegDeleteTreeW,
    };

    use super::*;

    const THROWAWAY_PREFIX: &str = "Software\\RABBIT-test-";

    /// A key of one test's own under `HKCU\Software`, deleted with
    /// everything in it when dropped, also when the test panics. No test
    /// here touches the real list of installed programs.
    struct ThrowawayKey {
        path: String,
    }

    impl ThrowawayKey {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default();
            Self {
                path: format!(
                    "{THROWAWAY_PREFIX}{}-{nanos}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ),
            }
        }

        fn root(&self) -> HkcuUninstallRoot {
            HkcuUninstallRoot::at(self.path.clone())
        }

        /// The key's name right below `HKCU\Software`.
        fn name(&self) -> &str {
            self.path.trim_start_matches("Software\\")
        }
    }

    impl Drop for ThrowawayKey {
        fn drop(&mut self) {
            // The recursive delete only ever sees a name this guard made up.
            if !self.path.starts_with(THROWAWAY_PREFIX) || self.name().contains('\\') {
                return;
            }
            let path = wide_string(&self.path);
            // SAFETY: `path` is NUL-terminated and outlives both calls.
            // RegDeleteTreeW empties the key and RegDeleteKeyW removes
            // whatever is left of it; a failure only leaves a test key.
            unsafe {
                RegDeleteTreeW(HKEY_CURRENT_USER, path.as_ptr());
                RegDeleteKeyW(HKEY_CURRENT_USER, path.as_ptr());
            }
        }
    }

    fn osara_entry() -> UninstallEntry {
        UninstallEntry::new(vec![
            RegistryValue::string("DisplayName", "OSARA"),
            RegistryValue::string("DisplayVersion", "2025.3.6.1234,abcdef12"),
            RegistryValue::string("Publisher", "NV Access"),
            RegistryValue::string(
                "UninstallString",
                "\"C:\\Users\\Someone\\AppData\\Roaming\\REAPER\\osara\\uninstall.exe\"",
            ),
        ])
    }

    #[test]
    fn the_string_type_codes_match_windows() {
        assert_eq!(REG_SZ_KIND, REG_SZ);
        assert_eq!(REG_EXPAND_SZ_KIND, REG_EXPAND_SZ);
    }

    #[test]
    fn an_entry_reads_back_byte_for_byte_whatever_its_value_types() {
        let key = ThrowawayKey::new();
        let root = key.root();
        assert_eq!(root.read_entry("OSARA").unwrap(), None);

        let mut values = osara_entry().values().to_vec();
        values.extend([
            RegistryValue::string("", "the default value"),
            RegistryValue {
                name: "DisplayIcon".to_string(),
                kind: REG_EXPAND_SZ,
                data: "%APPDATA%\\osara.ico"
                    .encode_utf16()
                    .chain(Some(0))
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            },
            RegistryValue {
                name: "NoModify".to_string(),
                kind: REG_DWORD,
                data: 1u32.to_le_bytes().to_vec(),
            },
        ]);
        let entry = UninstallEntry::new(values);
        root.write_entry("OSARA", &entry).unwrap();

        assert_eq!(root.read_entry("OSARA").unwrap(), Some(entry));
    }

    #[test]
    fn writing_an_entry_again_leaves_exactly_the_new_values() {
        let key = ThrowawayKey::new();
        let root = key.root();
        let mut values = osara_entry().values().to_vec();
        values.push(RegistryValue::string("Comments", "left over"));
        root.write_entry("OSARA", &UninstallEntry::new(values))
            .unwrap();

        // Two values dropped, two changed, one the same.
        let replacement = UninstallEntry::new(vec![
            RegistryValue::string("DisplayName", "OSARA"),
            RegistryValue::string("Publisher", "James Teh"),
            RegistryValue::string("UninstallString", "\"D:\\Portable\\osara\\uninstall.exe\""),
        ]);
        root.write_entry("OSARA", &replacement).unwrap();
        assert_eq!(root.read_entry("OSARA").unwrap(), Some(replacement));

        // A name spelled differently only in case is the same value, and it
        // keeps the spelling it had.
        let respelled = UninstallEntry::new(vec![RegistryValue::string("displayname", "OSARA 2")]);
        root.write_entry("OSARA", &respelled).unwrap();
        let read = root.read_entry("OSARA").unwrap().unwrap();
        let names: Vec<&str> = read.values().iter().map(RegistryValue::name).collect();
        assert_eq!(names, ["DisplayName"]);
        assert_eq!(read.string_value("DisplayName").as_deref(), Some("OSARA 2"));
    }

    #[test]
    fn deleting_an_entry_says_whether_it_was_there() {
        let key = ThrowawayKey::new();
        let root = key.root();
        root.write_entry("OSARA", &osara_entry()).unwrap();

        assert!(root.delete_entry("OSARA").unwrap());
        assert_eq!(root.read_entry("OSARA").unwrap(), None);
        assert!(!root.delete_entry("OSARA").unwrap());
        // A root that doesn't exist has no entries either.
        let missing_root = HkcuUninstallRoot::at(format!("{}\\Missing", key.path));
        assert!(!missing_root.delete_entry("OSARA").unwrap());
        assert_eq!(missing_root.read_entry("OSARA").unwrap(), None);
    }

    #[test]
    fn an_entry_with_subkeys_stays() {
        let key = ThrowawayKey::new();
        let root = key.root();
        root.write_entry("OSARA", &osara_entry()).unwrap();
        HkcuUninstallRoot::at(format!("{}\\OSARA", key.path))
            .write_entry("Child", &osara_entry())
            .unwrap();

        assert!(root.delete_entry("OSARA").is_err());
        assert_eq!(root.read_entry("OSARA").unwrap(), Some(osara_entry()));
    }

    #[test]
    fn the_throwaway_key_is_gone_once_its_guard_drops() {
        let key = ThrowawayKey::new();
        let name = key.name().to_string();
        key.root().write_entry("OSARA", &osara_entry()).unwrap();
        HkcuUninstallRoot::at(format!("{}\\OSARA", key.path))
            .write_entry("Child", &osara_entry())
            .unwrap();
        let software = HkcuUninstallRoot::at("Software");
        assert!(software.read_entry(&name).unwrap().is_some());

        drop(key);

        assert_eq!(software.read_entry(&name).unwrap(), None);
    }
}
