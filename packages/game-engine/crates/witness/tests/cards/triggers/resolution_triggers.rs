//! Frozen R4 delayed and reflexive trigger witnesses.
//! CR 400.7j, 513.2, 603.3b, 603.7, 603.12.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const LIBERATE: &str = record!("f17e6c85-4813-4b0f-a0e9-d295822e9bb5");
const CARRIER_PIGEONS: &str = record!("2d30a634-43c6-4e78-ba10-7ef7ee17ac40");
const TRANSLUMINANT: &str = record!("f4d54a89-8409-4fbc-b01f-3b03f352820f");
const CASEY_JONES: &str = record!("bc163b5a-0b1a-4257-8c07-5c7d5284015f");
const SPARKTONGUE_DRAGON: &str = record!("1817acd5-5ed0-47b8-8e56-4715a488867e");
const TRUE_CONVICTION: &str = record!("fc299c1c-50f3-492a-b6b8-a3664bb72ab7");
const LUCIUS: &str = record!("fa726aac-f4ae-4e34-ac08-36c1f50215f3");

fn deliveries(g: &Game) -> usize {
    g.events()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::TriggerCollected {
                    ability: EffectiveTriggerAbility::Followup { .. },
                    ..
                }
            )
        })
        .count()
}

fn cast_to_priority(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    Driver::cast(g, CardId(id));
    mtg_witness::equipped::pay_to_priority(g, u64::MAX, targets);
}

fn forge(g: &Game, change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut forged = g.fork();
    let mut state = serde_json::to_value(&forged.state).unwrap();
    change(&mut state);
    forged.state = serde_json::from_value(state).unwrap();
    forged.validate_state().unwrap_err()
}

fn target(object: ObjectId) -> TargetEntity {
    TargetEntity::Object { object }
}

#[test]
fn resolution_created_blink_uses_the_next_end_step_and_exile_incarnation() {
    let prepared = prepare(
        board(&[LIBERATE], TurnStep::End)
            .seats(4)
            .permanent_or_hand("Liberate", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 1, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bear = Driver::object(&g, 1);
    mtg_witness::equipped::cast(&mut g, u64::MAX, 0, vec![target(bear)]);
    let exiled = Driver::object(&g, 1);
    assert_eq!(g.current(exiled).unwrap().zone, Zone::Exile);
    assert_eq!(
        (g.state.rules.turn.number, g.state.rules.turn.step),
        (1, TurnStep::End)
    );
    assert_eq!(deliveries(&g), 0);
    checked(&g);

    assert!(
        forge(&g, |s| {
            let row = s["resolution_triggers"]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap();
            row["capture"]["actual"]["successor"]["incarnation"] = (exiled.incarnation + 1).into();
        })
        .contains("captured incarnation")
    );

    assert!(
        forge(&g, |s| {
            let row = s["resolution_triggers"]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap();
            row["capture"]["actual"]["successor"]["face"] = "back".into();
        })
        .contains("captured incarnation")
    );

    // Leave exile, reenter and get exiled again before the old timer fires.
    // A separate fixture-only fork probes the exact identity without changing
    // the accepted-input replay witness.
    let mut reincarnated = g.fork();
    let returned = reincarnated
        .move_object_for(exiled, Zone::Battlefield, BatchCause::Fixture)
        .unwrap()
        .successor;
    let different_exile = reincarnated
        .move_object_for(returned, Zone::Exile, BatchCause::Fixture)
        .unwrap()
        .successor;
    drive(&mut reincarnated, &[], triggered);
    drive(&mut reincarnated, &[], |g| {
        g.state.rules.stack.is_empty() && matches!(g.state.rules.frame, Frame::Priority)
    });
    assert_eq!(
        reincarnated.current(different_exile).unwrap().zone,
        Zone::Exile
    );

    drive(&mut g, &[], triggered);
    assert_eq!(
        (g.state.rules.turn.number, g.state.rules.turn.step),
        (2, TurnStep::End)
    );
    assert_eq!(g.state.rules.stack[0].controller, PlayerId(0));
    let first = serde_json::to_value(&g.state.rules.stack[0]).unwrap();
    assert!(
        forge(&g, |s| {
            s["stack"][0]["controller"] = serde_json::json!(1);
        })
        .contains("firing provenance")
    );
    assert!(
        forge(&g, |s| {
            let id = s["next_stack_id"].as_u64().unwrap();
            let mut duplicate = s["stack"][0].clone();
            duplicate["id"] = id.into();
            s["stack"].as_array_mut().unwrap().push(duplicate);
            s["next_stack_id"] = (id + 1).into();
        })
        .contains("duplicate trigger")
    );
    checked(&g);
    settle(&mut g);
    let returned = g.current(Driver::object(&g, 1)).unwrap();
    assert_eq!(returned.zone, Zone::Battlefield);
    assert_eq!(returned.controller, PlayerId(1));
    assert!(
        forge(&g, |s| s["stack"].as_array_mut().unwrap().push(first))
            .contains("resolution trigger origin")
    );
    assert!(
        forge(&g, |s| {
            let row = s["resolution_triggers"]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap();
            row["firing"] = serde_json::Value::Null;
            row["retired"] = false.into();
        })
        .contains("already passed")
    );
    drive(&mut g, &[], |g| {
        g.state.rules.turn.number == 3 && g.state.rules.turn.step == TurnStep::End
    });
    assert_eq!(deliveries(&g), 1);
    assert_recoverable((&g).into());
}

#[test]
fn resolution_created_next_turn_upkeep_draws_for_the_creator() {
    let mut setup = board(&[CARRIER_PIGEONS], TurnStep::PrecombatMain).permanent_or_hand(
        "Carrier Pigeons",
        0,
        0,
        true,
    );
    for _ in 0..4 {
        setup = setup.permanent_or_hand("Plains", 0, 0, false);
    }
    let mut g = Game::with_trace(prepare(setup), TraceMode::Detailed);
    mtg_witness::equipped::cast(&mut g, u64::MAX, 0, vec![]);
    assert_eq!(deliveries(&g), 0);
    drive(&mut g, &[], triggered);
    assert_eq!(g.state.rules.turn.step, TurnStep::Upkeep);
    assert_eq!(g.state.rules.active, Some(PlayerId(1)));
    assert_eq!(g.state.rules.stack[0].controller, PlayerId(0));
    checked(&g);
    settle(&mut g);
    assert_eq!((hand(&g, 0), hand(&g, 1)), (1, 0));
    assert_recoverable((&g).into());
}

#[test]
fn resolution_created_your_next_upkeep_skips_the_other_players_upkeep() {
    let mut setup = board(&[CASEY_JONES], TurnStep::PrecombatMain)
        .seats(4)
        .seed([41; 32])
        .permanent_or_hand("Casey Jones, Vigilante", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, true);
    for _ in 0..8 {
        setup = setup.permanent_or_hand("Mountain", 0, 0, false);
    }
    let mut g = Game::with_trace(prepare(setup), TraceMode::Detailed);
    mtg_witness::equipped::cast(&mut g, u64::MAX, 0, vec![]);
    assert_eq!(hand(&g, 0), 4);
    drive(&mut g, &[], |g| {
        g.state.rules.turn.number == 2 && g.state.rules.turn.step == TurnStep::Upkeep
    });
    assert_eq!(deliveries(&g), 0);
    drive(&mut g, &[], triggered);
    assert_eq!(
        (g.state.rules.turn.number, g.state.rules.turn.step),
        (5, TurnStep::Upkeep)
    );
    checked(&g);
    settle(&mut g);
    assert_eq!(hand(&g, 0), 1);
    assert_recoverable((&g).into());
}

#[test]
fn resolution_created_end_step_token_survives_the_activated_sources_sacrifice() {
    let prepared = prepare(
        board(&[TRANSLUMINANT], TurnStep::PostcombatMain)
            .permanent_or_hand("Transluminant", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let source = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    mtg_witness::equipped::pay_to_priority(&mut g, u64::MAX, vec![]);
    settle(&mut g);
    assert_eq!(
        g.current(Driver::object(&g, 0)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    drive(&mut g, &[], triggered);
    assert_eq!(g.state.rules.stack[0].source, source);
    assert_eq!(g.state.rules.turn.step, TurnStep::End);
    settle(&mut g);
    assert_eq!(g.state.rules.tokens.len(), 1);
    assert_recoverable((&g).into());
}

#[test]
fn resolution_created_reflexive_payment_announces_targets_after_actual_spend() {
    let mut setup = board(&[SPARKTONGUE_DRAGON], TurnStep::PrecombatMain).permanent_or_hand(
        "Sparktongue Dragon",
        0,
        0,
        true,
    );
    for _ in 0..8 {
        setup = setup.permanent_or_hand("Mountain", 0, 0, false);
    }
    let mut g = Game::with_trace(prepare(setup), TraceMode::Detailed);
    cast_to_priority(&mut g, 0, vec![]);
    drive(&mut g, &[], triggered);
    while matches!(g.next_test_decision().reason, Reason::Priority) {
        Driver::pass(&mut g);
    }
    let offer = Driver::request(&mut g, u64::MAX);
    assert_eq!(offer.reason, Reason::OptionalEffect);
    assert_eq!(deliveries(&g), 0);
    assert!(g.state.rules.stack[0].entity_targets.is_empty());

    let mut decline = g.fork();
    Driver::answer(
        &mut decline,
        u64::MAX,
        offer
            .pick_one(&Candidate::Flag {
                flag: Flag::Decline,
            })
            .unwrap(),
    );
    settle(&mut decline);
    assert_eq!(deliveries(&decline), 0);
    assert_recoverable((&decline).into());

    let accept = offer
        .pick_one(&Candidate::Flag { flag: Flag::Accept })
        .unwrap();
    let mut unpaid = g.fork();
    Driver::answer(&mut unpaid, u64::MAX, accept.clone());
    Driver::answer(
        &mut unpaid,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    settle(&mut unpaid);
    assert_eq!(deliveries(&unpaid), 0);

    Driver::answer(&mut g, u64::MAX, accept);
    checked(&g);
    Driver::pay(&mut g, &[6, 7, 8]);
    let targets = Driver::request(&mut g, u64::MAX);
    assert!(matches!(targets.reason, Reason::Targets { .. }));
    assert_eq!(deliveries(&g), 1);
    let followup = &g.state.rules.stack[0];
    assert!(matches!(
        followup.kind,
        StackKind::Triggered {
            ability: EffectiveTriggerAbility::Followup { .. },
            witness: TriggerEventWitness::ResolutionPaid { .. },
            ..
        }
    ));
    assert_eq!(g.state.rules.stack.len(), 1);
    assert!(
        forge(&g, |s| {
            s["stack"][0]["kind"]["witness"]["player"] = 1.into();
        })
        .contains("firing provenance")
    );
    checked(&g);
    let opponent = life(&g, 1);
    Driver::answer(
        &mut g,
        u64::MAX,
        targets.entity_targets(vec![TargetEntity::Player {
            player: PlayerId(1),
        }]),
    );
    settle(&mut g);
    assert_eq!(life(&g, 1), plus(&opponent, -3));
    assert_recoverable((&g).into());
}

fn watch_with_lucius(watched_controller: u8) -> (Game, ObjectId, ObjectId) {
    let prepared = prepare(
        board(&[LUCIUS], TurnStep::PrecombatMain)
            .seats(4)
            .permanent_or_hand("Lucius the Eternal", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, watched_controller, false)
            .permanent_or_hand("Murder", 0, 0, true)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let source = Driver::object(&g, 0);
    let watched = Driver::object(&g, 1);
    cast_to_priority(&mut g, 2, vec![target(source)]);
    while matches!(g.next_test_decision().reason, Reason::Priority) {
        Driver::pass(&mut g);
    }
    let targets = Driver::request(&mut g, u64::MAX);
    assert!(matches!(targets.reason, Reason::Targets { .. }));
    Driver::answer(
        &mut g,
        u64::MAX,
        targets.entity_targets(vec![target(watched)]),
    );
    settle(&mut g);
    (g, source, watched)
}

#[test]
fn resolution_created_departure_watches_the_chosen_permanent() {
    let (mut g, source, watched) = watch_with_lucius(1);
    let exile = Driver::object(&g, 0);
    assert_eq!(g.current(exile).unwrap().zone, Zone::Exile);
    assert_eq!(deliveries(&g), 0);
    checked(&g);
    cast_to_priority(&mut g, 3, vec![target(watched)]);
    drive(&mut g, &[], triggered);
    assert_eq!(g.state.rules.stack[0].source, source);
    assert_eq!(deliveries(&g), 1);
    checked(&g);
    settle(&mut g);
    assert_eq!(
        g.current(Driver::object(&g, 0)).unwrap().zone,
        Zone::Battlefield
    );
    assert_eq!(Driver::object(&g, 0).incarnation, exile.incarnation + 1);
    assert_recoverable((&g).into());
}

/// CR 603.6c and 800.4a: a non-owned creature still controlled by a
/// departing player moves from the battlefield to exile.
#[test]
fn resolution_created_departure_collects_concession_exile() {
    let (mut g, source, watched) = watch_with_lucius(2);
    let exiled_source = Driver::object(&g, 0);
    assert_eq!(g.current(exiled_source).unwrap().zone, Zone::Exile);
    assert_eq!(g.state.rules.cards[&watched.card].owner, PlayerId(1));
    assert_eq!(g.current(watched).unwrap().controller, PlayerId(2));
    assert_eq!(deliveries(&g), 0);
    checked(&g);

    g.concede(PlayerId(2)).unwrap();
    drive(&mut g, &[], triggered);
    assert_eq!(g.live_players(), [PlayerId(0), PlayerId(1), PlayerId(3)]);
    assert_eq!(g.current(Driver::object(&g, 1)).unwrap().zone, Zone::Exile);
    assert_eq!(g.state.rules.stack[0].source, source);
    assert!(
        matches!(&g.state.rules.stack[0].kind, StackKind::Triggered {
        witness: TriggerEventWitness::Moved { movement }, ..
    } if movement.previous == watched && movement.from == Zone::Battlefield
        && movement.destination == Zone::Exile)
    );
    assert_eq!(deliveries(&g), 1);
    checked(&g);

    settle(&mut g);
    let returned = Driver::object(&g, 0);
    assert_eq!(g.current(returned).unwrap().zone, Zone::Battlefield);
    assert_eq!(returned.incarnation, exiled_source.incarnation + 1);
    assert_recoverable((&g).into());
}

/// CR 608.2h/702.26b: a phased-out source uses the parent's last-known
/// battlefield characteristics for damage, including granted lifelink.
#[test]
fn resolution_created_reflexive_trigger_uses_phased_source_lki() {
    let mut setup = board_with(
        compiled(&[SPARKTONGUE_DRAGON, TRUE_CONVICTION], false),
        TurnStep::PrecombatMain,
    )
    .permanent_or_hand("Sparktongue Dragon", 0, 0, true);
    for _ in 0..8 {
        setup = setup.permanent_or_hand("Mountain", 0, 0, false);
    }
    let mut g = Game::with_trace(
        prepare(setup.permanent_or_hand("True Conviction", 0, 0, false)),
        TraceMode::Detailed,
    );
    cast_to_priority(&mut g, 0, vec![]);
    drive(&mut g, &[], triggered);
    checked(&g);
    let dragon = Driver::object(&g, 0);
    // Private fixture action: the phasing primitive is available without an
    // admitted phasing spell. The paid trigger proceeds through normal inputs.
    g.phase_out_permanents(phasing::PhaseSelection::Target(dragon))
        .unwrap();
    assert!(g.current(dragon).unwrap().phased_out.is_some());
    let last_known = g.state.rules.stack[0]
        .source_facts
        .as_ref()
        .unwrap()
        .clone();
    for keyword in [
        KeywordAbility::Flying,
        KeywordAbility::DoubleStrike,
        KeywordAbility::Lifelink,
    ] {
        assert!(last_known.keywords.contains(&keyword));
    }

    while matches!(g.next_test_decision().reason, Reason::Priority) {
        Driver::pass(&mut g);
    }
    let offer = Driver::request(&mut g, u64::MAX);
    assert_eq!(offer.reason, Reason::OptionalEffect);
    Driver::answer(
        &mut g,
        u64::MAX,
        offer
            .pick_one(&Candidate::Flag { flag: Flag::Accept })
            .unwrap(),
    );
    Driver::pay(&mut g, &[6, 7, 8]);
    let targets = Driver::request(&mut g, u64::MAX);
    assert!(matches!(targets.reason, Reason::Targets { .. }));
    assert_eq!(deliveries(&g), 1);
    assert_eq!(
        g.state.rules.stack[0].source_facts.as_ref(),
        Some(&last_known)
    );
    let (controller, opponent) = (life(&g, 0), life(&g, 1));
    Driver::answer(
        &mut g,
        u64::MAX,
        targets.entity_targets(vec![TargetEntity::Player {
            player: PlayerId(1),
        }]),
    );
    drive(&mut g, &[], |g| g.state.rules.stack.is_empty());
    g.validate_state().unwrap();
    assert_eq!(life(&g, 1), plus(&opponent, -3));
    assert_eq!(life(&g, 0), plus(&controller, 3));
}
