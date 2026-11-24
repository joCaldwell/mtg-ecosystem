//! Frozen R4 witnesses. CR 107.1b, 111.2, 120.2, 121.2, 608.2h,
//! 611.2c, 613.1f, 613.7b, 701.26b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, Tiers, definition_cells, definition_tiers};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r4/",
            $id,
            ".json"
        ))
    };
}

fn board(cards: &[(&str, Cell)]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(raw, cell) in cards {
        let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
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
        Cell::LifeTargetControllerPower,
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
            Cell::DamageSelectedPlayers,
        ),
        (
            record!("a3d287ec-1a39-4f91-acf1-7c1d2c00ed89"),
            Cell::DrawControllerCounted,
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
            Cell::LifeCounted,
        ),
        (
            record!("fbd99a88-8926-455e-8e23-58fed6d14719"),
            Cell::LifeCounted,
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
        Cell::DrawEachPlayer,
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
