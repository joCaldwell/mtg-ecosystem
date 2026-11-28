//! Damage recipients and sources (CR 120.2b, 120.3, 608.2h).
use super::*;

/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    damage: &ast::EffectDamage,
) -> Result<Instruction, Failure> {
    let ast::EffectDamage {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        source,
        amount,
        to,
        unpreventable,
        excess_to,
        excess_condition,
    } = damage;
    if starting_with.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
        || unpreventable.is_some()
        || excess_to.is_some()
        || excess_condition.is_some()
        || (decided_by.is_some()
            && !(optional.is_some()
                && matches!(decided_by.as_deref(), Some(ast::PlayerRef::You(_)))))
    {
        return Err(step("damage", "qualified"));
    }
    // CR 608.2c: a pronoun source is the first step's event object, which
    // the trigger validator proved; an activation's is its sacrificed self.
    let source = match source.as_ref() {
        ast::ObjectRef::SelfObject(_) => Ref::Source,
        ast::ObjectRef::It(_) if cx.context == Context::Triggered => {
            match antecedent::resolve(cx, source) {
                Ok(event @ Ref::EventObject { .. }) => event,
                _ => return Err(step("damage", "unbound source")),
            }
        }
        ast::ObjectRef::It(_) if cx.context == Context::Activated => {
            antecedent::record(cx, "it", Ref::Source)
        }
        ast::ObjectRef::This(this)
            if cx.context != Context::Spell
                && matches!(
                    this.noun.as_str(),
                    "creature" | "artifact" | "enchantment" | "land" | "permanent" | "planeswalker"
                ) =>
        {
            Ref::Source
        }
        _ => return Err(step("damage", "source")),
    };
    // CR 608.2c: "they control" in a count can refer to the single
    // explicitly targeted recipient. Only declare the recipient early
    // when this count needs it; unrelated targets keep printed order.
    let relative = match to.as_ref() {
        ast::DamageTarget::Player(to)
            if matches!(
                to.r#ref.as_ref(),
                ast::PlayerRef::TargetPlayer(_) | ast::PlayerRef::TargetOpponent(_)
            ) && (for_each.as_deref().is_some_and(names_controller)
                || count_names_controller(amount)) =>
        {
            Some(pilot_player(cx, &to.r#ref)?)
        }
        _ => None,
    };
    // CR 608.2c: "its" in the amount is the object dealing the damage.
    cx.subject = Some(source);
    let amount = if let Some(filter) = for_each {
        pilot_each_filter(cx, filter, amount, relative.as_ref())
    } else {
        pilot_amount_relative(cx, amount, relative.as_ref())
    };
    cx.subject = None;
    let amount = amount?;
    let to = match to.as_ref() {
        ast::DamageTarget::AnyTarget(target) if target.random.is_none() => kir::DamageTo::Ref {
            who: Ref::Target {
                slot: bind_target(
                    cx.program,
                    cx.slots,
                    target.selection_id.as_deref().unwrap_or(""),
                    TargetDomain::AnyDamageTarget,
                )?,
            },
        },
        ast::DamageTarget::Object(to) => recipient_object(cx, &to.r#ref, None)?,
        ast::DamageTarget::Player(to) => match relative {
            Some(players) => kir::DamageTo::Players { players },
            None => recipient_player(cx, &to.r#ref)?,
        },
        ast::DamageTarget::Them(_) => kir::DamageTo::Players {
            players: cx
                .player_binding
                .event_role()
                .map(PlayerExpr::event)
                .ok_or_else(|| operand("damage_recipient", "unbound event player"))?,
        },
        ast::DamageTarget::Each(to) => {
            // CR 608.2c: "each opponent and each permanent they control"
            // supplies a relative player domain to the second conjunct.
            // Only the exact pair gives a unique preceding player mention.
            use ast::AmountCounterCountOn::{ObjectRef, PlayerRef};
            let relative_player = match to.refs.as_slice() {
                [PlayerRef(player), ObjectRef(_)]
                    if matches!(&**player, ast::PlayerRef::EachOpponent(_)) =>
                {
                    Some(PlayerExpr::EachOpponent)
                }
                _ => None,
            };
            let mut parts = to
                .refs
                .iter()
                .map(|part| match part {
                    PlayerRef(player) => recipient_player(cx, player),
                    ObjectRef(object) => recipient_object(cx, object, relative_player.as_ref()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if parts.len() == 1 {
                parts.pop().expect("one damage recipient")
            } else {
                kir::DamageTo::All { parts }
            }
        }
        to => return Err(operand("damage_recipient", variant(to))),
    };
    let verb = Verb::Damage { source, to, amount };
    if optional.is_some() {
        Ok(Instruction::Kernel {
            node: kir::Node::Optional {
                chooser: PlayerExpr::you(cx.context),
                node: Box::new(kir::Node::Do(verb)),
            },
            source_block: cx.source_block,
        })
    } else {
        Ok(cx.act(verb))
    }
}

fn names_controller(filter: &ast::ObjectFilter) -> bool {
    matches!(
        filter.controlled_by.as_deref(),
        Some(ast::PlayerRef::They(_))
    )
}

fn count_names_controller(amount: &ast::Amount) -> bool {
    match amount {
        ast::Amount::Count(count) => names_controller(&count.of),
        ast::Amount::Twice(twice) => count_names_controller(&twice.of),
        _ => false,
    }
}

fn recipient_object(
    cx: &mut StepCx<'_>,
    object: &ast::ObjectRef,
    relative_player: Option<&PlayerExpr>,
) -> Result<kir::DamageTo, Failure> {
    if is_set(object) {
        return Ok(kir::DamageTo::Objects {
            set: pilot_board_set(cx, object, relative_player)?,
        });
    }
    let who = match object {
        ast::ObjectRef::Target(_) => Ref::Target {
            slot: bind_damage_object_target(cx.program, cx.slots, object)?,
        },
        ast::ObjectRef::It(_) | ast::ObjectRef::That(_) => antecedent::permanent(cx, object)?,
        _ => pilot_object(cx, object)?,
    };
    Ok(kir::DamageTo::Ref { who })
}

fn recipient_player(
    cx: &mut StepCx<'_>,
    player: &ast::PlayerRef,
) -> Result<kir::DamageTo, Failure> {
    let players = pilot_player(cx, player)?;
    Ok(match players {
        PlayerExpr::Is {
            who: who @ Ref::Target { .. },
        } => kir::DamageTo::Ref { who },
        players => kir::DamageTo::Players { players },
    })
}
