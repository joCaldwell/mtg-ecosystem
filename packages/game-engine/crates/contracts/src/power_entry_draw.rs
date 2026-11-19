//! CR 603.4/603.6a/603.10: power-filtered entry and intervening conditions.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

use super::*;
use serde_json::{Value, json};

fn draw_ability(ability: &TriggeredAbility) -> Option<bool> {
    let p = &ability.program;
    if ability.functioning_zone != FunctioningZone::Battlefield
        || ability.death_filter.is_some()
        || p.timing != SpellTiming::Instant
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || p.additional_sacrifice.is_some()
        || !p.targets.is_empty()
        || !p.modes.is_empty()
    {
        return None;
    }
    match p.instructions.as_slice() {
        [
            Instruction::DrawCards {
                recipient: None,
                count,
                ..
            },
        ] if *count == Quantity::from(1) => Some(false),
        [Instruction::OptionalDrawCards { .. }] => Some(true),
        _ => None,
    }
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if !d.triggered.iter().any(|a| {
        matches!(
            a.condition,
            TriggerCondition::ControlledCreatureEntersWithPower { .. }
        ) || matches!(
            a.intervening_if,
            Some(InterveningCondition::ControllerHasCreatureWithPower { .. })
        )
    }) {
        return Ok(());
    }
    let err = || "unsupported complete powered creature entry draw definition".to_string();
    let c = &d.characteristics;
    if !matches!(c.kind, CardKind::Creature | CardKind::Enchantment)
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || c.subtypes
            .iter()
            .any(|s| unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES.contains(&s.as_str()))
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !matches!(&d.spell, Some(p) if
            p.timing == SpellTiming::MainPhase
                && p.functioning_zone == FunctioningZone::Stack
                && p.resolution == SpellResolution::Permanent
                && p.additional_sacrifice.is_none()
                && p.targets.is_empty() && p.modes.is_empty() && p.instructions.is_empty())
    {
        return Err(err());
    }
    let group_keyword = match d.static_abilities.as_slice() {
        [] => None,
        [
            StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect:
                    StaticEffect::ModifyCreatures {
                        scope: CreatureScope::Controller,
                        power,
                        toughness,
                        keywords,
                        restrictions,
                    },
            },
        ] if c.kind == CardKind::Enchantment
            && *power == Quantity::zero()
            && *toughness == Quantity::zero()
            && restrictions.is_empty()
            && matches!(
                keywords.as_slice(),
                [KeywordAbility::Haste | KeywordAbility::Trample]
            ) =>
        {
            Some(keywords[0])
        }
        _ => return Err(err()),
    };

    let mut power_draw = None;
    let mut self_draw = false;
    for a in &d.triggered {
        let optional = draw_ability(a).ok_or_else(err)?;
        match (&a.condition, &a.intervening_if) {
            (TriggerCondition::ControlledCreatureEntersWithPower { .. }, None)
                if power_draw.is_none() =>
            {
                power_draw = Some(optional);
            }
            (
                TriggerCondition::SelfEntersBattlefield,
                Some(InterveningCondition::ControllerHasCreatureWithPower { .. }),
            ) if !optional && !self_draw => self_draw = true,
            _ => return Err(err()),
        }
    }
    match (group_keyword, power_draw, self_draw) {
        (None, Some(_), false)
        | (Some(KeywordAbility::Haste), Some(true), false)
        | (Some(KeywordAbility::Trample), Some(false), true) => Ok(()),
        _ => Err(err()),
    }
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let power_condition =
        json!({"type":"object","required":["controlled_creature_enters_with_power"]});
    let intervening_power =
        json!({"type":"object","required":["controller_has_creature_with_power"]});
    let has = |key: &str, predicate: &Value| json!({"required":[key],"properties":{(key):predicate}});
    let single = |item: &Value| json!({"minItems":1,"maxItems":1,"items":item});
    let capabilities = |additional: &[Capability]| {
        let allowed = &[
            NormalLayoutV1,
            TriggeredAbilityV1,
            PowerFilteredCreatureEntryV1,
        ];
        let allowed = [allowed.as_slice(), additional].concat();
        json!({"minItems":allowed.len(),"maxItems":allowed.len(),"uniqueItems":true,
            "items":{"enum":allowed}})
    };
    let permanent = |kind, trigger: &Value, statics: Value, caps| {
        json!({
            "properties":{"characteristics":{"properties":{"kind":{"const":kind}}},
                "triggered":trigger,"static_abilities":statics,"capabilities":caps}
        })
    };
    let instruction = |optional| {
        if optional {
            json!({"required":["op"],"properties":{"op":{"const":"optional_draw_cards"}}})
        } else {
            json!({"required":["op","count"],"properties":{"op":{"const":"draw_cards"},"count":{"const":"1"},"recipient":{"type":"null"}}})
        }
    };
    let ability = |condition: &Value, condition_if: &Value, optional: bool| {
        json!({
            "required":["functioning_zone","condition","program"],
            "properties":{"condition":condition,"intervening_if":condition_if,"death_filter":{"type":"null"},
                "functioning_zone":{"const":"battlefield"},"program":{
                    "properties":{"timing":{"const":"instant"},"functioning_zone":{"const":"stack"},
                        "resolution":{"const":"instructions"},"additional_sacrifice":{"type":"null"},
                        "modes":{"maxItems":0},"targets":{"maxItems":0},"instructions":single(&instruction(optional))}}}
        })
    };
    let power = |optional| ability(&power_condition, &json!({"type":"null"}), optional);
    let self_condition = ability(
        &json!({"const":"self_enters_battlefield"}),
        &intervening_power,
        false,
    );
    let self_condition = json!({"allOf":[self_condition, {"required":["intervening_if"]}]});
    let static_keyword = |keyword| {
        single(&json!({"properties":{
            "functioning_zone":{"const":"battlefield"},
            "effect":{"const":{"modify_creatures":{
                "scope":"controller","power":"0","toughness":"0","keywords":[keyword],"restrictions":[]
            }}}
        }}))
    };
    let two = json!({"minItems":2,"maxItems":2,
        "allOf":[{"contains":power(false)},{"contains":self_condition}]});
    let plain = |kind, base: &[Capability]| {
        json!({"anyOf":[
            permanent(kind,&single(&power(false)),json!({"maxItems":0}),capabilities(&[base, &[ControllerDrawV1]].concat())),
            permanent(kind,&single(&power(true)),json!({"maxItems":0}),capabilities(&[base, &[OptionalControllerDrawV1]].concat()))
        ]})
    };
    let disallowed: Vec<_> = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES)
        .collect();
    let defs = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    defs.push(json!({
        "if":{"anyOf":[
            has("triggered", &json!({"contains":{"anyOf":[
                has("condition", &power_condition), has("intervening_if", &intervening_power)
            ]}})),
            has("capabilities", &json!({"contains":{"const":PowerFilteredCreatureEntryV1}}))
        ]},
        "then":{"required":["spell","triggered"],"properties":{
            "characteristics":{"properties":{"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"additional_types":{"maxItems":0},
                "subtypes":{"items":{"not":{"enum":disallowed}}}}},
            "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
            "keywords":{"maxItems":0},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "spell":{"type":"object","properties":{"timing":{"const":"main_phase"},
                "functioning_zone":{"const":"stack"},"resolution":{"const":"permanent"},
                "additional_sacrifice":{"type":"null"},"targets":{"maxItems":0},
                "modes":{"maxItems":0},"instructions":{"maxItems":0}}}
        },"anyOf":[
            plain(CardKind::Creature,&[BaseCreatureV1,CastCreatureV1]),
            plain(CardKind::Enchantment,&[CastEnchantmentV1]),
            permanent(CardKind::Enchantment, &single(&power(true)), static_keyword(KeywordAbility::Haste),
                capabilities(&[CastEnchantmentV1,AttachmentsV1,OptionalControllerDrawV1])),
            permanent(CardKind::Enchantment, &two, static_keyword(KeywordAbility::Trample),
                capabilities(&[CastEnchantmentV1,AttachmentsV1,ControllerDrawV1,
                    SelfEntersBattlefieldV1,InterveningIfV1]))
        ]}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = unlimited_hand_size::witness();
    d.static_abilities.clear();
    let mut program = d.spell.clone().unwrap();
    program.resolution = SpellResolution::Instructions;
    program.timing = SpellTiming::Instant;
    program.instructions = vec![Instruction::OptionalDrawCards { source_block: 0 }];
    d.triggered = vec![TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::ControlledCreatureEntersWithPower {
            minimum_power: 3.into(),
            other: true,
        },
        death_filter: None,
        intervening_if: None,
        program,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn power_entry_draw_uses_a_complete_frozen_source_shape() {
    let base = witness();
    base.validate().unwrap();
    let mut wrong = base.clone();
    wrong.triggered[0]
        .program
        .instructions
        .push(Instruction::OptionalDrawCards { source_block: 0 });
    wrong.capabilities = wrong.requirements();
    assert!(wrong.validate().is_err());
    let mut wrong = base;
    wrong.triggered[0].condition = TriggerCondition::SelfEntersBattlefield;
    wrong.triggered[0].intervening_if =
        Some(InterveningCondition::ControllerHasCreatureWithPower {
            minimum_power: 3.into(),
        });
    wrong.capabilities = wrong.requirements();
    assert!(wrong.validate().is_err());
}
