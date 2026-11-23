//! CR 113.6b, 614.1a, 701.24c, 702.16j.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub const PROTECTION_TEXT: &str = "Protection from everything";

const COLORS: [Color; 5] = [Color::B, Color::G, Color::R, Color::U, Color::W];

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
