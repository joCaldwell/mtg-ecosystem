//! Exact printed control programs and their recovery boundaries.
//! CR 101.4, 115.3, 121.2c, 205.3m, 608.2b–e, 700.2d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

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
    let old = compile(mtg_witness::R5_MANIFEST, records.0);
    let new = compile(r6, records.1);
    assert_eq!(old.spell, new.spell);
    assert_eq!(old.modal_spell, new.modal_spell);
    assert_eq!(old.activated, new.activated);
    assert_eq!(old.triggered, new.triggered);
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(*new);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "control-results-witness")
        .seats(seats)
        .step(step)
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
    let mut game = Game::with_trace(
        board(
            frozen!("a4e5693f-12a0-451e-818d-d6efc7b4ed25"),
            2,
            TurnStep::PrecombatMain,
        )
        .permanent_or_hand("Decimate", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Sol Ring", 1, 1, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Glorious Anthem", 1, 1, false)
        .permanent_or_hand("Forest", 1, 1, false)
        .libraries("Island", 2)
        .prepare(),
        TraceMode::Detailed,
    );
    Driver::cast(&mut game, CardId(0));
    let targets = Driver::request(&mut game, 1);
    let picks = (5..=8)
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
    settle(&mut game);
    for id in 5..=8 {
        assert_eq!(
            game.zone(Driver::object(&game, id)),
            Some(Zone::Graveyard { owner: PlayerId(1) })
        );
    }
    assert!(game.trace().records.iter().any(|entry| {
        matches!(&entry.record, EvidenceRecord::Batch { batch }
            if matches!(&batch.group, EventGroup::Simultaneous { occurrences }
                if occurrences.len() == 4 && occurrences.iter().all(|occurrence| matches!(occurrence, ProposedEvent::DestroyPermanent { .. }))))
    }));
}

#[test]
fn lane2_distant_melody_binds_the_type_at_resolution() {
    let mut game = Game::new(
        board(
            frozen!("ac6c5852-71c7-4f19-8c87-ccb345052862"),
            2,
            TurnStep::PrecombatMain,
        )
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
    answer(
        &mut game,
        choice,
        Candidate::CreatureType {
            kind: mtg_contracts::kir::CreatureType::from_name("Bear").unwrap(),
        },
    );
    settle(&mut game);
    assert_eq!(hand(&game, PlayerId(0)), 2);
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
