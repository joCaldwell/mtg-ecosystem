//! Complete frozen Careful Study; no subset or drawn-only hand binding.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const STUDY: &str =
    include_str!("../../../tests/fixtures/32e9aa23-fc0a-4b82-82f9-1659c304428c.json");
#[test]
fn draw_discard_complete_source_cost_bindings_and_program() {
    let CompileResult::Lowered { definition: d } = compile(HEADER, STUDY) else {
        panic!("{:?}", compile(HEADER, STUDY))
    };
    d.validate().unwrap();
    assert!(controller_draw_discard_program(d.spell.as_ref().unwrap()));
    assert_eq!(d.provenance.source_index, 7578);
    assert_eq!(d.characteristics.mana_cost.colored, vec![Color::U]);
    assert_eq!(
        d.spell.unwrap().instructions,
        vec![
            Instruction::draw(PlayerExpr::you(Context::Spell), 2.into(), 0),
            Instruction::discard(
                kir::PlayerExpr::you(kir::Context::Spell),
                kir::Amount::Fixed { value: 2.into() },
                false,
                0
            )
        ]
    );
}
#[test]
fn draw_discard_accepts_one_card_draw_and_discard_spell() {
    let one = tests::changed_source(STUDY, |r| {
        let text = "Draw a card, then discard a card.";
        r["source"]["oracle_text"] = json!(text);
        let result = &mut r["faces"][0]["result"];
        result["lines"][0]["text"] = json!(text);
        let steps = &mut result["abilities"][0]["effects"][0]["steps"];
        steps[0]["amount"]["value"] = json!(1);
        steps[1]["what"]["count"]["value"] = json!(1);
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        let b = &mut r["binding"];
        for node in b["nodes"].as_array_mut().unwrap().iter_mut() {
            match node["id"].as_str() {
                Some(id)
                    if id.ends_with("/steps/0#objects") || id.ends_with("/steps/1#objects") =>
                {
                    node["cardinality"] = json!("one");
                }
                Some(id) if id.ends_with("/steps/1/what#query") => {
                    node["cardinality"] = json!("one");
                    node["expression"]["count"]["value"] = json!(1);
                }
                _ => {}
            }
        }
        b["nodes"].as_array_mut().unwrap().retain(|node| {
            !node["id"]
                .as_str()
                .is_some_and(|id| id.ends_with("/steps/1#quantity"))
        });
        b["references"][2]["expression"]["count"]["value"] = json!(1);
    });
    let compiled = compile(HEADER, &one);
    let CompileResult::Lowered { definition: d } = compiled else {
        panic!("{compiled:?}")
    };
    d.validate().unwrap();
    let program = d.spell.as_ref().unwrap();
    assert!(controller_draw_discard_program(program));
    assert!(
        d.capabilities
            .contains(&Capability::ControllerDrawDiscardV1)
    );
    assert!(!d.capabilities.contains(&Capability::ControllerDrawV1));
    assert_eq!(
        program.instructions,
        vec![
            Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 0),
            Instruction::discard(
                kir::PlayerExpr::you(kir::Context::Spell),
                kir::Amount::Fixed { value: 1.into() },
                false,
                0
            )
        ]
    );
}
#[test]
fn draw_discard_rejects_null_wrong_pool_partial_and_binding() {
    for case in 0..12 {
        let changed = tests::changed_source(STUDY, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0]["effects"][0];
            match case {
                0 => a["steps"][0]["who"]["of"] = Value::Null,
                1 => a["steps"][1]["who"]["of"] = Value::Null,
                2 => a["steps"][1]["what"]["ref"] = json!("them"),
                3 => a["steps"][1]["what"]["filter"]["types"] = json!(["creature"]),
                4 => a["steps"][1]["what"]["count"]["value"] = json!(-1),
                5 => a["steps"][1]["who"]["player"] = json!("opponent"),
                6 => a["condition"] = Value::Null,
                7 => {
                    a["steps"].as_array_mut().unwrap().pop();
                }
                8 => a["steps"][0]["optional"] = json!(true),
                9 => r["binding"]["references"][2]["target"] = json!("drawn-only"),
                10 => r["binding"]["nodes"][7]["timing"] = json!("announcement"),
                _ => r["binding"]["counts"]["bound"] = json!(2),
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
