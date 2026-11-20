//! CR 113.6b, 614.1a, 701.24c, 702.16j.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub const PROTECTION_TEXT: &str = "Protection from everything";
pub const REPLACEMENT_TEXT: &str = "If ~ would be put into a graveyard from anywhere, reveal ~ and shuffle it into its owner's library instead.";
const TYPE_LINE: &str = "Legendary Creature — Hydra Avatar";
const COST: &str = "{W}{W}{U}{U}{B}{B}{R}{R}{G}{G}";
const COLORS: [Color; 5] = [Color::B, Color::G, Color::R, Color::U, Color::W];
const CAPABILITIES: [Capability; 6] = [
    Capability::NormalLayoutV1,
    Capability::BaseCreatureV1,
    Capability::CastCreatureV1,
    Capability::CommanderIdentityV1,
    Capability::ProtectionFromEverythingV1,
    Capability::SelfGraveyardRevealShuffleV1,
];
const INVALID: &str =
    "requires the complete intrinsic protection and self graveyard replacement creature";

pub fn abilities() -> Value {
    let you = json!({"player":"you"});
    let affected = json!({"ref":"replacement-event-object","role":"affected"});
    json!([
        {"kind":"keywords","keywords":[{"keyword":"protection","from":[{"scope":"everything"}]}]},
        {"kind":"static","effect":{"static":"replacement","replacement":{
            "event":{"event":"move-zone","what":{"ref":"self"},
                "to":{"zone":"graveyard","owner":"any"},"from":"anywhere"},
            "occurrence":"each","replace":{"replacement":"effects","effects":[{"steps":[
                {"effect":"reveal","who":you,"what":affected},
                {"effect":"shuffle-into-library","who":you,"what":[affected],
                    "library":{"zone":"library","owner":"its-owner"}}
            ]}]}
        }}}
    ])
}

fn printed_text(name: &str) -> String {
    format!("{PROTECTION_TEXT}\n{}", REPLACEMENT_TEXT.replace('~', name))
}

pub fn source_matches(source: &Value, name: &str) -> bool {
    source["name"] == name
        && source["layout"] == "normal"
        && source["type_line"] == TYPE_LINE
        && source["mana_cost"] == COST
        && source["power"] == "10"
        && source["toughness"] == "10"
        && source["colors"] == json!(COLORS)
        && source["color_identity"] == json!(COLORS)
        && source["keywords"] == json!(["Protection"])
        && source["oracle_text"] == printed_text(name)
        && ["card_faces", "loyalty", "defense"]
            .iter()
            .all(|field| source.get(*field).is_none())
}

fn ast_marker(node: &Value) -> bool {
    match node {
        Value::Array(items) => items.iter().any(ast_marker),
        Value::Object(fields) => {
            // A player-granted keyword uses the same protection payload.
            // Only a printed keywords ability identifies intrinsic protection.
            (node["kind"] == "keywords"
                && node["keywords"].as_array().is_some_and(|keywords| {
                    keywords.iter().any(|keyword| {
                        keyword["keyword"] == "protection"
                            && keyword["from"].as_array().is_some_and(|sources| {
                                sources.iter().any(|from| from["scope"] == "everything")
                            })
                    })
                }))
                || (node["effect"] == "shuffle-into-library"
                    && node["what"].as_array().is_some_and(|items| {
                        items
                            .iter()
                            .any(|what| what["ref"] == "replacement-event-object")
                    }))
                || fields.values().any(ast_marker)
        }
        _ => false,
    }
}

fn text_marker(text: &str) -> bool {
    text.contains(PROTECTION_TEXT)
        || (text.contains("would be put into a graveyard from anywhere")
            && text.contains("shuffle it into its owner's library"))
}

pub fn candidate(text: &str, raw_ast: &str) -> Result<bool, String> {
    if text_marker(text) {
        return Ok(true);
    }
    Ok(
        ((raw_ast.contains("protection") && raw_ast.contains("everything"))
            || (raw_ast.contains("shuffle-into-library")
                && raw_ast.contains("replacement-event-object"))
            || raw_ast.contains(r"\u"))
            && ast_marker(&strict_read(raw_ast)?),
    )
}

pub fn static_ability() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Any,
        effect: StaticEffect::SelfGraveyardRevealShuffle,
    }
}

fn characteristics() -> Characteristics {
    use Color::*;
    Characteristics {
        kind: CardKind::Creature,
        additional_types: vec![],
        basic: false,
        legendary: true,
        snow: false,
        subtypes: vec!["Hydra".into(), "Avatar".into()],
        mana_cost: ManaCost {
            generic: Quantity::zero(),
            colored: vec![W, W, U, U, B, B, R, R, G, G],
        },
        hybrid_mana: vec![],
        colors: COLORS.to_vec(),
        color_identity: COLORS.to_vec(),
        power: Some(10.into()),
        toughness: Some(10.into()),
    }
}

fn spell() -> SpellProgram {
    SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated
        .iter()
        .any(|a| a.functioning_zone == FunctioningZone::Any)
        || d.triggered
            .iter()
            .any(|a| a.functioning_zone == FunctioningZone::Any)
    {
        return Err("all-zone functioning is reserved for the self graveyard replacement".into());
    }
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    let has_static = d
        .static_abilities
        .iter()
        .any(|a| a.effect == StaticEffect::SelfGraveyardRevealShuffle);
    if !has_static
        && !d
            .keywords
            .contains(&KeywordAbility::ProtectionFromEverything)
        && !candidate("", raw_ast)?
        && !((text_marker(raw) || raw.contains(r"\u"))
            && strict_read::<Value>(raw)?["oracle_text"]
                .as_str()
                .is_some_and(text_marker))
    {
        return Ok(());
    }

    let source: Value = strict_read(raw)?;
    if !source_matches(&source, &d.name)
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || strict_read::<Value>(raw_ast)? != abilities()
        || d.characteristics != characteristics()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || d.spell.as_ref() != Some(&spell())
        || d.keywords != [KeywordAbility::ProtectionFromEverything]
        || d.static_abilities != [static_ability()]
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != printed_text(&d.name)
                || labels.blocks != [PROTECTION_TEXT, REPLACEMENT_TEXT]
                || labels.statics != [1]
                || !labels.activated.is_empty()
                || !labels.triggered.is_empty()
                || !labels.mana.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let zone = json!({"const":FunctioningZone::Any});
    let effect = json!({"const":StaticEffect::SelfGraveyardRevealShuffle});
    schema["$defs"]["StaticAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["effect"],"properties":{"effect":effect}},
            "then":{"properties":{"functioning_zone":zone}},
            "else":{"properties":{"functioning_zone":{"not":zone}}}
        }));
    for kind in ["SpellProgram", "ActivatedAbility", "TriggeredAbility"] {
        let property = &mut schema["$defs"][kind]["properties"]["functioning_zone"];
        *property = json!({"allOf":[property.clone(),{"not":zone}]});
    }
    let intrinsic_zone = &mut schema["$defs"]["Definition"]["properties"]["intrinsic_zone"];
    *intrinsic_zone = json!({"allOf":[intrinsic_zone.clone(),{"not":zone}]});
    let cap = json!({"contains":{"enum":[
        Capability::ProtectionFromEverythingV1, Capability::SelfGraveyardRevealShuffleV1
    ]}});
    let keyword = KeywordAbility::ProtectionFromEverything;
    let c = serde_json::to_value(characteristics()).expect("creature characteristics serialize");
    let p = spell();
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"required":["keywords"],"properties":{"keywords":{"contains":{"const":keyword}}}},
            {"required":["static_abilities"],"properties":{"static_abilities":{"contains":{
                "required":["effect"],"properties":{"effect":effect}
            }}}},
            {"properties":{"capabilities":cap}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":PROTECTION_TEXT}}}}}
        ]},
        "then":{"required":["spell","keywords","static_abilities"],"properties":{
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "keywords":{"const":[keyword]},"static_abilities":{"const":[static_ability()]},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "characteristics":{"properties":{
                "kind":{"const":c["kind"]},"legendary":{"const":true},
                "basic":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0},
                "subtypes":{"const":c["subtypes"]},"mana_cost":{"const":c["mana_cost"]},
                "power":{"const":c["power"]},"toughness":{"const":c["toughness"]},
                "colors":{"const":c["colors"]},"color_identity":{"const":c["color_identity"]}
            }},
            "spell":{"type":"object","properties":{
                "timing":{"const":p.timing},"resolution":{"const":p.resolution},
                "functioning_zone":{"const":p.functioning_zone},"optional_additional_costs":{"maxItems":0},
                "additional_sacrifice":{"type":"null"},"targets":{"maxItems":0},
                "instructions":{"maxItems":0},"modes":{"maxItems":0}
            }},
            "capabilities":{"items":{"enum":CAPABILITIES},"uniqueItems":true,
                "minItems":CAPABILITIES.len(),"maxItems":CAPABILITIES.len()}
        }}
    }));
}

#[cfg(test)]
pub(super) mod tests;
