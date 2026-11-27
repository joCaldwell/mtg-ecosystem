//! "[One] for each [filter]" from exact frozen R5 source on private boards:
//! the count is the number of matching objects as the step is performed,
//! counted once (CR 608.2h). A counted draw is a sequence of single draws
//! (CR 121.2) whose count drawing cannot change. Every resolution is
//! checked for state validity, a snapshot round trip and journal replay,
//! and the game then continues to the next step.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Cell, Tiers, definition_tiers};

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const COLLECTIVE_UNCONSCIOUS: &str = record!("38abf769-e4ef-4b47-9c79-c8cda0b90f6c");
const FOLK_MEDICINE: &str = record!("135eb851-3f2d-45c5-bfb9-49d1e6c98344");
const GRIM_FLOWERING: &str = record!("65428247-b386-410c-91b7-81dd278c0927");
const KEEP_WATCH: &str = record!("847b5be5-cb0c-4857-bcd9-c5cfe55fdaeb");

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen for-each witness failed: {result:?}");
    };
    *definition
}

/// Seat 0 holds the frozen card and the Forests that pay for it; each seat
/// controls `creatures` Grizzly Bears, and seat 0's graveyard holds
/// `graveyard` creature cards and a Lightning Bolt.
fn game(raw: &str, creatures: [usize; 2], graveyard: usize) -> (Game, u32) {
    let definition = lowered(raw);
    assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
    let name = definition.name.clone();
    let cost = &definition.characteristics.mana_cost;
    let lands = cost.colored.len() as i64 + cost.generic.to_string().parse::<i64>().unwrap();
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut b = Board::new(catalog, "r4-for-each-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .permanent_or_hand(&name, 0, 0, true);
    for _ in 0..lands {
        b = b.permanent_or_hand("Forest", 0, 0, false);
    }
    for (seat, count) in creatures.into_iter().enumerate() {
        for _ in 0..count {
            b = b.permanent_or_hand("Grizzly Bears", seat as u8, seat as u8, false);
        }
    }
    let graveyard_zone = Zone::Graveyard { owner: PlayerId(0) };
    for _ in 0..graveyard {
        b = b.card("Grizzly Bears", 0, 0, graveyard_zone);
    }
    b = b.card("Lightning Bolt", 0, 0, graveyard_zone).card(
        "Grizzly Bears",
        1,
        1,
        Zone::Graveyard { owner: PlayerId(1) },
    );
    let lands = lands as u32;
    (
        Game::new(b.libraries("Island", 6).seed([3; 32]).prepare()),
        lands,
    )
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

fn hand(g: &Game) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(0) }))
        .count()
}

/// Cast card 0, pay, resolve it, check and replay, then play into the next
/// step.
fn resolve(g: &mut Game, lands: u32) {
    Driver::cast(g, CardId(0));
    Driver::pay(g, &(1..=lands).collect::<Vec<_>>());
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
    let step = g.state.rules.turn.step;
    while g.state.rules.turn.step == step {
        Driver::pass(g);
        Driver::request(g, u64::MAX);
    }
    checked(g);
}

/// Collective Unconscious: "Draw a card for each creature you control."
/// Only seat 0's creatures count; each is one draw.
#[test]
fn collective_unconscious_draws_one_card_for_each_creature_you_control() {
    for mine in [0, 3] {
        let (mut g, lands) = game(COLLECTIVE_UNCONSCIOUS, [mine, 2], 0);
        let before = hand(&g);
        resolve(&mut g, lands);
        assert_eq!(hand(&g), before - 1 + mine, "{mine} creatures");
    }
}

/// Grim Flowering: "Draw a card for each creature card in your graveyard."
/// A noncreature card and another player's graveyard do not count.
#[test]
fn grim_flowering_counts_only_creature_cards_in_your_graveyard() {
    let (mut g, lands) = game(GRIM_FLOWERING, [0, 0], 2);
    let before = hand(&g);
    resolve(&mut g, lands);
    assert_eq!(hand(&g), before - 1 + 2);
}

/// Folk Medicine: "You gain 1 life for each creature you control." One
/// life gain of the counted amount (CR 119.3).
#[test]
fn folk_medicine_gains_one_life_for_each_creature_you_control() {
    let (mut g, lands) = game(FOLK_MEDICINE, [2, 3], 0);
    let mut expected = g.state.rules.players[0].life.clone();
    expected.add(&Quantity::from(2));
    resolve(&mut g, lands);
    assert_eq!(g.state.rules.players[0].life, expected);
}

/// The counted draw opens its own `Executes` cell; a count of the hand,
/// which drawing itself changes, has no audited cell.
#[test]
fn a_counted_draw_counts_only_zones_drawing_cannot_change() {
    let d = lowered(KEEP_WATCH);
    assert!(
        mtg_contracts::kir::definition_cells(&d)
            .unwrap()
            .contains(&Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Draw))
    );
    let mut forged = lowered(COLLECTIVE_UNCONSCIOUS);
    let program = forged.spell.as_mut().unwrap();
    let json = serde_json::to_string(&program.instructions).unwrap();
    let hand = json.replace(
        r#""zone":{"zone":"battlefield"}"#,
        r#""zone":{"zone":"hand","of":{"player":"is","who":{"ref":"controller","role":"spell_controller"}}}"#,
    );
    assert_ne!(hand, json);
    program.instructions = serde_json::from_str(&hand).unwrap();
    assert!(mtg_contracts::kir::definition_cells(&forged).is_err());
}
