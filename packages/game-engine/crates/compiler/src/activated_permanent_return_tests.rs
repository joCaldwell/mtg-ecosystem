//! Complete archived Temporal Adept and hash-consistent unsupported AST probes.
use super::*;
use serde_json::{Value, json};
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const ADEPT: &str =
    include_str!("../../../tests/fixtures/fd9b5462-6578-4bf6-8026-23eaf5af3eee.json");
fn changed(mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(ADEPT, |r| {
        mutate(&mut r["faces"][0]["result"]["abilities"][0]);
        let result = &mut r["faces"][0]["result"];
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        let target = result["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
        let path = "/faces/0/abilities/0/effects/0/steps/0/what";
        for n in r["binding"]["nodes"].as_array_mut().unwrap() {
            if n["node"] == "selection" && n["path"] == path {
                n["expression"] = target.clone();
            }
        }
        for n in r["binding"]["references"].as_array_mut().unwrap() {
            if n["path"] == path {
                n["expression"] = target.clone();
            }
        }
    })
}
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    definition.validate().unwrap();
    definition
}
#[test]
fn complete_temporal_adept_preserves_paid_unqualified_owner_hand_activation() {
    assert_eq!(
        sha256(ADEPT.trim_end().as_bytes()),
        "1832ba119d921d02b2f53755a496d4f9f2ecfaacc1672f138b7c5d2a68329f53"
    );
    let r: Value = strict_read(ADEPT).unwrap();
    assert_eq!(
        r["source"]["oracle_text"],
        "{U}{U}{U}, {T}: Return target permanent to its owner's hand."
    );
    let d = lowered(ADEPT);
    assert_eq!(d.name, "Temporal Adept");
    assert_eq!(d.oracle_id, "fd9b5462-6578-4bf6-8026-23eaf5af3eee");
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::U, Color::U]
        }
    );
    assert_eq!(d.characteristics.power, Some(1.into()));
    assert_eq!(d.characteristics.toughness, Some(1.into()));
    assert_eq!(d.provenance.source_index, 38351);
    assert_eq!(
        d.provenance.source_sha256,
        "63e0fb4e43b958990f2b01b936058bf38fde8c31976aa617a0fcc7f3ba2385dd"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "4c1dfc8f0c97ccfb4a1a9eaeed0c269895f65ebb3d90e74aca8441330fb64b68"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "4725f48916370e68b109964adc30fc7c1e35239d0a0b9da6bc7d922589adb497"
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.source_json).unwrap(),
        r["source"]
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.abilities_json).unwrap(),
        r["faces"][0]["result"]["abilities"]
    );
    assert_eq!(d.activated.len(), 1);
    assert!(d.triggered.is_empty() && d.static_abilities.is_empty() && d.keywords.is_empty());
    let a = &d.activated[0];
    assert_eq!(
        a.cost,
        ActivationCost::Composite {
            costs: PermanentActivationCosts {
                mana: Some(ManaCost {
                    generic: 0.into(),
                    colored: vec![Color::U, Color::U, Color::U]
                }),
                tap: true,
                sacrifice: false
            }
        }
    );
    assert_eq!(
        a.program.instructions,
        [Instruction::ReturnToOwnerHand {
            target_slot: 0,
            source_block: 0
        }]
    );
    assert_eq!(a.program.targets.len(), 1);
    assert_eq!(
        a.program.targets[0].domain,
        TargetDomain::BattlefieldPermanent
    );
    assert_eq!(a.program.targets[0].selection_id, "line:0:7");
    let f = a.program.targets[0].filter.as_ref().unwrap();
    assert!(
        f.types.is_empty()
            && f.non_types.is_empty()
            && f.non_colors.is_empty()
            && !f.opponent
            && !f.tapped
    );
    assert!(
        d.capabilities
            .contains(&Capability::ActivatedPermanentOwnerHandV1)
    );
    assert!(
        d.capabilities
            .contains(&Capability::PermanentOwnerHandMoveV1)
    );
    let rebuilt = lowered(&changed(|_| {}));
    assert_eq!(rebuilt.activated, d.activated);
    assert_eq!(rebuilt.capabilities, d.capabilities);
}
#[test]
fn activated_permanent_return_rejects_qualified_or_incomplete_selectors() {
    for (field, value) in [
        ("filter", json!({"cls":"permanent","nonTypes":["land"]})),
        ("filter", json!({"types":["creature"]})),
        ("filter", json!({"cls":"permanent","control":"opponent"})),
        ("filter", json!({"cls":"permanent","status":["tapped"]})),
        ("count", json!({"amount":"fixed","value":2})),
        ("ref", json!("all")),
        ("selectionId", json!("")),
        ("upTo", json!(true)),
    ] {
        let raw = changed(|a| a["effects"][0]["steps"][0]["what"][field] = value);
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{field}"
        );
    }
    for (field, value) in [
        ("to", json!("your")),
        ("optional", json!(true)),
        ("duration", json!("until-end-of-turn")),
    ] {
        let raw = changed(|a| a["effects"][0]["steps"][0][field] = value);
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{field}"
        );
    }
}
#[test]
fn activated_permanent_return_rejects_incomplete_costs_and_extra_effects() {
    for costs in [
        json!([]),
        json!([{"cost":"tap-self"}]),
        json!([{"cost":"mana","symbols":["U","U","U"]}]),
        json!([{"cost":"mana","symbols":["-1"]},{"cost":"tap-self"}]),
    ] {
        let raw = changed(|a| a["costs"] = costs);
        assert!(matches!(
            compile(MANIFEST, &raw),
            CompileResult::Unsupported { .. }
        ));
    }
    let raw = changed(|a| {
        a["effects"].as_array_mut().unwrap().push(json!({"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1},"optional":false}]}))
    });
    assert!(matches!(
        compile(MANIFEST, &raw),
        CompileResult::Unsupported { .. }
    ));
}
