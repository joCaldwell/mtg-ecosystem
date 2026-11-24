//! "Exile" (CR 406.2, 400.3, 400.7).
use super::*;

/// Every member of an "all"/"each" set, one announced permanent, or every
/// card in the graveyard of an announced player (CR 404.1, 406.2).
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(cx: &mut StepCx<'_>, what: &serde_json::Value) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let what = object(what)?;
    if let ast::ObjectRef::ZoneCards(zone) = &what {
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
    if is_set(&what) {
        return Ok(cx.act(Verb::Move {
            what: lower_board_set(&what, cx.context)?,
            to: Destination::Exile,
        }));
    }
    Ok(Instruction::move_target(
        bind_permanent_target(cx.program, cx.slots, &what)?,
        kir::Destination::Exile,
        cx.source_block,
    ))
}
