//! Track 3A wave W10: variable, hybrid and Phyrexian mana costs, cycling,
//! kicker, flashback, a spell's additional sacrifice and activation cost
//! terms, from exact frozen R5 source on private boards. Every choice is
//! checked for state validity and an exact snapshot round trip; every
//! finished resolution replays from its journal.
//! Pinned CR 107.3a, 107.4e-f, 109.1, 118.3, 119.4, 122.1, 601.2b/f-h,
//! 602.2b, 608.2b, 701.21a, 702.29a, 702.33a/d, 702.34a, 733.1:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::HybridPayment;
use mtg_contracts::kir::{Cell, Tiers, definition_cells, definition_tiers};
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

mod activation_conditions;
mod chosen_objects;

const BLAZE: &str = record!("0596920f-9946-42f4-a03b-24aab67f9f1b");
const BOROS_RECRUIT: &str = record!("00abb43b-550f-42d2-9049-61ac56ff5d8d");
const VAULT_SKIRGE: &str = record!("64cf5d59-7bcd-4b0b-a160-c8468d4c0f60");
const LAY_WASTE: &str = record!("04cc9d99-69a9-499b-91bc-f2f30d217bdd");
const TOLARIAN_GEYSER: &str = record!("d2240ce8-0bd6-4c4f-a73e-fb0a2af5bfac");
const THINK_TWICE: &str = record!("fa85c5a2-8e83-4624-a35a-a0bbf17ecbb4");
const GEISTFLAME: &str = record!("2f01c3d9-e0fc-4cdf-b4db-daeb8bb24bc3");
const BONE_SPLINTERS: &str = record!("0c936582-d4c0-4da8-bc7e-49da5a433939");
const GREED: &str = record!("1ff62220-be95-4901-b8d8-812b9a1a1b0a");
const SPIKE_FEEDER: &str = record!("5b147eab-39dd-4c2a-8f59-d778037788aa");
const QUAGMIRE_DRUID: &str = record!("560e1e81-6675-4d08-8e82-cdf1abd4b3d0");
const VORACIOUS_NULL: &str = record!("0f92daf5-520d-472a-a835-e6680a92fdab");
const KRARK_CLAN_ENGINEERS: &str = record!("0964288c-0397-428a-9a84-784aaea4e040");
const GUTTERSNIPE: &str = record!("c6bdaf76-6a03-4695-9c4b-f040e73435af");
const CORROSIVE_GALE: &str = record!("6143073f-4110-4a37-bc0c-89fbda2fc839");
const UNSPEAKABLE_SYMBOL: &str = record!("a5690d5f-633c-4a1e-afba-5fd79dcbf20e");
const BEAST_ATTACK: &str = record!("3f5acd90-199c-4ed3-9c8a-eeac4670d876");

/// A two-seat main-phase board whose catalog adds `cards`, each compiled
/// from frozen source and occupying the named cells. A card with a W10 cell
/// earns `Executes` only.
fn board(cards: &[(&str, &[Cell])]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(raw, cells) in cards {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen cost witness failed: {result:?}");
        };
        let occupied = definition_cells(&definition).unwrap();
        for cell in cells {
            assert!(
                occupied.contains(cell),
                "{} lacks {cell:?}",
                definition.name
            );
        }
        if !cells.is_empty() {
            assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-cost-witness")
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

fn observation(g: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("missing observation for seat {seat}");
    };
    o
}

/// The next decision, which must be `seat`'s `reason`, after a check.
fn expect(g: &mut Game, seat: u8, reason: Reason<Rules>) -> Decision<Rules> {
    let d = Driver::request(g, u64::MAX);
    assert_eq!((d.player, &d.reason), (PlayerId(seat), &reason), "{d:?}");
    checked(g);
    d
}

/// Answer the pending one-slot pick with exactly `candidates`.
fn pick(g: &mut Game, seat: u8, reason: Reason<Rules>, candidates: &[Candidate<Rules>]) {
    let d = expect(g, seat, reason);
    let picks = candidates.iter().map(|c| d.index(c).unwrap()).collect();
    g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
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

/// The actions `seat` is offered at its pending priority decision.
fn actions(g: &mut Game) -> Vec<Action<Rules>> {
    let d = Driver::request(g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    d.candidates
        .into_iter()
        .filter_map(|c| match c {
            Candidate::Action { action } => Some(action),
            _ => None,
        })
        .collect()
}

fn life(g: &Game, seat: usize) -> Quantity {
    g.state.rules.players[seat].life.clone()
}

fn zone(g: &Game, card: u32) -> Option<Zone> {
    g.zone(Driver::object(g, card))
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

fn plus(base: &Quantity, n: i64) -> Quantity {
    let mut q = base.clone();
    q.add(&Quantity::from(n));
    q
}

/// Blaze: "Blaze deals X damage to any target." X is announced before
/// targets, adds that much generic mana, counts on the stack and is the
/// damage dealt (CR 107.3a, 601.2b, 601.2f).
#[test]
fn blaze_announces_x_pays_it_and_deals_that_much_damage() {
    let prepared = board(&[(BLAZE, &[Cell::XManaCost, Cell::DamageTargetX])])
        .permanent_or_hand("Blaze", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
        .libraries("Island", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let before = life(&g, 1);
    Driver::cast(&mut g, CardId(0));
    let d = expect(&mut g, 0, Reason::AnnounceX);
    assert!(matches!(d.shape, Shape::Number { .. }), "{d:?}");
    g.respond(d.player, d.id, Answer::Number { value: 2.into() })
        .unwrap();
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
    checked(&g);
    let player = d
        .candidates
        .iter()
        .position(|c| matches!(c, Candidate::Player { player } if *player == PlayerId(1)))
        .unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![u32::try_from(player).unwrap()]],
        },
    )
    .unwrap();
    // CR 202.3e: X counts toward the mana value of the spell on the stack;
    // the total is locked as the mana window opens (CR 601.2f).
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{d:?}");
    checked(&g);
    let entry = g.state.rules.stack.last().unwrap();
    assert_eq!(entry.x, Some(2.into()));
    assert_eq!(
        entry.total_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::R],
        }
    );
    // CR 702.34a: only flashback casts a spell from a graveyard.
    let mut forged = g.fork();
    forged.state.rules.stack[0].cast_from = Zone::Graveyard { owner: PlayerId(0) };
    assert!(
        forged
            .validate_state()
            .unwrap_err()
            .contains("casting origin")
    );
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    assert_eq!(life(&g, 1), plus(&before, -2));
    assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
}

/// Boros Recruit ({R/W}) and Vault Skirge ({1}{B/P}): each hybrid symbol
/// is announced as one of its alternatives; a Phyrexian symbol paid with
/// life costs 2 life, which a player with less than 2 life cannot pay
/// (CR 107.4e-f, 119.4, 601.2f-h). An unpayable announcement reverses
/// the cast (CR 733.1).
#[test]
fn hybrid_and_phyrexian_symbols_are_announced_and_paid() {
    let prepared = board(&[
        (BOROS_RECRUIT, &[Cell::HybridManaCost]),
        (VAULT_SKIRGE, &[Cell::HybridManaCost]),
    ])
    .permanent_or_hand("Boros Recruit", 0, 0, true)
    .permanent_or_hand("Plains", 0, 0, false)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    let d = expect(&mut g, 0, Reason::HybridCost);
    let offered: Vec<_> = d.candidates.clone();
    assert_eq!(
        offered,
        vec![
            Candidate::HybridPayment {
                payment: HybridPayment::Mana(Color::R)
            },
            Candidate::HybridPayment {
                payment: HybridPayment::Mana(Color::W)
            },
        ]
    );
    pick(
        &mut g,
        0,
        Reason::HybridCost,
        &[Candidate::HybridPayment {
            payment: HybridPayment::Mana(Color::W),
        }],
    );
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    assert_eq!(zone(&g, 0), Some(Zone::Battlefield));

    for (start, payment, paid) in [
        (20, HybridPayment::PayTwoLife, true),
        (2, HybridPayment::PayTwoLife, true),
        (1, HybridPayment::PayTwoLife, false),
        (1, HybridPayment::Mana(Color::B), true),
    ] {
        let prepared = board(&[(VAULT_SKIRGE, &[Cell::HybridManaCost])])
            .permanent_or_hand("Vault Skirge", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .life(PlayerId(0), start.into())
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        pick(
            &mut g,
            0,
            Reason::HybridCost,
            &[Candidate::HybridPayment { payment }],
        );
        let lands: &[u32] = if payment == HybridPayment::PayTwoLife {
            &[1]
        } else {
            &[1, 2]
        };
        Driver::pay(&mut g, lands);
        let d = Driver::request(&mut g, u64::MAX);
        checked(&g);
        if paid {
            let life_paid = if payment == HybridPayment::PayTwoLife {
                2
            } else {
                0
            };
            assert_eq!(life(&g, 0), plus(&start.into(), -life_paid));
            if start - life_paid > 0 {
                assert_eq!(d.reason, Reason::Priority);
                settle(&mut g);
                assert_eq!(zone(&g, 0), Some(Zone::Battlefield));
            }
        } else {
            // CR 119.4: 1 life cannot pay 2 life; the cast is reversed.
            assert_eq!(d.reason, Reason::Priority);
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(life(&g, 0), Quantity::from(1));
            assert_eq!(zone(&g, 0), Some(Zone::Hand { owner: PlayerId(0) }));
            assert_recoverable((&g).into());
        }
    }
}

/// Lay Waste: "Cycling {2}": pay {2} and discard this card from hand to
/// draw a card, at instant speed (CR 702.29a). The discard is public; the
/// drawn card stays hidden from the opponent.
#[test]
fn lay_waste_cycles_from_hand_and_draws_privately() {
    let prepared = board(&[(LAY_WASTE, &[Cell::Cycling])])
        .permanent_or_hand("Lay Waste", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .library_card("Storm Crow", 0)
        .library_card("Island", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let lay_waste = Driver::object(&g, 0);
    let cycle = Action::ActivateAbility {
        object: lay_waste,
        ability: 0,
    };
    assert!(actions(&mut g).contains(&cycle));
    Driver::action(&mut g, cycle);
    checked(&g);
    Driver::pay(&mut g, &[1, 2]);
    assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
    settle(&mut g);
    let crow = Driver::object(&g, 3);
    assert_eq!(g.zone(crow), Some(Zone::Hand { owner: PlayerId(0) }));
    assert_eq!(hand_size(&g, 0), 1);
    let storm_crow = oracle_id(FixtureCatalog::Witness.get(), "Storm Crow");
    assert!(
        observation(&g, 0)
            .view
            .objects
            .iter()
            .any(|o| o.oracle_id == storm_crow)
    );
    assert!(
        observation(&g, 1)
            .view
            .objects
            .iter()
            .all(|o| o.oracle_id != storm_crow)
    );
}

/// Tolarian Geyser: "Kicker {W}. Return target creature to its owner's
/// hand. Draw a card. If this spell was kicked, you gain 3 life." The
/// kicker is declared while casting, adds its cost, and the condition reads
/// that declaration (CR 601.2b, 601.2f, 702.33a, 702.33d).
#[test]
fn tolarian_geyser_reads_whether_its_kicker_was_paid() {
    for kicked in [false, true] {
        let prepared = board(&[(TOLARIAN_GEYSER, &[Cell::Kicker, Cell::Control])])
            .permanent_or_hand("Tolarian Geyser", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .libraries("Island", 2)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let before = life(&g, 0);
        Driver::cast(&mut g, CardId(0));
        let d = expect(&mut g, 0, Reason::AdditionalCosts);
        assert_eq!(d.candidates.len(), 1);
        let picks = if kicked { vec![0] } else { vec![] };
        g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
            .unwrap();
        let bears = Driver::object(&g, 5);
        Driver::pick_object(&mut g, bears);
        Driver::request(&mut g, u64::MAX);
        checked(&g);
        let cost = g.state.rules.stack.last().unwrap().total_cost.clone();
        assert_eq!(
            cost.colored.len(),
            if kicked { 2 } else { 1 },
            "kicker adds {{W}}"
        );
        let lands: &[u32] = if kicked { &[1, 2, 3, 4] } else { &[1, 2, 3] };
        Driver::pay(&mut g, lands);
        settle(&mut g);
        assert_eq!(zone(&g, 5), Some(Zone::Hand { owner: PlayerId(1) }));
        assert_eq!(hand_size(&g, 0), 1);
        assert_eq!(life(&g, 0), plus(&before, if kicked { 3 } else { 0 }));
    }
}

/// Think Twice: "Draw a card. Flashback {2}{U}." Cast from its owner's
/// graveyard for the flashback cost at normal timing, it is exiled as it
/// resolves; a cast-trigger observer sees the graveyard cast (CR 601.2i,
/// 702.34a). An abandoned graveyard cast returns the card to the graveyard
/// (CR 733.1), and nobody else, and no card without flashback, is offered
/// a graveyard cast.
#[test]
fn think_twice_flashes_back_from_the_graveyard_into_exile() {
    let prepared = board(&[(THINK_TWICE, &[Cell::Flashback]), (GUTTERSNIPE, &[])])
        .card("Think Twice", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
        .card(
            "Lightning Bolt",
            0,
            0,
            Zone::Graveyard { owner: PlayerId(0) },
        )
        .permanent_or_hand("Guttersnipe", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .libraries("Island", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let think_twice = Driver::object(&g, 0);
    let bolt = Driver::object(&g, 1);
    let offered = actions(&mut g);
    assert!(offered.contains(&Action::Cast { card: think_twice }));
    assert!(!offered.contains(&Action::Cast { card: bolt }));

    // An unfinished payment reverses the cast to the graveyard.
    let mut abandoned = g.fork();
    Driver::cast(&mut abandoned, CardId(0));
    Driver::request(&mut abandoned, u64::MAX);
    checked(&abandoned);
    assert_eq!(
        abandoned.state.rules.stack.last().unwrap().total_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::U],
        }
    );
    Driver::pay(&mut abandoned, &[]);
    checked(&abandoned);
    assert!(abandoned.state.rules.stack.is_empty());
    assert_eq!(
        abandoned.zone(think_twice),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );

    let opponent = life(&g, 1);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[3, 4, 5]);
    checked(&g);
    settle(&mut g);
    assert_eq!(zone(&g, 0), Some(Zone::Exile));
    assert_eq!(hand_size(&g, 0), 1);
    // Guttersnipe saw the graveyard cast.
    assert_eq!(life(&g, 1), plus(&opponent, -2));
    // The exiled card is not offered again.
    let exiled = Driver::object(&g, 0);
    assert!(!actions(&mut g).contains(&Action::Cast { card: exiled }));
}

/// Geistflame: "Geistflame deals 1 damage to any target. Flashback
/// {3}{R}." A flashback spell whose only target became illegal does not
/// resolve and is still exiled instead of going to the graveyard
/// (CR 608.2b, 702.34a).
#[test]
fn geistflame_flashback_that_fails_to_resolve_is_exiled() {
    let prepared = board(&[(GEISTFLAME, &[Cell::Flashback])])
        .card("Geistflame", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
        .permanent_or_hand("Lightning Bolt", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .libraries("Island", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 7);
    Driver::cast(&mut g, CardId(0));
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[2, 3, 4, 5]);
    checked(&g);
    // Holding priority, the caster kills the target first.
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[6]);
    settle(&mut g);
    assert_eq!(zone(&g, 7), Some(Zone::Graveyard { owner: PlayerId(1) }));
    assert_eq!(zone(&g, 0), Some(Zone::Exile));
    assert_eq!(zone(&g, 1), Some(Zone::Graveyard { owner: PlayerId(0) }));
}

/// Bone Splinters: "As an additional cost to cast this spell, sacrifice a
/// creature. Destroy target creature." The sacrifice is chosen after mana
/// and paid with it; with no creature to sacrifice the spell is not offered
/// (CR 601.2b, 601.2h, 701.21a).
#[test]
fn bone_splinters_sacrifices_a_creature_as_it_is_cast() {
    let prepared = board(&[(BONE_SPLINTERS, &[Cell::SpellAdditionalSacrifice])])
        .permanent_or_hand("Bone Splinters", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let splinters = Driver::object(&g, 0);
    assert!(!actions(&mut g).contains(&Action::Cast { card: splinters }));

    let prepared = board(&[(BONE_SPLINTERS, &[Cell::SpellAdditionalSacrifice])])
        .permanent_or_hand("Bone Splinters", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 2);
    let dreadmaw = Driver::object(&g, 3);
    Driver::cast(&mut g, CardId(0));
    Driver::pick_object(&mut g, dreadmaw);
    Driver::pay(&mut g, &[1]);
    let d = expect(&mut g, 0, Reason::SacrificeCost);
    // Only a creature the caster controls can be sacrificed.
    assert_eq!(d.candidates, vec![Candidate::Object { object: bears }]);
    pick(
        &mut g,
        0,
        Reason::SacrificeCost,
        &[Candidate::Object { object: bears }],
    );
    settle(&mut g);
    assert_eq!(zone(&g, 2), Some(Zone::Graveyard { owner: PlayerId(0) }));
    assert_eq!(zone(&g, 3), Some(Zone::Graveyard { owner: PlayerId(1) }));
}

/// Greed: "{B}, Pay 2 life: Draw a card." Life is paid with the mana; a
/// player with less than 2 life is not offered the ability (CR 119.4,
/// 602.2b).
#[test]
fn greed_pays_life_with_mana_and_is_not_offered_without_it() {
    for start in [20, 1] {
        let prepared = board(&[(GREED, &[Cell::ActivationPayLife])])
            .permanent_or_hand("Greed", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .life(PlayerId(0), start.into())
            .libraries("Island", 2)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let greed = Action::ActivateAbility {
            object: Driver::object(&g, 0),
            ability: 0,
        };
        if start < 2 {
            assert!(!actions(&mut g).contains(&greed));
            continue;
        }
        Driver::action(&mut g, greed);
        Driver::pay(&mut g, &[1]);
        assert_eq!(life(&g, 0), plus(&start.into(), -2));
        settle(&mut g);
        assert_eq!(hand_size(&g, 0), 1);
    }
}

/// Spike Feeder: "This creature enters with two +1/+1 counters on it.
/// Remove a +1/+1 counter from this creature: You gain 2 life." Each
/// activation removes one counter as its cost (CR 122.1, 602.2b).
#[test]
fn spike_feeder_removes_its_own_counters_as_a_cost() {
    let prepared = board(&[(SPIKE_FEEDER, &[Cell::ActivationRemoveCounters])])
        .permanent_or_hand("Spike Feeder", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    let feeder = Driver::object(&g, 0);
    assert_eq!(
        g.current(feeder).unwrap().plus_one_counters,
        Quantity::from(2)
    );
    let gain = Action::ActivateAbility {
        object: feeder,
        ability: 1,
    };
    let before = life(&g, 0);
    Driver::action(&mut g, gain.clone());
    Driver::request(&mut g, u64::MAX);
    checked(&g);
    assert_eq!(
        g.current(feeder).unwrap().plus_one_counters,
        Quantity::from(1)
    );
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&before, 2));
    // The last counter is paid too: the 0/0 creature dies to the state-based
    // action (CR 704.5f) and its paid ability still resolves (CR 113.7a).
    Driver::action(&mut g, gain.clone());
    settle(&mut g);
    assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
    assert_eq!(life(&g, 0), plus(&before, 4));
}

/// Quagmire Druid: "{G}, {T}, Sacrifice a creature: Destroy target
/// enchantment." The druid taps and the chosen creature is sacrificed after
/// mana, together (CR 602.2b, 701.21a).
#[test]
fn quagmire_druid_taps_and_sacrifices_a_chosen_creature() {
    let prepared = board(&[(QUAGMIRE_DRUID, &[Cell::SacrificeCostTap])])
        .permanent_or_hand("Quagmire Druid", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Glorious Anthem", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let druid = Driver::object(&g, 0);
    let bears = Driver::object(&g, 2);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: druid,
            ability: 0,
        },
    );
    let anthem = Driver::object(&g, 3);
    Driver::pick_object(&mut g, anthem);
    Driver::pay(&mut g, &[1]);
    let d = expect(&mut g, 0, Reason::SacrificeCost);
    assert!(d.candidates.contains(&Candidate::Object { object: druid }));
    pick(
        &mut g,
        0,
        Reason::SacrificeCost,
        &[Candidate::Object { object: bears }],
    );
    settle(&mut g);
    assert!(g.current(druid).unwrap().tapped);
    assert_eq!(zone(&g, 2), Some(Zone::Graveyard { owner: PlayerId(0) }));
    assert_eq!(zone(&g, 3), Some(Zone::Graveyard { owner: PlayerId(1) }));
}

/// Voracious Null: "{1}{B}, Sacrifice another creature: Put two +1/+1
/// counters on this creature. Activate only as a sorcery." "Another"
/// excludes the source: alone it is not offered (CR 109.1, 701.21a).
#[test]
fn voracious_null_sacrifices_only_another_creature() {
    for with_bears in [false, true] {
        let mut b = board(&[(VORACIOUS_NULL, &[Cell::SacrificeCostAnother])])
            .permanent_or_hand("Voracious Null", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false);
        if with_bears {
            b = b.permanent_or_hand("Grizzly Bears", 0, 0, false);
        }
        let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
        let null = Driver::object(&g, 0);
        let grow = Action::ActivateAbility {
            object: null,
            ability: 0,
        };
        if !with_bears {
            assert!(!actions(&mut g).contains(&grow));
            continue;
        }
        let bears = Driver::object(&g, 3);
        Driver::action(&mut g, grow);
        Driver::pay(&mut g, &[1, 2]);
        let d = expect(&mut g, 0, Reason::SacrificeCost);
        assert_eq!(d.candidates, vec![Candidate::Object { object: bears }]);
        pick(
            &mut g,
            0,
            Reason::SacrificeCost,
            &[Candidate::Object { object: bears }],
        );
        settle(&mut g);
        assert_eq!(
            g.current(null).unwrap().plus_one_counters,
            Quantity::from(2)
        );
    }
}

/// Krark-Clan Engineers: "{R}, Sacrifice two artifacts: Destroy target
/// artifact." Two distinct artifacts are chosen and sacrificed together
/// (CR 701.21a).
#[test]
fn krark_clan_engineers_sacrifices_two_artifacts() {
    let prepared = board(&[(KRARK_CLAN_ENGINEERS, &[Cell::SacrificeCostChoice])])
        .permanent_or_hand("Krark-Clan Engineers", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Wooden Sphere", 0, 0, false)
        .permanent_or_hand("Pristine Talisman", 0, 0, false)
        .permanent_or_hand("Sol Ring", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let engineers = Driver::object(&g, 0);
    let ring = Driver::object(&g, 4);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: engineers,
            ability: 0,
        },
    );
    Driver::pick_object(&mut g, ring);
    Driver::pay(&mut g, &[1]);
    for card in [2, 3] {
        let artifact = Driver::object(&g, card);
        pick(
            &mut g,
            0,
            Reason::SacrificeCost,
            &[Candidate::Object { object: artifact }],
        );
    }
    settle(&mut g);
    for (card, owner) in [(2, 0), (3, 0), (4, 1)] {
        assert_eq!(
            zone(&g, card),
            Some(Zone::Graveyard {
                owner: PlayerId(owner)
            })
        );
    }
}

/// Corrosive Gale ({X}{G/P}): "Corrosive Gale deals X damage to each
/// creature with flying." The Phyrexian symbol is announced before X
/// (CR 601.2b), and X stays unbound while it is; X then becomes the damage.
#[test]
fn corrosive_gale_announces_its_phyrexian_symbol_before_x() {
    let prepared = board(&[(
        CORROSIVE_GALE,
        &[Cell::XManaCost, Cell::HybridManaCost, Cell::DamageSet],
    )])
    .permanent_or_hand("Corrosive Gale", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Storm Crow", 1, 1, false)
    .permanent_or_hand("Grizzly Bears", 1, 1, false)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let before = life(&g, 0);
    Driver::cast(&mut g, CardId(0));
    pick(
        &mut g,
        0,
        Reason::HybridCost,
        &[Candidate::HybridPayment {
            payment: HybridPayment::PayTwoLife,
        }],
    );
    let d = expect(&mut g, 0, Reason::AnnounceX);
    g.respond(d.player, d.id, Answer::Number { value: 2.into() })
        .unwrap();
    Driver::pay(&mut g, &[1, 2]);
    assert_eq!(life(&g, 0), plus(&before, -2));
    settle(&mut g);
    assert_eq!(zone(&g, 3), Some(Zone::Graveyard { owner: PlayerId(1) }));
    assert_eq!(zone(&g, 4), Some(Zone::Battlefield));
    assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
}

/// Unspeakable Symbol: "Pay 3 life: Put a +1/+1 counter on target
/// creature." A cost with no mana opens no mana window after targets: the
/// life is paid at once and the activator gets priority (CR 601.2g,
/// 602.2b).
#[test]
fn unspeakable_symbol_pays_life_after_targets_without_a_mana_window() {
    let prepared = board(&[(UNSPEAKABLE_SYMBOL, &[Cell::ActivationPayLife])])
        .permanent_or_hand("Unspeakable Symbol", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let before = life(&g, 0);
    let symbol = Driver::object(&g, 0);
    let bears = Driver::object(&g, 2);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: symbol,
            ability: 0,
        },
    );
    Driver::pick_object(&mut g, bears);
    let d = expect(&mut g, 0, Reason::Priority);
    assert!(!matches!(d.reason, Reason::Mana { .. }));
    assert_eq!(life(&g, 0), plus(&before, -3));
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
    assert!(!g.current(Driver::object(&g, 1)).unwrap().tapped);
    settle(&mut g);
    assert_eq!(
        g.current(bears).unwrap().plus_one_counters,
        Quantity::from(1)
    );
}

/// Beast Attack: "Create a 4/4 green Beast creature token. Flashback
/// {2}{G}{G}{G}." With the same mana paid, a hand cast and a flashback cast
/// are publicly distinct while on the stack, since only the flashback spell
/// is exiled as it leaves (CR 702.34a). A forged origin in another player's
/// graveyard, or a graveyard origin without flashback, fails validation.
#[test]
fn beast_attack_flashback_is_public_on_the_stack_and_its_origin_is_validated() {
    let mut observed = vec![];
    for from_graveyard in [false, true] {
        let origin = if from_graveyard {
            Zone::Graveyard { owner: PlayerId(0) }
        } else {
            Zone::Hand { owner: PlayerId(0) }
        };
        let prepared = board(&[(BEAST_ATTACK, &[Cell::Flashback])])
            .card("Beast Attack", 0, 0, origin)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
        expect(&mut g, 0, Reason::Priority);
        let views: Vec<_> = (0..2).map(|seat| observation(&g, seat)).collect();
        for o in &views {
            assert_eq!(o.view.stack.len(), 1);
            assert_eq!(o.view.stack[0].flashback, from_graveyard);
        }
        observed.push(views);

        // Forged origins: another player's graveyard, and a graveyard origin
        // for a card without flashback.
        let mut forged = g.fork();
        forged.state.rules.stack[0].cast_from = Zone::Graveyard { owner: PlayerId(1) };
        assert!(
            forged
                .validate_state()
                .unwrap_err()
                .contains("casting origin")
        );
        let mut forged = g.fork();
        forged.state.rules.stack[0].cast_from = Zone::Hand { owner: PlayerId(1) };
        assert!(
            forged
                .validate_state()
                .unwrap_err()
                .contains("casting origin")
        );
        settle(&mut g);
        let expected = if from_graveyard {
            Zone::Exile
        } else {
            Zone::Graveyard { owner: PlayerId(0) }
        };
        assert_eq!(zone(&g, 0), Some(expected));
    }
    for (seat, (hand, flashback)) in observed[0].iter().zip(&observed[1]).enumerate() {
        assert_ne!(
            serde_json::to_string(hand).unwrap(),
            serde_json::to_string(flashback).unwrap(),
            "seat {seat}"
        );
    }
}

/// Corrosive Gale: the last opponents concede while the caster is still
/// announcing the Phyrexian symbol, or X (CR 104.2a, 800.4a). The game ends
/// with the unpaid spell on the stack; the finished state validates,
/// round-trips and replays from its journal.
#[test]
fn corrosive_gale_survives_a_concession_during_its_announcement() {
    for at_x in [false, true] {
        let prepared = board(&[(
            CORROSIVE_GALE,
            &[Cell::XManaCost, Cell::HybridManaCost, Cell::DamageSet],
        )])
        .permanent_or_hand("Corrosive Gale", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        if at_x {
            pick(
                &mut g,
                0,
                Reason::HybridCost,
                &[Candidate::HybridPayment {
                    payment: HybridPayment::Mana(Color::G),
                }],
            );
            expect(&mut g, 0, Reason::AnnounceX);
        } else {
            expect(&mut g, 0, Reason::HybridCost);
        }
        finish_by_concession(&mut g);
        let entry = g.state.rules.stack.last().unwrap();
        assert!(entry.paid.is_none());
        assert_eq!(entry.x, None);
        assert_eq!(entry.hybrid_colors.is_empty(), !at_x);
        // CR 601.2b: X follows the hybrid choice, so an X bound ahead of
        // it is an impossible announcement even after the game ends.
        if !at_x {
            let mut forged = g.fork();
            forged.state.rules.stack.last_mut().unwrap().x = Some(1.into());
            assert!(
                forged
                    .validate_state()
                    .unwrap_err()
                    .contains("announcement order")
            );
        }
    }
}

/// The last opponents concede, and the game finishes for seat 0 (CR 104.2a,
/// 800.4a); the finished state validates, round-trips and replays.
fn finish_by_concession(g: &mut Game) {
    for opponent in 1..4 {
        if g.is_live(PlayerId(opponent)) {
            g.concede(PlayerId(opponent)).unwrap();
        }
    }
    let mut end = g.advance(Budget::new(1));
    for _ in 0..16 {
        if !matches!(
            end,
            Advance::Suspended { .. } | Advance::AwaitingDecision(_)
        ) {
            break;
        }
        g.drain_trace(&mut |_| {});
        end = g.advance(Budget::new(1));
    }
    assert!(
        matches!(
            end,
            Advance::Finished {
                winner: PlayerId(0)
            }
        ),
        "{end:?}"
    );
    checked(g);
    assert_recoverable((&*g).into());
}

/// CR 601.2f: Corrosive Gale's cost locks only after its hybrid symbol and
/// X are announced, so a locked entry that lost either fails validation
/// after the game ends mid-payment.
#[test]
fn corrosive_gale_locked_after_concession_keeps_its_announcements() {
    let prepared = board(&[(
        CORROSIVE_GALE,
        &[Cell::XManaCost, Cell::HybridManaCost, Cell::DamageSet],
    )])
    .permanent_or_hand("Corrosive Gale", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    pick(
        &mut g,
        0,
        Reason::HybridCost,
        &[Candidate::HybridPayment {
            payment: HybridPayment::PayTwoLife,
        }],
    );
    let d = expect(&mut g, 0, Reason::AnnounceX);
    g.respond(d.player, d.id, Answer::Number { value: 0.into() })
        .unwrap();
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{d:?}");
    finish_by_concession(&mut g);
    let entry = g.state.rules.stack.last().unwrap();
    assert!(entry.cost_lock.is_some() && entry.paid.is_none());
    for (hybrid, x) in [(true, false), (false, true), (true, true)] {
        let mut forged = g.fork();
        let entry = forged.state.rules.stack.last_mut().unwrap();
        if hybrid {
            entry.hybrid_colors.clear();
        }
        if x {
            entry.x = None;
        }
        assert!(
            forged
                .validate_state()
                .unwrap_err()
                .contains("announcement order"),
            "{hybrid} {x}"
        );
    }
}

/// CR 601.2c, 601.2f: Tolarian Geyser's target is chosen before its cost
/// locks, so a locked, unpaid Geyser whose game ended during mana payment
/// keeps that target.
#[test]
fn locked_tolarian_geyser_keeps_its_target_after_concession() {
    let prepared = board(&[(TOLARIAN_GEYSER, &[Cell::Kicker])])
        .permanent_or_hand("Tolarian Geyser", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    let d = expect(&mut g, 0, Reason::AdditionalCosts);
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![]],
        },
    )
    .unwrap();
    let bears = Driver::object(&g, 2);
    Driver::pick_object(&mut g, bears);
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Mana { .. }), "{d:?}");
    finish_by_concession(&mut g);
    let mut forged = g.fork();
    let entry = forged.state.rules.stack.last_mut().unwrap();
    assert!(entry.cost_lock.is_some() && entry.paid.is_none());
    assert!(!entry.targets.is_empty());
    entry.targets.clear();
    for announced in &mut entry.announced_targets {
        announced.clear();
    }
    assert!(
        forged
            .validate_state()
            .unwrap_err()
            .contains("locked spell target")
    );
}

/// CR 601.2f-h: payment follows the cost lock, so a paid Beast Attack
/// without its lock fails validation even after the game ends.
#[test]
fn paid_beast_attack_keeps_its_cost_lock_after_concession() {
    let prepared = board(&[(BEAST_ATTACK, &[Cell::Flashback])])
        .permanent_or_hand("Beast Attack", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
    expect(&mut g, 0, Reason::Priority);
    finish_by_concession(&mut g);
    let mut forged = g.fork();
    let entry = forged.state.rules.stack.last_mut().unwrap();
    assert!(entry.paid.is_some());
    assert!(entry.cost_lock.take().is_some());
    assert!(
        forged
            .validate_state()
            .unwrap_err()
            .contains("missing spell cost lock")
    );
}

/// CR 601.2a, 903.8: a cast spell's only origins are its owner's hand, its
/// owner's command zone as that owner's commander, and its owner's
/// graveyard with flashback; every other recorded origin fails validation.
#[test]
fn beast_attack_rejects_every_impossible_casting_origin() {
    let prepared = board(&[(BEAST_ATTACK, &[Cell::Flashback])])
        .permanent_or_hand("Beast Attack", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
    expect(&mut g, 0, Reason::Priority);
    for origin in [
        Zone::Battlefield,
        Zone::Library { owner: PlayerId(0) },
        Zone::Stack,
        Zone::Exile,
        Zone::Command { owner: PlayerId(1) },
        // Not a commander: its own command zone is no origin either.
        Zone::Command { owner: PlayerId(0) },
        Zone::Hand { owner: PlayerId(1) },
        Zone::Graveyard { owner: PlayerId(1) },
    ] {
        let mut forged = g.fork();
        forged.state.rules.stack[0].cast_from = origin;
        assert!(
            forged
                .validate_state()
                .unwrap_err()
                .contains("casting origin"),
            "{origin:?}"
        );
    }
    settle(&mut g);
}

// Lane E: typecycling, a graveyard source exiled as its cost, and printed
// activation restrictions. Pinned CR 113.6j, 307.5, 508.1, 602.2a-b,
// 602.5b-d, 701.13a, 702.29a/e-f:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const ASH_BARRENS: &str = record!("58257464-278e-45fa-8e0b-bcd9a7500bc1");
const SHORELINE_RANGER: &str = record!("606fbf99-d3e3-4131-b991-3cb71c003834");
const ETERNAL_STUDENT: &str = record!("140cb5cf-8aa2-457c-9d65-5acefea00195");
const RUBBLEBELT_MAVERICK: &str = record!("1ba59349-a162-4444-b3e0-cddfc01ef03b");
const DRAKE_HATCHLING: &str = record!("bd55bcc4-f0c5-4c09-8b3b-2e25e9fbd9ee");
const GATE_TO_PHYREXIA: &str = record!("bbb005de-bbba-458e-87c1-912a004e80da");
const SHU_FARMER: &str = record!("0d059577-0790-4033-b719-a74b6a62262e");
const STEADFAST_UNICORN: &str = record!("c823d38c-e73c-4e09-ab20-c5e7de1f1c64");

/// Pass priority until a non-priority choice is pending.
fn next_choice(g: &mut Game) -> Decision<Rules> {
    for _ in 0..16 {
        let d = Driver::request(g, 1);
        if d.reason != Reason::Priority {
            checked(g);
            return d;
        }
        let pass = d
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        g.respond(d.player, d.id, pass).unwrap();
    }
    panic!("no choice within 16 priority passes")
}

fn offered_objects(d: &Decision<Rules>) -> Vec<ObjectId> {
    d.candidates
        .iter()
        .map(|c| match c {
            Candidate::Object { object } => *object,
            other => panic!("{other:?}"),
        })
        .collect()
}

fn shuffles(g: &Game, seat: u8) -> u64 {
    g.state.rules.match_state.as_ref().unwrap().library_shuffles[usize::from(seat)]
}

fn activate(object: ObjectId, ability: u32) -> Action<Rules> {
    Action::ActivateAbility { object, ability }
}

/// Ash Barrens: "Basic landcycling {1}" and Shoreline Ranger: "Islandcycling
/// {2}". Each discards itself from hand after mana, then searches for one
/// matching card, reveals it, puts it into hand and always shuffles
/// (CR 702.29a, 702.29e, 701.23a-b).
#[test]
fn typecycling_searches_only_its_land_type_and_always_shuffles() {
    for (raw, name, cost, expected) in [
        (ASH_BARRENS, "Ash Barrens", &[1u32][..], &[3u32, 5][..]),
        (SHORELINE_RANGER, "Shoreline Ranger", &[1, 2][..], &[3][..]),
    ] {
        for find in [true, false] {
            let prepared = board(&[(raw, &[Cell::Typecycling])])
                .seed([7; 32])
                .permanent_or_hand(name, 0, 0, true)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .library_card("Island", 0)
                .library_card("Grizzly Bears", 0)
                .library_card("Forest", 0)
                .library_card("Island", 1)
                .prepare();
            let mut g = Game::with_trace(prepared, TraceMode::Detailed);
            let source = Driver::object(&g, 0);
            // Only the hand ability is offered; the land's mana ability is not
            // a hand activation.
            assert!(actions(&mut g).contains(&activate(source, 0)));
            Driver::action(&mut g, activate(source, 0));
            checked(&g);
            Driver::pay(&mut g, cost);
            assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
            let d = next_choice(&mut g);
            assert_eq!((d.player, &d.reason), (PlayerId(0), &Reason::SearchLibrary));
            let lands: Vec<_> = expected.iter().map(|id| Driver::object(&g, *id)).collect();
            assert_eq!(offered_objects(&d), lands, "{name}");
            let picks = if find {
                vec![d.index(&Candidate::Object { object: lands[0] }).unwrap()]
            } else {
                vec![]
            };
            g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
                .unwrap();
            settle(&mut g);
            assert_eq!(shuffles(&g, 0), 1);
            assert_eq!(shuffles(&g, 1), 0);
            assert_eq!(
                zone(&g, 3),
                Some(if find {
                    Zone::Hand { owner: PlayerId(0) }
                } else {
                    Zone::Library { owner: PlayerId(0) }
                })
            );
            assert_eq!(hand_size(&g, 0), usize::from(find));
        }
    }
}

/// Eternal Student: "{1}{B}, Exile this card from your graveyard: Create two
/// 1/1 white and black Inkling creature tokens with flying." Only the owner
/// activates it from that owner's graveyard; mana is paid, then the card is
/// exiled as the cost (CR 113.6j, 602.2a-b, 701.13a).
#[test]
fn eternal_student_exiles_itself_from_its_owners_graveyard() {
    let prepared = board(&[(ETERNAL_STUDENT, &[Cell::GraveyardExileSource])])
        .card(
            "Eternal Student",
            0,
            0,
            Zone::Graveyard { owner: PlayerId(0) },
        )
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .card(
            "Eternal Student",
            1,
            1,
            Zone::Graveyard { owner: PlayerId(1) },
        )
        .permanent_or_hand("Eternal Student", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let student = Driver::object(&g, 0);
    let theirs = Driver::object(&g, 3);
    let in_play = Driver::object(&g, 4);
    let offered = actions(&mut g);
    assert!(offered.contains(&activate(student, 0)));
    // Another owner's graveyard and the battlefield are not where it works.
    assert!(!offered.contains(&activate(theirs, 0)));
    assert!(!offered.contains(&activate(in_play, 0)));
    assert!(g.validate_activation(PlayerId(1), theirs, 0).is_ok());
    assert!(g.validate_activation(PlayerId(0), theirs, 0).is_err());
    let before = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield)
        .count();
    Driver::action(&mut g, activate(student, 0));
    Driver::request(&mut g, u64::MAX);
    checked(&g);
    // Unpaid: the source stays in the graveyard until the cost is paid.
    assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
    let mut forged = g.fork();
    forged.state.rules.stack[0].cast_from = Zone::Hand { owner: PlayerId(0) };
    assert!(forged.validate_state().is_err());
    Driver::pay(&mut g, &[1, 2]);
    assert_eq!(zone(&g, 0), Some(Zone::Exile));
    // A paid entry whose source is still in the graveyard is impossible.
    let mut forged = g.fork();
    forged.state.rules.stack[0].paid = None;
    assert!(forged.validate_state().is_err());
    settle(&mut g);
    let after = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield)
        .count();
    assert_eq!(after, before + 2);
}

/// Rubblebelt Maverick: "{G}, Exile this card from your graveyard: Put a
/// +1/+1 counter on target creature. Activate only as a sorcery." The target
/// is chosen before mana and the exile (CR 601.2c, 602.2b, 602.5d).
#[test]
fn rubblebelt_maverick_targets_then_exiles_itself_at_sorcery_speed() {
    for step in [TurnStep::PrecombatMain, TurnStep::Upkeep] {
        let prepared = board(&[(RUBBLEBELT_MAVERICK, &[Cell::GraveyardExileSource])])
            .step(step)
            .card(
                "Rubblebelt Maverick",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            )
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let maverick = Driver::object(&g, 0);
        let bears = Driver::object(&g, 2);
        if step == TurnStep::Upkeep {
            assert!(!actions(&mut g).contains(&activate(maverick, 0)));
            continue;
        }
        Driver::action(&mut g, activate(maverick, 0));
        Driver::pick_object(&mut g, bears);
        assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
        Driver::pay(&mut g, &[1]);
        assert_eq!(zone(&g, 0), Some(Zone::Exile));
        settle(&mut g);
        assert_eq!(
            g.current(bears).unwrap().plus_one_counters,
            Quantity::from(1)
        );
    }
}

/// Drake Hatchling: "{U}: This creature gets +1/+0 until end of turn.
/// Activate only once each turn." A completed activation uses the limit for
/// this object; it is not offered again that turn (CR 602.5b).
#[test]
fn drake_hatchling_activates_only_once_each_turn() {
    let prepared = board(&[
        (DRAKE_HATCHLING, &[Cell::ActivationRestriction]),
        (SHU_FARMER, &[Cell::ActivationRestriction]),
    ])
    .permanent_or_hand("Drake Hatchling", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Shu Farmer", 0, 0, false)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let drake = Driver::object(&g, 0);
    let farmer = Driver::object(&g, 3);
    let pump = activate(drake, 0);
    assert!(actions(&mut g).contains(&pump));
    Driver::action(&mut g, pump.clone());
    Driver::request(&mut g, u64::MAX);
    assert_eq!(g.state.rules.stack.len(), 1);
    // Announced but unpaid: the limit is not used yet.
    assert!(g.state.rules.activation_uses.is_empty());
    Driver::pay(&mut g, &[1]);
    assert_eq!(g.state.rules.activation_uses.len(), 1);
    // A paid limited activation without its recorded use is impossible.
    let mut forged = g.fork();
    forged.state.rules.activation_uses.clear();
    assert!(forged.validate_state().is_err());
    // A use recorded for an unlimited ability is impossible.
    let mut forged = g.fork();
    let mut wrong = *forged.state.rules.activation_uses.iter().next().unwrap();
    wrong.source = farmer;
    forged.state.rules.activation_uses.insert(wrong);
    assert!(forged.validate_state().is_err());
    assert!(!actions(&mut g).contains(&pump));
    settle(&mut g);
    assert!(!actions(&mut g).contains(&pump));
    assert!(g.validate_activation(PlayerId(0), drake, 0).is_err());
}

/// Gate to Phyrexia: "Sacrifice a creature: Destroy target artifact.
/// Activate only during your upkeep and only once each turn." Shu Farmer:
/// "{T}: You gain 1 life. Activate only during your turn, before attackers
/// are declared." Steadfast Unicorn: "Activate only during your turn."
/// (CR 602.5b, 307.5, 508.1)
#[test]
fn printed_windows_gate_activation_by_turn_and_step() {
    let cases = [
        (TurnStep::Upkeep, [true, true, true]),
        (TurnStep::PrecombatMain, [false, true, true]),
        (TurnStep::BeginningCombat, [false, true, true]),
        (TurnStep::PostcombatMain, [false, false, true]),
    ];
    for (step, [gate_ok, farmer_ok, unicorn_ok]) in cases {
        let prepared = board(&[
            (GATE_TO_PHYREXIA, &[Cell::ActivationRestriction]),
            (SHU_FARMER, &[Cell::ActivationRestriction]),
            (STEADFAST_UNICORN, &[Cell::ActivationRestriction]),
        ])
        .step(step)
        .permanent_or_hand("Gate to Phyrexia", 0, 0, false)
        .permanent_or_hand("Shu Farmer", 0, 0, false)
        .permanent_or_hand("Steadfast Unicorn", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Shu Farmer", 1, 1, false)
        .permanent_or_hand("Steadfast Unicorn", 1, 1, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Gate to Phyrexia", 1, 1, false)
        .permanent_or_hand("Sol Ring", 1, 1, false)
        .prepare();
        let g = Game::with_trace(prepared, TraceMode::Detailed);
        let ok = |card, seat| {
            g.validate_activation(PlayerId(seat), Driver::object(&g, card), 0)
                .is_ok()
        };
        assert_eq!(
            [ok(0, 0), ok(1, 0), ok(2, 0)],
            [gate_ok, farmer_ok, unicorn_ok],
            "{step:?}"
        );
        // Never on another player's turn, at any step.
        assert!(!ok(7, 1) && !ok(8, 1), "{step:?}");
    }
}
