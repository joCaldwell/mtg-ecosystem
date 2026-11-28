//! Exact printed control programs and their recovery boundaries.
//! CR 101.4, 115.3, 121.2c, 205.3m, 608.2b–e, 700.2d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_observe::action::{Composer, STOP, Step};
use mtg_sim::policy::{Policy, Strategy};

macro_rules! frozen {
    ($id:literal) => {
        (
            include_str!(concat!(
                "../../../../../tests/fixtures/phase1-r5/",
                $id,
                ".json"
            )),
            include_str!(concat!(
                "../../../../../tests/fixtures/phase1-r6/",
                $id,
                ".json"
            )),
        )
    };
}

fn board(records: (&str, &str), seats: u8, step: TurnStep) -> Board {
    Board::new(catalog(&[records]), "control-results-witness")
        .seats(seats)
        .step(step)
}

fn catalog(records: &[(&str, &str)]) -> mtg_contracts::Catalog {
    let r6 = include_str!(
        "../../../../../tests/fixtures/archive-manifest-r6-candidate-6a4226a5e7c5ed8d0044770ac4ededc8c2e5586217b80e53b454dd4a9f952b37.json"
    );
    let compile = |manifest, record| {
        let result = mtg_witness::compile(manifest, record);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen control fixture failed: {result:?}");
        };
        definition
    };
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(r5_record, r6_record) in records {
        let old = compile(mtg_witness::R5_MANIFEST, r5_record);
        let new = compile(r6, r6_record);
        assert_eq!(old.spell, new.spell);
        assert_eq!(old.modal_spell, new.modal_spell);
        assert_eq!(old.activated, new.activated);
        assert_eq!(old.triggered, new.triggered);
        catalog.definitions.push(*new);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog
}

fn observe(game: &Game, seat: PlayerId) -> mtg_core::Observation {
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        game.observe(seat, Budget::unlimited())
    else {
        panic!("missing permitted view");
    };
    observation
}

fn next_resolution_choice(game: &mut Game) -> Decision<Rules> {
    for _ in 0..16 {
        let request = Driver::request(game, 1);
        if request.reason != Reason::Priority {
            return request;
        }
        assert!(!game.state.rules.stack.is_empty(), "resolved before choice");
        Driver::pass(game);
    }
    panic!("resolution choice was not offered");
}

fn settle(game: &mut Game) {
    for _ in 0..16 {
        let request = Driver::request(game, 1);
        assert_eq!(request.reason, Reason::Priority, "{request:?}");
        if game.state.rules.stack.is_empty() {
            assert_recoverable((&*game).into());
            return;
        }
        Driver::pass(game);
    }
    panic!("stack did not settle");
}

fn hand(game: &Game, player: PlayerId) -> usize {
    game.state
        .rules
        .objects
        .values()
        .filter(|object| object.zone == (Zone::Hand { owner: player }))
        .count()
}

fn answer(game: &mut Game, request: Decision<Rules>, candidate: Candidate<Rules>) {
    game.respond(
        request.player,
        request.id,
        request.pick_one(&candidate).unwrap(),
    )
    .unwrap();
}

#[test]
fn lane2_decimate_groups_independent_targets_in_one_destruction_event() {
    decimate([5, 6, 7, 8], false, 4);
}

#[test]
fn lane2_decimate_resolves_remaining_targets_after_one_leaves() {
    decimate([5, 6, 7, 8], true, 3);
}

#[test]
fn lane2_decimate_destroys_the_same_artifact_creature_only_once() {
    decimate([5, 5, 7, 8], false, 3);
}

fn decimate(target_ids: [u32; 4], bounce_creature: bool, destroyed: usize) {
    let mut game = Game::with_trace(
        Board::new(
            catalog(&[
                frozen!("a4e5693f-12a0-451e-818d-d6efc7b4ed25"),
                frozen!("a3a98bc9-caa0-49b7-951c-fe4e4f54e4ba"),
            ]),
            "decimate-witness",
        )
        .seats(2)
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand("Decimate", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Ornithopter", 1, 1, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Glorious Anthem", 1, 1, false)
        .permanent_or_hand("Forest", 1, 1, false)
        .permanent_or_hand("Unsummon", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .libraries("Island", 2)
        .prepare(),
        TraceMode::Detailed,
    );
    Driver::cast(&mut game, CardId(0));
    let targets = Driver::request(&mut game, 1);
    let picks = target_ids
        .into_iter()
        .map(|id| {
            vec![
                targets
                    .index(&Candidate::Object {
                        object: Driver::object(&game, id),
                    })
                    .unwrap(),
            ]
        })
        .collect();
    game.respond(targets.player, targets.id, Answer::Pick { picks })
        .unwrap();
    Driver::pay(&mut game, &[1, 2, 3, 4]);
    if bounce_creature {
        Driver::cast(&mut game, CardId(9));
        let creature = Driver::object(&game, 6);
        Driver::pick_object(&mut game, creature);
        Driver::pay(&mut game, &[10]);
    }
    settle(&mut game);
    for id in 5..=8 {
        let expected = match id {
            6 if bounce_creature => Zone::Hand { owner: PlayerId(1) },
            6 if !target_ids.contains(&6) => Zone::Battlefield,
            _ => Zone::Graveyard { owner: PlayerId(1) },
        };
        assert_eq!(game.zone(Driver::object(&game, id)), Some(expected));
    }
    assert!(game.trace().records.iter().any(|entry| {
        matches!(&entry.record, EvidenceRecord::Batch { batch }
            if matches!(&batch.group, EventGroup::Simultaneous { occurrences }
                if occurrences.len() == destroyed && occurrences.iter().all(|occurrence| matches!(occurrence, ProposedEvent::DestroyPermanent { .. }))))
    }));
}

#[test]
fn lane2_sibilant_spirit_offers_only_the_defender_a_draw() {
    let mut game = Game::new(
        board(
            frozen!("9798d307-a44a-4f44-94c7-03e499b94962"),
            2,
            TurnStep::BeginningCombat,
        )
        .permanent_or_hand("Sibilant Spirit", 0, 0, false)
        .libraries("Island", 2)
        .prepare(),
    );
    let spirit = Driver::object(&game, 0);
    let mut attackers = Driver::request(&mut game, 1024);
    for _ in 0..4 {
        if attackers.reason != Reason::Priority {
            break;
        }
        Driver::pass(&mut game);
        attackers = Driver::request(&mut game, 1024);
    }
    assert_eq!(attackers.reason, Reason::Attackers);
    game.respond(
        attackers.player,
        attackers.id,
        attackers.attack(vec![(spirit, PlayerId(1))]),
    )
    .unwrap();
    let choice = next_resolution_choice(&mut game);
    assert_eq!(
        (choice.player, &choice.reason),
        (PlayerId(1), &Reason::OptionalEffect)
    );
    assert_recoverable((&game).into());
    answer(&mut game, choice, Candidate::Flag { flag: Flag::Accept });
    settle(&mut game);
    assert_eq!(hand(&game, PlayerId(0)), 0);
    assert_eq!(hand(&game, PlayerId(1)), 1);
}

#[test]
fn lane2_distant_melody_binds_the_type_at_resolution() {
    let records = frozen!("ac6c5852-71c7-4f19-8c87-ccb345052862");
    let catalog = catalog(&[records]);
    let mut game = Game::new(
        board(records, 2, TurnStep::PrecombatMain)
            .permanent_or_hand("Distant Melody", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .libraries("Forest", 3)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1, 2, 3, 4]);
    let choice = next_resolution_choice(&mut game);
    assert_eq!(
        (choice.player, &choice.reason),
        (PlayerId(0), &Reason::ChooseCreatureType)
    );
    assert_recoverable((&game).into());
    let observation = observe(&game, choice.player);
    let bear = Candidate::CreatureType {
        kind: mtg_contracts::kir::CreatureType::from_name("Bear").unwrap(),
    };
    let decision = observation.decision.as_ref().unwrap();
    let answer = Strategy::new(&catalog, Policy::Develop, [0; 4])
        .choose_with_game(&observation, &game)
        .unwrap();
    assert_eq!(answer, decision.pick_one(&bear).unwrap());
    assert_eq!(
        Strategy::new(&catalog, Policy::Develop, [0; 4])
            .choose(&observation)
            .unwrap(),
        answer
    );

    // The RL head keeps the full legal vocabulary, including its final page.
    let last = decision.candidates.len() - 1;
    let prefix = vec![STOP; last / STOP];
    let Step::Choose(choices) = Composer::compose(decision, &prefix, None).unwrap() else {
        panic!("expected the final creature-type page");
    };
    let observer = mtg_observe::catalog::ObservationCatalog::new(catalog.into());
    let row = mtg_observe::encode(&observer, &observation, prefix.len(), &choices, true).unwrap();
    let Candidate::CreatureType { kind } = &decision.candidates[last] else {
        panic!("expected a creature type");
    };
    assert_eq!(row.cand[last % STOP][3], i32::from(kind.index()));
    assert_eq!(row.ptr_mask[STOP], 0);
    let mut actions = prefix;
    actions.push(last % STOP);
    let Step::Answer(rl_answer) = Composer::compose(decision, &actions, None).unwrap() else {
        panic!("the last page must yield an answer");
    };
    assert_eq!(
        rl_answer,
        decision.pick_one(&decision.candidates[last]).unwrap()
    );

    game.respond(choice.player, choice.id, answer).unwrap();
    settle(&mut game);
    assert_eq!(hand(&game, PlayerId(0)), 2);
}

#[test]
fn lane2_kwain_every_player_declines_from_their_empty_library() {
    let records = frozen!("ce6a85b5-e249-4582-a6be-ce200da5fd53");
    let catalog = catalog(&[records]);
    let mut game = Game::new(
        board(records, 4, TurnStep::PrecombatMain)
            .permanent_or_hand("Kwain, Itinerant Meddler", 0, 0, false)
            .prepare(),
    );
    let initial_lives = game
        .state
        .rules
        .players
        .iter()
        .map(|p| p.life.clone())
        .collect::<Vec<_>>();
    let kwain = Driver::object(&game, 0);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: kwain,
            ability: 0,
        },
    );
    let mut policy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
    for seat in 0..4 {
        let choice = next_resolution_choice(&mut game);
        assert_eq!(choice.player, PlayerId(seat));
        assert_eq!(
            choice.reason,
            Reason::OptionalResolution {
                previous: (0..seat).map(|prior| (PlayerId(prior), false)).collect(),
            }
        );
        let observation = observe(&game, choice.player);
        let answer = policy.choose_with_game(&observation, &game).unwrap();
        assert_eq!(
            answer,
            choice
                .pick_one(&Candidate::Flag {
                    flag: Flag::Decline
                })
                .unwrap()
        );
        game.respond(choice.player, choice.id, answer).unwrap();
    }
    settle(&mut game);
    for seat in 0..4 {
        assert_eq!(hand(&game, PlayerId(seat)), 0);
        assert_eq!(
            game.state.rules.players[usize::from(seat)].life,
            initial_lives[usize::from(seat)]
        );
    }
}

#[test]
fn lane2_seeded_sim_policy_resolves_distant_melody_and_kwain() {
    let catalog = catalog(&[
        frozen!("ac6c5852-71c7-4f19-8c87-ccb345052862"),
        frozen!("ce6a85b5-e249-4582-a6be-ce200da5fd53"),
    ]);
    let mut game = Game::with_trace(
        Board::new(catalog.clone(), "lane2-policy-witness")
            .step(TurnStep::PrecombatMain)
            .seed([47; 32])
            .permanent_or_hand("Distant Melody", 0, 0, true)
            .permanent_or_hand("Kwain, Itinerant Meddler", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .libraries("Island", 4)
            .prepare(),
        TraceMode::Detailed,
    );
    let initial_life = game.state.rules.players[0].life.clone();
    let mut seen_type = false;
    let mut accepts = 0;
    let mut resolved_both = false;
    let mut policy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
    for _ in 0..512 {
        let boundary = match Driver::settle(&mut game, 1024) {
            Advance::AwaitingDecision(decision) => decision,
            Advance::Finished { .. } | Advance::Draw if resolved_both => return,
            other => panic!("unexpected seeded simulation outcome {other:?}"),
        };
        if !resolved_both && seen_type && accepts == 4 && game.state.rules.stack.is_empty() {
            let draws = |card| {
                game.trace()
                    .records
                    .iter()
                    .filter_map(|entry| {
                        let EvidenceRecord::Batch { batch } = &entry.record else {
                            return None;
                        };
                        match batch.cause {
                            BatchCause::InstructionDraw { source, player, .. }
                                if source.card == card
                                    && matches!(batch.occurrences.as_slice(), [result]
                                if matches!(result.actual, EventOutcome::Moved { .. })) =>
                            {
                                Some(player)
                            }
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(draws(CardId(0)), vec![PlayerId(0); 2]);
            assert_eq!(draws(CardId(1)), (0..4).map(PlayerId).collect::<Vec<_>>());
            let mut life = initial_life.clone();
            life.add(&1.into());
            for player in &game.state.rules.players {
                assert_eq!(player.life, life);
            }
            resolved_both = true;
        }
        let observation = observe(&game, boundary.player);
        let answer = policy.choose_with_game(&observation, &game).unwrap();
        match boundary.reason {
            Reason::ChooseCreatureType => seen_type = true,
            Reason::OptionalResolution { .. } if !resolved_both => {
                assert_eq!(
                    answer,
                    boundary
                        .pick_one(&Candidate::Flag { flag: Flag::Accept })
                        .unwrap(),
                    "{:?} for seat {:?}",
                    boundary.reason,
                    boundary.player
                );
                accepts += 1;
            }
            _ => {}
        }
        let request = observation.decision.unwrap();
        let QueryResult::Complete(QueryValue::Validated(response)) =
            game.check_player_response(request.player, request.id, answer, Budget::unlimited())
        else {
            panic!(
                "sim policy generated an invalid response to {:?}",
                boundary.reason
            );
        };
        game.submit(response).unwrap();
    }
    panic!("the seeded simulation did not finish within 512 decisions");
}

#[test]
fn lane2_kwain_collects_in_apnap_order_then_uses_actual_drawers() {
    let mut game = Game::new(
        board(
            frozen!("ce6a85b5-e249-4582-a6be-ce200da5fd53"),
            4,
            TurnStep::PrecombatMain,
        )
        .permanent_or_hand("Kwain, Itinerant Meddler", 0, 0, false)
        .libraries("Forest", 4)
        .prepare(),
    );
    let initial_life = game.state.rules.players[0].life.clone();
    let kwain = Driver::object(&game, 0);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: kwain,
            ability: 0,
        },
    );
    for seat in 0..4 {
        let choice = next_resolution_choice(&mut game);
        assert_eq!(
            (choice.player, &choice.reason),
            (
                PlayerId(seat),
                &Reason::OptionalResolution {
                    previous: (0..seat)
                        .map(|prior| (PlayerId(prior), prior % 2 != 0))
                        .collect(),
                }
            )
        );
        assert_recoverable((&game).into());
        for viewer in 0..4 {
            assert_eq!(
                hand(&game, PlayerId(viewer)),
                0,
                "draws started before choices finished"
            );
            let QueryResult::Complete(QueryValue::Observation(observation)) =
                game.observe(PlayerId(viewer), Budget::unlimited())
            else {
                panic!("missing permitted view");
            };
            assert_eq!(observation.decision.is_some(), viewer == seat);
        }
        answer(
            &mut game,
            choice,
            Candidate::Flag {
                flag: if seat % 2 == 0 {
                    Flag::Decline
                } else {
                    Flag::Accept
                },
            },
        );
    }
    settle(&mut game);
    for seat in 0..4 {
        assert_eq!(hand(&game, PlayerId(seat)), usize::from(seat % 2));
        let mut expected = initial_life.clone();
        expected.add(&i64::from(seat % 2).into());
        assert_eq!(game.state.rules.players[usize::from(seat)].life, expected);
        let QueryResult::Complete(QueryValue::Observation(observation)) =
            game.observe(PlayerId(seat), Budget::unlimited())
        else {
            panic!("missing permitted view");
        };
        assert!(observation.view.objects.iter().all(|object| {
            !matches!(object.zone, Zone::Hand { owner } if owner != PlayerId(seat))
        }));
    }
}

#[test]
fn lane2_fiery_confluence_performs_three_copies_of_one_mode() {
    let mut game = Game::new(
        board(
            frozen!("3c22e031-4804-4c31-bd3c-c3f29d456b34"),
            2,
            TurnStep::PrecombatMain,
        )
        .permanent_or_hand("Fiery Confluence", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .libraries("Forest", 2)
        .prepare(),
    );
    let mut expected = game.state.rules.players[1].life.clone();
    expected.subtract(&6.into());
    Driver::cast(&mut game, CardId(0));
    let request = Driver::request(&mut game, 1);
    let choice = request
        .candidates
        .iter()
        .find(|c| {
            matches!(c, Candidate::Modes { labels, .. }
        if labels.len() == 3 && labels.iter().all(|l| l.contains("2 damage to each opponent")))
        })
        .unwrap()
        .clone();
    assert_recoverable((&game).into());
    answer(&mut game, request, choice);
    Driver::pay(&mut game, &[1, 2, 3, 4]);
    settle(&mut game);
    assert_eq!(game.state.rules.players[1].life, expected);
}

#[test]
fn lane2_fiery_confluence_preserves_repeat_modes_in_public_view_and_tensor() {
    let records = frozen!("3c22e031-4804-4c31-bd3c-c3f29d456b34");
    let observer = mtg_observe::catalog::ObservationCatalog::new(catalog(&[records]).into());
    let mut game = Game::new(
        board(records, 2, TurnStep::PrecombatMain)
            .permanent_or_hand("Fiery Confluence", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Sol Ring", 1, 1, false)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    let request = Driver::request(&mut game, 1);
    let observation = observe(&game, request.player);
    let decision = observation.decision.as_ref().unwrap();
    let index = request.candidates.iter().position(|candidate| {
        matches!(candidate, Candidate::Modes { chosen_modes, .. } if chosen_modes == &[0, 0, 2])
    }).unwrap();
    let Step::Choose(choices) = Composer::compose(decision, &[], None).unwrap() else {
        panic!("expected a mode decision");
    };
    let row = mtg_observe::encode(&observer, &observation, 0, &choices, true).unwrap();
    let packed = 0o311;
    assert_eq!(row.cand[index][3], packed);
    let choice = request.candidates[index].clone();
    answer(&mut game, request, choice);
    let artifact = Driver::object(&game, 5);
    Driver::pick_object(&mut game, artifact);
    Driver::pay(&mut game, &[1, 2, 3, 4]);
    Driver::pass(&mut game);
    let request = Driver::request(&mut game, 1);
    assert_eq!(request.player, PlayerId(1));
    let observation = observe(&game, request.player);
    let stack = observation.view.stack.last().unwrap();
    assert_eq!(stack.mode_mask, Some(0b101));
    assert_eq!(stack.chosen_modes.as_deref(), Some([0, 0, 2].as_slice()));
    let Step::Choose(choices) =
        Composer::compose(observation.decision.as_ref().unwrap(), &[], None).unwrap()
    else {
        panic!("expected opponent priority");
    };
    let row = mtg_observe::encode(&observer, &observation, 0, &choices, true).unwrap();
    assert_eq!(row.ent_num[observation.view.objects.len()][22], packed);
    settle(&mut game);
    assert_eq!(
        game.zone(Driver::object(&game, 5)),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
}

#[test]
fn lane2_scour_for_scrap_keeps_the_search_pronoun_in_its_mode() {
    let mut game = Game::new(
        board(
            frozen!("da422b3e-1092-47ad-97d6-a7a3ef060a53"),
            2,
            TurnStep::PrecombatMain,
        )
        .permanent_or_hand("Scour for Scrap", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .card("Sol Ring", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
        .library_card("Sol Ring", 0)
        .libraries("Forest", 2)
        .seed([9; 32])
        .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    let request = Driver::request(&mut game, 1);
    let choice = request
        .candidates
        .iter()
        .find(|c| matches!(c, Candidate::Modes { mask: 3, .. }))
        .unwrap()
        .clone();
    answer(&mut game, request, choice);
    let graveyard = Driver::object(&game, 5);
    Driver::pick_object(&mut game, graveyard);
    Driver::pay(&mut game, &[1, 2, 3, 4]);
    let search = next_resolution_choice(&mut game);
    assert_eq!(search.reason, Reason::SearchLibrary);
    assert_recoverable((&game).into());
    let found = Driver::object(&game, 6);
    answer(&mut game, search, Candidate::Object { object: found });
    settle(&mut game);
    for id in [5, 6] {
        assert_eq!(
            game.zone(Driver::object(&game, id)),
            Some(Zone::Hand { owner: PlayerId(0) })
        );
    }
}

#[test]
fn lane2_hollowhenge_counts_each_matching_creature_once() {
    let mut game = Game::new(
        board(
            frozen!("0e2418f8-2f32-4f79-8ce7-44eae8e764cb"),
            2,
            TurnStep::Untap,
        )
        .permanent_or_hand("Hollowhenge Overlord", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Hollowhenge Overlord", 1, 1, false)
        .libraries("Forest", 2)
        .prepare(),
    );
    for _ in 0..=game.live_players().len() {
        Driver::request(&mut game, 1);
        if game.state.rules.turn.step == TurnStep::Upkeep {
            break;
        }
        Driver::pass(&mut game);
    }
    assert_eq!(game.state.rules.stack.len(), 1);
    settle(&mut game);
    let tokens = game
        .state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && o.controller == PlayerId(0)
                && game.state.rules.tokens.contains_key(&o.id.card)
        })
        .count();
    assert_eq!(tokens, 1);
}
