//! Complete retained card definitions in synthetic public payload probes.
//! These are not accepted-pod, source-pool admission or replay witnesses.
use super::*;
use std::collections::BTreeSet;

fn fixture() -> (Catalog, PermittedView) {
    let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    for (manifest, raw, name, extra) in [
        (
            include_str!(
                "../../../../tests/fixtures/cards-data-filtered-damage/archive-manifest-current.json"
            ),
            include_str!(
                "../../../../tests/fixtures/cards-data-filtered-damage/4caa8e68-e599-416f-b839-3d009569ff29.json"
            ),
            "Alpha Myr",
            CardKind::Artifact,
        ),
        (
            include_str!(
                "../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"
            ),
            include_str!(
                "../../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
            ),
            "Research Thief",
            CardKind::Artifact,
        ),
        (
            include_str!(
                "../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"
            ),
            include_str!(
                "../../../../tests/fixtures/cards-data-d2ce/34fd461b-a913-4a60-9477-ce2bd5c6fc5a.json"
            ),
            "Fear of Failed Tests",
            CardKind::Enchantment,
        ),
    ] {
        let result = mtg_compiler::compile(manifest, raw);
        let mtg_compiler::CompileResult::Lowered { definition: d } = result else {
            panic!("{name}: {result:?}");
        };
        let record: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(d.name, name);
        assert_eq!(d.oracle_id, record["oracleIdentity"].as_str().unwrap());
        assert_eq!(
            d.provenance.source_sha256,
            record["sourceSha256"].as_str().unwrap()
        );
        assert_eq!(
            d.provenance.ast_sha256,
            record["astSha256"].as_str().unwrap()
        );
        assert_eq!(
            d.provenance.binding_sha256,
            record["bindingSha256"].as_str().unwrap()
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&d.provenance.source_json).unwrap(),
            record["source"]
        );
        assert_eq!(d.characteristics.kind, CardKind::Creature);
        assert_eq!(d.characteristics.additional_types, vec![extra]);
        assert!(
            d.characteristics.has_type(CardKind::Creature) && d.characteristics.has_type(extra)
        );
        c.definitions.push(*d);
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    v.active = Some(v.viewer);
    v.turn_owner = v.viewer;
    v.priority = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    (c, v)
}

fn spell<'a>(
    c: &'a Catalog,
    name: &str,
) -> (
    &'a mtg_contracts::Definition,
    &'a mtg_contracts::SpellProgram,
) {
    let d = c.definitions.iter().find(|d| d.name == name).unwrap();
    (d, d.spell.as_ref().unwrap())
}

#[test]
fn removal_policy_uses_complete_positive_and_negative_type_membership() {
    let (c, mut v) = fixture();
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .mana = BTreeMap::from([(Color::B, 1.into()), (Color::C, 1.into())]);
    v.objects = vec![
        wipe_tests::card(
            &c,
            "Go for the Throat",
            v.viewer.0,
            Zone::Hand { owner: v.viewer },
        ),
        wipe_tests::card(&c, "Research Thief", 0, Zone::Battlefield),
        wipe_tests::card(&c, "Alpha Myr", 1, Zone::Battlefield),
        wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield),
    ];
    let before = canonical(&v).unwrap();
    let (go, gp) = spell(&c, "Go for the Throat");
    assert_eq!(
        strategy(&c).object_target(&v, go, gp, &gp.targets[0], None),
        None,
        "an artifact creature is not a nonartifact creature, regardless of primary type"
    );
    assert!(!strategy(&c).definition_ready(&v, go));
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass,
        "do not initiate a harmful spell when only its own creature is actually legal"
    );
    assert_eq!(canonical(&v).unwrap(), before);

    let mut bear = wipe_tests::card(&c, "Grizzly Bears", 0, Zone::Battlefield);
    bear.handle = ObjectHandle("enemy-bear".into());
    v.objects.push(bear);
    assert_eq!(
        strategy(&c).object_target(&v, go, gp, &gp.targets[0], None),
        Some(ObjectHandle("enemy-bear".into()))
    );
    assert_eq!(
        strategy(&c).object_target(
            &v,
            go,
            gp,
            &gp.targets[0],
            Some(&[ObjectHandle("Research Thief".into())])
        ),
        None
    );

    let (shatter, sp) = spell(&c, "Shatter");
    assert_eq!(
        strategy(&c).object_target(&v, shatter, sp, &sp.targets[0], None),
        Some(ObjectHandle("Research Thief".into()))
    );
    assert_eq!(
        strategy(&c).object_target(
            &v,
            shatter,
            sp,
            &sp.targets[0],
            Some(&[ObjectHandle("Alpha Myr".into())])
        ),
        Some(ObjectHandle("Alpha Myr".into()))
    );
    assert_eq!(
        strategy(&c).object_target(
            &v,
            shatter,
            sp,
            &sp.targets[0],
            Some(&[ObjectHandle("enemy-bear".into())])
        ),
        None
    );
    v.objects[1].controller = v.viewer;
    assert_eq!(
        strategy(&c).object_target(&v, shatter, sp, &sp.targets[0], None),
        Some(ObjectHandle("Alpha Myr".into())),
        "harmful target preference follows control, not artifact value"
    );
}

#[test]
fn stack_policy_includes_enchantment_creatures_without_including_other_creatures() {
    let (c, mut v) = fixture();
    v.active = Some(PlayerId(0));
    v.turn_owner = PlayerId(0);
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .mana = BTreeMap::from([(Color::U, 1.into())]);
    v.objects = vec![wipe_tests::card(
        &c,
        "Swan Song",
        v.viewer.0,
        Zone::Hand { owner: v.viewer },
    )];
    stack_spell(&mut v, &c, "Research Thief", 0);
    stack_spell(&mut v, &c, "Fear of Failed Tests", 1);
    let before = canonical(&v).unwrap();
    let (_, p) = spell(&c, "Swan Song");
    let target = &p.targets[0];
    let fear = StackHandle("Fear of Failed Tests".into());
    assert_eq!(
        strategy(&c).stack_target(&v, target, None),
        Some(fear.clone())
    );
    assert_eq!(
        strategy(&c).stack_target(&v, target, Some(&[fear.clone()])),
        Some(fear)
    );
    assert_eq!(
        strategy(&c).stack_target(&v, target, Some(&[StackHandle("Research Thief".into())])),
        None
    );
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Cast {
            card: ObjectHandle("Swan Song".into())
        }
    );
    assert_eq!(canonical(&v).unwrap(), before);
    v.stack[1].controller = v.viewer;
    assert_eq!(strategy(&c).stack_target(&v, target, None), None);
    v.stack[1].controller = PlayerId(1);
    v.stack[1].cast_complete = false;
    assert_eq!(strategy(&c).stack_target(&v, target, None), None);
    v.stack.clear();
    stack_spell(&mut v, &c, "Lightning Bolt", 0);
    assert_eq!(
        strategy(&c).stack_target(&v, target, None),
        Some(StackHandle("Lightning Bolt".into())),
        "ordinary instant membership is unchanged"
    );
}

#[test]
fn enchantment_wipe_policy_scores_actual_enchantment_creature_membership() {
    let (c, mut v) = fixture();
    v.objects = vec![
        wipe_tests::card(&c, "Fear of Failed Tests", 0, Zone::Battlefield),
        wipe_tests::card(&c, "Research Thief", 1, Zone::Battlefield),
        wipe_tests::card(&c, "Grizzly Bears", 1, Zone::Battlefield),
        wipe_tests::card(&c, "Sol Ring", 1, Zone::Battlefield),
    ];
    let before = canonical(&v).unwrap();
    let (_, p) = spell(&c, "Tranquility");
    assert!(strategy(&c).in_set(&v, &v.objects[0], &ObjectSet::Enchantments));
    for o in &v.objects[1..] {
        assert!(!strategy(&c).in_set(&v, o, &ObjectSet::Enchantments));
    }
    assert!(strategy(&c).wipe_score(&v, p, &Quantity::zero()).unwrap() > Quantity::zero());
    for o in &v.objects[..3] {
        assert!(strategy(&c).in_set(&v, o, &ObjectSet::Creatures));
    }
    assert!(!strategy(&c).in_set(&v, &v.objects[3], &ObjectSet::Creatures));
    assert_eq!(canonical(&v).unwrap(), before);
    v.objects[0].controller = v.viewer;
    assert!(
        strategy(&c)
            .wipe_score(&v, p, &Quantity::zero())
            .unwrap()
            .is_negative()
    );
    v.objects[0].zone = Zone::Hand { owner: PlayerId(0) };
    assert_eq!(
        strategy(&c).wipe_score(&v, p, &Quantity::zero()),
        Some(Quantity::zero())
    );
}

#[test]
fn typed_artifact_sacrifice_filter_uses_all_types_and_preserves_subtype_and_control() {
    // This is an existing typed filter over real source definitions, not a
    // claim that a new artifact-sacrifice ability/card has been admitted.
    let (c, mut v) = fixture();
    v.objects = ["Alpha Myr", "Research Thief", "Sol Ring", "Grizzly Bears"]
        .into_iter()
        .map(|name| wipe_tests::card(&c, name, v.viewer.0, Zone::Battlefield))
        .collect();
    let mut enemy = wipe_tests::card(&c, "Alpha Myr", 0, Zone::Battlefield);
    enemy.handle = ObjectHandle("enemy-alpha".into());
    v.objects.push(enemy);
    let mut hand = wipe_tests::card(
        &c,
        "Research Thief",
        v.viewer.0,
        Zone::Hand { owner: v.viewer },
    );
    hand.handle = ObjectHandle("hand-research".into());
    v.objects.push(hand);
    let before = canonical(&v).unwrap();
    let mut filter = SacrificeFilter {
        types: vec![CardKind::Artifact],
        subtypes: vec![],
    };
    let names = |filter: &SacrificeFilter| {
        strategy(&c)
            .sacrifice_objects(&v, filter)
            .into_iter()
            .map(|o| o.handle.0.clone())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        names(&filter),
        BTreeSet::from([
            "Alpha Myr".into(),
            "Research Thief".into(),
            "Sol Ring".into()
        ])
    );
    filter.subtypes.push("Myr".into());
    assert_eq!(names(&filter), BTreeSet::from(["Alpha Myr".into()]));
    filter.subtypes.push("Wizard".into());
    assert!(
        names(&filter).is_empty(),
        "subtype requirements remain conjunctive"
    );
    assert_eq!(canonical(&v).unwrap(), before);
}
