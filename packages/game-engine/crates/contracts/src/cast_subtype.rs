//! Paid cast observers with subtype alternatives. CR 205.3, 601.2i and 603.3a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) fn subtype_spelling(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.as_bytes()[0].is_ascii_uppercase()
        && s.bytes()
            .all(|b| b.is_ascii_alphabetic() || matches!(b, b'-' | b' '))
}

fn body(line: &str) -> Option<(&str, &str)> {
    let rest = line
        .strip_prefix("Whenever you cast an ")
        .or_else(|| line.strip_prefix("Whenever you cast a "))?;
    rest.split_once(" spell, ")
}

fn filter(text: &str) -> Option<(Value, TypePredicate)> {
    let names = if let Some((start, end)) = text.rsplit_once(", or ") {
        start.split(", ").chain([end]).collect::<Vec<_>>()
    } else if let Some((a, b)) = text.split_once(" or ") {
        vec![a, b]
    } else {
        vec![text]
    };
    if names.len() > 16
        || names
            .iter()
            .enumerate()
            .any(|(i, s)| !subtype_spelling(s) || names[..i].contains(s))
    {
        return None;
    }
    let subtype = |name: &str| TypePredicate::Subtype {
        subtype: name.into(),
    };
    if let [name] = names.as_slice() {
        return Some((json!({"subtypes":[name],"cls":"spell"}), subtype(name)));
    }
    let terms = names.iter().map(|s| subtype(s)).collect();
    let any_of = names
        .iter()
        .map(|name| json!({"subtypes":[name]}))
        .collect::<Vec<_>>();
    Some((
        json!({"allOf":[{"anyOf":any_of}],"cls":"spell"}),
        TypePredicate::AnyOf { terms },
    ))
}

fn token_action(action: &str, block: u32) -> Option<(Value, Instruction)> {
    let printed = action
        .strip_prefix("create a ")?
        .strip_suffix(" creature token.")?;
    let (stats, names) = printed.split_once(' ')?;
    let (power, toughness) = stats.split_once('/')?;
    let power = equipped_trigger::count(power)?;
    let toughness = equipped_trigger::count(toughness)?;
    if !stats.bytes().all(|b| b.is_ascii_digit() || b == b'/') {
        return None;
    }
    let (color, subtypes) = names.split_once(' ')?;
    let color_value = match color {
        "white" => Color::W,
        "blue" => Color::U,
        "black" => Color::B,
        "red" => Color::R,
        "green" => Color::G,
        _ => return None,
    };
    let subtypes = subtypes
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if subtypes.is_empty() || subtypes.iter().any(|s| !subtype_spelling(s)) {
        return None;
    }
    let fixed = |n| json!({"amount":"fixed","value":n});
    let ast = json!({"effect":"create-token","count":fixed(1),"token":{
        "colors":[color],"supertypes":[],"types":["creature"],"subtypes":subtypes,"keywords":[],
        "power":fixed(power),"toughness":fixed(toughness)
    }});
    let token = CreatureToken {
        additional_types: vec![],
        colors: vec![color_value],
        subtypes,
        power: i64::from(power).into(),
        toughness: i64::from(toughness).into(),
        keywords: vec![],
    };
    token.validate().ok()?;
    Some((
        ast,
        Instruction::CreateCreatureTokens {
            count: 1,
            token,
            source_block: block,
        },
    ))
}

fn action(action: &str, block: u32) -> Option<(Value, Instruction)> {
    let optional = match action {
        "draw a card." => false,
        "you may draw a card." => true,
        _ => return token_action(action, block),
    };
    let mut draw =
        json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}});
    if optional {
        draw["optional"] = json!(true);
    }
    Some((
        draw,
        if optional {
            Instruction::OptionalDrawCards {
                source_block: block,
            }
        } else {
            Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: block,
            }
        },
    ))
}

pub fn candidate(line: &str) -> bool {
    body(line).is_some_and(|(which, text)| filter(which).is_some() && action(text, 0).is_some())
}

pub fn printed(line: &str, ast: &Value, block: u32) -> Option<TriggeredAbility> {
    let (which, text) = body(line)?;
    let (filter, spell) = filter(which)?;
    let (step, instruction) = action(text, block)?;
    let expected = json!({"kind":"triggered","trigger":{"trigger":"cast","who":{"player":"you"},"what":filter},
        "effects":[{"steps":[step]}]});
    (*ast == expected).then_some(TriggeredAbility {
        condition: TriggerCondition::SpellCast {
            caster: CasterRelation::Controller,
            spell,
        },
        functioning_zone: FunctioningZone::Battlefield,
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
            instructions: vec![instruction],
        },
    })
}

pub(super) fn restricted(p: &TypePredicate) -> bool {
    match p {
        TypePredicate::Subtype { .. } => true,
        TypePredicate::All { terms } | TypePredicate::AnyOf { terms } => {
            terms.iter().any(restricted)
        }
        _ => false,
    }
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let count = d
        .triggered
        .iter()
        .filter(
            |a| matches!(&a.condition,TriggerCondition::SpellCast {spell,..} if restricted(spell)),
        )
        .count();
    let raw = &d.provenance.source_json;
    if count == 0 && !raw.contains(" spell, ") && !raw.contains(r"\u") {
        return Ok(());
    }
    let row: Value = strict_read(raw)?;
    let oracle = row["oracle_text"].as_str().unwrap_or_default();
    let expected = oracle
        .lines()
        .enumerate()
        .filter(|(_, line)| candidate(line))
        .collect::<Vec<_>>();
    if count == 0 && expected.is_empty() {
        return Ok(());
    }
    let error = "requires exact printed cast subtype trigger";
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    if count != expected.len()
        || ast.len() != oracle.lines().count()
        || row["name"] != d.name
        || row["oracle_id"] != d.oracle_id
        || row["id"] != d.provenance.printing_id
        || row["layout"] != "normal"
    {
        return Err(error.into());
    }
    for (index, line) in expected {
        let block = u32::try_from(index).map_err(|_| error)?;
        let ordinal = ast[..index]
            .iter()
            .filter(|a| a["kind"] == "triggered")
            .count();
        let trigger = printed(line, &ast[index], block).ok_or(error)?;
        if d.triggered.get(ordinal) != Some(&trigger)
            || d.source_labels
                .as_ref()
                .is_some_and(|l| l.triggered.get(ordinal) != Some(&block))
        {
            return Err(error.into());
        }
    }
    Ok(())
}

pub(super) fn extend_schema(s: &mut Value) {
    let nested = json!({"$ref":"#/$defs/HasSubtypePredicate"});
    s["$defs"]["HasSubtypePredicate"] = json!({"anyOf":[
        {"type":"object","required":["predicate"],"properties":{"predicate":{"const":"subtype"}}},
        {"type":"object","required":["terms"],"properties":{"terms":{"contains":nested}}}
    ]});
    let found = json!({"type":"object","required":["spell_cast"],"properties":{
        "spell_cast":{"required":["spell"],"properties":{"spell":nested}}
    }});
    let cap = json!({"contains":{"const":Capability::CastSubtypePredicateV1}});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["triggered"],"properties":{"triggered":{"contains":{"required":["condition"],"properties":{"condition":found}}}}},
        "then":{"properties":{"capabilities":cap}},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/7e00b0cd-d212-4604-ba07-da21f4fe00b0.json"
    ))
    .unwrap();
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Grizzly Bears")
    .unwrap();
    let ast = composition_fixtures::stamp(&mut d, &row);
    d.characteristics.legendary = true;
    d.characteristics.subtypes = vec!["Dwarf".into(), "Advisor".into()];
    d.characteristics.colors = vec![Color::W];
    d.characteristics.color_identity = vec![Color::W];
    d.characteristics.mana_cost.colored = vec![Color::W];
    d.triggered =
        vec![printed(row["source"]["oracle_text"].as_str().unwrap(), &ast[0], 0).unwrap()];
    d.capabilities = d.requirements();
    d
}
