//! CR 400.7j, 608.2c–g, 609.3, 701.20, 701.24, 702.144a and 707.10.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const SCAN: &str =
    "Shuffle your library, then reveal cards from the top of it until you reveal a nonland card.";
const SPELL_TEXT: &str = concat!(
    "Shuffle your library, then reveal cards from the top of it until you reveal a nonland card. ",
    "Exile that card and put the rest on the bottom of your library in a random order. ",
    "You may cast the exiled card without paying its mana cost."
);
const TEXT: &str = concat!(
    "Demonstrate (When you cast this spell, you may copy it. If you do, choose an opponent to also copy it.)\n",
    "Shuffle your library, then reveal cards from the top of it until you reveal a nonland card. ",
    "Exile that card and put the rest on the bottom of your library in a random order. ",
    "You may cast the exiled card without paying its mana cost."
);
const INVALID: &str = "requires the complete targetless demonstrate and reveal-and-cast sorcery";
const CAPS: [Capability; 8] = [
    Capability::NormalLayoutV1,
    Capability::CastSorceryV1,
    Capability::TriggeredAbilityV1,
    Capability::CastTriggerV1,
    Capability::ResolutionCastV1,
    Capability::RandomBottomV1,
    Capability::RevealAndCastV1,
    Capability::DemonstrateV1,
];

fn abilities() -> Value {
    let you = json!({"player":"you"});
    let library = json!({"zone":"library","owner":"your"});
    let card = json!({"cls":"card"});
    json!([
        {"kind":"keywords","keywords":[{"keyword":"demonstrate"}]},
        {"kind":"spell","effects":[
            {"steps":[
                {"effect":"shuffle","who":you,"library":library},
                {"effect":"scan-library","who":you,"operation":"reveal","library":library,
                    "until":{"count":{"amount":"fixed","value":1},"matching":{"nonTypes":["land"],"cls":"card"}}}
            ]},
            {"steps":[
                {"effect":"exile","what":{"ref":"that","filter":card}},
                {"effect":"move-zone","what":{"ref":"rest"},
                    "to":{"zone":"library","owner":"your","position":"bottom"},"order":"random"}
            ]},
            {"steps":[{"effect":"play-card","instruction":{
                "who":you,"action":"cast","what":{"ref":"related","number":"singular","filter":card,
                    "relation":{"relation":"event-result","event":"exile","scope":"contextual"}},
                "alternativeCost":{"cost":"without-mana-cost"}},"optional":true}]}
        ]}
    ])
}

fn reveal() -> Instruction {
    Instruction::RevealAndCast { source_block: 1 }
}

fn demonstrate() -> Instruction {
    Instruction::Demonstrate { source_block: 0 }
}

fn complete_program(p: &SpellProgram, timing: SpellTiming, instruction: Instruction) -> bool {
    p.timing == timing
        && p.resolution == SpellResolution::Instructions
        && p.functioning_zone == FunctioningZone::Stack
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.targets.is_empty()
        && p.modes.is_empty()
        && p.instructions == [instruction]
}

pub(super) fn is_demonstrate_program(p: &SpellProgram) -> bool {
    complete_program(p, SpellTiming::Instant, demonstrate())
}

fn reserved(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::RevealAndCast { .. } | Instruction::Demonstrate { .. }
        )
    }) || p.modes.iter().any(|m| reserved(&m.program))
}

fn source_marker(source: &Value) -> bool {
    source["keywords"]
        .as_array()
        .is_some_and(|keywords| keywords.iter().any(|k| k == "Demonstrate"))
        || source["oracle_text"].as_str().is_some_and(|text| {
            text.contains(SCAN) || text.lines().any(|line| line.starts_with("Demonstrate"))
        })
}

fn ast_marker(node: &Value) -> bool {
    match node {
        Value::Array(items) => items.iter().any(ast_marker),
        Value::Object(fields) => {
            node["keyword"] == "demonstrate"
                || node["effect"] == "scan-library"
                || fields.values().any(ast_marker)
        }
        _ => false,
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    let uses_operation = d.spell.as_ref().is_some_and(reserved)
        || d.triggered.iter().any(|a| reserved(&a.program))
        || d.activated.iter().any(|a| reserved(&a.program))
        || d.static_abilities.iter().any(|a| {
            matches!(&a.effect, StaticEffect::GrantSpellCascade { program, .. } if reserved(program))
        });
    if !uses_operation
        && !d.capabilities.contains(&Capability::DemonstrateV1)
        && !d.capabilities.contains(&Capability::RevealAndCastV1)
        && !((raw.contains("Demonstrate") || raw.contains(SCAN) || raw.contains(r"\u"))
            && source_marker(&strict_read(raw)?))
        && !((raw_ast.contains("demonstrate")
            || raw_ast.contains("scan-library")
            || raw_ast.contains(r"\u"))
            && ast_marker(&strict_read(raw_ast)?))
    {
        return Ok(());
    }

    let source: Value = strict_read(raw)?;
    let c = &d.characteristics;
    if c.kind != CardKind::Sorcery
        || c.basic
        || c.legendary
        || c.snow
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.keywords.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d
            .spell
            .as_ref()
            .is_some_and(|p| complete_program(p, SpellTiming::MainPhase, reveal()))
        || !matches!(d.triggered.as_slice(), [t] if t.functioning_zone == FunctioningZone::Stack
            && t.condition == TriggerCondition::SelfSpellCast
            && t.intervening_if.is_none()
            && t.death_filter.is_none()
            && is_demonstrate_program(&t.program))
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["layout"] != "normal"
        || source["type_line"] != "Sorcery"
        || ["card_faces", "power", "toughness", "loyalty", "defense"]
            .iter()
            .any(|field| source.get(*field).is_some())
        || source["keywords"] != json!(["Demonstrate"])
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["oracle_text"] != TEXT
        || strict_read::<Value>(raw_ast)? != abilities()
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != TEXT
                || l.blocks != ["Demonstrate", SPELL_TEXT]
                || l.triggered != [0]
                || !l.activated.is_empty()
                || !l.mana.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

fn has_instruction(op: &str) -> Value {
    json!({"type":"object","required":["instructions"],"properties":{"instructions":{"contains":{
        "required":["op"],"properties":{"op":{"const":op}}
    }}}})
}

pub(super) fn reveal_spell_schema() -> Value {
    json!({"required":["spell"],"properties":{"spell":has_instruction("reveal_and_cast")}})
}

fn program_schema(timing: SpellTiming, instruction: Instruction) -> Value {
    json!({"type":"object","properties":{
        "timing":{"const":timing},"resolution":{"const":SpellResolution::Instructions},
        "functioning_zone":{"const":FunctioningZone::Stack},"targets":{"maxItems":0},
        "modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},
        "additional_sacrifice":{"type":"null"},"instructions":{"const":[instruction]}
    }})
}

pub(super) fn demonstrate_trigger_schema() -> Value {
    json!({"properties":{
        "condition":{"const":TriggerCondition::SelfSpellCast},"functioning_zone":{"const":FunctioningZone::Stack},
        "intervening_if":{"type":"null"},"death_filter":{"type":"null"},
        "program":program_schema(SpellTiming::Instant, demonstrate())
    }})
}

pub(super) fn extend_schema(schema: &mut Value) {
    let reserved =
        json!({"anyOf":[has_instruction("reveal_and_cast"),has_instruction("demonstrate")]});
    // Each nested mode is a SpellProgram and enforces the same restriction.
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"modes":{"items":{"properties":{"program":{"not":reserved}}}}}
        }));
    let oracle = TEXT
        .replace('(', r"\(")
        .replace(')', r"\)")
        .replace('.', r"\.")
        .replace('\n', r"\\n");
    let source = json!({"pattern":r#""oracle_text"\s*:\s*"([^"]*\\n)?Demonstrate|Shuffle your library, then reveal cards from the top of it until you reveal a nonland card\.|"keywords"\s*:\s*\[[^\]]*"Demonstrate""#});
    let keyword = json!({"pattern":r#""keyword"\s*:\s*"demonstrate""#});
    let scan = json!({"pattern":r#""effect"\s*:\s*"scan-library""#});
    let triggered = json!({"required":["triggered"],"properties":{"triggered":{"contains":{
        "required":["program"],"properties":{"program":reserved}
    }}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"activated":{"items":{"properties":{"program":{"not":reserved}}}}},
        "if":{"anyOf":[
            {"properties":{"spell":reserved}},triggered,
            {"properties":{"capabilities":{"contains":{"enum":[Capability::DemonstrateV1,Capability::RevealAndCastV1]}}}},
            {"properties":{"provenance":{"properties":{"source_json":source}}}},
            {"properties":{"provenance":{"properties":{"abilities_json":{"anyOf":[keyword,scan]}}}}}
        ]},
        "then":{"required":["spell","triggered"],"properties":{
            "spell":program_schema(SpellTiming::MainPhase, reveal()),
            "triggered":{"minItems":1,"maxItems":1,"items":demonstrate_trigger_schema()},
            "characteristics":{"properties":{
                "kind":{"const":CardKind::Sorcery},"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                "power":{"type":"null"},"toughness":{"type":"null"},
                "colors":{"items":{"not":{"const":Color::C}}},"color_identity":{"items":{"not":{"const":Color::C}}},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
            }},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"keywords":{"maxItems":0},
            "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"static_abilities":{"maxItems":0},
            "source_labels":{"properties":{
                "text":{"const":TEXT},"blocks":{"const":["Demonstrate",SPELL_TEXT]},"triggered":{"const":[0]},
                "activated":{"maxItems":0},"mana":{"maxItems":0},"statics":{"maxItems":0},"spell_modes":{"maxItems":0}
            }},
            "provenance":{"properties":{
                "source_json":{"pattern":format!(r#""oracle_text"\s*:\s*"{oracle}""#)},
                "abilities_json":{"allOf":[keyword,scan]}
            }},
            "capabilities":{"minItems":CAPS.len(),"maxItems":CAPS.len(),"uniqueItems":true,"items":{"enum":CAPS}}
        }}
    }));
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) use tests::witness;
