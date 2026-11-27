//! Frozen R5 witnesses for pronoun antecedents and a spell's stack mana value.
//! CR 202.3e, 400.7, 603.10a, 608.2b-c, 608.2h, 122.6.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{ExecutionTier, definition_tiers};
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

const BRIGHTMARE: &str = record!("32025108-0dba-40bd-88db-5fcc48e3d054");
const CALDERA_PYREMAW: &str = record!("3eb5ae68-7f76-4874-80a4-80058b10fc7a");
const KAERVEK: &str = record!("c7b72d38-0aa0-4e17-9dd5-9276d7cb21ec");
const BOUNTEOUS_KIRIN: &str = record!("31d572b8-2c5e-4819-b2a6-e7ef90872d9e");
const BLAZE: &str = record!("0596920f-9946-42f4-a03b-24aab67f9f1b");
const SWALLOWING_PLAGUE: &str = record!("91cac048-90ca-41bc-9177-56c78453c4a2");
const JET: &str = record!("77cd2246-6906-4dce-8580-4c9d2e68c477");

fn board(cards: &[&str]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in cards {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen antecedent witness failed: {result:?}");
        };
        assert!(
            definition_tiers(&definition)
                .unwrap()
                .admits(ExecutionTier::Executes)
        );
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-verb-operands").step(TurnStep::PrecombatMain)
}

fn life(game: &Game, seat: usize) -> Quantity {
    game.state.rules.players[seat].life.clone()
}

fn minus(mut value: Quantity, by: i64) -> Quantity {
    value.subtract(&by.into());
    value
}

/// Pass priority until the stack is empty, answering each target request
/// with `targets` and accepting each optional effect.
fn drive(game: &mut Game, mut targets: impl FnMut(&Decision<Rules>) -> Answer) {
    for _ in 0..64 {
        let request = Driver::request(game, 1);
        let answer = match &request.reason {
            Reason::Priority if game.state.rules.stack.is_empty() => {
                game.validate_state().unwrap();
                assert_recoverable((&*game).into());
                return;
            }
            Reason::Priority => mtg_witness::temporary::pass(&request),
            Reason::Targets { .. } => targets(&request),
            Reason::OptionalEffect => request
                .pick_one(&Candidate::Flag { flag: Flag::Accept })
                .unwrap(),
            reason => panic!("unexpected decision {reason:?}"),
        };
        game.respond(request.player, request.id, answer).unwrap();
    }
    panic!("no empty-stack priority");
}

/// "Tap up to one target creature. You gain life equal to that creature's
/// power": the life is the tapped opponent's creature's power, not
/// Brightmare's own. Choosing no target gains nothing.
#[test]
fn brightmare_gains_the_tapped_targets_power() {
    for chosen in [true, false] {
        let prepared = board(&[BRIGHTMARE])
            .permanent_or_hand("Brightmare", 0, 0, true)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
            .libraries("Plains", 4)
            .prepare();
        let mut game = Game::with_trace(prepared, TraceMode::Detailed);
        let dreadmaw = Driver::object(&game, 4);
        let before = life(&game, 0);
        Driver::cast(&mut game, CardId(0));
        Driver::pay(&mut game, &[1, 2, 3]);
        drive(&mut game, |request| {
            if chosen {
                request.entity_targets(vec![TargetEntity::Object { object: dreadmaw }])
            } else {
                Answer::Pick {
                    picks: vec![vec![]],
                }
            }
        });
        let mut expected = before;
        if chosen {
            expected.add(&6.into());
        }
        assert_eq!(life(&game, 0), expected, "chosen: {chosen}");
        assert_eq!(game.current(dreadmaw).unwrap().tapped, chosen);
    }
}

/// "Put a +1/+1 counter on this creature. Then this creature deals damage
/// equal to its power to target opponent": its power after the counter
/// (4), not the triggering instant's.
#[test]
fn caldera_pyremaw_deals_its_own_power_after_the_counter() {
    let prepared = board(&[CALDERA_PYREMAW])
        .permanent_or_hand("Caldera Pyremaw", 0, 0, false)
        .permanent_or_hand("Lightning Bolt", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .libraries("Mountain", 4)
        .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let pyremaw = Driver::object(&game, 0);
    let bears = Driver::object(&game, 3);
    let before = life(&game, 1);
    Driver::cast(&mut game, CardId(1));
    let bolt = Driver::request(&mut game, u64::MAX);
    let answer = bolt.entity_targets(vec![TargetEntity::Object { object: bears }]);
    game.respond(bolt.player, bolt.id, answer).unwrap();
    Driver::pay(&mut game, &[2]);
    drive(&mut game, |request| {
        request.entity_targets(vec![TargetEntity::Player {
            player: PlayerId(1),
        }])
    });
    assert_eq!(game.current(pyremaw).unwrap().plus_one_counters, 1.into());
    assert_eq!(life(&game, 1), minus(before, 4));
    assert!(game.current(bears).is_err());
}

/// "Whenever an opponent casts a spell, Kaervek deals damage equal to that
/// spell's mana value to any target": Blaze with X = 3 has mana value 4
/// on the stack (CR 202.3e), not its printed 1.
#[test]
fn kaervek_reads_the_cast_spells_mana_value_with_x() {
    let prepared = board(&[KAERVEK, BLAZE])
        .permanent_or_hand("Kaervek the Merciless", 1, 1, false)
        .permanent_or_hand("Blaze", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .libraries("Mountain", 4)
        .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let before = [life(&game, 0), life(&game, 1)];
    Driver::cast(&mut game, CardId(1));
    let x = Driver::request(&mut game, u64::MAX);
    assert_eq!(x.reason, Reason::AnnounceX);
    game.respond(x.player, x.id, Answer::Number { value: 3.into() })
        .unwrap();
    let blaze = Driver::request(&mut game, u64::MAX);
    let answer = blaze.entity_targets(vec![TargetEntity::Player {
        player: PlayerId(1),
    }]);
    game.respond(blaze.player, blaze.id, answer).unwrap();
    Driver::pay(&mut game, &[2, 3, 4, 5]);
    drive(&mut game, |request| {
        assert_eq!(request.player, PlayerId(1));
        request.entity_targets(vec![TargetEntity::Player {
            player: PlayerId(0),
        }])
    });
    let [caster, kaervek] = before;
    assert_eq!(life(&game, 0), minus(caster, 4));
    assert_eq!(life(&game, 1), minus(kaervek, 3));
}

/// "Whenever you cast a Spirit or Arcane spell, you may gain life equal to
/// that spell's mana value": Swallowing Plague with X = 2 is mana value 4,
/// and then gains its own 2 life as it kills the targeted Grizzly Bears.
#[test]
fn bounteous_kirin_gains_the_arcane_spells_mana_value_with_x() {
    let prepared = board(&[BOUNTEOUS_KIRIN, SWALLOWING_PLAGUE])
        .permanent_or_hand("Bounteous Kirin", 0, 0, false)
        .permanent_or_hand("Swallowing Plague", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .libraries("Swamp", 4)
        .prepare();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&game, 6);
    let before = life(&game, 0);
    Driver::cast(&mut game, CardId(1));
    let x = Driver::request(&mut game, u64::MAX);
    assert_eq!(x.reason, Reason::AnnounceX);
    game.respond(x.player, x.id, Answer::Number { value: 2.into() })
        .unwrap();
    let plague = Driver::request(&mut game, u64::MAX);
    let answer = plague.entity_targets(vec![TargetEntity::Object { object: bears }]);
    game.respond(plague.player, plague.id, answer).unwrap();
    Driver::pay(&mut game, &[2, 3, 4, 5]);
    drive(&mut game, |request| {
        panic!("unexpected targets {request:?}")
    });
    assert!(game.current(bears).is_err());
    let mut expected = before;
    expected.add(&6.into());
    assert_eq!(life(&game, 0), expected);
}

/// Negative neighbour: "put a +1/+1 counter on each of up to two target
/// creatures" is one simultaneous placement (CR 122.6), which per-target
/// leaves would split into two events, so the shape rejects by name.
#[test]
fn jet_rejects_counters_on_each_of_several_targets() {
    let mtg_compiler::CompileResult::Unsupported { path, reason } =
        mtg_witness::compile(mtg_witness::R5_MANIFEST, JET)
    else {
        panic!("Jet, Freedom Fighter did not reject");
    };
    assert!(
        reason.ends_with("operand:target:up_to:counters"),
        "{path}: {reason}"
    );
}
