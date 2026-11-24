//! "Gain life" and "lose life" (CR 119).
use super::*;

/// CR 119.3: one player gains or loses a positive fixed amount.
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
    let player = lower_player(
        cx.program,
        cx.slots,
        who,
        cx.context,
        cx.player_binding.event_role(),
    )?;
    let amount = Amount::Fixed {
        value: fixed_positive_amount(&typed(amount, "amount")?)?,
    };
    Ok(cx.act(if gain {
        Verb::GainLife { player, amount }
    } else {
        Verb::LoseLife { player, amount }
    }))
}
