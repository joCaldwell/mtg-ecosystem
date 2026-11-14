//! Complete archived Royal Assassin and hash-consistent unsupported AST probes.
use super::*;
use serde_json::{Value, json};
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const ROYAL: &str =
    include_str!("../../../tests/fixtures/9ed6f28f-a3db-48c5-9ab0-b90a7fba5f57.json");

fn changed(mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(ROYAL, |record| {
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
fn complete_royal_assassin_preserves_source_tap_and_tapped_creature_destruction() {
    assert_eq!(
        sha256(ROYAL.trim_end().as_bytes()),
        "68f34dd8d643dfb3f6eec84ef0faee8356ddc2004bf2bc206d36fc0010f8ed08"
    );
    let record: Value = strict_read(ROYAL).unwrap();
    assert_eq!(
        record["source"]["oracle_text"],
        "{T}: Destroy target tapped creature."
    );
    let d = lowered(ROYAL);
    assert_eq!(d.name, "Royal Assassin");
    assert_eq!(d.oracle_id, "9ed6f28f-a3db-48c5-9ab0-b90a7fba5f57");
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::B, Color::B]
        }
    );
    assert_eq!(d.characteristics.power, Some(1.into()));
    assert_eq!(d.characteristics.toughness, Some(1.into()));
    assert_eq!(d.provenance.source_index, 23960);
    assert_eq!(
        d.provenance.source_sha256,
        "54841f5273ac6fe42a52c8f6d2b76c714ebe648f2f97ce4aaedc522a44569872"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "f5300678d7d31104d7ec9bb3d40099f7aa78bc2dbe2c36e39bc5ebe8904ba254"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "d4b95e0216d6f1ac8dade0b60e1f3c6f35e4b809a601713724c25fb23172767b"
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.source_json).unwrap(),
        record["source"]
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.abilities_json).unwrap(),
        record["faces"][0]["result"]["abilities"]
    );
    assert_eq!(d.activated.len(), 1);
    assert!(d.triggered.is_empty() && d.static_abilities.is_empty() && d.keywords.is_empty());
    let a = &d.activated[0];
    assert_eq!(a.cost, ActivationCost::TapSource);
    assert_eq!(
        a.program.instructions,
        [Instruction::DestroyPermanent {
            target_slot: 0,
            optional: false,
            regeneration_forbidden: false,
            source_block: 0
        }]
    );
    assert_eq!(a.program.targets.len(), 1);
    assert_eq!(
        a.program.targets[0].domain,
        TargetDomain::BattlefieldPermanent
    );
    assert_eq!(a.program.targets[0].selection_id, "line:0:3");
    let f = a.program.targets[0].filter.as_ref().unwrap();
    assert_eq!(f.types, [CardKind::Creature]);
    assert!(f.tapped && !f.opponent && f.non_types.is_empty() && f.non_colors.is_empty());
    assert!(d.capabilities.contains(&Capability::TapSourceDestroyV1));
    assert!(
        d.capabilities
            .contains(&Capability::TappedPermanentTargetV1)
    );
    let rebuilt = lowered(&changed(|_| {}));
    assert_eq!(rebuilt.activated, d.activated);
    assert_eq!(rebuilt.capabilities, d.capabilities);
}
#[test]
fn tap_source_destroy_rejects_unrepresented_target_qualifiers() {
    for (field, value) in [
        ("filter", json!({"types":["creature"]})),
        (
            "filter",
            json!({"types":["creature"],"status":["untapped"]}),
        ),
        ("filter", json!({"types":["artifact"],"status":["tapped"]})),
        (
            "filter",
            json!({"types":["creature"],"status":["tapped"],"control":"opponent"}),
        ),
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
}
#[test]
fn tap_source_destroy_rejects_unrepresented_costs_flags_and_compositions() {
    for costs in [
        json!([]),
        json!([{"cost":"mana","symbols":["1"]}]),
        json!([{"cost":"mana","symbols":["1"]},{"cost":"tap-self"}]),
    ] {
        let raw = changed(|a| a["costs"] = costs);
        assert!(matches!(
            compile(MANIFEST, &raw),
            CompileResult::Unsupported { .. }
        ));
    }
    for field in ["optional", "regenerationForbidden", "unknown"] {
        let raw = changed(|a| a["effects"][0]["steps"][0][field] = json!(true));
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{field}"
        );
    }
    let raw = changed(|a| {
        a["effects"].as_array_mut().unwrap().push(json!({"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1},"optional":false}]}))
    });
    assert!(matches!(
        compile(MANIFEST, &raw),
        CompileResult::Unsupported { .. }
    ));
}
