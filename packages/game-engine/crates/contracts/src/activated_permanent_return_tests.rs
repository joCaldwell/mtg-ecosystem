//! Synthetic typed-contract witnesses. Exact archived Temporal Adept semantics
//! are independently qualified by compiler/core source tests.
use super::*;

pub(super) fn activated_return() -> Definition {
    let mut d = activated_spell_counter_tests::activated_counter();
    let a = &mut d.activated[0];
    let ActivationCost::Composite { costs } = &mut a.cost else {
        panic!("composite")
    };
    costs.mana = Some(ManaCost {
        generic: 0.into(),
        colored: vec![Color::U, Color::U, Color::U],
    });
    a.program.targets[0].domain = TargetDomain::BattlefieldPermanent;
    a.program.targets[0].filter = Some(PermanentFilter {
        types: vec![],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        tapped: false,
    });
    a.program.instructions = vec![Instruction::ReturnToOwnerHand {
        target_slot: 0,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn activated_permanent_return_round_trips_with_existing_costs_and_all_context_capabilities() {
    let d = activated_return();
    d.validate().unwrap();
    for cap in [
        Capability::ActivatedPermanentOwnerHandV1,
        Capability::PermanentOwnerHandMoveV1,
        Capability::OwnerHandMoveV1,
        Capability::PermanentRemovalV1,
        Capability::CompositeActivationCostV1,
    ] {
        assert!(d.capabilities.contains(&cap));
    }
    let value = serde_json::to_value(&d).unwrap();
    assert_eq!(
        value["activated"][0]["cost"]["composite"]["costs"]["tap"],
        true
    );
    assert_eq!(
        value["activated"][0]["cost"]["composite"]["costs"]["mana"]["colored"],
        serde_json::json!(["U", "U", "U"])
    );
    assert_eq!(
        strict_read::<Definition>(&serde_json::to_string(&d).unwrap()).unwrap(),
        d
    );
    let mut zero = d.clone();
    let ActivationCost::Composite { costs } = &mut zero.activated[0].cost else {
        panic!("composite")
    };
    costs.mana = Some(ManaCost {
        generic: 0.into(),
        colored: vec![],
    });
    zero.validate().unwrap();
}

#[test]
fn activated_permanent_return_requires_contextual_capabilities_without_promoting_ordinary_bounce() {
    for cap in [
        Capability::ActivatedPermanentOwnerHandV1,
        Capability::PermanentOwnerHandMoveV1,
        Capability::OwnerHandMoveV1,
        Capability::PermanentRemovalV1,
        Capability::CompositeActivationCostV1,
    ] {
        let mut d = activated_return();
        d.capabilities.remove(&cap);
        assert!(d.validate().is_err(), "{cap:?}");
    }
    for name in ["Regress", "Disperse", "Unsummon"] {
        let mut d = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-sacrifice-ir.json"
        ))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| d.name == name)
        .unwrap();
        d.validate().unwrap();
        assert!(
            !d.requirements()
                .contains(&Capability::ActivatedPermanentOwnerHandV1)
        );
        d.capabilities
            .insert(Capability::ActivatedPermanentOwnerHandV1);
        assert!(d.validate().is_err(), "spare context label: {name}");
    }
}

#[test]
fn activated_permanent_return_rejects_other_costs_qualifiers_and_compositions() {
    for case in [
        "tap-only",
        "mana-only",
        "no-tap",
        "no-mana",
        "sacrifice",
        "negative",
        "colorless",
        "artifact",
        "creature-target",
        "missing-filter",
        "nonland",
        "type",
        "non-color",
        "opponent",
        "tapped",
        "slot",
        "draw",
        "additional",
        "ability-zone",
        "program-zone",
        "timing",
        "resolution",
    ] {
        let mut d = activated_return();
        let a = &mut d.activated[0];
        match case {
            "tap-only" => a.cost = ActivationCost::TapSource,
            "mana-only" => {
                a.cost = ActivationCost::Mana {
                    cost: ManaCost {
                        generic: 0.into(),
                        colored: vec![Color::U, Color::U, Color::U],
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
            "creature-target" => {
                a.program.targets[0].domain = TargetDomain::BattlefieldCreature;
                a.program.targets[0].filter = None;
            }
            "missing-filter" => a.program.targets[0].filter = None,
            "nonland" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_types
                .push(CardKind::Land),
            "type" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types
                .push(CardKind::Creature),
            "non-color" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_colors
                .push(Color::B),
            "opponent" => a.program.targets[0].filter.as_mut().unwrap().opponent = true,
            "tapped" => a.program.targets[0].filter.as_mut().unwrap().tapped = true,
            "slot" => {
                a.program.instructions = vec![Instruction::ReturnToOwnerHand {
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
fn activated_permanent_return_context_recurses_and_preserves_other_supported_forms() {
    let mut d = activated_return();
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
            .contains(&Capability::ActivatedPermanentOwnerHandV1)
    );
    assert!(
        d.capabilities
            .contains(&Capability::PermanentOwnerHandMoveV1)
    );
    assert!(d.validate().is_err());
    for program in [child, d.activated[0].program.clone()] {
        let mut trigger = activated_return();
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
                .contains(&Capability::ActivatedPermanentOwnerHandV1)
        );
        assert!(trigger.validate().is_err());
    }
    for old in [
        tap_source_destroy_tests::tap_source_destroy(),
        activated_permanent_tap_tests::activated_tap(),
        activated_spell_counter_tests::activated_counter(),
    ] {
        assert!(
            !old.requirements()
                .contains(&Capability::ActivatedPermanentOwnerHandV1)
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
        "../../../tests/fixtures/expected-sacrifice-ir.json"
    ))
    .unwrap()
    .validate()
    .unwrap();
}
