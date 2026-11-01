//! The planner's rules and the uninstall preflight, on temporary resource
//! folders with receipts and manifests built in each test. Nothing here
//! removes anything: planning never does.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{TempDir, tempdir};

use super::{
    ANY_REAPER_PROCESS_CHECK, KeptReason, PROGRAMS_RUNNING_CHECK, PathRefusal, Removability,
    RemovalReason, UNINSTALL_NOT_YET_KEY, UninstallNote, UninstallPlan, UninstallPlanInput,
    UninstallPlanItem, UninstallPreflightOptions, UninstallRoots, UninstallWarning,
    build_uninstall_plan, run_uninstall_preflight_with_processes,
};
use crate::model::{ComponentDetection, Confidence, InstallationKind, Platform};
use crate::package::uninstall::UninstallStrategy;
use crate::package::{
    PACKAGE_REAPACK, PACKAGE_REAPER, PackageSpec, builtin_package_specs,
    parse_embedded_package_spec, validate_package_spec,
};
use crate::preflight::{PreflightReport, PreflightStatus, REAPER_PROCESS_CHECK, RunningProcess};
use crate::reapack::write_test_registry;
use crate::receipt::{InstallState, InstalledFileReceipt, PackageReceipt};

const SUMMARY: &str = r#""summary": { "removes_key": "test-removes", "keeps_key": "test-keeps" }"#;

/// A temporary folder holding a resource folder `REAPER` and roots that
/// know it and a CLAP folder `CLAP` beside it.
pub(super) struct Fixture {
    pub(super) dir: TempDir,
    pub(super) resource: PathBuf,
    pub(super) roots: UninstallRoots,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let mut roots = UninstallRoots::new(&resource);
        roots.clap_dir = Some(dir.path().join("CLAP"));
        Self {
            dir,
            resource,
            roots,
        }
    }

    /// Write `relative`, with `/` between its components, inside the
    /// resource folder.
    pub(super) fn file(&self, relative: &str) -> PathBuf {
        let path = relative
            .split('/')
            .fold(self.resource.clone(), |path, component| {
                path.join(component)
            });
        write(&path, b"file contents")
    }

    fn plan(
        &self,
        specs: &[PackageSpec],
        detections: &[ComponentDetection],
        state: Option<&InstallState>,
    ) -> UninstallPlan {
        self.plan_with(specs, detections, state, InstallationKind::Standard, false)
    }

    fn plan_with(
        &self,
        specs: &[PackageSpec],
        detections: &[ComponentDetection],
        state: Option<&InstallState>,
        target: InstallationKind,
        remove_settings: bool,
    ) -> UninstallPlan {
        build_uninstall_plan(&UninstallPlanInput {
            roots: &self.roots,
            platform: Platform::Windows,
            target,
            specs,
            detections,
            state,
            remove_settings,
        })
    }
}

pub(super) fn write(path: &Path, contents: &[u8]) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
    path.to_path_buf()
}

/// A Windows package `id` whose plug-ins are `reaper_<id>*.dll` (or
/// `.clap`, for one that installs into the CLAP folder), with
/// `extra` spliced in among its fields. It must pass the manifest's own
/// validation, like every shipped manifest.
pub(super) fn spec(id: &str, extra: &str, uninstall: &str) -> PackageSpec {
    let source = format!(
        r#"{{
            "id": "{id}",
            "display_name": "{id}",
            "display_name_key": "package-{id}",
            "display_description_key": "package-{id}-description",
            "recommended": false,
            "supported_platforms": ["windows"],
            "user_plugin_prefixes": ["reaper_{id}"],
            "user_plugin_suffixes": {{ "windows": [".dll", ".clap"], "macos": [".dylib"] }},
            {extra}
            "uninstall": {uninstall}
        }}"#
    );
    let spec = parse_embedded_package_spec(&source)
        .unwrap_or_else(|error| panic!("test manifest should parse: {error}\n{source}"));
    if let Err(message) = validate_package_spec(&spec) {
        panic!("test manifest should validate: {message}\n{source}");
    }
    spec.to_package_spec(Platform::Windows)
}

/// An uninstall block with one Windows `remove_files` plan holding `fields`.
pub(super) fn remove_files(fields: &str) -> String {
    remove_files_with("", fields)
}

/// [`remove_files`] with `top` among the block's own fields.
pub(super) fn remove_files_with(top: &str, fields: &str) -> String {
    format!(
        r#"{{ {top} "plans": [{{ "platform": "windows", "strategy": "remove_files", {SUMMARY}, {fields} }}] }}"#
    )
}

pub(super) fn detection(id: &str, installed: bool) -> ComponentDetection {
    ComponentDetection {
        package_id: id.to_string(),
        display_name: id.to_string(),
        installed,
        version: None,
        detector: "test".to_string(),
        confidence: Confidence::High,
        files: Vec::new(),
        notes: Vec::new(),
    }
}

/// The detections for `installed` packages, plus ReaPack as not installed,
/// so ReaPack's records hold no sway unless a test says so.
pub(super) fn installed(ids: &[&str]) -> Vec<ComponentDetection> {
    let mut detections: Vec<ComponentDetection> =
        ids.iter().map(|id| detection(id, true)).collect();
    if !ids.contains(&PACKAGE_REAPACK) {
        detections.push(detection(PACKAGE_REAPACK, false));
    }
    detections
}

/// A receipt entry for `id` listing `files` as (path, sha256).
fn receipt(id: &str, files: &[(&str, Option<&str>)]) -> InstallState {
    let mut state = InstallState::default();
    state.packages.insert(
        id.to_string(),
        PackageReceipt {
            id: id.to_string(),
            version: None,
            source_url: None,
            source_sha256: None,
            variant: None,
            channel: None,
            installed_files: files
                .iter()
                .map(|(path, sha256)| InstalledFileReceipt {
                    path: PathBuf::from(path),
                    sha256: sha256.map(str::to_string),
                    size: Some(13),
                })
                .collect(),
            installed_at: None,
            rabbit_version: None,
            architecture: None,
            vendor_added_files: Vec::new(),
        },
    );
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

fn kept(item: &UninstallPlanItem, reason: KeptReason) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = item
        .kept
        .iter()
        .filter(|kept| kept.reason == reason)
        .map(|kept| kept.path.clone())
        .collect();
    paths.sort();
    paths
}

fn sorted(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    paths
}

#[test]
fn receipt_files_with_a_hash_are_candidates_and_the_rest_never_are() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo-x64.dll");
    let script = fixture.file("Scripts/demo.py");
    fixture.file("reaper-kb.ini");
    fs::create_dir_all(fixture.resource.join("demo")).unwrap();
    let state = receipt(
        "demo",
        &[
            ("UserPlugins/reaper_demo-x64.dll", Some("aa")),
            ("Scripts/demo.py", Some("bb")),
            // Recorded, but gone since: nothing to remove.
            ("Scripts/gone.py", Some("cc")),
            // Folders and user content are recorded without a hash.
            ("demo", None),
            ("reaper-kb.ini", None),
        ],
    );
    let specs = [spec(
        "demo",
        "",
        &remove_files(r#""remove": { "receipt_files": true }"#),
    )];

    let plan = fixture.plan(&specs, &installed(&["demo"]), Some(&state));
    let demo = item(&plan, "demo");

    assert_eq!(demo.removability, Removability::Removable);
    assert_eq!(removed(demo), sorted(vec![plugin.clone(), script]));
    assert!(
        demo.files
            .iter()
            .all(|file| file.reason == RemovalReason::Receipt)
    );
    let plugin_file = demo.files.iter().find(|file| file.path == plugin).unwrap();
    assert_eq!(plugin_file.size, Some(13));
    assert_eq!(plugin_file.receipt_sha256.as_deref(), Some("aa"));
    assert!(demo.has_receipt);
    assert!(demo.notes.is_empty(), "RABBIT installed this copy");
    assert!(demo.kept.is_empty());
}

#[test]
fn reaper_configuration_and_rabbits_folder_always_stay() {
    let fixture = Fixture::new();
    let ini = fixture.file("reaper.ini");
    let keys = fixture.file("reaper-kb.ini");
    let merge_backup = fixture.file("reaper-kb.ini.osara-merge-backup");
    let receipt_file = fixture.file("RABBIT/install-state.json");
    // A receipt anyone can edit, naming files no package may take.
    let state = receipt(
        "demo",
        &[
            ("reaper.ini", Some("aa")),
            ("reaper-kb.ini.osara-merge-backup", Some("bb")),
            ("RABBIT/install-state.json", Some("cc")),
        ],
    );
    let specs = [spec(
        "demo",
        "",
        &remove_files(
            r#""remove": { "receipt_files": true, "paths": ["{resource}/reaper-kb.ini"] }"#,
        ),
    )];

    let plan = fixture.plan(&specs, &installed(&["demo"]), Some(&state));
    let demo = item(&plan, "demo");

    assert_eq!(demo.removability, Removability::Removable);
    assert!(demo.files.is_empty());
    assert_eq!(
        kept(demo, KeptReason::ReaperConfiguration),
        sorted(vec![ini, keys, merge_backup])
    );
    assert_eq!(kept(demo, KeptReason::RabbitFolder), vec![receipt_file]);
}

#[test]
fn the_keep_list_wins_over_every_rule_that_would_remove() {
    let fixture = Fixture::new();
    let keymap = fixture.file("KeyMaps/Demo.ReaperKeyMap");
    let backup = fixture.file("KeyMaps/DemoBackup.ReaperKeyMap");
    let mine = fixture.file("KeyMaps/MineToo.ReaperKeyMap");
    let specs = [spec(
        "demo",
        "",
        &remove_files(
            r#""remove": { "paths": ["{resource}/KeyMaps/*.ReaperKeyMap"] },
               "keep": ["{resource}/KeyMaps/DemoBackup.ReaperKeyMap", "{resource}/KeyMaps/Mine*"]"#,
        ),
    )];

    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    let demo = item(&plan, "demo");

    assert_eq!(removed(demo), vec![keymap]);
    assert_eq!(kept(demo, KeptReason::KeepList), sorted(vec![backup, mine]));
}

/// A copy without a receipt can go where the manifest names its
/// files, with a note that RABBIT didn't install it.
#[test]
fn the_plugin_scan_and_wildcard_paths_find_a_copy_without_a_receipt() {
    let fixture = Fixture::new();
    let x64 = fixture.file("UserPlugins/reaper_demo-x64.dll");
    let x86 = fixture.file("UserPlugins/REAPER_DEMO32.DLL");
    fixture.file("UserPlugins/reaper_other.dll");
    fixture.file("UserPlugins/reaper_demo.txt");
    let po = fixture.file("demo/locale/de_DE.po");
    fixture.file("demo/locale/readme.txt");
    let specs = [spec(
        "demo",
        "",
        &remove_files(
            r#""remove": { "receipt_files": true, "matching_user_plugins": true,
                           "paths": ["{resource}/demo/locale/*.po"] }"#,
        ),
    )];

    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    let demo = item(&plan, "demo");

    assert_eq!(demo.removability, Removability::Removable);
    assert_eq!(removed(demo), sorted(vec![x64.clone(), x86, po.clone()]));
    let reason = |path: &Path| {
        demo.files
            .iter()
            .find(|file| file.path == path)
            .unwrap()
            .reason
    };
    assert_eq!(reason(&x64), RemovalReason::MatchingUserPlugin);
    assert_eq!(reason(&po), RemovalReason::ManifestPath);
    assert!(!demo.has_receipt);
    assert_eq!(demo.notes, vec![UninstallNote::NoReceipt]);
}

/// A file the manifest finds and the receipt lists goes as the manifest's,
/// which removes it even if it changed, and keeps the receipt's hash for the
/// report.
#[test]
fn a_file_found_twice_keeps_the_strongest_reason_and_the_receipt_hash() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo-x64.dll");
    let state = receipt("demo", &[("UserPlugins/reaper_demo-x64.dll", Some("aa"))]);
    let specs = [spec(
        "demo",
        "",
        &remove_files(r#""remove": { "receipt_files": true, "matching_user_plugins": true }"#),
    )];

    let plan = fixture.plan(&specs, &installed(&["demo"]), Some(&state));
    let demo = item(&plan, "demo");

    assert_eq!(demo.files.len(), 1);
    assert_eq!(demo.files[0].path, plugin);
    assert_eq!(demo.files[0].reason, RemovalReason::MatchingUserPlugin);
    assert!(demo.files[0].reason.named_by_manifest());
    assert_eq!(demo.files[0].receipt_sha256.as_deref(), Some("aa"));
}

#[test]
fn receipt_paths_outside_the_resource_folder_need_a_manifest_path() {
    let fixture = Fixture::new();
    let clap = write(&fixture.dir.path().join("CLAP").join("demo.clap"), b"clap");
    let elsewhere = write(
        &fixture.dir.path().join("Elsewhere").join("evil.dll"),
        b"evil",
    );
    let state = receipt(
        "demo",
        &[
            (clap.to_str().unwrap(), Some("aa")),
            (elsewhere.to_str().unwrap(), Some("bb")),
            ("../Elsewhere/evil.dll", Some("cc")),
        ],
    );
    let specs = [spec(
        "demo",
        "",
        &remove_files(r#""remove": { "receipt_files": true, "paths": ["{clap_dir}/demo.clap"] }"#),
    )];

    let plan = fixture.plan(&specs, &installed(&["demo"]), Some(&state));
    let demo = item(&plan, "demo");

    assert_eq!(demo.removability, Removability::Removable);
    assert_eq!(removed(demo), vec![clap]);
    assert_eq!(demo.files[0].reason, RemovalReason::ManifestPath);
    assert_eq!(demo.files[0].receipt_sha256.as_deref(), Some("aa"));
    let outside = kept(demo, KeptReason::OutsideResourceFolder);
    assert_eq!(outside.len(), 2, "{outside:?}");
    assert!(outside.contains(&elsewhere));
}

/// FFmpeg and the language packs: a copy without a receipt is most likely
/// the user's own.
#[test]
fn packages_that_need_a_receipt_are_refused_without_one() {
    let fixture = Fixture::new();
    let pack = fixture.file("LangPack/de_DE.ReaperLangPack");
    let specs = [spec(
        "langpack-demo",
        r#""install_destination": "lang_pack","#,
        &remove_files_with(
            r#""requires_receipt": true,"#,
            r#""remove": { "receipt_files": true }"#,
        ),
    )];

    let plan = fixture.plan(&specs, &installed(&["langpack-demo"]), None);
    assert_eq!(
        item(&plan, "langpack-demo").removability,
        Removability::NoReceipt
    );
    assert!(item(&plan, "langpack-demo").files.is_empty());

    let state = receipt(
        "langpack-demo",
        &[("LangPack/de_DE.ReaperLangPack", Some("aa"))],
    );
    let plan = fixture.plan(&specs, &installed(&["langpack-demo"]), Some(&state));
    assert_eq!(
        item(&plan, "langpack-demo").removability,
        Removability::Removable
    );
    assert_eq!(removed(item(&plan, "langpack-demo")), vec![pack]);
}

/// Planning reads existence and size only. A hash that no longer matches
/// changes nothing here; the engine checks it right before the move.
#[test]
fn planning_reads_sizes_but_never_checks_hashes() {
    let fixture = Fixture::new();
    let plugin = write(
        &fixture.resource.join("UserPlugins").join("reaper_demo.dll"),
        b"a different build entirely",
    );
    let state = receipt("demo", &[("UserPlugins/reaper_demo.dll", Some("0000"))]);
    let specs = [spec(
        "demo",
        "",
        &remove_files(r#""remove": { "receipt_files": true }"#),
    )];

    let plan = fixture.plan(&specs, &installed(&["demo"]), Some(&state));
    let demo = item(&plan, "demo");

    assert_eq!(demo.removability, Removability::Removable);
    assert_eq!(demo.files[0].path, plugin);
    assert_eq!(demo.files[0].size, Some(26));
    assert_eq!(demo.files[0].receipt_sha256.as_deref(), Some("0000"));
}

/// With the file held open so nothing else may read it, planning still
/// works: it never opens a file to hash it.
#[cfg(windows)]
#[test]
fn planning_never_opens_a_file_to_read_it() {
    use std::os::windows::fs::OpenOptionsExt;

    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let state = receipt("demo", &[("UserPlugins/reaper_demo.dll", Some("aa"))]);
    let specs = [spec(
        "demo",
        "",
        &remove_files(r#""remove": { "receipt_files": true }"#),
    )];

    let _held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&plugin)
        .unwrap();
    assert!(
        crate::hash::sha256_file(&plugin).is_err(),
        "the file can't be read while it is held"
    );

    let plan = fixture.plan(&specs, &installed(&["demo"]), Some(&state));
    let demo = item(&plan, "demo");
    assert_eq!(demo.removability, Removability::Removable);
    assert_eq!(demo.files[0].size, Some(13));
}

const DEMO_EXTENSION: &str = "Demo Extension";

fn scan_plan() -> String {
    remove_files(r#""remove": { "matching_user_plugins": true }"#)
}

#[test]
fn reapack_blocks_what_it_manages_while_installed() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    write_test_registry(
        &fixture.resource,
        &[(DEMO_EXTENSION, "1.0", &["UserPlugins/reaper_demo.dll"])],
    );
    let specs = [spec("demo", "", &scan_plan())];

    let plan = fixture.plan(&specs, &installed(&["demo", PACKAGE_REAPACK]), None);
    let demo = item(&plan, "demo");

    assert_eq!(
        demo.removability,
        Removability::ManagedByReaPack {
            path: plugin,
            reapack_package: DEMO_EXTENSION.to_string()
        }
    );
    assert!(demo.removability.is_blocked());
    assert!(demo.files.is_empty());

    // Without a detection for ReaPack, RABBIT can't tell that it is gone.
    let plan = fixture.plan(&specs, &[detection("demo", true)], None);
    assert!(matches!(
        item(&plan, "demo").removability,
        Removability::ManagedByReaPack { .. }
    ));
}

#[test]
fn reapack_lookups_that_cant_decide_block() {
    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    write(
        &fixture.resource.join("ReaPack").join("registry.db"),
        b"not a database",
    );
    let specs = [spec("demo", "", &scan_plan())];

    let plan = fixture.plan(&specs, &installed(&["demo", PACKAGE_REAPACK]), None);
    assert!(matches!(
        item(&plan, "demo").removability,
        Removability::ReaPackCheckFailed { .. }
    ));
}

/// Once ReaPack itself is gone, nothing keeps its records, and
/// copies it installed may go, with a note.
#[test]
fn copies_reapack_once_installed_may_go_once_it_is_gone() {
    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    write_test_registry(
        &fixture.resource,
        &[(DEMO_EXTENSION, "1.0", &["UserPlugins/reaper_demo.dll"])],
    );
    let specs = [spec("demo", "", &scan_plan())];

    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    let demo = item(&plan, "demo");

    assert_eq!(
        demo.removability,
        Removability::RemovableOrphanedFromReaPack
    );
    assert!(demo.removability.is_removable());
    assert_eq!(removed(demo), vec![plugin]);
    assert!(demo.notes.contains(&UninstallNote::OrphanedFromReaPack {
        reapack_packages: vec![DEMO_EXTENSION.to_string()]
    }));

    // Records that can't be read count as owned, and so as orphaned too.
    write(
        &fixture.resource.join("ReaPack").join("registry.db"),
        b"not a database",
    );
    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    let demo = item(&plan, "demo");
    assert_eq!(
        demo.removability,
        Removability::RemovableOrphanedFromReaPack
    );
    assert!(demo.notes.contains(&UninstallNote::OrphanedFromReaPack {
        reapack_packages: Vec::new()
    }));
}

/// ReaPack lists itself in its own records, so its own file never blocks
/// it. Before it goes, the plan names what it still manages.
#[test]
fn removing_reapack_names_the_extensions_it_still_manages() {
    let fixture = Fixture::new();
    let own = fixture.file("UserPlugins/reaper_reapack-x64.dll");
    fixture.file("UserPlugins/reaper_demo.dll");
    write_test_registry(
        &fixture.resource,
        &[
            ("ReaPack", "1.2.5", &["UserPlugins/reaper_reapack-x64.dll"]),
            (DEMO_EXTENSION, "1.0", &["UserPlugins/reaper_demo.dll"]),
            ("A script", "1.0", &["Scripts/a_script.lua"]),
        ],
    );
    let specs = [spec(
        PACKAGE_REAPACK,
        "",
        &remove_files_with(
            r#""reapack_owned": "ignore","#,
            r#""remove": { "matching_user_plugins": true }"#,
        ),
    )];

    let plan = fixture.plan(&specs, &installed(&[PACKAGE_REAPACK]), None);
    let reapack = item(&plan, PACKAGE_REAPACK);

    assert_eq!(reapack.removability, Removability::Removable);
    assert_eq!(removed(reapack), vec![own]);
    assert!(
        reapack
            .warnings
            .contains(&UninstallWarning::ReaPackStillManages {
                reapack_packages: vec![DEMO_EXTENSION.to_string()]
            })
    );
}

#[test]
fn warnings_follow_the_manifest_and_whats_installed() {
    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    let demo_block = remove_files_with(
        r#""always_warn": [{ "message_key": "test-always", "confirm": true }],
           "warn_when_installed": [
               { "package": "other", "message_key": "test-other" },
               { "package": "absent", "message_key": "test-absent" }
           ],"#,
        r#""remove": { "matching_user_plugins": true }"#,
    );
    let specs = [
        spec("demo", "", &demo_block),
        spec("other", r#""depends_on": ["demo"],"#, &scan_plan()),
        spec("absent", r#""depends_on": ["demo"],"#, &scan_plan()),
    ];

    let plan = fixture.plan(&specs, &installed(&["demo", "other"]), None);
    let demo = item(&plan, "demo");

    assert_eq!(
        demo.warnings,
        vec![
            UninstallWarning::Always {
                message_key: "test-always".to_string(),
                confirm: true
            },
            UninstallWarning::OtherPackageInstalled {
                package_id: "other".to_string(),
                message_key: "test-other".to_string()
            },
            UninstallWarning::DependentsStay {
                package_ids: vec!["other".to_string()]
            },
        ]
    );
    assert!(demo.confirm, "the warning asks for a confirmation");
    assert!(!demo.unbacked);
    assert!(!item(&plan, "other").confirm);
}

/// A machine-wide package serves every REAPER, so a portable
/// target can't remove it. On a standard target it warns that every program
/// loses it, and needs every REAPER closed.
#[test]
fn machine_wide_packages_need_a_standard_target() {
    let fixture = Fixture::new();
    write(
        &fixture.dir.path().join("CLAP").join("reaper_wide.clap"),
        b"clap",
    );
    let specs = [spec(
        "wide",
        r#""requires_standard_install": true, "install_destination": "windows_clap_dir","#,
        &remove_files(r#""remove": { "matching_user_plugins": true }"#),
    )];

    let portable = fixture.plan_with(
        &specs,
        &installed(&["wide"]),
        None,
        InstallationKind::Portable,
        false,
    );
    assert_eq!(
        item(&portable, "wide").removability,
        Removability::NeedsStandardTarget
    );
    assert_eq!(item(&portable, "wide").strategy, None);

    let standard = fixture.plan(&specs, &installed(&["wide"]), None);
    let wide = item(&standard, "wide");
    assert_eq!(wide.removability, Removability::Removable);
    assert!(wide.every_reaper_closed);
    assert!(wide.warnings.contains(&UninstallWarning::MachineWide));

    let absent = fixture.plan_with(&specs, &[], None, InstallationKind::Portable, false);
    assert_eq!(
        item(&absent, "wide").removability,
        Removability::NotInstalled
    );
}

/// "Removes it only from the standard REAPER" would promise a removal no
/// target offers when the package's plan removes nothing. Such a plan holds
/// on a portable target too, note and all.
#[test]
fn a_machine_wide_package_without_a_removal_says_why_on_every_target() {
    let fixture = Fixture::new();
    let specs = [
        spec(
            "by-hand",
            r#""requires_standard_install": true,"#,
            r#"{ "plans": [{ "strategy": "manual", "note_key": "test-by-hand" }] }"#,
        ),
        spec(
            "later",
            r#""requires_standard_install": true,"#,
            r#"{ "plans": [{ "strategy": "not_supported", "note_key": "uninstall-not-yet" }] }"#,
        ),
    ];
    for target in [InstallationKind::Standard, InstallationKind::Portable] {
        let plan = fixture.plan_with(
            &specs,
            &installed(&["by-hand", "later"]),
            None,
            target,
            false,
        );
        let by_hand = item(&plan, "by-hand");
        assert_eq!(
            by_hand.removability,
            Removability::Manual {
                note_key: "test-by-hand".to_string()
            },
            "{target:?}"
        );
        assert_eq!(by_hand.strategy, Some(UninstallStrategy::Manual));
        assert!(by_hand.warnings.contains(&UninstallWarning::MachineWide));
        assert_eq!(
            item(&plan, "later").removability,
            Removability::NotSupported {
                note_key: UNINSTALL_NOT_YET_KEY.to_string()
            },
            "{target:?}"
        );
    }
}

#[test]
fn manual_and_not_supported_plans_say_why() {
    let fixture = Fixture::new();
    let manual = spec(
        "manual",
        "",
        r#"{ "plans": [{ "strategy": "manual", "note_key": "test-by-hand" }] }"#,
    );
    let later = spec(
        "later",
        "",
        r#"{ "plans": [{ "strategy": "not_supported", "note_key": "uninstall-not-yet" }] }"#,
    );
    let specs = [manual, later];

    let plan = fixture.plan(&specs, &installed(&["manual", "later"]), None);
    assert_eq!(
        item(&plan, "manual").removability,
        Removability::Manual {
            note_key: "test-by-hand".to_string()
        }
    );
    assert_eq!(
        item(&plan, "later").removability,
        Removability::NotSupported {
            note_key: UNINSTALL_NOT_YET_KEY.to_string()
        }
    );
    assert!(item(&plan, "later").removability.is_blocked());

    // Not installed is the answer for a package that isn't there, whatever
    // its plan says.
    let plan = fixture.plan(&specs, &installed(&[]), None);
    assert_eq!(
        item(&plan, "manual").removability,
        Removability::NotInstalled
    );
    assert!(!item(&plan, "manual").removability.is_blocked());
}

/// Locating and trusting a vendor uninstaller comes with the vendor runner.
/// Until then no vendor plan can go ahead, and the plan says what it would
/// need: administrator rights, no backup, every REAPER closed.
#[test]
fn vendor_plans_wait_for_an_uninstaller_rabbit_can_trust() {
    let fixture = Fixture::new();
    let specs = [spec(
        "vendor",
        "",
        &format!(
            r#"{{ "plans": [{{
                "platform": "windows",
                "strategy": "vendor_uninstaller",
                "elevation": "required",
                "backup": "none",
                {SUMMARY},
                "vendor_uninstaller": {{
                    "kind": "inno",
                    "locate": [{{ "registry": {{ "hive": "hklm", "key": "Vendor_is1", "value": "UninstallString" }} }}],
                    "must_be_in": {{ "dir": "{{program_data}}/Vendor/uninstall", "only_files": "unins000.*" }},
                    "timeout_seconds": 600
                }},
                "kept_user_data": [{{ "path": "{{resource}}/Vendor", "note_key": "test-kept" }}]
            }}] }}"#
        ),
    )];

    let plan = fixture.plan(&specs, &installed(&["vendor"]), None);
    let vendor = item(&plan, "vendor");

    assert_eq!(vendor.removability, Removability::NoTrustedUninstaller);
    assert_eq!(vendor.strategy, Some(UninstallStrategy::VendorUninstaller));
    assert!(vendor.requires_elevation && vendor.unbacked && vendor.confirm);
    assert!(vendor.every_reaper_closed);
    assert_eq!(
        kept(vendor, KeptReason::UserData),
        vec![fixture.resource.join("Vendor")]
    );
}

/// "Also remove settings" takes the package's kept_user_data
/// into the plan, but never a folder root, REAPER's configuration, anything
/// on the keep list, or what isn't there.
#[test]
fn settings_stay_unless_the_caller_asks_for_them() {
    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    let grooves = fixture.resource.join("Data").join("Grooves");
    write(&grooves.join("mine.rgt"), b"groove");
    let settings = fixture.file("Demo.ini");
    let ini = fixture.file("reaper.ini");
    let backup = fixture.file("KeyMaps/DemoBackup.ReaperKeyMap");
    let replaced = fixture.file("KeyMaps/DemoReplacedBackup.ReaperKeyMap");
    let specs = [spec(
        "demo",
        "",
        &remove_files(
            r#""remove": { "matching_user_plugins": true },
               "keep": ["{resource}/KeyMaps/DemoBackup.ReaperKeyMap"],
               "kept_user_data": [
                   { "path": "{resource}/KeyMaps/DemoReplacedBackup.ReaperKeyMap", "note_key": "test-replaced" },
                   { "path": "{resource}/KeyMaps", "note_key": "test-keymaps" },
                   { "path": "{resource}/Data/Grooves", "note_key": "test-grooves" },
                   { "path": "{resource}/Demo.ini", "note_key": "test-settings" },
                   { "path": "{resource}", "note_key": "test-resource" },
                   { "path": "{resource}/reaper.ini", "note_key": "test-ini" },
                   { "path": "{resource}/KeyMaps/DemoBackup.ReaperKeyMap", "note_key": "test-backup" },
                   { "path": "{resource}/Gone.ini", "note_key": "test-gone" }
               ]"#,
        ),
    )];
    let detections = installed(&["demo"]);

    let plan = fixture.plan(&specs, &detections, None);
    let demo = item(&plan, "demo");
    assert!(
        demo.files
            .iter()
            .all(|file| file.reason != RemovalReason::Setting)
    );
    assert_eq!(demo.kept.len(), 8);
    let removable: Vec<PathBuf> = demo
        .kept
        .iter()
        .filter(|kept| kept.removable_with_settings)
        .map(|kept| kept.path.clone())
        .collect();
    assert_eq!(
        sorted(removable),
        sorted(vec![grooves.clone(), settings.clone()])
    );
    assert_eq!(
        kept(demo, KeptReason::FolderRoot),
        vec![fixture.resource.clone()]
    );
    assert_eq!(
        kept(demo, KeptReason::ReaperConfiguration),
        vec![ini.clone()]
    );
    assert_eq!(kept(demo, KeptReason::KeepList), vec![backup.clone()]);
    assert_eq!(
        kept(demo, KeptReason::KeyMap),
        sorted(vec![replaced.clone(), fixture.resource.join("KeyMaps")])
    );
    let gone = demo
        .kept
        .iter()
        .find(|kept| kept.note_key.as_deref() == Some("test-gone"))
        .unwrap();
    assert!(!gone.exists && !gone.removable_with_settings);

    let plan = fixture.plan_with(&specs, &detections, None, InstallationKind::Standard, true);
    let demo = item(&plan, "demo");
    let taken: Vec<PathBuf> = demo
        .files
        .iter()
        .filter(|file| file.reason == RemovalReason::Setting)
        .map(|file| file.path.clone())
        .collect();
    assert_eq!(sorted(taken), sorted(vec![grooves.clone(), settings]));
    assert!(
        demo.files
            .iter()
            .find(|file| file.path == grooves)
            .is_some_and(|file| file.is_dir && file.size.is_none())
    );
    assert!(demo.kept.iter().all(|kept| !kept.removable_with_settings));
    assert_eq!(demo.kept.len(), 6, "{:?}", demo.kept);
    assert!(ini.is_file() && backup.is_file() && replaced.is_file());
}

/// A package that can't be removed loses nothing, not even with "Also
/// remove settings": its data is all listed as kept.
#[test]
fn a_blocked_package_keeps_its_settings() {
    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    let settings = fixture.file("Demo.ini");
    write_test_registry(
        &fixture.resource,
        &[(DEMO_EXTENSION, "1.0", &["UserPlugins/reaper_demo.dll"])],
    );
    let specs = [spec(
        "demo",
        "",
        &remove_files(
            r#""remove": { "matching_user_plugins": true },
               "kept_user_data": [{ "path": "{resource}/Demo.ini", "note_key": "test-settings" }]"#,
        ),
    )];

    let plan = fixture.plan_with(
        &specs,
        &installed(&["demo", PACKAGE_REAPACK]),
        None,
        InstallationKind::Standard,
        true,
    );
    let demo = item(&plan, "demo");
    assert!(matches!(
        demo.removability,
        Removability::ManagedByReaPack { .. }
    ));
    assert!(demo.files.is_empty());
    assert_eq!(kept(demo, KeptReason::UserData), vec![settings]);
}

/// A path RABBIT can't place is a path it can't check. When a token stands
/// for nothing on this computer, the package is refused rather than removed
/// without that file.
#[test]
fn a_folder_rabbit_cant_find_blocks_the_package() {
    let mut fixture = Fixture::new();
    fixture.roots.clap_dir = None;
    fixture.file("UserPlugins/reaper_demo.dll");
    let specs = [spec(
        "demo",
        "",
        &remove_files(
            r#""remove": { "matching_user_plugins": true, "paths": ["{clap_dir}/demo.clap"] }"#,
        ),
    )];

    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    assert_eq!(
        item(&plan, "demo").removability,
        Removability::UnsafePath {
            path: PathBuf::from("{clap_dir}/demo.clap"),
            refusal: PathRefusal::UnknownRoot {
                token: "{clap_dir}".to_string()
            }
        }
    );
    assert!(item(&plan, "demo").files.is_empty());
}

/// A path the guard refuses blocks the whole package: removing the rest
/// would leave it half there while reporting success.
#[cfg(windows)]
#[test]
fn a_junction_where_a_file_should_be_blocks_the_package() {
    use super::paths::make_junction;

    let fixture = Fixture::new();
    let target = fixture.dir.path().join("target");
    write(&target.join("keep.txt"), b"keep");
    let link = fixture.resource.join("UserPlugins").join("reaper_demo.dll");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    make_junction(&link, &target);
    let specs = [spec("demo", "", &scan_plan())];

    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    assert_eq!(
        item(&plan, "demo").removability,
        Removability::UnsafePath {
            path: link,
            refusal: PathRefusal::Link
        }
    );
    assert!(item(&plan, "demo").files.is_empty());
    assert!(target.join("keep.txt").is_file());
}

/// RABBIT never goes through a junction out of the resource folder, the way
/// Surge XT's `SurgeXTData` junctions lead into ProgramData.
#[cfg(windows)]
#[test]
fn a_junctioned_folder_never_leads_a_removal_out_of_its_root() {
    use super::paths::make_junction;

    let fixture = Fixture::new();
    let outside = fixture.dir.path().join("Outside");
    let outside_plugin = write(&outside.join("reaper_demo.dll"), b"not REAPER's");
    make_junction(&fixture.resource.join("UserPlugins"), &outside);
    let specs = [spec("demo", "", &scan_plan())];

    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    assert!(matches!(
        &item(&plan, "demo").removability,
        Removability::UnsafePath {
            refusal: PathRefusal::OutsideRoot,
            ..
        }
    ));
    assert!(outside_plugin.is_file());
}

/// The guard runs again right before each move. A file that turned into a
/// junction after planning is refused then.
#[cfg(windows)]
#[test]
fn the_guard_runs_again_on_a_planned_file() {
    use super::paths::make_junction;

    let fixture = Fixture::new();
    let plugin = fixture.file("UserPlugins/reaper_demo.dll");
    let specs = [spec("demo", "", &scan_plan())];
    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    let planned = &item(&plan, "demo").files[0];
    assert_eq!(planned.check(&fixture.roots), Ok(()));

    fs::remove_file(&plugin).unwrap();
    let target = fixture.dir.path().join("target");
    fs::create_dir_all(&target).unwrap();
    make_junction(&plugin, &target);
    assert_eq!(planned.check(&fixture.roots), Err(PathRefusal::Link));
}

#[test]
fn a_resource_folder_reached_through_a_link_still_plans() {
    let fixture = Fixture::new();
    let real = fixture.dir.path().join("Real REAPER");
    let plugin = write(
        &real.join("UserPlugins").join("reaper_demo.dll"),
        b"plug-in",
    );
    let aliased = fixture.dir.path().join("Aliased REAPER");
    #[cfg(windows)]
    super::paths::make_junction(&aliased, &real);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, &aliased).unwrap();
    let roots = UninstallRoots::new(&aliased);
    let specs = [spec("demo", "", &scan_plan())];

    let plan = build_uninstall_plan(&UninstallPlanInput {
        roots: &roots,
        platform: Platform::Windows,
        target: InstallationKind::Standard,
        specs: &specs,
        detections: &installed(&["demo"]),
        state: None,
        remove_settings: false,
    });
    let demo = item(&plan, "demo");
    assert_eq!(demo.removability, Removability::Removable);
    assert_eq!(
        removed(demo),
        vec![aliased.join("UserPlugins").join("reaper_demo.dll")]
    );
    assert!(plugin.is_file());
}

/// Extensions go before REAPER. Packages with no dependency between them
/// keep manifest order.
#[test]
fn packages_are_removed_before_what_they_depend_on() {
    let fixture = Fixture::new();
    let specs = builtin_package_specs(Platform::Windows);
    let plan = fixture.plan(&specs, &[], None);
    let all: Vec<String> = specs.iter().map(|spec| spec.id.clone()).collect();

    let order: Vec<&str> = plan
        .in_removal_order(&all)
        .iter()
        .map(|item| item.package_id.as_str())
        .collect();

    assert_eq!(order.len(), all.len());
    assert_eq!(order.last(), Some(&PACKAGE_REAPER));
    let position = |id: &str| order.iter().position(|candidate| *candidate == id).unwrap();
    for spec in &specs {
        for dependency in &spec.depends_on {
            assert!(
                position(&spec.id) < position(dependency),
                "{} must go before {dependency}: {order:?}",
                spec.id
            );
        }
    }
    let some = plan.in_removal_order(&[PACKAGE_REAPER.to_string(), "sws".to_string()]);
    assert_eq!(
        some.iter()
            .map(|item| item.package_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sws", PACKAGE_REAPER]
    );
}

#[test]
fn packages_without_a_dependency_between_them_keep_manifest_order() {
    let fixture = Fixture::new();
    let not_yet =
        r#"{ "plans": [{ "strategy": "not_supported", "note_key": "uninstall-not-yet" }] }"#;
    let specs = [
        spec("host", "", not_yet),
        spec("first", r#""depends_on": ["host"],"#, not_yet),
        spec("standalone", "", not_yet),
        spec("second", r#""depends_on": ["first"],"#, not_yet),
    ];
    let plan = fixture.plan(&specs, &[], None);
    let all: Vec<String> = specs.iter().map(|spec| spec.id.clone()).collect();

    let order: Vec<&str> = plan
        .in_removal_order(&all)
        .iter()
        .map(|item| item.package_id.as_str())
        .collect();
    assert_eq!(order, vec!["standalone", "second", "first", "host"]);
}

/// Every shipped manifest plans on both platforms and both targets without
/// tripping over anything. With nothing on disk, a package whose plan
/// removes files is removable unless it needs a receipt; every other
/// installed package says why RABBIT won't remove it.
#[test]
fn every_shipped_manifest_plans() {
    let fixture = Fixture::new();
    for platform in [Platform::Windows, Platform::MacOs] {
        let specs = builtin_package_specs(platform);
        let all_installed: Vec<ComponentDetection> =
            specs.iter().map(|spec| detection(&spec.id, true)).collect();
        for target in [InstallationKind::Standard, InstallationKind::Portable] {
            let nothing = build_uninstall_plan(&UninstallPlanInput {
                roots: &fixture.roots,
                platform,
                target,
                specs: &specs,
                detections: &[],
                state: None,
                remove_settings: false,
            });
            assert_eq!(nothing.items.len(), specs.len());
            assert!(
                nothing
                    .items
                    .iter()
                    .all(|item| item.removability == Removability::NotInstalled)
            );

            let everything = build_uninstall_plan(&UninstallPlanInput {
                roots: &fixture.roots,
                platform,
                target,
                specs: &specs,
                detections: &all_installed,
                state: None,
                remove_settings: true,
            });
            for (item, spec) in everything.items.iter().zip(&specs) {
                let removes_files = spec
                    .uninstall
                    .plan_for(platform, target)
                    .is_some_and(|plan| plan.strategy == UninstallStrategy::RemoveFiles);
                let standard_removes = spec
                    .uninstall
                    .plan_for(platform, InstallationKind::Standard)
                    .is_some_and(|plan| {
                        matches!(
                            plan.strategy,
                            UninstallStrategy::RemoveFiles | UninstallStrategy::VendorUninstaller
                        )
                    });
                if spec.requires_standard_install
                    && target == InstallationKind::Portable
                    && standard_removes
                {
                    assert_eq!(item.removability, Removability::NeedsStandardTarget);
                } else if removes_files && !spec.uninstall.requires_receipt {
                    assert!(
                        item.removability.is_removable(),
                        "{}: {:?}",
                        item.package_id,
                        item.removability
                    );
                } else {
                    assert!(
                        item.removability.is_blocked(),
                        "{}: {:?}",
                        item.package_id,
                        item.removability
                    );
                }
                assert!(item.files.is_empty());
            }
        }
    }
}

// The uninstall preflight.

pub(super) fn running(pid: &str, name: &str, exe: &Path) -> RunningProcess {
    RunningProcess {
        pid: pid.to_string(),
        name: name.to_string(),
        executable_path: Some(exe.to_path_buf()),
    }
}

fn check_status(report: &PreflightReport, name: &str) -> Option<PreflightStatus> {
    report
        .checks
        .iter()
        .find(|check| check.name == name)
        .map(|check| check.status)
}

/// A demo extension in the resource folder and a machine-wide package, both
/// installed and removable.
fn preflight_fixture() -> (Fixture, UninstallPlan) {
    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    write(
        &fixture.dir.path().join("CLAP").join("reaper_wide.clap"),
        b"clap",
    );
    let specs = [
        spec("demo", "", &scan_plan()),
        spec(
            "wide",
            r#""requires_standard_install": true, "install_destination": "windows_clap_dir","#,
            &remove_files(r#""remove": { "matching_user_plugins": true }"#),
        ),
    ];
    let plan = fixture.plan(&specs, &installed(&["demo", "wide"]), None);
    assert!(
        plan.items
            .iter()
            .all(|item| item.removability.is_removable())
    );
    (fixture, plan)
}

/// There is no way to allow a running REAPER for uninstall,
/// so the dry run reports it as a blocker too.
#[test]
fn the_targets_reaper_blocks_even_a_dry_run() {
    let (fixture, plan) = preflight_fixture();
    let app = fixture.dir.path().join("Program").join("reaper.exe");
    let options = UninstallPreflightOptions {
        dry_run: true,
        target_app_path: Some(app.clone()),
    };

    let report = run_uninstall_preflight_with_processes(
        &plan,
        &["demo".to_string()],
        &options,
        &[running("1", "reaper.exe", &app)],
        &[],
    );
    assert!(!report.passed);
    assert_eq!(
        check_status(&report, REAPER_PROCESS_CHECK),
        Some(PreflightStatus::Fail)
    );

    let report =
        run_uninstall_preflight_with_processes(&plan, &["demo".to_string()], &options, &[], &[]);
    assert!(report.passed, "{report:?}");
    assert_eq!(check_status(&report, ANY_REAPER_PROCESS_CHECK), None);
}

/// A portable REAPER doesn't stop removing an extension from the standard
/// target, but it does stop a machine-wide package it may have loaded.
#[test]
fn a_portable_reaper_blocks_machine_wide_packages_only() {
    let (fixture, plan) = preflight_fixture();
    let options = UninstallPreflightOptions {
        dry_run: false,
        target_app_path: Some(fixture.dir.path().join("Program").join("reaper.exe")),
    };
    let portable = [running(
        "2",
        "reaper.exe",
        &fixture.dir.path().join("Portable").join("reaper.exe"),
    )];

    let demo = run_uninstall_preflight_with_processes(
        &plan,
        &["demo".to_string()],
        &options,
        &portable,
        &[],
    );
    assert!(demo.passed, "{demo:?}");

    let wide = run_uninstall_preflight_with_processes(
        &plan,
        &["demo".to_string(), "wide".to_string()],
        &options,
        &portable,
        &[],
    );
    assert!(!wide.passed);
    assert_eq!(
        check_status(&wide, REAPER_PROCESS_CHECK),
        Some(PreflightStatus::Pass)
    );
    assert_eq!(
        check_status(&wide, ANY_REAPER_PROCESS_CHECK),
        Some(PreflightStatus::Fail)
    );
}

#[test]
fn programs_a_plan_names_block_while_running() {
    let fixture = Fixture::new();
    fixture.file("UserPlugins/reaper_demo.dll");
    let specs = [spec(
        "demo",
        "",
        &remove_files(
            r#""remove": { "matching_user_plugins": true },
               "blocking_programs": ["Demo Standalone.exe"]"#,
        ),
    )];
    let plan = fixture.plan(&specs, &installed(&["demo"]), None);
    let options = UninstallPreflightOptions {
        dry_run: true,
        target_app_path: Some(fixture.dir.path().join("Program").join("reaper.exe")),
    };
    let standalone = running(
        "3",
        "demo standalone.EXE",
        &fixture.dir.path().join("Demo Standalone.exe"),
    );

    let report = run_uninstall_preflight_with_processes(
        &plan,
        &["demo".to_string()],
        &options,
        &[],
        std::slice::from_ref(&standalone),
    );
    assert!(!report.passed);
    assert_eq!(
        check_status(&report, PROGRAMS_RUNNING_CHECK),
        Some(PreflightStatus::Fail)
    );

    // A package that isn't asked for, or can't be removed, needs nothing
    // closed.
    let report = run_uninstall_preflight_with_processes(
        &plan,
        &[],
        &options,
        &[],
        std::slice::from_ref(&standalone),
    );
    assert!(report.passed);
    assert_eq!(check_status(&report, PROGRAMS_RUNNING_CHECK), None);
}
