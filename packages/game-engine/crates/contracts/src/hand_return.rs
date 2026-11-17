//! Complete ordinary counted-draw hand programs, CR121.2/401.4/608.2c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(crate) fn has_hand_program(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::DrawControllerCards { .. } | Instruction::PutHandOnLibrary { .. }
        )
    }) || p.modes.iter().any(|m| has_hand_program(&m.program))
}
/// Exact counted controller draws followed by an ordered whole-hand return.
pub fn controller_hand_return_program(p: &SpellProgram) -> bool {
    p.additional_sacrifice.is_none()
        && p.modes.is_empty()
        && p.targets.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && matches!(p.instructions.as_slice(),[Instruction::DrawControllerCards{count:draw,source_block:0},Instruction::PutHandOnLibrary{count,source_block:0}]if !draw.is_negative()&&!count.is_negative())
}
pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_hand_program(&a.program))
        || d.triggered.iter().any(|a| has_hand_program(&a.program))
    {
        return Err("hand return requires an ordinary spell".into());
    }
    if !d.spell.as_ref().is_some_and(has_hand_program) {
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
        return Err("hand return requires complete ordinary source".into());
    }
    Ok(())
}
pub(crate) fn extend_schema(s: &mut serde_json::Value) {
    use serde_json::json;
    s["$defs"]["ControllerHandReturnProgram"] = json!({"properties":{"additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"instructions":{"minItems":2,"maxItems":2,"prefixItems":[{"required":["op","count","source_block"],"properties":{"op":{"const":"draw_controller_cards"},"count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}}},{"required":["op","count","source_block"],"properties":{"op":{"const":"put_hand_on_library"},"count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}}}],"items":false}}});
    s["$defs"]["HandReturnFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"enum":["draw_controller_cards","put_hand_on_library"]}}}}},"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/HandReturnFreeProgram"}}}}}});
    s["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"not":{"$ref":"#/$defs/HandReturnFreeProgram"}},"then":{"anyOf":[{"$ref":"#/$defs/ControllerHandReturnProgram"},{"$ref":"#/$defs/ControllerDrawDiscardProgram"}]}}));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":{"$ref":"#/$defs/HandReturnFreeProgram"}}}});
    let family = json!({"anyOf":[{"$ref":"#/$defs/ControllerHandReturnProgram"},{"$ref":"#/$defs/ControllerDrawDiscardProgram"}]});
    let defs = s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap();
    defs.push(json!({"properties":{"activated":{"items":{"properties":{"program":{"$ref":"#/$defs/HandReturnFreeProgram"}}}},"triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/HandReturnFreeProgram"}}}}}}));
    defs.push(json!({"if":has,"then":{"properties":{"spell":family,"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}}},"capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::CastInstantV1,Capability::CastSorceryV1,Capability::ControllerHandReturnV1,Capability::ControllerDrawDiscardV1]},"allOf":[{"contains":{"const":Capability::NormalLayoutV1}}]}}},"else":{"properties":{"capabilities":{"not":{"contains":{"enum":[Capability::ControllerHandReturnV1,Capability::ControllerDrawDiscardV1]}}}}}}));
    for (program, cap) in [
        (
            "ControllerHandReturnProgram",
            Capability::ControllerHandReturnV1,
        ),
        (
            "ControllerDrawDiscardProgram",
            Capability::ControllerDrawDiscardV1,
        ),
    ] {
        defs.push(json!({"if":{"required":["spell"],"properties":{"spell":{"type":"object","$ref":format!("#/$defs/{program}")}}},"then":{"properties":{"capabilities":{"contains":{"const":cap}}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}}));
    }
    defs.push(json!({"if":has,"then":{"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},"then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"contains":{"const":Capability::CastInstantV1},"not":{"contains":{"const":Capability::CastSorceryV1}}}}},"else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"contains":{"const":Capability::CastSorceryV1},"not":{"contains":{"const":Capability::CastInstantV1}}}}}}}));
}
#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = crate::controller_library::witness();
    d.spell.as_mut().unwrap().instructions = vec![
        Instruction::DrawControllerCards {
            count: 3.into(),
            source_block: 0,
        },
        Instruction::PutHandOnLibrary {
            count: 2.into(),
            source_block: 0,
        },
    ];
    d.capabilities = d.requirements();
    d
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hand_return_complete_count_and_scope() {
        for count in ["0", "2", "999999999999999999999999"] {
            let mut d = witness();
            if let Instruction::PutHandOnLibrary { count: c, .. } =
                &mut d.spell.as_mut().unwrap().instructions[1]
            {
                *c = count.parse().unwrap();
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            for variant in 0..6 {
                let mut b = d.clone();
                let p = b.spell.as_mut().unwrap();
                match variant {
                    0 => {
                        p.instructions.pop();
                    }
                    1 => p.instructions.swap(0, 1),
                    2 => p.instructions.push(p.instructions[1].clone()),
                    3 => {
                        if let Instruction::DrawControllerCards { count, .. } =
                            &mut p.instructions[0]
                        {
                            *count = (-1).into()
                        }
                    }
                    4 => {
                        if let Instruction::PutHandOnLibrary { source_block, .. } =
                            &mut p.instructions[1]
                        {
                            *source_block = 1
                        }
                    }
                    _ => b.characteristics.kind = CardKind::Creature,
                };
                b.capabilities = b.requirements();
                assert!(b.validate().is_err(), "{variant}");
            }
        }
    }
}
