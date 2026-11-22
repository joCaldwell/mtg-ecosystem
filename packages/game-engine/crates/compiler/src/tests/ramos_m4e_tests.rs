use super::*;
use serde_json::json;
const MANIFEST: &str =
    include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
const SOURCE: &str =
    include_str!("../../../../tests/fixtures/ramos/71366469-04ea-4526-bf24-7abc02fdc5ab.json");
#[test]
fn ramos_m4e_complete_frozen_source_and_contract_fences() {
    assert_eq!(
        sha256(SOURCE.as_bytes()),
        "96c5a33dc72b4647ca64e81e10979ea91646e8aabdc37f6a386c5a9865911fdd"
    );
    let CompileResult::Lowered { definition: d } = compile(MANIFEST, SOURCE) else {
        panic!("{:?}", compile(MANIFEST, SOURCE))
    };
    d.validate().unwrap();
    assert!(d.characteristics.has_type(CardKind::Artifact));
    assert!(d.characteristics.has_type(CardKind::Creature));
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 7.into(),
            colored: vec![Color::W, Color::U]
        }
    );
    assert_eq!(
        (
            d.characteristics.power.clone(),
            d.characteristics.toughness.clone()
        ),
        (Some(4.into()), Some(4.into()))
    );
    assert_eq!(d.characteristics.subtypes, ["Sphinx"]);
    assert_eq!(d.keywords, [KeywordAbility::Flying]);
    assert_eq!(d.triggered.len(), 1);
    assert_eq!(d.triggered[0].program, canonical_cascade_program(2));
    assert_eq!(
        d.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Stack,
            effect: StaticEffect::ReduceSelfGenericCost {
                per_object: 1.into(),
                count: BattlefieldCount {
                    kind: CardKind::Artifact,
                    controller: CountController::Caster
                }
            }
        }]
    );
    assert!(
        d.capabilities
            .contains(&Capability::CountedSelfGenericCostReductionV1)
    );
    assert!(!d.capabilities.contains(&Capability::BoardWipesV1));
    for mutation in 0..8 {
        let mut bad = d.clone();
        match mutation {
            0 => bad
                .capabilities
                .remove(&Capability::CountedSelfGenericCostReductionV1),
            1 => bad.capabilities.insert(Capability::BoardWipesV1),
            _ => {
                let a = &mut bad.static_abilities[0];
                if mutation == 2 {
                    a.functioning_zone = FunctioningZone::Battlefield;
                }
                let StaticEffect::ReduceSelfGenericCost { per_object, count } = &mut a.effect
                else {
                    panic!()
                };
                match mutation {
                    3 => *per_object = 0.into(),
                    4 => *per_object = (-1).into(),
                    5 => count.kind = CardKind::Creature,
                    6 => count.controller = CountController::Any,
                    7 => bad.characteristics.subtypes.push("Aura".into()),
                    _ => {}
                }
                false
            }
        };
        assert!(bad.validate().is_err(), "mutation {mutation}");
    }
    let mut multiple = d.clone();
    multiple
        .static_abilities
        .push(multiple.static_abilities[0].clone());
    multiple.validate().unwrap();
}
#[test]
fn ramos_m4e_rehashed_partial_extra_selector_and_binding_rejections() {
    for selector in [
        json!(null),
        json!({}),
        json!({"types":["creature"]}),
        json!({"types":["artifact"],"controller":"you"}),
        json!({"types":["artifact"],"owner":"you"}),
        json!({"types":["artifact","land"]}),
        json!({"types":["artifact"],"status":["tapped"]}),
    ] {
        let changed = tests::changed_source(SOURCE, |r| {
            for key in ["abilities", "lines"] {
                let a = if key == "abilities" {
                    &mut r["faces"][0]["result"][key][0]
                } else {
                    &mut r["faces"][0]["result"][key][0]["ability"]
                };
                a["keywords"][0]["filter"] = selector.clone();
            }
        });
        assert!(
            !matches!(compile(MANIFEST, &changed), CompileResult::Lowered { .. }),
            "{selector}"
        );
    }
    for mutation in 0..10 {
        let changed = tests::changed_source(SOURCE, |r| match mutation {
            0 => r["binding"]["nodes"][4]["role"] = "owner".into(),
            1 => r["binding"]["nodes"][5]["keyword"] = "flying".into(),
            2 => r["binding"]["nodes"][5]["you"] = "foreign".into(),
            3 => r["binding"]["nodes"][5]["abilities"] = json!(["triggered"]),
            4 => r["faces"][0]["result"]["abilities"][0]["keywords"][0]["extra"] = true.into(),
            5 => r["source"]["mana_cost"] = "{7}{W}{U}{P}".into(),
            6 => r["binding"]["nodes"][1]["manaCost"] = "{8}{W}{U}".into(),
            7 => r["binding"]["nodes"][0]["identity"] = "foreign".into(),
            8 => r["binding"]["nodes"][2]["source"] = "foreign".into(),
            _ => r["binding"]["identity"] = "foreign".into(),
        });
        assert!(
            !matches!(compile(MANIFEST, &changed), CompileResult::Lowered { .. }),
            "{mutation}"
        );
    }
    let changed = tests::changed_source(SOURCE, |r| r["binding"]["fullyBound"] = false.into());
    assert!(matches!(
        compile(MANIFEST, &changed),
        CompileResult::Lowered { .. }
    ));
}
