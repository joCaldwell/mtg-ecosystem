//! Complete archived Seeker of Skybreak and hash-consistent unsupported AST probes.
use super::*;
use serde_json::{Value, json};
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const SEEKER: &str =
    include_str!("../../../tests/fixtures/704723f3-0490-4d96-930a-aacc6d19ba8d.json");

fn changed(mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(SEEKER, |record| {
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
fn complete_seeker_of_skybreak_preserves_source_tap_and_creature_untap() {
    assert_eq!(
        sha256(SEEKER.trim_end().as_bytes()),
        "9ac057c2225b2d50f630c3593cce81ee7f272cdbb07d2dd20cb1c8fa895a85e5"
    );
    let record: Value = strict_read(SEEKER).unwrap();
    assert_eq!(
        record["source"]["oracle_text"],
        "{T}: Untap target creature."
    );
    let d = lowered(SEEKER);
    assert_eq!(d.name, "Seeker of Skybreak");
    assert_eq!(d.oracle_id, "704723f3-0490-4d96-930a-aacc6d19ba8d");
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert_eq!(d.characteristics.color_identity, vec![Color::G]);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::G]
        }
    );
    assert_eq!(d.characteristics.power, Some(2.into()));
    assert_eq!(d.characteristics.toughness, Some(1.into()));
    assert_eq!(d.provenance.source_index, 16865);
    assert_eq!(
        d.provenance.source_sha256,
        "55f1beb544167b6ac4ab5d4c2435b3b84cce6f9928b50b8b90558a629d60c8fc"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "44e9e9a861888434c8290871f1c95dfebe6eadc7b3b53940abe1fa54e47f87d5"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "317976d6b07c03f2b6f20a0dd150fc2472ddefdfb2e174bffe07bdecd0021cce"
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
        [Instruction::UntapPermanent {
            target_slot: 0,
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
    assert!(!f.tapped && !f.opponent && f.non_types.is_empty() && f.non_colors.is_empty());
    assert!(
        d.capabilities
            .contains(&Capability::TapSourceUntapCreatureV1)
    );
    assert!(d.capabilities.contains(&Capability::UntapPermanentV1));
    let rebuilt = lowered(&changed(|_| {}));
    assert_eq!(rebuilt.activated, d.activated);
    assert_eq!(rebuilt.capabilities, d.capabilities);
}
#[test]
fn tap_source_untap_rejects_unrepresented_target_qualifiers() {
    for (field, value) in [
        ("filter", json!({"cls":"permanent"})),
        ("filter", json!({"types":["creature"],"status":["tapped"]})),
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
fn tap_source_untap_rejects_unrepresented_costs_flags_and_compositions() {
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
