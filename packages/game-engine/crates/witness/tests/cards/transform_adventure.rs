//! Transforming double-faced cards (CR 202.3b, 400.7, 701.27, 712.2,
//! 712.8, 712.14, 712.18) and adventurers (CR 715).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
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

const CAPTIVE: &str = record!("0272ca81-e727-4f4b-b06e-072d70bb5558");
const FARMER: &str = record!("60d591f6-6d84-45d2-9d78-a2ef07a3e403");
const TOME: &str = record!("b32253d6-5f00-40a8-a7e5-dc4655cd288c");
const YOUTH: &str = record!("7552a9b4-b82f-491f-ad52-e271cf730211");
const BORROWER: &str = record!("c7b044c3-3cfa-407e-bf20-2875e8e04b7b");
const RIMROCK: &str = record!("24db950b-e4fe-42d0-977b-4957eb8229d9");
const SMAUG: &str = record!("fc75cfec-e1f6-4cb5-8a3c-2f7b2c39c03f");
const HEIRLOOM: &str = record!("1374498c-58fd-40b9-b63b-f87fedc42e58");
const HEIR: &str = record!("d5b9c62d-fe98-4cbc-b4b6-012702bf6657");
const GOLLUM: &str = record!("0296d57e-e5b9-456f-bb21-eb584adefb4c");
const BOOMERANG: &str = record!("dc4a4996-108a-4aac-850f-2d9f76403446");

fn lower(raw: &str) -> Definition {
    match mtg_witness::compile(mtg_witness::R4_MANIFEST, raw) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        outcome => panic!("two-part witness did not lower: {outcome:?}"),
    }
}

fn board(records: &[&str]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for record in records {
        let definition = lower(record);
        if catalog
            .definition_by_oracle_id(&definition.oracle_id)
            .is_none()
        {
            catalog.definitions.push(definition);
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "transform-adventure")
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

fn row<'a>(view: &'a Observation, oracle_id: &str) -> Option<&'a VisibleObject> {
    view.view.objects.iter().find(|o| o.oracle_id == oracle_id)
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

/// Declare `attackers` against seat 1 and finish combat without blocks.
fn attack(game: &mut Game, attackers: &[ObjectId]) {
    for _ in 0..64 {
        let r = Driver::request(game, u64::MAX);
        if game.state.rules.turn.step == TurnStep::PostcombatMain {
            return;
        }
        let answer = match &r.reason {
            Reason::Attackers => r.attack(attackers.iter().map(|a| (*a, PlayerId(1))).collect()),
            Reason::Blockers => r.block(vec![]),
            Reason::Priority => mtg_witness::temporary::pass(&r),
            other => panic!("unexpected decision {other:?}"),
        };
        game.respond(r.player, r.id, answer).unwrap();
    }
    panic!("combat did not finish");
}

#[test]
fn transform_activation_turns_the_same_permanent_over_once_then_it_attacks() {
    let printed = lower(CAPTIVE);
    assert_eq!(printed.layout, FaceLayout::Transforming);
    assert!(
        printed
            .capabilities
            .contains(&Capability::TransformingDoubleFacedV1)
    );
    assert!(
        printed
            .capabilities
            .contains(&Capability::TransformSourceV1)
    );
    assert_eq!(
        mtg_contracts::kir::definition_tiers(&printed).unwrap(),
        mtg_contracts::kir::Tiers::EXECUTES
    );
    // CR 202.3b, 712.8e: no mana cost, but the front face's mana value.
    let back = printed.back_face.as_ref().unwrap();
    assert_eq!(back.name, "Ulvenwald Abomination");
    assert!(back.characteristics.mana_cost.colored.is_empty());
    assert_eq!(back.characteristics.mana_cost.generic, 0.into());
    assert_eq!(back.characteristics.printed_mana_value(), 2.into());
    assert!(back.characteristics.colors.is_empty());

    let board = lands(
        board(&[CAPTIVE, BOOMERANG]).permanent_or_hand("Ulvenwald Captive", 0, 0, false),
        "Forest",
        14,
    )
    .permanent_or_hand("Boomerang", 0, 0, true)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false);
    let mut game = Game::new(board.prepare());
    let captive = Driver::object(&game, 0);
    let life = game.state.rules.players[1].life.clone();
    assert!(
        game.copiable_definition(captive)
            .unwrap()
            .keywords
            .contains(&KeywordAbility::Defender)
    );

    // Two activations; the second resolves first and transforms.
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: captive,
            ability: 0,
        },
    );
    Driver::pay(&mut game, &[1, 2, 3, 4, 5, 6, 7]);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: captive,
            ability: 0,
        },
    );
    Driver::pay(&mut game, &[8, 9, 10, 11, 12, 13, 14]);
    assert_eq!(game.state.rules.stack.len(), 2);
    assert!(game.state.rules.stack.iter().all(|entry| {
        entry.source == captive
            && entry.source_face == CardFace::Front
            && entry.source_transforms == 0
    }));
    while game.state.rules.stack.len() == 2 {
        Driver::pass(&mut game);
        Driver::request(&mut game, u64::MAX);
    }
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(game.current(captive).unwrap().transforms, 1);
    // The remaining activation is still the front face's ability
    // (CR 113.7a) and its transform is ignored (CR 701.27f).
    assert_recoverable((&game).into());
    let restored = restore(&game);
    assert_eq!(restored.state_digest(), game.state_digest());
    settle_stack(&mut game);
    assert_eq!(game.state.rules.stack.len(), 0);

    // CR 712.18: the same object with its back face up.
    assert_eq!(Driver::object(&game, 0), captive);
    let transformed = game.current(captive).unwrap();
    assert_eq!(transformed.transforms, 1);
    assert_eq!(transformed.face(), CardFace::Back);
    assert_eq!(
        game.copiable_definition(captive).unwrap().name,
        "Ulvenwald Abomination"
    );
    let characteristics = game.characteristics(captive).unwrap();
    assert_eq!(characteristics.power, Some(4.into()));
    assert_eq!(characteristics.toughness, Some(6.into()));
    assert_eq!(characteristics.printed_mana_value(), 2.into());
    assert!(characteristics.colors.is_empty());
    assert!(
        !game
            .copiable_definition(captive)
            .unwrap()
            .keywords
            .contains(&KeywordAbility::Defender)
    );
    for seat in [0, 1] {
        let observed = view(&game, seat);
        let faces = row(&observed, &printed.oracle_id)
            .unwrap()
            .faces
            .as_ref()
            .unwrap();
        assert_eq!(faces.layout, FaceLayout::Transforming);
        assert_eq!(faces.front_name, "Ulvenwald Captive");
        assert_eq!(faces.back_name, "Ulvenwald Abomination");
        assert_eq!(faces.active_face, CardFace::Back);
    }
    assert_recoverable((&game).into());

    // The back face has no defender: it attacks for 4.
    attack(&mut game, &[captive]);
    let mut expected = life;
    expected.subtract(&4.into());
    assert_eq!(game.state.rules.players[1].life, expected);

    // CR 400.7, 712.14: a zone change makes a new front-face object.
    Driver::cast(&mut game, CardId(15));
    Driver::pick_object(&mut game, captive);
    Driver::pay(&mut game, &[16, 17]);
    settle_stack(&mut game);
    let returned = Driver::object(&game, 0);
    assert_ne!(returned, captive);
    let object = game.current(returned).unwrap();
    assert_eq!(object.zone, Zone::Hand { owner: PlayerId(0) });
    assert_eq!(object.transforms, 0);
    assert_eq!(
        game.copiable_definition(returned).unwrap().name,
        "Ulvenwald Captive"
    );
    assert_eq!(
        row(&view(&game, 0), &printed.oracle_id)
            .unwrap()
            .faces
            .as_ref()
            .unwrap()
            .active_face,
        CardFace::Front
    );
    assert!(row(&view(&game, 1), &printed.oracle_id).is_none());
}

#[test]
fn transform_trigger_turns_the_permanent_over_and_the_back_face_attacks() {
    let printed = lower(FARMER);
    let mut game = Game::new(
        board(&[FARMER])
            .permanent_or_hand("Desperate Farmer", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Lightning Bolt", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false)
            .prepare(),
    );
    let farmer = Driver::object(&game, 0);
    let bears = Driver::object(&game, 1);
    Driver::cast(&mut game, CardId(2));
    Driver::pick_object(&mut game, bears);
    Driver::pay(&mut game, &[3]);
    settle_stack(&mut game);
    assert_eq!(game.current(farmer).unwrap().transforms, 1);
    assert_eq!(
        game.copiable_definition(farmer).unwrap().name,
        "Depraved Harvester"
    );
    let characteristics = game.characteristics(farmer).unwrap();
    assert_eq!(characteristics.power, Some(4.into()));
    assert_eq!(characteristics.toughness, Some(3.into()));
    assert_eq!(characteristics.printed_mana_value(), 3.into());
    assert_eq!(
        row(&view(&game, 1), &printed.oracle_id)
            .unwrap()
            .faces
            .as_ref()
            .unwrap()
            .active_face,
        CardFace::Back
    );
    let mut life = game.state.rules.players[0].life.clone();
    attack(&mut game, &[farmer]);
    // Lifelink from the back face (CR 702.15b).
    life.add(&4.into());
    assert_eq!(game.state.rules.players[0].life, life);
    assert_recoverable((&game).into());
}

/// Pass priority until `seat`'s next precombat main phase.
fn next_main(game: &mut Game, seat: u8) {
    // Every seat needs a library to reach its own draw step.
    let turn = game.state.rules.turn.number;
    for _ in 0..256 {
        let r = Driver::request(game, u64::MAX);
        if game.state.rules.turn.number > turn
            && game.state.rules.turn.step == TurnStep::PrecombatMain
            && game.state.rules.active == Some(PlayerId(seat))
            && r.reason == Reason::Priority
        {
            return;
        }
        let answer = match &r.reason {
            Reason::Attackers => r.attack(vec![]),
            _ => mtg_witness::temporary::pass(&r),
        };
        game.respond(r.player, r.id, answer).unwrap();
    }
    panic!("no next main phase");
}

#[test]
fn transform_activation_from_the_back_face_turns_it_front_up_again() {
    let mut game = Game::new(
        lands(
            board(&[TOME]).permanent_or_hand("Mysterious Tome", 0, 0, false),
            "Island",
            3,
        )
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .seats(4)
        .libraries("Island", 2)
        .prepare(),
    );
    let tome = Driver::object(&game, 0);
    let bears = Driver::object(&game, 4);
    let hand = |game: &Game| {
        game.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
            .count()
    };
    let before = hand(&game);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: tome,
            ability: 0,
        },
    );
    Driver::pay(&mut game, &[1, 2]);
    settle_stack(&mut game);
    let object = game.current(tome).unwrap();
    assert_eq!(object.face(), CardFace::Back);
    assert!(object.tapped);
    assert_eq!(hand(&game), before + 1);
    assert_eq!(
        game.copiable_definition(tome).unwrap().name,
        "Chilling Chronicle"
    );
    // CR 202.3b: the back face's mana value is the front's {2}{U}.
    assert_eq!(
        game.characteristics(tome).unwrap().printed_mana_value(),
        3.into()
    );

    // On the next turn the back face's own ability taps a permanent and
    // transforms it front face up again.
    next_main(&mut game, 0);
    assert_eq!(game.current(tome).unwrap().transforms, 1);
    assert!(!game.current(tome).unwrap().tapped);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: tome,
            ability: 0,
        },
    );
    Driver::pick_object(&mut game, bears);
    Driver::pay(&mut game, &[3]);
    assert_eq!(game.state.rules.stack[0].source_face, CardFace::Back);
    assert_eq!(game.state.rules.stack[0].source_transforms, 1);
    settle_stack(&mut game);
    assert!(game.current(bears).unwrap().tapped);
    let object = game.current(tome).unwrap();
    assert_eq!(object.transforms, 2);
    assert_eq!(object.face(), CardFace::Front);
    assert_eq!(
        game.copiable_definition(tome).unwrap().name,
        "Mysterious Tome"
    );
    assert_recoverable((&game).into());
}

#[test]
fn once_each_turn_limit_belongs_to_the_activated_face() {
    let mut game = Game::new(
        board(&[HEIR])
            .permanent_or_hand("Heir of Falkenrath", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, true)
            .prepare(),
    );
    let heir = Driver::object(&game, 0);
    let discard = Driver::object(&game, 1);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: heir,
            ability: 0,
        },
    );
    Driver::pick_object(&mut game, discard);
    settle_stack(&mut game);
    // CR 602.5b, 712.18: the front face's limited ability was used; the
    // back face has its own abilities and the state stays valid.
    let object = game.current(heir).unwrap();
    assert_eq!(object.face(), CardFace::Back);
    assert!(
        game.state
            .rules
            .activation_uses
            .iter()
            .any(|u| { u.source == heir && u.face == CardFace::Front && u.ability == 0 })
    );
    assert!(game.has_keyword(heir, KeywordAbility::Flying));
    assert_eq!(game.characteristics(heir).unwrap().power, Some(3.into()));
    assert_recoverable((&game).into());
    restore(&game);
    let life = game.state.rules.players[1].life.clone();
    attack(&mut game, &[heir]);
    let mut expected = life;
    expected.subtract(&3.into());
    assert_eq!(game.state.rules.players[1].life, expected);
}

#[test]
fn optional_upkeep_transform_survives_restore_with_both_seats_views() {
    for accept in [true, false] {
        let printed = lower(YOUTH);
        let mut game = Game::new(
            board(&[YOUTH])
                .step(TurnStep::Untap)
                .permanent_or_hand("Cloistered Youth", 0, 0, false)
                .libraries("Island", 3)
                .prepare(),
        );
        let youth = Driver::object(&game, 0);
        let mut choice = None;
        for _ in 0..16 {
            let r = Driver::request(&mut game, u64::MAX);
            if r.reason == Reason::OptionalEffect {
                choice = Some(r);
                break;
            }
            let answer = mtg_witness::temporary::pass(&r);
            game.respond(r.player, r.id, answer).unwrap();
        }
        let choice = choice.expect("upkeep transform choice");
        assert_eq!(choice.player, PlayerId(0));
        let before = [view(&game, 0), view(&game, 1)];
        let mut game = restore(&game);
        let owner = view(&game, 0);
        let opponent = view(&game, 1);
        assert_eq!(owner, before[0]);
        assert_eq!(opponent, before[1]);
        assert_eq!(
            owner.decision.as_ref().unwrap().reason,
            Reason::OptionalEffect
        );
        assert!(opponent.decision.is_none());
        assert!(opponent.decision_context.is_none());
        // Both seats see the front face of the battlefield permanent.
        for observed in [&owner, &opponent] {
            let faces = row(observed, &printed.oracle_id)
                .unwrap()
                .faces
                .as_ref()
                .unwrap();
            assert_eq!(faces.active_face, CardFace::Front);
        }
        let source = owner.decision_context.unwrap().source.unwrap();
        assert_eq!(source.oracle_id, printed.oracle_id);
        assert_eq!(
            source.text,
            "At the beginning of your upkeep, you may transform this creature."
        );
        let flag = if accept { Flag::Accept } else { Flag::Decline };
        Driver::answer(
            &mut game,
            1,
            choice.pick_one(&Candidate::Flag { flag }).unwrap(),
        );
        Driver::request(&mut game, u64::MAX);
        assert_eq!(game.current(youth).unwrap().transforms, u32::from(accept));
        // Play to the end step: only Unholy Fiend's trigger costs 1 life.
        let mut life = game.state.rules.players[0].life.clone();
        for _ in 0..64 {
            let r = Driver::request(&mut game, u64::MAX);
            if game.state.rules.turn.step == TurnStep::Cleanup || game.state.rules.turn.number > 1 {
                break;
            }
            let answer = match &r.reason {
                Reason::Attackers => r.attack(vec![]),
                _ => mtg_witness::temporary::pass(&r),
            };
            game.respond(r.player, r.id, answer).unwrap();
        }
        if accept {
            life.subtract(&1.into());
        }
        assert_eq!(game.state.rules.players[0].life, life);
    }
}

#[test]
fn adventure_resolves_to_exile_and_its_creature_is_cast_from_exile() {
    let printed = lower(BORROWER);
    assert_eq!(printed.layout, FaceLayout::Adventurer);
    assert!(printed.capabilities.contains(&Capability::AdventurerV1));
    let adventure = printed.back_face.as_ref().unwrap();
    assert_eq!(adventure.name, "Petty Theft");
    assert_eq!(adventure.characteristics.kind, CardKind::Instant);
    assert_eq!(adventure.characteristics.subtypes, ["Adventure"]);
    assert_eq!(adventure.characteristics.colors, [Color::U]);
    assert_eq!(printed.characteristics.colors, [Color::U]);
    assert_eq!(
        mtg_contracts::kir::definition_tiers(&printed).unwrap(),
        mtg_contracts::kir::Tiers::EXECUTES
    );

    let mut game = Game::new(
        lands(
            board(&[BORROWER]).permanent_or_hand("Brazen Borrower", 0, 0, true),
            "Island",
            5,
        )
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .prepare(),
    );
    let card = Driver::object(&game, 0);
    let bears = Driver::object(&game, 6);
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(can(&decision, Action::Cast { card }));
    assert!(can(&decision, Action::CastBackFace { card }));
    assert!(!can(&decision, Action::PlayLand { object: card }));
    // The adventurer in hand is hidden from the opponent (CR 712.7 analog
    // for a hidden zone: no name or face is published).
    assert!(row(&view(&game, 1), &printed.oracle_id).is_none());
    assert_eq!(
        row(&view(&game, 0), &printed.oracle_id)
            .unwrap()
            .faces
            .as_ref()
            .unwrap()
            .active_face,
        CardFace::Front
    );

    Driver::action(&mut game, Action::CastBackFace { card });
    // A face-related decision: the target of the Adventure spell.
    let targets = Driver::request(&mut game, u64::MAX);
    assert!(matches!(targets.reason, Reason::Targets { .. }));
    let before = [view(&game, 0), view(&game, 1)];
    let mut game = restore(&game);
    let owner = view(&game, 0);
    let opponent = view(&game, 1);
    assert_eq!(owner, before[0]);
    assert_eq!(opponent, before[1]);
    assert!(opponent.decision.is_none());
    assert!(opponent.decision_context.is_none());
    let source = owner
        .decision_context
        .as_ref()
        .unwrap()
        .source
        .as_ref()
        .unwrap();
    assert_eq!(
        source.text,
        "Return target nonland permanent an opponent controls to its owner's hand."
    );
    // CR 715.3b: on the stack only the Adventure's characteristics exist.
    for observed in [&owner, &opponent] {
        let spell = row(observed, &printed.oracle_id).unwrap();
        assert_eq!(spell.zone, Zone::Stack);
        let faces = spell.faces.as_ref().unwrap();
        assert_eq!(faces.active_face, CardFace::Back);
        assert_eq!(faces.back_name, "Petty Theft");
        let c = spell.characteristics.as_ref().unwrap();
        assert_eq!(c.kind, CardKind::Instant);
        assert_eq!(c.printed_mana_value(), 2.into());
    }
    Driver::pick_object(&mut game, bears);
    Driver::pay(&mut game, &[1, 2]);
    assert_eq!(view(&game, 1).view.stack[0].mana_value, Some(2.into()));
    settle_stack(&mut game);
    assert_eq!(
        game.current(Driver::object(&game, 6)).unwrap().zone,
        Zone::Hand { owner: PlayerId(1) }
    );

    // CR 715.3d: exiled with permission; the opponent sees it on an adventure.
    let exiled = Driver::object(&game, 0);
    let object = game.current(exiled).unwrap();
    assert_eq!(object.zone, Zone::Exile);
    assert_eq!(object.id.face, CardFace::Front);
    assert_eq!(object.adventure_player, Some(PlayerId(0)));
    for seat in [0, 1] {
        let observed = view(&game, seat);
        let faces = row(&observed, &printed.oracle_id)
            .unwrap()
            .faces
            .as_ref()
            .unwrap();
        assert!(faces.on_adventure);
        assert_eq!(faces.active_face, CardFace::Front);
    }
    assert_recoverable((&game).into());
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(can(&decision, Action::Cast { card: exiled }));
    assert!(!can(&decision, Action::CastBackFace { card: exiled }));
    Driver::action(&mut game, Action::Cast { card: exiled });
    Driver::pay(&mut game, &[3, 4, 5]);
    settle_stack(&mut game);
    let creature = Driver::object(&game, 0);
    let object = game.current(creature).unwrap();
    assert_eq!(object.zone, Zone::Battlefield);
    assert_eq!(object.adventure_player, None);
    let characteristics = game.characteristics(creature).unwrap();
    assert_eq!(characteristics.kind, CardKind::Creature);
    assert_eq!(characteristics.power, Some(3.into()));
    assert!(game.has_keyword(creature, KeywordAbility::Flying));
    assert_recoverable((&game).into());
}

#[test]
fn adventure_that_does_not_resolve_goes_to_the_graveyard() {
    let mut game = Game::new(
        board(&[RIMROCK])
            .permanent_or_hand("Rimrock Knight", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Lightning Bolt", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false)
            .prepare(),
    );
    let card = Driver::object(&game, 0);
    let bears = Driver::object(&game, 2);
    Driver::action(&mut game, Action::CastBackFace { card });
    Driver::pick_object(&mut game, bears);
    Driver::pay(&mut game, &[1]);
    // In response, the only target dies; the Adventure does not resolve.
    Driver::cast(&mut game, CardId(3));
    Driver::pick_object(&mut game, bears);
    Driver::pay(&mut game, &[4]);
    settle_stack(&mut game);
    let card = Driver::object(&game, 0);
    let object = game.current(card).unwrap();
    assert_eq!(object.zone, Zone::Graveyard { owner: PlayerId(0) });
    assert_eq!(object.adventure_player, None);
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(!can(&decision, Action::Cast { card }));
}

#[test]
fn adventurer_commander_casts_its_adventure_from_the_command_zone() {
    let printed = lower(SMAUG);
    assert!(printed.characteristics.legendary);
    let source: serde_json::Value = serde_json::from_str(&printed.provenance.source_json).unwrap();
    let facts = mtg_contracts::bulk_catalog::BulkCardFacts::from_source(&source).unwrap();
    assert!(facts.legendary_creature);
    let mut game = Game::new(
        lands(
            board(&[SMAUG])
                .card(
                    "Smaug, the Great Calamity",
                    0,
                    0,
                    Zone::Command { owner: PlayerId(0) },
                )
                .commander(CardId(0)),
            "Mountain",
            5,
        )
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .prepare(),
    );
    let smaug = Driver::object(&game, 0);
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(can(&decision, Action::CastBackFace { card: smaug }));
    Driver::action(&mut game, Action::CastBackFace { card: smaug });
    let bears = Driver::object(&game, 6);
    Driver::pick_object(&mut game, bears);
    Driver::pay(&mut game, &[1, 2, 3, 4, 5]);
    for _ in 0..16 {
        let r = Driver::request(&mut game, u64::MAX);
        let answer = match &r.reason {
            // CR 903.9a: keep the commander in exile on its adventure.
            Reason::CommanderReturn { .. } => r
                .pick_one(&Candidate::Flag {
                    flag: Flag::Destination,
                })
                .unwrap(),
            Reason::Priority if game.state.rules.stack.is_empty() => break,
            _ => mtg_witness::temporary::pass(&r),
        };
        game.respond(r.player, r.id, answer).unwrap();
    }
    assert_eq!(
        game.current(Driver::object(&game, 6)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    let exiled = Driver::object(&game, 0);
    assert_eq!(game.current(exiled).unwrap().zone, Zone::Exile);
    assert_eq!(
        game.current(exiled).unwrap().adventure_player,
        Some(PlayerId(0))
    );
}

#[test]
fn two_part_layouts_outside_this_checkpoint_reject_by_name() {
    let cases = [
        ("split", record!("03d3fee8-f7de-4152-8c18-6d279cc6c125")),
        ("prototype", record!("02457c7f-eced-4bf3-9b61-309b4ceab2d5")),
        ("flip", record!("0664dcc8-fa68-4802-936f-ecd2979ebf3c")),
        ("meld", record!("022b42a6-ee0f-4080-b973-c1b68938c3be")),
    ];
    for (layout, raw) in cases {
        let mtg_compiler::CompileResult::Unsupported { path, reason } =
            mtg_witness::compile(mtg_witness::R4_MANIFEST, raw)
        else {
            panic!("{layout} did not reject");
        };
        assert_eq!(path, "source.layout");
        assert_eq!(reason, format!("layout:{layout}"));
    }
    // A trigger on a transform event has no executor yet.
    assert!(matches!(
        mtg_witness::compile(mtg_witness::R4_MANIFEST, HEIRLOOM),
        mtg_compiler::CompileResult::Unsupported { .. }
    ));
    // Gollum's Adventure passes the layout gate but not its own text.
    assert!(matches!(
        mtg_witness::compile(mtg_witness::R4_MANIFEST, GOLLUM),
        mtg_compiler::CompileResult::Unsupported { .. }
    ));
}

#[test]
fn two_part_snapshot_rejects_impossible_transform_and_adventure_state() {
    let valid = Game::new(
        board(&[CAPTIVE, BORROWER])
            .permanent_or_hand("Ulvenwald Captive", 0, 0, false)
            .permanent_or_hand("Brazen Borrower", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .card("Island", 0, 0, Zone::Exile)
            .prepare(),
    );
    type Forge = fn(&mut Object);
    let cases: [(u32, Forge, &str); 7] = [
        // A nontransforming card or a transforming card off the battlefield.
        (2, |o| o.transforms = 1, "impossible transformed object"),
        (
            0,
            |o| {
                o.transforms = 1;
                o.zone = Zone::Graveyard { owner: PlayerId(0) };
            },
            "impossible transformed object",
        ),
        // A transforming card is never put onto the battlefield transformed.
        (0, |o| o.id.face = CardFace::Back, "impossible back face"),
        // An Adventure is never on the battlefield or in a hand.
        (1, |o| o.id.face = CardFace::Back, "impossible back face"),
        // Adventure permission only on an exiled adventurer.
        (
            1,
            |o| o.adventure_player = Some(PlayerId(0)),
            "impossible adventure permission",
        ),
        (
            3,
            |o| o.adventure_player = Some(PlayerId(0)),
            "impossible adventure permission",
        ),
        (
            1,
            |o| {
                o.zone = Zone::Exile;
                o.adventure_player = Some(PlayerId(7));
            },
            "impossible adventure permission",
        ),
    ];
    for (id, forge, message) in cases {
        let mut forged = valid.fork();
        forge(forged.state.rules.objects.get_mut(&CardId(id)).unwrap());
        let snapshot = forged.snapshot().unwrap();
        let Err(error) = Game::restore(forged.prepared.as_ref().clone(), &snapshot) else {
            panic!("accepted impossible two-part state on object {id}: {message}");
        };
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn transform_ability_snapshot_rejects_impossible_faces() {
    let mut game = Game::new(
        lands(
            board(&[CAPTIVE]).permanent_or_hand("Ulvenwald Captive", 0, 0, false),
            "Forest",
            7,
        )
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .prepare(),
    );
    let captive = Driver::object(&game, 0);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: captive,
            ability: 0,
        },
    );
    Driver::pay(&mut game, &[1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(game.state.rules.stack.len(), 1);
    // The source has not transformed as many times as the baseline says.
    let mut forged = game.fork();
    forged.state.rules.stack[0].source_transforms = 2;
    let snapshot = forged.snapshot().unwrap();
    let Err(error) = Game::restore(forged.prepared.as_ref().clone(), &snapshot) else {
        panic!("accepted an impossible transform baseline");
    };
    assert!(error.contains("impossible ability face"), "{error}");
}

const PIGEONS: &str = record!("2d30a634-43c6-4e78-ba10-7ef7ee17ac40");

/// No frozen transforming card creates a delayed trigger at this checkpoint.
/// This test-only layout pairs two exact R4 faces: Carrier Pigeons, whose
/// enters trigger creates one, and Ulvenwald Abomination, which has none.
fn delayed_transform_record() -> String {
    use serde_json::{Value, json};
    let mut card: Value = serde_json::from_str(PIGEONS).unwrap();
    let captive: Value = serde_json::from_str(CAPTIVE).unwrap();
    let mut front = card["source"].clone();
    for key in ["layout", "card_faces"] {
        front.as_object_mut().unwrap().remove(key);
    }
    let back = captive["source"]["card_faces"][1].clone();
    let source = card["source"].as_object_mut().unwrap();
    for key in ["oracle_text", "mana_cost", "power", "toughness", "colors"] {
        source.remove(key);
    }
    source.insert("layout".into(), json!("transform"));
    source.insert(
        "name".into(),
        json!("Carrier Pigeons // Ulvenwald Abomination"),
    );
    source.insert("card_faces".into(), json!([front, back]));
    let mut back_parse = captive["faces"][1].clone();
    back_parse["sourceFaceIndex"] = json!(1);
    card["faces"].as_array_mut().unwrap().push(back_parse);
    let mut back_resolution = captive["resolution"]["faces"][1].clone();
    back_resolution["sourceFaceIndex"] = json!(1);
    card["resolution"]["faces"]
        .as_array_mut()
        .unwrap()
        .push(back_resolution);
    let abilities = json!([
        card["faces"][0]["result"]["abilities"],
        card["faces"][1]["result"]["abilities"],
    ]);
    card["astSha256"] = json!(sha256(abilities.to_string().as_bytes()));
    for (key, hash) in [
        ("source", "sourceSha256"),
        ("resolution", "resolutionSha256"),
        ("binding", "bindingSha256"),
    ] {
        card[hash] = json!(sha256(card[key].to_string().as_bytes()));
    }
    card.to_string()
}

#[test]
fn delayed_trigger_keeps_the_face_that_created_it() {
    let raw = delayed_transform_record();
    let printed = lower(&raw);
    assert_eq!(printed.layout, FaceLayout::Transforming);
    let mut setup = board(&[&raw])
        .permanent_or_hand("Carrier Pigeons", 0, 0, true)
        .libraries("Plains", 2);
    for _ in 0..4 {
        setup = setup.permanent_or_hand("Plains", 0, 0, false);
    }
    let mut game = Game::new(setup.prepare());
    let card = Driver::object(&game, 0);
    Driver::action(&mut game, Action::Cast { card });
    Driver::pay(&mut game, &[5, 6, 7, 8]);
    // Resolve the creature spell, then its enters trigger, which creates a
    // delayed trigger for the next turn's upkeep (CR 603.7).
    settle_stack(&mut game);
    let pigeons = Driver::object(&game, 0);
    assert_eq!(game.current(pigeons).unwrap().zone, Zone::Battlefield);
    assert_eq!(game.state.rules.resolution_triggers.len(), 1);
    // Private fixture action: no admitted ability transforms this test-only
    // card. Turn it over before the delayed trigger fires (CR 712.18).
    game.state
        .rules
        .objects
        .get_mut(&pigeons.card)
        .unwrap()
        .transforms = 1;
    assert_eq!(
        game.copiable_definition(pigeons).unwrap().name,
        "Ulvenwald Abomination"
    );
    // Replay cannot reproduce the fixture action; state validation checks
    // the delayed trigger's recorded face and baseline instead.
    game.validate_state().unwrap();
    let hand = |game: &Game| {
        game.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
            .count()
    };
    let before = hand(&game);
    for _ in 0..64 {
        let r = Driver::request(&mut game, u64::MAX);
        if game.state.rules.turn.number == 2 && game.state.rules.turn.step == TurnStep::Draw {
            break;
        }
        let answer = match &r.reason {
            Reason::Attackers => r.attack(vec![]),
            _ => mtg_witness::temporary::pass(&r),
        };
        game.respond(r.player, r.id, answer).unwrap();
    }
    // CR 113.7a: the front face's delayed draw still resolves for its creator.
    assert_eq!(hand(&game), before + 1);
    game.validate_state().unwrap();
}
