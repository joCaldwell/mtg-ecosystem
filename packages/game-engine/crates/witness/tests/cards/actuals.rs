//! E1 parity gate for creation, battlefield entry and zone movement from
//! exact frozen R5 source on private boards. One writer publishes each
//! member as an `Actual`; history, trigger matching, delayed watchers and
//! mana reversal read its facets. Every family agrees across all four trace
//! modes (state digest, pending and placed triggers, ledger, the next
//! authorized decision and the published actuals), with and without a
//! matching listener, and replays, restores and forks at every quantum.
//! CR 111.1-2, 111.13, 400.7, 603.2c, 603.6a, 603.10a, 608.3, 700.4, 733.1:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
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

/// "Create two 1/1 white Soldier creature tokens."
const RAISE_THE_ALARM: &str = record!("5b2364d7-a811-4595-a1b4-224c70555ffa");
/// "Whenever a creature you control enters, this enchantment deals 1 damage
/// to each opponent."
const IMPACT_TREMORS: &str = record!("9242cd3e-1a71-4700-8182-9c1005616033");
/// "Whenever a nontoken creature an opponent controls dies, create a tapped
/// 2/2 black Zombie creature token."
const OVERSEER_OF_THE_DAMNED: &str = record!("c1f085ce-5b52-45b3-aef0-f77f36b3da36");
/// "When this creature enters, create two Treasure tokens."
const RAPACIOUS_DRAGON: &str = record!("0944ec2e-1dd9-459f-8f1d-667242cf52fe");
/// "Whenever you sacrifice a permanent, put a +1/+1 counter on Korvold and
/// draw a card."
const KORVOLD: &str = record!("9ae669dd-7e60-4649-b96e-35da28be641a");

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const MODES: [TraceMode; 4] = [
    TraceMode::Off,
    TraceMode::Compact,
    TraceMode::Stream,
    TraceMode::Detailed,
];

fn prepared(records: &[&str], cards: impl FnOnce(Board) -> Board) -> Prepared {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in records {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen actuals witness failed: {result:?}");
        };
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let board = Board::new(catalog, "e1-actuals-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2);
    cards(board).libraries("Island", 6).seed([11; 32]).prepare()
}

fn open_journal(g: &mut Game) {
    g.actual_journal = Some(Box::default());
}

fn take_journal(g: &mut Game) -> ActualJournal {
    *g.actual_journal.take().expect("an open journal")
}

/// Pass priority until the stack entry `id` has left the stack.
fn resolve(g: &mut Game, id: StackId) {
    for _ in 0..24 {
        let d = Driver::request(g, u64::MAX);
        if !g.state.rules.stack.iter().any(|s| s.id == id) {
            return;
        }
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        Driver::pass(g);
    }
    panic!("stack entry {id:?} did not resolve");
}

/// Pass priority until the stack is empty.
fn settle(g: &mut Game) {
    for _ in 0..48 {
        let d = Driver::request(g, u64::MAX);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        if g.state.rules.stack.is_empty() {
            return;
        }
        Driver::pass(g);
    }
    panic!("the stack did not empty");
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// Everything a family's parity gate compares at one boundary.
#[derive(Debug, PartialEq)]
struct Gate {
    digest: String,
    pending: String,
    stack: String,
    history: [triggers::TurnCastHistory; 4],
    decision: String,
    actuals: Vec<Actual>,
    events: Vec<EventId>,
    ledger: LedgerDelta,
}
fn gate(g: &Game, journal: &ActualJournal) -> Gate {
    Gate {
        digest: g.state_digest().unwrap(),
        pending: format!("{:?}", g.state.rules.pending_triggers),
        stack: format!("{:?}", g.state.rules.stack),
        history: g.state.rules.cast_history.clone(),
        decision: format!("{:?}", g.next_test_decision()),
        actuals: journal.actuals.clone(),
        events: journal.events.clone(),
        ledger: journal.ledger.clone(),
    }
}

/// Run `scenario` once per trace mode and require one identical gate.
fn parity(scenario: impl Fn(TraceMode) -> (Game, ActualJournal)) -> (Game, ActualJournal) {
    let mut first: Option<Gate> = None;
    let mut last = None;
    for mode in MODES {
        let (g, journal) = scenario(mode);
        let at = gate(&g, &journal);
        match &first {
            None => first = Some(at),
            Some(first) => assert_eq!(first, &at, "{mode:?}"),
        }
        last = Some((g, journal));
    }
    last.unwrap()
}

fn seat(delta: &LedgerDelta, seat: PlayerId) -> Option<&SeatDelta> {
    delta.seats.iter().find(|row| row.seat == seat)
}

fn row(seat: PlayerId) -> SeatDelta {
    SeatDelta {
        seat,
        permanent_left: false,
        creature_died: false,
        artifacts_entered: 0,
        creatures_entered: 0,
        nonlands_entered: 0,
        tokens_created: 0,
    }
}

fn created(journal: &ActualJournal) -> Vec<&Actual> {
    journal
        .actuals
        .iter()
        .filter(|a| a.facet(FacetKind::Created).is_some())
        .collect()
}

fn triggers_from(g: &Game, source: ObjectId) -> usize {
    g.state
        .rules
        .stack
        .iter()
        .filter(|s| s.source == source && matches!(s.kind, StackKind::Triggered { .. }))
        .count()
        + g.state
            .rules
            .pending_triggers
            .iter()
            .filter(|t| t.source == source)
            .count()
}

/// Raise the Alarm resolving, with Impact Tremors under `observer`'s control.
fn raise_the_alarm(mode: TraceMode, observer: u8) -> (Game, ActualJournal) {
    let p = prepared(&[RAISE_THE_ALARM, IMPACT_TREMORS], |b| {
        b.permanent_or_hand("Impact Tremors", observer, observer, false)
            .permanent_or_hand("Raise the Alarm", 0, 0, true)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
    });
    let mut g = Game::with_trace(p, mode);
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[2, 3]);
    Driver::request(&mut g, u64::MAX);
    let alarm = g.state.rules.stack.last().unwrap().id;
    open_journal(&mut g);
    resolve(&mut g, alarm);
    let journal = take_journal(&mut g);
    (g, journal)
}

/// CR 111.1-2, 603.2c, 603.6a: two tokens one instruction creates are two
/// members of one occurrence, each created by its controller and entering
/// the battlefield; the reducer counts each once by facet. Impact Tremors
/// sees each entry. With the enchantment under the opponent's control
/// nothing matches, and the same actuals and ledger change are published.
#[test]
fn creation_and_entry_publish_one_actual_per_token_with_or_without_a_listener() {
    let (g, journal) = parity(|mode| raise_the_alarm(mode, 0));
    let tokens = created(&journal);
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].id.occurrence, tokens[1].id.occurrence);
    assert_eq!((tokens[0].id.member, tokens[1].id.member), (0, 1));
    for token in &tokens {
        let Some(Facet::Created {
            creator,
            invocation,
            ..
        }) = token.facet(FacetKind::Created)
        else {
            unreachable!()
        };
        assert_eq!(*creator, P0);
        assert_eq!(*invocation, token.origin.invocation);
        assert!(invocation.is_some());
        let Some(Facet::Entered {
            object,
            controller,
            tapped,
            token: is_token,
            attached_to,
            ..
        }) = token.facet(FacetKind::Entered)
        else {
            panic!("a created token enters")
        };
        assert_eq!(
            (*controller, *tapped, *is_token, *attached_to),
            (P0, false, true, None)
        );
        assert!(token.facet(FacetKind::Moved).is_none());
        assert_eq!(token.origin.root, Root::Effect);
        assert!(token.produced_by_effect() && token.lineage.is_none());
        let reference = token.reference(FacetKind::Entered).unwrap();
        assert_eq!(
            reference.resolve(&journal.actuals),
            token.facet(FacetKind::Entered)
        );
        assert_eq!(g.current(*object).unwrap().zone, Zone::Battlefield);
    }
    // The spell itself moves to the graveyard; nothing enters with it.
    let spell = journal
        .actuals
        .iter()
        .find_map(|a| a.movement().filter(|m| m.from == Zone::Stack))
        .unwrap();
    assert_eq!(spell.destination, Zone::Graveyard { owner: P0 });
    let expected = LedgerDelta {
        seats: vec![SeatDelta {
            creatures_entered: 2,
            nonlands_entered: 2,
            tokens_created: 2,
            ..row(P0)
        }],
    };
    assert_eq!(journal.ledger, expected);
    let row0 = &g.state.rules.cast_history[0];
    assert_eq!(
        (row0.tokens_created, row0.creatures_entered.clone()),
        (2, 2.into())
    );
    let tremors = Driver::object(&g, 0);
    assert_eq!(triggers_from(&g, tremors), 2);
    checked(&g);
    assert_recoverable((&g).into());

    // Zero-match parity: the same members and ledger change.
    let (quiet, silent) = parity(|mode| raise_the_alarm(mode, 1));
    assert_eq!(created(&silent), tokens);
    assert_eq!(silent.ledger, journal.ledger);
    assert_eq!(triggers_from(&quiet, Driver::object(&quiet, 0)), 0);
    assert!(quiet.state.rules.pending_triggers.is_empty());
    assert_recoverable((&quiet).into());
}

/// Lightning Bolt kills an opponent's Grizzly Bears with Overseer of the
/// Damned under `observer`'s control.
fn bolt_the_bears(mode: TraceMode, observer: u8) -> (Game, ActualJournal) {
    let p = prepared(&[OVERSEER_OF_THE_DAMNED], |b| {
        b.permanent_or_hand("Overseer of the Damned", observer, observer, false)
            .permanent_or_hand("Lightning Bolt", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
    });
    let mut g = Game::with_trace(p, mode);
    let bears = Driver::object(&g, 3);
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[2]);
    Driver::request(&mut g, u64::MAX);
    open_journal(&mut g);
    settle(&mut g);
    let journal = take_journal(&mut g);
    (g, journal)
}

/// CR 400.7, 603.10a, 700.4: a creature dying is one movement member that
/// retains its complete prior battlefield row and names its graveyard
/// successor. The reducer records the departure and death for the prior
/// controller. Overseer of the Damned sees the death and its trigger
/// creates a tapped Zombie: creation and entry with tapped entry status,
/// never a tap (CR 603.2e). Without a matching listener the death publishes
/// the same member and ledger change.
#[test]
fn movement_retains_the_prior_row_with_or_without_a_listener() {
    let (g, journal) = parity(|mode| bolt_the_bears(mode, 0));
    let bears = Driver::object(&prepared_game(0), 3);
    let death = journal
        .actuals
        .iter()
        .find(|a| a.departure().is_some())
        .unwrap();
    let Some(Facet::Moved {
        object,
        prior: Some(prior),
        from,
        to,
        successor: Some(successor),
        how,
    }) = death.facet(FacetKind::Moved)
    else {
        panic!("a death is a movement")
    };
    assert_eq!(*object, bears);
    assert_eq!((prior.controller, prior.zone), (P1, Zone::Battlefield));
    assert!(prior.characteristics.has_type(CardKind::Creature));
    assert_eq!(prior.marked_damage, 3.into());
    assert_eq!(
        (*from, *to, *how),
        (
            Zone::Battlefield,
            Destination::Zone(Zone::Graveyard { owner: P1 }),
            MoveKind::Put
        )
    );
    assert_eq!(
        (successor.card, successor.incarnation),
        (bears.card, bears.incarnation + 1)
    );
    assert_eq!(death.origin.root, Root::StateBased);
    assert!(death.facet(FacetKind::Entered).is_none());
    let died = SeatDelta {
        permanent_left: true,
        creature_died: true,
        ..row(P1)
    };
    assert_eq!(seat(&journal.ledger, P1), Some(&died));
    let [zombie] = created(&journal)[..] else {
        panic!("one Zombie")
    };
    assert!(matches!(
        zombie.facet(FacetKind::Entered),
        Some(Facet::Entered {
            controller: P0,
            tapped: true,
            token: true,
            ..
        })
    ));
    let zombie_row = SeatDelta {
        creatures_entered: 1,
        nonlands_entered: 1,
        tokens_created: 1,
        ..row(P0)
    };
    assert_eq!(seat(&journal.ledger, P0), Some(&zombie_row));
    checked(&g);
    assert_recoverable((&g).into());

    let (quiet, silent) = parity(|mode| bolt_the_bears(mode, 1));
    let silent_death = silent
        .actuals
        .iter()
        .find(|a| a.departure().is_some())
        .unwrap();
    assert_eq!(silent_death.facets, death.facets);
    assert_eq!(silent.ledger, LedgerDelta { seats: vec![died] });
    assert!(created(&silent).is_empty());
    assert_recoverable((&quiet).into());
}

fn prepared_game(observer: u8) -> Game {
    Game::new(prepared(&[OVERSEER_OF_THE_DAMNED], |b| {
        b.permanent_or_hand("Overseer of the Damned", observer, observer, false)
            .permanent_or_hand("Lightning Bolt", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
    }))
}

/// CR 733.1: Rapacious Dragon's two Treasures leave this turn's ledger with
/// three entries and two creations. A Treasure sacrificed for mana toward a
/// Murder that cannot be paid publishes its departure, which Korvold sees.
/// Reversal applies the receipt's exact inverse ledger change, removes
/// Korvold's trigger by the cost's event identity and synthesizes nothing:
/// the ledger, triggers and Treasure are exactly as before the payment.
fn reversed_treasure(mode: TraceMode) -> (Game, ActualJournal) {
    let p = prepared(&[RAPACIOUS_DRAGON, KORVOLD], |mut b| {
        b = b
            .permanent_or_hand("Rapacious Dragon", 0, 0, true)
            .permanent_or_hand("Murder", 0, 0, true)
            .permanent_or_hand("Korvold, Fae-Cursed King", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false);
        for _ in 0..5 {
            b = b.permanent_or_hand("Mountain", 0, 0, false);
        }
        b
    });
    let mut g = Game::with_trace(p, mode);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[4, 5, 6, 7, 8]);
    settle(&mut g);
    let prior = (
        g.state.rules.cast_history.clone(),
        format!("{:?}", g.state.rules.pending_triggers),
    );
    let row0 = &prior.0[0];
    assert_eq!(
        (
            row0.tokens_created,
            row0.artifacts_entered.clone(),
            row0.permanent_left
        ),
        (2, 2.into(), false)
    );
    let korvold = Driver::object(&g, 2);
    let theirs = Driver::object(&g, 3);
    let murder = Driver::object(&g, 1);
    let treasure = g
        .state
        .rules
        .objects
        .values()
        .find(|o| g.state.rules.tokens.contains_key(&o.id.card))
        .unwrap()
        .id;
    Driver::cast(&mut g, murder.card);
    Driver::pick_object(&mut g, theirs);
    let d = Driver::request(&mut g, u64::MAX);
    let slot = d
        .index(&Candidate::Action {
            action: Action::ActivateMana { object: treasure },
        })
        .unwrap();
    open_journal(&mut g);
    g.respond(
        d.player,
        d.id,
        Answer::Pay {
            pay: PayAnswer::Activate { source: slot },
        },
    )
    .unwrap();
    let d = Driver::request(&mut g, u64::MAX);
    let black = d.pick_one(&Candidate::Color { color: Color::B }).unwrap();
    g.respond(d.player, d.id, black).unwrap();
    Driver::request(&mut g, u64::MAX);
    let journal = take_journal(&mut g);
    {
        let (Frame::Mana { cast } | Frame::Payment { cast }) = &g.state.rules.frame else {
            panic!("expected the payment window")
        };
        let receipt = cast
            .mana_activations
            .iter()
            .find_map(|r| r.permanent.as_ref())
            .unwrap();
        assert!(receipt.sacrificed && !receipt.kept);
        // The journal also saw the mana being added, which is no cost.
        assert!(!receipt.cost_events.is_empty());
        assert!(
            receipt
                .cost_events
                .iter()
                .all(|e| journal.events.contains(e))
        );
        assert_eq!(receipt.ledger, journal.ledger);
        assert_eq!(
            receipt.ledger,
            LedgerDelta {
                seats: vec![SeatDelta {
                    permanent_left: true,
                    ..row(P0)
                }]
            }
        );
        let departure = journal
            .actuals
            .iter()
            .find(|a| a.departure().is_some())
            .unwrap();
        assert_eq!(departure.origin.root, Root::Cost(CostOf::ManaAbility));
        assert!(receipt.cost_events.contains(&departure.id));
        // Korvold's sacrifice trigger waits on the departure.
        assert_eq!(triggers_from(&g, korvold), 1);
        assert!(g.state.rules.cast_history[0].permanent_left);
        checked(&g);
    }
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    assert_eq!(Driver::request(&mut g, u64::MAX).reason, Reason::Priority);
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(
        (
            g.state.rules.cast_history.clone(),
            format!("{:?}", g.state.rules.pending_triggers)
        ),
        prior
    );
    assert_eq!(triggers_from(&g, korvold), 0);
    assert_eq!(g.current(treasure).map(|o| o.zone), Ok(Zone::Battlefield));
    assert_eq!(g.current(korvold).unwrap().plus_one_counters, 0.into());
    (g, journal)
}

#[test]
fn reversed_mana_sacrifice_restores_the_prior_ledger_and_synthesizes_no_trigger() {
    let (g, _) = parity(reversed_treasure);
    checked(&g);
    assert_recoverable((&g).into());
}

/// CR 111.13, 608.3: a resolving copy of a permanent spell becomes a token
/// without being created. On the same resolution, the writer publishes a
/// move and a token entry with no creation facet when the resolving object
/// is a spell copy; the reducer leaves created-token history unchanged
/// while counting the entry, and Impact Tremors sees the token enter. No
/// runtime instruction copies a permanent spell yet, so the copy is forged
/// on the published state.
#[test]
fn resolving_permanent_spell_copy_enters_as_a_token_without_creation() {
    let p = prepared(&[IMPACT_TREMORS], |b| {
        b.permanent_or_hand("Impact Tremors", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
    });
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[2, 3]);
    Driver::request(&mut g, u64::MAX);
    let spell = g.state.rules.stack.last().unwrap().clone();
    let before = g.fork();
    open_journal(&mut g);
    resolve(&mut g, spell.id);
    let journal = take_journal(&mut g);
    let batch = g
        .trace
        .records
        .iter()
        .find_map(|record| match &record.record {
            EvidenceRecord::Batch { batch }
                if matches!(batch.cause, BatchCause::SpellCompletion { stack_id, .. }
                    if stack_id == spell.id) =>
            {
                Some((**batch).clone())
            }
            _ => None,
        })
        .unwrap();
    let real = g
        .canonical_actuals(&before, &batch.cause, &batch.occurrences)
        .unwrap();
    // The published actual is the writer's: an ordinary card enters.
    assert_eq!(
        real.0,
        journal
            .actuals
            .iter()
            .filter(|a| a.id.occurrence.batch == batch.batch_id)
            .cloned()
            .collect::<Vec<_>>()
    );
    let [entry] = &real.0[..] else {
        panic!("one resolution member")
    };
    assert_eq!(
        entry.facet(FacetKind::Moved).map(|f| match f {
            Facet::Moved { how, .. } => *how,
            _ => unreachable!(),
        }),
        Some(MoveKind::Resolved { stack_id: spell.id })
    );
    assert!(matches!(
        entry.facet(FacetKind::Entered),
        Some(Facet::Entered { token: false, .. })
    ));

    let mut copy = g.fork();
    let bears = copy
        .prepared
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap()
        .clone();
    std::sync::Arc::make_mut(&mut copy.state.rules.tokens).insert(
        spell.source.card,
        tokens::TokenIdentity {
            source: spell.source,
            spec: tokens::TokenTemplate::SpellCopy {
                original: spell.source,
                stack_id: spell.id,
            },
            definition: bears,
        },
    );
    let (actuals, delta) = copy
        .canonical_actuals(&before, &batch.cause, &batch.occurrences)
        .unwrap();
    let [entry] = &actuals[..] else {
        panic!("one resolution member")
    };
    let kinds = entry.facets.iter().map(Facet::kind).collect::<Vec<_>>();
    assert_eq!(kinds, [FacetKind::Moved, FacetKind::Entered]);
    assert!(matches!(
        entry.facet(FacetKind::Entered),
        Some(Facet::Entered {
            token: true,
            controller: P0,
            ..
        })
    ));
    assert_eq!(
        delta,
        LedgerDelta {
            seats: vec![SeatDelta {
                creatures_entered: 1,
                nonlands_entered: 1,
                ..row(P0)
            }]
        }
    );
    assert_eq!(delta, real.1);
    // "Whenever a creature you control enters" sees the token enter.
    let tremors = Driver::object(&copy, 0);
    copy.state.rules.pending_triggers.clear();
    copy.collect_triggers(&batch.cause, &before, &batch.occurrences)
        .unwrap();
    let [trigger] = &copy.state.rules.pending_triggers[..] else {
        panic!("one entry trigger")
    };
    assert_eq!(trigger.source, tremors);
    assert!(
        matches!(&trigger.witness, TriggerEventWitness::Entered { movement: Some(movement), .. }
        if movement.destination == Zone::Battlefield && movement.successor.card == spell.source.card)
    );
    checked(&g);
    assert_recoverable((&g).into());
}
