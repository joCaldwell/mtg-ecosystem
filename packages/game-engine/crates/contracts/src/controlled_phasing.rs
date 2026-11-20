//! A single targetless controller-set status change under CR 702.26.
use super::*;
use serde_json::{Value, json};

fn contains(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::PhaseOutControllerPermanents { .. }))
        || p.modes.iter().any(|m| contains(&m.program))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let fail = || "phasing requires the complete ordinary controller spell".to_string();
    if d.activated.iter().any(|a|contains(&a.program)) || d.triggered.iter().any(|a|contains(&a.program))
        || d.static_abilities.iter().any(|a|matches!(&a.effect,StaticEffect::GrantSpellCascade{program,..} if contains(program))) {return Err(fail());}
    let uses = d.spell.as_ref().is_some_and(contains);
    if !uses
        && !d.provenance.abilities_json.contains("phase-out")
        && !d.provenance.source_json.contains("phase out")
        && !d.provenance.source_json.contains("\\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let clause = "All permanents you control phase out.";
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let expected = json!([{"kind":"spell","effects":[{"steps":[{"effect":"phase-out","what":{"ref":"all","filter":{"cls":"permanent","control":"you"}}}]}]}]);
    if !uses && !text.contains(clause) && !ast.to_string().contains("\"phase-out\"") {
        return Ok(());
    }
    let c = &d.characteristics;
    let (kind, timing) = match c.kind {
        CardKind::Sorcery => ("Sorcery", SpellTiming::MainPhase),
        CardKind::Instant => ("Instant", SpellTiming::Instant),
        _ => return Err(fail()),
    };
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }
    let Some(p) = &d.spell else {
        return Err(fail());
    };
    if text != clause
        || ast != expected
        || source["type_line"] != kind
        || source["layout"] != "normal"
        || source["id"] != d.provenance.printing_id
        || source["oracle_id"] != d.oracle_id
        || source["name"] != d.name
        || source["mana_cost"] != cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || c.snow
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || p.resolution != SpellResolution::Instructions
        || p.timing != timing
        || p.functioning_zone != FunctioningZone::Stack
        || !p.targets.is_empty()
        || !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !matches!(
            p.instructions.as_slice(),
            [Instruction::PhaseOutControllerPermanents { source_block: 0 }]
        )
        || d.source_labels.as_ref().is_some_and(|l| {
            *l != SourceLabels {
                text: clause.into(),
                blocks: vec![clause.into()],
                ..Default::default()
            }
        })
    {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn schema(s: &mut Value) {
    let op =
        json!({"required":["op"],"properties":{"op":{"const":"phase_out_controller_permanents"}}});
    let cap = json!({"contains":{"const":Capability::ControlledPermanentPhasingV1}});
    let free = json!({"$ref":"#/$defs/ControllerPhaseFree"});
    s["$defs"]["ControllerPhaseFree"] = json!({"properties":{"instructions":{"not":{"contains":op}},"modes":{"items":{"properties":{"program":free}}}}});
    let shape = json!({"required":["spell","capabilities"],"properties":{"capabilities":cap,"spell":{"type":"object","properties":{"instructions":{"minItems":1,"maxItems":1,"items":{"allOf":[op,{"properties":{"source_block":{"const":0}}}]}},"resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},"targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}},"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0}}});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}},"static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":free}}}}}}}},"if":{"anyOf":[{"required":["capabilities"],"properties":{"capabilities":cap}},{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},{"properties":{"provenance":{"properties":{"source_json":{"pattern":"All permanents you control phase out\\."}}}}}]},"then":shape}));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_phasing_requires_the_full_bound_instruction_and_printed_cost() {
        let catalog = Catalog::read(include_str!(
            "../../../tests/fixtures/synthetic-controlled-phasing-ir.json"
        ))
        .unwrap();
        let spell = &catalog.definitions[0];
        for change in [
            "extra operation",
            "different cost",
            "erased operation",
            "wrong block",
            "wrong capability",
        ] {
            let mut wrong = spell.clone();
            let p = wrong.spell.as_mut().unwrap();
            match change {
                "extra operation" => p
                    .instructions
                    .push(Instruction::PhaseOutControllerPermanents { source_block: 0 }),
                "different cost" => wrong.characteristics.mana_cost.generic = 2.into(),
                "erased operation" => p.instructions.clear(),
                "wrong block" => {
                    p.instructions[0] =
                        Instruction::PhaseOutControllerPermanents { source_block: 1 }
                }
                _ => {}
            }
            wrong.capabilities = wrong.requirements();
            if change == "wrong capability" {
                wrong
                    .capabilities
                    .remove(&Capability::ControlledPermanentPhasingV1);
            }
            assert!(wrong.validate().is_err(), "{change}");
        }
    }
}
