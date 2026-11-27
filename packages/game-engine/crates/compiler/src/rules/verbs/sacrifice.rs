//! "Sacrifice" (CR 701.21).
use super::*;

/// CR 701.21a: a player sacrifices one qualifying permanent of their choice: the
/// damaged player, each player, each opponent, or one announced player.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: &PlayerSelector,
    what: &FilterObject<Count, ast::ObjectFilter>,
    chosen_by: &PlayerSelector,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if who != chosen_by || who.of.is_some() || what.reference != "filter" || !fixed_one(&what.count)
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
        ast::PlayerRef::TargetPlayer(target) => match *target {
            ast::PlayerRefTargetPlayer {
                count: None,
                up_to: None,
                random: None,
                selection_id: Some(id),
            } if !id.is_empty() => PlayerExpr::target(bind_target(
                cx.program,
                cx.slots,
                &id,
                TargetDomain::AnyPlayer,
            )?),
            _ => return Err(operand("player", "target_player:qualified")),
        },
        other => return Err(operand("player", variant(&other))),
    };
    Ok(cx.act(Verb::Sacrifice {
        player,
        count: Amount::fixed(1),
        filter: lower_sacrifice_filter(&what.filter, cx.context)?,
    }))
}

// CR 506.4 and 509.1g: a disjunction of combat statuses is tested on
// the current permanent. The rest of the conjunction uses board terms.
fn lower_sacrifice_filter(filter: &ast::ObjectFilter, context: Context) -> Result<Pred, Failure> {
    if let Some([group]) = filter.all_of.as_deref()
        && present_fields(group) == ["any_of"]
        && let Some(alternatives) = group.any_of.as_deref()
        && alternatives.len() >= 2
        && alternatives
            .iter()
            .all(|arm| present_fields(arm) == ["status"])
    {
        let alternatives = alternatives
            .iter()
            .map(|arm| match arm.status.as_deref() {
                Some([status]) if status == "attacking" => Ok(Pred(vec![Atom::Attacking])),
                Some([status]) if status == "blocking" => Ok(Pred(vec![Atom::Blocking])),
                _ => Err(operand("sacrifice_filter", "combat_status")),
            })
            .collect::<Result<_, _>>()?;
        let mut base = filter.clone();
        base.all_of = None;
        let mut predicate = lower_filter(&base, context)?;
        predicate.0.push(Atom::AnyOf { alternatives });
        return Ok(predicate);
    }
    lower_filter(filter, context)
}
