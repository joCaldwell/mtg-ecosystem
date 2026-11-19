use super::*;

const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const TALISMAN: &str =
    include_str!("../../../tests/fixtures/ramos/14d2979d-5728-42d7-a027-0eb1f754655d.json");
const CITY: &str =
    include_str!("../../../tests/fixtures/ramos/f25351e3-539b-4bbc-b92d-6480acf4d722.json");

#[test]
fn pain_mana_lowers_complete_source_and_rejects_wrong_activator_binding() {
    let result = compile(MANIFEST, TALISMAN);
    let CompileResult::Lowered { definition } = result else {
        panic!("expected a complete artifact definition, got {result:?}");
    };
    assert_eq!(definition.characteristics.kind, CardKind::Artifact);
    let [safe, colored] = definition.mana_abilities.as_slice() else {
        panic!("expected two tap abilities");
    };
    assert_eq!(
        safe.production,
        ManaProduction::Fixed {
            symbols: vec![Color::C],
        }
    );
    assert_eq!(safe.self_damage, None);
    assert_eq!(
        colored.production,
        ManaProduction::Choice {
            colors: vec![Color::U, Color::R],
        }
    );
    assert_eq!(colored.self_damage, Some(1.into()));
    assert_eq!(definition.source_labels.as_ref().unwrap().mana, [0, 1]);
    assert_eq!(
        definition.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastArtifactV1,
            Capability::PermanentManaV1,
            Capability::NoncombatDamageV1,
            Capability::DisjointPainManaV1,
        ])
    );

    let wrong_activator = crate::tests::changed_source(TALISMAN, |record| {
        let references = record["binding"]["references"].as_array_mut().unwrap();
        let other_activator = references
            .iter()
            .find(|reference| reference["path"] == "/faces/0/abilities/0/costs/0/payer")
            .unwrap()["target"]
            .clone();
        let recipient = references
            .iter_mut()
            .find(|reference| reference["path"] == "/faces/0/abilities/1/effects/1/steps/0/to/ref")
            .unwrap();
        recipient["target"] = other_activator;
    });
    assert!(matches!(
        compile(MANIFEST, &wrong_activator),
        CompileResult::Unsupported { path, .. } if path == "binding.mana"
    ));
    let CompileResult::Lowered { definition } = compile(MANIFEST, CITY) else {
        panic!("the separate tap trigger and mana ability must lower together");
    };
    assert!(!disjoint_pain_mana_family(&definition));
    assert_eq!(definition.mana_abilities.len(), 1);
    assert_eq!(definition.mana_abilities[0].production, ManaProduction::AnyColor);
    assert_eq!(definition.mana_abilities[0].self_damage, None);

    let wrong_controller = crate::tests::changed_source(CITY, |record| {
        let references = record["binding"]["references"].as_array_mut().unwrap();
        let activator = references[4]["target"].clone();
        references[2]["target"] = activator;
    });
    assert!(matches!(
        compile(MANIFEST, &wrong_controller),
        CompileResult::Unsupported { path, .. } if path == "binding.becomes-tapped"
    ));
    let missing_trigger = crate::tests::changed_source(CITY, |record| {
        let result = &mut record["faces"][0]["result"];
        result["abilities"].as_array_mut().unwrap().remove(0);
        result["lines"].as_array_mut().unwrap().remove(0);
    });
    assert!(matches!(
        compile(MANIFEST, &missing_trigger),
        CompileResult::Invalid { path, .. } if path == "lines"
    ));
}
