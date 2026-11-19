//! CR 608.2b/c/h, 701.6 and 701.17.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn contains_counter_mill(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::CounterSpellAndMillController { .. }))
        || p.modes
            .iter()
            .any(|mode| contains_counter_mill(&mode.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    const INVALID: &str = "counter and controller mill requires a sole ordinary instant and one unfiltered spell target";
    if d.activated
        .iter()
        .any(|a| contains_counter_mill(&a.program))
        || d.triggered
            .iter()
            .any(|a| contains_counter_mill(&a.program))
    {
        return Err(INVALID.into());
    }
    let Some(p) = d.spell.as_ref().filter(|p| contains_counter_mill(p)) else {
        return Ok(());
    };
    let c = &d.characteristics;
    if c.kind != CardKind::Instant
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || c.basic
        || c.legendary
        || c.snow
        || c.power.is_some()
        || c.toughness.is_some()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || p.timing != SpellTiming::Instant
        || p.resolution != SpellResolution::Instructions
        || p.functioning_zone != FunctioningZone::Stack
        || !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !matches!(p.targets.as_slice(), [t] if t.slot == 0 && t.selection_id == "line:0:1"
            && t.chooser == Chooser::Controller && t.stage == EvaluationStage::Announcement
            && t.domain == TargetDomain::StackSpell && t.filter.is_none())
        || !matches!(p.instructions.as_slice(),
            [Instruction::CounterSpellAndMillController { target_slot: 0, count, source_block: 0 }]
                if *count > Quantity::zero())
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let op = json!({"required":["op"],"properties":{"op":{"const":"counter_spell_and_mill_controller"}}});
    let has = json!({"type":"object","properties":{"instructions":{"contains":op}}});
    // Each nested mode is a SpellProgram, so this excludes the operation at any mode depth.
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"modes":{"items":{"properties":{"program":{"not":has}}}}},
        "if":has,"then":{"properties":{
            "timing":{"const":"instant"},"resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},
            "optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":op},
            "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
                "slot":{"const":0},"selection_id":{"const":"line:0:1"},"domain":{"const":"stack_spell"},
                "chooser":{"const":"controller"},"stage":{"const":"announcement"},"filter":{"type":"null"}
            }}}
        }}
    }));
    let caps = [
        NormalLayoutV1,
        CastInstantV1,
        StackSpellTargetV1,
        CounterStackObjectV1,
        CounterSpellControllerMillV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "activated":{"items":{"properties":{"program":{"not":has}}}},
            "triggered":{"items":{"properties":{"program":{"not":has}}}}
        },
        "if":{"required":["spell"],"properties":{"spell":has}},
        "then":{"properties":{
            "characteristics":{"properties":{
                "kind":{"const":"instant"},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "additional_types":{"maxItems":0},"subtypes":{"maxItems":0},"power":{"type":"null"},"toughness":{"type":"null"},
                "colors":{"items":{"not":{"const":"C"}}},"color_identity":{"items":{"not":{"const":"C"}}},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}
            }},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},
            "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
            "capabilities":{"uniqueItems":true,"minItems":caps.len(),"maxItems":caps.len(),"items":{"enum":caps}}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":CounterSpellControllerMillV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Counterspell")
    .unwrap();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::CounterSpellAndMillController {
        target_slot: 0,
        count: 3.into(),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn counter_spell_controller_mill_exact_capabilities_and_shape() {
    let d = witness();
    d.validate().unwrap();
    use Capability::*;
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastInstantV1,
            CounterStackObjectV1,
            StackSpellTargetV1,
            CounterSpellControllerMillV1
        ])
    );
    for cap in &d.capabilities {
        let mut bad = d.clone();
        bad.capabilities.remove(cap);
        assert!(bad.validate().is_err());
    }
    let mut spurious = d.clone();
    spurious.capabilities.insert(FixedMillProgramV1);
    assert!(spurious.validate().is_err());

    let mutations: &[fn(&mut Definition)] = &[
        |d| d.characteristics.kind = CardKind::Sorcery,
        |d| d.x_mana_cost = true,
        |d| d.keywords.push(KeywordAbility::Flash),
        |d| d.spell.as_mut().unwrap().targets[0].selection_id = "line:0:2".into(),
        |d| {
            d.spell.as_mut().unwrap().targets[0].domain =
                TargetDomain::StackActivatedOrTriggeredAbility
        },
        |d| {
            let p = d.spell.as_mut().unwrap();
            p.targets.push(p.targets[0].clone());
        },
        |d| {
            d.spell.as_mut().unwrap().targets[0].filter = Some(PermanentFilter {
                types: vec![CardKind::Creature],
                non_types: vec![],
                non_colors: vec![],
                tapped: false,
                opponent: false,
            })
        },
        |d| {
            d.spell
                .as_mut()
                .unwrap()
                .instructions
                .push(Instruction::CounterStackObject {
                    target_slot: 0,
                    source_block: 0,
                })
        },
        |d| {
            let p = d.spell.as_mut().unwrap();
            let child = p.clone();
            p.instructions.clear();
            p.targets.clear();
            p.modes.push(SpellMode { program: child });
        },
        |d| {
            d.spell.as_mut().unwrap().additional_sacrifice = Some(SacrificeFilter {
                types: vec![CardKind::Creature],
                subtypes: vec![],
            })
        },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut bad = d.clone();
        mutate(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "mutation {index}");
    }
    for (count, target_slot, source_block, valid) in [
        ("1", 0, 0, true),
        ("999999999999999999999999", 0, 0, true),
        ("0", 0, 0, false),
        ("-1", 0, 0, false),
        ("3", 1, 0, false),
        ("3", 0, 1, false),
    ] {
        let mut d = d.clone();
        d.spell.as_mut().unwrap().instructions[0] = Instruction::CounterSpellAndMillController {
            count: count.parse().unwrap(),
            target_slot,
            source_block,
        };
        assert_eq!(d.validate().is_ok(), valid);
    }
}
