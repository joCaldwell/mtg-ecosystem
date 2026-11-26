//! Compiler unit tests: exact frozen rows and targeted shape/rejection cases.
use super::*;

mod board_filter_tests;
mod operand_tests;
mod r4_binding_tests;

#[test]
fn bounded_token_types_preserve_single_type_bytes() {
    let mut source: SourceToken = serde_json::from_value(serde_json::json!({
        "types":["artifact","creature"], "supertypes":[], "subtypes":["Latticekin"],
        "colors":[], "keywords":[], "power":{"amount":"fixed","value":3},
        "toughness":{"amount":"fixed","value":7}
    }))
    .unwrap();
    let token = lower_creature_token(&source).unwrap();
    assert!(token.characteristics().has_type(CardKind::Artifact));
    source.types.reverse();
    assert_eq!(lower_creature_token(&source).unwrap(), token);
    source.types.pop();
    let single = lower_creature_token(&source).unwrap();
    let bytes = serde_json::to_value(&single).unwrap();
    assert!(bytes.get("additional_types").is_none());
    assert_eq!(
        serde_json::from_value::<CreatureToken>(bytes).unwrap(),
        single
    );
    assert!(!single.characteristics().has_type(CardKind::Artifact));
    source.types.push("land".into());
    assert!(lower_creature_token(&source).is_err());
}

pub(super) fn changed_source(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut r: serde_json::Value = serde_json::from_str(raw).unwrap();
    change(&mut r);
    r["sourceSha256"] = sha256(canonical(&r["source"]).unwrap().as_bytes()).into();
    r["bindingSha256"] = sha256(canonical(&r["binding"]).unwrap().as_bytes()).into();
    if r.get("resolution").is_some() {
        r["resolutionSha256"] = sha256(canonical(&r["resolution"]).unwrap().as_bytes()).into();
    }
    let abilities = serde_json::json!([r["faces"][0]["result"]["abilities"]]);
    r["astSha256"] = sha256(canonical(&abilities).unwrap().as_bytes()).into();
    canonical(&r).unwrap()
}

#[test]
fn rejects_hash_version_unknown_field_and_incomplete_face() {
    let raw = include_str!("../../../../tests/fixtures/phase1-r5/unsummon.json");
    for bad in [
        raw.replace("\"status\":\"parsed\"", "\"status\":\"unsupported\""),
        raw.replacen(
            "\"sourceIndex\":",
            "\"extraSemanticField\":true,\"sourceIndex\":",
            1,
        ),
        raw.replace("Return target creature", "Destroy target creature"),
        raw.replace("\"sourceFaceIndex\":0", "\"sourceFaceIndex\":1"),
    ] {
        assert!(!matches!(
            compile(fixtures::MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
    }
    let bad = fixtures::MANIFEST.replace("\"schemaVersion\":4", "\"schemaVersion\":3");
    assert!(matches!(compile(&bad, raw), CompileResult::Invalid { .. }));
}

#[test]
fn synthetic_cast_color_uses_spell_characteristics() {
    let filter: SourceTypeFilter =
        serde_json::from_str(r#"{"cls":"spell","colors":["blue"]}"#).unwrap();
    let predicate = lower_type_predicate(&filter, Some("spell"), true, 0).unwrap();
    let mut spell: Characteristics = serde_json::from_value(serde_json::json!({
        "kind":"instant", "subtypes":[], "basic":false, "legendary":false,
        "mana_cost":{"generic":"0","colored":[]}, "colors":["R"], "color_identity":["R","U"],
        "power":null, "toughness":null
    }))
    .unwrap();
    assert_eq!(predicate, Pred(vec![Atom::Color { color: Color::U }]));
    assert!(!spell.colors.contains(&Color::U));
    spell.colors.push(Color::U);
    assert!(spell.colors.contains(&Color::U));
}
