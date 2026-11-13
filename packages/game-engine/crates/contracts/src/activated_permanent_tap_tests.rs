//! Synthetic typed-contract witnesses, not claims about archived source facts.
//! Compiler/core independently exercise the exact retained activated-tap source.
use super::*;

pub(super) fn activated_tap() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Sol Ring")
    .unwrap();
    d.mana_abilities.clear();
    d.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: PermanentActivationCosts {
                tap: true,
                sacrifice: false,
                mana: Some(ManaCost {
                    generic: 1.into(),
                    colored: vec![],
                }),
            },
        },
        program: SpellProgram {
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: "activated-target".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::BattlefieldPermanent,
                filter: Some(PermanentFilter {
                    types: vec![CardKind::Artifact, CardKind::Creature, CardKind::Land],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                }),
            }],
            instructions: vec![Instruction::TapPermanent {
                target_slot: 0,
                source_block: 0,
            }],
            modes: vec![],
        },
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn activated_tap_uses_existing_costs_and_order_independent_exact_type_union() {
    for types in [
        vec![CardKind::Artifact, CardKind::Creature, CardKind::Land],
        vec![CardKind::Land, CardKind::Artifact, CardKind::Creature],
        vec![CardKind::Creature, CardKind::Land, CardKind::Artifact],
    ] {
        let mut d = activated_tap();
        d.activated[0].program.targets[0]
            .filter
            .as_mut()
            .unwrap()
            .types = types;
        d.capabilities = d.requirements();
        d.validate().unwrap();
        assert!(d.capabilities.contains(&Capability::TapPermanentV1));
        assert!(
            d.capabilities
                .contains(&Capability::ActivatedTapPermanentV1)
        );
        let encoded = serde_json::to_string(&d).unwrap();
        assert_eq!(strict_read::<Definition>(&encoded).unwrap(), d);
    }
    for cost in [
        ManaCost {
            generic: 0.into(),
            colored: vec![],
        },
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::U, Color::R],
        },
    ] {
        let mut d = activated_tap();
        let ActivationCost::Composite { costs } = &mut d.activated[0].cost else {
            panic!("composite")
        };
        costs.mana = Some(cost);
        d.validate().unwrap();
    }
}

#[test]
fn activated_tap_requires_exact_capability_closure_without_promoting_ordinary_tap() {
    for cap in [
        Capability::ActivatedTapPermanentV1,
        Capability::TapPermanentV1,
        Capability::CompositeActivationCostV1,
        Capability::PermanentRemovalV1,
    ] {
        let mut d = activated_tap();
        d.capabilities.remove(&cap);
        assert!(d.validate().is_err(), "{cap:?}");
    }
    let mut ordinary = tap_tests::tap();
    ordinary.validate().unwrap();
    assert!(
        !ordinary
            .requirements()
            .contains(&Capability::ActivatedTapPermanentV1)
    );
    ordinary
        .capabilities
        .insert(Capability::ActivatedTapPermanentV1);
    assert!(
        ordinary.validate().is_err(),
        "spare contextual label cannot widen an ordinary spell"
    );
    let mut d = activated_tap();
    d.activated.clear();
    assert!(
        d.validate().is_err(),
        "spare capability without activated tap must reject"
    );
}

#[test]
fn activated_tap_rejects_other_costs_source_kinds_filters_and_compositions() {
    for case in [
        "no-tap",
        "no-mana",
        "sacrifice",
        "negative",
        "colorless",
        "tap-only",
        "mana-only",
        "creature",
        "enchantment",
        "creature-domain",
        "missing-filter",
        "duplicate",
        "extra-type",
        "missing-type",
        "non-types",
        "non-colors",
        "opponent",
        "tapped",
        "draw",
        "ability-zone",
        "program-zone",
        "timing",
        "resolution",
        "additional",
        "slot",
    ] {
        let mut d = activated_tap();
        let a = &mut d.activated[0];
        match case {
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
            "tap-only" => a.cost = ActivationCost::TapSource,
            "mana-only" => {
                a.cost = ActivationCost::Mana {
                    cost: ManaCost {
                        generic: 1.into(),
                        colored: vec![],
                    },
                }
            }
            "creature" => d.characteristics.kind = CardKind::Creature,
            "enchantment" => d.characteristics.kind = CardKind::Enchantment,
            "creature-domain" => a.program.targets[0].domain = TargetDomain::BattlefieldCreature,
            "missing-filter" => a.program.targets[0].filter = None,
            "duplicate" => {
                a.program.targets[0].filter.as_mut().unwrap().types[2] = CardKind::Artifact
            }
            "extra-type" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types
                .push(CardKind::Enchantment),
            "missing-type" => {
                a.program.targets[0].filter.as_mut().unwrap().types.pop();
            }
            "non-types" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_types
                .push(CardKind::Land),
            "non-colors" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_colors
                .push(Color::B),
            "opponent" => a.program.targets[0].filter.as_mut().unwrap().opponent = true,
            "tapped" => a.program.targets[0].filter.as_mut().unwrap().tapped = true,
            "draw" => a.program.instructions.push(Instruction::DrawCards {
                count: 1.into(),
                recipient: None,
                source_block: 0,
            }),
            "ability-zone" => a.functioning_zone = FunctioningZone::Stack,
            "program-zone" => a.program.functioning_zone = FunctioningZone::Battlefield,
            "timing" => a.program.timing = SpellTiming::MainPhase,
            "resolution" => a.program.resolution = SpellResolution::Permanent,
            "additional" => {
                a.program.additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Creature],
                    subtypes: vec![],
                })
            }
            "slot" => {
                a.program.instructions[0] = Instruction::TapPermanent {
                    target_slot: 1,
                    source_block: 0,
                }
            }
            _ => unreachable!(),
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{case}");
    }
}

#[test]
fn activated_tap_context_and_requirements_recurse_and_old_programs_stay_valid() {
    let d = activated_tap();
    let child = d.activated[0].program.clone();
    let mut modal = d.clone();
    let p = &mut modal.activated[0].program;
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
    modal.capabilities = modal.requirements();
    assert!(
        modal
            .capabilities
            .contains(&Capability::ActivatedTapPermanentV1)
    );
    assert!(modal.capabilities.contains(&Capability::TapPermanentV1));
    assert!(modal.validate().is_err());
    for program in [child.clone(), modal.activated[0].program.clone()] {
        let mut trigger = d.clone();
        trigger.activated.clear();
        trigger.triggered = vec![TriggeredAbility {
            condition: TriggerCondition::SelfEntersBattlefield,
            death_filter: None,
            intervening_if: None,
            functioning_zone: FunctioningZone::Battlefield,
            program: program.clone(),
        }];
        trigger.capabilities = trigger.requirements();
        assert!(
            !trigger
                .capabilities
                .contains(&Capability::ActivatedTapPermanentV1)
        );
        assert!(trigger.validate().is_err());
        let mut ordinary = tap_tests::tap();
        ordinary.spell = Some(program);
        ordinary.capabilities = ordinary.requirements();
        assert!(
            ordinary.validate().is_err(),
            "new permanent target is not an ordinary tap form"
        );
    }
    for json in [
        include_str!("../../../tests/fixtures/expected-wipes-ir.json"),
        include_str!("../../../tests/fixtures/expected-attachments-ir.json"),
    ] {
        Catalog::read(json).unwrap().validate().unwrap();
    }
    tap_tests::tap().validate().unwrap();
}
