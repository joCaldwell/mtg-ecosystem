//! Frozen R5 witnesses. CR 107.1b, 111.2, 120.2, 121.2, 608.2b/h,
//! 611.2c, 613.1f, 613.7b, 701.26, 800.4a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, Tiers, definition_cells, definition_tiers};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

fn board(cards: &[(&str, Cell)]) -> Board {
    board_with(cards, None)
}

fn board_with(cards: &[(&str, Cell)], support: Option<&Definition>) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.extend(support.cloned());
    for &(raw, cell) in cards {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen verb witness failed: {result:?}");
        };
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        assert!(definition_cells(&definition).unwrap().contains(&cell));
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-verb-operands").step(TurnStep::PrecombatMain)
}

fn resolve(game: &mut Game) {
    for _ in 0..24 {
        let request = Driver::request(game, 1);
        assert_eq!(request.reason, Reason::Priority);
        if game.state.rules.stack.is_empty() {
            game.validate_state().unwrap();
            assert_recoverable((&*game).into());
            return;
        }
        let answer = request
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        game.respond(request.player, request.id, answer).unwrap();
    }
    panic!("no empty-stack priority");
}

fn advance_until(game: &mut Game, stop: impl Fn(&Frame) -> bool) {
    for _ in 0..100 {
        if stop(&game.state.rules.frame) {
            return;
        }
        match game.advance(Budget::new(1)) {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(request) if request.reason == Reason::Priority => {
                let answer = mtg_witness::temporary::pass(&request);
                game.respond(request.player, request.id, answer).unwrap();
            }
            other => panic!("unexpected resolution boundary {other:?}"),
        }
    }
    panic!("resolution checkpoint not reached");
}

#[test]
fn verb_operands_beast_within_creates_for_the_last_controller() {
    let prepared = board(&[(
        record!("7735eeba-693b-47e2-bd51-414379cf1016"),
        Cell::TokensForTargetController,
    )])
    .permanent_or_hand("Beast Within", 0, 0, true)
    .permanent_or_hand("Grizzly Bears", 1, 2, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let target = Driver::object(&game, 1);
    Driver::cast(&mut game, CardId(0));
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[2, 3, 4]);
    advance_until(
        &mut game,
        |stage| matches!(stage, Frame::Resolve { frame } if frame.cursor == 1),
    );
    // CR 800.4a: the original beneficiary has left after the removal.
    let mut departure = game.fork();
    departure.concede(PlayerId(2)).unwrap();
    resolve(&mut departure);
    assert!(!departure.is_live(PlayerId(2)));
    assert!(departure.state.rules.tokens.is_empty());
    assert_eq!(
        departure.state.rules.objects[&target.card].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );

    resolve(&mut game);
    assert_eq!(
        game.state.rules.objects[&target.card].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    let mut tokens = game.state.rules.tokens.keys();
    let token = *tokens.next().expect("one created token");
    assert!(tokens.next().is_none());
    assert_eq!(game.state.rules.cards[&token].owner, PlayerId(2));
    assert_eq!(game.state.rules.objects[&token].controller, PlayerId(2));
    let token = game
        .characteristics(Driver::object(&game, token.0))
        .unwrap();
    assert_eq!(token.subtypes, ["Beast"]);
    assert_eq!(
        (token.power, token.toughness),
        (Some(3.into()), Some(3.into()))
    );
}

#[test]
fn verb_operands_swords_gains_life_from_the_exiled_incarnation() {
    let prepared = board(&[(
        record!("b1544f21-7e98-461b-aed5-e748b0168c52"),
        Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::GainLife),
    )])
    .permanent_or_hand("Swords to Plowshares", 0, 0, true)
    .permanent_or_hand("Grizzly Bears", 1, 2, false)
    .permanent_or_hand("Plains", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let target = Driver::object(&game, 1);
    let prior = game.state.rules.players[2].life.clone();
    Driver::cast(&mut game, CardId(0));
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[2]);
    resolve(&mut game);
    assert_eq!(game.state.rules.objects[&target.card].zone, Zone::Exile);
    let mut expected = prior;
    expected.add(&2.into());
    assert_eq!(game.state.rules.players[2].life, expected);
}

#[test]
fn verb_operands_heroic_intervention_captures_permanents_and_expires() {
    let prepared = board(&[(
        record!("24882fa2-3fe9-4c1b-aa3d-0e6488b9db27"),
        Cell::SetKeywordsUntilEndOfTurn,
    )])
    .permanent_or_hand("Heroic Intervention", 0, 0, true)
    .permanent_or_hand("Grizzly Bears", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 1, 1, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[3, 4]);
    resolve(&mut game);
    for keyword in [KeywordAbility::Hexproof, KeywordAbility::Indestructible] {
        assert!(
            game.current_keywords(Driver::object(&game, 1))
                .contains(&keyword)
        );
        assert!(
            game.current_keywords(Driver::object(&game, 3))
                .contains(&keyword)
        );
        assert!(
            !game
                .current_keywords(Driver::object(&game, 2))
                .contains(&keyword)
        );
    }
    for _ in 0..48 {
        let request = Driver::request(&mut game, 1);
        if game.state.rules.effects.is_empty() {
            assert!(
                !game
                    .current_keywords(Driver::object(&game, 1))
                    .contains(&KeywordAbility::Indestructible)
            );
            game.validate_state().unwrap();
            return;
        }
        let answer = mtg_witness::temporary::pass(&request);
        game.respond(request.player, request.id, answer).unwrap();
    }
    panic!("grant did not expire at cleanup");
}

#[test]
fn verb_operands_dramatic_reversal_untaps_only_controlled_nonlands() {
    let prepared = board(&[(
        record!("da46904c-8fb8-44c2-b2ab-775a1cc12ec3"),
        Cell::UntapSet,
    )])
    .permanent_or_hand("Dramatic Reversal", 0, 0, true)
    .permanent_or_hand("Sol Ring", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1, 2]);
    assert!(game.state.rules.objects[&CardId(1)].tapped);
    assert!(game.state.rules.objects[&CardId(2)].tapped);
    resolve(&mut game);
    assert!(!game.state.rules.objects[&CardId(1)].tapped);
    assert!(game.state.rules.objects[&CardId(2)].tapped);
}

#[test]
fn verb_operands_guttersnipe_damages_every_opponent_and_concentrate_draws_three() {
    let prepared = board(&[
        (
            record!("c6bdaf76-6a03-4695-9c4b-f040e73435af"),
            Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Damage),
        ),
        (
            record!("a3d287ec-1a39-4f91-acf1-7c1d2c00ed89"),
            Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Draw),
        ),
    ])
    .permanent_or_hand("Concentrate", 0, 0, true)
    .permanent_or_hand("Guttersnipe", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let life = game
        .state
        .rules
        .players
        .iter()
        .map(|player| player.life.clone())
        .collect::<Vec<_>>();
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[2, 3, 4, 5]);
    advance_until(&mut game, |stage| {
        matches!(stage, Frame::DamageEffects { .. })
    });
    assert_recoverable((&game).into());

    let mut wrong_scope = game.fork();
    let Frame::DamageEffects { transaction } = &mut wrong_scope.state.rules.frame else {
        panic!("prepared set damage");
    };
    let DamageContinuation::Kernel { resume } = transaction.continuation else {
        panic!("kernel damage continuation");
    };
    transaction.entries[0].recipient = DamageRecipient::Player {
        player: PlayerId(0),
    };
    let Frame::Kernel { task } = &mut wrong_scope.state.rules.frames[resume.0] else {
        panic!("saved damage task");
    };
    let mut facts = serde_json::to_value(&**task).unwrap();
    facts["stage"]["recipients"][0] = serde_json::to_value(TargetEntity::Player {
        player: PlayerId(0),
    })
    .unwrap();
    **task = serde_json::from_value(facts).unwrap();
    assert!(
        wrong_scope
            .validate_state()
            .unwrap_err()
            .contains("kernel damage")
    );

    // A previously captured opponent may leave before damage is committed.
    let mut departure = game.fork();
    departure.concede(PlayerId(1)).unwrap();
    resolve(&mut departure);
    assert!(!departure.is_live(PlayerId(1)));
    let mut expected = game.state.rules.players[2].life.clone();
    expected.subtract(&2.into());
    assert_eq!(departure.state.rules.players[2].life, expected);

    resolve(&mut game);
    assert_eq!(game.state.rules.players[0].life, life[0]);
    for (n, prior) in life.into_iter().enumerate().skip(1) {
        let mut expected = prior;
        expected.subtract(&2.into());
        assert_eq!(game.state.rules.players[n].life, expected);
    }
    assert_eq!(game.state.rules.libraries[&PlayerId(0)].len(), 1);
}

#[test]
fn verb_operands_counts_the_current_hand_and_only_the_controllers_graveyard() {
    let prepared = board(&[
        (
            record!("4492c329-174a-44aa-be4d-9f018173b9a7"),
            Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::GainLife),
        ),
        (
            record!("fbd99a88-8926-455e-8e23-58fed6d14719"),
            Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::GainLife),
        ),
    ])
    .permanent_or_hand("Union of the Third Path", 0, 0, true)
    .permanent_or_hand("Blossoming Wreath", 0, 0, true)
    .permanent_or_hand("Plains", 0, 0, false)
    .permanent_or_hand("Plains", 0, 0, false)
    .permanent_or_hand("Plains", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .card(
        "Grizzly Bears",
        0,
        0,
        Zone::Graveyard { owner: PlayerId(0) },
    )
    .card(
        "Grizzly Bears",
        1,
        1,
        Zone::Graveyard { owner: PlayerId(1) },
    )
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let mut life = game.state.rules.players[0].life.clone();
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[2, 3, 4]);
    resolve(&mut game);
    life.add(&2.into());
    assert_eq!(game.state.rules.players[0].life, life);
    Driver::cast(&mut game, CardId(1));
    Driver::pay(&mut game, &[5]);
    resolve(&mut game);
    life.add(&1.into());
    assert_eq!(game.state.rules.players[0].life, life);
}

#[test]
fn verb_operands_each_player_draws_in_apnap_order() {
    let prepared = board(&[(
        record!("6d42dff5-97ec-4768-b112-84a3584d0b87"),
        Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Draw),
    )])
    .permanent_or_hand("Vision Skeins", 0, 0, true)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1, 2]);
    resolve(&mut game);
    let drawn = game
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Committed {
                actual: EventOutcome::Moved { movement },
                ..
            } => match (movement.from, movement.destination) {
                (Zone::Library { .. }, Zone::Hand { owner }) => Some(owner.0),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(drawn, [0, 0, 1, 1, 2, 2, 3, 3]);
}

#[test]
fn verb_operands_swords_treats_negative_power_as_zero() {
    let ox = FixtureCatalog::Match
        .get()
        .definitions
        .iter()
        .find(|definition| definition.name == "Yoked Ox")
        .unwrap();
    let prepared = board_with(
        &[(
            record!("b1544f21-7e98-461b-aed5-e748b0168c52"),
            Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::GainLife),
        )],
        Some(ox),
    )
    .permanent_or_hand("Disfigure", 0, 0, true)
    .permanent_or_hand("Swords to Plowshares", 0, 0, true)
    .permanent_or_hand("Yoked Ox", 1, 2, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Plains", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let target = Driver::object(&game, 2);
    Driver::cast(&mut game, CardId(0));
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[3]);
    resolve(&mut game);
    assert_eq!(
        game.characteristics(target).unwrap().power,
        Some((-2).into())
    );

    let life = game.state.rules.players[2].life.clone();
    Driver::cast(&mut game, CardId(1));
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[4]);
    resolve(&mut game);
    assert_eq!(game.state.rules.objects[&target.card].zone, Zone::Exile);
    assert_eq!(game.state.rules.players[2].life, life);
    assert!(!game.events().iter().any(|event| matches!(
        event,
        Event::Committed {
            proposed: ProposedEvent::GainLife { .. },
            ..
        }
    )));
}

#[test]
fn verb_operands_keyword_grant_does_not_follow_a_bounced_target() {
    let prepared = board(&[(
        record!("6e460d39-0f18-4a2d-9c8d-3eaba301d285"),
        Cell::TargetKeywordsUntilEndOfTurn,
    )])
    .permanent_or_hand("Rush of Vitality", 0, 0, true)
    .permanent_or_hand("Unsummon", 0, 0, true)
    .permanent_or_hand("Grizzly Bears", 1, 1, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let target = Driver::object(&game, 2);
    Driver::cast(&mut game, CardId(0));
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[3, 4]);
    Driver::cast(&mut game, CardId(1));
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[5]);
    resolve(&mut game);
    let returned = &game.state.rules.objects[&target.card];
    assert_ne!(returned.id, target);
    assert_eq!(returned.zone, Zone::Hand { owner: PlayerId(1) });
    assert!(game.state.rules.effects.is_empty());
}

#[test]
fn verb_operands_deluge_recovers_with_opponents_creatures_tapped() {
    let prepared = board(&[(
        record!("cd271e61-0135-484f-ae02-5aaca57c1124"),
        Cell::TapSet,
    )])
    .permanent_or_hand("Deluge", 0, 0, true)
    .permanent_or_hand("Grizzly Bears", 1, 1, false)
    .permanent_or_hand("Storm Crow", 2, 2, false)
    .permanent_or_hand("Grizzly Bears", 3, 3, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[4, 5, 6]);
    advance_until(
        &mut game,
        |stage| matches!(stage, Frame::Resolve { frame } if frame.cursor == 1),
    );
    assert_recoverable((&game).into());
    let mut restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    resolve(&mut restored);
    assert!(restored.state.rules.objects[&CardId(1)].tapped);
    assert!(!restored.state.rules.objects[&CardId(2)].tapped);
    assert!(restored.state.rules.objects[&CardId(3)].tapped);
}

/// CR 603.10a, 608.2h: the dying creature's last battlefield appearance
/// supplies its toughness even when the card has a successor in graveyard.
#[test]
fn verb_operands_proper_burial_reads_the_dying_creature() {
    let prepared = board(&[(
        record!("e5bf77c2-a9b2-4366-8412-e263160403c1"),
        Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::GainLife),
    )])
    .permanent_or_hand("Proper Burial", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Murder", 0, 0, true)
    .permanent_or_hand("Glorious Anthem", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();

    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let creature = Driver::object(&game, 1);
    let initial_life = game.state.rules.players[0].life.clone();
    Driver::cast(&mut game, CardId(5));
    Driver::pick_object(&mut game, creature);
    Driver::pay(&mut game, &[2, 3, 4]);
    resolve(&mut game);
    assert_eq!(
        game.state.rules.objects[&creature.card].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    let mut expected = initial_life;
    expected.add(&3.into());
    assert_eq!(game.state.rules.players[0].life, expected);
    assert_recoverable((&game).into());
}

#[test]
fn verb_operands_damage_and_counters_share_the_death_event_power() {
    use mtg_contracts::kir::EvaluatedVerb;
    let prepared = board(&[
        (
            record!("5f4ff27f-ebc1-4a86-8b0b-eeea470a25fb"),
            Cell::Evaluated(EvaluatedVerb::Damage),
        ),
        (
            record!("6343cf6f-b1e3-49ce-8933-8fe6c9dc83f3"),
            Cell::Evaluated(EvaluatedVerb::PutCounters),
        ),
        (
            record!("993e41e6-0643-4a1f-8ada-b7a4aae486d9"),
            Cell::Evaluated(EvaluatedVerb::Damage),
        ),
    ])
    .seats(4)
    .permanent_or_hand("Stalking Vengeance", 0, 0, false)
    .permanent_or_hand("Kresh the Bloodbraided", 1, 1, false)
    .permanent_or_hand("Jackal Pup", 0, 0, false)
    .permanent_or_hand("Giant Growth", 0, 0, true)
    .permanent_or_hand("Lightning Bolt", 0, 0, true)
    .permanent_or_hand("Murder", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Mountain", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let creature = Driver::object(&game, 2);
    let before = game
        .state
        .rules
        .players
        .iter()
        .map(|row| row.life.clone())
        .collect::<Vec<_>>();
    Driver::cast(&mut game, CardId(3));
    Driver::pick_object(&mut game, creature);
    Driver::pay(&mut game, &[6]);
    resolve(&mut game);

    Driver::cast(&mut game, CardId(4));
    Driver::pick_object(&mut game, creature);
    Driver::pay(&mut game, &[7]);
    resolve(&mut game);
    let mut damaged = before[0].clone();
    damaged.subtract(&3.into());
    assert_eq!(game.state.rules.players[0].life, damaged);
    assert_eq!(
        game.characteristics(creature).unwrap().power,
        Some(5.into())
    );

    Driver::cast(&mut game, CardId(5));
    Driver::pick_object(&mut game, creature);
    Driver::pay(&mut game, &[8, 9, 10]);
    loop {
        let request = Driver::request(&mut game, 1);
        let answer = match &request.reason {
            Reason::Priority => mtg_witness::temporary::pass(&request),
            Reason::Targets { .. } => request.entity_targets(vec![TargetEntity::Player {
                player: PlayerId(2),
            }]),
            Reason::OptionalEffect => {
                assert_eq!(request.player, PlayerId(1));
                assert_recoverable((&game).into());
                let answer = request
                    .pick_one(&Candidate::Flag { flag: Flag::Accept })
                    .unwrap();
                game.respond(request.player, request.id, answer).unwrap();
                break;
            }
            reason => panic!("unexpected decision {reason:?}"),
        };
        game.respond(request.player, request.id, answer).unwrap();
    }
    resolve(&mut game);
    assert!(game.current(creature).is_err());
    assert_eq!(
        game.current(Driver::object(&game, 1))
            .unwrap()
            .plus_one_counters,
        5.into()
    );
    let mut hit = before[2].clone();
    hit.subtract(&5.into());
    assert_eq!(game.state.rules.players[2].life, hit);
    assert_eq!(game.state.rules.players[3].life, before[3]);
}

#[test]
fn verb_operands_damage_rechecks_hexproof_at_resolution() {
    let prepared = board(&[(
        record!("24882fa2-3fe9-4c1b-aa3d-0e6488b9db27"),
        Cell::SetKeywordsUntilEndOfTurn,
    )])
    .seats(4)
    .permanent_or_hand("Lightning Bolt", 0, 0, true)
    .permanent_or_hand("Heroic Intervention", 1, 1, true)
    .permanent_or_hand("Grizzly Bears", 1, 1, false)
    .permanent_or_hand("Mountain", 0, 0, false)
    .permanent_or_hand("Forest", 1, 1, false)
    .permanent_or_hand("Forest", 1, 1, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let target = Driver::object(&game, 2);
    Driver::cast(&mut game, CardId(0));
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[3]);
    Driver::pass(&mut game);
    Driver::cast(&mut game, CardId(1));
    Driver::pay(&mut game, &[4, 5]);
    resolve(&mut game);
    let remaining = game.current(target).unwrap();
    assert_eq!(remaining.zone, Zone::Battlefield);
    assert_eq!(remaining.marked_damage, Quantity::zero());
    assert!(
        game.current_keywords(target)
            .contains(&KeywordAbility::Hexproof)
    );
    assert!(game.trace().events.iter().any(|event| matches!(
        event,
        Event::SpellCompleted {
            resolved: false,
            ..
        }
    )));
}

#[test]
fn verb_operands_damage_and_life_share_the_paid_sacrifice_object() {
    use mtg_contracts::kir::EvaluatedVerb;
    let prepared = board(&[
        (
            record!("84d47913-53ac-4d4d-bac3-bc0306d43e12"),
            Cell::Evaluated(EvaluatedVerb::Damage),
        ),
        (
            record!("2d92a035-dd7a-4426-a8c0-f04e0b836dad"),
            Cell::Evaluated(EvaluatedVerb::GainLife),
        ),
    ])
    .seats(4)
    .permanent_or_hand("Bloodshot Cyclops", 0, 0, false)
    .permanent_or_hand("Disciple of Griselbrand", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 0, 0, false)
    .permanent_or_hand("Giant Growth", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .libraries("Forest", 4)
    .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let [pumped, other] = [2, 3].map(|card| Driver::object(&game, card));
    let initial = [0, 2].map(|seat| game.state.rules.players[seat].life.clone());
    Driver::cast(&mut game, CardId(4));
    Driver::pick_object(&mut game, pumped);
    Driver::pay(&mut game, &[5]);
    resolve(&mut game);

    let cyclops = Driver::object(&game, 0);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: cyclops,
            ability: 0,
        },
    );
    let targets = Driver::request(&mut game, 1);
    let answer = targets.entity_targets(vec![TargetEntity::Player {
        player: PlayerId(2),
    }]);
    game.respond(targets.player, targets.id, answer).unwrap();
    Driver::pick_object(&mut game, pumped);
    assert_recoverable((&game).into());
    resolve(&mut game);

    let disciple = Driver::object(&game, 1);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: disciple,
            ability: 0,
        },
    );
    Driver::pay(&mut game, &[6]);
    Driver::pick_object(&mut game, other);
    assert_recoverable((&game).into());
    resolve(&mut game);

    let [mut life, mut hit] = initial;
    life.add(&2.into());
    hit.subtract(&5.into());
    assert_eq!(game.state.rules.players[0].life, life);
    assert_eq!(game.state.rules.players[2].life, hit);
    assert!(game.current(pumped).is_err());
    assert!(game.current(other).is_err());
}
