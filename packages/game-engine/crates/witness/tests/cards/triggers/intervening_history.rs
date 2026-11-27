//! Frozen R5 witnesses for CR 603.4's two checkpoints.
use super::*;
use mtg_contracts::kir::{Intervening, TurnFact};

const AMBUSHER: &str = record!("4d67357c-5939-45c8-a656-aaf76728a5a6");
const JEM: &str = record!("5802aa98-a9b7-4b0d-9d7c-c3938701870d");
const BULETTE: &str = record!("4e690b82-9877-41f2-9dcd-cffdb6e3cd64");

#[test]
fn intervening_history_frozen_conditions_lower() {
    for (record, expected) in [
        (BULETTE, TurnFact::CreatureDied),
        (
            record!("1b82312c-f748-4c36-8934-ecbcfc10cc7d"),
            TurnFact::CreatureDiedUnderYourControl,
        ),
        (
            record!("009a399b-c78e-475b-8bc3-7db2afc9d676"),
            TurnFact::PermanentLeftUnderYourControl,
        ),
        (
            record!("f9ea5b9e-a23d-48cf-8601-56e79ca844f9"),
            TurnFact::Attacked,
        ),
        (
            record!("a177295e-3b58-4e46-a1cb-fc003a7a0848"),
            TurnFact::LifeGained,
        ),
        (
            record!("5802aa98-a9b7-4b0d-9d7c-c3938701870d"),
            TurnFact::SpellsCastFromHand,
        ),
        (
            record!("064a2f4a-ff3f-4622-8727-24b22965416d"),
            TurnFact::NoncreatureSpellsCast,
        ),
    ] {
        let definitions = compiled(&[record], false);
        let condition = definitions[0].triggered[0]
            .condition
            .intervening_if()
            .unwrap();
        assert!(matches!(Intervening::of(condition),
            Ok(Intervening::TurnHistory { fact, .. }) if fact == expected));
    }
}

#[test]
fn intervening_history_cast_can_prevent_collection_or_resolution() {
    for cast_before_trigger in [false, true] {
        let mut game = Game::with_trace(
            prepare(
                board_with(compiled(&[JEM], false), TurnStep::PrecombatMain)
                    .permanent_or_hand("Jem Lightfoote, Sky Explorer", 0, 0, false)
                    .permanent_or_hand("Giant Growth", 0, 0, true)
                    .permanent_or_hand("Forest", 0, 0, false),
            ),
            TraceMode::Detailed,
        );
        let jem = Driver::object(&game, 0);
        let cast = |game: &mut Game| {
            Driver::cast(game, CardId(1));
            Driver::pick_object(game, jem);
            Driver::pay(game, &[2]);
        };
        if cast_before_trigger {
            cast(&mut game);
            settle(&mut game);
        }
        drive(&mut game, &[], |g| g.state.rules.turn.step == TurnStep::End);
        assert_eq!(triggered(&game), !cast_before_trigger);
        if !cast_before_trigger {
            checked(&game);
            cast(&mut game);
            assert_eq!(game.state.rules.cast_history[0].hand_casts, 1);
            settle(&mut game);
        }
        assert_eq!(game.state.rules.cast_history[0].draws, 0);
        checked(&game);
    }
}

#[test]
fn intervening_history_creature_death_uses_its_pre_move_controller() {
    let mut game = Game::with_trace(
        prepare(
            board_with(compiled(&[BULETTE], false), TurnStep::PrecombatMain)
                .permanent_or_hand("Bulette", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let victim = Driver::object(&game, 1);
    Driver::cast(&mut game, CardId(2));
    Driver::pick_object(&mut game, victim);
    Driver::pay(&mut game, &[3]);
    settle(&mut game);
    assert!(!game.state.rules.cast_history[0].creature_died);
    assert!(game.state.rules.cast_history[1].creature_died);
    assert!(game.state.rules.cast_history[1].permanent_left);
    checked(&game);
    drive(&mut game, &[], triggered);
    settle(&mut game);
    assert_eq!(
        game.current(Driver::object(&game, 0))
            .unwrap()
            .plus_one_counters,
        1.into()
    );
}

#[test]
fn intervening_history_keeps_other_frozen_blockers() {
    for (record, blocker) in [
        (AMBUSHER, "cell:static:continuous-change"),
        (
            record!("0f208cd3-7673-4380-9cb8-5b6f09966ebd"),
            "diagnostic:unsupported-reference",
        ),
    ] {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, record);
        assert!(format!("{result:?}").contains(blocker), "{result:?}");
    }
}

#[test]
fn intervening_history_attack_is_attributed_and_resets() {
    let mut game = Game::with_trace(
        prepare(
            board_with(vec![], TurnStep::BeginningCombat).permanent_or_hand(
                "Grizzly Bears",
                0,
                0,
                false,
            ),
        ),
        TraceMode::Detailed,
    );
    let attacker = Driver::object(&game, 0);
    drive(&mut game, &[attacker], |g| {
        g.state.rules.turn.step == TurnStep::EndCombat
    });
    assert!(game.state.rules.cast_history[0].attacked);
    assert!(!game.state.rules.cast_history[1].attacked);
    checked(&game);
    drive(&mut game, &[], |g| g.state.rules.turn.number == 2);
    assert!(
        game.state
            .rules
            .cast_history
            .iter()
            .all(|row| !row.attacked)
    );
    checked(&game);
}

#[test]
fn intervening_history_entries_count_current_types_through_turn_boundary() {
    const MALCATOR: &str = record!("0d1c6a82-943a-46bd-97ed-6055226ef2ab");
    const LADY: &str = record!("356a85d9-2570-448c-a455-f4ce915a604f");
    const GARGOYLES: &str = record!("6d24fc3e-eaa9-4ac8-b271-7dc83b812acf");
    let mut game = Game::with_trace(
        prepare(
            board_with(
                compiled(&[MALCATOR, LADY, GARGOYLES], false),
                TurnStep::PrecombatMain,
            )
            .permanent_or_hand("Malcator, Purity Overseer", 0, 0, true)
            .permanent_or_hand("Lady of Laughter", 0, 0, false)
            .permanent_or_hand("Gargoyle Flock", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, true),
        ),
        TraceMode::Detailed,
    );
    let land = Driver::object(&game, 6);
    Driver::action(&mut game, Action::PlayLand { object: land });
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[3, 4, 5]);
    settle(&mut game);

    let row = &game.state.rules.cast_history[0];
    assert_eq!(row.artifacts_entered, 1.into());
    assert_eq!(row.creatures_entered, 2.into());
    assert_eq!(row.nonlands_entered, 2.into());
    checked(&game);
    let mut forged: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
    forged.state.rules.cast_history[0].artifacts_entered = Quantity::from(-1);
    assert_forgery_rejected(&game, forged, "snapshot turn history invariant");
    let malcator = Driver::object(&game, 0);
    let lady = Driver::object(&game, 1);
    let gargoyles = Driver::object(&game, 2);
    drive(&mut game, &[], |g| g.state.rules.turn.step == TurnStep::End);
    settle(&mut game);
    assert_eq!(intervening(&game, malcator), [(true, false)]);
    assert_eq!(intervening(&game, lady), [(true, true), (false, true)]);
    assert_eq!(intervening(&game, gargoyles), [(true, true), (false, true)]);
    assert_eq!(game.state.rules.cast_history[0].draws, 1);

    let turn = game.state.rules.turn.number;
    drive(&mut game, &[], |g| g.state.rules.turn.number > turn);
    let row = &game.state.rules.cast_history[0];
    assert_eq!(row.artifacts_entered, Quantity::zero());
    assert_eq!(row.creatures_entered, Quantity::zero());
    assert_eq!(row.nonlands_entered, Quantity::zero());
    checked(&game);
}

#[test]
fn intervening_history_lictors_own_entry_does_not_count_as_an_opponents_entry() {
    const LICTOR: &str = record!("3fdf064d-a5be-4925-87b1-3547997f0840");
    for opponent_entered in [false, true] {
        let mut game = Game::with_trace(
            prepare(
                board_with(compiled(&[LICTOR], false), TurnStep::PrecombatMain)
                    .permanent_or_hand("Lictor", 1, 1, true)
                    .permanent_or_hand("Grizzly Bears", 0, 0, true)
                    .permanent_or_hand("Forest", 0, 0, false)
                    .permanent_or_hand("Forest", 0, 0, false)
                    .permanent_or_hand("Forest", 1, 1, false)
                    .permanent_or_hand("Forest", 1, 1, false)
                    .permanent_or_hand("Forest", 1, 1, false)
                    .permanent_or_hand("Forest", 1, 1, false),
            ),
            TraceMode::Detailed,
        );
        if opponent_entered {
            Driver::cast(&mut game, CardId(1));
            Driver::pay(&mut game, &[2, 3]);
            settle(&mut game);
        }
        drive(&mut game, &[], |g| {
            g.state.rules.priority == PlayerId(1) && matches!(g.state.rules.frame, Frame::Priority)
        });
        Driver::cast(&mut game, CardId(0));
        Driver::pay(&mut game, &[4, 5, 6, 7]);
        settle(&mut game);
        let lictor = Driver::object(&game, 0);
        assert_eq!(
            intervening(&game, lictor),
            if opponent_entered {
                vec![(true, true), (false, true)]
            } else {
                vec![(true, false)]
            }
        );
        assert_eq!(game.state.rules.tokens.len(), usize::from(opponent_entered));
    }
}
