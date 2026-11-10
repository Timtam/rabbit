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

use std::collections::BTreeMap;

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
    /// Warnings shown whenever this package is about to be uninstalled,
    /// whatever else is installed, each with a question before starting if
    /// it asks for one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always_warn: Vec<AlwaysWarning>,
    /// Whether the details say that the package serves every REAPER and
    /// every other program on this computer, which RABBIT says by itself
    /// for a package that needs a standard REAPER. Turned off for one that
    /// serves a single program, such as the JAWS scripts. Only a package
    /// with `requires_standard_install` may turn it off.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub machine_wide_warning: bool,
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
            keys.extend(
                plan.remove
                    .vendor_added_files
                    .as_ref()
                    .map(|vendor_added| vendor_added.by_hand_note_key.as_str()),
            );
            keys.extend(
                plan.machine_cleanup
                    .as_ref()
                    .map(|cleanup| cleanup.by_hand_key.as_str()),
            );
        }
        keys
    }
}

/// Serde's default for a flag that is on unless the manifest says
/// otherwise.
fn default_true() -> bool {
    true
}

/// Leaves a flag that is on by default out of what RABBIT writes.
fn is_true(value: &bool) -> bool {
    *value
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
    /// all" by itself where it is needed, so a manifest can't forget it. On
    /// Windows the same names also keep the package's vendor installer from
    /// starting while one of them runs. A screen reader is never allowed
    /// here: a blind user can't close it to get past the check.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking_programs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_uninstaller: Option<VendorUninstallerSpec>,
    /// HKCU uninstall keys a removal leaves pointing at nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registry_cleanup: Vec<RegistryCleanup>,
    /// The uninstaller folder and HKLM key a Windows installer leaves
    /// behind, which only the elevated helper removes, after the plan's own
    /// files are in the backup set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_cleanup: Option<MachineCleanupSpec>,
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
            && self.machine_cleanup.is_none()
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
    /// Administrator rights. On Windows a vendor's uninstaller runs with
    /// them, through the elevated helper; on macOS the run's one
    /// administrator command deletes a `remove_files` plan's trees for
    /// good, so that plan needs `backup: none`.
    Required,
    /// Only for a macOS app, `{app_path}` with `backup: to_trash`: the app
    /// goes into the user's Trash through the run's administrator command
    /// only when RABBIT can't move it there itself.
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
    /// Files the receipt records as added by the package's installer
    /// outside the resource folder (`vendor_added_files`), such as the JAWS
    /// scripts' files in JAWS's settings folders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_added_files: Option<VendorAddedRemoval>,
}

impl UninstallRemoveSpec {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Which of the files the receipt records as added by the installer go
/// with the package. Only a recorded file that matches one of `files`
/// qualifies, and only while it is unchanged since the install; every
/// other file stays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VendorAddedRemoval {
    /// Patterns written like `install_snapshot` roots, where `*` may stand
    /// for a whole name at any level. Each lies below one of the package's
    /// `install_snapshot` roots, since RABBIT records files only there.
    pub files: Vec<UninstallPath>,
    /// Says how to remove by hand the files that stay.
    pub by_hand_note_key: String,
}

/// What a Windows installer leaves in machine-wide places to find its own
/// uninstaller: a folder in `{program_files_x86}` and a key in HKLM's list
/// of installed programs. Only an administrator may remove them, so the
/// elevated helper does, and only while what is there matches this block
/// exactly. A backup can't bring them back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineCleanupSpec {
    /// The folder, below `{program_files_x86}`. It goes after its files,
    /// and only once it is empty.
    pub dir: UninstallPath,
    /// Every file the folder may hold, by name, in the order they are
    /// deleted. Anything else in the folder refuses the cleanup.
    pub files: Vec<String>,
    pub registry: MachineCleanupRegistry,
    /// The steps to remove the package by hand, for when RABBIT can't.
    pub by_hand_key: String,
}

/// The uninstall key a [`MachineCleanupSpec`] deletes last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineCleanupRegistry {
    pub hive: RegistryHive,
    /// One key name under the Uninstall key, such as `Reaper_JawsScripts`.
    pub key: String,
    pub view: RegistryViewSpec,
    /// Every value the key may hold, each with the path it must name: the
    /// folder, or one of its files. Any other value refuses the cleanup.
    #[serde(deserialize_with = "values_named_once")]
    pub values: BTreeMap<String, UninstallPath>,
}

/// Reads [`MachineCleanupRegistry::values`], refusing a value name given
/// twice. JSON allows a repeated key and serde keeps only the last, so a
/// typo in one of the two would otherwise go unnoticed.
fn values_named_once<'de, D>(deserializer: D) -> Result<BTreeMap<String, UninstallPath>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Values;

    impl<'de> serde::de::Visitor<'de> for Values {
        type Value = BTreeMap<String, UninstallPath>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map from value names to paths")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut values = BTreeMap::new();
            while let Some((name, path)) = map.next_entry::<String, UninstallPath>()? {
                if values.contains_key(&name) {
                    return Err(serde::de::Error::custom(format!(
                        "machine_cleanup.registry.values lists {name:?} twice"
                    )));
                }
                values.insert(name, path);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_map(Values)
}

/// The registry view a key lives in. Only the 32-bit view, where a 32-bit
/// installer's key lands on 64-bit Windows, until a package needs another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryViewSpec {
    Bit32,
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
    /// The `CFBundleIdentifier` a macOS app bundle here must have, such as
    /// `com.cockos.reaper`. A bundle with another one, or none, isn't the
    /// app the manifest means, and RABBIT leaves it for the user to remove
    /// in Finder. Only for a `backup: to_trash` plan, where it is checked;
    /// required for `elevation: if_needed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
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
    /// Exit codes besides 0 with which this uninstaller ends when it has
    /// done its work. The verdict then rests on `verify_gone` and the
    /// registry entry alone, so a plan that lists any must have
    /// `verify_gone`. REAPER's uninstaller needs it: the NSIS UAC plugin
    /// (v0.2.4) it bundles sets the exit code to 1223 whenever it runs
    /// elevated, whatever happens next, and REAPER's script never sets it
    /// back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub success_exit_codes: Vec<i32>,
    /// Paths that must be gone afterwards. Only tested for existence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verify_gone: Vec<UninstallPath>,
    /// Paths reported, never removed, when they are still there afterwards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub report_leftovers: Vec<UninstallPath>,
    /// What the vendor's uninstaller leaves behind that RABBIT removes
    /// itself, in this order, once `verify_gone` has passed, also when what
    /// is still there goes at the next restart: links as links and folders
    /// only when empty. Only the elevated helper removes them, so only plans
    /// with `elevation: required` may list them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_leftovers: Vec<LeftoverRemoval>,
}

/// One thing a vendor's uninstaller leaves behind that RABBIT removes.
/// Limited to two kinds that can't take anything else with them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LeftoverRemoval {
    /// A junction, removed as a link. What it points to is never touched;
    /// anything that isn't a junction stays.
    Junction(UninstallPath),
    /// A folder, removed only when it is empty and isn't a link.
    EmptyDir(UninstallPath),
}

impl LeftoverRemoval {
    pub fn path(&self) -> &UninstallPath {
        match self {
            Self::Junction(path) | Self::EmptyDir(path) => path,
        }
    }
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
    /// macOS only: have macOS list its Audio Units again, so hosts stop
    /// offering the plug-ins that are gone. Runs as the user, and a failure
    /// never undoes the removal.
    RefreshAudioUnits {},
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
    /// `C:\Program Files (x86)` alone, as Windows names it
    /// (`FOLDERID_ProgramFilesX86`), never read from the environment: where
    /// a 32-bit installer writes on 64-bit Windows. `{program_files}`
    /// stands for both Program Files folders.
    ProgramFilesX86,
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
    pub const ALL: [Self; 12] = [
        Self::Resource,
        Self::AppDir,
        Self::AppPath,
        Self::Appdata,
        Self::Localappdata,
        Self::ClapDir,
        Self::CommonProgramFiles,
        Self::ProgramFiles,
        Self::ProgramFilesX86,
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
            Self::ProgramFilesX86 => "program_files_x86",
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
    /// Windows folders come from Windows environment variables, or for
    /// `{program_files_x86}` from Windows' own name for the folder, and
    /// `{home}` from `$HOME`, which Windows doesn't set.
    pub fn platform(self) -> Option<SupportedPlatform> {
        match self {
            Self::Appdata
            | Self::Localappdata
            | Self::ClapDir
            | Self::CommonProgramFiles
            | Self::ProgramFiles
            | Self::ProgramFilesX86
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
const REMOVE_LEFTOVERS: PathUse = PathUse::new("remove_leftovers", true, false, false);
const LOCATE: PathUse = PathUse::new("locate", false, false, false);
const MUST_EQUAL: PathUse = PathUse::new("install_location.must_equal", false, false, false);
const MUST_BE_IN: PathUse = PathUse::new("must_be_in", false, false, false);
const ONLY_IF_VALUE_IN: PathUse =
    PathUse::new("registry_cleanup.only_if_value_in", false, false, false);
const MACHINE_CLEANUP_DIR: PathUse = PathUse::new("machine_cleanup.dir", true, false, false);
const MACHINE_CLEANUP_VALUE: PathUse =
    PathUse::new("machine_cleanup.registry.values", false, false, false);

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
    // RABBIT says a package serves the whole computer only for one that
    // needs a standard REAPER, so for any other there is nothing to turn
    // off, and the flag would mislead whoever reads the manifest.
    if !uninstall.machine_wide_warning && !spec.requires_standard_install {
        return Err(
            "uninstall machine_wide_warning false needs requires_standard_install: only such a \
             package gets the machine-wide warning"
                .to_string(),
        );
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
                && remove.vendor_added_files.is_none()
                && plan.machine_cleanup.is_none()
            {
                return Err(
                    "a remove_files plan needs something to remove: receipt_files, \
                     matching_user_plugins, paths, trees, vendor_added_files or machine_cleanup"
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
    // Before the general elevation rules, whose words about macOS would
    // only confuse here: machine_cleanup states its own elevation and
    // backup needs.
    validate_machine_cleanup(spec, plan)?;
    validate_elevation_and_backup(plan)?;
    validate_vendor_added_files(spec, plan)?;

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
        if let Some(bundle_id) = &tree.bundle_id
            && !is_package_id(bundle_id)
        {
            return Err(format!(
                "remove.trees entry {:?} has the bundle_id {bundle_id:?}, which must be a full \
                 identifier such as com.example.app",
                tree.path.as_str()
            ));
        }
        // Only the Trash route reads the identifier, through `tree_route`.
        // The administrator command's deletes never do, so one there would
        // look like a guard and guard nothing.
        if tree.bundle_id.is_some() && plan.backup != UninstallBackup::ToTrash {
            return Err(format!(
                "remove.trees entry {:?} has a bundle_id, but RABBIT checks one only for an app a \
                 backup to_trash plan moves to the Trash; the administrator command never reads \
                 it, so it would guard nothing",
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
        // text safe there, but a real package id never needs more than this,
        // and one that starts with a letter or digit can't pass for an
        // option of pkgutil's.
        if !is_package_id(id) {
            return Err(format!(
                "macos_pkg_forget id {id:?} must be a full package id such as \
                 org.example.app.pkg"
            ));
        }
    }
    // Only the run's administrator command forgets receipts, and only a plan
    // that removes its files for good has one.
    if !plan.macos_pkg_forget.is_empty() && plan.elevation != UninstallElevation::Required {
        return Err(
            "macos_pkg_forget needs elevation required: only the administrator command \
             forgets installer receipts"
                .to_string(),
        );
    }
    // On macOS the names become one pattern for pgrep, run as root right
    // before the removal, so they may hold nothing a pattern could read as
    // more than a letter. A plan without a platform applies there too, for
    // a package offered on macOS.
    let applies_on_macos = plan.platform != Some(SupportedPlatform::Windows)
        && spec.supported_platforms.contains(&SupportedPlatform::Macos);
    for program in &plan.blocking_programs {
        if program.trim().is_empty() || program.contains(['/', '\\']) {
            return Err(format!(
                "blocking_programs entry {program:?} must be a program name, not a path"
            ));
        }
        if is_screen_reader_program(program) {
            return Err(format!(
                "blocking_programs entry {program:?} is a screen reader, which a blind user \
                 can't close to get past the check"
            ));
        }
        if applies_on_macos
            && !program
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_'))
        {
            return Err(format!(
                "blocking_programs entry {program:?} may only use letters, digits, spaces, - and \
                 _ in a plan for macOS, where it becomes a pattern for pgrep"
            ));
        }
    }
    if plan.platform != Some(SupportedPlatform::Macos)
        && plan
            .post
            .iter()
            .any(|step| matches!(step, UninstallPostStep::RefreshAudioUnits {}))
    {
        return Err(
            "refresh_audio_units needs \"platform\": \"macos\": only macOS has Audio Units"
                .to_string(),
        );
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
    if plan.strategy == UninstallStrategy::RemoveFiles {
        for tree in &plan.remove.trees {
            check_elevated_tree(plan, tree)?;
        }
    }
    Ok(())
}

/// A macOS package id or bundle identifier: ASCII letters, digits, `.`, `-`
/// and `_`, starting with a letter or a digit. Both end up as words in a
/// root shell command or in a check of what root may touch.
fn is_package_id(id: &str) -> bool {
    id.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
}

/// Screen readers by their process names, without `.exe`: JAWS, NVDA,
/// Windows Narrator and macOS VoiceOver.
const SCREEN_READER_PROGRAMS: &[&str] = &["jfw", "nvda", "narrator", "voiceover"];

/// Whether `program`, a `blocking_programs` entry, is a screen reader.
/// RABBIT would refuse to uninstall the package, or to start its Windows
/// installer, while one of these runs, and a blind user can't close it.
fn is_screen_reader_program(program: &str) -> bool {
    let name = program.trim().to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    SCREEN_READER_PROGRAMS.contains(&name)
}

/// Removals that need administrator rights never move root-owned files into
/// the user's backups, and whole folders never go into a backup set at all.
fn validate_elevation_and_backup(plan: &UninstallPlanSpec) -> Result<(), String> {
    let has_trees = !plan.remove.trees.is_empty();
    if plan.strategy == UninstallStrategy::RemoveFiles {
        validate_files_elevation(plan)?;
    } else if plan.elevation == UninstallElevation::Required && plan.backup == UninstallBackup::Move
    {
        return Err(
            "elevation required needs backup none or to_trash: RABBIT never moves \
             files it needs administrator rights for into the user's backups"
                .to_string(),
        );
    }
    if plan.elevation == UninstallElevation::IfNeeded
        && (!has_trees || plan.backup != UninstallBackup::ToTrash)
    {
        return Err("elevation if_needed is only for trees with backup to_trash".to_string());
    }
    // RABBIT moves things to the Trash only on macOS, and only whole
    // objects such as an app bundle.
    if plan.backup == UninstallBackup::ToTrash
        && (!has_trees || plan.platform != Some(SupportedPlatform::Macos))
    {
        return Err(
            "backup to_trash needs remove.trees and \"platform\": \"macos\": only macOS has a \
             Trash RABBIT moves things to"
                .to_string(),
        );
    }
    // RABBIT moves one app to the Trash per package. A second that failed
    // after the first went could not be reported as one outcome, and the
    // administrator command takes only one.
    if plan.backup == UninstallBackup::ToTrash && plan.remove.trees.len() != 1 {
        return Err(
            "backup to_trash moves exactly one app to the Trash, so it needs exactly one \
             remove.trees entry"
                .to_string(),
        );
    }
    if has_trees && plan.backup == UninstallBackup::Move {
        return Err(
            "remove.trees can't be moved into a backup set; use backup to_trash or none"
                .to_string(),
        );
    }
    Ok(())
}

/// What a `remove_files` plan may ask of administrator rights. Only macOS
/// removes files with them, in the run's one administrator command, and
/// that command takes only whole folders: it never handles files one by
/// one, and nothing a receipt or a settings list names ever reaches it. It
/// deletes for good, so `elevation: required` goes with `backup: none`, and
/// nothing else deletes for good, so `backup: none` goes with it too. The
/// one thing root moves rather than deletes is an app RABBIT can't move to
/// the Trash itself, which is what `if_needed` is for.
fn validate_files_elevation(plan: &UninstallPlanSpec) -> Result<(), String> {
    let elevated = plan.elevation != UninstallElevation::Never;
    if elevated && plan.platform != Some(SupportedPlatform::Macos) {
        return Err(format!(
            "a remove_files plan with elevation {} needs \"platform\": \"macos\": only macOS \
             removes files with administrator rights",
            elevation_name(plan.elevation)
        ));
    }
    if elevated {
        let remove = &plan.remove;
        let one_by_one = [
            ("receipt_files", remove.receipt_files),
            ("matching_user_plugins", remove.matching_user_plugins),
            ("paths", !remove.paths.is_empty()),
            ("dirs_if_empty", !remove.dirs_if_empty.is_empty()),
            ("vendor_added_files", remove.vendor_added_files.is_some()),
            ("registry_cleanup", !plan.registry_cleanup.is_empty()),
        ];
        if let Some((field, _)) = one_by_one.iter().find(|(_, used)| *used) {
            return Err(format!(
                "a remove_files plan with elevation {} removes only remove.trees, so it may not \
                 use {field}: root never handles files one by one, nor anything a receipt names",
                elevation_name(plan.elevation)
            ));
        }
    }
    match (plan.elevation, plan.backup) {
        (UninstallElevation::Required, UninstallBackup::Move | UninstallBackup::ToTrash) => Err(
            "elevation required needs backup none: the administrator command deletes what it \
             removes for good, and RABBIT never moves files it needs administrator rights for \
             into the user's backups; only an if_needed app goes to the Trash"
                .to_string(),
        ),
        (UninstallElevation::Never, UninstallBackup::None) => Err(
            "backup none needs elevation required: only the administrator command deletes for \
             good; anything else goes into a backup set or the Trash"
                .to_string(),
        ),
        _ => Ok(()),
    }
}

/// The trees root may touch, once their paths have passed [`check_path`]:
/// - `elevation: required` deletes each one for good, so each must be an
///   absolute macOS path naming something directly in one of the fixed
///   system folders, never a token such as `{home}` or `{resource}`; one
///   directly in `/Applications` must be an `.app`;
/// - `elevation: if_needed` may move REAPER's app into the user's Trash, so
///   each tree must be `{app_path}`, an `.app` with the bundle identifier
///   the app must have.
fn check_elevated_tree(plan: &UninstallPlanSpec, tree: &UninstallTree) -> Result<(), String> {
    let shown = tree.path.as_str();
    let parsed = tree.path.parse()?;
    let is_app = tree
        .must_end_with
        .as_deref()
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".app"));
    match plan.elevation {
        UninstallElevation::Never => Ok(()),
        UninstallElevation::Required => {
            if parsed.root != UninstallPathRoot::MacosAbsolute {
                return Err(format!(
                    "with elevation required, remove.trees entry {shown:?} must be an absolute \
                     macOS path: root never resolves a token"
                ));
            }
            let prefix = MACOS_ABSOLUTE_PREFIXES.iter().find(|prefix| {
                parsed.components.len() > prefix.len() && parsed.components.starts_with(prefix)
            });
            if prefix.is_none_or(|prefix| parsed.components.len() != prefix.len() + 1) {
                return Err(format!(
                    "with elevation required, remove.trees entry {shown:?} must name something \
                     directly in /Applications, /Library/Application Support or a \
                     /Library/Audio/Plug-Ins format folder, not deeper"
                ));
            }
            let in_applications =
                prefix.is_some_and(|prefix| prefix.len() == 1 && prefix[0] == "Applications");
            if in_applications && !is_app {
                return Err(format!(
                    "with elevation required, remove.trees entry {shown:?} sits directly in \
                     /Applications, so it needs must_end_with \".app\""
                ));
            }
            Ok(())
        }
        UninstallElevation::IfNeeded => {
            let app_path = parsed.root == UninstallPathRoot::Token(UninstallPathToken::AppPath)
                && parsed.components.is_empty();
            if !app_path || !is_app || tree.bundle_id.is_none() {
                return Err(format!(
                    "with elevation if_needed, remove.trees entry {shown:?} must be {{app_path}} \
                     with must_end_with \".app\" and a bundle_id: only an app RABBIT has \
                     checked goes to the Trash with administrator rights"
                ));
            }
            Ok(())
        }
    }
}

/// The rules for `remove.vendor_added_files`. The files sit outside the
/// resource folder, so the patterns must keep well away from it:
/// - only a Windows `remove_files` plan of a package that needs a standard
///   REAPER, whose resource folder is then `{appdata}/REAPER`;
/// - each pattern starts with `{appdata}` or `{localappdata}`, spells out
///   the folder after it, which may not be `REAPER`, and names nothing
///   called RABBIT. This rule is what keeps the patterns out of the
///   resource folder. No name may end in a dot or a space, which Windows
///   drops, so `REAPER.` can't stand in for `REAPER`;
/// - each pattern lies below an `install_snapshot` root, written the same
///   way up to there, because RABBIT records added files only below those
///   roots. The roots don't keep out of the resource folder (the JAWS
///   scripts list `{appdata}/REAPER/UserPlugins`); the rule above does. A
///   `*` in the root matches only a `*` in the pattern.
///
/// A `*` may stand for a whole name at any level, as in a snapshot root.
/// The run-time guard checks every matched file again.
fn validate_vendor_added_files(
    spec: &EmbeddedPackageSpec,
    plan: &UninstallPlanSpec,
) -> Result<(), String> {
    let Some(vendor_added) = &plan.remove.vendor_added_files else {
        return Ok(());
    };
    if plan.strategy != UninstallStrategy::RemoveFiles
        || plan.platform != Some(SupportedPlatform::Windows)
    {
        return Err(
            "remove.vendor_added_files needs a remove_files plan with \"platform\": \"windows\": \
             only RABBIT's Windows installs record the files an installer adds"
                .to_string(),
        );
    }
    if !spec.requires_standard_install {
        return Err(
            "remove.vendor_added_files needs requires_standard_install: only then is the \
             resource folder {appdata}/REAPER, which the patterns keep out of"
                .to_string(),
        );
    }
    if vendor_added.files.is_empty() {
        return Err("remove.vendor_added_files needs at least one pattern in files".to_string());
    }
    if vendor_added.by_hand_note_key.is_empty() {
        return Err(
            "remove.vendor_added_files needs a by_hand_note_key saying how to remove the files \
             that stay"
                .to_string(),
        );
    }
    for pattern in &vendor_added.files {
        check_vendor_added_pattern(spec, pattern)
            .map_err(|message| format!("remove.vendor_added_files.files: {message}"))?;
    }
    Ok(())
}

fn check_vendor_added_pattern(
    spec: &EmbeddedPackageSpec,
    pattern: &UninstallPath,
) -> Result<(), String> {
    let shown = pattern.as_str();
    let parsed = pattern.parse_pattern()?;
    let token = match parsed.root {
        UninstallPathRoot::Token(
            token @ (UninstallPathToken::Appdata | UninstallPathToken::Localappdata),
        ) => token,
        _ => {
            return Err(format!(
                "{shown:?} must start with {{appdata}} or {{localappdata}}"
            ));
        }
    };
    if parsed
        .components
        .iter()
        .any(|component| ends_in_dot_or_space(component))
    {
        return Err(format!(
            "{shown:?} may not end a name with a dot or a space, which Windows drops: \
             {{appdata}}/REAPER. is {{appdata}}/REAPER"
        ));
    }
    match parsed.components.first() {
        Some(first) if !first.contains('*') => {
            if first.eq_ignore_ascii_case("REAPER") {
                return Err(format!(
                    "{shown:?} may not reach into a folder called REAPER right below {{appdata}} \
                     or {{localappdata}}: {{appdata}}/REAPER is the resource folder of a \
                     standard REAPER, whose files only the receipt names"
                ));
            }
        }
        _ => {
            return Err(format!(
                "{shown:?} must spell out the folder right after {{{}}}",
                token.name()
            ));
        }
    }
    if has_rabbit_component(&parsed) {
        return Err(format!(
            "{shown:?} reaches into RABBIT's own folder, which holds receipts, backups and logs"
        ));
    }
    let covered = spec
        .uninstall
        .install_snapshot
        .iter()
        .any(|root| snapshot_root_covers(root, &parsed));
    if !covered {
        return Err(format!(
            "{shown:?} must lie below one of the package's install_snapshot roots, written the \
             same way up to there: RABBIT records added files only below those"
        ));
    }
    Ok(())
}

/// Whether `pattern` lies below the snapshot root `root`, with at least one
/// more component. A `*` in the root matches only a `*`.
fn snapshot_root_covers(root: &UninstallPath, pattern: &ParsedUninstallPath<'_>) -> bool {
    let Ok(root) = root.parse_pattern() else {
        return false;
    };
    pattern.components.len() > root.components.len() && starts_with_ignoring_case(&root, pattern)
}

/// Whether `path` is `prefix` or lies below it: the same root, then each of
/// the prefix's components, compared as text ignoring ASCII case.
fn starts_with_ignoring_case(
    prefix: &ParsedUninstallPath<'_>,
    path: &ParsedUninstallPath<'_>,
) -> bool {
    prefix.root == path.root
        && path
            .components
            .get(..prefix.components.len())
            .is_some_and(|start| {
                start
                    .iter()
                    .zip(&prefix.components)
                    .all(|(in_path, in_prefix)| in_path.eq_ignore_ascii_case(in_prefix))
            })
}

/// Whether any component is called RABBIT, whose folder holds receipts,
/// backups and logs.
fn has_rabbit_component(parsed: &ParsedUninstallPath<'_>) -> bool {
    parsed
        .components
        .iter()
        .any(|component| component.eq_ignore_ascii_case(RABBIT_FOLDER))
}

/// Windows drops a trailing dot or space from every name in a path, so a
/// name that ends in one is a second spelling of a shorter name.
fn ends_in_dot_or_space(name: &str) -> bool {
    name.ends_with(['.', ' '])
}

/// The rules for `machine_cleanup`, which the elevated helper carries out
/// with administrator rights, so everything it may touch is named here
/// outright:
/// - only a Windows `remove_files` plan with `elevation: never` and
///   `backup: move`, of a package that needs a standard REAPER: the plan's
///   own files go into a backup set as the user, and the cleanup is the one
///   step with administrator rights;
/// - the folder is one exact folder below `{program_files_x86}`, and
///   `files` are plain file names, each once;
/// - the key is one HKLM key in the 32-bit view, and every value it may
///   hold names the folder or one of `files` in it. Exactly one names the
///   folder: RABBIT checks the folder through it.
fn validate_machine_cleanup(
    spec: &EmbeddedPackageSpec,
    plan: &UninstallPlanSpec,
) -> Result<(), String> {
    let Some(cleanup) = &plan.machine_cleanup else {
        return Ok(());
    };
    if plan.strategy != UninstallStrategy::RemoveFiles
        || plan.platform != Some(SupportedPlatform::Windows)
    {
        return Err(
            "machine_cleanup needs a remove_files plan with \"platform\": \"windows\": it \
             removes what a Windows installer leaves in Program Files and HKLM"
                .to_string(),
        );
    }
    if plan.elevation != UninstallElevation::Never || plan.backup != UninstallBackup::Move {
        return Err(
            "machine_cleanup needs elevation never and backup move: the plan's own files go into \
             a backup set without administrator rights, and only the cleanup uses them"
                .to_string(),
        );
    }
    if !spec.requires_standard_install {
        return Err(
            "machine_cleanup needs requires_standard_install: the folder and key serve the whole \
             computer, so only a standard REAPER's uninstall removes them"
                .to_string(),
        );
    }

    check_path(plan, &cleanup.dir, MACHINE_CLEANUP_DIR)?;
    let dir = cleanup.dir.parse()?;
    let program_files_x86 = UninstallPathRoot::Token(UninstallPathToken::ProgramFilesX86);
    if dir.root != program_files_x86 {
        return Err(format!(
            "machine_cleanup.dir: {:?} must start with {{program_files_x86}}, the one folder a \
             32-bit installer writes to",
            cleanup.dir.as_str()
        ));
    }
    if dir
        .components
        .iter()
        .any(|component| ends_in_dot_or_space(component))
    {
        return Err(format!(
            "machine_cleanup.dir: {:?} may not end a name with a dot or a space, which Windows \
             drops",
            cleanup.dir.as_str()
        ));
    }

    if cleanup.files.is_empty() {
        return Err("machine_cleanup.files needs at least one file name".to_string());
    }
    for (index, name) in cleanup.files.iter().enumerate() {
        if !is_plain_file_name(name) {
            return Err(format!(
                "machine_cleanup.files entry {name:?} must be a plain file name: no folder, no \
                 *, no : and nothing else Windows reads another way"
            ));
        }
        if cleanup.files[..index]
            .iter()
            .any(|other| other.eq_ignore_ascii_case(name))
        {
            return Err(format!(
                "machine_cleanup.files lists {name:?} twice, ignoring case"
            ));
        }
    }

    let registry = &cleanup.registry;
    if registry.hive != RegistryHive::Hklm {
        return Err(
            "machine_cleanup.registry may only name an HKLM key, which only administrators can \
             change"
                .to_string(),
        );
    }
    check_registry_key("machine_cleanup.registry", &registry.key)?;
    // The 32-bit view is the only one. A new view must be thought through
    // here first, so this match has no catch-all.
    match registry.view {
        RegistryViewSpec::Bit32 => {}
    }
    if registry.values.is_empty() {
        return Err(
            "machine_cleanup.registry.values needs the values the key holds, one of them naming \
             the folder"
                .to_string(),
        );
    }
    let mut naming_the_folder = 0;
    for (index, (name, path)) in registry.values.iter().enumerate() {
        if name.is_empty() {
            return Err(
                "machine_cleanup.registry.values needs value names; RABBIT never checks a key's \
                 default value"
                    .to_string(),
            );
        }
        if registry
            .values
            .keys()
            .take(index)
            .any(|other| other.eq_ignore_ascii_case(name))
        {
            return Err(format!(
                "machine_cleanup.registry.values lists {name:?} twice, ignoring case"
            ));
        }
        check_path(plan, path, MACHINE_CLEANUP_VALUE)?;
        let shown = path.as_str();
        let parsed = path.parse()?;
        let in_folder = starts_with_ignoring_case(&dir, &parsed);
        match parsed.components.get(dir.components.len()..) {
            Some([]) if in_folder => naming_the_folder += 1,
            Some([file])
                if in_folder
                    && cleanup
                        .files
                        .iter()
                        .any(|listed| listed.eq_ignore_ascii_case(file)) => {}
            _ => {
                return Err(format!(
                    "machine_cleanup.registry.values: {name:?} must name machine_cleanup.dir or \
                     one of machine_cleanup.files in it, not {shown:?}"
                ));
            }
        }
    }
    if naming_the_folder != 1 {
        return Err(
            "machine_cleanup.registry.values needs exactly one value naming machine_cleanup.dir: \
             RABBIT checks the folder through it"
                .to_string(),
        );
    }

    if cleanup.by_hand_key.is_empty() {
        return Err(
            "machine_cleanup needs a by_hand_key saying how to remove the package by hand"
                .to_string(),
        );
    }
    Ok(())
}

/// A file name on its own: not empty, no folder, nothing Windows reads as
/// a pattern, a stream or a device, and no trailing dot or space, which
/// Windows drops, so one name can't be spelt two ways. That also refuses
/// `.` and `..`.
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && !ends_in_dot_or_space(name)
        && !name.chars().any(|ch| {
            matches!(
                ch,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '{' | '}'
            ) || ch.is_control()
        })
        && !is_windows_device_name(name)
}

/// Whether Windows may read `name` as a device such as `NUL` or `COM1`.
/// The part before the first dot decides, so `nul.txt` counts too. This
/// errs on the side of refusing: newer Windows versions read fewer names
/// this way.
fn is_windows_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
    let named = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"]
        .iter()
        .any(|device| stem.eq_ignore_ascii_case(device));
    // COM and LPT with one digit, the superscripts 1, 2 and 3 included.
    let numbered = ["COM", "LPT"].iter().any(|device| {
        stem.get(..device.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(device))
            && {
                let mut rest = stem[device.len()..].chars();
                matches!(
                    (rest.next(), rest.next()),
                    (Some('0'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None)
                )
            }
    });
    named || numbered
}

fn elevation_name(elevation: UninstallElevation) -> &'static str {
    match elevation {
        UninstallElevation::Never => "never",
        UninstallElevation::Required => "required",
        UninstallElevation::IfNeeded => "if_needed",
    }
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
    check_success_exit_codes(vendor)?;
    for path in &vendor.report_leftovers {
        check_path(plan, path, REPORT_LEFTOVERS)?;
    }
    for leftover in &vendor.remove_leftovers {
        check_leftover_removal(plan, leftover)?;
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

/// A code in `success_exit_codes` stands for "done", so something else has
/// to say whether the uninstaller really removed anything: `verify_gone`.
/// 0 always counts and NSIS's 1 always means the user cancelled.
fn check_success_exit_codes(vendor: &VendorUninstallerSpec) -> Result<(), String> {
    let codes = &vendor.success_exit_codes;
    if codes.is_empty() {
        return Ok(());
    }
    if vendor.verify_gone.is_empty() {
        return Err(
            "success_exit_codes needs verify_gone: only what is gone afterwards can tell such a \
             code from a failure"
                .to_string(),
        );
    }
    for (index, code) in codes.iter().enumerate() {
        if *code == 0 {
            return Err("success_exit_codes needn't list 0, which always counts".to_string());
        }
        if *code == 1 && vendor.kind == VendorUninstallerKind::Nsis {
            return Err(
                "success_exit_codes may not list 1 for an nsis uninstaller: it means the user \
                 cancelled"
                    .to_string(),
            );
        }
        if codes[..index].contains(code) {
            return Err(format!("success_exit_codes lists {code} twice"));
        }
    }
    Ok(())
}

/// The tokens a `remove_leftovers` path may start with: folders every user
/// shares. The elevated helper that removes leftovers resolves these itself
/// and never reads a per-user folder, which may belong to another account
/// than the one it runs as, and which a standard user could rearrange
/// under an administrator's feet.
const MACHINE_WIDE_TOKENS: [UninstallPathToken; 4] = [
    UninstallPathToken::CommonProgramFiles,
    UninstallPathToken::ProgramFiles,
    UninstallPathToken::ProgramFilesX86,
    UninstallPathToken::ProgramData,
];

fn check_leftover_removal(
    plan: &UninstallPlanSpec,
    leftover: &LeftoverRemoval,
) -> Result<(), String> {
    let path = leftover.path();
    check_path(plan, path, REMOVE_LEFTOVERS)?;
    if plan.elevation != UninstallElevation::Required {
        return Err(
            "remove_leftovers needs elevation required: only the elevated helper removes \
             leftovers"
                .to_string(),
        );
    }
    let machine_wide = matches!(
        path.parse().map(|parsed| parsed.root),
        Ok(UninstallPathRoot::Token(token)) if MACHINE_WIDE_TOKENS.contains(&token)
    );
    if !machine_wide {
        return Err(format!(
            "remove_leftovers: {:?} must start with {{common_program_files}}, \
             {{program_files}}, {{program_files_x86}} or {{program_data}}; the elevated helper \
             never reads per-user folders",
            path.as_str()
        ));
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
    if has_rabbit_component(&parsed) {
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
    has_rabbit_component(parsed)
        || parsed
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
        LeftoverRemoval, MACHINE_WIDE_TOKENS, RegistryHive, RegistryViewSpec, UninstallBackup,
        UninstallPath, UninstallPostStep, UninstallStrategy, UninstallTarget,
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
                            { "path": "{app_path}", "must_end_with": ".app", "name_contains": "REAPER", "bundle_id": "com.cockos.reaper" }
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
                        "remove_leftovers": [
                            { "junction": "{common_program_files}/CLAP/Surge Synth Team/SurgeXTData" },
                            { "junction": "{common_program_files}/VST3/Surge Synth Team/SurgeXTData" },
                            { "empty_dir": "{common_program_files}/CLAP/Surge Synth Team" },
                            { "empty_dir": "{common_program_files}/VST3/Surge Synth Team" }
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
                        "removes_key": "uninstall-summary-surge-macos-removes",
                        "keeps_key": "uninstall-summary-surge-keeps"
                    },
                    "blocking_programs": ["Surge XT", "Surge XT Effects", "surge-xt-cli"],
                    "remove": {
                        "trees": [
                            { "path": "/Applications/Surge XT.app", "must_end_with": ".app", "name_contains": "Surge XT" },
                            { "path": "/Applications/Surge XT Effects.app", "must_end_with": ".app", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/Components/Surge XT.component", "must_end_with": ".component", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/Components/Surge XT Effects.component", "must_end_with": ".component", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/CLAP/Surge XT.clap", "must_end_with": ".clap", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/CLAP/Surge XT Effects.clap", "must_end_with": ".clap", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/LV2/Surge XT.lv2", "must_end_with": ".lv2", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/LV2/Surge XT Effects.lv2", "must_end_with": ".lv2", "name_contains": "Surge XT" },
                            { "path": "/Library/Application Support/Surge XT", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/VST3/Surge XT Effects.vst3", "must_end_with": ".vst3", "name_contains": "Surge XT" },
                            { "path": "/Library/Audio/Plug-Ins/VST3/Surge XT.vst3", "must_end_with": ".vst3", "name_contains": "Surge XT" }
                        ]
                    },
                    "macos_pkg_forget": [
                        "org.surge-synth-team.surge-xt.vst3.pkg",
                        "org.surge-synth-team.surge-xt.component.pkg",
                        "org.surge-synth-team.surge-xt.clap.pkg",
                        "org.surge-synth-team.surge-xt.lv2.pkg",
                        "org.surge-synth-team.surge-xt.app.pkg",
                        "org.surge-synth-team.surge-xt.resources.pkg",
                        "org.surge-synth-team.surge-xt-fx.vst3.pkg",
                        "org.surge-synth-team.surge-xt-fx.component.pkg",
                        "org.surge-synth-team.surge-xt-fx.clap.pkg",
                        "org.surge-synth-team.surge-xt-fx.lv2.pkg",
                        "org.surge-synth-team.surge-xt-fx.app.pkg",
                        "com.surge-synth-team.surge-xt.vst3.pkg",
                        "com.surge-synth-team.surge-xt.component.pkg",
                        "com.surge-synth-team.surge-xt.app.pkg",
                        "com.surge-synth-team.surge-xt.resources.pkg",
                        "com.surge-synth-team.surge-xt-fx.vst3.pkg",
                        "com.surge-synth-team.surge-xt-fx.component.pkg",
                        "com.surge-synth-team.surge-xt-fx.app.pkg"
                    ],
                    "kept_user_data": [
                        { "path": "{documents}/Surge XT", "note_key": "uninstall-kept-surge-user-data" },
                        { "path": "{documents}/Surge Synth Team/Surge XT", "note_key": "uninstall-kept-surge-user-data" },
                        { "path": "{home}/Library/Application Support/Surge Synth Team/Surge XT", "note_key": "uninstall-kept-surge-settings-macos" },
                        { "path": "{home}/Library/Application Support/Surge XT", "note_key": "uninstall-kept-surge-factory-copy-macos" }
                    ],
                    "post": [ { "refresh_audio_units": {} } ]
                }
            ]
        }"#;
        let jaws = r#"{
            "machine_wide_warning": false,
            "install_snapshot": [
                "{appdata}/Freedom Scientific/JAWS/*/Settings/*",
                "{appdata}/REAPER/UserPlugins"
            ],
            "plans": [
                {
                    "platform": "windows",
                    "strategy": "remove_files",
                    "summary": {
                        "removes_key": "uninstall-summary-jaws-scripts-removes",
                        "keeps_key": "uninstall-summary-jaws-scripts-keeps"
                    },
                    "remove": {
                        "paths": [
                            "{appdata}/REAPER/UserPlugins/Reaper_ComAccess32.dll",
                            "{appdata}/REAPER/UserPlugins/Reaper_ComAccess64.dll"
                        ],
                        "vendor_added_files": {
                            "files": ["{appdata}/Freedom Scientific/JAWS/*/Settings/*/*"],
                            "by_hand_note_key": "uninstall-note-jaws-scripts-by-hand"
                        }
                    },
                    "machine_cleanup": {
                        "dir": "{program_files_x86}/Reaper_JawsScripts",
                        "files": ["Uninstall.dat", "Uninstall.exe"],
                        "registry": {
                            "hive": "hklm",
                            "key": "Reaper_JawsScripts",
                            "view": "bit32",
                            "values": {
                                "UninstallDirectory": "{program_files_x86}/Reaper_JawsScripts",
                                "Uninstall.dat": "{program_files_x86}/Reaper_JawsScripts/Uninstall.dat"
                            }
                        },
                        "by_hand_key": "uninstall-by-hand-jaws-scripts"
                    }
                }
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
        assert_eq!(
            mac.remove.trees[0].bundle_id.as_deref(),
            Some("com.cockos.reaper")
        );
        // It is the plan REAPER ships with on macOS, word for word.
        let shipped = embedded_package_manifest()
            .packages
            .into_iter()
            .find(|spec| spec.id == "reaper")
            .expect("REAPER is shipped")
            .uninstall;
        for target in [InstallationKind::Standard, InstallationKind::Portable] {
            assert_eq!(
                reaper.plan_for(Platform::MacOs, target),
                shipped.plan_for(Platform::MacOs, target),
                "{target:?}"
            );
        }

        let surge = demo_spec(r#""requires_standard_install": true,"#, surge).uninstall;
        let mac = surge
            .plan_for(Platform::MacOs, InstallationKind::Standard)
            .expect("Surge XT on macOS has a plan");
        assert_eq!(mac.post, [UninstallPostStep::RefreshAudioUnits {}]);
        // It is the plan Surge XT ships with on macOS, word for word.
        let shipped = embedded_package_manifest()
            .packages
            .into_iter()
            .find(|spec| spec.id == "surge-xt")
            .expect("Surge XT is shipped")
            .uninstall;
        for target in [InstallationKind::Standard, InstallationKind::Portable] {
            assert_eq!(
                surge.plan_for(Platform::MacOs, target),
                shipped.plan_for(Platform::MacOs, target),
                "{target:?}"
            );
        }
        let vendor = surge
            .plan_for(Platform::Windows, InstallationKind::Standard)
            .and_then(|plan| plan.vendor_uninstaller.as_ref())
            .expect("Surge XT on Windows has a vendor uninstaller");
        assert_eq!(
            vendor.remove_leftovers,
            [
                LeftoverRemoval::Junction(UninstallPath::new(
                    "{common_program_files}/CLAP/Surge Synth Team/SurgeXTData"
                )),
                LeftoverRemoval::Junction(UninstallPath::new(
                    "{common_program_files}/VST3/Surge Synth Team/SurgeXTData"
                )),
                LeftoverRemoval::EmptyDir(UninstallPath::new(
                    "{common_program_files}/CLAP/Surge Synth Team"
                )),
                LeftoverRemoval::EmptyDir(UninstallPath::new(
                    "{common_program_files}/VST3/Surge Synth Team"
                )),
            ]
        );

        let jaws = demo_spec(
            r#""requires_standard_install": true, "supported_platforms": ["windows"],"#,
            jaws,
        )
        .uninstall;
        assert!(!jaws.machine_wide_warning);
        let plan = jaws
            .plan_for(Platform::Windows, InstallationKind::Standard)
            .expect("the JAWS scripts have a plan on Windows");
        let vendor_added = plan
            .remove
            .vendor_added_files
            .as_ref()
            .expect("the plan removes recorded script files");
        assert_eq!(
            vendor_added.files,
            [UninstallPath::new(
                "{appdata}/Freedom Scientific/JAWS/*/Settings/*/*"
            )]
        );
        let cleanup = plan
            .machine_cleanup
            .as_ref()
            .expect("the plan cleans up the uninstaller folder and key");
        assert_eq!(
            cleanup.dir,
            UninstallPath::new("{program_files_x86}/Reaper_JawsScripts")
        );
        assert_eq!(cleanup.files, ["Uninstall.dat", "Uninstall.exe"]);
        assert_eq!(cleanup.registry.hive, RegistryHive::Hklm);
        assert_eq!(cleanup.registry.view, RegistryViewSpec::Bit32);
        assert_eq!(
            cleanup.registry.values.keys().collect::<Vec<_>>(),
            ["Uninstall.dat", "UninstallDirectory"]
        );
        // The by-hand texts are Fluent keys like the rest, so the shipped
        // manifest's test checks them once the manifest names them.
        let keys = jaws.fluent_keys();
        for key in [
            "uninstall-note-jaws-scripts-by-hand",
            "uninstall-by-hand-jaws-scripts",
        ] {
            assert!(keys.contains(&key), "{key} is missing from {keys:?}");
        }
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

    /// An NSIS block that takes `codes` as success exit codes, with or
    /// without a `verify_gone` path.
    fn nsis_with_success_codes(codes: &str, verify_gone: bool) -> String {
        let verify_gone = if verify_gone {
            r#", "verify_gone": ["{app_path}"]"#
        } else {
            ""
        };
        vendor_plans(&format!(
            r#""vendor_uninstaller": {{
                "kind": "nsis",
                "locate": [{{ "registry": {{ "hive": "hklm", "key": "Demo", "value": "UninstallString" }} }}],
                "install_location": {{
                    "registry": {{ "hive": "hklm", "key": "Demo", "value": "InstallLocation" }},
                    "must_equal": "{{app_dir}}"
                }},
                "must_be_in": {{ "under": "{{program_files}}" }},
                "timeout_seconds": 600,
                "success_exit_codes": {codes}{verify_gone}
            }}"#
        ))
    }

    #[test]
    fn success_exit_codes_need_verify_gone_and_never_take_0_or_a_cancel() {
        accepts("", &nsis_with_success_codes("[1223]", true));
        accepts("", &nsis_with_success_codes("[]", false));
        rejects(
            "",
            &nsis_with_success_codes("[1223]", false),
            "success_exit_codes needs verify_gone",
        );
        rejects("", &nsis_with_success_codes("[0]", true), "needn't list 0");
        rejects(
            "",
            &nsis_with_success_codes("[1]", true),
            "may not list 1 for an nsis uninstaller",
        );
        rejects(
            "",
            &nsis_with_success_codes("[1223, 5, 1223]", true),
            "lists 1223 twice",
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

    /// A macOS `remove_files` plan with `fields`, next to a Windows plan so
    /// the package is covered everywhere.
    fn macos_remove_plans(fields: &str) -> String {
        plans(&[
            r#"{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }"#,
            &format!(
                r#"{{ "platform": "macos", "strategy": "remove_files", {SUMMARY}, {fields} }}"#
            ),
        ])
    }

    /// The tree Surge XT's macOS plan removes first, as an elevated tree.
    const ELEVATED_APP_TREE: &str = r#""elevation": "required", "backup": "none",
        "remove": { "trees": [{ "path": "/Applications/Demo.app", "must_end_with": ".app", "name_contains": "Demo" }] }"#;

    /// REAPER's app as an `if_needed` tree, with `tree` as its fields.
    fn if_needed_app(tree: &str) -> String {
        macos_remove_plans(&format!(
            r#""elevation": "if_needed", "backup": "to_trash",
               "remove": {{ "trees": [{{ {tree} }}] }}"#
        ))
    }

    const REAPER_APP_TREE: &str = r#""path": "{app_path}", "must_end_with": ".app", "name_contains": "REAPER", "bundle_id": "com.cockos.reaper""#;

    #[test]
    fn elevation_never_moves_files_into_the_users_backups() {
        rejects(
            "",
            &macos_remove_plans(
                r#""elevation": "required",
                   "remove": { "trees": [{ "path": "/Applications/Demo.app", "must_end_with": ".app" }] }"#,
            ),
            "elevation required needs backup none",
        );
        rejects(
            "",
            &macos_remove_plans(
                r#""elevation": "if_needed", "backup": "none",
                   "remove": { "trees": [{ "path": "{app_path}", "must_end_with": ".app", "bundle_id": "com.example.demo" }] }"#,
            ),
            "if_needed is only for trees with backup to_trash",
        );
        rejects(
            "",
            &macos_remove_plans(
                r#""remove": { "trees": [{ "path": "{resource}/Demo.app", "must_end_with": ".app" }] }"#,
            ),
            "can't be moved into a backup set",
        );
        // A vendor's uninstaller keeps its own rule: no backup set either.
        rejects(
            "",
            &vendor_plans(&format!(r#""elevation": "required", {INNO_IN_PLACE}"#)),
            "elevation required needs backup none or to_trash",
        );
    }

    /// Rule 1: only macOS removes files with administrator rights; on
    /// Windows a vendor's uninstaller does that.
    #[test]
    fn an_elevated_remove_files_plan_is_for_macos_only() {
        accepts("", &macos_remove_plans(ELEVATED_APP_TREE));
        accepts("", &if_needed_app(REAPER_APP_TREE));
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""elevation": "required", "backup": "none", "remove": { "receipt_files": true }"#,
            )]),
            "elevation required needs \"platform\": \"macos\"",
        );
        rejects(
            "",
            &windows_remove_plans(r#""elevation": "required", "backup": "none""#),
            "elevation required needs \"platform\": \"macos\"",
        );
        rejects(
            "",
            &plans(&[&remove_plan(&format!(
                r#""elevation": "if_needed", "backup": "to_trash",
                   "remove": {{ "trees": [{{ {REAPER_APP_TREE} }}] }}"#
            ))]),
            "elevation if_needed needs \"platform\": \"macos\"",
        );
    }

    /// Rule 2: root never handles files one by one, and never anything a
    /// receipt or a list of files names.
    #[test]
    fn an_elevated_remove_files_plan_removes_only_trees() {
        accepts("", &macos_remove_plans(ELEVATED_APP_TREE));
        for (other, field) in [
            (r#""receipt_files": true"#, "receipt_files"),
            (r#""matching_user_plugins": true"#, "matching_user_plugins"),
            (
                r#""paths": ["/Library/Application Support/Demo/demo.txt"]"#,
                "paths",
            ),
            (
                r#""dirs_if_empty": ["/Library/Application Support/Demo"]"#,
                "dirs_if_empty",
            ),
        ] {
            rejects(
                "",
                &macos_remove_plans(&format!(
                    r#""elevation": "required", "backup": "none",
                       "remove": {{ {other},
                           "trees": [{{ "path": "/Applications/Demo.app", "must_end_with": ".app" }}] }}"#
                )),
                &format!("removes only remove.trees, so it may not use {field}"),
            );
        }
        rejects(
            "",
            &macos_remove_plans(&format!(
                r#""elevation": "if_needed", "backup": "to_trash",
                   "remove": {{ "receipt_files": true, "trees": [{{ {REAPER_APP_TREE} }}] }}"#
            )),
            "elevation if_needed removes only remove.trees, so it may not use receipt_files",
        );
    }

    /// Rule 3: what root deletes is named outright, directly in one of the
    /// fixed system folders, and an app there must be an `.app`.
    #[test]
    fn an_elevated_tree_sits_directly_in_a_system_folder() {
        for path in [
            "/Library/Audio/Plug-Ins/VST3/Demo.vst3",
            "/Library/Audio/Plug-Ins/Components/Demo.component",
            "/Library/Audio/Plug-Ins/CLAP/Demo.clap",
            "/Library/Audio/Plug-Ins/LV2/Demo.lv2",
            "/Library/Application Support/Demo",
        ] {
            accepts(
                "",
                &macos_remove_plans(&format!(
                    r#""elevation": "required", "backup": "none",
                       "remove": {{ "trees": [{{ "path": "{path}", "name_contains": "Demo" }}] }}"#
                )),
            );
        }
        for (path, expected) in [
            ("{home}/Library/Demo", "must be an absolute macOS path"),
            ("{resource}/Demo", "must be an absolute macOS path"),
            ("{documents}/Demo", "must be an absolute macOS path"),
            ("/Library/Audio/Plug-Ins/VST3/Demo/Demo.vst3", "not deeper"),
            ("/Library/Application Support/Demo/Data", "not deeper"),
            ("/Applications/Demo/Demo.app", "not deeper"),
        ] {
            rejects(
                "",
                &macos_remove_plans(&format!(
                    r#""elevation": "required", "backup": "none",
                       "remove": {{ "trees": [{{ "path": "{path}", "name_contains": "Demo" }}] }}"#
                )),
                expected,
            );
        }
        rejects(
            "",
            &macos_remove_plans(
                r#""elevation": "required", "backup": "none",
                   "remove": { "trees": [{ "path": "{app_path}", "must_end_with": ".app" }] }"#,
            ),
            "must be an absolute macOS path",
        );
        rejects(
            "",
            &macos_remove_plans(
                r#""elevation": "required", "backup": "none",
                   "remove": { "trees": [{ "path": "/Applications/Demo.app", "name_contains": "Demo" }] }"#,
            ),
            "needs must_end_with \".app\"",
        );
        // Only for what root deletes: a tree that goes to the Trash as the
        // user may sit deeper.
        accepts(
            "",
            &macos_remove_plans(
                r#""backup": "to_trash",
                   "remove": { "trees": [{ "path": "/Library/Application Support/Demo/Data", "name_contains": "Data" }] }"#,
            ),
        );
    }

    /// Rule 4, as decision 32 has it: root moves only REAPER's app into the
    /// Trash, and only one RABBIT can tell is that app.
    #[test]
    fn an_if_needed_tree_is_the_app_with_its_bundle_id() {
        accepts("", &if_needed_app(REAPER_APP_TREE));
        for tree in [
            r#""path": "{app_path}", "must_end_with": ".app", "name_contains": "REAPER""#,
            r#""path": "{app_path}", "name_contains": "REAPER", "bundle_id": "com.cockos.reaper""#,
            r#""path": "{app_path}", "must_end_with": ".bundle", "bundle_id": "com.cockos.reaper""#,
            r#""path": "/Applications/REAPER.app", "must_end_with": ".app", "bundle_id": "com.cockos.reaper""#,
            r#""path": "{resource}/REAPER.app", "must_end_with": ".app", "bundle_id": "com.cockos.reaper""#,
        ] {
            rejects(
                "",
                &if_needed_app(tree),
                "must be {app_path} with must_end_with \".app\" and a bundle_id",
            );
        }
    }

    /// Rule 5: only macOS has a Trash RABBIT moves things to, and only whole
    /// objects go there.
    #[test]
    fn backup_to_trash_needs_trees_on_macos() {
        accepts(
            "",
            &macos_remove_plans(
                r#""backup": "to_trash",
                   "remove": { "trees": [{ "path": "{app_path}", "must_end_with": ".app" }] }"#,
            ),
        );
        rejects(
            "",
            &macos_remove_plans(r#""backup": "to_trash", "remove": { "receipt_files": true }"#),
            "backup to_trash needs remove.trees and \"platform\": \"macos\"",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""backup": "to_trash",
                   "remove": { "trees": [{ "path": "{app_path}", "must_end_with": ".app" }] }"#,
            )]),
            "backup to_trash needs remove.trees and \"platform\": \"macos\"",
        );
        rejects(
            "",
            &vendor_plans(&format!(r#""backup": "to_trash", {INNO_IN_PLACE}"#)),
            "backup to_trash needs remove.trees and \"platform\": \"macos\"",
        );
    }

    /// One app per package goes to the Trash: a second that failed after
    /// the first went couldn't be reported as one outcome, and root's
    /// command moves only one.
    #[test]
    fn a_trash_plan_moves_exactly_one_app() {
        accepts("", &if_needed_app(REAPER_APP_TREE));
        for elevation in ["never", "if_needed"] {
            rejects(
                "",
                &macos_remove_plans(&format!(
                    r#""elevation": "{elevation}", "backup": "to_trash",
                       "remove": {{ "trees": [
                           {{ {REAPER_APP_TREE} }},
                           {{ "path": "/Applications/Other.app", "must_end_with": ".app", "name_contains": "Other" }}
                       ] }}"#
                )),
                "backup to_trash moves exactly one app to the Trash",
            );
        }
    }

    /// Rule 6: only root deletes for good, so a plan without administrator
    /// rights can't delete anything for good either.
    #[test]
    fn backup_none_needs_elevation_required() {
        accepts("", &macos_remove_plans(ELEVATED_APP_TREE));
        rejects(
            "",
            &macos_remove_plans(
                r#""backup": "none",
                   "remove": { "trees": [{ "path": "/Applications/Demo.app", "must_end_with": ".app" }] }"#,
            ),
            "backup none needs elevation required",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""backup": "none", "remove": { "receipt_files": true }"#,
            )]),
            "backup none needs elevation required",
        );
    }

    /// Rule 7: receipts are forgotten only by the administrator command,
    /// and an id can't pass for one of pkgutil's options.
    #[test]
    fn pkg_forget_needs_elevation_and_ids_that_start_with_a_letter_or_digit() {
        let forgetting = |ids: &str| {
            macos_remove_plans(&format!(
                r#"{ELEVATED_APP_TREE}, "macos_pkg_forget": [{ids}]"#
            ))
        };
        accepts("", &forgetting(r#""org.example.demo.pkg", "9demo.pkg""#));
        for id in ["-demo.pkg", ".demo.pkg", "_demo.pkg", ""] {
            rejects(
                "",
                &forgetting(&format!("\"{id}\"")),
                "must be a full package id",
            );
        }
        rejects(
            "",
            &macos_remove_plans(
                r#""backup": "to_trash",
                   "remove": { "trees": [{ "path": "{app_path}", "must_end_with": ".app" }] },
                   "macos_pkg_forget": ["org.example.demo.pkg"]"#,
            ),
            "macos_pkg_forget needs elevation required",
        );
    }

    /// Rule 8: on macOS the program names become one pattern for pgrep.
    #[test]
    fn program_names_for_macos_are_plain_words() {
        let blocking = |names: &str| {
            macos_remove_plans(&format!(
                r#"{ELEVATED_APP_TREE}, "blocking_programs": [{names}]"#
            ))
        };
        accepts(
            "",
            &blocking(r#""Surge XT", "Surge XT Effects", "demo_2-x""#),
        );
        for name in ["Surge XT.app", "Surge*", "Surge|XT", "(Surge)", "Surgé XT"] {
            rejects("", &blocking(&format!("\"{name}\"")), "a pattern for pgrep");
        }
        // A plan for every platform runs on macOS too.
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""remove": { "receipt_files": true }, "blocking_programs": ["demo.exe"]"#,
            )]),
            "a pattern for pgrep",
        );
        // Windows keeps its file names, in its own plans and in the plans of
        // a package offered only there.
        accepts(
            "",
            &windows_remove_plans(r#""blocking_programs": ["Surge XT.exe"]"#),
        );
        accepts(
            r#""supported_platforms": ["windows"],"#,
            &plans(&[&remove_plan(
                r#""remove": { "receipt_files": true }, "blocking_programs": ["demo.exe"]"#,
            )]),
        );
    }

    /// A plan never names a screen reader among the programs to close: the
    /// package couldn't be uninstalled, nor its Windows installer started,
    /// while it runs, and a blind user can't close it.
    #[test]
    fn program_names_never_name_a_screen_reader() {
        for name in ["jfw.exe", "JFW.EXE", "nvda.exe", "Narrator.exe", "nvda"] {
            rejects(
                "",
                &windows_remove_plans(&format!(r#""blocking_programs": ["Demo.exe", "{name}"]"#)),
                "is a screen reader",
            );
        }
        rejects(
            "",
            &macos_remove_plans(&format!(
                r#"{ELEVATED_APP_TREE}, "blocking_programs": ["VoiceOver"]"#
            )),
            "is a screen reader",
        );
        accepts(
            "",
            &windows_remove_plans(r#""blocking_programs": ["Surge XT.exe", "nvda_demo.exe"]"#),
        );
    }

    /// Rule 9: only macOS has Audio Units to refresh.
    #[test]
    fn refresh_audio_units_is_for_macos_plans() {
        accepts(
            "",
            &macos_remove_plans(&format!(
                r#"{ELEVATED_APP_TREE}, "post": [{{ "refresh_audio_units": {{}} }}]"#
            )),
        );
        rejects(
            "",
            &windows_remove_plans(r#""post": [{ "refresh_audio_units": {} }]"#),
            "refresh_audio_units needs \"platform\": \"macos\"",
        );
        rejects(
            "",
            &plans(&[&remove_plan(
                r#""remove": { "receipt_files": true }, "post": [{ "refresh_audio_units": {} }]"#,
            )]),
            "refresh_audio_units needs \"platform\": \"macos\"",
        );
    }

    #[test]
    fn a_bundle_id_is_a_full_identifier() {
        accepts(
            "",
            &if_needed_app(
                r#""path": "{app_path}", "must_end_with": ".app", "bundle_id": "com.cockos.reaper-arm_64""#,
            ),
        );
        for bundle_id in ["", "-com.cockos.reaper", "com.cockos.reaper;", "com cockos"] {
            rejects(
                "",
                &if_needed_app(&format!(
                    r#""path": "{{app_path}}", "must_end_with": ".app", "bundle_id": "{bundle_id}""#
                )),
                "which must be a full identifier",
            );
        }
    }

    /// Only the Trash route compares a bundle's identifier. Root's delete
    /// never reads one, so a manifest may not give one there and seem to
    /// guard root with it.
    #[test]
    fn a_bundle_id_goes_only_where_rabbit_checks_it() {
        accepts("", &if_needed_app(REAPER_APP_TREE));
        accepts(
            "",
            &macos_remove_plans(&format!(
                r#""elevation": "never", "backup": "to_trash",
                   "remove": {{ "trees": [{{ {REAPER_APP_TREE} }}] }}"#
            )),
        );
        rejects(
            "",
            &macos_remove_plans(
                r#""elevation": "required", "backup": "none",
                   "remove": { "trees": [{ "path": "/Applications/Surge XT.app", "must_end_with": ".app",
                       "name_contains": "Surge XT", "bundle_id": "org.example.surge" }] }"#,
            ),
            "checks one only for an app a backup to_trash plan",
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

    /// An Inno plan whose uninstaller leaves `leftovers` behind for RABBIT
    /// to remove, with `elevation` spliced in.
    fn leftover_plans(elevation: &str, leftovers: &str) -> String {
        vendor_plans(&format!(
            r#"{elevation} "vendor_uninstaller": {{
                "kind": "inno",
                "locate": [{{ "registry": {{ "hive": "hklm", "key": "Demo_is1", "value": "UninstallString" }} }}],
                "must_be_in": {{ "dir": "{{program_data}}/Demo/uninstall", "only_files": "unins000.*" }},
                "timeout_seconds": 600,
                "remove_leftovers": [{leftovers}]
            }}"#
        ))
    }

    const ELEVATED: &str = r#""elevation": "required", "backup": "none","#;

    #[test]
    fn leftovers_are_junctions_or_empty_folders_in_machine_wide_folders() {
        accepts(
            "",
            &leftover_plans(
                ELEVATED,
                r#"{ "junction": "{common_program_files}/CLAP/Demo/Data" },
                   { "empty_dir": "{common_program_files}/CLAP/Demo" },
                   { "empty_dir": "{program_files}/Demo" },
                   { "junction": "{program_data}/Demo/Link" }"#,
            ),
        );
        // Only these two kinds, each naming one path.
        for unknown in [
            r#"{ "tree": "{common_program_files}/CLAP/Demo" }"#,
            r#"{ "file": "{common_program_files}/CLAP/Demo/a.dll" }"#,
            r#"{ "junction": "{common_program_files}/CLAP/Demo/Data", "empty_dir": "{common_program_files}/CLAP/Demo" }"#,
            r#""{common_program_files}/CLAP/Demo""#,
        ] {
            let source = format!(
                r#"{{ "id": "demo", "display_name": "Demo", "display_name_key": "a",
                     "display_description_key": "b", "recommended": false,
                     "uninstall": {} }}"#,
                leftover_plans(ELEVATED, unknown)
            );
            assert!(
                parse_embedded_package_spec(&source).is_err(),
                "{unknown} should not parse"
            );
        }
    }

    #[test]
    fn leftovers_need_the_elevated_helper_and_a_folder_below_a_machine_wide_root() {
        rejects(
            "",
            &leftover_plans(
                "",
                r#"{ "junction": "{common_program_files}/CLAP/Demo/Data" }"#,
            ),
            "remove_leftovers needs elevation required",
        );
        for (leftover, expected) in [
            // Per-user folders: the elevated helper never reads them.
            (
                r#"{ "junction": "{clap_dir}/Demo/Data" }"#,
                "must start with {common_program_files}",
            ),
            (
                r#"{ "empty_dir": "{resource}/Demo" }"#,
                "must start with {common_program_files}",
            ),
            (
                r#"{ "empty_dir": "{localappdata}/Demo" }"#,
                "must start with {common_program_files}",
            ),
            (
                r#"{ "empty_dir": "{app_dir}/Demo" }"#,
                "only for comparison",
            ),
            (
                r#"{ "junction": "{app_path}" }"#,
                "allowed only in remove.trees and verify_gone",
            ),
            // The root itself, RABBIT's folder, patterns and odd paths.
            (r#"{ "empty_dir": "{program_files}" }"#, "is a folder root"),
            (
                r#"{ "empty_dir": "{common_program_files}/Demo/RABBIT" }"#,
                "RABBIT's own folder",
            ),
            (
                r#"{ "junction": "{common_program_files}/Demo/*Data" }"#,
                "may not use *",
            ),
            (
                r#"{ "junction": "{common_program_files}/Demo/../Data" }"#,
                "may not use . or ..",
            ),
            (
                r#"{ "junction": "/Library/Audio/Plug-Ins/VST3/Demo" }"#,
                "allowed only in plans with",
            ),
        ] {
            rejects("", &leftover_plans(ELEVATED, leftover), expected);
        }
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
            &macos_remove_plans(
                r#""backup": "to_trash", "remove": { "trees": [{ "path": "{resource}/Demo*.app", "must_end_with": ".app" }] }"#,
            ),
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
            &macos_remove_plans(
                r#""backup": "to_trash", "remove": { "trees": [{ "path": "{app_path}/Contents", "must_end_with": ".app" }] }"#,
            ),
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

    /// A macOS plan deleting the tree at `path` with administrator rights,
    /// next to a Windows plan. The tree must end with the path's own
    /// extension, if it has one, as an app in /Applications must.
    fn macos_tree_plans(path: &str) -> String {
        let must_end_with = std::path::Path::new(path)
            .extension()
            .map(|extension| format!(r#", "must_end_with": ".{}""#, extension.to_string_lossy()))
            .unwrap_or_default();
        macos_remove_plans(&format!(
            r#""elevation": "required", "backup": "none",
               "remove": {{ "trees": [{{ "path": "{path}", "name_contains": "Demo"{must_end_with} }}] }}"#
        ))
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
            &path_plan("/Library/Application Support/Demo/demo.txt"),
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
                &macos_remove_plans(&format!(
                    r#""backup": "to_trash", "remove": {{ "trees": [{tree}] }}"#
                )),
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
            &macos_remove_plans(&format!(
                r#"{ELEVATED_APP_TREE}, "macos_pkg_forget": ["org.example.demo.pkg; rm -rf /"]"#
            )),
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
        // The 32-bit Program Files folder is a folder to list like the
        // others, on Windows only.
        accepts(windows_only, &with_roots(r#""{program_files_x86}/Demo""#));
        rejects(
            r#""supported_platforms": ["macos"],"#,
            &with_roots(r#""{program_files_x86}/Demo""#),
            "{program_files_x86} exists only on windows, where the package isn't offered",
        );
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

    /// A package offered on Windows only that needs a standard REAPER, like
    /// the JAWS scripts.
    const STANDARD_WINDOWS: &str =
        r#""requires_standard_install": true, "supported_platforms": ["windows"],"#;

    /// The snapshot root the JAWS scripts' manifest lists.
    const JAWS_ROOT: &str = r#""{appdata}/Freedom Scientific/JAWS/*/Settings/*""#;

    /// An uninstall block listing the snapshot roots `roots` and one
    /// Windows `remove_files` plan that removes the recorded files matching
    /// `files`.
    fn vendor_added_plans(roots: &str, files: &str) -> String {
        format!(
            r#"{{ "install_snapshot": [{roots}],
                 "plans": [{{ "platform": "windows", "strategy": "remove_files", {SUMMARY},
                     "remove": {{ "vendor_added_files": {{
                         "files": [{files}], "by_hand_note_key": "test-by-hand" }} }} }}] }}"#
        )
    }

    #[test]
    fn program_files_x86_is_a_windows_machine_wide_folder_token() {
        use super::{UninstallPathToken, UninstallTokenKind};

        let token = UninstallPathToken::from_name("program_files_x86").unwrap();
        assert_eq!(token, UninstallPathToken::ProgramFilesX86);
        assert_eq!(token.kind(), UninstallTokenKind::FolderRoot);
        assert_eq!(
            token.platform(),
            Some(crate::package::SupportedPlatform::Windows)
        );
        assert!(MACHINE_WIDE_TOKENS.contains(&token));

        rejects(
            "",
            &path_plan("{program_files_x86}/Demo/demo.dll"),
            "{program_files_x86} exists only on windows",
        );
        accepts(
            "",
            &windows_remove_plans(r#""keep": ["{program_files_x86}/Demo"]"#),
        );
        rejects(
            "",
            &windows_remove_plans(r#""keep": ["{program_files_x86}/Demo/RABBIT"]"#),
            "RABBIT's own folder",
        );
        // The elevated helper resolves it itself, so leftovers may sit in it.
        accepts(
            "",
            &leftover_plans(ELEVATED, r#"{ "empty_dir": "{program_files_x86}/Demo" }"#),
        );
        rejects(
            "",
            &leftover_plans(ELEVATED, r#"{ "empty_dir": "{program_files_x86}" }"#),
            "is a folder root",
        );
        rejects(
            "",
            &leftover_plans(ELEVATED, r#"{ "empty_dir": "{localappdata}/Demo" }"#),
            "{program_files}, {program_files_x86} or {program_data}",
        );
    }

    #[test]
    fn vendor_added_files_need_install_snapshot_roots_that_cover_them() {
        accepts(
            STANDARD_WINDOWS,
            &vendor_added_plans(
                JAWS_ROOT,
                r#""{appdata}/Freedom Scientific/JAWS/*/Settings/*/*""#,
            ),
        );
        // Literal names compare ignoring case, and a pattern may go deeper.
        accepts(
            STANDARD_WINDOWS,
            &vendor_added_plans(
                JAWS_ROOT,
                r#""{appdata}/freedom scientific/jaws/*/settings/*/reaper*.jss",
                   "{appdata}/Freedom Scientific/JAWS/*/Settings/*/Sub/*""#,
            ),
        );
        // Any of several roots will do.
        accepts(
            STANDARD_WINDOWS,
            &vendor_added_plans(
                &format!(r#""{{appdata}}/Demo/Data", {JAWS_ROOT}"#),
                r#""{appdata}/Freedom Scientific/JAWS/*/Settings/*/*", "{appdata}/Demo/Data/*.ini""#,
            ),
        );
        for (roots, files) in [
            // No root at all: RABBIT records nothing.
            ("", r#""{appdata}/Freedom Scientific/JAWS/*/Settings/*/*""#),
            // The root itself, which names folders, not files below them.
            (
                JAWS_ROOT,
                r#""{appdata}/Freedom Scientific/JAWS/*/Settings/*""#,
            ),
            // A * in the root matches only a *.
            (
                JAWS_ROOT,
                r#""{appdata}/Freedom Scientific/JAWS/2025/Settings/enu/*""#,
            ),
            // Wider than the root.
            (
                JAWS_ROOT,
                r#""{appdata}/Freedom Scientific/*/*/Settings/*/*""#,
            ),
            // Beside it.
            (JAWS_ROOT, r#""{appdata}/Freedom Scientific/Other/*""#),
            // Another token, even with the same folders after it.
            (
                JAWS_ROOT,
                r#""{localappdata}/Freedom Scientific/JAWS/*/Settings/*/*""#,
            ),
        ] {
            rejects(
                STANDARD_WINDOWS,
                &vendor_added_plans(roots, files),
                "must lie below one of the package's install_snapshot roots",
            );
        }
    }

    #[test]
    fn vendor_added_files_start_with_appdata_or_localappdata_and_never_reach_reaper() {
        accepts(
            STANDARD_WINDOWS,
            &vendor_added_plans(
                r#""{localappdata}/Demo/Data""#,
                r#""{localappdata}/Demo/Data/*""#,
            ),
        );
        for (root, file, expected) in [
            (
                "{program_data}/Demo",
                "{program_data}/Demo/*",
                "must start with {appdata} or {localappdata}",
            ),
            (
                "{program_files_x86}/Demo",
                "{program_files_x86}/Demo/*",
                "must start with {appdata} or {localappdata}",
            ),
            (
                "{resource}/Demo",
                "{resource}/Demo/*",
                "must start with {appdata} or {localappdata}",
            ),
            (
                "{documents}/Demo",
                "{documents}/Demo/*",
                "must start with {appdata} or {localappdata}",
            ),
            (
                "{appdata}/Demo",
                "/Library/Application Support/Demo/*",
                "must start with {appdata} or {localappdata}",
            ),
            // The standard resource folder, which holds RABBIT's own folder
            // too, however it is spelt.
            (
                "{appdata}/REAPER/UserPlugins",
                "{appdata}/REAPER/UserPlugins/*",
                "may not reach into a folder called REAPER",
            ),
            (
                "{appdata}/reaper/UserPlugins",
                "{appdata}/reaper/UserPlugins/*",
                "may not reach into a folder called REAPER",
            ),
            (
                "{localappdata}/REAPER/Data",
                "{localappdata}/REAPER/Data/*",
                "may not reach into a folder called REAPER",
            ),
            // Windows drops a trailing dot or space, so these are REAPER too.
            (
                "{appdata}/REAPER./UserPlugins",
                "{appdata}/REAPER./UserPlugins/*",
                "may not end a name with a dot or a space",
            ),
            (
                "{appdata}/REAPER /UserPlugins",
                "{appdata}/REAPER /UserPlugins/*",
                "may not end a name with a dot or a space",
            ),
            (
                "{appdata}/Demo",
                "{appdata}/Demo/Data./*",
                "may not end a name with a dot or a space",
            ),
            (
                "{appdata}/Demo",
                "{appdata}/*/Data/*",
                "must spell out the folder right after {appdata}",
            ),
            (
                "{appdata}/Demo",
                "{appdata}/Demo/RABBIT/*",
                "RABBIT's own folder",
            ),
            (
                "{appdata}/Demo",
                "{appdata}/Demo/rabbit/*",
                "RABBIT's own folder",
            ),
            (
                "{appdata}/Demo",
                "{appdata}/Demo/../x",
                "may not use . or ..",
            ),
            (
                "{appdata}/Demo",
                "{appdata}/Demo/a:b",
                "may not contain ':'",
            ),
        ] {
            rejects(
                STANDARD_WINDOWS,
                &vendor_added_plans(&format!("\"{root}\""), &format!("\"{file}\"")),
                expected,
            );
        }
    }

    #[test]
    fn vendor_added_files_need_a_windows_remove_files_plan_for_a_standard_reaper() {
        let block = r#""vendor_added_files": { "files": ["{appdata}/Demo/Data/*"], "by_hand_note_key": "a" }"#;
        let roots = r#""install_snapshot": ["{appdata}/Demo/Data"],"#;
        rejects(
            r#""supported_platforms": ["windows"],"#,
            &vendor_added_plans(r#""{appdata}/Demo/Data""#, r#""{appdata}/Demo/Data/*""#),
            "remove.vendor_added_files needs requires_standard_install",
        );
        for plans_json in [
            // Every platform, so macOS too.
            format!(
                r#"{{ {roots} "plans": [{{ "strategy": "remove_files", {SUMMARY}, "remove": {{ {block} }} }}] }}"#
            ),
            format!(
                r#"{{ {roots} "plans": [
                    {{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }},
                    {{ "platform": "macos", "strategy": "remove_files", {SUMMARY}, "remove": {{ {block} }} }}
                ] }}"#
            ),
            format!(
                r#"{{ {roots} "plans": [
                    {{ "platform": "windows", "strategy": "vendor_uninstaller", {SUMMARY}, {INNO_IN_PLACE},
                       "remove": {{ {block} }} }},
                    {{ "platform": "macos", "strategy": "not_supported", "note_key": "a" }}
                ] }}"#
            ),
        ] {
            rejects(
                r#""requires_standard_install": true,"#,
                &plans_json,
                "remove.vendor_added_files needs a remove_files plan with \"platform\": \"windows\"",
            );
        }
        rejects(
            STANDARD_WINDOWS,
            &vendor_added_plans(r#""{appdata}/Demo/Data""#, ""),
            "needs at least one pattern in files",
        );
        rejects(
            STANDARD_WINDOWS,
            &format!(
                r#"{{ {roots} "plans": [{{ "platform": "windows", "strategy": "remove_files", {SUMMARY},
                     "remove": {{ "vendor_added_files": {{ "files": ["{{appdata}}/Demo/Data/*"], "by_hand_note_key": "" }} }} }}] }}"#
            ),
            "needs a by_hand_note_key",
        );
        // Root never handles files one by one, these included. The general
        // elevation rules run first, so this is what a macOS plan hears.
        rejects(
            r#""requires_standard_install": true,"#,
            &format!(
                r#"{{ {roots} "plans": [
                    {{ "platform": "windows", "strategy": "not_supported", "note_key": "a" }},
                    {{ "platform": "macos", "strategy": "remove_files", {SUMMARY},
                       "elevation": "required", "backup": "none",
                       "remove": {{ {block},
                           "trees": [{{ "path": "/Applications/Demo.app", "must_end_with": ".app" }}] }} }}
                ] }}"#
            ),
            "may not use vendor_added_files",
        );
        // An elevated Windows plan hears that only macOS removes files
        // with administrator rights; these files go as the user.
        rejects(
            STANDARD_WINDOWS,
            &format!(
                r#"{{ {roots} "plans": [{{ "platform": "windows", "strategy": "remove_files", {SUMMARY},
                     "elevation": "required", "backup": "none", "remove": {{ {block} }} }}] }}"#
            ),
            "only macOS removes files with administrator rights",
        );
    }

    /// A `machine_cleanup` block for the folder `dir` holding `files`, with
    /// `registry` as its key.
    fn cleanup_block(dir: &str, files: &str, registry: &str) -> String {
        format!(
            r#""machine_cleanup": {{ "dir": "{dir}", "files": [{files}], "registry": {registry},
                 "by_hand_key": "test-by-hand" }}"#
        )
    }

    /// The HKLM key `Demo` in the 32-bit view, holding `values`.
    fn cleanup_registry(values: &str) -> String {
        format!(r#"{{ "hive": "hklm", "key": "Demo", "view": "bit32", "values": {{ {values} }} }}"#)
    }

    /// The JAWS scripts' cleanup, for a folder called Demo.
    fn demo_cleanup() -> String {
        cleanup_block(
            "{program_files_x86}/Demo",
            r#""Uninstall.dat", "Uninstall.exe""#,
            &cleanup_registry(
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "Uninstall.dat": "{program_files_x86}/Demo/Uninstall.dat""#,
            ),
        )
    }

    /// An uninstall block whose only plan is a Windows `remove_files` plan
    /// with `fields` and nothing else to remove, for a package offered on
    /// Windows only.
    fn windows_only_remove_plans(fields: &str) -> String {
        plans(&[&format!(
            r#"{{ "platform": "windows", "strategy": "remove_files", {SUMMARY}, {fields} }}"#
        )])
    }

    /// Checks that `uninstall` fails to parse with an error naming
    /// `expected`.
    fn refuses_to_parse(uninstall: &str, expected: &str) {
        let source = format!(
            r#"{{ "id": "demo", "display_name": "Demo", "display_name_key": "a",
                 "display_description_key": "b", "recommended": false,
                 "uninstall": {uninstall} }}"#
        );
        match parse_embedded_package_spec(&source) {
            Ok(_) => panic!("{uninstall} should not parse"),
            Err(error) => assert!(
                error.to_string().contains(expected),
                "expected a parse error containing {expected:?}, got {error}"
            ),
        }
    }

    #[test]
    fn machine_cleanup_needs_a_windows_remove_files_plan_for_a_standard_reaper() {
        let cleanup = demo_cleanup();
        // On its own it is something to remove, and beside other files too.
        accepts(STANDARD_WINDOWS, &windows_only_remove_plans(&cleanup));
        accepts(
            STANDARD_WINDOWS,
            &windows_only_remove_plans(&format!(
                r#""remove": {{ "receipt_files": true }}, {cleanup}"#
            )),
        );
        rejects(
            r#""supported_platforms": ["windows"],"#,
            &windows_only_remove_plans(&cleanup),
            "machine_cleanup needs requires_standard_install",
        );
        for plans_json in [
            plans(&[&format!(
                r#"{{ "strategy": "remove_files", {SUMMARY}, {cleanup} }}"#
            )]),
            macos_remove_plans(&format!(
                r#""remove": {{ "receipt_files": true }}, {cleanup}"#
            )),
            vendor_plans(&format!("{INNO_IN_PLACE}, {cleanup}")),
        ] {
            rejects(
                r#""requires_standard_install": true,"#,
                &plans_json,
                "machine_cleanup needs a remove_files plan with \"platform\": \"windows\"",
            );
        }
        rejects(
            STANDARD_WINDOWS,
            &windows_only_remove_plans(
                &cleanup.replace(r#""by_hand_key": "test-by-hand""#, r#""by_hand_key": """#),
            ),
            "machine_cleanup needs a by_hand_key",
        );
    }

    #[test]
    fn machine_cleanup_keeps_the_plans_own_elevation_never_and_backup_move() {
        let cleanup = demo_cleanup();
        for elevation_and_backup in [
            r#""elevation": "required", "backup": "none""#,
            r#""elevation": "required""#,
            r#""backup": "none""#,
            r#""backup": "to_trash""#,
            r#""elevation": "if_needed", "backup": "to_trash""#,
        ] {
            rejects(
                STANDARD_WINDOWS,
                &windows_only_remove_plans(&format!("{elevation_and_backup}, {cleanup}")),
                "machine_cleanup needs elevation never and backup move",
            );
        }
        accepts(
            STANDARD_WINDOWS,
            &windows_only_remove_plans(&format!(
                r#""elevation": "never", "backup": "move", {cleanup}"#
            )),
        );
    }

    #[test]
    fn machine_cleanup_uses_only_program_files_x86_and_hklm_in_the_32_bit_view() {
        let in_folder = |dir: &str| {
            cleanup_block(
                dir,
                r#""Uninstall.exe""#,
                &cleanup_registry(&format!(r#""UninstallDirectory": "{dir}""#)),
            )
        };
        accepts(
            STANDARD_WINDOWS,
            &windows_only_remove_plans(&in_folder("{program_files_x86}/Demo")),
        );
        accepts(
            STANDARD_WINDOWS,
            &windows_only_remove_plans(&in_folder("{program_files_x86}/Demo/Uninstaller")),
        );
        for (dir, expected) in [
            (
                "{program_files}/Demo",
                "must start with {program_files_x86}",
            ),
            (
                "{common_program_files}/Demo",
                "must start with {program_files_x86}",
            ),
            ("{program_data}/Demo", "must start with {program_files_x86}"),
            ("{appdata}/Demo", "must start with {program_files_x86}"),
            ("{resource}/Demo", "must start with {program_files_x86}"),
            (
                "{program_files_x86}",
                "is a folder root, which is never removed",
            ),
            // The field name shows the folder failed, not the value that
            // repeats it.
            (
                "{program_files_x86}/Demo*",
                r#"machine_cleanup.dir: "{program_files_x86}/Demo*" may not use *"#,
            ),
            (
                "{program_files_x86}/Demo/../Other",
                r#"machine_cleanup.dir: "{program_files_x86}/Demo/../Other" may not use . or .."#,
            ),
            (
                "{program_files_x86}/RABBIT",
                r#"machine_cleanup.dir: "{program_files_x86}/RABBIT" reaches into RABBIT's own folder"#,
            ),
            (
                "{program_files_x86}/Demo.",
                r#"machine_cleanup.dir: "{program_files_x86}/Demo." may not end a name with a dot or a space"#,
            ),
            (
                "{program_files_x86}/Demo /Uninstaller",
                "may not end a name with a dot or a space",
            ),
            ("{app_dir}/Demo", "only for comparison"),
            ("{app_path}", "allowed only in remove.trees and verify_gone"),
            ("/Applications/Demo.app", "allowed only in plans with"),
        ] {
            rejects(
                STANDARD_WINDOWS,
                &windows_only_remove_plans(&in_folder(dir)),
                expected,
            );
        }

        let with_key = |hive: &str, key: &str| {
            cleanup_block(
                "{program_files_x86}/Demo",
                r#""Uninstall.exe""#,
                &format!(
                    r#"{{ "hive": "{hive}", "key": "{key}", "view": "bit32",
                         "values": {{ "UninstallDirectory": "{{program_files_x86}}/Demo" }} }}"#
                ),
            )
        };
        rejects(
            STANDARD_WINDOWS,
            &windows_only_remove_plans(&with_key("hkcu", "Demo")),
            "may only name an HKLM key",
        );
        for key in [r"Software\\Demo", "Demo/Other", ""] {
            rejects(
                STANDARD_WINDOWS,
                &windows_only_remove_plans(&with_key("hklm", key)),
                "must be one key name under the Uninstall key",
            );
        }
        // Only the 32-bit view exists, and it must be named.
        refuses_to_parse(
            &windows_only_remove_plans(&demo_cleanup().replace("bit32", "bit64")),
            "unknown variant `bit64`",
        );
        refuses_to_parse(
            &windows_only_remove_plans(&demo_cleanup().replace(r#""view": "bit32","#, "")),
            "missing field `view`",
        );
    }

    #[test]
    fn machine_cleanup_files_are_plain_unique_names() {
        let listing = |files: &str| {
            windows_only_remove_plans(&cleanup_block(
                "{program_files_x86}/Demo",
                files,
                &cleanup_registry(r#""UninstallDirectory": "{program_files_x86}/Demo""#),
            ))
        };
        accepts(STANDARD_WINDOWS, &listing(r#""Uninstall.exe""#));
        accepts(
            STANDARD_WINDOWS,
            &listing(r#""Uninstall.dat", "Uninstall.exe", "unins000.dat""#),
        );
        // Names that only start like a device are plain.
        accepts(
            STANDARD_WINDOWS,
            &listing(r#""Console.exe", "COM10.dat", "nullsoft.ini", "LPT.txt""#),
        );
        for name in [
            "",
            "Sub/Uninstall.exe",
            r"Sub\\Uninstall.exe",
            "*.exe",
            "Uninstall.exe:stream",
            "C:",
            "a?b",
            "a|b",
            "a<b",
            ".",
            "..",
            "Uninstall.exe.",
            "Uninstall.exe ",
            "{program_files_x86}",
            // Devices, with or without an extension.
            "NUL",
            "con.txt",
            "Aux.log.dat",
            "COM1",
            "lpt9.dat",
            "CONOUT$",
            "COM\u{b9}",
        ] {
            rejects(
                STANDARD_WINDOWS,
                &listing(&format!("\"{name}\"")),
                "must be a plain file name",
            );
        }
        rejects(
            STANDARD_WINDOWS,
            &listing(r#""Uninstall.exe", "uninstall.EXE""#),
            "lists \"uninstall.EXE\" twice, ignoring case",
        );
        rejects(
            STANDARD_WINDOWS,
            &listing(""),
            "machine_cleanup.files needs at least one file name",
        );
    }

    #[test]
    fn machine_cleanup_registry_values_point_into_its_folder_and_one_names_it() {
        let holding = |values: &str| {
            windows_only_remove_plans(&cleanup_block(
                "{program_files_x86}/Demo",
                r#""Uninstall.dat", "Uninstall.exe""#,
                &cleanup_registry(values),
            ))
        };
        accepts(
            STANDARD_WINDOWS,
            &holding(r#""UninstallDirectory": "{program_files_x86}/Demo""#),
        );
        // Names in the folder compare ignoring case, as Windows does.
        accepts(
            STANDARD_WINDOWS,
            &holding(
                r#""UninstallDirectory": "{program_files_x86}/demo",
                   "Uninstall.dat": "{program_files_x86}/DEMO/uninstall.DAT",
                   "UninstallString": "{program_files_x86}/Demo/Uninstall.exe""#,
            ),
        );
        for (values, expected) in [
            ("", "needs the values the key holds"),
            (
                r#""Uninstall.dat": "{program_files_x86}/Demo/Uninstall.dat""#,
                "needs exactly one value naming machine_cleanup.dir",
            ),
            (
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "InstallLocation": "{program_files_x86}/Demo""#,
                "needs exactly one value naming machine_cleanup.dir",
            ),
            (
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "Other": "{program_files_x86}/Other""#,
                "must name machine_cleanup.dir or one of machine_cleanup.files in it",
            ),
            (
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "Other": "{program_files_x86}/Demo/Other.exe""#,
                "must name machine_cleanup.dir or one of machine_cleanup.files in it",
            ),
            (
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "Other": "{program_files_x86}/Demo/Sub/Uninstall.dat""#,
                "must name machine_cleanup.dir or one of machine_cleanup.files in it",
            ),
            (
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "Other": "{program_files_x86}""#,
                "must name machine_cleanup.dir or one of machine_cleanup.files in it",
            ),
            // The native Program Files folder holds another folder, even
            // one with the same name.
            (
                r#""UninstallDirectory": "{program_files}/Demo""#,
                "must name machine_cleanup.dir or one of machine_cleanup.files in it",
            ),
            (
                r#""UninstallDirectory": "{appdata}/Demo""#,
                "must name machine_cleanup.dir or one of machine_cleanup.files in it",
            ),
            (
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "Other": "{program_files_x86}/Demo/*.dat""#,
                r#"machine_cleanup.registry.values: "{program_files_x86}/Demo/*.dat" may not use *"#,
            ),
            (
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "Other": "{program_files_x86}/Demo/../Demo""#,
                r#"machine_cleanup.registry.values: "{program_files_x86}/Demo/../Demo" may not use . or .."#,
            ),
            (r#""": "{program_files_x86}/Demo""#, "needs value names"),
            (
                r#""UninstallDirectory": "{program_files_x86}/Demo",
                   "uninstalldirectory": "{program_files_x86}/Demo""#,
                "lists \"uninstalldirectory\" twice, ignoring case",
            ),
        ] {
            rejects(STANDARD_WINDOWS, &holding(values), expected);
        }
        // JSON allows a name twice with the same spelling, and serde would
        // keep only the last, so the manifest doesn't even parse.
        refuses_to_parse(
            &holding(
                r#""UninstallDirectory": "{program_files_x86}/Other",
                   "UninstallDirectory": "{program_files_x86}/Demo""#,
            ),
            "machine_cleanup.registry.values lists \"UninstallDirectory\" twice",
        );
    }

    #[test]
    fn a_manual_plan_carries_no_vendor_added_files_or_machine_cleanup() {
        let roots = r#""install_snapshot": ["{appdata}/Demo/Data"],"#;
        for fields in [
            r#""remove": { "vendor_added_files": { "files": ["{appdata}/Demo/Data/*"], "by_hand_note_key": "a" } }"#.to_string(),
            demo_cleanup(),
        ] {
            for strategy in ["manual", "not_supported"] {
                rejects(
                    STANDARD_WINDOWS,
                    &format!(
                        r#"{{ {roots} "plans": [{{ "platform": "windows", "strategy": "{strategy}",
                             "note_key": "a", {fields} }}] }}"#
                    ),
                    "changes nothing",
                );
            }
        }
    }

    #[test]
    fn machine_wide_warning_can_be_turned_off_only_for_machine_wide_packages() {
        let with_warning = |value: &str| {
            format!(
                r#"{{ "machine_wide_warning": {value},
                     "plans": [{{ "strategy": "not_supported", "note_key": "a" }}] }}"#
            )
        };
        let machine_wide = r#""requires_standard_install": true,"#;
        accepts(machine_wide, &with_warning("false"));
        accepts(machine_wide, &with_warning("true"));
        accepts("", &with_warning("true"));
        rejects(
            "",
            &with_warning("false"),
            "machine_wide_warning false needs requires_standard_install",
        );

        // On unless a manifest says otherwise, and written out only when off.
        let default = demo_spec(
            "",
            &plans(&[r#"{ "strategy": "not_supported", "note_key": "a" }"#]),
        )
        .uninstall;
        assert!(default.machine_wide_warning);
        let written = serde_json::to_value(&default).unwrap();
        assert!(written.get("machine_wide_warning").is_none(), "{written}");
        let off = demo_spec(machine_wide, &with_warning("false")).uninstall;
        assert!(!off.machine_wide_warning);
        let written = serde_json::to_value(&off).unwrap();
        assert_eq!(written["machine_wide_warning"], serde_json::json!(false));
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
            r#"{ "plans": [{ "strategy": "not_supported", "note_key": "a" }], "machine_wide_warnings": false }"#,
            r#"{ "plans": [{ "strategy": "remove_files", "remove": { "vendor_added_file": { "files": [] } } }] }"#,
            r#"{ "plans": [{ "strategy": "remove_files",
                 "remove": { "vendor_added_files": { "by_hand_note": "a", "files": [] } } }] }"#,
            r#"{ "plans": [{ "strategy": "remove_files", "machine_cleanups": {} }] }"#,
            r#"{ "plans": [{ "strategy": "remove_files",
                 "machine_cleanup": { "file": ["Uninstall.exe"], "dir": "{program_files_x86}/Demo" } }] }"#,
            r#"{ "plans": [{ "strategy": "remove_files",
                 "machine_cleanup": { "dir": "{program_files_x86}/Demo", "files": ["Uninstall.exe"],
                     "registry": { "value": {}, "hive": "hklm", "key": "Demo", "view": "bit32" } } }] }"#,
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
