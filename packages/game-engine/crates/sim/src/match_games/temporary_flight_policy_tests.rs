//! Synthetic permitted-observation controls; the paid legal-pod proof is separate.
use super::*;
use mtg_contracts::KeywordAbility as K;
fn fixture() -> (Catalog, PermittedView) {
    let c = mtg_compiler::catalog::embedded_catalog("blocking-matches").unwrap();
    let mut v = wipe_tests::view(&mtg_compiler::catalog::embedded_catalog("wipes").unwrap());
    v.active = Some(v.viewer);
    v.turn_owner = v.viewer;
    v.turn.step = TurnStep::DeclareAttackers;
    v.turn.lands_played = 1;
    v.objects = vec![
        wipe_tests::card(&c, "Jump", v.viewer.0, Zone::Hand { owner: v.viewer }),
        wipe_tests::card(&c, "Ertai, Wizard Adept", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Island", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Isamaru, Hound of Konda", 0, Zone::Battlefield),
    ];
    v.objects[1].tapped = true; // Already declared, rather than a future attack.
    v.combat = vec![mtg_core::VisibleAttacker {
        attacker: v.objects[1].handle.clone(),
        defender: PlayerId(0),
        blocked: false,
        blockers: vec![],
    }];
    (c, v)
}
fn preferred(
    c: &Catalog,
    v: &PermittedView,
    candidates: Option<&[ObjectHandle]>,
) -> Option<ObjectHandle> {
    let d = c.definitions.iter().find(|d| d.name == "Jump").unwrap();
    let p = d.spell.as_ref().unwrap();
    strategy(c).object_target(v, d, p, &p.targets[0], candidates)
}
#[test]
fn temporary_flight_policy_lifts_actual_attacker_before_blockers_with_paid_public_ir() {
    let (c, v) = fixture();
    let before = canonical(&v).unwrap();
    let mut s = strategy(&c);
    assert_eq!(
        s.priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Cast {
            card: v.objects[0].handle.clone()
        }
    );
    assert_eq!(s.mana_plan.len(), 1);
    assert_eq!(s.mana_plan[0].source, v.objects[2].handle);
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    for cs in [
        vec![],
        vec![ObjectHandle("stale".into())],
        vec![v.objects[3].handle.clone()],
    ] {
        assert_eq!(preferred(&c, &v, Some(&cs)), None);
    }
    assert_eq!(
        preferred(&c, &v, Some(&[v.objects[1].handle.clone()])),
        Some(v.objects[1].handle.clone())
    );
    assert_eq!(canonical(&v).unwrap(), before);
}
#[test]
fn temporary_flight_policy_declines_redundant_ineligible_and_unprovable_benefit() {
    let (c, v) = fixture();
    for n in 0..22 {
        let mut x = v.clone();
        match n {
            0 => x.objects[1].keywords.push(K::Flying),
            1 => x.objects[1].keywords.push(K::Shroud),
            2 => x.objects[1].controller = PlayerId(0),
            3 => x.combat.clear(),
            4 => x.objects[3].tapped = true,
            5 => x.objects[3].keywords.push(K::Flying),
            6 => x.objects[3].keywords.push(K::Reach),
            7 => x.objects[3].controller = PlayerId(2),
            8 => x.objects[3]
                .restrictions
                .push(CreatureRestriction::CantBlock),
            9 => x.objects[3]
                .restrictions
                .push(CreatureRestriction::CantAttackOrBlock),
            10 => x.turn.step = TurnStep::PrecombatMain,
            11 => x.turn.step = TurnStep::DeclareBlockers,
            12 => x.turn.step = TurnStep::FirstStrikeDamage,
            13 => x.combat[0].blocked = true,
            14 => x.combat[0].blockers.push(x.objects[3].handle.clone()),
            15 => x.objects[1]
                .restrictions
                .push(CreatureRestriction::CantBeBlocked),
            16 => x.objects[1].keywords.push(K::Menace),
            17 => x.objects[1].characteristics.as_mut().unwrap().power = Some(0.into()),
            18 => x.objects[1].zone = Zone::Hand { owner: x.viewer },
            19 => x.active = Some(PlayerId(0)),
            20 => x.objects[3].characteristics = None,
            21 => x.objects[1].handle = ObjectHandle("fresh-incarnation".into()),
            _ => unreachable!(),
        }
        assert_eq!(preferred(&c, &x, None), None, "case {n}");
        assert_eq!(
            strategy(&c).priority(&observation(x)).unwrap(),
            PlayerResponse::Pass,
            "case {n}"
        );
    }
    let mut no_mana = v;
    no_mana.objects[2].tapped = true;
    assert!(strategy(&c).reactive_spells(&no_mana).unwrap().is_none());
}
#[test]
fn temporary_flight_policy_counts_only_actual_defender_and_current_control() {
    let (c, mut v) = fixture();
    // Friendly hexproof and reach do not make offensive flight redundant.
    v.objects[1].keywords = vec![K::Hexproof, K::Reach];
    v.objects[1].owner = PlayerId(0);
    let mut other = wipe_tests::card(&c, "Grizzly Bears", 2, Zone::Battlefield);
    other.keywords.push(K::Reach);
    v.objects.push(other);
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
    // A second actual defending flyer defeats the stated guaranteed evasion.
    v.objects[4].controller = PlayerId(0);
    assert_eq!(preferred(&c, &v, None), None);
    v.objects[4].tapped = true;
    assert_eq!(preferred(&c, &v, None), Some(v.objects[1].handle.clone()));
}
#[test]
fn temporary_flight_policy_intervening_stack_and_defensive_forecasts_are_excluded() {
    let (c, mut v) = fixture();
    stack_spell(&mut v, &c, "Giant Growth", 0);
    assert_eq!(preferred(&c, &v, None), None);
    assert!(strategy(&c).reactive_spells(&v).unwrap().is_none());
    v.stack.clear();
    v.active = Some(PlayerId(0));
    v.turn_owner = PlayerId(0);
    v.objects[1].tapped = false;
    v.objects[3].keywords.push(K::Flying);
    v.combat[0].attacker = v.objects[3].handle.clone();
    v.combat[0].defender = v.viewer;
    assert_eq!(preferred(&c, &v, None), None);
}
#[test]
fn temporary_flight_policy_announced_targets_retain_mandatory_legal_fallback() {
    let (c, mut v) = fixture();
    let d = c.definitions.iter().find(|d| d.name == "Jump").unwrap();
    let p = d.spell.as_ref().unwrap();
    v.objects.remove(0);
    let viewer = v.viewer;
    stack_spell(&mut v, &c, "Jump", viewer.0);
    v.stack.last_mut().unwrap().cast_complete = false;
    let h = v.objects[0].handle.clone();
    assert_eq!(preferred(&c, &v, None), Some(h.clone()));
    let mut o = observation(v);
    o.decision = Some(PlayerRequest::Targets {
        token: mtg_core::DecisionToken("flight".into()),
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
    // Preference can disappear after announcement, but the offered legal choice
    // remains mandatory; policy cannot abandon an already announced spell.
    o.view.objects[0].keywords.push(K::Flying);
    assert_eq!(preferred(&c, &o.view, None), None);
    let before = canonical(&o).unwrap();
    assert_eq!(
        strategy(&c).choose(&o).unwrap(),
        PlayerResponse::Targets { objects: vec![h] }
    );
    assert_eq!(canonical(&o).unwrap(), before);
}
#[test]
fn temporary_flight_policy_complete_pure_ir_uses_no_card_or_profile_identity() {
    let (mut c, v) = fixture();
    let id = v.objects[0].oracle_id.clone();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == id)
        .unwrap();
    d.name = "Synthetic renamed flight".into();
    d.validate().unwrap();
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Cast {
            card: v.objects[0].handle.clone()
        }
    );
    for n in 0..6 {
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
            3 => d.characteristics.kind = CardKind::Creature,
            4 => {
                let t = d.spell.as_ref().unwrap().targets[0].clone();
                d.spell.as_mut().unwrap().targets.push(t);
            }
            5 => {
                if let I::GrantKeywords { keywords, .. } =
                    &mut d.spell.as_mut().unwrap().instructions[0]
                {
                    keywords.push(K::Reach);
                }
            }
            _ => unreachable!(),
        }
        assert!(!temporary_flying_spell(d, d.spell.as_ref().unwrap()));
    }
}
