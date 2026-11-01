//! The `uninstall` block every package manifest carries: what removing the
//! package means on each platform and REAPER target.
//!
//! Facts about a package live in its manifest, and that includes how it comes
//! off again: which files are RABBIT's to remove, what always stays, whether a
//! vendor uninstaller does the work, or why RABBIT can't do it at all. One
//! generic engine carries these plans out, in [`crate::uninstall`]. Packages
//! whose turn hasn't come yet say `not_supported`.
//!
//! The rules here run from [`super::validate_package_spec`] when the embedded
//! manifest is loaded, like the rest of the manifest's validation, so a plan
//! that couldn't be carried out safely fails RABBIT's own tests rather than a
//! user's uninstall.

use serde::{Deserialize, Serialize};

use super::{EmbeddedPackageSpec, SupportedPlatform};
use crate::model::{InstallationKind, Platform};

/// A package's `uninstall` block. Required on every manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// A misspelt field must fail loudly here rather than vanish: a `keep` list
// typed as `kep` would otherwise be dropped, and the files it protects with it.
#[serde(deny_unknown_fields)]
pub struct UninstallSpec {
    /// Uninstall only copies RABBIT has a receipt for. Set where a copy
    /// without one is most likely the user's own (FFmpeg, language packs).
    #[serde(default)]
    pub requires_receipt: bool,
    /// What happens when ReaPack's records claim a file this package would
    /// remove.
    #[serde(default)]
    pub reapack_owned: ReapackOwnedPolicy,
    /// Warnings shown when another package is installed, such as SWS's note
    /// that keys in OSARA's key map call SWS actions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warn_when_installed: Vec<InstalledPackageWarning>,
    /// Warnings shown whenever this package is about to be uninstalled, such
    /// as OSARA's warning that REAPER stops speaking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always_warn: Vec<AlwaysWarning>,
    /// Folders the package's installer writes to that no receipt path
    /// covers, such as the JAWS scripts' files in JAWS's own settings
    /// folders. RABBIT lists them right before it runs the installer and
    /// again afterwards, and records every file the installer added or
    /// changed there on the receipt (`vendor_added_files`), so a later
    /// uninstall knows which of them came from the package.
    ///
    /// Written like plan paths, except that `*` may stand for a whole folder
    /// name at any level (`{appdata}/Freedom Scientific/JAWS/*/Settings/*`
    /// covers every JAWS version and language). A matched folder is listed
    /// with everything below it. These paths are only ever read, never
    /// removed. The manifest's validation keeps them narrow: a folder token,
    /// then a folder spelt out, and nothing called RABBIT.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub install_snapshot: Vec<UninstallPath>,
    /// The plans, in order. The first one matching the platform and target
    /// wins, the same rule `http_artifact.targets` uses.
    pub plans: Vec<UninstallPlanSpec>,
}

impl UninstallSpec {
    /// The plan that applies on `platform` to a `target` REAPER, if any.
    pub fn plan_for(
        &self,
        platform: Platform,
        target: InstallationKind,
    ) -> Option<&UninstallPlanSpec> {
        self.plans
            .iter()
            .find(|plan| plan.applies_to(platform, target))
    }

    /// Every Fluent key this block names, for the test that checks each one
    /// exists.
    pub fn fluent_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = Vec::new();
        keys.extend(
            self.warn_when_installed
                .iter()
                .map(|warning| warning.message_key.as_str()),
        );
        keys.extend(
            self.always_warn
                .iter()
                .map(|warning| warning.message_key.as_str()),
        );
        for plan in &self.plans {
            if let Some(summary) = &plan.summary {
                keys.push(&summary.removes_key);
                keys.push(&summary.keeps_key);
            }
            keys.extend(
                plan.kept_user_data
                    .iter()
                    .map(|kept| kept.note_key.as_str()),
            );
            keys.extend(plan.note_key.as_deref());
        }
        keys
    }
}

/// What RABBIT does when ReaPack's records claim one of the files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReapackOwnedPolicy {
    /// Refuse the whole package and point to ReaPack's own uninstall, so
    /// ReaPack's records never go out of date behind its back.
    #[default]
    Block,
    /// ReaPack itself, which lists its own file in its records.
    Ignore,
}

/// A warning shown while another package is installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledPackageWarning {
    /// The other package's id.
    pub package: String,
    pub message_key: String,
}

/// A warning shown whenever the package is about to be uninstalled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlwaysWarning {
    pub message_key: String,
    /// Also ask a Yes/No question, with No focused, before starting.
    #[serde(default)]
    pub confirm: bool,
}

/// One plan: how the package comes off on one platform and target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UninstallPlanSpec {
    /// `None` means every platform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<SupportedPlatform>,
    #[serde(default)]
    pub target: UninstallTarget,
    pub strategy: UninstallStrategy,
    #[serde(default)]
    pub elevation: UninstallElevation,
    #[serde(default)]
    pub backup: UninstallBackup,
    /// The two sentences Review shows. Required for plans that remove
    /// something.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<UninstallSummary>,
    #[serde(default, skip_serializing_if = "UninstallRemoveSpec::is_empty")]
    pub remove: UninstallRemoveSpec,
    /// Paths never deleted, even when a rule in `remove` matches them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep: Vec<UninstallPath>,
    /// What stays behind, shown as kept in the details text and the report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kept_user_data: Vec<KeptUserData>,
    /// Program names that must not be running. The engine adds "no REAPER at
    /// all" by itself where it is needed, so a manifest can't forget it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking_programs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_uninstaller: Option<VendorUninstallerSpec>,
    /// HKCU uninstall keys a removal leaves pointing at nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registry_cleanup: Vec<RegistryCleanup>,
    /// macOS installer package ids for `pkgutil --forget`, spelt out in full.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macos_pkg_forget: Vec<String>,
    /// Named helpers that run after the files are gone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post: Vec<UninstallPostStep>,
    /// Why RABBIT can't uninstall the package, or how to do it by hand.
    /// Required for `manual` and `not_supported`; on other plans, a note on
    /// what stays behind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_key: Option<String>,
}

impl UninstallPlanSpec {
    /// Whether this plan matches `platform` and a `target` REAPER.
    pub fn applies_to(&self, platform: Platform, target: InstallationKind) -> bool {
        self.platform
            .is_none_or(|own| own.matches_platform(platform))
            && self.target.matches(target)
    }

    /// True when the plan holds nothing beyond where it applies, its
    /// strategy and its note: the shape of a `manual` or `not_supported`
    /// plan, which changes nothing.
    fn changes_nothing(&self) -> bool {
        self.elevation == UninstallElevation::Never
            && self.backup == UninstallBackup::Move
            && self.summary.is_none()
            && self.remove.is_empty()
            && self.keep.is_empty()
            && self.kept_user_data.is_empty()
            && self.blocking_programs.is_empty()
            && self.vendor_uninstaller.is_none()
            && self.registry_cleanup.is_empty()
            && self.macos_pkg_forget.is_empty()
            && self.post.is_empty()
    }
}

/// Which REAPER targets a plan covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UninstallTarget {
    #[default]
    Any,
    Standard,
    Portable,
}

impl UninstallTarget {
    pub fn matches(self, target: InstallationKind) -> bool {
        match self {
            Self::Any => true,
            Self::Standard => target == InstallationKind::Standard,
            Self::Portable => target == InstallationKind::Portable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UninstallStrategy {
    /// RABBIT moves the package's own files into a backup set (or, for
    /// `trees`, to the Trash or away for good, as `backup` says).
    RemoveFiles,
    /// RABBIT runs the vendor's uninstaller silently and checks the result.
    VendorUninstaller,
    /// RABBIT changes nothing; `note_key` says how to remove it by hand.
    Manual,
    /// RABBIT changes nothing; `note_key` says why. Also marks packages whose
    /// turn hasn't come yet.
    NotSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UninstallElevation {
    #[default]
    Never,
    Required,
    /// Only for `trees` with `backup: to_trash`: the object goes into the
    /// run's administrator command only when RABBIT can't move it itself.
    IfNeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UninstallBackup {
    /// Into a backup set under `RABBIT/backups`, so a restore can undo it.
    #[default]
    Move,
    ToTrash,
    /// Gone for good. Review says a backup can't bring it back.
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UninstallSummary {
    pub removes_key: String,
    pub keeps_key: String,
}

/// The files a `remove_files` plan takes away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UninstallRemoveSpec {
    /// Receipt entries that have a hash, meaning regular files RABBIT wrote.
    #[serde(default)]
    pub receipt_files: bool,
    /// Files in the install folder matching the package's
    /// `user_plugin_prefixes` and `user_plugin_suffixes`.
    #[serde(default)]
    pub matching_user_plugins: bool,
    /// Files. `*` is allowed in the last component only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<UninstallPath>,
    /// Folders removed after the files, and only when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirs_if_empty: Vec<UninstallPath>,
    /// Whole folders such as macOS bundles, removed in this order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trees: Vec<UninstallTree>,
}

impl UninstallRemoveSpec {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A whole folder to remove. The name checks guard against a token or path
/// that resolves somewhere unexpected: at least one of them must be given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UninstallTree {
    pub path: UninstallPath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_end_with: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_contains: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeptUserData {
    pub path: UninstallPath,
    pub note_key: String,
}

/// How to find, trust and run a vendor's own uninstaller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VendorUninstallerSpec {
    pub kind: VendorUninstallerKind,
    /// Where the uninstaller is. The first entry that exists wins.
    pub locate: Vec<UninstallerLocation>,
    /// The install folder the registry records and what it must equal.
    /// Required for `nsis`, whose `_?=` argument names the folder the
    /// uninstaller deletes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_location: Option<InstallLocationCheck>,
    /// Where the uninstaller must sit before RABBIT runs it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_be_in: Option<UninstallerFolder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<String>,
    /// After this long the package fails as "still open" and the run stops.
    pub timeout_seconds: u64,
    /// After this long the progress page says RABBIT is still waiting.
    #[serde(default = "default_still_waiting_seconds")]
    pub still_waiting_seconds: u64,
    /// Paths that must be gone afterwards. Only tested for existence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verify_gone: Vec<UninstallPath>,
    /// Paths reported, never removed, when they are still there afterwards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub report_leftovers: Vec<UninstallPath>,
}

fn default_still_waiting_seconds() -> u64 {
    30
}

/// The kind picks the engine's behaviour: for NSIS a temporary copy, `_?=`
/// and NSIS exit codes; for Inno running in place, Inno exit codes and
/// polling until the files are gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VendorUninstallerKind {
    Nsis,
    Inno,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum UninstallerLocation {
    Registry(UninstallRegistryValue),
    Path(UninstallPath),
}

/// A value under one of Windows' `...\CurrentVersion\Uninstall` keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UninstallRegistryValue {
    pub hive: RegistryHive,
    /// One key name under the Uninstall key, such as `REAPER`.
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryHive {
    Hklm,
    Hkcu,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallLocationCheck {
    pub registry: UninstallRegistryValue,
    pub must_equal: UninstallPath,
}

/// Where a vendor uninstaller must sit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UninstallerFolder {
    Exact(ExactUninstallerFolder),
    Under(UninstallerFolderRoot),
}

/// Exactly this vendor folder, holding nothing but files matching
/// `only_files`. The content check matters because standard users can
/// usually create files in ProgramData subfolders, so a planted DLL next to
/// the uninstaller would run with administrator rights.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactUninstallerFolder {
    pub dir: UninstallPath,
    pub only_files: String,
}

/// Anywhere below this folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UninstallerFolderRoot {
    pub under: UninstallPath,
}

/// Remove an HKCU uninstall key, but only while it still points into a
/// folder this package owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryCleanup {
    pub hive: RegistryHive,
    /// One key name under the Uninstall key, such as `OSARA`.
    pub key: String,
    pub only_if_value_in: RegistryValueInFolder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryValueInFolder {
    pub value: String,
    pub dir: UninstallPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum UninstallPostStep {
    /// Set REAPER's `langpack=` back to `<>` when it names a pack file this
    /// run actually removed.
    ResetReaperLangpackIfSelected {},
}

/// A path in an uninstall plan, kept exactly as the manifest writes it: a
/// token such as `{resource}` followed by `/`-separated components, or, in
/// macOS plans, an absolute path below one of a few fixed folders. The
/// manifest's validation checks it when it loads; the uninstall engine
/// resolves it when it runs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UninstallPath(String);

impl UninstallPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Split the path into its root and components, refusing anything that
    /// could step outside the root or be read two ways: `.` and `..`, empty
    /// components, backslashes, drive or stream colons, stray braces, and a
    /// `*` anywhere but the last component.
    pub fn parse(&self) -> Result<ParsedUninstallPath<'_>, String> {
        self.parse_with(false)
    }

    /// [`parse`](Self::parse) for an `install_snapshot` root, which RABBIT
    /// only ever lists and never removes. There a `*` may appear in any
    /// component, and may be the whole component, because the JAWS version
    /// and language folders can be called anything. Everything else is
    /// refused exactly as `parse` refuses it.
    pub fn parse_pattern(&self) -> Result<ParsedUninstallPath<'_>, String> {
        self.parse_with(true)
    }

    fn parse_with(&self, wildcards_anywhere: bool) -> Result<ParsedUninstallPath<'_>, String> {
        let path = self.0.as_str();
        let (root, rest) = if let Some(after_brace) = path.strip_prefix('{') {
            let (name, rest) = after_brace
                .split_once('}')
                .ok_or_else(|| format!("{path:?} opens a token but never closes it"))?;
            let token = UninstallPathToken::from_name(name)
                .ok_or_else(|| format!("{path:?} uses the unknown token {{{name}}}"))?;
            (UninstallPathRoot::Token(token), rest)
        } else if path.starts_with('/') {
            (UninstallPathRoot::MacosAbsolute, path)
        } else {
            return Err(format!(
                "{path:?} must start with a token such as {{resource}}, or be an absolute macOS path"
            ));
        };
        let components: Vec<&str> = if rest.is_empty() {
            Vec::new()
        } else {
            rest.strip_prefix('/')
                .ok_or_else(|| format!("{path:?} needs a / after its token"))?
                .split('/')
                .collect()
        };
        let last = components.len().saturating_sub(1);
        for (index, component) in components.iter().enumerate() {
            if component.is_empty() {
                return Err(format!("{path:?} has an empty component"));
            }
            if *component == "." || *component == ".." {
                return Err(format!("{path:?} may not use . or .."));
            }
            if let Some(bad) = component
                .chars()
                .find(|ch| matches!(ch, '\\' | ':' | '{' | '}') || ch.is_control())
            {
                return Err(format!("{path:?} may not contain {bad:?}"));
            }
            if component.contains('*') && !wildcards_anywhere {
                if index != last {
                    return Err(format!(
                        "{path:?} uses * before its last component; * is allowed only there"
                    ));
                }
                if component.chars().all(|ch| ch == '*') {
                    return Err(format!(
                        "{path:?} ends in a bare *, which would match everything in that folder"
                    ));
                }
            }
        }
        Ok(ParsedUninstallPath { root, components })
    }
}

/// An [`UninstallPath`] split into its root and the components after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUninstallPath<'a> {
    pub root: UninstallPathRoot,
    /// For an absolute macOS path, every component after the leading `/`.
    pub components: Vec<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallPathRoot {
    Token(UninstallPathToken),
    MacosAbsolute,
}

/// The tokens a manifest path can start with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallPathToken {
    /// The target's resource folder.
    Resource,
    /// REAPER's app folder, from detection or the receipt.
    AppDir,
    /// REAPER's app itself: `reaper.exe` on Windows, the `.app` bundle on
    /// macOS.
    AppPath,
    Appdata,
    Localappdata,
    /// The per-user CLAP folder on Windows.
    ClapDir,
    CommonProgramFiles,
    ProgramFiles,
    ProgramData,
    Home,
    Documents,
}

/// How a token may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallTokenKind {
    /// A folder a path can be built on. The root itself is never removed.
    FolderRoot,
    /// A folder to compare against, never a root to remove from. `{app_dir}`
    /// can come from HKCU or the command line, so comparing it with itself
    /// would prove nothing.
    ComparisonRoot,
    /// The thing being removed. Allowed only alone, in `trees` and
    /// `verify_gone`.
    Object,
}

impl UninstallPathToken {
    pub const ALL: [Self; 11] = [
        Self::Resource,
        Self::AppDir,
        Self::AppPath,
        Self::Appdata,
        Self::Localappdata,
        Self::ClapDir,
        Self::CommonProgramFiles,
        Self::ProgramFiles,
        Self::ProgramData,
        Self::Home,
        Self::Documents,
    ];

    /// The name between the braces.
    pub fn name(self) -> &'static str {
        match self {
            Self::Resource => "resource",
            Self::AppDir => "app_dir",
            Self::AppPath => "app_path",
            Self::Appdata => "appdata",
            Self::Localappdata => "localappdata",
            Self::ClapDir => "clap_dir",
            Self::CommonProgramFiles => "common_program_files",
            Self::ProgramFiles => "program_files",
            Self::ProgramData => "program_data",
            Self::Home => "home",
            Self::Documents => "documents",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|token| token.name() == name)
    }

    pub fn kind(self) -> UninstallTokenKind {
        match self {
            Self::AppDir => UninstallTokenKind::ComparisonRoot,
            Self::AppPath => UninstallTokenKind::Object,
            _ => UninstallTokenKind::FolderRoot,
        }
    }

    /// The only platform the token resolves on, or `None` for both. The
    /// Windows folders come from Windows environment variables, and `{home}`
    /// from `$HOME`, which Windows doesn't set.
    pub fn platform(self) -> Option<SupportedPlatform> {
        match self {
            Self::Appdata
            | Self::Localappdata
            | Self::ClapDir
            | Self::CommonProgramFiles
            | Self::ProgramFiles
            | Self::ProgramData => Some(SupportedPlatform::Windows),
            Self::Home => Some(SupportedPlatform::Macos),
            Self::Resource | Self::AppDir | Self::AppPath | Self::Documents => None,
        }
    }
}

/// The folders absolute macOS paths may sit below, as path components. A
/// path must add at least one component after one of them.
pub(crate) const MACOS_ABSOLUTE_PREFIXES: &[&[&str]] = &[
    &["Library", "Audio", "Plug-Ins", "VST3"],
    &["Library", "Audio", "Plug-Ins", "Components"],
    &["Library", "Audio", "Plug-Ins", "CLAP"],
    &["Library", "Audio", "Plug-Ins", "LV2"],
    &["Applications"],
    &["Library", "Application Support"],
];

/// RABBIT's own folder inside the resource folder. It holds receipts,
/// backups, logs and locks, and no uninstall may ever touch it.
pub(crate) const RABBIT_FOLDER: &str = "RABBIT";

/// How a field uses its paths, which decides what they may look like.
#[derive(Debug, Clone, Copy)]
struct PathUse {
    field: &'static str,
    /// Removes what it names, so a bare folder root and `{app_dir}` are
    /// refused.
    removes: bool,
    /// May be an object token such as `{app_path}`.
    object: bool,
    /// May end in a `*` pattern.
    wildcard: bool,
}

impl PathUse {
    const fn new(field: &'static str, removes: bool, object: bool, wildcard: bool) -> Self {
        Self {
            field,
            removes,
            object,
            wildcard,
        }
    }
}

const REMOVE_PATHS: PathUse = PathUse::new("remove.paths", true, false, true);
const DIRS_IF_EMPTY: PathUse = PathUse::new("remove.dirs_if_empty", true, false, false);
const TREES: PathUse = PathUse::new("remove.trees", true, true, false);
const KEEP: PathUse = PathUse::new("keep", false, false, true);
const KEPT_USER_DATA: PathUse = PathUse::new("kept_user_data", false, false, false);
const VERIFY_GONE: PathUse = PathUse::new("verify_gone", false, true, false);
const REPORT_LEFTOVERS: PathUse = PathUse::new("report_leftovers", false, false, true);
const LOCATE: PathUse = PathUse::new("locate", false, false, false);
const MUST_EQUAL: PathUse = PathUse::new("install_location.must_equal", false, false, false);
const MUST_BE_IN: PathUse = PathUse::new("must_be_in", false, false, false);
const ONLY_IF_VALUE_IN: PathUse =
    PathUse::new("registry_cleanup.only_if_value_in", false, false, false);

/// The load-time rules for a package's uninstall block. Called from
/// [`super::validate_package_spec`], so a violation panics when the embedded
/// manifest loads, like every other manifest mistake.
pub(super) fn validate_uninstall_spec(spec: &EmbeddedPackageSpec) -> Result<(), String> {
    let uninstall = &spec.uninstall;
    if uninstall
        .warn_when_installed
        .iter()
        .any(|warning| warning.package == spec.id)
    {
        return Err("uninstall warn_when_installed names the package itself".to_string());
    }
    for root in &uninstall.install_snapshot {
        check_snapshot_root(spec, root)
            .map_err(|message| format!("uninstall install_snapshot: {message}"))?;
    }
    for (index, plan) in uninstall.plans.iter().enumerate() {
        validate_plan(spec, plan)
            .map_err(|message| format!("uninstall plan #{index}: {message}"))?;
    }
    validate_plan_coverage(spec)
}

/// Every supported platform, with both a standard and a portable REAPER, must
/// reach a plan, and every plan must be reached. Portable targets are exempt
/// for packages that need a standard REAPER, because RABBIT never offers them
/// there. An unreached plan is a mistake: the first matching plan wins, so a
/// plan below a broader one would silently never apply.
fn validate_plan_coverage(spec: &EmbeddedPackageSpec) -> Result<(), String> {
    let plans = &spec.uninstall.plans;
    let mut reached = vec![false; plans.len()];
    for supported in &spec.supported_platforms {
        for target in [InstallationKind::Standard, InstallationKind::Portable] {
            if target == InstallationKind::Portable && spec.requires_standard_install {
                continue;
            }
            let platform = supported.platform();
            match plans
                .iter()
                .position(|plan| plan.applies_to(platform, target))
            {
                Some(index) => reached[index] = true,
                None => {
                    return Err(format!(
                        "uninstall has no plan for {} with a {} REAPER",
                        platform_name(*supported),
                        target_name(target)
                    ));
                }
            }
        }
    }
    match reached.iter().position(|reached| !reached) {
        Some(index) => Err(format!(
            "uninstall plan #{index} never applies: earlier plans already cover everything it \
             matches, or it matches no platform and target the package is offered on"
        )),
        None => Ok(()),
    }
}

fn validate_plan(spec: &EmbeddedPackageSpec, plan: &UninstallPlanSpec) -> Result<(), String> {
    let strategy = strategy_name(plan.strategy);
    match plan.strategy {
        UninstallStrategy::Manual | UninstallStrategy::NotSupported => {
            if plan.note_key.is_none() {
                return Err(format!("a {strategy} plan needs a note_key saying why"));
            }
            if !plan.changes_nothing() {
                return Err(format!(
                    "a {strategy} plan changes nothing, so it carries only platform, target and \
                     note_key"
                ));
            }
            return Ok(());
        }
        UninstallStrategy::RemoveFiles => {
            if plan.vendor_uninstaller.is_some() {
                return Err(
                    "vendor_uninstaller belongs only in a vendor_uninstaller plan".to_string(),
                );
            }
            let remove = &plan.remove;
            if !remove.receipt_files
                && !remove.matching_user_plugins
                && remove.paths.is_empty()
                && remove.trees.is_empty()
            {
                return Err(
                    "a remove_files plan needs something to remove: receipt_files, \
                     matching_user_plugins, paths or trees"
                        .to_string(),
                );
            }
        }
        UninstallStrategy::VendorUninstaller => {
            let Some(vendor) = &plan.vendor_uninstaller else {
                return Err(
                    "a vendor_uninstaller plan needs a vendor_uninstaller block".to_string()
                );
            };
            // NSIS and Inno are Windows installers; no macOS package has a
            // vendor uninstaller.
            if plan.platform != Some(SupportedPlatform::Windows) {
                return Err("a vendor_uninstaller plan needs \"platform\": \"windows\"".to_string());
            }
            if plan.remove.receipt_files {
                return Err("a vendor_uninstaller plan may not use receipt_files".to_string());
            }
            validate_vendor_uninstaller(plan, vendor)?;
        }
    }

    if plan.summary.is_none() {
        return Err(format!(
            "a {strategy} plan needs summary keys for the Review page"
        ));
    }
    validate_elevation_and_backup(plan)?;

    // The scan matches a file when ANY prefix starts its name, and "" starts
    // every name, so one empty prefix among good ones claims every file with
    // the right suffix just as an all-empty list would.
    if plan.remove.matching_user_plugins
        && (spec.user_plugin_prefixes.is_empty()
            || spec
                .user_plugin_prefixes
                .iter()
                .any(|prefix| prefix.is_empty()))
    {
        return Err(
            "matching_user_plugins needs non-empty user_plugin_prefixes; an empty prefix \
             would match every file with the right suffix"
                .to_string(),
        );
    }
    for tree in &plan.remove.trees {
        let has_check = |check: &Option<String>| check.as_deref().is_some_and(|c| !c.is_empty());
        if !has_check(&tree.must_end_with) && !has_check(&tree.name_contains) {
            return Err(format!(
                "remove.trees entry {:?} needs must_end_with or name_contains",
                tree.path.as_str()
            ));
        }
    }
    for cleanup in &plan.registry_cleanup {
        if plan.platform != Some(SupportedPlatform::Windows) {
            return Err("registry_cleanup needs \"platform\": \"windows\"".to_string());
        }
        if cleanup.hive != RegistryHive::Hkcu {
            return Err("registry_cleanup may only remove HKCU keys".to_string());
        }
        check_registry_key("registry_cleanup", &cleanup.key)?;
        if cleanup.only_if_value_in.value.is_empty() {
            return Err("registry_cleanup.only_if_value_in needs a value name".to_string());
        }
    }
    if !plan.macos_pkg_forget.is_empty() && plan.platform != Some(SupportedPlatform::Macos) {
        return Err("macos_pkg_forget needs \"platform\": \"macos\"".to_string());
    }
    for id in &plan.macos_pkg_forget {
        // These end up as words in a root shell command. Quoting makes any
        // text safe there, but a real package id never needs more than this.
        if id.is_empty()
            || !id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        {
            return Err(format!(
                "macos_pkg_forget id {id:?} must be a full package id such as \
                 org.example.app.pkg"
            ));
        }
    }
    for program in &plan.blocking_programs {
        if program.trim().is_empty() || program.contains(['/', '\\']) {
            return Err(format!(
                "blocking_programs entry {program:?} must be a program name, not a path"
            ));
        }
    }

    for path in &plan.remove.paths {
        check_path(plan, path, REMOVE_PATHS)?;
    }
    for path in &plan.remove.dirs_if_empty {
        check_path(plan, path, DIRS_IF_EMPTY)?;
    }
    for tree in &plan.remove.trees {
        check_path(plan, &tree.path, TREES)?;
    }
    for path in &plan.keep {
        check_path(plan, path, KEEP)?;
    }
    for kept in &plan.kept_user_data {
        check_path(plan, &kept.path, KEPT_USER_DATA)?;
    }
    for cleanup in &plan.registry_cleanup {
        check_path(plan, &cleanup.only_if_value_in.dir, ONLY_IF_VALUE_IN)?;
    }
    Ok(())
}

/// Removals that need administrator rights never move root-owned files into
/// the user's backups, and whole folders never go into a backup set at all.
fn validate_elevation_and_backup(plan: &UninstallPlanSpec) -> Result<(), String> {
    let has_trees = !plan.remove.trees.is_empty();
    match plan.elevation {
        UninstallElevation::Required if plan.backup == UninstallBackup::Move => {
            return Err(
                "elevation required needs backup none or to_trash: RABBIT never moves \
                 files it needs administrator rights for into the user's backups"
                    .to_string(),
            );
        }
        UninstallElevation::IfNeeded if !has_trees || plan.backup != UninstallBackup::ToTrash => {
            return Err("elevation if_needed is only for trees with backup to_trash".to_string());
        }
        _ => {}
    }
    if has_trees && plan.backup == UninstallBackup::Move {
        return Err(
            "remove.trees can't be moved into a backup set; use backup to_trash or none"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_vendor_uninstaller(
    plan: &UninstallPlanSpec,
    vendor: &VendorUninstallerSpec,
) -> Result<(), String> {
    if vendor.locate.is_empty() {
        return Err("vendor_uninstaller needs at least one locate entry".to_string());
    }
    if vendor.timeout_seconds == 0 {
        return Err("vendor_uninstaller.timeout_seconds must be above zero".to_string());
    }
    for location in &vendor.locate {
        match location {
            UninstallerLocation::Registry(value) => check_registry_value("locate", value)?,
            UninstallerLocation::Path(path) => check_path(plan, path, LOCATE)?,
        }
    }
    if let Some(check) = &vendor.install_location {
        check_registry_value("install_location", &check.registry)?;
        check_path(plan, &check.must_equal, MUST_EQUAL)?;
    }
    match &vendor.must_be_in {
        Some(UninstallerFolder::Exact(folder)) => {
            check_path(plan, &folder.dir, MUST_BE_IN)?;
            if folder.only_files.contains(['/', '\\'])
                || folder.only_files.is_empty()
                || folder.only_files.chars().all(|ch| ch == '*')
            {
                return Err(format!(
                    "must_be_in.only_files {:?} must be a file name pattern such as unins000.*",
                    folder.only_files
                ));
            }
        }
        Some(UninstallerFolder::Under(root)) => check_path(plan, &root.under, MUST_BE_IN)?,
        None => {}
    }
    for path in &vendor.verify_gone {
        check_path(plan, path, VERIFY_GONE)?;
    }
    for path in &vendor.report_leftovers {
        check_path(plan, path, REPORT_LEFTOVERS)?;
    }

    if vendor.kind == VendorUninstallerKind::Nsis {
        // `_?=` makes the install folder the one the uninstaller deletes
        // from, so it must be checked against the registry and must sit
        // strictly below a known root.
        if vendor.install_location.is_none() {
            return Err("an nsis uninstaller needs install_location".to_string());
        }
        if !matches!(vendor.must_be_in, Some(UninstallerFolder::Under(_))) {
            return Err(
                "an nsis uninstaller needs must_be_in with under: its install folder must sit \
                 below that root"
                    .to_string(),
            );
        }
    }
    if plan.elevation == UninstallElevation::Required {
        // Only HKLM is writable by administrators alone. A path, or an HKCU
        // value any process of the user's can write, could point the
        // administrator prompt at anything.
        if vendor.must_be_in.is_none() {
            return Err("an elevated vendor uninstaller needs must_be_in".to_string());
        }
        let hklm_only = vendor.locate.iter().all(|location| {
            matches!(location, UninstallerLocation::Registry(value) if value.hive == RegistryHive::Hklm)
        });
        if !hklm_only {
            return Err(
                "an elevated vendor uninstaller may only be located through HKLM registry \
                 values, never a path"
                    .to_string(),
            );
        }
        if vendor
            .install_location
            .as_ref()
            .is_some_and(|check| check.registry.hive != RegistryHive::Hklm)
        {
            return Err(
                "an elevated vendor uninstaller reads install_location from HKLM only".to_string(),
            );
        }
    }
    Ok(())
}

fn check_registry_value(field: &str, value: &UninstallRegistryValue) -> Result<(), String> {
    check_registry_key(field, &value.key)?;
    if value.value.is_empty() {
        return Err(format!("{field} needs a registry value name"));
    }
    Ok(())
}

/// Registry keys are named relative to Windows' Uninstall key, so a
/// separator could only ever point somewhere else.
fn check_registry_key(field: &str, key: &str) -> Result<(), String> {
    if key.is_empty() || key.contains(['\\', '/']) {
        return Err(format!(
            "{field}: registry key {key:?} must be one key name under the Uninstall key, \
             without separators"
        ));
    }
    Ok(())
}

fn check_path(
    plan: &UninstallPlanSpec,
    path: &UninstallPath,
    usage: PathUse,
) -> Result<(), String> {
    let field = usage.field;
    let shown = path.as_str();
    let parsed = path
        .parse()
        .map_err(|message| format!("{field}: {message}"))?;
    if !usage.wildcard && shown.contains('*') {
        return Err(format!(
            "{field}: {shown:?} may not use *; it must name exactly one thing"
        ));
    }
    match parsed.root {
        UninstallPathRoot::Token(token) => {
            if let Some(only_on) = token.platform()
                && plan.platform != Some(only_on)
            {
                return Err(format!(
                    "{field}: {{{}}} exists only on {}, so {shown:?} needs a plan with \
                     \"platform\": \"{}\"",
                    token.name(),
                    platform_name(only_on),
                    platform_name(only_on)
                ));
            }
            match token.kind() {
                UninstallTokenKind::Object => {
                    if !usage.object {
                        return Err(format!(
                            "{field}: {{{}}} names the object itself and is allowed only in \
                             remove.trees and verify_gone",
                            token.name()
                        ));
                    }
                    if !parsed.components.is_empty() {
                        return Err(format!(
                            "{field}: {{{}}} names the object itself, so {shown:?} can't add \
                             components after it",
                            token.name()
                        ));
                    }
                }
                UninstallTokenKind::ComparisonRoot if usage.removes => {
                    return Err(format!(
                        "{field}: {{{}}} is only for comparison, never a root to remove from",
                        token.name()
                    ));
                }
                UninstallTokenKind::FolderRoot | UninstallTokenKind::ComparisonRoot => {
                    if usage.removes && parsed.components.is_empty() {
                        return Err(format!(
                            "{field}: {shown:?} is a folder root, which is never removed; add at \
                             least one component"
                        ));
                    }
                }
            }
        }
        UninstallPathRoot::MacosAbsolute => {
            if plan.platform != Some(SupportedPlatform::Macos) {
                return Err(format!(
                    "{field}: absolute paths like {shown:?} are allowed only in plans with \
                     \"platform\": \"macos\""
                ));
            }
            let below_known_folder = MACOS_ABSOLUTE_PREFIXES.iter().any(|prefix| {
                parsed.components.len() > prefix.len() && parsed.components.starts_with(prefix)
            });
            if !below_known_folder {
                return Err(format!(
                    "{field}: {shown:?} must sit below /Applications, /Library/Application \
                     Support or a /Library/Audio/Plug-Ins format folder"
                ));
            }
        }
    }
    if names_rabbit_folder(&parsed) {
        return Err(format!(
            "{field}: {shown:?} reaches into RABBIT's own folder, which holds receipts, backups \
             and logs and is never touched"
        ));
    }
    Ok(())
}

/// The rules for an `install_snapshot` root. Nothing is ever removed through
/// one, but what the snapshot records is what a later uninstall may offer to
/// remove, so a root must stay narrow:
/// - it starts with a folder token, never an absolute path or an object
///   token, and the token must exist on a platform the package is offered
///   on;
/// - the first component after the token is spelt out, so a root never
///   covers everything in `%APPDATA%` or the resource folder;
/// - no component is called RABBIT, whose folder holds receipts, backups and
///   logs. The listing also skips `{resource}/RABBIT` wherever a `*` would
///   reach it.
fn check_snapshot_root(spec: &EmbeddedPackageSpec, root: &UninstallPath) -> Result<(), String> {
    let shown = root.as_str();
    let parsed = root.parse_pattern()?;
    let UninstallPathRoot::Token(token) = parsed.root else {
        return Err(format!(
            "{shown:?} must start with a folder token such as {{appdata}}"
        ));
    };
    if token.kind() != UninstallTokenKind::FolderRoot {
        return Err(format!(
            "{{{}}} isn't a folder to list, so {shown:?} can't start with it",
            token.name()
        ));
    }
    if let Some(only_on) = token.platform()
        && !spec.supported_platforms.contains(&only_on)
    {
        return Err(format!(
            "{{{}}} exists only on {}, where the package isn't offered, so {shown:?} would \
             never resolve",
            token.name(),
            platform_name(only_on)
        ));
    }
    match parsed.components.first() {
        Some(first) if !first.contains('*') => {}
        _ => {
            return Err(format!(
                "{shown:?} must spell out the folder right after {{{}}}, so it never lists \
                 everything there",
                token.name()
            ));
        }
    }
    if parsed
        .components
        .iter()
        .any(|component| component.eq_ignore_ascii_case(RABBIT_FOLDER))
    {
        return Err(format!(
            "{shown:?} reaches into RABBIT's own folder, which holds receipts, backups and logs"
        ));
    }
    Ok(())
}

/// Whether a path names `{resource}/RABBIT`, something under it, or a
/// pattern that could match it. Any component called RABBIT counts, and so
/// does a last component whose `*` could match RABBIT, whatever the root:
/// the resource folder can also be reached through `{appdata}` or `{home}`,
/// so `{appdata}/REAPER/RAB*` reaches RABBIT's folder as surely as
/// `{resource}/RAB*` does. Only the last component can hold a `*`. The
/// run-time guard checks the resolved path as well.
fn names_rabbit_folder(parsed: &ParsedUninstallPath<'_>) -> bool {
    if parsed
        .components
        .iter()
        .any(|component| component.eq_ignore_ascii_case(RABBIT_FOLDER))
    {
        return true;
    }
    parsed
        .components
        .last()
        .is_some_and(|last| wildcard_matches_ignoring_case(last, RABBIT_FOLDER))
}

/// `*` matches any run of characters, including none. ASCII case is
/// ignored, as the default file systems on Windows and macOS do.
pub(crate) fn wildcard_matches_ignoring_case(pattern: &str, name: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        // No `*` at all: the whole name had to be the prefix.
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

fn platform_name(platform: SupportedPlatform) -> &'static str {
    match platform {
        SupportedPlatform::Windows => "windows",
        SupportedPlatform::Macos => "macos",
    }
}

fn target_name(target: InstallationKind) -> &'static str {
    match target {
        InstallationKind::Standard => "standard",
        InstallationKind::Portable => "portable",
    }
}

fn strategy_name(strategy: UninstallStrategy) -> &'static str {
    match strategy {
        UninstallStrategy::RemoveFiles => "remove_files",
        UninstallStrategy::VendorUninstaller => "vendor_uninstaller",
        UninstallStrategy::Manual => "manual",
        UninstallStrategy::NotSupported => "not_supported",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        UninstallBackup, UninstallPath, UninstallStrategy, UninstallTarget,
        wildcard_matches_ignoring_case,
    };
    use crate::model::{InstallationKind, Platform};
    use crate::package::{
        EmbeddedPackageSpec, embedded_package_manifest, parse_embedded_package_spec,
        validate_package_spec,
    };

    const SUMMARY: &str =
        r#""summary": { "removes_key": "test-removes", "keeps_key": "test-keeps" }"#;

    /// A package offered on both platforms, with `extra` spliced in among its
    /// fields and `uninstall` as its uninstall block.
    fn demo_spec(extra: &str, uninstall: &str) -> EmbeddedPackageSpec {
        let source = format!(
            r#"{{
                "id": "demo",
                "display_name": "Demo",
                "display_name_key": "package-demo",
                "display_description_key": "package-demo-description",
                "recommended": false,
                "user_plugin_prefixes": ["reaper_demo"],
                "user_plugin_suffixes": {{ "windows": [".dll"], "macos": [".dylib"] }},
                {extra}
                "uninstall": {uninstall}
            }}"#
        );
        parse_embedded_package_spec(&source)
            .unwrap_or_else(|error| panic!("test manifest should parse: {error}\n{source}"))
    }

    /// An uninstall block holding `plans`, each already written as JSON.
    fn plans(plans: &[&str]) -> String {
        format!(r#"{{ "plans": [{}] }}"#, plans.join(","))
    }

    /// A `remove_files` plan with summary keys and the given fields.
    fn remove_plan(fields: &str) -> String {
        format!(r#"{{ "strategy": "remove_files", {SUMMARY}, {fields} }}"#)
    }

    fn accepts(extra: &str, uninstall: &str) {
        let spec = demo_spec(extra, uninstall);
        if let Err(message) = validate_package_spec(&spec) {
            panic!("expected {uninstall} to be accepted, got {message:?}");
        }
    }

    fn rejects(extra: &str, uninstall: &str, expected: &str) {
        let spec = demo_spec(extra, uninstall);
        match validate_package_spec(&spec) {
            Ok(()) => panic!("expected {uninstall} to be refused with {expected:?}"),
            Err(message) => assert!(
                message.contains(expected),
                "expected an error containing {expected:?}, got {message:?}"
            ),
        }
    }

    /// The uninstall blocks the plan sketches for later phases must already
    /// fit this schema and its rules, so filling them in later needs no
    /// schema change.
    #[test]
    fn the_plans_examples_parse_and_validate() {
        let sws = r#"{
            "reapack_owned": "block",
            "warn_when_installed": [
                { "package": "osara", "message_key": "uninstall-warning-sws-osara-keymap" },
                { "package": "reapack", "message_key": "uninstall-warning-sws-reapack-scripts" }
            ],
            "plans": [
                {
                    "platform": "windows",
                    "strategy": "remove_files",
                    "summary": {
                        "removes_key": "uninstall-summary-sws-removes",
                        "keeps_key": "uninstall-summary-sws-keeps"
                    },
                    "remove": {
                        "receipt_files": true,
                        "matching_user_plugins": true,
                        "paths": [
                            "{resource}/Scripts/sws_python.py",
                            "{resource}/Scripts/sws_python32.py",
                            "{resource}/Scripts/sws_python64.py"
                        ]
                    },
                    "kept_user_data": [
                        { "path": "{resource}/Data/Grooves", "note_key": "uninstall-kept-sws-grooves" },
                        { "path": "{resource}/S&M.ini", "note_key": "uninstall-kept-sws-settings" }
                    ]
                },
                {
                    "platform": "macos",
                    "strategy": "remove_files",
                    "summary": {
                        "removes_key": "uninstall-summary-sws-removes",
                        "keeps_key": "uninstall-summary-sws-keeps"
                    },
                    "remove": { "receipt_files": true, "matching_user_plugins": true }
                }
            ]
        }"#;
        let langpack = r#"{
            "requires_receipt": true,
            "plans": [
                {
                    "strategy": "remove_files",
                    "summary": {
                        "removes_key": "uninstall-summary-langpack-removes",
                        "keeps_key": "uninstall-summary-langpack-keeps"
                    },
                    "remove": { "receipt_files": true },
                    "post": [ { "reset_reaper_langpack_if_selected": {} } ]
                }
            ]
        }"#;
        let osara = r#"{
            "always_warn": [
                { "message_key": "uninstall-warning-osara-speech", "confirm": true }
            ],
            "plans": [
                {
                    "platform": "windows",
                    "strategy": "remove_files",
                    "summary": {
                        "removes_key": "uninstall-summary-osara-removes",
                        "keeps_key": "uninstall-summary-osara-keeps"
                    },
                    "remove": {
                        "receipt_files": true,
                        "matching_user_plugins": true,
                        "paths": [
                            "{resource}/KeyMaps/OSARA.ReaperKeyMap",
                            "{resource}/osara/locale/*.po",
                            "{resource}/osara/uninstall.exe"
                        ],
                        "dirs_if_empty": ["{resource}/osara/locale", "{resource}/osara"]
                    },
                    "keep": ["{resource}/reaper-kb.ini"],
                    "registry_cleanup": [
                        {
                            "hive": "hkcu",
                            "key": "OSARA",
                            "only_if_value_in": { "value": "UninstallString", "dir": "{resource}/osara" }
                        }
                    ]
                },
                {
                    "platform": "macos",
                    "strategy": "remove_files",
                    "summary": {
                        "removes_key": "uninstall-summary-osara-removes",
                        "keeps_key": "uninstall-summary-osara-keeps"
                    },
                    "remove": {
                        "receipt_files": true,
                        "matching_user_plugins": true,
                        "paths": [
                            "{resource}/KeyMaps/OSARA.ReaperKeyMap",
                            "{resource}/osara/locale/*.po"
                        ],
                        "dirs_if_empty": ["{resource}/osara/locale", "{resource}/osara"]
                    },
                    "keep": ["{resource}/reaper-kb.ini"]
                }
            ]
        }"#;
        let reaper = r#"{
            "plans": [
                {
                    "platform": "windows",
                    "target": "standard",
                    "strategy": "vendor_uninstaller",
                    "elevation": "required",
                    "backup": "none",
                    "summary": {
                        "removes_key": "uninstall-summary-reaper-windows-removes",
                        "keeps_key": "uninstall-summary-reaper-keeps"
                    },
                    "vendor_uninstaller": {
                        "kind": "nsis",
                        "locate": [
                            { "registry": { "hive": "hklm", "key": "REAPER", "value": "UninstallString" } }
                        ],
                        "install_location": {
                            "registry": { "hive": "hklm", "key": "REAPER", "value": "InstallLocation" },
                            "must_equal": "{app_dir}"
                        },
                        "must_be_in": { "under": "{program_files}" },
                        "arguments": ["/S"],
                        "timeout_seconds": 600,
                        "verify_gone": ["{app_path}"]
                    },
                    "kept_user_data": [
                        { "path": "{resource}", "note_key": "uninstall-kept-reaper-resource" }
                    ]
                },
                {
                    "platform": "windows",
                    "target": "portable",
                    "strategy": "manual",
                    "note_key": "uninstall-manual-reaper-portable"
                },
                {
                    "platform": "macos",
                    "strategy": "remove_files",
                    "elevation": "if_needed",
                    "backup": "to_trash",
                    "summary": {
                        "removes_key": "uninstall-summary-reaper-macos-removes",
                        "keeps_key": "uninstall-summary-reaper-keeps"
                    },
                    "remove": {
                        "trees": [
                            { "path": "{app_path}", "must_end_with": ".app", "name_contains": "REAPER" }
                        ]
                    },
                    "kept_user_data": [
                        { "path": "{resource}", "note_key": "uninstall-kept-reaper-resource" }
                    ]
                }
            ]
        }"#;
        let surge = r#"{
            "plans": [
                {
                    "platform": "windows",
                    "strategy": "vendor_uninstaller",
                    "elevation": "required",
                    "backup": "none",
                    "summary": {
                        "removes_key": "uninstall-summary-surge-removes",
                        "keeps_key": "uninstall-summary-surge-keeps"
                    },
                    "blocking_programs": ["Surge XT.exe", "Surge XT Effects.exe"],
                    "vendor_uninstaller": {
                        "kind": "inno",
                        "locate": [
                            { "registry": { "hive": "hklm", "key": "69F3FE96-DEEC-4C7C-B72D-E8957EC8411B_is1", "value": "UninstallString" } }
                        ],
                        "must_be_in": { "dir": "{program_data}/Surge XT/uninstall", "only_files": "unins000.*" },
                        "arguments": ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"],
                        "timeout_seconds": 600,
                        "verify_gone": ["{common_program_files}/VST3/Surge Synth Team/Surge XT.vst3"],
                        "report_leftovers": [
                            "{common_program_files}/CLAP/Surge Synth Team/SurgeXTData",
                            "{common_program_files}/VST3/Surge Synth Team/SurgeXTData"
                        ]
                    },
                    "kept_user_data": [
                        { "path": "{documents}/Surge XT", "note_key": "uninstall-kept-surge-user-data" }
                    ]
                },
                {
                    "platform": "macos",
                    "strategy": "remove_files",
                    "elevation": "required",
                    "backup": "none",
                    "summary": {
                        "removes_key": "uninstall-summary-surge-removes",
                        "keeps_key": "uninstall-summary-surge-keeps"
                    },
                    "blocking_programs": ["Surge XT", "Surge XT Effects"],
                    "remove": {
                        "trees": [
                            { "path": "/Applications/Surge XT.app", "must_end_with": ".app", "name_contains": "Surge XT" },
                            { "path": "/Applications/Surge XT Effects.app", "must_end_with": ".app", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/VST3/Surge XT.vst3", "must_end_with": ".vst3", "name_contains": "Surge XT" },
                            { "path": "/Library/Application Support/Surge XT", "name_contains": "Surge XT" }
                        ]
                    },
                    "macos_pkg_forget": [
                        "org.surge-synth-team.surge-xt.vst3.pkg",
                        "org.surge-synth-team.surge-xt.app.pkg"
                    ]
                }
            ]
        }"#;
        let jaws = r#"{
            "plans": [
                { "platform": "windows", "strategy": "manual", "note_key": "uninstall-manual-jaws-scripts" }
            ]
        }"#;

        accepts("", sws);
        accepts("", langpack);
        accepts("", osara);
        accepts("", reaper);
        accepts(r#""requires_standard_install": true,"#, surge);
        accepts(
            r#""requires_standard_install": true, "supported_platforms": ["windows"],"#,
            jaws,
        );

        // And they parse into what they say.
        let reaper = demo_spec("", reaper).uninstall;
        let portable = reaper
            .plan_for(Platform::Windows, InstallationKind::Portable)
            .expect("portable REAPER on Windows has a plan");
        assert_eq!(portable.strategy, UninstallStrategy::Manual);
        let mac = reaper
            .plan_for(Platform::MacOs, InstallationKind::Portable)
            .expect("REAPER on macOS has a plan");
        assert_eq!(mac.backup, UninstallBackup::ToTrash);
        assert_eq!(mac.remove.trees[0].path.as_str(), "{app_path}");
    }

    #[test]
    fn the_first_matching_plan_wins() {
        let uninstall = demo_spec(
            "",
            &plans(&[
                r#"{ "platform": "windows", "target": "portable", "strategy": "manual", "note_key": "a" }"#,
                r#"{ "strategy": "not_supported", "note_key": "b" }"#,
            ]),
        )
        .uninstall;
        let note = |platform, target| {
            uninstall
                .plan_for(platform, target)
                .and_then(|plan| plan.note_key.clone())
        };
        assert_eq!(
            note(Platform::Windows, InstallationKind::Portable).as_deref(),
            Some("a")
        );
        assert_eq!(
            note(Platform::Windows, InstallationKind::Standard).as_deref(),
            Some("b")
        );
        assert_eq!(
            note(Platform::MacOs, InstallationKind::Portable).as_deref(),
            Some("b")
        );
        assert_eq!(uninstall.plans[1].target, UninstallTarget::Any);
    }

    #[test]
    fn every_platform_and_target_needs_a_plan() {
        rejects(
            "",
            &plans(&[]),
            "no plan for windows with a standard REAPER",
        );
        rejects(
            "",
            &plans(&[r#"{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }"#]),
            "no plan for macos with a standard REAPER",
        );
        rejects(
            "",
            &plans(&[r#"{ "target": "standard", "strategy": "not_supported", "note_key": "a" }"#]),
            "no plan for windows with a portable REAPER",
        );
        // A package that needs a standard REAPER is never offered on a
        // portable one, so it needs no plan there.
        accepts(
            r#""requires_standard_install": true,"#,
            &plans(&[r#"{ "target": "standard", "strategy": "not_supported", "note_key": "a" }"#]),
        );
        accepts(
            r#""supported_platforms": ["windows"],"#,
            &plans(&[r#"{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }"#]),
        );
    }

    #[test]
    fn a_plan_that_never_applies_is_refused() {
        // Shadowed by the broader plan above it.
        rejects(
            "",
            &plans(&[
                r#"{ "strategy": "not_supported", "note_key": "a" }"#,
                r#"{ "platform": "windows", "strategy": "manual", "note_key": "b" }"#,
            ]),
            "plan #1 never applies",
        );
        // For a platform the package isn't offered on.
        rejects(
            r#""supported_platforms": ["windows"],"#,
            &plans(&[
                r#"{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }"#,
                r#"{ "platform": "macos", "strategy": "not_supported", "note_key": "a" }"#,
            ]),
            "plan #1 never applies",
        );
        // For a portable REAPER the package is never offered on.
        rejects(
            r#""requires_standard_install": true,"#,
            &plans(&[
                r#"{ "target": "portable", "strategy": "manual", "note_key": "a" }"#,
                r#"{ "strategy": "not_supported", "note_key": "b" }"#,
            ]),
            "plan #0 never applies",
        );
    }

    #[test]
    fn manual_and_not_supported_plans_need_a_note_and_nothing_else() {
        rejects(
            "",
            &plans(&[r#"{ "strategy": "not_supported" }"#]),
            "needs a note_key",
        );
        rejects(
            "",
            &plans(&[r#"{ "strategy": "manual" }"#]),
            "needs a note_key",
        );
        rejects(
            "",
            &plans(&[
                r#"{ "strategy": "manual", "note_key": "a", "remove": { "receipt_files": true } }"#,
            ]),
            "changes nothing",
        );
        rejects(
            "",
            &plans(&[
                r#"{ "strategy": "not_supported", "note_key": "a", "elevation": "required", "backup": "none" }"#,
            ]),
            "changes nothing",
        );
        rejects(
            "",
            &plans(&[&format!(
                r#"{{ "strategy": "not_supported", "note_key": "a", {SUMMARY} }}"#
            )]),
            "changes nothing",
        );
    }

    #[test]
    fn remove_files_plans_need_summary_keys_and_something_to_remove() {
        rejects(
            "",
            &plans(&[r#"{ "strategy": "remove_files", "remove": { "receipt_files": true } }"#]),
            "needs summary keys",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""remove": { "dirs_if_empty": ["{resource}/demo"] }"#,
            )]),
            "needs something to remove",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""remove": { "receipt_files": true },
                   "vendor_uninstaller": {
                       "kind": "inno",
                       "locate": [{ "path": "{resource}/unins000.exe" }],
                       "timeout_seconds": 60
                   }"#,
            )]),
            "belongs only in a vendor_uninstaller plan",
        );
        accepts(
            "",
            &plans(&[&remove_plan(r#""remove": { "receipt_files": true }"#)]),
        );
    }

    /// A Windows plan for a vendor uninstaller with `fields`, next to a
    /// macOS plan so the package is covered everywhere.
    fn vendor_plans(fields: &str) -> String {
        plans(&[
            &format!(
                r#"{{ "platform": "windows", "strategy": "vendor_uninstaller", {SUMMARY}, {fields} }}"#
            ),
            r#"{ "platform": "macos", "strategy": "not_supported", "note_key": "a" }"#,
        ])
    }

    const INNO_IN_PLACE: &str = r#""vendor_uninstaller": {
        "kind": "inno",
        "locate": [{ "registry": { "hive": "hklm", "key": "Demo_is1", "value": "UninstallString" } }],
        "must_be_in": { "dir": "{program_data}/Demo/uninstall", "only_files": "unins000.*" },
        "timeout_seconds": 600
    }"#;

    #[test]
    fn vendor_uninstaller_plans_need_their_block_and_windows() {
        accepts("", &vendor_plans(INNO_IN_PLACE));
        rejects(
            "",
            &vendor_plans(r#""backup": "none""#),
            "needs a vendor_uninstaller block",
        );
        rejects(
            "",
            &plans(&[&format!(
                r#"{{ "strategy": "vendor_uninstaller", {SUMMARY}, {INNO_IN_PLACE} }}"#
            )]),
            "needs \"platform\": \"windows\"",
        );
        rejects(
            "",
            &vendor_plans(&format!(
                r#"{INNO_IN_PLACE}, "remove": {{ "receipt_files": true }}"#
            )),
            "may not use receipt_files",
        );
        rejects(
            "",
            &vendor_plans(
                r#""vendor_uninstaller": { "kind": "inno", "locate": [], "timeout_seconds": 60 }"#,
            ),
            "at least one locate entry",
        );
        rejects(
            "",
            &vendor_plans(
                r#""vendor_uninstaller": {
                    "kind": "inno",
                    "locate": [{ "path": "{program_data}/Demo/unins000.exe" }],
                    "timeout_seconds": 0
                }"#,
            ),
            "above zero",
        );
        rejects(
            "",
            &vendor_plans(
                r#""vendor_uninstaller": {
                    "kind": "inno",
                    "locate": [{ "path": "{program_data}/Demo/unins000.exe" }],
                    "must_be_in": { "dir": "{program_data}/Demo", "only_files": "*" },
                    "timeout_seconds": 60
                }"#,
            ),
            "must be a file name pattern",
        );
    }

    #[test]
    fn nsis_uninstallers_need_an_install_location_below_a_root() {
        rejects(
            "",
            &vendor_plans(
                r#""vendor_uninstaller": {
                    "kind": "nsis",
                    "locate": [{ "registry": { "hive": "hklm", "key": "Demo", "value": "UninstallString" } }],
                    "must_be_in": { "under": "{program_files}" },
                    "timeout_seconds": 600
                }"#,
            ),
            "needs install_location",
        );
        rejects(
            "",
            &vendor_plans(
                r#""vendor_uninstaller": {
                    "kind": "nsis",
                    "locate": [{ "registry": { "hive": "hklm", "key": "Demo", "value": "UninstallString" } }],
                    "install_location": {
                        "registry": { "hive": "hklm", "key": "Demo", "value": "InstallLocation" },
                        "must_equal": "{app_dir}"
                    },
                    "must_be_in": { "dir": "{program_files}/Demo", "only_files": "Uninstall.exe" },
                    "timeout_seconds": 600
                }"#,
            ),
            "must_be_in with under",
        );
    }

    #[test]
    fn elevation_never_moves_files_into_the_users_backups() {
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""elevation": "required", "remove": { "receipt_files": true }"#,
            )]),
            "elevation required needs backup none or to_trash",
        );
        accepts(
            "",
            &plans(&[&remove_plan(
                r#""elevation": "required", "backup": "none", "remove": { "receipt_files": true }"#,
            )]),
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""elevation": "if_needed", "backup": "to_trash", "remove": { "receipt_files": true }"#,
            )]),
            "if_needed is only for trees with backup to_trash",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""elevation": "if_needed", "backup": "none",
                   "remove": { "trees": [{ "path": "{resource}/Demo.app", "must_end_with": ".app" }] }"#,
            )]),
            "if_needed is only for trees with backup to_trash",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""remove": { "trees": [{ "path": "{resource}/Demo.app", "must_end_with": ".app" }] }"#,
            )]),
            "can't be moved into a backup set",
        );
    }

    #[test]
    fn elevated_vendor_uninstallers_are_found_through_hklm_only() {
        let elevated = r#""elevation": "required", "backup": "none""#;
        accepts("", &vendor_plans(&format!("{elevated}, {INNO_IN_PLACE}")));
        rejects(
            "",
            &vendor_plans(&format!(
                r#"{elevated}, "vendor_uninstaller": {{
                    "kind": "inno",
                    "locate": [{{ "registry": {{ "hive": "hklm", "key": "Demo_is1", "value": "UninstallString" }} }}],
                    "timeout_seconds": 600
                }}"#
            )),
            "needs must_be_in",
        );
        for locate in [
            r#"{ "path": "{program_data}/Demo/uninstall/unins000.exe" }"#,
            r#"{ "registry": { "hive": "hkcu", "key": "Demo_is1", "value": "UninstallString" } }"#,
        ] {
            rejects(
                "",
                &vendor_plans(&format!(
                    r#"{elevated}, "vendor_uninstaller": {{
                        "kind": "inno",
                        "locate": [
                            {{ "registry": {{ "hive": "hklm", "key": "Demo_is1", "value": "UninstallString" }} }},
                            {locate}
                        ],
                        "must_be_in": {{ "dir": "{{program_data}}/Demo/uninstall", "only_files": "unins000.*" }},
                        "timeout_seconds": 600
                    }}"#
                )),
                "only be located through HKLM",
            );
        }
        rejects(
            "",
            &vendor_plans(&format!(
                r#"{elevated}, "vendor_uninstaller": {{
                    "kind": "nsis",
                    "locate": [{{ "registry": {{ "hive": "hklm", "key": "Demo", "value": "UninstallString" }} }}],
                    "install_location": {{
                        "registry": {{ "hive": "hkcu", "key": "Demo", "value": "InstallLocation" }},
                        "must_equal": "{{app_dir}}"
                    }},
                    "must_be_in": {{ "under": "{{program_files}}" }},
                    "timeout_seconds": 600
                }}"#
            )),
            "install_location from HKLM only",
        );
        rejects(
            "",
            &vendor_plans(
                r#""vendor_uninstaller": {
                    "kind": "inno",
                    "locate": [{ "registry": { "hive": "hklm", "key": "Software\\Demo", "value": "UninstallString" } }],
                    "timeout_seconds": 600
                }"#,
            ),
            "without separators",
        );
    }

    /// One removal path in a plan for every platform.
    fn path_plan(path: &str) -> String {
        plans(&[&remove_plan(&format!(
            r#""remove": {{ "paths": ["{path}"] }}"#
        ))])
    }

    #[test]
    fn paths_use_known_tokens_and_plain_components() {
        accepts("", &path_plan("{resource}/osara/locale/*.po"));
        accepts("", &path_plan("{resource}/Scripts/sws_python*.py"));
        for (path, expected) in [
            ("{resorce}/UserPlugins/demo.dll", "unknown token {resorce}"),
            ("{resource/UserPlugins/demo.dll", "never closes it"),
            ("UserPlugins/demo.dll", "must start with a token"),
            ("{resource}UserPlugins", "needs a / after its token"),
            (
                "{resource}/UserPlugins/../reaper.ini",
                "may not use . or ..",
            ),
            ("{resource}/./reaper.ini", "may not use . or .."),
            ("{resource}//reaper.ini", "empty component"),
            ("{resource}/UserPlugins/", "empty component"),
            (r"{resource}/UserPlugins\\demo.dll", r"may not contain '\\'"),
            ("{resource}/C:", "may not contain ':'"),
            ("{resource}/{app_dir}/demo.dll", "may not contain '{'"),
            ("{resource}/*/demo.dll", "only there"),
            ("{resource}/UserPlugins/*", "bare *"),
        ] {
            rejects("", &path_plan(path), expected);
        }
        // A wildcard is only for paths that may match several files.
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""remove": { "receipt_files": true, "dirs_if_empty": ["{resource}/osara*"] }"#,
            )]),
            "may not use *",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""backup": "none", "remove": { "trees": [{ "path": "{resource}/Demo*.app", "must_end_with": ".app" }] }"#,
            )]),
            "may not use *",
        );
    }

    #[test]
    fn tokens_are_used_only_where_their_kind_allows() {
        rejects(
            "",
            &path_plan("{resource}"),
            "is a folder root, which is never removed",
        );
        rejects(
            "",
            &path_plan("{app_dir}/Plugins/demo.dll"),
            "only for comparison",
        );
        rejects(
            "",
            &path_plan("{app_path}"),
            "allowed only in remove.trees and verify_gone",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""backup": "none", "remove": { "trees": [{ "path": "{app_path}/Contents", "must_end_with": ".app" }] }"#,
            )]),
            "can't add components after it",
        );
        // Windows folders exist only on Windows, and $HOME only on macOS.
        rejects(
            "",
            &path_plan("{program_files}/Demo/demo.dll"),
            "{program_files} exists only on windows",
        );
        rejects(
            "",
            &windows_remove_plans(r#""keep": ["{home}/Demo"]"#),
            "{home} exists only on macos",
        );
        accepts(
            "",
            &windows_remove_plans(r#""keep": ["{clap_dir}/demo.clap"]"#),
        );
    }

    /// A macOS plan removing the tree at `path`, next to a Windows plan.
    fn macos_tree_plans(path: &str) -> String {
        plans(&[
            r#"{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }"#,
            &format!(
                r#"{{ "platform": "macos", "strategy": "remove_files", "elevation": "required",
                     "backup": "none", {SUMMARY},
                     "remove": {{ "trees": [{{ "path": "{path}", "name_contains": "Demo" }}] }} }}"#
            ),
        ])
    }

    #[test]
    fn absolute_paths_stay_in_macos_plans_below_known_folders() {
        accepts("", &macos_tree_plans("/Applications/Demo.app"));
        accepts(
            "",
            &macos_tree_plans("/Library/Audio/Plug-Ins/VST3/Demo.vst3"),
        );
        accepts("", &macos_tree_plans("/Library/Application Support/Demo"));
        for path in [
            "/Applications",
            "/Library/Audio/Plug-Ins/VST3",
            "/Library/Audio/Plug-Ins/Demo.vst3",
            "/usr/local/bin/demo",
        ] {
            rejects("", &macos_tree_plans(path), "must sit below /Applications");
        }
        rejects("", &macos_tree_plans("/"), "empty component");
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""backup": "none", "remove": { "trees": [{ "path": "/Applications/Demo.app", "must_end_with": ".app" }] }"#,
            )]),
            "allowed only in plans with \"platform\": \"macos\"",
        );
    }

    #[test]
    fn nothing_may_reach_into_rabbits_own_folder() {
        for path in [
            "{resource}/RABBIT/install-state.json",
            "{resource}/rabbit/logs/old.txt",
            "{resource}/R*T",
            "{resource}/RAB*",
        ] {
            rejects("", &path_plan(path), "RABBIT's own folder");
        }
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""remove": { "receipt_files": true }, "keep": ["{resource}/RABBIT"]"#,
            )]),
            "RABBIT's own folder",
        );
        // The resource folder reached another way still holds RABBIT/.
        rejects(
            "",
            &windows_remove_plans(r#""keep": ["{appdata}/REAPER/RABBIT/locks"]"#),
            "RABBIT's own folder",
        );
        // So does a pattern that could match it there: on a standard install
        // %APPDATA%\REAPER and $HOME/Library/Application Support/REAPER are
        // the resource folder.
        rejects(
            "",
            &plans(&[
                &format!(
                    r#"{{ "platform": "windows", "strategy": "remove_files", {SUMMARY},
                         "remove": {{ "paths": ["{{appdata}}/REAPER/RAB*"] }} }}"#
                ),
                r#"{ "platform": "macos", "strategy": "not_supported", "note_key": "a" }"#,
            ]),
            "RABBIT's own folder",
        );
        rejects(
            "",
            &plans(&[
                r#"{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }"#,
                &format!(
                    r#"{{ "platform": "macos", "strategy": "remove_files", {SUMMARY},
                         "remove": {{ "paths": ["{{home}}/Library/Application Support/REAPER/R*"] }} }}"#
                ),
            ]),
            "RABBIT's own folder",
        );
        // A pattern that can't match RABBIT is fine.
        accepts("", &path_plan("{resource}/Rab*.ini"));
        accepts("", &path_plan("{resource}/UserPlugins/reaper_demo*.dll"));
    }

    #[test]
    fn trees_need_a_name_check() {
        for tree in [
            r#"{ "path": "{resource}/Demo.app" }"#,
            r#"{ "path": "{resource}/Demo.app", "must_end_with": "", "name_contains": "" }"#,
        ] {
            rejects(
                "",
                &plans(&[&remove_plan(&format!(
                    r#""backup": "to_trash", "remove": {{ "trees": [{tree}] }}"#
                ))]),
                "needs must_end_with or name_contains",
            );
        }
    }

    #[test]
    fn matching_user_plugins_needs_a_prefix() {
        let uninstall = plans(&[&remove_plan(
            r#""remove": { "matching_user_plugins": true }"#,
        )]);
        assert!(validate_package_spec(&demo_spec("", &uninstall)).is_ok());
        // One empty prefix among good ones matches every name just the same.
        for prefixes in [
            Vec::new(),
            vec![String::new()],
            vec![String::new(), "reaper_demo".to_string()],
            vec!["reaper_demo".to_string(), String::new()],
        ] {
            let mut spec = demo_spec("", &uninstall);
            spec.user_plugin_prefixes = prefixes;
            let message = validate_package_spec(&spec).unwrap_err();
            assert!(
                message.contains("non-empty user_plugin_prefixes"),
                "got {message:?}"
            );
        }
    }

    /// A Windows `remove_files` plan with `fields`, next to a macOS plan.
    fn windows_remove_plans(fields: &str) -> String {
        plans(&[
            &format!(
                r#"{{ "platform": "windows", "strategy": "remove_files", {SUMMARY},
                     "remove": {{ "receipt_files": true }}, {fields} }}"#
            ),
            r#"{ "platform": "macos", "strategy": "not_supported", "note_key": "a" }"#,
        ])
    }

    #[test]
    fn registry_cleanup_removes_only_hkcu_keys_on_windows() {
        let cleanup = |hive: &str, key: &str| {
            format!(
                r#""registry_cleanup": [{{
                    "hive": "{hive}", "key": "{key}",
                    "only_if_value_in": {{ "value": "UninstallString", "dir": "{{resource}}/demo" }}
                }}]"#
            )
        };
        accepts("", &windows_remove_plans(&cleanup("hkcu", "Demo")));
        rejects(
            "",
            &windows_remove_plans(&cleanup("hklm", "Demo")),
            "only remove HKCU keys",
        );
        rejects(
            "",
            &windows_remove_plans(&cleanup("hkcu", r"Software\\Demo")),
            "without separators",
        );
        rejects(
            "",
            &plans(&[&remove_plan(&format!(
                r#""remove": {{ "receipt_files": true }}, {}"#,
                cleanup("hkcu", "Demo")
            ))]),
            "registry_cleanup needs \"platform\": \"windows\"",
        );
    }

    #[test]
    fn pkg_ids_and_program_names_are_checked() {
        rejects(
            "",
            &windows_remove_plans(r#""macos_pkg_forget": ["org.example.demo.pkg"]"#),
            "macos_pkg_forget needs \"platform\": \"macos\"",
        );
        rejects(
            "",
            &plans(&[
                r#"{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }"#,
                &format!(
                    r#"{{ "platform": "macos", "strategy": "remove_files", {SUMMARY},
                         "remove": {{ "receipt_files": true }},
                         "macos_pkg_forget": ["org.example.demo.pkg; rm -rf /"] }}"#
                ),
            ]),
            "must be a full package id",
        );
        rejects(
            "",
            &windows_remove_plans(r#""blocking_programs": ["C:/Demo/demo.exe"]"#),
            "must be a program name, not a path",
        );
    }

    /// Snapshot roots are only listed, so `*` may stand for whole folder
    /// names, but each root still has to stay narrow and resolvable.
    #[test]
    fn install_snapshot_roots_are_checked() {
        let windows_only = r#""supported_platforms": ["windows"],"#;
        let with_roots = |roots: &str| {
            format!(
                r#"{{ "install_snapshot": [{roots}],
                     "plans": [{{ "strategy": "not_supported", "note_key": "a" }}] }}"#
            )
        };
        accepts(
            windows_only,
            &with_roots(
                r#""{appdata}/Freedom Scientific/JAWS/*/Settings/*", "{appdata}/REAPER/UserPlugins""#,
            ),
        );
        accepts("", &with_roots(r#""{resource}/Data/*/presets""#));
        for (root, expected) in [
            (
                r#""{appdata}/*/Settings""#,
                "must spell out the folder right after {appdata}",
            ),
            (
                r#""{appdata}""#,
                "must spell out the folder right after {appdata}",
            ),
            (r#""{app_path}""#, "{app_path} isn't a folder to list"),
            (r#""{app_dir}/Plugins""#, "{app_dir} isn't a folder to list"),
            (
                r#""/Applications/Demo.app""#,
                "must start with a folder token",
            ),
            (
                r#""{appdata}/REAPER/RABBIT/logs""#,
                "reaches into RABBIT's own folder",
            ),
            (r#""{appdata}/JAWS/../REAPER""#, "may not use . or .."),
            (
                r#""{home}/Library/Demo""#,
                "where the package isn't offered",
            ),
        ] {
            rejects(windows_only, &with_roots(root), expected);
        }
        // The plan paths keep their stricter grammar.
        assert!(
            UninstallPath::new("{appdata}/JAWS/*/Settings")
                .parse()
                .is_err()
        );
        assert!(
            UninstallPath::new("{appdata}/JAWS/*/Settings")
                .parse_pattern()
                .is_ok()
        );
    }

    #[test]
    fn a_package_may_not_warn_about_itself() {
        rejects(
            "",
            r#"{
                "warn_when_installed": [{ "package": "demo", "message_key": "a" }],
                "plans": [{ "strategy": "not_supported", "note_key": "b" }]
            }"#,
            "names the package itself",
        );
    }

    /// A field the schema doesn't know is a typo, and a typo in `keep` would
    /// otherwise drop the files it protects without a word.
    #[test]
    fn unknown_fields_are_refused() {
        for uninstall in [
            r#"{ "plans": [{ "strategy": "not_supported", "note_key": "a" }], "requires_reciept": true }"#,
            r#"{ "plans": [{ "strategy": "remove_files", "kep": ["{resource}/reaper.ini"] }] }"#,
            r#"{ "plans": [{ "strategy": "remove_files", "remove": { "receipt_file": true } }] }"#,
        ] {
            let source = format!(
                r#"{{
                    "id": "demo",
                    "display_name": "Demo",
                    "display_name_key": "package-demo",
                    "display_description_key": "package-demo-description",
                    "recommended": false,
                    "user_plugin_prefixes": [],
                    "user_plugin_suffixes": {{ "windows": [], "macos": [] }},
                    "uninstall": {uninstall}
                }}"#
            );
            let error = parse_embedded_package_spec(&source).unwrap_err();
            assert!(
                error.to_string().contains("unknown field"),
                "expected an unknown-field error, got {error}"
            );
        }
    }

    #[test]
    fn parsed_paths_name_their_root_and_components() {
        use super::{UninstallPathRoot, UninstallPathToken, UninstallTokenKind};

        let path = UninstallPath::new("{resource}/osara/locale/*.po");
        let parsed = path.parse().unwrap();
        assert_eq!(
            parsed.root,
            UninstallPathRoot::Token(UninstallPathToken::Resource)
        );
        assert_eq!(parsed.components, vec!["osara", "locale", "*.po"]);

        let path = UninstallPath::new("/Library/Application Support/Surge XT");
        let parsed = path.parse().unwrap();
        assert_eq!(parsed.root, UninstallPathRoot::MacosAbsolute);
        assert_eq!(
            parsed.components,
            vec!["Library", "Application Support", "Surge XT"]
        );

        for token in UninstallPathToken::ALL {
            assert_eq!(UninstallPathToken::from_name(token.name()), Some(token));
        }
        assert_eq!(
            UninstallPathToken::AppPath.kind(),
            UninstallTokenKind::Object
        );
        assert_eq!(
            UninstallPathToken::AppDir.kind(),
            UninstallTokenKind::ComparisonRoot
        );
    }

    #[test]
    fn wildcards_match_like_file_names() {
        assert!(wildcard_matches_ignoring_case("R*T", "RABBIT"));
        assert!(wildcard_matches_ignoring_case("rab*", "RABBIT"));
        assert!(wildcard_matches_ignoring_case("*", "RABBIT"));
        assert!(wildcard_matches_ignoring_case("RABBIT", "rabbit"));
        assert!(wildcard_matches_ignoring_case("*.po", "de.po"));
        assert!(wildcard_matches_ignoring_case("a*b*c", "aXbYc"));
        assert!(wildcard_matches_ignoring_case("a*b*b", "abb"));
        assert!(!wildcard_matches_ignoring_case("a*a", "a"));
        assert!(!wildcard_matches_ignoring_case("*.po", "de.pot"));
        assert!(!wildcard_matches_ignoring_case("Rab*.ini", "RABBIT"));
        assert!(!wildcard_matches_ignoring_case("RABBIT", "RABBITS"));
    }

    /// Every Fluent key a manifest names must exist, or the user would see
    /// the bare key. Checking en-US is enough: the locale parity test then
    /// holds the other four to the same keys.
    #[test]
    fn every_fluent_key_a_manifest_names_exists_in_english() {
        let localizer =
            crate::localization::Localizer::embedded(crate::localization::DEFAULT_LOCALE).unwrap();
        for spec in embedded_package_manifest().packages {
            let mut keys = vec![
                spec.display_name_key.as_str(),
                spec.display_description_key.as_str(),
            ];
            keys.extend(
                spec.variants
                    .iter()
                    .map(|variant| variant.display_name_key.as_str()),
            );
            keys.extend(
                spec.channels
                    .iter()
                    .map(|channel| channel.display_name_key.as_str()),
            );
            keys.extend(spec.uninstall.fluent_keys());
            for key in keys {
                assert!(
                    !localizer.text(key).missing,
                    "{} names the Fluent key {key:?}, which en-US doesn't define",
                    spec.id
                );
            }
        }
    }

    #[test]
    fn uninstall_warnings_name_packages_that_exist() {
        let packages = embedded_package_manifest().packages;
        for spec in &packages {
            for warning in &spec.uninstall.warn_when_installed {
                assert!(
                    packages.iter().any(|other| other.id == warning.package),
                    "{} warns about {:?}, which isn't a package",
                    spec.id,
                    warning.package
                );
            }
        }
    }
}
