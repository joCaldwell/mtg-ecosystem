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
        && matches!(p.instructions.as_slice(),[Instruction::DrawControllerCards{count:draw,source_block:0},Instruction::DiscardCards{recipient:None,count,source_block:0}] if !draw.is_negative()&&!count.is_negative())
}
pub(crate) fn extend_schema(s: &mut serde_json::Value) {
    use serde_json::json;
    s["$defs"]["ControllerDrawDiscardProgram"] = json!({"properties":{"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"targets":{"maxItems":0},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},"instructions":{"minItems":2,"maxItems":2,"prefixItems":[{"required":["op","count","source_block"],"properties":{"op":{"const":"draw_controller_cards"},"count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"source_block":{"const":0}}},{"required":["op","count","source_block"],"properties":{"op":{"const":"discard_cards"},"count":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"recipient":{"type":"null"},"source_block":{"const":0}}}],"items":false}}});
}
#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = hand_return::witness();
    d.spell.as_mut().unwrap().instructions[1] = Instruction::DiscardCards {
        recipient: None,
        count: 2.into(),
        source_block: 0,
    };
    d.capabilities = d.requirements();
    d
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draw_discard_quantity_and_whole_scope() {
        for n in ["0", "2", "99999999999999999999999"] {
            let mut d = witness();
            if let Instruction::DiscardCards { count, .. } =
                &mut d.spell.as_mut().unwrap().instructions[1]
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
                        if let Instruction::DiscardCards { count, .. } = &mut p.instructions[1] {
                            *count = (-1).into()
                        }
                    }
                    4 => {
                        if let Instruction::DiscardCards { recipient, .. } = &mut p.instructions[1]
                        {
                            *recipient = Some(ActionRecipient::EventPlayer)
                        }
                    }
                    5 => {
                        if let Instruction::DiscardCards { source_block, .. } =
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
