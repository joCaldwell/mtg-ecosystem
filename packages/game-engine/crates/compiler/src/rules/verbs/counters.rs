//! "Put counters" (CR 122).
use super::*;

/// CR 122.1, 122.6: a positive fixed number of +1/+1 counters on the exact
/// source or one announced creature.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    on: &serde_json::Value,
    counter: &SourceCounter,
    count: &serde_json::Value,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if counter.r#type != "+1/+1" {
        return Err(step("put-counters", "counter_kind"));
    }
    let on = object(on)?;
    let on = if is_set(&on) {
        lower_board_set(&on, cx.context)?
    } else if is_this_creature(&on) {
        kir::SetExpr::source()
    } else {
        kir::SetExpr::target(bind_creature_target(cx.program, cx.slots, &on)?)
    };
    Ok(Instruction::plus_one_counters(
        on,
        fixed_positive_amount(&typed(count, "amount")?)?,
        cx.source_block,
    ))
}

/// CR 107.14, 122.1f, 704.5c: a fixed award to you or to each opponent.
pub(super) fn player(
    cx: &mut StepCx<'_>,
    who: &PlayerSelector,
    counter: &SourceCounter,
    count: &serde_json::Value,
) -> Result<Instruction, Failure> {
    let player = match (who.player.as_str(), &who.selection_id, &who.of) {
        ("you", None, None) => PlayerExpr::you(cx.context),
        ("each-opponent", None, None) => PlayerExpr::EachOpponent,
        _ => return Err(step("get-counters", "player")),
    };
    let counter = match counter.r#type.as_str() {
        "poison" => kir::PlayerCounterKind::Poison,
        "energy" => kir::PlayerCounterKind::Energy,
        _ => return Err(step("get-counters", "counter_kind")),
    };
    Ok(cx.act(Verb::GiveCounters {
        player,
        counter,
        amount: Amount::Fixed {
            value: fixed_positive_amount(&typed(count, "amount")?)?,
        },
    }))
}
