//! CR 109.5, 113.7a, 603.2c/3a/10, 605.1, 701.26a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PRINTED_TRIGGER: &str = "Whenever this land becomes tapped, it deals ";
const SHAPE_ERROR: &str =
    "source tap damage requires a complete land with a separate any-color mana ability";

fn controller_damage(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Damage {
                to: kir::DamageTo::Ref {
                    who: kir::Ref::Controller { .. }
                },
                amount: fixed!(_),
                ..
            })
        )
    }) || program
        .modes
        .iter()
        .any(|mode| controller_damage(&mode.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.spell.as_ref().is_some_and(controller_damage)
        || d.activated.iter().any(|a| controller_damage(&a.program))
    {
        return Err(SHAPE_ERROR.into());
    }
    let printed_trigger = d.provenance.source_json.contains(PRINTED_TRIGGER)
        && strict_read::<Value>(&d.provenance.source_json)?["oracle_text"]
            .as_str()
            .is_some_and(|text| text.contains(PRINTED_TRIGGER));
    if !printed_trigger
        && !d.triggered.iter().any(|a| {
            a.condition == TriggerCondition::SelfBecomesTapped || controller_damage(&a.program)
        })
    {
        return Ok(());
    }
    let ([trigger], [mana]) = (d.triggered.as_slice(), d.mana_abilities.as_slice()) else {
        return Err(SHAPE_ERROR.into());
    };
    let c = &d.characteristics;
    let p = &trigger.program;
    if c.kind != CardKind::Land
        || c.basic
        || c.snow
        || c.legendary
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || c.mana_cost.generic != Quantity::zero()
        || !c.mana_cost.colored.is_empty()
        || d.intrinsic_zone != Some(FunctioningZone::Battlefield)
        || d.x_mana_cost
        || d.spell.is_some()
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || mana.self_damage.is_some()
        || mana.restriction.is_some()
        || mana.source_block != 1
        || mana.production != ManaProduction::AnyColor
        || !mana.costs.tap
        || mana.costs.mana.is_some()
        || mana.costs.sacrifice
        || mana.costs.remove_source_plus_one_counters.is_some()
        || trigger.condition != TriggerCondition::SelfBecomesTapped
        || trigger.functioning_zone != FunctioningZone::Battlefield
        || trigger.intervening_if.is_some()
        || trigger.death_filter.is_some()
        || p.functioning_zone != FunctioningZone::Stack
        || p.timing != SpellTiming::Instant
        || p.resolution != SpellResolution::Instructions
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.targets.is_empty()
        || !p.modes.is_empty()
        || !matches!(
            p.instructions.as_slice(),
            [kernel!(Damage {
                to: kir::DamageTo::Ref {
                    who: kir::Ref::Controller { .. }
                },
                amount: fixed!(amount),
                ..
            } @ 0)]
                if *amount > Quantity::zero()
        )
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.blocks.len() != 2 || labels.triggered != [0] || labels.mana != [1]
        })
    {
        return Err(SHAPE_ERROR.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let condition = TriggerCondition::SelfBecomesTapped;
    let op = instruction_schema::controller_damage();
    let free = json!({"$ref":"#/$defs/ControllerDamageFreeProgram"});
    schema["$defs"]["ControllerDamageFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":op}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["TriggeredAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["condition"],"properties":{"condition":{"const":condition}}},
            "then":{"properties":{
                "functioning_zone":{"const":"battlefield"},
                "intervening_if":{"type":"null"},"death_filter":{"type":"null"},
                "program":{"properties":{
                    "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},
                    "resolution":{"const":"instructions"},"targets":{"maxItems":0},
                    "additional_sacrifice":{"type":"null"},
                    "optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},
                    "instructions":{"minItems":1,"maxItems":1,"items":{"allOf":[op,
                        {"properties":{"source_block":{"const":0},"amount":{"pattern":"^[1-9][0-9]*$"}}}
                    ]}}
                }}
            }},
            "else":{"properties":{"program":free}}
        }));
    let capability = Capability::SourceTapTriggerV1;
    let capabilities = [
        Capability::NormalLayoutV1,
        Capability::LandPlayV1,
        Capability::PermanentManaV1,
        Capability::TriggeredAbilityV1,
        Capability::NoncombatDamageV1,
        capability,
    ];
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{
                "spell":{"anyOf":[{"type":"null"},free]},
                "activated":{"items":{"properties":{"program":free}}}
            },
            "if":{"anyOf":[
                {"required":["triggered"],"properties":{"triggered":{"contains":{
                    "required":["condition"],"properties":{"condition":{"const":condition}}
                }}}},
                {"required":["provenance"],"properties":{"provenance":{"properties":{
                    "source_json":{"pattern":r#""oracle_text"\s*:\s*"[^"]*Whenever this land becomes tapped, it deals "#}
                }}}}
            ]},
            "then":{"required":["intrinsic_zone"],"properties":{
                "characteristics":{"properties":{
                    "kind":{"const":"land"},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0},
                    "basic":{"const":false},"snow":{"const":false},"legendary":{"const":false},
                    "mana_cost":{"const":{"generic":"0","colored":[]}}
                }},
                "source_labels":{"properties":{
                    "blocks":{"minItems":2,"maxItems":2},"triggered":{"const":[0]},"mana":{"const":[1]}
                }},
                "intrinsic_zone":{"const":"battlefield"},"x_mana_cost":{"const":false},
                "spell":{"type":"null"},"activated":{"maxItems":0},"static_abilities":{"maxItems":0},
                "keywords":{"maxItems":0},
                "triggered":{"minItems":1,"maxItems":1,"items":{"properties":{"condition":{"const":condition}}}},
                "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                    "source_block":{"const":1},"production":{"const":{"mana":"any_color"}},
                    "restriction":{"type":"null"},"self_damage":{"type":"null"},
                    "costs":{"properties":{
                        "tap":{"const":true},"mana":{"type":"null"},"sacrifice":{"const":false},
                        "remove_source_plus_one_counters":{"type":"null"}
                    }}
                }}},
                "capabilities":{
                    "items":{"enum":capabilities},
                    "allOf":capabilities.map(|cap| json!({"contains":{"const":cap}}))
                }
            }},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":capability}}}}}
        }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Command Tower")
    .unwrap();
    d.source_labels = None;
    d.mana_abilities[0].production = ManaProduction::AnyColor;
    d.mana_abilities[0].source_block = 1;
    d.triggered.push(TriggeredAbility {
        death_filter: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfBecomesTapped,
        intervening_if: None,
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::damage(
                kir::DamageTo::Ref {
                    who: kir::Ref::controller(kir::Context::Triggered),
                },
                kir::Amount::Fixed { value: 1.into() },
                0,
            )],
            modes: vec![],
        },
    });
    d.capabilities = d.requirements();
    d
}
