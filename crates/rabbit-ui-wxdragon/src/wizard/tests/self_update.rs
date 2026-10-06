use std::path::PathBuf;

use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};

use super::support::*;
use crate::wizard::*;

#[test]
fn apply_summary_appends_signed_count_when_signatures_were_verified() {
    use rabbit_core::self_update::{ReplacedFile, SignatureVerdictRecord};
    use rabbit_core::signature::SignatureVerdict;

    let report = sample_apply_report(
        vec![ReplacedFile {
            install_path: PathBuf::from("/install/RABBIT"),
            backup_path: PathBuf::from("/install/RABBIT.rabbit-old"),
        }],
        vec![SignatureVerdictRecord {
            source_path: PathBuf::from("/staging/RABBIT"),
            verdict: SignatureVerdict::Signed {
                details: "valid on disk".to_string(),
            },
        }],
    );

    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let summary = format_self_update_apply_summary(&localizer, &report);
    assert!(summary.contains("Replaced 1 file(s)"));
    assert!(summary.contains("Signature verification: 1 signed."));
}

#[test]
fn apply_summary_omits_signature_clause_when_no_verdicts_recorded() {
    use rabbit_core::self_update::ReplacedFile;

    let report = sample_apply_report(
        vec![ReplacedFile {
            install_path: PathBuf::from("/install/RABBIT"),
            backup_path: PathBuf::from("/install/RABBIT.rabbit-old"),
        }],
        Vec::new(),
    );

    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let summary = format_self_update_apply_summary(&localizer, &report);
    assert!(summary.contains("Replaced 1 file(s)"));
    assert!(!summary.contains("Signature verification"));
}

#[test]
fn apply_summary_reports_signed_and_unsigned_split() {
    use rabbit_core::self_update::{ReplacedFile, SignatureVerdictRecord};
    use rabbit_core::signature::SignatureVerdict;

    let report = sample_apply_report(
        vec![
            ReplacedFile {
                install_path: PathBuf::from("/install/RABBIT"),
                backup_path: PathBuf::from("/install/RABBIT.rabbit-old"),
            },
            ReplacedFile {
                install_path: PathBuf::from("/install/rabbit-cli"),
                backup_path: PathBuf::from("/install/rabbit-cli.rabbit-old"),
            },
        ],
        vec![
            SignatureVerdictRecord {
                source_path: PathBuf::from("/staging/RABBIT"),
                verdict: SignatureVerdict::Signed {
                    details: "ok".to_string(),
                },
            },
            SignatureVerdictRecord {
                source_path: PathBuf::from("/staging/rabbit-cli"),
                verdict: SignatureVerdict::Unsigned {
                    reason: "no signtool".to_string(),
                },
            },
        ],
    );

    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let summary = format_self_update_apply_summary(&localizer, &report);
    assert!(summary.contains("Signature verification: 1 signed, 1 unsigned."));
}
