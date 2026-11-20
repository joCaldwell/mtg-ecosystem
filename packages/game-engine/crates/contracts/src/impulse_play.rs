//! CR 305.1–3, 400.7j, 406.3, 601.2, 608.2c and 611.2a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const COSTS_REMINDER: &str = " (If you cast a spell this way, you still pay its costs. You can play a land this way only if you have an available land play remaining.)";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayWindow {
    ThisTurn,
    ThroughNextControllerTurn,
}

pub fn candidate(text: &str) -> bool {
    text.starts_with("Exile the top ") && text.contains("you may play those cards.")
}

fn spec(text: &str, kind: CardKind) -> Option<(Value, SpellProgram)> {
    let timing = match kind {
        CardKind::Instant => SpellTiming::Instant,
        CardKind::Sorcery => SpellTiming::MainPhase,
        _ => return None,
    };
    let (exile, grant) = text
        .strip_prefix("Exile the top ")?
        .split_once(" cards of your library. ")?;
    let count = equipped_trigger::count(exile)?;
    if count < 2 {
        return None;
    }
    let (duration, window) = match grant.strip_suffix(COSTS_REMINDER).unwrap_or(grant) {
        "Until end of turn, you may play those cards." => ("end-of-turn", PlayWindow::ThisTurn),
        "Until the end of your next turn, you may play those cards." => (
            "end-of-your-next-turn",
            PlayWindow::ThroughNextControllerTurn,
        ),
        _ => return None,
    };
    let ast = json!([{"kind":"spell","effects":[
        {"steps":[{"effect":"exile","what":{"ref":"library-cards","position":"top","count":{"amount":"fixed","value":count},"of":{"zone":"library","owner":"your"}}}]},
        {"steps":[{"effect":"grant-play-permission","permission":{"who":{"player":"you"},"action":"play","what":{"ref":"that","filter":{"cls":"card"},"plural":true}},"duration":{"duration":duration}}]}
    ]}]);
    let program = SpellProgram {
        timing,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        targets: vec![],
        modes: vec![],
        instructions: vec![Instruction::ExileLibraryForPlay {
            count,
            window,
            source_block: 0,
        }],
    };
    Some((ast, program))
}

pub fn printed(text: &str, ast: &Value, kind: CardKind) -> Option<SpellProgram> {
    let (expected, program) = spec(text, kind)?;
    (*ast == expected).then_some(program)
}

pub(super) fn chapter(text: &str, source_block: u32) -> Option<(Value, SpellProgram)> {
    let (ast, mut program) = spec(text, CardKind::Instant)?;
    let [
        Instruction::ExileLibraryForPlay {
            source_block: block,
            ..
        },
    ] = program.instructions.as_mut_slice()
    else {
        return None;
    };
    *block = source_block;
    Some((ast[0]["effects"].clone(), program))
}

fn uses(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::ExileLibraryForPlay { .. }))
        || program.modes.iter().any(|m| uses(&m.program))
}

pub(super) fn validate(definition: &Definition) -> Result<(), String> {
    let error = "library play permission requires a complete exact ordinary spell";
    if definition.activated.iter().any(|a| uses(&a.program))
        || definition.triggered.iter().any(|a| uses(&a.program) && !matches!(
            a.condition, TriggerCondition::SagaChapter { .. }
        ))
        || definition.static_abilities.iter().any(|a| matches!(&a.effect, StaticEffect::Bestow { program, .. } | StaticEffect::GrantSpellCascade { program, .. } if uses(program))) {
        return Err(error.into());
    }
    let used = definition.spell.as_ref().is_some_and(uses);
    let raw = &definition.provenance.source_json;
    if !used && !raw.contains("Exile the top ") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let Some((ast, expected)) = spec(
        source["oracle_text"].as_str().unwrap_or_default(),
        definition.characteristics.kind,
    ) else {
        return if used { Err(error.into()) } else { Ok(()) };
    };
    let c = &definition.characteristics;
    if definition.spell.as_ref() != Some(&expected)
        || strict_read::<Value>(&definition.provenance.abilities_json)? != ast
        || source["oracle_id"] != definition.oracle_id
        || source["id"] != definition.provenance.printing_id
        || source["name"] != definition.name
        || source["layout"] != "normal"
        || source["type_line"]
            != if c.kind == CardKind::Instant {
                "Instant"
            } else {
                "Sorcery"
            }
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || !definition.triggered.is_empty()
        || !definition.activated.is_empty()
        || !definition.static_abilities.is_empty()
        || !definition.mana_abilities.is_empty()
        || !definition.keywords.is_empty()
        || !c.hybrid_mana.is_empty()
        || definition.intrinsic_zone.is_some()
        || definition.x_mana_cost
    {
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let op = json!({"required":["op"],"properties":{"op":{"const":"exile_library_for_play"}}});
    let free = json!({"$ref":"#/$defs/ImpulseFreeProgram"});
    schema["$defs"]["ImpulseFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":op}}, "modes":{"items":{"properties":{"program":free}}}
    }});
    let chapter = json!({"$ref":"#/$defs/SagaImpulseChapter"});
    let cap = json!({"contains":{"const":Capability::LibraryPlayWindowV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "activated":{"items":{"properties":{"program":free}}},
            "triggered":{"items":{"anyOf":[{"properties":{"program":free}},chapter]}},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{
                "bestow":{"properties":{"program":free}},
                "grant_spell_cascade":{"properties":{"program":free}}
            }}}}}
        },
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
        "then":{"properties":{"capabilities":cap,
            "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
            "spell":{"properties":{
                "instructions":{"minItems":1,"maxItems":1,"items":op},
                "modes":{"maxItems":0},"targets":{"maxItems":0},
                "optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"}
            }}
        }},
        "else":{
            "if":{"required":["triggered"],"properties":{"triggered":{"contains":chapter}}},
            "then":{"properties":{"capabilities":cap}},
            "else":{"properties":{"capabilities":{"not":cap}}}
        }
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/584cf0fd-112a-4ca6-9c0e-1f3228a7d325.json"
    ))
    .unwrap();
    let mut definition = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.characteristics.kind == CardKind::Sorcery)
    .unwrap();
    let ast = composition_fixtures::stamp(&mut definition, &row);
    definition.characteristics.colors = vec![Color::R];
    definition.characteristics.color_identity = vec![Color::R];
    definition.characteristics.mana_cost = ManaCost {
        generic: 1.into(),
        colored: vec![Color::R],
    };
    definition.spell = printed(
        row["source"]["oracle_text"].as_str().unwrap(),
        &ast,
        CardKind::Sorcery,
    );
    definition.capabilities = definition.requirements();
    definition
}
