//! Typed kernel probes, NOT accepted-history, printed-card or admission claims.
//! The unchanged protective pair uses complete archived Heroic Intervention;
//! every other keyword vector is explicitly a synthetic typed control. Private
//! mutations below deliberately bypass lifecycle/replay. All tests are UNRUN;
//! the parent owns native qualification and paid source/departure witnesses.
//! CR 400.7, 608.2h, 611.2c, 613.1f, 702.11b, 702.12b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_witness::equipped::{answer, object, request};
use mtg_witness::temporary::{announce, pass};

const PROTECTION: &[KeywordAbility] = &[KeywordAbility::Hexproof, KeywordAbility::Indestructible];
const REVIEWED: &[KeywordAbility] = &[
    KeywordAbility::Flying,
    KeywordAbility::Deathtouch,
    KeywordAbility::Indestructible,
    KeywordAbility::Lifelink,
    KeywordAbility::Hexproof,
    KeywordAbility::Haste,
    KeywordAbility::Trample,
    KeywordAbility::FirstStrike,
];

fn prepared(keywords: &[KeywordAbility]) -> Prepared {
    let mut p = mtg_witness::equipped::prepared(
        &[
            ("Giant Growth", true),
            ("Grizzly Bears", false),
            ("Sol Ring", false),
            ("Forest", false),
            ("Goblin Bombardment", false),
            ("Grizzly Bears", false),
            ("Forest", true),
            ("Forest", false),
        ],
        false,
    );
    let mtg_compiler::CompileResult::Lowered { mut definition } = mtg_witness::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/24882fa2-3fe9-4c1b-aa3d-0e6488b9db27.json"),
    ) else {
        panic!("complete pinned permanent-grant source")
    };
    let program = definition.spell.as_mut().unwrap();
    let Instruction::GrantKeywordsSet {
        keywords: grants,
        modifier: None,
        ..
    } = &mut program.instructions[0]
    else {
        panic!("one pure set grant")
    };
    assert_eq!(grants.as_slice(), PROTECTION);
    if keywords != PROTECTION {
        *grants = keywords.to_vec();
    }
    if keywords != PROTECTION {
        definition.name = "Synthetic typed permanent-set keyword control".into();
        definition.capabilities = definition.requirements();
    }
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    // Owner and controller are independent: both borrowed recipients qualify.
    p.config.cards[1].owner = PlayerId(2);
    p.config.cards[2].owner = PlayerId(3);
    p.config.cards[5].controller = PlayerId(1);
    let mut c = (*p.catalog).clone();
    c.definitions.push(*definition);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    p
}

fn resolving(g: &mut Game) -> Resolving {
    for _ in 0..2000 {
        if let Frame::Resolve { frame } = &g.state.rules.frame
            && frame.cursor == 0
        {
            return frame.clone();
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            other => panic!("bounded kernel resolving prefix: {other:?}"),
        }
    }
    panic!("kernel resolving bound")
}

fn grant(keywords: &[KeywordAbility]) -> (Game, ResolvedEffect, BatchCause) {
    let mut g = Game::with_trace(prepared(keywords), TraceMode::Detailed);
    request(&mut g, 1);
    announce(&mut g, 1, 0, vec![]);
    let frame = resolving(&mut g);
    let targets = g.objects_in_set(
        &ObjectSet::ControllerPermanents,
        g.entry(frame.stack_id).unwrap().controller,
    );
    assert_eq!(targets, [1, 2, 3, 4, 7].map(|id| object(&g, id)));
    let mut target_handles = vec![];
    for target in &targets {
        target_handles.extend(g.capture_trigger_source_handles(*target).unwrap());
    }
    let effect = ResolvedEffect {
        id: EffectId(g.state.rules.next_effect_id),
        origin: EffectOrigin {
            activated_ability: None,
            mode: None,
            stack_id: frame.stack_id,
            source: g.entry(frame.stack_id).unwrap().source,
            controller: g.entry(frame.stack_id).unwrap().controller,
            source_handles: g
                .entry(frame.stack_id)
                .unwrap()
                .effect_source_handles
                .clone(),
            cursor: frame.cursor,
            source_block: 0,
        },
        target_handles,
        expires_in_cleanup_of_turn: g.state.rules.turn.number,
        kind: ResolvedEffectKind::GrantKeywordsSet {
            targets,
            modifier: None,
            keywords: keywords.to_vec(),
        },
    };
    let cause = g.instruction_cause(&frame, 0).unwrap();
    (g, effect, cause)
}

fn event(effect: ResolvedEffect) -> ProposedEvent {
    ProposedEvent::CreateEffect {
        effect: Box::new(effect),
    }
}

#[test]
fn permanent_set_kernel_complete_mixed_capture_and_all_historical_handles_are_atomic() {
    let (g, effect, cause) = grant(PROTECTION);
    for mutation in 0..13 {
        let mut bad = effect.clone();
        let ResolvedEffectKind::GrantKeywordsSet {
            targets, keywords, ..
        } = &mut bad.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                targets.reverse();
                bad.target_handles.reverse();
            }
            1 => {
                targets.pop();
                bad.target_handles.truncate(targets.len() * 4);
            }
            2..=5 => {
                let replacement = object(
                    &g,
                    match mutation {
                        2 => 1, // duplicate
                        3 => 5, // another controller
                        4 => 6, // hand, not battlefield
                        _ => 8, // library, not battlefield
                    },
                );
                targets[1] = replacement;
                // Even coherent handles cannot authorize the wrong selection.
                for viewer in 0..4 {
                    bad.target_handles[4 + viewer] = g
                        .handle_for(PlayerId(viewer as u8), replacement)
                        .unwrap_or_else(|_| bad.target_handles[4 + viewer].clone());
                }
            }
            6 => bad.target_handles.swap(0, 4),
            7 => bad.origin.controller = PlayerId(1),
            8 => bad.origin.source_block += 1,
            9 => bad.origin.source_handles.swap(0, 1),
            10 => keywords.reverse(),
            11 => targets[1].incarnation += 1,
            _ => bad.id.0 += 1,
        }
        let mut fork = g.fork();
        let before = fork.state.clone();
        let trace = fork.trace.clone();
        assert!(
            fork.commit(event(bad), cause.clone()).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(fork.state, before);
        assert_eq!(fork.trace, trace);
    }
    let mut g = g;
    let before = g.state.clone();
    let trace = g.trace.clone();
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
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    g.commit(event(effect.clone()), cause).unwrap();
    assert_eq!(g.state.rules.effects, vec![effect.clone()]);
    assert_eq!(g.state.rules.next_effect_id, effect.id.0 + 1);
    g.validate_effect_state().unwrap();
    for id in [1, 2, 3, 4, 7] {
        for keyword in PROTECTION {
            assert!(g.has_keyword(object(&g, id), *keyword));
        }
    }
    for id in [0, 5, 6, 8] {
        assert!(!g.has_keyword(object(&g, id), KeywordAbility::Hexproof));
    }
    let ResolvedEffectKind::GrantKeywordsSet { targets, .. } = &effect.kind else {
        unreachable!()
    };
    // PumpSet and ExileDamaged retain the historical creature-only guard.
    assert!(!g.valid_set_effect_targets(&effect, targets, None));
    assert!(!g.valid_set_effect_targets(&effect, targets, Some(&ObjectSet::ControllerCreatures)));
}

#[test]
fn permanent_set_kernel_historical_control_owner_departure_and_incarnation_do_not_reselect() {
    let (mut g, effect, cause) = grant(PROTECTION);
    let captured = object(&g, 2);
    g.commit(event(effect), cause).unwrap();
    g.state
        .rules
        .objects
        .get_mut(&CardId(2))
        .unwrap()
        .controller = PlayerId(1);
    g.state
        .rules
        .objects
        .get_mut(&CardId(5))
        .unwrap()
        .controller = PlayerId(0);
    g.validate_effect_state().unwrap();
    assert!(g.has_keyword(captured, KeywordAbility::Hexproof));
    assert!(!g.has_keyword(object(&g, 5), KeywordAbility::Hexproof));
    // Direct incarnation changes are a structural probe, not legal gameplay.
    let o = g.state.rules.objects.get_mut(&CardId(2)).unwrap();
    o.id.incarnation += 1;
    let successor = o.id;
    assert!(!g.has_keyword(captured, KeywordAbility::Hexproof));
    assert!(!g.has_keyword(successor, KeywordAbility::Hexproof));
    g.validate_effect_state().unwrap();
    // Direct structural mutation bypasses advance's publication boundary.
    // Refresh that boundary before reading views; queries never repair handles.
    g.refresh_visibility().unwrap();
    for viewer in 0..4 {
        assert!(g.handle_for(PlayerId(viewer), captured).is_err());
        assert!(g.handle_for(PlayerId(viewer), successor).is_ok());
        let visible = g.visible_effects(PlayerId(viewer)).unwrap();
        let VisibleEffect::GrantKeywordsSet {
            targets,
            modifier: None,
            ..
        } = &visible[0]
        else {
            panic!("shared pure set projection")
        };
        assert!(targets[1].current.is_none());
        assert_eq!(
            targets[1].announced,
            g.state.rules.effects[0].target_handles[4 + viewer as usize]
        );
    }
    // Historical recipients may be absent after their owner leaves. Their
    // physical identity and original capture persist until cleanup.
    g.state.rules.objects.remove(&CardId(1));
    g.validate_effect_state().unwrap();
    let mut bad = g.fork();
    let ResolvedEffectKind::GrantKeywordsSet { targets, .. } = &mut bad.state.rules.effects[0].kind
    else {
        unreachable!()
    };
    targets[0].card = CardId(u32::MAX);
    assert!(bad.validate_effect_state().is_err());
}

#[test]
fn permanent_set_kernel_pure_modifier_none_and_shared_wire_shape_reject_forgery() {
    let (g, effect, cause) = grant(PROTECTION);
    let encoded = serde_json::to_value(&effect.kind).unwrap();
    assert_eq!(encoded["effect"], "grant_keywords_set");
    assert!(encoded.get("modifier").is_none());
    assert_eq!(
        strict_read::<ResolvedEffectKind>(&encoded.to_string()).unwrap(),
        effect.kind
    );
    for field in ["set", "timestamp", "permanent_targets"] {
        let mut bad = encoded.clone();
        bad[field] = serde_json::json!("controller_permanents");
        assert!(strict_read::<ResolvedEffectKind>(&bad.to_string()).is_err());
    }
    for (power, toughness) in [(0, 0), (1, 0), (-1, 1)] {
        let mut bad = effect.clone();
        let ResolvedEffectKind::GrantKeywordsSet { modifier, .. } = &mut bad.kind else {
            unreachable!()
        };
        *modifier = Some(CreatureStatModifier {
            power: power.into(),
            toughness: toughness.into(),
        });
        let mut creation = g.fork();
        let before = creation.state.clone();
        assert!(creation.commit(event(bad.clone()), cause.clone()).is_err());
        assert_eq!(creation.state, before);
        let mut history = g.fork();
        history
            .commit(event(effect.clone()), cause.clone())
            .unwrap();
        history.state.rules.effects[0] = bad;
        assert!(history.validate_effect_state().is_err());
    }
}

#[test]
fn permanent_set_kernel_single_instruction_projects_pure_grant_and_retains_scalar_creature_gate() {
    let (mut g, effect, _) = grant(PROTECTION);
    let Frame::Resolve { frame } = &g.state.rules.frame else {
        unreachable!()
    };
    let frame = frame.clone();
    let program = g.program_for(g.entry(frame.stack_id).unwrap()).unwrap();
    assert_eq!(program.instructions.len(), 1);
    let instruction = program.instructions[0].clone();
    let before: Vec<_> = [1, 2, 3, 4, 7]
        .map(|id| g.object_before(object(&g, id)).unwrap())
        .into();
    g.execute_wipes_instruction(frame, instruction).unwrap();
    assert_eq!(g.state.rules.effects, vec![effect]);
    let Frame::Resolve { frame } = &g.state.rules.frame else {
        unreachable!()
    };
    assert_eq!(frame.cursor, 1);
    assert!(matches!(
        frame.outcomes.as_slice(),
        [InstructionOutcome::Effect { actual: Some(_) }]
    ));
    for old in before {
        let current = g.object_before(old.object).unwrap();
        assert_eq!(current.characteristics, old.characteristics);
        assert_eq!(current.keywords, PROTECTION);
    }
    g.refresh_effect_handles().unwrap();
    for viewer in 0..4 {
        let visible = g.visible_effects(PlayerId(viewer)).unwrap();
        let [
            VisibleEffect::GrantKeywordsSet {
                targets,
                modifier: None,
                keywords,
                ..
            },
        ] = visible.as_slice()
        else {
            panic!("one shared pure public set effect")
        };
        assert_eq!(targets.len(), 5);
        assert_eq!(keywords, PROTECTION);
        assert!(targets.iter().all(|target| target.current.is_some()));
    }
    // Deliberately invalid synthetic scalar effects test the old derivation
    // fence in isolation. They cannot be produced by a validated source.
    for id in [2, 3, 4] {
        let mut synthetic = g.fork();
        synthetic.state.rules.effects[0].kind = ResolvedEffectKind::GrantKeywords {
            target: object(&g, id),
            modifier: None,
            keywords: PROTECTION.to_vec(),
        };
        for keyword in PROTECTION {
            assert!(!synthetic.has_keyword(object(&g, id), *keyword));
        }
    }
}

fn damage_transaction(keywords: &[KeywordAbility], departed: bool) -> Game {
    let (mut g, _, _) = grant(keywords);
    let Frame::Resolve { frame } = &g.state.rules.frame else {
        unreachable!()
    };
    let frame = frame.clone();
    let instruction = g
        .program_for(g.entry(frame.stack_id).unwrap())
        .unwrap()
        .instructions[0]
        .clone();
    g.execute_wipes_instruction(frame, instruction).unwrap();
    request(&mut g, 1);
    let source = object(&g, 4);
    answer(
        &mut g,
        1,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    loop {
        let r = request(&mut g, 1);
        let response = match &r {
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: vec![TargetEntity::Player {
                    player: PlayerId(1),
                }],
            },
            Request::Targets { .. } => panic!("Bombardment uses entity targets"),
            Request::SacrificeCost { .. } => Response::SacrificeCost {
                object: object(&g, 1),
            },
            Request::Priority { .. } => break,
            other => panic!("kernel activation prefix {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    resolving(&mut g);
    if departed {
        g.move_object(source, Zone::Graveyard { owner: PlayerId(0) })
            .unwrap();
    }
    let Frame::Resolve { frame } = &g.state.rules.frame else {
        unreachable!()
    };
    let frame = frame.clone();
    let facts = g.source_facts(g.entry(frame.stack_id).unwrap()).unwrap();
    assert_eq!(facts.object, source);
    assert_eq!(facts.zone, Zone::Battlefield);
    assert_eq!(facts.characteristics.kind, CardKind::Enchantment);
    assert_eq!(facts.keywords, keywords);
    g.prepare_noncombat_damage(
        frame,
        NoncombatDamageOccurrence {
            source: facts,
            controller: PlayerId(0),
            recipient: DamageRecipient::Player {
                player: PlayerId(1),
            },
            amount: 1.into(),
        },
        0,
    )
    .unwrap();
    g
}

#[test]
fn permanent_set_kernel_synthetic_all_eight_keywords_validate_current_and_departed_noncreature_lki()
{
    for keyword in REVIEWED {
        for departed in [false, true] {
            let mut g = damage_transaction(&[*keyword], departed);
            let tx = g.damage_transaction().unwrap().clone();
            g.validate_damage_transaction(&tx).unwrap();
            assert_eq!(g.current(tx.entries[0].source.object).is_err(), departed);
            for mutation in 0..4 {
                let mut bad = tx.clone();
                match mutation {
                    0 => bad.entries[0].source.keywords.push(*keyword),
                    1 => bad.entries[0].source.keywords.push(KeywordAbility::Flash),
                    2 => bad.entries[0].source.zone = Zone::Graveyard { owner: PlayerId(0) },
                    _ => bad.entries[0].source.owner = PlayerId(1),
                }
                assert!(
                    g.validate_damage_transaction(&bad).is_err(),
                    "{keyword:?} mutation {mutation}"
                );
            }
            let before = g.state.rules.players[0].life.clone();
            g.damage_effect_step().unwrap();
            assert_eq!(g.state.rules.players[1].life, 39.into());
            assert_eq!(
                g.state.rules.players[0].life,
                if *keyword == KeywordAbility::Lifelink {
                    41.into()
                } else {
                    before
                }
            );
        }
    }
}

#[test]
fn permanent_set_kernel_synthetic_creature_set_vocabulary_cannot_authorize_noncreature_lki() {
    for keyword in REVIEWED {
        let g = damage_transaction(&[*keyword], true);
        let tx = g.damage_transaction().unwrap().clone();
        g.validate_damage_transaction(&tx).unwrap();
        let mut g = g;
        let mut prepared = g.prepared.as_ref().clone();
        let mut catalog = (*prepared.catalog).clone();
        let definition = catalog
            .definitions
            .iter_mut()
            .find(|d| {
                d.spell
                    .as_ref()
                    .and_then(temporary_permanent_set_keyword_program)
                    .is_some()
            })
            .unwrap();
        let Instruction::GrantKeywordsSet { set, .. } =
            &mut definition.spell.as_mut().unwrap().instructions[0]
        else {
            unreachable!()
        };
        *set = ObjectSet::ControllerCreatures;
        definition.capabilities = definition.requirements();
        catalog.validate().unwrap();
        prepared.catalog_digest = digest(&catalog).unwrap();
        prepared.catalog = Arc::new(catalog);
        g.prepared = Arc::new(prepared);
        assert!(
            g.validate_damage_transaction(&tx)
                .unwrap_err()
                .contains("keyword LKI")
        );
    }
}
