//! Bounded complete Apex source witnesses. Prepared midgame roots remain
//! distinct from real trusted-RNG setup and from source-admitted pods.
use super::*;
const APEX: &str =
    include_str!("../../../../tests/fixtures/ramos/b4d6747c-3516-4e71-be30-af007c6b1dc4.json");
const HYDRA: &str = include_str!(
    "../../../../tests/fixtures/cards-data-d2cc/b3f2265b-dd65-4b74-8b74-35ee0b147617.json"
);
const BLOW: &str = include_str!("../../../../tests/fixtures/dismantling-blow.json");
#[test]
fn m4a_producer_identity() {
    eprintln!(
        "M4A_PRODUCER {}",
        serde_json::json!({"build_id":crate::build_id(),"opt_level":TEST_OPT_LEVEL,"debug_assertions":cfg!(debug_assertions)})
    );
    assert_eq!(TEST_OPT_LEVEL, "2");
    assert!(cfg!(debug_assertions));
}
pub(super) fn replace(p: &mut Prepared, owner: PlayerId, name: &str, zone: Zone) -> CardId {
    let id = |name: &str| {
        p.catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let oracle = id(name);
    let forest = id("Forest");
    let c = p
        .config
        .cards
        .iter_mut()
        .filter(|c| {
            c.owner == owner
                && c.oracle_id == forest
                && matches!(c.zone, Zone::Hand { .. } | Zone::Library { .. })
        })
        .min_by_key(|c| usize::from(matches!(c.zone, Zone::Library { .. })))
        .unwrap();
    c.oracle_id = oracle;
    c.zone = zone;
    c.id
}
pub(super) fn source_prepared(library: &[&str], observer: bool) -> Prepared {
    let mut p = prepared_with_records("Apex Devastator", library, &[APEX, HYDRA, BLOW]);
    for name in [
        "Forest", "Forest", "Forest", "Forest", "Forest", "Forest", "Forest", "Forest", "Forest",
        "Forest", "Forest", "Forest", "Plains", "Plains", "Plains", "Island", "Island", "Island",
    ] {
        replace(&mut p, PlayerId(0), name, Zone::Battlefield);
    }
    if observer {
        replace(&mut p, PlayerId(1), "Managorger Hydra", Zone::Battlefield);
    }
    for name in ["Island", "Island", "Island", "Island"] {
        replace(&mut p, PlayerId(1), name, Zone::Battlefield);
    }
    for name in ["Counterspell", "Stifle"] {
        replace(&mut p, PlayerId(1), name, Zone::Hand { owner: PlayerId(1) });
    }
    p.profile_digest = digest(&p.config).unwrap();
    p
}
pub(super) fn resolve_entry(g: &mut Game, id: StackId) {
    for _ in 0..100 {
        if !g.state.rules.stack.iter().any(|s| s.id == id) {
            return;
        }
        answer(g, Response::Pass);
        request(g);
    }
    panic!("entry did not finish");
}
fn with_ramos(library: &[&str], observer: bool, mode: TraceMode) -> Game {
    let mut g = Game::with_trace(source_prepared(library, observer), mode);
    let ramos = paid(&mut g, "Ramos, Dragon Engine");
    resolve_entry(&mut g, ramos);
    g
}
fn sibling_ids(g: &Game, original: StackId) -> Vec<StackId> {
    g.state
        .rules
        .stack
        .iter()
        .filter_map(|s| match &s.kind {
            StackKind::Triggered {
                witness: TriggerEventWitness::SpellCast { cast },
                ..
            } if cast.stack_id == original
                && g.definition(s.source.card).unwrap().name == "Apex Devastator" =>
            {
                Some(s.id)
            }
            _ => None,
        })
        .collect()
}
fn assert_bad_restore(g: &Game) {
    assert!(Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).is_err());
}
// Leave the real completed payment response accepted but unexecuted. Synthetic
// ID/capacity injections below then exercise the actual atomic rules root step.
pub(super) fn before_payment(g: &mut Game) {
    let card = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            g.definition(o.id.card).unwrap().name == "Apex Devastator"
                && matches!(o.zone, Zone::Hand { .. })
        })
        .unwrap()
        .id
        .card;
    answer(g, Response::Cast { card });
    loop {
        let r = request(g);
        let response = match &r {
            Request::Mana { .. } => m3c_tests::fund_response(g, &r),
            Request::SpendMana {
                cost, available, ..
            } => Response::SpendMana {
                spending: m3c_tests::spending(cost, available),
            },
            other => panic!("payment {other:?}"),
        };
        let finish = matches!(response, Response::SpendMana { .. });
        g.respond(r.player(), r.decision_id(), response).unwrap();
        if finish {
            return;
        }
    }
}
#[test]
fn m4a_explicit_pending_order_placement_replay_and_distinct_visible_handles() {
    let mut g = with_ramos(&["Grizzly Bears"], true, TraceMode::Detailed);
    before_payment(&mut g);
    let mut pending_proved = false;
    loop {
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => {
                if g.state.rules.pending_triggers.len() == 6 {
                    pending_proved = true;
                    prove(&g);
                    for mutation in 0..3 {
                        let mut forged = g.fork();
                        let apex = forged
                            .state
                            .rules
                            .pending_triggers
                            .iter()
                            .enumerate()
                            .filter(|(_, t)| {
                                forged.definition(t.source.card).unwrap().name == "Apex Devastator"
                            })
                            .map(|(i, _)| i)
                            .collect::<Vec<_>>();
                        match mutation {
                            0 => {
                                forged.state.rules.pending_triggers.remove(apex[0]);
                            }
                            1 => {
                                forged.state.rules.pending_triggers[apex[0]].ability =
                                    EffectiveTriggerAbility::Intrinsic { index: 1 }
                            }
                            2 => forged.state.rules.pending_triggers.swap(apex[0], apex[1]),
                            _ => unreachable!(),
                        }
                        assert_bad_restore(&forged);
                    }
                }
            }
            Advance::AwaitingDecision(Request::OrderTriggers {
                candidates,
                player,
                decision_id,
                ..
            }) => {
                assert_eq!(player, PlayerId(0));
                assert_eq!(candidates.len(), 5);
                let order = candidates.into_iter().rev().collect::<Vec<_>>();
                g.respond(
                    player,
                    decision_id,
                    Response::OrderTriggers {
                        order: order.clone(),
                    },
                )
                .unwrap();
                for _ in 0..100 {
                    prove(&g);
                    if matches!(
                        g.advance(Budget::new(1)),
                        Advance::AwaitingDecision(Request::Priority { .. })
                    ) {
                        break;
                    }
                }
                let placed = g
                    .state
                    .rules
                    .stack
                    .iter()
                    .filter_map(|s| match &s.kind {
                        StackKind::Triggered { trigger_id, .. } if s.controller == PlayerId(0) => {
                            Some(*trigger_id)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(placed, order);
                for viewer in [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)] {
                    let QueryResult::Complete(QueryValue::Observation(o)) =
                        g.observe(viewer, Budget::unlimited())
                    else {
                        panic!()
                    };
                    let handles = o
                        .view
                        .stack
                        .iter()
                        .map(|s| serde_json::to_string(&s.handle).unwrap())
                        .collect::<BTreeSet<_>>();
                    assert_eq!(handles.len(), 7);
                }
                break;
            }
            other => panic!("placement {other:?}"),
        }
    }
    assert!(pending_proved);
    prove(&g);
}
#[test]
fn m4a_complete_batch_capacity_and_id_arithmetic_precede_publication() {
    let mut g = Game::new(source_prepared(&[], false));
    before_payment(&mut g);
    let original = g.top().unwrap().clone();
    assert!(original.paid.is_none());
    // Common bound, independent of the fixture's printed four instances.
    let retained = g.state.rules.stack.len() + g.state.rules.pending_triggers.len();
    g.preflight_trigger_emission(capacities::STACK_LIMIT - retained)
        .unwrap();
    assert!(
        g.preflight_trigger_emission(capacities::STACK_LIMIT - retained + 1)
            .is_err()
    );
    assert!(
        matches!(g.typed_fault(), Some(RuleFault::StackCapacityExceeded { requested, .. }) if *requested == capacities::STACK_LIMIT + 1)
    );
    g.state.rules.typed_fault = None;
    for (trigger, stack) in [(u64::MAX - 3, 10), (10, u32::MAX - 3)] {
        let mut injected = g.fork();
        injected.state.rules.next_trigger_id = trigger;
        injected.state.rules.next_stack_id = stack;
        let before = injected.state.clone();
        let trace = injected.trace.clone();
        assert!(matches!(
            injected.advance(Budget::new(1)),
            Advance::Fault(_)
        ));
        let mut after = injected.state.clone();
        after.rules.frame = before.rules.frame.clone();
        after.rules.typed_fault = before.rules.typed_fault.clone();
        assert_eq!(after, before);
        assert_eq!(injected.trace, trace);
        assert!(injected.state.rules.pending_triggers.is_empty());
    }
    // Exact checked arithmetic fit and one-over are synthetic structural probes.
    g.state.rules.next_trigger_id = u64::MAX - 4;
    g.state.rules.next_stack_id = u32::MAX - 4;
    g.preflight_trigger_emission(4).unwrap();
    assert!(g.preflight_trigger_emission(5).is_err());
}
#[test]
fn m4a_direct_collector_checks_whole_batch_before_any_id_or_trigger_publication() {
    let mut g = Game::new(source_prepared(&[], false));
    let original = paid(&mut g, "Apex Devastator");
    // Synthetic internal boundary: replay the collector against the same paid
    // source after removing its already collected entries. This is not a second
    // cast or a recovery claim. It distinguishes batch preflight from root-only
    // rollback and from a successful insertion followed by sibling overflow.
    g.state.rules.stack.retain(|s| s.id == original);
    let entry = g.top().unwrap().clone();
    for (trigger, stack) in [(u64::MAX - 3, 10), (10, u32::MAX - 3)] {
        let mut injected = g.fork();
        injected.state.rules.next_trigger_id = trigger;
        injected.state.rules.next_stack_id = stack;
        let before = injected.state.clone();
        let trace = injected.trace.clone();
        assert!(injected.collect_cast_triggers(&entry).is_err());
        assert!(injected.typed_fault().is_some());
        injected.state.rules.typed_fault = before.rules.typed_fault.clone();
        assert_eq!(injected.state, before);
        assert_eq!(injected.trace, trace);
    }
}
#[test]
fn m4a_generic_validated_ir_vector_emits_every_instance_without_printed_ceiling() {
    let mut p = source_prepared(&[], false);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Apex Devastator")
        .unwrap();
    // Explicit synthetic IR shape control, not a complete archived-source
    // positive. Generic executor capacity comes from actual validated vectors.
    let count = 17;
    d.triggered = vec![d.triggered[0].clone(); count];
    c.validate().unwrap();
    let max_vector = c
        .definitions
        .iter()
        .map(|d| d.triggered.len())
        .max()
        .unwrap();
    let bound = (4 * capacities::OWNED_LIMIT)
        .checked_mul(max_vector)
        .unwrap();
    p.catalog_digest = digest(c).unwrap();
    let mut g = Game::new(p);
    let original = paid(&mut g, "Apex Devastator");
    assert_eq!(sibling_ids(&g, original).len(), count);
    assert!(count <= bound && bound < capacities::STACK_LIMIT);
    let abilities = g
        .state
        .rules
        .stack
        .iter()
        .filter_map(|s| match s.kind {
            StackKind::Triggered { ability, .. } => Some(ability),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        abilities,
        (0..count as u32)
            .map(|index| EffectiveTriggerAbility::Intrinsic { index })
            .collect()
    );
}
#[test]
fn m4a_synthetic_shared_stack_overflow_rolls_back_whole_paid_cast_step() {
    let mut seed = Game::new(source_prepared(&[], false));
    before_payment(&mut seed);
    seed.advance(Budget::new(1));
    assert_eq!(seed.state.rules.pending_triggers.len(), 4);
    let template = seed.state.rules.pending_triggers[0].clone();
    let mut g = Game::new(source_prepared(&[], false));
    before_payment(&mut g);
    // Injection is deliberately not a reachable paid game or restore witness.
    // The final four-instance emission exceeds the common ceiling by one.
    g.state
        .rules
        .pending_triggers
        .resize(capacities::STACK_LIMIT - 4, template);
    let before = g.state.clone();
    let trace = g.trace.clone();
    assert!(matches!(g.advance(Budget::new(1)), Advance::Fault(_)));
    assert!(
        matches!(g.typed_fault(), Some(RuleFault::StackCapacityExceeded { requested, .. }) if *requested == capacities::STACK_LIMIT + 1)
    );
    let mut after = g.state.clone();
    after.rules.frame = before.rules.frame.clone();
    after.rules.typed_fault = before.rules.typed_fault.clone();
    assert_eq!(after, before);
    assert_eq!(g.trace, trace);
}
#[test]
fn m4a_each_sibling_uses_m3_optional_cost_selection_and_owned_child_payment() {
    for kick in [false, true] {
        let mut g = with_ramos(&["Dismantling Blow"], false, TraceMode::Detailed);
        let original = paid(&mut g, "Apex Devastator");
        offer(&mut g);
        let owner = g.top().unwrap().id;
        answer(&mut g, Response::OptionalEffect { perform: true });
        assert!(matches!(request(&mut g), Request::AdditionalCosts { .. }));
        prove(&g);
        let foreign_owner = sibling_ids(&g, original)[0];
        assert_ne!(foreign_owner, owner);
        let mut forged = g.fork();
        let mut frame = serde_json::to_value(&forged.state.rules.frame).unwrap();
        frame["cast"]["completion"]["permission"]["owner"] = serde_json::json!(foreign_owner);
        forged.state.rules.frame = serde_json::from_value(frame).unwrap();
        assert_bad_restore(&forged);
        assert!(g.state.rules.stack.iter().any(|s| s.id == owner));
        assert_eq!(sibling_ids(&g, original).len(), 4);
        m3c_tests::finish_announcement(&mut g, kick, None, None);
        assert!(!g.state.rules.stack.iter().any(|s| s.id == owner));
        assert_eq!(sibling_ids(&g, original).len(), 3);
        let child = g
            .state
            .rules
            .stack
            .iter()
            .rev()
            .find(|s| s.kind.is_spell())
            .unwrap();
        assert_eq!(
            g.definition(child.source.card).unwrap().name,
            "Dismantling Blow"
        );
        assert_eq!(
            child.selected_additional_costs.as_ref().map(Vec::len),
            Some(usize::from(kick))
        );
        assert_eq!(
            child.total_cost.generic,
            if kick { 2.into() } else { 0.into() }
        );
        prove(&g);
    }
}
#[test]
fn m4a_equal_mana_value_is_skipped_and_each_nohit_sibling_finishes_independently() {
    let mut g = Game::new(source_prepared(
        &["Apex Devastator", "Grizzly Bears"],
        false,
    ));
    let original = paid(&mut g, "Apex Devastator");
    offer(&mut g);
    let Frame::Cascade { progress } = &g.state.rules.frame else {
        panic!()
    };
    let value = serde_json::to_value(progress).unwrap();
    assert_eq!(value["exiled"].as_array().unwrap().len(), 2);
    answer(&mut g, Response::OptionalEffect { perform: true });
    request(&mut g);
    let child = g.top().unwrap().id;
    assert_eq!(
        g.definition(g.top().unwrap().source.card).unwrap().name,
        "Grizzly Bears"
    );
    resolve_entry(&mut g, child);
    for _ in 0..100 {
        if sibling_ids(&g, original).is_empty() {
            break;
        }
        let r = request(&mut g);
        assert!(
            matches!(r, Request::Priority { .. }),
            "equal MV must not offer: {r:?}"
        );
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    request(&mut g);
    assert!(sibling_ids(&g, original).is_empty());
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 1);
    assert!(!g.state.rules.players[0].failed_draw);
    prove(&g);
}
fn private_bottom(g: &Game) -> bool {
    let Frame::Cascade { progress } = &g.state.rules.frame else {
        return false;
    };
    if progress.stage != cascade::CascadeStage::Bottom {
        return false;
    }
    let mut forged = g.fork();
    let mut frame = serde_json::to_value(&forged.state.rules.frame).unwrap();
    let permutation = frame["progress"]["permutation"].as_array_mut().unwrap();
    if permutation.len() <= 1 {
        return false;
    }
    permutation.swap(0, 1);
    forged.state.rules.frame = serde_json::from_value(frame).unwrap();
    assert_bad_restore(&forged);
    // Future library order stays private to all four viewers, including its
    // owner. This negative changes no setup RNG, turn or paid source witness.
    let mut future = g.fork();
    future
        .state
        .rules
        .libraries
        .get_mut(&PlayerId(1))
        .unwrap()
        .swap(0, 1);
    assert_bad_restore(&future);
    for viewer in [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)] {
        let before = g.state.clone();
        assert_eq!(observe_counted(g, viewer), observe_counted(&forged, viewer));
        assert_eq!(observe_counted(g, viewer), observe_counted(&future, viewer));
        assert_eq!(g.state, before);
    }
    true
}
fn real_setup(seed: u8) -> Prepared {
    let mut p = source_prepared(&["Grizzly Bears"], false);
    let basics = ["Forest", "Island", "Mountain"].map(|name| {
        p.catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    });
    for c in &mut p.config.cards {
        if matches!(c.zone, Zone::Command { .. }) {
            continue;
        }
        c.zone = Zone::Library { owner: c.owner };
        if c.owner == PlayerId(0) && c.id.0 >= 3 {
            c.oracle_id = basics[(c.id.0 % 3) as usize].clone();
        }
        if c.owner != PlayerId(0) {
            c.oracle_id = basics[0].clone();
        }
    }
    p.config.initial.turn_step = TurnStep::Untap;
    let mut config =
        MatchConfig::for_profile("commander-staples-v1", [seed; 32], PlayerId(0)).unwrap();
    config.decks = std::array::from_fn(|seat| {
        let owner = PlayerId(seat as u8);
        let mut cards = BTreeMap::<String, u32>::new();
        for c in p.config.cards.iter().filter(|c| c.owner == owner) {
            *cards.entry(c.oracle_id.clone()).or_default() += 1;
        }
        Decklist {
            commander: p
                .config
                .cards
                .iter()
                .find(|c| c.owner == owner && matches!(c.zone, Zone::Command { .. }))
                .unwrap()
                .oracle_id
                .clone(),
            cards: cards
                .into_iter()
                .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
                .collect(),
        }
    });
    p.match_config = Some(config);
    p.profile_digest = digest(&(&p.config, &p.match_config)).unwrap();
    p
}
#[test]
fn m4a_real_setup_four_private_bottom_groups_three_traces_and_budget_replay() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut found = None;
        for seed in 0..128 {
            let mut g = Game::with_trace(real_setup(seed), mode);
            for _ in 0..4 {
                answer(&mut g, Response::Mulligan { take: false });
            }
            request(&mut g);
            if g.state.rules.objects.values().any(|o| {
                o.zone == (Zone::Hand { owner: PlayerId(0) })
                    && g.definition(o.id.card).unwrap().name == "Apex Devastator"
            }) {
                found = Some(g);
                break;
            }
        }
        let mut g = found.expect("deterministic opening Apex witness");
        let mut developed = false;
        for _ in 0..4000 {
            let r = request(&mut g);
            let response = match &r {
                Request::Priority {
                    player: PlayerId(0),
                    ..
                } if g.active_player() == Some(PlayerId(0))
                    && matches!(
                        g.turn().step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    ) =>
                {
                    let lands = g
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(0))
                        .count();
                    if lands >= 10 {
                        developed = true;
                        break;
                    }
                    if g.turn().lands_played == 0 {
                        let card = g
                            .state
                            .rules
                            .objects
                            .values()
                            .find(|o| {
                                o.zone == (Zone::Hand { owner: PlayerId(0) })
                                    && g.definition(o.id.card)
                                        .unwrap()
                                        .characteristics
                                        .has_type(CardKind::Land)
                            })
                            .unwrap()
                            .id
                            .card;
                        Response::PlayLand { card }
                    } else {
                        Response::Pass
                    }
                }
                Request::Priority { .. } => Response::Pass,
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                Request::Discard {
                    count, candidates, ..
                } => Response::Discard {
                    objects: candidates
                        .iter()
                        .filter(|o| g.definition(o.card).unwrap().name != "Apex Devastator")
                        .take(*count)
                        .copied()
                        .collect(),
                },
                other => panic!("real development {other:?}"),
            };
            g.respond(r.player(), r.decision_id(), response).unwrap();
        }
        assert!(developed);
        let original = paid(&mut g, "Apex Devastator");
        let sibling_count = sibling_ids(&g, original).len();
        assert_eq!(sibling_count, 4);
        let shuffles = g
            .state
            .rules
            .match_state
            .as_ref()
            .unwrap()
            .library_shuffles
            .clone();
        let mut verifier = SnapshotVerifier::new(g.prepared.as_ref().clone(), mode);
        let mut groups = BTreeSet::new();
        let mut offered = 0;
        let mut privacy_checked = false;
        for _ in 0..2500 {
            if sibling_ids(&g, original).is_empty() {
                break;
            }
            if let Frame::Cascade { progress } = &g.state.rules.frame {
                if progress.stage == cascade::CascadeStage::Randomize
                    && groups.insert(progress.frame.stack_id)
                {
                    verifier.verify(&g.snapshot().unwrap()).unwrap();
                    let mut generous = g.fork();
                    let expected = generous.advance(Budget::unlimited());
                    let actual = loop {
                        match g.advance(Budget::new(1)) {
                            Advance::Suspended { .. } => {
                                verifier.verify(&g.snapshot().unwrap()).unwrap();
                                privacy_checked |= private_bottom(&g);
                            }
                            other => break other,
                        }
                    };
                    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                    assert_eq!(g.state, generous.state);
                    assert_eq!(g.trace, generous.trace);
                    continue;
                }
                if progress.stage == cascade::CascadeStage::Bottom {
                    let mut forged = g.fork();
                    let mut frame = serde_json::to_value(&forged.state.rules.frame).unwrap();
                    let permutation = frame["progress"]["permutation"].as_array_mut().unwrap();
                    if permutation.len() > 1 {
                        permutation.swap(0, 1);
                        forged.state.rules.frame = serde_json::from_value(frame).unwrap();
                        assert_bad_restore(&forged);
                        for viewer in [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)] {
                            assert_eq!(
                                observe_counted(&g, viewer),
                                observe_counted(&forged, viewer)
                            );
                        }
                    }
                }
            }
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(r) => {
                    verifier.verify(&g.snapshot().unwrap()).unwrap();
                    let response = match r {
                        Request::Priority { .. } => Response::Pass,
                        Request::OptionalEffect { .. } => {
                            offered += 1;
                            Response::OptionalEffect { perform: false }
                        }
                        other => panic!("real siblings {other:?}"),
                    };
                    g.respond(r.player(), r.decision_id(), response).unwrap();
                }
                other => panic!("real cascade {other:?}"),
            }
        }
        assert_eq!(groups.len(), 4);
        assert!(privacy_checked);
        assert_eq!(
            offered,
            if g.state
                .rules
                .objects
                .values()
                .any(|o| g.definition(o.id.card).unwrap().name == "Grizzly Bears"
                    && o.zone == (Zone::Library { owner: PlayerId(0) }))
            {
                4
            } else {
                0
            }
        );
        assert_eq!(
            g.state.rules.match_state.as_ref().unwrap().library_shuffles,
            shuffles
        );
        prove(&g);
    }
}
#[test]
fn m4a_paid_apex_four_same_block_siblings_one_ramos_and_apnap_observer() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = with_ramos(&["Grizzly Bears"], true, mode);
        let original = paid(&mut g, "Apex Devastator");
        let siblings = sibling_ids(&g, original);
        assert_eq!(siblings.len(), 4);
        let mut abilities = BTreeSet::new();
        let mut triggers = BTreeSet::new();
        let mut witnesses = vec![];
        let mut ramos = 0;
        let mut hydra = 0;
        for s in &g.state.rules.stack {
            if let StackKind::Triggered {
                ability,
                trigger_id,
                event,
                witness: TriggerEventWitness::SpellCast { cast },
                ..
            } = &s.kind
            {
                assert_eq!(cast.stack_id, original);
                assert_eq!(
                    event,
                    &TriggerOccurrenceId::Cast(CastOccurrenceId { cast: original })
                );
                witnesses.push(cast);
                match g.definition(s.source.card).unwrap().name.as_str() {
                    "Apex Devastator" => {
                        abilities.insert(*ability);
                        triggers.insert(*trigger_id);
                    }
                    "Ramos, Dragon Engine" => ramos += 1,
                    "Managorger Hydra" => hydra += 1,
                    other => panic!("unexpected {other}"),
                }
            }
        }
        assert_eq!(
            abilities,
            (0..4)
                .map(|index| EffectiveTriggerAbility::Intrinsic { index })
                .collect()
        );
        assert_eq!(triggers.len(), 4);
        assert_eq!((ramos, hydra), (1, 1));
        assert!(witnesses.windows(2).all(|w| w[0] == w[1]));
        assert_eq!(g.top().unwrap().controller, PlayerId(1)); // APNAP: opponent above active player.
        for viewer in [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)] {
            let before = g.state.clone();
            let QueryResult::Complete(QueryValue::Observation(o)) =
                g.observe(viewer, Budget::unlimited())
            else {
                panic!()
            };
            assert_eq!(o.view.stack.len(), 7);
            assert_eq!(
                serde_json::to_value(&o.view.stack)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .len(),
                7
            );
            assert_eq!(g.state, before);
        }
        if mode == TraceMode::Detailed {
            assert_eq!(
                g.events()
                    .iter()
                    .filter(
                        |e| matches!(e, Event::SpellCast { stack_id, .. } if *stack_id == original)
                    )
                    .count(),
                1
            );
        }
        prove(&g);
        let mut forged = g.fork();
        let first = siblings[0];
        let second = siblings[1];
        let a = forged
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.id == first)
            .unwrap()
            .kind
            .clone();
        forged
            .state
            .rules
            .stack
            .iter_mut()
            .find(|s| s.id == second)
            .unwrap()
            .kind = a;
        assert_bad_restore(&forged);
    }
}
#[test]
fn m4a_paid_counter_original_before_between_siblings_and_stifle_one_leave_others() {
    for (counter_original, finish_first) in [(true, false), (true, true), (false, false)] {
        let mut g = Game::new(source_prepared(&["Grizzly Bears"], false));
        let original = paid(&mut g, "Apex Devastator");
        let original_source = g.top().unwrap().source;
        let all = sibling_ids(&g, original);
        if finish_first {
            offer(&mut g);
            answer(&mut g, Response::OptionalEffect { perform: false });
            request(&mut g);
            assert_eq!(sibling_ids(&g, original).len(), 3);
        }
        let targeted = if counter_original {
            original
        } else {
            *all.last().unwrap()
        };
        answer(&mut g, Response::Pass);
        assert_eq!(request(&mut g).player(), PlayerId(1));
        let counter = paid_by(
            &mut g,
            if counter_original {
                "Counterspell"
            } else {
                "Stifle"
            },
            PlayerId(1),
            Some(targeted),
        );
        resolve_entry(&mut g, counter);
        prove(&g);
        assert!(!g.state.rules.stack.iter().any(|s| s.id == targeted));
        let remaining = sibling_ids(&g, original);
        assert_eq!(
            remaining.len(),
            if finish_first || !counter_original {
                3
            } else {
                4
            }
        );
        let mut owners = BTreeSet::new();
        for _ in 0..remaining.len() {
            offer(&mut g);
            let Frame::Cascade { progress } = &g.state.rules.frame else {
                panic!()
            };
            let value = serde_json::to_value(progress).unwrap();
            assert_eq!(value["threshold"], "10");
            assert_eq!(progress.frame.spell.source, original_source);
            assert!(owners.insert(g.top().unwrap().id));
            prove(&g);
            answer(&mut g, Response::OptionalEffect { perform: false });
            request(&mut g);
        }
        assert_eq!(owners.len(), remaining.len());
        assert_eq!(g.state.rules.stack.len(), usize::from(!counter_original));
        prove(&g);
    }
}
#[test]
fn m4a_sibling_offer_tokens_and_rehashed_multiplicity_witnesses_do_not_cross() {
    let mut g = Game::new(source_prepared(&["Grizzly Bears"], false));
    let original = paid(&mut g, "Apex Devastator");
    let siblings = sibling_ids(&g, original);
    for mutation in 0..4 {
        let mut forged = g.fork();
        let indices = siblings
            .iter()
            .map(|id| {
                forged
                    .state
                    .rules
                    .stack
                    .iter()
                    .position(|s| s.id == *id)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        match mutation {
            0 => {
                forged.state.rules.stack.remove(indices[0]);
            }
            1 => {
                let StackKind::Triggered { ability, .. } =
                    &mut forged.state.rules.stack[indices[0]].kind
                else {
                    panic!()
                };
                *ability = EffectiveTriggerAbility::Intrinsic { index: 1 };
            }
            2 => {
                let a = forged.state.rules.stack[indices[0]].kind.clone();
                let b = forged.state.rules.stack[indices[1]].kind.clone();
                forged.state.rules.stack[indices[0]].kind = b;
                forged.state.rules.stack[indices[1]].kind = a;
            }
            3 => {
                let StackKind::Triggered {
                    witness: TriggerEventWitness::SpellCast { cast },
                    ..
                } = &mut forged.state.rules.stack[indices[0]].kind
                else {
                    panic!()
                };
                cast.stack_id = siblings[1];
            }
            _ => unreachable!(),
        }
        assert_bad_restore(&forged);
    }
    offer(&mut g);
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!()
    };
    let token = o.decision.unwrap().token().clone();
    let QueryResult::Incomplete { work, .. } = g.check_player_response(
        PlayerId(0),
        token.clone(),
        PlayerResponse::OptionalEffect { perform: false },
        Budget::new(1),
    ) else {
        panic!()
    };
    let saved = g.save_work(&work).unwrap();
    answer(&mut g, Response::OptionalEffect { perform: false });
    request(&mut g);
    offer(&mut g);
    let before = g.state.clone();
    assert!(g.restore_work(&saved).is_err());
    for viewer in [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)] {
        assert!(matches!(
            g.check_player_response(
                viewer,
                token.clone(),
                PlayerResponse::OptionalEffect { perform: true },
                Budget::unlimited()
            ),
            QueryResult::Rejected(_)
        ));
    }
    assert_eq!(g.state, before);
    prove(&g);
}
#[test]
fn m4a_apex_into_existing_cascade_defers_children_and_counts_each_actual_cast() {
    let mut g = with_ramos(
        &["Bloodbraid Elf", "Shardless Agent", "Grizzly Bears"],
        false,
        TraceMode::Detailed,
    );
    let original = paid(&mut g, "Apex Devastator");
    let siblings = sibling_ids(&g, original);
    let mut actual = vec![original];
    for expected in ["Bloodbraid Elf", "Shardless Agent", "Grizzly Bears"] {
        offer(&mut g);
        let owner = g.top().unwrap().id;
        answer(&mut g, Response::OptionalEffect { perform: true });
        let mut r = request(&mut g);
        if let Request::OrderTriggers { candidates, .. } = &r {
            let response = Response::OrderTriggers {
                order: candidates.clone(),
            };
            g.respond(r.player(), r.decision_id(), response).unwrap();
            r = request(&mut g);
        }
        assert!(matches!(r, Request::Priority { .. }));
        assert!(!g.state.rules.stack.iter().any(|s| s.id == owner));
        assert!(g.state.rules.stack.iter().any(|s| s.id == original));
        let child = g
            .state
            .rules
            .stack
            .iter()
            .rev()
            .find(|s| s.kind.is_spell())
            .unwrap();
        assert_eq!(g.definition(child.source.card).unwrap().name, expected);
        actual.push(child.id);
        assert_eq!(sibling_ids(&g, original).len(), 3);
        prove(&g);
    }
    for id in actual {
        let ramos = g.events().iter().filter(|e| matches!(e, Event::TriggerCollected { source, witness: TriggerEventWitness::SpellCast { cast }, .. } if cast.stack_id == id && g.definition(source.card).unwrap().name == "Ramos, Dragon Engine")).count();
        assert_eq!(ramos, 1);
    }
    assert_eq!(siblings.len(), 4);
}
