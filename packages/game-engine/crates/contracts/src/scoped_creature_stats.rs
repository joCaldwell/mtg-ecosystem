//! Fixed controller-side additive stats at layer 7c (CR 205.3m, 208.3,
//! 208.3a, 611.3a/b and 613.4c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires a complete printed controller-side scoped creature stat modifier";
// The frozen, reviewed subset of CR 205.3m creature types and their printed
// plurals. A type may occur in either the explicit creature intersection or
// the standalone subtype selector.
const SUBTYPES: [(&str, &str); 14] = [
    ("Cat", "Cats"),
    ("Dinosaur", "Dinosaurs"),
    ("Eldrazi", "Eldrazi"),
    ("Kithkin", "Kithkin"),
    ("Knight", "Knights"),
    ("Kobold", "Kobolds"),
    ("Merfolk", "Merfolk"),
    ("Sliver", "Slivers"),
    ("Soldier", "Soldiers"),
    ("Spawn", "Spawn"),
    ("Squirrel", "Squirrels"),
    ("Vampire", "Vampires"),
    ("Warrior", "Warriors"),
    ("Zombie", "Zombies"),
];

fn stat(word: &str) -> Option<u32> {
    let digits = word.strip_prefix('+')?;
    let value = digits.parse::<u32>().ok()?;
    (value.to_string() == digits).then_some(value)
}

/// Derive the executable modifier and exact archive AST from the whole printed
/// sentence. An omitted creature qualifier is retained in the expected AST.
pub fn printed(text: &str) -> Option<(StaticAbility, Value)> {
    let (subject, magnitude) = text.strip_suffix('.')?.split_once(" you control get ")?;
    let (power, toughness) = magnitude.split_once('/')?;
    let power = stat(power)?;
    let toughness = stat(toughness)?;
    if power == 0 && toughness == 0 {
        return None;
    }
    let (subject, other) = subject
        .strip_prefix("Other ")
        .map_or((subject, false), |rest| (rest, true));
    let mut ast_filter = json!({"control":"you"});
    if other {
        ast_filter["other"] = json!(true);
    }
    let filter = match subject {
        "creatures" if other => {
            ast_filter["types"] = json!(["creature"]);
            ControllerCreatureStatFilter::Any
        }
        "artifact creatures" => {
            ast_filter["types"] = json!(["artifact", "creature"]);
            ControllerCreatureStatFilter::Artifact
        }
        _ => {
            let subtypes = if let Some(explicit) = subject.strip_suffix(" creatures") {
                let values: Vec<_> = explicit.split(' ').collect();
                if !(1..=2).contains(&values.len())
                    || values
                        .iter()
                        .any(|v| !SUBTYPES.iter().any(|(singular, _)| v == singular))
                    || values.iter().collect::<BTreeSet<_>>().len() != values.len()
                {
                    return None;
                }
                ast_filter["types"] = json!(["creature"]);
                values
            } else {
                // A subtype-only selector can include Kindreds in the full
                // rules. A noncreature permanent has no stats to change. If
                // it becomes a creature, the live subtype filter is checked
                // again by the same layer traversal (CR 208.3/208.3a).
                vec![SUBTYPES.iter().find(|(_, plural)| subject == *plural)?.0]
            };
            ast_filter["subtypes"] = json!(subtypes);
            ControllerCreatureStatFilter::Subtypes {
                subtypes: subtypes.into_iter().map(str::to_owned).collect(),
            }
        }
    };
    let fixed = |value| json!({"sign":1,"amount":{"amount":"fixed","value":value}});
    Some((
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControllerScopedCreatureStats {
                filter,
                other,
                power: i64::from(power).into(),
                toughness: i64::from(toughness).into(),
            },
        },
        json!({"kind":"static","effect":{"static":"modify","what":{"ref":"each","filter":ast_filter},
            "modifiers":[{"modifier":"pt","power":fixed(power),"toughness":fixed(toughness)}]}}),
    ))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let cap = d.capabilities.contains(&Capability::ScopedCreatureStatsV1);
    let c = &d.characteristics;
    let raw = &d.provenance.source_json;
    if !cap
        && (c.kind != CardKind::Creature
            || (!raw.contains(" you control get ") && !raw.contains(r"\u")))
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let Some((ability, ast)) = printed(text) else {
        return if cap { Err(INVALID.into()) } else { Ok(()) };
    };
    let type_line = source["type_line"].as_str().ok_or(INVALID)?;
    let (head, subtypes) = type_line.split_once(" — ").unwrap_or((type_line, ""));
    let expected_type = match (c.legendary, c.additional_types.as_slice()) {
        (false, []) => "Creature",
        (true, []) => "Legendary Creature",
        (false, [CardKind::Artifact]) => "Artifact Creature",
        _ => return Err(INVALID.into()),
    };
    if !cap
        || c.kind != CardKind::Creature
        || c.basic
        || c.snow
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || d.static_abilities != [ability]
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.triggered.is_empty()
        || source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["power"].as_str().map(str::to_owned) != c.power.as_ref().map(ToString::to_string)
        || source["toughness"].as_str().map(str::to_owned)
            != c.toughness.as_ref().map(ToString::to_string)
        || source["keywords"] != json!([])
        || head != expected_type
        || !subtypes
            .split_whitespace()
            .eq(c.subtypes.iter().map(String::as_str))
        || strict_read::<Vec<Value>>(&d.provenance.abilities_json)? != [ast]
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != [text]
                || labels.statics != [0]
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
    use Capability::*;
    let member = "controller_scoped_creature_stats";
    let cap = json!({"contains":{"const":ScopedCreatureStatsV1}});
    let effect = json!({"required":[member],"type":"object"});
    let nonnegative = json!({"pattern":"^(0|[1-9][0-9]*)$"});
    let positive = json!({"pattern":"^[1-9][0-9]*$"});
    schema["$defs"]["StaticEffect"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{(member):{
                "properties":{"power":nonnegative,"toughness":nonnegative,"filter":{"properties":{
                    "subtypes":{"minItems":1,"maxItems":2,"uniqueItems":true,
                        "items":{"enum":SUBTYPES.map(|(name,_)|name)}}
                }}},
                "anyOf":[{"properties":{"power":positive}},{"properties":{"toughness":positive}}],
                "if":{"properties":{"filter":{"properties":{"selector":{"const":"any"}}}}},
                "then":{"properties":{"other":{"const":true}}}
            }}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["static_abilities"],"properties":{
            "static_abilities":{"contains":{"required":["effect"],"properties":{"effect":effect}}}
        }},
        "then":{"properties":{
            "characteristics":{"properties":{"kind":{"const":"creature"},"basic":{"const":false},
                "snow":{"const":false},"additional_types":{"enum":[[],["artifact"]]},"hybrid_mana":{"maxItems":0}}},
            "capabilities":cap,"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "keywords":{"maxItems":0},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "triggered":{"maxItems":0},
            "static_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "effect":effect,"functioning_zone":{"const":"battlefield"}
            }}},
            "source_labels":{"required":["statics"],"properties":{
                "blocks":{"minItems":1,"maxItems":1},"statics":{"const":[0]},"activated":{"maxItems":0},
                "mana":{"maxItems":0},"triggered":{"maxItems":0},"spell_modes":{"maxItems":0}
            }}
        }},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let record: Value = strict_read(include_str!(
        "../../../tests/fixtures/96d0e4dd-6cc2-4349-ac44-785b50f8dd90.json"
    ))
    .unwrap();
    let source = &record["source"];
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Grizzly Bears")
    .unwrap();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics.colors = vec![Color::W];
    d.characteristics.color_identity = vec![Color::W];
    d.characteristics.subtypes = vec!["Cat".into()];
    d.characteristics.power = Some(2.into());
    d.characteristics.toughness = Some(1.into());
    d.characteristics.mana_cost = ManaCost {
        generic: 2.into(),
        colored: vec![Color::W],
    };
    d.static_abilities = vec![printed(source["oracle_text"].as_str().unwrap()).unwrap().0];
    d.capabilities = d.requirements();
    d
}
