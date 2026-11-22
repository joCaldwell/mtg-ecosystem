//! Complete fixed-mana kicker/removal/draw family. CR 118.8, 601.2b/f,
//! 702.33a/d/e and 607.2, pinned 2026-09-25.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn validate_program(p: &SpellProgram) -> Result<(), String> {
    let conditional = p
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::DrawCardsIfAdditionalCost { .. }));
    if p.optional_additional_costs.is_empty() {
        return if conditional {
            Err("conditional draw requires its optional cost declaration".into())
        } else {
            Ok(())
        };
    }
    let [cost] = p.optional_additional_costs.as_slice() else {
        return Err("multiple optional costs require separate complete source support".into());
    };
    if cost.identity.is_empty()
        || cost.mana.generic.is_negative()
        || p.additional_sacrifice.is_some()
        || !p.modes.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || !matches!(p.targets.as_slice(), [t] if t.slot == 0 && t.domain == TargetDomain::BattlefieldPermanent)
        || !matches!(p.instructions.as_slice(), [kernel!(Destroy { what: target_object!(0), regeneration_forbidden: _ }),
            Instruction::DrawCardsIfAdditionalCost { cost_identity, count, .. }]
            if *cost_identity == cost.identity && count > &Quantity::zero())
    {
        return Err("unsupported complete optional fixed-mana cost/linked draw program".into());
    }
    Ok(())
}
pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    // A declaration belongs to the spell, never a trigger, mode or activation.
    if d.triggered.iter().any(|a| {
        !a.program.optional_additional_costs.is_empty()
            || a.program
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::DrawCardsIfAdditionalCost { .. }))
    }) || d.activated.iter().any(|a| {
        !a.program.optional_additional_costs.is_empty()
            || a.program
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::DrawCardsIfAdditionalCost { .. }))
    }) {
        return Err("optional casting costs require an ordinary spell declaration".into());
    }
    if let Some(p) = &d.spell {
        if p.modes
            .iter()
            .any(|m| !m.program.optional_additional_costs.is_empty())
        {
            return Err("optional costs on modes are unsupported".into());
        }
        for cost in &p.optional_additional_costs {
            if !matches!(
                d.characteristics.kind,
                CardKind::Instant | CardKind::Sorcery
            ) || d.x_mana_cost
                || !d.static_abilities.is_empty()
                || cost.identity
                    != format!(
                        "card:{}/faces/0/abilities/{}/keywords/0#keyword",
                        d.oracle_id, cost.source_block
                    )
            {
                return Err("optional cost identity must name this source's exact keyword".into());
            }
        }
    }
    Ok(())
}
pub(super) fn schema(schema: &mut serde_json::Value) {
    // Schema constrains the identity syntax. Definition validation checks the
    // relationship between the enclosing source, its declaration, and the draw.
    use serde_json::json;
    let conditional =
        json!({"required":["op"],"properties":{"op":{"const":"draw_cards_if_additional_cost"}}});
    let costs = json!({"required":["optional_additional_costs"],"properties":{"optional_additional_costs":{"minItems":1}}});
    schema["$defs"]["OptionalAdditionalManaCost"]["properties"]["identity"]["pattern"] =
        json!("^card:[0-9a-f-]+/faces/0/abilities/[0-9]+/keywords/0#keyword$");
    schema["$defs"]["OptionalAdditionalManaCost"]["properties"]["mana"]["properties"]["generic"]
        ["pattern"] = json!("^(0|[1-9][0-9]*)$");
    schema["$defs"]["SpellProgram"].as_object_mut().unwrap()
        .entry("allOf").or_insert_with(|| json!([])).as_array_mut().unwrap().push(json!({
        "if":costs,
        "then":{"properties":{
            "optional_additional_costs":{"minItems":1,"maxItems":1},
            "additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "targets":{"minItems":1,"maxItems":1,"items":{"properties":{"slot":{"const":0},"domain":{"const":"battlefield_permanent"}}}},
            "instructions":{"minItems":2,"maxItems":2,"prefixItems":[
                instruction_schema::target_destroy(Some(0), None),
                {"properties":{"op":{"const":"draw_cards_if_additional_cost"},"count":{"pattern":"^[1-9][0-9]*$"},"cost_identity":{"pattern":"^card:[0-9a-f-]+/faces/0/abilities/[0-9]+/keywords/0#keyword$"}}}
            ]}
        }},
        "else":{"properties":{"instructions":{"items":{"not":conditional}}}}
    }));
    schema["$defs"]["OptionalCostFreeProgram"] = json!({"properties":{
        "optional_additional_costs":{"maxItems":0},"instructions":{"items":{"not":conditional}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/OptionalCostFreeProgram"}}}}
    }});
    let free = json!({"$ref":"#/$defs/OptionalCostFreeProgram"});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({"properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}},"spell":{"properties":{"modes":{"items":{"properties":{"program":free}}}}}}}),
        json!({"if":{"required":["spell"],"properties":{"spell":{"type":"object","allOf":[costs]}}},
            "then":{"properties":{"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"x_mana_cost":{"const":false},"static_abilities":{"maxItems":0},"capabilities":{"contains":{"const":Capability::OptionalAdditionalManaCostV1}}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::OptionalAdditionalManaCostV1}}}}}})
    ]);
}
