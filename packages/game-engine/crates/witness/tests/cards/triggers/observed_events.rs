//! Frozen R5 witnesses for CR 121.2, 509.3g, 601.2c, 603.4 and 608.2h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const WARSTORM_SURGE: &str = record!("42fb1a1c-ab3d-4cdc-a6ff-a591f7481583");
const GLEAM_OF_BATTLE: &str = record!("19f23d9e-0c05-4028-9c64-60b9a20020f9");
const LOYAL_GUARDIAN: &str = record!("de1cfa83-2049-48ab-b093-2f120a3ef4f2");
const SHEOLDRED: &str = record!("34f34409-326d-4994-a0ea-1a69aa278f03");
const THORN_LIEUTENANT: &str = record!("2b761ce6-4fb1-4f22-b51c-d9f25f62b39a");
const STORMCHASER_DRAKE: &str = record!("a9530416-d5fb-4ddd-ae6f-0eefb9f276c5");
const ABYSSAL_NIGHTSTALKER: &str = record!("10733767-1c97-4e2e-b02b-54bf346f6583");
const WORTHY_KNIGHT: &str = record!("37733e29-3ade-4bfd-89c9-efd13b548a53");
const STAFF: &str = record!("fc71710a-8294-4370-89da-7ab1c2b38d19");
const THUNDERBREAK_REGENT: &str = record!("bb04f927-0348-4a14-9e74-381f18083f6a");

fn aim_at_player(g: &mut Game, player: PlayerId) {
    let d = Driver::request(g, u64::MAX);
    let answer = d.pick_one(&Candidate::Player { player }).unwrap();
    g.respond(d.player, d.id, answer).unwrap();
}

fn clone_latest_trigger(forged: &mut Snapshot) {
    let mut duplicate = forged.state.rules.stack.last().unwrap().clone();
    let StackKind::Triggered { trigger_id, .. } = &mut duplicate.kind else {
        panic!("expected a triggered ability");
    };
    trigger_id.0 = forged.state.rules.next_trigger_id;
    duplicate.id.0 = forged.state.rules.next_stack_id;
    forged.state.rules.next_trigger_id += 1;
    forged.state.rules.next_stack_id += 1;
    forged.state.rules.stack.push(duplicate);
}

#[test]
fn observed_events_warstorm_uses_the_entrants_departure_power() {
    let mut g = Game::with_trace(
        prepare(
            board(&[WARSTORM_SURGE], TurnStep::PrecombatMain)
                .permanent_or_hand("Warstorm Surge", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Regress", 1, 1, true)
                .permanent_or_hand("Island", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[3, 4]);
    drive(&mut g, &[], |g| {
        matches!(g.state.rules.frame, Frame::TriggerTargets { .. })
    });
    aim_at_player(&mut g, PlayerId(1));
    let _ = Driver::request(&mut g, u64::MAX);
    let entrant = Driver::object(&g, 1);
    assert!(triggered(&g));

    let mut duplicated: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    clone_latest_trigger(&mut duplicated);
    assert_forgery_rejected(&g, duplicated, "duplicate trigger occurrence");

    let mut wrong_controller: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    wrong_controller
        .state
        .rules
        .stack
        .last_mut()
        .unwrap()
        .controller = PlayerId(1);
    assert_forgery_rejected(&g, wrong_controller, "event object binding invariant");

    let mut wrong_subject: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let StackKind::Triggered { witness, .. } =
        &mut wrong_subject.state.rules.stack.last_mut().unwrap().kind
    else {
        unreachable!();
    };
    let mut wire = serde_json::to_value(&*witness).unwrap();
    wire["binding"]["last_known"]["object"] = serde_json::to_value(Driver::object(&g, 0)).unwrap();
    *witness = serde_json::from_value(wire).unwrap();
    assert_forgery_rejected(&g, wrong_subject, "event object binding invariant");

    Driver::cast(&mut g, CardId(2));
    Driver::pick_object(&mut g, entrant);
    Driver::pay(&mut g, &[5]);
    drive(&mut g, &[], |g| {
        g.state.rules.stack.len() == 1 && triggered(g)
    });
    assert_eq!(g.current(entrant).unwrap().zone, Zone::Battlefield);
    Driver::pass(&mut g);
    Driver::cast(&mut g, CardId(6));
    Driver::pick_object(&mut g, entrant);
    Driver::pay(&mut g, &[7, 8, 9]);
    drive(&mut g, &[], |g| {
        g.state.rules.stack.len() == 1 && triggered(g)
    });
    assert_ne!(Driver::object(&g, 1), entrant);
    checked(&g);
    let before = life(&g, 1);
    settle(&mut g);
    assert_eq!(life(&g, 1), plus(&before, -5));
    assert_recoverable((&g).into());
}

#[test]
fn observed_events_gleam_counts_the_declared_attackers_separately() {
    let mut g = Game::with_trace(
        prepare(
            board(&[GLEAM_OF_BATTLE], TurnStep::BeginningCombat)
                .permanent_or_hand("Gleam of Battle", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Storm Crow", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let attackers = [Driver::object(&g, 1), Driver::object(&g, 2)];
    drive(&mut g, &attackers, |g| g.state.rules.stack.len() == 2);
    checked(&g);
    settle(&mut g);
    for attacker in attackers {
        assert_eq!(g.current(attacker).unwrap().plus_one_counters, 1.into());
    }
    assert_eq!(
        g.current(Driver::object(&g, 3)).unwrap().plus_one_counters,
        0.into()
    );
    assert_recoverable((&g).into());
}

#[test]
fn observed_events_guardian_rechecks_commander_at_resolution() {
    for bounce in [false, true] {
        let mut g = Game::with_trace(
            prepare(
                board(&[LOYAL_GUARDIAN], TurnStep::PrecombatMain)
                    .permanent_or_hand("Loyal Guardian", 0, 0, false)
                    .permanent_or_hand("Isamaru, Hound of Konda", 0, 0, false)
                    .permanent_or_hand("Grizzly Bears", 0, 0, false)
                    .permanent_or_hand("Regress", 1, 1, true)
                    .permanent_or_hand("Island", 1, 1, false)
                    .permanent_or_hand("Island", 1, 1, false)
                    .permanent_or_hand("Island", 1, 1, false)
                    .commander(CardId(1)),
            ),
            TraceMode::Detailed,
        );
        let source = Driver::object(&g, 0);
        drive(&mut g, &[], triggered);
        checked(&g);
        let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        let StackKind::Triggered {
            match_condition, ..
        } = &mut forged.state.rules.stack[0].kind
        else {
            unreachable!();
        };
        *match_condition = Some(InterveningCheck::ControlsOwnCommander {
            controller: PlayerId(0),
            commander: None,
            passed: true,
        });
        assert_forgery_rejected(&g, forged, "invalid trigger match condition proof");
        if bounce {
            Driver::pass(&mut g);
            Driver::cast(&mut g, CardId(3));
            let commander = Driver::object(&g, 1);
            Driver::pick_object(&mut g, commander);
            Driver::pay(&mut g, &[4, 5, 6]);
            for _ in 0..128 {
                let d = Driver::request(&mut g, u64::MAX);
                if g.state.rules.stack.is_empty() {
                    break;
                }
                let answer = match d.reason {
                    Reason::Priority => mtg_witness::temporary::pass(&d),
                    Reason::CommanderReturn { .. } => d
                        .pick_one(&Candidate::Flag {
                            flag: Flag::Destination,
                        })
                        .unwrap(),
                    other => panic!("unexpected decision {other:?}"),
                };
                g.respond(d.player, d.id, answer).unwrap();
            }
        } else {
            settle(&mut g);
        }
        assert_eq!(
            g.current(source).unwrap().plus_one_counters,
            i64::from(!bounce).into()
        );
        assert_eq!(intervening(&g, source), [(true, true), (false, !bounce)]);
        assert_recoverable((&g).into());
    }
}

#[test]
fn observed_events_each_successful_draw_fires_for_its_drawing_player() {
    for target in [PlayerId(0), PlayerId(1)] {
        let mut g = Game::with_trace(
            prepare(
                board(&[SHEOLDRED], TurnStep::PrecombatMain)
                    .permanent_or_hand("Sheoldred, the Apocalypse", 0, 0, false)
                    .permanent_or_hand("Sign in Blood", 0, 0, true)
                    .permanent_or_hand("Swamp", 0, 0, false)
                    .permanent_or_hand("Swamp", 0, 0, false),
            ),
            TraceMode::Detailed,
        );
        let before = life(&g, target.0 as usize);
        Driver::cast(&mut g, CardId(1));
        aim_at_player(&mut g, target);
        Driver::pay(&mut g, &[2, 3]);
        drive(&mut g, &[], |g| {
            g.state.rules.stack.len() == 2 && triggered(g)
        });
        checked(&g);
        settle(&mut g);
        let delta = if target == PlayerId(0) { 2 } else { -6 };
        assert_eq!(life(&g, target.0 as usize), plus(&before, delta));
        assert_eq!(hand(&g, target.0), 2);
        assert_recoverable((&g).into());
    }
}

#[test]
fn observed_events_becoming_target_survives_payment_only_for_completed_casts() {
    for complete in [false, true] {
        let mut g = Game::with_trace(
            prepare(
                board(
                    &[THORN_LIEUTENANT, STORMCHASER_DRAKE],
                    TurnStep::PrecombatMain,
                )
                .permanent_or_hand("Thorn Lieutenant", 1, 1, false)
                .permanent_or_hand("Stormchaser Drake", 0, 0, false)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
            ),
            TraceMode::Detailed,
        );
        Driver::cast(&mut g, CardId(2));
        let thorn = Driver::object(&g, 0);
        Driver::pick_object(&mut g, thorn);
        let _ = Driver::request(&mut g, u64::MAX);
        checked(&g);
        for (field, value, error) in [
            (
                "source/controller",
                serde_json::json!(0),
                "target announcement witness invariant",
            ),
            (
                "witness/target_controller_live",
                serde_json::json!(false),
                "trigger event witness invariant",
            ),
        ] {
            let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
            let stack = forged.state.rules.stack.last_mut().unwrap();
            let mut wire = serde_json::to_value(&*stack).unwrap();
            let path = format!("/target_triggers/0/{field}");
            *wire.pointer_mut(&path).unwrap() = value;
            *stack = serde_json::from_value(wire).unwrap();
            assert_forgery_rejected(&g, forged, error);
        }
        Driver::pay(&mut g, if complete { &[4] } else { &[] });
        settle(&mut g);
        assert_eq!(g.state.rules.tokens.len(), usize::from(complete));

        let cards_before = hand(&g, 0);
        Driver::cast(&mut g, CardId(3));
        let drake = Driver::object(&g, 1);
        Driver::pick_object(&mut g, drake);
        Driver::pay(&mut g, &[5]);
        settle(&mut g);
        assert_eq!(hand(&g, 0), cards_before);
        assert_recoverable((&g).into());
    }
}

#[test]
fn observed_events_nightstalker_waits_for_blockers() {
    for blocked in [false, true] {
        let mut g = Game::with_trace(
            prepare(
                board(&[ABYSSAL_NIGHTSTALKER], TurnStep::BeginningCombat)
                    .permanent_or_hand("Abyssal Nightstalker", 0, 0, false)
                    .permanent_or_hand("Grizzly Bears", 1, 1, false)
                    .permanent_or_hand("Island", 1, 1, true),
            ),
            TraceMode::Detailed,
        );
        let attacker = Driver::object(&g, 0);
        let blocker = Driver::object(&g, 1);
        for _ in 0..128 {
            let d = Driver::request(&mut g, u64::MAX);
            if g.state.rules.turn.step == TurnStep::DeclareBlockers
                && d.reason == Reason::Priority
                && g.state.rules.stack.is_empty()
            {
                break;
            }
            let answer = match d.reason {
                Reason::Priority => mtg_witness::temporary::pass(&d),
                Reason::Attackers => d.attack(vec![(attacker, PlayerId(1))]),
                Reason::Blockers => d.block(if blocked {
                    vec![(blocker, attacker)]
                } else {
                    vec![]
                }),
                Reason::ResolveDiscard => {
                    assert_eq!(d.player, PlayerId(1));
                    d.pick_one(&Candidate::Object {
                        object: Driver::object(&g, 2),
                    })
                    .unwrap()
                }
                other => panic!("unexpected decision {other:?}"),
            };
            g.respond(d.player, d.id, answer).unwrap();
        }
        assert_eq!(g.state.rules.turn.step, TurnStep::DeclareBlockers);
        assert_eq!(hand(&g, 1), usize::from(blocked));
        assert_recoverable((&g).into());
    }
}

#[test]
fn observed_events_spell_cast_subtype_and_mixed_entry_branch_resolve() {
    let mut g = Game::with_trace(
        prepare(
            board(&[STAFF, WORTHY_KNIGHT], TurnStep::PrecombatMain)
                .permanent_or_hand("Staff of the Flame Magus", 0, 0, false)
                .permanent_or_hand("Worthy Knight", 0, 0, false)
                .permanent_or_hand("Worthy Knight", 0, 0, true)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, true),
        ),
        TraceMode::Detailed,
    );
    let before = life(&g, 0);
    Driver::cast(&mut g, CardId(2));
    Driver::pay(&mut g, &[4, 5]);
    settle(&mut g);
    assert_eq!(g.state.rules.tokens.len(), 1);
    assert_eq!(life(&g, 0), before);

    Driver::cast(&mut g, CardId(3));
    aim_at_player(&mut g, PlayerId(1));
    Driver::pay(&mut g, &[6]);
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&before, 1));
    let land = Driver::object(&g, 7);
    Driver::action(&mut g, Action::PlayLand { object: land });
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&before, 2));
    assert_recoverable((&g).into());
}

#[test]
fn observed_events_regent_keeps_unresolved_target_actor_rejected() {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, THUNDERBREAK_REGENT);
    assert!(
        matches!(result, mtg_compiler::CompileResult::Unsupported { .. }),
        "{result:?}"
    );
}

#[test]
fn observed_events_an_opponents_activated_ability_can_target_thorn_lieutenant() {
    let mut g = Game::with_trace(
        prepare(
            board(&[THORN_LIEUTENANT], TurnStep::PrecombatMain)
                .permanent_or_hand("Thorn Lieutenant", 1, 1, false)
                .permanent_or_hand("Icy Manipulator", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let target = Driver::object(&g, 0);
    let manipulator = Driver::object(&g, 1);
    icy(&mut g, manipulator, target, 2);
    settle(&mut g);
    assert!(g.current(target).unwrap().tapped);
    assert_eq!(g.state.rules.tokens.len(), 1);
    assert_recoverable((&g).into());
}

#[test]
fn observed_events_warhound_compares_each_opponents_lands_separately() {
    const LOYAL_WARHOUND: &str = record!("cc6a83c7-e645-4a53-9550-be79b42cd851");
    for ahead in [false, true] {
        let mut setup = board(&[LOYAL_WARHOUND], TurnStep::PrecombatMain)
            .seats(4)
            .seed([8; 32])
            .permanent_or_hand("Loyal Warhound", 0, 0, true)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Island", 1, 1, false)
            .permanent_or_hand("Island", 1, 1, false)
            .permanent_or_hand("Island", 2, 2, false)
            .permanent_or_hand("Island", 2, 2, false)
            .library_card("Plains", 0);
        if ahead {
            setup = setup.permanent_or_hand("Island", 1, 1, false);
        }
        let mut g = Game::with_trace(prepare(setup), TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2]);
        drive(&mut g, &[], |g| {
            triggered(g) || g.state.rules.stack.is_empty()
        });
        let source = Driver::object(&g, 0);
        let plains = Driver::object(&g, 7);
        assert_eq!(triggered(&g), ahead);
        if ahead {
            checked(&g);
            let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
            let StackKind::Triggered {
                match_condition, ..
            } = &mut forged.state.rules.stack[0].kind
            else {
                unreachable!();
            };
            let Some(InterveningCheck::OpponentControlsMore { own, .. }) = match_condition else {
                unreachable!();
            };
            *own = 3;
            assert_forgery_rejected(&g, forged, "invalid trigger match condition proof");

            drive(&mut g, &[], |g| {
                matches!(
                    g.state.rules.frame,
                    Frame::ResolutionChoice {
                        choice: ResolutionChoice::Search { .. },
                        ..
                    }
                )
            });
            Driver::pick_object(&mut g, plains);
            settle(&mut g);
            let fetched = g.current(Driver::object(&g, 7)).unwrap();
            assert_eq!(fetched.zone, Zone::Battlefield);
            assert!(fetched.tapped);
            assert_eq!(intervening(&g, source), [(true, true), (false, true)]);
        } else {
            assert_eq!(
                g.current(plains).unwrap().zone,
                Zone::Library { owner: PlayerId(0) }
            );
            assert_eq!(intervening(&g, source), [(true, false)]);
        }
        assert_recoverable((&g).into());
    }
}

#[test]
fn observed_events_countering_the_trigger_does_not_target_its_permanent() {
    const STIFLE: &str = record!("b3b00911-ece7-4484-bc36-f211ce72b6cc");
    let mut g = Game::with_trace(
        prepare(
            board(&[THORN_LIEUTENANT, STIFLE], TurnStep::PrecombatMain)
                .permanent_or_hand("Thorn Lieutenant", 1, 1, false)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Stifle", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let thorn = Driver::object(&g, 0);
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, thorn);
    Driver::pay(&mut g, &[3]);
    drive(&mut g, &[], triggered);
    let target = g.state.rules.stack.last().unwrap().id;

    Driver::cast(&mut g, CardId(2));
    let d = Driver::request(&mut g, u64::MAX);
    let answer = d.pick_one(&Candidate::Stack { stack: target }).unwrap();
    g.respond(d.player, d.id, answer).unwrap();
    Driver::pay(&mut g, &[4]);
    settle(&mut g);

    assert_eq!(
        g.current(Driver::object(&g, 0)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert!(g.state.rules.tokens.is_empty());
    assert_eq!(
        g.events()
            .iter()
            .filter(|event| matches!(event,
                Event::TriggerCollected { source, .. } if *source == thorn
            ))
            .count(),
        1
    );
    assert_recoverable((&g).into());
}

#[test]
fn observed_events_keeper_compares_only_the_opponent_whose_step_began() {
    const KEEPER: &str = record!("49b9a3ac-d245-431a-bf40-64d1b8dc2b91");
    for ahead in [false, true] {
        let mut setup = board_with(compiled(&[KEEPER], false), TurnStep::PrecombatMain)
            .seats(3)
            .permanent_or_hand("Keeper of the Accord", 1, 1, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 1, 1, false);
        for _ in 0..3 {
            setup = setup.permanent_or_hand("Grizzly Bears", 2, 2, false);
        }
        if ahead {
            setup = setup.permanent_or_hand("Grizzly Bears", 0, 0, false);
        }
        let mut game = Game::with_trace(prepare(setup), TraceMode::Detailed);
        let keeper = Driver::object(&game, 0);
        let bear = Driver::object(&game, 1);
        drive(&mut game, &[], |g| g.state.rules.turn.step == TurnStep::End);
        assert_eq!(triggered(&game), ahead);
        checked(&game);
        if ahead {
            Driver::cast(&mut game, CardId(2));
            Driver::pick_object(&mut game, bear);
            Driver::pay(&mut game, &[3]);
            settle(&mut game);
        }
        assert_eq!(
            intervening(&game, keeper)
                .into_iter()
                .filter(|&(at_match, passed)| !at_match || passed)
                .collect::<Vec<_>>(),
            if ahead {
                vec![(true, true), (false, false)]
            } else {
                vec![]
            }
        );
        assert!(game.state.rules.tokens.is_empty());
    }
}

#[test]
fn observed_events_map_compares_only_the_entering_lands_controller() {
    const MAP: &str = record!("ef833546-f755-4ac5-867a-3926984e68a0");
    let mut game = Game::with_trace(
        prepare(
            board_with(compiled(&[MAP], false), TurnStep::PrecombatMain)
                .seats(3)
                .permanent_or_hand("Archaeomancer's Map", 1, 1, false)
                .permanent_or_hand("Island", 0, 0, true)
                .permanent_or_hand("Island", 2, 2, false)
                .permanent_or_hand("Plains", 2, 2, false),
        ),
        TraceMode::Detailed,
    );
    let map = Driver::object(&game, 0);
    let land = Driver::object(&game, 1);
    Driver::action(&mut game, Action::PlayLand { object: land });
    drive(&mut game, &[], triggered);
    let mut forged: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
    let StackKind::Triggered {
        match_condition: Some(InterveningCheck::OpponentControlsMore { opponents, .. }),
        ..
    } = &mut forged.state.rules.stack.last_mut().unwrap().kind
    else {
        panic!("missing entrant controller comparison");
    };
    assert_eq!(*opponents, [(PlayerId(0), 1)]);
    opponents[0].0 = PlayerId(2);
    assert_forgery_rejected(&game, forged, "invalid trigger match condition proof");
    checked(&game);
    settle(&mut game);
    assert_eq!(intervening(&game, map), [(true, true), (false, true)]);
}
