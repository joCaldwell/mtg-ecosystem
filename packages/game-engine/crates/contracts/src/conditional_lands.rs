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
    let optional = d.static_abilities.iter().any(|a| {
        matches!(a.effect, StaticEffect::EntersTappedUnlessPayLife { .. })
    });
    let typed = d.characteristics.kind == CardKind::Land
        && !d.characteristics.basic
        && !d.characteristics.subtypes.is_empty();
    if optional && d.static_abilities.len() != 1 {
        return Err("optional-life entry requires one uncomposed replacement".into());
    }
    if conditional || optional || typed {
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
                        StaticEffect::EntersTappedUnlessPayLife { amount } => amount.is_negative(),
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
    let life_effect = json!({"type":"object","required":["enters_tapped_unless_pay_life"]});
    s["$defs"]["StaticEffect"]["allOf"].as_array_mut().unwrap().push(json!({"properties":{
        "enters_tapped_unless_pay_life":{"properties":{"amount":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"}}}
    }}));
    let optional = json!({"required":["static_abilities"],"properties":{"static_abilities":{"contains":{"required":["effect"],"properties":{"effect":life_effect}}}}});
    let conditional = json!({"required":["static_abilities"],"properties":{"static_abilities":{"contains":{"required":["effect"],"properties":{"effect":effect}}}}});
    let typed = json!({"required":["characteristics"],"properties":{"characteristics":{"required":["kind","basic","subtypes"],"properties":{"kind":{"const":"land"},"basic":{"const":false},"subtypes":{"minItems":1}}}}});
    for (present, cap) in [
        (conditional.clone(), Capability::ConditionalBasicLandEntryV1),
        (optional.clone(), Capability::OptionalLifeLandEntryV1),
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
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"anyOf":[conditional,optional.clone(),typed]},"then":{"properties":{
        "characteristics":{"properties":{"kind":{"const":"land"},"basic":{"const":false},"additional_types":{"maxItems":0},"subtypes":{"minItems":1,"uniqueItems":true,"items":{"enum":["Plains","Island","Swamp","Mountain","Forest"]}},"mana_cost":{"properties":{"generic":{"const":"0"},"colored":{"maxItems":0}}},"power":{"type":"null"},"toughness":{"type":"null"}}},
        "intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},"x_mana_cost":{"const":false},
        "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"keywords":{"maxItems":0},
        "static_abilities":{"items":{"properties":{"functioning_zone":{"const":"battlefield"},"effect":{"anyOf":[{"const":"enters_tapped"},effect,life_effect]}}}},
        "capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::LandPlayV1,Capability::TypedNonbasicIntrinsicManaV1,Capability::ConditionalBasicLandEntryV1,Capability::OptionalLifeLandEntryV1,Capability::EntersTappedV1]},"allOf":[{"contains":{"const":Capability::NormalLayoutV1}},{"contains":{"const":Capability::LandPlayV1}}]}
    }}}));
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":optional,"then":{"properties":{
        "static_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{"effect":{"required":["enters_tapped_unless_pay_life"]}}}}
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
    pub(crate) fn optional_land() -> Definition {
        let mut d = land();
        d.oracle_id = "00000000-0000-0000-0000-000000000036".into();
        d.static_abilities[0].effect = StaticEffect::EntersTappedUnlessPayLife { amount: 2.into() };
        d.capabilities = d.requirements();
        d
    }
    #[test]
    fn optional_life_lands_contract_parameters_and_composition_fences() {
        for n in ["0", "1", "2", "3", "999999999999999999999999999999999999999999999999"] {
            let mut d = optional_land();
            d.static_abilities[0].effect = StaticEffect::EntersTappedUnlessPayLife { amount: n.parse().unwrap() };
            d.validate().unwrap();
            let roundtrip: Definition = strict_read(&serde_json::to_string(&d).unwrap()).unwrap();
            assert_eq!(d, roundtrip);
        }
        for case in 0..8 {
            let mut d = optional_land();
            match case {
                0 => d.static_abilities[0].effect = StaticEffect::EntersTappedUnlessPayLife { amount: (-1).into() },
                1 => d.static_abilities.push(d.static_abilities[0].clone()),
                2 => d.static_abilities.push(StaticAbility { functioning_zone: FunctioningZone::Battlefield, effect: StaticEffect::EntersTapped }),
                3 => d.static_abilities.push(land().static_abilities.remove(0)),
                4 => d.characteristics.kind = CardKind::Artifact,
                5 => { d.capabilities.remove(&Capability::OptionalLifeLandEntryV1); },
                6 => { d.capabilities.insert(Capability::ConditionalBasicLandEntryV1); },
                _ => d.characteristics.subtypes.clear(),
            }
            assert!(d.validate().is_err(), "{case}");
        }
    }
    #[test]
    fn optional_life_lands_wrong_null_and_missing_ir_operands_reject() {
        use serde_json::json;
        for amount in [json!(null), json!(2), json!("02"), json!("2.0"), json!("-0"), json!({})] {
            let mut value = serde_json::to_value(optional_land()).unwrap();
            value["static_abilities"][0]["effect"]["enters_tapped_unless_pay_life"]["amount"] = amount;
            assert!(strict_read::<Definition>(&value.to_string()).is_err());
        }
        for effect in [json!({"enters_tapped_unless_pay_life":{}}), json!({"enters_tapped_unless_pay_life":null}), json!({"enters_tapped_unless_pay_life":{"amount":"2","payer":"owner"}})] {
            let mut value = serde_json::to_value(optional_land()).unwrap();
            value["static_abilities"][0]["effect"] = effect;
            assert!(strict_read::<Definition>(&value.to_string()).is_err());
        }
    }
    #[test]
    fn optional_life_lands_do_not_broaden_ramos_cost_or_undeclared_draw_fences() {
        let effects = Catalog::read(include_str!("../../../tests/fixtures/expected-effects-ir.json")).unwrap();
        let program = effects.definitions.iter().find(|d| d.name == "Giant Growth").unwrap().spell.clone().unwrap();
        for costs in [false, true] {
            let mut p = program.clone();
            p.instructions = vec![Instruction::DrawCardsIfAdditionalCost {
                cost_identity: "undeclared".into(), count: 1.into(), source_block: 0,
            }];
            if costs {
                p.optional_additional_costs = vec![OptionalAdditionalManaCost {
                    identity: "undeclared".into(), source_block: 0,
                    mana: ManaCost { generic: 0.into(), colored: vec![] },
                }];
            }
            assert!(optional_costs::validate_program(&p).is_err());
            // Exercise land, activation, trigger and mode contexts independently.
            let mut land = optional_land();
            land.spell = Some(p.clone());
            land.capabilities = land.requirements();
            assert!(land.validate().is_err());
            for source in effects.definitions.iter().filter(|d| !d.activated.is_empty() || !d.triggered.is_empty()) {
                let mut d = source.clone();
                d.source_labels = None;
                if let Some(a) = d.activated.first_mut() { a.program = p.clone(); }
                if let Some(a) = d.triggered.first_mut() { a.program = p.clone(); }
                d.capabilities = d.requirements();
                assert!(d.validate().is_err(), "{}", d.name);
            }
            let mut d = effects.definitions.iter().find(|d| d.name == "Giant Growth").unwrap().clone();
            d.source_labels = None;
            let spell = d.spell.as_mut().unwrap();
            spell.instructions.clear();
            spell.targets.clear();
            spell.modes = vec![SpellMode { program: p.clone() }, SpellMode { program: p }];
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
        }
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
