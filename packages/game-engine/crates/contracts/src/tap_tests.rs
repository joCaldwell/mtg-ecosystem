//! Typed contract probes, not claims about an archived card's source semantics.
use super::*;

pub(super) fn tap() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-removal-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Naturalize")
    .unwrap();
    let p = d.spell.as_mut().unwrap();
    p.targets[0].domain = TargetDomain::BattlefieldCreature;
    p.targets[0].filter = None;
    p.instructions = vec![
        Instruction::TapPermanent {
            target_slot: 0,
            source_block: 0,
        },
        Instruction::DrawCards {
            count: 1.into(),
            recipient: None,
            source_block: 1,
        },
    ];
    d.capabilities = d.requirements();
    d
}

#[test]
fn ordinary_tap_and_optional_draw_one_round_trip_in_current_schema() {
    for cantrip in [false, true] {
        for sorcery in [false, true] {
            let mut d = tap();
            if !cantrip {
                d.spell.as_mut().unwrap().instructions.truncate(1);
            }
            if sorcery {
                d.characteristics.kind = CardKind::Sorcery;
                d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            assert!(d.capabilities.contains(&Capability::TapPermanentV1));
            assert!(d.capabilities.contains(&Capability::CreatureTargetV1));
            assert_eq!(
                d.capabilities.contains(&Capability::ControllerDrawV1),
                cantrip
            );
            let json = serde_json::to_string(&d).unwrap();
            assert_eq!(strict_read::<Definition>(&json).unwrap(), d);
            let mut catalog = Catalog::read(include_str!(
                "../../../tests/fixtures/expected-removal-ir.json"
            ))
            .unwrap();
            catalog.definitions = vec![d];
            assert_eq!(catalog.schema_version, 1);
            let json = serde_json::to_string(&catalog).unwrap();
            assert_eq!(Catalog::read(&json).unwrap(), catalog);
        }
    }
}

#[test]
fn tap_requires_its_capability_and_strict_instruction_fields() {
    let mut d = tap();
    d.capabilities.remove(&Capability::TapPermanentV1);
    assert!(d.validate().is_err());
    let encoded = r#"{"op":"tap_permanent","target_slot":0,"source_block":0}"#;
    assert_eq!(
        strict_read::<Instruction>(encoded).unwrap(),
        Instruction::TapPermanent {
            target_slot: 0,
            source_block: 0
        }
    );
    for encoded in [
        r#"{"op":"tap_permanent","target_slot":0,"source_block":0,"optional":false}"#,
        r#"{"op":"tap_permanent","target_slot":0}"#,
        r#"{"op":"tap_permanent","target_slot":null,"source_block":0}"#,
        r#"{"op":"tap_permanent","target_slot":0,"source_block":"0"}"#,
    ] {
        assert!(strict_read::<Instruction>(encoded).is_err(), "{encoded}");
    }
}

#[test]
fn tap_rejects_qualified_or_mismatched_targets_and_additional_costs() {
    for case in [
        "permanent",
        "controlled",
        "blocking",
        "player",
        "empty_filter",
        "typed_filter",
        "tapped_filter",
        "missing_target",
        "extra_target",
        "target_slot",
        "instruction_slot",
        "cost",
        "zone",
        "resolution",
    ] {
        let mut d = tap();
        let p = d.spell.as_mut().unwrap();
        match case {
            "permanent" => p.targets[0].domain = TargetDomain::BattlefieldPermanent,
            "controlled" => p.targets[0].domain = TargetDomain::ControlledBattlefieldCreature,
            "blocking" => p.targets[0].domain = TargetDomain::BlockingBattlefieldCreature,
            "player" => p.targets[0].domain = TargetDomain::AnyPlayer,
            "empty_filter" | "typed_filter" | "tapped_filter" => {
                p.targets[0].filter = Some(PermanentFilter {
                    types: if case == "typed_filter" {
                        vec![CardKind::Creature]
                    } else {
                        vec![]
                    },
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: case == "tapped_filter",
                });
            }
            "missing_target" => p.targets.clear(),
            "extra_target" => {
                let mut target = p.targets[0].clone();
                target.slot = 1;
                target.selection_id = "typed-probe-second-target".into();
                p.targets.push(target);
            }
            "target_slot" => p.targets[0].slot = 1,
            "instruction_slot" => {
                p.instructions[0] = Instruction::TapPermanent {
                    target_slot: 1,
                    source_block: 0,
                };
            }
            "cost" => {
                p.additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Artifact],
                    subtypes: vec![],
                });
            }
            "zone" => p.functioning_zone = FunctioningZone::Battlefield,
            "resolution" => p.resolution = SpellResolution::Permanent,
            _ => unreachable!(),
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{case}");
    }
}

#[test]
fn tap_rejects_other_draw_counts_recipients_orders_and_compositions() {
    for case in [
        "draw_zero",
        "draw_two",
        "explicit_controller",
        "each_opponent",
        "target_recipient",
        "draw_first",
        "duplicate_tap",
        "duplicate_draw",
        "extra_effect",
    ] {
        let mut d = tap();
        let p = d.spell.as_mut().unwrap();
        match case {
            "draw_zero" | "draw_two" => {
                let Instruction::DrawCards { count, .. } = &mut p.instructions[1] else {
                    unreachable!()
                };
                *count = if case == "draw_zero" {
                    0.into()
                } else {
                    2.into()
                };
            }
            "explicit_controller" | "each_opponent" | "target_recipient" => {
                let Instruction::DrawCards { recipient, .. } = &mut p.instructions[1] else {
                    unreachable!()
                };
                *recipient = Some(match case {
                    "explicit_controller" => LifeRecipient::Controller,
                    "each_opponent" => LifeRecipient::EachOpponent,
                    "target_recipient" => LifeRecipient::Target { target_slot: 0 },
                    _ => unreachable!(),
                });
            }
            "draw_first" => p.instructions.swap(0, 1),
            "duplicate_tap" => p.instructions.push(p.instructions[0].clone()),
            "duplicate_draw" => p.instructions.push(p.instructions[1].clone()),
            "extra_effect" => p.instructions.push(Instruction::UntapPermanent {
                target_slot: 0,
                source_block: 2,
            }),
            _ => unreachable!(),
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{case}");
    }
}

#[test]
fn tap_context_rejection_and_capability_detection_recurse_through_modes() {
    let d = tap();
    let child = d.spell.clone().unwrap();
    let mut modal = d.clone();
    let p = modal.spell.as_mut().unwrap();
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
    let nested = modal.spell.clone().unwrap();
    modal.spell.as_mut().unwrap().modes[0].program = nested.clone();
    modal.capabilities = modal.requirements();
    assert!(modal.capabilities.contains(&Capability::TapPermanentV1));
    assert!(modal.validate().is_err());
    for (activated, program) in [
        (true, child.clone()),
        (false, child),
        (true, nested.clone()),
        (false, nested),
    ] {
        let mut permanent = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
        if activated {
            permanent.activated.push(ActivatedAbility {
                functioning_zone: FunctioningZone::Battlefield,
                cost: ActivationCost::TapSource,
                program,
            });
        } else {
            permanent.triggered.push(TriggeredAbility {
                condition: TriggerCondition::SelfEntersBattlefield,
                intervening_if: None,
                death_filter: None,
                functioning_zone: FunctioningZone::Battlefield,
                program,
            });
        }
        permanent.capabilities = permanent.requirements();
        assert!(permanent.capabilities.contains(&Capability::TapPermanentV1));
        assert!(permanent.validate().is_err());
        permanent.capabilities.remove(&Capability::TapPermanentV1);
        assert!(permanent.validate().is_err());
    }
    let mut permanent = d;
    permanent.characteristics.kind = CardKind::Creature;
    permanent.capabilities = permanent.requirements();
    assert!(permanent.validate().is_err());
}
