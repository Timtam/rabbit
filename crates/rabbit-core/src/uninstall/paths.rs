//! Where an uninstall plan's paths point, and the guard every removal passes.
//!
//! Manifest paths start with a token such as `{resource}`. [`UninstallRoots`]
//! holds what each token stands for on this computer and for one REAPER
//! target, and [`resolve_path`] turns a manifest path into real paths. The
//! guard, [`check_removal`], then decides whether RABBIT may remove what a
//! path names. It runs while the plan is built, and the engine runs it again
//! right before each move, because files can change in between.
//!
//! The guard compares two ways. Containment ("is this inside the folder it
//! was declared under?") uses canonical paths and compares them exactly, so
//! a resource folder reached through a junction or a Documents folder that
//! OneDrive redirected still works, and nothing can step out of its folder
//! through a link. The refusals ("is this RABBIT's own folder, or a whole
//! folder root?") compare ignoring case, the way both platforms' default
//! file systems do, because there a looser match can only refuse more.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};

use serde::{Deserialize, Serialize};

use crate::model::Platform;
use crate::package::uninstall::{
    MACOS_ABSOLUTE_PREFIXES, RABBIT_FOLDER, UninstallPath, UninstallPathRoot, UninstallPathToken,
    UninstallTokenKind, wildcard_matches_ignoring_case,
};

/// What each path token stands for, for one REAPER target on this computer.
/// `None` (or an empty list) means the token can't be resolved here, for
/// example a Windows folder on macOS.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UninstallRoots {
    /// The target's resource folder, absolute.
    pub resource: PathBuf,
    /// REAPER's app: `reaper.exe` on Windows, the `.app` bundle on macOS.
    pub app_path: Option<PathBuf>,
    /// The folder REAPER's app sits in.
    pub app_dir: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub localappdata: Option<PathBuf>,
    pub clap_dir: Option<PathBuf>,
    pub common_program_files: Option<PathBuf>,
    pub program_files: Vec<PathBuf>,
    pub program_data: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub documents: Option<PathBuf>,
}

impl UninstallRoots {
    /// Roots with only the resource folder known. Every other token stays
    /// unresolved until the caller fills it in.
    pub fn new(resource: &Path) -> Self {
        Self {
            resource: absolute(resource),
            ..Self::default()
        }
    }

    /// The roots for a target at `resource` with REAPER's app at
    /// `app_path`, read from this computer. Windows folders resolve only on
    /// Windows and `{home}` only on macOS, as the manifest's validation
    /// expects: Git Bash sets `HOME` on Windows, but no plan means it there.
    ///
    /// `{documents}` stays unresolved for now. Windows keeps it as a known
    /// folder that an environment variable can't name, and no package
    /// RABBIT removes so far uses it.
    pub fn for_target(platform: Platform, resource: &Path, app_path: Option<&Path>) -> Self {
        let windows = platform == Platform::Windows;
        let app_path = app_path.map(absolute);
        Self {
            resource: absolute(resource),
            app_dir: app_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
            app_path,
            appdata: windows.then(rabbit_platform::user_appdata_dir).flatten(),
            localappdata: windows
                .then(rabbit_platform::user_local_appdata_dir)
                .flatten(),
            clap_dir: windows.then(rabbit_platform::windows_clap_dir).flatten(),
            common_program_files: windows
                .then(rabbit_platform::windows_common_program_files_dir)
                .flatten(),
            program_files: if windows {
                rabbit_platform::windows_program_files_dirs()
            } else {
                Vec::new()
            },
            program_data: windows
                .then(rabbit_platform::windows_program_data_dir)
                .flatten(),
            home: (!windows).then(rabbit_platform::user_home_dir).flatten(),
            documents: None,
        }
    }

    /// Every path `token` stands for. Only `{program_files}` can stand for
    /// more than one (the 64-bit and the 32-bit folder).
    pub fn resolve(&self, token: UninstallPathToken) -> Vec<PathBuf> {
        let one = |path: &Option<PathBuf>| path.iter().cloned().collect();
        match token {
            UninstallPathToken::Resource => vec![self.resource.clone()],
            UninstallPathToken::AppDir => one(&self.app_dir),
            UninstallPathToken::AppPath => one(&self.app_path),
            UninstallPathToken::Appdata => one(&self.appdata),
            UninstallPathToken::Localappdata => one(&self.localappdata),
            UninstallPathToken::ClapDir => one(&self.clap_dir),
            UninstallPathToken::CommonProgramFiles => one(&self.common_program_files),
            UninstallPathToken::ProgramFiles => self.program_files.clone(),
            UninstallPathToken::ProgramData => one(&self.program_data),
            UninstallPathToken::Home => one(&self.home),
            UninstallPathToken::Documents => one(&self.documents),
        }
    }

    /// Every folder a removal may never be, nor contain: removing one would
    /// take far more than a package with it. `{app_path}` isn't here, because
    /// it names the thing a `trees` removal takes away.
    fn folder_roots(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.resource.as_path())
            .chain(
                [
                    &self.app_dir,
                    &self.appdata,
                    &self.localappdata,
                    &self.clap_dir,
                    &self.common_program_files,
                    &self.program_data,
                    &self.home,
                    &self.documents,
                ]
                .into_iter()
                .filter_map(Option::as_deref),
            )
            .chain(self.program_files.iter().map(PathBuf::as_path))
    }

    /// The folders `token`'s object may sit directly in, for a `trees`
    /// removal. Only `{app_path}` is an object token: REAPER's app lives in
    /// `/Applications`, `~/Applications`, or a portable target's folder.
    fn allowed_object_parents(&self, token: UninstallPathToken) -> Vec<PathBuf> {
        match token {
            UninstallPathToken::AppPath => {
                let mut parents = vec![PathBuf::from("/Applications")];
                parents.extend(self.home.as_ref().map(|home| home.join("Applications")));
                parents.push(self.resource.clone());
                parents
            }
            _ => Vec::new(),
        }
    }
}

/// A manifest path resolved for this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPath {
    pub path: PathBuf,
    /// The checks every removal of this path must pass.
    pub guard: RemovalGuard,
    /// The path is a bare folder root such as `{resource}`, which is never
    /// removed itself.
    pub is_root: bool,
    /// The last component is a `*` pattern; see [`expand_wildcard`].
    pub wildcard: bool,
}

/// Resolve a manifest path. A token that stands for nothing here is
/// refused, because RABBIT can't tell where that path would be; a token
/// that stands for several folders gives one path for each.
pub fn resolve_path(
    path: &UninstallPath,
    roots: &UninstallRoots,
) -> Result<Vec<ResolvedPath>, PathRefusal> {
    // Embedded manifests are validated when they load, so this only fails
    // for a hand-built spec. Refusing is still the safe answer.
    let parsed = path.parse().map_err(|_| PathRefusal::NotPlain)?;
    let wildcard = parsed
        .components
        .last()
        .is_some_and(|last| last.contains('*'));
    match parsed.root {
        UninstallPathRoot::Token(token) => {
            let bases = roots.resolve(token);
            if bases.is_empty() {
                return Err(PathRefusal::UnknownRoot {
                    token: format!("{{{}}}", token.name()),
                });
            }
            Ok(bases
                .into_iter()
                .map(|base| {
                    let full = parsed
                        .components
                        .iter()
                        .fold(base.clone(), |path, component| path.join(component));
                    let guard = match token.kind() {
                        UninstallTokenKind::Object => RemovalGuard {
                            root: None,
                            allowed_parents: roots.allowed_object_parents(token),
                            ..RemovalGuard::default()
                        },
                        UninstallTokenKind::FolderRoot | UninstallTokenKind::ComparisonRoot => {
                            RemovalGuard::under(base)
                        }
                    };
                    ResolvedPath {
                        path: full,
                        guard,
                        is_root: parsed.components.is_empty()
                            && token.kind() != UninstallTokenKind::Object,
                        wildcard,
                    }
                })
                .collect())
        }
        UninstallPathRoot::MacosAbsolute => {
            // Validation only lets such a path sit below one of the known
            // folders, which then serves as its root.
            let prefix = MACOS_ABSOLUTE_PREFIXES
                .iter()
                .find(|prefix| {
                    parsed.components.len() > prefix.len() && parsed.components.starts_with(prefix)
                })
                .ok_or(PathRefusal::OutsideRoot)?;
            let root = prefix
                .iter()
                .fold(PathBuf::from("/"), |path, component| path.join(component));
            Ok(vec![ResolvedPath {
                path: PathBuf::from(path.as_str()),
                guard: RemovalGuard::under(root),
                is_root: false,
                wildcard,
            }])
        }
    }
}

/// The files in a wildcard path's folder whose names match its last
/// component, ignoring case, in name order. Only names are compared; the
/// guard checks each match afterwards. A folder that doesn't exist holds
/// nothing.
pub fn expand_wildcard(resolved: &ResolvedPath) -> io::Result<Vec<PathBuf>> {
    let (Some(folder), Some(pattern)) = (
        resolved.path.parent(),
        resolved.path.file_name().and_then(|name| name.to_str()),
    ) else {
        return Ok(Vec::new());
    };
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut matches = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| wildcard_matches_ignoring_case(pattern, name))
        {
            matches.push(entry.path());
        }
    }
    matches.sort();
    Ok(matches)
}

/// What [`check_removal`] checks for one path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemovalGuard {
    /// The folder the path must stay inside: the folder its token stands for.
    /// `None` for an object token, which names the thing itself.
    pub root: Option<PathBuf>,
    /// For an object token: the folders it may sit directly in.
    pub allowed_parents: Vec<PathBuf>,
    /// For `trees`: the name must end with this, ignoring case.
    pub must_end_with: Option<String>,
    /// For `trees`: the name must contain this, ignoring case.
    pub name_contains: Option<String>,
}

impl RemovalGuard {
    /// A path that must stay inside `root`.
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
            ..Self::default()
        }
    }
}

/// Why the guard refused a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PathRefusal {
    #[error("RABBIT can't find the folder {token} stands for on this computer")]
    UnknownRoot { token: String },
    #[error("it isn't a plain path: it is relative, or uses . or ..")]
    NotPlain,
    #[error("it isn't inside the folder it belongs to")]
    OutsideRoot,
    #[error("it is a whole folder RABBIT never removes, or holds one")]
    FolderRoot,
    #[error("it is a symbolic link or junction, which RABBIT never follows or removes")]
    Link,
    #[error("it is in RABBIT's own folder, which holds receipts, backups and logs")]
    RabbitFolder,
    #[error("its name isn't what the package manifest expects")]
    UnexpectedName,
    #[error("it isn't in a folder it may be removed from")]
    ParentNotAllowed,
    #[error("RABBIT couldn't check it: {message}")]
    Unreadable { message: String },
}

impl PathRefusal {
    fn unreadable(path: &Path, error: &io::Error) -> Self {
        Self::Unreadable {
            message: format!("{}: {error}", path.display()),
        }
    }
}

/// Whether RABBIT may remove `path`, which must exist.
///
/// In order, the path must:
/// - be absolute, with no `.` or `..` anywhere;
/// - sit strictly inside its root, so a folder root is never removed itself;
/// - not be a symbolic link or a junction. Only the last component is
///   tested: links further up are allowed, because the canonical comparison
///   below catches any that lead out of the root. On Windows, Rust reports
///   every name-surrogate reparse point (symbolic links and junctions) as a
///   link, and other reparse points such as OneDrive placeholders as what
///   they stand for;
/// - not be in `{resource}/RABBIT`, however it is reached;
/// - not be, nor hold, a folder root such as the resource folder;
/// - after canonicalizing, still have its folder inside the canonical root;
/// - for an object token, sit directly in one of its allowed folders;
/// - for `trees`, pass the manifest's name checks.
pub fn check_removal(
    roots: &UninstallRoots,
    path: &Path,
    guard: &RemovalGuard,
) -> Result<(), PathRefusal> {
    if !is_plain_absolute(path) {
        return Err(PathRefusal::NotPlain);
    }
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(PathRefusal::NotPlain);
    };
    if let Some(root) = &guard.root
        && !strictly_inside(path, root)
    {
        return Err(if same_ignoring_case(path, root) {
            PathRefusal::FolderRoot
        } else {
            PathRefusal::OutsideRoot
        });
    }
    let rabbit_folder = roots.resource.join(RABBIT_FOLDER);
    if inside_or_same(path, &rabbit_folder) {
        return Err(PathRefusal::RabbitFolder);
    }

    let metadata =
        fs::symlink_metadata(path).map_err(|error| PathRefusal::unreadable(path, &error))?;
    if metadata.file_type().is_symlink() {
        return Err(PathRefusal::Link);
    }
    let canonical_parent =
        fs::canonicalize(parent).map_err(|error| PathRefusal::unreadable(parent, &error))?;
    let canonical = canonical_parent.join(name);

    // RABBIT's folder as the resource folder's real path spells it, and,
    // should RABBIT itself be a link, as that link's target spells it.
    let rabbit_spellings = [
        canonical_or_absolute(&roots.resource).join(RABBIT_FOLDER),
        canonical_or_absolute(&rabbit_folder),
    ];
    if rabbit_spellings
        .iter()
        .any(|rabbit| inside_or_same(&canonical, rabbit))
    {
        return Err(PathRefusal::RabbitFolder);
    }
    for root in roots.folder_roots() {
        if inside_or_same(root, path) || inside_or_same(&canonical_or_absolute(root), &canonical) {
            return Err(PathRefusal::FolderRoot);
        }
    }
    if let Some(root) = &guard.root {
        let canonical_root =
            fs::canonicalize(root).map_err(|error| PathRefusal::unreadable(root, &error))?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(PathRefusal::OutsideRoot);
        }
    }
    if !guard.allowed_parents.is_empty()
        && !guard
            .allowed_parents
            .iter()
            .filter_map(|allowed| fs::canonicalize(allowed).ok())
            .any(|allowed| allowed == canonical_parent)
    {
        return Err(PathRefusal::ParentNotAllowed);
    }
    let name = name.to_string_lossy().to_lowercase();
    let ends_well = guard
        .must_end_with
        .as_deref()
        .is_none_or(|suffix| name.ends_with(&suffix.to_lowercase()));
    let contains_well = guard
        .name_contains
        .as_deref()
        .is_none_or(|part| name.contains(&part.to_lowercase()));
    if !ends_well || !contains_well {
        return Err(PathRefusal::UnexpectedName);
    }
    Ok(())
}

/// Whether `path` is at or below `{resource}/RABBIT`, compared by its text.
/// Candidates that come from a receipt are dropped this way before the guard
/// would refuse them: RABBIT's own files are never a package's to remove.
pub fn is_in_rabbit_folder(roots: &UninstallRoots, path: &Path) -> bool {
    inside_or_same(path, &roots.resource.join(RABBIT_FOLDER))
}

/// Whether `path` is the same as `other`, or below it, compared by text and
/// ignoring case.
pub fn inside_or_same(path: &Path, other: &Path) -> bool {
    match (folded_components(path), folded_components(other)) {
        (Some(path), Some(other)) => !other.is_empty() && path.starts_with(&other),
        _ => false,
    }
}

/// Whether two paths name the same thing by their text, ignoring case.
pub fn same_ignoring_case(left: &Path, right: &Path) -> bool {
    match (folded_components(left), folded_components(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// Whether `path` is strictly below `root`, compared by text and ignoring
/// case.
fn strictly_inside(path: &Path, root: &Path) -> bool {
    match (folded_components(path), folded_components(root)) {
        (Some(path), Some(root)) => {
            !root.is_empty() && path.len() > root.len() && path.starts_with(&root)
        }
        _ => false,
    }
}

/// `path` canonicalized, or made absolute when it doesn't exist yet.
fn canonical_or_absolute(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| absolute(path))
}

/// `path` made absolute, with no `.` or `..` left. A path that can't be made
/// absolute is kept as it is; the guard then refuses it as not plain.
pub(crate) fn absolute(path: &Path) -> PathBuf {
    crate::rollback::absolute_without_dots(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir))
}

/// The components of `path`, lowercased, with `\\?\C:\` read like `C:\`.
/// `None` when the path holds `.` or `..`: its text can't say where it ends.
fn folded_components(path: &Path) -> Option<Vec<String>> {
    path.components()
        .map(|component| match component {
            Component::Prefix(prefix) => Some(match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                    format!("{}:", char::from(letter))
                }
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => format!(
                    r"\\{}\{}",
                    server.to_string_lossy(),
                    share.to_string_lossy()
                ),
                _ => prefix.as_os_str().to_string_lossy().into_owned(),
            }),
            Component::RootDir => Some("/".to_string()),
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            Component::CurDir | Component::ParentDir => None,
        })
        .map(|part| part.map(|part| part.to_lowercase()))
        .collect()
}

/// Make `link` a junction to the folder `target` with `mklink /J`, which
/// needs no administrator rights, unlike a symbolic link.
#[cfg(all(test, windows))]
pub(crate) fn make_junction(link: &Path, target: &Path) {
    // cmd reads a `/` in a path as the start of an option.
    let windows_path = |path: &Path| path.to_string_lossy().replace('/', "\\");
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(windows_path(link))
        .arg(windows_path(target))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::{TempDir, tempdir};

    use super::{
        PathRefusal, RemovalGuard, ResolvedPath, UninstallRoots, check_removal, expand_wildcard,
        resolve_path,
    };
    use crate::package::uninstall::UninstallPath;

    /// A temporary folder holding a resource folder `REAPER`, and roots that
    /// know it and a CLAP folder beside it.
    fn fixture() -> (TempDir, UninstallRoots) {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let mut roots = UninstallRoots::new(&resource);
        roots.clap_dir = Some(dir.path().join("CLAP"));
        (dir, roots)
    }

    fn write(path: &Path) -> PathBuf {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"file").unwrap();
        path.to_path_buf()
    }

    fn resolve(path: &str, roots: &UninstallRoots) -> Result<Vec<ResolvedPath>, PathRefusal> {
        resolve_path(&UninstallPath::new(path), roots)
    }

    fn resolve_one(path: &str, roots: &UninstallRoots) -> ResolvedPath {
        let [resolved] = resolve(path, roots).unwrap().try_into().unwrap();
        resolved
    }

    #[test]
    fn tokens_resolve_to_their_folders() {
        let (dir, mut roots) = fixture();
        roots.program_files = vec![dir.path().join("PF"), dir.path().join("PF86")];
        roots.app_path = Some(roots.resource.join("REAPER.app"));

        let plugin = resolve_one("{resource}/UserPlugins/reaper_demo.dll", &roots);
        assert_eq!(
            plugin.path,
            roots.resource.join("UserPlugins").join("reaper_demo.dll")
        );
        assert_eq!(plugin.guard, RemovalGuard::under(roots.resource.clone()));
        assert!(!plugin.is_root && !plugin.wildcard);

        assert!(
            resolve_one("{resource}", &roots).is_root,
            "a bare folder root is the root itself"
        );
        assert!(resolve_one("{resource}/osara/locale/*.po", &roots).wildcard);

        let program_files = resolve("{program_files}/Demo", &roots).unwrap();
        assert_eq!(
            program_files
                .iter()
                .map(|resolved| resolved.path.clone())
                .collect::<Vec<_>>(),
            vec![
                dir.path().join("PF").join("Demo"),
                dir.path().join("PF86").join("Demo")
            ]
        );

        // An object token names the thing itself: it has no root to stay
        // under, only folders it may sit in.
        let app = resolve_one("{app_path}", &roots);
        assert_eq!(app.path, roots.resource.join("REAPER.app"));
        assert_eq!(app.guard.root, None);
        assert!(app.guard.allowed_parents.contains(&roots.resource));
        assert!(!app.is_root);

        assert_eq!(
            resolve("{documents}/Surge XT", &roots),
            Err(PathRefusal::UnknownRoot {
                token: "{documents}".to_string()
            })
        );
    }

    #[test]
    fn the_guard_refuses_folder_roots_dots_and_paths_outside_their_root() {
        let (dir, roots) = fixture();
        let inside = RemovalGuard::under(roots.resource.clone());
        let plugin = write(&roots.resource.join("UserPlugins").join("reaper_demo.dll"));
        let outside = write(&dir.path().join("elsewhere").join("reaper_demo.dll"));

        assert_eq!(check_removal(&roots, &plugin, &inside), Ok(()));
        assert_eq!(
            check_removal(&roots, &roots.resource, &inside),
            Err(PathRefusal::FolderRoot)
        );
        assert_eq!(
            check_removal(&roots, &outside, &inside),
            Err(PathRefusal::OutsideRoot)
        );
        let dotted = roots
            .resource
            .join("UserPlugins")
            .join("..")
            .join("..")
            .join("elsewhere")
            .join("reaper_demo.dll");
        assert_eq!(
            check_removal(&roots, &dotted, &inside),
            Err(PathRefusal::NotPlain)
        );
        assert_eq!(
            check_removal(&roots, Path::new("UserPlugins/reaper_demo.dll"), &inside),
            Err(PathRefusal::NotPlain)
        );
    }

    /// Reached through another root, the resource folder is still a whole
    /// folder: `{appdata}/REAPER` is the standard resource folder.
    #[test]
    fn the_guard_refuses_anything_that_holds_a_folder_root() {
        let (dir, mut roots) = fixture();
        roots.appdata = Some(dir.path().to_path_buf());
        let through_appdata = RemovalGuard::under(dir.path().to_path_buf());
        assert_eq!(
            check_removal(&roots, &dir.path().join("REAPER"), &through_appdata),
            Err(PathRefusal::FolderRoot)
        );
        let clap = roots.clap_dir.clone().unwrap();
        fs::create_dir_all(&clap).unwrap();
        assert_eq!(
            check_removal(&roots, &clap, &through_appdata),
            Err(PathRefusal::FolderRoot)
        );
    }

    #[test]
    fn the_guard_refuses_rabbits_folder_however_it_is_reached() {
        let (dir, mut roots) = fixture();
        roots.appdata = Some(dir.path().to_path_buf());
        let inside = RemovalGuard::under(roots.resource.clone());
        let receipt = write(&roots.resource.join("RABBIT").join("install-state.json"));
        let backup = write(&roots.resource.join("rabbit").join("backups").join("x.dll"));

        assert_eq!(
            check_removal(&roots, &receipt, &inside),
            Err(PathRefusal::RabbitFolder)
        );
        assert_eq!(
            check_removal(&roots, &backup, &inside),
            Err(PathRefusal::RabbitFolder),
            "case doesn't matter"
        );
        assert_eq!(
            check_removal(&roots, &roots.resource.join("RABBIT"), &inside),
            Err(PathRefusal::RabbitFolder)
        );
        let through_appdata = dir
            .path()
            .join("REAPER")
            .join("RABBIT")
            .join("install-state.json");
        assert_eq!(
            check_removal(
                &roots,
                &through_appdata,
                &RemovalGuard::under(dir.path().to_path_buf())
            ),
            Err(PathRefusal::RabbitFolder)
        );
    }

    #[test]
    fn object_tokens_sit_in_an_allowed_folder_and_pass_the_name_checks() {
        let (dir, mut roots) = fixture();
        let app = roots.resource.join("REAPER.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        roots.app_path = Some(app.clone());
        let tree = RemovalGuard {
            must_end_with: Some(".app".to_string()),
            name_contains: Some("reaper".to_string()),
            ..resolve_one("{app_path}", &roots).guard
        };
        assert_eq!(check_removal(&roots, &app, &tree), Ok(()));

        let elsewhere = dir.path().join("Other").join("REAPER.app");
        fs::create_dir_all(&elsewhere).unwrap();
        assert_eq!(
            check_removal(&roots, &elsewhere, &tree),
            Err(PathRefusal::ParentNotAllowed)
        );

        let wrong_name = roots.resource.join("Surge XT.app");
        fs::create_dir_all(&wrong_name).unwrap();
        assert_eq!(
            check_removal(&roots, &wrong_name, &tree),
            Err(PathRefusal::UnexpectedName)
        );
    }

    #[test]
    fn wildcards_expand_to_matching_names_only() {
        let (_dir, roots) = fixture();
        let locale = roots.resource.join("osara").join("locale");
        let de = write(&locale.join("de_DE.po"));
        let fr = write(&locale.join("FR_fr.PO"));
        write(&locale.join("readme.txt"));
        let mut expected = vec![de, fr];
        expected.sort();
        assert_eq!(
            expand_wildcard(&resolve_one("{resource}/osara/locale/*.po", &roots)).unwrap(),
            expected
        );
        assert!(
            expand_wildcard(&resolve_one("{resource}/nothing/here/*.po", &roots))
                .unwrap()
                .is_empty()
        );
    }

    /// Rust reports a junction as a link, so the guard never removes one as
    /// if it were a file or a folder, and never goes through one out of the
    /// root. A resource folder that is itself reached through a junction
    /// still works, because roots are compared canonicalized.
    #[cfg(windows)]
    #[test]
    fn junctions_are_refused_and_never_followed() {
        use super::make_junction;

        let (dir, roots) = fixture();
        let inside = RemovalGuard::under(roots.resource.clone());
        let target = dir.path().join("target");
        write(&target.join("reaper_demo.dll"));

        let link = roots.resource.join("UserPlugins").join("reaper_demo.dll");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        make_junction(&link, &target);
        assert_eq!(
            check_removal(&roots, &link, &inside),
            Err(PathRefusal::Link)
        );

        // A junction further up leads out of the resource folder.
        let scripts = roots.resource.join("Scripts");
        make_junction(&scripts, &target);
        assert_eq!(
            check_removal(&roots, &scripts.join("reaper_demo.dll"), &inside),
            Err(PathRefusal::OutsideRoot)
        );

        // The resource folder itself reached through a junction.
        let real = dir.path().join("real REAPER");
        let plugin = write(&real.join("UserPlugins").join("reaper_demo.dll"));
        let aliased = dir.path().join("aliased REAPER");
        make_junction(&aliased, &real);
        assert_eq!(
            check_removal(
                &UninstallRoots::new(&aliased),
                &aliased.join("UserPlugins").join("reaper_demo.dll"),
                &RemovalGuard::under(aliased.clone())
            ),
            Ok(())
        );
        assert!(plugin.is_file(), "checking removes nothing");
        assert!(target.join("reaper_demo.dll").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_refused_and_never_followed() {
        use std::os::unix::fs::symlink;

        let (dir, roots) = fixture();
        let inside = RemovalGuard::under(roots.resource.clone());
        let target = write(&dir.path().join("target").join("reaper_demo.dll"));

        let link = roots.resource.join("UserPlugins").join("reaper_demo.dll");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&target, &link).unwrap();
        assert_eq!(
            check_removal(&roots, &link, &inside),
            Err(PathRefusal::Link)
        );

        let scripts = roots.resource.join("Scripts");
        symlink(target.parent().unwrap(), &scripts).unwrap();
        assert_eq!(
            check_removal(&roots, &scripts.join("reaper_demo.dll"), &inside),
            Err(PathRefusal::OutsideRoot)
        );

        let real = dir.path().join("real REAPER");
        write(&real.join("UserPlugins").join("reaper_demo.dll"));
        let aliased = dir.path().join("aliased REAPER");
        symlink(&real, &aliased).unwrap();
        assert_eq!(
            check_removal(
                &UninstallRoots::new(&aliased),
                &aliased.join("UserPlugins").join("reaper_demo.dll"),
                &RemovalGuard::under(aliased.clone())
            ),
            Ok(())
        );
    }
}
