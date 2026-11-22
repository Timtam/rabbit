use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Windows,
    MacOs,
}

impl Platform {
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "windows") {
            Some(Self::Windows)
        } else if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else {
            None
        }
    }

    pub fn extension_library_suffix(self) -> &'static str {
        match self {
            Self::Windows => ".dll",
            Self::MacOs => ".dylib",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Architecture {
    X86,
    X64,
    Arm64,
    Arm64Ec,
    Universal,
    Unknown,
}

impl Architecture {
    pub fn current() -> Self {
        if cfg!(target_arch = "x86") {
            Self::X86
        } else if cfg!(target_arch = "x86_64") {
            Self::X64
        } else if cfg!(target_arch = "aarch64") {
            Self::Arm64
        } else {
            Self::Unknown
        }
    }

    /// Token used in release artifact filenames (`rabbit-<version>-<os>-<arch>[.exe]`).
    /// Returns `None` for variants that the release pipeline does not produce, so
    /// callers can safely skip arch-mismatch checks against unknown / synthetic builds.
    pub fn release_artifact_token(self) -> Option<&'static str> {
        match self {
            Self::X86 => Some("i686"),
            Self::X64 => Some("x86_64"),
            Self::Arm64 => Some("aarch64"),
            Self::Arm64Ec | Self::Universal | Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallationKind {
    Standard,
    Portable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub source: String,
    pub path: Option<PathBuf>,
    pub detail: String,
}

impl Evidence {
    pub fn new(
        source: impl Into<String>,
        path: Option<PathBuf>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            path,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installation {
    pub kind: InstallationKind,
    pub platform: Platform,
    pub app_path: PathBuf,
    pub resource_path: PathBuf,
    pub version: Option<Version>,
    pub architecture: Option<Architecture>,
    pub writable: bool,
    pub confidence: Confidence,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentDetection {
    pub package_id: String,
    pub display_name: String,
    pub installed: bool,
    pub version: Option<Version>,
    pub detector: String,
    pub confidence: Confidence,
    pub files: Vec<PathBuf>,
    pub notes: Vec<String>,
    /// Why RABBIT leaves this copy to ReaPack, for a package whose manifest
    /// says `leave_to_reapack`. `None` when RABBIT may install, update and
    /// uninstall it as usual. Only `detect_components` sets it, since it
    /// alone knows whether ReaPack is installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_to_reapack: Option<LeftToReapack>,
}

impl ComponentDetection {
    pub fn not_installed(package_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            package_id: package_id.into(),
            display_name: display_name.into(),
            installed: false,
            version: None,
            detector: "not-found".to_string(),
            confidence: Confidence::High,
            files: Vec::new(),
            notes: Vec::new(),
            left_to_reapack: None,
        }
    }
}

/// Why RABBIT leaves a package's copy to ReaPack: it doesn't install over
/// it, update it or uninstall it. Only for a package whose manifest says
/// `leave_to_reapack`, and only while ReaPack is installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "reason")]
pub enum LeftToReapack {
    /// ReaPack's records name the package's plug-in: ReaPack installed it
    /// as `reapack_package` and keeps it up to date.
    Managed { reapack_package: String },
    /// RABBIT couldn't read ReaPack's records, so it can't tell whether
    /// ReaPack installed the package, and leaves it alone to be safe.
    RecordsUnreadable { error: String },
}
