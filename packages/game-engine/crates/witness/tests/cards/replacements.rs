//! E2: replacement before commit, first tranche. Token quantity, counter
//! quantity and fixed counters on entry from exact frozen R5 source on
//! private boards. A proposal is matched against the battlefield's static
//! quantity replacements, ordered by the affected object's controller when
//! the order matters, transformed once per instance, recorded in lineage and
//! committed as one atomic group.
//! CR 111.1, 111.13, 122.6, 122.6a, 306.5b, 614.1a, 614.1c, 614.5, 614.6,
//! 614.12, 614.16, 616.1, 616.1e-f, 616.2, 714.3a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{self, StaticEffect, StaticRule};
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

/// "If an effect would create one or more tokens under your control, it
/// creates twice that many of those tokens instead. / If an effect would put
/// one or more counters on a permanent you control, it puts twice that many
/// of those counters on that permanent instead."
const DOUBLING_SEASON: &str = record!("01546b7d-a233-4176-8843-d732074dc5b6");
/// "If one or more +1/+1 counters would be put on a creature you control,
/// twice that many +1/+1 counters are put on that creature instead."
const BRANCHING_EVOLUTION: &str = record!("28fe909b-06e0-424c-9f75-c824a25f5865");
/// "If an effect would create one or more tokens under your control, it
/// creates twice that many of those tokens instead."
const ANOINTED_PROCESSION: &str = record!("7246d45b-2185-4cdd-981b-5419b7d52bce");
/// Same text as Anointed Procession.
const PARALLEL_LIVES: &str = record!("84dc94b2-95fb-4d53-aaa2-191cb645639f");
/// "If one or more +1/+1 counters would be put on a creature you control,
/// that many plus one +1/+1 counters are put on it instead."
const HARDENED_SCALES: &str = record!("a1f3da21-af6d-450e-bf0b-985d158418e6");
/// "If one or more +1/+1 counters would be put on a creature you control,
/// twice that many +1/+1 counters are put on it instead."
const CORPSEJACK_MENACE: &str = record!("ca0cc02b-b106-4eca-9388-d4b48dd3be49");
/// "If one or more -1/-1 counters would be put on a creature you control,
/// that many minus one -1/-1 counters are put on it instead."
const VIZIER_OF_REMEDIES: &str = record!("79770e65-740a-44c7-bea2-a24e6a722c22");
/// "Put a +1/+1 counter on each of up to two target creatures."
const GIRD_FOR_BATTLE: &str = record!("bc72d463-8818-4c19-bd5a-f47841d1adb0");
/// "When this creature enters, put a +1/+1 counter on each of up to two
/// target creatures."
const GAVONY_SILVERSMITH: &str = record!("a0ee4faa-1ab0-4b4f-a564-ee2358500daf");

const COHORT: [&str; 6] = [
    DOUBLING_SEASON,
    BRANCHING_EVOLUTION,
    ANOINTED_PROCESSION,
    PARALLEL_LIVES,
    HARDENED_SCALES,
    CORPSEJACK_MENACE,
];

fn lowered(raw: &str) -> Definition {
    match mtg_witness::compile(mtg_witness::R5_MANIFEST, raw) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        other => panic!("frozen replacement witness failed: {other:?}"),
    }
}

fn quantity_rules(
    d: &Definition,
) -> Vec<(kir::QuantityEvent, mtg_contracts::QuantityModification)> {
    d.static_abilities
        .iter()
        .filter_map(|a| match &a.effect {
            StaticEffect::Rule {
                rule: StaticRule::ModifyQuantity { event, change },
                condition: None,
            } => Some((event.clone(), change.clone())),
            _ => None,
        })
        .collect()
}

/// The six whole-source quantity candidates lower to `ModifyQuantity`
/// statics, every printed ability accounted for, and validate at `Executes`.
#[test]
fn the_six_quantity_candidates_compile() {
    use kir::{CounterRecipients as R, QuantityEvent as E, ReplacementOrigin as O};
    use mtg_contracts::QuantityModification as M;
    let twice = M::Multiply { factor: 2.into() };
    let tokens = (E::CreateTokens { origin: O::Effect }, twice.clone());
    let plus_one_twice = (
        E::PutCounters {
            counter: Some(kir::CounterKind::PlusOnePlusOne),
            recipients: R::Creatures,
            origin: O::Any,
        },
        twice.clone(),
    );
    let expected = [
        vec![
            tokens.clone(),
            (
                E::PutCounters {
                    counter: None,
                    recipients: R::Permanents,
                    origin: O::Effect,
                },
                twice.clone(),
            ),
        ],
        vec![plus_one_twice.clone()],
        vec![tokens.clone()],
        vec![tokens.clone()],
        vec![(
            E::PutCounters {
                counter: Some(kir::CounterKind::PlusOnePlusOne),
                recipients: R::Creatures,
                origin: O::Any,
            },
            M::Add { amount: 1.into() },
        )],
        vec![plus_one_twice],
    ];
    for (raw, expected) in COHORT.iter().zip(expected) {
        let d = lowered(raw);
        assert_eq!(quantity_rules(&d), expected, "{}", d.name);
        assert_eq!(d.static_abilities.len(), expected.len(), "{}", d.name);
        assert_eq!(
            kir::definition_tiers(&d).unwrap(),
            kir::Tiers::EXECUTES,
            "{}",
            d.name
        );
    }
}

/// The adjacent subtraction rejects by name (CR 614.1a): "that many minus
/// one" is not an admitted quantity change.
#[test]
fn vizier_of_remedies_subtraction_rejects_by_name() {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, VIZIER_OF_REMEDIES);
    let mtg_compiler::CompileResult::Unsupported { reason, .. } = &result else {
        panic!("Vizier of Remedies lowered: {result:?}");
    };
    assert!(
        reason.ends_with("static:replacement:put-counters:change"),
        "{reason}"
    );
}

/// "Each of up to two target creatures" lowers to one placement over the
/// counted occurrence's slots (CR 115.3, 122.6).
#[test]
fn counted_counter_targets_lower_to_one_placement() {
    for raw in [GIRD_FOR_BATTLE, GAVONY_SILVERSMITH] {
        let d = lowered(raw);
        let program = d
            .spell
            .as_ref()
            .filter(|p| !p.targets.is_empty())
            .or_else(|| d.triggered.first().map(|t| &t.program))
            .expect("a targeted program");
        assert_eq!(program.target_group(), Some(2), "{}", d.name);
        let [Instruction::Kernel { node, .. }] = program.instructions.as_slice() else {
            panic!("{}: {:?}", d.name, program.instructions)
        };
        assert_eq!(
            node.verb(),
            Some(&kir::Verb::PutCounters {
                on: kir::SetExpr::Targets { slots: vec![0, 1] },
                counter: kir::CounterKind::PlusOnePlusOne,
                amount: kir::Amount::fixed(1),
            }),
            "{}",
            d.name
        );
    }
}

/// "Put a +1/+1 counter on target creature."
const BATTLEGROWTH: &str = record!("650bf82e-7f83-470c-bf4a-34281bfe9341");
/// "Vigilance / This creature enters with three +1/+1 counters on it."
const FAITHFUL_WATCHDOG: &str = record!("139fb542-89fd-4b9e-87e6-ec925bcca93a");
/// "Create two 1/1 white Soldier creature tokens."
const RAISE_THE_ALARM: &str = record!("5b2364d7-a811-4595-a1b4-224c70555ffa");
/// Saga: "I, II — Create a 2/2 white Knight creature token with vigilance.
/// III — Knights you control get +2/+1 until end of turn."
const HISTORY_OF_BENALIA: &str = record!("c15bb7eb-aaaa-4468-9641-8f706d6137e8");

const P0: PlayerId = PlayerId(0);
const MODES: [TraceMode; 4] = [
    TraceMode::Off,
    TraceMode::Compact,
    TraceMode::Stream,
    TraceMode::Detailed,
];

fn catalog(records: &[&str]) -> Catalog {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in records {
        catalog.definitions.push(lowered(raw));
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog
}

fn board(records: &[&str], seats: u8) -> Board {
    Board::new(catalog(records), "e2-replacements")
        .step(TurnStep::PrecombatMain)
        .seats(seats)
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// Pass priority until a decision other than priority, or an empty stack.
fn until_choice(g: &mut Game) -> Decision<Rules> {
    for _ in 0..48 {
        let d = Driver::request(g, u64::MAX);
        if d.reason != Reason::Priority || g.state.rules.stack.is_empty() {
            checked(g);
            return d;
        }
        Driver::pass(g);
    }
    panic!("no decision");
}

fn pick(g: &mut Game, d: &Decision<Rules>, objects: &[ObjectId]) {
    let picks = objects
        .iter()
        .map(|object| d.index(&Candidate::Object { object: *object }).unwrap())
        .collect();
    g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
        .unwrap();
}

fn plus(g: &Game, card: u32) -> Quantity {
    g.current(Driver::object(g, card))
        .unwrap()
        .plus_one_counters
        .clone()
}

fn named(g: &Game, name: &str) -> ObjectId {
    let oracle = oracle_id(&g.prepared.catalog, name).to_owned();
    g.state
        .rules
        .objects
        .values()
        .find(|o| {
            o.zone == Zone::Battlefield
                && g.copiable_definition(o.id)
                    .is_ok_and(|d| d.oracle_id == oracle)
        })
        .unwrap()
        .id
}

fn key(g: &Game, name: &str, ability: u32) -> ReplacementKey {
    ReplacementKey {
        source: named(g, name),
        ability,
    }
}

/// The lineage of every published counter member on `object`.
fn counter_lineage(
    journal: &ActualJournal,
    object: ObjectId,
) -> Vec<(Quantity, Vec<ReplacementKey>)> {
    journal
        .actuals
        .iter()
        .filter_map(|a| {
            let change = a.counters()?;
            (change.holder
                == CounterHolder::Permanent {
                    object,
                    controller: P0,
                })
            .then(|| {
                (
                    change.delta(),
                    a.lineage.as_ref().map_or(vec![], |l| l.applied.clone()),
                )
            })
        })
        .collect()
}

/// One +1/+1 counter by an effect with Hardened Scales and Doubling Season:
/// the creature's controller chooses which applies first (CR 616.1e).
/// Scales then Season gives (1 + 1) × 2 = 4; Season then Scales gives
/// 1 × 2 + 1 = 3. The choice suspends the resolution unwritten and replays
/// it, recoverably at every quantum and in every trace mode.
fn scales_and_season(mode: TraceMode, scales_first: bool) -> (Game, ActualJournal) {
    let p = board(&[BATTLEGROWTH, HARDENED_SCALES, DOUBLING_SEASON], 2)
        .permanent_or_hand("Battlegrowth", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Hardened Scales", 0, 0, false)
        .permanent_or_hand("Doubling Season", 0, 0, false)
        .libraries("Island", 6)
        .seed([12; 32])
        .prepare();
    let mut g = Game::with_trace(p, mode);
    let bears = Driver::object(&g, 2);
    Driver::cast(&mut g, CardId(0));
    let d = Driver::request(&mut g, u64::MAX);
    pick(&mut g, &d, &[bears]);
    Driver::pay(&mut g, &[1]);
    let d = until_choice(&mut g);
    assert_eq!(d.player, P0);
    assert_eq!(
        d.reason,
        Reason::ReplacementOrder {
            subject: Some(bears)
        }
    );
    // Nothing is written while the order is pending (CR 614.6).
    assert_eq!(plus(&g, 2), Quantity::zero());
    assert_recoverable((&g).into());
    let first = named(
        &g,
        if scales_first {
            "Hardened Scales"
        } else {
            "Doubling Season"
        },
    );
    g.actual_journal = Some(Box::default());
    pick(&mut g, &d, &[first]);
    let d = until_choice(&mut g);
    assert_eq!(d.reason, Reason::Priority);
    let journal = *g.actual_journal.take().unwrap();
    (g, journal)
}

#[test]
fn scales_and_season_order_gives_four_or_three() {
    for scales_first in [true, false] {
        let mut digests = vec![];
        for mode in MODES {
            let (g, journal) = scales_and_season(mode, scales_first);
            let expected = if scales_first { 4 } else { 3 };
            assert_eq!(plus(&g, 2), Quantity::from(expected), "{mode:?}");
            let scales = key(&g, "Hardened Scales", 0);
            let season = key(&g, "Doubling Season", 1);
            let order = if scales_first {
                vec![scales, season]
            } else {
                vec![season, scales]
            };
            assert_eq!(
                counter_lineage(&journal, Driver::object(&g, 2)),
                vec![(Quantity::from(expected), order)]
            );
            checked(&g);
            assert_recoverable((&g).into());
            digests.push(g.state_digest().unwrap());
        }
        assert!(digests.windows(2).all(|w| w[0] == w[1]));
    }
}

/// Multiplications commute, so Corpsejack Menace and Branching Evolution
/// apply in instance order without a question: 1 × 2 × 2 = 4.
#[test]
fn commuting_multipliers_apply_without_a_choice() {
    let p = board(&[BATTLEGROWTH, CORPSEJACK_MENACE, BRANCHING_EVOLUTION], 2)
        .permanent_or_hand("Battlegrowth", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Corpsejack Menace", 0, 0, false)
        .permanent_or_hand("Branching Evolution", 0, 0, false)
        .libraries("Island", 6)
        .prepare();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let bears = Driver::object(&g, 2);
    Driver::cast(&mut g, CardId(0));
    let d = Driver::request(&mut g, u64::MAX);
    pick(&mut g, &d, &[bears]);
    Driver::pay(&mut g, &[1]);
    let d = until_choice(&mut g);
    assert_eq!(d.reason, Reason::Priority);
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(plus(&g, 2), Quantity::from(4));
    // Corpsejack's own counters on another creature it controls only.
    assert_eq!(plus(&g, 3), Quantity::zero());
    checked(&g);
    assert_recoverable((&g).into());
}

/// Fixed counters on a card's entry (CR 122.6, 614.1c): Faithful Watchdog
/// enters with three, (3 + 1) × 2 = 8 under Scales then Season. The
/// entering spell is the choice's subject.
#[test]
fn entry_counters_on_a_card_are_replaced() {
    let p = board(&[FAITHFUL_WATCHDOG, HARDENED_SCALES, DOUBLING_SEASON], 2)
        .permanent_or_hand("Faithful Watchdog", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Hardened Scales", 0, 0, false)
        .permanent_or_hand("Doubling Season", 0, 0, false)
        .libraries("Island", 6)
        .prepare();
    let mut g = Game::with_trace(p, TraceMode::Stream);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let spell = Driver::object(&g, 0);
    let d = until_choice(&mut g);
    assert_eq!(
        d.reason,
        Reason::ReplacementOrder {
            subject: Some(spell)
        }
    );
    let scales = named(&g, "Hardened Scales");
    pick(&mut g, &d, &[scales]);
    let d = until_choice(&mut g);
    assert_eq!(d.reason, Reason::Priority);
    assert_eq!(plus(&g, 0), Quantity::from(8));
    checked(&g);
    assert_recoverable((&g).into());
}

/// Doubling Season's effect-only counter clause sees entry counters
/// (CR 614.16, 306.5b, 714.3a): a Saga enters with two lore counters, so
/// chapters I and II each trigger once; the precombat-main lore counter is
/// a turn-based action and is not doubled.
#[test]
fn saga_entry_lore_is_doubled_but_its_turn_based_lore_is_not() {
    let p = board(&[HISTORY_OF_BENALIA, DOUBLING_SEASON], 2)
        .permanent_or_hand("History of Benalia", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Doubling Season", 0, 0, false)
        .libraries("Island", 6)
        .prepare();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    for _ in 0..48 {
        let d = Driver::request(&mut g, u64::MAX);
        if d.reason == Reason::Priority
            && g.state.rules.stack.is_empty()
            && g.current(Driver::object(&g, 0))
                .is_ok_and(|o| o.zone == Zone::Battlefield)
        {
            break;
        }
        match d.reason {
            Reason::OrderTriggers => {
                let Shape::Order { items, .. } = &d.shape else {
                    panic!("{d:?}")
                };
                g.respond(
                    d.player,
                    d.id,
                    Answer::Order {
                        order: items.clone(),
                    },
                )
                .unwrap();
            }
            _ => Driver::pass(&mut g),
        }
    }
    let saga = g.current(Driver::object(&g, 0)).unwrap();
    assert_eq!(
        saga.counter_count(&kir::CounterKind::Lore),
        Quantity::from(2)
    );
    let knights = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && g.state.rules.tokens.contains_key(&o.id.card))
        .count();
    // Two chapter triggers, each creating one Knight doubled to two.
    assert_eq!(knights, 4);
    checked(&g);
    assert_recoverable((&g).into());
}

/// Raise the Alarm's two Soldiers become four under Parallel Lives and
/// eight with Anointed Procession too (CR 614.1a, 616.1e: they commute).
/// Two invocations make two cohorts, each its own creation event with its
/// own invocation and lineage.
#[test]
fn doubled_token_cohorts_are_per_invocation() {
    let p = board(&[RAISE_THE_ALARM, PARALLEL_LIVES, ANOINTED_PROCESSION], 2)
        .permanent_or_hand("Raise the Alarm", 0, 0, true)
        .permanent_or_hand("Raise the Alarm", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Parallel Lives", 0, 0, false)
        .permanent_or_hand("Anointed Procession", 0, 0, false)
        .libraries("Island", 6)
        .prepare();
    for mode in MODES {
        let mut g = Game::with_trace(p.clone(), mode);
        g.actual_journal = Some(Box::default());
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[2, 3]);
        let first = g.state.rules.stack.last().unwrap().id;
        Driver::cast(&mut g, CardId(1));
        Driver::pay(&mut g, &[4, 5]);
        let second = g.state.rules.stack.last().unwrap().id;
        let d = until_choice(&mut g);
        assert_eq!(d.reason, Reason::Priority);
        let journal = *g.actual_journal.take().unwrap();
        let cohorts: Vec<(StackId, usize, Vec<ReplacementKey>)> = journal
            .actuals
            .iter()
            .filter_map(|a| match a.facet(FacetKind::Created)? {
                Facet::Created { invocation, .. } => Some((
                    invocation.unwrap(),
                    a.lineage.as_ref().unwrap().applied.clone(),
                    a.id.occurrence.clone(),
                )),
                _ => None,
            })
            .fold(
                vec![],
                |mut out: Vec<(StackId, usize, Vec<ReplacementKey>, OccurrenceId)>,
                 (inv, applied, id)| {
                    match out.iter_mut().find(|row| row.3 == id) {
                        Some(row) => row.1 += 1,
                        None => out.push((inv, 1, applied, id)),
                    }
                    out
                },
            )
            .into_iter()
            .map(|(inv, n, applied, _)| (inv, n, applied))
            .collect();
        let lives = key(&g, "Parallel Lives", 0);
        let procession = key(&g, "Anointed Procession", 0);
        let mut applied = vec![lives, procession];
        applied.sort();
        // The later spell resolves first.
        assert_eq!(
            cohorts,
            vec![(second, 8, applied.clone()), (first, 8, applied)],
            "{mode:?}"
        );
        assert_eq!(g.state.rules.cast_history[0].tokens_created, 16, "{mode:?}");
        checked(&g);
        assert_recoverable((&g).into());
    }
}

/// A creation the replacement makes too large faults before any token is
/// made: two Soldiers would fit, the doubled four do not (no partial group).
#[test]
fn an_oversized_replaced_creation_allocates_nothing() {
    let p = board(&[RAISE_THE_ALARM, PARALLEL_LIVES], 2)
        .permanent_or_hand("Raise the Alarm", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Parallel Lives", 0, 0, false)
        .libraries("Island", 6)
        .prepare();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    // Fill the table, under the other player, so exactly three more fit.
    let free = g.token_capacity() as usize - g.allocated_capacity_count();
    g.commit_batch(
        BatchCause::Fixture,
        EventGroup::Occurrence {
            proposed: ProposedEvent::CreatePredefinedTokens {
                controller: PlayerId(1),
                source: Driver::object(&g, 1),
                count: free as u32 - 3,
                kind: mtg_contracts::PredefinedToken::Treasure,
                tapped: false,
            },
        },
    )
    .unwrap();
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let before = g.allocated_capacity_count();
    let soldiers = g.state.clone();
    let mut advance = Advance::Suspended {
        required_minimum: 1,
    };
    for _ in 0..16 {
        advance = g.advance(Budget::new(u64::MAX));
        match &advance {
            Advance::AwaitingDecision(d) if d.reason == Reason::Priority => Driver::pass(&mut g),
            _ => break,
        }
    }
    assert!(matches!(advance, Advance::Fault(_)), "{advance:?}");
    assert!(matches!(
        g.typed_fault(),
        Some(RuleFault::TokenCapacityExceeded { requested: 4, .. })
    ));
    assert_eq!(g.allocated_capacity_count(), before);
    assert_eq!(g.state.rules.tokens.len(), soldiers.rules.tokens.len());
}

/// "Proliferate. / Draw a card."
const CONTENTIOUS_PLAN: &str = record!("2fc6de19-9d04-422b-90bc-eee2f9f8c84c");
/// "Populate."
const WAKE_THE_REFLECTIONS: &str = record!("211868ca-662b-4054-bf50-c9e16bb58c49");

fn loyalty(g: &Game, card: u32) -> Quantity {
    g.current(Driver::object(g, card))
        .unwrap()
        .loyalty_counters
        .clone()
}

/// Cost versus effect origin (CR 118.11, 306.5b, 614.16). Doubling Season's
/// counter clause is effect-only: Jace Beleren's intrinsic entry loyalty is
/// a replacement's placement and doubles to six; its +2 loyalty cost stays
/// +2; a resolving proliferate is an effect and puts two.
#[test]
fn doubling_season_doubles_effects_but_not_a_loyalty_cost() {
    let p = board(
        &[super::planeswalker::JACE, DOUBLING_SEASON, CONTENTIOUS_PLAN],
        2,
    )
    .permanent_or_hand("Jace Beleren", 0, 0, true)
    .permanent_or_hand("Contentious Plan", 0, 0, true)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Doubling Season", 0, 0, false)
    .libraries("Island", 8)
    .prepare();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[2, 3, 4]);
    until_choice(&mut g);
    assert_eq!(loyalty(&g, 0), Quantity::from(6));
    let jace = Driver::object(&g, 0);
    g.actual_journal = Some(Box::default());
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: jace,
            ability: 0,
        },
    );
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    assert_eq!(loyalty(&g, 0), Quantity::from(8));
    let journal = *g.actual_journal.take().unwrap();
    let cost: Vec<_> = journal
        .actuals
        .iter()
        .filter(|a| a.counters().is_some())
        .map(|a| {
            (
                a.origin.root,
                a.lineage.is_some(),
                a.counters().unwrap().delta(),
            )
        })
        .collect();
    assert_eq!(
        cost,
        vec![(Root::Cost(CostOf::Loyalty), false, Quantity::from(2))]
    );
    until_choice(&mut g);
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[5, 6]);
    let d = until_choice(&mut g);
    assert_eq!(d.reason, Reason::Proliferate);
    pick(&mut g, &d, &[jace]);
    until_choice(&mut g);
    assert_eq!(loyalty(&g, 0), Quantity::from(10));
    checked(&g);
    assert_recoverable((&g).into());
}

/// An ordering choice reached in the step that consumed an input (the
/// proliferate selection) keeps that input and replays it unchanged.
#[test]
fn a_choice_after_an_input_replays_that_input() {
    let p = board(
        &[
            BATTLEGROWTH,
            CONTENTIOUS_PLAN,
            HARDENED_SCALES,
            DOUBLING_SEASON,
        ],
        2,
    )
    .permanent_or_hand("Battlegrowth", 0, 0, true)
    .permanent_or_hand("Contentious Plan", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 0, 0, false)
    .permanent_or_hand("Hardened Scales", 0, 0, false)
    .permanent_or_hand("Doubling Season", 0, 0, false)
    .libraries("Island", 8)
    .prepare();
    for mode in MODES {
        let mut g = Game::with_trace(p.clone(), mode);
        let bears = Driver::object(&g, 5);
        let scales = named(&g, "Hardened Scales");
        let season = named(&g, "Doubling Season");
        Driver::cast(&mut g, CardId(0));
        let d = Driver::request(&mut g, u64::MAX);
        pick(&mut g, &d, &[bears]);
        Driver::pay(&mut g, &[2]);
        let d = until_choice(&mut g);
        pick(&mut g, &d, &[scales]);
        until_choice(&mut g);
        assert_eq!(plus(&g, 5), Quantity::from(4));
        Driver::cast(&mut g, CardId(1));
        Driver::pay(&mut g, &[3, 4]);
        let d = until_choice(&mut g);
        assert_eq!(d.reason, Reason::Proliferate);
        pick(&mut g, &d, &[bears]);
        let d = until_choice(&mut g);
        assert_eq!(
            d.reason,
            Reason::ReplacementOrder {
                subject: Some(bears)
            }
        );
        let Frame::ReplacementOrder { order } = &g.state.rules.frame else {
            panic!("{:?}", g.state.rules.frame)
        };
        assert!(order.deferred.is_some());
        assert_eq!(plus(&g, 5), Quantity::from(4));
        assert_recoverable((&g).into());
        pick(&mut g, &d, &[season]);
        until_choice(&mut g);
        // 4 + (1 × 2 + 1).
        assert_eq!(plus(&g, 5), Quantity::from(7), "{mode:?}");
        checked(&g);
        assert_recoverable((&g).into());
    }
}

/// "Each of up to two target creatures" under Hardened Scales: one
/// simultaneous placement whose two members each get two counters, with
/// the same group and their own lineage (CR 122.6, 608.2b).
#[test]
fn a_target_group_placement_is_one_event() {
    let p = board(&[GIRD_FOR_BATTLE, HARDENED_SCALES], 2)
        .permanent_or_hand("Gird for Battle", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Hardened Scales", 0, 0, false)
        .libraries("Island", 6)
        .prepare();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let bears = [Driver::object(&g, 2), Driver::object(&g, 3)];
    Driver::cast(&mut g, CardId(0));
    let d = Driver::request(&mut g, u64::MAX);
    pick(&mut g, &d, &bears);
    Driver::pay(&mut g, &[1]);
    g.actual_journal = Some(Box::default());
    until_choice(&mut g);
    let journal = *g.actual_journal.take().unwrap();
    let scales = key(&g, "Hardened Scales", 0);
    let members: Vec<_> = journal
        .actuals
        .iter()
        .filter(|a| a.counters().is_some())
        .collect();
    assert_eq!(members.len(), 2);
    assert!(members[0].group.is_some() && members[0].group == members[1].group);
    assert_eq!(
        members[0].id.occurrence.batch,
        members[1].id.occurrence.batch
    );
    for (member, object) in members.iter().zip(bears) {
        assert_eq!(member.counters().unwrap().delta(), Quantity::from(2));
        assert_eq!(member.lineage.as_ref().unwrap().applied, vec![scales]);
        assert_eq!(
            g.current(object).unwrap().plus_one_counters,
            Quantity::from(2)
        );
    }
    checked(&g);
    assert_recoverable((&g).into());
}

/// With Scales and Season, each member of the group asks its own order.
/// After the first answer nothing is written; the second answer commits
/// both members together. A later member failing its replay leaves no
/// partial group: the forged fault keeps the pre-step state.
#[test]
fn a_target_group_asks_per_member_and_commits_once() {
    let p = board(&[GIRD_FOR_BATTLE, HARDENED_SCALES, DOUBLING_SEASON], 2)
        .permanent_or_hand("Gird for Battle", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Hardened Scales", 0, 0, false)
        .permanent_or_hand("Doubling Season", 0, 0, false)
        .libraries("Island", 6)
        .prepare();
    let mut g = Game::with_trace(p, TraceMode::Compact);
    let bears = [Driver::object(&g, 2), Driver::object(&g, 3)];
    let scales = named(&g, "Hardened Scales");
    let season = named(&g, "Doubling Season");
    Driver::cast(&mut g, CardId(0));
    let d = Driver::request(&mut g, u64::MAX);
    pick(&mut g, &d, &bears);
    Driver::pay(&mut g, &[1]);
    let first = until_choice(&mut g);
    let Reason::ReplacementOrder { subject: Some(one) } = first.reason else {
        panic!("{first:?}")
    };
    pick(&mut g, &first, &[scales]);
    let second = until_choice(&mut g);
    let Reason::ReplacementOrder {
        subject: Some(other),
    } = second.reason
    else {
        panic!("{second:?}")
    };
    assert_ne!(one, other);
    for object in bears {
        assert_eq!(
            g.current(object).unwrap().plus_one_counters,
            Quantity::zero()
        );
    }
    assert_recoverable((&g).into());

    // A forged answer the replay cannot honor faults with nothing written.
    let mut forged = g.fork();
    let digest = forged.state_digest().unwrap();
    {
        let Frame::ReplacementOrder { order } = &mut forged.state.rules.frame else {
            panic!()
        };
        // The recorded first answer names an instance it was never offered.
        order.answers[0].chosen = ReplacementKey {
            source: bears[0],
            ability: 0,
        };
    }
    let answer = second
        .pick_one(&Candidate::Object { object: season })
        .unwrap();
    forged.respond(second.player, second.id, answer).unwrap();
    assert!(matches!(
        forged.advance(Budget::new(u64::MAX)),
        Advance::Fault(_)
    ));
    for object in bears {
        assert_eq!(
            forged.current(object).unwrap().plus_one_counters,
            Quantity::zero()
        );
    }
    assert_ne!(forged.state_digest().unwrap(), digest);

    pick(&mut g, &second, &[season]);
    until_choice(&mut g);
    // (1 + 1) × 2 on the first member, 1 × 2 + 1 on the second.
    let counters = |object| g.current(object).unwrap().plus_one_counters.clone();
    assert_eq!(counters(one), Quantity::from(4));
    assert_eq!(counters(other), Quantity::from(3));
    checked(&g);
    assert_recoverable((&g).into());
}

fn gird_at_two_players(active_replacements: bool, reverse: bool) -> (Game, [ObjectId; 2]) {
    let mut board = board(&[GIRD_FOR_BATTLE, HARDENED_SCALES, DOUBLING_SEASON], 4)
        .permanent_or_hand("Gird for Battle", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 2, 2, false)
        .permanent_or_hand("Hardened Scales", 2, 2, false)
        .permanent_or_hand("Doubling Season", 2, 2, false);
    if active_replacements {
        board = board
            .permanent_or_hand("Hardened Scales", 0, 0, false)
            .permanent_or_hand("Doubling Season", 0, 0, false);
    }
    let mut game = Game::new(board.libraries("Island", 6).prepare());
    let bears = [Driver::object(&game, 2), Driver::object(&game, 3)];
    let mut targets = bears;
    if reverse {
        targets.reverse();
    }
    Driver::cast(&mut game, CardId(0));
    let d = Driver::request(&mut game, u64::MAX);
    pick(&mut game, &d, &targets);
    Driver::pay(&mut game, &[1]);
    (game, bears)
}

#[test]
fn simultaneous_replacement_choices_follow_apnap_in_either_target_order() {
    // CR 616.1, 101.4: both orders ask the active player first. The
    // target order only determines the members' positions in the batch.
    for reverse in [false, true] {
        let (mut game, [ours, theirs]) = gird_at_two_players(true, reverse);
        let first = until_choice(&mut game);
        assert_eq!(first.player, P0);
        assert_eq!(
            first.reason,
            Reason::ReplacementOrder {
                subject: Some(ours)
            }
        );
        let scales = Driver::object(&game, 6);
        pick(&mut game, &first, &[scales]);
        let second = until_choice(&mut game);
        assert_eq!(second.player, PlayerId(2));
        assert_eq!(
            second.reason,
            Reason::ReplacementOrder {
                subject: Some(theirs)
            }
        );
        assert_eq!(plus(&game, 2), Quantity::zero());
        assert_eq!(plus(&game, 3), Quantity::zero());
        let season = Driver::object(&game, 5);
        pick(&mut game, &second, &[season]);
        // An unrelated departure leaves both accepted answers applicable.
        game.concede(PlayerId(1)).unwrap();
        assert_eq!(until_choice(&mut game).reason, Reason::Priority);
        assert_eq!(plus(&game, 2), Quantity::from(4));
        assert_eq!(plus(&game, 3), Quantity::from(3));
        assert_recoverable((&game).into());
    }
}

#[test]
fn a_concession_rebuilds_a_suspended_order_and_restores() {
    // CR 800.4a: the opponent's Bear and both replacement sources leave
    // immediately. The survivor still gets the original single counter.
    let (mut game, [_, theirs]) = gird_at_two_players(false, true);
    let opponent = PlayerId(2);
    let request = until_choice(&mut game);
    assert_eq!(request.player, opponent);
    assert_eq!(
        request.reason,
        Reason::ReplacementOrder {
            subject: Some(theirs)
        }
    );
    assert_eq!(plus(&game, 2), Quantity::zero());
    game.concede(opponent).unwrap();
    assert_recoverable((&game).into());
    assert!(matches!(
        game.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    assert_eq!(game.live_players(), vec![P0, PlayerId(1), PlayerId(3)]);
    assert!(game.current(theirs).is_err());
    assert_eq!(plus(&game, 2), Quantity::zero());
    assert_recoverable((&game).into());
    let snapshot = game.snapshot().unwrap();
    let mut restored = Game::restore(game.prepared.as_ref().clone(), &snapshot).unwrap();
    for replay in [&mut game, &mut restored] {
        assert_eq!(until_choice(replay).reason, Reason::Priority);
        assert_eq!(plus(replay, 2), Quantity::from(1));
    }
    assert_eq!(restored.snapshot().unwrap(), game.snapshot().unwrap());
    assert_recoverable((&game).into());
}

/// Fixed entry counters on a token (CR 111.1, 122.6): populating a token
/// copy of Faithful Watchdog under Hardened Scales and Parallel Lives makes
/// two copies, each entering with 3 + 1 counters.
#[test]
fn entry_counters_on_a_token_are_replaced() {
    let p = board(
        &[
            FAITHFUL_WATCHDOG,
            WAKE_THE_REFLECTIONS,
            HARDENED_SCALES,
            PARALLEL_LIVES,
        ],
        2,
    )
    .permanent_or_hand("Wake the Reflections", 0, 0, true)
    .permanent_or_hand("Plains", 0, 0, false)
    .permanent_or_hand("Faithful Watchdog", 0, 0, false)
    .permanent_or_hand("Hardened Scales", 0, 0, false)
    .permanent_or_hand("Parallel Lives", 0, 0, false)
    .libraries("Island", 6)
    .prepare();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    // The private board makes the Watchdog a creature token to populate,
    // with the counters its own entry gave it.
    let dog = Driver::object(&g, 2);
    g.state
        .rules
        .objects
        .get_mut(&dog.card)
        .unwrap()
        .plus_one_counters = 3.into();
    let definition = g.copiable_definition(dog).unwrap().clone();
    std::sync::Arc::make_mut(&mut g.state.rules.tokens).insert(
        dog.card,
        tokens::TokenIdentity {
            source: dog,
            spec: tokens::TokenTemplate::TokenCopy { original: dog },
            definition,
        },
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1]);
    for _ in 0..16 {
        let d = Driver::request(&mut g, u64::MAX);
        match d.reason {
            Reason::Populate => pick(&mut g, &d, &[dog]),
            Reason::Priority if g.state.rules.stack.is_empty() => break,
            Reason::Priority => Driver::pass(&mut g),
            _ => panic!("{d:?}"),
        }
    }
    let copies: Vec<_> = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && o.id != dog
                && g.state.rules.tokens.contains_key(&o.id.card)
        })
        .map(|o| o.plus_one_counters.clone())
        .collect();
    assert_eq!(copies, vec![Quantity::from(4), Quantity::from(4)]);
}

/// Four seats: the order offer reaches only its chooser. The others see
/// no decision, no candidate and nothing of the suspended resolution.
#[test]
fn a_pending_order_is_private_to_its_chooser() {
    let p = board(&[BATTLEGROWTH, HARDENED_SCALES, DOUBLING_SEASON], 4)
        .permanent_or_hand("Battlegrowth", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Hardened Scales", 0, 0, false)
        .permanent_or_hand("Doubling Season", 0, 0, false)
        .libraries("Island", 6)
        .prepare();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let bears = Driver::object(&g, 2);
    Driver::cast(&mut g, CardId(0));
    let d = Driver::request(&mut g, u64::MAX);
    pick(&mut g, &d, &[bears]);
    Driver::pay(&mut g, &[1]);
    let d = until_choice(&mut g);
    assert_eq!(d.player, P0);
    let observe = |seat: u8| {
        let QueryResult::Complete(QueryValue::Observation(o)) =
            g.observe(PlayerId(seat), Budget::unlimited())
        else {
            panic!("missing observation");
        };
        o
    };
    let mine = observe(0);
    let pending = mine.decision.as_ref().expect("the chooser's offer");
    assert!(matches!(
        pending.reason,
        Reason::ReplacementOrder { subject: Some(_) }
    ));
    assert_eq!(pending.candidates.len(), 3);
    for seat in 1..4 {
        let theirs = observe(seat);
        assert!(theirs.decision.is_none(), "seat {seat}");
        assert_eq!(theirs.view.objects.len(), observe(seat).view.objects.len());
    }
    checked(&g);
    assert_recoverable((&g).into());
}
