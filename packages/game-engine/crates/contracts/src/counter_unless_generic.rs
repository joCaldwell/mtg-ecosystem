//! CR 118.12a, 608.2d, 605.3a and 701.6 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn has_conditional_counter(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::CounterSpellUnlessPays { .. }))
        || p.modes.iter().any(|m| has_conditional_counter(&m.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated
        .iter()
        .any(|a| has_conditional_counter(&a.program))
        || d.triggered
            .iter()
            .any(|a| has_conditional_counter(&a.program))
    {
        return Err("conditional counter requires an ordinary instant".into());
    }
    let Some(p) = d.spell.as_ref().filter(|p| has_conditional_counter(p)) else {
        return Ok(());
    };
    let c = &d.characteristics;
    if c.kind != CardKind::Instant
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || c.snow
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || !matches!(p.targets.as_slice(), [t] if t.domain == TargetDomain::StackSpell && t.filter.is_none())
        || !matches!(p.instructions.as_slice(), [Instruction::CounterSpellUnlessPays {
            target_slot: 0, generic, source_block: 0
        }] if *generic > Quantity::zero())
    {
        return Err("unsupported complete conditional counter instant".into());
    }
    // Shared instant and instruction validation owns timing, zone, and slot binding.
    Ok(())
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use Capability::*;
    use serde_json::json;
    let op = "counter_spell_unless_pays";
    let free = json!({"$ref":"#/$defs/CounterUnlessGenericFreeProgram"});
    schema["$defs"]["CounterUnlessGenericFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"properties":{"op":{"const":op}}}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"not":free},"then":{"properties":{
            "timing":{"const":SpellTiming::Instant},"functioning_zone":{"const":"stack"},
            "resolution":{"const":"instructions"},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},
            "optional_additional_costs":{"maxItems":0},
            "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
                "slot":{"const":0},"selection_id":{"minLength":1},"chooser":{"const":"controller"},
                "stage":{"const":"announcement"},"domain":{"const":TargetDomain::StackSpell},"filter":{"type":"null"}
            }}},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":op}}}}
        }}
    }));
    let caps = [
        NormalLayoutV1,
        CastInstantV1,
        CounterUnlessGenericV1,
        CounterStackObjectV1,
        StackSpellTargetV1,
    ];
    let required_caps: Vec<_> = caps
        .iter()
        .map(|c| json!({"contains":{"const":c}}))
        .collect();
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "activated":{"items":{"properties":{"program":free}}},
            "triggered":{"items":{"properties":{"program":free}}}
        },
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
        "then":{"properties":{
            "capabilities":{"maxItems":caps.len(),"allOf":required_caps},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "characteristics":{"properties":{
                "kind":{"const":CardKind::Instant},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "power":{"type":"null"},"toughness":{"type":"null"},
                "colors":{"items":{"not":{"const":Color::C}}},"color_identity":{"items":{"not":{"const":Color::C}}},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
            }}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":CounterUnlessGenericV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Counterspell")
    .unwrap();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::CounterSpellUnlessPays {
        target_slot: 0,
        generic: 3.into(),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn counter_unless_generic_closed_ir() {
    use Capability::*;
    use serde_json::json;
    let d = witness();
    d.validate().unwrap();
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastInstantV1,
            CounterUnlessGenericV1,
            CounterStackObjectV1,
            StackSpellTargetV1
        ])
    );
    for cap in &d.capabilities {
        let mut bad = d.clone();
        bad.capabilities.remove(cap);
        assert!(bad.validate().is_err());
    }
    let op =
        json!({"op":"counter_spell_unless_pays","target_slot":0,"generic":"3","source_block":0});
    assert_eq!(
        serde_json::to_value(&d.spell.as_ref().unwrap().instructions[0]).unwrap(),
        op
    );
    let mut extra = op.clone();
    extra["payer"] = json!("controller");
    assert!(serde_json::from_value::<Instruction>(extra).is_err());
    for (field, value) in [
        ("generic", json!("0")),
        ("generic", json!("-1")),
        ("target_slot", json!(1)),
        ("source_block", json!(1)),
    ] {
        let mut bad = d.clone();
        let mut changed = op.clone();
        changed[field] = value;
        bad.spell.as_mut().unwrap().instructions[0] = serde_json::from_value(changed).unwrap();
        assert!(bad.validate().is_err());
    }
    for mutate in [
        (|d: &mut Definition| d.characteristics.kind = CardKind::Sorcery) as fn(&mut Definition),
        |d| d.characteristics.subtypes.push("Lesson".into()),
        |d| d.x_mana_cost = true,
        |d| d.keywords.push(KeywordAbility::Flash),
        |d| {
            d.spell.as_mut().unwrap().targets[0].domain =
                TargetDomain::StackActivatedOrTriggeredAbility
        },
        |d| {
            d.spell.as_mut().unwrap().targets[0].filter = Some(PermanentFilter {
                types: vec![CardKind::Instant],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false,
                power: None,
            })
        },
        |d| {
            let p = d.spell.as_mut().unwrap();
            p.instructions.push(p.instructions[0].clone());
        },
        |d| {
            let p = d.spell.as_mut().unwrap();
            let mode = SpellMode { program: p.clone() };
            p.modes = vec![mode; 2];
            p.targets.clear();
            p.instructions.clear();
        },
        |d| {
            d.spell.as_mut().unwrap().additional_sacrifice = Some(SacrificeFilter {
                types: vec![CardKind::Creature],
                subtypes: vec![],
            })
        },
        |d| {
            d.spell
                .as_mut()
                .unwrap()
                .optional_additional_costs
                .push(OptionalAdditionalManaCost {
                    identity: "kicker".into(),
                    source_block: 0,
                    mana: d.characteristics.mana_cost.clone(),
                })
        },
    ] {
        let mut bad = d.clone();
        mutate(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
}
