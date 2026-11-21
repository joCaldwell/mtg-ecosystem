//! Whole frozen Ponder; old search decoding keeps explicit absent operands.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const PONDER: &str =
    include_str!("../../../tests/fixtures/02090581-61aa-4348-ad57-451be8ee91c2.json");
#[test]
fn library_order_complete_ponder_cost_blocks_binding_and_legacy_search() {
    assert_eq!(
        sha256(PONDER.trim_end().as_bytes()),
        "714693853546130e0db54dc73aeb55eab00b4f17ebd53a266ee5bcdacd5f0eb1"
    );
    let v: Value = serde_json::from_str(PONDER).unwrap();
    let CompileResult::Lowered { definition: d } = compile(HEADER, PONDER) else {
        panic!("{:?}", compile(HEADER, PONDER))
    };
    d.validate().unwrap();
    assert_eq!(d.provenance.source_index, 331);
    assert_eq!(d.provenance.source_sha256, v["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, v["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, v["bindingSha256"]);
    assert_eq!(v["source"]["legalities"]["commander"], "legal");
    assert_eq!(d.characteristics.mana_cost.generic, 0.into());
    assert_eq!(d.characteristics.mana_cost.colored, vec![Color::U]);
    assert!(controller_library_order_program(d.spell.as_ref().unwrap()));
    assert_eq!(
        d.spell.as_ref().unwrap().instructions,
        vec![
            Instruction::OrderLibraryTop {
                amount: 3.into(),
                source_block: 0
            },
            Instruction::OptionalShuffleLibrary { source_block: 0 },
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
            Capability::CastSorceryV1,
            Capability::ControllerLibraryOrderV1,
            Capability::ControllerDrawV1
        ])
    );
    for (name, raw) in fixtures::STAPLES_RECORDS
        .iter()
        .filter(|(n, _)| matches!(*n, "Rampant Growth" | "Explosive Vegetation"))
    {
        assert!(
            matches!(
                compile(fixtures::MANIFEST, raw),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
}
#[test]
fn library_order_rejects_qualified_null_partial_and_wrong_binding_source() {
    for case in 0..14 {
        let near_miss = case == 8;
        let changed = tests::changed_source(PONDER, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"];
            match case {
                0 => a[0]["effects"][0]["steps"][0]["who"]["of"] = Value::Null,
                1 => a[0]["effects"][0]["steps"][0]["what"]["top"]["value"] = json!(-1),
                2 => a[0]["effects"][0]["steps"][1]["what"]["ref"] = json!("all"),
                3 => a[0]["effects"][0]["steps"][1]["order"] = json!("random"),
                4 => a[0]["effects"][1]["steps"][0]["who"]["of"] = Value::Null,
                5 => a[0]["effects"][1]["steps"][0]["optional"] = Value::Null,
                6 => a[0]["effects"][1]["steps"][0]["optional"] = json!(false),
                7 => a[0]["effects"][0]["condition"] = Value::Null,
                8 => a[1]["effects"][0]["steps"][0]["amount"]["value"] = json!(2),
                9 => a[1]["effects"][0]["steps"][0]["who"]["selectionId"] = Value::Null,
                10 => {
                    a.as_array_mut().unwrap().pop();
                }
                11 => r["binding"]["references"][3]["target"] = json!("wrong-pool"),
                12 => {
                    let n = r["binding"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|n| n["operation"] == "shuffle")
                        .unwrap();
                    n["decider"] = json!("other-player");
                }
                _ => r["binding"]["counts"]["bound"] = json!(7),
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
        if near_miss {
            // A fixed draw of two after the same ordering is an ordinary
            // sequence: the block pipeline lowers it, marked composed, as two
            // single draws (CR 121.2) with every other instruction unchanged.
            let d = tests::composed_near_miss(compile(HEADER, &changed));
            let draw = Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 1,
            };
            assert_eq!(
                d.spell.as_ref().unwrap().instructions,
                vec![
                    Instruction::OrderLibraryTop {
                        amount: 3.into(),
                        source_block: 0
                    },
                    Instruction::OptionalShuffleLibrary { source_block: 0 },
                    draw.clone(),
                    draw
                ]
            );
            continue;
        }
        assert!(
            !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
            "case {case}"
        );
    }
}
