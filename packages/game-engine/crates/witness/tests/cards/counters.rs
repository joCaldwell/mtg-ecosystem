//! Lane COUNTERS: counter kinds beyond +1/+1 from exact frozen R5 source on
//! private boards: -1/-1 counters and their state-based death and
//! annihilation, proliferate across kinds, and removal costs after
//! annihilation, keyword counters in layer 6. Every decision is checked for state validity and an exact
//! snapshot round trip; every finished resolution replays from its journal.
//! Pinned CR 107.3, 122.1a-b, 122.3, 613.1f, 613.7c, 602.2b, 701.34a, 704.5f, 704.5q:
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
const SPONTANEOUS_FLIGHT: &str = record!("04da8c46-9703-47f2-974b-c144464ec380");
const DARING_FIENDBONDER: &str = record!("0a746c49-cc44-467a-85b7-3c498559aea7");

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
        .find(|v| {
            v.oracle_id == g.state.rules.cards[&object.card].oracle_id
                && v.zone == Zone::Battlefield
        })
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
    forged
        .state
        .rules
        .objects
        .get_mut(&dreadmaw)
        .unwrap()
        .counters[0]
        .count = 0.into();
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
    let mut g = Game::with_trace(
        scenario.libraries("Island", 2).prepare(),
        TraceMode::Detailed,
    );
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
    assert_eq!(
        g.current(feeder).unwrap().plus_one_counters,
        Quantity::from(2)
    );
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

fn next_turn(g: &mut Game) {
    let turn = g.state.rules.turn.number;
    for _ in 0..100 {
        let r = Driver::request(g, u64::MAX);
        if g.state.rules.turn.number > turn {
            checked(g);
            return;
        }
        let answer = match r.reason {
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::Attackers => r.attack(vec![]),
            ref reason => panic!("unexpected turn boundary {reason:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("cleanup did not finish");
}

/// Spontaneous Flight: "Target creature gets +2/+2 until end of turn. Put
/// a flying counter on it." The pump ends at cleanup; the flying counter
/// keeps granting flying in later turns (CR 122.1b, 514.2, 613.1f).
#[test]
fn spontaneous_flight_flying_counter_outlasts_its_pump() {
    let prepared = board(&[(SPONTANEOUS_FLIGHT, &[Cell::KindCountersTarget])])
        .permanent_or_hand("Spontaneous Flight", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 4);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g, 0);
    answer(&mut g, &d, &[Candidate::Object { object: bears }]);
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    assert!(g.current_keywords(bears).contains(&KeywordAbility::Flying));
    assert_eq!(toughness(&g, 4), Quantity::from(4));
    next_turn(&mut g);
    assert!(g.state.rules.effects.is_empty());
    assert!(g.current_keywords(bears).contains(&KeywordAbility::Flying));
    assert_eq!(toughness(&g, 4), Quantity::from(2));
    assert_eq!(
        g.current(bears)
            .unwrap()
            .counter_count(&CounterKind::Keyword {
                keyword: KeywordAbility::Flying
            }),
        Quantity::from(1)
    );
}

/// Daring Fiendbonder: "{1}{B}, Exile this card from your graveyard: Put an
/// indestructible counter on target creature. Activate only as a sorcery."
/// The counter makes the creature survive Murder (CR 122.1b, 702.12b).
#[test]
fn daring_fiendbonder_indestructible_counter_survives_destruction() {
    let prepared = board(&[(DARING_FIENDBONDER, &[Cell::KindCountersTarget])])
        .card(
            "Daring Fiendbonder",
            0,
            0,
            Zone::Graveyard { owner: PlayerId(0) },
        )
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Murder", 1, 1, true)
        .permanent_or_hand("Swamp", 1, 1, false)
        .permanent_or_hand("Swamp", 1, 1, false)
        .permanent_or_hand("Swamp", 1, 1, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 3);
    let fiendbonder = Driver::object(&g, 0);
    let activate = Action::ActivateAbility {
        object: fiendbonder,
        ability: 0,
    };
    let d = Driver::request(&mut g, u64::MAX);
    assert!(
        d.candidates.contains(&Candidate::Action {
            action: activate.clone()
        }),
        "{d:?}"
    );
    Driver::action(&mut g, activate);
    let d = targets(&mut g, 0);
    answer(&mut g, &d, &[Candidate::Object { object: bears }]);
    Driver::pay(&mut g, &[1, 2]);
    settle(&mut g);
    assert_eq!(zone(&g, 0), Zone::Exile);
    assert!(
        g.current_keywords(bears)
            .contains(&KeywordAbility::Indestructible)
    );
    Driver::pass(&mut g);
    Driver::cast(&mut g, CardId(4));
    let d = targets(&mut g, 1);
    answer(&mut g, &d, &[Candidate::Object { object: bears }]);
    Driver::pay(&mut g, &[5, 6, 7]);
    settle(&mut g);
    assert_eq!(zone(&g, 3), Zone::Battlefield);
    assert_eq!(zone(&g, 4), Zone::Graveyard { owner: PlayerId(1) });
}

const TRANQUILIZE: &str = record!("1133633a-b717-42cb-b477-4e18584d876f");

/// Tranquilize: "Tap target creature an opponent controls and put three
/// stun counters on it." Each would-be untap, from its controller's untap
/// step or Seeker of Skybreak's "Untap target creature", removes one stun
/// counter instead; with none left it untaps (CR 122.1d, 502.3, 614.1a).
#[test]
fn tranquilize_stun_counters_replace_each_untap() {
    let prepared = board(&[(TRANQUILIZE, &[Cell::KindCountersTarget])])
        .permanent_or_hand("Tranquilize", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Seeker of Skybreak", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .libraries("Island", 8)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 4);
    let stun = |g: &Game| g.current(bears).unwrap().counter_count(&CounterKind::Stun);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g, 0);
    answer(&mut g, &d, &[Candidate::Object { object: bears }]);
    Driver::pay(&mut g, &[1, 2]);
    settle(&mut g);
    assert!(g.current(bears).unwrap().tapped);
    assert_eq!(stun(&g), Quantity::from(3));
    // An untap effect is replaced too.
    let seeker = Driver::object(&g, 3);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: seeker,
            ability: 0,
        },
    );
    let d = targets(&mut g, 0);
    answer(&mut g, &d, &[Candidate::Object { object: bears }]);
    settle(&mut g);
    assert!(g.current(bears).unwrap().tapped);
    assert_eq!(stun(&g), Quantity::from(2));
    // Seat 1's next two untap steps each remove one; the third untaps.
    let seat_one_turn = |g: &mut Game| loop {
        next_turn(g);
        if g.state.rules.active == Some(PlayerId(1)) {
            break;
        }
    };
    for expected in [1, 0] {
        seat_one_turn(&mut g);
        assert!(g.current(bears).unwrap().tapped);
        assert_eq!(stun(&g), Quantity::from(expected));
    }
    seat_one_turn(&mut g);
    assert!(!g.current(bears).unwrap().tapped);
    assert!(g.current(bears).unwrap().counters.is_empty());
}

const ASSEMBLE_THE_LEGION: &str = record!("6f81bef3-6ca0-4cf4-aa99-7b2813eeef04");
const BARRINS_CODEX: &str = record!("f7c237e6-17a4-4e72-8cc3-456aec738394");

/// Advance to seat 0's next turn, accepting every optional effect seat 0 is
/// offered on the way.
fn next_own_turn(g: &mut Game) {
    let turn = g.state.rules.turn.number;
    for _ in 0..400 {
        let r = Driver::request(g, u64::MAX);
        if g.state.rules.turn.number > turn
            && g.state.rules.active == Some(PlayerId(0))
            && r.reason == Reason::Priority
            && g.state.rules.stack.is_empty()
            && g.state.rules.turn.step != TurnStep::Untap
            && g.state.rules.turn.step != TurnStep::Upkeep
        {
            checked(g);
            return;
        }
        let answer = match r.reason {
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::Attackers => r.attack(vec![]),
            Reason::OptionalEffect => {
                checked(g);
                r.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap()
            }
            ref reason => panic!("unexpected turn boundary {reason:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("seat 0's next turn did not arrive");
}

fn tokens(g: &Game) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && g.state.rules.tokens.contains_key(&o.id.card))
        .count()
}

/// Assemble the Legion: "At the beginning of your upkeep, put a muster
/// counter on this enchantment. Then create a 1/1 ... token ... for each
/// muster counter on this enchantment." The count is read after this
/// upkeep's placement: one token, then two more (CR 122.1, 608.2h).
#[test]
fn assemble_the_legion_counts_its_own_muster_counters() {
    let prepared = board(&[(
        ASSEMBLE_THE_LEGION,
        &[Cell::KindCountersSource, Cell::CountedTokens],
    )])
    .permanent_or_hand("Assemble the Legion", 0, 0, false)
    .libraries("Island", 12)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let legion = Driver::object(&g, 0);
    let muster = CounterKind::Named {
        name: "muster".into(),
    };
    next_own_turn(&mut g);
    assert_eq!(
        g.current(legion).unwrap().counter_count(&muster),
        Quantity::from(1)
    );
    assert_eq!(tokens(&g), 1);
    next_own_turn(&mut g);
    assert_eq!(
        g.current(legion).unwrap().counter_count(&muster),
        Quantity::from(2)
    );
    assert_eq!(tokens(&g), 3);
    assert_recoverable((&g).into());
}

/// Barrin's Codex: "you may put a page counter on this artifact" each
/// upkeep; "{4}, {T}, Sacrifice this artifact: Draw X cards, where X is the
/// number of page counters on this artifact." The sacrificed source's
/// counters are read as it last existed (CR 122.1, 608.2h).
#[test]
fn barrins_codex_reads_its_last_known_page_counters() {
    let mut scenario = board(&[(BARRINS_CODEX, &[Cell::KindCountersSource])]).permanent_or_hand(
        "Barrin's Codex",
        0,
        0,
        false,
    );
    for _ in 0..4 {
        scenario = scenario.permanent_or_hand("Island", 0, 0, false);
    }
    let mut g = Game::with_trace(
        scenario.libraries("Island", 12).prepare(),
        TraceMode::Detailed,
    );
    let codex = Driver::object(&g, 0);
    next_own_turn(&mut g);
    next_own_turn(&mut g);
    let page = CounterKind::Named {
        name: "page".into(),
    };
    assert_eq!(
        g.current(codex).unwrap().counter_count(&page),
        Quantity::from(2)
    );
    let hand = |g: &Game| {
        g.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
            .count()
    };
    let before = hand(&g);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: codex,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    assert_eq!(zone(&g, 0), Zone::Graveyard { owner: PlayerId(0) });
    settle(&mut g);
    assert_eq!(hand(&g), before + 2);
}

const GRIM_AFFLICTION: &str = record!("411ba1a2-30bb-4618-b874-6be0d996f07d");
const HUMILITY: &str = record!("ed7bdb3e-5c51-4547-9266-76a791e0b2b0");

/// Grim Affliction on Spike Feeder (two +1/+1 counters): "Put a -1/-1
/// counter on target creature, then proliferate." Proliferate adds one of
/// each kind to the Feeder in one atomic placement, before state-based
/// actions annihilate them (CR 122.3, 608.2, 701.34a, 704.5q).
#[test]
fn grim_affliction_proliferates_both_kinds_on_one_permanent() {
    let prepared = board(&[
        (GRIM_AFFLICTION, &[Cell::KindCountersTarget]),
        (SPIKE_FEEDER, &[]),
    ])
    .permanent_or_hand("Spike Feeder", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Grim Affliction", 0, 0, true)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .libraries("Island", 2)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    let feeder = Driver::object(&g, 0);
    Driver::cast(&mut g, CardId(4));
    let d = targets(&mut g, 0);
    answer(&mut g, &d, &[Candidate::Object { object: feeder }]);
    Driver::pay(&mut g, &[5, 6, 7]);
    let d = expect(&mut g, 0, &Reason::Proliferate);
    answer(&mut g, &d, &[Candidate::Object { object: feeder }]);
    // One atomic placement on the Feeder carries both kinds before SBAs.
    settle(&mut g);
    let placements: Vec<_> = g
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Committed {
                actual: EventOutcome::CountersPlaced { object, placed },
                ..
            } if *object == feeder => Some(placed.clone()),
            _ => None,
        })
        .collect();
    let change = |counter: CounterKind, amount: i64, previous: i64, current: i64| CounterChange {
        counter,
        amount: amount.into(),
        previous: previous.into(),
        current: current.into(),
    };
    assert_eq!(
        placements.last().unwrap(),
        &vec![
            change(CounterKind::PlusOnePlusOne, 1, 2, 3),
            change(CounterKind::MinusOneMinusOne, 1, 1, 2),
        ],
        "{placements:?}"
    );
    let o = g.current(feeder).unwrap();
    assert_eq!(o.plus_one_counters, Quantity::from(1));
    assert!(o.counters.is_empty(), "{:?}", o.counters);
}

/// Restore a snapshot of `g` whose state JSON `tamper` edits, re-digested,
/// and return the rejection.
fn restore_rejection(g: &Game, tamper: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut forged: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
    tamper(&mut forged["state"]);
    forged["state_sha256"] = mtg_contracts::digest(&forged["state"]).unwrap().into();
    Game::restore(g.prepared.as_ref().clone(), &forged.to_string())
        .err()
        .expect("forged counter state restored")
}

/// The kinds a reserved rules name or an unexecuted keyword could forge.
fn forged_kinds() -> Vec<CounterKind> {
    let mut kinds: Vec<_> = ["shield", "loyalty", "flying"]
        .into_iter()
        .map(|name| CounterKind::Named { name: name.into() })
        .collect();
    kinds.push(CounterKind::Keyword {
        keyword: KeywordAbility::Shroud,
    });
    kinds
}

/// CR 122.1, 122.1b-c: a reserved rules name cannot hide in a plain named
/// counter, nor an unexecuted keyword in a keyword counter. Starting from a
/// valid placed stun counter, changing only its kind is rejected by state
/// validation and by restore, both on counter-kind admission.
#[test]
fn reserved_counter_names_are_rejected_in_state_and_restore() {
    let prepared = board(&[(TRANQUILIZE, &[Cell::KindCountersTarget])])
        .permanent_or_hand("Tranquilize", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .libraries("Island", 4)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 3);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g, 0);
    answer(&mut g, &d, &[Candidate::Object { object: bears }]);
    Driver::pay(&mut g, &[1, 2]);
    settle(&mut g);
    // The baseline is valid and restores exactly.
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    assert_eq!(g.current(bears).unwrap().counters.len(), 1);
    for kind in forged_kinds() {
        let mut forged = g.fork();
        forged
            .state
            .rules
            .objects
            .get_mut(&bears.card)
            .unwrap()
            .counters[0]
            .kind = kind.clone();
        let err = forged.validate_state().unwrap_err();
        assert!(
            err.contains("snapshot counter kind admission"),
            "{kind:?}: {err}"
        );
        let value = serde_json::to_value(&kind).unwrap();
        let err = restore_rejection(&g, |state| {
            state["objects"][bears.card.0.to_string()]["counters"][0]["kind"] = value;
        });
        assert!(
            err.contains("snapshot counter kind admission"),
            "{kind:?}: {err}"
        );
    }
}

/// CR 608.2h: Barrin's Codex's sacrificed source keeps its last-known page
/// counters on the stack entry. Changing only the retained kind to a
/// reserved or unexecuted one is rejected by state validation and restore.
#[test]
fn retained_source_counter_facts_are_admitted() {
    let mut scenario = board(&[(BARRINS_CODEX, &[Cell::KindCountersSource])]).permanent_or_hand(
        "Barrin's Codex",
        0,
        0,
        false,
    );
    for _ in 0..4 {
        scenario = scenario.permanent_or_hand("Island", 0, 0, false);
    }
    let mut g = Game::with_trace(
        scenario.libraries("Island", 12).prepare(),
        TraceMode::Detailed,
    );
    let codex = Driver::object(&g, 0);
    next_own_turn(&mut g);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: codex,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    assert_eq!(zone(&g, 0), Zone::Graveyard { owner: PlayerId(0) });
    let entry = g.state.rules.stack.last().unwrap();
    let page = CounterKind::Named {
        name: "page".into(),
    };
    assert_eq!(entry.source_facts.as_ref().unwrap().counters[0].kind, page);
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    let top = g.state.rules.stack.len() - 1;
    for kind in forged_kinds() {
        let mut forged = g.fork();
        forged.state.rules.stack[top]
            .source_facts
            .as_mut()
            .unwrap()
            .counters[0]
            .kind = kind.clone();
        let err = forged.validate_state().unwrap_err();
        assert!(
            err.contains("captured counter kind admission"),
            "{kind:?}: {err}"
        );
        let value = serde_json::to_value(&kind).unwrap();
        let err = restore_rejection(&g, |state| {
            state["stack"][top]["source_facts"]["counters"][0]["kind"] = value;
        });
        assert!(
            err.contains("captured counter kind admission"),
            "{kind:?}: {err}"
        );
    }
}

/// CR 613.1f, 613.7a, 613.7c: a flying counter placed before Humility's
/// "All creatures lose all abilities" is overridden by the later static; a
/// second flying counter after it gives every flying counter the later
/// timestamp, so the creature flies again.
#[test]
fn keyword_counter_timestamps_order_against_ability_loss() {
    let mut scenario = board(&[
        (SPONTANEOUS_FLIGHT, &[Cell::KindCountersTarget]),
        (HUMILITY, &[]),
    ])
    .permanent_or_hand("Spontaneous Flight", 0, 0, true)
    .permanent_or_hand("Humility", 0, 0, true)
    .permanent_or_hand("Spontaneous Flight", 0, 0, true)
    .permanent_or_hand("Grizzly Bears", 0, 0, false);
    for _ in 0..10 {
        scenario = scenario.permanent_or_hand("Plains", 0, 0, false);
    }
    let mut g = Game::with_trace(
        scenario.libraries("Island", 4).prepare(),
        TraceMode::Detailed,
    );
    let bears = Driver::object(&g, 3);
    let flying = |g: &Game| g.current_keywords(bears).contains(&KeywordAbility::Flying);
    let flight = |g: &mut Game, card: u32, lands: &[u32]| {
        Driver::cast(g, CardId(card));
        let d = targets(g, 0);
        answer(g, &d, &[Candidate::Object { object: bears }]);
        Driver::pay(g, lands);
        settle(g);
    };
    flight(&mut g, 0, &[4, 5, 6]);
    assert!(flying(&g));
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[7, 8, 9, 10]);
    settle(&mut g);
    assert!(
        !flying(&g),
        "an earlier keyword counter loses to the later loss"
    );
    flight(&mut g, 2, &[11, 12, 13]);
    assert!(flying(&g), "the refreshed timestamp follows the loss");
    let flying_counter = CounterKind::Keyword {
        keyword: KeywordAbility::Flying,
    };
    assert_eq!(
        g.current(bears).unwrap().counter_count(&flying_counter),
        Quantity::from(2)
    );
}
