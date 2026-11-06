//! Entries in Windows' per-user list of installed programs that a removal
//! would leave pointing at nothing.
//!
//! OSARA's installer writes `HKCU\...\Uninstall\OSARA`, whose
//! `UninstallString` names the uninstaller in the resource folder's `osara`
//! folder. Once RABBIT has moved that folder's files into a backup set, the
//! entry points at a file that is gone, so it goes too. But the list has one
//! OSARA entry for the whole computer, written by whichever OSARA install
//! came last, so it goes only while the program it names sits in this
//! REAPER's folder. One that points anywhere else, such as into the standard
//! REAPER while a portable one is uninstalled, belongs to that REAPER and
//! stays.
//!
//! The entry is read right before it would go, never while the plan is
//! built: the wizard builds its plan on the UI thread, and a plan should
//! say the same wherever it is built.

use std::path::Path;

use rabbit_platform::HkcuUninstallRoot;

use super::paths::same_folder;
use super::{PlannedRegistryCleanup, RegistryEntryAction, RegistryEntryOutcome, UninstallProblem};

/// Carry out `cleanups` in `list`, or with `remove` false only say what
/// would happen. An entry that can't be read or removed is a problem and
/// stays; nothing here undoes the removal of the files.
pub(super) fn clean_registry_entries(
    list: &HkcuUninstallRoot,
    cleanups: &[PlannedRegistryCleanup],
    remove: bool,
) -> (Vec<RegistryEntryOutcome>, Vec<UninstallProblem>) {
    let mut outcomes = Vec::new();
    let mut problems = Vec::new();
    for cleanup in cleanups {
        let key = format!("HKCU\\{}\\{}", list.path(), cleanup.key);
        let entry = match list.read_entry(&cleanup.key) {
            Ok(Some(entry)) => entry,
            Ok(None) => {
                outcomes.push(RegistryEntryOutcome {
                    key,
                    action: RegistryEntryAction::NotFound,
                });
                continue;
            }
            Err(error) => {
                problems.push(UninstallProblem::RegistryEntryUnreadable {
                    key,
                    error: error.to_string(),
                });
                continue;
            }
        };
        let found = entry.string_value(&cleanup.value);
        if !found
            .as_deref()
            .is_some_and(|value| points_into(value, &cleanup.dir))
        {
            outcomes.push(RegistryEntryOutcome {
                key,
                action: RegistryEntryAction::PointsElsewhere {
                    found,
                    expected: cleanup.dir.clone(),
                },
            });
            continue;
        }
        if !remove {
            outcomes.push(RegistryEntryOutcome {
                key,
                action: RegistryEntryAction::WouldRemove,
            });
            continue;
        }
        match list.delete_entry(&cleanup.key) {
            Ok(true) => outcomes.push(RegistryEntryOutcome {
                key,
                action: RegistryEntryAction::Removed,
            }),
            // Gone between the read and the delete: nothing left to do.
            Ok(false) => outcomes.push(RegistryEntryOutcome {
                key,
                action: RegistryEntryAction::NotFound,
            }),
            Err(error) => problems.push(UninstallProblem::RegistryEntryNotRemoved {
                key,
                error: error.to_string(),
            }),
        }
    }
    (outcomes, problems)
}

/// Whether the program `value` names sits directly in `dir`. A value whose
/// program RABBIT can't pin down, or names by a relative path, points
/// nowhere RABBIT can vouch for. OSARA's install uses it too, to tell the
/// entry its installer wrote into a portable REAPER from anyone else's.
pub(crate) fn points_into(value: &str, dir: &Path) -> bool {
    let Some(program) = program_named_by(value) else {
        return false;
    };
    let program = Path::new(program);
    program.is_absolute()
        && program
            .parent()
            .is_some_and(|folder| same_folder(folder, dir))
}

/// The program a command line such as an `UninstallString` names, the way
/// Windows reads it: the text between the first pair of quotes, or, without
/// quotes, everything up to the first `.exe` that ends a word, so the
/// arguments after it drop away. `None` when RABBIT can't tell where the
/// program's name ends: an unclosed quote, or no `.exe` in a value that
/// holds a space.
pub(crate) fn program_named_by(value: &str) -> Option<&str> {
    let value = value.trim();
    if let Some(quoted) = value.strip_prefix('"') {
        let (program, _) = quoted.split_once('"')?;
        return (!program.is_empty()).then_some(program);
    }
    // Lowercasing only ASCII keeps every byte where it was, so an offset in
    // `lower` is the same offset in `value`.
    let lower = value.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find(".exe") {
        let end = from + at + ".exe".len();
        if lower[end..].chars().next().is_none_or(char::is_whitespace) {
            return Some(&value[..end]);
        }
        from = end;
    }
    (!value.is_empty() && !value.contains(char::is_whitespace)).then_some(value)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{points_into, program_named_by};

    #[test]
    fn the_program_is_what_a_command_line_starts_with() {
        for (value, program) in [
            (
                r#""C:\REAPER\osara\uninstall.exe""#,
                Some(r"C:\REAPER\osara\uninstall.exe"),
            ),
            (
                r#"  "C:\My REAPER\osara\uninstall.exe" /S  "#,
                Some(r"C:\My REAPER\osara\uninstall.exe"),
            ),
            (
                r"C:\REAPER\osara\uninstall.exe",
                Some(r"C:\REAPER\osara\uninstall.exe"),
            ),
            (
                r"C:\My REAPER\osara\Uninstall.EXE /S _?=C:\My REAPER",
                Some(r"C:\My REAPER\osara\Uninstall.EXE"),
            ),
            // `.exe` inside a folder's name doesn't end the program.
            (
                r"C:\tools.exe.d\osara\uninstall.exe /S",
                Some(r"C:\tools.exe.d\osara\uninstall.exe"),
            ),
            (
                r"C:\REAPER\osara\uninstall",
                Some(r"C:\REAPER\osara\uninstall"),
            ),
            (r#""C:\REAPER\osara\uninstall.exe"#, None),
            (r#""""#, None),
            (r"C:\My REAPER\osara\uninstall /S", None),
            ("", None),
            ("   ", None),
        ] {
            assert_eq!(program_named_by(value), program, "{value:?}");
        }
    }

    #[test]
    fn an_entry_points_into_a_folder_only_with_its_program_right_there() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        let osara = resource.join("osara");
        fs::create_dir_all(&osara).unwrap();
        let quoted = |path: &std::path::Path| format!("\"{}\"", path.display());

        assert!(points_into(&quoted(&osara.join("uninstall.exe")), &osara));
        assert!(points_into(
            &format!("{} /S", osara.join("uninstall.exe").display()),
            &osara
        ));
        // Another REAPER's OSARA, and nearby folders that aren't this one.
        let other = dir.path().join("Other REAPER").join("osara");
        assert!(!points_into(&quoted(&other.join("uninstall.exe")), &osara));
        assert!(!points_into(
            &quoted(&resource.join("uninstall.exe")),
            &osara
        ));
        assert!(!points_into(
            &quoted(&osara.join("locale").join("uninstall.exe")),
            &osara
        ));
        // A relative program could be anywhere.
        assert!(!points_into(r#""osara\uninstall.exe""#, &osara));
        assert!(!points_into("", &osara));
    }

    /// The folder and the uninstaller are gone by the time the entry is
    /// read, and the value may spell the folder differently from RABBIT.
    #[test]
    fn a_folder_that_is_gone_still_matches_however_it_is_spelt() {
        let dir = tempdir().unwrap();
        let resource = dir.path().join("REAPER");
        fs::create_dir_all(&resource).unwrap();
        let osara = resource.join("osara");
        let program = osara.join("uninstall.exe");

        assert!(points_into(&format!("\"{}\"", program.display()), &osara));
        #[cfg(windows)]
        {
            let shouted = program.display().to_string().to_uppercase();
            assert!(points_into(&format!("\"{shouted}\""), &osara));
            let forward = program.display().to_string().replace('\\', "/");
            assert!(points_into(&forward, &osara));
            // Windows' canonical spelling, `\\?\C:\...`.
            let verbatim = fs::canonicalize(&resource)
                .unwrap()
                .join("osara")
                .join("uninstall.exe");
            assert!(points_into(&format!("\"{}\"", verbatim.display()), &osara));
        }
    }

    /// A resource folder reached through a junction is the same folder as
    /// its target, as the guard sees it.
    #[cfg(windows)]
    #[test]
    fn a_resource_folder_reached_through_a_junction_still_matches() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("Real REAPER");
        fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("Linked REAPER");
        crate::uninstall::paths::make_junction(&link, &real);

        let program = format!("\"{}\"", real.join("osara").join("uninstall.exe").display());
        assert!(points_into(&program, &link.join("osara")));
        assert!(!points_into(
            &program,
            &dir.path().join("Elsewhere").join("osara")
        ));
    }
}
