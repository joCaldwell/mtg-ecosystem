//! Frozen R5 witnesses for leaves-the-battlefield, block and damaged-
//! permanent events (CR 120.3, 509.3a, 509.3c, 603.6c, 603.10a).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const THRAGTUSK: &str = record!("0dd0e91a-d16b-4718-8d11-1a3fcf8e0753");
const CHAMBERED_NAUTILUS: &str = record!("0e1267d4-0155-4503-829a-03264c8d14a0");
const NOBLE_STAND: &str = record!("1aa1e34c-b2ff-4750-9e73-5707a8f49bfb");
const RAVENOUS_DAGGERTOOTH: &str = record!("f5ef25fb-545e-41b7-8fed-d3cfff3d8668");
const SATYR_HOPLITE: &str = record!("ee7dc820-10ee-429a-a1ef-ecd6a11f5608");

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

/// Thragtusk: "When this creature leaves the battlefield, create a 3/3
/// green Beast creature token." Returning it to hand is a departure that is
/// not a death; the ability looks back at the permanent (CR 603.10a).
#[test]
fn object_events_bounce_triggers_leaves_the_battlefield() {
    let mut g = Game::with_trace(
        prepare(
            board(&[THRAGTUSK], TurnStep::PrecombatMain)
                .permanent_or_hand("Thragtusk", 0, 0, false)
                .permanent_or_hand("Unsummon", 1, 1, true)
                .permanent_or_hand("Island", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let thragtusk = Driver::object(&g, 0);
    assert!(matches!(
        g.definition(CardId(0)).unwrap().triggered[1]
            .condition
            .pattern(),
        EventPattern::LeavesBattlefield { .. }
    ));
    // The opponent bounces it at instant speed after the active player passes.
    drive(&mut g, &[], |g| g.state.rules.priority == PlayerId(1));
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, thragtusk);
    Driver::pay(&mut g, &[2]);
    drive(&mut g, &[], triggered);
    checked(&g);
    settle(&mut g);
    assert!(g.current(thragtusk).is_err());
    assert_eq!(tokens(&g, 0), 1);
    assert_recoverable((&g).into());
}

/// Ravenous Daggertooth: "Whenever this creature is dealt damage, you gain
/// 2 life." Lightning Bolt deals it lethal damage; the ability triggers as
/// the damage is dealt, before the creature dies.
#[test]
fn object_events_damage_to_a_creature_triggers() {
    let mut g = Game::with_trace(
        prepare(
            board(&[RAVENOUS_DAGGERTOOTH], TurnStep::PrecombatMain)
                .permanent_or_hand("Ravenous Daggertooth", 0, 0, false)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let daggertooth = Driver::object(&g, 0);
    let before = life(&g, 0);
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, daggertooth);
    Driver::pay(&mut g, &[2]);
    settle(&mut g);
    assert!(g.current(daggertooth).is_err());
    assert_eq!(life(&g, 0), plus(&before, 2));
    assert_recoverable((&g).into());
}

/// Chambered Nautilus ("Whenever this creature becomes blocked, you may
/// draw a card") attacks and the opponent's Grizzly Bears blocks it, which
/// triggers the opponent's Noble Stand ("Whenever a creature you control
/// blocks, you gain 2 life"). Both events come from the one declaration.
#[test]
fn object_events_block_and_becomes_blocked_trigger() {
    let mut g = Game::with_trace(
        prepare(
            board(
                &[CHAMBERED_NAUTILUS, NOBLE_STAND],
                TurnStep::BeginningCombat,
            )
            .permanent_or_hand("Chambered Nautilus", 0, 0, false)
            .permanent_or_hand("Noble Stand", 1, 1, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let (nautilus, bears) = (Driver::object(&g, 0), Driver::object(&g, 2));
    let (cards, life_before) = (hand(&g, 0), life(&g, 1));
    for _ in 0..64 {
        let d = Driver::request(&mut g, u64::MAX);
        if g.state.rules.stack.len() == 2 {
            break;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(vec![(nautilus, PlayerId(1))]),
            Reason::Blockers => d.block(vec![(bears, nautilus)]),
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    assert_eq!(g.state.rules.stack.len(), 2);
    checked(&g);
    settle(&mut g);
    assert_eq!(hand(&g, 0), cards + 1);
    assert_eq!(life(&g, 1), plus(&life_before, 2));
    assert_recoverable((&g).into());
}

/// Satyr Hoplite: "Heroic — Whenever you cast a spell that targets this
/// creature, put a +1/+1 counter on this creature." A Giant Growth on the
/// Grizzly Bears does not trigger it; one on the Hoplite does, from the
/// targets the spell had as it became cast (CR 601.2c, 601.2i).
#[test]
fn object_events_heroic_reads_the_cast_spells_targets() {
    let mut g = Game::with_trace(
        prepare(
            board(&[SATYR_HOPLITE], TurnStep::PrecombatMain)
                .permanent_or_hand("Satyr Hoplite", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (hoplite, bears) = (Driver::object(&g, 0), Driver::object(&g, 1));
    for (card, target, land, expected) in [(2, bears, 4, 0), (3, hoplite, 5, 1)] {
        Driver::cast(&mut g, CardId(card));
        Driver::pick_object(&mut g, target);
        Driver::pay(&mut g, &[land]);
        if expected == 1 {
            drive(&mut g, &[], triggered);
            checked(&g);
        }
        settle(&mut g);
        assert_eq!(
            g.current(hoplite).unwrap().plus_one_counters,
            expected.into()
        );
    }
    assert_recoverable((&g).into());
}
