//! "Discard" (CR 701.9, 101.4, 608.2b-d).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Discard a fixed number of unqualified cards from each named player's
/// own hand. A random discard names one player target of a spell.
pub(super) fn lower(cx: &mut StepCx<'_>, discard: &DiscardStep) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let (who, reference, filter, amount, random) = match discard {
        DiscardStep::Chosen { who, what } if what.count.amount == "fixed" => (
            who,
            &what.reference,
            &what.filter,
            Amount::fixed(i64::from(what.count.value)),
            false,
        ),
        DiscardStep::Random {
            who,
            what,
            random: true,
        } if cx.context == Context::Spell => {
            let amount = match what.count {
                DrawAmount::Fixed { value } => Amount::fixed(i64::from(value)),
                DrawAmount::X {} => Amount::announced_x(),
                DrawAmount::ThatMuch {} => return Err(step("discard", "random_amount")),
            };
            (who, &what.reference, &what.filter, amount, true)
        }
        DiscardStep::Chosen { .. } => return Err(step("discard", "amount")),
        DiscardStep::Random { .. } => return Err(step("discard", "random")),
    };
    if reference != "filter"
        || *filter != ObjectFilter::Class(CardFilter { cls: "card".into() })
        || amount.as_fixed().is_some_and(|n| n <= &Quantity::zero())
    {
        return Err(step("discard", "cardinality"));
    }
    let player = match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::ThatPlayer(_) if cx.player_binding == PlayerBinding::DamageRecipient => {
            PlayerExpr::event(kir::EventPlayerRole::DamagedPlayer)
        }
        ast::PlayerRef::DefendingPlayer(_)
            if cx.player_binding.event_role() == Some(kir::EventPlayerRole::DefendingPlayer) =>
        {
            PlayerExpr::event(kir::EventPlayerRole::DefendingPlayer)
        }
        ast::PlayerRef::EachPlayer(_) => PlayerExpr::EachPlayer,
        ast::PlayerRef::TargetOpponent(target) => match *target {
            ast::PlayerRefTargetOpponent {
                count: None,
                up_to: None,
                random: None,
                selection_id: Some(id),
            } => PlayerExpr::target(bind_target(
                cx.program,
                cx.slots,
                &id,
                TargetDomain::OpponentPlayer,
            )?),
            _ => return Err(operand("player", "target_opponent:qualified")),
        },
        who => lower_player_ref(cx.program, cx.slots, who, cx.context, None)?,
    };
    if random && !player.is_target() {
        return Err(step("discard", "random_recipient"));
    }
    Ok(Instruction::discard(
        player,
        amount,
        random,
        cx.source_block,
    ))
}
