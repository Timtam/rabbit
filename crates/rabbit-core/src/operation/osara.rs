use std::path::{Path, PathBuf};

use crate::artifact::ArtifactKind;
use crate::error::{IoPathContext, RabbitError, Result};
use crate::model::Platform;
use crate::package::PACKAGE_OSARA;
use crate::reapack::extract_scr_lines;
use crate::text_file::{read_text_file_lossless, write_text_file_lossless};

use super::{
    PackageAutomationSupport, PlannedAutomationKind, PlannedExecutionKind,
    PlannedExecutionOverride, UnattendedPostInstallReport, backup_file_for_unattended_change,
    replace_file_from_source, target_likely_portable,
};

pub(super) const TITLE: &str = "OSARA";

/// OSARA-specific automation routing. Today: Windows installer is unattended,
/// macOS archive is unattended via the OSARA-asset extractor.
pub(super) fn automation_support_for(
    kind: ArtifactKind,
    platform: Platform,
) -> Option<PackageAutomationSupport> {
    match (kind, platform) {
        (ArtifactKind::Installer, Platform::Windows) => Some(
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::VendorInstaller),
        ),
        (ArtifactKind::Archive, Platform::MacOs) => Some(
            PackageAutomationSupport::AvailableUnattended(PlannedAutomationKind::ArchiveExtraction),
        ),
        _ => None,
    }
}

/// OSARA-specific message variant used when the unattended path also applied
/// the key-map replacement step. Returns `None` when the replacement was not
/// requested (caller should fall back to the generic message). The pair
/// is (English text for the saved JSON report, structured code for the
/// localizable UI surface).
pub(super) fn unattended_install_message(
    replace_osara_keymap: bool,
    keymap_was_backed_up: bool,
) -> Option<(String, super::PackageOperationMessage)> {
    if !replace_osara_keymap {
        return None;
    }
    Some(if keymap_was_backed_up {
        (
            "RABBIT ran the upstream installer unattended, backed up reaper-kb.ini, applied the OSARA key map replacement, and updated the RABBIT receipt.".to_string(),
            super::PackageOperationMessage::OsaraUnattendedInstalledKeymapBackedUp,
        )
    } else {
        (
            "RABBIT ran the upstream installer unattended, applied the OSARA key map replacement, and updated the RABBIT receipt.".to_string(),
            super::PackageOperationMessage::OsaraUnattendedInstalledKeymapReplaced,
        )
    })
}

pub(super) fn manual_install_notes(
    resource_path: &Path,
    replace_osara_keymap: bool,
) -> Vec<String> {
    let mut notes = vec![
        "OSARA's Windows installer supports standard and portable REAPER targets; preserve an existing key map unless the user explicitly chooses replacement."
            .to_string(),
    ];
    if replace_osara_keymap {
        notes.push(format!(
            "The selected workflow replaces the current key map. Back up {} before replacing it with the OSARA key map.",
            resource_path.join("reaper-kb.ini").display()
        ));
    } else {
        notes.push(format!(
            "The selected workflow preserves the current key map. Leave {} unchanged.",
            resource_path.join("reaper-kb.ini").display()
        ));
    }
    notes
}

/// Files installed by OSARA that the receipt should reference. Filtered to
/// the on-disk existing ones after the unattended run.
pub(super) fn receipt_paths(resource_path: &Path, replace_osara_keymap: bool) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let keymap_path = resource_path.join("KeyMaps").join("OSARA.ReaperKeyMap");
    if keymap_path.exists() {
        paths.push(keymap_path);
    }
    let support_dir = resource_path.join("osara");
    if support_dir.exists() {
        paths.push(support_dir);
    }
    if replace_osara_keymap {
        let current_keymap = resource_path.join("reaper-kb.ini");
        if current_keymap.exists() {
            paths.push(current_keymap);
        }
    }
    paths
}

/// Post-install fixups specific to OSARA: clean up the portable
/// uninstaller stub on Windows and apply the key-map replacement when the
/// user opted into it.
pub(super) fn post_install_unattended(
    resource_path: &Path,
    platform: Platform,
    target_app_path: Option<&Path>,
    replace_osara_keymap: bool,
) -> Result<UnattendedPostInstallReport> {
    let mut report = UnattendedPostInstallReport::default();
    if matches!(platform, Platform::Windows)
        && target_likely_portable(resource_path, target_app_path)
    {
        let uninstall_path = resource_path.join("osara").join("uninstall.exe");
        if uninstall_path.is_file() {
            std::fs::remove_file(&uninstall_path).with_path(&uninstall_path)?;
        }
    }
    if replace_osara_keymap {
        report = apply_osara_keymap_replacement(resource_path)?;
    }
    Ok(report)
}

pub(super) fn verification_paths(resource_path: &Path, replace_osara_keymap: bool) -> Vec<PathBuf> {
    let mut paths = vec![
        resource_path.join("UserPlugins"),
        resource_path.join("KeyMaps").join("OSARA.ReaperKeyMap"),
        resource_path.join("osara"),
    ];
    if replace_osara_keymap {
        paths.push(resource_path.join("reaper-kb.ini"));
    }
    paths
}

pub(super) fn installer_arguments(
    kind: ArtifactKind,
    platform: Platform,
    resource_path: &Path,
) -> Option<Vec<String>> {
    match (kind, platform) {
        (ArtifactKind::Installer, Platform::Windows) => {
            Some(osara_windows_installer_arguments(resource_path))
        }
        _ => None,
    }
}

pub(super) fn planned_execution_override(
    kind: ArtifactKind,
    platform: Platform,
    resource_path: &Path,
) -> Option<PlannedExecutionOverride> {
    match (kind, platform) {
        (ArtifactKind::Archive, Platform::MacOs) => Some(PlannedExecutionOverride {
            kind: PlannedExecutionKind::ExtractArchiveAndCopyOsaraAssets,
            arguments: vec![resource_path.display().to_string()],
            use_cached_working_dir: true,
        }),
        _ => None,
    }
}

fn osara_windows_installer_arguments(resource_path: &Path) -> Vec<String> {
    vec!["/S".to_string(), format!("/D={}", resource_path.display())]
}

pub(super) fn osara_manual_steps(
    kind: ArtifactKind,
    resource_path: &Path,
    replace_osara_keymap: bool,
) -> Vec<String> {
    let mut steps = match kind {
        ArtifactKind::Installer => vec![format!(
            "When the OSARA installer asks for the REAPER target, choose this resource or portable folder: {}",
            resource_path.display()
        )],
        ArtifactKind::Archive | ArtifactKind::SevenZipArchive => vec![format!(
            "Run the OSARA installer from the extracted archive and target this REAPER resource or portable folder: {}",
            resource_path.display()
        )],
        ArtifactKind::DiskImage => vec![format!(
            "Run the OSARA installer from the opened disk image and target this REAPER resource or portable folder: {}",
            resource_path.display()
        )],
        ArtifactKind::ExtensionBinary => vec![format!(
            "Copy the OSARA extension into this REAPER UserPlugins folder: {}",
            resource_path.join("UserPlugins").display()
        )],
    };
    if replace_osara_keymap {
        steps.push(format!(
            "After backing up {}, replace the current key map with the OSARA key map if the installer offers that option.",
            resource_path.join("reaper-kb.ini").display()
        ));
    } else {
        steps.push(
            "Preserve the current key map if the OSARA installer offers a replacement option."
                .to_string(),
        );
    }
    steps
}

pub(super) fn apply_osara_keymap_replacement(
    resource_path: &Path,
) -> Result<UnattendedPostInstallReport> {
    let replacement_source = resource_path.join("KeyMaps").join("OSARA.ReaperKeyMap");
    if !replacement_source.is_file() {
        return Err(RabbitError::PostInstallVerificationFailed {
            missing_paths: vec![replacement_source],
        });
    }

    let current_keymap = resource_path.join("reaper-kb.ini");
    let mut report = UnattendedPostInstallReport::default();
    let mut preserved_scr_lines: Vec<String> = Vec::new();

    if current_keymap.is_file() {
        // Capture the existing SCR records before overwriting. ReaPack
        // registers every installed ReaScript through these lines (via
        // REAPER's `AddRemoveReaScript` API); a plain overwrite would
        // wipe them, so installed packages would silently vanish from
        // REAPER's actions list until the user manually ran
        // "ReaPack: Synchronize packages" inside REAPER. Re-appending
        // the lines after writing OSARA's key map sidesteps that step.
        //
        // Encoding-tolerant read: script/action names in reaper-kb.ini can
        // carry ANSI-code-page bytes on Windows, which a strict UTF-8 read
        // rejects (same failure shape as issue #7's reapack.ini).
        let existing = read_text_file_lossless(&current_keymap)?.text;
        preserved_scr_lines = extract_scr_lines(&existing);

        let (backup_path, backup_manifest_path) = backup_file_for_unattended_change(
            resource_path,
            PACKAGE_OSARA,
            &current_keymap,
            "osara-keymap-replacement",
        )?;
        report.backup_paths.push(backup_path);
        report.backup_manifest_path = Some(backup_manifest_path);
    }

    replace_file_from_source(&replacement_source, &current_keymap)?;

    if !preserved_scr_lines.is_empty() {
        append_lines_preserving_newline(&current_keymap, &preserved_scr_lines)?;
    }

    Ok(report)
}

fn append_lines_preserving_newline(target_path: &Path, lines: &[String]) -> Result<()> {
    // Encoding-tolerant read/write: the target is the freshly written OSARA
    // key map (ASCII in practice, so it decodes as UTF-8), but don't assume.
    // Appended SCR lines are usually ASCII too; when they carry non-ASCII
    // decoded from an ANSI-encoded old key map, they're re-encoded in the
    // target's encoding — those bytes were code-page-ambiguous to begin with,
    // and modern REAPER reads UTF-8, so this is the least-wrong rendering.
    let decoded = read_text_file_lossless(target_path)?;
    let existing = decoded.text;
    let newline = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out = existing;
    if !out.is_empty() && !out.ends_with(newline) {
        out.push_str(newline);
    }
    for line in lines {
        out.push_str(line);
        out.push_str(newline);
    }
    write_text_file_lossless(target_path, &out, decoded.encoding)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::apply_osara_keymap_replacement;

    fn seed_osara_replacement_source(resource_path: &std::path::Path, body: &str) {
        let keymaps = resource_path.join("KeyMaps");
        fs::create_dir_all(&keymaps).unwrap();
        fs::write(keymaps.join("OSARA.ReaperKeyMap"), body).unwrap();
    }

    #[test]
    fn preserves_existing_scr_lines_when_replacing_keymap() {
        let dir = tempdir().unwrap();
        let resource_path = dir.path();
        seed_osara_replacement_source(resource_path, "osara keymap\r\n");

        let existing = "ACT 1 0 \"_RSabc\" \"Custom\" _SWS_ABOUT\r\n\
                        KEY 9 65 _RSabc 0\r\n\
                        SCR 4 0 RSdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef \"Script: foo.lua\" foo.lua\r\n\
                        SCR 260 32060 RScafef00d \"Script: midi.lua\" midi.lua\r\n";
        fs::write(resource_path.join("reaper-kb.ini"), existing).unwrap();

        let report = apply_osara_keymap_replacement(resource_path).unwrap();

        let new_contents = fs::read_to_string(resource_path.join("reaper-kb.ini")).unwrap();
        assert!(new_contents.starts_with("osara keymap\r\n"));
        assert!(new_contents.contains(
            "SCR 4 0 RSdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef \"Script: foo.lua\" foo.lua"
        ));
        assert!(new_contents.contains("SCR 260 32060 RScafef00d \"Script: midi.lua\" midi.lua"));
        // Non-SCR records from the prior key map must NOT come back —
        // OSARA's replacement is intentionally an authoritative rewrite of
        // KEY / ACT bindings.
        assert!(!new_contents.contains("ACT 1 0"));
        assert!(!new_contents.contains("KEY 9 65"));

        // Backup was created since an existing reaper-kb.ini was present.
        assert_eq!(report.backup_paths.len(), 1);
        assert_eq!(
            fs::read_to_string(&report.backup_paths[0]).unwrap(),
            existing
        );
    }

    #[test]
    fn replacement_is_clean_when_old_keymap_has_no_scr_lines() {
        let dir = tempdir().unwrap();
        let resource_path = dir.path();
        seed_osara_replacement_source(resource_path, "osara keymap\r\n");
        fs::write(
            resource_path.join("reaper-kb.ini"),
            "ACT 1 0 \"_RSabc\" \"Custom\" _SWS_ABOUT\r\nKEY 9 65 _RSabc 0\r\n",
        )
        .unwrap();

        apply_osara_keymap_replacement(resource_path).unwrap();

        assert_eq!(
            fs::read_to_string(resource_path.join("reaper-kb.ini")).unwrap(),
            "osara keymap\r\n"
        );
    }

    #[test]
    fn replacement_when_no_prior_keymap_writes_only_osara_content() {
        let dir = tempdir().unwrap();
        let resource_path = dir.path();
        seed_osara_replacement_source(resource_path, "osara keymap\r\n");

        let report = apply_osara_keymap_replacement(resource_path).unwrap();

        assert_eq!(
            fs::read_to_string(resource_path.join("reaper-kb.ini")).unwrap(),
            "osara keymap\r\n"
        );
        assert!(report.backup_paths.is_empty());
        assert!(report.backup_manifest_path.is_none());
    }

    #[test]
    fn replacement_tolerates_ansi_encoded_keymap() {
        // Companion fix to issue #7: reaper-kb.ini script/action names can
        // carry ANSI-code-page bytes on Windows (e.g. CP-1252 0x92 for a
        // curly apostrophe), which a strict UTF-8 read rejects. The SCR
        // capture must survive that and re-append the line.
        let dir = tempdir().unwrap();
        let resource_path = dir.path();
        seed_osara_replacement_source(resource_path, "osara keymap\r\n");

        let mut existing = b"KEY 9 65 _RSabc 0\r\nSCR 4 0 RSdeadbeef \"Script: Tom".to_vec();
        existing.push(0x92);
        existing.extend_from_slice(b"s tool.lua\" tool.lua\r\n");
        fs::write(resource_path.join("reaper-kb.ini"), &existing).unwrap();

        apply_osara_keymap_replacement(resource_path).unwrap();

        let new_contents = fs::read(resource_path.join("reaper-kb.ini")).unwrap();
        let lossy = String::from_utf8_lossy(&new_contents);
        assert!(lossy.starts_with("osara keymap\r\n"));
        assert!(lossy.contains("SCR 4 0 RSdeadbeef"));
        assert!(lossy.contains("tool.lua"));
        assert!(!lossy.contains("KEY 9 65"));
    }

    #[test]
    fn preserves_scr_lines_using_lf_newline_when_osara_keymap_is_lf() {
        let dir = tempdir().unwrap();
        let resource_path = dir.path();
        // LF-only replacement source — exercise the newline detection path.
        seed_osara_replacement_source(resource_path, "osara keymap\n");
        fs::write(
            resource_path.join("reaper-kb.ini"),
            "SCR 4 0 RSdeadbeef \"Script: foo.lua\" foo.lua\n",
        )
        .unwrap();

        apply_osara_keymap_replacement(resource_path).unwrap();

        let new_contents = fs::read_to_string(resource_path.join("reaper-kb.ini")).unwrap();
        assert!(!new_contents.contains("\r\n"));
        assert_eq!(
            new_contents,
            "osara keymap\nSCR 4 0 RSdeadbeef \"Script: foo.lua\" foo.lua\n"
        );
    }
}
