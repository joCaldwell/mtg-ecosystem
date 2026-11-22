//! A stack trigger observes the tap cost of a mana ability (CR 106.12,
//! 603.2–3 and 605.1b). Its damage instruction is not a mana ability.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "filter", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaTapFilter {
    Land,
    NonbasicLand,
    LandSubtype { subtype: BasicLandType },
}

impl ManaTapFilter {
    pub fn matches(self, characteristics: &Characteristics) -> bool {
        characteristics.has_type(CardKind::Land)
            && match self {
                Self::Land => true,
                Self::NonbasicLand => !characteristics.basic,
                Self::LandSubtype { subtype } => characteristics
                    .subtypes
                    .iter()
                    .any(|s| s == subtype.subtype()),
            }
    }
}

pub fn printed(text: &str) -> Option<(Value, TriggeredAbility)> {
    let (land, effect) = text
        .strip_prefix("Whenever a player taps ")?
        .split_once(" for mana, ")?;
    let damage = effect
        .strip_prefix("this enchantment deals ")?
        .strip_suffix(" damage to that player.")?
        .parse::<u32>()
        .ok()?;
    if damage == 0 {
        return None;
    }
    let (condition, filter) = match land {
        "a land" => (ManaTapFilter::Land, json!({"types":["land"]})),
        "a nonbasic land" => (
            ManaTapFilter::NonbasicLand,
            json!({"nonSupertypes":["basic"],"types":["land"]}),
        ),
        "an Island" => (
            ManaTapFilter::LandSubtype {
                subtype: BasicLandType::Island,
            },
            json!({"subtypes":["Island"]}),
        ),
        _ => return None,
    };
    let fixed = |value| json!({"amount":"fixed","value":value});
    let ast = json!({"kind":"triggered","trigger":{"trigger":"taps-for-mana",
        "what":{"ref":"filter","filter":filter,"count":fixed(1)},"who":{"player":"a-player"}},
        "effects":[{"steps":[{"effect":"damage","source":{"ref":"this","noun":"enchantment"},"amount":fixed(damage),
            "to":{"to":"player","ref":{"player":"that-player"}}}]}]});
    Some((
        ast,
        TriggeredAbility {
            condition: TriggerCondition::ManaTapped { filter: condition },
            functioning_zone: FunctioningZone::Battlefield,
            intervening_if: None,
            death_filter: None,
            program: SpellProgram {
                functioning_zone: FunctioningZone::Stack,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                targets: vec![],
                modes: vec![],
                instructions: vec![Instruction::damage(
                    kir::DamageTo::Ref {
                        who: kir::Ref::EventPlayer {
                            role: kir::EventPlayerRole::ActivatingPlayer,
                        },
                    },
                    kir::Amount::Fixed {
                        value: i64::from(damage).into(),
                    },
                    0,
                )],
            },
        },
    ))
}

fn uses(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Damage {
                to: kir::DamageTo::Ref {
                    who: kir::Ref::EventPlayer {
                        role: kir::EventPlayerRole::ActivatingPlayer
                    }
                },
                amount: fixed!(_),
                ..
            })
        )
    }) || program.modes.iter().any(|m| uses(&m.program))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let use_trigger = d
        .triggered
        .iter()
        .any(|a| matches!(a.condition, TriggerCondition::ManaTapped { .. }) || uses(&a.program));
    let raw = &d.provenance.source_json;
    if d.spell.as_ref().is_some_and(uses) || d.activated.iter().any(|a| uses(&a.program))
        || d.static_abilities.iter().any(|a| matches!(&a.effect, StaticEffect::GrantSpellCascade{program,..}|StaticEffect::Bestow{program,..} if uses(program)))
    {
        return Err("mana tap damage requires its witnessed stack trigger".into());
    }
    if !use_trigger && !raw.contains(" for mana, this enchantment deals ") && !raw.contains(r"\u") {
        return Ok(());
    }
    let row: Value = strict_read(raw)?;
    let source = row["oracle_text"].as_str().unwrap_or_default();
    let expected = printed(source);
    if expected.is_none() && !use_trigger {
        return Ok(());
    }
    let fail = "requires complete printed mana tap damage and the activating player";
    let (ast, trigger) = expected.ok_or(fail)?;
    let c = &d.characteristics;
    if d.triggered != [trigger]
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || c.kind != CardKind::Enchantment
        || c.legendary
        || c.basic
        || c.snow
        || !c.hybrid_mana.is_empty()
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || row["layout"] != "normal"
        || row["type_line"] != "Enchantment"
        || row["name"] != d.name
        || row["oracle_id"] != d.oracle_id
        || row["id"] != d.provenance.printing_id
        || row["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || row["colors"] != json!(c.colors)
        || row["color_identity"] != json!(c.color_identity)
        || strict_read::<Value>(&d.provenance.abilities_json)? != json!([ast])
    {
        return Err(fail.into());
    }
    Ok(())
}

pub(super) fn extend_schema(s: &mut Value) {
    let cap = json!({"contains":{"const":Capability::ManaTapTriggerV1}});
    let opcode = instruction_schema::activator_damage();
    let trigger = json!({"required":["condition"],"properties":{"condition":{"type":"object","required":["mana_tapped"]}}});
    let free = json!({"$ref":"#/$defs/ManaTapDamageFreeProgram"});
    s["$defs"]["ManaTapDamageFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":opcode}},
        "modes":{"items":{"properties":{"program":free}}}
    }});

    let source = json!({"required":["provenance"],"properties":{"provenance":{"required":["source_json"],
        "properties":{"source_json":{"pattern":"Whenever a player taps (a land|a nonbasic land|an Island) for mana, this enchantment deals [1-9][0-9]* damage to that player[.]"}}}}});
    let program = json!({"required":["instructions","targets"],"properties":{"instructions":{
        "minItems":1,"maxItems":1,"items":opcode
    },"targets":{"maxItems":0},"modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},
    "additional_sacrifice":{"type":"null"},"timing":{"const":"instant"},"resolution":{"const":"instructions"},
    "functioning_zone":{"const":"stack"}}});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "spell":free,
            "activated":{"items":{"properties":{"program":free}}},
            "triggered":{"items":{"if":trigger,"else":{"properties":{"program":free}}}},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{
                "bestow":{"properties":{"program":free}},"grant_spell_cascade":{"properties":{"program":free}}
            }}}}}
        },
        "if":{"anyOf":[source,{"required":["capabilities"],"properties":{"capabilities":cap}},
            {"required":["triggered"],"properties":{"triggered":{"contains":trigger}}}]},
        "then":{"required":["triggered","capabilities"],"properties":{"capabilities":cap,
            "activated":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "triggered":{"minItems":1,"maxItems":1,"items":{"allOf":[trigger,{"required":["program"],"properties":{
                "program":program,"functioning_zone":{"const":"battlefield"},"death_filter":{"type":"null"},
                "intervening_if":{"type":"null"}
            }}]}}
        }}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/phase1-r4/0f1afedd-c60f-454f-b84a-c8117aec0128.json"
    ))
    .unwrap();
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Furnace of Rath")
    .unwrap();
    composition_fixtures::stamp(&mut d, &row);
    d.characteristics.mana_cost = ManaCost {
        generic: 3.into(),
        colored: vec![Color::R],
    };
    d.triggered = vec![
        printed(row["source"]["oracle_text"].as_str().unwrap())
            .unwrap()
            .1,
    ];
    d.static_abilities.clear();
    d.capabilities = d.requirements();
    d
}
