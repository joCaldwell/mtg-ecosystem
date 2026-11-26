//! Resolution-time counts from exact frozen R5 source on private boards:
//! "where X is [a number]" and "+N/+N for each [filter]" are read as the
//! printed instruction is performed (CR 107.3, 608.2h), after the earlier
//! instructions of the same spell and after anything that resolved in
//! between casting and resolution. A P/T change keeps the value it read
//! (CR 611.2c). Every resolution is checked for state validity, a snapshot
//! round trip and journal replay at every quantum.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Cell, definition_cells};

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const CHAIN_REACTION: &str = record!("086b2564-9114-4ba2-94fd-b490f98f38a7");
const HELLKITE_IGNITER: &str = record!("12aba1d4-477f-4d36-96bb-c3c7bbd51b75");
const CONFRONT_THE_UNKNOWN: &str = record!("221240c5-3c97-438c-b906-2508eacf190f");
const SHAMANIC_REVELATION: &str = record!("d1d171de-1c6d-4fb9-817a-9c689c709f3d");
const OVERWHELMING_STAMPEDE: &str = record!("e1e96802-fd0c-41f1-aa21-3287d75a0e88");
const BECOME_THE_AVALANCHE: &str = record!("e5fcbbaf-9431-4fa8-ac11-12ace1f2054c");
const SANGUIMANCY: &str = record!("f0a85525-b174-41d9-8871-870af29d3480");

const P0: PlayerId = PlayerId(0);

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen resolution-count witness failed: {result:?}");
    };
    *definition
}

/// Seat 0 holds `card` (card 0) and then `hand`; it controls `lands` basic
/// lands of `land` (cards 1..=lands after the hand) and `mine`; seat 1
/// controls `theirs`.
struct Setup<'a> {
    card: &'a str,
    hand: &'a [&'a str],
    land: &'a str,
    mine: &'a [&'a str],
    theirs: &'a [&'a str],
}

/// The game and the ids of the lands that pay for card 0.
fn game(setup: Setup<'_>) -> (Game, Vec<u32>) {
    game_with(lowered(setup.card), setup)
}

fn game_with(definition: Definition, setup: Setup<'_>) -> (Game, Vec<u32>) {
    let name = definition.name.clone();
    let cost = &definition.characteristics.mana_cost;
    let lands = cost.colored.len() + cost.generic.to_u32().unwrap() as usize;
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut b = Board::new(catalog, "r5-resolution-count-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .permanent_or_hand(&name, 0, 0, true);
    for card in setup.hand {
        b = b.permanent_or_hand(card, 0, 0, true);
    }
    for _ in 0..lands {
        b = b.permanent_or_hand(setup.land, 0, 0, false);
    }
    for card in setup.mine {
        b = b.permanent_or_hand(card, 0, 0, false);
    }
    for card in setup.theirs {
        b = b.permanent_or_hand(card, 1, 1, false);
    }
    let first = 1 + setup.hand.len() as u32;
    (
        Game::new(b.libraries("Island", 6).seed([5; 32]).prepare()),
        (first..first + lands as u32).collect(),
    )
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// Pass priority until the stack is empty, then check and replay.
fn resolve(g: &mut Game) {
    for _ in 0..16 {
        let d = Driver::request(g, u64::MAX);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        if g.state.rules.stack.is_empty() {
            break;
        }
        Driver::pass(g);
    }
    assert!(g.state.rules.stack.is_empty());
    checked(g);
    assert_recoverable((&*g).into());
}

fn cast(g: &mut Game, lands: &[u32], target: Option<ObjectId>) {
    Driver::cast(g, CardId(0));
    if let Some(object) = target {
        Driver::pick_object(g, object);
    }
    Driver::pay(g, lands);
}

fn hand(g: &Game) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == (Zone::Hand { owner: P0 }))
        .count()
}

fn stats(g: &Game, card: u32) -> (Quantity, Quantity) {
    let c = g.characteristics(Driver::object(g, card)).unwrap();
    (c.power.unwrap(), c.toughness.unwrap())
}

fn pt(power: i64, toughness: i64) -> (Quantity, Quantity) {
    (power.into(), toughness.into())
}

/// Chain Reaction: "deals X damage to each creature, where X is the number
/// of creatures on the battlefield." Four creatures exist when it is cast;
/// a Lightning Bolt cast in response kills one, so X is three when Chain
/// Reaction resolves (CR 608.2h).
#[test]
fn chain_reaction_counts_creatures_as_it_resolves_not_as_it_is_cast() {
    let (mut g, lands) = game(Setup {
        card: CHAIN_REACTION,
        hand: &["Lightning Bolt"],
        land: "Mountain",
        mine: &["Mountain", "Colossal Dreadmaw"],
        theirs: &["Grizzly Bears", "Grizzly Bears", "Grizzly Bears"],
    });
    let (bolt_land, dreadmaw, bear) = (6, 7, 8);
    cast(&mut g, &lands, None);
    Driver::cast(&mut g, CardId(1));
    let bear = Driver::object(&g, bear);
    Driver::pick_object(&mut g, bear);
    Driver::pay(&mut g, &[bolt_land]);
    resolve(&mut g);
    let dreadmaw = g.current(Driver::object(&g, dreadmaw)).unwrap();
    assert_eq!(dreadmaw.zone, Zone::Battlefield);
    assert_eq!(dreadmaw.marked_damage, 3.into());
    for bear in 8..11 {
        let zone = g.current(Driver::object(&g, bear)).unwrap().zone;
        assert_eq!(zone, Zone::Graveyard { owner: PlayerId(1) });
    }
}

/// Overwhelming Stampede: "creatures you control gain trample and get
/// +X/+X, where X is the greatest power among creatures you control." X is
/// read once; the change does not grow as it raises that power (CR 611.2c).
#[test]
fn overwhelming_stampede_reads_the_greatest_power_once() {
    let (mut g, lands) = game(Setup {
        card: OVERWHELMING_STAMPEDE,
        hand: &[],
        land: "Forest",
        mine: &["Grizzly Bears", "Colossal Dreadmaw"],
        theirs: &["Colossal Dreadmaw", "Grizzly Bears"],
    });
    assert!(
        definition_cells(&lowered(OVERWHELMING_STAMPEDE))
            .unwrap()
            .contains(&Cell::PumpCounted)
    );
    cast(&mut g, &lands, None);
    resolve(&mut g);
    assert_eq!(stats(&g, 6), pt(8, 8));
    assert_eq!(stats(&g, 7), pt(12, 12));
    assert_eq!(stats(&g, 8), pt(6, 6));
    assert_eq!(stats(&g, 9), pt(2, 2));
}

/// Become the Avalanche: "Draw a card for each creature you control with
/// power 4 or greater. Then creatures you control get +X/+X until end of
/// turn, where X is the number of cards in your hand." The second
/// instruction counts the hand after the first one drew.
#[test]
fn become_the_avalanche_counts_the_hand_after_its_own_draw() {
    let (mut g, lands) = game(Setup {
        card: BECOME_THE_AVALANCHE,
        hand: &["Lightning Bolt", "Lightning Bolt"],
        land: "Forest",
        mine: &["Colossal Dreadmaw", "Grizzly Bears"],
        theirs: &["Colossal Dreadmaw"],
    });
    cast(&mut g, &lands, None);
    resolve(&mut g);
    assert_eq!(hand(&g), 3);
    assert_eq!(stats(&g, 9), pt(9, 9));
    assert_eq!(stats(&g, 10), pt(5, 5));
    assert_eq!(stats(&g, 11), pt(6, 6));
}

/// Confront the Unknown: "Investigate, then target creature gets +1/+1
/// until end of turn for each Clue you control." The Clue the first
/// instruction created is counted.
#[test]
fn confront_the_unknown_counts_the_clue_it_just_created() {
    let (mut g, lands) = game(Setup {
        card: CONFRONT_THE_UNKNOWN,
        hand: &[],
        land: "Forest",
        mine: &["Grizzly Bears"],
        theirs: &[],
    });
    let bear = Driver::object(&g, 2);
    cast(&mut g, &lands, Some(bear));
    resolve(&mut g);
    assert_eq!(stats(&g, 2), pt(3, 3));
}

/// Shamanic Revelation: "Draw a card for each creature you control. /
/// Ferocious — You gain 4 life for each creature you control with power 4
/// or greater." With no creatures both counts are zero: nothing is drawn
/// and no life changes (CR 119.8).
#[test]
fn shamanic_revelation_multiplies_its_counts_and_does_nothing_for_zero() {
    for (mine, drawn, gained) in [
        (&[][..], 0, 0),
        (&["Colossal Dreadmaw", "Grizzly Bears"][..], 2, 4),
    ] {
        let (mut g, lands) = game(Setup {
            card: SHAMANIC_REVELATION,
            hand: &[],
            land: "Forest",
            mine,
            theirs: &["Colossal Dreadmaw"],
        });
        let mut life = g.state.rules.players[0].life.clone();
        life.add(&Quantity::from(gained));
        cast(&mut g, &lands, None);
        resolve(&mut g);
        assert_eq!(hand(&g), drawn, "{mine:?}");
        assert_eq!(g.state.rules.players[0].life, life, "{mine:?}");
    }
}

/// Sanguimancy: "You draw X cards and you lose X life, where X is your
/// devotion to black." Both instructions read the same devotion: the
/// counted draws change neither the battlefield nor a mana cost.
#[test]
fn sanguimancy_draws_and_loses_its_devotion() {
    let (mut g, lands) = game(Setup {
        card: SANGUIMANCY,
        hand: &[],
        land: "Swamp",
        mine: &["Vampire Nighthawk", "Typhoid Rats"],
        theirs: &["Vampire Nighthawk"],
    });
    let mut life = g.state.rules.players[0].life.clone();
    life.subtract(&Quantity::from(3));
    cast(&mut g, &lands, None);
    resolve(&mut g);
    assert_eq!(hand(&g), 3);
    assert_eq!(g.state.rules.players[0].life, life);
}

/// Hellkite Igniter: "{1}{R}: This creature gets +X/+0 until end of turn,
/// where X is the number of artifacts you control." The counted source
/// change opens the counted P/T cell; no announced X is read.
#[test]
fn hellkite_igniter_counts_artifacts_for_its_own_pump() {
    let d = lowered(HELLKITE_IGNITER);
    assert!(definition_cells(&d).unwrap().contains(&Cell::PumpCounted));
    let json = serde_json::to_string(&d.activated).unwrap();
    assert!(json.contains(r#""delta":"counted""#), "{json}");
    assert!(!json.contains("announced_x"), "{json}");
}

/// A synthetic program: "deals X damage to each creature, where X is the
/// greatest power among creatures you control. Creatures you control get
/// +1/+1 until end of turn." The damage's recorded X stays valid after the
/// later pump raises that power (CR 608.2h); every quantum re-validates it.
#[test]
fn recorded_set_damage_count_survives_a_later_change_to_its_source() {
    let mut definition = lowered(CHAIN_REACTION);
    let program = definition.spell.as_mut().unwrap();
    let json = serde_json::to_string(&program.instructions).unwrap();
    let count = r#"{"amount":"count","set":{"set":"matching","zone":{"zone":"battlefield"},"filter":[{"atom":"type","kind":"creature"}]}}"#;
    let mine = r#"[{"atom":"type","kind":"creature"},{"atom":"controlled_by","player":{"player":"is","who":{"ref":"controller","role":"spell_controller"}}}]"#;
    let greatest = format!(
        r#"{{"amount":"greatest","set":{{"set":"matching","zone":{{"zone":"battlefield"}},"filter":{mine}}},"stat":"power"}}"#
    );
    assert!(json.contains(count), "{json}");
    program.instructions = serde_json::from_str(&json.replace(count, &greatest)).unwrap();
    let mine: mtg_contracts::kir::Pred = serde_json::from_str(mine).unwrap();
    program.instructions.push(Instruction::pump(
        mtg_contracts::kir::SetExpr::battlefield(mine),
        Quantity::from(1),
        Quantity::from(1),
        0,
    ));
    let (mut g, lands) = game_with(
        definition,
        Setup {
            card: CHAIN_REACTION,
            hand: &[],
            land: "Mountain",
            mine: &["Grizzly Bears", "Grizzly Bears"],
            theirs: &["Colossal Dreadmaw"],
        },
    );
    cast(&mut g, &lands, None);
    resolve(&mut g);
    let dreadmaw = g.current(Driver::object(&g, 7)).unwrap();
    assert_eq!(dreadmaw.marked_damage, 2.into());
    // Two damage, then +1/+1: each Bear survives as a 3/3.
    for bear in [5, 6] {
        let bear_now = g.current(Driver::object(&g, bear)).unwrap();
        assert_eq!(bear_now.zone, Zone::Battlefield);
        assert_eq!(bear_now.marked_damage, 2.into());
        assert_eq!(stats(&g, bear), pt(3, 3));
    }
}
