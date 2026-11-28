//! "Exile" (CR 406.2, 400.3, 400.7).
use super::*;

/// Every member of an "all"/"each" set, one announced permanent or
/// graveyard card, or every card in the graveyard of an announced player
/// (CR 404.1, 406.2). "You may" offers the controller the choice as the
/// step resolves (CR 608.2d).
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(cx: &mut StepCx<'_>, exile: &ast::EffectExile) -> Result<Instruction, Failure> {
    let ast::EffectExile {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        what,
        from,
        who,
        face_down,
        counters,
        pile,
    } = exile;
    if starting_with.is_some()
        || for_each.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
    {
        return Err(step("exile", "qualified"));
    }
    // CR 406.3, 708: a face-down or countered exile, a pile, another
    // exiling player or a printed origin zone has no reviewed lowering.
    if face_down.is_some() || counters.is_some() || pile.is_some() {
        return Err(step("exile", "exiled_state"));
    }
    if who.is_some() || decided_by.is_some() {
        return Err(step("exile", "mover"));
    }
    if from.is_some() {
        return Err(step("exile", "from"));
    }
    uncounted(cx)?;
    let movement = exile_object(cx, what)?;
    Ok(match optional {
        None => movement,
        Some(_) => optional_step(cx, movement)?,
    })
}

fn exile_object(cx: &mut StepCx<'_>, what: &ast::ObjectRef) -> Result<Instruction, Failure> {
    if let ast::ObjectRef::ZoneCards(zone) = what {
        let ast::ObjectRefZoneCards { zone, owner } = &**zone;
        if *zone != ast::ObjectRefZoneCardsZone::Graveyard {
            return Err(operand("exile_zone", zone.as_str()));
        }
        let zone = match &**owner {
            ast::PlayerRef::EachPlayer(_) => kir::ZoneExpr::Graveyards,
            ast::PlayerRef::TargetPlayer(target) => {
                let ast::PlayerRefTargetPlayer {
                    count: None,
                    up_to: None,
                    random: None,
                    selection_id: Some(id),
                } = &**target
                else {
                    return Err(operand("player", "target_player:qualified"));
                };
                kir::ZoneExpr::Graveyard {
                    of: PlayerExpr::target(bind_target(
                        cx.program,
                        cx.slots,
                        id,
                        TargetDomain::AnyPlayer,
                    )?),
                }
            }
            other => return Err(operand("exile_zone_owner", variant(other))),
        };
        return Ok(cx.act(Verb::Move {
            what: SetExpr::Matching {
                zone,
                filter: Pred::default(),
            },
            to: Destination::Exile,
        }));
    }
    if is_set(what) {
        let what = match graveyard_set(cx, what)? {
            Some(set) => set,
            None => lower_board_set(what, cx.context)?,
        };
        return Ok(cx.act(Verb::Move {
            what,
            to: Destination::Exile,
        }));
    }
    let slot = if matches!(what, ast::ObjectRef::Target(t) if t.filter.zone.is_some()) {
        bind_graveyard_target(cx.program, cx.slots, what, false)?
    } else {
        bind_permanent_target(cx.program, cx.slots, what)?
    };
    Ok(Instruction::move_target(
        slot,
        kir::Destination::Exile,
        cx.source_block,
    ))
}

/// CR 603.5, 608.2d: a printed "you may" around one announced movement:
/// the target is announced with the ability and the controller chooses
/// at resolution.
pub(super) fn optional_step(
    cx: &StepCx<'_>,
    movement: Instruction,
) -> Result<Instruction, Failure> {
    let Instruction::Kernel { node, source_block } = movement;
    if kir::target_slots(&node).len() != 1 || node.verb().is_none() {
        return Err(step("exile", "optional"));
    }
    Ok(Instruction::Kernel {
        node: kir::Node::Optional {
            chooser: PlayerExpr::you(cx.context),
            node: Box::new(node),
        },
        source_block,
    })
}
