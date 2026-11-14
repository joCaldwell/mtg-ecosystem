//! Exact d705 archive source and explicitly labelled mutated source controls.
use super::*;
use serde_json::json;
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const RAW: &str = include_str!("../../../tests/fixtures/c462adf3-4ade-4b90-bdec-7247b080471f.json");
#[test]
fn temporary_indestructible_complete_withstand_death() {
    assert_eq!(
        sha256(RAW.trim_end().as_bytes()),
        "f68167a0729366ded35cdd527d118c5f499b85226c6b816238674ea065fa2f75"
    );
    let CompileResult::Lowered { definition: d } = compile(HEADER, RAW) else {
        panic!("Withstand Death")
    };
    d.validate().unwrap();
    assert_eq!(d.name, "Withstand Death");
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 0.into(),
            colored: vec![Color::G]
        }
    );
    assert_eq!(d.characteristics.color_identity, [Color::G]);
    assert_eq!(d.provenance.source_index, 29600);
    assert_eq!(
        d.provenance.source_sha256,
        "c5f612ee21c892c59610fe297156f13009dcb767b979d88e24be5c76d21f1038"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "db293a1c7bf36063a1961db3d9168f786a91808ce42af3d6f9956bd020c6da38"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "2252071bd53db8bbcd04d6f9375dc62ba45063abc9157140b7fecad0f72eef0c"
    );
    assert!(
        d.capabilities
            .contains(&Capability::TemporaryIndestructibleSpellV1)
    );
    assert!(d.capabilities.contains(&Capability::IndestructibleV1));
    assert_eq!(d.capabilities, d.requirements());
    let p = d.spell.as_ref().unwrap();
    assert_eq!(p.targets.len(), 1);
    assert_eq!(p.targets[0].selection_id, "line:0:0");
    assert!(
        matches!(p.instructions.as_slice(),[Instruction::GrantKeywords { target_slot:0, keywords, duration:Duration::UntilEndOfTurn, source_block:0 }] if keywords == &[KeywordAbility::Indestructible])
    );
}
#[test]
fn temporary_indestructible_strict_mirror_and_unsupported_source_shapes() {
    assert!(
        read_abilities(r#"[{"kind":"spell","effects":[{"steps":[{"effect":"gain-abilities"}]}]}]"#)
            .is_err()
    );
    for variant in 0..9 {
        let raw = tests::changed_source(RAW, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            let s = &mut a["effects"][0]["steps"][0];
            match variant {
                0 => s["abilities"][0]["keyword"]["keyword"] = "flying".into(),
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
