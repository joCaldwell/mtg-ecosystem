//! CR 109.5, 121.2, 601.2h and 602.2b: the activated ability uses the
//! composite source payment and draws only on resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str =
    "requires complete tapped dual land with mana, tap and source sacrifice to draw";
const TAPPED_LINE: &str = "This land enters tapped.";

pub(super) fn present(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Land
        && d.activated.iter().any(|a| {
            matches!(&a.cost, ActivationCost::Composite { costs } if costs.tap && costs.sacrifice)
                && a.program
                    .instructions
                    .iter()
                    .any(|i| matches!(i, Instruction::DrawCards { .. }))
        })
}

fn enters_tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    if !present(d) && !d.capabilities.contains(&Capability::LandSacrificeDrawV1) {
        return Ok(false);
    }
    let [ability] = d.activated.as_slice() else {
        return Err(INVALID.into());
    };
    let ActivationCost::Composite {
        costs:
            PermanentActivationCosts {
                tap: true,
                sacrifice: true,
                mana: Some(payment),
                remove_source_plus_one_counters: None,
            },
    } = &ability.cost
    else {
        return Err(INVALID.into());
    };
    let [
        ManaAbility {
            production: ManaProduction::Choice { colors },
            source_block: 1,
            ..
        },
    ] = d.mana_abilities.as_slice()
    else {
        return Err(INVALID.into());
    };
    listed_mana_choice::validate(d)?;
    let [first, second] = colors.as_slice() else {
        return Err(INVALID.into());
    };
    let c = &d.characteristics;
    let p = &ability.program;
    if c.kind != CardKind::Land
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !c.colors.is_empty()
        || !d.keywords.is_empty()
        || !d.triggered.is_empty()
        || d.x_mana_cost
        || d.static_abilities != [enters_tapped()]
        || ability.functioning_zone != FunctioningZone::Battlefield
        || p.functioning_zone != FunctioningZone::Stack
        || p.timing != SpellTiming::Instant
        || p.resolution != SpellResolution::Instructions
        || !p.targets.is_empty()
        || !p.modes.is_empty()
        || !p.optional_additional_costs.is_empty()
        || p.additional_sacrifice.is_some()
        || p.instructions
            != [Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 2,
            }]
        || payment.generic.is_negative()
        || payment.colored.contains(&Color::C)
    {
        return Err(INVALID.into());
    }

    let source: Value = strict_read(&d.provenance.source_json)?;
    let payment_text = payment.printed_fixed_cost();
    let mana_line = format!("{{T}}: Add {{{first:?}}} or {{{second:?}}}.");
    let draw_line = format!("{payment_text}, {{T}}, Sacrifice this land: Draw a card.");
    let lines = [TAPPED_LINE, mana_line.as_str(), draw_line.as_str()];
    let text = lines.join("\n");
    let set = |colors: &[Color]| colors.iter().copied().collect::<BTreeSet<_>>();
    let mut color_identity = set(colors);
    color_identity.extend(payment.colored.iter().copied());
    let produced: Vec<Color> =
        serde_json::from_value(source["produced_mana"].clone()).map_err(|_| INVALID)?;
    if source["oracle_text"] != text
        || source["type_line"] != "Land"
        || source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != ""
        || source["keywords"] != json!([])
        || source["colors"] != json!([])
        || source["color_identity"] != json!(c.color_identity)
        || c.color_identity.len() != color_identity.len()
        || set(&c.color_identity) != color_identity
        || produced.len() != 2
        || set(&produced) != set(colors)
    {
        return Err(INVALID.into());
    }
    let symbols: Vec<_> = payment_text.trim_matches(['{', '}']).split("}{").collect();
    let you = json!({"player":"you"});
    let land = json!({"ref":"this","noun":"land"});
    let expected = json!([
        {"kind":"static","effect":{"static":"enters-tapped","what":land}},
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{
            "effect":"add-mana","who":you,"mana":{"mana":"choice","options":[
                {"mana":"fixed","symbols":[first]},{"mana":"fixed","symbols":[second]}
            ]}
        }]}]},
        {"kind":"activated","costs":[{"cost":"mana","symbols":symbols},{"cost":"tap-self"},
            {"cost":"sacrifice","what":land}],"effects":[{"steps":[{
                "effect":"draw","who":you,"amount":{"amount":"fixed","value":1}
        }]}]}
    ]);
    if strict_read::<Value>(&d.provenance.abilities_json)? != expected
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != lines
                || labels.statics != [0]
                || labels.mana != [1]
                || labels.activated != [2]
                || !labels.triggered.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(true)
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;

    let capability = json!({"contains":{"const":LandSacrificeDrawV1}});
    let required = [
        NormalLayoutV1,
        LandPlayV1,
        EntersTappedV1,
        PermanentManaV1,
        ListedManaChoiceV1,
        ActivatedAbilityV1,
        CompositeActivationCostV1,
        ControllerDrawV1,
        LandSacrificeDrawV1,
    ];
    let has_draw = json!({"required":["op"],"properties":{"op":{"const":"draw_cards"}}});
    let source_costs = json!({"required":["composite"],"properties":{
        "composite":{"properties":{"costs":{"properties":{"tap":{"const":true},"sacrifice":{"const":true}}}}}
    }});
    let selected = json!({"required":["activated","characteristics"],"properties":{
        "characteristics":{"properties":{"kind":{"const":"land"}}},
        "activated":{"contains":{"properties":{"program":{"properties":{"instructions":{"contains":has_draw}}},"cost":source_costs}}}
    }});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":selected,
        "then":{"required":["intrinsic_zone","static_abilities","mana_abilities"],"properties":{
            "capabilities":{"items":{"enum":required},"allOf":required.map(|cap|json!({"contains":{"const":cap}}))},
            "characteristics":{"properties":{
                "kind":{"const":"land"},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "additional_types":{"maxItems":0},"subtypes":{"maxItems":0},"colors":{"maxItems":0}
            }},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
            "triggered":{"maxItems":0},"keywords":{"maxItems":0},
            "static_abilities":{"const":[enters_tapped()]},
            "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "source_block":{"const":1},
                "production":{"properties":{"mana":{"const":"choice"},"colors":{"minItems":2,"maxItems":2}}}
            }}},
            "activated":{"minItems":1,"maxItems":1,"items":{"properties":{
                "functioning_zone":{"const":"battlefield"},
                "cost":{"required":["composite"],"properties":{"composite":{"properties":{"costs":{
                    "required":["mana"],"properties":{
                        "tap":{"const":true},"sacrifice":{"const":true},
                        "mana":{"type":"object","properties":{
                            "generic":{"pattern":"^(0|[1-9][0-9]*)$"},
                            "colored":{"items":{"enum":["W","U","B","R","G"]}}
                        }},
                        "remove_source_plus_one_counters":{"type":"null"}
                    }
                }}}}},
                "program":{"properties":{
                    "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
                    "targets":{"maxItems":0},"modes":{"maxItems":0},
                    "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
                    "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{
                        "op":{"const":"draw_cards"},"recipient":{"type":"null"},
                        "count":{"const":"1"},"source_block":{"const":2}
                    }}}
                }}
            }}},
            "source_labels":{"required":["statics"],"properties":{
                "blocks":{"minItems":3,"maxItems":3},"mana":{"const":[1]},"statics":{"const":[0]},
                "triggered":{"maxItems":0},"activated":{"const":[2]},"spell_modes":{"maxItems":0}
            }}
        }},
        "else":{"properties":{"capabilities":{"not":capability}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let archive = include_str!("../../../tests/fixtures/01ed9139-293d-4477-a145-3c6fe59266c7.json");
    let record: Value = strict_read(archive).unwrap();
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
    d.characteristics.color_identity = vec![Color::R, Color::U];
    d.mana_abilities[0].source_block = 1;
    d.mana_abilities[0].production = ManaProduction::Choice {
        colors: vec![Color::U, Color::R],
    };
    d.static_abilities = vec![enters_tapped()];
    d.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: PermanentActivationCosts {
                tap: true,
                sacrifice: true,
                mana: Some(ManaCost {
                    generic: 4.into(),
                    colored: vec![],
                }),
                remove_source_plus_one_counters: None,
            },
        },
        program: SpellProgram {
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            targets: vec![],
            instructions: vec![Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 2,
            }],
            modes: vec![],
        },
    }];
    d.capabilities = d.requirements();
    d
}
