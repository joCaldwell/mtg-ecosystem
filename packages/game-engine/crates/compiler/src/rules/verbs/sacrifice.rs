//! "Sacrifice" (CR 701.21).
use super::*;

/// CR 701.21a: one player sacrifices one creature of their choice: the
/// damaged player, each player, each opponent, or one announced opponent.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: &PlayerSelector,
    what: &FilterObject,
    chosen_by: &PlayerSelector,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if who != chosen_by
        || who.of.is_some()
        || what.reference != "filter"
        || !fixed_one(&what.count)
        || what.filter
            != ObjectFilter::Types(Filter {
                types: vec!["creature".into()],
            })
    {
        return Err(step("sacrifice", "chooser_or_cardinality"));
    }
    let player = match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::ThatPlayer(_) if cx.player_binding == PlayerBinding::DamageRecipient => {
            PlayerExpr::event(kir::EventPlayerRole::DamagedPlayer)
        }
        ast::PlayerRef::EachPlayer(_) => PlayerExpr::EachPlayer,
        ast::PlayerRef::EachOpponent(_) => PlayerExpr::EachOpponent,
        // CR 601.2c / 608.2d / 701.21: the opponent target is bound at
        // announcement; that player selects a controlled creature only when
        // the sacrifice instruction resolves.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        ast::PlayerRef::TargetOpponent(target) => match *target {
            ast::PlayerRefTargetOpponent {
                count: None,
                up_to: None,
                random: None,
                selection_id,
            } => PlayerExpr::target(bind_target(
                cx.program,
                cx.slots,
                selection_id.as_deref().unwrap_or(""),
                TargetDomain::OpponentPlayer,
            )?),
            _ => return Err(operand("player", "target_opponent:qualified")),
        },
        other => return Err(operand("player", variant(&other))),
    };
    Ok(Instruction::sacrifice_creature(player, cx.source_block))
}
