//! "Draw" (CR 121).
use super::*;

/// CR 121.1–121.2c: a count read once and applied one card at a time in APNAP order.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(cx: &mut StepCx<'_>, draw: &ast::EffectDraw) -> Result<Instruction, Failure> {
    let ast::EffectDraw {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        who,
        amount,
    } = draw;
    if starting_with.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
    {
        return Err(step("draw", "qualified"));
    }
    check_optional_chooser("draw", optional.is_some(), who, decided_by.as_deref())?;
    let player = match who.as_ref() {
        ast::PlayerRef::Owner(owner) => PlayerExpr::OwnerOf {
            object: kir::Ref::Target {
                slot: bind_permanent_target(cx.program, cx.slots, &owner.of)?,
            },
        },
        who => pilot_player(cx, who)?,
    };
    let amount = match for_each {
        Some(filter) => lower_each_filter((**filter).clone(), amount, cx.context)?,
        None => pilot_amount(cx, amount)?,
    };
    let verb = Verb::Draw { player, amount };
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
