//! Synthetic typed contract probes; exact archived Royal Assassin source is
//! independently compiled and executed by compiler/core source tests.
use super::*;

pub(super) fn tap_source_destroy() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Prodigal Sorcerer")
    .unwrap();
    let p = &mut d.activated[0].program;
    p.targets[0].domain = TargetDomain::BattlefieldPermanent;
    p.targets[0].filter = Some(PermanentFilter {
        types: vec![CardKind::Creature],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        tapped: true,
    });
    p.instructions = vec![Instruction::DestroyPermanent {
        target_slot: 0,
        optional: false,
        regeneration_forbidden: false,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn source_tap_destroy_round_trips_with_existing_unit_cost_and_current_capabilities() {
    let d = tap_source_destroy();
    d.validate().unwrap();
    for cap in [
        Capability::TapSourceDestroyV1,
        Capability::TappedPermanentTargetV1,
        Capability::PermanentRemovalV1,
        Capability::TapSourceCostV1,
    ] {
        assert!(d.capabilities.contains(&cap));
    }
    let value = serde_json::to_value(&d).unwrap();
    assert_eq!(value["activated"][0]["cost"], "tap_source");
    assert_eq!(
        strict_read::<Definition>(&serde_json::to_string(&d).unwrap()).unwrap(),
        d
    );
    let mut c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    c.definitions = vec![d];
    assert_eq!(
        Catalog::read(&serde_json::to_string(&c).unwrap()).unwrap(),
        c
    );
}

#[test]
fn source_tap_destroy_requires_its_contextual_capability_without_promoting_old_costs() {
    for cap in [
        Capability::TapSourceDestroyV1,
        Capability::TappedPermanentTargetV1,
        Capability::PermanentRemovalV1,
        Capability::TapSourceCostV1,
    ] {
        let mut d = tap_source_destroy();
        d.capabilities.remove(&cap);
        assert!(d.validate().is_err(), "{cap:?}");
    }
    // This pre-existing composite sacrifice+mana destroy shape stays separate.
    let mut sacrifice = tap_source_destroy();
    sacrifice.activated[0].cost = ActivationCost::Composite {
        costs: PermanentActivationCosts {
            tap: false,
            sacrifice: true,
            mana: Some(ManaCost {
                generic: 1.into(),
                colored: vec![],
            }),
        },
    };
    sacrifice.capabilities = sacrifice.requirements();
    assert!(
        !sacrifice
            .capabilities
            .contains(&Capability::TapSourceDestroyV1)
    );
    sacrifice.validate().unwrap();
    for (catalog, name) in [
        (
            include_str!("../../../tests/fixtures/expected-wipes-ir.json"),
            "Prodigal Sorcerer",
        ),
        (
            include_str!("../../../tests/fixtures/expected-blocking-matches-ir.json"),
            "Swift Response",
        ),
    ] {
        let mut d = Catalog::read(catalog)
            .unwrap()
            .definitions
            .into_iter()
            .find(|d| d.name == name)
            .unwrap();
        d.validate().unwrap();
        assert!(!d.requirements().contains(&Capability::TapSourceDestroyV1));
        d.capabilities.insert(Capability::TapSourceDestroyV1);
        assert!(d.validate().is_err(), "spare contextual label: {name}");
    }
}

#[test]
fn source_tap_destroy_rejects_unreviewed_costs_filters_programs_and_source_kinds() {
    for case in [
        "mana",
        "composite-tap",
        "sacrifice-source",
        "artifact",
        "untapped",
        "missing-filter",
        "extra-type",
        "non-type",
        "non-color",
        "opponent",
        "optional",
        "regeneration",
        "draw",
        "slot",
        "domain",
        "ability-zone",
        "program-zone",
        "timing",
        "resolution",
        "additional",
    ] {
        let mut d = tap_source_destroy();
        let a = &mut d.activated[0];
        match case {
            "mana" => {
                a.cost = ActivationCost::Mana {
                    cost: ManaCost {
                        generic: 1.into(),
                        colored: vec![],
                    },
                }
            }
            "composite-tap" => {
                a.cost = ActivationCost::Composite {
                    costs: PermanentActivationCosts {
                        tap: true,
                        sacrifice: false,
                        mana: Some(ManaCost {
                            generic: 1.into(),
                            colored: vec![],
                        }),
                    },
                }
            }
            "sacrifice-source" => a.cost = ActivationCost::SacrificeSource,
            "artifact" => {
                d.characteristics.kind = CardKind::Artifact;
                d.characteristics.power = None;
                d.characteristics.toughness = None;
            }
            "untapped" => a.program.targets[0].filter.as_mut().unwrap().tapped = false,
            "missing-filter" => a.program.targets[0].filter = None,
            "extra-type" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types
                .push(CardKind::Artifact),
            "non-type" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_types
                .push(CardKind::Artifact),
            "non-color" => a.program.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_colors
                .push(Color::B),
            "opponent" => a.program.targets[0].filter.as_mut().unwrap().opponent = true,
            "optional" => {
                let Instruction::DestroyPermanent { optional, .. } = &mut a.program.instructions[0]
                else {
                    panic!("destroy")
                };
                *optional = true;
            }
            "regeneration" => {
                let Instruction::DestroyPermanent {
                    regeneration_forbidden,
                    ..
                } = &mut a.program.instructions[0]
                else {
                    panic!("destroy")
                };
                *regeneration_forbidden = true;
            }
            "draw" => a.program.instructions.push(Instruction::DrawCards {
                count: 1.into(),
                recipient: None,
                source_block: 0,
            }),
            "slot" => {
                a.program.instructions = vec![Instruction::DestroyPermanent {
                    target_slot: 1,
                    optional: false,
                    regeneration_forbidden: false,
                    source_block: 0,
                }]
            }
            "domain" => a.program.targets[0].domain = TargetDomain::BattlefieldCreature,
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
            _ => unreachable!(),
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{case}");
    }
}

#[test]
fn source_tap_destroy_requirements_recurse_and_old_icy_and_ordinary_destruction_stay_valid() {
    let mut d = tap_source_destroy();
    let child = d.activated[0].program.clone();
    let p = &mut d.activated[0].program;
    p.targets.clear();
    p.instructions.clear();
    p.modes = vec![
        SpellMode {
            program: child.clone(),
        },
        SpellMode { program: child },
    ];
    d.capabilities = d.requirements();
    assert!(d.capabilities.contains(&Capability::TapSourceDestroyV1));
    assert!(
        d.validate().is_err(),
        "nested destruction cannot evade the context guard"
    );
    d.capabilities.remove(&Capability::TapSourceDestroyV1);
    assert!(d.validate().is_err());
    let icy = activated_permanent_tap_tests::activated_tap();
    assert!(!icy.requirements().contains(&Capability::TapSourceDestroyV1));
    icy.validate().unwrap();
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
