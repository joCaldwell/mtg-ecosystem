//! "Sacrifice" (CR 701.21).
use super::*;

/// CR 701.21a: the controller, each player or each opponent sacrifices a
/// fixed number of qualifying permanents. A damaged or announced player
/// can be instructed to sacrifice one.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: &PlayerSelector,
    what: &FilterObject<Count, ast::ObjectFilter>,
    chosen_by: Option<&PlayerSelector>,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    // When no chooser is printed, CR 701.21a still requires the
    // player instructed to sacrifice to choose the permanents they control.
    if chosen_by.is_some_and(|chooser| chooser != who)
        || who.of.is_some()
        || what.reference != "filter"
        || what.count.amount != "fixed"
        || what.count.value == 0
        || (!matches!(who.player.as_str(), "you" | "each-player" | "each-opponent")
            && !fixed_one(&what.count))
    {
        return Err(step("sacrifice", "chooser_or_cardinality"));
    }
    let player = match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::ThatPlayer(_) if cx.player_binding == PlayerBinding::DamageRecipient => {
            PlayerExpr::event(kir::EventPlayerRole::DamagedPlayer)
        }
        ast::PlayerRef::ThatPlayer(_)
            if cx.player_binding.event_role() == Some(kir::EventPlayerRole::ActivePlayer) =>
        {
            PlayerExpr::event(kir::EventPlayerRole::ActivePlayer)
        }
        ast::PlayerRef::You(_) => PlayerExpr::you(cx.context),
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
    let mut source_filter = what.filter.clone();
    let other = source_filter.other.take();
    let greatest_power = greatest_power_filter(&mut source_filter)?;
    let mut filter = lower_sacrifice_filter(&source_filter, cx.context)?;
    match other {
        Some(true) => filter.0.push(Atom::IsNot {
            object: Ref::Source,
        }),
        Some(false) => return Err(operand("sacrifice_filter", "other:false")),
        None => {}
    }
    Ok(cx.act(Verb::Sacrifice {
        player,
        count: Amount::fixed(i64::from(what.count.value)),
        filter,
        greatest_power,
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

// CR 608.2h: the archive repeats the population in the extremum and in
// allOf. Both must name the same creatures controlled by the chooser.
fn greatest_power_filter(filter: &mut ast::ObjectFilter) -> Result<bool, Failure> {
    let Some([property]) = filter.properties.as_deref() else {
        return Ok(false);
    };
    let ast::Amount::AttributeExtremum(extremum) = property.comparison.value.as_ref() else {
        return Ok(false);
    };
    if property.property != ast::PropertyConstraintProperty::Power
        || property.comparison.op != ast::ComparisonOp::Eq
        || extremum.attribute != ast::ObjectFilterImplicitComparisonAttribute::Power
        || extremum.extremum != ast::AmountAttributeExtremumExtremum::Maximum
        || filter.all_of.as_deref() != Some(std::slice::from_ref(extremum.among.as_ref()))
    {
        return Err(operand("sacrifice_filter", "greatest_power_population"));
    }
    let mut population = extremum.among.as_ref().clone();
    filter.all_of = None;
    filter.properties = None;
    if population.control.take() != Some(ast::ObjectFilterControl::ThatPlayer)
        || population != *filter
    {
        return Err(operand("sacrifice_filter", "greatest_power_population"));
    }
    Ok(true)
}
