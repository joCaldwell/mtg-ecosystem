//! Synthetic public-view controls; the paid accepted-pod witness is separate.
use super::*;
use mtg_contracts::{Capability, KeywordAbility as K};
fn fixture() -> (Catalog, PermittedView) {
    let c = mtg_compiler::catalog::embedded_catalog("blocking-matches").unwrap();
    let mut v = wipe_tests::view(&mtg_compiler::catalog::embedded_catalog("wipes").unwrap());
    v.active = Some(PlayerId(0));
    v.turn_owner = PlayerId(0);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.objects = vec![
        wipe_tests::card(
            &c,
            "Mortal's Resolve",
            v.viewer.0,
            Zone::Hand { owner: v.viewer },
        ),
        wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Forest", v.viewer.0, Zone::Battlefield),
    ];
    stack_spell(&mut v, &c, "Swift Response", 0);
    v.objects[1].tapped = true;
    v.stack[0].targets = vec![mtg_core::VisibleTarget {
        announced: v.objects[1].handle.clone(),
        current: Some(v.objects[1].handle.clone()),
    }];
    v.objects[0] = wipe_tests::card(
        &c,
        "Mortal's Resolve",
        v.viewer.0,
        Zone::Hand { owner: v.viewer },
    );
    let mut extra = wipe_tests::card(&c, "Forest", v.viewer.0, Zone::Battlefield);
    extra.handle = ObjectHandle("second-Forest".into());
    v.objects.push(extra);
    (c, v)
}
// These are validated synthetic family parameters, not additional card sources
// or profile admission. Start from the frozen complete compound definition.
fn parameterized_fixture(power: i64, toughness: i64) -> (Catalog, PermittedView) {
    let (mut c, mut v) = fixture();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == v.objects[0].oracle_id)
        .unwrap();
    d.name = format!("Synthetic compound {power}/{toughness}");
    d.oracle_id = format!("synthetic-compound-{power}-{toughness}");
    let p = d.spell.as_mut().unwrap();
    let I::GrantKeywords {
        modifier:
            Some(mtg_contracts::CreatureStatModifier {
                power: p_bonus,
                toughness: t_bonus,
            }),
        ..
    } = &mut p.instructions[0]
    else {
        panic!("expected frozen compound modifier")
    };
    *p_bonus = power.into();
    *t_bonus = toughness.into();
    assert_eq!(
        compound_indestructible_parameters(p),
        Some((&power.into(), &toughness.into()))
    );
    d.validate().unwrap();
    v.objects[0].oracle_id = d.oracle_id.clone();
    v.objects[0].handle = ObjectHandle(d.name.clone());
    (c, v)
}
fn cast(v: &PermittedView) -> PlayerResponse {
    PlayerResponse::Cast {
        card: v.objects[0].handle.clone(),
    }
}
fn announce(v: &mut PermittedView) {
    let mut card = v.objects.remove(0);
    card.zone = Zone::Stack;
    let mut entry = v.stack.last().unwrap().clone();
    entry.handle = Some(StackHandle("synthetic-compound-announcement".into()));
    entry.spell = Some(card.handle.clone());
    entry.controller = v.viewer;
    entry.cast_complete = false;
    entry.targets.clear();
    v.objects.push(card);
    v.stack.push(entry);
}
fn preferred(c: &Catalog, v: &PermittedView, cs: Option<&[ObjectHandle]>) -> Option<ObjectHandle> {
    let d = definition(c, &v.objects[0].oracle_id).unwrap();
    let p = d.spell.as_ref().unwrap();
    strategy(c).object_target(v, d, p, &p.targets[0], cs)
}
#[test]
fn compound_policy_complete_ir_has_distinct_identity_and_two_mana_payment() {
    let (c, v) = fixture();
    let d = definition(&c, &v.objects[0].oracle_id).unwrap();
    let p = d.spell.as_ref().unwrap();
    assert!(compound_indestructible_spell(d, p));
    assert!(!temporary_indestructible_spell(d, p));
    assert_eq!(p.instructions.len(), 1);
    let before = canonical(&v).unwrap();
    let mut s = strategy(&c);
    assert_eq!(
        s.priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Cast {
            card: v.objects[0].handle.clone()
        }
    );
    assert_eq!(s.mana_plan.len(), 2);
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    for offered in [
        vec![],
        vec![ObjectHandle("stale".into())],
        vec![v.objects[2].handle.clone()],
    ] {
        assert_eq!(preferred(&c, &v, Some(&offered)), None);
    }
    assert_eq!(canonical(&v).unwrap(), before);
}
#[test]
fn compound_policy_no_proactive_enemy_protected_or_ineligible_benefit() {
    let (c, v) = fixture();
    for n in 0..13 {
        let mut x = v.clone();
        match n {
            0 => x.stack.clear(),
            1 => x.objects[1].keywords.push(K::Indestructible),
            2 => x.objects[1].controller = PlayerId(0),
            3 => x.objects[1].keywords.push(K::Shroud),
            4 => x.objects[1].keywords.push(K::Hexproof),
            5 => x.objects[1].tapped = false,
            6 => x.objects[1].zone = Zone::Hand { owner: x.viewer },
            7 => x.stack[0].targets[0].current = None,
            8 => x.stack[0].cast_complete = false,
            9 => x.stack[0].controller = x.viewer,
            10 => x.objects[1].handle = ObjectHandle("new-incarnation".into()),
            11 => x.objects[1].characteristics = None,
            12 => x.objects.last_mut().unwrap().tapped = true,
            _ => unreachable!(),
        }
        assert_eq!(
            strategy(&c).priority(&observation(x)).unwrap(),
            PlayerResponse::Pass,
            "case {n}"
        );
    }
}
#[test]
fn compound_policy_own_incomplete_announcement_skips_itself_and_fallback_stays_mandatory() {
    let (c, mut v) = fixture();
    let d = definition(&c, &v.objects[0].oracle_id).unwrap();
    let p = d.spell.as_ref().unwrap();
    let viewer = v.viewer;
    let target = v.objects[1].handle.clone();
    v.objects.remove(0);
    stack_spell(&mut v, &c, "Mortal's Resolve", viewer.0);
    v.stack.last_mut().unwrap().cast_complete = false;
    // An unrelated, more valuable legal candidate cannot hide a failed threat lookup.
    v.objects.push(wipe_tests::card(
        &c,
        "Yargle and Multani",
        viewer.0,
        Zone::Battlefield,
    ));
    let candidates = vec![v.objects.last().unwrap().handle.clone(), target.clone()];
    assert_eq!(
        strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&candidates)),
        Some(target.clone())
    );
    let mut o = observation(v);
    o.decision = Some(PlayerRequest::Targets {
        token: mtg_core::DecisionToken("compound".into()),
        player: viewer,
        occurrences: p.targets.clone(),
        candidates: vec![target.clone()],
    });
    assert_eq!(
        strategy(&c).choose(&o).unwrap(),
        PlayerResponse::Targets {
            objects: vec![target.clone()]
        }
    );
    o.view
        .objects
        .iter_mut()
        .find(|x| x.handle == target)
        .unwrap()
        .keywords
        .push(K::Indestructible);
    assert_eq!(
        strategy(&c).choose(&o).unwrap(),
        PlayerResponse::Targets {
            objects: vec![target]
        }
    );
}
#[test]
fn compound_policy_exact_shape_capability_and_identity_controls() {
    let (mut c, v) = fixture();
    let id = v.objects[0].oracle_id.clone();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == id)
        .unwrap();
    d.name = "Synthetic renamed compound".into();
    d.validate().unwrap();
    let original = d.clone();
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    // Invalid mutations are recognizer boundaries, never admitted card evidence.
    for n in 0..16 {
        let mut b = original.clone();
        let p = b.spell.as_mut().unwrap();
        match n {
            0 => {
                // A formerly split same-target program is no longer a complete effect.
                let I::GrantKeywords {
                    modifier: Some(m),
                    keywords,
                    duration,
                    source_block,
                    target_slot,
                } = p.instructions[0].clone()
                else {
                    unreachable!()
                };
                p.instructions = vec![
                    I::ModifyPowerToughness {
                        target_slot,
                        power: m.power,
                        toughness: m.toughness,
                        duration,
                        source_block,
                    },
                    I::GrantKeywords {
                        target_slot,
                        modifier: None,
                        keywords,
                        duration,
                        source_block,
                    },
                ];
            }
            1 => {
                if let I::GrantKeywords { target_slot, .. } = &mut p.instructions[0] {
                    *target_slot = 1;
                }
            }
            2 => {
                if let I::GrantKeywords { source_block, .. } = &mut p.instructions[0] {
                    *source_block = 1;
                }
            }
            3 => {
                if let I::GrantKeywords { keywords, .. } = &mut p.instructions[0] {
                    keywords.push(K::Indestructible);
                }
            }
            4 => {
                if let I::GrantKeywords { modifier, .. } = &mut p.instructions[0] {
                    *modifier = None;
                }
            }
            5 => {
                if let I::GrantKeywords { keywords, .. } = &mut p.instructions[0] {
                    *keywords = vec![K::Flying];
                }
            }
            6 => {
                b.capabilities
                    .remove(&Capability::CompoundIndestructibleSpellV1);
            }
            7 => p.targets[0].selection_id.clear(),
            8 => p.targets[0].domain = TargetDomain::ControlledBattlefieldCreature,
            9 => p.instructions.push(p.instructions[0].clone()),
            10 => p.targets[0].slot = 1,
            11 => {
                p.targets[0].filter = Some(mtg_contracts::PermanentFilter {
                    types: vec![],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                })
            }
            12 => p.functioning_zone = mtg_contracts::FunctioningZone::Battlefield,
            13 => p.resolution = SpellResolution::Permanent,
            14 => p.timing = SpellTiming::MainPhase,
            15 => b.characteristics.kind = CardKind::Sorcery,
            _ => unreachable!(),
        }
        // The shared contracts helper owns shape recognition. Capability and
        // definition/timing context remain additional policy requirements.
        assert_eq!(
            compound_indestructible_parameters(b.spell.as_ref().unwrap()).is_some(),
            matches!(n, 6 | 14 | 15),
            "shared shape case {n}"
        );
        assert!(
            !compound_indestructible_spell(&b, b.spell.as_ref().unwrap()),
            "case {n}"
        );
    }
    let mut detached = original.spell.as_ref().unwrap().clone();
    if let I::GrantKeywords {
        modifier: Some(mtg_contracts::CreatureStatModifier { power, .. }),
        ..
    } = &mut detached.instructions[0]
    {
        *power = 2.into();
    }
    assert!(compound_indestructible_parameters(&detached).is_some());
    assert!(!compound_indestructible_spell(&original, &detached));
}

#[test]
fn compound_policy_actual_combat_damage_and_unrelated_stack_controls() {
    let (c, mut v) = fixture();
    v.stack.clear();
    v.objects.retain(|o| o.zone != Zone::Stack);
    let mut enemy = wipe_tests::card(&c, "Yargle and Multani", 0, Zone::Battlefield);
    enemy.handle = ObjectHandle("opposing-attacker".into());
    let enemy_handle = enemy.handle.clone();
    v.objects.push(enemy);
    v.combat = vec![mtg_core::VisibleAttacker {
        attacker: enemy_handle.clone(),
        defender: v.viewer,
        blocked: true,
        blockers: vec![v.objects[1].handle.clone()],
    }];
    v.turn.step = TurnStep::DeclareBlockers;
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    for n in 0..7 {
        let mut x = v.clone();
        match n {
            0 => x.turn.step = TurnStep::CombatDamage,
            1 => x.combat[0]
                .blockers
                .push(ObjectHandle("ambiguous-blocker".into())),
            2 => x.objects.last_mut().unwrap().keywords.push(K::FirstStrike),
            3 => x.objects.last_mut().unwrap().keywords.push(K::DoubleStrike),
            4 => x.objects[1].keywords.push(K::Indestructible),
            5 => {
                x.objects
                    .last_mut()
                    .unwrap()
                    .characteristics
                    .as_mut()
                    .unwrap()
                    .power = Some(1.into())
            }
            6 => stack_spell(&mut x, &c, "Unsummon", 0),
            _ => unreachable!(),
        }
        assert_eq!(preferred(&c, &x, None), None, "combat control {n}");
    }
    v.combat.clear();
    stack_spell(&mut v, &c, "Lightning Bolt", 0);
    v.stack[0].entity_targets = vec![mtg_core::VisibleEntityTarget::Object {
        object: mtg_core::VisibleTarget {
            announced: v.objects[1].handle.clone(),
            current: Some(v.objects[1].handle.clone()),
        },
    }];
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    v.objects[1].characteristics.as_mut().unwrap().toughness = Some(4.into());
    assert_eq!(preferred(&c, &v, None), None);
    v.objects[1].marked_damage = 1.into();
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    v.effects
        .push(mtg_core::VisibleEffect::PreventCombatDamage {
            handle: mtg_core::EffectHandle("synthetic-prevention".into()),
            origin: mtg_core::VisibleEffectSource {
                source: mtg_core::VisibleTarget {
                    announced: enemy_handle,
                    current: None,
                },
                oracle_id: "synthetic".into(),
                controller: v.viewer,
            },
            expires_in_cleanup_of_turn: v.turn.number,
        });
    assert_eq!(preferred(&c, &v, None), None);
}

#[test]
fn compound_policy_unsupported_removal_does_not_become_a_threat() {
    let (c, v) = fixture();
    for name in ["Unsummon", "Time Ebb", "Disfigure", "Cruel Edict"] {
        let mut x = v.clone();
        x.stack.clear();
        x.objects.retain(|o| o.zone != Zone::Stack);
        stack_spell(&mut x, &c, name, 0);
        x.stack[0].targets = vec![mtg_core::VisibleTarget {
            announced: x.objects[1].handle.clone(),
            current: Some(x.objects[1].handle.clone()),
        }];
        assert_eq!(preferred(&c, &x, None), None, "unsupported threat {name}");
        assert_eq!(
            strategy(&c).priority(&observation(x)).unwrap(),
            PlayerResponse::Pass
        );
    }
}

#[test]
fn compound_policy_parameterized_validated_ir_preserves_complete_defensive_benefit() {
    for (power, toughness) in [(1, 1), (2, 0), (2, 2), (0, 0)] {
        let (mut c, mut v) = parameterized_fixture(power, toughness);
        let d = definition(&c, &v.objects[0].oracle_id).unwrap();
        let p = d.spell.as_ref().unwrap();
        assert!(compound_indestructible_spell(d, p));
        assert!(!temporary_indestructible_spell(d, p));
        assert_eq!(p.instructions.len(), 1);
        let before = canonical(&v).unwrap();
        let program_before = p.clone();
        let mut s = strategy(&c);
        assert_eq!(s.priority(&observation(v.clone())).unwrap(), cast(&v));
        assert_eq!(s.mana_plan.len(), 2);
        assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
        assert_eq!(canonical(&v).unwrap(), before);
        assert_eq!(*d.spell.as_ref().unwrap(), program_before);
        // Ownership does not override current friendly control.
        v.objects[1].owner = PlayerId(0);
        assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
        // The existing sole public creature-wipe threat also applies to every
        // parameter value; no new threat recognition is introduced.
        let wipes = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
        c.definitions.push(
            wipes
                .definitions
                .iter()
                .find(|d| d.name == "Day of Judgment")
                .unwrap()
                .clone(),
        );
        v.stack.clear();
        v.objects.retain(|o| o.zone != Zone::Stack);
        stack_spell(&mut v, &c, "Day of Judgment", 0);
        assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    }
}

#[test]
fn compound_policy_signed_family_recognition_does_not_initiate_harmful_bonuses() {
    for (power, toughness) in [(-1, 1), (1, -1), (-2, -2), (0, -5)] {
        let (c, v) = parameterized_fixture(power, toughness);
        let d = definition(&c, &v.objects[0].oracle_id).unwrap();
        assert!(compound_indestructible_spell(d, d.spell.as_ref().unwrap()));
        assert_eq!(preferred(&c, &v, None), None);
        assert_eq!(
            strategy(&c).priority(&observation(v)).unwrap(),
            PlayerResponse::Pass
        );
    }
}

#[test]
fn compound_policy_nonpositive_or_unknown_resulting_toughness_is_not_protected() {
    for (bonus, current) in [
        (0, Some(0)),
        (0, Some(-1)),
        (2, Some(-2)),
        (2, Some(-3)),
        (2, None),
    ] {
        let (c, mut v) = parameterized_fixture(2, bonus);
        v.objects[1].characteristics.as_mut().unwrap().toughness = current.map(Quantity::from);
        assert_eq!(preferred(&c, &v, None), None);
        assert_eq!(
            strategy(&c).priority(&observation(v)).unwrap(),
            PlayerResponse::Pass
        );
    }
}

#[test]
fn compound_policy_parameterized_defense_needs_original_public_threat_and_friendly_target() {
    for (power, toughness) in [(1, 1), (2, 0), (2, 2), (0, 0)] {
        let (c, v) = parameterized_fixture(power, toughness);
        for case in 0..8 {
            let mut x = v.clone();
            match case {
                0 => x.stack.clear(),
                1 => x.objects[1].keywords.push(K::Indestructible),
                2 => x.objects[1].controller = PlayerId(0),
                3 => x.objects[1].characteristics = None,
                4 => x.stack[0].targets[0].current = None,
                5 => x.stack[0].cast_complete = false,
                6 => {
                    let source = x.stack[0].spell.as_ref().unwrap().clone();
                    x.objects
                        .iter_mut()
                        .find(|o| o.handle == source)
                        .unwrap()
                        .oracle_id = "hidden-definition".into();
                }
                7 => x.objects[1].keywords.push(K::Shroud),
                _ => unreachable!(),
            }
            assert_eq!(
                preferred(&c, &x, None),
                None,
                "{power}/{toughness}, case {case}"
            );
            assert_eq!(
                strategy(&c).priority(&observation(x)).unwrap(),
                PlayerResponse::Pass,
                "{power}/{toughness}, case {case}"
            );
        }
    }
}

#[test]
fn compound_policy_parameterized_lethal_damage_includes_pump_alone_saves() {
    for (power, toughness) in [(1, 1), (2, 0), (2, 2), (0, 0)] {
        let (c, mut v) = parameterized_fixture(power, toughness);
        v.stack.clear();
        v.objects.retain(|o| o.zone != Zone::Stack);
        stack_spell(&mut v, &c, "Lightning Bolt", 0);
        v.stack[0].entity_targets = vec![mtg_core::VisibleEntityTarget::Object {
            object: mtg_core::VisibleTarget {
                announced: v.objects[1].handle.clone(),
                current: Some(v.objects[1].handle.clone()),
            },
        }];
        // Original 2 toughness faces 3 damage. In the +2/+2 case the pump
        // alone saves it; the complete spell is still useful defense.
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            cast(&v)
        );
        v.objects[1].characteristics.as_mut().unwrap().toughness = Some(4.into());
        assert_eq!(preferred(&c, &v, None), None, "originally nonlethal");
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            PlayerResponse::Pass
        );
        v.objects[1].marked_damage = 1.into();
        assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
        v.stack[0].entity_targets.clear();
        assert_eq!(preferred(&c, &v, None), None, "no inferred hidden target");
    }
}

#[test]
fn compound_policy_parameterized_one_on_one_combat_includes_pump_and_deathtouch_defense() {
    for (power, toughness) in [(1, 1), (2, 0), (2, 2), (0, 0)] {
        let (c, mut v) = parameterized_fixture(power, toughness);
        v.stack.clear();
        v.objects.retain(|o| o.zone != Zone::Stack);
        let mut enemy = wipe_tests::card(&c, "Grizzly Bears", 0, Zone::Battlefield);
        enemy.handle = ObjectHandle("synthetic-opposing-combatant".into());
        let enemy_handle = enemy.handle.clone();
        v.objects.push(enemy);
        let target = v.objects[1].handle.clone();
        v.turn.step = TurnStep::DeclareBlockers;
        v.combat = vec![mtg_core::VisibleAttacker {
            attacker: enemy_handle.clone(),
            defender: v.viewer,
            blocked: true,
            blockers: vec![target.clone()],
        }];
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            cast(&v)
        );
        // Same conservative protection for a friendly attacker already blocked.
        v.combat[0].attacker = target.clone();
        v.combat[0].defender = PlayerId(0);
        v.combat[0].blockers = vec![enemy_handle];
        assert_eq!(preferred(&c, &v, None), Some(target.clone()));
        v.objects
            .last_mut()
            .unwrap()
            .characteristics
            .as_mut()
            .unwrap()
            .power = Some(1.into());
        assert_eq!(preferred(&c, &v, None), None, "no original lethal threat");
        v.objects.last_mut().unwrap().keywords.push(K::Deathtouch);
        assert_eq!(preferred(&c, &v, None), Some(target));
    }
}

#[test]
fn compound_policy_parameterized_announcement_finishes_targets_and_planned_payment() {
    for (power, toughness) in [(1, 1), (2, 0), (2, 2), (0, 0)] {
        let (c, mut v) = parameterized_fixture(power, toughness);
        let d = definition(&c, &v.objects[0].oracle_id).unwrap();
        let p = d.spell.as_ref().unwrap();
        let target = v.objects[1].handle.clone();
        let mut s = strategy(&c);
        assert_eq!(s.priority(&observation(v.clone())).unwrap(), cast(&v));
        let sources: Vec<_> = s.mana_plan.iter().map(|m| m.source.clone()).collect();
        announce(&mut v);
        assert_eq!(
            s.object_target(&v, d, p, &p.targets[0], Some(&[target.clone()])),
            Some(target.clone())
        );
        // Preference may disappear after initiation; core's mandatory legal
        // candidates and the announced mana payment must still be completed.
        v.objects[0].keywords.push(K::Indestructible);
        let mut o = observation(v);
        o.decision = Some(PlayerRequest::Targets {
            token: mtg_core::DecisionToken("compound-family-target".into()),
            player: o.view.viewer,
            occurrences: p.targets.clone(),
            candidates: vec![target.clone()],
        });
        assert_eq!(
            s.choose(&o).unwrap(),
            PlayerResponse::Targets {
                objects: vec![target]
            }
        );
        let cost = d.characteristics.mana_cost.clone();
        o.decision = Some(PlayerRequest::Mana {
            token: mtg_core::DecisionToken("compound-family-mana".into()),
            player: o.view.viewer,
            cost: cost.clone(),
            mana_ability_source: None,
        });
        for source in sources {
            assert_eq!(
                s.choose(&o).unwrap(),
                PlayerResponse::ActivateMana {
                    object: source.clone()
                }
            );
            o.view
                .objects
                .iter_mut()
                .find(|x| x.handle == source)
                .unwrap()
                .tapped = true;
            o.view
                .players
                .iter_mut()
                .find(|p| p.player == o.view.viewer)
                .unwrap()
                .mana
                .entry(Color::G)
                .or_default()
                .add(&1.into());
        }
        let available = BTreeMap::from([(Color::G, 2.into())]);
        assert_eq!(
            o.view
                .players
                .iter()
                .find(|p| p.player == o.view.viewer)
                .unwrap()
                .mana,
            available
        );
        assert_eq!(
            s.choose(&o).unwrap(),
            PlayerResponse::FinishMana { undo_mana: true }
        );
        o.decision = Some(PlayerRequest::SpendMana {
            token: mtg_core::DecisionToken("compound-family-spend".into()),
            player: o.view.viewer,
            cost,
            available: available.clone(),
            mana_ability_source: None,
        });
        assert_eq!(
            s.choose(&o).unwrap(),
            PlayerResponse::SpendMana {
                spending: available
            }
        );
    }
}

#[test]
fn compound_policy_signed_or_unsafe_announced_program_keeps_legal_target_fallback() {
    for (power, toughness, current_toughness) in [(-1, 1, 2), (1, -1, 2), (0, 0, 0)] {
        let (c, mut v) = parameterized_fixture(power, toughness);
        let d = definition(&c, &v.objects[0].oracle_id).unwrap();
        let p = d.spell.as_ref().unwrap();
        v.objects[1].characteristics.as_mut().unwrap().toughness = Some(current_toughness.into());
        assert_eq!(preferred(&c, &v, None), None);
        let target = v.objects[1].handle.clone();
        announce(&mut v);
        let mut o = observation(v);
        o.decision = Some(PlayerRequest::Targets {
            token: mtg_core::DecisionToken("compound-signed-mandatory-target".into()),
            player: o.view.viewer,
            occurrences: p.targets.clone(),
            candidates: vec![target.clone()],
        });
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            PlayerResponse::Targets {
                objects: vec![target]
            }
        );
    }
}
