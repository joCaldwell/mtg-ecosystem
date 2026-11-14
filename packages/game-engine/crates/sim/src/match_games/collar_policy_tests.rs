//! Synthetic permitted views over actual IR, separate from paid gameplay.
use super::*;
fn fixture() -> (Catalog, PermittedView) {
    let c = mtg_compiler::catalog::embedded_catalog("sacrifice").unwrap();
    let base = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&base);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.objects = ["Basilisk Collar", "Isamaru, Hound of Konda", "Serra Angel"]
        .map(|n| wipe_tests::card(&c, n, v.viewer.0, Zone::Battlefield))
        .to_vec();
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .mana = BTreeMap::from([(Color::W, 2.into())]);
    (c, v)
}
#[test]
fn collar_policy_actual_ir_equips_best_then_stays_attached() {
    let (c, mut v) = fixture();
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Basilisk Collar")
        .unwrap();
    let p = &d.activated[0].program;
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::ActivateAbility {
            object: v.objects[0].handle.clone(),
            ability: 0
        }
    );
    assert_eq!(
        strategy(&c).object_target(&v, d, p, &p.targets[0], None),
        Some(v.objects[2].handle.clone())
    );
    v.objects[0].attached_to = Some(v.objects[2].handle.clone());
    v.objects[2]
        .keywords
        .extend([KeywordAbility::Deathtouch, KeywordAbility::Lifelink]);
    for _ in 0..5 {
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            PlayerResponse::Pass
        );
    }
}
#[test]
fn collar_policy_respects_timing_mana_control_targetability_and_offers() {
    let (c, v) = fixture();
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Basilisk Collar")
        .unwrap();
    let p = &d.activated[0].program;
    for change in 0..5 {
        let mut x = v.clone();
        match change {
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
            _ => {
                for o in &mut x.objects[1..] {
                    o.keywords.push(KeywordAbility::Shroud);
                }
            }
        }
        assert_eq!(
            strategy(&c).priority(&observation(x)).unwrap(),
            PlayerResponse::Pass,
            "change {change}"
        );
    }
    for offers in [
        vec![],
        vec![ObjectHandle("stale".into())],
        vec![v.objects[0].handle.clone()],
    ] {
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&offers)),
            None
        );
    }
    let low = v.objects[1].handle.clone();
    assert_eq!(
        strategy(&c).object_target(&v, d, p, &p.targets[0], Some(std::slice::from_ref(&low))),
        Some(low)
    );
}
