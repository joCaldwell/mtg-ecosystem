//! Complete unchanged frozen Wanderer, not intrinsic haste or a patched source.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const WANDERER: &str =
    include_str!("../../../tests/fixtures/ramos/ad9b7fbc-61c8-43ee-a65c-99206fd1e4df.json");
#[test]
fn ramos_m4b_complete_wanderer_live_modifier_and_two_independent_cascades() {
    assert_eq!(sha256(WANDERER.as_bytes()), "e6d8ce748bbe4ffd53dd9e75c9279edf0de60a3d2bdade1cdd6d70e4108c6eb5");
    let record: serde_json::Value = strict_read(WANDERER).unwrap();
    assert_eq!(record["sourceIndex"], 26149);
    assert_eq!(record["binding"]["fullyBound"], true);
    let CompileResult::Lowered { definition: d } = compile(MANIFEST, WANDERER) else {
        panic!("complete frozen Wanderer must lower")
    };
    d.validate().unwrap();
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert!(d.characteristics.legendary);
    assert_eq!(d.characteristics.subtypes, ["Elemental"]);
    assert_eq!(d.characteristics.power, Some(7.into()));
    assert_eq!(d.characteristics.toughness, Some(5.into()));
    assert_eq!(d.characteristics.mana_cost, ManaCost {
        generic: 5.into(), colored: vec![Color::G, Color::U, Color::R],
    });
    assert_eq!(d.characteristics.colors, [Color::G, Color::R, Color::U]);
    assert_eq!(d.characteristics.color_identity, [Color::G, Color::R, Color::U]);
    assert!(d.keywords.is_empty(), "haste must remain a live grant");
    assert_eq!(d.static_abilities, [StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ModifyCreatures {
            scope: CreatureScope::Controller, power: 0.into(), toughness: 0.into(),
            keywords: vec![KeywordAbility::Haste], restrictions: vec![],
        },
    }]);
    assert_eq!(d.triggered.len(), 2);
    for a in &d.triggered {
        assert_eq!(a.condition, TriggerCondition::SelfSpellCast);
        assert_eq!(a.functioning_zone, FunctioningZone::Stack);
        assert!(matches!(a.program.instructions.as_slice(), [Instruction::Cascade {
            threshold: mtg_contracts::CascadeThreshold::SourceSpellManaValue, source_block: 1,
        }]));
    }
    for cap in [Capability::AttachmentsV1, Capability::CascadeV1,
        Capability::ResolutionCastV1, Capability::RandomBottomV1] {
        let mut bad = d.as_ref().clone();
        bad.capabilities.remove(&cap);
        assert!(bad.validate().is_err());
    }
}
#[test]
fn ramos_m4b_complete_source_mutations_never_drop_static_or_cascade_operands() {
    for mutation in 0..9 {
        let raw = tests::changed_source(WANDERER, |r| {
            for section in ["abilities", "lines"] {
                let a = if section == "abilities" {
                    &mut r["faces"][0]["result"][section][0]
                } else { &mut r["faces"][0]["result"][section][0]["ability"] };
                let e = &mut a["effect"];
                match mutation {
                    0 => e["what"]["filter"]["control"] = serde_json::json!("opponent"),
                    1 => e["what"]["filter"]["types"] = serde_json::json!(["artifact"]),
                    2 => e["what"]["filter"]["other"] = serde_json::json!(true),
                    3 => e["what"]["filter"]["status"] = serde_json::json!(["tapped"]),
                    4 => e["modifiers"][0]["abilities"][0]["keyword"]["cost"] = serde_json::json!([]),
                    5 => e["modifiers"][0]["abilities"][0]["keyword"]["keyword"] = serde_json::json!("flash"),
                    6 => e["modifiers"][0]["extra_clause"] = serde_json::json!({}),
                    7 => e["modifiers"] = serde_json::json!([]),
                    8 => a["extra_clause"] = serde_json::json!({}),
                    _ => unreachable!(),
                }
            }
        });
        assert!(!matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }), "static {mutation}");
    }
    for index in 0..2 {
        let raw = tests::changed_source(WANDERER, |r| {
            for section in ["abilities", "lines"] {
                let a = if section == "abilities" { &mut r["faces"][0]["result"][section][1] }
                    else { &mut r["faces"][0]["result"][section][1]["ability"] };
                a["keywords"][index]["params"] = serde_json::json!([]);
            }
        });
        assert!(!matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }));
    }
    let raw = tests::changed_source(WANDERER, |r| { r["source"]["oracle_text"] = serde_json::json!("Haste"); });
    assert!(!matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }));
}
