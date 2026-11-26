//! "Draw" (CR 121).
use super::*;

/// CR 121.1-121.2c: draw one card at a time, in APNAP order for each player.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: &PlayerSelector,
    amount: &DrawAmount,
    optional: bool,
    for_each: Option<&serde_json::Value>,
) -> Result<Instruction, Failure> {
    if optional {
        return Err(step("draw", "optional"));
    }
    let bound = cx.bound_x();
    // CR 121.2, 608.2h: "draw a card for each [filter]" is one counted
    // sequence of single draws by the controller, counted once.
    if let Some(for_each) = for_each {
        let player = lower_player_ref(
            cx.program,
            cx.slots,
            typed::<ast::PlayerRef>(who, "player")?,
            cx.context,
            cx.player_binding.event_role(),
        )?;
        let (None, DrawAmount::Fixed { value }) = (&bound, amount) else {
            return Err(operand("forEach", "per_object_amount"));
        };
        if !player.is_you() {
            return Err(step("draw", "for_each_recipient"));
        }
        let per_object = ast::Amount::Fixed(Box::new(ast::AmountFixed {
            value: i64::from(*value),
        }));
        return Ok(cx.act(Verb::Draw {
            player,
            amount: lower_for_each(for_each, &per_object, cx.context)?,
        }));
    }
    let player = match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::EachPlayer(_) => PlayerExpr::EachPlayer,
        ast::PlayerRef::Owner(owner)
            if cx.context == Context::Spell
                && bound.is_none()
                && matches!(amount, DrawAmount::Fixed { value: 2 }) =>
        {
            let slot = bind_permanent_target(cx.program, cx.slots, &owner.of)?;
            if !matches!(
                cx.program.instructions.last().and_then(Instruction::verb),
                Some(Verb::Move {
                    what: SetExpr::One { object: kir::Ref::Target { slot: moved } },
                    to: Destination::OwnerLibraryShuffled,
                }) if *moved == slot
            ) {
                return Err(step("draw", "owner_of_unbound_target"));
            }
            return Ok(cx.act(Verb::Draw {
                player: PlayerExpr::OwnerOf {
                    object: kir::Ref::Target { slot },
                },
                amount: Amount::fixed(2),
            }));
        }
        who => lower_player_ref(
            cx.program,
            cx.slots,
            who,
            cx.context,
            cx.player_binding.event_role(),
        )?,
    };
    let counted = player.is_you() || player == PlayerExpr::EachPlayer;
    // CR 107.3, 608.2h: a bound X is counted as the controller draws.
    if let Some(x) = bound {
        return match (amount, player.is_you()) {
            (DrawAmount::X {}, true) => Ok(cx.act(Verb::Draw { player, amount: x })),
            _ => Err(step("draw", "recipient_or_count")),
        };
    }
    match (amount, &player) {
        (DrawAmount::Fixed { value: 1..=2 }, PlayerExpr::Is { .. }) => {
            // CR 121.2: one ordinary instruction for each single draw.
            if matches!(amount, DrawAmount::Fixed { value: 2 }) {
                cx.program.instructions.push(Instruction::draw(
                    player.clone(),
                    Quantity::from(1),
                    cx.source_block,
                ));
            }
            Ok(Instruction::draw(
                player,
                Quantity::from(1),
                cx.source_block,
            ))
        }
        (DrawAmount::Fixed { value }, _) if counted && *value > 0 => Ok(cx.act(Verb::Draw {
            player,
            amount: Amount::fixed(i64::from(*value)),
        })),
        (DrawAmount::X {}, _) if counted => Ok(cx.act(Verb::Draw {
            player,
            amount: Amount::announced_x(),
        })),
        (DrawAmount::Fixed { .. } | DrawAmount::X {}, _) => Err(step("draw", "recipient_or_count")),
        (DrawAmount::ThatMuch {}, _) => Err(operand("amount", variant(amount))),
    }
}
