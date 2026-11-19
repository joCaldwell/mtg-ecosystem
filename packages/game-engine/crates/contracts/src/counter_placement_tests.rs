use super::*;
pub(super) fn provider(modification: CounterPlacementModification) -> Definition {
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let mut d = c
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    d.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ControllerCreaturePlusOneCounterReplacement { modification },
    }];
    d.capabilities = d.requirements();
    d
}
#[test]
fn counter_placement_contract_parameters_closure_and_real_guards() {
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let base = c
        .definitions
        .iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    for modification in [
        CounterPlacementModification::Add { amount: 7.into() },
        CounterPlacementModification::Multiply {
            factor: "10000000000000000000000000000000000000".parse().unwrap(),
        },
    ] {
        let mut d = base.clone();
        d.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControllerCreaturePlusOneCounterReplacement {
                modification: modification.clone(),
            },
        }];
        d.capabilities = d.requirements();
        d.validate().unwrap();
        assert_eq!(
            strict_read::<Definition>(&canonical(&d).unwrap()).unwrap(),
            d
        );
        let mut missing = d.clone();
        missing
            .capabilities
            .remove(&Capability::ControllerCreaturePlusOneCounterReplacementV1);
        assert!(missing.validate().is_err());
        for bad_zone in [FunctioningZone::Stack] {
            let mut bad = d.clone();
            bad.static_abilities[0].functioning_zone = bad_zone;
            bad.capabilities = bad.requirements();
            assert!(bad.validate().is_err());
        }
        for parameter in [Quantity::zero(), (-1).into()] {
            let mut bad = d.clone();
            bad.static_abilities[0].effect =
                StaticEffect::ControllerCreaturePlusOneCounterReplacement {
                    modification: match &modification {
                        CounterPlacementModification::Add { .. } => {
                            CounterPlacementModification::Add { amount: parameter }
                        }
                        CounterPlacementModification::Multiply { .. } => {
                            CounterPlacementModification::Multiply { factor: parameter }
                        }
                    },
                };
            bad.capabilities = bad.requirements();
            assert!(bad.validate().is_err());
        }
        let mut bad = d.clone();
        bad.characteristics.kind = CardKind::Land;
        bad.characteristics.power = None;
        bad.characteristics.toughness = None;
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
        assert_eq!(
            modification.apply(&3.into()).to_string(),
            match modification {
                CounterPlacementModification::Add { .. } => "10",
                CounterPlacementModification::Multiply { .. } =>
                    "30000000000000000000000000000000000000",
            }
        );
    }
    for invalid in [
        serde_json::json!({"operation":"add","amount":"1","factor":"2"}),
        serde_json::json!({"operation":"multiply","factor":"2","extra":true}),
    ] {
        assert!(serde_json::from_value::<CounterPlacementModification>(invalid).is_err());
    }
    assert_eq!(CURRENT_IR_VERSION, 1);
}
