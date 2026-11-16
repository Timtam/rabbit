//! Tests of restoring a key map (backup plan 6.5): the key map comes back
//! as the backup has it and keeps the scripts registered since, each once,
//! as OSARA's key map replacement keeps them; it is a backup of its own,
//! whatever else its set holds; its before-restore set lists as a key map,
//! so the restore can be undone the same way; it is all or nothing, and
//! never while REAPER may run. Temporary folders only.

use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use tempfile::tempdir;

use super::restore_run_tests::{snapshot, snapshot_but_lock_folder};
use super::restore_seam::{self, Step};
use super::{
    BEFORE_RESTORE_REASON, BackupEntryState, BackupEnvironment, BackupPackageRecord,
    BackupSetWriter, KEYMAP_BACKUP_REASON, KeymapOrigin, OutsideRestoreRules,
    ReplacedPackageRecord, RestoreBackupActionKind, RestoreBackupOptions, RestoreBackupReport,
    RestoreItem, RestoreRunOptions, RestoreStatus, RestoredKeymap, UPDATE_BACKUP_REASON,
    load_backup_manifest, restore_backup_item_with, restore_backup_set_with, restore_backups_with,
    save_backup_manifest, temporary_restore_path,
};
use crate::backup_overview::{
    BackupEntry, BackupGroup, BackupItem, BackupSetKind, BackupStatus, RestoreScope, overview_with,
};
use crate::cancel::CancelToken;
use crate::error::Result;
use crate::hash::sha256_bytes;
use crate::progress::ProgressReporter;
use crate::receipt::{load_install_state, save_install_state};
use crate::restore_refusal::{RestoreRefusal, TargetRefusal, refusal};
use crate::uninstall::paths::link_folder;
use crate::version::Version;

const KB: &str = "reaper-kb.ini";

/// The key map set OSARA's key map replacement wrote.
const SET: &str = "unattended-unix-100-000000000";

/// A key binding of the user's own, and one of OSARA's key map.
const OWN_KEY: &str = "KEY 1 65 40044 0";
const OSARA_KEY: &str = "KEY 1 65 40001 0";

/// A script registered before OSARA's key map replaced the user's, which
/// the replacement kept, and one ReaPack installed since.
const OLD_SCRIPT: &str = "SCR 4 0 RSold \"Custom: old.lua\" old.lua";
const NEW_SCRIPT: &str = "SCR 4 0 RSnew \"Custom: new.lua\" new.lua";

/// The old script, registered in the MIDI editor's section as well: one
/// script in another section counts as another script.
const OLD_SCRIPT_IN_MIDI: &str = "SCR 4 32060 RSold \"Custom: old.lua\" old.lua";

/// A real run, which must not let REAPER run: a key map never comes back
/// while it may.
const APPLY: RestoreRunOptions = RestoreRunOptions {
    dry_run: false,
    quick: false,
    allow_reaper_running: false,
};

const DRY_RUN: RestoreRunOptions = RestoreRunOptions {
    dry_run: true,
    ..APPLY
};

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// `lines`, each ending in `newline`.
fn text(lines: &[&str], newline: &str) -> String {
    lines
        .iter()
        .map(|line| format!("{line}{newline}"))
        .collect()
}

fn set_path(resource: &Path, id: &str) -> PathBuf {
    resource.join("RABBIT").join("backups").join(id)
}

/// The sets in `resource`'s `RABBIT/backups`, by name.
fn sets(resource: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(resource.join("RABBIT").join("backups"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// A resource folder `REAPER` in `dir` whose key map set `SET` holds
/// `old`, the user's own key map, which OSARA's key map replaced, while
/// `reaper-kb.ini` is `now`.
fn keymap_replaced(dir: &Path, old: &[u8], now: &[u8]) -> PathBuf {
    let resource = dir.join("REAPER");
    let keymap = resource.join(KB);
    write(&keymap, old);
    let mut writer = BackupSetWriter::new(&resource, SET, KEYMAP_BACKUP_REASON);
    writer.set_keymap_origin(KeymapOrigin::OwnKeymap).unwrap();
    writer.copy_file(Some("osara"), &keymap).unwrap();
    fs::write(&keymap, now).unwrap();
    resource
}

/// An uninstall's set `set_id` in `resource` that holds SWS's plug-in,
/// gone from its place, with SWS's receipt entry. Returns the plug-in.
fn sws_uninstalled(resource: &Path, set_id: &str) -> PathBuf {
    let plugin = resource.join("UserPlugins").join("reaper_sws-x64.dll");
    write(&plugin, b"sws");
    let mut writer = BackupSetWriter::new(resource, set_id, "uninstall");
    writer
        .record_package(
            "sws",
            BackupPackageRecord {
                receipt: None,
                langpack_before: None,
                with_settings: None,
            },
        )
        .unwrap();
    writer.move_file(Some("sws"), &plugin, None, false).unwrap();
    plugin
}

/// Restore `item` from `resource` as the user, with no REAPER running and
/// nowhere outside the resource folder to write to.
fn restore(
    resource: &Path,
    item: &RestoreItem,
    options: &RestoreRunOptions,
) -> Result<RestoreBackupReport> {
    restore_backup_item_with(
        resource,
        item,
        options,
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
    )
}

fn scripts_added(report: &RestoreBackupReport) -> Option<usize> {
    report
        .keymap
        .as_ref()
        .and_then(|keymap| keymap.scripts_added)
}

/// The key map's backup in `set_id`, as the overview lists it.
fn keymap_row(resource: &Path, set_id: &str) -> BackupEntry {
    overview_with(resource, &BackupEnvironment::AS_USER)
        .unwrap()
        .entries
        .into_iter()
        .find(|entry| {
            entry.item
                == BackupItem::Keymap {
                    set_id: set_id.to_string(),
                }
        })
        .unwrap_or_else(|| panic!("no key map in {set_id}"))
}

/// The key map comes back as the backup has it, then gets the script lines
/// of the one it replaced that it doesn't register, each once and ending as
/// its own lines end: a script it registers already isn't doubled, one the
/// replaced key map lists twice comes once, and one registered in another
/// section comes too. A dry run counts them and changes nothing, and the
/// quick plan reads no key map. RABBIT's records don't change.
#[test]
fn a_key_map_restore_keeps_the_scripts_registered_since_each_once() {
    for (old_newline, now_newline) in [
        ("\r\n", "\r\n"),
        ("\n", "\n"),
        ("\n", "\r\n"),
        ("\r\n", "\n"),
    ] {
        let dir = tempdir().unwrap();
        let old = text(&[OWN_KEY, OLD_SCRIPT], old_newline);
        let now = text(
            &[
                OSARA_KEY,
                OLD_SCRIPT,
                NEW_SCRIPT,
                OLD_SCRIPT_IN_MIDI,
                NEW_SCRIPT,
            ],
            now_newline,
        );
        let resource = keymap_replaced(dir.path(), old.as_bytes(), now.as_bytes());
        let keymap = resource.join(KB);
        let case = format!("{old_newline:?} then {now_newline:?}");

        let before = snapshot(dir.path());
        let planned = restore(&resource, &RestoreItem::keymap(SET), &DRY_RUN).unwrap();
        let quick = restore(
            &resource,
            &RestoreItem::keymap(SET),
            &RestoreRunOptions {
                quick: true,
                ..DRY_RUN
            },
        )
        .unwrap();
        assert_eq!(snapshot(dir.path()), before, "{case}");
        assert_eq!(planned.status, RestoreStatus::WouldRestore, "{case}");
        assert_eq!(
            planned.keymap,
            Some(RestoredKeymap {
                path: keymap.clone(),
                replaced: true,
                scripts_added: Some(2),
            }),
            "{case}"
        );
        assert!(planned.keymap_only, "{case}");
        assert_eq!(scripts_added(&quick), None, "{case}");
        assert!(quick.keymap.is_some(), "{case}");

        let report = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();

        assert_eq!(report.status, RestoreStatus::Restored, "{case}");
        assert_eq!(scripts_added(&report), Some(2), "{case}");
        assert_eq!(
            fs::read_to_string(&keymap).unwrap(),
            format!(
                "{old}{}",
                text(&[NEW_SCRIPT, OLD_SCRIPT_IN_MIDI], old_newline)
            ),
            "{case}"
        );
        let before_restore = report.before_restore_set_id.clone().unwrap();
        assert_eq!(
            fs::read_to_string(set_path(&resource, &before_restore).join(KB)).unwrap(),
            now,
            "{case}"
        );
        assert!(report.packages.is_empty(), "{case}");
        assert!(
            !resource.join("RABBIT").join("install-state.json").exists(),
            "{case}"
        );
        assert!(report.saved_report.is_some(), "{case}");
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["keymap"]["scripts_added"], 2, "{case}");
        assert_eq!(json["keymap"]["replaced"], true, "{case}");
        assert_eq!(json["keymap_only"], true, "{case}");
    }
}

/// A script whose name goes beyond ASCII is kept as OSARA's replacement
/// keeps it: written in UTF-8, it comes back byte for byte; carrying a byte
/// of an ANSI code page, as REAPER on Windows may write one, it survives.
#[test]
fn scripts_with_names_beyond_ascii_are_kept() {
    let dir = tempdir().unwrap();
    let old = text(&[OWN_KEY], "\r\n");
    let utf8 = "SCR 4 0 RSgross \"Custom: Größe ändern.lua\" groesse.lua";
    let now = text(&[OSARA_KEY, utf8], "\r\n");
    let resource = keymap_replaced(dir.path(), old.as_bytes(), now.as_bytes());

    let report = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();

    assert_eq!(scripts_added(&report), Some(1));
    assert_eq!(
        fs::read(resource.join(KB)).unwrap(),
        format!("{old}{utf8}\r\n").as_bytes()
    );

    let dir = tempdir().unwrap();
    let mut now = b"KEY 9 65 _RSabc 0\r\nSCR 4 0 RSdeadbeef \"Script: Tom".to_vec();
    now.push(0x92);
    now.extend_from_slice(b"s tool.lua\" tool.lua\r\n");
    let resource = keymap_replaced(dir.path(), old.as_bytes(), &now);

    let report = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();

    assert_eq!(scripts_added(&report), Some(1));
    let restored = fs::read(resource.join(KB)).unwrap();
    let lossy = String::from_utf8_lossy(&restored);
    assert!(lossy.starts_with(&old), "{lossy}");
    assert!(lossy.contains("SCR 4 0 RSdeadbeef"), "{lossy}");
    assert!(lossy.contains("tool.lua"), "{lossy}");
    assert!(!lossy.contains("KEY 9 65"), "{lossy}");
}

/// What a key map restore replaced goes into a before-restore set that the
/// overview lists as a key map, found by what it holds, restored on its own
/// like any key map. Restoring it undoes the restore, and restoring the set
/// that made, whole, goes forward again: each keeps the scripts the same
/// way. So a script the old key map registers that the key map it replaced
/// no longer does stays after the undo too.
#[test]
fn a_key_map_restore_is_undone_through_its_own_backup() {
    let dir = tempdir().unwrap();
    let old = text(&[OWN_KEY, OLD_SCRIPT], "\r\n");
    let now = text(&[OSARA_KEY, OLD_SCRIPT, NEW_SCRIPT], "\r\n");
    let resource = keymap_replaced(dir.path(), old.as_bytes(), now.as_bytes());
    let keymap = resource.join(KB);
    let restored_keymap = format!("{old}{NEW_SCRIPT}\r\n");

    let restored = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();
    assert_eq!(fs::read_to_string(&keymap).unwrap(), restored_keymap);
    let undo = restored.before_restore_set_id.unwrap();

    let row = keymap_row(&resource, &undo);
    assert_eq!(row.group, BackupGroup::Keymaps);
    assert_eq!(row.set_kind, BackupSetKind::BeforeRestore);
    assert_eq!(row.status, BackupStatus::CanRestore);
    assert_eq!(row.restore_scope, Some(RestoreScope::Keymap));
    assert_eq!(row.refusal, None);
    assert!(row.only_copy);
    // The key map is all the set holds.
    assert!(row.shares_set_with.is_empty());
    assert_eq!(row.size, now.len() as u64);

    let undone = restore(&resource, &RestoreItem::keymap(&undo), &APPLY).unwrap();

    assert_eq!(undone.status, RestoreStatus::Restored);
    assert_eq!(fs::read_to_string(&keymap).unwrap(), now);
    assert_eq!(scripts_added(&undone), Some(0));

    let again = undone.before_restore_set_id.unwrap();
    let forward = restore(&resource, &RestoreItem::whole_set(&again), &APPLY).unwrap();

    assert_eq!(forward.status, RestoreStatus::Restored);
    assert_eq!(fs::read_to_string(&keymap).unwrap(), restored_keymap);
    assert!(forward.keymap.is_some() && !forward.keymap_only);

    // The old script was removed since OSARA's key map replaced the old
    // one. The undo keeps it, as it keeps every script the key map it
    // replaces registers.
    let dir = tempdir().unwrap();
    let now = text(&[OSARA_KEY, NEW_SCRIPT], "\r\n");
    let resource = keymap_replaced(dir.path(), old.as_bytes(), now.as_bytes());
    let keymap = resource.join(KB);
    let restored = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();
    assert_eq!(fs::read_to_string(&keymap).unwrap(), restored_keymap);
    let undo = restored.before_restore_set_id.unwrap();

    let undone = restore(&resource, &RestoreItem::keymap(&undo), &APPLY).unwrap();

    assert_eq!(scripts_added(&undone), Some(1));
    assert_eq!(
        fs::read_to_string(&keymap).unwrap(),
        format!("{now}{OLD_SCRIPT}\r\n")
    );
}

/// The key map alone comes back from a set that holds other files too, a
/// copy of RABBIT's records among them, which a restore of the whole set
/// refuses. The set's other files stay in it as they were, those on disk
/// stay as they are, RABBIT's records don't change, and what the restore
/// replaced is the key map alone.
#[test]
fn a_key_map_comes_back_alone_and_its_set_keeps_the_rest() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let keymap = resource.join(KB);
    let plugin = resource.join("UserPlugins").join("reaper_osara64.dll");
    let receipt = resource.join("RABBIT").join("install-state.json");
    let old = text(&[OWN_KEY, OLD_SCRIPT], "\r\n");
    write(&keymap, old.as_bytes());
    write(&plugin, b"older osara");
    write(&receipt, b"{\"packages\":{}}");
    let id = "restore-unix-200-before-restore";
    let mut writer = BackupSetWriter::new(&resource, id, BEFORE_RESTORE_REASON);
    writer.copy_file(Some("osara"), &keymap).unwrap();
    writer.copy_file(Some("osara"), &plugin).unwrap();
    writer.copy_file(None, &receipt).unwrap();
    let now = text(&[OSARA_KEY, OLD_SCRIPT, NEW_SCRIPT], "\r\n");
    fs::write(&keymap, &now).unwrap();
    fs::write(&plugin, b"newer osara").unwrap();
    fs::write(&receipt, b"{\"packages\":{},\"declined_packages\":[]}").unwrap();
    let set = set_path(&resource, id);
    let set_before = snapshot(&set);

    let whole = restore(&resource, &RestoreItem::whole_set(id), &APPLY).unwrap_err();
    assert_eq!(refusal(&whole).code(), "receipt-in-set");
    let report = restore(&resource, &RestoreItem::keymap(id), &APPLY).unwrap();

    assert_eq!(report.status, RestoreStatus::Restored);
    assert!(report.keymap_only);
    let targets: Vec<&Path> = report
        .actions
        .iter()
        .map(|action| action.target_path.as_path())
        .collect();
    assert_eq!(targets, [keymap.as_path()]);
    assert_eq!(
        fs::read_to_string(&keymap).unwrap(),
        format!("{old}{NEW_SCRIPT}\r\n")
    );
    assert_eq!(fs::read(&plugin).unwrap(), b"newer osara");
    assert_eq!(
        fs::read(&receipt).unwrap(),
        b"{\"packages\":{},\"declined_packages\":[]}"
    );
    assert_eq!(snapshot(&set), set_before);
    let undo = report.before_restore_set_id.unwrap();
    let manifest = load_backup_manifest(&set_path(&resource, &undo))
        .unwrap()
        .unwrap();
    assert_eq!(manifest.files.len(), 1);
    assert_eq!(manifest.files[0].original_path, Path::new(KB));
}

/// OSARA's key map set restored whole is the same as its key map alone,
/// which is all it holds: the same files, and the scripts registered since
/// kept, in a dry run and for real, through the command line's restore of
/// a whole set too. A whole set that holds other files as well keeps them
/// the same way.
#[test]
fn a_key_map_set_restored_whole_keeps_the_scripts_too() {
    let dir = tempdir().unwrap();
    let old = text(&[OWN_KEY], "\n");
    let now = text(&[OSARA_KEY, NEW_SCRIPT], "\n");
    let resource = keymap_replaced(dir.path(), old.as_bytes(), now.as_bytes());

    let alone = restore(&resource, &RestoreItem::keymap(SET), &DRY_RUN).unwrap();
    let whole = restore(&resource, &RestoreItem::whole_set(SET), &DRY_RUN).unwrap();
    // The before-restore set each would make is named by the second it
    // was planned in, so only whether there is one counts.
    let files = |report: &RestoreBackupReport| -> Vec<(PathBuf, PathBuf, bool)> {
        report
            .actions
            .iter()
            .map(|action| {
                (
                    action.target_path.clone(),
                    action.source_path.clone(),
                    action.current_backup_path.is_some(),
                )
            })
            .collect()
    };
    assert_eq!(
        (whole.status, files(&whole), &whole.keymap),
        (alone.status, files(&alone), &alone.keymap)
    );
    assert!(alone.keymap_only && !whole.keymap_only);

    let report = restore_backup_set_with(
        &resource,
        SET,
        &RestoreBackupOptions {
            dry_run: false,
            allow_reaper_running: false,
            package_id: None,
        },
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
    )
    .unwrap();

    assert_eq!(report.status, RestoreStatus::Restored);
    assert_eq!(scripts_added(&report), Some(1));
    assert_eq!(
        fs::read_to_string(resource.join(KB)).unwrap(),
        format!("{old}{NEW_SCRIPT}\n")
    );

    // A set that holds the key map and another file, restored whole, puts
    // both back and keeps the scripts the same way.
    let keymap = resource.join(KB);
    let plugin = resource.join("UserPlugins").join("reaper_osara64.dll");
    let older = text(&[OWN_KEY, OLD_SCRIPT], "\n");
    fs::write(&keymap, &older).unwrap();
    write(&plugin, b"older osara");
    let both = "restore-unix-200-before-restore";
    let mut writer = BackupSetWriter::new(&resource, both, BEFORE_RESTORE_REASON);
    writer.copy_file(Some("osara"), &keymap).unwrap();
    writer.copy_file(Some("osara"), &plugin).unwrap();
    fs::write(&keymap, &now).unwrap();
    fs::write(&plugin, b"newer osara").unwrap();

    let report = restore(&resource, &RestoreItem::whole_set(both), &APPLY).unwrap();

    assert_eq!(report.status, RestoreStatus::Restored);
    assert_eq!(report.actions.len(), 2);
    assert_eq!(scripts_added(&report), Some(1));
    assert_eq!(
        fs::read_to_string(&keymap).unwrap(),
        format!("{older}{NEW_SCRIPT}\n")
    );
    assert_eq!(fs::read(&plugin).unwrap(), b"older osara");
}

/// A restore that can't add the scripts to the key map it put back is all
/// or nothing, as every restore is: the key map it replaced goes back from
/// the before-restore set, byte for byte, and that set goes again. The
/// report says why, and is saved.
#[test]
fn a_key_map_that_cant_keep_its_scripts_goes_back_as_it_was() {
    let dir = tempdir().unwrap();
    let old = text(&[OWN_KEY], "\r\n");
    let now = text(&[OSARA_KEY, NEW_SCRIPT], "\r\n");
    let resource = keymap_replaced(dir.path(), old.as_bytes(), now.as_bytes());
    let keymap = resource.join(KB);
    let failing = keymap.clone();
    let _hook = restore_seam::install(move |step, path| {
        if step == Step::KeepingScripts && path == failing {
            Err(io::Error::other("the disk is full"))
        } else {
            Ok(())
        }
    });

    let report = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();

    assert_eq!(report.status, RestoreStatus::NotRestored);
    assert!(
        report
            .error
            .as_deref()
            .is_some_and(|error| error.contains("the disk is full")),
        "{report:?}"
    );
    assert_eq!(fs::read(&keymap).unwrap(), now.as_bytes());
    assert_eq!(scripts_added(&report), None);
    assert_eq!(report.actions[0].action, RestoreBackupActionKind::PutBack);
    assert_eq!(report.before_restore_set_id, None);
    assert_eq!(sets(&resource), [SET]);
    assert!(report.saved_report.is_some());
}

/// With no key map there, a key map restore puts the old one back as the
/// backup has it: there is no key map to keep scripts from, so it adds no
/// line, and the report says nothing was replaced. Nothing goes into a
/// before-restore set.
#[test]
fn a_key_map_restore_with_no_key_map_there_adds_no_script() {
    let dir = tempdir().unwrap();
    let old = text(&[OWN_KEY, OLD_SCRIPT], "\r\n");
    let resource = keymap_replaced(dir.path(), old.as_bytes(), b"");
    let keymap = resource.join(KB);
    fs::remove_file(&keymap).unwrap();
    let nothing_replaced = RestoredKeymap {
        path: keymap.clone(),
        replaced: false,
        scripts_added: Some(0),
    };

    let planned = restore(&resource, &RestoreItem::keymap(SET), &DRY_RUN).unwrap();
    assert_eq!(planned.keymap.as_ref(), Some(&nothing_replaced));
    let report = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();

    assert_eq!(report.status, RestoreStatus::Restored);
    assert_eq!(report.keymap.as_ref(), Some(&nothing_replaced));
    assert_eq!(fs::read_to_string(&keymap).unwrap(), old);
    assert_eq!(report.before_restore_set_id, None);
}

/// An old key map in an ANSI code page, as REAPER on Windows may have
/// written it, comes back byte for byte, its own lines and all. The lines
/// it gets from the UTF-8 key map it replaced go in its encoding: a
/// character up to U+00FF as its one byte, any other as `?`.
#[test]
fn an_ansi_key_map_comes_back_byte_for_byte_with_new_lines_in_its_encoding() {
    let dir = tempdir().unwrap();
    let mut old = b"KEY 1 65 40044 0\r\nSCR 4 0 RSold \"Script: Tom".to_vec();
    old.push(0x92);
    old.extend_from_slice(b"s tool.lua\" tool.lua\r\n");
    let latin = "SCR 4 0 RSgross \"Custom: Gr\u{f6}\u{df}e.lua\" g.lua";
    let beyond = "SCR 4 0 RSnote \"Custom: \u{266a}.lua\" note.lua";
    let now = text(&[OSARA_KEY, latin, beyond], "\r\n");
    let resource = keymap_replaced(dir.path(), &old, now.as_bytes());

    let report = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();

    assert_eq!(scripts_added(&report), Some(2));
    let mut expected = old.clone();
    expected.extend_from_slice(b"SCR 4 0 RSgross \"Custom: Gr\xf6\xdfe.lua\" g.lua\r\n");
    expected.extend_from_slice(b"SCR 4 0 RSnote \"Custom: ?.lua\" note.lua\r\n");
    assert_eq!(fs::read(resource.join(KB)).unwrap(), expected);
}

/// The key map with the scripts kept is written to a temporary file that
/// then takes the key map's place, so the key map is never part of a file.
/// When the key map with the lines can't take its place, the one put back
/// is still whole, and the restore puts the key map it replaced back. When
/// that fails too, the key map stays exactly as the backup has it, which
/// the report says, and the before-restore set keeps the only copy of the
/// one it replaced.
#[test]
fn a_key_map_is_whole_whenever_keeping_its_scripts_fails() {
    for put_back_fails in [false, true] {
        let dir = tempdir().unwrap();
        let old = text(&[OWN_KEY], "\r\n");
        let now = text(&[OSARA_KEY, NEW_SCRIPT], "\r\n");
        let resource = keymap_replaced(dir.path(), old.as_bytes(), now.as_bytes());
        let keymap = resource.join(KB);
        let temporary = temporary_restore_path(&keymap);
        // What the key map and the temporary file hold when the second is
        // about to take the first's place.
        let seen: Rc<RefCell<Vec<String>>> = Rc::default();
        let _hook = {
            let (keymap, temporary, seen) = (keymap.clone(), temporary.clone(), Rc::clone(&seen));
            restore_seam::install(move |step, path| match step {
                Step::ReplacingWithScripts if path == keymap => {
                    *seen.borrow_mut() = vec![
                        fs::read_to_string(&keymap).unwrap(),
                        fs::read_to_string(&temporary).unwrap(),
                    ];
                    Err(io::Error::other("the disk is full"))
                }
                Step::PuttingBack if put_back_fails => Err(io::Error::other("still full")),
                _ => Ok(()),
            })
        };

        let report = restore(&resource, &RestoreItem::keymap(SET), &APPLY).unwrap();

        let case = format!("put back fails: {put_back_fails}");
        assert_eq!(
            *seen.borrow(),
            [old.clone(), format!("{old}{NEW_SCRIPT}\r\n")],
            "{case}"
        );
        assert!(fs::symlink_metadata(&temporary).is_err(), "{case}");
        assert_eq!(scripts_added(&report), None, "{case}");
        if put_back_fails {
            assert_eq!(report.status, RestoreStatus::FilesLeft, "{case}");
            assert_eq!(report.actions[0].action, RestoreBackupActionKind::Left);
            assert_eq!(fs::read(&keymap).unwrap(), old.as_bytes(), "{case}");
            let kept = report.before_restore_set_id.clone().unwrap();
            assert_eq!(
                fs::read(set_path(&resource, &kept).join(KB)).unwrap(),
                now.as_bytes(),
                "{case}"
            );
        } else {
            assert_eq!(report.status, RestoreStatus::NotRestored, "{case}");
            assert_eq!(report.actions[0].action, RestoreBackupActionKind::PutBack);
            assert_eq!(fs::read(&keymap).unwrap(), now.as_bytes(), "{case}");
            assert_eq!(sets(&resource), [SET], "{case}");
        }
    }
}

/// When RABBIT's records can't be saved after the key map got its scripts,
/// the restore is all or nothing as ever: the key map goes back as it was
/// before the restore, byte for byte, and the package's file goes again.
#[test]
fn a_key_map_with_its_scripts_goes_back_when_the_records_fail() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let keymap = resource.join(KB);
    let old = text(&[OWN_KEY], "\n");
    write(&keymap, old.as_bytes());
    let id = "uninstall-unix-500";
    let plugin = resource.join("UserPlugins").join("reaper_sws-x64.dll");
    write(&plugin, b"sws");
    let mut writer = BackupSetWriter::new(&resource, id, "uninstall");
    writer
        .record_package(
            "sws",
            BackupPackageRecord {
                receipt: None,
                langpack_before: None,
                with_settings: None,
            },
        )
        .unwrap();
    writer.move_file(Some("sws"), &plugin, None, false).unwrap();
    // The key map goes into the uninstall's set as a file of no package.
    writer.copy_file(None, &keymap).unwrap();
    let now = text(&[OSARA_KEY, NEW_SCRIPT], "\n");
    fs::write(&keymap, &now).unwrap();
    let _hook = restore_seam::install(|step, _| {
        if step == Step::SavingRecords {
            Err(io::Error::other("the disk is full"))
        } else {
            Ok(())
        }
    });

    let report = restore(&resource, &RestoreItem::whole_set(id), &APPLY).unwrap();

    assert_eq!(report.status, RestoreStatus::NotRestored, "{report:?}");
    assert!(report.keymap.is_some());
    assert_eq!(scripts_added(&report), None);
    assert_eq!(fs::read(&keymap).unwrap(), now.as_bytes());
    assert!(!plugin.exists());
    assert_eq!(sets(&resource), [id]);
}

/// A file that leads to the key map another way is the key map all the
/// same: through a link folder inside the resource folder, which a restore
/// allows, and, on Windows and on a Mac's volume that counts names in
/// another case as one, by a name in other case with `..` in the resource
/// folder's path. Its scripts are kept, and it never comes back while
/// REAPER may run.
#[test]
fn a_key_map_reached_another_way_is_the_key_map() {
    let dir = tempdir().unwrap();
    let old = text(&[OWN_KEY], "\n");
    let now = text(&[OSARA_KEY, NEW_SCRIPT], "\n");
    let resource = keymap_replaced(dir.path(), old.as_bytes(), now.as_bytes());
    link_folder(&resource.join("loop"), &resource);
    #[cfg_attr(not(any(windows, target_os = "macos")), allow(unused_mut))]
    let mut cases = vec![(resource.clone(), PathBuf::from("loop").join(KB))];
    #[cfg(any(windows, target_os = "macos"))]
    if super::restore_run_tests::names_in_another_case_are_one(dir.path()) {
        fs::create_dir_all(dir.path().join("other")).unwrap();
        cases.push((
            dir.path().join("other").join("..").join("REAPER"),
            PathBuf::from("REAPER-KB.INI"),
        ));
    }
    let set = set_path(&resource, SET);
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    let may_run = RestoreRunOptions {
        allow_reaper_running: true,
        ..APPLY
    };

    for (resource_path, original) in cases {
        let case = original.display().to_string();
        manifest.files[0].original_path = original;
        save_backup_manifest(&set, &manifest).unwrap();
        fs::write(resource.join(KB), &now).unwrap();
        let error = restore(&resource_path, &RestoreItem::whole_set(SET), &may_run).unwrap_err();
        assert_eq!(
            refusal(&error).code(),
            "keymap-needs-reaper-closed",
            "{case}"
        );

        let report = restore(&resource_path, &RestoreItem::whole_set(SET), &APPLY).unwrap();

        assert_eq!(report.status, RestoreStatus::Restored, "{case}");
        assert_eq!(scripts_added(&report), Some(1), "{case}");
        assert_eq!(
            fs::read_to_string(resource.join(KB)).unwrap(),
            format!("{old}{NEW_SCRIPT}\n"),
            "{case}"
        );
    }
}

/// The key map in an update's set, as older versions wrote one there, is a
/// backup of its own too: its row and a restore of it alone agree, and only
/// the key map comes back, with its scripts kept, while ReaPack and RABBIT's
/// records stay as they are. The whole set goes back package by package and
/// never puts its key map back, so it is not refused while REAPER may run.
#[test]
fn a_key_map_in_an_update_s_set_comes_back_alone() {
    use super::swap_tests::{NEW_ONLY, PLUGIN, SHARED, reapack_new, reapack_old, records_with};

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let keymap = resource.join(KB);
    let plugin = resource.join(PLUGIN);
    let old = text(&[OWN_KEY], "\r\n");
    write(&keymap, old.as_bytes());
    write(&plugin, b"reapack 1.2.4");
    write(&resource.join(SHARED), b"shared");
    let id = "unix-100";
    let mut writer = BackupSetWriter::new(&resource, id, UPDATE_BACKUP_REASON);
    writer
        .record_replaced(
            "reapack",
            ReplacedPackageRecord {
                receipt: Some(reapack_old()),
                replaced_by: Some(Version::parse("1.2.5").unwrap()),
            },
        )
        .unwrap();
    writer.copy_file(Some("reapack"), &plugin).unwrap();
    writer.copy_file(Some("osara"), &keymap).unwrap();
    write(&plugin, b"reapack 1.2.5");
    write(&resource.join(NEW_ONLY), b"new in 1.2.5");
    let now = text(&[OSARA_KEY, NEW_SCRIPT], "\r\n");
    fs::write(&keymap, &now).unwrap();
    save_install_state(&resource, &records_with(Some(reapack_new()))).unwrap();
    let records = load_install_state(&resource).unwrap();

    let row = keymap_row(&resource, id);
    assert_eq!(
        (row.status, row.restore_scope, row.refusal),
        (BackupStatus::CanRestore, Some(RestoreScope::Keymap), None)
    );
    let report = restore(&resource, &RestoreItem::keymap(id), &APPLY).unwrap();

    assert_eq!(report.status, RestoreStatus::Restored);
    assert_eq!(scripts_added(&report), Some(1));
    assert_eq!(
        fs::read_to_string(&keymap).unwrap(),
        format!("{old}{NEW_SCRIPT}\r\n")
    );
    assert_eq!(fs::read(&plugin).unwrap(), b"reapack 1.2.5");
    assert_eq!(load_install_state(&resource).unwrap(), records);

    let whole = restore(
        &resource,
        &RestoreItem::whole_set(id),
        &RestoreRunOptions {
            allow_reaper_running: true,
            ..APPLY
        },
    )
    .unwrap();

    assert_eq!(whole.status, RestoreStatus::Restored);
    assert_eq!(whole.keymap, None);
    assert_eq!(fs::read(&plugin).unwrap(), b"reapack 1.2.4");
    assert_eq!(
        fs::read_to_string(&keymap).unwrap(),
        format!("{old}{NEW_SCRIPT}\r\n")
    );
}

/// The key map and an earlier version of one update's set in one run, as
/// two ticked rows: the key map comes back alone, with its scripts kept,
/// then the earlier version by the swap, each with the fingerprint listed
/// and a before-restore set of its own; the set doesn't change.
#[test]
fn a_key_map_and_an_earlier_version_of_one_set_come_back_in_one_run() {
    use super::swap_tests::{NEW_ONLY, PLUGIN, SHARED, reapack_new, reapack_old, records_with};

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let keymap = resource.join(KB);
    let plugin = resource.join(PLUGIN);
    let old = text(&[OWN_KEY], "\r\n");
    write(&keymap, old.as_bytes());
    write(&plugin, b"reapack 1.2.4");
    write(&resource.join(SHARED), b"shared");
    let id = "unix-100";
    let mut writer = BackupSetWriter::new(&resource, id, UPDATE_BACKUP_REASON);
    writer
        .record_replaced(
            "reapack",
            ReplacedPackageRecord {
                receipt: Some(reapack_old()),
                replaced_by: Some(Version::parse("1.2.5").unwrap()),
            },
        )
        .unwrap();
    writer.copy_file(Some("reapack"), &plugin).unwrap();
    writer.copy_file(Some("osara"), &keymap).unwrap();
    drop(writer);
    write(&plugin, b"reapack 1.2.5");
    write(&resource.join(NEW_ONLY), b"new in 1.2.5");
    fs::write(&keymap, text(&[OSARA_KEY, NEW_SCRIPT], "\r\n")).unwrap();
    save_install_state(&resource, &records_with(Some(reapack_new()))).unwrap();
    let set = set_path(&resource, id);
    let set_before = snapshot(&set);
    let listed = crate::backup_overview::overview_with(&resource, &BackupEnvironment::AS_USER)
        .unwrap()
        .sets
        .into_iter()
        .find(|listed| listed.id == id)
        .and_then(|listed| listed.fingerprint);
    assert!(listed.is_some());

    let report = restore_backups_with(
        &resource,
        &[
            RestoreItem {
                fingerprint: listed.clone(),
                ..RestoreItem::keymap(id)
            },
            RestoreItem {
                fingerprint: listed,
                ..RestoreItem::package(id, "reapack")
            },
        ],
        &APPLY,
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap();

    let statuses: Vec<RestoreStatus> = report.backups.iter().map(|item| item.status).collect();
    assert_eq!(
        statuses,
        [RestoreStatus::Restored, RestoreStatus::Restored],
        "{report:#?}"
    );
    assert!(report.backups[0].keymap_only);
    assert_eq!(
        report.backups[0]
            .keymap
            .as_ref()
            .and_then(|keymap| keymap.scripts_added),
        Some(1)
    );
    assert_ne!(
        report.backups[0].before_restore_set_id,
        report.backups[1].before_restore_set_id
    );
    assert_eq!(
        fs::read_to_string(&keymap).unwrap(),
        format!("{old}{NEW_SCRIPT}\r\n")
    );
    assert_eq!(fs::read(&plugin).unwrap(), b"reapack 1.2.4");
    let state = load_install_state(&resource).unwrap().unwrap();
    assert_eq!(
        state.packages["reapack"].version,
        Some(Version::parse("1.2.4").unwrap())
    );
    assert_eq!(snapshot(&set), set_before);
}

/// A file of an earlier version that is the key map by its real place,
/// reached through a link folder inside the resource folder, as only a
/// manifest RABBIT didn't write can name it, is refused: a version's files
/// never include the key map, which comes back only as a key map of its
/// own, with its scripts kept. One package or the whole set, in a dry run
/// and for real, with REAPER allowed to run or not; nothing changes.
#[test]
fn a_version_file_that_is_the_key_map_is_refused() {
    use super::swap_tests::{NEW_ONLY, PLUGIN, SHARED, reapack_new, reapack_old, records_with};

    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let keymap = resource.join(KB);
    let plugin = resource.join(PLUGIN);
    write(&keymap, text(&[OWN_KEY], "\r\n").as_bytes());
    write(&plugin, b"reapack 1.2.4");
    write(&resource.join(SHARED), b"shared");
    let id = "unix-100";
    let mut writer = BackupSetWriter::new(&resource, id, UPDATE_BACKUP_REASON);
    writer
        .record_replaced(
            "reapack",
            ReplacedPackageRecord {
                receipt: Some(reapack_old()),
                replaced_by: Some(Version::parse("1.2.5").unwrap()),
            },
        )
        .unwrap();
    writer.copy_file(Some("reapack"), &plugin).unwrap();
    writer.copy_file(Some("reapack"), &keymap).unwrap();
    write(&plugin, b"reapack 1.2.5");
    write(&resource.join(NEW_ONLY), b"new in 1.2.5");
    fs::write(&keymap, text(&[OSARA_KEY, NEW_SCRIPT], "\r\n")).unwrap();
    save_install_state(&resource, &records_with(Some(reapack_new()))).unwrap();
    link_folder(&resource.join("loop"), &resource);
    let set = set_path(&resource, id);
    let mut manifest = load_backup_manifest(&set).unwrap().unwrap();
    manifest
        .files
        .iter_mut()
        .find(|entry| entry.original_path.file_name() == Some(KB.as_ref()))
        .unwrap()
        .original_path = PathBuf::from("loop").join(KB);
    save_backup_manifest(&set, &manifest).unwrap();
    let before = snapshot(dir.path());

    let mut locked = false;
    for item in [
        RestoreItem::package(id, "reapack"),
        RestoreItem::whole_set(id),
    ] {
        for allow_reaper_running in [false, true] {
            for dry_run in [true, false] {
                let options = RestoreRunOptions {
                    dry_run,
                    quick: false,
                    allow_reaper_running,
                };
                let error = restore(&resource, &item, &options).unwrap_err();
                let case = format!("{item:?} {options:?}");
                let RestoreRefusal::TargetRefused { path, reason, .. } = refusal(&error) else {
                    panic!("{case}: {error:?}");
                };
                assert_eq!(*reason, TargetRefusal::KeymapAmongVersionFiles, "{case}");
                assert!(path.ends_with(PathBuf::from("loop").join(KB)), "{case}");
                // A dry run takes no lock, so until a real run took one,
                // not even its folder is there.
                locked |= !dry_run;
                if !locked {
                    assert_eq!(snapshot(dir.path()), before, "{case}");
                }
            }
        }
    }
    // Refused under the lock, a real run leaves only the lock's empty
    // folder.
    assert_eq!(snapshot_but_lock_folder(dir.path(), &resource), before);
}

/// A set without a manifest, restored whole, keeps the scripts the same way
/// when its layout puts the key map back, in a dry run and for real.
#[test]
fn a_set_without_a_manifest_keeps_the_scripts_too() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    let old = text(&[OWN_KEY], "\n");
    let now = text(&[OSARA_KEY, NEW_SCRIPT], "\n");
    write(&resource.join(KB), now.as_bytes());
    write(&set_path(&resource, "unix-300").join(KB), old.as_bytes());

    let planned = restore(&resource, &RestoreItem::whole_set("unix-300"), &DRY_RUN).unwrap();
    assert_eq!(scripts_added(&planned), Some(1));
    let report = restore(&resource, &RestoreItem::whole_set("unix-300"), &APPLY).unwrap();

    assert_eq!(report.status, RestoreStatus::Restored);
    assert_eq!(scripts_added(&report), Some(1));
    assert_eq!(
        fs::read_to_string(resource.join(KB)).unwrap(),
        format!("{old}{NEW_SCRIPT}\n")
    );
}

/// A key map never comes back while REAPER may run: REAPER may save its
/// own key map when it quits, over the one restored. So a restore told
/// REAPER may run is refused, before anything changes and in a dry run
/// too, for the key map alone and for a whole set that puts it back, with
/// a manifest or without. In a run of several backups, only the one that
/// puts the key map back is refused; the others come back.
#[test]
fn a_key_map_is_never_restored_while_reaper_may_run() {
    let dir = tempdir().unwrap();
    let resource = keymap_replaced(
        dir.path(),
        text(&[OWN_KEY], "\r\n").as_bytes(),
        text(&[OSARA_KEY], "\r\n").as_bytes(),
    );
    let plugin = resource.join("UserPlugins").join("reaper_osara64.dll");
    write(&plugin, b"osara");
    let with_plugin = "restore-unix-200-before-restore";
    let mut writer = BackupSetWriter::new(&resource, with_plugin, BEFORE_RESTORE_REASON);
    writer.copy_file(Some("osara"), &resource.join(KB)).unwrap();
    writer.copy_file(Some("osara"), &plugin).unwrap();
    write(
        &set_path(&resource, "unix-300").join(KB),
        text(&[OWN_KEY], "\r\n").as_bytes(),
    );
    let may_run = |dry_run| RestoreRunOptions {
        dry_run,
        quick: false,
        allow_reaper_running: true,
    };
    let before = snapshot(dir.path());

    for item in [
        RestoreItem::keymap(SET),
        RestoreItem::whole_set(SET),
        RestoreItem::keymap(with_plugin),
        RestoreItem::whole_set(with_plugin),
        RestoreItem::whole_set("unix-300"),
    ] {
        for dry_run in [true, false] {
            let error = restore_backup_item_with(
                &resource,
                &item,
                &may_run(dry_run),
                &BackupEnvironment::AS_USER,
                || panic!("a refused key map looked up where outside files go"),
            )
            .unwrap_err();
            assert_eq!(
                refusal(&error),
                &RestoreRefusal::KeymapNeedsReaperClosed {
                    backup_id: item.set_id.clone()
                },
                "{item:?}"
            );
            let words = error.to_string();
            assert!(
                !words.contains("--") && !words.contains("rabbit "),
                "{words}"
            );
        }
    }
    assert_eq!(snapshot(dir.path()), before);

    let sws = sws_uninstalled(&resource, "uninstall-unix-400");
    let report = restore_backups_with(
        &resource,
        &[
            RestoreItem::keymap(SET),
            RestoreItem::package("uninstall-unix-400", "sws"),
        ],
        &may_run(false),
        &BackupEnvironment::AS_USER,
        OutsideRestoreRules::default,
        &ProgressReporter::noop(),
        &CancelToken::new(),
    )
    .unwrap();

    let statuses: Vec<RestoreStatus> = report.backups.iter().map(|item| item.status).collect();
    assert_eq!(
        statuses,
        [RestoreStatus::NotRestored, RestoreStatus::Restored]
    );
    assert_eq!(
        report.backups[0].refusal,
        Some(RestoreRefusal::KeymapNeedsReaperClosed {
            backup_id: SET.to_string()
        })
    );
    assert!(report.backups[0].keymap_only && !report.backups[1].keymap_only);
    assert_eq!(
        fs::read_to_string(resource.join(KB)).unwrap(),
        text(&[OSARA_KEY], "\r\n")
    );
    assert_eq!(fs::read(&sws).unwrap(), b"sws");
}

/// Without being told REAPER may run, a key map restore checks the target's
/// REAPER as every restore does: a real one is refused while it runs, with
/// nothing changed, and a dry run warns that it runs and refuses nothing.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn a_running_reaper_refuses_a_key_map_restore_as_any_restore() {
    let dir = tempdir().unwrap();
    let resource = keymap_replaced(
        dir.path(),
        text(&[OWN_KEY], "\r\n").as_bytes(),
        text(&[OSARA_KEY], "\r\n").as_bytes(),
    );
    let target = super::tests::targets_reaper(&resource);
    let running = move || vec![target.clone()];
    let environment = BackupEnvironment {
        reaper_processes: &running,
        ..BackupEnvironment::AS_USER
    };
    let before = snapshot(dir.path());
    let restore = |options: &RestoreRunOptions| {
        restore_backup_item_with(
            &resource,
            &RestoreItem::keymap(SET),
            options,
            &environment,
            OutsideRestoreRules::default,
        )
    };

    let error = restore(&APPLY).unwrap_err();
    assert_eq!(refusal(&error).code(), "reaper-running");
    // The refusal says the key map comes back, since letting REAPER run
    // is refused for it: nobody is told to try that.
    assert!(
        matches!(
            refusal(&error),
            RestoreRefusal::ReaperRunning {
                puts_keymap_back: true,
                ..
            }
        ),
        "{error:?}"
    );
    let planned = restore(&DRY_RUN).unwrap();
    assert_eq!(planned.status, RestoreStatus::WouldRestore);
    assert!(planned.preflight.passed);
    assert_eq!(
        super::tests::check_named(&planned, crate::preflight::REAPER_PROCESS_CHECK).status,
        crate::preflight::PreflightStatus::Warn
    );
    assert_eq!(snapshot(dir.path()), before);

    // A whole set that puts the key map back says so too, one without a
    // manifest included, also among other backups of a run; a restore that
    // leaves the key map alone doesn't.
    write(
        &set_path(&resource, "unix-300").join(KB),
        text(&[OWN_KEY], "\r\n").as_bytes(),
    );
    sws_uninstalled(&resource, "uninstall-unix-400");
    let before = snapshot(dir.path());
    let keymap_back = |items: &[RestoreItem]| {
        let error = restore_backups_with(
            &resource,
            items,
            &APPLY,
            &environment,
            OutsideRestoreRules::default,
            &ProgressReporter::noop(),
            &CancelToken::new(),
        )
        .unwrap_err();
        match refusal(&error) {
            RestoreRefusal::ReaperRunning {
                every_reaper_for,
                puts_keymap_back,
                ..
            } if every_reaper_for.is_empty() => *puts_keymap_back,
            other => panic!("{other:?}"),
        }
    };
    assert!(keymap_back(&[RestoreItem::whole_set(SET)]));
    assert!(keymap_back(&[RestoreItem::whole_set("unix-300")]));
    assert!(keymap_back(&[
        RestoreItem::package("uninstall-unix-400", "sws"),
        RestoreItem::keymap(SET),
    ]));
    assert!(!keymap_back(&[RestoreItem::package(
        "uninstall-unix-400",
        "sws"
    )]));
    assert_eq!(snapshot(dir.path()), before);
}

/// A key map's backup and a restore of the key map alone give the same
/// refusal, code, fields and all, in a dry run and for real: another
/// account's set, a finished copy that is gone, a copy that never
/// finished, and two copies for the one place. A set with no key map
/// refuses the key map alone. Nothing changes.
#[test]
fn key_map_rows_and_runs_give_equal_refusals() {
    let dir = tempdir().unwrap();
    let resource = dir.path().join("REAPER");
    write(&resource.join(KB), b"keys now");
    let keymap_set = |id: &str| {
        BackupSetWriter::new(&resource, id, KEYMAP_BACKUP_REASON)
            .copy_file(Some("osara"), &resource.join(KB))
            .unwrap();
        set_path(&resource, id)
    };
    let theirs = keymap_set("unattended-unix-100-000000000");
    let gone = keymap_set("unattended-unix-200-000000000");
    fs::remove_file(gone.join(KB)).unwrap();
    let unfinished = keymap_set("unattended-unix-300-000000000");
    let mut manifest = load_backup_manifest(&unfinished).unwrap().unwrap();
    manifest.files[0].state = BackupEntryState::Pending;
    manifest.files[0].sha256 = sha256_bytes(b"what it never finished");
    save_backup_manifest(&unfinished, &manifest).unwrap();
    fs::write(unfinished.join(KB), b"half").unwrap();
    let twice = keymap_set("unattended-unix-400-000000000");
    let mut manifest = load_backup_manifest(&twice).unwrap().unwrap();
    manifest.files.push(manifest.files[0].clone());
    save_backup_manifest(&twice, &manifest).unwrap();
    sws_uninstalled(&resource, "uninstall-unix-500");
    let owner_of = |path: &Path| Some(if path == theirs { 0 } else { 501 });
    let environment = BackupEnvironment {
        owner_of: &owner_of,
        ..BackupEnvironment::AS_USER
    };

    let listed = overview_with(&resource, &environment).unwrap();

    let before = snapshot(dir.path());
    let run = |id: &str| {
        let mut refusals = Vec::new();
        for dry_run in [true, false] {
            let error = restore_backup_item_with(
                &resource,
                &RestoreItem::keymap(id),
                &RestoreRunOptions { dry_run, ..APPLY },
                &environment,
                OutsideRestoreRules::default,
            )
            .unwrap_err();
            refusals.push(refusal(&error).clone());
        }
        assert_eq!(refusals[0], refusals[1], "{id}: the dry run said otherwise");
        refusals.pop().unwrap()
    };
    for (id, status, code) in [
        (
            "unattended-unix-100-000000000",
            BackupStatus::NotYours,
            "not-yours",
        ),
        (
            "unattended-unix-200-000000000",
            BackupStatus::PartMissing,
            "part-missing",
        ),
        (
            "unattended-unix-300-000000000",
            BackupStatus::NothingInIt,
            "no-keymap-in-set",
        ),
        (
            "unattended-unix-400-000000000",
            BackupStatus::CanRestore,
            "target-refused",
        ),
    ] {
        let row = listed
            .entries
            .iter()
            .find(|entry| {
                entry.item
                    == BackupItem::Keymap {
                        set_id: id.to_string(),
                    }
            })
            .unwrap();
        assert_eq!((row.status, row.restore_scope), (status, None), "{id}");
        let refused = run(id);
        assert_eq!(refused.code(), code, "{id}");
        assert_eq!(row.refusal.as_ref(), Some(&refused), "{id}");
    }
    assert_eq!(
        run("uninstall-unix-500"),
        RestoreRefusal::NoKeymapInSet {
            backup_id: "uninstall-unix-500".to_string()
        }
    );
    // A place refused under the lock leaves only the lock's empty folder.
    assert_eq!(snapshot_but_lock_folder(dir.path(), &resource), before);
}
