//! Combat-triggered keywords lowered to their defining triggers, from exact
//! frozen R5 source on private boards: CR 702.83 (exalted), 702.116
//! (myriad), 702.105 (dethrone), 702.149 (training), 702.130 (afflict),
//! 702.23 (rampage), 702.121 (melee), 702.91 (battle cry) and 702.25
//! (flanking).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const IGNOBLE_HIERARCH: &str = record!("c8de43a3-ebd3-4000-b343-a6ffed11d34d");
const NOBLE_HIERARCH: &str = record!("98aa9424-5912-4bd6-9300-b3972a31d8af");
const CATHEDRAL_OF_WAR: &str = record!("5ff647e4-730a-498f-8f2c-5bd64d5a9780");
const BANSHEE_OF_THE_DREAD_CHOIR: &str = record!("282b4dbf-ec00-48ba-8010-73bee489fa7a");
const SCURRY_OF_SQUIRRELS: &str = record!("3beed054-a099-4887-b1bd-39cf9e514e94");
const CALLER_OF_THE_PACK: &str = record!("3b6cbd2a-02ba-4afb-a081-0f25d830a8ee");
const GRENZOS_CUTTHROAT: &str = record!("9a133991-8a7a-4bfd-b0ae-d76433450c03");
const APPRENTICE_SHARPSHOOTER: &str = record!("40d6e1ff-995b-42fc-8146-8d5e5f1adcd9");
const GRYFF_RIDER: &str = record!("e1ca5c24-bbd8-4eed-bb2b-17163f90e72b");
const KHENRA_ETERNAL: &str = record!("91b46b22-8062-434b-8432-5ed1062ccaf9");
const AERATHI_BERSERKER: &str = record!("5cb495a2-c683-4066-b6ee-d0b7d8843cb9");
const DEPUTIZED_PROTESTER: &str = record!("09947592-1730-4c1c-bfa5-be53a68d6919");
const ACCORDER_PALADIN: &str = record!("6891c8b7-7112-40dc-8b94-b84ad1885e5e");
const BENALISH_CAVALRY: &str = record!("0bf619f9-4b19-404c-995f-f23a4b859dd4");
const ALLEY_GRIFTERS: &str = record!("399af8ea-7a6f-4f43-bd39-dc29a6e75e06");
const GARRUKS_UPRISING: &str = record!("3127ae9b-a7a7-43ec-89d7-688f8445b33d");
const SUMALA_RUMBLERS: &str = record!("09189f67-b33f-4775-bfd5-862154da7526");
const MARAXUS_OF_KELD: &str = record!("af4b67c5-53dc-4e4d-9ec4-fa51cc3cd49a");

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

fn plus_one(g: &Game, object: ObjectId) -> Quantity {
    g.current(object).unwrap().plus_one_counters.clone()
}

/// Four seats at 40, 40, 30 and 40 life. Grenzo's Cutthroat attacking seat
/// 1, tied with seats 0 and 3 for the most life, gets a +1/+1 counter;
/// attacking seat 2 it gets none (CR 702.105a, read at the declaration).
#[test]
fn dethrone_counts_only_an_attack_on_the_player_with_the_most_life() {
    for (defender, counters) in [(1, 1), (2, 0)] {
        let mut g = Game::with_trace(
            prepare(
                board(&[GRENZOS_CUTTHROAT], TurnStep::BeginningCombat)
                    .seats(4)
                    .life(PlayerId(2), Quantity::from(30))
                    .permanent_or_hand("Grenzo's Cutthroat", 0, 0, false),
            ),
            TraceMode::Detailed,
        );
        let cutthroat = Driver::object(&g, 0);
        declare(&mut g, &[(cutthroat, PlayerId(defender))]);
        assert_eq!(triggers_of(&g, cutthroat), counters);
        settle(&mut g);
        assert_eq!(plus_one(&g, cutthroat), Quantity::from(counters as i64));
        assert_recoverable((&g).into());
    }
}

/// Apprentice Sharpshooter (1/4) and Gryff Rider (2/1) attack with Grizzly
/// Bears (2/2): only the Sharpshooter attacks with a creature of greater
/// power, so only it trains (CR 702.149a); Gryff Rider's equal-power
/// companion does not count.
#[test]
fn training_needs_another_attacker_with_greater_power() {
    let mut g = Game::with_trace(
        prepare(
            board(
                &[APPRENTICE_SHARPSHOOTER, GRYFF_RIDER],
                TurnStep::BeginningCombat,
            )
            .permanent_or_hand("Apprentice Sharpshooter", 0, 0, false)
            .permanent_or_hand("Gryff Rider", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (sharpshooter, rider, bears) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
    );
    declare(
        &mut g,
        &[
            (sharpshooter, PlayerId(1)),
            (rider, PlayerId(1)),
            (bears, PlayerId(1)),
        ],
    );
    assert_eq!(triggers_of(&g, sharpshooter), 1);
    assert_eq!(triggers_of(&g, rider), 0);
    checked(&g);
    settle(&mut g);
    assert_eq!(plus_one(&g, sharpshooter), Quantity::from(1));
    assert_eq!(plus_one(&g, rider), Quantity::zero());
    assert_recoverable((&g).into());
}

/// Drive a combat with explicit blocks by `blocks`, stopping at the first
/// priority in the declare blockers step with an empty stack.
fn blocked_combat(g: &mut Game, attacks: &[(ObjectId, PlayerId)], blocks: &[(ObjectId, ObjectId)]) {
    for _ in 0..512 {
        let d = Driver::request(g, u64::MAX);
        if g.state.rules.turn.step == TurnStep::DeclareBlockers
            && d.reason == Reason::Priority
            && g.state.rules.stack.is_empty()
        {
            return;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(attacks.to_vec()),
            Reason::Blockers => d.block(
                blocks
                    .iter()
                    .copied()
                    .filter(|(b, _)| g.current(*b).unwrap().controller == d.player)
                    .collect(),
            ),
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
            },
            Reason::Discard | Reason::ResolveDiscard => {
                d.pick_one(&d.candidates[0].clone()).unwrap()
            }
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the blocks were never declared");
}

/// Four seats. Two Khenra Eternals ("Afflict 1") attack seats 1 and 2; seat
/// 2 blocks its attacker. Only that Eternal's defending player, seat 2,
/// loses 1 life (CR 702.130a, 506.2); seat 1, not blocking, loses none.
#[test]
fn afflict_makes_the_blocking_defender_lose_life() {
    let mut g = Game::with_trace(
        prepare(
            board(&[KHENRA_ETERNAL], TurnStep::BeginningCombat)
                .seats(4)
                .permanent_or_hand("Khenra Eternal", 0, 0, false)
                .permanent_or_hand("Khenra Eternal", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 2, 2, false),
        ),
        TraceMode::Detailed,
    );
    let (first, second, bears) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
    );
    let life = |g: &Game, p: usize| g.state.rules.players[p].life.clone();
    let before = [life(&g, 1), life(&g, 2)];
    blocked_combat(
        &mut g,
        &[(first, PlayerId(1)), (second, PlayerId(2))],
        &[(bears, second)],
    );
    let mut lost = before[1].clone();
    lost.subtract(&Quantity::from(1));
    assert_eq!([life(&g, 1), life(&g, 2)], [before[0].clone(), lost]);
    checked(&g);
    assert_recoverable((&g).into());
}

/// Aerathi Berserker ("Rampage 3", 2/4) is blocked by three Grizzly Bears:
/// two blockers beyond the first give +6/+6, counted once as the trigger
/// resolves (CR 702.23a-b). Blocked by one, it gets nothing.
#[test]
fn rampage_counts_blockers_beyond_the_first() {
    for (blockers, expected) in [(3, pt(8, 10)), (1, pt(2, 4))] {
        let mut b = board(&[AERATHI_BERSERKER], TurnStep::BeginningCombat).permanent_or_hand(
            "Aerathi Berserker",
            0,
            0,
            false,
        );
        for _ in 0..blockers {
            b = b.permanent_or_hand("Grizzly Bears", 1, 1, false);
        }
        let mut g = Game::with_trace(prepare(b), TraceMode::Detailed);
        let berserker = Driver::object(&g, 0);
        let blocks: Vec<_> = (1..=blockers)
            .map(|i| (Driver::object(&g, i), berserker))
            .collect();
        blocked_combat(&mut g, &[(berserker, PlayerId(1))], &blocks);
        assert_eq!(power_toughness(&g, berserker), expected);
        checked(&g);
        assert_recoverable((&g).into());
    }
}

/// Four seats. Deputized Protester (2/1, melee) attacks seat 1 and Grizzly
/// Bears attacks seat 2: two opponents were attacked, so the Protester gets
/// +2/+2 (CR 702.121a). Attacking alone it gets +1/+1.
#[test]
fn melee_counts_each_opponent_attacked_this_combat() {
    for (with_bears, expected) in [(true, pt(4, 3)), (false, pt(3, 2))] {
        let mut g = Game::with_trace(
            prepare(
                board(&[DEPUTIZED_PROTESTER], TurnStep::BeginningCombat)
                    .seats(4)
                    .permanent_or_hand("Deputized Protester", 0, 0, false)
                    .permanent_or_hand("Grizzly Bears", 0, 0, false),
            ),
            TraceMode::Detailed,
        );
        let (protester, bears) = (Driver::object(&g, 0), Driver::object(&g, 1));
        let mut attacks = vec![(protester, PlayerId(1))];
        if with_bears {
            attacks.push((bears, PlayerId(2)));
        }
        declare(&mut g, &attacks);
        assert_eq!(triggers_of(&g, protester), 1);
        settle(&mut g);
        assert_eq!(power_toughness(&g, protester), expected);
        assert_recoverable((&g).into());
    }
}

/// Two Accorder Paladins (3/1, battle cry) attack with Grizzly Bears while a
/// second Bears stays home. Each battle cry gives each other attacking
/// creature +1/+0 (CR 702.91a-b): the Bears get +2/+0, each Paladin +1/+0
/// from the other and none from itself, the home Bears nothing.
#[test]
fn battle_cry_pumps_each_other_attacking_creature() {
    let mut g = Game::with_trace(
        prepare(
            board(&[ACCORDER_PALADIN], TurnStep::BeginningCombat)
                .permanent_or_hand("Accorder Paladin", 0, 0, false)
                .permanent_or_hand("Accorder Paladin", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let [first, second, bears, home] = [0, 1, 2, 3].map(|i| Driver::object(&g, i));
    declare(
        &mut g,
        &[
            (first, PlayerId(1)),
            (second, PlayerId(1)),
            (bears, PlayerId(1)),
        ],
    );
    assert_eq!(g.state.rules.stack.len(), 2);
    checked(&g);
    settle(&mut g);
    assert_eq!(power_toughness(&g, first), pt(4, 1));
    assert_eq!(power_toughness(&g, second), pt(4, 1));
    assert_eq!(power_toughness(&g, bears), pt(4, 2));
    assert_eq!(power_toughness(&g, home), pt(2, 2));
    assert_recoverable((&g).into());
}

/// Benalish Cavalry (2/2, flanking) is blocked by Grizzly Bears and by the
/// opponent's own Benalish Cavalry. Flanking triggers once, for the blocker
/// without flanking (CR 702.25a): the Bears become 1/1, the other Cavalry
/// stays 2/2. Damage then kills the Bears.
#[test]
fn flanking_shrinks_each_blocker_without_flanking() {
    let mut g = Game::with_trace(
        prepare(
            board(&[BENALISH_CAVALRY], TurnStep::BeginningCombat)
                .permanent_or_hand("Benalish Cavalry", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Benalish Cavalry", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let [cavalry, bears, other] = [0, 1, 2].map(|i| Driver::object(&g, i));
    assert!(
        g.definition(CardId(0))
            .unwrap()
            .keywords
            .contains(&KeywordAbility::Flanking)
    );
    blocked_combat(
        &mut g,
        &[(cavalry, PlayerId(1))],
        &[(bears, cavalry), (other, cavalry)],
    );
    assert_eq!(power_toughness(&g, bears), pt(1, 1));
    assert_eq!(power_toughness(&g, other), pt(2, 2));
    assert_eq!(power_toughness(&g, cavalry), pt(2, 2));
    checked(&g);
    assert_recoverable((&g).into());
}

/// Alley Grifters ("Whenever this creature becomes blocked, defending player
/// discards a card") reads the same becomes-blocked defending player as
/// afflict (CR 506.2): in four seats only the blocking seat 2 discards.
#[test]
fn becomes_blocked_defending_player_discards() {
    let mut g = Game::with_trace(
        prepare(
            board(&[ALLEY_GRIFTERS], TurnStep::BeginningCombat)
                .seats(4)
                .permanent_or_hand("Alley Grifters", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 2, 2, false)
                .permanent_or_hand("Island", 2, 2, true)
                .permanent_or_hand("Island", 1, 1, true),
        ),
        TraceMode::Detailed,
    );
    let [grifters, bears, held, other] = [0, 1, 2, 3].map(|i| Driver::object(&g, i));
    blocked_combat(&mut g, &[(grifters, PlayerId(2))], &[(bears, grifters)]);
    let zone = |object: ObjectId| g.state.rules.objects[&object.card].zone;
    assert_eq!(zone(held), Zone::Graveyard { owner: PlayerId(2) });
    assert_eq!(zone(other), Zone::Hand { owner: PlayerId(1) });
    checked(&g);
    assert_recoverable((&g).into());
}

/// Four seats. Sumala Rumblers (power = creatures you control) attacks with
/// Grizzly Bears beside it and Garruk's Uprising out. Myriad's two accepted
/// copies enter together (CR 608.2f, 702.116a), each seeing four creatures,
/// so each is a 4-power creature entering and the Uprising triggers twice.
/// Copies made one at a time would leave the first at 3 power.
#[test]
fn myriad_copies_enter_simultaneously() {
    let mut g = Game::with_trace(
        prepare(
            board(
                &[SUMALA_RUMBLERS, GARRUKS_UPRISING],
                TurnStep::BeginningCombat,
            )
            .seats(4)
            .permanent_or_hand("Sumala Rumblers", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Garruk's Uprising", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let [rumblers, _, uprising] = [0, 1, 2].map(|i| Driver::object(&g, i));
    let mut accept = std::collections::VecDeque::from([true, true]);
    combat(&mut g, &[(rumblers, PlayerId(1))], &mut accept, |g| {
        myriad_tokens(g).len() == 2 && !g.state.rules.pending_triggers.is_empty()
            || myriad_tokens(g).len() == 2 && triggered(g)
    });
    let pending = g
        .state
        .rules
        .pending_triggers
        .iter()
        .filter(|t| t.source == uprising)
        .count()
        + triggers_of(&g, uprising);
    assert_eq!(pending, 2, "one draw trigger per 4-power copy");
    for (token, _) in myriad_tokens(&g) {
        assert_eq!(power_toughness(&g, token), pt(4, 4));
    }
    // CR 111.1: both copies count as tokens created this turn.
    assert_eq!(g.state.rules.cast_history[0].tokens_created, 2);
    checked(&g);
    assert_recoverable((&g).into());
}

/// Apprentice Sharpshooter (1/4) attacks with Maraxus of Keld (power =
/// untapped artifacts, creatures and lands you control) beside one Forest.
/// Maraxus is 3 power before the declaration but 1 once both attackers have
/// tapped, tying the Sharpshooter, so training does not trigger (CR 508.1m,
/// 603.10, 702.149a).
#[test]
fn training_reads_power_after_the_attackers_tap() {
    let mut g = Game::with_trace(
        prepare(
            board(
                &[APPRENTICE_SHARPSHOOTER, MARAXUS_OF_KELD],
                TurnStep::BeginningCombat,
            )
            .permanent_or_hand("Apprentice Sharpshooter", 0, 0, false)
            .permanent_or_hand("Maraxus of Keld", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let [sharpshooter, maraxus] = [0, 1].map(|i| Driver::object(&g, i));
    assert_eq!(power_toughness(&g, maraxus), pt(3, 3));
    declare(
        &mut g,
        &[(sharpshooter, PlayerId(1)), (maraxus, PlayerId(1))],
    );
    assert_eq!(power_toughness(&g, maraxus), pt(1, 1));
    assert_eq!(triggers_of(&g, sharpshooter), 0);
    assert!(g.state.rules.stack.is_empty());
    assert_recoverable((&g).into());
}

// CR 508.6, 702.121a: attacking Jace still triggers melee. Only the optional
// direct attack on another opponent adds to its bonus.
#[test]
fn melee_counts_players_and_not_their_planeswalkers() {
    let jace = crate::planeswalker::JACE;
    for direct_attack in [false, true] {
        let mut g = Game::with_trace(
            prepare(
                board(&[DEPUTIZED_PROTESTER, jace], TurnStep::BeginningCombat)
                    .seats(3)
                    .permanent_or_hand("Deputized Protester", 0, 0, false)
                    .permanent_or_hand("Jace Beleren", 1, 1, false)
                    .permanent_or_hand("Grizzly Bears", 0, 0, false),
            ),
            TraceMode::Detailed,
        );
        let d = crate::evasion::until(&mut g, &Reason::Attackers);
        let protester = Driver::object(&g, 0);
        let defender = Candidate::Object {
            object: Driver::object(&g, 1),
        };
        let bear = Driver::object(&g, 2);
        let Shape::Pick { slots, .. } = &d.shape else {
            panic!("expected attack slots")
        };
        let picks = slots
            .iter()
            .map(|slot| {
                if slot.subject == d.index(&Candidate::Object { object: protester }) {
                    vec![d.index(&defender).unwrap()]
                } else if direct_attack
                    && slot.subject == d.index(&Candidate::Object { object: bear })
                {
                    vec![
                        d.index(&Candidate::Player {
                            player: PlayerId(2),
                        })
                        .unwrap(),
                    ]
                } else {
                    vec![]
                }
            })
            .collect();
        g.respond(d.player, d.id, Answer::Pick { picks }).unwrap();
        Driver::request(&mut g, u64::MAX);
        assert_eq!(triggers_of(&g, protester), 1);
        settle(&mut g);
        let bonus = i64::from(direct_attack);
        assert_eq!(power_toughness(&g, protester), pt(2 + bonus, 1 + bonus));
        checked(&g);
    }
}

// CR 702.116a, 508.4: two accepted copies enter together. One attacks
// Jace rather than his controller, and both leave at end of combat.
#[test]
fn myriad_copies_can_enter_attacking_a_planeswalker() {
    let mut g = Game::with_trace(
        prepare(
            board(
                &[BANSHEE_OF_THE_DREAD_CHOIR, crate::planeswalker::JACE],
                TurnStep::BeginningCombat,
            )
            .seats(4)
            .permanent_or_hand("Banshee of the Dread Choir", 0, 0, false)
            .permanent_or_hand("Jace Beleren", 2, 2, false),
        ),
        TraceMode::Detailed,
    );
    let banshee = Driver::object(&g, 0);
    let jace = Driver::object(&g, 1);
    declare(&mut g, &[(banshee, PlayerId(1))]);
    let accept = Candidate::Flag { flag: Flag::Accept };
    let d = crate::evasion::until(&mut g, &Reason::OptionalEffect);
    g.respond(d.player, d.id, d.pick_one(&accept).unwrap())
        .unwrap();
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::MyriadDefender);
    assert_eq!(
        d.candidates,
        vec![
            Candidate::Player {
                player: PlayerId(2)
            },
            Candidate::Object { object: jace }
        ]
    );
    checked(&g);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Object { object: jace }).unwrap(),
    )
    .unwrap();

    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::OptionalEffect);
    assert!(
        myriad_tokens(&g).is_empty(),
        "all copies enter after the choices"
    );
    checked(&g);
    g.respond(d.player, d.id, d.pick_one(&accept).unwrap())
        .unwrap();
    let mut choices = std::collections::VecDeque::new();
    combat(&mut g, &[], &mut choices, |g| {
        myriad_tokens(g).len() == 2 && g.state.rules.stack.is_empty()
    });
    let tokens = myriad_tokens(&g);
    for (token, defender) in &tokens {
        assert!(g.current(*token).unwrap().tapped);
        let attack = g
            .state
            .rules
            .combat
            .iter()
            .find(|a| a.attacker == *token)
            .unwrap();
        assert_eq!(
            attack.planeswalker,
            (*defender == Some(PlayerId(2))).then_some(jace)
        );
    }
    checked(&g);
    combat(&mut g, &[], &mut choices, |g| {
        g.state.rules.turn.step == TurnStep::EndCombat && triggered(g)
    });
    assert_eq!(
        g.state.rules.objects[&jace.card].zone,
        Zone::Graveyard { owner: PlayerId(2) }
    );
    assert_eq!(g.state.rules.players[2].life, 40.into());
    assert_eq!(g.state.rules.players[3].life, 36.into());
    combat(&mut g, &[], &mut choices, |g| {
        g.state.rules.turn.step == TurnStep::EndCombat && g.state.rules.stack.is_empty()
    });
    assert!(myriad_tokens(&g).is_empty());
    checked(&g);
}
