//! Face-down spells and permanents: morph and megamorph (CR 702.37, 708,
//! 116.2b). Witnesses are exact Archive R5 records.
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

const HERMIT: &str = record!("69d2e5f3-2f4d-4a36-bcd5-181164ecd894");
const AINOK: &str = record!("7a5dae5f-29ab-4996-b236-c754de8769bc");
const CAVERN: &str = record!("3763de30-28e1-4689-a71c-07d2fea3a466");
const AKROMA: &str = record!("2b80faaf-92fd-4fa0-a3f6-8bb263e7ff1d");
const NAMELESS: &str = record!("bd8d0498-53f9-484c-a59a-78daca468859");
const BATTLEGROWTH: &str = record!("650bf82e-7f83-470c-bf4a-34281bfe9341");
const SPHERE: &str = record!("09c96077-3804-4f12-a613-5bebc5e0413f");

fn lower(raw: &str) -> Definition {
    match mtg_witness::compile(mtg_witness::R5_MANIFEST, raw) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        outcome => panic!("morph witness did not lower: {outcome:?}"),
    }
}

fn board() -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for record in [
        HERMIT,
        AINOK,
        CAVERN,
        AKROMA,
        NAMELESS,
        BATTLEGROWTH,
        SPHERE,
    ] {
        catalog.definitions.push(lower(record));
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "face-down")
        .step(TurnStep::PrecombatMain)
        .seats(2)
}

fn lands(mut board: Board, name: &str, count: usize) -> Board {
    for _ in 0..count {
        board = board.permanent_or_hand(name, 0, 0, false);
    }
    board
}

fn view(game: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(view)) =
        game.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("observation unavailable");
    };
    view
}

fn can(decision: &Decision<Rules>, action: Action<Rules>) -> bool {
    decision.index(&Candidate::Action { action }).is_some()
}

fn settle_stack(game: &mut Game) {
    while !game.state.rules.stack.is_empty() {
        Driver::pass(game);
        Driver::request(game, u64::MAX);
    }
}

fn restore(game: &Game) -> Game {
    let snapshot = game.snapshot().unwrap();
    Game::restore(game.prepared.as_ref().clone(), &snapshot).unwrap()
}

/// The one card of the stack or the one Thelonite Hermit row the viewer
/// sees in `zone`, face down or not.
fn seen(view: &Observation, zone: Zone) -> VisibleObject {
    let rows: Vec<_> = view
        .view
        .objects
        .iter()
        .filter(|o| {
            o.zone == zone
                && (zone == Zone::Stack
                    || o.face_down
                    || o.characteristics
                        .as_ref()
                        .is_some_and(|c| c.subtypes == ["Elf", "Shaman"]))
        })
        .collect();
    assert_eq!(rows.len(), 1, "{zone:?}");
    rows[0].clone()
}

/// CR 708.2a: a 2/2 creature with no name, text, subtypes or mana cost.
fn assert_face_down_characteristics(c: &Characteristics) {
    assert_eq!(c.kind, CardKind::Creature);
    assert!(c.additional_types.is_empty());
    assert!(c.subtypes.is_empty());
    assert!(c.colors.is_empty());
    assert!(!c.legendary);
    assert_eq!(c.printed_mana_value(), 0.into());
    assert_eq!(c.power, Some(2.into()));
    assert_eq!(c.toughness, Some(2.into()));
}

/// Cast card 0 face down from hand with three Forests (cards 2-4) and let
/// it resolve.
fn cast_face_down(game: &mut Game) -> ObjectId {
    let card = Driver::object(game, 0);
    Driver::action(game, Action::CastFaceDown { card });
    Driver::request(game, u64::MAX);
    assert_eq!(
        game.state.rules.stack[0].total_cost,
        ManaCost {
            generic: 3.into(),
            colored: vec![],
        }
    );
    Driver::pay(game, &[2, 3, 4]);
    settle_stack(game);
    Driver::object(game, 0)
}

#[test]
fn morph_and_megamorph_lower_to_face_down_casts() {
    let hermit = lower(HERMIT);
    assert_eq!(
        hermit.face_down,
        Some(FaceDownCast {
            kind: FaceDownKind::Morph,
            cost: ManaCost {
                generic: 3.into(),
                colored: vec![Color::G, Color::G],
            },
        })
    );
    assert!(hermit.capabilities.contains(&Capability::FaceDownCastV1));
    assert_eq!(
        hermit.triggered[0].condition.pattern(),
        &mtg_contracts::kir::EventPattern::self_turned_face_up()
    );
    assert_eq!(
        mtg_contracts::kir::definition_tiers(&hermit).unwrap(),
        mtg_contracts::kir::Tiers::EXECUTES
    );
    let ainok = lower(AINOK);
    assert_eq!(ainok.face_down.unwrap().kind, FaceDownKind::Megamorph);
    // A land card with morph is cast face down as a creature spell.
    let cavern = lower(CAVERN);
    assert_eq!(cavern.characteristics.kind, CardKind::Land);
    assert!(cavern.spell.is_none());
    assert!(cavern.face_down.is_some());
}

#[test]
fn a_face_down_spell_is_a_nameless_colorless_2_2_for_three_and_stays_private() {
    let printed = lower(HERMIT);
    let mut game = Game::new(
        lands(
            board()
                .permanent_or_hand("Thelonite Hermit", 0, 0, true)
                .permanent_or_hand("Ainok Survivalist", 0, 0, true),
            "Forest",
            8,
        )
        .prepare(),
    );
    let hermit = Driver::object(&game, 0);
    let ainok = Driver::object(&game, 1);
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(can(&decision, Action::Cast { card: hermit }));
    assert!(can(&decision, Action::CastFaceDown { card: hermit }));
    assert!(can(&decision, Action::CastFaceDown { card: ainok }));
    // A Forest has no morph ability (CR 702.37d).
    assert!(!can(
        &decision,
        Action::CastFaceDown {
            card: Driver::object(&game, 2)
        }
    ));

    // CR 702.37c, 601.2f: the quote prices the face-down spell, not the card.
    let handle = game.handle_for(PlayerId(0), hermit).unwrap();
    assert_eq!(
        game.quote_face_down_cast_cost(PlayerId(0), &handle)
            .unwrap(),
        ManaCost {
            generic: 3.into(),
            colored: vec![],
        }
    );
    Driver::action(&mut game, Action::CastFaceDown { card: hermit });
    Driver::pay(&mut game, &[2, 3, 4]);
    // CR 708.4: on the stack it is a face-down creature spell.
    let spell = game.state.rules.stack[0].source;
    assert!(game.current(spell).unwrap().face_down);
    assert_face_down_characteristics(&game.characteristics(spell).unwrap());
    let opponent = view(&game, 1);
    let row = seen(&opponent, Zone::Stack);
    assert!(row.face_down);
    assert!(row.oracle_id.is_empty());
    assert!(row.faces.is_none());
    assert_face_down_characteristics(row.characteristics.as_ref().unwrap());
    assert_eq!(opponent.view.stack[0].mana_value, None);
    // CR 708.5: its controller may look at it.
    let own = seen(&view(&game, 0), Zone::Stack);
    assert!(own.face_down);
    assert_eq!(own.oracle_id, printed.oracle_id);
    assert_recoverable((&game).into());
    assert_eq!(restore(&game).state_digest(), game.state_digest());

    settle_stack(&mut game);
    let permanent = Driver::object(&game, 0);
    let object = game.current(permanent).unwrap();
    assert_eq!(object.zone, Zone::Battlefield);
    assert!(object.face_down);
    // No abilities: not an Elf, and the Saproling anthem does not apply.
    assert_face_down_characteristics(&game.characteristics(permanent).unwrap());
    assert!(game.current_keywords(permanent).is_empty());
    assert!(game.state.rules.stack.is_empty());
    let row = seen(&view(&game, 1), Zone::Battlefield);
    assert!(row.face_down && row.oracle_id.is_empty());
    game.validate_state().unwrap();
    assert_recoverable((&game).into());
}

#[test]
fn opponents_see_the_same_view_for_two_different_face_down_cards() {
    let views = [
        ["Thelonite Hermit", "Ainok Survivalist"],
        ["Ainok Survivalist", "Thelonite Hermit"],
    ]
    .map(|[first, second]| {
        let mut game = Game::new(
            lands(
                board()
                    .permanent_or_hand(first, 0, 0, true)
                    .permanent_or_hand(second, 0, 0, true),
                "Forest",
                8,
            )
            .prepare(),
        );
        let card = Driver::object(&game, 0);
        Driver::action(&mut game, Action::CastFaceDown { card });
        Driver::pay(&mut game, &[2, 3, 4]);
        let on_stack = [view(&game, 1), view(&game, 0)];
        settle_stack(&mut game);
        [on_stack, [view(&game, 1), view(&game, 0)]]
    });
    let [hermit, ainok] = &views;
    for step in 0..2 {
        // Opponent observations are identical, permitted history included.
        assert_eq!(hermit[step][0], ainok[step][0]);
        // The controller can tell them apart.
        assert_ne!(hermit[step][1], ainok[step][1]);
    }
}

#[test]
fn turning_face_up_is_a_special_action_that_triggers_and_keeps_the_object() {
    let printed = lower(HERMIT);
    let mut game = Game::new(
        lands(
            board()
                .permanent_or_hand("Thelonite Hermit", 0, 0, true)
                .permanent_or_hand("Grizzly Bears", 1, 1, false),
            "Forest",
            8,
        )
        .prepare(),
    );
    let hermit = cast_face_down(&mut game);
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(can(&decision, Action::TurnFaceUp { object: hermit }));
    // The opponent cannot turn it face up.
    assert!(!can(
        &decision,
        Action::TurnFaceUp {
            object: Driver::object(&game, 1)
        }
    ));
    Driver::action(&mut game, Action::TurnFaceUp { object: hermit });
    // CR 116.2b: no stack object; the controller pays the morph cost.
    assert!(game.state.rules.stack.is_empty());
    let pay = Driver::request(&mut game, u64::MAX);
    let Shape::Pay { cost, .. } = &pay.shape else {
        panic!("expected the morph payment");
    };
    assert_eq!(*cost, printed.face_down.as_ref().unwrap().cost);
    let source = pay
        .index(&Candidate::Action {
            action: Action::ActivateMana {
                object: Driver::object(&game, 5),
            },
        })
        .unwrap();
    game.respond(
        pay.player,
        pay.id,
        Answer::Pay {
            pay: PayAnswer::Activate { source },
        },
    )
    .unwrap();
    // Suspended mid-payment: the special action's frame recovers exactly.
    assert_recoverable((&game).into());
    assert_eq!(restore(&game).state_digest(), game.state_digest());
    Driver::pay(&mut game, &[6, 7, 8, 9]);

    // CR 708.8: the same object, with its normal characteristics.
    assert_eq!(Driver::object(&game, 0), hermit);
    let object = game.current(hermit).unwrap();
    assert!(!object.face_down);
    assert_eq!(object.zone, Zone::Battlefield);
    let c = game.characteristics(hermit).unwrap();
    assert_eq!(c.subtypes, ["Elf", "Shaman"]);
    assert_eq!(c.power, Some(1.into()));
    // CR 603.2, 702.37e: its "turned face up" ability triggered.
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(game.state.rules.stack[0].source, hermit);
    assert_eq!(game.state.rules.priority, PlayerId(0));
    // Everyone sees it now.
    assert!(
        !seen(&view(&game, 1), Zone::Battlefield)
            .oracle_id
            .is_empty()
    );
    settle_stack(&mut game);
    let saprolings: Vec<_> = game
        .state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(0))
        .filter(|o| {
            game.characteristics(o.id)
                .is_ok_and(|c| c.subtypes == ["Saproling"])
        })
        .map(|o| o.id)
        .collect();
    assert_eq!(saprolings.len(), 4);
    for token in saprolings {
        // All Saprolings get +1/+1 now that the Hermit is face up.
        assert_eq!(game.characteristics(token).unwrap().power, Some(2.into()));
    }
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(!can(&decision, Action::TurnFaceUp { object: hermit }));
    game.validate_state().unwrap();
    assert_recoverable((&game).into());
}

#[test]
fn an_unpaid_morph_cost_abandons_the_special_action() {
    let mut game = Game::new(
        lands(
            board().permanent_or_hand("Thelonite Hermit", 0, 0, true),
            "Forest",
            4,
        )
        .prepare(),
    );
    let hermit = cast_face_down(&mut game);
    Driver::action(&mut game, Action::TurnFaceUp { object: hermit });
    // One Forest cannot pay {3}{G}{G}; the tap is reversed (CR 733.1).
    Driver::pay(&mut game, &[1]);
    assert!(game.current(hermit).unwrap().face_down);
    assert!(!game.current(Driver::object(&game, 1)).unwrap().tapped);
    let decision = Driver::request(&mut game, u64::MAX);
    assert_eq!(decision.reason, Reason::Priority);
    assert_recoverable((&game).into());
}

#[test]
fn megamorph_turns_face_up_with_a_counter_and_its_trigger_targets() {
    let mut game = Game::new(
        lands(
            board()
                .permanent_or_hand("Ainok Survivalist", 0, 0, true)
                .permanent_or_hand("Glorious Anthem", 1, 1, false),
            "Forest",
            6,
        )
        .prepare(),
    );
    let ainok = cast_face_down(&mut game);
    let anthem = Driver::object(&game, 1);
    Driver::action(&mut game, Action::TurnFaceUp { object: ainok });
    Driver::pay(&mut game, &[5, 6]);
    // CR 702.37b: the megamorph cost adds a +1/+1 counter.
    let object = game.current(ainok).unwrap();
    assert!(!object.face_down);
    assert_eq!(object.plus_one_counters, 1.into());
    let c = game.characteristics(ainok).unwrap();
    assert_eq!((c.power, c.toughness), (Some(3.into()), Some(2.into())));
    Driver::pick_object(&mut game, anthem);
    settle_stack(&mut game);
    assert_eq!(
        game.current(Driver::object(&game, 1)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert_recoverable((&game).into());
}

#[test]
fn a_face_down_permanent_is_revealed_as_it_leaves_the_battlefield() {
    let hermit = lower(HERMIT);
    let ainok = lower(AINOK);
    let mut game = Game::new(
        lands(
            board()
                .permanent_or_hand("Thelonite Hermit", 0, 0, true)
                .permanent_or_hand("Ainok Survivalist", 0, 0, true),
            "Forest",
            6,
        )
        .permanent_or_hand("Unsummon", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Lightning Bolt", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .prepare(),
    );
    let face_down = cast_face_down(&mut game);
    // CR 708.9: bounced to its owner's hand, it is revealed to everyone.
    Driver::cast(&mut game, CardId(8));
    Driver::pick_object(&mut game, face_down);
    Driver::pay(&mut game, &[9]);
    settle_stack(&mut game);
    let returned = game.current(Driver::object(&game, 0)).unwrap();
    assert_eq!(returned.zone, Zone::Hand { owner: PlayerId(0) });
    assert!(!returned.face_down);
    assert!(
        view(&game, 1)
            .view
            .known_searched_hand
            .contains(&(PlayerId(0), hermit.oracle_id.clone()))
    );
    assert_recoverable((&game).into());

    // CR 708.9: dying, it is revealed in its owner's graveyard.
    let card = Driver::object(&game, 1);
    Driver::action(&mut game, Action::CastFaceDown { card });
    Driver::pay(&mut game, &[5, 6, 7]);
    settle_stack(&mut game);
    let target = Driver::object(&game, 1);
    assert!(game.current(target).unwrap().face_down);
    Driver::cast(&mut game, CardId(10));
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[11]);
    settle_stack(&mut game);
    let dead = Driver::object(&game, 1);
    assert_eq!(
        game.current(dead).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    let row = view(&game, 1)
        .view
        .objects
        .into_iter()
        .find(|o| {
            o.zone == Zone::Graveyard { owner: PlayerId(0) } && o.oracle_id == ainok.oracle_id
        })
        .unwrap();
    assert!(!row.face_down);
    game.validate_state().unwrap();
}

#[test]
fn a_land_with_morph_is_cast_as_a_creature_and_turns_face_up_into_a_land() {
    let mut game = Game::new(
        lands(
            board()
                .permanent_or_hand("Zoetic Cavern", 0, 0, true)
                .permanent_or_hand("Grizzly Bears", 1, 1, false),
            "Forest",
            5,
        )
        .prepare(),
    );
    let decision = Driver::request(&mut game, u64::MAX);
    let cavern = Driver::object(&game, 0);
    assert!(can(&decision, Action::PlayLand { object: cavern }));
    assert!(!can(&decision, Action::Cast { card: cavern }));
    let cavern = cast_face_down(&mut game);
    assert_face_down_characteristics(&game.characteristics(cavern).unwrap());
    // A face-down permanent has no mana abilities.
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(!can(&decision, Action::ActivateMana { object: cavern }));
    // Turning it face up is not a land play.
    let played = game.state.rules.turn.lands_played;
    Driver::action(&mut game, Action::TurnFaceUp { object: cavern });
    Driver::pay(&mut game, &[5, 6]);
    assert_eq!(game.state.rules.turn.lands_played, played);
    let c = game.characteristics(cavern).unwrap();
    assert_eq!(c.kind, CardKind::Land);
    assert!(!c.has_type(CardKind::Creature));
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(can(&decision, Action::ActivateMana { object: cavern }));
    game.validate_state().unwrap();
    assert_recoverable((&game).into());
}

#[test]
fn a_face_down_legend_has_no_face_up_abilities_or_supertypes() {
    let mut game = Game::new(
        lands(
            board().permanent_or_hand("Akroma, Angel of Fury", 0, 0, true),
            "Mountain",
            9,
        )
        .prepare(),
    );
    let akroma = cast_face_down(&mut game);
    // Not legendary, no flying, trample or protection, no pump ability.
    assert_face_down_characteristics(&game.characteristics(akroma).unwrap());
    assert!(game.current_keywords(akroma).is_empty());
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(!can(
        &decision,
        Action::ActivateAbility {
            object: akroma,
            ability: 0
        }
    ));
    assert!(can(&decision, Action::TurnFaceUp { object: akroma }));
    Driver::action(&mut game, Action::TurnFaceUp { object: akroma });
    Driver::pay(&mut game, &[1, 5, 6, 7, 8, 9]);
    let c = game.characteristics(akroma).unwrap();
    assert!(c.legendary);
    assert!(game.has_keyword(akroma, KeywordAbility::Flying));
    assert_recoverable((&game).into());
}

#[test]
fn snapshots_reject_impossible_face_down_objects() {
    let mut game = Game::new(
        lands(
            board()
                .permanent_or_hand("Thelonite Hermit", 0, 0, true)
                .permanent_or_hand("Ainok Survivalist", 0, 0, true),
            "Forest",
            5,
        )
        .prepare(),
    );
    let hermit = cast_face_down(&mut game);
    game.validate_state().unwrap();
    // A Forest has no morph ability.
    let mut forged = game.fork();
    forged
        .state
        .rules
        .objects
        .get_mut(&CardId(5))
        .unwrap()
        .face_down = true;
    assert!(
        forged
            .validate_state()
            .unwrap_err()
            .contains("face-down object")
    );
    // A face-down card cannot be in a hand.
    let mut forged = game.fork();
    forged
        .state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .face_down = true;
    assert!(
        forged
            .validate_state()
            .unwrap_err()
            .contains("face-down object")
    );
    // Face-up facts never become face down without the zone change.
    let mut forged = game.fork();
    forged
        .state
        .rules
        .objects
        .get_mut(&hermit.card)
        .unwrap()
        .transforms = 1;
    assert!(forged.validate_state().is_err());
}

#[test]
fn a_land_turned_face_up_while_attacking_is_removed_from_combat() {
    let mut game = Game::new(
        lands(
            board().permanent_or_hand("Zoetic Cavern", 0, 0, false),
            "Forest",
            2,
        )
        .prepare(),
    );
    // A face-down Cavern that has been under its controller's control since
    // the turn began (CR 302.6).
    let cavern = Driver::object(&game, 0);
    game.state
        .rules
        .objects
        .get_mut(&cavern.card)
        .unwrap()
        .face_down = true;
    game.validate_state().unwrap();
    let life = game.state.rules.players[1].life.clone();
    let mut turned = false;
    for _ in 0..64 {
        let r = Driver::request(&mut game, u64::MAX);
        if game.state.rules.turn.step == TurnStep::PostcombatMain {
            break;
        }
        match &r.reason {
            Reason::Attackers => {
                let answer = r.attack(vec![(cavern, PlayerId(1))]);
                game.respond(r.player, r.id, answer).unwrap();
            }
            Reason::Blockers => {
                let answer = r.block(vec![]);
                game.respond(r.player, r.id, answer).unwrap();
            }
            Reason::Priority
                if !turned && game.state.rules.combat.iter().any(|a| a.attacker == cavern) =>
            {
                turned = true;
                Driver::action(&mut game, Action::TurnFaceUp { object: cavern });
                Driver::pay(&mut game, &[1, 2]);
                // CR 506.4: a land is not a creature; it leaves combat.
                assert!(game.state.rules.combat.is_empty());
                assert!(
                    !game
                        .characteristics(cavern)
                        .unwrap()
                        .has_type(CardKind::Creature)
                );
                // A private board start is not a replayable journal, so
                // only the state invariants are checked here.
                game.validate_state().unwrap();
            }
            Reason::Priority => {
                let answer = mtg_witness::temporary::pass(&r);
                game.respond(r.player, r.id, answer).unwrap();
            }
            other => panic!("unexpected decision {other:?}"),
        }
    }
    assert!(turned);
    assert_eq!(game.state.rules.players[1].life, life);
}

#[test]
fn a_reversed_face_down_cast_returns_the_card_unrevealed() {
    let mut game = Game::new(
        lands(
            board().permanent_or_hand("Thelonite Hermit", 0, 0, true),
            "Forest",
            2,
        )
        .prepare(),
    );
    let before = view(&game, 1);
    let card = Driver::object(&game, 0);
    Driver::action(&mut game, Action::CastFaceDown { card });
    // Two Forests cannot pay {3}: the attempt is reversed (CR 733.1).
    Driver::pay(&mut game, &[1, 2]);
    assert!(game.state.rules.stack.is_empty());
    let object = game.current(Driver::object(&game, 0)).unwrap();
    assert_eq!(object.zone, Zone::Hand { owner: PlayerId(0) });
    assert!(!object.face_down);
    let after = view(&game, 1);
    assert!(after.view.known_searched_hand.is_empty());
    assert_eq!(after.view.objects, before.view.objects);
    game.validate_state().unwrap();
    assert_recoverable((&game).into());
}

#[test]
fn a_face_down_card_has_no_characteristic_defining_power_and_toughness() {
    let mut game = Game::new(
        lands(
            board().permanent_or_hand("Nameless One", 0, 0, true),
            "Island",
            6,
        )
        .permanent_or_hand("Azure Mage", 1, 1, false)
        .permanent_or_hand("Temporal Adept", 1, 1, false)
        .prepare(),
    );
    // CR 708.2, 604.3: face down it has no ability defining its P/T.
    let nameless = cast_face_down(&mut game);
    assert_face_down_characteristics(&game.characteristics(nameless).unwrap());
    Driver::action(&mut game, Action::TurnFaceUp { object: nameless });
    Driver::pay(&mut game, &[1, 5, 6]);
    // Face up, it counts the Wizards: itself, Azure Mage and Temporal Adept (CR 208.2a).
    let c = game.characteristics(nameless).unwrap();
    assert_eq!((c.power, c.toughness), (Some(3.into()), Some(3.into())));
    assert_eq!(c.subtypes, ["Wizard", "Avatar"]);
    assert_recoverable((&game).into());
}

/// CR 708.5-708.6: two face-down creatures cast in the same public order
/// look the same to an opponent choosing a target among them, whichever
/// physical card came first; only public facts order the candidates.
#[test]
fn an_opponents_target_decision_does_not_order_face_down_cards_by_identity() {
    let views = [[0, 1], [1, 0]].map(|order| {
        let mut game = Game::new(
            lands(
                board()
                    .permanent_or_hand("Thelonite Hermit", 0, 0, true)
                    .permanent_or_hand("Ainok Survivalist", 0, 0, true),
                "Forest",
                6,
            )
            .permanent_or_hand("Unsummon", 1, 1, true)
            .permanent_or_hand("Island", 1, 1, false)
            .prepare(),
        );
        for (card, lands) in order.into_iter().zip([[2, 3, 4], [5, 6, 7]]) {
            let card = Driver::object(&game, card);
            Driver::action(&mut game, Action::CastFaceDown { card });
            Driver::pay(&mut game, &lands);
            settle_stack(&mut game);
        }
        Driver::pass(&mut game);
        Driver::cast(&mut game, CardId(8));
        let targets = Driver::request(&mut game, u64::MAX);
        assert!(matches!(targets.reason, Reason::Targets { .. }));
        assert_eq!(targets.player, PlayerId(1));
        let opponent = view(&game, 1);
        assert!(opponent.decision.is_some());
        opponent
    });
    assert_eq!(views[0], views[1]);
}

/// CR 708.9: a player leaving the game reveals the face-down spells and
/// permanents they own to the remaining players, each named by that
/// viewer's own handle for it, so a viewer knows which card each was.
#[test]
fn a_departing_player_reveals_their_face_down_cards() {
    let hermit = lower(HERMIT);
    let ainok = lower(AINOK);
    let mut game = Game::new(
        lands(
            board()
                .seats(3)
                .permanent_or_hand("Thelonite Hermit", 0, 0, true)
                .permanent_or_hand("Ainok Survivalist", 0, 0, true),
            "Forest",
            6,
        )
        .prepare(),
    );
    for (card, lands) in [(0, [2, 3, 4]), (1, [5, 6, 7])] {
        let card = Driver::object(&game, card);
        Driver::action(&mut game, Action::CastFaceDown { card });
        Driver::pay(&mut game, &lands);
        settle_stack(&mut game);
    }
    let objects = [0, 1].map(|card| Driver::object(&game, card));
    let handles = [1, 2].map(|seat| objects.map(|o| game.handle_for(PlayerId(seat), o).unwrap()));
    assert!(view(&game, 1).view.revealed_face_down.is_empty());
    game.concede(PlayerId(0)).unwrap();
    Driver::settle(&mut game, u64::MAX);
    for (seat, [h, a]) in [1, 2].into_iter().zip(handles) {
        let mut expected = vec![
            RevealedFaceDown {
                handle: Some(h),
                owner: PlayerId(0),
                oracle_id: hermit.oracle_id.clone(),
            },
            RevealedFaceDown {
                handle: Some(a),
                owner: PlayerId(0),
                oracle_id: ainok.oracle_id.clone(),
            },
        ];
        expected.sort_by_key(|r| {
            let handle = r.handle.as_ref().unwrap().0.clone();
            (handle.len(), handle)
        });
        let mut seen = view(&game, seat).view.revealed_face_down;
        seen.sort_by_key(|r| {
            let handle = r.handle.as_ref().unwrap().0.clone();
            (handle.len(), handle)
        });
        assert_eq!(seen, expected);
    }
    game.validate_state().unwrap();
    assert_recoverable((&game).into());
}

/// CR 122.2, 708.8: a counter put on a face-down land stays when it turns
/// face up; snapshots accept a noncreature permanent with counters that was
/// a face-down creature, and recovery replays it.
#[test]
fn a_counter_on_a_face_down_land_survives_turning_face_up_and_recovers() {
    let mut game = Game::new(
        lands(
            board()
                .permanent_or_hand("Zoetic Cavern", 0, 0, true)
                .permanent_or_hand("Battlegrowth", 0, 0, true),
            "Forest",
            6,
        )
        .prepare(),
    );
    let cavern = cast_face_down(&mut game);
    Driver::cast(&mut game, CardId(1));
    Driver::pick_object(&mut game, cavern);
    Driver::pay(&mut game, &[5]);
    settle_stack(&mut game);
    assert_eq!(game.current(cavern).unwrap().plus_one_counters, 1.into());
    Driver::action(&mut game, Action::TurnFaceUp { object: cavern });
    Driver::pay(&mut game, &[6, 7]);
    let object = game.current(cavern).unwrap();
    assert!(!object.face_down);
    assert_eq!(object.plus_one_counters, 1.into());
    assert_eq!(game.characteristics(cavern).unwrap().kind, CardKind::Land);
    game.validate_state().unwrap();
    assert_eq!(restore(&game).state_digest(), game.state_digest());
    assert_recoverable((&game).into());
}

/// CR 601.2f, 702.37c: Develop casts face down only at the rules quote,
/// {3} plus increases such as Sphere of Resistance's.
#[test]
fn the_simulator_casts_face_down_only_at_the_quoted_cost() {
    use mtg_sim::policy::{Policy, Strategy};
    for taxed in [false, true] {
        let mut board = lands(
            board().permanent_or_hand("Thelonite Hermit", 0, 0, true),
            "Forest",
            3,
        );
        if taxed {
            board = board.permanent_or_hand("Sphere of Resistance", 1, 1, false);
        }
        let mut game = Game::new(board.prepare());
        Driver::request(&mut game, u64::MAX);
        let o = view(&game, 0);
        let d = o.decision.as_ref().unwrap();
        let card = game
            .handle_for(PlayerId(0), Driver::object(&game, 0))
            .unwrap();
        let cost = game.quote_face_down_cast_cost(PlayerId(0), &card).unwrap();
        assert_eq!(cost.generic, (3 + i64::from(taxed)).into());
        let expected = if taxed {
            Action::Pass
        } else {
            Action::CastFaceDown { card }
        };
        let mut strategy = Strategy::new(&game.prepared.catalog, Policy::Develop, [0; 4]);
        assert_eq!(
            strategy.choose_with_game(&o, &game).unwrap(),
            d.pick_one(&Candidate::Action { action: expected }).unwrap()
        );
    }
}

/// CR 708.5-708.6: attack and block declarations offering two face-down
/// creatures, cast in the same public order from swapped physical cards,
/// are the same decision for the attacker and the same observation for the
/// blocking opponent: slots and candidates follow public order alone.
#[test]
fn combat_declarations_do_not_order_face_down_cards_by_identity() {
    let runs = [[0, 1], [1, 0]].map(|order| {
        let mut game = Game::new(
            lands(
                board()
                    .permanent_or_hand("Thelonite Hermit", 0, 0, true)
                    .permanent_or_hand("Ainok Survivalist", 0, 0, true),
                "Forest",
                6,
            )
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .libraries("Island", 4)
            .prepare(),
        );
        for (card, lands) in order.into_iter().zip([[2, 3, 4], [5, 6, 7]]) {
            let card = Driver::object(&game, card);
            Driver::action(&mut game, Action::CastFaceDown { card });
            Driver::pay(&mut game, &lands);
            settle_stack(&mut game);
        }
        let attackers: Vec<_> = [0, 1].map(|card| Driver::object(&game, card)).to_vec();
        let (mut attack, mut block) = (None, None);
        for _ in 0..256 {
            let r = Driver::request(&mut game, u64::MAX);
            let answer = match &r.reason {
                Reason::Attackers
                    if r.player == PlayerId(0) && game.state.rules.turn.number > 1 =>
                {
                    attack = view(&game, 0).decision;
                    r.attack(attackers.iter().map(|a| (*a, PlayerId(1))).collect())
                }
                Reason::Attackers => r.attack(vec![]),
                Reason::Blockers => {
                    block = Some(view(&game, 1));
                    r.block(vec![])
                }
                Reason::Priority => mtg_witness::temporary::pass(&r),
                other => panic!("unexpected decision {other:?}"),
            };
            game.respond(r.player, r.id, answer).unwrap();
            if block.is_some() {
                break;
            }
        }
        (
            attack.expect("attack declaration"),
            block.expect("block declaration"),
        )
    });
    let [(attack_a, block_a), (attack_b, block_b)] = runs;
    assert_eq!(attack_a, attack_b);
    assert_eq!(block_a, block_b);
}
