//! Complete frozen family witnesses; mutated ASTs below are unsupported controls.
use super::*;
use serde_json::json;
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
// raw, name, oracle ID, record/source/AST/binding hashes, source index, color, bonuses
const SOURCES: &[(
    &str,
    &str,
    &str,
    &str,
    &str,
    &str,
    &str,
    u32,
    Color,
    i64,
    i64,
)] = &[
    (
        include_str!("../../../tests/fixtures/ef29f979-1a6d-47bb-83f5-4679389471da.json"),
        "Mortal's Resolve",
        "ef29f979-1a6d-47bb-83f5-4679389471da",
        "dd42d4da1b3412af58b145b7a6657dfb5cb98a047efca164ecaa1f5fa34560ad",
        "71b794378a2dc7529072ebcdc59ed52b203e0e164510d758c3f1de160cbd0d59",
        "3f1ca48780e3f8a0661d987a50c1a69b54e3d3af315f2589afe09b8c2f225811",
        "5f2ba2465e70f10af914637a882a0b8c33466130b22c159b98ecba6f909acef2",
        36182,
        Color::G,
        1,
        1,
    ),
    (
        include_str!("../../../tests/fixtures/467e22c3-6107-40ff-afc7-5960710c970b.json"),
        "Adamant Will",
        "467e22c3-6107-40ff-afc7-5960710c970b",
        "d61101bc7af4150ccd5d99b879d6b05e27758e24e0a6afdeb32c7a7610d770ab",
        "42dd4c033da6f0444a68c7bbdd9311d110c27d3f1cbd6a839e3b2adf8b104c18",
        "5f85e210ae3f5489233aacd79146a5519eb411e2aeb2044c899d3ded910d1fc9",
        "a790788083d54b7760cd5b801131ba35065198a4215749fd9e4fbd5f1b08e98d",
        10593,
        Color::W,
        2,
        2,
    ),
    (
        include_str!("../../../tests/fixtures/795e0ffb-fcd9-4cb9-ad18-8a0217229bfe.json"),
        "Unlikely Aid",
        "795e0ffb-fcd9-4cb9-ad18-8a0217229bfe",
        "cf650fa0764517eec93a92133aa0245c93bdffa203ba1927f40a924fbc806696",
        "ae86dc4ecdf8c6c8cec142fdfadfc00d588c44dca9b7fb1f9fbdb0f33e88832d",
        "bc05ce23c6826a998d60f04d6cfbee7ffad39bbb55a942a60a95bfcc53dda5fd",
        "371b0ae4f18209047f95f1e16ea0c234d3ede984886649a08bd3152f8c504c50",
        18297,
        Color::B,
        2,
        0,
    ),
];

#[test]
fn compound_indestructible_family_complete_sources_share_capabilities_and_path() {
    let mut capabilities = None;
    for &(raw, name, id, record, source, ast, binding, index, color, power, toughness) in SOURCES {
        assert_eq!(sha256(raw.trim_end().as_bytes()), record);
        let result = compile(HEADER, raw);
        let CompileResult::Lowered { definition: d } = result else {
            panic!("complete {name}: {result:?}")
        };
        d.validate().unwrap();
        assert_eq!(CURRENT_IR_VERSION, 1);
        assert_eq!(d.name, name);
        assert_eq!(d.oracle_id, id);
        assert_eq!(d.characteristics.kind, CardKind::Instant);
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: 1.into(),
                colored: vec![color]
            }
        );
        assert_eq!(d.characteristics.color_identity, [color]);
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(d.provenance.source_sha256, source);
        assert_eq!(d.provenance.ast_sha256, ast);
        assert_eq!(d.provenance.binding_sha256, binding);
        assert_eq!(d.capabilities, d.requirements());
        assert!(
            d.capabilities
                .contains(&Capability::CompoundIndestructibleSpellV1)
        );
        for cap in [
            Capability::TemporaryIndestructibleSpellV1,
            Capability::TemporaryFlyingSpellV1,
            Capability::TemporaryDeathtouchSpellV1,
        ] {
            assert!(!d.capabilities.contains(&cap));
        }
        if let Some(expected) = &capabilities {
            assert_eq!(&d.capabilities, expected);
        }
        capabilities = Some(d.capabilities.clone());
        let p = d.spell.as_ref().unwrap();
        assert_eq!(p.targets[0].selection_id, "line:0:0");
        assert_eq!(
            compound_indestructible_parameters(p),
            Some((&power.into(), &toughness.into()))
        );
    }
}

#[test]
fn compound_indestructible_family_new_sources_reject_shape_near_misses() {
    for &(raw, name, ..) in &SOURCES[1..] {
        for variant in 0..16 {
            let changed = tests::changed_source(raw, |r| {
                let a = &mut r["faces"][0]["result"]["abilities"][0];
                let steps = &mut a["effects"][0]["steps"];
                match variant {
                    0 => steps.as_array_mut().unwrap().reverse(),
                    1 => steps[0]["duration"]["duration"] = "forever".into(),
                    2 => steps[1]["duration"]["duration"] = "forever".into(),
                    3 => steps[1]["what"]["selectionId"] = "line:0:1".into(),
                    4 => steps[0]["what"]["count"]["value"] = 2.into(),
                    5 => steps[1]["what"]["filter"]["control"] = "you".into(),
                    6 => steps[1]["abilities"][0]["keyword"]["keyword"] = "flying".into(),
                    7 => { let x = steps[1]["abilities"][0].clone(); steps[1]["abilities"].as_array_mut().unwrap().push(x); },
                    8 => { let x = steps[0].clone(); steps.as_array_mut().unwrap().push(x); },
                    9 => steps[0]["power"]["amount"] = json!({"amount":"variable","name":"x"}),
                    10 => a["effects"][0]["bindings"] = json!([{"variable":"unused","value":{"amount":"fixed","value":1}}]),
                    11 => { a["kind"] = "activated".into(); a["costs"] = json!([{"cost":"tap"}]); },
                    12 => { let child = a["effects"].clone(); a["effects"] = json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":child.clone()},{"effects":child}]}]}]); },
                    13 => steps[1]["what"]["selectionId"] = "".into(),
                    14 => steps[0]["extra"] = true.into(),
                    _ => steps.as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"count":{"amount":"fixed","value":1}})),
                }
                r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
            });
            assert!(
                !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
                "{name} variant {variant}"
            );
        }
    }
}
