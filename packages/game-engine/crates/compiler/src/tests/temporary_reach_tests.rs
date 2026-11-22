//! Two complete temporary-Reach sources, with an extra-untap source rejected.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const SNARE: &str =
    include_str!("../../../../tests/fixtures/7d011e22-335b-465f-9111-e95a72ebffe9.json");
const TOWER: &str =
    include_str!("../../../../tests/fixtures/58853200-efbe-41cc-87f1-720841308f4d.json");
const GRASP: &str =
    include_str!("../../../../tests/fixtures/021ce403-cbfa-4d69-a6e9-473d58bd477a.json");
#[test]
fn temporary_reach_complete_costs_atomic_instructions_and_extra_clause_rejection() {
    for (raw, index, generic, power, toughness, set) in [
        (SNARE, 18842u32, 0i64, 1i64, 1i64, false),
        (TOWER, 13288u32, 1i64, 0i64, 5i64, true),
    ] {
        let v: Value = serde_json::from_str(raw).unwrap();
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{:?}", compile(HEADER, raw))
        };
        d.validate().unwrap();
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(d.provenance.source_sha256, v["sourceSha256"]);
        assert_eq!(d.provenance.ast_sha256, v["astSha256"]);
        assert_eq!(d.provenance.binding_sha256, v["bindingSha256"]);
        assert_eq!(d.characteristics.mana_cost.generic, Quantity::from(generic));
        assert_eq!(d.characteristics.mana_cost.colored, vec![Color::G]);
        assert!(d.capabilities.contains(&Capability::ReachV1));
        let p = d.spell.as_ref().unwrap();
        assert_eq!(p.instructions.len(), 1);
        match &p.instructions[0] {
            Instruction::GrantKeywords {
                modifier: Some(m),
                keywords,
                ..
            } if !set => {
                assert_eq!(
                    (m.power.clone(), m.toughness.clone()),
                    (power.into(), toughness.into())
                );
                assert_eq!(keywords, &[KeywordAbility::Reach]);
            }
            Instruction::GrantKeywordsSet {
                set: ObjectSet::ControllerCreatures,
                modifier: Some(m),
                keywords,
                ..
            } if set => {
                assert_eq!(
                    (m.power.clone(), m.toughness.clone()),
                    (power.into(), toughness.into())
                );
                assert_eq!(keywords, &[KeywordAbility::Reach]);
            }
            other => panic!("{other:?}"),
        }
    }
    assert!(matches!(
        compile(HEADER, GRASP),
        CompileResult::Unsupported { .. }
    ));
}
#[test]
fn temporary_reach_strict_selectors_keywords_and_durations() {
    for raw in [SNARE, TOWER] {
        for case in 0..7 {
            let changed = tests::changed_source(raw, |r| {
                let a = &mut r["faces"][0]["result"]["abilities"][0];
                let step = &mut a["effects"][0]["steps"][1];
                match case {
                    0 => step["abilities"][0]["keyword"]["keyword"] = json!("defender"),
                    1 => step["abilities"][0]["keyword"]["cost"] = json!([]),
                    2 => step["duration"]["duration"] = json!("forever"),
                    3 => step["what"]["filter"]["types"] = json!(["land"]),
                    4 => step["what"]["filter"]["other"] = Value::Null,
                    5 => {
                        a["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}));
                    }
                    _ => a["effects"][0]["condition"] = Value::Null,
                };
                let copy = a.clone();
                r["faces"][0]["result"]["lines"][0]["ability"] = copy;
            });
            assert!(
                !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
                "{case}"
            );
        }
    }
}
