//! "Discard" (CR 701.9).
use super::*;

/// CR 701.9a: the damaged player ("that player") or "you" discards one
/// card of their choice, in a damage trigger.
pub(super) fn lower(cx: &mut StepCx<'_>, discard: &DiscardStep) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let DiscardStep::Chosen { who, what } = discard else {
        return Err(step("discard", "random"));
    };
    if cx.player_binding != PlayerBinding::DamageRecipient {
        return Err(step("discard", "outside_damage_trigger"));
    }
    if what.reference != "filter"
        || !fixed_one(&what.count)
        || what.filter != ObjectFilter::Class(CardFilter { cls: "card".into() })
    {
        return Err(step("discard", "cardinality"));
    }
    let player = match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::ThatPlayer(_) => PlayerExpr::event(kir::EventPlayerRole::DamagedPlayer),
        ast::PlayerRef::You(_) => PlayerExpr::you(cx.context),
        other => return Err(operand("player", variant(&other))),
    };
    Ok(Instruction::discard(
        player,
        Amount::fixed(1),
        false,
        cx.source_block,
    ))
}
