//! CR 601.2c, 608.2b. Restrict the new target predicate to its complete spell.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn deserialize_power<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<CreaturePowerFilter>, D::Error> {
    CreaturePowerFilter::deserialize(deserializer).map(Some)
}

fn uses_restricted_operand(p: &SpellProgram) -> bool {
    p.targets
        .iter()
        .any(|t| t.filter.as_ref().is_some_and(|f| f.power.is_some()))
        || p.instructions
            .iter()
            .any(|i| matches!(i, Instruction::DistributePlusOneCounters { .. }))
        || p.modes.iter().any(|m| uses_restricted_operand(&m.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    const INVALID: &str = "unsupported power-filtered removal or divided-counter spell";
    if d.activated
        .iter()
        .any(|a| uses_restricted_operand(&a.program))
        || d.triggered
            .iter()
            .any(|a| uses_restricted_operand(&a.program))
        || d.static_abilities.iter().any(|a| {
            matches!(&a.effect,
            StaticEffect::GrantSpellCascade { program, .. } if uses_restricted_operand(program))
        })
    {
        return Err(INVALID.into());
    }
    let Some(p) = d.spell.as_ref().filter(|p| uses_restricted_operand(p)) else {
        return Ok(());
    };
    let c = &d.characteristics;
    let timing = match c.kind {
        CardKind::Instant => SpellTiming::Instant,
        CardKind::Sorcery => SpellTiming::MainPhase,
        _ => return Err(INVALID.into()),
    };
    if !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || c.basic
        || c.legendary
        || c.snow
        || c.power.is_some()
        || c.toughness.is_some()
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || p.timing != timing
    {
        return Err(INVALID.into());
    }
    if c.kind == CardKind::Instant
        && !p.modes.is_empty()
        && *p == divided_plus_one_counters_modal_program()
    {
        return Ok(());
    }
    if !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !matches!(p.targets.as_slice(), [t] if t.slot == 0 && t.selection_id == "line:0:1"
            && t.domain == TargetDomain::BattlefieldPermanent && t.chooser == Chooser::Controller
            && t.stage == EvaluationStage::Announcement && t.filter.as_ref().is_some_and(|f|
                f.types == [CardKind::Creature] && f.non_types.is_empty() && f.non_colors.is_empty()
                && !f.opponent && !f.tapped && f.power.is_some()))
        || !matches!(
            p.instructions.as_slice(),
            [
                kernel!(Destroy { what: target_object!(0), regeneration_forbidden: false } @ 0)
                    | kernel!(Move { what: target_object!(0), to: kir::Destination::Exile } @ 0)
            ]
        )
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let filter = json!({"type":"object","required":["power"]});
    let target = json!({"required":["filter"],"properties":{"filter":filter}});
    let power_target = json!({"type":"object","required":["targets"],"properties":{"targets":{"contains":target}}});
    let has = json!({"type":"object","anyOf":[power_target,{"required":["instructions"],"properties":{
        "instructions":{"contains":{"properties":{"op":{"const":"distribute_plus_one_counters"}}}}
    }}]});
    let free_child =
        json!({"properties":{"program":{"$ref":"#/$defs/PowerAndDivisionFreeProgram"}}});
    schema["$defs"]["PowerAndDivisionFreeProgram"] =
        json!({"not":has,"properties":{"modes":{"items":free_child}}});
    let modal = json!({"$ref":"#/$defs/DividedPlusOneCountersModalProgram"});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "anyOf":[modal,{"properties":{"modes":{"items":free_child}}}],
        "if":has,"then":{"anyOf":[{"$ref":"#/$defs/DividedCountersExileMode"},
            {"$ref":"#/$defs/DividedCountersDistributionMode"},{"properties":{
            "resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},"modes":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "slot":{"const":0},"selection_id":{"const":"line:0:1"},"chooser":{"const":"controller"},
                "stage":{"const":"announcement"},"domain":{"const":"battlefield_permanent"},
                "filter":{"allOf":[filter],"properties":{"types":{"const":["creature"]},"non_types":{"maxItems":0},
                    "non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}}}
            }}},
            "instructions":{"minItems":1,"maxItems":1,"items":instruction_schema::at(json!({"anyOf":[
                instruction_schema::target_destroy(Some(0), Some(false)),
                instruction_schema::target_move(Some(0), "exile")
            ]}), 0)}
        }}]}
    }));
    let closure = |timing, cast| {
        let caps = [
            NormalLayoutV1,
            cast,
            PermanentRemovalV1,
            PowerFilteredRemovalV1,
        ];
        json!({"properties":{"spell":{"properties":{"timing":{"const":timing}}},
            "capabilities":{"uniqueItems":true,"minItems":caps.len(),"maxItems":caps.len(),"items":{"enum":caps}}
        }})
    };
    let caps = divided_plus_one_counters::CAPABILITIES;
    let standalone = json!({"properties":{"spell":{"properties":{"modes":{"maxItems":0},
        "targets":{"minItems":1,"items":{"properties":{"selection_id":{"const":"line:0:1"}}}}}}},
        "if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
        "then":closure(SpellTiming::Instant,CastInstantV1),"else":closure(SpellTiming::MainPhase,CastSorceryV1)});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{
            "activated":{"items":free_child},"triggered":{"items":free_child},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":free_child}}}}}
        },
        "if":{"required":["spell"],"properties":{"spell":{"type":"object","not":{"$ref":"#/$defs/PowerAndDivisionFreeProgram"}}}},
        "then":{
            "properties":{
                "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"basic":{"const":false},
                    "legendary":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0},
                    "subtypes":{"maxItems":0},"power":{"type":"null"},"toughness":{"type":"null"}}},
                "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},"keywords":{"maxItems":0},
                "static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0}
            },
            "if":{"properties":{"spell":modal}},
            "then":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}},
                "capabilities":{"uniqueItems":true,"minItems":caps.len(),"maxItems":caps.len(),"items":{"enum":caps}}}},
            "else":standalone
        },
        "else":{"properties":{"capabilities":{"not":{"contains":{"enum":[PowerFilteredRemovalV1,DividedPlusOneCountersV1]}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-removal-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Naturalize")
    .unwrap();
    let filter = d.spell.as_mut().unwrap().targets[0]
        .filter
        .as_mut()
        .unwrap();
    filter.types = vec![CardKind::Creature];
    filter.power = Some(CreaturePowerFilter::AtLeast { value: 4.into() });
    d.capabilities = d.requirements();
    d
}

#[test]
fn power_filtered_removal_shape_and_capability_closure() {
    use Capability::*;
    let d = witness();
    d.validate().unwrap();
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastInstantV1,
            PermanentRemovalV1,
            PowerFilteredRemovalV1
        ])
    );
    let mut serialized = serde_json::to_value(&d).unwrap();
    let filter = &mut serialized["spell"]["targets"][0]["filter"];
    assert_eq!(
        filter["power"],
        json!({"comparison":"at_least","value":"4"})
    );
    for bad in [
        Value::Null,
        json!({}),
        json!({"comparison":"ge","value":"4"}),
        json!({"comparison":"at_least","value":4}),
        json!({"comparison":"at_least","value":"04"}),
        json!({"comparison":"at_most","value":"4","extra":true}),
    ] {
        filter["power"] = bad;
        assert!(serde_json::from_value::<PermanentFilter>(filter.clone()).is_err());
    }
    filter.as_object_mut().unwrap().remove("power");
    let legacy: PermanentFilter = serde_json::from_value(filter.clone()).unwrap();
    assert_eq!(serde_json::to_value(legacy).unwrap(), *filter);

    let mut sorcery = d.clone();
    sorcery.characteristics.kind = CardKind::Sorcery;
    let p = sorcery.spell.as_mut().unwrap();
    p.timing = SpellTiming::MainPhase;
    p.targets[0].filter.as_mut().unwrap().power = Some(CreaturePowerFilter::AtMost {
        value: "-123456789012345678901".parse().unwrap(),
    });
    p.instructions[0] = Instruction::move_target(0, kir::Destination::Exile, 0);
    sorcery.capabilities = sorcery.requirements();
    sorcery.validate().unwrap();
    assert_eq!(
        sorcery.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastSorceryV1,
            PermanentRemovalV1,
            PowerFilteredRemovalV1
        ])
    );
    assert_eq!(
        strict_read::<Definition>(&serde_json::to_string(&sorcery).unwrap()).unwrap(),
        sorcery
    );
    for cap in &sorcery.capabilities.clone() {
        sorcery.capabilities.remove(cap);
        assert!(sorcery.validate().is_err());
        sorcery.capabilities.insert(*cap);
    }
    let mutations: &[fn(&mut SpellProgram)] = &[
        |p| p.targets[0].filter.as_mut().unwrap().types.clear(),
        |p| {
            p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types
                .push(CardKind::Artifact)
        },
        |p| {
            p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_types
                .push(CardKind::Land)
        },
        |p| {
            p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_colors
                .push(Color::B)
        },
        |p| p.targets[0].filter.as_mut().unwrap().opponent = true,
        |p| p.targets[0].filter.as_mut().unwrap().tapped = true,
        |p| p.targets[0].domain = TargetDomain::StackSpell,
        |p| p.targets.push(p.targets[0].clone()),
        |p| p.instructions.push(p.instructions[0].clone()),
        |p| p.instructions[0] = Instruction::move_target(0, kir::Destination::Exile, 1),
        |p| p.instructions[0] = Instruction::move_target(1, kir::Destination::Exile, 0),
        |p| p.instructions[0] = Instruction::tap_target(0, 0),
        |p| {
            if let Some((slot, _, _, block)) = p.instructions[0].target_destroy() {
                p.instructions[0] =
                    Instruction::optional_destroy_target(Context::Spell, slot, block);
            }
        },
        |p| {
            if let kernel!(Destroy {
                regeneration_forbidden,
                ..
            }) = &mut p.instructions[0]
            {
                *regeneration_forbidden = true;
            }
        },
        |p| {
            p.additional_sacrifice = Some(SacrificeFilter {
                types: vec![CardKind::Creature],
                subtypes: vec![],
            })
        },
        |p| {
            let child = p.clone();
            p.targets.clear();
            p.instructions.clear();
            p.modes.push(SpellMode { program: child });
        },
    ];
    for (i, mutate) in mutations.iter().enumerate() {
        let mut bad = d.clone();
        mutate(bad.spell.as_mut().unwrap());
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "mutation {i}");
    }
    let power = d.spell.unwrap().targets.remove(0).filter.unwrap().power;
    let mut activated = crate::tap_source_destroy_tests::tap_source_destroy();
    activated.activated[0].program.targets[0]
        .filter
        .as_mut()
        .unwrap()
        .power = power.clone();
    activated.capabilities = activated.requirements();
    assert!(activated.validate().is_err());

    let mut aura = crate::aura_control_tests::aura();
    let t = &mut aura.spell.as_mut().unwrap().targets[0];
    t.domain = TargetDomain::BattlefieldPermanent;
    t.filter = Some(PermanentFilter {
        types: vec![],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        tapped: false,
        power,
    });
    assert!(aura_control_domain(&aura).is_none());
    aura.capabilities = aura.requirements();
    assert!(aura.validate().is_err());
}
