//! Two install failures a user can act on, in plain words: a vendor's
//! installer that ended with an exit code, and programs that have to be
//! closed before an installer can run. The English text goes into the
//! report item's `message`; [`localized_install_problem`] gives the same
//! text in the user's language, for the wizard and the CLI alike.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::PackageOperationMessage;
use crate::localization::Localizer;

/// What a vendor installer's exit code means, for the installers whose
/// codes RABBIT knows. Today that is Inno Setup, which runs Surge XT's
/// install on Windows; its codes are documented under "Setup Exit Codes".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallerExitMeaning {
    /// Inno Setup 2: cancelled before it began installing. Under
    /// `/VERYSILENT /SUPPRESSMSGBOXES` there is nothing left to cancel but
    /// the Windows permission prompt Inno raises itself, so this is in
    /// practice a declined prompt: Inno ends with 2 then, not with 1223.
    CancelledBeforeInstalling,
    /// Inno Setup 5: it stopped at a question that nobody can answer while
    /// it runs unattended, `/SUPPRESSMSGBOXES` answered Abort, and it
    /// rolled back. Usually another program holds one of its files open.
    StoppedAtQuestion,
    /// Inno Setup 7: its "preparing to install" step found that it can't
    /// continue.
    PreparationFailed,
    /// Inno Setup 8: the same step found that Windows has to restart first.
    RestartNeeded,
}

/// The meaning of Inno Setup's exit code `exit_code`, for the codes a user
/// can do something about.
pub(super) fn inno_setup_exit_meaning(exit_code: i32) -> Option<InstallerExitMeaning> {
    match exit_code {
        2 => Some(InstallerExitMeaning::CancelledBeforeInstalling),
        5 => Some(InstallerExitMeaning::StoppedAtQuestion),
        7 => Some(InstallerExitMeaning::PreparationFailed),
        8 => Some(InstallerExitMeaning::RestartNeeded),
        _ => None,
    }
}

/// The English text of [`PackageOperationMessage::VendorInstallerFailed`]
/// for `package`. Says the same as the en-US Fluent keys
/// [`localized_install_problem`] uses. Like every package status message,
/// it doesn't start with the package's name: the wizard puts the name in
/// front of it, and the report and the CLI show the package beside it.
pub(super) fn vendor_installer_failed_text(
    package: &str,
    exit_code: i32,
    meaning: Option<InstallerExitMeaning>,
    log_path: Option<&Path>,
) -> String {
    let explanation = match meaning {
        Some(InstallerExitMeaning::CancelledBeforeInstalling) => {
            "That usually means the Windows permission prompt was declined, so nothing changed. \
             Run RABBIT again and choose Yes when Windows asks for permission."
                .to_string()
        }
        Some(InstallerExitMeaning::StoppedAtQuestion) => format!(
            "That means it stopped at a question it can't ask while RABBIT runs it \
             unattended, usually because another program has one of its files open. Close \
             other programs, especially other music programs and {package}, then run RABBIT \
             again."
        ),
        Some(InstallerExitMeaning::PreparationFailed) => {
            "That means it found, while getting ready, that it can't install right now. \
             Restart the computer, then run RABBIT again."
                .to_string()
        }
        Some(InstallerExitMeaning::RestartNeeded) => {
            "That means it found, while getting ready, that Windows has to restart first. \
             Restart the computer, then run RABBIT again."
                .to_string()
        }
        None => format!(
            "Close other programs and run RABBIT again. If it stops again, install {package} \
             from its publisher's website."
        ),
    };
    let mut text = format!("The installer ended with exit code {exit_code}. {explanation}");
    if let Some(path) = log_path {
        text.push_str(&format!(" The installer's log is at {}.", path.display()));
    }
    text
}

/// The English text of [`PackageOperationMessage::BlockingProgramsRunning`]
/// for `package`. Says the same as the en-US Fluent key.
pub(super) fn blocking_programs_running_text(package: &str, programs: &[String]) -> String {
    format!(
        "RABBIT didn't start the installer because {package} is open ({}), and the installer \
         can't replace a program that is running. Close it, then run RABBIT again.",
        programs.join(", ")
    )
}

/// The text of `message` in `localizer`'s language, with `package` as the
/// package's name, for the messages this module explains:
/// [`PackageOperationMessage::VendorInstallerFailed`] and
/// [`PackageOperationMessage::BlockingProgramsRunning`]. `None` for every
/// other message, and when the locale lacks one of the keys, so the caller
/// shows the item's English `message` instead.
pub fn localized_install_problem(
    localizer: &Localizer,
    message: &PackageOperationMessage,
    package: &str,
) -> Option<String> {
    let sentence = |key: &str, arguments: &[(&str, &str)]| {
        let text = localizer.format(key, arguments);
        (!text.missing).then_some(text.value)
    };
    match message {
        PackageOperationMessage::VendorInstallerFailed {
            exit_code,
            meaning,
            log_path,
            ..
        } => {
            let code = exit_code.to_string();
            let mut sentences = vec![sentence(
                "package-status-vendor-installer-failed",
                &[("code", &code)],
            )?];
            let explanation = match meaning {
                Some(InstallerExitMeaning::CancelledBeforeInstalling) => {
                    "package-status-vendor-exit-cancelled-before-installing"
                }
                Some(InstallerExitMeaning::StoppedAtQuestion) => {
                    "package-status-vendor-exit-stopped-at-question"
                }
                Some(InstallerExitMeaning::PreparationFailed) => {
                    "package-status-vendor-exit-preparation-failed"
                }
                Some(InstallerExitMeaning::RestartNeeded) => {
                    "package-status-vendor-exit-restart-needed"
                }
                None => "package-status-vendor-exit-unknown",
            };
            sentences.push(sentence(explanation, &[("package", package)])?);
            if let Some(path) = log_path {
                let path = path.display().to_string();
                sentences.push(sentence(
                    "package-status-vendor-installer-log",
                    &[("path", &path)],
                )?);
            }
            Some(sentences.join(" "))
        }
        PackageOperationMessage::BlockingProgramsRunning { programs } => sentence(
            "package-status-blocking-programs-running",
            &[("package", package), ("programs", &programs.join(", "))],
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        InstallerExitMeaning, blocking_programs_running_text, inno_setup_exit_meaning,
        localized_install_problem, vendor_installer_failed_text,
    };
    use crate::localization::{DEFAULT_LOCALE, Localizer, embedded_locales};
    use crate::operation::PackageOperationMessage;

    const LOG: &str =
        r"C:\Users\Toni Barth\AppData\Roaming\REAPER\RABBIT\logs\surge-xt-installer-unix-1.log";

    fn failed(exit_code: i32, log_path: Option<&str>) -> PackageOperationMessage {
        PackageOperationMessage::VendorInstallerFailed {
            program: r"C:\cache\surge-xt-setup.exe".to_string(),
            exit_code,
            meaning: inno_setup_exit_meaning(exit_code),
            log_path: log_path.map(PathBuf::from),
        }
    }

    /// Only the Inno Setup codes a user can act on get a meaning.
    #[test]
    fn inno_setup_codes_that_matter_get_a_meaning() {
        assert_eq!(
            inno_setup_exit_meaning(2),
            Some(InstallerExitMeaning::CancelledBeforeInstalling)
        );
        assert_eq!(
            inno_setup_exit_meaning(5),
            Some(InstallerExitMeaning::StoppedAtQuestion)
        );
        assert_eq!(
            inno_setup_exit_meaning(7),
            Some(InstallerExitMeaning::PreparationFailed)
        );
        assert_eq!(
            inno_setup_exit_meaning(8),
            Some(InstallerExitMeaning::RestartNeeded)
        );
        for code in [0, 1, 3, 4, 6, 9, 1223, -1] {
            assert_eq!(inno_setup_exit_meaning(code), None, "{code}");
        }
    }

    /// The English text in the report says what the en-US keys say, for
    /// every meaning, with and without a log, so the CLI and the wizard
    /// read the same in English as the saved report.
    #[test]
    fn the_english_text_matches_the_en_us_keys() {
        let english = Localizer::embedded(DEFAULT_LOCALE).unwrap();
        for code in [2, 5, 7, 8, 1] {
            for log in [None, Some(LOG)] {
                let message = failed(code, log);
                let PackageOperationMessage::VendorInstallerFailed {
                    exit_code,
                    meaning,
                    log_path,
                    ..
                } = &message
                else {
                    unreachable!()
                };
                assert_eq!(
                    localized_install_problem(&english, &message, "Surge XT").unwrap(),
                    vendor_installer_failed_text(
                        "Surge XT",
                        *exit_code,
                        *meaning,
                        log_path.as_deref()
                    ),
                    "{code} {log:?}"
                );
            }
        }
        let programs = vec![
            "Surge XT.exe".to_string(),
            "Surge XT Effects.exe".to_string(),
        ];
        assert_eq!(
            localized_install_problem(
                &english,
                &PackageOperationMessage::BlockingProgramsRunning {
                    programs: programs.clone()
                },
                "Surge XT"
            )
            .unwrap(),
            blocking_programs_running_text("Surge XT", &programs)
        );
    }

    /// Exit code 5, the one the hardware pass hit, tells the user what to
    /// close, and code 2, a declined permission prompt, to choose Yes. Every
    /// locale names the code and the log, and the package where the advice
    /// needs it.
    #[test]
    fn every_locale_explains_an_installer_exit_code() {
        let english = Localizer::embedded(DEFAULT_LOCALE).unwrap();
        let text = localized_install_problem(&english, &failed(5, Some(LOG)), "Surge XT").unwrap();
        assert_eq!(
            text,
            format!(
                "The installer ended with exit code 5. That means it stopped at a question it \
                 can't ask while RABBIT runs it unattended, usually because another program has \
                 one of its files open. Close other programs, especially other music programs \
                 and Surge XT, then run RABBIT again. The installer's log is at {LOG}."
            )
        );
        assert_eq!(
            localized_install_problem(&english, &failed(2, None), "Surge XT").unwrap(),
            "The installer ended with exit code 2. That usually means the Windows permission \
             prompt was declined, so nothing changed. Run RABBIT again and choose Yes when \
             Windows asks for permission."
        );

        for &locale in embedded_locales() {
            let localizer = Localizer::embedded(locale).unwrap();
            let mut seen = Vec::new();
            for code in [2, 5, 7, 8, 1] {
                for log in [None, Some(LOG)] {
                    let text =
                        localized_install_problem(&localizer, &failed(code, log), "Surge XT")
                            .unwrap_or_else(|| panic!("{locale} lacks a key for code {code}"));
                    assert!(text.contains(&code.to_string()), "{locale}: {text}");
                    // The wizard puts the name in front, so the message
                    // names the package only where the advice needs it.
                    assert_eq!(
                        text.contains("Surge XT"),
                        matches!(code, 5 | 1),
                        "{locale}: {text}"
                    );
                    assert!(!text.contains("Some("), "{locale}: {text}");
                    assert_eq!(text.contains(LOG), log.is_some(), "{locale}: {text}");
                    if log.is_none() {
                        seen.push(text);
                    }
                }
            }
            // Each meaning, and the fallback, reads differently.
            let distinct: std::collections::BTreeSet<_> = seen.iter().collect();
            assert_eq!(distinct.len(), seen.len(), "{locale}: {seen:#?}");
        }
    }

    /// Every locale names what is open: the package and its programs.
    #[test]
    fn every_locale_names_the_programs_to_close() {
        let message = PackageOperationMessage::BlockingProgramsRunning {
            programs: vec!["Surge XT.exe".to_string()],
        };
        for &locale in embedded_locales() {
            let localizer = Localizer::embedded(locale).unwrap();
            let text = localized_install_problem(&localizer, &message, "Surge XT")
                .unwrap_or_else(|| panic!("{locale} lacks the key"));
            assert!(
                text.contains("Surge XT.exe") && text.contains("Surge XT "),
                "{locale}: {text}"
            );
        }
    }

    /// Other messages are left to the caller.
    #[test]
    fn other_messages_are_not_explained_here() {
        let english = Localizer::embedded(DEFAULT_LOCALE).unwrap();
        for message in [
            PackageOperationMessage::UnattendedInstalled,
            PackageOperationMessage::InstallFailed {
                error: "x".to_string(),
                antivirus_block: false,
            },
        ] {
            assert_eq!(localized_install_problem(&english, &message, "OSARA"), None);
        }
    }
}
