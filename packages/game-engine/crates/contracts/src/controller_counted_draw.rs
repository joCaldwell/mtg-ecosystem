//! CR121.2/121.4/608.2c: standalone controller draws, one attempt per card.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
/// A single unbound counted controller draw, with source position supplied by its owner.
pub(crate) fn standalone_controller_draw(p: &SpellProgram) -> Option<(&Quantity, u32)> {
    if p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || !p.targets.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
    {
        return None;
    }
    match p.instructions.as_slice() {
        [
            Instruction::DrawControllerCards {
                count,
                source_block,
            },
        ] if !count.is_negative() => Some((count, *source_block)),
        _ => None,
    }
}
/// Complete counted controller draws, optionally followed by fixed controller life loss.
pub fn controller_counted_draw_program(p: &SpellProgram) -> bool {
    standalone_controller_draw(p).is_some_and(|(_, block)| block == 0)
        || (p.additional_sacrifice.is_none()
            && p.optional_additional_costs.is_empty()
            && p.modes.is_empty()
            && p.targets.is_empty()
            && p.functioning_zone == FunctioningZone::Stack
            && p.resolution == SpellResolution::Instructions
            && match p.instructions.as_slice() {
                [
                    Instruction::DrawControllerCards {
                        count,
                        source_block: 0,
                    },
                    Instruction::LoseLife {
                        recipient: LifeRecipient::Controller,
                        amount,
                        source_block: 0,
                    },
                ] => !count.is_negative() && *amount > Quantity::zero(),
                _ => false,
            })
}
pub(crate) fn extend_schema(s: &mut serde_json::Value) {
    s["$defs"]["ControllerCountedDrawProgram"] = serde_json::json!({"properties":{"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"instructions":{"minItems":1,"maxItems":2,"prefixItems":[{"required":["op","count","source_block"],"properties":{"op":{"const":"draw_controller_cards"},"count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}}},{"required":["op","recipient","amount","source_block"],"properties":{"op":{"const":"lose_life"},"recipient":{"const":"controller"},"amount":{"type":"string","pattern":"^[1-9][0-9]*$"},"source_block":{"const":0}}}],"items":false}}});
}
#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = hand_return::witness();
    d.spell.as_mut().unwrap().instructions.truncate(1);
    d.capabilities = d.requirements();
    d
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controller_counted_draw_quantities_capability_and_cast_kind() {
        for n in ["0", "1", "2", "3", "999999999999999999999999999999"] {
            for kind in [CardKind::Instant, CardKind::Sorcery] {
                let mut d = witness();
                d.characteristics.kind = kind;
                let p = d.spell.as_mut().unwrap();
                p.timing = if kind == CardKind::Instant {
                    SpellTiming::Instant
                } else {
                    SpellTiming::MainPhase
                };
                let Instruction::DrawControllerCards { count, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *count = n.parse().unwrap();
                d.capabilities = d.requirements();
                d.validate().unwrap();
                assert!(controller_counted_draw_program(d.spell.as_ref().unwrap()));
                assert!(
                    d.capabilities
                        .contains(&Capability::ControllerCountedDrawV1)
                );
                assert!(!d.capabilities.contains(&Capability::ControllerHandReturnV1));
                assert!(
                    !d.capabilities
                        .contains(&Capability::ControllerDrawDiscardV1)
                );
                let decoded: Definition = strict_read(&canonical(&d).unwrap()).unwrap();
                assert_eq!(decoded, d);
                let mut missing = d.clone();
                missing
                    .capabilities
                    .remove(&Capability::ControllerCountedDrawV1);
                assert!(missing.validate().is_err());
                for cap in [
                    Capability::ControllerDrawV1,
                    Capability::ControllerHandReturnV1,
                    Capability::ControllerDrawDiscardV1,
                ] {
                    let mut extra = d.clone();
                    extra.capabilities.insert(cap);
                    assert!(extra.validate().is_err());
                }
            }
        }
    }
    #[test]
    fn controller_counted_draw_rejects_nonordinary_costed_and_mixed_programs() {
        for case in 0..8 {
            let mut d = witness();
            let p = d.spell.as_mut().unwrap();
            match case {
                0 => p.instructions.push(p.instructions[0].clone()),
                1 => {
                    if let Instruction::DrawControllerCards { count, .. } = &mut p.instructions[0] {
                        *count = (-1).into()
                    }
                }
                2 => {
                    if let Instruction::DrawControllerCards { source_block, .. } =
                        &mut p.instructions[0]
                    {
                        *source_block = 1
                    }
                }
                3 => {
                    p.additional_sacrifice = Some(SacrificeFilter {
                        types: vec![CardKind::Creature],
                        subtypes: vec![],
                    })
                }
                4 => p.functioning_zone = FunctioningZone::Battlefield,
                5 => d.characteristics.kind = CardKind::Enchantment,
                6 => d.keywords.push(KeywordAbility::Flying),
                _ => {
                    let child = p.clone();
                    p.instructions.clear();
                    p.modes = vec![
                        SpellMode {
                            program: child.clone(),
                        },
                        SpellMode { program: child },
                    ]
                }
            }
            d.capabilities = d.requirements();
            assert!(d.validate().is_err(), "{case}");
        }
    }
    #[test]
    fn controller_counted_draw_disjoint_ir_tail_transitions() {
        for mut d in [hand_return::witness(), draw_discard::witness()] {
            d.validate().unwrap();
            assert!(!controller_counted_draw_program(d.spell.as_ref().unwrap()));
            d.spell.as_mut().unwrap().instructions.truncate(1);
            d.capabilities = d.requirements();
            d.validate().unwrap();
            assert!(controller_counted_draw_program(d.spell.as_ref().unwrap()));
            assert!(!controller_hand_return_program(d.spell.as_ref().unwrap()));
            assert!(!controller_draw_discard_program(d.spell.as_ref().unwrap()));
        }
    }
    #[test]
    fn controller_counted_draw_life_quantities_and_exact_capabilities() {
        for (count, loss) in [
            ("0", "1"),
            ("1", "3"),
            ("3", "3"),
            ("999999999999999999999999", "999999999999999999999999"),
        ] {
            for kind in [CardKind::Instant, CardKind::Sorcery] {
                let mut d = witness();
                d.characteristics.kind = kind;
                let p = d.spell.as_mut().unwrap();
                p.timing = if kind == CardKind::Instant {
                    SpellTiming::Instant
                } else {
                    SpellTiming::MainPhase
                };
                p.instructions = vec![
                    Instruction::DrawControllerCards {
                        count: count.parse().unwrap(),
                        source_block: 0,
                    },
                    Instruction::LoseLife {
                        recipient: LifeRecipient::Controller,
                        amount: loss.parse().unwrap(),
                        source_block: 0,
                    },
                ];
                d.capabilities = d.requirements();
                d.validate().unwrap();
                assert_eq!(
                    d.capabilities,
                    BTreeSet::from([
                        Capability::NormalLayoutV1,
                        if kind == CardKind::Instant {
                            Capability::CastInstantV1
                        } else {
                            Capability::CastSorceryV1
                        },
                        Capability::ControllerCountedDrawV1,
                        Capability::LifeLossV1
                    ])
                );
                let decoded: Definition = strict_read(&canonical(&d).unwrap()).unwrap();
                assert_eq!(decoded, d);
                let mut missing = d.clone();
                missing.capabilities.remove(&Capability::LifeLossV1);
                assert!(missing.validate().is_err());
                let mut pure = d.clone();
                pure.spell.as_mut().unwrap().instructions.pop();
                assert!(pure.validate().is_err());
                pure.capabilities = pure.requirements();
                pure.validate().unwrap();
            }
        }
    }
    #[test]
    fn controller_counted_draw_life_requires_one_positive_controller_tail() {
        for case in 0..10 {
            let mut d = witness();
            d.spell
                .as_mut()
                .unwrap()
                .instructions
                .push(Instruction::LoseLife {
                    recipient: LifeRecipient::Controller,
                    amount: 3.into(),
                    source_block: 0,
                });
            let p = d.spell.as_mut().unwrap();
            match case {
                0 => p.instructions.reverse(),
                1 => p.instructions.push(p.instructions[1].clone()),
                2 => {
                    if let Instruction::LoseLife { amount, .. } = &mut p.instructions[1] {
                        *amount = 0.into()
                    }
                }
                3 => {
                    if let Instruction::LoseLife { amount, .. } = &mut p.instructions[1] {
                        *amount = (-1).into()
                    }
                }
                4 => {
                    if let Instruction::LoseLife { source_block, .. } = &mut p.instructions[1] {
                        *source_block = 1
                    }
                }
                5 => {
                    if let Instruction::LoseLife { recipient, .. } = &mut p.instructions[1] {
                        *recipient = LifeRecipient::EachOpponent
                    }
                }
                6 => {
                    if let Instruction::LoseLife { recipient, .. } = &mut p.instructions[1] {
                        *recipient = LifeRecipient::EventPlayer
                    }
                }
                7 => {
                    if let Instruction::LoseLife { recipient, .. } = &mut p.instructions[1] {
                        *recipient = LifeRecipient::Target { target_slot: 0 }
                    }
                }
                8 => {
                    p.instructions[1] = Instruction::GainLife {
                        recipient: LifeRecipient::Controller,
                        amount: 3.into(),
                        source_block: 0,
                    }
                }
                _ => {
                    p.instructions[0] = Instruction::DrawCards {
                        recipient: None,
                        count: 1.into(),
                        source_block: 0,
                    }
                }
            }
            d.capabilities = d.requirements();
            if case == 9 {
                // This remains valid legacy IR, outside the counted family.
                assert!(!controller_counted_draw_program(d.spell.as_ref().unwrap()));
                d.validate().unwrap();
            } else {
                assert!(d.validate().is_err(), "{case}");
            }
        }
    }
}
