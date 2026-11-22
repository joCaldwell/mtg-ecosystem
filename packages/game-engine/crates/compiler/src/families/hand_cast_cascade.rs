//! CR 109.5, 601.2i, 611.2f, 702.85a. Complete hand-cast cascade source.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
#[cfg(test)]
use mtg_contracts::hand_cast_cascade::hand_cast_cascade_ability;
use mtg_contracts::hand_cast_cascade::{hand_cast_cascade_ast, hand_cast_cascade_source};
use serde_json::{Value, json};

pub(crate) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<Vec<Ability>>, Failure> {
    if !source
        .oracle_text
        .contains("as you cast spells from your hand this turn")
        && !result.abilities.get().contains("grant-cast-keyword")
    {
        return Ok(None);
    }
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    if !hand_cast_cascade_source(&source.oracle_text, &source.name)
        || ast != hand_cast_cascade_ast()
    {
        return Err(unsupported(
            "abilities.hand_cast_cascade",
            "the complete self-combat-damage cascade grant is required",
        ));
    }
    let mut graph = Bindings::new(&source.oracle_id, shape);
    graph.node("/faces/0/abilities/0", "ability", "ability", json!({"source":graph.id("/source", "object"), "face":graph.id("/faces/0", "face"), "abilityKind":"keywords", "you":graph.id("/faces/0/abilities/0", "you"), "rules":["113", "109.5"]}));
    graph.node("/faces/0/abilities/0", "you", "runtime-player", json!({"scope":graph.id("/faces/0/abilities/0", "ability"), "role":"static-source-controller", "rules":["109.5"]}));
    graph.node("/faces/0/abilities/0/keywords/0", "you", "runtime-player", json!({"scope":graph.id("/faces/0/abilities/0", "ability"), "role":"static-source-controller", "rules":["109.5", "702.19"]}));
    graph.node("/faces/0/abilities/0/keywords/0", "keyword", "keyword-ability", json!({"scope":graph.id("/faces/0/abilities/0", "ability"), "keyword":"trample", "abilities":["static"], "you":graph.id("/faces/0/abilities/0/keywords/0", "you"), "rules":["702.19"]}));
    graph.node("/faces/0/abilities/1", "ability", "ability", json!({"source":graph.id("/source", "object"), "face":graph.id("/faces/0", "face"), "abilityKind":"triggered", "you":graph.id("/faces/0/abilities/1", "you"), "rules":["113", "109.5"]}));
    graph.node("/faces/0/abilities/1", "you", "runtime-player", json!({"scope":graph.id("/faces/0/abilities/1", "ability"), "role":"trigger-controller", "rules":["109.5"]}));
    graph.node("/faces/0/abilities/1/trigger", "event", "event", json!({"scope":graph.id("/faces/0/abilities/1", "ability"), "operation":"trigger:deals-damage", "inputs":[graph.id("/source", "object")], "availability":"when-performed", "rules":["608.2c"]}));
    graph.node("/faces/0/abilities/1/trigger", "objects", "event-result", json!({"event":graph.id("/faces/0/abilities/1/trigger", "event"), "result":"objects", "cardinality":"one", "availability":"if-produced", "identity":"same-objects", "rules":["608.2h"]}));
    graph.node("/faces/0/abilities/1/trigger", "player", "event-result", json!({"event":graph.id("/faces/0/abilities/1/trigger", "event"), "result":"player", "cardinality":"one", "availability":"if-produced", "identity":"same-objects", "rules":["608.2h"]}));
    graph.node("/faces/0/abilities/1/trigger", "quantity", "event-result", json!({"event":graph.id("/faces/0/abilities/1/trigger", "event"), "result":"quantity", "cardinality":"one", "availability":"if-produced", "identity":"measured-quantity", "rules":["608.2h"]}));
    graph.node("/faces/0/abilities/1/effects/0/steps/0", "grant", "cast-keyword-grant", json!({"scope":graph.id("/faces/0/abilities/1", "ability"), "created":"resolution", "duration":"this-turn", "keyword":"cascade", "rules":["113.7a", "514.2", "603.3a", "611.2a", "611.2c", "611.2f"]}));
    graph.node("/faces/0/abilities/1/effects/0/steps/0/what", "spells", "future-cast-spells", json!({"grant":graph.id("/faces/0/abilities/1/effects/0/steps/0", "grant"), "caster":graph.id("/faces/0/abilities/1", "you"), "from":{"zone":"hand", "owner":graph.id("/faces/0/abilities/1", "you")}, "timing":"as-cast", "cardinality":"many", "rules":["601.2a", "601.2i", "611.2f"]}));
    graph.reference_rules(
        "/faces/0/abilities/1/trigger/what",
        &json!({"ref":"self"}),
        &graph.id("/source", "object"),
        &["201.5"],
    );
    graph.reference_rules(
        "/faces/0/abilities/1/effects/0/steps/0/caster",
        &json!({"player":"you"}),
        &graph.id("/faces/0/abilities/1", "you"),
        &["109.5"],
    );
    graph.reference_rules(
        "/faces/0/abilities/1/effects/0/steps/0/from/owner",
        &json!({"player":"you"}),
        &graph.id("/faces/0/abilities/1", "you"),
        &["109.5"],
    );
    graph.reference_rules(
        "/faces/0/abilities/1/effects/0/steps/0/what",
        &json!({"recipient":"they"}),
        &graph.id("/faces/0/abilities/1/effects/0/steps/0/what", "spells"),
        &["601.2a", "611.2f", "702.85a"],
    );
    graph.validate_with_features(binding, &[json!({"feature":"cast-keyword-grant", "path":"/faces/0/abilities/1/effects/0/steps/0", "message":"Keep one source-independent grant per resolution and expire it during cleanup. Apply each grant to that controller's later completed casts from hand before collecting cast triggers.", "rules":["109.5", "113.7a", "514.2", "601.2i", "603.3a", "611.2a", "611.2f", "702.85a", "702.85c"]})], "incomplete hand-cast cascade binding graph")?;
    let mut abilities = read_abilities(&json!([ast[0]]).to_string())?;
    abilities.push(Ability::HandCastCascade);
    Ok(Some(abilities))
}

#[test]
fn hand_cast_cascade() {
    let old =
        include_str!("../../../../tests/fixtures/ramos/9efe8aff-9a7d-4397-b5fd-c1a0fad7c15f.json");
    let old_manifest =
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    assert!(
        matches!(compile(old_manifest,old), CompileResult::Unsupported {path,..} if path=="status")
    );
    let Some(path) = std::env::var_os("MTG_GRANT_HAND_CAST_CASCADE_ARCHIVE_PROBE") else {
        return;
    };
    let sheet = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        sha256(sheet.as_bytes()),
        "1797d0c1ae2b3988e92038d42efd75aebd952c9ad259711e4e2d06d84949ecb8"
    );
    let lines = sheet.lines().collect::<Vec<_>>();
    let [manifest, record] = lines.as_slice() else {
        panic!("expected one exact source")
    };
    assert_eq!(
        sha256(record.as_bytes()),
        "e14b03d2b0721bcdde1a106d0b2966c0713bdf963e2b3d03d10d71c3d2533904"
    );
    let result = compile(manifest, record);
    let CompileResult::Lowered { definition: d } = result else {
        panic!("{result:?}")
    };
    assert_eq!(d.triggered, [hand_cast_cascade_ability()]);
    assert_eq!(d.keywords, [KeywordAbility::Trample]);
    assert_eq!(
        d.characteristics.mana_cost,
        mana_cost("{U}{B}{R}{G}").unwrap()
    );
    assert!(d.characteristics.legendary);
    let bad = tests::changed_source(record, |r| {
        r["binding"]["nodes"][13]["from"]["zone"] = json!("exile");
    });
    assert!(
        matches!(compile(manifest,&bad),CompileResult::Unsupported {path,..} if path=="binding")
    );
}
