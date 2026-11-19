//! A standalone spell with one announced player target. CR 104.3e, 601.2c, 608.2b.
use super::*;
use serde_json::{Value, json};

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if !d.capabilities.contains(&Capability::TargetedGameLossV1) {
        return Ok(());
    }
    let c = &d.characteristics;
    let ordinary = c.kind == CardKind::Sorcery
        && c.subtypes.is_empty()
        && c.additional_types.is_empty()
        && !c.basic
        && !c.snow
        && !c.legendary
        && !d.x_mana_cost
        && d.intrinsic_zone.is_none()
        && d.keywords.is_empty()
        && d.static_abilities.is_empty()
        && d.triggered.is_empty()
        && d.activated.is_empty()
        && d.mana_abilities.is_empty();
    if ordinary && d.spell.as_ref().is_some_and(|p| {
        p.timing == SpellTiming::MainPhase && p.functioning_zone == FunctioningZone::Stack
            && p.resolution == SpellResolution::Instructions && p.modes.is_empty()
            && p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty()
            && matches!(p.targets.as_slice(), [t] if t.slot == 0 && t.domain == TargetDomain::AnyPlayer
                && t.filter.is_none() && t.chooser == Chooser::Controller && t.stage == EvaluationStage::Announcement && !t.selection_id.is_empty())
            && matches!(p.instructions.as_slice(), [Instruction::LoseGameTargetPlayer {target_slot:0,source_block:0}])
    }) { Ok(()) } else { Err("unsupported complete targeted game loss spell".into()) }
}

pub(super) fn extend_schema(schema: &mut Value) {
    let instruction =
        json!({"required":["op"],"properties":{"op":{"const":"lose_game_target_player"}}});
    let free = json!({"$ref":"#/$defs/GameLossFreeProgram"});
    schema["$defs"]["GameLossFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":instruction}},"modes":{"items":{"properties":{"program":free}}}
    }});
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":free}}});
    let caps = [
        Capability::NormalLayoutV1,
        Capability::CastSorceryV1,
        Capability::AnyPlayerTargetV1,
        Capability::TargetedGameLossV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({"properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}}}}),
        json!({"if":has,"then":{"properties":{
            "capabilities":{"minItems":caps.len(),"maxItems":caps.len(),"uniqueItems":true,"items":{"enum":caps}},
            "characteristics":{"properties":{"kind":{"const":"sorcery"},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                "basic":{"const":false},"snow":{"const":false},"legendary":{"const":false}}},
            "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},"keywords":{"maxItems":0},
            "triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "spell":{"properties":{"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
                "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},
                "targets":{"minItems":1,"maxItems":1,"items":{"properties":{"domain":{"const":"any_player"},"slot":{"const":0},
                    "selection_id":{"minLength":1},"chooser":{"const":"controller"},"stage":{"const":"announcement"},"filter":{"type":"null"}}}},
                "instructions":{"minItems":1,"maxItems":1,"items":{"const":Instruction::LoseGameTargetPlayer{target_slot:0,source_block:0}}}
            }}
        }},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::TargetedGameLossV1}}}}}})
    ]);
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = targeted_counted_draw::witness();
    d.characteristics.kind = CardKind::Sorcery;
    d.characteristics.subtypes.clear();
    d.name = "Synthetic Targeted Loss".into();
    d.source_labels = None;
    let p = d.spell.as_mut().unwrap();
    p.timing = SpellTiming::MainPhase;
    p.instructions = vec![Instruction::LoseGameTargetPlayer {
        target_slot: 0,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn targeted_game_loss_contract_rejects_added_and_unbound_effects() {
    let d = witness();
    d.validate().unwrap();
    for variant in 0..3 {
        let mut malformed = d.clone();
        let p = malformed.spell.as_mut().unwrap();
        match variant {
            0 => p.instructions.push(p.instructions[0].clone()),
            1 => p.targets[0].domain = TargetDomain::OpponentPlayer,
            _ => {
                p.instructions[0] = Instruction::LoseGameTargetPlayer {
                    target_slot: 1,
                    source_block: 0,
                }
            }
        }
        malformed.capabilities = malformed.requirements();
        assert!(malformed.validate().is_err());
    }
}
