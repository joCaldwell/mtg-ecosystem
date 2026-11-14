//! Exact archive source; mutants are explicit unsupported-source controls.
use super::*;
use serde_json::json;
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const RAW: &str = include_str!("../../../tests/fixtures/ef29f979-1a6d-47bb-83f5-4679389471da.json");
#[test]
fn compound_indestructible_complete_mortals_resolve() {
    assert_eq!(
        sha256(RAW.trim_end().as_bytes()),
        "dd42d4da1b3412af58b145b7a6657dfb5cb98a047efca164ecaa1f5fa34560ad"
    );
    let result = compile(HEADER, RAW);
    let CompileResult::Lowered { definition: d } = result else {
        panic!("complete Mortal's Resolve: {result:?}")
    };
    d.validate().unwrap();
    assert_eq!(d.oracle_id, "ef29f979-1a6d-47bb-83f5-4679389471da");
    assert_eq!(d.name, "Mortal's Resolve");
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::G]
        }
    );
    assert_eq!(d.characteristics.color_identity, [Color::G]);
    assert_eq!(d.provenance.source_index, 36182);
    assert_eq!(
        d.provenance.source_sha256,
        "71b794378a2dc7529072ebcdc59ed52b203e0e164510d758c3f1de160cbd0d59"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "3f1ca48780e3f8a0661d987a50c1a69b54e3d3af315f2589afe09b8c2f225811"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "5f2ba2465e70f10af914637a882a0b8c33466130b22c159b98ecba6f909acef2"
    );
    assert_eq!(d.capabilities, d.requirements());
    assert!(
        d.capabilities
            .contains(&Capability::CompoundIndestructibleSpellV1)
    );
    assert!(
        !d.capabilities
            .contains(&Capability::TemporaryIndestructibleSpellV1)
    );
    assert!(!d.capabilities.contains(&Capability::TemporaryFlyingSpellV1));
    let p = d.spell.as_ref().unwrap();
    assert_eq!(p.targets.len(), 1);
    assert_eq!(p.targets[0].selection_id, "line:0:0");
    assert!(matches!(p.instructions.as_slice(),[
        Instruction::ModifyPowerToughness{target_slot:0,power,toughness,duration:Duration::UntilEndOfTurn,source_block:0},
        Instruction::GrantKeywords{target_slot:0,keywords,duration:Duration::UntilEndOfTurn,source_block:0}
    ] if *power==1.into() && *toughness==1.into() && keywords==&[KeywordAbility::Indestructible]));
}
#[test]
fn compound_indestructible_rejects_mutated_steps_occurrences_bindings_and_contexts() {
    for variant in 0..19 {
        let raw = tests::changed_source(RAW, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            let steps = &mut a["effects"][0]["steps"];
            match variant {
                0=>steps.as_array_mut().unwrap().reverse(),
                1=>steps[0]["power"]["amount"]["value"]=2.into(),
                2=>steps[0]["toughness"]["sign"]=(-1).into(),
                3=>steps[0]["duration"]["duration"]="forever".into(),
                4=>steps[1]["duration"]["duration"]="forever".into(),
                5=>steps[1]["abilities"][0]["keyword"]["keyword"]="flying".into(),
                6=>steps[1]["abilities"].as_array_mut().unwrap().push(json!({"gained":"keyword","keyword":{"keyword":"indestructible"}})),
                7=>steps[1]["what"]["selectionId"]="line:0:1".into(),
                8=>steps[0]["what"]["count"]["value"]=2.into(),
                9=>steps[1]["what"]["filter"]["control"]="you".into(),
                10=>{let x=steps[1].clone(); steps.as_array_mut().unwrap().push(x);},
                11=>steps.as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"count":{"amount":"fixed","value":1}})),
                12=>a["effects"][0]["bindings"]=json!([{"variable":"unused","value":{"amount":"fixed","value":1}}]),
                13=>{let child=a["effects"].clone(); a["effects"]=json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":child.clone()},{"effects":child}]}]}]);},
                14=>{a["kind"]="activated".into(); a["costs"]=json!([{"cost":"tap"}]);},
                15=>steps[1]["abilities"][0]["keyword"]["extra"]=true.into(),
                16=>steps[1]["what"]["selectionId"]="".into(),
                17=>steps[1]["abilities"][0]["keyword"]["keyword"]="reach".into(),
                _=>a["effects"][0]["steps"][0]["extra"]=true.into(),
            }
            r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
        });
        assert!(
            !matches!(compile(HEADER, &raw), CompileResult::Lowered { .. }),
            "variant {variant}"
        );
    }
    // Sentence bindings define executable values. The archive reference graph
    // remains diagnostic for ordinary targets; raw-record provenance pins it.
    for bindings in [
        json!([{"variable":"unused","value":{"amount":"fixed","value":1}}]),
        json!([{"variable":"x","value":{"amount":"fixed","value":1}}, {"variable":"x","value":{"amount":"fixed","value":1}}]),
        json!([{"variable":"x","value":{"amount":"count","of":{"types":["creature"],"zone":{"zone":"battlefield"}}}}]),
    ] {
        let raw = tests::changed_source(RAW, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            a["effects"][0]["bindings"] = bindings;
            r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
        });
        assert!(
            !matches!(compile(HEADER, &raw), CompileResult::Lowered { .. }),
            "unconsumed or duplicate executable sentence binding"
        );
    }
}
