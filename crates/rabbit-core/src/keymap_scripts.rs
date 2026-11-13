//! Keeping REAPER's scripts in the Actions list when one key map replaces
//! another.
//!
//! ReaPack registers every ReaScript it installs through a script line
//! (`SCR`) in `reaper-kb.ini` (see [`crate::reapack::extract_scr_lines`]).
//! A key map that replaces `reaper-kb.ini` whole, as OSARA's key map does,
//! or an older key map a restore puts back, would wipe those lines: the
//! scripts would vanish from REAPER's Actions list until the user ran
//! "ReaPack: Synchronize packages". So the key map that replaces another
//! gets the script lines of the one it replaced that it doesn't register
//! itself, each once. OSARA's key map replacement and the restore of a key
//! map both do it this way.

use std::collections::HashSet;
use std::path::Path;

use crate::error::Result;
use crate::osara_keymap::script_key;
use crate::reapack::extract_scr_lines;
use crate::text_file::{
    DecodedTextFile, TextFileEncoding, encode_text, read_text_file_lossless,
    write_text_file_lossless,
};

/// The script lines of `replaced`, the text of a key map, that
/// `replacement`, the text of the key map replacing it, doesn't register.
/// A script counts by its section and id, so one registered in several
/// sections stays in each. Each comes once: the replacement's own lines
/// would otherwise come back a second time, and a script `replaced` lists
/// twice would too. A script line whose section and id can't be read is
/// kept as it is.
pub fn scripts_to_keep(replaced: &str, replacement: &str) -> Vec<String> {
    let owned_key = |(section, id): (&str, &str)| (section.to_string(), id.to_string());
    let mut registered = replacement
        .lines()
        .filter_map(script_key)
        .map(owned_key)
        .collect::<HashSet<_>>();
    extract_scr_lines(replaced)
        .into_iter()
        .filter(|line| script_key(line).is_none_or(|key| registered.insert(owned_key(key))))
        .collect()
}

/// The key map at `keymap_path`, which just replaced the key map whose text
/// is `replaced`, with the script lines of `replaced` it doesn't register
/// after its own (see [`scripts_to_keep`]), as
/// [`append_lines_preserving_newline`] adds them: the bytes to write over
/// it, and how many lines they add. `None` when there are none to add.
/// Nothing is written, so the caller writes the bytes the way it must.
pub(crate) fn keymap_with_scripts_kept(
    keymap_path: &Path,
    replaced: &str,
) -> Result<Option<(Vec<u8>, usize)>> {
    let keymap = read_text_file_lossless(keymap_path)?;
    let lines = scripts_to_keep(replaced, &keymap.text);
    if lines.is_empty() {
        return Ok(None);
    }
    let (text, encoding) = with_lines(keymap, &lines);
    Ok(Some((encode_text(&text, encoding), lines.len())))
}

/// Append `lines` to the text file at `target_path`, each ending the way
/// the file's own lines end, CRLF or LF, and in the file's own encoding.
pub(crate) fn append_lines_preserving_newline(target_path: &Path, lines: &[String]) -> Result<()> {
    let (text, encoding) = with_lines(read_text_file_lossless(target_path)?, lines);
    write_text_file_lossless(target_path, &text, encoding)
}

/// `decoded`, a text file as it was read, with `lines` after it (see
/// [`append_lines_preserving_newline`]), and the encoding to write it in.
fn with_lines(decoded: DecodedTextFile, lines: &[String]) -> (String, TextFileEncoding) {
    // The file goes back in its own encoding, so its own text comes back
    // as it was. The lines may come from a key map in another encoding:
    // from an ANSI-encoded old key map into OSARA's key map, which is
    // ASCII in practice and so reads as UTF-8, or, when a restore puts an
    // ANSI-encoded old key map back, from the UTF-8 key map it replaced.
    // Either way they are written in the file's encoding. Into UTF-8, a
    // byte of an ANSI code page goes as the Latin-1 character it was read
    // as: those bytes were code-page-ambiguous to begin with, and modern
    // REAPER reads UTF-8, so this is the least-wrong rendering. Into an
    // ANSI file, a character up to U+00FF goes as its one byte and any
    // other as `?`, which no single byte can mean.
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
    (out, decoded.encoding)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{keymap_with_scripts_kept, scripts_to_keep};

    #[test]
    fn only_scripts_the_replacement_does_not_register_are_kept_each_once() {
        let replaced = "KEY 1 65 40001 0\r\n\
                        SCR 4 0 RSosara \"Custom: OSARA old.lua\" osara.lua\r\n\
                        SCR 4 0 RSmine \"Custom: mine.lua\" mine.lua\r\n\
                        SCR 4 0 RSmine \"Custom: mine.lua\" mine.lua\r\n\
                        SCR 4 32060 RSosara \"Custom: OSARA old.lua\" osara.lua\r\n\
                        SCR 4\r\n";
        let replacement = "SCR 4 0 RSosara \"Custom: OSARA new.lua\" osara.lua\r\n";

        assert_eq!(
            scripts_to_keep(replaced, replacement),
            [
                "SCR 4 0 RSmine \"Custom: mine.lua\" mine.lua",
                "SCR 4 32060 RSosara \"Custom: OSARA old.lua\" osara.lua",
                "SCR 4",
            ]
        );
        assert!(scripts_to_keep("KEY 1 65 40001 0\n", replacement).is_empty());
    }

    /// The key map with the scripts kept ends each line as its own lines
    /// end, and nothing is written: the caller writes it.
    #[test]
    fn kept_scripts_end_as_the_key_map_s_own_lines_do() {
        let dir = tempdir().unwrap();
        let keymap = dir.path().join("reaper-kb.ini");
        let mine = "SCR 4 0 RSmine \"Custom: mine.lua\" mine.lua\r\n";
        for (replacement, expected) in [
            (
                "KEY 1 65 40001 0\r\n",
                "KEY 1 65 40001 0\r\nSCR 4 0 RSmine \"Custom: mine.lua\" mine.lua\r\n",
            ),
            (
                "KEY 1 65 40001 0",
                "KEY 1 65 40001 0\nSCR 4 0 RSmine \"Custom: mine.lua\" mine.lua\n",
            ),
        ] {
            fs::write(&keymap, replacement).unwrap();
            let kept = keymap_with_scripts_kept(&keymap, mine).unwrap();
            assert_eq!(kept, Some((expected.as_bytes().to_vec(), 1)));
            assert_eq!(fs::read_to_string(&keymap).unwrap(), replacement);
        }

        // Nothing to keep is nothing to write.
        fs::write(&keymap, b"KEY 1 65 40001 0").unwrap();
        assert_eq!(
            keymap_with_scripts_kept(&keymap, "KEY 1 66 40002 0\n").unwrap(),
            None
        );
        assert_eq!(fs::read(&keymap).unwrap(), b"KEY 1 65 40001 0");
    }
}
