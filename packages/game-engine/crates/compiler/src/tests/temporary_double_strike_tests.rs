//! Complete fixed Double Strike programs, never a subset of conditional source.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const STROBE: &str =
    include_str!("../../../../tests/fixtures/9bf4ee0f-6e28-4793-b251-f81ee444a119.json");
const FURY: &str =
    include_str!("../../../../tests/fixtures/7aac51ef-ae03-4a9e-9a48-46223787f291.json");
const RIOT: &str =
    include_str!("../../../../tests/fixtures/49f42896-90cd-4a05-bcca-12b19f780446.json");
const RAGE: &str =
    include_str!("../../../../tests/fixtures/2fdc0d17-b683-4263-949e-4f74ec76badf.json");
#[test]
fn temporary_double_strike_complete_sources_costs_shapes_and_conditional_rejection() {
    for (raw, index, generic, pump, set) in [
        (STROBE, 23511u32, 0i64, false, false),
        (FURY, 18489, 2, true, false),
        (RIOT, 11125, 4, false, true),
    ] {
        let CompileResult::Lowered { definition: d } = compile(HEADER, raw) else {
            panic!("{:?}", compile(HEADER, raw))
        };
        let pin: Value = serde_json::from_str(raw).unwrap();
        d.validate().unwrap();
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(d.provenance.source_sha256, pin["sourceSha256"]);
        assert_eq!(d.provenance.ast_sha256, pin["astSha256"]);
        assert_eq!(d.provenance.binding_sha256, pin["bindingSha256"]);
        assert_eq!(d.characteristics.mana_cost.generic, Quantity::from(generic));
        assert_eq!(d.characteristics.mana_cost.colored, vec![Color::R]);
        assert!(d.capabilities.contains(&Capability::DoubleStrikeV1));
        let p = d.spell.as_ref().unwrap();
        assert_eq!(p.instructions.len(), 1);
        if set {
            assert!(
                matches!(&p.instructions[0],Instruction::GrantKeywordsSet{set:ObjectSet::ControllerCreatures,modifier:None,keywords,duration:Duration::UntilEndOfTurn,source_block:0}if keywords==&[KeywordAbility::DoubleStrike])
            );
        } else {
            let Instruction::GrantKeywords {
                target_slot: 0,
                modifier,
                keywords,
                duration: Duration::UntilEndOfTurn,
                source_block: 0,
            } = &p.instructions[0]
            else {
                panic!("targeted grant")
            };
            assert_eq!(keywords, &[KeywordAbility::DoubleStrike]);
            assert_eq!(
                modifier
                    .as_ref()
                    .map(|m| (m.power.clone(), m.toughness.clone())),
                pump.then(|| (1.into(), 1.into()))
            );
        }
    }
    assert!(matches!(
        compile(HEADER, RAGE),
        CompileResult::Unsupported { .. }
    ));
}
#[test]
fn temporary_double_strike_rejects_extra_scopes_and_partial_bindings() {
    for raw in [STROBE, FURY, RIOT] {
        for case in 0..7 {
            let changed = tests::changed_source(raw, |r| {
                let a = &mut r["faces"][0]["result"]["abilities"][0];
                let steps = a["effects"][0]["steps"].as_array_mut().unwrap();
                let s = steps.last_mut().unwrap();
                match case {
                0=>s["abilities"][0]["keyword"]["keyword"]=json!("defender"),
                1=>s["duration"]["duration"]=json!("forever"),
                2=>s["what"]["filter"]["other"]=Value::Null,
                3=>s["what"]["filter"]["types"]=json!(["land"]),
                4=>{let x=s["abilities"][0].clone();s["abilities"].as_array_mut().unwrap().push(x);},
                5=>steps.push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})),
                _=>a["effects"][0]["condition"]=Value::Null,
            }
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
