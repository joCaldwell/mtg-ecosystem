//! Typed scalar-effect kernel probes, NOT accepted-history witnesses. A paid
//! complete-source prefix supplies the resolving instruction; deliberate private
//! mutations below bypass the gameplay lifecycle. Native execution is UNRUN;
//! the parent owns the combined gate and paid departure/replay witnesses.
//! CR 400.7, 611.2a/c, 613.1f/4c/5-6:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{object, request};
use super::temporary_indestructible_tests::{announce, pass, target};
use super::*;

fn resolving_grant(compound: bool) -> (Game, ResolvedEffect, BatchCause) {
    let mut p = equipped_indestructible_tests::prepared(
        &[
            ("Giant Growth", true),
            ("Grizzly Bears", false),
            ("Grizzly Bears", false),
            ("Swamp", false),
            ("Swamp", false),
        ],
        false,
    );
    let raw = if compound {
        include_str!("../../../../tests/fixtures/058a8475-649f-433a-b258-42bc03bf40ba.json")
    } else {
        include_str!("../../../../tests/fixtures/533d6b9f-a145-44c4-9d16-74fcd9c0ae2f.json")
    };
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        raw,
    ) else {
        panic!("complete pinned scalar-grant source")
    };
    let mut c = (*p.catalog).clone();
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    c.definitions.push(*definition);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    let targets = target(&g, 1);
    announce(&mut g, 1, 0, targets);
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
    let program = g.program_for(&frame.spell).unwrap();
    assert_eq!(program.instructions.len(), 1);
    let Instruction::GrantKeywords {
        target_slot: 0,
        modifier,
        keywords,
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    } = &program.instructions[0]
    else {
        panic!("one atomic scalar source instruction")
    };
    assert_eq!(
        modifier,
        &compound.then(|| CreatureStatModifier {
            power: Quantity::from(3),
            toughness: Quantity::from(1),
        })
    );
    assert_eq!(
        keywords,
        &if compound {
            vec![KeywordAbility::Lifelink]
        } else {
            vec![KeywordAbility::Deathtouch, KeywordAbility::Indestructible]
        }
    );
    let kind = ResolvedEffectKind::GrantKeywords {
        target: object(&g, 1),
        modifier: modifier.clone(),
        keywords: keywords.clone(),
    };
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
        target_handles: g.capture_trigger_source_handles(object(&g, 1)).unwrap(),
        expires_in_cleanup_of_turn: g.state.rules.turn.number,
        kind,
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
fn scalar_keyword_kernel_modifier_forgery_rejects_creation_and_history() {
    for compound in [false, true] {
        let (g, effect, cause) = resolving_grant(compound);
        let ResolvedEffectKind::GrantKeywords {
            modifier: expected, ..
        } = &effect.kind
        else {
            unreachable!()
        };
        for replacement in [
            None,
            Some(CreatureStatModifier {
                power: Quantity::zero(),
                toughness: Quantity::zero(),
            }),
            Some(CreatureStatModifier {
                power: Quantity::from(-3),
                toughness: Quantity::from(1),
            }),
            Some(CreatureStatModifier {
                power: Quantity::from(3),
                toughness: Quantity::from(2),
            }),
            Some(CreatureStatModifier {
                power: "900719925474099312345678901234567890".parse().unwrap(),
                toughness: Quantity::from(1),
            }),
        ] {
            if &replacement == expected {
                continue;
            }
            let mut creation = g.fork();
            let mut bad = effect.clone();
            let ResolvedEffectKind::GrantKeywords { modifier, .. } = &mut bad.kind else {
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
            let ResolvedEffectKind::GrantKeywords { modifier, .. } =
                &mut history.state.rules.effects[0].kind
            else {
                unreachable!()
            };
            *modifier = replacement;
            assert!(history.validate_effect_state().is_err());
        }
    }
}

#[test]
fn scalar_keyword_kernel_creation_preserves_exact_target_keywords_and_provenance_guards() {
    for compound in [false, true] {
        let (g, effect, cause) = resolving_grant(compound);
        for mutation in 0..10 {
            let mut g = g.fork();
            let mut bad = effect.clone();
            match mutation {
                0 => {
                    let ResolvedEffectKind::GrantKeywords { target, .. } = &mut bad.kind else {
                        unreachable!()
                    };
                    *target = object(&g, 2);
                    bad.target_handles = g.capture_trigger_source_handles(*target).unwrap();
                }
                1 | 2 => {
                    let ResolvedEffectKind::GrantKeywords { keywords, .. } = &mut bad.kind else {
                        unreachable!()
                    };
                    if mutation == 1 {
                        keywords.push(keywords[0]);
                    } else if keywords.len() > 1 {
                        keywords.reverse();
                    } else {
                        *keywords = vec![KeywordAbility::Flying];
                    }
                }
                3 => bad.origin.source_block += 1,
                4 => bad.origin.cursor += 1,
                5 => bad.origin.controller = PlayerId(1),
                6 => bad.expires_in_cleanup_of_turn += 1,
                7 => bad.target_handles.swap(0, 1),
                8 => bad.origin.source_handles.swap(0, 1),
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
}

#[test]
fn scalar_keyword_kernel_one_effect_all_metadata_and_late_failure_rollback() {
    for compound in [false, true] {
        let (mut g, effect, cause) = resolving_grant(compound);
        let Frame::Resolve { frame } = &g.state.rules.frame else {
            unreachable!()
        };
        let frame = frame.clone();
        let instruction = g.program_for(&frame.spell).unwrap().instructions[0].clone();
        let before = g.state.clone();
        let trace = g.trace.clone();
        let original_recipient = g.object_before(object(&g, 1)).unwrap();
        let mut recipient = original_recipient.clone();
        let outsider = g.object_before(object(&g, 2)).unwrap();
        // The second occurrence fails after private creation succeeded; every
        // characteristic, allocator, handle and evidence write must roll back.
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
        assert_eq!(g.object_before(recipient.object).unwrap(), recipient);
        g.execute_effect_instruction(frame, instruction).unwrap();
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
        let ResolvedEffectKind::GrantKeywords {
            target,
            modifier,
            keywords,
        } = &effect.kind
        else {
            unreachable!()
        };
        if let Some(modifier) = modifier {
            recipient
                .characteristics
                .power
                .as_mut()
                .unwrap()
                .add(&modifier.power);
            recipient
                .characteristics
                .toughness
                .as_mut()
                .unwrap()
                .add(&modifier.toughness);
        }
        recipient.keywords.extend(keywords);
        assert_eq!(g.object_before(recipient.object).unwrap(), recipient);
        assert_eq!(g.object_before(outsider.object).unwrap(), outsider);
        // Direct kernel execution does not publish query handles. Normal
        // advance does this before queries, which must themselves be read-only.
        g.refresh_effect_handles().unwrap();
        let published = g.state.clone();
        for viewer in 0..4 {
            let viewer = PlayerId(viewer);
            let visible = g.visible_effects(viewer).unwrap();
            assert_eq!(visible.len(), 1);
            let VisibleEffect::GrantKeywords {
                handle,
                origin,
                target: visible_target,
                modifier: visible_modifier,
                keywords: visible_keywords,
                expires_in_cleanup_of_turn,
            } = &visible[0]
            else {
                panic!("one complete public effect")
            };
            assert_eq!(
                handle,
                &g.effect_handle(viewer, &EffectKey::Resolved { id: effect.id })
                    .unwrap()
            );
            assert_eq!(origin.controller, effect.origin.controller);
            assert_eq!(
                origin.oracle_id,
                g.definition(effect.origin.source.card).unwrap().oracle_id
            );
            assert_eq!(
                origin.source.announced,
                effect.origin.source_handles[viewer.0 as usize]
            );
            assert_eq!(
                origin.source.current,
                g.handle_for(viewer, effect.origin.source).ok()
            );
            assert_eq!(
                visible_target.announced,
                effect.target_handles[viewer.0 as usize]
            );
            assert_eq!(visible_target.current, g.handle_for(viewer, *target).ok());
            assert_eq!(visible_modifier, modifier);
            assert_eq!(visible_keywords, keywords);
            assert_eq!(
                *expires_in_cleanup_of_turn,
                effect.expires_in_cleanup_of_turn
            );
            if !compound {
                // Preserve the pre-modifier scalar wire bytes, including field
                // order, rather than merely accepting absent/null equivalence.
                assert_eq!(
                    serde_json::to_string(&effect.kind).unwrap(),
                    format!(
                        "{{\"effect\":\"grant_keywords\",\"target\":{},\"keywords\":{}}}",
                        serde_json::to_string(target).unwrap(),
                        serde_json::to_string(keywords).unwrap(),
                    )
                );
                assert_eq!(
                    serde_json::to_string(&visible[0]).unwrap(),
                    format!(
                        "{{\"effect\":\"grant_keywords\",\"handle\":{},\"origin\":{},\"target\":{},\"keywords\":{},\"expires_in_cleanup_of_turn\":{}}}",
                        serde_json::to_string(handle).unwrap(),
                        serde_json::to_string(origin).unwrap(),
                        serde_json::to_string(visible_target).unwrap(),
                        serde_json::to_string(keywords).unwrap(),
                        effect.expires_in_cleanup_of_turn,
                    )
                );
            }
        }
        assert_eq!(g.state, published);
        g.validate_effect_state().unwrap();
        let creations: Vec<_> = g
            .trace
            .records
            .iter()
            .filter_map(|r| match &r.record {
                EvidenceRecord::Batch { batch } => Some(batch),
                _ => None,
            })
            .flat_map(|batch| &batch.occurrences)
            .filter(|occurrence| matches!(occurrence.proposed, ProposedEvent::CreateEffect { .. }))
            .collect();
        assert_eq!(creations.len(), 1);
        assert_eq!(
            creations[0].actual,
            EventOutcome::EffectCreated {
                effect: Box::new(effect.clone())
            }
        );
        assert_eq!(
            creations[0]
                .before
                .objects
                .iter()
                .find(|o| o.object == *target),
            Some(&original_recipient)
        );
    }
}

#[test]
fn scalar_keyword_kernel_historical_target_survives_control_and_never_rebinds() {
    for compound in [false, true] {
        let (mut g, effect, cause) = resolving_grant(compound);
        let captured = object(&g, 1);
        let original = g.object_before(captured).unwrap().characteristics;
        g.commit(event(effect.clone()), cause).unwrap();
        let applied = g.object_before(captured).unwrap();
        g.state
            .rules
            .objects
            .get_mut(&CardId(1))
            .unwrap()
            .controller = PlayerId(1);
        // Structural validation authenticates properties against the source;
        // it must not reselect a historical recipient from present control.
        // These private mutations make no accepted restore/replay claim.
        g.validate_effect_state().unwrap();
        assert_eq!(
            g.object_before(captured).unwrap().characteristics,
            applied.characteristics
        );
        assert_eq!(g.current_keywords(captured), applied.keywords);
        assert_eq!(g.state.rules.effects, vec![effect.clone()]);
        let current = g.state.rules.objects.get_mut(&CardId(1)).unwrap();
        current.id.incarnation += 1;
        let successor = current.id;
        assert!(g.object_before(captured).is_err());
        assert_eq!(
            g.object_before(successor).unwrap().characteristics,
            original
        );
        assert!(g.current_keywords(successor).is_empty());
        g.validate_effect_state().unwrap();
        // Publish the deliberately changed incarnation before inspecting views.
        g.refresh_visibility().unwrap();
        for viewer in 0..4 {
            let visible = g.visible_effects(PlayerId(viewer)).unwrap();
            let VisibleEffect::GrantKeywords {
                target,
                modifier,
                keywords,
                ..
            } = &visible[0]
            else {
                panic!("historical scalar effect")
            };
            assert_eq!(target.announced, effect.target_handles[viewer as usize]);
            assert!(target.current.is_none());
            let ResolvedEffectKind::GrantKeywords {
                modifier: expected_modifier,
                keywords: expected_keywords,
                ..
            } = &effect.kind
            else {
                unreachable!()
            };
            assert_eq!(modifier, expected_modifier);
            assert_eq!(keywords, expected_keywords);
        }
    }
}
