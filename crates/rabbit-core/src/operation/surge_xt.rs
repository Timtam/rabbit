use std::path::{Path, PathBuf};

use crate::artifact::ArtifactKind;
use crate::model::Platform;

use super::install_problem::{InstallerExitMeaning, inno_setup_exit_meaning};
use super::{
    PackageAutomationSupport, PlannedAutomationKind, PlannedExecutionKind, PlannedExecutionOverride,
};

pub(super) const TITLE: &str = "Surge XT";

/// Filename suffix of the inner `.pkg` we run from the mounted Surge XT
/// DMG. Plain `.pkg` keeps the matcher resilient to upstream filename
/// drift (the build prefix is `surge-xt-macOS-NIGHTLY-…` today but has
/// shifted before). The DMG only ever contains one installer pkg, so a
/// loose suffix match is unambiguous.
const MACOS_PKG_SUFFIX: &str = ".pkg";

/// How the log of Surge XT's Windows installer starts its name in RABBIT's
/// logs folder; see [`crate::report::installer_log_path`].
pub(super) const INSTALLER_LOG_NAME: &str = "surge-xt-installer";

/// Surge XT-specific automation routing. Both upstream artifacts ship
/// vendor installers that need administrator rights:
/// - Windows: Inno Setup `setup.exe` (silent flags below). RABBIT starts it
///   without elevation, and Inno raises the permission prompt itself.
/// - macOS: distribution `.pkg` wrapped in a `.dmg`, executed elevated by
///   `/usr/sbin/installer` via [`run_pkg_installer_from_disk_image`].
pub(super) fn automation_support_for(
    kind: ArtifactKind,
    platform: Platform,
) -> Option<PackageAutomationSupport> {
    match (kind, platform) {
        (ArtifactKind::Installer, Platform::Windows) => Some(
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::VendorInstaller),
        ),
        (ArtifactKind::DiskImage, Platform::MacOs) => Some(
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::DiskImageInstall),
        ),
        _ => None,
    }
}

pub(super) fn installer_arguments(
    kind: ArtifactKind,
    platform: Platform,
    installer_log: Option<&Path>,
) -> Option<Vec<String>> {
    match (kind, platform) {
        (ArtifactKind::Installer, Platform::Windows) => {
            Some(surge_xt_inno_setup_arguments(installer_log))
        }
        _ => None,
    }
}

/// Whether Surge XT's installer here is Inno Setup's `setup.exe`, on
/// Windows. It writes the log `/LOG=` asks for, and it rolls back before it
/// ends with a code other than 0, so that code is final. The macOS `.pkg`
/// does neither.
pub(super) fn runs_inno_setup(kind: ArtifactKind, platform: Platform) -> bool {
    matches!(
        (kind, platform),
        (ArtifactKind::Installer, Platform::Windows)
    )
}

/// What an exit code of Surge XT's installer means, for the codes Inno
/// Setup documents and a user can act on.
pub(super) fn exit_meaning(
    kind: ArtifactKind,
    platform: Platform,
    exit_code: i32,
) -> Option<InstallerExitMeaning> {
    match (kind, platform) {
        (ArtifactKind::Installer, Platform::Windows) => inno_setup_exit_meaning(exit_code),
        _ => None,
    }
}

/// Inno Setup silent-install command line. `/VERYSILENT
/// /SUPPRESSMSGBOXES /NORESTART` is the documented unattended trio.
/// `/TYPE=full` picks the "Full installation" option (CLAP + VST3 +
/// Standalone + Effects variants + Data + Patches + Wavetables), and
/// `/ALLUSERS` forces the per-machine install scope the package ships
/// with by default (`{commoncf64}\VST3\…` and `{commonappdata}\Surge XT`).
///
/// `/NOCLOSEAPPLICATIONS` stops Inno Setup from asking Windows' Restart
/// Manager to close programs. Inno registers every file it would install
/// that matches Surge's `CloseApplicationsFilter` (`*.exe,*.vst3,*.ttf`),
/// including the six fonts in `C:\Windows\Fonts` that Surge's uninstaller
/// leaves behind and `onlyifdoesntexist` never replaces. When a program had
/// one of those fonts open and refused to close, Inno asked whether to
/// abort, `/SUPPRESSMSGBOXES` answered Abort, and setup ended with exit code
/// 5 having installed nothing. Closing programs could also close the user's
/// screen reader. A running Surge XT is caught before the installer starts
/// instead, from the manifest's `blocking_programs`.
///
/// `/LOG=<path>` has Inno write its log next to RABBIT's reports, so a
/// failure can be looked into. The argument carries no quotes of its own:
/// `Command` on Windows and the elevated start's parameter string both put
/// an argument with a space in quotes, and Inno's parameter parser drops
/// quotes wherever they are, so a path with spaces arrives whole. Quotes
/// written into the argument would reach Inno escaped with backslashes,
/// which it keeps. RABBIT starts this installer with `Command` today; Inno
/// raises the permission prompt itself and hands its command line on to the
/// elevated copy unchanged. Declining that prompt ends it with code 2.
fn surge_xt_inno_setup_arguments(installer_log: Option<&Path>) -> Vec<String> {
    let mut arguments = vec![
        "/VERYSILENT".to_string(),
        "/SUPPRESSMSGBOXES".to_string(),
        "/NORESTART".to_string(),
        "/NOCLOSEAPPLICATIONS".to_string(),
        "/TYPE=full".to_string(),
        "/ALLUSERS".to_string(),
    ];
    if let Some(log) = installer_log {
        arguments.push(format!("/LOG={}", log.display()));
    }
    arguments
}

pub(super) fn planned_execution_override(
    kind: ArtifactKind,
    platform: Platform,
) -> Option<PlannedExecutionOverride> {
    match (kind, platform) {
        (ArtifactKind::DiskImage, Platform::MacOs) => Some(PlannedExecutionOverride {
            kind: PlannedExecutionKind::MountDiskImageAndRunPkgInstaller,
            arguments: vec![MACOS_PKG_SUFFIX.to_string()],
            use_cached_working_dir: false,
        }),
        _ => None,
    }
}

/// Paths the post-install verification step expects to exist. We point
/// at the system VST3 bundle and the factory-data root rather than the
/// inner DLL/dylib: the bundle directory is created by the installer
/// before the inner binary lands, and the factory-data root is the
/// "no-portable-install" signal RABBIT wants every run to corroborate.
///
/// On Windows we **only** target `%CommonProgramFiles%` (the native /
/// 64-bit Common Files root, equivalent to Inno Setup's `{commoncf64}`)
/// because the Surge XT nightly only ships a win64 setup.exe. Including
/// the `(x86)` root here would require both paths to exist for
/// verification to pass — but the installer only writes to one of them,
/// so the other would always be reported as a "missing path" and the
/// run would fail post-install verification despite a healthy install.
pub(super) fn verification_paths(platform: Platform) -> Vec<PathBuf> {
    match platform {
        Platform::Windows => {
            let mut paths = Vec::new();
            if let Some(common) = rabbit_platform::windows_common_program_files_dir() {
                paths.push(
                    common
                        .join("VST3")
                        .join("Surge Synth Team")
                        .join("Surge XT.vst3"),
                );
            }
            if let Some(program_data) = rabbit_platform::windows_program_data_dir() {
                paths.push(program_data.join("Surge XT"));
            }
            paths
        }
        Platform::MacOs => vec![
            PathBuf::from("/Library/Audio/Plug-Ins/VST3/Surge XT.vst3"),
            PathBuf::from("/Library/Application Support/Surge XT"),
        ],
    }
}

/// Files the receipt should reference after a successful run, filtered
/// to the on-disk existing ones so the receipt doesn't claim ownership
/// of a path the installer didn't actually create.
pub(super) fn receipt_paths(platform: Platform) -> Vec<PathBuf> {
    verification_paths(platform)
        .into_iter()
        .filter(|path| path.exists())
        .collect()
}
