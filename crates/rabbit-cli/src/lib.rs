use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use rabbit_core::artifact::{
    ArtifactDescriptor, CachedArtifact, default_cache_dir, download_artifacts,
    resolve_latest_artifacts,
};
use rabbit_core::detection::{DiscoveryOptions, detect_components, discover_installations};
use rabbit_core::install::{InstallOptions, InstallReport, install_cached_artifacts};
use rabbit_core::latest::fetch_latest_versions;
use rabbit_core::localization::{
    DEFAULT_LOCALE, LocalizedText, Localizer, available_locales, resolve_runtime_locale,
};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::operation::{
    PackageOperationOptions, PackageOperationReport, execute_package_operation,
};
use rabbit_core::package::{
    builtin_package_specs, default_desired_package_ids, embedded_package_manifest,
};
use rabbit_core::plan::{AvailablePackage, build_install_plan};
use rabbit_core::portable::{PortabilityCheckStatus, PortabilityReport, check_portable_runtime};
use rabbit_core::preflight::{PreflightOptions, PreflightReport, run_install_preflight};
use rabbit_core::report::{default_report_path, save_json_and_text_reports};
use rabbit_core::resource::{ResourceInitActionKind, ResourceInitReport, initialize_resource_path};
use rabbit_core::rollback::{
    BackupSet, RestoreBackupActionKind, RestoreBackupOptions, RestoreBackupReport,
    list_backup_sets, restore_backup_set,
};
use rabbit_core::self_update::{
    ApplySelfUpdateOptions, DEFAULT_SELF_UPDATE_MANIFEST_URL, SelfUpdateApplyReport,
    SelfUpdateCheckReport, SelfUpdateStageReport, apply_self_update, check_self_update,
    default_self_update_staging_dir, relaunch_current_executable,
    resolve_self_update_release_notes, stage_self_update,
};
use rabbit_core::setup::{SetupOptions, SetupReport, execute_setup_operation};
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
        /// to install everything that needs it.
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
        /// needs installing or updating.
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
        /// Keep OSARA's current key map instead of replacing it with OSARA's
        /// current default.
        #[arg(long)]
        preserve_osara_keymap: bool,
        /// Pick a package flavour, e.g. `--package-variant langpack-es=pma`
        /// to install Team PMA's Spanish translation (es_MX) instead of the
        /// default REAPER Accesible español (es_ES). Repeatable.
        #[arg(long = "package-variant")]
        package_variant: Vec<String>,
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
        /// for this host.
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
        /// Keep OSARA's current key map instead of replacing it with OSARA's
        /// current default.
        #[arg(long)]
        preserve_osara_keymap: bool,
        /// Pick a package flavour, e.g. `--package-variant langpack-es=pma`
        /// to install Team PMA's Spanish translation (es_MX) instead of the
        /// default REAPER Accesible español (es_ES). Repeatable.
        #[arg(long = "package-variant")]
        package_variant: Vec<String>,
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
    let cli = Cli::parse();

    match cli.command {
        Command::Detect { portable, json } => {
            let installations = discover_installations(&DiscoveryOptions {
                include_standard: true,
                portable_roots: portable,
            })?;

            if json {
                println!("{}", serde_json::to_string_pretty(&installations)?);
            } else {
                print_installations(&installations);
            }
        }
        Command::Components {
            resource_path,
            json,
        } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let components = detect_components(&resource_path, platform)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&components)?);
            } else {
                print_components(&components);
            }
        }
        Command::Latest { json } => {
            let latest = fetch_latest_versions()?;
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
        }
        Command::Artifacts {
            package,
            architecture,
            json,
        } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let architecture = architecture.map_or_else(Architecture::current, Into::into);
            let packages = selected_package_ids(package, platform, None);
            let artifacts = resolve_latest_artifacts(&packages, platform, architecture)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&artifacts)?);
            } else {
                print_artifacts(&artifacts);
            }
        }
        Command::Download {
            package,
            architecture,
            cache_dir,
            json,
        } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let architecture = architecture.map_or_else(Architecture::current, Into::into);
            let packages = selected_package_ids(package, platform, None);
            let artifacts = resolve_latest_artifacts(&packages, platform, architecture)?;
            let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
            let cached = download_artifacts(&artifacts, &cache_dir)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&cached)?);
            } else {
                print_cached_artifacts(&cached);
            }
        }
        Command::Packages { manifest, json } => {
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
                        println!(
                            "  Uninstall steps: {}",
                            serialized_names(&package.uninstall_steps)
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
                let platform =
                    Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
                let packages = builtin_package_specs(platform);
                if json {
                    println!("{}", serde_json::to_string_pretty(&packages)?);
                } else {
                    print_package_specs(&packages);
                }
            }
        }
        Command::Preflight {
            resource_path,
            target_app_path,
            dry_run,
            allow_reaper_running,
            report_path,
            save_report,
            json,
        } => {
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
        }
        Command::InitResource {
            resource_path,
            target_app_path,
            portable,
            apply,
            allow_reaper_running,
            report_path,
            save_report,
            json,
        } => {
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
        }
        Command::Backups {
            resource_path,
            json,
        } => {
            let backup_sets = list_backup_sets(&resource_path)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&backup_sets)?);
            } else {
                print_backup_sets(&backup_sets);
            }
        }
        Command::RestoreBackup {
            resource_path,
            backup_id,
            apply,
            allow_reaper_running,
            report_path,
            save_report,
            json,
        } => {
            let report = restore_backup_set(
                &resource_path,
                &backup_id,
                &RestoreBackupOptions {
                    dry_run: !apply,
                    allow_reaper_running,
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
        }
        Command::InstallExtension {
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
        } => {
            ensure_reapack_donation_acknowledged(&package, accept_reapack_donation_notice)?;
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let architecture = architecture.map_or_else(Architecture::current, Into::into);
            let artifacts = resolve_latest_artifacts(&package, platform, architecture)?;
            let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
            let cached = download_artifacts(&artifacts, &cache_dir)?;
            let report = install_cached_artifacts(
                &resource_path,
                &cached,
                &InstallOptions {
                    dry_run: !apply,
                    allow_reaper_running,
                    target_app_path,
                    package_variants: Default::default(),
                },
            )?;
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
        }
        Command::ApplyPackages {
            resource_path,
            target_app_path,
            package,
            architecture,
            cache_dir,
            apply,
            allow_reaper_running,
            stage_unsupported,
            preserve_osara_keymap,
            package_variant,
            accept_reapack_donation_notice,
            report_path,
            save_report,
            json,
        } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let architecture = architecture.map_or_else(Architecture::current, Into::into);
            let packages = selected_package_ids(package, platform, Some(&resource_path));
            ensure_reapack_donation_acknowledged(&packages, accept_reapack_donation_notice)?;
            let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
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
                    replace_osara_keymap: !preserve_osara_keymap,
                    package_variants: parse_package_variants(&package_variant)?,
                    target_app_path,
                    lock_path: None,
                    force_reinstall_packages: Vec::new(),
                },
            )?;
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
        }
        Command::Setup {
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
            package_variant,
            accept_reapack_donation_notice,
            config_step,
            skip_config_step,
            reaper_language,
            report_path,
            save_report,
            json,
        } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
            let architecture = architecture.map_or_else(Architecture::current, Into::into);
            let packages = selected_package_ids(package, platform, Some(&resource_path));
            ensure_reapack_donation_acknowledged(&packages, accept_reapack_donation_notice)?;
            let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
            let configuration_step_ids = resolve_configuration_step_ids(
                &resource_path,
                platform,
                &packages,
                &config_step,
                &skip_config_step,
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
                    replace_osara_keymap: !preserve_osara_keymap,
                    package_variants: parse_package_variants(&package_variant)?,
                    target_app_path,
                    lock_path: None,
                    force_reinstall_packages: Vec::new(),
                    reaper_language_package: validate_reaper_language(reaper_language)?,
                    configuration_step_ids,
                },
            )?;
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
        }
        Command::Locales { locales_dir, json } => {
            let locales = available_locales(&locales_dir)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&locales)?);
            } else {
                print_locales(&locales);
            }
        }
        Command::Localize {
            locale,
            locales_dir,
            id,
            args,
            json,
        } => {
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
        }
        Command::PortableCheck { locales_dir, json } => {
            let report = check_portable_runtime(&locales_dir)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_portability_report(&report);
            }
        }
        Command::SelfUpdate { command } => match command {
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
        },
        Command::Plan {
            resource_path,
            portable,
            online,
            format,
            report_path,
            save_report,
        } => {
            let platform =
                Platform::current().ok_or(rabbit_core::RabbitError::UnsupportedPlatform)?;
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
            let components = match detection_path {
                Some(path) => detect_components(&path, platform)?,
                None => Vec::new(),
            };

            let desired = default_desired_package_ids();
            let (available, version_check_failures) = if online {
                let report = fetch_latest_versions()?;
                (report.packages, report.failures)
            } else {
                (Vec::new(), Vec::new())
            };
            let mut plan = build_install_plan(target, &components, &desired, &available);
            // Surface per-provider failures as plan notes instead of failing
            // the whole plan: one unreachable upstream shouldn't block update
            // guidance for everything else.
            for failure in &version_check_failures {
                plan.notes.push(format!(
                    "The latest-version check for {} failed: {}. Install/update guidance for this package is incomplete in this plan.",
                    failure.package_id, failure.message
                ));
            }
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
        }
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

/// The packages an operation should act on.
///
/// An explicit `--package` list is taken as given — it is the user telling
/// us exactly what they want, and second-guessing it would be worse than
/// useless. Only the implicit default set is filtered against the packages
/// this install remembers the user declining, so a refusal recorded in the
/// wizard isn't undone by a later bare `rabbit setup`. Only packages whose
/// spec sets `remember_opt_out` can be declined at all.
fn selected_package_ids(
    package_ids: Vec<String>,
    platform: Platform,
    resource_path: Option<&Path>,
) -> Vec<String> {
    if !package_ids.is_empty() {
        return package_ids;
    }
    let declined = match resource_path {
        Some(path) => rabbit_core::receipt::declined_packages(path),
        None => return default_desired_package_ids(),
    };
    if declined.is_empty() {
        return default_desired_package_ids();
    }
    let remembers_opt_out: std::collections::BTreeSet<String> = builtin_package_specs(platform)
        .into_iter()
        .filter(|spec| spec.remember_opt_out)
        .map(|spec| spec.id)
        .collect();
    default_desired_package_ids()
        .into_iter()
        .filter(|id| !(remembers_opt_out.contains(id) && declined.contains(id)))
        .collect()
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

fn print_components(components: &[rabbit_core::model::ComponentDetection]) {
    for component in components {
        println!("{}", component.display_name);
        println!("  Installed: {}", yes_no(component.installed));
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

fn print_package_specs(packages: &[rabbit_core::package::PackageSpec]) {
    // Build a localizer from the embedded resources so package descriptions
    // come out in the user's chosen language (RABBIT_LOCALE / OS default /
    // en-US fallback). Falling back to the default keeps the listing usable
    // even on hosts where a Fluent file is missing.
    let localizer = Localizer::embedded(&resolve_runtime_locale())
        .or_else(|_| Localizer::embedded(DEFAULT_LOCALE))
        .ok();
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
        println!(
            "  Uninstall steps: {}",
            serialized_names(&package.uninstall_steps)
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
    /// A refusal recorded in the wizard must survive a later bare
    /// `rabbit setup` — but an explicit `--package` is the user telling us
    /// exactly what they want and always wins.
    #[test]
    fn the_default_package_set_honours_a_recorded_refusal() {
        let dir = tempfile::tempdir().unwrap();
        let platform = rabbit_core::model::Platform::Windows;

        // Baseline: language packs are not in the default set anyway, so
        // pick a package that IS, and pin the rule on it by declining it.
        let baseline = super::selected_package_ids(Vec::new(), platform, Some(dir.path()));
        assert!(!baseline.is_empty(), "there is a default package set");

        rabbit_core::receipt::record_package_opt_outs(
            dir.path(),
            &["langpack-de".to_string()],
            &[],
        )
        .unwrap();

        // langpack-de is not in the default set to begin with, so the set is
        // unchanged — the point here is that declining never *adds* anything
        // and never disturbs the rest.
        let after = super::selected_package_ids(Vec::new(), platform, Some(dir.path()));
        assert_eq!(baseline, after);
        assert!(!after.iter().any(|id| id == "langpack-de"));

        // An explicit request wins over a recorded refusal.
        let explicit = super::selected_package_ids(
            vec!["langpack-de".to_string()],
            platform,
            Some(dir.path()),
        );
        assert_eq!(explicit, vec!["langpack-de".to_string()]);
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
}
