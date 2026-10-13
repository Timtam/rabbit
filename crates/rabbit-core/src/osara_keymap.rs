//! Whether replacing `reaper-kb.ini` with the OSARA key map would lose
//! anything of the user's.
//!
//! OSARA's installer copies OSARA's key map to `KeyMaps/OSARA.ReaperKeyMap`
//! on every install (RABBIT's macOS install does the same), and replacing
//! the key map copies it from there. Until an update overwrites that file,
//! it is the key map of the OSARA that's installed now. A `reaper-kb.ini`
//! that matches it holds nothing of the user's, so the new OSARA key map can
//! replace it. One that doesn't carries the user's own changes, and OSARA's
//! "Merge OSARA key map" action is the way to bring the new bindings in.
//!
//! The comparison is by meaning, not bytes:
//! - Script registrations (`SCR`) are left out. ReaPack adds them whenever
//!   it installs a script, and the replacement keeps them anyway.
//! - Key bindings (`KEY`) compare by modifiers, key, section and action. The
//!   `# Main : Ctrl+X : …` comment after them is REAPER's description of the
//!   binding, not part of it.
//! - Custom actions (`ACT`) and anything else compare as whole lines.
//! - Order doesn't matter.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::detection::detect_components;
use crate::model::Platform;
use crate::package::PACKAGE_OSARA;
use crate::text_file::read_text_file_lossless;

/// How the key map REAPER uses compares with the installed OSARA key map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OsaraKeymapState {
    /// No `reaper-kb.ini`, or one without key bindings or custom actions:
    /// REAPER's default keys.
    DefaultKeys,
    /// `reaper-kb.ini` is the installed OSARA key map, apart from script
    /// registrations.
    UnchangedOsara,
    /// `reaper-kb.ini` differs from the installed OSARA key map.
    OwnChanges,
    /// There is a key map, but no OSARA key map to compare it with, or one
    /// of the two couldn't be read.
    Unknown,
}

/// The key map choice a run makes when the user doesn't make one, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsaraKeymapDefault {
    pub osara_installed: bool,
    pub state: OsaraKeymapState,
    pub replace: bool,
}

pub fn current_keymap_path(resource_path: &Path) -> PathBuf {
    resource_path.join("reaper-kb.ini")
}

pub fn osara_keymap_path(resource_path: &Path) -> PathBuf {
    resource_path.join("KeyMaps").join("OSARA.ReaperKeyMap")
}

/// Compare `reaper-kb.ini` with the OSARA key map next to it. Run it before
/// OSARA is installed: the install overwrites the OSARA key map with the new
/// version's.
pub fn osara_keymap_state(resource_path: &Path) -> OsaraKeymapState {
    let current_path = current_keymap_path(resource_path);
    if !current_path.exists() {
        return OsaraKeymapState::DefaultKeys;
    }
    let Ok(current) = read_text_file_lossless(&current_path) else {
        return OsaraKeymapState::Unknown;
    };
    let current = KeymapRecords::parse(&current.text);
    if current.is_empty() {
        return OsaraKeymapState::DefaultKeys;
    }
    let Ok(osara) = read_text_file_lossless(&osara_keymap_path(resource_path)) else {
        return OsaraKeymapState::Unknown;
    };
    if current == KeymapRecords::parse(&osara.text) {
        OsaraKeymapState::UnchangedOsara
    } else {
        OsaraKeymapState::OwnChanges
    }
}

/// Whether a run replaces the key map unless told otherwise. A new OSARA
/// brings its key map along. Once OSARA is there, only a key map that is
/// still OSARA's own gets replaced: any other would lose the user's changes,
/// or swap REAPER's default keys they chose to keep for OSARA's.
pub fn replace_by_default(osara_installed: bool, state: OsaraKeymapState) -> bool {
    !osara_installed || state == OsaraKeymapState::UnchangedOsara
}

/// [`replace_by_default`] for a resource folder, finding out whether OSARA
/// is installed there. When that can't be told, OSARA counts as new, the
/// choice RABBIT always made before it compared key maps.
pub fn osara_keymap_default(resource_path: &Path, platform: Platform) -> OsaraKeymapDefault {
    let osara_installed = detect_components(resource_path, platform)
        .map(|detections| {
            detections
                .iter()
                .any(|detection| detection.package_id == PACKAGE_OSARA && detection.installed)
        })
        .unwrap_or(false);
    let state = osara_keymap_state(resource_path);
    OsaraKeymapDefault {
        osara_installed,
        state,
        replace: replace_by_default(osara_installed, state),
    }
}

/// The section and id a script registration (`SCR flags section id "name"
/// path`) is known by, or `None` for any other line.
pub fn script_key(line: &str) -> Option<(&str, &str)> {
    let mut fields = line.split_whitespace();
    if fields.next() != Some("SCR") {
        return None;
    }
    let section = fields.nth(1)?;
    Some((section, fields.next()?))
}

/// A key map's bindings and custom actions, in a form two key maps can be
/// compared by.
#[derive(Debug, Default, PartialEq, Eq)]
struct KeymapRecords {
    /// (modifiers, key, section) → the action bound to it.
    keys: BTreeMap<(String, String, String), String>,
    /// Custom actions and any other line, with runs of whitespace made one
    /// space.
    other: BTreeSet<String>,
}

impl KeymapRecords {
    fn parse(text: &str) -> Self {
        let mut records = Self::default();
        for line in text.trim_start_matches('\u{feff}').lines() {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            match fields.as_slice() {
                [] | ["SCR", ..] => {}
                [first, ..] if first.starts_with('#') => {}
                ["KEY", modifiers, key, action, section, ..] => {
                    records.keys.insert(
                        (
                            (*modifiers).to_string(),
                            (*key).to_string(),
                            (*section).to_string(),
                        ),
                        (*action).to_string(),
                    );
                }
                _ => {
                    records.other.insert(fields.join(" "));
                }
            }
        }
        records
    }

    fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.other.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    const OSARA_MAP: &str = "ACT 0 0 \"_OSARA_a1\" \"Custom: OSARA thing\" 40001 40002\r\n\
        SCR 4 0 RS0SARA \"Custom: OSARA script.lua\" OSARA/script.lua\r\n\
        KEY 1 65 40001 0 # Main : A : Track: Insert new track\r\n\
        KEY 9 66 _OSARA_a1 0 # Main : Ctrl+B : Custom: OSARA thing\r\n\
        KEY 1 67 0 0 # Main : C : Disabled default\r\n";

    fn resource(current: Option<&str>, osara: Option<&str>) -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        if let Some(current) = current {
            fs::write(dir.path().join("reaper-kb.ini"), current).unwrap();
        }
        if let Some(osara) = osara {
            fs::create_dir_all(dir.path().join("KeyMaps")).unwrap();
            fs::write(dir.path().join("KeyMaps").join("OSARA.ReaperKeyMap"), osara).unwrap();
        }
        dir
    }

    fn state(current: Option<&str>, osara: Option<&str>) -> OsaraKeymapState {
        osara_keymap_state(resource(current, osara).path())
    }

    #[test]
    fn no_key_map_is_reaper_defaults() {
        assert_eq!(state(None, Some(OSARA_MAP)), OsaraKeymapState::DefaultKeys);
        assert_eq!(state(None, None), OsaraKeymapState::DefaultKeys);
        // Only registered scripts, no bindings: still REAPER's own keys.
        assert_eq!(
            state(
                Some("SCR 4 0 RSabc \"Custom: x.lua\" x.lua\r\n\r\n"),
                Some(OSARA_MAP)
            ),
            OsaraKeymapState::DefaultKeys
        );
        assert_eq!(state(Some(""), None), OsaraKeymapState::DefaultKeys);
    }

    #[test]
    fn the_osara_key_map_itself_is_unchanged() {
        assert_eq!(
            state(Some(OSARA_MAP), Some(OSARA_MAP)),
            OsaraKeymapState::UnchangedOsara
        );
    }

    #[test]
    fn scripts_order_comments_and_line_endings_are_not_changes() {
        // What REAPER and ReaPack leave behind on an untouched OSARA key map:
        // more scripts registered, lines in another order, LF endings, a
        // BOM, spacing, and the binding comments in REAPER's own words.
        let current = "\u{feff}KEY 1 67 0 0\n\
            KEY  9\t66 _OSARA_a1 0 # Principal : Ctrl+B : something else\n\
            SCR 4 0 RSabc \"Custom: mine.lua\" mine.lua\n\
            ACT 0 0 \"_OSARA_a1\"  \"Custom: OSARA thing\" 40001 40002\n\
            KEY 1 65 40001 0\n\
            SCR 4 0 RS0SARA \"Custom: OSARA script.lua\" OSARA/script.lua\n\
            SCR 4 0 RS0SARA \"Custom: OSARA script.lua\" OSARA/script.lua\n";
        assert_eq!(
            state(Some(current), Some(OSARA_MAP)),
            OsaraKeymapState::UnchangedOsara
        );
    }

    #[test]
    fn a_rebound_key_is_a_change() {
        let current = OSARA_MAP.replace("KEY 1 65 40001 0", "KEY 1 65 40044 0");
        assert_eq!(
            state(Some(&current), Some(OSARA_MAP)),
            OsaraKeymapState::OwnChanges
        );
    }

    #[test]
    fn an_added_or_removed_binding_is_a_change() {
        let added = format!("{OSARA_MAP}KEY 1 68 40100 0\r\n");
        assert_eq!(
            state(Some(&added), Some(OSARA_MAP)),
            OsaraKeymapState::OwnChanges
        );
        let removed = OSARA_MAP.replace("KEY 1 67 0 0 # Main : C : Disabled default\r\n", "");
        assert_eq!(
            state(Some(&removed), Some(OSARA_MAP)),
            OsaraKeymapState::OwnChanges
        );
        // The same key bound in another section is a binding of its own.
        let other_section = format!("{OSARA_MAP}KEY 1 65 40001 32060\r\n");
        assert_eq!(
            state(Some(&other_section), Some(OSARA_MAP)),
            OsaraKeymapState::OwnChanges
        );
    }

    #[test]
    fn a_new_or_edited_custom_action_is_a_change() {
        let added = format!("{OSARA_MAP}ACT 0 0 \"_mine\" \"Custom: mine\" 40001\r\n");
        assert_eq!(
            state(Some(&added), Some(OSARA_MAP)),
            OsaraKeymapState::OwnChanges
        );
        let edited = OSARA_MAP.replace("40001 40002", "40001 40003");
        assert_eq!(
            state(Some(&edited), Some(OSARA_MAP)),
            OsaraKeymapState::OwnChanges
        );
    }

    #[test]
    fn a_key_map_without_an_osara_one_to_compare_with_is_unknown() {
        assert_eq!(state(Some(OSARA_MAP), None), OsaraKeymapState::Unknown);
    }

    #[test]
    fn an_ansi_encoded_key_map_still_compares() {
        // Action names can carry ANSI code page bytes on Windows (issue #7).
        let mut current = OSARA_MAP.as_bytes().to_vec();
        current.extend_from_slice(b"SCR 4 0 RSdef \"Custom: Tom");
        current.push(0x92);
        current.extend_from_slice(b"s tool.lua\" tool.lua\r\n");
        let dir = resource(None, Some(OSARA_MAP));
        fs::write(dir.path().join("reaper-kb.ini"), current).unwrap();
        assert_eq!(
            osara_keymap_state(dir.path()),
            OsaraKeymapState::UnchangedOsara
        );
    }

    #[test]
    fn only_an_installed_osara_keeps_a_key_map_that_is_not_its_own() {
        use OsaraKeymapState::*;
        for state in [DefaultKeys, UnchangedOsara, OwnChanges, Unknown] {
            assert!(replace_by_default(false, state), "{state:?}");
        }
        assert!(replace_by_default(true, UnchangedOsara));
        assert!(!replace_by_default(true, DefaultKeys));
        assert!(!replace_by_default(true, OwnChanges));
        assert!(!replace_by_default(true, Unknown));
    }

    #[test]
    fn script_keys_come_from_registration_lines_only() {
        assert_eq!(
            script_key("SCR 4 0 RSabc \"Custom: x.lua\" x.lua"),
            Some(("0", "RSabc"))
        );
        assert_eq!(
            script_key("SCR\t260\t32060\tRSdef \"y\" y.lua"),
            Some(("32060", "RSdef"))
        );
        assert_eq!(script_key("SCR 4 0"), None);
        assert_eq!(script_key("KEY 1 65 40001 0"), None);
        assert_eq!(script_key("SCRIPT 1 2 3"), None);
    }
}
