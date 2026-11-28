//! Frozen-source checks for color predicates, player antecedents, and result
//! and counter amounts (CR 105.2, 121.2, 122.1, 400.7, 608.2b,c,h, 701.17c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const R6: &str = include_str!(
    "../../../../../tests/fixtures/archive-manifest-r6-candidate-6a4226a5e7c5ed8d0044770ac4ededc8c2e5586217b80e53b454dd4a9f952b37.json"
);

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

fn scenario(cards: &[(&str, &str)]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(r5, r6) in cards {
        for (manifest, raw) in [(mtg_witness::R5_MANIFEST, r5), (R6, r6)] {
            let result = mtg_witness::compile(manifest, raw);
            let mtg_compiler::CompileResult::Lowered { definition } = result else {
                panic!("frozen operand witness failed: {result:?}");
            };
            definition_tiers(&definition).unwrap();
            if manifest == R6 {
                catalog.definitions.push(*definition);
            }
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "lane1-operands-b2").step(TurnStep::PrecombatMain)
}

fn pick_player(game: &mut Game, player: PlayerId) {
    let request = Driver::request(game, 1);
    game.respond(
        request.player,
        request.id,
        request.player_targets(vec![player]),
    )
    .unwrap();
}

fn choice(game: &mut Game, wanted: Reason<Rules>) -> Decision<Rules> {
    for _ in 0..24 {
        let request = Driver::request(game, 1);
        if request.reason == wanted {
            assert_recoverable((&*game).into());
            return request;
        }
        assert_eq!(request.reason, Reason::Priority);
        Driver::pass(game);
    }
    panic!("no {wanted:?} decision");
}

#[test]
fn counters_follow_the_target_incarnation_through_destroy_and_exile() {
    let battlegrowth = frozen!("650bf82e-7f83-470c-bf4a-34281bfe9341");
    for (card, name, destination, life) in [
        (
            frozen!("57fd34d3-3e30-4467-bace-3f4dc2a85c80"),
            "Parting Thoughts",
            Zone::Graveyard { owner: PlayerId(0) },
            39,
        ),
        (
            frozen!("b233a9e4-0b4c-41e8-a5b8-77824241966b"),
            "Flay Essence",
            Zone::Exile,
            41,
        ),
    ] {
        let prepared = scenario(&[battlegrowth, card])
            .permanent_or_hand("Battlegrowth", 0, 0, true)
            .permanent_or_hand(name, 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .library_card("Island", 0)
            .prepare();
        let mut game = Game::new(prepared);
        let bear = Driver::object(&game, 2);
        Driver::cast(&mut game, CardId(0));
        Driver::pick_object(&mut game, bear);
        Driver::pay(&mut game, &[3]);
        resolve(&mut game);
        Driver::cast(&mut game, CardId(1));
        Driver::pick_object(&mut game, bear);
        Driver::pay(&mut game, &[4, 5, 6]);
        resolve(&mut game);
        assert_eq!(game.zone(Driver::object(&game, 2)), Some(destination));
        assert_eq!(game.state.rules.players[0].life, life.into());
        let drawn_card = if life == 39 {
            Zone::Hand { owner: PlayerId(0) }
        } else {
            Zone::Library { owner: PlayerId(0) }
        };
        assert_eq!(game.zone(Driver::object(&game, 7)), Some(drawn_card));
    }
}

#[test]
fn color_cardinality_is_shared_by_move_and_sacrifice() {
    for (card, name, lands, destination) in [
        (
            frozen!("0012bc78-e69d-4a67-a302-e5fe0dfd4407"),
            "Ravnica at War",
            vec![4, 5, 6, 7],
            Zone::Exile,
        ),
        (
            frozen!("c854f9e8-6a1c-4515-9912-3847978bf5d5"),
            "Renounce the Guilds",
            vec![4, 5],
            Zone::Graveyard { owner: PlayerId(0) },
        ),
    ] {
        let mut game = Game::new(
            scenario(&[card])
                .permanent_or_hand(name, 0, 0, true)
                .permanent_or_hand("Yargle and Multani", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Darksteel Myr", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .prepare(),
        );
        Driver::cast(&mut game, CardId(0));
        Driver::pay(&mut game, &lands);
        if destination != Zone::Exile {
            let request = choice(&mut game, Reason::ForcedSacrifice { previous: vec![] });
            let multicolored = Driver::object(&game, 1);
            assert_eq!(request.objects_offered(), [multicolored]);
            Driver::answer(&mut game, 1, request.pick_objects(vec![multicolored]));
        }
        resolve(&mut game);
        for (id, zone) in [
            (1, destination),
            (2, Zone::Battlefield),
            (3, Zone::Battlefield),
        ] {
            assert_eq!(game.zone(Driver::object(&game, id)), Some(zone));
        }
    }
}

#[test]
fn subject_relative_counts_feed_life_loss_and_damage() {
    for (card, name, loss) in [
        (
            frozen!("1c7c1867-20cb-4035-bb2a-42760edf0a55"),
            "Villainous Wrath",
            3,
        ),
        (
            frozen!("be6cb3e6-7492-4e30-83a9-9bc413e50cdc"),
            "Chaotic Backlash",
            4,
        ),
    ] {
        let mut game = Game::new(
            scenario(&[card])
                .permanent_or_hand(name, 0, 0, true)
                .permanent_or_hand("Soul's Attendant", 1, 1, false)
                .permanent_or_hand("Storm Crow", 1, 1, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Storm Crow", 0, 0, false)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .prepare(),
        );
        Driver::cast(&mut game, CardId(0));
        pick_player(&mut game, PlayerId(1));
        Driver::pay(&mut game, &[5, 6, 7, 8, 9]);
        resolve(&mut game);
        assert_eq!(game.state.rules.players[0].life, 40.into());
        assert_eq!(game.state.rules.players[1].life, (40 - loss).into());
    }
}

#[test]
fn a_mill_destination_read_counts_only_matching_moved_cards() {
    let mut game = Game::new(
        scenario(&[frozen!("49b84c50-ca0a-4947-8225-ae41aa08e1f1")])
            .permanent_or_hand("Coerced Confession", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .library_card("Grizzly Bears", 1)
            .library_card("Forest", 1)
            .library_card("Storm Crow", 1)
            .library_card("Forest", 0)
            .library_card("Forest", 0)
            .library_card("Island", 0)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    let request = Driver::request(&mut game, 1);
    assert_eq!(request.reason, Reason::HybridCost);
    Driver::answer(
        &mut game,
        1,
        request
            .pick_one(&Candidate::HybridPayment {
                payment: HybridPayment::Mana(Color::U),
            })
            .unwrap(),
    );
    pick_player(&mut game, PlayerId(1));
    Driver::pay(&mut game, &[1, 2, 3, 4, 5]);
    resolve(&mut game);
    for id in 6..9 {
        assert_eq!(
            game.zone(Driver::object(&game, id)),
            Some(Zone::Graveyard { owner: PlayerId(1) })
        );
    }
    for id in 9..11 {
        assert_eq!(
            game.zone(Driver::object(&game, id)),
            Some(Zone::Hand { owner: PlayerId(0) })
        );
    }
    assert_eq!(
        game.zone(Driver::object(&game, 11)),
        Some(Zone::Library { owner: PlayerId(0) })
    );
}

#[test]
fn a_mill_destination_read_also_feeds_life_loss() {
    let mut game = Game::new(
        scenario(&[frozen!("c724dd2e-c62d-4a20-a1b9-54264d0b811a")])
            .permanent_or_hand("Liliana's Indignation", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .library_card("Grizzly Bears", 0)
            .library_card("Island", 0)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    let request = Driver::request(&mut game, 1);
    assert_eq!(request.reason, Reason::AnnounceX);
    Driver::answer(&mut game, 1, Answer::Number { value: 2.into() });
    pick_player(&mut game, PlayerId(1));
    Driver::pay(&mut game, &[1, 2, 3]);
    resolve(&mut game);
    assert_eq!(game.state.rules.players[1].life, 38.into());
}

#[test]
fn draw_quantity_uses_completed_draws_and_keeps_choices_private() {
    let mut game = Game::new(
        scenario(&[frozen!("46db3dc2-9eb1-4be3-96f9-0013119fcd97")])
            .permanent_or_hand("Ancient Excavation", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, true)
            .permanent_or_hand("Darksteel Myr", 0, 0, true)
            .library_card("Island", 0)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1, 2, 3, 4]);
    let request = choice(&mut game, Reason::ResolveDiscard);
    assert_eq!(request.player, PlayerId(0));
    assert_eq!(request.required_count(), 1);
    let drawn = Driver::object(&game, 7);
    assert!(request.objects_offered().contains(&drawn));
    let QueryResult::Complete(QueryValue::Observation(other_seat)) =
        game.observe(PlayerId(1), Budget::unlimited())
    else {
        panic!("expected permitted observation");
    };
    assert!(other_seat.decision.is_none());
    assert!(
        !other_seat
            .view
            .objects
            .iter()
            .any(|object| object.zone == (Zone::Hand { owner: PlayerId(0) }))
    );
    Driver::answer(&mut game, 1, request.pick_objects(vec![drawn]));
    resolve(&mut game);
}

#[test]
fn draw_quantity_also_feeds_life_loss_in_the_selected_mode() {
    let mut game = Game::new(
        scenario(&[frozen!("57ea9f63-8267-4303-a3dd-9ded220294dc")])
            .permanent_or_hand("Rise of the Deathbringer", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .libraries("Forest", 4)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    let request = Driver::request(&mut game, 1);
    assert_eq!(request.reason, Reason::ChooseModes);
    let mode = request
        .candidates
        .iter()
        .find(|candidate| matches!(candidate, Candidate::Modes { mask: 1, .. }))
        .unwrap();
    Driver::answer(&mut game, 1, request.pick_one(mode).unwrap());
    Driver::pay(&mut game, &[2, 3, 4, 5, 6]);
    resolve(&mut game);
    assert_eq!(game.state.rules.players[0].life, 38.into());
    assert_eq!(game.zone(Driver::object(&game, 1)), Some(Zone::Battlefield));
}

#[test]
fn filtered_public_graveyard_sets_can_move_or_supply_an_actual_count() {
    let own = Zone::Graveyard { owner: PlayerId(0) };
    let opponent = Zone::Graveyard { owner: PlayerId(1) };
    for (card, name, exile) in [
        (
            frozen!("a69a24d0-ca58-4a44-8af1-a3bd1608d2f9"),
            "Raise the Past",
            false,
        ),
        (
            frozen!("ce00e0d0-c6f4-4fe8-97c3-7279b06d8fdb"),
            "Crypt Incursion",
            true,
        ),
    ] {
        let mut game = Game::new(
            scenario(&[card])
                .permanent_or_hand(name, 0, 0, true)
                .card("Grizzly Bears", 0, 0, own)
                .card("Storm Crow", 0, 0, own)
                .card("Darksteel Myr", 0, 0, own)
                .card("Llanowar Elves", 1, 1, opponent)
                .card("Forest", 0, 0, own)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .prepare(),
        );
        Driver::cast(&mut game, CardId(0));
        if exile {
            pick_player(&mut game, PlayerId(0));
            Driver::pay(&mut game, &[6, 7, 8]);
        } else {
            Driver::pay(&mut game, &[6, 7, 8, 9]);
        }
        resolve(&mut game);
        let destination = if exile {
            Zone::Exile
        } else {
            Zone::Battlefield
        };
        for id in 1..=2 {
            assert_eq!(game.zone(Driver::object(&game, id)), Some(destination));
        }
        assert_eq!(
            game.zone(Driver::object(&game, 3)),
            Some(if exile { Zone::Exile } else { own })
        );
        assert_eq!(game.zone(Driver::object(&game, 4)), Some(opponent));
        assert_eq!(game.zone(Driver::object(&game, 5)), Some(own));
        assert_eq!(
            game.state.rules.players[0].life,
            (if exile { 49 } else { 40 }).into()
        );
    }
}

#[test]
fn that_player_discards_after_their_damage() {
    let mut game = Game::new(
        scenario(&[frozen!("aba5c046-2f8c-4749-b347-9a7d8cb1c865")])
            .permanent_or_hand("Ozai's Cruelty", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, true)
            .permanent_or_hand("Forest", 1, 1, true)
            .permanent_or_hand("Forest", 0, 0, true)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    pick_player(&mut game, PlayerId(1));
    Driver::pay(&mut game, &[1, 2, 3]);
    let request = choice(&mut game, Reason::ResolveDiscard);
    assert_eq!(request.player, PlayerId(1));
    assert_eq!(game.state.rules.players[1].life, 38.into());
    let selected = [4, 5].map(|id| Driver::object(&game, id));
    assert_eq!(request.objects_offered(), selected);
    Driver::answer(&mut game, 1, request.pick_objects(selected.into()));
    resolve(&mut game);
    assert_eq!(
        game.zone(Driver::object(&game, 6)),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
}

#[test]
fn unproved_result_and_player_shapes_are_located() {
    for (card, code) in [
        (
            frozen!("90e46d04-9d53-4a24-aff0-f8c61e1db97d"),
            "per_player_controller",
        ),
        (
            frozen!("b17ea905-0696-4e58-b564-557e87236e27"),
            "operand:filter:relation",
        ),
        (
            frozen!("e5223a09-f732-4747-8914-e6546ab0ef4c"),
            "graveyard_entry_owner",
        ),
        (
            frozen!("a46f9136-b904-403e-b263-e99c9a2fe2ec"),
            "operand:amount:counter_count:recipient",
        ),
        (
            frozen!("c4d36522-3ace-4bfb-bf2d-1a366f458698"),
            "unbound event player",
        ),
    ] {
        for (manifest, raw) in [(mtg_witness::R5_MANIFEST, card.0), (R6, card.1)] {
            let result = mtg_witness::compile(manifest, raw);
            assert!(format!("{result:?}").contains(code), "{result:?}");
        }
    }
}
