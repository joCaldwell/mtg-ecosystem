//! CR 603.3d, 603.5 and 608.2b–d. The trigger always announces one target.
//! The trigger's controller decides whether to return the target during resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn contains_return(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::ReturnEntryTargetCreature { .. }))
        || program.modes.iter().any(|m| contains_return(&m.program))
}

fn trigger_program(p: &SpellProgram, keyword_blocks: usize) -> bool {
    let [target] = p.targets.as_slice() else {
        return false;
    };
    let [
        Instruction::ReturnEntryTargetCreature {
            target_slot: 0,
            source_block,
            ..
        },
    ] = p.instructions.as_slice()
    else {
        return false;
    };
    (*source_block as usize) <= keyword_blocks
        && p.timing == SpellTiming::Instant
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.modes.is_empty()
        && target.slot == 0
        && !target.selection_id.is_empty()
        && target.chooser == Chooser::Controller
        && target.stage == EvaluationStage::Announcement
        && target.domain == TargetDomain::BattlefieldPermanent
        && target.filter.as_ref().is_some_and(|f| {
            f.types == [CardKind::Creature]
                && f.non_types.is_empty()
                && f.non_colors.is_empty()
                && f.power.is_none()
                && !f.tapped
        })
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if !d
        .spell
        .iter()
        .chain(d.activated.iter().map(|a| &a.program))
        .chain(d.triggered.iter().map(|a| &a.program))
        .any(contains_return)
    {
        return Ok(());
    }
    let err = || "entry return requires one complete self-entry creature trigger".to_string();
    let c = &d.characteristics;
    let [trigger] = d.triggered.as_slice() else {
        return Err(err());
    };
    let cast = d.spell.as_ref().ok_or_else(err)?;
    if c.kind != CardKind::Creature
        || !c.additional_types.is_empty()
        || c.basic
        || c.legendary
        || c.snow
        || c.subtypes
            .iter()
            .any(|s| unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES.contains(&s.as_str()))
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !matches!(d.keywords.as_slice(), [] | [KeywordAbility::Flying])
        || !trigger_program(&trigger.program, d.keywords.len())
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || cast.timing != SpellTiming::MainPhase
        || cast.resolution != SpellResolution::Permanent
        || cast.functioning_zone != FunctioningZone::Stack
        || cast.additional_sacrifice.is_some()
        || !cast.optional_additional_costs.is_empty()
        || !cast.instructions.is_empty()
        || !cast.targets.is_empty()
        || !cast.modes.is_empty()
        || trigger.condition != TriggerCondition::SelfEntersBattlefield
        || trigger.intervening_if.is_some()
        || trigger.death_filter.is_some()
        || trigger.functioning_zone != FunctioningZone::Battlefield
    {
        return Err(err());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let op =
        json!({"required":["op"],"properties":{"op":{"const":"return_entry_target_creature"}}});
    let has_op = json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":op}}});
    let singleton = |item: Value| json!({"minItems":1,"maxItems":1,"items":item});
    // Every mode is itself a SpellProgram, so forbidding the opcode in direct
    // child modes also fences all deeper paths.
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"modes":{"items":{"properties":{"program":{"not":has_op}}}}},
        "if":has_op,"then":{"properties":{
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"timing":{"const":"instant"},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "instructions":singleton(json!({"allOf":[op],"properties":{"target_slot":{"const":0},"source_block":{"maximum":1}}})),
            "targets":singleton(json!({"required":["filter"],"properties":{"slot":{"const":0},"selection_id":{"minLength":1},
                "chooser":{"const":"controller"},"stage":{"const":"announcement"},"domain":{"const":"battlefield_permanent"},
                "filter":{"type":"object","properties":{"types":{"const":["creature"]},"non_types":{"maxItems":0},
                    "non_colors":{"maxItems":0},"tapped":{"const":false}}}}}))
        }}
    }));
    let trigger = json!({"required":["program"],"properties":{"program":has_op}});
    let cap = json!({"contains":{"const":CreatureEntryReturnV1}});
    let caps = |flying: bool| {
        let mut allowed = vec![
            NormalLayoutV1,
            BaseCreatureV1,
            CastCreatureV1,
            TriggeredAbilityV1,
            SelfEntersBattlefieldV1,
            PermanentRemovalV1,
            CreatureEntryReturnV1,
        ];
        if flying {
            allowed.push(FlyingV1);
        }
        json!({"items":{"enum":allowed},"uniqueItems":true,"minItems":allowed.len(),"maxItems":allowed.len()})
    };
    let subtypes = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES)
        .collect::<Vec<_>>();
    let defs = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    defs.push(json!({"properties":{"spell":{"not":has_op},"activated":{"items":{"not":trigger}}}}));
    defs.push(json!({
        "if":{"anyOf":[{"required":["triggered"],"properties":{"triggered":{"contains":trigger}}},
                        {"properties":{"capabilities":cap}}]},
        "then":{"required":["triggered","spell"],"properties":{
            "characteristics":{"required":["power","toughness"],"properties":{"kind":{"const":"creature"},"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"additional_types":{"maxItems":0},"subtypes":{"items":{"not":{"enum":subtypes}}},
                "power":{"type":"string"},"toughness":{"type":"string","pattern":"^[1-9][0-9]*$"}}},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"activated":{"maxItems":0},
            "static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"enum":[[],["flying"]]},
            "spell":{"type":"object","properties":{"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},
                "resolution":{"const":"permanent"},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"instructions":{"maxItems":0},
                "targets":{"maxItems":0},"modes":{"maxItems":0}}},
            "triggered":singleton(json!({"allOf":[trigger],"properties":{"condition":{"const":"self_enters_battlefield"},
                "functioning_zone":{"const":"battlefield"},"death_filter":{"type":"null"},"intervening_if":{"type":"null"}}}))
        },
        "if":{"required":["keywords"],"properties":{"keywords":{"const":["flying"]}}},
        "then":{"properties":{"capabilities":caps(true)}},
        "else":{"properties":{"capabilities":caps(false),"triggered":{"items":{"properties":{"program":{"properties":{
            "instructions":{"items":{"properties":{"source_block":{"const":0}}}}}}}}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = power_entry_draw::witness();
    let trigger = &mut d.triggered[0];
    trigger.condition = TriggerCondition::SelfEntersBattlefield;
    trigger.program.targets = vec![TargetSpec {
        slot: 0,
        selection_id: "line:0:6".into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::BattlefieldPermanent,
        filter: Some(PermanentFilter {
            power: None,
            types: vec![CardKind::Creature],
            non_types: vec![],
            non_colors: vec![],
            opponent: false,
            tapped: false,
        }),
    }];
    trigger.program.instructions = vec![Instruction::ReturnEntryTargetCreature {
        target_slot: 0,
        optional: false,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn creature_entry_return() {
    let base = witness();
    base.validate().unwrap();
    assert!(
        base.capabilities
            .contains(&Capability::CreatureEntryReturnV1)
    );
    assert!(!base.capabilities.contains(&Capability::OwnerHandMoveV1));
    let mut flying = base.clone();
    flying.keywords.push(KeywordAbility::Flying);
    let p = &mut flying.triggered[0].program;
    p.targets[0].filter.as_mut().unwrap().opponent = true;
    p.instructions[0] = Instruction::ReturnEntryTargetCreature {
        target_slot: 0,
        optional: true,
        source_block: 1,
    };
    flying.capabilities = flying.requirements();
    flying.validate().unwrap();

    for mutation in 0..16 {
        let mut d = base.clone();
        let p = &mut d.triggered[0].program;
        match mutation {
            0 => p.targets[0].filter.as_mut().unwrap().tapped = true,
            1 => p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types
                .push(CardKind::Artifact),
            2 => p.targets[0].domain = TargetDomain::BattlefieldCreature,
            3 => p.instructions.push(p.instructions[0].clone()),
            4 => p.targets.clear(),
            5 => p.timing = SpellTiming::MainPhase,
            6 => d.triggered[0].condition = TriggerCondition::SelfDies,
            7 => d.triggered.push(d.triggered[0].clone()),
            8 => d.keywords.push(KeywordAbility::Haste),
            9 => d.characteristics.legendary = true,
            10 => d.characteristics.snow = true,
            11 => d.characteristics.additional_types.push(CardKind::Artifact),
            12 => d.x_mana_cost = true,
            13 => d
                .spell
                .as_mut()
                .unwrap()
                .modes
                .push(SpellMode { program: p.clone() }),
            14 => {
                p.instructions[0] = Instruction::ReturnToOwnerHand {
                    target_slot: 0,
                    source_block: 0,
                }
            }
            _ => {
                d.spell.as_mut().unwrap().additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Creature],
                    subtypes: vec![],
                })
            }
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "mutation {mutation}");
    }
    let mut wrong = base;
    wrong
        .capabilities
        .remove(&Capability::CreatureEntryReturnV1);
    assert!(wrong.validate().is_err());
    wrong.triggered.clear();
    wrong.capabilities = wrong.requirements();
    wrong.capabilities.insert(Capability::CreatureEntryReturnV1);
    assert!(wrong.validate().is_err());
}
