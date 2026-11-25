//! "Gets +N/+N until end of turn" (CR 611.2c, 613.4c).
use super::*;

/// A fixed signed P/T change until end of turn of a reviewed board set, the
/// exact source or one announced creature.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    power: &ast::SignedAmount,
    toughness: &ast::SignedAmount,
    duration: &SourceDuration,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    until_end_of_turn(duration)?;
    let what = object(what)?;
    let set = if is_set(&what) {
        lower_reviewed_set(&what, cx.context)?
    } else if is_this_creature(&what) {
        kir::SetExpr::source()
    } else if let Some(slot) = continuous::followup_target(cx, &what) {
        SetExpr::target(slot)
    } else {
        kir::SetExpr::target(bind_creature_target(cx.program, cx.slots, &what)?)
    };
    let power = delta(power)?;
    let toughness = delta(toughness)?;
    match (power, toughness) {
        (kir::PumpDelta::Fixed { value: power }, kir::PumpDelta::Fixed { value: toughness }) => {
            Ok(Instruction::pump(set, power, toughness, cx.source_block))
        }
        (power, toughness) => Ok(cx.act(Verb::ModifyUntil {
            what: set,
            layer: kir::Layer::PowerToughnessModify,
            change: kir::Change::PowerToughnessX { power, toughness },
            until: kir::Until::EndOfTurn,
        })),
    }
}

fn delta(value: &ast::SignedAmount) -> Result<kir::PumpDelta, Failure> {
    if matches!(*value.amount, ast::Amount::X(_)) {
        Ok(kir::PumpDelta::X {
            negative: *value.sign == ast::SignedAmountSign::Minus1,
        })
    } else {
        Ok(kir::PumpDelta::Fixed {
            value: signed_fixed(value)?,
        })
    }
}
