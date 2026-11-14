//! Complete frozen source, never a model-derived card definition.
use super::*;
use serde_json::json;
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const MYR: &str = include_str!("../../../tests/fixtures/f90fe86c-5cca-483b-93c2-6e856fa01c88.json");
const RELIC: &str =
    include_str!("../../../tests/fixtures/2bda1fb1-528c-4e66-89f1-bb1a85e9181e.json");
#[test]
fn indestructible_complete_sources_preserve_types_costs_and_provenance() {
    for (raw, name, kind, cost, index, record, source, binding) in [
        (
            MYR,
            "Darksteel Myr",
            CardKind::Creature,
            3,
            37662,
            "a72f08b25fe10e9f9ffdb64d6210d556e4e9da63034a664e406adf908cd276d1",
            "d2c6e1758e992f649d07bc855a097b4323c0f4c7309a4bf692f47ef0679a6778",
            "aa91847511172e3c0ee4fe4b2a950c137bce4090123d4b3a6f4e4624749b0a35",
        ),
        (
            RELIC,
            "Darksteel Relic",
            CardKind::Artifact,
            0,
            6531,
            "4a7fc010333c2990094b20bf66d60099e5a94502c85736b4d018713f4e87e382",
            "db6897b256569d33201997f22e098203ab944ed40874c85100cdc5605d789325",
            "4a51ae7610727b768b6f4f4ddb46797c685c0b24af7efe28a3db53e0aef13ed6",
        ),
    ] {
        assert_eq!(sha256(raw.trim_end().as_bytes()), record);
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{}", name)
        };
        d.validate().unwrap();
        assert_eq!(d.name, name);
        assert_eq!(d.characteristics.kind, kind);
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: cost.into(),
                colored: vec![]
            }
        );
        assert!(d.characteristics.colors.is_empty() && d.characteristics.color_identity.is_empty());
        assert_eq!(d.keywords, [KeywordAbility::Indestructible]);
        assert_eq!(d.capabilities, d.requirements());
        assert!(d.capabilities.contains(&Capability::IndestructibleV1));
        assert!(d.activated.is_empty() && d.triggered.is_empty() && d.static_abilities.is_empty());
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(d.provenance.source_sha256, source);
        assert_eq!(d.provenance.binding_sha256, binding);
        assert_eq!(
            d.provenance.ast_sha256,
            "712b80b94c162348f2a34cd39dedc4eea5d31e6899a00753d1f27a3e743ea87b"
        );
        assert_eq!(
            d.characteristics.additional_types,
            if kind == CardKind::Creature {
                vec![CardKind::Artifact]
            } else {
                vec![]
            }
        );
        assert_eq!(
            d.characteristics.power,
            if kind == CardKind::Creature {
                Some(0.into())
            } else {
                None
            }
        );
        assert_eq!(
            d.characteristics.toughness,
            if kind == CardKind::Creature {
                Some(1.into())
            } else {
                None
            }
        );
    }
}
#[test]
fn indestructible_complete_source_rejects_parameters_and_unknown_compositions() {
    for k in [
        json!({"keyword":"indestructible","cost":[]}),
        json!({"keyword":"indestructible","filter":{"types":["creature"]}}),
        json!({"keyword":"unknown"}),
    ] {
        let raw = tests::changed_source(MYR, |r| {
            let a = &mut r["faces"][0]["result"];
            a["abilities"][0]["keywords"]
                .as_array_mut()
                .unwrap()
                .push(k);
            a["lines"][0]["ability"] = a["abilities"][0].clone();
        });
        assert!(!matches!(
            compile(HEADER, &raw),
            CompileResult::Lowered { .. }
        ));
    }
    // Printed repetitions are retained, as for the other intrinsic keywords.
    let raw = tests::changed_source(MYR, |r| {
        let a = &mut r["faces"][0]["result"];
        a["abilities"][0]["keywords"]
            .as_array_mut()
            .unwrap()
            .push(json!({"keyword":"indestructible"}));
        a["lines"][0]["ability"] = a["abilities"][0].clone();
    });
    let CompileResult::Lowered { definition } = compile(HEADER, &raw) else {
        panic!("redundant intrinsic")
    };
    assert_eq!(definition.keywords, [KeywordAbility::Indestructible; 2]);
}

#[test]
fn indestructible_complete_token_and_static_grant_sources_reject() {
    for (raw, token) in [
        (
            include_str!("../../../tests/fixtures/hornet-queen.json"),
            true,
        ),
        (
            include_str!("../../../tests/fixtures/spidersilk-armor.json"),
            false,
        ),
    ] {
        assert!(matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
        let changed = tests::changed_source(raw, |r| {
            let result = &mut r["faces"][0]["result"];
            if token {
                result["abilities"][1]["effects"][0]["steps"][0]["token"]["keywords"][0] =
                    json!({"keyword":"indestructible"});
            } else {
                result["abilities"][0]["effect"]["modifiers"][1]["abilities"][0]["keyword"] =
                    json!({"keyword":"indestructible"});
            }
            let abilities = result["abilities"].as_array().unwrap().clone();
            for (i, a) in abilities.into_iter().enumerate() {
                result["lines"][i]["ability"] = a;
            }
        });
        assert!(
            matches!(
                compile(fixtures::MANIFEST, &changed),
                CompileResult::Unsupported { .. }
            ),
            "token={token}"
        );
    }
}
