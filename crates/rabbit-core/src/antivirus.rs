//! Antivirus interoperability for the download cache.
//!
//! Thin domain wrapper over the platform Defender-exclusion primitive so
//! callers (the wizard) stay on `rabbit_core::*` and don't need a direct
//! `rabbit-platform` dependency. See
//! [`rabbit_platform::defender`] for the rationale and behavior.

use std::path::Path;

pub use rabbit_platform::DefenderExclusionOutcome;

/// Best-effort: ensure RABBIT's download cache folder is excluded from
/// Windows Defender so freshly built, low-prevalence (but code-signed)
/// installers — OSARA's snapshots especially — aren't quarantined as false
/// positives mid-download. Adds the exclusion once, under a single
/// administrator prompt, and only when it isn't already present; a no-op
/// (returning [`DefenderExclusionOutcome::Unsupported`]) off Windows. The
/// caller must treat every outcome as non-fatal.
pub fn ensure_cache_excluded_from_antivirus(cache_dir: &Path) -> DefenderExclusionOutcome {
    rabbit_platform::ensure_path_excluded(cache_dir)
}
