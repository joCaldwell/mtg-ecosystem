//! A single targetless controller-set status change under CR 702.26.
use super::*;
use serde_json::{Value, json};

fn contains(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::ExileResolvingSpell { .. }
                | Instruction::ProtectControllerUntilNextTurn { .. }
                | Instruction::PhaseOutControllerPermanents { .. }
                | Instruction::FreezeControllerLifeUntilNextTurn { .. }
        )
    }) || p.modes.iter().any(|m| contains(&m.program))
}

fn controller_window_program() -> Vec<Instruction> {
    vec![
        Instruction::FreezeControllerLifeUntilNextTurn { source_block: 0 },
        Instruction::ProtectControllerUntilNextTurn { source_block: 0 },
        Instruction::PhaseOutControllerPermanents { source_block: 0 },
        Instruction::ExileResolvingSpell { source_block: 1 },
    ]
}

const WINDOW: &str =
    "Until your next turn, your life total can't change and you gain protection from everything.";
const PHASE: &str = "All permanents you control phase out.";

fn controller_window_source(name: &str, text: &str, ast: &Value) -> Option<Vec<String>> {
    let (status, terminal) = text.split_once('\n')?;
    if terminal != format!("Exile {name}.") {
        return None;
    }
    let line = format!("{WINDOW} {PHASE}");
    let suffix = status.strip_prefix(&line)?;
    // CR 207.2a: a single terminal parenthetical on the phasing sentence is reminder text.
    if !suffix.is_empty()
        && !suffix
            .strip_prefix(" (")
            .and_then(|s| s.strip_suffix(')'))
            .is_some_and(|s| !s.is_empty() && !s.contains(['(', ')']))
    {
        return None;
    }
    let rules=["cant-gain-life","cant-lose-life"].map(|rule|json!({"effect":"player-rule","rule":{"who":{"player":"you"},"rule":rule},"duration":{"duration":"your-next-turn"}}));
    let expected = json!([{"kind":"spell","effects":[{"steps":[{"effect":"simultaneous","steps":rules},{"effect":"player-gain-abilities","who":{"player":"you"},"abilities":[{"gained":"keyword","keyword":{"keyword":"protection","from":[{"scope":"everything"}]}}],"duration":{"duration":"your-next-turn"}}]},{"steps":[{"effect":"phase-out","what":{"ref":"all","filter":{"cls":"permanent","control":"you"}}}]}]},{"kind":"spell","effects":[{"steps":[{"effect":"exile","what":{"ref":"self"}}]}]}]);
    (ast == &expected).then(|| vec![line, "Exile ~.".into()])
}

pub fn controller_window_instructions(
    name: &str,
    text: &str,
    ast: &Value,
) -> Option<Vec<Instruction>> {
    controller_window_source(name, text, ast).map(|_| controller_window_program())
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let fail = || "phasing requires the complete ordinary controller spell".to_string();
    if d.activated.iter().any(|a|contains(&a.program)) || d.triggered.iter().any(|a|contains(&a.program))
        || d.static_abilities.iter().any(|a|matches!(&a.effect,StaticEffect::GrantSpellCascade{program,..} if contains(program))) {return Err(fail());}
    let uses = d.spell.as_ref().is_some_and(contains);
    if !uses
        && !d.provenance.abilities_json.contains("phase-out")
        && !d.provenance.source_json.contains("phase out")
        && !d.provenance.source_json.contains("life total can")
        && !d
            .provenance
            .source_json
            .contains("protection from everything")
        && !d.provenance.abilities_json.contains("\"self\"")
        && !d.provenance.abilities_json.contains("everything")
        && !d.provenance.abilities_json.contains("cant-lose-life")
        && !d.provenance.source_json.contains("\\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let freeze = "Until your next turn, your life total can't change.";
    let protection = "Until your next turn, you gain protection from everything.";
    let phase = "All permanents you control phase out.";
    let self_exile = format!("Exile {}.", d.name);
    let exile_ast =
        json!([{"kind":"spell","effects":[{"steps":[{"effect":"exile","what":{"ref":"self"}}]}]}]);
    let (clause, expected, instruction) = if text == self_exile {
        (
            self_exile.as_str(),
            exile_ast.clone(),
            Instruction::ExileResolvingSpell { source_block: 0 },
        )
    } else if text == freeze {
        let rules=["cant-gain-life","cant-lose-life"].map(|rule|json!({"effect":"player-rule","rule":{"who":{"player":"you"},"rule":rule},"duration":{"duration":"your-next-turn"}}));
        (
            freeze,
            json!([{"kind":"spell","effects":[{"steps":[{"effect":"simultaneous","steps":rules}]}]}]),
            Instruction::FreezeControllerLifeUntilNextTurn { source_block: 0 },
        )
    } else if text == protection {
        (
            protection,
            json!([{"kind":"spell","effects":[{"steps":[{"effect":"player-gain-abilities","who":{"player":"you"},"abilities":[{"gained":"keyword","keyword":{"keyword":"protection","from":[{"scope":"everything"}]}}],"duration":{"duration":"your-next-turn"}}]}]}]),
            Instruction::ProtectControllerUntilNextTurn { source_block: 0 },
        )
    } else {
        (
            phase,
            json!([{"kind":"spell","effects":[{"steps":[{"effect":"phase-out","what":{"ref":"all","filter":{"cls":"permanent","control":"you"}}}]}]}]),
            Instruction::PhaseOutControllerPermanents { source_block: 0 },
        )
    };
    if !uses
        && text != self_exile
        && ast != exile_ast
        && !text.contains(phase)
        && !text.contains(freeze)
        && !text.contains(protection)
        && !ast.to_string().contains("\"phase-out\"")
        && !ast.to_string().contains("\"scope\":\"everything\"")
        && !ast.to_string().contains("cant-lose-life")
    {
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
    let window = controller_window_source(&d.name, text, &ast);
    let instructions = if window.is_some() {
        controller_window_program()
    } else {
        vec![instruction]
    };
    if (window.is_none() && (text != clause || ast != expected))
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
        || p.instructions != instructions
        || d.source_labels.as_ref().is_some_and(|l| {
            *l != SourceLabels {
                text: text.into(),
                blocks: window.clone().unwrap_or_else(|| {
                    vec![if clause == self_exile {
                        "Exile ~.".into()
                    } else {
                        clause.into()
                    }]
                }),
                ..Default::default()
            }
        })
    {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn schema(s: &mut Value) {
    for (name, capability, free_name, pattern) in [
        (
            "phase_out_controller_permanents",
            Capability::ControlledPermanentPhasingV1,
            "ControllerPhaseFree",
            "All permanents you control phase out",
        ),
        (
            "freeze_controller_life_until_next_turn",
            Capability::ControllerLifeFreezeV1,
            "ControllerLifeFreezeFree",
            "Until your next turn, your life total can't change",
        ),
        (
            "exile_resolving_spell",
            Capability::ExileResolvingSpellV1,
            "ResolvingSelfExileFree",
            "^$",
        ),
        (
            "protect_controller_until_next_turn",
            Capability::ControllerProtectionUntilNextTurnV1,
            "ControllerProtectFree",
            "Until your next turn, you gain protection from everything",
        ),
    ] {
        let op = json!({"required":["op"],"properties":{"op":{"const":name}}});
        let cap = json!({"contains":{"const":capability}});
        let free = json!({"$ref":format!("#/$defs/{free_name}")});
        s["$defs"][free_name] = json!({"properties":{"instructions":{"not":{"contains":op}},"modes":{"items":{"properties":{"program":free}}}}});
        let shape = json!({"required":["spell","capabilities"],"properties":{"capabilities":cap,"spell":{"type":"object","properties":{"instructions":{"minItems":1,"maxItems":1,"items":{"allOf":[op,{"properties":{"source_block":{"const":0}}}]}},"resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},"targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}},"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0}}});
        let mut composed = shape.clone();
        composed["properties"]["spell"]["properties"]["instructions"] =
            json!({"const":controller_window_program()});
        composed["properties"]["capabilities"] = json!({"allOf":([Capability::ControllerLifeFreezeV1,Capability::ControllerProtectionUntilNextTurnV1,Capability::ControlledPermanentPhasingV1,Capability::ExileResolvingSpellV1].map(|c|json!({"contains":{"const":c}})))});
        s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}},"static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":free}}}}}}}},"if":{"anyOf":[{"required":["capabilities"],"properties":{"capabilities":cap}},{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},{"properties":{"provenance":{"properties":{"source_json":{"pattern":pattern}}}}}]},"then":{"anyOf":[shape,composed]}}));
    }
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
