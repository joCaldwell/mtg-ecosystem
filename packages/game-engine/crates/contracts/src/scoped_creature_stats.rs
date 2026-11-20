//! Fixed controller-side additive stats at layer 7c (CR 205.3m, 208.3,
//! 208.3a, 611.3a/b and 613.4c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires a complete printed controller-side scoped creature stat modifier";
// The frozen, reviewed subset of CR 205.3m creature types and their printed
// plurals. A type may occur in either the explicit creature intersection or
// the standalone subtype selector.
const SUBTYPES: [(&str, &str); 19] = [
    ("Cat", "Cats"),
    ("Dinosaur", "Dinosaurs"),
    ("Dragon", "Dragons"),
    ("Eldrazi", "Eldrazi"),
    ("Elf", "Elves"),
    ("Kithkin", "Kithkin"),
    ("Knight", "Knights"),
    ("Kobold", "Kobolds"),
    ("Merfolk", "Merfolk"),
    ("Pirate", "Pirates"),
    ("Sliver", "Slivers"),
    ("Soldier", "Soldiers"),
    ("Spawn", "Spawn"),
    ("Spirit", "Spirits"),
    ("Squirrel", "Squirrels"),
    ("Treefolk", "Treefolk"),
    ("Vampire", "Vampires"),
    ("Warrior", "Warriors"),
    ("Zombie", "Zombies"),
];

fn stat(word: &str) -> Option<(i64, Value)> {
    let (multiplier, digits) = if let Some(digits) = word.strip_prefix('+') {
        (1, digits)
    } else {
        (-1, word.strip_prefix('-')?)
    };
    let value = digits.parse::<u32>().ok()?;
    (value.to_string() == digits).then(|| {
        (
            multiplier * i64::from(value),
            json!({"sign":multiplier,"amount":{"amount":"fixed","value":value}}),
        )
    })
}

/// Derive the executable modifier and exact archive AST from the whole printed
/// sentence. An omitted creature qualifier is retained in the expected AST.
pub fn printed(text: &str) -> Option<(StaticAbility, Value)> {
    let (subject, magnitude) = text.strip_suffix('.')?.split_once(" you control get ")?;
    let (power, toughness) = magnitude.split_once('/')?;
    let (power, printed_power) = stat(power)?;
    let (toughness, printed_toughness) = stat(toughness)?;
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
        // The equal positive anthem retains its longstanding wire shape.
        "Creatures" if !other && (power < 0 || toughness < 0) => {
            ast_filter["types"] = json!(["creature"]);
            ControllerCreatureStatFilter::Any
        }
        "artifact creatures" | "Artifact creatures" => {
            ast_filter["types"] = json!(["artifact", "creature"]);
            ControllerCreatureStatFilter::Artifact
        }
        "Creatures" => return None,
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
    Some((
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControllerScopedCreatureStats {
                filter,
                other,
                power: power.into(),
                toughness: toughness.into(),
            },
        },
        json!({"kind":"static","effect":{"static":"modify","what":{"ref":"each","filter":ast_filter},
            "modifiers":[{"modifier":"pt","power":printed_power,"toughness":printed_toughness}]}}),
    ))
}

const PLAIN_KEYWORDS: [KeywordAbility; 12] = [
    KeywordAbility::Flash,
    KeywordAbility::Flying,
    KeywordAbility::Vigilance,
    KeywordAbility::Reach,
    KeywordAbility::Trample,
    KeywordAbility::Haste,
    KeywordAbility::Lifelink,
    KeywordAbility::Deathtouch,
    KeywordAbility::FirstStrike,
    KeywordAbility::DoubleStrike,
    KeywordAbility::Defender,
    KeywordAbility::Indestructible,
];

// This profile consumes every source block. Simple intrinsic keyword blocks
// retain their ordinary IR. No unknown or parameterized keyword is inferred.
pub fn printed_keywords(line: &str) -> Option<(Vec<KeywordAbility>, Value)> {
    let line = line
        .strip_suffix(" (You may cast this spell any time you could cast an instant.)")
        .filter(|line| *line == "Flash")
        .unwrap_or(line);
    let mut keywords = Vec::new();
    let mut ast = Vec::new();
    for part in line.split(", ") {
        let word = part.to_ascii_lowercase();
        let keyword: KeywordAbility = serde_json::from_value(json!(word)).ok()?;
        if !PLAIN_KEYWORDS.contains(&keyword) {
            return None;
        }
        keywords.push(keyword);
        ast.push(json!({"keyword":word}));
    }
    Some((keywords, json!({"kind":"keywords","keywords":ast})))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let cap = d.capabilities.contains(&Capability::ScopedCreatureStatsV1);
    let c = &d.characteristics;
    let raw = &d.provenance.source_json;
    if !cap && !raw.contains(" you control get ") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    if !cap
        && !text
            .lines()
            .any(|line| printed(line).is_some() && scoped_static::printed(line).is_none())
    {
        return Ok(());
    }
    let type_line = source["type_line"].as_str().ok_or(INVALID)?;
    let (head, subtypes) = type_line.split_once(" — ").unwrap_or((type_line, ""));
    let expected_type = match (c.kind, c.legendary, c.additional_types.as_slice()) {
        (CardKind::Creature, false, []) => "Creature",
        (CardKind::Creature, true, []) => "Legendary Creature",
        (CardKind::Creature, false, [CardKind::Artifact]) => "Artifact Creature",
        (CardKind::Creature, true, [CardKind::Artifact]) => "Legendary Artifact Creature",
        (CardKind::Enchantment, false, []) => "Enchantment",
        (CardKind::Enchantment, true, []) => "Legendary Enchantment",
        (CardKind::Artifact, false, []) => "Artifact",
        (CardKind::Artifact, true, []) => "Legendary Artifact",
        _ => return Err(INVALID.into()),
    };
    let mut abilities = Vec::new();
    let mut keywords = Vec::new();
    let mut ast = Vec::new();
    let mut labels = Vec::new();
    for (block, line) in text.lines().enumerate() {
        if let Some((ability, expected)) =
            printed(line).filter(|_| scoped_static::printed(line).is_none())
        {
            abilities.push(ability);
            labels.push(u32::try_from(block).map_err(|_| INVALID)?);
            ast.push(expected);
        } else if let Some((occurrences, expected)) = printed_keywords(line) {
            keywords.extend(occurrences);
            ast.push(expected);
        } else {
            return Err(INVALID.into());
        }
    }
    if !cap
        || abilities.is_empty()
        || c.basic
        || c.snow
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || d.static_abilities != abilities
        || d.keywords != keywords
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
        || head != expected_type
        || !subtypes
            .split_whitespace()
            .eq(c.subtypes.iter().map(String::as_str))
        || strict_read::<Vec<Value>>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|source_labels| {
            source_labels.text != text
                || !source_labels
                    .blocks
                    .iter()
                    .map(String::as_str)
                    .eq(text.lines())
                || source_labels.statics != labels
                || !source_labels.activated.is_empty()
                || !source_labels.triggered.is_empty()
                || !source_labels.mana.is_empty()
                || !source_labels.spell_modes.is_empty()
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
    let signed = json!({"pattern":"^(0|-?[1-9][0-9]*)$"});
    let nonzero = json!({"pattern":"^-?[1-9][0-9]*$"});
    schema["$defs"]["StaticEffect"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{(member):{
                "properties":{"power":signed,"toughness":signed,"filter":{"properties":{
                    "subtypes":{"minItems":1,"maxItems":2,"uniqueItems":true,
                        "items":{"enum":SUBTYPES.map(|(name,_)|name)}}
                }}},
                "anyOf":[{"properties":{"power":nonzero}},{"properties":{"toughness":nonzero}}],
                "if":{"properties":{"filter":{"properties":{"selector":{"const":"any"}}},
                    "other":{"const":false}}},
                "then":{"anyOf":[
                    {"properties":{"power":{"pattern":"^-[1-9][0-9]*$"}}},
                    {"properties":{"toughness":{"pattern":"^-[1-9][0-9]*$"}}}
                ]}

            }}
        }));
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["static_abilities"],"properties":{
            "static_abilities":{"contains":{"required":["effect"],"properties":{"effect":effect}}}
        }},
        "then":{"properties":{
            "characteristics":{"properties":{"kind":{"enum":["creature","artifact","enchantment"]},"basic":{"const":false},
                "snow":{"const":false},"additional_types":{"enum":[[],["artifact"]]},"hybrid_mana":{"maxItems":0}}},
            "capabilities":cap,"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "keywords":{"items":{"enum":PLAIN_KEYWORDS}},
            "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "triggered":{"maxItems":0},
            "static_abilities":{"minItems":1,"items":{"properties":{
                "effect":effect,"functioning_zone":{"const":"battlefield"}
            }}},
            "source_labels":{"required":["statics"],"properties":{
                "blocks":{"minItems":1},"statics":{"minItems":1,"uniqueItems":true},"activated":{"maxItems":0},
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
