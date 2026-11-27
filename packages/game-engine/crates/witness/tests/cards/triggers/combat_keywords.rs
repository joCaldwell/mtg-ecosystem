//! Combat-triggered keywords lowered to their defining triggers, from exact
//! frozen R5 source on private boards: CR 702.83 (exalted).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const IGNOBLE_HIERARCH: &str = record!("c8de43a3-ebd3-4000-b343-a6ffed11d34d");
const NOBLE_HIERARCH: &str = record!("98aa9424-5912-4bd6-9300-b3972a31d8af");
const CATHEDRAL_OF_WAR: &str = record!("5ff647e4-730a-498f-8f2c-5bd64d5a9780");

fn power_toughness(g: &Game, object: ObjectId) -> (Option<Quantity>, Option<Quantity>) {
    let c = g.characteristics(object).unwrap();
    (c.power, c.toughness)
}

fn pt(power: i64, toughness: i64) -> (Option<Quantity>, Option<Quantity>) {
    (Some(power.into()), Some(toughness.into()))
}

/// Declare `attacks` and stop with the declaration's triggers on the stack
/// (or none), answering every other decision with the default.
fn declare(g: &mut Game, attacks: &[(ObjectId, PlayerId)]) {
    let mut declared = false;
    for _ in 0..256 {
        let d = Driver::request(g, u64::MAX);
        if declared && d.reason == Reason::Priority {
            return;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => {
                declared = true;
                d.attack(attacks.to_vec())
            }
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
            },
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the attack was never declared");
}

fn triggers_of(g: &Game, source: ObjectId) -> usize {
    g.state
        .rules
        .stack
        .iter()
        .filter(|s| s.source == source && matches!(s.kind, StackKind::Triggered { .. }))
        .count()
}

/// Four seats. Grizzly Bears attacks alone: Ignoble Hierarch's and Cathedral
/// of War's exalted each trigger (a land's too, CR 702.83a) and Bears gets
/// +2/+2. The opponent's Noble Hierarch does not trigger: the attacker is
/// not a creature its controller controls. Each instance triggers
/// separately (CR 702.83b).
#[test]
fn exalted_instances_each_pump_the_creature_attacking_alone() {
    let mut g = Game::with_trace(
        prepare(
            board(
                &[IGNOBLE_HIERARCH, NOBLE_HIERARCH, CATHEDRAL_OF_WAR],
                TurnStep::BeginningCombat,
            )
            .seats(4)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Ignoble Hierarch", 0, 0, false)
            .permanent_or_hand("Cathedral of War", 0, 0, false)
            .permanent_or_hand("Noble Hierarch", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let (bears, ignoble, cathedral, noble) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
        Driver::object(&g, 3),
    );
    declare(&mut g, &[(bears, PlayerId(2))]);
    assert_eq!(triggers_of(&g, ignoble), 1);
    assert_eq!(triggers_of(&g, cathedral), 1);
    assert_eq!(triggers_of(&g, noble), 0);
    checked(&g);
    settle(&mut g);
    assert_eq!(power_toughness(&g, bears), pt(4, 4));
    assert_eq!(power_toughness(&g, ignoble), pt(0, 1));
    assert_recoverable((&g).into());
}

/// Two attackers: neither attacks alone (CR 506.5), so no exalted
/// ability triggers.
#[test]
fn exalted_does_not_trigger_when_two_creatures_attack() {
    let mut g = Game::with_trace(
        prepare(
            board(&[IGNOBLE_HIERARCH], TurnStep::BeginningCombat)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Ignoble Hierarch", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (first, second) = (Driver::object(&g, 0), Driver::object(&g, 1));
    declare(&mut g, &[(first, PlayerId(1)), (second, PlayerId(1))]);
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(power_toughness(&g, first), pt(2, 2));
    assert_eq!(power_toughness(&g, second), pt(2, 2));
    assert_recoverable((&g).into());
}
