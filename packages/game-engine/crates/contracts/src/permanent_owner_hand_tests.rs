//! Typed contract probes; archived source facts are tested by the compiler.
use super::*;

pub(super) fn permanent_owner_hand() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-removal-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Naturalize")
    .unwrap();
    let p = d.spell.as_mut().unwrap();
    p.targets[0].filter = Some(PermanentFilter {
        types: vec![],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        tapped: false,
    });
    p.instructions = vec![Instruction::ReturnToOwnerHand {
        target_slot: 0,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn ordinary_permanent_owner_hand_accepts_exact_filters_and_spell_kinds() {
    for nonland in [false, true] {
        for sorcery in [false, true] {
            let mut d = permanent_owner_hand();
            if nonland {
                d.spell.as_mut().unwrap().targets[0]
                    .filter
                    .as_mut()
                    .unwrap()
                    .non_types = vec![CardKind::Land];
            }
            if sorcery {
                d.characteristics.kind = CardKind::Sorcery;
                d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            for capability in [
                Capability::PermanentOwnerHandMoveV1,
                Capability::OwnerHandMoveV1,
                Capability::PermanentRemovalV1,
            ] {
                assert!(d.capabilities.contains(&capability));
                let mut missing = d.clone();
                missing.capabilities.remove(&capability);
                assert!(missing.validate().is_err(), "{capability:?}");
            }
            assert_eq!(
                strict_read::<Definition>(&serde_json::to_string(&d).unwrap()).unwrap(),
                d
            );
            let mut catalog = Catalog::read(include_str!(
                "../../../tests/fixtures/expected-removal-ir.json"
            ))
            .unwrap();
            catalog.definitions = vec![d];
            assert_eq!(catalog.schema_version, 1);
            assert_eq!(
                Catalog::read(&serde_json::to_string(&catalog).unwrap()).unwrap(),
                catalog
            );
        }
    }
}

#[test]
fn permanent_owner_hand_rejects_unrepresented_target_qualifiers_and_domains() {
    for case in [
        "missing_filter",
        "types",
        "noncreature",
        "mixed_exclusions",
        "duplicate_exclusion",
        "non_colors",
        "opponent",
        "tapped",
        "controlled_creature",
        "blocking_creature",
        "graveyard_creature",
        "player",
        "stack",
    ] {
        let mut d = permanent_owner_hand();
        let p = d.spell.as_mut().unwrap();
        match case {
            "missing_filter" => p.targets[0].filter = None,
            "types" => p.targets[0].filter.as_mut().unwrap().types = vec![CardKind::Artifact],
            "noncreature" => {
                p.targets[0].filter.as_mut().unwrap().non_types = vec![CardKind::Creature];
            }
            "mixed_exclusions" => {
                p.targets[0].filter.as_mut().unwrap().non_types =
                    vec![CardKind::Land, CardKind::Creature];
            }
            "duplicate_exclusion" => {
                p.targets[0].filter.as_mut().unwrap().non_types =
                    vec![CardKind::Land, CardKind::Land];
            }
            "non_colors" => p.targets[0].filter.as_mut().unwrap().non_colors = vec![Color::B],
            "opponent" => p.targets[0].filter.as_mut().unwrap().opponent = true,
            "tapped" => p.targets[0].filter.as_mut().unwrap().tapped = true,
            "controlled_creature" => {
                p.targets[0].domain = TargetDomain::ControlledBattlefieldCreature;
            }
            "blocking_creature" => p.targets[0].domain = TargetDomain::BlockingBattlefieldCreature,
            "graveyard_creature" => p.targets[0].domain = TargetDomain::OwnGraveyardCreatureCard,
            "player" => p.targets[0].domain = TargetDomain::AnyPlayer,
            "stack" => p.targets[0].domain = TargetDomain::StackSpell,
            _ => unreachable!(),
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{case}");
    }
}

#[test]
fn permanent_owner_hand_rejects_incomplete_or_composed_programs() {
    for case in [
        "missing_target",
        "extra_target",
        "target_slot",
        "instruction_slot",
        "missing_instruction",
        "extra_return",
        "extra_draw",
        "extra_other_effect",
        "cost",
        "zone",
        "resolution",
    ] {
        let mut d = permanent_owner_hand();
        let p = d.spell.as_mut().unwrap();
        match case {
            "missing_target" => p.targets.clear(),
            "extra_target" => {
                let mut target = p.targets[0].clone();
                target.slot = 1;
                target.selection_id = "typed-probe-second-target".into();
                p.targets.push(target);
            }
            "target_slot" => p.targets[0].slot = 1,
            "instruction_slot" => {
                p.instructions[0] = Instruction::ReturnToOwnerHand {
                    target_slot: 1,
                    source_block: 0,
                };
            }
            "missing_instruction" => p.instructions.clear(),
            "extra_return" => p.instructions.push(p.instructions[0].clone()),
            "extra_draw" => p.instructions.push(Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 1,
            }),
            "extra_other_effect" => p.instructions.push(Instruction::DestroyPermanent {
                target_slot: 0,
                source_block: 1,
                optional: false,
                regeneration_forbidden: false,
            }),
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
fn permanent_owner_hand_capability_recurses_but_other_contexts_reject() {
    let d = permanent_owner_hand();
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
    assert!(
        modal
            .capabilities
            .contains(&Capability::PermanentOwnerHandMoveV1)
    );
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
        assert!(
            permanent
                .capabilities
                .contains(&Capability::PermanentOwnerHandMoveV1)
        );
        assert!(permanent.validate().is_err());
        permanent
            .capabilities
            .remove(&Capability::PermanentOwnerHandMoveV1);
        assert!(permanent.validate().is_err());
    }
    let mut permanent = d;
    permanent.characteristics.kind = CardKind::Creature;
    permanent.capabilities = permanent.requirements();
    assert!(permanent.validate().is_err());
}

#[test]
fn retained_creature_owner_hand_definition_keeps_existing_capabilities() {
    let d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-removal-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Unsummon")
    .unwrap();
    let p = d.spell.as_ref().unwrap();
    assert_eq!(p.targets[0].domain, TargetDomain::BattlefieldCreature);
    assert_eq!(p.targets[0].filter, None);
    assert!(matches!(
        p.instructions.as_slice(),
        [Instruction::ReturnToOwnerHand { target_slot: 0, .. }]
    ));
    assert_eq!(d.capabilities, d.requirements());
    assert!(d.capabilities.contains(&Capability::OwnerHandMoveV1));
    assert!(d.capabilities.contains(&Capability::CreatureTargetV1));
    assert!(!d.capabilities.contains(&Capability::PermanentRemovalV1));
    assert!(
        !d.capabilities
            .contains(&Capability::PermanentOwnerHandMoveV1)
    );
    d.validate().unwrap();
}
