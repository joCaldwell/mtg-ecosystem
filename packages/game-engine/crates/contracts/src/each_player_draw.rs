//! CR 121.2c: every player draws the indicated number of cards in APNAP order.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl<'de> Deserialize<'de> for DrawEachCount {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // An empty struct variant enforces the operand-free X on the wire.
        #[derive(Deserialize)]
        #[serde(tag = "amount", rename_all = "snake_case", deny_unknown_fields)]
        enum Count {
            Fixed { value: Quantity },
            AnnouncedX {},
        }
        Ok(match Count::deserialize(d)? {
            Count::Fixed { value } => Self::Fixed { value },
            Count::AnnouncedX {} => Self::AnnouncedX,
        })
    }
}

fn has_draw(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::DrawEachPlayer { .. }))
        || p.modes.iter().any(|m| has_draw(&m.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_draw(&a.program))
        || d.triggered.iter().any(|a| has_draw(&a.program))
    {
        return Err("each-player draw requires an ordinary spell".into());
    }
    let Some(p) = d.spell.as_ref().filter(|p| has_draw(p)) else {
        return Ok(());
    };
    let [
        Instruction::DrawEachPlayer {
            count,
            source_block: 0,
        },
    ] = p.instructions.as_slice()
    else {
        return Err("each-player draw requires a sole instruction from block zero".into());
    };
    let valid_count = match count {
        DrawEachCount::Fixed { value } => !d.x_mana_cost && *value > Quantity::zero(),
        DrawEachCount::AnnouncedX => d.x_mana_cost,
    };
    // Common definition checks enforce cast timing, stack zone, instruction
    // resolution, characteristic consistency, and the exact capability closure.
    let c = &d.characteristics;
    if !valid_count
        || !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
        || !c.subtypes.is_empty()
        || c.snow
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
        return Err("unsupported complete each-player draw spell".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let cap = Capability::EachPlayerDrawV1;
    let op = "draw_each_player";
    let free = json!({"$ref":"#/$defs/EachPlayerDrawFreeProgram"});
    schema["$defs"]["EachPlayerDrawFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"properties":{"op":{"const":op}}}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"not":free},"then":{"properties":{
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},
            "optional_additional_costs":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":op}}}}
        }}
    }));
    let timing_and_caps = |timing, cast| {
        json!({"properties":{
            "spell":{"properties":{"timing":{"const":timing}}},
            "capabilities":{"maxItems":3,"allOf":[{"contains":{"const":cap}},{"contains":{"const":cast}},{"contains":{"const":Capability::NormalLayoutV1}}]}
        }})
    };
    // The shared X schema checks that the paid X flag and count variant agree.
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({"properties":{
            "activated":{"items":{"properties":{"program":free}}},
            "triggered":{"items":{"properties":{"program":free}}}
        }}),
        json!({"if":{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
            "then":{"properties":{
                "intrinsic_zone":{"type":"null"},"keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
                "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},
                "characteristics":{"properties":{
                    "kind":{"enum":["instant","sorcery"]},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                    "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                    "power":{"type":"null"},"toughness":{"type":"null"},
                    "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
                }}
            },"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
            "then":timing_and_caps(SpellTiming::Instant,Capability::CastInstantV1),
            "else":timing_and_caps(SpellTiming::MainPhase,Capability::CastSorceryV1)},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
        }),
    ]);
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = controller_library::witness();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::DrawEachPlayer {
        count: DrawEachCount::Fixed { value: 2.into() },
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn each_player_draw() {
    use serde_json::json;
    let fixed = |n: i64| DrawEachCount::Fixed { value: n.into() };
    for (count, x, valid) in [
        (fixed(2), false, true),
        (fixed(1), false, true),
        (DrawEachCount::AnnouncedX, true, true),
        (DrawEachCount::AnnouncedX, false, false),
        (fixed(2), true, false),
        (fixed(0), false, false),
        (fixed(-1), false, false),
    ] {
        let mut d = witness();
        d.x_mana_cost = x;
        d.spell.as_mut().unwrap().instructions = vec![Instruction::DrawEachPlayer {
            count: count.clone(),
            source_block: 0,
        }];
        assert_eq!(d.validate().is_ok(), valid);
        if valid {
            assert_eq!(
                serde_json::from_value::<DrawEachCount>(serde_json::to_value(&count).unwrap())
                    .unwrap(),
                count
            );
            assert_eq!(
                d.requirements(),
                BTreeSet::from([
                    Capability::CastInstantV1,
                    Capability::NormalLayoutV1,
                    Capability::EachPlayerDrawV1
                ])
            );
            assert!(d.capabilities.remove(&Capability::EachPlayerDrawV1));
            assert!(d.validate().is_err());
        }
    }
    let mutations: &[fn(&mut Definition)] = &[
        |d| {
            d.spell.as_mut().unwrap().targets =
                targeted_counted_draw::witness().spell.unwrap().targets
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
            let Instruction::DrawEachPlayer { source_block, .. } =
                &mut d.spell.as_mut().unwrap().instructions[0]
            else {
                unreachable!()
            };
            *source_block = 1;
        },
        |d| d.characteristics.subtypes.push("Arcane".into()),
        |d| d.keywords.push(KeywordAbility::Flash),
    ];
    for mutate in mutations {
        let mut d = witness();
        mutate(&mut d);
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    for wire in [
        json!({"op":"draw_each_player","count":{"amount":"announced_x","value":"1"},"source_block":0}),
        json!({"op":"draw_each_player","count":{"amount":"fixed","value":"1"},"source_block":0,"recipient":"event_player"}),
    ] {
        assert!(serde_json::from_value::<Instruction>(wire).is_err());
    }
}
