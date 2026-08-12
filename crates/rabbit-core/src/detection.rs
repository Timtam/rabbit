use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::arch_probe::probe_executable_architecture;
use crate::error::{IoPathContext, RabbitError, Result};
use crate::metadata::file_version;
use crate::model::{
    ComponentDetection, Confidence, Evidence, Installation, InstallationKind, Platform,
};
use crate::package::{InstallDestination, PackageDetector, PackageSpec, builtin_package_specs};
use crate::reapack::package_owner_for_file;
use crate::receipt::{ReceiptVerification, load_install_state, verify_package_receipt};
use regex::Regex;

#[derive(Debug, Clone, Default)]
pub struct DiscoveryOptions {
    pub include_standard: bool,
    pub portable_roots: Vec<PathBuf>,
}

impl DiscoveryOptions {
    pub fn standard() -> Self {
        Self {
            include_standard: true,
            portable_roots: Vec::new(),
        }
    }
}

pub fn discover_installations(options: &DiscoveryOptions) -> Result<Vec<Installation>> {
    let Some(platform) = Platform::current() else {
        return Ok(Vec::new());
    };

    let mut installations = Vec::new();

    if options.include_standard
        && let Some(standard) = discover_standard_installation(platform)
    {
        installations.push(standard);
    }

    for portable_root in &options.portable_roots {
        if let Some(portable) = discover_portable_installation(platform, portable_root) {
            installations.push(portable);
        }
    }

    Ok(installations)
}

pub fn default_standard_installation(platform: Platform) -> Option<Installation> {
    match platform {
        Platform::Windows => standard_windows_installation(false),
        Platform::MacOs => standard_macos_installation(false),
    }
}

/// If the folder holding the currently running RABBIT executable also
/// contains a portable REAPER installation, return that folder.
///
/// This powers the "drop RABBIT next to a portable REAPER" convenience:
/// the wizard folds this path into its portable roots and selects it by
/// default, so a user who keeps RABBIT alongside their portable REAPER can
/// check for updates without ever reaching for the Browse button.
///
/// Returns `None` when the executable path can't be resolved, the platform
/// is unsupported, or no portable REAPER lives next to the executable.
pub fn colocated_portable_root() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    portable_root_in_dir(dir)
}

/// Directory-driven core of [`colocated_portable_root`], split out so it can
/// be exercised in tests without depending on `std::env::current_exe`.
/// Returns `Some(dir)` when `dir` holds a portable REAPER install that
/// [`discover_installations`] would surface (reaper.exe + reaper.ini on
/// Windows; a `REAPER*.app` bundle on macOS).
fn portable_root_in_dir(dir: &Path) -> Option<PathBuf> {
    let platform = Platform::current()?;
    discover_portable_installation(platform, dir).map(|_| dir.to_path_buf())
}

pub fn detect_components(
    resource_path: &Path,
    platform: Platform,
) -> Result<Vec<ComponentDetection>> {
    detect_components_with_probes(
        resource_path,
        platform,
        rabbit_platform::read_uninstall_display_version,
    )
}

pub(crate) fn detect_components_with_probes(
    resource_path: &Path,
    platform: Platform,
    uninstall_display_version: fn(&str) -> Option<String>,
) -> Result<Vec<ComponentDetection>> {
    let state = load_install_state(resource_path)?;
    let mut detections = Vec::new();

    for spec in builtin_package_specs(platform) {
        detections.push(detect_component_with_probes(
            resource_path,
            platform,
            &spec,
            state.as_ref(),
            uninstall_display_version,
        )?);
    }

    Ok(detections)
}

pub fn detect_component(
    resource_path: &Path,
    platform: Platform,
    spec: &PackageSpec,
    state: Option<&crate::receipt::InstallState>,
) -> Result<ComponentDetection> {
    detect_component_with_probes(
        resource_path,
        platform,
        spec,
        state,
        rabbit_platform::read_uninstall_display_version,
    )
}

pub(crate) fn detect_component_with_probes(
    resource_path: &Path,
    platform: Platform,
    spec: &PackageSpec,
    state: Option<&crate::receipt::InstallState>,
    uninstall_display_version: fn(&str) -> Option<String>,
) -> Result<ComponentDetection> {
    match verify_package_receipt(resource_path, state, &spec.id)? {
        ReceiptVerification::Verified(receipt) => {
            let files = receipt
                .installed_files
                .iter()
                .map(|file| resource_path.join(&file.path))
                .collect();
            return Ok(ComponentDetection {
                package_id: spec.id.clone(),
                display_name: spec.display_name.clone(),
                installed: true,
                version: receipt.version,
                detector: "rabbit-receipt".to_string(),
                confidence: Confidence::High,
                files,
                notes: Vec::new(),
            });
        }
        ReceiptVerification::Mismatch(receipt) => {
            let files = matching_user_plugin_files(resource_path, platform, spec)?;
            if !files.is_empty() {
                return Ok(ComponentDetection {
                    package_id: spec.id.clone(),
                    display_name: spec.display_name.clone(),
                    installed: true,
                    version: receipt.version,
                    detector: "rabbit-receipt-mismatch".to_string(),
                    confidence: Confidence::Medium,
                    files,
                    notes: vec![
                        "RABBIT has a receipt for this package, but installed files do not match it."
                            .to_string(),
                    ],
                });
            }
        }
        ReceiptVerification::MissingReceipt | ReceiptVerification::MissingPackage => {}
    }

    let files = matching_user_plugin_files(resource_path, platform, spec)?;

    // Run the package's detector probes in their declared order; the first
    // that yields a detection wins. This is the data-driven replacement for
    // the old package-id-keyed fallback chain — each package's `detectors`
    // list now encodes its exact probe order and parameters (registry name,
    // scan pattern). Receipt (above) and file-presence (below) are the
    // special bookends. The file-based probes no-op on an empty file set, so
    // out-of-tree detectors (JAWS / Surge) coexist in the same loop.
    for detector in &spec.detectors {
        if let Some(detection) = run_detector_probe(
            detector,
            resource_path,
            platform,
            spec,
            &files,
            uninstall_display_version,
        )? {
            return Ok(detection);
        }
    }

    if files.is_empty() {
        return Ok(ComponentDetection::not_installed(
            spec.id.clone(),
            spec.display_name.clone(),
        ));
    }

    // A matching file is present but no probe could read a version.
    Ok(ComponentDetection {
        package_id: spec.id.clone(),
        display_name: spec.display_name.clone(),
        installed: true,
        version: None,
        detector: "file-presence".to_string(),
        confidence: Confidence::Medium,
        files,
        notes: vec!["Package is present, but this RABBIT version cannot reliably read its version without a RABBIT receipt.".to_string()],
    })
}

/// A version recovered from on-disk files by [`detect_version_from_files_with_probes`]:
/// `(version, detector name, confidence, notes)`.
/// Run a single detector probe. Version probes (file-version,
/// reapack-registry, uninstall-displayversion, binary-string, ffmpeg) read
/// `files`; out-of-tree probes (JAWS, Surge) ignore them. `RabbitReceipt` /
/// `UserPluginFile` are caller-handled bookends and no-op here. Returns the
/// first detection this probe yields, else `None`.
fn run_detector_probe(
    detector: &PackageDetector,
    resource_path: &Path,
    platform: Platform,
    spec: &PackageSpec,
    files: &[PathBuf],
    uninstall_display_version: fn(&str) -> Option<String>,
) -> Result<Option<ComponentDetection>> {
    let detection = match detector {
        PackageDetector::RabbitReceipt | PackageDetector::UserPluginFile => None,
        PackageDetector::FileVersionMetadata => {
            let mut found = None;
            for file in files {
                if let Some(version) = file_version(file)? {
                    found = Some(version_component(
                        spec,
                        version,
                        "file-version-metadata",
                        Confidence::High,
                        Vec::new(),
                        files,
                    ));
                    break;
                }
            }
            found
        }
        PackageDetector::ReapackRegistry => {
            let mut found = None;
            for file in files {
                if let Some(owner) = package_owner_for_file(resource_path, file)? {
                    found = Some(version_component(
                        spec,
                        owner.version,
                        "reapack-registry",
                        Confidence::High,
                        vec![format!(
                            "Version came from ReaPack registry entry {}/{}/{}.",
                            owner.remote, owner.category, owner.package
                        )],
                        files,
                    ));
                    break;
                }
            }
            found
        }
        // Registry probe is gated on a matching file being present, matching
        // the old behavior (it lived inside the files-non-empty path): the
        // uninstall key alone, without the plug-in on disk, shouldn't report
        // the package as installed.
        PackageDetector::UninstallDisplayVersion { display_name } if !files.is_empty() => {
            uninstall_display_version(display_name)
                .and_then(|value| crate::version::Version::parse(&value).ok())
                .map(|version| {
                    version_component(
                        spec,
                        version,
                        "windows-uninstall-displayversion",
                        Confidence::High,
                        vec![format!(
                            "Version came from the {display_name} Windows installer's Uninstall registry key."
                        )],
                        files,
                    )
                })
        }
        PackageDetector::UninstallDisplayVersion { .. } => None,
        PackageDetector::BinaryVersionString {
            pattern,
            label,
            note,
        } => {
            let regex = Regex::new(pattern).map_err(|err| RabbitError::InvalidDetectorPattern {
                pattern: pattern.clone(),
                message: err.to_string(),
            })?;
            let mut found = None;
            for file in files {
                if let Some(version) = scan_binary_version(file, &regex)? {
                    found = Some(version_component(
                        spec,
                        version,
                        label,
                        Confidence::Medium,
                        vec![note.clone()],
                        files,
                    ));
                    break;
                }
            }
            found
        }
        PackageDetector::FfmpegLibavformat => detect_ffmpeg_version(files)
            .map(|(version, detector, confidence, notes)| {
                version_component(spec, version, &detector, confidence, notes, files)
            }),
        PackageDetector::JawsUninstallExe => detect_jaws_scripts_via_uninstall_exe(spec),
        PackageDetector::SurgeVendorFiles => {
            detect_surge_xt_vendor_files(spec, platform, uninstall_display_version)
        }
    };
    Ok(detection)
}

/// Build an installed [`ComponentDetection`] from a recovered version.
fn version_component(
    spec: &PackageSpec,
    version: crate::version::Version,
    detector: &str,
    confidence: Confidence,
    notes: Vec<String>,
    files: &[PathBuf],
) -> ComponentDetection {
    ComponentDetection {
        package_id: spec.id.clone(),
        display_name: spec.display_name.clone(),
        installed: true,
        version: Some(version),
        detector: detector.to_string(),
        confidence,
        files: files.to_vec(),
        notes,
    }
}

/// Scan a binary file for an embedded version string matched by `regex`
/// (capture group 1). Generalizes the OSARA / ReaKontrol / SWS / ReaPack
/// embedded-version-string scanners — each is just a different anchored
/// pattern carried in the manifest. Reads the whole file (these plug-ins are
/// small) and uses the first capture that parses as a version.
fn scan_binary_version(path: &Path, regex: &Regex) -> Result<Option<crate::version::Version>> {
    let bytes = fs::read(path).with_path(path)?;
    Ok(scan_text_version(&String::from_utf8_lossy(&bytes), regex))
}

fn scan_text_version(text: &str, regex: &Regex) -> Option<crate::version::Version> {
    regex.captures_iter(text).find_map(|captures| {
        captures
            .get(1)
            .and_then(|group| crate::version::Version::parse(group.as_str()).ok())
    })
}

/// FFmpeg version detector that tries three probes in order:
///
/// 1. **`ffmpeg.exe` `ProductVersion` StringFileInfo** — vanilla
///    FFmpeg builds attach only a manifest to the binaries, so this
///    field is usually absent; but if a vendor (winget MSI wrapper,
///    custom build) patches it in, we read it for free without a
///    process spawn. `FileVersion` carries `LIBAVUTIL_VERSION` and
///    isn't useful here (libavutil 60.x.y for FFmpeg 8.x).
/// 2. **Binary string scan of `ffmpeg.exe`** — vanilla FFmpeg's
///    `show_banner` format string is a single contiguous literal of
///    the shape `"%s version <FFMPEG_VERSION>, Copyright (c) %d-%d the
///    FFmpeg developers\n"`, with `FFMPEG_VERSION` substituted at
///    compile time. We anchor on the distinctive `the FFmpeg
///    developers` suffix, then read backwards to pick out the version
///    token between `version ` and `,`. This replaces the previous
///    `ffmpeg.exe -version` subprocess spawn, which on Windows blocked
///    the UI thread for ~30 s while AV scanned FFmpeg's dozens of
///    DLL dependencies on every launch. The scan is two `memmem`
///    searches over a ~100 KB read — well below 1 ms in practice.
/// 3. **libavformat-major filename heuristic** — fallback when no
///    executable is present or the binary scan can't find the
///    anchor. Maps `avformat-XX.dll`'s libavformat major to the
///    FFmpeg release major (lib 58→FFmpeg 4, 59→5, 60→6, 61→7,
///    62→8, i.e. `lib_major - 54`). Patch level isn't recoverable
///    from the filename alone, so we synthesize as `<major>.0.0` at
///    `Medium` confidence.
fn detect_ffmpeg_version(
    files: &[PathBuf],
) -> Option<(crate::version::Version, String, Confidence, Vec<String>)> {
    // Walk `ffmpeg.exe` / `ffprobe.exe` / `ffplay.exe`. Any matched
    // file's parent points at the UserPlugins directory the
    // executables live in.
    let candidates: Vec<PathBuf> = files
        .iter()
        .find_map(|file| file.parent())
        .map(|parent| {
            ["ffmpeg.exe", "ffprobe.exe", "ffplay.exe"]
                .iter()
                .map(|name| parent.join(name))
                .filter(|path| path.is_file())
                .collect()
        })
        .unwrap_or_default();

    // Probe 1: ProductVersion (cheap, no process spawn). Almost always
    // absent on vanilla FFmpeg — kept so a future vendor that patches
    // VERSIONINFO works without a spawn.
    for exe_path in &candidates {
        let Some(raw) = rabbit_platform::read_string_file_info_key(exe_path, "ProductVersion")
        else {
            continue;
        };
        let Some(version) = ffmpeg_version_from_product_version_string(&raw) else {
            continue;
        };
        let major = version.numeric_parts().first().copied().unwrap_or(0);
        if (4..=9).contains(&major) {
            let exe_name = exe_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("ffmpeg.exe")
                .to_string();
            return Some((
                version,
                "ffmpeg-productversion".to_string(),
                Confidence::High,
                vec![format!(
                    "Version came from {exe_name}'s `ProductVersion` StringFileInfo entry ({raw:?})."
                )],
            ));
        }
    }

    // Probe 2: binary string scan of `ffmpeg.exe`. The vanilla FFmpeg
    // build's `show_banner` format string embeds `FFMPEG_VERSION`
    // verbatim, so the literal version sits inside one contiguous
    // string in `.rdata` we can grep out without touching the
    // executable as a process. We invoke at most one of the three
    // candidates (ffmpeg, ffprobe, ffplay) — they all share the same
    // fftools cmdutils path, so the first hit is authoritative.
    for exe_path in &candidates {
        let Ok(bytes) = fs::read(exe_path) else {
            continue;
        };
        let Some(version) = ffmpeg_version_from_binary_bytes(&bytes) else {
            continue;
        };
        let major = version.numeric_parts().first().copied().unwrap_or(0);
        if (4..=9).contains(&major) {
            let exe_name = exe_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("ffmpeg.exe")
                .to_string();
            return Some((
                version,
                "ffmpeg-binary-version-string".to_string(),
                Confidence::High,
                vec![format!(
                    "Version came from a binary scan of {exe_name} for FFmpeg's `show_banner` format string."
                )],
            ));
        }
    }

    // Probe 3: libavformat-major filename heuristic.
    for file in files {
        let basename = file.file_name().and_then(|name| name.to_str())?;
        if let Some(lib_major) = libavformat_major_from_basename(basename) {
            let ffmpeg_major = ffmpeg_major_from_libavformat_major(lib_major)?;
            let version = crate::version::Version::parse(format!("{ffmpeg_major}.0.0")).ok()?;
            return Some((
                version,
                "ffmpeg-libavformat-major".to_string(),
                Confidence::Medium,
                vec![format!(
                    "Mapped {basename}'s libavformat major version to the corresponding FFmpeg release major. The patch level isn't recoverable from the DLL filename alone, so the detected version is reported as `<major>.0.0`."
                )],
            ));
        }
    }
    None
}

/// Scan a vanilla FFmpeg binary for the `show_banner` format-string
/// literal and pull the embedded `FFMPEG_VERSION` out of it. The
/// banner is one contiguous string. Upstream FFmpeg uses:
///
/// ```text
/// %s version <VERSION>, Copyright (c) <year>-<year> the FFmpeg developers
/// ```
///
/// — but some redistributors patch the format. Gyan.dev's full builds,
/// for example, drop the comma and pad the version with trailing
/// spaces (`%s version 8.1.1-full_build-www.gyan.dev         Copyright
/// (c) …`), and tordona's ARM snapshots embed `n8.1.1-6-…` directly.
/// We anchor on the distinctive `the FFmpeg developers` suffix, walk
/// back to find the trailing `version ` token, and treat the *next*
/// `Copyright` (with any preceding comma/whitespace stripped) as the
/// terminator. That matches both the upstream and the Gyan variant
/// without enumerating every redistributor's exact format.
///
/// `<VERSION>` is whatever `FFMPEG_VERSION` was `#define`d to at
/// compile time — `8.1.1` for Gyan stable, `n8.1.1` / `n8.1.1-6-…`
/// for snapshot / git builds — which we parse with the same helper
/// that handles `ProductVersion`. Returns `None` when the anchor
/// can't be found (non-FFmpeg binary, stripped strings table, …).
fn ffmpeg_version_from_binary_bytes(bytes: &[u8]) -> Option<crate::version::Version> {
    const ANCHOR: &[u8] = b" the FFmpeg developers";
    let anchor_pos = bytes.windows(ANCHOR.len()).position(|w| w == ANCHOR)?;
    // The format string is one contiguous run; cap the look-back at
    // 256 bytes so a stripped-string-table binary that happens to have
    // the anchor in an unrelated context can't drag us through MB of
    // bytes hunting for `version `.
    let start_limit = anchor_pos.saturating_sub(256);
    let prelude = &bytes[start_limit..anchor_pos];
    let prelude_str = std::str::from_utf8(prelude).ok()?;
    let version_marker = "version ";
    let version_pos = prelude_str.rfind(version_marker)?;
    let after = &prelude_str[version_pos + version_marker.len()..];
    let copyright_pos = after.find("Copyright")?;
    let version_str =
        after[..copyright_pos].trim_end_matches(|ch: char| ch.is_whitespace() || ch == ',');
    ffmpeg_version_from_product_version_string(version_str)
}

/// Pull the leading version-shaped run out of FFmpeg's
/// `ProductVersion` string. Accepts forms like:
///
/// - `8.1.1` (Gyan stable)
/// - `n8.1.1` (BtbN-style tag)
/// - `n8.1.1-6-gdeadbeef-tordona-ffmpeg-builds-binaries` (tordona)
/// - `8.1.1-6-gdeadbeef`
///
/// Strips an optional `n` / `v` prefix, takes the longest leading
/// digit-or-dot run, and parses it as a `Version`.
fn ffmpeg_version_from_product_version_string(raw: &str) -> Option<crate::version::Version> {
    let stripped = raw.trim().trim_start_matches(['n', 'N', 'v', 'V']);
    let mut end = 0;
    for (idx, ch) in stripped.char_indices() {
        if ch.is_ascii_digit() || ch == '.' {
            end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let candidate = stripped[..end].trim_end_matches('.');
    crate::version::Version::parse(candidate).ok()
}

/// Parse the libavformat major from `avformat-<MAJOR>.dll` (Windows) or
/// `libavformat.<MAJOR>.dylib` (macOS) filenames. Other filenames
/// return `None`.
fn libavformat_major_from_basename(basename: &str) -> Option<u64> {
    let lower = basename.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("avformat-") {
        let stem = rest.strip_suffix(".dll")?;
        return stem.parse().ok();
    }
    if let Some(rest) = lower.strip_prefix("libavformat.") {
        let stem = rest.strip_suffix(".dylib")?;
        return stem.parse().ok();
    }
    None
}

fn ffmpeg_major_from_libavformat_major(libavformat_major: u64) -> Option<u64> {
    // libavformat 58 → FFmpeg 4; each subsequent libavformat major
    // bump corresponds to the next FFmpeg major. Computed rather than
    // table-lookup so a future FFmpeg release that follows the same
    // pattern keeps working without a code change.
    libavformat_major.checked_sub(54)
}

/// The directory a package's plug-in file(s) live in. The default is
/// `<resource>/UserPlugins`; a data-driven package that declares a
/// `github_release.install_destination` other than UserPlugins (app2clap's
/// per-user CLAP folder) is scanned there instead. Returns `None` when the
/// destination can't be resolved (e.g. the CLAP folder on a non-Windows
/// host), in which case detection finds no files.
fn package_scan_dir(resource_path: &Path, spec: &PackageSpec) -> Option<PathBuf> {
    match spec.install_destination {
        InstallDestination::WindowsClapDir => rabbit_platform::windows_clap_dir(),
        InstallDestination::LangPack => Some(resource_path.join("LangPack")),
        InstallDestination::UserPlugins => Some(resource_path.join("UserPlugins")),
    }
}

pub(crate) fn matching_user_plugin_files(
    resource_path: &Path,
    _platform: Platform,
    spec: &PackageSpec,
) -> Result<Vec<PathBuf>> {
    let Some(scan_dir) = package_scan_dir(resource_path, spec) else {
        return Ok(Vec::new());
    };
    if !scan_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&scan_dir).with_path(&scan_dir)? {
        let entry = entry.with_path(&scan_dir)?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let lower_name = file_name.to_ascii_lowercase();

        let prefix_matches = spec
            .user_plugin_prefixes
            .iter()
            .any(|prefix| lower_name.starts_with(&prefix.to_ascii_lowercase()));
        let suffix_matches = spec
            .user_plugin_suffixes
            .iter()
            .any(|suffix| lower_name.ends_with(&suffix.to_ascii_lowercase()));

        if prefix_matches && suffix_matches {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn discover_standard_installation(platform: Platform) -> Option<Installation> {
    match platform {
        Platform::Windows => standard_windows_installation(true),
        Platform::MacOs => standard_macos_installation(true),
    }
}

fn discover_portable_installation(platform: Platform, root: &Path) -> Option<Installation> {
    match platform {
        Platform::Windows => discover_portable_windows(root),
        Platform::MacOs => discover_portable_macos(root),
    }
}

fn standard_windows_installation(require_existing: bool) -> Option<Installation> {
    let resource_path = rabbit_platform::user_appdata_dir().map(|path| path.join("REAPER"))?;

    let app_path = windows_reaper_app_candidates()
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\REAPER\reaper.exe"));

    if require_existing && !app_path.exists() && !resource_path.exists() {
        return None;
    }

    let mut evidence = Vec::new();
    if app_path.exists() {
        evidence.push(Evidence::new(
            "standard-windows-app-path",
            Some(app_path.clone()),
            "Found reaper.exe in a standard application directory.",
        ));
    }
    if resource_path.exists() {
        evidence.push(Evidence::new(
            "standard-windows-resource-path",
            Some(resource_path.clone()),
            "Found the standard REAPER resource path.",
        ));
    }

    let version = file_version(&app_path).ok().flatten();
    if let Some(version) = &version {
        evidence.push(Evidence::new(
            "standard-windows-file-version",
            Some(app_path.clone()),
            format!("Read REAPER version {version} from executable metadata."),
        ));
    }

    let probed_architecture = probe_executable_architecture(&app_path);
    Some(Installation {
        kind: InstallationKind::Standard,
        platform: Platform::Windows,
        app_path,
        resource_path: resource_path.clone(),
        version,
        architecture: Some(probed_architecture),
        writable: is_probably_writable(&resource_path),
        confidence: if !require_existing && evidence.is_empty() {
            Confidence::Low
        } else if evidence.len() > 1 {
            Confidence::High
        } else {
            Confidence::Medium
        },
        evidence,
    })
}

fn windows_reaper_app_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // Prefer the install path the REAPER uninstaller wrote to the registry.
    // This catches non-default install dirs the user may have picked, plus the
    // default 64-bit location `C:\Program Files\REAPER (x64)\` which the
    // hardcoded `Program Files\REAPER\` fallback below misses.
    for key in ["REAPER", "REAPER_x64", "REAPER (x64)", "REAPER (x86_64)"] {
        if let Some(install_location) = rabbit_platform::read_uninstall_install_location(key) {
            let trimmed = install_location.trim().trim_end_matches(['\\', '/']);
            if !trimmed.is_empty() {
                let candidate = PathBuf::from(trimmed).join("reaper.exe");
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
    }

    // Also walk the standard Program Files dirs for both the plain `REAPER`
    // subfolder and the `REAPER (x64)` variant the 64-bit installer uses by
    // default. Order matters: registry hits win, then 64-bit-named variants,
    // then the bare folder name.
    for program_files in rabbit_platform::windows_program_files_dirs() {
        for subdir in ["REAPER (x64)", "REAPER (x86_64)", "REAPER"] {
            let candidate = program_files.join(subdir).join("reaper.exe");
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }

    candidates
}

fn discover_portable_windows(root: &Path) -> Option<Installation> {
    let app_path = root.join("reaper.exe");
    let ini_path = root.join("reaper.ini");
    if !app_path.is_file() || !ini_path.is_file() {
        return None;
    }

    let version = file_version(&app_path).ok().flatten();
    let mut evidence = vec![
        Evidence::new(
            "portable-windows-app-path",
            Some(app_path.clone()),
            "Found reaper.exe in the selected portable folder.",
        ),
        Evidence::new(
            "portable-windows-reaper-ini",
            Some(ini_path),
            "Found reaper.ini in the selected portable folder.",
        ),
    ];
    if let Some(version) = &version {
        evidence.push(Evidence::new(
            "portable-windows-file-version",
            Some(app_path.clone()),
            format!("Read REAPER version {version} from executable metadata."),
        ));
    }

    let probed_architecture = probe_executable_architecture(&app_path);
    Some(Installation {
        kind: InstallationKind::Portable,
        platform: Platform::Windows,
        app_path: app_path.clone(),
        resource_path: root.to_path_buf(),
        version,
        architecture: Some(probed_architecture),
        writable: is_probably_writable(root),
        confidence: Confidence::High,
        evidence,
    })
}

fn standard_macos_installation(require_existing: bool) -> Option<Installation> {
    let home = rabbit_platform::user_home_dir()?;
    let resource_path = home
        .join("Library")
        .join("Application Support")
        .join("REAPER");
    let app_path = [
        "/Applications/REAPER.app",
        "/Applications/REAPER64.app",
        "/Applications/REAPER-ARM.app",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|path| path.exists())
    .unwrap_or_else(|| PathBuf::from("/Applications/REAPER.app"));

    if require_existing && !app_path.exists() && !resource_path.exists() {
        return None;
    }

    let mut evidence = Vec::new();
    if app_path.exists() {
        evidence.push(Evidence::new(
            "standard-macos-app-path",
            Some(app_path.clone()),
            "Found REAPER.app in /Applications.",
        ));
    }
    if resource_path.exists() {
        evidence.push(Evidence::new(
            "standard-macos-resource-path",
            Some(resource_path.clone()),
            "Found the standard REAPER resource path.",
        ));
    }

    let version = file_version(&app_path).ok().flatten();
    if let Some(version) = &version {
        evidence.push(Evidence::new(
            "standard-macos-app-version",
            Some(app_path.clone()),
            format!("Read REAPER version {version} from app metadata."),
        ));
    }

    let probed_architecture = probe_executable_architecture(&app_path);
    Some(Installation {
        kind: InstallationKind::Standard,
        platform: Platform::MacOs,
        app_path,
        resource_path: resource_path.clone(),
        version,
        architecture: Some(probed_architecture),
        writable: is_probably_writable(&resource_path),
        confidence: if !require_existing && evidence.is_empty() {
            Confidence::Low
        } else if evidence.len() > 1 {
            Confidence::High
        } else {
            Confidence::Medium
        },
        evidence,
    })
}

fn discover_portable_macos(root: &Path) -> Option<Installation> {
    let app_path = fs::read_dir(root)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.to_ascii_lowercase().contains("reaper"))
        })?;

    let ini_path = root.join("reaper.ini");
    let mut evidence = vec![Evidence::new(
        "portable-macos-app-bundle",
        Some(app_path.clone()),
        "Found a REAPER app bundle in the selected portable folder.",
    )];
    let confidence = if ini_path.exists() {
        evidence.push(Evidence::new(
            "portable-macos-reaper-ini",
            Some(ini_path),
            "Found reaper.ini in the selected portable folder.",
        ));
        Confidence::High
    } else {
        Confidence::Medium
    };

    let version = file_version(&app_path).ok().flatten();
    if let Some(version) = &version {
        evidence.push(Evidence::new(
            "portable-macos-app-version",
            Some(app_path.clone()),
            format!("Read REAPER version {version} from app metadata."),
        ));
    }

    let probed_architecture = probe_executable_architecture(&app_path);
    Some(Installation {
        kind: InstallationKind::Portable,
        platform: Platform::MacOs,
        app_path,
        resource_path: root.to_path_buf(),
        version,
        architecture: Some(probed_architecture),
        writable: is_probably_writable(root),
        confidence,
        evidence,
    })
}

fn is_probably_writable(path: &Path) -> bool {
    let existing_path = if path.exists() {
        path
    } else {
        path.parent().unwrap_or(path)
    };

    fs::metadata(existing_path)
        .map(|metadata| !metadata.permissions().readonly())
        .unwrap_or(false)
}

/// Detect a JAWS-for-REAPER scripts install by reading the version that the
/// vendor NSIS installer left on disk. The flow is:
///
///   1. Read the `Reaper_JawsScripts` Programs-and-Features uninstall key's
///      `UninstallDirectory` REG_SZ value (HKLM\SOFTWARE\WoW6432Node\… on
///      64-bit Windows). The NSIS installer writes this value during install.
///   2. Read the StringFileInfo "FileVersion" resource off
///      `<dir>\Uninstall.exe`. The script author bumps it per release, so
///      it's the most reliable on-disk version stamp for users who haven't
///      let RABBIT install the package yet (the receipt detector handles the
///      RABBIT-managed case earlier).
///
/// Returns `None` when the registry key is missing, the uninstaller is
/// missing, or the FileVersion resource cannot be parsed as a RABBIT `Version`.
/// Always `None` on non-Windows hosts.
fn detect_jaws_scripts_via_uninstall_exe(spec: &PackageSpec) -> Option<ComponentDetection> {
    let install_dir =
        rabbit_platform::read_uninstall_value("Reaper_JawsScripts", "UninstallDirectory")?;
    let uninstall_exe = PathBuf::from(install_dir).join("Uninstall.exe");
    if !uninstall_exe.is_file() {
        return None;
    }
    let raw = rabbit_platform::read_file_version_string(&uninstall_exe)?;
    let version = crate::version::Version::parse(&raw).ok()?;
    Some(ComponentDetection {
        package_id: spec.id.clone(),
        display_name: spec.display_name.clone(),
        installed: true,
        version: Some(version),
        detector: "jaws-scripts-uninstall-exe".to_string(),
        confidence: Confidence::High,
        files: vec![uninstall_exe],
        notes: vec![
            "Version came from the JAWS-for-REAPER scripts vendor uninstaller's FileVersion resource."
                .to_string(),
        ],
    })
}

/// AppId of the Surge XT Inno Setup installer (`MyID` constant in the
/// upstream `surge64.iss`). Combined with Inno Setup's `_is1` suffix
/// this is the literal registry key name under
/// `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\` that the
/// installer writes `DisplayVersion` into. Inno Setup stores the GUID
/// *without* curly braces, which is why the value here is the raw
/// dashed form rather than `{…}`.
const SURGE_XT_INNO_SETUP_UNINSTALL_KEY: &str = "69F3FE96-DEEC-4C7C-B72D-E8957EC8411B_is1";

/// Probe the on-disk Surge XT install and read whatever version stamp we
/// can recover. Returns `None` when nothing matches — that's the
/// "Surge XT isn't installed" signal.
///
/// Detection layering, in decreasing order of accuracy:
/// 1. **Inno Setup uninstall registry key** (Windows only). The
///    installer writes the exact `NIGHTLY-<YYYY-MM-DD>-<sha>` token
///    into `HKLM\…\Uninstall\<AppId>_is1\DisplayVersion`. This is the
///    primary signal for vendor-installed Surge XT on Windows — it
///    matches the nightly token RABBIT's receipt records bit-for-bit.
/// 2. **VST3 bundle file metadata** (cross-platform). On Windows the
///    inner PE's VS_VERSIONINFO is empty today (`0.0.0.0`), so this
///    path rarely fires there. On macOS the bundle's
///    `Info.plist`/`CFBundleShortVersionString` carries the upstream
///    semver (e.g. `1.3.4`) — accurate-but-not-nightly. Versions read
///    here compare strictly lower than any `NIGHTLY-…` under
///    `Version::cmp_lenient` (1 < 2026), so the planner naturally
///    schedules an Update for pre-existing vendor-installed copies.
fn detect_surge_xt_vendor_files(
    spec: &PackageSpec,
    platform: Platform,
    uninstall_display_version: fn(&str) -> Option<String>,
) -> Option<ComponentDetection> {
    if matches!(platform, Platform::Windows)
        && let Some(raw) = uninstall_display_version(SURGE_XT_INNO_SETUP_UNINSTALL_KEY)
        && let Ok(version) = crate::version::Version::parse(&raw)
    {
        let bundle = surge_xt_system_vst3_bundle(platform);
        return Some(ComponentDetection {
            package_id: spec.id.clone(),
            display_name: spec.display_name.clone(),
            installed: true,
            version: Some(version),
            detector: "surge-xt-inno-uninstall-displayversion".to_string(),
            confidence: Confidence::High,
            files: bundle.map(|path| vec![path]).unwrap_or_default(),
            notes: vec![
                "Version came from the Surge XT Inno Setup installer's Uninstall \
                         registry key, which records the exact nightly build identifier."
                    .to_string(),
            ],
        });
    }
    let bundle = surge_xt_system_vst3_bundle(platform)?;
    let version = surge_xt_version_from_bundle(&bundle)?;
    Some(ComponentDetection {
        package_id: spec.id.clone(),
        display_name: spec.display_name.clone(),
        installed: true,
        version: Some(version),
        detector: "surge-xt-vendor-vst3-bundle".to_string(),
        confidence: Confidence::Medium,
        files: vec![bundle],
        notes: vec![
            "Version came from the Surge XT VST3 bundle's file metadata. RABBIT could not match \
             this install to a receipt or the installer's uninstall registry entry, so the \
             reported version is the upstream semver the build was cut from (e.g. 1.3.4) rather \
             than the nightly token (NIGHTLY-YYYY-MM-DD-sha)."
                .to_string(),
        ],
    })
}

/// Resolve the first existing system VST3 bundle path Surge XT installs
/// to. On Windows that's `<CommonProgramFiles>\VST3\Surge Synth Team\
/// Surge XT.vst3`; on macOS it's `/Library/Audio/Plug-Ins/VST3/Surge
/// XT.vst3`. The bundle is a directory in both VST3-spec layouts since
/// VST 3.6.7 (older flat-file layouts are no longer shipped by Surge XT
/// upstream, but `read_file_version_parts` handles both shapes anyway).
fn surge_xt_system_vst3_bundle(platform: Platform) -> Option<PathBuf> {
    match platform {
        Platform::Windows => rabbit_platform::windows_common_program_files_dirs()
            .into_iter()
            .map(|dir| {
                dir.join("VST3")
                    .join("Surge Synth Team")
                    .join("Surge XT.vst3")
            })
            .find(|path| path.exists()),
        Platform::MacOs => {
            let path = PathBuf::from("/Library/Audio/Plug-Ins/VST3/Surge XT.vst3");
            path.exists().then_some(path)
        }
    }
}

/// Read the version stamp from a Surge XT VST3 bundle. Tries
/// `read_file_version_parts` on the bundle directory first — that path
/// handles macOS `Info.plist`'s `CFBundleShortVersionString` and the
/// Windows VST3-bundle convention where the inner PE under
/// `Contents\<arch>-win\<basename>` carries VERSIONINFO. As a fallback
/// (older flat-VST3 layout, or when the inner binary lives at a
/// non-default sub-path) we probe the inner PE directly.
fn surge_xt_version_from_bundle(bundle: &Path) -> Option<crate::version::Version> {
    if let Some(parts) = rabbit_platform::read_file_version_parts(bundle)
        && let Ok(version) = crate::version::Version::parse(format_version_parts(parts))
    {
        return Some(version);
    }
    for inner in surge_xt_inner_pe_candidates(bundle) {
        if inner.is_file()
            && let Some(parts) = rabbit_platform::read_file_version_parts(&inner)
            && let Ok(version) = crate::version::Version::parse(format_version_parts(parts))
        {
            return Some(version);
        }
    }
    None
}

/// Render a `[u32; 4]` version tuple as a `1.2.3.4` literal, trimming
/// trailing `.0` slots so `[1, 3, 4, 0]` reports as `1.3.4`. Matches the
/// Inno Setup `AppVersion`/`AppVerName` semantics Surge XT uses, where
/// the build number is zero for tagged releases.
fn format_version_parts(parts: [u32; 4]) -> String {
    let mut rendered = format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3]);
    while rendered.ends_with(".0") && rendered.matches('.').count() > 1 {
        rendered.truncate(rendered.len() - 2);
    }
    rendered
}

/// Candidate paths for the inner PE binary inside a Windows VST3 bundle.
/// The VST3 spec puts it at `Contents\<arch>-win\<bundle-basename>` —
/// `x86_64-win` for Surge XT's only Windows installer flavour today.
/// `x86-win` covers a hypothetical future return of a 32-bit native
/// build (the nightly channel dropped Win32 setup.exes after Surge XT
/// 1.3.4 stable).
fn surge_xt_inner_pe_candidates(bundle: &Path) -> Vec<PathBuf> {
    let basename = bundle
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Surge XT.vst3");
    vec![
        bundle.join("Contents").join("x86_64-win").join(basename),
        bundle.join("Contents").join("x86-win").join(basename),
    ]
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        DiscoveryOptions, default_standard_installation, detect_components, detect_ffmpeg_version,
        discover_installations, ffmpeg_major_from_libavformat_major,
        ffmpeg_version_from_product_version_string, libavformat_major_from_basename,
        package_scan_dir, scan_text_version,
    };
    use regex::Regex;

    /// The binary-version-string regexes from the manifest, mirrored as
    /// literals so the scanner tests don't depend on the embedded JSON.
    const OSARA_SNAPSHOT_PATTERN: &str = r"(20[0-9]{2}(?:\.[0-9]+){2,})";
    const SWS_PATTERN: &str = r"([0-9]+(?:\.[0-9]+){2,}) #[0-9a-fA-F]{6,}";
    const REAPACK_PATTERN: &str = r"ReaPack(?:/| v)([0-9]+(?:\.[0-9]+)+)";

    fn scan(pattern: &str, text: &str) -> Option<crate::version::Version> {
        scan_text_version(text, &Regex::new(pattern).unwrap())
    }
    use crate::model::Platform;
    use crate::package::{
        PACKAGE_APP2CLAP, PACKAGE_FFMPEG, PACKAGE_OSARA, PACKAGE_REAKONTROL, PACKAGE_REAPACK,
        PACKAGE_SURGE_XT, PACKAGE_SWS, package_specs_by_id,
    };

    #[test]
    fn package_scan_dir_follows_install_destination() {
        let resource = std::path::Path::new("/some/reaper");
        let specs = package_specs_by_id(Platform::Windows);
        // ReaKontrol installs to UserPlugins → scanned under the resource path.
        let reakontrol = specs.get(PACKAGE_REAKONTROL).unwrap();
        assert_eq!(
            package_scan_dir(resource, reakontrol),
            Some(resource.join("UserPlugins"))
        );
        // app2clap installs to the per-user CLAP folder → scanned there, NOT
        // under the resource path. (windows_clap_dir resolves only on Windows.)
        let app2clap = specs.get(PACKAGE_APP2CLAP).unwrap();
        #[cfg(target_os = "windows")]
        {
            assert_eq!(
                package_scan_dir(resource, app2clap),
                rabbit_platform::windows_clap_dir()
            );
            assert_ne!(
                package_scan_dir(resource, app2clap),
                Some(resource.join("UserPlugins"))
            );
        }
        #[cfg(not(target_os = "windows"))]
        {
            // No CLAP folder off-Windows → nothing to scan.
            assert_eq!(package_scan_dir(resource, app2clap), None);
        }
    }

    #[test]
    fn detects_extensions_by_user_plugin_prefix() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(plugins.join("reaper_osara64.dll"), b"").unwrap();
        fs::write(plugins.join("reaper_sws-x64.dll"), b"").unwrap();
        fs::write(plugins.join("reaper_reapack-x64.dll"), b"").unwrap();

        let detections = detect_components(dir.path(), Platform::Windows).unwrap();
        let installed: Vec<_> = detections
            .iter()
            .filter(|detection| detection.installed)
            .map(|detection| detection.package_id.as_str())
            .collect();

        assert!(installed.contains(&PACKAGE_OSARA));
        assert!(installed.contains(&PACKAGE_SWS));
        assert!(installed.contains(&PACKAGE_REAPACK));
    }

    #[test]
    fn ffmpeg_libavformat_filenames_round_trip_to_major_only_versions() {
        assert_eq!(libavformat_major_from_basename("avformat-62.dll"), Some(62));
        assert_eq!(
            libavformat_major_from_basename("AVFORMAT-62.DLL"),
            Some(62),
            "case-insensitive on Windows"
        );
        assert_eq!(
            libavformat_major_from_basename("libavformat.62.dylib"),
            Some(62),
            "macOS dylib layout"
        );
        assert_eq!(libavformat_major_from_basename("avformat.dll"), None);
        assert_eq!(libavformat_major_from_basename("avcodec-62.dll"), None);
        assert_eq!(libavformat_major_from_basename("avformat-foo.dll"), None);

        assert_eq!(ffmpeg_major_from_libavformat_major(58), Some(4));
        assert_eq!(ffmpeg_major_from_libavformat_major(62), Some(8));
        assert_eq!(ffmpeg_major_from_libavformat_major(53), None);
    }

    #[test]
    fn ffmpeg_product_version_parser_extracts_release_string() {
        // Plain stable: Gyan.dev's `ProductVersion`.
        assert_eq!(
            ffmpeg_version_from_product_version_string("8.1.1")
                .unwrap()
                .raw(),
            "8.1.1"
        );
        // BtbN/tordona-style with `n` prefix.
        assert_eq!(
            ffmpeg_version_from_product_version_string("n8.1.1")
                .unwrap()
                .raw(),
            "8.1.1"
        );
        // tordona-style autobuild trailer — we drop everything after
        // the version run.
        assert_eq!(
            ffmpeg_version_from_product_version_string("n8.1.1-6-gdeadbeef-tordona-ffmpeg")
                .unwrap()
                .raw(),
            "8.1.1"
        );
        // Older 4-component style.
        assert_eq!(
            ffmpeg_version_from_product_version_string("v8.1.1.0")
                .unwrap()
                .raw(),
            "8.1.1.0"
        );
        // Whitespace.
        assert_eq!(
            ffmpeg_version_from_product_version_string("  8.0  ")
                .unwrap()
                .raw(),
            "8.0"
        );
        // No version-shaped prefix.
        assert!(ffmpeg_version_from_product_version_string("git-master").is_none());
        assert!(ffmpeg_version_from_product_version_string("").is_none());
    }

    #[test]
    fn ffmpeg_version_falls_back_to_libavformat_filename_when_exe_has_no_versioninfo() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        let avformat = plugins.join("avformat-62.dll");
        let avcodec = plugins.join("avcodec-62.dll");
        // A stub ffmpeg.exe with no PE resource section — file_version
        // returns Ok(None) so the helper drops to the libavformat
        // fallback.
        let ffmpeg_exe = plugins.join("ffmpeg.exe");
        fs::write(&avformat, b"").unwrap();
        fs::write(&avcodec, b"").unwrap();
        fs::write(&ffmpeg_exe, b"").unwrap();

        let (version, detector, confidence, _notes) =
            detect_ffmpeg_version(&[avcodec.clone(), avformat.clone()]).unwrap();
        assert_eq!(version.raw(), "8.0.0");
        assert_eq!(detector, "ffmpeg-libavformat-major");
        assert_eq!(confidence, super::Confidence::Medium);

        // Without an avformat file in the list, neither probe applies
        // — we don't synthesize from avcodec / avutil because their
        // major-bump cadence isn't perfectly synchronized to FFmpeg's,
        // and the stubbed ffmpeg.exe has no readable VS_FIXEDFILEINFO.
        assert!(detect_ffmpeg_version(&[avcodec]).is_none());
    }

    #[test]
    fn parses_ffmpeg_version_from_show_banner_format_string() {
        // FFmpeg's `show_banner` literal is one contiguous string in
        // .rdata; embed the same shape in a fake "binary" and verify
        // the scanner extracts the version.
        let mut bytes = vec![0u8; 1024];
        bytes.extend_from_slice(
            b"\0\0\0%s version 8.1.1, Copyright (c) %d-%d the FFmpeg developers\n\0",
        );
        bytes.extend(std::iter::repeat_n(0u8, 1024));
        let version = super::ffmpeg_version_from_binary_bytes(&bytes).unwrap();
        assert_eq!(version.raw(), "8.1.1");
    }

    #[test]
    fn parses_ffmpeg_snapshot_version_from_show_banner() {
        // tordona / BtbN snapshot builds substitute `n8.1.1-6-gdeadbeef`
        // (or similar) into FFMPEG_VERSION. The scanner strips the `n`
        // and keeps the leading digit-or-dot run.
        let mut bytes = vec![0u8; 256];
        bytes.extend_from_slice(
            b"%s version n8.1.1-6-gdeadbeef, Copyright (c) %d-%d the FFmpeg developers\n",
        );
        let version = super::ffmpeg_version_from_binary_bytes(&bytes).unwrap();
        assert_eq!(version.raw(), "8.1.1");
    }

    #[test]
    fn parses_ffmpeg_gyan_full_build_banner_without_comma() {
        // Gyan.dev's full builds patch the banner format to drop the
        // comma between the version and `Copyright` and pad the
        // version with trailing spaces. Real captured shape from a
        // 2026 Gyan release ffmpeg.exe (the `n` prefix is part of the
        // version suffix, not a snapshot marker):
        //   `%s version 8.1.1-full_build-www.gyan.dev         Copyright (c) %d-%d the FFmpeg developers`
        let mut bytes = vec![0u8; 256];
        bytes.extend_from_slice(
            b"%s version 8.1.1-full_build-www.gyan.dev         Copyright (c) %d-%d the FFmpeg developers \n",
        );
        let version = super::ffmpeg_version_from_binary_bytes(&bytes).unwrap();
        assert_eq!(version.raw(), "8.1.1");
    }

    #[test]
    fn binary_scan_returns_none_without_anchor() {
        // A non-FFmpeg binary won't have the distinctive
        // `the FFmpeg developers` anchor; falling through here lets the
        // caller drop to Probe 3 (libavformat filename heuristic).
        let bytes = b"random binary contents with version 9.9.9 inside but no anchor";
        assert!(super::ffmpeg_version_from_binary_bytes(bytes).is_none());
    }

    #[test]
    fn detects_externally_installed_ffmpeg_via_avformat_filename() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        // Drop an avformat-62.dll from a hypothetical external FFmpeg 8
        // install. RABBIT shouldn't have a receipt for it, so detection
        // must fall back to the libavformat-major heuristic.
        fs::write(plugins.join("avformat-62.dll"), b"").unwrap();
        fs::write(plugins.join("avcodec-62.dll"), b"").unwrap();

        let detections = detect_components(dir.path(), Platform::Windows).unwrap();
        let ffmpeg = detections
            .iter()
            .find(|detection| detection.package_id == PACKAGE_FFMPEG)
            .expect("ffmpeg row missing from Windows detections");
        assert!(ffmpeg.installed);
        let version = ffmpeg.version.as_ref().expect("version not detected");
        assert_eq!(version.raw(), "8.0.0");
        assert_eq!(ffmpeg.detector, "ffmpeg-libavformat-major");
    }

    #[test]
    fn detects_windows_portable_installation_from_selected_folder() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("reaper.exe"), b"").unwrap();
        fs::write(dir.path().join("reaper.ini"), b"").unwrap();

        let installations = discover_installations(&DiscoveryOptions {
            include_standard: false,
            portable_roots: vec![dir.path().to_path_buf()],
        })
        .unwrap();

        if cfg!(target_os = "windows") {
            assert_eq!(installations.len(), 1);
        } else {
            assert!(installations.is_empty());
        }
    }

    #[test]
    fn colocated_portable_root_detects_portable_reaper_next_to_executable() {
        let dir = tempdir().unwrap();
        // Lay down the Windows portable signature (reaper.exe + reaper.ini)
        // and a macOS REAPER app bundle so the assertion holds on both hosts.
        fs::write(dir.path().join("reaper.exe"), b"").unwrap();
        fs::write(dir.path().join("reaper.ini"), b"").unwrap();
        fs::create_dir_all(dir.path().join("REAPER.app")).unwrap();

        if Platform::current().is_some() {
            assert_eq!(
                super::portable_root_in_dir(dir.path()),
                Some(dir.path().to_path_buf())
            );
        }
    }

    #[test]
    fn colocated_portable_root_is_none_without_portable_reaper() {
        let dir = tempdir().unwrap();
        assert!(super::portable_root_in_dir(dir.path()).is_none());
    }

    #[test]
    fn parses_osara_snapshot_version_from_binary_text() {
        let version = scan(OSARA_SNAPSHOT_PATTERN, "OSARA 2024.3.6.1332,13560ef7").unwrap();
        assert_eq!(version.raw(), "2024.3.6.1332");
    }

    #[test]
    fn parses_reakontrol_snapshot_version_from_binary_text() {
        let version = scan(OSARA_SNAPSHOT_PATTERN, "reaKontrol 2026.2.16.100,abcdef0").unwrap();
        assert_eq!(version.raw(), "2026.2.16.100");
    }

    #[test]
    fn detects_reakontrol_version_by_binary_scan_when_metadata_is_unavailable() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(
            plugins.join("reaper_kontrol_mk2.dll"),
            b"reaKontrol\0snapshot\0 2026.2.16.100,abcdef0\0",
        )
        .unwrap();

        let detections =
            super::detect_components_with_probes(dir.path(), Platform::Windows, |_| None).unwrap();
        let reakontrol = detections
            .iter()
            .find(|detection| detection.package_id == PACKAGE_REAKONTROL)
            .unwrap();

        assert_eq!(reakontrol.version.as_ref().unwrap().raw(), "2026.2.16.100");
        assert_eq!(reakontrol.detector, "reakontrol-binary-version-string");
    }

    #[test]
    fn detects_osara_version_by_binary_scan_when_metadata_is_unavailable() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(
            plugins.join("reaper_osara64.dll"),
            b"OSARA\0snapshot\0 2024.3.6.1332,13560ef7\0",
        )
        .unwrap();

        // Inject a no-op uninstall-registry probe so the test does not pick up
        // any OSARA install that happens to be present on the dev/CI host —
        // the binary-scan fallback is what we are exercising here.
        let detections =
            super::detect_components_with_probes(dir.path(), Platform::Windows, |_| None).unwrap();
        let osara = detections
            .iter()
            .find(|detection| detection.package_id == PACKAGE_OSARA)
            .unwrap();

        assert_eq!(osara.version.as_ref().unwrap().raw(), "2024.3.6.1332");
        assert_eq!(osara.detector, "osara-binary-version-string");
    }

    #[test]
    fn parses_sws_version_with_commit_hash() {
        let version = scan(SWS_PATTERN, "SWS Extension v2.14.0.1 #2dadf4b\0").unwrap();
        assert_eq!(version.raw(), "2.14.0.1");
    }

    #[test]
    fn parses_sws_three_component_version_with_commit_hash() {
        let version = scan(SWS_PATTERN, "v2.14.0 #abcdef0\0").unwrap();
        assert_eq!(version.raw(), "2.14.0");
    }

    #[test]
    fn rejects_sws_version_pattern_without_commit_hash() {
        assert!(scan(SWS_PATTERN, "plain 1.2.3 with no anchor").is_none());
    }

    #[test]
    fn detects_sws_version_by_binary_scan_when_metadata_is_unavailable() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(
            plugins.join("reaper_sws-x64.dll"),
            b"SWS Extension\0v2.14.0.1 #2dadf4b\0",
        )
        .unwrap();

        let detections =
            super::detect_components_with_probes(dir.path(), Platform::Windows, |_| None).unwrap();
        let sws = detections
            .iter()
            .find(|detection| detection.package_id == PACKAGE_SWS)
            .unwrap();

        assert_eq!(sws.version.as_ref().unwrap().raw(), "2.14.0.1");
        assert_eq!(sws.detector, "sws-binary-version-string");
    }

    #[test]
    fn detects_surge_xt_via_inno_setup_uninstall_registry() {
        let dir = tempdir().unwrap();

        // Surge XT lives outside <resource>/UserPlugins entirely, so the
        // resource path itself doesn't need anything special — what
        // matters is that the registry callback returns the exact
        // NIGHTLY token under Surge XT's Inno Setup `_is1` key.
        let stub = |key: &str| {
            if key == "69F3FE96-DEEC-4C7C-B72D-E8957EC8411B_is1" {
                Some("NIGHTLY-2024-01-15-d9f42fb".to_string())
            } else {
                None
            }
        };

        let detections =
            super::detect_components_with_probes(dir.path(), Platform::Windows, stub).unwrap();
        let surge = detections
            .iter()
            .find(|detection| detection.package_id == PACKAGE_SURGE_XT)
            .expect("Surge XT row should appear in the detection set");

        assert!(surge.installed, "Surge XT must be reported as installed");
        assert_eq!(
            surge.version.as_ref().unwrap().raw(),
            "NIGHTLY-2024-01-15-d9f42fb"
        );
        assert_eq!(surge.detector, "surge-xt-inno-uninstall-displayversion");
    }

    #[test]
    fn formats_surge_xt_version_parts_trimming_trailing_zero() {
        assert_eq!(super::format_version_parts([1, 3, 4, 0]), "1.3.4");
        assert_eq!(super::format_version_parts([1, 3, 4, 7]), "1.3.4.7");
        assert_eq!(super::format_version_parts([1, 3, 0, 0]), "1.3");
        // Never trim below "major.minor" — `1.0.0.0` must report as `1.0`,
        // not `1`, so the result still parses as a `Version` (which
        // requires at least one digit but stays readable as a semver).
        assert_eq!(super::format_version_parts([1, 0, 0, 0]), "1.0");
    }

    #[test]
    fn inner_surge_xt_pe_candidates_cover_both_architectures() {
        let bundle = std::path::PathBuf::from("/tmp/Surge XT.vst3");
        let candidates = super::surge_xt_inner_pe_candidates(&bundle);
        let expected = [
            bundle
                .join("Contents")
                .join("x86_64-win")
                .join("Surge XT.vst3"),
            bundle
                .join("Contents")
                .join("x86-win")
                .join("Surge XT.vst3"),
        ];
        assert_eq!(candidates, expected);
    }

    #[test]
    fn parses_reapack_version_from_user_agent() {
        let version = scan(
            REAPACK_PATTERN,
            "Mozilla/5.0 ReaPack/1.2.6 (Cockos REAPER)\0",
        )
        .unwrap();
        assert_eq!(version.raw(), "1.2.6");
    }

    #[test]
    fn parses_reapack_version_from_legacy_about_form() {
        let version = scan(REAPACK_PATTERN, "\0ReaPack v1.2.6\0").unwrap();
        assert_eq!(version.raw(), "1.2.6");
    }

    #[test]
    fn rejects_reapack_version_without_anchor() {
        assert!(scan(REAPACK_PATTERN, "just 1.2.6 by itself").is_none());
    }

    #[test]
    fn detects_reapack_version_by_binary_scan_when_metadata_is_unavailable() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(
            plugins.join("reaper_reapack-x64.dll"),
            b"User-Agent: ReaPack/1.2.6 (REAPER)\0",
        )
        .unwrap();

        let detections =
            super::detect_components_with_probes(dir.path(), Platform::Windows, |_| None).unwrap();
        let reapack = detections
            .iter()
            .find(|detection| detection.package_id == PACKAGE_REAPACK)
            .unwrap();

        assert_eq!(reapack.version.as_ref().unwrap().raw(), "1.2.6");
        assert_eq!(reapack.detector, "reapack-binary-version-string");
    }

    #[test]
    fn exposes_default_standard_installation_target() {
        let Some(platform) = Platform::current() else {
            return;
        };

        let installation = default_standard_installation(platform).unwrap();

        assert_eq!(installation.kind, crate::model::InstallationKind::Standard);
        assert_eq!(installation.platform, platform);
        match platform {
            Platform::Windows => {
                assert!(installation.resource_path.ends_with("REAPER"));
                assert!(installation.app_path.ends_with("reaper.exe"));
            }
            Platform::MacOs => {
                assert!(installation.resource_path.ends_with("REAPER"));
                assert!(installation.app_path.ends_with("REAPER.app"));
            }
        }
    }
}
