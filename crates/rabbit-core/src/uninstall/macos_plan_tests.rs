//! macOS plans on every platform: how REAPER.app and an administrator
//! removal like Surge XT's are planned, in which order a run takes them, and
//! what the read before a run makes of a whole app bundle. A temporary
//! folder stands in for `/`, and every fact macOS would give is handed in,
//! so nothing here reads or changes a real Mac's apps, Trash or system
//! folders.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{TempDir, tempdir};

use super::execute::{UninstallEnvironment, execute_uninstall_in};
use super::preflight::{VolumeProbe, backup_space_check, moves_resource_files};
use super::tests::{installed, write};
use super::{
    ByHandReason, FakeMacosObjects, KeptReason, MacosObjects, NoAdministratorShell,
    NoElevatedHelper, NothingListed, ObjectFacts, ObjectProbe, PathRefusal, Removability,
    RemovalReason, RemovalRoute, RemovedFile, TrashFolderFacts, UninstallPlan, UninstallPlanInput,
    UninstallPlanItem, UninstallReport, UninstallRequest, UninstallRoots, UninstallStatus,
    build_uninstall_plan,
};
use crate::cancel::CancelToken;
use crate::model::{InstallationKind, Platform};
use crate::package::uninstall::UninstallPostStep;
use crate::package::{
    PACKAGE_REAPER, PackageSpec, parse_embedded_package_spec, validate_package_spec,
};
use crate::progress::ProgressReporter;

const SUMMARY: &str = r#""summary": { "removes_key": "test-removes", "keeps_key": "test-keeps" }"#;
pub(super) const REAPER_ID: &str = "com.cockos.reaper";
pub(super) const ME: u32 = 501;
pub(super) const DISK: u64 = 16_777_220;

/// A macOS package `id` with `extra` among its fields and one plan holding
/// `plan`. It must pass the manifest's own validation, like every shipped
/// manifest.
pub(super) fn mac_spec(id: &str, extra: &str, plan: &str) -> PackageSpec {
    let source = format!(
        r#"{{
            "id": "{id}",
            "display_name": "{id}",
            "display_name_key": "package-{id}",
            "display_description_key": "package-{id}-description",
            "recommended": false,
            "supported_platforms": ["macos"],
            "user_plugin_prefixes": ["reaper_{id}"],
            "user_plugin_suffixes": {{ "windows": [".dll"], "macos": [".dylib"] }},
            {extra}
            "uninstall": {{ "plans": [{{ "platform": "macos", "strategy": "remove_files", {SUMMARY}, {plan} }}] }}
        }}"#
    );
    let spec = parse_embedded_package_spec(&source)
        .unwrap_or_else(|error| panic!("test manifest should parse: {error}\n{source}"));
    if let Err(message) = validate_package_spec(&spec) {
        panic!("test manifest should validate: {message}\n{source}");
    }
    spec.to_package_spec(Platform::MacOs)
}

/// REAPER's app as the shipped plan will have it, with `elevation`.
pub(super) fn reaper_spec(elevation: &str) -> PackageSpec {
    mac_spec(
        PACKAGE_REAPER,
        "",
        &format!(
            r#""elevation": "{elevation}", "backup": "to_trash",
               "remove": {{ "trees": [{{ "path": "{{app_path}}", "must_end_with": ".app",
                   "name_contains": "REAPER", "bundle_id": "{REAPER_ID}" }}] }},
               "kept_user_data": [{{ "path": "{{resource}}", "note_key": "test-kept" }}]"#
        ),
    )
}

/// A package the administrator command deletes, shaped like Surge XT's
/// macOS plan.
pub(super) fn surge() -> PackageSpec {
    mac_spec(
        "surge",
        r#""requires_standard_install": true,"#,
        r#""elevation": "required", "backup": "none",
           "blocking_programs": ["Surge"],
           "remove": { "trees": [
               { "path": "/Applications/Surge.app", "must_end_with": ".app", "name_contains": "Surge" },
               { "path": "/Library/Audio/Plug-Ins/VST3/Surge.vst3", "must_end_with": ".vst3", "name_contains": "Surge" },
               { "path": "/Library/Application Support/Surge", "name_contains": "Surge" }
           ] },
           "kept_user_data": [{ "path": "{documents}/Surge", "note_key": "test-kept" }],
           "macos_pkg_forget": ["org.example.surge.vst3.pkg", "org.example.surge.app.pkg"],
           "post": [{ "refresh_audio_units": {} }]"#,
    )
}

/// An extension whose plug-in goes into the backup set.
pub(super) fn extension() -> PackageSpec {
    mac_spec(
        "ext",
        r#""depends_on": ["reaper"],"#,
        r#""remove": { "paths": ["{resource}/UserPlugins/reaper_ext.dylib"] }"#,
    )
}

/// A temporary folder holding a resource folder, a stand-in for `/` with
/// REAPER.app in its Applications folder, and roots that know them all.
pub(super) struct MacFixture {
    pub(super) dir: TempDir,
    pub(super) resource: PathBuf,
    pub(super) system: PathBuf,
    pub(super) app: PathBuf,
    pub(super) roots: UninstallRoots,
}

impl MacFixture {
    pub(super) fn new() -> Self {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let system = dir.path().join("system");
        for folder in [
            "Applications",
            "Library/Application Support",
            "Library/Audio/Plug-Ins/VST3",
        ] {
            fs::create_dir_all(join(&system, folder)).unwrap();
        }
        let app = system.join("Applications").join("REAPER.app");
        write(&join(&app, "Contents/MacOS/REAPER"), b"REAPER");
        let mut roots = UninstallRoots::new(&resource);
        roots.system_root = system.clone();
        roots.app_path = Some(app.clone());
        roots.app_dir = Some(system.join("Applications"));
        roots.home = Some(dir.path().join("home"));
        roots.documents = Some(dir.path().join("Documents"));
        Self {
            dir,
            resource,
            system,
            app,
            roots,
        }
    }

    /// Surge's three trees, as real folders.
    pub(super) fn install_surge(&self) {
        for tree in [
            "Applications/Surge.app/Contents/MacOS/Surge",
            "Library/Audio/Plug-Ins/VST3/Surge.vst3/Contents/Info.plist",
            "Library/Application Support/Surge/presets.txt",
        ] {
            write(&join(&self.system, tree), b"surge");
        }
    }

    pub(super) fn plan(
        &self,
        specs: &[PackageSpec],
        ids: &[&str],
        objects: &dyn ObjectProbe,
        remove_settings: bool,
    ) -> UninstallPlan {
        build_uninstall_plan(&UninstallPlanInput {
            roots: &self.roots,
            platform: Platform::MacOs,
            target: InstallationKind::Standard,
            specs,
            detections: &installed(ids),
            state: None,
            remove_settings,
            vendor_lookup: &NothingListed,
            objects,
        })
    }

    /// The facts macOS would give for REAPER.app, and a Trash of the user's
    /// on the same volume.
    pub(super) fn objects(&self, app: ObjectFacts) -> FakeMacosObjects {
        FakeMacosObjects::new()
            .with_object(&self.app, app)
            .with_trash(my_trash())
    }
}

/// `relative`, with `/` between its components, below `base`.
pub(super) fn join(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

/// REAPER.app as a user who dragged it into /Applications owns it.
pub(super) fn owned_by_me() -> ObjectFacts {
    ObjectFacts {
        is_dir: true,
        is_symlink: false,
        owner_uid: ME,
        owned_by_me: true,
        writable: true,
        parent_writable_searchable: true,
        removable_by_me: true,
        parent_sticky: false,
        parent_owned_by_me: false,
        locked: false,
        parent_locked: false,
        dev: DISK,
        bundle_identifier: Some(REAPER_ID.to_string()),
    }
}

/// REAPER.app as an installer running as root left it.
pub(super) fn owned_by_root() -> ObjectFacts {
    ObjectFacts {
        owner_uid: 0,
        owned_by_me: false,
        writable: false,
        removable_by_me: false,
        ..owned_by_me()
    }
}

pub(super) fn my_trash() -> TrashFolderFacts {
    TrashFolderFacts {
        path: PathBuf::from("/Users/me/.Trash"),
        is_dir: true,
        is_symlink: false,
        owner_uid: ME,
        owned_by_me: true,
        dev: DISK,
    }
}

fn item<'a>(plan: &'a UninstallPlan, id: &str) -> &'a UninstallPlanItem {
    plan.item(id)
        .unwrap_or_else(|| panic!("the plan has no item for {id}"))
}

/// Make `link` point to the folder `target`: a junction on Windows, a
/// symbolic link elsewhere. The guard sees both as links.
pub(super) fn link_folder(link: &Path, target: &Path) {
    #[cfg(windows)]
    super::paths::make_junction(link, target);
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[test]
fn an_app_the_user_may_move_goes_to_the_trash_without_a_prompt() {
    let fixture = MacFixture::new();
    let plan = fixture.plan(
        &[reaper_spec("if_needed")],
        &[PACKAGE_REAPER],
        &fixture.objects(owned_by_me()),
        false,
    );
    let reaper = item(&plan, PACKAGE_REAPER);

    assert_eq!(reaper.removability, Removability::Removable);
    assert_eq!(reaper.route(), RemovalRoute::Trash);
    assert!(reaper.can_remove());
    assert!(!reaper.requires_elevation && !reaper.needs_administrator());
    // Only the target's REAPER need be closed: no pgrep looks for others.
    assert!(!reaper.every_reaper_closed);
    // Gone from its place all the same, so the question before starting.
    assert!(reaper.unbacked && reaper.confirm);
    let [app] = reaper.files.as_slice() else {
        panic!("one tree: {:?}", reaper.files);
    };
    assert_eq!(app.path, fixture.app);
    assert_eq!(app.reason, RemovalReason::Tree);
    assert_eq!(app.bundle_id.as_deref(), Some(REAPER_ID));
    assert!(app.is_dir);
}

#[test]
fn a_root_owned_app_in_applications_goes_through_the_administrator_command() {
    let fixture = MacFixture::new();
    let plan = fixture.plan(
        &[reaper_spec("if_needed")],
        &[PACKAGE_REAPER],
        &fixture.objects(owned_by_root()),
        false,
    );
    let reaper = item(&plan, PACKAGE_REAPER);

    assert_eq!(reaper.removability, Removability::Removable);
    assert!(reaper.requires_elevation);
    assert_eq!(reaper.route(), RemovalRoute::Administrator);
    assert!(reaper.needs_administrator() && reaper.can_remove());
    assert_eq!(reaper.files[0].path, fixture.app);
    // Root's pgrep finds every REAPER by name, so RABBIT's own checks ask
    // for every REAPER closed too.
    assert!(reaper.every_reaper_closed);
}

#[test]
fn a_root_owned_app_is_left_to_the_user_unless_the_plan_allows_root() {
    let fixture = MacFixture::new();
    let plan = fixture.plan(
        &[reaper_spec("never")],
        &[PACKAGE_REAPER],
        &fixture.objects(owned_by_root()),
        false,
    );
    let reaper = item(&plan, PACKAGE_REAPER);

    assert_eq!(
        reaper.removability,
        Removability::ByHand {
            path: fixture.app.clone(),
            reason: ByHandReason::NotMovableByYou,
        }
    );
    assert!(reaper.removability.is_blocked() && !reaper.can_remove());
    assert!(!reaper.requires_elevation);
    assert!(reaper.files.is_empty());
    // What stays is still said.
    assert!(
        reaper
            .kept
            .iter()
            .any(|kept| kept.path == fixture.resource && kept.reason == KeptReason::FolderRoot)
    );
}

/// Whether a reason is the one a case expects.
type IsExpected = fn(&ByHandReason) -> bool;

/// Every reason RABBIT leaves REAPER.app for the user to move in Finder.
#[test]
fn an_app_rabbit_cant_trust_or_check_is_left_to_the_user() {
    let fixture = MacFixture::new();
    let app = fixture.app.clone();
    let cases: Vec<(FakeMacosObjects, IsExpected)> = vec![
        (
            fixture.objects(ObjectFacts {
                locked: true,
                ..owned_by_me()
            }),
            |reason| *reason == ByHandReason::Locked,
        ),
        (
            fixture.objects(ObjectFacts {
                bundle_identifier: Some("com.example.other".to_string()),
                ..owned_by_root()
            }),
            |reason| *reason == ByHandReason::NotTheExpectedApp,
        ),
        (
            FakeMacosObjects::new()
                .with_unreadable_object(&app, std::io::ErrorKind::PermissionDenied)
                .with_trash(my_trash()),
            |reason| matches!(reason, ByHandReason::CouldNotCheck { .. }),
        ),
        (
            FakeMacosObjects::new().with_object(&app, owned_by_root()),
            |reason| *reason == ByHandReason::NoTrashFolder,
        ),
        (
            FakeMacosObjects::new()
                .with_object(&app, owned_by_root())
                .with_trash(TrashFolderFacts {
                    is_dir: false,
                    is_symlink: true,
                    ..my_trash()
                }),
            |reason| *reason == ByHandReason::NoTrashFolder,
        ),
        (
            FakeMacosObjects::new()
                .with_object(&app, owned_by_root())
                .with_trash(TrashFolderFacts {
                    dev: DISK + 1,
                    ..my_trash()
                }),
            |reason| *reason == ByHandReason::NotOnTrashVolume,
        ),
    ];
    for (objects, expected) in cases {
        let plan = fixture.plan(
            &[reaper_spec("if_needed")],
            &[PACKAGE_REAPER],
            &objects,
            false,
        );
        let reaper = item(&plan, PACKAGE_REAPER);
        match &reaper.removability {
            Removability::ByHand { path, reason } => {
                assert_eq!(path, &app);
                assert!(expected(reason), "{reason:?}");
            }
            other => panic!("expected the app to be left to the user, got {other:?}"),
        }
        assert!(!reaper.requires_elevation && !reaper.can_remove());
    }
}

/// Root's move is checked like root's deletes: /Applications reached
/// through a link could send it anywhere.
#[test]
fn root_never_moves_reaper_out_of_a_linked_applications_folder() {
    let fixture = MacFixture::new();
    let applications = fixture.system.join("Applications");
    let real = fixture.dir.path().join("real Applications");
    write(&join(&real, "REAPER.app/Contents/MacOS/REAPER"), b"REAPER");
    fs::remove_dir_all(&applications).unwrap();
    link_folder(&applications, &real);

    let plan = fixture.plan(
        &[reaper_spec("if_needed")],
        &[PACKAGE_REAPER],
        &fixture.objects(owned_by_root()),
        false,
    );
    let reaper = item(&plan, PACKAGE_REAPER);
    assert_eq!(
        reaper.removability,
        Removability::UnsafePath {
            path: fixture.app.clone(),
            refusal: PathRefusal::LinkedAncestor,
        }
    );
    assert!(!reaper.requires_elevation);
    // An app the user may move needs no such check: it moves as the user.
    let plan = fixture.plan(
        &[reaper_spec("if_needed")],
        &[PACKAGE_REAPER],
        &fixture.objects(owned_by_me()),
        false,
    );
    assert_eq!(item(&plan, PACKAGE_REAPER).route(), RemovalRoute::Trash);
}

#[test]
fn what_root_deletes_passes_the_administrator_guard_while_planning() {
    let fixture = MacFixture::new();
    fixture.install_surge();
    let user_data = fixture.dir.path().join("Documents").join("Surge");
    write(&user_data.join("patch.fxp"), b"mine");

    // Even with "Also remove settings": root never takes the user's data,
    // and nothing of this package goes into a backup set.
    let plan = fixture.plan(&[surge()], &["surge"], &FakeMacosObjects::new(), true);
    let surge = item(&plan, "surge");

    assert_eq!(surge.removability, Removability::Removable);
    assert_eq!(surge.route(), RemovalRoute::Administrator);
    assert!(surge.needs_administrator() && surge.can_remove());
    assert!(surge.requires_elevation && surge.unbacked && surge.confirm);
    assert!(surge.every_reaper_closed);
    assert_eq!(surge.blocking_programs, ["Surge"]);
    assert_eq!(
        surge.macos_pkg_forget,
        ["org.example.surge.vst3.pkg", "org.example.surge.app.pkg"]
    );
    assert_eq!(surge.post, [UninstallPostStep::RefreshAudioUnits {}]);
    assert_eq!(
        surge
            .files
            .iter()
            .map(|file| (file.path.clone(), file.reason))
            .collect::<Vec<_>>(),
        [
            "Applications/Surge.app",
            "Library/Audio/Plug-Ins/VST3/Surge.vst3",
            "Library/Application Support/Surge",
        ]
        .map(|tree| (join(&fixture.system, tree), RemovalReason::Tree))
    );
    let kept = surge
        .kept
        .iter()
        .find(|kept| kept.path == user_data)
        .expect("the user's data is listed as kept");
    assert_eq!(kept.reason, KeptReason::UserData);
    assert!(kept.exists && !kept.removable_with_settings);
}

#[test]
fn a_linked_plug_in_folder_keeps_root_away_from_the_package() {
    let fixture = MacFixture::new();
    fixture.install_surge();
    let vst3 = join(&fixture.system, "Library/Audio/Plug-Ins/VST3");
    let elsewhere = fixture.dir.path().join("external VST3");
    write(
        &join(&elsewhere, "Surge.vst3/Contents/Info.plist"),
        b"surge",
    );
    fs::remove_dir_all(&vst3).unwrap();
    link_folder(&vst3, &elsewhere);
    let user_data = fixture.dir.path().join("Documents").join("Surge");
    write(&user_data.join("patch.fxp"), b"mine");

    let plan = fixture.plan(&[surge()], &["surge"], &FakeMacosObjects::new(), false);
    let surge = item(&plan, "surge");
    assert_eq!(
        surge.removability,
        Removability::UnsafePath {
            path: vst3.join("Surge.vst3"),
            refusal: PathRefusal::LinkedAncestor,
        }
    );
    // Blocked or not, root's route never takes the user's data, so the
    // details don't offer "Also remove settings" for it.
    assert!(surge.kept.iter().any(|kept| kept.path == user_data));
    assert!(
        surge.kept.iter().all(|kept| !kept.removable_with_settings),
        "{:?}",
        surge.kept
    );
}

#[test]
fn the_administrator_command_runs_first_and_reaper_last_among_the_rest() {
    let fixture = MacFixture::new();
    fixture.install_surge();
    write(
        &join(&fixture.resource, "UserPlugins/reaper_ext.dylib"),
        b"ext",
    );
    let specs = [reaper_spec("if_needed"), extension(), surge()];
    let ids: Vec<String> = [PACKAGE_REAPER, "ext", "surge"].map(String::from).to_vec();
    let order = |plan: &UninstallPlan| -> Vec<String> {
        plan.in_run_order(&ids)
            .iter()
            .map(|item| item.package_id.clone())
            .collect()
    };

    let plan = fixture.plan(
        &specs,
        &[PACKAGE_REAPER, "ext", "surge"],
        &fixture.objects(owned_by_me()),
        false,
    );
    assert_eq!(order(&plan), ["surge", "ext", PACKAGE_REAPER]);

    // Once only root can move REAPER's app, REAPER joins the administrator
    // command: after the other packages there, before everything else.
    let plan = fixture.plan(
        &specs,
        &[PACKAGE_REAPER, "ext", "surge"],
        &fixture.objects(owned_by_root()),
        false,
    );
    assert_eq!(order(&plan), ["surge", PACKAGE_REAPER, "ext"]);
}

#[test]
fn without_administrator_packages_the_run_order_is_the_removal_order() {
    let fixture = MacFixture::new();
    write(
        &join(&fixture.resource, "UserPlugins/reaper_ext.dylib"),
        b"ext",
    );
    let plan = fixture.plan(
        &[reaper_spec("never"), extension()],
        &[PACKAGE_REAPER, "ext"],
        &fixture.objects(owned_by_me()),
        false,
    );
    let ids = [PACKAGE_REAPER, "ext"].map(String::from).to_vec();
    let ids_of = |items: Vec<&UninstallPlanItem>| -> Vec<String> {
        items.iter().map(|item| item.package_id.clone()).collect()
    };
    assert_eq!(
        ids_of(plan.in_run_order(&ids)),
        ids_of(plan.in_removal_order(&ids))
    );
    assert_eq!(ids_of(plan.in_run_order(&ids)), ["ext", PACKAGE_REAPER]);
}

/// Volumes as the space check sees them: the backup always on another
/// volume, with no room at all.
struct NoRoomElsewhere;

impl VolumeProbe for NoRoomElsewhere {
    fn same_volume(&self, _left: &Path, _right: &Path) -> Option<bool> {
        Some(false)
    }

    fn available_space(&self, _path: &Path) -> Option<u64> {
        Some(0)
    }
}

/// Volumes as a run sees them: one volume for everything, with room.
pub(super) struct OneVolume;

impl VolumeProbe for OneVolume {
    fn same_volume(&self, _left: &Path, _right: &Path) -> Option<bool> {
        Some(true)
    }

    fn available_space(&self, _path: &Path) -> Option<u64> {
        Some(u64::MAX)
    }
}

/// What goes to the Trash or through the administrator command never goes
/// into the backup set: it needs no room there, and a portable REAPER's app
/// in its resource folder doesn't call for the UserPlugins rehearsal.
#[test]
fn only_backup_set_files_count_for_the_space_check_and_the_write_rehearsal() {
    let mut fixture = MacFixture::new();
    fixture.install_surge();
    let portable_app = fixture.resource.join("REAPER.app");
    write(&join(&portable_app, "Contents/MacOS/REAPER"), b"REAPER");
    fixture.roots.app_path = Some(portable_app.clone());
    fixture.roots.app_dir = Some(fixture.resource.clone());
    fixture.app = portable_app;

    let ids = [PACKAGE_REAPER, "surge"].map(String::from).to_vec();
    let plan = fixture.plan(
        &[reaper_spec("never"), surge()],
        &[PACKAGE_REAPER, "surge"],
        &fixture.objects(owned_by_me()),
        false,
    );
    assert_eq!(item(&plan, PACKAGE_REAPER).route(), RemovalRoute::Trash);
    assert!(
        item(&plan, PACKAGE_REAPER).files[0]
            .path
            .starts_with(&fixture.resource)
    );
    assert_eq!(item(&plan, "surge").route(), RemovalRoute::Administrator);
    assert!(backup_space_check(&plan, &ids, &NoRoomElsewhere).is_none());
    assert!(!moves_resource_files(
        &plan,
        &plan.items.iter().collect::<Vec<_>>()
    ));

    // An extension's plug-in does go into the backup set.
    write(
        &join(&fixture.resource, "UserPlugins/reaper_ext.dylib"),
        b"ext",
    );
    let ids = [PACKAGE_REAPER, "surge", "ext"].map(String::from).to_vec();
    let plan = fixture.plan(
        &[reaper_spec("never"), surge(), extension()],
        &[PACKAGE_REAPER, "surge", "ext"],
        &fixture.objects(owned_by_me()),
        false,
    );
    let (_, packages) =
        backup_space_check(&plan, &ids, &NoRoomElsewhere).expect("the plug-in needs room");
    assert_eq!(packages, ["ext"]);
    assert!(moves_resource_files(
        &plan,
        &plan.items.iter().collect::<Vec<_>>()
    ));
}

/// The checks count file by file, as the run moves them: a Trash-route
/// package's app goes to the Trash whole, but its settings, with "Also
/// remove settings", go into the backup set like any other package's.
#[test]
fn a_trash_route_packages_settings_still_count_for_the_backup_checks() {
    let mut fixture = MacFixture::new();
    let portable_app = fixture.resource.join("REAPER.app");
    write(&join(&portable_app, "Contents/MacOS/REAPER"), b"REAPER");
    fixture.roots.app_path = Some(portable_app.clone());
    fixture.roots.app_dir = Some(fixture.resource.clone());
    fixture.app = portable_app;
    let settings = fixture.resource.join("Demo");
    write(&settings.join("settings.ini"), b"mine");
    let demo = mac_spec(
        "demo",
        "",
        r#""elevation": "never", "backup": "to_trash",
           "remove": { "trees": [{ "path": "{app_path}", "must_end_with": ".app", "name_contains": "REAPER" }] },
           "kept_user_data": [{ "path": "{resource}/Demo", "note_key": "test-kept" }]"#,
    );
    let ids = ["demo".to_string()];
    let check = |remove_settings: bool| {
        let plan = fixture.plan(
            std::slice::from_ref(&demo),
            &["demo"],
            &fixture.objects(owned_by_me()),
            remove_settings,
        );
        let planned = item(&plan, "demo");
        assert_eq!(planned.route(), RemovalRoute::Trash);
        assert_eq!(planned.files[0].path, fixture.app);
        (
            backup_space_check(&plan, &ids, &NoRoomElsewhere).map(|(_, packages)| packages),
            moves_resource_files(&plan, &plan.items.iter().collect::<Vec<_>>()),
        )
    };

    // Only the app, which needs no room and no rehearsal.
    assert_eq!(check(false), (None, false));
    // The settings go into the backup set.
    assert_eq!(check(true), (Some(vec!["demo".to_string()]), true));
}

/// Every file and folder below `root`, with each file's contents.
pub(super) fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut found = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            found.insert(path.clone(), None);
            found.extend(snapshot(&path));
        } else if metadata.is_file() {
            found.insert(path.clone(), Some(fs::read(&path).unwrap()));
        } else {
            found.insert(path.clone(), None);
        }
    }
    found
}

/// One engine run of `ids` on the fixture, with everything it asks the
/// computer stood in.
fn run(
    fixture: &MacFixture,
    specs: &[PackageSpec],
    ids: &[&str],
    objects: &dyn MacosObjects,
    dry_run: bool,
) -> UninstallReport {
    let detections = Cell::new(0);
    let detect = || {
        let call = detections.get();
        detections.set(call + 1);
        Ok(if call == 0 {
            installed(ids)
        } else {
            installed(&[])
        })
    };
    let reapers = Vec::<crate::preflight::RunningProcess>::new;
    let named = |_: &[String]| Vec::new();
    execute_uninstall_in(
        &UninstallRequest {
            resource_path: fixture.resource.clone(),
            platform: Platform::MacOs,
            package_ids: ids.iter().map(ToString::to_string).collect(),
            remove_settings: false,
            dry_run,
            target_app_path: Some(fixture.app.clone()),
            target: Some(InstallationKind::Standard),
        },
        &UninstallEnvironment {
            specs,
            roots: &fixture.roots,
            target: InstallationKind::Standard,
            target_app_path: Some(fixture.app.clone()),
            detect: &detect,
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
    .unwrap()
}

/// An app bundle holds thousands of files and often links. Read file by
/// file, one link would keep the whole app, and every file would be hashed
/// for a backup set it never goes into. The dry run checks the guard and
/// that the app is there, and that is all.
#[test]
fn a_dry_run_reads_an_app_bundle_whole_and_never_walks_it() {
    let fixture = MacFixture::new();
    let elsewhere = fixture.dir.path().join("Frameworks elsewhere");
    write(&elsewhere.join("library.dylib"), b"shared");
    link_folder(&join(&fixture.app, "Contents/Frameworks"), &elsewhere);

    let report = run(
        &fixture,
        &[reaper_spec("if_needed")],
        &[PACKAGE_REAPER],
        &fixture.objects(owned_by_me()),
        true,
    );
    let item = report.item(PACKAGE_REAPER).unwrap();

    assert_eq!(item.status, UninstallStatus::WouldRemove);
    assert_eq!(
        item.removed,
        [RemovedFile {
            path: fixture.app.clone(),
            reason: RemovalReason::Tree,
            size: 0,
            changed_since_install: false,
            backup_path: None,
            trashed_to: None,
        }]
    );
    assert!(item.kept.is_empty(), "{:?}", item.kept);
    assert!(
        fixture
            .app
            .join("Contents")
            .join("MacOS")
            .join("REAPER")
            .is_file()
    );
}

#[test]
fn the_portable_reaper_app_is_chosen_the_same_way_every_time() {
    let dir = tempdir().unwrap();
    let folder = dir.path();
    for name in [
        "REAPER64.app",
        "REAPER-ARM.app",
        "My REAPER.app",
        "REAPER.app",
        "Notes.app",
    ] {
        fs::create_dir_all(folder.join(name)).unwrap();
    }
    write(&folder.join("reaper.ini"), b"[REAPER]");
    let chosen = || {
        let detected = crate::detection::portable_macos_app_bundle(folder);
        assert_eq!(
            crate::preflight::portable_target_app_path(folder, Some(Platform::MacOs)),
            detected,
            "the checks and detection choose the same app"
        );
        detected
    };

    assert_eq!(chosen(), Some(folder.join("REAPER.app")));
    fs::remove_dir(folder.join("REAPER.app")).unwrap();
    assert_eq!(chosen(), Some(folder.join("REAPER64.app")));
    fs::remove_dir(folder.join("REAPER64.app")).unwrap();
    assert_eq!(chosen(), Some(folder.join("REAPER-ARM.app")));
    fs::remove_dir(folder.join("REAPER-ARM.app")).unwrap();
    assert_eq!(chosen(), Some(folder.join("My REAPER.app")));
    fs::remove_dir(folder.join("My REAPER.app")).unwrap();
    assert_eq!(chosen(), None);
}

#[test]
fn a_by_hand_app_says_how_to_move_it_in_finder() {
    let path = PathBuf::from("/Applications/REAPER.app");
    let not_movable = Removability::ByHand {
        path: path.clone(),
        reason: ByHandReason::NotMovableByYou,
    };
    assert!(not_movable.is_blocked());
    assert_eq!(
        not_movable.message("REAPER"),
        "RABBIT can't move /Applications/REAPER.app to the Trash, because your account may not \
         move it: it belongs to another account or to macOS, or sits in a folder you can't \
         change. To remove REAPER, select it in Finder and press Command-Delete; Finder asks for \
         an administrator's password if it needs one."
    );
    let locked = Removability::ByHand {
        path,
        reason: ByHandReason::Locked,
    };
    assert!(
        locked
            .message("REAPER")
            .contains("because it is locked. To remove REAPER, untick Locked in its Get Info window, then select it in Finder"),
        "{}",
        locked.message("REAPER")
    );
    // Get Info's Locked clears only the lock a user sets.
    assert!(
        locked.message("REAPER").ends_with(
            "If Locked isn't ticked, or Finder still can't move it, it has a lock that Finder \
             can't remove."
        ),
        "{}",
        locked.message("REAPER")
    );
    // The report and `--json` name the route and the reason.
    let json = serde_json::to_value(&locked).unwrap();
    assert_eq!(json["kind"], "by-hand");
    assert_eq!(json["reason"]["kind"], "locked");
    assert_eq!(
        serde_json::from_value::<Removability>(json).unwrap(),
        locked
    );
}
