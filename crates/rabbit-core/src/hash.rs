use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{IoPathContext, Result};

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_path(path)?;
    sha256_reader(&mut file).with_path(path)
}

/// SHA-256 of everything `reader` yields from where it stands, hex-encoded.
/// For a file already held open, such as one locked against changes
/// between its hash check and its start.
pub fn sha256_reader(reader: &mut dyn Read) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(to_hex(&hasher.finalize()))
}

/// SHA-256 of an in-memory buffer, hex-encoded. Used for artifacts whose
/// identity IS their content (community language packs published at a fixed
/// URL with no version string).
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    to_hex(&hasher.finalize())
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{sha256_bytes, sha256_file, sha256_reader};

    #[test]
    fn hashes_file_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sample.txt");
        fs::write(&file, b"rabbit").unwrap();

        assert_eq!(
            sha256_file(&file).unwrap(),
            "d37d96b42ad43384915e4513505c30c0b1c4e7c765b5577eda25b5dbd7f26d89"
        );
    }

    #[test]
    fn hashes_in_memory_bytes_like_a_file() {
        // Content-hash "versions" are computed from bytes in memory; they
        // must match what hashing the same bytes on disk produces.
        let dir = tempdir().unwrap();
        let file = dir.path().join("sample.txt");
        fs::write(&file, b"rabbit").unwrap();

        assert_eq!(sha256_bytes(b"rabbit"), sha256_file(&file).unwrap());
    }

    #[test]
    fn hashes_an_open_reader_like_a_file() {
        assert_eq!(
            sha256_reader(&mut &b"rabbit"[..]).unwrap(),
            sha256_bytes(b"rabbit")
        );
    }
}
