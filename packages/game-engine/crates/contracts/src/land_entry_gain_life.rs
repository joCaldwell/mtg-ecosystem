//! CR 109.5, 119.3, 603.6a and 608.2c: the exact entrant observes its own
//! entry, then the trigger's controller gains the fixed life amount.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete mandatory controller life gain on land entry";
const TAPPED_LINE: &str = "This land enters tapped.";
const GAIN_PREFIX: &str = "When this land enters, you gain ";

pub(super) fn present(definition: &Definition) -> bool {
    definition.characteristics.kind == CardKind::Land
        && definition.triggered.iter().any(|trigger| {
            trigger.condition == TriggerCondition::SelfEntersBattlefield
                && trigger
                    .program
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, Instruction::GainLife { .. }))
        })
}

fn enters_tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if !present(d) && !d.capabilities.contains(&Capability::LandEntryGainLifeV1) {
        return Ok(());
    }
    let c = &d.characteristics;
    let [trigger] = d.triggered.as_slice() else {
        return Err(INVALID.into());
    };
    let [mana] = d.mana_abilities.as_slice() else {
        return Err(INVALID.into());
    };
    let tapped = match d.static_abilities.as_slice() {
        [] => false,
        [entry] if *entry == enters_tapped() => true,
        _ => return Err(INVALID.into()),
    };
    let block = u32::from(tapped);
    let program = &trigger.program;
    let [
        Instruction::GainLife {
            recipient: LifeRecipient::Controller,
            amount,
            source_block,
        },
    ] = program.instructions.as_slice()
    else {
        return Err(INVALID.into());
    };
    let amount: i64 = amount
        .to_string()
        .parse()
        .ok()
        .filter(|n| *n > 0)
        .ok_or(INVALID)?;
    if c.kind != CardKind::Land
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !c.colors.is_empty()
        || !d.activated.is_empty()
        || !d.keywords.is_empty()
        || trigger.condition != TriggerCondition::SelfEntersBattlefield
        || trigger.functioning_zone != FunctioningZone::Battlefield
        || trigger.intervening_if.is_some()
        || trigger.death_filter.is_some()
        || program.functioning_zone != FunctioningZone::Stack
        || program.timing != SpellTiming::Instant
        || program.resolution != SpellResolution::Instructions
        || !program.targets.is_empty()
        || !program.modes.is_empty()
        || !program.optional_additional_costs.is_empty()
        || program.additional_sacrifice.is_some()
        || *source_block != block
        || mana.source_block != block + 1
        || mana.costs
            != (PermanentActivationCosts {
                remove_source_plus_one_counters: None,
                tap: true,
                mana: None,
                sacrifice: false,
            })
        || mana.self_damage.is_some()
        || mana.restriction.is_some()
    {
        return Err(INVALID.into());
    }

    let (production, printed, colors) = match &mana.production {
        ManaProduction::Fixed { symbols } if symbols.len() == 1 => (
            json!({"mana":"fixed","symbols":symbols}),
            format!("{{{:?}}}", symbols[0]),
            symbols,
        ),
        ManaProduction::Choice { colors } => {
            listed_mana_choice::validate(d)?;
            let symbols: Vec<_> = colors.iter().map(|c| format!("{{{c:?}}}")).collect();
            let printed = if let [first, second] = symbols.as_slice() {
                format!("{first} or {second}")
            } else {
                let (last, earlier) = symbols.split_last().expect("validated listed colors");
                format!("{}, or {last}", earlier.join(", "))
            };
            let options: Vec<_> = colors
                .iter()
                .map(|color| json!({"mana":"fixed","symbols":[color]}))
                .collect();
            (json!({"mana":"choice","options":options}), printed, colors)
        }
        _ => return Err(INVALID.into()),
    };

    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or(INVALID)?;
    let lines: Vec<_> = text.lines().collect();
    let expected_mana = format!("{{T}}: Add {printed}.");
    let printed_amount = lines
        .get(block as usize)
        .and_then(|line| line.strip_prefix(GAIN_PREFIX))
        .and_then(|line| line.strip_suffix(" life."))
        .ok_or(INVALID)?;
    let production_set: BTreeSet<_> = colors.iter().copied().collect();
    let color_identity: Vec<_> = colors.iter().copied().filter(|c| *c != Color::C).collect();
    let produced_mana: Vec<Color> =
        serde_json::from_value(source["produced_mana"].clone()).map_err(|_| INVALID)?;
    if lines.join("\n") != text
        || lines.len() != (block + 2) as usize
        || (tapped && lines[0] != TAPPED_LINE)
        || !matches_fixed_amount(printed_amount, amount)
        || lines[(block + 1) as usize] != expected_mana
        || source["type_line"] != "Land"
        || source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != ""
        || source["keywords"] != json!([])
        || source["colors"] != json!([])
        || source["color_identity"] != json!(c.color_identity)
        || produced_mana.len() != colors.len()
        || produced_mana.into_iter().collect::<BTreeSet<_>>() != production_set
        || c.color_identity.len() != color_identity.len()
        || c.color_identity.iter().collect::<BTreeSet<_>>() != color_identity.iter().collect()
    {
        return Err(INVALID.into());
    }

    let subject = json!({"ref":"this","noun":"land"});
    let you = json!({"player":"you"});
    let mut ast = Vec::new();
    if tapped {
        ast.push(json!({"kind":"static","effect":{"static":"enters-tapped","what":subject}}));
    }
    ast.extend([
        json!({"kind":"triggered","trigger":{"trigger":"enters","what":subject},
            "effects":[{"steps":[{"effect":"gain-life","who":you,"amount":{"amount":"fixed","value":amount}}]}]}),
        json!({"kind":"activated","costs":[{"cost":"tap-self"}],
            "effects":[{"steps":[{"effect":"add-mana","who":you,"mana":production}]}]}),
    ]);
    if strict_read::<Vec<Value>>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != lines
                || labels.mana != [block + 1]
                || labels.triggered != [block]
                || !labels.statics.iter().copied().eq(tapped.then_some(0))
                || !labels.activated.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;

    let capability = json!({"contains":{"const":LandEntryGainLifeV1}});
    let condition = json!({
        "required":["condition","program"],
        "properties":{
            "condition":{"const":"self_enters_battlefield"},
            "program":{"required":["instructions"],"properties":{
                "instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"gain_life"}}}}
            }}
        }
    });
    let branch = |tapped: bool| {
        let block = u32::from(tapped);
        let static_abilities = if tapped {
            json!({"const":[enters_tapped()]})
        } else {
            json!({"maxItems":0})
        };
        let required_static_capability = json!({"contains":{"const":EntersTappedV1}});
        let static_capability = if tapped {
            required_static_capability
        } else {
            json!({"not":required_static_capability})
        };
        json!({"required":tapped.then_some("static_abilities").into_iter().collect::<Vec<_>>(),"properties":{
            "triggered":{"items":{"properties":{"program":{"properties":{
                "instructions":{"items":{"properties":{"source_block":{"const":block}}}}
            }}}}},
            "static_abilities":static_abilities,
            "capabilities":static_capability,
            "mana_abilities":{"items":{"properties":{"source_block":{"const":block+1}}}},
            "source_labels":{"properties":{
                "mana":{"const":[block+1]},
                "triggered":{"const":[block]},
                "statics":{"const":tapped.then_some(0).into_iter().collect::<Vec<_>>()},
                "blocks":{"minItems":block+2,"maxItems":block+2},
                "activated":{"maxItems":0},
                "spell_modes":{"maxItems":0}
            }}
        }})
    };
    let triggered = json!({"minItems":1,"maxItems":1,"items":{"properties":{
        "condition":{"const":"self_enters_battlefield"},
        "functioning_zone":{"const":"battlefield"},"intervening_if":{"type":"null"},"death_filter":{"type":"null"},
        "program":{"properties":{
            "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
            "targets":{"maxItems":0},"modes":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{
                "op":{"const":"gain_life"},"recipient":{"const":"controller"},
                "amount":{"pattern":"^[1-9][0-9]*$"}
            }}}
        }}
    }}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"properties":{"capabilities":capability}},
            {"required":["characteristics","triggered"],"properties":{
                "characteristics":{"properties":{"kind":{"const":"land"}}},
                "triggered":{"contains":condition}
            }}
        ]},
        "then":{
            "required":["triggered","mana_abilities","intrinsic_zone"],
            "oneOf":[branch(false),branch(true)],
            "properties":{
                "characteristics":{"properties":{
                    "kind":{"const":"land"},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                    "additional_types":{"maxItems":0},"subtypes":{"maxItems":0},"colors":{"maxItems":0}
                }},
                "x_mana_cost":{"const":false},"intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
                "activated":{"maxItems":0},"keywords":{"maxItems":0},
                "capabilities":{"allOf":[capability,{"contains":{"const":LifeGainV1}},
                    {"contains":{"const":SelfEntersBattlefieldV1}},{"contains":{"const":TriggeredAbilityV1}}]},
                "triggered":triggered,
                "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                    "self_damage":{"type":"null"},"restriction":{"type":"null"},
                    "costs":{"properties":{
                        "tap":{"const":true},"sacrifice":{"const":false},"mana":{"type":"null"},
                        "remove_source_plus_one_counters":{"type":"null"}
                    }},
                    "production":{"oneOf":[
                        {"properties":{"mana":{"const":"fixed"},"symbols":{"minItems":1,"maxItems":1}}},
                        {"properties":{"mana":{"const":"choice"}}}
                    ]}
                }}}
            }
        }
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let raw = include_str!("../../../tests/fixtures/5d641bf6-0f93-4189-8dc1-ec7ea446dade.json");
    let record: Value = strict_read(raw).unwrap();
    let source = &record["source"];
    let mut d = listed_mana_choice::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
    d.characteristics.color_identity = vec![Color::U, Color::W];
    d.mana_abilities[0].source_block = 2;
    d.static_abilities = vec![enters_tapped()];
    d.triggered = vec![TriggeredAbility {
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
            instructions: vec![Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: 1.into(),
                source_block: 1,
            }],
            modes: vec![],
        },
    }];
    d.capabilities = d.requirements();
    d
}
