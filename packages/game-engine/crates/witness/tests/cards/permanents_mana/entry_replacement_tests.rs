//! CR 614.1c/614.12, 800.4a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Tiers, definition_cells, definition_tiers};

fn catalog() -> Catalog {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in [
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/027dd013-baa7-4111-b3c9-f4d1414e9c45.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/672e190d-8ea0-4a2e-b74f-5d35304631e4.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/9e7a240d-dc33-47ac-9f17-77fab4c1c340.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/f0ec8681-da50-466b-8cdd-1dc710deccd9.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/139fb542-89fd-4b9e-87e6-ec925bcca93a.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/f1750962-a87c-49f6-b731-02ae971ac6ea.json"
        ),
    ] {
        let compiled = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
            panic!("entry replacement source did not lower: {compiled:?}");
        };
        definition.validate().unwrap();
        definition_cells(&definition).unwrap();
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog
}

#[test]
fn conditional_entry_counts_only_existing_permanents_and_live_opponents() {
    for (name, existing, seats, tapped) in [
        ("Glacial Fortress", &[][..], 2, true),
        ("Glacial Fortress", &["Plains"][..], 2, false),
        ("Seachrome Coast", &["Forest", "Forest"][..], 2, false),
        (
            "Seachrome Coast",
            &["Forest", "Forest", "Forest"][..],
            2,
            true,
        ),
        ("Deserted Beach", &["Forest"][..], 2, true),
        ("Deserted Beach", &["Forest", "Forest"][..], 2, false),
        ("Sea of Clouds", &[][..], 2, true),
        ("Sea of Clouds", &[][..], 3, false),
    ] {
        let mut board = Board::new(catalog(), "entry-replacement-witness")
            .step(TurnStep::PrecombatMain)
            .permanent_or_hand(name, 0, 0, true);
        for land in existing {
            board = board.permanent_or_hand(land, 0, 0, false);
        }
        let mut game = Game::new(board.libraries("Forest", 4).prepare());
        for seat in (seats..4).rev() {
            game.concede(PlayerId(seat)).unwrap();
            let _ = Driver::request(&mut game, u64::MAX);
        }
        assert_eq!(game.live_players().len(), usize::from(seats));
        let object = Driver::object(&game, 0);
        Driver::action(&mut game, Action::PlayLand { object });
        let _ = Driver::request(&mut game, u64::MAX);
        let successor = Driver::object(&game, 0);
        assert_ne!(successor, object);
        assert_eq!(game.current(successor).unwrap().tapped, tapped, "{name}");
        game.validate_state().unwrap();
        let snapshot = game.snapshot().unwrap();
        let restored = Game::restore(game.prepared.as_ref().clone(), &snapshot).unwrap();
        assert_eq!(restored.snapshot().unwrap(), snapshot);
        assert_recoverable(JournalCase::from(&game));
    }
}

#[test]
fn moves_from_other_zones_use_the_same_pre_entry_board() {
    for zone in [
        Zone::Library { owner: PlayerId(0) },
        Zone::Graveyard { owner: PlayerId(0) },
    ] {
        let board = Board::new(catalog(), "entry-replacement-move-witness")
            .step(TurnStep::PrecombatMain)
            .card("Seachrome Coast", 0, 0, zone)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false);
        let mut game = Game::new(board.libraries("Forest", 4).prepare());
        let object = Driver::object(&game, 0);
        let moved = game.move_object(object, Zone::Battlefield).unwrap();
        assert!(!game.current(moved.successor).unwrap().tapped);
        game.validate_state().unwrap();
    }
}

#[test]
fn counters_are_present_on_the_new_incarnation() {
    let board = Board::new(catalog(), "entry-counters-witness")
        .step(TurnStep::PrecombatMain)
        .card(
            "Faithful Watchdog",
            0,
            0,
            Zone::Graveyard { owner: PlayerId(0) },
        );
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    let object = Driver::object(&game, 0);
    let moved = game.move_object(object, Zone::Battlefield).unwrap();
    assert_eq!(
        game.current(moved.successor).unwrap().plus_one_counters,
        3.into()
    );
    game.validate_state().unwrap();
}

#[test]
fn faithful_watchdog_resolves_and_survives_as_a_three_three() {
    let board = Board::new(catalog(), "entry-counters-cast-witness")
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand("Faithful Watchdog", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false);
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1, 2]);
    assert_eq!(
        game.current(Driver::object(&game, 0)).unwrap().zone,
        Zone::Stack
    );
    for _ in 0..4 {
        Driver::pass(&mut game);
    }
    let _ = Driver::request(&mut game, u64::MAX);
    let object = Driver::object(&game, 0);
    let watchdog = game.current(object).unwrap();
    assert_eq!(watchdog.zone, Zone::Battlefield);
    assert_eq!(watchdog.plus_one_counters, 3.into());
    let stats = game.characteristics(object).unwrap();
    assert_eq!(
        (stats.power, stats.toughness),
        (Some(3.into()), Some(3.into()))
    );
    game.validate_state().unwrap();
    assert_recoverable(JournalCase::from(&game));
}

#[test]
fn shockland_offer_is_recoverable_and_unpayable_entry_skips_the_offer() {
    for (life, answer, tapped, final_life) in [
        (20, Some(Flag::Accept), false, 18),
        (20, Some(Flag::Decline), true, 20),
        (1, None, true, 1),
    ] {
        let board = Board::new(catalog(), "entry-life-witness")
            .step(TurnStep::PrecombatMain)
            .life(PlayerId(0), life.into())
            .permanent_or_hand("Hallowed Fountain", 0, 0, true);
        let mut game = Game::new(board.libraries("Forest", 4).prepare());
        let object = Driver::object(&game, 0);
        Driver::action(&mut game, Action::PlayLand { object });
        let decision = Driver::request(&mut game, u64::MAX);
        if let Some(answer) = answer {
            assert_eq!(decision.reason, Reason::OptionalEffect);
            assert_eq!(
                game.current(object).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
            game.validate_state().unwrap();
            let snapshot = game.snapshot().unwrap();
            let restored = Game::restore(game.prepared.as_ref().clone(), &snapshot).unwrap();
            assert_eq!(restored.snapshot().unwrap(), snapshot);
            assert_recoverable(JournalCase::from(&game));
            game.concede(PlayerId(3)).unwrap();
            let decision = Driver::request(&mut game, u64::MAX);
            assert_eq!(decision.reason, Reason::OptionalEffect);
            let picked = decision
                .pick_one(&Candidate::Flag { flag: answer })
                .unwrap();
            game.respond(decision.player, decision.id, picked).unwrap();
            let _ = Driver::request(&mut game, u64::MAX);
        } else {
            assert_ne!(decision.reason, Reason::OptionalEffect);
        }
        let successor = Driver::object(&game, 0);
        assert_ne!(successor, object);
        assert_eq!(game.current(successor).unwrap().tapped, tapped);
        assert_eq!(game.state.rules.players[0].life, final_life.into());
        game.validate_state().unwrap();
        assert_recoverable(JournalCase::from(&game));
    }
}

#[test]
fn departing_entrant_controller_cancels_a_suspended_land_play() {
    let board = Board::new(catalog(), "entry-life-departure-witness")
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand("Hallowed Fountain", 0, 0, true);
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    let object = Driver::object(&game, 0);
    Driver::action(&mut game, Action::PlayLand { object });
    assert_eq!(
        Driver::request(&mut game, u64::MAX).reason,
        Reason::OptionalEffect
    );
    game.concede(PlayerId(0)).unwrap();
    let _ = Driver::request(&mut game, u64::MAX);
    assert!(game.current(object).is_err());
    game.validate_state().unwrap();
    assert_recoverable(JournalCase::from(&game));
}
