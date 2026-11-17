//! Complete ordinary order/optional-shuffle/draw, CR401.4/608.2c/701.24.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(crate) fn has_order(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::OrderLibraryTop { .. } | Instruction::OptionalShuffleLibrary { .. }
        )
    }) || p.modes.iter().any(|m| has_order(&m.program))
}
/// One fixed controller top ordering, optional shuffle, then one mandatory draw.
pub fn controller_library_order_program(p: &SpellProgram) -> bool {
    p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.modes.is_empty()
        && p.targets.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && matches!(p.instructions.as_slice(),[Instruction::OrderLibraryTop{amount,source_block:0},Instruction::OptionalShuffleLibrary{source_block:0},Instruction::DrawCards{recipient:None,count,source_block:1}]if !amount.is_negative()&&*count==1.into())
}
pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_order(&a.program))
        || d.triggered.iter().any(|a| has_order(&a.program))
    {
        return Err("library ordering requires an ordinary spell".into());
    }
    if !d.spell.as_ref().is_some_and(has_order) {
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
        return Err("library ordering requires complete ordinary source".into());
    }
    Ok(())
}
pub(crate) fn extend_schema(s: &mut serde_json::Value) {
    use serde_json::json;
    s["$defs"]["ControllerLibraryOrderProgram"] = json!({"properties":{"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"instructions":{"minItems":3,"maxItems":3,"prefixItems":[{"required":["op","amount","source_block"],"properties":{"op":{"const":"order_library_top"},"amount":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}}},{"required":["op","source_block"],"properties":{"op":{"const":"optional_shuffle_library"},"source_block":{"const":0}}},{"required":["op","count","source_block"],"properties":{"op":{"const":"draw_cards"},"count":{"const":"1"},"recipient":{"type":"null"},"source_block":{"const":1}}}],"items":false}}});
    s["$defs"]["OrderFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"enum":["order_library_top","optional_shuffle_library"]}}}}},"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/OrderFreeProgram"}}}}}});
    s["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"not":{"$ref":"#/$defs/OrderFreeProgram"}},"then":{"$ref":"#/$defs/ControllerLibraryOrderProgram"}}));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":{"$ref":"#/$defs/OrderFreeProgram"}}}});
    let defs = s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap();
    defs.push(json!({"properties":{"activated":{"items":{"properties":{"program":{"$ref":"#/$defs/OrderFreeProgram"}}}},"triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/OrderFreeProgram"}}}}}}));
    defs.push(json!({"if":has,"then":{"properties":{"spell":{"$ref":"#/$defs/ControllerLibraryOrderProgram"},"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}}},"capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::CastInstantV1,Capability::CastSorceryV1,Capability::ControllerLibraryOrderV1,Capability::ControllerDrawV1]},"allOf":[{"contains":{"const":Capability::NormalLayoutV1}},{"contains":{"const":Capability::ControllerLibraryOrderV1}},{"contains":{"const":Capability::ControllerDrawV1}}]}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerLibraryOrderV1}}}}}}));
    defs.push(json!({"if":has,"then":{"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},"then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"contains":{"const":Capability::CastInstantV1},"not":{"contains":{"const":Capability::CastSorceryV1}}}}},"else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"contains":{"const":Capability::CastSorceryV1},"not":{"contains":{"const":Capability::CastInstantV1}}}}}}}));
}
#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = crate::controller_library::witness();
    d.spell.as_mut().unwrap().instructions = vec![
        Instruction::OrderLibraryTop {
            amount: 3.into(),
            source_block: 0,
        },
        Instruction::OptionalShuffleLibrary { source_block: 0 },
        Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 1,
        },
    ];
    d.capabilities = d.requirements();
    d
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_order_complete_program_count_and_scope() {
        for count in ["0", "3", "999999999999999999999999"] {
            let mut d = witness();
            if let Instruction::OrderLibraryTop { amount, .. } =
                &mut d.spell.as_mut().unwrap().instructions[0]
            {
                *amount = count.parse().unwrap();
            }
            d.validate().unwrap();
            for variant in 0..7 {
                let mut b = d.clone();
                let p = b.spell.as_mut().unwrap();
                match variant {
                    0 => {
                        p.instructions.remove(1);
                    }
                    1 => {
                        p.instructions.pop();
                    }
                    2 => p.instructions.swap(0, 1),
                    3 => p.instructions.push(p.instructions[2].clone()),
                    4 => {
                        if let Instruction::OrderLibraryTop { amount, .. } = &mut p.instructions[0]
                        {
                            *amount = (-1).into()
                        }
                    }
                    5 => {
                        if let Instruction::OptionalShuffleLibrary { source_block } =
                            &mut p.instructions[1]
                        {
                            *source_block = 1
                        }
                    }
                    _ => b.characteristics.kind = CardKind::Enchantment,
                };
                b.capabilities = b.requirements();
                assert!(b.validate().is_err(), "{variant}");
            }
        }
    }
}
