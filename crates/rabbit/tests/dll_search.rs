//! rabbit.exe must take the DLLs it imports from System32 only, never from
//! the folder it sits in: its elevated uninstall helper is rabbit.exe too,
//! and a portable RABBIT often sits in a folder the user's own programs can
//! write to. build.rs links it with `/DEPENDENTLOADFLAG:0x800`; this reads
//! the flag back out of the built program.
#![cfg(all(windows, target_env = "msvc"))]

/// `LOAD_LIBRARY_SEARCH_SYSTEM32`.
const SYSTEM32_ONLY: u16 = 0x800;

#[test]
fn rabbit_exe_takes_its_dlls_from_system32_only() {
    let path = env!("CARGO_BIN_EXE_rabbit");
    let image = std::fs::read(path).unwrap();
    assert_eq!(
        dependent_load_flags(&image),
        Some(SYSTEM32_ONLY),
        "{path} must be linked with /DEPENDENTLOADFLAG:0x800"
    );
}

/// The `DependentLoadFlags` of a PE image's load configuration, or `None`
/// when the image has no load configuration that holds them.
fn dependent_load_flags(image: &[u8]) -> Option<u16> {
    let u16_at = |offset: usize| -> Option<u16> {
        Some(u16::from_le_bytes(
            image.get(offset..offset + 2)?.try_into().ok()?,
        ))
    };
    let u32_at = |offset: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            image.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    let pe = u32_at(0x3c)? as usize;
    if image.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    let coff = pe + 4;
    let sections = u16_at(coff + 2)? as usize;
    let optional = coff + 20;
    let optional_size = u16_at(coff + 16)? as usize;
    // Where the data directories start, and where DependentLoadFlags sits
    // in IMAGE_LOAD_CONFIG_DIRECTORY, for PE32+ and PE32.
    let (directories, flags_offset) = match u16_at(optional)? {
        0x20b => (optional + 112, 78),
        0x10b => (optional + 96, 54),
        _ => return None,
    };
    // Entry 10 is the load configuration.
    let config_rva = u32_at(directories + 10 * 8)?;
    if config_rva == 0 {
        return None;
    }
    let section_table = optional + optional_size;
    let config = (0..sections).find_map(|index| {
        let section = section_table + index * 40;
        let virtual_size = u32_at(section + 8)?;
        let virtual_address = u32_at(section + 12)?;
        let raw_size = u32_at(section + 16)?;
        let raw_pointer = u32_at(section + 20)?;
        let end = virtual_address.checked_add(virtual_size.max(raw_size))?;
        (virtual_address..end)
            .contains(&config_rva)
            .then(|| (config_rva - virtual_address + raw_pointer) as usize)
    })?;
    // The structure grew over the years; an older one ends before the flags.
    if (u32_at(config)? as usize) < flags_offset + 2 {
        return None;
    }
    u16_at(config + flags_offset)
}
