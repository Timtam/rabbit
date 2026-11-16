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

use super::PlannedRemoval;
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
    /// `C:\Program Files (x86)` alone, as Windows names it, never from the
    /// environment.
    pub program_files_x86: Option<PathBuf>,
    pub program_data: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub documents: Option<PathBuf>,
    /// Where absolute macOS paths such as `/Applications/Surge XT.app`
    /// start: `/` on a real computer. Tests point it at a temporary folder
    /// holding their own `Applications` and `Library`, so the macOS rules
    /// for those folders run on every platform without touching the real
    /// ones.
    pub system_root: PathBuf,
}

/// The folder absolute macOS paths start from on a real computer.
pub const SYSTEM_ROOT: &str = "/";

impl UninstallRoots {
    /// Roots with only the resource folder known, and absolute macOS paths
    /// starting at [`SYSTEM_ROOT`]. Every other token stays unresolved until
    /// the caller fills it in.
    pub fn new(resource: &Path) -> Self {
        Self {
            resource: absolute(resource),
            system_root: PathBuf::from(SYSTEM_ROOT),
            ..Self::default()
        }
    }

    /// The roots for a target at `resource` with REAPER's app at
    /// `app_path`, read from this computer. Windows folders resolve only on
    /// Windows and `{home}` only on macOS, as the manifest's validation
    /// expects: Git Bash sets `HOME` on Windows, but no plan means it there.
    /// `{documents}` is the user's Documents folder, which Windows keeps as a
    /// known folder an environment variable can't name. `{program_files_x86}`
    /// is read as a known folder too, the way the elevated helper reads it.
    pub fn for_target(platform: Platform, resource: &Path, app_path: Option<&Path>) -> Self {
        let windows = platform == Platform::Windows;
        // A trailing slash, which shell completion adds to a folder such as
        // `/Applications/REAPER.app/`, makes `lstat` and `rename` follow a
        // link in the last place, so it never gets into a plan: rebuilding
        // the path from its components drops it.
        let app_path = app_path.map(absolute).map(|path| {
            if windows {
                path
            } else {
                path.components().collect::<PathBuf>()
            }
        });
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
            program_files_x86: windows
                .then(|| rabbit_platform::machine_program_files_x86_dir().ok())
                .flatten(),
            program_data: windows
                .then(rabbit_platform::windows_program_data_dir)
                .flatten(),
            home: (!windows).then(rabbit_platform::user_home_dir).flatten(),
            documents: rabbit_platform::user_documents_dir(),
            system_root: PathBuf::from(SYSTEM_ROOT),
        }
    }

    /// The path an absolute macOS path's `components` stand for, below
    /// [`system_root`](Self::system_root).
    fn system_path<'c>(&self, components: impl IntoIterator<Item = &'c str>) -> PathBuf {
        components
            .into_iter()
            .fold(self.system_root.clone(), |path, component| {
                path.join(component)
            })
    }

    /// The fixed system folders an absolute macOS path may sit below, as
    /// they resolve here: `/Applications`, `/Library/Application Support`
    /// and the `/Library/Audio/Plug-Ins` format folders.
    fn system_folders(&self) -> impl Iterator<Item = PathBuf> + '_ {
        MACOS_ABSOLUTE_PREFIXES
            .iter()
            .map(|prefix| self.system_path(prefix.iter().copied()))
    }

    /// `/Applications` as it resolves here: the one folder root may move
    /// REAPER's app out of, into the user's Trash.
    pub fn system_applications(&self) -> PathBuf {
        self.system_path(["Applications"])
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
            UninstallPathToken::ProgramFilesX86 => one(&self.program_files_x86),
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
                    &self.program_files_x86,
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
                let mut parents = vec![self.system_applications()];
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
            Ok(vec![ResolvedPath {
                path: roots.system_path(parsed.components.iter().copied()),
                guard: RemovalGuard::under(roots.system_path(prefix.iter().copied())),
                is_root: false,
                wildcard,
            }])
        }
    }
}

/// A `vendor_added_files` pattern resolved for this computer, such as
/// `{appdata}/Freedom Scientific/JAWS/*/Settings/*/*`. RABBIT never lists a
/// folder for it: it only decides whether a path the receipt records
/// matches; see [`pattern_matches`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedPattern {
    /// The folder the pattern's token stands for.
    pub base: PathBuf,
    /// The components below `base`, in order. A `*` in one stands for any
    /// part of a name.
    pub components: Vec<String>,
    /// What every match must pass: it stays inside `base` and the plain
    /// names before the first `*`, such as `%APPDATA%\Freedom
    /// Scientific\JAWS`.
    pub guard: RemovalGuard,
}

/// Resolve a `vendor_added_files` pattern, one [`ResolvedPattern`] per
/// folder its token stands for. A token that stands for nothing here is
/// refused, as [`resolve_path`] refuses it. Validation lets only
/// `{appdata}` and `{localappdata}` through; a hand-built spec with any
/// other root that isn't a plain folder root is refused too.
pub(crate) fn resolve_pattern(
    path: &UninstallPath,
    roots: &UninstallRoots,
) -> Result<Vec<ResolvedPattern>, PathRefusal> {
    let parsed = path.parse_pattern().map_err(|_| PathRefusal::NotPlain)?;
    let token = match parsed.root {
        UninstallPathRoot::Token(token) if token.kind() == UninstallTokenKind::FolderRoot => token,
        _ => return Err(PathRefusal::OutsideRoot),
    };
    if parsed.components.is_empty() {
        return Err(PathRefusal::FolderRoot);
    }
    let bases = roots.resolve(token);
    if bases.is_empty() {
        return Err(PathRefusal::UnknownRoot {
            token: format!("{{{}}}", token.name()),
        });
    }
    // The plain names before the first `*`, and never the last component:
    // that is the file itself, which can't be its own guard.
    let plain = parsed
        .components
        .iter()
        .take(parsed.components.len() - 1)
        .take_while(|component| !component.contains('*'))
        .collect::<Vec<_>>();
    Ok(bases
        .into_iter()
        .map(|base| {
            let guard_root = plain
                .iter()
                .fold(base.clone(), |path, component| path.join(component));
            ResolvedPattern {
                base,
                components: parsed
                    .components
                    .iter()
                    .map(|component| component.to_string())
                    .collect(),
                guard: RemovalGuard::under(guard_root),
            }
        })
        .collect())
}

/// Whether `path` matches `pattern`: it is plain and absolute, starts with
/// the pattern's base (compared ignoring case), and then has exactly one
/// name for each of the pattern's components, each the same name ignoring
/// ASCII case or matching its `*`. A name holding `:` never matches: on
/// Windows that names an alternate data stream, not a file of its own.
pub(crate) fn pattern_matches(pattern: &ResolvedPattern, path: &Path) -> bool {
    if !is_plain_absolute(path) {
        return false;
    }
    let Some(base) = folded_components(&pattern.base) else {
        return false;
    };
    let components: Vec<Component<'_>> = path.components().collect();
    if components.len() != base.len() + pattern.components.len() {
        return false;
    }
    let (start, rest) = components.split_at(base.len());
    let same_base = start
        .iter()
        .zip(&base)
        .all(|(component, folded)| fold_component(*component).as_ref() == Some(folded));
    same_base
        && rest
            .iter()
            .zip(&pattern.components)
            .all(|(component, wanted)| {
                let Component::Normal(name) = component else {
                    return false;
                };
                let Some(name) = name.to_str() else {
                    return false;
                };
                !name.contains(':')
                    && if wanted.contains('*') {
                        wildcard_matches_ignoring_case(wanted, name)
                    } else {
                        name.eq_ignore_ascii_case(wanted)
                    }
            })
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
    #[error("it isn't the link RABBIT expected there")]
    NotALink,
    #[error("it is in RABBIT's own folder, which holds receipts, backups and logs")]
    RabbitFolder,
    #[error("its name isn't what the package manifest expects")]
    UnexpectedName,
    #[error("it isn't in a folder it may be removed from")]
    ParentNotAllowed,
    #[error("RABBIT couldn't check it: {message}")]
    Unreadable { message: String },
    /// For a removal with administrator rights: a folder on the way to it
    /// is a link, which could send root somewhere else.
    #[error("a folder on the way to it is a symbolic link, which RABBIT never lets root follow")]
    LinkedAncestor,
    /// For a removal with administrator rights: it isn't directly in one of
    /// the fixed system folders only an administrator can change.
    #[error("it isn't directly in a system folder that only an administrator can change")]
    NotRootControlled,
    /// For a removal with administrator rights: it, or a folder on the way
    /// to it, isn't a folder.
    #[error("it isn't a folder")]
    NotADirectory,
    /// For a removal with administrator rights: its path can't be written
    /// as text, which the administrator command needs.
    #[error("its name isn't valid text, so RABBIT can't name it to the administrator command")]
    NotUtf8,
    /// For a removal with administrator rights: another volume is mounted
    /// exactly there. `rm -R -x` only stays out of a volume mounted below
    /// the folder it is given, so it would empty this one.
    #[error("another disk or volume is mounted there, and RABBIT never lets root reach into it")]
    MountPoint,
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
    check_removal_of(roots, path, guard, LastComponent::NotALink)
}

/// [`check_removal`] for a link that is to go as a link, such as a junction
/// a vendor's uninstaller leaves behind: every check is the same, except
/// that the last component must be a link rather than must not be one.
/// Whatever removes it must still remove the link itself, never what it
/// points to.
pub fn check_link_removal(
    roots: &UninstallRoots,
    path: &Path,
    guard: &RemovalGuard,
) -> Result<(), PathRefusal> {
    check_removal_of(roots, path, guard, LastComponent::Link)
}

/// What the last component of a path to remove must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LastComponent {
    NotALink,
    Link,
}

fn check_removal_of(
    roots: &UninstallRoots,
    path: &Path,
    guard: &RemovalGuard,
    last: LastComponent,
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
    let is_link = metadata.file_type().is_symlink();
    match last {
        LastComponent::NotALink if is_link => return Err(PathRefusal::Link),
        LastComponent::Link if !is_link => return Err(PathRefusal::NotALink),
        _ => {}
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

/// Whether the run's administrator command may delete `planned`, an
/// absolute macOS path from a manifest such as `/Applications/Surge XT.app`,
/// and the exact text to name it with there.
///
/// Root deletes for good, so this asks more than [`check_removal`], which
/// it runs first. The path must also:
/// - have come from one of the fixed system folders, and sit directly in
///   it: never a token such as `{home}` or `{resource}`, never deeper;
/// - be a real folder, not a link and not a file;
/// - be reached through real folders only, from the system root down:
///   `lstat` of every folder on the way, so no link anywhere can send root
///   somewhere else;
/// - be on the same volume as the folder holding it, so no volume is
///   mounted exactly there;
/// - be valid UTF-8, the only text the administrator command takes.
pub fn check_administrator_removal(
    roots: &UninstallRoots,
    planned: &PlannedRemoval,
) -> Result<String, PathRefusal> {
    check_removal(roots, &planned.path, &planned.guard)?;
    let folder = planned
        .guard
        .root
        .as_deref()
        .filter(|root| roots.system_folders().any(|folder| folder == *root))
        .ok_or(PathRefusal::NotRootControlled)?;
    check_root_reaches(roots, &planned.path, folder, &rabbit_platform::volume_id)
}

/// Whether the run's administrator command may move `planned`, REAPER's
/// app, out of `/Applications` into the user's Trash, and the exact text
/// to name it with there.
///
/// Root moves the app only when RABBIT can't, so this asks more than
/// [`check_removal`], which it runs first: the app must sit directly in
/// `/Applications`, a folder only an administrator can change, never in a
/// folder of the user's or on another volume; it must be a real folder, not
/// a link, with no volume mounted on it; and every folder on the way to it
/// from the system root must be a real folder too, read with `lstat`.
/// Whether it is the app the manifest means, and whether the user's Trash
/// may take it, is [`tree_route`](super::macos_objects::tree_route)'s to
/// say.
pub fn check_administrator_trash_move(
    roots: &UninstallRoots,
    planned: &PlannedRemoval,
) -> Result<String, PathRefusal> {
    check_removal(roots, &planned.path, &planned.guard)?;
    check_root_reaches(
        roots,
        &planned.path,
        &roots.system_applications(),
        &rabbit_platform::volume_id,
    )
}

/// The checks both administrator steps share: `path` sits directly in
/// `folder`, is a real folder, every folder from the system root down to
/// `folder` is a real folder, not a link, and `path` is on `folder`'s
/// volume, as `volume` reads them. Returns `path` as text, spelt exactly
/// as `folder` and its name, which it must already be.
fn check_root_reaches(
    roots: &UninstallRoots,
    path: &Path,
    folder: &Path,
    volume: &dyn Fn(&Path) -> io::Result<u64>,
) -> Result<String, PathRefusal> {
    if path.parent() != Some(folder) {
        return Err(PathRefusal::NotRootControlled);
    }
    let name = path.file_name().ok_or(PathRefusal::NotPlain)?;
    let exact = folder.join(name);
    // A trailing slash makes `lstat` and `rename` follow a link in the last
    // place, though `Path` compares as if it weren't there. Root gets only
    // folder/name, spelt exactly so, and so is everything checked here.
    if path.as_os_str() != exact.as_os_str() {
        return Err(PathRefusal::NotPlain);
    }
    let path = exact.as_path();
    let metadata =
        fs::symlink_metadata(path).map_err(|error| PathRefusal::unreadable(path, &error))?;
    if metadata.file_type().is_symlink() {
        return Err(PathRefusal::Link);
    }
    if !metadata.is_dir() {
        return Err(PathRefusal::NotADirectory);
    }
    let below_root = folder
        .strip_prefix(&roots.system_root)
        .map_err(|_| PathRefusal::NotRootControlled)?;
    let mut on_the_way = roots.system_root.clone();
    real_folder(&on_the_way)?;
    for component in below_root.components() {
        let Component::Normal(name) = component else {
            return Err(PathRefusal::NotPlain);
        };
        on_the_way.push(name);
        real_folder(&on_the_way)?;
    }
    // `rm -R -x` stays on the volume of the folder it is given: it keeps
    // out of a volume mounted below that folder, but empties one mounted
    // on the folder itself. A mount there shows as a volume other than the
    // holding folder's. Where volumes can't be read this way, as on
    // Windows, which has no administrator command, nothing is compared.
    match (volume(path), volume(folder)) {
        (Ok(tree), Ok(holder)) if tree != holder => return Err(PathRefusal::MountPoint),
        (Err(error), _) if error.kind() != io::ErrorKind::Unsupported => {
            return Err(PathRefusal::unreadable(path, &error));
        }
        (_, Err(error)) if error.kind() != io::ErrorKind::Unsupported => {
            return Err(PathRefusal::unreadable(folder, &error));
        }
        _ => {}
    }
    path.to_str()
        .map(str::to_string)
        .ok_or(PathRefusal::NotUtf8)
}

/// `folder`, read with `lstat`, is a folder and not a link.
fn real_folder(folder: &Path) -> Result<(), PathRefusal> {
    let metadata =
        fs::symlink_metadata(folder).map_err(|error| PathRefusal::unreadable(folder, &error))?;
    if metadata.file_type().is_symlink() {
        Err(PathRefusal::LinkedAncestor)
    } else if metadata.is_dir() {
        Ok(())
    } else {
        Err(PathRefusal::NotADirectory)
    }
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

/// Whether two folders are the same one, whether or not they still exist:
/// by their text, ignoring case, or by their real paths. A folder that is
/// gone is placed by the real path of the nearest folder above it that is
/// still there, so a resource folder reached through a junction, or spelt
/// with a short 8.3 name, still matches after the folder inside it was
/// removed.
pub(crate) fn same_folder(left: &Path, right: &Path) -> bool {
    if same_ignoring_case(left, right) {
        return true;
    }
    match (real_path_so_far(left), real_path_so_far(right)) {
        (Some(left), Some(right)) => same_ignoring_case(&left, &right),
        _ => false,
    }
}

/// `path` with the part of it that exists canonicalized and the rest kept
/// as written. `None` for a path that isn't plain and absolute, and for one
/// none of which exists.
fn real_path_so_far(path: &Path) -> Option<PathBuf> {
    if !is_plain_absolute(path) {
        return None;
    }
    let mut existing = path;
    let mut missing = Vec::new();
    loop {
        if let Ok(real) = fs::canonicalize(existing) {
            return Some(
                missing
                    .iter()
                    .rev()
                    .fold(real, |path, name| path.join(name)),
            );
        }
        missing.push(existing.file_name()?);
        existing = existing.parent()?;
    }
}

/// Whether `path` is strictly below `root`, compared by text and ignoring
/// case.
pub(crate) fn strictly_inside(path: &Path, root: &Path) -> bool {
    match (folded_components(path), folded_components(root)) {
        (Some(path), Some(root)) => {
            !root.is_empty() && path.len() > root.len() && path.starts_with(&root)
        }
        _ => false,
    }
}

/// `path` canonicalized, or made absolute when it doesn't exist yet.
fn canonical_or_absolute(path: &Path) -> PathBuf {
    if is_network_path(path) {
        return absolute(path);
    }
    fs::canonicalize(path).unwrap_or_else(|_| absolute(path))
}

/// Whether `path` is on a network share, `\\server\share\...` in either
/// spelling. The guard never resolves such a root, it compares its text:
/// resolving it connects to that server, and in the elevated helper, which
/// is given the resource folder only to keep away from it, that would sign
/// in there with an administrator's account, on a server the standard user
/// who started RABBIT may have chosen.
pub(crate) fn is_network_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..))
    )
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
pub(crate) fn folded_components(path: &Path) -> Option<Vec<String>> {
    path.components().map(fold_component).collect()
}

/// One component as [`folded_components`] compares it.
fn fold_component(component: Component<'_>) -> Option<String> {
    let part = match component {
        Component::Prefix(prefix) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                format!("{}:", char::from(letter))
            }
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => format!(
                r"\\{}\{}",
                server.to_string_lossy(),
                share.to_string_lossy()
            ),
            _ => prefix.as_os_str().to_string_lossy().into_owned(),
        },
        Component::RootDir => "/".to_string(),
        Component::Normal(part) => part.to_string_lossy().into_owned(),
        Component::CurDir | Component::ParentDir => return None,
    };
    Some(part.to_lowercase())
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

/// Make `link` point to the folder `target`: a junction on Windows, which
/// needs no administrator rights, a symbolic link elsewhere. RABBIT, and
/// its guard, see both as links.
#[cfg(test)]
pub(crate) fn link_folder(link: &Path, target: &Path) {
    #[cfg(windows)]
    make_junction(link, target);
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::{TempDir, tempdir};

    use super::{
        PathRefusal, RemovalGuard, ResolvedPath, ResolvedPattern, UninstallRoots,
        check_administrator_removal, check_administrator_trash_move, check_removal,
        check_root_reaches, expand_wildcard, link_folder, pattern_matches, resolve_path,
        resolve_pattern,
    };
    use crate::model::Platform;
    use crate::package::uninstall::UninstallPath;
    use crate::uninstall::{PlannedRemoval, RemovalReason};

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

    /// A share in either spelling is a network path the guard never
    /// resolves; a drive, local device paths and relative paths aren't.
    #[cfg(windows)]
    #[test]
    fn shares_are_network_paths_and_drives_are_not() {
        use super::is_network_path;
        for path in [
            "\\\\server\\share\\REAPER",
            "\\\\?\\UNC\\server\\share\\REAPER",
        ] {
            assert!(is_network_path(Path::new(path)), "{path}");
        }
        for path in [
            "C:\\REAPER",
            "\\\\?\\C:\\REAPER",
            "\\\\.\\C:\\REAPER",
            "REAPER",
        ] {
            assert!(!is_network_path(Path::new(path)), "{path}");
        }
    }

    /// The guard works with a resource folder on a share, as the elevated
    /// helper gets it when `%APPDATA%` is redirected: a file in a local
    /// folder may go, and one spelt inside the share's RABBIT folder is
    /// still refused, by its text.
    #[cfg(windows)]
    #[test]
    fn a_resource_folder_on_a_share_is_kept_away_from_by_its_text() {
        let (dir, mut roots) = fixture();
        let share = PathBuf::from("\\\\rabbit-test.invalid\\share\\REAPER");
        roots.resource = share.clone();
        let clap = dir.path().join("CLAP");
        let plugin = write(&clap.join("demo.clap"));
        assert_eq!(
            check_removal(&roots, &plugin, &RemovalGuard::under(clap)),
            Ok(())
        );
        assert_eq!(
            check_removal(
                &roots,
                &share.join("RABBIT").join("install-state.json"),
                &RemovalGuard::under(share.clone())
            ),
            Err(PathRefusal::RabbitFolder)
        );
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
        // `{program_files_x86}` is the 32-bit folder alone.
        assert_eq!(
            resolve("{program_files_x86}/Demo", &roots),
            Err(PathRefusal::UnknownRoot {
                token: "{program_files_x86}".to_string()
            })
        );
        roots.program_files_x86 = Some(dir.path().join("PF86"));
        let x86 = resolve_one("{program_files_x86}/Demo", &roots);
        assert_eq!(x86.path, dir.path().join("PF86").join("Demo"));
        assert_eq!(x86.guard, RemovalGuard::under(dir.path().join("PF86")));

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

    /// A temporary folder standing in for `/`, holding the system folders
    /// absolute macOS paths may name, and roots that start such paths there.
    fn system_fixture() -> (TempDir, UninstallRoots) {
        let (dir, mut roots) = fixture();
        let system = dir.path().join("system");
        for folder in [
            "Applications",
            "Library/Application Support",
            "Library/Audio/Plug-Ins/VST3",
            "Library/Audio/Plug-Ins/Components",
        ] {
            fs::create_dir_all(
                folder
                    .split('/')
                    .fold(system.clone(), |path, part| path.join(part)),
            )
            .unwrap();
        }
        roots.system_root = system;
        (dir, roots)
    }

    /// A tree from a manifest, resolved and planned as the planner would.
    fn planned_tree(manifest_path: &str, roots: &UninstallRoots) -> PlannedRemoval {
        let resolved = resolve_one(manifest_path, roots);
        PlannedRemoval {
            is_dir: resolved.path.is_dir(),
            path: resolved.path,
            reason: RemovalReason::Tree,
            size: None,
            receipt_sha256: None,
            bundle_id: None,
            guard: resolved.guard,
        }
    }

    #[test]
    fn absolute_macos_paths_start_at_the_system_root() {
        let (_dir, mut roots) = system_fixture();
        let system = roots.system_root.clone();
        let vst3 = system
            .join("Library")
            .join("Audio")
            .join("Plug-Ins")
            .join("VST3");

        let tree = resolve_one("/Library/Audio/Plug-Ins/VST3/Demo.vst3", &roots);
        assert_eq!(tree.path, vst3.join("Demo.vst3"));
        assert_eq!(tree.guard, RemovalGuard::under(vst3));

        roots.app_path = Some(system.join("Applications").join("REAPER.app"));
        let app = resolve_one("{app_path}", &roots);
        assert!(
            app.guard
                .allowed_parents
                .contains(&system.join("Applications")),
            "{:?}",
            app.guard.allowed_parents
        );
        assert!(
            !app.guard
                .allowed_parents
                .contains(&PathBuf::from("/Applications"))
        );

        // A real computer's start at /.
        let real = UninstallRoots::new(Path::new("/REAPER"));
        assert_eq!(real.system_root, PathBuf::from("/"));
        assert_eq!(
            resolve_one("/Applications/Demo.app", &real).path,
            Path::new("/Applications/Demo.app")
        );
    }

    #[test]
    fn the_administrator_command_may_delete_a_real_folder_directly_in_a_system_folder() {
        let (_dir, roots) = system_fixture();
        for manifest_path in [
            "/Applications/Demo.app",
            "/Library/Audio/Plug-Ins/VST3/Demo.vst3",
            "/Library/Application Support/Demo",
        ] {
            let tree = planned_tree(manifest_path, &roots);
            fs::create_dir_all(tree.path.join("Contents")).unwrap();
            assert_eq!(
                check_administrator_removal(&roots, &tree),
                Ok(tree.path.to_str().unwrap().to_string()),
                "{manifest_path}"
            );
        }
    }

    #[test]
    fn the_administrator_command_never_deletes_through_a_link_on_the_way() {
        let (dir, roots) = system_fixture();
        let elsewhere = dir.path().join("elsewhere");
        fs::create_dir_all(elsewhere.join("Demo.clap")).unwrap();
        let plug_ins = roots
            .system_root
            .join("Library")
            .join("Audio")
            .join("Plug-Ins");
        link_folder(&plug_ins.join("CLAP"), &elsewhere);

        let tree = planned_tree("/Library/Audio/Plug-Ins/CLAP/Demo.clap", &roots);
        assert_eq!(
            check_administrator_removal(&roots, &tree),
            Err(PathRefusal::LinkedAncestor)
        );
        assert!(
            elsewhere.join("Demo.clap").is_dir(),
            "checking removes nothing"
        );
    }

    #[test]
    fn the_administrator_command_never_deletes_a_link_a_file_or_anything_deeper() {
        let (dir, mut roots) = system_fixture();
        let support = roots
            .system_root
            .join("Library")
            .join("Application Support");

        // The object itself a link.
        let target = dir.path().join("target");
        fs::create_dir_all(&target).unwrap();
        link_folder(&support.join("Linked"), &target);
        assert_eq!(
            check_administrator_removal(
                &roots,
                &planned_tree("/Library/Application Support/Linked", &roots)
            ),
            Err(PathRefusal::Link)
        );

        // A file where a folder should be.
        write(&support.join("Demo"));
        assert_eq!(
            check_administrator_removal(
                &roots,
                &planned_tree("/Library/Application Support/Demo", &roots)
            ),
            Err(PathRefusal::NotADirectory)
        );

        // Two levels down, which no manifest may name.
        fs::create_dir_all(support.join("Deep").join("Data")).unwrap();
        assert_eq!(
            check_administrator_removal(
                &roots,
                &planned_tree("/Library/Application Support/Deep/Data", &roots)
            ),
            Err(PathRefusal::NotRootControlled)
        );

        // Anything a token names, even a folder below the system root, as
        // every token folder is on a real Mac: only the system-folder check
        // refuses `{home}/Demo` here.
        let home = roots.system_root.join("Users").join("me");
        roots.home = Some(home.clone());
        let mine = roots.resource.join("Demo");
        let at_home = home.join("Demo");
        for (manifest_path, folder) in [("{resource}/Demo", &mine), ("{home}/Demo", &at_home)] {
            fs::create_dir_all(folder).unwrap();
            assert_eq!(
                check_administrator_removal(&roots, &planned_tree(manifest_path, &roots)),
                Err(PathRefusal::NotRootControlled),
                "{manifest_path}"
            );
        }
        assert!(
            target.is_dir() && mine.is_dir() && at_home.is_dir(),
            "checking removes nothing"
        );
    }

    /// `rm -R -x` stays out of a volume mounted below the folder it is
    /// given, but not out of one mounted on that folder itself. A tree on
    /// another volume than the folder holding it is such a mount, and root
    /// never gets it, nor an app to move. One on the same volume passes,
    /// and so does every tree where volumes can't be read, as on Windows.
    #[test]
    fn a_volume_mounted_exactly_at_a_tree_is_never_given_to_root() {
        let (_dir, roots) = system_fixture();
        let support = roots
            .system_root
            .join("Library")
            .join("Application Support");
        let tree = support.join("Demo");
        fs::create_dir_all(tree.join("Contents")).unwrap();
        let app = roots.system_root.join("Applications").join("REAPER.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        let applications = roots.system_root.join("Applications");
        let text = |path: &Path| path.to_str().unwrap().to_string();

        // The tree, or the app, is the root of another volume.
        let mounted_on = |mounted: &Path| {
            let mounted = mounted.to_path_buf();
            move |path: &Path| -> std::io::Result<u64> { Ok(if path == mounted { 2 } else { 1 }) }
        };
        assert_eq!(
            check_root_reaches(&roots, &tree, &support, &mounted_on(&tree)),
            Err(PathRefusal::MountPoint)
        );
        assert_eq!(
            check_root_reaches(&roots, &app, &applications, &mounted_on(&app)),
            Err(PathRefusal::MountPoint)
        );

        // One volume for all: it passes.
        let one_volume = |_: &Path| -> std::io::Result<u64> { Ok(1) };
        assert_eq!(
            check_root_reaches(&roots, &tree, &support, &one_volume),
            Ok(text(&tree))
        );
        // Volumes RABBIT can't read this way are never compared.
        let unsupported = |_: &Path| -> std::io::Result<u64> {
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no"))
        };
        assert_eq!(
            check_root_reaches(&roots, &tree, &support, &unsupported),
            Ok(text(&tree))
        );
        // Any other failure to read one refuses.
        let denied = |_: &Path| -> std::io::Result<u64> {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "denied",
            ))
        };
        assert!(matches!(
            check_root_reaches(&roots, &tree, &support, &denied),
            Err(PathRefusal::Unreadable { .. })
        ));
        // And the real reader passes a temporary folder, on one volume.
        assert_eq!(
            check_root_reaches(&roots, &tree, &support, &rabbit_platform::volume_id),
            Ok(text(&tree))
        );
        assert!(tree.is_dir() && app.is_dir(), "checking removes nothing");
    }

    #[test]
    fn root_moves_reaper_only_out_of_applications_through_real_folders() {
        let (dir, mut roots) = system_fixture();
        let applications = roots.system_root.join("Applications");
        let app = applications.join("REAPER.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        roots.app_path = Some(app.clone());
        let tree = |roots: &UninstallRoots| PlannedRemoval {
            guard: RemovalGuard {
                must_end_with: Some(".app".to_string()),
                name_contains: Some("REAPER".to_string()),
                ..resolve_one("{app_path}", roots).guard
            },
            ..planned_tree("{app_path}", roots)
        };
        assert_eq!(
            check_administrator_trash_move(&roots, &tree(&roots)),
            Ok(app.to_str().unwrap().to_string())
        );

        // A portable REAPER's app sits in the resource folder: the guard
        // allows it there, but root never moves it.
        let portable = roots.resource.join("REAPER.app");
        fs::create_dir_all(&portable).unwrap();
        roots.app_path = Some(portable);
        assert_eq!(
            check_administrator_trash_move(&roots, &tree(&roots)),
            Err(PathRefusal::NotRootControlled)
        );

        // /Applications itself a link.
        let real = dir.path().join("real Applications");
        fs::create_dir_all(real.join("REAPER.app")).unwrap();
        fs::remove_dir_all(&applications).unwrap();
        link_folder(&applications, &real);
        roots.app_path = Some(app);
        assert_eq!(
            check_administrator_trash_move(&roots, &tree(&roots)),
            Err(PathRefusal::LinkedAncestor)
        );
        assert!(real.join("REAPER.app").is_dir(), "checking removes nothing");
    }

    /// `path` with a separator added at its end, as shell completion types
    /// a folder.
    fn with_trailing_separator(path: &Path) -> PathBuf {
        let mut text = path.as_os_str().to_os_string();
        text.push(std::path::MAIN_SEPARATOR_STR);
        PathBuf::from(text)
    }

    /// A trailing slash makes `lstat` and `rename` follow a link in the last
    /// place on macOS, so a macOS target's app path loses it, and root is
    /// never handed a path spelt with one.
    #[test]
    fn a_trailing_slash_never_reaches_root() {
        let (dir, mut roots) = system_fixture();
        let applications = roots.system_root.join("Applications");
        let app = applications.join("REAPER.app");
        fs::create_dir_all(app.join("Contents")).unwrap();

        let target = UninstallRoots::for_target(
            Platform::MacOs,
            &roots.resource,
            Some(&with_trailing_separator(&app)),
        );
        let normalized = target.app_path.unwrap();
        assert_eq!(normalized.as_os_str(), app.as_os_str());

        let tree = |roots: &UninstallRoots| PlannedRemoval {
            guard: RemovalGuard {
                must_end_with: Some(".app".to_string()),
                name_contains: Some("REAPER".to_string()),
                ..resolve_one("{app_path}", roots).guard
            },
            ..planned_tree("{app_path}", roots)
        };
        // Spelt with the slash, even a real folder is refused.
        roots.app_path = Some(with_trailing_separator(&app));
        assert_eq!(
            check_administrator_trash_move(&roots, &tree(&roots)),
            Err(PathRefusal::NotPlain)
        );

        // REAPER.app a link to an app elsewhere: refused as a link where
        // `lstat` sees one, and for its spelling where the slash hides it.
        let elsewhere = dir.path().join("elsewhere").join("REAPER.app");
        fs::create_dir_all(elsewhere.join("Contents")).unwrap();
        fs::remove_dir_all(&app).unwrap();
        link_folder(&app, &elsewhere);
        let refusal = check_administrator_trash_move(&roots, &tree(&roots));
        assert!(
            matches!(refusal, Err(PathRefusal::Link | PathRefusal::NotPlain)),
            "{refusal:?}"
        );
        assert!(
            elsewhere.join("Contents").is_dir(),
            "checking moves nothing"
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
        // The 32-bit Program Files folder is a root of its own.
        let x86 = dir.path().join("Program Files (x86)");
        fs::create_dir_all(&x86).unwrap();
        roots.program_files_x86 = Some(x86.clone());
        assert_eq!(
            check_removal(&roots, &x86, &through_appdata),
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

    /// A `vendor_added_files` pattern keeps its components as written, and
    /// its guard stops at the last plain folder before the first `*`: for
    /// the JAWS scripts, the vendor's JAWS folder.
    #[test]
    fn a_vendor_pattern_is_guarded_by_its_last_plain_folder() {
        let (dir, mut roots) = fixture();
        roots.appdata = Some(dir.path().to_path_buf());
        let pattern = UninstallPath::new("{appdata}/Freedom Scientific/JAWS/*/Settings/*/*");
        let resolved = resolve_pattern(&pattern, &roots).unwrap();
        assert_eq!(
            resolved,
            vec![ResolvedPattern {
                base: dir.path().to_path_buf(),
                components: ["Freedom Scientific", "JAWS", "*", "Settings", "*", "*"]
                    .map(str::to_string)
                    .to_vec(),
                guard: RemovalGuard::under(dir.path().join("Freedom Scientific").join("JAWS")),
            }]
        );

        // With no `*` at all the guard is the file's own folder, never the
        // file itself.
        let plain = resolve_pattern(&UninstallPath::new("{appdata}/Vendor/data.ini"), &roots)
            .unwrap()
            .remove(0);
        assert_eq!(plain.guard, RemovalGuard::under(dir.path().join("Vendor")));
    }

    /// A token RABBIT can't place refuses the pattern, as it refuses any
    /// manifest path; so does a root that isn't a plain folder root.
    #[test]
    fn a_vendor_pattern_needs_a_folder_root_rabbit_can_place() {
        let (_dir, roots) = fixture();
        assert_eq!(
            resolve_pattern(
                &UninstallPath::new("{appdata}/Freedom Scientific/JAWS/*/Settings/*/*"),
                &roots
            ),
            Err(PathRefusal::UnknownRoot {
                token: "{appdata}".to_string()
            })
        );
        for refused in [
            "{app_path}",
            "{app_dir}/Vendor/*",
            "/Library/Application Support/Vendor/*",
        ] {
            assert_eq!(
                resolve_pattern(&UninstallPath::new(refused), &roots),
                Err(PathRefusal::OutsideRoot),
                "{refused}"
            );
        }
        assert_eq!(
            resolve_pattern(&UninstallPath::new("{resource}"), &roots),
            Err(PathRefusal::FolderRoot)
        );
    }

    /// A path matches only with one name per component of the pattern
    /// below its base: the base and plain names compared ignoring case,
    /// `*` standing for any name. A relative path, one with `..`, and a
    /// name holding `:`, which on Windows names a stream of another file,
    /// never match.
    #[test]
    fn a_vendor_pattern_matches_one_name_per_component() {
        let (dir, mut roots) = fixture();
        roots.appdata = Some(dir.path().to_path_buf());
        let pattern = resolve_pattern(
            &UninstallPath::new("{appdata}/Freedom Scientific/JAWS/*/Settings/*/reaper*"),
            &roots,
        )
        .unwrap()
        .remove(0);
        let below = |parts: &[&str]| {
            parts
                .iter()
                .fold(dir.path().to_path_buf(), |path, part| path.join(part))
        };

        for matching in [
            below(&[
                "Freedom Scientific",
                "JAWS",
                "2025",
                "Settings",
                "enu",
                "reaper.jss",
            ]),
            below(&[
                "FREEDOM SCIENTIFIC",
                "jaws",
                "2026",
                "settings",
                "deu",
                "REAPER.JKM",
            ]),
        ] {
            assert!(
                pattern_matches(&pattern, &matching),
                "{}",
                matching.display()
            );
        }
        let shouted_base = PathBuf::from(dir.path().to_string_lossy().to_uppercase())
            .join("Freedom Scientific/JAWS/2025/Settings/enu/reaper.jss");
        assert!(pattern_matches(&pattern, &shouted_base));

        for other in [
            // Too shallow, too deep.
            below(&[
                "Freedom Scientific",
                "JAWS",
                "2025",
                "Settings",
                "reaper.ini",
            ]),
            below(&[
                "Freedom Scientific",
                "JAWS",
                "2025",
                "Settings",
                "enu",
                "x",
                "reaper.jss",
            ]),
            // A plain name or the file's pattern that doesn't match.
            below(&[
                "Freedom Scientific",
                "JAWS",
                "2025",
                "Other",
                "enu",
                "reaper.jss",
            ]),
            below(&[
                "Freedom Scientific",
                "JAWS",
                "2025",
                "Settings",
                "enu",
                "default.jcf",
            ]),
            // A stream of a file that would match.
            below(&[
                "Freedom Scientific",
                "JAWS",
                "2025",
                "Settings",
                "enu",
                "reaper.jss:x",
            ]),
            // Through `..`, and relative.
            below(&[
                "Freedom Scientific",
                "JAWS",
                "..",
                "JAWS",
                "Settings",
                "enu",
                "reaper.jss",
            ]),
            PathBuf::from("Freedom Scientific/JAWS/2025/Settings/enu/reaper.jss"),
            // As deep, but below a folder beside the base.
            dir.path()
                .with_file_name("elsewhere")
                .join("Freedom Scientific/JAWS/2025/Settings/enu/reaper.jss"),
        ] {
            assert!(!pattern_matches(&pattern, &other), "{}", other.display());
        }
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
