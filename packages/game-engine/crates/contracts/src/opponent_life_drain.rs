//! CR 119.3 and 608.2c/h. The controller gains the total life lost to the resolving spell.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn has_drain(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| i.drain_amount().is_some())
        || p.modes.iter().any(|m| has_drain(&m.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_drain(&a.program))
        || d.triggered.iter().any(|a| has_drain(&a.program))
    {
        return Err("opponent drain requires a sorcery spell".into());
    }
    let Some(p) = d.spell.as_ref().filter(|p| has_drain(p)) else {
        return Ok(());
    };
    let [
        instruction @ Instruction::Kernel {
            source_block: 0, ..
        },
    ] = p.instructions.as_slice()
    else {
        return Err("opponent drain requires a sole instruction from block zero".into());
    };
    let valid_amount = match instruction.drain_amount() {
        Some(fixed!(value)) => *value > Quantity::zero() && !d.x_mana_cost,
        Some(amount) => amount.is_announced_x() && d.x_mana_cost,
        None => false,
    };
    let c = &d.characteristics;
    if !valid_amount
        || c.kind != CardKind::Sorcery
        || !c.subtypes.is_empty()
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || d.intrinsic_zone.is_some()
        || !p.targets.is_empty()
        || !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
    {
        return Err("unsupported complete opponent drain sorcery".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use Capability::*;
    use serde_json::json;
    let drain = instruction_schema::kernel_seq();
    let free = json!({"$ref":"#/$defs/OpponentLifeDrainFreeProgram"});
    schema["$defs"]["OpponentLifeDrainFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":drain}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"not":free},"then":{"properties":{
            "timing":{"const":SpellTiming::MainPhase},"functioning_zone":{"const":"stack"},
            "resolution":{"const":"instructions"},"targets":{"maxItems":0},"modes":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":drain}
        }}
    }));
    let caps = [
        NormalLayoutV1,
        CastSorceryV1,
        OpponentLifeDrainV1,
        LifeLossV1,
        LifeGainV1,
    ];
    let required_caps: Vec<_> = caps
        .iter()
        .map(|c| json!({"contains":{"const":c}}))
        .collect();
    // The shared announced-X schema pairs the operand with x_mana_cost.
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({"properties":{
            "activated":{"items":{"properties":{"program":free}}},
            "triggered":{"items":{"properties":{"program":free}}}
        }}),
        json!({"if":{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
            "then":{"properties":{
                "capabilities":{"maxItems":caps.len(),"allOf":required_caps},
                "intrinsic_zone":{"type":"null"},"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
                "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},
                "characteristics":{"properties":{
                    "kind":{"const":CardKind::Sorcery},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                    "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                    "power":{"type":"null"},"toughness":{"type":"null"},
                    "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
                }}
            }},"else":{"properties":{"capabilities":{"not":{"contains":{"const":OpponentLifeDrainV1}}}}}
        }),
    ]);
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = controller_library::witness();
    d.characteristics.kind = CardKind::Sorcery;
    let p = d.spell.as_mut().unwrap();
    p.timing = SpellTiming::MainPhase;
    p.instructions = vec![Instruction::drain(Context::Spell, kir::Amount::fixed(3), 0)];
    d.capabilities = d.requirements();
    d
}

#[test]
fn opponent_life_drain() {
    use Capability::*;
    use serde_json::json;
    let fixed = |n: i64| kir::Amount::fixed(n);
    for (amount, x, valid) in [
        (fixed(3), false, true),
        (kir::Amount::announced_x(), true, true),
        (fixed(1), true, false),
        (kir::Amount::announced_x(), false, false),
        (fixed(0), false, false),
        (fixed(-1), false, false),
    ] {
        let mut d = witness();
        d.spell.as_mut().unwrap().instructions =
            vec![Instruction::drain(Context::Spell, amount, 0)];
        d.x_mana_cost = x;
        assert_eq!(d.validate().is_ok(), valid);
        if valid {
            let op = &d.spell.as_ref().unwrap().instructions[0];
            assert_eq!(
                serde_json::from_value::<Instruction>(serde_json::to_value(op).unwrap()).unwrap(),
                *op
            );
            assert_eq!(
                d.requirements(),
                BTreeSet::from([
                    NormalLayoutV1,
                    CastSorceryV1,
                    LifeGainV1,
                    LifeLossV1,
                    OpponentLifeDrainV1
                ])
            );
            for cap in [LifeGainV1, LifeLossV1, OpponentLifeDrainV1] {
                let mut missing = d.clone();
                missing.capabilities.remove(&cap);
                assert!(missing.validate().is_err());
            }
        }
    }
    let mutations: &[fn(&mut Definition)] =
        &[
            |d| d.characteristics.kind = CardKind::Instant,
            |d| d.characteristics.subtypes.push("Arcane".into()),
            |d| d.keywords.push(KeywordAbility::Flash),
            |d| d.spell.as_mut().unwrap().timing = SpellTiming::Instant,
            |d| {
                d.spell.as_mut().unwrap().targets =
                    targeted_counted_draw::witness().spell.unwrap().targets
            },
            |d| {
                d.spell.as_mut().unwrap().optional_additional_costs.push(
                    OptionalAdditionalManaCost {
                        identity: "kicker".into(),
                        source_block: 0,
                        mana: d.characteristics.mana_cost.clone(),
                    },
                )
            },
            |d| {
                let p = d.spell.as_mut().unwrap();
                p.instructions.push(p.instructions[0].clone());
            },
            |d| {
                let p = d.spell.as_mut().unwrap();
                let mode = SpellMode { program: p.clone() };
                p.instructions.clear();
                p.modes = vec![mode; 2];
            },
            |d| {
                if let Instruction::Kernel { source_block, .. } =
                    &mut d.spell.as_mut().unwrap().instructions[0]
                {
                    *source_block = 1;
                }
            },
        ];
    for mutate in mutations {
        let mut d = witness();
        mutate(&mut d);
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    // Kernel operands are strict: no unused operand, no legacy spelling.
    let pair = |amount: serde_json::Value, extra: serde_json::Value| {
        let mut wire =
            serde_json::to_value(Instruction::drain(Context::Spell, kir::Amount::fixed(3), 0))
                .unwrap();
        wire["node"]["seq"][0]["do"]["amount"] = amount;
        if let Some(extra) = extra.as_object() {
            wire.as_object_mut().unwrap().extend(extra.clone());
        }
        wire
    };
    assert!(
        serde_json::from_value::<Instruction>(pair(
            json!({"amount":"fixed","value":"3"}),
            json!({})
        ))
        .is_ok()
    );
    for wire in [
        pair(
            json!({"amount":"var","of":{"ref":"announced_x"},"value":"1"}),
            json!({}),
        ),
        pair(
            json!({"amount":"fixed","value":"3"}),
            json!({"target_slot":0}),
        ),
        json!({"op":"drain_opponents","amount":{"amount":"fixed","value":"3"},"source_block":0}),
    ] {
        assert!(serde_json::from_value::<Instruction>(wire).is_err());
    }
    // The pair is the only audited sequence: any other one is rejected.
    for (first, second) in [(0, 0), (1, 1)] {
        let mut d = witness();
        let Instruction::Kernel {
            node: kir::Node::Seq(steps),
            ..
        } = &mut d.spell.as_mut().unwrap().instructions[0]
        else {
            panic!("drain pair");
        };
        *steps = vec![steps[first].clone(), steps[second].clone()];
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
}
