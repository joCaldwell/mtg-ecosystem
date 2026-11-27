//! "Put counters" (CR 122).
use super::*;

/// CR 122.1, 122.6: a positive fixed number of +1/+1 counters, or a fixed
/// or announced-X number of -1/-1 counters (CR 122.1a), on the exact source,
/// one announced creature or a matching creature set.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    counters: &ast::EffectPutCounters,
) -> Result<Instruction, Failure> {
    let ast::EffectPutCounters {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        counter,
        count,
        on,
        who,
        up_to,
        max_total,
    } = counters;
    if starting_with.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
        || up_to.is_some()
        || max_total.is_some()
        || who
            .as_deref()
            .is_some_and(|who| !matches!(who, ast::PlayerRef::You(_)))
        || (decided_by.is_some()
            && !(optional.is_some()
                && matches!(decided_by.as_deref(), Some(ast::PlayerRef::You(_)))))
    {
        return Err(step("put-counters", "qualified"));
    }
    let counter = object_counter_kind(&counter.r#type)?;
    let amount = if let Some(filter) = for_each {
        lower_each_filter((**filter).clone(), count, cx.context)?
    } else {
        pilot_amount(cx, count)?
    };
    let on = on.as_ref();
    let on = if is_set(on) {
        pilot_board_set(cx, on)?
    } else if is_this_creature(on) || (!counter.needs_creature() && is_this_permanent(on)) {
        kir::SetExpr::source()
    } else if matches!(on, ast::ObjectRef::It(_) | ast::ObjectRef::That(_))
        && let Ok(object @ Ref::EventObject { .. }) = pilot_object(cx, on)
    {
        SetExpr::One { object }
    } else if let Some(slot) = continuous::followup_target(cx, on) {
        SetExpr::target(slot)
    } else if counter.needs_creature() {
        kir::SetExpr::target(bind_creature_target(cx.program, cx.slots, on)?)
    } else {
        // CR 122.1: other kinds go on any permanent the target admits.
        kir::SetExpr::target(
            bind_creature_target(cx.program, cx.slots, on)
                .or_else(|_| bind_permanent_target(cx.program, cx.slots, on))?,
        )
    };
    Ok(cx.act(Verb::PutCounters {
        on,
        counter,
        amount,
    }))
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

/// CR 122.1, 609.3: the fixed one-lore removal on an announced permanent.
pub(super) fn remove(
    cx: &mut StepCx<'_>,
    from: &serde_json::Value,
    counter: &SourceCounter,
    count: &serde_json::Value,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if counter.r#type != "lore"
        || fixed_positive_amount(&typed(count, "amount")?)? != Quantity::from(1)
    {
        return Err(step("remove-counters", "counter_kind_or_amount"));
    }
    let from = bind_permanent_target(cx.program, cx.slots, &object(from)?)?;
    Ok(cx.act(Verb::RemoveLoreCounter {
        from: kir::Ref::Target { slot: from },
    }))
}
