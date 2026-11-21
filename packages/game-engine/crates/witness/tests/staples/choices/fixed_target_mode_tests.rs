use super::*;
use mtg_witness::equipped::{
    answer, object, pay_and_resolve, pay_to_priority, prepared, recover, request,
};
#[test]
fn paid_repeated_modes_resolve_all_five_printed_effects_and_preserve_duplicate_targets() {
    let result = mtg_witness::compile(
        include_str!("../../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!(
            "../../../../../tests/fixtures/ramos/164651c5-b831-4070-8491-16d30ac8395b.json"
        ),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    let mut p = prepared(
        &[
            ("Forest", true),
            ("Plains", false),
            ("Island", false),
            ("Swamp", false),
            ("Mountain", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
            ("Grizzly Bears", false),
            ("Short Sword", false),
            ("Forest", false),
            ("Island", false),
        ],
        false,
    );
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let seat = PlayerId(1);
    for n in [10, 11] {
        p.config.cards[n].owner = seat;
        p.config.cards[n].controller = seat;
    }
    p.config.cards[10].zone = Zone::Graveyard { owner: seat };
    p.config.cards[11].zone = Zone::Library { owner: seat };
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.push(*definition);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&*p.catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    let spell = object(&g, 0).card;
    let bear = object(&g, 8);
    let sword = object(&g, 9);
    answer(&mut g, 1, Response::Cast { card: spell });
    let offered = request(&mut g, 1);
    assert!(matches!(
        offered,
        Request::Modes {
            allow_repeat: true,
            exactly: Some(5),
            ..
        }
    ));
    let mut repeated = g.fork();
    answer(&mut repeated, 1, Response::ModeSubset { modes: vec![3; 5] });
    for _ in 0..5 {
        answer(
            &mut repeated,
            1,
            Response::EntityTargets {
                entities: vec![TargetEntity::Player { player: seat }],
            },
        );
    }
    pay_and_resolve(&mut repeated, 1, vec![]);
    assert_eq!(repeated.state.rules.players[1].life, 30.into());
    recover(&repeated);
    answer(
        &mut g,
        1,
        Response::ModeSubset {
            modes: vec![4, 3, 2, 1, 0],
        },
    );
    answer(
        &mut g,
        1,
        Response::Targets {
            objects: vec![bear],
        },
    );
    for _ in 0..2 {
        answer(
            &mut g,
            1,
            Response::PlayerTargets {
                players: vec![seat],
            },
        );
    }
    answer(
        &mut g,
        1,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player { player: seat }],
        },
    );
    answer(
        &mut g,
        1,
        Response::Targets {
            objects: vec![sword],
        },
    );
    pay_to_priority(&mut g, 1, vec![]);
    let mut saved = None;
    for _ in 0..500 {
        if let Frame::Resolve { frame } = &g.state.rules.frame
            && frame.cursor == 1
        {
            assert_eq!(frame.legal_targets, [true; 5]);
            assert_eq!(
                frame
                    .target_facts
                    .iter()
                    .map(Option::is_some)
                    .collect::<Vec<_>>(),
                [true, false, false, false, true]
            );
            saved =
                Some(Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap());
            break;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r @ Request::Priority { .. }) => g
                .respond(r.player(), r.decision_id(), Response::Pass)
                .unwrap(),
            other => panic!("expected a paid resolving frame: {other:?}"),
        }
    }
    let mut restored = saved.expect("missing partially resolved modal spell");
    for game in [&mut g, &mut restored] {
        while !game.state.rules.stack.is_empty() {
            let r = request(game, 1);
            if !game.state.rules.stack.is_empty() {
                assert!(matches!(r, Request::Priority { .. }));
                answer(game, 1, Response::Pass);
            }
        }
    }
    assert_eq!(g.state, restored.state);
    assert_eq!(g.trace(), restored.trace());
    assert!(g.current(bear).unwrap().phased_out.is_some());
    assert_eq!(
        g.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == (Zone::Hand { owner: seat }))
            .count(),
        1
    );
    assert_eq!(g.zone(object(&g, 10)), Some(Zone::Exile));
    assert_eq!(g.state.rules.players[1].life, 38.into());
    assert!(matches!(
        g.zone(object(&g, 9)),
        Some(Zone::Graveyard { .. })
    ));
    recover(&g);
}

#[test]
fn paid_mixed_target_modes_place_counters_then_shuffle_opposing_graveyard_and_replay() {
    let p = crate::staples_tests::seeded_mixed_target_fixture(include_str!(
        "../../../../../tests/fixtures/ramos/9c0f7cdc-9095-40ae-bf83-75a5ffda72b9.json"
    ));
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let owner = PlayerId(1);
    let mut board = None;
    for _ in 0..1200 {
        let r = request(&mut g, 1);
        let seat = r.player();
        let response = match &r {
            Request::Mulligan { .. } => Response::Mulligan { take: false },
            Request::Priority { .. }
                if g.active_player() == Some(seat)
                    && g.turn().step == TurnStep::PrecombatMain
                    && matches!(seat.0, 0 | 2) =>
            {
                let lands = g
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| {
                        o.zone == Zone::Battlefield
                            && o.controller == seat
                            && g.definition(o.id.card)
                                .unwrap()
                                .characteristics
                                .has_type(CardKind::Land)
                    })
                    .count();
                if seat == PlayerId(0) && lands >= 3 {
                    let find = |zone, name| {
                        g.state
                            .rules
                            .objects
                            .values()
                            .find(|o| {
                                o.zone == zone && g.definition(o.id.card).unwrap().name == name
                            })
                            .unwrap()
                            .id
                    };
                    let card = find(Zone::Hand { owner: seat }, "Quandrix Command").card;
                    let creature = find(Zone::Battlefield, "Isamaru, Hound of Konda");
                    let buried = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| o.zone == Zone::Graveyard { owner })
                        .unwrap()
                        .id;
                    board = Some((card, creature, buried));
                    break;
                }
                if seat == PlayerId(2)
                    && lands > 0
                    && let Some(id) = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| o.zone == Zone::Command { owner: seat })
                        .map(|o| o.id.card)
                {
                    answer(&mut g, 1, Response::Cast { card: id });
                    pay_and_resolve(&mut g, 1, vec![]);
                    continue;
                }
                if g.turn().lands_played == 0 && (seat == PlayerId(0) || lands == 0) {
                    let name = if seat == PlayerId(2) {
                        "Plains"
                    } else if lands == 0 {
                        "Island"
                    } else {
                        "Forest"
                    };
                    let card = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.zone == Zone::Hand { owner: seat }
                                && g.definition(o.id.card).unwrap().name == name
                        })
                        .unwrap()
                        .id
                        .card;
                    Response::PlayLand { card }
                } else {
                    Response::Pass
                }
            }
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates
                    .iter()
                    .copied()
                    .filter(|o| g.definition(o.card).unwrap().name != "Quandrix Command")
                    .take(*count)
                    .collect(),
            },
            other => panic!("unexpected seeded development {other:?}"),
        };
        g.respond(seat, r.decision_id(), response).unwrap();
    }
    let (card, creature, buried) = board.expect("paid mixed-mode opening");
    let before = g.state.rules.match_state.as_ref().unwrap().library_shuffles;
    let mut departed = g.fork();
    answer(&mut departed, 1, Response::Cast { card });
    answer(&mut departed, 1, Response::ModeSubset { modes: vec![0, 3] });
    answer(
        &mut departed,
        1,
        Response::Targets {
            objects: vec![creature],
        },
    );
    answer(
        &mut departed,
        1,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player { player: owner }],
        },
    );
    pay_to_priority(&mut departed, 1, vec![]);
    departed.concede(PlayerId(2)).unwrap();
    let mut recovered = false;
    for _ in 0..500 {
        if let Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Shuffle { player, .. },
        } = &departed.state.rules.frame
        {
            assert_eq!(*player, owner);
            assert_eq!(frame.legal_targets, [false, true]);
            recover(&departed);
            recovered = true;
            break;
        }
        match departed.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r @ Request::Priority { .. }) => departed
                .respond(r.player(), r.decision_id(), Response::Pass)
                .unwrap(),
            other => panic!("expected shuffle after target departure: {other:?}"),
        }
    }
    assert!(recovered, "missing shuffle continuation");
    answer(&mut g, 1, Response::Cast { card });
    answer(&mut g, 1, Response::ModeSubset { modes: vec![3, 2] });
    assert!(
        matches!(request(&mut g,1),Request::Targets{candidates,..} if candidates.contains(&creature))
    );
    answer(
        &mut g,
        1,
        Response::Targets {
            objects: vec![creature],
        },
    );
    assert!(matches!(request(&mut g, 1), Request::EntityTargets { .. }));
    recover(&g);
    let mut forged = g.fork();
    let Frame::EntityTargets { cast } = &forged.state.rules.frame else {
        panic!("expected dependent player selection")
    };
    forged.state.rules.frame = Frame::Targets { cast: cast.clone() };
    assert!(
        Game::restore(
            forged.prepared.as_ref().clone(),
            &forged.snapshot().unwrap()
        )
        .is_err()
    );
    for select_card in [false, true] {
        let mut branch = g.fork();
        let mut entities = vec![TargetEntity::Player { player: owner }];
        if select_card {
            entities.push(TargetEntity::Object { object: buried });
        }
        answer(&mut branch, 1, Response::EntityTargets { entities });
        pay_and_resolve(&mut branch, 1, vec![]);
        assert_eq!(
            branch.current(creature).unwrap().plus_one_counters,
            2.into()
        );
        assert_eq!(
            branch.zone(branch.object_id(buried.card).unwrap()),
            Some(if select_card {
                Zone::Library { owner }
            } else {
                Zone::Graveyard { owner }
            })
        );
        let mut expected = before;
        expected[owner.0 as usize] += 1;
        assert_eq!(
            branch
                .state
                .rules
                .match_state
                .as_ref()
                .unwrap()
                .library_shuffles,
            expected
        );
        recover(&branch);
    }
}

#[test]
fn paid_exact_two_targeted_modes_bind_distinct_occurrences_and_replay_print_order() {
    let raw = include_str!("../../../../../tests/fixtures/synthetic-fixed-target-modes.json");
    let result = mtg_witness::compile(
        include_str!("../../../../../tests/fixtures/archive-manifest-current.json"),
        raw,
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    let mut p = prepared(&[("Forest", true), ("Forest", false)], false);
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.push(*definition);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&*p.catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    let card = object(&g, 0).card;
    answer(&mut g, 1, Response::Cast { card });
    let modes = request(&mut g, 1);
    assert!(matches!(
        modes,
        Request::Modes {
            exactly: Some(2),
            ..
        }
    ));
    let caster = modes.player();
    let revision = modes.decision_id();
    for ids in [vec![0], vec![1, 1], vec![0, 1, 2]] {
        assert!(
            g.respond(caster, revision, Response::ModeSubset { modes: ids })
                .is_err()
        );
    }
    answer(&mut g, 1, Response::ModeSubset { modes: vec![1, 0] });
    let r = request(&mut g, 1);
    assert!(
        matches!(&r,Request::PlayerTargets{occurrences,..} if occurrences.iter().map(|s|s.slot).collect::<Vec<_>>()==[0,1])
    );
    let mut split = g.fork();
    answer(
        &mut split,
        1,
        Response::PlayerTargets {
            players: vec![PlayerId(1), PlayerId(2)],
        },
    );
    loop {
        let r = request(&mut split, 1);
        let response = match &r {
            Request::Priority { .. } => break,
            Request::Mana { cost, .. } if split.payment_pool(caster, cost).is_ok() => {
                Response::FinishMana { undo_mana: false }
            }
            Request::Mana { .. } => Response::ActivateMana {
                object: object(&split, 1),
            },
            Request::SpendMana { .. } => Response::SpendMana {
                spending: BTreeMap::from([(Color::G, 1.into())]),
            },
            other => panic!("unexpected paid mode request {other:?}"),
        };
        answer(&mut split, 1, response);
    }
    let id = split.top().unwrap().id;
    recover(&split);
    for all in [false, true] {
        let mut branch = split.fork();
        branch.concede(PlayerId(1)).unwrap();
        if all {
            branch.concede(PlayerId(2)).unwrap();
        }
        while !branch.state.rules.stack.is_empty() {
            let r = request(&mut branch, 1);
            if branch.state.rules.stack.is_empty() {
                break;
            }
            assert!(matches!(r, Request::Priority { .. }));
            answer(&mut branch, 1, Response::Pass);
        }
        assert!(branch.events().iter().any(|e|matches!(e,Event::SpellCompleted{stack_id,resolved} if *stack_id==id && *resolved==!all)));
        if !all {
            assert_eq!(branch.state.rules.players[2].life, 38.into());
        }
        recover(&branch);
    }
    // Reusing the same player across distinct occurrences is legal.
    answer(
        &mut g,
        1,
        Response::PlayerTargets {
            players: vec![PlayerId(1), PlayerId(1)],
        },
    );
    recover(&g);
    pay_and_resolve(&mut g, 1, vec![]);
    assert_eq!(g.state.rules.players[1].life, 41.into());
    recover(&g);
}
