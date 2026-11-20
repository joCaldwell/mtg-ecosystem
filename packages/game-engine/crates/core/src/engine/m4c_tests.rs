//! UNQUALIFIED source-only drafts. Reuse the cascade harness; private Prepared
//! roots preserve physical cards but establish no legal pod/profile admission.
use super::m4a_tests::{before_payment, replace};
use super::*;
const IMOTI: &str =
    include_str!("../../../../tests/fixtures/ramos/eca32dcd-6845-433e-a631-ed1f0ee78f25.json");
const RECLAMATION: &str = include_str!("../../../../tests/fixtures/natural-reclamation.json");
const MAGIC: &str =
    include_str!("../../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json");
pub(super) fn root(original: &str, library: &[&str], board: bool) -> Prepared {
    let mut p = m4a_tests::source_prepared(library, true);
    let c = Arc::make_mut(&mut p.catalog);
    for raw in [
        IMOTI,
        RECLAMATION,
        MAGIC,
        include_str!("../../../../tests/fixtures/blasphemous-act.json"),
    ] {
        c.definitions.push(lowered(raw));
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let original_id = c
        .definitions
        .iter()
        .find(|d| d.name == original)
        .unwrap()
        .oracle_id
        .clone();
    let apex_id = c
        .definitions
        .iter()
        .find(|d| d.name == "Apex Devastator")
        .unwrap()
        .oracle_id
        .clone();
    p.config
        .cards
        .iter_mut()
        .find(|c| c.owner == PlayerId(0) && c.oracle_id == apex_id)
        .unwrap()
        .oracle_id = original_id;
    if board {
        replace(
            &mut p,
            PlayerId(0),
            "Imoti, Celebrant of Bounty",
            Zone::Battlefield,
        );
    }
    // Ramos covers the blue observer/counter helpers' colors as well. These
    // remain private duplicate/basic-heavy roots, never legal-pod claims.
    let ramos = p.config.cards[0].oracle_id.clone();
    let other_commander = p.config.commanders[1];
    p.config
        .cards
        .iter_mut()
        .find(|c| c.id == other_commander)
        .unwrap()
        .oracle_id = ramos;
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn synthetic_grants(p: &mut Prepared, minimum: Quantity, count: usize, native: bool) {
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Imoti, Celebrant of Bounty")
        .unwrap();
    let mut a = d.static_abilities[0].clone();
    let StaticEffect::GrantSpellCascade { eligibility, .. } = &mut a.effect else {
        panic!()
    };
    *eligibility = mtg_contracts::SpellCascadeEligibility::ManaValueAtLeast { minimum };
    d.static_abilities = vec![a; count];
    if !native {
        d.triggered.clear();
    }
    d.capabilities = d.requirements();
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
}
pub(super) fn settle(g: &mut Game) {
    for _ in 0..400 {
        let r = request(g);
        if g.state.rules.stack.is_empty() && g.state.rules.pending_triggers.is_empty() {
            return;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::OptionalEffect { .. } => Response::OptionalEffect { perform: false },
            Request::OrderTriggers { candidates, .. } => {
                Response::OrderTriggers { order: candidates }
            }
            x => panic!("settle {x:?}"),
        };
        answer(g, response);
    }
    panic!("settle bound");
}
pub(super) fn grants(g: &Game, original: StackId) -> Vec<&StackInstance> {
    g.state.rules.stack.iter().filter(|s| matches!(&s.kind,
        StackKind::Triggered { ability: EffectiveTriggerAbility::Granted { .. }, witness: TriggerEventWitness::SpellCast { cast }, .. } if cast.stack_id == original)).collect()
}
pub(super) fn public_proof(g: &Game) {
    let state = g.state.clone();
    let trace = g.trace.clone();
    for seat in 0..4 {
        let viewer = PlayerId(seat);
        let QueryResult::Complete(QueryValue::Observation(expected)) =
            g.observe(viewer, Budget::unlimited())
        else {
            panic!()
        };
        let mut result = g.observe(viewer, Budget::new(1));
        for _ in 0..10000 {
            result = match result {
                QueryResult::Incomplete { work, .. } => {
                    let saved = g.save_work(&work).unwrap();
                    assert_eq!(
                        strict_read::<serde_json::Value>(&saved).unwrap()["work_version"],
                        30
                    );
                    g.resume_work(g.restore_work(&saved).unwrap(), Budget::new(1))
                }
                QueryResult::Complete(QueryValue::Observation(actual)) => {
                    assert_eq!(actual, expected);
                    break;
                }
                _ => panic!("incremental observation"),
            };
        }
        let public = canonical(&expected).unwrap();
        assert!(!public.contains("grant_source"));
        assert!(!public.contains("cast_history"));
        assert!(!public.contains("first_overall"));
        assert!(!public.contains("first_from_exile"));
        assert!(!public.contains("incarnation"));
        for s in &expected.view.stack {
            if let Some(t) = &s.triggered {
                match &t.ability {
                    VisibleTriggerAbility::Intrinsic { .. } => (),
                    VisibleTriggerAbility::Granted {
                        source,
                        oracle_id,
                        static_ability_index,
                    } => {
                        assert_eq!(
                            t.oracle_id,
                            g.definition(
                                g.state
                                    .rules
                                    .stack
                                    .iter()
                                    .find(|x| g.stack_handle_for(viewer, x.id).ok().as_ref()
                                        == Some(&t.handle))
                                    .unwrap()
                                    .source
                                    .card
                            )
                            .unwrap()
                            .oracle_id
                        );
                        assert!(!source.announced.0.is_empty());
                        assert!(!oracle_id.is_empty());
                        assert!(*static_ability_index < 257);
                    }
                }
            }
        }
    }
    assert_eq!(g.state, state);
    assert_eq!(g.trace, trace);
    prove(g);
    let wire = g.snapshot().unwrap();
    let mut reader = std::io::Cursor::new(wire.as_bytes());
    assert_eq!(
        Game::restore_from_reader(g.prepared.as_ref().clone(), &mut reader)
            .unwrap()
            .state,
        state
    );
    assert_eq!(
        SnapshotVerifier::new(g.prepared.as_ref().clone(), g.trace.mode)
            .verify(&wire)
            .unwrap()
            .state,
        state
    );
}
#[test]
fn m4c_paid_imoti_intrinsic_then_paid_ramos_has_granted_rules_source_and_frozen_caster() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(root("Imoti, Celebrant of Bounty", &["Forest"], false), mode);
        let first = paid(&mut g, "Imoti, Celebrant of Bounty");
        assert!(grants(&g, first).is_empty());
        assert!(g.state.rules.stack.iter().any(|s| matches!(
            s.kind,
            StackKind::Triggered {
                ability: EffectiveTriggerAbility::Intrinsic { index: 0 },
                ..
            }
        ) && s.source == g.state.rules.stack[0].source));
        settle(&mut g);
        public_proof(&g);
        let original = paid(&mut g, "Ramos, Dragon Engine");
        let a = grants(&g, original);
        assert_eq!(a.len(), 1);
        let StackKind::Triggered {
            grant: Some(provenance),
            ..
        } = &a[0].kind
        else {
            panic!()
        };
        assert_eq!(
            a[0].source,
            g.state
                .rules
                .stack
                .iter()
                .find(|s| s.id == original)
                .unwrap()
                .source
        );
        assert_ne!(a[0].source, provenance.source_facts.object);
        assert_eq!(a[0].controller, PlayerId(0));
        public_proof(&g);
    }
}
#[test]
fn m4c_paid_five_mana_nonqualifier_and_apex_native_four_plus_grant_are_independent() {
    let mut p = root("Natural Reclamation", &["Forest"], true);
    replace(&mut p, PlayerId(0), "Arcane Signet", Zone::Battlefield);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let original = paid(&mut g, "Natural Reclamation");
    assert!(grants(&g, original).is_empty());
    public_proof(&g);
    let mut g = Game::new(root("Apex Devastator", &["Forest"], true));
    let original = paid(&mut g, "Apex Devastator");
    assert_eq!(grants(&g, original).len(), 1);
    let intrinsic: BTreeSet<_> = g
        .state
        .rules
        .stack
        .iter()
        .filter_map(|s| match &s.kind {
            StackKind::Triggered {
                ability: EffectiveTriggerAbility::Intrinsic { index },
                witness: TriggerEventWitness::SpellCast { cast },
                ..
            } if cast.stack_id == original && s.source == g.state.rules.stack[0].source => {
                Some(*index)
            }
            _ => None,
        })
        .collect();
    assert_eq!(intrinsic, BTreeSet::from([0, 1, 2, 3]));
    public_proof(&g);
}
#[test]
fn m4c_synthetic_static_vectors_thresholds_and_grantor_without_intrinsic_cascade() {
    for (minimum, count, expected) in [(0, 2, 2), (6, 2, 2), (11, 2, 0), (2, 17, 17)] {
        let mut p = root("Apex Devastator", &["Forest"], true);
        synthetic_grants(&mut p, minimum.into(), count, false);
        let bound = p.catalog.cast_trigger_fanout_bound(400).unwrap();
        assert!(bound >= 400 * count);
        let mut g = Game::new(p);
        let original = paid(&mut g, "Apex Devastator");
        assert_eq!(grants(&g, original).len(), expected);
        let keys: BTreeSet<_> = grants(&g, original)
            .iter()
            .map(|s| match s.kind {
                StackKind::Triggered { ability, .. } => ability,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(keys.len(), expected);
        public_proof(&g);
    }
}
#[test]
fn m4c_normalized_grant_keys_omissions_duplicates_swaps_and_controller_forgeries_require_replay() {
    let mut p = root("Apex Devastator", &["Forest"], true);
    synthetic_grants(&mut p, 6.into(), 2, false);
    // Private duplicate in the graveyard is NOT a live legendary multiplicity
    // positive. It supplies a well-typed but historically false source key.
    let backup = replace(
        &mut p,
        PlayerId(0),
        "Imoti, Celebrant of Bounty",
        Zone::Graveyard { owner: PlayerId(0) },
    );
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Off);
    let original = paid(&mut g, "Apex Devastator");
    public_proof(&g);
    let indices: Vec<_> = g
        .state
        .rules
        .stack
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            grants(&g, original)
                .iter()
                .any(|a| a.id == s.id)
                .then_some(i)
        })
        .collect();
    for mutation in 0..7 {
        let mut forged = g.fork();
        match mutation {
            0 => {
                forged.state.rules.stack.remove(indices[0]);
            }
            1 | 2 => {
                let a = forged.state.rules.stack[indices[1]].kind.clone();
                let StackKind::Triggered { ability: other, .. } = a else {
                    panic!()
                };
                let StackKind::Triggered { ability, .. } =
                    &mut forged.state.rules.stack[indices[0]].kind
                else {
                    panic!()
                };
                let old = *ability;
                *ability = other;
                if mutation == 2 {
                    let StackKind::Triggered { ability, .. } =
                        &mut forged.state.rules.stack[indices[1]].kind
                    else {
                        panic!()
                    };
                    *ability = old;
                }
            }
            3 => {
                let s = &mut forged.state.rules.stack[indices[0]];
                let StackKind::Triggered {
                    ability,
                    grant: Some(grant),
                    ..
                } = &mut s.kind
                else {
                    panic!()
                };
                let EffectiveTriggerAbility::Granted { grant_source, .. } = ability else {
                    panic!()
                };
                grant_source.incarnation += 1;
                grant.source_facts.object = *grant_source;
            }
            4 => {
                let s = &mut forged.state.rules.stack[indices[0]];
                s.controller = PlayerId(2);
                s.source_facts.as_mut().unwrap().controller = PlayerId(2);
                let StackKind::Triggered {
                    grant: Some(grant),
                    witness: TriggerEventWitness::SpellCast { cast },
                    ..
                } = &mut s.kind
                else {
                    panic!()
                };
                grant.source_facts.controller = PlayerId(2);
                cast.caster = PlayerId(2);
                cast.spell.controller = PlayerId(2);
            }
            5 => {
                let object = g.object_id(backup).unwrap();
                let handles = (0..4)
                    .map(|v| g.handle_for(PlayerId(v), object).unwrap())
                    .collect();
                let StackKind::Triggered {
                    ability,
                    grant: Some(grant),
                    ..
                } = &mut forged.state.rules.stack[indices[0]].kind
                else {
                    panic!()
                };
                let EffectiveTriggerAbility::Granted { grant_source, .. } = ability else {
                    panic!()
                };
                *grant_source = object;
                grant.source_facts.object = object;
                grant.source_handles = handles;
            }
            6 => {
                let s = &mut forged.state.rules.stack[indices[0]];
                let StackKind::Triggered { ability, grant, .. } = &mut s.kind else {
                    panic!()
                };
                *ability = EffectiveTriggerAbility::Intrinsic { index: 0 };
                *grant = None;
            }
            _ => unreachable!(),
        }
        forged.refresh_visibility().unwrap();
        let wire = forged.snapshot().unwrap(); // Rehash all mutually edited fields.
        Game::read_snapshot(forged.prepared.as_ref().clone(), &wire)
            .expect("normalized shape must reach historical proof");
        assert!(
            Game::restore(forged.prepared.as_ref().clone(), &wire).is_err(),
            "historical lie {mutation}"
        );
    }
}
#[test]
fn m4c_granted_and_original_countered_independently_and_threshold_comes_from_original() {
    for counter_original in [false, true] {
        let mut g = Game::new(root("Apex Devastator", &["Grizzly Bears"], true));
        let original = paid(&mut g, "Apex Devastator");
        let granted = grants(&g, original)[0].id;
        let before = g.state.rules.stack.len();
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
            Some(if counter_original { original } else { granted }),
        );
        m4a_tests::resolve_entry(&mut g, counter);
        assert_eq!(g.state.rules.stack.len(), before - 1);
        assert_eq!(
            g.state.rules.stack.iter().any(|s| s.id == granted),
            counter_original
        );
        if counter_original {
            offer(&mut g);
            let Frame::Cascade { progress } = &g.state.rules.frame else {
                panic!()
            };
            assert_eq!(serde_json::to_value(progress).unwrap()["threshold"], "10");
        }
        public_proof(&g);
    }
}

#[test]
fn m4c_aggregate_observers_intrinsic_and_grants_fit_oneover_and_id_overflow_are_atomic() {
    let mut p = root("Apex Devastator", &["Forest"], true);
    synthetic_grants(&mut p, 6.into(), 2, false);
    let mut seed = Game::new(p);
    before_payment(&mut seed);
    // Explicit synthetic completed-stack control for direct collector isolation.
    let cost = seed.top().unwrap().total_cost.clone();
    seed.state.rules.stack.last_mut().unwrap().paid = Some(cost);
    let original = seed.top().unwrap().clone();
    let mut probe = seed.fork();
    probe.collect_cast_triggers(&original).unwrap();
    let batch = probe.state.rules.pending_triggers.len();
    assert_eq!(batch, 7);
    let template = probe.state.rules.pending_triggers[0].clone();
    for one_over in [false, true] {
        let mut g = seed.fork();
        g.state.rules.pending_triggers.resize(
            capacities::STACK_LIMIT - batch - usize::from(!one_over),
            template.clone(),
        );
        let state = g.state.clone();
        let trace = g.trace.clone();
        let result = g.collect_cast_triggers(&original);
        if one_over {
            assert!(result.is_err());
            assert!(matches!(
                g.typed_fault(),
                Some(RuleFault::StackCapacityExceeded { .. })
            ));
            g.state.rules.typed_fault = state.rules.typed_fault.clone();
            assert_eq!(g.state, state);
            assert_eq!(g.trace, trace);
        } else {
            result.unwrap();
            assert_eq!(
                g.state.rules.pending_triggers.len() + g.state.rules.stack.len(),
                capacities::STACK_LIMIT
            );
        }
    }
    for (trigger, stack) in [(u64::MAX - 6, 10), (10, u32::MAX - 6)] {
        let mut g = seed.fork();
        g.state.rules.next_trigger_id = trigger;
        g.state.rules.next_stack_id = stack;
        let before = g.state.clone();
        let trace = g.trace.clone();
        assert!(g.collect_cast_triggers(&original).is_err());
        assert!(g.typed_fault().is_some());
        g.state.rules.typed_fault = before.rules.typed_fault.clone();
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
    }
    let mut g = seed.fork();
    g.state.rules.next_trigger_id = u64::MAX - 7;
    g.state.rules.next_stack_id = u32::MAX - 7;
    g.collect_cast_triggers(&original).unwrap();
    assert_eq!(g.state.rules.pending_triggers.len(), 7);
}
#[test]
fn m4c_whole_paid_completion_history_and_id_faults_restore_payment_and_trace() {
    for fault in 0..3 {
        let mut g = Game::new(root("Apex Devastator", &["Forest"], true));
        before_payment(&mut g);
        match fault {
            0 => g.state.rules.next_trigger_id = u64::MAX - 4, // four intrinsic + observer + grant
            1 => g.state.rules.next_stack_id = u32::MAX - 4,
            2 => set_post_paid_history_fault(&mut g),
            _ => unreachable!(),
        }
        let state = g.state.clone();
        let trace = g.trace.clone();
        assert!(
            g.history_charge() <= capacities::HISTORY_LIMIT,
            "pre-step history must fit"
        );
        assert!(matches!(g.advance(Budget::new(1)), Advance::Fault(_)));
        assert!(g.typed_fault().is_some());
        if fault == 2 {
            assert!(matches!(
                g.typed_fault(),
                Some(RuleFault::HistoryCapacityExceeded { .. })
            ));
        }
        let mut after = g.state.clone();
        after.rules.frame = state.rules.frame.clone();
        after.rules.typed_fault = state.rules.typed_fault.clone();
        assert_eq!(after, state);
        assert_eq!(g.trace, trace);
    }
}
#[test]
fn m4c_direct_collector_history_failure_publishes_no_handles_ids_or_pending_payload() {
    let mut g = Game::new(root("Apex Devastator", &["Forest"], true));
    before_payment(&mut g);
    let cost = g.top().unwrap().total_cost.clone();
    g.state.rules.stack.last_mut().unwrap().paid = Some(cost);
    g.state.rules.retained_auxiliary_charge = capacities::HISTORY_LIMIT - 1;
    let entry = g.top().unwrap().clone();
    let state = g.state.clone();
    let trace = g.trace.clone();
    assert!(g.collect_cast_triggers(&entry).is_err());
    assert!(matches!(
        g.typed_fault(),
        Some(RuleFault::HistoryCapacityExceeded { .. })
    ));
    g.state.rules.typed_fault = state.rules.typed_fault.clone();
    assert_eq!(g.state, state);
    assert_eq!(g.trace, trace);
}
#[test]
fn m4c_lower_threshold_synthetic_descriptor_child_additional_costs_keep_exact_parent() {
    // Blow is MV3: this is explicitly a lower-threshold descriptor control,
    // never an Imoti-Min6 positive. Child selected kicker remains genuine M3.
    for kick in [false, true] {
        let mut p = root("Apex Devastator", &["Dismantling Blow"], true);
        synthetic_grants(&mut p, 0.into(), 1, false);
        replace(&mut p, PlayerId(0), "Arcane Signet", Zone::Battlefield);
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        let original = paid(&mut g, "Apex Devastator");
        let granted = grants(&g, original)[0].id;
        // Put this granted sibling on top through the real ordering choice in
        // an independent pending-boundary test below; here canonical native
        // siblings use the same owned child machinery and all remain distinct.
        offer(&mut g);
        let Frame::Cascade { progress } = &g.state.rules.frame else {
            panic!()
        };
        let owner = progress.frame.stack_id;
        assert!(owner == granted || g.state.rules.stack.iter().any(|s| s.id == granted));
        answer(&mut g, Response::OptionalEffect { perform: true });
        assert!(matches!(request(&mut g), Request::AdditionalCosts { .. }));
        public_proof(&g);
        m3c_tests::finish_announcement(&mut g, kick, None, None);
        assert!(!g.state.rules.stack.iter().any(|s| s.id == owner));
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
        assert_eq!(grants(&g, child.id).len(), 1); // Free mana cost is still MV3 >= synthetic zero.
        assert_eq!(
            child.selected_additional_costs.as_ref().unwrap().len(),
            usize::from(kick)
        );
        public_proof(&g);
    }
}

#[test]
fn m4c_pending_tagged_grant_views_exact_order_stale_work_and_accepted_replay() {
    let mut g = Game::new(root("Apex Devastator", &["Forest"], true));
    before_payment(&mut g);
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    assert_eq!(g.state.rules.pending_triggers.len(), 6);
    prove(&g);
    let r = request(&mut g);
    let Request::OrderTriggers { candidates, .. } = &r else {
        panic!()
    };
    assert_eq!(candidates.len(), 5); // four intrinsic + granted; observer is APNAP seat one
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(r.player(), Budget::unlimited())
    else {
        panic!()
    };
    let Some(PlayerRequest::OrderTriggers {
        token,
        candidates: visible,
        ..
    }) = o.decision
    else {
        panic!()
    };
    assert_eq!(
        visible
            .iter()
            .map(|v| v.handle.clone().0)
            .collect::<BTreeSet<_>>()
            .len(),
        5
    );
    assert_eq!(
        visible
            .iter()
            .filter(|v| matches!(v.ability, VisibleTriggerAbility::Granted { .. }))
            .count(),
        1
    );
    for candidate in &visible {
        let expected = match candidate.ability {
            VisibleTriggerAbility::Intrinsic { index } => {
                let spell = g.state.rules.stack.first().unwrap();
                g.trigger_source_label(spell.source, EffectiveTriggerAbility::Intrinsic { index })
            }
            VisibleTriggerAbility::Granted { ref oracle_id, .. } => {
                let definition = g
                    .prepared
                    .catalog
                    .definitions
                    .iter()
                    .find(|d| &d.oracle_id == oracle_id)
                    .unwrap();
                let labels = definition.source_labels.as_ref().unwrap();
                Some(labels.blocks[labels.statics[0] as usize].as_str())
            }
        };
        assert_eq!(candidate.label.as_deref(), expected);
        assert!(candidate.label.is_some());
    }
    let response = PlayerResponse::OrderTriggers {
        order: visible.iter().rev().map(|v| v.handle.clone()).collect(),
    };
    let QueryResult::Incomplete { work, .. } =
        g.check_player_response(r.player(), token.clone(), response.clone(), Budget::new(1))
    else {
        panic!()
    };
    let saved = g.save_work(&work).unwrap();
    let order: Vec<_> = candidates.iter().rev().copied().collect();
    g.respond(
        r.player(),
        r.decision_id(),
        Response::OrderTriggers {
            order: order.clone(),
        },
    )
    .unwrap();
    request(&mut g);
    assert!(g.restore_work(&saved).is_err());
    assert!(matches!(
        g.check_player_response(r.player(), token, response, Budget::unlimited()),
        QueryResult::Rejected(_)
    ));
    let placed: Vec<_> = g
        .state
        .rules
        .stack
        .iter()
        .filter_map(|s| match s.kind {
            StackKind::Triggered { trigger_id, .. } if s.controller == PlayerId(0) => {
                Some(trigger_id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(placed, order);
    public_proof(&g);
}
#[test]
fn m4c_synthetic_stack_x_and_cost_reduction_use_stack_value_not_payment() {
    for (x, qualifies) in [(0, false), (5, true)] {
        let mut p = root("Apex Devastator", &["Forest"], true);
        // Explicit synthetic spell combines the existing supported X damage
        // program and a single self-cascade. No printed-source claim.
        let c = Arc::make_mut(&mut p.catalog);
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Apex Devastator")
            .unwrap();
        d.x_mana_cost = true;
        d.characteristics.kind = CardKind::Sorcery;
        d.characteristics.legendary = false;
        d.characteristics.subtypes.clear();
        d.characteristics.power = None;
        d.characteristics.toughness = None;
        d.characteristics.mana_cost.generic = 0.into();
        d.characteristics.mana_cost.colored = vec![Color::G, Color::G];
        d.triggered.truncate(1);
        d.spell = lowered(include_str!("../../../../tests/fixtures/earthquake.json"))
            .spell
            .clone();
        d.capabilities = d.requirements();
        c.validate().unwrap();
        p.catalog_digest = digest(c).unwrap();
        let mut g = Game::new(p);
        answer(&mut g, Response::Cast { card: CardId(1) });
        assert!(matches!(request(&mut g), Request::AnnounceX { .. }));
        answer(&mut g, Response::AnnounceX { value: x.into() });
        m3c_tests::finish_announcement(&mut g, false, None, None);
        let original = g
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.kind.is_spell())
            .unwrap();
        assert_eq!(g.stack_mana_value(original).unwrap(), Quantity::from(x + 2));
        assert_eq!(grants(&g, original.id).len(), usize::from(qualifies));
        public_proof(&g);
    }
    // Isolated synthetic stack control: a paid total of zero preserves the
    // original cost/characteristic/X. Complete reduced/free witnesses follow.
    let mut g = Game::new(root("Apex Devastator", &["Forest"], true));
    before_payment(&mut g);
    let entry = g.state.rules.stack.last_mut().unwrap();
    entry.total_cost = ManaCost {
        generic: 0.into(),
        colored: vec![],
    };
    entry.paid = Some(entry.total_cost.clone());
    let entry = entry.clone();
    assert_eq!(g.stack_mana_value(&entry).unwrap(), 10.into());
    g.collect_cast_triggers(&entry).unwrap();
    assert_eq!(
        g.state
            .rules
            .pending_triggers
            .iter()
            .filter(|t| t.grant.is_some())
            .count(),
        1
    );
}

#[test]
fn m4c_complete_reduced_act_and_free_wanderer_preserve_original_mana_value() {
    let mut p = root("Blasphemous Act", &["Forest"], true);
    replace(&mut p, PlayerId(0), "Grizzly Bears", Zone::Battlefield);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let id = paid(&mut g, "Blasphemous Act");
    let entry = g.state.rules.stack.iter().find(|s| s.id == id).unwrap();
    assert_eq!(g.stack_mana_value(entry).unwrap(), 9.into());
    assert!(entry.total_cost.generic < 5.into());
    assert_eq!(grants(&g, id).len(), 1);
    public_proof(&g);
    let mut p = root("Apex Devastator", &[], true);
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.push(lowered(include_str!(
        "../../../../tests/fixtures/ramos/ad9b7fbc-61c8-43ee-a65c-99206fd1e4df.json"
    )));
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    replace(
        &mut p,
        PlayerId(0),
        "Maelstrom Wanderer",
        Zone::Library { owner: PlayerId(0) },
    );
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    paid(&mut g, "Apex Devastator");
    offer(&mut g);
    answer(&mut g, Response::OptionalEffect { perform: true });
    m3c_tests::finish_announcement(&mut g, false, None, None);
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
        "Maelstrom Wanderer"
    );
    assert_eq!(child.total_cost.generic, 0.into());
    assert!(child.total_cost.colored.is_empty());
    assert_eq!(g.stack_mana_value(child).unwrap(), 8.into());
    assert_eq!(grants(&g, child.id).len(), 1);
    public_proof(&g);
}

#[test]
fn m4c_genuine_granted_parent_child_kicker_nested_mana_failure_keep_and_undo() {
    // Imoti's actual minimum SIX applies to Apex, never to MV3 Blow. Blow's
    // selected kicker belongs to the free child of that granted invocation.
    for undo in [false, true] {
        let mut p = root("Apex Devastator", &["Dismantling Blow"], true);
        for name in ["Bloodbraid Elf", "Shardless Agent", "Healing Salve"] {
            replace(&mut p, PlayerId(0), name, Zone::Hand { owner: PlayerId(0) });
        }
        for name in std::iter::repeat_n("Forest", 12).chain(["Mountain", "Mountain"]) {
            replace(&mut p, PlayerId(0), name, Zone::Battlefield);
        }
        for _ in 0..2 {
            replace(&mut p, PlayerId(0), "Golgari Signet", Zone::Battlefield);
        }
        replace(&mut p, PlayerId(0), "Arcane Signet", Zone::Battlefield);
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        paid(&mut g, "Ramos, Dragon Engine");
        settle(&mut g);
        paid(&mut g, "Bloodbraid Elf");
        settle(&mut g);
        paid(&mut g, "Shardless Agent");
        settle(&mut g);
        let salve = g
            .state
            .rules
            .objects
            .values()
            .find(|o| g.definition(o.id.card).unwrap().name == "Healing Salve")
            .unwrap()
            .id;
        answer(&mut g, Response::Cast { card: salve.card });
        m3c_tests::finish_announcement(&mut g, false, None, None);
        settle(&mut g);
        let original = paid(&mut g, "Apex Devastator");
        let granted = grants(&g, original)[0].id;
        offer(&mut g);
        let Frame::Cascade { progress } = &g.state.rules.frame else {
            panic!()
        };
        assert_eq!(progress.frame.stack_id, granted);
        let ramos = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.zone == Zone::Battlefield
                    && g.definition(o.id.card).unwrap().name == "Ramos, Dragon Engine"
            })
            .unwrap()
            .id;
        let counters = g.current(ramos).unwrap().plus_one_counters.clone();
        assert!(counters >= 5.into());
        let before = g.state.rules.frame.clone();
        let resources = g.state.rules.players[0].mana.clone();
        let casts = g
            .events()
            .iter()
            .filter(|e| matches!(e, Event::SpellCast { .. }))
            .count();
        let target = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.zone == Zone::Battlefield
                    && g.definition(o.id.card).unwrap().name == "Arcane Signet"
            })
            .unwrap()
            .id;
        answer(&mut g, Response::OptionalEffect { perform: true });
        let Request::AdditionalCosts { candidates, .. } = request(&mut g) else {
            panic!()
        };
        answer(
            &mut g,
            Response::AdditionalCosts {
                selected: vec![candidates[0].identity.clone()],
            },
        );
        answer(
            &mut g,
            Response::Targets {
                objects: vec![target],
            },
        );
        assert!(matches!(request(&mut g), Request::Mana { .. }));
        let signets: Vec<_> = g
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && g.definition(o.id.card).unwrap().name == "Golgari Signet"
            })
            .map(|o| o.id)
            .collect();
        answer(&mut g, Response::ActivateMana { object: signets[0] });
        answer(&mut g, Response::ActivateMana { object: ramos });
        answer(&mut g, Response::FinishMana { undo_mana: false });
        answer(
            &mut g,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::U, 1.into())]),
            },
        );
        answer(&mut g, Response::ActivateMana { object: signets[1] });
        answer(&mut g, Response::FinishMana { undo_mana: false });
        answer(
            &mut g,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::U, 1.into())]),
            },
        );
        assert!(matches!(request(&mut g), Request::Mana { .. }));
        assert!(
            !g.can_pay(PlayerId(0), &g.top().unwrap().total_cost)
                .unwrap()
        );
        public_proof(&g);
        answer(&mut g, Response::FinishMana { undo_mana: undo });
        assert!(matches!(request(&mut g), Request::OptionalEffect { .. }));
        assert_eq!(g.state.rules.frame, before);
        assert_eq!(
            g.events()
                .iter()
                .filter(|e| matches!(e, Event::SpellCast { .. }))
                .count(),
            casts
        );
        let mut remaining = counters.clone();
        if !undo {
            remaining.subtract(&5.into());
        }
        assert_eq!(g.current(ramos).unwrap().plus_one_counters, remaining);
        assert_eq!(g.mana_ability_used(ramos), !undo);
        if undo {
            assert_eq!(g.state.rules.players[0].mana, resources);
        }
        assert!(
            signets
                .iter()
                .all(|s| g.current(*s).unwrap().tapped == !undo)
        );
        public_proof(&g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        m3c_tests::finish_announcement(&mut g, false, Some(target), None);
        let child = g
            .state
            .rules
            .stack
            .iter()
            .rev()
            .find(|s| s.kind.is_spell())
            .unwrap();
        assert!(grants(&g, child.id).is_empty()); // Actual Imoti minimum excludes MV3.
        assert_eq!(
            g.events()
                .iter()
                .filter(|e| matches!(e, Event::SpellCast { .. }))
                .count(),
            casts + 1
        );
        assert!(!g.state.rules.stack.iter().any(|s| s.id == granted));
        public_proof(&g);
    }
}

pub(super) fn set_post_paid_history_fault(g: &mut Game) {
    // Calibrate from the real accepted payment step, not an already
    // over-limit starting state. Coalescing matches advance().
    let original = g.top().unwrap().id;
    let mut completed = g.fork();
    completed.in_rule_step = true;
    completed.step().unwrap();
    assert!(
        completed
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.id == original)
            .unwrap()
            .paid
            .is_some()
    );
    let grant_growth = completed
        .grant_payload_charge()
        .checked_sub(g.grant_payload_charge())
        .unwrap();
    assert!(grant_growth > 0);
    // Find the narrow headroom that reaches paid completion in the
    // actual staged step. Payment events have their own retained
    // history, so grant bytes alone are not the pre-payment margin.
    let maximum = capacities::HISTORY_LIMIT
        .checked_sub(g.history_charge())
        .unwrap();
    let mut low = 0;
    let mut high = maximum;
    while low < high {
        let middle = low + (high - low) / 2;
        let mut probe = g.fork();
        probe.state.rules.retained_auxiliary_charge += maximum - middle;
        probe.in_rule_step = true;
        let result = probe.step();
        if let Err(error) = result {
            assert!(
                matches!(
                    probe.typed_fault(),
                    Some(RuleFault::HistoryCapacityExceeded { .. })
                ),
                "unexpected calibration fault: {error}"
            );
        }
        if probe
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.id == original)
            .unwrap()
            .paid
            .is_some()
        {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let headroom = low;
    g.state.rules.retained_auxiliary_charge += maximum - headroom;
    assert_eq!(g.history_charge(), capacities::HISTORY_LIMIT - headroom);
    println!("M4C_HISTORY_PROBE headroom={headroom} grant_growth={grant_growth}");
    let mut reached = g.fork();
    reached.in_rule_step = true;
    let step = reached.step();
    assert!(
        reached
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.id == original)
            .unwrap()
            .paid
            .is_some()
    );
    assert!(
        reached.history_charge() > capacities::HISTORY_LIMIT
            || matches!(
                reached.typed_fault(),
                Some(RuleFault::HistoryCapacityExceeded { .. })
            )
    );
    if step.is_err() {
        assert!(matches!(
            reached.typed_fault(),
            Some(RuleFault::HistoryCapacityExceeded { .. })
        ));
    }
}
