use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::arch_probe::probe_executable_architecture;
use crate::error::{IoPathContext, RabbitError, Result};
use crate::metadata::file_version;
use crate::model::{
    ComponentDetection, Confidence, Evidence, Installation, InstallationKind, LeftToReapack,
    Platform,
};
use crate::package::{
    InstallDestination, PACKAGE_REAPACK, PackageDetector, PackageSpec, builtin_package_specs,
    package_specs_by_id,
};
use crate::reapack::{
    package_owner_for_file, reapack_counts_as_installed, reapack_package_with_file_in,
};
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
    let specs = builtin_package_specs(platform);
    let mut detections = Vec::new();

    for spec in &specs {
        detections.push(detect_component_with_probes(
            resource_path,
            platform,
            spec,
            state.as_ref(),
            uninstall_display_version,
        )?);
    }

    // ReaPack's records only have their hold while ReaPack is installed, so
    // this waits until ReaPack's own detection is in.
    if reapack_counts_as_installed(&detections) {
        for (spec, detection) in specs.iter().zip(&mut detections) {
            let left = left_to_reapack(resource_path, spec);
            // ReaPack installed over the copy RABBIT had, so its files no
            // longer match RABBIT's receipt: ReaPack's records, not the
            // receipt, say what is there now. A copy that still matches the
            // receipt is the one RABBIT installed, and the receipt has its
            // version right.
            if matches!(left, Some(LeftToReapack::Managed { .. }))
                && detection.detector == "rabbit-receipt-mismatch"
            {
                *detection = detect_component_with_probes(
                    resource_path,
                    platform,
                    spec,
                    None,
                    uninstall_display_version,
                )?;
            }
            detection.left_to_reapack = left;
        }
    }

    Ok(detections)
}

/// The packages among `package_ids` whose copies in `resource_path` RABBIT
/// leaves to ReaPack right now, each with why, for a command that installs
/// outside the install pipeline, such as `rabbit install-extension`. Only
/// ReaPack itself is detected for it, and only when a named package's
/// manifest says `leave_to_reapack`. A detection of ReaPack that fails
/// counts it as installed, so the copy is left alone then too.
pub fn packages_left_to_reapack(
    resource_path: &Path,
    platform: Platform,
    package_ids: &[String],
) -> Vec<(String, LeftToReapack)> {
    let specs = package_specs_by_id(platform);
    let asked: Vec<&PackageSpec> = package_ids
        .iter()
        .filter_map(|id| specs.get(id))
        .filter(|spec| spec.leave_to_reapack)
        .collect();
    if asked.is_empty() {
        return Vec::new();
    }
    let reapack: Vec<ComponentDetection> = specs
        .get(PACKAGE_REAPACK)
        .and_then(|spec| {
            let state = load_install_state(resource_path).ok()?;
            detect_component(resource_path, platform, spec, state.as_ref()).ok()
        })
        .into_iter()
        .collect();
    if !reapack_counts_as_installed(&reapack) {
        return Vec::new();
    }
    asked
        .into_iter()
        .filter_map(|spec| Some((spec.id.clone(), left_to_reapack(resource_path, spec)?)))
        .collect()
}

/// Why RABBIT leaves `spec`'s copy in `resource_path` to ReaPack, for a
/// package whose manifest says `leave_to_reapack`: ReaPack's records name a
/// file in its install folder that matches its plug-in names, or RABBIT
/// couldn't read them. A file the records name counts even when it is gone
/// from disk, since ReaPack still thinks it installed it. The caller checks
/// that ReaPack is installed.
pub(crate) fn left_to_reapack(resource_path: &Path, spec: &PackageSpec) -> Option<LeftToReapack> {
    if !spec.leave_to_reapack {
        return None;
    }
    // ReaPack only installs inside the resource folder, and its records
    // name each file by its path there with `/` between the folders.
    let folder = spec.install_destination.resource_components()?.join("/");
    match reapack_package_with_file_in(resource_path, &folder, |name| {
        user_plugin_name_matches(spec, name)
    }) {
        Ok(Some(reapack_package)) => Some(LeftToReapack::Managed { reapack_package }),
        Ok(None) => None,
        Err(error) => Some(LeftToReapack::RecordsUnreadable {
            error: error.to_string(),
        }),
    }
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
    detect_component_seeing(
        resource_path,
        platform,
        spec,
        state,
        uninstall_display_version,
        &|destination| former_outside_folder(platform, resource_path, destination),
    )
}

/// [`detect_component_with_probes`], with `former_outside` giving the
/// folder outside the resource folder that a former install destination
/// names for this REAPER, if it counts here (see [`former_scan_dirs`]).
/// Tests give a temporary one, so they never look in the real CLAP folder.
pub(crate) fn detect_component_seeing(
    resource_path: &Path,
    platform: Platform,
    spec: &PackageSpec,
    state: Option<&crate::receipt::InstallState>,
    uninstall_display_version: fn(&str) -> Option<String>,
    former_outside: &dyn Fn(InstallDestination) -> Option<PathBuf>,
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
                left_to_reapack: None,
            });
        }
        ReceiptVerification::Mismatch(receipt) => {
            let files = detected_user_plugin_files(resource_path, platform, spec, former_outside)?;
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
                    left_to_reapack: None,
                });
            }
        }
        ReceiptVerification::MissingReceipt | ReceiptVerification::MissingPackage => {}
    }

    let files = detected_user_plugin_files(resource_path, platform, spec, former_outside)?;

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
        left_to_reapack: None,
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
        PackageDetector::ReaperApp => detect_reaper_app(spec, platform, resource_path),
    };
    Ok(detection)
}

/// REAPER for the folder being examined: the portable install `resource_path`
/// is the root of, or the standard install whose resource folder it is. The
/// version comes from the executable, so this works whether or not RABBIT
/// installed it and whatever REAPER has done to its own configuration since.
fn detect_reaper_app(
    spec: &PackageSpec,
    platform: Platform,
    resource_path: &Path,
) -> Option<ComponentDetection> {
    let installation = discover_portable_installation(platform, resource_path).or_else(|| {
        default_standard_installation(platform)
            .filter(|standard| standard.resource_path == resource_path)
            .filter(|standard| standard.app_path.exists())
    })?;
    let files = vec![installation.app_path.clone()];
    Some(match installation.version {
        Some(version) => version_component(
            spec,
            version,
            "reaper-app",
            Confidence::High,
            Vec::new(),
            &files,
        ),
        // The app is there but would not say which version it is: still
        // installed, and saying so beats reporting it missing.
        None => ComponentDetection {
            package_id: spec.id.clone(),
            display_name: spec.display_name.clone(),
            installed: true,
            version: None,
            detector: "reaper-app".to_string(),
            confidence: Confidence::Medium,
            files,
            notes: vec!["Found REAPER, but it reported no version of its own.".to_string()],
            left_to_reapack: None,
        },
    })
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
        left_to_reapack: None,
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

/// The directory a package's plug-in file(s) live in: the folder its
/// `install_destination` names, such as `<resource>/UserPlugins` (the
/// default), `<resource>/UserPlugins/FX` or `<resource>/LangPack`. Returns
/// `None` when the destination can't be resolved (e.g. the CLAP folder on a
/// non-Windows host), in which case detection finds no files.
fn package_scan_dir(resource_path: &Path, spec: &PackageSpec) -> Option<PathBuf> {
    let destination = spec.install_destination;
    destination
        .folder_in(resource_path)
        .or_else(|| destination.outside_folder())
}

/// The folders `spec` installed into before it moved to its install folder
/// (its `former_install_destinations`), where a copy installed back then
/// still counts as the package. One inside the resource folder counts for
/// every REAPER. One outside it, such as the CLAP folder, is shared by
/// every REAPER on the computer, and only a standard REAPER's receipt ever
/// recorded a copy there, so `outside` gives it only for the standard
/// REAPER (see [`former_outside_folder`]): a portable REAPER offers the
/// package as a new install rather than ticking an update of a copy that
/// isn't its own, and the wizard says the shared copy is there (see
/// [`copies_in_former_outside_folders`]).
fn former_scan_dirs(
    resource_path: &Path,
    spec: &PackageSpec,
    outside: &dyn Fn(InstallDestination) -> Option<PathBuf>,
) -> Vec<PathBuf> {
    spec.former_install_destinations
        .iter()
        .filter_map(|destination| {
            destination
                .folder_in(resource_path)
                .or_else(|| outside(*destination))
        })
        .collect()
}

/// The folder outside every resource folder that `destination` names on
/// this computer, when `resource_path` is the standard REAPER's resource
/// folder, the only one a former outside install counts for (see
/// [`former_scan_dirs`]).
pub(crate) fn former_outside_folder(
    platform: Platform,
    resource_path: &Path,
    destination: InstallDestination,
) -> Option<PathBuf> {
    let folder = destination.outside_folder()?;
    (crate::uninstall::execute::target_kind_of(platform, resource_path)
        == InstallationKind::Standard)
        .then_some(folder)
}

/// `spec`'s copies in the folders it installed into before, as detection
/// counts them for the REAPER at `resource_path` (see
/// [`former_scan_dirs`]): the files there whose names match its plug-in
/// names. An update moves each one no receipt lists into its backup set,
/// so REAPER doesn't load it beside the new version. A folder that can't
/// be read gives none, and the update goes on without it.
pub(crate) fn former_copies(
    resource_path: &Path,
    spec: &PackageSpec,
    outside: &dyn Fn(InstallDestination) -> Option<PathBuf>,
) -> Vec<PathBuf> {
    former_scan_dirs(resource_path, spec, outside)
        .iter()
        .flat_map(|folder| user_plugin_files_in(folder, spec).unwrap_or_default())
        .collect()
}

/// `spec`'s copies in a folder outside every resource folder that it
/// installed into before, such as an app2clap an earlier RABBIT put in the
/// CLAP folder: the files there whose names match its plug-in names. Every
/// REAPER on the computer loads what is in that folder, portable ones too,
/// and so does every other program that reads it, whichever REAPER
/// detection counts the copy for. Empty when there are none, or the folder
/// can't be read.
pub fn copies_in_former_outside_folders(spec: &PackageSpec) -> Vec<PathBuf> {
    former_outside_copies_seeing(spec, &InstallDestination::outside_folder)
}

/// [`copies_in_former_outside_folders`], with `outside` giving the folder
/// a destination names. Tests give a temporary one.
fn former_outside_copies_seeing(
    spec: &PackageSpec,
    outside: &dyn Fn(InstallDestination) -> Option<PathBuf>,
) -> Vec<PathBuf> {
    spec.former_install_destinations
        .iter()
        .filter(|destination| destination.resource_components().is_none())
        .filter_map(|destination| outside(*destination))
        .flat_map(|folder| user_plugin_files_in(&folder, spec).unwrap_or_default())
        .collect()
}

/// The files detection counts as `spec`'s when no matching receipt says
/// which they are: those in its install folder whose names match its
/// plug-in names (see [`matching_user_plugin_files`]), then those in each
/// folder it installed into before (see [`former_scan_dirs`]).
fn detected_user_plugin_files(
    resource_path: &Path,
    platform: Platform,
    spec: &PackageSpec,
    former_outside: &dyn Fn(InstallDestination) -> Option<PathBuf>,
) -> Result<Vec<PathBuf>> {
    let mut files = matching_user_plugin_files(resource_path, platform, spec)?;
    for folder in former_scan_dirs(resource_path, spec, former_outside) {
        files.extend(user_plugin_files_in(&folder, spec)?);
    }
    Ok(files)
}

/// The files in `spec`'s install folder whose names match its plug-in
/// names. Only that folder: the files RABBIT records for a package a
/// vendor installer put in place come from here.
pub(crate) fn matching_user_plugin_files(
    resource_path: &Path,
    _platform: Platform,
    spec: &PackageSpec,
) -> Result<Vec<PathBuf>> {
    let Some(scan_dir) = package_scan_dir(resource_path, spec) else {
        return Ok(Vec::new());
    };
    user_plugin_files_in(&scan_dir, spec)
}

/// The files directly in `scan_dir` whose names match `spec`'s plug-in
/// names, sorted. Other files and folders there are left out, as the
/// folder may hold other plug-ins too.
fn user_plugin_files_in(scan_dir: &Path, spec: &PackageSpec) -> Result<Vec<PathBuf>> {
    if !scan_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(scan_dir).with_path(scan_dir)? {
        let entry = entry.with_path(scan_dir)?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if user_plugin_name_matches(spec, file_name) {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Whether `file_name` starts with one of the package's
/// `user_plugin_prefixes` and ends with one of its `user_plugin_suffixes`,
/// ignoring ASCII case. Shared with the uninstall planner, so a file
/// detection counts as the package's is exactly a file uninstall may remove.
pub(crate) fn user_plugin_name_matches(spec: &PackageSpec, file_name: &str) -> bool {
    let lower_name = file_name.to_ascii_lowercase();
    let prefix_matches = spec
        .user_plugin_prefixes
        .iter()
        .any(|prefix| lower_name.starts_with(&prefix.to_ascii_lowercase()));
    let suffix_matches = spec
        .user_plugin_suffixes
        .iter()
        .any(|suffix| lower_name.ends_with(&suffix.to_ascii_lowercase()));
    prefix_matches && suffix_matches
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

/// The REAPER app bundle in a portable macOS folder: an `.app` directly in
/// `root` whose name contains "reaper", ignoring case. With more than one,
/// the choice is the same every time, so Review and the run that follows
/// name the same app: `REAPER.app` first, then `REAPER64.app` and
/// `REAPER-ARM.app`, the order a standard install is looked for in, then
/// any other by name. The wizard's custom portable target uses it as well,
/// so the GUI and `rabbit uninstall` name the same app in one folder.
pub fn portable_macos_app_bundle(root: &Path) -> Option<PathBuf> {
    const PREFERRED: [&str; 3] = ["reaper.app", "reaper64.app", "reaper-arm.app"];
    let mut bundles: Vec<(usize, PathBuf)> = fs::read_dir(root)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_ascii_lowercase();
            let is_app = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("app"));
            (is_app && name.contains("reaper")).then(|| {
                let rank = PREFERRED
                    .iter()
                    .position(|preferred| *preferred == name)
                    .unwrap_or(PREFERRED.len());
                (rank, path)
            })
        })
        .collect();
    bundles.sort();
    bundles.into_iter().next().map(|(_, path)| path)
}

fn discover_portable_macos(root: &Path) -> Option<Installation> {
    let app_path = portable_macos_app_bundle(root)?;

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
        left_to_reapack: None,
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
            left_to_reapack: None,
        });
    }
    surge_xt_bundle_detection(spec, platform, surge_xt_system_vst3_bundle(platform)?)
}

/// Surge XT as its system VST3 `bundle`, which exists, shows it.
///
/// On a Mac a bundle whose version can't be read still counts as
/// installed. The version is only in its `Contents/Info.plist`, and the
/// macOS uninstall deletes this VST3 last, with `rm -R`, which unlinks
/// every file it can before it fails on a folder: a removal stopped there
/// may have taken the plist with it. The bundle is still Surge's, so it
/// stays detected and the uninstall is offered again, which is why the
/// plan removes it last. On Windows nothing changes: no version, no
/// detection.
fn surge_xt_bundle_detection(
    spec: &PackageSpec,
    platform: Platform,
    bundle: PathBuf,
) -> Option<ComponentDetection> {
    let Some(version) = surge_xt_version_from_bundle(&bundle) else {
        if !matches!(platform, Platform::MacOs) {
            return None;
        }
        return Some(ComponentDetection {
            package_id: spec.id.clone(),
            display_name: spec.display_name.clone(),
            installed: true,
            version: None,
            detector: "surge-xt-vendor-vst3-bundle-unversioned".to_string(),
            confidence: Confidence::Low,
            files: vec![bundle],
            notes: vec![
                "The Surge XT VST3 bundle is there, but RABBIT couldn't read its version from \
                 it. It may be partly removed."
                    .to_string(),
            ],
            left_to_reapack: None,
        });
    };
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
        left_to_reapack: None,
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
    const OSARA_PULL_REQUEST_PATTERN: &str = r"(pr[0-9]+-[0-9]+,[0-9a-fA-F]+)";
    const SWS_PATTERN: &str = r"([0-9]+(?:\.[0-9]+){2,}) #[0-9a-fA-F]{6,}";
    const REAPACK_PATTERN: &str = r"ReaPack(?:/| v)([0-9]+(?:\.[0-9]+)+)";

    fn scan(pattern: &str, text: &str) -> Option<crate::version::Version> {
        scan_text_version(text, &Regex::new(pattern).unwrap())
    }
    use crate::model::Platform;
    use crate::package::{
        PACKAGE_APP2CLAP, PACKAGE_FFMPEG, PACKAGE_LREAPARAMS, PACKAGE_OSARA, PACKAGE_REAKONTROL,
        PACKAGE_REAPACK, PACKAGE_SURGE_XT, PACKAGE_SWS, package_specs_by_id,
    };

    /// REAPER puts nothing in UserPlugins, so before it had a detector of
    /// its own it was only ever as visible as its receipt: a portable folder
    /// RABBIT had not installed, or whose receipt no longer matched, read as
    /// "REAPER is not installed" and got reinstalled on the next run.
    #[test]
    #[cfg(target_os = "windows")]
    fn a_portable_reaper_is_detected_without_any_receipt() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("reaper.exe"), b"not a real executable").unwrap();
        fs::write(
            root.join("reaper.ini"),
            b"[REAPER]
",
        )
        .unwrap();

        let detections = detect_components(root, Platform::Windows).unwrap();
        let reaper = detections
            .iter()
            .find(|detection| detection.package_id == crate::package::PACKAGE_REAPER)
            .expect("REAPER is in the manifest");
        assert!(reaper.installed, "{reaper:?}");
        assert_eq!(reaper.detector, "reaper-app");
        assert_eq!(reaper.files, vec![root.join("reaper.exe")]);

        // An empty folder still reports REAPER as missing.
        let empty = tempdir().unwrap();
        let detections = detect_components(empty.path(), Platform::Windows).unwrap();
        let reaper = detections
            .iter()
            .find(|detection| detection.package_id == crate::package::PACKAGE_REAPER)
            .unwrap();
        assert!(!reaper.installed, "{reaper:?}");
    }

    /// A receipt from RABBIT 0.4.2 or earlier still records reaper.ini's
    /// size. REAPER rewrites that file whenever it runs, so its size
    /// changes, and the receipt must still count: a used portable REAPER
    /// stays installed with the receipt's version, or `rabbit setup` would
    /// reinstall it on every run.
    #[test]
    fn an_older_receipt_with_the_size_of_reaper_ini_still_finds_reaper() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("reaper.exe"), b"reaper binary").unwrap();
        fs::write(root.join("reaper.ini"), b"[REAPER]\r\n").unwrap();
        // As 0.4.2 wrote it: both files with their sizes then.
        let state: crate::receipt::InstallState = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "packages": {
                    "reaper": {
                        "id": "reaper",
                        "version": "7.80",
                        "source_url": null,
                        "source_sha256": null,
                        "installed_files": [
                            { "path": "reaper.exe", "sha256": null, "size": 13 },
                            { "path": "reaper.ini", "sha256": null, "size": 10 }
                        ],
                        "installed_at": null,
                        "rabbit_version": "0.4.2",
                        "architecture": null
                    }
                }
            }"#,
        )
        .unwrap();
        let specs = package_specs_by_id(Platform::Windows);
        let spec = specs.get(crate::package::PACKAGE_REAPER).unwrap();
        let detect = || {
            super::detect_component_with_probes(root, Platform::Windows, spec, Some(&state), |_| {
                None
            })
            .unwrap()
        };
        assert!(detect().installed, "the receipt matches as written");

        // REAPER runs and rewrites its configuration.
        fs::write(
            root.join("reaper.ini"),
            b"[REAPER]\r\nlast_project=demo.rpp\r\n",
        )
        .unwrap();

        let reaper = detect();
        assert!(reaper.installed, "{reaper:?}");
        assert_eq!(reaper.detector, "rabbit-receipt");
        assert_eq!(
            reaper.version,
            Some(crate::version::Version::parse("7.80").unwrap())
        );
    }

    /// A pull-request build's binary holds its own version *and*, further
    /// on, the help text "For example: 2024.3.6.1332,13560ef7" — which looks
    /// exactly like a snapshot version. Checked against the real
    /// pr1454-534,240c4663 dll. The pull-request scan therefore runs first,
    /// or a build with no receipt would report that example as its version.
    #[test]
    fn a_pull_request_build_reports_its_own_version_not_the_help_text() {
        let binary = "…pr1454-534,240c4663…Copyright (C) 2014-2025 NV Access Limited             …This will be in the form: year.month.day.build,commit.              For example: 2024.3.6.1332,13560ef7.…";

        assert_eq!(
            scan(OSARA_PULL_REQUEST_PATTERN, binary).unwrap().raw(),
            "pr1454-534,240c4663"
        );
        // The snapshot scan is what a pull-request build used to fall to.
        assert_eq!(
            scan(OSARA_SNAPSHOT_PATTERN, binary).unwrap().raw(),
            "2024.3.6.1332",
            "the snapshot pattern matches the help text, so it must run second"
        );
        // A regular snapshot build is unaffected: its own version comes
        // before the help text, as in the installed dll.
        let snapshot = "…2026.8.9.2302…For example: 2024.3.6.1332,13560ef7.…";
        assert_eq!(
            scan(OSARA_SNAPSHOT_PATTERN, snapshot).unwrap().raw(),
            "2026.8.9.2302"
        );
        assert!(scan(OSARA_PULL_REQUEST_PATTERN, snapshot).is_none());
    }

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
        // app2clap installs to REAPER's own UserPlugins\FX folder → scanned
        // there, under the resource path, on every platform.
        let app2clap = specs.get(PACKAGE_APP2CLAP).unwrap();
        assert_eq!(
            package_scan_dir(resource, app2clap),
            Some(resource.join("UserPlugins").join("FX"))
        );
    }

    /// app2clap's spec for Windows.
    fn app2clap() -> crate::package::PackageSpec {
        package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_APP2CLAP)
            .unwrap()
    }

    /// A receipt with app2clap at `version`, listing `path`, relative to
    /// the resource folder or absolute, as holding `contents`.
    fn app2clap_receipt(
        version: &str,
        path: &std::path::Path,
        contents: &[u8],
    ) -> crate::receipt::InstallState {
        let mut state = crate::receipt::InstallState::default();
        state.packages.insert(
            PACKAGE_APP2CLAP.to_string(),
            crate::receipt::PackageReceipt {
                id: PACKAGE_APP2CLAP.to_string(),
                version: Some(crate::version::Version::parse(version).unwrap()),
                source_url: None,
                source_sha256: None,
                variant: None,
                channel: None,
                installed_files: vec![crate::receipt::InstalledFileReceipt {
                    path: path.to_path_buf(),
                    sha256: Some(crate::hash::sha256_bytes(contents)),
                    size: Some(contents.len() as u64),
                }],
                installed_at: None,
                rabbit_version: None,
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );
        state
    }

    /// Detect app2clap in `resource` with `state`, where `clap` is the
    /// CLAP folder of a standard REAPER, `None` for a portable one.
    fn detect_app2clap(
        resource: &std::path::Path,
        state: Option<&crate::receipt::InstallState>,
        clap: Option<&std::path::Path>,
    ) -> crate::model::ComponentDetection {
        let outside = |destination: crate::package::InstallDestination| {
            (destination == crate::package::InstallDestination::WindowsClapDir)
                .then(|| clap.map(std::path::Path::to_path_buf))
                .flatten()
        };
        super::detect_component_seeing(
            resource,
            Platform::Windows,
            &app2clap(),
            state,
            |_| None,
            &outside,
        )
        .unwrap()
    }

    /// app2clap in REAPER's own UserPlugins\FX folder is found by its
    /// receipt, which records the file relative to the resource folder,
    /// and without one by its name. The other plug-ins in that folder,
    /// files and folders alike, are never taken for it.
    #[test]
    fn app2clap_is_found_in_the_fx_folder() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let fx = resource.join("UserPlugins").join("FX");
        fs::create_dir_all(fx.join("Other Synth.vst3")).unwrap();
        fs::write(fx.join("Other Synth.clap"), b"other").unwrap();
        fs::write(fx.join("app2clap.clap"), b"app2clap").unwrap();
        let relative = std::path::Path::new("UserPlugins")
            .join("FX")
            .join("app2clap.clap");
        let state = app2clap_receipt("2026.5.17.34", &relative, b"app2clap");

        let found = detect_app2clap(&resource, Some(&state), None);
        assert!(found.installed, "{found:?}");
        assert_eq!(found.detector, "rabbit-receipt");
        assert_eq!(found.version.unwrap().raw(), "2026.5.17.34");
        assert_eq!(found.files, vec![fx.join("app2clap.clap")]);

        let found = detect_app2clap(&resource, None, None);
        assert!(found.installed, "{found:?}");
        assert_eq!(found.detector, "file-presence");
        assert_eq!(found.version, None);
        assert_eq!(found.files, vec![fx.join("app2clap.clap")]);

        fs::remove_file(fx.join("app2clap.clap")).unwrap();
        assert!(!detect_app2clap(&resource, None, None).installed);
    }

    /// An app2clap an earlier RABBIT installed into the CLAP folder, a
    /// temporary one here, whose receipt names it by its absolute path,
    /// still counts as installed with its version. So does one moved by
    /// hand into UserPlugins\FX since, its receipt still naming the CLAP
    /// folder.
    #[test]
    fn an_older_receipt_naming_the_clap_folder_still_finds_app2clap() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap = dir.path().join("CLAP");
        fs::create_dir_all(&resource).unwrap();
        fs::create_dir_all(&clap).unwrap();
        fs::write(clap.join("app2clap.clap"), b"old app2clap").unwrap();
        let state = app2clap_receipt("2026.5.16.31", &clap.join("app2clap.clap"), b"old app2clap");

        for standard in [Some(clap.as_path()), None] {
            let found = detect_app2clap(&resource, Some(&state), standard);
            assert!(found.installed, "{found:?}");
            assert_eq!(found.detector, "rabbit-receipt");
            assert_eq!(found.version.unwrap().raw(), "2026.5.16.31");
            assert_eq!(found.files, vec![clap.join("app2clap.clap")]);
        }

        let fx = resource.join("UserPlugins").join("FX");
        fs::create_dir_all(&fx).unwrap();
        fs::rename(clap.join("app2clap.clap"), fx.join("app2clap.clap")).unwrap();
        let found = detect_app2clap(&resource, Some(&state), Some(&clap));
        assert!(found.installed, "{found:?}");
        assert_eq!(found.detector, "rabbit-receipt-mismatch");
        assert_eq!(found.version.unwrap().raw(), "2026.5.16.31");
        assert_eq!(found.files, vec![fx.join("app2clap.clap")]);
    }

    /// A copy in the CLAP folder with no receipt, such as one put there by
    /// hand, counts as installed for the standard REAPER, as it did while
    /// app2clap installed there. For a portable REAPER it doesn't: the
    /// folder is every REAPER's, and RABBIT never installed there for one.
    /// Detection in a temporary resource folder, never the standard
    /// REAPER's, doesn't look in the real CLAP folder at all.
    #[test]
    fn a_copy_in_the_clap_folder_counts_only_for_the_standard_reaper() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap = dir.path().join("CLAP");
        fs::create_dir_all(&resource).unwrap();
        fs::create_dir_all(&clap).unwrap();
        fs::write(clap.join("app2clap.clap"), b"by hand").unwrap();
        fs::write(clap.join("Other Synth.clap"), b"other").unwrap();

        let found = detect_app2clap(&resource, None, Some(&clap));
        assert!(found.installed, "{found:?}");
        assert_eq!(found.version, None);
        assert_eq!(found.files, vec![clap.join("app2clap.clap")]);

        assert!(!detect_app2clap(&resource, None, None).installed);
        assert_eq!(
            super::former_outside_folder(
                Platform::Windows,
                &resource,
                crate::package::InstallDestination::WindowsClapDir
            ),
            None
        );
    }

    /// For the standard REAPER, the CLAP folder is where a former copy of
    /// app2clap counts. Only the paths are compared: nothing is read or
    /// written in the real resource folder or the real CLAP folder.
    #[cfg(target_os = "windows")]
    #[test]
    fn the_standard_reaper_looks_in_the_clap_folder_for_a_former_copy() {
        use crate::package::InstallDestination;

        let standard = crate::preflight::standard_resource_path(Some(Platform::Windows))
            .expect("the standard resource folder on Windows");
        assert_eq!(
            super::former_outside_folder(
                Platform::Windows,
                &standard,
                InstallDestination::WindowsClapDir
            ),
            rabbit_platform::windows_clap_dir()
        );
        assert!(rabbit_platform::windows_clap_dir().is_some());
        assert_eq!(
            super::former_outside_folder(
                Platform::Windows,
                &standard,
                InstallDestination::UserPluginsFx
            ),
            None
        );
    }

    /// The copies an update moves out of the way are the ones detection
    /// counts: in the CLAP folder only for the standard REAPER, and only
    /// files matching app2clap's plug-in names. The copies every REAPER
    /// loads are those in the CLAP folder, whichever REAPER asks.
    #[test]
    fn former_copies_are_the_ones_detection_counts() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let clap = dir.path().join("CLAP");
        fs::create_dir_all(&resource).unwrap();
        fs::create_dir_all(clap.join("app2clap.clap.d")).unwrap();
        fs::write(clap.join("app2clap.clap"), b"by hand").unwrap();
        fs::write(clap.join("Other Synth.clap"), b"other").unwrap();
        let spec = app2clap();
        let standard = |destination: crate::package::InstallDestination| {
            (destination == crate::package::InstallDestination::WindowsClapDir)
                .then(|| clap.clone())
        };

        assert_eq!(
            super::former_copies(&resource, &spec, &standard),
            vec![clap.join("app2clap.clap")]
        );
        assert!(super::former_copies(&resource, &spec, &|_| None).is_empty());
        assert_eq!(
            super::former_outside_copies_seeing(&spec, &standard),
            vec![clap.join("app2clap.clap")]
        );
        fs::remove_file(clap.join("app2clap.clap")).unwrap();
        assert!(super::former_outside_copies_seeing(&spec, &standard).is_empty());
        // A package that never moved has no former copies.
        let reapack = package_specs_by_id(Platform::Windows)
            .remove(crate::package::PACKAGE_REAPACK)
            .unwrap();
        assert!(super::former_outside_copies_seeing(&reapack, &|_| Some(clap.clone())).is_empty());
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

    /// A Surge XT VST3 bundle that a stopped macOS removal emptied, its
    /// Info.plist gone with the rest, is still Surge XT on a Mac, with no
    /// version: the uninstall is offered again for what is left. On
    /// Windows a bundle with no version is still not enough.
    #[test]
    fn a_surge_xt_vst3_bundle_with_no_version_is_still_detected_on_a_mac() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("Surge XT.vst3");
        fs::create_dir_all(bundle.join("Contents").join("MacOS")).unwrap();

        let mac_spec = package_specs_by_id(Platform::MacOs)
            .remove(PACKAGE_SURGE_XT)
            .unwrap();
        let mac = super::surge_xt_bundle_detection(&mac_spec, Platform::MacOs, bundle.clone())
            .expect("a Mac still has Surge XT");
        assert!(mac.installed);
        assert_eq!(mac.version, None);
        assert_eq!(mac.detector, "surge-xt-vendor-vst3-bundle-unversioned");
        assert_eq!(mac.confidence, crate::model::Confidence::Low);
        assert_eq!(mac.files, std::slice::from_ref(&bundle));

        let windows_spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_SURGE_XT)
            .unwrap();
        assert_eq!(
            super::surge_xt_bundle_detection(&windows_spec, Platform::Windows, bundle),
            None
        );
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

    /// The LReaParams detection among `detections`.
    fn lreaparams(
        detections: &[crate::model::ComponentDetection],
    ) -> &crate::model::ComponentDetection {
        detections
            .iter()
            .find(|detection| detection.package_id == PACKAGE_LREAPARAMS)
            .unwrap()
    }

    /// Without a receipt, LReaParams is found by its file. Its DLL carries no
    /// version resource, so the file alone gives no version.
    #[test]
    fn detects_lreaparams_by_its_file() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        let dll = plugins.join("reaper_lreaparams.dll");
        fs::write(&dll, b"LReaParams\0").unwrap();

        let detections =
            super::detect_components_with_probes(dir.path(), Platform::Windows, |_| None).unwrap();
        let found = lreaparams(&detections);
        assert!(found.installed);
        assert_eq!(found.version, None);
        assert_eq!(found.detector, "file-presence");
        assert_eq!(found.files, vec![dll]);
        assert_eq!(found.left_to_reapack, None);
    }

    /// While ReaPack is installed, an LReaParams its records name is left to
    /// it, with the version from those records, and still once its file is
    /// gone: ReaPack thinks it installed it. Before ReaPack is there, or
    /// after it is gone, nothing keeps those records, and RABBIT treats the
    /// copy as its own.
    #[test]
    fn lreaparams_is_left_to_reapack_while_reapack_is_installed() {
        use crate::model::LeftToReapack;
        use crate::reapack::write_test_registry;

        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        let dll = plugins.join("reaper_lreaparams.dll");
        fs::write(&dll, b"LReaParams\0").unwrap();
        write_test_registry(
            dir.path(),
            &[(
                "LReaParams.ext",
                "0.0.75",
                &[
                    "UserPlugins/reaper_lreaparams.dll",
                    "UserPlugins/Guide/INSTALL.txt",
                ],
            )],
        );
        let detect = || {
            super::detect_components_with_probes(dir.path(), Platform::Windows, |_| None).unwrap()
        };

        let detections = detect();
        let found = lreaparams(&detections);
        assert!(found.installed);
        assert_eq!(found.detector, "reapack-registry");
        assert_eq!(found.version.as_ref().unwrap().raw(), "0.0.75");
        assert_eq!(found.left_to_reapack, None);

        fs::write(plugins.join("reaper_reapack-x64.dll"), b"ReaPack\0").unwrap();
        let managed = Some(LeftToReapack::Managed {
            reapack_package: "LReaParams.ext".to_string(),
        });
        let detections = detect();
        assert_eq!(lreaparams(&detections).left_to_reapack, managed);
        // Only a package whose manifest asks for it is left to ReaPack.
        assert!(
            detections
                .iter()
                .filter(|detection| detection.package_id != PACKAGE_LREAPARAMS)
                .all(|detection| detection.left_to_reapack.is_none())
        );

        fs::remove_file(&dll).unwrap();
        let detections = detect();
        let found = lreaparams(&detections);
        assert!(!found.installed);
        assert_eq!(found.left_to_reapack, managed);
    }

    /// Only a file directly in UserPlugins whose name is LReaParams' counts,
    /// an unreadable database leaves the copy alone to be safe, and a
    /// package that doesn't ask to be left to ReaPack never is.
    #[test]
    fn what_reapacks_records_say_about_a_package_left_to_it() {
        use crate::model::LeftToReapack;
        use crate::reapack::{registry_path, write_test_registry};

        let specs = package_specs_by_id(Platform::Windows);
        let lreaparams = &specs[PACKAGE_LREAPARAMS];
        let sws = &specs[PACKAGE_SWS];

        let dir = tempdir().unwrap();
        assert_eq!(super::left_to_reapack(dir.path(), lreaparams), None);
        write_test_registry(
            dir.path(),
            &[
                ("Something else", "1.0", &["UserPlugins/reaper_other.dll"]),
                ("Nested", "1.0", &["UserPlugins/old/reaper_lreaparams.dll"]),
                (
                    "SWS/S&M Extension",
                    "2.14.0.7",
                    &["UserPlugins/reaper_sws-x64.dll"],
                ),
            ],
        );
        assert_eq!(super::left_to_reapack(dir.path(), lreaparams), None);
        assert_eq!(super::left_to_reapack(dir.path(), sws), None);

        let dir = tempdir().unwrap();
        write_test_registry(
            dir.path(),
            &[(
                "LReaParams.ext",
                "0.0.75",
                // ReaPack writes paths as its index spells them.
                &["userplugins/REAPER_LREAPARAMS.DLL"],
            )],
        );
        assert_eq!(
            super::left_to_reapack(dir.path(), lreaparams),
            Some(LeftToReapack::Managed {
                reapack_package: "LReaParams.ext".to_string()
            })
        );

        let dir = tempdir().unwrap();
        let db = registry_path(dir.path());
        fs::create_dir_all(db.parent().unwrap()).unwrap();
        fs::write(&db, b"this is not an SQLite database, just text").unwrap();
        assert!(matches!(
            super::left_to_reapack(dir.path(), lreaparams),
            Some(LeftToReapack::RecordsUnreadable { .. })
        ));

        // Something other than a file where the database belongs is no more
        // readable than a damaged one, as uninstall's check has it.
        let dir = tempdir().unwrap();
        fs::create_dir_all(registry_path(dir.path())).unwrap();
        assert!(matches!(
            super::left_to_reapack(dir.path(), lreaparams),
            Some(LeftToReapack::RecordsUnreadable { error }) if error.contains("not a file")
        ));
    }

    /// When ReaPack installs over the LReaParams RABBIT had, the copy is
    /// ReaPack's: its version, detector and files come from ReaPack's
    /// records, not from RABBIT's receipt, which no longer fits it. A copy
    /// that still fits the receipt is the one RABBIT installed, so the
    /// receipt keeps saying which version it is.
    #[test]
    fn an_lreaparams_reapack_installed_over_rabbits_copy_is_reapacks() {
        use crate::model::LeftToReapack;
        use crate::reapack::write_test_registry;
        use crate::receipt::{PackageReceiptParams, save_install_state, upsert_package_receipt};

        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(plugins.join("reaper_reapack-x64.dll"), b"ReaPack\0").unwrap();
        let dll = plugins.join("reaper_lreaparams.dll");
        fs::write(&dll, b"LReaParams 0.0.70 from RABBIT").unwrap();
        let mut state = crate::receipt::InstallState::default();
        upsert_package_receipt(
            &mut state,
            dir.path(),
            PackageReceiptParams {
                variant: None,
                channel: None,
                package_id: PACKAGE_LREAPARAMS,
                version: Some(crate::version::Version::parse("0.0.70").unwrap()),
                source_url: None,
                source_sha256: None,
                installed_paths: std::slice::from_ref(&dll),
                installed_at: None,
                architecture: None,
            },
        )
        .unwrap();
        save_install_state(dir.path(), &state).unwrap();
        write_test_registry(
            dir.path(),
            &[(
                "LReaParams.ext",
                "0.0.76",
                &["UserPlugins/reaper_lreaparams.dll"],
            )],
        );
        let managed = Some(LeftToReapack::Managed {
            reapack_package: "LReaParams.ext".to_string(),
        });
        let detect = || {
            super::detect_components_with_probes(dir.path(), Platform::Windows, |_| None).unwrap()
        };

        let detections = detect();
        let found = lreaparams(&detections);
        assert_eq!(found.detector, "rabbit-receipt");
        assert_eq!(found.version.as_ref().unwrap().raw(), "0.0.70");
        assert_eq!(found.left_to_reapack, managed);

        fs::write(&dll, b"LReaParams 0.0.76 from ReaPack, a longer file").unwrap();
        let detections = detect();
        let found = lreaparams(&detections);
        assert!(found.installed);
        assert_eq!(found.detector, "reapack-registry");
        assert_eq!(found.version.as_ref().unwrap().raw(), "0.0.76");
        assert_eq!(found.files, vec![dll.clone()]);
        assert!(
            found.notes.iter().all(|note| !note.contains("receipt")),
            "{:?}",
            found.notes
        );
        assert_eq!(found.left_to_reapack, managed);

        // Without ReaPack, nothing keeps its records, and the receipt is
        // what RABBIT goes by, mismatch and all.
        fs::remove_file(plugins.join("reaper_reapack-x64.dll")).unwrap();
        let detections = detect();
        let found = lreaparams(&detections);
        assert_eq!(found.detector, "rabbit-receipt-mismatch");
        assert_eq!(found.left_to_reapack, None);
    }

    /// The packages a command names that are ReaPack's to look after: only
    /// those whose manifest asks for it, whose copy ReaPack's records name,
    /// while ReaPack is installed.
    #[test]
    fn which_named_packages_are_left_to_reapack() {
        use crate::model::LeftToReapack;
        use crate::reapack::write_test_registry;

        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        fs::create_dir_all(&plugins).unwrap();
        let named = [PACKAGE_LREAPARAMS.to_string(), PACKAGE_SWS.to_string()];
        let left = || super::packages_left_to_reapack(dir.path(), Platform::Windows, &named);
        assert_eq!(left(), Vec::new());

        write_test_registry(
            dir.path(),
            &[
                (
                    "LReaParams.ext",
                    "0.0.75",
                    &["UserPlugins/reaper_lreaparams.dll"],
                ),
                (
                    "SWS/S&M Extension",
                    "2.14.0.7",
                    &["UserPlugins/reaper_sws-x64.dll"],
                ),
            ],
        );
        fs::write(plugins.join("reaper_lreaparams.dll"), b"LReaParams\0").unwrap();
        // ReaPack isn't installed, so its records have no hold.
        assert_eq!(left(), Vec::new());

        fs::write(plugins.join("reaper_reapack-x64.dll"), b"ReaPack\0").unwrap();
        assert_eq!(
            left(),
            vec![(
                PACKAGE_LREAPARAMS.to_string(),
                LeftToReapack::Managed {
                    reapack_package: "LReaParams.ext".to_string()
                }
            )]
        );
        assert_eq!(
            super::packages_left_to_reapack(
                dir.path(),
                Platform::Windows,
                &[PACKAGE_SWS.to_string(), PACKAGE_REAPACK.to_string()]
            ),
            Vec::new()
        );
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
