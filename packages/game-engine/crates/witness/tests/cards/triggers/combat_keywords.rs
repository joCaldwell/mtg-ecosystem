//! Combat-triggered keywords lowered to their defining triggers, from exact
//! frozen R5 source on private boards: CR 702.83 (exalted) and 702.116
//! (myriad).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const IGNOBLE_HIERARCH: &str = record!("c8de43a3-ebd3-4000-b343-a6ffed11d34d");
const NOBLE_HIERARCH: &str = record!("98aa9424-5912-4bd6-9300-b3972a31d8af");
const CATHEDRAL_OF_WAR: &str = record!("5ff647e4-730a-498f-8f2c-5bd64d5a9780");
const BANSHEE_OF_THE_DREAD_CHOIR: &str = record!("282b4dbf-ec00-48ba-8010-73bee489fa7a");
const SCURRY_OF_SQUIRRELS: &str = record!("3beed054-a099-4887-b1bd-39cf9e514e94");
const CALLER_OF_THE_PACK: &str = record!("3b6cbd2a-02ba-4afb-a081-0f25d830a8ee");

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

/// Drive a combat: declare `attacks`, answer optional effects from
/// `accept` in order, declare no blocks, order triggers as offered, and stop
/// when `stop` holds.
fn combat(
    g: &mut Game,
    attacks: &[(ObjectId, PlayerId)],
    accept: &mut std::collections::VecDeque<bool>,
    stop: impl Fn(&Game) -> bool,
) {
    for _ in 0..512 {
        let d = Driver::request(g, u64::MAX);
        if stop(g) {
            return;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(attacks.to_vec()),
            Reason::Blockers => d.block(vec![]),
            Reason::Targets { .. } => d.pick_one(&d.candidates[0].clone()).unwrap(),
            Reason::OptionalEffect => {
                let flag = if accept.pop_front().expect("an optional answer") {
                    Flag::Accept
                } else {
                    Flag::Decline
                };
                d.pick_one(&Candidate::Flag { flag }).unwrap()
            }
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
            },
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the combat never reached its stop");
}

/// The myriad token copies on the battlefield, with the player each attacks.
fn myriad_tokens(g: &Game) -> Vec<(ObjectId, Option<PlayerId>)> {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && g.state.rules.tokens.contains_key(&o.id.card))
        .map(|o| {
            (
                o.id,
                g.state
                    .rules
                    .combat
                    .iter()
                    .find(|a| a.attacker == o.id)
                    .map(|a| a.defender),
            )
        })
        .collect()
}

/// Four seats. Banshee of the Dread Choir attacks seat 1. Myriad offers a
/// copy for each other opponent, seat 2 then seat 3 (never the defending
/// player); accepting seat 2 and declining seat 3 creates one token copy,
/// tapped and attacking seat 2 (CR 702.116a, 508.4). The token was never
/// declared, so its own myriad does not trigger (CR 508.4). Both deal
/// combat damage to their players; at end of combat the token is exiled.
#[test]
fn myriad_copies_attack_other_opponents_until_end_of_combat() {
    let mut g = Game::with_trace(
        prepare(
            board(&[BANSHEE_OF_THE_DREAD_CHOIR], TurnStep::BeginningCombat)
                .seats(4)
                .permanent_or_hand("Banshee of the Dread Choir", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let banshee = Driver::object(&g, 0);
    let mut accept = std::collections::VecDeque::from([true, false]);
    combat(&mut g, &[(banshee, PlayerId(1))], &mut accept, |g| {
        !myriad_tokens(g).is_empty() && g.state.rules.stack.is_empty()
    });
    assert!(accept.is_empty(), "exactly two offers");
    let [(token, Some(defender))] = myriad_tokens(&g)[..] else {
        panic!("one attacking token: {:?}", myriad_tokens(&g));
    };
    assert_eq!(defender, PlayerId(2));
    assert!(g.current(token).unwrap().tapped);
    assert_eq!(
        g.definition(token.card).unwrap().name,
        "Banshee of the Dread Choir"
    );
    assert_eq!(g.state.rules.resolution_triggers.len(), 1);
    checked(&g);
    assert_recoverable((&g).into());
    let life = |g: &Game, p: u8| g.state.rules.players[usize::from(p)].life.clone();
    let before = [life(&g, 1), life(&g, 2), life(&g, 3)];
    combat(&mut g, &[], &mut accept, |g| {
        g.state.rules.turn.step == TurnStep::EndCombat && triggered(g)
    });
    let mut dealt = before.clone();
    for (i, amount) in [4, 4, 0].into_iter().enumerate() {
        dealt[i].subtract(&Quantity::from(amount));
    }
    assert_eq!([life(&g, 1), life(&g, 2), life(&g, 3)], dealt);
    assert_eq!(myriad_tokens(&g).len(), 1);
    checked(&g);
    combat(&mut g, &[], &mut accept, |g| {
        g.state.rules.turn.step == TurnStep::EndCombat && g.state.rules.stack.is_empty()
    });
    assert!(myriad_tokens(&g).is_empty(), "the token was exiled");
    assert_eq!(g.current(banshee).unwrap().zone, Zone::Battlefield);
    assert_recoverable((&g).into());
}

/// Scurry of Squirrels prints "Myriad, myriad": each instance triggers
/// separately (CR 702.116b), so four seats and both accepted make two
/// copies for each other opponent, and each instance's delayed trigger
/// exiles its own two at end of combat.
#[test]
fn myriad_instances_each_create_and_exile_their_own_copies() {
    let mut g = Game::with_trace(
        prepare(
            board(&[SCURRY_OF_SQUIRRELS], TurnStep::BeginningCombat)
                .seats(4)
                .permanent_or_hand("Scurry of Squirrels", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let scurry = Driver::object(&g, 0);
    assert_eq!(g.definition(CardId(0)).unwrap().triggered.len(), 3);
    let mut accept = std::collections::VecDeque::from([true, true, true, true]);
    combat(&mut g, &[(scurry, PlayerId(3))], &mut accept, |g| {
        myriad_tokens(g).len() == 4 && g.state.rules.stack.is_empty()
    });
    let mut defenders: Vec<_> = myriad_tokens(&g).into_iter().map(|(_, d)| d).collect();
    defenders.sort();
    assert_eq!(
        defenders,
        [
            Some(PlayerId(1)),
            Some(PlayerId(1)),
            Some(PlayerId(2)),
            Some(PlayerId(2))
        ]
    );
    assert_eq!(g.state.rules.resolution_triggers.len(), 2);
    checked(&g);
    combat(&mut g, &[], &mut accept, |g| {
        g.state.rules.turn.step == TurnStep::EndCombat && g.state.rules.stack.len() == 2
    });
    checked(&g);
    assert_recoverable((&g).into());
    combat(&mut g, &[], &mut accept, |g| {
        g.state.rules.turn.step == TurnStep::EndCombat && g.state.rules.stack.len() == 1
    });
    assert_eq!(myriad_tokens(&g).len(), 2, "one instance's copies exiled");
    combat(&mut g, &[], &mut accept, |g| {
        g.state.rules.turn.step == TurnStep::EndCombat && g.state.rules.stack.is_empty()
    });
    assert!(myriad_tokens(&g).is_empty());
    assert_recoverable((&g).into());
}

/// Seats 2 and 3 have lost (CR 800.4a): the only opponent left is the
/// defending player, so myriad offers nothing, creates nothing and creates
/// no delayed trigger.
#[test]
fn myriad_offers_nothing_without_another_opponent() {
    let mut g = Game::with_trace(
        prepare(
            board(&[CALLER_OF_THE_PACK], TurnStep::BeginningCombat)
                .life(PlayerId(2), Quantity::zero())
                .life(PlayerId(3), Quantity::zero())
                .permanent_or_hand("Caller of the Pack", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let caller = Driver::object(&g, 0);
    let mut accept = std::collections::VecDeque::new();
    combat(&mut g, &[(caller, PlayerId(1))], &mut accept, |g| {
        g.state.rules.turn.step == TurnStep::DeclareBlockers
    });
    assert!(!g.is_live(PlayerId(2)) && !g.is_live(PlayerId(3)));
    assert!(myriad_tokens(&g).is_empty());
    assert!(g.state.rules.resolution_triggers.is_empty());
    assert_recoverable((&g).into());
}

/// Myriad's copies enter in one event with no battlefield-entry stage, so a
/// myriad card whose own entry replacement would modify that entry (CR
/// 614.1c, 614.12) is rejected by name rather than lowered without it.
#[test]
fn myriad_rejects_entry_replacements_by_name() {
    use mtg_contracts::kir::{Cell, definition_cells};
    let [definition] = &compiled(&[BANSHEE_OF_THE_DREAD_CHOIR], true)[..] else {
        unreachable!()
    };
    assert!(definition.triggered[0].is_myriad());
    assert!(
        definition_cells(definition)
            .unwrap()
            .contains(&Cell::Myriad)
    );
    for effect in [
        StaticEffect::EntersTapped,
        StaticEffect::EntersWithPlusOneCounters {
            count: Quantity::from(1),
        },
    ] {
        let mut d = definition.clone();
        d.static_abilities.push(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        });
        let error = definition_cells(&d).unwrap_err();
        assert!(format!("{error:?}").contains("myriad"), "{error:?}");
    }
}
