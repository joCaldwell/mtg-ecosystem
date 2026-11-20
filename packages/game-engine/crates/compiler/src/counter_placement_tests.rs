//! Complete unchanged archived providers; isolated source-boundary near misses.
use super::*;
use serde_json::{Value, json};
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const SCALES: &str =
    include_str!("../../../tests/fixtures/ramos/a1f3da21-af6d-450e-bf0b-985d158418e6.json");
const CORPSEJACK: &str =
    include_str!("../../../tests/fixtures/ramos/ca0cc02b-b106-4eca-9388-d4b48dd3be49.json");
fn lower(raw: &str) -> Box<Definition> {
    let result = compile(MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    definition
}
#[test]
fn counter_placement_complete_frozen_sources_and_characteristics() {
    for (raw, modification, kind, source_hash) in [
        (
            SCALES,
            CounterPlacementModification::Add { amount: 1.into() },
            CardKind::Enchantment,
            "19fe818b645e5db9195ad3c9aa3564e67765dfbcbd0a5c8739f8c4c2ecca537c",
        ),
        (
            CORPSEJACK,
            CounterPlacementModification::Multiply { factor: 2.into() },
            CardKind::Creature,
            "5936c3792a9fe9098db3b73666da3a039a2edc15a6921ff16858aa46a2447845",
        ),
    ] {
        let d = lower(raw);
        assert_eq!(d.provenance.source_sha256, source_hash);
        assert_eq!(d.characteristics.kind, kind);
        assert_eq!(
            d.static_abilities,
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::ControllerCreaturePlusOneCounterReplacement { modification }
            }]
        );
        assert!(d.triggered.is_empty() && d.activated.is_empty() && d.mana_abilities.is_empty());
        assert!(
            d.requirements()
                .contains(&Capability::ControllerCreaturePlusOneCounterReplacementV1)
        );
        if kind == CardKind::Creature {
            assert_eq!(d.characteristics.power, Some(4.into()));
            assert_eq!(d.characteristics.toughness, Some(4.into()));
            assert_eq!(d.characteristics.subtypes, ["Fungus"]);
            assert_eq!(
                d.characteristics.mana_cost,
                ManaCost {
                    generic: 2.into(),
                    colored: vec![Color::B, Color::G]
                }
            );
        }
        d.validate().unwrap();
    }
}
#[test]
fn counter_placement_parameterized_lowering_and_isolated_boundary_negatives() {
    for (raw, change, expected) in [
        (
            SCALES,
            json!({"operation":"add","amount":{"amount":"fixed","value":7}}),
            CounterPlacementModification::Add { amount: 7.into() },
        ),
        (
            CORPSEJACK,
            json!({"operation":"multiply","factor":3}),
            CounterPlacementModification::Multiply { factor: 3.into() },
        ),
    ] {
        let modified = tests::changed_source(raw, |r| {
            let result = &mut r["faces"][0]["result"];
            result["abilities"][0]["effect"]["replacement"]["replace"]["change"] = change;
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        // Deliberate synthetic near-source quantity variation; archived witness stays unchanged.
        assert_eq!(
            lower(&modified).static_abilities[0].effect,
            StaticEffect::ControllerCreaturePlusOneCounterReplacement {
                modification: expected
            }
        );
    }
    for raw in [SCALES, CORPSEJACK] {
        for mutate in [
            (|r: &mut Value| r["event"]["on"][0]["filter"]["control"] = "opponent".into())
                as fn(&mut Value),
            |r| r["event"]["on"][0]["filter"]["types"] = json!(["artifact"]),
            |r| r["event"]["on"][0]["count"]["value"] = 2.into(),
            |r| r["event"]["counter"]["type"] = "charge".into(),
            |r| r["event"]["quantity"]["op"] = "eq".into(),
            |r| r["event"]["quantity"]["value"]["value"] = 0.into(),
            |r| r["event"]["cause"] = "effect".into(),
            |r| r["event"]["on"][0]["filter"]["power"] = 4.into(),
            |r| r["event"]["entry"] = true.into(),
            |r| r["occurrence"] = "first".into(),
            |r| {
                r["replace"]["change"] =
                    json!({"operation":"add","amount":{"amount":"fixed","value":0}})
            },
            |r| r["replace"]["change"] = json!({"operation":"multiply","factor":0}),
            |r| {
                r["replace"]["change"] =
                    json!({"operation":"add","factor":2,"amount":{"amount":"fixed","value":1}})
            },
            |r| r["replace"]["change"]["unconsumed"] = true.into(),
        ] {
            let modified = tests::changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                mutate(&mut result["abilities"][0]["effect"]["replacement"]);
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(
                !matches!(compile(MANIFEST, &modified), CompileResult::Lowered { .. }),
                "boundary admitted"
            );
        }
    }
}
