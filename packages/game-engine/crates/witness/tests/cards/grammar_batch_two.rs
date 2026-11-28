//! Whole-card witnesses for the additive grammar cut (CR 107.3m, 120.2a,
//! 202.3, 400.7, 601.2i, 602.2b, 603.4, 608.2c, 608.2h, 614.12).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_compiler::CompileResult;
use mtg_contracts::kir::definition_tiers;
use mtg_core::testkit::*;

const HEADER: &str = include_str!(
    "../../../../tests/fixtures/archive-manifest-r6-candidate-1732e894984e0574d8ae100411c9f01fc22aca12baaa46cd64bc43d7fab7b4af.json"
);
macro_rules! frozen {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r6/1732e894984e0574d8ae100411c9f01fc22aca12baaa46cd64bc43d7fab7b4af/",
            $id,
            ".json"
        ))
    };
}
const HYDRA: &str = frozen!("b14d05c0-fe10-4079-a90e-0aea1a8fd375");
const DRUDGE: &str = frozen!("1c6a0d19-c7dd-4353-95ad-dfdef2d5d2d9");
const HELLKITE: &str = frozen!("6e64f1e0-ddd6-4ee7-8797-086e234b02e8");
const PADEEM: &str = frozen!("0c7ba712-6a99-4d2f-9242-a2163a11f69c");
const UNEARTH: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/4e0b647a-6e1a-421b-bca9-8d5e346bda80.json");

fn board(card: &str, extra: Option<&str>, step: TurnStep) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for (manifest, raw) in [(HEADER, card)]
        .into_iter()
        .chain(extra.map(|raw| (mtg_witness::R5_MANIFEST, raw)))
    {
        let CompileResult::Lowered { definition } = mtg_compiler::compile(manifest, raw) else {
            panic!(
                "whole-card witness did not lower: {:?}",
                mtg_compiler::compile(manifest, raw)
            );
        };
        definition_tiers(&definition).unwrap();
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "grammar-batch-two").step(step)
}

fn priority(game: &mut Game) -> Decision<Rules> {
    let request = Driver::request(game, 1);
    assert_eq!(request.reason, Reason::Priority);
    request
}

fn resolve(game: &mut Game) {
    for _ in 0..32 {
        priority(game);
        if game.state.rules.stack.is_empty() {
            return;
        }
        Driver::pass(game);
    }
    panic!("stack did not resolve");
}

fn choose_x(game: &mut Game, x: i64) {
    assert_eq!(Driver::request(game, 1).reason, Reason::AnnounceX);
    Driver::answer(game, 1, Answer::Number { value: x.into() });
}

#[test]
fn grammar_batch_two_hydra_enters_with_announced_x_and_uses_last_known_power() {
    let mut fixture = board(HYDRA, None, TurnStep::PrecombatMain)
        .permanent_or_hand("Lifeblood Hydra", 0, 0, true)
        .permanent_or_hand("Murder", 0, 0, true);
    for _ in 0..6 {
        fixture = fixture.permanent_or_hand("Forest", 0, 0, false);
    }
    for _ in 0..3 {
        fixture = fixture.permanent_or_hand("Swamp", 0, 0, false);
    }
    let mut game = Game::new(fixture.libraries("Island", 5).prepare());
    Driver::cast(&mut game, CardId(0));
    choose_x(&mut game, 3);
    Driver::pay(&mut game, &[2, 3, 4, 5, 6, 7]);
    resolve(&mut game);
    let hydra = Driver::object(&game, 0);
    assert_eq!(game.current(hydra).unwrap().plus_one_counters, 3.into());
    assert_eq!(game.characteristics(hydra).unwrap().power, Some(3.into()));
    Driver::cast(&mut game, CardId(1));
    Driver::pick_object(&mut game, hydra);
    Driver::pay(&mut game, &[8, 9, 10]);
    resolve(&mut game);
    assert_eq!(
        game.zone(Driver::object(&game, 0)),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    assert_eq!(game.state.rules.players[0].life, 43.into());
    assert_eq!(game.state.rules.cast_history[0].draws, 3);
    assert_recoverable((&game).into());
}

#[test]
fn grammar_batch_two_hydra_that_was_not_a_resolving_spell_uses_zero() {
    let mut game = Game::new(
        board(HYDRA, Some(UNEARTH), TurnStep::PrecombatMain)
            .card(
                "Lifeblood Hydra",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            )
            .permanent_or_hand("Unearth", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .libraries("Island", 3)
            .prepare(),
    );
    let old = Driver::object(&game, 0);
    Driver::cast(&mut game, CardId(1));
    Driver::pick_object(&mut game, old);
    Driver::pay(&mut game, &[2]);
    resolve(&mut game);
    let new = Driver::object(&game, 0);
    assert_eq!(new.incarnation, old.incarnation + 2);
    assert_eq!(game.zone(new), Some(Zone::Graveyard { owner: PlayerId(0) }));
    assert_eq!(game.state.rules.players[0].life, 40.into());
    assert_eq!(game.state.rules.cast_history[0].draws, 0);
    assert_recoverable((&game).into());
}

#[test]
fn grammar_batch_two_drudge_reads_both_graveyard_actions_and_ignores_hand_casts() {
    let cases = [
        (
            "Think Twice",
            "fa85c5a2-8e83-4624-a35a-a0bbf17ecbb4",
            true,
            false,
            2,
        ),
        (
            "Think Twice",
            "fa85c5a2-8e83-4624-a35a-a0bbf17ecbb4",
            false,
            false,
            1,
        ),
        (
            "Dregscape Zombie",
            "be9d1346-4416-4ade-ae84-7a4121e0bd12",
            true,
            true,
            1,
        ),
    ];
    for (name, id, from_graveyard, activating, expected_draws) in cases {
        let raw = mtg_compiler::fixtures::FILES
            .iter()
            .find(|(path, _)| *path == format!("phase1-r5/{id}.json"))
            .unwrap()
            .1;
        let zone = if from_graveyard {
            Zone::Graveyard { owner: PlayerId(0) }
        } else {
            Zone::Hand { owner: PlayerId(0) }
        };
        let mut game = Game::new(
            board(DRUDGE, Some(raw), TurnStep::PrecombatMain)
                .permanent_or_hand("Laboratory Drudge", 0, 0, false)
                .card(name, 0, 0, zone)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .libraries("Island", 5)
                .prepare(),
        );
        if activating {
            let object = Driver::object(&game, 1);
            Driver::action(&mut game, Action::ActivateAbility { object, ability: 0 });
            Driver::pay(&mut game, &[2]);
        } else {
            Driver::cast(&mut game, CardId(1));
            Driver::pay(&mut game, if from_graveyard { &[2, 3, 4] } else { &[2, 3] });
        }
        resolve(&mut game);
        for _ in 0..100 {
            let request = Driver::request(&mut game, 1);
            if game.turn().step == TurnStep::End && request.reason == Reason::Priority {
                break;
            }
            match request.reason {
                Reason::Priority => Driver::pass(&mut game),
                Reason::Attackers => Driver::answer(&mut game, 1, request.attack(vec![])),
                Reason::OrderTriggers => Driver::answer(
                    &mut game,
                    1,
                    Answer::Order {
                        order: (0..u32::try_from(request.candidates.len()).unwrap()).collect(),
                    },
                ),
                _ => panic!("unexpected turn decision {request:?}"),
            }
        }
        assert_eq!(game.turn().step, TurnStep::End);
        resolve(&mut game);
        assert_eq!(game.state.rules.cast_history[0].draws, expected_draws);
        assert_recoverable((&game).into());
    }
}

#[test]
fn grammar_batch_two_hellkite_uses_actual_combat_damage_and_exact_mana_value() {
    let mut game = Game::new(
        board(HELLKITE, None, TurnStep::PrecombatMain)
            .permanent_or_hand("Steel Hellkite", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 2, 1, false)
            .permanent_or_hand("Grizzly Bears", 2, 2, false)
            .permanent_or_hand("Sol Ring", 1, 1, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .libraries("Island", 3)
            .prepare(),
    );
    let object = Driver::object(&game, 0);
    Driver::action(&mut game, Action::ActivateAbility { object, ability: 0 });
    Driver::pay(&mut game, &[4, 5]);
    resolve(&mut game);
    for _ in 0..100 {
        let request = Driver::request(&mut game, 1);
        if game.turn().step == TurnStep::PostcombatMain && request.reason == Reason::Priority {
            break;
        }
        match request.reason {
            Reason::Priority => Driver::pass(&mut game),
            Reason::Attackers => {
                Driver::answer(&mut game, 1, request.attack(vec![(object, PlayerId(1))]))
            }
            Reason::Blockers => Driver::answer(&mut game, 1, request.block(vec![])),
            _ => panic!("unexpected combat decision {request:?}"),
        }
    }
    assert_eq!(game.turn().step, TurnStep::PostcombatMain);
    assert_eq!(game.state.rules.players[1].life, 34.into());
    let wipe = Action::ActivateAbility { object, ability: 1 };
    Driver::action(&mut game, wipe.clone());
    choose_x(&mut game, 2);
    Driver::pay(&mut game, &[6, 7]);
    resolve(&mut game);
    assert_eq!(
        game.zone(Driver::object(&game, 1)),
        Some(Zone::Graveyard { owner: PlayerId(2) })
    );
    for id in [2, 3] {
        assert_eq!(
            game.zone(Driver::object(&game, id)),
            Some(Zone::Battlefield)
        );
    }
    assert!(
        priority(&mut game)
            .index(&Candidate::Action { action: wipe })
            .is_none()
    );
    assert_recoverable((&game).into());
}

#[test]
fn grammar_batch_two_padeem_checks_tied_mana_value_twice() {
    for remove_in_response in [false, true] {
        let mut game = Game::new(
            board(PADEEM, None, TurnStep::Untap)
                .permanent_or_hand("Padeem, Consul of Innovation", 0, 0, false)
                .permanent_or_hand("Sol Ring", 0, 0, false)
                .permanent_or_hand("Sol Ring", 1, 1, false)
                .permanent_or_hand("Shatter", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false)
                .libraries("Island", 4)
                .prepare(),
        );
        for _ in 0..=game.live_players().len() {
            priority(&mut game);
            if game.turn().step == TurnStep::Upkeep {
                break;
            }
            assert_eq!(game.turn().step, TurnStep::Untap);
            Driver::pass(&mut game);
        }
        assert_eq!(game.turn().step, TurnStep::Upkeep);
        assert_eq!(game.state.rules.stack.len(), 1, "a tied artifact qualifies");
        for (id, grants_hexproof) in [(1, true), (2, false)] {
            let ring = Driver::object(&game, id);
            assert_eq!(
                game.object_before(ring)
                    .unwrap()
                    .keywords
                    .contains(&KeywordAbility::Hexproof),
                grants_hexproof
            );
        }
        if remove_in_response {
            let ring = Driver::object(&game, 1);
            Driver::cast(&mut game, CardId(3));
            Driver::pick_object(&mut game, ring);
            Driver::pay(&mut game, &[1, 4]);
        }
        resolve(&mut game);
        assert_eq!(
            game.state.rules.cast_history[0].draws,
            u32::from(!remove_in_response)
        );
        assert_recoverable((&game).into());
    }
}

#[test]
fn grammar_batch_two_announced_x_neighbours_lower_as_whole_cards() {
    use mtg_contracts::kir::StaticEffect;
    for raw in [
        frozen!("30b5b764-478a-46fa-8f95-a1f515925a2d"),
        frozen!("32654ffa-b869-4f8b-a153-4242e8f9d92e"),
        frozen!("38517711-e570-4337-b269-addcf8bfdd74"),
        frozen!("3eb49097-42f8-4388-907d-32a738749d8a"),
        frozen!("6970f285-dc2b-437b-b099-ca1d3ef106ab"),
        frozen!("7a51780a-fa28-4ee0-94c7-4330800ca9cb"),
        frozen!("7c852dfe-8238-461e-814e-9667807f2cf5"),
        frozen!("82f3faa8-39fa-450b-843f-d60a4c36d8f7"),
        frozen!("b7daa74c-6142-4107-9355-be98af6ccf13"),
        frozen!("bb2cc0f4-6abd-48d6-b442-76eafbfd2309"),
        frozen!("cca03b76-dd3c-4228-b97a-f7fa20819b2e"),
        frozen!("ce02ef24-d53e-4025-bc45-ca27aba89fe9"),
        frozen!("d1dde190-8124-4590-9507-801ea4c8cde3"),
        frozen!("e38e3723-05f5-4a51-8364-1cda19f9cc49"),
        frozen!("fa8e5291-1a50-4c05-820c-8ae756364d33"),
    ] {
        let result = mtg_compiler::compile(HEADER, raw);
        let CompileResult::Lowered { definition } = result else {
            panic!("frozen announced-X witness did not lower: {result:?}");
        };
        definition_tiers(&definition).unwrap();
        assert!(definition.static_abilities.iter().any(|ability| {
            matches!(
                ability.effect,
                StaticEffect::EntersWithAnnouncedXCounters { .. }
            )
        }));
    }
}
