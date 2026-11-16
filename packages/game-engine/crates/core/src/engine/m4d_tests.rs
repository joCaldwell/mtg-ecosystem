//! Private source-backed paid roots; no pod/consumer admission.
use super::m4a_tests::replace;
use super::m4c_tests::{grants, public_proof, settle};
use super::*;
const NEXUS: &str =
    include_str!("../../../../tests/fixtures/ramos/b1c346a4-fc9b-474a-9d51-cac168e363fa.json");
const SORCERER: &str =
    include_str!("../../../../tests/fixtures/ramos/a450ef56-8511-43e0-bdca-0c25f54ba338.json");
pub(super) fn root(original: &str, library: &[&str], board: &[&str]) -> Prepared {
    let mut p = m4c_tests::root("Bloodbraid Elf", &vec!["Forest"; library.len()], false);
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.extend([lowered(NEXUS), lowered(SORCERER)]);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let original_id = c
        .definitions
        .iter()
        .find(|d| d.name == original)
        .unwrap()
        .oracle_id
        .clone();
    p.config.cards[1].oracle_id = original_id;
    for (setup, name) in p
        .config
        .cards
        .iter_mut()
        .filter(|s| s.owner == PlayerId(0) && matches!(s.zone, Zone::Library { .. }))
        .zip(library)
    {
        setup.oracle_id = c
            .definitions
            .iter()
            .find(|d| d.name == *name)
            .unwrap()
            .oracle_id
            .clone();
    }
    let forest = c
        .definitions
        .iter()
        .find(|d| d.name == "Forest")
        .unwrap()
        .oracle_id
        .clone();
    let hydra = c
        .definitions
        .iter()
        .find(|d| d.name == "Managorger Hydra")
        .unwrap()
        .oracle_id
        .clone();
    for setup in &mut p.config.cards {
        if setup.oracle_id == hydra {
            setup.oracle_id = forest.clone();
        }
    }
    for name in board {
        replace(&mut p, PlayerId(0), name, Zone::Battlefield);
    }
    replace(&mut p, PlayerId(0), "Swamp", Zone::Battlefield);
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p
}
pub(super) fn flags(g: &Game, seat: usize) -> (bool, bool) {
    let h = g.state.rules.cast_history[seat];
    (h.any_cast, h.exile_cast)
}
pub(super) fn witness(g: &Game, original: StackId) -> &triggers::CastOccurrence {
    g.state
        .rules
        .stack
        .iter()
        .find_map(|s| match &s.kind {
            StackKind::Triggered {
                witness: TriggerEventWitness::SpellCast { cast },
                ..
            } if cast.stack_id == original => Some(cast.as_ref()),
            _ => None,
        })
        .unwrap()
}
#[test]
fn m4d_nexus_self_cast_no_observer_history_and_paid_later_spell() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(root("Maelstrom Nexus", &["Forest"], &[]), mode);
        assert_eq!(flags(&g, 0), (false, false));
        let nexus = paid(&mut g, "Maelstrom Nexus");
        assert!(grants(&g, nexus).is_empty());
        assert_eq!(flags(&g, 0), (true, false));
        public_proof(&g);
        settle(&mut g);
        let ramos = paid(&mut g, "Ramos, Dragon Engine");
        assert!(grants(&g, ramos).is_empty());
        public_proof(&g);
        let mut lie = g.fork();
        lie.state.rules.cast_history[0] = triggers::TurnCastHistory::default();
        assert!(Game::restore(lie.prepared.as_ref().clone(), &lie.snapshot().unwrap()).is_err());
    }
}
#[test]
fn m4d_first_hand_then_genuine_free_exile_child_has_only_sorcerer_grant() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(
            root(
                "Bloodbraid Elf",
                &["Grizzly Bears"],
                &["Maelstrom Nexus", "Wild-Magic Sorcerer"],
            ),
            mode,
        );
        let original = paid(&mut g, "Bloodbraid Elf");
        assert_eq!(grants(&g, original).len(), 1);
        assert!(witness(&g, original).history.first_overall);
        assert!(!witness(&g, original).history.first_from_exile);
        public_proof(&g);
        offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        m3c_tests::finish_announcement(&mut g, false, None, None);
        let child = g
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.kind.is_spell() && s.cast_from == Zone::Exile)
            .unwrap()
            .id;
        assert_eq!(grants(&g, child).len(), 1);
        let cast = witness(&g, child);
        assert!(!cast.history.first_overall);
        assert!(cast.history.first_from_exile);
        assert_eq!(flags(&g, 0), (true, true));
        public_proof(&g);
        // Source provenance and firstness remain historical after later ledger use.
        let mut lie = g.fork();
        for s in &mut lie.state.rules.stack {
            if let StackKind::Triggered {
                witness: TriggerEventWitness::SpellCast { cast },
                ..
            } = &mut s.kind
            {
                if cast.stack_id == child {
                    cast.history.first_from_exile = false;
                }
            }
        }
        assert!(Game::restore(lie.prepared.as_ref().clone(), &lie.snapshot().unwrap()).is_err());
    }
}
#[test]
fn m4d_prior_exile_before_sorcerer_entry_and_source_self_exile_consume() {
    // A real free Sorcerer child consumes first-exile while still on the stack.
    // Equal MV would not hit: use complete Apex instead of altering either definition.
    let mut p = root(
        "Apex Devastator",
        &["Wild-Magic Sorcerer", "Grizzly Bears"],
        &[],
    );
    p.config.cards[1].oracle_id = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Apex Devastator")
        .unwrap()
        .oracle_id
        .clone();
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    paid(&mut g, "Apex Devastator");
    offer(&mut g);
    answer(&mut g, Response::OptionalEffect { perform: true });
    m3c_tests::finish_announcement(&mut g, false, None, None);
    let sorcerer = g
        .state
        .rules
        .stack
        .iter()
        .find(|s| s.kind.is_spell() && s.cast_from == Zone::Exile)
        .unwrap()
        .id;
    assert!(grants(&g, sorcerer).is_empty());
    assert_eq!(flags(&g, 0), (true, true));
    public_proof(&g);
    m4a_tests::resolve_entry(&mut g, sorcerer);
    offer(&mut g);
    answer(&mut g, Response::OptionalEffect { perform: true });
    m3c_tests::finish_announcement(&mut g, false, None, None);
    let later = g
        .state
        .rules
        .stack
        .iter()
        .find(|s| s.kind.is_spell() && s.cast_from == Zone::Exile)
        .unwrap()
        .id;
    assert!(grants(&g, later).is_empty());
    assert_eq!(flags(&g, 0), (true, true));
    public_proof(&g);
    settle(&mut g);
    let ramos = paid(&mut g, "Ramos, Dragon Engine");
    assert!(grants(&g, ramos).is_empty());
    public_proof(&g);
}
#[test]
fn m4d_no_observer_ledger_strict_codec_digest_and_replay_forgery() {
    let mut g = Game::with_trace(
        root("Ramos, Dragon Engine", &["Forest"], &[]),
        TraceMode::Off,
    );
    paid(&mut g, "Ramos, Dragon Engine");
    assert_eq!(flags(&g, 0), (true, false));
    public_proof(&g);
    let bytes = g.snapshot().unwrap();
    let value: serde_json::Value = strict_read(&bytes).unwrap();
    assert_eq!(value["snapshot_version"], 32);
    assert_eq!(
        state_digest::state_digest(&g.state).unwrap(),
        digest(&g.state).unwrap()
    );
    for mutation in 0..4 {
        let mut lie = g.fork();
        match mutation {
            0 => lie.state.rules.cast_history[0].any_cast = false,
            1 => lie.state.rules.cast_history[0].exile_cast = true,
            2 => lie.state.rules.cast_history[1].any_cast = true,
            3 => {
                lie.state.rules.cast_history[1].exile_cast = true;
            }
            _ => unreachable!(),
        }
        let forged = lie.snapshot().unwrap();
        let mut verifier = SnapshotVerifier::new(g.prepared.as_ref().clone(), TraceMode::Off);
        assert!(verifier.verify(&forged).is_err());
        assert_eq!(verifier.verify(&bytes).unwrap().state, g.state);
    }
    let mut wire = serde_json::to_value(&g.state).unwrap();
    wire.as_object_mut().unwrap().remove("cast_history");
    assert!(serde_json::from_value::<State>(wire).is_err());
}
#[test]
fn m4d_turn_advance_resets_all_original_seats_but_phase_and_departure_do_not() {
    let mut p = root("Ramos, Dragon Engine", &["Forest"], &[]);
    let forest = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Forest")
        .unwrap()
        .oracle_id
        .clone();
    for setup in &mut p.config.cards {
        if setup.owner == PlayerId(0)
            && setup.oracle_id == forest
            && matches!(setup.zone, Zone::Hand { .. })
        {
            setup.zone = Zone::Library { owner: PlayerId(0) };
        }
    }
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    paid(&mut g, "Ramos, Dragon Engine");
    settle(&mut g);
    let turn = g.state.rules.turn.number;
    g.concede(PlayerId(3)).unwrap();
    request(&mut g);
    assert_eq!(flags(&g, 0), (true, false));
    for _ in 0..160 {
        if g.state.rules.turn.number != turn {
            break;
        }
        if g.state.rules.turn.step != TurnStep::Cleanup {
            assert_eq!(flags(&g, 0), (true, false));
        }
        let r = request(&mut g);
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                candidates, count, ..
            } => Response::Discard {
                objects: candidates.into_iter().take(count).collect(),
            },
            other => panic!("turn request {other:?}"),
        };
        answer(&mut g, response);
        request(&mut g);
    }
    assert!(g.state.rules.turn.number > turn);
    assert!(
        g.state
            .rules
            .cast_history
            .iter()
            .all(|h| !h.any_cast && !h.exile_cast)
    );
    public_proof(&g);
}
#[test]
fn m4d_paid_completion_faults_roll_back_history_payment_siblings_and_trace() {
    for fault in 0..3 {
        let mut g = Game::new(root(
            "Apex Devastator",
            &["Forest"],
            &["Maelstrom Nexus", "Wild-Magic Sorcerer"],
        ));
        m4a_tests::before_payment(&mut g);
        assert_eq!(flags(&g, 0), (false, false));
        match fault {
            0 => g.state.rules.next_trigger_id = u64::MAX,
            1 => g.state.rules.next_stack_id = u32::MAX,
            2 => m4c_tests::set_post_paid_history_fault(&mut g),
            _ => unreachable!(),
        }
        let state = g.state.clone();
        let trace = g.trace.clone();
        assert!(g.history_charge() <= capacities::HISTORY_LIMIT);
        assert!(matches!(g.advance(Budget::new(1)), Advance::Fault(_)));
        assert_eq!(flags(&g, 0), (false, false));
        // The public step keeps only its typed fault/frame, never payment or collector state.
        assert_eq!(g.state.rules.stack, state.rules.stack);
        assert_eq!(g.state.rules.players, state.rules.players);
        assert_eq!(g.state.rules.next_trigger_id, state.rules.next_trigger_id);
        assert_eq!(g.state.rules.next_stack_id, state.rules.next_stack_id);
        assert_eq!(g.state.rules.pending_triggers, state.rules.pending_triggers);
        assert_eq!(g.trace, trace);
    }
}
#[test]
fn m4d_direct_collector_failure_is_atomic_with_previously_empty_history() {
    let mut g = Game::new(root("Apex Devastator", &["Forest"], &["Maelstrom Nexus"]));
    m4a_tests::before_payment(&mut g);
    // Synthetic paid marker isolates the direct collector transaction.
    let cost = g.top().unwrap().total_cost.clone();
    g.state.rules.stack.last_mut().unwrap().paid = Some(cost);
    let original = g.top().unwrap().id;
    g.state.rules.retained_auxiliary_charge = capacities::HISTORY_LIMIT - 1;
    let entry = g
        .state
        .rules
        .stack
        .iter()
        .find(|s| s.id == original)
        .unwrap()
        .clone();
    let before = g.state.clone();
    let trace = g.trace.clone();
    assert!(g.collect_cast_triggers(&entry).is_err());
    assert!(matches!(
        g.typed_fault(),
        Some(RuleFault::HistoryCapacityExceeded { .. })
    ));
    g.state.rules.typed_fault = before.rules.typed_fault.clone();
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}
