//! Ordinary look/pick-one/remainder programs, CR121.5/400.7/404.3/608.2c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(crate) fn has_selection(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::SelectLibraryCard { .. }))
        || p.modes.iter().any(|m| has_selection(&m.program))
}
/// Exactly one controller library selection; no target, cost or other operation.
pub fn controller_library_selection_program(p: &SpellProgram) -> bool {
    p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.modes.is_empty()
        && p.targets.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && matches!(p.instructions.as_slice(),[Instruction::SelectLibraryCard{amount,source_block:0,..}] if !amount.is_negative())
}
pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_selection(&a.program))
        || d.triggered.iter().any(|a| has_selection(&a.program))
    {
        return Err("library selection requires ordinary spell".into());
    }
    if !d.spell.as_ref().is_some_and(has_selection) {
        return Ok(());
    }
    let c = &d.characteristics;
    if !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
        || !c.additional_types.is_empty()
        || c.basic
        || c.legendary
        || c.power.is_some()
        || c.toughness.is_some()
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
    {
        return Err("library selection requires complete ordinary source".into());
    }
    Ok(())
}
pub(crate) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    schema["$defs"]["ControllerLibrarySelectionProgram"] = json!({"properties":{"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"instructions":{"minItems":1,"maxItems":1,"items":{"required":["op","amount","remainder","source_block"],"properties":{"op":{"const":"select_library_card"},"amount":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"remainder":{"enum":["bottom","graveyard"]},"source_block":{"const":0}}}}}});
    schema["$defs"]["SelectionFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"select_library_card"}}}}},"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/SelectionFreeProgram"}}}}}});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"not":{"$ref":"#/$defs/SelectionFreeProgram"}},"then":{"$ref":"#/$defs/ControllerLibrarySelectionProgram"}}));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":{"$ref":"#/$defs/SelectionFreeProgram"}}}});
    let defs = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    defs.push(json!({"properties":{"activated":{"items":{"properties":{"program":{"$ref":"#/$defs/SelectionFreeProgram"}}}},"triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/SelectionFreeProgram"}}}}}}));
    defs.push(json!({"if":has,"then":{"properties":{"spell":{"$ref":"#/$defs/ControllerLibrarySelectionProgram"},"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}}},"capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::CastInstantV1,Capability::CastSorceryV1,Capability::ControllerLibrarySelectionV1]},"allOf":[{"contains":{"const":Capability::NormalLayoutV1}},{"contains":{"const":Capability::ControllerLibrarySelectionV1}}]}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerLibrarySelectionV1}}}}}}));
    defs.push(json!({"if":has,"then":{"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},"then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"contains":{"const":Capability::CastInstantV1},"not":{"contains":{"const":Capability::CastSorceryV1}}}}},"else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"contains":{"const":Capability::CastSorceryV1},"not":{"contains":{"const":Capability::CastInstantV1}}}}}}}));
}
#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = crate::controller_library::witness();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::SelectLibraryCard {
        amount: 3.into(),
        remainder: LibraryRemainder::Bottom,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_selection_remainder_wire_uses_only_schema_strings() {
        for name in ["bottom", "graveyard"] {
            assert!(serde_json::from_value::<LibraryRemainder>(serde_json::json!(name)).is_ok());
            assert!(
                serde_json::from_value::<LibraryRemainder>(serde_json::json!({name:null})).is_err()
            );
        }
        for invalid in [
            serde_json::json!("exile"),
            serde_json::Value::Null,
            serde_json::json!({"bottom":{}}),
        ] {
            assert!(serde_json::from_value::<LibraryRemainder>(invalid).is_err());
        }
    }
    #[test]
    fn library_selection_closed_shape_and_exact_capability() {
        for amount in ["0", "3", "999999999999999999999999"] {
            for remainder in [LibraryRemainder::Bottom, LibraryRemainder::Graveyard] {
                let mut d = witness();
                d.spell.as_mut().unwrap().instructions = vec![Instruction::SelectLibraryCard {
                    amount: amount.parse().unwrap(),
                    remainder,
                    source_block: 0,
                }];
                d.capabilities = d.requirements();
                d.validate().unwrap();
                for variant in 0..5 {
                    let mut b = d.clone();
                    let p = b.spell.as_mut().unwrap();
                    match variant {
                        0 => p.instructions.push(Instruction::DrawCards {
                            recipient: None,
                            count: 1.into(),
                            source_block: 0,
                        }),
                        1 => {
                            if let Instruction::SelectLibraryCard { amount, .. } =
                                &mut p.instructions[0]
                            {
                                *amount = (-1).into()
                            }
                        }
                        2 => {
                            if let Instruction::SelectLibraryCard { source_block, .. } =
                                &mut p.instructions[0]
                            {
                                *source_block = 1
                            }
                        }
                        3 => p.resolution = SpellResolution::Permanent,
                        _ => {
                            b.capabilities.insert(Capability::ControllerDrawV1);
                        }
                    };
                    if variant != 4 {
                        b.capabilities = b.requirements();
                    }
                    assert!(b.validate().is_err(), "{variant}");
                }
            }
        }
    }
}

#[cfg(test)]
#[test]
fn library_families_reject_optional_cost_and_conditional_draw_union() {
    for mut d in [
        witness(),
        crate::library_order::witness(),
        crate::hand_return::witness(),
        crate::draw_discard::witness(),
        crate::controller_counted_draw::witness(),
    ] {
        d.validate().unwrap();
        let identity = "source-bound-test-cost".to_string();
        d.spell
            .as_mut()
            .unwrap()
            .optional_additional_costs
            .push(OptionalAdditionalManaCost {
                identity: identity.clone(),
                source_block: 0,
                mana: ManaCost {
                    generic: 1.into(),
                    colored: vec![],
                },
            });
        d.capabilities = d.requirements();
        assert!(!controller_library_selection_program(
            d.spell.as_ref().unwrap()
        ));
        assert!(!controller_library_order_program(d.spell.as_ref().unwrap()));
        assert!(!controller_hand_return_program(d.spell.as_ref().unwrap()));
        assert!(!controller_draw_discard_program(d.spell.as_ref().unwrap()));
        assert!(!controller_counted_draw_program(d.spell.as_ref().unwrap()));
        assert!(d.validate().is_err());
        let p = d.spell.as_mut().unwrap();
        p.optional_additional_costs.clear();
        p.instructions.push(Instruction::DrawCardsIfAdditionalCost {
            cost_identity: identity,
            count: 1.into(),
            source_block: 0,
        });
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    let schema = catalog_schema();
    for family in [
        "ControllerLibrarySelectionProgram",
        "ControllerLibraryOrderProgram",
        "ControllerHandReturnProgram",
        "ControllerDrawDiscardProgram",
        "ControllerCountedDrawProgram",
    ] {
        assert_eq!(
            schema["$defs"][family]["properties"]["optional_additional_costs"]["maxItems"],
            0
        );
    }
}
