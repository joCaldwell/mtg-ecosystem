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
    } = damage;
    if starting_with.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
        || unpreventable.is_some()
        || excess_to.is_some()
        || (decided_by.is_some()
            && !(optional.is_some()
                && matches!(decided_by.as_deref(), Some(ast::PlayerRef::You(_)))))
    {
        return Err(step("damage", "qualified"));
    }
    let source = match source.as_ref() {
        ast::ObjectRef::SelfObject(_) => Ref::Source,
        ast::ObjectRef::It(_) if cx.context == Context::Triggered => Ref::EventObject {
            role: cx
                .player_binding
                .event_object_role()
                .ok_or_else(|| step("damage", "unbound source"))?,
        },
        ast::ObjectRef::It(_) if cx.context == Context::Activated => Ref::Source,
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
    let amount = if let Some(filter) = for_each {
        lower_each_filter((**filter).clone(), amount, cx.context)?
    } else {
        pilot_amount(cx, amount)?
    };
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
        ast::DamageTarget::Object(to) => recipient_object(cx, &to.r#ref)?,
        ast::DamageTarget::Player(to) => recipient_player(cx, &to.r#ref)?,
        ast::DamageTarget::Them(_) => kir::DamageTo::Players {
            players: cx
                .player_binding
                .event_role()
                .map(PlayerExpr::event)
                .ok_or_else(|| operand("damage_recipient", "unbound event player"))?,
        },
        ast::DamageTarget::Each(to) => {
            let mut parts = to
                .refs
                .iter()
                .map(|part| match part {
                    ast::AmountCounterCountOn::PlayerRef(player) => recipient_player(cx, player),
                    ast::AmountCounterCountOn::ObjectRef(object) => recipient_object(cx, object),
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

fn recipient_object(
    cx: &mut StepCx<'_>,
    object: &ast::ObjectRef,
) -> Result<kir::DamageTo, Failure> {
    if is_set(object) {
        return Ok(kir::DamageTo::Objects {
            set: pilot_board_set(cx, object)?,
        });
    }
    let who = if matches!(object, ast::ObjectRef::Target(_)) {
        Ref::Target {
            slot: bind_damage_object_target(cx.program, cx.slots, object)?,
        }
    } else {
        pilot_object(cx, object)?
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
