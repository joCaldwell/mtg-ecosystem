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
