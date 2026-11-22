//! Synthetic IR controls; complete archived source lives in compiler tests.
use super::*;
fn base() -> Definition {
    Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json"))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| d.characteristics.kind == CardKind::Creature)
        .unwrap()
}
fn grant(minimum: Quantity) -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::GrantSpellCascade {
            eligibility: SpellCascadeEligibility::ManaValueAtLeast { minimum },
            program: canonical_cascade_program(7),
        },
    }
}
#[test]
fn static_spell_cascade_grant_generic_exact_quantities_vectors_and_no_intrinsic_requirement() {
    for minimum in [
        0.into(),
        2.into(),
        9.into(),
        "999999999999999999999999999999999999999999999999"
            .parse::<Quantity>()
            .unwrap(),
    ] {
        for count in [1, 2, 17, 257] {
            let mut d = base();
            d.triggered.clear();
            d.static_abilities = vec![grant(minimum.clone()); count];
            d.capabilities = d.requirements();
            d.validate().unwrap();
            for cap in [
                Capability::StaticSpellCascadeGrantV1,
                Capability::CascadeV1,
                Capability::ResolutionCastV1,
                Capability::RandomBottomV1,
            ] {
                let mut bad = d.clone();
                bad.capabilities.remove(&cap);
                assert!(bad.validate().is_err());
            }
            let c = Catalog {
                schema_version: CURRENT_IR_VERSION,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d],
            };
            assert_eq!(c.cast_trigger_fanout_bound(400), Some(400 * count));
        }
    }
}
#[test]
fn static_spell_cascade_grant_canonical_program_zone_and_quantity_fences() {
    for mutation in 0..10 {
        let mut d = base();
        d.static_abilities = vec![grant(6.into())];
        let a = &mut d.static_abilities[0];
        let StaticEffect::GrantSpellCascade {
            eligibility,
            program,
        } = &mut a.effect
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                *eligibility = SpellCascadeEligibility::ManaValueAtLeast {
                    minimum: (-1).into(),
                }
            }
            1 => a.functioning_zone = FunctioningZone::Stack,
            2 => program.timing = SpellTiming::MainPhase,
            3 => program.functioning_zone = FunctioningZone::Battlefield,
            4 => program.resolution = SpellResolution::Permanent,
            5 => program.instructions.clear(),
            6 => program.instructions.push(program.instructions[0].clone()),
            7 => program.instructions.push(Instruction::draw(
                PlayerExpr::you(Context::Spell),
                1.into(),
                7,
            )),
            8 => program
                .optional_additional_costs
                .push(OptionalAdditionalManaCost {
                    identity: "synthetic".into(),
                    source_block: 0,
                    mana: ManaCost {
                        generic: 0.into(),
                        colored: vec![],
                    },
                }),
            9 => program.modes.push(SpellMode {
                program: canonical_cascade_program(7),
            }),
            _ => unreachable!(),
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "shape {mutation}");
    }
}

#[test]
fn static_spell_cascade_grant_history_disjoint_capability_closure() {
    for eligibility in [
        SpellCascadeEligibility::FirstCastEachTurn {},
        SpellCascadeEligibility::FirstCastFromExileEachTurn {},
    ] {
        let mut d = base();
        d.triggered.clear();
        d.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantSpellCascade {
                eligibility,
                program: canonical_cascade_program(0),
            },
        }];
        d.capabilities = d.requirements();
        d.validate().unwrap();
        for cap in [
            Capability::CastHistoryV1,
            Capability::StaticSpellCascadeGrantV1,
            Capability::CascadeV1,
            Capability::ResolutionCastV1,
            Capability::RandomBottomV1,
        ] {
            let mut bad = d.clone();
            assert!(bad.capabilities.remove(&cap));
            assert!(bad.validate().is_err());
        }
        let mut wire = serde_json::to_value(&d).unwrap();
        wire["static_abilities"][0]["effect"]["grant_spell_cascade"]["eligibility"]["minimum"] =
            serde_json::json!("1");
        assert!(serde_json::from_value::<Definition>(wire).is_err());
    }
    let mut d = base();
    d.capabilities.insert(Capability::CastHistoryV1);
    assert!(d.validate().is_err());
}
