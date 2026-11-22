//! A mandatory discard by the announced player (CR 701.9a-b, 601.2c, 608.2b-c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

// Use an empty struct variant on input so announced X rejects extra operands.
impl<'de> Deserialize<'de> for RandomDiscardCount {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
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

fn has_discard(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::DiscardTargetPlayer { .. }
                | Instruction::DiscardTargetPlayerAtRandom { .. }
        )
    }) || p.modes.iter().any(|m| has_discard(&m.program))
}

fn player_discard(p: &SpellProgram, domain: TargetDomain) -> bool {
    matches!(p.instructions.as_slice(), [Instruction::DiscardTargetPlayer {
        target_slot: 0, count, source_block: 0,
    }] if *count > Quantity::zero())
        && matches!(p.targets.as_slice(), [t] if t.domain == domain && t.filter.is_none())
}

// CR 701.9b: only the explicitly random discard bypasses the player's choice.
fn random_discard_family(p: &SpellProgram, x_cost: bool) -> bool {
    use Instruction::{DiscardTargetPlayer, DiscardTargetPlayerAtRandom};
    if p.additional_sacrifice.is_some()
        || !p.modes.is_empty()
        || !matches!(p.targets.as_slice(), [t] if t.filter.is_none()
            && matches!(t.domain, TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer))
    {
        return false;
    }
    match p.instructions.as_slice() {
        [
            DiscardTargetPlayerAtRandom {
                target_slot: 0,
                source_block: 0,
                count,
            },
        ] => match count {
            RandomDiscardCount::Fixed { value } => !x_cost && *value > Quantity::zero(),
            RandomDiscardCount::AnnouncedX => x_cost,
        },
        [
            DiscardTargetPlayerAtRandom {
                target_slot: 0,
                source_block: 0,
                count: RandomDiscardCount::Fixed { value },
            },
            DiscardTargetPlayer {
                target_slot: 0,
                source_block: 0,
                count,
            },
        ] => !x_cost && *value == Quantity::from(1) && *count == Quantity::from(1),
        _ => false,
    }
}

fn enchantment_destroy(p: &SpellProgram) -> bool {
    matches!(
        p.instructions.as_slice(),
        [Instruction::DestroyPermanent {
            target_slot: 0,
            optional: false,
            regeneration_forbidden: false,
            source_block: 0,
        }]
    ) && matches!(p.targets.as_slice(), [t] if t.domain == TargetDomain::BattlefieldPermanent
        && t.filter.as_ref().is_some_and(|f| f.types == [CardKind::Enchantment]
            && f.non_types.is_empty() && f.non_colors.is_empty() && !f.opponent && f.power.is_none() && !f.tapped))
}

fn two_draws(p: &SpellProgram) -> bool {
    p.targets.is_empty() && p.instructions.len() == 2 && p.instructions.iter().all(|i|
        matches!(i, kernel!(Draw { player: you!(), amount: fixed!(count) } @ 0) if *count == Quantity::from(1)))
}

fn discard_family(p: &SpellProgram) -> bool {
    if p.additional_sacrifice.is_some() || !p.optional_additional_costs.is_empty() {
        return false;
    }
    if p.modes.is_empty() {
        return player_discard(p, TargetDomain::AnyPlayer)
            || player_discard(p, TargetDomain::OpponentPlayer);
    }
    // The ordinary modal validator checks the empty root, stack zone, timing,
    // non-nesting, target bindings and nonempty children. All options belong
    // to the same raw source block, independent of their option index.
    p.timing == SpellTiming::Instant
        && p.modes.len() == 3
        && p.modes.iter().all(|m| {
            m.program.additional_sacrifice.is_none()
                && m.program.optional_additional_costs.is_empty()
        })
        && p.modes
            .iter()
            .any(|m| player_discard(&m.program, TargetDomain::AnyPlayer))
        && p.modes.iter().any(|m| enchantment_destroy(&m.program))
        && p.modes.iter().any(|m| two_draws(&m.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_discard(&a.program))
        || d.triggered.iter().any(|a| has_discard(&a.program))
    {
        return Err("targeted discard requires a spell source".into());
    }
    let Some(p) = d.spell.as_ref().filter(|p| has_discard(p)) else {
        return Ok(());
    };
    let c = &d.characteristics;
    let arcane_sorcery =
        c.kind == CardKind::Sorcery && p.modes.is_empty() && c.subtypes == ["Arcane"];
    let chosen = (c.subtypes.is_empty() || arcane_sorcery) && !d.x_mana_cost && discard_family(p);
    let random = c.subtypes.is_empty() && random_discard_family(p, d.x_mana_cost);
    if !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
        || !c.additional_types.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.keywords.is_empty()
        || !(chosen || random)
    {
        return Err("unsupported complete targeted discard spell".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use Capability::*;
    use serde_json::{Value, json};
    let free = json!({"$ref":"#/$defs/TargetedDiscardFreeProgram"});
    let op = "discard_target_player";
    let random = "discard_target_player_at_random";
    schema["$defs"]["TargetedDiscardFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"properties":{"op":{"enum":[op,random]}}}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    let target = |domain, filter: Value| {
        let mut item = json!({"properties":{
            "slot":{"const":0},"selection_id":{"minLength":1},"domain":{"const":domain},
            "chooser":{"const":"controller"},"stage":{"const":"announcement"},"filter":filter
        }});
        if domain == TargetDomain::BattlefieldPermanent {
            item["required"] = json!(["filter"]);
        }
        json!({"minItems":1,"maxItems":1,"items":item})
    };
    let program = |targets: Value, instructions: Value| {
        json!({"properties":{
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"targets":targets,
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "instructions":instructions
        }})
    };
    let discard = |domain| {
        program(
            target(domain, json!({"type":"null"})),
            json!({"minItems":1,"maxItems":1,"items":{"properties":{"op":{"const":op}}}}),
        )
    };
    let destroy = program(
        target(
            TargetDomain::BattlefieldPermanent,
            json!({
                "type":"object","properties":{"types":{"const":["enchantment"]},
                    "non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}}
            }),
        ),
        json!({"minItems":1,"maxItems":1,"items":{"properties":{
            "op":{"const":"destroy_permanent"},"target_slot":{"const":0},"source_block":{"const":0},
            "optional":{"const":false},"regeneration_forbidden":{"const":false}
        }}}),
    );
    let draw = program(
        json!({"maxItems":0}),
        json!({"minItems":2,"maxItems":2,"items":instruction_schema::at(instruction_schema::single_draw(), 0)}),
    );
    let mode = |shape: Value| json!({"contains":{"properties":{"program":shape}}});
    let modal = json!({"required":["modes"],"properties":{
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"timing":{"const":"instant"},"functioning_zone":{"const":"stack"},
        "resolution":{"const":"instructions"},"targets":{"maxItems":0},"instructions":{"maxItems":0},
        "modes":{"minItems":3,"maxItems":3,"items":{"properties":{"program":{"properties":{"timing":{"const":"instant"}}}}},
            "allOf":[mode(discard(TargetDomain::AnyPlayer)),mode(destroy),mode(draw)]}
    }});
    let random_program = |domain, x, followed_by_choice| {
        let count = if x {
            json!({"const":RandomDiscardCount::AnnouncedX})
        } else if followed_by_choice {
            json!({"const":RandomDiscardCount::Fixed{value:1.into()}})
        } else {
            json!({"properties":{"amount":{"const":"fixed"}}})
        };
        let mut ops = vec![json!({"properties":{"op":{"const":random},"count":count}})];
        if followed_by_choice {
            ops.push(json!({"const":Instruction::DiscardTargetPlayer{target_slot:0,count:1.into(),source_block:0}}));
        }
        program(
            target(domain, json!({"type":"null"})),
            json!({"minItems":ops.len(),"maxItems":ops.len(),"prefixItems":ops}),
        )
    };
    let mut programs = vec![modal.clone()];

    // Kind, timing, subtype and the exact capability set are definition facts.
    let variant = |kind: CardKind, shape: Value, mut caps: Vec<Capability>, x: bool| {
        let random = caps.contains(&RandomPlayerDiscardV1);
        let (timing, cast, subtypes) = if kind == CardKind::Instant {
            (SpellTiming::Instant, CastInstantV1, json!({"maxItems":0}))
        } else {
            (
                SpellTiming::MainPhase,
                CastSorceryV1,
                if random {
                    json!({"maxItems":0})
                } else {
                    json!({"enum":[[],["Arcane"]]})
                },
            )
        };
        caps.extend([NormalLayoutV1, cast]);
        let required: Vec<_> = caps
            .iter()
            .map(|cap| json!({"contains":{"const":cap}}))
            .collect();
        json!({"properties":{
            "x_mana_cost":{"const":x},
            "spell":{"allOf":[shape,{"properties":{"timing":{"const":timing}}}]},
            "characteristics":{"properties":{"kind":{"const":kind},"subtypes":subtypes}},
            "capabilities":{"maxItems":caps.len(),"allOf":required}
        }})
    };
    let mut alternatives = vec![variant(
        CardKind::Instant,
        modal,
        vec![
            TargetPlayerDiscardV1,
            ModalChoiceV1,
            AnyPlayerTargetV1,
            PermanentRemovalV1,
            ControllerDrawV1,
        ],
        false,
    )];
    for (domain, target_cap) in [
        (TargetDomain::AnyPlayer, AnyPlayerTargetV1),
        (TargetDomain::OpponentPlayer, OpponentTargetV1),
    ] {
        for (program, x, caps) in [
            (
                discard(domain),
                false,
                vec![target_cap, TargetPlayerDiscardV1],
            ),
            (
                random_program(domain, false, false),
                false,
                vec![target_cap, RandomPlayerDiscardV1],
            ),
            (
                random_program(domain, true, false),
                true,
                vec![target_cap, RandomPlayerDiscardV1],
            ),
            (
                random_program(domain, false, true),
                false,
                vec![target_cap, RandomPlayerDiscardV1, TargetPlayerDiscardV1],
            ),
        ] {
            for kind in [CardKind::Instant, CardKind::Sorcery] {
                alternatives.push(variant(kind, program.clone(), caps.clone(), x));
            }
            programs.push(program);
        }
    }
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"if":{"not":free},"then":{"oneOf":programs}}));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":free}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({"properties":{
            "activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}}
        }}),
        json!({"if":has,"then":{"oneOf":alternatives,"properties":{
            "mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},
            "triggered":{"maxItems":0},"keywords":{"maxItems":0},
            "intrinsic_zone":{"type":"null"},"characteristics":{"properties":{
                "additional_types":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}
            }}
        }},"else":{"properties":{"capabilities":{"not":{"contains":{"enum":[TargetPlayerDiscardV1,RandomPlayerDiscardV1]}}}}}})
    ]);
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = targeted_counted_draw::witness();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::DiscardTargetPlayer {
        target_slot: 0,
        count: 2.into(),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
pub(super) fn random_witness() -> Definition {
    let mut d = witness();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::DiscardTargetPlayerAtRandom {
        target_slot: 0,
        count: RandomDiscardCount::Fixed { value: 1.into() },
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn random_discard() {
    use Capability::{RandomPlayerDiscardV1, TargetPlayerDiscardV1};
    let fixed = |n: i64| RandomDiscardCount::Fixed { value: n.into() };
    let op = |count| Instruction::DiscardTargetPlayerAtRandom {
        target_slot: 0,
        count,
        source_block: 0,
    };
    let chosen = |n: i64| Instruction::DiscardTargetPlayer {
        target_slot: 0,
        count: n.into(),
        source_block: 0,
    };
    for (x, instructions, valid) in [
        (false, vec![op(fixed(2))], true),
        (true, vec![op(RandomDiscardCount::AnnouncedX)], true),
        (false, vec![op(fixed(1)), chosen(1)], true),
        (false, vec![op(fixed(0))], false),
        (false, vec![op(fixed(-1))], false),
        (true, vec![op(fixed(1))], false),
        (false, vec![op(RandomDiscardCount::AnnouncedX)], false),
        (
            true,
            vec![op(RandomDiscardCount::AnnouncedX), chosen(1)],
            false,
        ),
        (false, vec![op(fixed(2)), chosen(1)], false),
        (false, vec![op(fixed(1)), chosen(2)], false),
        (false, vec![chosen(1), op(fixed(1))], false),
        (false, vec![op(fixed(1)), op(fixed(1))], false),
    ] {
        let mut d = random_witness();
        d.x_mana_cost = x;
        d.spell.as_mut().unwrap().instructions = instructions;
        for domain in [TargetDomain::AnyPlayer, TargetDomain::OpponentPlayer] {
            d.spell.as_mut().unwrap().targets[0].domain = domain;
            d.capabilities = d.requirements();
            assert_eq!(d.validate().is_ok(), valid, "{:?}", d.spell);
            if valid {
                assert_eq!(
                    d.capabilities.contains(&TargetPlayerDiscardV1),
                    d.spell.as_ref().unwrap().instructions.len() == 2
                );
                assert!(d.capabilities.remove(&RandomPlayerDiscardV1));
                assert!(d.validate().is_err());
            }
        }
    }
    let mut d = random_witness();
    d.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BattlefieldCreature;
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());

    let mut d = random_witness();
    let child = d.spell.clone().unwrap();
    let p = d.spell.as_mut().unwrap();
    p.targets.clear();
    p.instructions.clear();
    p.modes = vec![SpellMode { program: child }; 2];
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());

    let malformed = serde_json::json!({"amount":"announced_x","value":"1"});
    assert!(serde_json::from_value::<RandomDiscardCount>(malformed).is_err());
    for count in [fixed(2), RandomDiscardCount::AnnouncedX] {
        let wire = serde_json::to_value(&count).unwrap();
        assert_eq!(
            serde_json::from_value::<RandomDiscardCount>(wire).unwrap(),
            count
        );
    }
}

#[cfg(test)]
#[test]
fn targeted_discard_admits_exact_standalone_and_modal_programs() {
    let mut d = witness();
    d.characteristics.kind = CardKind::Sorcery;
    d.characteristics.subtypes = vec!["Arcane".into()];
    let p = d.spell.as_mut().unwrap();
    p.timing = SpellTiming::MainPhase;
    p.targets[0].domain = TargetDomain::OpponentPlayer;
    for (n, valid) in [
        ("-1", false),
        ("0", false),
        ("1", true),
        ("9999999999999999999999999", true),
    ] {
        let Instruction::DiscardTargetPlayer { count, .. } =
            &mut d.spell.as_mut().unwrap().instructions[0]
        else {
            unreachable!()
        };
        *count = n.parse().unwrap();
        d.capabilities = d.requirements();
        assert_eq!(d.validate().is_ok(), valid, "count {n}");
    }
    let mut d = witness();
    d.characteristics.kind = CardKind::Instant;
    let p = d.spell.as_mut().unwrap();
    p.timing = SpellTiming::Instant;
    let discard = p.clone();
    let mut destroy = discard.clone();
    destroy.targets[0].domain = TargetDomain::BattlefieldPermanent;
    destroy.targets[0].filter = Some(PermanentFilter {
        power: None,
        types: vec![CardKind::Enchantment],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        tapped: false,
    });
    destroy.instructions = vec![Instruction::DestroyPermanent {
        target_slot: 0,
        optional: false,
        regeneration_forbidden: false,
        source_block: 0,
    }];
    let mut draw = discard.clone();
    draw.targets.clear();
    draw.instructions = vec![Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 0); 2];
    p.targets.clear();
    p.instructions.clear();
    p.modes = [destroy, draw, discard]
        .map(|program| SpellMode { program })
        .into();
    d.capabilities = d.requirements();
    d.validate().unwrap();
    let mut wrong_caps = d.clone();
    wrong_caps
        .capabilities
        .remove(&Capability::TargetPlayerDiscardV1);
    assert!(wrong_caps.validate().is_err());

    let mutations: [fn(&mut SpellProgram); 4] = [
        |p| {
            p.modes[1].program.instructions.pop();
        },
        |p| p.modes[2].program.targets[0].domain = TargetDomain::OpponentPlayer,
        |p| {
            p.modes[0] = p.modes[2].clone();
        },
        |p| {
            if let Instruction::DiscardTargetPlayer { source_block, .. } =
                &mut p.modes[2].program.instructions[0]
            {
                *source_block = 2;
            }
        },
    ];
    for mutate in mutations {
        let mut bad = d.clone();
        mutate(bad.spell.as_mut().unwrap());
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
}
