//! Frozen R5 witnesses for CR 608.2b, 701.6, 701.19, 702.34a and 704.5f–h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
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

fn board() -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in [
        record!("0baabe39-72ae-47bd-a095-cbf7eb8a6361"), // Ancient Ziggurat
        record!("36937483-30cb-449a-8028-75017a124922"), // Boseiju
        record!("215fd0b4-5817-4335-a0ed-1694e44bde41"), // Omen Hawker
        record!("cc187110-1148-4090-bbb8-e205694a39f5"), // Counterspell
        record!("3407fe41-fdd3-4119-8f70-4bc4590a379f"), // Negate
        record!("46665089-aa3d-44c3-964d-6638dfbb5782"), // Essence Scatter
        record!("b3b00911-ece7-4484-bc36-f211ce72b6cc"), // Stifle
        record!("d91b3c6f-f8a1-4859-90ca-22cd79517552"), // Minor Misstep
        record!("c61fe162-2202-4e56-9ba0-393547f9875f"), // Mana Leak
        record!("d64b0848-0193-4025-ba62-63ecd8fb9f50"), // Spell Pierce
        record!("fa85c5a2-8e83-4624-a35a-a0bbf17ecbb4"), // Think Twice
        record!("e3172ac0-213c-4da4-8b7a-8f84bc237cde"), // Mending Touch
        record!("c180ed02-07f4-4538-9bea-8c249234a8e2"), // Drudge Skeletons
        record!("1149e5ac-554a-41b1-84ae-bac42579c1aa"), // Thrun, the Last Troll
        record!("e4a6c421-bef1-4360-a2dc-ae9c61d86b2f"), // Welding Jar
        record!("b81f041d-98db-4408-9472-c483e4a502bc"), // Terror
    ] {
        match mtg_witness::compile(mtg_witness::R5_MANIFEST, raw) {
            mtg_compiler::CompileResult::Lowered { definition } => {
                if catalog
                    .definition_by_oracle_id(&definition.oracle_id)
                    .is_none()
                {
                    catalog.definitions.push(*definition);
                }
            }
            other => panic!("frozen counter or regeneration witness: {other:?}"),
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-counter-regeneration").step(TurnStep::PrecombatMain)
}

const Q: u64 = u64::MAX;
const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn add(board: Board, name: &str, controller: PlayerId, hand: bool) -> Board {
    board.permanent_or_hand(name, controller.0, controller.0, hand)
}

fn select(game: &mut Game, candidate: Candidate<Rules>) {
    let request = Driver::request(game, Q);
    let answer = request.pick_one(&candidate).unwrap();
    game.respond(request.player, request.id, answer).unwrap();
}

fn cast_on(game: &mut Game, card: u32, land: &[u32], target: ObjectId) {
    Driver::cast(game, CardId(card));
    select(game, Candidate::Object { object: target });
    Driver::pay(game, land);
}

fn counter(game: &mut Game, card: u32, land: &[u32]) -> StackId {
    let target = game.state.rules.stack[0].id;
    Driver::pass(game);
    Driver::cast(game, CardId(card));
    let request = Driver::request(game, Q);
    assert_eq!(request.candidates, [Candidate::Stack { stack: target }]);
    let permitted = game.next_test_viewer_decision();
    assert!(matches!(
        permitted.candidates.as_slice(),
        [Candidate::Stack { .. }]
    ));
    select(game, Candidate::Stack { stack: target });
    Driver::pay(game, land);
    target
}

fn pass_round(game: &mut Game) {
    for _ in game.live_players() {
        Driver::pass(game);
    }
}

fn resolve(game: &mut Game) {
    pass_round(game);
    assert_eq!(Driver::request(game, Q).reason, Reason::Priority);
}

fn zone(game: &Game, card: u32) -> Zone {
    game.current(Driver::object(game, card)).unwrap().zone
}

#[test]
fn counter_regeneration_spell_moves_to_graveyard_and_flashback_to_exile() {
    for flashback in [false, true] {
        let board = board().card(
            "Think Twice",
            0,
            0,
            if flashback {
                Zone::Graveyard { owner: P0 }
            } else {
                Zone::Hand { owner: P0 }
            },
        );
        let mut game = Game::new(
            add(
                add(
                    add(
                        add(
                            add(add(board, "Counterspell", P1, true), "Island", P0, false),
                            "Island",
                            P0,
                            false,
                        ),
                        "Island",
                        P0,
                        false,
                    ),
                    "Island",
                    P1,
                    false,
                ),
                "Island",
                P1,
                false,
            )
            .libraries("Forest", 4)
            .prepare(),
        );
        Driver::cast(&mut game, CardId(0));
        Driver::pay(&mut game, if flashback { &[2, 3, 4] } else { &[2, 3] });
        counter(&mut game, 1, &[5, 6]);
        resolve(&mut game);
        assert!(game.state.rules.stack.is_empty());
        assert_eq!(
            zone(&game, 0),
            if flashback {
                Zone::Exile
            } else {
                Zone::Graveyard { owner: P0 }
            }
        );
        assert_eq!(zone(&game, 1), Zone::Graveyard { owner: P1 });
        assert_eq!(game.state.rules.libraries[&P0].len(), 4);
    }
}

fn reject(game: &Game, tamper: impl FnOnce(&mut serde_json::Value), expected: &str) {
    let mut forged: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    tamper(&mut forged["state"]);
    forged["state_sha256"] = mtg_contracts::digest(&forged["state"]).unwrap().into();
    let error = Game::restore(game.prepared.as_ref().clone(), &forged.to_string())
        .err()
        .expect("forged state restored");
    assert!(error.contains(expected), "{error}");
}

#[test]
fn counter_regeneration_uncounterable_spell_is_legal_but_cannot_be_removed() {
    let mut board = add(
        add(board(), "Thrun, the Last Troll", P0, true),
        "Counterspell",
        P1,
        true,
    );
    for _ in 0..4 {
        board = add(board, "Forest", P0, false);
    }
    for _ in 0..2 {
        board = add(board, "Island", P1, false);
    }
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[2, 3, 4, 5]);
    let target = counter(&mut game, 1, &[6, 7]);
    pass_round(&mut game);
    for _ in 0..50 {
        if matches!(&game.state.rules.frame, Frame::Resolve { frame } if frame.cursor == 1) {
            break;
        }
        game.advance(Budget::new(1));
    }
    let Frame::Resolve { frame } = &game.state.rules.frame else {
        panic!("counter outcome missing")
    };
    assert!(
        matches!(frame.outcomes.as_slice(), [InstructionOutcome::CounterPrevented { target: id }] if *id == target)
    );
    assert_eq!(zone(&game, 0), Zone::Stack);
    reject(
        &game,
        |rules| {
            let outcome = &mut rules["frame"]["frame"]["outcomes"][0];
            outcome["outcome"] = "countered".into();
            outcome["movement"] = serde_json::Value::Null;
        },
        "snapshot counter outcome invariant",
    );
    Driver::request(&mut game, Q);
    resolve(&mut game);
    assert_eq!(zone(&game, 0), Zone::Battlefield);
}

#[test]
fn counter_regeneration_mana_leak_offers_the_spell_controller_live_payment() {
    for pay in [false, true] {
        let mut board = add(add(board(), "Think Twice", P0, true), "Mana Leak", P1, true);
        for _ in 0..5 {
            board = add(board, "Island", P0, false);
        }
        for _ in 0..2 {
            board = add(board, "Island", P1, false);
        }
        let mut game = Game::new(board.libraries("Forest", 4).prepare());
        Driver::cast(&mut game, CardId(0));
        Driver::pay(&mut game, &[2, 3]);
        counter(&mut game, 1, &[7, 8]);
        pass_round(&mut game);
        let offer = Driver::request(&mut game, Q);
        assert_eq!((offer.player, offer.reason), (P0, Reason::OptionalEffect));
        select(
            &mut game,
            Candidate::Flag {
                flag: if pay { Flag::Accept } else { Flag::Decline },
            },
        );
        if pay {
            Driver::pay(&mut game, &[4, 5, 6]);
        }
        assert_eq!(Driver::request(&mut game, Q).reason, Reason::Priority);
        if pay {
            assert_eq!(zone(&game, 0), Zone::Stack);
            resolve(&mut game);
        }
        assert_eq!(zone(&game, 0), Zone::Graveyard { owner: P0 });
        assert_eq!(
            game.state.rules.libraries[&P0].len(),
            if pay { 3 } else { 4 }
        );
    }
}

#[test]
fn counter_regeneration_ability_counter_removes_only_the_invocation() {
    let mut game = Game::new(
        add(
            add(
                add(
                    add(board(), "Drudge Skeletons", P0, false),
                    "Stifle",
                    P1,
                    true,
                ),
                "Swamp",
                P0,
                false,
            ),
            "Island",
            P1,
            false,
        )
        .libraries("Forest", 4)
        .prepare(),
    );
    let source = Driver::object(&game, 0);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    Driver::pay(&mut game, &[2]);
    counter(&mut game, 1, &[3]);
    resolve(&mut game);
    assert!(game.state.rules.stack.is_empty());
    assert!(game.state.rules.effects.is_empty());
    assert!(game.current(Driver::object(&game, 2)).unwrap().tapped);
    assert_eq!(zone(&game, 0), Zone::Battlefield);
}

#[test]
fn counter_regeneration_shield_clears_damage_and_taps_only_its_recipient() {
    let mut board = board();
    for (name, hand) in [
        ("Colossal Dreadmaw", false),
        ("Lightning Bolt", true),
        ("Mending Touch", true),
        ("Murder", true),
        ("Mountain", false),
        ("Forest", false),
        ("Swamp", false),
        ("Swamp", false),
        ("Swamp", false),
        ("Grizzly Bears", false),
    ] {
        board = add(board, name, P0, hand);
    }
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    let target = Driver::object(&game, 0);
    cast_on(&mut game, 1, &[4], target);
    resolve(&mut game);
    assert_eq!(game.current(target).unwrap().marked_damage, 3.into());
    cast_on(&mut game, 2, &[5], target);
    resolve(&mut game);
    let restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    reject(
        &game,
        |rules| rules["effects"][0]["kind"]["target"]["incarnation"] = 999.into(),
        "effect definition/provenance invariant",
    );
    reject(
        &game,
        |rules| rules["effects"][0]["expires_in_cleanup_of_turn"] = 0.into(),
        "effect state identity invariant",
    );
    // After the invocation leaves, only replay can prove the original
    // selection against a forged recipient and its matching public handles.
    assert!(game.state.rules.stack.is_empty());
    let other = Driver::object(&game, 9);
    let handles: Vec<_> = (0..4)
        .map(|viewer| game.handle_for(PlayerId(viewer), other).unwrap())
        .collect();
    reject(
        &game,
        |rules| {
            rules["effects"][0]["kind"]["target"] = serde_json::to_value(other).unwrap();
            rules["effects"][0]["target_handles"] = serde_json::to_value(handles).unwrap();
        },
        "snapshot is not reachable from this scenario's accepted inputs",
    );
    cast_on(&mut game, 3, &[6, 7, 8], target);
    resolve(&mut game);
    let creature = game.current(target).unwrap();
    assert_eq!(creature.zone, Zone::Battlefield);
    assert!(creature.tapped);
    assert_eq!(creature.marked_damage, Quantity::zero());
    assert!(game.state.rules.effects.is_empty());
}

#[test]
fn counter_regeneration_forbidden_destruction_and_zero_toughness_bypass_shields() {
    for (lethal, lands) in [("Terror", &[4, 5][..]), ("Disfigure", &[4][..])] {
        let mut board = board();
        for (name, hand) in [
            ("Grizzly Bears", false),
            ("Mending Touch", true),
            (lethal, true),
            ("Forest", false),
            ("Swamp", false),
            ("Swamp", false),
        ] {
            board = add(board, name, P0, hand);
        }
        let mut game = Game::new(board.libraries("Forest", 4).prepare());
        let target = Driver::object(&game, 0);
        cast_on(&mut game, 1, &[3], target);
        resolve(&mut game);
        assert_eq!(game.state.rules.effects.len(), 1);
        cast_on(&mut game, 2, lands, target);
        resolve(&mut game);
        assert_eq!(zone(&game, 0), Zone::Graveyard { owner: P0 });
        assert!(!game.state.rules.effects.iter().any(|effect| {
            matches!(effect.kind, ResolvedEffectKind::RegenerationShield { .. })
        }));
    }
}

#[test]
fn counter_regeneration_spell_filters_choose_from_the_paid_stack() {
    for (spell, lands, eligible) in [
        ("Grizzly Bears", &[6, 7][..], ["Essence Scatter"].as_slice()),
        (
            "Think Twice",
            &[8, 9][..],
            ["Negate", "Spell Pierce"].as_slice(),
        ),
    ] {
        let mut board = add(board(), spell, P0, true);
        for counter in [
            "Essence Scatter",
            "Negate",
            "Spell Pierce",
            "Minor Misstep",
            "Stifle",
        ] {
            board = add(board, counter, P1, true);
        }
        for land in ["Forest", "Forest", "Island", "Island"] {
            board = add(board, land, P0, false);
        }
        for _ in 0..2 {
            board = add(board, "Island", P1, false);
        }
        let mut game = Game::new(board.libraries("Forest", 4).prepare());
        Driver::cast(&mut game, CardId(0));
        Driver::pay(&mut game, lands);
        Driver::pass(&mut game);
        let decision = Driver::request(&mut game, Q);
        let offered = decision
            .candidates
            .iter()
            .filter_map(|candidate| match candidate {
                Candidate::Action {
                    action: Action::Cast { card },
                } => Some(game.definition(card.card).unwrap().name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(offered, eligible);
    }

    let mut board = board();
    for (name, seat, hand) in [
        ("Lightning Bolt", P0, true),
        ("Minor Misstep", P1, true),
        ("Grizzly Bears", P0, false),
        ("Mountain", P0, false),
        ("Island", P1, false),
    ] {
        board = add(board, name, seat, hand);
    }
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    let target = Driver::object(&game, 2);
    cast_on(&mut game, 0, &[3], target);
    counter(&mut game, 1, &[4]);
    resolve(&mut game);
    assert_eq!(zone(&game, 0), Zone::Graveyard { owner: P0 });
    assert_eq!(
        game.current(target).unwrap().marked_damage,
        Quantity::zero()
    );
}

#[test]
fn counter_regeneration_self_shield_expires_at_cleanup() {
    let mut board = board().step(TurnStep::End);
    for (name, seat, hand) in [
        ("Drudge Skeletons", P0, false),
        ("Swamp", P0, false),
        ("Murder", P1, true),
        ("Swamp", P1, false),
        ("Swamp", P1, false),
        ("Swamp", P1, false),
    ] {
        board = add(board, name, seat, hand);
    }
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    let source = Driver::object(&game, 0);
    let turn = game.state.rules.turn.number;
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    Driver::pay(&mut game, &[1]);
    resolve(&mut game);
    assert!(matches!(&game.state.rules.effects[..], [effect]
        if effect.kind == ResolvedEffectKind::RegenerationShield { target: source }));
    resolve(&mut game);
    assert!(game.state.rules.turn.number > turn);
    assert!(game.state.rules.effects.is_empty());
    cast_on(&mut game, 2, &[3, 4, 5], source);
    resolve(&mut game);
    assert_eq!(zone(&game, 0), Zone::Graveyard { owner: P0 });
}

fn finish_mana(game: &mut Game) {
    Driver::answer(
        game,
        Q,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
}

fn spend_witness(
    game: &mut Game,
    unrestricted: std::collections::BTreeMap<Color, Quantity>,
    restricted: Vec<mtg_contracts::RestrictedMana>,
) {
    Driver::answer(
        game,
        Q,
        Answer::Pay {
            pay: PayAnswer::SpendRestricted {
                unrestricted,
                restricted,
            },
        },
    );
}

fn witness_pool(game: &mut Game) -> ManaPool {
    let decision = Driver::request(game, Q);
    let Shape::Pay { pool, .. } = decision.shape else {
        panic!("expected a payment window")
    };
    pool
}

fn priority_mana(game: &mut Game, card: u32) {
    Driver::action(
        game,
        Action::ActivateMana {
            object: Driver::object(game, card),
        },
    );
}

fn round_trip(game: &Game) -> Game {
    game.validate_state().unwrap();
    let raw = game.snapshot().unwrap();
    let restored = Game::restore(game.prepared.as_ref().clone(), &raw).unwrap();
    assert_eq!(restored.snapshot().unwrap(), raw);
    restored
}

#[test]
fn creature_only_mana_is_bound_to_the_purchase_on_restore() {
    use std::collections::BTreeMap;
    let board = board()
        .permanent_or_hand("Ancient Ziggurat", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Think Twice", 0, 0, true)
        .permanent_or_hand("Drudge Skeletons", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false);
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    priority_mana(&mut game, 0);
    select(&mut game, Candidate::Color { color: Color::B });
    priority_mana(&mut game, 1);
    priority_mana(&mut game, 4);
    Driver::request(&mut game, Q);
    let mut game = round_trip(&game);
    let bucket = game.state.rules.players[0].restricted_mana.clone();
    assert_eq!(bucket.len(), 1);

    let mut illegal = round_trip(&game);
    Driver::cast(&mut illegal, CardId(2));
    finish_mana(&mut illegal);
    let offer = Driver::request(&mut illegal, Q);
    assert!(witness_pool(&mut illegal).restricted.is_empty());
    assert!(
        illegal
            .respond(
                offer.player,
                offer.id,
                Answer::Pay {
                    pay: PayAnswer::SpendRestricted {
                        unrestricted: BTreeMap::from([(Color::U, 1.into())]),
                        restricted: bucket.clone(),
                    },
                }
            )
            .is_err()
    );

    Driver::cast(&mut game, CardId(3));
    assert_eq!(witness_pool(&mut game).restricted, bucket);
    finish_mana(&mut game);
    spend_witness(&mut game, BTreeMap::from([(Color::U, 1.into())]), bucket);
    Driver::request(&mut game, Q);
    assert!(game.state.rules.players[0].restricted_mana.is_empty());
    assert!(game.state.rules.stack.last().unwrap().paid.is_some());
}

#[test]
fn uncounterable_rider_uses_the_selected_mana_and_only_that_spell() {
    use std::collections::BTreeMap;
    for use_rider in [false, true] {
        let board = board()
            .permanent_or_hand("Boseiju, Who Shelters All", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Think Twice", 0, 0, true)
            .permanent_or_hand("Counterspell", 1, 1, true)
            .permanent_or_hand("Island", 1, 1, false)
            .permanent_or_hand("Island", 1, 1, false);
        let mut game = Game::new(board.libraries("Forest", 4).prepare());
        priority_mana(&mut game, 0);
        priority_mana(&mut game, 1);
        priority_mana(&mut game, 2);
        let mut game = round_trip(&game);
        Driver::cast(&mut game, CardId(3));
        finish_mana(&mut game);
        let pool = witness_pool(&mut game);
        assert_eq!(pool.restricted.len(), 1);
        if use_rider {
            spend_witness(
                &mut game,
                BTreeMap::from([(Color::U, 1.into())]),
                pool.restricted,
            );
        } else {
            Driver::answer(
                &mut game,
                Q,
                Answer::Pay {
                    pay: PayAnswer::Spend {
                        unrestricted: BTreeMap::from([(Color::U, 1.into()), (Color::G, 1.into())]),
                    },
                },
            );
        }
        Driver::request(&mut game, Q);
        let mut game = round_trip(&game);
        assert_eq!(
            game.state.rules.stack.last().unwrap().mana_uncounterable,
            use_rider
        );
        counter(&mut game, 4, &[5, 6]);
        resolve(&mut game);
        assert_eq!(game.state.rules.stack.len(), usize::from(use_rider));
    }
}

#[test]
fn activation_mana_is_eligible_only_for_activation_while_rider_mana_can_pay_resolution() {
    use std::collections::BTreeMap;
    let board = board()
        .permanent_or_hand("Think Twice", 0, 0, true)
        .permanent_or_hand("Mana Leak", 1, 1, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Omen Hawker", 0, 0, false)
        .permanent_or_hand("Boseiju, Who Shelters All", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 1, 1, false)
        .permanent_or_hand("Island", 1, 1, false);
    let mut game = Game::new(board.libraries("Forest", 4).prepare());
    for card in 4..=7 {
        priority_mana(&mut game, card);
    }
    Driver::cast(&mut game, CardId(0));
    finish_mana(&mut game);
    let pool = witness_pool(&mut game);
    assert_eq!(pool.restricted.len(), 1);
    Driver::answer(
        &mut game,
        Q,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([(Color::U, 2.into())]),
            },
        },
    );
    counter(&mut game, 1, &[8, 9]);
    pass_round(&mut game);
    select(&mut game, Candidate::Flag { flag: Flag::Accept });
    // Add two ordinary mana. Neither of Omen Hawker's units may contribute.
    for card in [2, 3] {
        let object = Driver::object(&game, card);
        let decision = Driver::request(&mut game, Q);
        let source = decision
            .index(&Candidate::Action {
                action: Action::ActivateMana { object },
            })
            .unwrap();
        Driver::answer(
            &mut game,
            Q,
            Answer::Pay {
                pay: PayAnswer::Activate { source },
            },
        );
    }
    finish_mana(&mut game);
    let pool = witness_pool(&mut game);
    assert_eq!(pool.restricted.len(), 1);
    spend_witness(
        &mut game,
        BTreeMap::from([(Color::U, 2.into())]),
        pool.restricted,
    );
    Driver::request(&mut game, Q);
    assert_eq!(zone(&game, 0), Zone::Stack);
    assert!(!game.state.rules.stack[0].mana_uncounterable);
    assert_eq!(game.state.rules.players[0].restricted_mana.len(), 2);
}

#[test]
fn ranked_frozen_spend_restrictions_compile() {
    for (raw, expected) in [
        (
            record!("7fab8d65-af51-47d3-8f10-2676bf6e8ba3"),
            mtg_contracts::ManaSpendRestriction::ColorlessEldraziSpellOrAbility,
        ),
        (
            record!("8ea46945-d5ab-4209-b473-4769e7b8b962"),
            mtg_contracts::ManaSpendRestriction::ColorlessSpell,
        ),
        (
            record!("b36accb4-cde6-4b1d-b549-8c975cfa999d"),
            mtg_contracts::ManaSpendRestriction::ArtifactSpellOrArtifactAbility,
        ),
        (
            record!("f9d3b046-0b95-4103-a630-4b3fb88bb60b"),
            mtg_contracts::ManaSpendRestriction::LegendarySpellUncounterable,
        ),
    ] {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("ranked restriction {expected:?} must lower: {result:?}");
        };
        assert!(
            definition
                .mana_abilities
                .iter()
                .any(|ability| ability.spend_restriction == Some(expected))
        );
    }
}
