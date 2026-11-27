//! Frozen R5 witnesses for disjoint trigger alternatives and same-kind
//! departures (CR 119.9, 506.4, 509.1a, 509.3a, 603.2c, 603.6c, 603.10a).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const SAVVY_HUNTER: &str = record!("602132c2-8ee8-41f8-bfac-cb17d32203f5");
const VELA: &str = record!("185e34f0-60ae-4b07-94ac-96cef11b1604");
const BRANDYWINE_FARMER: &str = record!("0fea3406-e5c7-4d5c-9890-6f1bc04a7cb8");
const MACH_1: &str = record!("75810ce7-328c-4512-8e36-fd3badbd3997");

fn tokens(g: &Game, seat: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && o.controller == PlayerId(seat)
                && g.state.rules.tokens.contains_key(&o.id.card)
        })
        .count()
}

/// Triggers collected so far from `source`.
fn collected(g: &Game, source: ObjectId) -> usize {
    g.events()
        .iter()
        .filter(|e| matches!(e, Event::TriggerCollected { source: s, .. } if *s == source))
        .count()
}

/// Savvy Hunter: "Whenever this creature attacks or blocks, create a Food
/// token." One Hunter attacks and the other blocks it: each declaration
/// triggers its own Hunter once (CR 508.3a, 509.3a).
#[test]
fn event_alternatives_attack_or_block_trigger_once_each() {
    let mut g = Game::with_trace(
        prepare(
            board(&[SAVVY_HUNTER], TurnStep::BeginningCombat)
                .permanent_or_hand("Savvy Hunter", 0, 0, false)
                .permanent_or_hand("Savvy Hunter", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let (attacker, blocker) = (Driver::object(&g, 0), Driver::object(&g, 1));
    assert!(matches!(
        g.definition(CardId(0)).unwrap().triggered[0].condition.pattern(),
        EventPattern::AnyOf { first, second }
            if matches!(**first, EventPattern::Attacks { .. })
                && matches!(**second, EventPattern::Blocks { .. })
    ));
    for _ in 0..64 {
        let d = Driver::request(&mut g, u64::MAX);
        if tokens(&g, 1) == 1 {
            break;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(vec![(attacker, PlayerId(1))]),
            Reason::Blockers => {
                // The attack trigger resolved before blockers.
                assert_eq!(tokens(&g, 0), 1);
                d.block(vec![(blocker, attacker)])
            }
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
        if triggered(&g) {
            checked(&g);
        }
    }
    assert_eq!((tokens(&g, 0), tokens(&g, 1)), (1, 1));
    assert_eq!((collected(&g, attacker), collected(&g, blocker)), (1, 1));
    settle(&mut g);
    assert_recoverable((&g).into());
}

/// Vela the Night-Clad: "Whenever Vela or another creature you control
/// leaves the battlefield, each opponent loses 1 life." The Grizzly Bears
/// dying and Vela being returned to hand each trigger it, Vela looking back
/// at itself (CR 603.6c, 603.10a).
#[test]
fn event_alternatives_self_or_another_creature_leaves() {
    let mut g = Game::with_trace(
        prepare(
            board(&[VELA], TurnStep::PrecombatMain)
                .permanent_or_hand("Vela the Night-Clad", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Lightning Bolt", 1, 1, true)
                .permanent_or_hand("Unsummon", 1, 1, true)
                .permanent_or_hand("Mountain", 1, 1, false)
                .permanent_or_hand("Island", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let (vela, bears) = (Driver::object(&g, 0), Driver::object(&g, 1));
    let before = life(&g, 1);
    drive(&mut g, &[], |g| g.state.rules.priority == PlayerId(1));
    Driver::cast(&mut g, CardId(2));
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[4]);
    drive(&mut g, &[], triggered);
    checked(&g);
    settle(&mut g);
    assert!(g.current(bears).is_err());
    assert_eq!(life(&g, 1), plus(&before, -1));
    drive(&mut g, &[], |g| g.state.rules.priority == PlayerId(1));
    Driver::cast(&mut g, CardId(3));
    Driver::pick_object(&mut g, vela);
    Driver::pay(&mut g, &[5]);
    drive(&mut g, &[], triggered);
    checked(&g);
    settle(&mut g);
    assert!(g.current(vela).is_err());
    assert_eq!(life(&g, 1), plus(&before, -2));
    assert_eq!(collected(&g, vela), 2);
    assert_recoverable((&g).into());
}

/// Brandywine Farmer: "When this creature enters or leaves the battlefield,
/// create a Food token." Casting it and then returning it to hand each
/// trigger the one ability (CR 603.2c, 603.6a, 603.6c).
#[test]
fn event_alternatives_enters_or_leaves() {
    let mut g = Game::with_trace(
        prepare(
            board(&[BRANDYWINE_FARMER], TurnStep::PrecombatMain)
                .permanent_or_hand("Brandywine Farmer", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Unsummon", 1, 1, true)
                .permanent_or_hand("Island", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    drive(&mut g, &[], triggered);
    checked(&g);
    settle(&mut g);
    assert_eq!(tokens(&g, 0), 1);
    let farmer = g
        .state
        .rules
        .objects
        .values()
        .find(|o| o.id.card == CardId(0) && o.zone == Zone::Battlefield)
        .unwrap()
        .id;
    drive(&mut g, &[], |g| g.state.rules.priority == PlayerId(1));
    Driver::cast(&mut g, CardId(4));
    Driver::pick_object(&mut g, farmer);
    Driver::pay(&mut g, &[5]);
    drive(&mut g, &[], triggered);
    checked(&g);
    settle(&mut g);
    assert_eq!(hand(&g, 0), 1);
    assert_eq!(tokens(&g, 0), 2);
    assert_recoverable((&g).into());
}

/// Answer the default to every decision, keeping cards on top for surveil,
/// until `stop` holds.
fn drive_surveil(g: &mut Game, stop: impl Fn(&Game) -> bool) {
    for _ in 0..256 {
        let d = Driver::request(g, u64::MAX);
        if stop(g) {
            return;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::OptionalEffect => d.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap(),
            Reason::SurveilGraveyard => Answer::Pick {
                picks: vec![vec![]],
            },
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the board never reached its stop");
}

/// MACH-1: "When MACH-1 enters and whenever you gain life, surveil 1. This
/// ability triggers only once each turn." Two Grizzly Bears entering make
/// Soul's Attendant gain life twice; MACH-1 triggers for the first gain only
/// (CR 119.9, 603.2c).
#[test]
fn event_alternatives_entry_or_life_gain_once_each_turn() {
    let mut g = Game::with_trace(
        prepare(
            board(&[MACH_1], TurnStep::PrecombatMain)
                .permanent_or_hand("MACH-1, Swooping Scoundrel", 0, 0, false)
                .permanent_or_hand("Soul's Attendant", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, true)
                .permanent_or_hand("Grizzly Bears", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let mach = Driver::object(&g, 0);
    let definition = g.definition(CardId(0)).unwrap();
    assert!(definition.triggered[0].condition.once_each_turn());
    assert!(matches!(
        definition.triggered[0].condition.pattern(),
        EventPattern::AnyOf { second, .. } if matches!(**second, EventPattern::GainsLife { .. })
    ));
    let before = life(&g, 0);
    for (card, lands) in [(2, [4, 5]), (3, [6, 7])] {
        Driver::cast(&mut g, CardId(card));
        Driver::pay(&mut g, &lands);
        drive_surveil(&mut g, |g| {
            g.state.rules.stack.is_empty()
                && g.state.rules.pending_triggers.is_empty()
                && matches!(g.state.rules.frame, Frame::Priority)
        });
        checked(&g);
    }
    assert_eq!(life(&g, 0), plus(&before, 2));
    assert_eq!(collected(&g, mach), 1);
    assert_eq!(g.state.rules.trigger_uses.len(), 1);
    assert_recoverable((&g).into());
}

const HIGH_RISE_SAWJACK: &str = record!("0049438b-e37e-4c57-820e-f2ddf3322170");
const CAVE_TIGER: &str = record!("d3bac8b8-0b2a-4c67-b087-7a0bf1359468");

/// Declare `attackers` against seat 1 and `blocks`, then stop with every
/// trigger of the declaration on the stack.
fn declare(g: &mut Game, attackers: &[ObjectId], blocks: &[(ObjectId, ObjectId)], stack: usize) {
    for _ in 0..64 {
        let d = Driver::request(g, u64::MAX);
        if g.state.rules.stack.len() == stack {
            return;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(attackers.iter().map(|a| (*a, PlayerId(1))).collect()),
            Reason::Blockers => d.block(blocks.to_vec()),
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
            },
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the declaration never triggered");
}

/// Cave Tiger: "Whenever this creature becomes blocked by a creature, this
/// creature gets +1/+1 until end of turn." Two blockers trigger it twice,
/// once per blocking creature (CR 509.3d).
#[test]
fn event_alternatives_becomes_blocked_by_a_creature_triggers_per_blocker() {
    let mut g = Game::with_trace(
        prepare(
            board(&[CAVE_TIGER], TurnStep::BeginningCombat)
                .permanent_or_hand("Cave Tiger", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let (tiger, a, b) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
    );
    assert!(matches!(
        g.definition(CardId(0)).unwrap().triggered[0].condition.pattern(),
        EventPattern::BlockedByCreature { .. }
    ));
    declare(&mut g, &[tiger], &[(a, tiger), (b, tiger)], 2);
    checked(&g);
    assert_eq!(collected(&g, tiger), 2);
    settle(&mut g);
    assert_recoverable((&g).into());
}

/// High-Rise Sawjack: "Whenever this creature blocks a creature with
/// flying, this creature gets +2/+0 until end of turn." The Sawjack blocking
/// Storm Crow triggers; the one blocking Grizzly Bears does not. Each
/// attacker is read as it was declared (CR 509.3b, 509.3f).
#[test]
fn event_alternatives_blocks_a_filtered_creature_reads_the_attacker() {
    let mut g = Game::with_trace(
        prepare(
            board(&[HIGH_RISE_SAWJACK], TurnStep::BeginningCombat)
                .permanent_or_hand("Storm Crow", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("High-Rise Sawjack", 1, 1, false)
                .permanent_or_hand("High-Rise Sawjack", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let (crow, bears, high, low) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
        Driver::object(&g, 3),
    );
    declare(&mut g, &[crow, bears], &[(high, crow), (low, bears)], 1);
    checked(&g);
    assert_eq!((collected(&g, high), collected(&g, low)), (1, 0));
    settle(&mut g);
    assert_recoverable((&g).into());
}

const CAT_COLLECTOR: &str = record!("52f1b5a4-62ff-44de-80da-503d4e52d312");

/// Cat Collector: "Whenever you gain life for the first time during each of
/// your turns, create a 1/1 white Cat creature token." Each player's Soul's
/// Attendant gains life as two Grizzly Bears and the Cat enter on seat 0's
/// turn: only seat 0's Collector triggers, and only for its first gain
/// (CR 102.1, 119.9, 603.2).
#[test]
fn event_alternatives_first_life_gain_during_your_turn() {
    let mut g = Game::with_trace(
        prepare(
            board(&[CAT_COLLECTOR], TurnStep::PrecombatMain)
                .permanent_or_hand("Cat Collector", 0, 0, false)
                .permanent_or_hand("Soul's Attendant", 0, 0, false)
                .permanent_or_hand("Cat Collector", 1, 1, false)
                .permanent_or_hand("Soul's Attendant", 1, 1, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, true)
                .permanent_or_hand("Grizzly Bears", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (mine, theirs) = (Driver::object(&g, 0), Driver::object(&g, 2));
    let condition = &g.definition(CardId(0)).unwrap().triggered[1].condition;
    assert!(condition.your_turn() && condition.pattern().nth().is_some());
    let before = (life(&g, 0), life(&g, 1));
    for (card, lands) in [(4, [6, 7]), (5, [8, 9])] {
        Driver::cast(&mut g, CardId(card));
        Driver::pay(&mut g, &lands);
        settle(&mut g);
    }
    assert_eq!(
        (life(&g, 0), life(&g, 1)),
        (plus(&before.0, 3), plus(&before.1, 3))
    );
    assert_eq!((collected(&g, mine), collected(&g, theirs)), (1, 0));
    assert_eq!((tokens(&g, 0), tokens(&g, 1)), (1, 0));
    assert_recoverable((&g).into());
}

const MOCKINGBIRD: &str = record!("1fd00656-83ba-4feb-b089-224007936b99");
const REHEARSED_DEBATER: &str = record!("e30ccf07-0969-4e81-ba43-c50a2ff9c040");

/// Mockingbird, Ace Agent: "Whenever you cast a spell that targets a
/// creature you control, put a +1/+1 counter on Mockingbird." Rehearsed
/// Debater: "Repartee — Whenever you cast an instant or sorcery spell that
/// targets a creature, this creature gets +1/+1 until end of turn." Giant
/// Growth on your Grizzly Bears triggers both; Lightning Bolt on the
/// opponent's triggers only the Debater. Targets are read as the spell
/// becomes cast (CR 115.1, 601.2c, 601.2i).
#[test]
fn event_alternatives_cast_spell_targeting_a_filtered_creature() {
    let mut g = Game::with_trace(
        prepare(
            board(&[MOCKINGBIRD, REHEARSED_DEBATER], TurnStep::PrecombatMain)
                .permanent_or_hand("Mockingbird, Ace Agent", 0, 0, false)
                .permanent_or_hand("Rehearsed Debater", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (mockingbird, debater, mine, theirs) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
        Driver::object(&g, 3),
    );
    assert!(matches!(
        g.definition(CardId(0)).unwrap().triggered[0].condition.pattern(),
        EventPattern::Cast { target, .. } if target.reads_characteristics()
    ));
    for (card, target, land, expected) in [(4, mine, 6, (1, 1)), (5, theirs, 7, (1, 2))] {
        Driver::cast(&mut g, CardId(card));
        Driver::pick_object(&mut g, target);
        Driver::pay(&mut g, &[land]);
        drive(&mut g, &[], triggered);
        // The pending trigger retains the targeted permanents' facts.
        checked(&g);
        settle(&mut g);
        assert_eq!(
            (collected(&g, mockingbird), collected(&g, debater)),
            expected
        );
    }
    assert_eq!(g.current(mockingbird).unwrap().plus_one_counters, 1.into());
    assert!(g.current(theirs).is_err());
    assert_recoverable((&g).into());
}

const IMPERIAL_COSMOGRAPHER: &str = record!("b2aa052c-08b7-4334-9ea7-e78da57ff9ea");

/// Imperial Cosmographer: "Whenever another creature you control leaves
/// the battlefield without dying, put two +1/+1 counters on this creature."
/// Returning one Grizzly Bears to hand triggers it; Lightning Bolt killing
/// the other does not (CR 603.6c, 700.4).
#[test]
fn event_alternatives_leaves_without_dying() {
    let mut g = Game::with_trace(
        prepare(
            board(&[IMPERIAL_COSMOGRAPHER], TurnStep::PrecombatMain)
                .permanent_or_hand("Imperial Cosmographer", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Unsummon", 0, 0, true)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (cosmographer, bounced, killed) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
    );
    for (card, target, land) in [(3, bounced, 5), (4, killed, 6)] {
        Driver::cast(&mut g, CardId(card));
        Driver::pick_object(&mut g, target);
        Driver::pay(&mut g, &[land]);
        settle(&mut g);
        assert!(g.current(target).is_err());
    }
    assert_eq!(collected(&g, cosmographer), 1);
    assert_eq!(
        g.current(cosmographer).unwrap().plus_one_counters,
        2.into()
    );
    assert_recoverable((&g).into());
}
