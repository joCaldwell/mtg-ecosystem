//! Stack-local protection and its supported compositions.
use super::*;
use serde_json::{Value, json};

const PROTECTION_TEXT: &str = "This spell can't be countered.";
const DESTROY_TEXT: &str = "Destroy target nonland permanent.";
const TARGET_ID: &str = "line:1:1";

fn protects_source(ability: &StaticAbility) -> bool {
    ability.effect == StaticEffect::CantBeCountered
}

// Preserve the wire capabilities of the existing protected board damage program.
fn legacy_damage_spell(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Instant
        && d.spell.as_ref().is_some_and(|p| {
            p.targets.is_empty()
                && p.modes.is_empty()
                && matches!(
                    p.instructions.as_slice(),
                    [Instruction::DamageSet {
                        sets,
                        each_player: true,
                        amount: WipeAmount::Fixed { .. },
                        source_block: 1,
                    }] if sets == &[ObjectSet::Creatures]
                )
        })
}

pub(super) fn capability(d: &Definition) -> Capability {
    if legacy_damage_spell(d) {
        Capability::BoardWipesV1
    } else {
        Capability::SelfSpellProtectionV1
    }
}

fn target() -> TargetSpec {
    TargetSpec {
        slot: 0,
        selection_id: TARGET_ID.into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::BattlefieldPermanent,
        filter: Some(PermanentFilter {
            types: vec![],
            non_types: vec![CardKind::Land],
            non_colors: vec![],
            opponent: false,
            tapped: false,
        }),
    }
}

fn source_abilities() -> Value {
    json!([
        {"kind":"static","effect":{
            "static":"cant","what":{"ref":"this","noun":"spell"},"action":"be-countered"
        }},
        {"kind":"spell","effects":[{"steps":[{"effect":"destroy","what":{
            "ref":"target","filter":{"cls":"permanent","nonTypes":["land"]},
            "count":{"amount":"fixed","value":1},"selectionId":TARGET_ID
        }}]}]}
    ])
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let protected = d.static_abilities.iter().any(protects_source);
    let raw = &d.provenance.source_json;
    if !(protected || raw.contains(PROTECTION_TEXT) && raw.contains(DESTROY_TEXT)) {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let protected_destruction = text.contains(PROTECTION_TEXT) && text.contains(DESTROY_TEXT);
    if !protected_destruction && (!protected || legacy_damage_spell(d)) {
        return Ok(());
    }

    let fail =
        || "self-spell protection requires the complete nonland destruction spell".to_owned();
    let type_line = match d.characteristics.kind {
        CardKind::Instant => "Instant",
        CardKind::Sorcery => "Sorcery",
        _ => return Err(fail()),
    };
    let program = d.spell.as_ref().ok_or_else(fail)?;
    if text != format!("{PROTECTION_TEXT}\n{DESTROY_TEXT}")
        || source["layout"] != "normal"
        || source["type_line"] != type_line
        || d.characteristics.snow
        || !d.characteristics.subtypes.is_empty()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || d.x_mana_cost
        || !matches!(
            d.static_abilities.as_slice(),
            [StaticAbility {
                effect: StaticEffect::CantBeCountered,
                functioning_zone: FunctioningZone::Stack,
            }]
        )
        || program.additional_sacrifice.is_some()
        || !program.optional_additional_costs.is_empty()
        || !program.modes.is_empty()
        || program.targets.as_slice() != [target()]
        || !matches!(
            program.instructions.as_slice(),
            [Instruction::DestroyPermanent {
                target_slot: 0,
                optional: false,
                regeneration_forbidden: false,
                source_block: 1,
            }]
        )
        || strict_read::<Value>(&d.provenance.abilities_json)? != source_abilities()
    {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let protection = json!({"required":["effect"],"properties":{
        "effect":{"const":StaticEffect::CantBeCountered}
    }});
    schema["$defs"]["StaticAbility"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":protection,
            "then":{"properties":{"functioning_zone":{"const":"stack"}}}
        }));
    let protected = json!({"required":["static_abilities"],"properties":{
        "static_abilities":{"contains":protection}
    }});
    let legacy = json!({"properties":{
        "characteristics":{"properties":{"kind":{"const":"instant"}}},
        "spell":{"type":"object","required":["instructions"],"properties":{
            "targets":{"maxItems":0},"modes":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{
                "op":{"const":"damage_set"},"sets":{"const":["creatures"]},"each_player":{"const":true},
                "amount":{"properties":{"amount":{"const":"fixed"}}},"source_block":{"const":1}
            }}}
        }}
    }});
    let source = json!({"properties":{"provenance":{"properties":{"source_json":{"allOf":[
        {"pattern":r#""oracle_text"\s*:\s*"[^"]*This spell can't be countered\."#},
        {"pattern":r#""oracle_text"\s*:\s*"[^"]*Destroy target nonland permanent\."#}
    ]}}}}});
    let cast = |capability, timing| {
        json!({"properties":{
            "spell":{"properties":{"timing":{"const":timing}}},
            "capabilities":{"minItems":4,"maxItems":4,"uniqueItems":true,"items":{"enum":[
                Capability::NormalLayoutV1,Capability::PermanentRemovalV1,
                Capability::SelfSpellProtectionV1,capability
            ]}}
        }})
    };
    let targets = json!({"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
        "slot":{"const":0},"selection_id":{"const":TARGET_ID},
        "domain":{"const":"battlefield_permanent"},
        "filter":{"type":"object","properties":{
            "types":{"const":[]},"non_types":{"const":["land"]},"non_colors":{"const":[]},
            "opponent":{"const":false},"tapped":{"const":false}
        }}
    }}});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"anyOf":[source,{"allOf":[protected,{"not":legacy}]}]},
            "then":{
                "required":["spell","static_abilities"],
                "properties":{
                    "static_abilities":{"minItems":1,"maxItems":1,"items":protection},
                    "keywords":{"maxItems":0},"activated":{"maxItems":0},
                    "triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},
                    "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
                    "characteristics":{"properties":{
                        "kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},
                        "subtypes":{"maxItems":0},"snow":{"const":false},"basic":{"const":false},
                        "legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}
                    }},
                    "spell":{"type":"object","required":["targets","instructions"],"properties":{
                        "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
                        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
                        "modes":{"maxItems":0},
                        "targets":targets,
                        "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{
                            "op":{"const":"destroy_permanent"},"target_slot":{"const":0},
                            "optional":{"const":false},"regeneration_forbidden":{"const":false},
                            "source_block":{"const":1}
                        }}}
                    }}
                },
                "if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
                "then":cast(Capability::CastInstantV1,SpellTiming::Instant),
                "else":cast(Capability::CastSorceryV1,SpellTiming::MainPhase)
            },
            "else":{"properties":{"capabilities":{"not":{"contains":{
                "const":Capability::SelfSpellProtectionV1
            }}}}}
        }));
}

#[cfg(test)]
pub(super) fn witness(mut d: Definition) -> Definition {
    d.source_labels = None;
    let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
    source["oracle_text"] = json!(format!("{PROTECTION_TEXT}\n{DESTROY_TEXT}"));
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = source_abilities().to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    let p = d.spell.as_mut().unwrap();
    p.targets = vec![target()];
    p.instructions = vec![Instruction::DestroyPermanent {
        target_slot: 0,
        optional: false,
        regeneration_forbidden: false,
        source_block: 1,
    }];
    d.capabilities = d.requirements();
    d
}
