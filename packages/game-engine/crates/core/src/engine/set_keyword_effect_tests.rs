//! Kernel probes of typed set-effect validation and rollback, NOT accepted-history
//! witnesses. The paid source prefix only supplies a real resolving instruction;
//! subsequent private mutations deliberately bypass the gameplay lifecycle.
//! Native execution of these new probes is UNRUN; parent owns qualification.
//! CR 400.7, 611.2a/c, 613.1f/5-6, 514.2:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{object, request};
use super::temporary_indestructible_tests::{announce, pass};
use super::*;

fn resolving_grant() -> (Game, ResolvedEffect, BatchCause) {
    resolving_set_grant(false)
}

fn resolving_set_grant(compound: bool) -> (Game, ResolvedEffect, BatchCause) {
    let mut rows = vec![
        ("Giant Growth", true),
        ("Grizzly Bears", false),
        ("Grizzly Bears", false),
        ("Grizzly Bears", false),
    ];
    rows.extend(if compound {
        vec![("Plains", false); 3]
    } else {
        vec![("Mountain", false)]
    });
    let mut p = equipped_indestructible_tests::prepared(&rows, false);
    let raw = if compound {
        include_str!("../../../../tests/fixtures/531f78d5-5004-4b02-99c7-b390cb342fd9.json")
    } else {
        include_str!("../../../../tests/fixtures/91e33674-b13a-41de-8998-4b57016ae42f.json")
    };
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        raw,
    ) else {
        panic!("complete pinned set-grant source")
    };
    let mut c = (*p.catalog).clone();
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    p.config.cards[3].controller = PlayerId(1);
    c.definitions.push(*definition);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    announce(&mut g, 1, 0, vec![]);
    let mut found = None;
    for _ in 0..2000 {
        if let Frame::Resolve { frame } = &g.state.rules.frame
            && frame.cursor == 0
        {
            found = Some(frame.clone());
            break;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(r) => {
                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            other => panic!("resolving prefix: {other:?}"),
        }
    }
    let frame = found.expect("bounded paid resolving prefix");
    let targets = g.objects_in_set(&ObjectSet::ControllerCreatures, PlayerId(0));
    assert_eq!(targets, vec![object(&g, 1), object(&g, 2)]);
    let mut target_handles = vec![];
    for target in &targets {
        target_handles.extend(g.capture_trigger_source_handles(*target).unwrap());
    }
    let effect = ResolvedEffect {
        id: EffectId(g.state.rules.next_effect_id),
        origin: EffectOrigin {
            mode: None,
            stack_id: frame.spell.id,
            source: frame.spell.source,
            controller: frame.spell.controller,
            source_handles: frame.spell.effect_source_handles.clone(),
            cursor: frame.cursor,
            source_block: 0,
        },
        target_handles,
        expires_in_cleanup_of_turn: g.state.rules.turn.number,
        kind: ResolvedEffectKind::GrantKeywordsSet {
            targets,
            modifier: compound.then(|| CreatureSetModifier {
                power: Quantity::from(1),
                toughness: Quantity::zero(),
            }),
            keywords: vec![if compound {
                KeywordAbility::Indestructible
            } else {
                KeywordAbility::Haste
            }],
        },
    };
    let cause = Game::wipe_cause(&frame, 0);
    (g, effect, cause)
}

fn event(effect: ResolvedEffect) -> ProposedEvent {
    ProposedEvent::CreateEffect {
        effect: Box::new(effect),
    }
}

#[test]
fn set_keyword_kernel_creation_rejects_wrong_members_order_and_authority_atomically() {
    let (g, effect, cause) = resolving_grant();
    for mutation in 0..12 {
        let mut g = g.fork();
        let mut bad = effect.clone();
        match mutation {
            0 => {
                let ResolvedEffectKind::GrantKeywordsSet { targets, .. } = &mut bad.kind else {
                    unreachable!()
                };
                targets.reverse();
                // Even coherently reordered historical handles cannot authorize
                // a noncanonical current selection.
                bad.target_handles.rotate_left(4);
            }
            1 => {
                let ResolvedEffectKind::GrantKeywordsSet { targets, .. } = &mut bad.kind else {
                    unreachable!()
                };
                targets.pop();
                bad.target_handles.truncate(4);
            }
            2..=4 => {
                let replacement = match mutation {
                    2 => object(&g, 1), // duplicate
                    3 => object(&g, 3), // other controller
                    _ => object(&g, 4), // noncreature
                };
                let ResolvedEffectKind::GrantKeywordsSet { targets, .. } = &mut bad.kind else {
                    unreachable!()
                };
                targets[1] = replacement;
                bad.target_handles.truncate(4);
                bad.target_handles
                    .extend(g.capture_trigger_source_handles(replacement).unwrap());
            }
            5 => bad.origin.source_block = 1,
            6 => {
                let ResolvedEffectKind::GrantKeywordsSet { keywords, .. } = &mut bad.kind else {
                    unreachable!()
                };
                *keywords = vec![KeywordAbility::Flying];
            }
            7 => bad.origin.controller = PlayerId(1),
            8 => bad.expires_in_cleanup_of_turn += 1,
            9 => bad.target_handles.swap(0, 4),
            10 => bad.origin.source_handles.swap(0, 1),
            _ => bad.id.0 += 1,
        }
        let before = g.state.clone();
        let trace = g.trace.clone();
        assert!(
            g.commit(event(bad), cause.clone()).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(g.state, before, "mutation {mutation}");
        assert_eq!(g.trace, trace, "mutation {mutation}");
    }
}

#[test]
fn set_keyword_kernel_one_effect_and_late_failure_roll_back_allocator_and_handles() {
    let (mut g, effect, cause) = resolving_grant();
    let before = g.state.clone();
    let trace = g.trace.clone();
    // First creation succeeds privately; the repeated ID fails. The whole
    // ordered group must restore the allocator, effect vector and evidence.
    assert!(
        g.commit_batch(
            cause.clone(),
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: event(effect.clone())
                    },
                    EventGroup::Occurrence {
                        proposed: event(effect.clone())
                    },
                ],
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    g.commit(event(effect.clone()), cause).unwrap();
    assert_eq!(g.state.rules.effects, vec![effect.clone()]);
    assert_eq!(g.state.rules.next_effect_id, effect.id.0 + 1);
    for id in [1, 2] {
        assert!(g.has_keyword(object(&g, id), KeywordAbility::Haste));
    }
    assert!(!g.has_keyword(object(&g, 3), KeywordAbility::Haste));
    g.validate_effect_state().unwrap();
}

#[test]
fn set_keyword_kernel_historical_membership_survives_control_but_never_rebinds_incarnation() {
    for compound in [false, true] {
        let (mut g, effect, cause) = resolving_set_grant(compound);
        let keyword = if compound {
            KeywordAbility::Indestructible
        } else {
            KeywordAbility::Haste
        };
        let captured = object(&g, 1);
        let original = g.object_before(captured).unwrap().characteristics;
        g.commit(event(effect), cause).unwrap();
        let applied = g.object_before(captured).unwrap().characteristics;
        g.state
            .rules
            .objects
            .get_mut(&CardId(1))
            .unwrap()
            .controller = PlayerId(1);
        g.state
            .rules
            .objects
            .get_mut(&CardId(3))
            .unwrap()
            .controller = PlayerId(0);
        // A structural historical check must not re-evaluate current controllers.
        // This direct call makes no authenticated restore/replay claim.
        g.validate_effect_state().unwrap();
        assert!(g.has_keyword(captured, keyword));
        assert_eq!(g.object_before(captured).unwrap().characteristics, applied);
        assert!(!g.has_keyword(object(&g, 3), keyword));
        assert_eq!(
            g.object_before(object(&g, 3)).unwrap().characteristics,
            original
        );
        let current = g.state.rules.objects.get_mut(&CardId(1)).unwrap();
        current.id.incarnation += 1;
        let successor = current.id;
        assert!(!g.has_keyword(captured, keyword));
        assert!(!g.has_keyword(successor, keyword));
        assert!(g.object_before(captured).is_err());
        assert_eq!(
            g.object_before(successor).unwrap().characteristics,
            original
        );
        g.validate_effect_state().unwrap();
    }
}

#[test]
fn set_keyword_kernel_modifier_forgery_rejects_creation_and_historical_state() {
    for compound in [false, true] {
        let (g, effect, cause) = resolving_set_grant(compound);
        let ResolvedEffectKind::GrantKeywordsSet {
            modifier: expected, ..
        } = &effect.kind
        else {
            unreachable!()
        };
        let replacements = [
            None,
            Some(CreatureSetModifier {
                power: Quantity::zero(),
                toughness: Quantity::zero(),
            }),
            Some(CreatureSetModifier {
                power: Quantity::from(-1),
                toughness: Quantity::zero(),
            }),
            Some(CreatureSetModifier {
                power: Quantity::from(1),
                toughness: Quantity::from(1),
            }),
            Some(CreatureSetModifier {
                power: "900719925474099312345678901234567890".parse().unwrap(),
                toughness: Quantity::zero(),
            }),
        ];
        for replacement in replacements {
            if &replacement == expected {
                continue;
            }
            let mut creation = g.fork();
            let mut bad = effect.clone();
            let ResolvedEffectKind::GrantKeywordsSet { modifier, .. } = &mut bad.kind else {
                unreachable!()
            };
            *modifier = replacement.clone();
            let before = creation.state.clone();
            let trace = creation.trace.clone();
            assert!(creation.commit(event(bad), cause.clone()).is_err());
            assert_eq!(creation.state, before);
            assert_eq!(creation.trace, trace);

            let mut history = g.fork();
            history
                .commit(event(effect.clone()), cause.clone())
                .unwrap();
            history.validate_effect_state().unwrap();
            let ResolvedEffectKind::GrantKeywordsSet { modifier, .. } =
                &mut history.state.rules.effects[0].kind
            else {
                unreachable!()
            };
            *modifier = replacement;
            // Structural history validation binds every property to the source
            // instruction; this deliberately forged state is not a restore witness.
            assert!(history.validate_effect_state().is_err());
        }
    }
}

#[test]
fn set_keyword_kernel_compound_execution_creates_all_layers_once_and_rolls_back() {
    let (mut g, effect, cause) = resolving_set_grant(true);
    let Frame::Resolve { frame } = &g.state.rules.frame else {
        unreachable!()
    };
    let frame = frame.clone();
    let program = g.program_for(&frame.spell).unwrap();
    assert_eq!(program.instructions.len(), 1);
    let instruction = program.instructions[0].clone();
    let before = g.state.clone();
    let trace = g.trace.clone();
    let captured: Vec<_> = [1, 2]
        .into_iter()
        .map(|id| g.object_before(object(&g, id)).unwrap())
        .collect();
    let outsider = g.object_before(object(&g, 3)).unwrap();
    for fact in &captured {
        assert!(!fact.keywords.contains(&KeywordAbility::Indestructible));
    }
    // A late group failure must undo all layers together, including the ID,
    // handles and trace, even after private creation succeeded.
    assert!(
        g.commit_batch(
            cause,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: event(effect.clone())
                    },
                    EventGroup::Occurrence {
                        proposed: event(effect.clone())
                    },
                ],
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    for fact in &captured {
        assert_eq!(&g.object_before(fact.object).unwrap(), fact);
    }

    g.execute_wipes_instruction(frame, instruction).unwrap();
    assert_eq!(g.state.rules.effects, vec![effect.clone()]);
    assert_eq!(
        g.state.rules.next_effect_id,
        before.rules.next_effect_id + 1
    );
    let Frame::Resolve { frame } = &g.state.rules.frame else {
        unreachable!()
    };
    assert_eq!(frame.cursor, 1);
    assert_eq!(frame.outcomes.len(), 1);
    assert!(matches!(
        frame.outcomes[0],
        InstructionOutcome::Effect { actual: Some(_) }
    ));
    let expected_modifier = Some(CreatureSetModifier {
        power: Quantity::from(1),
        toughness: Quantity::zero(),
    });
    // This kernel invoked the instruction directly. Normal advance publishes
    // these handles before any query; the query itself must never mint them.
    g.refresh_effect_handles().unwrap();
    for viewer in 0..4 {
        let visible = g.visible_effects(PlayerId(viewer)).unwrap();
        assert_eq!(visible.len(), 1);
        let VisibleEffect::GrantKeywordsSet {
            modifier,
            targets,
            keywords,
            ..
        } = &visible[0]
        else {
            panic!("one complete public effect")
        };
        assert_eq!(modifier, &expected_modifier);
        assert_eq!(targets.len(), captured.len());
        assert_eq!(keywords, &[KeywordAbility::Indestructible]);
    }
    for mut fact in captured {
        fact.characteristics
            .power
            .as_mut()
            .unwrap()
            .add(&Quantity::from(1));
        fact.keywords.push(KeywordAbility::Indestructible);
        // The same typed facts consumed by departure/LKI capture see both
        // properties immediately after the single creation boundary.
        assert_eq!(g.object_before(fact.object).unwrap(), fact);
    }
    assert_eq!(g.object_before(outsider.object).unwrap(), outsider);
    g.validate_effect_state().unwrap();
}
