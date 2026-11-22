//! Complete ordinary counted-draw programs, CR 121.2/305.2/401.4/608.2c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(crate) fn has_hand_program(p: &SpellProgram) -> bool {
    controller_draw_discard_program(p)
        || p.instructions.iter().any(|i| {
            i.counted_controller_draw().is_some()
                || matches!(
                    i,
                    Instruction::PutHandOnLibrary { .. }
                        | Instruction::GrantLandPlaysThisTurn { .. }
                )
        })
        || p.modes.iter().any(|m| has_hand_program(&m.program))
}
/// Exact counted controller draws followed by an ordered whole-hand return.
pub fn controller_hand_return_program(p: &SpellProgram) -> bool {
    p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.modes.is_empty()
        && p.targets.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && matches!(p.instructions.as_slice(),[kernel!(Draw { player: you!(), amount: fixed!(draw) } @ 0),Instruction::PutHandOnLibrary{count,source_block:0}]if !draw.is_negative()&&!count.is_negative())
}
pub(crate) fn controller_draw_land_allowance_program(p: &SpellProgram) -> bool {
    p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.modes.is_empty()
        && p.targets.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && matches!(
            p.instructions.as_slice(),
            [kernel!(Draw { player: you!(), amount: fixed!(draw) } @ 0),
             Instruction::GrantLandPlaysThisTurn { count, source_block: 0 }]
                if !draw.is_negative() && !count.is_negative()
        )
}
pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if (d.activated.iter().any(|a| has_hand_program(&a.program))
        && !paid_counted_draw::definition_matches(d))
        || (d.triggered.iter().any(|a| has_hand_program(&a.program))
            && !flash_end_step_draw::definition_matches(d))
    {
        return Err("unsupported complete counted controller ability source".into());
    }
    if !d.spell.as_ref().is_some_and(has_hand_program) {
        return Ok(());
    }
    if !d.spell.as_ref().is_some_and(|p| {
        ordinary_sequence::program(p)
            || controller_hand_return_program(p)
            || controller_draw_discard_program(p)
            || controller_counted_draw_program(p)
            || controller_draw_land_allowance_program(p)
    }) {
        return Err("unsupported complete controller draw/hand return spell".into());
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
        return Err("counted controller programs require complete ordinary source".into());
    }
    Ok(())
}
pub(crate) fn extend_schema(s: &mut serde_json::Value) {
    use serde_json::json;
    // CR 121.1-121.2: these programs admit any fixed controller draw, one
    // card included.
    let counted_draw = instruction_schema::at(
        instruction_schema::fixed_controller_draw("^(0|[1-9][0-9]*)$"),
        0,
    );
    s["$defs"]["ControllerDrawLandAllowanceProgram"] = json!({"properties":{
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
        "modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},
        "resolution":{"const":"instructions"},"instructions":{"minItems":2,"maxItems":2,
            "prefixItems":[counted_draw,{"required":["op","count","source_block"],"properties":{
                "op":{"const":"grant_land_plays_this_turn"},
                "count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}
            }}],"items":false}
    }});
    s["$defs"]["ControllerHandReturnProgram"] = json!({"properties":{"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"instructions":{"minItems":2,"maxItems":2,"prefixItems":[counted_draw.clone(),{"required":["op","count","source_block"],"properties":{"op":{"const":"put_hand_on_library"},"count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}}}],"items":false}}});
    s["$defs"]["HandReturnFreeProgram"] = json!({"not":{"$ref":"#/$defs/ControllerDrawDiscardProgram"},"properties":{"instructions":{"items":{"not":{"anyOf":[instruction_schema::any_counted_draw(),{"required":["op"],"properties":{"op":{"enum":["put_hand_on_library","grant_land_plays_this_turn"]}}}]}}},"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/HandReturnFreeProgram"}}}}}});
    s["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"not":{"$ref":"#/$defs/HandReturnFreeProgram"}},"then":{"anyOf":[{"$ref":"#/$defs/ControllerHandReturnProgram"},{"$ref":"#/$defs/ControllerDrawDiscardProgram"},{"$ref":"#/$defs/ControllerCountedDrawProgram"},{"$ref":"#/$defs/ControllerDrawLandAllowanceProgram"},{"$ref":"#/$defs/FlashEndStepDrawProgram"},{"$ref":"#/$defs/PaidCountedDrawProgram"},{"$ref":ordinary_sequence::SCHEMA}]}}));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":{"$ref":"#/$defs/HandReturnFreeProgram"}}}});
    let family = json!({"anyOf":[{"$ref":"#/$defs/ControllerHandReturnProgram"},{"$ref":"#/$defs/ControllerDrawDiscardProgram"},{"$ref":"#/$defs/ControllerCountedDrawProgram"},{"$ref":"#/$defs/ControllerDrawLandAllowanceProgram"},{"$ref":ordinary_sequence::SCHEMA}]});
    let defs = s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap();
    defs.push(json!({"allOf":[{"anyOf":[
        {"properties":{"activated":{"items":{"properties":{"program":{"$ref":"#/$defs/HandReturnFreeProgram"}}}}}},
        {"$ref":"#/$defs/PaidCountedDrawDefinition"}
    ]},{"anyOf":[
        {"properties":{"triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/HandReturnFreeProgram"}}}}}},
        {"$ref":"#/$defs/FlashEndStepDrawDefinition"}
    ]}]}));
    defs.push(json!({"if":has,"then":{"properties":{"spell":family,"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}}},"capabilities":{"allOf":[{"contains":{"const":Capability::NormalLayoutV1}}]}}},"else":{"properties":{"capabilities":{"not":{"contains":{"enum":[Capability::ControllerHandReturnV1,Capability::ControllerDrawDiscardV1,Capability::ControllerDrawLandAllowanceV1]}}}}}}));
    defs.push(json!({"if":{"allOf":[has.clone(),{"not":{"properties":{"spell":{"$ref":ordinary_sequence::SCHEMA}}}}]},
        "then":{"properties":{"capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::CastInstantV1,Capability::CastSorceryV1,Capability::ControllerHandReturnV1,Capability::ControllerDrawDiscardV1,Capability::ControllerCountedDrawV1,Capability::ControllerDrawLandAllowanceV1,Capability::LifeLossV1]}}}}}));
    // LifeLoss is an independent required capability only for the exact tail.
    let loses_life = json!({"properties":{"spell":{"properties":{"instructions":{"contains":instruction_schema::kernel_verb("lose_life")}}}}});
    defs.push(json!({
        "if":has,
        "then":{
            "if":loses_life,
            "then":{"properties":{"capabilities":{"contains":{"const":Capability::LifeLossV1}}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::LifeLossV1}}}}}
        }
    }));
    for (program, cap) in [
        (
            "ControllerCountedDrawProgram",
            Capability::ControllerCountedDrawV1,
        ),
        (
            "ControllerHandReturnProgram",
            Capability::ControllerHandReturnV1,
        ),
        (
            "ControllerDrawDiscardProgram",
            Capability::ControllerDrawDiscardV1,
        ),
        (
            "ControllerDrawLandAllowanceProgram",
            Capability::ControllerDrawLandAllowanceV1,
        ),
    ] {
        let ordinary = json!({"required":["spell"],"properties":{"spell":{"type":"object","$ref":format!("#/$defs/{program}")}}});
        let composed_with = |instruction: serde_json::Value| {
            json!({"required":["spell"],"properties":{"spell":{
                "$ref":ordinary_sequence::SCHEMA,"properties":{"instructions":{"contains":instruction}}
            }}})
        };
        let op = |op: &str| json!({"properties":{"op":{"const":op}}});
        let counted_flash = json!({"allOf":[
            {"$ref":"#/$defs/FlashEndStepDrawDefinition"},
            {"properties":{"triggered":{"contains":{"properties":{"program":{
                "properties":{"instructions":{"contains":instruction_schema::any_counted_draw()}}
            }}}}}}
        ]});
        let family = match cap {
            Capability::ControllerCountedDrawV1 => {
                json!({"anyOf":[ordinary,composed_with(instruction_schema::any_counted_draw()),counted_flash,{"$ref":"#/$defs/PaidCountedDrawDefinition"}]})
            }
            Capability::ControllerDrawLandAllowanceV1 => {
                json!({"anyOf":[ordinary,composed_with(op("grant_land_plays_this_turn"))]})
            }
            _ => ordinary,
        };
        defs.push(json!({"if":family,"then":{"properties":{"capabilities":{"contains":{"const":cap}}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}}));
    }
    defs.push(json!({"if":has,"then":{"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},"then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"contains":{"const":Capability::CastInstantV1},"not":{"contains":{"const":Capability::CastSorceryV1}}}}},"else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"contains":{"const":Capability::CastSorceryV1},"not":{"contains":{"const":Capability::CastInstantV1}}}}}}}));
}
#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = crate::controller_library::witness();
    d.spell.as_mut().unwrap().instructions = vec![
        Instruction::draw(PlayerExpr::you(Context::Spell), 3.into(), 0),
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
    // CR 121.1-121.2: a hand return or land allowance after one card is the
    // same family; Rust validation, capabilities and the exported schema agree.
    #[test]
    fn hand_programs_admit_one_drawn_card_in_rust_and_schema() {
        for tail in [
            Instruction::PutHandOnLibrary {
                count: 1.into(),
                source_block: 0,
            },
            Instruction::GrantLandPlaysThisTurn {
                count: 1.into(),
                source_block: 0,
            },
        ] {
            let mut d = witness();
            d.spell.as_mut().unwrap().instructions = vec![
                Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 0),
                tail.clone(),
            ];
            d.capabilities = d.requirements();
            d.validate().unwrap();
            let family = if matches!(tail, Instruction::PutHandOnLibrary { .. }) {
                Capability::ControllerHandReturnV1
            } else {
                Capability::ControllerDrawLandAllowanceV1
            };
            let p = d.spell.as_ref().unwrap();
            assert!(controller_hand_return_program(p) || controller_draw_land_allowance_program(p));
            assert!(d.capabilities.contains(&family), "{tail:?}");
            assert!(!d.capabilities.contains(&Capability::ControllerDrawV1));
        }
        let schema = catalog_schema();
        for name in [
            "ControllerHandReturnProgram",
            "ControllerDrawLandAllowanceProgram",
        ] {
            let first = &schema["$defs"][name]["properties"]["instructions"]["prefixItems"][0];
            let text = first.to_string();
            assert!(
                !text.contains(r#""not":{"const":"1"}"#),
                "{name} excludes one card"
            );
            assert!(text.contains(r#""verb":{"const":"draw"}"#), "{name}");
        }
    }
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
                        // A truncated tail is now valid counted IR; two counted instructions remain outside every family.
                        p.instructions.push(p.instructions[0].clone());
                    }
                    1 => p.instructions.swap(0, 1),
                    2 => p.instructions.push(p.instructions[1].clone()),
                    3 => {
                        if let kernel!(Draw {
                            player: you!(),
                            amount: fixed!(count)
                        }) = &mut p.instructions[0]
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
