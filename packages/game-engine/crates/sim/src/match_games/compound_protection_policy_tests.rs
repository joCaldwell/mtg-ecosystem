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
    assert_eq!(p.instructions.len(), 2);
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
    for n in 0..10 {
        let mut b = original.clone();
        let p = b.spell.as_mut().unwrap();
        match n {
            0 => p.instructions.reverse(),
            1 => {
                if let I::ModifyPowerToughness { power, .. } = &mut p.instructions[0] {
                    *power = 2.into();
                }
            }
            2 => {
                if let I::ModifyPowerToughness { toughness, .. } = &mut p.instructions[0] {
                    *toughness = 0.into();
                }
            }
            3 => {
                if let I::GrantKeywords { source_block, .. } = &mut p.instructions[1] {
                    *source_block = 1;
                }
            }
            4 => {
                if let I::GrantKeywords { target_slot, .. } = &mut p.instructions[1] {
                    *target_slot = 1;
                }
            }
            5 => {
                if let I::GrantKeywords { keywords, .. } = &mut p.instructions[1] {
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
            _ => unreachable!(),
        }
        assert!(
            !compound_indestructible_spell(&b, b.spell.as_ref().unwrap()),
            "case {n}"
        );
    }
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
