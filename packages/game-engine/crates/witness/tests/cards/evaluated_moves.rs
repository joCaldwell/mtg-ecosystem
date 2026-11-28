//! Frozen R5 zone changes performed by the shared evaluator.
//! CR 110.2a, 400.3, 400.7, 401.4, 603.2, 608.2b-c, 608.2h, 701.24a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{self, Cell, EvaluatedVerb, definition_cells};
use mtg_contracts::{Definition, Instruction};
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

const BOOMERANG: &str = record!("dc4a4996-108a-4aac-850f-2d9f76403446");
const SWORDS: &str = record!("b1544f21-7e98-461b-aed5-e748b0168c52");
const REANIMATE: &str = record!("a044474a-cd72-4e9d-bd8d-a08f2de9cdc0");
const EXCLUSION_MAGE: &str = record!("fd28fe55-36cd-4242-8dd4-edb58fbb9895");
const RUN_AGROUND: &str = record!("6ef9bf18-6b09-4398-a502-6867d999ab0c");
const NATURAL_OBSOLESCENCE: &str = record!("fbafbe3a-3980-4457-a77b-3f72df7b3329");
const OBLATION: &str = record!("860e744d-899c-43c7-84bb-d52c69d2f45a");
const MALANTHROPE: &str = record!("0f99e921-61e7-4fc8-8115-4162712b4737");
/// "When this creature enters, put target card from an opponent's
/// graveyard on the bottom of their library." (frozen R5 line 3110)
const LANDSCAPER_COLOS: &str = record!("144bef8b-3583-4e98-9285-a1965e7e17f8");
/// "This creature enters with three +1/+1 counters on it."
const FAITHFUL_WATCHDOG: &str = record!("139fb542-89fd-4b9e-87e6-ec925bcca93a");
/// "If one or more +1/+1 counters would be put on a creature you control,
/// that many plus one +1/+1 counters are put on it instead."
const HARDENED_SCALES: &str = record!("a1f3da21-af6d-450e-bf0b-985d158418e6");
const CLOUDSHIFT: &str = record!("6879f5ce-7a1b-4606-bad1-885779b0d456");
const FLICKER: &str = record!("19ea326f-45ff-4bc0-a957-7d3f22dd8c6f");
const GHOSTLY_FLICKER: &str = record!("ad0070db-6454-41cb-861f-8f5b8fc2a3b8");
const RESTORATION_ANGEL: &str = record!("dfbd3afc-9905-4cff-a4f4-df08a4d0a7fa");
const MIRACULOUS_RECOVERY: &str = record!("cea73a4b-e10a-4517-a92b-24a08321009f");
const CHAOS_WARP: &str = record!("07a0cba9-8768-4fd9-a3d5-b0f83b4bf8e8");
const COILING_ORACLE: &str = record!("69fd4ddf-9ed8-4c56-bef3-9944daf05e4f");
const SATYR_WAYFINDER: &str = record!("ead4c01a-e2ae-49c9-95d9-651d81005907");
const GRISLY_SALVAGE: &str = record!("a89ebf63-63c9-42cd-98b4-ec852cd9b0a8");
const RASHMI: &str = record!("01a79429-7d57-4a51-9815-a20905ed4707");
const HOARD_SMELTER: &str = record!("cf9db2be-3c95-45a2-ab85-694e3bd3bf1d");

const MOVED: Cell = Cell::Evaluated(EvaluatedVerb::Move);

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen evaluated move witness failed: {result:?}");
    };
    *definition
}

/// Each frozen card compiles and every one of its moves occupies the
/// evaluated cell or one of the two reviewed higher-tier labels.
fn board(cards: &[&str], seats: u8) -> Board {
    let moves = cards.iter().map(|raw| lowered(raw)).collect();
    board_of(moves, &[], seats)
}

/// `moves` carry a move; `others` are frozen cards without one.
fn board_of(moves: Vec<Definition>, others: &[&str], seats: u8) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for definition in moves {
        let cells = definition_cells(&definition).unwrap();
        assert!(
            cells.contains(&MOVED)
                || cells.contains(&Cell::MoveTargetOwnerHand)
                || cells.contains(&Cell::MoveTargetOwnerLibraryTop),
            "{}: {cells:?}",
            definition.name
        );
        catalog.definitions.push(definition);
    }
    catalog
        .definitions
        .extend(others.iter().map(|raw| lowered(raw)));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r5-evaluated-moves")
        .step(TurnStep::PrecombatMain)
        .seats(seats)
        .seed([41; 32])
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

fn observation(g: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("missing observation for seat {seat}");
    };
    o
}

/// Pass priority until the stack is empty, recovering at the end.
fn settle(g: &mut Game) {
    for _ in 0..32 {
        let d = Driver::request(g, 1);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        if g.state.rules.stack.is_empty() {
            checked(g);
            assert_recoverable((&*g).into());
            return;
        }
        Driver::pass(g);
    }
    panic!("no empty-stack priority");
}

/// Pass priority until a non-priority decision.
fn next_choice(g: &mut Game) -> Decision<Rules> {
    for _ in 0..32 {
        let d = Driver::request(g, 1);
        if d.reason != Reason::Priority {
            checked(g);
            return d;
        }
        Driver::pass(g);
    }
    panic!("no resolution choice");
}

fn cast_at(g: &mut Game, card: u32, target: u32, lands: &[u32]) {
    Driver::cast(g, CardId(card));
    Driver::pick_object(g, Driver::object(g, target));
    Driver::pay(g, lands);
}

fn library(g: &Game, seat: u8) -> Vec<CardId> {
    g.state.rules.libraries[&PlayerId(seat)].clone()
}

fn known_library(g: &Game, viewer: u8) -> Vec<String> {
    observation(g, viewer)
        .view
        .objects
        .iter()
        .filter(|o| matches!(o.zone, Zone::Library { .. }))
        .map(|o| o.oracle_id.clone())
        .collect()
}

#[test]
fn actual_movement_result_reenters_under_the_printed_controller() {
    for (raw, spell, controller, mana) in [
        (CLOUDSHIFT, "Cloudshift", PlayerId(0), &[2][..]),
        (FLICKER, "Flicker", PlayerId(1), &[2, 3][..]),
    ] {
        let mut game = Game::new(
            board(&[raw], 4)
                .permanent_or_hand(spell, 0, 0, true)
                .permanent_or_hand("Grizzly Bears", 1, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .prepare(),
        );
        let prior = Driver::object(&game, 1);
        cast_at(&mut game, 0, 1, mana);
        settle(&mut game);
        let returned = Driver::object(&game, 1);
        assert_eq!(returned.incarnation, prior.incarnation + 2);
        assert_eq!(game.zone(returned), Some(Zone::Battlefield));
        assert_eq!(
            game.state.rules.objects[&returned.card].controller,
            controller
        );
    }
}

#[test]
fn declining_restoration_angel_does_not_perform_either_move() {
    let mut game = Game::new(
        board(&[RESTORATION_ANGEL], 4)
            .permanent_or_hand("Restoration Angel", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .prepare(),
    );
    let prior = Driver::object(&game, 1);
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[2, 3, 4, 5]);
    assert!(matches!(
        next_choice(&mut game).reason,
        Reason::Targets { .. }
    ));
    Driver::pick_object(&mut game, prior);
    let ask = next_choice(&mut game);
    assert_eq!(ask.reason, Reason::OptionalEffect);
    assert_recoverable((&game).into());
    let mut accepted =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    for (game, flag) in [(&mut game, Flag::Decline), (&mut accepted, Flag::Accept)] {
        game.respond(
            ask.player,
            ask.id,
            ask.pick_one(&Candidate::Flag { flag }).unwrap(),
        )
        .unwrap();
        settle(game);
    }
    assert_eq!(Driver::object(&game, 1), prior);
    assert_eq!(
        Driver::object(&accepted, 1).incarnation,
        prior.incarnation + 2
    );
}

#[test]
fn the_plural_result_tracks_both_successors() {
    let mut game = Game::new(
        board(&[GHOSTLY_FLICKER], 4)
            .permanent_or_hand("Ghostly Flicker", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    let prior = [Driver::object(&game, 1), Driver::object(&game, 2)];
    Driver::cast(&mut game, CardId(0));
    let ask = Driver::request(&mut game, 1);
    let picks = prior
        .iter()
        .map(|&object| ask.index(&Candidate::Object { object }).unwrap())
        .collect();
    game.respond(ask.player, ask.id, Answer::Pick { picks: vec![picks] })
        .unwrap();
    Driver::pay(&mut game, &[3, 4, 5]);
    settle(&mut game);
    for old in prior {
        let next = game.object_id(old.card).unwrap();
        assert_eq!(next.incarnation, old.incarnation + 2);
        assert_eq!(game.zone(next), Some(Zone::Battlefield));
    }
}

#[test]
fn a_later_pump_measures_the_destroyed_targets_mana_value() {
    let mut game = Game::new(
        board_of(vec![], &[HOARD_SMELTER], 4)
            .permanent_or_hand("Hoard-Smelter Dragon", 0, 0, false)
            .permanent_or_hand("Sol Ring", 1, 1, false)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .prepare(),
    );
    let source = Driver::object(&game, 0);
    let target = Driver::object(&game, 1);
    let mut expected = game.characteristics(source).unwrap().power.unwrap();
    expected.add(&game.characteristics(target).unwrap().printed_mana_value());
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[2, 3, 4, 5]);
    settle(&mut game);
    let destroyed = Driver::object(&game, 1);
    assert_eq!(
        game.zone(destroyed),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
    assert_eq!(game.characteristics(source).unwrap().power, Some(expected));
}

#[test]
fn counters_act_on_the_entered_incarnation() {
    let mut game = Game::new(
        board(&[MIRACULOUS_RECOVERY], 4)
            .permanent_or_hand("Miraculous Recovery", 0, 0, true)
            .card(
                "Grizzly Bears",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            )
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .prepare(),
    );
    let prior = Driver::object(&game, 1);
    cast_at(&mut game, 0, 1, &[2, 3, 4, 5, 6]);
    settle(&mut game);
    let entered = Driver::object(&game, 1);
    assert_eq!(entered.incarnation, prior.incarnation + 1);
    assert_eq!(game.zone(entered), Some(Zone::Battlefield));
    assert_eq!(
        game.state.rules.objects[&entered.card].plus_one_counters,
        1.into()
    );
}

#[test]
fn the_owner_reveals_after_chaos_warps_shuffle() {
    let mut game = Game::new(
        board(&[CHAOS_WARP], 4)
            .permanent_or_hand("Chaos Warp", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 1, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false)
            .library_card("Island", 0)
            .prepare(),
    );
    let prior = Driver::object(&game, 1);
    cast_at(&mut game, 0, 1, &[2, 3, 4]);
    settle(&mut game);
    let entered = Driver::object(&game, 1);
    assert_eq!(entered.incarnation, prior.incarnation + 2);
    assert_eq!(game.zone(entered), Some(Zone::Battlefield));
    assert_eq!(
        game.state.rules.objects[&entered.card].controller,
        PlayerId(1)
    );
    assert_eq!(library(&game, 0), [CardId(5)]);
}

#[test]
fn a_revealed_top_card_uses_the_matching_branch_and_remains_known_in_hand() {
    for (name, destination) in [
        ("Plains", Zone::Battlefield),
        ("Grizzly Bears", Zone::Hand { owner: PlayerId(0) }),
    ] {
        let mut game = Game::new(
            board(&[COILING_ORACLE], 4)
                .permanent_or_hand("Coiling Oracle", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .library_card(name, 0)
                .library_card("Mountain", 0)
                .prepare(),
        );
        Driver::cast(&mut game, CardId(0));
        Driver::pay(&mut game, &[1, 2]);
        settle(&mut game);
        assert_eq!(game.zone(Driver::object(&game, 3)), Some(destination));
        let mut seen = vec![];
        for seat in 0..4 {
            let view = observation(&game, seat).view;
            assert!(view.known_library_positions.is_empty());
            seen.push(view.known_searched_hand);
        }
        assert!(seen.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(
            seen[0].len(),
            usize::from(matches!(destination, Zone::Hand { .. }))
        );
    }
}

#[test]
fn revealed_prefix_choice_binds_only_the_picked_member_and_its_remainder() {
    let make_game = |tail| {
        Game::new(
            board(&[SATYR_WAYFINDER], 4)
                .permanent_or_hand("Satyr Wayfinder", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .library_card("Grizzly Bears", 0)
                .library_card("Plains", 0)
                .library_card("Grizzly Bears", 0)
                .library_card("Grizzly Bears", 0)
                .library_card(tail, 0)
                .prepare(),
        )
    };
    let (mut game, mut alternate) = (make_game("Mountain"), make_game("Island"));
    for game in [&mut game, &mut alternate] {
        Driver::cast(game, CardId(0));
        Driver::pay(game, &[1, 2]);
    }
    let ask = next_choice(&mut game);
    assert_eq!(next_choice(&mut alternate), ask);
    assert_eq!(ask.player, PlayerId(0));
    assert_eq!(ask.reason, Reason::OptionalEffect);
    for seat in 0..4 {
        let seen = observation(&game, seat);
        assert_eq!(seen, observation(&alternate, seat));
        assert_eq!(seen.view.known_library_positions.len(), 4);
    }
    assert_recoverable((&game).into());
    game.respond(
        ask.player,
        ask.id,
        ask.pick_one(&Candidate::Flag { flag: Flag::Accept })
            .unwrap(),
    )
    .unwrap();
    settle(&mut game);
    assert_eq!(
        game.zone(Driver::object(&game, 4)),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
    for object in [3, 5, 6] {
        assert_eq!(
            game.zone(Driver::object(&game, object)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
    }
    assert_eq!(library(&game, 0), [CardId(7)]);
    assert!(
        kir::definition_cells(&lowered(GRISLY_SALVAGE))
            .unwrap()
            .contains(&MOVED)
    );
    assert!(matches!(
        mtg_witness::compile(mtg_witness::R5_MANIFEST, RASHMI),
        mtg_compiler::CompileResult::Unsupported { .. }
    ));
}

/// CR 608.2b, 400.7: Unsummon's target left before it resolved, so the
/// spell does nothing; the card that left is not returned a second time.
#[test]
fn unsummon_skips_a_target_that_left_before_it_resolves() {
    let mut g = Game::new(
        board(&[BOOMERANG], 2)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Boomerang", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    let bears = Driver::object(&g, 2);
    cast_at(&mut g, 0, 2, &[3]);
    // CR 117.3c: the caster receives priority and responds.
    cast_at(&mut g, 1, 2, &[4, 5]);
    checked(&g);
    settle(&mut g);
    let hand = Zone::Hand { owner: PlayerId(1) };
    let returned = Driver::object(&g, 2);
    assert_ne!(returned, bears);
    assert_eq!(g.zone(returned), Some(hand));
    assert_eq!(
        g.zone(Driver::object(&g, 0)),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    // One movement of the bears: Boomerang's.
    let moves = g
        .events()
        .iter()
        .filter(|e| {
            matches!(e, Event::Committed { actual: EventOutcome::Moved { movement }, .. }
            if movement.previous.card == CardId(2))
        })
        .count();
    assert_eq!(moves, 1);
}

/// CR 400.3, 608.2h: exiling a creature another player owns puts it in
/// exile; its last-known controller gains life equal to its last-known
/// power after it has left the battlefield.
#[test]
fn swords_reads_the_exiled_creatures_last_known_controller_and_power() {
    let mut g = Game::new(
        board(&[SWORDS], 2)
            .permanent_or_hand("Swords to Plowshares", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 1, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .prepare(),
    );
    let life = |g: &Game, seat: usize| g.state.rules.players[seat].life.clone();
    let before = (life(&g, 0), life(&g, 1));
    cast_at(&mut g, 0, 1, &[2]);
    settle(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 1)), Some(Zone::Exile));
    let mut gained = before.0.clone();
    gained.add(&Quantity::from(2));
    assert_eq!((life(&g, 0), life(&g, 1)), (gained, before.1));
}

/// CR 110.2a, 603.2, 400.3: Reanimate puts an opponent's creature card
/// onto the battlefield under its caster's control, whose entry trigger
/// is that player's. Returning it later sends it to its owner's hand.
#[test]
fn reanimated_creature_triggers_for_its_controller_and_returns_to_its_owner() {
    let mut g = Game::new(
        board(&[REANIMATE, EXCLUSION_MAGE], 2)
            .permanent_or_hand("Reanimate", 0, 0, true)
            .card(
                "Exclusion Mage",
                1,
                1,
                Zone::Graveyard { owner: PlayerId(1) },
            )
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    let life = g.state.rules.players[0].life.clone();
    cast_at(&mut g, 0, 1, &[3]);
    let trigger = next_choice(&mut g);
    // The entry trigger's controller chooses its target: the opponent's
    // creature (CR 603.3d).
    assert_eq!(trigger.player, PlayerId(0));
    let bears = Driver::object(&g, 2);
    Driver::pick_object(&mut g, bears);
    settle(&mut g);
    let mage = Driver::object(&g, 1);
    assert_eq!(g.zone(mage), Some(Zone::Battlefield));
    assert_eq!(g.state.rules.objects[&mage.card].controller, PlayerId(0));
    assert_eq!(
        g.zone(Driver::object(&g, 2)),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
    let mut lost = life;
    lost.subtract(&Quantity::from(3));
    assert_eq!(g.state.rules.players[0].life, lost);
    // CR 400.3: the stolen permanent goes to its owner's hand.
    cast_at(&mut g, 4, 1, &[5]);
    settle(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 1)),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
}

/// CR 401.4, 701.24a: a permanent goes on top of or under its owner's
/// library, or into it and that library is shuffled. No seat receives a
/// library position the others lack, and the same seed shuffles the same
/// way.
#[test]
fn library_moves_place_on_top_bottom_or_shuffle_deterministically() {
    let run = || {
        let mut setup = board(&[RUN_AGROUND, NATURAL_OBSOLESCENCE, OBLATION], 2)
            .permanent_or_hand("Run Aground", 0, 0, true)
            .permanent_or_hand("Natural Obsolescence", 0, 0, true)
            .permanent_or_hand("Oblation", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Sol Ring", 1, 1, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false);
        for land in ["Island", "Island", "Island", "Island", "Forest", "Forest"] {
            setup = setup.permanent_or_hand(land, 0, 0, false);
        }
        for _ in 0..3 {
            setup = setup.permanent_or_hand("Plains", 0, 0, false);
        }
        let mut g = Game::new(
            setup
                .libraries("Island", 3)
                .libraries("Forest", 3)
                .prepare(),
        );
        let bears_id = oracle_id(FixtureCatalog::Witness.get(), "Grizzly Bears").to_owned();
        cast_at(&mut g, 0, 3, &[6, 7, 8, 9]);
        settle(&mut g);
        assert_eq!(library(&g, 1).first(), Some(&CardId(3)));
        for viewer in 0..2 {
            assert!(
                !known_library(&g, viewer).contains(&bears_id),
                "viewer {viewer}"
            );
        }
        cast_at(&mut g, 1, 4, &[10, 11]);
        settle(&mut g);
        assert_eq!(library(&g, 1).last(), Some(&CardId(4)));
        let size = library(&g, 1).len();
        cast_at(&mut g, 2, 5, &[12, 13, 14]);
        settle(&mut g);
        // Oblation: shuffled in, then its owner draws two (CR 701.24a).
        assert_eq!(library(&g, 1).len(), size + 1 - 2);
        for viewer in 0..2 {
            assert!(
                !known_library(&g, viewer).contains(&bears_id),
                "viewer {viewer}"
            );
        }
        library(&g, 1)
    };
    assert_eq!(run(), run());
}

/// CR 400.2, 401.2, 402.3: in a four-player game a move from the battlefield
/// to a library tells every seat the same thing, and worlds that differ
/// only in another player's hand stay indistinguishable to the others.
#[test]
fn four_seat_moves_reveal_nothing_about_other_hidden_cards() {
    let world = |hidden: &str| {
        let mut g = Game::new(
            board(&[RUN_AGROUND], 4)
                .permanent_or_hand("Run Aground", 0, 0, true)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .libraries("Forest", 2)
                .card(hidden, 2, 2, Zone::Hand { owner: PlayerId(2) })
                .prepare(),
        );
        cast_at(&mut g, 0, 1, &[2, 3, 4, 5]);
        settle(&mut g);
        assert_eq!(library(&g, 1).first(), Some(&CardId(1)));
        g
    };
    let (island, swamp) = (world("Island"), world("Swamp"));
    for viewer in [0, 1, 3] {
        assert_eq!(
            observation(&island, viewer),
            observation(&swamp, viewer),
            "viewer {viewer}"
        );
    }
    assert_ne!(observation(&island, 2), observation(&swamp, 2));
}

/// CR 608.2c, 608.2h, 406.2: "for each creature card exiled this way" reads
/// the cards the earlier step actually exiled, by their characteristics in
/// exile, once the whole graveyard has moved. A land card does not count.
#[test]
fn malanthrope_counts_the_creature_cards_exiled_this_way() {
    let graveyard = Zone::Graveyard { owner: PlayerId(1) };
    let mut g = Game::new(
        board(&[MALANTHROPE], 2)
            .permanent_or_hand("Malanthrope", 0, 0, true)
            .card("Grizzly Bears", 1, 1, graveyard)
            .card("Grizzly Bears", 1, 1, graveyard)
            .card("Forest", 1, 1, graveyard)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[4, 5, 6]);
    let target = next_choice(&mut g);
    assert_eq!(target.player, PlayerId(0));
    let opponent = target
        .candidates
        .iter()
        .find(|c| matches!(c, Candidate::Player { player } if *player == PlayerId(1)))
        .cloned()
        .expect("opponent target");
    let answer = target.pick_one(&opponent).unwrap();
    g.respond(target.player, target.id, answer).unwrap();
    settle(&mut g);
    for card in 1..4 {
        assert_eq!(g.zone(Driver::object(&g, card)), Some(Zone::Exile));
    }
    let malanthrope = Driver::object(&g, 0);
    assert_eq!(
        g.state.rules.objects[&malanthrope.card].plus_one_counters,
        Quantity::from(2)
    );
}

/// CR 400.7, 608.2c, 608.2h: a filtered "returned this way" read of a
/// permanent that went to its owner's hand tests the permanent's last-known
/// characteristics; the card now in that hand is not read. Nurturing Pixie's
/// shape ("return ... to its owner's hand. If a permanent was returned this
/// way, put a +1/+1 counter on this creature"; frozen R5 line 21521) does not
/// lower yet, so frozen Exclusion Mage's return trigger is followed by frozen
/// Malanthrope's "put a +1/+1 counter on this creature for each creature card
/// [moved] this way" step, which reads step 0 with a creature filter.
#[test]
fn a_returned_creature_counts_by_its_last_known_characteristics() {
    let mut mage = lowered(EXCLUSION_MAGE);
    let malanthrope = lowered(MALANTHROPE);
    let [
        Instruction::Kernel {
            node: kir::Node::Seq(steps),
            ..
        },
    ] = malanthrope.triggered[0].program.instructions.as_slice()
    else {
        panic!("Malanthrope's two-step trigger");
    };
    let program = &mut mage.triggered[0].program;
    let [Instruction::Kernel { node, .. }] = program.instructions.as_mut_slice() else {
        panic!("Exclusion Mage's one-step trigger");
    };
    *node = kir::Node::Seq(vec![node.clone(), steps[1].clone()]);
    mage.source_labels = None;
    mage.capabilities = mage.requirements();
    let mut g = Game::new(
        board_of(vec![mage], &[], 2)
            .permanent_or_hand("Exclusion Mage", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[2, 3, 4]);
    let trigger = next_choice(&mut g);
    assert_eq!(trigger.player, PlayerId(0));
    let target = Driver::object(&g, 1);
    Driver::pick_object(&mut g, target);
    settle(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 1)),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
    let mage = Driver::object(&g, 0);
    assert_eq!(
        g.state.rules.objects[&mage.card].plus_one_counters,
        Quantity::from(1)
    );
}

/// CR 400.3, 401.4: Landscaper Colos puts a card from an opponent's
/// graveyard on the bottom of that card's owner's library, not its
/// controller's.
#[test]
fn landscaper_colos_puts_an_opponents_card_under_their_library() {
    let mut setup = board(&[LANDSCAPER_COLOS], 2)
        .permanent_or_hand("Landscaper Colos", 0, 0, true)
        .card(
            "Grizzly Bears",
            1,
            1,
            Zone::Graveyard { owner: PlayerId(1) },
        );
    for _ in 0..6 {
        setup = setup.permanent_or_hand("Plains", 0, 0, false);
    }
    let mut g = Game::new(setup.libraries("Forest", 3).prepare());
    let own = library(&g, 0);
    let theirs = library(&g, 1);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[2, 3, 4, 5, 6, 7]);
    let trigger = next_choice(&mut g);
    assert_eq!(trigger.player, PlayerId(0));
    let target = Driver::object(&g, 1);
    Driver::pick_object(&mut g, target);
    settle(&mut g);
    assert_eq!(library(&g, 0), own);
    let mut expected = theirs;
    expected.push(CardId(1));
    assert_eq!(library(&g, 1), expected);
    assert_eq!(
        g.zone(Driver::object(&g, 1)),
        Some(Zone::Library { owner: PlayerId(1) })
    );
}

/// CR 122.6, 614.1c, 614.12: a creature card returned from a graveyard to
/// the battlefield enters with its printed counters as replaced by Hardened
/// Scales: Faithful Watchdog enters with 3 + 1.
#[test]
fn a_reanimated_creature_enters_with_replaced_counters() {
    let mut g = Game::new(
        board_of(
            vec![lowered(REANIMATE)],
            &[FAITHFUL_WATCHDOG, HARDENED_SCALES],
            2,
        )
        .permanent_or_hand("Reanimate", 0, 0, true)
        .card(
            "Faithful Watchdog",
            0,
            0,
            Zone::Graveyard { owner: PlayerId(0) },
        )
        .permanent_or_hand("Hardened Scales", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .prepare(),
    );
    let life = g.state.rules.players[0].life.clone();
    cast_at(&mut g, 0, 1, &[3]);
    settle(&mut g);
    let watchdog = Driver::object(&g, 1);
    assert_eq!(g.zone(watchdog), Some(Zone::Battlefield));
    assert_eq!(
        g.state.rules.objects[&watchdog.card].plus_one_counters,
        Quantity::from(4)
    );
    let mut lost = life;
    lost.subtract(&Quantity::from(2));
    assert_eq!(g.state.rules.players[0].life, lost);
}
