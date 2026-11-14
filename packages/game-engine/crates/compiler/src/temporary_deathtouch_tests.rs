//! Exact d705 archive source and explicitly labelled mutated source controls.
use super::*;
use serde_json::json;
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const RAW: &str = include_str!("../../../tests/fixtures/b75fb789-6a78-4d23-b872-7627b478fb6f.json");
#[test]
fn temporary_deathtouch_complete_serpents_gift() {
    assert_eq!(
        sha256(RAW.trim_end().as_bytes()),
        "609022e2d6a4569c836a8490ab9b200501f7abf203eaebaaa9bbf6e149f6e836"
    );
    let CompileResult::Lowered { definition: d } = compile(HEADER, RAW) else {
        panic!("Serpent's Gift")
    };
    d.validate().unwrap();
    assert_eq!(d.name, "Serpent's Gift");
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::G]
        }
    );
    assert_eq!(d.characteristics.color_identity, [Color::G]);
    assert_eq!(d.provenance.source_index, 27600);
    assert_eq!(
        d.provenance.source_sha256,
        "5e6f68fe1681e09e9f3faa1979b217619b0c58a6d43f2f88159d770c64579e6f"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "17b8cc784edcd8b8501740cd8f60cbd7a6e04613e18166c75c91db1d7abdbb1d"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "920737a787011947206ff954053c8cb90425605fc14268f4d16c4ac678879ac1"
    );
    assert!(
        d.capabilities
            .contains(&Capability::TemporaryDeathtouchSpellV1)
    );
    assert!(d.capabilities.contains(&Capability::DeathtouchV1));
    assert_eq!(d.capabilities, d.requirements());
    if let Ok(out) = std::env::var("MTG_TEMPORARY_DEATHTOUCH_EVIDENCE") {
        let mut c = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-effects-ir.json"
        ))
        .unwrap();
        c.definitions = vec![(*d).clone()];
        std::fs::write(
            std::path::Path::new(&out).join("serpents-gift-ir.json"),
            canonical(&c).unwrap(),
        )
        .unwrap();
    }
    let p = d.spell.as_ref().unwrap();
    assert_eq!(p.targets.len(), 1);
    assert_eq!(p.targets[0].selection_id, "line:0:0");
    assert!(
        matches!(p.instructions.as_slice(),[Instruction::GrantKeywords { target_slot:0, keywords, duration:Duration::UntilEndOfTurn, source_block:0 }] if keywords == &[KeywordAbility::Deathtouch])
    );
}
#[test]
fn temporary_deathtouch_strict_mirror_and_unsupported_source_shapes() {
    assert!(
        read_abilities(r#"[{"kind":"spell","effects":[{"steps":[{"effect":"gain-abilities"}]}]}]"#)
            .is_err()
    );
    for variant in 0..9 {
        let raw = tests::changed_source(RAW, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            let s = &mut a["effects"][0]["steps"][0];
            match variant {
                0 => s["abilities"][0]["keyword"]["keyword"] = "reach".into(),
                1 => s["abilities"][0]["keyword"]["extra"] = true.into(),
                2 => s["duration"]["duration"] = "forever".into(),
                3 => s["what"]["count"]["value"] = 2.into(),
                4 => s["what"]["filter"]["control"] = "opponent".into(),
                5 => s["abilities"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"gained":"keyword","keyword":{"keyword":"deathtouch"}})),
                6 => s["extra"] = true.into(),
                7 => {
                    let child = s.clone();
                    *s = json!({"effect":"modal","count":{"exactly":1},"options":[{"effects":[{"steps":[child.clone()]}]},{"effects":[{"steps":[child]}]}]});
                }
                _ => {
                    a["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"count":{"amount":"fixed","value":1}}));
                }
            }
            r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
        });
        assert!(
            !matches!(compile(HEADER, &raw), CompileResult::Lowered { .. }),
            "variant {variant}"
        );
    }
}
