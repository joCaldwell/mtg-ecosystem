//! CR 121.2, 601.2c and 608.2b-c: ordinary fixed draws for one announced player.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(crate) fn has_counted_draw(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::DrawCards {count, ..} if *count != Quantity::from(1)))
        || p.modes.iter().any(|m| has_counted_draw(&m.program))
}

/// A standalone, unqualified target-player draw with an executable count other than one.
pub fn targeted_counted_draw_program(p: &SpellProgram) -> bool {
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
        && matches!(p.instructions.as_slice(), [Instruction::DrawCards {
            recipient: Some(LifeRecipient::Target { target_slot: 0 }),
            count,
            source_block: 0,
        }] if !count.is_negative() && *count != Quantity::from(1))
}

pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_counted_draw(&a.program))
        || d.triggered.iter().any(|a| has_counted_draw(&a.program))
    {
        return Err("targeted counted draw requires an ordinary spell source".into());
    }
    if !d.spell.as_ref().is_some_and(has_counted_draw) {
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
        return Err("targeted counted draw requires a complete ordinary spell definition".into());
    }
    Ok(())
}

pub(crate) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let capability = Capability::TargetedCountedDrawV1;
    let family = json!({"type":"object","properties":{
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},
        "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
        "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
            "slot":{"const":0},"selection_id":{"minLength":1},"domain":{"const":"any_player"},
            "chooser":{"const":"controller"},"stage":{"const":"announcement"},"filter":{"type":"null"}
        }}},
        "instructions":{"minItems":1,"maxItems":1,"items":{
            "required":["op","recipient","count","source_block"],"properties":{
                "op":{"const":"draw_cards"},"recipient":{"const":LifeRecipient::Target {target_slot:0}},
                "count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$","not":{"const":"1"}},
                "source_block":{"const":0}
            }
        }}
    }});
    let free = json!({"$ref":"#/$defs/TargetedCountedDrawFreeProgram"});
    schema["$defs"]["TargetedCountedDrawFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["op","count"],"properties":{
            "op":{"const":"draw_cards"},"count":{"not":{"const":"1"}}
        }}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"if":{"not":free},"then":family}));
    let has_draw =
        json!({"required":["spell"],"properties":{"spell":{"type":"object","not":free}}});
    let timing_and_caps = |timing, cast| {
        json!({"properties":{
            "spell":{"properties":{"timing":{"const":timing}}},
            "capabilities":{"maxItems":4,"items":{"enum":[
                Capability::NormalLayoutV1,Capability::AnyPlayerTargetV1,capability,cast
            ]},"allOf":[
                {"contains":{"const":Capability::NormalLayoutV1}},
                {"contains":{"const":Capability::AnyPlayerTargetV1}},
                {"contains":{"const":capability}},
                {"contains":{"const":cast}}
            ]}
        }})
    };
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({"properties":{
            "activated":{"items":{"properties":{"program":free}}},
            "triggered":{"items":{"properties":{"program":free}}}
        }}),
        json!({"if":has_draw,"then":{"allOf":[
            {"properties":{
                "spell":family,"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
                "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},
                "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
                "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},
                    "additional_types":{"maxItems":0},"subtypes":{"maxItems":0},
                    "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                    "power":{"type":"null"},"toughness":{"type":"null"}
                }}
            }},
            {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
             "then":timing_and_caps(SpellTiming::Instant, Capability::CastInstantV1),
             "else":timing_and_caps(SpellTiming::MainPhase, Capability::CastSorceryV1)}
        ]},"else":{"properties":{"capabilities":{"not":{"contains":{"const":capability}}}}}}),
    ]);
}

#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = crate::controller_library::witness();
    let p = d.spell.as_mut().unwrap();
    p.targets = vec![TargetSpec {
        slot: 0,
        selection_id: "line:0:0".into(),
        domain: TargetDomain::AnyPlayer,
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        filter: None,
    }];
    p.instructions = vec![Instruction::DrawCards {
        recipient: Some(LifeRecipient::Target { target_slot: 0 }),
        count: 4.into(),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn targeted_counted_draw_requires_nonnegative_fixed_count_and_exact_capabilities() {
    for count in [-1, 0, 1, 2, 4] {
        let mut d = witness();
        let p = d.spell.as_mut().unwrap();
        let Instruction::DrawCards { count: n, .. } = &mut p.instructions[0] else {
            unreachable!()
        };
        *n = count.into();
        assert_eq!(targeted_counted_draw_program(p), count >= 0 && count != 1);
        d.capabilities = d.requirements();
        assert_eq!(d.validate().is_ok(), count >= 0);
        d.capabilities.insert(if count == 1 {
            Capability::TargetedCountedDrawV1
        } else {
            Capability::TargetPlayerDrawV1
        });
        assert!(d.validate().is_err());
    }
    let mut mixed = witness();
    mixed
        .spell
        .as_mut()
        .unwrap()
        .instructions
        .push(Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 0,
        });
    mixed.capabilities = mixed.requirements();
    assert!(mixed.validate().is_err());
}
