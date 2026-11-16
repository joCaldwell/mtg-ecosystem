//! Complete frozen ordinary Surveil sources; Flashback is a whole-source negative.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const CONSIDER: &str =
    include_str!("../../../tests/fixtures/4c9bcba6-87b5-4fb3-97ee-6fe5b739337d.json");
const CURATE: &str =
    include_str!("../../../tests/fixtures/75e8907d-f726-4a63-ba8b-c0fd4936734e.json");
const GAZE: &str =
    include_str!("../../../tests/fixtures/be668c2d-71ea-4346-8980-1fbf5e4cbed3.json");
#[test]
fn controller_surveil_complete_frozen_sources_and_flashback_rejection() {
    for (raw, index, record, count, generic) in [
        (
            CONSIDER,
            11525,
            "44a612ba6f76b6f576c29d5c5aed2da67c577145748f5e560568a0b3d065cb46",
            1,
            0,
        ),
        (
            CURATE,
            17744,
            "339915b7ecd94638f80df95dfb92c7c7ec02545b1a37f900db2c1e04fd1886a7",
            2,
            1,
        ),
    ] {
        assert_eq!(sha256(raw.trim_end().as_bytes()), record);
        let value: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(value["binding"]["fullyBound"], true);
        assert_eq!(value["source"]["legalities"]["commander"], "legal");
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("complete source")
        };
        d.validate().unwrap();
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(
            d.provenance.source_sha256,
            value["sourceSha256"].as_str().unwrap()
        );
        assert_eq!(
            d.provenance.ast_sha256,
            value["astSha256"].as_str().unwrap()
        );
        assert_eq!(
            d.provenance.binding_sha256,
            value["bindingSha256"].as_str().unwrap()
        );
        assert_eq!(d.characteristics.mana_cost.generic, Quantity::from(generic));
        assert_eq!(d.characteristics.mana_cost.colored, [Color::U]);
        let p = d.spell.as_ref().unwrap();
        assert!(mtg_contracts::controller_surveil_program(p));
        assert!(!mtg_contracts::controller_scry_program(p));
        assert_eq!(
            p.instructions,
            vec![
                Instruction::Surveil {
                    amount: count.into(),
                    source_block: 0
                },
                Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 1
                }
            ]
        );
        assert_eq!(
            d.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::CastInstantV1,
                Capability::ControllerSurveilV1,
                Capability::ControllerDrawV1
            ])
        );
    }
    assert_eq!(
        sha256(GAZE.trim_end().as_bytes()),
        "b70eb8fe7762ef5f16f7b296737622cb362585f93cbbb5315b0fa3ce47a2daec"
    );
    assert!(matches!(
        compile(HEADER, GAZE),
        CompileResult::Unsupported { .. }
    ));
}
#[test]
fn controller_surveil_rejects_qualified_or_unconsumed_source() {
    for raw in [CONSIDER, CURATE] {
        for case in 0..8 {
            let changed = tests::changed_source(raw, |r| {
                let abilities = r["faces"][0]["result"]["abilities"].as_array_mut().unwrap();
                match case {
                    0 => abilities[0]["effects"][0]["steps"][0]["amount"]["value"] = json!(-1),
                    1 => abilities[0]["effects"][0]["steps"][0]["who"] = Value::Null,
                    2 => abilities[0]["effects"][0]["condition"] = Value::Null,
                    3 => abilities[1]["effects"][0]["steps"][0]["who"]["selectionId"] = Value::Null,
                    4 => abilities[1]["effects"][0]["steps"][0]["who"]["of"] = Value::Null,
                    5 => abilities[0]["effects"][0]["steps"][0]["amount"]["extra"] = Value::Null,
                    6 => {
                        abilities[0]["effects"][0]["bindings"] = json!([{"variable":"x","value":1}])
                    }
                    _ => abilities.push(abilities[0].clone()),
                }
                let copied = abilities.clone();
                for (line, a) in r["faces"][0]["result"]["lines"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .filter(|l| l.get("ability").is_some())
                    .zip(copied)
                {
                    line["ability"] = a;
                }
            });
            assert!(
                !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
                "case {case}"
            );
        }
    }
}
