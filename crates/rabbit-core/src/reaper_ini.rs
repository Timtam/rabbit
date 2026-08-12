//! Edits to REAPER's own `reaper.ini`.
//!
//! Sibling of [`crate::reapack`], which does the same for `reapack.ini`.
//! REAPER writes this file through the Win32 profile-string APIs, so it is
//! frequently NOT valid UTF-8 — every edit here decodes losslessly, changes
//! only the line it must, and writes back in the original encoding with the
//! original line endings.
//!
//! Today the only edit is selecting a language pack, which REAPER stores as
//! `langpack=<file name>` under `[REAPER]` (verified against real installs;
//! `<>` is REAPER's "no language pack" sentinel).

use std::fs;
use std::path::Path;

use crate::error::{IoPathContext, Result};
use crate::text_file::{TextFileEncoding, read_text_file_lossless, write_text_file_lossless};

/// `reaper.ini`, relative to the REAPER resource path.
pub const REAPER_INI_RELATIVE_PATH: &str = "reaper.ini";
/// Section REAPER keeps its main preferences in.
const REAPER_SECTION: &str = "[REAPER]";
/// Key naming the active language pack file.
const LANGPACK_KEY: &str = "langpack";
/// Value REAPER writes when no language pack is selected.
const LANGPACK_NONE: &str = "<>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangPackSelectionOutcome {
    /// `reaper.ini` already selected this language pack — nothing written.
    AlreadySelected,
    /// The existing `langpack=` value was replaced.
    Replaced,
    /// A `langpack=` key was added (to an existing or new `[REAPER]` section).
    Added,
    /// `reaper.ini` didn't exist and was created with just this setting.
    CreatedFile,
}

/// Point REAPER at the language pack named `file_name` (e.g.
/// `es_ES.ReaperLangPack`), which must already be installed under
/// `<resource_path>/LangPack/`. Idempotent: selecting the pack that is
/// already active reports [`LangPackSelectionOutcome::AlreadySelected`]
/// without rewriting the file.
pub fn select_lang_pack(resource_path: &Path, file_name: &str) -> Result<LangPackSelectionOutcome> {
    let ini_path = resource_path.join(REAPER_INI_RELATIVE_PATH);
    let (original, encoding) = if ini_path.is_file() {
        let decoded = read_text_file_lossless(&ini_path)?;
        (decoded.text, decoded.encoding)
    } else {
        (String::new(), TextFileEncoding::Utf8)
    };

    if original.is_empty() {
        let text = format!("{REAPER_SECTION}\r\n{LANGPACK_KEY}={file_name}\r\n");
        fs::create_dir_all(resource_path).with_path(resource_path)?;
        write_text_file_lossless(&ini_path, &text, encoding)?;
        return Ok(LangPackSelectionOutcome::CreatedFile);
    }

    // Preserve the file's existing line ending — REAPER writes `\r\n` on
    // Windows, and normalising would visually scramble the whole file.
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    match rewrite_with_lang_pack(&original, file_name, newline) {
        Some((text, outcome)) => {
            fs::create_dir_all(resource_path).with_path(resource_path)?;
            write_text_file_lossless(&ini_path, &text, encoding)?;
            Ok(outcome)
        }
        None => Ok(LangPackSelectionOutcome::AlreadySelected),
    }
}

/// The language pack `reaper.ini` currently selects, if any. `None` when the
/// file is missing, the key is absent, or the value is REAPER's `<>`
/// no-pack sentinel.
pub fn selected_lang_pack(resource_path: &Path) -> Result<Option<String>> {
    let ini_path = resource_path.join(REAPER_INI_RELATIVE_PATH);
    if !ini_path.is_file() {
        return Ok(None);
    }
    let decoded = read_text_file_lossless(&ini_path)?;
    Ok(current_lang_pack(&decoded.text))
}

/// Pure form of [`selected_lang_pack`], over already-decoded text.
fn current_lang_pack(text: &str) -> Option<String> {
    let mut in_reaper_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_reaper_section = trimmed.eq_ignore_ascii_case(REAPER_SECTION);
            continue;
        }
        if !in_reaper_section {
            continue;
        }
        if let Some(value) = lang_pack_value(trimmed) {
            return if value.is_empty() || value == LANGPACK_NONE {
                None
            } else {
                Some(value.to_string())
            };
        }
    }
    None
}

/// `Some(value)` when `line` is the `langpack=` assignment.
fn lang_pack_value(line: &str) -> Option<&str> {
    let (key, value) = line.split_once('=')?;
    key.trim()
        .eq_ignore_ascii_case(LANGPACK_KEY)
        .then(|| value.trim())
}

/// Rewrite `original` so `[REAPER]` selects `file_name`. Returns `None` when
/// it already does (nothing to write). Only the `langpack=` line is touched;
/// every other byte is preserved verbatim.
fn rewrite_with_lang_pack(
    original: &str,
    file_name: &str,
    newline: &str,
) -> Option<(String, LangPackSelectionOutcome)> {
    if current_lang_pack(original).as_deref() == Some(file_name) {
        return None;
    }

    let lines: Vec<&str> = original.split_inclusive('\n').collect();
    let mut output = String::with_capacity(original.len() + file_name.len() + 16);
    let mut in_reaper_section = false;
    let mut seen_reaper_section = false;
    let mut replaced = false;
    // Index (into `output`-so-far line stream) where a new key can be
    // inserted: just after the `[REAPER]` header, if we never find the key.
    let mut insert_at: Option<usize> = None;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_reaper_section = trimmed.eq_ignore_ascii_case(REAPER_SECTION);
            if in_reaper_section {
                seen_reaper_section = true;
            }
            output.push_str(line);
            if in_reaper_section {
                insert_at = Some(output.len());
            }
            continue;
        }
        if in_reaper_section && !replaced && lang_pack_value(trimmed).is_some() {
            // Replace just the value, keeping the original line ending.
            let ending = line
                .len()
                .checked_sub(line.trim_end_matches(['\r', '\n']).len())
                .filter(|len| *len > 0)
                .map(|len| &line[line.len() - len..])
                .unwrap_or(newline);
            output.push_str(&format!("{LANGPACK_KEY}={file_name}{ending}"));
            replaced = true;
            continue;
        }
        output.push_str(line);
    }

    if replaced {
        return Some((output, LangPackSelectionOutcome::Replaced));
    }

    if let Some(at) = insert_at {
        // `[REAPER]` exists but carries no `langpack=` — insert one directly
        // under the header so it lands in the right section.
        let mut with_key = String::with_capacity(output.len() + file_name.len() + 16);
        with_key.push_str(&output[..at]);
        with_key.push_str(&format!("{LANGPACK_KEY}={file_name}{newline}"));
        with_key.push_str(&output[at..]);
        return Some((with_key, LangPackSelectionOutcome::Added));
    }

    // No `[REAPER]` section at all — append one.
    debug_assert!(!seen_reaper_section);
    if !output.ends_with('\n') && !output.is_empty() {
        output.push_str(newline);
    }
    output.push_str(&format!(
        "{REAPER_SECTION}{newline}{LANGPACK_KEY}={file_name}{newline}"
    ));
    Some((output, LangPackSelectionOutcome::Added))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn replaces_an_existing_langpack_value_and_keeps_everything_else() {
        // Mirrors a real reaper.ini: CRLF endings, other keys around it, and
        // REAPER's `<>` no-pack sentinel as the starting value.
        let original = "[REAPER]\r\nlegacy_filebrowse=1\r\nlangpack=<>\r\nwnd_state=1\r\n\r\n[anothersection]\r\nlangpack=leaveme\r\n";
        let (text, outcome) = rewrite_with_lang_pack(original, "es_ES.ReaperLangPack", "\r\n")
            .expect("value differs, so a rewrite is needed");

        assert_eq!(outcome, LangPackSelectionOutcome::Replaced);
        assert!(text.contains("langpack=es_ES.ReaperLangPack\r\n"));
        // Untouched neighbours, including a same-named key in another section.
        assert!(text.contains("legacy_filebrowse=1\r\n"));
        assert!(text.contains("wnd_state=1\r\n"));
        assert!(text.contains("[anothersection]\r\nlangpack=leaveme\r\n"));
        assert!(!text.contains("langpack=<>"));
        // CRLF preserved throughout — no stray bare LF introduced.
        assert_eq!(text.matches('\n').count(), text.matches("\r\n").count());
    }

    #[test]
    fn adds_the_key_when_the_reaper_section_has_none() {
        let original = "[REAPER]\nlegacy_filebrowse=1\n\n[other]\nkey=1\n";
        let (text, outcome) = rewrite_with_lang_pack(original, "de_DE.ReaperLangPack", "\n")
            .expect("key absent, so a rewrite is needed");

        assert_eq!(outcome, LangPackSelectionOutcome::Added);
        // Inserted INSIDE [REAPER], not appended at the end of the file.
        let reaper_at = text.find("[REAPER]").unwrap();
        let other_at = text.find("[other]").unwrap();
        let key_at = text.find("langpack=de_DE.ReaperLangPack").unwrap();
        assert!(reaper_at < key_at && key_at < other_at, "{text:?}");
    }

    #[test]
    fn appends_a_reaper_section_when_the_file_has_none() {
        let original = "[other]\nkey=1\n";
        let (text, outcome) = rewrite_with_lang_pack(original, "es_ES.ReaperLangPack", "\n")
            .expect("no section, so a rewrite is needed");

        assert_eq!(outcome, LangPackSelectionOutcome::Added);
        assert!(text.starts_with("[other]\nkey=1\n"));
        assert!(text.contains("[REAPER]\nlangpack=es_ES.ReaperLangPack\n"));
    }

    #[test]
    fn selecting_the_active_pack_is_a_no_op() {
        let original = "[REAPER]\r\nlangpack=es_ES.ReaperLangPack\r\n";
        assert!(rewrite_with_lang_pack(original, "es_ES.ReaperLangPack", "\r\n").is_none());
    }

    #[test]
    fn reads_the_selected_pack_and_treats_the_sentinel_as_none() {
        assert_eq!(
            current_lang_pack("[REAPER]\r\nlangpack=Deutsch.ReaperLangPack\r\n").as_deref(),
            Some("Deutsch.ReaperLangPack")
        );
        // REAPER's "no language pack" marker.
        assert_eq!(current_lang_pack("[REAPER]\r\nlangpack=<>\r\n"), None);
        assert_eq!(current_lang_pack("[REAPER]\r\nother=1\r\n"), None);
        // A langpack key outside [REAPER] must not be read as the selection.
        assert_eq!(current_lang_pack("[other]\r\nlangpack=nope\r\n"), None);
    }

    #[test]
    fn creates_the_file_when_absent_then_round_trips() {
        let dir = tempdir().unwrap();
        let resource = dir.path();

        assert_eq!(
            select_lang_pack(resource, "es_ES.ReaperLangPack").unwrap(),
            LangPackSelectionOutcome::CreatedFile
        );
        assert_eq!(
            selected_lang_pack(resource).unwrap().as_deref(),
            Some("es_ES.ReaperLangPack")
        );
        // Idempotent second call.
        assert_eq!(
            select_lang_pack(resource, "es_ES.ReaperLangPack").unwrap(),
            LangPackSelectionOutcome::AlreadySelected
        );
        // Switching languages replaces the value.
        assert_eq!(
            select_lang_pack(resource, "de_DE.ReaperLangPack").unwrap(),
            LangPackSelectionOutcome::Replaced
        );
        assert_eq!(
            selected_lang_pack(resource).unwrap().as_deref(),
            Some("de_DE.ReaperLangPack")
        );
    }

    #[test]
    fn preserves_non_utf8_bytes_elsewhere_in_the_file() {
        // reaper.ini is written by the Win32 profile APIs and is frequently
        // in the active ANSI code page, not UTF-8. Editing the langpack line
        // must not corrupt bytes we never touched.
        let dir = tempdir().unwrap();
        let resource = dir.path();
        let ini = resource.join(REAPER_INI_RELATIVE_PATH);
        let mut bytes = b"[REAPER]\r\nlangpack=<>\r\nlastproject=C:\\Musik\\".to_vec();
        bytes.extend_from_slice(&[0xDC, 0xE4, 0xF6]); // Ü ä ö in CP1252
        bytes.extend_from_slice(b".rpp\r\n");
        std::fs::write(&ini, &bytes).unwrap();

        select_lang_pack(resource, "de_DE.ReaperLangPack").unwrap();

        let after = std::fs::read(&ini).unwrap();
        assert!(
            after.windows(3).any(|w| w == [0xDC, 0xE4, 0xF6]),
            "non-UTF-8 bytes must survive the edit"
        );
        assert_eq!(
            selected_lang_pack(resource).unwrap().as_deref(),
            Some("de_DE.ReaperLangPack")
        );
    }
}
