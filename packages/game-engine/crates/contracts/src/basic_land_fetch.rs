//! CR 205.4c, 601.2h, 602.2b, 701.23b and 701.24a/e (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};
use std::num::NonZeroU32;

pub fn basic_land_fetch_program(force_tapped: bool) -> SpellProgram {
    conditional_basic_land_fetch_program(force_tapped, None)
}

pub fn conditional_basic_land_fetch_program(
    force_tapped: bool,
    minimum_lands_to_untap: Option<NonZeroU32>,
) -> SpellProgram {
    SpellProgram {
        additional_sacrifice: None,
        optional_additional_costs: vec![],
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        targets: vec![],
        instructions: vec![Instruction::FetchBasicLand {
            force_tapped,
            minimum_lands_to_untap,
            source_block: 0,
        }],
        modes: vec![],
    }
}

fn has_fetch(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::FetchBasicLand { .. }))
        || program.modes.iter().any(|mode| has_fetch(&mode.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<bool, String> {
    let misplaced = d.spell.as_ref().is_some_and(has_fetch)
        || d.triggered.iter().any(|a| has_fetch(&a.program));
    if !misplaced && !d.activated.iter().any(|a| has_fetch(&a.program)) {
        return Ok(false);
    }

    let invalid = "requires a complete tap-and-sacrifice basic land fetch activation";
    let [activation] = d.activated.as_slice() else {
        return Err(invalid.into());
    };
    let [
        Instruction::FetchBasicLand {
            force_tapped,
            minimum_lands_to_untap,
            ..
        },
    ] = activation.program.instructions.as_slice()
    else {
        return Err(invalid.into());
    };
    let c = &d.characteristics;
    let valid_cost = match (c.kind, &activation.cost) {
        (CardKind::Land, ActivationCost::TapPayLifeSacrificeSource { life }) => !life.is_negative(),
        (CardKind::Land | CardKind::Artifact, ActivationCost::Composite { costs }) => {
            validate_permanent_costs(costs)?;
            costs.tap
                && costs.sacrifice
                && costs.remove_source_plus_one_counters.is_none()
                && costs.mana.is_some() == (c.kind == CardKind::Artifact)
        }
        _ => false,
    };
    if !valid_cost
        || misplaced
        || c.basic
        || c.snow
        || c.legendary
        || d.x_mana_cost
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || activation.functioning_zone != FunctioningZone::Battlefield
        || minimum_lands_to_untap.is_some() && !*force_tapped
        || activation.program
            != conditional_basic_land_fetch_program(*force_tapped, *minimum_lands_to_untap)
    {
        return Err(invalid.into());
    }
    validate_archived_activation(d, activation, *force_tapped, *minimum_lands_to_untap)
        .map_err(|_| invalid)?;
    Ok(true)
}

fn validate_archived_activation(
    d: &Definition,
    activation: &ActivatedAbility,
    force_tapped: bool,
    minimum_lands_to_untap: Option<NonZeroU32>,
) -> Result<(), String> {
    let invalid = "basic land fetch source disagrees with its cost or destination";
    let source: Value = strict_read(&d.provenance.source_json)?;
    let (cost_text, effect_text) = source["oracle_text"]
        .as_str()
        .and_then(|text| text.split_once(": "))
        .ok_or(invalid)?;
    let noun = if d.characteristics.kind == CardKind::Land {
        "land"
    } else {
        "artifact"
    };
    let tap = json!({"cost":"tap-self"});
    let sacrifice = json!({"cost":"sacrifice","what":{"ref":"this","noun":noun}});
    let (expected_cost, cost_ast) = match &activation.cost {
        ActivationCost::Composite { costs } => match &costs.mana {
            None => (
                format!("{{T}}, Sacrifice this {noun}"),
                vec![tap, sacrifice],
            ),
            Some(mana) => {
                let printed = mana.printed_fixed_cost();
                let symbols: Vec<_> = printed.trim_matches(['{', '}']).split("}{").collect();
                (
                    format!("{printed}, {{T}}, Sacrifice this {noun}"),
                    vec![json!({"cost":"mana","symbols":symbols}), tap, sacrifice],
                )
            }
        },
        ActivationCost::TapPayLifeSacrificeSource { life } => {
            let value = life.to_string().parse::<u64>().map_err(|_| invalid)?;
            (
                format!("{{T}}, Pay {value} life, Sacrifice this {noun}"),
                vec![
                    tap,
                    json!({"cost":"pay-life","amount":{"amount":"fixed","value":value}}),
                    sacrifice,
                ],
            )
        }
        _ => return Err(invalid.into()),
    };
    let suffix = if force_tapped {
        " tapped, then shuffle."
    } else {
        ", then shuffle."
    };
    let (effect_text, trailing) = effect_text
        .split_once(" Then ")
        .map_or((effect_text, ""), |(search, _)| {
            (search, &effect_text[search.len()..])
        });
    let untap = match minimum_lands_to_untap {
        Some(minimum) => Some(fetched_land_untap_clause(trailing, minimum).ok_or(invalid)?),
        None if trailing.is_empty() => None,
        None => return Err(invalid.into()),
    };
    let moved = effect_text
        .strip_prefix("Search your library for a basic land card, put ")
        .and_then(|text| text.strip_suffix(suffix))
        .and_then(|text| text.strip_suffix(" onto the battlefield"));
    let moved = match moved {
        Some("it") => json!({"ref":"it"}),
        Some("that card") => json!({"ref":"that","filter":{"cls":"card"}}),
        _ => return Err(invalid.into()),
    };
    let you = json!({"player":"you"});
    let mut movement = json!({"effect":"move-zone","what":moved,"to":{"zone":"battlefield"}});
    if force_tapped {
        movement["tapped"] = json!(true);
    }
    let mut effects = vec![json!({"steps":[
        {"effect":"search","who":you,"zones":{"selection":"all","of":[{"zone":"library","owner":"your"}]},
            "for":{"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},
                "count":{"amount":"fixed","value":1}}},
        movement,
        {"effect":"shuffle","who":you,"library":{"zone":"library","owner":you}}
    ]})];
    effects.extend(untap);
    let expected = json!([{"kind":"activated","costs":cost_ast,"effects":effects}]);
    if cost_text != expected_cost || strict_read::<Value>(&d.provenance.abilities_json)? != expected
    {
        return Err(invalid.into());
    }
    Ok(())
}

fn instruction_schema() -> Value {
    json!({"required":["op"],"properties":{"op":{"const":"fetch_basic_land"}}})
}

pub(super) fn program_schema() -> Value {
    json!({"required":["instructions"],"properties":{"instructions":{"contains":instruction_schema()}}})
}

pub(super) fn activation_schema() -> Value {
    json!({"required":["program"],"properties":{"program":program_schema()}})
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;

    let selected = json!({"required":["activated"],"properties":{"activated":{"contains":activation_schema()}}});
    let single = json!({"allOf":[instruction_schema(),{"properties":{"source_block":{"const":0}}},
        {"if":{"required":["minimum_lands_to_untap"],"properties":{"minimum_lands_to_untap":{"type":"integer"}}},
         "then":{"properties":{"force_tapped":{"const":true}}}}
    ]});
    let program = json!({"properties":{
        "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
        "targets":{"maxItems":0},"modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},
        "additional_sacrifice":{"type":"null"},
        "instructions":{"minItems":1,"maxItems":1,"items":single}
    }});
    let common = [NormalLayoutV1, ActivatedAbilityV1, BasicLandFetchV1];
    let cost = |mana: Value| {
        let required = if mana["type"] == "object" {
            vec!["mana"]
        } else {
            vec![]
        };
        json!({"required":["composite"],"properties":{
            "composite":{"properties":{"costs":{"required":required,"properties":{"tap":{"const":true},"sacrifice":{"const":true},
                "remove_source_plus_one_counters":{"type":"null"},"mana":mana
            }}}}
        }})
    };
    let branch = |kind, cost, caps: Vec<Capability>| {
        let required = [common.as_slice(), caps.as_slice()].concat();
        json!({"properties":{
            "characteristics":{"properties":{"kind":{"const":kind}}},
            "capabilities":{"items":{"enum":required},
                "allOf":required.iter().map(|cap|json!({"contains":{"const":cap}})).collect::<Vec<_>>()},
            "activated":{"items":{"properties":{"cost":cost}}}
        }})
    };
    let land = branch(
        CardKind::Land,
        cost(json!({"type":"null"})),
        vec![LandPlayV1, CompositeActivationCostV1],
    );
    let life = branch(
        CardKind::Land,
        json!({"required":["tap_pay_life_sacrifice_source"]}),
        vec![
            LandPlayV1,
            TapSourceCostV1,
            SacrificeSourceCostV1,
            PayLifeCostV1,
        ],
    );
    let artifact = branch(
        CardKind::Artifact,
        cost(json!({"type":"object","properties":{
            "generic":{"pattern":"^(0|[1-9][0-9]*)$"},"colored":{"items":{"enum":["W","U","B","R","G"]}}
        }})),
        vec![CastArtifactV1, CompositeActivationCostV1],
    );
    let free = json!({"properties":{
        "instructions":{"items":{"not":instruction_schema()}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/BasicLandFetchFreeProgram"}}}}
    }});
    schema["$defs"]["BasicLandFetchFreeProgram"] = free;
    let free = json!({"$ref":"#/$defs/BasicLandFetchFreeProgram"});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({"properties":{"spell":free,"triggered":{"items":{"properties":{"program":free}}}}}),
        json!({"if":selected,"then":{
            "oneOf":[land,life,artifact],
            "properties":{
                "characteristics":{"properties":{"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                    "subtypes":{"maxItems":0},"additional_types":{"maxItems":0}}},
                "x_mana_cost":{"const":false},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},
                "mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},
                "activated":{"minItems":1,"maxItems":1,"items":{"properties":{
                    "functioning_zone":{"const":"battlefield"},"program":program
                }}}
            }
        },"else":{"properties":{
            "capabilities":{"not":{"contains":{"const":BasicLandFetchV1}}},
            "activated":{"items":{"properties":{"program":free}}}
        }}}),
    ]);
}
