//! Whole frozen Ponder; old search decoding keeps explicit absent operands.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const BRAINSTORM: &str =
    include_str!("../../../../tests/fixtures/36cd2364-d113-47d1-b2c4-b088d9eb88dd.json");
#[test]
fn hand_return_complete_source_cost_bindings_and_program() {
    let CompileResult::Lowered { definition: d } = compile(HEADER, BRAINSTORM) else {
        panic!("{:?}", compile(HEADER, BRAINSTORM))
    };
    d.validate().unwrap();
    assert!(controller_hand_return_program(d.spell.as_ref().unwrap()));
    assert_eq!(d.provenance.source_index, 8170);
    assert_eq!(d.characteristics.mana_cost.colored, vec![Color::U]);
    assert_eq!(
        d.spell.unwrap().instructions,
        vec![
            Instruction::draw(PlayerExpr::you(Context::Spell), 3.into(), 0),
            Instruction::PutHandOnLibrary {
                count: 2.into(),
                source_block: 0
            }
        ]
    );
}
#[test]
fn hand_return_rejects_qualified_partial_wrong_domain_and_binding() {
    for case in 0..12 {
        let changed = tests::changed_source(BRAINSTORM, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0]["effects"][0];
            match case {
                0 => a["steps"][0]["who"]["of"] = Value::Null,
                1 => a["steps"][1]["what"]["filter"]["zone"]["owner"] = json!("opponent"),
                2 => a["steps"][1]["what"]["ref"] = json!("them"),
                3 => a["steps"][1]["order"] = json!("random"),
                4 => a["steps"][1]["what"]["count"]["value"] = json!(-1),
                5 => a["steps"][1]["to"]["position"] = json!("bottom"),
                6 => a["condition"] = Value::Null,
                7 => {
                    a["steps"].as_array_mut().unwrap().pop();
                }
                8 => a["steps"][0]["optional"] = json!(true),
                9 => r["binding"]["references"][3]["target"] = json!("drawn-only"),
                10 => r["binding"]["nodes"][7]["timing"] = json!("announcement"),
                _ => r["binding"]["counts"]["bound"] = json!(4),
            };
            let copy = r["faces"][0]["result"]["abilities"][0].clone();
            r["faces"][0]["result"]["lines"][0]["ability"] = copy;
        });
        assert!(
            !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
            "{case}"
        );
    }
}
