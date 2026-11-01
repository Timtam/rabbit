use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use rabbit_core::artifact::{
    ArtifactDescriptor, CachedArtifact, default_cache_dir, download_artifacts,
    resolve_latest_artifacts_on,
};
use rabbit_core::cancel::CancelToken;
use rabbit_core::detection::{DiscoveryOptions, detect_components, discover_installations};
use rabbit_core::install::{InstallOptions, InstallReport, install_cached_artifacts};
use rabbit_core::latest::fetch_latest_versions_on;
use rabbit_core::localization::{
    DEFAULT_LOCALE, LocalizedText, Localizer, available_locales, resolve_runtime_locale,
};
use rabbit_core::model::{Architecture, ComponentDetection, InstallationKind, Platform};
use rabbit_core::operation::{
    PackageOperationItem, PackageOperationOptions, PackageOperationReport, PackageOperationStatus,
    PlannedExecutionKind, execute_package_operation,
};
use rabbit_core::osara_keymap::{OsaraKeymapDefault, OsaraKeymapState, osara_keymap_default};
use rabbit_core::package::uninstall::{
    UninstallBackup, UninstallElevation, UninstallPlanSpec, UninstallSpec, UninstallStrategy,
};
use rabbit_core::package::{PACKAGE_OSARA, PackageChannels, STABLE_CHANNEL, remembered_channels};
use rabbit_core::package::{
    SupportedPlatform, builtin_package_specs, default_desired_package_ids,
    embedded_package_manifest,
};
use rabbit_core::plan::{
    AvailablePackage, PlanActionKind, build_install_plan_with_installed_channels,
};
use rabbit_core::portable::{PortabilityCheckStatus, PortabilityReport, check_portable_runtime};
use rabbit_core::preflight::{
    PreflightOptions, PreflightReport, PreflightStatus, run_install_preflight,
};
use rabbit_core::progress::ProgressReporter;
use rabbit_core::report::{default_report_path, save_json_and_text_reports};
use rabbit_core::resource::{ResourceInitActionKind, ResourceInitReport, initialize_resource_path};
use rabbit_core::rollback::{
    BackupSet, RestoreBackupActionKind, RestoreBackupOptions, RestoreBackupReport, RestoredPackage,
    list_backup_sets, restore_backup_set,
};
use rabbit_core::self_update::{
    ApplySelfUpdateOptions, DEFAULT_SELF_UPDATE_MANIFEST_URL, SelfUpdateApplyReport,
    SelfUpdateCheckReport, SelfUpdateStageReport, apply_self_update, check_self_update,
    default_self_update_staging_dir, relaunch_current_executable,
    resolve_self_update_release_notes, stage_self_update,
};
use rabbit_core::setup::{SetupOptions, SetupReport, execute_setup_operation};
use rabbit_core::uninstall::{
    KeptFileReason, KeptReason, Removability, RemovalReason, UninstallMessage, UninstallNote,
    UninstallPlan, UninstallPlanItem, UninstallReport, UninstallReportItem, UninstallRequest,
    UninstallStatus, UninstallWarning, execute_uninstall, plan_uninstall_at,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "rabbit")]
#[command(version)]
#[command(about = "Diagnostic CLI for REAPER Accessibility Bootstrap & Bundle Installation Tool")]
#[command(help_template = "\
{name} {version}
{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}\
")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Detect {
        /// Also probe this folder for a portable REAPER install. Repeatable.
        #[arg(long)]
        portable: Vec<PathBuf>,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Components {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Latest {
        /// Check a package on another channel instead of its regular release,
        /// e.g. `--package-channel reaper=dev`. Repeatable.
        #[arg(long = "package-channel")]
        package_channel: Vec<String>,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Artifacts {
        /// Package id to resolve, repeatable. Defaults to the recommended
        /// set.
        #[arg(long)]
        package: Vec<String>,
        /// Install for this architecture instead of the one detected for the
        /// host.
        #[arg(long, value_enum)]
        architecture: Option<CliArchitecture>,
        /// Check a package on another channel instead of its regular release,
        /// e.g. `--package-channel reaper=dev`. Repeatable.
        #[arg(long = "package-channel")]
        package_channel: Vec<String>,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Download {
        /// Package id to download, repeatable. Defaults to the recommended
        /// set.
        #[arg(long)]
        package: Vec<String>,
        /// Install for this architecture instead of the one detected for the
        /// host.
        #[arg(long, value_enum)]
        architecture: Option<CliArchitecture>,
        /// Where downloads are cached. Defaults to RABBIT's own folder inside
        /// the system temp directory.
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Check a package on another channel instead of its regular release,
        /// e.g. `--package-channel reaper=dev`. Repeatable.
        #[arg(long = "package-channel")]
        package_channel: Vec<String>,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Packages {
        /// Read the package manifest from this file instead of the built-in
        /// one.
        #[arg(long)]
        manifest: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Preflight {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Path to REAPER itself (`reaper.exe`, or `REAPER.app` on macOS)
        /// when it is not in the usual place for the given resource folder.
        #[arg(long)]
        target_app_path: Option<PathBuf>,
        /// Report what preflight would check without touching anything.
        #[arg(long)]
        dry_run: bool,
        /// Run even though REAPER is open. Files REAPER holds open may fail
        /// to be replaced, so prefer closing it first.
        #[arg(long)]
        allow_reaper_running: bool,
        /// Write the JSON run report to exactly this path.
        #[arg(long)]
        report_path: Option<PathBuf>,
        /// Save a JSON run report under the resource folder's `RABBIT`
        /// directory, so there is a record of what happened.
        #[arg(long)]
        save_report: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    InitResource {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Path to REAPER itself (`reaper.exe`, or `REAPER.app` on macOS)
        /// when it is not in the usual place for the given resource folder.
        #[arg(long)]
        target_app_path: Option<PathBuf>,
        /// Prepare the folder as a portable REAPER install rather than a
        /// system one.
        #[arg(long)]
        portable: bool,
        /// Actually make the changes. Without it the command is a dry run
        /// that only reports what it would do.
        #[arg(long)]
        apply: bool,
        /// Run even though REAPER is open. Files REAPER holds open may fail
        /// to be replaced, so prefer closing it first.
        #[arg(long)]
        allow_reaper_running: bool,
        /// Write the JSON run report to exactly this path.
        #[arg(long)]
        report_path: Option<PathBuf>,
        /// Save a JSON run report under the resource folder's `RABBIT`
        /// directory, so there is a record of what happened.
        #[arg(long)]
        save_report: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Backups {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    RestoreBackup {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Which backup set to roll back to, as listed by `backups`.
        #[arg(long)]
        backup_id: String,
        /// Restore only this package, from a backup set an uninstall wrote.
        /// Its files come back, its entry goes back into RABBIT's receipt,
        /// it comes off the list of packages you uninstalled or turned
        /// down, and REAPER's language is set back if the uninstall reset it
        /// and no language was chosen since. Refused when the package was
        /// installed again after the uninstall.
        #[arg(long)]
        package: Option<String>,
        /// Actually make the changes. Without it the command is a dry run
        /// that only reports what it would do.
        #[arg(long)]
        apply: bool,
        /// Run even though REAPER is open. Files REAPER holds open may fail
        /// to be replaced, so prefer closing it first.
        #[arg(long)]
        allow_reaper_running: bool,
        /// Write the JSON run report to exactly this path.
        #[arg(long)]
        report_path: Option<PathBuf>,
        /// Save a JSON run report under the resource folder's `RABBIT`
        /// directory, so there is a record of what happened.
        #[arg(long)]
        save_report: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    InstallExtension {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Path to REAPER itself (`reaper.exe`, or `REAPER.app` on macOS)
        /// when it is not in the usual place for the given resource folder.
        #[arg(long)]
        target_app_path: Option<PathBuf>,
        /// Package id to install, repeatable. Required — use `apply-packages`
        /// to install everything that needs it. Naming a package you
        /// uninstalled or turned down takes it off that list.
        #[arg(long, required = true)]
        package: Vec<String>,
        /// Install for this architecture instead of the one detected for the
        /// host.
        #[arg(long, value_enum)]
        architecture: Option<CliArchitecture>,
        /// Where downloads are cached. Defaults to RABBIT's own folder inside
        /// the system temp directory.
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Actually make the changes. Without it the command is a dry run
        /// that only reports what it would do.
        #[arg(long)]
        apply: bool,
        /// Run even though REAPER is open. Files REAPER holds open may fail
        /// to be replaced, so prefer closing it first.
        #[arg(long)]
        allow_reaper_running: bool,
        /// Acknowledge ReaPack's donation notice. ReaPack shows it on first
        /// run and RABBIT will not install it unattended until this is
        /// passed.
        #[arg(long)]
        accept_reapack_donation_notice: bool,
        /// Install a package from another channel, e.g. `--package-channel
        /// reaper=dev` for REAPER's development builds from landoleet.org, or
        /// `--package-channel osara=pr:1454` for the test build of an OSARA pull
        /// request. `<package>=stable` goes back to the regular release. A
        /// package stays on the channel it was last installed from until told
        /// otherwise. Repeatable.
        #[arg(long = "package-channel")]
        package_channel: Vec<String>,
        /// Write the JSON run report to exactly this path. Without `--apply`
        /// the report says which artifacts would be downloaded and installed.
        #[arg(long)]
        report_path: Option<PathBuf>,
        /// Save a JSON run report under the resource folder's `RABBIT`
        /// directory, so there is a record of what happened.
        #[arg(long)]
        save_report: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    ApplyPackages {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Path to REAPER itself (`reaper.exe`, or `REAPER.app` on macOS)
        /// when it is not in the usual place for the given resource folder.
        #[arg(long)]
        target_app_path: Option<PathBuf>,
        /// Package id to act on, repeatable. Defaults to every package that
        /// needs installing or updating, except the ones you uninstalled or
        /// turned down. Naming one of those installs it and takes it off
        /// that list.
        #[arg(long)]
        package: Vec<String>,
        /// Install for this architecture instead of the one detected for the
        /// host.
        #[arg(long, value_enum)]
        architecture: Option<CliArchitecture>,
        /// Where downloads are cached. Defaults to RABBIT's own folder inside
        /// the system temp directory.
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Actually make the changes. Without it the command is a dry run
        /// that only reports what it would do.
        #[arg(long)]
        apply: bool,
        /// Run even though REAPER is open. Files REAPER holds open may fail
        /// to be replaced, so prefer closing it first.
        #[arg(long)]
        allow_reaper_running: bool,
        /// Download the packages RABBIT cannot install automatically and
        /// leave them in the cache for you to run yourself, instead of
        /// skipping them.
        #[arg(long)]
        stage_unsupported: bool,
        /// Keep your current key map (reaper-kb.ini). OSARA's "Merge OSARA
        /// key map" action can add the new OSARA key bindings to it later.
        /// By default RABBIT keeps it when OSARA is already installed and the
        /// key map differs from the installed OSARA key map.
        #[arg(long, conflicts_with = "replace_osara_keymap")]
        preserve_osara_keymap: bool,
        /// Replace your key map (reaper-kb.ini) with OSARA's, after backing
        /// it up. By default RABBIT does that when OSARA is new, or when the
        /// key map is still the installed OSARA key map with no changes of
        /// your own.
        #[arg(long)]
        replace_osara_keymap: bool,
        /// Pick a package flavour, e.g. `--package-variant langpack-es=pma`
        /// to install Team PMA's Spanish translation (es_MX) instead of the
        /// default REAPER Accesible español (es_ES). Repeatable.
        #[arg(long = "package-variant")]
        package_variant: Vec<String>,
        /// Install a package from another channel, e.g. `--package-channel
        /// reaper=dev` for REAPER's development builds from landoleet.org, or
        /// `--package-channel osara=pr:1454` for the test build of an OSARA pull
        /// request. `<package>=stable` goes back to the regular release. A
        /// package stays on the channel it was last installed from until told
        /// otherwise. Repeatable.
        #[arg(long = "package-channel")]
        package_channel: Vec<String>,
        /// Acknowledge ReaPack's donation notice. ReaPack shows it on first
        /// run and RABBIT will not install it unattended until this is
        /// passed.
        #[arg(long)]
        accept_reapack_donation_notice: bool,
        /// Write the JSON run report to exactly this path.
        #[arg(long)]
        report_path: Option<PathBuf>,
        /// Save a JSON run report under the resource folder's `RABBIT`
        /// directory, so there is a record of what happened.
        #[arg(long)]
        save_report: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Setup {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Path to REAPER itself (`reaper.exe`, or `REAPER.app` on macOS)
        /// when it is not in the usual place for the given resource folder.
        #[arg(long)]
        target_app_path: Option<PathBuf>,
        /// Treat the target as a portable REAPER install, creating it if
        /// needed.
        #[arg(long)]
        portable: bool,
        /// Package id to install, repeatable. Defaults to the recommended set
        /// for this host, except the packages you uninstalled or turned
        /// down. Naming one of those installs it and takes it off that list.
        #[arg(long)]
        package: Vec<String>,
        /// Install for this architecture instead of the one detected for the
        /// host.
        #[arg(long, value_enum)]
        architecture: Option<CliArchitecture>,
        /// Where downloads are cached. Defaults to RABBIT's own folder inside
        /// the system temp directory.
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Actually make the changes. Without it the command is a dry run
        /// that only reports what it would do.
        #[arg(long)]
        apply: bool,
        /// Run even though REAPER is open. Files REAPER holds open may fail
        /// to be replaced, so prefer closing it first.
        #[arg(long)]
        allow_reaper_running: bool,
        /// Download the packages RABBIT cannot install automatically and
        /// leave them in the cache for you to run yourself, instead of
        /// skipping them.
        #[arg(long)]
        stage_unsupported: bool,
        /// Keep your current key map (reaper-kb.ini). OSARA's "Merge OSARA
        /// key map" action can add the new OSARA key bindings to it later.
        /// By default RABBIT keeps it when OSARA is already installed and the
        /// key map differs from the installed OSARA key map.
        #[arg(long, conflicts_with = "replace_osara_keymap")]
        preserve_osara_keymap: bool,
        /// Replace your key map (reaper-kb.ini) with OSARA's, after backing
        /// it up. By default RABBIT does that when OSARA is new, or when the
        /// key map is still the installed OSARA key map with no changes of
        /// your own.
        #[arg(long)]
        replace_osara_keymap: bool,
        /// Pick a package flavour, e.g. `--package-variant langpack-es=pma`
        /// to install Team PMA's Spanish translation (es_MX) instead of the
        /// default REAPER Accesible español (es_ES). Repeatable.
        #[arg(long = "package-variant")]
        package_variant: Vec<String>,
        /// Install a package from another channel, e.g. `--package-channel
        /// reaper=dev` for REAPER's development builds from landoleet.org, or
        /// `--package-channel osara=pr:1454` for the test build of an OSARA pull
        /// request. `<package>=stable` goes back to the regular release. A
        /// package stays on the channel it was last installed from until told
        /// otherwise. Repeatable.
        #[arg(long = "package-channel")]
        package_channel: Vec<String>,
        /// Acknowledge ReaPack's donation notice. ReaPack shows it on first
        /// run and RABBIT will not install it unattended until this is
        /// passed.
        #[arg(long)]
        accept_reapack_donation_notice: bool,
        /// Run exactly this configuration step, repeatable. Overrides the
        /// default selection, so only the steps you list run.
        #[arg(long = "config-step")]
        config_step: Vec<String>,
        /// Skip this configuration step, repeatable. Everything else that
        /// would run by default still runs.
        #[arg(long = "skip-config-step")]
        skip_config_step: Vec<String>,
        /// Language pack to make active after installing, e.g.
        /// `--reaper-language langpack-de`. Several packs can be installed
        /// at once; this picks which one REAPER uses. Defaults to the only
        /// installed pack when there is exactly one.
        #[arg(long = "reaper-language")]
        reaper_language: Option<String>,
        /// Write the JSON run report to exactly this path.
        #[arg(long)]
        report_path: Option<PathBuf>,
        /// Save a JSON run report under the resource folder's `RABBIT`
        /// directory, so there is a record of what happened.
        #[arg(long)]
        save_report: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Uninstall packages. Without --apply it only shows what it would do.
    #[command(after_help = UNINSTALL_AFTER_HELP)]
    Uninstall {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: PathBuf,
        /// Path to REAPER itself (`reaper.exe`, or `REAPER.app` on macOS)
        /// when it is not in the usual place for the given resource folder.
        #[arg(long)]
        target_app_path: Option<PathBuf>,
        /// Package id to uninstall, repeatable. Required: there is no
        /// "uninstall everything". `components` says which packages RABBIT
        /// can uninstall.
        #[arg(long, required = true)]
        package: Vec<String>,
        /// Also remove each package's settings, such as SWS's S&M.ini and
        /// your grooves. They go into the same backup set as its files, so
        /// `restore-backup` brings them back. Without it, settings stay.
        #[arg(long)]
        remove_settings: bool,
        /// Actually uninstall. Without it the command is a dry run that only
        /// reports what it would do. With it, nothing changes at all when
        /// any package asked for can't be uninstalled.
        #[arg(long)]
        apply: bool,
        /// Write the JSON run report to exactly this path.
        #[arg(long)]
        report_path: Option<PathBuf>,
        /// Save a JSON run report under the resource folder's `RABBIT`
        /// directory. A run with --apply always saves one there.
        #[arg(long)]
        save_report: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Locales {
        /// Directory holding the `.ftl` translation files.
        #[arg(long, default_value = "locales")]
        locales_dir: PathBuf,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    Localize {
        /// Locale to render the message in, e.g. `de-DE`.
        #[arg(long, default_value_t = DEFAULT_LOCALE.to_string())]
        locale: String,
        /// Directory holding the `.ftl` translation files.
        #[arg(long, default_value = "locales")]
        locales_dir: PathBuf,
        /// Message id to render.
        #[arg(long)]
        id: String,
        /// Message argument as `name=value`, repeatable.
        #[arg(long = "arg")]
        args: Vec<String>,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    PortableCheck {
        /// Directory holding the `.ftl` translation files.
        #[arg(long, default_value = "locales")]
        locales_dir: PathBuf,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    SelfUpdate {
        #[command(subcommand)]
        command: SelfUpdateCommand,
    },
    Plan {
        /// REAPER's resource folder — the one holding `reaper.ini`. Usually
        /// `%APPDATA%\REAPER` on Windows and `~/Library/Application
        /// Support/REAPER` on macOS; for a portable install it is the REAPER
        /// folder itself.
        #[arg(long)]
        resource_path: Option<PathBuf>,
        /// Also probe this folder for a portable REAPER install. Repeatable.
        #[arg(long)]
        portable: Vec<PathBuf>,
        /// Compare what is installed against the live upstream feeds instead
        /// of planning offline.
        #[arg(long)]
        online: bool,
        /// Install a package from another channel, e.g. `--package-channel
        /// reaper=dev` for REAPER's development builds from landoleet.org, or
        /// `--package-channel osara=pr:1454` for the test build of an OSARA pull
        /// request. `<package>=stable` goes back to the regular release. A
        /// package stays on the channel it was last installed from until told
        /// otherwise. Repeatable.
        #[arg(long = "package-channel")]
        package_channel: Vec<String>,
        /// Output format for the plan.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        /// Write the JSON run report to exactly this path.
        #[arg(long)]
        report_path: Option<PathBuf>,
        /// Save a JSON run report under the resource folder's `RABBIT`
        /// directory, so there is a record of what happened.
        #[arg(long)]
        save_report: bool,
    },
}

/// Shown below `rabbit uninstall --help`. Every other command that changes
/// files has `--allow-reaper-running`; this says why uninstall doesn't.
const UNINSTALL_AFTER_HELP: &str = "\
REAPER must be closed. For REAPER on Windows, Surge XT, app2clap and the JAWS \
scripts, every REAPER on the computer must be closed.

There is no --allow-reaper-running, unlike the commands that install: a \
running REAPER holds its extensions' files open, so they can't be moved, and \
it writes reaper.ini again when it closes, which would undo RABBIT setting \
REAPER's language back.

Removed files move into a backup set. `rabbit restore-backup --backup-id <id> \
--package <id> --apply` brings one package back.

Exit codes: 0 when every package asked for was removed or isn't installed; 2 \
when one of them can't be uninstalled, and then nothing changes; 1 when a \
removal failed or was stopped.";

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
enum SelfUpdateCommand {
    /// Report whether a newer RABBIT has been published, and what changed.
    Check {
        /// Release manifest to check. Defaults to RABBIT's own.
        #[arg(long, default_value = DEFAULT_SELF_UPDATE_MANIFEST_URL)]
        manifest_url: String,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Download and verify an update without installing it yet.
    Stage {
        /// Release manifest to update from. Defaults to RABBIT's own.
        #[arg(long, default_value = DEFAULT_SELF_UPDATE_MANIFEST_URL)]
        manifest_url: String,
        /// Where to put the downloaded update. Defaults to a folder
        /// alongside the current install.
        #[arg(long)]
        staging_dir: Option<PathBuf>,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Install a staged update over the current RABBIT.
    Apply {
        /// Release manifest to update from. Defaults to RABBIT's own.
        #[arg(long, default_value = DEFAULT_SELF_UPDATE_MANIFEST_URL)]
        manifest_url: String,
        /// Where the staged update was downloaded to, if not the default.
        #[arg(long)]
        staging_dir: Option<PathBuf>,
        /// Which RABBIT installation to replace, if not the running one.
        #[arg(long)]
        install_root: Option<PathBuf>,
        /// Relaunch RABBIT once the update is installed.
        #[arg(long)]
        restart: bool,
        /// Print the result as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliArchitecture {
    X86,
    X64,
    Arm64,
    Arm64Ec,
    Universal,
}

impl From<CliArchitecture> for Architecture {
    fn from(value: CliArchitecture) -> Self {
        match value {
            CliArchitecture::X86 => Self::X86,
            CliArchitecture::X64 => Self::X64,
            CliArchitecture::Arm64 => Self::Arm64,
            CliArchitecture::Arm64Ec => Self::Arm64Ec,
            CliArchitecture::Universal => Self::Universal,
        }
    }
}

/// Parse the process argv via clap and dispatch to the matching subcommand.
/// Used by the merged `rabbit` binary when it sees CLI arguments.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    run_args(std::env::args_os().collect())
}

/// The stack of the thread the CLI runs on. Debug builds give every
/// temporary its own stack slot, and clap builds every command's flags in
/// one generated function: parsing alone took more than the 1 MB Windows
/// gives the main thread once `uninstall` arrived. Release builds need a
/// fraction of this; reserving it costs nothing until it is used.
const CLI_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Parse `args` and run the command on a thread with [`CLI_STACK_SIZE`] of
/// stack, so neither the parser nor a command depends on how much the
/// main thread happens to have.
fn run_args(args: Vec<std::ffi::OsString>) -> Result<(), Box<dyn std::error::Error>> {
    let outcome = std::thread::Builder::new()
        .name("rabbit-cli".to_string())
        .stack_size(CLI_STACK_SIZE)
        .spawn(move || run_with(Cli::parse_from(args)).map_err(|error| error.to_string()))?
        .join()
        // A panic carries on as if it had happened on the calling thread.
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    outcome.map_err(Into::into)
}

/// Each command runs in a function of its own. As arms of one match, a
/// debug build gave every command's locals a slot in the same stack frame,
/// more than the 1 MB stack Windows gives the main thread, so every command
/// overflowed before doing anything.
fn run_with(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        command @ Command::Detect { .. } => run_detect(command),
        command @ Command::Components { .. } => run_components(command),
        command @ Command::Latest { .. } => run_latest(command),
        command @ Command::Artifacts { .. } => run_artifacts(command),
        command @ Command::Download { .. } => run_download(command),
        command @ Command::Packages { .. } => run_packages(command),
        command @ Command::Preflight { .. } => run_preflight(command),
        command @ Command::InitResource { .. } => run_init_resource(command),
        command @ Command::Backups { .. } => run_backups(command),
        command @ Command::RestoreBackup { .. } => run_restore_backup(command),
        command @ Command::InstallExtension { .. } => run_install_extension(command),
        command @ Command::ApplyPackages { .. } => run_apply_packages(command),
        command @ Command::Setup { .. } => run_setup(command),
        command @ Command::Uninstall { .. } => run_uninstall(command),
        command @ Command::Locales { .. } => run_locales(command),
        command @ Command::Localize { .. } => run_localize(command),
        command @ Command::PortableCheck { .. } => run_portable_check(command),
        command @ Command::SelfUpdate { .. } => run_self_update(command),
        command @ Command::Plan { .. } => run_plan(command),
    }
}

fn run_detect(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Detect { portable, json } = command else {
        unreachable!("dispatched as Command::Detect");
    };
    let installations = discover_installations(&DiscoveryOptions {
        include_standard: true,
        portable_roots: portable,
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&installations)?);
    } else {
        print_installations(&installations);
    }
    Ok(())
}

fn run_components(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Components {
        resource_path,
        json,
    } = command
    else {
        unreachable!("dispatched as Command::Components");
    };
    let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
    let components = detect_components(&resource_path, platform)?;
    // What RABBIT could uninstall here. A receipt that can't be read only
    // costs these lines; detection has already said what is installed.
    let plan = plan_uninstall_at(&resource_path, platform, &components).ok();
    if json {
        let listed: Vec<ComponentListing<'_>> = components
            .iter()
            .map(|component| ComponentListing {
                component,
                uninstall: plan
                    .as_ref()
                    .and_then(|plan| plan.item(&component.package_id))
                    .map(UninstallAbility::of),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&listed)?);
    } else {
        print_components(&components, plan.as_ref());
    }
    Ok(())
}

/// One package in `components --json`: its detection as it always was, and
/// whether RABBIT can uninstall it.
#[derive(Debug, Serialize)]
struct ComponentListing<'a> {
    #[serde(flatten)]
    component: &'a ComponentDetection,
    #[serde(skip_serializing_if = "Option::is_none")]
    uninstall: Option<UninstallAbility>,
}

/// Whether RABBIT can uninstall a package right now, and if not, why.
#[derive(Debug, Serialize)]
struct UninstallAbility {
    /// `rabbit uninstall --package <id> --apply` would remove it.
    can_uninstall: bool,
    removability: Removability,
}

impl UninstallAbility {
    fn of(item: &UninstallPlanItem) -> Self {
        Self {
            can_uninstall: item.removability.is_removable() && !item.unbacked,
            removability: item.removability.clone(),
        }
    }
}

fn run_latest(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Latest {
        json,
        package_channel,
    } = command
    else {
        unreachable!("dispatched as Command::Latest");
    };
    let channels = parse_package_channels(&package_channel)?;
    let latest = fetch_latest_versions_on(&channels)?;
    warn_channel_fallbacks(
        &channels,
        latest
            .packages
            .iter()
            .map(|package| (package.package_id.as_str(), package.channel.as_deref())),
    );
    if json {
        println!("{}", serde_json::to_string_pretty(&latest)?);
    } else {
        print_latest(&latest.packages);
        // Failed providers (e.g. the SWS homepage being down) go to
        // stderr so scripts parsing stdout see only resolved versions.
        for failure in &latest.failures {
            eprintln!(
                "warning: could not check the latest version of {}: {}",
                failure.package_id, failure.message
            );
        }
    }
    Ok(())
}

fn run_artifacts(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Artifacts {
        package,
        architecture,
        json,
        package_channel,
    } = command
    else {
        unreachable!("dispatched as Command::Artifacts");
    };
    let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
    let architecture = architecture.map_or_else(Architecture::current, Into::into);
    let packages = select_packages(package, None).package_ids;
    let channels = parse_package_channels(&package_channel)?;
    let artifacts = resolve_latest_artifacts_on(&packages, platform, architecture, &channels)?;
    warn_channel_fallbacks(&channels, artifact_channels(&artifacts));
    if json {
        println!("{}", serde_json::to_string_pretty(&artifacts)?);
    } else {
        print_artifacts(&artifacts);
    }
    Ok(())
}

fn run_download(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Download {
        package,
        architecture,
        cache_dir,
        json,
        package_channel,
    } = command
    else {
        unreachable!("dispatched as Command::Download");
    };
    let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
    let architecture = architecture.map_or_else(Architecture::current, Into::into);
    let packages = select_packages(package, None).package_ids;
    let channels = parse_package_channels(&package_channel)?;
    let artifacts = resolve_latest_artifacts_on(&packages, platform, architecture, &channels)?;
    warn_channel_fallbacks(&channels, artifact_channels(&artifacts));
    let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
    let cached = download_artifacts(&artifacts, &cache_dir)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&cached)?);
    } else {
        print_cached_artifacts(&cached);
    }
    Ok(())
}

fn run_packages(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Packages { manifest, json } = command else {
        unreachable!("dispatched as Command::Packages");
    };
    if manifest {
        let manifest = embedded_package_manifest();
        if json {
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        } else {
            println!("Schema version: {}", manifest.schema_version);
            for package in &manifest.packages {
                println!("{}", package.id);
                println!("  Display name: {}", package.display_name);
                println!("  Kind: {}", serialized_name(&package.package_kind));
                println!("  Required: {}", yes_no(package.required));
                println!("  Recommended: {}", yes_no(package.recommended));
                println!("  Channels: {}", channel_names(&package.channels));
                println!(
                    "  Supported platforms: {}",
                    serialized_names(&package.supported_platforms)
                );
                println!(
                    "  Supported architectures: {}",
                    serialized_names(&package.supported_architectures)
                );
                println!(
                    "  Artifact source: {}",
                    artifact_source_label(
                        package.github_release.is_some(),
                        package.http_artifact.is_some(),
                        package.hfs_listing.is_some(),
                    )
                );
                println!("  Detectors: {}", serialized_names(&package.detectors));
                println!(
                    "  Install steps: {}",
                    serialized_names(&package.install_steps)
                );
                print_uninstall_plans(
                    &package.uninstall,
                    &package.supported_platforms,
                    package.requires_standard_install,
                    None,
                );
                println!(
                    "  Backup policy: {}",
                    serialized_name(&package.backup_policy)
                );
                println!(
                    "  Windows suffixes: {}",
                    string_names(&package.user_plugin_suffixes.windows)
                );
                println!(
                    "  macOS suffixes: {}",
                    string_names(&package.user_plugin_suffixes.macos)
                );
            }
        }
    } else {
        let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
        let packages = builtin_package_specs(platform);
        if json {
            println!("{}", serde_json::to_string_pretty(&packages)?);
        } else {
            print_package_specs(&packages);
        }
    }
    Ok(())
}

fn run_preflight(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Preflight {
        resource_path,
        target_app_path,
        dry_run,
        allow_reaper_running,
        report_path,
        save_report,
        json,
    } = command
    else {
        unreachable!("dispatched as Command::Preflight");
    };
    let report = run_install_preflight(
        &resource_path,
        &PreflightOptions {
            dry_run,
            allow_reaper_running,
            target_app_path,
        },
    );
    let report_path =
        selected_report_path(Some(&resource_path), report_path, save_report, "preflight")?;
    save_optional_report(report_path.as_deref(), &report)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_preflight_report(&report);
    }
    if !report.passed {
        std::process::exit(2);
    }
    Ok(())
}

fn run_init_resource(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::InitResource {
        resource_path,
        target_app_path,
        portable,
        apply,
        allow_reaper_running,
        report_path,
        save_report,
        json,
    } = command
    else {
        unreachable!("dispatched as Command::InitResource");
    };
    let report = initialize_resource_path(
        &resource_path,
        &rabbit_core::resource::ResourceInitOptions {
            dry_run: !apply,
            portable,
            include_extension_support_dirs: true,
            allow_reaper_running,
            target_app_path,
        },
    )?;
    let report_path = selected_report_path(
        Some(&resource_path),
        report_path,
        save_report,
        "init-resource",
    )?;
    save_optional_report(report_path.as_deref(), &report)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_resource_init_report(&report);
    }
    Ok(())
}

fn run_backups(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Backups {
        resource_path,
        json,
    } = command
    else {
        unreachable!("dispatched as Command::Backups");
    };
    let backup_sets = list_backup_sets(&resource_path)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&backup_sets)?);
    } else {
        print_backup_sets(&backup_sets);
    }
    Ok(())
}

fn run_restore_backup(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::RestoreBackup {
        resource_path,
        backup_id,
        package,
        apply,
        allow_reaper_running,
        report_path,
        save_report,
        json,
    } = command
    else {
        unreachable!("dispatched as Command::RestoreBackup");
    };
    let report = restore_backup_set(
        &resource_path,
        &backup_id,
        &RestoreBackupOptions {
            dry_run: !apply,
            allow_reaper_running,
            package_id: package.map(|id| id.trim().to_string()),
        },
    )?;
    let report_path = selected_report_path(
        Some(&resource_path),
        report_path,
        save_report,
        "restore-backup",
    )?;
    save_optional_report(report_path.as_deref(), &report)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_restore_backup_report(&report);
    }
    Ok(())
}

fn run_install_extension(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::InstallExtension {
        resource_path,
        target_app_path,
        package,
        architecture,
        cache_dir,
        apply,
        allow_reaper_running,
        accept_reapack_donation_notice,
        report_path,
        save_report,
        json,
        package_channel,
    } = command
    else {
        unreachable!("dispatched as Command::InstallExtension");
    };
    ensure_reapack_donation_acknowledged(&package, accept_reapack_donation_notice)?;
    let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
    let architecture = architecture.map_or_else(Architecture::current, Into::into);
    let channels = effective_package_channels(Some(&resource_path), &package, &package_channel)?;
    let artifacts = resolve_latest_artifacts_on(&package, platform, architecture, &channels)?;
    warn_channel_fallbacks(&channels, artifact_channels(&artifacts));
    // Without --apply, nothing is downloaded: every other command
    // treats "no --apply" as "change nothing on this machine", and a
    // download is a change to it. There is no file to install from
    // and so no install report; what there is to say is which
    // artifacts would be fetched, and whether the target is ready
    // for them.
    if !apply {
        let preflight = run_install_preflight(
            &resource_path,
            &PreflightOptions {
                dry_run: true,
                allow_reaper_running,
                target_app_path: target_app_path.clone(),
            },
        );
        let planned = PlannedInstall {
            resource_path: resource_path.clone(),
            dry_run: true,
            preflight,
            artifacts,
        };
        let report_path = selected_report_path(
            Some(&resource_path),
            report_path,
            save_report,
            "install-extension",
        )?;
        save_optional_report(report_path.as_deref(), &planned)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&planned)?);
        } else {
            print_preflight_report(&planned.preflight);
            print_artifacts(&planned.artifacts);
            println!(
                "Dry run: nothing was downloaded or installed. Pass --apply to install, \
                 or use `rabbit download` to fetch without installing."
            );
        }
        forget_refusals_of_named(&resource_path, &package, false);
        return Ok(());
    }
    let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
    let cached = download_artifacts(&artifacts, &cache_dir)?;
    let report = install_cached_artifacts(
        &resource_path,
        &cached,
        &InstallOptions {
            dry_run: false,
            allow_reaper_running,
            target_app_path,
            package_variants: Default::default(),
        },
    )?;
    forget_refusals_of_named(&resource_path, &package, true);
    let report_path = selected_report_path(
        Some(&resource_path),
        report_path,
        save_report,
        "install-extension",
    )?;
    save_optional_report(report_path.as_deref(), &report)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_install_report(&report);
    }
    Ok(())
}

fn run_apply_packages(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::ApplyPackages {
        resource_path,
        target_app_path,
        package,
        architecture,
        cache_dir,
        apply,
        allow_reaper_running,
        stage_unsupported,
        preserve_osara_keymap,
        replace_osara_keymap,
        package_variant,
        accept_reapack_donation_notice,
        report_path,
        save_report,
        json,
        package_channel,
    } = command
    else {
        unreachable!("dispatched as Command::ApplyPackages");
    };
    let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
    let architecture = architecture.map_or_else(Architecture::current, Into::into);
    let selection = select_packages(package, Some(&resource_path));
    note_skipped_packages(&selection, platform);
    let packages = selection.package_ids.clone();
    ensure_reapack_donation_acknowledged(&packages, accept_reapack_donation_notice)?;
    let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
    let channels = effective_package_channels(Some(&resource_path), &packages, &package_channel)?;
    let keymap = osara_keymap_choice(
        &resource_path,
        platform,
        replace_osara_keymap,
        preserve_osara_keymap,
    );
    let report = execute_package_operation(
        &resource_path,
        &packages,
        platform,
        architecture,
        &cache_dir,
        &PackageOperationOptions {
            dry_run: !apply,
            allow_reaper_running,
            stage_unsupported,
            replace_osara_keymap: keymap.replace,
            package_variants: parse_package_variants(&package_variant)?,
            package_channels: channels.clone(),
            target_app_path,
            lock_path: None,
            force_reinstall_packages: Vec::new(),
        },
    )?;
    warn_channel_fallbacks(&channels, item_channels(&report.items));
    explain_osara_keymap_choice(&keymap, &report.items);
    if selection.named {
        forget_refusals_of_named(&resource_path, &packages, apply);
    }
    let report_path = selected_report_path(
        Some(&resource_path),
        report_path,
        save_report,
        "apply-packages",
    )?;
    save_optional_report(report_path.as_deref(), &report)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_package_operation_report(&report);
    }
    // Everything installable was attempted; a per-package failure
    // still yields a non-zero exit for scripts/CI. Distinct from the
    // hard-error path (Err) and from preflight's exit(2).
    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}

fn run_setup(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Setup {
        resource_path,
        target_app_path,
        portable,
        package,
        architecture,
        cache_dir,
        apply,
        allow_reaper_running,
        stage_unsupported,
        preserve_osara_keymap,
        replace_osara_keymap,
        package_variant,
        package_channel,
        accept_reapack_donation_notice,
        config_step,
        skip_config_step,
        reaper_language,
        report_path,
        save_report,
        json,
    } = command
    else {
        unreachable!("dispatched as Command::Setup");
    };
    let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
    let architecture = architecture.map_or_else(Architecture::current, Into::into);
    let selection = select_packages(package, Some(&resource_path));
    note_skipped_packages(&selection, platform);
    let packages = selection.package_ids.clone();
    ensure_reapack_donation_acknowledged(&packages, accept_reapack_donation_notice)?;
    let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
    let configuration_step_ids = resolve_configuration_step_ids(
        &resource_path,
        platform,
        &packages,
        &config_step,
        &skip_config_step,
    );
    let channels = effective_package_channels(Some(&resource_path), &packages, &package_channel)?;
    let keymap = osara_keymap_choice(
        &resource_path,
        platform,
        replace_osara_keymap,
        preserve_osara_keymap,
    );
    let report = execute_setup_operation(
        &resource_path,
        &packages,
        platform,
        architecture,
        &cache_dir,
        &SetupOptions {
            dry_run: !apply,
            portable,
            allow_reaper_running,
            stage_unsupported,
            replace_osara_keymap: keymap.replace,
            package_variants: parse_package_variants(&package_variant)?,
            package_channels: channels.clone(),
            target_app_path,
            lock_path: None,
            force_reinstall_packages: Vec::new(),
            reaper_language_package: validate_reaper_language(reaper_language)?,
            configuration_step_ids,
        },
    )?;
    warn_channel_fallbacks(&channels, item_channels(&report.package_operation.items));
    explain_osara_keymap_choice(&keymap, &report.package_operation.items);
    if selection.named {
        forget_refusals_of_named(&resource_path, &packages, apply);
    }
    let report_path =
        selected_report_path(Some(&resource_path), report_path, save_report, "setup")?;
    save_optional_report(report_path.as_deref(), &report)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_setup_report(&report);
    }
    if report.package_operation.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}

fn run_uninstall(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Uninstall {
        resource_path,
        target_app_path,
        package,
        remove_settings,
        apply,
        report_path,
        save_report,
        json,
    } = command
    else {
        unreachable!("dispatched as Command::Uninstall");
    };
    let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
    let package_ids = validate_uninstall_packages(&package, platform)?;
    let report = uninstall_report(
        &UninstallRequest {
            resource_path: resource_path.clone(),
            platform,
            package_ids,
            remove_settings,
            dry_run: !apply,
            target_app_path,
            target: None,
        },
        &CancelToken::new(),
    )?;
    // A run with --apply saved its report under RABBIT/logs by itself;
    // `--save-report` only adds one where that didn't happen, such as a dry
    // run.
    let report_path = if report.saved_report.is_some() && report_path.is_none() {
        None
    } else {
        selected_report_path(Some(&resource_path), report_path, save_report, "uninstall")?
    };
    save_optional_report(report_path.as_deref(), &report)?;
    if let Some(saved) = &report.saved_report {
        eprintln!("Report saved (JSON): {}", saved.json_path.display());
        eprintln!("Report saved (text): {}", saved.text_path.display());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_uninstall_report(&report, cli_localizer().as_ref());
    }
    let code = uninstall_exit_code(&report);
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// Carry out (or with `dry_run`, only plan) an uninstall, and narrow the
/// report's plan to the packages asked for: the plan covers every package
/// on the target, and the output is about these.
fn uninstall_report(
    request: &UninstallRequest,
    cancel: &CancelToken,
) -> Result<UninstallReport, Box<dyn std::error::Error>> {
    let mut report = execute_uninstall(request, &ProgressReporter::noop(), cancel)?;
    report
        .plan
        .items
        .retain(|item| request.package_ids.contains(&item.package_id));
    Ok(report)
}

/// The `--package` ids for `uninstall`, each once, in the order given. An id
/// no package on this platform has is refused before anything runs, so a
/// typo can't pass for "nothing to remove".
fn validate_uninstall_packages(
    ids: &[String],
    platform: Platform,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let offered: Vec<String> = builtin_package_specs(platform)
        .into_iter()
        .map(|spec| spec.id)
        .collect();
    let mut packages: Vec<String> = Vec::new();
    for id in ids {
        let id = id.trim();
        if !offered.iter().any(|known| known == id) {
            let elsewhere = embedded_package_manifest()
                .packages
                .iter()
                .any(|spec| spec.id == id);
            return Err(if elsewhere {
                format!(
                    "package {id:?} isn't offered on {}, so there is nothing of it to uninstall",
                    platform_name(platform)
                )
            } else {
                format!(
                    "--package names unknown package {id:?} (available: {})",
                    offered.join(", ")
                )
            }
            .into());
        }
        if !packages.iter().any(|known| known == id) {
            packages.push(id.to_string());
        }
    }
    Ok(packages)
}

/// The exit code for an uninstall run. The same table serves the dry run
/// and `--apply`:
/// - 1: a package failed or was stopped, or the run stopped after it had
///   already removed something;
/// - 2: a package asked for can't be uninstalled, and nothing changed;
/// - 0: everything else, including packages that aren't installed (so a
///   script can run twice) and removals that left something behind.
///
/// A hard error before any change, such as another RABBIT holding the
/// lock, is returned as `Err` before this is asked.
fn uninstall_exit_code(report: &UninstallReport) -> i32 {
    if report.has_failures() {
        1
    } else if report.blocked_before_changes() {
        2
    } else {
        0
    }
}

fn run_locales(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Locales { locales_dir, json } = command else {
        unreachable!("dispatched as Command::Locales");
    };
    let locales = available_locales(&locales_dir)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&locales)?);
    } else {
        print_locales(&locales);
    }
    Ok(())
}

fn run_localize(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Localize {
        locale,
        locales_dir,
        id,
        args,
        json,
    } = command
    else {
        unreachable!("dispatched as Command::Localize");
    };
    let localizer = Localizer::from_locale_dir(&locales_dir, &locale)?;
    let owned_args = parse_localization_args(args)?;
    let borrowed_args = owned_args
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let message = localizer.format(&id, &borrowed_args);
    if json {
        println!("{}", serde_json::to_string_pretty(&message)?);
    } else {
        print_localized_text(&message);
    }
    Ok(())
}

fn run_portable_check(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::PortableCheck { locales_dir, json } = command else {
        unreachable!("dispatched as Command::PortableCheck");
    };
    let report = check_portable_runtime(&locales_dir)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_portability_report(&report);
    }
    Ok(())
}

fn run_self_update(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::SelfUpdate { command } = command else {
        unreachable!("dispatched as Command::SelfUpdate");
    };
    match command {
        SelfUpdateCommand::Check { manifest_url, json } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let report = check_self_update(platform, &manifest_url)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_self_update_report(&report);
                // Only the check command pays for the notes fetch: stage
                // and apply print the same report, and neither should
                // spend a round-trip on text nobody asked to read while
                // an update is being installed.
                print_self_update_release_notes(&report);
            }
        }
        SelfUpdateCommand::Stage {
            manifest_url,
            staging_dir,
            json,
        } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let staging_dir = staging_dir.unwrap_or_else(default_self_update_staging_dir);
            let report = stage_self_update(platform, &manifest_url, &staging_dir)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_self_update_stage_report(&report);
            }
        }
        SelfUpdateCommand::Apply {
            manifest_url,
            staging_dir,
            install_root,
            restart,
            json,
        } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let staging_dir = staging_dir.unwrap_or_else(default_self_update_staging_dir);
            let stage = stage_self_update(platform, &manifest_url, &staging_dir)?;
            let report = apply_self_update(
                &stage,
                &ApplySelfUpdateOptions {
                    install_root,
                    install_target_basename: None,
                    ..Default::default()
                },
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_self_update_apply_report(&report);
            }
            if restart && !report.replaced_files.is_empty() {
                let pid = relaunch_current_executable()?;
                if !json {
                    println!("Relaunched RABBIT with PID {pid}; exiting current process.");
                }
                return Ok(());
            }
        }
    }
    Ok(())
}

fn run_plan(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Plan {
        resource_path,
        portable,
        online,
        format,
        report_path,
        save_report,
        package_channel,
    } = command
    else {
        unreachable!("dispatched as Command::Plan");
    };
    let platform = Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
    let installations = discover_installations(&DiscoveryOptions {
        include_standard: true,
        portable_roots: portable,
    })?;
    let explicit_resource_path = resource_path.clone();
    let target = match resource_path.as_ref() {
        Some(path) => installations
            .iter()
            .find(|installation| installation.resource_path == *path)
            .cloned(),
        None => installations.first().cloned(),
    };
    let plan_report_resource_path = resource_path.clone().or_else(|| {
        target
            .as_ref()
            .map(|installation| installation.resource_path.clone())
    });
    let detection_path = explicit_resource_path.or_else(|| {
        target
            .as_ref()
            .map(|installation| installation.resource_path.clone())
    });
    let components = match detection_path.as_ref() {
        Some(path) => detect_components(path, platform)?,
        None => Vec::new(),
    };

    let desired = default_desired_package_ids();
    let channels =
        effective_package_channels(detection_path.as_deref(), &desired, &package_channel)?;
    let (available, version_check_failures) = if online {
        let report = fetch_latest_versions_on(&channels)?;
        (report.packages, report.failures)
    } else {
        (Vec::new(), Vec::new())
    };
    let installed_channels = detection_path
        .as_deref()
        .map(rabbit_core::receipt::installed_channels_at)
        .unwrap_or_default();
    let mut plan = build_install_plan_with_installed_channels(
        target,
        &components,
        &desired,
        &available,
        &installed_channels,
    );
    // Surface per-provider failures as plan notes instead of failing
    // the whole plan: one unreachable upstream shouldn't block update
    // guidance for everything else.
    for failure in &version_check_failures {
        plan.notes.push(format!(
            "The latest-version check for {} failed: {}. Install/update guidance for this package is incomplete in this plan.",
            failure.package_id, failure.message
        ));
    }
    plan.notes.extend(channel_fallbacks(
        &channels,
        available
            .iter()
            .map(|package| (package.package_id.as_str(), package.channel.as_deref())),
    ));
    let report_path = selected_report_path(
        plan_report_resource_path.as_deref(),
        report_path,
        save_report,
        "plan",
    )?;
    save_optional_report(report_path.as_deref(), &plan)?;
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
        OutputFormat::Text => print_plan(&plan),
    }
    Ok(())
}

fn parse_localization_args(args: Vec<String>) -> rabbit_core::Result<Vec<(String, String)>> {
    args.into_iter()
        .map(|arg| {
            let Some((name, value)) = arg.split_once('=') else {
                return Err(rabbit_core::RabbitError::Localization {
                    path: None,
                    message: format!("localization argument must use name=value: {arg}"),
                });
            };
            if name.is_empty() {
                return Err(rabbit_core::RabbitError::Localization {
                    path: None,
                    message: format!("localization argument name is empty: {arg}"),
                });
            }
            Ok((name.to_string(), value.to_string()))
        })
        .collect()
}

/// The packages an operation acts on.
struct PackageSelection {
    package_ids: Vec<String>,
    /// The user named them with `--package`.
    named: bool,
    /// Packages the default list left out, because the user uninstalled
    /// them or turned them down.
    skipped: Vec<String>,
}

/// The packages an operation should act on.
///
/// An explicit `--package` list is taken as given — it is the user telling
/// us exactly what they want, and second-guessing it would be worse than
/// useless. Only the implicit default set is filtered against the packages
/// this install remembers the user uninstalling or turning down, every one
/// of them, so neither a refusal recorded in the wizard nor an uninstall is
/// undone by a later bare `rabbit setup`.
fn select_packages(package_ids: Vec<String>, resource_path: Option<&Path>) -> PackageSelection {
    if !package_ids.is_empty() {
        return PackageSelection {
            package_ids,
            named: true,
            skipped: Vec::new(),
        };
    }
    let declined = resource_path
        .map(rabbit_core::receipt::declined_packages)
        .unwrap_or_default();
    let (skipped, package_ids) = default_desired_package_ids()
        .into_iter()
        .partition(|id| declined.contains(id));
    PackageSelection {
        package_ids,
        named: false,
        skipped,
    }
}

/// Say which packages the default list left out, and how to get each one
/// back. On stderr, like the other notes, so `--json` stays JSON.
fn note_skipped_packages(selection: &PackageSelection, platform: Platform) {
    let specs = builtin_package_specs(platform);
    for id in &selection.skipped {
        let name = specs
            .iter()
            .find(|spec| spec.id == *id)
            .map_or(id.as_str(), |spec| spec.display_name.as_str());
        eprintln!(
            "note: skipping {name}: you uninstalled it or turned it down. Pass --package {id} \
             to install it."
        );
    }
}

/// A package named with `--package` is the user saying they want it, so a
/// run that installs takes it off the packages they uninstalled or turned
/// down, and later runs offer it again. That holds even when its install
/// fails: the answer was still yes. A dry run only says it would.
fn forget_refusals_of_named(resource_path: &Path, package_ids: &[String], apply: bool) {
    let declined = rabbit_core::receipt::declined_packages(resource_path);
    let named: Vec<String> = package_ids
        .iter()
        .filter(|id| declined.contains(*id))
        .cloned()
        .collect();
    if named.is_empty() {
        return;
    }
    if !apply {
        for id in &named {
            eprintln!(
                "note: {id} is on the list of packages you uninstalled or turned down. With \
                 --apply, naming it takes it off, so later runs offer it again."
            );
        }
        return;
    }
    match rabbit_core::receipt::record_package_opt_outs(resource_path, &[], &named) {
        Ok(()) => {
            for id in &named {
                eprintln!(
                    "note: {id} is no longer on the list of packages you uninstalled or turned \
                     down."
                );
            }
        }
        // A preference is never worth failing a finished install over.
        Err(error) => eprintln!(
            "warning: could not take {} off the list of packages you uninstalled or turned \
             down: {error}",
            named.join(", ")
        ),
    }
}

/// Pick the configuration steps the CLI should run.
///
/// CLI rules:
/// - When `--config-step <id>` is passed (one or more times), the
///   resolved set is exactly that allowlist — `--skip-config-step` is
///   ignored. Steps whose dependency is not satisfied still run through
///   the same `SkippedDependencyMissing` path the wizard takes; the
///   resolver here only chooses which ids the setup pipeline considers.
/// - Otherwise, the resolver defaults to "every recommended step whose
///   dependency package is either in this run's `--package` list or
///   already detected on disk", minus anything in `--skip-config-step`.
///
/// This mirrors the wizard's auto-tick-when-recommended behaviour, so
/// CLI users get the same default outcome (ReaPack remote added when
/// ReaPack is part of the install) without having to know the step ids.
fn resolve_configuration_step_ids(
    resource_path: &Path,
    platform: rabbit_core::model::Platform,
    package_ids: &[String],
    explicit: &[String],
    skip: &[String],
) -> Vec<String> {
    use std::collections::BTreeSet;
    let skip_set: BTreeSet<&str> = skip.iter().map(String::as_str).collect();
    if !explicit.is_empty() {
        return explicit
            .iter()
            .filter(|id| !skip_set.contains(id.as_str()))
            .cloned()
            .collect();
    }

    let requested: BTreeSet<String> = package_ids.iter().cloned().collect();
    let mut installed_or_pending: BTreeSet<String> = requested.clone();
    if let Ok(detections) = rabbit_core::detection::detect_components(resource_path, platform) {
        for detection in detections {
            if detection.installed {
                installed_or_pending.insert(detection.package_id);
            }
        }
    }

    rabbit_core::configuration::builtin_configuration_steps()
        .into_iter()
        .filter(|step| step.recommended && !skip_set.contains(step.id.as_str()))
        .filter(|step| {
            // ANY-of: "set REAPER's language" lists every language pack, so
            // one pack is enough to make the step relevant.
            //
            // Steps flagged `requires_fresh_dependency` are matched
            // against the packages THIS RUN asks for, not against whatever
            // is already on disk. Otherwise `rabbit setup` on a machine
            // that happens to have a language pack installed would switch
            // REAPER's interface language without being asked. Such a step
            // is still reachable with an explicit `--config-step`.
            let candidates: &BTreeSet<String> = if step.requires_fresh_dependency {
                &requested
            } else {
                &installed_or_pending
            };
            step.requires_packages.is_empty()
                || step
                    .requires_packages
                    .iter()
                    .any(|pkg| candidates.contains(pkg))
        })
        .filter(|step| {
            // Skip recommended steps that are already in place — they'd
            // be a no-op. Users can still force a re-run via explicit
            // `--config-step <id>` (the early `if !explicit.is_empty()`
            // branch above bypasses this filter).
            !rabbit_core::configuration::is_configuration_step_applied(
                resource_path,
                step,
                &Default::default(),
            )
            .unwrap_or(false)
        })
        .map(|step| step.id)
        .collect()
}

/// Refuse to proceed when ReaPack is in the user's package selection but
/// the donation acknowledgement flag is missing. Mirrors the GUI's dedicated
/// ReaPack ack page: the user must explicitly opt in before RABBIT stages
/// or launches the ReaPack install/update.
fn ensure_reapack_donation_acknowledged(
    package_ids: &[String],
    accepted: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if accepted {
        return Ok(());
    }
    if !package_ids
        .iter()
        .any(|id| id == rabbit_core::package::PACKAGE_REAPACK)
    {
        return Ok(());
    }
    Err(
        "ReaPack is in this run's plan but the donation acknowledgement is missing. \
         Re-run with --accept-reapack-donation-notice to confirm you have read \
         https://reapack.com/donate and want RABBIT to install or update ReaPack."
            .into(),
    )
}

/// What `install-extension` reports without `--apply`: the artifacts it
/// would download and install, and whether the target is ready for them.
/// Not an [`InstallReport`], because nothing was downloaded and so nothing
/// was planned file by file.
#[derive(Debug, serde::Serialize)]
struct PlannedInstall {
    resource_path: PathBuf,
    dry_run: bool,
    preflight: PreflightReport,
    artifacts: Vec<ArtifactDescriptor>,
}

fn save_optional_report<T>(
    report_path: Option<&Path>,
    report: &T,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: serde::Serialize + ?Sized,
{
    if let Some(report_path) = report_path {
        let saved = save_json_and_text_reports(report_path, report)?;
        eprintln!("Report saved (JSON): {}", saved.json_path.display());
        eprintln!("Report saved (text): {}", saved.text_path.display());
    }
    Ok(())
}

fn selected_report_path(
    resource_path: Option<&Path>,
    explicit_report_path: Option<PathBuf>,
    save_report: bool,
    operation_name: &str,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    if let Some(path) = explicit_report_path {
        return Ok(Some(path));
    }

    if save_report {
        let Some(resource_path) = resource_path else {
            let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--save-report requires a selected resource path",
            );
            return Err(Box::new(error));
        };
        return Ok(Some(default_report_path(resource_path, operation_name)));
    }

    Ok(None)
}

fn print_installations(installations: &[rabbit_core::model::Installation]) {
    if installations.is_empty() {
        println!("No REAPER installations detected.");
        return;
    }

    for (index, installation) in installations.iter().enumerate() {
        println!("Installation {}", index + 1);
        println!("  Type: {:?}", installation.kind);
        println!("  App: {}", installation.app_path.display());
        println!("  Resource path: {}", installation.resource_path.display());
        println!(
            "  Version: {}",
            installation
                .version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown".to_string())
        );
        println!(
            "  Architecture: {}",
            installation
                .architecture
                .as_ref()
                .map(|architecture| format!("{architecture:?}"))
                .unwrap_or_else(|| "unknown".to_string())
        );
        println!("  Writable: {}", yes_no(installation.writable));
        println!("  Confidence: {:?}", installation.confidence);
        println!();
    }
}

fn print_components(components: &[ComponentDetection], plan: Option<&UninstallPlan>) {
    for component in components {
        println!("{}", component.display_name);
        println!("  Installed: {}", yes_no(component.installed));
        if let Some(item) = plan.and_then(|plan| plan.item(&component.package_id)) {
            println!("  Uninstall: {}", uninstall_ability_text(item));
        }
        println!(
            "  Version: {}",
            component
                .version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown".to_string())
        );
        println!("  Detector: {}", component.detector);
        if !component.files.is_empty() {
            println!("  Files:");
            for file in &component.files {
                println!("    {}", file.display());
            }
        }
        for note in &component.notes {
            println!("  Note: {note}");
        }
        println!();
    }
}

/// Whether RABBIT can uninstall a package, and if not, why, for one line of
/// `rabbit components`.
fn uninstall_ability_text(item: &UninstallPlanItem) -> String {
    let name = &item.display_name;
    match &item.removability {
        Removability::NotInstalled => "nothing to remove, it isn't installed".to_string(),
        removability if removability.is_removable() && item.unbacked => {
            format!("not yet: removing {name} takes a step a backup can't undo")
        }
        removability if removability.is_removable() => {
            format!("yes, with rabbit uninstall --package {}", item.package_id)
        }
        Removability::Manual { .. } => format!(
            "by hand only; rabbit uninstall --package {} says how",
            item.package_id
        ),
        removability => format!("no. {}", removability.message(name)),
    }
}

fn print_latest(latest: &[AvailablePackage]) {
    for package in latest {
        println!(
            "{}: {}",
            package.package_id,
            package
                .version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown".to_string())
        );
    }
}

fn print_artifacts(artifacts: &[ArtifactDescriptor]) {
    for artifact in artifacts {
        println!("{}", artifact.package_id);
        println!("  Version: {}", artifact.version);
        println!("  Platform: {:?}", artifact.platform);
        println!("  Architecture: {:?}", artifact.architecture);
        println!("  Kind: {:?}", artifact.kind);
        println!("  File: {}", artifact.file_name);
        println!("  URL: {}", artifact.url);
    }
}

fn print_cached_artifacts(cached: &[CachedArtifact]) {
    for artifact in cached {
        println!("{}", artifact.descriptor.package_id);
        println!("  Version: {}", artifact.descriptor.version);
        println!("  File: {}", artifact.path.display());
        println!("  Size: {}", artifact.size);
        println!("  SHA-256: {}", artifact.sha256);
        println!(
            "  Reused existing file: {}",
            yes_no(artifact.reused_existing_file)
        );
    }
}

fn print_install_report(report: &InstallReport) {
    println!("Resource path: {}", report.resource_path.display());
    println!("Dry run: {}", yes_no(report.dry_run));
    print_preflight_report(&report.preflight);
    println!("Receipt written: {}", yes_no(report.receipt_written));
    if let Some(receipt_backup_path) = &report.receipt_backup_path {
        println!("Receipt backup: {}", receipt_backup_path.display());
    }
    if let Some(backup_manifest_path) = &report.backup_manifest_path {
        println!("Backup manifest: {}", backup_manifest_path.display());
    }
    for action in &report.actions {
        println!("{}", action.package_id);
        println!("  Action: {:?}", action.action);
        println!("  Source: {}", action.source_path.display());
        println!("  Target: {}", action.target_path.display());
        if let Some(backup_path) = &action.backup_path {
            println!("  Backup: {}", backup_path.display());
        }
        println!("  Size: {}", action.size);
        println!("  SHA-256: {}", action.sha256);
    }
}

fn print_resource_init_report(report: &ResourceInitReport) {
    println!("Resource path: {}", report.resource_path.display());
    println!("Dry run: {}", yes_no(report.dry_run));
    println!("Portable layout: {}", yes_no(report.portable));
    print_preflight_report(&report.preflight);
    for action in &report.actions {
        let verb = match action.action {
            ResourceInitActionKind::WouldCreate => "Would create",
            ResourceInitActionKind::Created => "Created",
            ResourceInitActionKind::AlreadyExists => "Already exists",
        };
        println!("  {verb} {:?}: {}", action.kind, action.path.display());
    }
}

fn print_backup_sets(backup_sets: &[BackupSet]) {
    if backup_sets.is_empty() {
        println!("No backup sets found.");
        return;
    }

    for backup_set in backup_sets {
        println!("{}", backup_set.id);
        println!("  Path: {}", backup_set.path.display());
        if let Some(created_at) = &backup_set.created_at {
            println!("  Created: {created_at}");
        }
        if let Some(reason) = &backup_set.reason {
            println!("  Reason: {reason}");
        }
        if let Some(manifest_path) = &backup_set.manifest_path {
            println!("  Manifest: {}", manifest_path.display());
        }
        if let Some(warning) = &backup_set.warning {
            println!("  Warning: {warning}");
        }
        println!("  Files: {}", backup_set.files.len());
        for file in &backup_set.files {
            println!("    {}", file.display());
        }
    }
}

fn print_restore_backup_report(report: &RestoreBackupReport) {
    println!("Resource path: {}", report.resource_path.display());
    println!("Backup id: {}", report.backup_id);
    println!("Backup path: {}", report.backup_path.display());
    println!("Dry run: {}", yes_no(report.dry_run));
    print_preflight_report(&report.preflight);
    for action in &report.actions {
        let verb = match action.action {
            RestoreBackupActionKind::WouldRestore => "Would restore",
            RestoreBackupActionKind::Restored => "Restored",
        };
        println!("  {verb}: {}", action.target_path.display());
        println!("    Source: {}", action.source_path.display());
        if let Some(current_backup_path) = &action.current_backup_path {
            println!("    Current file backup: {}", current_backup_path.display());
        }
        println!("    Size: {}", action.size);
        println!("    SHA-256: {}", action.sha256);
    }
    for target in &report.skipped_targets {
        println!(
            "  Skipped: {} (its backup never finished, so the file was never changed)",
            target.display()
        );
    }
    if let Some(package_id) = &report.package_id {
        println!("Package: {package_id}");
    }
    for package in &report.packages {
        print_restored_package(package, report.dry_run);
    }
}

/// What a restore did for a package from an uninstall set, besides putting
/// its files back.
fn print_restored_package(package: &RestoredPackage, dry_run: bool) {
    let (back, off) = if dry_run {
        ("would go back", "would come off")
    } else {
        ("went back", "came off")
    };
    println!("{}", package.package_id);
    if package.receipt_merged {
        println!(
            "  Its entry in RABBIT's receipt {back}, and it {off} the list of packages you \
             uninstalled or turned down."
        );
    } else {
        println!(
            "  It {off} the list of packages you uninstalled or turned down. RABBIT had no \
             receipt for this copy, so it has none now either."
        );
    }
    let Some(value) = &package.langpack_before else {
        return;
    };
    match (&package.langpack_error, package.langpack_restored) {
        (Some(error), _) => println!(
            "  REAPER's language could not be set back to {value} ({error}). Choose it in \
             REAPER's preferences."
        ),
        (None, true) if dry_run => println!("  REAPER's language would be set back to {value}."),
        (None, true) => println!("  REAPER's language is set back to {value}."),
        (None, false) => println!(
            "  REAPER's language stays as it is: one was chosen since the uninstall, so RABBIT \
             doesn't set it back to {value}."
        ),
    }
}

/// Parse repeated `--package-variant <package>=<variant>` flags into the map
/// the operation options carry. Rejects a malformed pair loudly rather than
/// silently ignoring it, so a typo can't quietly install the default
/// flavour when the user asked for the other one.
fn parse_package_variants(
    pairs: &[String],
) -> Result<std::collections::BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut variants = std::collections::BTreeMap::new();
    for pair in pairs {
        let Some((package, variant)) = pair.split_once('=') else {
            return Err(
                format!("--package-variant expects <package>=<variant>, got {pair:?}").into(),
            );
        };
        let (package, variant) = (package.trim(), variant.trim());
        if package.is_empty() || variant.is_empty() {
            return Err(
                format!("--package-variant expects <package>=<variant>, got {pair:?}").into(),
            );
        }
        let known: Vec<String> = builtin_package_specs(
            Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?,
        )
        .into_iter()
        .find(|spec| spec.id == package)
        .map(|spec| spec.variants.iter().map(|v| v.id.clone()).collect())
        .unwrap_or_default();
        if !known.iter().any(|id| id == variant) {
            return Err(format!(
                "package {package:?} has no variant {variant:?} (available: {})",
                if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                }
            )
            .into());
        }
        variants.insert(package.to_string(), variant.to_string());
    }
    Ok(variants)
}

/// Parse repeated `--package-channel <package>=<channel>` flags. Rejected
/// loudly - an unknown package, a channel the package doesn't offer, or a
/// parameter of the wrong shape (`osara=pr:abc`) - so a typo can't quietly
/// install the regular release instead. `<package>=stable` is accepted and
/// kept, so it can override a channel remembered from an earlier install.
fn parse_package_channels(pairs: &[String]) -> Result<PackageChannels, Box<dyn std::error::Error>> {
    let manifest = embedded_package_manifest();
    let mut channels = PackageChannels::new();
    for pair in pairs {
        let Some((package, channel)) = pair.split_once('=') else {
            return Err(
                format!("--package-channel expects <package>=<channel>, got {pair:?}").into(),
            );
        };
        let (package, channel) = (package.trim(), channel.trim());
        if package.is_empty() || channel.is_empty() {
            return Err(
                format!("--package-channel expects <package>=<channel>, got {pair:?}").into(),
            );
        }
        let Some(spec) = manifest.packages.iter().find(|spec| spec.id == package) else {
            return Err(format!("--package-channel names unknown package {package:?}").into());
        };
        if channel != STABLE_CHANNEL && !spec.offers_channel(channel) {
            let offered: Vec<String> = std::iter::once(STABLE_CHANNEL.to_string())
                .chain(
                    spec.channels
                        .iter()
                        .map(|candidate| match candidate.parameter {
                            Some(_) => format!("{}:<number>", candidate.id),
                            None => candidate.id.clone(),
                        }),
                )
                .collect();
            return Err(format!(
                "package {package:?} has no channel {channel:?} (available: {})",
                offered.join(", ")
            )
            .into());
        }
        channels.insert(package.to_string(), channel.to_string());
    }
    Ok(channels)
}

/// `stable` followed by every channel a package offers, as a user would
/// type it: `dev`, or `pr:<number>` for one that takes a parameter.
fn channel_names(channels: &[rabbit_core::package::PackageChannel]) -> String {
    std::iter::once(STABLE_CHANNEL.to_string())
        .chain(channels.iter().map(|channel| match channel.parameter {
            Some(_) => format!("{}:<number>", channel.id),
            None => channel.id.clone(),
        }))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The channel each of `package_ids` should use: the one its last install at
/// `resource_path` recorded, overridden by any `--package-channel` flag. The
/// CLI counts as expert mode, so unlike the wizard's normal mode it keeps a
/// package on its channel between runs.
fn effective_package_channels(
    resource_path: Option<&Path>,
    package_ids: &[String],
    flags: &[String],
) -> Result<PackageChannels, Box<dyn std::error::Error>> {
    let mut channels = resource_path
        .map(|path| remembered_channels(path, package_ids))
        .unwrap_or_default();
    channels.extend(parse_package_channels(flags)?);
    Ok(channels)
}

/// One line for each package that was asked for a channel but resolved to
/// the regular release. Only a pull request whose test build is gone does
/// that, and falling back is right, since an expired build should not fail
/// the run. But someone who asked for `osara=pr:1454` has to hear that they
/// are getting the snapshot instead.
fn channel_fallbacks<'a>(
    requested: &PackageChannels,
    resolved: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> Vec<String> {
    resolved
        .into_iter()
        .filter_map(|(package_id, channel)| {
            let wanted = requested.get(package_id)?;
            (wanted != STABLE_CHANNEL && channel != Some(wanted.as_str())).then(|| {
                format!(
                    "{package_id}: nothing to install from {wanted}, so the regular release is \
                     used instead (the pull request is closed, or GitHub deleted its build)"
                )
            })
        })
        .collect()
}

fn artifact_channels(
    artifacts: &[rabbit_core::artifact::ArtifactDescriptor],
) -> impl Iterator<Item = (&str, Option<&str>)> {
    artifacts
        .iter()
        .map(|artifact| (artifact.package_id.as_str(), artifact.channel.as_deref()))
}

fn item_channels(
    items: &[rabbit_core::operation::PackageOperationItem],
) -> impl Iterator<Item = (&str, Option<&str>)> {
    items
        .iter()
        .map(|item| (item.package_id.as_str(), item.artifact.channel.as_deref()))
}

/// The OSARA key map choice for a run: what the user asked for, or else
/// what fits the target's key map, as the wizard picks it.
fn osara_keymap_choice(
    resource_path: &Path,
    platform: Platform,
    replace: bool,
    preserve: bool,
) -> OsaraKeymapRunChoice {
    if replace || preserve {
        return OsaraKeymapRunChoice {
            replace,
            automatic: None,
        };
    }
    let default = osara_keymap_default(resource_path, platform);
    OsaraKeymapRunChoice {
        replace: default.replace,
        automatic: Some(default),
    }
}

struct OsaraKeymapRunChoice {
    replace: bool,
    /// Why, when RABBIT chose rather than the user.
    automatic: Option<OsaraKeymapDefault>,
}

/// Say what RABBIT chose for the key map and why, when it chose and the run
/// installs OSARA. On stderr, like the other notes, so `--json` stays JSON.
fn explain_osara_keymap_choice(choice: &OsaraKeymapRunChoice, items: &[PackageOperationItem]) {
    let Some(default) = &choice.automatic else {
        return;
    };
    let installs_osara = items.iter().any(|item| {
        item.package_id == PACKAGE_OSARA
            && matches!(
                item.plan_action,
                PlanActionKind::Install | PlanActionKind::Update
            )
            && matches!(
                item.status,
                PackageOperationStatus::InstalledOrChecked
                    | PackageOperationStatus::PlannedUnattended
            )
    });
    if installs_osara {
        eprintln!("note: {}", osara_keymap_note(default));
    }
}

fn osara_keymap_note(default: &OsaraKeymapDefault) -> String {
    let merge = "To add the new OSARA key bindings to it, run OSARA's \"Merge OSARA key map\" \
                 action in REAPER (Extensions menu, OSARA), or pass --replace-osara-keymap to \
                 replace it.";
    match (default.replace, default.osara_installed, default.state) {
        (true, false, _) => {
            "OSARA is new here, so the OSARA key map becomes your key map, after a backup of \
             any reaper-kb.ini. Pass --preserve-osara-keymap to keep your current keys."
                .to_string()
        }
        (true, true, _) => {
            "reaper-kb.ini is the installed OSARA key map with no changes of your own, so the \
             new OSARA key map replaces it, after a backup. Registered scripts stay. Pass \
             --preserve-osara-keymap to keep it."
                .to_string()
        }
        (false, _, OsaraKeymapState::DefaultKeys) => {
            "You use REAPER's default key bindings rather than the OSARA key map, so they stay. \
             Pass --replace-osara-keymap to switch to the OSARA key map."
                .to_string()
        }
        (false, _, OsaraKeymapState::Unknown) => format!(
            "RABBIT couldn't compare reaper-kb.ini with the installed OSARA key map, so it \
             stays as it is. {merge}"
        ),
        (false, _, _) => {
            format!("reaper-kb.ini has changes of your own, so it stays as it is. {merge}")
        }
    }
}

fn warn_channel_fallbacks<'a>(
    requested: &PackageChannels,
    resolved: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) {
    for line in channel_fallbacks(requested, resolved) {
        eprintln!("warning: {line}");
    }
}

/// Check `--reaper-language <package>` names a language pack we actually
/// ship. Without this the value is looked up in the install receipts, finds
/// nothing for a typo, and the run quietly leaves REAPER in English — the
/// user asked for German and got silence. Same standard as
/// `parse_package_variants`: reject loudly rather than install the wrong
/// thing.
fn validate_reaper_language(
    package: Option<String>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let Some(package) = package else {
        return Ok(None);
    };
    let package = package.trim().to_string();
    // The one "set REAPER's language" step lists every language pack we
    // ship, so it is the authoritative set.
    let known: Vec<String> = rabbit_core::configuration::builtin_configuration_steps()
        .into_iter()
        .find(|step| step.id == rabbit_core::configuration::CONFIG_SET_REAPER_LANGUAGE)
        .map(|step| step.requires_packages)
        .unwrap_or_default();
    if !known.contains(&package) {
        return Err(format!(
            "--reaper-language expects a language pack, got {package:?} (available: {})",
            if known.is_empty() {
                "none".to_string()
            } else {
                known.join(", ")
            }
        )
        .into());
    }
    Ok(Some(package))
}

fn print_package_operation_report(report: &PackageOperationReport) {
    println!("Resource path: {}", report.resource_path.display());
    println!("Dry run: {}", yes_no(report.dry_run));
    if let Some(install_report) = &report.install_report {
        print_preflight_report(&install_report.preflight);
    }
    if let Some(path) = &report.receipt_backup_path {
        println!("Receipt backup: {}", path.display());
    }
    if let Some(path) = &report.receipt_backup_manifest_path {
        println!("Backup manifest: {}", path.display());
    }

    for item in &report.items {
        println!("{}", item.package_id);
        println!("  Plan action: {:?}", item.plan_action);
        println!("  Status: {:?}", item.status);
        println!("  Kind: {:?}", item.artifact.kind);
        println!("  Version: {}", item.artifact.version);
        println!("  URL: {}", item.artifact.url);
        println!("  Message: {}", item.message);
        for path in &item.backup_paths {
            println!("  Backup file: {}", path.display());
        }
        if let Some(path) = &item.backup_manifest_path {
            println!("  Backup manifest: {}", path.display());
        }
        if let Some(plan) = &item.planned_execution {
            println!("  Planned execution: {:?}", plan.kind);
            println!("    Artifact: {}", plan.artifact_location);
            if let Some(program) = &plan.program {
                println!("    Program: {program}");
            } else if plan.kind == PlannedExecutionKind::LaunchInstallerExecutable {
                println!("    Program: the installer inside the .zip, once it is downloaded");
            }
            if !plan.arguments.is_empty() {
                println!("    Arguments: {}", plan.arguments.join(" "));
            }
            if let Some(path) = &plan.working_directory {
                println!("    Working directory: {}", path.display());
            }
            for path in &plan.verification_paths {
                println!("    Verify: {}", path.display());
            }
        }
        if let Some(instruction) = &item.manual_instruction {
            println!("  Manual step: {}", instruction.title);
            for step in &instruction.steps {
                println!("    Step: {step}");
            }
            for note in &instruction.notes {
                println!("    Note: {note}");
            }
        }
        if let Some(cached) = &item.cached_artifact {
            println!("  Cached: {}", cached.path.display());
            println!("  SHA-256: {}", cached.sha256);
        }
        if let Some(action) = &item.install_action {
            println!("  Install action: {:?}", action.action);
            println!("  Target: {}", action.target_path.display());
        }
    }
}

fn print_setup_report(report: &SetupReport) {
    println!("Setup resource path: {}", report.resource_path.display());
    println!("Dry run: {}", yes_no(report.dry_run));
    println!();
    println!("Resource initialization");
    print_resource_init_report(&report.resource_init);
    println!();
    println!("Package operation");
    print_package_operation_report(&report.package_operation);
}

/// The embedded translations in the user's language (RABBIT_LOCALE, the
/// system's, en-US as the fallback), for the text that comes from the
/// package manifests: descriptions, uninstall notes and instructions. The
/// CLI's own text stays English. Falling back to the default keeps the
/// output usable even on hosts where a Fluent file is missing; with no
/// locale at all, the keys are shown.
fn cli_localizer() -> Option<Localizer> {
    Localizer::embedded(&resolve_runtime_locale())
        .or_else(|_| Localizer::embedded(DEFAULT_LOCALE))
        .ok()
}

/// The text of `key` in the user's language, or the key itself when there
/// is none.
fn localized_text(localizer: Option<&Localizer>, key: &str) -> String {
    localizer
        .map(|localizer| localizer.text(key))
        .filter(|text| !text.missing)
        .map_or_else(|| key.to_string(), |text| text.value)
}

fn print_package_specs(packages: &[rabbit_core::package::PackageSpec]) {
    let localizer = cli_localizer();
    for package in packages {
        println!("{}", package.id);
        println!("  Display name: {}", package.display_name);
        println!("  Display name key: {}", package.display_name_key);
        if let Some(localizer) = localizer.as_ref() {
            let description = localizer.text(&package.display_description_key);
            if !description.missing {
                println!("  Description: {}", description.value);
            }
        }
        println!("  Kind: {}", serialized_name(&package.package_kind));
        println!("  Required: {}", yes_no(package.required));
        println!("  Recommended: {}", yes_no(package.recommended));
        println!("  Channels: {}", channel_names(&package.channels));
        println!(
            "  Supported platforms: {}",
            serialized_names(&package.supported_platforms)
        );
        println!(
            "  Supported architectures: {}",
            serialized_names(&package.supported_architectures)
        );
        println!(
            "  Artifact source: {}",
            artifact_source_label(
                package.github_release.is_some(),
                package.http_artifact.is_some(),
                package.hfs_listing.is_some(),
            )
        );
        println!("  Detectors: {}", serialized_names(&package.detectors));
        println!(
            "  Install steps: {}",
            serialized_names(&package.install_steps)
        );
        print_uninstall_plans(
            &package.uninstall,
            &package.supported_platforms,
            package.requires_standard_install,
            localizer.as_ref(),
        );
        println!(
            "  Backup policy: {}",
            serialized_name(&package.backup_policy)
        );
        println!(
            "  Plugin prefixes: {}",
            string_names(&package.user_plugin_prefixes)
        );
        println!(
            "  Plugin suffixes: {}",
            string_names(&package.user_plugin_suffixes)
        );
    }
}

/// One line per supported platform and REAPER target, saying which uninstall
/// plan applies there. With a localizer, a plan's note is shown as text;
/// without one (the raw `--manifest` listing), as its Fluent key.
fn print_uninstall_plans(
    uninstall: &UninstallSpec,
    platforms: &[SupportedPlatform],
    requires_standard_install: bool,
    localizer: Option<&Localizer>,
) {
    println!("  Uninstall:");
    for platform in platforms {
        for target in [InstallationKind::Standard, InstallationKind::Portable] {
            let label = format!(
                "{}, {} REAPER",
                serialized_name(platform),
                serialized_name(&target)
            );
            if target == InstallationKind::Portable && requires_standard_install {
                println!("    {label}: not offered, needs a standard REAPER");
                continue;
            }
            let line = uninstall.plan_for(platform.platform(), target).map_or_else(
                || "no plan".to_string(),
                |plan| describe_uninstall_plan(plan, localizer),
            );
            println!("    {label}: {line}");
        }
    }
}

fn describe_uninstall_plan(plan: &UninstallPlanSpec, localizer: Option<&Localizer>) -> String {
    let mut line = serialized_name(&plan.strategy);
    let mut details = Vec::new();
    if plan.elevation != UninstallElevation::Never {
        details.push(format!("elevation {}", serialized_name(&plan.elevation)));
    }
    if plan.strategy == UninstallStrategy::RemoveFiles || plan.backup != UninstallBackup::Move {
        details.push(format!("backup {}", serialized_name(&plan.backup)));
    }
    if !details.is_empty() {
        line.push_str(&format!(" ({})", details.join(", ")));
    }
    if let Some(key) = &plan.note_key {
        let note = localizer
            .map(|localizer| localizer.text(key))
            .filter(|text| !text.missing);
        // Instructions for removing a package by hand run over several
        // lines; each goes below the plan's line, indented under it, so the
        // listing still reads one package and one target at a time.
        match note {
            Some(note) => line.push_str(&format!(": {}", note.value.replace('\n', "\n      "))),
            None => line.push_str(&format!(": {key}")),
        }
    }
    line
}

fn serialized_name<T: Serialize + ?Sized>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        // Externally-tagged enum struct/newtype variants (e.g. the data-
        // carrying `PackageDetector`s) serialize as a single-key object
        // `{"variant_name": {...}}`; show just the tag, not the params, so
        // the describe output stays readable.
        Ok(serde_json::Value::Object(map)) if map.len() == 1 => map
            .into_iter()
            .next()
            .map(|(tag, _)| tag)
            .unwrap_or_else(|| "(invalid)".to_string()),
        Ok(value) => value.to_string(),
        Err(_) => "(invalid)".to_string(),
    }
}

/// One-word label for a package's data-driven artifact source, for the
/// `packages` / `describe` diagnostic output.
fn artifact_source_label(
    github_release: bool,
    http_artifact: bool,
    hfs_listing: bool,
) -> &'static str {
    if github_release {
        "github_release"
    } else if http_artifact {
        "http_artifact"
    } else if hfs_listing {
        "hfs_listing"
    } else {
        "(none)"
    }
}

fn serialized_names<T: Serialize>(values: &[T]) -> String {
    let names = values.iter().map(serialized_name).collect::<Vec<_>>();
    string_names(&names)
}

fn string_names(values: &[String]) -> String {
    if values.is_empty() {
        "(none)".to_string()
    } else {
        values.join(", ")
    }
}

/// The uninstall plan or report as text, in the layout of the README's
/// example: the preflight, then one block per package with what goes, what
/// stays, warnings and notes, then what to do next.
fn print_uninstall_report(report: &UninstallReport, localizer: Option<&Localizer>) {
    let resource = report.resource_path.display();
    if report.dry_run {
        println!("Uninstall plan for {resource} (dry run, nothing changed)");
    } else {
        println!("Uninstall from {resource}");
    }
    if report.target == InstallationKind::Portable {
        println!("This is a portable REAPER.");
    }
    if report.remove_settings {
        println!("Settings go too (--remove-settings).");
    }
    println!();
    println!("Preflight");
    for check in &report.preflight.checks {
        let status = match check.status {
            PreflightStatus::Pass => "pass",
            PreflightStatus::Warn => "warn",
            PreflightStatus::Fail => "fail",
        };
        println!("  {status:<8} {}", check.message);
    }
    for item in &report.items {
        println!();
        print_uninstall_item(report, item, localizer);
    }

    println!();
    let blocked: Vec<&str> = report
        .items
        .iter()
        .filter(|item| item.status == UninstallStatus::Blocked)
        .map(|item| item.package_id.as_str())
        .collect();
    if report.dry_run {
        if !blocked.is_empty() {
            println!(
                "Blocked: {}. Nothing would change with --apply until {} left out.",
                blocked.join(", "),
                if blocked.len() == 1 {
                    "it is"
                } else {
                    "they are"
                }
            );
        } else if report
            .items
            .iter()
            .any(|item| item.status == UninstallStatus::WouldRemove)
        {
            println!("Dry run: nothing changed. Pass --apply to uninstall.");
        } else {
            println!("Nothing to uninstall.");
        }
        return;
    }
    if let Some(set_id) = &report.backup_set_id {
        println!("The removed files are in the backup set {set_id}. To put a package back:");
        println!(
            "  rabbit restore-backup --resource-path \"{resource}\" --backup-id {set_id} \
             --package <id> --apply"
        );
    } else if report.blocked_before_changes() {
        println!(
            "Nothing changed, because RABBIT can't uninstall {}.",
            blocked.join(", ")
        );
    }
}

fn print_uninstall_item(
    report: &UninstallReport,
    item: &UninstallReportItem,
    localizer: Option<&Localizer>,
) {
    let plan = report.plan.item(&item.package_id);
    let name = match plan.and_then(|plan| plan.installed_version.as_ref()) {
        Some(version) => format!("{} {version}", item.display_name),
        None => item.display_name.clone(),
    };
    // A package to remove by hand gets the instructions, in the user's
    // language, right here rather than a pointer elsewhere.
    if let UninstallMessage::NotRemovable { removability } = &item.message_code {
        match removability {
            Removability::Manual { note_key } => {
                println!("{name}: can't uninstall automatically.");
                for line in localized_text(localizer, note_key).lines() {
                    println!("  {line}");
                }
                return;
            }
            Removability::NotSupported { note_key } => {
                println!("{name}: {}", localized_text(localizer, note_key));
                return;
            }
            _ => {}
        }
    }
    let heading = match item.status {
        UninstallStatus::WouldRemove => format!(
            "will uninstall. RABBIT moves its files into a backup set; {}.",
            if plan.is_some_and(|plan| plan.requires_elevation) {
                "it needs a permission prompt"
            } else {
                "no permission prompt"
            }
        ),
        UninstallStatus::Removed => "removed.".to_string(),
        UninstallStatus::RemovedWithLeftovers => {
            "removed, but RABBIT still finds some of it.".to_string()
        }
        UninstallStatus::RemovedPendingRestart => {
            "removed; the rest goes at the next restart.".to_string()
        }
        UninstallStatus::NotInstalled => {
            "not installed, so there is nothing to remove.".to_string()
        }
        UninstallStatus::Blocked => format!("blocked. {}", item.message),
        UninstallStatus::Failed => format!("failed. {}", item.message),
        UninstallStatus::Cancelled => format!("stopped. {}", item.message),
        UninstallStatus::NotReached => format!("not reached. {}", item.message),
    };
    println!("{name}: {heading}");
    let would = item.status == UninstallStatus::WouldRemove;
    if !would && !item.status.is_removed() {
        return;
    }

    let shown = |path: &Path| display_path(&report.resource_path, path);
    let detail = |label: &str, text: &str| println!("  {label:<8} {text}");
    for file in &item.removed {
        let label = match (file.reason == RemovalReason::Setting, would) {
            (true, true) => "remove setting",
            (true, false) => "removed setting",
            (false, true) => "remove",
            (false, false) => "removed",
        };
        let changed = if file.changed_since_install {
            " (changed since RABBIT installed it)"
        } else {
            ""
        };
        detail(label, &format!("{}{changed}", shown(&file.path)));
    }
    for dir in &item.removed_dirs {
        detail("removed", &format!("{} (an empty folder)", shown(dir)));
    }
    let keep = if would { "keep" } else { "kept" };
    for kept in &item.kept {
        let why = match kept.reason {
            KeptFileReason::ChangedSinceInstall => "changed since RABBIT installed it",
            KeptFileReason::HoldsLink => "holds a link, which RABBIT never follows",
        };
        detail(keep, &format!("{} ({why})", shown(&kept.path)));
    }
    if let Some(plan) = plan {
        for kept in plan
            .kept
            .iter()
            .filter(|kept| kept.exists && kept.reason != KeptReason::RabbitFolder)
        {
            let note = match &kept.note_key {
                Some(key) => localized_text(localizer, key),
                None => kept_reason_text(kept.reason).to_string(),
            };
            detail(keep, &format!("{} ({note})", shown(&kept.path)));
        }
        if !report.remove_settings && plan.kept.iter().any(|kept| kept.removable_with_settings) {
            detail(
                "note",
                "Pass --remove-settings to remove the settings too. They go into the same \
                 backup set, so a restore brings them back.",
            );
        }
        for warning in &plan.warnings {
            detail(
                "warning",
                &uninstall_warning_text(warning, &item.display_name, localizer),
            );
        }
        for note in &plan.notes {
            detail(
                "note",
                &uninstall_note_text(note, &item.display_name, localizer),
            );
        }
    }
    for path in &item.leftovers {
        detail("leftover", &shown(path));
    }
    for problem in &item.problems {
        detail("problem", &problem.message());
    }
}

/// `path` relative to the resource folder when it is inside it, the way the
/// user knows it from REAPER; anything else in full.
fn display_path(resource: &Path, path: &Path) -> String {
    path.strip_prefix(resource)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map_or_else(
            || path.display().to_string(),
            |relative| relative.display().to_string(),
        )
}

/// Why something stays, when its manifest gives no note of its own.
fn kept_reason_text(reason: KeptReason) -> &'static str {
    match reason {
        KeptReason::UserData => "your data",
        KeptReason::FolderRoot => "a folder other things share",
        KeptReason::ReaperConfiguration => "REAPER's own settings",
        KeptReason::KeyMap => "a key map",
        KeptReason::KeepList => "always kept",
        KeptReason::RabbitFolder => "RABBIT's own folder",
        KeptReason::OutsideResourceFolder => {
            "RABBIT's receipt lists it, but it is outside the REAPER folder and the package \
             doesn't name it"
        }
    }
}

fn uninstall_warning_text(
    warning: &UninstallWarning,
    name: &str,
    localizer: Option<&Localizer>,
) -> String {
    match warning {
        UninstallWarning::Always { message_key, .. }
        | UninstallWarning::OtherPackageInstalled { message_key, .. } => {
            localized_text(localizer, message_key)
        }
        UninstallWarning::DependentsStay { package_ids } => format!(
            "These packages install on top of {name} and stay: {}. They may not work without it.",
            package_ids.join(", ")
        ),
        UninstallWarning::MachineWide => format!(
            "{name} serves every REAPER and every other program on this computer, so they all \
             lose it."
        ),
        UninstallWarning::ReaPackStillManages { reapack_packages } => format!(
            "ReaPack installed these, and they stay without updates once it is gone: {}.",
            reapack_packages.join(", ")
        ),
        UninstallWarning::ReaPackRecordsUnreadable { reason } => format!(
            "RABBIT couldn't read ReaPack's records to say what it still manages ({reason})."
        ),
    }
}

fn uninstall_note_text(note: &UninstallNote, name: &str, localizer: Option<&Localizer>) -> String {
    match note {
        UninstallNote::NoReceipt => {
            format!(
                "RABBIT didn't install this copy. It removes the files {name} is known to install."
            )
        }
        UninstallNote::OrphanedFromReaPack { reapack_packages } => {
            let installed_as = if reapack_packages.is_empty() {
                String::new()
            } else {
                format!(" as {}", reapack_packages.join(", "))
            };
            format!(
                "ReaPack once installed this copy{installed_as}. ReaPack is gone, so RABBIT \
                 removes it; ReaPack's records stay."
            )
        }
        UninstallNote::Manifest { note_key } => localized_text(localizer, note_key),
    }
}

/// How the CLI names a platform in its own text.
fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "Windows",
        Platform::MacOs => "macOS",
    }
}

fn print_preflight_report(report: &PreflightReport) {
    println!("Preflight passed: {}", yes_no(report.passed));
    for check in &report.checks {
        println!("  {}: {:?}: {}", check.name, check.status, check.message);
    }
}

fn print_plan(plan: &rabbit_core::plan::InstallPlan) {
    if let Some(target) = &plan.target {
        println!("Target resource path: {}", target.resource_path.display());
    } else {
        println!("Target resource path: not selected");
    }

    for action in &plan.actions {
        println!("{}", action.package_id);
        println!("  Action: {:?}", action.action);
        println!(
            "  Installed version: {}",
            action
                .installed_version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown".to_string())
        );
        println!(
            "  Available version: {}",
            action
                .available_version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown".to_string())
        );
        println!("  Reason: {}", action.reason);
    }

    for note in &plan.notes {
        println!("Note: {note}");
    }
}

fn print_locales(locales: &[String]) {
    for locale in locales {
        println!("{locale}");
    }
}

fn print_localized_text(message: &LocalizedText) {
    println!("{}", message.value);
    println!("  Id: {}", message.id);
    println!("  Locale: {}", message.locale);
    println!("  Fallback: {}", yes_no(message.fallback_used));
    println!("  Missing: {}", yes_no(message.missing));
    for error in &message.formatting_errors {
        println!("  Formatting error: {error}");
    }
}

fn print_portability_report(report: &PortabilityReport) {
    println!("Portable runtime passed: {}", yes_no(report.passed));
    println!(
        "Executable: {}",
        report
            .current_exe
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("Current directory: {}", report.current_dir.display());
    println!(
        "Locales directory: {} ({})",
        report.locales_dir.display(),
        if report.locales_dir_present {
            "present"
        } else {
            "absent"
        }
    );
    println!("Embedded resources: {}", report.embedded_resources.len());
    for resource in &report.embedded_resources {
        println!(
            "  {} {} ({} bytes)",
            resource.kind, resource.id, resource.bytes
        );
    }
    println!(
        "Required external resources: {}",
        report.required_external_resources.len()
    );
    for check in &report.checks {
        println!(
            "  {}: {}: {}",
            check.name,
            portability_status_label(check.status),
            check.message
        );
    }
}

fn print_self_update_report(report: &SelfUpdateCheckReport) {
    println!("Manifest URL: {}", report.manifest_url);
    println!("Channel: {}", report.channel);
    println!("Current version: {}", report.current_version);
    println!("Latest version: {}", report.latest_version);
    println!("Published at: {}", report.published_at);
    println!("Update available: {}", yes_no(report.update_available));
    println!(
        "Requires manual transition: {}",
        yes_no(report.requires_manual_transition)
    );
    if let Some(minimum) = report.minimum_supported_previous_version.as_ref() {
        println!("Minimum supported previous version: {minimum}");
    }
    if let Some(url) = report.release_notes_url.as_ref() {
        println!("Release notes: {url}");
    }
    println!("Asset platform: {:?}", report.asset.platform);
    println!("Asset URL: {}", report.asset.url);
    println!("Asset SHA-256: {}", report.asset.sha256);
}

/// Print what the pending update actually changes, covering every release
/// between the running version and the latest. Silent when there is no
/// update, or when the notes can't be fetched — the check itself has already
/// reported everything that matters.
fn print_self_update_release_notes(report: &SelfUpdateCheckReport) {
    let Some(notes) = resolve_self_update_release_notes(report) else {
        return;
    };
    println!();
    println!("What's new since {}:", report.current_version);
    println!("{notes}");
}

fn print_self_update_stage_report(report: &SelfUpdateStageReport) {
    print_self_update_report(&report.check);
    println!("Staging directory: {}", report.staging_dir.display());
    println!(
        "Staged asset: {}",
        report
            .staged_asset_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not staged".to_string())
    );
    println!("Downloaded: {}", yes_no(report.downloaded));
    println!(
        "Reused existing staged file: {}",
        yes_no(report.reused_existing_file)
    );
    println!("Ready to apply: {}", yes_no(report.ready_to_apply));
    if let Some(sha256) = report.verified_sha256.as_ref() {
        println!("Verified SHA-256: {sha256}");
    }
    println!("Status: {}", report.status_message);
}

fn print_self_update_apply_report(report: &SelfUpdateApplyReport) {
    print_self_update_stage_report(&report.stage);
    println!("Install root: {}", report.install_root.display());
    println!("Replaced files: {}", report.replaced_files.len());
    for replaced in &report.replaced_files {
        println!(
            "  {} (rollback: {})",
            replaced.install_path.display(),
            replaced.backup_path.display()
        );
    }
    if !report.skipped_files.is_empty() {
        println!("Skipped files (no matching install target):");
        for path in &report.skipped_files {
            println!("  {}", path.display());
        }
    }
    println!("Status: {}", report.status_message);
}

fn portability_status_label(status: PortabilityCheckStatus) -> &'static str {
    match status {
        PortabilityCheckStatus::Passed => "passed",
        PortabilityCheckStatus::Warning => "warning",
        PortabilityCheckStatus::Failed => "failed",
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::{Cli, Command, DEFAULT_SELF_UPDATE_MANIFEST_URL, SelfUpdateCommand};

    /// A command runs on the main thread, which gets 1 MB of stack on
    /// Windows, and debug builds (tests included) use far more stack than
    /// release builds. Dispatching from one big match once took more than
    /// all of it; keep a command that loads every package manifest well
    /// inside half of it.
    #[test]
    fn a_command_runs_within_half_the_windows_main_thread_stack() {
        let cli = Cli::try_parse_from(["rabbit", "packages", "--json"]).unwrap();
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(move || super::run_with(cli).map_err(|error| error.to_string()))
            .unwrap()
            .join()
            .expect("the command must not overflow its stack")
            .unwrap();
    }

    /// In a debug build, clap's parser alone takes more than the 1 MB of
    /// stack Windows gives the main thread, so the CLI runs on a thread of
    /// its own. Parse and run a command the whole way through it.
    #[test]
    fn the_cli_thread_has_room_to_parse_and_run_a_command() {
        let args = ["rabbit", "packages", "--json"].map(std::ffi::OsString::from);
        super::run_args(args.to_vec()).unwrap();
    }

    #[test]
    fn a_channel_that_resolved_to_the_regular_release_is_reported() {
        let requested: rabbit_core::package::PackageChannels = [
            ("osara".to_string(), "pr:1".to_string()),
            ("reaper".to_string(), "dev".to_string()),
            ("sws".to_string(), "stable".to_string()),
        ]
        .into_iter()
        .collect();
        let lines = super::channel_fallbacks(
            &requested,
            [
                ("osara", None),
                ("reaper", Some("dev")),
                ("sws", None),
                ("reapack", None),
            ],
        );
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].starts_with("osara: nothing to install from pr:1"),
            "{lines:?}"
        );
    }

    #[test]
    fn setup_command_parses_target_app_path() {
        let cli = Cli::try_parse_from([
            "rabbit",
            "setup",
            "--resource-path",
            "C:\\PortableREAPER",
            "--target-app-path",
            "C:\\PortableREAPER\\reaper.exe",
            "--portable",
        ])
        .unwrap();

        match cli.command {
            Command::Setup {
                resource_path,
                target_app_path,
                portable,
                ..
            } => {
                assert_eq!(resource_path, PathBuf::from("C:\\PortableREAPER"));
                assert_eq!(
                    target_app_path,
                    Some(PathBuf::from("C:\\PortableREAPER\\reaper.exe"))
                );
                assert!(portable);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn setup_command_parses_accept_reapack_donation_notice_flag() {
        let cli = Cli::try_parse_from([
            "rabbit",
            "setup",
            "--resource-path",
            "C:\\PortableREAPER",
            "--portable",
            "--accept-reapack-donation-notice",
        ])
        .unwrap();

        match cli.command {
            Command::Setup {
                accept_reapack_donation_notice,
                ..
            } => assert!(accept_reapack_donation_notice),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn ensure_reapack_donation_acknowledged_returns_err_when_unaccepted() {
        let result = super::ensure_reapack_donation_acknowledged(
            &["reapack".to_string(), "osara".to_string()],
            false,
        );
        assert!(result.is_err(), "expected refusal");
        let message = result.err().unwrap().to_string();
        assert!(
            message.contains("--accept-reapack-donation-notice"),
            "error should point at the flag, got {message:?}"
        );
    }

    #[test]
    fn ensure_reapack_donation_acknowledged_passes_when_accepted() {
        let result = super::ensure_reapack_donation_acknowledged(&["reapack".to_string()], true);
        assert!(result.is_ok());
    }

    #[test]
    fn ensure_reapack_donation_acknowledged_passes_when_reapack_not_in_plan() {
        let result = super::ensure_reapack_donation_acknowledged(
            &["osara".to_string(), "sws".to_string()],
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn setup_command_parses_preserve_osara_keymap_flag() {
        let cli = Cli::try_parse_from([
            "rabbit",
            "setup",
            "--resource-path",
            "C:\\PortableREAPER",
            "--portable",
            "--preserve-osara-keymap",
        ])
        .unwrap();

        match cli.command {
            Command::Setup {
                preserve_osara_keymap,
                ..
            } => {
                assert!(preserve_osara_keymap);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn the_key_map_flags_force_either_way_but_not_both() {
        for command in ["setup", "apply-packages"] {
            let cli = Cli::try_parse_from([
                "rabbit",
                command,
                "--resource-path",
                "C:\\REAPER",
                "--replace-osara-keymap",
            ])
            .unwrap();
            match cli.command {
                Command::Setup {
                    replace_osara_keymap,
                    preserve_osara_keymap,
                    ..
                }
                | Command::ApplyPackages {
                    replace_osara_keymap,
                    preserve_osara_keymap,
                    ..
                } => assert!(replace_osara_keymap && !preserve_osara_keymap),
                other => panic!("unexpected command: {other:?}"),
            }
            assert!(
                Cli::try_parse_from([
                    "rabbit",
                    command,
                    "--resource-path",
                    "C:\\REAPER",
                    "--replace-osara-keymap",
                    "--preserve-osara-keymap",
                ])
                .is_err()
            );
        }
    }

    #[test]
    fn without_a_flag_the_key_map_choice_fits_the_target() {
        use rabbit_core::osara_keymap::OsaraKeymapState;
        let dir = tempfile::tempdir().unwrap();
        let platform = rabbit_core::model::Platform::Windows;

        let forced = super::osara_keymap_choice(dir.path(), platform, false, true);
        assert!(!forced.replace && forced.automatic.is_none());
        let forced = super::osara_keymap_choice(dir.path(), platform, true, false);
        assert!(forced.replace && forced.automatic.is_none());

        // No OSARA here yet: its key map comes along, and RABBIT says why.
        let chosen = super::osara_keymap_choice(dir.path(), platform, false, false);
        assert!(chosen.replace);
        let default = chosen.automatic.unwrap();
        assert!(!default.osara_installed);
        assert_eq!(default.state, OsaraKeymapState::DefaultKeys);
        assert!(super::osara_keymap_note(&default).contains("--preserve-osara-keymap"));
    }

    #[test]
    fn the_key_map_note_says_what_to_do_about_a_kept_key_map() {
        use rabbit_core::osara_keymap::{OsaraKeymapDefault, OsaraKeymapState};
        let note = |state| {
            super::osara_keymap_note(&OsaraKeymapDefault {
                osara_installed: true,
                state,
                replace: rabbit_core::osara_keymap::replace_by_default(true, state),
            })
        };
        assert!(note(OsaraKeymapState::UnchangedOsara).contains("no changes of your own"));
        assert!(note(OsaraKeymapState::OwnChanges).contains("Merge OSARA key map"));
        assert!(note(OsaraKeymapState::Unknown).contains("Merge OSARA key map"));
        let defaults = note(OsaraKeymapState::DefaultKeys);
        assert!(defaults.contains("default key bindings") && !defaults.contains("Merge"));
    }

    #[test]
    fn preflight_command_parses_target_app_path() {
        let cli = Cli::try_parse_from([
            "rabbit",
            "preflight",
            "--resource-path",
            "C:\\Users\\Test\\AppData\\Roaming\\REAPER",
            "--target-app-path",
            "C:\\Program Files\\REAPER\\reaper.exe",
        ])
        .unwrap();

        match cli.command {
            Command::Preflight {
                target_app_path, ..
            } => {
                assert_eq!(
                    target_app_path,
                    Some(PathBuf::from("C:\\Program Files\\REAPER\\reaper.exe"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn self_update_check_command_parses_manifest_url() {
        let cli = Cli::try_parse_from([
            "rabbit",
            "self-update",
            "check",
            "--manifest-url",
            "https://example.test/rabbit-update-stable.json",
        ])
        .unwrap();

        match cli.command {
            Command::SelfUpdate { command } => match command {
                SelfUpdateCommand::Check { manifest_url, json } => {
                    assert_eq!(
                        manifest_url,
                        "https://example.test/rabbit-update-stable.json"
                    );
                    assert!(!json);
                }
                other => panic!("unexpected self-update command: {other:?}"),
            },
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn self_update_stage_command_parses_staging_dir() {
        let cli = Cli::try_parse_from([
            "rabbit",
            "self-update",
            "stage",
            "--staging-dir",
            "C:\\Temp\\RABBIT-Update",
        ])
        .unwrap();

        match cli.command {
            Command::SelfUpdate { command } => match command {
                SelfUpdateCommand::Stage {
                    staging_dir,
                    manifest_url,
                    json,
                } => {
                    assert_eq!(staging_dir, Some(PathBuf::from("C:\\Temp\\RABBIT-Update")));
                    assert_eq!(manifest_url, DEFAULT_SELF_UPDATE_MANIFEST_URL);
                    assert!(!json);
                }
                other => panic!("unexpected self-update command: {other:?}"),
            },
            other => panic!("unexpected command: {other:?}"),
        }
    }
    /// A language pack that is merely already installed must not drag the
    /// "set REAPER's language" step into the DEFAULT step set. Otherwise
    /// `rabbit setup` on a machine that happens to have a pack on disk
    /// would switch REAPER's interface language without being asked.
    #[test]
    fn language_step_is_not_a_default_for_a_merely_installed_pack() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("RABBIT")).unwrap();
        std::fs::create_dir_all(root.join("LangPack")).unwrap();
        std::fs::write(root.join("LangPack").join("de_DE.ReaperLangPack"), b"x").unwrap();
        std::fs::write(
            root.join("RABBIT").join("install-state.json"),
            r#"{"schema_version":1,"packages":{"langpack-de":{"id":"langpack-de","version":null,"source_url":null,"source_sha256":null,"installed_files":[{"path":"LangPack/de_DE.ReaperLangPack","sha256":null,"size":null}],"installed_at":null,"rabbit_version":null,"architecture":null}}}"#,
        )
        .unwrap();

        let language_step = rabbit_core::configuration::CONFIG_SET_REAPER_LANGUAGE;

        let defaults = super::resolve_configuration_step_ids(
            root,
            rabbit_core::model::Platform::Windows,
            &["osara".to_string()],
            &[],
            &[],
        );
        assert!(
            !defaults.iter().any(|id| id == language_step),
            "installing something unrelated must not activate a language pack: {defaults:?}"
        );

        // Asking for the pack in this run opts in.
        let requested = super::resolve_configuration_step_ids(
            root,
            rabbit_core::model::Platform::Windows,
            &["langpack-de".to_string()],
            &[],
            &[],
        );
        assert!(
            requested.iter().any(|id| id == language_step),
            "requesting a language pack must select the step that activates it: {requested:?}"
        );

        // And it stays reachable explicitly even when nothing is requested.
        let explicit = super::resolve_configuration_step_ids(
            root,
            rabbit_core::model::Platform::Windows,
            &[],
            &[language_step.to_string()],
            &[],
        );
        assert!(explicit.iter().any(|id| id == language_step));
    }
    /// A package the user uninstalled or turned down stays out of a later
    /// bare `rabbit setup`, whichever package it is, and the run says so.
    /// An explicit `--package` is the user telling us exactly what they want
    /// and always wins.
    #[test]
    fn the_default_package_set_honours_every_recorded_refusal() {
        use rabbit_core::package::PACKAGE_SWS;
        let dir = tempfile::tempdir().unwrap();

        let baseline = super::select_packages(Vec::new(), Some(dir.path()));
        assert!(!baseline.named && baseline.skipped.is_empty());
        assert!(
            baseline.package_ids.iter().any(|id| id == PACKAGE_SWS),
            "SWS is in the default set, and doesn't remember an opt-out of its own"
        );

        // A removed SWS lands in the same set as a refused language pack.
        // langpack-de isn't in the default set to begin with, so declining it
        // skips nothing.
        rabbit_core::receipt::record_package_opt_outs(
            dir.path(),
            &[PACKAGE_SWS.to_string(), "langpack-de".to_string()],
            &[],
        )
        .unwrap();
        let after = super::select_packages(Vec::new(), Some(dir.path()));
        assert_eq!(after.skipped, vec![PACKAGE_SWS.to_string()]);
        let mut expected = baseline.package_ids.clone();
        expected.retain(|id| id != PACKAGE_SWS);
        assert_eq!(after.package_ids, expected);

        // An explicit request wins over a recorded refusal.
        let explicit = super::select_packages(
            vec![PACKAGE_SWS.to_string(), "langpack-de".to_string()],
            Some(dir.path()),
        );
        assert!(explicit.named && explicit.skipped.is_empty());
        assert_eq!(
            explicit.package_ids,
            vec![PACKAGE_SWS.to_string(), "langpack-de".to_string()]
        );
        // Without a resource folder there is nothing to remember.
        let nowhere = super::select_packages(Vec::new(), None);
        assert_eq!(nowhere.package_ids, baseline.package_ids);
    }

    /// Naming a package is the user's verdict: a run that installs takes
    /// it off the refusals, a dry run only says it would, and packages not
    /// named keep theirs.
    #[test]
    fn naming_a_package_clears_its_refusal_once_a_run_installs() {
        use rabbit_core::package::PACKAGE_SWS;
        let dir = tempfile::tempdir().unwrap();
        rabbit_core::receipt::record_package_opt_outs(
            dir.path(),
            &[PACKAGE_SWS.to_string(), "langpack-de".to_string()],
            &[],
        )
        .unwrap();
        let named = vec![PACKAGE_SWS.to_string(), "osara".to_string()];

        super::forget_refusals_of_named(dir.path(), &named, false);
        let declined = rabbit_core::receipt::declined_packages(dir.path());
        assert!(declined.contains(PACKAGE_SWS), "a dry run changes nothing");

        super::forget_refusals_of_named(dir.path(), &named, true);
        let declined = rabbit_core::receipt::declined_packages(dir.path());
        assert!(!declined.contains(PACKAGE_SWS));
        assert!(declined.contains("langpack-de"), "{declined:?}");
    }
    /// Every flag has to explain itself. The README's advice for anything
    /// not covered by its examples is "run `RABBIT --help`", which is only
    /// honest if `--help` actually says something — and 47 of 50 flags used
    /// to print nothing but their own name.
    #[test]
    fn every_cli_flag_documents_itself() {
        use clap::CommandFactory;

        fn walk(command: &clap::Command, path: &str, missing: &mut Vec<String>) {
            for arg in command.get_arguments() {
                let id = arg.get_id().as_str();
                if matches!(id, "help" | "version") {
                    continue;
                }
                if arg.get_help().is_none() && arg.get_long_help().is_none() {
                    missing.push(format!("{path} --{id}"));
                }
            }
            for sub in command.get_subcommands() {
                walk(sub, &format!("{path} {}", sub.get_name()), missing);
            }
        }

        let mut missing = Vec::new();
        walk(&super::Cli::command(), "rabbit", &mut missing);
        assert!(missing.is_empty(), "flags with no help text: {missing:#?}");
    }
    #[test]
    fn package_channel_flags_are_validated_against_the_manifest() {
        let parsed = super::parse_package_channels(&["reaper=dev".to_string()]).unwrap();
        assert_eq!(parsed.get("reaper").map(String::as_str), Some("dev"));
        // `stable` is always accepted, so it can undo a remembered channel.
        let stable = super::parse_package_channels(&["reaper=stable".to_string()]).unwrap();
        assert_eq!(stable.get("reaper").map(String::as_str), Some("stable"));

        for bad in [
            "reaper=nightly",
            "reaper=dev:1",
            "nosuch=dev",
            "reaper",
            "=dev",
            "reaper=",
        ] {
            assert!(
                super::parse_package_channels(&[bad.to_string()]).is_err(),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn an_explicit_channel_flag_overrides_the_remembered_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut packages = std::collections::BTreeMap::new();
        packages.insert(
            "reaper".to_string(),
            rabbit_core::receipt::PackageReceipt {
                id: "reaper".to_string(),
                version: None,
                variant: None,
                channel: Some("dev".to_string()),
                source_url: None,
                source_sha256: None,
                installed_files: Vec::new(),
                installed_at: None,
                rabbit_version: None,
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );
        rabbit_core::receipt::save_install_state(
            dir.path(),
            &rabbit_core::receipt::InstallState {
                schema_version: 1,
                packages,
                declined_packages: Default::default(),
                applied_configuration: Default::default(),
            },
        )
        .unwrap();
        let ids = vec!["reaper".to_string()];

        // The CLI counts as expert mode: with no flag, it stays on dev.
        let remembered = super::effective_package_channels(Some(dir.path()), &ids, &[]).unwrap();
        assert_eq!(remembered.get("reaper").map(String::as_str), Some("dev"));

        // `reaper=stable` is how you go back.
        let back = super::effective_package_channels(
            Some(dir.path()),
            &ids,
            &["reaper=stable".to_string()],
        )
        .unwrap();
        assert_eq!(back.get("reaper").map(String::as_str), Some("stable"));
    }

    #[test]
    fn uninstall_plans_print_their_strategy_and_note() {
        use rabbit_core::package::uninstall::UninstallPlanSpec;

        let plan = |json: &str| serde_json::from_str::<UninstallPlanSpec>(json).unwrap();
        let not_yet = plan(r#"{ "strategy": "not_supported", "note_key": "uninstall-not-yet" }"#);
        let english = rabbit_core::localization::Localizer::embedded("en-US").unwrap();
        assert_eq!(
            super::describe_uninstall_plan(&not_yet, Some(&english)),
            "not_supported: RABBIT can't uninstall this yet."
        );
        // The raw manifest listing has no localizer and shows the key.
        assert_eq!(
            super::describe_uninstall_plan(&not_yet, None),
            "not_supported: uninstall-not-yet"
        );
        let elevated = plan(
            r#"{ "strategy": "remove_files", "elevation": "required", "backup": "none",
                 "remove": { "receipt_files": true } }"#,
        );
        assert_eq!(
            super::describe_uninstall_plan(&elevated, None),
            "remove_files (elevation required, backup none)"
        );
        // Manual instructions keep their lines, indented under the plan's.
        let manual = plan(
            r#"{ "platform": "windows", "strategy": "manual",
                 "note_key": "uninstall-manual-jaws-scripts" }"#,
        );
        let described = super::describe_uninstall_plan(&manual, Some(&english));
        let lines: Vec<&str> = described.lines().collect();
        assert!(lines[0].starts_with("manual: RABBIT can't remove the JAWS scripts yet."));
        assert_eq!(lines[1], "      1. Close REAPER.");
        assert!(lines.len() > 3);
        assert!(lines[1..].iter().all(|line| line.starts_with("      ")));
    }

    #[test]
    fn uninstall_parses_its_flags_and_needs_a_package() {
        use clap::CommandFactory;

        let cli = Cli::try_parse_from([
            "rabbit",
            "uninstall",
            "--resource-path",
            "C:\\REAPER",
            "--target-app-path",
            "C:\\REAPER\\reaper.exe",
            "--package",
            "sws",
            "--package",
            "reapack",
            "--remove-settings",
            "--apply",
            "--report-path",
            "C:\\report.json",
            "--save-report",
            "--json",
        ])
        .unwrap();
        match cli.command {
            Command::Uninstall {
                resource_path,
                target_app_path,
                package,
                remove_settings,
                apply,
                report_path,
                save_report,
                json,
            } => {
                assert_eq!(resource_path, PathBuf::from("C:\\REAPER"));
                assert_eq!(
                    target_app_path,
                    Some(PathBuf::from("C:\\REAPER\\reaper.exe"))
                );
                assert_eq!(package, vec!["sws".to_string(), "reapack".to_string()]);
                assert!(remove_settings && apply && save_report && json);
                assert_eq!(report_path, Some(PathBuf::from("C:\\report.json")));
            }
            other => panic!("unexpected command: {other:?}"),
        }
        // A dry run unless told otherwise, with settings kept.
        let cli = Cli::try_parse_from([
            "rabbit",
            "uninstall",
            "--resource-path",
            "C:\\REAPER",
            "--package",
            "sws",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Uninstall {
                apply: false,
                remove_settings: false,
                ..
            }
        ));

        // There is no "remove everything".
        assert!(
            Cli::try_parse_from(["rabbit", "uninstall", "--resource-path", "C:\\REAPER"]).is_err()
        );
        // And no way to run it with REAPER open; the help says why.
        let error = Cli::try_parse_from([
            "rabbit",
            "uninstall",
            "--resource-path",
            "C:\\REAPER",
            "--package",
            "sws",
            "--allow-reaper-running",
        ])
        .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
        let help = Cli::command()
            .find_subcommand("uninstall")
            .and_then(|command| command.get_after_help().map(ToString::to_string))
            .unwrap();
        assert!(
            help.contains("There is no --allow-reaper-running"),
            "{help}"
        );
        assert!(help.contains("every REAPER on the computer must be closed"));
    }

    #[test]
    fn restore_backup_parses_one_package() {
        let cli = Cli::try_parse_from([
            "rabbit",
            "restore-backup",
            "--resource-path",
            "C:\\REAPER",
            "--backup-id",
            "uninstall-unix-1",
            "--package",
            "sws",
        ])
        .unwrap();
        match cli.command {
            Command::RestoreBackup {
                backup_id, package, ..
            } => {
                assert_eq!(backup_id, "uninstall-unix-1");
                assert_eq!(package.as_deref(), Some("sws"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    /// A typo must not pass for "nothing to remove": an id no package on
    /// this platform has stops the command before anything runs.
    #[test]
    fn uninstall_package_ids_are_checked_up_front() {
        use rabbit_core::model::Platform;

        let ids = |ids: &[&str]| ids.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(
            super::validate_uninstall_packages(
                &ids(&["sws", " sws ", "reapack"]),
                Platform::Windows
            )
            .unwrap(),
            ids(&["sws", "reapack"])
        );
        let unknown = super::validate_uninstall_packages(&ids(&["swss"]), Platform::Windows)
            .unwrap_err()
            .to_string();
        assert!(unknown.contains("unknown package \"swss\""), "{unknown}");
        assert!(
            unknown.contains("sws, "),
            "the error lists what there is: {unknown}"
        );
        let elsewhere = super::validate_uninstall_packages(&ids(&["ffmpeg"]), Platform::MacOs)
            .unwrap_err()
            .to_string();
        assert!(elsewhere.contains("isn't offered on macOS"), "{elsewhere}");
    }

    /// A portable REAPER in a temporary folder with SWS and OSARA in it, as
    /// this platform names their plug-ins.
    struct FakeReaper {
        _dir: tempfile::TempDir,
        resource: PathBuf,
        sws: PathBuf,
    }

    impl FakeReaper {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let resource = dir.path().join("REAPER");
            let suffix = if cfg!(windows) { "-x64.dll" } else { ".dylib" };
            let plugins = resource.join("UserPlugins");
            std::fs::create_dir_all(&plugins).unwrap();
            let sws = plugins.join(format!("reaper_sws{suffix}"));
            std::fs::write(&sws, b"SWS").unwrap();
            std::fs::write(plugins.join(format!("reaper_osara{suffix}")), b"OSARA").unwrap();
            std::fs::write(resource.join("S&M.ini"), b"[settings]").unwrap();
            Self {
                _dir: dir,
                resource,
                sws,
            }
        }

        fn request(&self, ids: &[&str], apply: bool) -> rabbit_core::uninstall::UninstallRequest {
            rabbit_core::uninstall::UninstallRequest {
                resource_path: self.resource.clone(),
                platform: rabbit_core::model::Platform::current().unwrap(),
                package_ids: ids.iter().map(ToString::to_string).collect(),
                remove_settings: false,
                dry_run: !apply,
                target_app_path: None,
                target: None,
            }
        }

        fn run(&self, ids: &[&str], apply: bool) -> rabbit_core::uninstall::UninstallReport {
            super::uninstall_report(
                &self.request(ids, apply),
                &rabbit_core::cancel::CancelToken::new(),
            )
            .unwrap()
        }

        fn receipt(&self) -> rabbit_core::receipt::InstallState {
            rabbit_core::receipt::load_install_state(&self.resource)
                .unwrap()
                .unwrap_or_default()
        }
    }

    /// One exit-code table for the dry run and `--apply`: 0 when every
    /// package can go or isn't there, 2 when one can't and nothing changed,
    /// 1 when a run that had started stopped.
    #[test]
    fn uninstall_exit_codes_follow_the_table() {
        let reaper = FakeReaper::new();

        let preview = reaper.run(&["sws"], false);
        assert_eq!(super::uninstall_exit_code(&preview), 0);
        assert!(reaper.sws.is_file(), "a dry run changes nothing");

        // Not installed: nothing to do, so a script can run twice.
        assert_eq!(
            super::uninstall_exit_code(&reaper.run(&["reakontrol"], false)),
            0
        );
        assert_eq!(
            super::uninstall_exit_code(&reaper.run(&["reakontrol"], true)),
            0
        );

        // OSARA can't be uninstalled yet, so asking for it with SWS blocks
        // both, in the dry run and with --apply alike.
        assert_eq!(
            super::uninstall_exit_code(&reaper.run(&["sws", "osara"], false)),
            2
        );
        let blocked = reaper.run(&["sws", "osara"], true);
        assert_eq!(super::uninstall_exit_code(&blocked), 2);
        assert!(reaper.sws.is_file(), "a blocked run changes nothing");
        assert!(blocked.backup_set_id.is_none());

        let removed = reaper.run(&["sws"], true);
        assert_eq!(super::uninstall_exit_code(&removed), 0, "{removed:#?}");
        assert!(!reaper.sws.exists());
        assert!(reaper.receipt().declined_packages.contains("sws"));

        // Stopped before SWS could go: the run asked for didn't happen.
        std::fs::write(&reaper.sws, b"SWS again").unwrap();
        let cancel = rabbit_core::cancel::CancelToken::new();
        cancel.cancel();
        let stopped = super::uninstall_report(&reaper.request(&["sws"], true), &cancel).unwrap();
        assert_eq!(super::uninstall_exit_code(&stopped), 1);
        assert!(reaper.sws.is_file());
    }

    /// `--json` prints the report: the preflight, one item per package with
    /// its status and message code, and the plan for just those packages,
    /// with the keys of anything a UI would put in its own language.
    #[test]
    fn uninstall_json_carries_the_plan_and_the_message_codes() {
        let reaper = FakeReaper::new();
        let report = reaper.run(&["sws", "osara"], false);
        let json = serde_json::to_value(&report).unwrap();

        assert_eq!(json["dry_run"], true);
        assert_eq!(json["target"], "portable");
        assert!(
            json["preflight"]["checks"]
                .as_array()
                .is_some_and(|checks| !checks.is_empty())
        );
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        let item = |id: &str| {
            items
                .iter()
                .find(|item| item["package_id"] == id)
                .unwrap_or_else(|| panic!("no item for {id}: {json:#}"))
        };
        assert_eq!(item("sws")["status"], "would-remove");
        assert_eq!(item("sws")["message_code"]["code"], "would-remove");
        let removed = item("sws")["removed"].as_array().unwrap();
        assert!(
            removed.iter().any(|file| file["path"].as_str().is_some_and(
                |path| path.ends_with(reaper.sws.file_name().unwrap().to_str().unwrap())
            )),
            "{removed:#?}"
        );
        assert_eq!(item("osara")["status"], "blocked");
        assert_eq!(item("osara")["message_code"]["code"], "not-removable");
        assert_eq!(
            item("osara")["message_code"]["removability"],
            serde_json::json!({ "kind": "not-supported", "note_key": "uninstall-not-yet" })
        );

        let planned: Vec<&str> = json["plan"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["package_id"].as_str())
            .collect();
        assert_eq!(planned.len(), 2, "only the packages asked for: {planned:?}");
        let sws_plan = json["plan"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["package_id"] == "sws")
            .unwrap();
        assert!(
            sws_plan["kept"]
                .as_array()
                .unwrap()
                .iter()
                .any(|kept| kept["note_key"] == "uninstall-kept-sws-settings"),
            "{sws_plan:#}"
        );
    }

    /// Uninstall SWS through the CLI's own path, then put just SWS back with
    /// `restore-backup --package`: its files return, its receipt entry is
    /// merged into the receipt as it is now, and it is no longer remembered
    /// as removed.
    #[test]
    fn restore_backup_brings_one_uninstalled_package_back() {
        use rabbit_core::receipt::{InstalledFileReceipt, PackageReceipt, save_install_state};

        let reaper = FakeReaper::new();
        let mut state = rabbit_core::receipt::InstallState::default();
        let entry = PackageReceipt {
            id: "sws".to_string(),
            version: None,
            variant: None,
            channel: None,
            source_url: None,
            source_sha256: None,
            installed_files: vec![InstalledFileReceipt {
                path: reaper
                    .sws
                    .strip_prefix(&reaper.resource)
                    .unwrap()
                    .to_path_buf(),
                sha256: Some(rabbit_core::hash::sha256_file(&reaper.sws).unwrap()),
                size: Some(3),
            }],
            installed_at: None,
            rabbit_version: None,
            architecture: None,
            vendor_added_files: Vec::new(),
        };
        state.packages.insert("sws".to_string(), entry.clone());
        state.declined_packages.insert("langpack-de".to_string());
        save_install_state(&reaper.resource, &state).unwrap();

        let removed = reaper.run(&["sws"], true);
        assert_eq!(super::uninstall_exit_code(&removed), 0);
        let set_id = removed.backup_set_id.clone().unwrap();
        assert!(!reaper.sws.exists());
        assert!(!reaper.receipt().packages.contains_key("sws"));

        let resource = reaper.resource.to_string_lossy().into_owned();
        let restore = Cli::try_parse_from([
            "rabbit",
            "restore-backup",
            "--resource-path",
            resource.as_str(),
            "--backup-id",
            set_id.as_str(),
            "--package",
            "sws",
            "--apply",
            "--allow-reaper-running",
            "--json",
        ])
        .unwrap();
        super::run_with(restore).unwrap();

        assert_eq!(std::fs::read(&reaper.sws).unwrap(), b"SWS");
        let receipt = reaper.receipt();
        assert_eq!(receipt.packages.get("sws"), Some(&entry));
        assert!(!receipt.declined_packages.contains("sws"));
        assert!(
            receipt.declined_packages.contains("langpack-de"),
            "the receipt is merged into, not replaced"
        );
    }
}
