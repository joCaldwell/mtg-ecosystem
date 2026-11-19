//! CR 701.9, 601.2c and 608.2b-c: ordinary fixed discards for one announced player.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(crate) fn has_discard(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::TargetPlayerDiscardCards { .. }))
        || p.modes.iter().any(|m| has_discard(&m.program))
}

/// One complete spell or modal child with a single announced player.
pub fn target_player_discard_program(p: &SpellProgram) -> bool {
    p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.modes.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && matches!(p.targets.as_slice(), [t] if t.slot == 0
            && !t.selection_id.is_empty()
            && t.domain == TargetDomain::AnyPlayer
            && t.chooser == Chooser::Controller
            && t.stage == EvaluationStage::Announcement
            && t.filter.is_none())
        && matches!(p.instructions.as_slice(), [Instruction::TargetPlayerDiscardCards {
            target_slot: 0,
            count,
            ..
        }] if *count > Quantity::from(0))
}

pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_discard(&a.program))
        || d.triggered.iter().any(|a| has_discard(&a.program))
    {
        return Err("targeted discard requires an ordinary spell source".into());
    }
    if !d.spell.as_ref().is_some_and(has_discard) {
        return Ok(());
    }
    let c = &d.characteristics;
    if !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || c.basic
        || c.legendary
        || c.snow
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
        return Err("targeted discard requires a complete ordinary spell definition".into());
    }
    Ok(())
}


pub(crate) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let op = json!({"required":["op"],"properties":{"op":{"const":"target_player_discard_cards"}}});
    let free = json!({"$ref":"#/$defs/TargetDiscardFreeProgram"});
    schema["$defs"]["TargetDiscardFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":op}},"modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"properties":{"instructions":{"contains":op}}},"then":{"properties":{
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
                "slot":{"const":0},"selection_id":{"minLength":1},"domain":{"const":"any_player"},
                "chooser":{"const":"controller"},"stage":{"const":"announcement"},"filter":{"type":"null"}
            }}},
            "instructions":{"maxItems":1,"items":{"properties":{
                "target_slot":{"const":0},"count":{"pattern":"^[1-9][0-9]*$"}
            }}}
        }}
    }));
    let cap = json!({"contains":{"const":Capability::TargetPlayerDiscardV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"activated":{"items":{"properties":{"program":free}}},
                      "triggered":{"items":{"properties":{"program":free}}}},
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
        "then":{"properties":{"capabilities":cap,"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},
              "subtypes":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
              "power":{"type":"null"},"toughness":{"type":"null"}}}}},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = targeted_counted_draw::witness();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::TargetPlayerDiscardCards {
        target_slot: 0,
        count: 3.into(),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_discard_validates_selected_modal_program() {
        let mut d = witness();
        let root = d.spell.as_mut().unwrap();
        let child = root.clone();
        root.instructions.clear();
        root.targets.clear();
        root.modes = vec![SpellMode { program: child }; 2];
        d.capabilities = d.requirements();
        d.validate().unwrap();
        assert!(d.capabilities.contains(&Capability::TargetPlayerDiscardV1));
        d.spell.as_mut().unwrap().modes[1].program.targets[0].domain = TargetDomain::OpponentPlayer;
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
}
