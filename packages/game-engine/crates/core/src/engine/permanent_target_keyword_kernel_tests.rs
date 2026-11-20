//! Synthetic low-level kernel probes, NOT paid casts or accepted-history proof.
//! Complete frozen Tamiyo source supplies the program; private frames, control,
//! departures and catalog mutations below deliberately bypass normal lifecycle.
//! Every test is UNRUN; parent owns native/replay and actual-paid qualification.
//! Archive d705222b280a71cb, physical 28168 / index 28166, record
//! 914e134284083b1540c71d66f115424264560de04ee18674b39f0270daa836cd.
//! CR20260925 109.5, 400.7, 608.2b/c, 611.2c, 613.1f, 702.11/12:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{object, request};
use super::*;

const PAIR: &[KeywordAbility] = &[KeywordAbility::Hexproof, KeywordAbility::Indestructible];
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
const ORACLE: &str = "bb2b324c-970a-4920-884e-c92ba49669f0";

fn prepared(keywords: &[KeywordAbility]) -> Prepared {
    let mut p = equipped_indestructible_tests::prepared(
        &[
            ("Giant Growth", true),
            ("Sol Ring", false),
            ("Forest", false),
            ("Grizzly Bears", false),
            ("Goblin Bombardment", false),
            ("Forest", true),
        ],
        false,
    );
    let mtg_compiler::CompileResult::Lowered { mut definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/bb2b324c-970a-4920-884e-c92ba49669f0.json"),
    ) else {
        panic!("complete pinned Tamiyo source")
    };
    assert_eq!(definition.oracle_id, ORACLE);
    assert_eq!(definition.characteristics.kind, CardKind::Instant);
    assert_eq!(definition.characteristics.mana_cost.generic, 0.into());
    assert_eq!(definition.characteristics.mana_cost.colored, [Color::G]);
    assert_eq!(definition.provenance.source_index, 28166);
    assert_eq!(
        definition.provenance.source_sha256,
        "48e40697b4ba35ab71ae951b900f756eeeb6b0ab5d3a9e4ca6434326474e0a78"
    );
    assert_eq!(
        definition.provenance.ast_sha256,
        "050500699479157305337569bca1c1d753142907cc8ba254403d7ecc75053b86"
    );
    assert_eq!(
        definition.provenance.binding_sha256,
        "2ecd1544ded73bedd155b56966a2e3d3886b3c83288e58f5081a5c0ae85a5ec6"
    );
    let p0 = definition.spell.as_mut().unwrap();
    let descriptor = temporary_permanent_keyword_program(p0).unwrap();
    assert_eq!(descriptor.keywords, PAIR);
    assert_eq!(descriptor.controller_life, Some(&Quantity::from(2)));
    assert!(descriptor.modifier.is_none());
    assert!(temporary_creature_keyword_program(p0).is_none());
    if keywords != PAIR {
        let Instruction::GrantKeywords {
            keywords: grants, ..
        } = &mut p0.instructions[0]
        else {
            unreachable!()
        };
        *grants = keywords.to_vec();
        definition.name = "Synthetic scalar permanent keyword vector".into();
        definition.capabilities = definition.requirements();
    }
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    p.config.cards[1].owner = PlayerId(2); // borrowed artifact is controlled by caster
    p.config.cards[3].controller = PlayerId(1); // owned creature is not controlled
    let mut catalog = (*p.catalog).clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(&catalog).unwrap();
    p.catalog = Arc::new(catalog);
    p.profile_digest = digest(&p.config).unwrap();
    p
}

// Intentionally no cast/payment helper and no paid field fabricated. This
// resolving frame is only an input to source/target/event kernel validators.
fn kernel(keywords: &[KeywordAbility], id: u32) -> (Game, ResolvedEffect, BatchCause) {
    let mut g = Game::with_trace(prepared(keywords), TraceMode::Detailed);
    request(&mut g, 1);
    g.move_object(object(&g, 0), Zone::Stack).unwrap();
    let source = object(&g, 0);
    g.refresh_visibility().unwrap();
    let target = object(&g, id);
    let handles = g.capture_trigger_source_handles(target).unwrap();
    let spell = StackInstance {
        alternative_cost: None,
        hybrid_colors: vec![],
        hand_cast_permission: None,
        cost_lock: None,
        selected_additional_costs: None,
        x: None,
        death_controller: None,
        sacrificed: None,
        sacrificed_set: vec![],
        mode_subset: None,
        mode: None,
        effect_source_handles: g.capture_trigger_source_handles(source).unwrap(),
        source_facts: None,
        entity_targets: vec![],
        announced_entity_targets: vec![],
        stack_targets: vec![],
        announced_stack_targets: vec![],
        kind: StackKind::Spell,
        player_targets: vec![],
        id: StackId(g.state.rules.next_stack_id),
        source,
        controller: PlayerId(0),
        targets: vec![target],
        announced_targets: handles.iter().cloned().map(|h| vec![h]).collect(),
        paid: None,
        total_cost: g
            .definition(source.card)
            .unwrap()
            .characteristics
            .mana_cost
            .clone(),
        cast_from: Zone::Hand { owner: PlayerId(0) },
    };
    g.state.rules.next_stack_id += 1;
    g.state.rules.stack.push(spell.clone());
    let frame = Resolving {
        stack_id: spell.id,
        legal_targets: g.announced_target_legality(&spell),
        spell,
        cursor: 0,
        target_facts: vec![None],
        outcomes: vec![],
        group_discard: None,
        each_player_draw: None,
        random_discard: None,
        entrant_draw: None,
        draw_progress: None,
        set_damage: None,
        condition_check: None,
    };
    assert_eq!(frame.legal_targets, [true]);
    let effect = ResolvedEffect {
        id: EffectId(g.state.rules.next_effect_id),
        origin: EffectOrigin {
            activated_ability: None,
            mode: None,
            stack_id: frame.spell.id,
            source,
            controller: PlayerId(0),
            source_handles: frame.spell.effect_source_handles.clone(),
            cursor: 0,
            source_block: 0,
        },
        target_handles: handles,
        expires_in_cleanup_of_turn: g.state.rules.turn.number,
        kind: ResolvedEffectKind::GrantKeywords {
            target,
            modifier: None,
            keywords: keywords.to_vec(),
        },
    };
    let cause = Game::wipe_cause(&frame, 0);
    g.state.rules.frame = Frame::Resolve { frame };
    (g, effect, cause)
}
fn event(effect: ResolvedEffect) -> ProposedEvent {
    ProposedEvent::CreateEffect {
        effect: Box::new(effect),
    }
}
fn frame(g: &Game) -> Resolving {
    let Frame::Resolve { frame } = &g.state.rules.frame else {
        panic!("kernel frame")
    };
    frame.clone()
}
fn instruction(g: &Game) -> Instruction {
    let f = frame(g);
    g.program_for(&f.spell).unwrap().instructions[f.cursor as usize].clone()
}
fn edit_program(g: &mut Game, edit: impl FnOnce(&mut SpellProgram)) {
    let mut p = g.prepared.as_ref().clone();
    let mut c = (*p.catalog).clone();
    edit(
        c.definitions
            .iter_mut()
            .find(|d| d.oracle_id == ORACLE)
            .unwrap()
            .spell
            .as_mut()
            .unwrap(),
    );
    // Deliberately do not validate forged catalogs: validators must not derive
    // permanent authority from an unrelated instruction or capability label.
    p.catalog = Arc::new(c);
    g.prepared = Arc::new(p);
}

#[test]
fn permanent_target_kernel_control_type_zone_incarnation_targetability_and_preflight() {
    let (g, _, _) = kernel(PAIR, 1);
    let entry = frame(&g).spell;
    let p = g.program_for(&entry).unwrap();
    assert!(g.has_target_domain(
        p,
        PlayerId(0),
        g.targeting_characteristics(&entry).unwrap().as_ref()
    ));
    for id in [1, 2, 4] {
        assert!(g.object_target_legal(&entry, object(&g, id)));
    }
    for id in [0, 3, 5, 6] {
        assert!(!g.object_target_legal(&entry, object(&g, id)));
    }
    let mut stale = object(&g, 1);
    stale.incarnation += 1;
    assert!(!g.object_target_legal(&entry, stale));
    let mut changed = g.fork();
    changed
        .state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .controller = PlayerId(1);
    assert!(!changed.object_target_legal(&entry, object(&changed, 1)));
    assert_eq!(changed.announced_target_legality(&entry), [false]);
    let mut blocked = g.fork();
    let mut p = blocked.prepared.as_ref().clone();
    let mut c = (*p.catalog).clone();
    let oracle = blocked.definition(CardId(1)).unwrap().oracle_id.clone();
    c.definitions
        .iter_mut()
        .find(|d| d.oracle_id == oracle)
        .unwrap()
        .keywords
        .push(KeywordAbility::Shroud);
    p.catalog = Arc::new(c);
    blocked.prepared = Arc::new(p);
    assert!(!blocked.object_target_legal(&entry, object(&blocked, 1)));
    // Normal plain-permanent filters still ignore ownership and controller.
    let mut spec = p_target(&g);
    spec.domain = TargetDomain::BattlefieldPermanent;
    assert!(changed.permanent_target_legal(object(&changed, 1), PlayerId(0), &spec));
}
fn p_target(g: &Game) -> TargetSpec {
    g.program_for(&frame(g).spell).unwrap().targets[0].clone()
}

#[test]
fn permanent_target_kernel_creation_facts_and_whole_ordered_rollback() {
    let (g, effect, cause) = kernel(PAIR, 1);
    for mutation in 0..15 {
        let mut bad = effect.clone();
        match mutation {
            0 => bad.origin.source_block += 1,
            1 => bad.origin.cursor += 1,
            2 => bad.origin.controller = PlayerId(1),
            3 => bad.origin.source.incarnation += 1,
            4 => bad.origin.stack_id.0 += 1,
            5 => bad.origin.mode = Some(0),
            6 => bad.expires_in_cleanup_of_turn += 1,
            7 => bad.target_handles.swap(0, 1),
            8 => bad.origin.source_handles.swap(0, 1),
            9 => bad.id.0 += 1,
            _ => {
                let ResolvedEffectKind::GrantKeywords {
                    target,
                    modifier,
                    keywords,
                } = &mut bad.kind
                else {
                    unreachable!()
                };
                match mutation {
                    10 => target.incarnation += 1,
                    11 => *target = object(&g, 3),
                    12 => keywords.reverse(),
                    13 => keywords.push(KeywordAbility::Hexproof),
                    _ => {
                        *modifier = Some(CreatureStatModifier {
                            power: 0.into(),
                            toughness: 0.into(),
                        })
                    }
                }
            }
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
            cause,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: event(effect.clone())
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::GainLife {
                            player: PlayerId(0),
                            amount: 2.into()
                        }
                    },
                    EventGroup::Occurrence {
                        proposed: event(effect)
                    },
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}

#[test]
fn permanent_target_kernel_one_effect_then_separate_frozen_controller_life_and_wire() {
    let (mut g, effect, _) = kernel(PAIR, 1);
    let before = g.object_before(object(&g, 1)).unwrap();
    g.execute_effect_instruction(frame(&g), instruction(&g))
        .unwrap();
    assert_eq!(g.state.rules.effects, [effect.clone()]);
    assert_eq!(frame(&g).cursor, 1);
    assert_eq!(g.state.rules.players[0].life, 40.into());
    assert_eq!(
        g.object_before(object(&g, 1)).unwrap().characteristics,
        before.characteristics
    );
    // Captured grant survives control change. The life tail uses invocation
    // controller 0, independently of target's new controller 1 and owner 2.
    g.state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .controller = PlayerId(1);
    g.execute_damage_life_instruction(frame(&g), instruction(&g))
        .unwrap();
    assert_eq!(g.state.rules.players[0].life, 42.into());
    assert_eq!(g.state.rules.players[1].life, 40.into());
    assert_eq!(g.state.rules.players[2].life, 40.into());
    assert_eq!(frame(&g).cursor, 2);
    assert_eq!(frame(&g).outcomes.len(), 2);
    g.validate_effect_state().unwrap();
    assert!(g.has_keyword(object(&g, 1), KeywordAbility::Indestructible));
    let encoded = serde_json::to_value(&effect.kind).unwrap();
    assert_eq!(
        encoded,
        serde_json::json!({"effect":"grant_keywords", "target":object(&g,1), "keywords":PAIR})
    );
    assert_eq!(
        strict_read::<ResolvedEffectKind>(&encoded.to_string()).unwrap(),
        effect.kind
    );
    g.refresh_effect_handles().unwrap();
    for v in 0..4 {
        let views = g.visible_effects(PlayerId(v)).unwrap();
        let [
            VisibleEffect::GrantKeywords {
                target,
                modifier: None,
                keywords,
                ..
            },
        ] = views.as_slice()
        else {
            panic!("legacy scalar projection")
        };
        assert_eq!(target.announced, effect.target_handles[v as usize]);
        assert!(target.current.is_some());
        assert_eq!(keywords, PAIR);
        assert!(
            serde_json::to_value(&views[0])
                .unwrap()
                .get("modifier")
                .is_none()
        );
    }
}

#[test]
fn permanent_target_kernel_history_departure_incarnation_and_provenance_fences() {
    let (mut g, effect, cause) = kernel(PAIR, 1);
    g.commit(event(effect.clone()), cause).unwrap();
    let captured = object(&g, 1);
    g.state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .controller = PlayerId(1);
    g.validate_effect_state().unwrap();
    assert!(g.has_keyword(captured, KeywordAbility::Hexproof));
    g.state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .id
        .incarnation += 1;
    assert!(!g.has_keyword(captured, KeywordAbility::Hexproof));
    assert!(!g.has_keyword(object(&g, 1), KeywordAbility::Hexproof));
    g.state.rules.objects.remove(&CardId(0));
    g.state.rules.objects.remove(&CardId(1));
    // Retire the synthetic continuation before publishing the departed-source
    // view; only the historical continuous effect remains under inspection.
    g.state.rules.stack.clear();
    g.state.rules.frame = Frame::Priority;
    g.validate_effect_state().unwrap();
    g.refresh_visibility().unwrap();
    for viewer in 0..4 {
        let visible = g.visible_effects(PlayerId(viewer)).unwrap();
        let [VisibleEffect::GrantKeywords { origin, target, .. }] = visible.as_slice() else {
            panic!("historical scalar projection")
        };
        assert!(origin.source.current.is_none());
        assert!(target.current.is_none());
        assert_eq!(target.announced, effect.target_handles[viewer as usize]);
    }
    for mutation in 0..7 {
        let mut bad = g.fork();
        edit_program(&mut bad, |p| match mutation {
            0 => p.targets[0].domain = TargetDomain::BattlefieldCreature,
            1 => {
                let Instruction::GrantKeywords { target_slot, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *target_slot = 1;
            }
            2 => p.instructions.swap(0, 1),
            3 => {
                let i = p.instructions[1].clone();
                p.instructions.push(i);
            }
            4 => p.targets[0].filter = None,
            5 => {
                let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                keywords.reverse();
            }
            _ => {
                let Instruction::GrantKeywords { modifier, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *modifier = Some(CreatureStatModifier {
                    power: 0.into(),
                    toughness: 0.into(),
                });
            }
        });
        assert!(
            bad.validate_effect_state().is_err(),
            "source forgery {mutation}"
        );
    }
}

#[test]
fn permanent_target_kernel_creation_rechecks_control_and_legacy_creature_grants_stay_ineffective() {
    let (mut g, effect, cause) = kernel(PAIR, 1);
    g.state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .controller = PlayerId(1);
    let before = g.state.clone();
    assert!(g.commit(event(effect.clone()), cause).is_err());
    assert_eq!(g.state, before);
    g.execute_effect_instruction(frame(&g), instruction(&g))
        .unwrap();
    assert!(g.state.rules.effects.is_empty());
    let (mut g, effect, _) = kernel(PAIR, 1);
    edit_program(&mut g, |p| {
        p.targets[0].domain = TargetDomain::BattlefieldCreature;
        p.targets[0].filter = None;
        p.instructions.truncate(1);
    });
    g.state.rules.effects.push(effect);
    assert!(!g.has_keyword(object(&g, 1), KeywordAbility::Hexproof));
    assert!(g.validate_effect_state().is_err());
}

#[test]
fn permanent_target_kernel_all_targets_invalid_skips_life_tail() {
    for change in 0..3 {
        let (mut g, _, _) = kernel(PAIR, 1);
        match change {
            0 => {
                g.state
                    .rules
                    .objects
                    .get_mut(&CardId(1))
                    .unwrap()
                    .controller = PlayerId(1)
            }
            1 => {
                g.state
                    .rules
                    .objects
                    .get_mut(&CardId(1))
                    .unwrap()
                    .id
                    .incarnation += 1;
            }
            _ => {
                g.state.rules.objects.get_mut(&CardId(1)).unwrap().zone = Zone::Exile;
            }
        }
        let mut f = frame(&g);
        f.legal_targets = g.announced_target_legality(&f.spell);
        assert_eq!(f.legal_targets, [false]);
        g.state.rules.frame = Frame::Resolve { frame: f };
        g.step().unwrap();
        assert!(g.state.rules.effects.is_empty());
        assert_eq!(g.state.rules.players[0].life, 40.into());
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(
            g.current(object(&g, 0)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
    }
}

// The grant above remains a synthetic kernel setup. Only this existing,
// ordinary sacrifice-creature activation supplies a real damage continuation;
// no sacrifice-mana, printed Tamiyo payment, or accepted-history claim follows.
fn noncreature_damage(
    keywords: &[KeywordAbility],
    departed: bool,
    source_controller: PlayerId,
) -> Game {
    use super::equipped_indestructible_tests::answer;
    use super::temporary_indestructible_tests::pass;
    let (mut g, _, _) = kernel(keywords, 4);
    g.execute_effect_instruction(frame(&g), instruction(&g))
        .unwrap();
    g.execute_damage_life_instruction(frame(&g), instruction(&g))
        .unwrap();
    g.step().unwrap(); // finish synthetic spell; no fabricated paid field
    g.state
        .rules
        .objects
        .get_mut(&CardId(3))
        .unwrap()
        .controller = PlayerId(0);
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
    for _ in 0..100 {
        let r = request(&mut g, 1);
        let response = match &r {
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: vec![TargetEntity::Player {
                    player: PlayerId(1),
                }],
            },
            Request::SacrificeCost { .. } => Response::SacrificeCost {
                object: object(&g, 3),
            },
            Request::Priority { .. } => break,
            other => panic!("existing activation prefix {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    let mut found = None;
    for _ in 0..2000 {
        if let Frame::Resolve { frame } = &g.state.rules.frame {
            found = Some(frame.clone());
            break;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            other => panic!("activation resolution {other:?}"),
        }
    }
    found.expect("bounded existing activation resolution");
    // A structural control probe, not a legal control-changing action: frozen
    // activation controller 0 differs from the damage source's controller.
    g.state
        .rules
        .objects
        .get_mut(&CardId(4))
        .unwrap()
        .controller = source_controller;
    g.retain_source_lki(source).unwrap();
    if departed {
        g.move_object(source, Zone::Graveyard { owner: PlayerId(0) })
            .unwrap();
    }
    let f = frame(&g);
    let facts = g.source_facts(&f.spell).unwrap();
    assert_eq!(facts.zone, Zone::Battlefield);
    assert_eq!(facts.characteristics.kind, CardKind::Enchantment);
    assert_eq!(facts.keywords, keywords);
    g.prepare_noncombat_damage(
        f,
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
fn permanent_target_kernel_synthetic_eight_keyword_noncreature_current_departed_lki() {
    for keyword in REVIEWED {
        for departed in [false, true] {
            let mut g = noncreature_damage(&[*keyword], departed, PlayerId(0));
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
                assert!(g.validate_damage_transaction(&bad).is_err());
            }
            g.damage_effect_step().unwrap();
            assert_eq!(g.state.rules.players[1].life, 39.into());
            assert_eq!(
                g.state.rules.players[0].life,
                if *keyword == KeywordAbility::Lifelink {
                    43.into()
                } else {
                    42.into()
                }
            );
        }
    }
}

#[test]
fn permanent_target_kernel_source_tail_or_capability_shape_cannot_bless_unrelated_lki() {
    // Haste is absent from the other catalog keyword grant sources. Keep the
    // real life tail and capability list while corrupting only source shape.
    let g = noncreature_damage(&[KeywordAbility::Haste], true, PlayerId(0));
    let tx = g.damage_transaction().unwrap().clone();
    g.validate_damage_transaction(&tx).unwrap();
    for mutation in 0..5 {
        let mut bad = g.fork();
        edit_program(&mut bad, |p| match mutation {
            0 => {
                p.targets[0].domain = TargetDomain::BattlefieldCreature;
                p.targets[0].filter = None;
            }
            1 => {
                let i = p.instructions[1].clone();
                p.instructions.push(i);
            }
            2 => p.instructions.swap(0, 1),
            3 => {
                let Instruction::GrantKeywords { modifier, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *modifier = Some(CreatureStatModifier {
                    power: 0.into(),
                    toughness: 0.into(),
                });
            }
            _ => {
                let Instruction::GainLife { recipient, .. } = &mut p.instructions[1] else {
                    unreachable!()
                };
                *recipient = LifeRecipient::EachOpponent;
            }
        });
        assert!(
            bad.validate_damage_transaction(&tx)
                .unwrap_err()
                .contains("keyword LKI"),
            "source shape {mutation}"
        );
    }
}

#[test]
fn permanent_target_kernel_zero_tail_has_no_event_and_nonfamily_zero_stays_rejected() {
    let (mut g, _, _) = kernel(PAIR, 1);
    edit_program(&mut g, |p| {
        let Instruction::GainLife { amount, .. } = &mut p.instructions[1] else {
            unreachable!()
        };
        *amount = 0.into();
    });
    g.execute_effect_instruction(frame(&g), instruction(&g))
        .unwrap();
    let before = g.state.clone();
    let trace = g.trace.clone();
    g.execute_damage_life_instruction(frame(&g), instruction(&g))
        .unwrap();
    assert_eq!(frame(&g).cursor, 2);
    assert!(matches!(
        frame(&g).outcomes.last(),
        Some(InstructionOutcome::Life { actual: None })
    ));
    assert_eq!(g.state.rules.players, before.rules.players);
    assert_eq!(g.state.rules.effects, before.rules.effects);
    assert_eq!(g.trace, trace); // no event/trigger, only instruction completion
    let (mut bad, _, _) = kernel(PAIR, 1);
    edit_program(&mut bad, |p| {
        p.targets[0].domain = TargetDomain::BattlefieldCreature;
        p.targets[0].filter = None;
        let Instruction::GainLife { amount, .. } = &mut p.instructions[1] else {
            unreachable!()
        };
        *amount = 0.into();
    });
    let mut f = frame(&bad);
    f.cursor = 1;
    f.outcomes.push(InstructionOutcome::Effect { actual: None });
    bad.state.rules.frame = Frame::Resolve { frame: f };
    let before = bad.state.clone();
    let trace = bad.trace.clone();
    assert!(
        bad.execute_damage_life_instruction(frame(&bad), instruction(&bad))
            .is_err()
    );
    assert_eq!(bad.state, before);
    assert_eq!(bad.trace, trace);
    assert!(
        bad.commit(
            ProposedEvent::GainLife {
                player: PlayerId(0),
                amount: 0.into()
            },
            BatchCause::Fixture
        )
        .is_err()
    );
}

#[test]
fn permanent_target_kernel_damage_source_control_and_grant_life_recipient_are_distinct() {
    for departed in [false, true] {
        let mut g = noncreature_damage(&[KeywordAbility::Lifelink], departed, PlayerId(2));
        let tx = g.damage_transaction().unwrap().clone();
        assert_eq!(tx.entries[0].controller, PlayerId(0));
        assert_eq!(tx.entries[0].source.controller, PlayerId(2));
        assert_eq!(tx.entries[0].source.owner, PlayerId(0));
        assert_eq!(g.state.rules.players[0].life, 42.into());
        g.validate_damage_transaction(&tx).unwrap();
        g.damage_effect_step().unwrap();
        assert_eq!(g.state.rules.players[0].life, 42.into());
        assert_eq!(g.state.rules.players[1].life, 39.into());
        assert_eq!(g.state.rules.players[2].life, 41.into());
    }
}

#[test]
fn permanent_target_kernel_normal_target_requests_and_validation_preserve_queries() {
    let (mut g, _, _) = kernel(PAIR, 1);
    let f = frame(&g);
    g.state.rules.frame = Frame::Targets {
        cast: Casting {
            completion: CastCompletion::Priority,
            sacrifices: vec![],
            origin: AnnouncementOrigin::Spell,
            spending: None,
            spending_restricted: vec![],
            stack_id: f.spell.id,
            original_object: f.spell.source,
            prior_passes: vec![],
            mana_activations: vec![],
            undo_mana: false,
        },
    };
    let before = g.state.clone();
    let trace = g.trace.clone();
    let Request::Targets { candidates, .. } = g.decision_for(PlayerId(0)).unwrap().unwrap() else {
        panic!("normal target route")
    };
    assert_eq!(candidates, [1, 2, 4].map(|id| object(&g, id)));
    for id in [1, 2, 4] {
        assert!(
            g.validate_response(
                PlayerId(0),
                &Response::Targets {
                    objects: vec![object(&g, id)]
                }
            )
            .is_ok()
        );
    }
    for id in [0, 3, 5] {
        assert!(
            g.validate_response(
                PlayerId(0),
                &Response::Targets {
                    objects: vec![object(&g, id)]
                }
            )
            .is_err()
        );
    }
    assert!(g.decision_for(PlayerId(1)).unwrap().is_none());
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}

#[test]
fn permanent_target_kernel_optional_tail_and_history_shape_use_existing_expiry() {
    let (mut g, effect, _) = kernel(PAIR, 2);
    edit_program(&mut g, |p| p.instructions.truncate(1));
    assert!(
        temporary_permanent_keyword_program(g.program_for(&frame(&g).spell).unwrap())
            .unwrap()
            .controller_life
            .is_none()
    );
    g.execute_effect_instruction(frame(&g), instruction(&g))
        .unwrap();
    assert_eq!(g.state.rules.effects, [effect.clone()]);
    assert_eq!(frame(&g).cursor, 1);
    g.validate_effect_state().unwrap();
    for mutation in 0..7 {
        let mut bad = g.fork();
        let e = &mut bad.state.rules.effects[0];
        match mutation {
            0 => e.origin.cursor = 1,
            1 => e.origin.source_block = 1,
            2 => e.expires_in_cleanup_of_turn += 1,
            3 => {
                e.target_handles.pop();
            }
            4 => e.origin.source.card = CardId(u32::MAX),
            _ => {
                let ResolvedEffectKind::GrantKeywords {
                    target, modifier, ..
                } = &mut e.kind
                else {
                    unreachable!()
                };
                if mutation == 5 {
                    target.card = CardId(u32::MAX);
                } else {
                    *modifier = Some(CreatureStatModifier {
                        power: 0.into(),
                        toughness: 0.into(),
                    });
                }
            }
        }
        assert!(
            bad.validate_effect_state().is_err(),
            "historical shape {mutation}"
        );
    }
    assert!(g.has_keyword(object(&g, 2), KeywordAbility::Hexproof));
    g.commit(ProposedEvent::CleanupEffects, BatchCause::Cleanup)
        .unwrap();
    assert!(g.state.rules.effects.is_empty());
    assert!(!g.has_keyword(object(&g, 2), KeywordAbility::Hexproof));
    assert_eq!(g.state.rules.players[0].life, 40.into());
    // A recorded positive life change alone cannot create a keyword effect.
    let mut no_grant = g.fork();
    no_grant
        .commit(
            ProposedEvent::GainLife {
                player: PlayerId(0),
                amount: 2.into(),
            },
            BatchCause::Fixture,
        )
        .unwrap();
    assert_eq!(no_grant.state.rules.players[0].life, 42.into());
    assert!(!no_grant.has_keyword(object(&no_grant, 2), KeywordAbility::Hexproof));
}
