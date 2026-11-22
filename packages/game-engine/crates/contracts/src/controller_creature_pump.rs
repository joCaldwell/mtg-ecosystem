//! CR 109.5, 608.2h, 611.2c and 613.4c: the spell's controller supplies the
//! fixed recipient set once, at resolution, for the rest of the turn.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "Creatures you control get ";

fn printed_stats(text: &str) -> Option<((i64, Value), (i64, Value))> {
    fn signed(text: &str) -> Option<(i64, Value)> {
        let (sign, digits) = match text.as_bytes().first()? {
            b'+' => (1, &text[1..]),
            b'-' => (-1, &text[1..]),
            _ => return None,
        };
        let amount: u32 = digits.parse().ok()?;
        (digits == amount.to_string()).then(|| {
            (
                sign * i64::from(amount),
                json!({"sign":sign,"amount":{"amount":"fixed","value":amount}}),
            )
        })
    }
    let (power, toughness) = text
        .strip_prefix(PREFIX)?
        .strip_suffix(" until end of turn.")?
        .split_once('/')?;
    Some((signed(power)?, signed(toughness)?))
}

pub(super) fn program(p: &SpellProgram) -> bool {
    matches!(
        p.instructions.as_slice(),
        [i] if i.pumped().is_some_and(|(what, _, _, block)| block == 0
            && ObjectSet::ControllerCreatures.denotes(what))
    ) && p.targets.is_empty()
        && p.modes.is_empty()
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
}

fn uses(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        ObjectSet::pumped(i).is_some_and(|what| ObjectSet::ControllerCreatures.denotes(what))
    }) || p.modes.iter().any(|m| uses(&m.program))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let bad = || "requires the complete ordinary controller-creature fixed pump".to_string();
    if d.activated.iter().any(|a| uses(&a.program))
        || d.triggered.iter().any(|a| uses(&a.program))
        || d.static_abilities.iter().any(
            |a| matches!(&a.effect, StaticEffect::GrantSpellCascade { program, .. } if uses(program)),
        )
    {
        return Err(bad());
    }
    let active = d.spell.as_ref().is_some_and(uses);
    if !active && !d.provenance.source_json.contains(PREFIX) {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let Some(((power, printed_power), (toughness, printed_toughness))) =
        source["oracle_text"].as_str().and_then(printed_stats)
    else {
        return if active { Err(bad()) } else { Ok(()) };
    };
    let spell = d.spell.as_ref().filter(|p| program(p)).ok_or_else(bad)?;
    let c = &d.characteristics;
    let type_line = match c.kind {
        CardKind::Instant if spell.timing == SpellTiming::Instant => "Instant",
        CardKind::Sorcery if spell.timing == SpellTiming::MainPhase => "Sorcery",
        _ => return Err(bad()),
    };
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let expected = json!([{"kind":"spell","effects":[{"steps":[{
        "effect":"pump","what":{"ref":"each","filter":{"types":["creature"],"control":"you"}},
        "power":printed_power,"toughness":printed_toughness,"duration":{"duration":"end-of-turn"}
    }]}]}]);
    if ast != expected
        || spell.instructions
            != [Instruction::pump(
                ObjectSet::ControllerCreatures.kernel(kir::Context::Spell),
                Quantity::from(power),
                Quantity::from(toughness),
                0,
            )]
        || d.x_mana_cost
        || c.basic
        || c.snow
        || c.legendary
        || !c.subtypes.is_empty()
        || !c.hybrid_mana.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || source["layout"] != "normal"
        || source["type_line"] != type_line
        || source["id"] != d.provenance.printing_id
        || source["oracle_id"] != d.oracle_id
        || source["name"] != d.name
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.keywords.is_empty()
    {
        return Err(bad());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) -> Value {
    let op = instruction_schema::any_pump(json!({"anyOf":[
        {"const":ObjectSet::ControllerCreatures.kernel(Context::Spell)},
        {"const":ObjectSet::ControllerCreatures.kernel(Context::Activated)}
    ]}));
    let has_pump = json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":op}}});
    let free = json!({"$ref":"#/$defs/ControllerCreaturePumpFree"});
    schema["$defs"]["ControllerCreaturePumpFree"] = json!({
        "not":has_pump,
        "properties":{"modes":{"items":{"properties":{"program":free}}}}
    });
    let root = json!({"$ref":"#/$defs/ControllerCreaturePumpProgram"});
    schema["$defs"]["ControllerCreaturePumpProgram"] = json!({
        "type":"object","required":["instructions"],
        "properties":{
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "targets":{"maxItems":0},"modes":{"maxItems":0},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "instructions":{"minItems":1,"maxItems":1,"items":{"allOf":[
                op,{"properties":{"source_block":{"const":0}}}
            ]}}
        }
    });
    let nested = json!({"items":{"properties":{"program":free}}});
    let ordinary = json!({
        "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
        "x_mana_cost":{"const":false},
        "capabilities":{"contains":{"const":Capability::BoardWipesV1}},
        "activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},
        "mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},
        "provenance":{"properties":{"source_json":{"pattern":
            r#""oracle_text"\s*:\s*"Creatures you control get [+-][0-9]+/[+-][0-9]+ until end of turn\.""#
        }}}
    });
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{
                "spell":{"anyOf":[free,root]},"activated":nested,"triggered":nested,
                "static_abilities":{"items":{"properties":{"effect":{"properties":{
                    "grant_spell_cascade":{"properties":{"program":free}}
                }}}}}
            },
            "if":{"required":["spell"],"properties":{"spell":{"type":"object","allOf":[has_pump]}}},
            "then":{"properties":ordinary}
        }));
    root
}
