//! Track 3A wave W10: variable, hybrid and Phyrexian mana costs, cycling,
//! kicker, flashback, a spell's additional sacrifice and activation cost
//! terms, from exact frozen R4 source on private boards. Every choice is
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
            "../../../../tests/fixtures/phase1-r4/",
            $id,
            ".json"
        ))
    };
}

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
        let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
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
