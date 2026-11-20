//! Complete larger draws; legacy unrolled draws retain their IR and capabilities.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const HARMONIZE: &str =
    include_str!("../../../tests/fixtures/7eff84f1-f772-497a-b350-bbc93d0230f7.json");
const CONCENTRATE: &str =
    include_str!("../../../tests/fixtures/a3d287ec-1a39-4f91-acf1-7c1d2c00ed89.json");
const TIDINGS: &str =
    include_str!("../../../tests/fixtures/72897780-094d-4a21-8b1c-419a9defd2fb.json");
const INGENUITY: &str =
    include_str!("../../../tests/fixtures/ce588fd3-642e-4df9-8061-6fcfc6e81270.json");
const DIVINATION: &str =
    include_str!("../../../tests/fixtures/273b339c-964b-4a18-8eb5-ceb8abcdfd9e.json");
const MISTS: &str =
    include_str!("../../../tests/fixtures/c81ca8ff-92f3-481e-82f7-0673b6c74ea0.json");
const INSPIRATION: &str =
    include_str!("../../../tests/fixtures/8f32ceb2-92c2-4dde-bf73-40bb79c3fcef.json");
const BONES: &str =
    include_str!("../../../tests/fixtures/5bf4d8d9-a2b2-4dba-ac05-9d4470a89db2.json");
#[test]
fn controller_counted_draw_complete_costs_quantities_and_extra_clause_rejection() {
    for (raw, n, generic, color) in [
        (HARMONIZE, 3i64, 2i64, Color::G),
        (CONCENTRATE, 3, 2, Color::U),
        (TIDINGS, 4, 3, Color::U),
        (INGENUITY, 3, 3, Color::U),
    ] {
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{:?}", compile(HEADER, raw))
        };
        let pin: Value = serde_json::from_str(raw).unwrap();
        d.validate().unwrap();
        assert_eq!(d.characteristics.mana_cost.generic, Quantity::from(generic));
        assert_eq!(d.characteristics.mana_cost.colored, vec![color, color]);
        assert_eq!(d.provenance.source_sha256, pin["sourceSha256"]);
        assert_eq!(d.provenance.ast_sha256, pin["astSha256"]);
        assert_eq!(d.provenance.binding_sha256, pin["bindingSha256"]);
        assert!(
            d.capabilities
                .contains(&Capability::ControllerCountedDrawV1)
        );
        assert!(!d.capabilities.contains(&Capability::ControllerDrawV1));
        assert_eq!(
            d.spell.as_ref().unwrap().instructions,
            vec![Instruction::DrawControllerCards {
                count: n.into(),
                source_block: 0
            }]
        );
        assert!(controller_counted_draw_program(d.spell.as_ref().unwrap()));
    }
    // The complete sequence is supported. It must still reject a parse
    // that drops the printed life loss after scrying and drawing.
    let incomplete = tests::changed_source(BONES, |r| {
        let result = &mut r["faces"][0]["result"];
        result["abilities"][0]["effects"]
            .as_array_mut()
            .unwrap()
            .truncate(1);
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    });
    let result = compile(HEADER, &incomplete);
    assert!(
        matches!(&result, CompileResult::Unsupported { path, reason }
            if path == "scalar sequence" && reason == "requires complete ordinary printed and typed steps"),
        "{result:?}"
    );
}
#[test]
fn controller_counted_draw_preserves_legacy_one_two_and_targeted_draws() {
    for (raw, n, target) in [
        (MISTS, 1usize, false),
        (DIVINATION, 2, false),
        (INSPIRATION, 2, true),
    ] {
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{:?}", compile(HEADER, raw))
        };
        d.validate().unwrap();
        let p = d.spell.as_ref().unwrap();
        assert_eq!(p.instructions.len(), n);
        assert!(
            !d.capabilities
                .contains(&Capability::ControllerCountedDrawV1)
        );
        assert!(!controller_counted_draw_program(p));
        for i in &p.instructions {
            assert_eq!(
                *i,
                Instruction::DrawCards {
                    recipient: target.then_some(LifeRecipient::Target { target_slot: 0 }),
                    count: 1.into(),
                    source_block: 0
                }
            )
        }
        assert_eq!(p.targets.len(), usize::from(target));
    }
}
#[test]
fn controller_counted_draw_strict_qualifiers_bindings_and_truncated_source_reject() {
    for case in 0..10 {
        let changed = tests::changed_source(HARMONIZE, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            let step = &mut a["effects"][0]["steps"][0];
            match case {
                0=>step["who"]["selectionId"]=Value::Null,
                1=>step["who"]["of"]=Value::Null,
                2=>step["who"]["player"]=json!("each-player"),
                3=>step["optional"]=json!(true),
                4=>step["amount"]["value"]=json!(-1),
                5=>a["effects"][0]["condition"]=Value::Null,
                6=>a["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})),
                7=>r["binding"]["references"][0]["target"]=json!("foreign"),
                8=>r["binding"]["nodes"][3]["role"]=json!("event-player"),
                _=>r["binding"]["references"]=json!([]),
            }
            let copy = r["faces"][0]["result"]["abilities"][0].clone();
            r["faces"][0]["result"]["lines"][0]["ability"] = copy;
        });
        assert!(
            !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
            "{case}"
        );
    }
    for raw in [
        include_str!("../../../tests/fixtures/36cd2364-d113-47d1-b2c4-b088d9eb88dd.json"),
        include_str!("../../../tests/fixtures/32e9aa23-fc0a-4b82-82f9-1659c304428c.json"),
    ] {
        let changed = tests::changed_source(raw, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"]
                .as_array_mut()
                .unwrap()
                .truncate(1);
            let copy = r["faces"][0]["result"]["abilities"][0].clone();
            r["faces"][0]["result"]["lines"][0]["ability"] = copy;
        });
        assert!(!matches!(
            compile(HEADER, &changed),
            CompileResult::Lowered { .. }
        ));
    }
}

const CRAVING: &str =
    include_str!("../../../tests/fixtures/78725353-9274-420a-b722-add0f43c444e.json");
const AMBITION: &str =
    include_str!("../../../tests/fixtures/84de4fec-2f38-4293-93d3-b3882c5aac14.json");
#[test]
fn controller_counted_draw_life_complete_sources_costs_and_extra_clause_rejection() {
    for raw in [CRAVING, AMBITION] {
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{:?}", compile(HEADER, raw))
        };
        let pin: Value = serde_json::from_str(raw).unwrap();
        d.validate().unwrap();
        assert_eq!(d.characteristics.mana_cost.generic, 3.into());
        assert_eq!(d.characteristics.mana_cost.colored, vec![Color::B]);
        assert_eq!(d.provenance.source_sha256, pin["sourceSha256"]);
        assert_eq!(d.provenance.ast_sha256, pin["astSha256"]);
        assert_eq!(d.provenance.binding_sha256, pin["bindingSha256"]);
        assert!(
            d.capabilities
                .contains(&Capability::ControllerCountedDrawV1)
        );
        assert!(d.capabilities.contains(&Capability::LifeLossV1));
        assert_eq!(
            d.spell.as_ref().unwrap().instructions,
            vec![
                Instruction::DrawControllerCards {
                    count: 3.into(),
                    source_block: 0
                },
                Instruction::LoseLife {
                    recipient: LifeRecipient::Controller,
                    amount: 3.into(),
                    source_block: 0
                }
            ]
        );
    }
    assert!(matches!(
        compile(
            HEADER,
            include_str!("../../../tests/fixtures/fde459b2-f45f-40d2-b328-58f65440f494.json")
        ),
        CompileResult::Unsupported { .. }
    ));
}
#[test]
fn controller_counted_draw_life_preserves_legacy_two_card_combinations() {
    for (raw, targeted) in [
        (
            include_str!("../../../tests/fixtures/7ffae8f8-3006-4969-a339-6d30678f87ea.json"),
            false,
        ),
        (
            include_str!("../../../tests/fixtures/c8e21bc4-ea57-46d6-9e7e-ad95e17ea0fd.json"),
            false,
        ),
        (
            include_str!("../../../tests/fixtures/c6207f6a-a624-4754-88f5-dbe700c841ff.json"),
            true,
        ),
    ] {
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{:?}", compile(HEADER, raw))
        };
        assert!(
            !d.capabilities
                .contains(&Capability::ControllerCountedDrawV1)
        );
        let p = d.spell.as_ref().unwrap();
        assert_eq!(p.instructions.len(), 3);
        let recipient = if targeted {
            LifeRecipient::Target { target_slot: 0 }
        } else {
            LifeRecipient::Controller
        };
        for i in &p.instructions[..2] {
            assert_eq!(
                *i,
                Instruction::DrawCards {
                    recipient: targeted.then_some(recipient.clone()),
                    count: 1.into(),
                    source_block: 0
                }
            );
        }
        assert_eq!(
            p.instructions[2],
            Instruction::LoseLife {
                recipient,
                amount: 2.into(),
                source_block: 0
            }
        );
    }
}
#[test]
fn controller_counted_draw_life_rejects_null_qualifiers_and_partial_bindings() {
    for case in 0..12 {
        let changed = tests::changed_source(CRAVING, |r| {
            let effects = &mut r["faces"][0]["result"]["abilities"][0]["effects"];
            match case {
                0 => effects[0]["steps"][1]["who"]["selectionId"] = Value::Null,
                1 => effects[0]["steps"][1]["who"]["of"] = Value::Null,
                2 => effects[0]["steps"][1]["who"]["player"] = json!("each-opponent"),
                3 => effects[0]["steps"][1]["optional"] = json!(true),
                4 => effects[0]["steps"][1]["amount"]["value"] = json!(0),
                5 => effects[0]["steps"][1]["amount"]["value"] = json!(-1),
                6 => effects[0]["steps"].as_array_mut().unwrap().reverse(),
                7 => {
                    let loss = effects[0]["steps"].as_array_mut().unwrap().pop().unwrap();
                    effects
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"steps":[loss]}));
                }
                8 => effects[0]["condition"] = Value::Null,
                9 => r["binding"]["references"][1]["target"] = json!("foreign"),
                10 => {
                    for n in r["binding"]["nodes"].as_array_mut().unwrap() {
                        if n["operation"] == "lose-life" {
                            n["operation"] = json!("gain-life");
                        }
                    }
                }
                _ => effects[0]["steps"].as_array_mut().unwrap().truncate(1),
            }
            let copy = r["faces"][0]["result"]["abilities"][0].clone();
            r["faces"][0]["result"]["lines"][0]["ability"] = copy;
        });
        assert!(
            !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
            "{case}"
        );
    }
}
