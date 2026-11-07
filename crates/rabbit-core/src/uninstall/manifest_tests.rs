//! The shipped manifests' uninstall plans, on temporary resource folders:
//! what each package RABBIT can uninstall would remove and keep, the
//! warnings it shows, and the copies it refuses. Planning never removes
//! anything, and every folder here is a fake one; the CLAP folder too.
//! REAPER on macOS is planned on a stand-in Mac, where every fact macOS
//! would give is handed in, and its dry runs and its one real run move
//! nothing but a temporary app into a temporary Trash.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rabbit_platform::{ShellCommand, ShellStep};

use super::admin_batch::{
    AdministratorStepKind, AdministratorStepRecord, AdministratorStepVerdict, MV, PGREP, TEST,
    administrator_steps, prepare_package, trash_name,
};
use super::execute::{UninstallEnvironment, execute_uninstall_in};
use super::macos_plan_tests::{
    DISK, MacFixture, OneVolume, join, my_trash, owned_by_me, owned_by_root, snapshot,
};
use super::tests::{Fixture, FoundAt, detection, installed, write};
use super::{
    ByHandReason, FakeMacosObjects, KeptReason, MacosObjects, NoAdministratorShell,
    NoElevatedHelper, NoMacosObjects, NothingListed, ObjectFacts, ObjectProbe,
    PlannedRegistryCleanup, Removability, RemovalReason, RemovalRoute, TrashFolderFacts,
    UNINSTALL_NOT_YET_KEY, UninstallMessage, UninstallNote, UninstallPlan, UninstallPlanInput,
    UninstallPlanItem, UninstallReport, UninstallRequest, UninstallRoots, UninstallStatus,
    UninstallWarning, VendorLookup, VendorRefusal, build_uninstall_plan,
};
use crate::cancel::CancelToken;
use crate::detection::detect_component_with_probes;
use crate::localization::{Localizer, embedded_locales};
use crate::model::{ComponentDetection, InstallationKind, Platform};
use crate::package::uninstall::{
    UninstallBackup, UninstallElevation, UninstallPostStep, UninstallStrategy,
    VendorUninstallerKind,
};
use crate::package::{
    PACKAGE_APP2CLAP, PACKAGE_FFMPEG, PACKAGE_JAWS_SCRIPTS, PACKAGE_OSARA, PACKAGE_REAKONTROL,
    PACKAGE_REAPACK, PACKAGE_REAPER, PACKAGE_SURGE_XT, PACKAGE_SWS, PackageSpec,
    builtin_package_specs, embedded_package_manifest, validate_package_spec,
};
use crate::preflight::{MACOS_REAPER_PROCESS_NAMES, RunningProcess};
use crate::progress::ProgressReporter;
use crate::reapack::write_test_registry;
use crate::receipt::{InstallState, InstalledFileReceipt, PackageReceipt};

const LANGPACK_DE: &str = "langpack-de";
const LANGPACK_ES: &str = "langpack-es";
const LANGPACK_FR: &str = "langpack-fr";
const SWS_IN_REAPACK: &str = "SWS/S&M extension";

/// The shipped packages for one platform, planned on a fixture.
struct Shipped<'f> {
    roots: &'f UninstallRoots,
    resource: &'f Path,
    platform: Platform,
    specs: Vec<PackageSpec>,
    /// Who may move a macOS app, as the test says macOS would answer.
    objects: &'f dyn ObjectProbe,
}

impl<'f> Shipped<'f> {
    fn new(fixture: &'f Fixture, platform: Platform) -> Self {
        Self {
            roots: &fixture.roots,
            resource: &fixture.resource,
            platform,
            specs: builtin_package_specs(platform),
            objects: &NoMacosObjects,
        }
    }

    /// The shipped macOS packages on a stand-in Mac, where `objects` says
    /// who may move REAPER's app and what the user's Trash is.
    fn on_mac(mac: &'f MacFixture, objects: &'f dyn ObjectProbe) -> Self {
        Self {
            roots: &mac.roots,
            resource: &mac.resource,
            platform: Platform::MacOs,
            specs: builtin_package_specs(Platform::MacOs),
            objects,
        }
    }

    fn plan(
        &self,
        detections: &[ComponentDetection],
        state: Option<&InstallState>,
    ) -> UninstallPlan {
        self.plan_with(detections, state, InstallationKind::Standard, false)
    }

    fn plan_with(
        &self,
        detections: &[ComponentDetection],
        state: Option<&InstallState>,
        target: InstallationKind,
        remove_settings: bool,
    ) -> UninstallPlan {
        self.plan_looking_up(detections, state, target, remove_settings, &NothingListed)
    }

    fn plan_looking_up(
        &self,
        detections: &[ComponentDetection],
        state: Option<&InstallState>,
        target: InstallationKind,
        remove_settings: bool,
        vendor_lookup: &dyn VendorLookup,
    ) -> UninstallPlan {
        build_uninstall_plan(&UninstallPlanInput {
            roots: self.roots,
            platform: self.platform,
            target,
            specs: &self.specs,
            detections,
            state,
            remove_settings,
            vendor_lookup,
            objects: self.objects,
        })
    }

    /// Real detection on the fixture for `ids`, plus ReaPack as not
    /// installed unless it is among them. Only for packages whose detectors
    /// look nowhere but the resource folder: app2clap's look in the real
    /// CLAP folder, so its tests stand detection in.
    fn detect(&self, state: Option<&InstallState>, ids: &[&str]) -> Vec<ComponentDetection> {
        let mut detections: Vec<ComponentDetection> = self
            .specs
            .iter()
            .filter(|spec| ids.contains(&spec.id.as_str()))
            .map(|spec| {
                detect_component_with_probes(self.resource, self.platform, spec, state, |_| None)
                    .unwrap()
            })
            .collect();
        if !ids.contains(&PACKAGE_REAPACK) {
            detections.push(detection(PACKAGE_REAPACK, false));
        }
        detections
    }
}

/// A receipt listing, for each package, its files: paths below the
/// resource folder with `/` between components, or absolute paths. A file
/// gets a made-up hash, which planning never checks, and its real size; a
/// folder gets neither, as install records it.
fn receipt(fixture: &Fixture, packages: &[(&str, &[&str])]) -> InstallState {
    let mut state = InstallState::default();
    for (id, files) in packages {
        let installed_files = files
            .iter()
            .map(|file| {
                let recorded: PathBuf = if Path::new(file).is_absolute() {
                    PathBuf::from(file)
                } else {
                    file.split('/').collect()
                };
                let metadata = fs::metadata(fixture.resource.join(&recorded)).unwrap();
                InstalledFileReceipt {
                    path: recorded,
                    sha256: metadata.is_file().then(|| format!("sha256 of {file}")),
                    size: metadata.is_file().then_some(metadata.len()),
                }
            })
            .collect();
        state.packages.insert(
            id.to_string(),
            PackageReceipt {
                id: id.to_string(),
                version: None,
                source_url: None,
                source_sha256: None,
                variant: None,
                channel: None,
                installed_files,
                installed_at: None,
                rabbit_version: None,
                architecture: None,
                vendor_added_files: Vec::new(),
            },
        );
    }
    state
}

fn item<'a>(plan: &'a UninstallPlan, id: &str) -> &'a UninstallPlanItem {
    plan.item(id).unwrap_or_else(|| panic!("no item for {id}"))
}

/// The paths an item removes, sorted.
fn removed(item: &UninstallPlanItem) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = item.files.iter().map(|file| file.path.clone()).collect();
    paths.sort();
    paths
}

fn reason_of(item: &UninstallPlanItem, path: &Path) -> RemovalReason {
    item.files
        .iter()
        .find(|file| file.path == path)
        .unwrap_or_else(|| panic!("{} isn't removed", path.display()))
        .reason
}

/// The package's own data an item keeps, as (path, note key, exists),
/// sorted.
fn kept_data(item: &UninstallPlanItem) -> Vec<(PathBuf, String, bool)> {
    let mut kept: Vec<(PathBuf, String, bool)> = item
        .kept
        .iter()
        .filter(|kept| kept.reason == KeptReason::UserData)
        .map(|kept| {
            (
                kept.path.clone(),
                kept.note_key.clone().unwrap_or_default(),
                kept.exists,
            )
        })
        .collect();
    kept.sort();
    kept
}

fn sorted(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    paths
}

/// Each package's strategy on one platform and target; `None` where no plan
/// applies.
type Expected<'a> = &'a [(&'a str, Option<UninstallStrategy>)];

/// Which shipped packages RABBIT can uninstall, and how, on each platform
/// and target. The packages whose turn hasn't come say so, and the two RABBIT
/// leaves to the user say how to do it by hand.
#[test]
fn the_shipped_plans_say_which_packages_rabbit_can_uninstall() {
    for spec in embedded_package_manifest().packages {
        validate_package_spec(&spec).unwrap_or_else(|message| panic!("{}: {message}", spec.id));
    }

    let remove_files = Some(UninstallStrategy::RemoveFiles);
    let vendor = Some(UninstallStrategy::VendorUninstaller);
    let manual = Some(UninstallStrategy::Manual);
    let not_supported = Some(UninstallStrategy::NotSupported);
    let cases: &[(Platform, InstallationKind, Expected<'_>)] = &[
        (
            Platform::Windows,
            InstallationKind::Standard,
            &[
                (PACKAGE_REAPER, vendor),
                (PACKAGE_OSARA, remove_files),
                (PACKAGE_SWS, remove_files),
                (PACKAGE_REAPACK, remove_files),
                (PACKAGE_REAKONTROL, remove_files),
                (PACKAGE_JAWS_SCRIPTS, manual),
                (PACKAGE_FFMPEG, remove_files),
                (PACKAGE_SURGE_XT, vendor),
                (PACKAGE_APP2CLAP, remove_files),
                (LANGPACK_ES, remove_files),
                (LANGPACK_DE, remove_files),
                (LANGPACK_FR, remove_files),
            ],
        ),
        (
            Platform::Windows,
            InstallationKind::Portable,
            // The JAWS scripts, Surge XT and app2clap serve the whole
            // computer, so a portable target can't remove them. Surge XT
            // and app2clap have a removal it can't reach; the JAWS scripts
            // keep the standard REAPER's plan, which removes nothing.
            &[
                (PACKAGE_REAPER, manual),
                (PACKAGE_OSARA, remove_files),
                (PACKAGE_SWS, remove_files),
                (PACKAGE_REAPACK, remove_files),
                (PACKAGE_REAKONTROL, remove_files),
                (PACKAGE_JAWS_SCRIPTS, manual),
                (PACKAGE_FFMPEG, remove_files),
                (PACKAGE_SURGE_XT, None),
                (PACKAGE_APP2CLAP, None),
                (LANGPACK_ES, remove_files),
                (LANGPACK_DE, remove_files),
                (LANGPACK_FR, remove_files),
            ],
        ),
    ];
    // On macOS one plan serves a standard and a portable REAPER: its app
    // goes to the Trash wherever it is.
    let macos = [
        (PACKAGE_REAPER, remove_files),
        (PACKAGE_OSARA, remove_files),
        (PACKAGE_SWS, remove_files),
        (PACKAGE_REAPACK, remove_files),
        (PACKAGE_REAKONTROL, remove_files),
        (PACKAGE_SURGE_XT, not_supported),
        (LANGPACK_ES, remove_files),
        (LANGPACK_DE, remove_files),
        (LANGPACK_FR, remove_files),
    ];
    let cases = cases.iter().copied().chain([
        (Platform::MacOs, InstallationKind::Standard, &macos[..]),
        (Platform::MacOs, InstallationKind::Portable, &macos[..]),
    ]);

    let fixture = Fixture::new();
    for (platform, target, expected) in cases {
        let shipped = Shipped::new(&fixture, platform);
        let plan = shipped.plan_with(&[], None, target, false);
        let strategies: BTreeMap<&str, Option<UninstallStrategy>> = plan
            .items
            .iter()
            .map(|item| (item.package_id.as_str(), item.strategy))
            .collect();
        let expected: BTreeMap<&str, Option<UninstallStrategy>> =
            expected.iter().copied().collect();
        assert_eq!(strategies, expected, "{platform:?}, {target:?}");
    }

    // The two packages RABBIT leaves to the user say how, in their own
    // words rather than "not yet".
    let windows = Shipped::new(&fixture, Platform::Windows);
    let everything: Vec<ComponentDetection> = windows
        .specs
        .iter()
        .map(|spec| detection(&spec.id, true))
        .collect();
    let plan = windows.plan(&everything, None);
    assert_eq!(
        item(&plan, PACKAGE_JAWS_SCRIPTS).removability,
        Removability::Manual {
            note_key: "uninstall-manual-jaws-scripts".to_string()
        }
    );
    let plan = windows.plan_with(&everything, None, InstallationKind::Portable, false);
    assert_eq!(
        item(&plan, PACKAGE_REAPER).removability,
        Removability::Manual {
            note_key: "uninstall-manual-reaper-portable".to_string()
        }
    );
    // The JAWS DLLs are in the standard REAPER's folder whatever the target,
    // so a portable target shows the same steps, rather than sending the
    // user to a standard REAPER that can't remove them either. Surge XT
    // comes off only from the standard REAPER, as app2clap does.
    assert_eq!(
        item(&plan, PACKAGE_JAWS_SCRIPTS).removability,
        Removability::Manual {
            note_key: "uninstall-manual-jaws-scripts".to_string()
        }
    );
    assert_eq!(
        item(&plan, PACKAGE_SURGE_XT).removability,
        Removability::NeedsStandardTarget
    );
    // On macOS, Surge XT waits for a later version.
    let macos = Shipped::new(&fixture, Platform::MacOs);
    let everything: Vec<ComponentDetection> = macos
        .specs
        .iter()
        .map(|spec| detection(&spec.id, true))
        .collect();
    let macos_plan = macos.plan(&everything, None);
    assert_eq!(
        item(&macos_plan, PACKAGE_SURGE_XT).removability,
        Removability::NotSupported {
            note_key: UNINSTALL_NOT_YET_KEY.to_string()
        }
    );
    assert_eq!(
        item(&plan, PACKAGE_APP2CLAP).removability,
        Removability::NeedsStandardTarget
    );
}

/// SWS on Windows: its plug-in, found by name, and both Python bridge
/// scripts, the second of which older receipts miss. Grooves and settings
/// stay unless the user asks for them, and REAPER's own files always stay.
#[test]
fn sws_on_windows_removes_its_plugin_and_scripts_and_keeps_the_users_data() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_sws-x64.dll");
    let bridge = fixture.file("Scripts/sws_python.py");
    let bridge64 = fixture.file("Scripts/sws_python64.py");
    fixture.file("Data/Grooves/Swing.rgt");
    let grooves = fixture.resource.join("Data").join("Grooves");
    let settings = fixture.file("S&M.ini");
    let cycle_actions = fixture.file("S&M_Cyclactions.ini");
    let breeder = fixture.file("BR.ini");
    fixture.file("reaper.ini");
    fixture.file("reaper-kb.ini");
    let reapack = fixture.file("UserPlugins/reaper_reapack-x64.dll");
    // Install records the plug-in and the first script with hashes, and
    // the folders without.
    let state = receipt(
        &fixture,
        &[(
            PACKAGE_SWS,
            &[
                "UserPlugins",
                "UserPlugins/reaper_sws-x64.dll",
                "Scripts/sws_python.py",
                "Data/Grooves",
            ],
        )],
    );
    let shipped = Shipped::new(&fixture, Platform::Windows);

    let plan = shipped.plan(&installed(&[PACKAGE_SWS]), Some(&state));
    let sws = item(&plan, PACKAGE_SWS);

    assert_eq!(sws.removability, Removability::Removable);
    assert_eq!(
        removed(sws),
        sorted(vec![plugin.clone(), bridge.clone(), bridge64.clone()])
    );
    assert!(!removed(sws).contains(&reapack));
    assert_eq!(reason_of(sws, &plugin), RemovalReason::MatchingUserPlugin);
    assert_eq!(reason_of(sws, &bridge64), RemovalReason::ManifestPath);
    let resource = &fixture.resource;
    let settings_note = "uninstall-kept-sws-settings".to_string();
    assert_eq!(
        kept_data(sws),
        vec![
            (breeder.clone(), settings_note.clone(), true),
            (
                grooves.clone(),
                "uninstall-kept-sws-grooves".to_string(),
                true
            ),
            (settings.clone(), settings_note.clone(), true),
            (
                cycle_actions.clone(),
                "uninstall-kept-sws-cycle-actions".to_string(),
                true
            ),
            (
                resource.join("SWS-RenamePresets.txt"),
                settings_note.clone(),
                false
            ),
            (resource.join("sws-autocoloricon.ini"), settings_note, false),
        ]
    );
    assert!(sws.warnings.is_empty(), "{:?}", sws.warnings);
    assert!(sws.notes.is_empty(), "{:?}", sws.notes);
    assert!(!sws.confirm && !sws.unbacked && !sws.every_reaper_closed);
    assert!(sws.post.is_empty());

    // "Also remove settings" takes the grooves and settings files that
    // exist, and never REAPER's own configuration.
    let plan = shipped.plan_with(
        &installed(&[PACKAGE_SWS]),
        Some(&state),
        InstallationKind::Standard,
        true,
    );
    let sws = item(&plan, PACKAGE_SWS);
    assert_eq!(
        removed(sws),
        sorted(vec![
            plugin.clone(),
            bridge.clone(),
            bridge64.clone(),
            grooves.clone(),
            settings,
            cycle_actions,
            breeder,
        ])
    );
    assert_eq!(reason_of(sws, &grooves), RemovalReason::Setting);
    assert_eq!(
        kept_data(sws)
            .into_iter()
            .map(|(path, _, _)| path)
            .collect::<Vec<_>>(),
        vec![
            resource.join("SWS-RenamePresets.txt"),
            resource.join("sws-autocoloricon.ini"),
        ]
    );

    // A copy RABBIT has no receipt for comes off the same way, with a note.
    let plan = shipped.plan(&installed(&[PACKAGE_SWS]), None);
    let sws = item(&plan, PACKAGE_SWS);
    assert_eq!(sws.removability, Removability::Removable);
    assert_eq!(removed(sws), sorted(vec![plugin, bridge, bridge64]));
    assert_eq!(sws.notes, vec![UninstallNote::NoReceipt]);
}

/// SWS on macOS: RABBIT copies only the plug-in from the disk image, so
/// that is all it removes. Windows files a folder may hold aren't SWS's
/// here.
#[test]
fn sws_on_macos_removes_only_its_plugin() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_sws-arm64.dylib");
    fixture.file("UserPlugins/reaper_sws-x64.dll");
    fixture.file("Scripts/sws_python.py");
    let settings = fixture.file("S&M.ini");
    let state = receipt(
        &fixture,
        &[(PACKAGE_SWS, &["UserPlugins/reaper_sws-arm64.dylib"])],
    );
    let shipped = Shipped::new(&fixture, Platform::MacOs);

    let plan = shipped.plan(&installed(&[PACKAGE_SWS]), Some(&state));
    let sws = item(&plan, PACKAGE_SWS);

    assert_eq!(sws.removability, Removability::Removable);
    assert_eq!(removed(sws), vec![plugin]);
    assert!(kept_data(sws).contains(&(settings, "uninstall-kept-sws-settings".to_string(), true)));
}

/// SWS warns that OSARA's key map loses keys, but only while OSARA is
/// installed, and that scripts may stop working while ReaPack is.
#[test]
fn sws_warns_about_osaras_keys_while_osara_is_installed() {
    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_sws-x64.dll");
    let shipped = Shipped::new(&fixture, Platform::Windows);
    let osara_warning = UninstallWarning::OtherPackageInstalled {
        package_id: PACKAGE_OSARA.to_string(),
        message_key: "uninstall-warning-sws-osara-keymap".to_string(),
    };
    let reapack_warning = UninstallWarning::OtherPackageInstalled {
        package_id: PACKAGE_REAPACK.to_string(),
        message_key: "uninstall-warning-sws-reapack-scripts".to_string(),
    };

    let plan = shipped.plan(&installed(&[PACKAGE_SWS]), None);
    assert!(item(&plan, PACKAGE_SWS).warnings.is_empty());

    let plan = shipped.plan(&installed(&[PACKAGE_SWS, PACKAGE_OSARA]), None);
    let sws = item(&plan, PACKAGE_SWS);
    assert_eq!(sws.warnings, vec![osara_warning.clone()]);
    assert!(sws.removability.is_removable());
    assert!(
        !sws.confirm,
        "a warning about other packages asks no question"
    );

    let plan = shipped.plan(
        &installed(&[PACKAGE_SWS, PACKAGE_OSARA, PACKAGE_REAPACK]),
        None,
    );
    assert_eq!(
        item(&plan, PACKAGE_SWS).warnings,
        vec![osara_warning, reapack_warning]
    );
}

/// An SWS that ReaPack installed and still manages is ReaPack's to remove.
/// Once ReaPack itself is gone, nothing keeps its records, and the copy may
/// go.
#[test]
fn sws_managed_by_reapack_is_blocked_until_reapack_is_gone() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_sws-x64.dll");
    let settings = fixture.file("S&M.ini");
    write_test_registry(
        &fixture.resource,
        &[(
            SWS_IN_REAPACK,
            "2.14.0",
            &["UserPlugins/reaper_sws-x64.dll"],
        )],
    );
    let shipped = Shipped::new(&fixture, Platform::Windows);

    // Detection itself sees the copy through ReaPack's records.
    let detections = shipped.detect(None, &[PACKAGE_SWS]);
    assert!(
        detections
            .iter()
            .any(|found| found.package_id == PACKAGE_SWS
                && found.installed
                && found.detector == "reapack-registry")
    );

    let mut with_reapack = detections.clone();
    with_reapack.retain(|found| found.package_id != PACKAGE_REAPACK);
    with_reapack.push(detection(PACKAGE_REAPACK, true));
    let plan = shipped.plan(&with_reapack, None);
    let sws = item(&plan, PACKAGE_SWS);
    assert_eq!(
        sws.removability,
        Removability::ManagedByReaPack {
            path: plugin.clone(),
            reapack_package: SWS_IN_REAPACK.to_string()
        }
    );
    assert!(sws.removability.is_blocked());
    assert!(sws.files.is_empty());
    // Its settings are still listed as staying.
    assert!(
        kept_data(sws)
            .iter()
            .any(|(path, _, exists)| *path == settings && *exists)
    );

    let plan = shipped.plan(&detections, None);
    let sws = item(&plan, PACKAGE_SWS);
    assert_eq!(sws.removability, Removability::RemovableOrphanedFromReaPack);
    assert_eq!(removed(sws), vec![plugin]);
    assert!(sws.notes.contains(&UninstallNote::OrphanedFromReaPack {
        reapack_packages: vec![SWS_IN_REAPACK.to_string()]
    }));
}

/// ReaPack lists itself in its own records, which never block it. Its
/// settings, its records and everything it installed stay; before it goes,
/// the plan names the extensions it still manages.
#[test]
fn reapack_removes_its_plugin_and_keeps_what_it_installed() {
    let fixture = Fixture::new();
    let own = fixture.file("UserPlugins/reaper_reapack-x64.dll");
    let sws = fixture.file("UserPlugins/reaper_sws-x64.dll");
    let script = fixture.file("Scripts/ReaTeam Scripts/a_script.lua");
    let settings = fixture.file("reapack.ini");
    write_test_registry(
        &fixture.resource,
        &[
            ("ReaPack", "1.2.5", &["UserPlugins/reaper_reapack-x64.dll"]),
            (
                SWS_IN_REAPACK,
                "2.14.0",
                &["UserPlugins/reaper_sws-x64.dll"],
            ),
            ("A script", "1.0", &["Scripts/ReaTeam Scripts/a_script.lua"]),
        ],
    );
    let records = fixture.resource.join("ReaPack");
    let shipped = Shipped::new(&fixture, Platform::Windows);
    let detections = installed(&[PACKAGE_REAPACK, PACKAGE_SWS]);

    let plan = shipped.plan(&detections, None);
    let reapack = item(&plan, PACKAGE_REAPACK);

    assert_eq!(reapack.removability, Removability::Removable);
    assert_eq!(removed(reapack), vec![own.clone()]);
    assert_eq!(
        kept_data(reapack),
        vec![
            (
                records.clone(),
                "uninstall-kept-reapack-data".to_string(),
                true
            ),
            (
                settings.clone(),
                "uninstall-kept-reapack-settings".to_string(),
                true
            ),
        ]
    );
    assert_eq!(
        reapack.notes,
        vec![
            UninstallNote::NoReceipt,
            UninstallNote::Manifest {
                note_key: "uninstall-note-reapack-packages-stay".to_string()
            },
        ]
    );
    assert!(
        reapack
            .warnings
            .contains(&UninstallWarning::ReaPackStillManages {
                reapack_packages: vec![SWS_IN_REAPACK.to_string()]
            })
    );

    // "Also remove settings" takes ReaPack's settings and records, never
    // what it installed.
    let plan = shipped.plan_with(&detections, None, InstallationKind::Standard, true);
    let reapack = item(&plan, PACKAGE_REAPACK);
    assert_eq!(removed(reapack), sorted(vec![own, records, settings]));
    assert!(!removed(reapack).contains(&sws));
    assert!(!removed(reapack).contains(&script));

    // macOS: the plug-in for the Mac's architecture.
    let fixture = Fixture::new();
    let own = fixture.file("UserPlugins/reaper_reapack-arm64.dylib");
    let plan = Shipped::new(&fixture, Platform::MacOs).plan(&installed(&[PACKAGE_REAPACK]), None);
    assert_eq!(removed(item(&plan, PACKAGE_REAPACK)), vec![own]);
}

/// ReaKontrol: one plug-in. The FX maps are the user's, and its folder goes
/// only once "Also remove settings" has emptied it.
#[test]
fn reakontrol_removes_its_plugin_and_keeps_the_fx_maps() {
    for (platform, file) in [
        (Platform::Windows, "UserPlugins/reaper_kontrol.dll"),
        (Platform::MacOs, "UserPlugins/reaper_kontrol.dylib"),
    ] {
        let fixture = Fixture::new();
        let plugin = fixture.file(file);
        fixture.file("reaKontrol/fxMaps/Serum.rkfm");
        let folder = fixture.resource.join("reaKontrol");
        let maps = folder.join("fxMaps");
        let state = receipt(&fixture, &[(PACKAGE_REAKONTROL, &[file])]);
        let shipped = Shipped::new(&fixture, platform);

        let plan = shipped.plan(&installed(&[PACKAGE_REAKONTROL]), Some(&state));
        let reakontrol = item(&plan, PACKAGE_REAKONTROL);
        assert_eq!(reakontrol.removability, Removability::Removable);
        assert_eq!(removed(reakontrol), vec![plugin.clone()]);
        assert_eq!(
            kept_data(reakontrol),
            vec![(
                maps.clone(),
                "uninstall-kept-reakontrol-fx-maps".to_string(),
                true
            )]
        );
        assert_eq!(reakontrol.dirs_if_empty, vec![folder.clone()]);

        let plan = shipped.plan_with(
            &installed(&[PACKAGE_REAKONTROL]),
            Some(&state),
            InstallationKind::Standard,
            true,
        );
        let reakontrol = item(&plan, PACKAGE_REAKONTROL);
        assert_eq!(removed(reakontrol), sorted(vec![plugin, maps]));
        assert_eq!(reakontrol.dirs_if_empty, vec![folder]);
    }
}

/// FFmpeg: exactly the files its receipt lists. Files from installs that
/// recorded nothing stay, and a copy without a receipt is most likely the
/// user's own, so RABBIT refuses it. There is no FFmpeg package on macOS.
#[test]
fn ffmpeg_removes_what_its_receipt_lists_and_refuses_a_copy_without_one() {
    let fixture = Fixture::new();
    let recorded_names = [
        "UserPlugins/avcodec-63.dll",
        "UserPlugins/avformat-63.dll",
        "UserPlugins/avutil-61.dll",
        "UserPlugins/ffmpeg.exe",
        "UserPlugins/ffprobe.exe",
    ];
    let recorded: Vec<PathBuf> = recorded_names
        .iter()
        .map(|name| fixture.file(name))
        .collect();
    let older = fixture.file("UserPlugins/avcodec-62.dll");
    let state = receipt(&fixture, &[(PACKAGE_FFMPEG, &recorded_names)]);
    let shipped = Shipped::new(&fixture, Platform::Windows);

    let plan = shipped.plan(&installed(&[PACKAGE_FFMPEG]), Some(&state));
    let ffmpeg = item(&plan, PACKAGE_FFMPEG);
    assert_eq!(ffmpeg.removability, Removability::Removable);
    assert_eq!(removed(ffmpeg), sorted(recorded));
    assert!(
        ffmpeg
            .files
            .iter()
            .all(|file| file.reason == RemovalReason::Receipt && file.receipt_sha256.is_some())
    );
    assert!(!removed(ffmpeg).contains(&older));

    let plan = shipped.plan(&installed(&[PACKAGE_FFMPEG]), None);
    let ffmpeg = item(&plan, PACKAGE_FFMPEG);
    assert_eq!(ffmpeg.removability, Removability::NoReceipt);
    assert!(ffmpeg.removability.is_blocked());
    assert!(ffmpeg.files.is_empty());

    let fixture = Fixture::new();
    let plan = Shipped::new(&fixture, Platform::MacOs).plan(&installed(&[]), None);
    assert!(plan.item(PACKAGE_FFMPEG).is_none());
    assert!(plan.item(PACKAGE_APP2CLAP).is_none());
}

/// app2clap: one file in the user's CLAP folder, which every CLAP host of
/// theirs loads. The folder and the other plug-ins in it stay, every REAPER
/// must be closed, and a portable target can't remove it.
#[test]
fn app2clap_removes_one_file_from_the_clap_folder_on_a_standard_target() {
    let fixture = Fixture::new();
    let clap_dir = fixture.dir.path().join("CLAP");
    let plugin = write(&clap_dir.join("app2clap.clap"), b"file contents");
    let other = write(&clap_dir.join("Other Synth.clap"), b"file contents");
    let state = receipt(&fixture, &[(PACKAGE_APP2CLAP, &[plugin.to_str().unwrap()])]);
    let shipped = Shipped::new(&fixture, Platform::Windows);

    let plan = shipped.plan(&installed(&[PACKAGE_APP2CLAP]), Some(&state));
    let app2clap = item(&plan, PACKAGE_APP2CLAP);
    assert_eq!(app2clap.removability, Removability::Removable);
    assert_eq!(removed(app2clap), vec![plugin.clone()]);
    assert!(!removed(app2clap).contains(&other));
    assert_eq!(reason_of(app2clap, &plugin), RemovalReason::ManifestPath);
    assert!(
        app2clap.files[0].receipt_sha256.is_some(),
        "the receipt lends the file its hash"
    );
    assert!(app2clap.every_reaper_closed);
    assert!(app2clap.warnings.contains(&UninstallWarning::MachineWide));
    let folder = app2clap
        .kept
        .iter()
        .find(|kept| kept.path == clap_dir)
        .expect("the CLAP folder is listed as staying");
    assert_eq!(folder.reason, KeptReason::FolderRoot);
    assert_eq!(
        folder.note_key.as_deref(),
        Some("uninstall-kept-app2clap-clap-folder")
    );
    assert!(!folder.removable_with_settings);

    // Not even "Also remove settings" takes the folder.
    let plan = shipped.plan_with(
        &installed(&[PACKAGE_APP2CLAP]),
        Some(&state),
        InstallationKind::Standard,
        true,
    );
    assert_eq!(removed(item(&plan, PACKAGE_APP2CLAP)), vec![plugin.clone()]);

    let plan = shipped.plan_with(
        &installed(&[PACKAGE_APP2CLAP]),
        Some(&state),
        InstallationKind::Portable,
        false,
    );
    let app2clap = item(&plan, PACKAGE_APP2CLAP);
    assert_eq!(app2clap.removability, Removability::NeedsStandardTarget);
    assert!(app2clap.files.is_empty());

    // The manifest names the file, so a copy without a receipt may go too.
    let plan = shipped.plan(&installed(&[PACKAGE_APP2CLAP]), None);
    let app2clap = item(&plan, PACKAGE_APP2CLAP);
    assert_eq!(app2clap.removability, Removability::Removable);
    assert_eq!(removed(app2clap), vec![plugin]);
    assert_eq!(app2clap.notes, vec![UninstallNote::NoReceipt]);
}

/// Each language pack removes the one file its receipt names, whichever
/// variant that is, then sets REAPER back to no language pack if it used
/// that file. Packs placed by hand beside it stay, on both platforms.
#[test]
fn language_packs_remove_the_pack_their_receipt_names() {
    for platform in [Platform::Windows, Platform::MacOs] {
        for (id, file) in [
            (LANGPACK_DE, "de_DE.ReaperLangPack"),
            (LANGPACK_ES, "es_ES.ReaperLangPack"),
            (LANGPACK_ES, "es_MX.ReaperLangPack"),
            (LANGPACK_FR, "fr_CA.ReaperLangPack"),
        ] {
            let fixture = Fixture::new();
            let relative = format!("LangPack/{file}");
            let pack = fixture.file(&relative);
            let by_hand = fixture.file("LangPack/Deutsch.ReaperLangPack");
            let state = receipt(&fixture, &[(id, &[relative.as_str()])]);
            let shipped = Shipped::new(&fixture, platform);
            let detections = shipped.detect(Some(&state), &[LANGPACK_DE, LANGPACK_ES, LANGPACK_FR]);

            let plan = shipped.plan(&detections, Some(&state));
            let langpack = item(&plan, id);
            assert_eq!(
                langpack.removability,
                Removability::Removable,
                "{platform:?}, {file}"
            );
            assert_eq!(removed(langpack), vec![pack.clone()]);
            assert_eq!(reason_of(langpack, &pack), RemovalReason::Receipt);
            assert_eq!(
                langpack.post,
                vec![UninstallPostStep::ResetReaperLangpackIfSelected {}]
            );
            assert!(langpack.kept.is_empty(), "{:?}", langpack.kept);
            assert!(langpack.notes.is_empty());

            // Remove settings or not, the hand-placed pack and the folder
            // stay, and the other packs aren't installed at all.
            let plan =
                shipped.plan_with(&detections, Some(&state), InstallationKind::Portable, true);
            assert_eq!(removed(item(&plan, id)), vec![pack]);
            assert!(by_hand.is_file());
            for other in [LANGPACK_DE, LANGPACK_ES, LANGPACK_FR] {
                if other != id {
                    assert_eq!(item(&plan, other).removability, Removability::NotInstalled);
                }
            }
        }
    }
}

/// A language pack RABBIT didn't install is the user's: detection doesn't
/// count it as RABBIT's package, and even a detection that did would get a
/// refusal rather than a removal.
#[test]
fn a_language_pack_placed_by_hand_is_refused() {
    let fixture = Fixture::new();
    fixture.file("LangPack/de_DE.ReaperLangPack");
    fixture.file("LangPack/Deutsch.ReaperLangPack");
    let shipped = Shipped::new(&fixture, Platform::Windows);

    let detections = shipped.detect(None, &[LANGPACK_DE]);
    let plan = shipped.plan(&detections, None);
    assert_eq!(
        item(&plan, LANGPACK_DE).removability,
        Removability::NotInstalled
    );

    // A receipt for another package doesn't count either.
    fixture.file("UserPlugins/reaper_sws-x64.dll");
    let state = receipt(
        &fixture,
        &[(PACKAGE_SWS, &["UserPlugins/reaper_sws-x64.dll"])],
    );
    let plan = shipped.plan(&installed(&[LANGPACK_DE]), Some(&state));
    let langpack = item(&plan, LANGPACK_DE);
    assert_eq!(langpack.removability, Removability::NoReceipt);
    assert!(langpack.removability.is_blocked());
    assert!(langpack.files.is_empty());
}

/// What OSARA's installer puts into a REAPER on Windows (osara.nsi): the
/// plug-in for each architecture, OSARA's copy of its key map, its
/// translations and its uninstaller.
const OSARA_WINDOWS_FILES: [&str; 7] = [
    "UserPlugins/reaper_osara32.dll",
    "UserPlugins/reaper_osara64.dll",
    "UserPlugins/reaper_osara_arm64ec.dll",
    "KeyMaps/OSARA.ReaperKeyMap",
    "osara/locale/de_DE.po",
    "osara/locale/fr_CA.po",
    "osara/uninstall.exe",
];

/// What stays when OSARA goes, with the note each gets and the reason
/// "Also remove settings" can't take it: the user's key map, the backups
/// OSARA made of it when it replaced it or merged its keys into it, and
/// `reaper-extstate.ini`, where OSARA keeps its settings beside every other
/// extension's.
const OSARA_KEPT: [(&str, &str, KeptReason); 4] = [
    (
        "reaper-kb.ini",
        "uninstall-kept-osara-key-map",
        KeptReason::ReaperConfiguration,
    ),
    (
        "KeyMaps/OSARAReplacedBackup.ReaperKeyMap",
        "uninstall-kept-osara-replaced-key-map",
        KeptReason::KeyMap,
    ),
    (
        "reaper-kb.ini.osara-merge-backup",
        "uninstall-kept-osara-merged-key-map",
        KeptReason::ReaperConfiguration,
    ),
    (
        "reaper-extstate.ini",
        "uninstall-kept-osara-settings",
        KeptReason::ReaperConfiguration,
    ),
];

/// A resource folder holding what OSARA leaves behind on purpose.
fn write_osara_kept_files(fixture: &Fixture) {
    for (file, _, _) in OSARA_KEPT {
        fixture.file(file);
    }
}

/// That OSARA's plan keeps everything in [`OSARA_KEPT`], as it should, and
/// asks nothing before it goes: no warning, no question. The people who use
/// RABBIT know that REAPER stops speaking without OSARA.
fn assert_osara_keeps_its_users_data_and_asks_nothing(
    fixture: &Fixture,
    osara: &UninstallPlanItem,
) {
    for (file, note_key, reason) in OSARA_KEPT {
        let path = file
            .split('/')
            .fold(fixture.resource.clone(), |path, part| path.join(part));
        assert!(!removed(osara).contains(&path), "{file} goes");
        let kept = osara
            .kept
            .iter()
            .find(|kept| kept.path == path)
            .unwrap_or_else(|| panic!("{file} isn't listed as kept: {:?}", osara.kept));
        assert_eq!(kept.reason, reason, "{file}");
        assert_eq!(kept.note_key.as_deref(), Some(note_key), "{file}");
        assert!(kept.exists, "{file}");
        assert!(
            !kept.removable_with_settings,
            "{file} must stay even with Also remove settings"
        );
    }
    assert!(osara.warnings.is_empty(), "{:?}", osara.warnings);
    assert!(!osara.confirm, "removing OSARA asks no question");
    assert!(!osara.unbacked && !osara.requires_elevation && !osara.every_reaper_closed);
}

/// OSARA on Windows: its plug-ins, its key map file, translations and
/// uninstaller go, then its folders if that empties them, then its entry in
/// Windows' list of installed programs if the entry points into this
/// REAPER. The same on a portable REAPER. The user's key map, the backups
/// OSARA made of it, OSARA's settings and anything else in its folders stay,
/// with or without "Also remove settings".
#[test]
fn osara_on_windows_removes_its_files_and_keeps_the_key_map_and_settings() {
    let fixture = Fixture::new();
    let files: Vec<PathBuf> = OSARA_WINDOWS_FILES
        .iter()
        .map(|file| fixture.file(file))
        .collect();
    write_osara_kept_files(&fixture);
    let own_key_map = fixture.file("KeyMaps/Mine.ReaperKeyMap");
    let own_notes = fixture.file("osara/my notes.txt");
    let sws = fixture.file("UserPlugins/reaper_sws-x64.dll");
    // As install records OSARA: the plug-in and its key map file with a
    // hash, and its folder and the key map it replaced without one, the
    // way user content is recorded.
    let mut state = receipt(
        &fixture,
        &[(
            PACKAGE_OSARA,
            &[
                "UserPlugins/reaper_osara64.dll",
                "KeyMaps/OSARA.ReaperKeyMap",
                "osara",
                "reaper-kb.ini",
            ],
        )],
    );
    for entry in &mut state
        .packages
        .get_mut(PACKAGE_OSARA)
        .unwrap()
        .installed_files
    {
        if entry.path == Path::new("reaper-kb.ini") {
            entry.sha256 = None;
        }
    }
    let resource = &fixture.resource;
    let shipped = Shipped::new(&fixture, Platform::Windows);

    for target in [InstallationKind::Standard, InstallationKind::Portable] {
        for remove_settings in [false, true] {
            let plan = shipped.plan_with(
                &installed(&[PACKAGE_OSARA]),
                Some(&state),
                target,
                remove_settings,
            );
            let osara = item(&plan, PACKAGE_OSARA);

            assert_eq!(osara.removability, Removability::Removable, "{target:?}");
            assert_eq!(removed(osara), sorted(files.clone()), "{target:?}");
            for kept in [&own_key_map, &own_notes, &sws] {
                assert!(!removed(osara).contains(kept), "{}", kept.display());
            }
            let plugin = resource
                .join("UserPlugins")
                .join("reaper_osara_arm64ec.dll");
            assert_eq!(reason_of(osara, &plugin), RemovalReason::MatchingUserPlugin);
            let key_map = resource.join("KeyMaps").join("OSARA.ReaperKeyMap");
            assert_eq!(reason_of(osara, &key_map), RemovalReason::ManifestPath);
            assert_eq!(
                osara.dirs_if_empty,
                vec![
                    resource.join("osara").join("locale"),
                    resource.join("osara")
                ]
            );
            assert_eq!(
                osara.registry_cleanup,
                vec![PlannedRegistryCleanup {
                    key: "OSARA".to_string(),
                    value: "UninstallString".to_string(),
                    dir: resource.join("osara"),
                }]
            );
            assert_osara_keeps_its_users_data_and_asks_nothing(&fixture, osara);
            assert!(osara.notes.is_empty(), "{:?}", osara.notes);
        }
    }

    // A copy RABBIT has no receipt for comes off the same way, with a note.
    let plan = shipped.plan(&installed(&[PACKAGE_OSARA]), None);
    let osara = item(&plan, PACKAGE_OSARA);
    assert_eq!(osara.removability, Removability::Removable);
    assert_eq!(removed(osara), sorted(files));
    assert_eq!(osara.notes, vec![UninstallNote::NoReceipt]);
}

/// OSARA on macOS: RABBIT copies the plug-in, the key map file and the
/// translations out of OSARA's installer, so those go, then the folders if
/// that empties them. macOS has no list of installed programs to clean up.
#[test]
fn osara_on_macos_removes_its_plugin_key_map_and_translations() {
    let fixture = Fixture::new();
    let files: Vec<PathBuf> = [
        "UserPlugins/reaper_osara.dylib",
        "KeyMaps/OSARA.ReaperKeyMap",
        "osara/locale/de_DE.po",
        "osara/locale/pt_BR.po",
    ]
    .iter()
    .map(|file| fixture.file(file))
    .collect();
    write_osara_kept_files(&fixture);
    // A Windows plug-in a shared folder may hold isn't OSARA's here.
    let windows_plugin = fixture.file("UserPlugins/reaper_osara64.dll");
    let shipped = Shipped::new(&fixture, Platform::MacOs);

    for remove_settings in [false, true] {
        let plan = shipped.plan_with(
            &installed(&[PACKAGE_OSARA]),
            None,
            InstallationKind::Standard,
            remove_settings,
        );
        let osara = item(&plan, PACKAGE_OSARA);

        assert_eq!(osara.removability, Removability::Removable);
        assert_eq!(removed(osara), sorted(files.clone()));
        assert!(!removed(osara).contains(&windows_plugin));
        assert_eq!(
            osara.dirs_if_empty,
            vec![
                fixture.resource.join("osara").join("locale"),
                fixture.resource.join("osara")
            ]
        );
        assert!(osara.registry_cleanup.is_empty());
        assert_osara_keeps_its_users_data_and_asks_nothing(&fixture, osara);
    }
}

/// An OSARA that ReaPack installed and still manages is ReaPack's to
/// remove, like any other package. Nothing of it is planned, its entry in
/// Windows' list of installed programs included.
#[test]
fn osara_managed_by_reapack_is_reapacks_to_remove() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_osara64.dll");
    fixture.file("osara/locale/de_DE.po");
    write_test_registry(
        &fixture.resource,
        &[(
            "OSARA",
            "2025.3.6.1234",
            &["UserPlugins/reaper_osara64.dll"],
        )],
    );
    let shipped = Shipped::new(&fixture, Platform::Windows);

    let plan = shipped.plan(&installed(&[PACKAGE_OSARA, PACKAGE_REAPACK]), None);
    let osara = item(&plan, PACKAGE_OSARA);
    assert_eq!(
        osara.removability,
        Removability::ManagedByReaPack {
            path: plugin,
            reapack_package: "OSARA".to_string()
        }
    );
    assert!(osara.files.is_empty() && osara.dirs_if_empty.is_empty());
    assert!(osara.registry_cleanup.is_empty());
}

/// The Windows folders Surge XT's and REAPER's plans name, in the fixture:
/// Common Files and ProgramData beside the resource folder, a Documents
/// folder, and REAPER in a Program Files folder. Nothing here is a real
/// Windows folder.
fn vendor_fixture() -> Fixture {
    let mut fixture = Fixture::new();
    let root = fixture.dir.path().to_path_buf();
    fixture.roots.common_program_files = Some(root.join("Common Files"));
    fixture.roots.program_data = Some(root.join("ProgramData"));
    fixture.roots.program_files = vec![root.join("Program Files")];
    fixture.roots.documents = Some(root.join("Documents"));
    let app = root
        .join("Program Files")
        .join("REAPER (x64)")
        .join("reaper.exe");
    fixture.roots.app_dir = app.parent().map(Path::to_path_buf);
    fixture.roots.app_path = Some(app);
    fixture
}

/// Surge XT on Windows: its own Inno Setup uninstaller removes it, with
/// administrator rights and no backup, once RABBIT found it in HKLM and
/// trusts it. Afterwards RABBIT removes the links into Surge's data folder
/// and the vendor folders left empty, and checks that every plug-in format
/// is gone. Its patches and fonts stay, Surge itself must be closed, and so
/// must every REAPER.
#[test]
fn surge_xt_on_windows_runs_its_own_uninstaller() {
    let fixture = vendor_fixture();
    let common = fixture.roots.common_program_files.clone().unwrap();
    let patches = write(
        &fixture
            .roots
            .documents
            .clone()
            .unwrap()
            .join("Surge XT")
            .join("Patches")
            .join("Mine.fxp"),
        b"patch",
    )
    .parent()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf();
    let uninstaller = fixture
        .roots
        .program_data
        .clone()
        .unwrap()
        .join("Surge XT")
        .join("uninstall")
        .join("unins000.exe");
    let shipped = Shipped::new(&fixture, Platform::Windows);
    let detections = installed(&[PACKAGE_SURGE_XT]);

    // Without an uninstaller RABBIT trusts, Surge XT stays, and nothing asks
    // for permission.
    let plan = shipped.plan(&detections, None);
    let surge = item(&plan, PACKAGE_SURGE_XT);
    assert_eq!(
        surge.removability,
        Removability::NoTrustedUninstaller {
            refusal: VendorRefusal::NotListed
        }
    );
    assert!(!surge.can_remove());

    let plan = shipped.plan_looking_up(
        &detections,
        None,
        InstallationKind::Standard,
        true,
        &FoundAt(uninstaller.clone()),
    );
    let surge = item(&plan, PACKAGE_SURGE_XT);
    assert_eq!(surge.removability, Removability::Removable);
    assert!(surge.can_remove() && surge.needs_elevated_helper());
    assert!(surge.requires_elevation && surge.unbacked && surge.confirm);
    assert!(surge.every_reaper_closed);
    assert_eq!(
        surge.blocking_programs,
        ["Surge XT.exe", "Surge XT Effects.exe"]
    );
    assert!(surge.files.is_empty(), "nothing moves into a backup");
    let summary = surge.summary.as_ref().unwrap();
    assert_eq!(summary.removes_key, "uninstall-summary-surge-removes");
    assert_eq!(summary.keeps_key, "uninstall-summary-surge-keeps");
    // Its summary already says it goes for every music program; the same
    // thing isn't said twice.
    assert!(!surge.warnings.contains(&UninstallWarning::MachineWide));
    assert_eq!(
        surge.notes,
        vec![UninstallNote::Manifest {
            note_key: "uninstall-note-surge-fonts-stay".to_string()
        }]
    );
    // The patches stay, even with "Also remove settings".
    assert_eq!(
        kept_data(surge),
        vec![(patches, "uninstall-kept-surge-user-data".to_string(), true)]
    );
    assert!(surge.kept.iter().all(|kept| !kept.removable_with_settings));

    let run = surge.vendor_run.as_ref().unwrap();
    assert_eq!(run.kind, VendorUninstallerKind::Inno);
    assert_eq!(run.uninstaller, uninstaller);
    assert_eq!(
        run.arguments,
        ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]
    );
    assert_eq!(run.timeout_seconds, 600);
    assert_eq!(run.still_waiting_seconds, 30);
    let vst3 = common.join("VST3").join("Surge Synth Team");
    let clap = common.join("CLAP").join("Surge Synth Team");
    assert_eq!(
        run.verify_gone,
        [
            vst3.join("Surge XT.vst3"),
            vst3.join("Surge XT Effects.vst3"),
            clap.join("Surge XT.clap"),
            clap.join("Surge XT Effects.clap"),
        ]
    );
    // The links first (named again as empty folders, in case a plain empty
    // folder stands where a link was expected), then the vendor folders if
    // nothing else is left in them. Each is listed once.
    assert_eq!(
        run.remove_leftovers,
        [
            clap.join("SurgeXTData"),
            vst3.join("SurgeXTData"),
            clap.clone(),
            vst3.clone(),
        ]
    );
    assert!(run.report_leftovers.is_empty());

    // A portable REAPER can't remove it; the standard REAPER can.
    let plan = shipped.plan_looking_up(
        &detections,
        None,
        InstallationKind::Portable,
        false,
        &FoundAt(uninstaller),
    );
    let surge = item(&plan, PACKAGE_SURGE_XT);
    assert_eq!(surge.removability, Removability::NeedsStandardTarget);
    assert!(surge.vendor_run.is_none());
}

/// A standard REAPER on Windows: its own NSIS uninstaller removes the
/// program, with administrator rights and no backup, and must leave the
/// resource folder alone. RABBIT checks afterwards that reaper.exe is gone,
/// warns that the extensions stay, and lists the resource folder as kept,
/// never as something to remove. A portable REAPER stays manual.
#[test]
fn reaper_on_windows_runs_its_own_uninstaller_and_keeps_the_resource_folder() {
    let fixture = vendor_fixture();
    let app = fixture.roots.app_path.clone().unwrap();
    let uninstaller = app.with_file_name("Uninstall.exe");
    let shipped = Shipped::new(&fixture, Platform::Windows);
    let detections = installed(&[PACKAGE_REAPER, PACKAGE_OSARA, PACKAGE_SWS]);

    let plan = shipped.plan_looking_up(
        &detections,
        None,
        InstallationKind::Standard,
        true,
        &FoundAt(uninstaller.clone()),
    );
    let reaper = item(&plan, PACKAGE_REAPER);
    assert_eq!(reaper.removability, Removability::Removable);
    assert!(reaper.needs_elevated_helper() && reaper.confirm);
    assert!(reaper.every_reaper_closed);
    assert!(reaper.blocking_programs.is_empty());
    assert!(reaper.files.is_empty());
    let summary = reaper.summary.as_ref().unwrap();
    assert_eq!(
        summary.removes_key,
        "uninstall-summary-reaper-windows-removes"
    );
    assert_eq!(summary.keeps_key, "uninstall-summary-reaper-keeps");
    let run = reaper.vendor_run.as_ref().unwrap();
    assert_eq!(run.kind, VendorUninstallerKind::Nsis);
    assert_eq!(run.uninstaller, uninstaller);
    assert_eq!(run.arguments, ["/S"]);
    assert_eq!(run.timeout_seconds, 600);
    assert_eq!(run.verify_gone, [app]);
    assert!(run.remove_leftovers.is_empty());

    // The resource folder stays, whole, whatever the user asks for.
    let resource = reaper
        .kept
        .iter()
        .find(|kept| kept.path == fixture.resource)
        .expect("the resource folder is listed as staying");
    assert_eq!(resource.reason, KeptReason::FolderRoot);
    assert_eq!(
        resource.note_key.as_deref(),
        Some("uninstall-kept-reaper-resource")
    );
    assert!(!resource.removable_with_settings);

    // Its extensions stay, and the user is told so once, naming them
    // (decision 16).
    assert_eq!(
        reaper.warnings,
        [UninstallWarning::DependentsStay {
            package_ids: vec![PACKAGE_OSARA.to_string(), PACKAGE_SWS.to_string()]
        }]
    );

    // REAPER comes off last, after everything that installs on top of it.
    let order: Vec<&str> = plan
        .in_removal_order(&[
            PACKAGE_REAPER.to_string(),
            PACKAGE_SURGE_XT.to_string(),
            PACKAGE_SWS.to_string(),
        ])
        .iter()
        .map(|item| item.package_id.as_str())
        .collect();
    assert_eq!(order.last(), Some(&PACKAGE_REAPER));

    let plan = shipped.plan_looking_up(
        &detections,
        None,
        InstallationKind::Portable,
        false,
        &FoundAt(uninstaller),
    );
    assert_eq!(
        item(&plan, PACKAGE_REAPER).removability,
        Removability::Manual {
            note_key: "uninstall-manual-reaper-portable".to_string()
        }
    );
}

// ---------------------------------------------------------------------------
// REAPER on macOS, from the shipped plan, on a stand-in Mac.
// ---------------------------------------------------------------------------

/// The resource folder stays whole, with REAPER's own note, whatever the
/// user asks for: it is a folder root, never one of REAPER's settings, and
/// nothing REAPER removes is the folder itself.
fn assert_keeps_the_resource_folder(reaper: &UninstallPlanItem, resource: &Path) {
    let kept = reaper
        .kept
        .iter()
        .find(|kept| kept.path == resource)
        .unwrap_or_else(|| panic!("the resource folder isn't listed as staying: {reaper:#?}"));
    assert_eq!(kept.reason, KeptReason::FolderRoot);
    assert_eq!(
        kept.note_key.as_deref(),
        Some("uninstall-kept-reaper-resource")
    );
    assert!(!kept.removable_with_settings);
    assert!(
        reaper.files.iter().all(|file| file.path != resource),
        "{:?}",
        reaper.files
    );
}

/// One engine run of the shipped macOS REAPER on `mac`, with `objects`
/// answering for macOS. Detection finds REAPER by its app, before the run
/// and after it, as it does on a Mac. Nothing asks for a password: the
/// administrator shell is one that never runs.
fn run_reaper_on_mac(
    mac: &MacFixture,
    target: InstallationKind,
    objects: &dyn MacosObjects,
    remove_settings: bool,
    dry_run: bool,
) -> UninstallReport {
    let detect = || {
        let ids: &[&str] = if mac.app.exists() {
            &[PACKAGE_REAPER]
        } else {
            &[]
        };
        Ok(installed(ids))
    };
    run_reaper_on_mac_detecting(mac, target, objects, remove_settings, dry_run, &detect).unwrap()
}

/// The same run, with `detect` standing in for detection.
fn run_reaper_on_mac_detecting(
    mac: &MacFixture,
    target: InstallationKind,
    objects: &dyn MacosObjects,
    remove_settings: bool,
    dry_run: bool,
    detect: &dyn Fn() -> crate::error::Result<Vec<ComponentDetection>>,
) -> crate::error::Result<UninstallReport> {
    let specs = builtin_package_specs(Platform::MacOs);
    let reapers = Vec::<RunningProcess>::new;
    let named = |_: &[String]| Vec::new();
    execute_uninstall_in(
        &UninstallRequest {
            resource_path: mac.resource.clone(),
            platform: Platform::MacOs,
            package_ids: vec![PACKAGE_REAPER.to_string()],
            remove_settings,
            dry_run,
            target_app_path: Some(mac.app.clone()),
            target: Some(target),
        },
        &UninstallEnvironment {
            specs: &specs,
            roots: &mac.roots,
            target,
            target_app_path: Some(mac.app.clone()),
            detect,
            reaper_processes: &reapers,
            named_processes: &named,
            volumes: &OneVolume,
            installed_programs: None,
            vendor_lookup: &NothingListed,
            elevated_helper: &NoElevatedHelper,
            macos_objects: objects,
            administrator_shell: &NoAdministratorShell,
        },
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
}

/// `path` as the text root is given.
fn text(path: &Path) -> String {
    path.to_str().unwrap().to_string()
}

/// The shipped macOS plan for REAPER: one plan for a standard and a
/// portable REAPER, which moves the app to the Trash, asks root only when
/// it must, and keeps the resource folder. Windows keeps its own plans: a
/// standard REAPER goes through its own uninstaller, a portable one stays
/// the user's to remove by hand.
#[test]
fn reapers_shipped_plans_send_the_mac_app_to_the_trash_and_leave_windows_as_it_was() {
    let reaper = embedded_package_manifest()
        .packages
        .into_iter()
        .find(|spec| spec.id == PACKAGE_REAPER)
        .expect("REAPER is shipped")
        .uninstall;

    let standard = reaper
        .plan_for(Platform::MacOs, InstallationKind::Standard)
        .expect("a standard REAPER on macOS has a plan");
    let portable = reaper
        .plan_for(Platform::MacOs, InstallationKind::Portable)
        .expect("a portable REAPER on macOS has a plan");
    assert!(std::ptr::eq(standard, portable), "one plan for both");
    assert_eq!(standard.strategy, UninstallStrategy::RemoveFiles);
    assert_eq!(standard.elevation, UninstallElevation::IfNeeded);
    assert_eq!(standard.backup, UninstallBackup::ToTrash);
    let [tree] = standard.remove.trees.as_slice() else {
        panic!("one tree: {:?}", standard.remove.trees);
    };
    assert_eq!(tree.path.as_str(), "{app_path}");
    assert_eq!(tree.must_end_with.as_deref(), Some(".app"));
    assert_eq!(tree.name_contains.as_deref(), Some("REAPER"));
    assert_eq!(tree.bundle_id.as_deref(), Some("com.cockos.reaper"));
    assert!(standard.remove.paths.is_empty() && !standard.remove.receipt_files);
    let [kept] = standard.kept_user_data.as_slice() else {
        panic!("one kept folder: {:?}", standard.kept_user_data);
    };
    assert_eq!(kept.path.as_str(), "{resource}");
    assert!(standard.blocking_programs.is_empty() && standard.post.is_empty());

    let windows = reaper
        .plan_for(Platform::Windows, InstallationKind::Standard)
        .expect("a standard REAPER on Windows has a plan");
    assert_eq!(windows.strategy, UninstallStrategy::VendorUninstaller);
    assert_eq!(windows.elevation, UninstallElevation::Required);
    assert_eq!(
        windows.summary.as_ref().unwrap().removes_key,
        "uninstall-summary-reaper-windows-removes"
    );
    let vendor = windows.vendor_uninstaller.as_ref().unwrap();
    assert_eq!(vendor.kind, VendorUninstallerKind::Nsis);
    let windows_portable = reaper
        .plan_for(Platform::Windows, InstallationKind::Portable)
        .expect("a portable REAPER on Windows has a plan");
    assert_eq!(windows_portable.strategy, UninstallStrategy::Manual);
    assert_eq!(
        windows_portable.note_key.as_deref(),
        Some("uninstall-manual-reaper-portable")
    );
}

/// The two sentences Review shows for REAPER on macOS, and the note on the
/// folder that stays, read in every language RABBIT ships, none of them
/// borrowed from English.
#[test]
fn reapers_macos_plan_reads_in_every_language() {
    let reaper = embedded_package_manifest()
        .packages
        .into_iter()
        .find(|spec| spec.id == PACKAGE_REAPER)
        .expect("REAPER is shipped")
        .uninstall;
    let plan = reaper
        .plan_for(Platform::MacOs, InstallationKind::Standard)
        .unwrap();
    let summary = plan.summary.as_ref().unwrap();
    let mut keys = vec![summary.removes_key.as_str(), summary.keeps_key.as_str()];
    keys.extend(
        plan.kept_user_data
            .iter()
            .map(|kept| kept.note_key.as_str()),
    );
    assert_eq!(
        keys,
        [
            "uninstall-summary-reaper-macos-removes",
            "uninstall-summary-reaper-keeps",
            "uninstall-kept-reaper-resource",
        ]
    );
    assert_eq!(embedded_locales().len(), 5);
    for &locale in embedded_locales() {
        let localizer = Localizer::embedded(locale).unwrap();
        for key in &keys {
            let text = localizer.text(key);
            assert!(
                !text.missing && !text.fallback_used && text.formatting_errors.is_empty(),
                "{locale}: {key}: {text:?}"
            );
            assert_eq!(text.locale, locale);
            assert!(
                !text.value.trim().is_empty() && !text.value.contains('{'),
                "{locale}: {key}: {}",
                text.value
            );
        }
    }
}

/// REAPER.app the user may move, in /Applications: the shipped plan sends
/// it to the Trash as the user, with no password. A dry run gives root
/// nothing and moves nothing.
#[test]
fn a_reaper_app_the_user_may_move_goes_to_the_trash_with_no_administrator_step() {
    let mac = MacFixture::new();
    let objects = mac.objects(owned_by_me());
    let shipped = Shipped::on_mac(&mac, &objects);

    let plan = shipped.plan(&installed(&[PACKAGE_REAPER]), None);
    let reaper = item(&plan, PACKAGE_REAPER);

    assert_eq!(reaper.strategy, Some(UninstallStrategy::RemoveFiles));
    assert_eq!(reaper.removability, Removability::Removable);
    assert_eq!(reaper.route(), RemovalRoute::Trash);
    assert!(reaper.can_remove());
    assert!(!reaper.requires_elevation && !reaper.needs_administrator());
    // Only the target's REAPER must be closed: no pgrep looks for others.
    assert!(!reaper.every_reaper_closed);
    assert!(reaper.blocking_programs.is_empty());
    // Gone from its place all the same, so the question before starting.
    assert_eq!(reaper.backup, UninstallBackup::ToTrash);
    assert!(reaper.unbacked && reaper.confirm);
    let [app] = reaper.files.as_slice() else {
        panic!("one tree: {:?}", reaper.files);
    };
    assert_eq!(app.path, mac.app);
    assert_eq!(app.reason, RemovalReason::Tree);
    assert_eq!(app.bundle_id.as_deref(), Some("com.cockos.reaper"));
    assert!(app.is_dir);
    let summary = reaper.summary.as_ref().unwrap();
    assert_eq!(
        summary.removes_key,
        "uninstall-summary-reaper-macos-removes"
    );
    assert_eq!(summary.keeps_key, "uninstall-summary-reaper-keeps");
    assert_keeps_the_resource_folder(reaper, &mac.resource);
    // No receipt lists REAPER here, but the app is found by its name and
    // bundle ID alone, so no note says RABBIT goes by what REAPER is known
    // to install.
    assert!(reaper.notes.is_empty(), "{:?}", reaper.notes);
    assert!(reaper.warnings.is_empty(), "{:?}", reaper.warnings);

    let report = run_reaper_on_mac(&mac, InstallationKind::Standard, &objects, false, true);
    let planned = report.item(PACKAGE_REAPER).unwrap();
    assert_eq!(planned.status, UninstallStatus::WouldRemove);
    assert!(
        report.administrator_steps.is_empty(),
        "{:?}",
        report.administrator_steps
    );
    assert!(objects.trashed().is_empty());
    assert!(join(&mac.app, "Contents/MacOS/REAPER").is_file());
}

/// REAPER.app only root may move, directly in /Applications: the shipped
/// plan lets the run's one administrator command move it into the user's
/// Trash (decision 32), with every REAPER closed. Root gets exactly these
/// steps: pgrep for REAPER first, then the Trash checked as a folder and as
/// no link, then the app moved in under a fresh name. A dry run lists the
/// same steps and runs none of them.
#[test]
fn a_root_owned_reaper_app_in_applications_goes_to_the_trash_through_the_administrator_command() {
    let mac = MacFixture::new();
    let objects = mac.objects(owned_by_root());
    let shipped = Shipped::on_mac(&mac, &objects);

    let plan = shipped.plan(&installed(&[PACKAGE_REAPER]), None);
    let reaper = item(&plan, PACKAGE_REAPER);

    assert_eq!(reaper.removability, Removability::Removable);
    assert_eq!(reaper.route(), RemovalRoute::Administrator);
    assert!(reaper.requires_elevation && reaper.needs_administrator() && reaper.can_remove());
    // Root's pgrep finds every REAPER by name, so RABBIT's own checks ask
    // for every REAPER closed too.
    assert!(reaper.every_reaper_closed);
    assert_eq!(removed(reaper), std::slice::from_ref(&mac.app));
    assert_keeps_the_resource_folder(reaper, &mac.resource);
    assert!(reaper.notes.is_empty(), "{:?}", reaper.notes);

    // The command root would be given for it, word for word.
    let trash = my_trash().path;
    let at = SystemTime::now();
    let package = prepare_package(reaper, &mac.roots, &objects, at).unwrap();
    let batch = administrator_steps(&[package], None).unwrap();
    let offset = rabbit_platform::local_utc_offset_seconds(at).unwrap_or(0);
    let fresh = trash.join(trash_name(&mac.app, at, offset).unwrap());
    let command =
        |program: &str, arguments: &[&str]| ShellCommand::new(program, arguments.iter().copied());
    let pattern = MACOS_REAPER_PROCESS_NAMES.join("|");
    assert_eq!(pattern, "REAPER|reaper64");
    assert_eq!(
        batch.steps,
        [
            ShellStep::RunExpectingNoMatch(command(PGREP, &["-x", "-i", "--", &pattern])),
            ShellStep::Run(command(TEST, &["-d", &text(&trash)])),
            ShellStep::Run(command(TEST, &["!", "-L", &text(&trash)])),
            ShellStep::Run(command(MV, &["-h", "-n", &text(&mac.app), &text(&fresh)])),
        ]
    );

    let report = run_reaper_on_mac(&mac, InstallationKind::Standard, &objects, false, true);
    assert_eq!(
        report.item(PACKAGE_REAPER).unwrap().status,
        UninstallStatus::WouldRemove
    );
    let record = |kind: AdministratorStepKind, target: String| AdministratorStepRecord {
        package_id: Some(PACKAGE_REAPER.to_string()),
        kind,
        target,
        destination: None,
        verdict: AdministratorStepVerdict::WouldRun,
    };
    let [programs, is_folder, is_not_link, moved] = report.administrator_steps.as_slice() else {
        panic!("four steps: {:#?}", report.administrator_steps);
    };
    assert_eq!(
        *programs,
        AdministratorStepRecord {
            package_id: None,
            kind: AdministratorStepKind::ProgramsClosed,
            target: MACOS_REAPER_PROCESS_NAMES.join(", "),
            destination: None,
            verdict: AdministratorStepVerdict::WouldRun,
        }
    );
    assert_eq!(
        *is_folder,
        record(AdministratorStepKind::TrashIsFolder, text(&trash))
    );
    assert_eq!(
        *is_not_link,
        record(AdministratorStepKind::TrashIsNotLink, text(&trash))
    );
    // The name in the Trash is made from the time a run moves the app, so
    // only its shape is the same here.
    let destination = PathBuf::from(moved.destination.clone().expect("where the app would go"));
    assert_eq!(
        *moved,
        AdministratorStepRecord {
            destination: Some(text(&destination)),
            ..record(AdministratorStepKind::MoveToTrash, text(&mac.app))
        }
    );
    assert_eq!(destination.parent(), Some(trash.as_path()));
    let name = destination.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("REAPER 20")
            && name.ends_with(".app")
            && name.len() == "REAPER 2026-11-07 15.30.12.app".len(),
        "{name}"
    );
    assert!(objects.trashed().is_empty());
    assert!(join(&mac.app, "Contents/MacOS/REAPER").is_file());
}

/// Whether a reason is the one a case expects.
type IsExpected = fn(&ByHandReason) -> bool;

/// Every REAPER.app in /Applications the shipped plan leaves to the user to
/// move in Finder, and why: a Locked one, which not even root can move; one
/// that isn't REAPER by its bundle ID; one RABBIT can't check; and one only
/// root could move whose Trash root can't safely move it into. None asks for
/// a password, and the resource folder is still listed as staying.
#[test]
fn reaper_apps_rabbit_cant_move_on_macos_are_left_to_the_user_with_the_reason() {
    let mac = MacFixture::new();
    let app = mac.app.clone();
    let cases: Vec<(&str, FakeMacosObjects, IsExpected)> = vec![
        (
            "Locked",
            mac.objects(ObjectFacts {
                locked: true,
                ..owned_by_me()
            }),
            |reason| *reason == ByHandReason::Locked,
        ),
        (
            "Locked, and only root's",
            mac.objects(ObjectFacts {
                locked: true,
                ..owned_by_root()
            }),
            |reason| *reason == ByHandReason::Locked,
        ),
        (
            "another app's bundle ID",
            mac.objects(ObjectFacts {
                bundle_identifier: Some("com.example.other".to_string()),
                ..owned_by_root()
            }),
            |reason| *reason == ByHandReason::NotTheExpectedApp,
        ),
        (
            "no bundle ID",
            mac.objects(ObjectFacts {
                bundle_identifier: None,
                ..owned_by_me()
            }),
            |reason| *reason == ByHandReason::NotTheExpectedApp,
        ),
        (
            "unreadable",
            FakeMacosObjects::new()
                .with_unreadable_object(&app, io::ErrorKind::PermissionDenied)
                .with_trash(my_trash()),
            |reason| matches!(reason, ByHandReason::CouldNotCheck { .. }),
        ),
        (
            "no Trash",
            FakeMacosObjects::new().with_object(&app, owned_by_root()),
            |reason| *reason == ByHandReason::NoTrashFolder,
        ),
        (
            "Trash on another disk",
            FakeMacosObjects::new()
                .with_object(&app, owned_by_root())
                .with_trash(TrashFolderFacts {
                    dev: DISK + 1,
                    ..my_trash()
                }),
            |reason| *reason == ByHandReason::NotOnTrashVolume,
        ),
    ];
    for (case, objects, expected) in cases {
        let shipped = Shipped::on_mac(&mac, &objects);
        let plan = shipped.plan(&installed(&[PACKAGE_REAPER]), None);
        let reaper = item(&plan, PACKAGE_REAPER);
        match &reaper.removability {
            Removability::ByHand { path, reason } => {
                assert_eq!(path, &app, "{case}");
                assert!(expected(reason), "{case}: {reason:?}");
            }
            other => panic!("{case}: expected the app to be left to the user, got {other:?}"),
        }
        assert!(reaper.removability.is_blocked(), "{case}");
        assert!(
            !reaper.requires_elevation && !reaper.can_remove() && reaper.files.is_empty(),
            "{case}"
        );
        assert_keeps_the_resource_folder(reaper, &mac.resource);

        // A dry run stops at it, and gives root nothing.
        let report = run_reaper_on_mac(&mac, InstallationKind::Standard, &objects, false, true);
        let planned = report.item(PACKAGE_REAPER).unwrap();
        assert_eq!(planned.status, UninstallStatus::Blocked, "{case}");
        assert_eq!(
            planned.message_code,
            UninstallMessage::NotRemovable {
                removability: reaper.removability.clone()
            },
            "{case}"
        );
        assert!(report.administrator_steps.is_empty(), "{case}");
        assert!(objects.trashed().is_empty(), "{case}");
    }
    assert!(join(&app, "Contents/MacOS/REAPER").is_file());
}

/// Root moves REAPER.app only out of /Applications, a folder only an
/// administrator can change. One only root may move anywhere else, in the
/// user's own Applications folder or in a portable REAPER's folder, is the
/// user's to move in Finder.
#[test]
fn a_root_owned_reaper_app_outside_applications_is_left_to_the_user() {
    for (folder, target) in [
        ("home/Applications", InstallationKind::Standard),
        ("REAPER", InstallationKind::Portable),
    ] {
        let mut mac = MacFixture::new();
        let parent = join(mac.dir.path(), folder);
        let app = parent.join("REAPER.app");
        write(&join(&app, "Contents/MacOS/REAPER"), b"REAPER");
        mac.roots.app_path = Some(app.clone());
        mac.roots.app_dir = Some(parent);
        mac.app = app.clone();
        let objects = mac.objects(owned_by_root());
        let shipped = Shipped::on_mac(&mac, &objects);

        let plan = shipped.plan_with(&installed(&[PACKAGE_REAPER]), None, target, false);
        let reaper = item(&plan, PACKAGE_REAPER);

        assert_eq!(
            reaper.removability,
            Removability::ByHand {
                path: app.clone(),
                reason: ByHandReason::NotMovableByYou,
            },
            "{folder}"
        );
        assert!(
            !reaper.requires_elevation && !reaper.can_remove(),
            "{folder}"
        );
        assert!(reaper.files.is_empty(), "{folder}");
        assert_keeps_the_resource_folder(reaper, &mac.resource);
    }
}

/// A portable REAPER's app sits in its own folder, which is also its
/// resource folder. The same shipped plan sends the app to the Trash as the
/// user, and the folder, with everything of the user's in it, stays as it
/// was, even with "Also remove settings": only RABBIT's own folder in it
/// gains the run's report and receipt.
#[test]
fn a_portable_reaper_app_goes_to_the_trash_and_its_folder_stays_untouched() {
    let mut mac = MacFixture::new();
    let portable_app = mac.resource.join("REAPER.app");
    write(&join(&portable_app, "Contents/MacOS/REAPER"), b"REAPER");
    mac.roots.app_path = Some(portable_app.clone());
    mac.roots.app_dir = Some(mac.resource.clone());
    mac.app = portable_app.clone();
    for file in [
        "reaper.ini",
        "reaper-kb.ini",
        "UserPlugins/reaper_sws.dylib",
        "KeyMaps/Mine.ReaperKeyMap",
        "Projects/Song.rpp",
    ] {
        write(&join(&mac.resource, file), file.as_bytes());
    }
    let trash = mac.dir.path().join("Trash");
    fs::create_dir(&trash).unwrap();
    let objects = mac
        .objects(owned_by_me())
        .with_trash(TrashFolderFacts {
            path: trash.clone(),
            ..my_trash()
        })
        .with_trash_bin(&trash);
    let shipped = Shipped::on_mac(&mac, &objects);

    for remove_settings in [false, true] {
        let plan = shipped.plan_with(
            &installed(&[PACKAGE_REAPER]),
            None,
            InstallationKind::Portable,
            remove_settings,
        );
        let reaper = item(&plan, PACKAGE_REAPER);
        assert_eq!(reaper.removability, Removability::Removable);
        assert_eq!(reaper.route(), RemovalRoute::Trash);
        assert!(!reaper.requires_elevation && !reaper.every_reaper_closed);
        assert_eq!(removed(reaper), std::slice::from_ref(&portable_app));
        assert_keeps_the_resource_folder(reaper, &mac.resource);
    }

    // Everything but the app and RABBIT's own folder.
    let users = |found: BTreeMap<PathBuf, Option<Vec<u8>>>| {
        let rabbit = mac.resource.join("RABBIT");
        found
            .into_iter()
            .filter(|(path, _)| !path.starts_with(&portable_app) && !path.starts_with(&rabbit))
            .collect::<BTreeMap<_, _>>()
    };
    let before = users(snapshot(&mac.resource));
    assert!(before.contains_key(&mac.resource.join("reaper.ini")));

    let report = run_reaper_on_mac(&mac, InstallationKind::Portable, &objects, true, false);

    let reaper = report.item(PACKAGE_REAPER).unwrap();
    assert_eq!(
        reaper.status,
        UninstallStatus::Removed,
        "{}",
        reaper.message
    );
    assert_eq!(objects.trashed(), std::slice::from_ref(&portable_app));
    assert!(!portable_app.exists());
    assert!(join(&trash, "REAPER.app/Contents/MacOS/REAPER").is_file());
    assert_eq!(users(snapshot(&mac.resource)), before);
    assert!(report.administrator_steps.is_empty());
}

/// A target that names a REAPER.app that isn't there, such as a mistyped
/// `--target-app-path`, while detection still finds REAPER at another app:
/// moving nothing would report REAPER removed. The shipped plan leaves it
/// to the user instead, naming the path it looked for, and a real run
/// changes nothing; the app detection found stays where it is.
#[test]
fn a_reaper_app_that_isnt_where_the_target_says_is_left_to_the_user() {
    let mut mac = MacFixture::new();
    let found = mac.app.clone();
    let missing = join(&mac.system, "Applications/REAPER64.app");
    mac.roots.app_path = Some(missing.clone());
    mac.app = missing.clone();
    let trash = mac.dir.path().join("Trash");
    fs::create_dir(&trash).unwrap();
    let objects = FakeMacosObjects::new()
        .with_object(&found, owned_by_me())
        .with_trash(TrashFolderFacts {
            path: trash.clone(),
            ..my_trash()
        })
        .with_trash_bin(&trash);
    let shipped = Shipped::on_mac(&mac, &objects);

    let plan = shipped.plan(&installed(&[PACKAGE_REAPER]), None);
    let reaper = item(&plan, PACKAGE_REAPER);
    match &reaper.removability {
        Removability::ByHand {
            path,
            reason: ByHandReason::CouldNotCheck { .. },
        } => assert_eq!(path, &missing),
        other => panic!("expected the missing app to be left to the user, got {other:?}"),
    }
    assert!(reaper.removability.is_blocked() && !reaper.can_remove());
    assert!(reaper.files.is_empty());
    assert_keeps_the_resource_folder(reaper, &mac.resource);

    // Detection finds REAPER by the app that is there, as it would on a Mac.
    let detect = || Ok(installed(&[PACKAGE_REAPER]));
    for dry_run in [true, false] {
        let report = run_reaper_on_mac_detecting(
            &mac,
            InstallationKind::Standard,
            &objects,
            false,
            dry_run,
            &detect,
        )
        .unwrap();
        let planned = report.item(PACKAGE_REAPER).unwrap();
        assert_eq!(
            planned.status,
            UninstallStatus::Blocked,
            "{dry_run}: {}",
            planned.message
        );
        assert!(planned.removed.is_empty(), "{dry_run}");
        assert!(report.administrator_steps.is_empty(), "{dry_run}");
    }
    assert!(objects.trashed().is_empty());
    assert!(join(&found, "Contents/MacOS/REAPER").is_file());
    assert!(fs::read_dir(&trash).unwrap().next().is_none());
}

/// A REAPER.app that was dragged into /Applications and never opened has
/// no resource folder yet. A real run takes the install lock all the same,
/// before it plans, which makes the folder on purpose; afterwards the
/// receipt there records that REAPER was uninstalled, so the next install
/// doesn't tick it again. A dry run writes nothing, the folder included.
#[test]
fn removing_a_reaper_whose_resource_folder_doesnt_exist_holds_the_install_lock() {
    let mac = MacFixture::new();
    fs::remove_dir_all(&mac.resource).unwrap();
    let trash = mac.dir.path().join("Trash");
    fs::create_dir(&trash).unwrap();
    let objects = mac
        .objects(owned_by_me())
        .with_trash(TrashFolderFacts {
            path: trash.clone(),
            ..my_trash()
        })
        .with_trash_bin(&trash);
    let lock = crate::lock::default_package_install_lock_path(&mac.resource);
    // Detection is asked before the plan and again after the move; the
    // lock must be held both times.
    let unlocked = std::cell::Cell::new(0);
    let detect = || {
        if !lock.is_file() {
            unlocked.set(unlocked.get() + 1);
        }
        let ids: &[&str] = if mac.app.exists() {
            &[PACKAGE_REAPER]
        } else {
            &[]
        };
        Ok(installed(ids))
    };

    let report = run_reaper_on_mac_detecting(
        &mac,
        InstallationKind::Standard,
        &objects,
        false,
        true,
        &detect,
    )
    .unwrap();
    assert_eq!(
        report.item(PACKAGE_REAPER).unwrap().status,
        UninstallStatus::WouldRemove
    );
    assert!(!mac.resource.exists(), "a dry run makes no folder");
    unlocked.set(0);

    let report = run_reaper_on_mac_detecting(
        &mac,
        InstallationKind::Standard,
        &objects,
        false,
        false,
        &detect,
    )
    .unwrap();
    let reaper = report.item(PACKAGE_REAPER).unwrap();
    assert_eq!(
        reaper.status,
        UninstallStatus::Removed,
        "{}",
        reaper.message
    );
    assert_eq!(unlocked.get(), 0, "detection ran without the install lock");
    assert!(!lock.exists(), "the lock is let go at the end");
    assert_eq!(objects.trashed(), std::slice::from_ref(&mac.app));
    let state = crate::receipt::load_install_state(&mac.resource)
        .unwrap()
        .expect("the receipt is saved in the new folder");
    assert!(state.declined_packages.contains(PACKAGE_REAPER));
    assert!(report.saved_report.is_some());
}

/// A real run against a resource folder that doesn't exist, with nothing
/// there to remove, such as a mistyped --resource-path: the folders the
/// install lock needed go again, and no report is left behind.
#[test]
fn a_run_that_changes_nothing_leaves_no_folder_where_there_was_none() {
    let mac = MacFixture::new();
    fs::remove_dir_all(&mac.resource).unwrap();
    fs::remove_dir_all(&mac.app).unwrap();
    let objects = mac.objects(owned_by_me());
    let detect = || Ok(installed(&[]));

    let report = run_reaper_on_mac_detecting(
        &mac,
        InstallationKind::Standard,
        &objects,
        false,
        false,
        &detect,
    )
    .unwrap();
    assert_eq!(
        report.item(PACKAGE_REAPER).unwrap().status,
        UninstallStatus::NotInstalled
    );
    assert!(report.saved_report.is_none());
    assert!(!mac.resource.exists(), "the lock's folders went again");
    assert!(mac.resource.parent().unwrap().is_dir());
    assert!(objects.trashed().is_empty());
}
