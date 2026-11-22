//! Complete Vigilance source; counter/Flashback clauses never lower partially.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const PROPORTIONS: &str =
    include_str!("../../../../tests/fixtures/1e03aad2-6426-4d18-a0e5-bc3d8e6b7b11.json");
const COURAGE: &str =
    include_str!("../../../../tests/fixtures/203eb24c-f135-44e2-bdae-80b7b3d0fb15.json");
#[test]
fn temporary_vigilance_complete_source_cost_and_single_effect() {
    let CompileResult::Lowered { definition: d } = compile(HEADER, PROPORTIONS) else {
        panic!("{:?}", compile(HEADER, PROPORTIONS))
    };
    let raw: Value = serde_json::from_str(PROPORTIONS).unwrap();
    d.validate().unwrap();
    assert_eq!(d.provenance.source_index, 4564);
    assert_eq!(d.provenance.source_sha256, raw["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, raw["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, raw["bindingSha256"]);
    assert_eq!(d.characteristics.mana_cost.generic, Quantity::from(5));
    assert_eq!(
        d.characteristics.mana_cost.colored,
        vec![Color::G, Color::G]
    );
    assert!(d.capabilities.contains(&Capability::VigilanceV1));
    assert_eq!(
        d.spell.unwrap().instructions,
        vec![Instruction::GrantKeywordsSet {
            set: ObjectSet::ControllerCreatures,
            modifier: Some(CreatureStatModifier {
                power: 10.into(),
                toughness: 10.into()
            }),
            keywords: vec![KeywordAbility::Vigilance],
            duration: Duration::UntilEndOfTurn,
            source_block: 0
        }]
    );
    assert!(matches!(
        compile(HEADER, COURAGE),
        CompileResult::Unsupported { .. }
    ));
}
#[test]
fn temporary_vigilance_rejects_mismatched_capture_extra_clauses_and_qualifiers() {
    for case in 0..7 {
        let changed = tests::changed_source(PROPORTIONS, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            let s = &mut a["effects"][0]["steps"][1];
            match case {
                0=>s["what"]["filter"]["control"]=json!("opponent"),
                1=>s["what"]["filter"]["types"]=json!(["land"]),
                2=>s["duration"]["duration"]=json!("forever"),
                3=>s["abilities"][0]["keyword"]["keyword"]=json!("defender"),
                4=>s["what"]["filter"]["other"]=Value::Null,
                5=>a["effects"][0]["condition"]=Value::Null,
                _=>a["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})),
            }
            let ability = a.clone();
            r["faces"][0]["result"]["lines"][0]["ability"] = ability;
        });
        assert!(
            !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
            "{case}"
        );
    }
}
