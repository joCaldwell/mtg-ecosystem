//! CR 702.29a, 602.2a-b, 701.9a (2026-09-25): fixed mana, discard
//! this hand source, draw one. No cycling observers/modifiers are admitted.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub fn hand_discard_draw_activation(a: &ActivatedAbility) -> bool {
    let p = &a.program;
    a.functioning_zone == FunctioningZone::Hand
        && matches!(&a.cost, ActivationCost::DiscardSource { mana } if !mana.generic.is_negative())
        && p.functioning_zone == FunctioningZone::Stack
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.optional_additional_costs.is_empty()
        && p.additional_sacrifice.is_none()
        && p.targets.is_empty()
        && p.modes.is_empty()
        && matches!(p.instructions.as_slice(), [Instruction::DrawCards { recipient: None, count, .. }] if *count == Quantity::from(1))
}

// Complete source composition currently qualified for lowering. Neither the
// subtype count nor the mana quantity selects a mechanic or a capacity.
pub(super) fn definition(d: &Definition) -> bool {
    land_definition(d) || spell_definition(d)
}

fn land_definition(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Land
        && !d.characteristics.basic
        && !d.characteristics.subtypes.is_empty()
        && d.characteristics
            .subtypes
            .iter()
            .all(|s| basic_land_color(s).is_some())
        && d.mana_abilities.is_empty()
        && d.triggered.is_empty()
        && d.keywords.is_empty()
        && matches!(d.activated.as_slice(), [a] if hand_discard_draw_activation(a))
        && matches!(
            d.static_abilities.as_slice(),
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTapped
            }]
        )
}

fn spell_definition(d: &Definition) -> bool {
    matches!(
        d.characteristics.kind,
        CardKind::Instant | CardKind::Sorcery
    ) && !d.x_mana_cost
        && d.characteristics.hybrid_mana.is_empty()
        && d.mana_abilities.is_empty()
        && d.static_abilities.is_empty()
        && d.triggered.is_empty()
        && d.keywords.is_empty()
        && matches!(d.activated.as_slice(), [a] if hand_discard_draw_activation(a)
            && matches!(a.program.instructions.as_slice(), [Instruction::DrawCards { source_block: 1, .. }]))
        && d.spell.as_ref().is_some_and(|p| {
            p.resolution == SpellResolution::Instructions
                && p.modes.is_empty()
                && p.additional_sacrifice.is_none()
                && p.optional_additional_costs.is_empty()
        })
}

pub(super) fn validate_spell_source(d: &Definition) -> Result<(), String> {
    use serde_json::{Value, json};
    let active = d.activated.iter().any(|a| {
        a.functioning_zone == FunctioningZone::Hand
            || matches!(a.cost, ActivationCost::DiscardSource { .. })
    });
    if !matches!(
        d.characteristics.kind,
        CardKind::Instant | CardKind::Sorcery
    ) || (!active && !d.provenance.source_json.contains("Cycling {"))
    {
        return Ok(());
    }
    let bad = || "requires one ordinary spell instruction and exact fixed-mana cycling".to_string();
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or_else(bad)?;
    if !active && !text.lines().any(|line| line.starts_with("Cycling {")) {
        return Ok(());
    }
    if !spell_definition(d) {
        return Err(bad());
    }
    let ActivationCost::DiscardSource { mana } = &d.activated[0].cost else {
        return Err(bad());
    };
    let cost = mana.printed_fixed_cost();
    let keyword = format!("Cycling {cost}");
    let reminder = format!("{keyword} ({cost}, Discard this card: Draw a card.)");
    let Some((body, cycling)) = text.split_once('\n') else {
        return Err(bad());
    };
    let symbols: Vec<_> = cost
        .split_terminator('}')
        .map(|s| s.trim_start_matches('{'))
        .collect();
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let step = &ast[0]["effects"][0]["steps"][0];
    let c = &d.characteristics;
    let type_line = if c.kind == CardKind::Instant {
        "Instant"
    } else {
        "Sorcery"
    };
    if body.is_empty()
        || (cycling != keyword && cycling != reminder)
        || step["effect"].as_str().is_none()
        || ast
            != json!([
                {"kind":"spell","effects":[{"steps":[step]}]},
                {"kind":"keywords","keywords":[{"keyword":"cycling","cost":[{"cost":"mana","symbols":symbols}]}]}
            ])
        || source["layout"] != "normal"
        || source["type_line"] != type_line
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
    {
        return Err(bad());
    }
    Ok(())
}

pub(super) fn schema(s: &mut serde_json::Value) {
    use serde_json::json;
    s["$defs"]["HandDiscardDrawActivation"] = json!({
        "type": "object",
        "required": [
            "functioning_zone",
            "cost",
            "program"
        ],
        "properties": {
            "functioning_zone": {
                "const": "hand"
            },
            "cost": {
                "type": "object",
                "required": [
                    "discard_source"
                ],
                "properties": {
                    "discard_source": {
                        "type": "object",
                        "required": [
                            "mana"
                        ],
                        "properties": {
                            "mana": {
                                "type": "object",
                                "properties": {
                                    "generic": {
                                        "pattern": "^(0|[1-9][0-9]*)$"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "program": {
                "properties": {
                    "functioning_zone": {
                        "const": "stack"
                    },
                    "timing": {
                        "const": "instant"
                    },
                    "resolution": {
                        "const": "instructions"
                    },
                    "targets": {
                        "maxItems": 0
                    },
                    "modes": {
                        "maxItems": 0
                    },
                    "additional_sacrifice": {
                        "type": "null"
                    },
                    "optional_additional_costs": {
                        "maxItems": 0
                    },
                    "instructions": {
                        "minItems": 1,
                        "maxItems": 1,
                        "items": {
                            "properties": {
                                "op": {
                                    "const": "draw_cards"
                                },
                                "count": {
                                    "const": "1"
                                },
                                "recipient": {
                                    "type": "null"
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    let hand =
        json!({"required":["functioning_zone"],"properties":{"functioning_zone":{"const":"hand"}}});
    let discard = json!({"required":["cost"],"properties":{"cost":{"type":"object","required":["discard_source"]}}});
    s["$defs"]["ActivatedAbility"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"anyOf":[hand.clone(),discard.clone()]},"then":{"$ref":"#/$defs/HandDiscardDrawActivation"}}));
    for (present, cap) in [
        (hand, Capability::HandActivationV1),
        (discard.clone(), Capability::DiscardSourceCostV1),
    ] {
        let present =
            json!({"required":["activated"],"properties":{"activated":{"contains":present}}});
        s["$defs"]["Definition"]["allOf"]
            .as_array_mut()
            .unwrap()
            .push(json!({"if":present,
                "then":{"properties":{"capabilities":{"contains":{"const":cap}}}},
                "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
            }));
    }
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"required":["activated"],"properties":{"activated":{"contains":discard}}},"then":{"properties":{
        "activated":{"minItems":1,"maxItems":1,"items":{"$ref":"#/$defs/HandDiscardDrawActivation"}},
        "mana_abilities":{"maxItems":0},"triggered":{"maxItems":0},"keywords":{"maxItems":0},
        "capabilities":{"allOf":[{"contains":{"const":Capability::ActivatedAbilityV1}},{"contains":{"const":Capability::ManaActivationCostV1}},{"contains":{"const":Capability::ControllerDrawV1}}]}
    },"anyOf":[
        {"properties":{
            "characteristics":{"properties":{"kind":{"const":"land"},"basic":{"const":false},"subtypes":{"minItems":1}}},
            "static_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{"functioning_zone":{"const":"battlefield"},"effect":{"const":"enters_tapped"}}}}
        }},
        {"required":["spell"],"properties":{
            "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},
            "static_abilities":{"maxItems":0},
            "x_mana_cost":{"const":false},
            "spell":{"type":"object","properties":{
                "resolution":{"const":"instructions"},"modes":{"maxItems":0},
                "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}
            }}
        }}
    ]}}));
    // Spell, trigger and intrinsic mana contexts remain outside hand.
    // The opening placement contract owns the static hand exception.
    for kind in ["SpellProgram", "TriggeredAbility"] {
        s["$defs"][kind]
            .as_object_mut()
            .unwrap()
            .entry("allOf")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .unwrap()
            .push(json!({"properties":{"functioning_zone":{"not":{"const":"hand"}}}}));
    }
    s["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"properties":{"intrinsic_zone":{"not":{"const":"hand"}}}}));
}

#[cfg(test)]
pub(crate) fn witness() -> Definition {
    // Synthetic contract control; the compiler separately checks frozen source.
    let mut d = conditional_lands::tests::land();
    d.static_abilities[0].effect = StaticEffect::EntersTapped;
    d.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Hand,
        cost: ActivationCost::DiscardSource {
            mana: ManaCost {
                generic: 3.into(),
                colored: vec![],
            },
        },
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            modes: vec![],
            instructions: vec![Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 1,
            }],
        },
    }];
    d.capabilities = d.requirements();
    d
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn cycling_contract_exact_shape_quantities_and_capabilities() {
        for n in ["0", "3", "17", "999999999999999999999999999999999999999999"] {
            let mut d = witness();
            if let ActivationCost::DiscardSource { mana } = &mut d.activated[0].cost {
                mana.generic = n.parse().unwrap();
                mana.colored = vec![Color::U, Color::C];
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            let v = serde_json::to_value(&d).unwrap();
            assert_eq!(v["activated"][0]["functioning_zone"], "hand");
            assert_eq!(
                v["activated"][0]["cost"],
                json!({"discard_source":{"mana":{"generic":n,"colored":["U","C"]}}})
            );
            assert_eq!(strict_read::<Definition>(&v.to_string()).unwrap(), d);
            for cap in [
                Capability::HandActivationV1,
                Capability::DiscardSourceCostV1,
                Capability::ManaActivationCostV1,
                Capability::ControllerDrawV1,
            ] {
                assert!(d.capabilities.contains(&cap));
                let mut missing = d.clone();
                missing.capabilities.remove(&cap);
                assert!(missing.validate().is_err());
            }
        }
    }
    #[test]
    fn cycling_contract_rejects_partial_wrong_zone_cost_and_draw() {
        for case in 0..17 {
            let mut d = witness();
            match case {
                0 => d.activated[0].functioning_zone = FunctioningZone::Battlefield,
                1 => d.activated[0].functioning_zone = FunctioningZone::Stack,
                2 => {
                    d.activated[0].cost = ActivationCost::Mana {
                        cost: ManaCost {
                            generic: 3.into(),
                            colored: vec![],
                        },
                    }
                }
                3 => {
                    if let ActivationCost::DiscardSource { mana } = &mut d.activated[0].cost {
                        mana.generic = (-1).into()
                    }
                }
                4 => d.activated[0].program.instructions.clear(),
                5 => {
                    if let Instruction::DrawCards { count, .. } =
                        &mut d.activated[0].program.instructions[0]
                    {
                        *count = 2.into()
                    }
                }
                6 => {
                    if let Instruction::DrawCards { recipient, .. } =
                        &mut d.activated[0].program.instructions[0]
                    {
                        *recipient = Some(LifeRecipient::Controller)
                    }
                }
                7 => d.activated[0]
                    .program
                    .instructions
                    .push(Instruction::DiscardCards {
                        recipient: None,
                        count: 1.into(),
                        source_block: 1,
                    }),
                8 => d.activated[0].program.functioning_zone = FunctioningZone::Hand,
                9 => d.static_abilities[0].functioning_zone = FunctioningZone::Hand,
                10 => d.intrinsic_zone = Some(FunctioningZone::Hand),
                11 => {
                    d.static_abilities[0].effect =
                        StaticEffect::EntersTappedUnlessPayLife { amount: 2.into() }
                }
                12 => d.static_abilities.clear(),
                13 => d.activated.push(d.activated[0].clone()),
                14 => d.characteristics.kind = CardKind::Creature,
                15 => d.activated[0].program.timing = SpellTiming::MainPhase,
                _ => d.x_mana_cost = true,
            }
            d.capabilities = d.requirements();
            assert!(d.validate().is_err(), "{case}");
        }
        for cap in [
            Capability::HandActivationV1,
            Capability::ActivatedAbilityV1,
            Capability::ManaActivationCostV1,
            Capability::ControllerDrawV1,
        ] {
            let mut spare = conditional_lands::tests::land();
            spare.capabilities.insert(cap);
            assert!(spare.validate().is_err(), "noncycling spare {cap:?}");
        }
    }
    #[test]
    fn cycling_contract_strict_null_unknown_and_quantity_spellings() {
        for value in [
            json!(null),
            json!(3),
            json!("03"),
            json!("3.0"),
            json!("-0"),
            json!({}),
        ] {
            let mut v = serde_json::to_value(witness()).unwrap();
            v["activated"][0]["cost"]["discard_source"]["mana"]["generic"] = value;
            assert!(strict_read::<Definition>(&v.to_string()).is_err());
        }
        for cost in [
            json!({"discard_source":null}),
            json!({"discard_source":{}}),
            json!({"discard_source":{"mana":null}}),
            json!({"discard_source":{"mana":{"generic":"3","colored":[]},"card":"another"}}),
        ] {
            let mut v = serde_json::to_value(witness()).unwrap();
            v["activated"][0]["cost"] = cost;
            assert!(strict_read::<Definition>(&v.to_string()).is_err());
        }
    }
}
