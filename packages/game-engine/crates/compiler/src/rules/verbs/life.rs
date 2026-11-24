//! "Gain life" and "lose life" (CR 119).
use super::*;

/// CR 107.1b, 119.3, 608.2h: one life change with current counts or a
/// targeted creature's last known power when the text calls for it.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    gain: bool,
    who: &PlayerSelector,
    amount: &serde_json::Value,
    for_each: Option<&serde_json::Value>,
) -> Result<Instruction, Failure> {
    if for_each.is_some() {
        return Err(unsupported("forEach", "unsupported event result"));
    }
    uncounted(cx)?;
    let amount: ast::Amount = typed(amount, "amount")?;
    let (player, amount) = match (gain, typed::<ast::PlayerRef>(who, "player")?) {
        (true, ast::PlayerRef::Controller(controller)) => {
            let slot = preceding_target(cx.program, cx.source_block, &controller.of)?;
            let object = kir::Ref::Target { slot };
            let amount = match amount {
                ast::Amount::Attribute(attribute)
                    if attribute.attribute == ast::PropertyConstraintProperty::Power
                        && *attribute.of == *controller.of =>
                {
                    Amount::StatOf {
                        object,
                        stat: kir::Stat::Power,
                    }
                }
                other => Amount::Fixed {
                    value: fixed_positive_amount(&other)?,
                },
            };
            (PlayerExpr::ControllerOf { object }, amount)
        }
        _ => (
            lower_player(
                cx.program,
                cx.slots,
                who,
                cx.context,
                cx.player_binding.event_role(),
            )?,
            match &amount {
                ast::Amount::Count(count) => lower_count(count, cx.context)?,
                ast::Amount::X(_) => Amount::announced_x(),
                _ => Amount::Fixed {
                    value: fixed_positive_amount(&amount)?,
                },
            },
        ),
    };
    Ok(cx.act(if gain {
        Verb::GainLife { player, amount }
    } else {
        Verb::LoseLife { player, amount }
    }))
}
