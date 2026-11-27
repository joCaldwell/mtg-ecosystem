//! "Gain life" and "lose life" (CR 119).
use super::*;

/// CR 107.1b, 119.3, 608.2h: read the quantity and apply simultaneous life changes.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    life: &ast::EffectVariant99,
) -> Result<Instruction, Failure> {
    let ast::EffectVariant99 {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        effect,
        who,
        amount,
    } = life;
    if decided_by.is_some()
        || starting_with.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
    {
        return Err(step(effect.as_str(), "qualified"));
    }
    let gain = *effect == ast::ReplacementEventVariant4Event::GainLife;
    let player = match who.as_ref() {
        ast::PlayerRef::Controller(controller) => {
            let slot = preceding_target(cx.program, cx.source_block, &controller.of)?;
            PlayerExpr::ControllerOf {
                object: kir::Ref::Target { slot },
            }
        }
        who => pilot_player(cx, who)?,
    };
    let amount = match (for_each, amount.as_ref()) {
        (Some(filter), amount) => lower_each_filter((**filter).clone(), amount, cx.context)?,
        (None, ast::Amount::Variant(quantity)) => {
            let ast::AmountVariant::LifeChange(change) = quantity.as_ref() else {
                return Err(operand("amount", variant(quantity)));
            };
            return linked_change(cx, gain, player, change, optional.is_some());
        }
        (None, ast::Amount::Attribute(attribute))
            if attribute.attribute == ast::PropertyConstraintProperty::Power
                && matches!(who.as_ref(), ast::PlayerRef::Controller(controller) if controller.of == attribute.of) =>
        {
            let PlayerExpr::ControllerOf { object } = player else {
                unreachable!()
            };
            Amount::StatOf {
                object,
                stat: kir::Stat::Power,
            }
        }
        (None, ast::Amount::Attribute(attribute))
            if !gain && attribute.attribute == ast::PropertyConstraintProperty::ManaValue =>
        {
            graveyard_return_mana_value(cx, &attribute.of)?
        }
        (None, amount) => pilot_amount(cx, amount)?,
    };
    Ok(wrap(cx, action(gain, player, amount), optional.is_some()))
}

fn action(gain: bool, player: PlayerExpr, amount: Amount) -> Verb {
    if gain {
        Verb::GainLife { player, amount }
    } else {
        Verb::LoseLife { player, amount }
    }
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

// R5 prints the polarity and "this way" but supplies no explicit reference
// edge. Only the immediately preceding unguarded life action of that
// polarity in the same block supplies this projection.
fn linked_change(
    cx: &mut StepCx<'_>,
    gain: bool,
    player: PlayerExpr,
    change: &ast::AmountVariantLifeChange,
    optional: bool,
) -> Result<Instruction, Failure> {
    use ast::{ObjectRelationEventResultScope::ThisWay, PlayerFilterLifeChangeChange as Change};
    let producer = cx
        .program
        .instructions
        .last()
        .filter(|previous| previous.source_block() == cx.source_block);
    if optional
        || change.scope != ThisWay
        || change.who.is_some()
        || !matches!(
            (change.change, producer.and_then(Instruction::verb)),
            (Change::Lost, Some(Verb::LoseLife { .. }))
                | (Change::Gained, Some(Verb::GainLife { .. }))
        )
    {
        return Err(operand("amount", "no dominating life-change projection"));
    }
    let Instruction::Kernel { node, .. } = cx.program.instructions.pop().expect("checked producer");
    let amount = Amount::Var {
        of: kir::Ref::StepResult {
            step: 0,
            role: kir::StepRole::Amount,
        },
    };
    Ok(Instruction::Kernel {
        node: kir::Node::Seq(vec![node, kir::Node::Do(action(gain, player, amount))]),
        source_block: cx.source_block,
    })
}

/// CR 202.3, 608.2h: "that card" can read the graveyard target's mana
/// value after the immediately preceding move has changed its incarnation.
fn graveyard_return_mana_value(cx: &StepCx<'_>, what: &ast::ObjectRef) -> Result<Amount, Failure> {
    let [target] = cx.program.targets.as_slice() else {
        return Err(operand("amount", "graveyard_return_target"));
    };
    let object = kir::Ref::Target { slot: target.slot };
    if !cx.program.optional_target
        && matches!(target.domain, TargetDomain::GraveyardCard { .. })
        && matches!(what, ast::ObjectRef::That(that)
            if that.plural.is_none()
                && present_fields(&that.filter) == ["cls"]
                && that.filter.cls == Some(ast::ObjectFilterCls::Card))
        && cx.program.instructions.last().is_some_and(|previous| {
            previous.source_block() == cx.source_block
                && matches!(previous.verb(), Some(Verb::Move {
                    what: SetExpr::One { object: moved },
                    to: Destination::Battlefield { .. },
                }) if *moved == object)
        })
    {
        return Ok(Amount::StatOf {
            object,
            stat: kir::Stat::ManaValue,
        });
    }
    Err(operand("amount", "graveyard_return_target"))
}
