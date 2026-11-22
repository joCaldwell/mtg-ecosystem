//! Exact Pressure Point source and explicitly synthetic scope-consumption probes.
use super::*;
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const PRESSURE: &str =
    include_str!("../../../../tests/fixtures/d4bbdd0c-e578-49ed-b2f7-b1decb04ee87.json");

fn changed(text: &str, mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(PRESSURE, |record| {
        record["source"]["oracle_text"] = text.into();
        mutate(record);
        let source = &record["source"];
        let lines = source_lines(
            source["oracle_text"].as_str().unwrap(),
            source["name"].as_str().unwrap(),
            source["type_line"].as_str().unwrap(),
        )
        .unwrap();
        let result = &mut record["faces"][0]["result"];
        assert_eq!(lines.len(), result["abilities"].as_array().unwrap().len());
        result["lines"] = Value::Array(
            lines
                .into_iter()
                .enumerate()
                .map(|(i, text)| json!({"text":text,"ok":true,"ability":result["abilities"][i]}))
                .collect(),
        );
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

#[test]
fn tap_then_two_draws_is_an_ordinary_composed_sequence() {
    // A fixed draw of two is two single draws after the tap (CR 121.2).
    let raw = changed("Synthetic tap creature.\nSynthetic draw clause.", |r| {
        r["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["amount"]["value"] =
            2.into()
    });
    let d = tests::composed_near_miss(crate::tests::synthetic_compile(MANIFEST, &raw));
    let spell = d.spell.as_ref().unwrap();
    assert_eq!(
        tests::sole_target(spell),
        (
            TargetDomain::BattlefieldCreature,
            vec![],
            vec![],
            vec![],
            false,
            false
        )
    );
    assert_eq!(
        spell.instructions,
        [
            Instruction::tap_target(0, 0),
            Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 1),
            Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 1)
        ]
    );
}
