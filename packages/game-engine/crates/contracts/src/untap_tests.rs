//! Typed contract probes; exact archived source lowering lives in compiler tests.
use super::*;

pub(super) fn untap() -> Definition {
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
    p.instructions = vec![Instruction::UntapPermanent {
        target_slot: 0,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn ordinary_untap_requires_current_capability_and_strict_instruction_fields() {
    let mut d = untap();
    d.validate().unwrap();
    assert!(d.capabilities.contains(&Capability::UntapPermanentV1));
    let json = serde_json::to_string(&d).unwrap();
    assert_eq!(strict_read::<Definition>(&json).unwrap(), d);
    d.capabilities.remove(&Capability::UntapPermanentV1);
    assert!(d.validate().is_err());
    for json in [
        r#"{"op":"untap_permanent","target_slot":0,"source_block":0,"optional":false}"#,
        r#"{"op":"untap_permanent","target_slot":0}"#,
        r#"{"op":"untap_permanent","target_slot":null,"source_block":0}"#,
    ] {
        assert!(strict_read::<Instruction>(json).is_err());
    }
}

#[test]
fn untap_rejects_qualified_targets_and_incomplete_or_composed_programs() {
    for case in [
        "types",
        "non_types",
        "non_colors",
        "opponent",
        "tapped",
        "filter",
        "domain",
        "slot",
        "extra",
        "targets",
        "cost",
        "zone",
        "resolution",
    ] {
        let mut d = untap();
        let p = d.spell.as_mut().unwrap();
        match case {
            "types" => p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types
                .push(CardKind::Creature),
            "non_types" => p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_types
                .push(CardKind::Creature),
            "non_colors" => p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_colors
                .push(Color::B),
            "opponent" => p.targets[0].filter.as_mut().unwrap().opponent = true,
            "tapped" => p.targets[0].filter.as_mut().unwrap().tapped = true,
            "filter" => p.targets[0].filter = None,
            "domain" => p.targets[0].domain = TargetDomain::BattlefieldCreature,
            "slot" => {
                p.instructions = vec![Instruction::UntapPermanent {
                    target_slot: 1,
                    source_block: 0,
                }]
            }
            "extra" => p.instructions.push(Instruction::DrawCards {
                count: 1.into(),
                recipient: None,
                source_block: 0,
            }),
            "targets" => p.targets.clear(),
            "cost" => {
                p.additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Creature],
                    subtypes: vec![],
                })
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
fn untap_context_rejection_and_requirements_recurse_without_trusting_labels() {
    let d = untap();
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
    modal.capabilities = modal.requirements();
    assert!(modal.capabilities.contains(&Capability::UntapPermanentV1));
    assert!(modal.validate().is_err());
    let nested = modal.spell.clone().unwrap();
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
                .contains(&Capability::UntapPermanentV1)
        );
        assert!(permanent.validate().is_err());
        permanent.capabilities.remove(&Capability::UntapPermanentV1);
        assert!(permanent.validate().is_err());
    }
}
