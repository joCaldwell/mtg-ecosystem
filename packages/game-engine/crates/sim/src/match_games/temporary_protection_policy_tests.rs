//! Synthetic permitted-view controls, separate from the paid legal-pod witness.
use super::*;
use mtg_contracts::KeywordAbility as K;
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
            "Withstand Death",
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
    (c, v)
}
fn preferred(
    c: &Catalog,
    v: &PermittedView,
    candidates: Option<&[ObjectHandle]>,
) -> Option<ObjectHandle> {
    let d = definition(c, &v.objects[0].oracle_id).unwrap();
    let p = d.spell.as_ref().unwrap();
    strategy(c).object_target(v, d, p, &p.targets[0], candidates)
}
fn cast(v: &PermittedView) -> PlayerResponse {
    PlayerResponse::Cast {
        card: v.objects[0].handle.clone(),
    }
}
#[test]
fn temporary_protection_policy_actual_ir_reacts_to_completed_destruction_and_current_candidates() {
    let (c, mut v) = fixture();
    let before = canonical(&v).unwrap();
    let mut s = strategy(&c);
    assert_eq!(s.priority(&observation(v.clone())).unwrap(), cast(&v));
    assert_eq!(s.mana_plan.len(), 1);
    assert_eq!(s.mana_plan[0].source, v.objects[2].handle);
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    for offered in [
        vec![],
        vec![ObjectHandle("stale".into())],
        vec![v.objects[2].handle.clone()],
    ] {
        assert_eq!(preferred(&c, &v, Some(&offered)), None);
    }
    assert_eq!(
        preferred(&c, &v, Some(&[v.objects[1].handle.clone()])),
        Some(v.objects[1].handle.clone())
    );
    assert_eq!(canonical(&v).unwrap(), before);
    v.objects[1].owner = PlayerId(0); // Current control remains friendly.
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    // A larger unthreatened creature must not steal protection.
    v.objects.push(wipe_tests::card(
        &c,
        "Yargle and Multani",
        v.viewer.0,
        Zone::Battlefield,
    ));
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
}
#[test]
fn temporary_protection_policy_declines_no_threat_protected_enemy_stale_and_ineligible() {
    let (c, v) = fixture();
    for n in 0..13 {
        let mut x = v.clone();
        match n {
            0 => x.stack.clear(),
            1 => x.objects[1].keywords.push(K::Indestructible),
            2 => x.objects[1].controller = PlayerId(0),
            3 => x.objects[1].keywords.push(K::Shroud),
            4 => x.objects[1].keywords.push(K::Hexproof), // Incoming caster cannot target it.
            5 => x.objects[1].tapped = false,             // Swift no longer threatens it.
            6 => x.objects[1].zone = Zone::Hand { owner: x.viewer },
            7 => x.stack[0].targets[0].current = None,
            8 => x.stack[0].cast_complete = false,
            9 => x.stack[0].controller = x.viewer,
            10 => x.objects[1].handle = ObjectHandle("new-incarnation".into()),
            11 => x.objects[1].characteristics = None,
            12 => x.objects[2].tapped = true,
            _ => unreachable!(),
        }
        assert_eq!(
            strategy(&c).priority(&observation(x.clone())).unwrap(),
            PlayerResponse::Pass,
            "case {n}"
        );
        if n != 12 {
            assert_eq!(preferred(&c, &x, None), None, "case {n}");
        }
    }
    let mut own_main = v;
    own_main.stack.clear();
    own_main.objects.retain(|o| o.zone != Zone::Stack);
    own_main.active = Some(own_main.viewer);
    own_main.turn_owner = own_main.viewer;
    assert_eq!(
        strategy(&c).priority(&observation(own_main)).unwrap(),
        PlayerResponse::Pass
    );
}
#[test]
fn temporary_protection_policy_announced_choice_skips_own_proposal_and_retains_legal_fallback() {
    let (c, mut v) = fixture();
    let viewer = v.viewer;
    // Move the actual hand source to stack with a distinct handle, as core does.
    let d = definition(&c, &v.objects[0].oracle_id).unwrap();
    let p = d.spell.as_ref().unwrap();
    v.objects.remove(0);
    stack_spell(&mut v, &c, "Withstand Death", viewer.0);
    v.stack.last_mut().unwrap().cast_complete = false;
    let h = ObjectHandle("Grizzly Bears".into());
    let mut o = observation(v.clone());
    o.decision = Some(PlayerRequest::Targets {
        token: mtg_core::DecisionToken("protection".into()),
        player: viewer,
        occurrences: p.targets.clone(),
        candidates: vec![h.clone()],
    });
    assert_eq!(
        strategy(&c).choose(&o).unwrap(),
        PlayerResponse::Targets {
            objects: vec![h.clone()]
        }
    );
    // Threat becomes ineligible after initiation: a mandatory legal target is
    // still chosen. Preference is not permission to abandon the announcement.
    o.view
        .objects
        .iter_mut()
        .find(|o| o.handle == h)
        .unwrap()
        .keywords
        .push(K::Indestructible);
    let before = canonical(&o).unwrap();
    assert_eq!(
        strategy(&c).choose(&o).unwrap(),
        PlayerResponse::Targets { objects: vec![h] }
    );
    assert_eq!(canonical(&o).unwrap(), before);
    v.stack.last_mut().unwrap().cast_complete = true;
    assert!(strategy(&c).reactive_spells(&v).unwrap().is_none());
}
#[test]
fn temporary_protection_policy_remaining_one_on_one_combat_is_conservative() {
    let (c, mut v) = fixture();
    v.stack.clear();
    v.objects.retain(|o| o.zone != Zone::Stack);
    let mut enemy = wipe_tests::card(&c, "Grizzly Bears", 0, Zone::Battlefield);
    enemy.handle = ObjectHandle("opposing-bears".into());
    v.objects.push(enemy);
    v.combat = vec![mtg_core::VisibleAttacker {
        attacker: v.objects[3].handle.clone(),
        defender: v.viewer,
        blocked: true,
        blockers: vec![v.objects[1].handle.clone()],
    }];
    for step in [TurnStep::DeclareBlockers, TurnStep::FirstStrikeDamage] {
        v.turn.step = step;
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            cast(&v)
        );
        // The same public 1:1 link protects a friendly attacker too.
        let mut attacking = v.clone();
        attacking.combat[0].attacker = v.objects[1].handle.clone();
        attacking.combat[0].blockers = vec![v.objects[3].handle.clone()];
        assert_eq!(
            preferred(&c, &attacking, None),
            Some(v.objects[1].handle.clone())
        );
    }
    for n in 0..10 {
        let mut x = v.clone();
        x.turn.step = TurnStep::DeclareBlockers;
        match n {
            0 => x.turn.step = TurnStep::CombatDamage,
            1 => x.turn.step = TurnStep::EndCombat,
            2 => x.turn.step = TurnStep::DeclareAttackers,
            3 => x.combat[0].blockers.clear(),
            4 => x.combat[0]
                .blockers
                .push(ObjectHandle("second-blocker".into())),
            5 => x.objects[3].keywords.push(K::FirstStrike),
            6 => x.objects[3].keywords.push(K::DoubleStrike),
            7 => x.objects[1].keywords.push(K::FirstStrike),
            8 => x.objects[3].characteristics.as_mut().unwrap().power = Some(1.into()),
            9 => x.objects[3].zone = Zone::Graveyard { owner: PlayerId(0) },
            _ => unreachable!(),
        }
        assert_eq!(preferred(&c, &x, None), None, "combat case {n}");
    }
    v.objects[3].characteristics.as_mut().unwrap().power = Some(1.into());
    v.objects[1].marked_damage = 1.into();
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    v.objects[1].marked_damage = 0.into();
    v.objects[3].keywords.push(K::Deathtouch);
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
}
#[test]
fn temporary_protection_policy_fixed_announced_lethal_damage_and_prevention_controls() {
    let (c, mut v) = fixture();
    v.stack.clear();
    v.objects.retain(|o| o.zone != Zone::Stack);
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
    let origin = mtg_core::VisibleEffectSource {
        source: mtg_core::VisibleTarget {
            announced: ObjectHandle("visible-prevention-source".into()),
            current: None,
        },
        oracle_id: "synthetic-permitted-effect".into(),
        controller: v.viewer,
    };
    // Existing permitted effects provide enough information to decline rather
    // than invent a replacement/prevention ordering or a residual damage sum.
    for effect in [
        mtg_core::VisibleEffect::PreventCombatDamage {
            handle: mtg_core::EffectHandle("fog".into()),
            origin: origin.clone(),
            expires_in_cleanup_of_turn: v.turn.number,
        },
        mtg_core::VisibleEffect::PreventNextDamage {
            handle: mtg_core::EffectHandle("shield".into()),
            origin: origin.clone(),
            target: v.stack[0].entity_targets[0].clone(),
            remaining: 3.into(),
            expires_in_cleanup_of_turn: v.turn.number,
        },
        mtg_core::VisibleEffect::DoubleDamageToPermanentOrPlayer {
            handle: mtg_core::EffectHandle("double".into()),
            origin,
        },
    ] {
        v.effects = vec![effect];
        assert_eq!(preferred(&c, &v, None), None);
    }
    v.effects.clear();
    v.stack[0].entity_targets.clear();
    assert_eq!(preferred(&c, &v, None), None);
}
#[test]
fn temporary_protection_policy_synthetic_identity_and_invalid_shape_controls_are_separate() {
    let (mut c, v) = fixture();
    let id = v.objects[0].oracle_id.clone();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == id)
        .unwrap();
    d.name = "Synthetic renamed protection".into();
    d.validate().unwrap();
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        cast(&v)
    );
    // Deliberately invalid shape mutations test the recognizer boundary only.
    // They are not admitted definitions or legal-pod/permitted-IR evidence.
    for n in 0..3 {
        let mut altered = c.clone();
        let d = altered
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == id)
            .unwrap();
        match n {
            0 => d.spell.as_mut().unwrap().instructions.push(I::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            }),
            1 => d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase,
            2 => {
                d.spell.as_mut().unwrap().targets[0].domain =
                    TargetDomain::ControlledBattlefieldCreature
            }
            _ => unreachable!(),
        }
        assert!(!temporary_indestructible_spell(
            d,
            d.spell.as_ref().unwrap()
        ));
    }
}
