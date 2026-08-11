use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, RabbitError>;

#[derive(Debug, Error)]
pub enum RabbitError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("SQLite error at {path}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    #[error("HTTP error for {url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote data error for {url}: {message}")]
    RemoteData { url: String, message: String },

    #[error("invalid artifact URL {url}: {message}")]
    InvalidArtifactUrl { url: String, message: String },

    #[error(
        "the download of {url} kept getting interrupted (received {bytes_downloaded} bytes; last error: {message}); check the internet connection and try again"
    )]
    DownloadInterrupted {
        url: String,
        bytes_downloaded: u64,
        message: String,
    },

    #[error("hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("invalid backup id: {0}")]
    InvalidBackupId(String),

    #[error("backup not found at {0}")]
    BackupNotFound(PathBuf),

    #[error("no artifact found for {package_id} on {platform:?}/{architecture:?}")]
    NoArtifactFound {
        package_id: String,
        platform: crate::model::Platform,
        architecture: crate::model::Architecture,
    },

    #[error("artifact kind {kind:?} for {package_id} is not supported by this installer step")]
    UnsupportedArtifactKind {
        package_id: String,
        kind: crate::artifact::ArtifactKind,
    },

    #[error("could not resolve the install location for {package_id}: {reason}")]
    InstallLocationUnavailable { package_id: String, reason: String },

    #[error("invalid detector regex {pattern:?}: {message}")]
    InvalidDetectorPattern { pattern: String, message: String },

    #[error("archive {archive} for {package_id} did not contain a {package_id} extension binary")]
    ArchiveMissingExtensionBinary {
        archive: PathBuf,
        package_id: String,
    },

    #[error("archive {archive} could not be read: {message}")]
    ArchiveRead { archive: PathBuf, message: String },

    #[error("archive {archive} did not contain the expected OSARA installer assets")]
    OsaraArchiveMissingAssets { archive: PathBuf },

    #[error("disk image {image} for {package_id} did not contain a {package_id} extension binary")]
    DiskImageMissingExtensionBinary { image: PathBuf, package_id: String },

    #[error("disk image {image} did not contain the expected app bundle {bundle}")]
    DiskImageMissingAppBundle { image: PathBuf, bundle: String },

    #[error("disk image {image} could not be mounted: {message}")]
    DiskImageMount { image: PathBuf, message: String },

    #[error("a package installation is already in progress (lock {lock_path}, PID {pid})")]
    PackageInstallInProgress { lock_path: PathBuf, pid: u32 },

    #[error("self-update artifact {path} failed signature verification: {reason}")]
    SelfUpdateSignatureInvalid { path: PathBuf, reason: String },

    #[error("preflight failed: {message}")]
    PreflightFailed { message: String },

    #[error("invalid planned execution: {message}")]
    InvalidPlannedExecution { message: String },

    #[error("process failed for {program} with exit code {exit_code:?}")]
    ProcessFailed {
        program: String,
        exit_code: Option<i32>,
    },

    #[error(
        "the Windows administrator approval prompt for {program} was cancelled or declined; re-run and approve the prompt to continue, or pick a portable REAPER target that doesn't need elevation"
    )]
    UserCancelledElevation { program: String },

    #[error(
        "macOS blocked updating {path} ({source}). This is a permission block, not REAPER being open: grant RABBIT Full Disk Access (or App Management) under System Settings > Privacy & Security, then quit and relaunch RABBIT and try again. If that doesn't help, the file may be locked or owned by another account and need to be removed manually."
    )]
    MacOsWriteDenied {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "Windows security software blocked {path} ({source}). RABBIT could not read or move the file because Microsoft Defender (or another antivirus) flagged it and usually removed it. This is typically a FALSE POSITIVE on an unsigned installer such as an OSARA development snapshot, not a sign that RABBIT or the download is unsafe — the file was verified against the publisher's checksum before this step. To continue: open Windows Security > Virus & threat protection > Protection history, find the blocked item and choose Allow (or Restore), then run RABBIT again; alternatively add RABBIT's download folder as an exclusion, or install this one package manually from the publisher's own page. RABBIT deliberately never turns your virus protection off."
    )]
    WindowsFileBlockedByAntivirus {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("post-install verification failed; missing paths: {missing_paths:?}")]
    PostInstallVerificationFailed { missing_paths: Vec<PathBuf> },

    #[error("invalid version string: {0}")]
    InvalidVersion(String),

    #[error("localization error: {message}")]
    Localization {
        path: Option<PathBuf>,
        message: String,
    },

    #[error("unsupported platform")]
    UnsupportedPlatform,
}

pub trait IoPathContext<T> {
    fn with_path(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoPathContext<T> for std::io::Result<T> {
    fn with_path(self, path: impl Into<PathBuf>) -> Result<T> {
        let path = path.into();
        self.map_err(|source| io_error_at(path, source))
    }
}

/// Windows `ERROR_VIRUS_INFECTED` — the operation was refused because
/// security software flagged the file.
#[cfg(windows)]
const ERROR_VIRUS_INFECTED: i32 = 225;
/// Windows `ERROR_VIRUS_DELETED` — same, and the file was removed.
#[cfg(windows)]
const ERROR_VIRUS_DELETED: i32 = 226;

/// Turn a path-tagged [`std::io::Error`] into the most specific
/// [`RabbitError`] we can, so users get a cause and a fix instead of a raw
/// OS code.
///
/// Today that means recognising the two Windows status codes that mean
/// "antivirus blocked this": unlike, say, `ACCESS_DENIED`, codes 225/226
/// exist for no other reason, so classifying them here — rather than at one
/// call site — is unambiguous and covers every file operation RABBIT
/// performs (download, cache, extract, install).
pub fn io_error_at(path: PathBuf, source: std::io::Error) -> RabbitError {
    #[cfg(windows)]
    {
        if matches!(
            source.raw_os_error(),
            Some(ERROR_VIRUS_INFECTED) | Some(ERROR_VIRUS_DELETED)
        ) {
            return RabbitError::WindowsFileBlockedByAntivirus { path, source };
        }
    }
    RabbitError::Io { path, source }
}

pub trait JsonPathContext<T> {
    fn with_json_path(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> JsonPathContext<T> for serde_json::Result<T> {
    fn with_json_path(self, path: impl Into<PathBuf>) -> Result<T> {
        let path = path.into();
        self.map_err(|source| RabbitError::Json { path, source })
    }
}

pub trait SqlitePathContext<T> {
    fn with_sqlite_path(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> SqlitePathContext<T> for rusqlite::Result<T> {
    fn with_sqlite_path(self, path: impl Into<PathBuf>) -> Result<T> {
        let path = path.into();
        self.map_err(|source| RabbitError::Sqlite { path, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Codes that mean something other than "antivirus blocked this" must
    /// keep the generic I/O shape on every platform.
    #[test]
    fn classifies_unrelated_os_errors_as_generic_io() {
        let path = PathBuf::from("cache/osara/osara.exe");
        // 2 = ENOENT / ERROR_FILE_NOT_FOUND, 5 = EIO / ERROR_ACCESS_DENIED.
        for code in [2, 5] {
            let error = io_error_at(path.clone(), std::io::Error::from_raw_os_error(code));
            assert!(
                matches!(error, RabbitError::Io { .. }),
                "os error {code} should stay a generic Io error"
            );
        }
    }

    /// `ERROR_VIRUS_INFECTED` (225) and `ERROR_VIRUS_DELETED` (226) exist
    /// only for security-software blocks, so they must produce the
    /// actionable antivirus error instead of a raw I/O code — this is the
    /// failure a user hit when Defender quarantined OSARA's unsigned
    /// snapshot installer mid-install.
    #[cfg(windows)]
    #[test]
    fn classifies_windows_antivirus_blocks() {
        let path = PathBuf::from(r"C:\Users\x\AppData\Local\Temp\rabbit-cache\osara\osara.exe");
        for code in [ERROR_VIRUS_INFECTED, ERROR_VIRUS_DELETED] {
            let error = io_error_at(path.clone(), std::io::Error::from_raw_os_error(code));
            assert!(
                matches!(error, RabbitError::WindowsFileBlockedByAntivirus { .. }),
                "os error {code} should map to WindowsFileBlockedByAntivirus"
            );
            let message = error.to_string();
            // The message has to carry the path and the self-service fix,
            // and must never suggest turning protection off.
            assert!(message.contains("osara.exe"), "{message}");
            assert!(message.contains("Protection history"), "{message}");
            assert!(message.contains("FALSE POSITIVE"), "{message}");
            assert!(
                message.contains("never turns your virus protection off"),
                "{message}"
            );
        }
    }
}
