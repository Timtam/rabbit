//! The shipped manifests' uninstall plans, on temporary resource folders:
//! what each package RABBIT can uninstall would remove and keep, the
//! warnings it shows, and the copies it refuses. Planning never removes
//! anything, and every folder here is a fake one; the CLAP folder too.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::tests::{Fixture, FoundAt, detection, installed, write};
use super::{
    KeptReason, NothingListed, PlannedRegistryCleanup, Removability, RemovalReason,
    UNINSTALL_NOT_YET_KEY, UninstallNote, UninstallPlan, UninstallPlanInput, UninstallPlanItem,
    UninstallWarning, VendorLookup, VendorRefusal, build_uninstall_plan,
};
use crate::detection::detect_component_with_probes;
use crate::model::{ComponentDetection, InstallationKind, Platform};
use crate::package::uninstall::{UninstallPostStep, UninstallStrategy, VendorUninstallerKind};
use crate::package::{
    PACKAGE_APP2CLAP, PACKAGE_FFMPEG, PACKAGE_JAWS_SCRIPTS, PACKAGE_OSARA, PACKAGE_REAKONTROL,
    PACKAGE_REAPACK, PACKAGE_REAPER, PACKAGE_SURGE_XT, PACKAGE_SWS, PackageSpec,
    builtin_package_specs, embedded_package_manifest, validate_package_spec,
};
use crate::reapack::write_test_registry;
use crate::receipt::{InstallState, InstalledFileReceipt, PackageReceipt};

const LANGPACK_DE: &str = "langpack-de";
const LANGPACK_ES: &str = "langpack-es";
const LANGPACK_FR: &str = "langpack-fr";
const SWS_IN_REAPACK: &str = "SWS/S&M extension";

/// The shipped packages for one platform, planned on a fixture.
struct Shipped<'f> {
    fixture: &'f Fixture,
    platform: Platform,
    specs: Vec<PackageSpec>,
}

impl<'f> Shipped<'f> {
    fn new(fixture: &'f Fixture, platform: Platform) -> Self {
        Self {
            fixture,
            platform,
            specs: builtin_package_specs(platform),
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
            roots: &self.fixture.roots,
            platform: self.platform,
            target,
            specs: &self.specs,
            detections,
            state,
            remove_settings,
            vendor_lookup,
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
                detect_component_with_probes(
                    &self.fixture.resource,
                    self.platform,
                    spec,
                    state,
                    |_| None,
                )
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
    let macos = [
        (PACKAGE_REAPER, not_supported),
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
    // On macOS, Surge XT and REAPER wait for a later version.
    let macos = Shipped::new(&fixture, Platform::MacOs);
    let everything: Vec<ComponentDetection> = macos
        .specs
        .iter()
        .map(|spec| detection(&spec.id, true))
        .collect();
    let macos_plan = macos.plan(&everything, None);
    for id in [PACKAGE_REAPER, PACKAGE_SURGE_XT] {
        assert_eq!(
            item(&macos_plan, id).removability,
            Removability::NotSupported {
                note_key: UNINSTALL_NOT_YET_KEY.to_string()
            },
            "{id}"
        );
    }
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
    assert!(surge.warnings.contains(&UninstallWarning::MachineWide));
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

    // Its extensions stay, and the user is told so (decision 16).
    assert!(reaper.warnings.contains(&UninstallWarning::Always {
        message_key: "uninstall-warning-reaper-extensions-stay".to_string(),
        confirm: false
    }));
    assert!(reaper.warnings.contains(&UninstallWarning::DependentsStay {
        package_ids: vec![PACKAGE_OSARA.to_string(), PACKAGE_SWS.to_string()]
    }));

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
