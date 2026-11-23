//! Reads CPU architecture out of an executable's file header.
//!
//! REAPER installs are decoupled from the host RABBIT runs on: an Apple Silicon
//! Mac can host an Intel REAPER under Rosetta, and Windows-on-ARM can host an
//! x86_64 REAPER. Stamping installs with `Architecture::current()` (the running
//! RABBIT's compile-time arch) silently mismatches the per-arch artifact picker
//! when those configurations come up. This module probes the actual binary so
//! detection can record what the targeted REAPER really is.
//!
//! The header parsing itself is delegated to the `object` crate (the same one
//! the Rust toolchain uses for `addr2line`, `cargo-binutils`, etc.) — we just
//! map its `Architecture` enum into ours and detect Mach-O fat / universal
//! binaries up front. Probe failures all collapse to `Architecture::Unknown`,
//! which the artifact resolver already treats as "fall back to the platform
//! default" so a corrupt or unreadable file can't regress existing behavior.
//!
//! Windows on ARM adds hybrid files, whose header names another machine than
//! the code they run as. A linked ARM64EC file, such as REAPER for Windows on
//! ARM, names x64 (`IMAGE_FILE_MACHINE_AMD64`); an ARM64X file names ARM64.
//! Both have hybrid (CHPE) metadata, which their load configuration points
//! at and plain x64 and ARM64 files don't have. [`classify_pe`] reads that
//! pointer, so the probe tells an ARM64EC REAPER from an x64 one.

use std::fs;
use std::mem::offset_of;
use std::ops::Range;
use std::path::{Path, PathBuf};

use object::pe::{
    self, ImageDosHeader, ImageLoadConfigDirectory32, ImageLoadConfigDirectory64, ImageNtHeaders32,
    ImageNtHeaders64,
};
use object::read::pe::ImageNtHeaders;
use object::{FileKind, LittleEndian as LE};

use crate::model::Architecture;

/// Probes the executable at `install_path`. On macOS the detection layer hands
/// us a `.app` bundle; we drill into `Contents/MacOS/` first. On Windows /
/// for plain files we use the path as given.
pub fn probe_executable_architecture(install_path: &Path) -> Architecture {
    let Some(executable) = resolve_executable_path(install_path) else {
        return Architecture::Unknown;
    };
    let Ok(data) = fs::read(&executable) else {
        return Architecture::Unknown;
    };
    architecture_from_bytes(&data)
}

fn architecture_from_bytes(data: &[u8]) -> Architecture {
    let architecture = header_architecture(data);
    // A linked ARM64EC program names x64 in its header but runs as ARM64EC.
    // REAPER for Windows on ARM is one, and its updates and plug-ins are
    // the builds for an ARM64EC REAPER. Only a file whose header already
    // reads as x64 or ARM64 can become ARM64EC here, so a malformed one
    // keeps the answer its header gives.
    //
    // An ARM64X program names ARM64 and stays `Arm64`: Windows starts it in
    // its native ARM64 view, as an ARM64 process, which loads ARM64 and
    // ARM64X modules but no x64 or ARM64EC ones. Such a process could load
    // none of the builds the manifests offer for `Arm64` on Windows, because
    // there `Arm64` stands for the ARM64EC REAPER that a new install from an
    // ARM RABBIT gets. RABBIT reads it as `Arm64` anyway, because REAPER
    // ships ARM64EC and no ARM64 or ARM64X build of it exists to choose. If
    // one ever appears, the package rules must give it builds of its own.
    match (architecture, classify_pe(data)) {
        (Architecture::X64 | Architecture::Arm64, PeKind::Arm64Ec) => Architecture::Arm64Ec,
        _ => architecture,
    }
}

/// The architecture a file's header names, as `object` reads it.
fn header_architecture(data: &[u8]) -> Architecture {
    // Mach-O fat / universal binaries have to be checked before the generic
    // `object::File::parse` path: the artifact picker shortcuts them to
    // `Universal` (matching REAPER's `_universal.dmg` naming) and bypasses
    // the per-slice arch question entirely.
    match FileKind::parse(data) {
        Ok(FileKind::MachOFat32 | FileKind::MachOFat64) => return Architecture::Universal,
        Ok(_) => {}
        Err(_) => return Architecture::Unknown,
    }

    let Ok(file) = object::File::parse(data) else {
        return Architecture::Unknown;
    };
    match object::Object::architecture(&file) {
        object::Architecture::I386 => Architecture::X86,
        // Also a linked ARM64EC file, which `architecture_from_bytes` tells
        // apart by its hybrid metadata.
        object::Architecture::X86_64 => Architecture::X64,
        // `object` maps the COFF Machine field only: `IMAGE_FILE_MACHINE_ARM64`
        // (also what ARM64X images carry) and `IMAGE_FILE_MACHINE_ARM64EC`
        // (used by .obj files) both become `Aarch64`.
        object::Architecture::Aarch64 => Architecture::Arm64,
        _ => Architecture::Unknown,
    }
}

/// What a Windows program or library is built for, as the Windows loader
/// tells it apart. [`classify_pe`] reads it from the file's headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeKind {
    /// 32-bit x86.
    X86,
    /// x64 (AMD64).
    X64,
    /// ARM64EC: ARM64 code that follows x64's conventions, so it mixes with
    /// x64 code in one process. REAPER for Windows on ARM is built so.
    Arm64Ec,
    /// ARM64X: one file with an ARM64 and an ARM64EC view, so both kinds of
    /// process can load it.
    Arm64X,
    /// Plain ARM64.
    Arm64,
    /// Another machine type, or not a PE file at all.
    Other,
}

impl PeKind {
    /// Whether an ARM64EC process, such as REAPER on Windows on ARM, can
    /// load a library of this kind. It runs x64 code under emulation,
    /// ARM64EC code natively, and an ARM64X library through its ARM64EC
    /// view. It can't load plain ARM64 or x86 code.
    pub fn loads_in_arm64ec(self) -> bool {
        matches!(self, Self::X64 | Self::Arm64Ec | Self::Arm64X)
    }

    /// The short name reports use, such as `arm64ec`.
    pub fn name(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X64 => "x64",
            Self::Arm64Ec => "arm64ec",
            Self::Arm64X => "arm64x",
            Self::Arm64 => "arm64",
            Self::Other => "other",
        }
    }
}

/// The kind of the PE file at `path`; [`PeKind::Other`] when it can't be
/// read.
pub fn classify_pe_file(path: &Path) -> PeKind {
    fs::read(path).map_or(PeKind::Other, |data| classify_pe(&data))
}

/// The kind of the PE file in `data`, read from its headers.
///
/// A linked ARM64EC file names x64 in its file header, and an ARM64X file
/// names ARM64. Each is told apart from a plain one by the hybrid (CHPE)
/// metadata pointer in its load configuration. Every read is checked
/// against the file's size: a file whose load configuration is missing,
/// cut short or outside its sections reads as the plain kind its header
/// names, and one whose headers can't be read, or that isn't a PE file at
/// all, as [`PeKind::Other`].
pub fn classify_pe(data: &[u8]) -> PeKind {
    let headers = match FileKind::parse(data) {
        Ok(FileKind::Pe32) => machine_and_hybrid::<ImageNtHeaders32>(data, CHPE_POINTER_32),
        Ok(FileKind::Pe64) => machine_and_hybrid::<ImageNtHeaders64>(data, CHPE_POINTER_64),
        _ => None,
    };
    let Some((machine, hybrid)) = headers else {
        return PeKind::Other;
    };
    match machine {
        // With hybrid metadata, an x86 file is a CHPE x86 one, which still
        // loads only into an x86 process.
        pe::IMAGE_FILE_MACHINE_I386 => PeKind::X86,
        pe::IMAGE_FILE_MACHINE_AMD64 if hybrid => PeKind::Arm64Ec,
        pe::IMAGE_FILE_MACHINE_AMD64 => PeKind::X64,
        pe::IMAGE_FILE_MACHINE_ARM64 if hybrid => PeKind::Arm64X,
        pe::IMAGE_FILE_MACHINE_ARM64 => PeKind::Arm64,
        // The machine types object files use for ARM64EC and ARM64X. A
        // linked file has one of the two above instead.
        pe::IMAGE_FILE_MACHINE_ARM64EC => PeKind::Arm64Ec,
        pe::IMAGE_FILE_MACHINE_ARM64X => PeKind::Arm64X,
        _ => PeKind::Other,
    }
}

/// Where the hybrid metadata pointer sits in a 32-bit load configuration.
const CHPE_POINTER_32: Range<usize> = {
    let start = offset_of!(ImageLoadConfigDirectory32, chpe_metadata_pointer);
    start..start + 4
};

/// Where the hybrid metadata pointer sits in a 64-bit load configuration.
const CHPE_POINTER_64: Range<usize> = {
    let start = offset_of!(ImageLoadConfigDirectory64, chpe_metadata_pointer);
    start..start + 8
};

/// The machine a PE file's header names, and whether its load configuration
/// points at hybrid metadata. `None` when the headers can't be read.
/// `chpe_pointer` is where that pointer sits in a load configuration of the
/// file's bitness.
fn machine_and_hybrid<Pe: ImageNtHeaders>(
    data: &[u8],
    chpe_pointer: Range<usize>,
) -> Option<(pe::Machine, bool)> {
    let dos_header = ImageDosHeader::parse(data).ok()?;
    let mut offset = u64::from(dos_header.nt_headers_offset());
    let (nt_headers, data_directories) = Pe::parse(data, &mut offset).ok()?;
    let machine = nt_headers.file_header().machine.get(LE);
    // The load configuration is found through the section that holds it.
    // With no such entry, an empty one, or a section table or section that
    // can't be read, the file has no hybrid metadata RABBIT can see.
    let hybrid = data_directories
        .get(pe::IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG)
        .filter(|directory| directory.size.get(LE) != 0)
        .and_then(|directory| {
            let sections = nt_headers.sections(data, offset).ok()?;
            sections.pe_data_at(data, directory.virtual_address.get(LE))
        })
        .is_some_and(|load_config| points_at_hybrid_metadata(load_config, chpe_pointer));
    Some((machine, hybrid))
}

/// Whether `load_config`, a load configuration and what follows it in its
/// section, points at hybrid metadata. Its own `Size` field says how many
/// of its fields the file has, so the pointer counts only when it is among
/// them, the file holds it, and it isn't zero.
fn points_at_hybrid_metadata(load_config: &[u8], chpe_pointer: Range<usize>) -> bool {
    let Some(size) = load_config
        .get(..4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
    else {
        return false;
    };
    usize::try_from(size).is_ok_and(|size| size >= chpe_pointer.end)
        && load_config
            .get(chpe_pointer)
            .is_some_and(|pointer| pointer.iter().any(|&byte| byte != 0))
}

fn resolve_executable_path(install_path: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(install_path).ok()?;
    if metadata.is_file() {
        return Some(install_path.to_path_buf());
    }

    // macOS .app bundle: read the executable from Contents/MacOS/. We don't
    // parse Info.plist for CFBundleExecutable — the directory always
    // contains exactly one file in practice (REAPER, Rabbit, etc.), and a
    // name-stem match against the bundle takes priority for robustness.
    let macos_dir = install_path.join("Contents").join("MacOS");
    let stem = install_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned);

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&macos_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_file = entry
                .file_type()
                .map(|file_type| file_type.is_file())
                .unwrap_or(false);
            if is_file {
                candidates.push(path);
            }
        }
    }

    if let Some(bundle_name) = stem.as_deref()
        && let Some(matching) = candidates
            .iter()
            .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(bundle_name))
    {
        return Some(matching.clone());
    }
    candidates.into_iter().next()
}

/// Minimal PE files of each kind, for the tests of this crate and of the
/// crates that use it, whose dev-dependencies turn on the `test-support`
/// feature. Never used outside tests.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod test_support {
    use super::PeKind;

    /// Where the one section starts in memory.
    pub const SECTION_RVA: u32 = 0x1000;
    /// Where the one section starts in the file.
    pub const SECTION_FILE_OFFSET: usize = 0x200;
    /// The one section's size, in memory and in the file. The file ends
    /// with it.
    pub const SECTION_SIZE: usize = 0x200;

    /// A hybrid metadata pointer: any address that isn't zero.
    const HYBRID_METADATA: u64 = 0x1_8000_1100;

    /// A minimal PE file of `kind`, whose load configuration points at
    /// hybrid metadata for an ARM64EC or ARM64X file and not for any other.
    /// An x86 file, and one of another machine, is PE32; the others PE32+.
    pub fn pe_image(kind: PeKind) -> Vec<u8> {
        let (machine, hybrid) = match kind {
            PeKind::X86 => (0x014c, false),
            PeKind::X64 => (0x8664, false),
            PeKind::Arm64Ec => (0x8664, true),
            PeKind::Arm64X => (0xaa64, true),
            PeKind::Arm64 => (0xaa64, false),
            // 32-bit ARM (ARMNT), which RABBIT has no kind for.
            PeKind::Other => (0x01c4, false),
        };
        let image = if matches!(kind, PeKind::X86 | PeKind::Other) {
            PeImage::pe32(machine)
        } else {
            PeImage::pe32_plus(machine)
        };
        PeImage {
            chpe_pointer: if hybrid { HYBRID_METADATA } else { 0 },
            ..image
        }
        .bytes()
    }

    /// The parts of a minimal PE file that tests vary. [`PeImage::bytes`]
    /// lays it out as the headers, one section header, and one section,
    /// which holds the load configuration.
    #[derive(Debug, Clone)]
    pub struct PeImage {
        /// The machine the file header names.
        pub machine: u16,
        /// PE32+ (64-bit) rather than PE32.
        pub pe32_plus: bool,
        /// How many data directories the optional header has. The load
        /// configuration's is the 11th.
        pub data_directories: u32,
        /// Where the load configuration's data directory points.
        pub load_config_rva: u32,
        /// The size that data directory gives.
        pub load_config_directory_size: u32,
        /// The load configuration's own `Size` field.
        pub load_config_size: u32,
        /// The hybrid metadata pointer; zero for none.
        pub chpe_pointer: u64,
    }

    impl PeImage {
        /// A PE32 file for `machine`, with a whole 32-bit load configuration
        /// at the start of its section and no hybrid metadata.
        pub fn pe32(machine: u16) -> Self {
            Self {
                machine,
                pe32_plus: false,
                data_directories: 16,
                load_config_rva: SECTION_RVA,
                load_config_directory_size: 0xc0,
                load_config_size: 0xc0,
                chpe_pointer: 0,
            }
        }

        /// A PE32+ file for `machine`, with a whole 64-bit load
        /// configuration at the start of its section and no hybrid metadata.
        pub fn pe32_plus(machine: u16) -> Self {
            Self {
                pe32_plus: true,
                load_config_directory_size: 0x140,
                load_config_size: 0x140,
                ..Self::pe32(machine)
            }
        }

        /// Where the hybrid metadata pointer sits in this file's load
        /// configuration. The offsets are the PE format's own, written out
        /// here rather than taken from the code under test, so a wrong
        /// offset there fails the tests.
        pub fn chpe_field(&self) -> std::ops::Range<usize> {
            if self.pe32_plus { 200..208 } else { 124..128 }
        }

        /// The size of the fixed part of this file's optional header, before
        /// its data directories.
        pub fn optional_header_fixed_size(&self) -> usize {
            if self.pe32_plus { 112 } else { 96 }
        }

        /// The file's bytes. Only the part of the load configuration that
        /// falls inside the section is written.
        pub fn bytes(&self) -> Vec<u8> {
            let fixed = self.optional_header_fixed_size();
            let optional_size = fixed + 8 * self.data_directories as usize;
            let mut bytes = vec![0u8; 64];
            bytes[0..2].copy_from_slice(b"MZ");
            bytes[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes()); // e_lfanew
            bytes.extend_from_slice(b"PE\0\0");
            bytes.extend_from_slice(&self.machine.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes()); // NumberOfSections
            bytes.extend_from_slice(&[0u8; 12]); // timestamp and symbol table
            bytes.extend_from_slice(&(optional_size as u16).to_le_bytes());
            bytes.extend_from_slice(&0x0022u16.to_le_bytes()); // executable, large-address aware

            let mut optional = vec![0u8; optional_size];
            let magic: u16 = if self.pe32_plus { 0x020b } else { 0x010b };
            optional[0..2].copy_from_slice(&magic.to_le_bytes());
            optional[32..36].copy_from_slice(&SECTION_RVA.to_le_bytes()); // SectionAlignment
            optional[36..40].copy_from_slice(&(SECTION_FILE_OFFSET as u32).to_le_bytes()); // FileAlignment
            optional[56..60].copy_from_slice(&0x2000u32.to_le_bytes()); // SizeOfImage
            optional[60..64].copy_from_slice(&(SECTION_FILE_OFFSET as u32).to_le_bytes()); // SizeOfHeaders
            optional[fixed - 4..fixed].copy_from_slice(&self.data_directories.to_le_bytes());
            if self.data_directories > 10 {
                let entry = fixed + 8 * 10; // IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG
                optional[entry..entry + 4].copy_from_slice(&self.load_config_rva.to_le_bytes());
                optional[entry + 4..entry + 8]
                    .copy_from_slice(&self.load_config_directory_size.to_le_bytes());
            }
            bytes.extend_from_slice(&optional);

            let mut section_header = [0u8; 40];
            section_header[0..6].copy_from_slice(b".rdata");
            section_header[8..12].copy_from_slice(&(SECTION_SIZE as u32).to_le_bytes()); // VirtualSize
            section_header[12..16].copy_from_slice(&SECTION_RVA.to_le_bytes()); // VirtualAddress
            section_header[16..20].copy_from_slice(&(SECTION_SIZE as u32).to_le_bytes()); // SizeOfRawData
            section_header[20..24].copy_from_slice(&(SECTION_FILE_OFFSET as u32).to_le_bytes()); // PointerToRawData
            section_header[36..40].copy_from_slice(&0x4000_0040u32.to_le_bytes()); // initialized, readable
            bytes.extend_from_slice(&section_header);
            bytes.resize(SECTION_FILE_OFFSET, 0);

            let mut section = vec![0u8; SECTION_SIZE];
            if let Some(start) = self.load_config_rva.checked_sub(SECTION_RVA) {
                let start = start as usize;
                put(&mut section, start, &self.load_config_size.to_le_bytes());
                let pointer = if self.pe32_plus {
                    self.chpe_pointer.to_le_bytes().to_vec()
                } else {
                    (self.chpe_pointer as u32).to_le_bytes().to_vec()
                };
                put(&mut section, start + self.chpe_field().start, &pointer);
            }
            bytes.extend_from_slice(&section);
            bytes
        }
    }

    /// Write `bytes` into `buffer` at `at`, leaving out what falls past its
    /// end.
    fn put(buffer: &mut [u8], at: usize, bytes: &[u8]) {
        for (index, byte) in bytes.iter().enumerate() {
            if let Some(slot) = at.checked_add(index).and_then(|at| buffer.get_mut(at)) {
                *slot = *byte;
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::test_support::{PeImage, SECTION_FILE_OFFSET, SECTION_RVA, SECTION_SIZE, pe_image};
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    /// Every kind [`classify_pe`] tells apart.
    const EVERY_KIND: [PeKind; 6] = [
        PeKind::X86,
        PeKind::X64,
        PeKind::Arm64Ec,
        PeKind::Arm64X,
        PeKind::Arm64,
        PeKind::Other,
    ];

    /// Where the optional header starts in the files [`PeImage`] writes:
    /// after the 64-byte DOS header, the signature and the file header.
    const OPTIONAL_HEADER: usize = 64 + 4 + 20;

    /// The kind a file's header alone names, for the hybrid kinds' plain
    /// counterparts.
    fn plain(kind: PeKind) -> PeKind {
        match kind {
            PeKind::Arm64Ec => PeKind::X64,
            PeKind::Arm64X => PeKind::Arm64,
            other => other,
        }
    }

    #[test]
    fn classifies_each_kind_of_pe_file() {
        for kind in EVERY_KIND {
            assert_eq!(classify_pe(&pe_image(kind)), kind, "{kind:?}");
        }
    }

    /// An ARM64EC process loads x64, ARM64EC and ARM64X libraries, and no
    /// plain ARM64 or x86 ones.
    #[test]
    fn only_x64_arm64ec_and_arm64x_load_in_an_arm64ec_process() {
        let loadable: Vec<PeKind> = EVERY_KIND
            .into_iter()
            .filter(|kind| kind.loads_in_arm64ec())
            .collect();
        assert_eq!(loadable, [PeKind::X64, PeKind::Arm64Ec, PeKind::Arm64X]);
        let names: Vec<&str> = EVERY_KIND.into_iter().map(PeKind::name).collect();
        assert_eq!(names, ["x86", "x64", "arm64ec", "arm64x", "arm64", "other"]);
    }

    /// REAPER for Windows on ARM names x64 in its header and probes as
    /// ARM64EC. An ARM64X program starts as an ARM64 process and probes as
    /// ARM64; the plain kinds probe as their header says.
    #[test]
    fn probes_hybrid_programs_by_the_process_they_start_as() {
        let dir = tempdir().unwrap();
        for (kind, expected) in [
            (PeKind::Arm64Ec, Architecture::Arm64Ec),
            (PeKind::Arm64X, Architecture::Arm64),
            (PeKind::X64, Architecture::X64),
            (PeKind::Arm64, Architecture::Arm64),
            (PeKind::X86, Architecture::X86),
            (PeKind::Other, Architecture::Unknown),
        ] {
            let path = dir.path().join(format!("reaper-{}.exe", kind.name()));
            std::fs::write(&path, pe_image(kind)).unwrap();
            assert_eq!(probe_executable_architecture(&path), expected, "{kind:?}");
            assert_eq!(classify_pe_file(&path), kind, "{kind:?}");
        }
        assert_eq!(
            classify_pe_file(&dir.path().join("missing.dll")),
            PeKind::Other
        );
    }

    /// The hybrid metadata pointer is read where the PE format puts it:
    /// `CHPEMetadataPointer` in `IMAGE_LOAD_CONFIG_DIRECTORY32` and `64`.
    #[test]
    fn the_hybrid_pointer_sits_where_the_pe_format_puts_it() {
        assert_eq!(CHPE_POINTER_32, 124..128);
        assert_eq!(CHPE_POINTER_64, 200..208);
    }

    /// A CHPE x86 file, x86 code with hybrid metadata from Windows 10 on
    /// ARM, still loads only into an x86 process.
    #[test]
    fn an_x86_file_with_hybrid_metadata_is_x86() {
        let chpe_x86 = PeImage {
            chpe_pointer: 0x1000_1100,
            ..PeImage::pe32(0x014c)
        };
        assert_eq!(classify_pe(&chpe_x86.bytes()), PeKind::X86);
        assert_eq!(
            architecture_from_bytes(&chpe_x86.bytes()),
            Architecture::X86
        );
    }

    /// The pointer counts only when the load configuration's own size
    /// covers it, in a PE32+ file and in a PE32 one, where it sits at
    /// another offset.
    #[test]
    fn the_hybrid_pointer_counts_only_within_the_load_configurations_size() {
        for image in [PeImage::pe32_plus(0x8664), PeImage::pe32(0x8664)] {
            let end = image.chpe_field().end as u32;
            let hybrid = PeImage {
                chpe_pointer: 0x1100,
                ..image
            };
            let just_covered = PeImage {
                load_config_size: end,
                ..hybrid.clone()
            };
            assert_eq!(classify_pe(&just_covered.bytes()), PeKind::Arm64Ec);
            let one_short = PeImage {
                load_config_size: end - 1,
                ..hybrid.clone()
            };
            assert_eq!(classify_pe(&one_short.bytes()), PeKind::X64);
            let empty = PeImage {
                load_config_size: 0,
                ..hybrid
            };
            assert_eq!(classify_pe(&empty.bytes()), PeKind::X64);
        }
    }

    /// Without a load configuration RABBIT can read, a hybrid file reads as
    /// the plain kind its header names, and the probe gives the answer the
    /// header gives.
    #[test]
    fn a_file_without_a_readable_load_configuration_reads_as_its_header_says() {
        for kind in [PeKind::Arm64Ec, PeKind::Arm64X] {
            let machine = if kind == PeKind::Arm64Ec {
                0x8664
            } else {
                0xaa64
            };
            let hybrid = PeImage {
                chpe_pointer: 0x1_8000_1100,
                ..PeImage::pe32_plus(machine)
            };
            let section_end = SECTION_RVA + SECTION_SIZE as u32;
            let variants = [
                // No data directory for the load configuration.
                PeImage {
                    data_directories: 10,
                    ..hybrid.clone()
                },
                // An empty data directory.
                PeImage {
                    load_config_directory_size: 0,
                    ..hybrid.clone()
                },
                // One that points nowhere.
                PeImage {
                    load_config_rva: 0,
                    ..hybrid.clone()
                },
                // One outside every section.
                PeImage {
                    load_config_rva: 0x5000,
                    ..hybrid.clone()
                },
                // A load configuration the section's end cuts off before
                // the pointer.
                PeImage {
                    load_config_rva: section_end - 100,
                    ..hybrid.clone()
                },
            ];
            for variant in variants {
                let bytes = variant.bytes();
                assert_eq!(classify_pe(&bytes), plain(kind), "{variant:?}");
                assert_eq!(
                    architecture_from_bytes(&bytes),
                    header_architecture(&bytes),
                    "{variant:?}"
                );
            }
            // A section whose data the file doesn't hold.
            let cut = &hybrid.bytes()[..SECTION_FILE_OFFSET + 100];
            assert_eq!(classify_pe(cut), plain(kind));
            assert_eq!(architecture_from_bytes(cut), header_architecture(cut));
        }
    }

    /// Cut short anywhere, a file of any kind never panics, never reads as
    /// hybrid, and probes as its header alone says, as before RABBIT read
    /// hybrid metadata.
    #[test]
    fn a_truncated_file_falls_back_to_what_its_header_says() {
        for kind in EVERY_KIND {
            let bytes = pe_image(kind);
            for length in 0..bytes.len() {
                let cut = &bytes[..length];
                let read = classify_pe(cut);
                assert!(
                    read == plain(kind) || read == PeKind::Other,
                    "{kind:?} cut to {length}: {read:?}"
                );
                assert_eq!(
                    architecture_from_bytes(cut),
                    header_architecture(cut),
                    "{kind:?} cut to {length}"
                );
            }
        }
    }

    /// Headers that lie about their sizes or offsets are read as no PE file
    /// at all, or as the machine they name when only the sections are
    /// broken, and the probe gives the answer it gave before.
    #[test]
    fn malformed_headers_never_read_as_hybrid() {
        let image = pe_image(PeKind::Arm64Ec);
        let patched = |at: usize, value: &[u8]| {
            let mut bytes = image.clone();
            bytes[at..at + value.len()].copy_from_slice(value);
            bytes
        };
        let fixed = PeImage::pe32_plus(0x8664).optional_header_fixed_size();
        for (what, bytes, expected) in [
            (
                "NT headers past the end",
                patched(0x3c, &0x10_000u32.to_le_bytes()),
                PeKind::Other,
            ),
            (
                "NT headers at the largest offset",
                patched(0x3c, &u32::MAX.to_le_bytes()),
                PeKind::Other,
            ),
            ("no PE signature", patched(64, b"NE\0\0"), PeKind::Other),
            (
                "an optional header smaller than its fixed part",
                patched(64 + 4 + 16, &4u16.to_le_bytes()),
                PeKind::Other,
            ),
            (
                "more data directories than the optional header holds",
                patched(OPTIONAL_HEADER + fixed - 4, &u32::MAX.to_le_bytes()),
                PeKind::Other,
            ),
            (
                "an unknown optional header magic",
                patched(OPTIONAL_HEADER, &0x0107u16.to_le_bytes()),
                PeKind::Other,
            ),
            (
                "more sections than the file holds",
                patched(64 + 4 + 2, &u16::MAX.to_le_bytes()),
                PeKind::X64,
            ),
            (
                "a section whose data lies past the end",
                patched(
                    OPTIONAL_HEADER + fixed + 8 * 16 + 20,
                    &0x7fff_0000u32.to_le_bytes(),
                ),
                PeKind::X64,
            ),
        ] {
            assert_eq!(classify_pe(&bytes), expected, "{what}");
            assert_eq!(
                architecture_from_bytes(&bytes),
                header_architecture(&bytes),
                "{what}"
            );
        }
    }

    #[test]
    fn files_that_arent_pe_read_as_other() {
        let mut mach_o = Vec::new();
        mach_o.extend_from_slice(&0xfeed_facfu32.to_le_bytes());
        mach_o.extend_from_slice(&MACHO_CPU_TYPE_ARM64.to_le_bytes());
        mach_o.extend_from_slice(&[0u8; 24]);
        for bytes in [
            &b""[..],
            &b"MZ"[..],
            &b"not an executable header"[..],
            &mach_o[..],
        ] {
            assert_eq!(classify_pe(bytes), PeKind::Other, "{bytes:?}");
        }
    }

    /// A minimal PE32+ image: a 64-byte DOS header pointing at NT headers
    /// with a 112-byte optional header, no data directories and no sections.
    pub(crate) fn write_pe64(path: &Path, machine: u16) {
        let mut bytes = vec![0u8; 64];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes()); // e_lfanew
        bytes.extend_from_slice(b"PE\0\0");
        bytes.extend_from_slice(&machine.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes()); // NumberOfSections
        bytes.extend_from_slice(&[0u8; 12]); // timestamp and symbol table
        bytes.extend_from_slice(&112u16.to_le_bytes()); // SizeOfOptionalHeader
        bytes.extend_from_slice(&0x0022u16.to_le_bytes()); // executable, large-address aware
        let mut optional = [0u8; 112];
        optional[0..2].copy_from_slice(&0x020bu16.to_le_bytes()); // PE32+
        bytes.extend_from_slice(&optional);
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn probes_pe_machine_types() {
        let dir = tempdir().unwrap();
        // Without a load configuration, the header's machine decides.
        for (machine, expected) in [
            (0x8664, Architecture::X64),     // AMD64
            (0xaa64, Architecture::Arm64),   // ARM64
            (0xa641, Architecture::Arm64Ec), // ARM64EC, as object files name it
            (0x014c, Architecture::X86),
        ] {
            let path = dir.path().join(format!("reaper-{machine:04x}.exe"));
            write_pe64(&path, machine);
            assert_eq!(
                probe_executable_architecture(&path),
                expected,
                "machine {machine:#06x}"
            );
        }
    }

    #[test]
    fn probes_mach_o_fat_as_universal() {
        // `FileKind::parse` reads 16 bytes minimum to disambiguate the magic;
        // FAT_MAGIC + nfat_arch + zero-padding is enough for it to recognise
        // the file as MachOFat32.
        let dir = tempdir().unwrap();
        let path = dir.path().join("rabbit");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xca, 0xfe, 0xba, 0xbe]); // FAT_MAGIC (big-endian on disk)
        bytes.extend_from_slice(&0u32.to_be_bytes()); // nfat_arch = 0
        bytes.extend_from_slice(&[0u8; 8]); // pad to 16 bytes total
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            probe_executable_architecture(&path),
            Architecture::Universal
        );
    }

    #[test]
    fn probes_mach_o_thin_arm64() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rabbit");
        write_mach_o_thin_64(&path, MACHO_CPU_TYPE_ARM64);
        assert_eq!(probe_executable_architecture(&path), Architecture::Arm64);
    }

    #[test]
    fn probes_mach_o_thin_x86_64() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rabbit");
        write_mach_o_thin_64(&path, MACHO_CPU_TYPE_X86_64);
        assert_eq!(probe_executable_architecture(&path), Architecture::X64);
    }

    #[test]
    fn unknown_for_unrecognised_magic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("garbage.bin");
        std::fs::write(&path, b"not an executable header").unwrap();
        assert_eq!(probe_executable_architecture(&path), Architecture::Unknown);
    }

    #[test]
    fn unknown_for_missing_path() {
        let dir = tempdir().unwrap();
        assert_eq!(
            probe_executable_architecture(&dir.path().join("nope")),
            Architecture::Unknown
        );
    }

    #[test]
    fn drills_into_macos_app_bundle() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("REAPER.app");
        let macos = bundle.join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        write_mach_o_thin_64(&macos.join("REAPER"), MACHO_CPU_TYPE_ARM64);
        assert_eq!(probe_executable_architecture(&bundle), Architecture::Arm64);
    }

    const MACHO_CPU_TYPE_X86_64: u32 = 0x0100_0007;
    pub(crate) const MACHO_CPU_TYPE_ARM64: u32 = 0x0100_000c;

    /// Writes a minimal 32-byte little-endian Mach-O 64-bit executable header
    /// — just enough for `object::File::parse` to identify the architecture.
    /// `ncmds` / `sizeofcmds` stay zero, which `object` accepts because we
    /// only ask it for `architecture()`, not load-command iteration.
    pub(crate) fn write_mach_o_thin_64(path: &Path, cputype: u32) {
        let mut file = File::create(path).unwrap();
        // MH_MAGIC_64 in little-endian (the modern Mach-O on-disk magic on
        // both Apple Silicon and Intel macOS).
        file.write_all(&0xfeed_facfu32.to_le_bytes()).unwrap();
        file.write_all(&cputype.to_le_bytes()).unwrap();
        file.write_all(&0u32.to_le_bytes()).unwrap(); // cpusubtype
        file.write_all(&2u32.to_le_bytes()).unwrap(); // filetype = MH_EXECUTE
        file.write_all(&0u32.to_le_bytes()).unwrap(); // ncmds
        file.write_all(&0u32.to_le_bytes()).unwrap(); // sizeofcmds
        file.write_all(&0u32.to_le_bytes()).unwrap(); // flags
        file.write_all(&0u32.to_le_bytes()).unwrap(); // reserved
    }
}
