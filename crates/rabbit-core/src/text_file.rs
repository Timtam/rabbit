//! Encoding-tolerant text I/O for config files RABBIT edits but does not
//! own — ReaPack's `reapack.ini` and REAPER's `reaper-kb.ini`.
//!
//! Those files are written by ReaPack / REAPER through Win32 profile-string
//! and C-runtime APIs, which on Windows encode text in the active ANSI code
//! page (e.g. Windows-1252) — or in UTF-16 when the file carries a BOM. A
//! strict `fs::read_to_string` therefore fails with "stream did not contain
//! valid UTF-8" the moment a repository name or action label contains a
//! single non-ASCII character (issue #7).
//!
//! The contract here is *byte preservation*: decode whatever is on disk into
//! a `String` we can run line-based logic on, and re-encode so that every
//! byte we didn't deliberately change comes back out exactly as it went in.
//!
//! - Valid UTF-8 → kept as-is (the common case; pure-ASCII ANSI files land
//!   here too, which is why this bug stayed hidden on most machines).
//! - UTF-16 LE/BE with BOM → decoded with `from_utf16_lossy`; re-encoded as
//!   the same UTF-16 flavour, BOM included.
//! - Anything else → treated as a single-byte ANSI encoding and decoded as
//!   Latin-1 (each byte 0x00–0xFF maps to the code point of the same value).
//!   That mapping is lossless and reversible for arbitrary bytes, so the
//!   user's CP-125x text survives a read→edit→write round trip untouched —
//!   we never guess the real code page, we just refuse to mangle it. Our own
//!   insertions are pure ASCII, which encodes identically in every ANSI code
//!   page.

use std::fs;
use std::path::Path;

use crate::error::{IoPathContext, Result};

/// How the bytes on disk were decoded, and therefore how an edited text
/// must be re-encoded to leave untouched content byte-identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFileEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    /// Not valid UTF-8 and no UTF-16 BOM: decoded byte-for-byte as Latin-1.
    /// Stands in for "whatever single-byte ANSI code page wrote this".
    AnsiLossless,
}

#[derive(Debug, Clone)]
pub struct DecodedTextFile {
    pub text: String,
    pub encoding: TextFileEncoding,
}

/// Read a text file without assuming UTF-8. Never fails on encoding —
/// only on I/O.
pub fn read_text_file_lossless(path: &Path) -> Result<DecodedTextFile> {
    let bytes = fs::read(path).with_path(path)?;
    Ok(decode_bytes(bytes))
}

/// Re-encode `text` per `encoding` and write it to `path`.
pub fn write_text_file_lossless(path: &Path, text: &str, encoding: TextFileEncoding) -> Result<()> {
    fs::write(path, encode_text(text, encoding)).with_path(path)
}

fn decode_bytes(bytes: Vec<u8>) -> DecodedTextFile {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return DecodedTextFile {
            text: decode_utf16(&bytes[2..], u16::from_le_bytes),
            encoding: TextFileEncoding::Utf16Le,
        };
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return DecodedTextFile {
            text: decode_utf16(&bytes[2..], u16::from_be_bytes),
            encoding: TextFileEncoding::Utf16Be,
        };
    }
    match String::from_utf8(bytes) {
        Ok(text) => DecodedTextFile {
            text,
            encoding: TextFileEncoding::Utf8,
        },
        Err(error) => DecodedTextFile {
            text: error.into_bytes().iter().map(|&b| char::from(b)).collect(),
            encoding: TextFileEncoding::AnsiLossless,
        },
    }
}

fn decode_utf16(bytes: &[u8], unit_from_bytes: fn([u8; 2]) -> u16) -> String {
    let mut units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| unit_from_bytes([pair[0], pair[1]]))
        .collect();
    // A trailing odd byte means the file is malformed UTF-16; keep the byte
    // as its own unit rather than silently dropping it.
    if let Some(&last) = bytes.chunks_exact(2).remainder().first() {
        units.push(u16::from(last));
    }
    String::from_utf16_lossy(&units)
}

fn encode_text(text: &str, encoding: TextFileEncoding) -> Vec<u8> {
    match encoding {
        TextFileEncoding::Utf8 => text.as_bytes().to_vec(),
        TextFileEncoding::Utf16Le => {
            let mut out = vec![0xFF, 0xFE];
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
        TextFileEncoding::Utf16Be => {
            let mut out = vec![0xFE, 0xFF];
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            out
        }
        TextFileEncoding::AnsiLossless => text
            .chars()
            .map(|ch| {
                // Latin-1 decode only produces chars ≤ U+00FF, and our own
                // insertions are ASCII, so this branch is total in practice.
                // A char above U+00FF could only appear if a caller spliced
                // text from a differently-encoded source; degrade to '?'
                // rather than emitting bytes the file's code page can't mean.
                u8::try_from(u32::from(ch)).unwrap_or(b'?')
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{TextFileEncoding, read_text_file_lossless, write_text_file_lossless};

    fn round_trip(bytes: &[u8]) -> (Vec<u8>, TextFileEncoding) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.ini");
        std::fs::write(&path, bytes).unwrap();
        let decoded = read_text_file_lossless(&path).unwrap();
        let encoding = decoded.encoding;
        write_text_file_lossless(&path, &decoded.text, encoding).unwrap();
        (std::fs::read(&path).unwrap(), encoding)
    }

    #[test]
    fn utf8_round_trips_unchanged() {
        let bytes = "[remotes]\r\nsize=1\r\nremote0=Füße|https://example.invalid|1|2\r\n"
            .as_bytes()
            .to_vec();
        let (out, encoding) = round_trip(&bytes);
        assert_eq!(encoding, TextFileEncoding::Utf8);
        assert_eq!(out, bytes);
    }

    #[test]
    fn ansi_bytes_round_trip_unchanged() {
        // CP-1252 "Tom’s Repo" — 0x92 is a curly apostrophe in CP-1252 and
        // invalid UTF-8, the exact shape from issue #7.
        let mut bytes = b"[remotes]\r\nsize=1\r\nremote0=Tom".to_vec();
        bytes.push(0x92);
        bytes.extend_from_slice(b"s Repo|https://example.invalid|1|2\r\n");
        let (out, encoding) = round_trip(&bytes);
        assert_eq!(encoding, TextFileEncoding::AnsiLossless);
        assert_eq!(out, bytes);
    }

    #[test]
    fn utf16le_round_trips_with_bom() {
        let text = "[remotes]\r\nsize=1\r\nremote0=Füße|https://example.invalid|1|2\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let (out, encoding) = round_trip(&bytes);
        assert_eq!(encoding, TextFileEncoding::Utf16Le);
        assert_eq!(out, bytes);
    }

    #[test]
    fn utf16be_round_trips_with_bom() {
        let text = "[remotes]\r\nsize=1\r\n";
        let mut bytes = vec![0xFE, 0xFF];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        let (out, encoding) = round_trip(&bytes);
        assert_eq!(encoding, TextFileEncoding::Utf16Be);
        assert_eq!(out, bytes);
    }

    #[test]
    fn decoded_text_is_usable_for_line_logic_regardless_of_encoding() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.ini");
        let mut bytes = b"[remotes]\r\nremote0=Tom".to_vec();
        bytes.push(0x92);
        bytes.extend_from_slice(b"s|https://example.invalid|1|2\r\n");
        std::fs::write(&path, &bytes).unwrap();

        let decoded = read_text_file_lossless(&path).unwrap();
        // ASCII content (section headers, URLs) is untouched by the Latin-1
        // decode, so string matching keeps working.
        assert!(decoded.text.contains("[remotes]"));
        assert!(decoded.text.contains("https://example.invalid"));
    }
}
