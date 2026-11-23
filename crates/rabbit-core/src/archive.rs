use std::fs;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use crate::error::{IoPathContext, RabbitError, Result};
use crate::package::{PackageSpec, PluginBundleSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedUserPlugin {
    pub source_archive: PathBuf,
    pub entry_name: String,
    pub extracted_path: PathBuf,
    pub file_name: String,
}

pub fn extract_user_plugin_from_archive(
    archive_path: &Path,
    spec: &PackageSpec,
    extract_dir: &Path,
) -> Result<ExtractedUserPlugin> {
    let file = fs::File::open(archive_path).with_path(archive_path)?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|source| RabbitError::ArchiveRead {
            archive: archive_path.to_path_buf(),
            message: source.to_string(),
        })?;

    let mut selected: Option<(usize, String, String)> = None;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|source| RabbitError::ArchiveRead {
                archive: archive_path.to_path_buf(),
                message: source.to_string(),
            })?;
        if !entry.is_file() {
            continue;
        }
        let entry_name = zip_entry_name(&entry, archive_path)?;
        let Some(basename) = Path::new(&entry_name)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
        else {
            continue;
        };
        if matches_user_plugin_file(&basename, spec) {
            selected = Some((index, entry_name, basename));
            break;
        }
    }

    let (index, entry_name, basename) =
        selected.ok_or_else(|| RabbitError::ArchiveMissingExtensionBinary {
            archive: archive_path.to_path_buf(),
            package_id: spec.id.clone(),
        })?;

    fs::create_dir_all(extract_dir).with_path(extract_dir)?;
    let extracted_path = extract_dir.join(&basename);
    if extracted_path.exists() {
        fs::remove_file(&extracted_path).with_path(&extracted_path)?;
    }

    let mut entry = archive
        .by_index(index)
        .map_err(|source| RabbitError::ArchiveRead {
            archive: archive_path.to_path_buf(),
            message: source.to_string(),
        })?;
    let mut output = fs::File::create(&extracted_path).with_path(&extracted_path)?;
    std::io::copy(&mut entry, &mut output).with_path(&extracted_path)?;
    output.flush().with_path(&extracted_path)?;

    Ok(ExtractedUserPlugin {
        source_archive: archive_path.to_path_buf(),
        entry_name,
        extracted_path,
        file_name: basename,
    })
}

/// One file of a plug-in bundle, extracted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedBundleFile {
    /// Where it goes inside the bundle folder, one name per component, as
    /// the archive spells them.
    pub relative: PathBuf,
    pub extracted_path: PathBuf,
}

/// Extract the plug-in bundle folder `bundle` names from the top of the zip
/// at `archive_path` into `extract_dir`, keeping the paths below it. Returns
/// the bundle's files, the binary last, each with its path inside the
/// bundle. The bundle folder is found ignoring ASCII case; everything else
/// in the archive is ignored.
///
/// Every entry of the archive is checked before anything is written, and
/// the whole archive is refused with [`RabbitError::ArchiveEntryRefused`]
/// for an entry that is a symbolic link, or whose name is absolute, has a
/// drive, a stream or an empty folder name, climbs out with `..`, or holds
/// a name Windows can't open as written, such as one ending in a dot or
/// naming a device. Inside the bundle, two files whose paths differ only in
/// case, and a path that is both a file and a folder, are refused the same
/// way: Windows would write one over the other. A bundle without its binary
/// is [`RabbitError::ArchiveMissingBundleBinary`]. `\` counts as a
/// separator too, as some Windows tools write it.
pub fn extract_plugin_bundle_from_archive(
    archive_path: &Path,
    bundle: &PluginBundleSpec,
    extract_dir: &Path,
) -> Result<Vec<ExtractedBundleFile>> {
    let file = fs::File::open(archive_path).with_path(archive_path)?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|source| RabbitError::ArchiveRead {
            archive: archive_path.to_path_buf(),
            message: source.to_string(),
        })?;
    let refused = |entry: &str, reason: &str| RabbitError::ArchiveEntryRefused {
        archive: archive_path.to_path_buf(),
        entry: entry.to_string(),
        reason: reason.to_string(),
    };

    // Every entry is checked first; only then is anything written.
    // Each file of the bundle: its index, its name and its names below the
    // bundle folder.
    let mut files: Vec<(usize, String, Vec<String>)> = Vec::new();
    let mut folders: Vec<Vec<String>> = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|source| RabbitError::ArchiveRead {
                archive: archive_path.to_path_buf(),
                message: source.to_string(),
            })?;
        let name = zip_entry_name(&entry, archive_path)?;
        if entry.is_symlink() || entry.external_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(refused(&name, "is a symbolic link"));
        }
        let components = bundle_entry_components(&name, entry.is_dir())
            .map_err(|reason| refused(&name, reason))?;
        if !components[0].eq_ignore_ascii_case(&bundle.name) {
            continue;
        }
        if entry.is_dir() {
            folders.push(components[1..].to_vec());
        } else if components.len() > 1 {
            files.push((index, name, components[1..].to_vec()));
        } else {
            return Err(refused(
                &name,
                "is a file where the plug-in bundle's folder should be",
            ));
        }
    }

    // Windows reads names ignoring case, so each file has a place of its
    // own and no file sits where another one's folder is.
    let folded = |components: &[String]| -> Vec<String> {
        components.iter().map(|name| name.to_lowercase()).collect()
    };
    let mut file_places = std::collections::BTreeSet::new();
    let mut folder_places: std::collections::BTreeSet<Vec<String>> =
        folders.iter().map(|folder| folded(folder)).collect();
    for (_, _, components) in &files {
        for depth in 1..components.len() {
            folder_places.insert(folded(&components[..depth]));
        }
    }
    for (_, name, components) in &files {
        let place = folded(components);
        if folder_places.contains(&place) {
            return Err(refused(name, "is both a file and a folder"));
        }
        if !file_places.insert(place) {
            return Err(refused(
                name,
                "is in the archive twice, perhaps spelt in other upper and lower case",
            ));
        }
    }

    let binary: Vec<String> = bundle.binary_components().map(str::to_lowercase).collect();
    let Some(binary_at) = files
        .iter()
        .position(|(_, _, components)| folded(components) == binary)
    else {
        return Err(RabbitError::ArchiveMissingBundleBinary {
            archive: archive_path.to_path_buf(),
            bundle: bundle.name.clone(),
            binary: bundle.binary.clone(),
        });
    };
    let binary_entry = files.remove(binary_at);
    files.sort_by(|left, right| left.2.cmp(&right.2));
    files.push(binary_entry);

    fs::create_dir_all(extract_dir).with_path(extract_dir)?;
    let mut extracted = Vec::with_capacity(files.len());
    for (index, _, components) in files {
        let relative: PathBuf = components.iter().collect();
        let target = extract_dir.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_path(parent)?;
        }
        let mut entry = archive
            .by_index(index)
            .map_err(|source| RabbitError::ArchiveRead {
                archive: archive_path.to_path_buf(),
                message: source.to_string(),
            })?;
        let mut output = fs::File::create(&target).with_path(&target)?;
        std::io::copy(&mut entry, &mut output).with_path(&target)?;
        output.flush().with_path(&target)?;
        extracted.push(ExtractedBundleFile {
            relative,
            extracted_path: target,
        });
    }
    Ok(extracted)
}

/// Windows' attribute of a symbolic link or junction, as a zip made on
/// Windows may record it in an entry's external attributes.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

/// The names an entry's path is made of, or why the entry is refused. The
/// one separator a folder entry ends with is dropped first.
fn bundle_entry_components(
    name: &str,
    is_dir: bool,
) -> std::result::Result<Vec<String>, &'static str> {
    if name.is_empty() {
        return Err("has no name");
    }
    if name.starts_with(['/', '\\']) {
        return Err("is an absolute path");
    }
    if name.contains(':') {
        return Err("names a drive or a stream");
    }
    let trimmed = if is_dir {
        name.strip_suffix(['/', '\\']).unwrap_or(name)
    } else {
        name
    };
    let mut components = Vec::new();
    for component in trimmed.split(['/', '\\']) {
        match component {
            "" => return Err("has an empty folder name"),
            "." | ".." => return Err("climbs out of its folder with . or .."),
            _ if !windows_opens_as_written(component) => {
                return Err("holds a name Windows can't open as written");
            }
            _ if is_rabbit_temporary_name(component) => {
                return Err("has a name RABBIT keeps for its own temporary files");
            }
            _ => components.push(component.to_string()),
        }
    }
    Ok(components)
}

/// Whether `name` ends the way the temporary copies of an install or a
/// restore do, ignoring ASCII case. Such an entry would take the name of
/// another file's copy while it waits beside its place, and could slip
/// unchecked bytes into that file's place.
fn is_rabbit_temporary_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".rabbit-tmp") || lower.ends_with(".rabbit-restore-tmp")
}

/// Whether Windows opens a file or folder by `name` under exactly that
/// name: no character it reserves, no control character, no dot or space
/// at the end, which it drops, and no device name such as `NUL`.
fn windows_opens_as_written(name: &str) -> bool {
    !name.is_empty()
        && !name.ends_with(['.', ' '])
        && !name.chars().any(|ch| {
            matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || ch.is_control()
        })
        && !crate::package::uninstall::is_windows_device_name(name)
}

fn matches_user_plugin_file(file_name: &str, spec: &PackageSpec) -> bool {
    let lower = file_name.to_ascii_lowercase();
    // An empty prefix list means "any name": REAPER language packs are named
    // after their language (es_ES.ReaperLangPack, Deutsch.ReaperLangPack…)
    // and share no common prefix, so they are identified by suffix alone.
    // Every other package pins both, and an empty list would otherwise match
    // nothing rather than everything.
    let prefix_match = spec.user_plugin_prefixes.is_empty()
        || spec
            .user_plugin_prefixes
            .iter()
            .any(|prefix| lower.starts_with(&prefix.to_ascii_lowercase()));
    let suffix_match = spec
        .user_plugin_suffixes
        .iter()
        .any(|suffix| lower.ends_with(&suffix.to_ascii_lowercase()));
    prefix_match && suffix_match
}

/// The entry's name as text. zip 9 decodes it on every call (UTF-8 when the
/// bytes are valid UTF-8, CP437 otherwise) and returns a `Result`. It can't
/// fail in 9.2, but if it ever does, report an unreadable archive rather than
/// skip an entry that might be the one we want.
fn zip_entry_name(
    entry: &zip::read::ZipFile<'_, BufReader<fs::File>>,
    archive: &Path,
) -> Result<String> {
    entry
        .name()
        .map(std::borrow::Cow::into_owned)
        .map_err(|source| RabbitError::ArchiveRead {
            archive: archive.to_path_buf(),
            message: source.to_string(),
        })
}

/// Extract every file whose immediate parent directory is `bin` (case
/// insensitive) into `extract_dir`, flattening the layout. Used by the
/// FFmpeg pipeline: BtbN's archives lay the runtime DLLs out under a
/// `ffmpeg-<tag>-<arch>-gpl-shared-<ver>/bin/` prefix, and we want all
/// of them dropped into `<resource>/UserPlugins/` regardless of which
/// specific FFmpeg sublibraries the user's plugins depend on. Returns
/// the per-file extraction records sorted by base name. Errors when the
/// archive contains no `bin/<file>` entries — the user fed us something
/// that doesn't match the expected BtbN layout.
pub fn extract_bin_directory_from_archive(
    archive_path: &Path,
    spec: &PackageSpec,
    extract_dir: &Path,
) -> Result<Vec<ExtractedUserPlugin>> {
    let file = fs::File::open(archive_path).with_path(archive_path)?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|source| RabbitError::ArchiveRead {
            archive: archive_path.to_path_buf(),
            message: source.to_string(),
        })?;

    fs::create_dir_all(extract_dir).with_path(extract_dir)?;
    let mut extracted = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|source| RabbitError::ArchiveRead {
                archive: archive_path.to_path_buf(),
                message: source.to_string(),
            })?;
        if !entry.is_file() {
            continue;
        }
        let entry_name = zip_entry_name(&entry, archive_path)?;
        let entry_path = Path::new(&entry_name);
        let parent_is_bin = entry_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("bin"));
        if !parent_is_bin {
            continue;
        }
        let Some(basename) = entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        let target = extract_dir.join(&basename);
        if target.exists() {
            fs::remove_file(&target).with_path(&target)?;
        }
        let mut output = fs::File::create(&target).with_path(&target)?;
        std::io::copy(&mut entry, &mut output).with_path(&target)?;
        output.flush().with_path(&target)?;
        extracted.push(ExtractedUserPlugin {
            source_archive: archive_path.to_path_buf(),
            entry_name,
            extracted_path: target,
            file_name: basename,
        });
    }
    extracted.sort_by(|a, b| a.file_name.cmp(&b.file_name));

    if extracted.is_empty() {
        return Err(RabbitError::ArchiveMissingExtensionBinary {
            archive: archive_path.to_path_buf(),
            package_id: spec.id.clone(),
        });
    }

    Ok(extracted)
}

/// 7z twin of [`extract_bin_directory_from_archive`]. Used by the
/// FFmpeg pipeline because its upstream, Gyan.dev, ships the shared
/// variant only as `.7z`. Walks every entry in the archive, picks the
/// ones whose immediate parent directory is `bin/` (case insensitive),
/// and extracts them flat into `extract_dir`. Errors when the archive
/// contains no `bin/<file>` entries.
pub fn extract_bin_directory_from_seven_zip_archive(
    archive_path: &Path,
    spec: &PackageSpec,
    extract_dir: &Path,
) -> Result<Vec<ExtractedUserPlugin>> {
    fs::create_dir_all(extract_dir).with_path(extract_dir)?;

    let extract_dir_owned = extract_dir.to_path_buf();
    let archive_path_owned = archive_path.to_path_buf();
    let extracted: std::cell::RefCell<Vec<ExtractedUserPlugin>> =
        std::cell::RefCell::new(Vec::new());
    let extract_error: std::cell::RefCell<Option<RabbitError>> = std::cell::RefCell::new(None);

    let result = sevenz_rust2::decompress_file_with_extract_fn(
        archive_path,
        &extract_dir_owned,
        |entry, reader, _dest| {
            // Skip directory entries — we only flatten files. The 7z
            // crate's callback is called for each entry; returning
            // `Ok(true)` continues with the next one without writing
            // anything.
            if entry.is_directory() {
                return Ok(true);
            }
            let entry_name = entry.name().to_string();
            let entry_path = Path::new(&entry_name);
            let parent_is_bin = entry_path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("bin"));
            if !parent_is_bin {
                return Ok(true);
            }
            let Some(basename) = entry_path
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .map(str::to_string)
            else {
                return Ok(true);
            };

            let target = extract_dir_owned.join(&basename);
            // Translate any std::io error into a RabbitError; the 7z
            // crate's callback expects a sevenz_rust2::Error so we
            // stash the typed error in extract_error, keeping the first
            // one, and stop the walk with an error. `Ok(false)` would
            // only end the current solid block and carry on with the
            // next. The outer code surfaces the stashed error after
            // `decompress_file_with_extract_fn` returns.
            macro_rules! bail_with {
                ($error:expr) => {{
                    let mut slot = extract_error.borrow_mut();
                    if slot.is_none() {
                        *slot = Some($error);
                    }
                    return Err(sevenz_rust2::Error::Other(std::borrow::Cow::Borrowed(
                        "extraction stopped after an error",
                    )));
                }};
            }

            if target.exists()
                && let Err(error) = fs::remove_file(&target)
            {
                bail_with!(crate::error::io_error_at(target.clone(), error));
            }
            let mut output = match fs::File::create(&target) {
                Ok(file) => file,
                Err(error) => {
                    bail_with!(crate::error::io_error_at(target.clone(), error));
                }
            };
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let read = match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(error) => {
                        bail_with!(crate::error::io_error_at(target.clone(), error));
                    }
                };
                if let Err(error) = output.write_all(&buffer[..read]) {
                    bail_with!(crate::error::io_error_at(target.clone(), error));
                }
            }
            if let Err(error) = output.flush() {
                bail_with!(crate::error::io_error_at(target.clone(), error));
            }
            extracted.borrow_mut().push(ExtractedUserPlugin {
                source_archive: archive_path_owned.clone(),
                entry_name,
                extracted_path: target,
                file_name: basename,
            });
            Ok(true)
        },
    );

    // Surface a RabbitError stashed by the per-entry callback in
    // preference to the 7z library's error — our IoPathContext shape
    // is what the rest of the install pipeline expects.
    if let Some(error) = extract_error.into_inner() {
        return Err(error);
    }
    if let Err(source) = result {
        return Err(RabbitError::ArchiveRead {
            archive: archive_path.to_path_buf(),
            message: source.to_string(),
        });
    }

    let mut extracted = extracted.into_inner();
    extracted.sort_by(|a, b| a.file_name.cmp(&b.file_name));

    if extracted.is_empty() {
        return Err(RabbitError::ArchiveMissingExtensionBinary {
            archive: archive_path.to_path_buf(),
            package_id: spec.id.clone(),
        });
    }

    Ok(extracted)
}

pub fn extract_all_files_flat(archive_path: &Path, extract_dir: &Path) -> Result<Vec<PathBuf>> {
    let file = fs::File::open(archive_path).with_path(archive_path)?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|source| RabbitError::ArchiveRead {
            archive: archive_path.to_path_buf(),
            message: source.to_string(),
        })?;

    fs::create_dir_all(extract_dir).with_path(extract_dir)?;
    let mut extracted = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|source| RabbitError::ArchiveRead {
                archive: archive_path.to_path_buf(),
                message: source.to_string(),
            })?;
        if !entry.is_file() {
            continue;
        }
        let name = zip_entry_name(&entry, archive_path)?;
        let Some(basename) = Path::new(&name)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        let target = extract_dir.join(&basename);
        if target.exists() {
            fs::remove_file(&target).with_path(&target)?;
        }
        let mut output = fs::File::create(&target).with_path(&target)?;
        std::io::copy(&mut entry, &mut output).with_path(&target)?;
        output.flush().with_path(&target)?;
        extracted.push(target);
    }
    extracted.sort();
    Ok(extracted)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedOsaraAssets {
    pub source_archive: PathBuf,
    pub installed_files: Vec<PathBuf>,
}

const OSARA_INSTALLER_RESOURCES_PREFIX: &str = "OSARAInstaller.app/Contents/Resources/";
const OSARA_DYLIB_BASENAME: &str = "reaper_osara.dylib";
const OSARA_KEYMAP_BASENAME: &str = "OSARA.ReaperKeyMap";
const OSARA_LOCALE_PREFIX: &str = "locale/";
const OSARA_LOCALE_EXTENSION: &str = ".po";

/// The part of `entry_name` after `OSARAInstaller.app/Contents/Resources/`,
/// wherever that folder sits. A snapshot zip has it at the root; a
/// pull-request build is zipped again by GitHub, so it may sit one folder
/// down. The match must start a path segment, so a folder merely *ending* in
/// that name is not mistaken for it.
fn osara_resources_suffix(entry_name: &str) -> Option<&str> {
    let mut search_from = 0;
    while let Some(relative) = entry_name[search_from..].find(OSARA_INSTALLER_RESOURCES_PREFIX) {
        let at = search_from + relative;
        if at == 0 || entry_name.as_bytes()[at - 1] == b'/' {
            return Some(&entry_name[at + OSARA_INSTALLER_RESOURCES_PREFIX.len()..]);
        }
        search_from = at + 1;
    }
    None
}

/// Unwrap an installer that arrives inside a zip - which is how GitHub
/// delivers every workflow artifact, including OSARA's pull-request builds.
/// The zip must hold exactly one `.exe`; it is extracted next to the zip and
/// its path returned, so the rest of the pipeline sees an ordinary installer.
pub fn extract_zipped_installer(zip_path: &Path) -> Result<PathBuf> {
    let file = fs::File::open(zip_path).with_path(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|source| RabbitError::ArchiveRead {
            archive: zip_path.to_path_buf(),
            message: source.to_string(),
        })?;
    let installers: Vec<usize> = (0..archive.len())
        .filter(|index| {
            archive.by_index(*index).is_ok_and(|entry| {
                entry.is_file()
                    && entry
                        .name()
                        .is_ok_and(|name| name.to_ascii_lowercase().ends_with(".exe"))
            })
        })
        .collect();
    let [index] = installers[..] else {
        return Err(RabbitError::ArchiveRead {
            archive: zip_path.to_path_buf(),
            message: format!(
                "expected exactly one installer (.exe) inside, found {}",
                installers.len()
            ),
        });
    };
    let mut entry = archive
        .by_index(index)
        .map_err(|source| RabbitError::ArchiveRead {
            archive: zip_path.to_path_buf(),
            message: source.to_string(),
        })?;
    let entry_name = zip_entry_name(&entry, zip_path)?;
    // Only the file name: never trust a path from inside an archive.
    let name = Path::new(&entry_name)
        .file_name()
        .map(|name| name.to_os_string())
        .ok_or_else(|| RabbitError::ArchiveRead {
            archive: zip_path.to_path_buf(),
            message: "installer entry has no file name".to_string(),
        })?;
    let folder = zip_path.with_extension("unzipped");
    fs::create_dir_all(&folder).with_path(&folder)?;
    let target = folder.join(name);
    let mut output = fs::File::create(&target).with_path(&target)?;
    std::io::copy(&mut entry, &mut output).with_path(&target)?;
    output.flush().with_path(&target)?;
    Ok(target)
}

pub fn extract_osara_macos_assets(
    archive_path: &Path,
    resource_path: &Path,
) -> Result<ExtractedOsaraAssets> {
    let file = fs::File::open(archive_path).with_path(archive_path)?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|source| RabbitError::ArchiveRead {
            archive: archive_path.to_path_buf(),
            message: source.to_string(),
        })?;

    let user_plugins = resource_path.join("UserPlugins");
    let key_maps = resource_path.join("KeyMaps");
    let osara_locale = resource_path.join("osara").join("locale");

    let mut installed_files = Vec::new();
    let mut found_dylib = false;
    let mut found_keymap = false;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|source| RabbitError::ArchiveRead {
                archive: archive_path.to_path_buf(),
                message: source.to_string(),
            })?;
        if !entry.is_file() {
            continue;
        }
        let entry_name = zip_entry_name(&entry, archive_path)?;
        let Some(suffix) = osara_resources_suffix(&entry_name) else {
            continue;
        };

        let target = if suffix == OSARA_DYLIB_BASENAME {
            found_dylib = true;
            user_plugins.join(OSARA_DYLIB_BASENAME)
        } else if suffix == OSARA_KEYMAP_BASENAME {
            found_keymap = true;
            key_maps.join(OSARA_KEYMAP_BASENAME)
        } else if let Some(locale_suffix) = suffix.strip_prefix(OSARA_LOCALE_PREFIX) {
            if !locale_suffix.ends_with(OSARA_LOCALE_EXTENSION) || locale_suffix.contains('/') {
                continue;
            }
            osara_locale.join(locale_suffix)
        } else {
            continue;
        };

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_path(parent)?;
        }
        let mut output = fs::File::create(&target).with_path(&target)?;
        std::io::copy(&mut entry, &mut output).with_path(&target)?;
        output.flush().with_path(&target)?;
        installed_files.push(target);
    }

    if !found_dylib || !found_keymap {
        return Err(RabbitError::OsaraArchiveMissingAssets {
            archive: archive_path.to_path_buf(),
        });
    }

    installed_files.sort();
    Ok(ExtractedOsaraAssets {
        source_archive: archive_path.to_path_buf(),
        installed_files,
    })
}

/// Write a zip at `path` holding `entries`, each a name and what the file
/// holds, stored as they are. A name ending in `/` is a folder. For tests.
#[cfg(test)]
pub(crate) fn write_test_zip(path: &Path, entries: &[(&str, &[u8])]) {
    use zip::write::SimpleFileOptions;

    let file = fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, contents) in entries {
        if name.ends_with('/') {
            writer.add_directory(*name, options).unwrap();
        } else {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
    }
    writer.finish().unwrap();
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    use std::path::PathBuf;

    use super::{
        extract_bin_directory_from_archive, extract_bin_directory_from_seven_zip_archive,
        extract_osara_macos_assets, extract_plugin_bundle_from_archive,
        extract_user_plugin_from_archive, extract_zipped_installer, osara_resources_suffix,
        write_test_zip,
    };
    use crate::error::RabbitError;
    use crate::model::Platform;
    use crate::package::{
        PACKAGE_FFMPEG, PACKAGE_LJUNO_116, PACKAGE_REAKONTROL, PluginBundleSpec,
        package_specs_by_id,
    };

    /// A package that identifies its file by suffix alone (an empty prefix
    /// list) must match any basename with that suffix — REAPER language
    /// packs are named after their language and share no common prefix.
    /// The previous `prefix_match && suffix_match` made an empty list match
    /// nothing at all.
    #[test]
    fn empty_prefix_list_matches_any_name_with_the_suffix() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("langpack.zip");
        write_test_archive(
            &archive_path,
            &[
                ("readme.txt", b"docs"),
                ("es_ES.ReaperLangPack", b"langpack-bytes"),
            ],
        );

        let mut spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_REAKONTROL)
            .unwrap();
        spec.user_plugin_prefixes = Vec::new();
        spec.user_plugin_suffixes = vec![".ReaperLangPack".to_string()];

        let extract_dir = dir.path().join("extract");
        let extracted =
            extract_user_plugin_from_archive(&archive_path, &spec, &extract_dir).unwrap();

        assert_eq!(extracted.file_name, "es_ES.ReaperLangPack");
        assert_eq!(
            std::fs::read(&extracted.extracted_path).unwrap(),
            b"langpack-bytes"
        );
    }

    #[test]
    fn extracts_matching_user_plugin_binary_from_zip() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("reaKontrol_windows_test.zip");
        write_test_archive(
            &archive_path,
            &[
                ("README.md", b"docs"),
                ("reaper_kontrol.dll", b"plugin-bytes"),
            ],
        );

        let spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_REAKONTROL)
            .unwrap();
        let extract_dir = dir.path().join("extract");
        let extracted =
            extract_user_plugin_from_archive(&archive_path, &spec, &extract_dir).unwrap();

        assert_eq!(extracted.file_name, "reaper_kontrol.dll");
        assert_eq!(
            std::fs::read(&extracted.extracted_path).unwrap(),
            b"plugin-bytes"
        );
    }

    #[test]
    fn errors_when_archive_lacks_user_plugin_binary() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("empty.zip");
        write_test_archive(&archive_path, &[("README.md", b"docs")]);

        let spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_REAKONTROL)
            .unwrap();
        let error = extract_user_plugin_from_archive(&archive_path, &spec, dir.path()).unwrap_err();

        assert!(matches!(
            error,
            RabbitError::ArchiveMissingExtensionBinary { .. }
        ));
    }

    #[test]
    fn finds_binary_inside_nested_directory() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("nested.zip");
        write_test_archive(
            &archive_path,
            &[("subdir/reaper_kontrol.dylib", b"mac-plugin")],
        );

        let spec = package_specs_by_id(Platform::MacOs)
            .remove(PACKAGE_REAKONTROL)
            .unwrap();
        let extracted = extract_user_plugin_from_archive(&archive_path, &spec, dir.path()).unwrap();

        assert_eq!(extracted.file_name, "reaper_kontrol.dylib");
        assert_eq!(
            std::fs::read(&extracted.extracted_path).unwrap(),
            b"mac-plugin"
        );
    }

    #[test]
    fn extracts_osara_macos_assets_into_resource_path() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("osara_test.zip");
        write_test_archive(
            &archive_path,
            &[
                ("OSARAInstaller.app/Contents/MacOS/applet", b"applet-binary"),
                (
                    "OSARAInstaller.app/Contents/Resources/reaper_osara.dylib",
                    b"osara-plugin",
                ),
                (
                    "OSARAInstaller.app/Contents/Resources/OSARA.ReaperKeyMap",
                    b"keymap-content",
                ),
                (
                    "OSARAInstaller.app/Contents/Resources/locale/de_DE.po",
                    b"de-locale",
                ),
                (
                    "OSARAInstaller.app/Contents/Resources/locale/fr_FR.po",
                    b"fr-locale",
                ),
                (
                    "OSARAInstaller.app/Contents/Resources/copying.txt",
                    b"license-text",
                ),
            ],
        );
        let resource_path = dir.path().join("REAPER");

        let report = extract_osara_macos_assets(&archive_path, &resource_path).unwrap();

        let dylib = resource_path.join("UserPlugins").join("reaper_osara.dylib");
        let keymap = resource_path.join("KeyMaps").join("OSARA.ReaperKeyMap");
        let de_locale = resource_path.join("osara").join("locale").join("de_DE.po");
        let fr_locale = resource_path.join("osara").join("locale").join("fr_FR.po");
        assert_eq!(std::fs::read(&dylib).unwrap(), b"osara-plugin");
        assert_eq!(std::fs::read(&keymap).unwrap(), b"keymap-content");
        assert_eq!(std::fs::read(&de_locale).unwrap(), b"de-locale");
        assert_eq!(std::fs::read(&fr_locale).unwrap(), b"fr-locale");
        assert!(report.installed_files.contains(&dylib));
        assert!(report.installed_files.contains(&keymap));
        assert!(report.installed_files.contains(&de_locale));
        assert!(report.installed_files.contains(&fr_locale));
        assert!(!resource_path.join("copying.txt").exists());
        assert!(!resource_path.join("Contents").exists());
    }

    #[test]
    fn finds_osara_resources_one_folder_down_but_not_in_a_lookalike_folder() {
        // A pull-request build is zipped again by GitHub, so the installer
        // folder may sit below the zip's root.
        assert_eq!(
            osara_resources_suffix("OSARAInstaller.app/Contents/Resources/reaper_osara.dylib"),
            Some("reaper_osara.dylib")
        );
        assert_eq!(
            osara_resources_suffix("mac/OSARAInstaller.app/Contents/Resources/OSARA.ReaperKeyMap"),
            Some("OSARA.ReaperKeyMap")
        );
        assert_eq!(
            osara_resources_suffix("NotOSARAInstaller.app/Contents/Resources/reaper_osara.dylib"),
            None,
            "the match must start a path segment"
        );
    }

    #[test]
    fn extracts_osara_assets_from_a_nested_pull_request_zip() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("osara_mac_pr1454-534,240c4663.zip");
        write_test_archive(
            &archive_path,
            &[
                (
                    "mac/OSARAInstaller.app/Contents/Resources/reaper_osara.dylib",
                    b"pr-plugin",
                ),
                (
                    "mac/OSARAInstaller.app/Contents/Resources/OSARA.ReaperKeyMap",
                    b"pr-keymap",
                ),
            ],
        );
        let resource_path = dir.path().join("REAPER");
        extract_osara_macos_assets(&archive_path, &resource_path).unwrap();
        assert_eq!(
            std::fs::read(resource_path.join("UserPlugins").join("reaper_osara.dylib")).unwrap(),
            b"pr-plugin"
        );
    }

    #[test]
    fn unwraps_a_zipped_installer_and_ignores_paths_inside_the_zip() {
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("osara_windows_pr1454-534,240c4663.zip");
        // A path that tries to climb out of the cache: only the file name may
        // be used.
        write_test_archive(
            &zip_path,
            &[("../../escape/osara_pr1454-534,240c4663.exe", b"nsis")],
        );
        let installer = extract_zipped_installer(&zip_path).unwrap();
        assert_eq!(
            installer,
            dir.path()
                .join("osara_windows_pr1454-534,240c4663.unzipped")
                .join("osara_pr1454-534,240c4663.exe")
        );
        assert_eq!(std::fs::read(&installer).unwrap(), b"nsis");
        assert!(!dir.path().join("escape").exists());
    }

    #[test]
    fn a_zip_without_exactly_one_installer_is_refused() {
        let dir = tempdir().unwrap();
        let none = dir.path().join("none.zip");
        write_test_archive(&none, &[("readme.txt", b"no installer")]);
        assert!(extract_zipped_installer(&none).is_err());
        let two = dir.path().join("two.zip");
        write_test_archive(&two, &[("a.exe", b"a"), ("b.exe", b"b")]);
        assert!(extract_zipped_installer(&two).is_err());
    }

    #[test]
    fn errors_when_osara_archive_is_missing_dylib_or_keymap() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("partial.zip");
        write_test_archive(
            &archive_path,
            &[(
                "OSARAInstaller.app/Contents/Resources/locale/en_US.po",
                b"en-locale",
            )],
        );
        let resource_path = dir.path().join("REAPER");

        let error = extract_osara_macos_assets(&archive_path, &resource_path).unwrap_err();
        assert!(matches!(
            error,
            RabbitError::OsaraArchiveMissingAssets { .. }
        ));
    }

    #[test]
    fn extract_bin_directory_pulls_only_bin_children_flat_into_extract_dir() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("ffmpeg-test.zip");
        write_test_archive(
            &archive_path,
            &[
                ("ffmpeg-n8.0/bin/avformat-62.dll", b"avformat"),
                ("ffmpeg-n8.0/bin/avcodec-62.dll", b"avcodec"),
                ("ffmpeg-n8.0/bin/ffmpeg.exe", b"ffmpeg-exe"),
                ("ffmpeg-n8.0/bin/sub/nested.dll", b"nested"),
                ("ffmpeg-n8.0/include/libavformat/avformat.h", b"header"),
                ("ffmpeg-n8.0/lib/avformat.lib", b"static-lib"),
                ("ffmpeg-n8.0/doc/README.md", b"docs"),
            ],
        );

        let spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_FFMPEG)
            .unwrap();
        let extract_dir = dir.path().join("extract");
        let extracted =
            extract_bin_directory_from_archive(&archive_path, &spec, &extract_dir).unwrap();

        let names: Vec<_> = extracted
            .iter()
            .map(|item| item.file_name.clone())
            .collect();
        assert_eq!(
            names,
            vec![
                "avcodec-62.dll".to_string(),
                "avformat-62.dll".to_string(),
                "ffmpeg.exe".to_string(),
            ]
        );
        assert_eq!(
            std::fs::read(extract_dir.join("avformat-62.dll")).unwrap(),
            b"avformat"
        );
        assert!(!extract_dir.join("avformat.h").exists());
        assert!(!extract_dir.join("nested.dll").exists());
    }

    #[test]
    fn extract_bin_directory_errors_when_archive_lacks_bin_entries() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("no-bin.zip");
        write_test_archive(
            &archive_path,
            &[("ffmpeg-n8.0/include/libavformat/avformat.h", b"header")],
        );

        let spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_FFMPEG)
            .unwrap();
        let error =
            extract_bin_directory_from_archive(&archive_path, &spec, dir.path()).unwrap_err();
        assert!(matches!(
            error,
            RabbitError::ArchiveMissingExtensionBinary { .. }
        ));
    }

    /// Real downloads compress their entries; the other tests store them.
    #[test]
    fn extracts_a_deflated_entry() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("reaKontrol_windows_test.zip");
        let payload: Vec<u8> = (0..64 * 1024).map(|i| (i % 251) as u8).collect();
        let file = std::fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer.start_file("README.md", options).unwrap();
        writer.write_all(b"docs").unwrap();
        writer.start_file("reaper_kontrol.dll", options).unwrap();
        writer.write_all(&payload).unwrap();
        writer.finish().unwrap();
        assert!(std::fs::metadata(&archive_path).unwrap().len() < payload.len() as u64);

        let spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_REAKONTROL)
            .unwrap();
        let extracted =
            extract_user_plugin_from_archive(&archive_path, &spec, &dir.path().join("extract"))
                .unwrap();

        assert_eq!(std::fs::read(&extracted.extracted_path).unwrap(), payload);
    }

    /// A name without the zip's UTF-8 flag is CP437, as Windows Explorer and
    /// older tools write it: 0xA4 is ñ.
    #[test]
    fn decodes_a_cp437_entry_name() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("langpack.zip");
        write_test_archive(
            &archive_path,
            &[("EspaXol.ReaperLangPack", b"langpack-bytes")],
        );
        // The writer sets no UTF-8 flag for an ASCII name; swap the
        // placeholder for the CP437 byte in both headers.
        let mut bytes = std::fs::read(&archive_path).unwrap();
        let mut patched = 0;
        for at in 0..bytes.len() - 7 {
            if &bytes[at..at + 7] == b"EspaXol" {
                bytes[at + 4] = 0xA4;
                patched += 1;
            }
        }
        assert_eq!(patched, 2);
        std::fs::write(&archive_path, bytes).unwrap();

        let mut spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_REAKONTROL)
            .unwrap();
        spec.user_plugin_prefixes = Vec::new();
        spec.user_plugin_suffixes = vec![".ReaperLangPack".to_string()];
        let extracted =
            extract_user_plugin_from_archive(&archive_path, &spec, &dir.path().join("extract"))
                .unwrap();

        assert_eq!(extracted.file_name, "Espa\u{f1}ol.ReaperLangPack");
    }

    /// FFmpeg's .7z keeps everything in one solid block, so entries RABBIT
    /// skips sit before and between the ones it extracts.
    #[test]
    fn seven_zip_bin_extraction_skips_unread_entries_in_a_solid_block() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("ffmpeg-test.7z");
        write_test_seven_zip(
            &archive_path,
            &[
                ("ffmpeg-n8.0/LICENSE.txt", b"license-text"),
                ("ffmpeg-n8.0/bin/avcodec-62.dll", b"avcodec"),
                ("ffmpeg-n8.0/doc/README.md", b"docs"),
                ("ffmpeg-n8.0/bin/avformat-62.dll", b"avformat"),
            ],
        );

        let spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_FFMPEG)
            .unwrap();
        let extract_dir = dir.path().join("extract");
        let extracted =
            extract_bin_directory_from_seven_zip_archive(&archive_path, &spec, &extract_dir)
                .unwrap();

        let names: Vec<_> = extracted
            .iter()
            .map(|item| item.file_name.as_str())
            .collect();
        assert_eq!(names, ["avcodec-62.dll", "avformat-62.dll"]);
        assert_eq!(
            std::fs::read(extract_dir.join("avcodec-62.dll")).unwrap(),
            b"avcodec"
        );
        assert_eq!(
            std::fs::read(extract_dir.join("avformat-62.dll")).unwrap(),
            b"avformat"
        );
        assert!(!extract_dir.join("LICENSE.txt").exists());
    }

    #[test]
    fn seven_zip_bin_extraction_errors_when_archive_lacks_bin_entries() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("no-bin.7z");
        write_test_seven_zip(
            &archive_path,
            &[("ffmpeg-n8.0/include/libavformat/avformat.h", b"header")],
        );

        let spec = package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_FFMPEG)
            .unwrap();
        let error = extract_bin_directory_from_seven_zip_archive(&archive_path, &spec, dir.path())
            .unwrap_err();
        assert!(matches!(
            error,
            RabbitError::ArchiveMissingExtensionBinary { .. }
        ));
    }

    /// LJuno-116's bundle as its release zip holds it.
    fn ljuno_bundle() -> PluginBundleSpec {
        package_specs_by_id(Platform::Windows)
            .remove(PACKAGE_LJUNO_116)
            .unwrap()
            .plugin_bundle
            .unwrap()
    }

    /// The bundle folder comes out whole, every file at its path inside it
    /// and the binary last; files outside it, and another bundle beside it,
    /// stay in the archive. The folder is found whatever its case.
    #[test]
    fn extracts_a_plugin_bundle_keeping_its_tree_and_the_binary_last() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("LJuno-116.0.99.66.vst3.zip");
        write_test_zip(
            &archive_path,
            &[
                ("ljuno-116.vst3/", b""),
                (
                    "ljuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3",
                    b"binary",
                ),
                ("ljuno-116.vst3/desktop.ini", b"ini"),
                ("ljuno-116.vst3/Contents/Resources/moduleinfo.json", b"{}"),
                ("ljuno-116.vst3/Plugin.ico", b"icon"),
                ("README.txt", b"docs"),
                ("Other.vst3/Contents/x86_64-win/Other.vst3", b"other"),
            ],
        );
        let extract_dir = dir.path().join("extract");

        let files =
            extract_plugin_bundle_from_archive(&archive_path, &ljuno_bundle(), &extract_dir)
                .unwrap();

        let relative = |text: &str| text.split('/').collect::<PathBuf>();
        let found: Vec<(PathBuf, Vec<u8>)> = files
            .iter()
            .map(|file| {
                assert_eq!(file.extracted_path, extract_dir.join(&file.relative));
                (
                    file.relative.clone(),
                    std::fs::read(&file.extracted_path).unwrap(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                (
                    relative("Contents/Resources/moduleinfo.json"),
                    b"{}".to_vec()
                ),
                (relative("Plugin.ico"), b"icon".to_vec()),
                (relative("desktop.ini"), b"ini".to_vec()),
                (
                    relative("Contents/x86_64-win/LJuno-116.vst3"),
                    b"binary".to_vec()
                ),
            ]
        );
        assert!(!extract_dir.join("README.txt").exists());
        assert!(!extract_dir.join("Other.vst3").exists());
        assert!(!dir.path().join("Other.vst3").exists());
    }

    /// Any entry that could land outside the folder it is extracted to,
    /// that Windows would open under another name, or that takes the name
    /// of an install's or a restore's temporary copy, refuses the whole
    /// archive, wherever in the archive it is, before anything is written.
    #[test]
    fn refuses_unsafe_entries_before_writing_anything() {
        let unsafe_names = [
            "../evil.dll",
            "LJuno-116.vst3/../../evil.dll",
            "LJuno-116.vst3/Contents/../../../evil.dll",
            "LJuno-116.vst3/./desktop.ini",
            "/absolute/evil.dll",
            "\\absolute\\evil.dll",
            "\\\\server\\share\\evil.dll",
            "C:/Windows/evil.dll",
            "C:evil.dll",
            "LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3:stream",
            "LJuno-116.vst3//desktop.ini",
            "LJuno-116.vst3/CON",
            "LJuno-116.vst3/nul.txt",
            "LJuno-116.vst3/trailing.",
            "LJuno-116.vst3/trailing ",
            "LJuno-116.vst3/a*b",
            "LJuno-116.vst3/a?b",
            "LJuno-116.vst3/a|b",
            "LJuno-116.vst3/a\u{1}b",
            "LJuno-116.vst3\\..\\evil.dll",
            "LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3.rabbit-tmp",
            "LJuno-116.vst3/desktop.ini.RABBIT-RESTORE-TMP",
        ];
        for unsafe_name in unsafe_names {
            for unsafe_first in [true, false] {
                let dir = tempdir().unwrap();
                let archive_path = dir.path().join("LJuno-116.vst3.zip");
                let good: &[(&str, &[u8])] = &[
                    ("LJuno-116.vst3/desktop.ini", b"ini"),
                    (
                        "LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3",
                        b"binary",
                    ),
                ];
                let bad: &[(&str, &[u8])] = &[(unsafe_name, b"evil")];
                let entries: Vec<(&str, &[u8])> = if unsafe_first {
                    bad.iter().chain(good).copied().collect()
                } else {
                    good.iter().chain(bad).copied().collect()
                };
                write_test_zip(&archive_path, &entries);
                let extract_dir = dir.path().join("extract");

                let error = extract_plugin_bundle_from_archive(
                    &archive_path,
                    &ljuno_bundle(),
                    &extract_dir,
                )
                .unwrap_err();

                assert!(
                    matches!(&error, RabbitError::ArchiveEntryRefused { entry, .. } if entry == unsafe_name),
                    "{unsafe_name:?}: {error}"
                );
                assert!(!extract_dir.exists(), "{unsafe_name:?} wrote something");
                assert!(!dir.path().join("evil.dll").exists());
            }
        }
    }

    /// A symbolic link in the archive, in the bundle or beside it, refuses
    /// the archive before anything is written: extracting it could point a
    /// later file anywhere.
    #[test]
    fn refuses_a_symbolic_link_entry_before_writing_anything() {
        for link_name in [
            "LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3",
            "LJuno-116.vst3/Contents/Resources",
            "elsewhere",
        ] {
            let dir = tempdir().unwrap();
            let archive_path = dir.path().join("LJuno-116.vst3.zip");
            {
                let file = std::fs::File::create(&archive_path).unwrap();
                let mut writer = zip::ZipWriter::new(file);
                let options =
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
                writer
                    .start_file("LJuno-116.vst3/desktop.ini", options)
                    .unwrap();
                writer.write_all(b"ini").unwrap();
                writer
                    .add_symlink(link_name, "../../../../evil", options)
                    .unwrap();
                writer.finish().unwrap();
            }
            let extract_dir = dir.path().join("extract");

            let error =
                extract_plugin_bundle_from_archive(&archive_path, &ljuno_bundle(), &extract_dir)
                    .unwrap_err();

            assert!(
                matches!(&error, RabbitError::ArchiveEntryRefused { entry, reason, .. }
                    if entry == link_name && reason.contains("symbolic link")),
                "{link_name}: {error}"
            );
            assert!(!extract_dir.exists());
        }
    }

    /// An entry that a zip made on Windows marks as a symbolic link or
    /// junction by the reparse-point bit of its attributes refuses the
    /// archive too, though its Unix mode calls it a plain file.
    #[test]
    fn refuses_an_entry_marked_as_a_reparse_point_before_writing_anything() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("LJuno-116.vst3.zip");
        let marked = "LJuno-116.vst3/Contents/Resources";
        write_test_zip(
            &archive_path,
            &[
                ("LJuno-116.vst3/desktop.ini", b"ini"),
                (marked, b"../../../../evil"),
                (
                    "LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3",
                    b"binary",
                ),
            ],
        );
        // Without the bit, the same archive extracts.
        extract_plugin_bundle_from_archive(
            &archive_path,
            &ljuno_bundle(),
            &dir.path().join("plain"),
        )
        .unwrap();

        // The entry's header in the central directory: its signature, the
        // length of its name at 28 and the name at 46. Its external
        // attributes, little-endian, are at 38; the reparse-point bit,
        // 0x400, is in their second byte.
        let mut bytes = std::fs::read(&archive_path).unwrap();
        let header = (0..bytes.len() - 46)
            .find(|&at| {
                let length = usize::from(u16::from_le_bytes([bytes[at + 28], bytes[at + 29]]));
                bytes[at..].starts_with(&[0x50, 0x4b, 0x01, 0x02])
                    && bytes.get(at + 46..at + 46 + length) == Some(marked.as_bytes())
            })
            .expect("the entry is in the central directory");
        bytes[header + 39] |= 0x04;
        std::fs::write(&archive_path, &bytes).unwrap();
        let extract_dir = dir.path().join("extract");

        let error =
            extract_plugin_bundle_from_archive(&archive_path, &ljuno_bundle(), &extract_dir)
                .unwrap_err();

        assert!(
            matches!(&error, RabbitError::ArchiveEntryRefused { entry, reason, .. }
                if entry == marked && reason.contains("symbolic link")),
            "{error}"
        );
        assert!(!extract_dir.exists());
    }

    /// Windows reads names ignoring case, so two files whose paths differ
    /// only in case, or a file where another one's folder is, would land on
    /// one place: the archive is refused before anything is written.
    #[test]
    fn refuses_files_that_would_land_on_one_place() {
        let cases: [&[(&str, &[u8])]; 3] = [
            &[
                ("LJuno-116.vst3/desktop.ini", b"one"),
                ("LJuno-116.vst3/Desktop.INI", b"two"),
            ],
            &[
                ("LJuno-116.vst3/Contents", b"a file"),
                (
                    "LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3",
                    b"binary",
                ),
            ],
            &[
                ("LJuno-116.vst3/Contents/Resources/", b""),
                ("LJuno-116.vst3/contents/resources", b"a file"),
            ],
        ];
        for entries in cases {
            let dir = tempdir().unwrap();
            let archive_path = dir.path().join("LJuno-116.vst3.zip");
            let mut entries = entries.to_vec();
            entries.push((
                "LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3",
                b"binary",
            ));
            entries.dedup_by_key(|(name, _)| *name);
            write_test_zip(&archive_path, &entries);
            let extract_dir = dir.path().join("extract");

            let error =
                extract_plugin_bundle_from_archive(&archive_path, &ljuno_bundle(), &extract_dir)
                    .unwrap_err();

            assert!(
                matches!(error, RabbitError::ArchiveEntryRefused { .. }),
                "{entries:?}: {error}"
            );
            assert!(!extract_dir.exists());
        }
    }

    /// A bundle without its binary, a file where the bundle's folder
    /// should be, and an archive without the bundle install nothing.
    #[test]
    fn a_bundle_without_its_binary_installs_nothing() {
        let cases: [&[(&str, &[u8])]; 3] = [
            &[("LJuno-116.vst3/desktop.ini", b"ini")],
            &[("Other.vst3/Contents/x86_64-win/LJuno-116.vst3", b"binary")],
            &[("LJuno-116.vst3/Contents/x86_64-win/LJuno-116.vst3/", b"")],
        ];
        for entries in cases {
            let dir = tempdir().unwrap();
            let archive_path = dir.path().join("LJuno-116.vst3.zip");
            write_test_zip(&archive_path, entries);
            let extract_dir = dir.path().join("extract");

            let error =
                extract_plugin_bundle_from_archive(&archive_path, &ljuno_bundle(), &extract_dir)
                    .unwrap_err();

            assert!(
                matches!(error, RabbitError::ArchiveMissingBundleBinary { .. }),
                "{entries:?}: {error}"
            );
            assert!(!extract_dir.exists());
        }

        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("LJuno-116.vst3.zip");
        write_test_zip(&archive_path, &[("LJuno-116.vst3", b"a flat plug-in")]);
        let error = extract_plugin_bundle_from_archive(
            &archive_path,
            &ljuno_bundle(),
            &dir.path().join("extract"),
        )
        .unwrap_err();
        assert!(
            matches!(error, RabbitError::ArchiveEntryRefused { .. }),
            "{error}"
        );
    }

    /// One solid LZMA2 block holding every entry.
    fn write_test_seven_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let mut writer = sevenz_rust2::ArchiveWriter::create(path).unwrap();
        writer
            .push_archive_entries(
                entries
                    .iter()
                    .map(|(name, _)| sevenz_rust2::ArchiveEntry::new_file(name))
                    .collect(),
                entries
                    .iter()
                    .map(|(_, data)| sevenz_rust2::SourceReader::from(*data))
                    .collect(),
            )
            .unwrap();
        writer.finish().unwrap();
    }

    fn write_test_archive(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, contents) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }
}
