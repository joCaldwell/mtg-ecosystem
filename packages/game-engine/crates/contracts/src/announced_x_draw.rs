//! CR 107.3, 601.2b-c, 608.2b-c and 121.2: draw and life changes use the spell's announced X.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

// Unit variants in externally tagged enums also accept {"controller":null}
// through Serde. Limit the new wire fields to the canonical recipient forms.
pub(super) fn deserialize_recipient<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<LifeRecipient, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged, deny_unknown_fields)]
    enum Recipient {
        Name(String),
        Target { target: Slot },
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Slot {
        target_slot: u32,
    }
    match Recipient::deserialize(deserializer)? {
        Recipient::Name(name) if name == "controller" => Ok(LifeRecipient::Controller),
        Recipient::Target {
            target: Slot { target_slot: 0 },
        } => Ok(LifeRecipient::Target { target_slot: 0 }),
        _ => Err(serde::de::Error::custom(
            "expected controller or player target slot zero",
        )),
    }
}

fn has_x_instruction(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::DrawAnnouncedX { .. } | Instruction::ChangeLifeAnnouncedX { .. }
        )
    }) || p.modes.iter().any(|m| has_x_instruction(&m.program))
}

#[derive(Clone, Copy)]
enum Shape {
    Draw,
    TargetDraw,
    TargetDrawLose,
    GainDraw,
    DrawDiscard,
}

impl Shape {
    const ALL: [Self; 5] = [
        Self::Draw,
        Self::TargetDraw,
        Self::TargetDrawLose,
        Self::GainDraw,
        Self::DrawDiscard,
    ];

    fn targeted(self) -> bool {
        matches!(self, Self::TargetDraw | Self::TargetDrawLose)
    }

    fn instructions(self) -> Vec<Instruction> {
        let recipient = if self.targeted() {
            LifeRecipient::Target { target_slot: 0 }
        } else {
            LifeRecipient::Controller
        };
        let draw = Instruction::DrawAnnouncedX {
            recipient: recipient.clone(),
            source_block: 0,
        };
        let life = |gain| Instruction::ChangeLifeAnnouncedX {
            recipient,
            gain,
            source_block: 0,
        };
        match self {
            Self::Draw | Self::TargetDraw => vec![draw],
            Self::TargetDrawLose => vec![draw, life(false)],
            Self::GainDraw => vec![life(true), draw],
            Self::DrawDiscard => vec![
                draw,
                Instruction::DiscardCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 0,
                },
            ],
        }
    }

    fn matches(self, p: &SpellProgram) -> bool {
        let targets_match = if self.targeted() {
            matches!(p.targets.as_slice(), [t] if t.slot == 0 && !t.selection_id.is_empty() && t.domain == TargetDomain::AnyPlayer && t.stage == EvaluationStage::Announcement && t.chooser == Chooser::Controller && t.filter.is_none())
        } else {
            p.targets.is_empty()
        };
        let mut instructions = self.instructions();
        if let Some(Instruction::DiscardCards {
            recipient: Some(ActionRecipient::Controller),
            ..
        }) = p.instructions.last()
            && let Some(Instruction::DiscardCards { recipient, .. }) = instructions.last_mut()
        {
            *recipient = Some(ActionRecipient::Controller);
        }
        targets_match && p.instructions == instructions
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_x_instruction(&a.program))
        || d.triggered.iter().any(|a| has_x_instruction(&a.program))
    {
        return Err("announced X draw requires a spell source".into());
    }
    let Some(p) = d.spell.as_ref().filter(|p| has_x_instruction(p)) else {
        return Ok(());
    };
    let c = &d.characteristics;
    if !d.x_mana_cost
        || !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !Shape::ALL.iter().any(|shape| shape.matches(p))
    {
        return Err("unsupported announced X spell definition".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let free = json!({"$ref":"#/$defs/AnnouncedXDrawFreeProgram"});
    schema["$defs"]["AnnouncedXDrawFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"enum":["draw_announced_x","change_life_announced_x"]}}}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["Instruction"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"properties":{"op":{"enum":["draw_announced_x","change_life_announced_x"]}}},
        "then":{"properties":{"source_block":{"const":0},"recipient":{"enum":[LifeRecipient::Controller,LifeRecipient::Target{target_slot:0}]}}}
    }));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":free}}});
    let mut alternatives = Vec::new();
    let mut programs = Vec::new();
    for shape in Shape::ALL {
        let targets = if shape.targeted() {
            json!({"minItems":1,"maxItems":1,"items":{"properties":{
                "slot":{"const":0},"selection_id":{"minLength":1},"domain":{"const":"any_player"},
                "stage":{"const":"announcement"},"chooser":{"const":"controller"},"filter":{"type":"null"}
            }}})
        } else {
            json!({"maxItems":0})
        };
        let instructions: Vec<_> = shape
            .instructions()
            .into_iter()
            .map(|i| match i {
                Instruction::DiscardCards { .. } => json!({"properties":{
                    "op":{"const":"discard_cards"},"count":{"const":"1"},"source_block":{"const":0},
                    "recipient":{"enum":[null,"controller"]}
                }}),
                i => json!({"const":i}),
            })
            .collect();
        let n = instructions.len();
        let program = json!({"properties":{"targets":targets,"instructions":{
            "minItems":n,"maxItems":n,"prefixItems":instructions
        },"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"}}});
        programs.push(program);
        for (kind, timing, cast) in [
            (
                CardKind::Instant,
                SpellTiming::Instant,
                Capability::CastInstantV1,
            ),
            (
                CardKind::Sorcery,
                SpellTiming::MainPhase,
                Capability::CastSorceryV1,
            ),
        ] {
            let mut caps = vec![
                Capability::AnnouncedXDrawV1,
                Capability::NormalLayoutV1,
                cast,
            ];
            if shape.targeted() {
                caps.push(Capability::AnyPlayerTargetV1);
            }
            match shape {
                Shape::TargetDrawLose => caps.push(Capability::LifeLossV1),
                Shape::GainDraw => caps.push(Capability::LifeGainV1),
                Shape::DrawDiscard => caps.push(Capability::ControllerDiscardV1),
                _ => {}
            }
            let required: Vec<_> = caps
                .iter()
                .map(|cap| json!({"contains":{"const":cap}}))
                .collect();
            alternatives.push(json!({"properties":{
                "capabilities":{"maxItems":caps.len(),"allOf":required},
                "characteristics":{"properties":{"kind":{"const":kind}}},
                "spell":{"properties":{"timing":{"const":timing},"targets":targets,"instructions":{
                    "minItems":n,"maxItems":n,"prefixItems":instructions
                }}}
            }}));
        }
    }
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"if":{"not":free},"then":{"anyOf":programs}}));
    let defs = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    defs.push(json!({"properties":{
        "activated":{"items":{"properties":{"program":free}}},
        "triggered":{"items":{"properties":{"program":free}}}
    }}));
    defs.push(json!({"if":has,"then":{"required":["x_mana_cost"],"properties":{
        "x_mana_cost":{"const":true},"intrinsic_zone":{"type":"null"},
        "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
        "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},
        "characteristics":{"properties":{"additional_types":{"maxItems":0},"subtypes":{"maxItems":0},
            "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
            "power":{"type":"null"},"toughness":{"type":"null"},
            "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
        }},
        "spell":{"properties":{"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"}}}
    },"anyOf":alternatives},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::AnnouncedXDrawV1}}}}}}));
    let wipe = json!({"required":["spell"],"properties":{"spell":{"type":"object","properties":{"instructions":{"contains":{
        "required":["op","amount"],"properties":{"op":{"const":"damage_set"},"amount":{"const":{"amount":"announced_x"}}}
    }}}}}});
    defs.push(json!({"if":{"anyOf":[has,wipe]},"then":{"required":["x_mana_cost"],"properties":{"x_mana_cost":{"const":true}}},"else":{"properties":{"x_mana_cost":{"const":false}}}}));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = targeted_counted_draw::witness();
    d.x_mana_cost = true;
    d.spell.as_mut().unwrap().instructions = Shape::TargetDrawLose.instructions();
    d.capabilities = d.requirements();
    d
}

#[test]
fn announced_x_draw_requires_exact_binding_and_sequence() {
    let base = witness();
    let mut malformed = serde_json::to_value(Shape::Draw.instructions().remove(0)).unwrap();
    malformed["recipient"] = serde_json::json!({"controller":null});
    assert!(serde_json::from_value::<Instruction>(malformed).is_err());
    let mut wrong_target = base.clone();
    wrong_target.spell.as_mut().unwrap().targets[0]
        .selection_id
        .clear();
    assert!(wrong_target.validate().is_err());
    let mut wrong_cap = base.clone();
    wrong_cap.capabilities.remove(&Capability::LifeLossV1);
    assert!(wrong_cap.validate().is_err());
    for shape in Shape::ALL {
        let mut d = base.clone();
        let p = d.spell.as_mut().unwrap();
        if !shape.targeted() {
            p.targets.clear();
        }
        p.instructions = shape.instructions();
        d.capabilities = d.requirements();
        assert!(d.validate().is_ok(), "{:?}", d.validate());
        assert!(!d.capabilities.contains(&Capability::BoardWipesV1));
        d.x_mana_cost = false;
        assert!(d.validate().is_err());
        d.x_mana_cost = true;
        d.spell
            .as_mut()
            .unwrap()
            .instructions
            .push(Instruction::OptionalDrawCards { source_block: 0 });
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
}
