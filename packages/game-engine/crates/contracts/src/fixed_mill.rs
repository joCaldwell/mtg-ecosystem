//! Ordinary fixed-count milling. CR701.17/109.5/608.2b-c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(crate) fn has_mill(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::MillCards { .. }))
        || p.modes.iter().any(|m| has_mill(&m.program))
}
/// Exactly one controller/target-player mill, optionally with one controller draw.
pub fn fixed_mill_program(p: &SpellProgram) -> bool {
    if p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
    {
        return false;
    }
    let target = match p.targets.as_slice() {
        [] => None,
        [t] if t.slot == 0
            && !t.selection_id.is_empty()
            && t.chooser == Chooser::Controller
            && t.stage == EvaluationStage::Announcement
            && t.domain == TargetDomain::AnyPlayer
            && t.filter.is_none() =>
        {
            Some(0)
        }
        _ => return false,
    };
    let mill = |i: &Instruction| matches!(i,Instruction::MillCards {recipient,count,..} if !count.is_negative() && match (target,recipient) {(None,None)=>true,(Some(0),Some(LifeRecipient::Target {target_slot:0}))=>true,_=>false});
    let draw = |i: &Instruction| matches!(i,Instruction::DrawCards {recipient:None,count,..} if *count==Quantity::from(1));
    match p.instructions.as_slice() {
        [i] => mill(i),
        [a, b] => (mill(a) && draw(b)) || (draw(a) && mill(b)),
        _ => false,
    }
}
pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_mill(&a.program))
        || d.triggered.iter().any(|a| has_mill(&a.program))
    {
        return Err("mill requires an ordinary spell source".into());
    }
    if !d.spell.as_ref().is_some_and(has_mill) {
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
        return Err("mill requires a complete ordinary spell definition".into());
    }
    Ok(())
}
pub(crate) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let root = json!({"properties":{"optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"}}});
    let draw = json!({"properties":{"op":{"const":"draw_cards"},"count":{"const":"1"},"recipient":{"type":"null"}}});
    let mill = |recipient: serde_json::Value| json!({"required":["op","count","source_block"],"properties":{"op":{"const":"mill_cards"},"count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"recipient":recipient}});
    let composition = |m: serde_json::Value| json!({"oneOf":[{"minItems":1,"maxItems":1,"items":m},{"minItems":2,"maxItems":2,"prefixItems":[m,draw],"items":false},{"minItems":2,"maxItems":2,"prefixItems":[draw,m],"items":false}]});
    let own = mill(json!({"type":"null"}));
    let mut targeted = mill(json!({"const":LifeRecipient::Target {target_slot:0}}));
    targeted["required"]
        .as_array_mut()
        .unwrap()
        .push(json!("recipient"));
    schema["$defs"]["FixedMillProgram"] = json!({"allOf":[root,{"oneOf":[
     {"properties":{"targets":{"maxItems":0},"instructions":composition(own)}},
     {"properties":{"targets":{"minItems":1,"maxItems":1,"items":{"properties":{"slot":{"const":0},"selection_id":{"minLength":1},"domain":{"const":"any_player"},"chooser":{"const":"controller"},"stage":{"const":"announcement"},"filter":{"type":"null"}}}},"instructions":composition(targeted)}}
    ]}]});
    schema["$defs"]["MillFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"mill_cards"}}}}},"modes":{"items":{"properties":{"program":{"$ref":"#/$defs/MillFreeProgram"}}}}}});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"not":{"$ref":"#/$defs/MillFreeProgram"}},"then":{"$ref":"#/$defs/FixedMillProgram"}}));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":{"$ref":"#/$defs/MillFreeProgram"}}}});
    let def = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    def.push(json!({"properties":{"activated":{"items":{"properties":{"program":{"$ref":"#/$defs/MillFreeProgram"}}}},"triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/MillFreeProgram"}}}}}}));
    def.push(json!({"if":has,"then":{"properties":{
  "spell":{"$ref":"#/$defs/FixedMillProgram"},"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
  "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}}},
  "capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::CastInstantV1,Capability::CastSorceryV1,Capability::FixedMillProgramV1,Capability::AnyPlayerTargetV1,Capability::ControllerDrawV1]},"allOf":[{"contains":{"const":Capability::NormalLayoutV1}},{"contains":{"const":Capability::FixedMillProgramV1}}]}
 }},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::FixedMillProgramV1}}}}}}));
    def.push(json!({"if":has,"then":{"allOf":[
  {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},"then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"contains":{"const":Capability::CastInstantV1},"not":{"contains":{"const":Capability::CastSorceryV1}}}}},"else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"contains":{"const":Capability::CastSorceryV1},"not":{"contains":{"const":Capability::CastInstantV1}}}}}},
  {"if":{"properties":{"spell":{"properties":{"targets":{"minItems":1}}}}},"then":{"properties":{"capabilities":{"contains":{"const":Capability::AnyPlayerTargetV1}}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::AnyPlayerTargetV1}}}}}},
  {"if":{"properties":{"spell":{"properties":{"instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"draw_cards"}}}}}}}},"then":{"properties":{"capabilities":{"contains":{"const":Capability::ControllerDrawV1}}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerDrawV1}}}}}}
 ]}}));
}
#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = crate::controller_library::witness();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::MillCards {
        recipient: None,
        count: 2.into(),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_mill_closed_parameterized_programs_and_exact_requirements() {
        for count in ["0", "2", "999999999999999999999999"] {
            for targeted in [false, true] {
                for order in 0..3 {
                    let mut d = witness();
                    let p = d.spell.as_mut().unwrap();
                    if targeted {
                        p.targets = vec![TargetSpec {
                            slot: 0,
                            selection_id: "mill-target".into(),
                            chooser: Chooser::Controller,
                            stage: EvaluationStage::Announcement,
                            domain: TargetDomain::AnyPlayer,
                            filter: None,
                        }];
                    }
                    p.instructions = vec![Instruction::MillCards {
                        recipient: targeted.then_some(LifeRecipient::Target { target_slot: 0 }),
                        count: count.parse().unwrap(),
                        source_block: 0,
                    }];
                    let draw = Instruction::DrawCards {
                        recipient: None,
                        count: 1.into(),
                        source_block: 1,
                    };
                    if order == 1 {
                        p.instructions.push(draw);
                    } else if order == 2 {
                        p.instructions.insert(0, draw);
                    }
                    assert!(fixed_mill_program(p));
                    d.capabilities = d.requirements();
                    d.validate().unwrap();
                    for cap in d.capabilities.clone() {
                        let mut bad = d.clone();
                        bad.capabilities.remove(&cap);
                        assert!(bad.validate().is_err());
                    }
                    let mut bad = d.clone();
                    bad.capabilities.insert(Capability::ControllerScryV1);
                    assert!(bad.validate().is_err());
                    for variant in 0..6 {
                        let mut bad = d.clone();
                        let p = bad.spell.as_mut().unwrap();
                        let i = p
                            .instructions
                            .iter_mut()
                            .find(|i| matches!(i, Instruction::MillCards { .. }))
                            .unwrap();
                        match variant {
                            0 => {
                                if let Instruction::MillCards { count, .. } = i {
                                    *count = (-1).into()
                                }
                            }
                            1 => {
                                if let Instruction::MillCards { recipient, .. } = i {
                                    *recipient = Some(LifeRecipient::EachOpponent)
                                }
                            }
                            2 => p.instructions.push(Instruction::Scry {
                                amount: 1.into(),
                                source_block: 0,
                            }),
                            3 => p.instructions.push(p.instructions[0].clone()),
                            4 => p.resolution = SpellResolution::Permanent,
                            _ => p.functioning_zone = FunctioningZone::Battlefield,
                        }
                        bad.capabilities = bad.requirements();
                        assert!(bad.validate().is_err(), "variant{variant}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod ramos_combined_cost_fences {
    use super::*;
    #[test]
    fn mill_combined_costs_and_conditional_draw_reject() {
        for targeted in [false, true] {
            let mut d = witness();
            let p = d.spell.as_mut().unwrap();
            if targeted {
                p.targets = vec![TargetSpec {
                    slot: 0,
                    selection_id: "mill-player".into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    domain: TargetDomain::AnyPlayer,
                    filter: None,
                }];
                let Instruction::MillCards { recipient, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *recipient = Some(LifeRecipient::Target { target_slot: 0 });
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            let identity = format!(
                "card:{}/faces/0/abilities/0/keywords/0#keyword",
                d.oracle_id
            );
            let p = d.spell.as_mut().unwrap();
            p.optional_additional_costs
                .push(OptionalAdditionalManaCost {
                    identity: identity.clone(),
                    source_block: 0,
                    mana: ManaCost {
                        generic: 1.into(),
                        colored: vec![],
                    },
                });
            assert!(!fixed_mill_program(p));
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
            let p = d.spell.as_mut().unwrap();
            p.optional_additional_costs.clear();
            p.instructions.push(Instruction::DrawCardsIfAdditionalCost {
                cost_identity: identity,
                count: 1.into(),
                source_block: 0,
            });
            assert!(!fixed_mill_program(p));
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
        }
    }
}
