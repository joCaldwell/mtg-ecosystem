//! "Sacrifice" (CR 701.21).
use super::*;

/// CR 701.21a: one designated permanent, or a choice among the
/// qualifying permanents each instructed player controls.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    sacrifice: &ast::EffectSacrifice,
) -> Result<Instruction, Failure> {
    let ast::EffectSacrifice {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        who,
        what,
        chosen_by,
    } = sacrifice;
    if starting_with.is_some()
        || for_each.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
        || chosen_by
            .as_deref()
            .is_some_and(|chooser| chooser != who.as_ref())
    {
        return Err(step("sacrifice", "chooser_or_distribution"));
    }
    check_optional_chooser("sacrifice", optional.is_some(), who, decided_by.as_deref())?;
    if matches!(
        &**who,
        ast::PlayerRef::EachPlayer(_) | ast::PlayerRef::EachOpponent(_)
    ) && matches!(&**what, ast::ObjectRef::All(set) if set.filter.controlled_by.is_some())
    {
        return Err(step("sacrifice", "all_per_player_actuals"));
    }
    let player = pilot_player(cx, who)?;
    let ast::ObjectRef::Filter(selection) = what.as_ref() else {
        if !matches!(
            what.as_ref(),
            ast::ObjectRef::It(_)
                | ast::ObjectRef::That(_)
                | ast::ObjectRef::SelfObject(_)
                | ast::ObjectRef::This(_)
        ) {
            return Err(step("sacrifice", "subject"));
        }
        let object = match what.as_ref() {
            ast::ObjectRef::It(_) | ast::ObjectRef::That(_) => antecedent::permanent(cx, what)?,
            _ => pilot_object(cx, what)?,
        };
        return Ok(wrap(
            cx,
            Verb::Sacrifice {
                player,
                count: Amount::fixed(1),
                filter: Pred(vec![Atom::Is { object }]),
                greatest_power: false,
            },
            optional.is_some(),
        ));
    };
    if selection.up_to.is_some()
        || selection.group_constraints.is_some()
        || selection.random.is_some()
    {
        return Err(step("sacrifice", "choice"));
    }
    let count = pilot_amount(cx, &selection.count)?;
    let mut source_filter = (*selection.filter).clone();
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
    Ok(wrap(
        cx,
        Verb::Sacrifice {
            player,
            count,
            filter,
            greatest_power,
        },
        optional.is_some(),
    ))
}

fn wrap(cx: &StepCx<'_>, verb: Verb, optional: bool) -> Instruction {
    if optional {
        Instruction::Kernel {
            node: kir::Node::Optional {
                chooser: PlayerExpr::you(cx.context),
                node: Box::new(kir::Node::Do(verb)),
            },
            source_block: cx.source_block,
        }
    } else {
        cx.act(verb)
    }
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
