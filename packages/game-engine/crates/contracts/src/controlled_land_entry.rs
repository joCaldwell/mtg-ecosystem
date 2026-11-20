//! CR 109.5, 603.2c, 603.3a/b, 603.6a and 603.10 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "Whenever a land you control enters, you gain ";
const INVALID: &str = "requires a complete controlled land entry trigger with fixed life gain and optional printed draw instruction";

pub fn controlled_land_query() -> Value {
    json!({"ref":"filter","filter":{"types":["land"],"control":"you"},"count":{"amount":"fixed","value":1}})
}

pub fn controlled_land_entry_trigger(life: Quantity, draw: bool) -> TriggeredAbility {
    let mut instructions = vec![Instruction::GainLife {
        recipient: LifeRecipient::Controller,
        amount: life,
        source_block: 0,
    }];
    if draw {
        instructions.push(Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 0,
        });
    }
    TriggeredAbility {
        condition: TriggerCondition::ControlledLandEntersBattlefield,
        functioning_zone: FunctioningZone::Battlefield,
        death_filter: None,
        intervening_if: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            instructions,
            targets: vec![],
            modes: vec![],
            optional_additional_costs: vec![],
            additional_sacrifice: None,
        },
    }
}

fn permanent_spell() -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        instructions: vec![],
        targets: vec![],
        modes: vec![],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
    }
}

fn printed_clause(text: &str) -> (Option<&str>, &str) {
    text.split_once(" — ")
        .map_or((None, text), |(word, clause)| (Some(word), clause))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if land_reclamation::present(d) {
        return Ok(());
    }
    let raw = &d.provenance.source_json;
    if !d
        .triggered
        .iter()
        .any(|trigger| trigger.condition == TriggerCondition::ControlledLandEntersBattlefield)
        && !d.capabilities.contains(&Capability::ControlledLandEntryV1)
    {
        if raw.contains(PREFIX) || raw.contains(r"\u") {
            let source: Value = strict_read(raw)?;
            if source["oracle_text"]
                .as_str()
                .is_some_and(|text| printed_clause(text).1.starts_with(PREFIX))
            {
                return Err(INVALID.into());
            }
        }
        return Ok(());
    }
    let [trigger] = d.triggered.as_slice() else {
        return Err(INVALID.into());
    };
    let [Instruction::GainLife { amount, .. }, rest @ ..] = trigger.program.instructions.as_slice()
    else {
        return Err(INVALID.into());
    };
    let draw = !rest.is_empty();
    let life = amount
        .to_string()
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or(INVALID)?;
    let c = &d.characteristics;
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or(INVALID)?;
    let (ability_word, clause) = printed_clause(text);
    let printed = clause
        .strip_prefix(PREFIX)
        .and_then(|s| {
            s.strip_suffix(if draw {
                " life and draw a card."
            } else {
                " life."
            })
        })
        .ok_or(INVALID)?;
    let type_line = format!(
        "{}Creature — {}",
        if c.legendary { "Legendary " } else { "" },
        c.subtypes.join(" ")
    );
    if c.kind != CardKind::Creature
        || c.basic
        || c.snow
        || !c.additional_types.is_empty()
        || c.subtypes.is_empty()
        || !c.hybrid_mana.is_empty()
        || c.power.is_none()
        || c.toughness.is_none()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || d.spell.as_ref() != Some(&permanent_spell())
        || *trigger != controlled_land_entry_trigger(amount.clone(), draw)
        || !matches_fixed_amount(printed, life)
        || ability_word.is_some_and(str::is_empty)
        || source["type_line"] != type_line
        || source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || (source["keywords"] != json!([])
            && source["keywords"] != json!(ability_word.into_iter().collect::<Vec<_>>()))
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["power"] != json!(c.power.as_ref().map(ToString::to_string))
        || source["toughness"] != json!(c.toughness.as_ref().map(ToString::to_string))
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
    {
        return Err(INVALID.into());
    }
    let you = json!({"player":"you"});
    let mut steps =
        vec![json!({"effect":"gain-life","who":you,"amount":{"amount":"fixed","value":life}})];
    if draw {
        steps.push(json!({"effect":"draw","who":you,"amount":{"amount":"fixed","value":1}}));
    }
    let mut ability = json!({"kind":"triggered","trigger":{"trigger":"enters","what":controlled_land_query()},
        "effects":[{"steps":steps}]});
    if let Some(word) = ability_word {
        ability["abilityWord"] = json!(word);
    }
    if strict_read::<Value>(&d.provenance.abilities_json)? != json!([ability])
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks.as_slice() != [text]
                || labels.triggered != [0]
                || !labels.mana.is_empty()
                || !labels.activated.is_empty()
                || !labels.statics.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let caps = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        TriggeredAbilityV1,
        ControlledLandEntryV1,
        LifeGainV1,
    ];
    let all_caps = [caps.as_slice(), &[ControllerDrawV1, CommanderIdentityV1]].concat();
    let with_draw = json!({"contains":{"const":ControllerDrawV1}});
    let as_commander = json!({"contains":{"const":CommanderIdentityV1}});
    let primary = json!({"properties":{"op":{"const":"gain_life"},"recipient":{"const":"controller"},
        "amount":{"pattern":"^[1-9][0-9]*$","maxLength":19},"source_block":{"const":0}}});
    let secondary = json!({"const":Instruction::DrawCards { recipient: None, count: 1.into(), source_block: 0 }});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"allOf":[{"not":land_reclamation::selector()},{"anyOf":[
            {"required":["triggered"],"properties":{"triggered":{"contains":{"required":["condition"],
                "properties":{"condition":{"const":"controlled_land_enters_battlefield"}}}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"(?:[^"\\]* — )?Whenever a land you control enters, you gain "#}}}}}
        ]}]},
        "then":{"required":["spell","triggered"],"allOf":[{
            "if":{"properties":{"triggered":{"items":{"properties":{"program":{"properties":{
                "instructions":{"minItems":2}
            }}}}}}},
            "then":{"properties":{"capabilities":with_draw}},
            "else":{"properties":{"capabilities":{"not":with_draw}}}
        },{
            "if":{"properties":{"characteristics":{"required":["legendary"],
                "properties":{"legendary":{"const":true}}}}},
            "then":{"properties":{"capabilities":as_commander}},
            "else":{"properties":{"capabilities":{"not":as_commander}}}
        }],"properties":{
            "capabilities":{"items":{"enum":all_caps},
                "allOf":caps.map(|cap|json!({"contains":{"const":cap}}))},
            "characteristics":{"properties":{"kind":{"const":"creature"},
                "basic":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0},
                "subtypes":{"minItems":1},"hybrid_mana":{"maxItems":0}}},
            "activated":{"maxItems":0},"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
            "mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "spell":{"const":permanent_spell()},
            "triggered":{"minItems":1,"maxItems":1,"items":{"properties":{
                "condition":{"const":"controlled_land_enters_battlefield"},
                "death_filter":{"type":"null"},"intervening_if":{"type":"null"},
                "functioning_zone":{"const":"battlefield"},"program":{"properties":{
                    "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},
                    "resolution":{"const":"instructions"},"targets":{"maxItems":0},
                    "modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},
                    "additional_sacrifice":{"type":"null"},
                    "instructions":{"minItems":1,"maxItems":2,"items":false,"prefixItems":[primary,secondary]}
                }}
            }}}
        }},
        "else":{"if":{"not":land_reclamation::selector()},"then":{"properties":{"capabilities":{"not":{"contains":{"const":ControlledLandEntryV1}}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let mut d = catalog
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    let r: Value = strict_read(include_str!(
        "../../../tests/fixtures/263b526c-8b81-44ee-a7c4-5c24bf9b42a4.json"
    ))
    .unwrap();
    let source = &r["source"];
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json").trim_end();
    let header: Value = strict_read(manifest).unwrap();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = r["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&r["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = r["bindingSha256"].as_str().unwrap().into();
    d.provenance.archive_manifest_sha256 = sha256(manifest.as_bytes());
    d.provenance.parser_sha256 = header["parser"]["sourceSha256"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics.subtypes = vec!["Insect".into()];
    d.characteristics.power = Some(1.into());
    d.characteristics.toughness = Some(3.into());
    d.triggered = vec![controlled_land_entry_trigger(1.into(), false)];
    d.capabilities = d.requirements();
    d
}

#[test]
fn controlled_land_entry_checks_mandatory_source_program() {
    let mut definition = witness();
    definition.validate().unwrap();
    let mut omitted = definition.clone();
    omitted.triggered.clear();
    omitted.capabilities = omitted.requirements();
    assert!(omitted.validate().is_err());
    definition.triggered[0] = controlled_land_entry_trigger(1.into(), true);
    definition.capabilities = definition.requirements();
    assert!(definition.validate().is_err());
}
