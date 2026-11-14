//! Exact d705 archive source and explicitly labelled mutated source controls.
use super::*;
use serde_json::json;
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const RAW: &str = include_str!("../../../tests/fixtures/f7518456-45ed-41d4-bd3c-5aacea28eb35.json");
#[test]
fn temporary_flying_complete_jump() {
    assert_eq!(
        sha256(RAW.trim_end().as_bytes()),
        "a164fca57e67134c6f5e1a44b482f4f2a945aac42e41ead3e9586e845de30e8f"
    );
    let CompileResult::Lowered { definition: d } = compile(HEADER, RAW) else {
        panic!("Jump")
    };
    d.validate().unwrap();
    assert_eq!(d.name, "Jump");
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 0.into(),
            colored: vec![Color::U]
        }
    );
    assert_eq!(d.characteristics.color_identity, [Color::U]);
    assert_eq!(d.provenance.source_index, 37408);
    assert_eq!(
        d.provenance.source_sha256,
        "315f9af7b00e1d3c7c122a0be565cac5f7abfd89de5a346a4c3595faaba5b81e"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "fd86fe1803247a0573bdc4c45c9dbaa18ea76fd03b2582744c8fea681eca764c"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "8c5dce2342d524fe04512520f3ef40628e37fc2bacb0aeb35b0c9e28d9ae0269"
    );
    assert!(d.capabilities.contains(&Capability::TemporaryFlyingSpellV1));
    assert!(d.capabilities.contains(&Capability::FlyingV1));
    assert_eq!(d.capabilities, d.requirements());
    if let Ok(out) = std::env::var("MTG_TEMPORARY_FLYING_EVIDENCE") {
        let mut c = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-effects-ir.json"
        ))
        .unwrap();
        c.definitions = vec![(*d).clone()];
        std::fs::write(
            std::path::Path::new(&out).join("jump-ir.json"),
            canonical(&c).unwrap(),
        )
        .unwrap();
    }
    let p = d.spell.as_ref().unwrap();
    assert_eq!(p.targets.len(), 1);
    assert_eq!(p.targets[0].selection_id, "line:0:0");
    assert!(
        matches!(p.instructions.as_slice(),[Instruction::GrantKeywords { target_slot:0, keywords, duration:Duration::UntilEndOfTurn, source_block:0 }] if keywords == &[KeywordAbility::Flying])
    );
}
#[test]
fn temporary_flying_strict_mirror_and_unsupported_source_shapes() {
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
                4 => s["what"]["filter"]["control"] = "you".into(),
                5 => s["abilities"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"gained":"keyword","keyword":{"keyword":"indestructible"}})),
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
