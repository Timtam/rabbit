use rabbit_core::localization::{DEFAULT_LOCALE, Localizer};
use rabbit_core::model::{Architecture, Platform};
use rabbit_core::package::{
    HostCapabilities, PACKAGE_OSARA, PACKAGE_REAPACK, builtin_package_specs,
};
use rabbit_core::plan::{InstallPlan, PlanAction, PlanActionKind};
use rabbit_core::version::Version;

use super::support::*;
use crate::wizard::text::*;
use crate::wizard::*;

#[test]
fn only_rows_that_offered_something_record_a_verdict() {
    // Three language-pack rows in three states, and only two of them
    // mean anything:
    //   - an Update left unticked is a refusal (the user was offered a
    //     newer pack and said no),
    //   - a Keep is silence (there was nothing to accept or refuse),
    //   - anything ticked is a yes.
    let localizer = Localizer::embedded("de-DE").unwrap();
    let installation = fake_installation();
    let action = |id: &str, kind| PlanAction {
        package_id: id.to_string(),
        action: kind,
        installed_version: None,
        available_version: None,
        reason: "test".to_string(),
    };
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![installation.clone()],
        Some(0),
        InstallPlan {
            target: Some(installation),
            actions: vec![
                action("langpack-de", PlanActionKind::Update),
                action("langpack-es", PlanActionKind::Keep),
                action("langpack-fr", PlanActionKind::Install),
                action(PACKAGE_OSARA, PlanActionKind::Install),
            ],
            notes: Vec::new(),
        },
    );
    let index_of = |id: &str| {
        model
            .package_rows
            .iter()
            .position(|row| row.package_id == id)
            .unwrap_or_else(|| panic!("row for {id} should exist"))
    };

    // Tick only the French pack (and OSARA, so the request is valid).
    let request = install_request_from_target(
        &model,
        &model.target_rows[0],
        &[index_of("langpack-fr"), index_of(PACKAGE_OSARA)],
        WizardInstallOptions::default(),
    )
    .unwrap();

    assert_eq!(
        request.accepted_packages,
        vec!["langpack-fr".to_string()],
        "a ticked pack is a yes"
    );
    assert_eq!(
        request.declined_packages,
        vec!["langpack-de".to_string()],
        "an unticked Update is a no; the Keep row must say nothing"
    );
    assert!(
        !request
            .declined_packages
            .iter()
            .any(|id| id == "langpack-es"),
        "an installed, up-to-date pack sits unticked because there is \
         nothing to do — that is not a refusal"
    );
}

#[test]
fn a_declined_language_pack_stops_being_ticked_by_default() {
    // RABBIT running in German suggests the German pack — that is the
    // whole point of the UI-language match. But someone who does not
    // want it had to untick it on every single launch, because nothing
    // remembered the refusal.
    let localizer = Localizer::embedded("de-DE").unwrap();
    let text = wizard_text(&localizer);
    let specs = builtin_package_specs(Platform::Windows);
    let host = HostCapabilities::default();
    let actions = vec![PlanAction {
        package_id: "langpack-de".to_string(),
        action: PlanActionKind::Install,
        installed_version: None,
        available_version: None,
        reason: "Missing".to_string(),
    }];

    let suggested = package_rows(
        &localizer,
        &text,
        Platform::Windows,
        Architecture::X64,
        &specs,
        &actions,
        &[],
        &host,
        &Default::default(),
    );
    assert!(
        suggested[0].selected,
        "a German RABBIT should suggest the German pack"
    );

    let declined: std::collections::BTreeSet<String> =
        ["langpack-de".to_string()].into_iter().collect();
    let remembered = package_rows(
        &localizer,
        &text,
        Platform::Windows,
        Architecture::X64,
        &specs,
        &actions,
        &[],
        &host,
        &declined,
    );
    assert!(
        !remembered[0].selected,
        "a pack the user turned down must not come back ticked"
    );
    assert!(
        remembered[0].available_for_target,
        "it must stay listed and tickable — this suppresses the tick, not the package"
    );
}

#[test]
fn the_language_step_names_a_language_pack_not_a_particular_one() {
    // Reported from Q&A: the row read "Set REAPER's language (requires
    // REAPER en espanol)". The step lists every language pack as an
    // any-of dependency and langpack-es sorts first, so the message
    // picked it out and made a general requirement look Spanish-specific.
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let step = rabbit_core::configuration::builtin_configuration_steps()
        .into_iter()
        .find(|step| step.id == rabbit_core::configuration::CONFIG_SET_REAPER_LANGUAGE)
        .expect("the language step exists");
    assert!(
        step.requires_packages.len() > 1,
        "this test only means anything while the step is an any-of"
    );

    let summary =
        configuration_row_summary(&localizer, &step, "Set REAPER's language", false, false);
    let reason = build_configuration_unavailability_reason(&localizer, &step, false, false)
        .expect("an unsatisfied dependency has a reason");

    for text in [&summary, &reason] {
        assert!(
            text.contains("language pack"),
            "the message should name the group: {text}"
        );
        for package_id in &step.requires_packages {
            let name = localizer.text(&format!("package-{package_id}"));
            if name.missing || name.value.is_empty() {
                continue;
            }
            assert!(
                !text.contains(&name.value),
                "must not single out {:?}: {text}",
                name.value
            );
        }
    }
}

#[test]
fn a_single_dependency_step_still_names_its_package() {
    // The fix above must not generalise away a genuinely specific
    // requirement: the ReaPack steps depend on exactly one package and
    // should keep saying so.
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let step = rabbit_core::configuration::builtin_configuration_steps()
        .into_iter()
        .find(|step| step.id == "reapack-add-reaper-accessibility-remote")
        .expect("the ReaPack remote step exists");

    let reason = build_configuration_unavailability_reason(&localizer, &step, false, false)
        .expect("an unsatisfied dependency has a reason");
    let reapack = localizer.text("package-reapack");
    assert!(
        reason.contains(&reapack.value),
        "a one-package step should name it: {reason}"
    );
}

#[test]
fn language_step_is_unavailable_while_no_language_pack_is_ticked() {
    // The step activates one of the packs you ticked — the "REAPER
    // language after installation" dropdown lists exactly those — so
    // with none ticked it has nothing to point at. It must be greyed
    // out rather than tickable-but-meaningless, even when a pack is
    // already sitting on disk from an earlier run.
    let (_localizer, model) = langpack_model(PlanActionKind::Keep);
    let pack = &model.package_rows[0];
    assert!(!pack.selected, "an already-installed pack starts unticked");

    let step = language_step(&model.configuration_rows);
    assert!(
        !step.available_for_target,
        "no language pack is ticked, so the step must not be tickable"
    );
    assert!(!step.selected, "and it must not be ticked");
}

#[test]
fn language_step_follows_the_pack_being_ticked_and_unticked() {
    let (localizer, model) = langpack_model(PlanActionKind::Keep);
    let mut package_rows = model.package_rows.clone();
    let mut configuration_rows = model.configuration_rows.clone();

    // Tick the pack: the user explicitly wants it re-staged, so the
    // step's default flips on.
    apply_checkbox_state_to_package_row(&model, &mut package_rows[0], true).unwrap();
    recompute_configuration_row_availability(
        &localizer,
        &package_rows,
        None,
        &mut configuration_rows,
    );
    let step = language_step(&configuration_rows);
    assert!(
        step.available_for_target,
        "ticking a language pack must make the step usable"
    );
    assert!(
        step.selected,
        "ticking a language pack must tick the step that activates it"
    );

    // Untick it again: nothing language-related is happening any more.
    apply_checkbox_state_to_package_row(&model, &mut package_rows[0], false).unwrap();
    recompute_configuration_row_availability(
        &localizer,
        &package_rows,
        None,
        &mut configuration_rows,
    );
    let step = language_step(&configuration_rows);
    assert!(
        !step.selected,
        "unticking the last pack must untick the step again"
    );
    assert!(
        !step.available_for_target,
        "and must grey it out, since there is no longer a pack to activate"
    );
}

#[test]
fn language_step_keeps_an_explicit_user_override() {
    let (localizer, model) = langpack_model(PlanActionKind::Keep);
    let mut package_rows = model.package_rows.clone();
    let mut configuration_rows = model.configuration_rows.clone();

    apply_checkbox_state_to_package_row(&model, &mut package_rows[0], true).unwrap();
    recompute_configuration_row_availability(
        &localizer,
        &package_rows,
        None,
        &mut configuration_rows,
    );
    assert!(language_step(&configuration_rows).selected);

    // The user unticks the step by hand: install the pack, but leave
    // REAPER's language alone. A later recompute must not re-tick it.
    let index = configuration_rows
        .iter()
        .position(|row| row.step_id == rabbit_core::configuration::CONFIG_SET_REAPER_LANGUAGE)
        .unwrap();
    configuration_rows[index].selected = false;
    recompute_configuration_row_availability(
        &localizer,
        &package_rows,
        None,
        &mut configuration_rows,
    );
    assert!(
        !language_step(&configuration_rows).selected,
        "an explicit untick must survive a recompute"
    );
}

#[test]
fn additive_steps_still_auto_tick_for_an_already_installed_dependency() {
    // The guard is deliberately narrow: only steps that CHANGE existing
    // configuration wait for a fresh install. Adding the ReaPack remote
    // is additive, so an already-installed ReaPack still auto-ticks it.
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let installation = fake_installation();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![installation.clone()],
        Some(0),
        InstallPlan {
            target: Some(installation),
            actions: vec![PlanAction {
                package_id: PACKAGE_REAPACK.to_string(),
                action: PlanActionKind::Keep,
                installed_version: Some(Version::parse("1.2.6").unwrap()),
                available_version: Some(Version::parse("1.2.6").unwrap()),
                reason: "installed".to_string(),
            }],
            notes: Vec::new(),
        },
    );
    let remote = model
        .configuration_rows
        .iter()
        .find(|row| row.step_id == "reapack-add-reaper-accessibility-remote")
        .expect("reapack remote row should exist");
    assert!(remote.available_for_target);
    assert!(
        remote.selected,
        "additive steps must keep auto-ticking for a dependency that is already installed"
    );
}

#[test]
fn configuration_row_unticks_when_dep_package_is_unticked_install() {
    // ReaPack is `recommended: false`. With a fresh target it lands as an
    // Install action that arrives unticked-by-default, so the
    // "add ReaPack remote" configuration step's dependency is NOT
    // satisfied — the configuration row must start unticked too,
    // otherwise the wizard would queue a config step that points at a
    // package the user hasn't asked for.
    let localizer = Localizer::embedded(DEFAULT_LOCALE).unwrap();
    let installation = fake_installation();
    let model = model_from_plan(
        &localizer,
        Platform::Windows,
        Architecture::X64,
        vec![installation.clone()],
        Some(0),
        InstallPlan {
            target: Some(installation),
            actions: vec![PlanAction {
                package_id: PACKAGE_REAPACK.to_string(),
                action: PlanActionKind::Install,
                installed_version: None,
                available_version: Some(Version::parse("1.2.6").unwrap()),
                reason: "Missing".to_string(),
            }],
            notes: Vec::new(),
        },
    );

    let reapack = &model.package_rows[0];
    // Plan's action lives on `original_action`; the unticked row's
    // current `action` is Keep so the visible row label says "Won't touch".
    assert_eq!(reapack.original_action, PlanActionKind::Install);
    assert_eq!(reapack.action, PlanActionKind::Keep);
    assert!(!reapack.selected, "ReaPack row should start unticked");

    let reapack_remote = model
        .configuration_rows
        .iter()
        .find(|row| row.step_id == "reapack-add-reaper-accessibility-remote")
        .expect("reapack-add-reaper-accessibility-remote row should exist");
    assert!(
        !reapack_remote.available_for_target,
        "config row's dependency package isn't going to land on disk; \
         row must be marked unavailable"
    );
    assert!(
        !reapack_remote.selected,
        "config row must not auto-tick when its dep package is an unticked Install"
    );
}
