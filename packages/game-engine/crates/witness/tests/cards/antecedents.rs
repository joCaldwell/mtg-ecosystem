//! Frozen R5 witnesses for pronoun antecedents and a spell's stack mana value.
//! CR 202.3e, 400.7, 603.2c, 603.10a, 608.2b-c, 608.2f, 608.2h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::CounterKind;
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
const SOMBERWALD_VIGILANTE: &str = record!("31c77036-8627-476a-996b-2a6b005c9e12");
const KOLAGHAN_ASPIRANT: &str = record!("334178f6-cf4b-4ea6-a24f-c7375d1b38de");
const QUAGMIRE_LAMPREY: &str = record!("f5b374d8-bf22-44f7-a18a-18c096a84640");
const GLOOM_SOWER: &str = record!("e5cc1492-c9cc-48f0-8c11-02ead883675c");
const GILDED_SCUTTLER: &str = record!("093f6123-6b1d-456f-a526-8740bc969b4e");
const ICE_FLAN: &str = record!("3b7120a6-700b-4c61-bea4-a835f0cc0bcb");

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
/// creatures" is one simultaneous placement (CR 608.2f, 603.2c), which
/// per-target leaves would split into two events, so the shape rejects by
/// name.
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

/// Attack with `attacker` and have `blocker` block it, then resolve the
/// declare-blockers triggers to an empty stack.
fn attack_into_block(game: &mut Game, attacker: ObjectId, blocker: ObjectId) {
    let mut declared = false;
    for _ in 0..128 {
        let request = Driver::request(game, u64::MAX);
        if declared
            && request.reason == Reason::Priority
            && game.state.rules.stack.is_empty()
            && game.state.rules.pending_triggers.is_empty()
        {
            assert_eq!(game.state.rules.turn.step, TurnStep::DeclareBlockers);
            game.validate_state().unwrap();
            return;
        }
        let answer = match &request.reason {
            Reason::Priority => mtg_witness::temporary::pass(&request),
            Reason::Attackers => request.attack(vec![(attacker, PlayerId(1))]),
            Reason::Blockers => {
                declared = true;
                request.block(vec![(blocker, attacker)])
            }
            reason => panic!("unexpected decision {reason:?}"),
        };
        game.respond(request.player, request.id, answer).unwrap();
    }
    panic!("the block never resolved");
}

/// "Whenever this creature becomes blocked by a creature, this creature
/// deals 1 damage to that creature" / "put a -1/-1 counter on that
/// creature" / "that creature's controller loses 2 life and you gain 2
/// life": "that creature" is the blocking Grizzly Bears (CR 509.3d,
/// 608.2c), never the attacker the trigger is printed on.
#[test]
fn becomes_blocked_by_a_creature_binds_the_blocker() {
    for (raw, name) in [
        (SOMBERWALD_VIGILANTE, "Somberwald Vigilante"),
        (KOLAGHAN_ASPIRANT, "Kolaghan Aspirant"),
        (QUAGMIRE_LAMPREY, "Quagmire Lamprey"),
        (GLOOM_SOWER, "Gloom Sower"),
    ] {
        let prepared = board(&[raw])
            .step(TurnStep::BeginningCombat)
            .permanent_or_hand(name, 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .libraries("Plains", 4)
            .prepare();
        let mut game = Game::with_trace(prepared, TraceMode::Detailed);
        let (attacker, bears) = (Driver::object(&game, 0), Driver::object(&game, 1));
        let before = [life(&game, 0), life(&game, 1)];
        attack_into_block(&mut game, attacker, bears);
        let blocker = game.current(bears).unwrap();
        let own = game.current(attacker).unwrap();
        let minus_one = CounterKind::MinusOneMinusOne;
        // (blocker damage, blocker -1/-1 counters, life change of each seat)
        let (damage, counters, drain) = match name {
            "Quagmire Lamprey" => (0, 1, 0),
            "Gloom Sower" => (0, 0, 2),
            _ => (1, 0, 0),
        };
        assert_eq!(blocker.marked_damage, damage.into(), "{name}");
        assert_eq!(blocker.counter_count(&minus_one), counters.into(), "{name}");
        assert_eq!(own.marked_damage, 0.into(), "{name}");
        assert_eq!(own.counter_count(&minus_one), 0.into(), "{name}");
        let [mine, theirs] = before;
        assert_eq!(life(&game, 0), minus(mine, -drain), "{name}");
        assert_eq!(life(&game, 1), minus(theirs, drain), "{name}");
        assert_recoverable((&game).into());
    }
}

/// "When this creature enters, tap target creature an opponent controls and
/// put a stun counter on it" (Gilded Scuttler), and the same in two
/// sentences on Ice Flan: "it" is the tapped target (CR 608.2c), not the
/// entering creature whose trigger it is.
#[test]
fn stun_counter_follows_the_tapped_target() {
    for (raw, name, lands) in [
        (GILDED_SCUTTLER, "Gilded Scuttler", 3),
        (ICE_FLAN, "Ice Flan", 6),
    ] {
        let mut board = board(&[raw])
            .permanent_or_hand(name, 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 1, 1, false);
        for _ in 0..lands {
            board = board.permanent_or_hand("Island", 0, 0, false);
        }
        let prepared = board.libraries("Island", 4).prepare();
        let mut game = Game::with_trace(prepared, TraceMode::Detailed);
        let bears = Driver::object(&game, 1);
        Driver::cast(&mut game, CardId(0));
        Driver::pay(&mut game, &(2..2 + lands).collect::<Vec<_>>());
        drive(&mut game, |request| {
            request.entity_targets(vec![TargetEntity::Object { object: bears }])
        });
        let stun = CounterKind::Stun;
        let blocker = game.current(bears).unwrap();
        assert!(blocker.tapped, "{name}");
        assert_eq!(blocker.counter_count(&stun), 1.into(), "{name}");
        let [entered] = game.creatures(PlayerId(0), false)[..] else {
            panic!("{name} is not the only creature its controller has");
        };
        let entered = game.current(entered).unwrap();
        assert_eq!(entered.zone, Zone::Battlefield);
        assert!(!entered.tapped, "{name}");
        assert_eq!(entered.counter_count(&stun), 0.into(), "{name}");
    }
}
