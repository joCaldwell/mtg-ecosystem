//! Synthetic typed-contract witnesses, not archived card source facts.
//! Compiler/core tests independently qualify the exact retained counter source.
use super::*;

pub(super) fn activated_counter() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Prodigal Sorcerer")
    .unwrap();
    d.activated[0].cost = ActivationCost::Composite {
        costs: PermanentActivationCosts {
            tap: true,
            sacrifice: false,
            mana: Some(ManaCost {
                generic: 2.into(),
                colored: vec![Color::U, Color::U],
            }),
        },
    };
    let p = &mut d.activated[0].program;
    p.targets[0].domain = TargetDomain::StackSpell;
    p.targets[0].filter = None;
    p.instructions = vec![Instruction::CounterStackObject {
        target_slot: 0,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn activated_counter_uses_existing_composite_cost_and_unqualified_stack_target() {
    let d = activated_counter();
    d.validate().unwrap();
    for cap in [
        Capability::ActivatedSpellCounterV1,
        Capability::CompositeActivationCostV1,
        Capability::CounterStackObjectV1,
        Capability::StackSpellTargetV1,
    ] {
        assert!(d.capabilities.contains(&cap));
    }
    assert!(
        !d.capabilities.contains(&Capability::TapSourceCostV1),
        "the composite cost retains its established capability"
    );
    let value = serde_json::to_value(&d).unwrap();
    assert_eq!(
        value["activated"][0]["cost"]["composite"]["costs"]["tap"],
        true
    );
    assert_eq!(
        value["activated"][0]["cost"]["composite"]["costs"]["mana"]["generic"],
        "2"
    );
    assert_eq!(
        strict_read::<Definition>(&serde_json::to_string(&d).unwrap()).unwrap(),
        d
    );
    let mut malformed = value;
    malformed["activated"][0]["cost"]["composite"]["costs"]["discard"] = true.into();
    assert!(strict_read::<Definition>(&serde_json::to_string(&malformed).unwrap()).is_err());
    for cost in [
        ManaCost {
            generic: 0.into(),
            colored: vec![],
        },
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::W, Color::B, Color::R, Color::G],
        },
    ] {
        let mut d = activated_counter();
        let ActivationCost::Composite { costs } = &mut d.activated[0].cost else {
            panic!("composite")
        };
        costs.mana = Some(cost);
        d.validate().unwrap();
    }
}

#[test]
fn activated_counter_requires_contextual_capability_without_promoting_ordinary_counterspells() {
    for cap in [
        Capability::ActivatedSpellCounterV1,
        Capability::CompositeActivationCostV1,
        Capability::CounterStackObjectV1,
        Capability::StackSpellTargetV1,
    ] {
        let mut d = activated_counter();
        d.capabilities.remove(&cap);
        assert!(d.validate().is_err(), "{cap:?}");
    }
    for name in ["Counterspell", "Swan Song", "Prodigal Sorcerer"] {
        let mut d = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-wipes-ir.json"
        ))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| d.name == name)
        .unwrap();
        d.validate().unwrap();
        assert!(
            !d.requirements()
                .contains(&Capability::ActivatedSpellCounterV1)
        );
        d.capabilities.insert(Capability::ActivatedSpellCounterV1);
        assert!(
            d.validate().is_err(),
            "spare activated-counter capability: {name}"
        );
    }
}

#[test]
fn activated_counter_rejects_other_costs_filters_effects_and_context_shapes() {
    for case in [
        "tap-only",
        "mana-only",
        "no-tap",
        "no-mana",
        "sacrifice",
        "negative",
        "colorless",
        "artifact",
        "ability-target",
        "filter",
        "empty-targets",
        "slot",
        "draw",
        "additional",
        "ability-zone",
        "program-zone",
        "timing",
        "resolution",
    ] {
        let mut d = activated_counter();
        let a = &mut d.activated[0];
        match case {
            "tap-only" => a.cost = ActivationCost::TapSource,
            "mana-only" => {
                a.cost = ActivationCost::Mana {
                    cost: ManaCost {
                        generic: 2.into(),
                        colored: vec![Color::U, Color::U],
                    },
                }
            }
            "no-tap" | "no-mana" | "sacrifice" | "negative" | "colorless" => {
                let ActivationCost::Composite { costs } = &mut a.cost else {
                    panic!("composite")
                };
                match case {
                    "no-tap" => costs.tap = false,
                    "no-mana" => costs.mana = None,
                    "sacrifice" => costs.sacrifice = true,
                    "negative" => costs.mana.as_mut().unwrap().generic = (-1).into(),
                    _ => costs.mana.as_mut().unwrap().colored.push(Color::C),
                }
            }
            "artifact" => {
                d.characteristics.kind = CardKind::Artifact;
                d.characteristics.power = None;
                d.characteristics.toughness = None;
            }
            "ability-target" => {
                a.program.targets[0].domain = TargetDomain::StackActivatedOrTriggeredAbility
            }
            "filter" => {
                a.program.targets[0].filter = Some(PermanentFilter {
                    types: vec![CardKind::Creature],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                })
            }
            "empty-targets" => a.program.targets.clear(),
            "slot" => {
                a.program.instructions = vec![Instruction::CounterStackObject {
                    target_slot: 1,
                    source_block: 0,
                }]
            }
            "draw" => a.program.instructions.push(Instruction::DrawCards {
                count: 1.into(),
                recipient: None,
                source_block: 0,
            }),
            "additional" => {
                a.program.additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Creature],
                    subtypes: vec![],
                })
            }
            "ability-zone" => a.functioning_zone = FunctioningZone::Stack,
            "program-zone" => a.program.functioning_zone = FunctioningZone::Battlefield,
            "timing" => a.program.timing = SpellTiming::MainPhase,
            "resolution" => a.program.resolution = SpellResolution::Permanent,
            _ => unreachable!(),
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{case}");
    }
}

#[test]
fn activated_counter_requirements_recurse_while_triggered_and_modal_forms_reject() {
    let mut d = activated_counter();
    let child = d.activated[0].program.clone();
    let p = &mut d.activated[0].program;
    p.targets.clear();
    p.instructions.clear();
    p.modes = vec![
        SpellMode {
            program: child.clone(),
        },
        SpellMode {
            program: child.clone(),
        },
    ];
    d.capabilities = d.requirements();
    assert!(
        d.capabilities
            .contains(&Capability::ActivatedSpellCounterV1)
    );
    assert!(d.validate().is_err());
    for program in [child, d.activated[0].program.clone()] {
        let mut trigger = activated_counter();
        trigger.activated.clear();
        trigger.triggered = vec![TriggeredAbility {
            condition: TriggerCondition::SelfEntersBattlefield,
            death_filter: None,
            intervening_if: None,
            functioning_zone: FunctioningZone::Battlefield,
            program,
        }];
        trigger.capabilities = trigger.requirements();
        assert!(
            !trigger
                .capabilities
                .contains(&Capability::ActivatedSpellCounterV1)
        );
        assert!(trigger.validate().is_err());
    }
    for old in [
        tap_source_destroy_tests::tap_source_destroy(),
        activated_permanent_tap_tests::activated_tap(),
    ] {
        assert!(
            !old.requirements()
                .contains(&Capability::ActivatedSpellCounterV1)
        );
        old.validate().unwrap();
    }
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .validate()
    .unwrap();
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-blocking-matches-ir.json"
    ))
    .unwrap()
    .validate()
    .unwrap();
}
