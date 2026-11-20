//! CR 109.5, 201.5, 614.8, 701.19a: one paid ability establishes a
//! destruction replacement for its own incarnation.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete fixed-mana regeneration of this creature";
const REGENERATION_REMINDER: &str = " (The next time this creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)";

/// Remove the exact reminder used by the frozen printings.
pub fn activation_line(line: &str) -> &str {
    line.strip_suffix(REGENERATION_REMINDER).unwrap_or(line)
}

/// Independently supported intrinsic keyword and its archive binding rule.
pub fn keyword(k: KeywordAbility) -> Option<(&'static str, &'static str)> {
    fixed_creature_activation::intrinsic_keyword(k)
}

/// Prove each printed keyword block before the final regeneration activation.
pub fn source_keywords(text: &str, raw_ast: &str) -> Result<Vec<Vec<KeywordAbility>>, String> {
    let ast: Vec<Value> = strict_read(raw_ast)?;
    let lines: Vec<_> = text.lines().collect();
    if lines.is_empty() || lines.len() != ast.len() || lines.join("\n") != text {
        return Err(INVALID.into());
    }
    let mut seen = Vec::new();
    lines[..lines.len() - 1]
        .iter()
        .zip(&ast)
        .map(|(line, actual)| {
            let (words, reminder) = line
                .split_once(" (")
                .map_or((*line, None), |(words, remainder)| {
                    (words, Some(format!("({remainder}")))
                });
            let mut keywords = Vec::new();
            let mut expected = Vec::new();
            for word in words.split(", ") {
                let parsed: KeywordAbility =
                    serde_json::from_value(json!(word.to_ascii_lowercase()))
                        .map_err(|_| INVALID)?;
                let (canonical_word, _) = keyword(parsed).ok_or(INVALID)?;
                if seen.contains(&parsed) {
                    return Err(INVALID.into());
                }
                seen.push(parsed);
                keywords.push(parsed);
                expected.push(json!({"keyword":canonical_word}));
            }
            let mut printed = expected
                .iter()
                .map(|k| k["keyword"].as_str().expect("known keyword"))
                .collect::<Vec<_>>()
                .join(", ");
            printed[..1].make_ascii_uppercase();
            let reminder_valid = match (keywords.as_slice(), reminder.as_deref()) {
                (_, None) => true,
                ([k], Some(text)) => fixed_creature_activation::keyword_reminder(*k) == Some(text),
                _ => false,
            };
            if words != printed
                || !reminder_valid
                || *actual != json!({"kind":"keywords","keywords":expected})
            {
                return Err(INVALID.into());
            }
            Ok(keywords)
        })
        .collect()
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    if !d.capabilities.contains(&Capability::SourceRegenerationV1) {
        return Ok(false);
    }
    let c = &d.characteristics;
    let [ability] = d.activated.as_slice() else {
        return Err(INVALID.into());
    };
    if c.kind != CardKind::Creature
        || !matches!(c.additional_types.as_slice(), [] | [CardKind::Artifact])
        || c.basic
        || c.snow
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.triggered.is_empty()
        || d.intrinsic_zone.is_some()
    {
        return Err(INVALID.into());
    }

    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or(INVALID)?;
    let keyword_blocks = source_keywords(text, &d.provenance.abilities_json)?;
    let block = u32::try_from(keyword_blocks.len()).map_err(|_| INVALID)?;
    let spec = fixed_creature_activation(
        activation_line(text.lines().last().ok_or(INVALID)?),
        &d.name,
        block,
    )
    .ok_or(INVALID)?;
    if !matches!(
        spec.ability.program.instructions.as_slice(),
        [Instruction::RegenerateSource { .. }]
    ) || *ability != spec.ability
        || !d
            .keywords
            .iter()
            .copied()
            .eq(keyword_blocks.iter().flatten().copied())
        || strict_read::<Vec<Value>>(&d.provenance.abilities_json)?.last() != Some(&spec.ast)
    {
        return Err(INVALID.into());
    }

    let mut types = Vec::new();
    if c.legendary {
        types.push("Legendary");
    }
    if c.additional_types == [CardKind::Artifact] {
        types.push("Artifact");
    }
    types.push("Creature");
    let type_line = source["type_line"].as_str().ok_or(INVALID)?;
    let (head, subtypes) = type_line.split_once(" — ").unwrap_or((type_line, ""));
    let source_keywords = source["keywords"].as_array().ok_or(INVALID)?;
    let has_keyword = |word: &str| {
        source_keywords
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(word)))
    };
    // The archive's keyword inventory also enumerates the keyword action
    // itself and the historical "heal" wording in its reminder.
    let has_reminder = text.ends_with(REGENERATION_REMINDER);
    if source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["power"].as_str().map(str::to_owned) != c.power.as_ref().map(ToString::to_string)
        || source["toughness"].as_str().map(str::to_owned)
            != c.toughness.as_ref().map(ToString::to_string)
        || head != types.join(" ")
        || !subtypes
            .split_whitespace()
            .eq(c.subtypes.iter().map(String::as_str))
        || source_keywords.len() != d.keywords.len() + 1 + usize::from(has_reminder)
        || !has_keyword("regenerate")
        || has_keyword("heal") != has_reminder
        || !d
            .keywords
            .iter()
            .all(|k| keyword(*k).is_some_and(|(word, _)| has_keyword(word)))
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks.len() != keyword_blocks.len() + 1
                || labels.blocks.last() != Some(&spec.label)
                || !labels.blocks[..keyword_blocks.len()]
                    .iter()
                    .zip(text.lines())
                    .all(|(label, line)| {
                        label == line.split_once(" (").map_or(line, |(words, _)| words)
                    })
                || labels.activated != [block]
                || !labels.triggered.is_empty()
                || !labels.statics.is_empty()
                || !labels.mana.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(true)
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let operation = json!({"required":["op"],"properties":{"op":{"const":"regenerate_source"}}});
    let has = json!({"required":["program"],"properties":{"program":{
        "required":["instructions"],"properties":{"instructions":{"contains":operation}}
    }}});
    let empty = json!({"maxItems":0});
    schema["$defs"]["NoRegenerationProgram"] = json!({"properties":{
        "instructions":{"not":{"contains":operation}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/NoRegenerationProgram"}}}}
    }});
    let no = json!({"$ref":"#/$defs/NoRegenerationProgram"});
    let capability = json!({"contains":{"const":SourceRegenerationV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "spell":no,
            "triggered":{"items":{"properties":{"program":no}}},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{
                "grant_spell_cascade":{"properties":{"program":no}}
            }}}}}
        },
        "if":{"required":["activated"],"properties":{"activated":{"contains":has}}},
        "then":{"properties":{
            "characteristics":{"properties":{
                "kind":{"const":"creature"},"basic":{"const":false},"snow":{"const":false},
                "hybrid_mana":empty,"additional_types":{"enum":[[],["artifact"]]}
            }},
            "capabilities":{"allOf":[capability,{"contains":{"const":ActivatedAbilityV1}},
                {"contains":{"const":ManaActivationCostV1}}]},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "triggered":empty,"static_abilities":empty,"mana_abilities":empty,
            "keywords":{"uniqueItems":true,"items":{"enum":[
                "flying","reach","trample","vigilance","defender","first strike","double strike",
                "flash","haste","menace","lifelink","deathtouch","hexproof","shroud","indestructible"
            ]}},
            "activated":{"minItems":1,"maxItems":1,"items":{"properties":{
                "functioning_zone":{"const":"battlefield"},
                "cost":{"type":"object","required":["mana"],"properties":{"mana":{"properties":{
                    "cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}},"anyOf":[
                        {"properties":{"generic":{"pattern":"^[1-9][0-9]*$"}}},
                        {"properties":{"colored":{"minItems":1}}}
                    ]}
                }}}},
                "program":{"properties":{
                    "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},
                    "resolution":{"const":"instructions"},"instructions":{"minItems":1,"maxItems":1,"items":operation},
                    "targets":empty,"modes":empty,"optional_additional_costs":empty,"additional_sacrifice":{"type":"null"}
                }}
            }}}
        }},
        "else":{"properties":{"capabilities":{"not":capability},
            "activated":{"items":{"properties":{"program":no}}}
        }}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let raw = include_str!("../../../tests/fixtures/8661d773-3b92-4fcf-8ffc-aced1ff7be8a.json");
    let record: Value = strict_read(raw).unwrap();
    let source = &record["source"];
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-four-colors-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Grizzly Bears")
    .unwrap();
    d.source_labels = None;
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
    let c = &mut d.characteristics;
    c.subtypes = vec!["Troll".into()];
    c.color_identity = vec![Color::B, Color::G];
    c.colors = c.color_identity.clone();
    c.mana_cost = ManaCost {
        generic: 1.into(),
        colored: vec![Color::B, Color::G],
    };
    c.power = Some(1.into());
    c.toughness = Some(4.into());
    d.keywords = vec![KeywordAbility::Defender, KeywordAbility::Reach];
    d.activated = vec![
        fixed_creature_activation("{1}{B}{G}: Regenerate this creature.", &d.name, 2)
            .unwrap()
            .ability,
    ];
    d.capabilities = d.requirements();
    d
}
