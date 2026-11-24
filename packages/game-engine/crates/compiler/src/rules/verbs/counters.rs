//! "Put counters" (CR 122).
use super::*;

/// CR 122.1, 122.6: a positive fixed number of +1/+1 counters on the exact
/// source or one announced creature.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    on: &SourceCounterRecipient,
    counter: &SourceCounter,
    count: &serde_json::Value,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if counter.r#type != "+1/+1" {
        return Err(step("put-counters", "counter_kind"));
    }
    let on: ast::ObjectRef = typed(on, "object")?;
    let on = if is_this_creature(&on) {
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
