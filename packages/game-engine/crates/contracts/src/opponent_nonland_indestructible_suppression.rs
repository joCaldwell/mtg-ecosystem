//! CR 608.2c, 611.2c, 613.1f, 613.7, 701.8 and 702.12.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub const TEXT: &str = "Nonland permanents your opponents control lose indestructible until end of turn.\nDestroy all nonland permanents.";
const INVALID: &str =
    "requires complete opponent nonland indestructible suppression followed by nonland destruction";
const CAPS: [Capability; 4] = [
    Capability::NormalLayoutV1,
    Capability::CastSorceryV1,
    Capability::BoardWipesV1,
    Capability::OpponentNonlandIndestructibleSuppressionV1,
];

pub fn abilities() -> Value {
    json!([
        {"kind":"spell","effects":[{"steps":[{"effect":"lose-abilities",
            "what":{"ref":"each","filter":{"nonTypes":["land"],"cls":"permanent","control":"opponent"}},
            "removal":{"removal":"listed","abilities":[{"gained":"keyword","keyword":{"keyword":"indestructible"}}]},
            "duration":{"duration":"end-of-turn"}}]}]},
        {"kind":"spell","effects":[{"steps":[{"effect":"destroy",
            "what":{"ref":"all","filter":{"nonTypes":["land"],"cls":"permanent"}}}]}]}
    ])
}

pub fn instructions() -> [Instruction; 2] {
    [
        Instruction::SuppressOpponentNonlandIndestructible { source_block: 0 },
        Instruction::DestroySet {
            set: ObjectSet::NonlandPermanents,
            regeneration_forbidden: false,
            source_block: 1,
        },
    ]
}

fn characteristics() -> Characteristics {
    Characteristics {
        kind: CardKind::Sorcery,
        basic: false,
        legendary: false,
        snow: false,
        power: None,
        toughness: None,
        additional_types: vec![],
        subtypes: vec![],
        mana_cost: ManaCost {
            generic: 5.into(),
            colored: vec![Color::W, Color::B],
        },
        colors: vec![Color::B, Color::W],
        color_identity: vec![Color::B, Color::W],
    }
}

fn labels() -> SourceLabels {
    SourceLabels {
        text: TEXT.into(),
        blocks: TEXT.lines().map(str::to_owned).collect(),
        ..Default::default()
    }
}

fn program() -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        timing: SpellTiming::MainPhase,
        targets: vec![],
        modes: vec![],
        instructions: instructions().into(),
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    }
}

fn contains_reserved(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::SuppressOpponentNonlandIndestructible { .. }
                | Instruction::DestroySet {
                    set: ObjectSet::NonlandPermanents,
                    regeneration_forbidden: false,
                    ..
                }
        )
    }) || p.modes.iter().any(|mode| contains_reserved(&mode.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| contains_reserved(&a.program))
        || d.triggered.iter().any(|a| contains_reserved(&a.program))
    {
        return Err(INVALID.into());
    }
    let reserved = d.spell.as_ref().is_some_and(contains_reserved);
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    if !reserved
        && !raw.contains("lose indestructible")
        && !raw.contains(r"\u")
        && !raw_ast.contains("lose-abilities")
        && !raw_ast.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let ast: Value = strict_read(raw_ast)?;
    if !reserved && source["oracle_text"] != TEXT && ast != abilities() {
        return Ok(());
    }
    if source["layout"] != "normal"
        || source["type_line"] != "Sorcery"
        || source["oracle_text"] != TEXT
        || source["oracle_id"] != d.oracle_id
        || source["name"] != d.name
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != "{5}{W}{B}"
        || source["keywords"] != json!([])
        || ["card_faces", "defense", "loyalty", "power", "toughness"]
            .iter()
            .any(|key| source.get(*key).is_some())
        || ast != abilities()
        || d.characteristics != characteristics()
        || source["colors"] != json!(d.characteristics.colors)
        || source["color_identity"] != json!(d.characteristics.color_identity)
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || d.spell.as_ref() != Some(&program())
        || d.source_labels.as_ref().is_some_and(|l| *l != labels())
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let [suppress, destroy] = instructions();
    let free = json!({"$ref":"#/$defs/OpponentNonlandSuppressionFreeProgram"});
    schema["$defs"]["OpponentNonlandSuppressionFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"anyOf":[
            {"required":["op"],"properties":{"op":{"const":"suppress_opponent_nonland_indestructible"}}},
            {"required":["op","set"],"properties":{"op":{"const":"destroy_set"},"set":{"const":ObjectSet::NonlandPermanents},"regeneration_forbidden":{"const":false}}}
        ]}}},"modes":{"items":{"properties":{"program":free}}}
    }});
    // The default false flag can also be explicitly serialized.
    let mut explicit = json!(destroy);
    explicit["regeneration_forbidden"] = json!(false);
    let caps = json!({"uniqueItems":true,"minItems":CAPS.len(),"maxItems":CAPS.len(),"items":{"enum":CAPS}});
    let text = r#""oracle_text"\s*:\s*"Nonland permanents your opponents control lose indestructible until end of turn\.\\nDestroy all nonland permanents\.""#;
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}}},
        "if":{"anyOf":[
            {"properties":{"capabilities":{"contains":{"const":Capability::OpponentNonlandIndestructibleSuppressionV1}}}},
            {"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":text}}}}}
        ]},
        "then":{"required":["spell"],"properties":{
            "capabilities":caps,
            "spell":{"type":"object","properties":{
                "functioning_zone":{"const":"stack"},"timing":{"const":"main_phase"},"resolution":{"const":"instructions"},
                "targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
                "instructions":{"minItems":2,"prefixItems":[{"const":suppress},{"enum":[destroy,explicit]}],"items":false}
            }},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
            "characteristics":{"properties":{"kind":{"const":"sorcery"},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0},
                "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"},
                "mana_cost":{"const":{"generic":"5","colored":["W","B"]}},"colors":{"const":["B","W"]},"color_identity":{"const":["B","W"]}}},
            "source_labels":{"properties":{"text":{"const":TEXT},"blocks":{"const":TEXT.lines().collect::<Vec<_>>()},
                "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana":{"maxItems":0},"statics":{"maxItems":0},"spell_modes":{"maxItems":0}}},
            "provenance":{"properties":{"source_json":{"pattern":text},"abilities_json":{"contentMediaType":"application/json","contentSchema":{"const":abilities()}}}}
        }}
    }));
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(super) use tests::witness;
