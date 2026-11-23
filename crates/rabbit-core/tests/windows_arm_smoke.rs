//! Real-upstream smoke tests for REAPER on Windows on ARM.
//!
//! REAPER for Windows on ARM is an ARM64EC program. An ARM64EC process loads
//! x64, ARM64EC and ARM64X libraries, but no plain ARM64 or x86 ones. These
//! tests check against the real downloads what RABBIT would put into such a
//! REAPER, and what the probe reads from REAPER's own builds:
//!
//! - `arm64ec_builds_load_in_an_arm64ec_reaper` resolves what RABBIT would
//!   download on Windows for an arm64-ec REAPER, takes each download apart
//!   as RABBIT's install does, and reads every program file and library in
//!   it with `arch_probe::classify_pe_file`. It prints a table and fails if
//!   a library RABBIT would put into that REAPER can't be loaded by an
//!   ARM64EC process. `x64_builds_load_in_an_x64_reaper` does the same for
//!   an x64 REAPER, as a control.
//! - `reaper_builds_probe_as_their_architecture` installs REAPER's arm64ec
//!   and x64 builds as portable copies and checks what the probe reads.
//!
//! Everything is written into temporary folders. The only installer that
//! runs is REAPER's, as the portable install RABBIT itself starts, in its
//! download folder and with `/D=` naming a temporary folder. The test never
//! asks for elevation: it refuses a plan that would elevate and starts the
//! installer with CreateProcess, which can't prompt. On GitHub's runners
//! every step has admin rights anyway, so there it is RABBIT's `/PORTABLE`
//! arguments and the `/D=` check that keep the install in the temporary
//! folder. The other vendor installers (OSARA, SWS, Surge XT, the JAWS
//! scripts) are listed as skipped: RABBIT runs them rather than taking them
//! apart, and running them would change the computer outside a temporary
//! folder. Each table is also written to the temporary folder as
//! `rabbit-windows-arm-smoke-<architecture>.md`, which the workflow adds to
//! the run's summary.
//!
//! The tests are `#[ignore]`d so a normal `cargo test` never downloads
//! anything, and even with `--ignored` they download and run nothing unless
//! `RABBIT_WINDOWS_ARM_SMOKE` is `1`, so a by-hand `cargo test --
//! --ignored` doesn't start REAPER's installer. `windows-arm-smoke.yml`
//! sets it and runs them on a Windows on ARM runner with `cargo test -p
//! rabbit-core --test windows_arm_smoke -- --ignored --nocapture
//! --test-threads=1`. The file compiles on every platform; REAPER's
//! installer runs only on Windows, so elsewhere REAPER is listed as
//! skipped.
//!
//! Like `macos_upstream_smoke.rs`, the file is named without "install" or
//! "setup", so Windows' installer detection doesn't ask to elevate the test
//! binary.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use rabbit_core::arch_probe::{PeKind, classify_pe_file};
use rabbit_core::archive::{
    extract_bin_directory_from_seven_zip_archive, extract_plugin_bundle_from_archive,
    extract_user_plugin_from_archive,
};
use rabbit_core::artifact::{
    ArtifactKind, CachedArtifact, download_artifacts, package_has_build_for,
    resolve_latest_artifacts,
};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::package::{
    PACKAGE_REAPER, PackageSpec, builtin_package_specs, package_specs_by_id,
};
use tempfile::tempdir;

/// The files a row is about: programs, and the libraries and CLAP and VST3
/// plug-ins REAPER loads.
const BINARY_EXTENSIONS: [&str; 4] = ["dll", "exe", "clap", "vst3"];

/// The files REAPER loads into its own process. A program (`.exe`) runs as
/// a process of its own, whatever REAPER is.
const LOADED_IN_PROCESS: [&str; 3] = ["dll", "clap", "vst3"];

/// The REAPER a table is about, and which libraries its process loads.
struct Reaper {
    architecture: Architecture,
    loads: fn(PeKind) -> bool,
}

/// One row of a table.
struct Row {
    package: String,
    /// The architecture RABBIT records for the download, such as `arm64-ec`.
    build: String,
    file: String,
    kind: Option<PeKind>,
    result: String,
    /// The row fails the test.
    fails: bool,
}

impl Row {
    /// A row about a package as a whole, rather than one of its files.
    fn package(package: &str, build: &str, file: &str, result: String, fails: bool) -> Self {
        Self {
            package: package.to_string(),
            build: build.to_string(),
            file: file.to_string(),
            kind: None,
            result,
            fails,
        }
    }
}

/// The environment variable that lets the ignored tests download and run
/// anything. Only `windows-arm-smoke.yml` sets it to `1`.
const SMOKE_SWITCH: &str = "RABBIT_WINDOWS_ARM_SMOKE";

/// Whether [`SMOKE_SWITCH`] is `1`. When it isn't, the test says so and
/// does nothing, as the Trash smoke test in rabbit-platform does.
fn smoke_run_wanted() -> bool {
    if std::env::var_os(SMOKE_SWITCH).as_deref() == Some(std::ffi::OsStr::new("1")) {
        return true;
    }
    eprintln!("{SMOKE_SWITCH} isn't 1, so nothing was downloaded or run");
    false
}

#[test]
#[ignore = "downloads every Windows package RABBIT offers an arm64-ec REAPER, and runs REAPER's installer as a portable install; run by the windows-arm-smoke workflow with RABBIT_WINDOWS_ARM_SMOKE=1"]
fn arm64ec_builds_load_in_an_arm64ec_reaper() {
    check_builds_for(&Reaper {
        architecture: Architecture::Arm64Ec,
        loads: PeKind::loads_in_arm64ec,
    });
}

#[test]
#[ignore = "downloads every Windows package RABBIT offers an x64 REAPER, and runs REAPER's installer as a portable install; run by the windows-arm-smoke workflow with RABBIT_WINDOWS_ARM_SMOKE=1"]
fn x64_builds_load_in_an_x64_reaper() {
    check_builds_for(&Reaper {
        architecture: Architecture::X64,
        loads: |kind| kind == PeKind::X64,
    });
}

/// REAPER's arm64ec build reads as ARM64EC and its x64 build as x64, both
/// from the program file and through detection of the portable folder.
#[cfg(windows)]
#[test]
#[ignore = "downloads REAPER's arm64ec and x64 installers and runs each as a portable install; run by the windows-arm-smoke workflow with RABBIT_WINDOWS_ARM_SMOKE=1"]
fn reaper_builds_probe_as_their_architecture() {
    use rabbit_core::arch_probe::probe_executable_architecture;
    use rabbit_core::detection::target_reaper_architecture;

    if !smoke_run_wanted() {
        return;
    }
    for (architecture, kind) in [
        (Architecture::Arm64Ec, PeKind::Arm64Ec),
        (Architecture::X64, PeKind::X64),
    ] {
        let work = tempdir().unwrap();
        let artifact = resolve_latest_artifacts(
            &[PACKAGE_REAPER.to_string()],
            Platform::Windows,
            architecture,
        )
        .expect("resolve REAPER")
        .remove(0);
        let cached = download_artifacts(&[artifact], &work.path().join("download"))
            .expect("download REAPER")
            .remove(0);
        let folder = work.path().join("REAPER");
        install_portable_reaper(&cached, &folder, work.path())
            .unwrap_or_else(|error| panic!("{architecture:?}: {error}"));

        let program = folder.join("reaper.exe");
        assert_eq!(classify_pe_file(&program), kind, "{architecture:?}");
        assert_eq!(
            probe_executable_architecture(&program),
            architecture,
            "{architecture:?}"
        );
        assert_eq!(
            target_reaper_architecture(Platform::Windows, &folder, None),
            architecture,
            "{architecture:?}"
        );
    }
}

/// Resolve, download and take apart every Windows package for `reaper`,
/// print the table, and fail on any row that fails.
fn check_builds_for(reaper: &Reaper) {
    if !smoke_run_wanted() {
        return;
    }
    let work = tempdir().unwrap();
    let mut rows = Vec::new();
    for spec in builtin_package_specs(Platform::Windows) {
        rows.extend(package_rows(&spec, reaper, work.path()));
    }

    let name = architecture_name(reaper.architecture);
    let table = table(
        &format!("Windows packages for an {name} REAPER"),
        reaper.architecture,
        &rows,
    );
    println!("{table}");
    let report = std::env::temp_dir().join(format!("rabbit-windows-arm-smoke-{name}.md"));
    if let Err(error) = fs::write(&report, &table) {
        println!("couldn't write {}: {error}", report.display());
    }

    let failures: Vec<String> = rows
        .iter()
        .filter(|row| row.fails)
        .map(|row| format!("{} {}: {}", row.package, row.file, row.result))
        .collect();
    assert!(
        failures.is_empty(),
        "for an {name} REAPER:\n{}",
        failures.join("\n")
    );
}

/// The rows for one package: one per program file and library RABBIT would
/// install, or one saying why there are none.
fn package_rows(spec: &PackageSpec, reaper: &Reaper, work: &Path) -> Vec<Row> {
    let package = spec.display_name.as_str();
    if !package_has_build_for(spec, Platform::Windows, reaper.architecture) {
        return vec![Row::package(
            package,
            "-",
            "-",
            "not offered: RABBIT has no build of it for this REAPER".to_string(),
            false,
        )];
    }
    let artifact = match resolve_latest_artifacts(
        std::slice::from_ref(&spec.id),
        Platform::Windows,
        reaper.architecture,
    ) {
        Ok(mut artifacts) if artifacts.len() == 1 => artifacts.remove(0),
        Ok(artifacts) => {
            return vec![Row::package(
                package,
                "-",
                "-",
                format!("resolved {} downloads instead of one", artifacts.len()),
                true,
            )];
        }
        Err(error) => {
            return vec![Row::package(
                package,
                "-",
                "-",
                format!("couldn't find the download: {error}"),
                true,
            )];
        }
    };
    let build = architecture_name(artifact.architecture);
    if let Some(reason) = not_taken_apart(&spec.id, artifact.kind) {
        return vec![Row::package(
            package,
            build,
            &artifact.file_name,
            format!("skipped: {reason}"),
            false,
        )];
    }

    let folder = work.join(&spec.id);
    let cached = match download_artifacts(std::slice::from_ref(&artifact), &folder.join("download"))
    {
        Ok(mut cached) if cached.len() == 1 => cached.remove(0),
        Ok(_) => {
            return vec![Row::package(
                package,
                build,
                &artifact.file_name,
                "the download came back empty".to_string(),
                true,
            )];
        }
        Err(error) => {
            return vec![Row::package(
                package,
                build,
                &artifact.file_name,
                format!("couldn't download it: {error}"),
                true,
            )];
        }
    };
    let files = match files_rabbit_installs(&cached, &folder.join("installed"), &folder) {
        Ok(files) => files,
        Err(error) => {
            return vec![Row::package(
                package,
                build,
                &artifact.file_name,
                format!("couldn't take it apart: {error}"),
                true,
            )];
        }
    };

    let binaries: Vec<(String, PathBuf)> = files
        .into_iter()
        .filter(|(name, _)| has_extension(name, &BINARY_EXTENSIONS))
        .collect();
    if binaries.is_empty() {
        return vec![Row::package(
            package,
            build,
            &artifact.file_name,
            "no program files or libraries".to_string(),
            false,
        )];
    }
    binaries
        .into_iter()
        .map(|(name, path)| {
            let kind = classify_pe_file(&path);
            let (result, fails) = if !has_extension(&name, &LOADED_IN_PROCESS) {
                ("not checked: runs as its own process".to_string(), false)
            } else if spec.id == PACKAGE_REAPER {
                ("not checked: part of REAPER itself".to_string(), false)
            } else if (reaper.loads)(kind) {
                ("ok".to_string(), false)
            } else {
                (
                    format!(
                        "FAILS: an {} REAPER can't load it",
                        architecture_name(reaper.architecture)
                    ),
                    true,
                )
            };
            Row {
                package: package.to_string(),
                build: build.to_string(),
                file: name,
                kind: Some(kind),
                result,
                fails,
            }
        })
        .collect()
}

/// Why a download of `kind` for `package_id` isn't taken apart, or `None`
/// when it is.
fn not_taken_apart(package_id: &str, kind: ArtifactKind) -> Option<&'static str> {
    match kind {
        ArtifactKind::Installer if package_id != PACKAGE_REAPER => Some(
            "a vendor installer, which RABBIT runs rather than takes apart, and which would \
             change the computer outside a temporary folder",
        ),
        ArtifactKind::Installer if !cfg!(windows) => {
            Some("REAPER's installer runs only on Windows")
        }
        ArtifactKind::DiskImage => Some("a macOS disk image"),
        _ => None,
    }
}

/// The files RABBIT would put into REAPER from `cached`, each with the name
/// a row shows, taken apart into `into` as `install::prepare_install_source`
/// does: an extension binary is copied as it is, a zip gives the one file
/// that matches the package's plug-in names, or for a plug-in bundle every
/// file of its bundle folder, and FFmpeg's `.7z` everything
/// in its `bin` folder. REAPER's installer installs a portable REAPER into
/// `into`, and every file of it counts. `work` is a temporary folder for
/// the dry run that plans it.
fn files_rabbit_installs(
    cached: &CachedArtifact,
    into: &Path,
    work: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    let descriptor = &cached.descriptor;
    let spec = || {
        package_specs_by_id(descriptor.platform)
            .remove(&descriptor.package_id)
            .ok_or_else(|| format!("no package {}", descriptor.package_id))
    };
    let named = |path: PathBuf| {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        (name, path)
    };
    match descriptor.kind {
        ArtifactKind::ExtensionBinary => {
            Ok(vec![(descriptor.file_name.clone(), cached.path.clone())])
        }
        // A package with a plug-in bundle, such as Lo-lo's VST3 instruments,
        // installs the archive's whole bundle folder; every file of it counts.
        ArtifactKind::Archive => {
            let spec = spec()?;
            match &spec.plugin_bundle {
                Some(bundle) => extract_plugin_bundle_from_archive(&cached.path, bundle, into)
                    .map(|extracted| {
                        extracted
                            .into_iter()
                            .map(|file| named(file.extracted_path))
                            .collect()
                    })
                    .map_err(|error| error.to_string()),
                None => extract_user_plugin_from_archive(&cached.path, &spec, into)
                    .map(|extracted| vec![named(extracted.extracted_path)])
                    .map_err(|error| error.to_string()),
            }
        }
        ArtifactKind::SevenZipArchive => {
            extract_bin_directory_from_seven_zip_archive(&cached.path, &spec()?, into)
                .map(|extracted| {
                    extracted
                        .into_iter()
                        .map(|file| named(file.extracted_path))
                        .collect()
                })
                .map_err(|error| error.to_string())
        }
        ArtifactKind::Installer if descriptor.package_id == PACKAGE_REAPER => {
            install_reaper_for_rows(cached, into, work)
        }
        kind => Err(format!("RABBIT doesn't take apart a {kind:?} here")),
    }
}

/// [`install_portable_reaper`] into `into`, and every file of it, named by
/// its path in the folder.
#[cfg(windows)]
fn install_reaper_for_rows(
    cached: &CachedArtifact,
    into: &Path,
    work: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    install_portable_reaper(cached, into, work)?;
    Ok(files_under(into)
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(into)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            (name, path)
        })
        .collect())
}

/// REAPER's installer runs only on Windows; [`not_taken_apart`] skips it
/// everywhere else before it is downloaded.
#[cfg(not(windows))]
fn install_reaper_for_rows(
    _cached: &CachedArtifact,
    _into: &Path,
    _work: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    Err("REAPER's installer runs only on Windows".to_string())
}

/// How long REAPER's installer may take before the test gives up on it.
#[cfg(windows)]
const INSTALLER_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(600);

/// Install REAPER from `cached`, its downloaded installer, as a portable
/// REAPER in `folder`, as RABBIT does. The arguments are RABBIT's own: a
/// dry run of its install for that folder plans them, the way the run
/// itself would. The test refuses a plan that would elevate, isn't a
/// portable install, or would install anywhere but `folder`, and starts the
/// installer as an ordinary child process. `work` is the temporary folder
/// that holds `folder`, and the dry run's files.
#[cfg(windows)]
fn install_portable_reaper(
    cached: &CachedArtifact,
    folder: &Path,
    work: &Path,
) -> Result<(), String> {
    use rabbit_core::operation::{PackageOperationOptions, execute_resolved_package_operation};

    if !folder.starts_with(work) {
        return Err(format!(
            "{} isn't inside the temporary folder {}",
            folder.display(),
            work.display()
        ));
    }
    fs::create_dir_all(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let program = folder.join("reaper.exe");
    let report = execute_resolved_package_operation(
        folder,
        vec![cached.descriptor.clone()],
        &work.join("dry-run-cache"),
        &PackageOperationOptions {
            dry_run: true,
            allow_reaper_running: true,
            stage_unsupported: false,
            replace_osara_keymap: false,
            target_app_path: Some(program.clone()),
            lock_path: None,
            force_reinstall_packages: Vec::new(),
            package_variants: Default::default(),
            package_channels: Default::default(),
        },
    )
    .map_err(|error| format!("RABBIT couldn't plan the install: {error}"))?;
    let plan = report
        .items
        .iter()
        .find_map(|item| item.planned_execution.clone())
        .ok_or("RABBIT planned no installer run")?;
    if plan.requires_elevation {
        return Err("RABBIT would elevate this install, and the test never does".to_string());
    }
    if !plan
        .arguments
        .iter()
        .any(|argument| argument == "/PORTABLE")
    {
        return Err(format!("not a portable install: {:?}", plan.arguments));
    }
    // NSIS installs where its last `/D=` says; there must be one, and it
    // must name `folder`.
    let destination = format!("/D={}", folder.display());
    let destinations: Vec<&String> = plan
        .arguments
        .iter()
        .filter(|argument| rabbit_platform::is_nsis_destination_argument(argument))
        .collect();
    if destinations != [&destination] {
        return Err(format!(
            "the install wouldn't go into {}: {:?}",
            folder.display(),
            plan.arguments
        ));
    }
    run_installer(&cached.path, &plan.arguments)?;
    if !program.is_file() {
        return Err(format!("the installer left no {}", program.display()));
    }
    Ok(())
}

/// Run `installer` with `arguments` as RABBIT's runner passes them, `/D=`
/// unquoted for NSIS, without RABBIT's secrets, and wait for it, at most
/// [`INSTALLER_TIME_LIMIT`]. It starts in its own folder, the temporary
/// download folder, as RABBIT's run starts it in the download cache.
#[cfg(windows)]
fn run_installer(installer: &Path, arguments: &[String]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::time::{Duration, Instant};

    let download_folder = installer
        .parent()
        .ok_or_else(|| format!("{} has no folder", installer.display()))?;
    let mut command = Command::new(installer);
    command.current_dir(download_folder);
    rabbit_platform::process::without_rabbit_secrets(&mut command);
    for argument in arguments {
        if rabbit_platform::is_nsis_destination_argument(argument) {
            command.raw_arg(argument);
        } else {
            command.arg(argument);
        }
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("couldn't start {}: {error}", installer.display()))?;
    let deadline = Instant::now() + INSTALLER_TIME_LIMIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("the installer ended with {status}")),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(500)),
            Ok(None) => {
                let _ = child.kill();
                return Err(format!(
                    "the installer didn't finish within {} seconds",
                    INSTALLER_TIME_LIMIT.as_secs()
                ));
            }
            Err(error) => return Err(format!("couldn't wait for the installer: {error}")),
        }
    }
}

/// Every file under `folder`, in its subfolders too, sorted.
#[cfg(windows)]
fn files_under(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut folders = vec![folder.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                folders.push(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    files
}

/// Whether `name` ends in one of `extensions`, ignoring case.
fn has_extension(name: &str, extensions: &[&str]) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions
                .iter()
                .any(|wanted| extension.eq_ignore_ascii_case(wanted))
        })
}

/// The name RABBIT's reports and manifests give `architecture`.
fn architecture_name(architecture: Architecture) -> &'static str {
    match architecture {
        Architecture::X86 => "x86",
        Architecture::X64 => "x64",
        Architecture::Arm64 => "arm64",
        Architecture::Arm64Ec => "arm64-ec",
        Architecture::Universal => "universal",
        Architecture::Unknown => "unknown",
    }
}

/// `rows` as a Markdown table under `title`, which reads as plain text in
/// the test's output and as a table in the run's summary.
fn table(title: &str, architecture: Architecture, rows: &[Row]) -> String {
    let mut text = format!(
        "### {title}\n\n\
         | Package | Build | File | Kind | An ARM64EC process loads it | Result for an {} REAPER |\n\
         | --- | --- | --- | --- | --- | --- |\n",
        architecture_name(architecture)
    );
    for row in rows {
        let (kind, loads) = match row.kind {
            Some(kind) => (
                kind.name(),
                if kind.loads_in_arm64ec() { "yes" } else { "no" },
            ),
            None => ("-", "-"),
        };
        let _ = writeln!(
            text,
            "| {} | {} | {} | {kind} | {loads} | {} |",
            cell(&row.package),
            cell(&row.build),
            cell(&row.file),
            cell(&row.result)
        );
    }
    text
}

/// `text` made safe for one cell of a Markdown table.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}
