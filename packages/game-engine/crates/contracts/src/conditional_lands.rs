//! Pure typed land entry/mana, CR 205.4c, 305.6, 614.12 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let conditional = d.static_abilities.iter().any(|a| {
        matches!(
            a.effect,
            StaticEffect::EntersTappedUnlessControllerBasicLands { .. }
        )
    });
    let typed = d.characteristics.kind == CardKind::Land
        && !d.characteristics.basic
        && !d.characteristics.subtypes.is_empty();
    if conditional || typed {
        let c = &d.characteristics;
        if c.kind != CardKind::Land
            || c.basic
            || !c.additional_types.is_empty()
            || c.subtypes.is_empty()
            || c.subtypes.iter().any(|s| basic_land_color(s).is_none())
            || c.subtypes.iter().collect::<BTreeSet<_>>().len() != c.subtypes.len()
            || !d.mana_abilities.is_empty()
            || !d.activated.is_empty()
            || !d.triggered.is_empty()
            || !d.keywords.is_empty()
            || d.x_mana_cost
            || d.static_abilities.iter().any(|a| {
                a.functioning_zone != FunctioningZone::Battlefield
                    || match &a.effect {
                        StaticEffect::EntersTapped => false,
                        StaticEffect::EntersTappedUnlessControllerBasicLands { minimum } => {
                            minimum.is_negative()
                        }
                        _ => true,
                    }
            })
        {
            return Err("unsupported complete pure typed nonbasic land".into());
        }
    }
    Ok(())
}
pub(super) fn schema(s: &mut serde_json::Value) {
    use serde_json::json;
    let effect =
        json!({"type":"object","required":["enters_tapped_unless_controller_basic_lands"]});
    s["$defs"]["StaticEffect"].as_object_mut().unwrap().entry("allOf").or_insert_with(||json!([])).as_array_mut().unwrap().push(json!({"properties":{
        "enters_tapped_unless_controller_basic_lands":{"properties":{"minimum":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"}}}
    }}));
    let conditional = json!({"required":["static_abilities"],"properties":{"static_abilities":{"contains":{"required":["effect"],"properties":{"effect":effect}}}}});
    let typed = json!({"required":["characteristics"],"properties":{"characteristics":{"required":["kind","basic","subtypes"],"properties":{"kind":{"const":"land"},"basic":{"const":false},"subtypes":{"minItems":1}}}}});
    for (present, cap) in [
        (conditional.clone(), Capability::ConditionalBasicLandEntryV1),
        (typed.clone(), Capability::TypedNonbasicIntrinsicManaV1),
    ] {
        s["$defs"]["Definition"]["allOf"]
            .as_array_mut()
            .unwrap()
            .push(json!({"if":present,
                "then":{"properties":{"capabilities":{"contains":{"const":cap}}}},
                "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
            }));
    }
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"anyOf":[conditional,typed]},"then":{"properties":{
        "characteristics":{"properties":{"kind":{"const":"land"},"basic":{"const":false},"additional_types":{"maxItems":0},"subtypes":{"minItems":1,"uniqueItems":true,"items":{"enum":["Plains","Island","Swamp","Mountain","Forest"]}},"mana_cost":{"properties":{"generic":{"const":"0"},"colored":{"maxItems":0}}},"power":{"type":"null"},"toughness":{"type":"null"}}},
        "intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},"x_mana_cost":{"const":false},
        "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"keywords":{"maxItems":0},
        "static_abilities":{"items":{"properties":{"functioning_zone":{"const":"battlefield"},"effect":{"anyOf":[{"const":"enters_tapped"},effect]}}}},
        "capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::LandPlayV1,Capability::TypedNonbasicIntrinsicManaV1,Capability::ConditionalBasicLandEntryV1,Capability::EntersTappedV1]},"allOf":[{"contains":{"const":Capability::NormalLayoutV1}},{"contains":{"const":Capability::LandPlayV1}}]}
    }}}));
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":typed,"then":{"allOf":[
        {"if":{"properties":{"static_abilities":{"contains":{"properties":{"effect":{"const":"enters_tapped"}}}}}},"then":{"properties":{"capabilities":{"contains":{"const":Capability::EntersTappedV1}}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::EntersTappedV1}}}}}}
    ]}}));
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    pub(crate) fn land() -> Definition {
        let mut d = Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json"))
            .unwrap()
            .definitions
            .into_iter()
            .find(|d| d.characteristics.basic)
            .unwrap();
        d.source_labels = None; // Synthetic contract shape, not printed-source evidence.
        d.characteristics.basic = false;
        d.characteristics.subtypes = vec!["Mountain".into(), "Forest".into()];
        d.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTappedUnlessControllerBasicLands { minimum: 2.into() },
        }];
        d.capabilities = d.requirements();
        d
    }
    #[test]
    fn conditional_lands_contract_parameters_and_fences() {
        for n in [
            "0",
            "1",
            "2",
            "3",
            "9999999999999999999999999999999999999999999",
        ] {
            let mut d = land();
            d.static_abilities[0].effect = StaticEffect::EntersTappedUnlessControllerBasicLands {
                minimum: n.parse().unwrap(),
            };
            d.validate().unwrap();
        }
        for mutation in 0..10 {
            let mut d = land();
            match mutation {
                0 => d.characteristics.basic = true,
                1 => d.characteristics.subtypes.push("Forest".into()),
                2 => d.characteristics.subtypes.push("Desert".into()),
                3 => d.characteristics.additional_types.push(CardKind::Artifact),
                4 => d.static_abilities[0].functioning_zone = FunctioningZone::Stack,
                5 => {
                    d.static_abilities[0].effect =
                        StaticEffect::EntersTappedUnlessControllerBasicLands {
                            minimum: (-1).into(),
                        }
                }
                6 => d.characteristics.subtypes.clear(),
                7 => {
                    d.capabilities
                        .remove(&Capability::TypedNonbasicIntrinsicManaV1);
                }
                8 => {
                    d.capabilities.insert(Capability::BasicLandManaV1);
                }
                _ => {
                    d.capabilities
                        .remove(&Capability::ConditionalBasicLandEntryV1);
                }
            };
            assert!(d.validate().is_err(), "{mutation}");
        }
    }
}
