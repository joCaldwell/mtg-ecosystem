//! CR 603.5, 608.2d, and 701.23b, 701.24a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub const TEXT: &str = "When this creature enters, you may search your library for a basic land card, put that card onto the battlefield tapped, then shuffle.";

pub fn printed(line: &str, block: u32, kind: CardKind) -> Option<(Value, TriggeredAbility)> {
    if kind != CardKind::Creature || line != TEXT {
        return None;
    }
    let who = json!({"player":"you"});
    let ast = json!({"kind":"triggered","trigger":{"trigger":"enters","what":{"ref":"this","noun":"creature"}},
        "effects":[{"steps":[{"effect":"optional-group","who":who,"steps":[
            {"effect":"search","who":who,"zones":{"selection":"all","of":[{"zone":"library","owner":"your"}]},
                "for":{"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},
                    "count":{"amount":"fixed","value":1}}},
            {"effect":"move-zone","what":{"ref":"that","filter":{"cls":"card"}},"to":{"zone":"battlefield"},"tapped":true},
            {"effect":"shuffle","who":who,"library":{"zone":"library","owner":who}}
        ]}]}]
    });
    let ability = TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            targets: vec![],
            modes: vec![],
            instructions: vec![
                Instruction::ChooseOptional {
                    chooser: EffectRecipient::Controller,
                    following: 1,
                    source_block: block,
                },
                Instruction::SearchRecipientBasicLand {
                    recipient: EffectRecipient::Controller,
                    source_block: block,
                },
            ],
        },
    };
    Some((ast, ability))
}

pub fn draw_sibling(line: &str, raw: &Value, block: u32) -> Option<TriggeredAbility> {
    let optional = match line {
        "When this creature dies, you may draw a card." => true,
        "When this creature dies, draw a card." => false,
        _ => return None,
    };
    let mut step =
        json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}});
    if optional {
        step["optional"] = json!(true);
    }
    let ast = json!({"kind":"triggered","trigger":{"trigger":"dies","what":{"ref":"this","noun":"creature"}},
        "effects":[{"steps":[step]}]
    });
    (*raw == ast).then(|| draw_program(block, optional))
}

fn draw_program(block: u32, optional: bool) -> TriggeredAbility {
    let (_, mut ability) =
        printed(TEXT, block, CardKind::Creature).expect("canonical entry search");
    ability.condition = TriggerCondition::SelfDies;
    ability.program.instructions = vec![if optional {
        Instruction::OptionalDrawCards {
            source_block: block,
        }
    } else {
        Instruction::draw(
            PlayerExpr::you(Context::Triggered),
            Quantity::from(1),
            block,
        )
    }];
    ability
}

pub(super) fn entry_schema() -> Vec<Value> {
    let (_, entry) = printed(TEXT, 0, CardKind::Creature).expect("canonical entry search");
    [("When this creature dies, you may draw a card.", true), ("When this creature dies, draw a card.", false)]
        .into_iter()
        .map(|(text, optional)| json!({
            "if":{"required":["provenance"],"properties":{"provenance":{"required":["source_json"],
                "properties":{"source_json":{"allOf":[
                    {"pattern":TEXT.replace('.', r"\.")},{"pattern":text.replace('.', r"\.")}
                ]}}}}},
            "then":{"required":["triggered"],"properties":{"triggered":{"const":[entry, draw_program(1, optional)]}}}
        }))
        .collect()
}

pub(super) fn validate_siblings(
    d: &Definition,
    row: &Value,
    oracle: &str,
    ast: &[Value],
) -> Result<(), String> {
    let mut lines = oracle.lines();
    let fail = "requires all source clauses and characteristics of the optional entry search";
    let c = &d.characteristics;
    let (head, subtypes) = row["type_line"]
        .as_str()
        .and_then(|text| text.split_once(" — "))
        .ok_or(fail)?;
    let additional_types: &[_] = match head {
        "Creature" => &[],
        "Artifact Creature" => &[CardKind::Artifact],
        _ => return Err(fail.into()),
    };
    if c.kind != CardKind::Creature
        || c.additional_types != additional_types
        || c.snow
        || c.legendary
        || c.basic
        || !subtypes
            .split_whitespace()
            .eq(c.subtypes.iter().map(String::as_str))
        || row["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || c.power.as_ref().map(ToString::to_string).as_deref() != row["power"].as_str()
        || c.toughness.as_ref().map(ToString::to_string).as_deref() != row["toughness"].as_str()
        || row["colors"] != json!(c.colors)
        || row["color_identity"] != json!(c.color_identity)
    {
        return Err(fail.into());
    }
    if lines.next() != Some(TEXT)
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || d.triggered.len() != ast.len()
    {
        return Err(fail.into());
    }
    match (lines.next(), ast.get(1), d.triggered.get(1)) {
        (None, None, None) => (),
        (Some(line), Some(raw), Some(ability))
            if draw_sibling(line, raw, 1).as_ref() == Some(ability) =>
        {
            ()
        }
        _ => return Err(fail.into()),
    }
    if lines.next().is_some() {
        return Err(fail.into());
    }
    Ok(())
}
