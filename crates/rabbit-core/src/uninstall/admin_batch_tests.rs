//! The administrator command's pure parts, on every platform: which steps
//! root is given and in what order, the exact shell text, how the run reads
//! what the command did, the name an app gets in the Trash, and the
//! stand-ins tests run the command and the Trash with. Nothing here asks
//! for a password or touches anything but temporary folders.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rabbit_platform::{
    AdministratorShellRequest, ElevationError, ShellCommand, ShellLinesOutcome, ShellStep,
    administrator_shell_text,
};
use tempfile::tempdir;

use super::admin_batch::{
    AdminPackage, AdminPackageResult, AdminShellEvent, AdminShellRun, AdminStepMeaning, AdminWork,
    AdministratorShell, AdministratorStepKind, AdministratorStepRecord, AdministratorStepVerdict,
    MV, NoAdministratorShell, PGREP, PKGUTIL, RM, RecordingAdministratorShell, StepError,
    SystemAdministratorShell, TEST, TrashMove, administrator_results, administrator_steps,
    classify_step_error, shell_run_from_error, trash_name, verdicts_after,
};
use super::macos_objects::{
    FakeMacosObjects, MacosObjects, NoMacosObjects, SystemMacosObjects, TrashErrorKind,
};
use super::paths::link_folder;
use super::tests::write;

const SURGE_APP: &str = "/Applications/Surge.app";
const SURGE_EFFECTS: &str = "/Applications/Surge Effects.app";
const SURGE_VST3: &str = "/Library/Audio/Plug-Ins/VST3/Surge.vst3";
const APP_RECEIPT: &str = "org.example.surge.app.pkg";
const VST3_RECEIPT: &str = "org.example.surge.vst3.pkg";
const REAPER_APP: &str = "/Applications/REAPER.app";
const TRASH: &str = "/Users/me/.Trash";
const TRASHED: &str = "/Users/me/.Trash/REAPER 2026-11-07 15.30.12.app";
/// Where BSD `mv` puts the app when a folder already has the fresh name.
const TRASHED_INSIDE: &str = "/Users/me/.Trash/REAPER 2026-11-07 15.30.12.app/REAPER.app";

fn run(program: &str, arguments: &[&str]) -> ShellStep {
    ShellStep::Run(ShellCommand::new(program, arguments.iter().copied()))
}

fn tolerated(program: &str, arguments: &[&str]) -> ShellStep {
    ShellStep::RunIgnoringFailure(ShellCommand::new(program, arguments.iter().copied()))
}

fn expecting_no_match(program: &str, arguments: &[&str]) -> ShellStep {
    ShellStep::RunExpectingNoMatch(ShellCommand::new(program, arguments.iter().copied()))
}

/// A package shaped like Surge XT's: apps first, the VST3 anchor last.
fn surge(trees: &[&str]) -> AdminPackage {
    AdminPackage {
        package_id: "surge".to_string(),
        blocking_programs: vec!["Surge".to_string(), "Surge Effects".to_string()],
        every_reaper_closed: true,
        work: AdminWork::Delete {
            trees: trees.iter().map(ToString::to_string).collect(),
            forget: vec![APP_RECEIPT.to_string(), VST3_RECEIPT.to_string()],
        },
    }
}

/// REAPER's app, which only root can move.
fn reaper_move() -> AdminPackage {
    AdminPackage {
        package_id: "reaper".to_string(),
        blocking_programs: Vec::new(),
        every_reaper_closed: true,
        work: AdminWork::MoveToTrash(Some(TrashMove {
            app: REAPER_APP.to_string(),
            trash: TRASH.to_string(),
            destination: TRASHED.to_string(),
        })),
    }
}

fn listed(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(ToString::to_string).collect()
}

#[test]
fn root_checks_for_running_programs_first_then_deletes_then_forgets_then_moves_reaper() {
    let batch = administrator_steps(
        &[
            surge(&[SURGE_APP, SURGE_EFFECTS, SURGE_VST3]),
            reaper_move(),
        ],
        Some(&listed(&[APP_RECEIPT, "com.apple.pkg.Other"])),
    )
    .unwrap();

    assert_eq!(
        batch.steps,
        [
            expecting_no_match(
                PGREP,
                &["-x", "-i", "--", "Surge|Surge Effects|REAPER|reaper64"]
            ),
            run(RM, &["-R", "-f", "-x", SURGE_APP]),
            run(RM, &["-R", "-f", "-x", SURGE_EFFECTS]),
            run(RM, &["-R", "-f", "-x", SURGE_VST3]),
            // Only the receipt pkgutil listed; the other is already gone.
            tolerated(PKGUTIL, &["--forget", APP_RECEIPT]),
            run(TEST, &["-d", TRASH]),
            run(TEST, &["!", "-L", TRASH]),
            run(MV, &["-h", "-n", REAPER_APP, TRASHED]),
        ]
    );
    assert_eq!(batch.packages, ["surge", "reaper"]);
    assert_eq!(batch.meaning.len(), batch.steps.len());
    assert_eq!(
        batch.meaning[0],
        AdminStepMeaning::ProgramsClosed {
            names: ["Surge", "Surge Effects", "REAPER", "reaper64"]
                .map(String::from)
                .to_vec()
        }
    );
    assert_eq!(
        batch.meaning[7],
        AdminStepMeaning::MoveToTrash {
            package_id: "reaper".to_string(),
            from: PathBuf::from(REAPER_APP),
            to: PathBuf::from(TRASHED),
        }
    );
}

/// The exact text root's shell runs, every word single-quoted, the
/// program-check step failing closed on anything but "no match".
#[test]
fn the_shell_text_root_runs_is_exactly_this() {
    let batch =
        administrator_steps(&[surge(&[SURGE_APP]), reaper_move()], Some(&listed(&[]))).unwrap();
    assert_eq!(
        administrator_shell_text(&batch.steps).unwrap(),
        "cd '/' || exit 100; exec >/dev/null; \
         '/usr/bin/pgrep' '-x' '-i' '--' 'Surge|Surge Effects|REAPER|reaper64'; [ \"$?\" -eq 1 ] || exit 101; \
         '/bin/rm' '-R' '-f' '-x' '/Applications/Surge.app' || exit 102; \
         '/bin/test' '-d' '/Users/me/.Trash' || exit 103; \
         '/bin/test' '!' '-L' '/Users/me/.Trash' || exit 104; \
         '/bin/mv' '-h' '-n' '/Applications/REAPER.app' '/Users/me/.Trash/REAPER 2026-11-07 15.30.12.app' || exit 105"
    );
}

#[test]
fn every_receipt_is_forgotten_when_listing_them_failed() {
    let batch = administrator_steps(&[surge(&[SURGE_APP])], None).unwrap();
    assert_eq!(
        batch.steps[2..],
        [
            tolerated(PKGUTIL, &["--forget", APP_RECEIPT]),
            tolerated(PKGUTIL, &["--forget", VST3_RECEIPT]),
        ]
    );
}

/// A package root deletes `tree` for, with `programs` to be closed.
fn deleting(id: &str, tree: &str, programs: &[&str], every_reaper_closed: bool) -> AdminPackage {
    AdminPackage {
        package_id: id.to_string(),
        blocking_programs: programs.iter().map(ToString::to_string).collect(),
        every_reaper_closed,
        work: AdminWork::Delete {
            trees: vec![tree.to_string()],
            forget: Vec::new(),
        },
    }
}

#[test]
fn programs_are_looked_for_once_and_reaper_only_when_it_matters() {
    let batch = administrator_steps(
        &[
            deleting("a", SURGE_APP, &["Surge", "Shared"], false),
            deleting("b", SURGE_EFFECTS, &["surge", "Other"], false),
        ],
        None,
    )
    .unwrap();
    assert_eq!(
        batch.steps[0],
        expecting_no_match(PGREP, &["-x", "-i", "--", "Surge|Shared|Other"])
    );
    assert_eq!(batch.steps.len(), 3);

    // Nothing to look for: no program check at all.
    let batch = administrator_steps(&[deleting("a", SURGE_APP, &[], false)], None).unwrap();
    assert_eq!(batch.steps, [run(RM, &["-R", "-f", "-x", SURGE_APP])]);
    assert_eq!(batch.packages, ["a"]);
}

/// A package whose trees are all gone already, with no receipt left to
/// forget, gives root nothing to do: no step of its own, and none of its
/// programs or REAPER's in the check before.
#[test]
fn a_package_root_has_nothing_left_to_do_for_adds_no_step() {
    let gone_reaper = AdminPackage {
        work: AdminWork::MoveToTrash(None),
        ..reaper_move()
    };
    let forgotten_already = AdminPackage {
        package_id: "forgotten".to_string(),
        blocking_programs: vec!["Forgotten".to_string()],
        every_reaper_closed: true,
        work: AdminWork::Delete {
            trees: Vec::new(),
            forget: vec![VST3_RECEIPT.to_string()],
        },
    };
    let batch = administrator_steps(
        &[
            deleting("a", SURGE_APP, &["Surge"], false),
            gone_reaper.clone(),
            forgotten_already.clone(),
        ],
        Some(&listed(&[APP_RECEIPT])),
    )
    .unwrap();
    assert_eq!(
        batch.steps,
        [
            expecting_no_match(PGREP, &["-x", "-i", "--", "Surge"]),
            run(RM, &["-R", "-f", "-x", SURGE_APP]),
        ]
    );
    assert_eq!(batch.packages, ["a", "reaper", "forgotten"]);

    // Alone, they leave the command empty.
    let batch = administrator_steps(&[gone_reaper], None).unwrap();
    assert!(batch.steps.is_empty() && batch.meaning.is_empty());
    // A receipt that may still be listed is root's to forget.
    let batch = administrator_steps(&[forgotten_already], None).unwrap();
    assert_eq!(
        batch.steps,
        [
            expecting_no_match(PGREP, &["-x", "-i", "--", "Forgotten|REAPER|reaper64"]),
            tolerated(PKGUTIL, &["--forget", VST3_RECEIPT]),
        ]
    );
}

/// What rabbit-platform would refuse to hand root, such as a line break in
/// a path, is refused while the steps are built, before any prompt is
/// announced; the stand-in for tests refuses it as the real shell does.
#[test]
fn a_word_the_administrator_shell_would_refuse_is_refused_before_any_prompt() {
    for (app, destination) in [
        ("/Applications/REAPER\r.app", TRASHED),
        (REAPER_APP, "/Users/me/.Trash/REAPER\n2026.app"),
        (REAPER_APP, "/Users/me/.Trash/REAPER\u{0}.app"),
    ] {
        let package = AdminPackage {
            work: AdminWork::MoveToTrash(Some(TrashMove {
                app: app.to_string(),
                trash: TRASH.to_string(),
                destination: destination.to_string(),
            })),
            ..reaper_move()
        };
        let error = administrator_steps(&[package], None).unwrap_err();
        assert!(error.contains("can't be given to root"), "{error}");
    }

    let shell = RecordingAdministratorShell::new();
    let answer = shell.run(
        &request(vec![run(
            RM,
            &["-R", "-f", "-x", "/Applications/Surge\n.app"],
        )]),
        &mut no_events(),
    );
    assert!(
        matches!(&answer, AdminShellRun::NotRun { reason } if reason.contains("line break")),
        "{answer:?}"
    );
}

#[test]
fn a_name_that_could_change_the_pattern_or_an_option_is_refused_before_any_prompt() {
    for name in ["Surge.*", "a|b", "(x)", "", "  "] {
        let package = AdminPackage {
            blocking_programs: vec![name.to_string()],
            ..surge(&[SURGE_APP])
        };
        let error = administrator_steps(&[package], None).unwrap_err();
        assert!(error.contains("pgrep"), "{name:?}: {error}");
    }
    for id in ["-x", ".hidden", "org.example;rm", "org example"] {
        let package = AdminPackage {
            work: AdminWork::Delete {
                trees: Vec::new(),
                forget: vec![id.to_string()],
            },
            ..surge(&[])
        };
        let error = administrator_steps(&[package], None).unwrap_err();
        assert!(error.contains("pkgutil"), "{id:?}: {error}");
    }
}

#[test]
fn more_steps_than_one_prompt_takes_are_refused() {
    let trees: Vec<String> = (0..154)
        .map(|n| format!("/Applications/Surge {n}.app"))
        .collect();
    let trees: Vec<&str> = trees.iter().map(String::as_str).collect();
    let error = administrator_steps(&[surge(&trees)], Some(&listed(&[]))).unwrap_err();
    assert!(error.contains("155 steps"), "{error}");
    assert!(administrator_steps(&[surge(&trees[1..])], Some(&listed(&[]))).is_ok());
}

/// 2026-11-07 14:30:12 UTC.
fn moment() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_794_061_812)
}

#[test]
fn an_app_moved_by_root_gets_its_name_with_the_local_time() {
    let app = Path::new("/Applications/REAPER.app");
    assert_eq!(
        trash_name(app, moment(), 3600).as_deref(),
        Some("REAPER 2026-11-07 15.30.12.app")
    );
    assert_eq!(
        trash_name(app, moment(), 0).as_deref(),
        Some("REAPER 2026-11-07 14.30.12.app")
    );
    // Across midnight, back into the day before.
    assert_eq!(
        trash_name(app, moment(), -15 * 3600).as_deref(),
        Some("REAPER 2026-11-06 23.30.12.app")
    );
    assert_eq!(
        trash_name(Path::new("/Applications/REAPER64.app"), UNIX_EPOCH, 0).as_deref(),
        Some("REAPER64 1970-01-01 00.00.00.app")
    );
    // A leap day.
    assert_eq!(
        trash_name(app, UNIX_EPOCH + Duration::from_secs(1_709_208_000), 0).as_deref(),
        Some("REAPER 2024-02-29 12.00.00.app")
    );
}

/// What the filesystem says after the command, for these paths.
fn presence_of(paths: &[(&str, Option<bool>)]) -> impl Fn(&Path) -> Option<bool> {
    let known: HashMap<PathBuf, Option<bool>> = paths
        .iter()
        .map(|(path, there)| (PathBuf::from(path), *there))
        .collect();
    move |path: &Path| known.get(path).copied().unwrap_or(Some(true))
}

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

fn batch_of_surge_and_reaper() -> super::admin_batch::AdminBatch {
    administrator_steps(
        &[
            surge(&[SURGE_APP, SURGE_EFFECTS, SURGE_VST3]),
            reaper_move(),
        ],
        Some(&listed(&[APP_RECEIPT, VST3_RECEIPT])),
    )
    .unwrap()
}

#[test]
fn a_command_that_completed_removed_what_is_gone() {
    let batch = batch_of_surge_and_reaper();
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::Completed,
        None,
        &presence_of(&[
            (SURGE_APP, Some(false)),
            (SURGE_EFFECTS, Some(false)),
            (SURGE_VST3, Some(false)),
            (REAPER_APP, Some(false)),
            (TRASHED, Some(true)),
            (TRASHED_INSIDE, Some(false)),
        ]),
    );
    assert_eq!(
        results,
        [
            (
                "surge".to_string(),
                AdminPackageResult::Removed {
                    gone: paths(&[SURGE_APP, SURGE_EFFECTS, SURGE_VST3]),
                    trashed_to: None,
                    ended_unclearly: None,
                }
            ),
            (
                "reaper".to_string(),
                AdminPackageResult::Removed {
                    gone: paths(&[REAPER_APP]),
                    trashed_to: Some(PathBuf::from(TRASHED)),
                    ended_unclearly: None,
                }
            ),
        ]
    );
}

/// The Trash may be a place macOS keeps private: the app is gone from its
/// place, so it is removed, but RABBIT names no place it couldn't see.
#[test]
fn a_trash_place_rabbit_cant_see_is_never_claimed() {
    let batch = administrator_steps(&[reaper_move()], None).unwrap();
    for unseen in [None, Some(false)] {
        let results = administrator_results(
            &batch,
            ShellLinesOutcome::Completed,
            None,
            &presence_of(&[(REAPER_APP, Some(false)), (TRASHED, unseen)]),
        );
        assert_eq!(
            results[0].1,
            AdminPackageResult::Removed {
                gone: paths(&[REAPER_APP]),
                trashed_to: None,
                ended_unclearly: None,
            }
        );
    }
}

/// `mv -n` leaves both alone, successfully, when the name is taken: the
/// filesystem, not the status, says REAPER is still there. The command
/// never stopped, so no step is named as where it stopped, and it said how
/// it ended.
#[test]
fn a_move_that_completed_but_left_the_app_failed() {
    let batch = administrator_steps(&[reaper_move()], None).unwrap();
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::Completed,
        None,
        &presence_of(&[]),
    );
    assert_eq!(
        results[0].1,
        AdminPackageResult::Failed {
            path: None,
            ended_unclearly: false,
            gone: Vec::new(),
            left: paths(&[REAPER_APP]),
            refused_by_macos: false,
            detail: None,
        }
    );
}

/// The app is gone, but root's move never ran, or may not have, and
/// RABBIT can't see it in the Trash: something else may have moved it, so
/// RABBIT can't say it moved REAPER to the Trash.
#[test]
fn an_app_gone_without_a_move_rabbit_knows_ran_is_unclear() {
    let batch = administrator_steps(&[reaper_move()], None).unwrap();
    let mv = batch
        .meaning
        .iter()
        .position(|meaning| matches!(meaning, AdminStepMeaning::MoveToTrash { .. }))
        .unwrap();
    let test_d = batch
        .meaning
        .iter()
        .position(|meaning| matches!(meaning, AdminStepMeaning::TrashIsFolder { .. }))
        .unwrap();
    let unseen = presence_of(&[(REAPER_APP, Some(false)), (TRASHED, None)]);
    for outcome in [
        ShellLinesOutcome::FailedAt { index: mv },
        ShellLinesOutcome::FailedAt { index: test_d },
        ShellLinesOutcome::Stopped { code: Some(255) },
    ] {
        let results = administrator_results(&batch, outcome, None, &unseen);
        assert_eq!(
            results[0].1,
            AdminPackageResult::TrashUnclear {
                path: PathBuf::from(REAPER_APP)
            },
            "{outcome:?}"
        );
    }

    // A move that ran, with the app gone: moved, though the Trash is
    // private.
    let results = administrator_results(&batch, ShellLinesOutcome::Completed, None, &unseen);
    assert!(matches!(
        &results[0].1,
        AdminPackageResult::Removed {
            trashed_to: None,
            ..
        }
    ));
}

/// BSD `mv` moves the app into a folder that already has the fresh name:
/// then that name isn't the app, and RABBIT never says it is.
#[test]
fn a_folder_already_at_the_trash_name_is_never_claimed_as_the_app() {
    let batch = administrator_steps(&[reaper_move()], None).unwrap();
    for inside in [Some(true), None] {
        let results = administrator_results(
            &batch,
            ShellLinesOutcome::Completed,
            None,
            &presence_of(&[
                (REAPER_APP, Some(false)),
                (TRASHED, Some(true)),
                (TRASHED_INSIDE, inside),
            ]),
        );
        assert_eq!(
            results[0].1,
            AdminPackageResult::Removed {
                gone: paths(&[REAPER_APP]),
                trashed_to: None,
                ended_unclearly: None,
            },
            "{inside:?}"
        );
    }
}

/// Surge's last step stopped the command, yet every path of Surge is gone:
/// Surge is removed, and REAPER, after it, wasn't reached.
#[test]
fn the_package_whose_step_stopped_the_command_is_where_the_run_stopped() {
    let batch = batch_of_surge_and_reaper();
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::FailedAt { index: 3 },
        Some("rm: Surge.vst3: Directory not empty"),
        &presence_of(&[
            (SURGE_APP, Some(false)),
            (SURGE_EFFECTS, Some(false)),
            (SURGE_VST3, Some(false)),
        ]),
    );
    assert!(matches!(
        &results[0].1,
        AdminPackageResult::Removed {
            ended_unclearly: None,
            ..
        }
    ));
    assert_eq!(
        results[1].1,
        AdminPackageResult::NotReached {
            stopped_at: "surge".to_string()
        }
    );
}

#[test]
fn a_step_failing_partway_fails_its_package_and_leaves_the_rest_unreached() {
    let batch = batch_of_surge_and_reaper();
    // Surge Effects.app (step 2) was refused; Surge.app went before it.
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::FailedAt { index: 2 },
        Some("rm: /Applications/Surge Effects.app: Operation not permitted"),
        &presence_of(&[(SURGE_APP, Some(false))]),
    );
    assert_eq!(
        results,
        [
            (
                "surge".to_string(),
                AdminPackageResult::Failed {
                    path: Some(PathBuf::from(SURGE_EFFECTS)),
                    ended_unclearly: false,
                    gone: paths(&[SURGE_APP]),
                    left: paths(&[SURGE_EFFECTS, SURGE_VST3]),
                    refused_by_macos: true,
                    detail: Some(
                        "rm: /Applications/Surge Effects.app: Operation not permitted".to_string()
                    ),
                }
            ),
            (
                "reaper".to_string(),
                AdminPackageResult::NotReached {
                    stopped_at: "surge".to_string()
                }
            ),
        ]
    );

    // Refused at its very first step: nothing of Surge is gone.
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::FailedAt { index: 1 },
        None,
        &presence_of(&[]),
    );
    assert!(matches!(
        &results[0].1,
        AdminPackageResult::Failed { path: Some(path), gone, refused_by_macos: false, detail: None, .. }
            if path == Path::new(SURGE_APP) && gone.is_empty()
    ));
    assert!(matches!(
        results[1].1,
        AdminPackageResult::NotReached { .. }
    ));
}

/// What can't be read as gone never counts as gone.
#[test]
fn a_path_that_cant_be_read_counts_as_still_there() {
    let batch = administrator_steps(&[surge(&[SURGE_APP])], None).unwrap();
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::Completed,
        None,
        &presence_of(&[(SURGE_APP, None)]),
    );
    assert!(matches!(
        &results[0].1,
        AdminPackageResult::Failed { left, .. } if *left == paths(&[SURGE_APP])
    ));
}

#[test]
fn a_running_program_found_by_root_means_nothing_ran() {
    let batch = batch_of_surge_and_reaper();
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::FailedAt { index: 0 },
        None,
        &presence_of(&[]),
    );
    assert_eq!(
        results,
        [
            ("surge".to_string(), AdminPackageResult::ProgramRunning),
            ("reaper".to_string(), AdminPackageResult::ProgramRunning),
        ]
    );
}

#[test]
fn a_command_that_ended_unclearly_is_judged_by_the_filesystem() {
    let batch = batch_of_surge_and_reaper();
    // REAPER is in the Trash where RABBIT can see it, so moved, whatever
    // the status.
    let all_gone = presence_of(&[
        (SURGE_APP, Some(false)),
        (SURGE_EFFECTS, Some(false)),
        (SURGE_VST3, Some(false)),
        (REAPER_APP, Some(false)),
        (TRASHED_INSIDE, Some(false)),
    ]);
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::Stopped { code: Some(255) },
        None,
        &all_gone,
    );
    assert!(results.iter().all(|(_, result)| matches!(
        result,
        AdminPackageResult::Removed {
            ended_unclearly: Some(Some(255)),
            ..
        }
    )));

    // Nothing gone: the first package failed, without a step to name, and
    // the next wasn't reached.
    let results = administrator_results(
        &batch,
        ShellLinesOutcome::Stopped { code: None },
        Some("osascript was ended"),
        &presence_of(&[]),
    );
    assert_eq!(
        results,
        [
            (
                "surge".to_string(),
                AdminPackageResult::Failed {
                    path: None,
                    ended_unclearly: true,
                    gone: Vec::new(),
                    left: paths(&[SURGE_APP, SURGE_EFFECTS, SURGE_VST3]),
                    refused_by_macos: false,
                    detail: Some("osascript was ended".to_string()),
                }
            ),
            (
                "reaper".to_string(),
                AdminPackageResult::NotReached {
                    stopped_at: "surge".to_string()
                }
            ),
        ]
    );
}

#[test]
fn only_operation_not_permitted_counts_as_macos_refusing() {
    assert_eq!(
        classify_step_error(Some("  rm: x: Operation not permitted \n")),
        StepError {
            refused_by_macos: true,
            detail: Some("rm: x: Operation not permitted".to_string()),
        }
    );
    assert_eq!(
        classify_step_error(Some("rm: x: Permission denied")),
        StepError {
            refused_by_macos: false,
            detail: Some("rm: x: Permission denied".to_string()),
        }
    );
    assert_eq!(classify_step_error(Some("   ")), StepError::default());
    assert_eq!(classify_step_error(None), StepError::default());
}

#[test]
fn the_report_records_each_step_and_how_it_went() {
    let batch = batch_of_surge_and_reaper();
    let records = batch.records(verdicts_after(ShellLinesOutcome::FailedAt { index: 2 }));
    let summary: Vec<(
        Option<&str>,
        AdministratorStepKind,
        AdministratorStepVerdict,
    )> = records
        .iter()
        .map(|record| (record.package_id.as_deref(), record.kind, record.verdict))
        .collect();
    use AdministratorStepKind as Kind;
    use AdministratorStepVerdict as Verdict;
    assert_eq!(
        summary,
        [
            (None, Kind::ProgramsClosed, Verdict::Ran),
            (Some("surge"), Kind::Delete, Verdict::Ran),
            (Some("surge"), Kind::Delete, Verdict::Failed),
            (Some("surge"), Kind::Delete, Verdict::NotRun),
            (Some("surge"), Kind::Forget, Verdict::NotRun),
            (Some("surge"), Kind::Forget, Verdict::NotRun),
            (Some("reaper"), Kind::TrashIsFolder, Verdict::NotRun),
            (Some("reaper"), Kind::TrashIsNotLink, Verdict::NotRun),
            (Some("reaper"), Kind::MoveToTrash, Verdict::NotRun),
        ]
    );
    assert_eq!(
        records[8],
        AdministratorStepRecord {
            package_id: Some("reaper".to_string()),
            kind: Kind::MoveToTrash,
            target: Path::new(REAPER_APP).display().to_string(),
            destination: Some(Path::new(TRASHED).display().to_string()),
            verdict: Verdict::NotRun,
        }
    );
    assert_eq!(records[0].target, "Surge, Surge Effects, REAPER, reaper64");
    assert_eq!(records[4].target, APP_RECEIPT);
    // The saved report spells them in kebab-case.
    let json = serde_json::to_value(&records[6]).unwrap();
    assert_eq!(json["kind"], "trash-is-folder");
    assert_eq!(json["verdict"], "not-run");
    assert!(
        batch
            .records(verdicts_after(ShellLinesOutcome::Stopped { code: None }))
            .iter()
            .all(|record| record.verdict == Verdict::Unknown)
    );
    assert!(
        batch
            .records(verdicts_after(ShellLinesOutcome::Completed))
            .iter()
            .all(|record| record.verdict == Verdict::Ran)
    );
}

#[test]
fn a_cancelled_window_is_a_decline_and_an_unanswered_wait_is_left_to_the_filesystem() {
    let osascript = PathBuf::from("/usr/bin/osascript");
    assert_eq!(
        shell_run_from_error(ElevationError::UserCancelledElevation {
            program: osascript.clone()
        }),
        AdminShellRun::Declined
    );
    assert!(matches!(
        shell_run_from_error(ElevationError::PromptFailed { number: -60005 }),
        AdminShellRun::NotRun { reason } if reason.contains("-60005")
    ));
    assert!(matches!(
        shell_run_from_error(ElevationError::LaunchFailed {
            program: osascript.clone(),
            code: 2
        }),
        AdminShellRun::NotRun { .. }
    ));
    assert!(matches!(
        shell_run_from_error(ElevationError::RefusedCommand {
            program: osascript.clone(),
            reason: "too many commands for one administrator prompt"
        }),
        AdminShellRun::NotRun { reason } if reason.contains("too many")
    ));
    assert!(matches!(
        shell_run_from_error(ElevationError::Unsupported),
        AdminShellRun::NotRun { .. }
    ));
    assert_eq!(
        shell_run_from_error(ElevationError::WaitFailed {
            program: osascript,
            message: "no answer".to_string()
        }),
        AdminShellRun::Ran {
            outcome: ShellLinesOutcome::Stopped { code: None },
            error_text: Some("no answer".to_string()),
        }
    );
}

fn request(steps: Vec<ShellStep>) -> AdministratorShellRequest {
    AdministratorShellRequest { steps }
}

fn no_events() -> impl FnMut(AdminShellEvent) {
    |event| panic!("no event expected: {event:?}")
}

/// No test build ever raises a password window.
#[test]
fn the_real_administrator_shell_refuses_in_a_test_build() {
    let shell = SystemAdministratorShell;
    let answer = shell.run(
        &request(vec![run(RM, &["-R", "-f", "-x", "/nowhere"])]),
        &mut no_events(),
    );
    assert!(
        matches!(&answer, AdminShellRun::NotRun { reason } if reason.contains("test build")),
        "{answer:?}"
    );
    assert!(matches!(
        NoAdministratorShell.run(&request(Vec::new()), &mut no_events()),
        AdminShellRun::NotRun { .. }
    ));
}

/// Neither RABBIT's own services nor the ones without elevation can run a
/// real administrator command from a test.
#[test]
fn the_services_never_run_a_real_command_from_a_test() {
    for services in [
        super::UninstallServices::system(crate::model::Platform::MacOs),
        super::UninstallServices::without_elevation(),
    ] {
        assert!(matches!(
            services.administrator_shell.run(
                &request(vec![run(RM, &["-R", "-f", "-x", "/nowhere"])]),
                &mut no_events()
            ),
            AdminShellRun::NotRun { .. }
        ));
        assert!(!services.macos_objects.runs_as_root());
    }
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn the_recording_shell_carries_steps_out_on_a_temporary_folder() {
    let dir = tempdir().unwrap();
    let app = dir.path().join("Surge.app");
    write(&app.join("Contents").join("MacOS").join("Surge"), b"surge");
    let missing = dir.path().join("Missing.app");
    let reaper = dir.path().join("REAPER.app");
    write(&reaper.join("Contents").join("Info.plist"), b"plist");
    let trash = dir.path().join("Trash");
    fs::create_dir(&trash).unwrap();
    let moved = trash.join("REAPER moved.app");
    let objects = FakeMacosObjects::new().with_receipts([APP_RECEIPT, "com.other"]);
    let shell = RecordingAdministratorShell::new().forgetting_in(objects.receipt_list());
    let steps = vec![
        expecting_no_match(PGREP, &["-x", "-i", "--", "Surge"]),
        run(RM, &["-R", "-f", "-x", text(&app)]),
        run(RM, &["-R", "-f", "-x", text(&missing)]),
        tolerated(PKGUTIL, &["--forget", APP_RECEIPT]),
        run(TEST, &["-d", text(&trash)]),
        run(TEST, &["!", "-L", text(&trash)]),
        run(MV, &["-h", "-n", text(&reaper), text(&moved)]),
    ];

    let answer = shell.run(&request(steps.clone()), &mut no_events());

    assert_eq!(
        answer,
        AdminShellRun::Ran {
            outcome: ShellLinesOutcome::Completed,
            error_text: None,
        }
    );
    assert!(!app.exists() && !reaper.exists());
    assert!(moved.join("Contents").join("Info.plist").is_file());
    assert_eq!(objects.receipts(), Some(listed(&["com.other"])));
    assert_eq!(shell.calls(), 1);
    assert_eq!(shell.steps(), steps);
}

/// Like BSD `mv -h -n`: a file or a link to a folder at the name leaves
/// both where they are, and the step still succeeds; only the filesystem
/// shows nothing moved.
#[test]
fn the_recording_shell_never_moves_onto_a_name_that_is_taken() {
    let dir = tempdir().unwrap();
    let reaper = dir.path().join("REAPER.app");
    fs::create_dir(&reaper).unwrap();
    let taken = dir.path().join("Taken.app");
    write(&taken, b"mine");
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    let taken_link = dir.path().join("Linked.app");
    link_folder(&taken_link, &elsewhere);

    for destination in [&taken, &taken_link] {
        let answer = RecordingAdministratorShell::new().run(
            &request(vec![run(
                MV,
                &["-h", "-n", text(&reaper), text(destination)],
            )]),
            &mut no_events(),
        );
        assert!(matches!(
            answer,
            AdminShellRun::Ran {
                outcome: ShellLinesOutcome::Completed,
                ..
            }
        ));
    }
    assert!(reaper.is_dir());
    assert_eq!(fs::read(&taken).unwrap(), b"mine");
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
}

/// BSD `mv` given a real folder as the destination moves the app into it,
/// under its own name; `-n` only keeps it from replacing one already
/// there.
#[test]
fn the_recording_shell_moves_into_a_real_folder_at_the_name_as_bsd_mv_does() {
    let dir = tempdir().unwrap();
    let reaper = dir.path().join("REAPER.app");
    write(&reaper.join("Contents").join("Info.plist"), b"plist");
    let taken = dir.path().join("Taken.app");
    write(&taken.join("mine.txt"), b"mine");

    let answer = RecordingAdministratorShell::new().run(
        &request(vec![run(MV, &["-h", "-n", text(&reaper), text(&taken)])]),
        &mut no_events(),
    );

    assert!(matches!(
        answer,
        AdminShellRun::Ran {
            outcome: ShellLinesOutcome::Completed,
            ..
        }
    ));
    assert!(!reaper.exists());
    assert!(
        taken
            .join("REAPER.app")
            .join("Contents")
            .join("Info.plist")
            .is_file()
    );
    assert_eq!(fs::read(taken.join("mine.txt")).unwrap(), b"mine");
}

/// A Trash folder that is a link to a folder passes `test -d` and stops
/// at `test ! -L`, before anything moves: a junction on Windows, a
/// symbolic link elsewhere.
#[test]
fn the_recording_shell_stops_at_a_linked_trash() {
    let dir = tempdir().unwrap();
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    let trash = dir.path().join("Trash");
    link_folder(&trash, &elsewhere);
    let reaper = dir.path().join("REAPER.app");
    fs::create_dir(&reaper).unwrap();
    let answer = RecordingAdministratorShell::new().run(
        &request(vec![
            run(TEST, &["-d", text(&trash)]),
            run(TEST, &["!", "-L", text(&trash)]),
            run(
                MV,
                &["-h", "-n", text(&reaper), text(&trash.join("REAPER.app"))],
            ),
        ]),
        &mut no_events(),
    );
    assert_eq!(
        answer,
        AdminShellRun::Ran {
            outcome: ShellLinesOutcome::FailedAt { index: 1 },
            error_text: None,
        }
    );
    assert!(reaper.is_dir());
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
}

#[test]
fn the_recording_shell_answers_as_it_was_told() {
    let dir = tempdir().unwrap();
    let first = dir.path().join("First.app");
    let second = dir.path().join("Second.app");
    for app in [&first, &second] {
        fs::create_dir(app).unwrap();
    }
    let steps = || {
        vec![
            run(RM, &["-R", "-f", "-x", text(&first)]),
            run(RM, &["-R", "-f", "-x", text(&second)]),
        ]
    };

    let declining = RecordingAdministratorShell::new().declining();
    assert_eq!(
        declining.run(&request(steps()), &mut no_events()),
        AdminShellRun::Declined
    );
    let not_run = RecordingAdministratorShell::new().not_run("wrong password");
    assert_eq!(
        not_run.run(&request(steps()), &mut no_events()),
        AdminShellRun::NotRun {
            reason: "wrong password".to_string()
        }
    );
    assert!(first.exists() && second.exists());
    assert_eq!(declining.calls(), 1);

    let failing = RecordingAdministratorShell::new()
        .failing_at(1)
        .saying("rm: Second.app: Operation not permitted");
    assert_eq!(
        failing.run(&request(steps()), &mut no_events()),
        AdminShellRun::Ran {
            outcome: ShellLinesOutcome::FailedAt { index: 1 },
            error_text: Some("rm: Second.app: Operation not permitted".to_string()),
        }
    );
    assert!(!first.exists() && second.exists());

    let stopping = RecordingAdministratorShell::new().stopping(Some(255));
    assert_eq!(
        stopping.run(&request(steps()), &mut no_events()),
        AdminShellRun::Ran {
            outcome: ShellLinesOutcome::Stopped { code: Some(255) },
            error_text: None,
        }
    );
    assert!(!second.exists());
}

#[test]
fn the_fake_trash_moves_into_its_folder_and_answers_as_it_was_told() {
    let dir = tempdir().unwrap();
    let bin = dir.path().join("Trash");
    fs::create_dir(&bin).unwrap();
    let app = dir.path().join("REAPER.app");
    let make_app = || fs::create_dir_all(&app).unwrap();

    make_app();
    let objects = FakeMacosObjects::new().with_trash_bin(&bin);
    assert_eq!(objects.move_to_trash(&app).unwrap(), bin.join("REAPER.app"));
    make_app();
    // The name is taken in the Trash: another one, as macOS does.
    assert_eq!(
        objects.move_to_trash(&app).unwrap(),
        bin.join("REAPER.app 2")
    );
    assert!(!app.exists());
    assert_eq!(objects.trashed(), [app.clone(), app.clone()]);
    assert_eq!(
        objects.move_to_trash(&app).unwrap_err().kind,
        TrashErrorKind::NotFound
    );

    make_app();
    let refusing = FakeMacosObjects::new()
        .with_trash_bin(&bin)
        .failing_to_trash(TrashErrorKind::PermissionDenied, "not allowed");
    let error = refusing.move_to_trash(&app).unwrap_err();
    assert_eq!(error.kind, TrashErrorKind::PermissionDenied);
    assert_eq!(error.message, "not allowed");
    assert!(app.exists());

    let claiming = FakeMacosObjects::new()
        .with_trash_bin(&bin)
        .claiming_to_trash_without_moving();
    assert_eq!(
        claiming.move_to_trash(&app).unwrap(),
        bin.join("REAPER.app 3")
    );
    assert!(app.exists());

    let unclear = FakeMacosObjects::new()
        .with_trash_bin(&bin)
        .trashing_without_saying_where();
    assert_eq!(
        unclear.move_to_trash(&app).unwrap_err().kind,
        TrashErrorKind::Unclear
    );
    assert!(!app.exists());

    make_app();
    assert_eq!(
        FakeMacosObjects::new()
            .move_to_trash(&app)
            .unwrap_err()
            .kind,
        TrashErrorKind::Unsupported
    );
}

#[test]
fn the_fake_lists_receipts_counts_refreshes_and_can_be_root() {
    let objects = FakeMacosObjects::new();
    assert_eq!(objects.installed_package_ids().unwrap(), BTreeSet::new());
    assert!(!objects.runs_as_root());
    objects.refresh_audio_units().unwrap();
    assert_eq!(objects.audio_unit_refreshes(), 1);

    let objects = FakeMacosObjects::new().with_receipts([APP_RECEIPT]);
    assert_eq!(
        objects.installed_package_ids().unwrap(),
        listed(&[APP_RECEIPT])
    );
    assert!(
        FakeMacosObjects::new()
            .with_unlistable_receipts()
            .installed_package_ids()
            .is_err()
    );
    assert!(FakeMacosObjects::new().running_as_root().runs_as_root());
}

/// The real ones change nothing in a test build, and no stand-in for "no
/// macOS" ever claims root.
#[test]
fn the_real_macos_objects_change_nothing_in_a_test_build() {
    let dir = tempdir().unwrap();
    let app = dir.path().join("REAPER.app");
    fs::create_dir(&app).unwrap();
    let error = SystemMacosObjects.move_to_trash(&app).unwrap_err();
    assert_eq!(error.kind, TrashErrorKind::Refused);
    assert!(app.is_dir());
    assert!(SystemMacosObjects.refresh_audio_units().is_err());

    assert_eq!(
        NoMacosObjects.move_to_trash(&app).unwrap_err().kind,
        TrashErrorKind::Unsupported
    );
    assert_eq!(
        NoMacosObjects.installed_package_ids().unwrap_err().kind(),
        io::ErrorKind::Unsupported
    );
    assert!(NoMacosObjects.refresh_audio_units().is_err());
    assert!(!NoMacosObjects.runs_as_root());
    // Off macOS nobody is root by this rule; on macOS CI the tests don't
    // run as root.
    assert!(!SystemMacosObjects.runs_as_root());
}
