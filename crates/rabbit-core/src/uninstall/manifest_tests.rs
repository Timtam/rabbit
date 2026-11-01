//! The shipped manifests' uninstall plans, on temporary resource folders:
//! what each package RABBIT can uninstall would remove and keep, the
//! warnings it shows, and the copies it refuses. Planning never removes
//! anything, and every folder here is a fake one; the CLAP folder too.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::tests::{Fixture, detection, installed, write};
use super::{
    KeptReason, Removability, RemovalReason, UninstallNote, UninstallPlan, UninstallPlanInput,
    UninstallPlanItem, UninstallWarning, build_uninstall_plan,
};
use crate::detection::detect_component_with_probes;
use crate::model::{ComponentDetection, InstallationKind, Platform};
use crate::package::uninstall::{UninstallPostStep, UninstallStrategy};
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
        build_uninstall_plan(&UninstallPlanInput {
            roots: &self.fixture.roots,
            platform: self.platform,
            target,
            specs: &self.specs,
            detections,
            state,
            remove_settings,
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
    let manual = Some(UninstallStrategy::Manual);
    let not_supported = Some(UninstallStrategy::NotSupported);
    let cases: &[(Platform, InstallationKind, Expected<'_>)] = &[
        (
            Platform::Windows,
            InstallationKind::Standard,
            &[
                (PACKAGE_REAPER, not_supported),
                (PACKAGE_OSARA, not_supported),
                (PACKAGE_SWS, remove_files),
                (PACKAGE_REAPACK, remove_files),
                (PACKAGE_REAKONTROL, remove_files),
                (PACKAGE_JAWS_SCRIPTS, manual),
                (PACKAGE_FFMPEG, remove_files),
                (PACKAGE_SURGE_XT, not_supported),
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
            // computer, so a portable target has no plan for them at all.
            &[
                (PACKAGE_REAPER, manual),
                (PACKAGE_OSARA, not_supported),
                (PACKAGE_SWS, remove_files),
                (PACKAGE_REAPACK, remove_files),
                (PACKAGE_REAKONTROL, remove_files),
                (PACKAGE_JAWS_SCRIPTS, None),
                (PACKAGE_FFMPEG, remove_files),
                (PACKAGE_SURGE_XT, None),
                (PACKAGE_APP2CLAP, None),
                (LANGPACK_ES, remove_files),
                (LANGPACK_DE, remove_files),
                (LANGPACK_FR, remove_files),
            ],
        ),
    ];
    let macos = |surge_xt| {
        [
            (PACKAGE_REAPER, not_supported),
            (PACKAGE_OSARA, not_supported),
            (PACKAGE_SWS, remove_files),
            (PACKAGE_REAPACK, remove_files),
            (PACKAGE_REAKONTROL, remove_files),
            (PACKAGE_SURGE_XT, surge_xt),
            (LANGPACK_ES, remove_files),
            (LANGPACK_DE, remove_files),
            (LANGPACK_FR, remove_files),
        ]
    };
    let macos_standard = macos(not_supported);
    let macos_portable = macos(None);
    let cases = cases.iter().copied().chain([
        (
            Platform::MacOs,
            InstallationKind::Standard,
            &macos_standard[..],
        ),
        (
            Platform::MacOs,
            InstallationKind::Portable,
            &macos_portable[..],
        ),
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
