//! Fixed-count modal instants and sorceries from exact frozen R5 source on
//! private boards. Modes are announced first, only selections whose targets
//! can be chosen are offered, and the chosen modes run in printed order.
//! Every choice is checked for state validity and an exact snapshot round
//! trip; every finished resolution replays from its journal.
//! Pinned CR 601.2b-c, 608.2c, 700.2a, 700.2c-d:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Tiers, definition_tiers};

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const AUSTERE_COMMAND: &str = record!("09cc8709-fe10-472a-b05c-e89f3523018d");
const INVOKE_THE_FIREMIND: &str = record!("2037659f-2efe-4321-afaf-961bbec35e9e");
const GRIXIS_CHARM: &str = record!("387ae7be-1cb5-4ae8-bb63-edf4941945a9");
const THORINS_LAST_STAND: &str = record!("3c4da169-1e50-4790-abdc-214e7b8a2ad5");
const FAREWELL: &str = record!("4eb813fd-2d5a-4b02-8193-662681ef4e7d");
const CLEANSING_NOVA: &str = record!("aff34f28-f707-4458-8af3-1bd5b13a6b10");

/// A two-seat main-phase board whose catalog adds the frozen modal `cards`.
/// Each must lower with a modal declaration and earn `Executes` only.
fn board(cards: &[&str]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in cards {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen modal witness failed: {result:?}");
        };
        assert!(definition.modal_spell.is_some(), "{}", definition.name);
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-modal-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
}

/// State validation and an exact snapshot round trip.
fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// The next decision, which must be `seat`'s `reason`, after a check.
fn expect(g: &mut Game, seat: u8, reason: Reason<Rules>) -> Decision<Rules> {
    let d = Driver::request(g, u64::MAX);
    assert_eq!((d.player, &d.reason), (PlayerId(seat), &reason), "{d:?}");
    checked(g);
    d
}

/// The offered mode masks of the pending mode decision, in offer order.
fn offered(d: &Decision<Rules>) -> Vec<u8> {
    d.candidates
        .iter()
        .map(|c| match c {
            Candidate::Modes { mask, .. } => *mask,
            other => panic!("non-mode candidate {other:?}"),
        })
        .collect()
}

/// Cast card 0 for seat 0 and answer the mode decision with `mask`, after
/// checking that exactly `masks` were offered.
fn cast_modes(g: &mut Game, masks: &[u8], mask: u8) {
    Driver::cast(g, CardId(0));
    let d = expect(g, 0, Reason::ChooseModes);
    assert!(matches!(d.shape, Shape::Pick { .. }), "{d:?}");
    assert_eq!(offered(&d), masks);
    let chosen = d
        .candidates
        .iter()
        .find(|c| matches!(c, Candidate::Modes { mask: m, .. } if *m == mask))
        .unwrap();
    g.respond(d.player, d.id, d.pick_one(chosen).unwrap())
        .unwrap();
}

/// Resolve to an empty stack, then prove replay from the journal.
fn settle(g: &mut Game) {
    for _ in 0..24 {
        let d = Driver::request(g, 1);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        if g.state.rules.stack.is_empty() {
            checked(g);
            assert_recoverable((&*g).into());
            return;
        }
        let pass = d
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        g.respond(d.player, d.id, pass).unwrap();
    }
    panic!("no empty-stack priority");
}

fn zone(g: &Game, card: u32) -> Option<Zone> {
    g.zone(Driver::object(g, card))
}

fn graveyard(seat: u8) -> Option<Zone> {
    Some(Zone::Graveyard {
        owner: PlayerId(seat),
    })
}

fn hand_size(g: &Game, seat: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone
                == (Zone::Hand {
                    owner: PlayerId(seat),
                })
        })
        .count()
}

/// Cleansing Nova: "Choose one — • Destroy all creatures. • Destroy all
/// artifacts and enchantments." Only the chosen mode is performed.
#[test]
fn cleansing_nova_performs_only_the_chosen_mode() {
    for (mask, bears, ring) in [
        (0b01, graveyard(1), Some(Zone::Battlefield)),
        (0b10, Some(Zone::Battlefield), graveyard(1)),
    ] {
        let mut b = board(&[CLEANSING_NOVA]).permanent_or_hand("Cleansing Nova", 0, 0, true);
        for _ in 0..5 {
            b = b.permanent_or_hand("Plains", 0, 0, false);
        }
        let mut g = Game::new(
            b.permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Sol Ring", 1, 1, false)
                .libraries("Island", 2)
                .prepare(),
        );
        cast_modes(&mut g, &[0b01, 0b10], mask);
        Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
        settle(&mut g);
        assert_eq!((zone(&g, 6), zone(&g, 7)), (bears, ring));
        assert_eq!(zone(&g, 0), graveyard(0));
    }
}

/// Austere Command: "Choose two —" offers each pair of distinct modes once
/// (CR 700.2d) and performs both chosen modes.
#[test]
fn austere_command_offers_each_distinct_pair_and_performs_both() {
    let mut b = board(&[AUSTERE_COMMAND]).permanent_or_hand("Austere Command", 0, 0, true);
    for _ in 0..6 {
        b = b.permanent_or_hand("Plains", 0, 0, false);
    }
    let mut g = Game::new(
        b.permanent_or_hand("Sol Ring", 1, 1, false)
            .permanent_or_hand("Glorious Anthem", 1, 1, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
            .libraries("Island", 2)
            .prepare(),
    );
    // Artifacts, and creatures with mana value 3 or less.
    cast_modes(
        &mut g,
        &[0b0011, 0b0101, 0b0110, 0b1001, 0b1010, 0b1100],
        0b0101,
    );
    Driver::pay(&mut g, &[1, 2, 3, 4, 5, 6]);
    settle(&mut g);
    assert_eq!(
        [7, 8, 9, 10].map(|card| zone(&g, card)),
        [
            graveyard(1),
            Some(Zone::Battlefield),
            graveyard(1),
            Some(Zone::Battlefield)
        ]
    );
}

/// Grixis Charm: a mode whose target cannot be chosen cannot be chosen
/// (CR 700.2a); the chosen mode's target is announced after the modes
/// (CR 601.2c, 700.2c) and the opponent sees the announced modes.
#[test]
fn grixis_charm_offers_only_targetable_modes_and_targets_after_them() {
    let lands = |b: Board| {
        b.permanent_or_hand("Grixis Charm", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false)
    };
    // No creature: only the permanent bounce and the anthem remain.
    let mut g = Game::new(
        lands(board(&[GRIXIS_CHARM]))
            .libraries("Island", 2)
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    let d = expect(&mut g, 0, Reason::ChooseModes);
    assert_eq!(offered(&d), [0b001, 0b100]);

    let mut g = Game::new(
        lands(board(&[GRIXIS_CHARM]))
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .libraries("Island", 2)
            .prepare(),
    );
    cast_modes(&mut g, &[0b001, 0b010, 0b100], 0b010);
    let bears = Driver::object(&g, 4);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
    checked(&g);
    assert_eq!(g.state.rules.stack.last().unwrap().mode_mask, Some(0b010));
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(1), Budget::unlimited())
    else {
        panic!("missing observation");
    };
    assert_eq!(o.view.stack.last().unwrap().mode_mask, Some(0b010));
    assert_eq!(d.candidates, [Candidate::Object { object: bears }], "{d:?}");
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Object { object: bears }).unwrap(),
    )
    .unwrap();
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    assert_eq!(zone(&g, 4), graveyard(1));
}

/// Thorin's Last Stand: one mode with two sentences performs both in order
/// (CR 608.2c); without an artifact or enchantment it cannot be chosen.
#[test]
fn thorins_last_stand_performs_a_whole_mode() {
    let base = |artifact: bool| {
        let mut b =
            board(&[THORINS_LAST_STAND]).permanent_or_hand("Thorin's Last Stand", 0, 0, true);
        for _ in 0..4 {
            b = b.permanent_or_hand("Plains", 0, 0, false);
        }
        if artifact {
            b = b.permanent_or_hand("Sol Ring", 1, 1, false);
        }
        Game::new(b.libraries("Island", 2).prepare())
    };
    let mut g = base(false);
    Driver::cast(&mut g, CardId(0));
    assert_eq!(offered(&expect(&mut g, 0, Reason::ChooseModes)), [0b01]);

    let mut g = base(true);
    let life = g.state.rules.players[0].life.clone();
    cast_modes(&mut g, &[0b01, 0b10], 0b10);
    let ring = Driver::object(&g, 5);
    Driver::pick_object(&mut g, ring);
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    settle(&mut g);
    assert_eq!(zone(&g, 5), graveyard(1));
    let mut gained = life;
    gained.add(&Quantity::from(2));
    assert_eq!(g.state.rules.players[0].life, gained);
}

/// Farewell: "Choose one or more —" offers every nonempty set of its four
/// modes, and performs exactly the chosen ones.
#[test]
fn farewell_offers_every_nonempty_set_of_modes() {
    let mut b = board(&[FAREWELL]).permanent_or_hand("Farewell", 0, 0, true);
    for _ in 0..6 {
        b = b.permanent_or_hand("Plains", 0, 0, false);
    }
    let mut g = Game::new(
        b.permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Sol Ring", 1, 1, false)
            .card(
                "Lightning Bolt",
                1,
                1,
                Zone::Graveyard { owner: PlayerId(1) },
            )
            .libraries("Island", 2)
            .prepare(),
    );
    // Creatures and graveyards.
    cast_modes(&mut g, &(1..16).collect::<Vec<u8>>(), 0b1010);
    Driver::pay(&mut g, &[1, 2, 3, 4, 5, 6]);
    settle(&mut g);
    assert_eq!(
        [7, 8, 9].map(|card| zone(&g, card)),
        [
            Some(Zone::Exile),
            Some(Zone::Battlefield),
            Some(Zone::Exile)
        ]
    );
}

/// Invoke the Firemind: modes are announced before X (CR 601.2b), and the
/// chosen mode reads the announced X.
#[test]
fn invoke_the_firemind_announces_modes_before_x() {
    for mask in [0b01, 0b10] {
        let mut g = Game::new(
            board(&[INVOKE_THE_FIREMIND])
                .permanent_or_hand("Invoke the Firemind", 0, 0, true)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false)
                .libraries("Island", 4)
                .prepare(),
        );
        let (hand, life) = (hand_size(&g, 0), g.state.rules.players[1].life.clone());
        cast_modes(&mut g, &[0b01, 0b10], mask);
        let d = expect(&mut g, 0, Reason::AnnounceX);
        g.respond(d.player, d.id, Answer::Number { value: 2.into() })
            .unwrap();
        if mask == 0b10 {
            let d = Driver::request(&mut g, u64::MAX);
            assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
            let player = d
                .pick_one(&Candidate::Player {
                    player: PlayerId(1),
                })
                .unwrap();
            g.respond(d.player, d.id, player).unwrap();
        }
        Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
        settle(&mut g);
        let mut damaged = life.clone();
        damaged.add(&Quantity::from(-2));
        assert_eq!(
            (hand_size(&g, 0), g.state.rules.players[1].life.clone()),
            if mask == 0b01 {
                (hand + 1, life)
            } else {
                (hand - 1, damaged)
            }
        );
    }
}

const JUND_CHARM: &str = record!("7bd2ac13-d598-4cf1-a483-2a6878aed93b");
const REVERENT_HOWL: &str = record!("86214b4c-a3ad-4372-b5fe-8f742a7a3d4c");
const GO_FORTH: &str = record!("a4f3df30-383b-47ad-8576-0ee69349faf5");
const MOUSER_ATTACK: &str = record!("bae5dc68-1fd1-428d-bee7-c238e07b21ea");
const SIMIC_CHARM: &str = record!("c767aa2e-0a95-401f-8e36-d90a87648eab");

/// The basic land paying each symbol of `cost`; generic and X are paid by
/// Mountains, with X announced as 1.
fn lands_for(cost: &str) -> Vec<&'static str> {
    cost.split(['{', '}'])
        .filter(|s| !s.is_empty())
        .flat_map(|symbol| {
            let land = match symbol {
                "W" => "Plains",
                "U" => "Island",
                "B" => "Swamp",
                "R" => "Mountain",
                "G" => "Forest",
                "X" => "Mountain",
                n => return vec!["Mountain"; n.parse().unwrap()],
            };
            vec![land]
        })
        .collect()
}

/// Every selection the spell offers on a shared board, each cast and
/// resolved in a fresh game: targets and resolution choices take the first
/// candidate. Each choice is checked and each resolution replays.
fn resolve_every_selection(raw: &str) -> Vec<u8> {
    let source: serde_json::Value = serde_json::from_str(raw).unwrap();
    let (name, cost) = (
        source["source"]["name"].as_str().unwrap(),
        source["source"]["mana_cost"].as_str().unwrap(),
    );
    let lands = lands_for(cost);
    let game = || {
        let mut b = board(&[raw]).permanent_or_hand(name, 0, 0, true);
        for land in &lands {
            b = b.permanent_or_hand(land, 0, 0, false);
        }
        Game::new(
            b.permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
                .permanent_or_hand("Sol Ring", 1, 1, false)
                .permanent_or_hand("Glorious Anthem", 1, 1, false)
                .card(
                    "Lightning Bolt",
                    1,
                    1,
                    Zone::Graveyard { owner: PlayerId(1) },
                )
                .libraries("Forest", 4)
                .seed([7; 32])
                .prepare(),
        )
    };
    let mut probe = game();
    Driver::cast(&mut probe, CardId(0));
    let masks = offered(&expect(&mut probe, 0, Reason::ChooseModes));
    let paid: Vec<u32> = (1..=lands.len() as u32).collect();
    for &mask in &masks {
        let mut g = game();
        cast_modes(&mut g, &masks, mask);
        for _ in 0..64 {
            let d = Driver::request(&mut g, u64::MAX);
            checked(&g);
            match &d.reason {
                Reason::Priority if g.state.rules.stack.is_empty() => break,
                Reason::Priority => g
                    .respond(
                        d.player,
                        d.id,
                        d.pick_one(&Candidate::Action {
                            action: Action::Pass,
                        })
                        .unwrap(),
                    )
                    .unwrap(),
                Reason::Mana { .. } => Driver::pay(&mut g, &paid),
                Reason::AnnounceX => g
                    .respond(d.player, d.id, Answer::Number { value: 1.into() })
                    .unwrap(),
                _ => g
                    .respond(d.player, d.id, d.pick_one(&d.candidates[0]).unwrap())
                    .unwrap(),
            }
        }
        assert!(
            g.state.rules.stack.is_empty(),
            "{name} {mask:#b} did not resolve"
        );
        assert_eq!(zone(&g, 0), graveyard(0), "{name} {mask:#b}");
        assert_recoverable((&g).into());
    }
    masks
}

/// Each offered mode of further frozen charms and modal spells resolves on
/// a shared board: counters, a player's graveyard, a library search, token
/// creation, a targeted draw and loss and targeted pumps.
#[test]
fn every_offered_mode_resolves() {
    for (raw, masks) in [
        (JUND_CHARM, &[0b001, 0b010, 0b100][..]),
        (REVERENT_HOWL, &[0b01, 0b10]),
        (GO_FORTH, &[0b01, 0b10]),
        (MOUSER_ATTACK, &[0b01, 0b10]),
        (SIMIC_CHARM, &[0b001, 0b010, 0b100]),
        (GRIXIS_CHARM, &[0b001, 0b010, 0b100]),
        (CLEANSING_NOVA, &[0b01, 0b10]),
    ] {
        assert_eq!(resolve_every_selection(raw), masks);
    }
}
