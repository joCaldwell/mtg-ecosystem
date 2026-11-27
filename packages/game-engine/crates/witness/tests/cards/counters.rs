//! Lane COUNTERS: counter kinds beyond +1/+1 from exact frozen R5 source on
//! private boards: -1/-1 counters and their state-based death and
//! annihilation, proliferate across kinds, and removal costs after
//! annihilation. Every decision is checked for state validity and an exact
//! snapshot round trip; every finished resolution replays from its journal.
//! Pinned CR 107.3, 122.1a, 122.3, 602.2b, 701.34a, 704.5f, 704.5q:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::HybridPayment;
use mtg_contracts::kir::{Cell, CounterKind, Tiers, definition_cells, definition_tiers};
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

const BLACK_SUNS_ZENITH: &str = record!("038c0165-32b6-4e81-8180-604b49905207");
const CONTAGION_CLASP: &str = record!("43f2d81e-aa01-4fa9-9046-6a27a05dbd2d");
const SCAR: &str = record!("4eafe158-abff-43e9-aeae-596430c5ce21");
const SPIKE_FEEDER: &str = record!("5b147eab-39dd-4c2a-8f59-d778037788aa");

/// A two-seat main-phase board whose catalog adds `cards`, each compiled
/// from frozen source and occupying the named cells; a new counter cell
/// earns `Executes` only.
fn board(cards: &[(&str, &[Cell])]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(raw, cells) in cards {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen counter witness failed: {result:?}");
        };
        let occupied = definition_cells(&definition).unwrap();
        for cell in cells {
            assert!(
                occupied.contains(cell),
                "{} lacks {cell:?}: {occupied:?}",
                definition.name
            );
        }
        if !cells.is_empty() {
            assert_eq!(
                definition_tiers(&definition).unwrap(),
                Tiers::EXECUTES,
                "{}",
                definition.name
            );
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "counter-kinds")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .seed([7; 32])
}

/// State validation and an exact snapshot round trip.
fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// The next decision, which must be `seat`'s `reason`, after a check.
fn expect(g: &mut Game, seat: u8, reason: &Reason<Rules>) -> Decision<Rules> {
    for _ in 0..24 {
        let d = Driver::request(g, u64::MAX);
        if d.reason == *reason {
            assert_eq!(d.player, PlayerId(seat), "{d:?}");
            checked(g);
            return d;
        }
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        Driver::pass(g);
    }
    panic!("expected {reason:?}");
}

fn targets(g: &mut Game, seat: u8) -> Decision<Rules> {
    for _ in 0..24 {
        let d = Driver::request(g, u64::MAX);
        if matches!(d.reason, Reason::Targets { .. }) {
            assert_eq!(d.player, PlayerId(seat), "{d:?}");
            checked(g);
            return d;
        }
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        Driver::pass(g);
    }
    panic!("expected a target decision");
}

fn answer(g: &mut Game, d: &Decision<Rules>, candidates: &[Candidate<Rules>]) {
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
        Driver::pass(g);
    }
    panic!("no empty-stack priority");
}

fn minus(g: &Game, card: u32) -> Quantity {
    g.current(Driver::object(g, card))
        .unwrap()
        .counter_count(&CounterKind::MinusOneMinusOne)
}

fn zone(g: &Game, card: u32) -> Zone {
    g.zone(Driver::object(g, card)).unwrap()
}

fn toughness(g: &Game, card: u32) -> Quantity {
    let object = Driver::object(g, card);
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(1), Budget::unlimited())
    else {
        panic!("missing observation");
    };
    let handle = o
        .view
        .objects
        .iter()
        .find(|v| v.oracle_id == g.state.rules.cards[&object.card].oracle_id && v.zone == Zone::Battlefield)
        .unwrap();
    handle
        .characteristics
        .as_ref()
        .unwrap()
        .toughness
        .clone()
        .unwrap()
}

/// Black Sun's Zenith: "Put X -1/-1 counters on each creature. Shuffle
/// Black Sun's Zenith into its owner's library." The announced X is placed
/// on every creature in one event; a 2/2 dies as a state-based action, a
/// 6/6 survives as a 4/4 whose public view shows the counters (CR 107.3,
/// 122.1a, 704.5f).
#[test]
fn black_suns_zenith_puts_x_minus_one_counters_on_each_creature() {
    let prepared = board(&[(BLACK_SUNS_ZENITH, &[Cell::KindCountersSet])])
        .permanent_or_hand("Black Sun's Zenith", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
        .libraries("Island", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    let d = expect(&mut g, 0, &Reason::AnnounceX);
    g.respond(d.player, d.id, Answer::Number { value: 2.into() })
        .unwrap();
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    settle(&mut g);
    assert_eq!(zone(&g, 5), Zone::Graveyard { owner: PlayerId(0) });
    assert_eq!(zone(&g, 6), Zone::Battlefield);
    assert_eq!(minus(&g, 6), Quantity::from(2));
    assert_eq!(toughness(&g, 6), Quantity::from(4));
    assert_eq!(zone(&g, 0), Zone::Library { owner: PlayerId(0) });
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("missing observation");
    };
    assert!(o.view.objects.iter().any(|v| v.counters
        == vec![VisibleCounter {
            kind: CounterKind::MinusOneMinusOne,
            count: 2.into(),
        }]));
    // CR 122.1: a -1/-1 counter row must be positive and on the battlefield.
    let mut forged = g.fork();
    let dreadmaw = Driver::object(&g, 6).card;
    forged.state.rules.objects.get_mut(&dreadmaw).unwrap().counters[0].count = 0.into();
    assert!(
        forged
            .validate_state()
            .unwrap_err()
            .contains("counter invariant")
    );
}

/// Contagion Clasp: its entry trigger puts a -1/-1 counter on a target
/// creature, and its proliferate adds another counter of each kind the
/// chosen permanent has (CR 122.1a, 701.34a).
#[test]
fn contagion_clasp_proliferates_minus_one_counters() {
    let mut scenario = board(&[(CONTAGION_CLASP, &[Cell::KindCountersTarget])])
        .permanent_or_hand("Contagion Clasp", 0, 0, true)
        .permanent_or_hand("Colossal Dreadmaw", 1, 1, false);
    for _ in 0..6 {
        scenario = scenario.permanent_or_hand("Island", 0, 0, false);
    }
    let mut g = Game::with_trace(scenario.libraries("Island", 2).prepare(), TraceMode::Detailed);
    let dreadmaw = Driver::object(&g, 1);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[2, 3]);
    let d = targets(&mut g, 0);
    answer(&mut g, &d, &[Candidate::Object { object: dreadmaw }]);
    settle(&mut g);
    assert_eq!(minus(&g, 1), Quantity::from(1));
    let clasp = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: clasp,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[4, 5, 6, 7]);
    let d = expect(&mut g, 0, &Reason::Proliferate);
    assert_eq!(d.candidates, vec![Candidate::Object { object: dreadmaw }]);
    answer(&mut g, &d, &[Candidate::Object { object: dreadmaw }]);
    settle(&mut g);
    assert_eq!(minus(&g, 1), Quantity::from(2));
    assert_eq!(toughness(&g, 1), Quantity::from(4));
}

/// Scar on Spike Feeder (entered with two +1/+1 counters): the -1/-1 and
/// one +1/+1 counter annihilate as a state-based action, leaving a 1/1 whose
/// "Remove a +1/+1 counter" cost is payable exactly once; paying it leaves
/// a 0/0 that dies (CR 122.3, 602.2b, 704.5f, 704.5q).
#[test]
fn scar_annihilates_with_plus_one_counters_and_bounds_their_removal_cost() {
    let prepared = board(&[(SCAR, &[Cell::KindCountersTarget]), (SPIKE_FEEDER, &[])])
        .permanent_or_hand("Spike Feeder", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Scar", 1, 1, true)
        .permanent_or_hand("Swamp", 1, 1, false)
        .libraries("Island", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    let feeder = Driver::object(&g, 0);
    assert_eq!(g.current(feeder).unwrap().plus_one_counters, Quantity::from(2));
    // Seat 1 casts Scar at instant speed in response to nothing.
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.player, PlayerId(0));
    Driver::pass(&mut g);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!((d.player, &d.reason), (PlayerId(1), &Reason::Priority));
    Driver::cast(&mut g, CardId(4));
    let d = expect(&mut g, 1, &Reason::HybridCost);
    answer(
        &mut g,
        &d,
        &[Candidate::HybridPayment {
            payment: HybridPayment::Mana(Color::B),
        }],
    );
    let d = targets(&mut g, 1);
    answer(&mut g, &d, &[Candidate::Object { object: feeder }]);
    Driver::pay(&mut g, &[5]);
    settle(&mut g);
    let o = g.current(feeder).unwrap();
    assert_eq!(o.plus_one_counters, Quantity::from(1));
    assert!(o.counters.is_empty(), "{:?}", o.counters);
    assert_eq!(toughness(&g, 0), Quantity::from(1));
    let gain = Action::ActivateAbility {
        object: feeder,
        ability: 1,
    };
    let d = Driver::request(&mut g, u64::MAX);
    if d.player != PlayerId(0) {
        Driver::pass(&mut g);
    }
    Driver::action(&mut g, gain);
    settle(&mut g);
    assert_eq!(zone(&g, 0), Zone::Graveyard { owner: PlayerId(0) });
}
