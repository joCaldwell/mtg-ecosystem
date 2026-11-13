//! Complete archived Icy Manipulator and hash-consistent unsupported AST probes.
use super::*;
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const ICY: &str = include_str!("../../../tests/fixtures/3608f1f7-8dc5-4dd1-ae91-c830e1de9529.json");

fn changed(mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(ICY, |record| {
        mutate(&mut record["faces"][0]["result"]["abilities"][0]);
        let result = &mut record["faces"][0]["result"];
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        let target = result["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
        let path = "/faces/0/abilities/0/effects/0/steps/0/what";
        for node in record["binding"]["nodes"].as_array_mut().unwrap() {
            if node["node"] == "selection" && node["path"] == path {
                node["expression"] = target.clone();
            }
        }
        for reference in record["binding"]["references"].as_array_mut().unwrap() {
            if reference["path"] == path {
                reference["expression"] = target.clone();
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
fn complete_icy_manipulator_preserves_paid_tap_and_union_target() {
    assert_eq!(
        sha256(ICY.trim_end().as_bytes()),
        "a14bc82cdc2d5aa57995db8e56718d80400c44d519bd0be55f71c6883fc9765e"
    );
    let record: Value = strict_read(ICY).unwrap();
    assert_eq!(
        record["source"]["oracle_text"],
        "{1}, {T}: Tap target artifact, creature, or land."
    );
    let d = lowered(ICY);
    assert_eq!(d.name, "Icy Manipulator");
    assert_eq!(d.oracle_id, "3608f1f7-8dc5-4dd1-ae91-c830e1de9529");
    assert_eq!(d.characteristics.kind, CardKind::Artifact);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 4.into(),
            colored: vec![]
        }
    );
    assert_eq!(d.provenance.source_index, 8052);
    assert_eq!(
        d.provenance.source_sha256,
        "af6c382c39adb75ea69fe0326cdef9e429874b5055e0df9f7f1b66f614b082a5"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "d6350e6ccc437c4cd1874f6577c278558e7fa0f569804bf0cc7930cff25208b6"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "30fd7f9c222aa542af37f29e52c981b451c4f8996dfd166df2b695c05301ccd9"
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.source_json).unwrap(),
        record["source"]
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.abilities_json).unwrap(),
        record["faces"][0]["result"]["abilities"]
    );
    assert!(d.triggered.is_empty() && d.static_abilities.is_empty() && d.mana_abilities.is_empty());
    assert_eq!(d.activated.len(), 1);
    let a = &d.activated[0];
    assert_eq!(a.functioning_zone, FunctioningZone::Battlefield);
    assert_eq!(
        a.cost,
        ActivationCost::Composite {
            costs: PermanentActivationCosts {
                mana: Some(ManaCost {
                    generic: 1.into(),
                    colored: vec![]
                }),
                tap: true,
                sacrifice: false,
            }
        }
    );
    assert_eq!(a.program.timing, SpellTiming::Instant);
    assert_eq!(
        a.program.instructions,
        [Instruction::TapPermanent {
            target_slot: 0,
            source_block: 0
        }]
    );
    assert_eq!(a.program.targets.len(), 1);
    let target = &a.program.targets[0];
    assert_eq!(target.selection_id, "line:0:5");
    assert_eq!(target.domain, TargetDomain::BattlefieldPermanent);
    let filter = target.filter.as_ref().unwrap();
    assert_eq!(
        filter.types.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([CardKind::Artifact, CardKind::Creature, CardKind::Land])
    );
    assert!(
        filter.non_types.is_empty()
            && filter.non_colors.is_empty()
            && !filter.opponent
            && !filter.tapped
    );
    assert!(
        d.capabilities
            .contains(&Capability::ActivatedTapPermanentV1)
    );
    assert!(d.capabilities.contains(&Capability::TapPermanentV1));
    let rebuilt = lowered(&changed(|_| {}));
    assert_eq!(rebuilt.activated, d.activated);
    assert_eq!(rebuilt.capabilities, d.capabilities);
}

#[test]
fn activated_tap_source_rejects_unrepresented_targets_and_step_fields() {
    for (field, value) in [
        ("count", json!({"amount":"fixed","value":0})),
        ("count", json!({"amount":"fixed","value":2})),
        ("ref", json!("all")),
        ("selectionId", json!("")),
        ("upTo", json!(true)),
        ("filter", json!({"cls":"permanent"})),
        ("filter", json!({"types":["creature"]})),
        ("filter", json!({"types":["artifact","creature","land"]})),
        ("filter", json!({"types":["enchantment"]})),
    ] {
        let raw = changed(|a| a["effects"][0]["steps"][0]["what"][field] = value);
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{field}"
        );
    }
    for field in ["optional", "duration", "unknown"] {
        let raw = changed(|a| a["effects"][0]["steps"][0][field] = json!(true));
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{field}"
        );
    }
}

#[test]
fn activated_tap_rejects_incomplete_costs_and_extra_effects() {
    for costs in [
        json!([]),
        json!([{"cost":"tap-self"}]),
        json!([{"cost":"mana","symbols":["1"]}]),
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
