//! Source creature pays fixed mana and taps to tap one targeted creature.
//! CR 302.6, 602.2b, 608.2b, 701.26a and 701.26b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete fixed-mana and source-tap activation to tap one creature";

/// Rule binding for the reviewed intrinsic keyword siblings.
pub fn keyword(k: KeywordAbility) -> Option<(&'static str, &'static str)> {
    match k {
        KeywordAbility::Defender => Some(("defender", "702.3")),
        KeywordAbility::Flying | KeywordAbility::Vigilance => fixed_activation_keyword(k),
        _ => None,
    }
}

fn parse_ability_word(line: &str) -> Result<(&str, Option<&str>), String> {
    if let Some((word, rest)) = line.split_once(" — ") {
        if word
            .split(' ')
            .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_alphabetic()))
        {
            return Ok((rest, Some(word)));
        }
        return Err(INVALID.into());
    }
    Ok((line, None))
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    if !d.capabilities.contains(&Capability::ActivatedCreatureTapV1) {
        return Ok(false);
    }
    let c = &d.characteristics;
    let [ability] = d.activated.as_slice() else {
        return Err(INVALID.into());
    };
    let ActivationCost::Composite {
        costs:
            PermanentActivationCosts {
                remove_source_plus_one_counters: None,
                tap: true,
                sacrifice: false,
                mana: Some(cost),
            },
    } = &ability.cost
    else {
        return Err(INVALID.into());
    };
    let block = u32::from(!d.keywords.is_empty());
    if c.kind != CardKind::Creature
        || !matches!(c.additional_types.as_slice(), [] | [CardKind::Artifact])
        || c.basic
        || c.legendary
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.triggered.is_empty()
        || d.intrinsic_zone.is_some()
        || !ordinary_tap_program(&ability.program)
        || ability.program.timing != SpellTiming::Instant
        || ability.program.instructions
            != [Instruction::TapPermanent {
                target_slot: 0,
                source_block: block,
            }]
        || ability.functioning_zone != FunctioningZone::Battlefield
        || cost.generic.is_negative()
        || (cost.generic == Quantity::zero() && cost.colored.is_empty())
        || cost.colored.contains(&Color::C)
    {
        return Err(INVALID.into());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or(INVALID)?;
    let lines: Vec<_> = text.lines().collect();
    if lines.len() != block as usize + 1 || lines.join("\n") != text {
        return Err(INVALID.into());
    }
    let (activation, word) = parse_ability_word(lines[block as usize])?;
    let cost_text = cost.printed_fixed_cost();
    if activation != format!("{cost_text}, {{T}}: Tap target creature.") {
        return Err(INVALID.into());
    }
    let symbols: Vec<_> = cost_text.trim_matches(['{', '}']).split("}{").collect();
    let prefix = word.map_or(0, |word| word.split(' ').count() + 1);
    let selection_id = format!("line:{block}:{}", symbols.len() + 4 + prefix);
    if ability.program.targets
        != [TargetSpec {
            slot: 0,
            domain: TargetDomain::BattlefieldCreature,
            filter: None,
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            selection_id: selection_id.clone(),
        }]
    {
        return Err(INVALID.into());
    }
    let what = json!({"ref":"target","filter":{"types":["creature"]},
        "count":{"amount":"fixed","value":1},"selectionId":selection_id});
    let mut active = json!({"kind":"activated","costs":[{"cost":"mana","symbols":symbols},
        {"cost":"tap-self"}],"effects":[{"steps":[{"effect":"tap","what":what}]}]});
    if let Some(word) = word {
        active["abilityWord"] = json!(word);
    }
    let mut ast = Vec::new();
    let mut names = Vec::new();
    for k in &d.keywords {
        let (name, _) = keyword(*k).ok_or(INVALID)?;
        if names.contains(&name) {
            return Err(INVALID.into());
        }
        names.push(name);
    }
    if !names.is_empty() {
        let mut printed = names.join(", ");
        printed[..1].make_ascii_uppercase();
        if lines[0] != printed {
            return Err(INVALID.into());
        }
        ast.push(json!({"kind":"keywords","keywords":names.iter().map(|name|json!({"keyword":name})).collect::<Vec<_>>() }));
    }
    ast.push(active);
    let metadata = source["keywords"].as_array().ok_or(INVALID)?;
    let type_line = source["type_line"].as_str().ok_or(INVALID)?;
    let (head, tail) = type_line.split_once(" — ").unwrap_or((type_line, ""));
    let expected_head = match (c.snow, c.additional_types.as_slice()) {
        (false, []) => "Creature",
        (false, [CardKind::Artifact]) => "Artifact Creature",
        (true, []) => "Snow Creature",
        _ => return Err(INVALID.into()),
    };
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
        || metadata.len() != names.len()
        || names.iter().any(|word| {
            !metadata
                .iter()
                .any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(word)))
        })
        || head != expected_head
        || !tail
            .split_whitespace()
            .eq(c.subtypes.iter().map(String::as_str))
        || strict_read::<Vec<Value>>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != lines
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

pub(super) fn extend_schema(schema: &mut Value, tap: &Value) {
    use Capability::*;

    schema["$defs"]["ActivatedCreatureTapAbility"] = json!({
        "required":["program","cost","functioning_zone"],
        "properties":{
            "functioning_zone":{"const":"battlefield"},
            "program":{"allOf":[{"$ref":"#/$defs/OrdinaryTapProgram"},{"properties":{
                "timing":{"const":"instant"},"instructions":{"minItems":1,"maxItems":1}
            }}]},
            "cost":{"type":"object","required":["composite"],"properties":{
                "composite":{"properties":{"costs":{"required":["mana"],"properties":{
                    "tap":{"const":true},"sacrifice":{"const":false},
                    "remove_source_plus_one_counters":{"type":"null"},
                    "mana":{"type":"object","properties":{
                        "generic":{"pattern":"^(0|[1-9][0-9]*)$"},
                        "colored":{"items":{"enum":["W","U","B","R","G"]}}
                    },"anyOf":[{"properties":{"generic":{"pattern":"^[1-9][0-9]*$"}}},
                        {"properties":{"colored":{"minItems":1}}}]}
                }}}}
            }}
        }
    });
    let capability = json!({"contains":{"const":ActivatedCreatureTapV1}});
    schema["$defs"]["ActivatedCreatureTapDefinition"] = json!({
        "required":["activated"],"properties":{
            "characteristics":{"properties":{
                "kind":{"const":"creature"},"basic":{"const":false},"legendary":{"const":false},
                "additional_types":{"enum":[[],["artifact"]]},"hybrid_mana":{"maxItems":0}
            }},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "capabilities":{"allOf":[capability,{"contains":{"const":TapPermanentV1}},
                {"contains":{"const":ActivatedAbilityV1}},{"contains":{"const":CompositeActivationCostV1}},
                {"not":{"contains":{"const":ActivatedTapPermanentV1}}}]},
            "keywords":{"uniqueItems":true,"items":{"enum":["defender","flying","vigilance"]}},
            "static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"triggered":{"maxItems":0},
            "activated":{"minItems":1,"maxItems":1,"items":{"$ref":"#/$defs/ActivatedCreatureTapAbility"}}
        }
    });
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["capabilities"],"properties":{"capabilities":capability}},
            "then":{"allOf":[{"$ref":"#/$defs/ActivatedCreatureTapDefinition"},{"properties":{
                "activated":{"contains":{"properties":{"program":{"properties":tap}}}}
            }}]}
        }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let r: Value = strict_read(include_str!(
        "../../../tests/fixtures/00d11662-9d10-4915-b80c-c1ad54b4e71b.json"
    ))
    .unwrap();
    let source = &r["source"];
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
    d.provenance.abilities_json = canonical(&r["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = r["bindingSha256"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics.colors = vec![Color::W];
    d.characteristics.color_identity = vec![Color::W];
    d.characteristics.subtypes = vec!["Human".into(), "Elf".into(), "Monk".into()];
    d.characteristics.power = Some(1.into());
    d.characteristics.toughness = Some(4.into());
    d.characteristics.mana_cost = ManaCost {
        generic: 3.into(),
        colored: vec![Color::W],
    };
    d.keywords = vec![KeywordAbility::Vigilance];
    d.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: PermanentActivationCosts {
                tap: true,
                sacrifice: false,
                mana: Some(ManaCost {
                    generic: 1.into(),
                    colored: vec![Color::W],
                }),
                remove_source_plus_one_counters: None,
            },
        },
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            targets: vec![TargetSpec {
                slot: 0,
                domain: TargetDomain::BattlefieldCreature,
                filter: None,
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                selection_id: "line:1:9".into(),
            }],
            instructions: vec![Instruction::TapPermanent {
                target_slot: 0,
                source_block: 1,
            }],
            modes: vec![],
        },
    }];
    d.capabilities = d.requirements();
    d
}
