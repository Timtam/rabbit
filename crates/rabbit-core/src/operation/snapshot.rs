//! The install-time snapshot: which files a package's own installer added or
//! changed in folders no receipt path covers.
//!
//! The JAWS scripts' vendor installer copies its script files into the JAWS
//! version and language folders it picks, and nothing tells RABBIT which.
//! So for a package whose manifest lists folders under
//! `uninstall.install_snapshot`, RABBIT lists them right before the
//! installer runs and again afterwards, and records every file that is new
//! or changed on the receipt, with its hash and size
//! ([`VendorAddedFile`]). Files that were there before and didn't change
//! are not recorded. A later uninstall can then remove the recorded files
//! that still hold exactly what the installer left, except those marked as
//! there before, which may have held the user's own changes.
//!
//! The first listing reads only sizes and modification times, which is quick
//! even for a few thousand files. Only the files that come out new or
//! changed are hashed, plus the few an earlier install recorded, so RABBIT
//! can tell whether those still held what that install left.
//!
//! An installer that runs with administrator rights, as the JAWS scripts'
//! always does, isn't the only thing writing meanwhile. JAWS keeps running
//! while its scripts install, and the files it creates in those folders are
//! new too. They belong to the user's account, though, while the files the
//! elevated installer creates belong to the Administrators group. So after
//! such an installer a new file is recorded only when Administrators,
//! SYSTEM or TrustedInstaller own it
//! ([`InstallSnapshot::new_files_only_from_administrators`]). The owner is
//! read here, while it still tells who created the file: a later restore
//! from a backup copies files back, and the copies belong to whoever
//! restored them. A changed file is recorded as described at
//! [`InstallSnapshot::record`] whoever owns it, because writing over a file
//! doesn't change its owner.
//!
//! Owners don't always tell the installer's files from JAWS's. With UAC
//! turned off both can run with full administrator rights, and a policy can
//! make the account that created a file its owner even when it ran
//! elevated. The rule then records every new file, as without it, or leaves
//! the installer's files out too. It only ever leaves files out, so a later
//! uninstall removes fewer files, never more.
//!
//! Nothing here may fail an install. By the time the second listing runs
//! the files are on disk, and the record only helps a later uninstall. So
//! every problem leaves files out of the record rather than putting wrong
//! ones in: a file RABBIT can't hash isn't recorded, nor is a new file whose
//! owner RABBIT can't read when the owner counts, and nothing below a
//! folder RABBIT couldn't list before the run is recorded either, because
//! otherwise everything in it, the user's own files included, would look
//! new afterwards. Links are never followed, so a junction can't pull files
//! from somewhere else into the record.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::hash::sha256_file;
use crate::package::uninstall::{UninstallPath, UninstallPathRoot, wildcard_matches_ignoring_case};
use crate::receipt::VendorAddedFile;
use crate::uninstall::paths::{UninstallRoots, is_in_rabbit_folder};

/// What the snapshot folders held right before an installer ran.
#[derive(Debug)]
pub(super) struct InstallSnapshot {
    roots: Vec<UninstallPath>,
    paths: UninstallRoots,
    before: Listing,
    /// The files an earlier install recorded, each with its hash right
    /// before this run: `None` when it was gone or couldn't be read.
    recorded: Vec<(VendorAddedFile, Option<String>)>,
    /// Whether a new file's owner is Administrators, SYSTEM or
    /// TrustedInstaller, when only those count; see
    /// [`Self::new_files_only_from_administrators`].
    owner_is_trusted: Option<fn(&Path) -> io::Result<bool>>,
}

impl InstallSnapshot {
    /// List `roots`, resolved through `paths`, before the installer runs.
    /// `recorded` is what the receipt already holds from earlier installs.
    pub(super) fn take(
        roots: &[UninstallPath],
        paths: UninstallRoots,
        recorded: &[VendorAddedFile],
    ) -> Self {
        let before = Listing::of(roots, &paths);
        let recorded = recorded
            .iter()
            .map(|file| (file.clone(), hash_regular_file(&file.path)))
            .collect();
        Self {
            roots: roots.to_vec(),
            paths,
            before,
            recorded,
            owner_is_trusted: None,
        }
    }

    /// Record a file that is new since the first listing only when
    /// `owner_is_trusted` says Administrators, SYSTEM or TrustedInstaller
    /// own it. Any other answer, an error included, leaves it out. Use it
    /// for an installer that runs elevated: Administrators normally own the
    /// files it creates, while the files other programs create meanwhile
    /// belong to the user. Changed files are recorded as before, whoever
    /// owns them.
    pub(super) fn new_files_only_from_administrators(
        self,
        owner_is_trusted: fn(&Path) -> io::Result<bool>,
    ) -> Self {
        Self {
            owner_is_trusted: Some(owner_is_trusted),
            ..self
        }
    }

    /// List the roots again after the installer ran, and return the record
    /// for the receipt: what earlier installs recorded, with every file this
    /// run added or changed put in or brought up to date.
    ///
    /// A changed file is marked `existed_before`, because what the installer
    /// replaced may have been the user's, unless an earlier install recorded
    /// it and it still held exactly that: then it was the installer's own
    /// file all along and keeps its earlier mark. Recorded files this run
    /// didn't touch stay as they were, even when the user changed or deleted
    /// them since, because an uninstall checks each file's hash anyway.
    ///
    /// When only new files Administrators own count, a new file someone
    /// else owns is left out. If an earlier install recorded it, the earlier
    /// entry stays as it was, as for any recorded file this run didn't
    /// write: a later uninstall removes the file only if it still holds
    /// exactly what that install left.
    pub(super) fn record(&self) -> Vec<VendorAddedFile> {
        let after = Listing::of(&self.roots, &self.paths);
        let mut record: BTreeMap<PathBuf, VendorAddedFile> = self
            .recorded
            .iter()
            .map(|(file, _)| (file.path.clone(), file.clone()))
            .collect();
        for (path, stamp) in &after.files {
            if self.before.is_unknown(path) {
                continue;
            }
            let existed_before = match self.before.files.get(path) {
                None if !self.new_file_counts(path) => continue,
                None => false,
                Some(before) if before == stamp => continue,
                Some(_) => self.earlier_mark(path).unwrap_or(true),
            };
            let Some(sha256) = hash_regular_file(path) else {
                continue;
            };
            record.insert(
                path.clone(),
                VendorAddedFile {
                    path: path.clone(),
                    sha256,
                    size: stamp.size,
                    existed_before,
                },
            );
        }
        record.into_values().collect()
    }

    /// The `existed_before` mark an earlier install gave `path`, if it
    /// recorded the file and the file still held exactly that right before
    /// this run.
    fn earlier_mark(&self, path: &Path) -> Option<bool> {
        self.recorded
            .iter()
            .find(|(file, hash_before)| {
                file.path == path && hash_before.as_deref() == Some(file.sha256.as_str())
            })
            .map(|(file, _)| file.existed_before)
    }

    /// Whether a new file at `path` counts as the installer's by its owner:
    /// always, unless only files Administrators own count, and then only
    /// when its owner could be read and is one of them.
    fn new_file_counts(&self, path: &Path) -> bool {
        self.owner_is_trusted
            .is_none_or(|owner_is_trusted| matches!(owner_is_trusted(path), Ok(true)))
    }
}

/// The size and modification time of a file, which is all the listing
/// compares. Writing a file changes at least one of them. The one exception
/// is an installer putting back the very file it wrote before, because NSIS
/// gives each file the time it had when the installer was built, and then
/// nothing changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    size: u64,
    modified: Option<SystemTime>,
}

impl FileStamp {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            size: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

/// Every regular file below the snapshot roots, and what couldn't be read.
#[derive(Debug, Default)]
struct Listing {
    files: BTreeMap<PathBuf, FileStamp>,
    /// Folders that couldn't be listed and files whose details couldn't be
    /// read, for a reason other than not being there.
    unreadable: Vec<PathBuf>,
}

impl Listing {
    fn of(roots: &[UninstallPath], paths: &UninstallRoots) -> Self {
        let mut listing = Self::default();
        for root in roots {
            // Embedded manifests are validated when they load, so a root
            // that doesn't parse, or names no folder token, can only come
            // from a hand-built spec. Listing nothing is the safe answer.
            let Ok(parsed) = root.parse_pattern() else {
                continue;
            };
            let UninstallPathRoot::Token(token) = parsed.root else {
                continue;
            };
            // A token that stands for nothing here, such as `{appdata}`
            // off Windows, simply has nothing to list.
            for base in paths.resolve(token) {
                for path in listing.expand(&base, &parsed.components) {
                    listing.add(&path, paths);
                }
            }
        }
        listing
    }

    /// Whether `path` is, or is below, something this listing couldn't read.
    fn is_unknown(&self, path: &Path) -> bool {
        self.unreadable
            .iter()
            .any(|unreadable| path.starts_with(unreadable))
    }

    /// Every path `components` lead to from `base`, each `*` component
    /// matched against the names in its folder.
    fn expand(&mut self, base: &Path, components: &[&str]) -> Vec<PathBuf> {
        let mut current = vec![base.to_path_buf()];
        for component in components {
            let mut next = Vec::new();
            for folder in &current {
                if !component.contains('*') {
                    next.push(folder.join(component));
                    continue;
                }
                for entry in self.read_dir(folder) {
                    let is_link = entry
                        .file_type()
                        .map(|file_type| file_type.is_symlink())
                        .unwrap_or(true);
                    let matches = entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| wildcard_matches_ignoring_case(component, name));
                    if matches && !is_link {
                        next.push(entry.path());
                    }
                }
            }
            current = next;
        }
        current
    }

    /// Put `path` into the listing: a file as it is, a folder with
    /// everything below it.
    fn add(&mut self, path: &Path, paths: &UninstallRoots) {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {}
            Ok(metadata) if metadata.is_dir() => self.walk(path, paths),
            Ok(metadata) if metadata.is_file() => {
                self.files
                    .insert(path.to_path_buf(), FileStamp::of(&metadata));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => self.unreadable.push(path.to_path_buf()),
        }
    }

    fn walk(&mut self, folder: &Path, paths: &UninstallRoots) {
        // RABBIT's own folder holds receipts, backups and logs, never a
        // package's files, whatever a root's `*` reaches.
        if is_in_rabbit_folder(paths, folder) {
            return;
        }
        for entry in self.read_dir(folder) {
            let path = entry.path();
            match entry.file_type() {
                Ok(file_type) if file_type.is_symlink() => {}
                Ok(file_type) if file_type.is_dir() => self.walk(&path, paths),
                Ok(file_type) if file_type.is_file() => match entry.metadata() {
                    Ok(metadata) => {
                        self.files.insert(path, FileStamp::of(&metadata));
                    }
                    Err(_) => self.unreadable.push(path),
                },
                Ok(_) => {}
                Err(_) => self.unreadable.push(path),
            }
        }
    }

    /// The entries of `folder`. A folder that isn't there has none; one
    /// that can't be read, or read to the end, is noted as unreadable.
    fn read_dir(&mut self, folder: &Path) -> Vec<fs::DirEntry> {
        let entries = match fs::read_dir(folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
            Err(_) => {
                self.unreadable.push(folder.to_path_buf());
                return Vec::new();
            }
        };
        let mut found = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => found.push(entry),
                Err(_) => {
                    self.unreadable.push(folder.to_path_buf());
                    break;
                }
            }
        }
        found
    }
}

/// The SHA-256 of `path` if it is a regular file RABBIT can read, not a
/// link.
fn hash_regular_file(path: &Path) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    sha256_file(path).ok()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use tempfile::{TempDir, tempdir};

    use super::InstallSnapshot;
    use crate::hash::sha256_bytes;
    use crate::model::Platform;
    use crate::package::uninstall::UninstallPath;
    use crate::package::{PACKAGE_JAWS_SCRIPTS, package_specs_by_id};
    use crate::receipt::VendorAddedFile;
    use crate::uninstall::paths::UninstallRoots;

    /// A fake `%APPDATA%` with a standard REAPER resource folder inside it,
    /// as on a real Windows machine.
    struct Fixture {
        _dir: TempDir,
        appdata: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let appdata = dir.path().join("AppData").join("Roaming");
            fs::create_dir_all(&appdata).unwrap();
            Self { _dir: dir, appdata }
        }

        fn paths(&self) -> UninstallRoots {
            UninstallRoots {
                appdata: Some(self.appdata.clone()),
                ..UninstallRoots::new(&self.appdata.join("REAPER"))
            }
        }

        /// The absolute path of `relative` below `%APPDATA%`.
        fn path(&self, relative: &str) -> PathBuf {
            self.appdata.join(relative)
        }

        /// Write `contents` to `relative` below `%APPDATA%`, making its
        /// folders, and return the absolute path.
        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
            path
        }

        fn jaws(&self, relative: &str) -> String {
            format!("Freedom Scientific/JAWS/{relative}")
        }
    }

    /// The roots the shipped JAWS scripts manifest declares, so these tests
    /// cover exactly what a real install lists.
    fn jaws_roots() -> Vec<UninstallPath> {
        package_specs_by_id(Platform::Windows)[PACKAGE_JAWS_SCRIPTS]
            .uninstall
            .install_snapshot
            .clone()
    }

    fn entry(path: &Path, contents: &str, existed_before: bool) -> VendorAddedFile {
        VendorAddedFile {
            path: path.to_path_buf(),
            sha256: sha256_bytes(contents.as_bytes()),
            size: contents.len() as u64,
            existed_before,
        }
    }

    fn find<'a>(record: &'a [VendorAddedFile], path: &Path) -> Option<&'a VendorAddedFile> {
        record.iter().find(|file| file.path == path)
    }

    #[test]
    fn the_jaws_scripts_manifest_lists_the_jaws_settings_and_reaper_plugin_folders() {
        let roots: Vec<String> = jaws_roots()
            .iter()
            .map(|root| root.as_str().to_string())
            .collect();
        assert_eq!(
            roots,
            [
                "{appdata}/Freedom Scientific/JAWS/*/Settings/*",
                "{appdata}/REAPER/UserPlugins",
            ]
        );
    }

    #[test]
    fn added_and_changed_files_are_recorded_and_untouched_ones_are_not() {
        let fixture = Fixture::new();
        let user_keys = fixture.write(&fixture.jaws("2026/Settings/deu/default.jkm"), "mine");
        let reaper_keys = fixture.write(
            &fixture.jaws("2026/Settings/deu/reaper.jkm"),
            "my reaper keys",
        );
        let other_version =
            fixture.write(&fixture.jaws("2025/Settings/enu/notepad.jss"), "notepad");
        let migration = fixture.write(&fixture.jaws("2026/SilentMigration.ini"), "a");
        let osara = fixture.write("REAPER/UserPlugins/reaper_osara64.dll", "osara");

        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &[]);

        // What the vendor installer does: new script files in two JAWS
        // versions and a subfolder, one existing file replaced, the REAPER
        // side DLL, and a write outside every root.
        let script = fixture.write(&fixture.jaws("2026/Settings/deu/reaper.jss"), "script");
        let helper = fixture.write(&fixture.jaws("2026/Settings/deu/snow_uia.jsb"), "helper");
        let sound = fixture.write(
            &fixture.jaws("2026/Settings/deu/Sounds/reaper_record.wav"),
            "wave",
        );
        let older = fixture.write(&fixture.jaws("2025/Settings/enu/reaper.jss"), "old script");
        fs::write(&reaper_keys, "the scripts' own keys").unwrap();
        let dll = fixture.write("REAPER/UserPlugins/Reaper_ComAccess64.dll", "com");
        fs::write(&migration, "rewritten by something else").unwrap();

        let record = snapshot.record();

        for (path, contents) in [
            (&script, "script"),
            (&helper, "helper"),
            (&sound, "wave"),
            (&older, "old script"),
            (&dll, "com"),
        ] {
            assert_eq!(
                find(&record, path),
                Some(&entry(path, contents, false)),
                "{} should be recorded as added",
                path.display()
            );
        }
        assert_eq!(
            find(&record, &reaper_keys),
            Some(&entry(&reaper_keys, "the scripts' own keys", true)),
            "a replaced file is recorded, marked as there before"
        );
        for untouched in [&user_keys, &other_version, &osara, &migration] {
            assert!(
                find(&record, untouched).is_none(),
                "{} must not be recorded",
                untouched.display()
            );
        }
        assert_eq!(record.len(), 6, "{record:#?}");
    }

    /// No JAWS folder, no plug-in folder, not even a resolvable `{appdata}`:
    /// none of it is an error, and folders the installer creates count as
    /// new.
    #[test]
    fn missing_roots_are_handled() {
        let fixture = Fixture::new();
        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &[]);
        let script = fixture.write(&fixture.jaws("2026/Settings/enu/reaper.jss"), "script");
        let dll = fixture.write("REAPER/UserPlugins/Reaper_ComAccess32.dll", "com32");
        let mut expected = vec![entry(&dll, "com32", false), entry(&script, "script", false)];
        expected.sort_by(|left, right| left.path.cmp(&right.path));
        assert_eq!(snapshot.record(), expected);

        // Nothing appears at all.
        let empty = Fixture::new();
        let snapshot = InstallSnapshot::take(&jaws_roots(), empty.paths(), &[]);
        assert!(snapshot.record().is_empty());

        // `{appdata}` stands for nothing here, as off Windows.
        let unresolved = Fixture::new();
        let paths = UninstallRoots::new(&unresolved.appdata.join("REAPER"));
        let snapshot = InstallSnapshot::take(&jaws_roots(), paths, &[]);
        unresolved.write(&unresolved.jaws("2026/Settings/enu/reaper.jss"), "script");
        assert!(snapshot.record().is_empty());
    }

    /// A folder RABBIT couldn't list before the run might have held anything,
    /// the user's files included, so nothing below it is recorded. A file
    /// where the `Settings` folder should be stands in for a folder RABBIT
    /// isn't allowed to read: listing it fails with something other than
    /// "not found".
    #[test]
    fn nothing_below_a_folder_that_could_not_be_listed_is_recorded() {
        let fixture = Fixture::new();
        let blocked = fixture.write(&fixture.jaws("2026/Settings"), "not a folder");
        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &[]);

        fs::remove_file(&blocked).unwrap();
        fixture.write(&fixture.jaws("2026/Settings/deu/reaper.jss"), "script");
        fixture.write(&fixture.jaws("2026/Settings/deu/default.jkm"), "mine");
        let dll = fixture.write("REAPER/UserPlugins/Reaper_ComAccess64.dll", "com");

        assert_eq!(snapshot.record(), [entry(&dll, "com", false)]);
    }

    /// On a reinstall the earlier record stays, and the installer's own files
    /// it rewrote get their new hash. One the user had changed since is
    /// marked as there before, since it held their changes, not the
    /// installer's file.
    #[test]
    fn a_reinstall_keeps_the_earlier_record_and_brings_it_up_to_date() {
        let fixture = Fixture::new();
        let deu = |name: &str| fixture.jaws(&format!("2026/Settings/deu/{name}"));
        let script = fixture.write(&deu("reaper.jss"), "v1");
        let keys = fixture.write(&deu("reaper.jkm"), "keys v1");
        let config = fixture.write(&deu("reaper.jcf"), "config v1");
        let deleted = fixture.path(&deu("reaper.jsd"));
        let replaced_earlier = fixture.write(&deu("default.jcf"), "theirs v1");
        let earlier = vec![
            entry(&script, "v1", false),
            entry(&keys, "keys v1", false),
            entry(&config, "config v1", false),
            entry(&deleted, "docs v1", false),
            entry(&replaced_earlier, "theirs v1", true),
        ];
        // Since then the user changed their keys and deleted one file.
        fs::write(&keys, "my own keys").unwrap();

        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &earlier);
        fs::write(&script, "v2, longer").unwrap();
        fs::write(&keys, "keys v2, longer").unwrap();
        fs::write(&replaced_earlier, "theirs v2, longer").unwrap();
        fixture.write(&deu("reaper.jsd"), "docs v2");
        let added = fixture.write(&deu("reaper_new.jsb"), "new");

        let record = snapshot.record();
        assert_eq!(
            find(&record, &script),
            Some(&entry(&script, "v2, longer", false))
        );
        assert_eq!(
            find(&record, &keys),
            Some(&entry(&keys, "keys v2, longer", true)),
            "the user's changes were replaced, so the file is marked"
        );
        assert_eq!(
            find(&record, &config),
            Some(&entry(&config, "config v1", false)),
            "a recorded file the installer left alone stays as it was"
        );
        assert_eq!(
            find(&record, &deleted),
            Some(&entry(&deleted, "docs v2", false))
        );
        assert_eq!(
            find(&record, &replaced_earlier),
            Some(&entry(&replaced_earlier, "theirs v2, longer", true)),
            "a file marked before keeps its mark"
        );
        assert_eq!(find(&record, &added), Some(&entry(&added, "new", false)));
        assert_eq!(record.len(), 6);
    }

    /// A link inside a snapshot folder is never followed: what it leads to
    /// isn't the installer's to record.
    #[test]
    fn links_are_not_followed() {
        let fixture = Fixture::new();
        let elsewhere = fixture.appdata.join("Elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let deu = fixture.path(&fixture.jaws("2026/Settings/deu"));
        fs::create_dir_all(&deu).unwrap();
        make_link(&deu.join("linked"), &elsewhere);

        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &[]);
        fs::write(elsewhere.join("reaper.jss"), "through the link").unwrap();
        let script = fixture.write(&fixture.jaws("2026/Settings/deu/reaper.jss"), "script");

        assert_eq!(snapshot.record(), [entry(&script, "script", false)]);
    }

    /// Whatever a root's `*` reaches, RABBIT's own folder is skipped.
    #[test]
    fn rabbits_own_folder_is_never_recorded() {
        let fixture = Fixture::new();
        let roots = [UninstallPath::new("{appdata}/REAPER/*")];
        let snapshot = InstallSnapshot::take(&roots, fixture.paths(), &[]);
        fixture.write("REAPER/RABBIT/logs/install.json", "{}");
        let plugin = fixture.write("REAPER/UserPlugins/reaper_demo.dll", "demo");
        assert_eq!(snapshot.record(), [entry(&plugin, "demo", false)]);
    }

    /// Stands in for the owner check after an elevated installer: the fake
    /// installer's files are named `reaper*`, and Administrators own them,
    /// while every other file belongs to the user. No real owner is read.
    fn administrators_own_reaper_files(path: &Path) -> io::Result<bool> {
        let name = path.file_name().unwrap().to_string_lossy();
        Ok(name.to_ascii_lowercase().starts_with("reaper"))
    }

    /// Stands in for owners nobody trusts.
    fn nobody_trusted(_: &Path) -> io::Result<bool> {
        Ok(false)
    }

    /// Stands in for owners RABBIT can't read.
    fn owner_unreadable(_: &Path) -> io::Result<bool> {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }

    /// What JAWS itself creates in its settings folders while the scripts
    /// install: new files, but the user's.
    fn jaws_writes_its_own_files(fixture: &Fixture) -> Vec<PathBuf> {
        vec![
            fixture.write(&fixture.jaws("2026/Settings/deu/DEFAULT.JCF"), "jaws"),
            fixture.write(
                &fixture.jaws("2026/Settings/deu/SettingsCenter.ini"),
                "jaws",
            ),
            fixture.write(
                &fixture.jaws("2026/Settings/Notifications/rules.json"),
                "jaws",
            ),
        ]
    }

    #[test]
    fn after_an_elevated_install_a_new_file_administrators_dont_own_is_not_recorded() {
        let fixture = Fixture::new();
        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &[])
            .new_files_only_from_administrators(administrators_own_reaper_files);
        let script = fixture.write(&fixture.jaws("2026/Settings/deu/reaper.jss"), "script");
        jaws_writes_its_own_files(&fixture);
        assert_eq!(snapshot.record(), [entry(&script, "script", false)]);

        // An owner RABBIT can't read counts as one it doesn't trust, even
        // for the installer's own files.
        let fixture = Fixture::new();
        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &[])
            .new_files_only_from_administrators(owner_unreadable);
        fixture.write(&fixture.jaws("2026/Settings/deu/reaper.jss"), "script");
        fixture.write("REAPER/UserPlugins/Reaper_ComAccess64.dll", "com");
        jaws_writes_its_own_files(&fixture);
        assert!(snapshot.record().is_empty());
    }

    #[test]
    fn after_an_elevated_install_a_new_file_administrators_own_is_recorded() {
        let fixture = Fixture::new();
        fixture.write(&fixture.jaws("2026/Settings/deu/default.jkm"), "mine");
        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &[])
            .new_files_only_from_administrators(administrators_own_reaper_files);

        // New files in a folder that was there and in one the installer
        // creates, and the REAPER side DLL.
        let script = fixture.write(&fixture.jaws("2026/Settings/deu/reaper.jss"), "script");
        let sound = fixture.write(
            &fixture.jaws("2026/Settings/deu/Sounds/reaper_record.wav"),
            "wave",
        );
        let older = fixture.write(&fixture.jaws("2025/Settings/enu/reaper.jss"), "old script");
        let dll = fixture.write("REAPER/UserPlugins/Reaper_ComAccess64.dll", "com");
        jaws_writes_its_own_files(&fixture);

        let mut expected = vec![
            entry(&script, "script", false),
            entry(&sound, "wave", false),
            entry(&older, "old script", false),
            entry(&dll, "com", false),
        ];
        expected.sort_by(|left, right| left.path.cmp(&right.path));
        assert_eq!(snapshot.record(), expected);
    }

    #[test]
    fn without_elevation_every_new_file_is_recorded_as_before() {
        let fixture = Fixture::new();
        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &[]);
        let script = fixture.write(&fixture.jaws("2026/Settings/deu/reaper.jss"), "script");
        let dll = fixture.write("REAPER/UserPlugins/Reaper_ComAccess64.dll", "com");
        let jaws_files = jaws_writes_its_own_files(&fixture);

        let mut expected = vec![entry(&script, "script", false), entry(&dll, "com", false)];
        expected.extend(jaws_files.iter().map(|path| entry(path, "jaws", false)));
        expected.sort_by(|left, right| left.path.cmp(&right.path));
        assert_eq!(snapshot.record(), expected);
    }

    /// Writing over a file keeps its owner, so the owner says nothing about
    /// who changed it. A changed file is recorded as without the rule: marked
    /// as there before, unless an earlier install recorded it unmarked and it
    /// still held exactly that.
    #[test]
    fn a_changed_file_keeps_its_existed_before_rule_whoever_owns_it() {
        let probes: [fn(&Path) -> io::Result<bool>; 2] = [nobody_trusted, owner_unreadable];
        for owner_is_trusted in probes {
            let fixture = Fixture::new();
            let deu = |name: &str| fixture.jaws(&format!("2026/Settings/deu/{name}"));
            let script = fixture.write(&deu("reaper.jss"), "v1");
            let keys = fixture.write(&deu("reaper.jkm"), "keys v1");
            let users = fixture.write(&deu("default.jkm"), "mine");
            let earlier = vec![entry(&script, "v1", false), entry(&keys, "keys v1", false)];
            // Since then the user changed the scripts' keys.
            fs::write(&keys, "my own keys").unwrap();

            let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &earlier)
                .new_files_only_from_administrators(owner_is_trusted);
            fs::write(&script, "v2, longer").unwrap();
            fs::write(&keys, "keys v2, longer").unwrap();
            fs::write(&users, "replaced by the installer").unwrap();

            let mut expected = vec![
                entry(&script, "v2, longer", false),
                entry(&keys, "keys v2, longer", true),
                entry(&users, "replaced by the installer", true),
            ];
            expected.sort_by(|left, right| left.path.cmp(&right.path));
            assert_eq!(snapshot.record(), expected);
        }
    }

    /// On a reinstall after an elevated installer, a recorded file the user
    /// had deleted and someone else creates again is left out, so its earlier
    /// entry stays as it was, as for any recorded file this run didn't write:
    /// a later uninstall checks its hash. One the installer writes again is
    /// brought up to date as before.
    #[test]
    fn an_elevated_reinstall_keeps_the_earlier_entry_of_a_file_someone_else_brought_back() {
        let fixture = Fixture::new();
        let deu = |name: &str| fixture.jaws(&format!("2026/Settings/deu/{name}"));
        let docs = fixture.path(&deu("reaper.jsd"));
        let uia = fixture.path(&deu("UIA.jsh"));
        let config = fixture.write(&deu("reaper.jcf"), "config v1");
        let earlier = vec![
            entry(&docs, "docs v1", false),
            entry(&uia, "uia v1", false),
            entry(&config, "config v1", false),
        ];

        let snapshot = InstallSnapshot::take(&jaws_roots(), fixture.paths(), &earlier)
            .new_files_only_from_administrators(administrators_own_reaper_files);
        fixture.write(&deu("reaper.jsd"), "docs v2");
        fixture.write(&deu("UIA.jsh"), "made by someone else");

        let mut expected = vec![
            entry(&docs, "docs v2", false),
            entry(&uia, "uia v1", false),
            entry(&config, "config v1", false),
        ];
        expected.sort_by(|left, right| left.path.cmp(&right.path));
        assert_eq!(snapshot.record(), expected);
    }

    #[cfg(windows)]
    fn make_link(link: &Path, target: &Path) {
        crate::uninstall::paths::make_junction(link, target);
    }

    #[cfg(unix)]
    fn make_link(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }
}
