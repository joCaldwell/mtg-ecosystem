//! Synthetic permitted observations over actual admitted IR; no gameplay proof.
use super::*;
use mtg_contracts::KeywordAbility as K;
fn fixture() -> (Catalog, PermittedView) {
    let c = mtg_compiler::catalog::embedded_catalog("sacrifice").unwrap();
    let baseline = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&baseline);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.objects = ["Darksteel Plate", "Isamaru, Hound of Konda", "Serra Angel"]
        .map(|n| wipe_tests::card(&c, n, v.viewer.0, Zone::Battlefield))
        .to_vec();
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .mana = BTreeMap::from([(Color::W, 2.into())]);
    (c, v)
}
fn activate(v: &PermittedView) -> PlayerResponse {
    PlayerResponse::ActivateAbility {
        object: v.objects[0].handle.clone(),
        ability: 0,
    }
}
fn attach(v: &mut PermittedView, index: usize) {
    for o in &mut v.objects[1..] {
        o.keywords.retain(|k| *k != K::Indestructible);
    }
    v.objects[0].attached_to = Some(v.objects[index].handle.clone());
    v.objects[index].keywords.push(K::Indestructible);
}
#[test]
fn plate_policy_incremental_protection_keeps_current_best_and_never_alternates() {
    let (c, mut v) = fixture();
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        activate(&v)
    );
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Darksteel Plate")
        .unwrap();
    let p = &d.activated[0].program;
    assert_eq!(
        strategy(&c).object_target(&v, d, p, &p.targets[0], None),
        Some(v.objects[2].handle.clone())
    );
    attach(&mut v, 2);
    for _ in 0..5 {
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            PlayerResponse::Pass
        );
    }
    attach(&mut v, 1);
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        activate(&v),
        "strict value improvement"
    );
    attach(&mut v, 2);
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass
    );
    // Equal-valued alternative with a higher handle must not trigger a move.
    let mut equal = v.objects[2].clone();
    equal.handle = ObjectHandle("zz-equal".into());
    equal.keywords.clear();
    v.objects.push(equal);
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass
    );
}
#[test]
fn plate_policy_readiness_control_timing_protection_and_candidate_authority() {
    let (c, v) = fixture();
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Darksteel Plate")
        .unwrap();
    let p = &d.activated[0].program;
    for mutation in 0..6 {
        let mut x = v.clone();
        match mutation {
            0 => x.active = Some(PlayerId(0)),
            1 => x.turn.step = TurnStep::End,
            2 => x.objects[0].controller = PlayerId(0),
            3 => x
                .players
                .iter_mut()
                .find(|p| p.player == x.viewer)
                .unwrap()
                .mana
                .clear(),
            4 => {
                for o in &mut x.objects[1..] {
                    o.keywords.push(K::Indestructible);
                }
            }
            _ => {
                for o in &mut x.objects[1..] {
                    o.keywords.push(K::Shroud);
                }
            }
        }
        assert_eq!(
            strategy(&c).priority(&observation(x)).unwrap(),
            PlayerResponse::Pass,
            "mutation {mutation}"
        );
    }
    let low = v.objects[1].handle.clone();
    for offered in [
        vec![],
        vec![ObjectHandle("stale".into())],
        vec![v.objects[0].handle.clone()],
    ] {
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&offered)),
            None
        );
    }
    assert_eq!(
        strategy(&c).object_target(&v, d, p, &p.targets[0], Some(std::slice::from_ref(&low))),
        Some(low.clone())
    );
    let mut x = v.clone();
    x.objects[2].keywords.push(K::Indestructible);
    assert_eq!(
        strategy(&c).object_target(&x, d, p, &p.targets[0], None),
        Some(low)
    );
    x.objects[1].controller = PlayerId(0);
    assert!(!strategy(&c).program_ready(&x, d, p));
}
#[test]
fn plate_policy_announced_fallback_is_separate_from_initiation_and_buffs_preserved() {
    let (c, mut v) = fixture();
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Darksteel Plate")
        .unwrap();
    let p = &d.activated[0].program;
    attach(&mut v, 1);
    v.objects[2].keywords.push(K::Indestructible); // Another source: no incremental gain.
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass
    );
    let h = v.objects[1].handle.clone();
    v.stack.push(VisibleStackEntry {
        x: None,
        mana_value: None,
        mode: None,
        triggered: None,
        entity_targets: vec![],
        handle: Some(StackHandle("equip".into())),
        stack_targets: vec![],
        spell: None,
        ability: Some(mtg_core::VisibleAbility {
            handle: StackHandle("equip".into()),
            oracle_id: d.oracle_id.clone(),
            ability: 0,
            source: mtg_core::VisibleTarget {
                announced: v.objects[0].handle.clone(),
                current: Some(v.objects[0].handle.clone()),
            },
        }),
        player_targets: vec![],
        controller: v.viewer,
        cast_complete: false,
        targets: vec![],
    });
    assert_eq!(
        strategy(&c).object_target(&v, d, p, &p.targets[0], Some(std::slice::from_ref(&h))),
        Some(h.clone())
    );
    // Already announced and only an independently protected target remains.
    let other = v.objects[2].handle.clone();
    let mut o = observation(v.clone());
    o.decision = Some(PlayerRequest::Targets {
        token: mtg_core::DecisionToken("equip-choice".into()),
        player: v.viewer,
        occurrences: p.targets.clone(),
        candidates: vec![other.clone()],
    });
    assert_eq!(
        strategy(&c).choose(&o).unwrap(),
        PlayerResponse::Targets {
            objects: vec![other]
        }
    );
    v.stack.clear();
    v.objects[0] = wipe_tests::card(&c, "Bonesplitter", v.viewer.0, Zone::Battlefield);
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        activate(&v),
        "ordinary buff remains useful on protected creatures"
    );
}

#[test]
fn plate_policy_current_attachment_uses_current_friendly_control() {
    let (c, mut v) = fixture();
    attach(&mut v, 2);
    // A formerly friendly recipient is no longer a useful attachment to retain.
    v.objects[2].controller = PlayerId(0);
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        activate(&v)
    );
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Darksteel Plate")
        .unwrap();
    let p = &d.activated[0].program;
    assert_eq!(
        strategy(&c).object_target(
            &v,
            d,
            p,
            &p.targets[0],
            Some(&[v.objects[1].handle.clone(), v.objects[2].handle.clone()])
        ),
        Some(v.objects[1].handle.clone())
    );
}
