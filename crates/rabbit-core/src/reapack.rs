use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{IoPathContext, Result, SqlitePathContext};
use crate::text_file::{TextFileEncoding, read_text_file_lossless, write_text_file_lossless};
use crate::version::Version;

pub const REAPACK_REGISTRY_RELATIVE_PATH: &str = "ReaPack/registry.db";
pub const REAPACK_INI_RELATIVE_PATH: &str = "reapack.ini";

/// Outcome of [`upsert_remote`]: tells callers whether the upsert was a
/// no-op (the URL was already configured), whether we appended into an
/// existing config, or whether we created a fresh `reapack.ini` (which
/// happens when ReaPack hasn't run yet — the user's first REAPER launch
/// will let ReaPack's own first-time migration add its default ReaTeam
/// repos alongside the remote we just wrote).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteUpsertOutcome {
    /// A remote with this URL was already configured; nothing changed.
    AlreadyPresent,
    /// Appended a new remote into the existing `reapack.ini`.
    Added,
    /// `reapack.ini` did not exist; created it with our remote as the
    /// only entry.
    CreatedFile,
}

/// Add a remote repository to ReaPack's `reapack.ini` config file.
/// Idempotent on URL: re-running with the same URL is a no-op rather
/// than appending a duplicate. Field encoding mirrors what ReaPack
/// itself writes — `name|url|enabled|autoinstall`, where `enabled` is
/// `0`/`1` and `autoinstall` is `0` (false), `1` (true), or `2` (use
/// the global default; what ReaPack writes for repos that don't
/// override the global setting).
pub fn upsert_remote(resource_path: &Path, name: &str, url: &str) -> Result<RemoteUpsertOutcome> {
    let ini_path = resource_path.join(REAPACK_INI_RELATIVE_PATH);
    // ReaPack writes this file through the Win32 profile-string APIs, which
    // encode in the active ANSI code page (or UTF-16 when the file carries a
    // BOM) — so it is frequently NOT valid UTF-8 (issue #7). Decode
    // losslessly and remember the encoding so the rewrite below puts every
    // byte we didn't change back exactly as it was; the lines we insert are
    // pure ASCII, which encodes identically in every supported encoding.
    let (original, encoding) = if ini_path.is_file() {
        let decoded = read_text_file_lossless(&ini_path)?;
        (decoded.text, decoded.encoding)
    } else {
        (String::new(), TextFileEncoding::Utf8)
    };
    let creating_new = original.is_empty();
    // Preserve the file's existing line ending — Windows users typically
    // have `\r\n` here, and overwriting to `\n` would visually scramble the
    // file in tools that don't normalise.
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    if !creating_new && url_is_already_present(&original, url) {
        return Ok(RemoteUpsertOutcome::AlreadyPresent);
    }

    let new_text = if creating_new {
        format!(
            "[remotes]{nl}size=1{nl}remote0={name}|{url}|1|2{nl}",
            nl = newline,
            name = name,
            url = url,
        )
    } else {
        rewrite_with_appended_remote(&original, name, url, newline)
    };

    fs::create_dir_all(resource_path).with_path(resource_path)?;
    write_text_file_lossless(&ini_path, &new_text, encoding)?;

    Ok(if creating_new {
        RemoteUpsertOutcome::CreatedFile
    } else {
        RemoteUpsertOutcome::Added
    })
}

/// `true` if `<resource_path>/reapack.ini` exists and lists `url` under
/// its `[remotes]` section. Used by the wizard / CLI to suppress the
/// "configure REAPER Accessibility ReaPack remote" step when the remote
/// is already wired up — there is nothing for us to do, and re-offering
/// it as actionable is misleading. A missing `reapack.ini` reads as
/// "not configured".
pub fn is_remote_configured(resource_path: &Path, url: &str) -> Result<bool> {
    let ini_path = resource_path.join(REAPACK_INI_RELATIVE_PATH);
    if !ini_path.is_file() {
        return Ok(false);
    }
    // Encoding-tolerant read (see `upsert_remote`): the URL we match on is
    // ASCII, and ASCII bytes decode identically under every encoding the
    // lossless reader produces, so matching is reliable even when repo
    // names around it are in some ANSI code page.
    let text = read_text_file_lossless(&ini_path)?.text;
    Ok(url_is_already_present(&text, url))
}

/// `true` when the `[remotes]` section already contains an entry whose
/// URL field matches `url` (the second pipe-delimited field of any
/// `remote<N>=...` line).
fn url_is_already_present(text: &str, url: &str) -> bool {
    let mut current_section: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some(name) = rest.strip_suffix(']') {
                current_section = Some(name.to_string());
                continue;
            }
        }
        if current_section.as_deref() != Some("remotes") {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if !key.trim().starts_with("remote") {
            continue;
        }
        let mut parts = value.trim().splitn(4, '|');
        let _name = parts.next();
        if let Some(existing_url) = parts.next() {
            if existing_url == url {
                return true;
            }
        }
    }
    false
}

/// Append a `remote<size>=...` entry into the existing `[remotes]`
/// section and bump `size=`. If the section doesn't exist yet, append
/// it at the end of the file as a fresh section with our entry as
/// `remote0`.
fn rewrite_with_appended_remote(original: &str, name: &str, url: &str, newline: &str) -> String {
    let mut lines: Vec<String> = original.lines().map(|s| s.to_string()).collect();

    // First pass: locate the `[remotes]` section and pull out its size +
    // the highest existing remote index.
    let mut current_section: Option<String> = None;
    let mut section_start: Option<usize> = None;
    let mut section_end: Option<usize> = None;
    let mut size_line_idx: Option<usize> = None;
    let mut current_size: u32 = 0;
    let mut max_remote_idx: Option<u32> = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some(sec_name) = rest.strip_suffix(']') {
                if current_section.as_deref() == Some("remotes") && section_end.is_none() {
                    section_end = Some(i);
                }
                current_section = Some(sec_name.to_string());
                if sec_name == "remotes" && section_start.is_none() {
                    section_start = Some(i);
                }
                continue;
            }
        }
        if current_section.as_deref() != Some("remotes") {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key == "size" {
            current_size = value.trim().parse().unwrap_or(0);
            size_line_idx = Some(i);
        } else if let Some(idx_str) = key.strip_prefix("remote") {
            if let Ok(idx) = idx_str.parse::<u32>() {
                max_remote_idx = Some(max_remote_idx.map_or(idx, |m| m.max(idx)));
            }
        }
    }
    if current_section.as_deref() == Some("remotes") && section_end.is_none() {
        section_end = Some(lines.len());
    }

    // Pick the next index. ReaPack treats `size` as authoritative for the
    // count, so we honour that — but we also fall back to "max existing
    // index + 1" if `size` is missing or out of sync.
    let next_index = current_size.max(max_remote_idx.map_or(0, |m| m + 1));
    let new_size = next_index + 1;
    let new_remote_line = format!("remote{}={}|{}|1|2", next_index, name, url);
    let new_size_line = format!("size={}", new_size);

    if let (Some(start), Some(end)) = (section_start, section_end) {
        // Insert the new remote line at the bottom of the existing
        // `[remotes]` section. Update or insert `size=`.
        if let Some(idx) = size_line_idx {
            lines[idx] = new_size_line;
            lines.insert(end, new_remote_line);
        } else {
            lines.insert(start + 1, new_size_line);
            // The size insertion shifts the section_end by 1.
            lines.insert(end + 1, new_remote_line);
        }
    } else {
        // No `[remotes]` section yet — append a fresh one at the end.
        if !lines.is_empty() {
            let last_is_blank = lines.last().is_some_and(|line| line.trim().is_empty());
            if !last_is_blank {
                lines.push(String::new());
            }
        }
        lines.push("[remotes]".to_string());
        lines.push("size=1".to_string());
        lines.push(format!("remote0={}|{}|1|2", name, url));
    }

    let mut joined = lines.join(newline);
    if !original.ends_with(newline) || joined.is_empty() {
        // Match the source file's trailing-newline convention.
    }
    if !joined.ends_with(newline) {
        joined.push_str(newline);
    }
    joined
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReapackEntry {
    pub remote: String,
    pub category: String,
    pub package: String,
    pub version: Version,
}

/// Extract every `SCR` line from the textual content of a `reaper-kb.ini`.
/// ReaPack registers every installed ReaScript action exclusively via
/// these lines (it never writes `KEY` or `ACT` records itself; REAPER
/// produces the line in response to ReaPack's `AddRemoveReaScript`
/// host-API call). Returning them as opaque strings lets callers
/// re-append them verbatim to a freshly written key map — preserving the
/// `_RS<hex>` action command IDs, which REAPER derives deterministically
/// from each script's path, so any user `KEY` binding that targets one
/// of those IDs keeps working.
pub fn extract_scr_lines(reaper_kb_ini_text: &str) -> Vec<String> {
    reaper_kb_ini_text
        .lines()
        .filter(|line| line.starts_with("SCR ") || line.starts_with("SCR\t"))
        .map(|line| line.to_string())
        .collect()
}

pub fn registry_path(resource_path: &Path) -> PathBuf {
    resource_path.join(REAPACK_REGISTRY_RELATIVE_PATH)
}

pub fn package_owner_for_file(
    resource_path: &Path,
    absolute_file: &Path,
) -> Result<Option<ReapackEntry>> {
    let db_path = registry_path(resource_path);
    if !db_path.is_file() {
        return Ok(None);
    }

    let Some(relative_path) = relative_reapack_path(resource_path, absolute_file) else {
        return Ok(None);
    };

    query_owner(&db_path, &relative_path)
}

pub fn list_entries(resource_path: &Path) -> Result<Vec<ReapackEntry>> {
    let db_path = registry_path(resource_path);
    if !db_path.is_file() {
        return Ok(Vec::new());
    }

    let connection = open_read_only(&db_path)?;
    let mut statement = connection
        .prepare("SELECT remote, category, package, version FROM entries ORDER BY remote, category, package")
        .with_sqlite_path(&db_path)?;

    let rows = statement
        .query_map([], |row| {
            let version_text: String = row.get(3)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                version_text,
            ))
        })
        .with_sqlite_path(&db_path)?;

    let mut entries = Vec::new();
    for row in rows {
        let (remote, category, package, version_text) = row.with_sqlite_path(&db_path)?;
        let Ok(version) = Version::parse(&version_text) else {
            continue;
        };
        entries.push(ReapackEntry {
            remote,
            category,
            package,
            version,
        });
    }

    Ok(entries)
}

fn query_owner(db_path: &Path, relative_path: &str) -> Result<Option<ReapackEntry>> {
    let connection = open_read_only(db_path)?;
    let sql = "SELECT e.remote, e.category, e.package, e.version \
               FROM entries e JOIN files f ON f.entry = e.id \
               WHERE f.path = ?1 LIMIT 1";
    let mut statement = connection.prepare(sql).with_sqlite_path(db_path)?;

    let entry = statement
        .query_row([relative_path], |row| {
            let version_text: String = row.get(3)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                version_text,
            ))
        })
        .optional()
        .with_sqlite_path(db_path)?;

    let Some((remote, category, package, version_text)) = entry else {
        return Ok(None);
    };
    let Ok(version) = Version::parse(&version_text) else {
        return Ok(None);
    };

    Ok(Some(ReapackEntry {
        remote,
        category,
        package,
        version,
    }))
}

fn open_read_only(db_path: &Path) -> Result<Connection> {
    Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).with_sqlite_path(db_path)
}

fn relative_reapack_path(resource_path: &Path, absolute_file: &Path) -> Option<String> {
    let canonical_resource = fs::canonicalize(resource_path)
        .with_path(resource_path)
        .ok()?;
    let canonical_file = fs::canonicalize(absolute_file)
        .with_path(absolute_file)
        .ok()?;
    let relative = canonical_file.strip_prefix(canonical_resource).ok()?;
    Some(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::{
        REAPACK_INI_RELATIVE_PATH, RemoteUpsertOutcome, extract_scr_lines, is_remote_configured,
        list_entries, package_owner_for_file, upsert_remote,
    };

    const TEST_REPO_NAME: &str = "REAPER Accessibility";
    const TEST_REPO_URL: &str = "https://github.com/Timtam/reapack/raw/master/index.xml";

    #[test]
    fn upsert_creates_reapack_ini_when_missing() {
        let dir = tempdir().unwrap();
        let outcome = upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();
        assert_eq!(outcome, RemoteUpsertOutcome::CreatedFile);

        let ini = fs::read_to_string(dir.path().join(REAPACK_INI_RELATIVE_PATH)).unwrap();
        assert!(ini.contains("[remotes]"));
        assert!(ini.contains("size=1"));
        assert!(ini.contains(&format!("remote0={}|{}|1|2", TEST_REPO_NAME, TEST_REPO_URL)));
    }

    #[test]
    fn upsert_is_idempotent_on_url() {
        let dir = tempdir().unwrap();
        let first = upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();
        assert_eq!(first, RemoteUpsertOutcome::CreatedFile);
        let second = upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();
        assert_eq!(second, RemoteUpsertOutcome::AlreadyPresent);

        // The single entry should still be the only one.
        let ini = fs::read_to_string(dir.path().join(REAPACK_INI_RELATIVE_PATH)).unwrap();
        assert_eq!(ini.matches(TEST_REPO_URL).count(), 1);
        assert!(ini.contains("size=1"));
    }

    #[test]
    fn upsert_appends_into_existing_remotes_section() {
        let dir = tempdir().unwrap();
        let ini_path = dir.path().join(REAPACK_INI_RELATIVE_PATH);
        // Mimic an existing reapack.ini with one prior entry.
        fs::write(
            &ini_path,
            "[reapack]\n\
             version=4\n\
             \n\
             [remotes]\n\
             size=1\n\
             remote0=ReaTeam Extensions|https://github.com/ReaTeam/Extensions/raw/master/index.xml|1|2\n",
        )
        .unwrap();

        let outcome = upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();
        assert_eq!(outcome, RemoteUpsertOutcome::Added);

        let ini = fs::read_to_string(&ini_path).unwrap();
        // Original remote preserved.
        assert!(ini.contains("remote0=ReaTeam Extensions|"));
        // Our remote appended at the next index.
        assert!(ini.contains(&format!("remote1={}|{}|1|2", TEST_REPO_NAME, TEST_REPO_URL)));
        // Size bumped.
        assert!(ini.contains("size=2"));
        // The unrelated [reapack] section is untouched.
        assert!(ini.contains("version=4"));
    }

    #[test]
    fn upsert_appends_at_size_when_indexes_have_gaps() {
        // ReaPack's `size` is the authoritative count; existing entries
        // could have gaps if a previous version removed remotes. Verify
        // we honour `size` when picking the next index.
        let dir = tempdir().unwrap();
        let ini_path = dir.path().join(REAPACK_INI_RELATIVE_PATH);
        fs::write(
            &ini_path,
            "[remotes]\n\
             size=10\n\
             remote0=ReaTeam Extensions|https://github.com/ReaTeam/Extensions/raw/master/index.xml|1|2\n\
             remote2=Old Repo|https://example.invalid/index.xml|1|2\n",
        )
        .unwrap();

        upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();

        let ini = fs::read_to_string(&ini_path).unwrap();
        assert!(ini.contains(&format!(
            "remote10={}|{}|1|2",
            TEST_REPO_NAME, TEST_REPO_URL
        )));
        assert!(ini.contains("size=11"));
    }

    #[test]
    fn upsert_creates_remotes_section_when_only_other_sections_exist() {
        let dir = tempdir().unwrap();
        let ini_path = dir.path().join(REAPACK_INI_RELATIVE_PATH);
        fs::write(&ini_path, "[reapack]\nversion=4\nlast_browse=\n").unwrap();

        let outcome = upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();
        assert_eq!(outcome, RemoteUpsertOutcome::Added);

        let ini = fs::read_to_string(&ini_path).unwrap();
        assert!(ini.contains("[reapack]"));
        assert!(ini.contains("version=4"));
        assert!(ini.contains("[remotes]"));
        assert!(ini.contains("size=1"));
        assert!(ini.contains(&format!("remote0={}|{}|1|2", TEST_REPO_NAME, TEST_REPO_URL)));
    }

    #[test]
    fn upsert_tolerates_ansi_encoded_ini_and_preserves_its_bytes() {
        // Regression for issue #7: ReaPack writes reapack.ini via the Win32
        // profile APIs in the ANSI code page, so a repo name with a CP-1252
        // curly apostrophe (0x92) makes the file invalid UTF-8. upsert must
        // neither error nor rewrite the user's bytes.
        let dir = tempdir().unwrap();
        let ini_path = dir.path().join(REAPACK_INI_RELATIVE_PATH);
        let mut original = b"[remotes]\r\nsize=1\r\nremote0=Tom".to_vec();
        original.push(0x92);
        original.extend_from_slice(b"s Repo|https://example.invalid/index.xml|1|2\r\n");
        fs::write(&ini_path, &original).unwrap();

        let outcome = upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();
        assert_eq!(outcome, RemoteUpsertOutcome::Added);

        let written = fs::read(&ini_path).unwrap();
        // The pre-existing ANSI line survives byte-for-byte (0x92 included).
        let mut expected_prefix = b"remote0=Tom".to_vec();
        expected_prefix.push(0x92);
        expected_prefix.extend_from_slice(b"s Repo|");
        assert!(
            written
                .windows(expected_prefix.len())
                .any(|w| w == expected_prefix.as_slice()),
            "original ANSI bytes must survive the rewrite"
        );
        // Our ASCII remote was appended and the size bumped.
        let lossy = String::from_utf8_lossy(&written);
        assert!(lossy.contains(&format!("remote1={}|{}|1|2", TEST_REPO_NAME, TEST_REPO_URL)));
        assert!(lossy.contains("size=2"));

        // And the round trip stays stable: the second run is a no-op.
        let second = upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();
        assert_eq!(second, RemoteUpsertOutcome::AlreadyPresent);
    }

    #[test]
    fn upsert_tolerates_utf16le_ini_and_keeps_it_utf16() {
        // A reapack.ini created with a UTF-16 LE BOM (the Win32 profile APIs
        // keep writing UTF-16 into such files). The upsert must read it,
        // append our remote, and keep the file UTF-16 so ReaPack still
        // parses it afterwards.
        let dir = tempdir().unwrap();
        let ini_path = dir.path().join(REAPACK_INI_RELATIVE_PATH);
        let text = "[remotes]\r\nsize=1\r\nremote0=ReaTeam Extensions|https://example.invalid/index.xml|1|2\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(&ini_path, &bytes).unwrap();

        let outcome = upsert_remote(dir.path(), TEST_REPO_NAME, TEST_REPO_URL).unwrap();
        assert_eq!(outcome, RemoteUpsertOutcome::Added);

        let written = fs::read(&ini_path).unwrap();
        assert_eq!(&written[..2], &[0xFF, 0xFE], "UTF-16 LE BOM preserved");
        let units: Vec<u16> = written[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let decoded = String::from_utf16(&units).unwrap();
        assert!(decoded.contains(&format!("remote1={}|{}|1|2", TEST_REPO_NAME, TEST_REPO_URL)));
        assert!(decoded.contains("size=2"));
    }

    #[test]
    fn is_remote_configured_tolerates_ansi_encoded_ini() {
        let dir = tempdir().unwrap();
        let ini_path = dir.path().join(REAPACK_INI_RELATIVE_PATH);
        let mut bytes = b"[remotes]\r\nsize=2\r\nremote0=Tom".to_vec();
        bytes.push(0x92);
        bytes.extend_from_slice(b"s Repo|https://example.invalid/index.xml|1|2\r\n");
        bytes.extend_from_slice(
            format!("remote1={}|{}|1|2\r\n", TEST_REPO_NAME, TEST_REPO_URL).as_bytes(),
        );
        fs::write(&ini_path, &bytes).unwrap();

        assert!(is_remote_configured(dir.path(), TEST_REPO_URL).unwrap());
        assert!(!is_remote_configured(dir.path(), "https://other.invalid/index.xml").unwrap());
    }

    #[test]
    fn is_remote_configured_reports_false_when_ini_is_missing() {
        let dir = tempdir().unwrap();
        assert!(!is_remote_configured(dir.path(), TEST_REPO_URL).unwrap());
    }

    #[test]
    fn is_remote_configured_reports_true_when_url_present_in_remotes_section() {
        let dir = tempdir().unwrap();
        let ini_path = dir.path().join(REAPACK_INI_RELATIVE_PATH);
        fs::write(
            &ini_path,
            format!(
                "[remotes]\nsize=1\nremote10={}|{}|1|2\n",
                TEST_REPO_NAME, TEST_REPO_URL
            ),
        )
        .unwrap();
        assert!(is_remote_configured(dir.path(), TEST_REPO_URL).unwrap());
    }

    #[test]
    fn is_remote_configured_reports_false_when_remotes_section_lists_other_urls() {
        let dir = tempdir().unwrap();
        let ini_path = dir.path().join(REAPACK_INI_RELATIVE_PATH);
        fs::write(
            &ini_path,
            "[remotes]\nsize=1\nremote0=ReaTeam Extensions|https://github.com/ReaTeam/Extensions/raw/master/index.xml|1|2\n",
        )
        .unwrap();
        assert!(!is_remote_configured(dir.path(), TEST_REPO_URL).unwrap());
    }

    #[test]
    fn extract_scr_lines_returns_only_scr_lines_in_order() {
        let text = "ACT 1 0 \"_RSabc\" \"Custom\" _SWS_ABOUT\r\n\
                    KEY 9 65 _RSabc 0\r\n\
                    SCR 4 0 RSdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef \"Script: foo.lua\" foo.lua\r\n\
                    SCR\t260\t32060\tRScafef00d \"Script: midi.lua\" midi.lua\r\n\
                    # SCR is the keyword we look for\r\n\
                    SCRIPT 1 2 3\r\n";
        let lines = extract_scr_lines(text);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("SCR 4 0 RSdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"));
        assert!(lines[0].contains("foo.lua"));
        assert!(lines[1].starts_with("SCR\t260\t32060"));
    }

    #[test]
    fn extract_scr_lines_handles_empty_and_no_scr_input() {
        assert!(extract_scr_lines("").is_empty());
        assert!(extract_scr_lines("KEY 9 65 40000 0\nACT 1 0 \"_x\" \"y\" 40000\n").is_empty());
    }

    #[test]
    fn reads_package_owner_for_registered_file() {
        let dir = tempdir().unwrap();
        let plugins = dir.path().join("UserPlugins");
        let reapack = dir.path().join("ReaPack");
        fs::create_dir_all(&plugins).unwrap();
        fs::create_dir_all(&reapack).unwrap();
        let sws_file = plugins.join("reaper_sws-x64.dll");
        fs::write(&sws_file, b"").unwrap();

        let connection = Connection::open(reapack.join("registry.db")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE entries (
                    id INTEGER PRIMARY KEY,
                    remote TEXT NOT NULL,
                    category TEXT NOT NULL,
                    package TEXT NOT NULL,
                    desc TEXT NOT NULL,
                    type INTEGER NOT NULL,
                    version TEXT NOT NULL,
                    author TEXT NOT NULL,
                    flags INTEGER DEFAULT 0,
                    UNIQUE(remote, category, package)
                );
                CREATE TABLE files (
                    id INTEGER PRIMARY KEY,
                    entry INTEGER NOT NULL,
                    path TEXT UNIQUE NOT NULL,
                    main INTEGER NOT NULL,
                    type INTEGER NOT NULL,
                    FOREIGN KEY(entry) REFERENCES entries(id)
                );
                INSERT INTO entries(id, remote, category, package, desc, type, version, author, flags)
                VALUES(1, 'ReaTeam Extensions', 'Extensions', 'SWS/S&M Extension', '', 0, '2.14.0.7', '', 0);
                INSERT INTO files(id, entry, path, main, type)
                VALUES(1, 1, 'UserPlugins/reaper_sws-x64.dll', 1, 0);",
            )
            .unwrap();

        let owner = package_owner_for_file(dir.path(), &sws_file)
            .unwrap()
            .unwrap();
        assert_eq!(owner.package, "SWS/S&M Extension");
        assert_eq!(owner.version.raw(), "2.14.0.7");

        let entries = list_entries(dir.path()).unwrap();
        assert_eq!(entries.len(), 1);
    }
}
