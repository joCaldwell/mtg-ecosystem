//! Exact complete five-source selection family and binding-presence controls.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const SOURCES: [&str; 5] = [
    include_str!("../../../tests/fixtures/02b5acf3-47cb-4d39-9307-e02656f1879b.json"),
    include_str!("../../../tests/fixtures/05a039ad-1689-4d13-b393-6c1219583b5d.json"),
    include_str!("../../../tests/fixtures/9b97a7fe-c5d9-4698-a49c-183b3f1ce4db.json"),
    include_str!("../../../tests/fixtures/a97da5a9-17fc-4367-81e1-17ff81601a63.json"),
    include_str!("../../../tests/fixtures/f6bd2902-7f8b-419e-bbc7-bcab0c1b7e01.json"),
];
#[test]
fn library_selection_five_complete_sources_costs_provenance_and_order() {
    for (i, raw) in SOURCES.iter().enumerate() {
        let v: Value = serde_json::from_str(raw).unwrap();
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{i}: {:?}", compile(HEADER, raw))
        };
        d.validate().unwrap();
        assert_eq!(v["source"]["legalities"]["commander"], "legal");
        assert_eq!(d.provenance.source_sha256, v["sourceSha256"]);
        assert_eq!(d.provenance.ast_sha256, v["astSha256"]);
        assert_eq!(d.provenance.binding_sha256, v["bindingSha256"]);
        let p = d.spell.as_ref().unwrap();
        assert!(controller_library_selection_program(p));
        let amount = [3, 2, 3, 3, 4][i];
        let remainder = if i == 0 || i == 2 {
            LibraryRemainder::Graveyard
        } else {
            LibraryRemainder::Bottom
        };
        assert_eq!(
            p.instructions,
            vec![Instruction::SelectLibraryCard {
                amount: amount.into(),
                remainder,
                source_block: 0
            }]
        );
        assert_eq!(
            d.characteristics.mana_cost.generic,
            if i == 1 { 0.into() } else { 1.into() }
        );
        assert_eq!(
            d.characteristics.mana_cost.colored,
            vec![if i == 2 { Color::B } else { Color::U }]
        );
        assert_eq!(
            d.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::ControllerLibrarySelectionV1,
                if i < 3 {
                    Capability::CastSorceryV1
                } else {
                    Capability::CastInstantV1
                }
            ])
        );
    }
}
#[test]
fn library_selection_rejects_partial_qualified_and_unbound_source() {
    for raw in SOURCES {
        for case in 0..15 {
            let changed = tests::changed_source(raw, |r| {
                let a = &mut r["faces"][0]["result"]["abilities"];
                match case {
                    0 => a[0]["effects"][0]["steps"][0]["who"]["of"] = Value::Null,
                    1 => a[0]["effects"][0]["steps"][0]["what"]["of"]["owner"] = json!("opponent"),
                    2 => a[0]["effects"][0]["steps"][0]["what"]["top"]["value"] = json!(-1),
                    3 => a[0]["effects"][0]["condition"] = Value::Null,
                    4 => a[0]["abilityWord"] = Value::Null,
                    5 => {
                        a[0]["effects"][1]["steps"][0]["steps"][0]["what"]["count"]["value"] =
                            json!(2)
                    }
                    6 => {
                        a[0]["effects"][1]["steps"][0]["steps"][0]["to"]["owner"] =
                            json!("its-owner")
                    }
                    7 => a[0]["effects"][1]["steps"][0]["steps"][1]["what"]["ref"] = json!("them"),
                    8 => a[0]["effects"][1]["steps"][0]["steps"][1]["order"] = json!("random"),
                    9 => a[0]["effects"][1]["steps"][0]["steps"][0]["tapped"] = Value::Null,
                    10 => a[0]["effects"][1]["bindings"] = json!([{"variable":"x","value":1}]),
                    11 => {
                        let extra = a[0].clone();
                        a.as_array_mut().unwrap().push(extra);
                    }
                    12 => r["binding"]["references"][3]["target"] = json!("wrong-pool"),
                    13 => {
                        let n = r["binding"]["nodes"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|n| n["node"] == "remainder")
                            .unwrap();
                        n["excluding"] = json!("wrong-selection");
                    }
                    _ => r["binding"]["counts"]["obligations"] = json!(0),
                }
                let copy = r["faces"][0]["result"]["abilities"]
                    .as_array()
                    .unwrap()
                    .clone();
                for (line, a) in r["faces"][0]["result"]["lines"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .filter(|l| l.get("ability").is_some())
                    .zip(copy)
                {
                    line["ability"] = a;
                }
            });
            assert!(
                crate::tests::family_or_source_rejects(&compile(HEADER, &changed)),
                "case {case}"
            );
        }
    }
}
