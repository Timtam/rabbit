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
//! [`UninstallViews`] and [`pending_file_renames`] only read. They serve the
//! trust checks before a vendor uninstaller runs with administrator rights,
//! which believe HKLM alone, because only administrators can change it.
//!
//! On non-Windows platforms the readers return `None` or nothing, an entry
//! is never there, and writing one fails as unsupported.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_SZ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};

const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

/// The machine-wide list of installed programs, under HKLM.
const MACHINE_UNINSTALL_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

/// Where Windows keeps the file operations it finishes at the next restart.
const SESSION_MANAGER_KEY: &str = "SYSTEM\\CurrentControlSet\\Control\\Session Manager";

/// The values holding those operations. `MoveFileEx` writes the first;
/// Windows also reads the second, which some servicing code writes.
const PENDING_RENAME_VALUES: [&str; 2] = [
    "PendingFileRenameOperations",
    "PendingFileRenameOperations2",
];

/// The registry's type codes for the string types (`winnt.h`), spelled out
/// here so the value helpers below work, and are tested, off Windows too.
const REG_SZ_KIND: u32 = 1;
const REG_EXPAND_SZ_KIND: u32 = 2;
const REG_MULTI_SZ_KIND: u32 = 7;

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

    fn entry_path(&self, name: &str) -> io::Result<String> {
        child_key_path(&self.path, name)
    }
}

/// `<root>\<name>`. An empty root or name, a root with a separator at either
/// end, and a name with a separator are refused, so a call only ever reaches
/// one entry right below the root: never the root itself, a key above it, or
/// one deeper down.
fn child_key_path(root: &str, name: &str) -> io::Result<String> {
    if root.is_empty() || root.starts_with('\\') || root.ends_with('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{root:?} isn't a registry key path"),
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

/// One of the two views 64-bit Windows keeps of `HKLM\SOFTWARE`. A 32-bit
/// installer writes its uninstall entry into the 32-bit view
/// (`WOW6432Node`), a 64-bit one into the native view, so a trust check
/// looks at both and knows which view each value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryView {
    /// The native 64-bit view, opened with `KEY_WOW64_64KEY`.
    Bit64,
    /// The 32-bit view, opened with `KEY_WOW64_32KEY`.
    Bit32,
}

impl RegistryView {
    /// Both views, the native one first.
    pub const ALL: [RegistryView; 2] = [RegistryView::Bit64, RegistryView::Bit32];
}

/// The hive a registry root lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hive {
    LocalMachine,
    CurrentUser,
}

/// A list of uninstall entries, read in every registry view. Only reads.
///
/// The real list is HKLM's, [`Self::local_machine`]. Only administrators
/// can change it, which is why an elevated uninstall may trust what it
/// says, and why nothing here falls back to HKCU. Tests point
/// [`Self::current_user_at`] at a throwaway key instead; HKCU can't be the
/// real list, because that constructor takes no HKLM path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallViews {
    hive: Hive,
    path: String,
}

impl UninstallViews {
    /// `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`, the list
    /// of programs installed for every user.
    pub fn local_machine() -> Self {
        Self {
            hive: Hive::LocalMachine,
            path: MACHINE_UNINSTALL_KEY.to_string(),
        }
    }

    /// A key under `HKEY_CURRENT_USER`, written without that prefix. For
    /// tests: HKCU shares one key between both views, so every view finds
    /// an entry made there.
    pub fn current_user_at(path: impl Into<String>) -> Self {
        Self {
            hive: Hive::CurrentUser,
            path: path.into(),
        }
    }

    /// Whether this is HKLM's real list, the only one an elevated step may
    /// believe.
    pub fn is_local_machine(&self) -> bool {
        self.hive == Hive::LocalMachine && self.path == MACHINE_UNINSTALL_KEY
    }

    /// The root's path inside its hive.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Whether the entry `key_name` exists in any view.
    pub fn key_exists(&self, key_name: &str) -> io::Result<bool> {
        Ok(!self.views_with_key(key_name)?.is_empty())
    }

    /// The views the entry `key_name` exists in, native first. Empty when
    /// it is in neither, and always off Windows. A key that is there but
    /// can't be opened is an error, never "missing".
    pub fn views_with_key(&self, key_name: &str) -> io::Result<Vec<RegistryView>> {
        Ok(self
            .read_values(key_name, &[])?
            .into_iter()
            .map(|found| found.view)
            .collect())
    }

    /// The string values `value_names` of the entry `key_name`, once for
    /// every view the entry exists in, native first.
    ///
    /// A value that is missing, or isn't `REG_SZ` or `REG_EXPAND_SZ`, reads
    /// as `None`. `REG_EXPAND_SZ` text comes back unexpanded, so a value
    /// such as `%ProgramFiles%\REAPER\Uninstall.exe` fails any check that
    /// wants an exact folder: careful, never wrong in the dangerous
    /// direction. Empty off Windows.
    pub fn read_values(&self, key_name: &str, value_names: &[&str]) -> io::Result<Vec<ViewValues>> {
        let path = child_key_path(&self.path, key_name)?;
        let mut found = Vec::new();
        for view in RegistryView::ALL {
            if let Some(values) = imp::read_view_values(self.hive, &path, view, value_names)? {
                found.push(ViewValues {
                    view,
                    values: value_names
                        .iter()
                        .map(|name| name.to_string())
                        .zip(values)
                        .collect(),
                });
            }
        }
        Ok(found)
    }
}

/// The values one view of an uninstall entry held, as
/// [`UninstallViews::read_values`] read them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewValues {
    view: RegistryView,
    values: Vec<(String, Option<String>)>,
}

impl ViewValues {
    pub fn view(&self) -> RegistryView {
        self.view
    }

    /// The text of the value `name`; `None` when it is missing, isn't a
    /// string, or wasn't asked for. Names ignore case, as Windows' do.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(asked, _)| same_value_name(asked, name))
            .and_then(|(_, value)| value.as_deref())
    }
}

/// A file operation Windows finishes at the next restart, as `MoveFileEx`
/// with `MOVEFILE_DELAY_UNTIL_REBOOT` recorded it. Installers and
/// uninstallers use this for files that were in use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFileRename {
    /// The file to move or delete.
    pub source: PathBuf,
    /// Where it moves to; `None` means it is deleted.
    pub target: Option<PathBuf>,
}

impl PendingFileRename {
    /// Whether `path` is this operation's source or target, ignoring case
    /// and the separator spelling, as Windows does.
    pub fn mentions(&self, path: &Path) -> bool {
        let wanted = comparable_path(path);
        comparable_path(&self.source) == wanted
            || self
                .target
                .as_deref()
                .is_some_and(|target| comparable_path(target) == wanted)
    }
}

fn comparable_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// The file operations waiting for the next restart, read from
/// `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager`. Only reads.
///
/// Empty when nothing is waiting, and always off Windows. A value of the
/// wrong type is an error rather than "nothing pending", so a caller never
/// mistakes an unreadable list for an empty one.
pub fn pending_file_renames() -> io::Result<Vec<PendingFileRename>> {
    read_pending_file_renames(Hive::LocalMachine, SESSION_MANAGER_KEY)
}

fn read_pending_file_renames(hive: Hive, path: &str) -> io::Result<Vec<PendingFileRename>> {
    let mut renames = Vec::new();
    for name in PENDING_RENAME_VALUES {
        let Some(value) = imp::read_raw_value(hive, path, name)? else {
            continue;
        };
        if value.kind != REG_MULTI_SZ_KIND {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{name} isn't a list of strings (registry type {})",
                    value.kind
                ),
            ));
        }
        let (pairs, _) = value.data.as_chunks::<2>();
        let units: Vec<u16> = pairs.iter().copied().map(u16::from_le_bytes).collect();
        renames.extend(parse_pending_file_renames(&units));
    }
    Ok(renames)
}

/// Parse a `PendingFileRenameOperations` value, given as UTF-16 code units.
///
/// The value is a list of NUL-terminated strings in pairs: the source, then
/// the target, where an empty target means "delete". The list ends with one
/// more NUL. Paths are NT paths, `\??\C:\...`; the prefix is removed, and
/// `\??\UNC\server\share` becomes `\\server\share`. A target starting with
/// `!` means "replace an existing file"; the `!` is removed too.
///
/// Because an empty target is itself an empty string, the end of the list
/// is found where a *source* would be empty, never by looking for two NULs
/// in a row.
pub fn parse_pending_file_renames(units: &[u16]) -> Vec<PendingFileRename> {
    let mut strings = units.split(|unit| *unit == 0);
    let mut renames = Vec::new();
    while let Some(source) = strings.next() {
        if source.is_empty() {
            break;
        }
        // A list cut short after its last source still names that file.
        let target = strings.next().unwrap_or_default();
        let target = String::from_utf16_lossy(target);
        let target = target.strip_prefix('!').unwrap_or(&target);
        renames.push(PendingFileRename {
            source: windows_path_from_nt(&String::from_utf16_lossy(source)),
            target: (!target.is_empty()).then(|| windows_path_from_nt(target)),
        });
    }
    renames
}

fn windows_path_from_nt(path: &str) -> PathBuf {
    let Some(path) = path.strip_prefix("\\??\\") else {
        return PathBuf::from(path);
    };
    match path.strip_prefix("UNC\\") {
        Some(share) => PathBuf::from(format!("\\\\{share}")),
        None => PathBuf::from(path),
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
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE,
        KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_OPTION_NON_VOLATILE, RegCloseKey, RegCreateKeyExW,
        RegDeleteKeyW, RegDeleteValueW, RegEnumValueW, RegOpenKeyExW, RegQueryInfoKeyW,
        RegQueryValueExW, RegSetValueExW,
    };

    use super::{Hive, RegistryValue, RegistryView, UninstallEntry, same_value_name, wide_string};

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

    fn hive_key(hive: Hive) -> HKEY {
        match hive {
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
            Hive::CurrentUser => HKEY_CURRENT_USER,
        }
    }

    /// Open `path` for reading, or `None` when it doesn't exist.
    fn open_for_reading(hive: Hive, path: &str, view_flag: u32) -> io::Result<Option<OpenKey>> {
        let path = wide_string(path);
        let mut hkey: HKEY = ptr::null_mut();
        // SAFETY: `path` is NUL-terminated and outlives the call, and `hkey`
        // is a valid place for the handle.
        let status = unsafe {
            RegOpenKeyExW(
                hive_key(hive),
                path.as_ptr(),
                0,
                KEY_QUERY_VALUE | view_flag,
                &mut hkey,
            )
        };
        if is_missing(status) {
            return Ok(None);
        }
        check(status)?;
        Ok(Some(OpenKey(hkey)))
    }

    /// The string values `names` of `path` in one view, in the order asked,
    /// or `None` when the key isn't in that view.
    pub(super) fn read_view_values(
        hive: Hive,
        path: &str,
        view: RegistryView,
        names: &[&str],
    ) -> io::Result<Option<Vec<Option<String>>>> {
        let view_flag = match view {
            RegistryView::Bit64 => KEY_WOW64_64KEY,
            RegistryView::Bit32 => KEY_WOW64_32KEY,
        };
        let Some(key) = open_for_reading(hive, path, view_flag)? else {
            return Ok(None);
        };
        let mut values = Vec::with_capacity(names.len());
        for name in names {
            values.push(query_value(&key, name)?.and_then(|value| value.as_string()));
        }
        Ok(Some(values))
    }

    /// One value of `path`, whatever its type, or `None` when the key or
    /// the value is missing.
    pub(super) fn read_raw_value(
        hive: Hive,
        path: &str,
        name: &str,
    ) -> io::Result<Option<RegistryValue>> {
        match open_for_reading(hive, path, 0)? {
            Some(key) => query_value(&key, name),
            None => Ok(None),
        }
    }

    fn query_value(key: &OpenKey, name: &str) -> io::Result<Option<RegistryValue>> {
        let wide_name = wide_string(name);
        // The first call only asks for the size; a value that grows between
        // two calls makes Windows ask for more room again.
        let mut data: Vec<u8> = Vec::new();
        for _ in 0..=GROWN_VALUE_RETRIES {
            let mut kind = 0u32;
            let mut length = data.len() as u32;
            let buffer = if data.is_empty() {
                ptr::null_mut()
            } else {
                data.as_mut_ptr()
            };
            // SAFETY: `wide_name` is NUL-terminated, and `buffer` is either
            // null, which asks only for the size, or holds `length` bytes.
            let status = unsafe {
                RegQueryValueExW(
                    key.0,
                    wide_name.as_ptr(),
                    ptr::null(),
                    &mut kind,
                    buffer,
                    &mut length,
                )
            };
            match status {
                ERROR_SUCCESS if !buffer.is_null() || length == 0 => {
                    data.truncate(length as usize);
                    return Ok(Some(RegistryValue {
                        name: name.to_string(),
                        kind,
                        data,
                    }));
                }
                ERROR_SUCCESS | ERROR_MORE_DATA => data = vec![0; (length as usize).max(1)],
                status if is_missing(status) => return Ok(None),
                status => return Err(os_error(status)),
            }
        }
        Err(io::Error::other(format!(
            "the registry value {name:?} kept growing while RABBIT read it"
        )))
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

    use super::{Hive, RegistryValue, RegistryView, UninstallEntry};

    pub(super) fn read_view_values(
        _hive: Hive,
        _path: &str,
        _view: RegistryView,
        _names: &[&str],
    ) -> io::Result<Option<Vec<Option<String>>>> {
        Ok(None)
    }

    pub(super) fn read_raw_value(
        _hive: Hive,
        _path: &str,
        _name: &str,
    ) -> io::Result<Option<RegistryValue>> {
        Ok(None)
    }

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

    fn utf16_units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn pending_renames_pair_sources_with_targets_and_read_empty_targets_as_deletes() {
        let units = utf16_units(concat!(
            "\\??\\C:\\Program Files\\Common Files\\VST3\\Surge XT.vst3\\Contents\\x86_64-win\\Surge XT.vst3\0",
            "\0",
            "\\??\\C:\\Windows\\Temp\\new.dll\0",
            "!\\??\\C:\\Program Files\\Vendor\\old.dll\0",
            "\\??\\UNC\\server\\share\\left.txt\0",
            "\0",
            "\0",
        ));

        assert_eq!(
            parse_pending_file_renames(&units),
            [
                PendingFileRename {
                    source: PathBuf::from(
                        "C:\\Program Files\\Common Files\\VST3\\Surge XT.vst3\\Contents\\x86_64-win\\Surge XT.vst3"
                    ),
                    target: None,
                },
                PendingFileRename {
                    source: PathBuf::from("C:\\Windows\\Temp\\new.dll"),
                    target: Some(PathBuf::from("C:\\Program Files\\Vendor\\old.dll")),
                },
                PendingFileRename {
                    source: PathBuf::from("\\\\server\\share\\left.txt"),
                    target: None,
                },
            ]
        );
    }

    #[test]
    fn an_empty_or_cut_short_pending_list_still_parses() {
        assert!(parse_pending_file_renames(&[]).is_empty());
        assert!(parse_pending_file_renames(&[0]).is_empty());
        assert!(parse_pending_file_renames(&[0, 0]).is_empty());
        // Nothing after the last source: the file is still named.
        assert_eq!(
            parse_pending_file_renames(&utf16_units("\\??\\C:\\a.dll")),
            [PendingFileRename {
                source: PathBuf::from("C:\\a.dll"),
                target: None,
            }]
        );
        // A path without the NT prefix is kept as it is.
        assert_eq!(
            parse_pending_file_renames(&utf16_units("C:\\b.dll\0D:\\c.dll\0\0")),
            [PendingFileRename {
                source: PathBuf::from("C:\\b.dll"),
                target: Some(PathBuf::from("D:\\c.dll")),
            }]
        );
    }

    #[test]
    fn a_pending_rename_mentions_its_paths_whatever_their_case() {
        let rename = PendingFileRename {
            source: PathBuf::from("C:\\Temp\\new.dll"),
            target: Some(PathBuf::from("C:\\Program Files\\Vendor\\old.dll")),
        };
        assert!(rename.mentions(Path::new("c:\\temp\\NEW.dll")));
        assert!(rename.mentions(Path::new("C:/Program Files/Vendor/old.dll")));
        assert!(!rename.mentions(Path::new("C:\\Program Files\\Vendor")));

        let delete = PendingFileRename {
            source: PathBuf::from("C:\\Program Files\\Vendor"),
            target: None,
        };
        assert!(delete.mentions(Path::new("C:\\Program Files\\Vendor\\")));
    }

    #[test]
    fn view_values_answer_by_name_ignoring_case() {
        let found = ViewValues {
            view: RegistryView::Bit32,
            values: vec![
                (
                    "UninstallString".to_string(),
                    Some("\"C:\\x\\u.exe\"".to_string()),
                ),
                ("InstallLocation".to_string(), None),
            ],
        };
        assert_eq!(found.view(), RegistryView::Bit32);
        assert_eq!(found.value("uninstallstring"), Some("\"C:\\x\\u.exe\""));
        assert_eq!(found.value("InstallLocation"), None);
        assert_eq!(found.value("DisplayName"), None);
    }

    #[test]
    fn the_machine_list_is_hklm_and_a_test_list_never_is() {
        let machine = UninstallViews::local_machine();
        assert!(machine.is_local_machine());
        assert_eq!(
            machine.path(),
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall"
        );
        assert!(
            !UninstallViews::current_user_at(
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall"
            )
            .is_local_machine()
        );
        for name in ["", "REAPER\\Child", "\\REAPER"] {
            let error = machine.read_values(name, &["UninstallString"]).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name:?}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_no_view_holds_an_entry_and_nothing_is_pending() {
        let machine = UninstallViews::local_machine();
        assert!(!machine.key_exists("REAPER").unwrap());
        assert!(
            machine
                .read_values("REAPER", &["UninstallString"])
                .unwrap()
                .is_empty()
        );
        assert!(pending_file_renames().unwrap().is_empty());
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
        HKEY_CURRENT_USER, REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ, RegDeleteKeyW, RegDeleteTreeW,
    };

    use super::*;

    fn utf16_le(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

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
        assert_eq!(REG_MULTI_SZ_KIND, REG_MULTI_SZ);
    }

    #[test]
    fn every_view_holding_an_entry_reports_the_values_asked_for() {
        let key = ThrowawayKey::new();
        key.root()
            .write_entry(
                "REAPER",
                &UninstallEntry::new(vec![
                    RegistryValue::string(
                        "UninstallString",
                        "\"C:\\Program Files\\REAPER (x64)\\Uninstall.exe\"",
                    ),
                    RegistryValue::string("InstallLocation", "C:\\Program Files\\REAPER (x64)"),
                    RegistryValue {
                        name: "DisplayIcon".to_string(),
                        kind: REG_EXPAND_SZ,
                        data: utf16_le("%ProgramFiles%\\REAPER (x64)\\reaper.exe\0"),
                    },
                    RegistryValue {
                        name: "NoModify".to_string(),
                        kind: REG_DWORD,
                        data: 1u32.to_le_bytes().to_vec(),
                    },
                ]),
            )
            .unwrap();
        let views = UninstallViews::current_user_at(key.path.clone());

        let found = views
            .read_values(
                "REAPER",
                &[
                    "UninstallString",
                    "installlocation",
                    "DisplayIcon",
                    "NoModify",
                    "QuietUninstallString",
                ],
            )
            .unwrap();

        // HKCU\Software is one key in both views, so both find the entry;
        // under HKLM each view has its own.
        assert_eq!(
            found.iter().map(ViewValues::view).collect::<Vec<_>>(),
            RegistryView::ALL
        );
        for view in &found {
            assert_eq!(
                view.value("UninstallString"),
                Some("\"C:\\Program Files\\REAPER (x64)\\Uninstall.exe\"")
            );
            assert_eq!(
                view.value("InstallLocation"),
                Some("C:\\Program Files\\REAPER (x64)")
            );
            // Never expanded.
            assert_eq!(
                view.value("DisplayIcon"),
                Some("%ProgramFiles%\\REAPER (x64)\\reaper.exe")
            );
            assert_eq!(view.value("NoModify"), None, "not a string");
            assert_eq!(view.value("QuietUninstallString"), None, "missing");
        }
        assert!(views.key_exists("REAPER").unwrap());
        assert_eq!(views.views_with_key("REAPER").unwrap(), RegistryView::ALL);

        assert!(!views.key_exists("Surge XT").unwrap());
        assert!(
            views
                .read_values("Surge XT", &["UninstallString"])
                .unwrap()
                .is_empty()
        );
        // A root that doesn't exist holds no entries either.
        let missing = UninstallViews::current_user_at(format!("{}\\Missing", key.path));
        assert!(!missing.key_exists("REAPER").unwrap());
    }

    /// Reads the real HKLM list, and only reads it.
    #[test]
    fn the_machine_list_opens_in_both_views() {
        let machine = UninstallViews::local_machine();
        assert!(
            !machine
                .key_exists("RABBIT-test-no-program-is-called-this")
                .unwrap()
        );
    }

    #[test]
    fn pending_renames_come_from_both_multi_string_values() {
        let key = ThrowawayKey::new();
        key.root()
            .write_entry(
                "Session Manager",
                &UninstallEntry::new(vec![
                    RegistryValue {
                        name: "PendingFileRenameOperations".to_string(),
                        kind: REG_MULTI_SZ,
                        data: utf16_le(
                            "\\??\\C:\\Vendor\\in use.dll\0\0\\??\\C:\\Temp\\new.dll\0!\\??\\C:\\Vendor\\old.dll\0\0",
                        ),
                    },
                    RegistryValue {
                        name: "PendingFileRenameOperations2".to_string(),
                        kind: REG_MULTI_SZ,
                        data: utf16_le("\\??\\D:\\Other\\file.txt\0\0\0"),
                    },
                    RegistryValue::string("Unrelated", "left alone"),
                ]),
            )
            .unwrap();
        let session_manager = format!("{}\\Session Manager", key.path);

        let renames = read_pending_file_renames(Hive::CurrentUser, &session_manager).unwrap();

        assert_eq!(
            renames,
            [
                PendingFileRename {
                    source: PathBuf::from("C:\\Vendor\\in use.dll"),
                    target: None,
                },
                PendingFileRename {
                    source: PathBuf::from("C:\\Temp\\new.dll"),
                    target: Some(PathBuf::from("C:\\Vendor\\old.dll")),
                },
                PendingFileRename {
                    source: PathBuf::from("D:\\Other\\file.txt"),
                    target: None,
                },
            ]
        );

        // A list that isn't a list is an error, never "nothing pending".
        key.root()
            .write_entry(
                "Session Manager",
                &UninstallEntry::new(vec![RegistryValue::string(
                    "PendingFileRenameOperations",
                    "\\??\\C:\\a.dll",
                )]),
            )
            .unwrap();
        let error = read_pending_file_renames(Hive::CurrentUser, &session_manager).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        // No key, or no values, means nothing is pending.
        key.root()
            .write_entry("Session Manager", &UninstallEntry::default())
            .unwrap();
        assert!(
            read_pending_file_renames(Hive::CurrentUser, &session_manager)
                .unwrap()
                .is_empty()
        );
        assert!(
            read_pending_file_renames(Hive::CurrentUser, &format!("{}\\Missing", key.path))
                .unwrap()
                .is_empty()
        );
    }

    /// Reads the real list in HKLM, and only reads it.
    #[test]
    fn the_real_pending_list_reads() {
        pending_file_renames().unwrap();
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
