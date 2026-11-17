//! CR121.2/121.4/608.2c: standalone controller draws, one attempt per card.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
/// One complete ordinary draw program with a nonnegative exact quantity.
pub fn controller_counted_draw_program(p: &SpellProgram) -> bool {
    p.additional_sacrifice.is_none()
        && p.modes.is_empty()
        && p.targets.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && matches!(p.instructions.as_slice(),[Instruction::DrawControllerCards{count,source_block:0}]if !count.is_negative())
}
pub(crate) fn extend_schema(s: &mut serde_json::Value) {
    s["$defs"]["ControllerCountedDrawProgram"] = serde_json::json!({"properties":{"additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"instructions":{"minItems":1,"maxItems":1,"items":{"required":["op","count","source_block"],"properties":{"op":{"const":"draw_controller_cards"},"count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}}}}}});
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
}
