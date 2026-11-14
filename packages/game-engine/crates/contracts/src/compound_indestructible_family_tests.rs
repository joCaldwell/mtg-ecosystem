//! Synthetic exact IR parameters, never invented real-card source claims.
use super::*;
use compound_indestructible_tests::compound;

#[test]
fn compound_indestructible_family_returns_signed_exact_parameters() {
    let capabilities = compound().capabilities;
    for (p, t) in [
        ("1", "1"),
        ("2", "2"),
        ("2", "0"),
        ("0", "0"),
        ("-2", "-1"),
        (
            "1234567890123456789012345678901234567890",
            "-999999999999999999999999999999999999999",
        ),
    ] {
        let mut d = compound();
        let program = d.spell.as_mut().unwrap();
        let Instruction::GrantKeywords {
            modifier: Some(CreatureStatModifier { power, toughness }),
            ..
        } = &mut program.instructions[0]
        else {
            unreachable!()
        };
        *power = p.parse().unwrap();
        *toughness = t.parse().unwrap();
        let (power, toughness) = compound_indestructible_parameters(program).unwrap();
        assert_eq!(power.to_string(), p);
        assert_eq!(toughness.to_string(), t);
        assert_eq!(
            d.requirements(),
            capabilities,
            "quantities do not select capabilities"
        );
        d.validate().unwrap();
        let json = canonical(&d).unwrap();
        let roundtrip: Definition = strict_read(&json).unwrap();
        roundtrip.validate().unwrap();
        assert_eq!(roundtrip, d);
    }
    let mut d = compound();
    d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
    d.characteristics.kind = CardKind::Sorcery;
    d.capabilities = d.requirements();
    d.validate().unwrap();
    assert!(compound_indestructible_parameters(d.spell.as_ref().unwrap()).is_some());
    // Kind is intentionally the definition validator/caller's responsibility.
    d.characteristics.kind = CardKind::Creature;
    assert!(compound_indestructible_parameters(d.spell.as_ref().unwrap()).is_some());
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}

#[test]
fn compound_indestructible_family_helper_checks_whole_program() {
    for variant in 0..14 {
        let mut d = compound();
        let p = d.spell.as_mut().unwrap();
        match variant {
            0 => compound_indestructible_tests::split(p, true),
            1 => p.instructions.push(p.instructions[0].clone()),
            2 => p.targets[0].selection_id.clear(),
            3 => p.targets[0].slot = 1,
            4 => p.targets[0].domain = TargetDomain::ControlledBattlefieldCreature,
            5 => {
                p.targets[0].filter = Some(PermanentFilter {
                    types: vec![],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                })
            }
            6 => p.functioning_zone = FunctioningZone::Battlefield,
            7 => p.resolution = SpellResolution::Permanent,
            8 => {
                p.additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Artifact],
                    subtypes: vec![],
                })
            }
            9 => {
                let child = p.clone();
                p.modes.push(SpellMode { program: child });
            }
            10 => compound_indestructible_tests::split(p, false),
            11 => {
                let Instruction::GrantKeywords { source_block, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *source_block = 1;
            }
            12 => {
                let Instruction::GrantKeywords { target_slot, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *target_slot = 1;
            }
            _ => {
                let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                keywords.push(KeywordAbility::Flying);
            }
        }
        assert!(
            compound_indestructible_parameters(p).is_none(),
            "variant {variant}"
        );
    }
    for d in [
        temporary_indestructible_tests::grant(),
        temporary_flying_tests::grant(),
        temporary_deathtouch_tests::grant(),
    ] {
        assert!(compound_indestructible_parameters(d.spell.as_ref().unwrap()).is_none());
    }
}

#[test]
fn compound_indestructible_family_schema_reuses_exact_quantity_and_shape() {
    let schema = catalog_schema();
    let instructions = &schema["$defs"]["CompoundIndestructibleInstructions"];
    assert_eq!(instructions["minItems"], 1);
    assert_eq!(instructions["maxItems"], 1);
    assert_eq!(
        instructions["items"]["required"],
        serde_json::json!(["modifier"])
    );
    assert_eq!(
        instructions["items"]["properties"]["modifier"],
        serde_json::json!({"$ref":"#/$defs/CreatureStatModifier"})
    );
    let modifier = &schema["$defs"]["CreatureStatModifier"];
    assert_eq!(modifier["additionalProperties"], false);
    assert_eq!(
        modifier["required"],
        serde_json::json!(["power", "toughness"])
    );
    for field in ["power", "toughness"] {
        assert_eq!(
            modifier["properties"][field],
            serde_json::json!({"$ref":"#/$defs/ExactQuantity"})
        );
    }
    assert_eq!(
        schema["$defs"]["ExactQuantity"]["pattern"],
        "^(0|[1-9][0-9]*|-[1-9][0-9]*)$"
    );
    {
        let properties = &instructions["items"]["properties"];
        assert_eq!(properties["source_block"]["const"], 0);
        assert_eq!(properties["target_slot"]["const"], 0);
        assert_eq!(properties["duration"]["const"], "until_end_of_turn");
    }
    assert_eq!(
        instructions["items"]["properties"]["keywords"]["const"],
        serde_json::json!(["indestructible"])
    );
    for field in ["power", "toughness"] {
        for invalid in [
            serde_json::json!(2),
            serde_json::json!(null),
            serde_json::json!("+2"),
            serde_json::json!("-0"),
            serde_json::json!("02"),
            serde_json::json!("1.5"),
        ] {
            let mut raw = serde_json::to_value(compound()).unwrap();
            raw["spell"]["instructions"][0]["modifier"][field] = invalid;
            assert!(strict_read::<Definition>(&raw.to_string()).is_err());
        }
    }
    // Other durations remain outside the IR vocabulary.
    let mut raw = serde_json::to_value(compound()).unwrap();
    raw["spell"]["instructions"][0]["duration"] = "forever".into();
    assert!(strict_read::<Definition>(&raw.to_string()).is_err());
}
