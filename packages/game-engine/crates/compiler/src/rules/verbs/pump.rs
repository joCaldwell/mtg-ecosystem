//! "Gets +N/+N until end of turn" (CR 611.2c, 613.4c).
use super::*;

/// A signed P/T change until end of turn of a reviewed board set, the exact
/// source or one announced creature: fixed, by announced X, or (CR 608.2h)
/// by a resolution count read once as the effect begins, either the
/// sentence's bound X or "+1/+1 for each [filter]".
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    power: &ast::SignedAmount,
    toughness: &ast::SignedAmount,
    duration: &SourceDuration,
    for_each: Option<&serde_json::Value>,
) -> Result<Instruction, Failure> {
    let bound = cx.bound_x();
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
    let (power, toughness) = match (for_each, &bound) {
        (Some(_), Some(_)) => return Err(operand("forEach", "bound_x")),
        (Some(for_each), None) => {
            let one = ast::Amount::Fixed(Box::new(ast::AmountFixed { value: 1 }));
            // CR 608.2h: the count is read once, as the effect begins.
            let count = lower_for_each(for_each, &one, cx.context)?;
            (per_object(power, &count)?, per_object(toughness, &count)?)
        }
        (None, bound) => (
            delta(power, bound.as_ref())?,
            delta(toughness, bound.as_ref())?,
        ),
    };
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

/// CR 608.2h: "+N/+N for each [filter]" changes by N times the count; a
/// zero part stays fixed.
fn per_object(value: &ast::SignedAmount, count: &Amount) -> Result<kir::PumpDelta, Failure> {
    let n = signed_fixed(value)?;
    if n == Quantity::zero() {
        return Ok(kir::PumpDelta::Fixed { value: n });
    }
    let negative = n.is_negative();
    let mut factor = n;
    if negative {
        let mut positive = Quantity::zero();
        positive.subtract(&factor);
        factor = positive;
    }
    Ok(kir::PumpDelta::Counted {
        amount: if factor == Quantity::from(1) {
            count.clone()
        } else {
            Amount::Times {
                factor,
                of: Box::new(count.clone()),
            }
        },
        negative,
    })
}

fn delta(value: &ast::SignedAmount, bound: Option<&Amount>) -> Result<kir::PumpDelta, Failure> {
    let negative = *value.sign == ast::SignedAmountSign::Minus1;
    if matches!(*value.amount, ast::Amount::X(_)) {
        Ok(match bound {
            Some(amount) => kir::PumpDelta::Counted {
                amount: amount.clone(),
                negative,
            },
            None => kir::PumpDelta::X { negative },
        })
    } else {
        Ok(kir::PumpDelta::Fixed {
            value: signed_fixed(value)?,
        })
    }
}
