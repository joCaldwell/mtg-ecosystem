//! Complete frozen mill sources; malformed actors/bindings never grant semantics.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const TOME: &str =
    include_str!("../../../tests/fixtures/446a9d7e-9c1a-4e83-8342-2ed5acae2eed.json");
const THOUGHT: &str =
    include_str!("../../../tests/fixtures/83101ba8-a569-4827-8c53-9ca0dfcd59a7.json");
const MENTAL: &str =
    include_str!("../../../tests/fixtures/e8d5f31c-7abf-4fbb-977e-8353a97daf7a.json");
#[test]
fn fixed_mill_complete_sources_cost_order_targets_and_provenance() {
    for (raw, index, count, target, draw, sorcery, record) in [
        (
            TOME,
            10284,
            5,
            true,
            false,
            true,
            "8dd11d462d6e0939b982c9aac76b2fa03b8aada88fc5a7de27d603ae80f5ee8c",
        ),
        (
            THOUGHT,
            19711,
            2,
            true,
            true,
            false,
            "066ca75c12b7b3188e88a4800f5b9e6864bd4703beac3a19f8bc7b39733fb000",
        ),
        (
            MENTAL,
            35150,
            2,
            false,
            true,
            false,
            "7b0f3cb2fce415b3ca7eaa4b8d14acc5b967944ca239be228bd736038ab04167",
        ),
    ] {
        assert_eq!(sha256(raw.trim_end().as_bytes()), record);
        let v: Value = serde_json::from_str(raw).unwrap();
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{index}: {:?}", compile(HEADER, raw))
        };
        d.validate().unwrap();
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(d.provenance.source_sha256, v["sourceSha256"]);
        assert_eq!(d.provenance.ast_sha256, v["astSha256"]);
        assert_eq!(d.provenance.binding_sha256, v["bindingSha256"]);
        assert_eq!(v["source"]["legalities"]["commander"], "legal");
        assert_eq!(d.characteristics.mana_cost.generic, 0.into());
        assert_eq!(d.characteristics.mana_cost.colored, vec![Color::U]);
        let p = d.spell.as_ref().unwrap();
        assert!(mtg_contracts::fixed_mill_program(p));
        assert_eq!(p.targets.len(), usize::from(target));
        assert_eq!(
            p.instructions[0],
            Instruction::mill(
                if target {
                    PlayerExpr::target(0)
                } else {
                    PlayerExpr::you(Context::Spell)
                },
                Amount::Fixed {
                    value: count.into()
                },
                0
            )
        );
        assert_eq!(p.instructions.len(), if draw { 2 } else { 1 });
        let mut caps = BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::FixedMillProgramV1,
            if sorcery {
                Capability::CastSorceryV1
            } else {
                Capability::CastInstantV1
            },
        ]);
        if target {
            caps.insert(Capability::AnyPlayerTargetV1);
        }
        if draw {
            caps.insert(Capability::ControllerDrawV1);
            assert_eq!(
                p.instructions[1],
                Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 1)
            );
        }
        assert_eq!(d.capabilities, caps);
    }
}
#[test]
fn fixed_mill_rejects_null_qualified_actors_counts_sentences_and_binding_drift() {
    for raw in [TOME, THOUGHT, MENTAL] {
        for case in 0..12 {
            let changed = tests::changed_source(raw, |r| {
                let a = &mut r["faces"][0]["result"]["abilities"];
                match case {
                    0 => a[0]["effects"][0]["steps"][0]["who"]["of"] = Value::Null,
                    1 => a[0]["effects"][0]["steps"][0]["who"]["selectionId"] = Value::Null,
                    2 => a[0]["effects"][0]["steps"][0]["amount"]["value"] = json!(-1),
                    3 => a[0]["effects"][0]["steps"][0]["amount"]["extra"] = Value::Null,
                    4 => a[0]["effects"][0]["condition"] = Value::Null,
                    5 => a[0]["effects"][0]["conditionKind"] = Value::Null,
                    6 => a[0]["effects"][0]["bindings"] = json!([{"variable":"x","value":1}]),
                    7 => a[0]["effects"][0]["steps"][0]["amount"]["amount"] = json!("x"),
                    8 => {
                        let extra = a[0].clone();
                        a.as_array_mut().unwrap().push(extra);
                    }
                    9 => r["binding"]["references"][0]["target"] = json!("wrong"),
                    10 => {
                        r["binding"]["references"][0]["expression"] =
                            json!({"player":"each-player"})
                    }
                    _ => r["binding"]["counts"]["bound"] = json!(0),
                }
                let copied = r["faces"][0]["result"]["abilities"]
                    .as_array()
                    .unwrap()
                    .clone();
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
                "case{case}"
            );
        }
    }
}
