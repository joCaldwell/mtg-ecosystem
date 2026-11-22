//! CR121.2/701.9/608.2c: whole controller draw, then chosen simultaneous discard.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub fn controller_draw_discard_program(p: &SpellProgram) -> bool {
    p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.modes.is_empty()
        && p.targets.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && matches!(p.instructions.as_slice(),[kernel!(Draw { player: draw_player, amount: fixed!(draw) } @ 0),kernel!(Discard { player: discard_player, amount: fixed!(count), random: false } @ 0)] if *draw_player == kir::PlayerExpr::you(kir::Context::Spell) && *discard_player == kir::PlayerExpr::you(kir::Context::Spell) && !draw.is_negative() && !count.is_negative())
}
pub(crate) fn extend_schema(s: &mut serde_json::Value) {
    use serde_json::json;
    s["$defs"]["ControllerDrawDiscardProgram"] = json!({"properties":{"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"instructions":{"minItems":2,"maxItems":2,"prefixItems":[instruction_schema::at(instruction_schema::fixed_spell_controller_draw("^(0|[1-9][0-9]*)$"), 0),instruction_schema::at(instruction_schema::spell_controller_discard(json!({"type":"string","pattern":"^(0|[1-9][0-9]*)$"})), 0)],"items":false}}});
}
#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = hand_return::witness();
    d.spell.as_mut().unwrap().instructions[1] = Instruction::discard(
        kir::PlayerExpr::you(kir::Context::Spell),
        kir::Amount::Fixed { value: 2.into() },
        false,
        0,
    );
    d.capabilities = d.requirements();
    d
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draw_discard_quantity_and_whole_scope() {
        for draw_count in ["1", "2"] {
            for n in ["0", "1", "2", "99999999999999999999999"] {
                let mut d = witness();
                d.spell.as_mut().unwrap().instructions = vec![
                    Instruction::draw(
                        kir::PlayerExpr::you(kir::Context::Spell),
                        draw_count.parse().unwrap(),
                        0,
                    ),
                    Instruction::discard(
                        kir::PlayerExpr::you(kir::Context::Spell),
                        kir::Amount::Fixed {
                            value: n.parse().unwrap(),
                        },
                        false,
                        0,
                    ),
                ];
                d.capabilities = d.requirements();
                d.validate().unwrap();
                assert!(controller_draw_discard_program(d.spell.as_ref().unwrap()));
                assert!(
                    d.capabilities
                        .contains(&Capability::ControllerDrawDiscardV1)
                );
                assert!(!d.capabilities.contains(&Capability::ControllerDrawV1));
                assert!(
                    !d.capabilities
                        .contains(&Capability::ControllerCountedDrawV1)
                );
            }
        }
        // Activated looting abilities share the [Draw(1), Discard(1)] shape
        // under Context::Activated and stay separate from spell draw/discard.
        let mut activated = witness();
        activated.spell.as_mut().unwrap().instructions = vec![
            Instruction::draw(kir::PlayerExpr::you(kir::Context::Activated), 1.into(), 0),
            Instruction::discard(
                kir::PlayerExpr::you(kir::Context::Activated),
                kir::Amount::Fixed { value: 1.into() },
                false,
                0,
            ),
        ];
        assert!(!controller_draw_discard_program(
            activated.spell.as_ref().unwrap()
        ));
        for n in ["0", "2", "99999999999999999999999"] {
            let mut d = witness();
            if let kernel!(Discard {
                player: you!() | event_player!(),
                amount: fixed!(count),
                random: false
            }) = &mut d.spell.as_mut().unwrap().instructions[1]
            {
                *count = n.parse().unwrap();
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            for case in 0..7 {
                let mut b = d.clone();
                let p = b.spell.as_mut().unwrap();
                match case {
                    0 => {
                        p.instructions.pop();
                        // A truncated tail is now valid counted IR; two counted instructions remain outside every family.
                        p.instructions.push(p.instructions[0].clone());
                    }
                    1 => p.instructions.swap(0, 1),
                    2 => p.instructions.push(p.instructions[1].clone()),
                    3 => {
                        if let kernel!(Discard {
                            player: you!() | event_player!(),
                            amount: fixed!(count),
                            random: false
                        }) = &mut p.instructions[1]
                        {
                            *count = (-1).into()
                        }
                    }
                    4 => {
                        if let kernel!(Discard { player, .. }) = &mut p.instructions[1] {
                            *player = kir::PlayerExpr::event(kir::EventPlayerRole::DamagedPlayer)
                        }
                    }
                    5 => {
                        if let kernel!(Discard { player: you!() | event_player!(), amount: fixed!(_), random: false } @ source_block) =
                            &mut p.instructions[1]
                        {
                            *source_block = 1
                        }
                    }
                    _ => b.characteristics.kind = CardKind::Enchantment,
                };
                b.capabilities = b.requirements();
                assert!(b.validate().is_err(), "{case}");
            }
        }
    }
}
