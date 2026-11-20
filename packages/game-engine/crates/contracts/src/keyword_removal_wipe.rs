//! Bounded layer-six keyword removal followed by simultaneous destruction.
//! CR 611.2c, 613.1f, 613.7 and 702.12.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ERROR: &str = "keyword removal sweep requires complete ordinary sorcery";

/// The equivalent printed sentence and closed AST must agree before lowering.
pub fn keyword_removal_wipe_source(text: &str) -> Option<Vec<Value>> {
    let mut lines = text.lines();
    let selector = lines
        .next()?
        .strip_suffix(" lose indestructible until end of turn.")?;
    let target = selector.strip_suffix(" your opponents control")?;
    if target != "Nonland permanents"
        || lines
            .next()?
            .strip_prefix("Destroy all ")?
            .strip_suffix('.')?
            != "nonland permanents"
        || lines.next().is_some()
    {
        return None;
    }
    let ability = |step| json!({"kind":"spell","effects":[{"steps":[step]}]});
    Some(vec![
        ability(json!({"effect":"lose-abilities",
            "what":{"ref":"each","filter":{"nonTypes":["land"],"cls":"permanent","control":"opponent"}},
            "removal":{"removal":"listed","abilities":[{"gained":"keyword","keyword":{"keyword":"indestructible"}}]},
            "duration":{"duration":"end-of-turn"}})),
        ability(
            json!({"effect":"destroy","what":{"ref":"all","filter":{"nonTypes":["land"],"cls":"permanent"}}}),
        ),
    ])
}
pub fn keyword_removal_wipe_program() -> SpellProgram {
    SpellProgram {
        timing: SpellTiming::MainPhase,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        targets: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
        modes: vec![],
        instructions: vec![
            Instruction::RemoveOpponentNonlandIndestructible { source_block: 0 },
            Instruction::DestroySet {
                set: ObjectSet::NonlandPermanents,
                regeneration_forbidden: false,
                source_block: 1,
            },
        ],
    }
}

fn reserved(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::RemoveOpponentNonlandIndestructible { .. }
                | Instruction::DestroySet {
                    set: ObjectSet::NonlandPermanents,
                    regeneration_forbidden: false,
                    ..
                }
        )
    }) || program.modes.iter().any(|mode| reserved(&mode.program))
}

pub(super) fn validate_definition(definition: &Definition) -> Result<(), String> {
    if definition.activated.iter().any(|a| reserved(&a.program))
        || definition.triggered.iter().any(|a| reserved(&a.program))
        || definition.static_abilities.iter().any(|a| {
            matches!(
                 &a.effect, StaticEffect::GrantSpellCascade { program, .. } if reserved(program)
            )
        })
    {
        return Err(ERROR.into());
    }
    let reserved_spell = definition.spell.as_ref().is_some_and(reserved);
    let raw = &definition.provenance.source_json;
    if !reserved_spell
        && !raw.contains(" lose indestructible until end of turn.")
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let Some(expected) = keyword_removal_wipe_source(text) else {
        return if reserved_spell {
            Err(ERROR.into())
        } else {
            Ok(())
        };
    };
    let c = &definition.characteristics;
    let mut printed_cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        printed_cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        printed_cost.push_str(&format!("{{{color:?}}}"));
    }
    let actual: Vec<Value> = strict_read(&definition.provenance.abilities_json)?;
    if actual != expected
        || definition.spell.as_ref() != Some(&keyword_removal_wipe_program())
        || source["layout"] != "normal"
        || source["type_line"] != "Sorcery"
        || source["id"] != definition.provenance.printing_id
        || source["oracle_id"] != definition.oracle_id
        || source["name"] != definition.name
        || source["mana_cost"] != printed_cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || definition.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || !text.lines().eq(l.blocks.iter().map(String::as_str))
                || !l.activated.is_empty()
                || !l.triggered.is_empty()
                || !l.mana.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
        || definition.characteristics.kind != CardKind::Sorcery
        || definition.characteristics.basic
        || definition.characteristics.legendary
        || definition.characteristics.snow
        || !definition.characteristics.additional_types.is_empty()
        || !definition.characteristics.subtypes.is_empty()
        || !definition.keywords.is_empty()
        || !definition.static_abilities.is_empty()
        || !definition.activated.is_empty()
        || !definition.triggered.is_empty()
        || !definition.mana_abilities.is_empty()
        || definition.intrinsic_zone.is_some()
        || definition.x_mana_cost
    {
        return Err(ERROR.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let free = json!({"$ref":"#/$defs/KeywordRemovalFreeProgram"});
    schema["$defs"]["KeywordRemovalFreeProgram"] = json!({
        "properties":{
            "instructions":{"items":{"not":{"anyOf":[
                {"required":["op"],"properties":{"op":{"const":"remove_opponent_nonland_indestructible"}}},
                {"required":["op","set"],"properties":{"op":{"const":"destroy_set"},
                    "set":{"const":"nonland_permanents"},"regeneration_forbidden":{"const":false}}}
            ]}}},
            "modes":{"items":{"properties":{"program":free}}}
        }
    });
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "triggered":{"items":{"properties":{"program":free}}},
            "activated":{"items":{"properties":{"program":free}}},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{
                "grant_spell_cascade":{"properties":{"program":free}}
            }}}}}
        },
        "if":{"anyOf":[
            {"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
            {"properties":{"provenance":{"properties":{"source_json":{
                "pattern":r#""oracle_text"\s*:\s*"[^"]* lose indestructible until end of turn\."#
            }}}}}
        ]},
        "then":{"required":["spell"],"properties":{
            "spell":{"type":"object","properties":{
                "functioning_zone":{"const":"stack"},"timing":{"const":"main_phase"},
                "resolution":{"const":"instructions"},"targets":{"maxItems":0},"modes":{"maxItems":0},
                "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
                "instructions":{"const":keyword_removal_wipe_program().instructions}
            }},
            "characteristics":{"properties":{"kind":{"const":"sorcery"},"additional_types":{"maxItems":0},
                "subtypes":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false}}},
            "static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},
            "mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},"intrinsic_zone":{"type":"null"},
            "x_mana_cost":{"const":false},
            "capabilities":{"minItems":4,"maxItems":4,"uniqueItems":true,"items":{"enum":[
                Capability::NormalLayoutV1, Capability::CastSorceryV1, Capability::BoardWipesV1,
                Capability::RemoveIndestructibleV1
            ]}}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::RemoveIndestructibleV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness(mut definition: Definition) -> Definition {
    let mut source: Value = strict_read(&definition.provenance.source_json).unwrap();
    let selector = "Nonland permanents";
    let term = "indestructible";
    let text = format!(
        "{selector} your opponents control lose {term} until end of turn.\nDestroy all {}.",
        selector.to_lowercase()
    );
    source["oracle_text"] = json!(text);
    definition.source_labels = None;
    definition.provenance.source_json = canonical(&source).unwrap();
    definition.provenance.source_sha256 = sha256(definition.provenance.source_json.as_bytes());
    definition.provenance.abilities_json =
        canonical(&keyword_removal_wipe_source(&text).unwrap()).unwrap();
    definition.provenance.ast_sha256 =
        sha256(format!("[{}]", definition.provenance.abilities_json).as_bytes());
    definition.spell = Some(keyword_removal_wipe_program());
    definition.capabilities = definition.requirements();
    definition
}
