//! CR 601.2b–d, 608.2b. Only the chosen mode supplies targets. The division
//! survives legality changes; each still-legal creature gets its announced share.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

/// Complete choose-one instant containing the supported divided-counter operation.
pub fn divided_plus_one_counters_modal_program() -> SpellProgram {
    let program = |targets, instructions| SpellProgram {
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets,
        instructions,
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    };
    let target = |selection: &str, domain, filter| TargetSpec {
        slot: 0,
        selection_id: selection.into(),
        stage: EvaluationStage::Announcement,
        chooser: Chooser::Controller,
        domain,
        filter,
    };
    let exile = program(
        vec![target(
            "line:1:2",
            TargetDomain::BattlefieldPermanent,
            Some(PermanentFilter {
                types: vec![CardKind::Creature],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false,
                power: Some(CreaturePowerFilter::AtLeast { value: 3.into() }),
            }),
        )],
        vec![Instruction::move_target(0, kir::Destination::Exile, 0)],
    );
    let draw = Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 0);
    let draw_loss = program(
        vec![],
        vec![
            draw.clone(),
            draw,
            Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::fixed(2),
                },
                0,
            ),
        ],
    );
    let distribute = program(
        vec![target("line:3:13", TargetDomain::BattlefieldCreature, None)],
        vec![Instruction::DistributePlusOneCounters {
            count: 2.into(),
            minimum_targets: 1,
            maximum_targets: 2,
            source_block: 0,
        }],
    );
    let mut root = program(vec![], vec![]);
    root.modes = [exile, draw_loss, distribute]
        .map(|program| SpellMode { program })
        .into();
    root
}

pub(super) const CAPABILITIES: [Capability; 10] = {
    use Capability::*;
    [
        NormalLayoutV1,
        CastInstantV1,
        ModalChoiceV1,
        PowerFilteredRemovalV1,
        PermanentRemovalV1,
        ControllerDrawV1,
        LifeLossV1,
        CreatureTargetV1,
        PlusOneCountersV1,
        DividedPlusOneCountersV1,
    ]
};

// Use the canonical operands for each ordered child. Allow the same explicit
// defaults as serde (no cost, no filter, ordinary controller draw).
pub(super) fn extend_schema(schema: &mut Value) {
    fn ordered(items: Vec<Value>) -> Value {
        if items.is_empty() {
            return json!({"maxItems":0});
        }
        json!({"minItems":items.len(),"prefixItems":items,"items":false})
    }
    fn program(p: &SpellProgram) -> Value {
        let targets = p.targets.iter().map(|t| {
            let filter = if let Some(f) = &t.filter {
                json!({"type":"object","required":["power"],"properties":{
                    "power":{"const":f.power},"types":{"const":f.types},"non_types":{"const":[]},
                    "non_colors":{"const":[]},"opponent":{"const":false},"tapped":{"const":false}
                }})
            } else {json!({"type":"null"})};
            let mut target = json!({"properties":{"slot":{"const":t.slot},"selection_id":{"const":t.selection_id},
                "stage":{"const":t.stage},"chooser":{"const":t.chooser},"domain":{"const":t.domain},"filter":filter}});
            if t.filter.is_some() { target["required"] = json!(["filter"]); }
            target
        }).collect();
        let instructions = p
            .instructions
            .iter()
            .map(|i| match i {
                kernel!(Draw { player: you!(), amount: fixed!(count) } @ source_block)
                    if *count == Quantity::from(1) =>
                {
                    instruction_schema::at(instruction_schema::single_draw(), *source_block)
                }
                _ => json!({"const":i}),
            })
            .collect();
        let modes = p
            .modes
            .iter()
            .map(|m| json!({"properties":{"program":program(&m.program)}}))
            .collect();
        let mut shape = json!({"type":"object","properties":{"timing":{"const":p.timing},"resolution":{"const":p.resolution},
            "functioning_zone":{"const":p.functioning_zone},"targets":ordered(targets),"instructions":ordered(instructions),
            "modes":ordered(modes),"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}});
        if !p.modes.is_empty() {
            shape["required"] = json!(["modes"]);
        }
        shape
    }
    let root = divided_plus_one_counters_modal_program();
    schema["$defs"]["DividedPlusOneCountersModalProgram"] = program(&root);
    schema["$defs"]["DividedCountersExileMode"] = program(&root.modes[0].program);
    schema["$defs"]["DividedCountersDistributionMode"] = program(&root.modes[2].program);
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = power_filtered_removal::witness();
    d.spell = Some(divided_plus_one_counters_modal_program());
    d.capabilities = d.requirements();
    d
}

#[test]
fn divided_plus_one_counters_closed_definition_and_capabilities() {
    let d = witness();
    d.validate().unwrap();
    assert_eq!(d.capabilities, BTreeSet::from(CAPABILITIES));
    assert_eq!(
        strict_read::<Definition>(&canonical(&d).unwrap()).unwrap(),
        d
    );
    let program = d.spell.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&program.modes[2].program.instructions).unwrap(),
        json!([
            {"op":"distribute_plus_one_counters","count":"2","minimum_targets":1,"maximum_targets":2,"source_block":0}
        ])
    );
    for cap in CAPABILITIES {
        let mut bad = d.clone();
        bad.capabilities.remove(&cap);
        assert!(bad.validate().is_err(), "{cap:?}");
    }
    let mutations: &[fn(&mut SpellProgram)] = &[
        |p| p.modes.swap(0, 1),
        |p| {
            p.modes.pop();
        },
        |p| p.modes.push(p.modes[1].clone()),
        |p| {
            p.instructions
                .push(p.modes[2].program.instructions[0].clone())
        },
        |p| p.modes[0].program.targets[0].selection_id = "line:0:1".into(),
        |p| {
            p.modes[0].program.targets[0].filter.as_mut().unwrap().power =
                Some(CreaturePowerFilter::AtLeast { value: 4.into() })
        },
        |p| p.modes[0].program.instructions[0] = Instruction::destroy_target(0, false, 0),
        |p| p.modes[1].program.instructions.swap(0, 2),
        |p| {
            p.modes[1].program.instructions.remove(0);
        },
        |p| {
            p.modes[1].program.instructions[2] = Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::EachOpponent,
                    amount: Amount::fixed(2),
                },
                0,
            )
        },
        |p| p.modes[2].program.targets[0].slot = 1,
        |p| p.modes[2].program.targets[0].domain = TargetDomain::ControlledBattlefieldCreature,
        |p| p.modes[2].program.targets[0].filter = p.modes[0].program.targets[0].filter.clone(),
        |p| {
            let t = p.modes[2].program.targets[0].clone();
            p.modes[2].program.targets.push(t);
        },
        |p| {
            p.modes[2].program.additional_sacrifice = Some(SacrificeFilter {
                types: vec![CardKind::Creature],
                subtypes: vec![],
            })
        },
        |p| {
            p.additional_sacrifice = Some(SacrificeFilter {
                types: vec![CardKind::Creature],
                subtypes: vec![],
            })
        },
        |p| *p = p.modes[2].program.clone(),
        |p| *p = p.modes[0].program.clone(),
    ];
    for (i, mutate) in mutations.iter().enumerate() {
        let mut bad = d.clone();
        mutate(bad.spell.as_mut().unwrap());
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "program mutation {i}");
    }
    for (count, minimum_targets, maximum_targets, source_block) in [
        (1, 1, 2, 0),
        (2, 0, 2, 0),
        (2, 1, 3, 0),
        (2, 2, 2, 0),
        (2, 1, 2, 2),
    ] {
        let mut bad = d.clone();
        bad.spell.as_mut().unwrap().modes[2].program.instructions[0] =
            Instruction::DistributePlusOneCounters {
                count: count.into(),
                minimum_targets,
                maximum_targets,
                source_block,
            };
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
    let mut activated = crate::tap_source_destroy_tests::tap_source_destroy();
    activated.activated[0].program = program.modes[2].program.clone();
    activated.capabilities = activated.requirements();
    assert!(activated.validate().is_err());
    let mut sorcery = d;
    sorcery.characteristics.kind = CardKind::Sorcery;
    sorcery.capabilities = sorcery.requirements();
    assert!(sorcery.validate().is_err());
}
