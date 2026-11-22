//! Ordinary targeted exile followed by mandatory fixed spell-controller life loss.
use super::*;
use serde_json::{Value, json};

const SOURCE_PREFIX: &str = "Exile target nonland permanent. You lose ";
const TARGET_ID: &str = "line:0:1";
const INVALID_FAMILY: &str =
    "nonland exile and life loss require one ordinary spell and its complete source";

fn contains_pair(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Move {
                what: target_object!(_),
                to: kir::Destination::Exile
            })
        )
    }) && program
        .instructions
        .iter()
        .any(|i| matches!(i, kernel!(LoseLife { .. })))
        || program
            .modes
            .iter()
            .any(|mode| contains_pair(&mode.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| contains_pair(&a.program))
        || d.triggered.iter().any(|a| contains_pair(&a.program))
    {
        return Err(INVALID_FAMILY.into());
    }
    let paired = d.spell.as_ref().is_some_and(contains_pair);
    if !paired && !d.provenance.source_json.contains(SOURCE_PREFIX) {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let oracle_text = source["oracle_text"].as_str().unwrap_or_default();
    if !paired && !oracle_text.contains(SOURCE_PREFIX) {
        return Ok(());
    }
    let fail = || INVALID_FAMILY.to_owned();
    let p = d.spell.as_ref().ok_or_else(fail)?;
    let (type_line, timing) = match d.characteristics.kind {
        CardKind::Instant => ("Instant", SpellTiming::Instant),
        CardKind::Sorcery => ("Sorcery", SpellTiming::MainPhase),
        _ => return Err(fail()),
    };
    let [target] = p.targets.as_slice() else {
        return Err(fail());
    };
    let [
        kernel!(Move { what: target_object!(0), to: kir::Destination::Exile } @ 0),
        kernel!(LoseLife { player: you!(), amount: fixed!(amount) } @ 0),
    ] = p.instructions.as_slice()
    else {
        return Err(fail());
    };
    let amount = amount.to_string().parse::<u32>().map_err(|_| fail())?;
    if amount == 0
        || oracle_text != format!("{SOURCE_PREFIX}{amount} life.")
        || source["type_line"] != type_line
        || d.characteristics.snow
        || !d.characteristics.subtypes.is_empty()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || d.x_mana_cost
        || p.functioning_zone != FunctioningZone::Stack
        || p.timing != timing
        || p.resolution != SpellResolution::Instructions
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || target.slot != 0
        || target.selection_id != TARGET_ID
        || target.domain != TargetDomain::BattlefieldPermanent
        || target.filter.as_ref().is_none_or(|f| {
            !f.types.is_empty()
                || f.non_types != [CardKind::Land]
                || !f.non_colors.is_empty()
                || f.opponent
                || f.power.is_some()
                || f.tapped
        })
    {
        return Err(fail());
    }

    // Read the original archive JSON because typed Option fields can erase
    // the difference between an absent qualifier and an explicit null.
    let abilities: Value = strict_read(&d.provenance.abilities_json)?;
    if abilities
        != json!([{"kind":"spell","effects":[
            {"steps":[{"effect":"exile","what":{
                "ref":"target","filter":{"cls":"permanent","nonTypes":["land"]},
                "count":{"amount":"fixed","value":1},"selectionId":TARGET_ID
            }}]},
            {"steps":[{"effect":"lose-life","who":{"player":"you"},
                       "amount":{"amount":"fixed","value":amount}}]}
        ]}])
    {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let exile = instruction_schema::target_move(None, "exile");
    let free = json!({"$ref":"#/$defs/NonlandExileLifeFreeProgram"});
    schema["$defs"]["NonlandExileLifeFreeProgram"] = json!({"properties":{
        "instructions":{"not":{"allOf":[{"contains":exile},{"contains":instruction_schema::kernel_verb("lose_life")}]}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    let family = json!({"$ref":"#/$defs/NonlandExileLifeProgram"});
    schema["$defs"]["NonlandExileLifeProgram"] = json!({
        "required":["targets","instructions"],"properties":{
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "modes":{"maxItems":0},
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "slot":{"const":0},"domain":{"const":"battlefield_permanent"},
                "selection_id":{"const":TARGET_ID},
                "filter":{"type":"object","properties":{
                    "types":{"const":[]},"non_types":{"const":["land"]},"non_colors":{"const":[]},
                    "opponent":{"const":false},"tapped":{"const":false}
                }}
            }}},
            "instructions":{"minItems":2,"maxItems":2,"prefixItems":[
                instruction_schema::at(instruction_schema::target_move(Some(0), "exile"), 0),
                instruction_schema::kernel_verb_at("lose_life", 0)
            ],"items":false}
        }
    });
    let program = |capability, timing| {
        json!({"properties":{
            "spell":{"properties":{"timing":{"const":timing}}},
            "capabilities":{"minItems":4,"maxItems":4,"uniqueItems":true,"items":{"enum":[
                Capability::NormalLayoutV1,Capability::PermanentRemovalV1,Capability::LifeLossV1,
                capability
            ]}}
        }})
    };
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{
                "activated":{"items":{"properties":{"program":free}}},
                "triggered":{"items":{"properties":{"program":free}}}
            },
            "if":{"anyOf":[
                {"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
                {"required":["provenance"],"properties":{"provenance":{"properties":{
                    "source_json":{"pattern":r#""oracle_text"\s*:\s*"[^"]*Exile target nonland permanent\. You lose "#}
                }}}}
            ]},
            "then":{
                "required":["spell"],"properties":{
                    "spell":{"type":"object","allOf":[family]},
                    "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
                    "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},
                    "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
                    "characteristics":{"properties":{
                        "kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},
                        "subtypes":{"maxItems":0},"snow":{"const":false},"basic":{"const":false},
                        "legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}
                    }}
                },
                "if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
                "then":program(Capability::CastInstantV1,SpellTiming::Instant),
                "else":program(Capability::CastSorceryV1,SpellTiming::MainPhase)
            }
        }));
}
